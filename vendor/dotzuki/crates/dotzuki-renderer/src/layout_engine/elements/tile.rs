//! Tile element — renders a single Game Boy tile, with optional repeat
//! to fill a rectangle, flip support, and template variable resolution
//! for the tile id.
//!
//! ## Features
//! - `tile_id` can be a literal number, a string, or a `{template}` variable
//! - `repeat` fills the element rect with the tile
//! - `flip_x` / `flip_y` for horizontal / vertical mirroring (requires
//!   [`render_tile_with_tiles`] for pixel-level flip; the Painter-based
//!   [`render_tile`] passes the flags through but flip fidelity depends
//!   on the backend)

use dotzuki_engine::render::painter::Painter;
use dotzuki_engine::render::{Rgba, TilePos};

use crate::layout_engine::types::{
    DataContext, LayoutElement, RenderContext, RenderError, TileParams,
};

// ── Public API ──────────────────────────────────────────────────────────────

/// Render a tile element via the [`Painter`] backend.
///
/// Handles template resolution, repeat-to-fill, and colour mapping.
/// Flip flags are passed through to [`Painter::draw_gb_tile`]; pixel-level
/// flip fidelity depends on the backend implementation.
///
/// # Arguments
/// * `element` — The layout element (position from `rect`).
/// * `params` — Deserialised [`TileParams`].
/// * `ctx` — Data context for resolving `{template}` tile ids.
/// * `_render_ctx` — Shared rendering state.
/// * `painter` — Drawing backend.
pub fn render_tile(
    element: &LayoutElement,
    params: &TileParams,
    ctx: &DataContext,
    _render_ctx: &RenderContext,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let tile_id = resolve_tile_id(params, ctx)?;
    let color = params
        .palette
        .as_deref()
        .map(parse_color)
        .unwrap_or(Rgba::INK_BLACK);
    let fallback = format!("[{}]", tile_id);

    let rect = &element.rect;

    let fill = params.repeat.is_some();
    let tw = rect.tw.unwrap_or(1);
    let th = rect.th.unwrap_or(1);

    let base_tx = rect.tx.resolve(ctx);
    let base_ty = rect.ty.resolve(ctx);

    let cols = if fill { tw } else { 1 };
    let rows = if fill { th } else { 1 };

    for row in 0..rows {
        for col in 0..cols {
            painter.draw_gb_tile(
                TilePos::new(base_tx + col, base_ty + row),
                tile_id,
                &fallback,
                color,
            );
        }
    }

    Ok(())
}

