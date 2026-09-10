//! Engine-level unit tests for the generic party model, driven by a small mock
//! game (mirrors the mock style in `battle/mod.rs`).

use super::*;

// ---- Mock id types ---------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MockSpecies(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MockMove(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MockItem(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MockStat {
    Hp,
    Attack,
    Speed,
}

/// Genetics: a flat bonus added to every stat (stand-in for DVs).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MockGenetics {
    bonus: u16,
}

/// Training: opaque, unused in the mock formula.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MockTraining;

/// The mock game: a single provider implementing every party trait.
///
/// `Debug` is required because `MonsterInstance<P>` / `PartyFull<P>` derive
/// `Debug` (propagating a `P: Debug` bound), which `.unwrap()` needs on errors.
#[derive(Debug)]
struct MockGame;

const STATS: [MockStat; 3] = [MockStat::Hp, MockStat::Attack, MockStat::Speed];

impl MonsterProvider for MockGame {
    type SpeciesId = MockSpecies;
    type MoveId = MockMove;
    type Genetics = MockGenetics;
    type Training = MockTraining;
    type Stat = MockStat;

    fn base_stat(&self, species: Self::SpeciesId, stat: Self::Stat) -> u16 {
        // Base depends on species id so evolution changes the numbers.
        let s = species.0;
        match stat {
            MockStat::Hp => 10 + s,
            MockStat::Attack => 5 + s,
            MockStat::Speed => 3 + s,
        }
    }

    fn calc_stat(
        &self,
        species: Self::SpeciesId,
        stat: Self::Stat,
        level: u8,
        genetics: &Self::Genetics,
        _training: &Self::Training,
    ) -> u16 {
        // Simple, hand-computable mock formula:
        //   stat = base + level + genetics.bonus   (HP gets +5 flat)
        let base = self.base_stat(species, stat);
        let flat = if matches!(stat, MockStat::Hp) { 5 } else { 0 };
        base + level as u16 + genetics.bonus + flat
    }

    fn stats(&self) -> &[Self::Stat] {
        &STATS
    }

    fn hp_stat(&self) -> Self::Stat {
        MockStat::Hp
    }

    fn max_moves(&self) -> usize {
        4
    }
}

impl ExpProvider for MockGame {
    /// Cubic curve: total exp to BE at `level` = level^3.
    fn exp_for_level(&self, _species: Self::SpeciesId, level: u8) -> u32 {
        let l = level as u32;
        l * l * l
    }

    fn max_level(&self) -> u8 {
        100
    }
}

impl EvolutionProvider for MockGame {
    type EvoItem = MockItem;

    fn evolution_target(
        &self,
        inst: &MonsterInstance<Self>,
        trigger: EvolutionTrigger<Self::EvoItem>,
    ) -> Option<Self::SpeciesId> {
        match trigger {
            // Species 1 evolves into species 2 at level >= 16.
            EvolutionTrigger::LevelUp if inst.species == MockSpecies(1) && inst.level >= 16 => {
                Some(MockSpecies(2))
            }
            // Species 2 evolves into 3 with item 99.
            EvolutionTrigger::Item(MockItem(99)) if inst.species == MockSpecies(2) => {
                Some(MockSpecies(3))
            }
            _ => None,
        }
    }
}

fn mk(level: u8) -> MonsterInstance<MockGame> {
    MonsterInstance::new(
        &MockGame,
        MockSpecies(1),
        level,
        MockGenetics { bonus: 2 },
        MockTraining,
    )
}

// ---- Stat calculation ------------------------------------------------------

#[test]
fn stat_calc_matches_hand_computed_formula() {
    let m = mk(5);
    // base_hp(species 1) = 10 + 1 = 11; +level 5 +bonus 2 +flat 5 = 23
    assert_eq!(m.stats.get(MockStat::Hp), 23);
    // base_attack = 5 + 1 = 6; +5 +2 = 13
    assert_eq!(m.stats.get(MockStat::Attack), 13);
    // base_speed = 3 + 1 = 4; +5 +2 = 11
    assert_eq!(m.stats.get(MockStat::Speed), 11);
    // current_hp initialized to max
    assert_eq!(m.current_hp, m.max_hp(&MockGame));
    assert_eq!(m.max_hp(&MockGame), 23);
}

#[test]
fn recalc_preserves_absolute_hp_clamped() {
    let mut m = mk(5);
    m.current_hp = 10; // damaged
    m.level = 6; // would raise max HP
    m.recalc_stats(&MockGame);
    // current_hp preserved (not scaled), still below new max
    assert_eq!(m.current_hp, 10);

    // Now drop level so max HP falls below current; current must clamp down.
    m.level = 1;
    m.current_hp = 999;
    m.recalc_stats(&MockGame);
    assert_eq!(m.current_hp, m.max_hp(&MockGame));
}

