//! Primitive shape elements — `bracket`, `pixel_rect`.
//!
//! These compose from the painter's `draw_pixel_rect` and reproduce the
//! pokered-ui `Frame` primitives (bracket box, raw rect) so layouts can be
//! expressed declaratively in `.gui`. Game-specific primitives (e.g. the
//! Gen-I HP bar) are NOT built in — games register them as `custom:*`
//! elements via the `ElementRegistry`.

use dotzuki_engine::render::painter::Painter;
use dotzuki_engine::render::Rgba;

use crate::layout_engine::elements::text::parse_color;
use crate::layout_engine::types::{BracketParams, DataContext, LayoutElement, PixelRectParams};

const TILE: u32 = 8;

/// Partial box border (1px edges) at the element rect. Matches
/// `Frame::bracket_box` (interior-corner pixel offsets).
pub fn render_bracket(
    element: &LayoutElement,
    params: &BracketParams,
    ctx: &DataContext,
    painter: &mut dyn Painter,
) {
    let tx = element.rect.tx.resolve(ctx);
    let ty = element.rect.ty.resolve(ctx);
    let tw = element.rect.tw.unwrap_or(1);
    let th = element.rect.th.unwrap_or(1);
    let color = params
        .color
        .as_deref()
        .map(parse_color)
        .unwrap_or(Rgba::INK_BLACK);

    let left_px = tx * TILE;
    let right_px = (tx + tw - 1) * TILE + 6;
    let top_px = ty * TILE;
    let bot_px = (ty + th - 1) * TILE + 6;

    if params.right {
        painter.draw_pixel_rect(right_px, top_px, 1, bot_px - top_px + 1, color);
    }
    if params.left {
        painter.draw_pixel_rect(left_px, top_px, 1, bot_px - top_px + 1, color);
    }
    if params.top {
        painter.draw_pixel_rect(left_px, top_px, right_px - left_px + 1, 1, color);
    }
    if params.bottom {
        painter.draw_pixel_rect(left_px, bot_px, right_px - left_px + 1, 1, color);
        if params.with_arrow && left_px >= 3 {
            let arrow_left = left_px - 3;
            painter.draw_pixel_rect(arrow_left, bot_px, 4, 1, color);
            painter.draw_pixel_rect(arrow_left, bot_px - 1, 1, 1, color);
            painter.draw_pixel_rect(arrow_left, bot_px + 1, 1, 1, color);
        }
    }
}

/// Raw filled rectangle in pixel coordinates.
pub fn render_pixel_rect(params: &PixelRectParams, painter: &mut dyn Painter) {
    let color = params
        .color
        .as_deref()
        .map(parse_color)
        .unwrap_or(Rgba::INK_BLACK);
    painter.draw_pixel_rect(params.px, params.py, params.pw, params.ph, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_engine::types::ElementParams;
    use crate::layout_engine::types::{Coord, ElementRect, LayoutElement, Visibility};
    use dotzuki_engine::render::{TilePos, TileRect};
    use core::cell::RefCell;

    #[derive(Default)]
    struct PxRec {
        rects: RefCell<Vec<(u32, u32, u32, u32, Rgba)>>,
    }
    impl Painter for PxRec {
        fn clear(&mut self, _c: Rgba) {}
        fn draw_text_box(&mut self, _r: TileRect, _c: Rgba) {}
        fn draw_text(&mut self, _p: TilePos, _t: &str, _c: Rgba) {}
        fn draw_glyph(&mut self, _p: TilePos, _g: char, _c: Rgba) {}
        fn draw_pixel_rect(&mut self, x: u32, y: u32, w: u32, h: u32, c: Rgba) {
            self.rects.borrow_mut().push((x, y, w, h, c));
        }
        fn draw_gb_tile(&mut self, _p: TilePos, _t: u8, _f: &str, _c: Rgba) {}
    }

    #[test]
    fn bracket_draws_requested_sides_only() {
        let e = LayoutElement {
            id: String::new(),
            element_type: "bracket".into(),
            rect: ElementRect {
                tx: Coord::Literal(2),
                ty: Coord::Literal(3),
                tw: Some(4),
                th: Some(2),
            },
            visible: Visibility::Static(true),
            z_index: 0,
            params: ElementParams::Bracket(BracketParams {
                color: None,
                left: true,
                right: false,
                top: false,
                bottom: true,
                with_arrow: false,
            }),
        };
        let ElementParams::Bracket(ref p) = e.params else {
            unreachable!()
        };
        let mut painter = PxRec::default();
        render_bracket(&e, p, &DataContext::new(), &mut painter);
        let rects = painter.rects.borrow();
        // one vertical (left) + one horizontal (bottom) line
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].2, 1, "left edge is 1px wide");
        assert_eq!(rects[1].3, 1, "bottom edge is 1px tall");
    }

    #[test]
    fn pixel_rect_draws_raw_rect() {
        let params = PixelRectParams {
            color: Some("white".into()),
            px: 10,
            py: 20,
            pw: 30,
            ph: 2,
        };
        let mut painter = PxRec::default();
        render_pixel_rect(&params, &mut painter);
        assert_eq!(
            *painter.rects.borrow(),
            vec![(10, 20, 30, 2, Rgba::INK_WHITE)]
        );
    }
}