/// Render a tile with full pixel-level flip support.
///
/// Requires direct access to the framebuffer, tileset, and palette.
/// Use this when `flip_x` or `flip_y` is needed and the Painter backend
/// does not support flipping.
///
/// # Arguments
/// * `element` — The layout element.
/// * `params` — Deserialised [`TileParams`].
/// * `ctx` — Data context for tile id resolution.
/// * `tileset` — The tile data source.
/// * `palette` — Palette for mapping colour indices to RGBA.
/// * `fb` — The framebuffer to draw into.
pub fn render_tile_with_tiles(
    element: &LayoutElement,
    params: &TileParams,
    ctx: &DataContext,
    tileset: &crate::tile::TileSet,
    palette: &crate::palette::Palette,
    fb: &mut dotzuki_engine::render::FrameBuffer,
) -> Result<(), RenderError> {
    use crate::tile::TILE_PIXELS;

    let tile_id = resolve_tile_id(params, ctx)?;
    let tile = tileset.get(tile_id as usize);

    let mut tile_data = tile.clone();
    if params.flip_x {
        tile_data = tile_data.flip_x();
    }
    if params.flip_y {
        tile_data = tile_data.flip_y();
    }

    let rect = &element.rect;
    let fill = params.repeat.is_some();
    let tw = rect.tw.unwrap_or(1);
    let th = rect.th.unwrap_or(1);

    let base_tx = rect.tx.resolve(ctx);
    let base_ty = rect.ty.resolve(ctx);

    let cols = if fill { tw } else { 1 };
    let rows = if fill { th } else { 1 };

    for row in 0..rows {
        for col in 0..cols {
            let px = (base_tx + col) * TILE_PIXELS as u32;
            let py = (base_ty + row) * TILE_PIXELS as u32;

            for ty in 0..TILE_PIXELS {
                let rgba_row = tile_data.render_row(ty, palette);
                for tx in 0..TILE_PIXELS {
                    let rgba = rgba_row[tx];
                    if rgba != dotzuki_engine::render::Rgba::TRANSPARENT {
                        fb.set_pixel(px + tx as u32, py + ty as u32, rgba);
                    }
                }
            }
        }
    }

    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Resolve `tile_id` from params — may be a literal number, a string, or
/// a `{template}` variable that expands via [`DataContext::resolve`].
pub fn resolve_tile_id(params: &TileParams, ctx: &DataContext) -> Result<u8, RenderError> {
    match &params.tile_id {
        serde_json::Value::Number(n) => n
            .as_u64()
            .and_then(|v| u8::try_from(v).ok())
            .ok_or(RenderError::MissingVariable),
        serde_json::Value::String(s) => {
            let resolved = ctx.resolve(s);
            resolved
                .trim()
                .parse::<u8>()
                .map_err(|_| RenderError::MissingVariable)
        }
        _ => Err(RenderError::MissingVariable),
    }
}

pub use crate::layout_engine::elements::text::parse_color;

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_engine::types::{Coord, ElementParams, ElementRect};
    use dotzuki_engine::render::Rgba as EngineRgba;

    // ── Recording painter ────────────────────────────────────────────

    #[derive(Debug, Default)]
    struct RecordingPainter {
        tile_calls: Vec<(TilePos, u8, String, EngineRgba)>,
    }

    impl RecordingPainter {
        fn new() -> Self {
            Self::default()
        }
    }

    impl Painter for RecordingPainter {
        fn clear(&mut self, _color: EngineRgba) {}

        fn draw_text_box(&mut self, _rect: dotzuki_engine::render::TileRect, _color: EngineRgba) {}

        fn draw_text(&mut self, _pos: TilePos, _text: &str, _color: EngineRgba) {}

        fn draw_glyph(&mut self, _pos: TilePos, _glyph: char, _color: EngineRgba) {}

        fn draw_pixel_rect(&mut self, _px: u32, _py: u32, _pw: u32, _ph: u32, _color: EngineRgba) {}

        fn draw_gb_tile(&mut self, pos: TilePos, tile_id: u8, fallback: &str, color: EngineRgba) {
            self.tile_calls
                .push((pos, tile_id, fallback.to_string(), color));
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────

    fn make_element(tile_id: serde_json::Value, tx: u32, ty: u32) -> LayoutElement {
        LayoutElement {
            id: String::new(),
            element_type: "tile".to_string(),
            rect: ElementRect {
                tx: Coord::Literal(tx),
                ty: Coord::Literal(ty),
                tw: Some(1),
                th: Some(1),
            },
            visible: crate::layout_engine::types::Visibility::Static(true),
            z_index: 0,
            params: ElementParams::Tile(TileParams {
                tile_id,
                flip_x: false,
                flip_y: false,
                palette: None,
                repeat: None,
            }),
        }
    }

    fn make_theme() -> crate::layout_engine::types::Theme {
        Default::default()
    }

    // ── Tests: resolve_tile_id ───────────────────────────────────────

    #[test]
    fn resolve_from_json_number() {
        let params = TileParams {
            tile_id: serde_json::Value::Number(serde_json::Number::from(42)),
            flip_x: false,
            flip_y: false,
            palette: None,
            repeat: None,
        };
        assert_eq!(resolve_tile_id(&params, &DataContext::new()).unwrap(), 42);
    }

    #[test]
    fn resolve_from_json_string() {
        let params = TileParams {
            tile_id: serde_json::Value::String("99".to_string()),
            flip_x: false,
            flip_y: false,
            palette: None,
            repeat: None,
        };
        assert_eq!(resolve_tile_id(&params, &DataContext::new()).unwrap(), 99);
    }

    #[test]
    fn resolve_from_template() {
        let params = TileParams {
            tile_id: serde_json::Value::String("{t}".to_string()),
            flip_x: false,
            flip_y: false,
            palette: None,
            repeat: None,
        };
        let mut ctx = DataContext::new();
        ctx.set("t", 77i64);
        assert_eq!(resolve_tile_id(&params, &ctx).unwrap(), 77);
    }

    #[test]
    fn resolve_invalid_number_errors() {
        let params = TileParams {
            tile_id: serde_json::Value::Number(serde_json::Number::from(99999)),
            flip_x: false,
            flip_y: false,
            palette: None,
            repeat: None,
        };
        assert!(resolve_tile_id(&params, &DataContext::new()).is_err());
    }

    #[test]
    fn resolve_invalid_string_errors() {
        let params = TileParams {
            tile_id: serde_json::Value::String("abc".to_string()),
            flip_x: false,
            flip_y: false,
            palette: None,
            repeat: None,
        };
        assert!(resolve_tile_id(&params, &DataContext::new()).is_err());
    }

    #[test]
    fn resolve_missing_template_errors() {
        let params = TileParams {
            tile_id: serde_json::Value::String("{missing}".to_string()),
            flip_x: false,
            flip_y: false,
            palette: None,
            repeat: None,
        };
        let ctx = DataContext::new();
        assert!(resolve_tile_id(&params, &ctx).is_err());
    }

    // ── Tests: parse_color ───────────────────────────────────────────

    #[test]
    fn parse_black() {
        assert_eq!(parse_color("black"), Rgba::INK_BLACK);
    }

    #[test]
    fn parse_darkgray() {
        assert_eq!(parse_color("darkgray"), Rgba::INK_DARK_GRAY);
    }

    #[test]
    fn parse_unknown_returns_black() {
        assert_eq!(parse_color("green"), Rgba::INK_BLACK);
    }

    // ── Tests: render_tile ───────────────────────────────────────────

    #[test]
    fn render_single_tile() {
        let elem = make_element(
            serde_json::Value::Number(serde_json::Number::from(10)),
            3,
            5,
        );
        let params = match &elem.params {
            ElementParams::Tile(p) => p,
            _ => unreachable!(),
        };
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let tilesets: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let rc = RenderContext {
            screen: "test",
            theme: &theme,
            fonts: &fonts,
            tilesets: &tilesets,
            images: crate::layout_engine::types::empty_image_registry(),
        };
        let mut p = RecordingPainter::new();

        render_tile(&elem, params, &ctx, &rc, &mut p).unwrap();

        assert_eq!(p.tile_calls.len(), 1);
        assert_eq!(p.tile_calls[0].0, TilePos::new(3, 5));
        assert_eq!(p.tile_calls[0].1, 10);
    }

    #[test]
    fn render_repeat_fills_rect() {
        let mut elem = make_element(serde_json::Value::Number(serde_json::Number::from(5)), 0, 0);
        elem.rect.tw = Some(3);
        elem.rect.th = Some(2);
        if let ElementParams::Tile(ref mut tp) = elem.params {
            tp.repeat = Some(1);
        }
        let params = match &elem.params {
            ElementParams::Tile(p) => p,
            _ => unreachable!(),
        };
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let tilesets: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let rc = RenderContext {
            screen: "test",
            theme: &theme,
            fonts: &fonts,
            tilesets: &tilesets,
            images: crate::layout_engine::types::empty_image_registry(),
        };
        let mut p = RecordingPainter::new();

        render_tile(&elem, params, &ctx, &rc, &mut p).unwrap();

        assert_eq!(p.tile_calls.len(), 6); // 3×2
        for call in &p.tile_calls {
            assert_eq!(call.1, 5);
        }
        assert_eq!(p.tile_calls[0].0, TilePos::new(0, 0));
        assert_eq!(p.tile_calls[5].0, TilePos::new(2, 1));
    }

    #[test]
    fn render_repeat_none_draws_once() {
        let elem = make_element(serde_json::Value::Number(serde_json::Number::from(7)), 0, 0);
        let params = match &elem.params {
            ElementParams::Tile(p) => p,
            _ => unreachable!(),
        };
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let tilesets: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let rc = RenderContext {
            screen: "test",
            theme: &theme,
            fonts: &fonts,
            tilesets: &tilesets,
            images: crate::layout_engine::types::empty_image_registry(),
        };
        let mut p = RecordingPainter::new();

        render_tile(&elem, params, &ctx, &rc, &mut p).unwrap();

        assert_eq!(p.tile_calls.len(), 1);
    }

    #[test]
    fn render_from_template() {
        let elem = make_element(serde_json::Value::String("{id}".to_string()), 0, 0);
        let params = match &elem.params {
            ElementParams::Tile(p) => p,
            _ => unreachable!(),
        };
        let mut ctx = DataContext::new();
        ctx.set("id", 77i64);
        let theme = make_theme();
        let fonts: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let tilesets: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let rc = RenderContext {
            screen: "test",
            theme: &theme,
            fonts: &fonts,
            tilesets: &tilesets,
            images: crate::layout_engine::types::empty_image_registry(),
        };
        let mut p = RecordingPainter::new();

        render_tile(&elem, params, &ctx, &rc, &mut p).unwrap();

        assert_eq!(p.tile_calls[0].1, 77);
    }

    #[test]
    fn render_with_palette() {
        let mut elem = make_element(serde_json::Value::Number(serde_json::Number::from(1)), 0, 0);
        if let ElementParams::Tile(ref mut tp) = elem.params {
            tp.palette = Some("darkgray".to_string());
        }
        let params = match &elem.params {
            ElementParams::Tile(p) => p,
            _ => unreachable!(),
        };
        let ctx = DataContext::new();
        let theme = make_theme();
        let fonts: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let tilesets: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let rc = RenderContext {
            screen: "test",
            theme: &theme,
            fonts: &fonts,
            tilesets: &tilesets,
            images: crate::layout_engine::types::empty_image_registry(),
        };
        let mut p = RecordingPainter::new();

        render_tile(&elem, params, &ctx, &rc, &mut p).unwrap();

        assert_eq!(p.tile_calls[0].3, Rgba::INK_DARK_GRAY);
    }

    // ── Tests: render_tile_with_tiles ────────────────────────────────

    #[test]
    fn render_tile_with_tiles_no_flip() {
        let elem = make_element(serde_json::Value::Number(serde_json::Number::from(0)), 0, 0);
        let params = match &elem.params {
            ElementParams::Tile(p) => p,
            _ => unreachable!(),
        };
        let ctx = DataContext::new();

        let data = [0u8; 16]; // blank tile
        let tileset = crate::tile::TileSet::from_2bpp(&data);
        let palette = crate::palette::GRAYSCALE_PALETTE;
        let mut fb = dotzuki_engine::render::FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(160, 144),
            dotzuki_engine::render::Rgba::WHITE,
        );

        let result = render_tile_with_tiles(&elem, params, &ctx, &tileset, &palette, &mut fb);
        assert!(result.is_ok());
    }

    #[test]
    fn render_tile_with_tiles_flip_x() {
        let elem = make_element(serde_json::Value::Number(serde_json::Number::from(0)), 0, 0);
        let params = TileParams {
            tile_id: serde_json::Value::Number(serde_json::Number::from(0)),
            flip_x: true,
            flip_y: false,
            palette: None,
            repeat: None,
        };
        let ctx = DataContext::new();

        let data = [0xFFu8; 16]; // all color 3
        let tileset = crate::tile::TileSet::from_2bpp(&data);
        let palette = crate::palette::GRAYSCALE_PALETTE;
        let mut fb = dotzuki_engine::render::FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(160, 144),
            dotzuki_engine::render::Rgba::WHITE,
        );

        let result = render_tile_with_tiles(&elem, &params, &ctx, &tileset, &palette, &mut fb);
        assert!(result.is_ok());
    }

    #[test]
    fn render_tile_with_tiles_repeat() {
        let mut elem = make_element(serde_json::Value::Number(serde_json::Number::from(0)), 2, 3);
        elem.rect.tw = Some(2);
        elem.rect.th = Some(2);
        let params = TileParams {
            tile_id: serde_json::Value::Number(serde_json::Number::from(0)),
            flip_x: false,
            flip_y: false,
            palette: None,
            repeat: Some(1),
        };
        let ctx = DataContext::new();

        let data = [0xFFu8; 16];
        let tileset = crate::tile::TileSet::from_2bpp(&data);
        let palette = crate::palette::GRAYSCALE_PALETTE;
        let mut fb = dotzuki_engine::render::FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(160, 144),
            dotzuki_engine::render::Rgba::WHITE,
        );

        let result = render_tile_with_tiles(&elem, &params, &ctx, &tileset, &palette, &mut fb);
        assert!(result.is_ok());

        let tile_pixel_count = 8 * 8;
        let filled_pixels: usize = (0..tile_pixel_count * 4)
            .filter(|&i| {
                let _x = (i % 4) as u32;
                fb.data.get(i).map_or(false, |&b| b != 0)
            })
            .count();
        assert!(filled_pixels > 0, "should have drawn some pixels");
    }

    #[test]
    fn render_tile_with_tiles_out_of_bounds_tile_does_not_panic() {
        let elem = make_element(
            serde_json::Value::Number(serde_json::Number::from(255)),
            0,
            0,
        );
        let params = match &elem.params {
            ElementParams::Tile(p) => p,
            _ => unreachable!(),
        };
        let ctx = DataContext::new();

        let data = [0u8; 16]; // only 1 tile
        let tileset = crate::tile::TileSet::from_2bpp(&data);
        let palette = crate::palette::GRAYSCALE_PALETTE;
        let mut fb = dotzuki_engine::render::FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(160, 144),
            dotzuki_engine::render::Rgba::WHITE,
        );

        let result = render_tile_with_tiles(&elem, params, &ctx, &tileset, &palette, &mut fb);
        assert!(result.is_ok());
    }
}
