#![no_std]
#![no_main]

extern crate alloc;

#[cfg(feature = "autopilot")]
mod autopilot;

use agb::input::{Button, ButtonController};
use dotzuki_engine::render_config::RenderConfig;
use pokered_app::game::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::{GameScreen, Lang};
use pokered_core::gamefreak_splash::SplashPhase;
use pokered_renderer::input::{GbButton, InputState};
use pokered_renderer::palette::GbColor;
use pokered_renderer::{FrameBuffer, Rgba};

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

// ── GBA MMIO video: mode 4 (240x160, paletted 8bpp) ───────────────────
const SCREEN_W: usize = 240;
const SCREEN_H: usize = 160;
// pokered's 160x144 viewport, centered.
const VIEW_X: usize = (SCREEN_W - 160) / 2; // 40
const VIEW_Y: usize = (SCREEN_H - 144) / 2; // 8
const MODE4_BG2: u16 = 0x0404;
const PAGE_SELECT: u16 = 0x0010;
const MODE4_PAGE_BYTES: usize = SCREEN_W * SCREEN_H;
const MODE4_PAGE_WORDS: usize = MODE4_PAGE_BYTES / 4;
const BORDER_INDEX: u32 = 0x0303_0303;
const DMA3_SOURCE: *mut u32 = 0x0400_00D4 as *mut u32;
const DMA3_DESTINATION: *mut u32 = 0x0400_00D8 as *mut u32;
const DMA3_CONTROL: *mut u32 = 0x0400_00DC as *mut u32;
const DMA_ENABLE: u32 = 1 << 31;
const DMA_32BIT: u32 = 1 << 26;
const VCOUNT: *const u16 = 0x0400_0006 as *const u16;

#[inline]
fn mode4_page(page: u8) -> *mut u32 {
    let offset = page as usize * 0xA000;
    (0x0600_0000 + offset) as *mut u32
}

#[inline]
fn set_display_control(mode: u16) {
    unsafe { core::ptr::write_volatile(0x0400_0000 as *mut u16, mode) }
}

#[inline]
fn is_vblank() -> bool {
    unsafe { core::ptr::read_volatile(VCOUNT) >= 160 }
}

/// Immediate DMA3 copy. GBA DMA is synchronous: the CPU resumes after all
/// words have reached VRAM and the enable bit has cleared.
#[inline]
unsafe fn dma3_copy_words(source: *const u32, destination: *mut u32, words: usize) {
    unsafe {
        core::ptr::write_volatile(DMA3_SOURCE, source as usize as u32);
        core::ptr::write_volatile(DMA3_DESTINATION, destination as usize as u32);
        core::ptr::write_volatile(DMA3_CONTROL, DMA_ENABLE | DMA_32BIT | words as u32);
    }
}

/// RGB888 → RGB555.
const fn rgb15(r: u8, g: u8, b: u8) -> u16 {
    ((r >> 3) as u16) | (((g >> 3) as u16) << 5) | (((b >> 3) as u16) << 10)
}

struct Mode4Presenter {
    draw_page: u8,
    ready_page: Option<u8>,
    ready_palette: [u16; 4],
}

impl Mode4Presenter {
    fn new(fb: &FrameBuffer) -> Self {
        // Both pages retain the fixed index-3 border; subsequent presents
        // only touch the centered 160x144 viewport.
        for page in 0..=1 {
            let dst = mode4_page(page);
            for i in 0..MODE4_PAGE_WORDS {
                unsafe { core::ptr::write_volatile(dst.add(i), BORDER_INDEX) };
            }
        }
        let palette = Self::palette(fb);
        Self::write_palette(&palette);
        set_display_control(MODE4_BG2);
        Self {
            draw_page: 1,
            ready_page: None,
            ready_palette: palette,
        }
    }

    #[inline]
    fn palette(fb: &FrameBuffer) -> [u16; 4] {
        let palette = fb.display_palette();
        let mut out = [0u16; 4];
        for (i, slot) in out.iter_mut().enumerate() {
            let c = palette.color(GbColor::from_u8(i as u8));
            *slot = rgb15(c.r, c.g, c.b);
        }
        out
    }

    #[inline]
    fn write_palette(palette: &[u16; 4]) {
        let dst = 0x0500_0000 as *mut u16;
        for (i, &color) in palette.iter().enumerate() {
            unsafe { core::ptr::write_volatile(dst.add(i), color) };
        }
    }

