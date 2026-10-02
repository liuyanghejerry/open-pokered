//! Original Game Boy text boxes write opaque font tiles, including the
//! white corners around the border stroke.

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
fn original_box_tiles_are_opaque_and_leave_the_next_tile_untouched() {
    let fb = render_box();
    assert_eq!(
        fb.get_pixel(16, 16),
        Some(Rgba::WHITE),
        "corner tile background"
    );
    assert_eq!(
        fb.get_pixel(24, 16),
        Some(Rgba::WHITE),
        "top tile background"
    );
    assert_eq!(
        fb.get_pixel(16, 32),
        Some(Rgba::WHITE),
        "side tile background"
    );
    assert_eq!(fb.get_pixel(24, 24), Some(Rgba::WHITE), "interior");
    assert_eq!(
        fb.get_pixel(24, 18),
        Some(Rgba::BLACK),
        "original $7A row 2 stroke"
    );
    assert_eq!(fb.get_pixel(15, 16), Some(BG), "adjacent map tile");
}

#[test]
fn original_text_box_matches_tile_reference_pixel_for_pixel() {
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

#[test]
fn latin_text_measurement_matches_the_glyphs_on_the_tile_grid() {
    use pokered_ui::TilePos;
    let mut whole = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut tiles = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut painter = FrameBufferPainter::new(&mut whole);
    assert_eq!(painter.measure_text_px("Ab▷19"), 5 * 8);
    painter.draw_text(TilePos::new(2, 3), "Ab▷19", Rgba::BLACK);
    for (i, ch) in "Ab▷19".chars().enumerate() {
        FrameBufferPainter::new(&mut tiles).draw_glyph(
            TilePos::new(2 + i as u32, 3),
            ch,
            Rgba::BLACK,
        );
    }
    for y in 0..144 {
        for x in 0..160 {
            assert_eq!(whole.get_pixel(x, y), tiles.get_pixel(x, y));
        }
    }
}
