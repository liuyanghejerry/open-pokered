//! Runtime map loading for zero-Rust game projects.
//!
//! A map lives in `<maps_dir>/<MapId>/` and consists of:
//!
//! - `map.tmx.json` — a Tiled JSON map. Visual layers render in stack order,
//!   split at the player's elevation by their integer custom property
//!   `level` (default 0): `level <= player elevation` draws below sprites,
//!   above it draws over them. The data layers are never rendered:
//!   `collision` is the level-0 collision grid, `collisionN` the level-N
//!   grid (any non-zero GID marks a solid tile at that level), and `stairs`
//!   marks elevation transitions (GID 1 = ascend on arrival, 2 = descend).
//! - `tileset.png` — a row-major tile atlas sliced by [`crate::tileset::PngTileset`]
//!   using the TMX `tilewidth`/`tileheight`.
//! - an entity sidecar — the dotzuki-editor writes `objects.json`
//!   (`{npcs, warps, …}`); older fixtures used `map.json`. [`MapObjects::load`]
//!   tries `objects.json` first and falls back to `map.json`; neither being
//!   present yields an empty sidecar, not an error.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use dotzuki_engine::overworld::actor::OverworldCollision;
use dotzuki_engine::render::{FrameBuffer, MapRenderState, Rgba};
use dotzuki_engine_tiled::{clean_gid, parse_tmx, tmx_to_map_state};
use serde::Deserialize;

use crate::tileset::PngTileset;
use crate::vfs::{join_path, DiskFiles, ProjectFiles};

/// Filename of the entity sidecar the dotzuki-editor writes today.
pub const OBJECTS_SIDECAR: &str = "objects.json";
/// Legacy sidecar filename, read as a fallback when `objects.json` is absent.
pub const LEGACY_SIDECAR: &str = "map.json";

/// A placed NPC in the per-map objects sidecar.
#[derive(Debug, Clone, Deserialize)]
pub struct NpcDef {
    /// Editor-assigned numeric id.
    pub id: u32,
    /// Display name (may be empty).
    #[serde(default)]
    pub name: String,
    /// Tile X.
    pub x: i32,
    /// Tile Y.
    pub y: i32,
    /// Facing direction (`"down"`, `"up"`, `"left"`, `"right"`).
    #[serde(default = "default_facing")]
    pub facing: String,
    /// Sprite identifier/path (may be empty while art is pending).
    #[serde(default)]
    pub sprite: String,
    /// Storyline/text the NPC speaks when talked to (may be empty).
    #[serde(default)]
    pub talk: String,
}

fn default_facing() -> String {
    "down".to_string()
}

/// A warp tile in the objects sidecar.
///
/// `dest_map` is validated lazily at warp time, not at load — a map must
/// load even while one of its warps points at a not-yet-created map.
#[derive(Debug, Clone, Deserialize)]
pub struct WarpDef {
    /// Tile X of the warp source.
    pub x: i32,
    /// Tile Y of the warp source.
    pub y: i32,
    /// Destination map id (empty while the warp is unlinked).
    #[serde(default)]
    pub dest_map: String,
    /// Destination tile X.
    #[serde(default)]
    pub dest_x: i32,
    /// Destination tile Y.
    #[serde(default)]
    pub dest_y: i32,
}

/// A sign tile in the objects sidecar.
#[derive(Debug, Clone, Deserialize)]
pub struct SignDef {
    /// Tile X.
    pub x: i32,
    /// Tile Y.
    pub y: i32,
    /// Text shown when the sign is read.
    #[serde(default)]
    pub text: String,
}

/// A random-encounter table entry: a species/encounter id and its relative
/// weight. The `id` is resolved at battle start by
/// [`crate::battle::BattleSetup::start_with`] — an encounter record first,
/// then a single enemy record (trainer queues and wild singles both work).
#[derive(Debug, Clone, Deserialize)]
pub struct EncounterTableEntry {
    /// Encounter or enemy record id.
    pub id: String,
    /// Relative draw weight within the zone's table.
    #[serde(default = "default_weight")]
    pub weight: u32,
}

