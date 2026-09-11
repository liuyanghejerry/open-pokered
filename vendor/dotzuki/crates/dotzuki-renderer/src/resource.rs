//! Resource loading pipeline: PNG → tile data.
//!
//! Generic asset pipeline for games whose graphics are stored as PNG files
//! in an asset directory (conventionally `gfx/`):
//! - PNG → 2bpp tile data conversion (grayscale to Game Boy color indices)
//! - PNG → 1bpp tile data conversion (for fonts)
//! - PNG → 4bpp / RGBA conversion
//! - Asset path resolution ([`AssetRoot`])
//! - A [`ResourceManager`] that loads and caches tilesets on demand
//!
//! The game-specific parts — the concrete asset-category enum (directory
//! layout) and named load helpers — live in the game crate, which implements
//! [`AssetKind`] for its own category type.

use dotzuki_engine::hash::HashMap;
use std::path::{Path, PathBuf};

use image::GenericImageView;
use thiserror::Error;

use crate::tile::{RgbaTileSet, TileSet, TILE_PIXELS};
use dotzuki_engine::render::Rgba;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during resource loading.
#[derive(Debug, Error)]
pub enum ResourceError {
    // `{0:?}` (not `{0}`): thiserror's no_std mode has no Display wrapper for
    // std::path::PathBuf, so the path renders Debug-quoted.
    #[error("asset root directory not found: {0:?}")]
    AssetRootNotFound(PathBuf),

    #[error("PNG file not found: {0:?}")]
    PngNotFound(PathBuf),

    #[error("failed to load PNG: {0}")]
    ImageError(#[from] image::ImageError),

    #[error("PNG dimensions {width}×{height} are not a multiple of 8 pixels")]
    InvalidDimensions { width: u32, height: u32 },

    #[error("unexpected grayscale value {value} at ({x}, {y}); expected 0, 85, 170, or 255")]
    InvalidGrayscale { value: u8, x: u32, y: u32 },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = core::result::Result<T, ResourceError>;

// ---------------------------------------------------------------------------
// Grayscale → GB color index mapping
// ---------------------------------------------------------------------------

/// Map a grayscale pixel value (0–255) to a Game Boy color index (0–3).
///
/// Classic GB-style PNGs use exactly four gray levels:
/// - 255 (white)  → color 0 (lightest / white)
/// - 170 (light)  → color 1 (light gray)
/// -  85 (dark)   → color 2 (dark gray)
/// -   0 (black)  → color 3 (darkest / black)
///
/// For robustness, values are snapped to the nearest level:
/// - 213–255 → 0
/// - 128–212 → 1
/// -  43–127 → 2
/// -   0– 42 → 3
#[inline]
pub fn grayscale_to_color_index(value: u8) -> u8 {
    match value {
        213..=255 => 0,
        128..=212 => 1,
        43..=127 => 2,
        0..=42 => 3,
    }
}

/// Map a grayscale pixel value to a color index, strict mode.
/// Only accepts the exact values 0, 85, 170, 255.
#[inline]
pub fn grayscale_to_color_index_strict(value: u8) -> Option<u8> {
    match value {
        255 => Some(0),
        170 => Some(1),
        85 => Some(2),
        0 => Some(3),
        _ => None,
    }
}

/// Map a 1bpp pixel value (0 or 255) to a Game Boy color index.
/// - 255 (white) → color 0
/// -   0 (black) → color 3
#[inline]
pub fn bw_to_color_index(value: u8) -> u8 {
    if value >= 128 {
        0
    } else {
        3
    }
}

// ---------------------------------------------------------------------------
// PNG → raw tile data conversion
// ---------------------------------------------------------------------------

/// Convert a grayscale PNG image to 2bpp tile data.
///
/// The image is read left-to-right, top-to-bottom in 8×8 tile units.
/// For an image of `W×H` pixels (both multiples of 8), the tile order is:
///   - Tile (0,0), (1,0), ..., (W/8-1, 0), (0,1), (1,1), ..., (W/8-1, H/8-1)
///
/// Each tile produces 16 bytes of 2bpp data.
///
/// Returns the raw 2bpp byte vector suitable for `TileSet::from_2bpp()`.
pub fn png_to_2bpp(img: &image::DynamicImage) -> Result<Vec<u8>> {
    let (w, h) = img.dimensions();
    if w % 8 != 0 || h % 8 != 0 {
        return Err(ResourceError::InvalidDimensions {
            width: w,
            height: h,
        });
    }
    let gray = img.to_luma8();
    let tiles_x = (w / 8) as usize;
    let tiles_y = (h / 8) as usize;
    let total_tiles = tiles_x * tiles_y;
    let mut data = Vec::with_capacity(total_tiles * 16);

    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            // Each tile: 8 rows, each row = 2 bytes (lo, hi)
            for row in 0..TILE_PIXELS {
                let mut lo: u8 = 0;
                let mut hi: u8 = 0;
                for col in 0..TILE_PIXELS {
                    let px = gray.get_pixel((tx * 8 + col) as u32, (ty * 8 + row) as u32)[0];
                    let color = grayscale_to_color_index(px);
                    let bit = 7 - col;
                    lo |= (color & 1) << bit;
                    hi |= ((color >> 1) & 1) << bit;
                }
                data.push(lo);
                data.push(hi);
            }
        }
    }
    Ok(data)
}

