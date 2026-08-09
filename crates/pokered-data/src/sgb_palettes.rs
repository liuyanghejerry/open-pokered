//! SGB/CGB palette data extracted from the original game.
//!
//! This module contains all Super Game Boy palette definitions including:
//! - The 37 SuperPalettes (town/cave/monster/UI palettes) for both Red and Blue
//! - Monster palette assignments (which Pokemon species map to which SuperPalette)
//! - Map/tileset constants for overworld palette selection
//! - Palette helper functions
//!
//! The base types (`SgbColor`, `SgbPaletteEntry`, `SgbPaletteId`, `SetPalCommand`)
//! are defined in `dotzuki-engine` and re-used here.
//!
//! Transcribed from `data/sgb/sgb_palettes.asm` and `data/pokemon/palettes.asm`.

pub use dotzuki_engine::palette::{
    SgbColor, SgbPaletteEntry, SgbPaletteId, SetPalCommand, NUM_SGB_PALS,
    SET_PAL_PARTY_MENU_HP_BARS, SET_PAL_DEFAULT,
};

/// Helper to create an SGB palette entry from 12 values (r,g,b × 4 colors).
const fn sgb_pal(
    r0: u8,
    g0: u8,
    b0: u8,
    r1: u8,
    g1: u8,
    b1: u8,
    r2: u8,
    g2: u8,
    b2: u8,
    r3: u8,
    g3: u8,
    b3: u8,
) -> SgbPaletteEntry {
    [
        SgbColor::new(r0, g0, b0),
        SgbColor::new(r1, g1, b1),
        SgbColor::new(r2, g2, b2),
        SgbColor::new(r3, g3, b3),
    ]
}

// ============================================================================
// SuperPalettes
// ============================================================================

