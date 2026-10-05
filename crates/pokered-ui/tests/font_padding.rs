//! Actual glyph ink must leave white pixels before textbox borders.
use dotzuki_engine::render_config::RenderConfig;
use pokered_core::{
    battle::menu::{MoveMenuState, MoveSlot},
    game_state::Lang,
    options_menu::{GameOptions, OptionsMenuState},
};
use pokered_data::{
    impl_traits::PokemonRenderData,
    moves::MoveId,
    species::Species,
    ui_layout::schema::{
        BATTLE_MOVE_DEFAULT_LAYOUT, BATTLE_PARTY_DEFAULT_LAYOUT, OPTIONS_DEFAULT_LAYOUT,
    },
};
use pokered_renderer::{FrameBuffer, Rgba};
use pokered_ui::{backends::FrameBufferPainter, menus, Ui};

fn blank_band(fb: &FrameBuffer, x0: u32, x1: u32, y0: u32, y1: u32) {
    for y in y0..y1 {
        for x in x0..x1 {
            assert_eq!(
                fb.get_pixel(x, y),
                Some(Rgba::WHITE),
                "text ink touches border gutter at ({x},{y})"
            );
        }
    }
}

#[test]
fn options_values_leave_a_gutter_before_each_separator() {
    for lang in [Lang::En, Lang::Zh] {
        let state = OptionsMenuState::new(GameOptions::default());
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        let mut painter = FrameBufferPainter::new(&mut fb).with_lang(lang);
        menus::options::draw(
            &state,
            &OPTIONS_DEFAULT_LAYOUT,
            &mut Ui::new(&mut painter),
            lang,
        );
        for y in [34, 74, 114] {
            blank_band(&fb, 8, 152, y, y + 6);
        }
    }
}

#[test]
fn four_move_and_party_rows_leave_a_bottom_gutter() {
    let moves = MoveMenuState::new(
        [
            MoveId::Tackle,
            MoveId::Growl,
            MoveId::LeechSeed,
            MoveId::VineWhip,
        ]
        .into_iter()
        .map(|move_id| MoveSlot {
            move_id,
            current_pp: 35,
            max_pp: 35,
            is_disabled: false,
        })
        .collect(),
    );
    for lang in [Lang::En, Lang::Zh] {
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        let mut painter = FrameBufferPainter::new(&mut fb).with_lang(lang);
        menus::battle_move::draw(
            &moves,
            &BATTLE_MOVE_DEFAULT_LAYOUT,
            &mut Ui::new(&mut painter),
            lang,
            &PokemonRenderData::new(lang == Lang::Zh),
        );
        blank_band(&fb, 40, 152, 136, 138);
        let party = (0..4)
            .map(|_| {
                pokered_core::pokemon::stats::create_pokemon(Species::Bulbasaur, 100, [255, 255])
                    .unwrap()
            })
            .collect::<Vec<_>>();
        fb.clear(Rgba::WHITE);
        let mut painter = FrameBufferPainter::new(&mut fb).with_lang(lang);
        menus::battle_party::draw(
            &party,
            3,
            &BATTLE_PARTY_DEFAULT_LAYOUT,
            &mut Ui::new(&mut painter),
            lang == Lang::Zh,
        );
        blank_band(&fb, 16, 144, 136, 138);
    }
}
