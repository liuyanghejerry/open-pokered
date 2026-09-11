//! Regression test: text-box border pixels OUTSIDE the border line must stay
//! transparent (leave the underlying framebuffer untouched) instead of being
//! painted with the box background color.

use dotzuki_engine::render_config::RenderConfig;
use pokered_renderer::embedded_font::{box_tiles, draw_box_tile, fill_tile};
use pokered_renderer::{FrameBuffer, Rgba};
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::{Painter, TileRect};

// Stand-in for the overworld map. Since PR #160 pokered's FrameBuffer is an
// indexed 2bpp buffer with an RGBA facade: writes quantize to the 4 grayscale
// GB shades, so the background must be a palette-exact shade to round-trip.
const BG: Rgba = Rgba::rgb(85, 85, 85);

fn render_box() -> FrameBuffer {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), BG);
    let mut painter = FrameBufferPainter::new(&mut fb);
    painter.draw_text_box(TileRect::new(2, 2, 10, 6), Rgba::BLACK);
    fb
}

fn render_box_reference(rect: TileRect) -> FrameBuffer {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), BG);
    let t = 8;
    let bx = rect.tx * t;
    let by = rect.ty * t;
    let inner_w = rect.tw - 2;
    let inner_h = rect.th - 2;
    let right_x = bx + (rect.tw - 1) * t;
    let bot_y = by + (rect.th - 1) * t;

    draw_box_tile(
        &box_tiles::TOP_LEFT,
        &box_tiles::outside::TOP_LEFT,
        bx,
        by,
        Rgba::BLACK,
        Rgba::WHITE,
        &mut fb,
    );
    for col in 0..inner_w {
        draw_box_tile(
            &box_tiles::HORIZONTAL,
            &box_tiles::outside::HORIZONTAL,
            bx + (1 + col) * t,
            by,
            Rgba::BLACK,
            Rgba::WHITE,
            &mut fb,
        );
    }
    draw_box_tile(
        &box_tiles::TOP_RIGHT,
        &box_tiles::outside::TOP_RIGHT,
        right_x,
        by,
        Rgba::BLACK,
        Rgba::WHITE,
        &mut fb,
    );
    for row in 0..inner_h {
        let y = by + (1 + row) * t;
        draw_box_tile(
            &box_tiles::VERTICAL_LEFT,
            &box_tiles::outside::VERTICAL_LEFT,
            bx,
            y,
            Rgba::BLACK,
            Rgba::WHITE,
            &mut fb,
        );
        for col in 0..inner_w {
            fill_tile(bx + (1 + col) * t, y, Rgba::WHITE, &mut fb);
        }
        draw_box_tile(
            &box_tiles::VERTICAL_RIGHT,
            &box_tiles::outside::VERTICAL_RIGHT,
            right_x,
            y,
            Rgba::BLACK,
            Rgba::WHITE,
            &mut fb,
        );
    }
    draw_box_tile(
        &box_tiles::BOTTOM_LEFT,
        &box_tiles::outside::BOTTOM_LEFT,
        bx,
        bot_y,
        Rgba::BLACK,
        Rgba::WHITE,
        &mut fb,
    );
    for col in 0..inner_w {
        draw_box_tile(
            &box_tiles::HORIZONTAL_BOTTOM,
            &box_tiles::outside::HORIZONTAL_BOTTOM,
            bx + (1 + col) * t,
            bot_y,
            Rgba::BLACK,
            Rgba::WHITE,
            &mut fb,
        );
    }
    draw_box_tile(
        &box_tiles::BOTTOM_RIGHT,
        &box_tiles::outside::BOTTOM_RIGHT,
        right_x,
        bot_y,
        Rgba::BLACK,
        Rgba::WHITE,
        &mut fb,
    );
    fb
}

#[test]
fn border_outside_is_transparent() {
    let fb = render_box();
    // Box occupies tiles (2,2)..(11,7) => pixels x 16..=95, y 16..=63.
    // Top-left corner tile starts at (16,16); pixel (16,16) is outside the
    // rounded corner arc and must keep the background color.
    assert_eq!(fb.get_pixel(16, 16), Some(BG), "corner outside arc");
    // One pixel above the top edge line (line is at rows 1-2 of the tile).
    assert_eq!(fb.get_pixel(24, 16), Some(BG), "above top edge");
    // One pixel left of the left edge line (line is at cols 1-2).
    assert_eq!(fb.get_pixel(16, 32), Some(BG), "left of left edge");
    // Interior must still be opaque white.
    assert_eq!(fb.get_pixel(24, 24), Some(Rgba::WHITE), "interior");
    // The border stroke itself is still ink.
    assert_eq!(fb.get_pixel(24, 17), Some(Rgba::BLACK), "top edge stroke");
}

#[test]
fn batched_text_box_matches_tile_reference_pixel_for_pixel() {
    for rect in [
        TileRect::new(0, 12, 20, 6),
        TileRect::new(2, 2, 10, 6),
        TileRect::new(4, 10, 16, 7),
        TileRect::new(18, 16, 2, 2),
    ] {
        let mut actual = FrameBuffer::new(RenderConfig::new(160, 144), BG);
        FrameBufferPainter::new(&mut actual).draw_text_box(rect, Rgba::BLACK);
        let expected = render_box_reference(rect);
        for y in 0..144 {
            for x in 0..160 {
                assert_eq!(
                    actual.get_pixel(x, y),
                    expected.get_pixel(x, y),
                    "text box mismatch for {rect:?} at ({x}, {y})",
                );
            }
        }
    }
}