/// SuperPalettes for Pokémon Red version.
/// 37 entries, each with 4 SGB colors (5-bit per channel).
/// Transcribed from data/sgb/sgb_palettes.asm with DEF(_RED).
pub const SUPER_PALETTES_RED: [SgbPaletteEntry; NUM_SGB_PALS] = [
    sgb_pal(31, 29, 31, 21, 28, 11, 20, 26, 31, 03, 02, 02), // PAL_ROUTE     0x00
    sgb_pal(31, 29, 31, 25, 28, 27, 20, 26, 31, 03, 02, 02), // PAL_PALLET    0x01
    sgb_pal(31, 29, 31, 17, 26, 03, 20, 26, 31, 03, 02, 02), // PAL_VIRIDIAN  0x02
    sgb_pal(31, 29, 31, 23, 25, 16, 20, 26, 31, 03, 02, 02), // PAL_PEWTER    0x03
    sgb_pal(31, 29, 31, 17, 20, 30, 20, 26, 31, 03, 02, 02), // PAL_CERULEAN  0x04
    sgb_pal(31, 29, 31, 27, 20, 27, 20, 26, 31, 03, 02, 02), // PAL_LAVENDER  0x05
    sgb_pal(31, 29, 31, 30, 18, 00, 20, 26, 31, 03, 02, 02), // PAL_VERMILION 0x06
    sgb_pal(31, 29, 31, 16, 30, 22, 20, 26, 31, 03, 02, 02), // PAL_CELADON   0x07
    sgb_pal(31, 29, 31, 31, 15, 22, 20, 26, 31, 03, 02, 02), // PAL_FUCHSIA   0x08
    sgb_pal(31, 29, 31, 26, 10, 06, 20, 26, 31, 03, 02, 02), // PAL_CINNABAR  0x09
    sgb_pal(31, 29, 31, 22, 14, 24, 20, 26, 31, 03, 02, 02), // PAL_INDIGO    0x0A
    sgb_pal(31, 29, 31, 27, 27, 03, 20, 26, 31, 03, 02, 02), // PAL_SAFFRON   0x0B
    sgb_pal(31, 29, 31, 20, 26, 31, 17, 23, 10, 03, 02, 02), // PAL_TOWNMAP   0x0C
    sgb_pal(31, 29, 31, 30, 30, 17, 17, 23, 10, 21, 00, 04), // PAL_LOGO1     0x0D (RED)
    sgb_pal(31, 29, 31, 30, 30, 17, 18, 18, 24, 07, 07, 16), // PAL_LOGO2     0x0E
    sgb_pal(31, 29, 31, 24, 20, 30, 11, 20, 30, 03, 02, 02), // PAL_0F        0x0F
    sgb_pal(31, 29, 31, 30, 22, 17, 16, 14, 19, 03, 02, 02), // PAL_MEWMON    0x10
    sgb_pal(31, 29, 31, 18, 20, 27, 11, 15, 23, 03, 02, 02), // PAL_BLUEMON   0x11
    sgb_pal(31, 29, 31, 31, 20, 10, 26, 10, 06, 03, 02, 02), // PAL_REDMON    0x12
    sgb_pal(31, 29, 31, 21, 25, 29, 14, 19, 25, 03, 02, 02), // PAL_CYANMON   0x13
    sgb_pal(31, 29, 31, 27, 22, 24, 21, 15, 23, 03, 02, 02), // PAL_PURPLEMON 0x14
    sgb_pal(31, 29, 31, 28, 20, 15, 21, 14, 09, 03, 02, 02), // PAL_BROWNMON  0x15
    sgb_pal(31, 29, 31, 20, 26, 16, 09, 20, 11, 03, 02, 02), // PAL_GREENMON  0x16
    sgb_pal(31, 29, 31, 30, 22, 24, 28, 15, 21, 03, 02, 02), // PAL_PINKMON   0x17
    sgb_pal(31, 29, 31, 31, 28, 14, 26, 20, 00, 03, 02, 02), // PAL_YELLOWMON 0x18
    sgb_pal(31, 29, 31, 26, 21, 22, 15, 15, 18, 03, 02, 02), // PAL_GRAYMON   0x19
    sgb_pal(31, 29, 31, 26, 21, 22, 27, 20, 06, 03, 02, 02), // PAL_SLOTS1    0x1A
    sgb_pal(31, 29, 31, 31, 31, 17, 25, 17, 21, 03, 02, 02), // PAL_SLOTS2    0x1B (RED)
    sgb_pal(31, 29, 31, 22, 31, 16, 25, 17, 21, 03, 02, 02), // PAL_SLOTS3    0x1C (RED)
    sgb_pal(31, 29, 31, 16, 19, 29, 25, 17, 21, 03, 02, 02), // PAL_SLOTS4    0x1D (RED)
    sgb_pal(31, 29, 31, 07, 07, 07, 02, 03, 03, 03, 02, 02), // PAL_BLACK     0x1E
    sgb_pal(31, 29, 31, 30, 26, 15, 09, 20, 11, 03, 02, 02), // PAL_GREENBAR  0x1F
    sgb_pal(31, 29, 31, 30, 26, 15, 26, 20, 00, 03, 02, 02), // PAL_YELLOWBAR 0x20
    sgb_pal(31, 29, 31, 30, 26, 15, 26, 10, 06, 03, 02, 02), // PAL_REDBAR    0x21
    sgb_pal(31, 29, 31, 30, 22, 17, 11, 15, 23, 03, 02, 02), // PAL_BADGE     0x22
    sgb_pal(31, 29, 31, 21, 14, 09, 18, 24, 22, 03, 02, 02), // PAL_CAVE      0x23
    sgb_pal(31, 29, 31, 31, 28, 14, 24, 20, 10, 03, 02, 02), // PAL_GAMEFREAK 0x24
];

