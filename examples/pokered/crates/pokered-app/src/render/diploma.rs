//! Renderer for the full-screen diploma (completed-POKeDEX certificate).
//!
//! A readable text presentation of the original Diploma screen: the bordered
//! certificate with "Diploma", the player name, the congratulations text and
//! the "GAME FREAK" signature.

use pokered_core::game_state::Lang;
use pokered_data::lang_data;
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::{FrameBuffer, Rgba};

const BG: Rgba = Rgba::WHITE;
const FG: Rgba = Rgba::BLACK;

/// Draw the diploma certificate to the 160x144 framebuffer.
pub fn draw_diploma(player_name: &str, fb: &mut FrameBuffer, lang: Lang) {
    let is_zh = lang == Lang::Zh;
    fb.clear(BG);

    // Certificate border.
    for x in 2..fb.width().saturating_sub(2) {
        fb.set_pixel(x, 4, FG);
        fb.set_pixel(x, fb.height() - 5, FG);
    }
    for y in 4..fb.height().saturating_sub(4) {
        fb.set_pixel(2, y, FG);
        fb.set_pixel(fb.width() - 3, y, FG);
    }

    draw_text(lang_data::ui_label("Diploma", is_zh), 62, 16, FG, fb);
    draw_text(player_name, 62, 32, FG, fb);
    if is_zh {
        // "Congrats! This diploma certifies that you have completed your
        // POKeDEX." — line-wrapped at ≤13 CJK glyphs per line.
        draw_text("恭喜！这份文凭", 34, 56, FG, fb);
        draw_text("证明你已完成了", 34, 72, FG, fb);
        draw_text("宝可梦图鉴！", 34, 88, FG, fb);
    } else {
        draw_text("Congrats! This", 34, 56, FG, fb);
        draw_text("diploma certifies", 34, 70, FG, fb);
        draw_text("that you have", 34, 84, FG, fb);
        draw_text("completed your", 34, 98, FG, fb);
        draw_text("POKeDEX.", 34, 112, FG, fb);
    }

    // Company signature — kept verbatim.
    draw_text("GAME FREAK", 56, 128, FG, fb);
}
