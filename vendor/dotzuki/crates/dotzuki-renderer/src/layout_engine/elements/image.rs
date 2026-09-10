//! Image element — blits a full-colour RGBA image into the element's rect.
//!
//! The `source` field in [`ImageParams`] is a template string (e.g.
//! `"{portrait}"`, or a literal key) resolved via [`DataContext::resolve`]. The
//! resolved key is looked up in [`RenderContext::images`] (an [`ImageRegistry`]
//! the consumer populates). When found, the image is nearest-neighbour scaled to
//! fit the rect's pixel box (aspect preserved, centred), honouring `flip_x` /
//! `flip_y`, with transparent pixels skipped.
//!
//! When the key is empty/unknown (e.g. the registry has no entry yet), a striped
//! placeholder rectangle is drawn instead — useful as an "image goes here" cue in
//! the layout editor. (`palette` is a GB-tileset concept and is ignored here.)

use crate::layout_engine::types::{
    DataContext, ImageParams, LayoutElement, RenderContext, RenderError,
};
use dotzuki_engine::render::painter::Painter;
use dotzuki_engine::render::Rgba;
use dotzuki_engine::render::TilePos;

/// Render an image element.
///
/// # Sprite data lookup
///
/// The `source` template is resolved via [`DataContext::resolve`], producing
/// a lookup key. This key is used to find a matching tileset in
/// `render_ctx.tilesets`. When found, the tileset's tiles are drawn row by
/// row at the element rect position, applying `flip_x`, `flip_y`, and
/// `palette` as specified in the params.
///
/// # Placeholder (no sprite data)
///
/// If the resolved key does not match any tileset, a striped placeholder
/// rectangle is drawn. The stripes alternate between `LightGray` and
/// `DarkGray` in a checkerboard pattern, visually indicating "image not found".
pub fn render_image(
    element: &LayoutElement,
    params: &ImageParams,
    ctx: &DataContext,
    render_ctx: &RenderContext,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let rect = &element.rect;
    let tw = rect.tw.unwrap_or(1);
    let th = rect.th.unwrap_or(1);
    let source_key = ctx.resolve(&params.source);
    let tx = rect.tx.resolve(ctx);
    let ty = rect.ty.resolve(ctx);

    if source_key.is_empty() || source_key == "?" {
        draw_placeholder(tx, ty, tw, th, painter);
        return Ok(());
    }

    // Look the resolved key up in the full-colour image registry. (`palette` is a
    // GB-tileset concept and does not apply to RGBA images, so it is ignored.)
    match render_ctx.images.get(&source_key) {
        Some(img) if !img.is_empty() => {
            // Fit the image inside the rect's pixel box, preserving aspect, centred.
            let (box_px, box_py) = TilePos::new(tx, ty).to_pixels();
            let box_w = tw * 8;
            let box_h = th * 8;
            let scale = (box_w as f32 / img.width as f32).min(box_h as f32 / img.height as f32);
            // f32::round is std-only; libm's roundf on no_std targets.
            #[cfg(target_os = "none")]
            fn round_u(x: f32) -> u32 {
                libm::roundf(x) as u32
            }
            #[cfg(not(target_os = "none"))]
            fn round_u(x: f32) -> u32 {
                x.round() as u32
            }
            let dst_w = round_u(img.width as f32 * scale).max(1);
            let dst_h = round_u(img.height as f32 * scale).max(1);
            let ox = box_px + (box_w.saturating_sub(dst_w)) / 2;
            let oy = box_py + (box_h.saturating_sub(dst_h)) / 2;
            painter.draw_rgba(
                ox,
                oy,
                dst_w,
                dst_h,
                &img.pixels,
                img.width,
                img.height,
                params.flip_x,
                params.flip_y,
            );
        }
        // Unknown key (or empty image) → striped "image not found" placeholder.
        _ => draw_placeholder(tx, ty, tw, th, painter),
    }
    Ok(())
}

