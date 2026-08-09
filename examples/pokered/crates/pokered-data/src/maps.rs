//! Map ID definitions — 248 maps from constants/map_constants.asm
//!
//! Order: cities ($00-$0A), unused ($0B), routes ($0C-$24), indoor/dungeon ($25-$F7)
//! NUM_MAPS = $F8 (248)

use crate::map_constants::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MapId {
    // Cities ($00-$0A)
    PalletTown = 0x00,
    ViridianCity = 0x01,
    PewterCity = 0x02,
    CeruleanCity = 0x03,
    LavenderTown = 0x04,
    VermilionCity = 0x05,
    CeladonCity = 0x06,
    FuchsiaCity = 0x07,
    CinnabarIsland = 0x08,
    IndigoPlateau = 0x09,
    SaffronCity = 0x0A,

    // Unused
    UnusedMap0B = 0x0B,

    // Routes ($0C-$24)
    Route1 = 0x0C,
    Route2 = 0x0D,
    Route3 = 0x0E,
    Route4 = 0x0F,
    Route5 = 0x10,
    Route6 = 0x11,
    Route7 = 0x12,
    Route8 = 0x13,
    Route9 = 0x14,
    Route10 = 0x15,
    Route11 = 0x16,
    Route12 = 0x17,
    Route13 = 0x18,
    Route14 = 0x19,
    Route15 = 0x1A,
    Route16 = 0x1B,
    Route17 = 0x1C,
    Route18 = 0x1D,
    Route19 = 0x1E,
    Route20 = 0x1F,
    Route21 = 0x20,
    Route22 = 0x21,
    Route23 = 0x22,
    Route24 = 0x23,
    Route25 = 0x24,

    // Indoor maps — Pallet Town group
    RedsHouse1F = 0x25,
    RedsHouse2F = 0x26,
    BluesHouse = 0x27,
    OaksLab = 0x28,

    // Viridian City group
    ViridianPokecenter = 0x29,
    ViridianMart = 0x2A,
    ViridianSchoolHouse = 0x2B,
    ViridianNicknameHouse = 0x2C,
    ViridianGym = 0x2D,

    // Route 2 group
    DiglettsCaveRoute2 = 0x2E,
    ViridianForestNorthGate = 0x2F,
    Route2TradeHouse = 0x30,
    Route2Gate = 0x31,
    ViridianForestSouthGate = 0x32,

    // Viridian Forest group
    ViridianForest = 0x33,

    // Pewter City group
    Museum1F = 0x34,
    Museum2F = 0x35,
    PewterGym = 0x36,
    PewterNidoranHouse = 0x37,
    PewterMart = 0x38,
    PewterSpeechHouse = 0x39,
    PewterPokecenter = 0x3A,

    // Mt Moon group
    MtMoon1F = 0x3B,
    MtMoonB1F = 0x3C,
    MtMoonB2F = 0x3D,

    // Cerulean City group
    CeruleanTrashedHouse = 0x3E,
    CeruleanTradeHouse = 0x3F,
    CeruleanPokecenter = 0x40,
    CeruleanGym = 0x41,
    BikeShop = 0x42,
    CeruleanMart = 0x43,

    // Route 4 group
    MtMoonPokecenter = 0x44,

    // Cerulean City 2 group
    CeruleanTrashedHouseCopy = 0x45,

    // Route 5 group
    Route5Gate = 0x46,
    UndergroundPathRoute5 = 0x47,
    Daycare = 0x48,

    // Route 6 group
    Route6Gate = 0x49,
    UndergroundPathRoute6 = 0x4A,
    UndergroundPathRoute6Copy = 0x4B,

    // Route 7 group
    Route7Gate = 0x4C,
    UndergroundPathRoute7 = 0x4D,
    UndergroundPathRoute7Copy = 0x4E,

    // Route 8 group
    Route8Gate = 0x4F,
    UndergroundPathRoute8 = 0x50,

    // Rock Tunnel group
    RockTunnelPokecenter = 0x51,
    RockTunnel1F = 0x52,

    // Power Plant group
    PowerPlant = 0x53,

    // Route 11 group
    Route11Gate1F = 0x54,
    DiglettsCaveRoute11 = 0x55,
    Route11Gate2F = 0x56,

    // Route 12 group
    Route12Gate1F = 0x57,

    // Sea Cottage group
    BillsHouse = 0x58,

    // Vermilion City group
    VermilionPokecenter = 0x59,
    PokemonFanClub = 0x5A,
    VermilionMart = 0x5B,
    VermilionGym = 0x5C,
    VermilionPidgeyHouse = 0x5D,
    VermilionDock = 0x5E,

    // SS Anne group
    SSAnne1F = 0x5F,
    SSAnne2F = 0x60,
    SSAnne3F = 0x61,
    SSAnneB1F = 0x62,
    SSAnneBow = 0x63,
    SSAnneKitchen = 0x64,
    SSAnneCaptainsRoom = 0x65,
    SSAnne1FRooms = 0x66,
    SSAnne2FRooms = 0x67,
    SSAnneB1FRooms = 0x68,

    // Victory Road / unused group
    UnusedMap69 = 0x69,
    UnusedMap6A = 0x6A,
    UnusedMap6B = 0x6B,
    VictoryRoad1F = 0x6C,

    // Pokemon League group
    UnusedMap6D = 0x6D,
    UnusedMap6E = 0x6E,
    UnusedMap6F = 0x6F,
    UnusedMap70 = 0x70,
    LancesRoom = 0x71,
    UnusedMap72 = 0x72,
    UnusedMap73 = 0x73,
    UnusedMap74 = 0x74,
    UnusedMap75 = 0x75,
    HallOfFame = 0x76,

    // Underground Path group
    UndergroundPathNorthSouth = 0x77,

    // Pokemon League 2 group
    ChampionsRoom = 0x78,

    // Underground Path 2 group
    UndergroundPathWestEast = 0x79,

    // Celadon City group
    CeladonMart1F = 0x7A,
    CeladonMart2F = 0x7B,
    CeladonMart3F = 0x7C,
    CeladonMart4F = 0x7D,
    CeladonMartRoof = 0x7E,
    CeladonMartElevator = 0x7F,
    CeladonMansion1F = 0x80,
    CeladonMansion2F = 0x81,
    CeladonMansion3F = 0x82,
    CeladonMansionRoof = 0x83,
    CeladonMansionRoofHouse = 0x84,
    CeladonPokecenter = 0x85,
    CeladonGym = 0x86,
    GameCorner = 0x87,
    CeladonMart5F = 0x88,
    GameCornerPrizeRoom = 0x89,
    CeladonDiner = 0x8A,
    CeladonChiefHouse = 0x8B,
    CeladonHotel = 0x8C,

    // Lavender Town group
    LavenderPokecenter = 0x8D,

    // Pokemon Tower group
    PokemonTower1F = 0x8E,
    PokemonTower2F = 0x8F,
    PokemonTower3F = 0x90,
    PokemonTower4F = 0x91,
    PokemonTower5F = 0x92,
    PokemonTower6F = 0x93,
    PokemonTower7F = 0x94,

    // Lavender Town 2 group
    MrFujisHouse = 0x95,
    LavenderMart = 0x96,
    LavenderCuboneHouse = 0x97,

    // Fuchsia City group
    FuchsiaMart = 0x98,
    FuchsiaBillsGrandpasHouse = 0x99,
    FuchsiaPokecenter = 0x9A,
    WardensHouse = 0x9B,

    // Safari Zone group
    SafariZoneGate = 0x9C,

    // Fuchsia City 2 group
    FuchsiaGym = 0x9D,
    FuchsiaMeetingRoom = 0x9E,

    // Seafoam Islands group
    SeafoamIslandsB1F = 0x9F,
    SeafoamIslandsB2F = 0xA0,
    SeafoamIslandsB3F = 0xA1,
    SeafoamIslandsB4F = 0xA2,

    // Vermilion City 2 group
    VermilionOldRodHouse = 0xA3,

    // Fuchsia City 3 group
    FuchsiaGoodRodHouse = 0xA4,

    // Pokemon Mansion group
    PokemonMansion1F = 0xA5,

    // Cinnabar Island group
    CinnabarGym = 0xA6,
    CinnabarLab = 0xA7,
    CinnabarLabTradeRoom = 0xA8,
    CinnabarLabMetronomeRoom = 0xA9,
    CinnabarLabFossilRoom = 0xAA,
    CinnabarPokecenter = 0xAB,
    CinnabarMart = 0xAC,
    CinnabarMartCopy = 0xAD,

    // Indigo Plateau group
    IndigoPlateauLobby = 0xAE,

    // Saffron City group
    CopycatsHouse1F = 0xAF,
    CopycatsHouse2F = 0xB0,
    FightingDojo = 0xB1,
    SaffronGym = 0xB2,
    SaffronPidgeyHouse = 0xB3,
    SaffronMart = 0xB4,
    SilphCo1F = 0xB5,
    SaffronPokecenter = 0xB6,
    MrPsychicsHouse = 0xB7,

    // Route 15 group
    Route15Gate1F = 0xB8,
    Route15Gate2F = 0xB9,

    // Route 16 group
    Route16Gate1F = 0xBA,
    Route16Gate2F = 0xBB,
    Route16FlyHouse = 0xBC,

    // Route 12 2 group
    Route12SuperRodHouse = 0xBD,

    // Route 18 group
    Route18Gate1F = 0xBE,
    Route18Gate2F = 0xBF,

    // Seafoam Islands 2 group
    SeafoamIslands1F = 0xC0,

    // Route 22 group
    Route22Gate = 0xC1,

    // Victory Road 2 group
    VictoryRoad2F = 0xC2,

    // Route 12 3 group
    Route12Gate2F = 0xC3,

    // Vermilion City 3 group
    VermilionTradeHouse = 0xC4,

    // Digletts Cave group
    DiglettsCave = 0xC5,

    // Victory Road 3 group
    VictoryRoad3F = 0xC6,

    // Rocket HQ group
    RocketHideoutB1F = 0xC7,
    RocketHideoutB2F = 0xC8,
    RocketHideoutB3F = 0xC9,
    RocketHideoutB4F = 0xCA,
    RocketHideoutElevator = 0xCB,
    UnusedMapCC = 0xCC,
    UnusedMapCD = 0xCD,
    UnusedMapCE = 0xCE,

    // Silph Co group
    SilphCo2F = 0xCF,
    SilphCo3F = 0xD0,
    SilphCo4F = 0xD1,
    SilphCo5F = 0xD2,
    SilphCo6F = 0xD3,
    SilphCo7F = 0xD4,
    SilphCo8F = 0xD5,

    // Pokemon Mansion 2 group
    PokemonMansion2F = 0xD6,
    PokemonMansion3F = 0xD7,
    PokemonMansionB1F = 0xD8,

    // Safari Zone 2 group
    SafariZoneEast = 0xD9,
    SafariZoneNorth = 0xDA,
    SafariZoneWest = 0xDB,
    SafariZoneCenter = 0xDC,
    SafariZoneCenterRestHouse = 0xDD,
    SafariZoneSecretHouse = 0xDE,
    SafariZoneWestRestHouse = 0xDF,
    SafariZoneEastRestHouse = 0xE0,
    SafariZoneNorthRestHouse = 0xE1,

    // Cerulean Cave group
    CeruleanCave2F = 0xE2,
    CeruleanCaveB1F = 0xE3,
    CeruleanCave1F = 0xE4,

    // Lavender Town 3 group
    NameRatersHouse = 0xE5,

    // Cerulean City 3 group
    CeruleanBadgeHouse = 0xE6,

    // Rock Tunnel 2 group
    UnusedMapE7 = 0xE7,
    RockTunnelB1F = 0xE8,

    // Silph Co 2 group
    SilphCo9F = 0xE9,
    SilphCo10F = 0xEA,
    SilphCo11F = 0xEB,
    SilphCoElevator = 0xEC,

    // Misc unused + special
    UnusedMapED = 0xED,
    UnusedMapEE = 0xEE,
    TradeCenter = 0xEF,
    Colosseum = 0xF0,
    UnusedMapF1 = 0xF1,
    UnusedMapF2 = 0xF2,
    UnusedMapF3 = 0xF3,
    UnusedMapF4 = 0xF4,

    // Pokemon League 3 group
    LoreleisRoom = 0xF5,
    BrunosRoom = 0xF6,
    AgathasRoom = 0xF7,
}

