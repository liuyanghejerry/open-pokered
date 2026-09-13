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

mod gba_asset_dims;
pub use gba_asset_dims::tile_dims;

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
    /// Decoded tilesets. GBA screen lifecycles retain only a handful of
    /// entries, so a compact linear cache avoids allocating a combined
    /// "<subdir>/<stem>" String on every lookup.
    cache: Vec<CachedAsset>,
    /// Registry misses are immutable for the lifetime of the ROM. Remember
    /// them so optional assets do not rescan the full generated table every
    /// frame.
    missing: Vec<(AssetCategory, String)>,
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
            return Ok(&self.cache[index].value);
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
