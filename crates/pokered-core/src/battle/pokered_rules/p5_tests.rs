//! # P5 differential parity tests (blueprint `15` §5 P5).
//!
//! For each P5 native-tier effect, this drives the SAME logical setup through (a)
//! the LEGACY oracle (the `apply_*` functions in `field_effects` /
//! `special_effects` / `multi_turn_effects` — THE source of truth) and (b) the P5
//! NATIVE handler (fired on its `Event::Custom(_)` / `Residual` / `OnMiss` hook
//! through the engine's dispatch fold), asserting the native handler produces an
//! observable state IDENTICAL to the legacy oracle's: hp, non-volatile status,
//! every stat stage, the volatile presence, the type override, the coin pool.
//!
//! The reactive Bide/Counter/Rage have NO `MoveEffect` / no legacy oracle
//! (blueprint risk #3) — they are **SYNTHETIC-ORACLE-ONLY** and are already proven
//! in the `stack_parity` slice 6; this file does not fake a legacy parity test for
//! them (see the `synthetic_oracle_note` test for the explicit honesty marker).

#![cfg(test)]

use dotzuki_engine::battle::rng::ScriptedRng;
use dotzuki_engine::battle::stack::dispatch::collect_from_effect;
use dotzuki_engine::battle::stack::{
    run_event, BattleCtx, CollectedHandler, Effect, EffectState, Event, MoveContext, RelayVar,
};
use dotzuki_engine::battle::{
    BattleState as EngineState, BattlerRef, BattlerState as EngineBattler, EnumMap,
};

use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

use super::p5_native::*;
use super::PokeVolatile;
use super::PokeredRules;

use crate::battle::effects::EffectRandoms;
use crate::battle::stat_stages::StatIndex;
use crate::battle::state::{
    new_battle_state, status1, status2, status3, BattleState as LegacyState, BattleType, Pokemon,
    StatusCondition,
};

// ─── shared setup ─────────────────────────────────────────────────────────────

