use crate::hash::HashMap;

/// A generic, string-keyed event flag system.
///
/// Stores boolean flags keyed by string name. This is the foundation for
/// game-event tracking (story progression, item pickups, trainer defeats)
/// in any JRPG. The struct is `#[non_exhaustive]` to allow game-specific
/// extensions in downstream crates.
///
/// # Design
///
/// - Flags are stored as `HashMap<String, bool>` — one entry per flag name.
/// - The string API (`get_flag`, `set_flag`, `remove_flag`) works with any
///   flag name, enabling game-specific typed wrappers.
/// - All methods are pure data manipulation — no I/O, no platform code.
///
/// # Example
///
/// ```ignore
/// let mut flags = EventFlags::new();
/// flags.set_flag("player_has_sword", true);
/// assert!(flags.get_flag("player_has_sword"));
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct EventFlags {
    flags: HashMap<String, bool>,
}

impl Default for EventFlags {
    fn default() -> Self {
        Self::new()
    }
}

impl EventFlags {
    /// Creates a new `EventFlags` with no flags set.
    #[inline]
    pub fn new() -> Self {
        Self {
            flags: HashMap::default(),
        }
    }

    /// Returns the number of flags that are currently set to `true`.
    pub fn count_set(&self) -> u32 {
        self.flags.values().filter(|&&v| v).count() as u32
    }

    /// Clears all flags.
    pub fn clear_all(&mut self) {
        self.flags.clear();
    }

    /// Returns the value of a named flag, or `false` if unset.
    #[inline]
    pub fn get_flag(&self, name: &str) -> bool {
        self.flags.get(name).copied().unwrap_or(false)
    }

    /// Sets a named flag to a specific value.
    #[inline]
    pub fn set_flag(&mut self, name: &str, value: bool) {
        self.flags.insert(name.to_owned(), value);
    }

    /// Removes a named flag entirely.
    #[inline]
    pub fn remove_flag(&mut self, name: &str) {
        self.flags.remove(name);
    }

    /// Returns an iterator over all `(name, value)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &bool)> {
        self.flags.iter()
    }

    /// Returns a shared reference to the underlying `HashMap`.
    pub fn as_hashmap(&self) -> &HashMap<String, bool> {
        &self.flags
    }

    /// Returns a mutable reference to the underlying `HashMap`.
    pub fn as_hashmap_mut(&mut self) -> &mut HashMap<String, bool> {
        &mut self.flags
    }

    /// Merges entries from another `HashMap` into this flag set.
    /// Existing keys are overwritten.
    pub fn merge_from(&mut self, other: &HashMap<String, bool>) {
        for (k, &v) in other {
            self.flags.insert(k.clone(), v);
        }
    }

    /// Clones the underlying `HashMap` for external use.
    pub fn to_hashmap(&self) -> HashMap<String, bool> {
        self.flags.clone()
    }

    /// Creates an `EventFlags` from an existing `HashMap`.
    pub fn from_hashmap(map: &HashMap<String, bool>) -> Self {
        let mut ef = Self::new();
        ef.merge_from(map);
        ef
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_all_cleared() {
        let flags = EventFlags::new();
        assert_eq!(flags.count_set(), 0);
    }

    #[test]
    fn default_same_as_new() {
        let a = EventFlags::new();
        let b = EventFlags::default();
        assert_eq!(a.count_set(), b.count_set());
    }

    #[test]
    fn set_and_get_flag() {
        let mut flags = EventFlags::new();
        assert!(!flags.get_flag("my_flag"));
        flags.set_flag("my_flag", true);
        assert!(flags.get_flag("my_flag"));
    }

    #[test]
    fn remove_flag() {
        let mut flags = EventFlags::new();
        flags.set_flag("temp", true);
        assert!(flags.get_flag("temp"));
        flags.remove_flag("temp");
        assert!(!flags.get_flag("temp"));
    }

    #[test]
    fn clear_all() {
        let mut flags = EventFlags::new();
        flags.set_flag("a", true);
        flags.set_flag("b", true);
        flags.set_flag("c", true);
        assert_eq!(flags.count_set(), 3);
        flags.clear_all();
        assert_eq!(flags.count_set(), 0);
    }

    #[test]
    fn merge_from() {
        let mut flags = EventFlags::new();
        let mut other = HashMap::default();
        other.insert("x".to_owned(), true);
        other.insert("y".to_owned(), false);
        flags.merge_from(&other);
        assert!(flags.get_flag("x"));
        assert!(!flags.get_flag("y"));
    }

    #[test]
    fn from_hashmap_roundtrip() {
        let mut map = HashMap::default();
        map.insert("a".to_owned(), true);
        map.insert("b".to_owned(), false);
        let flags = EventFlags::from_hashmap(&map);
        assert!(flags.get_flag("a"));
        assert!(!flags.get_flag("b"));
        assert_eq!(flags.to_hashmap(), map);
    }

    #[test]
    fn iter_works() {
        let mut flags = EventFlags::new();
        flags.set_flag("a", true);
        flags.set_flag("b", false);
        let pairs: Vec<_> = flags.iter().collect();
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn as_hashmap_mut_allows_modification() {
        let mut flags = EventFlags::new();
        flags.as_hashmap_mut().insert("direct".to_owned(), true);
        assert!(flags.get_flag("direct"));
    }

    #[test]
    fn non_exhaustive_allows_downstream_extension() {
        // This test verifies the struct compiles with #[non_exhaustive].
        let flags = EventFlags::new();
        assert_eq!(flags.count_set(), 0);
    }
}
