//! Resource loading pipeline: PNG → tile data.
//!
//! The generic machinery — PNG → 1bpp/2bpp/4bpp/RGBA conversion, `LoadedPng`,
//! the `AssetRoot` path skeleton, the `ResourceManager` cache, and
//! `ResourceError` — lives in `dotzuki_renderer::resource` and is re-exported
//! here. This module keeps the pokered-specific parts:
//! - [`PokemonSpriteSize`] — 5×5/6×6/7×7 front, 4×4 back sprite dimensions
//! - [`AssetCategory`] — the pokered `gfx/` directory layout
//! - [`AssetRoot`] / [`ResourceManager`] — thin wrappers adding pokered's
//!   `POKERED_GFX_DIR` override, compile-time baked gfx path, the embedded
//!   asset registry (wasm32/android/ios), and the named load helpers
//!   (`load_tileset`, `load_pokemon_front`, …)

use crate::alloc_prelude::*;
use core::ops::{Deref, DerefMut};
use std::path::PathBuf;

pub use dotzuki_renderer::resource::{
    bw_to_color_index, grayscale_to_16_levels, grayscale_to_color_index,
    grayscale_to_color_index_strict, load_1bpp_from_png, load_2bpp_from_png, load_tileset_from_png,
    load_tileset_from_png_1bpp, png_to_1bpp, png_to_2bpp, png_to_4bpp, png_to_rgba,
    png_to_tileset_1bpp, png_to_tileset_2bpp, png_to_tileset_4bpp, png_to_tileset_rgba, AssetKind,
    CachedTileSet, EmbeddedAssetLoader, LoadedPng, ResourceError, Result,
};

use crate::resource_catalog::{category_from_str, impl_named_loaders};
pub use crate::resource_catalog::{AssetCategory, PokemonSpriteSize};
use dotzuki_renderer::asset_provider::ResourceProvider;
use dotzuki_renderer::tile::{RgbaTileSet, TileSet};

impl AssetKind for AssetCategory {
    fn subdir(self) -> &'static str {
        AssetCategory::subdir(self)
    }

    fn is_1bpp(self) -> bool {
        AssetCategory::is_1bpp(self)
    }
}

// ---------------------------------------------------------------------------
// AssetRoot — path resolution (pokered wrapper)
// ---------------------------------------------------------------------------

/// Resolves paths to asset files under the pokered `gfx/` directory.
///
/// Thin wrapper over [`dotzuki_renderer::resource::AssetRoot`] that adds
/// pokered's own auto-detection (`POKERED_GFX_DIR` override and the
/// compile-time baked repo-root `gfx/` path) before delegating to the
/// engine's generic search. All path-resolution methods
/// (`resolve`, `resolve_checked`, `list_pngs`, `gfx_dir`) are inherited via
/// `Deref`.
#[derive(Debug, Clone)]
pub struct AssetRoot(dotzuki_renderer::resource::AssetRoot);

impl AssetRoot {
    /// Create from an explicit `gfx/` directory path.
    pub fn new(gfx_dir: impl Into<PathBuf>) -> Result<Self> {
        dotzuki_renderer::resource::AssetRoot::new(gfx_dir).map(Self)
    }

    /// Construct without file-system validation, for wasm32.
    ///
    /// `load_asset` on wasm32 reads from the embedded byte registry, so
    /// `gfx_dir` is never accessed; the path-existence check is skipped here.
    pub fn new_wasm() -> Self {
        Self(dotzuki_renderer::resource::AssetRoot::new_wasm())
    }

    /// Create from a parent directory that contains a `gfx/` subdirectory.
    pub fn from_parent(parent: impl AsRef<std::path::Path>) -> Result<Self> {
        dotzuki_renderer::resource::AssetRoot::from_parent(parent).map(Self)
    }

    /// Try to auto-detect the asset root. Resolution order: the
    /// `POKERED_GFX_DIR` override, the repo-root `gfx/` dir baked relative to
    /// this crate's manifest, then the engine's generic search (a `gfx/` in
    /// or above the current directory, then next to the executable).
    pub fn auto_detect() -> Result<Self> {
        // Explicit override: POKERED_GFX_DIR points directly at the gfx/ directory.
        // Takes precedence over auto-detection so the binary can be launched from
        // any working directory.
        if let Ok(dir) = std::env::var("POKERED_GFX_DIR") {
            let gfx = PathBuf::from(&dir);
            if gfx.is_dir() {
                return Self::new(gfx);
            }
            log::warn!(
                "POKERED_GFX_DIR={dir:?} is not a directory; falling back to auto-detection"
            );
        }

        // Compile-time fallback: the repo-root gfx/ dir, resolved
        // relative to this crate's manifest (crates/pokered-renderer
        // → gfx). This makes `cargo run`/tests and a locally
        // built binary work from any working directory. A relocated/packaged
        // binary's baked path won't exist, so we fall through to the
        // engine's generic search below.
        {
            let baked = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../gfx"));
            if baked.is_dir() {
                return Self::new(baked);
            }
        }

        dotzuki_renderer::resource::AssetRoot::auto_detect().map(Self)
    }
}

