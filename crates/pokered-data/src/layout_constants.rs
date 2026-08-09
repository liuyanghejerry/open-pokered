//! Pokémon-specific layout constants for title screen, intro scene, and
//! copyright screen.
//!
//! These constants were extracted from `dotzuki-renderer::layout` because they
//! are game-specific data, not generic coordinate-conversion utilities.

use crate::oam_constants::{SCREEN_WIDTH_PX, TILE_SIZE_PX};

/// OAM (Object Attribute Memory) offset constants — copied locally because
/// `dotzuki-renderer::layout` is not a dependency of `pokered-data`.
pub const OAM_Y_OFFSET: u32 = 16;
pub const OAM_X_OFFSET: u32 = 8;

/// Local aliases matching the original naming convention.
const SCREEN_WIDTH: u32 = SCREEN_WIDTH_PX as u32;
const TILE_SIZE: u32 = TILE_SIZE_PX;

/// Title screen layout constants.
/// Based on `engine/movie/title.asm` positions.
pub mod title_screen {
    use super::*;

    /// Pokemon logo position (from hlcoord 2, 1).
    /// Logo is 16 tiles wide × 7 tiles tall.
    pub const LOGO_TILE_X: u32 = 2;
    pub const LOGO_TILE_Y: u32 = 1;
    pub const LOGO_PIXEL_X: u32 = LOGO_TILE_X * TILE_SIZE; // 16
    pub const LOGO_PIXEL_Y: u32 = LOGO_TILE_Y * TILE_SIZE; // 8

    /// Version text Y position (from hlcoord _, 8).
    /// "Red Version" (10 tiles = 80px) or "Blue Version" (8 tiles = 64px).
    pub const VERSION_TILE_Y: u32 = 8;
    pub const VERSION_PIXEL_Y: u32 = VERSION_TILE_Y * TILE_SIZE; // 64

    /// Compute the centered X pixel position for version text of a given pixel width.
    /// Red Version is 80px wide (10 tiles), Blue Version is 64px wide (8 tiles).
    pub fn version_centered_x(width_px: u32) -> u32 {
        (SCREEN_WIDTH - width_px) / 2
    }

    /// Pokemon sprite position (from hlcoord 5, 10).
    /// This is where the front sprite is drawn on the title screen.
    pub const POKEMON_TILE_X: u32 = 5;
    pub const POKEMON_TILE_Y: u32 = 10;
    pub const POKEMON_PIXEL_X: u32 = POKEMON_TILE_X * TILE_SIZE; // 40
    pub const POKEMON_PIXEL_Y: u32 = POKEMON_TILE_Y * TILE_SIZE; // 80

    /// Player character sprite position (from OAM Y=$60, X=$5a).
    /// Player sprite is 5 tiles wide × 7 tiles tall (40×56 pixels).
    pub const PLAYER_OAM_Y: u32 = 0x60; // 96
    pub const PLAYER_OAM_X: u32 = 0x5a; // 90

    /// Get player sprite screen position from OAM values.
    pub fn player_screen_pos() -> (u32, u32) {
        (
            PLAYER_OAM_X.saturating_sub(OAM_X_OFFSET),
            PLAYER_OAM_Y.saturating_sub(OAM_Y_OFFSET),
        )
    }

    /// Copyright text position (from hlcoord 2, 17).
    pub const COPYRIGHT_TILE_X: u32 = 2;
    pub const COPYRIGHT_TILE_Y: u32 = 17;
    pub const COPYRIGHT_PIXEL_X: u32 = COPYRIGHT_TILE_X * TILE_SIZE; // 16
    pub const COPYRIGHT_PIXEL_Y: u32 = COPYRIGHT_TILE_Y * TILE_SIZE; // 136
}

/// Intro scene (Gengar vs Nidorino fight) layout constants.
/// From `engine/movie/intro.asm`.
pub mod intro_scene {
    use super::*;

