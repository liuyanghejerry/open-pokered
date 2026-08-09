//! Resource loading pipeline: PNG → tile data.
//!
//! The pokered disassembly stores graphics as PNG files in the `gfx/` directory.
//! This module provides:
//! - PNG → 2bpp tile data conversion (grayscale to Game Boy color indices)
//! - PNG → 1bpp tile data conversion (for fonts)
//! - Asset path resolution
//! - A `ResourceManager` that loads and caches tilesets, sprites, and other graphics

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::GenericImageView;
use thiserror::Error;

use crate::tile::{RgbaTileSet, TileSet, TILE_PIXELS};
use jrpg_engine::render::Rgba;
use jrpg_renderer::asset_provider::ResourceProvider;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during resource loading.
#[derive(Debug, Error)]
pub enum ResourceError {
    #[error("asset root directory not found: {0}")]
    AssetRootNotFound(PathBuf),

    #[error("PNG file not found: {0}")]
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

pub type Result<T> = std::result::Result<T, ResourceError>;

// ---------------------------------------------------------------------------
// Grayscale → GB color index mapping
// ---------------------------------------------------------------------------

/// Map a grayscale pixel value (0–255) to a Game Boy color index (0–3).
///
/// The pokered PNGs use exactly four gray levels:
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
// Pokémon sprite sizes
// ---------------------------------------------------------------------------

/// Pokémon front sprite dimensions in tiles.
///
/// In the original game, front sprites come in three sizes:
/// - 5×5 tiles (40×40 px) — small Pokémon (e.g., Bulbasaur, Pikachu)
/// - 6×6 tiles (48×48 px) — medium Pokémon (e.g., Venusaur, Blastoise)
/// - 7×7 tiles (56×56 px) — large Pokémon (e.g., Charizard, Gyarados)
///
/// Back sprites are always 4×4 tiles (32×32 px).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PokemonSpriteSize {
    /// 5×5 tiles = 40×40 pixels
    Small,
    /// 6×6 tiles = 48×48 pixels
    Medium,
    /// 7×7 tiles = 56×56 pixels
    Large,
}

impl PokemonSpriteSize {
    /// Width/height in tiles.
    pub fn tiles(self) -> u32 {
        match self {
            Self::Small => 5,
            Self::Medium => 6,
            Self::Large => 7,
        }
    }

    /// Width/height in pixels.
    pub fn pixels(self) -> u32 {
        self.tiles() * 8
    }

    /// Determine size from pixel dimensions.
    pub fn from_dimensions(width: u32, height: u32) -> Option<Self> {
        match (width, height) {
            (40, 40) => Some(Self::Small),
            (48, 48) => Some(Self::Medium),
            (56, 56) => Some(Self::Large),
            _ => None,
        }
    }

    /// Back sprite size (always 4×4 = 32×32).
    pub const BACK_TILES: u32 = 4;
    pub const BACK_PIXELS: u32 = 32;
}

// ---------------------------------------------------------------------------
// Asset categories
// ---------------------------------------------------------------------------

/// Categories of graphical assets in the pokered `gfx/` directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetCategory {
    /// Tilesets (gfx/tilesets/*.png) — overworld, house, cave, etc.
    Tileset,
    /// Overworld sprites (gfx/sprites/*.png) — player, NPCs
    Sprite,
    /// Pokémon front sprites (gfx/pokemon/front/*.png or front_rg/*.png)
    PokemonFront,
    /// Pokémon front sprites, Red/Green version (gfx/pokemon/front_rg/*.png)
    PokemonFrontRG,
    /// Pokémon back sprites (gfx/pokemon/back/*.png)
    PokemonBack,
    /// Font glyphs (gfx/font/*.png)
    Font,
    /// Trainer sprites (gfx/trainers/*.png)
    Trainer,
    /// Battle UI elements (gfx/battle/*.png)
    Battle,
    /// Title screen graphics (gfx/title/*.png)
    Title,
    /// Intro sequence graphics (gfx/intro/*.png)
    Intro,
    /// Town map (gfx/town_map/*.png)
    TownMap,
    /// Splash/copyright (gfx/splash/*.png)
    Splash,
    /// Overworld emotes (gfx/emotes/*.png)
    Emote,
    /// Trading animation (gfx/trade/*.png)
    Trade,
    /// Player-specific graphics (gfx/player/*.png)
    Player,
    /// Credits (gfx/credits/*.png)
    Credits,
    /// Slot machine (gfx/slots/*.png)
    Slots,
    /// Pokédex graphics (gfx/pokedex/*.png)
    Pokedex,
    /// SGB border (gfx/sgb/*.png)
    Sgb,
    /// Overworld NPC/object graphics (gfx/overworld/*.png)
    Overworld,
    /// Blockset graphics (gfx/blocksets/*.png)
    Blockset,
    /// Icon graphics (gfx/icons/*.png)
    Icon,
    /// Trainer card (gfx/trainer_card/*.png)
    TrainerCard,
}