impl Deref for AssetRoot {
    type Target = dotzuki_renderer::resource::AssetRoot;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// ResourceManager — load and cache assets (pokered wrapper)
// ---------------------------------------------------------------------------

/// Manages loading and caching of pokered's graphical resources.
///
/// Thin wrapper over the engine's
/// `dotzuki_renderer::resource::ResourceManager<AssetCategory>` that wires up
/// the embedded asset registry on wasm32/android/ios and adds the named
/// per-category load helpers. The generic cache API (`load_asset`,
/// `load_asset_2bpp`, `load_asset_1bpp`, `load`, `is_cached`, `evict`,
/// `clear_cache`, `cache_size`, `preload_category`, `root`) is inherited via
/// `Deref`/`DerefMut`.
pub struct ResourceManager(dotzuki_renderer::resource::ResourceManager<AssetCategory>);

impl ResourceManager {
    /// Create a new resource manager with the given asset root.
    pub fn new(root: AssetRoot) -> Self {
        let manager = dotzuki_renderer::resource::ResourceManager::new(root.0);
        // On wasm32/android/ios, assets are baked into the binary; load them
        // through the embedded registry instead of the file system.
        #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
        let manager = {
            let mut manager = manager;
            manager.set_embedded_loader(crate::embedded::get_embedded_asset);
            manager
        };
        Self(manager)
    }

    impl_named_loaders!();

    /// Load an RGBA tileset from a PNG file directly (no palette remapping).
    ///
    /// Unlike `load_tileset`, this returns [`RgbaTileSet`] with direct RGBA pixel data
    /// instead of palette-indexed tiles. The PNG is loaded from the gfx/ directory.
    ///
    /// This method does NOT cache the result.
    pub fn load_tileset_rgba(&self, name: &str) -> core::result::Result<RgbaTileSet, String> {
        self.0.load_tileset_rgba(AssetCategory::Tileset, name)
    }

    /// Load a tileset as 4bpp tile data (GBA-style 2-bitplane format).
    ///
    /// Converts the PNG to 4bpp bitplane data via `png_to_4bpp()` and caches the
    /// resulting [`TileSet`]. Subsequent calls return the cached reference.
    pub fn load_tileset_4bpp(&mut self, name: &str) -> core::result::Result<&TileSet, String> {
        self.0.load_tileset_4bpp(AssetCategory::Tileset, name)
    }

    /// Load a tileset as direct RGBA pixel data (no palette remapping).
    ///
    /// Converts the PNG to flat RGBA pixels via `png_to_rgba()` and caches the
    /// resulting [`TileSet`]. Subsequent calls return the cached reference.
    pub fn load_tileset_rgba_tileset(
        &mut self,
        name: &str,
    ) -> core::result::Result<&TileSet, String> {
        self.0
            .load_tileset_rgba_tileset(AssetCategory::Tileset, name)
    }
}

impl Deref for ResourceManager {
    type Target = dotzuki_renderer::resource::ResourceManager<AssetCategory>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ResourceManager {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// ---------------------------------------------------------------------------
// ResourceProvider implementation (bridges to dotzuki-renderer trait)
// ---------------------------------------------------------------------------

impl ResourceProvider for ResourceManager {
    fn load_asset(
        &mut self,
        category: &str,
        filename: &str,
    ) -> core::result::Result<&TileSet, String> {
        let cat = category_from_str(category)
            .ok_or_else(|| format!("unknown asset category: {}", category))?;
        self.0
            .load_asset(cat, filename)
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
        self.0
            .load_asset_2bpp(cat, filename)
            .map(|c| &c.tileset)
            .map_err(|e| e.to_string())
    }

    fn load_font(&mut self, name: &str) -> core::result::Result<&TileSet, String> {
        self.load_font(name)
            .map(|c| &c.tileset)
            .map_err(|e| e.to_string())
    }
}
