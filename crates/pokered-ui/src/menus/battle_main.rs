use pokered_core::battle::menu::BattleMenuState;
use pokered_core::game_state::Lang;
#[cfg(not(target_os = "none"))]
use pokered_data::ui_layout::schema::get_screen_v2_json;
use pokered_data::ui_layout::schema::BattleMainDefaultLayout;

use crate::engine::{Painter, Rgba, TilePos, Ui};
#[cfg(any(test, target_os = "none"))]
use crate::engine::TileRect;
#[cfg(not(target_os = "none"))]
use crate::v2::{self, DataContext};

/// Battle action menu (FIGHT / PKMN / ITEM / RUN), rendered as an OVERLAY on
/// the battle scene.
///
/// Desktop builds use the v2 layout engine and `battle_main.gui`, preserving
/// layout-editor hot reload. Bare-metal builds use an allocation-free compiled
/// equivalent because this menu sits on the GBA battle render hot path. A
/// parity test keeps its draw operations synchronized with the v2 layout.
/// `_layout` (v1 `BattleMainDefaultLayout`) remains for call-site compatibility.
pub fn draw<P: Painter>(
    state: &BattleMenuState,
    _layout: &BattleMainDefaultLayout,
    ui: &mut Ui<P>,
    lang: Lang,
) {
    // This tiny, fixed layout is on the GBA battle hot path. Rendering its
    // compiled form directly avoids parsing JSON and building a data context
    // every time the cursor moves, while desktop builds keep the editable v2
    // layout path used by the preview tools.
    #[cfg(target_os = "none")]
    {
        draw_compiled(state, ui.painter(), lang);
        return;
    }

    #[cfg(not(target_os = "none"))]
    draw_v2(state, ui, lang);
}

#[cfg(not(target_os = "none"))]
fn draw_v2<P: Painter>(state: &BattleMenuState, ui: &mut Ui<P>, lang: Lang) {
    let Some(json) = get_screen_v2_json("battle_main") else {
        return;
    };
    let Some(mut layout) = v2::parse_screen(json) else {
        return;
    };

    // The Chinese font advances 10 px, wider than the legacy 8 px tile grid.
    if lang == Lang::Zh {
        layout.theme.text_mode = dotzuki_renderer::layout_engine::types::TextMode::Proportional;
    }

    let mut ctx = DataContext::new();
    ctx.set("bcol", state.col() as i64);
    ctx.set("brow", state.row() as i64);
    ctx.set("__lang", v2::lang_code(lang));

    // Overlay: the battle sprites are already in the framebuffer; do not clear.
    v2::render_screen_overlay(&layout, &ctx, ui.painter());
}

#[cfg(any(test, target_os = "none"))]
fn draw_compiled<P: Painter>(state: &BattleMenuState, painter: &mut P, lang: Lang) {
    painter.draw_text_box(TileRect::new(0, 12, 20, 6), Rgba::INK_BLACK);
    painter.draw_text_box(TileRect::new(8, 12, 12, 6), Rgba::INK_BLACK);

    let (fight, item, run) = match lang {
        Lang::Zh => ("战斗", "道具", "逃跑"),
        _ => ("FIGHT", "ITEM", "RUN"),
    };
    draw_label(painter, TilePos::new(10, 14), fight, lang);
    painter.draw_gb_tile(TilePos::new(16, 14), 0xE1, "[225]", Rgba::INK_BLACK);
    painter.draw_gb_tile(TilePos::new(17, 14), 0xE2, "[226]", Rgba::INK_BLACK);
    draw_label(painter, TilePos::new(10, 16), item, lang);
    draw_label(painter, TilePos::new(16, 16), run, lang);

    draw_cursor(painter, cursor_position(state.row(), state.col()), lang);
}

fn cursor_position(row: usize, col: usize) -> TilePos {
    TilePos::new(9 + col as u32 * 6, 14 + row as u32 * 2)
}

fn draw_cursor<P: Painter>(painter: &mut P, cursor: TilePos, lang: Lang) {
    if lang == Lang::Zh && painter.supports_proportional() {
        painter.draw_text_px(cursor.tx * 8, cursor.ty * 8, "▶", Rgba::INK_BLACK);
    } else {
        painter.draw_glyph(cursor, '▶', Rgba::INK_BLACK);
    }
}