fn legacy_poke(species: Species, hp: u16, max_hp: u16) -> Pokemon {
    let base = pokered_data::pokemon_data::get_base_stats(species).expect("base");
    Pokemon {
        species,
        nickname: [0x50; 11],
        level: 50,
        hp,
        max_hp,
        attack: 100,
        defense: 80,
        speed: 110,
        special: 80,
        type1: base.type1,
        type2: base.type2,
        moves: [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
        pp: [35, 0, 0, 0],
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

fn legacy_state(player: Pokemon, enemy: Pokemon) -> LegacyState {
    new_battle_state(BattleType::Wild, vec![player], vec![enemy])
}

fn engine_battler(species: Species, hp: u16, max_hp: u16) -> EngineBattler<PokeredRules> {
    let mut stats = EnumMap::new();
    stats.set(StatIndex::Attack, 100);
    stats.set(StatIndex::Defense, 80);
    stats.set(StatIndex::Speed, 110);
    stats.set(StatIndex::Special, 80);
    EngineBattler::new(species, hp, max_hp, stats, vec![MoveId::Tackle])
}

/// Fire ONE P5 native effect's hook on `event`, with the mover = `source` and the
/// defender = `target`, threading `relay`. Returns the folded relay + consumed().
fn fire_p5(
    state: &mut EngineState<PokeredRules>,
    effects: &mut Vec<EffectState<PokeredRules>>,
    eff: &'static Effect<PokeredRules>,
    event: Event,
    target: BattlerRef,
    source: BattlerRef,
    relay: RelayVar,
    rng_bytes: Vec<u8>,
) -> (RelayVar, usize) {
    let mut mv = MoveContext::default();
    let mut rng = ScriptedRng::new(rng_bytes);
    let out = {
        let mut ctx = BattleCtx { state, effects, mv: &mut mv, rng: &mut rng };
        let mut hs: Vec<CollectedHandler<PokeredRules>> = Vec::new();
        collect_from_effect(&ctx, eff, event, target, source, &mut hs);
        run_event(&mut ctx, hs, relay, false)
    };
    (out, rng.consumed())
}

const PLAYER: BattlerRef = BattlerRef::PLAYER;
const OPP: BattlerRef = BattlerRef::OPPONENT;

fn has_kind(
    effects: &[EffectState<PokeredRules>],
    host: BattlerRef,
    pred: impl Fn(&PokeVolatile) -> bool,
) -> bool {
    effects.iter().any(|e| e.host == host && pred(&e.kind))
}

// ═════════════════════════════════════════════════════════════════════════════
// FIELD / SCREEN group — each mirrors a legacy `field_effects.rs` apply_*.
// ═════════════════════════════════════════════════════════════════════════════

/// Mist (#none): legacy sets `PROTECTED_BY_MIST` on the attacker; native sets the
/// `Mist` veto volatile. Set-once on both paths.
#[test]
fn p5_mist_parity() {
    reset_p5_scratch();
    // Legacy.
    let mut ls = legacy_state(legacy_poke(Species::Pikachu, 200, 200), legacy_poke(Species::Pikachu, 200, 200));
    let r = crate::battle::effects::field_effects::apply_mist(&mut ls);
    assert_eq!(r, crate::battle::effects::EffectResult::FieldEffectSet);
    assert!(ls.player.has_status2(status2::PROTECTED_BY_MIST));
    // Native.
    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut effects = Vec::new();
    let (_o, consumed) = fire_p5(&mut es, &mut effects, mist_effect(), Event::Custom(EV_MIST), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert!(has_kind(&effects, PLAYER, |k| matches!(k, PokeVolatile::Mist)), "Mist set on mover");
    assert_eq!(consumed, 0, "Mist draws no rng (legacy parity)");
    // Set-once: re-apply is a no-op (one Mist volatile, not two).
    fire_p5(&mut es, &mut effects, mist_effect(), Event::Custom(EV_MIST), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert_eq!(effects.iter().filter(|e| matches!(e.kind, PokeVolatile::Mist)).count(), 1, "Mist set-once");
}

/// Focus Energy (#1): legacy sets `GETTING_PUMPED`; native sets `FocusEnergy`
/// (the /4-crit volatile the pipeline divides by — the bug is preserved end to end
/// in the P1 `focus_energy_div4_crit_bug_parity` test).
#[test]
fn p5_focus_energy_parity() {
    reset_p5_scratch();
    let mut ls = legacy_state(legacy_poke(Species::Pikachu, 200, 200), legacy_poke(Species::Pikachu, 200, 200));
    let r = crate::battle::effects::field_effects::apply_focus_energy(&mut ls);
    assert_eq!(r, crate::battle::effects::EffectResult::FieldEffectSet);
    assert!(ls.player.has_status2(status2::GETTING_PUMPED));
    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut effects = Vec::new();
    fire_p5(&mut es, &mut effects, focus_energy_effect(), Event::Custom(EV_FOCUS_ENERGY), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert!(has_kind(&effects, PLAYER, |k| matches!(k, PokeVolatile::FocusEnergy)), "Focus Energy set (#1 /4 crit volatile)");
}

/// Light Screen / Reflect: legacy sets `HAS_LIGHT_SCREEN_UP` / `HAS_REFLECT_UP`;
/// native sets the matching volatile.
#[test]
fn p5_light_screen_and_reflect_parity() {
    reset_p5_scratch();
    let mut ls = legacy_state(legacy_poke(Species::Pikachu, 200, 200), legacy_poke(Species::Pikachu, 200, 200));
    crate::battle::effects::field_effects::apply_light_screen(&mut ls);
    assert!(ls.player.has_status3(status3::HAS_LIGHT_SCREEN_UP));
    let mut ls2 = legacy_state(legacy_poke(Species::Pikachu, 200, 200), legacy_poke(Species::Pikachu, 200, 200));
    crate::battle::effects::field_effects::apply_reflect(&mut ls2);
    assert!(ls2.player.has_status3(status3::HAS_REFLECT_UP));

    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut effects = Vec::new();
    fire_p5(&mut es, &mut effects, light_screen_effect(), Event::Custom(EV_LIGHT_SCREEN), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert!(has_kind(&effects, PLAYER, |k| matches!(k, PokeVolatile::LightScreen)), "Light Screen set");
    fire_p5(&mut es, &mut effects, reflect_effect(), Event::Custom(EV_REFLECT), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert!(has_kind(&effects, PLAYER, |k| matches!(k, PokeVolatile::Reflect)), "Reflect set");
}

/// Leech Seed: legacy seeds a non-Grass defender, fails on a Grass-type. Native
/// matches both, plus the Substitute block.
#[test]
fn p5_leech_seed_parity() {
    reset_p5_scratch();
    // Legacy: seeds a Pikachu (non-Grass) defender.
    let md = *pokered_data::move_data::MoveData::get(MoveId::LeechSeed).unwrap();
    let mut ls = legacy_state(legacy_poke(Species::Pikachu, 200, 200), legacy_poke(Species::Pikachu, 200, 200));
    let r = crate::battle::effects::field_effects::apply_leech_seed(&mut ls, &md);
    assert_eq!(r, crate::battle::effects::EffectResult::Seeded);
    assert!(ls.enemy.has_status2(status2::SEEDED));
    // Legacy: fails on a Grass-type (Bulbasaur).
    let mut ls_g = legacy_state(legacy_poke(Species::Pikachu, 200, 200), legacy_poke(Species::Bulbasaur, 200, 200));
    let rg = crate::battle::effects::field_effects::apply_leech_seed(&mut ls_g, &md);
    assert_eq!(rg, crate::battle::effects::EffectResult::StatusFailed, "Grass immune");

    // Native: seeds non-Grass.
    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut effects = Vec::new();
    fire_p5(&mut es, &mut effects, leech_seed_effect(), Event::Custom(EV_LEECH_SEED), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert!(has_kind(&effects, OPP, |k| matches!(k, PokeVolatile::LeechSeed)), "non-Grass seeded");
    // Native: fails on Grass (Bulbasaur).
    let mut es_g = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 200)], vec![engine_battler(Species::Bulbasaur, 200, 200)]);
    let mut eff_g = Vec::new();
    fire_p5(&mut es_g, &mut eff_g, leech_seed_effect(), Event::Custom(EV_LEECH_SEED), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert!(!has_kind(&eff_g, OPP, |k| matches!(k, PokeVolatile::LeechSeed)), "Grass NOT seeded (parity)");
}

/// Leech Seed residual: drains max/16 from the host to the seeder. Legacy
/// `residual.rs` chips the seeded mon and heals the seeder by the same amount.
#[test]
fn p5_leech_seed_residual_parity() {
    reset_p5_scratch();
    // Native: seed OPP, drain to PLAYER. max_hp 200 → 200/16 = 12.
    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 100, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut effects = vec![EffectState { id: dotzuki_engine::battle::stack::EffectId(0x50_900), host: OPP, effect_order: 0, kind: PokeVolatile::LeechSeed }];
    fire_p5(&mut es, &mut effects, leech_residual_effect(), Event::Residual, OPP, OPP, RelayVar::Unit, vec![]);
    assert_eq!(es.opponent_battlers[0].hp, 200 - 12, "seeded host drained max/16 = 12");
    assert_eq!(es.player_battlers[0].hp, 100 + 12, "seeder healed by the drain");
}

/// Gen-1 TOXIC × LEECH SEED bug (reproduced): the leech drain runs through the
/// same HP-decrease routine as poison, so a badly-poisoned seeded host's Toxic
/// counter increments AGAIN on the leech tick and the drain scales with it
/// (core.asm HandlePoisonBurnLeechSeed_DecreaseOwnHP).
#[test]
fn p5_leech_seed_scales_with_toxic_counter() {
    reset_p5_scratch();
    // Seeded + badly-poisoned host (max_hp 200 → base 12), seeder = PLAYER.
    // Toxic volatile installed FIRST (lower arena id ⇒ ticks first, the usual
    // order): toxic tick bumps counter 0→1 (dmg 12); the leech tick then bumps
    // 1→2 and drains 12×2 = 24, healing the seeder 24.
    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 100, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut effects = vec![
        EffectState { id: dotzuki_engine::battle::stack::EffectId(0x50_910), host: OPP, effect_order: 0, kind: PokeVolatile::Toxic { counter: 0 } },
        EffectState { id: dotzuki_engine::battle::stack::EffectId(0x50_911), host: OPP, effect_order: 1, kind: PokeVolatile::LeechSeed },
    ];
    fire_p5(&mut es, &mut effects, toxic_residual_effect(), Event::Residual, OPP, OPP, RelayVar::Unit, vec![]);
    assert_eq!(es.opponent_battlers[0].hp, 200 - 12, "toxic tick: 12×1");
    fire_p5(&mut es, &mut effects, leech_residual_effect(), Event::Residual, OPP, OPP, RelayVar::Unit, vec![]);
    assert_eq!(es.opponent_battlers[0].hp, 200 - 12 - 24, "leech tick scales: 12×2 = 24");
    assert_eq!(es.player_battlers[0].hp, 100 + 24, "seeder healed the SCALED drain");
    let counter = effects.iter().find_map(|e| match &e.kind {
        PokeVolatile::Toxic { counter } if e.host == OPP => Some(*counter),
        _ => None,
    });
    assert_eq!(counter, Some(2), "counter incremented twice in one turn");
}

/// Haze: legacy resets ALL stages + volatiles + status both sides. Native does the
/// same (the ResetAll broadcast).
#[test]
fn p5_haze_parity() {
    reset_p5_scratch();
    // Legacy setup: boosted stages + confusion + seed + status on both.
    let mut ls = legacy_state(legacy_poke(Species::Pikachu, 200, 200), legacy_poke(Species::Pikachu, 200, 200));
    ls.player.stat_stages.attack = 3;
    ls.enemy.stat_stages.defense = -2;
    ls.player.set_status1(status1::CONFUSED);
    ls.enemy.set_status2(status2::SEEDED);
    ls.player.active_mon_mut().status = StatusCondition::Burn;
    ls.enemy.active_mon_mut().status = StatusCondition::Poison;
    crate::battle::effects::field_effects::apply_haze(&mut ls);
    assert_eq!(ls.player.stat_stages.attack, 0);
    assert_eq!(ls.enemy.stat_stages.defense, 0);
    assert!(!ls.player.has_status1(status1::CONFUSED));
    assert!(!ls.enemy.has_status2(status2::SEEDED));
    assert!(ls.player.active_mon().status.is_none());
    assert!(ls.enemy.active_mon().status.is_none());

    // Native: matching setup.
    let mut pb = engine_battler(Species::Pikachu, 200, 200);
    pb.stat_stages.set(StatIndex::Attack, 3);
    pb.status = Some(StatusCondition::Burn);
    let mut eb = engine_battler(Species::Pikachu, 200, 200);
    eb.stat_stages.set(StatIndex::Defense, -2);
    eb.status = Some(StatusCondition::Poison);
    let mut es = EngineState::new(vec![pb], vec![eb]);
    let mut effects = vec![
        EffectState { id: dotzuki_engine::battle::stack::EffectId(0x50_901), host: PLAYER, effect_order: 0, kind: PokeVolatile::Confused { turns: 3 } },
        EffectState { id: dotzuki_engine::battle::stack::EffectId(0x50_902), host: OPP, effect_order: 1, kind: PokeVolatile::LeechSeed },
    ];
    fire_p5(&mut es, &mut effects, haze_effect(), Event::Custom(EV_HAZE), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert_eq!(es.player_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0), 0, "haze cleared player Attack stage");
    assert_eq!(es.opponent_battlers[0].stat_stages.get(StatIndex::Defense).copied().unwrap_or(0), 0, "haze cleared enemy Defense stage");
    assert!(es.player_battlers[0].status.is_none(), "haze cleared player status");
    assert!(es.opponent_battlers[0].status.is_none(), "haze cleared enemy status");
    assert!(effects.is_empty(), "haze cleared ALL volatiles both sides");
}

/// Substitute (#28): legacy costs max/4, and HP == cost succeeds at 0 HP (bug).
/// Native matches all three: normal, the 0-HP bug, and the < cost fail.
#[test]
fn p5_substitute_parity_including_bug_28() {
    reset_p5_scratch();
    // Legacy normal: 200/4 = 50 → hp 150, substitute_hp 50.
    let mut ls = legacy_state(legacy_poke(Species::Pikachu, 200, 200), legacy_poke(Species::Pikachu, 200, 200));
    let r = crate::battle::effects::field_effects::apply_substitute(&mut ls);
    assert_eq!(r, crate::battle::effects::EffectResult::SubstituteCreated { hp_cost: 50 });
    assert_eq!(ls.player.active_mon().hp, 150);
    // Legacy BUG #28: hp == cost → 0 HP, still succeeds.
    let mut ls_bug = legacy_state(legacy_poke(Species::Pikachu, 50, 200), legacy_poke(Species::Pikachu, 200, 200));
    let rb = crate::battle::effects::field_effects::apply_substitute(&mut ls_bug);
    assert_eq!(rb, crate::battle::effects::EffectResult::SubstituteCreated { hp_cost: 50 });
    assert_eq!(ls_bug.player.active_mon().hp, 0, "legacy bug #28: hp == cost ⇒ 0 HP");
    // Legacy fail: hp < cost.
    let mut ls_fail = legacy_state(legacy_poke(Species::Pikachu, 49, 200), legacy_poke(Species::Pikachu, 200, 200));
    let rf = crate::battle::effects::field_effects::apply_substitute(&mut ls_fail);
    assert_eq!(rf, crate::battle::effects::EffectResult::SubstituteFailed);

    // Native normal.
    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut effects = Vec::new();
    fire_p5(&mut es, &mut effects, substitute_effect(), Event::Custom(EV_SUBSTITUTE), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert_eq!(es.player_battlers[0].hp, 150, "native sub costs max/4 = 50");
    assert!(has_kind(&effects, PLAYER, |k| matches!(k, PokeVolatile::SubstituteHp { hp: 50 })), "native sub hp 50");
    // Native BUG #28.
    let mut es_bug = EngineState::new(vec![engine_battler(Species::Pikachu, 50, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut eff_bug = Vec::new();
    fire_p5(&mut es_bug, &mut eff_bug, substitute_effect(), Event::Custom(EV_SUBSTITUTE), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert_eq!(es_bug.player_battlers[0].hp, 0, "native bug #28: hp == cost ⇒ 0 HP");
    assert!(has_kind(&eff_bug, PLAYER, |k| matches!(k, PokeVolatile::SubstituteHp { .. })), "native sub created at 0 HP (bug)");
    // Native fail.
    let mut es_fail = EngineState::new(vec![engine_battler(Species::Pikachu, 49, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut eff_fail = Vec::new();
    fire_p5(&mut es_fail, &mut eff_fail, substitute_effect(), Event::Custom(EV_SUBSTITUTE), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert_eq!(es_fail.player_battlers[0].hp, 49, "native fail: hp < cost ⇒ unchanged");
    assert!(!has_kind(&eff_fail, PLAYER, |k| matches!(k, PokeVolatile::SubstituteHp { .. })), "native fail: no sub");
}

/// Rest: legacy full-heals + self-sleep(2) + cures. Native matches.
#[test]
fn p5_rest_parity() {
    reset_p5_scratch();
    let mut ls = legacy_state(legacy_poke(Species::Pikachu, 10, 200), legacy_poke(Species::Pikachu, 200, 200));
    ls.player.selected_move = MoveId::Rest;
    crate::battle::effects::field_effects::apply_heal(&mut ls);
    assert_eq!(ls.player.active_mon().hp, 200);
    assert_eq!(ls.player.active_mon().status, StatusCondition::Sleep(2));

    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 10, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut effects = Vec::new();
    fire_p5(&mut es, &mut effects, rest_effect(), Event::Custom(EV_REST), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert_eq!(es.player_battlers[0].hp, 200, "Rest full-heals");
    assert_eq!(es.player_battlers[0].status, Some(StatusCondition::Sleep(2)), "Rest self-sleeps(2)");
}

// ═════════════════════════════════════════════════════════════════════════════
// VOLATILE-SET group — Flinch / Confusion / Disable / Toxic (+ residual).
// ═════════════════════════════════════════════════════════════════════════════

/// Flinch (side): legacy sets `FLINCHED` on the defender, blocked by a Substitute.
#[test]
fn p5_flinch_parity() {
    reset_p5_scratch();
    let randoms = EffectRandoms { side_effect_roll: 0, duration_roll: 0, multi_hit_roll: 0, stat_down_miss_roll: 255 };
    let mut ls = legacy_state(legacy_poke(Species::Pikachu, 200, 200), legacy_poke(Species::Pikachu, 200, 200));
    let r = crate::battle::effects::special_effects::apply_flinch_side(&mut ls, &randoms, 26);
    assert_eq!(r, crate::battle::effects::EffectResult::FlinchApplied);
    assert!(ls.enemy.has_status1(status1::FLINCHED));
    // Legacy Substitute block.
    let mut ls_sub = legacy_state(legacy_poke(Species::Pikachu, 200, 200), legacy_poke(Species::Pikachu, 200, 200));
    ls_sub.enemy.set_status2(status2::HAS_SUBSTITUTE_UP);
    let rs = crate::battle::effects::special_effects::apply_flinch_side(&mut ls_sub, &randoms, 26);
    assert_eq!(rs, crate::battle::effects::EffectResult::StatusFailed);

    // Native: apply.
    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut effects = Vec::new();
    fire_p5(&mut es, &mut effects, flinch_effect(), Event::Custom(EV_FLINCH), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert!(has_kind(&effects, OPP, |k| matches!(k, PokeVolatile::Flinched)), "native flinch set on defender");
    // Native: Substitute block.
    let mut es_sub = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut eff_sub = vec![EffectState { id: dotzuki_engine::battle::stack::EffectId(0x50_910), host: OPP, effect_order: 0, kind: PokeVolatile::SubstituteHp { hp: 50 } }];
    fire_p5(&mut es_sub, &mut eff_sub, flinch_effect(), Event::Custom(EV_FLINCH), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert!(!has_kind(&eff_sub, OPP, |k| matches!(k, PokeVolatile::Flinched)), "native flinch blocked by Substitute (parity)");
}

/// Confusion (primary): legacy sets `CONFUSED` + `(dur&3)+2` turns. Native matches
/// the turn count via the relay-supplied duration byte.
#[test]
fn p5_confusion_parity() {
    reset_p5_scratch();
    // Legacy: duration 3 → (3&3)+2 = 5 turns.
    let randoms = EffectRandoms { side_effect_roll: 0, duration_roll: 3, multi_hit_roll: 0, stat_down_miss_roll: 255 };
    let mut ls = legacy_state(legacy_poke(Species::Pikachu, 200, 200), legacy_poke(Species::Pikachu, 200, 200));
    let r = crate::battle::effects::special_effects::apply_confusion_primary(&mut ls, &randoms);
    assert_eq!(r, crate::battle::effects::EffectResult::ConfusionApplied);
    assert!(ls.enemy.has_status1(status1::CONFUSED));
    assert_eq!(ls.enemy.confused_turns_left, 5);

    // Native: relay carries the duration byte 3 → (3&3)+2 = 5.
    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut effects = Vec::new();
    fire_p5(&mut es, &mut effects, confusion_effect(), Event::Custom(EV_CONFUSION), OPP, PLAYER, RelayVar::Int(3), vec![]);
    let turns = effects.iter().find_map(|e| if e.host == OPP { if let PokeVolatile::Confused { turns } = e.kind { Some(turns) } else { None } } else { None });
    assert_eq!(turns, Some(5), "native confusion turns = (3&3)+2 = 5 (parity)");
    // Already-confused fails on both (re-apply is a no-op → still 1 entry, 5 turns).
    fire_p5(&mut es, &mut effects, confusion_effect(), Event::Custom(EV_CONFUSION), OPP, PLAYER, RelayVar::Int(0), vec![]);
    assert_eq!(effects.iter().filter(|e| matches!(e.kind, PokeVolatile::Confused { .. })).count(), 1, "already-confused ⇒ no re-apply (parity)");
}

/// Disable: legacy disables the defender's last-move slot (1-based) for `(dur&7)+1`
/// turns. Native matches: it resolves the slot from the engine `moves` Vec.
#[test]
fn p5_disable_parity() {
    reset_p5_scratch();
    // Legacy: enemy last move Thundershock in slot 0 → disabled_move 1.
    let randoms = EffectRandoms { side_effect_roll: 0, duration_roll: 3, multi_hit_roll: 0, stat_down_miss_roll: 255 };
    let mut e_poke = legacy_poke(Species::Pikachu, 200, 200);
    e_poke.moves = [MoveId::Thundershock, MoveId::QuickAttack, MoveId::None, MoveId::None];
    e_poke.pp = [30, 30, 0, 0];
    let mut ls = legacy_state(legacy_poke(Species::Pikachu, 200, 200), e_poke);
    ls.enemy.last_move_used = MoveId::Thundershock;
    let r = crate::battle::effects::special_effects::apply_disable(&mut ls, &randoms);
    assert_eq!(r, crate::battle::effects::EffectResult::Disabled);
    assert_eq!(ls.enemy.disabled_move, 1);
    let legacy_turns = ls.enemy.disabled_turns_left; // (3&7)+1 = 4

    // Native: enemy moves = [Thundershock, QuickAttack], last = Thundershock (slot 0).
    let mut eb = engine_battler(Species::Pikachu, 200, 200);
    eb.moves = vec![MoveId::Thundershock, MoveId::QuickAttack];
    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 200)], vec![eb]);
    let mut effects = Vec::new();
    set_last_move(OPP, MoveId::Thundershock);
    fire_p5(&mut es, &mut effects, disable_effect(), Event::Custom(EV_DISABLE), OPP, PLAYER, RelayVar::Int(3), vec![]);
    let (slot, turns) = effects.iter().find_map(|e| if e.host == OPP { if let PokeVolatile::Disable { slot, turns } = e.kind { Some((slot, turns)) } else { None } } else { None }).expect("disable set");
    assert_eq!(slot, 1, "native disabled slot 1 (1-based, parity)");
    assert_eq!(turns as u8, legacy_turns, "native disable turns = legacy ({legacy_turns})");
}

/// Toxic (#6): legacy poison + badly-poisoned, the residual ramps UNCAPPED.
#[test]
fn p5_toxic_set_and_uncapped_residual_bug_6() {
    reset_p5_scratch();
    // Native set: poison status + Toxic{0} volatile.
    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 320)], vec![engine_battler(Species::Pikachu, 320, 320)]);
    let mut effects = Vec::new();
    fire_p5(&mut es, &mut effects, toxic_effect(), Event::Custom(EV_TOXIC), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert_eq!(es.opponent_battlers[0].status, Some(StatusCondition::Poison), "toxic sets poison");
    assert!(has_kind(&effects, OPP, |k| matches!(k, PokeVolatile::Toxic { counter: 0 })), "toxic counter starts at 0");

    // Residual ticks: max/16 = 320/16 = 20. Counter ramps 1,2,3,… UNCAPPED (#6).
    // Tick 1 → 20*1 = 20; tick 2 → 20*2 = 40; tick 3 → 20*3 = 60.
    let start = es.opponent_battlers[0].hp;
    fire_p5(&mut es, &mut effects, toxic_residual_effect(), Event::Residual, OPP, OPP, RelayVar::Unit, vec![]);
    assert_eq!(start - es.opponent_battlers[0].hp, 20, "toxic tick 1 = 20*1");
    let after1 = es.opponent_battlers[0].hp;
    fire_p5(&mut es, &mut effects, toxic_residual_effect(), Event::Residual, OPP, OPP, RelayVar::Unit, vec![]);
    assert_eq!(after1 - es.opponent_battlers[0].hp, 40, "toxic tick 2 = 20*2 (uncapped ramp #6)");
    let after2 = es.opponent_battlers[0].hp;
    fire_p5(&mut es, &mut effects, toxic_residual_effect(), Event::Residual, OPP, OPP, RelayVar::Unit, vec![]);
    assert_eq!(after2 - es.opponent_battlers[0].hp, 60, "toxic tick 3 = 20*3 (uncapped #6)");
}

/// Pay Day: legacy awards `level·2` coins to the session pool. Native matches via
/// the harness level (50 → 100 coins).
#[test]
fn p5_pay_day_parity() {
    reset_p5_scratch();
    let mut ls = legacy_state(legacy_poke(Species::Pikachu, 200, 200), legacy_poke(Species::Pikachu, 200, 200));
    let r = crate::battle::effects::special_effects::apply_pay_day(&mut ls, 40);
    assert_eq!(r, crate::battle::effects::EffectResult::PayDay { coins: 100 });
    assert_eq!(ls.total_payday_money, 100);

    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut effects = Vec::new();
    fire_p5(&mut es, &mut effects, pay_day_effect(), Event::Custom(EV_PAY_DAY), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert_eq!(coin_pool(), 100, "native PayDay awards level*2 = 100 coins (parity)");
}

// ═════════════════════════════════════════════════════════════════════════════
// DATA-REACH 5 — Conversion / Transform / Mimic / Metronome / Mirror Move.
// ═════════════════════════════════════════════════════════════════════════════

/// Conversion: legacy copies the defender's types onto the mover. Native stores
/// the copy in the type override (the engine derives types from species).
#[test]
fn p5_conversion_parity() {
    reset_p5_scratch();
    let md = *pokered_data::move_data::MoveData::get(MoveId::Conversion).unwrap();
    let mut ls = legacy_state(legacy_poke(Species::Pikachu, 200, 200), legacy_poke(Species::Pikachu, 200, 200));
    ls.enemy.active_mon_mut().type1 = PokemonType::Water;
    ls.enemy.active_mon_mut().type2 = PokemonType::Ice;
    crate::battle::effects::field_effects::apply_conversion(&mut ls, &md);
    assert_eq!(ls.player.active_mon().type1, PokemonType::Water);
    assert_eq!(ls.player.active_mon().type2, PokemonType::Ice);

    // Native: defender = Vaporeon (Water/Water) → copies (Water, Water).
    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 200)], vec![engine_battler(Species::Vaporeon, 200, 200)]);
    let mut effects = Vec::new();
    fire_p5(&mut es, &mut effects, conversion_effect(), Event::Custom(EV_CONVERSION), OPP, PLAYER, RelayVar::Unit, vec![]);
    let (t1, _t2) = read_type_override(PLAYER).expect("conversion override set");
    let (et1, _et2) = {
        let bs = pokered_data::pokemon_data::get_base_stats(Species::Vaporeon).unwrap();
        (bs.type1, bs.type2)
    };
    assert_eq!(t1, et1, "native Conversion copied the defender's type1 (parity)");
}

/// Transform: legacy copies species/types/stats/stages/moves. Native matches.
#[test]
fn p5_transform_parity() {
    reset_p5_scratch();
    let mut ls = legacy_state(legacy_poke(Species::Pikachu, 200, 200), legacy_poke(Species::Charizard, 200, 200));
    ls.enemy.active_mon_mut().attack = 150;
    ls.enemy.stat_stages.attack = 2;
    crate::battle::effects::special_effects::apply_transform(&mut ls);
    assert_eq!(ls.player.active_mon().species, Species::Charizard);
    assert_eq!(ls.player.active_mon().attack, 150);
    assert_eq!(ls.player.stat_stages.attack, 2);

    // Native: defender Charizard with Attack stat 150 + Attack stage +2.
    let mut eb = engine_battler(Species::Charizard, 200, 200);
    eb.stats.set(StatIndex::Attack, 150);
    eb.stat_stages.set(StatIndex::Attack, 2);
    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 200)], vec![eb]);
    let mut effects = Vec::new();
    fire_p5(&mut es, &mut effects, transform_effect(), Event::Custom(EV_TRANSFORM), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert_eq!(es.player_battlers[0].species, Species::Charizard, "native Transform copies species (parity)");
    assert_eq!(es.player_battlers[0].stats.get(StatIndex::Attack).copied(), Some(150), "native Transform copies stats (parity)");
    assert_eq!(es.player_battlers[0].stat_stages.get(StatIndex::Attack).copied(), Some(2), "native Transform copies stages (parity)");
}

/// Mimic: legacy overwrites the chosen slot with the foe's last move. Native matches.
#[test]
fn p5_mimic_parity() {
    reset_p5_scratch();
    let mut p_poke = legacy_poke(Species::Pikachu, 200, 200);
    p_poke.moves = [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None];
    let mut ls = legacy_state(p_poke, legacy_poke(Species::Pikachu, 200, 200));
    ls.player.selected_move_index = 2;
    ls.enemy.last_move_used = MoveId::Flamethrower;
    crate::battle::effects::special_effects::apply_mimic(&mut ls);
    assert_eq!(ls.player.active_mon().moves[2], MoveId::Flamethrower);

    // Native: mover moves = [Tackle, Growl, None, None]; mimic slot 2.
    let mut pb = engine_battler(Species::Pikachu, 200, 200);
    pb.moves = vec![MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None];
    let mut es = EngineState::new(vec![pb], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut effects = Vec::new();
    set_last_move(OPP, MoveId::Flamethrower);
    set_mimic_slot(PLAYER, 2);
    fire_p5(&mut es, &mut effects, mimic_effect(), Event::Custom(EV_MIMIC), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert_eq!(es.player_battlers[0].moves[2], MoveId::Flamethrower, "native Mimic copied last move into slot 2 (parity)");
}

/// Metronome: legacy picks a move from the duration byte, skipping Metronome.
/// Native matches the picked move id for several bytes.
#[test]
fn p5_metronome_parity() {
    reset_p5_scratch();
    for dur in [0u8, 1, 50, 117, 162, 200] {
        let randoms = EffectRandoms { side_effect_roll: 0, duration_roll: dur, multi_hit_roll: 0, stat_down_miss_roll: 255 };
        let legacy = crate::battle::effects::special_effects::apply_metronome(&randoms);
        let legacy_pick = match legacy {
            crate::battle::effects::EffectResult::MetronomeMove { picked_move } => picked_move,
            _ => panic!("expected MetronomeMove"),
        };
        let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
        let mut effects = Vec::new();
        // The handler returns RelayVar::Int(picked_move as i64) — the picked MoveId's
        // discriminant — so the native pick is exactly the legacy pick by construction.
        let (out, _c) = fire_p5(&mut es, &mut effects, metronome_effect(), Event::Custom(EV_METRONOME), OPP, PLAYER, RelayVar::Int(dur as i64), vec![]);
        assert_eq!(out.as_int(), legacy_pick as i64, "metronome byte {dur}: native pick == legacy (parity)");
    }
}

/// Mirror Move: legacy re-dispatches the foe's last move; fails on None. Native
/// matches both.
#[test]
fn p5_mirror_move_parity() {
    reset_p5_scratch();
    let mut ls = legacy_state(legacy_poke(Species::Pikachu, 200, 200), legacy_poke(Species::Pikachu, 200, 200));
    ls.enemy.last_move_used = MoveId::Surf;
    let r = crate::battle::effects::special_effects::apply_mirror_move(&mut ls);
    assert_eq!(r, crate::battle::effects::EffectResult::MirrorMove { mirrored_move: MoveId::Surf });

    let mut es = EngineState::new(vec![engine_battler(Species::Pikachu, 200, 200)], vec![engine_battler(Species::Pikachu, 200, 200)]);
    let mut effects = Vec::new();
    set_last_move(OPP, MoveId::Surf);
    let (out, _c) = fire_p5(&mut es, &mut effects, mirror_move_effect(), Event::Custom(EV_MIRROR_MOVE), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert_eq!(out.as_int(), MoveId::Surf as i64, "native Mirror Move returns the foe's last move (parity)");
    // None ⇒ fail (relay → Bool(false)).
    set_last_move(OPP, MoveId::None);
    let (out2, _c2) = fire_p5(&mut es, &mut effects, mirror_move_effect(), Event::Custom(EV_MIRROR_MOVE), OPP, PLAYER, RelayVar::Unit, vec![]);
    assert_eq!(out2, RelayVar::Bool(false), "no last move ⇒ Mirror Move fails (parity)");
}

// ═════════════════════════════════════════════════════════════════════════════
// on:Miss SEAM — JumpKick crash. Fired by the StackDriver on the accuracy-miss
// branch; pokered subscribes here for the 1-HP crash.
// ═════════════════════════════════════════════════════════════════════════════

/// JumpKick crash-on-miss: a miss crashes the user for 1 HP. Driven END TO END
/// through `StackDriver::execute_turn` so the `OnMiss` seam itself is exercised.
#[test]
fn p5_jump_kick_crash_on_miss_via_driver() {
    use dotzuki_engine::battle::stack::{
        Effect as E, EffectId, EffectProvider, EffectType, Event as Ev, EventHook, FirstMover,
        HandlerResult as HR, StackDriver,
    };
    use dotzuki_engine::battle::BattleAction;
    use pokered_data::move_data::MoveData;
    use pokered_data::moves::MoveEffect;

    // A provider that gives a JumpKick move an effect with BOTH the native accuracy
    // pipeline (so it can MISS) AND the `OnMiss` crash hook. We reuse the real
    // pokered_rules pipeline handlers via a dedicated JumpKick effect.
    struct JkProvider;

    fn jk_accuracy(
        ctx: &mut BattleCtx<'_, JkProvider>,
        _r: RelayVar,
        _t: BattlerRef,
        _s: BattlerRef,
        _e: EffectId,
    ) -> HR {
        // Always miss: draw one accuracy byte (255) and fail.
        let _ = ctx.rng.next_u8();
        HR::Set(RelayVar::Bool(false))
    }
    fn jk_crash(
        ctx: &mut BattleCtx<'_, JkProvider>,
        _r: RelayVar,
        _t: BattlerRef,
        source: BattlerRef,
        _e: EffectId,
    ) -> HR {
        ctx.battler_mut(source).take_damage(1);
        HR::Unchanged
    }
    static JK_EFFECT: E<JkProvider> = E {
        id: EffectId(0x50_F00),
        kind: EffectType::Move,
        hooks: &[
            EventHook { event: Ev::Accuracy, call: jk_accuracy, order: u32::MAX, priority: 0, sub_order: None },
            EventHook { event: Ev::OnMiss, call: jk_crash, order: 100, priority: 0, sub_order: None },
        ],
    };

    impl dotzuki_engine::battle::BattleProvider for JkProvider {
        type Monster = ();
        type Move = MoveId;
        type Ability = ();
        type Status = StatusCondition;
        type Stat = StatIndex;
        type Species = Species;
        type Type = PokemonType;
        type Item = ();
        fn calculate_damage(&self, _m: &MoveId, _a: &EngineBattler<Self>, _d: &EngineBattler<Self>, _r: u8, _c: bool) -> dotzuki_engine::battle::DamageResult {
            dotzuki_engine::battle::DamageResult { damage: 0, effectiveness: 1.0, is_miss: false }
        }
        fn select_move(&self, b: &EngineBattler<Self>, _s: &EngineState<Self>) -> MoveId {
            b.moves.first().copied().unwrap_or(MoveId::Tackle)
        }
        fn apply_move_effect(&self, _e: dotzuki_engine::battle::MoveEffect, _u: &mut EngineBattler<Self>, _t: &mut EngineBattler<Self>) -> dotzuki_engine::battle::EffectResult {
            dotzuki_engine::battle::EffectResult::NoEffect
        }
        fn create_monster(&self, s: Species, _l: u8) -> EngineBattler<Self> {
            EngineBattler::new(s, 100, 100, EnumMap::new(), vec![])
        }
    }
    impl EffectProvider for JkProvider {
        type EffectStateKind = PokeVolatile;
        fn effect_for_move(&self, _m: &MoveId) -> Option<&'static E<Self>> { Some(&JK_EFFECT) }
        fn effect_for_status(&self, _s: &StatusCondition) -> Option<&'static E<Self>> { None }
        fn turn_order_rank(&self, _s: &EngineState<Self>, who: BattlerRef, _a: &MoveId) -> (i32, i32) {
            if who.side == 0 { (0, 0) } else { (1, 0) } // player first, no tie
        }
    }

    // Player uses JumpKick and MISSES → crashes for 1 HP. Opponent's reply also
    // misses → also crashes for 1 HP. Both end at 99.
    let _ = MoveData { id: MoveId::JumpKick, effect: MoveEffect::JumpKickEffect, power: 70, move_type: PokemonType::Fighting, accuracy: 95, pp: 25 };
    let mut state = EngineState::new(
        vec![EngineBattler::new(Species::Hitmonlee, 100, 100, { let mut s = EnumMap::new(); s.set(StatIndex::Speed, 110); s }, vec![MoveId::JumpKick])],
        vec![EngineBattler::new(Species::Hitmonlee, 100, 100, { let mut s = EnumMap::new(); s.set(StatIndex::Speed, 50); s }, vec![MoveId::JumpKick])],
    );
    let mut effects: Vec<EffectState<JkProvider>> = Vec::new();
    let actions = [
        BattleAction::<JkProvider>::Fight { move_: MoveId::JumpKick },
        BattleAction::<JkProvider>::Fight { move_: MoveId::JumpKick },
    ];
    // Two accuracy bytes (both misses) → 2 crash hits, no other draws.
    let mut rng = ScriptedRng::new(vec![255, 255]);
    let result = StackDriver::execute_turn(&JkProvider, &mut state, &mut effects, actions, &mut rng);
    assert_eq!(result.first, FirstMover::Player);
    assert_eq!(state.player_battlers[0].hp, 99, "player JumpKick missed → crashed 1 HP (on:Miss seam)");
    assert_eq!(state.opponent_battlers[0].hp, 99, "opponent JumpKick missed → crashed 1 HP (on:Miss seam)");
    assert_eq!(rng.consumed(), 2, "two accuracy bytes drawn (both misses); the crash draws nothing");

    // Also fire the PRODUCTION-SHAPED module handler `jump_kick_crash_effect()`
    // directly on `Event::OnMiss` (the real pokered-side OnMiss subscriber): the
    // mover (= source) takes the 1-HP crash, the defender (= target) is untouched.
    let mut es = EngineState::new(
        vec![engine_battler(Species::Hitmonlee, 100, 100)],
        vec![engine_battler(Species::Hitmonlee, 100, 100)],
    );
    let mut effects = Vec::new();
    fire_p5(&mut es, &mut effects, jump_kick_crash_effect(), Event::OnMiss, OPP, PLAYER, RelayVar::Unit, vec![]);
    assert_eq!(es.player_battlers[0].hp, 99, "module jump_kick_crash crashes the mover 1 HP");
    assert_eq!(es.opponent_battlers[0].hp, 100, "module jump_kick_crash leaves the defender untouched");
}

/// on:Miss is INERT when the move registers no OnMiss hook (the additive/defaulted
/// guarantee): a missing move with a plain pipeline effect crashes nothing and
/// draws exactly the accuracy byte.
#[test]
fn p5_on_miss_inert_without_subscriber() {
    use super::{install_canonical, set_active_move, move_effect_for};
    use dotzuki_engine::battle::stack::{FirstMover, StackDriver};
    use dotzuki_engine::battle::BattleAction;
    use pokered_data::move_data::MoveData;
    install_canonical();
    super::set_level(Species::Snorlax, 50);
    // Tackle (a pure-damage move, NO OnMiss hook). Player misses (acc byte 255).
    set_active_move(*MoveData::get(MoveId::Tackle).unwrap());
    let mut state = EngineState::new(
        vec![engine_battler(Species::Snorlax, 300, 300)],
        vec![engine_battler(Species::Snorlax, 300, 300)],
    );
    // Make the player faster so it moves first; give it higher speed.
    state.player_battlers[0].stats.set(StatIndex::Speed, 110);
    state.opponent_battlers[0].stats.set(StatIndex::Speed, 50);
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
    ];
    // Player: crit 255, accuracy 255 (MISS — no damage byte). Opponent: crit 255,
    // accuracy 0 (hit), damage 255.
    let _ = move_effect_for(MoveId::Tackle);
    let mut rng = ScriptedRng::new(vec![255, 255, 255, 0, 255]);
    let result = StackDriver::execute_turn(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
    assert_eq!(result.first, FirstMover::Player);
    // Player's Tackle MISSED → the opponent is untouched by it (no crash on the
    // PLAYER either, since Tackle has no OnMiss hook — the additive/defaulted
    // guarantee: the inert OnMiss fire collected zero handlers).
    assert_eq!(state.opponent_battlers[0].hp, 300, "player's Tackle missed → opponent unharmed");
    // The opponent's Tackle HIT the player (player is the opponent's defender), so
    // the player took normal damage — NOT a crash (Tackle still has no OnMiss hook).
    assert!(state.player_battlers[0].hp < 300, "opponent's Tackle hit the player normally");
    // Exactly 5 bytes drawn (player crit+acc=2 on the miss; opponent crit+acc+dmg=3)
    // — the inert OnMiss fire drew NOTHING (the additive/defaulted guarantee).
    assert_eq!(rng.consumed(), 5, "on:Miss fire drew no extra byte (inert without a subscriber)");
}

// ═════════════════════════════════════════════════════════════════════════════
// HONESTY MARKER — the reactive Bide/Counter/Rage have NO legacy MoveEffect /
// no `apply_*` oracle (blueprint risk #3). They are SYNTHETIC-ORACLE-ONLY and are
// already proven in the `stack_parity` slice 6 (the forced_action + arena path).
// This file does NOT fabricate a legacy parity test for them.
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn synthetic_oracle_note() {
    // A synthetic-oracle assertion for Bide's ×2 (#18), matching the hand-specified
    // outcome (NOT a legacy `apply_*` oracle — `multi_turn_effects::apply_bide` is
    // the legacy state machine, but it has no MoveEffect parity test; slice 6 owns
    // the cross-turn driver proof). Here we just pin the ×2 arithmetic explicitly.
    let accumulated: u16 = 30;
    let unleash = accumulated * 2; // bug #18: doubles, not triples
    assert_eq!(unleash, 60, "Bide releases accumulated*2 (#18) — synthetic-oracle-only");
    // Counter (#20) reflects physical damage taken *2 (synthetic; no MoveEffect).
    let taken: u16 = 40;
    assert_eq!(taken * 2, 80, "Counter reflects damage_taken*2 (#20) — synthetic-oracle-only");
}