/// Total number of maps
pub const NUM_MAPS: usize = 248; // 0x00..=0xF7

/// First route map ID value
pub const FIRST_ROUTE_MAP: u8 = 0x0C;

/// Special map ID: return to last outdoor map
pub const LAST_MAP: u8 = 0xFF;

impl MapId {
    /// Get the map dimensions (width, height) in blocks
    pub fn dimensions(self) -> (u8, u8) {
        MAP_DIMENSIONS[self as usize]
    }

    /// Get map width in blocks
    pub fn width(self) -> u8 {
        MAP_DIMENSIONS[self as usize].0
    }

    /// Get map height in blocks
    pub fn height(self) -> u8 {
        MAP_DIMENSIONS[self as usize].1
    }

    /// Try to create MapId from a u8 value
    pub fn from_u8(value: u8) -> Option<MapId> {
        if (value as usize) < NUM_MAPS {
            // SAFETY: all values 0x00..=0xF7 are valid enum variants
            Some(unsafe { core::mem::transmute(value) })
        } else {
            None
        }
    }

    /// Check if this is a city map
    pub fn is_city(self) -> bool {
        (self as u8) < NUM_CITY_MAPS
    }

    /// Check if this is a route map
    pub fn is_route(self) -> bool {
        let v = self as u8;
        v >= FIRST_ROUTE_MAP && v <= 0x24
    }

