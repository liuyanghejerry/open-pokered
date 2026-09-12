use crate::alloc_prelude::*;
use pokered_core::game_state::Lang;
use pokered_core::save_menu::{SaveMenuState, SavePhase, YesNoChoice};
#[cfg(not(target_os = "none"))]
use pokered_data::ui_layout::schema::get_screen_v2_json;
use pokered_data::ui_layout::schema::{SaveAskPromptLayout, SaveDefaultLayout};

#[cfg(any(test, target_os = "none"))]
use crate::engine::TileRect;
use crate::engine::{Painter, Rgba, TilePos, Ui};
use crate::v2;

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

    let ctx = bindings(state, lang).dynamic();

    v2::render_screen(&layout, &ctx, ui.painter());
}

fn bindings(
    state: &SaveMenuState,
    lang: Lang,
) -> dotzuki_renderer::layout_engine::static_layout::Context<'static> {
    let is_zh = lang == Lang::Zh;
    let asking = matches!(
        state.phase,
        SavePhase::AskSave | SavePhase::ConfirmOverwrite
    );
    let (line_1, line_2) = match state.phase {
        SavePhase::AskSave | SavePhase::ConfirmOverwrite => {
            if is_zh {
                (String::from("是否要"), "保存游戏？")
            } else {
                (String::from("Save your"), "progress?")
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

    let mut ctx = dotzuki_renderer::layout_engine::static_layout::Context::new();
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
        "cursor_row",
        if state.cursor == YesNoChoice::Yes {
            0_i64
        } else {
            1_i64
        },
    );
    ctx
}

#[cfg(any(test, target_os = "none"))]
fn draw_compiled<P: Painter>(state: &SaveMenuState, painter: &mut P, lang: Lang) {
    pokered_data::ui_layout::schema::SAVE_STATIC_LAYOUT.render(
        &bindings(state, lang),
        painter,
        true,
        true,
    );
}

/// Resolve YES/NO cursor geometry from the compiled GUI source.
pub fn cursor_position(choice: YesNoChoice) -> TilePos {
    cursor_spec(choice).0
}

pub fn cursor_damage(choice: YesNoChoice) -> crate::DamageRect {
    crate::DamageRect::cursor(cursor_position(choice))
}

fn cursor_spec(choice: YesNoChoice) -> (TilePos, char) {
    let mut ctx: dotzuki_renderer::layout_engine::static_layout::Context<'_, 2> =
        dotzuki_renderer::layout_engine::static_layout::Context::new();
    ctx.set("asking", true);
    ctx.set(
        "cursor_row",
        if choice == YesNoChoice::Yes {
            0_i64
        } else {
            1_i64
        },
    );
    pokered_data::ui_layout::schema::SAVE_STATIC_LAYOUT
        .cursor(&ctx)
        .expect("save layout must declare a cursor")
}

/// Repaint only the changed YES/NO cursor cells of an already-rendered prompt.
pub fn redraw_cursor<P: Painter>(previous: YesNoChoice, current: YesNoChoice, painter: &mut P) {
    let old = cursor_position(previous);
    painter.draw_pixel_rect(old.tx * 8, old.ty * 8, 8, 9, Rgba::INK_WHITE);
    let (position, glyph) = cursor_spec(current);
    dotzuki_renderer::layout_engine::elements::cursor::draw_cursor_glyph(
        position,
        glyph,
        Rgba::INK_BLACK,
        painter.supports_proportional(),
        painter,
    );
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

        fn draw_pixel_rect(&mut self, _px: u32, _py: u32, _pw: u32, _ph: u32, _color: Rgba) {}

        fn draw_gb_tile(&mut self, _pos: TilePos, _tile_id: u8, _fallback: &str, _color: Rgba) {}

        fn draw_text_px(&mut self, px: u32, py: u32, text: &str, color: Rgba) {
            self.0.push(Op::TextPx(px, py, text.into(), color));
        }

        fn measure_text_px(&self, text: &str) -> u32 {
            dotzuki_renderer::embedded_font::measure_text(text)
        }

        fn draw_text_px_scaled(&mut self, px: u32, py: u32, text: &str, scale: u32, color: Rgba) {
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