/// SuperPalettes for Pokémon Blue version.
/// Only PAL_LOGO1 (0x0D), PAL_SLOTS2-4 (0x1B-0x1D) differ from Red.
pub const SUPER_PALETTES_BLUE: [SgbPaletteEntry; NUM_SGB_PALS] = [
    sgb_pal(31, 29, 31, 21, 28, 11, 20, 26, 31, 03, 02, 02), // PAL_ROUTE     0x00
    sgb_pal(31, 29, 31, 25, 28, 27, 20, 26, 31, 03, 02, 02), // PAL_PALLET    0x01
    sgb_pal(31, 29, 31, 17, 26, 03, 20, 26, 31, 03, 02, 02), // PAL_VIRIDIAN  0x02
    sgb_pal(31, 29, 31, 23, 25, 16, 20, 26, 31, 03, 02, 02), // PAL_PEWTER    0x03
    sgb_pal(31, 29, 31, 17, 20, 30, 20, 26, 31, 03, 02, 02), // PAL_CERULEAN  0x04
    sgb_pal(31, 29, 31, 27, 20, 27, 20, 26, 31, 03, 02, 02), // PAL_LAVENDER  0x05
    sgb_pal(31, 29, 31, 30, 18, 00, 20, 26, 31, 03, 02, 02), // PAL_VERMILION 0x06
    sgb_pal(31, 29, 31, 16, 30, 22, 20, 26, 31, 03, 02, 02), // PAL_CELADON   0x07
    sgb_pal(31, 29, 31, 31, 15, 22, 20, 26, 31, 03, 02, 02), // PAL_FUCHSIA   0x08
    sgb_pal(31, 29, 31, 26, 10, 06, 20, 26, 31, 03, 02, 02), // PAL_CINNABAR  0x09
    sgb_pal(31, 29, 31, 22, 14, 24, 20, 26, 31, 03, 02, 02), // PAL_INDIGO    0x0A
    sgb_pal(31, 29, 31, 27, 27, 03, 20, 26, 31, 03, 02, 02), // PAL_SAFFRON   0x0B
    sgb_pal(31, 29, 31, 20, 26, 31, 17, 23, 10, 03, 02, 02), // PAL_TOWNMAP   0x0C
    sgb_pal(31, 29, 31, 30, 30, 17, 21, 00, 04, 14, 19, 29), // PAL_LOGO1     0x0D (BLUE)
    sgb_pal(31, 29, 31, 30, 30, 17, 18, 18, 24, 07, 07, 16), // PAL_LOGO2     0x0E
    sgb_pal(31, 29, 31, 24, 20, 30, 11, 20, 30, 03, 02, 02), // PAL_0F        0x0F
    sgb_pal(31, 29, 31, 30, 22, 17, 16, 14, 19, 03, 02, 02), // PAL_MEWMON    0x10
    sgb_pal(31, 29, 31, 18, 20, 27, 11, 15, 23, 03, 02, 02), // PAL_BLUEMON   0x11
    sgb_pal(31, 29, 31, 31, 20, 10, 26, 10, 06, 03, 02, 02), // PAL_REDMON    0x12
    sgb_pal(31, 29, 31, 21, 25, 29, 14, 19, 25, 03, 02, 02), // PAL_CYANMON   0x13
    sgb_pal(31, 29, 31, 27, 22, 24, 21, 15, 23, 03, 02, 02), // PAL_PURPLEMON 0x14
    sgb_pal(31, 29, 31, 28, 20, 15, 21, 14, 09, 03, 02, 02), // PAL_BROWNMON  0x15
    sgb_pal(31, 29, 31, 20, 26, 16, 09, 20, 11, 03, 02, 02), // PAL_GREENMON  0x16
    sgb_pal(31, 29, 31, 30, 22, 24, 28, 15, 21, 03, 02, 02), // PAL_PINKMON   0x17
    sgb_pal(31, 29, 31, 31, 28, 14, 26, 20, 00, 03, 02, 02), // PAL_YELLOWMON 0x18
    sgb_pal(31, 29, 31, 26, 21, 22, 15, 15, 18, 03, 02, 02), // PAL_GRAYMON   0x19
    sgb_pal(31, 29, 31, 26, 21, 22, 27, 20, 06, 03, 02, 02), // PAL_SLOTS1    0x1A
    sgb_pal(31, 29, 31, 31, 31, 17, 16, 19, 29, 03, 02, 02), // PAL_SLOTS2    0x1B (BLUE)
    sgb_pal(31, 29, 31, 22, 31, 16, 16, 19, 29, 03, 02, 02), // PAL_SLOTS3    0x1C (BLUE)
    sgb_pal(31, 29, 31, 25, 17, 21, 16, 19, 29, 03, 02, 02), // PAL_SLOTS4    0x1D (BLUE)
    sgb_pal(31, 29, 31, 07, 07, 07, 02, 03, 03, 03, 02, 02), // PAL_BLACK     0x1E
    sgb_pal(31, 29, 31, 30, 26, 15, 09, 20, 11, 03, 02, 02), // PAL_GREENBAR  0x1F
    sgb_pal(31, 29, 31, 30, 26, 15, 26, 20, 00, 03, 02, 02), // PAL_YELLOWBAR 0x20
    sgb_pal(31, 29, 31, 30, 26, 15, 26, 10, 06, 03, 02, 02), // PAL_REDBAR    0x21
    sgb_pal(31, 29, 31, 30, 22, 17, 11, 15, 23, 03, 02, 02), // PAL_BADGE     0x22
    sgb_pal(31, 29, 31, 21, 14, 09, 18, 24, 22, 03, 02, 02), // PAL_CAVE      0x23
    sgb_pal(31, 29, 31, 31, 28, 14, 24, 20, 10, 03, 02, 02), // PAL_GAMEFREAK 0x24
];

