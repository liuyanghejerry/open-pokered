//! Bare-metal resource loading (`target_os = "none"`, `framebuffer` feature).
//!
//! Mirrors the hosted `resource` module's named load helpers (`load_tileset`,
//! `load_pokemon_front`, …) but sources tiles from the build-time
//! pre-converted registry (`crate::gba_assets::get_preconverted_asset`:
//! PNGs → GB 2bpp/1bpp bytes, baked by build.rs) instead of runtime PNG
//! decoding. Dimensions for `CachedTileSet::source_size` come from the
//! generated `gba_asset_dims` table.
//!
//! Encoding rule (matches build.rs): everything under `gfx/font/` is stored
//! 1bpp; every other category 2bpp. The per-call `load_asset_1bpp`/
//! `load_asset_2bpp` variants still decode from the registry's storage
//! encoding — for these two-color assets the resulting `TileSet` pixel data
//! is identical to the hosted PNG path (color 0 ↔ white, color 3 ↔ black),
//! with the same tile count.

use crate::alloc_prelude::*;
use crate::resource_catalog::{category_from_str, impl_named_loaders};
pub use crate::resource_catalog::{AssetCategory, PokemonSpriteSize};
use dotzuki_renderer::asset_provider::ResourceProvider;
use dotzuki_renderer::tile::{TileSet, TILE_PIXELS};

pub use crate::gba_rom_tile_dims as tile_dims;

/// Error type for the bare-metal loader: an asset missing from the
/// pre-converted registry (built from `gfx/` at compile time).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceError {
    pub key: String,
}

impl core::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "asset not in pre-converted registry: {}", self.key)
    }
}

pub type Result<T> = core::result::Result<T, ResourceError>;

// ---------------------------------------------------------------------------
// AssetRoot — no filesystem on bare metal; the registry IS the root
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct AssetRoot;

impl AssetRoot {
    pub fn new() -> Self {
        Self
    }
}

// ---------------------------------------------------------------------------
// CachedTileSet (field-compatible with the hosted struct)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CachedTileSet {
    pub tileset: TileSet,
    pub source_size: (u32, u32),
    pub tile_count: usize,
}

// ---------------------------------------------------------------------------
// ResourceManager
// ---------------------------------------------------------------------------

pub struct ResourceManager {
    root: AssetRoot,
    /// Least recently used first. Screen lifetimes can include all 151 dex
    /// pictures or many PC/battle sprites, so lifecycle clearing alone is
    /// insufficient on GBA. Evict BEFORE decoding the next asset.
    cache: Vec<CachedAsset>,
    /// Registry misses are immutable for the lifetime of the ROM. Remember
    /// them so optional assets do not rescan the full generated table every
    /// frame.
    missing: Vec<(AssetCategory, String)>,
}

