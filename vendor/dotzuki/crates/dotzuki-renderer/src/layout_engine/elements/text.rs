//! Text element — renders text with template variable resolution, word
//! wrapping, alignment, and colour mapping.
//!
//! ## Features
//! - Template variable resolution via [`DataContext::resolve`]
//! - Word wrapping when `TextParams::wrap` is `true`
//! - Left / centre / right alignment within the element rect
//! - Colour mapping from named ink-ramp strings and `#RRGGBB` hex literals

use dotzuki_engine::render::painter::Painter;
use dotzuki_engine::render::{Rgba, TilePos};

use crate::layout_engine::types::{
    DataContext, LayoutElement, RenderContext, RenderError, TextAlign, TextParams,
};

// ── Public API ──────────────────────────────────────────────────────────────

/// Render a text element into the framebuffer via `painter`.
///
/// # Arguments
/// * `element` — The layout element containing position (`rect`) and
///   text-specific parameters.
/// * `params` — Deserialised [`TextParams`].
/// * `ctx` — Data context for resolving template variables.
/// * `_render_ctx` — Shared rendering state (fonts, tilesets, theme).
/// * `painter` — Drawing backend.
pub fn render_text(
    element: &LayoutElement,
    params: &TextParams,
    ctx: &DataContext,
    render_ctx: &RenderContext,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    // ── Select language variant, then resolve template variables ──
    // `params.value` may be a plain string or a `@t("en", "中文")` per-locale
    // map; pick the active language (`DataContext::lang`) before substitution.
    let localized = params.value.get(ctx.lang());
    let resolved_text = ctx.resolve(localized);

    // ── Rect dimensions (tile grid) ──
    let rect = &element.rect;
    let tile_width = rect.tw.unwrap_or(20);
    let tile_height = rect.th.unwrap_or(18);
    let base_tx = rect.tx.resolve(ctx);
    let base_ty = rect.ty.resolve(ctx);
    let align = params.align.as_ref().unwrap_or(&TextAlign::Left);
    let wrap = params.wrap.as_deref() == Some("word");

    // ── Proportional (pixel-precise) path — high-resolution / CJK screens ──
    // Selected only when the theme opts in AND the painter supports it; pokered
    // (Theme::default() = Tile, recording mocks) always falls through to the
    // legacy tile path below, which is preserved byte-for-byte.
    let theme = render_ctx.theme;
    if theme.proportional(painter.supports_proportional()) {
        let color = params
            .color
            .as_deref()
            .map(parse_color)
            .unwrap_or_else(|| theme.ink_color());
        let base_px = base_tx * 8;
        let base_py = base_ty * 8;
        let width_px = tile_width * 8;
        let height_px = tile_height * 8;
        // Integer scale factor (1 = normal). Big title/heading text scales every
        // glyph pixel into a block; row pitch and measurement scale with it.
        let scale = params.scale.unwrap_or(1).max(1);
        // Row pitch: full CJK glyph height + a little leading (+ optional spacing).
        let line_h = (crate::embedded_font::GLYPH_SIZE + 3) * scale
            + params.line_spacing.unwrap_or(0) as u32;
        let lines = if wrap {
            word_wrap_px(&resolved_text, width_px, &*painter)
        } else {
            resolved_text.split('\n').map(|l| l.to_string()).collect()
        };
        let mut y = base_py;
        for line in &lines {
            if y >= base_py + height_px {
                break; // overflowed the rect
            }
            let w = painter.measure_text_px_scaled(line, scale);
            let off = match align {
                TextAlign::Left => 0,
                TextAlign::Center => width_px.saturating_sub(w) / 2,
                TextAlign::Right => width_px.saturating_sub(w),
            };
            painter.draw_text_px_scaled(base_px + off, y, line, scale, color);
            y += line_h;
        }
        return Ok(());
    }

    // ── Legacy tile path (Game Boy 8×8 grid) — byte-identical to before ──
    let color = params
        .color
        .as_deref()
        .map(parse_color)
        .unwrap_or(Rgba::INK_BLACK);
    let max_chars = tile_width as usize;
    let lines = if wrap {
        word_wrap(&resolved_text, max_chars)
    } else {
        hard_break_lines(&resolved_text, max_chars)
    };
    let line_spacing = params.line_spacing.unwrap_or(0) as u32;
    for (line_idx, line) in lines.iter().enumerate() {
        let row = base_ty + line_idx as u32 * (1 + line_spacing);
        if row >= base_ty + tile_height {
            break; // would overflow the allocated rect
        }

        let text_width = line.chars().count().min(max_chars) as u32;
        let offset_x = align_offset(text_width, tile_width, align);

        for (char_idx, ch) in line.chars().enumerate() {
            if char_idx >= max_chars {
                break;
            }
            let col = base_tx + offset_x + char_idx as u32;
            painter.draw_glyph(TilePos::new(col, row), ch, color);
        }
    }

    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Parse a colour string to an [`Rgba`].
///
/// Accepts the named ink-ramp shades — `"black"`, `"darkgray"` /
/// `"dark_gray"`, `"lightgray"` / `"light_gray"`, `"white"` — and hex
/// literals `#RGB`, `#RRGGBB`, or `#RRGGBBAA`. Unrecognised strings fall
/// back to [`Rgba::INK_BLACK`].
pub fn parse_color(s: &str) -> Rgba {
    if let Some(hex) = s.strip_prefix('#') {
        if let Some(c) = parse_hex_color(hex) {
            return c;
        }
    }
    match s.to_lowercase().as_str() {
        "black" => Rgba::INK_BLACK,
        "darkgray" | "dark_gray" => Rgba::INK_DARK_GRAY,
        "lightgray" | "light_gray" => Rgba::INK_LIGHT_GRAY,
        "white" => Rgba::INK_WHITE,
        _ => Rgba::INK_BLACK,
    }
}

fn parse_hex_color(hex: &str) -> Option<Rgba> {
    let nibble = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let byte = |hi: u8, lo: u8| Some(nibble(hi)? * 16 + nibble(lo)?);
    let b = hex.as_bytes();
    match b.len() {
        3 => Some(Rgba::rgb(
            byte(b[0], b[0])?,
            byte(b[1], b[1])?,
            byte(b[2], b[2])?,
        )),
        6 => Some(Rgba::rgb(
            byte(b[0], b[1])?,
            byte(b[2], b[3])?,
            byte(b[4], b[5])?,
        )),
        8 => Some(Rgba::new(
            byte(b[0], b[1])?,
            byte(b[2], b[3])?,
            byte(b[4], b[5])?,
            byte(b[6], b[7])?,
        )),
        _ => None,
    }
}

/// Split text on explicit newlines, then truncate each line to `max_chars`.
fn hard_break_lines(text: &str, max_chars: usize) -> Vec<String> {
    text.lines()
        .map(|line| {
            if line.chars().count() > max_chars {
                line.chars().take(max_chars).collect()
            } else {
                line.to_string()
            }
        })
        .collect()
}

/// Word-wrap `text` to fit within `max_chars` per line, breaking at
/// space boundaries when possible.  Words longer than a line are
/// hard-broken.
pub fn word_wrap(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            if word.chars().count() > max_chars {
                // Long word — force-break
                for chunk in chunk_str(word, max_chars) {
                    lines.push(chunk.to_string());
                }
            } else {
                current = word.to_string();
            }
        } else if current.chars().count() + 1 + word.chars().count() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(core::mem::take(&mut current));
            if word.chars().count() > max_chars {
                for chunk in chunk_str(word, max_chars) {
                    lines.push(chunk.to_string());
                }
            } else {
                current = word.to_string();
            }
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

/// Word-wrap `text` to a pixel `max_px` width using the painter's proportional
/// font metrics ([`Painter::measure_text_px`]). Breaks at spaces where possible;
/// a run wider than a line (e.g. CJK with no spaces) is split per-character by
/// measured width. Explicit `\n` start new lines. Used by the proportional path.
pub fn word_wrap_px(text: &str, max_px: u32, painter: &dyn Painter) -> Vec<String> {
    let max_px = max_px.max(1);
    let space_px = painter.measure_text_px(" ");
    let mut lines: Vec<String> = Vec::new();

    for raw in text.split('\n') {
        let mut current = String::new();
        let mut cur_px = 0u32;
        for word in raw.split(' ') {
            if word.is_empty() {
                continue;
            }
            let word_px = painter.measure_text_px(word);
            if word_px <= max_px {
                let sep = if current.is_empty() { 0 } else { space_px };
                if cur_px + sep + word_px <= max_px {
                    if !current.is_empty() {
                        current.push(' ');
                        cur_px += sep;
                    }
                    current.push_str(word);
                    cur_px += word_px;
                } else {
                    lines.push(core::mem::take(&mut current));
                    current.push_str(word);
                    cur_px = word_px;
                }
            } else {
                // Word wider than a whole line — break it character by character.
                if !current.is_empty() {
                    lines.push(core::mem::take(&mut current));
                    cur_px = 0;
                }
                for ch in word.chars() {
                    let ch_px = painter.measure_text_px(ch.encode_utf8(&mut [0u8; 4]));
                    if !current.is_empty() && cur_px + ch_px > max_px {
                        lines.push(core::mem::take(&mut current));
                        cur_px = 0;
                    }
                    current.push(ch);
                    cur_px += ch_px;
                }
            }
        }
        lines.push(current); // preserve blank lines from explicit \n
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Split a string into chunks of at most `max_chars` characters.
fn chunk_str(s: &str, max_chars: usize) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut remaining = s;
    while !remaining.is_empty() {
        let end = remaining
            .char_indices()
            .take(max_chars)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(remaining.len());
        chunks.push(&remaining[..end]);
        remaining = &remaining[end..];
    }
    chunks
}

/// Compute horizontal tile offset for a given alignment.
///
/// Returns the number of tile columns to shift right so that a
/// `text_width`-tile-wide string sits at the left, centre, or right
/// of an `available_width`-tile-wide area.
pub fn align_offset(text_width: u32, available_width: u32, align: &TextAlign) -> u32 {
    match align {
        TextAlign::Left => 0,
        TextAlign::Center => {
            if text_width >= available_width {
                0
            } else {
                (available_width - text_width) / 2
            }
        }
        TextAlign::Right => available_width.saturating_sub(text_width),
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_engine::types::{Coord, ElementParams, ElementRect};
    use dotzuki_engine::render::Rgba as EngineRgba;

    // ── Recording painter ────────────────────────────────────────────

    #[derive(Debug, Default)]
    struct RecordingPainter {
        glyphs: Vec<(TilePos, char, EngineRgba)>,
        texts: Vec<(TilePos, String, EngineRgba)>,
    }

    impl RecordingPainter {
        fn new() -> Self {
            Self::default()
        }

        fn glyph_at(&self, tx: u32, ty: u32) -> Option<char> {
            self.glyphs
                .iter()
                .find(|(pos, _, _)| pos.tx == tx && pos.ty == ty)
                .map(|(_, ch, _)| *ch)
        }
    }

    impl Painter for RecordingPainter {
        fn clear(&mut self, _color: EngineRgba) {}

        fn draw_text_box(&mut self, _rect: dotzuki_engine::render::TileRect, _color: EngineRgba) {}

        fn draw_text(&mut self, pos: TilePos, text: &str, color: EngineRgba) {
            self.texts.push((pos, text.to_string(), color));
        }

        fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: EngineRgba) {
            self.glyphs.push((pos, glyph, color));
        }

        fn draw_pixel_rect(&mut self, _px: u32, _py: u32, _pw: u32, _ph: u32, _color: EngineRgba) {}

        fn draw_gb_tile(
            &mut self,
            _pos: TilePos,
            _tile_id: u8,
            _fallback: &str,
            _color: EngineRgba,
        ) {
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────

    fn make_element(value: &str, tx: u32, ty: u32, tw: u32, th: u32) -> LayoutElement {
        LayoutElement {
            id: String::new(),
            element_type: "text".to_string(),
            rect: ElementRect {
                tx: Coord::Literal(tx),
                ty: Coord::Literal(ty),
                tw: Some(tw),
                th: Some(th),
            },
            visible: crate::layout_engine::types::Visibility::Static(true),
            z_index: 0,
            params: ElementParams::Text(TextParams {
                value: value.into(),
                format: None,
                color: None,
                align: None,
                font: None,
                wrap: None,
                line_spacing: None,
                scale: None,
            }),
        }
    }

    fn make_theme() -> crate::layout_engine::types::Theme {
        Default::default()
    }

    fn render_ctx<'a>(
        theme: &'a crate::layout_engine::types::Theme,
        fonts: &'a dotzuki_engine::hash::HashMap<String, ()>,
        tilesets: &'a dotzuki_engine::hash::HashMap<String, ()>,
    ) -> RenderContext<'a> {
        RenderContext {
            screen: "test",
            theme,
            fonts,
            tilesets,
            images: crate::layout_engine::types::empty_image_registry(),
        }
    }

    // ── Tests: parse_color ───────────────────────────────────────────

    #[test]
    fn parse_color_black() {
        assert_eq!(parse_color("black"), Rgba::INK_BLACK);
    }

    #[test]
    fn parse_color_darkgray_variants() {
        assert_eq!(parse_color("darkgray"), Rgba::INK_DARK_GRAY);
        assert_eq!(parse_color("dark_gray"), Rgba::INK_DARK_GRAY);
    }

    #[test]
    fn parse_color_lightgray_variants() {
        assert_eq!(parse_color("lightgray"), Rgba::INK_LIGHT_GRAY);
        assert_eq!(parse_color("light_gray"), Rgba::INK_LIGHT_GRAY);
    }

    #[test]
    fn parse_color_white() {
        assert_eq!(parse_color("white"), Rgba::INK_WHITE);
    }

    #[test]
    fn parse_color_case_insensitive() {
        assert_eq!(parse_color("BLACK"), Rgba::INK_BLACK);
        assert_eq!(parse_color("White"), Rgba::INK_WHITE);
    }

    #[test]
    fn parse_color_unknown_returns_black() {
        assert_eq!(parse_color("red"), Rgba::INK_BLACK);
        assert_eq!(parse_color(""), Rgba::INK_BLACK);
    }

    // ── Tests: word_wrap ─────────────────────────────────────────────

    #[test]
    fn word_wrap_short_text() {
        assert_eq!(word_wrap("hello", 10), vec!["hello"]);
    }

    #[test]
    fn word_wrap_splits_at_space() {
        let lines = word_wrap("hello world test", 10);
        assert_eq!(lines, vec!["hello", "world test"]);
    }

    #[test]
    fn word_wrap_exact_fit() {
        assert_eq!(word_wrap("12345 12345", 5), vec!["12345", "12345"]);
    }

    #[test]
    fn word_wrap_long_word_breaks() {
        let lines = word_wrap("supercalifragilistic", 5);
        for line in &lines {
            assert!(line.chars().count() <= 5, "{:?} too long", line);
        }
        assert!(lines.len() > 1);
    }

    #[test]
    fn word_wrap_empty_returns_empty_line() {
        assert_eq!(word_wrap("", 10), vec![""]);
    }

    #[test]
    fn word_wrap_preserves_spaces_in_result() {
        let lines = word_wrap("a b c", 3);
        // "a b" fits, "c" on next line
        assert_eq!(lines, vec!["a b", "c"]);
    }

    // ── Tests: align_offset ──────────────────────────────────────────

    #[test]
    fn align_left_is_zero() {
        assert_eq!(align_offset(5, 20, &TextAlign::Left), 0);
    }

    #[test]
    fn align_center() {
        assert_eq!(align_offset(5, 20, &TextAlign::Center), 7); // (20-5)/2
        assert_eq!(align_offset(4, 20, &TextAlign::Center), 8); // (20-4)/2
    }

    #[test]
    fn align_center_clamped() {
        assert_eq!(align_offset(25, 20, &TextAlign::Center), 0);
    }

    #[test]
    fn align_right() {
        assert_eq!(align_offset(5, 20, &TextAlign::Right), 15); // 20-5
        assert_eq!(align_offset(25, 20, &TextAlign::Right), 0); // saturating
    }

    // ── Tests: render_text ───────────────────────────────────────────

    #[test]
    fn render_simple_text() {
        let elem = make_element("Hello", 2, 3, 20, 18);
        let params = match &elem.params {
            ElementParams::Text(p) => p,
            _ => unreachable!(),
        };
        let ctx = DataContext::new();
        let theme = Default::default();
        let fonts = dotzuki_engine::hash::HashMap::default();
        let tilesets = dotzuki_engine::hash::HashMap::default();
        let rc = render_ctx(&theme, &fonts, &tilesets);
        let mut p = RecordingPainter::new();

        render_text(&elem, params, &ctx, &rc, &mut p).unwrap();

        assert_eq!(p.glyphs.len(), 5);
        assert_eq!(p.glyph_at(2, 3), Some('H'));
        assert_eq!(p.glyph_at(3, 3), Some('e'));
        assert_eq!(p.glyph_at(6, 3), Some('o'));
    }

    #[test]
    fn render_template_resolution() {
        let elem = make_element("{name} Lv{level}", 0, 0, 20, 18);
        let params = match &elem.params {
            ElementParams::Text(p) => p,
            _ => unreachable!(),
        };
        let mut ctx = DataContext::new();
        ctx.set("name", "SPARKIT");
        ctx.set("level", 25i64);
        let theme = make_theme();
        let fonts: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let tilesets: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let rc = render_ctx(&theme, &fonts, &tilesets);
        let mut p = RecordingPainter::new();

        render_text(&elem, params, &ctx, &rc, &mut p).unwrap();

        // Should produce "SPARKIT Lv25"
        let rendered: String = p.glyphs.iter().map(|(_, ch, _)| *ch).collect();
        assert_eq!(rendered, "SPARKIT Lv25");
    }

    #[test]
    fn localized_value_get_selects_and_falls_back() {
        use crate::layout_engine::types::LocalizedValue;
        let mut m = alloc::collections::BTreeMap::new();
        m.insert("en".to_string(), "YES".to_string());
        m.insert("zh".to_string(), "是".to_string());
        let lv = LocalizedValue::Localized(m);
        assert_eq!(lv.get("zh"), "是");
        assert_eq!(lv.get("en"), "YES");
        // Unknown locale falls back to `en`.
        assert_eq!(lv.get("ja"), "YES");
        // Plain returns itself for any locale.
        assert_eq!(LocalizedValue::Plain("HI".into()).get("zh"), "HI");
    }

    #[test]
    fn render_localized_text_picks_active_language() {
        use crate::layout_engine::types::LocalizedValue;
        let mut elem = make_element("placeholder", 0, 0, 20, 18);
        if let ElementParams::Text(ref mut tp) = elem.params {
            let mut m = alloc::collections::BTreeMap::new();
            m.insert("en".to_string(), "YES".to_string());
            m.insert("zh".to_string(), "是".to_string());
            tp.value = LocalizedValue::Localized(m);
        }
        let params = match &elem.params {
            ElementParams::Text(p) => p,
            _ => unreachable!(),
        };
        let theme = make_theme();
        let fonts: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let tilesets: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let rc = render_ctx(&theme, &fonts, &tilesets);

        // Chinese: `__lang = "zh"` selects the zh variant.
        let mut ctx = DataContext::new();
        ctx.set("__lang", "zh");
        let mut p = RecordingPainter::new();
        render_text(&elem, params, &ctx, &rc, &mut p).unwrap();
        let zh: String = p.glyphs.iter().map(|(_, ch, _)| *ch).collect();
        assert_eq!(zh, "是");

        // Default (no `__lang`) falls back to English.
        let ctx_default = DataContext::new();
        let mut p2 = RecordingPainter::new();
        render_text(&elem, params, &ctx_default, &rc, &mut p2).unwrap();
        let en: String = p2.glyphs.iter().map(|(_, ch, _)| *ch).collect();
        assert_eq!(en, "YES");
    }

    #[test]
    fn render_center_aligned() {
        let mut elem = make_element("AB", 0, 0, 10, 18);
        if let ElementParams::Text(ref mut tp) = elem.params {
            tp.align = Some(TextAlign::Center);
        }
        let params = match &elem.params {
            ElementParams::Text(p) => p,
            _ => unreachable!(),
        };
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let tilesets: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let rc = render_ctx(&theme, &fonts, &tilesets);
        let mut p = RecordingPainter::new();

        render_text(&elem, params, &ctx, &rc, &mut p).unwrap();

        // width=10, text="AB"=2 tiles → offset (10-2)/2=4
        assert_eq!(p.glyph_at(4, 0), Some('A'));
        assert_eq!(p.glyph_at(5, 0), Some('B'));
    }

    #[test]
    fn render_right_aligned() {
        let mut elem = make_element("X", 0, 0, 5, 18);
        if let ElementParams::Text(ref mut tp) = elem.params {
            tp.align = Some(TextAlign::Right);
        }
        let params = match &elem.params {
            ElementParams::Text(p) => p,
            _ => unreachable!(),
        };
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let tilesets: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let rc = render_ctx(&theme, &fonts, &tilesets);
        let mut p = RecordingPainter::new();

        render_text(&elem, params, &ctx, &rc, &mut p).unwrap();

        // width=5, text="X"=1 → offset 5-1=4
        assert_eq!(p.glyph_at(4, 0), Some('X'));
    }

    #[test]
    fn render_word_wrap() {
        let mut elem = make_element("hello world foo bar baz", 0, 0, 6, 18);
        if let ElementParams::Text(ref mut tp) = elem.params {
            tp.wrap = Some("word".to_string());
        }
        let params = match &elem.params {
            ElementParams::Text(p) => p,
            _ => unreachable!(),
        };
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let tilesets: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let rc = render_ctx(&theme, &fonts, &tilesets);
        let mut p = RecordingPainter::new();

        render_text(&elem, params, &ctx, &rc, &mut p).unwrap();

        // Line 0: "hello " → ty=0; Line 1: "world " → ty=1; etc.
        let has_row_0 = p.glyphs.iter().any(|(pos, _, _)| pos.ty == 0);
        let has_row_1 = p.glyphs.iter().any(|(pos, _, _)| pos.ty == 1);
        let has_row_2 = p.glyphs.iter().any(|(pos, _, _)| pos.ty == 2);
        assert!(has_row_0);
        assert!(has_row_1);
        assert!(has_row_2);
    }

    #[test]
    fn render_color_from_params() {
        let mut elem = make_element("Hi", 0, 0, 20, 18);
        if let ElementParams::Text(ref mut tp) = elem.params {
            tp.color = Some("white".to_string());
        }
        let params = match &elem.params {
            ElementParams::Text(p) => p,
            _ => unreachable!(),
        };
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let tilesets: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let rc = render_ctx(&theme, &fonts, &tilesets);
        let mut p = RecordingPainter::new();

        render_text(&elem, params, &ctx, &rc, &mut p).unwrap();

        assert!(!p.glyphs.is_empty());
        assert_eq!(p.glyphs[0].2, Rgba::INK_WHITE);
    }

    #[test]
    fn render_clips_to_rect_height() {
        let elem = make_element("A\nB\nC\nD\nE", 0, 0, 20, 2);
        let params = match &elem.params {
            ElementParams::Text(p) => p,
            _ => unreachable!(),
        };
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let tilesets: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let rc = render_ctx(&theme, &fonts, &tilesets);
        let mut p = RecordingPainter::new();

        render_text(&elem, params, &ctx, &rc, &mut p).unwrap();

        // Only rows 0 and 1 should be drawn (th=2)
        let max_ty = p.glyphs.iter().map(|(pos, _, _)| pos.ty).max().unwrap_or(0);
        assert!(max_ty < 2, "expected rows < 2, got max_ty={}", max_ty);
    }

    #[test]
    fn render_font_config() {
        // font field is accepted but currently unused (font selection is
        // handled by the render context at a higher level)
        let mut elem = make_element("OK", 0, 0, 20, 18);
        if let ElementParams::Text(ref mut tp) = elem.params {
            tp.font = Some("battle".to_string());
        }
        let params = match &elem.params {
            ElementParams::Text(p) => p,
            _ => unreachable!(),
        };
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let tilesets: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let rc = render_ctx(&theme, &fonts, &tilesets);
        let mut p = RecordingPainter::new();

        let result = render_text(&elem, params, &ctx, &rc, &mut p);
        assert!(result.is_ok());
        let rendered: String = p.glyphs.iter().map(|(_, ch, _)| *ch).collect();
        assert_eq!(rendered, "OK");
    }

    #[test]
    fn render_line_spacing() {
        let mut elem = make_element("A\nB", 0, 0, 20, 18);
        if let ElementParams::Text(ref mut tp) = elem.params {
            tp.line_spacing = Some(2); // 2 extra rows between lines
        }
        let params = match &elem.params {
            ElementParams::Text(p) => p,
            _ => unreachable!(),
        };
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let tilesets: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let rc = render_ctx(&theme, &fonts, &tilesets);
        let mut p = RecordingPainter::new();

        render_text(&elem, params, &ctx, &rc, &mut p).unwrap();

        // "A" at ty=0, "B" at ty = 0 + 1*(1+2) = 3
        assert_eq!(p.glyph_at(0, 0), Some('A'));
        assert_eq!(p.glyph_at(0, 3), Some('B'));
    }

    // ── Tests: chunk_str ─────────────────────────────────────────────

    #[test]
    fn chunk_str_simple() {
        assert_eq!(chunk_str("abc", 2), vec!["ab", "c"]);
    }

    #[test]
    fn chunk_str_exact_multiple() {
        assert_eq!(chunk_str("abcd", 2), vec!["ab", "cd"]);
    }

    #[test]
    fn chunk_str_empty() {
        let v: Vec<&str> = Vec::new();
        assert_eq!(chunk_str("", 5), v);
    }
}