/// Convert a 1bpp (black & white) PNG image to 1bpp tile data.
///
/// Each tile produces 8 bytes (1 byte per row, 1 bit per pixel).
/// bit=1 → black (color 3), bit=0 → white (color 0).
pub fn png_to_1bpp(img: &image::DynamicImage) -> Result<Vec<u8>> {
    let (w, h) = img.dimensions();
    if w % 8 != 0 || h % 8 != 0 {
        return Err(ResourceError::InvalidDimensions {
            width: w,
            height: h,
        });
    }
    let gray = img.to_luma8();
    let tiles_x = (w / 8) as usize;
    let tiles_y = (h / 8) as usize;
    let total_tiles = tiles_x * tiles_y;
    let mut data = Vec::with_capacity(total_tiles * 8);

    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            for row in 0..TILE_PIXELS {
                let mut byte: u8 = 0;
                for col in 0..TILE_PIXELS {
                    let px = gray.get_pixel((tx * 8 + col) as u32, (ty * 8 + row) as u32)[0];
                    // 1bpp: black pixel (0) → bit 1, white pixel (255) → bit 0
                    if px < 128 {
                        byte |= 1 << (7 - col);
                    }
                }
                data.push(byte);
            }
        }
    }
    Ok(data)
}

/// Convert a PNG to a `TileSet` using 2bpp encoding.
pub fn png_to_tileset_2bpp(img: &image::DynamicImage) -> Result<TileSet> {
    let data = png_to_2bpp(img)?;
    Ok(TileSet::from_2bpp(&data))
}

/// Convert a PNG to a `TileSet` using 1bpp encoding.
pub fn png_to_tileset_1bpp(img: &image::DynamicImage) -> Result<TileSet> {
    let data = png_to_1bpp(img)?;
    Ok(TileSet::from_1bpp(&data))
}

// ---------------------------------------------------------------------------
// 4bpp / RGBA conversion
// ---------------------------------------------------------------------------

/// Map a grayscale pixel value (0–255) to a 4bpp color index (0–15).
///
/// 0 → 0, 17 → 1, 34 → 2, …, 255 → 15
#[inline]
pub fn grayscale_to_16_levels(value: u8) -> u8 {
    let level = ((value as u16) * 16) / 256;
    level.min(15) as u8
}

/// Convert a grayscale PNG image to 4bpp tile data (GBA-style 2-bitplane format).
///
/// Each tile produces 32 bytes:
/// - First 16 bytes: bitplane 0 (lower 2 bits per pixel, standard 2bpp encoding)
/// - Next 16 bytes: bitplane 1 (upper 2 bits per pixel, standard 2bpp encoding)
///
/// Suitable for `TileSet::from_4bpp()`.
pub fn png_to_4bpp(img: &image::DynamicImage) -> Result<Vec<u8>> {
    let (w, h) = img.dimensions();
    if w % 8 != 0 || h % 8 != 0 {
        return Err(ResourceError::InvalidDimensions {
            width: w,
            height: h,
        });
    }
    let gray = img.to_luma8();
    let tiles_x = (w / 8) as usize;
    let tiles_y = (h / 8) as usize;
    let total_tiles = tiles_x * tiles_y;
    let mut data = Vec::with_capacity(total_tiles * 32);

    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            // Plane 0 (lower 2 bits): standard 2bpp encoding, 16 bytes
            for row in 0..TILE_PIXELS {
                let mut lo: u8 = 0;
                let mut hi: u8 = 0;
                for col in 0..TILE_PIXELS {
                    let px = gray.get_pixel((tx * 8 + col) as u32, (ty * 8 + row) as u32)[0];
                    let color = grayscale_to_16_levels(px);
                    let bit = 7 - col;
                    lo |= (color & 1) << bit;
                    hi |= ((color >> 1) & 1) << bit;
                }
                data.push(lo);
                data.push(hi);
            }
            // Plane 1 (upper 2 bits): standard 2bpp encoding, 16 bytes
            for row in 0..TILE_PIXELS {
                let mut lo: u8 = 0;
                let mut hi: u8 = 0;
                for col in 0..TILE_PIXELS {
                    let px = gray.get_pixel((tx * 8 + col) as u32, (ty * 8 + row) as u32)[0];
                    let color = grayscale_to_16_levels(px);
                    let bit = 7 - col;
                    lo |= ((color >> 2) & 1) << bit;
                    hi |= ((color >> 3) & 1) << bit;
                }
                data.push(lo);
                data.push(hi);
            }
        }
    }
    Ok(data)
}

/// Extract RGBA pixel data from a PNG image.
///
/// Returns a flat `Vec<Rgba>` of all pixels in row-major order.
/// The tile layout is expected to be handled by the caller (e.g. via
/// `TileSet::from_rgba()`).
pub fn png_to_rgba(img: &image::DynamicImage) -> Result<Vec<Rgba>> {
    let rgba = img.to_rgba8();
    let (w, h) = img.dimensions();
    let mut pixels = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let px = rgba.get_pixel(x, y);
            pixels.push(Rgba::from([px[0], px[1], px[2], px[3]]));
        }
    }
    Ok(pixels)
}

/// Convert a PNG to a `TileSet` using 4bpp encoding.
pub fn png_to_tileset_4bpp(img: &image::DynamicImage) -> Result<TileSet> {
    let data = png_to_4bpp(img)?;
    Ok(TileSet::from_4bpp(&data))
}

/// Convert a PNG to a `TileSet` using direct RGBA pixel data (no palette remapping).
pub fn png_to_tileset_rgba(img: &image::DynamicImage) -> Result<TileSet> {
    let (w, h) = img.dimensions();
    let tile_count = (w / 8 * h / 8) as usize;
    let pixels = png_to_rgba(img)?;
    Ok(TileSet::from_rgba(&pixels, tile_count))
}

// ---------------------------------------------------------------------------
// Asset categories
// ---------------------------------------------------------------------------

/// A category of graphical assets, mapping to a subdirectory of the asset
/// root (e.g. `tilesets`, `sprites`, `font`).
///
/// Games implement this for their own category enum that reflects their
/// asset directory layout.
pub trait AssetKind: Copy + Eq + core::hash::Hash {
    /// Subdirectory name under the asset root.
    fn subdir(self) -> &'static str;

