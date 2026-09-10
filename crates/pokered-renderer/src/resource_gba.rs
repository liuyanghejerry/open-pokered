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
use alloc::collections::BTreeMap;

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
// Asset categories (twin of the hosted resource.rs enum)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetCategory {
    Tileset,
    Sprite,
    PokemonFront,
    PokemonFrontRG,
    PokemonBack,
    Font,
    Trainer,
    Battle,
    Title,
    Intro,
    TownMap,
    Splash,
    Emote,
    Trade,
    Player,
    Credits,
    Slots,
    Pokedex,
    Sgb,
    Overworld,
    Blockset,
    Icon,
    TrainerCard,
}

impl AssetCategory {
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

    pub fn is_1bpp(self) -> bool {
        matches!(self, Self::Font)
    }
}

fn category_from_str(s: &str) -> Option<AssetCategory> {
    Some(match s {
        "tilesets" => AssetCategory::Tileset,
        "sprites" => AssetCategory::Sprite,
        "pokemon/front" => AssetCategory::PokemonFront,
        "pokemon/front_rg" => AssetCategory::PokemonFrontRG,
        "pokemon/back" => AssetCategory::PokemonBack,
        "font" => AssetCategory::Font,
        "trainers" => AssetCategory::Trainer,
        "battle" => AssetCategory::Battle,
        "title" => AssetCategory::Title,
        "intro" => AssetCategory::Intro,
        "town_map" => AssetCategory::TownMap,
        "splash" => AssetCategory::Splash,
        "emotes" => AssetCategory::Emote,
        "trade" => AssetCategory::Trade,
        "player" => AssetCategory::Player,
        "credits" => AssetCategory::Credits,
        "slots" => AssetCategory::Slots,
        "pokedex" => AssetCategory::Pokedex,
        "sgb" => AssetCategory::Sgb,
        "overworld" => AssetCategory::Overworld,
        "blocksets" => AssetCategory::Blockset,
        "icons" => AssetCategory::Icon,
        "trainer_card" => AssetCategory::TrainerCard,
        _ => return None,
    })
}

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
    /// decoded tilesets keyed by "<subdir>/<stem>" (leading ".png" stripped)
    cache: BTreeMap<String, CachedTileSet>,
}

impl ResourceManager {
    pub fn new(root: AssetRoot) -> Self {
        Self {
            root,
            cache: BTreeMap::new(),
        }
    }

    pub fn root(&self) -> &AssetRoot {
        &self.root
    }

    /// Resolve `name` (with or without `.png`, possibly nested like
    /// `flower/flower0.png`) inside `subdir` to the registry's
    /// `(&'static dir, &'static stem)` pair.
    fn registry_key(subdir: &str, name: &str) -> Option<(&'static str, &'static str)> {
        let name = name.strip_suffix(".png").unwrap_or(name);
        crate::gba_assets::PRECONVERTED_ASSETS
            .iter()
            .find(|(s, n, _)| {
                if name.contains('/') {
                    // nested asset (e.g. tilesets/flower/flower0):
                    // dir is "subdir/name_dir", stem is the last segment.
                    let full = format!("{}/{}", subdir, name);
                    let (parent, stem) = full.rsplit_once('/').unwrap_or(("", &full));
                    *s == parent && *n == stem
                } else {
                    *s == subdir && *n == name
                }
            })
            .map(|(s, n, _)| (*s, *n))
    }

    fn load_and_cache(&mut self, subdir: &str, name: &str) -> Result<&CachedTileSet> {
        let (reg_dir, reg_stem) = Self::registry_key(subdir, name).ok_or_else(|| {
            log::warn!("gba-asset miss: {}/{}", subdir, name);
            ResourceError {
                key: format!("{}/{}", subdir, name),
            }
        })?;
        let cache_key = format!("{}/{}", reg_dir, reg_stem);
        if !self.cache.contains_key(&cache_key) {
            let bytes = crate::gba_assets::get_preconverted_asset(reg_dir, reg_stem).ok_or_else(|| {
                log::warn!("gba-asset registry miss: {}/{}", reg_dir, reg_stem);
                ResourceError { key: cache_key.clone() }
            })?;
            // Decode with the registry's storage encoding (font → 1bpp,
            // everything else → 2bpp). Tile splitting must match the hosted
            // per-tile decode, and it does for both encodings.
            let tileset = if reg_dir == "font" {
                TileSet::from_1bpp(bytes)
            } else {
                TileSet::from_2bpp(bytes)
            };
            let source_size = tile_dims(reg_dir, reg_stem).unwrap_or((
                (tileset.len() as u32) * 8,
                8,
            ));
            let tile_count = tileset.len();
            self.cache.insert(
                cache_key.clone(),
                CachedTileSet {
                    tileset,
                    source_size,
                    tile_count,
                },
            );
        }
        Ok(self.cache.get(&cache_key).expect("just inserted"))
    }

    pub fn load(&mut self, category: AssetCategory, name: &str) -> Result<&CachedTileSet> {
        self.load_and_cache(category.subdir(), name)
    }

    // ── Named helpers (same surface as the hosted resource module) ─────────

    pub fn load_tileset(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::Tileset, name)
    }

    pub fn load_sprite(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::Sprite, name)
    }

    pub fn load_pokemon_front(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::PokemonFront, name)
    }

    pub fn load_pokemon_front_rg(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::PokemonFrontRG, name)
    }

    pub fn load_pokemon_back(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::PokemonBack, name)
    }

    pub fn load_font(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::Font, name)
    }

    pub fn load_trainer(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::Trainer, name)
    }

    pub fn load_battle(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::Battle, name)
    }

    pub fn load_title(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::Title, name)
    }

    pub fn load_intro(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::Intro, name)
    }

    pub fn load_town_map(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::TownMap, name)
    }

    pub fn load_splash(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::Splash, name)
    }

    pub fn load_trade(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::Trade, name)
    }

    pub fn load_slots(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::Slots, name)
    }

    pub fn load_pokedex(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::Pokedex, name)
    }

    pub fn load_emote(&mut self, name: &str) -> Result<&CachedTileSet> {
        self.load(AssetCategory::Emote, name)
    }

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

/// Pokémon front/back sprite size helper (hosted twin in `resource.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PokemonSpriteSize {
    Small,
    Medium,
    Large,
}

impl PokemonSpriteSize {
    pub fn tiles(self) -> u32 {
        match self {
            Self::Small => 5,
            Self::Medium => 6,
            Self::Large => 7,
        }
    }

    pub fn pixels(self) -> u32 {
        self.tiles() * 8
    }

    pub const BACK_TILES: u32 = 4;
    pub const BACK_PIXELS: u32 = 32;
}

/// Tile size in pixels (8), re-exported for draw code convenience.
pub const TILE_SIZE: u32 = TILE_PIXELS as u32;
