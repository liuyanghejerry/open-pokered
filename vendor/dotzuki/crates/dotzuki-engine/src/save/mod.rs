//! Save system for JRPG engine.
//!
//! Provides a storage-agnostic save/load framework with CRC16 checksum
//! validation. Game-specific data implements the [`SaveData`] trait, while
//! platform backends implement [`SaveStorage`]. The [`SaveManager`] ties
//! them together with multi-slot support.
//!
//! ## Wire format
//!
//! Each save slot stores: `[serialized_game_data][2-byte CRC16 checksum (LE)]`
//!
//! ## Example
//!
//! ```ignore
//! #[derive(Debug, Clone)]
//! struct MySave { name: String, level: u8 }
//!
//! impl SaveData for MySave {
//!     fn serialize(&self) -> Vec<u8> { /* ... */ }
//!     fn deserialize(data: &[u8]) -> Result<Self, SaveError> { /* ... */ }
//!     fn save_size() -> usize { 64 * 1024 }
//! }
//!
//! let storage = Box::new(InMemoryStorage::new());
//! let manager = SaveManager::<MySave>::new(storage);
//! manager.save(SaveSlot::Slot1, &my_data)?;
//! let loaded = manager.load(SaveSlot::Slot1)?;
//! ```

use core::cell::RefCell;

/// Errors that can occur during save/load operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SaveError {
    /// An I/O error occurred in the storage backend.
    #[error("I/O error: {0}")]
    IoError(String),

    /// The stored checksum does not match the computed checksum.
    #[error("invalid checksum — data may be corrupted")]
    InvalidChecksum,

    /// The deserialized data is structurally invalid.
    #[error("invalid data")]
    InvalidData,

    /// The requested save slot is empty (no data stored).
    #[error("slot is empty")]
    SlotEmpty,

    /// The requested save slot is already occupied.
    #[error("slot is full")]
    SlotFull,
}

/// Identifies a save slot.
///
/// Three save slots are supported, matching the classic JRPG convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaveSlot {
    Slot1,
    Slot2,
    Slot3,
}

impl SaveSlot {
    /// Returns the zero-based index of this slot (0, 1, or 2).
    pub fn index(&self) -> usize {
        match self {
            SaveSlot::Slot1 => 0,
            SaveSlot::Slot2 => 1,
            SaveSlot::Slot3 => 2,
        }
    }

    /// Returns all three save slots.
    pub fn all() -> Vec<SaveSlot> {
        vec![SaveSlot::Slot1, SaveSlot::Slot2, SaveSlot::Slot3]
    }
}

/// Platform-specific storage backend for save data.
///
/// Implementations handle the actual reading and writing of bytes to the
/// underlying medium (e.g. file system, browser localStorage, SRAM chip).
/// Takes `&self` so implementations must use interior mutability.
pub trait SaveStorage {
    /// Write data to the given slot index (0, 1, or 2).
    fn write(&self, slot: usize, data: &[u8]) -> Result<(), SaveError>;

    /// Read data from the given slot index.
    ///
    /// Returns `Err(SaveError::SlotEmpty)` if the slot contains no data.
    fn read(&self, slot: usize) -> Result<Vec<u8>, SaveError>;

    /// Returns `true` if the slot contains data.
    fn slot_exists(&self, slot: usize) -> bool;

    /// Delete data from the given slot index.
    fn delete_slot(&self, slot: usize) -> Result<(), SaveError>;
}

/// Game-specific save data that can be serialized, checksummed, and persisted.
///
/// Implementors define how their data is serialized to/from bytes and how
/// large each save slot is. A default CRC16 checksum is provided.
pub trait SaveData: Sized {
    /// Serialize this game data to a byte vector.
    ///
    /// The returned data must NOT include the checksum — that is appended
    /// automatically by [`SaveManager`].
    fn serialize(&self) -> Vec<u8>;

    /// Deserialize game data from a byte slice.
    ///
    /// The input does NOT include the checksum bytes — they are stripped
    /// by [`SaveManager`] before calling this method.
    fn deserialize(data: &[u8]) -> Result<Self, SaveError>;

    /// Compute a CRC16 checksum over the given serialized game data.
    ///
    /// Default implementation uses CRC-16/XMODEM (polynomial `0x1021`,
    /// initial value `0x0000`).
    fn checksum(data: &[u8]) -> u16 {
        crc16_xmodem(data)
    }