    /// Whether this category uses 1bpp encoding (fonts) vs 2bpp.
    fn is_1bpp(self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// AssetRoot — path resolution
// ---------------------------------------------------------------------------

/// Resolves paths to asset files under an asset directory (conventionally
/// `gfx/`).
///
/// The asset root can be:
/// - A game repository root (containing `gfx/`)
/// - Directly the `gfx/` directory itself
/// - Any custom path
#[derive(Debug, Clone)]
pub struct AssetRoot {
    /// The asset directory path.
    gfx_dir: PathBuf,
}

impl AssetRoot {
    /// Create from an explicit asset directory path.
    pub fn new(gfx_dir: impl Into<PathBuf>) -> Result<Self> {
        let gfx_dir = gfx_dir.into();
        if !gfx_dir.is_dir() {
            return Err(ResourceError::AssetRootNotFound(gfx_dir));
        }
        Ok(Self { gfx_dir })
    }

    /// Construct without file-system validation, for wasm32.
    ///
    /// `load_asset` on wasm32 reads from the embedded byte registry, so
    /// `gfx_dir` is never accessed; the path-existence check is skipped here.
    pub fn new_wasm() -> Self {
        Self {
            gfx_dir: PathBuf::from("gfx"),
        }
    }

    /// Create from a parent directory that contains a `gfx/` subdirectory.
    pub fn from_parent(parent: impl AsRef<Path>) -> Result<Self> {
        let gfx_dir = parent.as_ref().join("gfx");
        if !gfx_dir.is_dir() {
            return Err(ResourceError::AssetRootNotFound(gfx_dir));
        }
        Ok(Self { gfx_dir })
    }

    /// Try to auto-detect the asset root. Resolution order: the
    /// `DOTZUKI_GFX_DIR` override, then a `gfx/` in (or above) the current
    /// directory, then next to the executable.
    ///
    /// Games typically layer their own override env var and compile-time
    /// baked path on top, then delegate to this for the generic search.
    pub fn auto_detect() -> Result<Self> {
        // Explicit override: DOTZUKI_GFX_DIR points directly at the gfx/
        // directory. Takes precedence over auto-detection so the binary can
        // be launched from any working directory.
        if let Ok(dir) = std::env::var("DOTZUKI_GFX_DIR") {
            let gfx = PathBuf::from(&dir);
            if gfx.is_dir() {
                return Ok(Self { gfx_dir: gfx });
            }
            log::warn!(
                "DOTZUKI_GFX_DIR={dir:?} is not a directory; falling back to auto-detection"
            );
        }

        // Try current working directory first
        if let Ok(cwd) = std::env::current_dir() {
            let gfx = cwd.join("gfx");
            if gfx.is_dir() {
                return Ok(Self { gfx_dir: gfx });
            }
            // Walk up parent directories (up to 5 levels)
            let mut dir = cwd.as_path().to_path_buf();
            for _ in 0..5 {
                if let Some(parent) = dir.parent() {
                    let gfx = parent.join("gfx");
                    if gfx.is_dir() {
                        return Ok(Self { gfx_dir: gfx });
                    }
                    dir = parent.to_path_buf();
                } else {
                    break;
                }
            }
        }

        // Try relative to executable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                let gfx = exe_dir.join("gfx");
                if gfx.is_dir() {
                    return Ok(Self { gfx_dir: gfx });
                }
            }
        }

        Err(ResourceError::AssetRootNotFound(PathBuf::from("gfx")))
    }

    /// Get the asset directory path.
    pub fn gfx_dir(&self) -> &Path {
        &self.gfx_dir
    }

    /// Resolve a path to a specific asset file.
    pub fn resolve<K: AssetKind>(&self, category: K, filename: &str) -> PathBuf {
        self.gfx_dir.join(category.subdir()).join(filename)
    }

    /// Resolve path and verify the file exists.
    pub fn resolve_checked<K: AssetKind>(&self, category: K, filename: &str) -> Result<PathBuf> {
        let path = self.resolve(category, filename);
        if !path.is_file() {
            return Err(ResourceError::PngNotFound(path));
        }
        Ok(path)
    }

    /// List all PNG files in a category directory.
    pub fn list_pngs<K: AssetKind>(&self, category: K) -> Result<Vec<PathBuf>> {
        let dir = self.gfx_dir.join(category.subdir());
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut files = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "png") {
                files.push(path);
            }
        }
        files.sort();
        Ok(files)
    }
}

// ---------------------------------------------------------------------------
// ResourceManager — load and cache assets
// ---------------------------------------------------------------------------

/// A cached resource entry.
#[derive(Debug, Clone)]
pub struct CachedTileSet {
    /// The decoded tileset.
    pub tileset: TileSet,
    /// Original PNG dimensions in pixels (width, height).
    pub source_size: (u32, u32),
    /// Number of tiles in the set.
    pub tile_count: usize,
}

/// A loaded PNG image before tile conversion.
#[derive(Debug)]
pub struct LoadedPng {
    /// The decoded image.
    pub image: image::DynamicImage,
    /// Image dimensions (width, height).
    pub dimensions: (u32, u32),
}

