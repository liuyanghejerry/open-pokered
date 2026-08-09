//! Per-species cry data — a port of `data/pokemon/cries.asm` (`CryData`).
//!
//! `GetCryData` (home/pokemon.asm) loads, for a species, the base cry SFX id
//! plus the pitch/length bytes written to `wFrequencyModifier` /
//! `wTempoModifier`; the audio engine then applies them when the cry plays
//! (`Audio1_ApplyFrequencyModifier` / `Audio1_SetSfxTempo`, engine_1.asm).
//! Verified byte-for-byte by `scripts/verify_cry_data.py`.

use crate::species::Species;

/// One `mon_cry` row: base cry SFX plus pitch/length modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CryData {
    /// Base cry in `SfxId` id space (`SFX_CRY_00`..=`SFX_CRY_25`).
    pub sfx: u8,
    /// `wFrequencyModifier` — added to the cry's note frequencies.
    pub pitch: u8,
    /// `wTempoModifier` — the cry's tempo becomes `0x0080 + length`
    /// (`Audio1_SetSfxTempo`).
    pub length: u8,
}

/// `GetCryData`: base cry + pitch/length modifiers for a species.
pub fn cry_data(species: Species) -> CryData {
    match species {
    Species::Rhydon => CryData { sfx: 36, pitch: 0x00, length: 0x80 }, // Rhydon (SFX_CRY_11)
    Species::Kangaskhan => CryData { sfx: 22, pitch: 0x00, length: 0x80 }, // Kangaskhan (SFX_CRY_03)
    Species::NidoranM => CryData { sfx: 19, pitch: 0x00, length: 0x80 }, // Nidoran♂ (SFX_CRY_00)
    Species::Clefairy => CryData { sfx: 44, pitch: 0xCC, length: 0x01 }, // Clefairy (SFX_CRY_19)
    Species::Spearow => CryData { sfx: 35, pitch: 0x00, length: 0x80 }, // Spearow (SFX_CRY_10)
    Species::Voltorb => CryData { sfx: 25, pitch: 0xED, length: 0x80 }, // Voltorb (SFX_CRY_06)
    Species::Nidoking => CryData { sfx: 28, pitch: 0x00, length: 0x80 }, // Nidoking (SFX_CRY_09)
    Species::Slowbro => CryData { sfx: 50, pitch: 0x00, length: 0x80 }, // Slowbro (SFX_CRY_1F)
    Species::Ivysaur => CryData { sfx: 34, pitch: 0x20, length: 0x80 }, // Ivysaur (SFX_CRY_0F)
    Species::Exeggutor => CryData { sfx: 32, pitch: 0x00, length: 0x80 }, // Exeggutor (SFX_CRY_0D)
    Species::Lickitung => CryData { sfx: 31, pitch: 0x00, length: 0x80 }, // Lickitung (SFX_CRY_0C)
    Species::Exeggcute => CryData { sfx: 30, pitch: 0x00, length: 0x80 }, // Exeggcute (SFX_CRY_0B)
    Species::Grimer => CryData { sfx: 24, pitch: 0x00, length: 0x80 }, // Grimer (SFX_CRY_05)
    Species::Gengar => CryData { sfx: 26, pitch: 0x00, length: 0xFF }, // Gengar (SFX_CRY_07)
    Species::NidoranF => CryData { sfx: 20, pitch: 0x00, length: 0x80 }, // Nidoran♀ (SFX_CRY_01)
    Species::Nidoqueen => CryData { sfx: 29, pitch: 0x00, length: 0x80 }, // Nidoqueen (SFX_CRY_0A)
    Species::Cubone => CryData { sfx: 44, pitch: 0x00, length: 0x80 }, // Cubone (SFX_CRY_19)
    Species::Rhyhorn => CryData { sfx: 23, pitch: 0x00, length: 0x80 }, // Rhyhorn (SFX_CRY_04)
    Species::Lapras => CryData { sfx: 46, pitch: 0x00, length: 0x80 }, // Lapras (SFX_CRY_1B)
    Species::Arcanine => CryData { sfx: 40, pitch: 0x00, length: 0x80 }, // Arcanine (SFX_CRY_15)
    Species::Mew => CryData { sfx: 49, pitch: 0xEE, length: 0xFF }, // Mew (SFX_CRY_1E)
    Species::Gyarados => CryData { sfx: 42, pitch: 0x00, length: 0x80 }, // Gyarados (SFX_CRY_17)
    Species::Shellder => CryData { sfx: 43, pitch: 0x00, length: 0x80 }, // Shellder (SFX_CRY_18)
    Species::Tentacool => CryData { sfx: 45, pitch: 0x00, length: 0x80 }, // Tentacool (SFX_CRY_1A)
    Species::Gastly => CryData { sfx: 47, pitch: 0x00, length: 0x80 }, // Gastly (SFX_CRY_1C)
    Species::Scyther => CryData { sfx: 41, pitch: 0x00, length: 0x80 }, // Scyther (SFX_CRY_16)
    Species::Staryu => CryData { sfx: 49, pitch: 0x02, length: 0x20 }, // Staryu (SFX_CRY_1E)
    Species::Blastoise => CryData { sfx: 38, pitch: 0x00, length: 0x80 }, // Blastoise (SFX_CRY_13)
    Species::Pinsir => CryData { sfx: 39, pitch: 0x00, length: 0x80 }, // Pinsir (SFX_CRY_14)
    Species::Tangela => CryData { sfx: 37, pitch: 0x00, length: 0x80 }, // Tangela (SFX_CRY_12)
    Species::Growlithe => CryData { sfx: 50, pitch: 0x20, length: 0x40 }, // Growlithe (SFX_CRY_1F)
    Species::Onix => CryData { sfx: 42, pitch: 0xFF, length: 0xC0 }, // Onix (SFX_CRY_17)
    Species::Fearow => CryData { sfx: 43, pitch: 0x40, length: 0xA0 }, // Fearow (SFX_CRY_18)
    Species::Pidgey => CryData { sfx: 33, pitch: 0xDF, length: 0x04 }, // Pidgey (SFX_CRY_0E)
    Species::Slowpoke => CryData { sfx: 21, pitch: 0x00, length: 0x80 }, // Slowpoke (SFX_CRY_02)
    Species::Kadabra => CryData { sfx: 47, pitch: 0xA8, length: 0xC0 }, // Kadabra (SFX_CRY_1C)
    Species::Graveler => CryData { sfx: 55, pitch: 0x00, length: 0x80 }, // Graveler (SFX_CRY_24)
    Species::Chansey => CryData { sfx: 39, pitch: 0x0A, length: 0xC0 }, // Chansey (SFX_CRY_14)
    Species::Machoke => CryData { sfx: 50, pitch: 0x48, length: 0x60 }, // Machoke (SFX_CRY_1F)
    Species::MrMime => CryData { sfx: 51, pitch: 0x08, length: 0x40 }, // Mr.Mime (SFX_CRY_20)
    Species::Hitmonlee => CryData { sfx: 37, pitch: 0x80, length: 0xC0 }, // Hitmonlee (SFX_CRY_12)
    Species::Hitmonchan => CryData { sfx: 31, pitch: 0xEE, length: 0xC0 }, // Hitmonchan (SFX_CRY_0C)
    Species::Arbok => CryData { sfx: 42, pitch: 0xE0, length: 0x10 }, // Arbok (SFX_CRY_17)
    Species::Parasect => CryData { sfx: 49, pitch: 0x42, length: 0xFF }, // Parasect (SFX_CRY_1E)
    Species::Psyduck => CryData { sfx: 52, pitch: 0x20, length: 0x60 }, // Psyduck (SFX_CRY_21)
    Species::Drowzee => CryData { sfx: 32, pitch: 0x88, length: 0x20 }, // Drowzee (SFX_CRY_0D)
    Species::Golem => CryData { sfx: 37, pitch: 0xE0, length: 0x40 }, // Golem (SFX_CRY_12)
    Species::Magmar => CryData { sfx: 23, pitch: 0xFF, length: 0x30 }, // Magmar (SFX_CRY_04)
    Species::Electabuzz => CryData { sfx: 25, pitch: 0x8F, length: 0xFF }, // Electabuzz (SFX_CRY_06)
    Species::Magneton => CryData { sfx: 47, pitch: 0x20, length: 0xC0 }, // Magneton (SFX_CRY_1C)
    Species::Koffing => CryData { sfx: 37, pitch: 0xE6, length: 0xDD }, // Koffing (SFX_CRY_12)
    Species::Mankey => CryData { sfx: 29, pitch: 0xDD, length: 0x60 }, // Mankey (SFX_CRY_0A)
    Species::Seel => CryData { sfx: 31, pitch: 0x88, length: 0xC0 }, // Seel (SFX_CRY_0C)
    Species::Diglett => CryData { sfx: 30, pitch: 0xAA, length: 0x01 }, // Diglett (SFX_CRY_0B)
    Species::Tauros => CryData { sfx: 48, pitch: 0x11, length: 0x40 }, // Tauros (SFX_CRY_1D)
    Species::Farfetchd => CryData { sfx: 35, pitch: 0xDD, length: 0x01 }, // Farfetch'd (SFX_CRY_10)
    Species::Venonat => CryData { sfx: 45, pitch: 0x44, length: 0x40 }, // Venonat (SFX_CRY_1A)
    Species::Dragonite => CryData { sfx: 34, pitch: 0x3C, length: 0xC0 }, // Dragonite (SFX_CRY_0F)
    Species::Doduo => CryData { sfx: 30, pitch: 0xBB, length: 0x01 }, // Doduo (SFX_CRY_0B)
    Species::Poliwag => CryData { sfx: 33, pitch: 0xFF, length: 0xFF }, // Poliwag (SFX_CRY_0E)
    Species::Jynx => CryData { sfx: 32, pitch: 0xFF, length: 0xFF }, // Jynx (SFX_CRY_0D)
    Species::Moltres => CryData { sfx: 28, pitch: 0xF8, length: 0x40 }, // Moltres (SFX_CRY_09)
    Species::Articuno => CryData { sfx: 28, pitch: 0x80, length: 0x40 }, // Articuno (SFX_CRY_09)
    Species::Zapdos => CryData { sfx: 43, pitch: 0xFF, length: 0x80 }, // Zapdos (SFX_CRY_18)
    Species::Ditto => CryData { sfx: 33, pitch: 0xFF, length: 0xFF }, // Ditto (SFX_CRY_0E)
    Species::Meowth => CryData { sfx: 44, pitch: 0x77, length: 0x10 }, // Meowth (SFX_CRY_19)
    Species::Krabby => CryData { sfx: 51, pitch: 0x20, length: 0xE0 }, // Krabby (SFX_CRY_20)
    Species::Vulpix => CryData { sfx: 55, pitch: 0x4F, length: 0x10 }, // Vulpix (SFX_CRY_24)
    Species::Ninetales => CryData { sfx: 55, pitch: 0x88, length: 0x60 }, // Ninetales (SFX_CRY_24)
    Species::Pikachu => CryData { sfx: 34, pitch: 0xEE, length: 0x01 }, // Pikachu (SFX_CRY_0F)
    Species::Raichu => CryData { sfx: 28, pitch: 0xEE, length: 0x08 }, // Raichu (SFX_CRY_09)
    Species::Dratini => CryData { sfx: 34, pitch: 0x60, length: 0x40 }, // Dratini (SFX_CRY_0F)
    Species::Dragonair => CryData { sfx: 34, pitch: 0x40, length: 0x80 }, // Dragonair (SFX_CRY_0F)
    Species::Kabuto => CryData { sfx: 41, pitch: 0xBB, length: 0x40 }, // Kabuto (SFX_CRY_16)
    Species::Kabutops => CryData { sfx: 43, pitch: 0xEE, length: 0x01 }, // Kabutops (SFX_CRY_18)
    Species::Horsea => CryData { sfx: 44, pitch: 0x99, length: 0x10 }, // Horsea (SFX_CRY_19)
    Species::Seadra => CryData { sfx: 44, pitch: 0x3C, length: 0x01 }, // Seadra (SFX_CRY_19)
    Species::Sandshrew => CryData { sfx: 19, pitch: 0x20, length: 0x40 }, // Sandshrew (SFX_CRY_00)
    Species::Sandslash => CryData { sfx: 19, pitch: 0xFF, length: 0xFF }, // Sandslash (SFX_CRY_00)
    Species::Omanyte => CryData { sfx: 50, pitch: 0xF0, length: 0x01 }, // Omanyte (SFX_CRY_1F)
    Species::Omastar => CryData { sfx: 50, pitch: 0xFF, length: 0x40 }, // Omastar (SFX_CRY_1F)
    Species::Jigglypuff => CryData { sfx: 33, pitch: 0xFF, length: 0x35 }, // Jigglypuff (SFX_CRY_0E)
    Species::Wigglytuff => CryData { sfx: 33, pitch: 0x68, length: 0x60 }, // Wigglytuff (SFX_CRY_0E)
    Species::Eevee => CryData { sfx: 45, pitch: 0x88, length: 0x60 }, // Eevee (SFX_CRY_1A)
    Species::Flareon => CryData { sfx: 45, pitch: 0x10, length: 0x20 }, // Flareon (SFX_CRY_1A)
    Species::Jolteon => CryData { sfx: 45, pitch: 0x3D, length: 0x80 }, // Jolteon (SFX_CRY_1A)
    Species::Vaporeon => CryData { sfx: 45, pitch: 0xAA, length: 0xFF }, // Vaporeon (SFX_CRY_1A)
    Species::Machop => CryData { sfx: 50, pitch: 0xEE, length: 0x01 }, // Machop (SFX_CRY_1F)
    Species::Zubat => CryData { sfx: 48, pitch: 0xE0, length: 0x80 }, // Zubat (SFX_CRY_1D)
    Species::Ekans => CryData { sfx: 42, pitch: 0x12, length: 0x40 }, // Ekans (SFX_CRY_17)
    Species::Paras => CryData { sfx: 49, pitch: 0x20, length: 0xE0 }, // Paras (SFX_CRY_1E)
    Species::Poliwhirl => CryData { sfx: 33, pitch: 0x77, length: 0x60 }, // Poliwhirl (SFX_CRY_0E)
    Species::Poliwrath => CryData { sfx: 33, pitch: 0x00, length: 0xFF }, // Poliwrath (SFX_CRY_0E)
    Species::Weedle => CryData { sfx: 40, pitch: 0xEE, length: 0x01 }, // Weedle (SFX_CRY_15)
    Species::Kakuna => CryData { sfx: 38, pitch: 0xFF, length: 0x01 }, // Kakuna (SFX_CRY_13)
    Species::Beedrill => CryData { sfx: 38, pitch: 0x60, length: 0x80 }, // Beedrill (SFX_CRY_13)
    Species::Dodrio => CryData { sfx: 30, pitch: 0x99, length: 0x20 }, // Dodrio (SFX_CRY_0B)
    Species::Primeape => CryData { sfx: 29, pitch: 0xAF, length: 0x40 }, // Primeape (SFX_CRY_0A)
    Species::Dugtrio => CryData { sfx: 30, pitch: 0x2A, length: 0x10 }, // Dugtrio (SFX_CRY_0B)
    Species::Venomoth => CryData { sfx: 45, pitch: 0x29, length: 0x80 }, // Venomoth (SFX_CRY_1A)
    Species::Dewgong => CryData { sfx: 31, pitch: 0x23, length: 0xFF }, // Dewgong (SFX_CRY_0C)
    Species::Caterpie => CryData { sfx: 41, pitch: 0x80, length: 0x20 }, // Caterpie (SFX_CRY_16)
    Species::Metapod => CryData { sfx: 47, pitch: 0xCC, length: 0x01 }, // Metapod (SFX_CRY_1C)
    Species::Butterfree => CryData { sfx: 41, pitch: 0x77, length: 0x40 }, // Butterfree (SFX_CRY_16)
    Species::Machamp => CryData { sfx: 50, pitch: 0x08, length: 0xC0 }, // Machamp (SFX_CRY_1F)
    Species::Golduck => CryData { sfx: 52, pitch: 0xFF, length: 0x40 }, // Golduck (SFX_CRY_21)
    Species::Hypno => CryData { sfx: 32, pitch: 0xEE, length: 0x40 }, // Hypno (SFX_CRY_0D)
    Species::Golbat => CryData { sfx: 48, pitch: 0xFA, length: 0x80 }, // Golbat (SFX_CRY_1D)
    Species::Mewtwo => CryData { sfx: 49, pitch: 0x99, length: 0xFF }, // Mewtwo (SFX_CRY_1E)
    Species::Snorlax => CryData { sfx: 24, pitch: 0x55, length: 0x01 }, // Snorlax (SFX_CRY_05)
    Species::Magikarp => CryData { sfx: 42, pitch: 0x80, length: 0x00 }, // Magikarp (SFX_CRY_17)
    Species::Muk => CryData { sfx: 26, pitch: 0xEF, length: 0xFF }, // Muk (SFX_CRY_07)
    Species::Kingler => CryData { sfx: 51, pitch: 0xEE, length: 0xE0 }, // Kingler (SFX_CRY_20)
    Species::Cloyster => CryData { sfx: 43, pitch: 0x6F, length: 0xE0 }, // Cloyster (SFX_CRY_18)
    Species::Electrode => CryData { sfx: 25, pitch: 0xA8, length: 0x90 }, // Electrode (SFX_CRY_06)
    Species::Clefable => CryData { sfx: 44, pitch: 0xAA, length: 0x20 }, // Clefable (SFX_CRY_19)
    Species::Weezing => CryData { sfx: 37, pitch: 0xFF, length: 0xFF }, // Weezing (SFX_CRY_12)
    Species::Persian => CryData { sfx: 44, pitch: 0x99, length: 0xFF }, // Persian (SFX_CRY_19)
    Species::Marowak => CryData { sfx: 27, pitch: 0x4F, length: 0x60 }, // Marowak (SFX_CRY_08)
    Species::Haunter => CryData { sfx: 47, pitch: 0x30, length: 0x40 }, // Haunter (SFX_CRY_1C)
    Species::Abra => CryData { sfx: 47, pitch: 0xC0, length: 0x01 }, // Abra (SFX_CRY_1C)
    Species::Alakazam => CryData { sfx: 47, pitch: 0x98, length: 0xFF }, // Alakazam (SFX_CRY_1C)
    Species::Pidgeotto => CryData { sfx: 39, pitch: 0x28, length: 0xC0 }, // Pidgeotto (SFX_CRY_14)
    Species::Pidgeot => CryData { sfx: 39, pitch: 0x11, length: 0xFF }, // Pidgeot (SFX_CRY_14)
    Species::Starmie => CryData { sfx: 49, pitch: 0x00, length: 0x80 }, // Starmie (SFX_CRY_1E)
    Species::Bulbasaur => CryData { sfx: 34, pitch: 0x80, length: 0x01 }, // Bulbasaur (SFX_CRY_0F)
    Species::Venusaur => CryData { sfx: 34, pitch: 0x00, length: 0xC0 }, // Venusaur (SFX_CRY_0F)
    Species::Tentacruel => CryData { sfx: 45, pitch: 0xEE, length: 0xFF }, // Tentacruel (SFX_CRY_1A)
    Species::Goldeen => CryData { sfx: 41, pitch: 0x80, length: 0x40 }, // Goldeen (SFX_CRY_16)
    Species::Seaking => CryData { sfx: 41, pitch: 0x10, length: 0xFF }, // Seaking (SFX_CRY_16)
    Species::Ponyta => CryData { sfx: 56, pitch: 0x00, length: 0x80 }, // Ponyta (SFX_CRY_25)
    Species::Rapidash => CryData { sfx: 56, pitch: 0x20, length: 0xC0 }, // Rapidash (SFX_CRY_25)
    Species::Rattata => CryData { sfx: 53, pitch: 0x00, length: 0x80 }, // Rattata (SFX_CRY_22)
    Species::Raticate => CryData { sfx: 53, pitch: 0x20, length: 0xFF }, // Raticate (SFX_CRY_22)
    Species::Nidorino => CryData { sfx: 19, pitch: 0x2C, length: 0xC0 }, // Nidorino (SFX_CRY_00)
    Species::Nidorina => CryData { sfx: 20, pitch: 0x2C, length: 0xE0 }, // Nidorina (SFX_CRY_01)
    Species::Geodude => CryData { sfx: 55, pitch: 0xF0, length: 0x10 }, // Geodude (SFX_CRY_24)
    Species::Porygon => CryData { sfx: 56, pitch: 0xAA, length: 0xFF }, // Porygon (SFX_CRY_25)
    Species::Aerodactyl => CryData { sfx: 54, pitch: 0x20, length: 0xF0 }, // Aerodactyl (SFX_CRY_23)
    Species::Magnemite => CryData { sfx: 47, pitch: 0x80, length: 0x60 }, // Magnemite (SFX_CRY_1C)
    Species::Charmander => CryData { sfx: 23, pitch: 0x60, length: 0x40 }, // Charmander (SFX_CRY_04)
    Species::Squirtle => CryData { sfx: 48, pitch: 0x60, length: 0x40 }, // Squirtle (SFX_CRY_1D)
    Species::Charmeleon => CryData { sfx: 23, pitch: 0x20, length: 0x40 }, // Charmeleon (SFX_CRY_04)
    Species::Wartortle => CryData { sfx: 48, pitch: 0x20, length: 0x40 }, // Wartortle (SFX_CRY_1D)
    Species::Charizard => CryData { sfx: 23, pitch: 0x00, length: 0x80 }, // Charizard (SFX_CRY_04)
    Species::Oddish => CryData { sfx: 27, pitch: 0xDD, length: 0x01 }, // Oddish (SFX_CRY_08)
    Species::Gloom => CryData { sfx: 27, pitch: 0xAA, length: 0x40 }, // Gloom (SFX_CRY_08)
    Species::Vileplume => CryData { sfx: 54, pitch: 0x22, length: 0xFF }, // Vileplume (SFX_CRY_23)
    Species::Bellsprout => CryData { sfx: 52, pitch: 0x55, length: 0x01 }, // Bellsprout (SFX_CRY_21)
    Species::Weepinbell => CryData { sfx: 56, pitch: 0x44, length: 0x20 }, // Weepinbell (SFX_CRY_25)
    Species::Victreebel => CryData { sfx: 56, pitch: 0x66, length: 0xCC }, // Victreebel (SFX_CRY_25)
        _ => CryData {
            sfx: 0,
            pitch: 0,
            length: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spot_check_cries() {
        // Bulbasaur / Mewtwo rows from data/pokemon/cries.asm
        // (SfxId id space: Cry00 = 19; SFX_CRY_0F = 19+15, SFX_CRY_1E = 19+30).
        assert_eq!(
            cry_data(Species::Bulbasaur),
            CryData { sfx: 34, pitch: 0x80, length: 0x01 }
        );
        assert_eq!(
            cry_data(Species::Mewtwo),
            CryData { sfx: 49, pitch: 0x99, length: 0xFF }
        );
    }

    #[test]
    fn invalid_species_returns_silence() {
        let c = cry_data(Species::None);
        assert_eq!((c.pitch, c.length), (0, 0));
    }
}