/// Get the SuperPalettes table for a given game version.
pub fn super_palettes(is_red: bool) -> &'static [SgbPaletteEntry; NUM_SGB_PALS] {
    if is_red {
        &SUPER_PALETTES_RED
    } else {
        &SUPER_PALETTES_BLUE
    }
}

/// Look up an SGB palette entry by ID and version.
pub fn lookup_sgb_palette(id: SgbPaletteId, is_red: bool) -> &'static SgbPaletteEntry {
    &super_palettes(is_red)[id as usize]
}

// ============================================================================
// Monster Palette Assignments
// ============================================================================

/// Number of Pokémon species (including MISSINGNO at index 0).
pub const NUM_POKEMON_PLUS_ONE: usize = 151 + 1; // indices 0..=151

/// Monster palette assignments (internal index order, NOT Pokédex order).
/// Index 0 = MISSINGNO, 1 = BULBASAUR, ..., 151 = MEW.
/// Transcribed from data/pokemon/palettes.asm.
pub const MONSTER_PALETTES: [SgbPaletteId; NUM_POKEMON_PLUS_ONE] = [
    SgbPaletteId::MewMon,    // 0: MISSINGNO
    SgbPaletteId::GreenMon,  // 1: BULBASAUR
    SgbPaletteId::GreenMon,  // 2: IVYSAUR
    SgbPaletteId::GreenMon,  // 3: VENUSAUR
    SgbPaletteId::RedMon,    // 4: CHARMANDER
    SgbPaletteId::RedMon,    // 5: CHARMELEON
    SgbPaletteId::RedMon,    // 6: CHARIZARD
    SgbPaletteId::CyanMon,   // 7: SQUIRTLE
    SgbPaletteId::CyanMon,   // 8: WARTORTLE
    SgbPaletteId::CyanMon,   // 9: BLASTOISE
    SgbPaletteId::GreenMon,  // 10: CATERPIE
    SgbPaletteId::GreenMon,  // 11: METAPOD
    SgbPaletteId::CyanMon,   // 12: BUTTERFREE
    SgbPaletteId::YellowMon, // 13: WEEDLE
    SgbPaletteId::YellowMon, // 14: KAKUNA
    SgbPaletteId::YellowMon, // 15: BEEDRILL
    SgbPaletteId::BrownMon,  // 16: PIDGEY
    SgbPaletteId::BrownMon,  // 17: PIDGEOTTO
    SgbPaletteId::BrownMon,  // 18: PIDGEOT
    SgbPaletteId::GrayMon,   // 19: RATTATA
    SgbPaletteId::GrayMon,   // 20: RATICATE
    SgbPaletteId::BrownMon,  // 21: SPEAROW
    SgbPaletteId::BrownMon,  // 22: FEAROW
    SgbPaletteId::PurpleMon, // 23: EKANS
    SgbPaletteId::PurpleMon, // 24: ARBOK
    SgbPaletteId::YellowMon, // 25: PIKACHU
    SgbPaletteId::YellowMon, // 26: RAICHU
    SgbPaletteId::BrownMon,  // 27: SANDSHREW
    SgbPaletteId::BrownMon,  // 28: SANDSLASH
    SgbPaletteId::BlueMon,   // 29: NIDORAN_F
    SgbPaletteId::BlueMon,   // 30: NIDORINA
    SgbPaletteId::BlueMon,   // 31: NIDOQUEEN
    SgbPaletteId::PurpleMon, // 32: NIDORAN_M
    SgbPaletteId::PurpleMon, // 33: NIDORINO
    SgbPaletteId::PurpleMon, // 34: NIDOKING
    SgbPaletteId::PinkMon,   // 35: CLEFAIRY
    SgbPaletteId::PinkMon,   // 36: CLEFABLE
    SgbPaletteId::RedMon,    // 37: VULPIX
    SgbPaletteId::YellowMon, // 38: NINETALES
    SgbPaletteId::PinkMon,   // 39: JIGGLYPUFF
    SgbPaletteId::PinkMon,   // 40: WIGGLYTUFF
    SgbPaletteId::BlueMon,   // 41: ZUBAT
    SgbPaletteId::BlueMon,   // 42: GOLBAT
    SgbPaletteId::GreenMon,  // 43: ODDISH
    SgbPaletteId::RedMon,    // 44: GLOOM
    SgbPaletteId::RedMon,    // 45: VILEPLUME
    SgbPaletteId::RedMon,    // 46: PARAS
    SgbPaletteId::RedMon,    // 47: PARASECT
    SgbPaletteId::PurpleMon, // 48: VENONAT
    SgbPaletteId::PurpleMon, // 49: VENOMOTH
    SgbPaletteId::BrownMon,  // 50: DIGLETT
    SgbPaletteId::BrownMon,  // 51: DUGTRIO
    SgbPaletteId::YellowMon, // 52: MEOWTH
    SgbPaletteId::YellowMon, // 53: PERSIAN
    SgbPaletteId::YellowMon, // 54: PSYDUCK
    SgbPaletteId::CyanMon,   // 55: GOLDUCK
    SgbPaletteId::BrownMon,  // 56: MANKEY
    SgbPaletteId::BrownMon,  // 57: PRIMEAPE
    SgbPaletteId::BrownMon,  // 58: GROWLITHE
    SgbPaletteId::RedMon,    // 59: ARCANINE
    SgbPaletteId::BlueMon,   // 60: POLIWAG
    SgbPaletteId::BlueMon,   // 61: POLIWHIRL
    SgbPaletteId::BlueMon,   // 62: POLIWRATH
    SgbPaletteId::YellowMon, // 63: ABRA
    SgbPaletteId::YellowMon, // 64: KADABRA
    SgbPaletteId::YellowMon, // 65: ALAKAZAM
    SgbPaletteId::GrayMon,   // 66: MACHOP
    SgbPaletteId::GrayMon,   // 67: MACHOKE
    SgbPaletteId::GrayMon,   // 68: MACHAMP
    SgbPaletteId::GreenMon,  // 69: BELLSPROUT
    SgbPaletteId::GreenMon,  // 70: WEEPINBELL
    SgbPaletteId::GreenMon,  // 71: VICTREEBEL
    SgbPaletteId::CyanMon,   // 72: TENTACOOL
    SgbPaletteId::CyanMon,   // 73: TENTACRUEL
    SgbPaletteId::GrayMon,   // 74: GEODUDE
    SgbPaletteId::GrayMon,   // 75: GRAVELER
    SgbPaletteId::GrayMon,   // 76: GOLEM
    SgbPaletteId::RedMon,    // 77: PONYTA
    SgbPaletteId::RedMon,    // 78: RAPIDASH
    SgbPaletteId::PinkMon,   // 79: SLOWPOKE
    SgbPaletteId::PinkMon,   // 80: SLOWBRO
    SgbPaletteId::GrayMon,   // 81: MAGNEMITE
    SgbPaletteId::GrayMon,   // 82: MAGNETON
    SgbPaletteId::BrownMon,  // 83: FARFETCH'D
    SgbPaletteId::BrownMon,  // 84: DODUO
    SgbPaletteId::BrownMon,  // 85: DODRIO
    SgbPaletteId::BlueMon,   // 86: SEEL
    SgbPaletteId::BlueMon,   // 87: DEWGONG
    SgbPaletteId::PurpleMon, // 88: GRIMER
    SgbPaletteId::PurpleMon, // 89: MUK
    SgbPaletteId::GrayMon,   // 90: SHELLDER
    SgbPaletteId::GrayMon,   // 91: CLOYSTER
    SgbPaletteId::PurpleMon, // 92: GASTLY
    SgbPaletteId::PurpleMon, // 93: HAUNTER
    SgbPaletteId::PurpleMon, // 94: GENGAR
    SgbPaletteId::GrayMon,   // 95: ONIX
    SgbPaletteId::YellowMon, // 96: DROWZEE
    SgbPaletteId::YellowMon, // 97: HYPNO
    SgbPaletteId::RedMon,    // 98: KRABBY
    SgbPaletteId::RedMon,    // 99: KINGLER
    SgbPaletteId::YellowMon, // 100: VOLTORB
    SgbPaletteId::YellowMon, // 101: ELECTRODE
    SgbPaletteId::PinkMon,   // 102: EXEGGCUTE
    SgbPaletteId::GreenMon,  // 103: EXEGGUTOR
    SgbPaletteId::GrayMon,   // 104: CUBONE
    SgbPaletteId::GrayMon,   // 105: MAROWAK
    SgbPaletteId::BrownMon,  // 106: HITMONLEE
    SgbPaletteId::BrownMon,  // 107: HITMONCHAN
    SgbPaletteId::PinkMon,   // 108: LICKITUNG
    SgbPaletteId::PurpleMon, // 109: KOFFING
    SgbPaletteId::PurpleMon, // 110: WEEZING
    SgbPaletteId::GrayMon,   // 111: RHYHORN
    SgbPaletteId::GrayMon,   // 112: RHYDON
    SgbPaletteId::PinkMon,   // 113: CHANSEY
    SgbPaletteId::BlueMon,   // 114: TANGELA
    SgbPaletteId::BrownMon,  // 115: KANGASKHAN
    SgbPaletteId::CyanMon,   // 116: HORSEA
    SgbPaletteId::CyanMon,   // 117: SEADRA
    SgbPaletteId::RedMon,    // 118: GOLDEEN
    SgbPaletteId::RedMon,    // 119: SEAKING
    SgbPaletteId::RedMon,    // 120: STARYU
    SgbPaletteId::GrayMon,   // 121: STARMIE
    SgbPaletteId::PinkMon,   // 122: MR. MIME
    SgbPaletteId::GreenMon,  // 123: SCYTHER
    SgbPaletteId::MewMon,    // 124: JYNX
    SgbPaletteId::YellowMon, // 125: ELECTABUZZ
    SgbPaletteId::RedMon,    // 126: MAGMAR
    SgbPaletteId::BrownMon,  // 127: PINSIR
    SgbPaletteId::GrayMon,   // 128: TAUROS
    SgbPaletteId::RedMon,    // 129: MAGIKARP
    SgbPaletteId::BlueMon,   // 130: GYARADOS
    SgbPaletteId::CyanMon,   // 131: LAPRAS
    SgbPaletteId::GrayMon,   // 132: DITTO
    SgbPaletteId::GrayMon,   // 133: EEVEE
    SgbPaletteId::CyanMon,   // 134: VAPOREON
    SgbPaletteId::YellowMon, // 135: JOLTEON
    SgbPaletteId::RedMon,    // 136: FLAREON
    SgbPaletteId::GrayMon,   // 137: PORYGON
    SgbPaletteId::BlueMon,   // 138: OMANYTE
    SgbPaletteId::BlueMon,   // 139: OMASTAR
    SgbPaletteId::BrownMon,  // 140: KABUTO
    SgbPaletteId::BrownMon,  // 141: KABUTOPS
    SgbPaletteId::GrayMon,   // 142: AERODACTYL
    SgbPaletteId::PinkMon,   // 143: SNORLAX
    SgbPaletteId::BlueMon,   // 144: ARTICUNO
    SgbPaletteId::YellowMon, // 145: ZAPDOS
    SgbPaletteId::RedMon,    // 146: MOLTRES
    SgbPaletteId::GrayMon,   // 147: DRATINI
    SgbPaletteId::BlueMon,   // 148: DRAGONAIR
    SgbPaletteId::BrownMon,  // 149: DRAGONITE
    SgbPaletteId::MewMon,    // 150: MEWTWO
    SgbPaletteId::MewMon,    // 151: MEW
];

