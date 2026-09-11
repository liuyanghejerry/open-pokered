//! Game-agnostic wild-encounter + battle-handoff control flow (P0d).
//!
//! This module owns only the *step -> maybe-encounter -> handoff* state machine.
//! It deliberately knows nothing about any specific game:
//!
//! - Encounter **rate tables**, **grass/water/fishing slots**, **repel**, the
//!   "first tall-grass step" quirk, and every encounter-rate-by-tile lookup live
//!   GAME-SIDE behind [`EncounterProvider`] (architecture correction C5). The
//!   engine never hardcodes a table or a rate.
//! - The handoff result is a *neutral* [`EncounterStep`] carrying only an opaque
//!   species id + level. The engine does **not** construct a battle - that would
//!   couple overworld onto a concrete battle setup. The game turns the result
//!   into its own monster instance and seeds a battle. This mirrors the existing
//!   "bridge returns intent, game executes" pattern.
//! - All randomness flows through the shared
//!   [`BattleRng`](crate::battle::rng::BattleRng) trait so the draw order stays
//!   game-controlled - critical for reproducing the exact Gen-1 encounter-rate /
//!   slot draw sequence.

use crate::battle::rng::BattleRng;

/// How the player is currently traversing the world when a step completes.
///
/// The provider decides what each mode means (which table to roll, fishing rod
/// power, etc.); the engine just passes it through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncounterMode {
    /// Walking on land (tall grass / cave floor).
    Walking,
    /// Surfing on water.
    Surfing,
    /// Fishing with a rod of the given power tier.
    Fishing {
        /// Game-defined rod power (Old/Good/Super Rod, etc.).
        rod_power: u8,
    },
}

/// Outcome of a single completed step.
///
/// Neutral by construction: on an encounter it carries only `(species_id,
/// level)` so the game - not the engine - builds the battle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncounterStep<S> {
    /// No wild encounter fired this step.
    None,
    /// A wild encounter fired; the game should start a battle with this monster.
    Encounter {
        /// Opaque, game-defined species id + the level to spawn at.
        species_level: (S, u8),
    },
}

/// Game-supplied source of encounter rolls.
///
/// The engine never hardcodes tables, rates, or terrain rules - every data and
/// quirk question is answered by an implementor. New methods added in future
/// should be defaulted so existing games keep compiling unchanged.
pub trait EncounterProvider {
    /// Opaque species identifier the game understands. The engine treats it as
    /// data only and never inspects it.
    type Species: Copy + Eq + core::fmt::Debug;

    /// Roll a wild encounter for the tile the player just stepped onto.
    ///
    /// Returns the chosen `(species, level)` or `None`. The game owns the rate
    /// tables, grass/water/fishing slots, repel, the "first tall-grass step"
    /// quirk, and the encounter-rate-by-tile lookups. The game also owns the
    /// exact RNG draw order via `rng`, so the engine never decides how many
    /// bytes are consumed.
    fn roll_encounter(
        &self,
        map_id: u32,
        x: i32,
        y: i32,
        mode: EncounterMode,
        rng: &mut dyn BattleRng,
    ) -> Option<(Self::Species, u8)>;

    /// Cheap gate: is this tile encounter-eligible at all (tall grass / water)?
    ///
    /// Checked by the engine *before* [`roll_encounter`](Self::roll_encounter)
    /// so no RNG is consumed on plainly ineligible tiles.
    fn is_encounter_tile(&self, map_id: u32, x: i32, y: i32) -> bool;
}

/// Stateless driver for the wild-encounter control flow.
pub struct EncounterEngine;