    /// Total size in bytes of a single save slot, including the 2-byte checksum.
    ///
    /// This must equal `serialize().len() + 2`.
    fn save_size() -> usize;

    /// Validate a complete save slot payload (game data + 2-byte checksum).
    ///
    /// Default implementation splits off the trailing 2-byte checksum and
    /// compares it against [`checksum`](SaveData::checksum).
    fn validate(data: &[u8]) -> bool {
        if data.len() < 2 {
            return false;
        }
        let (game_data, cksum_bytes) = data.split_at(data.len() - 2);
        let stored_checksum = u16::from_le_bytes([cksum_bytes[0], cksum_bytes[1]]);
        Self::checksum(game_data) == stored_checksum
    }
}

/// Manages save/load operations across multiple slots.
///
/// `S` is the game-specific type implementing [`SaveData`].
pub struct SaveManager<S: SaveData> {
    storage: Box<dyn SaveStorage>,
    _phantom: core::marker::PhantomData<S>,
}

impl<S: SaveData> SaveManager<S> {
    /// Create a new save manager backed by the given storage implementation.
    pub fn new(storage: Box<dyn SaveStorage>) -> Self {
        Self {
            storage,
            _phantom: core::marker::PhantomData,
        }
    }

    /// Save game data to the given slot.
    ///
    /// Serializes the data, appends a checksum, and writes to storage.
    pub fn save(&self, slot: SaveSlot, data: &S) -> Result<(), SaveError> {
        let raw = data.serialize();
        let cksum = S::checksum(&raw);
        let mut payload = raw;
        payload.extend_from_slice(&cksum.to_le_bytes());
        self.storage.write(slot.index(), &payload)
    }

    /// Load game data from the given slot.
    ///
    /// Reads from storage, validates the checksum, and deserializes.
    pub fn load(&self, slot: SaveSlot) -> Result<S, SaveError> {
        let payload = self.storage.read(slot.index())?;
        if !S::validate(&payload) {
            return Err(SaveError::InvalidChecksum);
        }
        let game_data = &payload[..payload.len() - 2];
        S::deserialize(game_data)
    }

    /// List all slots and whether they contain data.
    pub fn list_slots(&self) -> Vec<(SaveSlot, bool)> {
        SaveSlot::all()
            .into_iter()
            .map(|slot| (slot, self.storage.slot_exists(slot.index())))
            .collect()
    }

    /// Delete the save data in the given slot.
    pub fn delete(&self, slot: SaveSlot) -> Result<(), SaveError> {
        self.storage.delete_slot(slot.index())
    }
}

// ---------------------------------------------------------------------------
// CRC16 implementation
// ---------------------------------------------------------------------------

