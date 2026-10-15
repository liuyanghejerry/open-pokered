//! Original StatusScreen2 uses GetMaxPP, including the seven-PP bonus cap.
use std::collections::BTreeMap;

use dotzuki_engine::render_data::RenderData;

use pokered_core::{
    game_state::Lang,
    pokemon::stats::create_pokemon_with_moves,
    stats_screen::{StatsPage, StatsScreenInput, StatsScreenState},
};
use pokered_data::{
    impl_traits::PokemonRenderData,
    items::ItemId,
    moves::MoveId,
    species::Species,
    ui_layout::schema::{STATS_PAGE1_LAYOUT, STATS_PAGE2_LAYOUT},
};
use pokered_ui::{menus::stats, Painter, Rgba, TilePos, TileRect, Ui};

#[derive(Default)]
struct Recorder(BTreeMap<(u32, u32), char>);

impl Painter for Recorder {
    fn clear(&mut self, _color: Rgba) {
        self.0.clear();
    }
    fn draw_text_box(&mut self, _rect: TileRect, _color: Rgba) {}
    fn draw_text(&mut self, pos: TilePos, text: &str, _color: Rgba) {
        for (offset, glyph) in text.chars().enumerate() {
            self.0.insert((pos.ty, pos.tx + offset as u32), glyph);
        }
    }
    fn draw_glyph(&mut self, pos: TilePos, glyph: char, _color: Rgba) {
        self.0.insert((pos.ty, pos.tx), glyph);
    }
    fn draw_pixel_rect(&mut self, _x: u32, _y: u32, _w: u32, _h: u32, _color: Rgba) {}
    fn draw_gb_tile(&mut self, _pos: TilePos, _id: u8, _fallback: &str, _color: Rgba) {}
}

fn rendered_pp(moves: [MoveId; 4], pp: [u8; 4], ups: [u8; 4]) -> Vec<String> {
    rendered_pp_with_data(moves, pp, ups, &PokemonRenderData::new(false))
}

fn rendered_pp_with_data(
    moves: [MoveId; 4],
    pp: [u8; 4],
    ups: [u8; 4],
    data: &dyn RenderData<Move = MoveId, Item = ItemId, Species = Species>,
) -> Vec<String> {
    let mut mon = create_pokemon_with_moves(Species::Pikachu, 20, [0x9a, 0x78], moves).unwrap();
    mon.pp = pp;
    mon.pp_ups = ups;
    let mut state = StatsScreenState::new(mon);
    state.update(StatsScreenInput { a: true, b: false });
    assert_eq!(state.page(), StatsPage::Moves);
    let mut recorder = Recorder::default();
    stats::draw(
        &state,
        &STATS_PAGE1_LAYOUT,
        &STATS_PAGE2_LAYOUT,
        &mut Ui::new(&mut recorder),
        Lang::En,
        data,
    );
    assert_eq!(
        state.pokemon().moves,
        moves,
        "drawing preserves the live slots"
    );
    assert_eq!(
        state.pokemon().pp,
        pp,
        "drawing cannot restore or consume PP"
    );
    assert_eq!(state.pokemon().pp_ups, ups);
    (0..18)
        .filter_map(|y| {
            let row: String = (0..20)
                .map(|x| recorder.0.get(&(y, x)).copied().unwrap_or(' '))
                .collect();
            row.split_once("PP")
                .and_then(|(_, text)| text.contains('/').then(|| text.trim().to_string()))
        })
        .collect()
}

#[test]
fn stats_moves_growl_pp_ups_follow_gen1_seven_point_cap() {
    // Independent source expectations: 40 + 0/7/14/21, not 40 + 0/8/16/24.
    for (ups, maximum) in [40, 47, 54, 61].into_iter().enumerate() {
        assert_eq!(
            rendered_pp(
                [MoveId::Growl, MoveId::None, MoveId::None, MoveId::None],
                [maximum, 0, 0, 0],
                [ups as u8, 0, 0, 0]
            ),
            vec![format!("{maximum}/{maximum}")]
        );
    }
}

#[test]
fn stats_moves_pp_zero_and_independent_slot_bonuses_are_preserved() {
    assert_eq!(
        rendered_pp(
            [
                MoveId::Thundershock,
                MoveId::Growl,
                MoveId::None,
                MoveId::None
            ],
            [0, 61, 0, 0],
            [1, 3, 0, 0]
        ),
        vec!["0/36", "61/61"]
    );
}

struct AlternatePpData(PokemonRenderData);

impl RenderData for AlternatePpData {
    type Move = MoveId;
    type Item = ItemId;
    type Species = Species;

    fn move_name(&self, id: MoveId) -> &str {
        self.0.move_name(id)
    }
    fn move_pp(&self, _id: MoveId) -> (u8, u8) {
        (12, 99)
    }
    fn move_type(&self, id: MoveId) -> u8 {
        self.0.move_type(id)
    }
    fn item_name(&self, id: ItemId) -> &str {
        self.0.item_name(id)
    }
    fn species_name(&self, id: Species) -> &str {
        self.0.species_name(id)
    }
}

#[test]
fn stats_moves_keep_provider_base_and_round_each_bonus_before_multiplication() {
    // A provider base of 12 contributes two per Up, not floor(12 * Ups / 5).
    // The out-of-range metadata case preserves this UI's existing clamp to 3.
    let data = AlternatePpData(PokemonRenderData::new(false));
    for (ups, expected) in [(1, "1/14"), (3, "1/18"), (4, "1/18")] {
        assert_eq!(
            rendered_pp_with_data(
                [MoveId::Growl, MoveId::None, MoveId::None, MoveId::None],
                [1, 0, 0, 0],
                [ups, 0, 0, 0],
                &data,
            ),
            vec![expected],
        );
    }
}
