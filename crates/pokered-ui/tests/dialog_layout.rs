//! Pixel-level regression tests for the overworld dialog box layout.
//!
//! The game renders with the Fusion Pixel 10px font (Latin 5px, CJK 10px
//! advance) on the original 20×18 grid of 8×8 tiles. Text wraps to fill the
//! 144px box interior — ~28 Latin or 14 CJK characters per line — and must
//! never cross the box's right border.

use pokered_data::ui_layout::schema::DIALOG_DEFAULT_LAYOUT;
use pokered_renderer::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;
use pokered_core::game_state::Lang;
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::{menus, Ui};

fn render_dialog(text: &str, lang: Lang) -> FrameBuffer {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    {
        let mut painter = FrameBufferPainter::new(&mut fb).with_lang(lang);
        let mut ui = Ui::new(&mut painter);
        menus::dialog::draw(text, true, &DIALOG_DEFAULT_LAYOUT, &mut ui, lang);
    }
    fb
}

/// True if any ink pixel appears inside the 1-tile right border column of the
/// standard 20×6 dialog box at (0,12) — tile column 19, pixel x 152..159,
/// interior rows (y 104..135). The VERTICAL_RIGHT border glyph inks x 157..158
/// (bitmap 0b00000110 → cols 5,6), so only x 155..156 is checked: ink there
/// means text ran into the border padding.
fn text_bleeds_into_right_border(fb: &FrameBuffer) -> bool {
    for y in 104..136 {
        for x in 155..157 {
            if let Some(px) = fb.get_pixel(x, y) {
                if px == Rgba::INK_BLACK || px == Rgba::BLACK {
                    return true;
                }
            }
        }
    }
    false
}

/// A typical zh dialogue page: two script-authored short lines joined the way
/// the overworld renderer joins page lines. With pixel wrapping the two short
/// lines are re-flowed into fuller lines rather than staying half-empty.
fn zh_page(joiner: &str) -> String {
    let line1 = "你好世界这是第一行对话哟"; // 12 chars
    let line2 = "第二行也写满了十三个字"; // 11 chars
    format!("{}{}{}", line1, joiner, line2)
}

#[test]
fn zh_dialog_stays_inside_box() {
    for (joiner, tag) in [("\n", "nl"), (" ", "sp")] {
        let fb = render_dialog(&zh_page(joiner), Lang::Zh);
        fb.save_png(std::path::Path::new(&format!("/tmp/dialog_zh_{}.png", tag))).ok();
        assert!(
            !text_bleeds_into_right_border(&fb),
            "zh dialog text (joiner {:?}) must not cross the box's right border",
            joiner
        );
    }
}

#[test]
fn zh_dialog_wraps_long_unbroken_text() {
    // 25 full-width chars with no line break — must wrap at the 144px
    // interior (14 full-width chars), not at the old 13-char cap.
    let text = "这是一段没有换行的超长中文对话内容用来测试自动换行";
    let fb = render_dialog(text, Lang::Zh);
    assert!(
        !text_bleeds_into_right_border(&fb),
        "long zh dialog text must wrap inside the box"
    );
}

#[test]
fn en_dialog_stays_inside_box() {
    let text = "Hello there!\nWelcome to the world of POKéMON! This is a long line that should wrap.";
    let fb = render_dialog(text, Lang::En);
    fb.save_png(std::path::Path::new("/tmp/dialog_en.png")).ok();
    assert!(
        !text_bleeds_into_right_border(&fb),
        "en dialog text must not cross the box's right border"
    );
}

#[test]
fn en_dialog_line_fills_box() {
    // The original 18-char-per-line authoring left the box half empty with
    // the 5px Latin font (18 × 5px = 90px of a 144px interior). A 27-char
    // line (135px) must stay on one line and fill the box.
    let text = "What will CHARIZARD do now?";
    assert_eq!(dotzuki_renderer::embedded_font::measure_text(text), 135);
    let fb = render_dialog(text, Lang::En);
    // Ink must reach past the old 90px limit (tile 12) without crossing the
    // right border — proving the line wraps at the pixel width, not the
    // character count. Scan the first text line's rows only (the box's
    // bottom-right ▼ arrow lives at row 16).
    let mut reached_past_90px = false;
    for y in 104..128 {
        for x in 100..152 {
            if let Some(px) = fb.get_pixel(x, y) {
                if px == Rgba::INK_BLACK || px == Rgba::BLACK {
                    reached_past_90px = true;
                }
            }
        }
    }
    assert!(reached_past_90px, "a 135px line must extend past the old 90px limit");
    assert!(!text_bleeds_into_right_border(&fb), "…but never cross the right border");
}
