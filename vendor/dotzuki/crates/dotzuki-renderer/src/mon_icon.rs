use dotzuki_engine::hash::HashMap;
use std::sync::Mutex;

use crate::asset_provider::ResourceProvider;
use crate::palette::{GbColor, Palette};
use crate::tile::{TileSet, TILE_PIXELS};
use crate::FbSurface;

pub use crate::icon::IconKind;

/// Animation frame for the party-screen mon icon.
///
/// In the original game, the *selected* party mon's icon alternates between
/// `Frame1` and `Frame2` every few VBlanks (faster the lower its HP).
/// Non-selected icons stay on `Frame1`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum IconFrame {
    Frame1,
    Frame2,
}

impl IconFrame {
    /// Pick a frame from a free-running counter.  Frame swaps every
    /// `period` ticks (e.g. 16 ≈ ~4 swaps/second at 60fps, matching the
    /// original mid-HP animation speed).
    pub fn from_counter(counter: u64, period: u64) -> Self {
        if period == 0 || (counter / period) % 2 == 0 {
            IconFrame::Frame1
        } else {
            IconFrame::Frame2
        }
    }
}

// Cache key includes both the icon kind and which frame, since the two
// frames are different bitmaps.
static CACHE: Mutex<Option<HashMap<(IconKind, IconFrame), &'static TileSet>>> = Mutex::new(None);

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
            filename: "fish.png",
            start_tile: 0,
            tile_count: 4,
        },
        (IconKind::Ball, _) => IconAsset {
            category: "sprites",
            filename: "ball.png",
            start_tile: 0,
            tile_count: 4,
        },
        (IconKind::Helix, _) => IconAsset {
            category: "sprites",
            filename: "ball.png",
            start_tile: 0,
            tile_count: 4,
        },
        (IconKind::Bug, IconFrame::Frame1) => IconAsset {
            category: "icons",
            filename: "bug.png",
            start_tile: 2,
            tile_count: 2,
        },
        (IconKind::Bug, IconFrame::Frame2) => IconAsset {
            category: "icons",
            filename: "bug.png",
            start_tile: 4,
            tile_count: 2,
        },
        (IconKind::Grass, IconFrame::Frame1) => IconAsset {
            category: "icons",
            filename: "plant.png",
            start_tile: 2,
            tile_count: 2,
        },
        (IconKind::Grass, IconFrame::Frame2) => IconAsset {
            category: "icons",
            filename: "plant.png",
            start_tile: 4,
            tile_count: 2,
        },
        (IconKind::Snake, IconFrame::Frame1) => IconAsset {
            category: "icons",
            filename: "snake.png",
            start_tile: 2,
            tile_count: 2,
        },
        (IconKind::Snake, IconFrame::Frame2) => IconAsset {
            category: "icons",
            filename: "snake.png",
            start_tile: 4,
            tile_count: 2,
        },
        (IconKind::Quadruped, IconFrame::Frame1) => IconAsset {
            category: "icons",
            filename: "quadruped.png",
            start_tile: 2,
            tile_count: 2,
        },
        (IconKind::Quadruped, IconFrame::Frame2) => IconAsset {
            category: "icons",
            filename: "quadruped.png",
            start_tile: 4,
            tile_count: 2,
        },
    }
}

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
/// and the right 8 px are a mirrored copy.  We replicate that here so
/// `draw_mon_icon` can always use the same 2×2 blit loop.  The two source
/// tiles (top half, bottom half) are expanded to four by adding X-flipped
/// copies for the right column.
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

pub fn load_mon_icon_tiles(
    provider: &mut dyn ResourceProvider,
    kind: IconKind,
    frame: IconFrame,
) -> Result<&TileSet, String> {
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
    let loaded = provider
        .load_asset(asset.category, asset.filename)
        .map_err(|e| format!("failed to load {}: {}", asset.filename, e))?;

    let source = loaded;
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

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render::Rgba;

    #[test]
    fn frame_from_counter_alternates() {
        assert_eq!(IconFrame::from_counter(0, 16), IconFrame::Frame1);
        assert_eq!(IconFrame::from_counter(15, 16), IconFrame::Frame1);
        assert_eq!(IconFrame::from_counter(16, 16), IconFrame::Frame2);
        assert_eq!(IconFrame::from_counter(31, 16), IconFrame::Frame2);
        assert_eq!(IconFrame::from_counter(32, 16), IconFrame::Frame1);
    }

    #[test]
    fn draw_mon_icon_respects_transparent_color0() {
        let mut fb = crate::FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(160, 144),
            Rgba::BLACK,
        );
        let ts = TileSet::blank(4);
        draw_mon_icon(
            &mut fb,
            &ts,
            0,
            0,
            &crate::palette::GRAYSCALE_SPRITE_PALETTE,
        );
        // All blank tiles should be transparent (color 0), so framebuffer stays black
        for dy in 0..16u32 {
            for dx in 0..16u32 {
                assert_eq!(
                    fb.get_pixel(dx, dy),
                    Some(Rgba::BLACK),
                    "blank icon area should remain background color at ({},{})",
                    dx,
                    dy
                );
            }
        }
    }
}
