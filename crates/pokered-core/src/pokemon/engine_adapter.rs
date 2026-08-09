//! Adoption of the generic `dotzuki-engine` party/monster model (milestone P0a,
//! step 2).
//!
//! This module implements the engine's provider traits
//! ([`MonsterProvider`], [`ExpProvider`], [`EvolutionProvider`]) for pokered's
//! concrete data types, and provides an adapter that converts between pokered's
//! [`Pokemon`] and an engine [`MonsterInstance`].
//!
//! It is **purely additive**: pokered's `Pokemon` is left as-is. Re-pointing
//! `Pokemon` onto `MonsterInstance` (the blueprint's step 3) is intentionally
//! NOT done here — it touches the most-tested type plus the serde/save
//! snapshots, so it is staged for a follow-up. The provider impls plus the
//! round-trippable adapter below are the safe subset that makes the engine
//! generics usable from pokered today. The provider's stat/exp/evolution
//! queries delegate directly to the existing Gen-1 functions, so those numbers
//! match the legacy code.
//!
//! The engine's `MonsterInstance::gain_exp` leveling path is now a true Gen-1
//! **drop-in** for `process_level_up`: [`PokeredMonsters`] overrides the engine's
//! defaulted `ExpProvider::levelup_current_hp` hook to grow current HP by the
//! max-HP delta (`hp += new_max_hp - old_max_hp`) and the
//! `ExpProvider::learn_moves_on_levelup` hook to learn moves from the species
//! learnset — matching `battle::experience::level_up`. The differential test
//! `gain_exp_matches_process_level_up` drives a damaged, leveling Pokemon
//! through both paths and asserts identical HP, max HP, level, move slots, and
//! learned-move set. Production leveling still flows through `process_level_up`
//! (the `Pokemon`-struct re-point is staged); see
//! `docs/engine-gap-analysis/05-p0-migration-report.md` (party section).

use dotzuki_engine::party::{
    EvolutionProvider, EvolutionTrigger as EngineEvolutionTrigger, ExpProvider, MonsterInstance,
    MonsterProvider, MonsterStatus, MoveSlot, StatSet,
};

use pokered_data::evos_moves::evos_moves_data;
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::pokemon_data::get_base_stats;
use pokered_data::species::Species;

use crate::battle::experience::growth::{exp_for_level, max_exp};
use crate::battle::experience::level_up::process_level_up;
use crate::battle::experience::stats::calc_all_stats;
use crate::battle::settlement::evolution::{
    check_item_evolution, check_level_evolution, check_trade_evolution,
};
use crate::battle::state::{Pokemon, StatusCondition};

/// The five stats pokered uses. Order here defines the [`StatSet`] order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PokeredStat {
    /// Hit points.
    Hp,
    /// Physical attack.
    Attack,
    /// Physical defense.
    Defense,
    /// Speed.
    Speed,
    /// Special (combined in Gen 1).
    Special,
}

const POKERED_STATS: [PokeredStat; 5] = [
    PokeredStat::Hp,
    PokeredStat::Attack,
    PokeredStat::Defense,
    PokeredStat::Speed,
    PokeredStat::Special,
];

/// Per-instance genetics for pokered: the two packed Gen-1 DV bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DvBytes(pub [u8; 2]);

/// Per-instance training for pokered: stat-exp `[hp, atk, def, spd, spc]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatExp(pub [u16; 5]);

impl Default for StatExp {
    fn default() -> Self {
        StatExp([0; 5])
    }
}

/// Zero-sized provider binding pokered's data to the engine party model.
///
/// All numbers are sourced from the existing pokered species / stat / exp /
/// evolution tables, so the engine generics behave exactly like the
/// hand-rolled pokered logic.
#[derive(Debug, Clone, Copy, Default)]
pub struct PokeredMonsters;

