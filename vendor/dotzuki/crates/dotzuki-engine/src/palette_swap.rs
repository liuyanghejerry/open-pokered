use crate::hash::HashMap;

use crate::render::Rgba;

/// A named palette swap that maps palette_group indices to replacement colors.
#[derive(Debug, Clone)]
pub struct PaletteSwap {
    pub name: String,
    /// Maps palette_group (0-255) to 4 replacement RGBA colors.
    pub mappings: HashMap<u8, [Rgba; 4]>,
}

/// Manages active palette swaps.
///
/// Allows registration of named [`PaletteSwap`]s and activating one at a time.
/// When active, the [`apply`](PaletteSwapManager::apply) method can be used
/// to override colors for a given palette group during rendering.
#[derive(Debug, Clone)]
pub struct PaletteSwapManager {
    pub swaps: HashMap<String, PaletteSwap>,
    pub active_swap: Option<String>,
}

impl PaletteSwapManager {
    /// Creates a new empty manager with no swaps and no active swap.
    pub fn new() -> Self {
        Self {
            swaps: HashMap::default(),
            active_swap: None,
        }
    }

    /// Registers a named palette swap.
    ///
    /// If a swap with the same name already exists, it is replaced.
    pub fn add_swap(&mut self, swap: PaletteSwap) {
        self.swaps.insert(swap.name.clone(), swap);
    }

    /// Sets the active swap by name.
    ///
    /// Returns `true` if the named swap exists and was activated,
    /// `false` if no swap with that name is registered.
    pub fn set_active(&mut self, name: &str) -> bool {
        if self.swaps.contains_key(name) {
            self.active_swap = Some(name.to_string());
            true
        } else {
            false
        }
    }

    /// Clears the active swap (disables palette swapping).
    pub fn clear_active(&mut self) {
        self.active_swap = None;
    }

    /// Applies the active palette swap for the given palette group.
    ///
    /// If an active swap is set and it contains a mapping for `palette_group`,
    /// the `colors` array is replaced with the mapped colors.
    /// Otherwise this is a no-op.
    pub fn apply(&self, palette_group: u8, colors: &mut [Rgba; 4]) {
        if let Some(ref active_name) = self.active_swap {
            if let Some(swap) = self.swaps.get(active_name) {
                if let Some(mapped) = swap.mappings.get(&palette_group) {
                    *colors = *mapped;
                }
            }
        }
    }
}

impl Default for PaletteSwapManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_swap(name: &str, mappings: Vec<(u8, [Rgba; 4])>) -> PaletteSwap {
        PaletteSwap {
            name: name.to_string(),
            mappings: mappings.into_iter().collect(),
        }
    }

    fn rgb(r: u8, g: u8, b: u8) -> Rgba {
        Rgba { r, g, b, a: 255 }
    }

    #[test]
    fn test_add_swap() {
        let mut manager = PaletteSwapManager::new();
        let swap = make_swap("day", vec![(0, [rgb(10, 10, 10); 4])]);
        manager.add_swap(swap);
        assert_eq!(manager.swaps.len(), 1);
        assert!(manager.swaps.contains_key("day"));
    }

    #[test]
    fn test_activate_existing_swap() {
        let mut manager = PaletteSwapManager::new();
        manager.add_swap(make_swap("night", vec![(1, [rgb(5, 5, 5); 4])]));
        assert!(manager.set_active("night"));
        assert_eq!(manager.active_swap, Some("night".to_string()));
    }

    #[test]
    fn test_activate_nonexistent_swap() {
        let mut manager = PaletteSwapManager::new();
        assert!(!manager.set_active("does_not_exist"));
        assert_eq!(manager.active_swap, None);
    }

    #[test]
    fn test_clear_active() {
        let mut manager = PaletteSwapManager::new();
        manager.add_swap(make_swap("seasonal", vec![]));
        manager.set_active("seasonal");
        assert!(manager.active_swap.is_some());
        manager.clear_active();
        assert!(manager.active_swap.is_none());
    }

    #[test]
    fn test_apply_swap_changes_colors() {
        let mut manager = PaletteSwapManager::new();
        let day_colors = [rgb(255, 255, 255); 4];
        let night_colors = [rgb(10, 10, 10); 4];
        manager.add_swap(make_swap("night", vec![(0, night_colors)]));
        manager.set_active("night");

        let mut colors = day_colors;
        manager.apply(0, &mut colors);
        assert_eq!(colors, night_colors);
    }

    #[test]
    fn test_noop_when_no_mapping_for_group() {
        let mut manager = PaletteSwapManager::new();
        let original = [rgb(100, 100, 100); 4];
        manager.add_swap(make_swap("test", vec![(0, [rgb(0, 0, 0); 4])]));
        manager.set_active("test");

        let mut colors = original;
        // palette_group=1 has no mapping
        manager.apply(1, &mut colors);
        assert_eq!(colors, original);
    }

    #[test]
    fn test_noop_when_no_active_swap() {
        let manager = PaletteSwapManager::new();
        let mut colors = [rgb(200, 200, 200); 4];
        manager.apply(0, &mut colors);
        assert_eq!(colors, [rgb(200, 200, 200); 4]);
    }

    #[test]
    fn test_swap_replaces_existing() {
        let mut manager = PaletteSwapManager::new();
        manager.add_swap(make_swap("pal", vec![(0, [rgb(1, 1, 1); 4])]));
        manager.add_swap(make_swap("pal", vec![(0, [rgb(2, 2, 2); 4])]));
        assert_eq!(manager.swaps.len(), 1);

        manager.set_active("pal");
        let mut colors = [rgb(0, 0, 0); 4];
        manager.apply(0, &mut colors);
        assert_eq!(colors, [rgb(2, 2, 2); 4]);
    }
}