// ---- Experience / leveling -------------------------------------------------

#[test]
fn gain_exp_crosses_one_level_boundary() {
    let mut m = mk(2); // exp_for_level(2) = 8
    m.exp = 8; // exactly at L2 threshold
               // Need exp_for_level(3) = 27 to reach L3.
    let result = m.gain_exp(&MockGame, 27 - 8);
    assert_eq!(result.old_level, 2);
    assert_eq!(result.new_level, 3);
    assert_eq!(result.levels_gained, 1);
    assert!(result.gained());
}

#[test]
fn gain_exp_crosses_multiple_levels() {
    let mut m = mk(1);
    m.exp = 1; // L1 = 1
               // Jump straight to L5: exp_for_level(5) = 125.
    let result = m.gain_exp(&MockGame, 125 - 1);
    assert_eq!(result.old_level, 1);
    assert_eq!(result.new_level, 5);
    assert_eq!(result.levels_gained, 4);
    // Stats recomputed for the new level.
    assert_eq!(m.stats.get(MockStat::Hp), 10 + 1 + 5 + 2 + 5);
}

#[test]
fn gain_exp_no_level_when_below_threshold() {
    let mut m = mk(2);
    m.exp = 8;
    let result = m.gain_exp(&MockGame, 1); // not enough for L3 (needs 27)
    assert_eq!(result.levels_gained, 0);
    assert_eq!(result.new_level, 2);
    assert!(!result.gained());
}

#[test]
fn gain_exp_caps_at_max_level() {
    let mut m = mk(99);
    let result = m.gain_exp(&MockGame, u32::MAX);
    assert_eq!(m.level, 100);
    assert_eq!(result.new_level, 100);
    // Exp is clamped to the max-level threshold.
    assert_eq!(m.exp, MockGame.exp_for_level(MockSpecies(1), 100));
    // Already at cap: no further gain.
    let again = m.gain_exp(&MockGame, u32::MAX);
    assert_eq!(again.levels_gained, 0);
}

// ---- Evolution -------------------------------------------------------------

#[test]
fn evolution_triggers_on_level_up() {
    let mut m = mk(16);
    let new_species = m.try_evolve(&MockGame, EvolutionTrigger::LevelUp);
    assert_eq!(new_species, Some(MockSpecies(2)));
    assert_eq!(m.species, MockSpecies(2));
    // Stats recomputed against the new species' base stats.
    // base_hp(species 2) = 12; +16 +2 +5 = 35
    assert_eq!(m.stats.get(MockStat::Hp), 35);
}

#[test]
fn evolution_canceled_when_provider_returns_none() {
    let mut m = mk(10); // below level 16
    let result = m.try_evolve(&MockGame, EvolutionTrigger::LevelUp);
    assert_eq!(result, None);
    assert_eq!(m.species, MockSpecies(1)); // unchanged
}

#[test]
fn evolution_by_item() {
    let mut m = mk(20);
    m.species = MockSpecies(2);
    m.recalc_stats(&MockGame);
    let result = m.try_evolve(&MockGame, EvolutionTrigger::Item(MockItem(99)));
    assert_eq!(result, Some(MockSpecies(3)));
    assert_eq!(m.species, MockSpecies(3));
    // Wrong item does nothing.
    let none = m.try_evolve(&MockGame, EvolutionTrigger::Item(MockItem(1)));
    assert_eq!(none, None);
}

// ---- Party -----------------------------------------------------------------

#[test]
fn party_add_until_full_then_rejects() {
    let mut party: Party<MockGame> = Party::new(3);
    assert!(party.is_empty());
    party.add(mk(5)).unwrap();
    party.add(mk(6)).unwrap();
    party.add(mk(7)).unwrap();
    assert!(party.is_full());
    assert_eq!(party.len(), 3);

    let overflow = mk(8);
    let err = party.add(overflow).unwrap_err();
    // Rejected monster handed back.
    assert_eq!(err.0.level, 8);
    assert_eq!(party.len(), 3);
}

#[test]
fn party_remove_reindexes() {
    let mut party: Party<MockGame> = Party::new(6);
    party.add(mk(1)).unwrap();
    party.add(mk(2)).unwrap();
    party.add(mk(3)).unwrap();
    let removed = party.remove(1).unwrap();
    assert_eq!(removed.level, 2);
    assert_eq!(party.len(), 2);
    assert_eq!(party.get(0).unwrap().level, 1);
    assert_eq!(party.get(1).unwrap().level, 3);
    assert!(party.remove(99).is_none());
}

