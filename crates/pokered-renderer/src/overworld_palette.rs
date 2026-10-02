//! Original GB overworld OBP0; color zero remains transparent in OAM.
use crate::palette::Palette;
use crate::Rgba;

/// GBPalNormal writes $D0, while ordinary facing OAM selects palette zero
/// (home/palettes.asm:20-26; data/sprites/facings.asm:47-59).
pub fn normal_sprite_palette() -> Palette {
    Palette::new(&[Rgba::TRANSPARENT, Rgba::WHITE, Rgba::rgb(0xaa, 0xaa, 0xaa), Rgba::BLACK])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FrameBuffer, tile::Tile};
    use dotzuki_engine::render_config::RenderConfig;

    #[test]
    fn sprite_highlights_are_opaque_white_over_the_background() {
        // 2bpp indices 0/1/2/3 repeated. OAM color 0 must leave the map
        // visible, while color 1 is WHITE and must cover that same map.
        let mut bytes = [0; 16];
        bytes[0] = 0x55;
        bytes[1] = 0x33;
        let tile = Tile::from_2bpp(&bytes);
        let background = Rgba::rgb(0x55, 0x55, 0x55);
        let mut fb = FrameBuffer::new(RenderConfig::new(8, 8), background);
        fb.blit_gb_tile(0, 0, &tile, &normal_sprite_palette(), true, false, false);
        assert_eq!(fb.get_pixel(0, 0), Some(background));
        assert_eq!(fb.get_pixel(1, 0), Some(Rgba::WHITE));
        assert_eq!(fb.get_pixel(2, 0), Some(Rgba::rgb(0xaa, 0xaa, 0xaa)));
        assert_eq!(fb.get_pixel(3, 0), Some(Rgba::BLACK));
    }
}
