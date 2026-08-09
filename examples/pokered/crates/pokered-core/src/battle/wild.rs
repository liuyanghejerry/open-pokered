//! Wild encounter triggering and slot selection.
//!
//! Faithfully implements the Gen 1 wild encounter algorithm from
//! `engine/battle/wild_encounters.asm` and `data/wild/probabilities.asm`.
//!
//! ## Algorithm (from TryDoWildEncounter):
//! 1. Various pre-checks (NPC movement, door tiles, ledge jumps, etc.) — handled by caller.
//! 2. If repel is active, decrement counter.
//! 3. Check tile type: grass tile uses wGrassRate, water tile uses wWaterRate.
//!    Indoor maps (>= FIRST_INDOOR_MAP) with non-Forest tileset use wGrassRate even
//!    without being on a grass/water tile.
//! 4. Compare encounter_rate against hRandomAdd — if random >= rate, no encounter.
//! 5. Use hRandomSub to select encounter slot from cumulative probability table.
//! 6. Look up (level, species) from the appropriate encounter table (grass or water).
//! 7. If repel active and wild mon level < party lead level, no encounter.
//!
//! ## Encounter Slot Probabilities (cumulative thresholds, from probabilities.asm):
//! Slot 0: 51/256 = 19.9%  (threshold  50)
//! Slot 1: 51/256 = 19.9%  (threshold 101)
//! Slot 2: 39/256 = 15.2%  (threshold 140)
//! Slot 3: 25/256 =  9.8%  (threshold 165)
//! Slot 4: 25/256 =  9.8%  (threshold 190)
//! Slot 5: 25/256 =  9.8%  (threshold 215)
//! Slot 6: 13/256 =  5.1%  (threshold 228)
//! Slot 7: 13/256 =  5.1%  (threshold 241)
//! Slot 8: 11/256 =  4.3%  (threshold 252)
//! Slot 9:  4/256 =  1.6%  (threshold 255 — capped at u8 max)

use pokered_data::species::Species;
use pokered_data::wild_data::WildEncounterTable;

/// Cumulative probability thresholds for encounter slot selection.
/// Each value is (cumulative_sum - 1). The random byte is compared
/// against these; the first slot where random <= threshold is chosen.
///
/// From data/wild/probabilities.asm: wild_chance_total values minus 1.
pub const ENCOUNTER_SLOT_THRESHOLDS: [u8; 10] = [
    50,  // slot 0:  51 cumulative - 1
    101, // slot 1: 102 cumulative - 1
    140, // slot 2: 141 cumulative - 1
    165, // slot 3: 166 cumulative - 1
    190, // slot 4: 191 cumulative - 1
    215, // slot 5: 216 cumulative - 1
    228, // slot 6: 229 cumulative - 1
    241, // slot 7: 242 cumulative - 1
    252, // slot 8: 253 cumulative - 1
    255, // slot 9: 256 cumulative - 1 (capped at u8)
];

/// Random values needed for wild encounter determination.
/// All values are u8 (0-255) to match the Game Boy's Random() output.
#[derive(Debug, Clone, Copy)]
pub struct WildEncounterRandoms {
    /// Compared against encounter_rate. If >= rate, no encounter.
    /// Corresponds to hRandomAdd in the ASM.
    pub encounter_roll: u8,
    /// Used to select encounter slot from probability table.
    /// Corresponds to hRandomSub in the ASM.
    pub slot_roll: u8,
}

/// Result of a wild encounter attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WildEncounterResult {
    /// No encounter triggered.
    NoEncounter,
    /// Encounter triggered — contains (level, species).
    Encounter { level: u8, species: Species },
    /// Encounter would have triggered, but repel blocked it
    /// (wild mon level < party lead level).
    RepelBlocked,
}

/// The context needed for encounter checks that the caller provides.
#[derive(Debug, Clone, Copy)]
pub struct EncounterContext {
    /// Whether a repel is currently active (remaining steps > 0).
    pub repel_active: bool,
    /// Level of the first Pokémon in the player's party (for repel check).
    pub party_lead_level: u8,
}

/// Select an encounter slot index (0-9) from the slot probability table.
///
/// Implements the .determineEncounterSlot loop from wild_encounters.asm:
/// iterates through cumulative thresholds until slot_roll <= threshold.
pub fn select_encounter_slot(slot_roll: u8) -> usize {
    for (i, &threshold) in ENCOUNTER_SLOT_THRESHOLDS.iter().enumerate() {
        if slot_roll <= threshold {
            return i;
        }
    }
    // Should never reach here since threshold[9] = 255 covers all u8 values,
    // but default to last slot for safety.
    9
}

