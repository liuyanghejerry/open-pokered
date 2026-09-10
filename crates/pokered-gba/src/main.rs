#![no_std]
#![no_main]

extern crate alloc;

#[cfg(feature = "autopilot")]
mod autopilot;

use agb::input::{Button, ButtonController};
use pokered_app::game::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_renderer::input::{GbButton, InputState};
use pokered_renderer::palette::GbColor;
use pokered_renderer::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;

/// Route the game crates' `log` output to the mGBA debug console.
struct GbaLogger;

impl log::Log for GbaLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        agb::println!("[{}] {}", record.level(), record.args());
    }
    fn flush(&self) {}
}

static LOGGER: GbaLogger = GbaLogger;

// ── GBA MMIO video: mode 3 (240x160, 16bpp) ───────────────────────────
const SCREEN_W: usize = 240;
const SCREEN_H: usize = 160;
// pokered's 160x144 viewport, centered.
const VIEW_X: usize = (SCREEN_W - 160) / 2; // 40
const VIEW_Y: usize = (SCREEN_H - 144) / 2; // 8

#[inline]
fn vram() -> &'static mut [u16] {
    unsafe { core::slice::from_raw_parts_mut(0x0600_0000 as *mut u16, SCREEN_W * SCREEN_H) }
}

#[inline]
fn set_display_control(mode: u16) {
    unsafe { core::ptr::write_volatile(0x0400_0000 as *mut u16, mode) }
}

/// RGB888 → RGB555.
const fn rgb15(r: u8, g: u8, b: u8) -> u16 {
    ((r >> 3) as u16) | (((g >> 3) as u16) << 5) | (((b >> 3) as u16) << 10)
}

// ── getrandom custom backend (no OS entropy on bare metal) ────────────
// Seeds from agb's global RNG mixed with a call counter — deterministic
// per boot, which is fine for a single-player RPG (the original hardware
// seeds from the rDIV timer the same way). thumbv4t has no atomics, and
// this only runs on the single-threaded game loop, so a plain static is
// sound.
static mut CALL_COUNT: u32 = 0;

getrandom::register_custom_getrandom!(custom_getrandom);

fn custom_getrandom(dest: &mut [u8]) -> Result<(), getrandom::Error> {
    let mut i = 0;
    while i < dest.len() {
        let n = unsafe {
            let c = CALL_COUNT;
            CALL_COUNT = c.wrapping_add(1);
            c
        };
        let z = (agb::rng::next_i32() as u32) ^ n.wrapping_mul(0x9E37_79B9);
        let bytes = z.to_le_bytes();
        let take = core::cmp::min(4, dest.len() - i);
        dest[i..i + take].copy_from_slice(&bytes[..take]);
        i += take;
    }
    Ok(())
}

// ── Frame presentation: packed 2bpp + display palette → RGB555 ────────
fn present(fb: &FrameBuffer) {
    // Build the 4-entry LUT from the live display palette (fades remap it).
    let palette = fb.display_palette();
    let mut lut = [0u16; 4];
    for (i, slot) in lut.iter_mut().enumerate() {
        let c = palette.color(GbColor::from_u8(i as u8));
        *slot = rgb15(c.r, c.g, c.b);
    }

    let packed = fb.packed();
    let vram = vram();
    let border = lut[3];

    // Top/bottom letterbox rows.
    for y in 0..VIEW_Y {
        let row = &mut vram[y * SCREEN_W..(y + 1) * SCREEN_W];
        row.fill(border);
    }
    for y in VIEW_Y + 144..SCREEN_H {
        let row = &mut vram[y * SCREEN_W..(y + 1) * SCREEN_W];
        row.fill(border);
    }
    // Viewport rows: left bar, 160 px from 2bpp, right bar.
    for y in 0..144 {
        let dst_row = &mut vram[(VIEW_Y + y) * SCREEN_W..(VIEW_Y + y + 1) * SCREEN_W];
        dst_row[..VIEW_X].fill(border);
        dst_row[VIEW_X + 160..].fill(border);
        let src_row = &packed[y * 40..(y + 1) * 40];
        let out = &mut dst_row[VIEW_X..VIEW_X + 160];
        for (byte_i, &b) in src_row.iter().enumerate() {
            out[byte_i * 4] = lut[(b & 0b11) as usize];
            out[byte_i * 4 + 1] = lut[((b >> 2) & 0b11) as usize];
            out[byte_i * 4 + 2] = lut[((b >> 4) & 0b11) as usize];
            out[byte_i * 4 + 3] = lut[((b >> 6) & 0b11) as usize];
        }
    }
}

/// The BIOS vblank counter at 0x03007FFC (incremented once per frame).
#[inline]
fn bios_vblank_count() -> u32 {
    unsafe { core::ptr::read_volatile(0x0300_7FFC as *const u32) }
}

// ── EWRAM main stack ───────────────────────────────────────────────────
// The BIOS pins the main stack at the top of IWRAM (~31 KiB usable), which
// is far too small for the pokered game loop (frame locals + renderer
// scratch approach 40 KiB). The IRQ stack (0x03007FFC) is separate and
// untouched. 64 KiB of EWRAM bss backs the relocated main stack; the game
// state (~29 KiB) also lives in EWRAM, leaving ~160 KiB of heap.
static mut EWRAM_STACK: [u32; 16384] = [0; 16384]; // 64 KiB

