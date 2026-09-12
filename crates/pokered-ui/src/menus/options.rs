use pokered_core::game_state::Lang;
use pokered_core::options_menu::{
    BattleAnimation, BattleStyle, OptionsMenuState, OptionsRow, TextSpeed,
};
#[cfg(not(target_os = "none"))]
use pokered_data::ui_layout::schema::get_screen_v2_json;
use pokered_data::ui_layout::schema::OptionsDefaultLayout;

#[cfg(any(test, target_os = "none"))]
use crate::engine::TileRect;
use crate::engine::{Painter, Rgba, TilePos, Ui};
use crate::v2;

/// Active cursor specification resolved from the compiled GUI source.
pub fn cursor_spec(state: &OptionsMenuState, lang: Lang) -> (TilePos, char) {
    pokered_data::ui_layout::schema::OPTIONS_STATIC_LAYOUT
        .cursor(&bindings(state, lang))
        .expect("options layout must declare exactly one active cursor")
}

/// Absolute tile position of the single visible options cursor. The legacy
/// layout parameter remains for call-site compatibility but is not authoritative.
pub fn cursor_position(
    state: &OptionsMenuState,
    _layout: &OptionsDefaultLayout,
    lang: Lang,
) -> TilePos {
    cursor_spec(state, lang).0
}

pub fn cursor_damage(cursor: TilePos) -> crate::DamageRect {
    crate::DamageRect::cursor(cursor)
}

/// Repaint only the two cursor cells of an already-rendered options screen.
pub fn redraw_cursor<P: Painter>(
    previous: (TilePos, char),
    current: (TilePos, char),
    painter: &mut P,
    lang: Lang,
) {
    // The proportional cursor advances by 10 px but its actual fallback ink
    // fits in 8x9.  Clearing the full advance would erase the adjacent CJK
    // option label, which starts one tile to the right.
    painter.draw_pixel_rect(previous.0.tx * 8, previous.0.ty * 8, 8, 9, Rgba::INK_WHITE);
    dotzuki_renderer::layout_engine::elements::cursor::draw_cursor_glyph(
        current.0,
        current.1,
        Rgba::INK_BLACK,
        lang == Lang::Zh && painter.supports_proportional(),
        painter,
    );
}

/// Options screen — rendered through the v2 layout engine from `options.gui`.
///
/// Cursor labels and geometry are both authored in `options.gui`; bindings
/// select one semantic state without copying coordinates into Rust.
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

    if lang == Lang::Zh {
        layout.theme.text_mode = dotzuki_renderer::layout_engine::types::TextMode::Proportional;
    }
    let ctx = bindings(state, lang).dynamic();

    v2::render_screen(&layout, &ctx, ui.painter());
}

fn bindings(
    state: &OptionsMenuState,
    lang: Lang,
) -> dotzuki_renderer::layout_engine::static_layout::Context<'static> {
    let mut ctx = dotzuki_renderer::layout_engine::static_layout::Context::new();
    let en = lang == Lang::En;
    let zh = lang == Lang::Zh;
    let text = state.row == OptionsRow::TextSpeed;
    let animation = state.row == OptionsRow::BattleAnimation;
    let style = state.row == OptionsRow::BattleStyle;
    ctx.set(
        "text_fast_en",
        text && en && state.options.text_speed == TextSpeed::Fast,
    );
    ctx.set(
        "text_medium_en",
        text && en && state.options.text_speed == TextSpeed::Medium,
    );
    ctx.set(
        "text_slow_en",
        text && en && state.options.text_speed == TextSpeed::Slow,
    );
    ctx.set(
        "text_fast_zh",
        text && zh && state.options.text_speed == TextSpeed::Fast,
    );
    ctx.set(
        "text_medium_zh",
        text && zh && state.options.text_speed == TextSpeed::Medium,
    );
    ctx.set(
        "text_slow_zh",
        text && zh && state.options.text_speed == TextSpeed::Slow,
    );
    ctx.set(
        "animation_on_en",
        animation && en && state.options.battle_animation == BattleAnimation::On,
    );
    ctx.set(
        "animation_off_en",
        animation && en && state.options.battle_animation == BattleAnimation::Off,
    );
    ctx.set(
        "animation_on_zh",
        animation && zh && state.options.battle_animation == BattleAnimation::On,
    );
    ctx.set(
        "animation_off_zh",
        animation && zh && state.options.battle_animation == BattleAnimation::Off,
    );
    ctx.set(
        "style_shift_en",
        style && en && state.options.battle_style == BattleStyle::Shift,
    );
    ctx.set(
        "style_set_en",
        style && en && state.options.battle_style == BattleStyle::Set,
    );
    ctx.set(
        "style_shift_zh",
        style && zh && state.options.battle_style == BattleStyle::Shift,
    );
    ctx.set(
        "style_set_zh",
        style && zh && state.options.battle_style == BattleStyle::Set,
    );
    ctx.set("cancel_active", state.row == OptionsRow::Cancel);
    ctx.set("__lang", v2::lang_code(lang));
    ctx.set("is_zh", lang == Lang::Zh);
    ctx.set("is_en", lang == Lang::En);

    ctx
}

#[cfg(any(test, target_os = "none"))]
fn draw_compiled<P: Painter>(state: &OptionsMenuState, painter: &mut P, lang: Lang) {
    pokered_data::ui_layout::schema::OPTIONS_STATIC_LAYOUT.render(
        &bindings(state, lang),
        painter,
        lang == Lang::Zh,
        true,
    );
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

        fn draw_text_px_scaled(&mut self, px: u32, py: u32, text: &str, scale: u32, color: Rgba) {
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
