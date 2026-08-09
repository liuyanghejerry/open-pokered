use super::*;
use crate::input::{keycode_to_gb_button, GbButton, InputState};
use crate::palette::*;
use jrpg_engine::render_config::RenderConfig;
use pokered_data::impl_traits::PokemonPaletteData;
use pokered_data::sgb_palettes::*;
use winit::keyboard::KeyCode;

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

#[test]
fn rgba_constructors() {
    let c = Rgba::new(1, 2, 3, 4);
    assert_eq!(c.to_array(), [1, 2, 3, 4]);
    let c2 = Rgba::rgb(10, 20, 30);
    assert_eq!(c2.to_array(), [10, 20, 30, 0xFF]);
    assert_eq!(Rgba::WHITE.to_array(), [0xFF, 0xFF, 0xFF, 0xFF]);
    assert_eq!(Rgba::BLACK.to_array(), [0, 0, 0, 0xFF]);
    assert_eq!(Rgba::TRANSPARENT.to_array(), [0, 0, 0, 0]);
}

#[test]
fn framebuffer_new_cleared() {
    let fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::BLACK);
    assert_eq!(fb.data.len(), 160 * 144 * 4);
    assert_eq!(fb.get_pixel(0, 0), Some(Rgba::BLACK));
    assert_eq!(fb.get_pixel(159, 143), Some(Rgba::BLACK));
}

#[test]
fn framebuffer_default_is_white() {
    let fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    assert_eq!(fb.get_pixel(0, 0), Some(Rgba::WHITE));
}

#[test]
fn framebuffer_set_get_pixel() {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let red = Rgba::rgb(0xFF, 0, 0);
    assert!(fb.set_pixel(10, 20, red));
    assert_eq!(fb.get_pixel(10, 20), Some(red));
    assert_eq!(fb.get_pixel(11, 20), Some(Rgba::WHITE));
}

#[test]
fn framebuffer_out_of_bounds() {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    assert!(!fb.set_pixel(160, 0, Rgba::BLACK));
    assert!(!fb.set_pixel(0, 144, Rgba::BLACK));
    assert!(!fb.set_pixel(200, 200, Rgba::BLACK));
    assert_eq!(fb.get_pixel(160, 0), None);
    assert_eq!(fb.get_pixel(0, 144), None);
}

#[test]
fn framebuffer_clear() {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    fb.set_pixel(50, 50, Rgba::BLACK);
    fb.clear(Rgba::rgb(0x12, 0x34, 0x56));
    assert_eq!(fb.get_pixel(50, 50), Some(Rgba::rgb(0x12, 0x34, 0x56)));
    assert_eq!(fb.get_pixel(0, 0), Some(Rgba::rgb(0x12, 0x34, 0x56)));
}

#[test]
fn framebuffer_fill_rect() {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let blue = Rgba::rgb(0, 0, 0xFF);
    fb.fill_rect(10, 20, 5, 3, blue);
    for y in 20..23 {
        for x in 10..15 {
            assert_eq!(fb.get_pixel(x, y), Some(blue), "pixel ({x}, {y})");
        }
    }
    assert_eq!(fb.get_pixel(9, 20), Some(Rgba::WHITE));
    assert_eq!(fb.get_pixel(15, 20), Some(Rgba::WHITE));
    assert_eq!(fb.get_pixel(10, 19), Some(Rgba::WHITE));
    assert_eq!(fb.get_pixel(10, 23), Some(Rgba::WHITE));
}

#[test]
fn framebuffer_fill_rect_clamped() {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    fb.fill_rect(155, 140, 20, 20, Rgba::BLACK);
    assert_eq!(fb.get_pixel(155, 140), Some(Rgba::BLACK));
    assert_eq!(fb.get_pixel(159, 143), Some(Rgba::BLACK));
    assert_eq!(fb.get_pixel(154, 140), Some(Rgba::WHITE));
}

#[test]
fn framebuffer_row_slice() {
    let fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let row = fb.row_slice(0).unwrap();
    assert_eq!(row.len(), 160 * 4);
    assert_eq!(&row[0..4], &[0xFF, 0xFF, 0xFF, 0xFF]);
    assert!(fb.row_slice(144).is_none());
}

#[test]
fn framebuffer_row_slice_mut() {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    {
        let row = fb.row_slice_mut(5).unwrap();
        row[0] = 0x12;
        row[1] = 0x34;
        row[2] = 0x56;
        row[3] = 0x78;
    }
    assert_eq!(fb.get_pixel(0, 5), Some(Rgba::new(0x12, 0x34, 0x56, 0x78)));
}

#[test]
fn framebuffer_blit_row() {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let src = [0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x33, 0x44];
    assert!(fb.blit_row(5, 10, &src, 2));
    assert_eq!(fb.get_pixel(5, 10), Some(Rgba::new(0xAA, 0xBB, 0xCC, 0xDD)));
    assert_eq!(fb.get_pixel(6, 10), Some(Rgba::new(0x11, 0x22, 0x33, 0x44)));
    assert_eq!(fb.get_pixel(4, 10), Some(Rgba::WHITE));
}

#[test]
fn framebuffer_blit_row_out_of_bounds() {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let src = [0; 8];
    assert!(!fb.blit_row(160, 0, &src, 2));
    assert!(!fb.blit_row(0, 144, &src, 2));
}

// --- Palette tests ---

#[test]
fn gb_color_from_u8() {
    assert_eq!(GbColor::from_u8(0), GbColor::White);
    assert_eq!(GbColor::from_u8(1), GbColor::LightGray);
    assert_eq!(GbColor::from_u8(2), GbColor::DarkGray);
    assert_eq!(GbColor::from_u8(3), GbColor::Black);
    assert_eq!(GbColor::from_u8(4), GbColor::White);
    assert_eq!(GbColor::from_u8(0xFF), GbColor::Black);
}

#[test]
fn dmg_palette_colors() {
    assert_eq!(
        DMG_PALETTE.color(GbColor::White),
        Rgba::rgb(0x9B, 0xBC, 0x0F)
    );
    assert_eq!(
        DMG_PALETTE.color(GbColor::LightGray),
        Rgba::rgb(0x8B, 0xAC, 0x0F)
    );
    assert_eq!(
        DMG_PALETTE.color(GbColor::DarkGray),
        Rgba::rgb(0x30, 0x62, 0x30)
    );
    assert_eq!(
        DMG_PALETTE.color(GbColor::Black),
        Rgba::rgb(0x0F, 0x38, 0x0F)
    );
}

#[test]
fn grayscale_palette_colors() {
    assert_eq!(
        GRAYSCALE_PALETTE.color(GbColor::White),
        Rgba::rgb(0xFF, 0xFF, 0xFF)
    );
    assert_eq!(GRAYSCALE_PALETTE.color(GbColor::Black), Rgba::rgb(0, 0, 0));
}

#[test]
fn bgp_register_normal() {
    let pal = Palette::from_bgp_register(DEFAULT_BGP, &GRAYSCALE_PALETTE);
    assert_eq!(pal.colors[0], Rgba::rgb(0xFF, 0xFF, 0xFF));
    assert_eq!(pal.colors[1], Rgba::rgb(0xAA, 0xAA, 0xAA));
    assert_eq!(pal.colors[2], Rgba::rgb(0x55, 0x55, 0x55));
    assert_eq!(pal.colors[3], Rgba::rgb(0x00, 0x00, 0x00));
}

#[test]
fn bgp_register_inverted() {
    let inverted: u8 = 0b00011011;
    let pal = Palette::from_bgp_register(inverted, &GRAYSCALE_PALETTE);
    assert_eq!(pal.colors[0], Rgba::rgb(0x00, 0x00, 0x00));
    assert_eq!(pal.colors[1], Rgba::rgb(0x55, 0x55, 0x55));
    assert_eq!(pal.colors[2], Rgba::rgb(0xAA, 0xAA, 0xAA));
    assert_eq!(pal.colors[3], Rgba::rgb(0xFF, 0xFF, 0xFF));
}

#[test]
fn bgp_register_white_out() {
    let pal = Palette::from_bgp_register(0x00, &GRAYSCALE_PALETTE);
    for i in 0..4 {
        assert_eq!(pal.colors[i], Rgba::rgb(0xFF, 0xFF, 0xFF));
    }
}

#[test]
fn palette_state_default() {
    let ps = PaletteState::default();
    assert_eq!(ps.bgp, DEFAULT_BGP);
    assert_eq!(ps.obp0, DEFAULT_OBP0);
    assert_eq!(ps.obp1, DEFAULT_OBP1);
}

#[test]
fn palette_state_white_out_and_reset() {
    let mut ps = PaletteState::default();
    ps.white_out();
    assert_eq!(ps.bgp, 0);
    assert_eq!(ps.obp0, 0);
    assert_eq!(ps.obp1, 0);
    ps.reset_normal();
    assert_eq!(ps.bgp, DEFAULT_BGP);
    assert_eq!(ps.obp0, DEFAULT_OBP0);
}

// --- Input tests ---

#[test]
fn input_state_initial() {
    let input = InputState::new();
    for button in GbButton::ALL {
        assert!(!input.is_held(button));
        assert!(!input.is_just_pressed(button));
        assert!(!input.is_just_released(button));
    }
    assert!(!input.any_held());
    assert!(!input.any_just_pressed());
}

#[test]
fn input_press_and_release() {
    let mut input = InputState::new();
    input.press(GbButton::A);
    assert!(input.is_held(GbButton::A));
    assert!(!input.is_held(GbButton::B));
    input.release(GbButton::A);
    assert!(!input.is_held(GbButton::A));
}

#[test]
fn input_just_pressed() {
    let mut input = InputState::new();
    input.press(GbButton::Start);
    assert!(input.is_just_pressed(GbButton::Start));
    input.begin_frame();
    assert!(!input.is_just_pressed(GbButton::Start));
    assert!(input.is_held(GbButton::Start));
}

#[test]
fn input_just_released() {
    let mut input = InputState::new();
    input.press(GbButton::Up);
    input.begin_frame();
    input.release(GbButton::Up);
    assert!(input.is_just_released(GbButton::Up));
}

#[test]
fn input_any_flags() {
    let mut input = InputState::new();
    assert!(!input.any_held());
    input.press(GbButton::Down);
    assert!(input.any_held());
    assert!(input.any_just_pressed());
}

#[test]
fn input_clear() {
    let mut input = InputState::new();
    input.press(GbButton::A);
    input.press(GbButton::B);
    input.clear();
    assert!(!input.any_held());
    assert_eq!(input.raw_current(), 0);
    assert_eq!(input.raw_previous(), 0);
}

#[test]
fn input_raw_values() {
    let mut input = InputState::new();
    input.press(GbButton::A);
    input.press(GbButton::Start);
    assert_eq!(
        input.raw_current(),
        GbButton::A.bit_mask() | GbButton::Start.bit_mask()
    );
}

#[test]
fn input_keycode_mapping() {
    assert_eq!(keycode_to_gb_button(KeyCode::ArrowUp), Some(GbButton::Up));
    assert_eq!(
        keycode_to_gb_button(KeyCode::ArrowDown),
        Some(GbButton::Down)
    );
    assert_eq!(
        keycode_to_gb_button(KeyCode::ArrowLeft),
        Some(GbButton::Left)
    );
    assert_eq!(
        keycode_to_gb_button(KeyCode::ArrowRight),
        Some(GbButton::Right)
    );
    assert_eq!(keycode_to_gb_button(KeyCode::KeyZ), Some(GbButton::A));
    assert_eq!(keycode_to_gb_button(KeyCode::KeyX), Some(GbButton::B));
    assert_eq!(keycode_to_gb_button(KeyCode::Enter), Some(GbButton::Start));
    assert_eq!(
        keycode_to_gb_button(KeyCode::Backspace),
        Some(GbButton::Select)
    );
    assert_eq!(
        keycode_to_gb_button(KeyCode::ShiftRight),
        Some(GbButton::Select)
    );
    assert_eq!(keycode_to_gb_button(KeyCode::KeyW), Some(GbButton::Up));
    assert_eq!(keycode_to_gb_button(KeyCode::KeyS), Some(GbButton::Down));
    assert_eq!(keycode_to_gb_button(KeyCode::KeyA), Some(GbButton::Left));
    assert_eq!(keycode_to_gb_button(KeyCode::KeyD), Some(GbButton::Right));
    assert_eq!(keycode_to_gb_button(KeyCode::Space), Some(GbButton::Start));
    assert_eq!(keycode_to_gb_button(KeyCode::KeyQ), None);
}

#[test]
fn input_set_from_keycode() {
    let mut input = InputState::new();
    input.set_from_keycode(KeyCode::ArrowUp, true);
    assert!(input.is_held(GbButton::Up));
    input.set_from_keycode(KeyCode::ArrowUp, false);
    assert!(!input.is_held(GbButton::Up));
    input.set_from_keycode(KeyCode::KeyQ, true);
    assert!(!input.any_held());
}

#[test]
fn input_multiple_buttons() {
    let mut input = InputState::new();
    input.press(GbButton::A);
    input.press(GbButton::Up);
    input.press(GbButton::Right);
    assert!(input.is_held(GbButton::A));
    assert!(input.is_held(GbButton::Up));
    assert!(input.is_held(GbButton::Right));
    assert!(!input.is_held(GbButton::B));
    input.release(GbButton::Up);
    assert!(!input.is_held(GbButton::Up));
    assert!(input.is_held(GbButton::A));
}

#[test]
fn gb_button_bit_masks_unique() {
    let mut seen = 0u8;
    for button in GbButton::ALL {
        let mask = button.bit_mask();
        assert_eq!(seen & mask, 0, "duplicate bit mask for {:?}", button);
        seen |= mask;
    }
    assert_eq!(seen, 0xFF);
}

#[test]
fn gb_color_all() {
    assert_eq!(GbColor::ALL.len(), 4);
    assert_eq!(GbColor::ALL[0], GbColor::White);
    assert_eq!(GbColor::ALL[3], GbColor::Black);
}

#[test]
fn obp0_default_makes_color0_transparent() {
    let pal = Palette::from_bgp_register(DEFAULT_OBP0, &GRAYSCALE_PALETTE);
    assert_eq!(pal.colors[0], Rgba::rgb(0xFF, 0xFF, 0xFF));
    assert_eq!(pal.colors[1], Rgba::rgb(0xFF, 0xFF, 0xFF));
    assert_eq!(pal.colors[2], Rgba::rgb(0xAA, 0xAA, 0xAA));
    assert_eq!(pal.colors[3], Rgba::rgb(0x00, 0x00, 0x00));
}

// =====================================================
// M5.2 — Tile, TileMap, Viewport, WindowLayer tests
// =====================================================

use crate::tile::{decode_2bpp_row, RgbaTileSet, Tile, TileSet, BYTES_PER_TILE, TILE_PIXELS};
use crate::tilemap::{BG_MAP_HEIGHT, BG_MAP_SIZE, BG_MAP_WIDTH};
use jrpg_engine::tilemap::Tilemap as TileMap;
use jrpg_engine::tilemap::TilemapEntry;
use crate::window_layer::WindowLayer;

// --- Tile tests ---

#[test]
fn tile_blank() {
    let t = Tile::blank();
    for row in 0..TILE_PIXELS {
        for col in 0..TILE_PIXELS {
            assert_eq!(t.get(row, col), 0);
        }
    }
}

#[test]
fn tile_decode_2bpp_all_zeros() {
    let data = [0u8; BYTES_PER_TILE];
    let t = Tile::from_2bpp(&data);
    for row in 0..TILE_PIXELS {
        for col in 0..TILE_PIXELS {
            assert_eq!(t.get(row, col), 0, "Expected 0 at ({row}, {col})");
        }
    }
}

#[test]
fn tile_decode_2bpp_all_ones() {
    // lo=0xFF, hi=0xFF for each row → all pixels = color 3
    let data = [0xFF; BYTES_PER_TILE];
    let t = Tile::from_2bpp(&data);
    for row in 0..TILE_PIXELS {
        for col in 0..TILE_PIXELS {
            assert_eq!(t.get(row, col), 3);
        }
    }
}

#[test]
fn tile_decode_2bpp_mixed() {
    // Row 0: lo=0b10000000, hi=0b00000000 → pixel 0 = color 1, rest = 0
    let mut data = [0u8; BYTES_PER_TILE];
    data[0] = 0b10000000; // lo byte, row 0
    data[1] = 0b00000000; // hi byte, row 0
    let t = Tile::from_2bpp(&data);
    assert_eq!(t.get(0, 0), 1); // lo=1, hi=0 → color 1
    assert_eq!(t.get(0, 1), 0);
    assert_eq!(t.get(0, 7), 0);
}

#[test]
fn tile_decode_2bpp_hi_bit() {
    // Row 0: lo=0, hi=0b10000000 → pixel 0 = color 2
    let mut data = [0u8; BYTES_PER_TILE];
    data[0] = 0b00000000;
    data[1] = 0b10000000;
    let t = Tile::from_2bpp(&data);
    assert_eq!(t.get(0, 0), 2); // lo=0, hi=1 → color 2
}

#[test]
fn tile_decode_2bpp_both_bits() {
    // Row 0: lo=0b10000000, hi=0b10000000 → pixel 0 = color 3
    let mut data = [0u8; BYTES_PER_TILE];
    data[0] = 0b10000000;
    data[1] = 0b10000000;
    let t = Tile::from_2bpp(&data);
    assert_eq!(t.get(0, 0), 3);
}

#[test]
fn tile_decode_alternating_pattern() {
    // lo=0b10101010, hi=0b01010101 for row 0
    // bit7: lo=1,hi=0 → 1 | bit6: lo=0,hi=1 → 2 | bit5: lo=1,hi=0 → 1 | ...
    let mut data = [0u8; BYTES_PER_TILE];
    data[0] = 0b10101010;
    data[1] = 0b01010101;
    let t = Tile::from_2bpp(&data);
    assert_eq!(t.get(0, 0), 1); // bit7: lo=1,hi=0
    assert_eq!(t.get(0, 1), 2); // bit6: lo=0,hi=1
    assert_eq!(t.get(0, 2), 1);
    assert_eq!(t.get(0, 3), 2);
    assert_eq!(t.get(0, 4), 1);
    assert_eq!(t.get(0, 5), 2);
    assert_eq!(t.get(0, 6), 1);
    assert_eq!(t.get(0, 7), 2);
}

#[test]
fn tile_flip_x() {
    let mut data = [0u8; BYTES_PER_TILE];
    data[0] = 0b10000000; // only pixel 0 has lo=1
    let t = Tile::from_2bpp(&data);
    let flipped = t.flip_x();
    assert_eq!(flipped.get(0, 7), 1); // pixel 0 → pixel 7
    assert_eq!(flipped.get(0, 0), 0);
}

#[test]
fn tile_flip_y() {
    let mut data = [0u8; BYTES_PER_TILE];
    data[0] = 0xFF; // row 0: all lo=1
    data[1] = 0x00;
    let t = Tile::from_2bpp(&data);
    assert_eq!(t.get(0, 0), 1);
    assert_eq!(t.get(7, 0), 0);
    let flipped = t.flip_y();
    assert_eq!(flipped.get(7, 0), 1); // row 0 → row 7
    assert_eq!(flipped.get(0, 0), 0);
}

#[test]
fn tile_render_row() {
    let mut data = [0u8; BYTES_PER_TILE];
    data[0] = 0xFF; // lo=all 1s for row 0
    data[1] = 0x00; // hi=all 0s → all pixels = color 1
    let t = Tile::from_2bpp(&data);
    let row = t.render_row(0, &GRAYSCALE_PALETTE);
    for pixel in &row {
        assert_eq!(*pixel, Rgba::rgb(0xAA, 0xAA, 0xAA)); // LightGray
    }
}

#[test]
fn decode_2bpp_row_function() {
    let result = decode_2bpp_row(0b11001100, 0b10101010);
    // bit7: lo=1,hi=1 → 3
    // bit6: lo=1,hi=0 → 1
    // bit5: lo=0,hi=1 → 2
    // bit4: lo=0,hi=0 → 0
    // bit3: lo=1,hi=1 → 3
    // bit2: lo=1,hi=0 → 1
    // bit1: lo=0,hi=1 → 2
    // bit0: lo=0,hi=0 → 0
    assert_eq!(result, [3, 1, 2, 0, 3, 1, 2, 0]);
}

// --- TileSet tests ---

#[test]
fn tileset_from_2bpp() {
    // 2 tiles worth of data
    let data = [0u8; BYTES_PER_TILE * 2];
    let ts = TileSet::from_2bpp(&data);
    assert_eq!(ts.len(), 2);
    assert!(!ts.is_empty());
}

#[test]
fn tileset_blank() {
    let ts = TileSet::blank(10);
    assert_eq!(ts.len(), 10);
    for i in 0..10 {
        assert_eq!(ts.get(i).get(0, 0), 0);
    }
}

#[test]
fn tileset_out_of_bounds_returns_blank() {
    let ts = TileSet::blank(1);
    let t = ts.get(999);
    assert_eq!(t.get(0, 0), 0);
}

#[test]
fn tileset_from_1bpp() {
    // 1bpp: 8 bytes per tile, bit=1 → color 3, bit=0 → color 0
    let mut data = [0u8; 8];
    data[0] = 0b10000001; // row 0: pixels 0 and 7 are black
    let ts = TileSet::from_1bpp(&data);
    assert_eq!(ts.len(), 1);
    assert_eq!(ts.get(0).get(0, 0), 3);
    assert_eq!(ts.get(0).get(0, 1), 0);
    assert_eq!(ts.get(0).get(0, 7), 3);
}

