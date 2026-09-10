//! Multi-layer map compositing.
//!
//! Provides [`render_layers`], which takes a sorted stack of [`MapLayer`]s,
//! renders each one's tilemap into a temporary framebuffer via a caller-supplied
//! tile-to-colour callback, then composites all layers bottom-to-top onto a
//! single output framebuffer.

use crate::tile::RgbaTile;
use crate::{DirtyRegion, FbSurface, FrameBuffer, TILE_SIZE};
use dotzuki_engine::render::Rgba;
use dotzuki_engine::render::{BlendMode, MapLayer};
use dotzuki_engine::tilemap::TilemapEntry;
use dotzuki_engine::hash::HashMap;

const TILE_PIXELS: u32 = TILE_SIZE;

/// A cache of pre-rendered RGBA tiles for `no_animation` layers.
///
/// Maps `(tile_id, palette_group)` → fully rendered RGBA tile, avoiding
/// per-pixel calls to the `tile_color` closure on every frame.
pub struct LayerTileCache {
    entries: HashMap<(u16, u8), RgbaTile>,
}

impl LayerTileCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::default(),
        }
    }

    /// Invalidate all cached entries (call when tileset or palette changes).
    pub fn invalidate(&mut self) {
        self.entries.clear();
    }

    /// Get a cached tile, or populate it by calling `tile_color` for all 64 pixels.
    pub fn get_or_insert<F>(&mut self, tile_id: u16, pal_group: u8, tile_color: &F) -> &RgbaTile
    where
        F: Fn(u16, u8, u8, u8) -> Rgba,
    {
        let key = (tile_id, pal_group);
        self.entries.entry(key).or_insert_with(|| {
            let mut pixels = [[Rgba::TRANSPARENT; TILE_PIXELS as usize]; TILE_PIXELS as usize];
            for py in 0..(TILE_PIXELS as u8) {
                for px in 0..(TILE_PIXELS as u8) {
                    pixels[py as usize][px as usize] = tile_color(tile_id, pal_group, px, py);
                }
            }
            RgbaTile { pixels }
        })
    }
}

impl Default for LayerTileCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Render a stack of map layers and composite them onto `fb`.
///
/// Always performs a **full redraw** across the whole viewport. With zero or
/// two-plus visible layers, `fb` is cleared to transparent first and the layers
/// are composited into a self-contained image. With exactly one visible layer
/// (the fast path), the layer is composited directly onto `fb` *without*
/// clearing, so fully-transparent tile pixels (alpha 0) reveal whatever the
/// caller drew beneath — a background fill, a parallax pass, etc. — instead of
/// stamping transparent-black over it.
///
/// An earlier version gated rendering on `fb.dirty_region` (skipping the frame
/// when nothing was marked dirty and clearing the dirty flag afterwards). That
/// incremental path was half-implemented: the only caller reuses one persistent
/// framebuffer and never re-marks it dirty, so after the first frame the gate
/// blanked the map entirely. Incremental layer rendering should only be
/// reintroduced as a complete implementation (caller-managed dirty lifecycle +
/// per-tile invalidation), not as a partial optimisation.
///
/// When a layer has `no_animation: true` and `cache` is provided, pre-rendered
/// RGBA tiles are used instead of calling `tile_color` for every pixel.
pub fn render_layers<F>(
    fb: &mut impl FbSurface,
    layers: &[MapLayer],
    camera_x: i32,
    camera_y: i32,
    width: u32,
    height: u32,
    tile_color: F,
) where
    F: Fn(u16, u8, u8, u8) -> Rgba,
{
    render_layers_sized(
        fb,
        layers,
        camera_x,
        camera_y,
        width,
        height,
        TILE_PIXELS,
        tile_color,
    )
}

/// Like [`render_layers`] but with a caller-specified `tile_size` (pixels per
/// tile side). [`render_layers`] is this with `tile_size == TILE_SIZE` (8).
/// Full-colour games whose tiles are not 8×8 (e.g. wuxia's 16×16 tiles) call
/// this so the grid step and flip maths match their tile size.
pub fn render_layers_sized<F>(
    fb: &mut impl FbSurface,
    layers: &[MapLayer],
    camera_x: i32,
    camera_y: i32,
    width: u32,
    height: u32,
    tile_size: u32,
    tile_color: F,
) where
    F: Fn(u16, u8, u8, u8) -> Rgba,
{
    render_layers_with_cache_sized(
        fb, layers, camera_x, camera_y, width, height, tile_size, tile_color, None,
    )
}

/// Like [`render_layers`] but accepts an optional [`LayerTileCache`] for
/// accelerating `no_animation` layers.
pub fn render_layers_with_cache<F>(
    fb: &mut impl FbSurface,
    layers: &[MapLayer],
    camera_x: i32,
    camera_y: i32,
    width: u32,
    height: u32,
    tile_color: F,
    cache: Option<&mut LayerTileCache>,
) where
    F: Fn(u16, u8, u8, u8) -> Rgba,
{
    render_layers_with_cache_sized(
        fb,
        layers,
        camera_x,
        camera_y,
        width,
        height,
        TILE_PIXELS,
        tile_color,
        cache,
    )
}

