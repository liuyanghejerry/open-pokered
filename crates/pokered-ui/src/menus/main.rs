use pokered_core::game_state::Lang;
use pokered_core::main_menu::MainMenuState;
use pokered_data::lang_data;
use pokered_data::ui_layout::schema::{get_screen_v2_json, MainDefaultLayout};

use crate::engine::{InkColor, Painter, TileRect, Ui};
use crate::v2::{self, DataContext, DataValue};

/// Title main menu (CONTINUE / NEW GAME / OPTION) — rendered through the v2
/// layout engine from `main.gui`.
///
/// The `_layout` parameter (the v1 `MainDefaultLayout`) is kept for call-site
/// compatibility but is no longer used; the layout now lives in `main.gui`.
/// Item labels (already localized) and the cursor index are bound into a
/// [`DataContext`] that the v2 `list` element consumes.
pub fn draw<P: Painter>(
    state: &MainMenuState,
    _layout: &MainDefaultLayout,
    ui: &mut Ui<P>,
    lang: Lang,
) {
    let Some(json) = get_screen_v2_json("main") else {
        return;
    };
    let Some(mut layout) = v2::parse_screen(json) else {
        return;
    };

    let is_zh = lang == Lang::Zh;
    let labels: Vec<DataValue> = state
        .item_labels()
        .iter()
        .map(|key| DataValue::Str(lang_data::ui_label(key, is_zh).to_string()))
        .collect();
    let num_items = labels.len() as u32;
    if num_items == 0 {
        return;
    }

    // Reproduce the v1 flex auto-height so the box hugs the entry count:
    // 1 border + 1 pad + (items spaced 2 rows) + 1 pad + 1 border = 2n + 3.
    v2::set_panel_height(&mut layout, 2 * num_items + 3);

    let mut ctx = DataContext::new();
    ctx.set("items", DataValue::List(labels));
    ctx.set("cursor", state.cursor as i64);
    ctx.set("__lang", v2::lang_code(lang));

    v2::render_screen(&layout, &ctx, ui.painter());

    // CONTINUE save-info panel — `DisplayContinueGameInfo`
    // (engine/menus/main_menu.asm:357-379): border at hlcoord 4,7 and the
    // PLAYER/BADGES/#DEX/TIME rows from `SaveScreenInfoText`. A loads the
    // game, B returns to the menu (handled in `MainMenuState`).
    if state.is_showing_continue_info() {
        let rect = TileRect::new(4, 7, 14, 10);
        ui.text_box(rect, InkColor::Black, true, |frame| {
            for (i, (label, value)) in state.continue_info_lines().iter().enumerate() {
                let ty = 1 + i as u32 * 2;
                frame.label(0, ty, lang_data::ui_label(label, is_zh), InkColor::Black);
                // Values are right-aligned-ish under the original's absolute
                // coords (name 12,9 / badges 17,11 / #dex 16,13 / time 13,15
                // relative to the box inner origin (5,8)).
                let tx = match i {
                    0 => 7,
                    1 => 12,
                    2 => 11,
                    _ => 8,
                };
                frame.label(tx, ty, value, InkColor::Black);
            }
        });
    }
}
