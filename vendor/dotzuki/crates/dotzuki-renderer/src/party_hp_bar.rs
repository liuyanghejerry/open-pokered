//! Party screen HP bar renderer — pixel-accurate segmented bar into FrameBuffer.
//!
//! Renders the same 9-tile HP bar layout as [`crate::battle_scene::draw_hp_bar`],
//! but writes 72×8 pixel rows directly into a [`FrameBuffer`] instead of a
//! [`ScreenTileBuffer`].  Tile graphics are fetched from `font_battle_extra.png`
//! and indexed at `tile_id - 0x62` (matching the `game_font` module).

#[cfg(not(target_os = "none"))]
use std::sync::Mutex;
#[cfg(target_os = "none")]
// thumbv4t has no atomics; single-threaded game loop.
#[cfg(target_os = "none")]
mod gba_mutex {
    use core::cell::UnsafeCell;
    pub struct Mutex<T>(UnsafeCell<T>);
    // Sound under the single-threaded bare-metal contract.
    unsafe impl<T> Sync for Mutex<T> {}
    impl<T> Mutex<T> {
        pub const fn new(v: T) -> Self {
            Self(UnsafeCell::new(v))
        }
        pub fn lock(&self) -> Result<&mut T, &'static str> {
            Ok(unsafe { &mut *self.0.get() })
        }
    }
}
#[cfg(target_os = "none")]
use gba_mutex::Mutex;

use crate::asset_provider::ResourceProvider;
use crate::battle_scene::{
    calc_hp_bar_pixels, BATTLE_HP_BAR_TILES, TILE_HP_BAR_LEFT, TILE_HP_EMPTY,
    TILE_HP_END_CAP_BATTLE, TILE_HP_FULL, TILE_HP_LABEL, TILE_HP_PARTIAL_BASE,
};
use crate::palette::{GbColor, Palette};
use crate::tile::{TileSet, TILE_PIXELS};
use crate::FbSurface;
use dotzuki_engine::render::Rgba;

const TILESET_BASE: u8 = 0x62;
const ASSET_FILENAME: &str = "font_battle_extra.png";

/// Palette for the party screen HP bar where colour 0 matches the screen
/// background (`InkColor::White` = #E0E0E0) so the HP bar tiles blend
/// seamlessly without a visible white rectangle.
const PARTY_HP_PALETTE: Palette = Palette {
    colors: [
        Rgba::rgb(0xE0, 0xE0, 0xE0), // match InkColor::White
        Rgba::rgb(0xAA, 0xAA, 0xAA),
        Rgba::rgb(0x55, 0x55, 0x55),
        Rgba::rgb(0x00, 0x00, 0x00),
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
    ],
    count: 4,
    _phantom: core::marker::PhantomData,
};

/// Convert a Game Boy tile ID to its 0-based index in `font_battle_extra.png`.
#[inline]
fn tileset_index(tile_id: u8) -> usize {
    (tile_id.wrapping_sub(TILESET_BASE)) as usize
}

static HP_BAR_TILESET: Mutex<Option<&'static TileSet>> = Mutex::new(None);

fn load_tileset(provider: &mut dyn ResourceProvider) -> Result<&'static TileSet, String> {
    {
        let guard = HP_BAR_TILESET
            .lock()
            .map_err(|e| format!("HP bar cache lock poisoned: {}", e))?;
        if let Some(ts) = *guard {
            return Ok(ts);
        }
    }
    let tileset = provider
        .load_asset_2bpp("font", ASSET_FILENAME)
        .map_err(|e| format!("failed to load {}: {}", ASSET_FILENAME, e))?
        .clone();
    let leaked: &'static TileSet = Box::leak(Box::new(tileset));
    let mut guard = HP_BAR_TILESET
        .lock()
        .map_err(|e| format!("HP bar cache lock poisoned: {}", e))?;
    *guard = Some(leaked);
    Ok(leaked)
}

fn blit_tile(
    fb: &mut impl FbSurface,
    tileset: &TileSet,
    index: usize,
    x: u32,
    y: u32,
    palette: &crate::palette::Palette,
) {
    let fb_h = fb.height();
    let fb_w = fb.width();
    let tile = tileset.get(index);
    for row in 0..TILE_PIXELS {
        let py = y + row as u32;
        if py >= fb_h {
            continue;
        }
        for col in 0..TILE_PIXELS {
            let px = x + col as u32;
            if px >= fb_w {
                continue;
            }
            let idx = tile.get(row, col);
            fb.set_pixel(px, py, palette.color(GbColor::from_u8(idx)));
        }
    }
}

fn segment_tile(remaining: &mut u32) -> u8 {
    if *remaining >= 8 {
        *remaining -= 8;
        TILE_HP_FULL
    } else if *remaining > 0 {
        let partial = *remaining;
        *remaining = 0;
        TILE_HP_PARTIAL_BASE + partial as u8
    } else {
        TILE_HP_EMPTY
    }
}

/// Draw a pixel-accurate, segmented HP bar for the party screen.
///
/// 9 tiles (72×8 px): `[HP:] [L_EDGE] [6 segments] [END_CAP]`.
/// Uses the same tile IDs as [`draw_hp_bar`](crate::battle_scene::draw_hp_bar).
///
/// `x_px, y_px` is the top-left pixel of the `HP:` label tile.
pub fn draw_party_hp_bar(
    fb: &mut impl FbSurface,
    provider: &mut dyn ResourceProvider,
    x_px: u32,
    y_px: u32,
    current_hp: u16,
    max_hp: u16,
) -> Result<(), String> {
    let tileset = load_tileset(provider)?;
    let pixels = calc_hp_bar_pixels(current_hp, max_hp);
    let pal = &PARTY_HP_PALETTE;

    blit_tile(fb, tileset, tileset_index(TILE_HP_LABEL), x_px, y_px, pal);

    blit_tile(
        fb,
        tileset,
        tileset_index(TILE_HP_BAR_LEFT),
        x_px + 8,
        y_px,
        pal,
    );

    let mut remaining = pixels;
    for i in 0..BATTLE_HP_BAR_TILES {
        let tid = segment_tile(&mut remaining);
        blit_tile(
            fb,
            tileset,
            tileset_index(tid),
            x_px + 16 + i * 8,
            y_px,
            pal,
        );
    }

    blit_tile(
        fb,
        tileset,
        tileset_index(TILE_HP_END_CAP_BATTLE),
        x_px + 16 + BATTLE_HP_BAR_TILES * 8,
        y_px,
        pal,
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tileset_index_maps_correctly() {
        assert_eq!(tileset_index(TILE_HP_BAR_LEFT), 0);
        assert_eq!(tileset_index(TILE_HP_EMPTY), 1);
        assert_eq!(tileset_index(TILE_HP_FULL), 9);
        assert_eq!(tileset_index(TILE_HP_END_CAP_BATTLE), 11);
        assert_eq!(tileset_index(TILE_HP_LABEL), 15);
    }

    #[test]
    fn segment_tile_full_and_empty() {
        let mut r = 48;
        assert_eq!(segment_tile(&mut r), TILE_HP_FULL);
        assert_eq!(r, 40);

        let mut r = 0;
        assert_eq!(segment_tile(&mut r), TILE_HP_EMPTY);
    }

    #[test]
    fn segment_tile_partial() {
        let mut r = 5;
        assert_eq!(segment_tile(&mut r), TILE_HP_PARTIAL_BASE + 5);
        assert_eq!(r, 0);
    }
}