/// Like [`render_layers_with_cache`] but with a caller-specified `tile_size`
/// (pixels per tile side).
///
/// Note: [`LayerTileCache`] stores fixed 8×8 [`RgbaTile`]s, so caching is
/// bypassed whenever `tile_size != TILE_SIZE` — the per-pixel `tile_color`
/// path is used instead. (wuxia's static maps render fine without the cache;
/// the full-colour `tile_color` closure is a cheap array index.)
pub fn render_layers_with_cache_sized<F>(
    fb: &mut impl FbSurface,
    layers: &[MapLayer],
    camera_x: i32,
    camera_y: i32,
    width: u32,
    height: u32,
    tile_size: u32,
    tile_color: F,
    mut cache: Option<&mut LayerTileCache>,
) where
    F: Fn(u16, u8, u8, u8) -> Rgba,
{
    // Collect and sort visible layers.
    let mut visible: Vec<&MapLayer> = layers.iter().filter(|l| l.visible).collect();
    visible.sort_by_key(|l| l.z_index);

    if visible.is_empty() {
        // No layers to draw — blank the framebuffer (full-redraw contract).
        fb.clear(Rgba::TRANSPARENT);
        return;
    }

    // Fast path: exactly one visible layer — composite it directly onto `fb`,
    // PRESERVING whatever the caller already drew. `render_single_layer` skips
    // fully-transparent (alpha == 0) tile pixels, so holes in the layer reveal
    // the caller's existing framebuffer (a background fill, a parallax pass…)
    // instead of stamping transparent-black over it. (Deliberately no
    // `fb.clear()` here — that is what makes single-layer transparency show
    // through; the >=2-layer path below clears + composites self-contained.)
    if visible.len() == 1 {
        render_single_layer(
            fb,
            visible[0],
            camera_x,
            camera_y,
            width,
            height,
            tile_size,
            &tile_color,
            &mut cache,
        );
        return;
    }

    // General path: clear, render each layer to a temp buffer, then composite.
    fb.clear(Rgba::TRANSPARENT);
    let mut temp = FrameBuffer {
        data: vec![0; (width * height * 4) as usize],
        width,
        height,
        dirty_region: DirtyRegion::full(width, height),
    };

    for layer in &visible {
        temp.clear(Rgba::TRANSPARENT);
        render_single_layer(
            &mut temp,
            layer,
            camera_x,
            camera_y,
            width,
            height,
            tile_size,
            &tile_color,
            &mut cache,
        );
        composite_onto(fb, &temp, layer.opacity, layer.blend_mode);
    }
}

/// Render a single layer across the whole viewport.
///
/// When `cache` is provided and `layer.no_animation` is true, pre-rendered
/// RGBA tiles are used instead of calling `tile_color` per pixel.
fn render_single_layer<F>(
    fb: &mut impl FbSurface,
    layer: &MapLayer,
    camera_x: i32,
    camera_y: i32,
    width: u32,
    height: u32,
    tile_size: u32,
    tile_color: &F,
    cache: &mut Option<&mut LayerTileCache>,
) where
    F: Fn(u16, u8, u8, u8) -> Rgba,
{
    let scroll_x = (camera_x as f32 * layer.scroll_factor.0) as i32;
    let scroll_y = (camera_y as f32 * layer.scroll_factor.1) as i32;

    // The cache stores fixed 8×8 tiles, so it's only valid at the default size.
    let use_cache = layer.no_animation && cache.is_some() && tile_size == TILE_PIXELS;

    for screen_y in 0..height {
        let world_y = scroll_y + screen_y as i32;
        if world_y < 0 {
            continue;
        }
        let tile_y = (world_y as u32) / tile_size;
        let pixel_y = (world_y as u32 % tile_size) as u8;

        for screen_x in 0..width {
            let world_x = scroll_x + screen_x as i32;
            if world_x < 0 {
                continue;
            }
            let tile_x = (world_x as u32) / tile_size;
            let pixel_x = (world_x as u32 % tile_size) as u8;

            if let Some(entry) = layer.tilemap.get(tile_x as u16, tile_y as u16) {
                let color = if use_cache {
                    let c = cache.as_mut().unwrap();
                    let rgba_tile = c.get_or_insert(entry.tile_id, entry.palette_group, tile_color);
                    let (eff_px, eff_py) = effective_pixel(pixel_x, pixel_y, entry, tile_size);
                    rgba_tile.pixels[eff_py as usize][eff_px as usize]
                } else {
                    let (eff_px, eff_py) = effective_pixel(pixel_x, pixel_y, entry, tile_size);
                    tile_color(entry.tile_id, entry.palette_group, eff_px, eff_py)
                };
                // Skip fully-transparent tile pixels so they don't overwrite
                // what's beneath. In the single-layer fast path this lets the
                // caller's background show through holes; in the multi-layer
                // path the temp buffer is already transparent here, so this is
                // a no-op that matches the prior behaviour.
                if color.a == 0 {
                    continue;
                }
                fb.set_pixel(screen_x, screen_y, color);
            }
        }
    }
}