/// CRC-16/XMODEM: polynomial `0x1021`, initial value `0x0000`.
pub fn crc16_xmodem(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

// ---------------------------------------------------------------------------
// In-memory storage (for testing)
// ---------------------------------------------------------------------------

/// An in-memory [`SaveStorage`] implementation backed by `Vec<Option<Vec<u8>>>`.
///
/// Intended for use in tests and as a reference implementation.
pub struct InMemoryStorage {
    slots: RefCell<Vec<Option<Vec<u8>>>>,
}

impl InMemoryStorage {
    /// Create a new in-memory storage with 3 empty slots.
    pub fn new() -> Self {
        Self {
            slots: RefCell::new(vec![None, None, None]),
        }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl SaveStorage for InMemoryStorage {
    fn write(&self, slot: usize, data: &[u8]) -> Result<(), SaveError> {
        let mut slots = self.slots.borrow_mut();
        if slot >= slots.len() {
            return Err(SaveError::IoError(format!(
                "slot index {} out of range (max {})",
                slot,
                slots.len() - 1
            )));
        }
        slots[slot] = Some(data.to_vec());
        Ok(())
    }

    fn read(&self, slot: usize) -> Result<Vec<u8>, SaveError> {
        let slots = self.slots.borrow();
        if slot >= slots.len() {
            return Err(SaveError::IoError(format!(
                "slot index {} out of range (max {})",
                slot,
                slots.len() - 1
            )));
        }
        slots[slot].clone().ok_or(SaveError::SlotEmpty)
    }

    fn slot_exists(&self, slot: usize) -> bool {
        self.slots
            .borrow()
            .get(slot)
            .map(|s| s.is_some())
            .unwrap_or(false)
    }

    fn delete_slot(&self, slot: usize) -> Result<(), SaveError> {
        let mut slots = self.slots.borrow_mut();
        if slot >= slots.len() {
            return Err(SaveError::IoError(format!(
                "slot index {} out of range (max {})",
                slot,
                slots.len() - 1
            )));
        }
        slots[slot] = None;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock save data: player name (up to 16 ASCII bytes), level, gold.
    ///
    /// Wire format: [16 bytes name (zero-padded)][1 byte level][4 bytes gold LE]
    /// Total game data size: 21 bytes. Save slot size: 23 bytes (21 + 2 checksum).
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct MockSave {
        player_name: String,
        level: u8,
        gold: u32,
    }

    impl MockSave {
        const NAME_LEN: usize = 16;
    }

    impl SaveData for MockSave {
        fn serialize(&self) -> Vec<u8> {
            let mut v = Vec::with_capacity(Self::NAME_LEN + 1 + 4);
            let name_bytes = self.player_name.as_bytes();
            let copy_len = name_bytes.len().min(Self::NAME_LEN);
            v.extend_from_slice(&name_bytes[..copy_len]);
            // Zero-pad to NAME_LEN
            v.resize(Self::NAME_LEN, 0);
            v.push(self.level);
            v.extend_from_slice(&self.gold.to_le_bytes());
            v
        }

        fn deserialize(data: &[u8]) -> Result<Self, SaveError> {
            if data.len() < Self::NAME_LEN + 1 + 4 {
                return Err(SaveError::InvalidData);
            }
            let name_bytes = &data[..Self::NAME_LEN];
            // Find the first zero byte as terminator
            let name_end = name_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(Self::NAME_LEN);
            let player_name = String::from_utf8(name_bytes[..name_end].to_vec())
                .map_err(|_| SaveError::InvalidData)?;
            let level = data[Self::NAME_LEN];
            let gold_start = Self::NAME_LEN + 1;
            let gold = u32::from_le_bytes([
                data[gold_start],
                data[gold_start + 1],
                data[gold_start + 2],
                data[gold_start + 3],
            ]);
            Ok(MockSave {
                player_name,
                level,
                gold,
            })
        }

        fn save_size() -> usize {
            // 21 bytes game data + 2 bytes checksum
            Self::NAME_LEN + 1 + 4 + 2
        }
    }

    // -----------------------------------------------------------------------
    // CRC16 smoke test
    // -----------------------------------------------------------------------

    #[test]
    fn test_crc16_known_value() {
        // "123456789" → CRC-16/XMODEM = 0x31C3
        let data = b"123456789";
        assert_eq!(crc16_xmodem(data), 0x31C3);
    }

    #[test]
    fn test_crc16_empty() {
        assert_eq!(crc16_xmodem(b""), 0x0000);
    }

    // -----------------------------------------------------------------------
    // Save / load round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn test_save_and_load_roundtrip() {
        let storage = Box::new(InMemoryStorage::new());
        let manager = SaveManager::<MockSave>::new(storage);

        let original = MockSave {
            player_name: "Ash".to_string(),
            level: 42,
            gold: 9999,
        };

        // Save to slot 1
        manager
            .save(SaveSlot::Slot1, &original)
            .expect("save should succeed");

        // Load from slot 1
        let loaded = manager.load(SaveSlot::Slot1).expect("load should succeed");

        assert_eq!(loaded, original, "loaded data should match original");
    }

    #[test]
    fn test_save_and_load_multiple_slots() {
        let storage = Box::new(InMemoryStorage::new());
        let manager = SaveManager::<MockSave>::new(storage);

        let save1 = MockSave {
            player_name: "Red".to_string(),
            level: 50,
            gold: 5000,
        };
        let save2 = MockSave {
            player_name: "Blue".to_string(),
            level: 48,
            gold: 4800,
        };
        let save3 = MockSave {
            player_name: "Green".to_string(),
            level: 55,
            gold: 5500,
        };

        manager.save(SaveSlot::Slot1, &save1).unwrap();
        manager.save(SaveSlot::Slot2, &save2).unwrap();
        manager.save(SaveSlot::Slot3, &save3).unwrap();

        assert_eq!(manager.load(SaveSlot::Slot1).unwrap(), save1);
        assert_eq!(manager.load(SaveSlot::Slot2).unwrap(), save2);
        assert_eq!(manager.load(SaveSlot::Slot3).unwrap(), save3);
    }

    // -----------------------------------------------------------------------
    // Empty slot error
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_empty_slot_returns_error() {
        let storage = Box::new(InMemoryStorage::new());
        let manager = SaveManager::<MockSave>::new(storage);

        let result = manager.load(SaveSlot::Slot1);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SaveError::SlotEmpty);
    }

    // -----------------------------------------------------------------------
    // Corrupted checksum error
    // -----------------------------------------------------------------------

    #[test]
    fn test_corrupted_data_returns_checksum_error() {
        let storage = Box::new(InMemoryStorage::new());

        // Manually write corrupted data (valid payload but wrong checksum)
        let mock = MockSave {
            player_name: "Ash".to_string(),
            level: 10,
            gold: 100,
        };
        let raw = mock.serialize();
        let mut payload = raw.clone();
        // Append a deliberately wrong checksum
        let wrong_cksum: u16 = 0xFFFF;
        payload.extend_from_slice(&wrong_cksum.to_le_bytes());
        storage.write(0, &payload).unwrap();

        let manager = SaveManager::<MockSave>::new(storage);
        let result = manager.load(SaveSlot::Slot1);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SaveError::InvalidChecksum);
    }

    // -----------------------------------------------------------------------
    // List slots
    // -----------------------------------------------------------------------

    #[test]
    fn test_list_slots() {
        let storage = Box::new(InMemoryStorage::new());
        let manager = SaveManager::<MockSave>::new(storage);

        let slots = manager.list_slots();
        assert_eq!(slots.len(), 3);
        for (_slot, has_data) in &slots {
            assert!(!has_data, "all slots should be empty initially");
        }

        let mock = MockSave {
            player_name: "Test".to_string(),
            level: 1,
            gold: 0,
        };
        manager.save(SaveSlot::Slot2, &mock).unwrap();

        let slots = manager.list_slots();
        assert!(!slots[0].1, "slot 1 should still be empty");
        assert!(slots[1].1, "slot 2 should have data");
        assert!(!slots[2].1, "slot 3 should still be empty");
    }

    // -----------------------------------------------------------------------
    // Delete slot
    // -----------------------------------------------------------------------

    #[test]
    fn test_delete_slot() {
        let storage = Box::new(InMemoryStorage::new());
        let manager = SaveManager::<MockSave>::new(storage);

        let mock = MockSave {
            player_name: "Del".to_string(),
            level: 7,
            gold: 77,
        };
        manager.save(SaveSlot::Slot1, &mock).unwrap();
        assert!(manager.list_slots()[0].1, "slot 1 should have data");

        manager.delete(SaveSlot::Slot1).unwrap();
        assert!(
            !manager.list_slots()[0].1,
            "slot 1 should be empty after delete"
        );

        let result = manager.load(SaveSlot::Slot1);
        assert_eq!(result.unwrap_err(), SaveError::SlotEmpty);
    }

    // -----------------------------------------------------------------------
    // SaveSlot enum
    // -----------------------------------------------------------------------

    #[test]
    fn test_save_slot_indices() {
        assert_eq!(SaveSlot::Slot1.index(), 0);
        assert_eq!(SaveSlot::Slot2.index(), 1);
        assert_eq!(SaveSlot::Slot3.index(), 2);
    }

    #[test]
    fn test_save_slot_all() {
        let all = SaveSlot::all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0], SaveSlot::Slot1);
        assert_eq!(all[1], SaveSlot::Slot2);
        assert_eq!(all[2], SaveSlot::Slot3);
    }

    // -----------------------------------------------------------------------
    // Save overwrite
    // -----------------------------------------------------------------------

    #[test]
    fn test_overwrite_slot() {
        let storage = Box::new(InMemoryStorage::new());
        let manager = SaveManager::<MockSave>::new(storage);

        let first = MockSave {
            player_name: "First".to_string(),
            level: 10,
            gold: 100,
        };
        manager.save(SaveSlot::Slot1, &first).unwrap();

        let second = MockSave {
            player_name: "Second".to_string(),
            level: 20,
            gold: 200,
        };
        manager.save(SaveSlot::Slot1, &second).unwrap();

        let loaded = manager.load(SaveSlot::Slot1).unwrap();
        assert_eq!(loaded, second, "overwritten slot should return new data");
        assert_ne!(loaded, first);
    }
}
