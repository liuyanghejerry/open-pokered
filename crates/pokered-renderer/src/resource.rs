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

use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

pub use dotzuki_renderer::resource::{
    bw_to_color_index, grayscale_to_16_levels, grayscale_to_color_index,
    grayscale_to_color_index_strict, load_1bpp_from_png, load_2bpp_from_png,
    load_tileset_from_png, load_tileset_from_png_1bpp, png_to_1bpp, png_to_2bpp, png_to_4bpp,
    png_to_rgba, png_to_tileset_1bpp, png_to_tileset_2bpp, png_to_tileset_4bpp,
    png_to_tileset_rgba, AssetKind, CachedTileSet, EmbeddedAssetLoader, LoadedPng, ResourceError,
    Result,
};

use dotzuki_renderer::asset_provider::ResourceProvider;
use dotzuki_renderer::tile::{RgbaTileSet, TileSet};

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

    /// Load a tileset (from gfx/tilesets/).
    pub fn load_tileset(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::Tileset, name)
    }

    /// Load an RGBA tileset from a PNG file directly (no palette remapping).
    ///
    /// Unlike `load_tileset`, this returns [`RgbaTileSet`] with direct RGBA pixel data
    /// instead of palette-indexed tiles. The PNG is loaded from the gfx/ directory.
    ///
    /// This method does NOT cache the result.
    pub fn load_tileset_rgba(&self, name: &str) -> std::result::Result<RgbaTileSet, String> {
        self.0.load_tileset_rgba(AssetCategory::Tileset, name)
    }

    /// Load a tileset as 4bpp tile data (GBA-style 2-bitplane format).
    ///
    /// Converts the PNG to 4bpp bitplane data via `png_to_4bpp()` and caches the
    /// resulting [`TileSet`]. Subsequent calls return the cached reference.
    pub fn load_tileset_4bpp(&mut self, name: &str) -> std::result::Result<&TileSet, String> {
        self.0.load_tileset_4bpp(AssetCategory::Tileset, name)
    }

    /// Load a tileset as direct RGBA pixel data (no palette remapping).
    ///
    /// Converts the PNG to flat RGBA pixels via `png_to_rgba()` and caches the
    /// resulting [`TileSet`]. Subsequent calls return the cached reference.
    pub fn load_tileset_rgba_tileset(
        &mut self,
        name: &str,
    ) -> std::result::Result<&TileSet, String> {
        self.0.load_tileset_rgba_tileset(AssetCategory::Tileset, name)
    }

    /// Load an overworld sprite PNG (from gfx/sprites/).
    pub fn load_sprite(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::Sprite, name)
    }

    /// Load a Pokémon front sprite (Blue version, from gfx/pokemon/front/).
    pub fn load_pokemon_front(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::PokemonFront, name)
    }

    /// Load a Pokémon front sprite (Red/Green version, from gfx/pokemon/front_rg/).
    pub fn load_pokemon_front_rg(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::PokemonFrontRG, name)
    }

    /// Load a Pokémon back sprite (from gfx/pokemon/back/).
    pub fn load_pokemon_back(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::PokemonBack, name)
    }

    /// Load a font PNG (from gfx/font/). Uses 1bpp encoding.
    pub fn load_font(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::Font, name)
    }

    /// Load a trainer sprite (from gfx/trainers/).
    pub fn load_trainer(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::Trainer, name)
    }

    /// Load a battle UI element (from gfx/battle/).
    pub fn load_battle(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::Battle, name)
    }

    /// Load a title screen graphic (from gfx/title/).
    pub fn load_title(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::Title, name)
    }

    /// Load an intro graphic (from gfx/intro/).
    pub fn load_intro(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::Intro, name)
    }

    /// Load a town map graphic (from gfx/town_map/).
    pub fn load_town_map(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::TownMap, name)
    }

    /// Load a splash/copyright graphic (from gfx/splash/).
    pub fn load_splash(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::Splash, name)
    }

    /// Load a trade animation graphic (from gfx/trade/).
    pub fn load_trade(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::Trade, name)
    }

    /// Load a slot machine graphic (from gfx/slots/).
    pub fn load_slots(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::Slots, name)
    }

    /// Load a pokédex graphic (from gfx/pokedex/).
    pub fn load_pokedex(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::Pokedex, name)
    }

    /// Load an overworld emote graphic (from gfx/emotes/).
    pub fn load_emote(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.0.load(AssetCategory::Emote, name)
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
    ) -> std::result::Result<&TileSet, String> {
        let cat = category_from_str(category)?;
        self.0
            .load_asset(cat, filename)
            .map(|c| &c.tileset)
            .map_err(|e| e.to_string())
    }

    fn load_asset_2bpp(
        &mut self,
        category: &str,
        filename: &str,
    ) -> std::result::Result<&TileSet, String> {
        let cat = category_from_str(category)?;
        self.0
            .load_asset_2bpp(cat, filename)
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