fn default_weight() -> u32 {
    1
}

/// One encounter zone: an inclusive map-tile rectangle plus the weighted
/// table drawn from when a step lands inside it.
#[derive(Debug, Clone, Deserialize)]
pub struct EncounterZone {
    /// Left edge (tile X, inclusive).
    pub x: i32,
    /// Top edge (tile Y, inclusive).
    pub y: i32,
    /// Width in tiles.
    pub w: i32,
    /// Height in tiles.
    pub h: i32,
    /// Weighted species/encounter table.
    #[serde(default)]
    pub table: Vec<EncounterTableEntry>,
}

impl EncounterZone {
    /// `true` when tile `(x, y)` lies inside the rectangle (inclusive).
    #[inline]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Random-encounter configuration in the objects sidecar (same shape as
/// pokered's `wild_data`: a per-step rate byte in /256 units — `rate: 25`
/// ≈ a 9.8% trigger chance per step — plus grass-like zones).
#[derive(Debug, Clone, Deserialize)]
pub struct EncounterConfig {
    /// Per-step trigger probability in /256 units.
    pub rate: u8,
    /// Encounter zones; a step rolls only when it lands inside one.
    #[serde(default)]
    pub zones: Vec<EncounterZone>,
}

/// The per-map entity sidecar: NPCs, warps, signs and random encounters.
///
/// Unknown keys in the JSON (e.g. a legacy `collision` grid, `music`,
/// `tileset` in old `map.json` fixtures) are ignored.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MapObjects {
    /// Placed NPCs.
    #[serde(default)]
    pub npcs: Vec<NpcDef>,
    /// Warp tiles.
    #[serde(default)]
    pub warps: Vec<WarpDef>,
    /// Sign tiles.
    #[serde(default)]
    pub signs: Vec<SignDef>,
    /// Random-encounter configuration; `None` (or absent in the JSON) means
    /// the map never triggers wild battles from walking.
    #[serde(default)]
    pub encounters: Option<EncounterConfig>,
}

impl MapObjects {
    /// Load the sidecar for a map directory: `objects.json` first, falling
    /// back to the legacy `map.json`. Returns an empty sidecar when neither
    /// exists. Disk convenience for [`load_with_files`](Self::load_with_files).
    ///
    /// # Errors
    ///
    /// Fails only when a sidecar file exists but cannot be read or parsed.
    pub fn load(map_dir: &Path) -> Result<Self> {
        Self::load_with_files(&DiskFiles::new(map_dir), "")
    }

    /// VFS form of [`load`](Self::load): `map_dir_rel` is the map directory
    /// as a project-relative POSIX path (`""` = the backend root).
    ///
    /// # Errors
    ///
    /// Fails only when a sidecar file exists but cannot be read or parsed.
    pub fn load_with_files(files: &dyn ProjectFiles, map_dir_rel: &str) -> Result<Self> {
        for name in [OBJECTS_SIDECAR, LEGACY_SIDECAR] {
            let rel = join_path(map_dir_rel, name);
            match files.read(&rel) {
                Ok(bytes) => {
                    let text =
                        String::from_utf8(bytes).with_context(|| format!("{rel} is not UTF-8"))?;
                    return serde_json::from_str(&text)
                        .with_context(|| format!("failed to parse {rel}"));
                }
                Err(_) => continue,
            }
        }
        Ok(Self::default())
    }
}

/// A loaded runtime map: visual layers, per-level collision grids, the
/// stairs grid, tileset pixels and the entity sidecar.
pub struct RuntimeMap {
    id: String,
    /// Map size in tiles.
    width: u16,
    height: u16,
    /// Tile size in pixels (from the TMX `tilewidth`/`tileheight`).
    tile_w: u32,
    tile_h: u32,
    /// Visual render state (the `collision*`/`stairs` data layers are excluded).
    state: MapRenderState,
    /// Collision cells per elevation level (`collision_levels[level][cell]`,
    /// `true` = solid); index 0 always exists (the `collision` layer, or an
    /// all-passable grid when the map has none).
    collision_levels: Vec<Vec<bool>>,
    /// `width * height` stair GIDs (flip flags stripped; 0 = no stair) when
    /// the map has a `stairs` layer.
    stairs: Option<Vec<u32>>,
    tileset: PngTileset,
    objects: MapObjects,
}

