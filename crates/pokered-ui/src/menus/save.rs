use crate::alloc_prelude::*;
use pokered_core::game_state::Lang;
use pokered_core::save_menu::{SaveMenuState, SavePhase, YesNoChoice};
#[cfg(not(target_os = "none"))]
use pokered_data::ui_layout::schema::get_screen_v2_json;
use pokered_data::ui_layout::schema::{SaveAskPromptLayout, SaveDefaultLayout};

use crate::engine::{Painter, Rgba, TilePos, Ui};
#[cfg(any(test, target_os = "none"))]
use crate::engine::TileRect;
#[cfg(not(target_os = "none"))]
use crate::v2::{self, DataContext};

/// Save screen, with phase-specific content in the shared `save.gui` layout.
/// Legacy layout arguments remain for compatibility with existing frontends.
pub fn draw<P: Painter>(
    state: &SaveMenuState,
    _layout: &SaveDefaultLayout,
    _ask_layout: &SaveAskPromptLayout,
    ui: &mut Ui<P>,
    lang: Lang,
) {
    #[cfg(target_os = "none")]
    {
        draw_compiled(state, ui.painter(), lang);
        return;
    }

    #[cfg(not(target_os = "none"))]
    draw_v2(state, ui, lang);
}

#[cfg(not(target_os = "none"))]
fn draw_v2<P: Painter>(state: &SaveMenuState, ui: &mut Ui<P>, lang: Lang) {
    let Some(json) = get_screen_v2_json("save") else {
        return;
    };
    let Some(mut layout) = v2::parse_screen(json) else {
        return;
    };
    // Use measured glyph widths in both languages for flush right value edges
    // and enough room for the prompt beside its choices.
    layout.theme.text_mode = dotzuki_renderer::layout_engine::types::TextMode::Proportional;

    let is_zh = lang == Lang::Zh;
    let asking = matches!(
        state.phase,
        SavePhase::AskSave | SavePhase::ConfirmOverwrite
    );
    let (line_1, line_2) = match state.phase {
        SavePhase::AskSave | SavePhase::ConfirmOverwrite => {
            if is_zh {
                ("是否要".into(), "保存游戏？")
            } else {
                ("Save your".into(), "progress?")
            }
        }
        SavePhase::Saving { .. } => (
            (if is_zh {
                "正在保存……"
            } else {
                "Now saving..."
            })
            .into(),
            "",
        ),
        SavePhase::SaveComplete | SavePhase::WaitAfterSave { .. } => {
            if is_zh {
                (format!("{}已保存", state.info.player_name), "游戏！")
            } else {
                (format!("{} saved", state.info.player_name), "the game!")
            }
        }
    };

    let mut ctx = DataContext::new();
    ctx.set("__lang", v2::lang_code(lang));
    ctx.set("player_name", state.info.player_name.clone());
    ctx.set("badges", state.info.num_badges.to_string());
    ctx.set("owned_count", state.info.pokedex_owned.to_string());
    ctx.set(
        "play_time",
        format!(
            "{}:{:02}",
            state.info.play_time_hours, state.info.play_time_minutes
        ),
    );
    ctx.set("message_line_1", line_1);
    ctx.set("message_line_2", line_2);
    ctx.set("asking", asking);
    ctx.set("show_status", !asking);
    ctx.set(
        "cursor_ty",
        if state.cursor == YesNoChoice::Yes {
            13_i64
        } else {
            15_i64
        },
    );
    v2::render_screen(&layout, &ctx, ui.painter());
}

#[cfg(any(test, target_os = "none"))]
fn draw_compiled<P: Painter>(state: &SaveMenuState, painter: &mut P, lang: Lang) {
    painter.clear(Rgba::INK_WHITE);
    painter.draw_text_box(TileRect::new(0, 0, 20, 11), Rgba::INK_BLACK);

    let (player, badges, pokedex, time) = match lang {
        Lang::Zh => ("玩家", "徽章", "图鉴", "时间"),
        Lang::En => ("PLAYER", "BADGES", "#DEX", "TIME"),
    };
    draw_label(painter, 2, 2, player);
    draw_right_aligned(painter, 10, 2, 8, &state.info.player_name);
    draw_label(painter, 2, 4, badges);
    draw_right_aligned(painter, 10, 4, 8, &state.info.num_badges.to_string());
    draw_label(painter, 2, 6, pokedex);
    draw_right_aligned(painter, 10, 6, 8, &state.info.pokedex_owned.to_string());
    draw_label(painter, 2, 8, time);
    draw_right_aligned(
        painter,
        10,
        8,
        8,
        &format!(
            "{}:{:02}",
            state.info.play_time_hours, state.info.play_time_minutes
        ),
    );

    painter.draw_text_box(TileRect::new(0, 12, 20, 6), Rgba::INK_BLACK);
    match state.phase {
        SavePhase::AskSave | SavePhase::ConfirmOverwrite => {
            let (line_1, line_2, yes, no) = match lang {
                Lang::Zh => ("是否要", "保存游戏？", "是", "否"),
                Lang::En => ("Save your", "progress?", "YES", "NO"),
            };
            draw_label(painter, 2, 13, line_1);
            draw_label(painter, 2, 15, line_2);
            draw_label(painter, 16, 13, yes);
            draw_label(painter, 16, 15, no);
            let cursor_ty = if state.cursor == YesNoChoice::Yes {
                13
            } else {
                15
            };
            painter.draw_text_px(14 * 8, cursor_ty * 8, "▶", Rgba::INK_BLACK);
        }
        SavePhase::Saving { .. } => {
            draw_label(
                painter,
                2,
                13,
                if lang == Lang::Zh {
                    "正在保存……"
                } else {
                    "Now saving..."
                },
            );
            draw_label(painter, 2, 15, "");
        }
        SavePhase::SaveComplete | SavePhase::WaitAfterSave { .. } => {
            let line_1 = match lang {
                Lang::Zh => format!("{}已保存", state.info.player_name),
                Lang::En => format!("{} saved", state.info.player_name),
            };
            draw_label(painter, 2, 13, &line_1);
            draw_label(
                painter,
                2,
                15,
                if lang == Lang::Zh {
                    "游戏！"
                } else {
                    "the game!"
                },
            );
        }
    }
}