impl LoadedPng {
    /// Load a PNG file from disk.
    /// On wasm32, this always returns an error since file system is not available.
    /// Use `load_from_bytes` for embedded assets on wasm32.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        #[cfg(not(target_arch = "wasm32"))]
        {
            if !path.is_file() {
                return Err(ResourceError::PngNotFound(path.to_path_buf()));
            }
            let image = image::open(path)?;
            let dimensions = image.dimensions();
            Ok(Self { image, dimensions })
        }
        #[cfg(target_arch = "wasm32")]
        {
            // File system not available on wasm32
            Err(ResourceError::PngNotFound(path.to_path_buf()))
        }
    }

    /// Load a PNG from raw bytes (for embedded assets on wasm32).
    pub fn load_from_bytes(data: &[u8]) -> Result<Self> {
        use std::io::Cursor;
        let image = image::load(Cursor::new(data), image::ImageFormat::Png)?;
        let dimensions = image.dimensions();
        Ok(Self { image, dimensions })
    }

    /// Convert to 2bpp tile data.
    pub fn to_2bpp(&self) -> Result<Vec<u8>> {
        png_to_2bpp(&self.image)
    }

    /// Convert to 1bpp tile data.
    pub fn to_1bpp(&self) -> Result<Vec<u8>> {
        png_to_1bpp(&self.image)
    }

    /// Convert to a TileSet using the appropriate encoding.
    pub fn to_tileset(&self, is_1bpp: bool) -> Result<TileSet> {
        if is_1bpp {
            png_to_tileset_1bpp(&self.image)
        } else {
            png_to_tileset_2bpp(&self.image)
        }
    }

    /// Width in tiles.
    pub fn tiles_x(&self) -> u32 {
        self.dimensions.0 / 8
    }

    /// Height in tiles.
    pub fn tiles_y(&self) -> u32 {
        self.dimensions.1 / 8
    }
}

/// Loader for embedded assets on wasm32/android/ios: maps a path relative to
/// the asset root (e.g. `tilesets/overworld.png`) to embedded PNG bytes.
pub type EmbeddedAssetLoader = fn(&str) -> Option<&'static [u8]>;

/// Manages loading and caching of graphical resources.
///
/// Assets are loaded on-demand and cached by their category + filename key.
/// The cache uses a simple `HashMap`; no eviction policy is needed because
/// a game's total asset set is typically bounded.
///
/// `K` is the game's asset-category type (implements [`AssetKind`]).
pub struct ResourceManager<K: AssetKind> {
    root: AssetRoot,
    cache: HashMap<(K, String), CachedTileSet>,
    embedded_loader: Option<EmbeddedAssetLoader>,
}

impl<K: AssetKind> ResourceManager<K> {
    /// Create a new resource manager with the given asset root.
    pub fn new(root: AssetRoot) -> Self {
        Self {
            root,
            cache: HashMap::default(),
            embedded_loader: None,
        }
    }

    /// Register the embedded-asset loader used on wasm32/android/ios, where
    /// the file system is unavailable and assets are baked into the binary.
    pub fn set_embedded_loader(&mut self, loader: EmbeddedAssetLoader) {
        self.embedded_loader = Some(loader);
    }

    /// Get the asset root.
    pub fn root(&self) -> &AssetRoot {
        &self.root
    }

    /// Number of cached entries.
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Clear the entire cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Remove a specific entry from cache.
    pub fn evict(&mut self, category: K, filename: &str) -> bool {
        self.cache
            .remove(&(category, filename.to_string()))
            .is_some()
    }

    /// Load a PNG file and convert it to a cached tileset.
    /// Returns from cache if already loaded.
    pub fn load_asset(&mut self, category: K, filename: &str) -> Result<&CachedTileSet> {
        let key = (category, filename.to_string());
        if !self.cache.contains_key(&key) {
            let entry = self.load_raw(category, filename, category.is_1bpp())?;
            self.cache.insert(key.clone(), entry);
        }
        Ok(self.cache.get(&key).unwrap())
    }

    /// Like `load_asset` but forces 2bpp decoding regardless of category default.
    ///
    /// Necessary for assets in a 1bpp-by-default category (e.g. fonts) whose
    /// PNG data is actually 2bpp.
    pub fn load_asset_2bpp(&mut self, category: K, filename: &str) -> Result<&CachedTileSet> {
        let cache_key = (category, format!("{}:2bpp", filename));
        if !self.cache.contains_key(&cache_key) {
            let entry = self.load_raw(category, filename, false)?;
            self.cache.insert(cache_key.clone(), entry);
        }
        Ok(self.cache.get(&cache_key).unwrap())
    }

    /// Like `load_asset` but forces 1bpp decoding regardless of category default.
    ///
    /// Necessary for assets in a 2bpp-by-default category which are stored as
    /// 1bpp on the target hardware.
    pub fn load_asset_1bpp(&mut self, category: K, filename: &str) -> Result<&CachedTileSet> {
        let cache_key = (category, format!("{}:1bpp", filename));
        if !self.cache.contains_key(&cache_key) {
            let entry = self.load_raw(category, filename, true)?;
            self.cache.insert(cache_key.clone(), entry);
        }
        Ok(self.cache.get(&cache_key).unwrap())
    }

    fn load_raw(&self, category: K, filename: &str, is_1bpp: bool) -> Result<CachedTileSet> {
        #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
        {
            // On wasm32/android/ios, load from embedded assets
            let relative_path = format!("{}/{}", category.subdir(), filename);
            let embedded = self.embedded_asset(&relative_path)?;
            let loaded = LoadedPng::load_from_bytes(embedded)?;
            let tileset = loaded.to_tileset(is_1bpp)?;
            Ok(CachedTileSet {
                tile_count: tileset.len(),
                source_size: loaded.dimensions,
                tileset,
            })
        }
        #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
        {
            // On native, load from file system
            let path = self.root.resolve_checked(category, filename)?;
            let loaded = LoadedPng::load(&path)?;
            let tileset = loaded.to_tileset(is_1bpp)?;
            Ok(CachedTileSet {
                tile_count: tileset.len(),
                source_size: loaded.dimensions,
                tileset,
            })
        }
    }