/// Lookup the SGB palette for a Pokémon by Pokédex index (1-based).
/// Returns None for invalid indices. Index 0 returns MISSINGNO's palette.
pub fn monster_palette(pokedex_index: u8) -> SgbPaletteId {
    if (pokedex_index as usize) < NUM_POKEMON_PLUS_ONE {
        MONSTER_PALETTES[pokedex_index as usize]
    } else {
        MONSTER_PALETTES[0] // fallback to MISSINGNO
    }
}

/// Determine a Pokémon's palette ID, taking Transform into account.
/// If `is_transformed` is true, uses PAL_GRAYMON (Ditto's palette).
/// Otherwise looks up from MonsterPalettes.
/// Mirrors `DeterminePaletteID` from engine/gfx/palettes.asm.
pub fn determine_palette_id(species_index: u8, is_transformed: bool) -> SgbPaletteId {
    if is_transformed {
        SgbPaletteId::GrayMon
    } else {
        monster_palette(species_index)
    }
}

// ============================================================================
// Map/Tileset Constants for Overworld Palette Selection
// ============================================================================

/// Tileset ID for Pokemon Tower / Agatha's room.
pub const TILESET_CEMETERY: u8 = 15;
/// Tileset ID for caves (Rock Tunnel, Victory Road, etc).
pub const TILESET_CAVERN: u8 = 17;
/// Number of city/town maps (PALLET_TOWN..SAFFRON_CITY = 0x00..0x0A).
pub const NUM_CITY_MAPS: u8 = 0x0B;
/// First indoor map ID (REDS_HOUSE_1F = 0x25).
pub const FIRST_INDOOR_MAP: u8 = 0x25;
/// CERULEAN_CAVE_2F map ID.
pub const MAP_CERULEAN_CAVE_2F: u8 = 0xE2;
/// CERULEAN_CAVE_1F map ID.
pub const MAP_CERULEAN_CAVE_1F: u8 = 0xE4;
/// LORELEI'S_ROOM map ID.
pub const MAP_LORELEIS_ROOM: u8 = 0xF5;
/// BRUNO'S_ROOM map ID.
pub const MAP_BRUNOS_ROOM: u8 = 0xF6;