/// Compute the effective intra-tile pixel coordinate after applying
/// horizontal and vertical flip flags, for a tile of side `tile_size` pixels.
#[inline]
fn effective_pixel(px: u8, py: u8, entry: &TilemapEntry, tile_size: u32) -> (u8, u8) {
    let last = (tile_size - 1) as u8;
    let eff_px = if entry.flip_h { last - px } else { px };
    let eff_py = if entry.flip_v { last - py } else { py };
    (eff_px, eff_py)
}

/// Composite `src` onto `dst` in-place using the given blend mode and
/// overall layer opacity.
///
/// `dst` may be any [`FbSurface`] (the RGBA engine buffer or the indexed
/// facade); source pixels blend against the destination's current color and
/// are written back through the surface. The intermediate temp buffer stays
/// an RGBA [`FrameBuffer`].
///
/// Invariant: on an indexed destination, `get_pixel` reads through the
/// *display* palette while `set_pixel` quantizes through the *base* palette,
/// so compositing must only run while the display palette is the base
/// palette — a fade/flash palette active here would shift indices on
/// round-trip. Not reachable today; documented as a constraint.
fn composite_onto(
    dst: &mut impl FbSurface,
    src: &FrameBuffer,
    opacity: f32,
    blend_mode: BlendMode,
) {
    let (w, h) = (dst.width(), dst.height());
    assert_eq!(w, src.width);
    assert_eq!(h, src.height);

    for y in 0..h {
        for x in 0..w {
            let off = ((y * w + x) * 4) as usize;
            let sr = src.data[off] as f32;
            let sg = src.data[off + 1] as f32;
            let sb = src.data[off + 2] as f32;
            let sa = src.data[off + 3] as f32;

            let d = dst.get_pixel(x, y).unwrap_or(Rgba::TRANSPARENT);
            let dr = d.r as f32;
            let dg = d.g as f32;
            let db = d.b as f32;
            let da = d.a as f32;

            // Effective source alpha = pixel alpha * layer opacity.
            let src_alpha = (sa / 255.0) * opacity;

            let (out_r, out_g, out_b, out_a) = match blend_mode {
                BlendMode::Normal => {
                    let a = src_alpha;
                    let inv_a = 1.0 - a;
                    let r = sr * a + dr * inv_a;
                    let g = sg * a + dg * inv_a;
                    let b = sb * a + db * inv_a;
                    let a_out = sa * opacity + da * (1.0 - src_alpha);
                    (r, g, b, a_out)
                }
                BlendMode::Additive => {
                    let r = (dr + sr * src_alpha).min(255.0);
                    let g = (dg + sg * src_alpha).min(255.0);
                    let b = (db + sb * src_alpha).min(255.0);
                    (r, g, b, 255.0)
                }
                BlendMode::Multiply => {
                    let inv_opacity = 1.0 - opacity;
                    // lerp between original dst and multiplied result based on opacity.
                    let r_mul = sr * dr / 255.0;
                    let g_mul = sg * dg / 255.0;
                    let b_mul = sb * db / 255.0;
                    let r = dr * inv_opacity + r_mul * opacity;
                    let g = dg * inv_opacity + g_mul * opacity;
                    let b = db * inv_opacity + b_mul * opacity;
                    (r, g, b, 255.0)
                }
            };

            dst.set_pixel(
                x,
                y,
                Rgba::new(
                    out_r.clamp(0.0, 255.0) as u8,
                    out_g.clamp(0.0, 255.0) as u8,
                    out_b.clamp(0.0, 255.0) as u8,
                    out_a.clamp(0.0, 255.0) as u8,
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DirtyRegion;
    use dotzuki_engine::render::{BlendMode, MapLayer};
    use dotzuki_engine::tilemap::{Tilemap, TilemapEntry};

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    /// A tile-colour callback that maps tile_id to a unique solid colour.
    /// tile_id 0 → transparent, 1 → red, 2 → green, 3 → blue, else → white.
    fn test_tile_color(tile_id: u16, _pal: u8, _px: u8, _py: u8) -> Rgba {
        match tile_id {
            0 => Rgba::TRANSPARENT,
            1 => Rgba::rgb(255, 0, 0),
            2 => Rgba::rgb(0, 255, 0),
            3 => Rgba::rgb(0, 0, 255),
            _ => Rgba::rgb(255, 255, 255),
        }
    }

    /// Build a small tilemap where every entry has the given tile_id.
    fn uniform_tilemap(w: u16, h: u16, tile_id: u16) -> Tilemap {
        let mut tm = Tilemap::new(w, h);
        let entry = TilemapEntry {
            tile_id,
            ..Default::default()
        };
        tm.fill_rect(0, 0, w, h, entry);
        tm
    }

    // ------------------------------------------------------------------
    // effective_pixel
    // ------------------------------------------------------------------

    #[test]
    fn effective_pixel_no_flip() {
        let e = TilemapEntry::default();
        assert_eq!(effective_pixel(3, 5, &e, 8), (3, 5));
    }

    #[test]
    fn effective_pixel_flip_h() {
        let e = TilemapEntry {
            flip_h: true,
            ..Default::default()
        };
        assert_eq!(effective_pixel(0, 0, &e, 8), (7, 0));
        assert_eq!(effective_pixel(7, 0, &e, 8), (0, 0));
    }

    #[test]
    fn effective_pixel_flip_v() {
        let e = TilemapEntry {
            flip_v: true,
            ..Default::default()
        };
        assert_eq!(effective_pixel(0, 0, &e, 8), (0, 7));
        assert_eq!(effective_pixel(0, 7, &e, 8), (0, 0));
    }

    #[test]
    fn effective_pixel_flip_both() {
        let e = TilemapEntry {
            flip_h: true,
            flip_v: true,
            ..Default::default()
        };
        assert_eq!(effective_pixel(0, 0, &e, 8), (7, 7));
    }

    #[test]
    fn effective_pixel_flip_16px() {
        // At a 16px tile size, flips mirror around 15 (not 7).
        let e = TilemapEntry {
            flip_h: true,
            flip_v: true,
            ..Default::default()
        };
        assert_eq!(effective_pixel(0, 0, &e, 16), (15, 15));
        assert_eq!(effective_pixel(15, 3, &e, 16), (0, 12));
    }

    #[test]
    fn render_layers_sized_16px_tiles() {
        // A 2×1 map of 16px tiles: tile (0,0)=green(2), tile (1,0)=blue(3).
        // With a 16px grid, screen pixel x=20 lands in tile (1,0) → blue;
        // x=4 lands in tile (0,0) → green. (At the default 8px grid, x=20
        // would be tile 2 — out of range — proving the size is honoured.)
        let mut tm = Tilemap::new(2, 1);
        tm.set(
            0,
            0,
            TilemapEntry {
                tile_id: 2,
                ..Default::default()
            },
        );
        tm.set(
            1,
            0,
            TilemapEntry {
                tile_id: 3,
                ..Default::default()
            },
        );
        let layer = MapLayer::new(tm, 0);

        let mut fb = make_fb(32, 16, Rgba::WHITE);
        render_layers_sized(&mut fb, &[layer], 0, 0, 32, 16, 16, test_tile_color);

        assert_eq!(
            fb.get_pixel(4, 8),
            Some(Rgba::rgb(0, 255, 0)),
            "left half = green tile 0"
        );
        assert_eq!(
            fb.get_pixel(20, 8),
            Some(Rgba::rgb(0, 0, 255)),
            "right half = blue tile 1"
        );
    }

    // ------------------------------------------------------------------
    // composite_onto
    // ------------------------------------------------------------------

    #[test]
    fn composite_normal_fully_opaque() {
        let mut dst = make_fb(2, 2, Rgba::rgb(0, 0, 0));
        let src = make_fb(2, 2, Rgba::rgb(255, 0, 0));
        composite_onto(&mut dst, &src, 1.0, BlendMode::Normal);
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(dst.get_pixel(x, y), Some(Rgba::rgb(255, 0, 0)));
            }
        }
    }

    #[test]
    fn composite_normal_half_opaque() {
        let mut dst = make_fb(1, 1, Rgba::rgb(0, 0, 0));
        let src = make_fb(1, 1, Rgba::rgb(200, 0, 0));
        composite_onto(&mut dst, &src, 0.5, BlendMode::Normal);
        // 200*0.5 + 0*0.5 = 100
        let p = dst.get_pixel(0, 0).unwrap();
        assert!((p.r as i32 - 100).abs() <= 1);
        assert_eq!(p.g, 0);
        assert_eq!(p.b, 0);
    }

    #[test]
    fn composite_normal_transparent_src_does_not_overwrite() {
        let mut dst = make_fb(1, 1, Rgba::rgb(100, 100, 100));
        let src = make_fb(1, 1, Rgba::TRANSPARENT);
        composite_onto(&mut dst, &src, 1.0, BlendMode::Normal);
        assert_eq!(dst.get_pixel(0, 0), Some(Rgba::rgb(100, 100, 100)));
    }

    #[test]
    fn composite_additive() {
        let mut dst = make_fb(1, 1, Rgba::rgb(50, 30, 10));
        let src = make_fb(1, 1, Rgba::rgb(100, 80, 60));
        composite_onto(&mut dst, &src, 1.0, BlendMode::Additive);
        let p = dst.get_pixel(0, 0).unwrap();
        // 50+100=150, 30+80=110, 10+60=70
        assert!(p.r >= 145);
        assert!(p.g >= 105);
        assert!(p.b >= 65);
    }

    #[test]
    fn composite_additive_clamps_at_255() {
        let mut dst = make_fb(1, 1, Rgba::rgb(200, 200, 200));
        let src = make_fb(1, 1, Rgba::rgb(200, 200, 200));
        composite_onto(&mut dst, &src, 1.0, BlendMode::Additive);
        let p = dst.get_pixel(0, 0).unwrap();
        // 200+200=400, clamped to 255.
        assert_eq!(p.r, 255);
        assert_eq!(p.g, 255);
        assert_eq!(p.b, 255);
    }

    #[test]
    fn composite_multiply() {
        let mut dst = make_fb(1, 1, Rgba::rgb(255, 128, 64));
        let src = make_fb(1, 1, Rgba::rgb(128, 255, 192));
        composite_onto(&mut dst, &src, 1.0, BlendMode::Multiply);
        let p = dst.get_pixel(0, 0).unwrap();
        // 255*128/255=128, 128*255/255=128, 64*192/255≈48
        assert!((p.r as i32 - 128).abs() <= 1);
        assert!((p.g as i32 - 128).abs() <= 1);
        assert!((p.b as i32 - 48).abs() <= 2);
    }

    // ------------------------------------------------------------------
    // render_layers – single layer
    // ------------------------------------------------------------------

    #[test]
    fn single_layer_renders_correctly() {
        // 8×8 tilemap, all tiles = tile_id 1 (red).
        let tm = uniform_tilemap(1, 1, 1);
        let layer = MapLayer::new(tm, 0);
        let mut fb = FrameBuffer {
            data: vec![0; (8 * 8 * 4) as usize],
            width: 8,
            height: 8,
            dirty_region: DirtyRegion::full(8, 8),
        };

        render_layers(&mut fb, &[layer], 0, 0, 8, 8, test_tile_color);

        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(fb.get_pixel(x, y), Some(Rgba::rgb(255, 0, 0)));
            }
        }
    }

    #[test]
    fn single_layer_respects_camera_offset() {
        // 2×1 tilemap: tile (0,0) = id 2 (green), tile (1,0) = id 3 (blue).
        let mut tm = Tilemap::new(2, 1);
        tm.set(
            0,
            0,
            TilemapEntry {
                tile_id: 2,
                ..Default::default()
            },
        );
        tm.set(
            1,
            0,
            TilemapEntry {
                tile_id: 3,
                ..Default::default()
            },
        );

        let layer = MapLayer::new(tm, 0);
        // 8×8 viewport, camera scrolled right by 8 pixels.
        let mut fb = FrameBuffer {
            data: vec![0; (8 * 8 * 4) as usize],
            width: 8,
            height: 8,
            dirty_region: DirtyRegion::full(8, 8),
        };

        render_layers(&mut fb, &[layer], 8, 0, 8, 8, test_tile_color);

        // Should see tile (1,0) = blue
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(fb.get_pixel(x, y), Some(Rgba::rgb(0, 0, 255)));
            }
        }
    }

    #[test]
    fn scroll_factor_half_speed() {
        // 2×1 tilemap: (0,0)=red(1), (1,0)=green(2). Camera moves 16px right
        // but scroll_factor=0.5 → effective scroll=8px → tile (1,0) visible.
        let mut tm = Tilemap::new(2, 1);
        tm.set(
            0,
            0,
            TilemapEntry {
                tile_id: 1,
                ..Default::default()
            },
        );
        tm.set(
            1,
            0,
            TilemapEntry {
                tile_id: 2,
                ..Default::default()
            },
        );

        let mut layer = MapLayer::new(tm, 0);
        layer.scroll_factor = (0.5, 0.5);

        let mut fb = FrameBuffer {
            data: vec![0; (8 * 8 * 4) as usize],
            width: 8,
            height: 8,
            dirty_region: DirtyRegion::full(8, 8),
        };

        render_layers(&mut fb, &[layer], 16, 0, 8, 8, test_tile_color);

        // Camera at 16, factor 0.5 → effective scroll 8 → tile (1,0) = green
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(fb.get_pixel(x, y), Some(Rgba::rgb(0, 255, 0)));
            }
        }
    }

    #[test]
    fn single_layer_transparent_tile_reveals_background() {
        // Regression: a SINGLE visible layer with a transparent tile must let
        // the caller's pre-existing framebuffer show through, rather than the
        // fast path clearing fb and stamping transparent-black over it.
        //
        // 2×1 map at 8px tiles: tile (0,0) = transparent (id 0), (1,0) = red (1).
        // The caller pre-fills the framebuffer blue; the left tile must keep the
        // blue background and the right (opaque) tile must overwrite it.
        let mut tm = Tilemap::new(2, 1);
        tm.set(
            0,
            0,
            TilemapEntry {
                tile_id: 0,
                ..Default::default()
            },
        ); // transparent
        tm.set(
            1,
            0,
            TilemapEntry {
                tile_id: 1,
                ..Default::default()
            },
        ); // red, opaque
        let layer = MapLayer::new(tm, 0);

        let mut fb = make_fb(16, 8, Rgba::rgb(0, 0, 255)); // blue background
        render_layers(&mut fb, &[layer], 0, 0, 16, 8, test_tile_color);

        // Left half (transparent tile) → blue background shows through.
        assert_eq!(
            fb.get_pixel(0, 0),
            Some(Rgba::rgb(0, 0, 255)),
            "transparent tile keeps background"
        );
        assert_eq!(
            fb.get_pixel(7, 7),
            Some(Rgba::rgb(0, 0, 255)),
            "transparent tile keeps background"
        );
        // Right half (opaque red tile) → covers background.
        assert_eq!(
            fb.get_pixel(8, 0),
            Some(Rgba::rgb(255, 0, 0)),
            "opaque tile overwrites background"
        );
        assert_eq!(
            fb.get_pixel(15, 7),
            Some(Rgba::rgb(255, 0, 0)),
            "opaque tile overwrites background"
        );
    }

    // ------------------------------------------------------------------
    // render_layers – multi-layer compositing
    // ------------------------------------------------------------------

    #[test]
    fn two_layers_composite_transparent_top() {
        // Bottom: all green (2). Top: all transparent (0).
        let bottom_tm = uniform_tilemap(1, 1, 2);
        let top_tm = uniform_tilemap(1, 1, 0);

        let bottom = MapLayer::new(bottom_tm, 0);
        let top = MapLayer::new(top_tm, 1);

        let mut fb = FrameBuffer {
            data: vec![0; (8 * 8 * 4) as usize],
            width: 8,
            height: 8,
            dirty_region: DirtyRegion::full(8, 8),
        };

        render_layers(&mut fb, &[bottom, top], 0, 0, 8, 8, test_tile_color);

        // Bottom green should show through transparent top.
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(fb.get_pixel(x, y), Some(Rgba::rgb(0, 255, 0)));
            }
        }
    }

    #[test]
    fn two_layers_opaque_top_covers_bottom() {
        // Bottom: green (2). Top: red (1), opaque.
        let bottom_tm = uniform_tilemap(1, 1, 2);
        let top_tm = uniform_tilemap(1, 1, 1);

        let bottom = MapLayer::new(bottom_tm, 0);
        let top = MapLayer::new(top_tm, 1);

        let mut fb = FrameBuffer {
            data: vec![0; (8 * 8 * 4) as usize],
            width: 8,
            height: 8,
            dirty_region: DirtyRegion::full(8, 8),
        };

        render_layers(&mut fb, &[bottom, top], 0, 0, 8, 8, test_tile_color);

        // Top red should cover bottom green.
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(fb.get_pixel(x, y), Some(Rgba::rgb(255, 0, 0)));
            }
        }
    }

    #[test]
    fn opacity_affects_visibility() {
        // Bottom: red (1). Top: green (2), opacity 0.5.
        let bottom_tm = uniform_tilemap(1, 1, 1);
        let top_tm = uniform_tilemap(1, 1, 2);

        let bottom = MapLayer::new(bottom_tm, 0);
        let mut top = MapLayer::new(top_tm, 1);
        top.opacity = 0.5;

        let mut fb = FrameBuffer {
            data: vec![0; (1 * 1 * 4) as usize],
            width: 1,
            height: 1,
            dirty_region: DirtyRegion::full(1, 1),
        };

        render_layers(&mut fb, &[bottom, top], 0, 0, 1, 1, test_tile_color);

        // Blend: src=green(0,255,0) at 0.5 over dst=red(255,0,0)
        // Normal: g = 255 * 0.5 + 0 * 0.5 = 127.5, r = 0 * 0.5 + 255 * 0.5 = 127.5
        let p = fb.get_pixel(0, 0).unwrap();
        let r = p.r as i32;
        let g = p.g as i32;
        // Red should be ~128 (half original)
        assert!(r >= 125 && r <= 130, "r={r}");
        // Green should be ~128 (half top)
        assert!(g >= 125 && g <= 130, "g={g}");
    }

    #[test]
    fn invisible_layer_skipped() {
        // Bottom: green (2). Top: red (1), invisible.
        let bottom_tm = uniform_tilemap(1, 1, 2);
        let top_tm = uniform_tilemap(1, 1, 1);

        let bottom = MapLayer::new(bottom_tm, 0);
        let mut top = MapLayer::new(top_tm, 1);
        top.visible = false;

        let mut fb = FrameBuffer {
            data: vec![0; (8 * 8 * 4) as usize],
            width: 8,
            height: 8,
            dirty_region: DirtyRegion::full(8, 8),
        };

        render_layers(&mut fb, &[bottom, top], 0, 0, 8, 8, test_tile_color);

        // Only green should be visible.
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(fb.get_pixel(x, y), Some(Rgba::rgb(0, 255, 0)));
            }
        }
    }

    #[test]
    fn empty_layers_produces_blank() {
        let mut fb = FrameBuffer {
            data: vec![0xFF; (8 * 8 * 4) as usize],
            width: 8,
            height: 8,
            dirty_region: DirtyRegion::full(8, 8),
        };

        render_layers(&mut fb, &[], 0, 0, 8, 8, test_tile_color);

        // Should be cleared to transparent black.
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(fb.get_pixel(x, y), Some(Rgba::TRANSPARENT));
            }
        }
    }

    #[test]
    fn z_index_sorting() {
        // Layer A: z=5 (red). Layer B: z=1 (green). B should render first (behind).
        let tm_a = uniform_tilemap(1, 1, 1);
        let tm_b = uniform_tilemap(1, 1, 2);

        let layer_a = MapLayer::new(tm_a, 5);
        let layer_b = MapLayer::new(tm_b, 1);

        // Pass in reverse z-order — render_layers must sort.
        let mut fb = FrameBuffer {
            data: vec![0; (8 * 8 * 4) as usize],
            width: 8,
            height: 8,
            dirty_region: DirtyRegion::full(8, 8),
        };

        render_layers(&mut fb, &[layer_a, layer_b], 0, 0, 8, 8, test_tile_color);

        // Layer B (green, z=1) rendered first, then A (red, z=5) on top.
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(fb.get_pixel(x, y), Some(Rgba::rgb(255, 0, 0)));
            }
        }
    }

    #[test]
    fn scroll_factor_vertical() {
        // 1×2 tilemap: (0,0)=red(1), (0,1)=green(2). Camera at y=8.
        let mut tm = Tilemap::new(1, 2);
        tm.set(
            0,
            0,
            TilemapEntry {
                tile_id: 1,
                ..Default::default()
            },
        );
        tm.set(
            0,
            1,
            TilemapEntry {
                tile_id: 2,
                ..Default::default()
            },
        );

        let layer = MapLayer::new(tm, 0);

        let mut fb = FrameBuffer {
            data: vec![0; (8 * 8 * 4) as usize],
            width: 8,
            height: 8,
            dirty_region: DirtyRegion::full(8, 8),
        };

        render_layers(&mut fb, &[layer], 0, 8, 8, 8, test_tile_color);

        // Should see green (tile at y=1).
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(fb.get_pixel(x, y), Some(Rgba::rgb(0, 255, 0)));
            }
        }
    }

    #[test]
    fn single_layer_optimization_bypasses_temp_buffer() {
        // Verify that the single-layer fast path renders correctly.
        // (The behaviour is identical to multi-layer with one layer,
        // but the internal path is different.)
        let tm = uniform_tilemap(2, 2, 3);
        let layer = MapLayer::new(tm, 0);

        let mut fb = FrameBuffer {
            data: vec![0xFF; (16 * 16 * 4) as usize],
            width: 16,
            height: 16,
            dirty_region: DirtyRegion::full(16, 16),
        };

        render_layers(&mut fb, &[layer], 0, 0, 16, 16, test_tile_color);

        for y in 0..16 {
            for x in 0..16 {
                assert_eq!(fb.get_pixel(x, y), Some(Rgba::rgb(0, 0, 255)));
            }
        }
    }

    // ------------------------------------------------------------------
    // DirtyRegion tests
    // ------------------------------------------------------------------

    #[test]
    fn dirty_region_empty_is_not_present() {
        let d = DirtyRegion::empty();
        assert!(!d.present);
        assert!(!d.contains_pixel(0, 0));
        assert!(!d.contains_pixel(100, 50));
    }

    #[test]
    fn dirty_region_full_contains_all() {
        let d = DirtyRegion::full(160, 144);
        assert!(d.present);
        assert!(d.contains_pixel(0, 0));
        assert!(d.contains_pixel(159, 143));
        assert!(!d.contains_pixel(160, 0));
        assert!(!d.contains_pixel(0, 144));
    }

    #[test]
    fn dirty_region_union_combines() {
        let a = DirtyRegion::new(0, 0, 10, 10);
        let b = DirtyRegion::new(5, 5, 20, 20);
        let u = a.union(&b);
        assert!(u.present);
        assert_eq!(u.x, 0);
        assert_eq!(u.y, 0);
        assert_eq!(u.width, 25);
        assert_eq!(u.height, 25);
    }

    #[test]
    fn dirty_region_union_with_empty() {
        let a = DirtyRegion::new(0, 0, 10, 10);
        let empty = DirtyRegion::empty();
        assert_eq!(a.union(&empty), a);
        assert_eq!(empty.union(&a), a);
    }

    #[test]
    fn frame_buffer_dirty_mark_and_check() {
        let mut fb = make_fb(20, 10, Rgba::WHITE);
        fb.clear_dirty();
        // Nothing dirty → no pixel should be dirty
        assert!(!fb.is_dirty_pixel(0, 0));
        assert!(!fb.is_dirty_pixel(10, 5));

        // Mark a region
        fb.mark_dirty_rect(5, 3, 10, 4);
        assert!(fb.is_dirty_pixel(5, 3));
        assert!(fb.is_dirty_pixel(14, 6));
        assert!(!fb.is_dirty_pixel(4, 3));
        assert!(!fb.is_dirty_pixel(0, 0));

        // Mark all dirty
        fb.mark_all_dirty();
        assert!(fb.is_dirty_pixel(0, 0));
        assert!(fb.is_dirty_pixel(19, 9));
    }

    #[test]
    fn frame_buffer_mark_dirty_tile() {
        let mut fb = make_fb(32, 24, Rgba::WHITE);
        fb.clear_dirty();
        fb.mark_dirty_tile(2, 3);
        // Tile (2,3) starts at pixel (16, 24)
        assert!(fb.is_dirty_pixel(16, 24));
        assert!(fb.is_dirty_pixel(23, 31));
        assert!(!fb.is_dirty_pixel(15, 24));
        assert!(!fb.is_dirty_pixel(0, 0));
    }

    // ------------------------------------------------------------------
    // LayerTileCache tests
    // ------------------------------------------------------------------

    #[test]
    fn cached_tiles_render_same_as_non_cached() {
        // Create a 4x4 tilemap with varying tile_ids.
        let mut tm = Tilemap::new(4, 4);
        let tiles = [1u16, 2, 3, 1, 2, 1, 3, 2, 3, 1, 2, 1, 1, 3, 2, 3];
        for y in 0..4u16 {
            for x in 0..4u16 {
                tm.set(
                    x,
                    y,
                    TilemapEntry {
                        tile_id: tiles[(y * 4 + x) as usize],
                        ..Default::default()
                    },
                );
            }
        }

        let mut layer = MapLayer::new(tm, 0);
        layer.no_animation = true;

        // Render without cache
        let mut fb_uncached = make_fb(32, 32, Rgba::WHITE);
        render_layers(
            &mut fb_uncached,
            &[layer.clone()],
            0,
            0,
            32,
            32,
            test_tile_color,
        );

        // Render with cache
        let mut fb_cached = make_fb(32, 32, Rgba::WHITE);
        let mut cache = LayerTileCache::new();
        render_layers_with_cache(
            &mut fb_cached,
            &[layer],
            0,
            0,
            32,
            32,
            test_tile_color,
            Some(&mut cache),
        );

        // Outputs must be identical
        for y in 0..32 {
            for x in 0..32 {
                assert_eq!(
                    fb_uncached.get_pixel(x, y),
                    fb_cached.get_pixel(x, y),
                    "pixel ({},{}) differs",
                    x,
                    y
                );
            }
        }
    }

    #[test]
    fn cache_invalidates_and_rebuilds() {
        let tm = uniform_tilemap(1, 1, 1);
        let mut layer = MapLayer::new(tm, 0);
        layer.no_animation = true;

        let mut cache = LayerTileCache::new();
        let mut fb = make_fb(8, 8, Rgba::WHITE);
        render_layers_with_cache(
            &mut fb,
            &[layer.clone()],
            0,
            0,
            8,
            8,
            test_tile_color,
            Some(&mut cache),
        );

        // Cache should have entries now
        assert!(!cache.entries.is_empty());

        // Invalidate
        cache.invalidate();
        assert!(cache.entries.is_empty());

        // Can still render (cache rebuilds)
        let mut fb2 = make_fb(8, 8, Rgba::WHITE);
        render_layers_with_cache(
            &mut fb2,
            &[layer],
            0,
            0,
            8,
            8,
            test_tile_color,
            Some(&mut cache),
        );
        assert!(!cache.entries.is_empty());
    }

    #[test]
    fn render_layers_ignores_dirty_region_and_always_redraws() {
        // Even with an empty dirty region, render_layers must perform a full
        // redraw — this is the regression guard for the blank-overworld bug.
        let tm = uniform_tilemap(2, 2, 1);
        let layer = MapLayer::new(tm, 0);

        let mut fb = make_fb(16, 16, Rgba::WHITE);
        fb.clear_dirty(); // nothing marked dirty

        render_layers(&mut fb, &[layer], 0, 0, 16, 16, test_tile_color);

        // The whole framebuffer must be redrawn red regardless of dirty state.
        for y in 0..16 {
            for x in 0..16 {
                assert_eq!(
                    fb.get_pixel(x, y),
                    Some(Rgba::rgb(255, 0, 0)),
                    "pixel ({},{}) should be red after full redraw",
                    x,
                    y
                );
            }
        }
    }

    #[test]
    fn layer_without_no_animation_does_not_use_cache() {
        let tm = uniform_tilemap(1, 1, 1);
        let mut layer = MapLayer::new(tm, 0);
        layer.no_animation = false;

        let mut cache = LayerTileCache::new();
        let mut fb = make_fb(8, 8, Rgba::WHITE);
        render_layers_with_cache(
            &mut fb,
            &[layer],
            0,
            0,
            8,
            8,
            test_tile_color,
            Some(&mut cache),
        );

        // Cache should be empty because no_animation was false
        assert!(cache.entries.is_empty());
    }

    // ------------------------------------------------------------------
    // Helper
    // ------------------------------------------------------------------

    fn make_fb(w: u32, h: u32, color: Rgba) -> FrameBuffer {
        let mut fb = FrameBuffer {
            data: vec![0; (w * h * 4) as usize],
            width: w,
            height: h,
            dirty_region: DirtyRegion::full(w, h),
        };
        fb.clear(color);
        fb
    }
}