#[test]
fn party_swap() {
    let mut party: Party<MockGame> = Party::new(6);
    party.add(mk(1)).unwrap();
    party.add(mk(2)).unwrap();
    party.swap(0, 1);
    assert_eq!(party.get(0).unwrap().level, 2);
    assert_eq!(party.get(1).unwrap().level, 1);
    // Out-of-range swap is a no-op (does not panic).
    party.swap(0, 99);
    assert_eq!(party.get(0).unwrap().level, 2);
}

#[test]
fn party_capacity_is_not_hardcoded() {
    let big: Party<MockGame> = Party::new(30);
    assert_eq!(big.capacity(), 30);
    let small: Party<MockGame> = Party::new(1);
    assert_eq!(small.capacity(), 1);
}

// ---- Boxes -----------------------------------------------------------------

#[test]
fn box_store_switch_add_remove() {
    let mut store: BoxStore<MockGame> = BoxStore::new(4, 2);
    assert_eq!(store.box_count(), 4);
    assert_eq!(store.current_index(), 0);

    store.current_mut().add(mk(5)).unwrap();
    store.current_mut().add(mk(6)).unwrap();
    assert!(store.current().is_full());
    // Third add to the full box is rejected.
    assert!(store.current_mut().add(mk(7)).is_err());

    // Switch to a fresh box.
    store.switch(2);
    assert_eq!(store.current_index(), 2);
    assert!(store.current().is_empty());
    store.current_mut().add(mk(9)).unwrap();
    assert_eq!(store.current().len(), 1);

    // Removing from box 0 still works via get_mut.
    let removed = store.get_mut(0).unwrap().remove(0).unwrap();
    assert_eq!(removed.level, 5);
    assert_eq!(store.get(0).unwrap().len(), 1);

    // Out-of-range switch is ignored.
    store.switch(99);
    assert_eq!(store.current_index(), 2);
}

// ---- Battle adapter --------------------------------------------------------

// A minimal `BattleProvider` so we can exercise `to_battler`. Its associated
// types reuse the mock monster ids, but `to_battler` is generic and bridges via
// caller-supplied closures, proving the two provider worlds need not share types.
#[derive(Debug)]
struct MockBattle;

impl crate::battle::BattleProvider for MockBattle {
    type Monster = ();
    type Move = MockMove;
    type Ability = ();
    type Status = MonsterStatus;
    type Stat = MockStat;
    type Species = MockSpecies;
    type Type = u8;
    type Item = MockItem;

    fn calculate_damage(
        &self,
        _move: &Self::Move,
        _attacker: &crate::battle::BattlerState<Self>,
        _defender: &crate::battle::BattlerState<Self>,
        _random: u8,
        _is_critical: bool,
    ) -> crate::battle::DamageResult {
        crate::battle::DamageResult {
            damage: 0,
            effectiveness: 1.0,
            is_miss: false,
        }
    }

    fn select_move(
        &self,
        _battler: &crate::battle::BattlerState<Self>,
        _state: &crate::battle::BattleState<Self>,
    ) -> Self::Move {
        MockMove(0)
    }

    fn apply_move_effect(
        &self,
        _effect: crate::battle::MoveEffect,
        _user: &mut crate::battle::BattlerState<Self>,
        _target: &mut crate::battle::BattlerState<Self>,
    ) -> crate::battle::EffectResult {
        crate::battle::EffectResult::NoEffect
    }

    fn create_monster(
        &self,
        species: Self::Species,
        level: u8,
    ) -> crate::battle::BattlerState<Self> {
        crate::battle::BattlerState::new(
            species,
            level as u16,
            level as u16,
            crate::battle::EnumMap::new(),
            Vec::new(),
        )
    }
}

#[test]
fn to_battler_maps_via_closures() {
    let mut m = mk(5);
    m.current_hp = 12;
    m.status = MonsterStatus::Sleep(3);
    m.moves.push(MoveSlot {
        move_id: MockMove(42),
        pp: 15,
        pp_up: 0,
    });
    // Identity closures here; in a real game these map P's ids to B's ids.
    let bs: crate::battle::BattlerState<MockBattle> =
        m.to_battler(&MockGame, |s| s, |mv| mv, |st| st);
    assert_eq!(bs.species, MockSpecies(1));
    assert_eq!(bs.hp, 12);
    assert_eq!(bs.max_hp, m.max_hp(&MockGame));
    assert_eq!(bs.moves, vec![MockMove(42)]);
    // Stats carried over through the EnumMap.
    let hp = m.max_hp(&MockGame);
    assert_eq!(bs.stats.get(MockStat::Hp), Some(&hp));
    // Stat stages start neutral (empty map) and status defaults to None.
    assert_eq!(bs.stat_stages.len(), 0);
    assert_eq!(bs.status, None);
}