/// Run `f` on the EWRAM stack. `f` never returns, so the switch is final.
#[inline(never)]
unsafe fn run_on_ewram_stack(f: fn() -> !) -> ! {
    unsafe {
        // Byte-exact top of EWRAM_STACK (64 KiB), 8-byte aligned.
        let base = core::ptr::addr_of_mut!(EWRAM_STACK) as usize;
        let new_sp = (base + EWRAM_STACK.len() * 4) & !0b111;
        let old_sp: usize;
        core::arch::asm!(
            "mov {old}, sp",
            "mov sp, {new}",
            old = out(reg) old_sp,
            new = in(reg) new_sp,
        );
        let _ = old_sp;
        f()
    }
}

#[agb::entry]
fn main(_gba: agb::Gba) -> ! {
    unsafe { run_on_ewram_stack(game_main) }
}

fn game_main() -> ! {
    let vblank = agb::interrupt::VBlank::get();
    let mut input = ButtonController::new();

    set_display_control(0x0403); // mode 3 + BG2
    let _ = unsafe { log::set_logger_racy(&LOGGER) };
    unsafe { log::set_max_level_racy(log::LevelFilter::Info) };

    agb::println!("pokered-gba: booting game core…");

    // PokemonGame is ~29 KB — nearly the whole IWRAM stack budget — so it
    // lives in an EWRAM static (bss), not on the stack.
    static mut GAME: Option<PokemonGame> = None;
    let game: &mut PokemonGame = unsafe {
        let slot = &mut *core::ptr::addr_of_mut!(GAME);
        slot.get_or_insert_with(|| PokemonGame::new_for_gba(GameVersion::Red))
    };
    agb::println!("pokered-gba: game constructed");

    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut frame: u32 = 0;

    loop {
        let mut state = InputState::new();
        #[cfg(feature = "autopilot")]
        {
            let held = autopilot::buttons_at(frame);
            if held & 1 != 0 {
                state.press(GbButton::A);
            }
            if held & 2 != 0 {
                state.press(GbButton::B);
            }
            if held & 8 != 0 {
                state.press(GbButton::Start);
            }
            if held & 4 != 0 {
                state.press(GbButton::Select);
            }
            if held & 64 != 0 {
                state.press(GbButton::Up);
            }
            if held & 128 != 0 {
                state.press(GbButton::Down);
            }
            if held & 32 != 0 {
                state.press(GbButton::Left);
            }
            if held & 16 != 0 {
                state.press(GbButton::Right);
            }
        }
        #[cfg(not(feature = "autopilot"))]
        {
            input.update();
            if input.is_pressed(Button::A) {
                state.press(GbButton::A);
            }
            if input.is_pressed(Button::B) {
                state.press(GbButton::B);
            }
            if input.is_pressed(Button::Start) {
                state.press(GbButton::Start);
            }
            if input.is_pressed(Button::Select) {
                state.press(GbButton::Select);
            }
            if input.is_pressed(Button::Up) {
                state.press(GbButton::Up);
            }
            if input.is_pressed(Button::Down) {
                state.press(GbButton::Down);
            }
            if input.is_pressed(Button::Left) {
                state.press(GbButton::Left);
            }
            if input.is_pressed(Button::Right) {
                state.press(GbButton::Right);
            }
        }

        let vb0 = bios_vblank_count();
        vblank.wait_for_vblank();
        let vb1 = bios_vblank_count();
        game.update(&state);
        let vb2 = bios_vblank_count();
        game.draw(&mut fb);
        let vb3 = bios_vblank_count();
        present(&fb);
        let vb4 = bios_vblank_count();
        if frame % 120 == 0 {
            agb::println!(
                "prof f{} wait={} upd={} draw={} present={}",
                frame,
                vb1.wrapping_sub(vb0),
                vb2.wrapping_sub(vb1),
                vb3.wrapping_sub(vb2),
                vb4.wrapping_sub(vb3)
            );
        }

        // Debug: mirror the packed 2bpp framebuffer into SRAM so mGBA's
        // .sav file carries a decodable snapshot. SRAM needs byte-wide
        // volatile writes.
        if frame == 2 || frame % 60 == 0 {
            let packed = fb.packed();
            let mut hdr = alloc::vec::Vec::with_capacity(8);
            hdr.extend_from_slice(b"FBDP");
            hdr.extend_from_slice(&(frame as u32).to_le_bytes());
            unsafe {
                let base = 0x0E00_0000 as *mut u8;
                for (i, &b) in hdr.iter().chain(packed.iter()).enumerate() {
                    if i >= 0x8000 {
                        break;
                    }
                    core::ptr::write_volatile(base.add(i), b);
                }
            }
        }

        frame = frame.wrapping_add(1);
        if frame == 1 { agb::println!("pokered-gba: first frame done"); }
        if frame % 10 == 0 {
            agb::println!(
                "pokered-gba: frame {} screen={:?}",
                frame,
                game.state.screen
            );
        }
    }
}
