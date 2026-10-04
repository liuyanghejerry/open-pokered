//! Pixel-level regression tests for the overworld dialog box layout.
//!
//! Keep the project's Fusion Pixel Latin/CJK typography on the 20×18 grid.
//! Text wraps at its measured width inside the 144px box interior and must
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

#[test]
fn project_font_descenders_do_not_touch_the_original_bottom_border() {
    use pokered_data::ui_layout::schema::BATTLE_TEXT_DEFAULT_LAYOUT;
    for lang in [Lang::En, Lang::Zh] {
        for battle in [false, true] {
            let mut actual = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            let mut painter = FrameBufferPainter::new(&mut actual).with_lang(lang);
            if battle {
                menus::battle_text::draw_hard_lines("First row.\ngyp points!", false,
                    &BATTLE_TEXT_DEFAULT_LAYOUT, &mut Ui::new(&mut painter), lang);
            } else {
                menus::dialog::draw("First row.\ngyp points!", false,
                    &DIALOG_DEFAULT_LAYOUT, &mut Ui::new(&mut painter), lang);
            }
            let mut expected = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            // Chinese battle rows retain #110's first-row position while
            // using the safe 12px pitch; ordinary dialogs start one tile lower.
            let second_y = if lang == Lang::Zh && battle { 115 } else if lang == Lang::Zh { 123 } else { 124 };
            dotzuki_renderer::embedded_font::draw_text("gyp points!", 8,
                second_y, Rgba::BLACK, &mut expected);
            for y in second_y..second_y + 11 { for x in 8..80 {
                assert_eq!(actual.get_pixel(x,y), expected.get_pixel(x,y),
                    "the entire unchanged project glyph must fit above the border: {lang:?} battle={battle} ({x},{y})");
            } }
            for y in 136..138 { for x in 8..144 {
                assert_eq!(actual.get_pixel(x,y), Some(Rgba::WHITE),
                    "keep descenders separate from the original border");
            } }
        }
    }
}