impl MonsterProvider for PokeredMonsters {
    type SpeciesId = Species;
    type MoveId = MoveId;
    type Genetics = DvBytes;
    type Training = StatExp;
    type Stat = PokeredStat;

    fn base_stat(&self, species: Self::SpeciesId, stat: Self::Stat) -> u16 {
        match get_base_stats(species) {
            Some(base) => match stat {
                PokeredStat::Hp => base.hp as u16,
                PokeredStat::Attack => base.attack as u16,
                PokeredStat::Defense => base.defense as u16,
                PokeredStat::Speed => base.speed as u16,
                PokeredStat::Special => base.special as u16,
            },
            None => 0,
        }
    }

    fn calc_stat(
        &self,
        species: Self::SpeciesId,
        stat: Self::Stat,
        level: u8,
        genetics: &Self::Genetics,
        training: &Self::Training,
    ) -> u16 {
        // Delegate to pokered's authoritative Gen-1 stat formula so the engine
        // and the legacy code stay numerically identical.
        let Some(base) = get_base_stats(species) else {
            return 0;
        };
        let (hp, attack, defense, speed, special) =
            calc_all_stats(base, genetics.0, &training.0, level);
        match stat {
            PokeredStat::Hp => hp,
            PokeredStat::Attack => attack,
            PokeredStat::Defense => defense,
            PokeredStat::Speed => speed,
            PokeredStat::Special => special,
        }
    }

    fn stats(&self) -> &[Self::Stat] {
        &POKERED_STATS
    }

    fn hp_stat(&self) -> Self::Stat {
        PokeredStat::Hp
    }

    fn max_moves(&self) -> usize {
        4
    }
}

impl ExpProvider for PokeredMonsters {
    fn exp_for_level(&self, species: Self::SpeciesId, level: u8) -> u32 {
        match get_base_stats(species) {
            Some(base) => exp_for_level(base.growth_rate, level),
            None => 0,
        }
    }

    fn max_level(&self) -> u8 {
        100
    }

    /// Gen-1 HP-growth policy: grow current HP by the max-HP increase, mirroring
    /// `process_level_up` (`mon.hp += new_max_hp - old_max_hp`). Implemented with
    /// saturating arithmetic, exactly like the legacy code.
    fn levelup_current_hp(&self, old_max_hp: u16, new_max_hp: u16, current_hp: u16) -> u16 {
        let hp_delta = new_max_hp.saturating_sub(old_max_hp);
        current_hp.saturating_add(hp_delta)
    }

    /// Gen-1 move-learning on level-up: consult the species learnset, skip moves
    /// already known, fill the first empty slot, and replace the last slot if all
    /// four are full — matching `level_up::learn_move_at_level`.
    fn learn_moves_on_levelup(
        &self,
        species: Self::SpeciesId,
        level: u8,
        moves: &mut Vec<MoveSlot<Self>>,
    ) -> Vec<Self::MoveId> {
        let all_data = evos_moves_data();
        let Some(entry) = all_data.iter().find(|e| e.species == species) else {
            return Vec::new();
        };

        let mut learned = Vec::new();
        // A level can have more than one learnset entry; the legacy
        // `learn_move_at_level` uses `.find` (first match only), so mirror that
        // by stopping after the first move at this level.
        if let Some(lm) = entry.learnset.iter().find(|lm| lm.level == level) {
            let move_id = lm.move_id;
            // Already known: do not re-learn (legacy returns None).
            if moves.iter().any(|m| m.move_id == move_id) {
                return learned;
            }
            let pp = get_move_max_pp(move_id);
            if moves.len() < self.max_moves() {
                // First empty slot — in the Vec model an empty slot is simply an
                // absent entry, so append.
                moves.push(MoveSlot { move_id, pp, pp_up: 0 });
            } else {
                // All slots full: replace the last slot (the real game prompts
                // the player; legacy overwrites slot 3).
                if let Some(last) = moves.last_mut() {
                    last.move_id = move_id;
                    last.pp = pp;
                    last.pp_up = 0;
                }
            }
            learned.push(move_id);
        }
        learned
    }
}