    /// Look up an embedded asset by path relative to the asset root.
    #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
    fn embedded_asset(&self, relative_path: &str) -> Result<&'static [u8]> {
        self.embedded_loader
            .and_then(|loader| loader(relative_path))
            .ok_or_else(|| ResourceError::PngNotFound(PathBuf::from(relative_path)))
    }

    /// Load a tileset as 4bpp tile data (GBA-style 2-bitplane format).
    ///
    /// Converts the PNG to 4bpp bitplane data via `png_to_4bpp()` and caches
    /// the resulting [`TileSet`]. Subsequent calls return the cached reference.
    pub fn load_tileset_4bpp(
        &mut self,
        category: K,
        name: &str,
    ) -> core::result::Result<&TileSet, String> {
        let filename = ensure_png_ext(name);
        let cache_key = (category, format!("{}:4bpp", filename));
        if !self.cache.contains_key(&cache_key) {
            let entry = self
                .load_tileset_4bpp_raw(category, &filename)
                .map_err(|e| e.to_string())?;
            self.cache.insert(cache_key.clone(), entry);
        }
        Ok(&self.cache.get(&cache_key).unwrap().tileset)
    }

    /// Load a tileset as direct RGBA pixel data (no palette remapping).
    ///
    /// Converts the PNG to flat RGBA pixels via `png_to_rgba()` and caches the
    /// resulting [`TileSet`]. Subsequent calls return the cached reference.
    pub fn load_tileset_rgba_tileset(
        &mut self,
        category: K,
        name: &str,
    ) -> core::result::Result<&TileSet, String> {
        let filename = ensure_png_ext(name);
        let cache_key = (category, format!("{}:rgba", filename));
        if !self.cache.contains_key(&cache_key) {
            let entry = self
                .load_tileset_rgba_raw(category, &filename)
                .map_err(|e| e.to_string())?;
            self.cache.insert(cache_key.clone(), entry);
        }
        Ok(&self.cache.get(&cache_key).unwrap().tileset)
    }

    /// Internal: load a PNG as 4bpp tile data, no caching.
    fn load_tileset_4bpp_raw(&self, category: K, filename: &str) -> Result<CachedTileSet> {
        #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
        {
            let relative_path = format!("{}/{}", category.subdir(), filename);
            let embedded = self.embedded_asset(&relative_path)?;
            let loaded = LoadedPng::load_from_bytes(embedded)?;
            let data = png_to_4bpp(&loaded.image)?;
            let tileset = TileSet::from_4bpp(&data);
            Ok(CachedTileSet {
                tile_count: tileset.len(),
                source_size: loaded.dimensions,
                tileset,
            })
        }
        #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
        {
            let path = self.root.resolve_checked(category, filename)?;
            let loaded = LoadedPng::load(&path)?;
            let data = png_to_4bpp(&loaded.image)?;
            let tileset = TileSet::from_4bpp(&data);
            Ok(CachedTileSet {
                tile_count: tileset.len(),
                source_size: loaded.dimensions,
                tileset,
            })
        }
    }

    /// Internal: load a PNG as RGBA tile data, no caching.
    fn load_tileset_rgba_raw(&self, category: K, filename: &str) -> Result<CachedTileSet> {
        #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
        {
            let relative_path = format!("{}/{}", category.subdir(), filename);
            let embedded = self.embedded_asset(&relative_path)?;
            let loaded = LoadedPng::load_from_bytes(embedded)?;
            let pixels = png_to_rgba(&loaded.image)?;
            let tile_count = (loaded.dimensions.0 / 8 * loaded.dimensions.1 / 8) as usize;
            let tileset = TileSet::from_rgba(&pixels, tile_count);
            Ok(CachedTileSet {
                tile_count: tileset.len(),
                source_size: loaded.dimensions,
                tileset,
            })
        }
        #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
        {
            let path = self.root.resolve_checked(category, filename)?;
            let loaded = LoadedPng::load(&path)?;
            let pixels = png_to_rgba(&loaded.image)?;
            let tile_count = (loaded.dimensions.0 / 8 * loaded.dimensions.1 / 8) as usize;
            let tileset = TileSet::from_rgba(&pixels, tile_count);
            Ok(CachedTileSet {
                tile_count: tileset.len(),
                source_size: loaded.dimensions,
                tileset,
            })
        }
    }

    /// Load an RGBA tileset from a PNG file directly (no palette remapping).
    ///
    /// Unlike `load_asset`, this returns [`RgbaTileSet`] with direct RGBA
    /// pixel data instead of palette-indexed tiles. The PNG is loaded from
    /// the asset directory.
    ///
    /// This method does NOT cache the result.
    pub fn load_tileset_rgba(
        &self,
        category: K,
        name: &str,
    ) -> core::result::Result<RgbaTileSet, String> {
        let filename = ensure_png_ext(name);
        let path = self.root.gfx_dir().join(category.subdir()).join(&filename);
        let data = std::fs::read(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        RgbaTileSet::from_rgba_png(&data)
    }

    /// Generic load by category and filename (with or without `.png`).
    pub fn load(&mut self, category: K, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(category, &filename)
    }

    /// Check if an asset is already cached.
    pub fn is_cached(&self, category: K, filename: &str) -> bool {
        self.cache.contains_key(&(category, filename.to_string()))
    }

    /// Pre-load all PNG files in a category directory.
    pub fn preload_category(&mut self, category: K) -> Result<usize> {
        let files = self.root.list_pngs(category)?;
        let mut count = 0;
        for path in &files {
            if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                // Ignore errors on individual files during bulk preload
                if self.load_asset(category, filename).is_ok() {
                    count += 1;
                }
            }
        }
        Ok(count)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Ensure a filename has a `.png` extension.
fn ensure_png_ext(name: &str) -> String {
    if name.ends_with(".png") {
        name.to_string()
    } else {
        format!("{}.png", name)
    }
}

/// Convenience: load a single PNG file to a `TileSet` (2bpp) without caching.
pub fn load_tileset_from_png(path: impl AsRef<Path>) -> Result<TileSet> {
    let loaded = LoadedPng::load(path)?;
    png_to_tileset_2bpp(&loaded.image)
}

/// Convenience: load a single PNG file to a `TileSet` (1bpp) without caching.
pub fn load_tileset_from_png_1bpp(path: impl AsRef<Path>) -> Result<TileSet> {
    let loaded = LoadedPng::load(path)?;
    png_to_tileset_1bpp(&loaded.image)
}

/// Convenience: load a PNG and return raw 2bpp bytes.
pub fn load_2bpp_from_png(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let loaded = LoadedPng::load(path)?;
    png_to_2bpp(&loaded.image)
}

/// Convenience: load a PNG and return raw 1bpp bytes.
pub fn load_1bpp_from_png(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let loaded = LoadedPng::load(path)?;
    png_to_1bpp(&loaded.image)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- grayscale_to_color_index ---

    #[test]
    fn grayscale_white_maps_to_color_0() {
        assert_eq!(grayscale_to_color_index(255), 0);
    }

    #[test]
    fn grayscale_light_gray_maps_to_color_1() {
        assert_eq!(grayscale_to_color_index(170), 1);
    }

    #[test]
    fn grayscale_dark_gray_maps_to_color_2() {
        assert_eq!(grayscale_to_color_index(85), 2);
    }

    #[test]
    fn grayscale_black_maps_to_color_3() {
        assert_eq!(grayscale_to_color_index(0), 3);
    }

    #[test]
    fn grayscale_snapping_near_white() {
        // 213–255 → 0
        assert_eq!(grayscale_to_color_index(213), 0);
        assert_eq!(grayscale_to_color_index(240), 0);
    }

    #[test]
    fn grayscale_snapping_near_light_gray() {
        // 128–212 → 1
        assert_eq!(grayscale_to_color_index(128), 1);
        assert_eq!(grayscale_to_color_index(212), 1);
    }

    #[test]
    fn grayscale_snapping_near_dark_gray() {
        // 43–127 → 2
        assert_eq!(grayscale_to_color_index(43), 2);
        assert_eq!(grayscale_to_color_index(127), 2);
    }

    #[test]
    fn grayscale_snapping_near_black() {
        // 0–42 → 3
        assert_eq!(grayscale_to_color_index(42), 3);
        assert_eq!(grayscale_to_color_index(1), 3);
    }

    // --- grayscale_to_color_index_strict ---

    #[test]
    fn strict_grayscale_exact_values() {
        assert_eq!(grayscale_to_color_index_strict(255), Some(0));
        assert_eq!(grayscale_to_color_index_strict(170), Some(1));
        assert_eq!(grayscale_to_color_index_strict(85), Some(2));
        assert_eq!(grayscale_to_color_index_strict(0), Some(3));
    }

    #[test]
    fn strict_grayscale_rejects_non_standard() {
        assert_eq!(grayscale_to_color_index_strict(128), None);
        assert_eq!(grayscale_to_color_index_strict(200), None);
        assert_eq!(grayscale_to_color_index_strict(50), None);
        assert_eq!(grayscale_to_color_index_strict(1), None);
    }

    // --- bw_to_color_index ---

    #[test]
    fn bw_white_maps_to_color_0() {
        assert_eq!(bw_to_color_index(255), 0);
        assert_eq!(bw_to_color_index(128), 0);
    }

    #[test]
    fn bw_black_maps_to_color_3() {
        assert_eq!(bw_to_color_index(0), 3);
        assert_eq!(bw_to_color_index(127), 3);
    }

    // --- png_to_2bpp with synthetic image ---

    #[test]
    fn png_to_2bpp_single_white_tile() {
        // Create an 8×8 all-white image
        let img =
            image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(8, 8, image::Luma([255])));
        let data = png_to_2bpp(&img).unwrap();
        assert_eq!(data.len(), 16); // 1 tile × 16 bytes
                                    // All white = color 0 → all bits 0
        for byte in &data {
            assert_eq!(*byte, 0x00);
        }
    }

    #[test]
    fn png_to_2bpp_single_black_tile() {
        // Create an 8×8 all-black image
        let img =
            image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(8, 8, image::Luma([0])));
        let data = png_to_2bpp(&img).unwrap();
        assert_eq!(data.len(), 16);
        // All black = color 3 → both lo and hi bytes = 0xFF
        for byte in &data {
            assert_eq!(*byte, 0xFF);
        }
    }

    #[test]
    fn png_to_2bpp_alternating_colors() {
        // Create an 8×8 image where first row alternates white(255)/black(0)
        let mut img = image::GrayImage::from_pixel(8, 8, image::Luma([255]));
        // Set odd columns of first row to black
        for col in (1..8).step_by(2) {
            img.put_pixel(col, 0, image::Luma([0]));
        }
        let dyn_img = image::DynamicImage::ImageLuma8(img);
        let data = png_to_2bpp(&dyn_img).unwrap();

        // First row: pixels are [0, 3, 0, 3, 0, 3, 0, 3]
        // lo byte: bits 7,5,3,1 = 0; bits 6,4,2,0 = 1 → 0b01010101 = 0x55
        // hi byte: same → 0x55
        assert_eq!(data[0], 0x55); // lo
        assert_eq!(data[1], 0x55); // hi
    }

    #[test]
    fn png_to_2bpp_light_gray_tile() {
        // All light gray (170) → color 1 → lo=1, hi=0
        let img =
            image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(8, 8, image::Luma([170])));
        let data = png_to_2bpp(&img).unwrap();
        for row in 0..8 {
            assert_eq!(data[row * 2], 0xFF); // lo = all 1s (color 1, bit 0 = 1)
            assert_eq!(data[row * 2 + 1], 0x00); // hi = all 0s (color 1, bit 1 = 0)
        }
    }

    #[test]
    fn png_to_2bpp_dark_gray_tile() {
        // All dark gray (85) → color 2 → lo=0, hi=1
        let img =
            image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(8, 8, image::Luma([85])));
        let data = png_to_2bpp(&img).unwrap();
        for row in 0..8 {
            assert_eq!(data[row * 2], 0x00); // lo = all 0s (color 2, bit 0 = 0)
            assert_eq!(data[row * 2 + 1], 0xFF); // hi = all 1s (color 2, bit 1 = 1)
        }
    }

    #[test]
    fn png_to_2bpp_multi_tile() {
        // 16×8 image = 2 tiles side by side
        let mut img = image::GrayImage::from_pixel(16, 8, image::Luma([255])); // all white
                                                                               // Make second tile all black
        for y in 0..8 {
            for x in 8..16 {
                img.put_pixel(x, y, image::Luma([0]));
            }
        }
        let dyn_img = image::DynamicImage::ImageLuma8(img);
        let data = png_to_2bpp(&dyn_img).unwrap();
        assert_eq!(data.len(), 32); // 2 tiles

        // First tile: all white (all zeros)
        for i in 0..16 {
            assert_eq!(data[i], 0x00);
        }
        // Second tile: all black (all 0xFF)
        for i in 16..32 {
            assert_eq!(data[i], 0xFF);
        }
    }

    #[test]
    fn png_to_2bpp_2x2_tiles() {
        // 16×16 image = 4 tiles in 2×2 arrangement
        // Tile order should be: (0,0), (1,0), (0,1), (1,1) — row-major
        let mut img = image::GrayImage::from_pixel(16, 16, image::Luma([255])); // all white
                                                                                // Make tile (1,0) = top-right all black
        for y in 0..8 {
            for x in 8..16 {
                img.put_pixel(x, y, image::Luma([0]));
            }
        }
        let dyn_img = image::DynamicImage::ImageLuma8(img);
        let data = png_to_2bpp(&dyn_img).unwrap();
        assert_eq!(data.len(), 64); // 4 tiles

        // Tile 0 (0,0): white
        for i in 0..16 {
            assert_eq!(data[i], 0x00);
        }
        // Tile 1 (1,0): black
        for i in 16..32 {
            assert_eq!(data[i], 0xFF);
        }
        // Tile 2 (0,1): white
        for i in 32..48 {
            assert_eq!(data[i], 0x00);
        }
        // Tile 3 (1,1): white
        for i in 48..64 {
            assert_eq!(data[i], 0x00);
        }
    }

    #[test]
    fn png_to_2bpp_rejects_non_multiple_of_8() {
        let img =
            image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(10, 8, image::Luma([255])));
        let result = png_to_2bpp(&img);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ResourceError::InvalidDimensions {
                width: 10,
                height: 8
            }
        ));
    }

    // --- png_to_1bpp ---

    #[test]
    fn png_to_1bpp_all_white() {
        let img =
            image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(8, 8, image::Luma([255])));
        let data = png_to_1bpp(&img).unwrap();
        assert_eq!(data.len(), 8); // 1 tile × 8 bytes
        for byte in &data {
            assert_eq!(*byte, 0x00); // all white → all bits 0
        }
    }

    #[test]
    fn png_to_1bpp_all_black() {
        let img =
            image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(8, 8, image::Luma([0])));
        let data = png_to_1bpp(&img).unwrap();
        assert_eq!(data.len(), 8);
        for byte in &data {
            assert_eq!(*byte, 0xFF); // all black → all bits 1
        }
    }

    #[test]
    fn png_to_1bpp_checkerboard() {
        // First row: alternating white/black pixels
        let mut img = image::GrayImage::from_pixel(8, 8, image::Luma([255]));
        for col in (0..8).step_by(2) {
            img.put_pixel(col, 0, image::Luma([0]));
        }
        let dyn_img = image::DynamicImage::ImageLuma8(img);
        let data = png_to_1bpp(&dyn_img).unwrap();
        // First row: black(0), white(255), black(0), white(255), ... → 0b10101010 = 0xAA
        assert_eq!(data[0], 0xAA);
    }

    #[test]
    fn png_to_1bpp_rejects_non_multiple_of_8() {
        let img =
            image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(7, 8, image::Luma([255])));
        assert!(png_to_1bpp(&img).is_err());
    }

    // --- png_to_tileset_2bpp ---

    #[test]
    fn png_to_tileset_2bpp_produces_correct_tile_count() {
        // 16×16 → 4 tiles
        let img =
            image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(16, 16, image::Luma([255])));
        let ts = png_to_tileset_2bpp(&img).unwrap();
        assert_eq!(ts.len(), 4);
    }

    #[test]
    fn png_to_tileset_1bpp_produces_correct_tile_count() {
        // 128×64 → 128 tiles (like a font sheet)
        let img = image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(
            128,
            64,
            image::Luma([255]),
        ));
        let ts = png_to_tileset_1bpp(&img).unwrap();
        assert_eq!(ts.len(), 128);
    }

    // --- 2bpp roundtrip: png_to_2bpp then TileSet::from_2bpp ---

    #[test]
    fn png_to_2bpp_roundtrip_with_tileset() {
        // Create a synthetic image with known pixel values
        let mut img = image::GrayImage::new(8, 8);
        // Row 0: all four colors: 255, 170, 85, 0, 255, 170, 85, 0
        let colors = [255u8, 170, 85, 0, 255, 170, 85, 0];
        for (col, &val) in colors.iter().enumerate() {
            img.put_pixel(col as u32, 0, image::Luma([val]));
        }
        // Rows 1-7: all white
        for y in 1..8 {
            for x in 0..8 {
                img.put_pixel(x, y, image::Luma([255]));
            }
        }

        let dyn_img = image::DynamicImage::ImageLuma8(img);
        let data_2bpp = png_to_2bpp(&dyn_img).unwrap();
        let ts = crate::tile::TileSet::from_2bpp(&data_2bpp);
        assert_eq!(ts.len(), 1);

        let tile = ts.get(0);
        // Row 0: color indices [0, 1, 2, 3, 0, 1, 2, 3]
        assert_eq!(tile.pixels[0], [0, 1, 2, 3, 0, 1, 2, 3]);
        // Rows 1-7: all color 0
        for row in 1..8 {
            assert_eq!(tile.pixels[row], [0; 8]);
        }
    }

    // --- 1bpp roundtrip ---

    #[test]
    fn png_to_1bpp_roundtrip_with_tileset() {
        let mut img = image::GrayImage::new(8, 8);
        // Row 0: alternating B/W: 0, 255, 0, 255, 0, 255, 0, 255
        for col in 0..8 {
            let val = if col % 2 == 0 { 0u8 } else { 255 };
            img.put_pixel(col, 0, image::Luma([val]));
        }
        // Rows 1-7: all white
        for y in 1..8 {
            for x in 0..8 {
                img.put_pixel(x, y, image::Luma([255]));
            }
        }

        let dyn_img = image::DynamicImage::ImageLuma8(img);
        let data = png_to_1bpp(&dyn_img).unwrap();
        let ts = crate::tile::TileSet::from_1bpp(&data);
        assert_eq!(ts.len(), 1);

        let tile = ts.get(0);
        // Row 0: 1bpp black=color 3, white=color 0 → [3, 0, 3, 0, 3, 0, 3, 0]
        assert_eq!(tile.pixels[0], [3, 0, 3, 0, 3, 0, 3, 0]);
        // Rows 1-7: all white = color 0
        for row in 1..8 {
            assert_eq!(tile.pixels[row], [0; 8]);
        }
    }

    // --- LoadedPng ---

    #[test]
    fn loaded_png_missing_file() {
        let result = LoadedPng::load("/nonexistent/path/to/file.png");
        assert!(result.is_err());
    }

    // --- AssetRoot / ResourceManager (synthetic temp-dir assets) ---

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum TestKind {
        Tiles,
        Font,
    }

    impl AssetKind for TestKind {
        fn subdir(self) -> &'static str {
            match self {
                Self::Tiles => "tiles",
                Self::Font => "font",
            }
        }

        fn is_1bpp(self) -> bool {
            matches!(self, Self::Font)
        }
    }

    /// Create a unique temp `gfx/` dir with one 16×8 2bpp PNG under `tiles/`
    /// and one 8×8 1bpp PNG under `font/`; returns its `AssetRoot`.
    fn temp_asset_root() -> AssetRoot {
        let unique = format!(
            "dotzuki-resource-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let gfx = std::env::temp_dir().join(unique);
        let tiles = gfx.join("tiles");
        let font = gfx.join("font");
        std::fs::create_dir_all(&tiles).unwrap();
        std::fs::create_dir_all(&font).unwrap();
        image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(16, 8, image::Luma([170])))
            .save(tiles.join("a.png"))
            .unwrap();
        image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(8, 8, image::Luma([0])))
            .save(font.join("f.png"))
            .unwrap();
        AssetRoot::new(&gfx).unwrap()
    }

    #[test]
    fn asset_root_resolve_and_list() {
        let root = temp_asset_root();
        let path = root.resolve(TestKind::Tiles, "a.png");
        assert!(path.to_str().unwrap().contains("tiles/a.png"));
        assert!(root.resolve_checked(TestKind::Tiles, "a.png").is_ok());
        assert!(root.resolve_checked(TestKind::Tiles, "missing.png").is_err());
        let pngs = root.list_pngs(TestKind::Tiles).unwrap();
        assert_eq!(pngs.len(), 1);
    }

    #[test]
    fn resource_manager_caches_by_category_and_filename() {
        let root = temp_asset_root();
        let mut mgr = ResourceManager::<TestKind>::new(root);
        assert_eq!(mgr.cache_size(), 0);
        assert!(!mgr.is_cached(TestKind::Tiles, "a.png"));

        let cached = mgr.load_asset(TestKind::Tiles, "a.png").unwrap();
        // 16×8 → 2 tiles, 2bpp
        assert_eq!(cached.source_size, (16, 8));
        assert_eq!(cached.tile_count, 2);
        assert_eq!(mgr.cache_size(), 1);
        assert!(mgr.is_cached(TestKind::Tiles, "a.png"));

        // Second load is a cache hit — still one entry
        let _ = mgr.load_asset(TestKind::Tiles, "a.png").unwrap();
        assert_eq!(mgr.cache_size(), 1);

        // Forced 2bpp/1bpp variants get their own cache keys
        let _ = mgr.load_asset_2bpp(TestKind::Tiles, "a.png").unwrap();
        assert_eq!(mgr.cache_size(), 2);

        assert!(mgr.evict(TestKind::Tiles, "a.png"));
        assert_eq!(mgr.cache_size(), 1);
        assert!(!mgr.is_cached(TestKind::Tiles, "a.png"));

        mgr.clear_cache();
        assert_eq!(mgr.cache_size(), 0);
    }

    #[test]
    fn resource_manager_category_default_encoding() {
        let root = temp_asset_root();
        let mut mgr = ResourceManager::<TestKind>::new(root);
        // Font category defaults to 1bpp: 8×8 → 1 tile
        let cached = mgr.load(TestKind::Font, "f").unwrap();
        assert_eq!(cached.tile_count, 1);
        // Generic `load` adds the .png extension itself
        assert!(mgr.is_cached(TestKind::Font, "f.png"));
    }
}
