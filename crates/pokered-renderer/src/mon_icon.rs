//! Pokémon-side party-menu icon loading.
//!
//! The generic engine loader (`dotzuki_renderer::mon_icon`) maps
//! [`IconKind::Water`]/[`IconKind::Ball`]/[`IconKind::Helix`] to
//! `sprites/fish.png` / `sprites/ball.png`, which do not exist in the
//! pret/pokered `gfx/` tree — those kinds silently failed to load, leaving
//! party-list entries (e.g. Lapras) without an icon. This module re-implements
//! the loader with the pokered asset names:
//!
//! | Kind        | Asset                      |
//! |-------------|----------------------------|
//! | Mon         | `sprites/monster.png`      |
//! | Fairy       | `sprites/fairy.png`        |
//! | Bird        | `sprites/bird.png`         |
//! | Water       | `sprites/seel.png`         |
//! | Ball        | `sprites/poke_ball.png`    |
//! | Helix       | `sprites/fossil.png`       |
//! | Bug/Grass/… | `icons/{bug,plant,…}.png`  |
//!
//! Everything else (frame animation policy, 8-wide mirror expansion, blit)
//! mirrors the engine module; `draw_mon_icon` is re-exported unchanged.

use crate::alloc_prelude::*;

#[cfg(not(target_os = "none"))]
pub use dotzuki_renderer::mon_icon::{IconFrame, draw_mon_icon};
#[cfg(not(target_os = "none"))]
use dotzuki_renderer::asset_provider::ResourceProvider;
use crate::sync_compat::Mutex;
use crate::hash_compat::HashMap;

use dotzuki_renderer::icon::IconKind;
use dotzuki_renderer::tile::TileSet;

/// Animation frame + asset coordinates for one icon kind.
struct IconAsset {
    category: &'static str,
    filename: &'static str,
    start_tile: usize,
    tile_count: usize,
}

fn asset_for(kind: IconKind, frame: IconFrame) -> IconAsset {
    match (kind, frame) {
        (IconKind::Mon, _) => IconAsset {
            category: "sprites",
            filename: "monster.png",
            start_tile: 12,
            tile_count: 4,
        },
        (IconKind::Fairy, _) => IconAsset {
            category: "sprites",
            filename: "fairy.png",
            start_tile: 12,
            tile_count: 4,
        },
        (IconKind::Bird, _) => IconAsset {
            category: "sprites",
            filename: "bird.png",
            start_tile: 12,
            tile_count: 4,
        },
        (IconKind::Water, _) => IconAsset {
            category: "sprites",
            filename: "seel.png",
            start_tile: 12,
            tile_count: 4,
        },
        (IconKind::Ball, _) => IconAsset {
            category: "sprites",
            filename: "poke_ball.png",
            start_tile: 0,
            tile_count: 4,
        },
        (IconKind::Helix, _) => IconAsset {
            category: "sprites",
            filename: "fossil.png",
            start_tile: 0,
            tile_count: 4,
        },
        (IconKind::Bug, IconFrame::Frame1) => icon_asset("bug.png", 2),
        (IconKind::Bug, IconFrame::Frame2) => icon_asset("bug.png", 4),
        (IconKind::Grass, IconFrame::Frame1) => icon_asset("plant.png", 2),
        (IconKind::Grass, IconFrame::Frame2) => icon_asset("plant.png", 4),
        (IconKind::Snake, IconFrame::Frame1) => icon_asset("snake.png", 2),
        (IconKind::Snake, IconFrame::Frame2) => icon_asset("snake.png", 4),
        (IconKind::Quadruped, IconFrame::Frame1) => icon_asset("quadruped.png", 2),
        (IconKind::Quadruped, IconFrame::Frame2) => icon_asset("quadruped.png", 4),
    }
}

const fn icon_asset(filename: &'static str, start_tile: usize) -> IconAsset {
    IconAsset {
        category: "icons",
        filename,
        start_tile,
        tile_count: 2,
    }
}

