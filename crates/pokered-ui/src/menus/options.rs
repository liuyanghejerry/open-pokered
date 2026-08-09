use pokered_core::game_state::Lang;
use pokered_core::options_menu::{
    BattleAnimation, BattleStyle, OptionsMenuState, OptionsRow, TextSpeed,
};
use pokered_data::ui_layout::schema::{
    get_screen_v2_json, OptionsDefaultLayout, OPTIONS_DEFAULT_LAYOUT,
};

use crate::engine::{Painter, Ui};
use crate::v2::{self, DataContext};

fn enum_offset(layout: &OptionsDefaultLayout, key: &str) -> u32 {
    layout
        .enum_position_map
        .iter()
        .find_map(|(k, v)| if k == key { Some(*v as u32) } else { None })
        .unwrap_or(0)
}

fn text_speed_key(state: &OptionsMenuState) -> &'static str {
    match state.options.text_speed {
        TextSpeed::Fast => "Fast",
        TextSpeed::Medium => "Medium",
        TextSpeed::Slow => "Slow",
    }
}

fn battle_animation_key(state: &OptionsMenuState) -> &'static str {
    match state.options.battle_animation {
        BattleAnimation::On => "On",
        BattleAnimation::Off => "Off",
    }
}

fn battle_style_key(state: &OptionsMenuState) -> &'static str {
    match state.options.battle_style {
        BattleStyle::Shift => "Shift",
        BattleStyle::Set => "Set",
    }
}

/// Options screen — rendered through the v2 layout engine from `options.gui`.
///
/// Single cursor: a ▶ on the active row only, at the selected option's
/// x-position. The absolute cursor positions replicate the v1 math (box inset
/// + per-enum x-offset from the v1 `enum_position_map`) and are fed to the
/// `.gui` cursor elements as `{rN_tx}`/`{rN_ty}` bindings; `{rN_active}`
/// toggles which row's ▶ is shown.
pub fn draw<P: Painter>(
    state: &OptionsMenuState,
    _layout: &OptionsDefaultLayout,
    ui: &mut Ui<P>,
    lang: Lang,
) {
    let Some(json) = get_screen_v2_json("options") else {
        return;
    };
    let Some(layout) = v2::parse_screen(json) else {
        return;
    };

    // Reuse the v1 cursor coordinates + enum-position map for pixel parity.
    let v1 = &OPTIONS_DEFAULT_LAYOUT;
    let cursors = v1.cursors.as_ref();

    let mut ctx = DataContext::new();

    // Rows 0..2 sit in bordered boxes (1-tile inset); the x-offset selects the
    // current enum value's column. Absolute = cursor.tx + 1 + offset, ty + 1.
    let c0 = &cursors[0];
    ctx.set("r0_tx", (c0.tx + 1 + enum_offset(v1, text_speed_key(state))) as i64);
    ctx.set("r0_ty", (c0.base_ty + 1) as i64);
    let c1 = &cursors[1];
    ctx.set("r1_tx", (c1.tx + 1 + enum_offset(v1, battle_animation_key(state))) as i64);
    ctx.set("r1_ty", (c1.base_ty + 1) as i64);
    let c2 = &cursors[2];
    ctx.set("r2_tx", (c2.tx + 1 + enum_offset(v1, battle_style_key(state))) as i64);
    ctx.set("r2_ty", (c2.base_ty + 1) as i64);
    // Cancel sits in a borderless region (no inset, no enum offset).
    let c3 = &cursors[3];
    ctx.set("r3_tx", c3.tx as i64);
    ctx.set("r3_ty", c3.base_ty as i64);

    ctx.set("r0_active", state.row == OptionsRow::TextSpeed);
    ctx.set("r1_active", state.row == OptionsRow::BattleAnimation);
    ctx.set("r2_active", state.row == OptionsRow::BattleStyle);
    ctx.set("r3_active", state.row == OptionsRow::Cancel);
    ctx.set("__lang", v2::lang_code(lang));

    v2::render_screen(&layout, &ctx, ui.painter());
}
