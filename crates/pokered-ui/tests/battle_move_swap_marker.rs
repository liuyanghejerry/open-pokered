//! Verify the battle move-swap armed marker (`▷`) stays bounded inside its
//! 8×9 cursor cell and matches between full and incremental (`redraw_selection`)
//! rendering in both English and Chinese.

use pokered_core::{
    battle::menu::{MoveMenuState, MoveSlot},
    game_state::Lang,
};
use pokered_data::{
    impl_traits::PokemonRenderData, moves::MoveId, ui_layout::schema::BATTLE_MOVE_DEFAULT_LAYOUT,
};
use pokered_renderer::{FrameBuffer, RenderConfig, Rgba};
use pokered_ui::{backends::FrameBufferPainter, menus::battle_move, Ui};

fn four_move_menu(armed_row: Option<usize>, cursor: usize) -> MoveMenuState {
    let mut state = MoveMenuState::new(vec![
        MoveSlot {
            move_id: MoveId::Pound,
            current_pp: 35,
            max_pp: 35,
            is_disabled: false,
        },
        MoveSlot {
            move_id: MoveId::Tackle,
            current_pp: 35,
            max_pp: 35,
            is_disabled: false,
        },
        MoveSlot {
            move_id: MoveId::Growl,
            current_pp: 40,
            max_pp: 40,
            is_disabled: false,
        },
        MoveSlot {
            move_id: MoveId::Thundershock,
            current_pp: 30,
            max_pp: 30,
            is_disabled: false,
        },
    ]);
    if let Some(armed) = armed_row {
        state.set_cursor(armed);
        assert_eq!(state.select_swap(), None);
    }
    state.set_cursor(cursor);
    state
}

fn render_full(state: &MoveMenuState, lang: Lang) -> FrameBuffer {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let data = PokemonRenderData::new(lang == Lang::Zh);
    let mut painter = FrameBufferPainter::new(&mut fb).with_lang(lang);
    let mut ui = Ui::new(&mut painter);
    battle_move::draw(state, &BATTLE_MOVE_DEFAULT_LAYOUT, &mut ui, lang, &data);
    fb
}

#[test]
fn armed_swap_marker_stays_confined_to_cursor_cell_without_touching_move_names() {
    for lang in [Lang::En, Lang::Zh] {
        let unarmed = render_full(&four_move_menu(None, 2), lang);
        let armed = render_full(&four_move_menu(Some(0), 2), lang);

        let (cell_x, cell_y) = if lang == Lang::Zh {
            (8u32, 94u32)
        } else {
            (40u32, 102u32)
        };
        let mut marker_pixels = 0usize;
        for y in 0..144 {
            for x in 0..160 {
                if armed.get_pixel(x, y) != unarmed.get_pixel(x, y) {
                    assert!(
                        (cell_x..cell_x + 8).contains(&x) && (cell_y..cell_y + 9).contains(&y),
                        "armed marker bled outside cursor cell ({cell_x}..{}, {cell_y}..{}) at ({x}, {y}) for {lang:?}",
                        cell_x + 8,
                        cell_y + 9,
                    );
                    marker_pixels += 1;
                }
            }
        }
        assert!(marker_pixels > 0, "armed marker must draw visible pixels for {lang:?}");
    }
}

#[test]
fn incremental_redraw_selection_matches_full_render_across_armed_cursor_transitions() {
    for lang in [Lang::En, Lang::Zh] {
        let data = PokemonRenderData::new(lang == Lang::Zh);
        for armed_row in 0..4 {
            let mut state = four_move_menu(Some(armed_row), 0);
            let mut incremental = render_full(&state, lang);
            for next in [1, 2, 3, 0, armed_row, (armed_row + 1) % 4] {
                let prev = state.cursor();
                state.set_cursor(next);
                {
                    let mut painter = FrameBufferPainter::new(&mut incremental).with_lang(lang);
                    let mut ui = Ui::new(&mut painter);
                    battle_move::redraw_selection(
                        prev,
                        &state,
                        &BATTLE_MOVE_DEFAULT_LAYOUT,
                        &mut ui,
                        lang,
                        &data,
                    );
                }
                let expected = render_full(&state, lang);
                for y in 0..144 {
                    for x in 0..160 {
                        assert_eq!(
                            incremental.get_pixel(x, y),
                            expected.get_pixel(x, y),
                            "incremental redraw mismatch at ({x}, {y}) for {lang:?} armed={armed_row} {prev}->{next}",
                        );
                    }
                }
            }
        }
    }
}
