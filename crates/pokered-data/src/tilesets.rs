use dotzuki_engine::tileset::TilesetTrait;

/// Number of built-in tilesets (i.e. those that exist in the original game's ROM).
/// `TilesetId::Custom(slot)` uses indices `slot < CUSTOM_TILESETS.len()` on top of these.
pub const NUM_BUILTIN_TILESETS: usize = 24;

/// Backwards-compatible alias for `NUM_BUILTIN_TILESETS` — used for sizing the
/// existing `TILESET_HEADERS` and `TILESET_PASSABLE_TILES` static arrays which
/// are indexed by built-in variants only.
pub const NUM_TILESETS: usize = NUM_BUILTIN_TILESETS;

/// Tileset identifier.
///
/// The first 24 variants are the built-in tilesets baked into the original
/// ROM, with stable u8 IDs `0..=23`. `Custom(slot)` references a runtime-extra
/// tileset registered via `crates/pokered-data/tileset_extras.json` and built
/// into the binary by `build.rs`. Custom tilesets inherit their semantics
/// (palettes, counter/grass tiles, animations, door/warp/spinner behaviour)
/// from a built-in *base* tileset; only the block arrangement and (optionally)
/// the passable-tile list are unique per custom tileset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TilesetId {
    Overworld,
    RedsHouse1,
    Mart,
    Forest,
    RedsHouse2,
    Dojo,
    Pokecenter,
    Gym,
    House,
    ForestGate,
    Museum,
    Underground,
    Gate,
    Ship,
    ShipPort,
    Cemetery,
    Interior,
    Cavern,
    Lobby,
    Mansion,
    Lab,
    Club,
    Facility,
    Plateau,
    /// A user-registered tileset. `slot` indexes `custom::CUSTOM_TILESETS`.
    Custom(u8),
}

impl TilesetId {
    /// Returns the stable u8 ID matching the original ROM (`0..=23`) for
    /// built-in tilesets. For `Custom(slot)` returns the base tileset's u8
    /// so that anything written into a Game-Boy-shaped save slot stays valid.
    pub fn to_u8(self) -> u8 {
        match self {
            TilesetId::Overworld => 0,
            TilesetId::RedsHouse1 => 1,
            TilesetId::Mart => 2,
            TilesetId::Forest => 3,
            TilesetId::RedsHouse2 => 4,
            TilesetId::Dojo => 5,
            TilesetId::Pokecenter => 6,
            TilesetId::Gym => 7,
            TilesetId::House => 8,
            TilesetId::ForestGate => 9,
            TilesetId::Museum => 10,
            TilesetId::Underground => 11,
            TilesetId::Gate => 12,
            TilesetId::Ship => 13,
            TilesetId::ShipPort => 14,
            TilesetId::Cemetery => 15,
            TilesetId::Interior => 16,
            TilesetId::Cavern => 17,
            TilesetId::Lobby => 18,
            TilesetId::Mansion => 19,
            TilesetId::Lab => 20,
            TilesetId::Club => 21,
            TilesetId::Facility => 22,
            TilesetId::Plateau => 23,
            // Custom tilesets borrow the base's u8 identity for save-state.
            // `from_name` recovers the actual `Custom(slot)` from the JSON
            // header on reload, so this lossy round-trip is intentional.
            TilesetId::Custom(slot) => match custom::base_index_for(slot) {
                Some(idx) => idx as u8,
                None => 0,
            },
        }
    }

    /// Returns the index suitable for `TILESET_HEADERS[..]` /
    /// `TILESET_PASSABLE_TILES[..]`. For `Custom`, returns the base index so
    /// that callers using these legacy arrays still get sensible (base-derived)
    /// behaviour. Prefer the accessor functions in `tileset_data` /
    /// `collision`, which add custom-aware overrides on top.
    ///
    /// Note: a `Custom(slot)` constructed directly (rather than via
    /// `from_name`) with a slot that is not in the registry is treated like
    /// the base tileset, with `Overworld` as the safe fallback for an
    /// out-of-range slot — see `base()`.
    pub fn to_index(self) -> usize {
        self.to_u8() as usize
    }

    /// Returns true for `Custom(_)`.
    pub const fn is_custom(self) -> bool {
        matches!(self, TilesetId::Custom(_))
    }