fn draw_placeholder(tx: u32, ty: u32, tw: u32, th: u32, painter: &mut dyn Painter) {
    for row in 0..th {
        for col in 0..tw {
            let color = if (col + row) % 2 == 0 {
                Rgba::INK_LIGHT_GRAY
            } else {
                Rgba::INK_DARK_GRAY
            };
            let (px, py) = TilePos::new(tx + col, ty + row).to_pixels();
            painter.draw_pixel_rect(px, py, 8, 8, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_engine::types::DataValue;
    use crate::layout_engine::types::{Coord, ElementParams, ElementRect};
    use dotzuki_engine::render::Rgba as EngineRgba;

    #[derive(Debug, Default)]
    struct RecordingPainter {
        pixel_rects: Vec<(u32, u32, u32, u32, EngineRgba)>,
        tile_calls: Vec<(u32, u32, u8)>,
    }

    impl Painter for RecordingPainter {
        fn clear(&mut self, _color: EngineRgba) {}

        fn draw_text_box(&mut self, _rect: dotzuki_engine::render::TileRect, _color: EngineRgba) {}

        fn draw_text(&mut self, _pos: TilePos, _text: &str, _color: EngineRgba) {}

        fn draw_glyph(&mut self, _pos: TilePos, _glyph: char, _color: EngineRgba) {}

        fn draw_pixel_rect(&mut self, px: u32, py: u32, pw: u32, ph: u32, color: EngineRgba) {
            self.pixel_rects.push((px, py, pw, ph, color));
        }

        fn draw_gb_tile(&mut self, pos: TilePos, tile_id: u8, _fallback: &str, _color: EngineRgba) {
            self.tile_calls.push((pos.tx, pos.ty, tile_id));
        }
    }

    fn make_element(rect: ElementRect, params: ImageParams) -> LayoutElement {
        LayoutElement {
            id: String::new(),
            element_type: "image".to_string(),
            rect,
            visible: crate::layout_engine::types::Visibility::Static(true),
            z_index: 0,
            params: ElementParams::Image(params),
        }
    }

    fn make_render_ctx() -> (
        RenderContext<'static>,
        dotzuki_engine::hash::HashMap<String, ()>,
        dotzuki_engine::hash::HashMap<String, ()>,
    ) {
        let fonts: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let tilesets: dotzuki_engine::hash::HashMap<String, ()> = dotzuki_engine::hash::HashMap::default();
        let theme = crate::layout_engine::types::Theme::default();
        // SAFETY: We leak the box to get a 'static reference. In tests this is fine.
        let fonts_ref: &'static dotzuki_engine::hash::HashMap<String, ()> =
            Box::leak(Box::new(fonts.clone()));
        let tilesets_ref: &'static dotzuki_engine::hash::HashMap<String, ()> =
            Box::leak(Box::new(tilesets.clone()));
        let theme_ref: &'static crate::layout_engine::types::Theme = Box::leak(Box::new(theme));
        (
            RenderContext {
                screen: "test",
                theme: theme_ref,
                fonts: fonts_ref,
                tilesets: tilesets_ref,
                images: crate::layout_engine::types::empty_image_registry(),
            },
            fonts,
            tilesets,
        )
    }

    #[test]
    fn resolves_source_from_datacontext() {
        let rect = ElementRect {
            tx: Coord::Literal(0),
            ty: Coord::Literal(0),
            tw: Some(1),
            th: Some(1),
        };
        let params = ImageParams {
            source: "{sprite}".to_string(),
            flip_x: false,
            flip_y: false,
            palette: None,
        };
        let elem = make_element(rect, params);
        let mut ctx = DataContext::new();
        ctx.set("sprite", "leafkit_front");
        let (rc, _fonts, _tilesets) = make_render_ctx();

        let mut painter = RecordingPainter::default();
        render_image(
            &elem,
            if let ElementParams::Image(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        )
        .unwrap();

        assert!(!painter.pixel_rects.is_empty());
    }

    #[test]
    fn unknown_source_draws_placeholder() {
        let rect = ElementRect {
            tx: Coord::Literal(0),
            ty: Coord::Literal(0),
            tw: Some(1),
            th: Some(1),
        };
        let params = ImageParams {
            source: "{missing}".to_string(),
            flip_x: false,
            flip_y: false,
            palette: None,
        };
        let elem = make_element(rect, params);
        let ctx = DataContext::new();
        let (rc, _fonts, _tilesets) = make_render_ctx();

        let mut painter = RecordingPainter::default();
        render_image(
            &elem,
            if let ElementParams::Image(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        )
        .unwrap();

        assert!(!painter.pixel_rects.is_empty());
    }

    #[test]
    fn placeholder_spans_full_rect() {
        let rect = ElementRect {
            tx: Coord::Literal(2),
            ty: Coord::Literal(1),
            tw: Some(3),
            th: Some(2),
        };
        let params = ImageParams {
            source: "{sprite}".to_string(),
            flip_x: false,
            flip_y: false,
            palette: None,
        };
        let elem = make_element(rect, params);
        let mut ctx = DataContext::new();
        ctx.set("sprite", DataValue::Str("unknown".to_string()));
        let (rc, _fonts, _tilesets) = make_render_ctx();

        let mut painter = RecordingPainter::default();
        render_image(
            &elem,
            if let ElementParams::Image(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        )
        .unwrap();

        assert_eq!(painter.pixel_rects.len(), 6);
        for (px, py, pw, ph, _) in &painter.pixel_rects {
            assert_eq!(*pw, 8);
            assert_eq!(*ph, 8);
            assert!(*px >= 16 && *px < 40);
            assert!(*py >= 8 && *py < 24);
        }
    }

    #[test]
    fn placeholder_alternates_colors() {
        let rect = ElementRect {
            tx: Coord::Literal(0),
            ty: Coord::Literal(0),
            tw: Some(2),
            th: Some(2),
        };
        let params = ImageParams {
            source: "{sprite}".to_string(),
            flip_x: false,
            flip_y: false,
            palette: None,
        };
        let elem = make_element(rect, params);
        let mut ctx = DataContext::new();
        ctx.set("sprite", "missing");
        let (rc, _fonts, _tilesets) = make_render_ctx();

        let mut painter = RecordingPainter::default();
        render_image(
            &elem,
            if let ElementParams::Image(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        )
        .unwrap();

        assert_eq!(painter.pixel_rects.len(), 4);
        let colors: Vec<EngineRgba> = painter.pixel_rects.iter().map(|r| r.4).collect();
        assert_eq!(colors[0], EngineRgba::INK_LIGHT_GRAY);
        assert_eq!(colors[1], EngineRgba::INK_DARK_GRAY);
        assert_eq!(colors[2], EngineRgba::INK_DARK_GRAY);
        assert_eq!(colors[3], EngineRgba::INK_LIGHT_GRAY);
    }

    #[test]
    fn empty_resolved_source_draws_placeholder() {
        let rect = ElementRect {
            tx: Coord::Literal(0),
            ty: Coord::Literal(0),
            tw: Some(2),
            th: Some(2),
        };
        let params = ImageParams {
            source: "{empty}".to_string(),
            flip_x: false,
            flip_y: false,
            palette: None,
        };
        let elem = make_element(rect, params);
        let mut ctx = DataContext::new();
        ctx.set("empty", "");
        let (rc, _fonts, _tilesets) = make_render_ctx();

        let mut painter = RecordingPainter::default();
        render_image(
            &elem,
            if let ElementParams::Image(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        )
        .unwrap();

        assert!(!painter.pixel_rects.is_empty());
    }

    #[test]
    fn flip_params_are_accepted() {
        let rect = ElementRect {
            tx: Coord::Literal(0),
            ty: Coord::Literal(0),
            tw: Some(1),
            th: Some(1),
        };
        let params = ImageParams {
            source: "{sprite}".to_string(),
            flip_x: true,
            flip_y: true,
            palette: Some("pal1".to_string()),
        };
        let elem = make_element(rect, params);
        let mut ctx = DataContext::new();
        ctx.set("sprite", "leafkit");
        let (rc, _fonts, _tilesets) = make_render_ctx();

        let mut painter = RecordingPainter::default();
        let result = render_image(
            &elem,
            if let ElementParams::Image(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn default_tw_th_is_1() {
        let rect = ElementRect {
            tx: Coord::Literal(5),
            ty: Coord::Literal(3),
            tw: None,
            th: None,
        };
        let params = ImageParams {
            source: "{sprite}".to_string(),
            flip_x: false,
            flip_y: false,
            palette: None,
        };
        let elem = make_element(rect, params);
        let mut ctx = DataContext::new();
        ctx.set("sprite", "leafkit");
        let (rc, _fonts, _tilesets) = make_render_ctx();

        let mut painter = RecordingPainter::default();
        render_image(
            &elem,
            if let ElementParams::Image(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        )
        .unwrap();

        assert_eq!(painter.pixel_rects.len(), 1);
        assert_eq!(painter.pixel_rects[0].0, 40);
        assert_eq!(painter.pixel_rects[0].1, 24);
        assert_eq!(painter.pixel_rects[0].2, 8);
        assert_eq!(painter.pixel_rects[0].3, 8);
    }

    #[test]
    fn registered_image_is_blitted_not_placeholder() {
        use crate::layout_engine::types::{ImageData, ImageRegistry, Theme};
        let rect = ElementRect {
            tx: Coord::Literal(0),
            ty: Coord::Literal(0),
            tw: Some(1),
            th: Some(1),
        };
        let params = ImageParams {
            source: "{sprite}".to_string(),
            flip_x: false,
            flip_y: false,
            palette: None,
        };
        let elem = make_element(rect, params);
        let mut ctx = DataContext::new();
        ctx.set("sprite", "hero");

        // A 1×1 opaque red image registered under the resolved key "hero".
        let red = EngineRgba::new(255, 0, 0, 255);
        let mut images = ImageRegistry::new();
        images.insert("hero".to_string(), ImageData::new(1, 1, vec![red]));

        let theme = Theme::default();
        let fonts = dotzuki_engine::hash::HashMap::<String, ()>::new();
        let tilesets = dotzuki_engine::hash::HashMap::<String, ()>::new();
        let rc = RenderContext {
            screen: "t",
            theme: &theme,
            fonts: &fonts,
            tilesets: &tilesets,
            images: &images,
        };

        let mut painter = RecordingPainter::default();
        render_image(
            &elem,
            if let ElementParams::Image(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        )
        .unwrap();

        // The 1×1 image scales to fill the 8×8-px cell → 64 red pixels via the
        // default per-pixel draw_rgba; none are the placeholder gray.
        assert_eq!(painter.pixel_rects.len(), 64, "8×8 box filled");
        assert!(
            painter.pixel_rects.iter().all(|r| r.4 == red),
            "every blitted pixel is the image colour, not a placeholder"
        );
        assert!(painter.pixel_rects.iter().all(|r| r.2 == 1 && r.3 == 1));
    }

    #[test]
    fn unknown_key_with_nonempty_registry_still_placeholders() {
        use crate::layout_engine::types::{ImageData, ImageRegistry, Theme};
        let rect = ElementRect {
            tx: Coord::Literal(0),
            ty: Coord::Literal(0),
            tw: Some(2),
            th: Some(2),
        };
        let params = ImageParams {
            source: "{sprite}".to_string(),
            flip_x: false,
            flip_y: false,
            palette: None,
        };
        let elem = make_element(rect, params);
        let mut ctx = DataContext::new();
        ctx.set("sprite", "missing");

        let mut images = ImageRegistry::new();
        images.insert(
            "other".to_string(),
            ImageData::new(1, 1, vec![EngineRgba::new(0, 255, 0, 255)]),
        );
        let theme = Theme::default();
        let fonts = dotzuki_engine::hash::HashMap::<String, ()>::new();
        let tilesets = dotzuki_engine::hash::HashMap::<String, ()>::new();
        let rc = RenderContext {
            screen: "t",
            theme: &theme,
            fonts: &fonts,
            tilesets: &tilesets,
            images: &images,
        };

        let mut painter = RecordingPainter::default();
        render_image(
            &elem,
            if let ElementParams::Image(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        )
        .unwrap();

        // 2×2 cells of placeholder = 4 draw_pixel_rect calls of 8×8.
        assert_eq!(painter.pixel_rects.len(), 4);
        assert!(painter.pixel_rects.iter().all(|r| r.2 == 8 && r.3 == 8));
    }
}