/// Repaint only the changed cursor cells of an already-rendered compiled
/// battle action menu.
///
/// The fallback arrow's ink fits within the cursor's 8-pixel-wide tile and
/// extends one pixel into the following tile row. Both affected regions are
/// plain menu paper, so restoring the old 8×9 cell before drawing the new
/// cursor is pixel-identical to rebuilding the complete menu.
pub fn redraw_cursor<P: Painter>(
    previous: (usize, usize),
    state: &BattleMenuState,
    painter: &mut P,
    lang: Lang,
) {
    let old = cursor_position(previous.0, previous.1);
    painter.draw_pixel_rect(old.tx * 8, old.ty * 8, 8, 9, Rgba::INK_WHITE);
    draw_cursor(painter, cursor_position(state.row(), state.col()), lang);
}

#[cfg(any(test, target_os = "none"))]
fn draw_label<P: Painter>(painter: &mut P, pos: TilePos, text: &str, lang: Lang) {
    if lang == Lang::Zh && painter.supports_proportional() {
        painter.draw_text_px_scaled(pos.tx * 8, pos.ty * 8, text, 1, Rgba::INK_BLACK);
    } else {
        for (offset, glyph) in text.chars().enumerate() {
            painter.draw_glyph(
                TilePos::new(pos.tx + offset as u32, pos.ty),
                glyph,
                Rgba::INK_BLACK,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_prelude::{String, Vec};
    use crate::engine::Rgba;
    use pokered_core::battle::menu::BattleMenuInput;

    #[derive(Debug, PartialEq)]
    enum Op {
        Clear(Rgba),
        Box(TileRect, Rgba),
        Text(TilePos, String, Rgba),
        Glyph(TilePos, char, Rgba),
        PixelRect(u32, u32, u32, u32, Rgba),
        Tile(TilePos, u8, String, Rgba),
        TextPx(u32, u32, String, Rgba),
        TextPxScaled(u32, u32, String, u32, Rgba),
    }

    #[derive(Default)]
    struct Recorder(Vec<Op>);

    impl Painter for Recorder {
        fn clear(&mut self, color: Rgba) {
            self.0.push(Op::Clear(color));
        }

        fn draw_text_box(&mut self, rect: TileRect, color: Rgba) {
            self.0.push(Op::Box(rect, color));
        }

        fn draw_text(&mut self, pos: TilePos, text: &str, color: Rgba) {
            self.0.push(Op::Text(pos, text.into(), color));
        }

        fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: Rgba) {
            self.0.push(Op::Glyph(pos, glyph, color));
        }

        fn draw_pixel_rect(&mut self, px: u32, py: u32, pw: u32, ph: u32, color: Rgba) {
            self.0.push(Op::PixelRect(px, py, pw, ph, color));
        }

        fn draw_gb_tile(&mut self, pos: TilePos, tile_id: u8, fallback: &str, color: Rgba) {
            self.0.push(Op::Tile(pos, tile_id, fallback.into(), color));
        }

        fn draw_text_px(&mut self, px: u32, py: u32, text: &str, color: Rgba) {
            self.0.push(Op::TextPx(px, py, text.into(), color));
        }

        fn draw_text_px_scaled(&mut self, px: u32, py: u32, text: &str, scale: u32, color: Rgba) {
            self.0
                .push(Op::TextPxScaled(px, py, text.into(), scale, color));
        }

        fn supports_proportional(&self) -> bool {
            true
        }
    }

    fn assert_compiled_matches_v2(state: &BattleMenuState, lang: Lang) {
        let mut expected = Recorder::default();
        draw_v2(state, &mut Ui::new(&mut expected), lang);

        let mut actual = Recorder::default();
        draw_compiled(state, &mut actual, lang);

        assert_eq!(actual.0, expected.0);
    }

    #[test]
    fn compiled_gba_layout_matches_v2_for_all_cursor_positions_and_languages() {
        for lang in [Lang::En, Lang::Zh] {
            let mut state = BattleMenuState::new();
            assert_compiled_matches_v2(&state, lang);

            state.update_frame(BattleMenuInput {
                right: true,
                ..BattleMenuInput::none()
            });
            assert_compiled_matches_v2(&state, lang);

            state.update_frame(BattleMenuInput {
                down: true,
                ..BattleMenuInput::none()
            });
            assert_compiled_matches_v2(&state, lang);

            state.update_frame(BattleMenuInput {
                left: true,
                ..BattleMenuInput::none()
            });
            assert_compiled_matches_v2(&state, lang);
        }
    }
}