/// Try to trigger a wild encounter.
///
/// This implements the core logic of TryDoWildEncounter from wild_encounters.asm.
/// Pre-conditions (NPC movement, door tiles, warps, etc.) are handled by the caller.
///
/// # Arguments
/// * `encounter_table` - The encounter table for the current tile (grass or water).
///   If None, no encounters are possible on this tile.
/// * `randoms` - Random values for encounter rate and slot selection.
/// * `context` - Repel state and party lead level.
///
/// # Returns
/// `WildEncounterResult` indicating no encounter, successful encounter, or repel blocked.
pub fn try_wild_encounter(
    encounter_table: Option<&WildEncounterTable>,
    randoms: &WildEncounterRandoms,
    context: &EncounterContext,
) -> WildEncounterResult {
    let table = match encounter_table {
        Some(t) => t,
        None => return WildEncounterResult::NoEncounter,
    };

    if table.encounter_rate == 0 {
        return WildEncounterResult::NoEncounter;
    }

    if table.mons.is_empty() {
        return WildEncounterResult::NoEncounter;
    }

    // ASM: cp b / jr nc — encounter triggers only when random < rate
    if randoms.encounter_roll >= table.encounter_rate {
        return WildEncounterResult::NoEncounter;
    }

    let slot_index = select_encounter_slot(randoms.slot_roll);
    let slot_index = slot_index.min(table.mons.len() - 1);
    let wild_mon = &table.mons[slot_index];

    // ASM: cp b / jr c — repel blocks when wild level < party lead level
    if context.repel_active && wild_mon.level < context.party_lead_level {
        return WildEncounterResult::RepelBlocked;
    }

    WildEncounterResult::Encounter {
        level: wild_mon.level,
        species: wild_mon.species,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_data::wild_data::WildMon;

    fn make_test_table() -> WildEncounterTable {
        WildEncounterTable {
            encounter_rate: 25,
            mons: vec![
                WildMon {
                    level: 3,
                    species: Species::Pidgey,
                }, // slot 0
                WildMon {
                    level: 3,
                    species: Species::Rattata,
                }, // slot 1
                WildMon {
                    level: 3,
                    species: Species::Rattata,
                }, // slot 2
                WildMon {
                    level: 2,
                    species: Species::Rattata,
                }, // slot 3
                WildMon {
                    level: 2,
                    species: Species::Pidgey,
                }, // slot 4
                WildMon {
                    level: 3,
                    species: Species::Pidgey,
                }, // slot 5
                WildMon {
                    level: 3,
                    species: Species::Pidgey,
                }, // slot 6
                WildMon {
                    level: 4,
                    species: Species::Rattata,
                }, // slot 7
                WildMon {
                    level: 4,
                    species: Species::Pidgey,
                }, // slot 8
                WildMon {
                    level: 5,
                    species: Species::Pidgey,
                }, // slot 9
            ],
        }
    }

    fn no_repel() -> EncounterContext {
        EncounterContext {
            repel_active: false,
            party_lead_level: 50,
        }
    }

    #[test]
    fn slot_selection_first_slot() {
        // slot_roll 0..=50 -> slot 0
        assert_eq!(select_encounter_slot(0), 0);
        assert_eq!(select_encounter_slot(50), 0);
    }

    #[test]
    fn slot_selection_second_slot() {
        // slot_roll 51..=101 -> slot 1
        assert_eq!(select_encounter_slot(51), 1);
        assert_eq!(select_encounter_slot(101), 1);
    }

    #[test]
    fn slot_selection_last_slot() {
        assert_eq!(select_encounter_slot(253), 9);
        assert_eq!(select_encounter_slot(255), 9);
    }

    #[test]
    fn slot_selection_all_boundaries() {
        assert_eq!(select_encounter_slot(50), 0);
        assert_eq!(select_encounter_slot(51), 1);
        assert_eq!(select_encounter_slot(101), 1);
        assert_eq!(select_encounter_slot(102), 2);
        assert_eq!(select_encounter_slot(140), 2);
        assert_eq!(select_encounter_slot(141), 3);
        assert_eq!(select_encounter_slot(165), 3);
        assert_eq!(select_encounter_slot(166), 4);
        assert_eq!(select_encounter_slot(190), 4);
        assert_eq!(select_encounter_slot(191), 5);
        assert_eq!(select_encounter_slot(215), 5);
        assert_eq!(select_encounter_slot(216), 6);
        assert_eq!(select_encounter_slot(228), 6);
        assert_eq!(select_encounter_slot(229), 7);
        assert_eq!(select_encounter_slot(241), 7);
        assert_eq!(select_encounter_slot(242), 8);
        assert_eq!(select_encounter_slot(252), 8);
        assert_eq!(select_encounter_slot(253), 9);
    }

    #[test]
    fn no_encounter_when_roll_exceeds_rate() {
        let table = make_test_table();
        let randoms = WildEncounterRandoms {
            encounter_roll: 25, // >= rate (25), no encounter
            slot_roll: 0,
        };
        let result = try_wild_encounter(Some(&table), &randoms, &no_repel());
        assert_eq!(result, WildEncounterResult::NoEncounter);
    }

    #[test]
    fn no_encounter_when_roll_well_above_rate() {
        let table = make_test_table();
        let randoms = WildEncounterRandoms {
            encounter_roll: 200,
            slot_roll: 0,
        };
        let result = try_wild_encounter(Some(&table), &randoms, &no_repel());
        assert_eq!(result, WildEncounterResult::NoEncounter);
    }

    #[test]
    fn encounter_when_roll_below_rate() {
        let table = make_test_table();
        let randoms = WildEncounterRandoms {
            encounter_roll: 24, // < rate (25), encounter triggers
            slot_roll: 0,       // slot 0 -> Pidgey lv3
        };
        let result = try_wild_encounter(Some(&table), &randoms, &no_repel());
        assert_eq!(
            result,
            WildEncounterResult::Encounter {
                level: 3,
                species: Species::Pidgey,
            }
        );
    }

    #[test]
    fn encounter_roll_zero_triggers() {
        let table = make_test_table();
        let randoms = WildEncounterRandoms {
            encounter_roll: 0,
            slot_roll: 253, // slot 9 -> Pidgey lv5
        };
        let result = try_wild_encounter(Some(&table), &randoms, &no_repel());
        assert_eq!(
            result,
            WildEncounterResult::Encounter {
                level: 5,
                species: Species::Pidgey,
            }
        );
    }

    #[test]
    fn no_encounter_with_no_table() {
        let randoms = WildEncounterRandoms {
            encounter_roll: 0,
            slot_roll: 0,
        };
        let result = try_wild_encounter(None, &randoms, &no_repel());
        assert_eq!(result, WildEncounterResult::NoEncounter);
    }

    #[test]
    fn no_encounter_with_zero_rate() {
        let table = WildEncounterTable {
            encounter_rate: 0,
            mons: vec![WildMon {
                level: 10,
                species: Species::Zubat,
            }],
        };
        let randoms = WildEncounterRandoms {
            encounter_roll: 0,
            slot_roll: 0,
        };
        let result = try_wild_encounter(Some(&table), &randoms, &no_repel());
        assert_eq!(result, WildEncounterResult::NoEncounter);
    }

    #[test]
    fn repel_blocks_lower_level_wild() {
        let table = make_test_table();
        let randoms = WildEncounterRandoms {
            encounter_roll: 0,
            slot_roll: 0, // slot 0 -> Pidgey lv3
        };
        let ctx = EncounterContext {
            repel_active: true,
            party_lead_level: 10, // lv10 > lv3, repel blocks
        };
        let result = try_wild_encounter(Some(&table), &randoms, &ctx);
        assert_eq!(result, WildEncounterResult::RepelBlocked);
    }

    #[test]
    fn repel_allows_equal_level_wild() {
        let table = make_test_table();
        let randoms = WildEncounterRandoms {
            encounter_roll: 0,
            slot_roll: 0, // slot 0 -> Pidgey lv3
        };
        let ctx = EncounterContext {
            repel_active: true,
            party_lead_level: 3, // lv3 == lv3, repel does NOT block (ASM: jr c, not jr z)
        };
        let result = try_wild_encounter(Some(&table), &randoms, &ctx);
        assert_eq!(
            result,
            WildEncounterResult::Encounter {
                level: 3,
                species: Species::Pidgey,
            }
        );
    }

    #[test]
    fn repel_allows_higher_level_wild() {
        let table = make_test_table();
        let randoms = WildEncounterRandoms {
            encounter_roll: 0,
            slot_roll: 253, // slot 9 -> Pidgey lv5
        };
        let ctx = EncounterContext {
            repel_active: true,
            party_lead_level: 3, // lv3 < lv5, repel does NOT block
        };
        let result = try_wild_encounter(Some(&table), &randoms, &ctx);
        assert_eq!(
            result,
            WildEncounterResult::Encounter {
                level: 5,
                species: Species::Pidgey,
            }
        );
    }

    #[test]
    fn encounter_rate_boundary_one() {
        // encounter_rate = 1: only roll of 0 triggers encounter
        let table = WildEncounterTable {
            encounter_rate: 1,
            mons: vec![WildMon {
                level: 50,
                species: Species::Mewtwo,
            }],
        };
        let randoms_hit = WildEncounterRandoms {
            encounter_roll: 0,
            slot_roll: 0,
        };
        let randoms_miss = WildEncounterRandoms {
            encounter_roll: 1,
            slot_roll: 0,
        };
        assert_eq!(
            try_wild_encounter(Some(&table), &randoms_hit, &no_repel()),
            WildEncounterResult::Encounter {
                level: 50,
                species: Species::Mewtwo
            }
        );
        assert_eq!(
            try_wild_encounter(Some(&table), &randoms_miss, &no_repel()),
            WildEncounterResult::NoEncounter
        );
    }
}