    /// Flip to the completed page at VBlank, then make the other page the
    /// next render target. The palette is global, so commit it with the page.
    #[inline]
    fn commit(&mut self) {
        let Some(page) = self.ready_page.take() else {
            return;
        };
        Self::write_palette(&self.ready_palette);
        set_display_control(MODE4_BG2 | if page == 1 { PAGE_SELECT } else { 0 });
        self.draw_page = page ^ 1;
    }

    /// DMA the centered 160x144 chunky index framebuffer into the hidden
    /// Mode 4 page. One transfer per row preserves the screen's 240px pitch.
    fn present(&mut self, fb: &FrameBuffer) {
        let indices = fb.indices();
        let dst = mode4_page(self.draw_page);
        for y in 0..144 {
            let src_word = unsafe { indices.as_ptr().add(y * 160).cast::<u32>() };
            let dst_word = ((VIEW_Y + y) * SCREEN_W + VIEW_X) / 4;
            unsafe { dma3_copy_words(src_word, dst.add(dst_word), 40) };
        }
        self.ready_palette = Self::palette(fb);
        self.ready_page = Some(self.draw_page);
    }
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

// ── Hardware frame clock and optional profiling ───────────────────────
// Timer 2 is free for general use in agb. At 16.78 MHz / 64, one tick is
// 3.815 us and the 16-bit counter spans 250 ms, enough for each frame phase.
const PROFILE_TIMER_DATA: *mut u16 = 0x0400_0108 as *mut u16;
const PROFILE_TIMER_CONTROL: *mut u16 = 0x0400_010A as *mut u16;
// One GBA video frame is exactly 280,896 CPU cycles.
const FRAME_TICKS: u32 = 280_896 / 64;

#[inline]
fn profile_timer_start() {
    unsafe {
        core::ptr::write_volatile(PROFILE_TIMER_CONTROL, 0);
        core::ptr::write_volatile(PROFILE_TIMER_DATA, 0);
        // Enable + 1/64 prescaler.
        core::ptr::write_volatile(PROFILE_TIMER_CONTROL, 0x0081);
    }
}

#[inline]
fn profile_now() -> u16 {
    unsafe { core::ptr::read_volatile(PROFILE_TIMER_DATA) }
}

#[cfg(feature = "profiling")]
#[derive(Default)]
struct ProfileSamples {
    wait: u32,
    update: u32,
    draw: u32,
    present: u32,
    dump: u32,
    total: u32,
    max_total: u16,
    frames: u32,
    renders: u32,
}

#[cfg(feature = "profiling")]
impl ProfileSamples {
    fn record(&mut self, marks: [u16; 6], rendered: bool) {
        let elapsed = |from: u16, to: u16| to.wrapping_sub(from);
        let wait = elapsed(marks[0], marks[1]);
        let update = elapsed(marks[1], marks[2]);
        let draw = elapsed(marks[2], marks[3]);
        let present = elapsed(marks[3], marks[4]);
        let dump = elapsed(marks[4], marks[5]);
        let total = elapsed(marks[0], marks[5]);
        self.wait += wait as u32;
        self.update += update as u32;
        if rendered {
            self.draw += draw as u32;
            self.present += present as u32;
        }
        self.dump += dump as u32;
        self.total += total as u32;
        self.max_total = self.max_total.max(total);
        self.frames += 1;
        self.renders += rendered as u32;
    }