// Cache key includes both the icon kind and which frame, since the two
// frames are different bitmaps.
#[cfg(not(target_os = "none"))]
static CACHE: Mutex<Option<HashMap<(IconKind, IconFrame), &'static TileSet>>> = Mutex::new(None);

/// The 16-wide icons are stored column-major in the Game Boy OAM; reorder the
/// four source tiles so `draw_mon_icon`'s 2×2 blit renders them correctly.
fn extract_16wide(source: &TileSet, start: usize) -> TileSet {
    let indices = [start, start + 2, start + 1, start + 3];
    let mut ts = TileSet::blank(4);
    for (i, &idx) in indices.iter().enumerate() {
        ts.set(i, source.get(idx).clone());
    }
    ts
}

/// The 8-wide icons (bug, plant, snake, quadruped) are stored column-major in
/// the Game Boy OAM with X-flip symmetry: the left 8 px form the icon's edge
/// and the right 8 px are a mirrored copy. The two source tiles (top half,
/// bottom half) are expanded to four by adding X-flipped copies for the right
/// column.
fn extract_8wide(source: &TileSet, start: usize) -> TileSet {
    let top = source.get(start).clone();
    let bot = source.get(start + 1).clone();
    let top_flip = top.flip_x();
    let bot_flip = bot.flip_x();
    let mut ts = TileSet::blank(4);
    ts.set(0, top);
    ts.set(1, bot);
    ts.set(2, top_flip);
    ts.set(3, bot_flip);
    ts
}

#[cfg(not(target_os = "none"))]
pub fn load_mon_icon_tiles(
    provider: &mut dyn ResourceProvider,
    kind: IconKind,
    frame: IconFrame,
) -> Result<&'static TileSet, String> {
    let key = (kind, frame);
    {
        let guard = CACHE
            .lock()
            .map_err(|e| format!("cache lock poisoned: {}", e))?;
        if let Some(ref map) = *guard {
            if let Some(tiles) = map.get(&key) {
                return Ok(tiles);
            }
        }
    }

    let asset = asset_for(kind, frame);
    let source = provider
        .load_asset(asset.category, asset.filename)
        .map_err(|e| format!("failed to load {}: {}", asset.filename, e))?;
    if source.len() < asset.start_tile + asset.tile_count {
        // For 8-wide icons whose frame2 slot doesn't exist in the asset,
        // silently fall back to frame1 so we never crash the party screen.
        if frame == IconFrame::Frame2 && asset.tile_count == 2 {
            return load_mon_icon_tiles(provider, kind, IconFrame::Frame1);
        }
        return Err(format!(
            "{} has only {} tiles, need at least {}",
            asset.filename,
            source.len(),
            asset.start_tile + asset.tile_count
        ));
    }

    let icon_tiles = if asset.tile_count == 2 {
        extract_8wide(source, asset.start_tile)
    } else {
        extract_16wide(source, asset.start_tile)
    };

    let leaked: &'static TileSet = Box::leak(Box::new(icon_tiles));
    let mut guard = CACHE
        .lock()
        .map_err(|e| format!("cache lock poisoned: {}", e))?;
    let map = guard.get_or_insert_with(HashMap::default);
    map.insert(key, leaked);
    Ok(leaked)
}

// ── Bare metal (GBA) ─────────────────────────────────────────────────────────
//
// The engine's `mon_icon` module is gpu-gated (std::sync cache), so on bare
// metal this local twin provides the same surface: the frame enum, the
// 2×2-tile blit and a registry-backed loader (the icon sheets ship in the
// pre-converted 2bpp registry under `sprites/` / `icons/`).

#[cfg(target_os = "none")]
mod gba {
    use crate::alloc_prelude::*;
    use crate::hash_compat::HashMap;

    use dotzuki_renderer::FbSurface;
    use dotzuki_renderer::icon::IconKind;
    use dotzuki_renderer::palette::{GbColor, Palette};
    use dotzuki_renderer::tile::{TileSet, TILE_PIXELS};

