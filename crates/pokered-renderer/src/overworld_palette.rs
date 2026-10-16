//! Original GB overworld OBP0; color zero remains transparent in OAM.
use crate::palette::Palette;
use crate::Rgba;

/// GBPalNormal writes $D0, while ordinary facing OAM selects palette zero
/// (home/palettes.asm:20-26; data/sprites/facings.asm:47-59).
pub fn normal_sprite_palette() -> Palette {
    Palette::new(&[
        Rgba::TRANSPARENT,
        Rgba::WHITE,
        Rgba::rgb(0xaa, 0xaa, 0xaa),
        Rgba::BLACK,
    ])
}

/// Decode a source GB register while raw tile indices are still available.
/// OBJ color zero is transparent regardless of the register's bottom bits.
pub fn background_palette(register: u8) -> Palette {
    let shades = [
        Rgba::WHITE,
        Rgba::rgb(0xaa, 0xaa, 0xaa),
        Rgba::rgb(0x55, 0x55, 0x55),
        Rgba::BLACK,
    ];
    Palette::new(&core::array::from_fn::<_, 4, _>(|index| {
        shades[((register >> (index * 2)) & 3) as usize]
    }))
}

pub fn sprite_palette(register: u8) -> Palette {
    let mut palette = background_palette(register);
    palette.colors[0] = Rgba::TRANSPARENT;
    palette
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{tile::Tile, FrameBuffer};
    use dotzuki_engine::render_config::RenderConfig;

    #[test]
    fn silph_fade_registers_map_raw_bg_and_obj_indices_independently() {
        // Literal independent fbcf7d home/fade.asm + data/gb_palettes.asm.
        // Raw OBJ0 index 1 at FadePal2 is BLACK, while raw BG0 is dark gray;
        // post-composite BGP remapping cannot implement both.
        let w = Rgba::WHITE;
        let l = Rgba::rgb(0xaa, 0xaa, 0xaa);
        let d = Rgba::rgb(0x55, 0x55, 0x55);
        let b = Rgba::BLACK;
        let t = Rgba::TRANSPARENT;
        for (bg, obj0, obj1, bg_expected, obj0_expected, obj1_expected) in [
            (0xe4, 0xd0, 0xe0, [w, l, d, b], [t, w, l, b], [t, w, d, b]),
            (0xf9, 0xe4, 0xe4, [l, d, b, b], [t, l, d, b], [t, l, d, b]),
            (0xfe, 0xfe, 0xf8, [d, b, b, b], [t, b, b, b], [t, d, b, b]),
            (0xff, 0xff, 0xff, [b, b, b, b], [t, b, b, b], [t, b, b, b]),
        ] {
            assert_eq!(&background_palette(bg).colors[..4], &bg_expected);
            assert_eq!(&sprite_palette(obj0).colors[..4], &obj0_expected);
            assert_eq!(&sprite_palette(obj1).colors[..4], &obj1_expected);
        }
        // Color zero stays transparent even though black registers map it to3.
        let mut fb = FrameBuffer::new(RenderConfig::new(8, 8), w);
        let mut bytes = [0; 16];
        bytes[0] = 0x40; // raw0, raw1, raw0...
        let tile = Tile::from_2bpp(&bytes);
        fb.blit_gb_tile(
            0,
            0,
            &Tile::blank(),
            &background_palette(0xfe),
            false,
            false,
            false,
        );
        fb.blit_gb_tile(0, 0, &tile, &sprite_palette(0xfe), true, false, false);
        assert_eq!(fb.get_pixel(0, 0), Some(d));
        assert_eq!(fb.get_pixel(1, 0), Some(b));
    }

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