fn get_move_max_pp(move_id: MoveId) -> u8 {
    use pokered_data::move_data::MOVES;
    MOVES
        .iter()
        .find(|m| m.id == move_id)
        .map(|m| m.pp)
        .unwrap_or(0)
}

impl EvolutionProvider for PokeredMonsters {
    type EvoItem = ItemId;

    fn evolution_target(
        &self,
        inst: &MonsterInstance<Self>,
        trigger: EngineEvolutionTrigger<Self::EvoItem>,
    ) -> Option<Self::SpeciesId> {
        // Delegate to pokered's existing evolution checks so the level/item/
        // trade conditions stay game-side.
        match trigger {
            EngineEvolutionTrigger::LevelUp => check_level_evolution(inst.species, inst.level),
            EngineEvolutionTrigger::Trade => check_trade_evolution(inst.species, inst.level),
            EngineEvolutionTrigger::Item(item) => {
                check_item_evolution(inst.species, inst.level, item)
            }
        }
    }
}

/// Re-export the engine trigger under a pokered-friendly name for callers.
pub type EvolutionTrigger = EngineEvolutionTrigger<ItemId>;

/// A pokered monster instance expressed in the generic engine model.
pub type PokeredMonster = MonsterInstance<PokeredMonsters>;

// ---- Adapter: Pokemon <-> MonsterInstance ----------------------------------

impl Pokemon {
    /// Build an engine [`MonsterInstance`] mirroring this pokered `Pokemon`.
    ///
    /// This is a *partial* projection. The engine model carries the
    /// battle/leveling-relevant fields (species, level, total_exp, DV bytes,
    /// stat-exp, computed stats, current HP, status, moves+PP). Fields the
    /// engine has no concept of — `nickname`, `type1`/`type2`, `is_traded`,
    /// `ot_id`/`ot_name`, the
    /// cached `max_hp`, and per-move `pp_ups` — are intentionally NOT
    /// represented and stay on the pokered side. `MoveSlot::pp_up` is taken from
    /// pokered's `pp_ups`, so PP-Ups *do* round-trip.
    ///
    /// Empty move slots (`MoveId::None`) are dropped: the engine model uses a
    /// variable-length `Vec`, so [`apply_monster_instance`] re-pads back to four
    /// slots.
    pub fn to_monster_instance(&self) -> PokeredMonster {
        let provider = PokeredMonsters;
        let mut stats = StatSet::zeroed(&provider);
        stats.set(PokeredStat::Hp, self.max_hp);
        stats.set(PokeredStat::Attack, self.attack);
        stats.set(PokeredStat::Defense, self.defense);
        stats.set(PokeredStat::Speed, self.speed);
        stats.set(PokeredStat::Special, self.special);

        let mut moves = Vec::new();
        for i in 0..4 {
            if self.moves[i] != MoveId::None {
                moves.push(MoveSlot {
                    move_id: self.moves[i],
                    pp: self.pp[i],
                    pp_up: self.pp_ups[i],
                });
            }
        }

        MonsterInstance {
            species: self.species,
            level: self.level,
            exp: self.total_exp,
            genetics: DvBytes(self.dv_bytes),
            training: StatExp(self.stat_exp),
            stats,
            current_hp: self.hp,
            status: status_to_engine(self.status),
            moves,
        }
    }