/// The elevation level a collision layer name encodes: `collision` ⇒ 0,
/// `collisionN` ⇒ N. `None` for any other layer name.
fn collision_layer_level(name: &str) -> Option<usize> {
    let suffix = name.strip_prefix("collision")?;
    if suffix.is_empty() {
        return Some(0);
    }
    suffix.parse::<usize>().ok().filter(|&n| n >= 1)
}

impl RuntimeMap {
    /// Load `<maps_dir>/<map_id>/` (TMX + tileset + sidecar) from disk.
    /// Convenience for [`load_with_files`](Self::load_with_files) over a
    /// [`DiskFiles`] rooted at `maps_dir`.
    ///
    /// # Errors
    ///
    /// Fails when `map.tmx.json` or `tileset.png` is missing/unreadable, the
    /// TMX does not parse, or the tileset is invalid. A missing sidecar is
    /// not an error.
    pub fn load(maps_dir: &Path, map_id: &str) -> Result<Self> {
        Self::load_with_files(&DiskFiles::new(maps_dir), "", map_id)
    }

    /// VFS form of [`load`](Self::load): `maps_dir_rel` is the maps
    /// directory as a project-relative POSIX path (`""` = the backend root).
    ///
    /// # Errors
    ///
    /// Same conditions as [`load`](Self::load).
    pub fn load_with_files(
        files: &dyn ProjectFiles,
        maps_dir_rel: &str,
        map_id: &str,
    ) -> Result<Self> {
        let map_dir = join_path(maps_dir_rel, map_id);
        let tmx_rel = join_path(&map_dir, "map.tmx.json");
        let bytes = files
            .read(&tmx_rel)
            .with_context(|| format!("failed to read {tmx_rel}"))?;
        let json = String::from_utf8(bytes).with_context(|| format!("{tmx_rel} is not UTF-8"))?;
        let tmx = parse_tmx(&json).map_err(|e| anyhow::anyhow!("parse {tmx_rel}: {e}"))?;

        let width = tmx.width.max(1) as u16;
        let height = tmx.height.max(1) as u16;

        // Collision grids per elevation level: `collision` is level 0,
        // `collisionN` level N (a non-zero GID ⇒ solid at that level).
        // Missing intermediate levels (e.g. `collision` + `collision2`
        // without `collision1`) are filled all-SOLID — an undefined level
        // must never be walkable, or the player could climb into a void.
        let cells = width as usize * height as usize;
        let mut grids: Vec<(usize, Vec<bool>)> = Vec::new();
        let mut stairs: Option<Vec<u32>> = None;
        for layer in &tmx.layers {
            if let Some(level) = collision_layer_level(&layer.name) {
                let mut grid = vec![false; cells];
                for (i, &gid) in layer.data.iter().enumerate().take(cells) {
                    grid[i] = gid != 0;
                }
                grids.push((level, grid));
            } else if layer.name == "stairs" {
                let mut grid = vec![0u32; cells];
                for (i, &gid) in layer.data.iter().enumerate().take(cells) {
                    grid[i] = clean_gid(gid);
                }
                stairs = Some(grid);
            }
        }
        let max_level = grids.iter().map(|(l, _)| *l).max().unwrap_or(0);
        let mut collision_levels = vec![vec![false; cells]; max_level + 1];
        for grid in collision_levels.iter_mut().skip(1) {
            grid.fill(true);
        }
        for (level, grid) in grids {
            collision_levels[level] = grid;
        }

        // Visual layers = everything except the collision*/stairs data layers.
        let mut visual = tmx.clone();
        visual
            .layers
            .retain(|l| collision_layer_level(&l.name).is_none() && l.name != "stairs");
        let state = tmx_to_map_state(&visual);

        let tileset_rel = join_path(&map_dir, "tileset.png");
        let png = files
            .read(&tileset_rel)
            .with_context(|| format!("failed to read tileset {tileset_rel}"))?;
        let tileset = PngTileset::from_png_bytes(&png, tmx.tile_width, tmx.tile_height)
            .with_context(|| format!("invalid tileset {tileset_rel}"))?;

        let objects = MapObjects::load_with_files(files, &map_dir)?;

        Ok(Self {
            id: map_id.to_string(),
            width,
            height,
            tile_w: tmx.tile_width,
            tile_h: tmx.tile_height,
            state,
            collision_levels,
            stairs,
            tileset,
            objects,
        })
    }

