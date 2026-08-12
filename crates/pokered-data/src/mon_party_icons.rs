//! Party-menu icon mapping per species.
//!
//! Mirrors `data/pokemon/menu_icons.asm` from the original disassembly:
//! every species is assigned one of ten icon kinds used when the party
//! menu draws the 16×16 sprite next to the Pokémon's name.
//!
//! The ordering of [`IconKind`] variants matches the asm `ICON_*`
//! constants defined in `constants/icon_constants.asm`.

use crate::species::Species;

pub use dotzuki_renderer::icon::IconKind;

/// Lookup the icon for a given species.
/// Species::None falls back to [`IconKind::Mon`].
pub fn icon_for_species(species: Species) -> IconKind {
    let idx = species as usize;
    if idx == 0 || idx > MON_PARTY_DATA.len() {
        return IconKind::Mon;
    }
    MON_PARTY_DATA[idx - 1]
}

/// Per-species icon table, indexed by `species as usize - 1`.
/// Order matches `data/pokemon/menu_icons.asm`.
pub const MON_PARTY_DATA: [IconKind; 151] = [
    IconKind::Grass,     // 1   Bulbasaur
    IconKind::Grass,     // 2   Ivysaur
    IconKind::Grass,     // 3   Venusaur
    IconKind::Mon,       // 4   Charmander
    IconKind::Mon,       // 5   Charmeleon
    IconKind::Mon,       // 6   Charizard
    IconKind::Water,     // 7   Squirtle
    IconKind::Water,     // 8   Wartortle
    IconKind::Water,     // 9   Blastoise
    IconKind::Bug,       // 10  Caterpie
    IconKind::Bug,       // 11  Metapod
    IconKind::Bug,       // 12  Butterfree
    IconKind::Bug,       // 13  Weedle
    IconKind::Bug,       // 14  Kakuna
    IconKind::Bug,       // 15  Beedrill
    IconKind::Bird,      // 16  Pidgey
    IconKind::Bird,      // 17  Pidgeotto
    IconKind::Bird,      // 18  Pidgeot
    IconKind::Quadruped, // 19  Rattata
    IconKind::Quadruped, // 20  Raticate
    IconKind::Bird,      // 21  Spearow
    IconKind::Bird,      // 22  Fearow
    IconKind::Snake,     // 23  Ekans
    IconKind::Snake,     // 24  Arbok
    IconKind::Fairy,     // 25  Pikachu
    IconKind::Fairy,     // 26  Raichu
    IconKind::Mon,       // 27  Sandshrew
    IconKind::Mon,       // 28  Sandslash
    IconKind::Mon,       // 29  NidoranF
    IconKind::Mon,       // 30  Nidorina
    IconKind::Mon,       // 31  Nidoqueen
    IconKind::Mon,       // 32  NidoranM
    IconKind::Mon,       // 33  Nidorino
    IconKind::Mon,       // 34  Nidoking
    IconKind::Fairy,     // 35  Clefairy
    IconKind::Fairy,     // 36  Clefable
    IconKind::Quadruped, // 37  Vulpix
    IconKind::Quadruped, // 38  Ninetales
    IconKind::Fairy,     // 39  Jigglypuff
    IconKind::Fairy,     // 40  Wigglytuff
    IconKind::Mon,       // 41  Zubat
    IconKind::Mon,       // 42  Golbat
    IconKind::Grass,     // 43  Oddish
    IconKind::Grass,     // 44  Gloom
    IconKind::Grass,     // 45  Vileplume
    IconKind::Bug,       // 46  Paras
    IconKind::Bug,       // 47  Parasect
    IconKind::Bug,       // 48  Venonat
    IconKind::Bug,       // 49  Venomoth
    IconKind::Mon,       // 50  Diglett
    IconKind::Mon,       // 51  Dugtrio
    IconKind::Mon,       // 52  Meowth
    IconKind::Mon,       // 53  Persian
    IconKind::Mon,       // 54  Psyduck
    IconKind::Mon,       // 55  Golduck
    IconKind::Mon,       // 56  Mankey
    IconKind::Mon,       // 57  Primeape
    IconKind::Quadruped, // 58  Growlithe
    IconKind::Quadruped, // 59  Arcanine
    IconKind::Mon,       // 60  Poliwag
    IconKind::Mon,       // 61  Poliwhirl
    IconKind::Mon,       // 62  Poliwrath
    IconKind::Mon,       // 63  Abra
    IconKind::Mon,       // 64  Kadabra
    IconKind::Mon,       // 65  Alakazam
    IconKind::Mon,       // 66  Machop
    IconKind::Mon,       // 67  Machoke
    IconKind::Mon,       // 68  Machamp
    IconKind::Grass,     // 69  Bellsprout
    IconKind::Grass,     // 70  Weepinbell
    IconKind::Grass,     // 71  Victreebel
    IconKind::Water,     // 72  Tentacool
    IconKind::Water,     // 73  Tentacruel
    IconKind::Mon,       // 74  Geodude
    IconKind::Mon,       // 75  Graveler
    IconKind::Mon,       // 76  Golem
    IconKind::Quadruped, // 77  Ponyta
    IconKind::Quadruped, // 78  Rapidash
    IconKind::Quadruped, // 79  Slowpoke
    IconKind::Mon,       // 80  Slowbro
    IconKind::Ball,      // 81  Magnemite
    IconKind::Ball,      // 82  Magneton
    IconKind::Bird,      // 83  Farfetch'd
    IconKind::Bird,      // 84  Doduo
    IconKind::Bird,      // 85  Dodrio
    IconKind::Water,     // 86  Seel
    IconKind::Water,     // 87  Dewgong
    IconKind::Mon,       // 88  Grimer
    IconKind::Mon,       // 89  Muk
    IconKind::Helix,     // 90  Shellder
    IconKind::Helix,     // 91  Cloyster
    IconKind::Mon,       // 92  Gastly
    IconKind::Mon,       // 93  Haunter
    IconKind::Mon,       // 94  Gengar
    IconKind::Snake,     // 95  Onix
    IconKind::Mon,       // 96  Drowzee
    IconKind::Mon,       // 97  Hypno
    IconKind::Water,     // 98  Krabby
    IconKind::Water,     // 99  Kingler
    IconKind::Ball,      // 100 Voltorb
    IconKind::Ball,      // 101 Electrode
    IconKind::Grass,     // 102 Exeggcute
    IconKind::Grass,     // 103 Exeggutor
    IconKind::Mon,       // 104 Cubone
    IconKind::Mon,       // 105 Marowak
    IconKind::Mon,       // 106 Hitmonlee
    IconKind::Mon,       // 107 Hitmonchan
    IconKind::Mon,       // 108 Lickitung
    IconKind::Mon,       // 109 Koffing
    IconKind::Mon,       // 110 Weezing
    IconKind::Quadruped, // 111 Rhyhorn
    IconKind::Mon,       // 112 Rhydon
    IconKind::Fairy,     // 113 Chansey
    IconKind::Grass,     // 114 Tangela
    IconKind::Mon,       // 115 Kangaskhan
    IconKind::Water,     // 116 Horsea
    IconKind::Water,     // 117 Seadra
    IconKind::Water,     // 118 Goldeen
    IconKind::Water,     // 119 Seaking
    IconKind::Helix,     // 120 Staryu
    IconKind::Helix,     // 121 Starmie
    IconKind::Mon,       // 122 Mr. Mime
    IconKind::Bug,       // 123 Scyther
    IconKind::Mon,       // 124 Jynx
    IconKind::Mon,       // 125 Electabuzz
    IconKind::Mon,       // 126 Magmar
    IconKind::Bug,       // 127 Pinsir
    IconKind::Quadruped, // 128 Tauros
    IconKind::Water,     // 129 Magikarp
    IconKind::Snake,     // 130 Gyarados
    IconKind::Water,     // 131 Lapras
    IconKind::Mon,       // 132 Ditto
    IconKind::Quadruped, // 133 Eevee
    IconKind::Quadruped, // 134 Vaporeon
    IconKind::Quadruped, // 135 Jolteon
    IconKind::Quadruped, // 136 Flareon
    IconKind::Mon,       // 137 Porygon
    IconKind::Helix,     // 138 Omanyte
    IconKind::Helix,     // 139 Omastar
    IconKind::Helix,     // 140 Kabuto
    IconKind::Helix,     // 141 Kabutops
    IconKind::Bird,      // 142 Aerodactyl
    IconKind::Mon,       // 143 Snorlax
    IconKind::Bird,      // 144 Articuno
    IconKind::Bird,      // 145 Zapdos
    IconKind::Bird,      // 146 Moltres
    IconKind::Snake,     // 147 Dratini
    IconKind::Snake,     // 148 Dragonair
    IconKind::Snake,     // 149 Dragonite
    IconKind::Mon,       // 150 Mewtwo
    IconKind::Mon,       // 151 Mew
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_full() {
        // Baseline is the original 151; the editor can append extra species.
        assert!(MON_PARTY_DATA.len() >= 151);
    }

    #[test]
    fn known_mappings() {
        assert_eq!(icon_for_species(Species::Bulbasaur), IconKind::Grass);
        assert_eq!(icon_for_species(Species::Charmander), IconKind::Mon);
        assert_eq!(icon_for_species(Species::Squirtle), IconKind::Water);
        assert_eq!(icon_for_species(Species::Pikachu), IconKind::Fairy);
        assert_eq!(icon_for_species(Species::Pidgey), IconKind::Bird);
        assert_eq!(icon_for_species(Species::Caterpie), IconKind::Bug);
        assert_eq!(icon_for_species(Species::Ekans), IconKind::Snake);
        assert_eq!(icon_for_species(Species::Rattata), IconKind::Quadruped);
        assert_eq!(icon_for_species(Species::Magnemite), IconKind::Ball);
        assert_eq!(icon_for_species(Species::Shellder), IconKind::Helix);
        assert_eq!(icon_for_species(Species::Mew), IconKind::Mon);
    }

    #[test]
    fn none_falls_back_to_mon() {
        assert_eq!(icon_for_species(Species::None), IconKind::Mon);
    }
}