const CACHE_BYTES: usize = 16 * 1024;
const CACHE_ENTRIES: usize = 32;
const MISSING_ENTRIES: usize = 16;

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[test]
    fn rom_lookup_and_battle_bank_match_decoded_assets() {
        use crate::gba_assets::{get_preconverted_asset, BATTLE_TILE_PIXELS, PRECONVERTED_ASSETS};
        for &(dir, name, bytes) in PRECONVERTED_ASSETS {
            assert_eq!(
                get_preconverted_asset(dir, name),
                Some(bytes),
                "{dir}/{name}"
            );
        }
        assert!(get_preconverted_asset("battle", "missing").is_none());
        assert!(get_preconverted_asset("missing", "font").is_none());

        let mut expected = TileSet::blank(256);
        let mut put = |dir, name, first, limit| {
            let bytes = get_preconverted_asset(dir, name).unwrap();
            let tiles = if dir == "font" {
                TileSet::from_1bpp(bytes)
            } else {
                TileSet::from_2bpp(bytes)
            };
            let count = tiles.len().min(limit);
            for index in 0..count {
                expected.set(first + index, tiles.get(index).clone());
            }
            count
        };
        put("font", "font", 0x80, 128);
        put("font", "font_extra", 0x60, 32);
        put("font", "font_battle_extra", 0x62, 256 - 0x62);
        put("battle", "battle_hud_1", 0x6d, 256 - 0x6d);
        let hud2 = put("battle", "battle_hud_2", 0x73, 256 - 0x73);
        put("battle", "battle_hud_3", 0x73 + hud2, 256 - 0x73 - hud2);
        put("battle", "balls", 0x31, 5);
        for index in 0..256 {
            assert_eq!(
                &BATTLE_TILE_PIXELS.0[index * 64..(index + 1) * 64],
                expected.get(index).pixels.as_flattened(),
                "tile {index:#x}"
            );
        }
    }

    #[test]
    fn all_rom_assets_fit_individually_and_dex_reload_preserves_pixels() {
        let mut rm = ResourceManager::new(AssetRoot::new());
        for &(dir, name, bytes) in crate::gba_assets::PRECONVERTED_ASSETS {
            let size = bytes.len() / if dir == "font" { 8 } else { 16 } * 64;
            assert!(
                size <= CACHE_BYTES,
                "oversize ROM asset {dir}/{name}: {size}"
            );
        }
        // Two sweeps require evicted pictures to be decoded again. Compare
        // every pixel with an uncached decode, not just the entry count.
        for _ in 0..2 {
            for &(dir, name, bytes) in crate::gba_assets::PRECONVERTED_ASSETS {
                if dir != "pokemon/front" {
                    continue;
                }
                let expected = TileSet::from_2bpp(bytes);
                let loaded = rm.load_pokemon_front(name).unwrap();
                assert_eq!(loaded.tileset.len(), expected.len());
                for i in 0..expected.len() {
                    assert_eq!(
                        loaded.tileset.get(i).pixels,
                        expected.get(i).pixels,
                        "{name} tile {i}"
                    );
                }
                assert!(
                    rm.cache
                        .iter()
                        .map(|e| e.value.tile_count * 64)
                        .sum::<usize>()
                        <= CACHE_BYTES
                );
                assert!(rm.cache.len() <= CACHE_ENTRIES);
            }
        }
    }

    #[test]
    fn cache_hits_refresh_recency_and_normalize_extensions() {
        let mut rm = ResourceManager::new(AssetRoot::new());
        rm.load_pokemon_front("bulbasaur").unwrap();
        rm.load_pokemon_front("ivysaur").unwrap();
        rm.load_pokemon_front("bulbasaur.png").unwrap();
        assert_eq!(rm.cache.len(), 2);
        assert_eq!(rm.cache[0].name, "ivysaur");
        assert_eq!(rm.cache[1].name, "bulbasaur");
        rm.clear_cache();
        assert!(rm.cache.is_empty());
        assert_eq!(rm.cache.capacity(), 0);
    }

    #[test]
    fn missing_asset_names_cannot_grow_without_bound() {
        let mut rm = ResourceManager::new(AssetRoot::new());
        for i in 0..100 {
            assert!(rm.load_pokemon_front(&format!("missing-{i}")).is_err());
            assert!(rm.missing.len() <= MISSING_ENTRIES);
        }
        assert!(rm.load_pokemon_front("missing-99.png").is_err());
        assert_eq!(rm.missing.len(), MISSING_ENTRIES);
        rm.clear_cache();
        assert_eq!(rm.missing.capacity(), 0);
    }

    #[test]
    fn both_animation_sheets_remain_borrowable_after_eviction() {
        let mut rm = ResourceManager::new(AssetRoot::new());
        for &(dir, name, _) in crate::gba_assets::PRECONVERTED_ASSETS {
            if dir == "pokemon/front" {
                rm.load_pokemon_front(name).unwrap();
            }
        }
        let (a, b) = rm.load_battle_animation_pair().unwrap();
        for (name, loaded) in [("move_anim_0", a), ("move_anim_1", b)] {
            let bytes = crate::gba_assets::get_preconverted_asset("battle", name).unwrap();
            let expected = TileSet::from_2bpp(bytes);
            assert_eq!(loaded.len(), expected.len());
            for i in 0..expected.len() {
                assert_eq!(loaded.get(i).pixels, expected.get(i).pixels);
            }
        }
        assert!(
            rm.cache
                .iter()
                .map(|e| e.value.tile_count * 64)
                .sum::<usize>()
                <= CACHE_BYTES
        );
    }
}

