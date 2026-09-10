use super::*;
use crate::tile::{TileFormat, TileSet};
use dotzuki_engine::render::Rgba;
use dotzuki_engine::render_config::RenderConfig;

#[test]
fn constants_are_consistent() {
    let config = RenderConfig::new(160, 144);
    assert_eq!(config.screen_width, 160);
    assert_eq!(config.screen_height, 144);
    assert_eq!(config.screen_width / 8, 20); // TILE_SIZE=8 → SCREEN_WIDTH_TILES=20
    assert_eq!(config.screen_height / 8, 18); // TILE_SIZE=8 → SCREEN_HEIGHT_TILES=18
    assert_eq!(BYTES_PER_PIXEL, 4);
    assert_eq!(
        config.screen_width as usize * config.screen_height as usize * BYTES_PER_PIXEL,
        92160
    );
}

// ── TileFormat & TileSet tests ──────────────────────────────────────────

#[test]
fn tileset_from_2bpp_has_gb_format() {
    let data = [0u8; 16];
    let ts = TileSet::from_2bpp(&data);
    assert_eq!(ts.tile_format(), TileFormat::Gb2bpp);
    assert_eq!(ts.len(), 1);
}

#[test]
fn tileset_blank_has_gb_format() {
    let ts = TileSet::blank(4);
    assert_eq!(ts.tile_format(), TileFormat::Gb2bpp);
    assert_eq!(ts.len(), 4);
}

#[test]
fn tileset_from_1bpp_has_gb_format() {
    let data = [0u8; 8];
    let ts = TileSet::from_1bpp(&data);
    assert_eq!(ts.tile_format(), TileFormat::Gb2bpp);
    assert_eq!(ts.len(), 1);
}

#[test]
fn from_4bpp_decodes_single_tile() {
    // Build a known 4bpp tile: all pixels should be color index 5 (0b0101).
    // Plane 0: 0xFF bytes for all 1s → contributes 0b11 → bits 0-1 = 3
    // Plane 1: 0x00 bytes for all 0s except one strategic byte → contributes 0b00 → bits 2-3 = 0
    // Combined: 0b0011 = 3
    //
    // For a pixel with color 0b0101 = 5:
    //   Plane 0 must contribute 0b01 (bits 0-1)
    //   Plane 1 must contribute 0b01 (bits 2-3)
    // To get 0b01 from standard 2bpp: lo=0xFF, hi=0x00
    //   ((hi>>bit)&1)<<1 | ((lo>>bit)&1) = (0)<<1 | 1 = 1
    let mut data = [0u8; 32];
    // Plane 0: set to give 2bpp color 1 for every pixel
    // lo=0xFF, hi=0x00 per row
    for row in 0..8 {
        data[row * 2] = 0xFF; // lo
        data[row * 2 + 1] = 0x00; // hi
    }
    // Plane 1: set to give 2bpp color 1 for every pixel
    for row in 0..8 {
        data[16 + row * 2] = 0xFF;
        data[16 + row * 2 + 1] = 0x00;
    }

    let ts = TileSet::from_4bpp(&data);
    assert_eq!(ts.tile_format(), TileFormat::Gba4bpp);
    assert_eq!(ts.len(), 1);

    let tile = ts.get(0);
    for row in 0..8 {
        for col in 0..8 {
            // Plane 0: 2bpp color 1, Plane 1: 2bpp color 1
            // Combined: (1 << 2) | 1 = 5
            assert_eq!(
                tile.pixels[row][col], 5,
                "pixel ({},{}) should be 5",
                row, col
            );
        }
    }
}

#[test]
fn from_4bpp_decodes_multiple_tiles() {
    let data = [0u8; 64]; // 2 tiles × 32 bytes
    let ts = TileSet::from_4bpp(&data);
    assert_eq!(ts.len(), 2);
    assert_eq!(ts.tile_format(), TileFormat::Gba4bpp);
}

#[test]
fn from_4bpp_zero_data_is_all_color_0() {
    let data = [0u8; 32];
    let ts = TileSet::from_4bpp(&data);
    let tile = ts.get(0);
    for row in 0..8 {
        for col in 0..8 {
            assert_eq!(tile.pixels[row][col], 0);
        }
    }
}

#[test]
fn from_rgba_creates_fullcolor_tileset() {
    let rgba_pixels: Vec<Rgba> = (0..64).map(|i| Rgba::new(i as u8, 0, 0, 255)).collect();

    let ts = TileSet::from_rgba(&rgba_pixels, 1);
    assert_eq!(ts.tile_format(), TileFormat::FullColor);
    assert_eq!(ts.len(), 1);

    let rgba_tile = ts.get_rgba(0).expect("should have rgba tile");
    assert_eq!(rgba_tile.pixels[0][0], Rgba::new(0, 0, 0, 255));
    assert_eq!(
        rgba_tile.pixels[0][63 & 7],
        Rgba::new((63 & 7) as u8, 0, 0, 255)
    );
}

#[test]
fn from_rgba_multiple_tiles() {
    let rgba_pixels: Vec<Rgba> = (0..128)
        .map(|i| Rgba::new((i % 256) as u8, 100, 200, 255))
        .collect();

    let ts = TileSet::from_rgba(&rgba_pixels, 2);
    assert_eq!(ts.tile_format(), TileFormat::FullColor);
    assert_eq!(ts.len(), 2);

    // Tile 0, pixel 0
    let t0 = ts.get_rgba(0).unwrap();
    assert_eq!(t0.pixels[0][0], Rgba::new(0, 100, 200, 255));

    // Tile 1, pixel 0 should be at offset 64
    let t1 = ts.get_rgba(1).unwrap();
    assert_eq!(t1.pixels[0][0], Rgba::new(64, 100, 200, 255));
}

#[test]
fn get_rgba_returns_none_for_non_fullcolor() {
    let ts = TileSet::blank(4);
    assert!(ts.get_rgba(0).is_none());
}

#[test]
fn get_rgba_returns_none_for_out_of_bounds() {
    let rgba_pixels = [Rgba::TRANSPARENT; 64];
    let ts = TileSet::from_rgba(&rgba_pixels, 1);
    assert!(ts.get_rgba(0).is_some());
    assert!(ts.get_rgba(1).is_none());
}

#[test]
fn from_4bpp_max_color_is_15() {
    // Build a tile where all pixels are color 15 (0b1111).
    // Plane 0: 2bpp color 3 → lo=0xFF, hi=0xFF
    // Plane 1: 2bpp color 3 → lo=0xFF, hi=0xFF
    // Combined: (3 << 2) | 3 = 15
    let mut data = [0u8; 32];
    for row in 0..8 {
        data[row * 2] = 0xFF;
        data[row * 2 + 1] = 0xFF;
        data[16 + row * 2] = 0xFF;
        data[16 + row * 2 + 1] = 0xFF;
    }

    let ts = TileSet::from_4bpp(&data);
    let tile = ts.get(0);
    for row in 0..8 {
        for col in 0..8 {
            assert_eq!(tile.pixels[row][col], 15);
        }
    }
}