#[test]
fn tileset_load_2bpp_at() {
    let mut ts = TileSet::blank(4);
    // Create a tile with all color 3
    let tile_data = [0xFF; BYTES_PER_TILE];
    ts.load_2bpp_at(2, &tile_data);
    assert_eq!(ts.get(0).get(0, 0), 0); // tile 0 still blank
    assert_eq!(ts.get(2).get(0, 0), 3); // tile 2 loaded
}

// --- RGBA Tile tests ---

#[test]
#[cfg(feature = "gpu")]
fn tileset_from_rgba_png_basic() {
    // Create a 16×8 RGBA PNG: tile 0 = red, tile 1 = green
    let mut img = image::RgbaImage::new(16, 8);
    // Tile 0 (left 8×8): all red
    for y in 0..8 {
        for x in 0..8 {
            img.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
        }
    }
    // Tile 1 (right 8×8): all green
    for y in 0..8 {
        for x in 8..16 {
            img.put_pixel(x, y, image::Rgba([0, 255, 0, 255]));
        }
    }

    // Encode to PNG bytes
    let mut png_bytes = Vec::new();
    let dyn_img = image::DynamicImage::from(img);
    dyn_img
        .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .unwrap();

    let ts = RgbaTileSet::from_rgba_png(&png_bytes).unwrap();
    assert_eq!(ts.len(), 2);
    assert!(!ts.is_empty());

    // Tile 0 should be all red
    for row in 0..TILE_PIXELS {
        for col in 0..TILE_PIXELS {
            assert_eq!(
                ts.get(0).pixels[row][col],
                Rgba::rgb(255, 0, 0),
                "Tile 0 pixel ({},{}) mismatch",
                row,
                col
            );
        }
    }

    // Tile 1 should be all green
    for row in 0..TILE_PIXELS {
        for col in 0..TILE_PIXELS {
            assert_eq!(
                ts.get(1).pixels[row][col],
                Rgba::rgb(0, 255, 0),
                "Tile 1 pixel ({},{}) mismatch",
                row,
                col
            );
        }
    }

    // Verify no palette remapping: actual RGBA values match exactly
        assert_eq!(ts.get(0).pixels[0][0], Rgba::from([255, 0, 0, 255]));
        assert_eq!(ts.get(1).pixels[0][0], Rgba::from([0, 255, 0, 255]));
}

#[test]
#[cfg(feature = "gpu")]
fn tileset_from_rgba_png_invalid_dimensions() {
    // 15×8 is not a multiple of 8 → should fail
    let img = image::RgbaImage::new(15, 8);
    let mut png_bytes = Vec::new();
    let dyn_img = image::DynamicImage::from(img);
    dyn_img
        .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .unwrap();
    assert!(RgbaTileSet::from_rgba_png(&png_bytes).is_err());
}

#[test]
#[cfg(feature = "gpu")]
fn tileset_from_rgba_png_blank_out_of_bounds() {
    // Create a valid 8×8 transparent PNG
    let img = image::RgbaImage::new(8, 8);
    let mut png_bytes = Vec::new();
    let dyn_img = image::DynamicImage::from(img);
    dyn_img
        .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .unwrap();

    let ts = RgbaTileSet::from_rgba_png(&png_bytes).unwrap();
    assert_eq!(ts.len(), 1);
    // Out-of-bounds should return transparent blank
    let blank = ts.get(999);
    for row in 0..TILE_PIXELS {
        for col in 0..TILE_PIXELS {
            assert_eq!(blank.pixels[row][col], Rgba::TRANSPARENT);
        }
    }
}

#[test]
#[cfg(feature = "gpu")]
fn tileset_from_rgba_png_alpha_channel() {
    // Create an 8×8 PNG with varying alpha
    let mut img = image::RgbaImage::new(8, 8);
    img.put_pixel(0, 0, image::Rgba([128, 64, 32, 16]));
    img.put_pixel(7, 7, image::Rgba([200, 100, 50, 200]));
    let mut png_bytes = Vec::new();
    let dyn_img = image::DynamicImage::from(img);
    dyn_img
        .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .unwrap();

    let ts = RgbaTileSet::from_rgba_png(&png_bytes).unwrap();
    assert_eq!(ts.len(), 1);
    assert_eq!(ts.get(0).pixels[0][0], Rgba::from([128, 64, 32, 16]));
    assert_eq!(ts.get(0).pixels[7][7], Rgba::from([200, 100, 50, 200]));
}

// --- TileMap tests ---

#[test]
fn tilemap_new_all_zero() {
    let tm = TileMap::new(32, 32);
    for y in 0..BG_MAP_HEIGHT {
        for x in 0..BG_MAP_WIDTH {
            assert_eq!(tm.get(x as u16, y as u16).unwrap().tile_id, 0);
        }
    }
}

#[test]
fn tilemap_set_get() {
    let mut tm = TileMap::new(32, 32);
    tm.set(5, 10, TilemapEntry { tile_id: 42, ..Default::default() });
    assert_eq!(tm.get(5, 10).unwrap().tile_id, 42);
    assert_eq!(tm.get(6, 10).unwrap().tile_id, 0);
}

#[test]
fn tilemap_no_wrapping() {
    let mut tm = TileMap::new(32, 32);
    tm.set(0, 0, TilemapEntry { tile_id: 99, ..Default::default() });
    assert_eq!(tm.get(32, 32), None); // out of bounds, no wrapping
    assert_eq!(tm.get(64, 64), None);
}

#[test]
fn tilemap_from_gb_tilemap() {
    let mut data = vec![0u8; BG_MAP_SIZE];
    data[0] = 5;
    data[31] = 10;
    data[32] = 15; // row 1, col 0
    let tm = TileMap::from_gb_tilemap(&data, 32, 32);
    assert_eq!(tm.get(0, 0).unwrap().tile_id, 5);
    assert_eq!(tm.get(31, 0).unwrap().tile_id, 10);
    assert_eq!(tm.get(0, 1).unwrap().tile_id, 15);
}

#[test]
fn tilemap_fill_rect() {
    let mut tm = TileMap::new(32, 32);
    tm.fill_rect(2, 3, 4, 5, TilemapEntry { tile_id: 77, ..Default::default() });
    assert_eq!(tm.get(2, 3).unwrap().tile_id, 77);
    assert_eq!(tm.get(5, 7).unwrap().tile_id, 77);
    assert_eq!(tm.get(1, 3).unwrap().tile_id, 0);
    assert_eq!(tm.get(6, 3).unwrap().tile_id, 0);
    assert_eq!(tm.get(2, 2).unwrap().tile_id, 0);
    assert_eq!(tm.get(2, 8).unwrap().tile_id, 0);
}

#[test]
fn tilemap_render_solid_tile() {
    let mut tm = TileMap::new(1, 1);
    tm.set(0, 0, TilemapEntry {
        tile_id: 0,
        ..Default::default()
    });

    let tile_data = [0xFFu8; BYTES_PER_TILE];
    let ts = TileSet::from_2bpp(&tile_data);

    let mut fb = FrameBuffer {
        data: vec![0u8; (8 * 8 * 4) as usize],
        width: 8,
        height: 8,
        dirty_region: DirtyRegion::full(8, 8),
    };
    fb.clear(Rgba::WHITE);

    for y in 0..tm.height {
        for x in 0..tm.width {
            if let Some(entry) = tm.get(x, y) {
                let tile = ts.get(entry.tile_id as usize);
                let base_x = x as u32 * 8;
                let base_y = y as u32 * 8;
                for py in 0..TILE_PIXELS {
                    for px in 0..TILE_PIXELS {
                        let color_idx = tile.pixels[py][px];
                        let rgba = GRAYSCALE_PALETTE.color(GbColor::from_u8(color_idx));
                        fb.set_pixel(base_x + px as u32, base_y + py as u32, rgba);
                    }
                }
            }
        }
    }

    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(
                fb.get_pixel(x, y),
                Some(Rgba::BLACK),
                "pixel ({},{}) should be black",
                x,
                y,
            );
        }
    }
}

#[test]
fn tilemap_render_with_scroll() {
    let mut tm = TileMap::new(2, 1);
    tm.set(0, 0, TilemapEntry {
        tile_id: 0,
        ..Default::default()
    });
    tm.set(1, 0, TilemapEntry {
        tile_id: 1,
        ..Default::default()
    });

    let mut ts_data = vec![0u8; BYTES_PER_TILE * 2];
    for b in ts_data.iter_mut().take(BYTES_PER_TILE) {
        *b = 0xFF;
    }
    let ts = TileSet::from_2bpp(&ts_data);

    let mut fb = FrameBuffer {
        data: vec![0u8; (8 * 8 * 4) as usize],
        width: 8,
        height: 8,
        dirty_region: DirtyRegion::full(8, 8),
    };
    fb.clear(Rgba::TRANSPARENT);

    let scroll_x: i32 = 4;
    let scroll_y: i32 = 0;

    let screen_w = fb.width;
    let screen_h = fb.height;
    for screen_y in 0..screen_h {
        let world_y = scroll_y + screen_y as i32;
        if world_y < 0 {
            continue;
        }
        let tile_y = (world_y as u32) / 8;
        let pixel_y = (world_y as u32 % 8) as usize;

        for screen_x in 0..screen_w {
            let world_x = scroll_x + screen_x as i32;
            if world_x < 0 {
                continue;
            }
            let tile_x = (world_x as u32) / 8;
            let pixel_x = (world_x as u32 % 8) as usize;

            if let Some(entry) = tm.get(tile_x as u16, tile_y as u16) {
                let tile = ts.get(entry.tile_id as usize);
                let color_idx = tile.pixels[pixel_y][pixel_x];
                let rgba = GRAYSCALE_PALETTE.color(GbColor::from_u8(color_idx));
                fb.set_pixel(screen_x, screen_y, rgba);
            }
        }
    }

    for y in 0..8 {
        for x in 0..4 {
            assert_eq!(
                fb.get_pixel(x, y),
                Some(Rgba::BLACK),
                "pixel ({},{}) should be black (tile 0)",
                x,
                y,
            );
        }
    }
    for y in 0..8 {
        for x in 4..8 {
            assert_eq!(
                fb.get_pixel(x, y),
                Some(Rgba::WHITE),
                "pixel ({},{}) should be white (tile 1)",
                x,
                y,
            );
        }
    }
}

// --- WindowLayer tests ---

#[test]
fn window_layer_disabled_by_default() {
    let wl = WindowLayer::new();
    assert!(!wl.enabled);
    assert_eq!(wl.wx, 7);
    assert_eq!(wl.wy, 0);
}

#[test]
fn window_layer_screen_x() {
    let mut wl = WindowLayer::new();
    wl.wx = 7;
    assert_eq!(wl.screen_x(), 0);
    wl.wx = 87;
    assert_eq!(wl.screen_x(), 80);
    wl.wx = 3; // less than 7, clamp to 0
    assert_eq!(wl.screen_x(), 0);
}

#[test]
fn window_layer_render_disabled() {
    let wl = WindowLayer::new();
    let ts = TileSet::blank(1);
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    wl.render(&mut fb, &ts, &GRAYSCALE_PALETTE);
    // Should not change anything since disabled
    assert_eq!(fb.get_pixel(0, 0), Some(Rgba::WHITE));
}

#[test]
fn window_layer_render_enabled() {
    let mut wl = WindowLayer::new();
    wl.enabled = true;
    wl.wx = 7; // starts at screen x=0
    wl.wy = 0; // starts at screen y=0

    // Tile 0 = all black
    let tile_data = [0xFF; BYTES_PER_TILE];
    let ts = TileSet::from_2bpp(&tile_data);

    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    wl.render(&mut fb, &ts, &GRAYSCALE_PALETTE);
    // Window covers entire screen with black tile
    assert_eq!(fb.get_pixel(0, 0), Some(Rgba::BLACK));
    assert_eq!(fb.get_pixel(159, 143), Some(Rgba::BLACK));
}

#[test]
fn window_layer_partial_coverage() {
    let mut wl = WindowLayer::new();
    wl.enabled = true;
    wl.wx = 7 + 80; // starts at screen x=80
    wl.wy = 72; // starts at screen y=72

    let tile_data = [0xFF; BYTES_PER_TILE];
    let ts = TileSet::from_2bpp(&tile_data);

    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    wl.render(&mut fb, &ts, &GRAYSCALE_PALETTE);

    // Above and left of window should remain white
    assert_eq!(fb.get_pixel(0, 0), Some(Rgba::WHITE));
    assert_eq!(fb.get_pixel(79, 71), Some(Rgba::WHITE));
    // Window area should be black
    assert_eq!(fb.get_pixel(80, 72), Some(Rgba::BLACK));
    assert_eq!(fb.get_pixel(159, 143), Some(Rgba::BLACK));
}

// =====================================================
// M5.3 — Sprite rendering tests
// =====================================================

use crate::sprite::{
    SpriteLayer, SpriteOamEntry, OAM_PALETTE, OAM_PRIORITY, OAM_X_FLIP, OAM_Y_FLIP,
};

#[test]
fn sprite_oam_from_raw() {
    let e = SpriteOamEntry::from_raw(32, 24, 0, 0);
    assert_eq!(e.y, 16); // 32 - 16
    assert_eq!(e.x, 16); // 24 - 8
}

#[test]
fn sprite_oam_attributes() {
    let e = SpriteOamEntry::new(
        0,
        0,
        0,
        OAM_PRIORITY | OAM_Y_FLIP | OAM_X_FLIP | OAM_PALETTE,
    );
    assert!(e.bg_priority());
    assert!(e.y_flip());
    assert!(e.x_flip());
    assert!(e.uses_obp1());

    let e2 = SpriteOamEntry::new(0, 0, 0, 0);
    assert!(!e2.bg_priority());
    assert!(!e2.y_flip());
    assert!(!e2.x_flip());
    assert!(!e2.uses_obp1());
}

#[test]
fn sprite_on_screen() {
    let on = SpriteOamEntry::new(0, 0, 0, 0);
    assert!(on.is_on_screen(160, 144));

    let off_right = SpriteOamEntry::new(0, 160, 0, 0);
    assert!(!off_right.is_on_screen(160, 144));

    let off_bottom = SpriteOamEntry::new(144, 0, 0, 0);
    assert!(!off_bottom.is_on_screen(160, 144));

    let partially_on = SpriteOamEntry::new(-4, -4, 0, 0);
    assert!(partially_on.is_on_screen(160, 144));

    let fully_off_left = SpriteOamEntry::new(0, -8, 0, 0);
    assert!(!fully_off_left.is_on_screen(160, 144));
}

#[test]
fn sprite_render_transparent_color0() {
    // Tile with all color 0 = transparent → nothing drawn
    let ts = TileSet::blank(1);
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut layer = SpriteLayer::new();
    layer.add(SpriteOamEntry::new(0, 0, 0, 0));
    layer.render(&mut fb, &ts, &GRAYSCALE_PALETTE, &GRAYSCALE_PALETTE, None);
    // Should still be white (color 0 is transparent)
    assert_eq!(fb.get_pixel(0, 0), Some(Rgba::WHITE));
}

#[test]
fn sprite_render_visible() {
    // Tile 0 = all color 3 (black)
    let tile_data = [0xFF; BYTES_PER_TILE];
    let ts = TileSet::from_2bpp(&tile_data);
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut layer = SpriteLayer::new();
    layer.add(SpriteOamEntry::new(10, 20, 0, 0));
    layer.render(&mut fb, &ts, &GRAYSCALE_PALETTE, &GRAYSCALE_PALETTE, None);
    // Pixel (20, 10) should be black
    assert_eq!(fb.get_pixel(20, 10), Some(Rgba::BLACK));
    // Pixel outside sprite should be white
    assert_eq!(fb.get_pixel(19, 10), Some(Rgba::WHITE));
    assert_eq!(fb.get_pixel(28, 10), Some(Rgba::WHITE));
}

#[test]
fn sprite_render_x_flip() {
    // Create a tile where only column 0 has color 1
    let mut data = [0u8; BYTES_PER_TILE];
    for row in 0..8 {
        data[row * 2] = 0b10000000; // lo: only bit 7 (col 0) set
        data[row * 2 + 1] = 0;
    }
    let ts = TileSet::from_2bpp(&data);

    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut layer = SpriteLayer::new();
    layer.add(SpriteOamEntry::new(0, 0, 0, OAM_X_FLIP));
    layer.render(&mut fb, &ts, &GRAYSCALE_PALETTE, &GRAYSCALE_PALETTE, None);

    // With x-flip, column 0 becomes column 7
    assert_eq!(fb.get_pixel(7, 0), Some(Rgba::rgb(0xAA, 0xAA, 0xAA))); // color 1 = light gray
    assert_eq!(fb.get_pixel(0, 0), Some(Rgba::WHITE)); // transparent (color 0)
}

#[test]
fn sprite_render_y_flip() {
    // Create a tile where only row 0 has color 3
    let mut data = [0u8; BYTES_PER_TILE];
    data[0] = 0xFF; // row 0 lo
    data[1] = 0xFF; // row 0 hi → color 3
    let ts = TileSet::from_2bpp(&data);

    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut layer = SpriteLayer::new();
    layer.add(SpriteOamEntry::new(0, 0, 0, OAM_Y_FLIP));
    layer.render(&mut fb, &ts, &GRAYSCALE_PALETTE, &GRAYSCALE_PALETTE, None);

    // With y-flip, row 0 → row 7
    assert_eq!(fb.get_pixel(0, 7), Some(Rgba::BLACK)); // color 3
    assert_eq!(fb.get_pixel(0, 0), Some(Rgba::WHITE)); // transparent (was row 7, now row 0)
}

#[test]
fn sprite_render_obp1() {
    // Use OBP1 palette
    let tile_data = [0xFF; BYTES_PER_TILE]; // all color 3
    let ts = TileSet::from_2bpp(&tile_data);

    let obp0 = Palette::new(&[Rgba::WHITE, Rgba::WHITE, Rgba::WHITE, Rgba::rgb(0xAA, 0, 0)]);
    let obp1 = Palette::new(&[Rgba::WHITE, Rgba::WHITE, Rgba::WHITE, Rgba::rgb(0, 0, 0xBB)]);

    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut layer = SpriteLayer::new();
    layer.add(SpriteOamEntry::new(0, 0, 0, OAM_PALETTE));
    layer.render(&mut fb, &ts, &obp0, &obp1, None);

    // Should use OBP1's color 3 (blue)
    assert_eq!(fb.get_pixel(0, 0), Some(Rgba::rgb(0, 0, 0xBB)));
}

#[test]
fn sprite_bg_priority() {
    let tile_data = [0xFF; BYTES_PER_TILE]; // all color 3
    let ts = TileSet::from_2bpp(&tile_data);

    // BG color buffer: pixel at (0,0) has BG color 2 (non-zero)
    let mut bg_buf = vec![0u8; (160 * 144) as usize];
    bg_buf[0] = 2; // pixel (0,0) has non-zero BG

    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut layer = SpriteLayer::new();
    layer.add(SpriteOamEntry::new(0, 0, 0, OAM_PRIORITY));
    layer.render(
        &mut fb,
        &ts,
        &GRAYSCALE_PALETTE,
        &GRAYSCALE_PALETTE,
        Some(&bg_buf),
    );

    // Pixel (0,0) should NOT be overwritten (BG priority, BG != 0)
    assert_eq!(fb.get_pixel(0, 0), Some(Rgba::WHITE));
    // Pixel (1,0) has BG color 0, so sprite draws there
    assert_eq!(fb.get_pixel(1, 0), Some(Rgba::BLACK));
}

#[test]
fn sprite_layer_clear() {
    let mut layer = SpriteLayer::new();
    layer.add(SpriteOamEntry::new(0, 0, 0, 0));
    layer.add(SpriteOamEntry::new(10, 10, 0, 0));
    assert_eq!(layer.entries.len(), 2);
    layer.clear();
    assert_eq!(layer.entries.len(), 0);
}

// =====================================================
// M5.4 — Text renderer & TextBox tests
// =====================================================

use crate::text_renderer::{write_tiles_at, ScreenTileBuffer};
use crate::textbox::{
    TextBoxFrame, TILE_BOTTOM_LEFT, TILE_BOTTOM_RIGHT, TILE_DOWN_ARROW, TILE_HORIZONTAL,
    TILE_SPACE, TILE_TOP_LEFT, TILE_TOP_RIGHT, TILE_VERTICAL,
};

// GB screen tile grid (160×144 → 20×18). The engine no longer hardcodes these
// (resolution is per-game now), so these text-renderer tests pin the GB fixture.
const SCREEN_TILES_X: u32 = 20;
const SCREEN_TILES_Y: u32 = 18;
const SCREEN_TILE_COUNT: usize = (SCREEN_TILES_X * SCREEN_TILES_Y) as usize;

#[test]
fn screen_tile_buffer_new_filled_with_space() {
    let buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    for i in 0..SCREEN_TILE_COUNT {
        assert_eq!(buf.tiles[i], 0x7F);
    }
}

#[test]
fn screen_tile_buffer_new_fills_with_spaces() {
    let buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    for i in 0..buf.tiles.len() {
        assert_eq!(buf.tiles[i], 0x7F);
    }
}

#[test]
fn screen_tile_buffer_get_set() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.set(5, 3, 0x42);
    assert_eq!(buf.get(5, 3), 0x42);
    assert_eq!(buf.get(4, 3), 0x7F);
    assert_eq!(buf.get(5, 2), 0x7F);
}

#[test]
fn screen_tile_buffer_get_out_of_bounds() {
    let buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    assert_eq!(buf.get(20, 0), 0x7F);
    assert_eq!(buf.get(0, 18), 0x7F);
    assert_eq!(buf.get(100, 100), 0x7F);
}

#[test]
fn screen_tile_buffer_set_out_of_bounds_ignored() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.set(20, 0, 0xFF);
    buf.set(0, 18, 0xFF);
    for i in 0..SCREEN_TILE_COUNT {
        assert_eq!(buf.tiles[i], 0x7F);
    }
}