    /// Map id (the directory name under `maps/`).
    #[inline]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Map width in tiles.
    #[inline]
    pub fn width(&self) -> u16 {
        self.width
    }

    /// Map height in tiles.
    #[inline]
    pub fn height(&self) -> u16 {
        self.height
    }

    /// Tile size in pixels `(width, height)`.
    #[inline]
    pub fn tile_size(&self) -> (u32, u32) {
        (self.tile_w, self.tile_h)
    }

    /// Map size in pixels (for camera bounds).
    #[inline]
    pub fn pixel_width(&self) -> i32 {
        self.width as i32 * self.tile_w as i32
    }

    /// Map size in pixels (for camera bounds).
    #[inline]
    pub fn pixel_height(&self) -> i32 {
        self.height as i32 * self.tile_h as i32
    }

    /// Visual layers (collision/stairs data layers excluded) with the map
    /// background.
    #[inline]
    pub fn render_state(&self) -> &MapRenderState {
        &self.state
    }

    /// The sliced tileset.
    #[inline]
    pub fn tileset(&self) -> &PngTileset {
        &self.tileset
    }

    /// The entity sidecar (NPCs, warps, signs).
    #[inline]
    pub fn objects(&self) -> &MapObjects {
        &self.objects
    }

    /// `true` if walking onto tile `(x, y)` is blocked at ground level.
    /// Out-of-bounds is solid (enclosed world); never panics.
    #[inline]
    pub fn is_blocked(&self, x: i32, y: i32) -> bool {
        self.is_blocked_at(0, x, y)
    }

