//! Pokedex display data: species classification, height, weight, and
//! multi-page flavor text. Source of truth: `pokemon/{Species}.json`
//! (the `pokedex` block), with the runtime table emitted by
//! `build.rs::generate_pokemon_and_evos_data`.

use crate::species::Species;

#[derive(Debug, Clone, Copy)]
pub struct PokedexEntry {
    pub species: Species,
    pub category: &'static str,
    pub height_feet: u8,
    pub height_inches: u8,
    pub weight_decipounds: u16,
    pub flavor_text_pages: &'static [&'static str],
}

impl PokedexEntry {
    pub fn weight_pounds(&self) -> f32 {
        self.weight_decipounds as f32 / 10.0
    }
}

pub const POKEDEX_ENTRIES: &[PokedexEntry] =
    &include!(concat!(env!("OUT_DIR"), "/pokedex_data_gen.rs"));

pub fn get_pokedex_entry(species: Species) -> Option<&'static PokedexEntry> {
    let idx = species as usize;
    if idx == 0 || idx > POKEDEX_ENTRIES.len() {
        None
    } else {
        Some(&POKEDEX_ENTRIES[idx - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_entries_present_in_dex_order() {
        // Baseline is the original 151; the editor can append extra species
        // (IDs 152+), so only the lower bound is asserted.
        assert!(POKEDEX_ENTRIES.len() >= 151);
        for (i, entry) in POKEDEX_ENTRIES.iter().enumerate() {
            assert_eq!(
                entry.species as usize,
                i + 1,
                "entry {} is out of dex order",
                i + 1
            );
        }
    }

    /// Spot-checks against the original `data/pokemon/dex_entries.asm`:
    /// Bulbasaur — SEED, 2'4", 15.0 lb; Mewtwo — GENETIC, 6'7", 269.0 lb.
    #[test]
    fn spot_check_bulbasaur_and_mewtwo() {
        let bulb = get_pokedex_entry(Species::Bulbasaur).unwrap();
        assert_eq!(bulb.category, "SEED");
        assert_eq!(bulb.height_feet, 2);
        assert_eq!(bulb.height_inches, 4);
        assert_eq!(bulb.weight_decipounds, 150);
        assert!(!bulb.flavor_text_pages.is_empty());

        let mewtwo = get_pokedex_entry(Species::Mewtwo).unwrap();
        assert_eq!(mewtwo.category, "GENETIC");
        assert_eq!(mewtwo.height_feet, 6);
        assert_eq!(mewtwo.height_inches, 7);
        assert_eq!(mewtwo.weight_decipounds, 2690);

        assert!(get_pokedex_entry(Species::None).is_none());
    }
}