impl AssetCategory {
    /// Subdirectory name under `gfx/`.
    pub fn subdir(self) -> &'static str {
        match self {
            Self::Tileset => "tilesets",
            Self::Sprite => "sprites",
            Self::PokemonFront => "pokemon/front",
            Self::PokemonFrontRG => "pokemon/front_rg",
            Self::PokemonBack => "pokemon/back",
            Self::Font => "font",
            Self::Trainer => "trainers",
            Self::Battle => "battle",
            Self::Title => "title",
            Self::Intro => "intro",
            Self::TownMap => "town_map",
            Self::Splash => "splash",
            Self::Emote => "emotes",
            Self::Trade => "trade",
            Self::Player => "player",
            Self::Credits => "credits",
            Self::Slots => "slots",
            Self::Pokedex => "pokedex",
            Self::Sgb => "sgb",
            Self::Overworld => "overworld",
            Self::Blockset => "blocksets",
            Self::Icon => "icons",
            Self::TrainerCard => "trainer_card",
        }
    }

    /// Whether this category uses 1bpp encoding (fonts) vs 2bpp.
    pub fn is_1bpp(self) -> bool {
        matches!(self, Self::Font)
    }
}

// ---------------------------------------------------------------------------
// AssetRoot — path resolution
// ---------------------------------------------------------------------------

/// Resolves paths to asset files under a `gfx/` directory.
///
/// The asset root can be:
/// - The pokered disassembly root (containing `gfx/`)
/// - Directly the `gfx/` directory itself
/// - Any custom path
#[derive(Debug, Clone)]
pub struct AssetRoot {
    /// The `gfx/` directory path.
    gfx_dir: PathBuf,
}