#[cfg(any(test, target_os = "none"))]
fn draw_label<P: Painter>(painter: &mut P, tx: u32, ty: u32, text: &str) {
    painter.draw_text_px_scaled(tx * 8, ty * 8, text, 1, Rgba::INK_BLACK);
}

#[cfg(any(test, target_os = "none"))]
fn draw_right_aligned<P: Painter>(
    painter: &mut P,
    tx: u32,
    ty: u32,
    tw: u32,
    text: &str,
) {
    let px = tx * 8 + (tw * 8).saturating_sub(painter.measure_text_px(text));
    painter.draw_text_px_scaled(px, ty * 8, text, 1, Rgba::INK_BLACK);
}

/// Repaint only the changed YES/NO cursor cells of an already-rendered prompt.
pub fn redraw_cursor<P: Painter>(
    previous: YesNoChoice,
    current: YesNoChoice,
    painter: &mut P,
) {
    let position = |choice| {
        TilePos::new(
            14,
            if choice == YesNoChoice::Yes { 13 } else { 15 },
        )
    };
    let old = position(previous);
    painter.draw_pixel_rect(old.tx * 8, old.ty * 8, 8, 9, Rgba::INK_WHITE);
    let current = position(current);
    painter.draw_text_px(current.tx * 8, current.ty * 8, "▶", Rgba::INK_BLACK);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_core::save_menu::SaveScreenInfo;

    #[derive(Debug, PartialEq)]
    enum Op {
        Clear(Rgba),
        Box(TileRect, Rgba),
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

        fn draw_text(&mut self, _pos: TilePos, _text: &str, _color: Rgba) {}

        fn draw_glyph(&mut self, _pos: TilePos, _glyph: char, _color: Rgba) {}

        fn draw_pixel_rect(
            &mut self,
            _px: u32,
            _py: u32,
            _pw: u32,
            _ph: u32,
            _color: Rgba,
        ) {
        }

        fn draw_gb_tile(
            &mut self,
            _pos: TilePos,
            _tile_id: u8,
            _fallback: &str,
            _color: Rgba,
        ) {
        }

        fn draw_text_px(&mut self, px: u32, py: u32, text: &str, color: Rgba) {
            self.0.push(Op::TextPx(px, py, text.into(), color));
        }

        fn measure_text_px(&self, text: &str) -> u32 {
            dotzuki_renderer::embedded_font::measure_text(text)
        }

        fn draw_text_px_scaled(
            &mut self,
            px: u32,
            py: u32,
            text: &str,
            scale: u32,
            color: Rgba,
        ) {
            self.0
                .push(Op::TextPxScaled(px, py, text.into(), scale, color));
        }

        fn supports_proportional(&self) -> bool {
            true
        }
    }

    fn assert_compiled_matches_v2(state: &SaveMenuState, lang: Lang) {
        let mut expected = Recorder::default();
        draw_v2(state, &mut Ui::new(&mut expected), lang);

        let mut actual = Recorder::default();
        draw_compiled(state, &mut actual, lang);

        assert_eq!(actual.0, expected.0, "state={state:?}, lang={lang:?}");
    }

    #[test]
    fn compiled_gba_layout_matches_v2_for_all_visible_phases_and_languages() {
        let infos = [
            SaveScreenInfo {
                player_name: "RED".into(),
                num_badges: 3,
                pokedex_owned: 42,
                play_time_hours: 12,
                play_time_minutes: 34,
            },
            SaveScreenInfo {
                player_name: "BLUE".into(),
                num_badges: 8,
                pokedex_owned: 151,
                play_time_hours: 255,
                play_time_minutes: 7,
            },
        ];

        for lang in [Lang::En, Lang::Zh] {
            for info in &infos {
                for phase in [
                    SavePhase::AskSave,
                    SavePhase::ConfirmOverwrite,
                    SavePhase::Saving {
                        frames_remaining: 30,
                    },
                    SavePhase::SaveComplete,
                    SavePhase::WaitAfterSave {
                        frames_remaining: 60,
                    },
                ] {
                    for cursor in [YesNoChoice::Yes, YesNoChoice::No] {
                        let state = SaveMenuState {
                            phase: phase.clone(),
                            cursor,
                            info: info.clone(),
                            has_previous_save: false,
                            is_different_player: false,
                            sfx_event: pokered_core::save_menu::SaveSfxEvent::None,
                        };
                        assert_compiled_matches_v2(&state, lang);
                    }
                }
            }
        }
    }
}
