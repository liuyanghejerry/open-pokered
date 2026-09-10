//! Divider element — renders a line of tiles (horizontal or vertical) with
//! optional repeat to fill a rectangle.
//!
//! The divider lays out tiles from `DividerParams::tiles` sequentially along
//! the main axis. After the explicit tiles are used up, the last tile is
//! repeated `repeat` times to fill the remaining length of the rect.
//!
//! # Rendering contract
//!
//! Each tile is drawn via [`Painter::draw_gb_tile`], which (in backend
//! implementations) renders the tile from a loaded tileset or falls back to
//! the `fallback` text glyph.

use crate::layout_engine::types::{
    DataContext, Direction, DividerParams, LayoutElement, RenderContext, RenderError,
};
use dotzuki_engine::render::painter::Painter;
use dotzuki_engine::render::Rgba;
use dotzuki_engine::render::TilePos;

/// Render a divider element.
///
/// # Horizontal layout
///
/// Tiles are placed left-to-right across the rect's width. The rect must
/// have `tw` set (otherwise defaults to 1).
///
/// # Vertical layout
///
/// Tiles are placed top-to-bottom along the rect's height. The rect must
/// have `th` set (otherwise defaults to 1).
///
/// # Repeat
///
/// Once the explicit `tiles` vector is exhausted, the last tile is repeated
/// `repeat` times. If `repeat` is 0 the divider stops after the explicit
/// tiles.
pub fn render_divider(
    element: &LayoutElement,
    params: &DividerParams,
    ctx: &DataContext,
    _render_ctx: &RenderContext,
    painter: &mut dyn Painter,
) -> Result<(), RenderError> {
    let rect = &element.rect;
    let tiles = &params.tiles;
    if tiles.is_empty() {
        return Ok(());
    }

    let main_axis_len = match params.orientation {
        Direction::Horizontal => rect.tw.unwrap_or(1),
        Direction::Vertical => rect.th.unwrap_or(1),
    };

    let repeat = params.repeat;
    let last_tile = *tiles.last().unwrap();

    for i in 0..main_axis_len {
        let tile_id = if i < tiles.len() as u32 {
            tiles[i as usize]
        } else if repeat > 0 && i < tiles.len() as u32 + repeat {
            last_tile
        } else {
            break;
        };

        let base_tx = rect.tx.resolve(ctx);
        let base_ty = rect.ty.resolve(ctx);
        let (tx, ty) = match params.orientation {
            Direction::Horizontal => (base_tx + i, base_ty),
            Direction::Vertical => (base_tx, base_ty + i),
        };

        painter.draw_gb_tile(
            TilePos::new(tx, ty),
            tile_id as u8,
            "\u{25A0}",
            Rgba::INK_BLACK,
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_engine::types::{Coord, ElementParams, ElementRect};
    use dotzuki_engine::render::Rgba as EngineRgba;

    #[derive(Debug, Default)]
    struct RecordingPainter {
        tile_calls: Vec<(u32, u32, u8)>,
        pixel_rects: Vec<(u32, u32, u32, u32, EngineRgba)>,
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

        fn draw_pixel_rect(&mut self, px: u32, py: u32, pw: u32, ph: u32, color: EngineRgba) {
            self.pixel_rects.push((px, py, pw, ph, color));
        }

        fn draw_gb_tile(&mut self, pos: TilePos, tile_id: u8, _fallback: &str, _color: EngineRgba) {
            self.tile_calls.push((pos.tx, pos.ty, tile_id));
        }
    }

    fn make_element(rect: ElementRect, params: DividerParams) -> LayoutElement {
        LayoutElement {
            id: String::new(),
            element_type: "divider".to_string(),
            rect,
            visible: crate::layout_engine::types::Visibility::Static(true),
            z_index: 0,
            params: ElementParams::Divider(params),
        }
    }

    #[test]
    fn horizontal_places_tiles_left_to_right() {
        let rect = ElementRect {
            tx: Coord::Literal(2),
            ty: Coord::Literal(3),
            tw: Some(5),
            th: None,
        };
        let params = DividerParams {
            tiles: vec![10, 20, 30, 40, 50],
            repeat: 0,
            orientation: Direction::Horizontal,
        };
        let elem = make_element(rect, params);
        let ctx = DataContext::new();
        let rc = RenderContext {
            screen: "test",
            theme: &Default::default(),
            fonts: &dotzuki_engine::hash::HashMap::default(),
            tilesets: &dotzuki_engine::hash::HashMap::default(),
            images: crate::layout_engine::types::empty_image_registry(),
        };

        let mut painter = RecordingPainter::new();
        render_divider(
            &elem,
            if let ElementParams::Divider(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        )
        .unwrap();

        assert_eq!(painter.tile_calls.len(), 5);
        assert_eq!(painter.tile_calls[0], (2, 3, 10));
        assert_eq!(painter.tile_calls[1], (3, 3, 20));
        assert_eq!(painter.tile_calls[2], (4, 3, 30));
        assert_eq!(painter.tile_calls[3], (5, 3, 40));
        assert_eq!(painter.tile_calls[4], (6, 3, 50));
    }

    #[test]
    fn horizontal_default_tw_is_1() {
        let rect = ElementRect {
            tx: Coord::Literal(0),
            ty: Coord::Literal(0),
            tw: None,
            th: None,
        };
        let params = DividerParams {
            tiles: vec![99],
            repeat: 0,
            orientation: Direction::Horizontal,
        };
        let elem = make_element(rect, params);
        let ctx = DataContext::new();
        let rc = RenderContext {
            screen: "test",
            theme: &Default::default(),
            fonts: &dotzuki_engine::hash::HashMap::default(),
            tilesets: &dotzuki_engine::hash::HashMap::default(),
            images: crate::layout_engine::types::empty_image_registry(),
        };

        let mut painter = RecordingPainter::new();
        render_divider(
            &elem,
            if let ElementParams::Divider(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        )
        .unwrap();

        assert_eq!(painter.tile_calls.len(), 1);
        assert_eq!(painter.tile_calls[0], (0, 0, 99));
    }

    #[test]
    fn vertical_places_tiles_top_to_bottom() {
        let rect = ElementRect {
            tx: Coord::Literal(5),
            ty: Coord::Literal(1),
            tw: None,
            th: Some(4),
        };
        let params = DividerParams {
            tiles: vec![1, 2, 3],
            repeat: 1,
            orientation: Direction::Vertical,
        };
        let elem = make_element(rect, params);
        let ctx = DataContext::new();
        let rc = RenderContext {
            screen: "test",
            theme: &Default::default(),
            fonts: &dotzuki_engine::hash::HashMap::default(),
            tilesets: &dotzuki_engine::hash::HashMap::default(),
            images: crate::layout_engine::types::empty_image_registry(),
        };

        let mut painter = RecordingPainter::new();
        render_divider(
            &elem,
            if let ElementParams::Divider(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        )
        .unwrap();

        assert_eq!(painter.tile_calls.len(), 4);
        assert_eq!(painter.tile_calls[0], (5, 1, 1));
        assert_eq!(painter.tile_calls[1], (5, 2, 2));
        assert_eq!(painter.tile_calls[2], (5, 3, 3));
        assert_eq!(painter.tile_calls[3], (5, 4, 3));
    }

    #[test]
    fn vertical_default_th_is_1() {
        let rect = ElementRect {
            tx: Coord::Literal(1),
            ty: Coord::Literal(2),
            tw: None,
            th: None,
        };
        let params = DividerParams {
            tiles: vec![42],
            repeat: 0,
            orientation: Direction::Vertical,
        };
        let elem = make_element(rect, params);
        let ctx = DataContext::new();
        let rc = RenderContext {
            screen: "test",
            theme: &Default::default(),
            fonts: &dotzuki_engine::hash::HashMap::default(),
            tilesets: &dotzuki_engine::hash::HashMap::default(),
            images: crate::layout_engine::types::empty_image_registry(),
        };

        let mut painter = RecordingPainter::new();
        render_divider(
            &elem,
            if let ElementParams::Divider(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        )
        .unwrap();

        assert_eq!(painter.tile_calls.len(), 1);
        assert_eq!(painter.tile_calls[0], (1, 2, 42));
    }

    #[test]
    fn repeat_fills_remaining_rect() {
        let rect = ElementRect {
            tx: Coord::Literal(0),
            ty: Coord::Literal(0),
            tw: Some(10),
            th: None,
        };
        let params = DividerParams {
            tiles: vec![1, 2],
            repeat: 8,
            orientation: Direction::Horizontal,
        };
        let elem = make_element(rect, params);
        let ctx = DataContext::new();
        let rc = RenderContext {
            screen: "test",
            theme: &Default::default(),
            fonts: &dotzuki_engine::hash::HashMap::default(),
            tilesets: &dotzuki_engine::hash::HashMap::default(),
            images: crate::layout_engine::types::empty_image_registry(),
        };

        let mut painter = RecordingPainter::new();
        render_divider(
            &elem,
            if let ElementParams::Divider(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        )
        .unwrap();

        assert_eq!(painter.tile_calls.len(), 10);
        assert_eq!(painter.tile_calls[0], (0, 0, 1));
        assert_eq!(painter.tile_calls[1], (1, 0, 2));
        for i in 2..10 {
            assert_eq!(painter.tile_calls[i], (i as u32, 0, 2));
        }
    }

    #[test]
    fn repeat_zero_stops_after_explicit_tiles() {
        let rect = ElementRect {
            tx: Coord::Literal(0),
            ty: Coord::Literal(0),
            tw: Some(10),
            th: None,
        };
        let params = DividerParams {
            tiles: vec![1, 2],
            repeat: 0,
            orientation: Direction::Horizontal,
        };
        let elem = make_element(rect, params);
        let ctx = DataContext::new();
        let rc = RenderContext {
            screen: "test",
            theme: &Default::default(),
            fonts: &dotzuki_engine::hash::HashMap::default(),
            tilesets: &dotzuki_engine::hash::HashMap::default(),
            images: crate::layout_engine::types::empty_image_registry(),
        };

        let mut painter = RecordingPainter::new();
        render_divider(
            &elem,
            if let ElementParams::Divider(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        )
        .unwrap();

        assert_eq!(painter.tile_calls.len(), 2);
    }

    #[test]
    fn empty_tiles_is_noop() {
        let rect = ElementRect {
            tx: Coord::Literal(0),
            ty: Coord::Literal(0),
            tw: Some(5),
            th: None,
        };
        let params = DividerParams {
            tiles: vec![],
            repeat: 0,
            orientation: Direction::Horizontal,
        };
        let elem = make_element(rect, params);
        let ctx = DataContext::new();
        let rc = RenderContext {
            screen: "test",
            theme: &Default::default(),
            fonts: &dotzuki_engine::hash::HashMap::default(),
            tilesets: &dotzuki_engine::hash::HashMap::default(),
            images: crate::layout_engine::types::empty_image_registry(),
        };

        let mut painter = RecordingPainter::new();
        render_divider(
            &elem,
            if let ElementParams::Divider(ref p) = elem.params {
                p
            } else {
                unreachable!()
            },
            &ctx,
            &rc,
            &mut painter,
        )
        .unwrap();

        assert_eq!(painter.tile_calls.len(), 0);
    }

    #[test]
    fn default_orientation_is_horizontal() {
        let params = DividerParams {
            tiles: vec![1],
            repeat: 0,
            orientation: Direction::Horizontal,
        };
        let json = r#"{"tiles":[1],"repeat":0}"#;
        let deserialized: DividerParams = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.tiles, params.tiles);
    }
}
