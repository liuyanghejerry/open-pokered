use crate::alloc_prelude::*;
use pokered_core::game_state::Lang;
use pokered_core::save_menu::{SaveMenuState, SavePhase, YesNoChoice};
use pokered_data::ui_layout::schema::{get_screen_v2_json, SaveAskPromptLayout, SaveDefaultLayout};

use crate::engine::{Painter, Rgba, TilePos, Ui};
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
