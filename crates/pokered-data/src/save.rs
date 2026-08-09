//! Raw SRAM save data container for Pokémon Red/Blue.
//!
//! `PokemonSaveData` wraps the complete 32 KiB Game Boy SRAM dump in its
//! original byte format, including the internal XOR checksums. It implements
//! the [`jrpg_engine::save::SaveData`] trait so that the engine-level
//! [`SaveManager`] can persist it through any [`SaveStorage`] backend.

/// Total size of a Game Boy .sav file (4 banks × 8 KiB).
pub const SAV_FILE_SIZE: usize = 0x8000;

/// Raw SRAM save data for Pokémon Red/Blue.
///
/// This is a byte-level container that holds the complete Game Boy SRAM
/// dump in the original layout (bank 0 = sprite buffers + Hall of Fame,
/// bank 1 = main save data + checksum, banks 2–3 = PC boxes + checksums).
///
/// Conversion to/from the rich [`SaveData`] struct (with parsed parties,
/// PC boxes, game data, etc.) is handled by the `pokered-core` crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PokemonSaveData {
    pub bytes: Vec<u8>,
}

impl PokemonSaveData {
    /// Create a new zero-filled save data block.
    pub fn new() -> Self {
        Self {
            bytes: vec![0u8; SAV_FILE_SIZE],
        }
    }

    /// Wrap an existing byte vector as save data.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Consume this wrapper and return the inner bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Return a reference to the inner bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl Default for PokemonSaveData {
    fn default() -> Self {
        Self::new()
    }
}
