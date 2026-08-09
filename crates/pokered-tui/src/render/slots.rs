//! Renderer for the Game Corner slot-machine screen.
//!
//! A readable text/tile presentation (per the task's guidance that a simpler
//! layout is acceptable): three reel windows showing the visible symbols, the
//! coin/bet HUD, and a status line. The reel/flag/payout logic lives entirely
//! in `pokered_core::slots_screen` (shared with the native app; this is a
//! pixel-for-pixel mirror of the app's `render/slots.rs`).

use pokered_core::slots_screen::{symbol_label, SlotsPhase, SlotsScreen};
use pokered_renderer::embedded_font::{draw_text, draw_text_scaled};
use pokered_renderer::{FrameBuffer, Rgba};

use super::draw_text_box;

const BG: Rgba = Rgba::WHITE;
const FG: Rgba = Rgba::BLACK;

fn fill_rect(fb: &mut FrameBuffer, x: u32, y: u32, w: u32, h: u32, color: Rgba) {
    for py in y..(y + h).min(fb.height()) {
        for px in x..(x + w).min(fb.width()) {
            fb.set_pixel(px, py, color);
        }
    }
}

fn outline_rect(fb: &mut FrameBuffer, x: u32, y: u32, w: u32, h: u32, color: Rgba) {
    fill_rect(fb, x, y, w, 1, color);
    fill_rect(fb, x, y + h.saturating_sub(1), w, 1, color);
    fill_rect(fb, x, y, 1, h, color);
    fill_rect(fb, x + w.saturating_sub(1), y, 1, h, color);
}

/// Draw the whole slots screen to the 160x144 framebuffer.
pub fn draw_slots(slots: &SlotsScreen, fb: &mut FrameBuffer) {
    fb.clear(BG);

    // Title.
    draw_text("SLOT MACHINE", 40, 6, FG, fb);

    // Three reel windows. Each shows top / middle / bottom symbols. The middle
    // row is the 1-coin payline, so highlight it.
    let reel_w = 40;
    let reel_h = 54;
    let gap = 4;
    let total_w = reel_w * 3 + gap * 2;
    let start_x = (fb.width() - total_w) / 2;
    let reel_y = 22;

    for i in 0..3usize {
        let rx = start_x + i as u32 * (reel_w + gap);
        outline_rect(fb, rx, reel_y, reel_w, reel_h, FG);

        let view = slots.machine.get_wheel_view(i);
        let spinning = matches!(slots.phase, SlotsPhase::Spinning) && !slots.reels_stopped[i];

        // Rows: top, middle (payline), bottom.
        let row_labels = [
            symbol_label(view.top),
            symbol_label(view.middle),
            symbol_label(view.bottom),
        ];
        for (r, label) in row_labels.iter().enumerate() {
            let ty = reel_y + 6 + r as u32 * 16;
            if r == 1 {
                // Highlight the center payline.
                fill_rect(fb, rx + 1, ty - 2, reel_w - 2, 13, FG);
                let text = if spinning { "----" } else { label };
                draw_text(text, rx + 6, ty, BG, fb);
            } else {
                let text = if spinning { "----" } else { label };
                draw_text(text, rx + 6, ty, FG, fb);
            }
        }
    }

    // HUD: coins + current bet.
    let hud_y = reel_y + reel_h + 6;
    draw_text(&format!("COINS {:>4}", slots.coins), 8, hud_y, FG, fb);
    let bet_text = format!("BET  {}", slots.bet);
    draw_text(&bet_text, 108, hud_y, FG, fb);

    // Status message box near the bottom.
    let box_y = 118;
    draw_text_box(fb, 0, box_y, 17, 1, FG);
    draw_text(&slots.message, 10, box_y + 10, FG, fb);

    // Contextual hint line just under the reels.
    let hint = match slots.phase {
        SlotsPhase::BetSelect => "UP/DN:BET  A:SPIN  B:EXIT",
        SlotsPhase::Spinning => "A: STOP REEL",
        SlotsPhase::Result => "A: CONTINUE",
    };
    draw_text(hint, 8, hud_y + 14, FG, fb);

    // On a win, flash the payout large in the center for readability.
    if matches!(slots.phase, SlotsPhase::Result) && slots.last_payout > 0 {
        let txt = format!("+{}", slots.last_payout);
        let scale = 2;
        let approx_w = txt.len() as u32 * 6 * scale;
        let x = (fb.width().saturating_sub(approx_w)) / 2;
        draw_text_scaled(&txt, x, reel_y + reel_h / 2 - 8, scale, FG, fb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render_config::RenderConfig;
    use pokered_core::slots_screen::SlotsInput;

    /// Rendering must not panic in any phase (guards against coordinate
    /// under/overflow in the manual rect drawing).
    #[test]
    fn draw_slots_does_not_panic_in_all_phases() {
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::BLACK);
        let mut s = SlotsScreen::new(false, 100, 1);
        draw_slots(&s, &mut fb); // BetSelect
        s.update_frame(SlotsInput { a: true, ..SlotsInput::none() });
        draw_slots(&s, &mut fb); // Spinning
        for _ in 0..2000 {
            if s.phase != SlotsPhase::Spinning {
                break;
            }
            s.update_frame(SlotsInput { a: true, ..SlotsInput::none() });
        }
        draw_slots(&s, &mut fb); // Result
    }
}
