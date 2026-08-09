use pokered_core::battle::menu::BattleMenuState;
use pokered_core::game_state::Lang;
use pokered_data::ui_layout::schema::{get_screen_v2_json, BattleMainDefaultLayout};

use crate::engine::{Painter, Ui};
use crate::v2::{self, DataContext};

/// Battle action menu (FIGHT / PKMN / ITEM / RUN) — rendered through the v2
/// layout engine from `battle_main.gui` as an OVERLAY on the battle scene.
///
/// The `_layout` (v1 `BattleMainDefaultLayout`) is kept for call-site
/// compatibility but unused. The 2×2 grid cursor is positioned by binding
/// `{bcol}`/`{brow}` from the menu state.
pub fn draw<P: Painter>(
    state: &BattleMenuState,
    _layout: &BattleMainDefaultLayout,
    ui: &mut Ui<P>,
    lang: Lang,
) {
    let Some(json) = get_screen_v2_json("battle_main") else {
        return;
    };
    let Some(layout) = v2::parse_screen(json) else {
        return;
    };

    let mut ctx = DataContext::new();
    ctx.set("bcol", state.col() as i64);
    ctx.set("brow", state.row() as i64);
    ctx.set("__lang", v2::lang_code(lang));

    // Overlay: the battle sprites are already in the framebuffer; do not clear.
    v2::render_screen_overlay(&layout, &ctx, ui.painter());
}