    /// Returns the base built-in tileset for this id.
    /// For built-in variants, returns `self`. For `Custom(slot)`, returns the
    /// registered base. If the slot is out of range (or the registry is
    /// missing/malformed), falls back to `Overworld` so that callers never
    /// observe a non-built-in `base()` and the runtime stays robust against
    /// stale `tileset_extras.json` entries.
    pub fn base(self) -> TilesetId {
        match self {
            TilesetId::Custom(slot) => custom::base_for(slot).unwrap_or(TilesetId::Overworld),
            other => other,
        }
    }

    pub fn from_u8(value: u8) -> Option<TilesetId> {
        match value {
            0 => Some(TilesetId::Overworld),
            1 => Some(TilesetId::RedsHouse1),
            2 => Some(TilesetId::Mart),
            3 => Some(TilesetId::Forest),
            4 => Some(TilesetId::RedsHouse2),
            5 => Some(TilesetId::Dojo),
            6 => Some(TilesetId::Pokecenter),
            7 => Some(TilesetId::Gym),
            8 => Some(TilesetId::House),
            9 => Some(TilesetId::ForestGate),
            10 => Some(TilesetId::Museum),
            11 => Some(TilesetId::Underground),
            12 => Some(TilesetId::Gate),
            13 => Some(TilesetId::Ship),
            14 => Some(TilesetId::ShipPort),
            15 => Some(TilesetId::Cemetery),
            16 => Some(TilesetId::Interior),
            17 => Some(TilesetId::Cavern),
            18 => Some(TilesetId::Lobby),
            19 => Some(TilesetId::Mansion),
            20 => Some(TilesetId::Lab),
            21 => Some(TilesetId::Club),
            22 => Some(TilesetId::Facility),
            23 => Some(TilesetId::Plateau),
            _ => None,
        }
    }

    /// Convert from the Debug/variant name string (e.g. "Overworld" → TilesetId::Overworld).
    /// Falls through to the registered custom tilesets if the name is not built-in.
    pub fn from_name(name: &str) -> Option<TilesetId> {
        match name {
            "Overworld" => Some(TilesetId::Overworld),
            "RedsHouse1" => Some(TilesetId::RedsHouse1),
            "Mart" => Some(TilesetId::Mart),
            "Forest" => Some(TilesetId::Forest),
            "RedsHouse2" => Some(TilesetId::RedsHouse2),
            "Dojo" => Some(TilesetId::Dojo),
            "Pokecenter" => Some(TilesetId::Pokecenter),
            "Gym" => Some(TilesetId::Gym),
            "House" => Some(TilesetId::House),
            "ForestGate" => Some(TilesetId::ForestGate),
            "Museum" => Some(TilesetId::Museum),
            "Underground" => Some(TilesetId::Underground),
            "Gate" => Some(TilesetId::Gate),
            "Ship" => Some(TilesetId::Ship),
            "ShipPort" => Some(TilesetId::ShipPort),
            "Cemetery" => Some(TilesetId::Cemetery),
            "Interior" => Some(TilesetId::Interior),
            "Cavern" => Some(TilesetId::Cavern),
            "Lobby" => Some(TilesetId::Lobby),
            "Mansion" => Some(TilesetId::Mansion),
            "Lab" => Some(TilesetId::Lab),
            "Club" => Some(TilesetId::Club),
            "Facility" => Some(TilesetId::Facility),
            "Plateau" => Some(TilesetId::Plateau),
            other => custom::slot_for_name(other).map(TilesetId::Custom),
        }
    }