    /// Check if this is an indoor map
    pub fn is_indoor(self) -> bool {
        (self as u8) >= FIRST_INDOOR_MAP
    }
}

/// Map dimensions (width, height) in blocks, indexed by MapId
/// From constants/map_constants.asm — map_const MACRO
pub const MAP_DIMENSIONS: [(u8, u8); NUM_MAPS] = [
    // Cities $00-$0A
    (10, 9),  // PalletTown
    (20, 18), // ViridianCity
    (20, 18), // PewterCity
    (20, 18), // CeruleanCity
    (10, 9),  // LavenderTown
    (20, 18), // VermilionCity
    (25, 18), // CeladonCity
    (20, 18), // FuchsiaCity
    (10, 9),  // CinnabarIsland
    (10, 9),  // IndigoPlateau
    (20, 18), // SaffronCity
    // Unused $0B
    (0, 0), // UnusedMap0B
    // Routes $0C-$24
    (10, 18), // Route1
    (10, 36), // Route2
    (35, 9),  // Route3
    (45, 9),  // Route4
    (10, 18), // Route5
    (10, 18), // Route6
    (10, 9),  // Route7
    (30, 9),  // Route8
    (30, 9),  // Route9
    (10, 36), // Route10
    (30, 9),  // Route11
    (10, 54), // Route12
    (30, 9),  // Route13
    (10, 27), // Route14
    (30, 9),  // Route15
    (20, 9),  // Route16
    (10, 72), // Route17
    (25, 9),  // Route18
    (10, 27), // Route19
    (50, 9),  // Route20
    (10, 45), // Route21
    (20, 9),  // Route22
    (10, 72), // Route23
    (10, 18), // Route24
    (30, 9),  // Route25
    // Indoor $25-$43 (Pallet, Viridian, Route2, Forest, Pewter, MtMoon, Cerulean)
    (4, 4),   // RedsHouse1F
    (4, 4),   // RedsHouse2F
    (4, 4),   // BluesHouse
    (5, 6),   // OaksLab
    (7, 4),   // ViridianPokecenter
    (4, 4),   // ViridianMart
    (4, 4),   // ViridianSchoolHouse
    (4, 4),   // ViridianNicknameHouse
    (10, 9),  // ViridianGym
    (4, 4),   // DiglettsCaveRoute2
    (5, 4),   // ViridianForestNorthGate
    (4, 4),   // Route2TradeHouse
    (5, 4),   // Route2Gate
    (5, 4),   // ViridianForestSouthGate
    (17, 24), // ViridianForest
    (10, 4),  // Museum1F
    (7, 4),   // Museum2F
    (5, 7),   // PewterGym
    (4, 4),   // PewterNidoranHouse
    (4, 4),   // PewterMart
    (4, 4),   // PewterSpeechHouse
    (7, 4),   // PewterPokecenter
    (20, 18), // MtMoon1F
    (14, 14), // MtMoonB1F
    (20, 18), // MtMoonB2F
    (4, 4),   // CeruleanTrashedHouse
    (4, 4),   // CeruleanTradeHouse
    (7, 4),   // CeruleanPokecenter
    (5, 7),   // CeruleanGym
    (4, 4),   // BikeShop
    (4, 4),   // CeruleanMart
    // $44-$68
    (7, 4),   // MtMoonPokecenter
    (4, 4),   // CeruleanTrashedHouseCopy
    (4, 3),   // Route5Gate
    (4, 4),   // UndergroundPathRoute5
    (4, 4),   // Daycare
    (4, 3),   // Route6Gate
    (4, 4),   // UndergroundPathRoute6
    (4, 4),   // UndergroundPathRoute6Copy
    (3, 4),   // Route7Gate
    (4, 4),   // UndergroundPathRoute7
    (4, 4),   // UndergroundPathRoute7Copy
    (3, 4),   // Route8Gate
    (4, 4),   // UndergroundPathRoute8
    (7, 4),   // RockTunnelPokecenter
    (20, 18), // RockTunnel1F
    (20, 18), // PowerPlant
    (4, 5),   // Route11Gate1F
    (4, 4),   // DiglettsCaveRoute11
    (4, 4),   // Route11Gate2F
    (5, 4),   // Route12Gate1F
    (4, 4),   // BillsHouse
    (7, 4),   // VermilionPokecenter
    (4, 4),   // PokemonFanClub
    (4, 4),   // VermilionMart
    (5, 9),   // VermilionGym
    (4, 4),   // VermilionPidgeyHouse
    (14, 6),  // VermilionDock
    (20, 9),  // SSAnne1F
    (20, 9),  // SSAnne2F
    (10, 3),  // SSAnne3F
    (15, 4),  // SSAnneB1F
    (10, 7),  // SSAnneBow
    (7, 8),   // SSAnneKitchen
    (3, 4),   // SSAnneCaptainsRoom
    (12, 8),  // SSAnne1FRooms
    (12, 8),  // SSAnne2FRooms
    (12, 8),  // SSAnneB1FRooms
    // $69-$6C (Victory Road / unused group)
    (0, 0),  // UnusedMap69
    (0, 0),  // UnusedMap6A
    (0, 0),  // UnusedMap6B
    (10, 9), // VictoryRoad1F
    // $6D-$76 (Pokemon League group)
    (0, 0),   // UnusedMap6D
    (0, 0),   // UnusedMap6E
    (0, 0),   // UnusedMap6F
    (0, 0),   // UnusedMap70
    (13, 13), // LancesRoom
    (0, 0),   // UnusedMap72
    (0, 0),   // UnusedMap73
    (0, 0),   // UnusedMap74
    (0, 0),   // UnusedMap75
    (5, 4),   // HallOfFame
    // $77 (Underground Path group)
    (4, 24), // UndergroundPathNorthSouth
    // $78 (Pokemon League 2 group)
    (4, 4), // ChampionsRoom
    // $79 (Underground Path 2 group)
    (25, 4), // UndergroundPathWestEast
    // $7A-$8C (Celadon City group)
    (10, 4), // CeladonMart1F
    (10, 4), // CeladonMart2F
    (10, 4), // CeladonMart3F
    (10, 4), // CeladonMart4F
    (10, 4), // CeladonMartRoof
    (2, 2),  // CeladonMartElevator
    (4, 6),  // CeladonMansion1F
    (4, 6),  // CeladonMansion2F
    (4, 6),  // CeladonMansion3F
    (4, 6),  // CeladonMansionRoof
    (4, 4),  // CeladonMansionRoofHouse
    (7, 4),  // CeladonPokecenter
    (5, 9),  // CeladonGym
    (10, 9), // GameCorner
    (10, 4), // CeladonMart5F
    (5, 4),  // GameCornerPrizeRoom
    (5, 4),  // CeladonDiner
    (4, 4),  // CeladonChiefHouse
    (7, 4),  // CeladonHotel
    // $8D (Lavender Town group)
    (7, 4), // LavenderPokecenter
    // $8E-$94 (Pokemon Tower group)
    (10, 9), // PokemonTower1F
    (10, 9), // PokemonTower2F
    (10, 9), // PokemonTower3F
    (10, 9), // PokemonTower4F
    (10, 9), // PokemonTower5F
    (10, 9), // PokemonTower6F
    (10, 9), // PokemonTower7F
    // $95-$97 (Lavender Town 2 group)
    (4, 4), // MrFujisHouse
    (4, 4), // LavenderMart
    (4, 4), // LavenderCuboneHouse
    // $98-$9B (Fuchsia City group)
    (4, 4), // FuchsiaMart
    (4, 4), // FuchsiaBillsGrandpasHouse
    (7, 4), // FuchsiaPokecenter
    (5, 4), // WardensHouse
    // $9C (Safari Zone group)
    (4, 3), // SafariZoneGate
    // $9D-$9E (Fuchsia City 2 group)
    (5, 9), // FuchsiaGym
    (7, 4), // FuchsiaMeetingRoom
    // $9F-$A2 (Seafoam Islands group)
    (15, 9), // SeafoamIslandsB1F
    (15, 9), // SeafoamIslandsB2F
    (15, 9), // SeafoamIslandsB3F
    (15, 9), // SeafoamIslandsB4F
    // $A3 (Vermilion City 2 group)
    (4, 4), // VermilionOldRodHouse
    // $A4 (Fuchsia City 3 group)
    (4, 4), // FuchsiaGoodRodHouse
    // $A5 (Pokemon Mansion group)
    (15, 14), // PokemonMansion1F
    // $A6-$AD (Cinnabar Island group)
    (10, 9), // CinnabarGym
    (9, 4),  // CinnabarLab
    (4, 4),  // CinnabarLabTradeRoom
    (4, 4),  // CinnabarLabMetronomeRoom
    (4, 4),  // CinnabarLabFossilRoom
    (7, 4),  // CinnabarPokecenter
    (4, 4),  // CinnabarMart
    (4, 4),  // CinnabarMartCopy
    // $AE (Indigo Plateau group)
    (8, 6), // IndigoPlateauLobby
    // $AF-$B7 (Saffron City group)
    (4, 4),  // CopycatsHouse1F
    (4, 4),  // CopycatsHouse2F
    (5, 6),  // FightingDojo
    (10, 9), // SaffronGym
    (4, 4),  // SaffronPidgeyHouse
    (4, 4),  // SaffronMart
    (15, 9), // SilphCo1F
    (7, 4),  // SaffronPokecenter
    (4, 4),  // MrPsychicsHouse
    // $B8-$B9 (Route 15 group)
    (4, 5), // Route15Gate1F
    (4, 4), // Route15Gate2F
    // $BA-$BC (Route 16 group)
    (4, 7), // Route16Gate1F
    (4, 4), // Route16Gate2F
    (4, 4), // Route16FlyHouse
    // $BD (Route 12 2 group)
    (4, 4), // Route12SuperRodHouse
    // $BE-$BF (Route 18 group)
    (4, 5), // Route18Gate1F
    (4, 4), // Route18Gate2F
    // $C0 (Seafoam Islands 2 group)
    (15, 9), // SeafoamIslands1F
    // $C1 (Route 22 group)
    (5, 4), // Route22Gate
    // $C2 (Victory Road 2 group)
    (15, 9), // VictoryRoad2F
    // $C3 (Route 12 3 group)
    (4, 4), // Route12Gate2F
    // $C4 (Vermilion City 3 group)
    (4, 4), // VermilionTradeHouse
    // $C5 (Digletts Cave group)
    (20, 18), // DiglettsCave
    // $C6 (Victory Road 3 group)
    (15, 9), // VictoryRoad3F
    // $C7-$CE (Rocket HQ group)
    (15, 14), // RocketHideoutB1F
    (15, 14), // RocketHideoutB2F
    (15, 14), // RocketHideoutB3F
    (15, 12), // RocketHideoutB4F
    (3, 4),   // RocketHideoutElevator
    (0, 0),   // UnusedMapCC
    (0, 0),   // UnusedMapCD
    (0, 0),   // UnusedMapCE
    // $CF-$D5 (Silph Co group)
    (15, 9), // SilphCo2F
    (15, 9), // SilphCo3F
    (15, 9), // SilphCo4F
    (15, 9), // SilphCo5F
    (13, 9), // SilphCo6F
    (13, 9), // SilphCo7F
    (13, 9), // SilphCo8F
    // $D6-$D8 (Pokemon Mansion 2 group)
    (15, 14), // PokemonMansion2F
    (15, 9),  // PokemonMansion3F
    (15, 14), // PokemonMansionB1F
    // $D9-$E1 (Safari Zone 2 group)
    (15, 13), // SafariZoneEast
    (20, 18), // SafariZoneNorth
    (15, 13), // SafariZoneWest
    (15, 13), // SafariZoneCenter
    (4, 4),   // SafariZoneCenterRestHouse
    (4, 4),   // SafariZoneSecretHouse
    (4, 4),   // SafariZoneWestRestHouse
    (4, 4),   // SafariZoneEastRestHouse
    (4, 4),   // SafariZoneNorthRestHouse
    // $E2-$E4 (Cerulean Cave group)
    (15, 9), // CeruleanCave2F
    (15, 9), // CeruleanCaveB1F
    (15, 9), // CeruleanCave1F
    // $E5 (Lavender Town 3 group)
    (4, 4), // NameRatersHouse
    // $E6 (Cerulean City 3 group)
    (4, 4), // CeruleanBadgeHouse
    // $E7-$E8 (Rock Tunnel 2 group)
    (0, 0),   // UnusedMapE7
    (20, 18), // RockTunnelB1F
    // $E9-$EC (Silph Co 2 group)
    (13, 9), // SilphCo9F
    (8, 9),  // SilphCo10F
    (9, 9),  // SilphCo11F
    (2, 2),  // SilphCoElevator
    // $ED-$F4 (Misc unused + special)
    (0, 0), // UnusedMapED
    (0, 0), // UnusedMapEE
    (5, 4), // TradeCenter
    (5, 4), // Colosseum
    (0, 0), // UnusedMapF1
    (0, 0), // UnusedMapF2
    (0, 0), // UnusedMapF3
    (0, 0), // UnusedMapF4
    // $F5-$F7 (Pokemon League 3 group)
    (5, 6), // LoreleisRoom
    (5, 6), // BrunosRoom
    (5, 6), // AgathasRoom
];
