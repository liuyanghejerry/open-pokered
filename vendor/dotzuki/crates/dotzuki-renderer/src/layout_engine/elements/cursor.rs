//! Cursor element — draws a selection glyph (▶) at a position computed from a
//! base + grid offset.
//!
//! The element's `rect.tx`/`rect.ty` is the base (origin) tile. The final
//! position is `base_tx + col*col_step` / `base_ty + row*row_step`, where
//! `col`/`row` are data bindings. This expresses:
//! - a 1-D list cursor (`row_step` set, `row = "{cursor}"`),
//! - a 2-D grid (battle FIGHT/PKMN/ITEM/RUN: `col_step`+`row_step`),
//! - an enum-offset selector (options: `col_step = 1`, `col = "{opt_index}"`).
//!
//! Multi-cursor screens (options rows, party ▶ + ◆) place several cursor
//! elements, each with its own `visible` condition and bindings.

use dotzuki_engine::render::painter::Painter;
use dotzuki_engine::render::TilePos;

use crate::layout_engine::elements::text::parse_color;
use crate::layout_engine::types::{CursorParams, DataContext, LayoutElement, Theme};

/// Render a cursor glyph into the framebuffer via `painter`.
pub fn render_cursor(
    element: &LayoutElement,
    params: &CursorParams,
    ctx: &DataContext,
    theme: &Theme,
    painter: &mut dyn Painter,
) {
    let base_tx = element.rect.tx.resolve(ctx);
    let base_ty = element.rect.ty.resolve(ctx);
    let col = params.col.resolve(ctx);
    let row = params.row.resolve(ctx);

    let tx = base_tx + col * params.col_step;
    let ty = base_ty + row * params.row_step;

    // Explicit colour wins; else the theme cursor ink (→ ink → INK_BLACK).
    let color = match params.color.as_deref() {
        Some(c) => parse_color(c),
        None => theme.cursor_ink(),
    };

    // Proportional screens place the glyph at pixel precision; the legacy tile
    // path is preserved byte-for-byte for pokered.
    if theme.proportional(painter.supports_proportional()) {
        let mut buf = [0u8; 4];
        painter.draw_text_px(
            tx * 8,
            ty * 8,
            params.glyph_char().encode_utf8(&mut buf),
            color,
        );
    } else {
        painter.draw_glyph(TilePos::new(tx, ty), params.glyph_char(), color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_engine::types::{Coord, ElementParams, ElementRect, Visibility};
    use dotzuki_engine::render::{Rgba, TileRect};
    use core::cell::RefCell;

    #[derive(Default)]
    struct Rec {
        glyphs: RefCell<Vec<(u32, u32, char)>>,
    }
    impl Painter for Rec {
        fn clear(&mut self, _c: Rgba) {}
        fn draw_text_box(&mut self, _r: TileRect, _c: Rgba) {}
        fn draw_text(&mut self, _p: TilePos, _t: &str, _c: Rgba) {}
        fn draw_glyph(&mut self, p: TilePos, g: char, _c: Rgba) {
            self.glyphs.borrow_mut().push((p.tx, p.ty, g));
        }
        fn draw_pixel_rect(&mut self, _x: u32, _y: u32, _w: u32, _h: u32, _c: Rgba) {}
        fn draw_gb_tile(&mut self, _p: TilePos, _t: u8, _f: &str, _c: Rgba) {}
    }

    fn elem(tx: u32, ty: u32, params: CursorParams) -> LayoutElement {
        LayoutElement {
            id: String::new(),
            element_type: "cursor".into(),
            rect: ElementRect {
                tx: Coord::Literal(tx),
                ty: Coord::Literal(ty),
                tw: Some(1),
                th: Some(1),
            },
            visible: Visibility::Static(true),
            z_index: 0,
            params: ElementParams::Cursor(params),
        }
    }

    fn cparams(col: Coord, row: Coord, col_step: u32, row_step: u32) -> CursorParams {
        CursorParams {
            glyph: None,
            color: None,
            col,
            row,
            col_step,
            row_step,
        }
    }

    #[test]
    fn grid_position_computed_from_col_row() {
        // base (1,12), 2x2 grid: col_step 9, row_step 2, at col=1,row=1 → (10,14)
        let mut ctx = DataContext::new();
        ctx.set("c", 1i64);
        ctx.set("r", 1i64);
        let e = elem(
            1,
            12,
            cparams(
                Coord::Template("{c}".into()),
                Coord::Template("{r}".into()),
                9,
                2,
            ),
        );
        let ElementParams::Cursor(ref p) = e.params else {
            unreachable!()
        };
        let mut painter = Rec::default();
        render_cursor(&e, p, &ctx, &Theme::default(), &mut painter);
        assert_eq!(painter.glyphs.borrow()[0], (10, 14, '\u{25B6}'));
    }

    #[test]
    fn defaults_to_base_and_triangle_glyph() {
        let e = elem(3, 5, cparams(Coord::Literal(0), Coord::Literal(0), 0, 0));
        let ElementParams::Cursor(ref p) = e.params else {
            unreachable!()
        };
        let mut painter = Rec::default();
        render_cursor(&e, p, &DataContext::new(), &Theme::default(), &mut painter);
        assert_eq!(painter.glyphs.borrow()[0], (3, 5, '\u{25B6}'));
    }
}