    /// Returns the variant name as a string (e.g. TilesetId::Overworld → "Overworld").
    /// For `Custom`, returns the registered PascalCase name, or the empty string
    /// if the slot is out of range.
    pub fn variant_name(self) -> &'static str {
        match self {
            TilesetId::Overworld => "Overworld",
            TilesetId::RedsHouse1 => "RedsHouse1",
            TilesetId::Mart => "Mart",
            TilesetId::Forest => "Forest",
            TilesetId::RedsHouse2 => "RedsHouse2",
            TilesetId::Dojo => "Dojo",
            TilesetId::Pokecenter => "Pokecenter",
            TilesetId::Gym => "Gym",
            TilesetId::House => "House",
            TilesetId::ForestGate => "ForestGate",
            TilesetId::Museum => "Museum",
            TilesetId::Underground => "Underground",
            TilesetId::Gate => "Gate",
            TilesetId::Ship => "Ship",
            TilesetId::ShipPort => "ShipPort",
            TilesetId::Cemetery => "Cemetery",
            TilesetId::Interior => "Interior",
            TilesetId::Cavern => "Cavern",
            TilesetId::Lobby => "Lobby",
            TilesetId::Mansion => "Mansion",
            TilesetId::Lab => "Lab",
            TilesetId::Club => "Club",
            TilesetId::Facility => "Facility",
            TilesetId::Plateau => "Plateau",
            TilesetId::Custom(slot) => custom::name_for(slot).unwrap_or(""),
        }
    }

    /// Returns the PNG filename (without extension) for this tileset, matching
    /// the files in `gfx/tilesets/`. For `Custom`, returns the registered
    /// snake_case basename, falling back to the base tileset's filename when
    /// the slot is unregistered.
    pub fn tileset_name(self) -> &'static str {
        match self {
            TilesetId::Overworld => "overworld",
            TilesetId::RedsHouse1 => "reds_house",
            TilesetId::Mart => "pokecenter", // shares pokecenter.bst
            TilesetId::Forest => "forest",
            TilesetId::RedsHouse2 => "reds_house", // shares reds_house.bst
            TilesetId::Dojo => "gym",              // shares gym.bst
            TilesetId::Pokecenter => "pokecenter",
            TilesetId::Gym => "gym",
            TilesetId::House => "house",
            TilesetId::ForestGate => "gate", // shares gate.bst
            TilesetId::Museum => "gate",     // shares gate.bst
            TilesetId::Underground => "underground",
            TilesetId::Gate => "gate",
            TilesetId::Ship => "ship",
            TilesetId::ShipPort => "ship_port",
            TilesetId::Cemetery => "cemetery",
            TilesetId::Interior => "interior",
            TilesetId::Cavern => "cavern",
            TilesetId::Lobby => "lobby",
            TilesetId::Mansion => "mansion",
            TilesetId::Lab => "lab",
            TilesetId::Club => "club",
            TilesetId::Facility => "facility",
            TilesetId::Plateau => "plateau",
            TilesetId::Custom(slot) => match custom::png_basename_for(slot) {
                Some(s) => s,
                None => "overworld",
            },
        }
    }
}

impl TilesetTrait for TilesetId {
    fn id(&self) -> u8 {
        self.to_u8()
    }

    fn name(&self) -> &'static str {
        self.tileset_name()
    }
}

/// Registry of user-defined tilesets, populated at compile time by `build.rs`
/// from `crates/pokered-data/tileset_extras.json`. When the file is absent the
/// generated `CUSTOM_TILESETS` slice is empty and every helper here returns
/// `None`, which the `TilesetId` accessors above translate into a safe
/// fallback to the base built-in behaviour.
pub mod custom {
    use super::TilesetId;

    include!(concat!(env!("OUT_DIR"), "/custom_tilesets_gen.rs"));

    /// Look up a custom slot by its PascalCase name.
    pub fn slot_for_name(name: &str) -> Option<u8> {
        CUSTOM_TILESETS
            .iter()
            .position(|t| t.name == name)
            .map(|i| i as u8)
    }

    /// Look up a custom tileset by slot.
    pub fn get(slot: u8) -> Option<&'static CustomTileset> {
        CUSTOM_TILESETS.get(slot as usize)
    }

    /// Resolve the base `TilesetId` for a custom slot.
    pub fn base_for(slot: u8) -> Option<TilesetId> {
        get(slot).and_then(|t| TilesetId::from_name(t.base))
    }

    /// Resolve the base built-in u8 (0..=23) for a custom slot.
    pub fn base_index_for(slot: u8) -> Option<u8> {
        base_for(slot).map(|b| match b {
            // `tileset_extras.json` only allows built-in names as `base`; if a
            // future schema change ever lets one custom rebase onto another,
            // we degrade gracefully to Overworld rather than panic.
            TilesetId::Custom(_) => TilesetId::Overworld.to_u8(),
            other => other.to_u8(),
        })
    }

    /// Registered PascalCase name for a custom slot.
    pub fn name_for(slot: u8) -> Option<&'static str> {
        get(slot).map(|t| t.name)
    }

    /// Registered snake_case PNG basename for a custom slot.
    pub fn png_basename_for(slot: u8) -> Option<&'static str> {
        get(slot).map(|t| t.png_basename)
    }

    /// Embedded `.bst` bytes for a custom slot.
    pub fn blockset_for(slot: u8) -> Option<&'static [u8]> {
        get(slot).map(|t| t.blockset)
    }

    /// Optional override for the passable-tile list.
    pub fn passable_override_for(slot: u8) -> Option<&'static [u8]> {
        get(slot).and_then(|t| t.passable_override)
    }

    /// Look up a custom slot by its snake_case PNG basename (e.g. "hoenn").
    pub fn slot_for_png_basename(basename: &str) -> Option<u8> {
        CUSTOM_TILESETS
            .iter()
            .position(|t| t.png_basename == basename)
            .map(|i| i as u8)
    }
}

