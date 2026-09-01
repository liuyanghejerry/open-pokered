//! Pokedex display data: species classification, height, weight, and
//! multi-page flavor text. Source of truth: `pokemon/{Species}.json`
//! (the `pokedex` block), with the runtime table emitted by
//! `build.rs::generate_pokemon_and_evos_data`.

use crate::species::Species;

#[derive(Debug, Clone, Copy)]
pub struct PokedexEntry {
    pub species: Species,
    pub category: &'static str,
    /// Simplified-Chinese classification (e.g. 种子宝可梦). Falls back to the
    /// English `category` at build time when a species has no `categoryZh`.
    pub category_zh: &'static str,
    pub height_feet: u8,
    pub height_inches: u8,
    pub weight_decipounds: u16,
    pub flavor_text_pages: &'static [&'static str],
    /// Simplified-Chinese flavor text, one string per `flavorTextPages`
    /// entry (same order). Falls back to the English pages at build time.
    pub flavor_text_pages_zh: &'static [&'static str],
}

impl PokedexEntry {
    pub fn weight_pounds(&self) -> f32 {
        self.weight_decipounds as f32 / 10.0
    }

    /// Category text for the display language (`is_zh` = Simplified Chinese).
    pub fn category_for(&self, is_zh: bool) -> &'static str {
        if is_zh {
            self.category_zh
        } else {
            self.category
        }
    }

    /// Flavor-text pages for the display language (`is_zh` = Simplified
    /// Chinese). Pages keep EN/ZH parity one-to-one.
    pub fn flavor_text_pages_for(&self, is_zh: bool) -> &'static [&'static str] {
        if is_zh {
            self.flavor_text_pages_zh
        } else {
            self.flavor_text_pages
        }
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

    /// Every canonical species carries Simplified-Chinese dex data
    /// (`tools/gen_pokedex_zh.py` injects `categoryZh`/`flavorTextPagesZh`
    /// with a 151/151 coverage check). Categories are at most 7 CJK chars so
    /// they fit the 18-tile entry box when the label is shifted left from
    /// the fixed x=8 position (7 × 2 = 14 tiles).
    #[test]
    fn canonical_151_have_chinese_dex_text() {
        assert!(POKEDEX_ENTRIES.len() >= 151);
        for entry in &POKEDEX_ENTRIES[..151] {
            let name = format!("{:?}", entry.species);
            assert!(
                entry.category_zh.chars().any(|c| c as u32 > 0x2E80),
                "{}: zh category is not Chinese: {:?}",
                name,
                entry.category_zh
            );
            assert!(
                entry.category_zh.chars().count() <= 7,
                "{}: zh category {} too wide for the entry box",
                name,
                entry.category_zh
            );
            assert_eq!(
                entry.flavor_text_pages_zh.len(),
                entry.flavor_text_pages.len(),
                "{}: zh/en flavor page count mismatch",
                name
            );
            for (i, page) in entry.flavor_text_pages_zh.iter().enumerate() {
                assert!(
                    !page.trim().is_empty() && page.chars().any(|c| c as u32 > 0x2E80),
                    "{}: zh flavor page {} is empty or not Chinese",
                    name,
                    i
                );
            }
        }
    }

    /// Bulbasaur's zh data spot check: 种子宝可梦, two translated pages.
    #[test]
    fn spot_check_chinese_bulbasaur() {
        let bulb = get_pokedex_entry(Species::Bulbasaur).unwrap();
        assert_eq!(bulb.category_for(true), "种子宝可梦");
        assert_eq!(bulb.category_for(false), "SEED");
        assert_eq!(
            bulb.flavor_text_pages_for(true).len(),
            bulb.flavor_text_pages_for(false).len()
        );
        assert!(bulb.flavor_text_pages_for(true)[0].contains("种子"));
    }
}
