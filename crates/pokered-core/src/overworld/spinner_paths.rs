//! Spinner-tile RLE movement tables, GENERATED from the original scripts
//! (scripts/RocketHideoutB2F/B3F.asm, ViridianGym.asm — the
//! `<Map>ArrowTilePlayerMovement` tables + their RLE movement lists;
//! DecodeArrowMovementRLE + map_objects.asm semantics: standing on (x,y)
//! feeds `dir × steps` straight-line simulated input while the sprite
//! spins (LoadSpinnerArrowTiles) regardless of travel direction. B1F/B4F
//! arrow tiles have no table (decorative).
//! Do not edit by hand — regenerate via scripts/extract_spinner_paths.py.

use crate::overworld::Direction;

/// One spin-pad entry: the pad input to simulate, repeated `steps` times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpinnerStep {
    pub dir: Direction,
    pub steps: u8,
}

/// (x, y) → movement list for the map. First match wins (asm order).
pub fn spinner_paths(map: &str) -> &'static [(u8, u8, &'static [SpinnerStep])] {
    match map {
        "RocketHideoutB2F" => &[
            (4, 9, &[SpinnerStep { dir: Direction::Left, steps: 2 }]),
            (4, 11, &[SpinnerStep { dir: Direction::Right, steps: 4 }]),
            (4, 15, &[SpinnerStep { dir: Direction::Up, steps: 4 }, SpinnerStep { dir: Direction::Right, steps: 4 }]),
            (4, 16, &[SpinnerStep { dir: Direction::Up, steps: 4 }, SpinnerStep { dir: Direction::Right, steps: 4 }, SpinnerStep { dir: Direction::Up, steps: 1 }]),
            (4, 19, &[SpinnerStep { dir: Direction::Left, steps: 2 }]),
            (4, 22, &[SpinnerStep { dir: Direction::Left, steps: 2 }, SpinnerStep { dir: Direction::Up, steps: 3 }]),
            (5, 14, &[SpinnerStep { dir: Direction::Down, steps: 2 }, SpinnerStep { dir: Direction::Right, steps: 4 }]),
            (6, 22, &[SpinnerStep { dir: Direction::Up, steps: 2 }]),
            (6, 24, &[SpinnerStep { dir: Direction::Up, steps: 4 }]),
            (8, 9, &[SpinnerStep { dir: Direction::Left, steps: 6 }]),
            (8, 12, &[SpinnerStep { dir: Direction::Up, steps: 1 }]),
            (8, 15, &[SpinnerStep { dir: Direction::Up, steps: 4 }]),
            (8, 19, &[SpinnerStep { dir: Direction::Left, steps: 6 }]),
            (8, 23, &[SpinnerStep { dir: Direction::Left, steps: 6 }, SpinnerStep { dir: Direction::Up, steps: 4 }]),
            (9, 14, &[SpinnerStep { dir: Direction::Down, steps: 2 }]),
            (9, 22, &[SpinnerStep { dir: Direction::Down, steps: 2 }]),
            (10, 9, &[SpinnerStep { dir: Direction::Left, steps: 8 }]),
            (10, 10, &[SpinnerStep { dir: Direction::Left, steps: 8 }, SpinnerStep { dir: Direction::Up, steps: 1 }]),
            (10, 15, &[SpinnerStep { dir: Direction::Left, steps: 8 }, SpinnerStep { dir: Direction::Up, steps: 6 }]),
            (10, 17, &[SpinnerStep { dir: Direction::Up, steps: 2 }, SpinnerStep { dir: Direction::Right, steps: 4 }]),
            (10, 19, &[SpinnerStep { dir: Direction::Up, steps: 2 }, SpinnerStep { dir: Direction::Right, steps: 4 }, SpinnerStep { dir: Direction::Up, steps: 2 }]),
            (10, 25, &[SpinnerStep { dir: Direction::Right, steps: 4 }]),
            (11, 14, &[SpinnerStep { dir: Direction::Down, steps: 2 }, SpinnerStep { dir: Direction::Right, steps: 4 }, SpinnerStep { dir: Direction::Down, steps: 2 }]),
            (11, 16, &[SpinnerStep { dir: Direction::Down, steps: 2 }, SpinnerStep { dir: Direction::Right, steps: 4 }]),
            (11, 18, &[SpinnerStep { dir: Direction::Down, steps: 2 }]),
            (12, 9, &[SpinnerStep { dir: Direction::Left, steps: 10 }]),
            (12, 11, &[SpinnerStep { dir: Direction::Left, steps: 10 }, SpinnerStep { dir: Direction::Up, steps: 2 }]),
            (12, 13, &[SpinnerStep { dir: Direction::Left, steps: 10 }, SpinnerStep { dir: Direction::Up, steps: 4 }]),
            (12, 17, &[SpinnerStep { dir: Direction::Up, steps: 2 }, SpinnerStep { dir: Direction::Right, steps: 2 }]),
            (13, 10, &[SpinnerStep { dir: Direction::Right, steps: 1 }, SpinnerStep { dir: Direction::Down, steps: 2 }]),
            (13, 12, &[SpinnerStep { dir: Direction::Right, steps: 1 }]),
            (13, 16, &[SpinnerStep { dir: Direction::Down, steps: 2 }, SpinnerStep { dir: Direction::Right, steps: 2 }]),
            (13, 18, &[SpinnerStep { dir: Direction::Down, steps: 2 }, SpinnerStep { dir: Direction::Left, steps: 2 }]),
            (13, 19, &[SpinnerStep { dir: Direction::Up, steps: 2 }, SpinnerStep { dir: Direction::Right, steps: 4 }, SpinnerStep { dir: Direction::Up, steps: 2 }, SpinnerStep { dir: Direction::Left, steps: 3 }]),
            (13, 22, &[SpinnerStep { dir: Direction::Down, steps: 2 }, SpinnerStep { dir: Direction::Left, steps: 4 }]),
            (13, 23, &[SpinnerStep { dir: Direction::Left, steps: 6 }, SpinnerStep { dir: Direction::Up, steps: 4 }, SpinnerStep { dir: Direction::Left, steps: 5 }]),
            (14, 17, &[SpinnerStep { dir: Direction::Up, steps: 2 }]),
            (15, 16, &[SpinnerStep { dir: Direction::Down, steps: 2 }]),
            (16, 14, &[SpinnerStep { dir: Direction::Up, steps: 1 }]),
            (16, 16, &[SpinnerStep { dir: Direction::Up, steps: 3 }]),
            (16, 18, &[SpinnerStep { dir: Direction::Up, steps: 5 }]),
            (17, 10, &[SpinnerStep { dir: Direction::Right, steps: 1 }, SpinnerStep { dir: Direction::Down, steps: 2 }, SpinnerStep { dir: Direction::Left, steps: 4 }]),
            (17, 11, &[SpinnerStep { dir: Direction::Left, steps: 10 }, SpinnerStep { dir: Direction::Up, steps: 2 }, SpinnerStep { dir: Direction::Left, steps: 5 }]),
        ],
        "RocketHideoutB3F" => &[
            (10, 13, &[SpinnerStep { dir: Direction::Right, steps: 4 }]),
            (10, 19, &[SpinnerStep { dir: Direction::Right, steps: 4 }, SpinnerStep { dir: Direction::Up, steps: 4 }, SpinnerStep { dir: Direction::Right, steps: 4 }]),
            (11, 18, &[SpinnerStep { dir: Direction::Down, steps: 4 }, SpinnerStep { dir: Direction::Right, steps: 4 }]),
            (12, 11, &[SpinnerStep { dir: Direction::Left, steps: 2 }]),
            (12, 17, &[SpinnerStep { dir: Direction::Right, steps: 4 }, SpinnerStep { dir: Direction::Up, steps: 2 }, SpinnerStep { dir: Direction::Right, steps: 2 }]),
            (12, 20, &[SpinnerStep { dir: Direction::Right, steps: 4 }, SpinnerStep { dir: Direction::Up, steps: 2 }, SpinnerStep { dir: Direction::Right, steps: 2 }, SpinnerStep { dir: Direction::Up, steps: 3 }]),
            (13, 16, &[SpinnerStep { dir: Direction::Right, steps: 4 }]),
            (14, 11, &[SpinnerStep { dir: Direction::Right, steps: 2 }]),
            (14, 15, &[SpinnerStep { dir: Direction::Right, steps: 4 }]),
            (14, 17, &[SpinnerStep { dir: Direction::Right, steps: 4 }, SpinnerStep { dir: Direction::Up, steps: 2 }]),
            (14, 19, &[SpinnerStep { dir: Direction::Right, steps: 4 }, SpinnerStep { dir: Direction::Up, steps: 4 }]),
            (15, 16, &[SpinnerStep { dir: Direction::Right, steps: 2 }]),
            (15, 18, &[SpinnerStep { dir: Direction::Down, steps: 4 }]),
            (16, 13, &[SpinnerStep { dir: Direction::Up, steps: 2 }]),
            (17, 12, &[SpinnerStep { dir: Direction::Down, steps: 4 }]),
            (18, 16, &[SpinnerStep { dir: Direction::Up, steps: 1 }]),
        ],
        "ViridianGym" => &[
            (19, 11, &[SpinnerStep { dir: Direction::Up, steps: 9 }]),
            (19, 1, &[SpinnerStep { dir: Direction::Left, steps: 8 }]),
            (18, 2, &[SpinnerStep { dir: Direction::Down, steps: 9 }]),
            (11, 2, &[SpinnerStep { dir: Direction::Right, steps: 6 }]),
            (16, 10, &[SpinnerStep { dir: Direction::Down, steps: 2 }]),
            (4, 6, &[SpinnerStep { dir: Direction::Down, steps: 7 }]),
            (5, 13, &[SpinnerStep { dir: Direction::Right, steps: 8 }]),
            (4, 14, &[SpinnerStep { dir: Direction::Right, steps: 9 }]),
            (0, 15, &[SpinnerStep { dir: Direction::Up, steps: 8 }]),
            (1, 15, &[SpinnerStep { dir: Direction::Up, steps: 6 }]),
            (13, 16, &[SpinnerStep { dir: Direction::Left, steps: 6 }]),
            (13, 17, &[SpinnerStep { dir: Direction::Left, steps: 12 }]),
        ],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::Direction;

    /// The generated tables cover the three maps whose scripts install
    /// BIT_SPINNING (RocketHideoutB2F/B3F, ViridianGym); B1F/B4F have none.
    #[test]
    fn three_maps_have_tables() {
        for m in ["RocketHideoutB2F", "RocketHideoutB3F", "ViridianGym"] {
            assert!(!spinner_paths(m).is_empty(), "{m} must have entries");
        }
        assert!(spinner_paths("RocketHideoutB1F").is_empty());
        assert!(spinner_paths("PalletTown").is_empty());
    }

    /// Spot-check against the asm: RocketHideoutB2F (4,9) runs PAD_LEFT × 2
    /// (RocketHideout2ArrowMovement1) and (4,11) runs PAD_RIGHT × 4 (…2).
    #[test]
    fn b2f_entries_match_asm() {
        let t = spinner_paths("RocketHideoutB2F");
        let find = |x, y| {
            t.iter().find(|e| e.0 == x && e.1 == y).unwrap_or_else(|| panic!("({x},{y}) missing"))
        };
        assert_eq!(
            find(4, 9).2,
            &[SpinnerStep { dir: Direction::Left, steps: 2 }]
        );
        assert_eq!(
            find(4, 11).2,
            &[SpinnerStep { dir: Direction::Right, steps: 4 }]
        );
    }

    /// Entries are straight-line runs (every step shares one direction per
    /// run; multi-run entries chain them), and coordinates are unique.
    #[test]
    fn entries_are_unique_coordinates() {
        for map in ["RocketHideoutB2F", "RocketHideoutB3F", "ViridianGym"] {
            let t = spinner_paths(map);
            let mut seen = std::collections::HashSet::new();
            for (x, y, _) in t {
                assert!(seen.insert((*x, *y)), "duplicate ({x},{y}) in {map}");
            }
        }
    }
}