    /// Gengar tilemap is placed at hlcoord 13, 7 → tile column 13, tile row 7.
    pub const GENGAR_TILE_X: u32 = 13;
    pub const GENGAR_TILE_Y: u32 = 7;
    pub const GENGAR_PIXEL_X: u32 = GENGAR_TILE_X * TILE_SIZE; // 104
    pub const GENGAR_PIXEL_Y: u32 = GENGAR_TILE_Y * TILE_SIZE; // 56

    /// Gengar tilemap dimensions: 7×7 tiles.
    pub const GENGAR_TILES_W: u32 = 7;
    pub const GENGAR_TILES_H: u32 = 7;

    /// Black bar regions: top 4 tile rows, bottom 4 tile rows.
    pub const BLACK_BAR_TOP_ROWS: u32 = 4;
    pub const BLACK_BAR_BOTTOM_START_ROW: u32 = 14;
    pub const BLACK_BAR_BOTTOM_ROWS: u32 = 4;
    pub const BLACK_BAR_TOP_PIXEL_H: u32 = BLACK_BAR_TOP_ROWS * TILE_SIZE; // 32
    pub const BLACK_BAR_BOTTOM_PIXEL_Y: u32 = BLACK_BAR_BOTTOM_START_ROW * TILE_SIZE; // 112

    /// Nidorino initial OAM base position.
    /// wBaseCoordX = 0, wBaseCoordY = 80 in the original.
    pub const NIDORINO_INIT_BASE_X: i32 = 0;
    pub const NIDORINO_INIT_BASE_Y: i32 = 80;

    /// Nidorino sprite grid: 6×6 tiles (6 columns × 6 rows of 8×8 sprites).
    pub const NIDORINO_GRID_COLS: u32 = 6;
    pub const NIDORINO_GRID_ROWS: u32 = 6;
    pub const NIDORINO_SPRITE_COUNT: u32 = NIDORINO_GRID_COLS * NIDORINO_GRID_ROWS; // 36

    /// Visible fight area: rows 4–13 (10 tile rows between the black bars).
    pub const FIGHT_AREA_TOP_ROW: u32 = 4;
    pub const FIGHT_AREA_BOTTOM_ROW: u32 = 14;
    pub const FIGHT_AREA_PIXEL_Y: u32 = FIGHT_AREA_TOP_ROW * TILE_SIZE; // 32
    pub const FIGHT_AREA_PIXEL_H: u32 = (FIGHT_AREA_BOTTOM_ROW - FIGHT_AREA_TOP_ROW) * TILE_SIZE;
    // 80
}

/// Copyright splash screen positions.
/// From `engine/movie/title.asm` LoadCopyrightTiles.
pub mod copyright_screen {
    use super::*;

    /// Copyright text position (from hlcoord 2, 7).
    pub const TEXT_TILE_X: u32 = 2;
    pub const TEXT_TILE_Y: u32 = 7;
    pub const TEXT_PIXEL_X: u32 = TEXT_TILE_X * TILE_SIZE; // 16
    pub const TEXT_PIXEL_Y: u32 = TEXT_TILE_Y * TILE_SIZE; // 56
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_screen_positions() {
        // Logo at tile (2, 1) = pixel (16, 8)
        assert_eq!(title_screen::LOGO_PIXEL_X, 16);
        assert_eq!(title_screen::LOGO_PIXEL_Y, 8);

        // Version text at tile row 8, centered horizontally
        // Red Version: 10 tiles (80px) -> centered at (160-80)/2 = 40
        // Blue Version: 8 tiles (64px) -> centered at (160-64)/2 = 48
        assert_eq!(title_screen::version_centered_x(80), 40);
        assert_eq!(title_screen::version_centered_x(64), 48);
        assert_eq!(title_screen::VERSION_PIXEL_Y, 64);

        // Pokemon at tile (5, 10) = pixel (40, 80)
        assert_eq!(title_screen::POKEMON_PIXEL_X, 40);
        assert_eq!(title_screen::POKEMON_PIXEL_Y, 80);

        // Player from OAM
        assert_eq!(title_screen::player_screen_pos(), (82, 80));
    }
}
