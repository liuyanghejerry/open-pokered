//! Game Corner cabinet, using the original Red tilemap and reel tiles.

use crate::alloc_prelude::*;
#[cfg(not(target_os = "none"))]
use std::sync::OnceLock;

use pokered_core::game_state::Lang;
use pokered_core::slots_screen::{PayoutStage, SlotsPhase, SlotsScreen};
use pokered_data::slot_machine::{SLOT_MACHINE_WHEEL1, SLOT_MACHINE_WHEEL2, SLOT_MACHINE_WHEEL3};
use pokered_data::ui_text::zh_slots_message;
use pokered_renderer::embedded_font::{draw_text, measure_text};
use pokered_renderer::{FrameBuffer, Rgba};

use super::draw_text_box;

const MAP: &[u8] = include_bytes!("../../../../gfx/slots/slots.tilemap");

#[cfg(not(target_os = "none"))]
fn tiles() -> &'static (image::RgbImage, image::RgbImage) {
    static TILES: OnceLock<(image::RgbImage, image::RgbImage)> = OnceLock::new();
    TILES.get_or_init(|| {
        let decode = |bytes| {
            image::load_from_memory(bytes)
                .expect("embedded slots PNG")
                .to_rgb8()
        };
        (
            decode(include_bytes!("../../../../gfx/slots/red_slots_1.png")),
            decode(include_bytes!("../../../../gfx/slots/red_slots_2.png")),
        )
    })
}

#[cfg(not(target_os = "none"))]
fn tile(fb: &mut FrameBuffer, sheet: &image::RgbImage, id: u8, x: u32, y: u32, flash: bool) {
    let tx = (id as u32 % (sheet.width() / 8)) * 8;
    let ty = (id as u32 / (sheet.width() / 8)) * 8;
    for dy in 0..8 {
        for dx in 0..8 {
            let [r, g, b] = sheet.get_pixel(tx + dx, ty + dy).0;
            // The original XORs BGP with $40: only color 3 changes to 2.
            // Reel sprites use OBP0 and are unaffected by the background flash.
            let color = if flash && r == 0 && g == 0 && b == 0 {
                Rgba::new(85, 85, 85, 255)
            } else {
                Rgba::new(r, g, b, 255)
            };
            fb.set_pixel(x + dx, y + dy, color);
        }
    }
}

// ── Bare metal (GBA) ──────────────────────────────────────────────────────────
//
// The slot sheets are 4-level grayscale PNGs, so the 2bpp round-trip through
// the pre-converted registry is EXACT: sample s (2-bit) → gray (s*85) →
// registry color index (3 - s) → gray ((3 - idx) * 85). The flash rule above
// maps pure black (idx 3) to the color-2 gray (85).

#[cfg(target_os = "none")]
mod gba {
    use crate::alloc_prelude::*;
    use alloc::vec::Vec;

    use pokered_renderer::resource::{AssetRoot, ResourceManager};
    use pokered_renderer::FrameBuffer;

    use super::Rgba;

    /// One slots sheet as per-pixel GB color indices (0..=3).
    pub struct SlotsSheet {
        pub w: u32,
        pub h: u32,
        idx: Vec<u8>,
    }

    impl SlotsSheet {
        pub fn width(&self) -> u32 {
            self.w
        }

        pub fn color_index(&self, x: u32, y: u32) -> u8 {
            if x < self.w && y < self.h {
                self.idx[(y * self.w + x) as usize]
            } else {
                0
            }
        }
    }

    fn sheet(rm: &mut ResourceManager, name: &str) -> SlotsSheet {
        let cached = rm
            .load_slots(name)
            .expect("slots sheet in pre-converted registry");
        let (w, h) = cached.source_size;
        let tpr = (w / 8) as usize;
        let mut idx = vec![0u8; (w * h) as usize];
        for i in 0..cached.tileset.len() {
            let (tx, ty) = (i % tpr, i / tpr);
            let tile = cached.tileset.get(i);
            for r in 0..8usize {
                for c in 0..8usize {
                    let px = (tx * 8 + c) as u32;
                    let py = (ty * 8 + r) as u32;
                    idx[(py * w + px) as usize] = tile.get(r, c);
                }
            }
        }
        SlotsSheet { w, h, idx }
    }