impl EncounterEngine {
    /// Call once per completed player step.
    ///
    /// The engine checks tile eligibility, then delegates the roll to the
    /// provider. Pure: state in, [`EncounterStep`] out, `rng` injected.
    ///
    /// # Draw order
    ///
    /// 1. If the tile is not encounter-eligible, returns
    ///    [`EncounterStep::None`] **without consuming any RNG**.
    /// 2. Otherwise calls [`EncounterProvider::roll_encounter`], which owns the
    ///    entire RNG draw sequence (rate roll, slot roll, repel checks, ...).
    pub fn on_step<E: EncounterProvider>(
        provider: &E,
        map_id: u32,
        x: i32,
        y: i32,
        mode: EncounterMode,
        rng: &mut dyn BattleRng,
    ) -> EncounterStep<E::Species> {
        if !provider.is_encounter_tile(map_id, x, y) {
            return EncounterStep::None;
        }
        match provider.roll_encounter(map_id, x, y, mode, rng) {
            Some(species_level) => EncounterStep::Encounter { species_level },
            None => EncounterStep::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::rng::ScriptedRng;

    /// Cumulative slot thresholds, mirroring a Gen-1 style table.
    /// `slot_roll <= threshold[i]` selects slot `i`.
    const THRESHOLDS: [u8; 4] = [99, 199, 254, 255];
    const SPECIES: [u8; 4] = [10, 20, 30, 40];
    const LEVELS: [u8; 4] = [3, 5, 7, 9];

    /// Mock provider: only tile (1,1) on map 0 is an encounter tile; rate is
    /// configurable. Rolls rate first, then slot - proving the engine leaves the
    /// draw order entirely to the provider.
    struct MockProvider {
        rate: u8,
        eligible_tile: (i32, i32),
    }

    impl MockProvider {
        fn new(rate: u8) -> Self {
            Self {
                rate,
                eligible_tile: (1, 1),
            }
        }

        fn select_slot(roll: u8) -> usize {
            for (i, &t) in THRESHOLDS.iter().enumerate() {
                if roll <= t {
                    return i;
                }
            }
            3
        }
    }

    impl EncounterProvider for MockProvider {
        type Species = u8;

        fn is_encounter_tile(&self, map_id: u32, x: i32, y: i32) -> bool {
            map_id == 0 && (x, y) == self.eligible_tile
        }

        fn roll_encounter(
            &self,
            _map_id: u32,
            _x: i32,
            _y: i32,
            _mode: EncounterMode,
            rng: &mut dyn BattleRng,
        ) -> Option<(u8, u8)> {
            // Draw order: rate byte first, then slot byte.
            let rate_roll = rng.next_u8();
            if rate_roll >= self.rate {
                return None;
            }
            let slot = Self::select_slot(rng.next_u8());
            Some((SPECIES[slot], LEVELS[slot]))
        }
    }

    #[test]
    fn non_encounter_tile_returns_none_and_consumes_no_rng() {
        let provider = MockProvider::new(255);
        let mut rng = ScriptedRng::new(vec![0, 0, 0]);
        // (5, 5) is not the eligible tile.
        let step = EncounterEngine::on_step(&provider, 0, 5, 5, EncounterMode::Walking, &mut rng);
        assert_eq!(step, EncounterStep::None);
        // No RNG consumed: stream still at byte 0.
        assert_eq!(rng.consumed(), 0);
    }

    #[test]
    fn roll_at_or_above_rate_returns_none() {
        let provider = MockProvider::new(100);
        // rate roll == rate (100) must NOT fire.
        let mut rng = ScriptedRng::new(vec![100, 0]);
        let step = EncounterEngine::on_step(&provider, 0, 1, 1, EncounterMode::Walking, &mut rng);
        assert_eq!(step, EncounterStep::None);

        // rate roll well above rate.
        let mut rng = ScriptedRng::new(vec![200, 0]);
        assert_eq!(
            EncounterEngine::on_step(&provider, 0, 1, 1, EncounterMode::Walking, &mut rng),
            EncounterStep::None
        );
    }

    #[test]
    fn roll_below_rate_returns_expected_species_level() {
        let provider = MockProvider::new(100);
        // rate roll 0 (< 100 => hit); slot roll 0 => slot 0 (species 10, lvl 3).
        let mut rng = ScriptedRng::new(vec![0, 0]);
        let step = EncounterEngine::on_step(&provider, 0, 1, 1, EncounterMode::Walking, &mut rng);
        assert_eq!(
            step,
            EncounterStep::Encounter {
                species_level: (10, 3)
            }
        );
    }

    #[test]
    fn slot_selection_picks_the_right_table_entry() {
        let provider = MockProvider::new(255);

        // slot roll 99 <= 99 => slot 0.
        let mut rng = ScriptedRng::new(vec![0, 99]);
        assert_eq!(
            EncounterEngine::on_step(&provider, 0, 1, 1, EncounterMode::Walking, &mut rng),
            EncounterStep::Encounter {
                species_level: (10, 3)
            }
        );

        // slot roll 100 => slot 1 (species 20, lvl 5).
        let mut rng = ScriptedRng::new(vec![0, 100]);
        assert_eq!(
            EncounterEngine::on_step(&provider, 0, 1, 1, EncounterMode::Walking, &mut rng),
            EncounterStep::Encounter {
                species_level: (20, 5)
            }
        );

        // slot roll 255 => slot 3 (species 40, lvl 9).
        let mut rng = ScriptedRng::new(vec![0, 255]);
        assert_eq!(
            EncounterEngine::on_step(&provider, 0, 1, 1, EncounterMode::Walking, &mut rng),
            EncounterStep::Encounter {
                species_level: (40, 9)
            }
        );
    }

    #[test]
    fn mode_is_passed_through_to_provider() {
        // A provider that only fires while Surfing, proving the engine forwards
        // the mode verbatim.
        struct SurfOnly;
        impl EncounterProvider for SurfOnly {
            type Species = u8;
            fn is_encounter_tile(&self, _m: u32, _x: i32, _y: i32) -> bool {
                true
            }
            fn roll_encounter(
                &self,
                _m: u32,
                _x: i32,
                _y: i32,
                mode: EncounterMode,
                _rng: &mut dyn BattleRng,
            ) -> Option<(u8, u8)> {
                matches!(mode, EncounterMode::Surfing).then_some((7, 12))
            }
        }
        let provider = SurfOnly;
        let mut rng = ScriptedRng::new(vec![0]);
        assert_eq!(
            EncounterEngine::on_step(&provider, 0, 0, 0, EncounterMode::Walking, &mut rng),
            EncounterStep::None
        );
        assert_eq!(
            EncounterEngine::on_step(&provider, 0, 0, 0, EncounterMode::Surfing, &mut rng),
            EncounterStep::Encounter {
                species_level: (7, 12)
            }
        );
        assert_eq!(
            EncounterEngine::on_step(
                &provider,
                0,
                0,
                0,
                EncounterMode::Fishing { rod_power: 2 },
                &mut rng
            ),
            EncounterStep::None
        );
    }
}
