// Pokémon-specific event flag extensions.
//
// The base `EventFlags` struct lives in `dotzuki-engine` as a generic
// string-keyed flag system. This module re‑exports it and adds Pokémon‑specific
// methods that depend on `pokered_data::event_flags::{EventFlag, EVENT_FLAGS_SIZE}`.

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use dotzuki_engine::overworld::event_flags::EventFlags as GenericEventFlags;
use pokered_data::event_flags::{EventFlag, EVENT_FLAGS_SIZE};

/// Pokémon‑specific event flag container.
///
/// Wraps the generic `EventFlags` from `jrpg‑engine` and adds methods
/// that operate directly on `EventFlag` typed constants and the original
/// game's bit‑packed serialization format.
#[derive(Debug, Clone)]
pub struct EventFlags(GenericEventFlags);

// ── Construction (delegated) ─────────────────────────────────────────

impl EventFlags {
    /// Creates a new `EventFlags` with no flags set.
    #[inline]
    pub fn new() -> Self {
        Self(GenericEventFlags::new())
    }

    /// Creates an `EventFlags` from an existing `HashMap`.
    pub fn from_hashmap(map: &HashMap<String, bool>) -> Self {
        Self(GenericEventFlags::from_hashmap(map))
    }
}

impl Default for EventFlags {
    fn default() -> Self {
        Self::new()
    }
}

// ── Deref / DerefMut → access to generic methods ───────────────────

impl Deref for EventFlags {
    type Target = GenericEventFlags;

    #[inline]
    fn deref(&self) -> &GenericEventFlags {
        &self.0
    }
}

impl DerefMut for EventFlags {
    #[inline]
    fn deref_mut(&mut self) -> &mut GenericEventFlags {
        &mut self.0
    }
}

// ── Pokémon‑specific flag methods ──────────────────────────────────

impl EventFlags {
    /// Returns whether the given typed event flag is set.
    #[inline]
    pub fn check(&self, flag: EventFlag) -> bool {
        self.0.get_flag(flag.name())
    }

    /// Sets a typed event flag.
    #[inline]
    pub fn set(&mut self, flag: EventFlag) {
        self.0.set_flag(flag.name(), true);
    }

    /// Clears (resets) a typed event flag.
    #[inline]
    pub fn reset(&mut self, flag: EventFlag) {
        self.0.set_flag(flag.name(), false);
    }

    /// Checks a flag by its original game bit index.
    ///
    /// First tries to find a matching `EventFlag` by bit index; if none
    /// is found, falls back to a raw `__RAW_BIT_{index}` key.
    #[inline]
    pub fn check_raw(&self, bit_index: u16) -> bool {
        for ef in EventFlag::ALL {
            if ef.bit_index() == bit_index {
                return self.0.get_flag(ef.name());
            }
        }
        let key = format!("__RAW_BIT_{}", bit_index);
        self.0.get_flag(&key)
    }

    /// Sets a flag by its original game bit index.
    #[inline]
    pub fn set_raw(&mut self, bit_index: u16) {
        for ef in EventFlag::ALL {
            if ef.bit_index() == bit_index {
                self.0.set_flag(ef.name(), true);
                return;
            }
        }
        let key = format!("__RAW_BIT_{}", bit_index);
        self.0.set_flag(&key, true);
    }

    /// Resets a flag by its original game bit index.
    #[inline]
    pub fn reset_raw(&mut self, bit_index: u16) {
        for ef in EventFlag::ALL {
            if ef.bit_index() == bit_index {
                self.0.set_flag(ef.name(), false);
                return;
            }
        }
        let key = format!("__RAW_BIT_{}", bit_index);
        self.0.set_flag(&key, false);
    }

    /// Serializes all known `EventFlag` constants into the original game's
    /// bit‑packed byte array format (`EVENT_FLAGS_SIZE` bytes).
    pub fn to_event_bytes(&self) -> [u8; EVENT_FLAGS_SIZE] {
        let mut data = [0u8; EVENT_FLAGS_SIZE];
        for &ef in EventFlag::ALL {
            if self.0.get_flag(ef.name()) {
                let byte = ef.byte_offset();
                if byte < EVENT_FLAGS_SIZE {
                    data[byte] |= ef.bit_mask();
                }
            }
        }
        data
    }

    /// Loads flag values from the original game's bit‑packed byte array.
    pub fn load_event_bytes(&mut self, data: &[u8]) {
        for &ef in EventFlag::ALL {
            let byte = ef.byte_offset();
            if byte < data.len() && (data[byte] & ef.bit_mask()) != 0 {
                self.0.set_flag(ef.name(), true);
            }
        }
    }

    /// Alias for `to_event_bytes` — serializes to the original game format.
    #[inline]
    pub fn as_bytes(&self) -> [u8; EVENT_FLAGS_SIZE] {
        self.to_event_bytes()
    }

