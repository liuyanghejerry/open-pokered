//! Sprite set data — which NPC sprites can appear on each outdoor map.
//!
//! From data/maps/sprite_sets.asm.

use crate::sprites::SpriteId;

pub const SPRITE_SET_LENGTH: usize = 11;
pub const NUM_SPRITE_SETS: usize = 10;
pub const NUM_SPLIT_SETS: usize = 12;
pub const NUM_OUTDOOR_MAPS: usize = 37;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SpriteSetId {
    PalletViridian = 0,
    PewterCerulean = 1,
    Lavender = 2,
    Vermilion = 3,
    Celadon = 4,
    Indigo = 5,
    Saffron = 6,
    SilenceBridge = 7,
    CyclingRoad = 8,
    Fuchsia = 9,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    NorthSouth,
    EastWest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitMapSpriteSet {
    pub direction: SplitDirection,
    pub coordinate: u8,
    pub set_north_or_west: SpriteSetId,
    pub set_south_or_east: SpriteSetId,
}

/// Which sprite set an outdoor map uses — either a direct set or a split set reference
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapSpriteSetRef {
    Direct(SpriteSetId),
    Split(u8), // index into SPLIT_MAP_SPRITE_SETS
}

use SplitDirection::*;
use SpriteId::*;
use SpriteSetId as S;

/// MapSpriteSets — one per outdoor map (0x00..0x24)
pub const MAP_SPRITE_SETS: [MapSpriteSetRef; NUM_OUTDOOR_MAPS] = {
    use MapSpriteSetRef::*;
    [
        Direct(S::PalletViridian), // PALLET_TOWN
        Direct(S::PalletViridian), // VIRIDIAN_CITY
        Direct(S::PewterCerulean), // PEWTER_CITY
        Direct(S::PewterCerulean), // CERULEAN_CITY
        Direct(S::Lavender),       // LAVENDER_TOWN
        Direct(S::Vermilion),      // VERMILION_CITY
        Direct(S::Celadon),        // CELADON_CITY
        Direct(S::Fuchsia),        // FUCHSIA_CITY
        Direct(S::PalletViridian), // CINNABAR_ISLAND
        Direct(S::Indigo),         // INDIGO_PLATEAU
        Direct(S::Saffron),        // SAFFRON_CITY
        Direct(S::PalletViridian), // UNUSED_MAP_0B
        Direct(S::PalletViridian), // ROUTE_1
        Split(0),                  // ROUTE_2
        Direct(S::PewterCerulean), // ROUTE_3
        Direct(S::PewterCerulean), // ROUTE_4
        Split(8),                  // ROUTE_5
        Split(9),                  // ROUTE_6
        Split(10),                 // ROUTE_7
        Split(11),                 // ROUTE_8
        Direct(S::PewterCerulean), // ROUTE_9
        Split(1),                  // ROUTE_10
        Split(2),                  // ROUTE_11
        Split(3),                  // ROUTE_12
        Direct(S::SilenceBridge),  // ROUTE_13
        Direct(S::SilenceBridge),  // ROUTE_14
        Split(4),                  // ROUTE_15
        Split(5),                  // ROUTE_16
        Direct(S::CyclingRoad),    // ROUTE_17
        Split(6),                  // ROUTE_18
        Direct(S::Fuchsia),        // ROUTE_19
        Split(7),                  // ROUTE_20
        Direct(S::PalletViridian), // ROUTE_21
        Direct(S::PalletViridian), // ROUTE_22
        Direct(S::Indigo),         // ROUTE_23
        Direct(S::PewterCerulean), // ROUTE_24
        Direct(S::PewterCerulean), // ROUTE_25
    ]
};

/// SplitMapSpriteSets — 12 split set configurations
pub const SPLIT_MAP_SPRITE_SETS: [SplitMapSpriteSet; NUM_SPLIT_SETS] = [
    SplitMapSpriteSet {
        direction: NorthSouth,
        coordinate: 37,
        set_north_or_west: S::PewterCerulean,
        set_south_or_east: S::PalletViridian,
    }, // ROUTE_2
    SplitMapSpriteSet {
        direction: NorthSouth,
        coordinate: 50,
        set_north_or_west: S::PewterCerulean,
        set_south_or_east: S::Lavender,
    }, // ROUTE_10
    SplitMapSpriteSet {
        direction: EastWest,
        coordinate: 57,
        set_north_or_west: S::Vermilion,
        set_south_or_east: S::SilenceBridge,
    }, // ROUTE_11
    SplitMapSpriteSet {
        direction: NorthSouth,
        coordinate: 21,
        set_north_or_west: S::Lavender,
        set_south_or_east: S::SilenceBridge,
    }, // ROUTE_12
    SplitMapSpriteSet {
        direction: EastWest,
        coordinate: 8,
        set_north_or_west: S::Fuchsia,
        set_south_or_east: S::SilenceBridge,
    }, // ROUTE_15
    SplitMapSpriteSet {
        direction: EastWest,
        coordinate: 24,
        set_north_or_west: S::CyclingRoad,
        set_south_or_east: S::Celadon,
    }, // ROUTE_16
    SplitMapSpriteSet {
        direction: EastWest,
        coordinate: 34,
        set_north_or_west: S::CyclingRoad,
        set_south_or_east: S::Fuchsia,
    }, // ROUTE_18
    SplitMapSpriteSet {
        direction: EastWest,
        coordinate: 53,
        set_north_or_west: S::PalletViridian,
        set_south_or_east: S::Fuchsia,
    }, // ROUTE_20
    SplitMapSpriteSet {
        direction: NorthSouth,
        coordinate: 33,
        set_north_or_west: S::PewterCerulean,
        set_south_or_east: S::Saffron,
    }, // ROUTE_5
    SplitMapSpriteSet {
        direction: NorthSouth,
        coordinate: 2,
        set_north_or_west: S::Saffron,
        set_south_or_east: S::Vermilion,
    }, // ROUTE_6
    SplitMapSpriteSet {
        direction: EastWest,
        coordinate: 17,
        set_north_or_west: S::Celadon,
        set_south_or_east: S::Saffron,
    }, // ROUTE_7
    SplitMapSpriteSet {
        direction: EastWest,
        coordinate: 3,
        set_north_or_west: S::Saffron,
        set_south_or_east: S::Lavender,
    }, // ROUTE_8
];

/// SpriteSets — 10 sets × 11 sprites each
pub const SPRITE_SETS: [[SpriteId; SPRITE_SET_LENGTH]; NUM_SPRITE_SETS] = [
    // SPRITESET_PALLET_VIRIDIAN
    [
        Blue,
        Youngster,
        Girl,
        Fisher,
        CooltrainerM,
        Gambler,
        Seel,
        Oak,
        Swimmer,
        PokeBall,
        GamblerAsleep,
    ],
    // SPRITESET_PEWTER_CERULEAN
    [
        Youngster,
        Rocket,
        SuperNerd,
        Hiker,
        Monster,
        Blue,
        Guard,
        CooltrainerF,
        CooltrainerM,
        PokeBall,
        UnusedGamblerAsleep2,
    ],
    // SPRITESET_LAVENDER
    [
        LittleGirl,
        Girl,
        SuperNerd,
        Hiker,
        Gambler,
        Monster,
        CooltrainerF,
        CooltrainerM,
        Guard,
        PokeBall,
        UnusedGamblerAsleep2,
    ],
    // SPRITESET_VERMILION
    [
        Beauty,
        SuperNerd,
        Youngster,
        Gambler,
        Monster,
        Guard,
        Sailor,
        CooltrainerF,
        CooltrainerM,
        PokeBall,
        UnusedGamblerAsleep2,
    ],
    // SPRITESET_CELADON
    [
        LittleGirl,
        LittleBoy,
        Girl,
        Fisher,
        MiddleAgedMan,
        Gramps,
        Monster,
        Guard,
        Rocket,
        PokeBall,
        Snorlax,
    ],
    // SPRITESET_INDIGO
    [
        Youngster,
        GymGuide,
        Monster,
        Blue,
        CooltrainerF,
        CooltrainerM,
        Swimmer,
        Guard,
        Gambler,
        PokeBall,
        UnusedGamblerAsleep2,
    ],
    // SPRITESET_SAFFRON
    [
        Rocket,
        Scientist,
        SilphWorkerM,
        SilphWorkerF,
        Gentleman,
        Bird,
        Rocker,
        CooltrainerM,
        Monster,
        PokeBall,
        UnusedGamblerAsleep2,
    ],
    // SPRITESET_SILENCE_BRIDGE
    [
        Biker,
        SuperNerd,
        MiddleAgedMan,
        CooltrainerF,
        CooltrainerM,
        Beauty,
        Fisher,
        Rocker,
        Monster,
        PokeBall,
        Snorlax,
    ],
    // SPRITESET_CYCLING_ROAD
    [
        Biker,
        CooltrainerM,
        SilphWorkerM,
        Fisher,
        Rocker,
        Hiker,
        Gambler,
        MiddleAgedMan,
        SuperNerd,
        PokeBall,
        Snorlax,
    ],
    // SPRITESET_FUCHSIA
    [
        Bird,
        CooltrainerM,
        Fairy,
        Fisher,
        Gambler,
        Monster,
        Seel,
        Swimmer,
        Youngster,
        PokeBall,
        Fossil,
    ],
];

impl SpriteSetId {
    pub fn sprites(self) -> &'static [SpriteId; SPRITE_SET_LENGTH] {
        &SPRITE_SETS[self as usize]
    }
}
