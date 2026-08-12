// Pokémon-specific event flag storage.
//
// The base `EventFlags` struct in `dotzuki-engine` is a generic string-keyed
// `HashMap` flag system. Pokémon does not need that generality: the original
// game stores every event flag as a single bit in the fixed `wEventFlags`
// array (`flag_array NUM_EVENTS`, `NUM_EVENTS = $A00` bits → 320 bytes).
// This module replaces the heap map with that exact fixed bit array.
//
// The only state that cannot live in the bit array is runtime-only dynamic
// keys with no `EventFlag` constant and no `__RAW_BIT_n` form — e.g. the
// `__OBJ_HIDDEN_*` / `__OBJ_SHOWN_*` NPC-visibility keys written by script
// effects. Those are kept in a small `extras` map (usually empty).

use std::collections::HashMap;

use pokered_data::event_flags::{EventFlag, EVENT_FLAGS_SIZE};

/// Pokémon‑specific event flag container.
///
/// Backed by a fixed `[u8; EVENT_FLAGS_SIZE]` bit array in the original
/// game's layout (bit index `n` lives in byte `n / 8`, bit `n % 8`), plus
/// a small `extras` map for dynamic keys that have no bit representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventFlags {
    bits: [u8; EVENT_FLAGS_SIZE],
    extras: HashMap<String, bool>,
}

/// Resolve a string flag name to a raw bit index, if it names an `EventFlag`
/// constant or uses the `__RAW_BIT_{index}` form.
#[inline]
fn bit_for_name(name: &str) -> Option<u16> {
    if let Some(flag) = EventFlag::from_name(name) {
        return Some(flag.bit_index());
    }
    if let Some(suffix) = name.strip_prefix("__RAW_BIT_") {
        if let Ok(index) = suffix.parse::<u16>() {
            return Some(index);
        }
    }
    None
}

impl EventFlags {
    /// Creates a new `EventFlags` with no flags set.
    #[inline]
    pub fn new() -> Self {
        Self {
            bits: [0u8; EVENT_FLAGS_SIZE],
            extras: HashMap::new(),
        }
    }

    /// Creates an `EventFlags` from an existing `HashMap`, routing each key
    /// to its bit (named / `__RAW_BIT_n`) or to the extras map.
    pub fn from_hashmap(map: &HashMap<String, bool>) -> Self {
        let mut ef = Self::new();
        ef.merge_from(map);
        ef
    }

    /// Direct bit read; out-of-range bits read as `false`.
    #[inline]
    fn get_bit(&self, bit_index: u16) -> bool {
        let byte = bit_index as usize / 8;
        if byte >= self.bits.len() {
            return false;
        }
        (self.bits[byte] & (1 << (bit_index % 8))) != 0
    }

    /// Direct bit write; out-of-range bits are ignored.
    #[inline]
    fn set_bit(&mut self, bit_index: u16, value: bool) {
        let byte = bit_index as usize / 8;
        if byte >= self.bits.len() {
            return;
        }
        let mask = 1 << (bit_index % 8);
        if value {
            self.bits[byte] |= mask;
        } else {
            self.bits[byte] &= !mask;
        }
    }
}

impl Default for EventFlags {
    fn default() -> Self {
        Self::new()
    }
}

// ── Pokémon‑specific typed flag methods ─────────────────────────────

impl EventFlags {
    /// Returns whether the given typed event flag is set.
    #[inline]
    pub fn check(&self, flag: EventFlag) -> bool {
        self.get_bit(flag.bit_index())
    }

    /// Sets a typed event flag.
    #[inline]
    pub fn set(&mut self, flag: EventFlag) {
        self.set_bit(flag.bit_index(), true);
    }

    /// Clears (resets) a typed event flag.
    #[inline]
    pub fn reset(&mut self, flag: EventFlag) {
        self.set_bit(flag.bit_index(), false);
    }

    /// Checks a flag by its original game bit index.
    #[inline]
    pub fn check_raw(&self, bit_index: u16) -> bool {
        self.get_bit(bit_index)
    }

    /// Sets a flag by its original game bit index.
    #[inline]
    pub fn set_raw(&mut self, bit_index: u16) {
        self.set_bit(bit_index, true);
    }

    /// Resets a flag by its original game bit index.
    #[inline]
    pub fn reset_raw(&mut self, bit_index: u16) {
        self.set_bit(bit_index, false);
    }

    /// Returns the number of flags that are currently set to `true`
    /// (named bits, raw bits, and extras).
    pub fn count_set(&self) -> u32 {
        let bit_count: u32 = self.bits.iter().map(|b| b.count_ones()).sum();
        let extra_count = self.extras.values().filter(|&&v| v).count() as u32;
        bit_count + extra_count
    }

    /// Clears all flags (bits and extras).
    pub fn clear_all(&mut self) {
        self.bits = [0u8; EVENT_FLAGS_SIZE];
        self.extras.clear();
    }