    /// Apply the engine-relevant fields of a [`MonsterInstance`] back onto this
    /// `Pokemon`, preserving the pokered-only fields (`nickname`, types,
    /// `is_traded`/`ot_id`/`ot_name`) and re-deriving the cached `max_hp`.
    ///
    /// Move slots are re-padded to the fixed `[MoveId; 4]` layout; trailing
    /// empty slots become `MoveId::None` with `pp = 0`.
    pub fn apply_monster_instance(&mut self, inst: &PokeredMonster) {
        self.species = inst.species;
        self.level = inst.level;
        self.total_exp = inst.exp;
        self.dv_bytes = inst.genetics.0;
        self.stat_exp = inst.training.0;
        self.max_hp = inst.stats.get(PokeredStat::Hp);
        self.attack = inst.stats.get(PokeredStat::Attack);
        self.defense = inst.stats.get(PokeredStat::Defense);
        self.speed = inst.stats.get(PokeredStat::Speed);
        self.special = inst.stats.get(PokeredStat::Special);
        self.hp = inst.current_hp;
        self.status = status_from_engine(inst.status);

        let mut moves = [MoveId::None; 4];
        let mut pp = [0u8; 4];
        let mut pp_ups = [0u8; 4];
        for (i, slot) in inst.moves.iter().take(4).enumerate() {
            moves[i] = slot.move_id;
            pp[i] = slot.pp;
            pp_ups[i] = slot.pp_up;
        }
        self.moves = moves;
        self.pp = pp;
        self.pp_ups = pp_ups;
    }
}

fn status_to_engine(s: StatusCondition) -> MonsterStatus {
    match s {
        StatusCondition::None => MonsterStatus::Healthy,
        StatusCondition::Sleep(t) => MonsterStatus::Sleep(t),
        StatusCondition::Poison => MonsterStatus::Poison,
        StatusCondition::Burn => MonsterStatus::Burn,
        StatusCondition::Freeze => MonsterStatus::Freeze,
        StatusCondition::Paralysis => MonsterStatus::Paralysis,
    }
}