#[test]
fn screen_tile_buffer_fill() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.fill(0xAB);
    for i in 0..SCREEN_TILE_COUNT {
        assert_eq!(buf.tiles[i], 0xAB);
    }
}

#[test]
fn screen_tile_buffer_set_row() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let row = [1, 2, 3, 4, 5];
    buf.set_row(2, &row);
    assert_eq!(buf.get(0, 2), 1);
    assert_eq!(buf.get(4, 2), 5);
    assert_eq!(buf.get(5, 2), 0x7F);
    assert_eq!(buf.get(0, 1), 0x7F);
}

#[test]
fn screen_tile_buffer_set_row_full_width() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let row: Vec<u8> = (0..20).collect();
    buf.set_row(0, &row);
    for i in 0..20u32 {
        assert_eq!(buf.get(i, 0), i as u8);
    }
}

#[test]
fn screen_tile_buffer_set_row_out_of_bounds() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.set_row(18, &[1, 2, 3]);
    for i in 0..SCREEN_TILE_COUNT {
        assert_eq!(buf.tiles[i], 0x7F);
    }
}

#[test]
fn screen_tile_buffer_copy_from_flat() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let mut data = vec![0xAA; SCREEN_TILE_COUNT];
    data[0] = 0x01;
    data[359] = 0x02;
    buf.copy_from_flat(&data);
    assert_eq!(buf.get(0, 0), 0x01);
    assert_eq!(buf.get(19, 17), 0x02);
    assert_eq!(buf.get(1, 0), 0xAA);
}

#[test]
fn screen_tile_buffer_copy_from_flat_partial() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.copy_from_flat(&[0x10, 0x20, 0x30]);
    assert_eq!(buf.get(0, 0), 0x10);
    assert_eq!(buf.get(1, 0), 0x20);
    assert_eq!(buf.get(2, 0), 0x30);
    assert_eq!(buf.get(3, 0), 0x7F);
}

#[test]
fn screen_tile_buffer_render_pixel_check() {
    let ts = TileSet::blank(256);
    let buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::rgb(0xFF, 0, 0));
    buf.render(&mut fb, &ts, &GRAYSCALE_PALETTE);
    assert_eq!(fb.get_pixel(0, 0), Some(Rgba::rgb(0xFF, 0xFF, 0xFF)));
}

#[test]
fn screen_tile_buffer_render_region() {
    let ts = TileSet::blank(256);
    let buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::rgb(0xFF, 0, 0));
    buf.render_region(&mut fb, &ts, &GRAYSCALE_PALETTE, 0, 0, 1, 1);
    assert_eq!(fb.get_pixel(0, 0), Some(Rgba::rgb(0xFF, 0xFF, 0xFF)));
    assert_eq!(fb.get_pixel(8, 0), Some(Rgba::rgb(0xFF, 0, 0)));
}

#[test]
fn write_tiles_at_basic() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    write_tiles_at(&mut buf, 3, 5, &[0x80, 0x81, 0x82]);
    assert_eq!(buf.get(3, 5), 0x80);
    assert_eq!(buf.get(4, 5), 0x81);
    assert_eq!(buf.get(5, 5), 0x82);
    assert_eq!(buf.get(6, 5), 0x7F);
    assert_eq!(buf.get(2, 5), 0x7F);
}

#[test]
fn write_tiles_at_clips_to_screen_width() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    write_tiles_at(&mut buf, 18, 0, &[0xA0, 0xA1, 0xA2, 0xA3]);
    assert_eq!(buf.get(18, 0), 0xA0);
    assert_eq!(buf.get(19, 0), 0xA1);
}

#[test]
fn textbox_standard_dialog() {
    let f = TextBoxFrame::standard_dialog();
    assert_eq!(f.x, 0);
    assert_eq!(f.y, 12);
    assert_eq!(f.width, 20);
    assert_eq!(f.height, 6);
}

#[test]
fn textbox_text_start() {
    let f = TextBoxFrame::standard_dialog();
    assert_eq!(f.text_start(), (1, 14));
}

#[test]
fn textbox_second_line_start() {
    let f = TextBoxFrame::standard_dialog();
    assert_eq!(f.second_line_start(), (1, 16));
}

#[test]
fn textbox_draw_frame_corners() {
    let f = TextBoxFrame::standard_dialog();
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    f.draw_frame(&mut buf);
    assert_eq!(buf.get(0, 12), TILE_TOP_LEFT);
    assert_eq!(buf.get(19, 12), TILE_TOP_RIGHT);
    assert_eq!(buf.get(0, 17), TILE_BOTTOM_LEFT);
    assert_eq!(buf.get(19, 17), TILE_BOTTOM_RIGHT);
}

#[test]
fn textbox_draw_frame_horizontal_edges() {
    let f = TextBoxFrame::standard_dialog();
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    f.draw_frame(&mut buf);
    for tx in 1..19 {
        assert_eq!(buf.get(tx, 12), TILE_HORIZONTAL, "top edge at tx={tx}");
        assert_eq!(buf.get(tx, 17), TILE_HORIZONTAL, "bottom edge at tx={tx}");
    }
}

#[test]
fn textbox_draw_frame_vertical_edges() {
    let f = TextBoxFrame::standard_dialog();
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    f.draw_frame(&mut buf);
    for ty in 13..17 {
        assert_eq!(buf.get(0, ty), TILE_VERTICAL, "left edge at ty={ty}");
        assert_eq!(buf.get(19, ty), TILE_VERTICAL, "right edge at ty={ty}");
    }
}

#[test]
fn textbox_draw_frame_interior_is_space() {
    let f = TextBoxFrame::standard_dialog();
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.fill(0x00);
    f.draw_frame(&mut buf);
    for ty in 13..17 {
        for tx in 1..19 {
            assert_eq!(buf.get(tx, ty), TILE_SPACE, "interior at ({tx},{ty})");
        }
    }
}

#[test]
fn textbox_clear_inner() {
    let f = TextBoxFrame::standard_dialog();
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    f.draw_frame(&mut buf);
    buf.set(5, 14, 0x42);
    buf.set(10, 15, 0x43);
    f.clear_inner(&mut buf);
    assert_eq!(buf.get(5, 14), TILE_SPACE);
    assert_eq!(buf.get(10, 15), TILE_SPACE);
    assert_eq!(buf.get(0, 12), TILE_TOP_LEFT);
    assert_eq!(buf.get(19, 17), TILE_BOTTOM_RIGHT);
}

#[test]
fn textbox_clear_all() {
    let f = TextBoxFrame::standard_dialog();
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    f.draw_frame(&mut buf);
    f.clear(&mut buf);
    for ty in 12..18 {
        for tx in 0..20 {
            assert_eq!(buf.get(tx, ty), TILE_SPACE, "at ({tx},{ty})");
        }
    }
}

#[test]
fn textbox_show_hide_down_arrow() {
    let f = TextBoxFrame::standard_dialog();
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    f.draw_frame(&mut buf);
    f.show_down_arrow(&mut buf);
    assert_eq!(buf.get(18, 16), TILE_DOWN_ARROW);
    f.hide_down_arrow(&mut buf);
    assert_eq!(buf.get(18, 16), TILE_SPACE);
}

#[test]
fn textbox_small_box() {
    let f = TextBoxFrame::new(5, 2, 6, 4);
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    f.draw_frame(&mut buf);
    assert_eq!(buf.get(5, 2), TILE_TOP_LEFT);
    assert_eq!(buf.get(10, 2), TILE_TOP_RIGHT);
    assert_eq!(buf.get(5, 5), TILE_BOTTOM_LEFT);
    assert_eq!(buf.get(10, 5), TILE_BOTTOM_RIGHT);
    for tx in 6..10 {
        assert_eq!(buf.get(tx, 2), TILE_HORIZONTAL);
        assert_eq!(buf.get(tx, 5), TILE_HORIZONTAL);
    }
    for ty in 3..5 {
        assert_eq!(buf.get(5, ty), TILE_VERTICAL);
        assert_eq!(buf.get(10, ty), TILE_VERTICAL);
    }
}

#[test]
fn textbox_minimum_size() {
    let f = TextBoxFrame::new(0, 0, 2, 2);
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.fill(0x00);
    f.draw_frame(&mut buf);
    assert_eq!(buf.get(0, 0), TILE_TOP_LEFT);
    assert_eq!(buf.get(1, 0), TILE_TOP_RIGHT);
    assert_eq!(buf.get(0, 1), TILE_BOTTOM_LEFT);
    assert_eq!(buf.get(1, 1), TILE_BOTTOM_RIGHT);
}

#[test]
fn textbox_too_small_noop() {
    let f = TextBoxFrame::new(0, 0, 1, 1);
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.fill(0x00);
    f.draw_frame(&mut buf);
    assert_eq!(buf.get(0, 0), 0x00);
}

#[test]
fn screen_tile_constants() {
    assert_eq!(SCREEN_TILES_X, 20);
    assert_eq!(SCREEN_TILES_Y, 18);
    assert_eq!(SCREEN_TILE_COUNT, 360);
}

// ===== M5.5 Menu Rendering Tests =====
use crate::menu::*;

#[test]
fn menu_cursor_new_defaults() {
    let c = MenuCursor::new(5, 3);
    assert_eq!(c.x, 5);
    assert_eq!(c.y, 3);
    assert_eq!(c.tile_behind, TILE_SPACE);
    assert!(!c.visible);
}

#[test]
fn menu_cursor_place_saves_tile() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.set(2, 4, 0x42);
    let mut c = MenuCursor::new(2, 4);
    c.place(&mut buf);
    assert_eq!(c.tile_behind, 0x42);
    assert_eq!(buf.get(2, 4), TILE_CURSOR_FILLED);
    assert!(c.visible);
}

#[test]
fn menu_cursor_place_does_not_save_cursor_tile() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.set(2, 4, TILE_CURSOR_FILLED);
    let mut c = MenuCursor::new(2, 4);
    c.place(&mut buf);
    assert_eq!(c.tile_behind, TILE_SPACE);
}

#[test]
fn menu_cursor_erase_writes_space() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let mut c = MenuCursor::new(3, 5);
    c.place(&mut buf);
    c.erase(&mut buf);
    assert_eq!(buf.get(3, 5), TILE_SPACE);
}

#[test]
fn menu_cursor_place_unfilled() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let c = MenuCursor::new(1, 1);
    c.place_unfilled(&mut buf);
    assert_eq!(buf.get(1, 1), TILE_CURSOR_UNFILLED);
}

#[test]
fn menu_cursor_restore() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.set(4, 6, 0xAA);
    let mut c = MenuCursor::new(4, 6);
    c.place(&mut buf);
    assert_eq!(buf.get(4, 6), TILE_CURSOR_FILLED);
    c.restore(&mut buf);
    assert_eq!(buf.get(4, 6), 0xAA);
}

#[test]
fn menu_cursor_move_to() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.set(2, 3, 0x11);
    buf.set(5, 7, 0x22);
    let mut c = MenuCursor::new(2, 3);
    c.place(&mut buf);
    c.move_to(&mut buf, 5, 7);
    assert_eq!(buf.get(2, 3), 0x11);
    assert_eq!(buf.get(5, 7), TILE_CURSOR_FILLED);
    assert_eq!(c.tile_behind, 0x22);
}

#[test]
fn menu_layout_new_defaults() {
    let frame = TextBoxFrame::new(10, 0, 10, 16);
    let ml = MenuLayout::new(frame, 12, 2, 7);
    assert_eq!(ml.item_count, 7);
    assert!(ml.double_spaced);
    assert_eq!(ml.current_item, 0);
    assert!(!ml.wrapping);
}

#[test]
fn menu_layout_cursor_positions() {
    let frame = TextBoxFrame::new(10, 0, 10, 16);
    let ml = MenuLayout::new(frame, 12, 2, 7);
    assert_eq!(ml.cursor_x(), 11);
    assert_eq!(ml.cursor_y_for_item(0), 2);
    assert_eq!(ml.cursor_y_for_item(1), 4);
    assert_eq!(ml.cursor_y_for_item(2), 6);
    assert_eq!(ml.cursor_y_for_item(3), 8);
}

#[test]
fn menu_layout_single_spaced() {
    let frame = TextBoxFrame::new(0, 0, 10, 10);
    let mut ml = MenuLayout::new(frame, 2, 1, 5);
    ml.double_spaced = false;
    assert_eq!(ml.cursor_y_for_item(0), 1);
    assert_eq!(ml.cursor_y_for_item(1), 2);
    assert_eq!(ml.cursor_y_for_item(4), 5);
}

#[test]
fn menu_layout_item_position() {
    let frame = TextBoxFrame::new(10, 0, 10, 16);
    let ml = MenuLayout::new(frame, 12, 2, 7);
    assert_eq!(ml.item_position(0), (12, 2));
    assert_eq!(ml.item_position(3), (12, 8));
}

#[test]
fn menu_layout_place_cursor() {
    let frame = TextBoxFrame::new(10, 0, 10, 16);
    let ml = MenuLayout::new(frame, 12, 2, 7);
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let cursor = ml.place_cursor(&mut buf);
    assert_eq!(buf.get(11, 2), TILE_CURSOR_FILLED);
    assert!(cursor.visible);
}

#[test]
fn menu_layout_move_cursor_down_up() {
    let frame = TextBoxFrame::new(10, 0, 10, 16);
    let mut ml = MenuLayout::new(frame, 12, 2, 3);
    assert!(ml.move_cursor_down());
    assert_eq!(ml.current_item, 1);
    assert!(ml.move_cursor_down());
    assert_eq!(ml.current_item, 2);
    assert!(!ml.move_cursor_down());
    assert_eq!(ml.current_item, 2);
    assert!(ml.move_cursor_up());
    assert_eq!(ml.current_item, 1);
    assert!(ml.move_cursor_up());
    assert_eq!(ml.current_item, 0);
    assert!(!ml.move_cursor_up());
    assert_eq!(ml.current_item, 0);
}

#[test]
fn menu_layout_wrapping() {
    let frame = TextBoxFrame::new(0, 0, 10, 10);
    let mut ml = MenuLayout::new(frame, 2, 1, 3);
    ml.wrapping = true;
    ml.current_item = 0;
    assert!(ml.move_cursor_up());
    assert_eq!(ml.current_item, 2);
    assert!(ml.move_cursor_down());
    assert_eq!(ml.current_item, 0);
    ml.current_item = 2;
    assert!(ml.move_cursor_down());
    assert_eq!(ml.current_item, 0);
}

#[test]
fn menu_layout_place_item_text() {
    let frame = TextBoxFrame::new(10, 0, 10, 16);
    let ml = MenuLayout::new(frame, 12, 2, 7);
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let tiles = encode_menu_str("ITEM");
    ml.place_item_text(&mut buf, 0, &tiles);
    assert_eq!(buf.get(12, 2), tiles[0]);
    assert_eq!(buf.get(13, 2), tiles[1]);
    assert_eq!(buf.get(14, 2), tiles[2]);
    assert_eq!(buf.get(15, 2), tiles[3]);
}

#[test]
fn start_menu_draw_with_pokedex() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let ml = StartMenuRenderer::draw(&mut buf, true);
    assert_eq!(ml.frame.x, 10);
    assert_eq!(ml.frame.y, 0);
    assert_eq!(ml.frame.width, 10);
    assert_eq!(ml.frame.height, 16);
    assert_eq!(ml.item_count, 7);
    assert_eq!(ml.top_item_x, 12);
    assert_eq!(ml.top_item_y, 2);
    assert_eq!(buf.get(10, 0), TILE_TOP_LEFT);
    assert_eq!(buf.get(19, 0), TILE_TOP_RIGHT);
    let pokedex_tiles = encode_menu_str("POK");
    assert_eq!(buf.get(12, 2), pokedex_tiles[0]);
}

#[test]
fn start_menu_draw_without_pokedex() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let ml = StartMenuRenderer::draw(&mut buf, false);
    assert_eq!(ml.frame.height, 14);
    assert_eq!(ml.item_count, 6);
    let pokemon_tiles = encode_menu_str("POK");
    assert_eq!(buf.get(12, 2), pokemon_tiles[0]);
}

#[test]
fn start_menu_draw_with_player_name() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let name = encode_menu_str("RED");
    let ml = StartMenuRenderer::draw_with_player_name(&mut buf, true, &name, false);
    assert_eq!(ml.frame.height, 16);
    assert_eq!(ml.item_count, 7);
    assert_eq!(buf.get(12, 8), name[0]);
    assert_eq!(buf.get(13, 8), name[1]);
    assert_eq!(buf.get(14, 8), name[2]);
}

#[test]
fn start_menu_link_mode_shows_reset() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let name = encode_menu_str("ASH");
    StartMenuRenderer::draw_with_player_name(&mut buf, true, &name, true);
    let reset_tiles = encode_menu_str("RESET");
    assert_eq!(buf.get(12, 10), reset_tiles[0]);
}

#[test]
fn encode_menu_str_basic() {
    let tiles = encode_menu_str("ABC");
    assert_eq!(tiles.len(), 3);
    assert_eq!(tiles[0], 0x80);
    assert_eq!(tiles[1], 0x81);
    assert_eq!(tiles[2], 0x82);
}

#[test]
fn textbox_template_message_box_coords() {
    let (x1, y1, x2, y2) = TextBoxTemplateId::MessageBox.coords();
    assert_eq!((x1, y1, x2, y2), (0, 12, 19, 17));
}

#[test]
fn textbox_template_to_frame() {
    let f = TextBoxTemplateId::MessageBox.to_frame();
    assert_eq!(f.x, 0);
    assert_eq!(f.y, 12);
    assert_eq!(f.width, 20);
    assert_eq!(f.height, 6);
}

#[test]
fn textbox_template_battle_menu_draw_with_text() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    TextBoxTemplateId::BattleMenu.draw_with_text(&mut buf);
    assert_eq!(buf.get(8, 12), TILE_TOP_LEFT);
    assert_eq!(buf.get(19, 17), TILE_BOTTOM_RIGHT);
    let fight = encode_menu_str("FIGHT");
    assert_eq!(buf.get(10, 14), fight[0]);
    let run = encode_menu_str("RUN");
    assert_eq!(buf.get(16, 16), run[0]);
}

#[test]
fn textbox_template_buy_sell_quit() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    TextBoxTemplateId::BuySellQuitMenu.draw_with_text(&mut buf);
    assert_eq!(buf.get(0, 0), TILE_TOP_LEFT);
    assert_eq!(buf.get(10, 6), TILE_BOTTOM_RIGHT);
    let buy = encode_menu_str("BUY");
    assert_eq!(buf.get(2, 1), buy[0]);
    let sell = encode_menu_str("SELL");
    assert_eq!(buf.get(2, 3), sell[0]);
    let quit = encode_menu_str("QUIT");
    assert_eq!(buf.get(2, 5), quit[0]);
}

#[test]
fn textbox_template_list_menu_box() {
    let f = TextBoxTemplateId::ListMenuBox.to_frame();
    assert_eq!(f.x, 4);
    assert_eq!(f.y, 2);
    assert_eq!(f.width, 16);
    assert_eq!(f.height, 11);
}

#[test]
fn textbox_template_money_box() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    TextBoxTemplateId::MoneyBox.draw_with_text(&mut buf);
    let money = encode_menu_str("MONEY");
    assert_eq!(buf.get(13, 0), money[0]);
}

#[test]
fn two_option_menu_yes_no() {
    let m = TwoOptionMenu::yes_no(10, 5);
    assert_eq!(m.frame.x, 10);
    assert_eq!(m.frame.y, 5);
    assert_eq!(m.frame.width, 6);
    assert_eq!(m.frame.height, 5);
    assert_eq!(m.text_x, 12);
    assert_eq!(m.text_y, 6);
    assert_eq!(m.current_item, 0);
}

#[test]
fn two_option_menu_draw() {
    let m = TwoOptionMenu::yes_no(10, 5);
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    m.draw(&mut buf);
    assert_eq!(buf.get(10, 5), TILE_TOP_LEFT);
    assert_eq!(buf.get(15, 9), TILE_BOTTOM_RIGHT);
    let yes = encode_menu_str("YES");
    assert_eq!(buf.get(12, 6), yes[0]);
    let no = encode_menu_str("NO");
    assert_eq!(buf.get(12, 8), no[0]);
}

#[test]
fn two_option_menu_cursor_position() {
    let mut m = TwoOptionMenu::yes_no(10, 5);
    assert_eq!(m.cursor_position(), (11, 6));
    m.current_item = 1;
    assert_eq!(m.cursor_position(), (11, 8));
}

#[test]
fn two_option_menu_save_restore_area() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    for y in 5..10 {
        for x in 10..16 {
            buf.set(x, y, (x + y * 20) as u8);
        }
    }
    let mut m = TwoOptionMenu::yes_no(10, 5);
    m.save_area(&buf);
    m.draw(&mut buf);
    assert_eq!(buf.get(10, 5), TILE_TOP_LEFT);
    m.restore_area(&mut buf);
    assert_eq!(buf.get(10, 5), (10 + 5 * 20) as u8);
    assert_eq!(buf.get(15, 9), (15 + 9 * 20) as u8);
}

#[test]
fn two_option_menu_custom() {
    let frame = TextBoxFrame::new(5, 3, 8, 5);
    let m = TwoOptionMenu::custom(frame, "TRADE", "CANCEL");
    assert_eq!(m.text_x, 7);
    assert_eq!(m.text_y, 4);
    let trade = encode_menu_str("TRADE");
    assert_eq!(m.option1_tiles, trade);
    let cancel = encode_menu_str("CANCEL");
    assert_eq!(m.option2_tiles, cancel);
}

