//! Regression test: text-box border pixels OUTSIDE the border line must stay
//! transparent (leave the underlying framebuffer untouched) instead of being
//! painted with the box background color.

use dotzuki_engine::render_config::RenderConfig;
use pokered_renderer::{FrameBuffer, Rgba};
use pokered_ui::backends::FrameBufferPainter;
use pokered_ui::{Painter, TileRect};

const BG: Rgba = Rgba::rgb(0x20, 0x60, 0x20); // stand-in for the overworld map

fn render_box() -> FrameBuffer {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), BG);
    let mut painter = FrameBufferPainter::new(&mut fb);
    painter.draw_text_box(TileRect::new(2, 2, 10, 6), Rgba::BLACK);
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
