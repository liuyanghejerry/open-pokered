use pokered_core::battle::menu::SafariBattleMenuState;
use pokered_core::game_state::Lang;
#[cfg(not(target_os = "none"))]
use pokered_data::ui_layout::schema::get_screen_v2_json;

#[cfg(any(test, target_os = "none"))]
use crate::engine::TileRect;
use crate::engine::{Painter, Rgba, TilePos, Ui};
use crate::v2;

/// Safari battle action menu (BALL / BAIT / ROCK / RUN) — rendered from
/// `battle_safari.gui` as an OVERLAY on the battle scene, mirroring `battle_main`.
/// The 2×2 grid cursor is positioned by `{bcol}`/`{brow}` from the menu state.
pub fn draw<P: Painter>(state: &SafariBattleMenuState, ui: &mut Ui<P>, lang: Lang) {
    #[cfg(target_os = "none")]
    {
        draw_compiled(state, ui.painter(), lang);
        return;
    }

    #[cfg(not(target_os = "none"))]
    draw_v2(state, ui, lang);
}

#[cfg(not(target_os = "none"))]
fn draw_v2<P: Painter>(state: &SafariBattleMenuState, ui: &mut Ui<P>, lang: Lang) {
    let Some(json) = get_screen_v2_json("battle_safari") else {
        return;
    };
    let Some(layout) = v2::parse_screen(json) else {
        return;
    };

    let ctx = bindings(state, lang).dynamic();

    v2::render_screen_overlay(&layout, &ctx, ui.painter());
}

fn bindings(
    state: &SafariBattleMenuState,
    lang: Lang,
) -> dotzuki_renderer::layout_engine::static_layout::Context<'static> {
    let mut ctx = dotzuki_renderer::layout_engine::static_layout::Context::new();
    ctx.set("bcol", state.col() as i64);
    ctx.set("brow", state.row() as i64);
    ctx.set("__lang", v2::lang_code(lang));

    ctx
}

#[cfg(any(test, target_os = "none"))]
fn draw_compiled<P: Painter>(state: &SafariBattleMenuState, painter: &mut P, lang: Lang) {
    pokered_data::ui_layout::schema::BATTLE_SAFARI_STATIC_LAYOUT.render(
        &bindings(state, lang),
        painter,
        false,
        false,
    );
}

pub fn cursor_position(row: usize, col: usize) -> TilePos {
    cursor_spec(row, col).0
}

fn cursor_spec(row: usize, col: usize) -> (TilePos, char) {
    let mut ctx: dotzuki_renderer::layout_engine::static_layout::Context<'_, 2> =
        dotzuki_renderer::layout_engine::static_layout::Context::new();
    ctx.set("brow", row as i64);
    ctx.set("bcol", col as i64);
    pokered_data::ui_layout::schema::BATTLE_SAFARI_STATIC_LAYOUT
        .cursor(&ctx)
        .expect("battle layout must declare a cursor")
}

fn draw_cursor<P: Painter>(painter: &mut P, cursor: (TilePos, char)) {
    painter.draw_glyph(cursor.0, cursor.1, Rgba::INK_BLACK);
}

/// Repaint only the changed cursor cells of an already-rendered Safari menu.
pub fn redraw_cursor<P: Painter>(
    previous: (usize, usize),
    state: &SafariBattleMenuState,
    painter: &mut P,
) {
    let old = cursor_position(previous.0, previous.1);
    painter.draw_pixel_rect(old.tx * 8, old.ty * 8, 8, 9, Rgba::INK_WHITE);
    draw_cursor(painter, cursor_spec(state.row(), state.col()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_prelude::Vec;
    use pokered_core::battle::menu::BattleMenuInput;

    #[derive(Debug, PartialEq)]
    enum Op {
        Box(TileRect, Rgba),
        Glyph(TilePos, char, Rgba),
    }

    #[derive(Default)]
    struct Recorder(Vec<Op>);

    impl Painter for Recorder {
        fn clear(&mut self, _color: Rgba) {}

        fn draw_text_box(&mut self, rect: TileRect, color: Rgba) {
            self.0.push(Op::Box(rect, color));
        }

        fn draw_text(&mut self, _pos: TilePos, _text: &str, _color: Rgba) {}

        fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: Rgba) {
            self.0.push(Op::Glyph(pos, glyph, color));
        }

        fn draw_pixel_rect(&mut self, _px: u32, _py: u32, _pw: u32, _ph: u32, _color: Rgba) {}

        fn draw_gb_tile(&mut self, _pos: TilePos, _tile_id: u8, _fallback: &str, _color: Rgba) {}
    }

    fn state_at(row: usize, col: usize) -> SafariBattleMenuState {
        let mut state = SafariBattleMenuState::new(30);
        state.update_frame(BattleMenuInput {
            down: row == 1,
            right: col == 1,
            ..BattleMenuInput::none()
        });
        state
    }

    #[test]
    fn compiled_gba_layout_matches_v2_for_all_cursor_positions_and_languages() {
        for lang in [Lang::En, Lang::Zh] {
            for row in 0..2 {
                for col in 0..2 {
                    let state = state_at(row, col);

                    let mut expected = Recorder::default();
                    draw_v2(&state, &mut Ui::new(&mut expected), lang);

                    let mut actual = Recorder::default();
                    draw_compiled(&state, &mut actual, lang);

                    assert_eq!(actual.0, expected.0);
                }
            }
        }
    }
}
