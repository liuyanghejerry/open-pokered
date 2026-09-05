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
    // (engine/menus/main_menu.asm:357-379): the box's top edge joins the menu
    // box's bottom edge (the original's interlocked look), the right edge
    // reaches the last screen column, and the PLAYER/BADGES/#DEX/TIME rows
    // come from `SaveScreenInfoText`. A loads the game, B returns to the menu
    // (handled in `MainMenuState`).
    if state.is_showing_continue_info() {
        let rect = TileRect::new(4, 8, 16, 10);
        let mut value_rows: Vec<(u32, String)> = Vec::new();
        ui.text_box(rect, InkColor::Black, true, |frame| {
            for (i, (label, value)) in state.continue_info_lines().iter().enumerate() {
                let ty = 1 + i as u32 * 2;
                frame.label(0, ty, lang_data::ui_label(label, is_zh), InkColor::Black);
                value_rows.push((ty, value.clone()));
            }
        });
        // Values share one right-aligned ink edge at pixel precision: the
        // tile-grid `label` API cannot flush 5 px proportional glyphs against
        // the border, so each value is drawn through the painter's pixel path
        // anchored so its ink ends at abs px 148 (3 px in from the interior's
        // 151 px edge).
        for (ty, value) in &value_rows {
            let painter = ui.painter();
            let px = (4 + 15) * 8 - 3 - painter.measure_text_px(value);
            painter.draw_text_px(px, (9 + ty) * 8, value, InkColor::Black.into());
        }
    }
}
