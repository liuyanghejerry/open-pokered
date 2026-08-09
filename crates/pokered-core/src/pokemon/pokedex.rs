use pokered_data::species::Species;
use serde::{Deserialize, Serialize};

pub const NUM_POKEMON: usize = 151;
const FLAG_BYTES: usize = (NUM_POKEMON + 7) / 8;

/// Gen1 Pokédex: two bit arrays (seen/owned) for 151 species.
/// Matches wPokedexSeen/wPokedexOwned in wram.asm (flag_array NUM_POKEMON = 19 bytes each).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pokedex {
    seen: [u8; FLAG_BYTES],
    owned: [u8; FLAG_BYTES],
}

fn species_to_bit(species: Species) -> Option<(usize, u8)> {
    let dex = species as u8;
    if dex == 0 || dex as usize > NUM_POKEMON {
        return None;
    }
    let index = (dex - 1) as usize;
    Some((index / 8, 1 << (index % 8)))
}

impl Pokedex {
    pub fn new() -> Self {
        Self {
            seen: [0; FLAG_BYTES],
            owned: [0; FLAG_BYTES],
        }
    }

    pub fn from_flags(owned: &[u8], seen: &[u8]) -> Self {
        let mut dex = Self::new();
        let len = FLAG_BYTES.min(owned.len());
        dex.owned[..len].copy_from_slice(&owned[..len]);
        let len = FLAG_BYTES.min(seen.len());
        dex.seen[..len].copy_from_slice(&seen[..len]);
        dex
    }

    pub fn set_seen(&mut self, species: Species) {
        if let Some((byte, bit)) = species_to_bit(species) {
            self.seen[byte] |= bit;
        }
    }

    pub fn set_owned(&mut self, species: Species) {
        if let Some((byte, bit)) = species_to_bit(species) {
            self.owned[byte] |= bit;
            self.seen[byte] |= bit;
        }
    }

    pub fn is_seen(&self, species: Species) -> bool {
        species_to_bit(species)
            .map(|(byte, bit)| self.seen[byte] & bit != 0)
            .unwrap_or(false)
    }

    pub fn is_owned(&self, species: Species) -> bool {
        species_to_bit(species)
            .map(|(byte, bit)| self.owned[byte] & bit != 0)
            .unwrap_or(false)
    }

    pub fn seen_count(&self) -> u32 {
        self.seen.iter().map(|b| b.count_ones()).sum()
    }

    pub fn owned_count(&self) -> u32 {
        self.owned.iter().map(|b| b.count_ones()).sum()
    }

    pub fn reset_seen(&mut self, species: Species) {
        if let Some((byte, bit)) = species_to_bit(species) {
            self.seen[byte] &= !bit;
        }
    }

    pub fn reset_owned(&mut self, species: Species) {
        if let Some((byte, bit)) = species_to_bit(species) {
            self.owned[byte] &= !bit;
        }
    }

    pub fn seen_flags(&self) -> &[u8; FLAG_BYTES] {
        &self.seen
    }

    pub fn owned_flags(&self) -> &[u8; FLAG_BYTES] {
        &self.owned
    }

    pub fn is_complete(&self) -> bool {
        self.owned_count() == NUM_POKEMON as u32
    }
}

impl Default for Pokedex {
    fn default() -> Self {
        Self::new()
    }
}