impl AssetRoot {
    /// Create from an explicit `gfx/` directory path.
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
    /// `POKERED_GFX_DIR` override, the example's `gfx/` dir baked relative to
    /// this crate's manifest, then a `gfx/` in (or above) the current directory,
    /// then next to the executable.
    pub fn auto_detect() -> Result<Self> {
        // Explicit override: POKERED_GFX_DIR points directly at the gfx/ directory.
        // Takes precedence over auto-detection so the binary can be launched from
        // any working directory.
        if let Ok(dir) = std::env::var("POKERED_GFX_DIR") {
            let gfx = PathBuf::from(&dir);
            if gfx.is_dir() {
                return Ok(Self { gfx_dir: gfx });
            }
            log::warn!(
                "POKERED_GFX_DIR={dir:?} is not a directory; falling back to auto-detection"
            );
        }

        // Compile-time fallback: the pokered example's gfx/ dir, resolved
        // relative to this crate's manifest (examples/pokered/crates/pokered-renderer
        // → examples/pokered/gfx). This makes `cargo run`/tests and a locally
        // built binary work from any working directory. A relocated/packaged
        // binary's baked path won't exist, so we fall through to the search below.
        {
            let baked = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../gfx"));
            if baked.is_dir() {
                return Ok(Self { gfx_dir: baked });
            }
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

    /// Get the `gfx/` directory path.
    pub fn gfx_dir(&self) -> &Path {
        &self.gfx_dir
    }

    /// Resolve a path to a specific asset file.
    pub fn resolve(&self, category: AssetCategory, filename: &str) -> PathBuf {
        self.gfx_dir.join(category.subdir()).join(filename)
    }

    /// Resolve path and verify the file exists.
    pub fn resolve_checked(&self, category: AssetCategory, filename: &str) -> Result<PathBuf> {
        let path = self.resolve(category, filename);
        if !path.is_file() {
            return Err(ResourceError::PngNotFound(path));
        }
        Ok(path)
    }

    /// List all PNG files in a category directory.
    pub fn list_pngs(&self, category: AssetCategory) -> Result<Vec<PathBuf>> {
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

/// Manages loading and caching of graphical resources.
///
/// Assets are loaded on-demand and cached by their category + filename key.
/// The cache uses a simple `HashMap`; no eviction policy is needed because
/// the total asset set is bounded (~668 PNG files, ~5 MB total).
pub struct ResourceManager {
    root: AssetRoot,
    cache: HashMap<(AssetCategory, String), CachedTileSet>,
}

impl ResourceManager {
    /// Create a new resource manager with the given asset root.
    pub fn new(root: AssetRoot) -> Self {
        Self {
            root,
            cache: HashMap::new(),
        }
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
    pub fn evict(&mut self, category: AssetCategory, filename: &str) -> bool {
        self.cache
            .remove(&(category, filename.to_string()))
            .is_some()
    }

    /// Load a PNG file and convert it to a cached tileset.
    /// Returns from cache if already loaded.
    pub fn load_asset(
        &mut self,
        category: AssetCategory,
        filename: &str,
    ) -> Result<&CachedTileSet> {
        let key = (category, filename.to_string());
        if !self.cache.contains_key(&key) {
            #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
            {
                // On wasm32/android/ios, load from embedded assets
                let relative_path = format!("{}/{}", category.subdir(), filename);
                let embedded =
                    crate::embedded::get_embedded_asset(&relative_path).ok_or_else(|| {
                        ResourceError::PngNotFound(std::path::PathBuf::from(&relative_path))
                    })?;
                let loaded = LoadedPng::load_from_bytes(embedded)?;
                let tileset = loaded.to_tileset(category.is_1bpp())?;
                let entry = CachedTileSet {
                    tile_count: tileset.len(),
                    source_size: loaded.dimensions,
                    tileset,
                };
                self.cache.insert(key.clone(), entry);
            }
            #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
            {
                // On native, load from file system
                let path = self.root.resolve_checked(category, filename)?;
                let loaded = LoadedPng::load(&path)?;
                let tileset = loaded.to_tileset(category.is_1bpp())?;
                let entry = CachedTileSet {
                    tile_count: tileset.len(),
                    source_size: loaded.dimensions,
                    tileset,
                };
                self.cache.insert(key.clone(), entry);
            }
        }
        Ok(self.cache.get(&key).unwrap())
    }

    /// Like `load_asset` but forces 2bpp decoding regardless of category default.
    ///
    /// Necessary for assets in `AssetCategory::Font` whose category default is
    /// 1bpp but whose PNG data is actually 2bpp (e.g. `font_extra.png`).
    pub fn load_asset_2bpp(
        &mut self,
        category: AssetCategory,
        filename: &str,
    ) -> Result<&CachedTileSet> {
        let cache_key = (category, format!("{}:2bpp", filename));
        if !self.cache.contains_key(&cache_key) {
            let entry = self.load_raw(category, filename, false)?;
            self.cache.insert(cache_key.clone(), entry);
        }
        Ok(self.cache.get(&cache_key).unwrap())
    }

    /// Like `load_asset` but forces 1bpp decoding regardless of category default.
    ///
    /// Necessary for assets in `AssetCategory::Battle` whose category default is
    /// 2bpp but which are stored as 1bpp on the Game Boy (e.g. `battle_hud_*.png`).
    pub fn load_asset_1bpp(
        &mut self,
        category: AssetCategory,
        filename: &str,
    ) -> Result<&CachedTileSet> {
        let cache_key = (category, format!("{}:1bpp", filename));
        if !self.cache.contains_key(&cache_key) {
            let entry = self.load_raw(category, filename, true)?;
            self.cache.insert(cache_key.clone(), entry);
        }
        Ok(self.cache.get(&cache_key).unwrap())
    }

    fn load_raw(
        &self,
        category: AssetCategory,
        filename: &str,
        is_1bpp: bool,
    ) -> Result<CachedTileSet> {
        #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
        {
            let relative_path = format!("{}/{}", category.subdir(), filename);
            let embedded =
                crate::embedded::get_embedded_asset(&relative_path).ok_or_else(|| {
                    ResourceError::PngNotFound(std::path::PathBuf::from(&relative_path))
                })?;
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
    pub fn load_tileset(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::Tileset, &filename)
    }

    /// Load an RGBA tileset from a PNG file directly (no palette remapping).
    ///
    /// Unlike `load_tileset`, this returns [`RgbaTileSet`] with direct RGBA pixel data
    /// instead of palette-indexed tiles. The PNG is loaded from the gfx/ directory.
    ///
    /// This method does NOT cache the result.
    pub fn load_tileset_rgba(&self, name: &str) -> std::result::Result<RgbaTileSet, String> {
        let filename = ensure_png_ext(name);
        let path = self.root.gfx_dir().join("tilesets").join(&filename);
        let data = std::fs::read(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        RgbaTileSet::from_rgba_png(&data)
    }

    /// Load a tileset as 4bpp tile data (GBA-style 2-bitplane format).
    ///
    /// Converts the PNG to 4bpp bitplane data via `png_to_4bpp()` and caches the
    /// resulting [`TileSet`]. Subsequent calls return the cached reference.
    pub fn load_tileset_4bpp(&mut self, name: &str) -> std::result::Result<&TileSet, String> {
        let filename = ensure_png_ext(name);
        let cache_key = (AssetCategory::Tileset, format!("{}:4bpp", filename));
        if !self.cache.contains_key(&cache_key) {
            let entry = self
                .load_tileset_4bpp_raw(&filename)
                .map_err(|e| e.to_string())?;
            self.cache.insert(cache_key.clone(), entry);
        }
        Ok(&self.cache.get(&cache_key).unwrap().tileset)
    }

    /// Load a tileset as direct RGBA pixel data (no palette remapping).
    ///
    /// Converts the PNG to flat RGBA pixels via `png_to_rgba()` and caches the
    /// resulting [`TileSet`]. Subsequent calls return the cached reference.
    pub fn load_tileset_rgba_tileset(&mut self, name: &str) -> std::result::Result<&TileSet, String> {
        let filename = ensure_png_ext(name);
        let cache_key = (AssetCategory::Tileset, format!("{}:rgba", filename));
        if !self.cache.contains_key(&cache_key) {
            let entry = self
                .load_tileset_rgba_raw(&filename)
                .map_err(|e| e.to_string())?;
            self.cache.insert(cache_key.clone(), entry);
        }
        Ok(&self.cache.get(&cache_key).unwrap().tileset)
    }

    /// Internal: load a PNG as 4bpp tile data, no caching.
    fn load_tileset_4bpp_raw(&self, filename: &str) -> Result<CachedTileSet> {
        #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
        {
            let relative_path = format!("{}/{}", AssetCategory::Tileset.subdir(), filename);
            let embedded = crate::embedded::get_embedded_asset(&relative_path).ok_or_else(|| {
                ResourceError::PngNotFound(std::path::PathBuf::from(&relative_path))
            })?;
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
            let path = self.root.resolve_checked(AssetCategory::Tileset, filename)?;
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
    fn load_tileset_rgba_raw(&self, filename: &str) -> Result<CachedTileSet> {
        #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
        {
            let relative_path = format!("{}/{}", AssetCategory::Tileset.subdir(), filename);
            let embedded = crate::embedded::get_embedded_asset(&relative_path).ok_or_else(|| {
                ResourceError::PngNotFound(std::path::PathBuf::from(&relative_path))
            })?;
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
            let path = self.root.resolve_checked(AssetCategory::Tileset, filename)?;
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

    /// Load an overworld sprite PNG (from gfx/sprites/).
    pub fn load_sprite(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::Sprite, &filename)
    }

    /// Load a Pokémon front sprite (Blue version, from gfx/pokemon/front/).
    pub fn load_pokemon_front(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::PokemonFront, &filename)
    }

    /// Load a Pokémon front sprite (Red/Green version, from gfx/pokemon/front_rg/).
    pub fn load_pokemon_front_rg(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::PokemonFrontRG, &filename)
    }

    /// Load a Pokémon back sprite (from gfx/pokemon/back/).
    pub fn load_pokemon_back(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::PokemonBack, &filename)
    }

    /// Load a font PNG (from gfx/font/). Uses 1bpp encoding.
    pub fn load_font(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::Font, &filename)
    }

    /// Load a trainer sprite (from gfx/trainers/).
    pub fn load_trainer(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::Trainer, &filename)
    }

    /// Load a battle UI element (from gfx/battle/).
    pub fn load_battle(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::Battle, &filename)
    }

    /// Load a title screen graphic (from gfx/title/).
    pub fn load_title(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::Title, &filename)
    }

    /// Load an intro graphic (from gfx/intro/).
    pub fn load_intro(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::Intro, &filename)
    }

    /// Load a town map graphic (from gfx/town_map/).
    pub fn load_town_map(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::TownMap, &filename)
    }

    /// Load a splash/copyright graphic (from gfx/splash/).
    pub fn load_splash(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::Splash, &filename)
    }