    /// `true` if walking onto tile `(x, y)` is blocked at elevation `level`.
    /// Out-of-bounds and levels the map doesn't define are solid; never panics.
    #[inline]
    pub fn is_blocked_at(&self, level: u8, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return true;
        }
        let idx = y as usize * self.width as usize + x as usize;
        self.collision_levels
            .get(level as usize)
            .and_then(|grid| grid.get(idx))
            .copied()
            .unwrap_or(true)
    }

    /// The number of elevation levels (1 for a single-level map).
    #[inline]
    pub fn level_count(&self) -> usize {
        self.collision_levels.len()
    }

    /// The stair GID on tile `(x, y)` (1 = ascend, 2 = descend), `None`
    /// when the map has no `stairs` layer, the tile is out-of-bounds, or
    /// the cell is empty.
    #[inline]
    pub fn stair_at(&self, x: i32, y: i32) -> Option<u32> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        let idx = y as usize * self.width as usize + x as usize;
        self.stairs
            .as_ref()
            .and_then(|grid| grid.get(idx))
            .copied()
            .filter(|&gid| gid != 0)
    }

    /// RGBA pixel of the tile with 1-based Tiled `gid` at intra-tile
    /// `(px, py)` — matches the `tile_color` callback shape of
    /// `dotzuki_renderer::layer_renderer::render_layers_sized` (the palette-group
    /// argument is unused: PNG tiles carry their own colours).
    #[inline]
    pub fn gid_pixel(&self, gid: u16, px: u8, py: u8) -> Rgba {
        self.tileset.gid_pixel(gid, px, py)
    }

    /// Render the map's visual layers into `fb` at camera offset
    /// `(camera_x, camera_y)` (world pixels at the framebuffer's top-left).
    ///
    /// Requires square tiles (the renderer's grid step is a single
    /// `tile_size`); fails for maps whose `tilewidth != tileheight`.
    pub fn render(
        &self,
        fb: &mut FrameBuffer,
        camera_x: i32,
        camera_y: i32,
        width: u32,
        height: u32,
    ) -> Result<()> {
        self.check_square_tiles()?;
        dotzuki_renderer::layer_renderer::render_layers_sized(
            fb,
            &self.state.layers,
            camera_x,
            camera_y,
            width,
            height,
            self.tile_w,
            |gid, _pal, px, py| self.gid_pixel(gid, px, py),
        );
        Ok(())
    }

    /// Render only the layers at or below `player_level` (`level <=
    /// player_level`) — the half of the stack drawn *under* the sprites on
    /// multi-level maps. Same square-tiles requirement as [`render`](Self::render).
    pub fn render_below(
        &self,
        fb: &mut FrameBuffer,
        camera_x: i32,
        camera_y: i32,
        width: u32,
        height: u32,
        player_level: i32,
    ) -> Result<()> {
        self.render_filtered(fb, camera_x, camera_y, width, height, |level| {
            level <= player_level
        })
    }

    /// Render only the layers above `player_level` — the half of the stack
    /// drawn *over* the sprites on multi-level maps. Same square-tiles
    /// requirement as [`render`](Self::render).
    pub fn render_above(
        &self,
        fb: &mut FrameBuffer,
        camera_x: i32,
        camera_y: i32,
        width: u32,
        height: u32,
        player_level: i32,
    ) -> Result<()> {
        self.render_filtered(fb, camera_x, camera_y, width, height, |level| {
            level > player_level
        })
    }

    /// Render the layers whose elevation `level` passes `keep`, preserving
    /// what is already in `fb`. Each kept group composites in `z_index`
    /// order (the renderer sorts the slice it is given), so the two halves
    /// of a split draw each preserve the original stack order.
    ///
    /// `render_layers_sized` has a full-redraw contract (it clears the
    /// framebuffer), so a partial stack renders into a temp buffer first
    /// and is then stamped over the existing frame — transparent pixels
    /// reveal what was drawn before (e.g. the sprites under an above-group).
    fn render_filtered(
        &self,
        fb: &mut FrameBuffer,
        camera_x: i32,
        camera_y: i32,
        width: u32,
        height: u32,
        keep: impl Fn(i32) -> bool,
    ) -> Result<()> {
        self.check_square_tiles()?;
        let layers: Vec<_> = self
            .state
            .layers
            .iter()
            .filter(|l| keep(l.level))
            .cloned()
            .collect();
        if layers.is_empty() {
            return Ok(());
        }
        let mut temp = FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(width, height),
            Rgba::TRANSPARENT,
        );
        dotzuki_renderer::layer_renderer::render_layers_sized(
            &mut temp,
            &layers,
            camera_x,
            camera_y,
            width,
            height,
            self.tile_w,
            |gid, _pal, px, py| self.gid_pixel(gid, px, py),
        );
        for (dst, src) in fb.data.chunks_exact_mut(4).zip(temp.data.chunks_exact(4)) {
            if src[3] != 0 {
                dst.copy_from_slice(src);
            }
        }
        Ok(())
    }

    /// Guard shared by the render methods: the layer renderer's grid step is
    /// a single `tile_size`, so tiles must be square.
    fn check_square_tiles(&self) -> Result<()> {
        if self.tile_w != self.tile_h {
            bail!(
                "map '{}': render_layers_sized needs square tiles, got {}x{}",
                self.id,
                self.tile_w,
                self.tile_h
            );
        }
        Ok(())
    }
}

impl OverworldCollision for RuntimeMap {
    fn is_blocked(&self, x: i32, y: i32) -> bool {
        self.is_blocked(x, y)
    }