    fn report_and_reset(&mut self, frame: u32) {
        let n = self.frames.max(1);
        let r = self.renders.max(1);
        agb::println!(
            "prof f{} ticks(avg) wait={} upd={} draw/render={} present/render={} dump={} total={} max={} renders={}",
            frame,
            self.wait / n,
            self.update / n,
            self.draw / r,
            self.present / r,
            self.dump / n,
            self.total / n,
            self.max_total,
            self.renders
        );
        *self = Self::default();
    }
}

// ── EWRAM main stack ───────────────────────────────────────────────────
// The BIOS pins the main stack at the top of IWRAM (~31 KiB usable), which
// is far too small for the pokered game loop (frame locals + renderer
// scratch approach 40 KiB). The IRQ stack (0x03007FFC) is separate and
// untouched. 64 KiB of EWRAM bss backs the relocated main stack; the game
// state (~29 KiB) also lives in EWRAM, leaving ~160 KiB of heap.
const EWRAM_STACK_WORDS: usize = 16384;
static mut EWRAM_STACK: [u32; EWRAM_STACK_WORDS] = [0; EWRAM_STACK_WORDS]; // 64 KiB

/// Run `f` on the EWRAM stack. `f` never returns, so the switch is final.
#[inline(never)]
unsafe fn run_on_ewram_stack(f: fn() -> !) -> ! {
    unsafe {
        // Byte-exact top of EWRAM_STACK (64 KiB), 8-byte aligned.
        let base = core::ptr::addr_of_mut!(EWRAM_STACK) as usize;
        let new_sp = (base + EWRAM_STACK_WORDS * 4) & !0b111;
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

    set_display_control(MODE4_BG2);
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
    let mut presenter = Mode4Presenter::new(&fb);
    profile_timer_start();
    let mut frame: u32 = 0;
    let mut last_clock = profile_now();
    let mut update_accumulator = FRAME_TICKS;
    let mut last_static_splash: Option<SplashPhase> = None;
    let mut last_language_select: Option<Lang> = None;
    #[cfg(feature = "profiling")]
    let mut profile = ProfileSamples::default();

    loop {
        let first_frame_pending = frame == 0;
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

        #[cfg(feature = "profiling")]
        let mark0 = profile_now();
        vblank.wait_for_vblank();
        // agb intentionally returns immediately when it notices that a slow
        // frame missed VBlank. A page flip must still happen in a real blank
        // interval or the upper and lower parts of the LCD can show different
        // pages, so wait for the next one in that case.
        if !is_vblank() {
            agb::display::busy_wait_for_vblank();
        }
        presenter.commit();
        #[cfg(feature = "profiling")]
        let mark1 = profile_now();
        let now = profile_now();
        update_accumulator += now.wrapping_sub(last_clock) as u32;
        last_clock = now;
        // Rendering can exceed one video frame. Catch the inexpensive game
        // simulation up to the hardware clock so animation and input timing
        // stay near 59.7 Hz instead of slowing down with the renderer.
        let mut updates = 0;
        while update_accumulator >= FRAME_TICKS && updates < 8 {
            game.update(&state);
            game.flush_deferred_transition();
            frame = frame.wrapping_add(1);
            update_accumulator -= FRAME_TICKS;
            updates += 1;
        }
        #[cfg(feature = "profiling")]
        let mark2 = profile_now();
        // The copyright, setup, and post-delay splash phases are completely
        // static. Keep the already-presented page while only advancing logic.
        let static_splash = if game.state.screen == GameScreen::GameFreakSplash
            && matches!(
                game.gamefreak_splash.phase,
                SplashPhase::BlackDelay | SplashPhase::Setup | SplashPhase::PostDelay
            ) {
            Some(game.gamefreak_splash.phase)
        } else {
            None
        };
        let language_select =
            (game.state.screen == GameScreen::LanguageSelect).then_some(game.state.config.language);
        let redraw = if static_splash.is_some() {
            static_splash != last_static_splash
        } else if language_select.is_some() {
            language_select != last_language_select
        } else {
            true
        };
        if redraw {
            game.draw(&mut fb);
        }
        #[cfg(feature = "profiling")]
        let mark3 = profile_now();
        if redraw {
            presenter.present(&fb);
        }
        last_static_splash = static_splash;
        last_language_select = language_select;
        #[cfg(feature = "profiling")]
        let mark4 = profile_now();

        // Debug: mirror the packed 2bpp framebuffer into SRAM so mGBA's
        // .sav file carries a decodable snapshot. SRAM needs byte-wide
        // volatile writes.
        #[cfg(feature = "framebuffer-dump")]
        if frame == 2 || frame % 60 == 0 {
            let mut hdr = [0u8; 8];
            hdr[..4].copy_from_slice(b"FBDP");
            hdr[4..].copy_from_slice(&frame.to_le_bytes());
            unsafe {
                let base = 0x0E00_0000 as *mut u8;
                for (i, &b) in hdr.iter().enumerate() {
                    core::ptr::write_volatile(base.add(i), b);
                }
                let indices = fb.indices();
                let mut out = hdr.len();
                for y in 0..144 {
                    for group in 0..20 {
                        let mut plane0 = 0u8;
                        let mut plane1 = 0u8;
                        for col in 0..8 {
                            let index = indices[y * 160 + group * 8 + col];
                            let bit = 7 - col;
                            plane0 |= (index & 1) << bit;
                            plane1 |= ((index >> 1) & 1) << bit;
                        }
                        core::ptr::write_volatile(base.add(out), plane0);
                        core::ptr::write_volatile(base.add(out + 1), plane1);
                        out += 2;
                    }
                }
            }
        }
        #[cfg(feature = "profiling")]
        {
            let mark5 = profile_now();
            profile.record([mark0, mark1, mark2, mark3, mark4, mark5], redraw);
            if profile.frames == 60 {
                profile.report_and_reset(frame);
            }
        }

        if first_frame_pending && updates > 0 {
            agb::println!("pokered-gba: first frame done");
        }
    }
}