#[test]
fn list_menu_renderer_new() {
    let lr = ListMenuRenderer::new(10);
    assert_eq!(lr.frame.x, 4);
    assert_eq!(lr.frame.y, 2);
    assert_eq!(lr.frame.width, 16);
    assert_eq!(lr.frame.height, 11);
    assert_eq!(lr.visible_count, 4);
    assert_eq!(lr.scroll_offset, 0);
    assert_eq!(lr.total_count, 10);
    assert_eq!(lr.current_item, 0);
    assert_eq!(lr.item_x, 6);
    assert_eq!(lr.item_start_y, 4);
    assert_eq!(lr.row_spacing, 2);
}

#[test]
fn list_menu_draw_frame() {
    let lr = ListMenuRenderer::new(5);
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    lr.draw_frame(&mut buf);
    assert_eq!(buf.get(4, 2), TILE_TOP_LEFT);
    assert_eq!(buf.get(19, 12), TILE_BOTTOM_RIGHT);
}

#[test]
fn list_menu_draw_item() {
    let lr = ListMenuRenderer::new(5);
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let item_tiles = encode_menu_str("POTION");
    lr.draw_item(&mut buf, 0, &item_tiles);
    assert_eq!(buf.get(6, 4), item_tiles[0]);
    assert_eq!(buf.get(7, 4), item_tiles[1]);
}

#[test]
fn list_menu_draw_cancel() {
    let lr = ListMenuRenderer::new(3);
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    lr.draw_cancel(&mut buf, 3);
    let cancel = encode_menu_str("CANCEL");
    assert_eq!(buf.get(6, 10), cancel[0]);
}

#[test]
fn list_menu_scroll_indicator() {
    let lr = ListMenuRenderer::new(10);
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    lr.draw_scroll_indicator(&mut buf);
    let y = 4 + (4 - 1) * 2 + 1;
    let x = 4 + 16 - 2;
    assert_eq!(buf.get(x, y), TILE_SCROLL_INDICATOR);
}

#[test]
fn list_menu_no_scroll_indicator_when_all_visible() {
    let lr = ListMenuRenderer::new(3);
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.fill(0x00);
    lr.draw_scroll_indicator(&mut buf);
    let y = 4 + (4 - 1) * 2 + 1;
    let x = 4 + 16 - 2;
    assert_eq!(buf.get(x, y), 0x00);
}

#[test]
fn list_menu_cursor_position() {
    let mut lr = ListMenuRenderer::new(10);
    assert_eq!(lr.cursor_position(), (5, 4));
    lr.current_item = 2;
    assert_eq!(lr.cursor_position(), (5, 8));
}

#[test]
fn list_menu_scroll_down_up() {
    let mut lr = ListMenuRenderer::new(10);
    assert!(lr.scroll_down());
    assert_eq!(lr.scroll_offset, 1);
    assert!(lr.scroll_up());
    assert_eq!(lr.scroll_offset, 0);
    assert!(!lr.scroll_up());
    assert_eq!(lr.scroll_offset, 0);
}

#[test]
fn list_menu_draw_item_quantity() {
    let lr = ListMenuRenderer::new(5);
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.fill(0x00);
    lr.draw_item_quantity(&mut buf, 0, 15);
    let y = 4 + 0 * 2 + 1;
    let x = 6 + 8;
    assert_eq!(buf.get(x, y), 0xF1);
    assert_eq!(buf.get(x + 1, y), 0xF6 + 1);
    assert_eq!(buf.get(x + 2, y), 0xF6 + 5);
}

#[test]
fn party_menu_draw_mon_entry() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let name = encode_menu_str("PIKACHU");
    PartyMenuRenderer::draw_mon_entry(&mut buf, 0, &name);
    assert_eq!(buf.get(3, 0), name[0]);
    assert_eq!(buf.get(4, 0), name[1]);
}

#[test]
fn party_menu_draw_mon_entry_second_slot() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let name = encode_menu_str("CHARIZARD");
    PartyMenuRenderer::draw_mon_entry(&mut buf, 2, &name);
    assert_eq!(buf.get(3, 4), name[0]);
}

#[test]
fn party_menu_draw_level_single_digit() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.fill(0x00);
    PartyMenuRenderer::draw_level(&mut buf, 0, 5);
    assert_eq!(buf.get(13, 0), 0x6E);
    assert_eq!(buf.get(14, 0), 0xF6 + 5);
    assert_eq!(buf.get(15, 0), 0x00);
}

#[test]
fn party_menu_draw_level_two_digits() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    PartyMenuRenderer::draw_level(&mut buf, 0, 42);
    assert_eq!(buf.get(13, 0), 0x6E);
    assert_eq!(buf.get(14, 0), 0xF6 + 4);
    assert_eq!(buf.get(15, 0), 0xF6 + 2);
}

#[test]
fn party_menu_draw_level_100() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    PartyMenuRenderer::draw_level(&mut buf, 0, 100);
    assert_eq!(buf.get(13, 0), 0x6E);
    assert_eq!(buf.get(14, 0), 0xF6 + 1);
    assert_eq!(buf.get(15, 0), 0xF6 + 0);
    assert_eq!(buf.get(16, 0), 0xF6 + 0);
}

#[test]
fn party_menu_draw_status() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let status = encode_menu_str("PSN");
    PartyMenuRenderer::draw_status(&mut buf, 1, &status);
    assert_eq!(buf.get(17, 2), status[0]);
    assert_eq!(buf.get(18, 2), status[1]);
    assert_eq!(buf.get(19, 2), status[2]);
}

#[test]
fn party_menu_hp_bar_position() {
    let (x, y) = PartyMenuRenderer::hp_bar_position(0);
    assert_eq!(x, 7);
    assert_eq!(y, 1);
    let (x2, y2) = PartyMenuRenderer::hp_bar_position(2);
    assert_eq!(x2, 7);
    assert_eq!(y2, 5);
}

#[test]
fn party_menu_draw_hp_bar_full() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    PartyMenuRenderer::draw_hp_bar(&mut buf, 0, 100, 100, 6);
    let (x, y) = PartyMenuRenderer::hp_bar_position(0);
    assert_eq!(buf.get(x, y), 0x71);
    for i in 0..6 {
        assert_eq!(buf.get(x + 1 + i, y), 0x72);
    }
    assert_eq!(buf.get(x + 7, y), 0x6D);
}

#[test]
fn party_menu_draw_hp_bar_empty() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    PartyMenuRenderer::draw_hp_bar(&mut buf, 0, 0, 100, 6);
    let (x, y) = PartyMenuRenderer::hp_bar_position(0);
    assert_eq!(buf.get(x, y), 0x71);
    for i in 0..6 {
        assert_eq!(buf.get(x + 1 + i, y), 0x73);
    }
    assert_eq!(buf.get(x + 7, y), 0x6D);
}

#[test]
fn party_menu_draw_hp_bar_half() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    PartyMenuRenderer::draw_hp_bar(&mut buf, 0, 50, 100, 6);
    let (x, y) = PartyMenuRenderer::hp_bar_position(0);
    assert_eq!(buf.get(x, y), 0x71);
    let filled = 3u32;
    for i in 0..filled {
        assert_eq!(buf.get(x + 1 + i, y), 0x72, "filled at {i}");
    }
    for i in filled..6 {
        assert_eq!(buf.get(x + 1 + i, y), 0x73, "empty at {i}");
    }
}

#[test]
fn party_menu_draw_hp_text() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.fill(0x00);
    PartyMenuRenderer::draw_hp_text(&mut buf, 0, 45, 120, 6);
    let (x, y) = PartyMenuRenderer::hp_bar_position(0);
    let text_x = x + 2 + 6;
    assert_ne!(buf.get(text_x, y), 0x00);
}

#[test]
fn party_menu_draw_unfilled_arrow() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    PartyMenuRenderer::draw_unfilled_arrow(&mut buf, 1);
    assert_eq!(buf.get(0, 2), TILE_CURSOR_UNFILLED);
}

#[test]
fn party_menu_cursor_position() {
    assert_eq!(PartyMenuRenderer::cursor_position(0), (0, 0));
    assert_eq!(PartyMenuRenderer::cursor_position(1), (0, 2));
    assert_eq!(PartyMenuRenderer::cursor_position(5), (0, 10));
}

// ===== M5.6 Battle Scene Rendering Tests =====

use crate::battle_scene::*;

#[test]
fn hp_bar_pixels_full() {
    assert_eq!(calc_hp_bar_pixels(100, 100), 48);
}

#[test]
fn hp_bar_pixels_empty() {
    assert_eq!(calc_hp_bar_pixels(0, 100), 0);
}

#[test]
fn hp_bar_pixels_half() {
    assert_eq!(calc_hp_bar_pixels(50, 100), 24);
}

#[test]
fn hp_bar_pixels_one_hp() {
    // Minimum 1 pixel when HP > 0
    assert_eq!(calc_hp_bar_pixels(1, 999), 1);
}

#[test]
fn hp_bar_pixels_max_zero() {
    assert_eq!(calc_hp_bar_pixels(0, 0), 0);
}

#[test]
fn hp_bar_pixels_overflow_clamped() {
    // current > max should still clamp to 48
    assert_eq!(calc_hp_bar_pixels(200, 100), 48);
}

#[test]
fn pokeball_status_tiles() {
    assert_eq!(PokeballStatus::Normal.tile(), TILE_POKEBALL_NORMAL);
    assert_eq!(PokeballStatus::StatusAilment.tile(), TILE_POKEBALL_STATUS);
    assert_eq!(PokeballStatus::Fainted.tile(), TILE_POKEBALL_FAINTED);
    assert_eq!(PokeballStatus::Empty.tile(), TILE_POKEBALL_EMPTY);
}

#[test]
fn draw_hp_bar_full() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    draw_hp_bar(&mut buf, 0, 0, 100, 100, TILE_HP_END_CAP_BATTLE, false);
    assert_eq!(buf.get(0, 0), TILE_HP_LABEL);
    assert_eq!(buf.get(1, 0), TILE_HP_BAR_LEFT);
    // All 6 tiles should be full
    for i in 0..6 {
        assert_eq!(buf.get(2 + i, 0), TILE_HP_FULL, "tile {} should be full", i);
    }
    assert_eq!(buf.get(8, 0), TILE_HP_END_CAP_BATTLE);
}

#[test]
fn draw_hp_bar_empty() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    draw_hp_bar(&mut buf, 0, 0, 0, 100, TILE_HP_END_CAP_BATTLE, false);
    assert_eq!(buf.get(0, 0), TILE_HP_LABEL);
    assert_eq!(buf.get(1, 0), TILE_HP_BAR_LEFT);
    for i in 0..6 {
        assert_eq!(
            buf.get(2 + i, 0),
            TILE_HP_EMPTY,
            "tile {} should be empty",
            i
        );
    }
    assert_eq!(buf.get(8, 0), TILE_HP_END_CAP_BATTLE);
}

#[test]
fn draw_hp_bar_partial() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    // 12 pixels = 1 full tile (8px) + 1 partial (4px) + 4 empty
    draw_hp_bar(&mut buf, 0, 0, 25, 100, TILE_HP_END_CAP_BATTLE, false);
    let pixels = calc_hp_bar_pixels(25, 100);
    assert_eq!(pixels, 12);
    assert_eq!(buf.get(2, 0), TILE_HP_FULL); // first 8px
    assert_eq!(buf.get(3, 0), TILE_HP_PARTIAL_BASE + 4); // 4px partial
    for i in 2..6 {
        assert_eq!(buf.get(2 + i, 0), TILE_HP_EMPTY);
    }
}

#[test]
fn draw_hp_bar_with_numbers() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    draw_hp_bar(&mut buf, 0, 0, 45, 120, TILE_HP_END_CAP_BATTLE, true);
    // HP numbers now placed at (x+1, y+1) per DrawHP_ in status_screen.asm
    // With x=0, y=0: numbers at (1, 1) → " 45/120"
    let num_start = 1u32;
    let num_y = 1u32;
    assert_eq!(buf.get(num_start, num_y), TILE_SPACE); // leading space for " 45"
    assert_eq!(buf.get(num_start + 1, num_y), 0xF6 + 4); // '4'
    assert_eq!(buf.get(num_start + 2, num_y), 0xF6 + 5); // '5'
    assert_eq!(buf.get(num_start + 3, num_y), 0xF3); // '/'
    assert_eq!(buf.get(num_start + 4, num_y), 0xF6 + 1); // '1'
    assert_eq!(buf.get(num_start + 5, num_y), 0xF6 + 2); // '2'
    assert_eq!(buf.get(num_start + 6, num_y), 0xF6 + 0); // '0'
}

#[test]
fn draw_level_single_digit() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    draw_level(&mut buf, 0, 0, 5);
    assert_eq!(buf.get(0, 0), 0x6E); // Lv tile
    assert_eq!(buf.get(1, 0), 0xF6 + 5); // '5'
}

#[test]
fn draw_level_two_digits() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    draw_level(&mut buf, 0, 0, 50);
    assert_eq!(buf.get(0, 0), 0x6E);
    assert_eq!(buf.get(1, 0), 0xF6 + 5); // '5'
    assert_eq!(buf.get(2, 0), 0xF6 + 0); // '0'
}

#[test]
fn draw_level_100() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    draw_level(&mut buf, 0, 0, 100);
    assert_eq!(buf.get(0, 0), 0x6E);
    assert_eq!(buf.get(1, 0), 0xF6 + 1); // '1'
    assert_eq!(buf.get(2, 0), 0xF6 + 0); // '0'
    assert_eq!(buf.get(3, 0), 0xF6 + 0); // '0'
}

#[test]
fn player_hud_clear() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.fill(0xFF);
    PlayerHud::clear(&mut buf);
    for row in 0..PlayerHud::CLEAR_H {
        for col in 0..PlayerHud::CLEAR_W {
            assert_eq!(
                buf.get(PlayerHud::CLEAR_X + col, PlayerHud::CLEAR_Y + row),
                TILE_SPACE,
                "({},{}) should be space",
                PlayerHud::CLEAR_X + col,
                PlayerHud::CLEAR_Y + row
            );
        }
    }
}

#[test]
fn player_hud_border() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    PlayerHud::draw_border(&mut buf);
    assert_eq!(buf.get(18, 9), TILE_HUD_CONNECTOR);
    assert_eq!(buf.get(18, 10), TILE_HUD_CONNECTOR);
    assert_eq!(buf.get(18, 11), TILE_HUD_PLAYER_CORNER);
    for i in 0..8 {
        assert_eq!(
            buf.get(17 - i, 11),
            TILE_HUD_HORIZONTAL,
            "col {} should be horizontal",
            17 - i
        );
    }
    assert_eq!(buf.get(9, 11), TILE_HUD_PLAYER_TRIANGLE);
}

#[test]
fn player_hud_draw_full() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let name = [0x80, 0x81, 0x82]; // "ABC" encoded (3 chars → +1 centering offset)
    PlayerHud::draw(&mut buf, &name, 25, None, 50, 100);
    // Name at (10+1, 7) due to CenterMonName (3 chars → +1)
    assert_eq!(buf.get(11, 7), 0x80);
    assert_eq!(buf.get(12, 7), 0x81);
    assert_eq!(buf.get(13, 7), 0x82);
    // Level at (14, 8): Lv25
    assert_eq!(buf.get(14, 8), 0x6E); // Lv
    assert_eq!(buf.get(15, 8), 0xF6 + 2); // '2'
    assert_eq!(buf.get(16, 8), 0xF6 + 5); // '5'
                                          // HP bar starts at (10, 9)
    assert_eq!(buf.get(10, 9), TILE_HP_LABEL);
    assert_eq!(buf.get(11, 9), TILE_HP_BAR_LEFT);
    // Border
    assert_eq!(buf.get(18, 11), TILE_HUD_PLAYER_CORNER);
}

#[test]
fn enemy_hud_clear() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.fill(0xFF);
    EnemyHud::clear(&mut buf);
    for row in 0..EnemyHud::CLEAR_H {
        for col in 0..EnemyHud::CLEAR_W {
            assert_eq!(
                buf.get(EnemyHud::CLEAR_X + col, EnemyHud::CLEAR_Y + row),
                TILE_SPACE,
            );
        }
    }
}

#[test]
fn enemy_hud_border() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    EnemyHud::draw_border(&mut buf);
    assert_eq!(buf.get(1, 2), TILE_HUD_CONNECTOR);
    assert_eq!(buf.get(1, 3), TILE_HUD_ENEMY_CORNER);
    for i in 0..8 {
        assert_eq!(buf.get(2 + i, 3), TILE_HUD_HORIZONTAL);
    }
    assert_eq!(buf.get(10, 3), TILE_HUD_ENEMY_TRIANGLE);
}

#[test]
fn enemy_hud_draw_no_hp_numbers() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.fill(0x00);
    let name = [0x80, 0x81]; // "AB"
    EnemyHud::draw(&mut buf, &name, 10, None, 30, 60);
    // HP bar at (2, 2) — should NOT have numbers after end cap
    // End cap at col 2+2+6 = 10
    assert_eq!(buf.get(10, 2), TILE_HP_END_CAP_ENEMY);
    // HP bar label and border corner
    // Border draws: (1,2)=connector, (1,3)=corner, (2..9,3)=horizontal, (10,3)=triangle
    // HP bar draws after border: (2,2)=HP_LABEL, (3,2)=BAR_LEFT, (4..9,2)=bars, (10,2)=END_CAP
    assert_eq!(buf.get(2, 2), TILE_HP_LABEL);
    assert_eq!(buf.get(1, 3), TILE_HUD_ENEMY_CORNER);
}

#[test]
fn pokeball_indicators_player() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let party = [
        PokeballStatus::Normal,
        PokeballStatus::StatusAilment,
        PokeballStatus::Fainted,
        PokeballStatus::Normal,
        PokeballStatus::Empty,
        PokeballStatus::Empty,
    ];
    PokeballIndicators::draw_player(&mut buf, &party);
    assert_eq!(buf.get(10, 11), TILE_POKEBALL_NORMAL);
    assert_eq!(buf.get(11, 11), TILE_POKEBALL_STATUS);
    assert_eq!(buf.get(12, 11), TILE_POKEBALL_FAINTED);
    assert_eq!(buf.get(13, 11), TILE_POKEBALL_NORMAL);
    assert_eq!(buf.get(14, 11), TILE_POKEBALL_EMPTY);
    assert_eq!(buf.get(15, 11), TILE_POKEBALL_EMPTY);
}

#[test]
fn pokeball_indicators_enemy() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let party = [
        PokeballStatus::Normal,
        PokeballStatus::Normal,
        PokeballStatus::Fainted,
        PokeballStatus::Empty,
        PokeballStatus::Empty,
        PokeballStatus::Empty,
    ];
    PokeballIndicators::draw_enemy(&mut buf, &party);
    assert_eq!(buf.get(7, 4), TILE_POKEBALL_NORMAL);
    assert_eq!(buf.get(6, 4), TILE_POKEBALL_NORMAL);
    assert_eq!(buf.get(5, 4), TILE_POKEBALL_FAINTED);
    assert_eq!(buf.get(4, 4), TILE_POKEBALL_EMPTY);
    assert_eq!(buf.get(3, 4), TILE_POKEBALL_EMPTY);
    assert_eq!(buf.get(2, 4), TILE_POKEBALL_EMPTY);
}

#[test]
fn pokeball_indicators_partial_party() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    let party = [
        PokeballStatus::Normal,
        PokeballStatus::Normal,
        PokeballStatus::Normal,
    ];
    PokeballIndicators::draw_player(&mut buf, &party);
    assert_eq!(buf.get(10, 11), TILE_POKEBALL_NORMAL);
    assert_eq!(buf.get(11, 11), TILE_POKEBALL_NORMAL);
    assert_eq!(buf.get(12, 11), TILE_POKEBALL_NORMAL);
    assert_eq!(buf.get(13, 11), TILE_POKEBALL_EMPTY);
    assert_eq!(buf.get(14, 11), TILE_POKEBALL_EMPTY);
    assert_eq!(buf.get(15, 11), TILE_POKEBALL_EMPTY);
}

#[test]
fn retreat_stage_layout() {
    assert_eq!(RetreatStage::Full.layout(), (1, 5, 7, 7));
    assert_eq!(RetreatStage::Medium.layout(), (3, 7, 5, 5));
    assert_eq!(RetreatStage::Small.layout(), (4, 9, 3, 3));
    assert_eq!(RetreatStage::Pokeball.layout(), (5, 11, 1, 1));
}

#[test]
fn retreat_stage_next() {
    assert_eq!(RetreatStage::Full.next(), Some(RetreatStage::Medium));
    assert_eq!(RetreatStage::Medium.next(), Some(RetreatStage::Small));
    assert_eq!(RetreatStage::Small.next(), Some(RetreatStage::Pokeball));
    assert_eq!(RetreatStage::Pokeball.next(), None);
}

#[test]
fn draw_retreat_stage_pokeball() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.fill(0xFF);
    draw_retreat_stage(&mut buf, RetreatStage::Pokeball);
    // 7×7 area should be cleared
    for row in 0..7 {
        for col in 0..7 {
            if col == 4 && row == 6 {
                // (5, 11) = (1+4, 5+6) = pokeball tile
                assert_eq!(buf.get(1 + col, 5 + row), TILE_POKEBALL_RETREATED);
            } else {
                assert_eq!(buf.get(1 + col, 5 + row), TILE_SPACE);
            }
        }
    }
}

#[test]
fn draw_retreat_stage_full_clears_area() {
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.fill(0xFF);
    draw_retreat_stage(&mut buf, RetreatStage::Full);
    // Full stage clears the 7×7 area but no pokeball
    for row in 0..7 {
        for col in 0..7 {
            assert_eq!(buf.get(1 + col, 5 + row), TILE_SPACE);
        }
    }
}