fn status_from_engine(s: MonsterStatus) -> StatusCondition {
    match s {
        MonsterStatus::Healthy => StatusCondition::None,
        MonsterStatus::Sleep(t) => StatusCondition::Sleep(t),
        MonsterStatus::Poison => StatusCondition::Poison,
        MonsterStatus::Burn => StatusCondition::Burn,
        MonsterStatus::Freeze => StatusCondition::Freeze,
        MonsterStatus::Paralysis => StatusCondition::Paralysis,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_data::types::PokemonType;

    // Bulbasaur (id 1) has a level-up evolution (-> Ivysaur @16) in Gen-1 data.
    const TEST_SPECIES: Species = Species::Bulbasaur;

    fn sample() -> Pokemon {
        let provider = PokeredMonsters;
        let dv_bytes = [0x9A, 0xBC];
        let stat_exp = [100u16, 200, 300, 400, 500];
        let level = 12u8;
        let base = get_base_stats(TEST_SPECIES).unwrap();
        let (hp, attack, defense, speed, special) =
            calc_all_stats(base, dv_bytes, &stat_exp, level);
        let _ = &provider;
        Pokemon {
            species: TEST_SPECIES,
            nickname: Some("Bud".to_string()),
            level,
            hp: hp / 2, // damaged
            max_hp: hp,
            attack,
            defense,
            speed,
            special,
            type1: base.type1,
            type2: base.type2,
            moves: [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None],
            pp: [35, 40, 0, 0],
            pp_ups: [1, 0, 0, 0],
            status: StatusCondition::Poison,
            dv_bytes,
            stat_exp,
            total_exp: exp_for_level(base.growth_rate, level),
            is_traded: false, ot_id: 0, ot_name: None,
        }
    }

    #[test]
    fn round_trip_is_identity() {
        let original = sample();
        let inst = original.to_monster_instance();

        // Engine instance mirrors the engine-relevant fields.
        assert_eq!(inst.species, original.species);
        assert_eq!(inst.level, original.level);
        assert_eq!(inst.exp, original.total_exp);
        assert_eq!(inst.current_hp, original.hp);
        assert_eq!(inst.stats.get(PokeredStat::Hp), original.max_hp);
        assert_eq!(inst.stats.get(PokeredStat::Attack), original.attack);
        assert_eq!(inst.stats.get(PokeredStat::Special), original.special);
        // Empty move slots are dropped (2 real moves).
        assert_eq!(inst.moves.len(), 2);
        assert_eq!(inst.moves[0].move_id, MoveId::Tackle);
        assert_eq!(inst.moves[0].pp_up, 1);

        // Apply back onto a fresh Pokemon that still carries OT/type fields.
        let mut rebuilt = sample();
        rebuilt.hp = 0;
        rebuilt.status = StatusCondition::None;
        rebuilt.attack = 0;
        rebuilt.moves = [MoveId::None; 4];
        rebuilt.apply_monster_instance(&inst);

        // Full struct identity (Pokemon derives PartialEq): the round trip
        // preserves every engine-relevant field, and apply does not touch the
        // pokered-only fields (nickname/types/is_traded) carried by `rebuilt`.
        assert_eq!(rebuilt, original);
    }

    #[test]
    fn nickname_and_types_preserved_by_apply() {
        let original = sample();
        let inst = original.to_monster_instance();
        let mut target = Pokemon {
            nickname: Some("KEEPME".to_string()),
            type1: PokemonType::Grass,
            type2: PokemonType::Poison,
            is_traded: true,
            ..sample()
        };
        target.apply_monster_instance(&inst);
        // Pokered-only fields untouched by apply.
        assert_eq!(target.nickname, Some("KEEPME".to_string()));
        assert!(target.is_traded);
        // Engine-relevant fields adopted from the instance.
        assert_eq!(target.species, original.species);
        assert_eq!(target.hp, original.hp);
    }

    #[test]
    fn stat_calc_matches_pokered() {
        let provider = PokeredMonsters;
        let species = TEST_SPECIES;
        let dv_bytes = DvBytes([0x12, 0x34]);
        let se = StatExp([10, 20, 30, 40, 50]);
        let level = 25;
        let base = get_base_stats(species).unwrap();
        let (hp, attack, _d, _s, special) = calc_all_stats(base, dv_bytes.0, &se.0, level);
        assert_eq!(provider.calc_stat(species, PokeredStat::Hp, level, &dv_bytes, &se), hp);
        assert_eq!(
            provider.calc_stat(species, PokeredStat::Attack, level, &dv_bytes, &se),
            attack
        );
        assert_eq!(
            provider.calc_stat(species, PokeredStat::Special, level, &dv_bytes, &se),
            special
        );
    }

    #[test]
    fn exp_provider_matches_pokered() {
        let provider = PokeredMonsters;
        let species = TEST_SPECIES;
        let growth = get_base_stats(species).unwrap().growth_rate;
        for level in [1u8, 5, 50, 100] {
            assert_eq!(provider.exp_for_level(species, level), exp_for_level(growth, level));
        }
    }

    /// Build a damaged Bulbasaur sitting one EXP point below the level-7
    /// boundary. Bulbasaur learns LeechSeed at level 7, so leveling past it
    /// exercises **both** the HP-delta growth path and the move-learning path.
    fn damaged_levelable() -> Pokemon {
        let base = get_base_stats(TEST_SPECIES).unwrap();
        let dv_bytes = [0x9A, 0xBC];
        let stat_exp = [120u16, 240, 360, 0, 80];
        let level = 6u8;
        let (hp, attack, defense, speed, special) =
            calc_all_stats(base, dv_bytes, &stat_exp, level);
        Pokemon {
            species: TEST_SPECIES,
            nickname: Some("Bud".to_string()),
            level,
            hp: (hp / 3).max(1), // clearly damaged, below max
            max_hp: hp,
            attack,
            defense,
            speed,
            special,
            type1: base.type1,
            type2: base.type2,
            // Packed front-to-back (two real moves, two empty) so the engine's
            // Vec model and the legacy [MoveId;4] model agree on slot placement.
            moves: [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None],
            pp: [35, 40, 0, 0],
            pp_ups: [0, 0, 0, 0],
            status: StatusCondition::None,
            dv_bytes,
            stat_exp,
            total_exp: exp_for_level(base.growth_rate, level),
            is_traded: false, ot_id: 0, ot_name: None,
        }
    }

    /// Differential parity: a damaged, leveling Pokemon driven through BOTH the
    /// engine `gain_exp` path (with pokered's HP-delta + move-learning hooks) and
    /// the legacy `process_level_up` path must end with identical current HP, max
    /// HP, level, move slots, PP, and learned-move set. This replaces the old
    /// tautological `gain_exp_via_engine_levels_up` test and is what proves the
    /// engine path is now a true Gen-1 drop-in.
    #[test]
    fn gain_exp_matches_process_level_up() {
        let provider = PokeredMonsters;
        let base = get_base_stats(TEST_SPECIES).unwrap();

        // EXP needed to climb from level 6 to level 9 (crosses LeechSeed@7).
        let start = damaged_levelable();
        let target_level = 9u8;
        let target_exp = exp_for_level(base.growth_rate, target_level);
        let delta = target_exp.saturating_sub(start.total_exp);
        assert!(delta > 0, "test must actually level up");

        // ---- Legacy path: process_level_up on the raw Pokemon ----
        let mut legacy = damaged_levelable();
        legacy.total_exp = legacy
            .total_exp
            .saturating_add(delta)
            .min(max_exp(base.growth_rate));
        let legacy_result = process_level_up(&mut legacy);
        assert!(legacy_result.leveled_up, "legacy path must level up");

        // ---- Engine path: gain_exp on the MonsterInstance, applied back ----
        let mut inst = damaged_levelable().to_monster_instance();
        let engine_result = inst.gain_exp(&provider, delta);
        let mut engine = damaged_levelable();
        engine.apply_monster_instance(&inst);

        // Level identical.
        assert_eq!(engine.level, legacy.level, "level mismatch");
        assert_eq!(engine_result.new_level, legacy_result.new_level);

        // Max HP and (crucially) grown current HP identical — proves the
        // HP-delta policy hook matches `hp += new_max_hp - old_max_hp`.
        assert_eq!(engine.max_hp, legacy.max_hp, "max_hp mismatch");
        assert_eq!(engine.hp, legacy.hp, "current hp grow-by-delta mismatch");

        // All other stats identical.
        assert_eq!(engine.attack, legacy.attack);
        assert_eq!(engine.defense, legacy.defense);
        assert_eq!(engine.speed, legacy.speed);
        assert_eq!(engine.special, legacy.special);

        // Move slots + PP identical — proves the move-learning hook matches.
        assert_eq!(engine.moves, legacy.moves, "learned move slots mismatch");
        assert_eq!(engine.pp, legacy.pp, "learned move PP mismatch");

        // Learned-move SETS identical (LeechSeed must appear in both).
        let mut engine_learned = engine_result.learned_moves.clone();
        let mut legacy_learned = legacy_result.learned_moves.clone();
        engine_learned.sort();
        legacy_learned.sort();
        assert_eq!(engine_learned, legacy_learned, "learned-move set mismatch");
        assert!(
            legacy_learned.contains(&MoveId::LeechSeed),
            "sanity: LeechSeed should be learned crossing level 7"
        );
    }

    #[test]
    fn evolution_via_engine_matches_settlement() {
        let provider = PokeredMonsters;
        let mut inst = sample().to_monster_instance();
        inst.level = 16;
        inst.recalc_stats(&provider);
        let engine_result = inst.try_evolve(&provider, EngineEvolutionTrigger::LevelUp);
        let direct = check_level_evolution(TEST_SPECIES, 16);
        assert_eq!(engine_result, direct);
        if let Some(target) = direct {
            assert_eq!(inst.species, target);
        }
    }
}
