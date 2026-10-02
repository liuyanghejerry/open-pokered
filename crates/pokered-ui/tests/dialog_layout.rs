//! Pixel-level regression tests for the overworld dialog box layout.
//!
//! The game uses the original 8px Latin cells and Fusion Pixel 10px CJK
//! on the original 20×18 grid of 8×8 tiles. Text wraps inside the
//! 144px box interior — 18 Latin or 14 CJK characters per line — and must
//! never cross the box's right border.

use dotzuki_engine::render_config::RenderConfig;
use pokered_core::game_state::Lang;
use pokered_data::ui_layout::schema::DIALOG_DEFAULT_LAYOUT;
use pokered_renderer::{FrameBuffer, Rgba};
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

/// The rightmost border column must retain the original $7C tile. Compare
/// every pixel with its stencil, including the padding beside both strokes.
fn text_bleeds_into_right_border(fb: &FrameBuffer) -> bool {
    let glyph = pokered_renderer::embedded_font::box_tiles::VERTICAL_RIGHT;
    for y in 104..136 {
        for x in 152..160 {
            let ink = glyph[(y % 8) as usize] & (0x80 >> (x % 8)) != 0;
            let expected = if ink { Rgba::BLACK } else { Rgba::WHITE };
            if fb.get_pixel(x, y) != Some(expected) {
                return true;
            }
        }
    }
    false
}

/// A typical zh dialogue page: two script-authored short lines joined the way
/// the overworld renderer joins page lines. Authored hard breaks survive;
/// each row also wraps independently at the actual pixel width.
fn zh_page(joiner: &str) -> String {
    let line1 = "你好世界这是第一行对话哟"; // 12 chars
    let line2 = "第二行也写满了十三个字"; // 11 chars
    format!("{}{}{}", line1, joiner, line2)
}

#[test]
fn zh_dialog_stays_inside_box() {
    for (joiner, tag) in [("\n", "nl"), (" ", "sp")] {
        let fb = render_dialog(&zh_page(joiner), Lang::Zh);
        fb.save_png(std::path::Path::new(&format!("/tmp/dialog_zh_{}.png", tag)))
            .ok();
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
    let text =
        "Hello there!\nWelcome to the world of POKéMON! This is a long line that should wrap.";
    let fb = render_dialog(text, Lang::En);
    fb.save_png(std::path::Path::new("/tmp/dialog_en.png")).ok();
    assert!(
        !text_bleeds_into_right_border(&fb),
        "en dialog text must not cross the box's right border"
    );
}

#[test]
fn en_dialog_line_fills_box() {
    // Eighteen original font cells fill the 144px interior on one line.
    let text = "123456789012345678";
    assert_eq!(pokered_renderer::embedded_font::measure_text(text), 144);
    let fb = render_dialog(text, Lang::En);
    let mut reached_last_cell = false;
    for y in 112..120 {
        for x in 144..152 {
            if fb.get_pixel(x, y) == Some(Rgba::BLACK) {
                reached_last_cell = true;
            }
        }
    }
    assert!(
        reached_last_cell,
        "the eighteenth glyph must occupy the last text cell"
    );
    assert!(
        !text_bleeds_into_right_border(&fb),
        "text stays inside the original border"
    );
}

#[test]
fn mixed_chinese_and_original_ascii_stay_inside_dialog_and_battle_boxes() {
    let text = "一二三四五六七八九十ABCD！测试";
    for lang in [Lang::En, Lang::Zh] {
        assert!(!text_bleeds_into_right_border(&render_dialog(text, lang)));
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        let mut painter = FrameBufferPainter::new(&mut fb).with_lang(lang);
        let mut ui = Ui::new(&mut painter);
        menus::battle_text::draw(
            text,
            false,
            &pokered_data::ui_layout::schema::BATTLE_TEXT_DEFAULT_LAYOUT,
            &mut ui,
            lang,
        );
        assert!(!text_bleeds_into_right_border(&fb));
    }
}
