//! Renderer for the Game Corner slot-machine screen.
//!
//! A readable text/tile presentation (per the task's guidance that a simpler
//! layout is acceptable): three reel windows showing the visible symbols, the
//! coin/bet HUD, and a status line. The reel/flag/payout logic lives entirely
//! in `pokered_core::slots_screen` (shared with the native app; this is a
//! pixel-for-pixel mirror of the app's `render/slots.rs`).

use pokered_core::game_state::Lang;
use pokered_core::slots_screen::{symbol_label, SlotsPhase, SlotsScreen};
use pokered_data::lang_data;
use pokered_data::ui_text::{zh_slot_symbol, zh_slots_message};
use pokered_renderer::embedded_font::{draw_text, draw_text_scaled, measure_text};
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
pub fn draw_slots(slots: &SlotsScreen, fb: &mut FrameBuffer, lang: Lang) {
    let is_zh = lang == Lang::Zh;
    fb.clear(BG);

    // Title.
    let title = lang_data::ui_label("SLOT MACHINE", is_zh);
    draw_text(title, (fb.width().saturating_sub(measure_text(title))) / 2, 6, FG, fb);

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
            zh_slot_symbol(symbol_label(view.top)),
            zh_slot_symbol(symbol_label(view.middle)),
            zh_slot_symbol(symbol_label(view.bottom)),
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
    draw_text(&format!("{} {:>4}", lang_data::ui_label("COINS", is_zh), slots.coins), 8, hud_y, FG, fb);
    let bet_text = format!("{}  {}", lang_data::ui_label("BET", is_zh), slots.bet);
    draw_text(&bet_text, 108, hud_y, FG, fb);

    // Status message box near the bottom.
    let box_y = 118;
    draw_text_box(fb, 0, box_y, 17, 1, FG);
    draw_text(&zh_slots_message(&slots.message, is_zh), 10, box_y + 10, FG, fb);

    // Contextual hint line just under the reels.
    let hint = match slots.phase {
        SlotsPhase::BetSelect => {
            if is_zh { "上/下：下注  A：开始  B：退出" } else { "UP/DN:BET  A:SPIN  B:EXIT" }
        }
        SlotsPhase::Spinning => {
            if is_zh { "A：停止转轮" } else { "A: STOP REEL" }
        }
        SlotsPhase::Result => {
            if is_zh { "A：继续" } else { "A: CONTINUE" }
        }
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
        use pokered_core::game_state::Lang;
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::BLACK);
        let mut s = SlotsScreen::new(false, 100, 1);
        draw_slots(&s, &mut fb, Lang::En); // BetSelect
        s.update_frame(SlotsInput { a: true, ..SlotsInput::none() });
        draw_slots(&s, &mut fb, Lang::En); // Spinning
        for _ in 0..2000 {
            if s.phase != SlotsPhase::Spinning {
                break;
            }
            s.update_frame(SlotsInput { a: true, ..SlotsInput::none() });
        }
        draw_slots(&s, &mut fb, Lang::En); // Result
        draw_slots(&s, &mut fb, Lang::Zh); // zh must render without panic too
        if std::env::var("POKERED_TUI_DEBUG_SHOTS").as_deref() == Ok("1") {
            let _ = fb.save_png(std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../target/tui-slots-zh.png"
            )));
        }
    }
}
