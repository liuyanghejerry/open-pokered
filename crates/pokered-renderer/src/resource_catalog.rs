//! Target-independent Pokemon resource names and typed convenience surface.

/// Pokémon front sprite dimensions in tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PokemonSpriteSize {
    Small,
    Medium,
    Large,
}

impl PokemonSpriteSize {
    /// Width and height in 8-pixel tiles.
    pub fn tiles(self) -> u32 {
        match self {
            Self::Small => 5,
            Self::Medium => 6,
            Self::Large => 7,
        }
    }

    /// Width and height in pixels.
    pub fn pixels(self) -> u32 {
        self.tiles() * 8
    }

    /// Converts a square sprite's pixel dimensions to its canonical size.
    pub fn from_dimensions(width: u32, height: u32) -> Option<Self> {
        match (width, height) {
            (40, 40) => Some(Self::Small),
            (48, 48) => Some(Self::Medium),
            (56, 56) => Some(Self::Large),
            _ => None,
        }
    }

    /// Back-sprite width and height in tiles.
    pub const BACK_TILES: u32 = 4;
    /// Back-sprite width and height in pixels.
    pub const BACK_PIXELS: u32 = 32;
}

/// Categories in the canonical pokered `gfx/` namespace.
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
    /// Canonical subdirectory below `gfx/`.
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

    /// Whether assets in this category use the 1bpp conversion path.
    pub fn is_1bpp(self) -> bool {
        matches!(self, Self::Font)
    }
}

pub(crate) fn category_from_str(value: &str) -> Option<AssetCategory> {
    Some(match value {
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

macro_rules! impl_named_loaders {
    () => {
        /// Loads an overworld tileset.
        pub fn load_tileset(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::Tileset, name)
        }
        /// Loads an overworld sprite.
        pub fn load_sprite(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::Sprite, name)
        }
        /// Loads a Blue-version Pokémon front sprite.
        pub fn load_pokemon_front(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::PokemonFront, name)
        }
        /// Loads a Red/Green-version Pokémon front sprite.
        pub fn load_pokemon_front_rg(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::PokemonFrontRG, name)
        }
        /// Loads a Pokémon back sprite.
        pub fn load_pokemon_back(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::PokemonBack, name)
        }
        /// Loads font glyphs.
        pub fn load_font(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::Font, name)
        }
        /// Loads a trainer sprite.
        pub fn load_trainer(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::Trainer, name)
        }
        /// Loads battle graphics.
        pub fn load_battle(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::Battle, name)
        }
        /// Loads title-screen graphics.
        pub fn load_title(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::Title, name)
        }
        /// Loads intro graphics.
        pub fn load_intro(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::Intro, name)
        }
        /// Loads town-map graphics.
        pub fn load_town_map(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::TownMap, name)
        }
        /// Loads splash-screen graphics.
        pub fn load_splash(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::Splash, name)
        }
        /// Loads trade-animation graphics.
        pub fn load_trade(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::Trade, name)
        }
        /// Loads slot-machine graphics.
        pub fn load_slots(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::Slots, name)
        }
        /// Loads Pokédex graphics.
        pub fn load_pokedex(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::Pokedex, name)
        }
        /// Loads an overworld emote.
        pub fn load_emote(&mut self, name: &str) -> Result<&CachedTileSet> {
            self.load(AssetCategory::Emote, name)
        }
    };
}
pub(crate) use impl_named_loaders;

#[cfg(test)]
mod tests {
    use super::{category_from_str, AssetCategory};

    #[test]
    fn every_category_round_trips_through_its_canonical_subdirectory() {
        let categories = [
            AssetCategory::Tileset,
            AssetCategory::Sprite,
            AssetCategory::PokemonFront,
            AssetCategory::PokemonFrontRG,
            AssetCategory::PokemonBack,
            AssetCategory::Font,
            AssetCategory::Trainer,
            AssetCategory::Battle,
            AssetCategory::Title,
            AssetCategory::Intro,
            AssetCategory::TownMap,
            AssetCategory::Splash,
            AssetCategory::Emote,
            AssetCategory::Trade,
            AssetCategory::Player,
            AssetCategory::Credits,
            AssetCategory::Slots,
            AssetCategory::Pokedex,
            AssetCategory::Sgb,
            AssetCategory::Overworld,
            AssetCategory::Blockset,
            AssetCategory::Icon,
            AssetCategory::TrainerCard,
        ];

        for category in categories {
            assert_eq!(category_from_str(category.subdir()), Some(category));
        }
        assert_eq!(category_from_str("unknown"), None);
    }
}