struct CachedAsset {
    category: AssetCategory,
    name: String,
    value: CachedTileSet,
}

impl ResourceManager {
    pub fn new(root: AssetRoot) -> Self {
        Self {
            root,
            cache: Vec::new(),
            missing: Vec::new(),
        }
    }

    pub fn root(&self) -> &AssetRoot {
        &self.root
    }

    /// Drop decoded assets at a screen-lifecycle boundary. The GBA cannot
    /// retain every boot/title/Oak tileset alongside the overworld script
    /// registry in its 256 KiB EWRAM.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.cache.shrink_to_fit();
        // Miss entries are only a lookup optimization. At GBA screen
        // boundaries their retained Strings and Vec capacity compete with
        // the next screen's decoded tiles, so trade the rescan for headroom.
        self.missing.clear();
        self.missing.shrink_to_fit();
    }

    /// Resolve `name` (with or without `.png`, possibly nested like
    /// `flower/flower0.png`) inside `subdir` to the registry's
    /// `(&'static dir, &'static stem)` pair.
    fn registry_key(subdir: &str, name: &str) -> Option<(&'static str, &'static str)> {
        let name = name.strip_suffix(".png").unwrap_or(name);
        let nested = name.rsplit_once('/');
        crate::gba_assets::PRECONVERTED_ASSETS
            .iter()
            .find(|(s, n, _)| {
                if let Some((relative_dir, stem)) = nested {
                    // Nested asset (e.g. tilesets/flower/flower0): compare
                    // the registry directory in two borrowed pieces instead
                    // of formatting a temporary full path for every entry.
                    s.strip_prefix(subdir)
                        .and_then(|rest| rest.strip_prefix('/'))
                        == Some(relative_dir)
                        && *n == stem
                } else {
                    *s == subdir && *n == name
                }
            })
            .map(|(s, n, _)| (*s, *n))
    }

    fn load_and_cache(&mut self, category: AssetCategory, name: &str) -> Result<&CachedTileSet> {
        let normalized_name = name.strip_suffix(".png").unwrap_or(name);
        if let Some(index) = self
            .cache
            .iter()
            .position(|entry| entry.category == category && entry.name == normalized_name)
        {
            self.cache[index..].rotate_left(1);
            return Ok(&self.cache.last().expect("cache hit").value);
        }
        let subdir = category.subdir();
        if self
            .missing
            .iter()
            .any(|(kind, missing_name)| *kind == category && missing_name == normalized_name)
        {
            return Err(ResourceError {
                key: format!("{}/{}", subdir, normalized_name),
            });
        }

        let Some((reg_dir, reg_stem)) = Self::registry_key(subdir, normalized_name) else {
            let cache_key = format!("{}/{}", subdir, normalized_name);
            log::warn!("gba-asset miss: {}", cache_key);
            if self.missing.len() == MISSING_ENTRIES {
                self.missing.remove(0);
            }
            self.missing.push((category, normalized_name.to_string()));
            return Err(ResourceError { key: cache_key });
        };
        let bytes =
            crate::gba_assets::get_preconverted_asset(reg_dir, reg_stem).ok_or_else(|| {
                log::warn!("gba-asset registry miss: {}/{}", reg_dir, reg_stem);
                ResourceError {
                    key: format!("{}/{}", subdir, normalized_name),
                }
            })?;
        let decoded_bytes = bytes.len() / if reg_dir == "font" { 8 } else { 16 }
            * core::mem::size_of::<dotzuki_renderer::tile::Tile>();
        let mut retained_bytes: usize = self
            .cache
            .iter()
            .map(|entry| {
                entry.value.tile_count * core::mem::size_of::<dotzuki_renderer::tile::Tile>()
            })
            .sum();
        while !self.cache.is_empty()
            && (retained_bytes + decoded_bytes > CACHE_BYTES || self.cache.len() >= CACHE_ENTRIES)
        {
            let oldest = self.cache.remove(0);
            retained_bytes -=
                oldest.value.tile_count * core::mem::size_of::<dotzuki_renderer::tile::Tile>();
        }
        // An individual asset larger than the budget is allowed only as the
        // sole entry. Current game assets fit; this also preserves the loader
        // contract for future assets without retaining other decoded data.
        // Decode with the registry's storage encoding (font → 1bpp,
        // everything else → 2bpp). Tile splitting must match the hosted
        // per-tile decode, and it does for both encodings.
        let tileset = if reg_dir == "font" {
            TileSet::from_1bpp(bytes)
        } else {
            TileSet::from_2bpp(bytes)
        };
        let source_size = tile_dims(reg_dir, reg_stem).unwrap_or(((tileset.len() as u32) * 8, 8));
        let tile_count = tileset.len();
        self.cache.push(CachedAsset {
            category,
            name: normalized_name.to_string(),
            value: CachedTileSet {
                tileset,
                source_size,
                tile_count,
            },
        });
        Ok(&self.cache.last().expect("just inserted").value)
    }

    pub fn load(&mut self, category: AssetCategory, name: &str) -> Result<&CachedTileSet> {
        self.load_and_cache(category, name)
    }

    /// Borrow both battle-animation sheets together. Copying these sheets
    /// every frame adds several KiB to the peak precisely while an attack's
    /// other effects are active. All current pairs fit the cache budget.
    pub fn load_battle_animation_pair(&mut self) -> Result<(&TileSet, &TileSet)> {
        self.load(AssetCategory::Battle, "move_anim_0")?;
        self.load(AssetCategory::Battle, "move_anim_1")?;
        let find = |name| {
            self.cache
                .iter()
                .find(|entry| entry.category == AssetCategory::Battle && entry.name == name)
                .map(|entry| &entry.value.tileset)
                .ok_or_else(|| ResourceError {
                    key: format!("battle/{name}: animation pair exceeds cache budget"),
                })
        };
        Ok((find("move_anim_0")?, find("move_anim_1")?))
    }

    impl_named_loaders!();

    // ── Generic (category-typed) API used by the app render code ───────────

    pub fn load_asset(
        &mut self,
        category: AssetCategory,
        filename: &str,
    ) -> Result<&CachedTileSet> {
        self.load(category, filename)
    }

    /// Hosted twin decodes the PNG as 2bpp regardless of category; on bare
    /// metal the registry stores `font/` as 1bpp bytes, and decoding those
    /// with `from_1bpp` yields the identical TileSet for these two-color
    /// assets (and the same tile count as the hosted 2bpp decode).
    pub fn load_asset_2bpp(
        &mut self,
        category: AssetCategory,
        filename: &str,
    ) -> Result<&CachedTileSet> {
        self.load(category, filename)
    }

    /// Hosted twin decodes the PNG as 1bpp (white/black only). The registry
    /// stores non-font assets as 2bpp; for these two-color assets the 2bpp
    /// decode produces the same pixels the hosted 1bpp decode would.
    pub fn load_asset_1bpp(
        &mut self,
        category: AssetCategory,
        filename: &str,
    ) -> Result<&CachedTileSet> {
        self.load(category, filename)
    }
}

impl ResourceProvider for ResourceManager {
    fn load_asset(
        &mut self,
        category: &str,
        filename: &str,
    ) -> core::result::Result<&TileSet, String> {
        let cat = category_from_str(category)
            .ok_or_else(|| format!("unknown asset category: {}", category))?;
        self.load(cat, filename)
            .map(|c| &c.tileset)
            .map_err(|e| e.to_string())
    }

    fn load_asset_2bpp(
        &mut self,
        category: &str,
        filename: &str,
    ) -> core::result::Result<&TileSet, String> {
        let cat = category_from_str(category)
            .ok_or_else(|| format!("unknown asset category: {}", category))?;
        self.load(cat, filename)
            .map(|c| &c.tileset)
            .map_err(|e| e.to_string())
    }

    fn load_font(&mut self, name: &str) -> core::result::Result<&TileSet, String> {
        self.load_font(name)
            .map(|c| &c.tileset)
            .map_err(|e| e.to_string())
    }
}

/// Tile size in pixels (8), re-exported for draw code convenience.
pub const TILE_SIZE: u32 = TILE_PIXELS as u32;