#[test]
fn scale_sprite_2x_basic() {
    // 2×2 source → 4×4 output
    let src = vec![1, 2, 3, 4];
    let result = scale_sprite_2x(&src, 2, 2);
    assert_eq!(result.len(), 16);
    // Row 0: 1,1,2,2
    assert_eq!(&result[0..4], &[1, 1, 2, 2]);
    // Row 1: 1,1,2,2 (duplicate of row 0)
    assert_eq!(&result[4..8], &[1, 1, 2, 2]);
    // Row 2: 3,3,4,4
    assert_eq!(&result[8..12], &[3, 3, 4, 4]);
    // Row 3: 3,3,4,4
    assert_eq!(&result[12..16], &[3, 3, 4, 4]);
}

#[test]
fn scale_sprite_2x_single_pixel() {
    let src = vec![42];
    let result = scale_sprite_2x(&src, 1, 1);
    assert_eq!(result, vec![42, 42, 42, 42]);
}

#[test]
fn trainer_pic_scroll_new() {
    let scroll = TrainerPicScroll::new(SCREEN_TILES_X);
    assert!(scroll.is_scrolling());
    assert_eq!(scroll.columns_remaining, 7);
    // When 0 columns visible, start_x = 20 - 0 = 20
    assert_eq!(scroll.visible_start_x(), 20);
}

#[test]
fn trainer_pic_scroll_step() {
    let mut scroll = TrainerPicScroll::new(SCREEN_TILES_X);
    let visible = scroll.step();
    assert_eq!(visible, 1);
    assert_eq!(scroll.columns_remaining, 6);
    assert!(scroll.is_scrolling());
    // visible_start_x = 20 - 1 = 19
    assert_eq!(scroll.visible_start_x(), 19);
}

#[test]
fn trainer_pic_scroll_complete() {
    let mut scroll = TrainerPicScroll::new(SCREEN_TILES_X);
    for _ in 0..7 {
        scroll.step();
    }
    assert!(!scroll.is_scrolling());
    assert_eq!(scroll.columns_remaining, 0);
    // visible_start_x = 20 - 7 = 13
    assert_eq!(scroll.visible_start_x(), 13);
}

#[test]
fn trainer_pic_scroll_extra_step_no_panic() {
    let mut scroll = TrainerPicScroll::new(SCREEN_TILES_X);
    for _ in 0..10 {
        scroll.step();
    }
    assert!(!scroll.is_scrolling());
    assert_eq!(scroll.columns_remaining, 0);
}

#[test]
fn trainer_pic_scroll_draw_visible() {
    let mut scroll = TrainerPicScroll::new(SCREEN_TILES_X);
    // 7×7 pic tiles (row-major)
    let mut pic_tiles = vec![0u8; 49];
    for i in 0..49 {
        pic_tiles[i] = i as u8 + 1;
    }
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);

    // Step once: 1 column visible at x=19
    scroll.step();
    scroll.draw_visible(&mut buf, &pic_tiles);
    // First column of each row should be drawn at x=19
    for row in 0..7 {
        let expected_tile = (row * 7 + 0) as u8 + 1; // col 0 of each row
        assert_eq!(buf.get(19, row as u32), expected_tile);
    }

    // Step 6 more times: fully visible, starting at x=13
    for _ in 0..6 {
        scroll.step();
    }
    buf.fill(0x00);
    scroll.draw_visible(&mut buf, &pic_tiles);
    // All 7 columns visible at x=13..19
    for row in 0..7 {
        for col in 0..7 {
            let expected_tile = (row * 7 + col) as u8 + 1;
            assert_eq!(buf.get(13 + col as u32, row as u32), expected_tile);
        }
    }
}

#[test]
fn battle_scene_new_defaults() {
    let scene = BattleScene::new();
    assert!(!scene.show_player_hud);
    assert!(!scene.show_enemy_hud);
    assert_eq!(scene.player_party, [PokeballStatus::Empty; 6]);
    assert_eq!(scene.enemy_party, [PokeballStatus::Empty; 6]);
    assert!(scene.retreat_stage.is_none());
    assert!(scene.trainer_scroll.is_none());
}

#[test]
fn battle_scene_draw_huds_hidden() {
    let scene = BattleScene::new();
    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    buf.fill(0xFF);
    scene.draw(
        &mut buf,
        &[0x80],
        25,
        None,
        50,
        100,
        &[0x81],
        10,
        None,
        30,
        60,
    );
    // HUDs are hidden, so nothing should be drawn at HUD positions
    // Player HUD area should still be 0xFF
    assert_eq!(buf.get(PlayerHud::NAME_X, PlayerHud::NAME_Y), 0xFF);
    assert_eq!(buf.get(EnemyHud::NAME_X, EnemyHud::NAME_Y), 0xFF);
}

#[test]
fn battle_scene_draw_huds_visible() {
    let mut scene = BattleScene::new();
    scene.show_player_hud = true;
    scene.show_enemy_hud = true;
    scene.player_party = [
        PokeballStatus::Normal,
        PokeballStatus::Normal,
        PokeballStatus::Normal,
        PokeballStatus::Empty,
        PokeballStatus::Empty,
        PokeballStatus::Empty,
    ];
    scene.enemy_party = [
        PokeballStatus::Normal,
        PokeballStatus::Fainted,
        PokeballStatus::Empty,
        PokeballStatus::Empty,
        PokeballStatus::Empty,
        PokeballStatus::Empty,
    ];

    let mut buf = ScreenTileBuffer::new(SCREEN_TILES_X, SCREEN_TILES_Y);
    scene.draw(
        &mut buf,
        &[0x80, 0x81, 0x82],
        25,
        None,
        50,
        100,
        &[0x90, 0x91],
        10,
        None,
        30,
        60,
    );
    // Player HUD: "ABC" (3 chars → +1 centering offset)
    assert_eq!(buf.get(PlayerHud::NAME_X + 1, PlayerHud::NAME_Y), 0x80);
    assert_eq!(
        buf.get(PlayerHud::HP_BAR_X, PlayerHud::HP_BAR_Y),
        TILE_HP_LABEL
    );
    // Enemy HUD: 2-char name → +2 centering offset
    assert_eq!(buf.get(EnemyHud::NAME_X + 2, EnemyHud::NAME_Y), 0x90);
    // Pokeball indicators
    assert_eq!(buf.get(10, 11), TILE_POKEBALL_NORMAL); // player
    assert_eq!(buf.get(7, 4), TILE_POKEBALL_NORMAL); // enemy first
    assert_eq!(buf.get(6, 4), TILE_POKEBALL_FAINTED); // enemy second
}

// ═══════════════════════════════════════════════════════════════════════
// M5.7 — Battle animation tests
// ═══════════════════════════════════════════════════════════════════════

use crate::battle_anim::*;

// ─── Data integrity tests ────────────────────────────────────────────

#[test]
fn battle_anim_constants_valid() {
    assert_eq!(NUM_SUBANIMS, 86);
    assert_eq!(NUM_FRAMEBLOCKS, 122);
    assert_eq!(NUM_BASECOORDS, 177);
    assert_eq!(NUM_MOVE_ANIMS, 203);
    assert_eq!(ANIM_BASE_TILE_ID, 0x31);
    assert_eq!(FIRST_SE_ID, 0xD8);
    assert_eq!(ANIM_END, 0xFF);
}

#[test]
fn base_coords_length_matches() {
    assert_eq!(BASE_COORDS.len(), NUM_BASECOORDS);
}

#[test]
fn base_coords_spot_check() {
    // Coord 0: (0x10, 0x68)
    assert_eq!(BASE_COORDS[0], (0x10, 0x68));
    // Coord 0x0A: (0x34, 0x28)
    assert_eq!(BASE_COORDS[0x0A], (0x34, 0x28));
}

#[test]
fn frame_block_data_length() {
    assert_eq!(FRAME_BLOCK_DATA.len(), NUM_FRAMEBLOCKS);
}

#[test]
fn subanim_data_length() {
    assert_eq!(SUBANIM_DATA.len(), NUM_SUBANIMS);
}

#[test]
fn move_anim_data_length() {
    assert_eq!(MOVE_ANIM_DATA.len(), NUM_MOVE_ANIMS);
}

#[test]
fn get_subanimation_all_valid() {
    for i in 0..NUM_SUBANIMS {
        let sub = get_subanimation(i);
        assert!(!sub.frames.is_empty(), "Subanim {} has no frames", i);
    }
}

#[test]
fn get_frame_block_all_valid() {
    // Frame block 0 is an empty placeholder in the original game
    assert!(get_frame_block(0).tiles.is_empty());
    for i in 1..NUM_FRAMEBLOCKS {
        let fb = get_frame_block(i);
        assert!(!fb.tiles.is_empty(), "Frame block {} has no tiles", i);
    }
}

#[test]
fn get_move_animation_all_valid() {
    for i in 0..NUM_MOVE_ANIMS {
        let ma = get_move_animation(i);
        assert!(!ma.commands.is_empty(), "Move anim {} has no commands", i);
    }
}

// ─── Enum conversion tests ──────────────────────────────────────────

#[test]
fn subanim_transform_from_u8_roundtrip() {
    assert_eq!(SubAnimTransform::from_u8(0), SubAnimTransform::Normal);
    assert_eq!(SubAnimTransform::from_u8(1), SubAnimTransform::HvFlip);
    assert_eq!(SubAnimTransform::from_u8(2), SubAnimTransform::HFlip);
    assert_eq!(SubAnimTransform::from_u8(3), SubAnimTransform::CoordFlip);
    assert_eq!(SubAnimTransform::from_u8(4), SubAnimTransform::Reverse);
    assert_eq!(SubAnimTransform::from_u8(5), SubAnimTransform::Enemy);
    // Out of range → Normal
    assert_eq!(SubAnimTransform::from_u8(6), SubAnimTransform::Normal);
    assert_eq!(SubAnimTransform::from_u8(255), SubAnimTransform::Normal);
}

#[test]
fn frame_block_mode_from_u8_roundtrip() {
    assert_eq!(FrameBlockMode::from_u8(0), FrameBlockMode::Mode00);
    assert_eq!(FrameBlockMode::from_u8(1), FrameBlockMode::Mode01);
    assert_eq!(FrameBlockMode::from_u8(2), FrameBlockMode::Mode02);
    assert_eq!(FrameBlockMode::from_u8(3), FrameBlockMode::Mode03);
    assert_eq!(FrameBlockMode::from_u8(4), FrameBlockMode::Mode04);
    assert_eq!(FrameBlockMode::from_u8(99), FrameBlockMode::Mode00);
}

#[test]
fn frame_block_mode_properties() {
    // Mode00: has delay, cleans OAM, does NOT advance dest
    assert!(FrameBlockMode::Mode00.has_delay());
    assert!(FrameBlockMode::Mode00.cleans_oam());
    assert!(!FrameBlockMode::Mode00.advances_dest());

    // Mode02: no delay, no clean, advances dest
    assert!(!FrameBlockMode::Mode02.has_delay());
    assert!(!FrameBlockMode::Mode02.cleans_oam());
    assert!(FrameBlockMode::Mode02.advances_dest());

    // Mode03: has delay, no clean, advances dest
    assert!(FrameBlockMode::Mode03.has_delay());
    assert!(!FrameBlockMode::Mode03.cleans_oam());
    assert!(FrameBlockMode::Mode03.advances_dest());

    // Mode04: has delay, no clean, does NOT advance dest
    assert!(FrameBlockMode::Mode04.has_delay());
    assert!(!FrameBlockMode::Mode04.cleans_oam());
    assert!(!FrameBlockMode::Mode04.advances_dest());
}

#[test]
fn special_effect_from_u8_boundary() {
    assert_eq!(
        SpecialEffect::from_u8(0xD8),
        Some(SpecialEffect::WavyScreen)
    );
    assert_eq!(
        SpecialEffect::from_u8(0xFE),
        Some(SpecialEffect::DarkScreenFlash)
    );
    assert_eq!(SpecialEffect::from_u8(0xD7), None); // below range
    assert_eq!(SpecialEffect::from_u8(0xFF), None); // above range (ANIM_END)
    assert_eq!(SpecialEffect::from_u8(0x00), None); // way below range
}

#[test]
fn animation_type_from_u8() {
    assert_eq!(AnimationType::from_u8(0), AnimationType::None);
    assert_eq!(
        AnimationType::from_u8(1),
        AnimationType::ShakeScreenVertically
    );
    assert_eq!(
        AnimationType::from_u8(4),
        AnimationType::BlinkEnemyMonSprite
    );
    assert_eq!(
        AnimationType::from_u8(6),
        AnimationType::ShakeScreenHorizontallySlow2
    );
    assert_eq!(AnimationType::from_u8(7), AnimationType::None);
}

// ─── decode_command tests ───────────────────────────────────────────

#[test]
fn decode_command_subanim() {
    // kind=0 → SubAnim, sound=5, subanim=10, tileset=1 (bits 7-6 of packed), delay=6 (bits 5-0)
    let packed = (1 << 6) | 6; // tileset=1, delay=6 → 0x46
    let raw = (0u8, 5u8, 10u8, packed);
    let cmd = AnimationPlayer::decode_command(&raw);
    match cmd {
        AnimCommand::SubAnim {
            sound_id,
            subanim_id,
            tileset,
            delay,
        } => {
            assert_eq!(sound_id, 5);
            assert_eq!(subanim_id, 10);
            assert_eq!(tileset, 1);
            assert_eq!(delay, 6);
        }
        _ => panic!("Expected SubAnim command"),
    }
}

#[test]
fn decode_command_subanim_tileset_zero() {
    // tileset=0, delay=0x3F (max)
    let raw = (0u8, 0u8, 42u8, 0x3F);
    let cmd = AnimationPlayer::decode_command(&raw);
    match cmd {
        AnimCommand::SubAnim {
            sound_id,
            subanim_id,
            tileset,
            delay,
        } => {
            assert_eq!(sound_id, 0);
            assert_eq!(subanim_id, 42);
            assert_eq!(tileset, 0);
            assert_eq!(delay, 0x3F);
        }
        _ => panic!("Expected SubAnim command"),
    }
}

#[test]
fn decode_command_effect() {
    // kind=1 → Effect, sound=73, effect=0xFB (ShakeScreen)
    let raw = (1u8, 73u8, 0xFBu8, 0u8);
    let cmd = AnimationPlayer::decode_command(&raw);
    match cmd {
        AnimCommand::Effect { sound_id, effect } => {
            assert_eq!(sound_id, 73);
            assert_eq!(effect, SpecialEffect::ShakeScreen);
        }
        _ => panic!("Expected Effect command"),
    }
}

#[test]
fn decode_command_effect_wavy_screen() {
    let raw = (1u8, 0u8, 0xD8u8, 0u8);
    let cmd = AnimationPlayer::decode_command(&raw);
    match cmd {
        AnimCommand::Effect { sound_id, effect } => {
            assert_eq!(sound_id, 0);
            assert_eq!(effect, SpecialEffect::WavyScreen);
        }
        _ => panic!("Expected Effect command"),
    }
}

// ─── resolve_transform tests ────────────────────────────────────────

#[test]
fn resolve_transform_enemy_player_turn() {
    // Player is attacker + Enemy transform → HFlip
    let mut player = AnimationPlayer::new();
    player.start(0, true); // player_is_attacker = true
    assert_eq!(
        player.resolve_transform(SubAnimTransform::Enemy),
        SubAnimTransform::HFlip
    );
}

#[test]
fn resolve_transform_enemy_enemy_turn() {
    // Enemy is attacker + Enemy transform → Normal
    let mut player = AnimationPlayer::new();
    player.start(0, false); // player_is_attacker = false
    assert_eq!(
        player.resolve_transform(SubAnimTransform::Enemy),
        SubAnimTransform::Normal
    );
}

#[test]
fn resolve_transform_non_enemy_player_turn() {
    // Player is attacker + non-Enemy → Normal (override per GetSubanimationTransform1)
    let mut player = AnimationPlayer::new();
    player.start(0, true);
    assert_eq!(
        player.resolve_transform(SubAnimTransform::HvFlip),
        SubAnimTransform::Normal
    );
    assert_eq!(
        player.resolve_transform(SubAnimTransform::HFlip),
        SubAnimTransform::Normal
    );
    assert_eq!(
        player.resolve_transform(SubAnimTransform::CoordFlip),
        SubAnimTransform::Normal
    );
    assert_eq!(
        player.resolve_transform(SubAnimTransform::Normal),
        SubAnimTransform::Normal
    );
}

#[test]
fn resolve_transform_non_enemy_enemy_turn() {
    // Enemy is attacker + non-Enemy → use specified type
    let mut player = AnimationPlayer::new();
    player.start(0, false);
    assert_eq!(
        player.resolve_transform(SubAnimTransform::HvFlip),
        SubAnimTransform::HvFlip
    );
    assert_eq!(
        player.resolve_transform(SubAnimTransform::HFlip),
        SubAnimTransform::HFlip
    );
    assert_eq!(
        player.resolve_transform(SubAnimTransform::CoordFlip),
        SubAnimTransform::CoordFlip
    );
    assert_eq!(
        player.resolve_transform(SubAnimTransform::Normal),
        SubAnimTransform::Normal
    );
}

// ─── render_frame_block tests ───────────────────────────────────────

#[test]
fn render_frame_block_normal_transform() {
    // Use frame block 1 (block 0 is empty), base coord 0: (0x10, 0x68)
    let mut dest = Vec::new();
    AnimationPlayer::render_frame_block(1, 0, SubAnimTransform::Normal, &mut dest);
    assert!(!dest.is_empty(), "Frame block 1 should produce OAM entries");
    let fb = FRAME_BLOCK_DATA[1];
    let (by, bx) = BASE_COORDS[0];
    // Frame block offsets are pixel bytes (dbsprite: x_tile*8 + x_px).
    let (x_off, y_off, raw_tile, flags) = fb[0];
    let expected_y = by as i32 + y_off as i32;
    let expected_x = bx as i32 + x_off as i32;
    assert_eq!(dest[0].y, expected_y);
    assert_eq!(dest[0].x, expected_x);
    assert_eq!(dest[0].tile_id, raw_tile.wrapping_add(ANIM_BASE_TILE_ID));
    assert_eq!(dest[0].attributes, flags);
}

#[test]
fn render_frame_block_hvflip_transform() {
    let mut dest = Vec::new();
    AnimationPlayer::render_frame_block(1, 0, SubAnimTransform::HvFlip, &mut dest);
    assert!(!dest.is_empty());
    let fb = FRAME_BLOCK_DATA[1];
    let (by, bx) = BASE_COORDS[0];
    let (x_off, y_off, raw_tile, flags) = fb[0];
    let expected_y = 136i32 - (by as i32 + y_off as i32);
    let expected_x = 168i32 - (bx as i32 + x_off as i32);
    assert_eq!(dest[0].y, expected_y);
    assert_eq!(dest[0].x, expected_x);
    assert_eq!(dest[0].tile_id, raw_tile.wrapping_add(ANIM_BASE_TILE_ID));
    let expected_flags = match flags & 0x60 {
        0x00 => (flags & !0x60) | 0x60,
        0x20 => (flags & !0x60) | 0x40,
        0x40 => (flags & !0x60) | 0x20,
        0x60 => flags & !0x60,
        _ => flags,
    };
    assert_eq!(dest[0].attributes, expected_flags);
}

#[test]
fn render_frame_block_hflip_transform() {
    let mut dest = Vec::new();
    AnimationPlayer::render_frame_block(1, 0, SubAnimTransform::HFlip, &mut dest);
    assert!(!dest.is_empty());
    let fb = FRAME_BLOCK_DATA[1];
    let (by, bx) = BASE_COORDS[0];
    let (x_off, y_off, _raw_tile, flags) = fb[0];
    let expected_y = by as i32 + y_off as i32 + 40;
    let expected_x = 168i32 - (bx as i32 + x_off as i32);
    assert_eq!(dest[0].y, expected_y);
    assert_eq!(dest[0].x, expected_x);
    assert_eq!(dest[0].attributes, flags ^ OAM_XFLIP);
}

#[test]
fn render_frame_block_coordflip_transform() {
    let mut dest = Vec::new();
    AnimationPlayer::render_frame_block(1, 0, SubAnimTransform::CoordFlip, &mut dest);
    assert!(!dest.is_empty());
    let fb = FRAME_BLOCK_DATA[1];
    let (by, bx) = BASE_COORDS[0];
    let (x_off, y_off, _raw_tile, flags) = fb[0];
    let expected_y = (136i32 - by as i32) + y_off as i32;
    let expected_x = (168i32 - bx as i32) + x_off as i32;
    assert_eq!(dest[0].y, expected_y);
    assert_eq!(dest[0].x, expected_x);
    assert_eq!(dest[0].attributes, flags);
}

#[test]
fn render_frame_block_out_of_bounds() {
    let mut dest = Vec::new();
    // Invalid frame block ID
    AnimationPlayer::render_frame_block(NUM_FRAMEBLOCKS, 0, SubAnimTransform::Normal, &mut dest);
    assert!(dest.is_empty());
    // Invalid base coord ID
    AnimationPlayer::render_frame_block(0, NUM_BASECOORDS, SubAnimTransform::Normal, &mut dest);
    assert!(dest.is_empty());
}

#[test]
fn render_frame_block_tile_count_matches_data() {
    // Each tile in frame block → one OAM entry
    for fb_id in 0..NUM_FRAMEBLOCKS {
        let mut dest = Vec::new();
        AnimationPlayer::render_frame_block(fb_id, 0, SubAnimTransform::Normal, &mut dest);
        assert_eq!(
            dest.len(),
            FRAME_BLOCK_DATA[fb_id].len(),
            "Frame block {} tile count mismatch",
            fb_id
        );
    }
}

