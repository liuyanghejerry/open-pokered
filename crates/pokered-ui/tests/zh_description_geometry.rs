use pokered_core::game_state::Lang;
use pokered_data::ui_layout::schema::{
    BATTLE_TEXT_DEFAULT_LAYOUT, OAK_SPEECH_NAME_CHOICE_LAYOUT, OAK_SPEECH_TEXT_PHASE_LAYOUT,
};
use pokered_ui::{menus, Painter, Rgba, TilePos, TileRect, Ui};

#[derive(Default)]
struct Recorder {
    text: Vec<(TilePos, String)>,
}
impl Painter for Recorder {
    fn clear(&mut self, _: Rgba) {}
    fn draw_text_box(&mut self, _: TileRect, _: Rgba) {}
    fn draw_text(&mut self, pos: TilePos, text: &str, _: Rgba) {
        self.text.push((pos, text.into()));
    }
    fn draw_glyph(&mut self, _: TilePos, _: char, _: Rgba) {}
    fn draw_pixel_rect(&mut self, _: u32, _: u32, _: u32, _: u32, _: Rgba) {}
    fn draw_gb_tile(&mut self, _: TilePos, _: u8, _: &str, _: Rgba) {}
}

#[test]
fn oak_rows_stay_above_the_bottom_border() {
    let mut p = Recorder::default();
    menus::oak_speech::draw_text_phase_localized(
        "这个世界生活着",
        "一种叫做宝可梦的神奇生物！",
        true,
        &OAK_SPEECH_TEXT_PHASE_LAYOUT,
        &mut Ui::new(&mut p),
        Lang::Zh,
    );
    assert_eq!(p.text[0].0, TilePos::new(1, 13));
    assert_eq!(p.text[1].0, TilePos::new(1, 15));
    assert!(p.text[1].0.ty * 8 + 12 <= 17 * 8);
}

#[test]
fn chinese_name_prompt_fits_the_short_box() {
    let mut p = Recorder::default();
    menus::oak_speech::draw_name_choice(
        &["NEW NAME", "RED", "ASH", "JACK"],
        0,
        "你的名字？",
        &OAK_SPEECH_NAME_CHOICE_LAYOUT,
        &mut Ui::new(&mut p),
    );
    let (pos, _) = p.text.iter().find(|(_, s)| s == "你的名字？").unwrap();
    assert_eq!(*pos, TilePos::new(1, 13));
    assert!(pos.ty * 8 + 12 <= 15 * 8);
}

#[test]
fn chinese_battle_pages_keep_prepared_rows_and_fit_above_the_border() {
    let mut p = Recorder::default();
    menus::battle_text::draw(
        "对方的皮卡丘使用了\n十万伏特！",
        true,
        &BATTLE_TEXT_DEFAULT_LAYOUT,
        &mut Ui::new(&mut p),
        Lang::Zh,
    );
    assert_eq!(
        p.text,
        vec![
            (TilePos::new(1, 13), "对方的皮卡丘使用了".into()),
            (TilePos::new(1, 15), "十万伏特！".into()),
        ]
    );
}

#[test]
fn every_chinese_machine_prompt_has_nonoverlapping_readable_rows() {
    for n in 0xc4..=0xfa {
        let mut p = Recorder::default();
        menus::bag::draw_machine_prompt(
            pokered_data::items::ItemId::from_id(n),
            Some(0),
            &mut Ui::new(&mut p),
            Lang::Zh,
        );
        let body: Vec<_> = p
            .text
            .iter()
            .filter(|(_, s)| !["是", "否"].contains(&s.as_str()))
            .collect();
        assert_eq!(body.len(), 3, "machine {n:02x}");
        for pair in body.windows(2) {
            assert!(
                pair[1].0.ty >= pair[0].0.ty + 2,
                "overlapping machine rows machine {n:02x}"
            );
        }
        for (pos, text) in body {
            assert!(pos.tx * 8 + pokered_data::dialogue_layout::measure_text(text) <= 152);
            assert!(pos.ty * 8 + 12 <= 136);
        }
    }
}
