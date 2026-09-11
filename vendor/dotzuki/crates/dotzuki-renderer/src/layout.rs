//! Lightweight layout engine for Game Boy screen coordinates.
//!
//! Game Boy has several coordinate systems that need conversion:
//! - **Screen coordinates**: Pixel positions (0-159 X, 0-143 Y)
//! - **Tile coordinates**: Tile positions (0-19 X, 0-17 Y), each tile is 8x8 pixels
//! - **OAM coordinates**: Sprite position registers with built-in offsets
//!
//! This module provides utilities to convert between these systems and define
//! common layout positions.

/// OAM (Object Attribute Memory) offset constants.
/// Game Boy sprites use these offsets to allow sprites to be partially off-screen.
pub const OAM_Y_OFFSET: u32 = 16;
pub const OAM_X_OFFSET: u32 = 8;

// ============================================================================
// Coordinate conversions
// ============================================================================

/// Convert tile coordinates to pixel coordinates.
/// `tile_x` and `tile_y` are in tile units (0-19, 0-17).
/// `tile_size` is the size of a tile in pixels (typically 8 for Game Boy).
/// Returns pixel coordinates of the tile's top-left corner.
#[inline]
pub fn tile_to_pixel(tile_x: u32, tile_y: u32, tile_size: u32) -> (u32, u32) {
    (tile_x * tile_size, tile_y * tile_size)
}

/// Convert pixel coordinates to tile coordinates.
/// `tile_size` is the size of a tile in pixels (typically 8 for Game Boy).
/// Returns the tile that contains the given pixel.
#[inline]
pub fn pixel_to_tile(pixel_x: u32, pixel_y: u32, tile_size: u32) -> (u32, u32) {
    (pixel_x / tile_size, pixel_y / tile_size)
}

/// Convert OAM coordinates to screen coordinates.
/// OAM Y = screen_y + oam_y_offset, OAM X = screen_x + oam_x_offset.
/// This allows sprites to be positioned partially off the top/left edges.
/// For Game Boy: oam_y_offset = 16, oam_x_offset = 8.
#[inline]
pub fn oam_to_screen(oam_y: u32, oam_x: u32, oam_y_offset: u32, oam_x_offset: u32) -> (u32, u32) {
    let screen_y = oam_y.saturating_sub(oam_y_offset);
    let screen_x = oam_x.saturating_sub(oam_x_offset);
    (screen_x, screen_y)
}

/// Convert screen coordinates to OAM coordinates.
/// OAM Y = screen_y + oam_y_offset, OAM X = screen_x + oam_x_offset.
/// For Game Boy: oam_y_offset = 16, oam_x_offset = 8.
#[inline]
pub fn screen_to_oam(
    screen_y: u32,
    screen_x: u32,
    oam_y_offset: u32,
    oam_x_offset: u32,
) -> (u32, u32) {
    (screen_y + oam_y_offset, screen_x + oam_x_offset)
}

/// Convert OAM coordinates (as i16 for signed math) to screen coordinates.
/// `screen_width` and `screen_height` are the screen dimensions in pixels.
/// Returns None if the sprite is completely off-screen.
#[inline]
pub fn oam_to_screen_signed(
    oam_y: i16,
    oam_x: i16,
    screen_width: u32,
    screen_height: u32,
) -> Option<(i32, i32)> {
    let screen_y = oam_y as i32 - OAM_Y_OFFSET as i32;
    let screen_x = oam_x as i32 - OAM_X_OFFSET as i32;

    // Check if sprite is visible (within screen bounds)
    if screen_y < -16 || screen_y >= screen_height as i32 {
        return None;
    }
    if screen_x < -8 || screen_x >= screen_width as i32 {
        return None;
    }

    Some((screen_x, screen_y))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_to_pixel() {
        assert_eq!(tile_to_pixel(0, 0, 8), (0, 0));
        assert_eq!(tile_to_pixel(2, 1, 8), (16, 8));
        assert_eq!(tile_to_pixel(7, 8, 8), (56, 64));
        assert_eq!(tile_to_pixel(5, 10, 8), (40, 80));
    }

    #[test]
    fn test_oam_to_screen() {
        // Player sprite from the title screen: OAM Y=$60, X=$5a
        assert_eq!(
            oam_to_screen(0x60, 0x5a, OAM_Y_OFFSET, OAM_X_OFFSET),
            (82, 80)
        );

        // Test edge cases
        assert_eq!(oam_to_screen(16, 8, OAM_Y_OFFSET, OAM_X_OFFSET), (0, 0));
        assert_eq!(oam_to_screen(0, 0, OAM_Y_OFFSET, OAM_X_OFFSET), (0, 0)); // saturating_sub
    }

    #[test]
    fn test_screen_to_oam() {
        assert_eq!(screen_to_oam(80, 82, OAM_Y_OFFSET, OAM_X_OFFSET), (96, 90));
        // (Y, X) -> (OAM_Y, OAM_X)
    }
}