    pub fn tiles() -> &'static (SlotsSheet, SlotsSheet) {
        // Single-threaded GBA: build once per process.
        // thumbv4t has no atomics; single-threaded GBA builds once per process.
        static mut TILES: Option<(SlotsSheet, SlotsSheet)> = None;
        unsafe {
            let slot = &mut *core::ptr::addr_of_mut!(TILES);
            slot.get_or_insert_with(|| {
                let mut rm = ResourceManager::new(AssetRoot::new());
                (sheet(&mut rm, "red_slots_1"), sheet(&mut rm, "red_slots_2"))
            })
        }
    }

    pub fn tile(fb: &mut FrameBuffer, sheet: &SlotsSheet, id: u8, x: u32, y: u32, flash: bool) {
        let tpr = sheet.width() / 8;
        let tx = (id as u32 % tpr) * 8;
        let ty = (id as u32 / tpr) * 8;
        for dy in 0..8 {
            for dx in 0..8 {
                let idx = sheet.color_index(tx + dx, ty + dy);
                let gray = (3 - idx as u32) * 85;
                // Original rule: flash turns pure black into the color-2 gray.
                let color = if flash && idx == 3 {
                    Rgba::new(85, 85, 85, 255)
                } else {
                    Rgba::new(gray as u8, gray as u8, gray as u8, 255)
                };
                fb.set_pixel(x + dx, y + dy, color);
            }
        }
    }
}

#[cfg(target_os = "none")]
use gba::{tile, tiles};