/// Resolve a generic `TilesetTrait` implementation back to the concrete
/// `TilesetId`, recovering `Custom(slot)` identities that share their base
/// tileset's u8 id (`to_u8` is intentionally lossy for save-state).
///
/// `TilesetTrait::name()` is the snake_case PNG basename for both built-ins
/// and customs, so a custom is detected by basename lookup first; otherwise
/// the built-in u8 id applies.
pub fn resolve_concrete<T: dotzuki_engine::tileset::TilesetTrait>(tileset: &T) -> TilesetId {
    if let Some(slot) = custom::slot_for_png_basename(tileset.name()) {
        return TilesetId::Custom(slot);
    }
    TilesetId::from_u8(tileset.id()).unwrap_or(TilesetId::Overworld)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_round_trip_to_u8() {
        for v in 0..=23u8 {
            let id = TilesetId::from_u8(v).expect("built-in");
            assert!(!id.is_custom());
            assert_eq!(id.to_u8(), v);
            assert_eq!(id.to_index(), v as usize);
            // base() of a built-in is itself.
            assert_eq!(id.base(), id);
        }
        assert!(TilesetId::from_u8(24).is_none());
    }

    #[test]
    fn from_name_unknown_returns_none_for_unregistered() {
        // A name that is neither built-in nor a registered custom tileset.
        assert!(TilesetId::from_name("NotARealTilesetXyzzy").is_none());
    }

    #[test]
    fn name_round_trip_builtin() {
        let names = [
            "Overworld",
            "RedsHouse1",
            "Mart",
            "Forest",
            "Plateau",
            "Cavern",
        ];
        for n in names {
            let id = TilesetId::from_name(n).unwrap();
            assert_eq!(id.variant_name(), n);
        }
    }

    /// All registered custom tilesets must round-trip name → id → name and
    /// must inherit a built-in base. Iterates over whatever is present in
    /// `tileset_extras.json` (possibly zero entries — then this is a no-op).
    #[test]
    fn custom_tilesets_have_valid_base_and_round_trip_name() {
        for (slot, ct) in custom::CUSTOM_TILESETS.iter().enumerate() {
            let id = TilesetId::from_name(ct.name).unwrap_or_else(|| {
                panic!("registered custom tileset {:?} not resolvable by name", ct.name)
            });
            assert_eq!(id, TilesetId::Custom(slot as u8));
            assert_eq!(id.variant_name(), ct.name);
            assert!(id.is_custom());
            // Base must resolve to a built-in tileset.
            let base = id.base();
            assert!(!base.is_custom(), "base of {:?} resolved to non-builtin", ct.name);
            assert!(base.to_u8() < 24);
            // Saved-state byte must be the base's byte (lossy-but-stable).
            assert_eq!(id.to_u8(), base.to_u8());
            // PNG basename must be non-empty.
            assert!(!id.tileset_name().is_empty());
            // Embedded blockset must be a multiple of 16 (block size).
            assert!(!ct.blockset.is_empty());
            assert_eq!(ct.blockset.len() % 16, 0);
        }
    }

    /// The custom-tileset accessors should fall back to the base when the
    /// slot is out of range, so there are no panics for `Custom(255)` etc.
    #[test]
    fn unregistered_custom_slot_falls_back_to_overworld() {
        let id = TilesetId::Custom(u8::MAX);
        assert_eq!(id.base(), TilesetId::Overworld);
        assert_eq!(id.to_u8(), TilesetId::Overworld.to_u8());
        assert_eq!(id.tileset_name(), "overworld");
        assert_eq!(id.variant_name(), "");
    }
}