    /// Returns the value of a named flag, or `false` if unset. Resolves
    /// `EventFlag` constant names and `__RAW_BIT_{index}` keys against the
    /// bit array; any other name reads the extras map.
    #[inline]
    pub fn get_flag(&self, name: &str) -> bool {
        match bit_for_name(name) {
            Some(bit) => self.get_bit(bit),
            None => self.extras.get(name).copied().unwrap_or(false),
        }
    }

    /// Sets a named flag to a specific value. Named / `__RAW_BIT_n` keys
    /// write the bit array; any other name goes into the extras map.
    #[inline]
    pub fn set_flag(&mut self, name: &str, value: bool) {
        match bit_for_name(name) {
            Some(bit) => self.set_bit(bit, value),
            None => {
                self.extras.insert(name.to_owned(), value);
            }
        }
    }

    /// Removes a named flag entirely (clears its bit / removes the extra).
    #[inline]
    pub fn remove_flag(&mut self, name: &str) {
        match bit_for_name(name) {
            Some(bit) => self.set_bit(bit, false),
            None => {
                self.extras.remove(name);
            }
        }
    }

    /// Returns an iterator over all set named flags and extras pairs
    /// (`(name, value)`). Unnamed raw bits are not expanded here — use
    /// `to_hashmap` for a full name→value snapshot.
    pub fn iter(&self) -> impl Iterator<Item = (&str, bool)> + '_ {
        EventFlag::ALL
            .iter()
            .filter(move |ef| self.get_bit(ef.bit_index()))
            .map(|ef| (ef.name(), true))
            .chain(self.extras.iter().map(|(k, v)| (k.as_str(), *v)))
    }

    /// Merges entries from another `HashMap` into this flag set.
    /// Existing keys are overwritten.
    pub fn merge_from(&mut self, other: &HashMap<String, bool>) {
        for (k, &v) in other {
            self.set_flag(k, v);
        }
    }

    /// Clones all flags (bits + extras) into a name→value `HashMap` for
    /// external use (e.g. seeding the script engine's flag store). Named
    /// bits use their `EventFlag` name; unnamed set bits use `__RAW_BIT_n`.
    pub fn to_hashmap(&self) -> HashMap<String, bool> {
        let mut map = HashMap::new();
        for &ef in EventFlag::ALL {
            if self.get_bit(ef.bit_index()) {
                map.insert(ef.name().to_owned(), true);
            }
        }
        for byte in 0..self.bits.len() {
            let mut value = self.bits[byte];
            while value != 0 {
                let bit_in_byte = value.trailing_zeros() as u16;
                value &= value - 1;
                let bit_index = (byte as u16) * 8 + bit_in_byte;
                if !EventFlag::ALL.iter().any(|ef| ef.bit_index() == bit_index) {
                    map.insert(format!("__RAW_BIT_{}", bit_index), true);
                }
            }
        }
        for (k, &v) in &self.extras {
            map.insert(k.clone(), v);
        }
        map
    }

    /// Shared access to the extras map (runtime-only dynamic keys that have
    /// no bit representation, e.g. `__OBJ_HIDDEN_*`). Used to persist them
    /// outside the fixed SRAM region.
    pub fn extras(&self) -> &HashMap<String, bool> {
        &self.extras
    }

    /// Serializes all flags into the original game's bit‑packed byte array
    /// format (`EVENT_FLAGS_SIZE` bytes — the full 320-byte `wEventFlags`).
    #[inline]
    pub fn to_event_bytes(&self) -> [u8; EVENT_FLAGS_SIZE] {
        self.bits
    }

    /// Alias for `to_event_bytes` — serializes to the original game format.
    #[inline]
    pub fn as_bytes(&self) -> [u8; EVENT_FLAGS_SIZE] {
        self.bits
    }

    /// Replaces the flag bits from the original game's bit‑packed byte
    /// array. Shorter inputs only fill the leading bytes; longer inputs are
    /// truncated to the array size. Extras are left untouched.
    pub fn load_event_bytes(&mut self, data: &[u8]) {
        for (dst, &src) in self.bits.iter_mut().zip(data) {
            *dst = src;
        }
    }

    /// Deserializes from the original game's bit‑packed byte array.
    pub fn from_bytes(data: [u8; EVENT_FLAGS_SIZE]) -> Self {
        Self {
            bits: data,
            extras: HashMap::new(),
        }
    }

    /// Deserializes from a byte slice (tolerant of any length).
    pub fn from_event_bytes(data: &[u8]) -> Self {
        let mut ef = Self::new();
        ef.load_event_bytes(data);
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
        flags.set_flag("extra_key", true);
        assert_eq!(flags.count_set(), 3);
        flags.clear_all();
        assert_eq!(flags.count_set(), 0);
        assert!(!flags.check(EventFlag::EVENT_GOT_STARTER));
        assert!(!flags.get_flag("extra_key"));
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
    fn raw_out_of_bounds_are_noops() {
        let mut flags = EventFlags::new();
        // Bits beyond the 320-byte array (2560 bits) are ignored, like the
        // original's wEventFlags indexing never touches them.
        flags.set_raw(0xFFFF);
        assert!(!flags.check_raw(0xFFFF));
        flags.set_raw(2559);
        assert!(flags.check_raw(2559));
        flags.reset_raw(2559);
        assert!(!flags.check_raw(2559));
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
    fn raw_bits_roundtrip_through_bytes() {
        // Unnamed bits must survive the byte-array round trip (the original
        // SRAM layout stores them whether or not a constant names them).
        let mut flags = EventFlags::new();
        flags.set_raw(1);
        flags.set_raw(2522); // EVENT_BEAT_ARTICUNO's bit
        let bytes = flags.as_bytes();
        let loaded = EventFlags::from_bytes(bytes);
        assert!(loaded.check_raw(1));
        assert!(loaded.check_raw(2522));
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
    fn named_and_raw_string_access() {
        let mut flags = EventFlags::new();
        // Named via string
        flags.set_flag("EVENT_GOT_STARTER", true);
        assert!(flags.get_flag("EVENT_GOT_STARTER"));
        assert!(flags.check(EventFlag::EVENT_GOT_STARTER));
        // Raw via string
        flags.set_flag("__RAW_BIT_100", true);
        assert!(flags.get_flag("__RAW_BIT_100"));
        assert!(flags.check_raw(100));
        assert!(!flags.check_raw(99));
        flags.remove_flag("__RAW_BIT_100");
        assert!(!flags.check_raw(100));
    }

    #[test]
    fn from_hashmap() {
        let mut map = HashMap::new();
        map.insert("EVENT_GOT_STARTER".to_owned(), true);
        map.insert("b".to_owned(), false);
        let flags = EventFlags::from_hashmap(&map);
        assert!(flags.get_flag("EVENT_GOT_STARTER"));
        assert!(!flags.get_flag("b"));
        assert_eq!(flags.to_hashmap().len(), 2);
    }

    #[test]
    fn to_hashmap_includes_named_raw_and_extras() {
        let mut flags = EventFlags::new();
        flags.set(EventFlag::EVENT_GOT_STARTER);
        flags.set_raw(1); // unnamed bit (bit 3 is EVENT_HALL_OF_FAME_DEX_RATING)
        flags.set_flag("__OBJ_HIDDEN_x", true);
        let map = flags.to_hashmap();
        assert_eq!(map.get("EVENT_GOT_STARTER"), Some(&true));
        assert_eq!(map.get("__RAW_BIT_1"), Some(&true));
        assert_eq!(map.get("__OBJ_HIDDEN_x"), Some(&true));
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn iter_yields_named_and_extras() {
        let mut flags = EventFlags::new();
        flags.set(EventFlag::EVENT_GOT_STARTER);
        flags.set_flag("__OBJ_HIDDEN_x", true);
        let pairs: Vec<(&str, bool)> = flags.iter().collect();
        assert!(pairs.contains(&("EVENT_GOT_STARTER", true)));
        assert!(pairs.contains(&("__OBJ_HIDDEN_x", true)));
    }

    #[test]
    fn to_event_bytes_pads_correctly() {
        let flags = EventFlags::new();
        let bytes = flags.to_event_bytes();
        assert_eq!(bytes.len(), EVENT_FLAGS_SIZE);
    }

    #[test]
    fn load_event_bytes_replaces_bits() {
        let mut data = [0u8; EVENT_FLAGS_SIZE];
        let flag = EventFlag::EVENT_GOT_STARTER;
        data[flag.byte_offset()] = flag.bit_mask();
        let mut flags = EventFlags::new();
        flags.load_event_bytes(&data);
        assert!(flags.check(flag));
        // Shorter slices only fill the leading bytes: the same flag's bit
        // (byte 4) survives a 4-byte load untouched.
        let short = [0u8; 4];
        flags.load_event_bytes(&short);
        assert!(flags.check(flag));
    }

    #[test]
    fn extras_are_outside_the_byte_array() {
        let mut flags = EventFlags::new();
        flags.set_flag("__OBJ_HIDDEN_BILLS_HOUSE_OBJ_1", true);
        let bytes = flags.as_bytes();
        // Extras live outside the fixed SRAM region and are not serialized.
        let loaded = EventFlags::from_bytes(bytes);
        assert!(!loaded.get_flag("__OBJ_HIDDEN_BILLS_HOUSE_OBJ_1"));
        // But they are part of the container itself (clone/session lifetime).
        let cloned = flags.clone();
        assert!(cloned.get_flag("__OBJ_HIDDEN_BILLS_HOUSE_OBJ_1"));
    }
}