/// Wrap by measured glyph width so both English and Chinese stay in the box.
fn message_lines(text: &str, width: u32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for ch in text.chars() {
        let candidate = format!("{line}{ch}");
        if measure_text(&candidate) > width && !line.is_empty() {
            // Prefer word boundaries in English, retaining the partial word.
            if let Some(space) = line.rfind(' ') {
                let rest = line[space + 1..].to_owned();
                lines.push(line[..space].to_owned());
                line = rest;
            } else {
                lines.push(core::mem::take(&mut line));
            }
        }
        if ch != ' ' || !line.is_empty() {
            line.push(ch);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Draw the original 20×12-tile cabinet above the dialogue box.
pub fn draw_slots(slots: &SlotsScreen, fb: &mut FrameBuffer, lang: Lang) {
    let (background, symbols) = tiles();
    let flash = slots.phase == SlotsPhase::Payout
        && slots.payout_stage == PayoutStage::Flash
        && slots.flash_on;
    let fg = if flash {
        Rgba::new(85, 85, 85, 255)
    } else {
        Rgba::BLACK
    };
    fb.clear(Rgba::WHITE);
    for (i, &id) in MAP.iter().enumerate() {
        tile(
            fb,
            background,
            id,
            (i % 20) as u32 * 8,
            (i / 20) as u32 * 8,
            flash,
        );
    }

    // AnimWheel emits six pairs of 8×8 sprites, bottom to top, at OAM
    // ($30/$50/$70, $58). Offsets point one byte past the displayed tiles.
    // An odd offset therefore displays three complete symbols; an even one
    // displays two complete symbols between two half symbols.
    for (i, wheel) in [
        SLOT_MACHINE_WHEEL1,
        SLOT_MACHINE_WHEEL2,
        SLOT_MACHINE_WHEEL3,
    ]
    .iter()
    .enumerate()
    {
        let offset = (slots.machine.wheel_offsets[i] as usize + 29) % 30;
        for row in 0..6 {
            let byte = offset + row;
            let symbol = wheel[byte / 2];
            let id = if byte % 2 == 0 {
                symbol.low_byte()
            } else {
                symbol.high_byte()
            };
            let x = 40 + i as u32 * 32;
            let y = 72 - row as u32 * 8;
            tile(fb, symbols, id, x, y, false);
            tile(fb, symbols, id + 1, x + 8, y, false);
        }
    }
    // Light the center line for one coin, outer horizontal lines for two,
    // and diagonals for three (SlotMachine_LightBalls).
    if slots.phase != SlotsPhase::BetSelect {
        for (row, required) in [(2, 3), (4, 2), (6, 1), (8, 2), (10, 3)] {
            if slots.bet >= required {
                for x in [24, 128] {
                    tile(fb, background, 0x14, x, row * 8, flash);
                    tile(fb, background, 0x15, x, (row + 1) * 8, flash);
                }
            }
        }
    }
    // Original credit / payout fields: (5,1) and (11,1), four digits.
    for (value, x) in [(slots.coins, 40), (slots.payout_remaining, 88)] {
        for (i, digit) in format!("{value:04}").chars().enumerate() {
            draw_text(&digit.to_string(), x + i as u32 * 8, 8, fg, fb);
        }
    }

    draw_text_box(fb, 0, 96, 18, 4, fg);
    let is_zh = lang == Lang::Zh;
    let text = zh_slots_message(&slots.message, is_zh);
    let width = if slots.phase == SlotsPhase::BetSelect {
        96
    } else {
        144
    };
    for (i, line) in message_lines(&text, width).iter().take(3).enumerate() {
        draw_text(line, 8, 104 + i as u32 * 10, fg, fb);
    }
    if slots.phase == SlotsPhase::Result && slots.coins > 0 {
        draw_text(
            if is_zh {
                "A：继续  B：退出"
            } else {
                "A: YES  B: NO"
            },
            8,
            124,
            fg,
            fb,
        );
    }
    if slots.phase == SlotsPhase::BetSelect {
        draw_text_box(fb, 112, 88, 4, 5, fg);
        for (i, bet) in [3, 2, 1].iter().enumerate() {
            let y = 96 + i as u32 * 16;
            draw_text(&format!("×{bet}"), 128, y, fg, fb);
            if slots.bet == *bet {
                draw_text(">", 120, y, fg, fb);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render_config::RenderConfig;
    use pokered_core::slots_screen::SlotsInput;

    fn render(s: &SlotsScreen) -> FrameBuffer {
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        draw_slots(s, &mut fb, Lang::En);
        fb
    }

    fn region(fb: &FrameBuffer, x: u32, y: u32, w: u32, h: u32) -> Vec<[u8; 4]> {
        (y..y + h)
            .flat_map(|py| (x..x + w).map(move |px| fb.get_pixel(px, py).unwrap().to_array()))
            .collect()
    }

    #[test]
    fn reel_motion_and_flash_preserve_stopped_symbols() {
        let mut s = SlotsScreen::new(false, 100, 42);
        assert_eq!(s.machine.wheel_offsets, [29; 3]);
        s.phase = SlotsPhase::Spinning;
        s.reels_stopped[0] = true;
        let before = render(&s);
        s.update_frame(SlotsInput::none());
        let after = render(&s);
        assert_eq!(
            region(&before, 40, 32, 16, 48),
            region(&after, 40, 32, 16, 48)
        );
        assert_ne!(
            region(&before, 72, 32, 16, 48),
            region(&after, 72, 32, 16, 48)
        );
        s.phase = SlotsPhase::Payout;
        s.payout_stage = PayoutStage::Flash;
        s.flash_on = false;
        let normal = render(&s);
        s.flash_on = true;
        let flash = render(&s);
        assert_eq!(
            region(&normal, 40, 32, 16, 48),
            region(&flash, 40, 32, 16, 48)
        );
        assert_ne!(
            region(&normal, 0, 16, 24, 80),
            region(&flash, 0, 16, 24, 80)
        );
    }

    #[test]
    fn localized_win_messages_fit_dialogue_box() {
        for lang in [false, true] {
            let text = zh_slots_message("CHERRY lined up! Scored 300 coins!", lang);
            let lines = message_lines(&text, 144);
            assert!(lines.len() <= 3);
            assert!(lines.iter().all(|line| measure_text(line) <= 144));
        }
    }

    /// Rendering must not panic in any phase (guards against coordinate
    /// under/overflow in the manual rect drawing).
    #[test]
    fn draw_slots_does_not_panic_in_all_phases() {
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::BLACK);
        let mut s = SlotsScreen::new(false, 100, 1);
        draw_slots(&s, &mut fb, Lang::En); // BetSelect
        s.update_frame(SlotsInput {
            a: true,
            ..SlotsInput::none()
        });
        draw_slots(&s, &mut fb, Lang::En); // Spinning (warm-up)
        for _ in 0..20000 {
            if s.phase != SlotsPhase::Spinning && s.phase != SlotsPhase::Payout {
                break;
            }
            s.update_frame(SlotsInput {
                a: true,
                ..SlotsInput::none()
            });
            draw_slots(&s, &mut fb, Lang::En); // covers Spinning + Payout frames
        }
        draw_slots(&s, &mut fb, Lang::En); // Result
    }
}