    /// Load a trade animation graphic (from gfx/trade/).
    pub fn load_trade(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::Trade, &filename)
    }

    /// Load a slot machine graphic (from gfx/slots/).
    pub fn load_slots(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::Slots, &filename)
    }

    /// Load a pokédex graphic (from gfx/pokedex/).
    pub fn load_pokedex(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::Pokedex, &filename)
    }

    pub fn load_emote(&mut self, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(AssetCategory::Emote, &filename)
    }

    /// Generic load by category and filename.
    pub fn load(&mut self, category: AssetCategory, name: &str) -> Result<&CachedTileSet> {
        let filename = ensure_png_ext(name);
        self.load_asset(category, &filename)
    }

    /// Check if an asset is already cached.
    pub fn is_cached(&self, category: AssetCategory, filename: &str) -> bool {
        self.cache.contains_key(&(category, filename.to_string()))
    }

    /// Pre-load all PNG files in a category directory.
    pub fn preload_category(&mut self, category: AssetCategory) -> Result<usize> {
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
// ResourceProvider implementation (bridges to jrpg-renderer trait)
// ---------------------------------------------------------------------------

impl ResourceProvider for ResourceManager {
    fn load_asset(&mut self, category: &str, filename: &str) -> std::result::Result<&TileSet, String> {
        let cat = category_from_str(category)?;
        self.load_asset(cat, filename)
            .map(|c| &c.tileset)
            .map_err(|e| e.to_string())
    }

    fn load_asset_2bpp(&mut self, category: &str, filename: &str) -> std::result::Result<&TileSet, String> {
        let cat = category_from_str(category)?;
        self.load_asset_2bpp(cat, filename)
            .map(|c| &c.tileset)
            .map_err(|e| e.to_string())
    }

    fn load_font(&mut self, name: &str) -> std::result::Result<&TileSet, String> {
        self.load_font(name)
            .map(|c| &c.tileset)
            .map_err(|e| e.to_string())
    }
}

fn category_from_str(s: &str) -> std::result::Result<AssetCategory, String> {
    match s {
        "tilesets" => Ok(AssetCategory::Tileset),
        "sprites" => Ok(AssetCategory::Sprite),
        "pokemon/front" => Ok(AssetCategory::PokemonFront),
        "pokemon/front_rg" => Ok(AssetCategory::PokemonFrontRG),
        "pokemon/back" => Ok(AssetCategory::PokemonBack),
        "font" => Ok(AssetCategory::Font),
        "trainers" => Ok(AssetCategory::Trainer),
        "battle" => Ok(AssetCategory::Battle),
        "title" => Ok(AssetCategory::Title),
        "intro" => Ok(AssetCategory::Intro),
        "town_map" => Ok(AssetCategory::TownMap),
        "splash" => Ok(AssetCategory::Splash),
        "emotes" => Ok(AssetCategory::Emote),
        "trade" => Ok(AssetCategory::Trade),
        "player" => Ok(AssetCategory::Player),
        "credits" => Ok(AssetCategory::Credits),
        "slots" => Ok(AssetCategory::Slots),
        "pokedex" => Ok(AssetCategory::Pokedex),
        "sgb" => Ok(AssetCategory::Sgb),
        "overworld" => Ok(AssetCategory::Overworld),
        "blocksets" => Ok(AssetCategory::Blockset),
        "icons" => Ok(AssetCategory::Icon),
        "trainer_card" => Ok(AssetCategory::TrainerCard),
        _ => Err(format!("unknown asset category: {}", s)),
    }
}
