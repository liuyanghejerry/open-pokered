use pokered_core::{game_state::Lang, overworld::BedroomDialogue};
use pokered_data::ui_layout::schema::DIALOG_DEFAULT_LAYOUT;
use pokered_ui::{menus, Painter, Rgba, TilePos, TileRect, Ui};

#[derive(Default)]
struct Recorder {
    text: Vec<(TilePos, String)>,
    glyphs: Vec<(TilePos, char)>,
}
impl Painter for Recorder {
    fn clear(&mut self, _: Rgba) {}
    fn draw_text_box(&mut self, _: TileRect, _: Rgba) {}
    fn draw_text(&mut self, pos: TilePos, text: &str, _: Rgba) {
        self.text.push((pos, text.into()));
    }
    fn draw_glyph(&mut self, pos: TilePos, glyph: char, _: Rgba) {
        self.glyphs.push((pos, glyph));
    }
    fn draw_pixel_rect(&mut self, _: u32, _: u32, _: u32, _: u32, _: Rgba) {}
    fn draw_gb_tile(&mut self, _: TilePos, _: u8, _: &str, _: Rgba) {}
}

#[test]
fn every_typewriter_prefix_keeps_the_core_rows_at_fixed_positions() {
    let mut d = BedroomDialogue::from_message(
        "大木博士：你需要自己的宝可梦来保护自己。我知道了！来，跟我来！",
    );
    let first = d.pages()[0].clone();
    for _ in 0..=d.total_chars() {
        let (a, b) = d.get_display_text().unwrap();
        let text = if b.is_empty() {
            a.clone()
        } else {
            format!("{a}\n{b}")
        };
        let mut p = Recorder::default();
        menus::dialog::draw_paginated(
            &text,
            d.waiting_for_input(),
            &DIALOG_DEFAULT_LAYOUT,
            &mut Ui::new(&mut p),
            Lang::Zh,
        );
        assert_eq!(p.text[0], (TilePos::new(1, 13), a));
        assert!(first.line1.starts_with(&p.text[0].1));
        if !b.is_empty() {
            assert_eq!(p.text[1], (TilePos::new(1, 15), b));
            assert!(first.line2.starts_with(&p.text[1].1));
        }
        assert!(p.text.len() <= 2);
        d.reveal_next_char();
    }
}

#[test]
fn arrow_and_layout_width_match_the_reserved_core_budget() {
    let mut p = Recorder::default();
    menus::dialog::draw_paginated(
        "你好！\n欢迎！",
        true,
        &DIALOG_DEFAULT_LAYOUT,
        &mut Ui::new(&mut p),
        Lang::Zh,
    );
    assert_eq!(p.glyphs, vec![(TilePos::new(18, 16), '▼')]);
    assert_eq!(
        pokered_data::dialogue_layout::LINE_WIDTH_PX as u32,
        (DIALOG_DEFAULT_LAYOUT.box_0.rect.tw - 2) * 8
    );
    assert_eq!(
        pokered_data::dialogue_layout::SECOND_LINE_WIDTH_PX as u32,
        (p.glyphs[0].0.tx - p.text[1].0.tx) * 8
    );
}