// ─── AnimationPlayer lifecycle tests ────────────────────────────────

#[test]
fn animation_player_new_is_finished() {
    let player = AnimationPlayer::new();
    assert!(player.is_finished());
    assert!(player.oam_entries().is_empty());
}

#[test]
fn animation_player_default_is_finished() {
    let player = AnimationPlayer::default();
    assert!(player.is_finished());
}

#[test]
fn animation_player_start_resets_state() {
    let mut player = AnimationPlayer::new();
    assert!(player.is_finished());
    // Start move 0 (Pound): has subanim commands
    player.start(0, true);
    assert!(!player.is_finished());
}

#[test]
fn animation_player_start_invalid_move() {
    let mut player = AnimationPlayer::new();
    player.start(9999, true);
    assert!(player.is_finished());
}

#[test]
fn animation_player_tick_finished_returns_done() {
    let mut player = AnimationPlayer::new();
    assert_eq!(player.tick(), AnimTickResult::Done);
}

// ─── tick state machine tests ───────────────────────────────────────

#[test]
fn animation_player_tick_effect_only_anim() {
    // Move 0xA5 (ShowPic, index 165) = [(1, 0, 220, 0)] → single Effect command
    // 220 = 0xDC = ShowEnemyMonPic
    let mut player = AnimationPlayer::new();
    player.start(165, true);
    let result = player.tick();
    match result {
        AnimTickResult::Effect { effect, .. } => {
            assert_eq!(effect, SpecialEffect::ShowEnemyMonPic);
        }
        other => panic!("Expected Effect, got {:?}", other),
    }
    // Next tick should be Done
    assert_eq!(player.tick(), AnimTickResult::Done);
    assert!(player.is_finished());
}

#[test]
fn animation_player_tick_subanim_produces_oam() {
    // Move 0 (Pound) = [(0, 1, 1, 8)] → SubAnim with subanim_id=1, tileset=0, delay=8
    let mut player = AnimationPlayer::new();
    player.start(0, true);
    let result = player.tick();
    // First tick should either produce Playing or WaitDelay (depending on delay and mode)
    match result {
        AnimTickResult::Playing { .. } | AnimTickResult::WaitDelay { .. } => {
            // After first tick, OAM buffer should have entries
            assert!(
                !player.oam_entries().is_empty(),
                "OAM buffer should have entries after SubAnim tick"
            );
        }
        other => panic!("Expected Playing or WaitDelay, got {:?}", other),
    }
}

#[test]
fn animation_player_runs_to_completion() {
    // Run a simple animation to completion (max 1000 ticks to prevent infinite loops)
    // Move 165 (ShowPic) = single Effect command
    let mut player = AnimationPlayer::new();
    player.start(165, true);
    let mut ticks = 0;
    loop {
        let result = player.tick();
        ticks += 1;
        if ticks > 1000 {
            panic!("Animation didn't finish in 1000 ticks");
        }
        if let AnimTickResult::Done = result {
            break;
        }
    }
    assert!(player.is_finished());
}

#[test]
fn animation_player_multi_command_anim() {
    // Move 0xA6 (EnemyFlash, index 166) = [(1, 0, 221, 0)] → Effect: 0xDD = ShowMonPic
    // Move 0xAD (XStatItem, index 173) = [(1,0,240,0), (1,0,226,0), (1,0,252,0)]
    //   → 3 Effect commands: 0xF0=LightScreenPalette, 0xE2=SpiralBallsInward, 0xFC=ResetScreenPalette
    let mut player = AnimationPlayer::new();
    player.start(173, true);

    let mut effects = Vec::new();
    let mut ticks = 0;
    loop {
        let result = player.tick();
        ticks += 1;
        if ticks > 100 {
            panic!("Too many ticks");
        }
        match result {
            AnimTickResult::Effect { effect, .. } => effects.push(effect),
            AnimTickResult::Done => break,
            _ => {}
        }
    }
    assert_eq!(effects.len(), 3);
    assert_eq!(effects[0], SpecialEffect::LightScreenPalette);
    assert_eq!(effects[1], SpecialEffect::SpiralBallsInward);
    assert_eq!(effects[2], SpecialEffect::ResetScreenPalette);
}

// ─── apply_effect tests ─────────────────────────────────────────────

#[test]
fn apply_effect_flash_screen_variants() {
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::FlashScreenLong),
        AnimEffect::FlashScreen { frames: 48 }
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::DarkScreenFlash),
        AnimEffect::FlashScreen { frames: 4 }
    );
}

#[test]
fn apply_effect_screen_palettes() {
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::DarkScreenPalette),
        AnimEffect::DarkScreenPalette
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::LightScreenPalette),
        AnimEffect::LightScreenPalette
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::ResetScreenPalette),
        AnimEffect::ResetScreenPalette
    );
}

#[test]
fn apply_effect_mon_visibility() {
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::HideMonPic),
        AnimEffect::HidePlayerMon
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::ShowMonPic),
        AnimEffect::ShowPlayerMon
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::HideEnemyMonPic),
        AnimEffect::HideEnemyMon
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::ShowEnemyMonPic),
        AnimEffect::ShowEnemyMon
    );
}

#[test]
fn apply_effect_blink_variants() {
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::BlinkEnemyMon),
        AnimEffect::BlinkEnemyMon { times: 6 }
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::BlinkMon),
        AnimEffect::BlinkPlayerMon { times: 6 }
    );
}

#[test]
fn apply_effect_slide_variants() {
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::SlideEnemyMonOff),
        AnimEffect::SlideEnemyMonOff
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::SlideMonHalfOff),
        AnimEffect::SlidePlayerMonHalfOff
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::SlideMonOff),
        AnimEffect::SlidePlayerMonOff
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::SlideMonDown),
        AnimEffect::SlidePlayerMonDown
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::SlideMonUp),
        AnimEffect::SlidePlayerMonUp
    );
}

#[test]
fn apply_effect_shoot_balls() {
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::ShootManyBallsUpward),
        AnimEffect::ShootBallsUpward { many: true }
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::ShootBallsUpward),
        AnimEffect::ShootBallsUpward { many: false }
    );
}

#[test]
fn apply_effect_shake_enemy_hud() {
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::ShakeEnemyHud),
        AnimEffect::ShakeEnemyHud { variant: 1 }
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::ShakeEnemyHud2),
        AnimEffect::ShakeEnemyHud { variant: 2 }
    );
}

#[test]
fn apply_effect_misc() {
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::WavyScreen),
        AnimEffect::WavyScreen
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::SubstituteMon),
        AnimEffect::SubstituteMon
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::TransformMon),
        AnimEffect::TransformMon
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::MinimizeMon),
        AnimEffect::MinimizeMon
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::BounceUpAndDown),
        AnimEffect::BounceUpAndDown
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::DelayAnimation10),
        AnimEffect::Delay10
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::DarkenMonPalette),
        AnimEffect::DarkenMonPalette
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::SquishMonPic),
        AnimEffect::SquishMonPic
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::ResetMonPosition),
        AnimEffect::ResetPlayerMonPosition
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::MoveMonHorizontally),
        AnimEffect::MovePlayerMonH
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::PetalsFalling),
        AnimEffect::PetalsFalling
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::LeavesFalling),
        AnimEffect::LeavesFalling
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::WaterDropletsEverywhere),
        AnimEffect::WaterDroplets
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::SpiralBallsInward),
        AnimEffect::SpiralBallsInward
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::ShakeBackAndForth),
        AnimEffect::ShakeBackAndForth
    );
    assert_eq!(
        AnimationPlayer::apply_effect(SpecialEffect::ShakeScreen),
        AnimEffect::ShakeScreenH {
            pixels: 8,
            frames: 72
        }
    );
}

// ─── HvFlip flag toggle logic ───────────────────────────────────────

#[test]
fn hvflip_flag_cycle() {
    // Test the XOR-with-0x60 cycle on isolated flag values
    // 0x00 → 0x60, 0x20 → 0x40, 0x40 → 0x20, 0x60 → 0x00
    let toggle = |flags: u8| -> u8 {
        match flags & 0x60 {
            0x00 => (flags & !0x60) | 0x60,
            0x20 => (flags & !0x60) | 0x40,
            0x40 => (flags & !0x60) | 0x20,
            0x60 => flags & !0x60,
            _ => flags,
        }
    };
    assert_eq!(toggle(0x00), 0x60);
    assert_eq!(toggle(0x20), 0x40);
    assert_eq!(toggle(0x40), 0x20);
    assert_eq!(toggle(0x60), 0x00);
    // With extra bits (e.g. priority bit 0x80)
    assert_eq!(toggle(0x80), 0xE0); // 0x80 | 0x60
    assert_eq!(toggle(0xA0), 0xC0); // 0x80 | 0x40
}

// ── Transition / Fade tests ──────────────────────────────────────────

use crate::transition::*;

#[test]
fn fade_palette_constants_all_black() {
    assert_eq!(FADE_PAL_1, FadePalette::new(0xFF, 0xFF, 0xFF));
}

#[test]
fn fade_palette_constants_normal() {
    assert_eq!(FADE_PAL_4.bgp, 0xE4);
    assert_eq!(FADE_PAL_4.obp0, 0xD0);
    assert_eq!(FADE_PAL_4.obp1, 0xE0);
    assert_eq!(FADE_PAL_4, FADE_PAL_5);
}

#[test]
fn fade_palette_constants_all_white() {
    assert_eq!(FADE_PAL_8, FadePalette::new(0x00, 0x00, 0x00));
}

#[test]
fn fade_palettes_array_length() {
    assert_eq!(FADE_PALETTES.len(), 8);
    assert_eq!(FADE_PALETTES[0], FADE_PAL_1);
    assert_eq!(FADE_PALETTES[7], FADE_PAL_8);
}

#[test]
fn fade_direction_in_from_black_indices() {
    let indices = FadeDirection::InFromBlack.palette_indices();
    assert_eq!(indices, &[0, 1, 2, 3]);
    assert_eq!(FadeDirection::InFromBlack.step_count(), 4);
}

#[test]
fn fade_direction_out_to_black_indices() {
    let indices = FadeDirection::OutToBlack.palette_indices();
    assert_eq!(indices, &[3, 2, 1, 0]);
    assert_eq!(FadeDirection::OutToBlack.step_count(), 4);
}

#[test]
fn fade_direction_out_to_white_indices() {
    let indices = FadeDirection::OutToWhite.palette_indices();
    assert_eq!(indices, &[5, 6, 7]);
    assert_eq!(FadeDirection::OutToWhite.step_count(), 3);
}

#[test]
fn fade_direction_in_from_white_indices() {
    let indices = FadeDirection::InFromWhite.palette_indices();
    assert_eq!(indices, &[6, 5, 4]);
    assert_eq!(FadeDirection::InFromWhite.step_count(), 3);
}

#[test]
fn instant_palette_normal() {
    let p = InstantPalette::Normal.palette();
    assert_eq!(p.bgp, 0xE4);
    assert_eq!(p.obp0, 0xD0);
}

#[test]
fn instant_palette_white_out() {
    let p = InstantPalette::WhiteOut.palette();
    assert_eq!(p.bgp, 0x00);
    assert_eq!(p.obp0, 0x00);
    assert_eq!(p.obp1, 0x00);
}

#[test]
fn fade_transition_new_is_idle() {
    let ft = FadeTransition::new();
    assert!(!ft.is_active());
    assert!(!ft.is_done());
    assert_eq!(ft.state(), FadeState::Idle);
    assert_eq!(ft.current_palette(), None);
}

#[test]
fn fade_transition_default_is_idle() {
    let ft = FadeTransition::default();
    assert_eq!(ft.state(), FadeState::Idle);
}

#[test]
fn fade_transition_start_sets_active() {
    let mut ft = FadeTransition::new();
    ft.start(FadeDirection::InFromBlack);
    assert!(ft.is_active());
    assert!(!ft.is_done());
    assert_eq!(
        ft.current_palette(),
        Some(FADE_PALETTES[0]) // FadePal1
    );
}

#[test]
fn fade_tick_idle_returns_idle() {
    let mut ft = FadeTransition::new();
    assert_eq!(ft.tick(), FadeTickResult::Idle);
}

#[test]
fn fade_in_from_black_full_sequence() {
    let mut ft = FadeTransition::new();
    ft.start(FadeDirection::InFromBlack);

    // 4 steps, 8 frames each = 32 ticks total
    for step in 0..4usize {
        let expected_pal = FADE_PALETTES[step]; // indices [0,1,2,3]
        for frame in 0..FADE_DELAY_FRAMES {
            let result = ft.tick();
            if step == 3 && frame == FADE_DELAY_FRAMES - 1 {
                assert_eq!(result, FadeTickResult::Done(expected_pal));
            } else if frame == FADE_DELAY_FRAMES - 1 && step < 3 {
                // Last frame of non-final step returns next step's palette
                assert_eq!(result, FadeTickResult::Fading(FADE_PALETTES[step + 1]));
            } else {
                assert_eq!(result, FadeTickResult::Fading(expected_pal));
            }
        }
    }

    assert!(ft.is_done());
    // One more tick transitions Done→Idle
    assert_eq!(ft.tick(), FadeTickResult::Idle);
    assert!(!ft.is_active());
    assert!(!ft.is_done());
}

#[test]
fn fade_out_to_white_full_sequence() {
    let mut ft = FadeTransition::new();
    ft.start(FadeDirection::OutToWhite);

    // 3 steps: indices [5,6,7]
    let indices = [5, 6, 7];
    for (step_idx, &pal_idx) in indices.iter().enumerate() {
        let expected_pal = FADE_PALETTES[pal_idx];
        for frame in 0..FADE_DELAY_FRAMES {
            let result = ft.tick();
            if step_idx == 2 && frame == FADE_DELAY_FRAMES - 1 {
                assert_eq!(result, FadeTickResult::Done(expected_pal));
            } else if frame == FADE_DELAY_FRAMES - 1 && step_idx < 2 {
                assert_eq!(
                    result,
                    FadeTickResult::Fading(FADE_PALETTES[indices[step_idx + 1]])
                );
            } else {
                assert_eq!(result, FadeTickResult::Fading(expected_pal));
            }
        }
    }
    assert!(ft.is_done());
}

#[test]
fn fade_out_to_black_ends_at_pal1() {
    let mut ft = FadeTransition::new();
    ft.start(FadeDirection::OutToBlack);

    // Run all 4*8 = 32 ticks
    let mut last = FadeTickResult::Idle;
    for _ in 0..32 {
        last = ft.tick();
    }
    // Final tick should be Done with FadePal1 (all black)
    assert_eq!(last, FadeTickResult::Done(FADE_PAL_1));
}

#[test]
fn fade_in_from_white_ends_at_pal5() {
    let mut ft = FadeTransition::new();
    ft.start(FadeDirection::InFromWhite);

    // 3*8 = 24 ticks
    let mut last = FadeTickResult::Idle;
    for _ in 0..24 {
        last = ft.tick();
    }
    assert_eq!(last, FadeTickResult::Done(FADE_PAL_5));
}

#[test]
fn fade_cancel_stops_fade() {
    let mut ft = FadeTransition::new();
    ft.start(FadeDirection::InFromBlack);
    assert!(ft.is_active());

    ft.cancel();
    assert!(!ft.is_active());
    assert_eq!(ft.state(), FadeState::Idle);
    assert_eq!(ft.tick(), FadeTickResult::Idle);
}

#[test]
fn fade_restart_during_active() {
    let mut ft = FadeTransition::new();
    ft.start(FadeDirection::InFromBlack);
    // Tick a few frames
    for _ in 0..5 {
        ft.tick();
    }
    // Restart with different direction
    ft.start(FadeDirection::OutToWhite);
    assert!(ft.is_active());
    let result = ft.tick();
    assert_eq!(result, FadeTickResult::Fading(FADE_PAL_6));
}

#[test]
fn fade_done_clears_after_one_tick() {
    let mut ft = FadeTransition::new();
    ft.start(FadeDirection::OutToWhite);
    // Run to completion: 3*8 = 24
    for _ in 0..24 {
        ft.tick();
    }
    assert!(ft.is_done());
    ft.tick(); // transitions Done→Idle
    assert_eq!(ft.state(), FadeState::Idle);
    assert_eq!(ft.current_palette(), None);
}

#[test]
fn load_gb_pal_offset_zero_is_normal() {
    let p = load_gb_pal(0);
    assert_eq!(p, FADE_PAL_4);
}

#[test]
fn load_gb_pal_offset_3_is_darker() {
    let p = load_gb_pal(3);
    assert_eq!(p, FADE_PAL_3);
}

#[test]
fn load_gb_pal_offset_6() {
    let p = load_gb_pal(6);
    assert_eq!(p, FADE_PAL_2);
}

#[test]
fn load_gb_pal_offset_9_is_darkest() {
    let p = load_gb_pal(9);
    assert_eq!(p, FADE_PAL_1);
}

#[test]
fn load_gb_pal_offset_large_saturates() {
    let p = load_gb_pal(12);
    assert_eq!(p, FADE_PAL_1);
}

#[test]
fn health_bar_color_green() {
    assert_eq!(get_health_bar_color(27), 0);
    assert_eq!(get_health_bar_color(48), 0);
    assert_eq!(get_health_bar_color(255), 0);
}

#[test]
fn health_bar_color_yellow() {
    assert_eq!(get_health_bar_color(10), 1);
    assert_eq!(get_health_bar_color(26), 1);
}

#[test]
fn health_bar_color_red() {
    assert_eq!(get_health_bar_color(0), 2);
    assert_eq!(get_health_bar_color(9), 2);
}

// ============================================================================
// M5.9: SGB/CGB Palette System Tests
// ============================================================================

#[test]
fn sgb_palette_id_enum_values() {
    assert_eq!(SgbPaletteId::Route as u8, 0x00);
    assert_eq!(SgbPaletteId::Pallet as u8, 0x01);
    assert_eq!(SgbPaletteId::Viridian as u8, 0x02);
    assert_eq!(SgbPaletteId::Pewter as u8, 0x03);
    assert_eq!(SgbPaletteId::Cerulean as u8, 0x04);
    assert_eq!(SgbPaletteId::Lavender as u8, 0x05);
    assert_eq!(SgbPaletteId::Vermilion as u8, 0x06);
    assert_eq!(SgbPaletteId::Celadon as u8, 0x07);
    assert_eq!(SgbPaletteId::Fuchsia as u8, 0x08);
    assert_eq!(SgbPaletteId::Cinnabar as u8, 0x09);
    assert_eq!(SgbPaletteId::Indigo as u8, 0x0A);
    assert_eq!(SgbPaletteId::Saffron as u8, 0x0B);
    assert_eq!(SgbPaletteId::TownMap as u8, 0x0C);
    assert_eq!(SgbPaletteId::Logo1 as u8, 0x0D);
    assert_eq!(SgbPaletteId::MewMon as u8, 0x10);
    assert_eq!(SgbPaletteId::GreenMon as u8, 0x16);
    assert_eq!(SgbPaletteId::YellowMon as u8, 0x18);
    assert_eq!(SgbPaletteId::GrayMon as u8, 0x19);
    assert_eq!(SgbPaletteId::Black as u8, 0x1E);
    assert_eq!(SgbPaletteId::GreenBar as u8, 0x1F);
    assert_eq!(SgbPaletteId::YellowBar as u8, 0x20);
    assert_eq!(SgbPaletteId::RedBar as u8, 0x21);
    assert_eq!(SgbPaletteId::Cave as u8, 0x23);
    assert_eq!(SgbPaletteId::GameFreak as u8, 0x24);
}

#[test]
fn sgb_palette_id_from_u8_valid() {
    assert_eq!(SgbPaletteId::from_u8(0x00), Some(SgbPaletteId::Route));
    assert_eq!(SgbPaletteId::from_u8(0x01), Some(SgbPaletteId::Pallet));
    assert_eq!(SgbPaletteId::from_u8(0x24), Some(SgbPaletteId::GameFreak));
}

#[test]
fn sgb_palette_id_from_u8_invalid() {
    assert_eq!(SgbPaletteId::from_u8(0x25), None);
    assert_eq!(SgbPaletteId::from_u8(0xFF), None);
}

#[test]
fn sgb_palette_id_roundtrip() {
    for i in 0..NUM_SGB_PALS as u8 {
        let id = SgbPaletteId::from_u8(i).unwrap();
        assert_eq!(id.as_u8(), i);
    }
}

#[test]
fn num_sgb_pals_correct() {
    assert_eq!(NUM_SGB_PALS, 37);
}

#[test]
fn set_pal_command_enum_values() {
    assert_eq!(SetPalCommand::BattleBlack as u8, 0x00);
    assert_eq!(SetPalCommand::Battle as u8, 0x01);
    assert_eq!(SetPalCommand::TownMap as u8, 0x02);
    assert_eq!(SetPalCommand::StatusScreen as u8, 0x03);
    assert_eq!(SetPalCommand::Pokedex as u8, 0x04);
    assert_eq!(SetPalCommand::Slots as u8, 0x05);
    assert_eq!(SetPalCommand::TitleScreen as u8, 0x06);
    assert_eq!(SetPalCommand::NidorinoIntro as u8, 0x07);
    assert_eq!(SetPalCommand::Generic as u8, 0x08);
    assert_eq!(SetPalCommand::Overworld as u8, 0x09);
    assert_eq!(SetPalCommand::PartyMenu as u8, 0x0A);
    assert_eq!(SetPalCommand::PokemonWholeScreen as u8, 0x0B);
    assert_eq!(SetPalCommand::GameFreakIntro as u8, 0x0C);
    assert_eq!(SetPalCommand::TrainerCard as u8, 0x0D);
}