/// Determine the overworld SGB palette for a given map.
///
/// Mirrors `SetPal_Overworld` from engine/gfx/palettes.asm.
///
/// # Parameters
/// - `tileset`: The current map's tileset ID.
/// - `map_id`: The current map ID.
/// - `last_map`: The town/route the current indoor map belongs to (wLastMap).
///
/// # Returns
/// The SGB palette ID to use for this map location.
pub fn overworld_palette_for_map(tileset: u8, map_id: u8, last_map: u8) -> SgbPaletteId {
    // Cemetery tileset → PAL_GRAYMON (Pokemon Tower, Agatha's room)
    if tileset == TILESET_CEMETERY {
        return SgbPaletteId::GrayMon;
    }
    // Cavern tileset → PAL_CAVE
    if tileset == TILESET_CAVERN {
        return SgbPaletteId::Cave;
    }

    // Determine the effective "town" value
    let town = if map_id < FIRST_INDOOR_MAP {
        // Outdoor town or route — use map_id directly
        map_id
    } else if map_id >= MAP_CERULEAN_CAVE_2F && map_id <= MAP_CERULEAN_CAVE_1F {
        // Cerulean Cave maps → PAL_CAVE
        return SgbPaletteId::Cave;
    } else if map_id == MAP_LORELEIS_ROOM {
        // Lorelei's room → PAL_ROUTE (xor a; inc a → 1 → PAL_ROUTE... actually 0+1=1=PAL_PALLET)
        return SgbPaletteId::Pallet;
    } else if map_id == MAP_BRUNOS_ROOM {
        // Bruno's room → PAL_CAVE
        return SgbPaletteId::Cave;
    } else {
        // Normal indoor map — use last_map (the town/route the building is in)
        last_map
    };

    // If town < NUM_CITY_MAPS, use (town + 1) as palette index.
    if town < NUM_CITY_MAPS {
        // town's palette = town + 1 (matches const_def order: PALLET=1, VIRIDIAN=2, etc.)
        SgbPaletteId::from_u8(town + 1).unwrap_or(SgbPaletteId::Route)
    } else {
        // Route or out-of-range → PAL_ROUTE
        SgbPaletteId::Route
    }
}

// ============================================================================
// HP Bar Color Conversion
// ============================================================================

/// Convert HP bar color index (0=green, 1=yellow, 2=red) to SGB palette ID.
pub fn hp_bar_to_sgb_palette(hp_bar_color: u8) -> SgbPaletteId {
    match hp_bar_color {
        0 => SgbPaletteId::GreenBar,
        1 => SgbPaletteId::YellowBar,
        _ => SgbPaletteId::RedBar,
    }
}
