use pokered_core::battle::menu::SafariBattleMenuState;
use pokered_core::game_state::Lang;
use pokered_data::ui_layout::schema::get_screen_v2_json;

use crate::engine::{Painter, Ui};
use crate::v2::{self, DataContext};

/// Safari battle action menu (BALL / BAIT / ROCK / RUN) — rendered from
/// `battle_safari.gui` as an OVERLAY on the battle scene, mirroring `battle_main`.
/// The 2×2 grid cursor is positioned by `{bcol}`/`{brow}` from the menu state.
pub fn draw<P: Painter>(state: &SafariBattleMenuState, ui: &mut Ui<P>, lang: Lang) {
    let Some(json) = get_screen_v2_json("battle_safari") else {
        return;
    };
    let Some(layout) = v2::parse_screen(json) else {
        return;
    };

    let mut ctx = DataContext::new();
    ctx.set("bcol", state.col() as i64);
    ctx.set("brow", state.row() as i64);
    ctx.set("__lang", v2::lang_code(lang));

    v2::render_screen_overlay(&layout, &ctx, ui.painter());
}