    fn is_blocked_at(&self, level: u8, x: i32, y: i32) -> bool {
        self.is_blocked_at(level, x, y)
    }
}

/// Convenience: the directory of one map under a project's maps dir.
pub fn map_dir(maps_dir: &Path, map_id: &str) -> PathBuf {
    maps_dir.join(map_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfs::MemoryFiles;
    use std::collections::HashMap;

    /// A 64×16 `tileset.png` (four 16×16 flat-colour tiles, row-major).
    fn tileset_png() -> Vec<u8> {
        let tile = 16u32;
        let mut img = image::RgbaImage::new(tile * 4, tile);
        for px in img.pixels_mut() {
            *px = image::Rgba([0xFF, 0x00, 0x00, 0xFF]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    /// Load a map from an in-memory project holding just the given TMX JSON
    /// plus a generated tileset.
    fn load_mem_map(tmx: &str) -> RuntimeMap {
        let files = MemoryFiles::new(HashMap::from([
            (
                "maps/Town/map.tmx.json".to_string(),
                tmx.as_bytes().to_vec(),
            ),
            ("maps/Town/tileset.png".to_string(), tileset_png()),
        ]));
        RuntimeMap::load_with_files(&files, "maps", "Town").expect("load map")
    }

    /// A 3×2 two-level map: ground + wall-top visual layers, `collision`
    /// (level 0), `collision1`, and a `stairs` layer (ascend at (1, 0) with
    /// a flipped GID, descend at (2, 1)).
    const TWO_LEVEL_TMX: &str = r#"{
  "width": 3, "height": 2, "tilewidth": 16, "tileheight": 16,
  "layers": [
    { "name": "ground", "width": 3, "height": 2, "data": [1,1,1,1,1,1] },
    { "name": "walltop", "width": 3, "height": 2, "data": [0,2,0,0,2,0],
      "properties": [{ "name": "level", "type": "int", "value": 1 }] },
    { "name": "collision", "width": 3, "height": 2, "data": [1,0,1,0,0,1] },
    { "name": "collision1", "width": 3, "height": 2, "data": [0,0,0,1,0,0] },
    { "name": "stairs", "width": 3, "height": 2, "data": [0,2147483649,0,0,0,2] }
  ],
  "tilesets": [
    { "firstgid": 1, "name": "ts", "tilewidth": 16, "tileheight": 16, "tilecount": 4 }
  ]
}"#;

    #[test]
    fn multi_level_collision_parsed_per_level() {
        let map = load_mem_map(TWO_LEVEL_TMX);
        assert_eq!(map.level_count(), 2);

        // Level 0 (the `collision` layer); `is_blocked` stays level 0.
        assert!(map.is_blocked(0, 0));
        assert!(!map.is_blocked(1, 0));
        assert!(map.is_blocked(2, 1));

        // Level 1 has a different grid.
        assert!(!map.is_blocked_at(1, 0, 0));
        assert!(map.is_blocked_at(1, 0, 1));
        assert!(!map.is_blocked_at(1, 2, 1));

        // Undefined levels and out-of-bounds are solid.
        assert!(map.is_blocked_at(2, 1, 0));
        assert!(map.is_blocked_at(1, -1, 0));
        assert!(map.is_blocked_at(1, 3, 0));

        // The trait impl agrees with the inherent methods.
        let collision: &dyn OverworldCollision = &map;
        assert!(collision.is_blocked(0, 0));
        assert!(collision.is_blocked_at(1, 0, 1));
    }

    #[test]
    fn stairs_parsed_and_excluded_from_render_layers() {
        let map = load_mem_map(TWO_LEVEL_TMX);
        assert_eq!(map.stair_at(1, 0), Some(1), "flip flag stripped, ascend");
        assert_eq!(map.stair_at(2, 1), Some(2), "descend");
        assert_eq!(map.stair_at(0, 0), None, "empty stair cell");
        assert_eq!(map.stair_at(-1, 0), None, "out-of-bounds");

        // Only the two visual layers render; `level` rides along.
        let layers = &map.render_state().layers;
        assert_eq!(layers.len(), 2, "collision*/stairs are not rendered");
        assert_eq!(layers[0].level, 0);
        assert_eq!(layers[1].level, 1);
    }

    #[test]
    fn missing_intermediate_level_is_all_solid() {
        // `collision` + `collision2` without `collision1`: the gap level is
        // filled all-solid so an undefined level is never walkable.
        let tmx = r#"{
  "width": 2, "height": 1, "tilewidth": 16, "tileheight": 16,
  "layers": [
    { "name": "ground", "width": 2, "height": 1, "data": [1,1] },
    { "name": "collision", "width": 2, "height": 1, "data": [0,0] },
    { "name": "collision2", "width": 2, "height": 1, "data": [0,1] }
  ],
  "tilesets": [
    { "firstgid": 1, "name": "ts", "tilewidth": 16, "tileheight": 16, "tilecount": 4 }
  ]
}"#;
        let map = load_mem_map(tmx);
        assert_eq!(map.level_count(), 3);
        assert!(map.is_blocked_at(1, 0, 0), "gap level 1 is all-solid");
        assert!(map.is_blocked_at(1, 1, 0));
        assert!(!map.is_blocked_at(2, 0, 0));
        assert!(map.is_blocked_at(2, 1, 0));
    }

    /// A split draw preserves what is already in the framebuffer: the above
    /// group stamps only its opaque pixels over the "sprites" beneath.
    #[test]
    fn split_render_preserves_lower_content() {
        use dotzuki_engine::render_config::RenderConfig;

        let map = load_mem_map(TWO_LEVEL_TMX);
        let mut fb = FrameBuffer::new(RenderConfig::new(48, 32), Rgba::TRANSPARENT);

        // Below group (level 0): the ground layer fills the view.
        map.render_below(&mut fb, 0, 0, 48, 32, 0).expect("below");
        assert_eq!(fb.get_pixel(0, 0), Some(Rgba::new(0xFF, 0, 0, 0xFF)));

        // A "sprite" pixel where the wall-top layer is transparent…
        let sprite = Rgba::new(0, 0, 0xFF, 0xFF);
        fb.fill_rect(0, 0, 1, 1, sprite);
        map.render_above(&mut fb, 0, 0, 48, 32, 0).expect("above");
        assert_eq!(fb.get_pixel(0, 0), Some(sprite), "holes reveal the sprite");
        // …and an opaque wall-top tile (tile (1,0)) stamps over the ground.
        assert_eq!(
            fb.get_pixel(16, 0),
            Some(Rgba::new(0xFF, 0, 0, 0xFF)),
            "opaque above-layer tile draws over"
        );

        // An empty above group (player at level 1 ⇒ nothing is higher) is a
        // no-op, not a clear.
        map.render_above(&mut fb, 0, 0, 48, 32, 1).expect("above");
        assert_eq!(
            fb.get_pixel(0, 0),
            Some(sprite),
            "empty group leaves fb alone"
        );
    }

    #[test]
    fn single_level_map_behaves_as_before() {
        // Legacy shape: only a `collision` layer, no stairs.
        let tmx = r#"{
  "width": 2, "height": 1, "tilewidth": 16, "tileheight": 16,
  "layers": [
    { "name": "ground", "width": 2, "height": 1, "data": [1,1] },
    { "name": "collision", "width": 2, "height": 1, "data": [0,1] }
  ],
  "tilesets": [
    { "firstgid": 1, "name": "ts", "tilewidth": 16, "tileheight": 16, "tilecount": 4 }
  ]
}"#;
        let map = load_mem_map(tmx);
        assert_eq!(map.level_count(), 1);
        assert!(!map.is_blocked(0, 0));
        assert!(map.is_blocked(1, 0));
        assert_eq!(map.stair_at(0, 0), None);
        assert_eq!(map.render_state().layers.len(), 1);
    }
}