#[test]
fn set_pal_command_from_u8_valid() {
    assert_eq!(
        SetPalCommand::from_u8(0x00),
        Some(SetPalCommand::BattleBlack)
    );
    assert_eq!(SetPalCommand::from_u8(0x09), Some(SetPalCommand::Overworld));
    assert_eq!(
        SetPalCommand::from_u8(0x0D),
        Some(SetPalCommand::TrainerCard)
    );
}

#[test]
fn set_pal_command_from_u8_invalid() {
    assert_eq!(SetPalCommand::from_u8(0x0E), None);
    assert_eq!(SetPalCommand::from_u8(0xFC), None);
    assert_eq!(SetPalCommand::from_u8(0xFF), None);
}

#[test]
fn set_pal_special_constants() {
    assert_eq!(SET_PAL_PARTY_MENU_HP_BARS, 0xFC);
    assert_eq!(SET_PAL_DEFAULT, 0xFF);
}

#[test]
fn sgb_color_to_rgba_white() {
    let c = SgbColor::new(31, 31, 31);
    let rgba = crate::palette::sgb_color_to_rgba(&c);
    // (31 << 3) | (31 >> 2) = 248 | 7 = 255
    assert_eq!(rgba.to_array(), [255, 255, 255, 255]);
}

#[test]
fn sgb_color_to_rgba_black() {
    let c = SgbColor::new(0, 0, 0);
    let rgba = crate::palette::sgb_color_to_rgba(&c);
    assert_eq!(rgba.to_array(), [0, 0, 0, 255]);
}

#[test]
fn sgb_color_to_rgba_mid() {
    let c = SgbColor::new(15, 15, 15);
    let rgba = crate::palette::sgb_color_to_rgba(&c);
    assert_eq!(rgba.to_array(), [123, 123, 123, 255]);
}

#[test]
fn sgb_color_to_rgba_specific() {
    let c = SgbColor::new(31, 29, 31);
    let rgba = crate::palette::sgb_color_to_rgba(&c);
    assert_eq!(rgba.r, 255);
    assert_eq!(rgba.g, 239);
    assert_eq!(rgba.b, 255);
}

#[test]
fn super_palettes_red_count() {
    assert_eq!(SUPER_PALETTES_RED.len(), NUM_SGB_PALS);
}

#[test]
fn super_palettes_blue_count() {
    assert_eq!(SUPER_PALETTES_BLUE.len(), NUM_SGB_PALS);
}

#[test]
fn super_palettes_red_route_color0() {
    // PAL_ROUTE color0 = (31,29,31)
    let entry = &SUPER_PALETTES_RED[SgbPaletteId::Route as usize];
    assert_eq!(entry[0], SgbColor::new(31, 29, 31));
}

#[test]
fn super_palettes_red_route_color3() {
    // PAL_ROUTE color3 = (3,2,2)
    let entry = &SUPER_PALETTES_RED[SgbPaletteId::Route as usize];
    assert_eq!(entry[3], SgbColor::new(3, 2, 2));
}

#[test]
fn super_palettes_red_vs_blue_logo1_differ() {
    let red_logo1 = &SUPER_PALETTES_RED[SgbPaletteId::Logo1 as usize];
    let blue_logo1 = &SUPER_PALETTES_BLUE[SgbPaletteId::Logo1 as usize];
    // color2 and color3 differ between Red and Blue
    assert_ne!(red_logo1[2], blue_logo1[2]);
    assert_ne!(red_logo1[3], blue_logo1[3]);
}

#[test]
fn super_palettes_red_vs_blue_slots2_differ() {
    let red = &SUPER_PALETTES_RED[SgbPaletteId::Slots2 as usize];
    let blue = &SUPER_PALETTES_BLUE[SgbPaletteId::Slots2 as usize];
    // color2 differs: Red=(25,17,21), Blue=(16,19,29)
    assert_ne!(red[2], blue[2]);
}

#[test]
fn super_palettes_red_vs_blue_pallet_same() {
    // PAL_PALLET should be identical in both versions
    let red = &SUPER_PALETTES_RED[SgbPaletteId::Pallet as usize];
    let blue = &SUPER_PALETTES_BLUE[SgbPaletteId::Pallet as usize];
    assert_eq!(red, blue);
}

#[test]
fn super_palettes_helper_selects_version() {
    let red = super_palettes(true);
    let blue = super_palettes(false);
    let red_logo1 = &red[SgbPaletteId::Logo1 as usize];
    let blue_logo1 = &blue[SgbPaletteId::Logo1 as usize];
    assert_eq!(red_logo1, &SUPER_PALETTES_RED[SgbPaletteId::Logo1 as usize]);
    assert_eq!(
        blue_logo1,
        &SUPER_PALETTES_BLUE[SgbPaletteId::Logo1 as usize]
    );
    assert_ne!(red_logo1, blue_logo1);
}

#[test]
fn lookup_sgb_palette_returns_correct_entry() {
    let entry = lookup_sgb_palette(SgbPaletteId::Cave, true);
    assert_eq!(entry, &SUPER_PALETTES_RED[SgbPaletteId::Cave as usize]);
}

#[test]
fn sgb_entry_to_palette_conversion() {
    let entry = &SUPER_PALETTES_RED[SgbPaletteId::Route as usize];
    let pal = sgb_entry_to_palette(entry);
    // Verify color0 is the RGBA conversion of (31,29,31)
    assert_eq!(
        pal.colors[0],
        sgb_color_to_rgba(&SgbColor::new(31, 29, 31))
    );
    assert_eq!(
        pal.colors[3],
        sgb_color_to_rgba(&SgbColor::new(3, 2, 2))
    );
}

#[test]
fn monster_palettes_count() {
    assert_eq!(MONSTER_PALETTES.len(), NUM_POKEMON_PLUS_ONE);
    assert_eq!(NUM_POKEMON_PLUS_ONE, 152);
}

#[test]
fn monster_palette_bulbasaur() {
    assert_eq!(monster_palette(1), SgbPaletteId::GreenMon);
}

#[test]
fn monster_palette_charmander() {
    assert_eq!(monster_palette(4), SgbPaletteId::RedMon);
}

#[test]
fn monster_palette_squirtle() {
    assert_eq!(monster_palette(7), SgbPaletteId::CyanMon);
}

#[test]
fn monster_palette_pikachu() {
    assert_eq!(monster_palette(25), SgbPaletteId::YellowMon);
}

#[test]
fn monster_palette_gengar() {
    assert_eq!(monster_palette(94), SgbPaletteId::PurpleMon);
}

#[test]
fn monster_palette_mewtwo() {
    assert_eq!(monster_palette(150), SgbPaletteId::MewMon);
}

#[test]
fn monster_palette_mew() {
    assert_eq!(monster_palette(151), SgbPaletteId::MewMon);
}

#[test]
fn monster_palette_missingno() {
    assert_eq!(monster_palette(0), SgbPaletteId::MewMon);
}

#[test]
fn monster_palette_out_of_range_fallback() {
    // Index >= 152 should fallback to MISSINGNO (MewMon)
    assert_eq!(monster_palette(200), SgbPaletteId::MewMon);
    assert_eq!(monster_palette(255), SgbPaletteId::MewMon);
}

#[test]
fn determine_palette_id_normal() {
    assert_eq!(determine_palette_id(1, false), SgbPaletteId::GreenMon); // Bulbasaur
    assert_eq!(determine_palette_id(25, false), SgbPaletteId::YellowMon); // Pikachu
}

#[test]
fn determine_palette_id_transformed() {
    // Any transformed Pokémon → PAL_GRAYMON (Ditto's palette)
    assert_eq!(determine_palette_id(1, true), SgbPaletteId::GrayMon);
    assert_eq!(determine_palette_id(25, true), SgbPaletteId::GrayMon);
    assert_eq!(determine_palette_id(150, true), SgbPaletteId::GrayMon);
}

#[test]
fn overworld_palette_cemetery_tileset() {
    assert_eq!(
        overworld_palette_for_map(TILESET_CEMETERY, 0x00, 0x00),
        SgbPaletteId::GrayMon
    );
    assert_eq!(
        overworld_palette_for_map(TILESET_CEMETERY, 0x50, 0x05),
        SgbPaletteId::GrayMon
    );
}

#[test]
fn overworld_palette_cavern_tileset() {
    assert_eq!(
        overworld_palette_for_map(TILESET_CAVERN, 0x00, 0x00),
        SgbPaletteId::Cave
    );
    assert_eq!(
        overworld_palette_for_map(TILESET_CAVERN, 0x50, 0x05),
        SgbPaletteId::Cave
    );
}

#[test]
fn overworld_palette_pallet_town() {
    // Pallet Town map_id = 0x00, tileset != cemetery/cavern
    // town = 0x00 < NUM_CITY_MAPS → palette = 0+1 = 1 = PAL_PALLET
    assert_eq!(
        overworld_palette_for_map(0, 0x00, 0x00),
        SgbPaletteId::Pallet
    );
}

#[test]
fn overworld_palette_viridian_city() {
    // Viridian City map_id = 0x01
    assert_eq!(
        overworld_palette_for_map(0, 0x01, 0x00),
        SgbPaletteId::Viridian
    );
}

#[test]
fn overworld_palette_saffron_city() {
    // Saffron City map_id = 0x0A (last city before NUM_CITY_MAPS=0x0B)
    assert_eq!(
        overworld_palette_for_map(0, 0x0A, 0x00),
        SgbPaletteId::Saffron
    );
}

#[test]
fn overworld_palette_route() {
    // A route map_id (>= NUM_CITY_MAPS but < FIRST_INDOOR_MAP)
    assert_eq!(
        overworld_palette_for_map(0, 0x0B, 0x00),
        SgbPaletteId::Route
    );
    assert_eq!(
        overworld_palette_for_map(0, 0x20, 0x00),
        SgbPaletteId::Route
    );
}

#[test]
fn overworld_palette_indoor_pallet_town() {
    // Indoor map (>= FIRST_INDOOR_MAP), last_map = Pallet Town (0x00)
    assert_eq!(
        overworld_palette_for_map(0, 0x25, 0x00),
        SgbPaletteId::Pallet
    );
}

#[test]
fn overworld_palette_indoor_celadon() {
    // Indoor map, last_map = Celadon City (map_id 0x06)
    // Palette = 0x06 + 1 = 0x07 = Celadon
    assert_eq!(
        overworld_palette_for_map(0, 0x30, 0x06),
        SgbPaletteId::Celadon
    );
}

#[test]
fn overworld_palette_indoor_route() {
    // Indoor map, last_map = route (>= NUM_CITY_MAPS)
    assert_eq!(
        overworld_palette_for_map(0, 0x30, 0x0C),
        SgbPaletteId::Route
    );
}

#[test]
fn overworld_palette_cerulean_cave() {
    // Cerulean Cave 2F (0xE2) and 1F (0xE4) → PAL_CAVE
    assert_eq!(
        overworld_palette_for_map(0, MAP_CERULEAN_CAVE_2F, 0x00),
        SgbPaletteId::Cave
    );
    assert_eq!(
        overworld_palette_for_map(0, MAP_CERULEAN_CAVE_1F, 0x00),
        SgbPaletteId::Cave
    );
    // Map 0xE3 (Cerulean Cave B1F) also in range
    assert_eq!(overworld_palette_for_map(0, 0xE3, 0x00), SgbPaletteId::Cave);
}

#[test]
fn overworld_palette_loreleis_room() {
    assert_eq!(
        overworld_palette_for_map(0, MAP_LORELEIS_ROOM, 0x00),
        SgbPaletteId::Pallet
    );
}

#[test]
fn overworld_palette_brunos_room() {
    assert_eq!(
        overworld_palette_for_map(0, MAP_BRUNOS_ROOM, 0x00),
        SgbPaletteId::Cave
    );
}

#[test]
fn hp_bar_to_sgb_palette_values() {
    assert_eq!(hp_bar_to_sgb_palette(0), SgbPaletteId::GreenBar);
    assert_eq!(hp_bar_to_sgb_palette(1), SgbPaletteId::YellowBar);
    assert_eq!(hp_bar_to_sgb_palette(2), SgbPaletteId::RedBar);
    assert_eq!(hp_bar_to_sgb_palette(3), SgbPaletteId::RedBar); // fallback
}

#[test]
fn color_palette_state_new_defaults() {
    let provider = PokemonPaletteData::new(true);
    let state = ColorPaletteState::new(PaletteMode::Dmg, true, &provider);
    assert_eq!(state.mode, PaletteMode::Dmg);
    assert_eq!(state.is_red, true);
    assert_eq!(state.sgb_bg_palette, SgbPaletteId::Route);
    assert_eq!(state.sgb_obj0_palette, SgbPaletteId::Route);
    assert_eq!(state.sgb_obj1_palette, SgbPaletteId::Route);
    assert_eq!(state.default_command, SetPalCommand::Generic);
}

#[test]
fn color_palette_state_dmg_mode_uses_dmg_palette() {
    let provider = PokemonPaletteData::new(true);
    let state = ColorPaletteState::new(PaletteMode::Dmg, true, &provider);
    let bg = state.bg_palette();
    let expected = PaletteState::default().bg_palette();
    assert_eq!(bg.colors, expected.colors);
}

#[test]
fn color_palette_state_sgb_mode_uses_sgb_palette() {
    let provider = PokemonPaletteData::new(true);
    let mut state = ColorPaletteState::new(PaletteMode::Sgb, true, &provider);
    state.sgb_bg_palette = SgbPaletteId::Cave;
    let bg = state.bg_palette();
    let expected = sgb_entry_to_palette(lookup_sgb_palette(SgbPaletteId::Cave, true));
    assert_eq!(bg.colors, expected.colors);
}

#[test]
fn color_palette_state_grayscale_mode() {
    let provider = PokemonPaletteData::new(true);
    let state = ColorPaletteState::new(PaletteMode::Grayscale, true, &provider);
    let bg = state.bg_palette();
    // Should use GRAYSCALE_PALETTE as base
    let mut ps = PaletteState::default();
    ps.base = GRAYSCALE_PALETTE;
    assert_eq!(bg.colors, ps.bg_palette().colors);
}

#[test]
fn color_palette_state_pocket_mode() {
    let provider = PokemonPaletteData::new(true);
    let state = ColorPaletteState::new(PaletteMode::Pocket, true, &provider);
    let bg = state.bg_palette();
    let mut ps = PaletteState::default();
    ps.base = POCKET_PALETTE;
    assert_eq!(bg.colors, ps.bg_palette().colors);
}

#[test]
fn color_palette_state_set_overworld() {
    let provider = PokemonPaletteData::new(true);
    let mut state = ColorPaletteState::new(PaletteMode::Sgb, true, &provider);
    state.set_overworld_palette(0, 0x00, 0x00); // Pallet Town
    assert_eq!(state.sgb_bg_palette, SgbPaletteId::Pallet);
    assert_eq!(state.default_command, SetPalCommand::Overworld);
}

#[test]
fn color_palette_state_set_battle() {
    let provider = PokemonPaletteData::new(true);
    let mut state = ColorPaletteState::new(PaletteMode::Sgb, true, &provider);
    // Bulbasaur (1, not transformed) vs Pikachu (25, not transformed), HP bars green
    state.set_battle_palette(1, false, 25, false, 0, 0);
    assert_eq!(state.sgb_obj0_palette, SgbPaletteId::GreenMon); // Bulbasaur
    assert_eq!(state.sgb_obj1_palette, SgbPaletteId::YellowMon); // Pikachu
    assert_eq!(state.sgb_bg_palette, SgbPaletteId::GreenBar); // HP bar green
    assert_eq!(state.default_command, SetPalCommand::Battle);
}

#[test]
fn color_palette_state_set_battle_transformed() {
    let provider = PokemonPaletteData::new(true);
    let mut state = ColorPaletteState::new(PaletteMode::Sgb, true, &provider);
    // Ditto transformed as Pikachu vs Mewtwo, HP bar yellow/red
    state.set_battle_palette(25, true, 150, false, 1, 2);
    assert_eq!(state.sgb_obj0_palette, SgbPaletteId::GrayMon); // transformed → GrayMon
    assert_eq!(state.sgb_obj1_palette, SgbPaletteId::MewMon); // Mewtwo
    assert_eq!(state.sgb_bg_palette, SgbPaletteId::YellowBar); // player HP bar yellow
}

// ==========================================================================
// M5.10 — Resource loading pipeline (PNG → tiles) tests
// ==========================================================================

use crate::resource::*;

// --- grayscale_to_color_index ---

#[test]
fn grayscale_white_maps_to_color_0() {
    assert_eq!(grayscale_to_color_index(255), 0);
}

#[test]
fn grayscale_light_gray_maps_to_color_1() {
    assert_eq!(grayscale_to_color_index(170), 1);
}

#[test]
fn grayscale_dark_gray_maps_to_color_2() {
    assert_eq!(grayscale_to_color_index(85), 2);
}

#[test]
fn grayscale_black_maps_to_color_3() {
    assert_eq!(grayscale_to_color_index(0), 3);
}

#[test]
fn grayscale_snapping_near_white() {
    // 213–255 → 0
    assert_eq!(grayscale_to_color_index(213), 0);
    assert_eq!(grayscale_to_color_index(240), 0);
}

#[test]
fn grayscale_snapping_near_light_gray() {
    // 128–212 → 1
    assert_eq!(grayscale_to_color_index(128), 1);
    assert_eq!(grayscale_to_color_index(212), 1);
}

#[test]
fn grayscale_snapping_near_dark_gray() {
    // 43–127 → 2
    assert_eq!(grayscale_to_color_index(43), 2);
    assert_eq!(grayscale_to_color_index(127), 2);
}

#[test]
fn grayscale_snapping_near_black() {
    // 0–42 → 3
    assert_eq!(grayscale_to_color_index(42), 3);
    assert_eq!(grayscale_to_color_index(1), 3);
}

// --- grayscale_to_color_index_strict ---

#[test]
fn strict_grayscale_exact_values() {
    assert_eq!(grayscale_to_color_index_strict(255), Some(0));
    assert_eq!(grayscale_to_color_index_strict(170), Some(1));
    assert_eq!(grayscale_to_color_index_strict(85), Some(2));
    assert_eq!(grayscale_to_color_index_strict(0), Some(3));
}

#[test]
fn strict_grayscale_rejects_non_standard() {
    assert_eq!(grayscale_to_color_index_strict(128), None);
    assert_eq!(grayscale_to_color_index_strict(200), None);
    assert_eq!(grayscale_to_color_index_strict(50), None);
    assert_eq!(grayscale_to_color_index_strict(1), None);
}

// --- bw_to_color_index ---

#[test]
fn bw_white_maps_to_color_0() {
    assert_eq!(bw_to_color_index(255), 0);
    assert_eq!(bw_to_color_index(128), 0);
}

#[test]
fn bw_black_maps_to_color_3() {
    assert_eq!(bw_to_color_index(0), 3);
    assert_eq!(bw_to_color_index(127), 3);
}

// --- png_to_2bpp with synthetic image ---

#[test]
fn png_to_2bpp_single_white_tile() {
    // Create an 8×8 all-white image
    let img =
        image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(8, 8, image::Luma([255])));
    let data = png_to_2bpp(&img).unwrap();
    assert_eq!(data.len(), 16); // 1 tile × 16 bytes
                                // All white = color 0 → all bits 0
    for byte in &data {
        assert_eq!(*byte, 0x00);
    }
}

#[test]
fn png_to_2bpp_single_black_tile() {
    // Create an 8×8 all-black image
    let img = image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(8, 8, image::Luma([0])));
    let data = png_to_2bpp(&img).unwrap();
    assert_eq!(data.len(), 16);
    // All black = color 3 → both lo and hi bytes = 0xFF
    for byte in &data {
        assert_eq!(*byte, 0xFF);
    }
}

#[test]
fn png_to_2bpp_alternating_colors() {
    // Create an 8×8 image where first row alternates white(255)/black(0)
    let mut img = image::GrayImage::from_pixel(8, 8, image::Luma([255]));
    // Set odd columns of first row to black
    for col in (1..8).step_by(2) {
        img.put_pixel(col, 0, image::Luma([0]));
    }
    let dyn_img = image::DynamicImage::ImageLuma8(img);
    let data = png_to_2bpp(&dyn_img).unwrap();

    // First row: pixels are [0, 3, 0, 3, 0, 3, 0, 3]
    // lo byte: bits 7,5,3,1 = 0; bits 6,4,2,0 = 1 → 0b01010101 = 0x55
    // hi byte: same → 0x55
    assert_eq!(data[0], 0x55); // lo
    assert_eq!(data[1], 0x55); // hi
}

#[test]
fn png_to_2bpp_light_gray_tile() {
    // All light gray (170) → color 1 → lo=1, hi=0
    let img =
        image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(8, 8, image::Luma([170])));
    let data = png_to_2bpp(&img).unwrap();
    for row in 0..8 {
        assert_eq!(data[row * 2], 0xFF); // lo = all 1s (color 1, bit 0 = 1)
        assert_eq!(data[row * 2 + 1], 0x00); // hi = all 0s (color 1, bit 1 = 0)
    }
}

#[test]
fn png_to_2bpp_dark_gray_tile() {
    // All dark gray (85) → color 2 → lo=0, hi=1
    let img =
        image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(8, 8, image::Luma([85])));
    let data = png_to_2bpp(&img).unwrap();
    for row in 0..8 {
        assert_eq!(data[row * 2], 0x00); // lo = all 0s (color 2, bit 0 = 0)
        assert_eq!(data[row * 2 + 1], 0xFF); // hi = all 1s (color 2, bit 1 = 1)
    }
}

