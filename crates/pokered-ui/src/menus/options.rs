use pokered_core::game_state::Lang;
use pokered_core::options_menu::{
    BattleAnimation, BattleStyle, OptionsMenuState, OptionsRow, TextSpeed,
};
#[cfg(not(target_os = "none"))]
use pokered_data::ui_layout::schema::get_screen_v2_json;
use pokered_data::ui_layout::schema::{OptionsDefaultLayout, OPTIONS_DEFAULT_LAYOUT};

use crate::engine::{Painter, Rgba, TilePos, Ui};
#[cfg(any(test, target_os = "none"))]
use crate::engine::TileRect;
#[cfg(not(target_os = "none"))]
use crate::v2::{self, DataContext};

fn enum_offset(layout: &OptionsDefaultLayout, key: &str) -> u32 {
    layout
        .enum_position_map
        .iter()
        .find_map(|(k, v)| if k == key { Some(*v as u32) } else { None })
        .unwrap_or(0)
}

/// Cursor x-offsets for the individually positioned Chinese choices in
/// `options.gui`. Each cursor sits one tile to the left of its label.
fn zh_enum_offset(key: &str) -> u32 {
    match key {
        "Medium" => 3,
        "Slow" => 6,
        "Off" => 8,
        "Set" => 6,
        // Fast / On / Shift and unknown keys all sit at offset 0.
        _ => 0,
    }
}