    /// Deserializes from the original game's bit‑packed byte array.
    pub fn from_bytes(data: [u8; EVENT_FLAGS_SIZE]) -> Self {
        let mut ef = Self::new();
        ef.load_event_bytes(&data);
        ef
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_data::event_flags::EventFlag;

    #[test]
    fn new_all_cleared() {
        let flags = EventFlags::new();
        assert_eq!(flags.count_set(), 0);
    }

    #[test]
    fn default_same_as_new() {
        let a = EventFlags::new();
        let b = EventFlags::default();
        assert_eq!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn set_and_check() {
        let mut flags = EventFlags::new();
        let flag = EventFlag::EVENT_GOT_STARTER;
        assert!(!flags.check(flag));
        flags.set(flag);
        assert!(flags.check(flag));
    }

    #[test]
    fn reset() {
        let mut flags = EventFlags::new();
        let flag = EventFlag::EVENT_GOT_STARTER;
        flags.set(flag);
        assert!(flags.check(flag));
        flags.reset(flag);
        assert!(!flags.check(flag));
    }

    #[test]
    fn multiple_independent() {
        let mut flags = EventFlags::new();
        let f1 = EventFlag::EVENT_GOT_STARTER;
        let f2 = EventFlag::EVENT_BEAT_BROCK;
        flags.set(f1);
        assert!(flags.check(f1));
        assert!(!flags.check(f2));
        flags.set(f2);
        assert!(flags.check(f1));
        assert!(flags.check(f2));
        flags.reset(f1);
        assert!(!flags.check(f1));
        assert!(flags.check(f2));
    }

    #[test]
    fn clear_all() {
        let mut flags = EventFlags::new();
        flags.set(EventFlag::EVENT_GOT_STARTER);
        flags.set(EventFlag::EVENT_BEAT_BROCK);
        assert_eq!(flags.count_set(), 2);
        flags.clear_all();
        assert_eq!(flags.count_set(), 0);
        assert!(!flags.check(EventFlag::EVENT_GOT_STARTER));
    }

    #[test]
    fn raw_operations() {
        let mut flags = EventFlags::new();
        // Bit 0
        assert!(!flags.check_raw(0));
        flags.set_raw(0);
        assert!(flags.check_raw(0));
        flags.reset_raw(0);
        assert!(!flags.check_raw(0));
        // High bit
        flags.set_raw(100);
        assert!(flags.check_raw(100));
        assert!(!flags.check_raw(99));
        assert!(!flags.check_raw(101));
    }

    #[test]
    fn raw_out_of_bounds_safe() {
        let mut flags = EventFlags::new();
        flags.set_raw(0xFFFF);
        assert!(flags.check_raw(0xFFFF));
        flags.reset_raw(0xFFFF);
        assert!(!flags.check_raw(0xFFFF));
    }

    #[test]
    fn save_load_roundtrip() {
        let mut flags = EventFlags::new();
        flags.set(EventFlag::EVENT_GOT_STARTER);
        flags.set(EventFlag::EVENT_BEAT_BROCK);
        flags.set(EventFlag::EVENT_BEAT_MISTY);
        let bytes = flags.as_bytes();
        let loaded = EventFlags::from_bytes(bytes);
        assert!(loaded.check(EventFlag::EVENT_GOT_STARTER));
        assert!(loaded.check(EventFlag::EVENT_BEAT_BROCK));
        assert!(loaded.check(EventFlag::EVENT_BEAT_MISTY));
        assert!(!loaded.check(EventFlag::EVENT_BEAT_LT_SURGE));
        assert_eq!(loaded.count_set(), 3);
    }

    #[test]
    fn merge_from() {
        let mut flags = EventFlags::new();
        let mut map = HashMap::new();
        map.insert("test_flag".to_owned(), true);
        flags.merge_from(&map);
        assert!(flags.get_flag("test_flag"));
    }

    #[test]
    fn get_flag_generic_access() {
        let mut flags = EventFlags::new();
        flags.set_flag("generic_key", true);
        assert!(flags.get_flag("generic_key"));
        flags.set_flag("generic_key", false);
        assert!(!flags.get_flag("generic_key"));
        flags.remove_flag("generic_key");
        assert!(!flags.get_flag("generic_key"));
    }

    #[test]
    fn from_hashmap() {
        let mut map = HashMap::new();
        map.insert("a".to_owned(), true);
        map.insert("b".to_owned(), false);
        let flags = EventFlags::from_hashmap(&map);
        assert!(flags.get_flag("a"));
        assert!(!flags.get_flag("b"));
        assert_eq!(flags.to_hashmap().len(), 2);
    }

    #[test]
    fn to_event_bytes_pads_correctly() {
        let flags = EventFlags::new();
        let bytes = flags.to_event_bytes();
        assert_eq!(bytes.len(), EVENT_FLAGS_SIZE);
    }
}