#[test]
fn png_to_2bpp_multi_tile() {
    // 16×8 image = 2 tiles side by side
    let mut img = image::GrayImage::from_pixel(16, 8, image::Luma([255])); // all white
                                                                           // Make second tile all black
    for y in 0..8 {
        for x in 8..16 {
            img.put_pixel(x, y, image::Luma([0]));
        }
    }
    let dyn_img = image::DynamicImage::ImageLuma8(img);
    let data = png_to_2bpp(&dyn_img).unwrap();
    assert_eq!(data.len(), 32); // 2 tiles

    // First tile: all white (all zeros)
    for i in 0..16 {
        assert_eq!(data[i], 0x00);
    }
    // Second tile: all black (all 0xFF)
    for i in 16..32 {
        assert_eq!(data[i], 0xFF);
    }
}

#[test]
fn png_to_2bpp_2x2_tiles() {
    // 16×16 image = 4 tiles in 2×2 arrangement
    // Tile order should be: (0,0), (1,0), (0,1), (1,1) — row-major
    let mut img = image::GrayImage::from_pixel(16, 16, image::Luma([255])); // all white
                                                                            // Make tile (1,0) = top-right all black
    for y in 0..8 {
        for x in 8..16 {
            img.put_pixel(x, y, image::Luma([0]));
        }
    }
    let dyn_img = image::DynamicImage::ImageLuma8(img);
    let data = png_to_2bpp(&dyn_img).unwrap();
    assert_eq!(data.len(), 64); // 4 tiles

    // Tile 0 (0,0): white
    for i in 0..16 {
        assert_eq!(data[i], 0x00);
    }
    // Tile 1 (1,0): black
    for i in 16..32 {
        assert_eq!(data[i], 0xFF);
    }
    // Tile 2 (0,1): white
    for i in 32..48 {
        assert_eq!(data[i], 0x00);
    }
    // Tile 3 (1,1): white
    for i in 48..64 {
        assert_eq!(data[i], 0x00);
    }
}

#[test]
fn png_to_2bpp_rejects_non_multiple_of_8() {
    let img =
        image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(10, 8, image::Luma([255])));
    let result = png_to_2bpp(&img);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ResourceError::InvalidDimensions {
            width: 10,
            height: 8
        }
    ));
}

// --- png_to_1bpp ---

#[test]
fn png_to_1bpp_all_white() {
    let img =
        image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(8, 8, image::Luma([255])));
    let data = png_to_1bpp(&img).unwrap();
    assert_eq!(data.len(), 8); // 1 tile × 8 bytes
    for byte in &data {
        assert_eq!(*byte, 0x00); // all white → all bits 0
    }
}

#[test]
fn png_to_1bpp_all_black() {
    let img = image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(8, 8, image::Luma([0])));
    let data = png_to_1bpp(&img).unwrap();
    assert_eq!(data.len(), 8);
    for byte in &data {
        assert_eq!(*byte, 0xFF); // all black → all bits 1
    }
}

#[test]
fn png_to_1bpp_checkerboard() {
    // First row: alternating white/black pixels
    let mut img = image::GrayImage::from_pixel(8, 8, image::Luma([255]));
    for col in (0..8).step_by(2) {
        img.put_pixel(col, 0, image::Luma([0]));
    }
    let dyn_img = image::DynamicImage::ImageLuma8(img);
    let data = png_to_1bpp(&dyn_img).unwrap();
    // First row: black(0), white(255), black(0), white(255), ... → 0b10101010 = 0xAA
    assert_eq!(data[0], 0xAA);
}

#[test]
fn png_to_1bpp_rejects_non_multiple_of_8() {
    let img =
        image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(7, 8, image::Luma([255])));
    assert!(png_to_1bpp(&img).is_err());
}

// --- png_to_tileset_2bpp ---

#[test]
fn png_to_tileset_2bpp_produces_correct_tile_count() {
    // 16×16 → 4 tiles
    let img =
        image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(16, 16, image::Luma([255])));
    let ts = png_to_tileset_2bpp(&img).unwrap();
    assert_eq!(ts.len(), 4);
}

#[test]
fn png_to_tileset_1bpp_produces_correct_tile_count() {
    // 128×64 → 128 tiles (like the font)
    let img =
        image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(128, 64, image::Luma([255])));
    let ts = png_to_tileset_1bpp(&img).unwrap();
    assert_eq!(ts.len(), 128);
}

// --- PokemonSpriteSize ---

#[test]
fn pokemon_sprite_size_small() {
    let s = PokemonSpriteSize::Small;
    assert_eq!(s.tiles(), 5);
    assert_eq!(s.pixels(), 40);
}

#[test]
fn pokemon_sprite_size_medium() {
    let s = PokemonSpriteSize::Medium;
    assert_eq!(s.tiles(), 6);
    assert_eq!(s.pixels(), 48);
}

#[test]
fn pokemon_sprite_size_large() {
    let s = PokemonSpriteSize::Large;
    assert_eq!(s.tiles(), 7);
    assert_eq!(s.pixels(), 56);
}

#[test]
fn pokemon_sprite_size_from_dimensions() {
    assert_eq!(
        PokemonSpriteSize::from_dimensions(40, 40),
        Some(PokemonSpriteSize::Small)
    );
    assert_eq!(
        PokemonSpriteSize::from_dimensions(48, 48),
        Some(PokemonSpriteSize::Medium)
    );
    assert_eq!(
        PokemonSpriteSize::from_dimensions(56, 56),
        Some(PokemonSpriteSize::Large)
    );
    assert_eq!(PokemonSpriteSize::from_dimensions(32, 32), None);
    assert_eq!(PokemonSpriteSize::from_dimensions(64, 64), None);
}

#[test]
fn pokemon_sprite_back_constants() {
    assert_eq!(PokemonSpriteSize::BACK_TILES, 4);
    assert_eq!(PokemonSpriteSize::BACK_PIXELS, 32);
}

// --- AssetCategory ---

#[test]
fn asset_category_subdir_tileset() {
    assert_eq!(AssetCategory::Tileset.subdir(), "tilesets");
}

#[test]
fn asset_category_subdir_pokemon_front() {
    assert_eq!(AssetCategory::PokemonFront.subdir(), "pokemon/front");
}

#[test]
fn asset_category_subdir_pokemon_front_rg() {
    assert_eq!(AssetCategory::PokemonFrontRG.subdir(), "pokemon/front_rg");
}

#[test]
fn asset_category_subdir_pokemon_back() {
    assert_eq!(AssetCategory::PokemonBack.subdir(), "pokemon/back");
}

#[test]
fn asset_category_subdir_font() {
    assert_eq!(AssetCategory::Font.subdir(), "font");
}

#[test]
fn asset_category_subdir_trainer() {
    assert_eq!(AssetCategory::Trainer.subdir(), "trainers");
}

#[test]
fn asset_category_font_is_1bpp() {
    assert!(AssetCategory::Font.is_1bpp());
}

#[test]
fn asset_category_tileset_is_not_1bpp() {
    assert!(!AssetCategory::Tileset.is_1bpp());
    assert!(!AssetCategory::Sprite.is_1bpp());
    assert!(!AssetCategory::PokemonFront.is_1bpp());
    assert!(!AssetCategory::Trainer.is_1bpp());
    assert!(!AssetCategory::Battle.is_1bpp());
}

// --- AssetRoot integration tests (use actual gfx/ directory) ---

/// Helper to get the pokered gfx root for integration tests.
/// Skips if gfx/ is not found (e.g., CI/checkout that hasn't run
/// `scripts/fetch-gfx.sh`).
fn get_test_asset_root() -> Option<AssetRoot> {
    // gfx/ lives at the repo root; this crate is at
    // crates/pokered-renderer, so two levels up.
    let candidate = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gfx");
    if candidate.is_dir() {
        return AssetRoot::new(candidate).ok();
    }
    None
}

#[test]
fn asset_root_from_parent_finds_gfx() {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // gfx/ lives at the repo root, two levels up from this crate
    // (crates/pokered-renderer).
    let repo_root = manifest.join("../..");
    if repo_root.join("gfx").is_dir() {
        let root = AssetRoot::from_parent(&repo_root).unwrap();
        assert!(root.gfx_dir().is_dir());
    }
}

#[test]
fn asset_root_resolve_path() {
    if let Some(root) = get_test_asset_root() {
        let path = root.resolve(AssetCategory::Tileset, "overworld.png");
        assert!(path.to_str().unwrap().contains("tilesets/overworld.png"));
    }
}

#[test]
fn asset_root_resolve_checked_existing_file() {
    if let Some(root) = get_test_asset_root() {
        let result = root.resolve_checked(AssetCategory::Tileset, "overworld.png");
        assert!(result.is_ok(), "overworld.png should exist: {:?}", result);
    }
}

#[test]
fn asset_root_resolve_checked_missing_file() {
    if let Some(root) = get_test_asset_root() {
        let result = root.resolve_checked(AssetCategory::Tileset, "nonexistent.png");
        assert!(result.is_err());
    }
}

#[test]
fn asset_root_list_pngs_tilesets() {
    if let Some(root) = get_test_asset_root() {
        let pngs = root.list_pngs(AssetCategory::Tileset).unwrap();
        assert!(!pngs.is_empty(), "tilesets should have PNGs");
        // All should end in .png
        for p in &pngs {
            assert_eq!(p.extension().unwrap(), "png");
        }
    }
}

#[test]
fn asset_root_list_pngs_pokemon_front() {
    if let Some(root) = get_test_asset_root() {
        let pngs = root.list_pngs(AssetCategory::PokemonFront).unwrap();
        assert!(
            pngs.len() > 100,
            "should have 151+ pokemon front sprites, got {}",
            pngs.len()
        );
    }
}

#[test]
fn asset_root_list_pngs_sorted() {
    if let Some(root) = get_test_asset_root() {
        let pngs = root.list_pngs(AssetCategory::Sprite).unwrap();
        let sorted: Vec<_> = {
            let mut v = pngs.clone();
            v.sort();
            v
        };
        assert_eq!(pngs, sorted, "list_pngs should return sorted paths");
    }
}

// --- Real PNG loading tests ---

#[test]
fn load_overworld_tileset_png() {
    if let Some(root) = get_test_asset_root() {
        let path = root.resolve(AssetCategory::Tileset, "overworld.png");
        if path.is_file() {
            let ts = load_tileset_from_png(&path).unwrap();
            // overworld.png is 128×48 = 16×6 tiles = 96 tiles
            assert_eq!(ts.len(), 96, "overworld tileset should have 96 tiles");
        }
    }
}

#[test]
fn load_font_png_1bpp() {
    if let Some(root) = get_test_asset_root() {
        let path = root.resolve(AssetCategory::Font, "font.png");
        if path.is_file() {
            let ts = load_tileset_from_png_1bpp(&path).unwrap();
            // font.png is 128×64 = 16×8 tiles = 128 tiles
            assert_eq!(ts.len(), 128, "font should have 128 tiles");
        }
    }
}

#[test]
fn load_sprite_red_png() {
    if let Some(root) = get_test_asset_root() {
        let path = root.resolve(AssetCategory::Sprite, "red.png");
        if path.is_file() {
            let ts = load_tileset_from_png(&path).unwrap();
            // red.png is 16×96 = 2×12 tiles = 24 tiles (4 frames × 6 tiles each for walking sprite)
            assert_eq!(ts.len(), 24, "red sprite should have 24 tiles");
        }
    }
}

#[test]
fn load_pokemon_front_bulbasaur() {
    if let Some(root) = get_test_asset_root() {
        let path = root.resolve(AssetCategory::PokemonFront, "bulbasaur.png");
        if path.is_file() {
            let loaded = LoadedPng::load(&path).unwrap();
            assert_eq!(loaded.dimensions, (40, 40));
            let size = PokemonSpriteSize::from_dimensions(40, 40);
            assert_eq!(size, Some(PokemonSpriteSize::Small));
            let ts = loaded.to_tileset(false).unwrap();
            assert_eq!(ts.len(), 25); // 5×5 tiles
        }
    }
}

#[test]
fn load_pokemon_front_charizard() {
    if let Some(root) = get_test_asset_root() {
        let path = root.resolve(AssetCategory::PokemonFront, "charizard.png");
        if path.is_file() {
            let loaded = LoadedPng::load(&path).unwrap();
            assert_eq!(loaded.dimensions, (56, 56));
            let size = PokemonSpriteSize::from_dimensions(56, 56);
            assert_eq!(size, Some(PokemonSpriteSize::Large));
            let ts = loaded.to_tileset(false).unwrap();
            assert_eq!(ts.len(), 49); // 7×7 tiles
        }
    }
}

#[test]
fn load_pokemon_back_bulbasaur() {
    if let Some(root) = get_test_asset_root() {
        let path = root.resolve(AssetCategory::PokemonBack, "bulbasaurb.png");
        if path.is_file() {
            let loaded = LoadedPng::load(&path).unwrap();
            assert_eq!(loaded.dimensions, (32, 32));
            let ts = loaded.to_tileset(false).unwrap();
            assert_eq!(ts.len(), 16); // 4×4 tiles
        }
    }
}

// --- ResourceManager tests ---

#[test]
fn resource_manager_new_empty_cache() {
    if let Some(root) = get_test_asset_root() {
        let mgr = ResourceManager::new(root);
        assert_eq!(mgr.cache_size(), 0);
    }
}

#[test]
fn resource_manager_load_tileset() {
    if let Some(root) = get_test_asset_root() {
        let mut mgr = ResourceManager::new(root);
        let result = mgr.load_tileset("overworld");
        if let Ok(cached) = result {
            assert_eq!(cached.source_size, (128, 48));
            assert_eq!(cached.tile_count, 96);
            assert_eq!(mgr.cache_size(), 1);
        }
    }
}

#[test]
fn resource_manager_cache_hit() {
    if let Some(root) = get_test_asset_root() {
        let mut mgr = ResourceManager::new(root);
        // First load
        let _ = mgr.load_tileset("overworld");
        assert_eq!(mgr.cache_size(), 1);
        // Second load should be cached
        let _ = mgr.load_tileset("overworld");
        assert_eq!(mgr.cache_size(), 1); // still just 1 entry
    }
}

#[test]
fn resource_manager_is_cached() {
    if let Some(root) = get_test_asset_root() {
        let mut mgr = ResourceManager::new(root);
        assert!(!mgr.is_cached(AssetCategory::Tileset, "overworld.png"));
        let _ = mgr.load_tileset("overworld");
        assert!(mgr.is_cached(AssetCategory::Tileset, "overworld.png"));
    }
}

#[test]
fn resource_manager_evict() {
    if let Some(root) = get_test_asset_root() {
        let mut mgr = ResourceManager::new(root);
        let _ = mgr.load_tileset("overworld");
        assert_eq!(mgr.cache_size(), 1);
        assert!(mgr.evict(AssetCategory::Tileset, "overworld.png"));
        assert_eq!(mgr.cache_size(), 0);
        assert!(!mgr.is_cached(AssetCategory::Tileset, "overworld.png"));
    }
}

#[test]
fn resource_manager_clear_cache() {
    if let Some(root) = get_test_asset_root() {
        let mut mgr = ResourceManager::new(root);
        let _ = mgr.load_tileset("overworld");
        let _ = mgr.load_sprite("red");
        assert!(mgr.cache_size() >= 2);
        mgr.clear_cache();
        assert_eq!(mgr.cache_size(), 0);
    }
}

#[test]
fn resource_manager_load_sprite() {
    if let Some(root) = get_test_asset_root() {
        let mut mgr = ResourceManager::new(root);
        if let Ok(cached) = mgr.load_sprite("red") {
            assert_eq!(cached.source_size, (16, 96));
            assert_eq!(cached.tile_count, 24);
        }
    }
}

#[test]
fn resource_manager_load_font() {
    if let Some(root) = get_test_asset_root() {
        let mut mgr = ResourceManager::new(root);
        if let Ok(cached) = mgr.load_font("font") {
            assert_eq!(cached.source_size, (128, 64));
            assert_eq!(cached.tile_count, 128);
        }
    }
}

#[test]
fn resource_manager_load_pokemon_front() {
    if let Some(root) = get_test_asset_root() {
        let mut mgr = ResourceManager::new(root);
        if let Ok(cached) = mgr.load_pokemon_front("bulbasaur") {
            assert_eq!(cached.source_size, (40, 40));
            assert_eq!(cached.tile_count, 25);
        }
    }
}

#[test]
fn resource_manager_load_pokemon_front_rg() {
    if let Some(root) = get_test_asset_root() {
        let mut mgr = ResourceManager::new(root);
        if let Ok(cached) = mgr.load_pokemon_front_rg("bulbasaur") {
            assert_eq!(cached.source_size, (40, 40));
            assert_eq!(cached.tile_count, 25);
        }
    }
}

#[test]
fn resource_manager_load_pokemon_back() {
    if let Some(root) = get_test_asset_root() {
        let mut mgr = ResourceManager::new(root);
        if let Ok(cached) = mgr.load_pokemon_back("bulbasaurb") {
            assert_eq!(cached.source_size, (32, 32));
            assert_eq!(cached.tile_count, 16);
        }
    }
}

#[test]
fn resource_manager_load_trainer() {
    if let Some(root) = get_test_asset_root() {
        let mut mgr = ResourceManager::new(root);
        if let Ok(cached) = mgr.load_trainer("beauty") {
            // Trainer sprites are typically 56×56 = 7×7 = 49 tiles
            assert!(cached.tile_count > 0);
        }
    }
}

#[test]
fn resource_manager_load_battle() {
    if let Some(root) = get_test_asset_root() {
        let mut mgr = ResourceManager::new(root);
        if let Ok(cached) = mgr.load_battle("move_anim_0") {
            assert!(cached.tile_count > 0);
        }
    }
}

#[test]
fn resource_manager_load_nonexistent() {
    if let Some(root) = get_test_asset_root() {
        let mut mgr = ResourceManager::new(root);
        let result = mgr.load_tileset("nonexistent_tileset_xyz");
        assert!(result.is_err());
    }
}

#[test]
fn resource_manager_generic_load() {
    if let Some(root) = get_test_asset_root() {
        let mut mgr = ResourceManager::new(root);
        if let Ok(cached) = mgr.load(AssetCategory::TownMap, "town_map") {
            assert!(cached.tile_count > 0);
        }
    }
}

#[test]
fn resource_manager_ensure_png_ext() {
    if let Some(root) = get_test_asset_root() {
        let mut mgr = ResourceManager::new(root);
        // Should work with or without .png extension
        let r1_ok = mgr.load_tileset("overworld").is_ok();
        let r2_ok = mgr.load_tileset("overworld.png").is_ok();
        // Both should succeed (or both fail if file missing)
        assert_eq!(r1_ok, r2_ok);
        assert_eq!(mgr.cache_size(), 1);
    }
}

// --- LoadedPng tests ---

#[test]
fn loaded_png_tiles_x_and_y() {
    if let Some(root) = get_test_asset_root() {
        let path = root.resolve(AssetCategory::Tileset, "overworld.png");
        if path.is_file() {
            let loaded = LoadedPng::load(&path).unwrap();
            assert_eq!(loaded.tiles_x(), 16); // 128 / 8
            assert_eq!(loaded.tiles_y(), 6); // 48 / 8
        }
    }
}

#[test]
fn loaded_png_missing_file() {
    let result = LoadedPng::load("/nonexistent/path/to/file.png");
    assert!(result.is_err());
}

// --- 2bpp roundtrip: png_to_2bpp then TileSet::from_2bpp ---

#[test]
fn png_to_2bpp_roundtrip_with_tileset() {
    // Create a synthetic image with known pixel values
    let mut img = image::GrayImage::new(8, 8);
    // Row 0: all four colors: 255, 170, 85, 0, 255, 170, 85, 0
    let colors = [255u8, 170, 85, 0, 255, 170, 85, 0];
    for (col, &val) in colors.iter().enumerate() {
        img.put_pixel(col as u32, 0, image::Luma([val]));
    }
    // Rows 1-7: all white
    for y in 1..8 {
        for x in 0..8 {
            img.put_pixel(x, y, image::Luma([255]));
        }
    }

    let dyn_img = image::DynamicImage::ImageLuma8(img);
    let data_2bpp = png_to_2bpp(&dyn_img).unwrap();
    let ts = crate::tile::TileSet::from_2bpp(&data_2bpp);
    assert_eq!(ts.len(), 1);

    let tile = ts.get(0);
    // Row 0: color indices [0, 1, 2, 3, 0, 1, 2, 3]
    assert_eq!(tile.pixels[0], [0, 1, 2, 3, 0, 1, 2, 3]);
    // Rows 1-7: all color 0
    for row in 1..8 {
        assert_eq!(tile.pixels[row], [0; 8]);
    }
}

// --- 1bpp roundtrip ---

#[test]
fn png_to_1bpp_roundtrip_with_tileset() {
    let mut img = image::GrayImage::new(8, 8);
    // Row 0: alternating B/W: 0, 255, 0, 255, 0, 255, 0, 255
    for col in 0..8 {
        let val = if col % 2 == 0 { 0u8 } else { 255 };
        img.put_pixel(col, 0, image::Luma([val]));
    }
    // Rows 1-7: all white
    for y in 1..8 {
        for x in 0..8 {
            img.put_pixel(x, y, image::Luma([255]));
        }
    }

    let dyn_img = image::DynamicImage::ImageLuma8(img);
    let data = png_to_1bpp(&dyn_img).unwrap();
    let ts = crate::tile::TileSet::from_1bpp(&data);
    assert_eq!(ts.len(), 1);

    let tile = ts.get(0);
    // Row 0: 1bpp black=color 3, white=color 0 → [3, 0, 3, 0, 3, 0, 3, 0]
    assert_eq!(tile.pixels[0], [3, 0, 3, 0, 3, 0, 3, 0]);
    // Rows 1-7: all white = color 0
    for row in 1..8 {
        assert_eq!(tile.pixels[row], [0; 8]);
    }
}