fn lang_enum_offset(layout: &OptionsDefaultLayout, key: &str, lang: Lang) -> u32 {
    match lang {
        Lang::Zh => zh_enum_offset(key),
        Lang::En => enum_offset(layout, key),
    }
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

/// Absolute tile position of the single visible options cursor.
pub fn cursor_position(
    state: &OptionsMenuState,
    layout: &OptionsDefaultLayout,
    lang: Lang,
) -> TilePos {
    let cursors = layout.cursors.as_ref();
    match state.row {
        OptionsRow::TextSpeed => TilePos::new(
            cursors[0].tx + 1 + lang_enum_offset(layout, text_speed_key(state), lang),
            cursors[0].base_ty + 1,
        ),
        OptionsRow::BattleAnimation => TilePos::new(
            cursors[1].tx
                + 1
                + lang_enum_offset(layout, battle_animation_key(state), lang),
            cursors[1].base_ty + 1,
        ),
        OptionsRow::BattleStyle => TilePos::new(
            cursors[2].tx + 1 + lang_enum_offset(layout, battle_style_key(state), lang),
            cursors[2].base_ty + 1,
        ),
        OptionsRow::Cancel => TilePos::new(cursors[3].tx, cursors[3].base_ty),
    }
}

/// Repaint only the two cursor cells of an already-rendered options screen.
pub fn redraw_cursor<P: Painter>(
    previous: TilePos,
    current: TilePos,
    painter: &mut P,
    lang: Lang,
) {
    // The proportional cursor advances by 10 px but its actual fallback ink
    // fits in 8x9.  Clearing the full advance would erase the adjacent CJK
    // option label, which starts one tile to the right.
    painter.draw_pixel_rect(previous.tx * 8, previous.ty * 8, 8, 9, Rgba::INK_WHITE);
    if lang == Lang::Zh {
        painter.draw_text_px(current.tx * 8, current.ty * 8, "▶", Rgba::INK_BLACK);
    } else {
        painter.draw_glyph(current, '▶', Rgba::INK_BLACK);
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
    #[cfg(target_os = "none")]
    {
        draw_compiled(state, ui.painter(), lang);
        return;
    }

    #[cfg(not(target_os = "none"))]
    draw_v2(state, ui, lang);
}

#[cfg(not(target_os = "none"))]
fn draw_v2<P: Painter>(state: &OptionsMenuState, ui: &mut Ui<P>, lang: Lang) {
    let Some(json) = get_screen_v2_json("options") else {
        return;
    };
    let Some(mut layout) = v2::parse_screen(json) else {
        return;
    };

    // Reuse the v1 cursor coordinates + enum-position map for pixel parity.
    let v1 = &OPTIONS_DEFAULT_LAYOUT;
    let cursors = v1.cursors.as_ref();

    // The Chinese font advances 10 px, wider than the legacy 8 px tile grid.
    if lang == Lang::Zh {
        layout.theme.text_mode = dotzuki_renderer::layout_engine::types::TextMode::Proportional;
    }

    let mut ctx = DataContext::new();

    // Rows 0..2 sit in bordered boxes (1-tile inset); the x-offset selects the
    // current enum value's column. Absolute = cursor.tx + 1 + offset, ty + 1.
    let c0 = &cursors[0];
    ctx.set("r0_tx", (c0.tx + 1 + lang_enum_offset(v1, text_speed_key(state), lang)) as i64);
    ctx.set("r0_ty", (c0.base_ty + 1) as i64);
    let c1 = &cursors[1];
    ctx.set("r1_tx", (c1.tx + 1 + lang_enum_offset(v1, battle_animation_key(state), lang)) as i64);
    ctx.set("r1_ty", (c1.base_ty + 1) as i64);
    let c2 = &cursors[2];
    ctx.set("r2_tx", (c2.tx + 1 + lang_enum_offset(v1, battle_style_key(state), lang)) as i64);
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
    ctx.set("is_zh", lang == Lang::Zh);
    ctx.set("is_en", lang == Lang::En);

    v2::render_screen(&layout, &ctx, ui.painter());
}

#[cfg(any(test, target_os = "none"))]
fn draw_compiled<P: Painter>(state: &OptionsMenuState, painter: &mut P, lang: Lang) {
    painter.clear(Rgba::INK_WHITE);
    painter.draw_text_box(TileRect::new(0, 0, 20, 5), Rgba::INK_BLACK);
    draw_label(painter, TilePos::new(1, 1), match lang {
        Lang::Zh => "文字速度",
        Lang::En => "TEXT SPEED",
    }, lang);
    match lang {
        Lang::En => draw_label(painter, TilePos::new(1, 3), " FAST  MEDIUM SLOW", lang),
        Lang::Zh => {
            draw_label(painter, TilePos::new(2, 3), "快", lang);
            draw_label(painter, TilePos::new(5, 3), "中", lang);
            draw_label(painter, TilePos::new(8, 3), "慢", lang);
        }
    }

    painter.draw_text_box(TileRect::new(0, 5, 20, 5), Rgba::INK_BLACK);
    draw_label(painter, TilePos::new(1, 6), match lang {
        Lang::Zh => "战斗动画",
        Lang::En => "BATTLE ANIMATION",
    }, lang);
    match lang {
        Lang::En => draw_label(painter, TilePos::new(1, 8), " ON       OFF", lang),
        Lang::Zh => {
            draw_label(painter, TilePos::new(2, 8), "开", lang);
            draw_label(painter, TilePos::new(10, 8), "关", lang);
        }
    }

    painter.draw_text_box(TileRect::new(0, 10, 20, 5), Rgba::INK_BLACK);
    draw_label(painter, TilePos::new(1, 11), match lang {
        Lang::Zh => "战斗模式",
        Lang::En => "BATTLE STYLE",
    }, lang);
    match lang {
        Lang::En => draw_label(painter, TilePos::new(1, 13), " SHIFT    SET", lang),
        Lang::Zh => {
            draw_label(painter, TilePos::new(2, 13), "替换", lang);
            draw_label(painter, TilePos::new(8, 13), "固定", lang);
        }
    }

    draw_label(painter, TilePos::new(2, 16), match lang {
        Lang::Zh => "取消",
        Lang::En => "CANCEL",
    }, lang);
    draw_cursor(painter, cursor_position(state, &OPTIONS_DEFAULT_LAYOUT, lang), lang);
}

#[cfg(any(test, target_os = "none"))]
fn draw_label<P: Painter>(painter: &mut P, position: TilePos, text: &str, lang: Lang) {
    if lang == Lang::Zh && painter.supports_proportional() {
        painter.draw_text_px_scaled(
            position.tx * 8,
            position.ty * 8,
            text,
            1,
            Rgba::INK_BLACK,
        );
    } else {
        for (offset, glyph) in text.chars().enumerate() {
            painter.draw_glyph(
                TilePos::new(position.tx + offset as u32, position.ty),
                glyph,
                Rgba::INK_BLACK,
            );
        }
    }
}

#[cfg(any(test, target_os = "none"))]
fn draw_cursor<P: Painter>(painter: &mut P, position: TilePos, lang: Lang) {
    if lang == Lang::Zh && painter.supports_proportional() {
        painter.draw_text_px(position.tx * 8, position.ty * 8, "▶", Rgba::INK_BLACK);
    } else {
        painter.draw_glyph(position, '▶', Rgba::INK_BLACK);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_prelude::{String, Vec};
    use pokered_core::options_menu::GameOptions;

    #[derive(Debug, PartialEq)]
    enum Op {
        Clear(Rgba),
        Box(TileRect, Rgba),
        Glyph(TilePos, char, Rgba),
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

        fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: Rgba) {
            self.0.push(Op::Glyph(pos, glyph, color));
        }

        fn draw_pixel_rect(&mut self, _px: u32, _py: u32, _pw: u32, _ph: u32, _color: Rgba) {}

        fn draw_gb_tile(&mut self, _pos: TilePos, _tile_id: u8, _fallback: &str, _color: Rgba) {}

        fn draw_text_px(&mut self, px: u32, py: u32, text: &str, color: Rgba) {
            self.0.push(Op::TextPx(px, py, text.into(), color));
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

    fn assert_compiled_matches_v2(state: &OptionsMenuState, lang: Lang) {
        let mut expected = Recorder::default();
        draw_v2(state, &mut Ui::new(&mut expected), lang);

        let mut actual = Recorder::default();
        draw_compiled(state, &mut actual, lang);

        assert_eq!(actual.0, expected.0, "state={state:?}, lang={lang:?}");
    }

    #[test]
    fn compiled_gba_layout_matches_v2_for_all_states_and_languages() {
        for lang in [Lang::En, Lang::Zh] {
            for row in [
                OptionsRow::TextSpeed,
                OptionsRow::BattleAnimation,
                OptionsRow::BattleStyle,
                OptionsRow::Cancel,
            ] {
                for text_speed in [TextSpeed::Fast, TextSpeed::Medium, TextSpeed::Slow] {
                    for battle_animation in [BattleAnimation::On, BattleAnimation::Off] {
                        for battle_style in [BattleStyle::Shift, BattleStyle::Set] {
                            let mut state = OptionsMenuState::new(GameOptions {
                                text_speed,
                                battle_animation,
                                battle_style,
                            });
                            state.row = row;
                            assert_compiled_matches_v2(&state, lang);
                        }
                    }
                }
            }
        }
    }
}
