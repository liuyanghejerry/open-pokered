//! Original PrintStatusCondition gives zero HP priority over ailment status.
//! Exercise the live stats-page renderer, including its layout binding.
use pokered_core::{
    battle::state::StatusCondition, game_state::Lang, pokemon::stats::create_pokemon,
    stats_screen::StatsScreenState,
};
use pokered_data::{
    impl_traits::PokemonRenderData,
    species::Species,
    ui_layout::schema::{STATS_PAGE1_LAYOUT, STATS_PAGE2_LAYOUT},
};
use pokered_ui::{menus::stats, Painter, Rgba, TilePos, TileRect, Ui};

#[derive(Default)]
struct Recorder(Vec<(TilePos, char)>);

impl Painter for Recorder {
    fn clear(&mut self, _color: Rgba) {}
    fn draw_text_box(&mut self, _rect: TileRect, _color: Rgba) {}
    fn draw_text(&mut self, pos: TilePos, text: &str, _color: Rgba) {
        for (offset, glyph) in text.chars().enumerate() {
            self.0
                .push((TilePos::new(pos.tx + offset as u32, pos.ty), glyph));
        }
    }
    fn draw_glyph(&mut self, pos: TilePos, glyph: char, _color: Rgba) {
        self.0.push((pos, glyph));
    }
    fn draw_pixel_rect(&mut self, _x: u32, _y: u32, _w: u32, _h: u32, _color: Rgba) {}
    fn draw_gb_tile(&mut self, _pos: TilePos, _id: u8, _fallback: &str, _color: Rgba) {}
}

fn rendered_status(hp: u16, status: StatusCondition) -> String {
    let mut mon = create_pokemon(Species::Bulbasaur, 100, [0xff, 0xff]).unwrap();
    mon.hp = hp;
    mon.status = status;
    let state = StatsScreenState::new(mon);
    let mut recorder = Recorder::default();
    stats::draw(
        &state,
        &STATS_PAGE1_LAYOUT,
        &STATS_PAGE2_LAYOUT,
        &mut Ui::new(&mut recorder),
        Lang::En,
        &PokemonRenderData::new(false),
    );
    assert_eq!(state.pokemon().hp, hp, "drawing must preserve current HP");
    assert_eq!(
        state.pokemon().status,
        status,
        "drawing must preserve ailment"
    );
    let mut glyphs: Vec<_> = recorder
        .0
        .into_iter()
        .filter(|(pos, _)| pos.ty == 6 && (16..19).contains(&pos.tx))
        .collect();
    glyphs.sort_by_key(|(pos, _)| pos.tx);
    glyphs
        .into_iter()
        .map(|(_, glyph)| glyph)
        .collect::<String>()
        .trim()
        .to_string()
}

#[test]
fn zero_hp_stats_show_fnt_before_every_stored_ailment() {
    for status in [
        StatusCondition::None,
        StatusCondition::Sleep(3),
        StatusCondition::Poison,
        StatusCondition::Burn,
        StatusCondition::Freeze,
        StatusCondition::Paralysis,
    ] {
        assert_eq!(
            rendered_status(0, status),
            "FNT",
            "stored status {status:?}"
        );
    }
}

#[test]
fn one_hp_stats_keep_ok_and_poison_distinct() {
    assert_eq!(rendered_status(1, StatusCondition::None), "OK");
    assert_eq!(rendered_status(1, StatusCondition::Poison), "PSN");
}

#[test]
fn positive_hp_stats_preserve_all_ailment_codes() {
    // A zero HP low byte must not be mistaken for fainting: the original
    // combines both current-HP bytes before deciding to show FNT.
    for (status, expected) in [
        (StatusCondition::None, "OK"),
        (StatusCondition::Sleep(3), "SLP"),
        (StatusCondition::Poison, "PSN"),
        (StatusCondition::Burn, "BRN"),
        (StatusCondition::Freeze, "FRZ"),
        (StatusCondition::Paralysis, "PAR"),
    ] {
        assert_eq!(rendered_status(256, status), expected);
    }
}