    use super::{asset_for, extract_16wide, extract_8wide};
    use crate::sync_compat::Mutex;

    /// Animation frame for the party-screen mon icon (engine twin).
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub enum IconFrame {
        Frame1,
        Frame2,
    }

    impl IconFrame {
        pub fn from_counter(counter: u64, period: u64) -> Self {
            if period == 0 || (counter / period) % 2 == 0 {
                IconFrame::Frame1
            } else {
                IconFrame::Frame2
            }
        }
    }

    static CACHE: Mutex<Option<HashMap<(IconKind, IconFrame), &'static TileSet>>> =
        Mutex::new(None);

    pub fn load_mon_icon_tiles(
        _provider: &mut dyn dotzuki_renderer::asset_provider::ResourceProvider,
        kind: IconKind,
        frame: IconFrame,
    ) -> Result<&'static TileSet, String> {
        let key = (kind, frame);
        {
            let guard = CACHE.lock().unwrap();
            if let Some(tiles) = guard.as_ref().and_then(|m| m.get(&key)) {
                return Ok(tiles);
            }
        }

        let asset = asset_for(kind, frame);
        let subdir = asset.category;
        let stem = asset.filename.trim_end_matches(".png");
        let bytes = crate::gba_assets::get_preconverted_asset(subdir, stem)
            .ok_or_else(|| format!("asset not in registry: {}/{}", subdir, stem))?;
        let source = if subdir == "font" {
            TileSet::from_1bpp(bytes)
        } else {
            TileSet::from_2bpp(bytes)
        };
        if source.len() < asset.start_tile + asset.tile_count {
            // For 8-wide icons whose frame2 slot doesn't exist in the asset,
            // silently fall back to frame1 so we never crash the party screen.
            if frame == IconFrame::Frame2 && asset.tile_count == 2 {
                return load_mon_icon_tiles(_provider, kind, IconFrame::Frame1);
            }
            return Err(format!(
                "{} has only {} tiles, need at least {}",
                asset.filename,
                source.len(),
                asset.start_tile + asset.tile_count
            ));
        }

        let icon_tiles = if asset.tile_count == 2 {
            extract_8wide(&source, asset.start_tile)
        } else {
            extract_16wide(&source, asset.start_tile)
        };
        let leaked: &'static TileSet = Box::leak(Box::new(icon_tiles));
        let mut guard = CACHE.lock().unwrap();
        guard.get_or_insert_with(HashMap::default).insert(key, leaked);
        Ok(leaked)
    }

    /// 2×2-tile icon blit (engine twin: color 0 transparent).
    pub fn draw_mon_icon(fb: &mut impl FbSurface, tiles: &TileSet, x: u32, y: u32, palette: &Palette) {
        let fb_h = fb.height();
        let fb_w = fb.width();
        let positions = [(0u32, 0u32), (0, 1), (1, 0), (1, 1)];
        for (i, (col, row)) in positions.iter().enumerate() {
            let tile = tiles.get(i);
            let base_x = x + col * TILE_PIXELS as u32;
            let base_y = y + row * TILE_PIXELS as u32;
            for r in 0..TILE_PIXELS {
                let screen_y = base_y + r as u32;
                if screen_y >= fb_h {
                    continue;
                }
                for c in 0..TILE_PIXELS {
                    let screen_x = base_x + c as u32;
                    if screen_x >= fb_w {
                        continue;
                    }
                    let color_idx = tile.get(r, c);
                    if color_idx == 0 {
                        continue;
                    }
                    fb.set_pixel(
                        screen_x,
                        screen_y,
                        palette.color(GbColor::from_u8(color_idx)),
                    );
                }
            }
        }
    }
}

#[cfg(target_os = "none")]
pub use gba::{draw_mon_icon, load_mon_icon_tiles, IconFrame};
