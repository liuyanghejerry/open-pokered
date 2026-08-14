//! P1 differential parity tests: for each authored bucket-A move, run the SAME
//! scenario + SAME byte vector through (a) the LEGACY oracle (`execute_turn` /
//! `apply_move_effect`) and (b) the STACK path (`StackDriver::execute_turn` with
//! the `PokeredRules` provider + the `rules.ron` effects), asserting
//!   * IDENTICAL resulting `BattleState` (both sides hp + status + stat stages), AND
//!   * identical `rng.consumed()` (count AND order).
//!
//! The numbers are REAL Gen-1 numbers: the harness reads the REAL `MoveData`
//! (power/type/accuracy) for each move and uses the real species' base stats /
//! type chart / DVs-equivalent stat block, so the damage that flows through is
//! exactly what the shipped game computes. Both paths share `calculate_damage`
//! (the damage authority), so the values agree by construction; the captured
//! values are asserted to specific Gen-1 numbers below for documentation.

#![cfg(test)]

use super::{
    clear_current_moves, clear_last_move_live, install_canonical, move_effect_for, set_active_move,
    set_current_move, set_last_move_live, species_types, sub_created_this_turn, PokeVolatile,
    PokeredRules,
};

use dotzuki_engine::battle::rng::ScriptedRng;
use dotzuki_engine::battle::stack::{
    EffectId, EffectProvider, EffectState, FirstMover, StackDriver, TurnEvent, TurnLog,
};
use dotzuki_engine::battle::{
    BattleAction, BattleState as EngineState, BattlerRef,
    BattlerState as EngineBattler, EnumMap,
};

use pokered_data::move_data::MoveData;
use pokered_data::moves::{MoveEffect, MoveId};
use pokered_data::species::Species;
use pokered_data::types::{Effectiveness, PokemonType};

use crate::battle::effects::EffectRandoms;
use crate::battle::move_execution::MoveRandoms;
use crate::battle::stat_stages::StatIndex;
use crate::battle::state::{
    new_battle_state, status2, BattleState as LegacyState, BattleType, Pokemon, Side,
    StatusCondition as LegacyStatus,
};
use crate::battle::turn::{execute_turn, TurnRandoms};

// ─── Scenario shape (mirrors the stack_parity DamageScenario, PokeredRules-side) ─

/// Per-mover RNG bytes in pokered's `MoveRandoms` FIELD order. Only the
/// meaningful ones are drawn; the rest are harmless struct padding. `side_effect`
/// is the P2 side-effect-chance byte (the `EffectRandoms::side_effect_roll`): it
/// is ALWAYS laid into the legacy `TurnRandoms` field AND, for a side-status move,
/// drawn by the stack at the same ordinal (after `damage`) — fired or not — so
/// `consumed()` stays invariant (blueprint §5 P2).
#[derive(Clone, Copy)]
struct MoveBytes {
    confusion: u8,
    paralysis: u8,
    crit: u8,
    accuracy: u8,
    damage: u8,
    side_effect: u8,
    /// P4: the `multi_hit_roll` byte the `RepeatHits` TwoToFive count source draws
    /// (the legacy `EffectRandoms::multi_hit_roll`). Drawn by the stack at the
    /// DamagingHit ordinal AFTER `damage`, only for a TwoToFive multi-hit move.
    multi_hit: u8,
}

impl MoveBytes {
    /// crit 255 (no crit), accuracy 0 (always hits), damage 255 (max roll),
    /// side_effect 255 (a 255 byte is `>= any threshold` ⇒ secondary does NOT fire),
    /// multi_hit 0 (< 96 ⇒ the 2-5 distribution yields 2 hits).
    fn always_hit() -> Self {
        Self { confusion: 255, paralysis: 255, crit: 255, accuracy: 0, damage: 255, side_effect: 255, multi_hit: 0 }
    }
}

/// Per-side mon config. Both movers use the SAME move + MoveData (mirroring the
/// existing `DamageScenario`); the active-move thread-local holds that one move,
/// so the per-mover damage math is identical on both paths by construction.
#[derive(Clone, Copy)]
struct Mon {
    species: Species,
    hp: u16,
    speed: u16,
    attack: u16,
    defense: u16,
    special: u16,
    focus_energy: bool,
    /// The mon's level (P3 `battler_level` / OHKO `LevelGE` / level `SetDamage`).
    /// Defaults to 50 (the P1/P2 fixed level).
    level: u16,
}

impl Mon {
    fn new(species: Species, hp: u16, speed: u16) -> Self {
        Self { species, hp, speed, attack: 100, defense: 80, special: 80, focus_energy: false, level: 50 }
    }
}

/// A P1 differential scenario: both movers use `move_id` (+ its real `MoveData`).
#[derive(Clone, Copy)]
struct Scenario {
    name: &'static str,
    player: Mon,
    enemy: Mon,
    move_id: MoveId,
    order_byte: u8,
    first: MoveBytes,
    second: MoveBytes,
}

impl Scenario {
    /// A base scenario: both Pikachu, player faster, both move with `always_hit`.
    fn base(name: &'static str, move_id: MoveId) -> Self {
        Self {
            name,
            player: Mon::new(Species::Pikachu, 200, 100),
            enemy: Mon::new(Species::Pikachu, 200, 50),
            move_id,
            order_byte: 0,
            first: MoveBytes::always_hit(),
            second: MoveBytes::always_hit(),
        }
    }
}

/// The REAL MoveData the shipped game uses for a move (power/type/accuracy/effect).
fn real_move(id: MoveId) -> MoveData {
    *MoveData::get(id).unwrap_or_else(|| panic!("no MoveData for {id:?}"))
}

// ─── Legacy oracle path ──────────────────────────────────────────────────────

fn poke(m: &Mon, move_id: MoveId) -> Pokemon {
    let base = pokered_data::pokemon_data::get_base_stats(m.species).expect("base stats");
    Pokemon {
        species: m.species,
        nickname: [0x50; 11],
        level: 50,
        hp: m.hp,
        max_hp: m.hp,
        attack: m.attack,
        defense: m.defense,
        speed: m.speed,
        special: m.special,
        type1: base.type1,
        type2: base.type2,
        moves: [move_id, MoveId::None, MoveId::None, MoveId::None],
        pp: [30, 0, 0, 0],
        pp_ups: [0; 4],
        status: LegacyStatus::None,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

fn to_move_randoms(b: MoveBytes) -> MoveRandoms {
    MoveRandoms {
        confusion_roll: b.confusion,
        paralysis_roll: b.paralysis,
        crit_roll: b.crit,
        accuracy_roll: b.accuracy,
        damage_roll: b.damage,
        // P2: the legacy side-effect roll is laid into the field at the same value
        // the stack draws, so a fired-vs-not side-status is byte-identical. P4: the
        // multi_hit_roll is laid into the legacy field too (the legacy
        // `apply_two_to_five` reads `randoms.multi_hit_roll` for its hit count).
        effect_randoms: EffectRandoms { side_effect_roll: b.side_effect, duration_roll: 0, multi_hit_roll: b.multi_hit, stat_down_miss_roll: 255 },
    }
}

/// Gen-1 two-to-five distribution (the harness's local copy of the engine
/// `determine_hit_count`): `roll<96⇒2, <192⇒3, <224⇒4, else⇒5`. Used to predict the
/// stack stream's hit count + the expected total damage (per-hit × N).
fn determine_hit_count(roll: u8) -> u8 {
    if roll < 96 { 2 } else if roll < 192 { 3 } else if roll < 224 { 4 } else { 5 }
}

/// Whether `effect` is a P4 TwoToFive multi-hit (draws ONE count byte at the
/// DamagingHit ordinal after `damage`).
fn is_two_to_five(effect: MoveEffect) -> bool {
    effect == MoveEffect::TwoToFiveAttacksEffect
}

/// Whether `effect` is a P4 Twineedle (Fixed 2 hits, NO count byte, + ONE
/// final-hit poison chance byte at the side_effect ordinal after the count).
fn is_twineedle(effect: MoveEffect) -> bool {
    effect == MoveEffect::TwineedleEffect
}

/// Whether `effect` is a P2 side-status (chance-gated) effect — the stack draws
/// the side-effect byte at the legacy ordinal (after `damage`) whether or not the
/// secondary fires, so the byte stream / `consumed()` is invariant.
fn is_side_status_effect(effect: MoveEffect) -> bool {
    use MoveEffect::*;
    matches!(
        effect,
        PoisonSideEffect1 | PoisonSideEffect2
            | BurnSideEffect1 | BurnSideEffect2
            | FreezeSideEffect1 | FreezeSideEffect2
            | ParalyzeSideEffect1 | ParalyzeSideEffect2
    )
}

/// Whether `effect` is a P3 SIDE foe-stat-down (chance-gated 85/256). Like the
/// side-status case, the nested-veto driver draws ONE chance byte at the legacy
/// ordinal (after `damage`) whether or not the drop fires/vetoes, so the byte
/// stream / `consumed()` is invariant.
fn is_side_foe_down_effect(effect: MoveEffect) -> bool {
    use MoveEffect::*;
    matches!(
        effect,
        AttackDownSideEffect | DefenseDownSideEffect | SpeedDownSideEffect | SpecialDownSideEffect
    )
}

fn legacy_run(s: &Scenario) -> LegacyState {
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![poke(&s.player, s.move_id)],
        vec![poke(&s.enemy, s.move_id)],
    );
    state.player.selected_move = s.move_id;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = s.move_id;
    state.enemy.selected_move_index = 0;
    if s.player.focus_energy {
        state.player.set_status2(status2::GETTING_PUMPED);
    }
    if s.enemy.focus_energy {
        state.enemy.set_status2(status2::GETTING_PUMPED);
    }
    let md = real_move(s.move_id);
    let randoms = TurnRandoms {
        order_random: s.order_byte,
        first_mover: to_move_randoms(s.first),
        second_mover: to_move_randoms(s.second),
    };
    execute_turn(&mut state, &md, &md, &randoms);
    state
}

// ─── Stack path (PokeredRules + StackDriver) ─────────────────────────────────

fn engine_battler(m: &Mon, move_id: MoveId) -> EngineBattler<PokeredRules> {
    let mut stats = EnumMap::new();
    stats.set(StatIndex::Attack, m.attack);
    stats.set(StatIndex::Defense, m.defense);
    stats.set(StatIndex::Speed, m.speed);
    stats.set(StatIndex::Special, m.special);
    EngineBattler::new(m.species, m.hp, m.hp, stats, vec![move_id])
}

fn first_mover(s: &Scenario) -> FirstMover {
    let provider = PokeredRules;
    let state = EngineState::new(
        vec![engine_battler(&s.player, s.move_id)],
        vec![engine_battler(&s.enemy, s.move_id)],
    );
    let pr = provider.turn_order_rank(&state, BattlerRef::PLAYER, &s.move_id);
    let er = provider.turn_order_rank(&state, BattlerRef::OPPONENT, &s.move_id);
    match pr.cmp(&er) {
        std::cmp::Ordering::Less => FirstMover::Player,
        std::cmp::Ordering::Greater => FirstMover::Opponent,
        std::cmp::Ordering::Equal => {
            if s.order_byte < 128 { FirstMover::Player } else { FirstMover::Opponent }
        }
    }
}

fn order_is_tie(s: &Scenario) -> bool {
    let provider = PokeredRules;
    let state = EngineState::new(
        vec![engine_battler(&s.player, s.move_id)],
        vec![engine_battler(&s.enemy, s.move_id)],
    );
    provider.turn_order_rank(&state, BattlerRef::PLAYER, &s.move_id)
        == provider.turn_order_rank(&state, BattlerRef::OPPONENT, &s.move_id)
}

/// Predict the byte stream the StackDriver draws, in FIRE order, + the count.
/// Mirrors the resolve_action control flow for the P1/P2 (no-confusion, no-status)
/// scenarios: order byte (tie only) → per mover [crit? acc damage? side_effect?].
///   * crit drawn iff power > 0;
///   * accuracy drawn always EXCEPT SwiftEffect (never misses, draws nothing);
///   * damage drawn iff power > 0 AND accuracy hit;
///   * P2 side_effect drawn iff the move is a chance-gated side-status AND it HIT
///     (the chance gate on the `DamagingHit` hook is drawn UNCONDITIONALLY of
///     whether the secondary fires, so consumed() is invariant). Drain/recoil and
///     primary-status hooks carry NO chance gate ⇒ no extra byte.
/// P6b-prereq stage 2: does the first mover's move leave the TARGET (the second
/// mover) PARALYZED when it acts? Primary `ParalyzeEffect` (Thunder Wave) paralyzes
/// 100% on hit; the side-effect variants paralyze when their chance byte fires
/// (1 ⇒ 26/256, 2 ⇒ 77/256). A paralyzed second mover then draws ONE paralysis
/// gate byte at its BeforeMove (before crit). Sleep/Freeze/Confusion infliction on a
/// second mover is not exercised by the differential suite, so it is not modelled
/// here (the direct gate tests drive those with explicit bytes).
fn move_paralyzes_target(md: &MoveData, first_hit: bool, first_side_byte: u8, target_subbed: bool) -> bool {
    if !first_hit || target_subbed {
        return false; // a Substitute blocks the status ⇒ no paralysis ⇒ no gate byte
    }
    match md.effect {
        MoveEffect::ParalyzeEffect => true, // primary, 100% on hit (e.g. Thunder Wave)
        MoveEffect::ParalyzeSideEffect1 => first_side_byte < 26,
        MoveEffect::ParalyzeSideEffect2 => first_side_byte < 77,
        _ => false,
    }
}

/// The no-substitute byte predictor (the common case).
fn build_stream(s: &Scenario, first: FirstMover, tie: bool) -> Vec<u8> {
    build_stream_sub(s, first, tie, false)
}

/// Like [`build_stream`] but with whether the SECOND mover holds a Substitute (which
/// blocks a first-mover-inflicted paralysis, suppressing the second mover's para
/// gate byte). Only the P2 substitute scenarios pass `true`.
fn build_stream_sub(s: &Scenario, first: FirstMover, tie: bool, second_subbed: bool) -> Vec<u8> {
    let md = real_move(s.move_id);
    let mut bytes = if tie { vec![s.order_byte] } else { Vec::new() };
    // `gate_para`: the mover is paralyzed at its BeforeMove (draws the paralysis gate
    // byte first; `< 63` would abort, but the harness scenarios use paralysis 255 ⇒
    // the gate passes and the move proceeds).
    let push = |bytes: &mut Vec<u8>, mb: MoveBytes, gate_para: bool| {
        if gate_para {
            bytes.push(mb.paralysis);
            if mb.paralysis < 63 {
                return; // fully paralyzed → move aborts (no crit/acc/damage)
            }
        }
        if md.power > 0 {
            bytes.push(mb.crit);
        }
        let draws_acc = md.effect != MoveEffect::SwiftEffect;
        if draws_acc {
            bytes.push(mb.accuracy);
        }
        // accuracy 100% → scaled 255; only byte 255 misses (the 1/256 bug). Swift
        // never draws acc and always hits.
        let scaled = (md.accuracy as u32 * 255 / 100).min(255) as u8;
        let hit = !draws_acc || mb.accuracy < scaled;
        if md.power > 0 && hit {
            bytes.push(mb.damage);
        }
        // P2: the side-status chance gate draws ONE byte (after damage) on a hit.
        // P3: the SIDE foe-stat-down chance gate (85/256) draws ONE byte too — the
        // nested-veto driver reads it at the same legacy ordinal whether or not the
        // drop fires/vetoes (the side_effect-at-legacy-ordinal invariant).
        if hit && (is_side_status_effect(md.effect) || is_side_foe_down_effect(md.effect)) {
            bytes.push(mb.side_effect);
        }
        // P4: a TwoToFive multi-hit draws ONE count byte (the multi_hit_roll) at the
        // DamagingHit ordinal AFTER `damage` (Fixed-count moves — Double Kick /
        // Bonemerang / Twineedle — draw NO count byte). It is drawn only on a hit
        // (the RepeatHits op rides DamagingHit, which fires after the first hit).
        if hit && is_two_to_five(md.effect) {
            bytes.push(mb.multi_hit);
        }
        // P4: Twineedle draws ONE final-hit poison chance byte (52/256) at the
        // side_effect ordinal (after the count — but Twineedle is Fixed(2) so there
        // is no count byte before it), drawn UNCONDITIONALLY of whether poison fires.
        if hit && is_twineedle(md.effect) {
            bytes.push(mb.side_effect);
        }
    };
    let (fb, sb) = match first {
        FirstMover::Player => (s.first, s.second),
        FirstMover::Opponent => (s.first, s.second),
    };
    // The first mover has no pre-existing gate status (scenarios set none), so it
    // never draws a gate byte.
    push(&mut bytes, fb, false);
    // The second mover may be paralyzed by the first mover's move this turn (Thunder
    // Wave / Body Slam), in which case its paralysis gate draws one byte first.
    let first_scaled = (md.accuracy as u32 * 255 / 100).min(255) as u8;
    let first_hit = md.effect == MoveEffect::SwiftEffect || fb.accuracy < first_scaled;
    let sec_para = move_paralyzes_target(&md, first_hit, fb.side_effect, second_subbed);
    // Both HP kept > 30 in every scenario so the second mover always acts.
    push(&mut bytes, sb, sec_para);
    bytes
}

/// Run the stack path: build engine state + the Focus-Energy arena, set the
/// active move, stream the bytes in FIRE order, return (state, consumed, first).
fn stack_run(s: &Scenario) -> (EngineState<PokeredRules>, usize, FirstMover) {
    install_canonical();
    set_active_move(real_move(s.move_id));
    let mut state = EngineState::new(
        vec![engine_battler(&s.player, s.move_id)],
        vec![engine_battler(&s.enemy, s.move_id)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    if s.player.focus_energy {
        effects.push(EffectState {
            id: EffectId(100),
            host: BattlerRef::PLAYER,
            effect_order: 0,
            kind: PokeVolatile::FocusEnergy,
        });
    }
    if s.enemy.focus_energy {
        effects.push(EffectState {
            id: EffectId(101),
            host: BattlerRef::OPPONENT,
            effect_order: 1,
            kind: PokeVolatile::FocusEnergy,
        });
    }
    let provider = PokeredRules;
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
    ];
    let first = first_mover(s);
    let tie = order_is_tie(s);
    let bytes = build_stream(s, first, tie);
    let mut rng = ScriptedRng::new(bytes);
    let result = StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
    assert_eq!(result.first, first, "[{}] first-mover probe disagreed", s.name);
    (state, rng.consumed(), first)
}

// ─── The differential oracle ─────────────────────────────────────────────────

fn legacy_stage(stages: &crate::battle::stat_stages::StatStages, st: StatIndex) -> i8 {
    match st {
        StatIndex::Attack => stages.attack,
        StatIndex::Defense => stages.defense,
        StatIndex::Speed => stages.speed,
        StatIndex::Special => stages.special,
        StatIndex::Accuracy => stages.accuracy,
        StatIndex::Evasion => stages.evasion,
    }
}

fn stack_stage(b: &EngineBattler<PokeredRules>, st: StatIndex) -> i8 {
    b.stat_stages.get(st).copied().unwrap_or(0)
}

/// Assert IDENTICAL BattleState (hp + status + every stat stage, both sides) AND
/// identical consumed() vs the predicted byte count.
fn run_scenario(s: &Scenario) {
    let legacy = legacy_run(s);
    let (stack, consumed, first) = stack_run(s);

    let lp = legacy.player.active_mon();
    let le = legacy.enemy.active_mon();
    let sp = &stack.player_battlers[0];
    let se = &stack.opponent_battlers[0];

    // hp parity, both sides.
    assert_eq!(lp.hp, sp.hp, "[{}] PLAYER hp: legacy={} stack={}", s.name, lp.hp, sp.hp);
    assert_eq!(le.hp, se.hp, "[{}] ENEMY hp: legacy={} stack={}", s.name, le.hp, se.hp);
    // status parity, both sides.
    assert_eq!(lp.status, sp.status.unwrap_or(LegacyStatus::None), "[{}] PLAYER status", s.name);
    assert_eq!(le.status, se.status.unwrap_or(LegacyStatus::None), "[{}] ENEMY status", s.name);
    // EVERY stat-stage, both sides (the self-Boost proof).
    for st in [
        StatIndex::Attack, StatIndex::Defense, StatIndex::Speed,
        StatIndex::Special, StatIndex::Accuracy, StatIndex::Evasion,
    ] {
        assert_eq!(
            legacy_stage(&legacy.player.stat_stages, st),
            stack_stage(sp, st),
            "[{}] PLAYER stage {:?}", s.name, st
        );
        assert_eq!(
            legacy_stage(&legacy.enemy.stat_stages, st),
            stack_stage(se, st),
            "[{}] ENEMY stage {:?}", s.name, st
        );
    }
    // consumed() parity (draw count + order).
    let expected = build_stream(s, first, order_is_tie(s)).len();
    assert_eq!(
        consumed, expected,
        "[{}] stack consumed {} bytes, expected {} (draw-order drift!)",
        s.name, consumed, expected
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// P6b SHADOW PROOF — validate that the engine TurnLog (P6a) faithfully NARRATES a
// real Gen-1 turn, against the production-representative legacy oracle.
//
// This is the de-risking step the user chose before any production UI rewrite: it
// proves, end to end, that (1) `execute_turn_logged` is non-perturbing on the
// REAL `PokeredRules` provider (same final state as `execute_turn` AND as the
// legacy `apply_move_effect`/`execute_turn` oracle), and (2) the resulting
// `TurnLog` carries exactly the events a frontend translator needs — damage,
// faint, status, stat-stage, crit, move-used — each matching the legacy outcome.
// No production code is touched; this is the contract the P6b translator will read.
// ═════════════════════════════════════════════════════════════════════════════

/// Like [`stack_run`] but via [`StackDriver::execute_turn_logged`]: returns the
/// final state + the narrated [`TurnLog`].
fn stack_run_logged(s: &Scenario) -> (EngineState<PokeredRules>, TurnLog<PokeredRules>) {
    install_canonical();
    set_active_move(real_move(s.move_id));
    let mut state = EngineState::new(
        vec![engine_battler(&s.player, s.move_id)],
        vec![engine_battler(&s.enemy, s.move_id)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    if s.player.focus_energy {
        effects.push(EffectState { id: EffectId(100), host: BattlerRef::PLAYER, effect_order: 0, kind: PokeVolatile::FocusEnergy });
    }
    if s.enemy.focus_energy {
        effects.push(EffectState { id: EffectId(101), host: BattlerRef::OPPONENT, effect_order: 1, kind: PokeVolatile::FocusEnergy });
    }
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
    ];
    let first = first_mover(s);
    let bytes = build_stream(s, first, order_is_tie(s));
    let mut rng = ScriptedRng::new(bytes);
    let (_result, log) =
        StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
    (state, log)
}

// ─── P1B batch 4: the DECOUPLED status/volatile-infliction seam ────────────────
//
// These verify that the game-agnostic `InflictVolatile` / `InflictStatus(amount)`
// ops (dotzuki-rules) drive Pokémon-specific volatiles/statuses purely through the
// `RuleBindings` seam — the engine never learns "confusion"/"sleep".

#[test]
fn decoupled_make_volatile_maps_the_game_vocabulary() {
    use dotzuki_rules::RuleBindings;
    let b = super::PokeredBindings;
    // The game binding is the ONLY place a volatile name → PokeVolatile.
    assert!(matches!(b.make_volatile("confusion", 4), Some(PokeVolatile::Confused { turns: 4 })));
    assert!(matches!(b.make_volatile("leechseed", 0), Some(PokeVolatile::LeechSeed)));
    assert!(matches!(b.make_volatile("toxic", 0), Some(PokeVolatile::Toxic { counter: 0 })));
    assert!(matches!(b.make_volatile("flinch", 0), Some(PokeVolatile::Flinched)));
    assert!(b.make_volatile("not-a-volatile", 0).is_none(), "unknown name ⇒ inert");
    assert_eq!(super::status_index_of("sleep"), Some(super::SLEEP_STATUS_INDEX));
}

/// A guaranteed confuser drives the generic `InflictVolatile` op end-to-end:
/// move → op → `make_volatile` binding → `ctx.install_effect` → a `Confused`
/// arena entry on the target. All-zero rng ⇒ every gate passes.
#[test]
fn confuse_ray_installs_confusion_via_decoupled_stack() {
    install_canonical();
    set_active_move(real_move(MoveId::ConfuseRay));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Gengar, 120, 110), MoveId::ConfuseRay)],
        vec![engine_battler(&Mon::new(Species::Rattata, 120, 40), MoveId::ConfuseRay)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::ConfuseRay },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::ConfuseRay },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(
        effects
            .iter()
            .any(|e| e.host == BattlerRef::OPPONENT && matches!(e.kind, PokeVolatile::Confused { .. })),
        "ConfuseRay must install a Confused volatile on the enemy via InflictVolatile"
    );
}

/// A sleep move drives `InflictStatus(amount)`: move → op → `set_status_with_amount`
/// → `Sleep(turns)` on the target, with the engine drawing the turn count.
#[test]
fn hypnosis_sleeps_target_via_decoupled_stack() {
    install_canonical();
    set_active_move(real_move(MoveId::Hypnosis));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Gengar, 120, 110), MoveId::Hypnosis)],
        vec![engine_battler(&Mon::new(Species::Rattata, 120, 40), MoveId::Hypnosis)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Hypnosis },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Hypnosis },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let (_r, log) = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    // A 1-turn sleep (all-zero rng ⇒ turns=1) is ticked away when the enemy tries
    // to act, so assert on the INFLICTION event, not the post-turn status: the
    // decoupled InflictStatus(amount) op set Sleep on the enemy.
    assert!(
        log.events.iter().any(|e| matches!(
            e,
            TurnEvent::StatusInflicted { target, status }
                if *target == BattlerRef::OPPONENT && matches!(status, LegacyStatus::Sleep(_))
        )),
        "Hypnosis must inflict Sleep on the enemy via InflictStatus(amount); log={:?}",
        log.events
    );
}

/// A flinch move installs the "flinch" volatile via `InflictVolatile`; the
/// flinch_gate then blocks the flinched mon's own move that turn.
#[test]
fn headbutt_flinches_and_blocks_target_via_decoupled_stack() {
    install_canonical();
    set_active_move(real_move(MoveId::Headbutt));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 200, 110), MoveId::Headbutt)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 200, 40), MoveId::Headbutt)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Headbutt },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Headbutt },
    ];
    // All-zero rng ⇒ the 30% flinch chance byte (0 < 77) fires; the faster mover
    // (Tauros) flinches the enemy, whose move is then blocked.
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let (_r, log) = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(
        log.events.iter().any(|e| matches!(
            e, TurnEvent::Blocked { actor } if *actor == BattlerRef::OPPONENT
        )),
        "the flinched enemy's move must be blocked; log={:?}",
        log.events
    );
    // Only the player's Headbutt should have executed (the enemy's was blocked
    // before its MoveUsed).
    let move_used = log.events.iter().filter(|e| matches!(e, TurnEvent::MoveUsed { .. })).count();
    assert_eq!(move_used, 1, "only the un-flinched mover acts; log={:?}", log.events);
}

/// Toxic sets Poison AND the Toxic counter volatile atomically (the
/// `TargetHasAnyStatus` guard + `InflictStatus` + `InflictVolatile`).
#[test]
fn toxic_sets_poison_and_toxic_volatile_via_decoupled_stack() {
    install_canonical();
    set_active_move(real_move(MoveId::Toxic));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 200, 110), MoveId::Toxic)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 200, 40), MoveId::Toxic)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Toxic },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Toxic },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(
        state.opponent_battlers[0].status,
        Some(LegacyStatus::Poison),
        "Toxic must poison the target"
    );
    assert!(
        effects
            .iter()
            .any(|e| e.host == BattlerRef::OPPONENT && matches!(e.kind, PokeVolatile::Toxic { .. })),
        "Toxic must also install the toxic-counter volatile"
    );
}

/// Rest cures + sleeps (self): RemoveStatus then InflictStatus("sleep", Const(2))
/// on Source, driven purely through the decoupled ops.
#[test]
fn rest_sleeps_self_via_decoupled_stack() {
    install_canonical();
    set_active_move(real_move(MoveId::Rest));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Snorlax, 200, 110), MoveId::Rest)],
        vec![engine_battler(&Mon::new(Species::Tauros, 200, 40), MoveId::Rest)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Rest },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Rest },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    // The faster mover (Snorlax) rested this turn; its sleep isn't ticked until
    // its next move, so it is asleep at turn end.
    assert!(
        matches!(state.player_battlers[0].status, Some(LegacyStatus::Sleep(_))),
        "Rest must put the user to sleep; got {:?}",
        state.player_battlers[0].status
    );
}

/// Focus Energy installs its self-volatile (pokered_crit already reads it — and
/// applies the Gen-1 /4 crit BUG).
#[test]
fn focus_energy_installs_volatile_via_decoupled_stack() {
    install_canonical();
    set_active_move(real_move(MoveId::FocusEnergy));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 200, 110), MoveId::FocusEnergy)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 200, 40), MoveId::FocusEnergy)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::FocusEnergy },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::FocusEnergy },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(
        effects
            .iter()
            .any(|e| e.host == BattlerRef::PLAYER && matches!(e.kind, PokeVolatile::FocusEnergy)),
        "Focus Energy must install its volatile on the user"
    );
}

/// Explosion KOs the user (the generic SetHp(Source, 0) op after the hit).
#[test]
fn explosion_kos_the_user_via_decoupled_stack() {
    install_canonical();
    set_active_move(real_move(MoveId::Explosion));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Electrode, 200, 200), MoveId::Explosion)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 400, 40), MoveId::Explosion)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Explosion },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Explosion },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(
        state.player_battlers[0].hp, 0,
        "the faster mover (Electrode) must faint from its own Explosion"
    );
}

/// Reflect installs its self-volatile (setter side of the decoupled seam).
#[test]
fn reflect_installs_volatile_via_decoupled_stack() {
    install_canonical();
    set_active_move(real_move(MoveId::Reflect));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Alakazam, 200, 110), MoveId::Reflect)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 200, 40), MoveId::Reflect)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Reflect },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Reflect },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(
        effects
            .iter()
            .any(|e| e.host == BattlerRef::PLAYER && matches!(e.kind, PokeVolatile::Reflect)),
        "Reflect must install its volatile on the user"
    );
}

/// Reflect halves physical damage taken (consumer side: pokered_damage reads the
/// defender's screen volatile). Light Screen would mirror this for special moves.
#[test]
fn reflect_halves_physical_damage_via_decoupled_stack() {
    // Run a physical Tackle into a defender, optionally pre-shielded by Reflect,
    // and report the damage the defender took.
    let tackle_damage = |reflect: bool| -> i32 {
        install_canonical();
        set_active_move(real_move(MoveId::Tackle));
        let mut state = EngineState::new(
            vec![engine_battler(&Mon::new(Species::Tauros, 300, 110), MoveId::Tackle)],
            vec![engine_battler(&Mon::new(Species::Snorlax, 500, 40), MoveId::Tackle)],
        );
        let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
        if reflect {
            effects.push(EffectState {
                id: EffectId(200),
                host: BattlerRef::OPPONENT,
                effect_order: 0,
                kind: PokeVolatile::Reflect,
            });
        }
        let actions = [
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
        ];
        let mut rng = ScriptedRng::new(vec![0u8; 64]);
        let (_r, log) = StackDriver::execute_turn_logged(
            &PokeredRules, &mut state, &mut effects, actions, &mut rng,
        );
        log_net_damage(&log, BattlerRef::OPPONENT)
    };
    let plain = tackle_damage(false);
    let shielded = tackle_damage(true);
    assert!(plain > 0 && shielded > 0, "both hits must deal damage ({plain}, {shielded})");
    assert!(
        shielded < plain,
        "Reflect must reduce physical damage: shielded {shielded} vs plain {plain}"
    );
}

/// Mist installs its self-volatile (setter) AND — via the already-wired
/// `effect_for_volatile` TryBoost veto — blocks a foe stat-down move (consumer).
#[test]
fn mist_installs_and_blocks_foe_stat_down_via_decoupled_stack() {
    // Setter: a mon uses Mist -> the Mist volatile lands on the user.
    install_canonical();
    set_active_move(real_move(MoveId::Mist));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Alakazam, 200, 110), MoveId::Mist)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 200, 40), MoveId::Mist)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Mist },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Mist },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(
        effects
            .iter()
            .any(|e| e.host == BattlerRef::PLAYER && matches!(e.kind, PokeVolatile::Mist)),
        "Mist must install its volatile on the user"
    );

    // Consumer: with Mist up on PLAYER, the OPPONENT's Growl cannot lower PLAYER's
    // Attack. Both mons Growl (power 0 -> nobody faints); we watch PLAYER's stage.
    let player_attack_drop = |mist: bool| -> i32 {
        install_canonical();
        set_active_move(real_move(MoveId::Growl));
        let mut state = EngineState::new(
            vec![engine_battler(&Mon::new(Species::Snorlax, 300, 40), MoveId::Growl)],
            vec![engine_battler(&Mon::new(Species::Tauros, 300, 110), MoveId::Growl)],
        );
        let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
        if mist {
            effects.push(EffectState {
                id: EffectId(300),
                host: BattlerRef::PLAYER,
                effect_order: 0,
                kind: PokeVolatile::Mist,
            });
        }
        let actions = [
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Growl },
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Growl },
        ];
        let mut rng = ScriptedRng::new(vec![0u8; 64]);
        let (_r, log) = StackDriver::execute_turn_logged(
            &PokeredRules, &mut state, &mut effects, actions, &mut rng,
        );
        log_stat_delta(&log, BattlerRef::PLAYER, StatIndex::Attack)
    };
    assert_eq!(player_attack_drop(false), -1, "Growl lowers an unprotected mon's Attack");
    assert_eq!(player_attack_drop(true), 0, "Mist must block the foe's Attack drop");
}

/// Counter reflects 2× the PHYSICAL damage the user took this turn (Gen-1 bug #20),
/// on the LIVE decoupled stack: the opponent's Tackle stamps the Counter user's
/// per-turn `DamageTaken` scratch, then Counter (−1 priority → moves last) reads it
/// and reflects via the load-bearing `pair_mut`. Asserted on state (the reflect is
/// applied through pair_mut, bypassing the driver's damage-log).
#[test]
fn counter_reflects_twice_physical_damage_via_decoupled_stack() {
    install_canonical();
    clear_current_moves();
    // Player uses Counter (−1 priority → last); opponent uses Tackle (physical).
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Counter));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Tackle));
    let (p_hp0, e_hp0) = (5000u16, 5000u16);
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Snorlax, p_hp0, 50), MoveId::Counter)],
        vec![engine_battler(&Mon::new(Species::Tauros, e_hp0, 200), MoveId::Tackle)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Counter },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    let player_loss = p_hp0 - state.player_battlers[0].hp;
    let opp_loss = e_hp0 - state.opponent_battlers[0].hp;
    assert!(player_loss > 0, "Counter user took the opponent's physical hit");
    assert_eq!(opp_loss, player_loss * 2, "Counter reflected 2× the physical damage taken");
}

/// Counter deals NOTHING when the damage taken was SPECIAL (Thundershock is Electric
/// → special in Gen-1): the `DamageTaken.physical` flag is false, so `counter_handler`
/// fails. Only the opponent's hit lands; the opponent is untouched.
#[test]
fn counter_ignores_special_damage_via_decoupled_stack() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Counter));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Thundershock));
    let (p_hp0, e_hp0) = (5000u16, 5000u16);
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Snorlax, p_hp0, 50), MoveId::Counter)],
        vec![engine_battler(&Mon::new(Species::Raichu, e_hp0, 200), MoveId::Thundershock)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Counter },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Thundershock },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(state.player_battlers[0].hp < p_hp0, "Counter user took the special hit");
    assert_eq!(
        state.opponent_battlers[0].hp, e_hp0,
        "Counter does NOT reflect special damage (opponent untouched)"
    );
}

/// Counter does NOT reflect a PHYSICAL move that isn't Normal/Fighting (Earthquake is
/// Ground → physical, but Counter-ineligible in Gen-1). This pins the faithful
/// NORMAL/FIGHTING rule vs the naive "any physical" proxy: the opponent stays untouched.
#[test]
fn counter_ignores_ground_physical_damage_via_decoupled_stack() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Counter));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Earthquake));
    let (p_hp0, e_hp0) = (5000u16, 5000u16);
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Snorlax, p_hp0, 50), MoveId::Counter)],
        vec![engine_battler(&Mon::new(Species::Rhydon, e_hp0, 200), MoveId::Earthquake)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Counter },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Earthquake },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(state.player_battlers[0].hp < p_hp0, "Counter user took the Earthquake hit");
    assert_eq!(
        state.opponent_battlers[0].hp, e_hp0,
        "Counter must NOT reflect Ground (physical but non-N/F) damage"
    );
}

/// A `Recharge` volatile forces the mon to skip its turn (Hyper Beam recharge) via
/// the engine's generic `forced_action` seam: the chosen Tackle is IGNORED, so the
/// opponent is untouched by the recharging mon.
#[test]
fn recharge_volatile_forces_nothing_via_decoupled_stack() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Tackle));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let (p_hp0, e_hp0) = (500u16, 500u16);
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Snorlax, p_hp0, 200), MoveId::Tackle)],
        vec![engine_battler(&Mon::new(Species::Snorlax, e_hp0, 50), MoveId::Splash)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = vec![EffectState {
        id: EffectId(400),
        host: BattlerRef::PLAYER,
        effect_order: 0,
        kind: PokeVolatile::Recharge,
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(
        state.opponent_battlers[0].hp, e_hp0,
        "the recharging mon's chosen Tackle was skipped (forced Nothing)"
    );
}

/// Hyper Beam installs the recharge volatile on the user when the target SURVIVES.
#[test]
fn hyper_beam_installs_recharge_via_decoupled_stack() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::HyperBeam));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Snorlax, 500, 200), MoveId::HyperBeam)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 5000, 50), MoveId::Splash)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::HyperBeam },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(state.opponent_battlers[0].hp < 5000, "Hyper Beam dealt damage");
    assert!(
        effects
            .iter()
            .any(|e| e.host == BattlerRef::PLAYER && matches!(e.kind, PokeVolatile::Recharge)),
        "Hyper Beam installs recharge when the target survives"
    );
}

/// Gen-1 quirk: a Hyper Beam that KOs the target needs NO recharge — no volatile.
#[test]
fn hyper_beam_ko_skips_recharge_via_decoupled_stack() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::HyperBeam));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Snorlax, 500, 200), MoveId::HyperBeam)],
        vec![engine_battler(&Mon::new(Species::Magikarp, 8, 50), MoveId::Splash)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::HyperBeam },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(state.opponent_battlers[0].hp, 0, "Hyper Beam KO'd the target");
    assert!(
        !effects
            .iter()
            .any(|e| e.host == BattlerRef::PLAYER && matches!(e.kind, PokeVolatile::Recharge)),
        "a KO Hyper Beam must NOT install recharge (Gen-1 quirk)"
    );
}

// ── Two-turn charge moves (Fly / Dig / Solar Beam) ──

/// A charge move's GATHER turn installs the Charging volatile and deals no damage.
#[test]
fn fly_charge_turn_installs_volatile_no_damage() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Fly));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Pidgeot, 300, 200), MoveId::Fly)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 500, 50), MoveId::Splash)],
    );
    let e_hp0 = state.opponent_battlers[0].hp;
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Fly },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(state.opponent_battlers[0].hp, e_hp0, "the charge turn deals no damage");
    assert!(
        effects.iter().any(|e| e.host == BattlerRef::PLAYER
            && matches!(e.kind, PokeVolatile::Charging { move_: MoveId::Fly, invulnerable: true })),
        "Fly's gather turn installs the Charging volatile"
    );
}

/// The STRIKE turn (Charging pre-set) forces the move, lands damage, and clears the
/// volatile.
#[test]
fn fly_strike_turn_deals_damage_and_clears() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Fly));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Pidgeot, 300, 200), MoveId::Fly)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 5000, 50), MoveId::Splash)],
    );
    let e_hp0 = state.opponent_battlers[0].hp;
    let mut effects: Vec<EffectState<PokeredRules>> = vec![EffectState {
        id: EffectId(500),
        host: BattlerRef::PLAYER,
        effect_order: 0,
        kind: PokeVolatile::Charging { move_: MoveId::Fly, invulnerable: true },
    }];
    // The chosen action is Splash; forced_action must override it to the charging Fly.
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(state.opponent_battlers[0].hp < e_hp0, "the strike turn lands the hit");
    assert!(
        !effects.iter().any(|e| matches!(e.kind, PokeVolatile::Charging { .. })),
        "the Charging volatile is consumed on the strike"
    );
}

/// A charge move that MISSES on its strike turn still ends the charge — the mon must
/// not stay stuck invulnerable (accuracy-miss returns before pokered_damage, so the
/// OnMiss handler clears it).
#[test]
fn fly_strike_miss_still_ends_the_charge() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Fly));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Pidgeot, 300, 200), MoveId::Fly)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 5000, 50), MoveId::Splash)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = vec![EffectState {
        id: EffectId(510),
        host: BattlerRef::PLAYER,
        effect_order: 0,
        kind: PokeVolatile::Charging { move_: MoveId::Fly, invulnerable: true },
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Fly },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    // All-0xFF stream → the accuracy roll misses (byte ≥ scaled accuracy).
    let mut rng = ScriptedRng::new(vec![0xFFu8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(
        !effects.iter().any(|e| matches!(e.kind, PokeVolatile::Charging { .. })),
        "a missed strike still clears the Charging volatile (no stuck invulnerability)"
    );
}

/// A semi-invulnerable Fly target can't be hit by an ordinary move (opponent moves
/// first while the mon is still up); the mon then strikes.
#[test]
fn fly_invulnerable_target_cannot_be_hit() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Fly));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Tackle));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Pidgeot, 3000, 50), MoveId::Fly)], // slow → strikes last
        vec![engine_battler(&Mon::new(Species::Tauros, 3000, 200), MoveId::Tackle)], // fast → hits first
    );
    let (p_hp0, e_hp0) = (state.player_battlers[0].hp, state.opponent_battlers[0].hp);
    let mut effects: Vec<EffectState<PokeredRules>> = vec![EffectState {
        id: EffectId(500),
        host: BattlerRef::PLAYER,
        effect_order: 0,
        kind: PokeVolatile::Charging { move_: MoveId::Fly, invulnerable: true },
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Fly },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(state.player_battlers[0].hp, p_hp0, "Tackle misses the invulnerable (flying) mon");
    assert!(state.opponent_battlers[0].hp < e_hp0, "the mon still strikes on the strike turn");
}

/// Gen 1 has NO Gust/Thunder vs Fly (or Earthquake/Fissure vs Dig) exception —
/// those are Gen 2+. Gust MISSES a mid-Fly target; only Swift bypasses (its
/// `ret z` precedes the invulnerability check, core.asm:5246-5248).
#[test]
fn gust_misses_fly_invulnerability_gen1() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Fly));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Gust));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Pidgeot, 3000, 50), MoveId::Fly)],
        vec![engine_battler(&Mon::new(Species::Pidgeot, 3000, 200), MoveId::Gust)],
    );
    let p_hp0 = state.player_battlers[0].hp;
    let mut effects: Vec<EffectState<PokeredRules>> = vec![EffectState {
        id: EffectId(500),
        host: BattlerRef::PLAYER,
        effect_order: 0,
        kind: PokeVolatile::Charging { move_: MoveId::Fly, invulnerable: true },
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Fly },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Gust },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(state.player_battlers[0].hp, p_hp0, "Gen 1: Gust canNOT hit a mid-Fly target");
}

/// The ONE bypass: Swift hits a mid-Fly target (never misses, and its check
/// returns before the semi-invulnerability test in the original).
#[test]
fn swift_hits_through_fly_invulnerability() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Fly));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Swift));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Pidgeot, 3000, 50), MoveId::Fly)],
        vec![engine_battler(&Mon::new(Species::Persian, 3000, 200), MoveId::Swift)],
    );
    let p_hp0 = state.player_battlers[0].hp;
    let mut effects: Vec<EffectState<PokeredRules>> = vec![EffectState {
        id: EffectId(500),
        host: BattlerRef::PLAYER,
        effect_order: 0,
        kind: PokeVolatile::Charging { move_: MoveId::Fly, invulnerable: true },
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Fly },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Swift },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(state.player_battlers[0].hp < p_hp0, "Swift still hits a mid-Fly target");
}

/// Solar Beam (ChargeEffect, unlike Fly's FlyEffect) charges + strikes, but does NOT
/// grant invulnerability — an opponent's ordinary move HITS the charging mon.
#[test]
fn solarbeam_strikes_but_is_not_invulnerable() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Solarbeam));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Tackle));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Venusaur, 3000, 50), MoveId::Solarbeam)], // slow → strikes last
        vec![engine_battler(&Mon::new(Species::Tauros, 3000, 200), MoveId::Tackle)], // fast → hits first
    );
    let (p_hp0, e_hp0) = (state.player_battlers[0].hp, state.opponent_battlers[0].hp);
    let mut effects: Vec<EffectState<PokeredRules>> = vec![EffectState {
        id: EffectId(500),
        host: BattlerRef::PLAYER,
        effect_order: 0,
        kind: PokeVolatile::Charging { move_: MoveId::Solarbeam, invulnerable: false },
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Solarbeam },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(state.player_battlers[0].hp < p_hp0, "Tackle HITS the non-invulnerable Solar Beam charger");
    assert!(state.opponent_battlers[0].hp < e_hp0, "the forced Solar Beam strikes");
}

// ── Thrash / Petal Dance lock-in ──

/// A first Thrash installs the rampage lock (2–3 uses; rng&1 + 2 = 2 here).
#[test]
fn thrash_first_use_installs_lock() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Thrash));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 3000, 200), MoveId::Thrash)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 5000, 50), MoveId::Splash)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Thrash },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(
        effects.iter().any(|e| e.host == BattlerRef::PLAYER
            && matches!(e.kind, PokeVolatile::LockedMove { move_: MoveId::Thrash, turns_left: 2, confuse_on_end: true })),
        "first Thrash installs the 2-use lock"
    );
}

/// On exhaustion the lock is removed and the user self-confuses (Gen-1 fatigue).
#[test]
fn thrash_exhaustion_self_confuses() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Thrash));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 3000, 200), MoveId::Thrash)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 5000, 50), MoveId::Splash)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = vec![EffectState {
        id: EffectId(600),
        host: BattlerRef::PLAYER,
        effect_order: 0,
        kind: PokeVolatile::LockedMove { move_: MoveId::Thrash, turns_left: 1, confuse_on_end: true },
    }];
    // The chosen action is Splash; forced_action must override it to the locked Thrash.
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(
        !effects.iter().any(|e| matches!(e.kind, PokeVolatile::LockedMove { .. })),
        "the rampage lock is removed on exhaustion"
    );
    assert!(
        effects.iter().any(|e| e.host == BattlerRef::PLAYER && matches!(e.kind, PokeVolatile::Confused { .. })),
        "the user self-confuses after the rampage ends"
    );
}

// ── Rage ──

/// Using Rage installs the lock-in volatile on the user.
#[test]
fn rage_use_installs_lock() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Rage));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 3000, 200), MoveId::Rage)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 5000, 50), MoveId::Splash)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Rage },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(
        effects.iter().any(|e| e.host == BattlerRef::PLAYER && matches!(e.kind, PokeVolatile::Rage)),
        "Rage locks the user in"
    );
}

/// A raging mon's Attack rises one stage each time it is hit by a damaging move.
#[test]
fn rage_attack_rises_when_hit() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Rage));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Tackle));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Snorlax, 5000, 50), MoveId::Rage)], // slow → hit first
        vec![engine_battler(&Mon::new(Species::Tauros, 5000, 200), MoveId::Tackle)], // fast → hits
    );
    let mut effects: Vec<EffectState<PokeredRules>> = vec![EffectState {
        id: EffectId(700),
        host: BattlerRef::PLAYER,
        effect_order: 0,
        kind: PokeVolatile::Rage,
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Rage },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(
        state.player_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
        1,
        "the raging mon's Attack rose after taking a hit"
    );
}

// ── Trapping moves (Wrap / Bind / Fire Spin / Clamp) ──

/// Wrap installs the trap and BINDS the foe: moving first, the wrapper installs
/// Trapping, so the foe (moving second) is forced to do Nothing (its Tackle skipped).
#[test]
fn wrap_installs_trap_and_binds_the_foe() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Wrap));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Tackle));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 5000, 200), MoveId::Wrap)], // fast → wraps first
        vec![engine_battler(&Mon::new(Species::Snorlax, 5000, 50), MoveId::Tackle)],
    );
    let (p_hp0, e_hp0) = (state.player_battlers[0].hp, state.opponent_battlers[0].hp);
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Wrap },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(
        effects.iter().any(|e| e.host == BattlerRef::PLAYER && matches!(e.kind, PokeVolatile::Trapping { move_: MoveId::Wrap, .. })),
        "Wrap installs the trapping volatile"
    );
    assert_eq!(state.player_battlers[0].hp, p_hp0, "the bound foe's Tackle was skipped");
    assert!(state.opponent_battlers[0].hp < e_hp0, "Wrap dealt damage");
}

/// A trapping user in progress is forced to re-issue the move (menu ignored) and the
/// foe stays bound.
#[test]
fn trapped_user_is_forced_and_foe_stays_bound() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Wrap));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Tackle));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 5000, 200), MoveId::Wrap)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 5000, 50), MoveId::Tackle)],
    );
    let (p_hp0, e_hp0) = (state.player_battlers[0].hp, state.opponent_battlers[0].hp);
    let mut effects: Vec<EffectState<PokeredRules>> = vec![EffectState {
        id: EffectId(800),
        host: BattlerRef::PLAYER,
        effect_order: 0,
        kind: PokeVolatile::Trapping { move_: MoveId::Wrap, turns_left: 2 },
    }];
    // Player's chosen action is Splash; forced_action must re-issue Wrap.
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(state.opponent_battlers[0].hp < e_hp0, "the forced Wrap re-hit (Splash ignored)");
    assert_eq!(state.player_battlers[0].hp, p_hp0, "the bound foe's Tackle was skipped");
}

/// Trapping duration uses the multi-hit WEIGHTS (3/8 for 2–3, 1/8 for 4–5 —
/// effects.asm TrappingEffect re-rolls `& 3` when the first draw ≥ 2), not a
/// flat `& 3`. A duration byte of 200 lands in the 192..224 bucket ⇒ 4 turns.
#[test]
fn wrap_duration_uses_multi_hit_weights() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Wrap));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 5000, 200), MoveId::Wrap)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 5000, 50), MoveId::Splash)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Wrap },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    // All-200 stream: every roll sees 200 — the damage-roll rejection loop
    // terminates via its bound, and the trapping-duration draw reads 200.
    let mut rng = ScriptedRng::new(vec![200u8; 256]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    let turns = effects.iter().find_map(|e| match &e.kind {
        PokeVolatile::Trapping { turns_left, .. } if e.host == BattlerRef::PLAYER => Some(*turns_left),
        _ => None,
    });
    assert_eq!(turns, Some(3), "duration byte 200 ⇒ 4 turns (stored as turns − 1 = 3)");
}

// ── Bide ──

/// While storing, Bide deals no damage and folds the damage it takes into its
/// accumulator (decrementing the store counter).
#[test]
fn bide_accumulates_damage_taken() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Bide));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Tackle));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Snorlax, 5000, 50), MoveId::Bide)], // slow → hit before its residual
        vec![engine_battler(&Mon::new(Species::Tauros, 5000, 200), MoveId::Tackle)],
    );
    let (p_hp0, e_hp0) = (state.player_battlers[0].hp, state.opponent_battlers[0].hp);
    let mut effects: Vec<EffectState<PokeredRules>> = vec![EffectState {
        id: EffectId(900),
        host: BattlerRef::PLAYER,
        effect_order: 0,
        kind: PokeVolatile::Bide { turns_left: 2, accumulated: 0 },
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Bide },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    let taken = p_hp0 - state.player_battlers[0].hp;
    assert!(taken > 0, "the Bide user took the opponent's hit");
    let bide = effects.iter().find_map(|e| match &e.kind {
        PokeVolatile::Bide { turns_left, accumulated } if e.host == BattlerRef::PLAYER => Some((*turns_left, *accumulated)),
        _ => None,
    });
    assert_eq!(bide, Some((1, taken)), "Bide folds the damage taken and decrements 2→1");
    assert_eq!(state.opponent_battlers[0].hp, e_hp0, "Bide deals no damage while storing");
}

/// Bide stores for 2 OR 3 turns at random — effects.asm:782-786
/// `(BattleRandom & 1) + 2`. An even duration byte installs a 2-turn store, an
/// odd byte a 3-turn store (observed after the same-turn residual decrement:
/// 2→1 vs 3→2).
#[test]
fn bide_duration_rolls_two_or_three_turns() {
    for (byte, expected) in [(0u8, 1u8), (1u8, 2u8)] {
        install_canonical();
        clear_current_moves();
        set_current_move(BattlerRef::PLAYER, real_move(MoveId::Bide));
        set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
        let mut state = EngineState::new(
            vec![engine_battler(&Mon::new(Species::Snorlax, 5000, 50), MoveId::Bide)],
            vec![engine_battler(&Mon::new(Species::Tauros, 5000, 200), MoveId::Splash)],
        );
        let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
        let actions = [
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Bide },
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
        ];
        let mut rng = ScriptedRng::new(vec![byte; 64]);
        let _ = StackDriver::execute_turn_logged(
            &PokeredRules, &mut state, &mut effects, actions, &mut rng,
        );
        let turns = effects.iter().find_map(|e| match &e.kind {
            PokeVolatile::Bide { turns_left, .. } if e.host == BattlerRef::PLAYER => Some(*turns_left),
            _ => None,
        });
        assert_eq!(
            turns,
            Some(expected),
            "byte {byte}: install {} turns, ticked to {expected}",
            expected + 1
        );
    }
}

/// On exhaustion Bide unleashes accumulated × 2 (Gen-1 ×2, not ×3) and clears.
#[test]
fn bide_unleashes_double_accumulated() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Bide));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Snorlax, 5000, 200), MoveId::Bide)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 5000, 50), MoveId::Splash)],
    );
    let e_hp0 = state.opponent_battlers[0].hp;
    let mut effects: Vec<EffectState<PokeredRules>> = vec![EffectState {
        id: EffectId(901),
        host: BattlerRef::PLAYER,
        effect_order: 0,
        kind: PokeVolatile::Bide { turns_left: 1, accumulated: 100 },
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Bide },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(state.opponent_battlers[0].hp, e_hp0 - 200, "Bide unleashes accumulated × 2");
    assert!(
        !effects.iter().any(|e| matches!(e.kind, PokeVolatile::Bide { .. })),
        "the Bide volatile is cleared after unleashing"
    );
}

// ── Transform ──

/// Transform copies the target's species (and stats/moves) onto the user's engine
/// battler and installs the one-shot Transformed marker.
#[test]
fn transform_copies_target_identity() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Transform));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Ditto, 200, 50), MoveId::Transform)],
        vec![engine_battler(&Mon::new(Species::Tauros, 400, 200), MoveId::Tackle)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Transform },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(state.player_battlers[0].species, Species::Tauros, "user takes the target's species");
    assert!(
        effects.iter().any(|e| e.host == BattlerRef::PLAYER && matches!(e.kind, PokeVolatile::Transformed)),
        "the Transformed marker is installed"
    );
}

// ── Substitute (替身) — phase 1: create + formula absorb ──

/// Substitute raises a doll costing max_hp/4 HP, with that many HP (SubstituteHp).
#[test]
fn substitute_creates_the_doll() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Substitute));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Snorlax, 400, 100), MoveId::Substitute)],
        vec![engine_battler(&Mon::new(Species::Pikachu, 200, 50), MoveId::Splash)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Substitute },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    let sub_hp = effects.iter().find_map(|e| match &e.kind {
        PokeVolatile::SubstituteHp { hp } if e.host == BattlerRef::PLAYER => Some(*hp),
        _ => None,
    });
    assert_eq!(sub_hp, Some(100), "the doll has max_hp/4 = 100 HP");
    assert_eq!(state.player_battlers[0].hp, 300, "the user paid max_hp/4 = 100 HP");
}

/// Substitute creation preserves the Gen-1 bug #28: HP == cost SUCCEEDS, leaving the
/// user at 0 HP; and it FAILS (no-op) if a Substitute is already up or HP < cost.
#[test]
fn substitute_creation_edge_cases() {
    install_canonical();
    // (a) HP exactly == cost ⇒ succeeds, user left at 0 (bug #28).
    {
        clear_current_moves();
        set_current_move(BattlerRef::PLAYER, real_move(MoveId::Substitute));
        set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
        let mut state = EngineState::new(
            vec![engine_battler(&Mon::new(Species::Snorlax, 400, 100), MoveId::Substitute)],
            vec![engine_battler(&Mon::new(Species::Pikachu, 200, 50), MoveId::Splash)],
        );
        state.player_battlers[0].hp = 100; // == cost (400/4)
        let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
        let actions = [
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Substitute },
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
        ];
        let mut rng = ScriptedRng::new(vec![0u8; 64]);
        let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
        assert!(
            effects.iter().any(|e| matches!(e.kind, PokeVolatile::SubstituteHp { .. })),
            "hp == cost still raises the doll (bug #28)"
        );
        assert_eq!(state.player_battlers[0].hp, 0, "left at 0 HP (bug #28)");
    }
    // (b) HP < cost ⇒ fails, no doll, HP untouched.
    {
        clear_current_moves();
        set_current_move(BattlerRef::PLAYER, real_move(MoveId::Substitute));
        set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
        let mut state = EngineState::new(
            vec![engine_battler(&Mon::new(Species::Snorlax, 400, 100), MoveId::Substitute)],
            vec![engine_battler(&Mon::new(Species::Pikachu, 200, 50), MoveId::Splash)],
        );
        state.player_battlers[0].hp = 99; // < cost
        let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
        let actions = [
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Substitute },
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
        ];
        let mut rng = ScriptedRng::new(vec![0u8; 64]);
        let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
        assert!(
            !effects.iter().any(|e| matches!(e.kind, PokeVolatile::SubstituteHp { .. })),
            "hp < cost fails to raise a doll"
        );
        assert_eq!(state.player_battlers[0].hp, 99, "a failed Substitute costs no HP");
    }
}

/// A Substitute absorbs an incoming hit: the defender's REAL HP is untouched and the
/// doll's HP drops by the damage dealt (the formula-damage `Event::Damage` seam).
#[test]
fn substitute_absorbs_the_hit() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Tackle));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 400, 200), MoveId::Tackle)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 2000, 50), MoveId::Splash)],
    );
    // The enemy holds a big Substitute so one hit cannot break it.
    let mut effects = vec![EffectState {
        id: EffectId(0x9300),
        host: BattlerRef::OPPONENT,
        effect_order: 999,
        kind: PokeVolatile::SubstituteHp { hp: 1000 },
    }];
    let e_hp0 = state.opponent_battlers[0].hp;
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    assert_eq!(state.opponent_battlers[0].hp, e_hp0, "the mon's real HP is fully protected");
    let sub_left = effects.iter().find_map(|e| match &e.kind {
        PokeVolatile::SubstituteHp { hp } if e.host == BattlerRef::OPPONENT => Some(*hp),
        _ => None,
    });
    assert!(matches!(sub_left, Some(hp) if hp < 1000), "the doll absorbed the damage: {sub_left:?}");
}

/// An overkill hit breaks the Substitute (volatile removed) and the mon STILL takes
/// nothing — Gen-1 does not spill the overflow into the real HP.
#[test]
fn substitute_breaks_without_overflow() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Tackle));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 400, 200), MoveId::Tackle)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 2000, 50), MoveId::Splash)],
    );
    // A 1-HP doll: any hit overkills it.
    let mut effects = vec![EffectState {
        id: EffectId(0x9300),
        host: BattlerRef::OPPONENT,
        effect_order: 999,
        kind: PokeVolatile::SubstituteHp { hp: 1 },
    }];
    let e_hp0 = state.opponent_battlers[0].hp;
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    assert!(
        !effects.iter().any(|e| e.host == BattlerRef::OPPONENT && matches!(e.kind, PokeVolatile::SubstituteHp { .. })),
        "the doll broke (volatile removed)"
    );
    assert_eq!(state.opponent_battlers[0].hp, e_hp0, "the overkill does NOT spill into real HP");
}

// ── Substitute phase 2: direct-mutate ops vs the doll (redirect_hp_loss seam) ──

/// Super Fang deals `mon_curHP/2` (read from the MON, per the oracle) into the DOLL —
/// the mon's real HP is untouched. Exercises the `redirect_hp_loss` binding routing a
/// `DamageCurrentHpFraction` op (a DamagingHit direct-mutate that bypasses Event::Damage).
#[test]
fn super_fang_hits_the_substitute() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::SuperFang));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 400, 200), MoveId::SuperFang)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 2000, 50), MoveId::Splash)],
    );
    let mut effects = vec![EffectState {
        id: EffectId(0x9400),
        host: BattlerRef::OPPONENT,
        effect_order: 999,
        kind: PokeVolatile::SubstituteHp { hp: 1500 },
    }];
    let e_hp0 = state.opponent_battlers[0].hp; // 2000
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::SuperFang },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    assert_eq!(state.opponent_battlers[0].hp, e_hp0, "the mon's real HP is untouched — Super Fang hit the doll");
    let sub = effects.iter().find_map(|e| match &e.kind {
        PokeVolatile::SubstituteHp { hp } if e.host == BattlerRef::OPPONENT => Some(*hp),
        _ => None,
    });
    assert_eq!(sub, Some(500), "the doll absorbed mon_curHP/2 = 1000 (1500 - 1000)");
}

/// An OHKO move (Horn Drill) breaks the DOLL instead of KO-ing the mon — the mon
/// survives. Exercises `redirect_hp_loss` routing a `SetHp(Target,0)` op's implied loss.
#[test]
fn ohko_breaks_the_substitute() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::HornDrill));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 400, 200), MoveId::HornDrill)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 2000, 50), MoveId::Splash)],
    );
    let mut effects = vec![EffectState {
        id: EffectId(0x9400),
        host: BattlerRef::OPPONENT,
        effect_order: 999,
        kind: PokeVolatile::SubstituteHp { hp: 1500 },
    }];
    let e_hp0 = state.opponent_battlers[0].hp;
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::HornDrill },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    assert!(
        !effects.iter().any(|e| e.host == BattlerRef::OPPONENT && matches!(e.kind, PokeVolatile::SubstituteHp { .. })),
        "the OHKO broke the doll"
    );
    assert_eq!(state.opponent_battlers[0].hp, e_hp0, "the mon survived — the OHKO hit the doll, not the mon");
}

/// A multi-hit move (Double Kick, 2 hits) sends BOTH hits into the doll — hit #1 via
/// the driver's Event::Damage, hit #2 via `redirect_hp_loss` on the RepeatHits op. The
/// doll drops by the move's FULL two-hit total (a naive impl reads the post-absorb
/// `ctx.mv.damage` of 0 for the re-hits, so only hit #1 would land).
#[test]
fn multi_hit_all_hits_hit_the_substitute() {
    install_canonical();
    // The two-hit total against a bare mon (both hits real).
    let total = {
        clear_current_moves();
        set_current_move(BattlerRef::PLAYER, real_move(MoveId::DoubleKick));
        set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
        let mut state = EngineState::new(
            vec![engine_battler(&Mon::new(Species::Tauros, 400, 200), MoveId::DoubleKick)],
            vec![engine_battler(&Mon::new(Species::Snorlax, 4000, 50), MoveId::Splash)],
        );
        let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
        let actions = [
            BattleAction::<PokeredRules>::Fight { move_: MoveId::DoubleKick },
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
        ];
        let mut rng = ScriptedRng::new(vec![0u8; 64]);
        let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
        4000 - state.opponent_battlers[0].hp
    };
    assert!(total > 0, "Double Kick dealt damage");

    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::DoubleKick));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 400, 200), MoveId::DoubleKick)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 4000, 50), MoveId::Splash)],
    );
    let mut effects = vec![EffectState {
        id: EffectId(0x9400),
        host: BattlerRef::OPPONENT,
        effect_order: 999,
        kind: PokeVolatile::SubstituteHp { hp: 3900 },
    }];
    let e_hp0 = state.opponent_battlers[0].hp;
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::DoubleKick },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    assert_eq!(state.opponent_battlers[0].hp, e_hp0, "BOTH hits went to the doll — the mon's real HP is untouched");
    let sub = effects.iter().find_map(|e| match &e.kind {
        PokeVolatile::SubstituteHp { hp } if e.host == BattlerRef::OPPONENT => Some(*hp),
        _ => None,
    });
    assert_eq!(sub, Some(3900 - total), "the doll absorbed BOTH hits (full two-hit total), not just hit #1");
}

/// Recoil and drain read the REAL damage the move dealt to a doll (the oracle bases
/// them on the formula number even against a sub): Double-Edge still recoils the user,
/// Mega Drain still heals it — the Event::Damage fold must not zero `last_damage`.
#[test]
fn recoil_and_drain_read_real_damage_through_a_sub() {
    install_canonical();
    // (a) Double-Edge into a foe's sub: the user STILL takes recoil.
    {
        clear_current_moves();
        set_current_move(BattlerRef::PLAYER, real_move(MoveId::DoubleEdge));
        set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
        let mut state = EngineState::new(
            vec![engine_battler(&Mon::new(Species::Tauros, 2000, 200), MoveId::DoubleEdge)],
            vec![engine_battler(&Mon::new(Species::Snorlax, 4000, 50), MoveId::Splash)],
        );
        let mut effects = vec![EffectState {
            id: EffectId(0x9500), host: BattlerRef::OPPONENT, effect_order: 999,
            kind: PokeVolatile::SubstituteHp { hp: 3900 },
        }];
        let (p_hp0, e_hp0) = (state.player_battlers[0].hp, state.opponent_battlers[0].hp);
        let actions = [
            BattleAction::<PokeredRules>::Fight { move_: MoveId::DoubleEdge },
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
        ];
        let mut rng = ScriptedRng::new(vec![0u8; 64]);
        let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
        assert!(state.player_battlers[0].hp < p_hp0, "the user recoils off the real damage even though the sub absorbed it");
        assert_eq!(state.opponent_battlers[0].hp, e_hp0, "the foe's real HP is protected by its doll");
    }
    // (b) Mega Drain into a foe's sub: the user STILL heals.
    {
        clear_current_moves();
        set_current_move(BattlerRef::PLAYER, real_move(MoveId::MegaDrain));
        set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
        let mut state = EngineState::new(
            vec![engine_battler(&Mon::new(Species::Tauros, 2000, 200), MoveId::MegaDrain)],
            vec![engine_battler(&Mon::new(Species::Snorlax, 4000, 50), MoveId::Splash)],
        );
        state.player_battlers[0].hp = 500; // hurt, so the drain heal is observable
        let mut effects = vec![EffectState {
            id: EffectId(0x9500), host: BattlerRef::OPPONENT, effect_order: 999,
            kind: PokeVolatile::SubstituteHp { hp: 3900 },
        }];
        let actions = [
            BattleAction::<PokeredRules>::Fight { move_: MoveId::MegaDrain },
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
        ];
        let mut rng = ScriptedRng::new(vec![0u8; 64]);
        let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
        assert!(state.player_battlers[0].hp > 500, "the user heals from draining the doll (real damage)");
    }
}

/// An OHKO breaks the doll UNCONDITIONALLY — even when the mon's current HP is BELOW
/// the doll's HP (so a naive "absorb mon_hp into the doll" would leave the doll alive).
#[test]
fn ohko_breaks_a_sub_below_the_mon_hp() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::HornDrill));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 400, 200), MoveId::HornDrill)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 2000, 50), MoveId::Splash)],
    );
    state.opponent_battlers[0].hp = 100; // mon HP (100) < doll HP (1500)
    let mut effects = vec![EffectState {
        id: EffectId(0x9500), host: BattlerRef::OPPONENT, effect_order: 999,
        kind: PokeVolatile::SubstituteHp { hp: 1500 },
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::HornDrill },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    assert!(
        !effects.iter().any(|e| e.host == BattlerRef::OPPONENT && matches!(e.kind, PokeVolatile::SubstituteHp { .. })),
        "the OHKO broke the doll unconditionally (mon HP < doll HP)"
    );
    assert_eq!(state.opponent_battlers[0].hp, 100, "the mon survived");
}

/// The per-turn "raised a doll" flag survives a SAME-TURN break, so the game can
/// still narrate "put in a SUBSTITUTE!" even when the doll is created and broken in one
/// turn — and clear_current_moves resets it before the next turn.
#[test]
fn substitute_created_flag_survives_a_same_turn_break() {
    install_canonical();
    clear_current_moves();
    assert!(!sub_created_this_turn(BattlerRef::PLAYER), "flag starts clear");
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Substitute));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Tackle));
    // Fast Pikachu raises a tiny 10-HP doll; slow, strong Tauros breaks it the same turn.
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Pikachu, 40, 200), MoveId::Substitute)],
        vec![engine_battler(&Mon::new(Species::Tauros, 1000, 50), MoveId::Tackle)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Substitute },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    assert!(
        !effects.iter().any(|e| e.host == BattlerRef::PLAYER && matches!(e.kind, PokeVolatile::SubstituteHp { .. })),
        "the doll was created then broken the same turn (none remains)"
    );
    assert!(
        sub_created_this_turn(BattlerRef::PLAYER),
        "the raise flag survives the same-turn break (so the creation line still narrates)"
    );
    clear_current_moves();
    assert!(!sub_created_this_turn(BattlerRef::PLAYER), "the flag is reset before the next turn");
}

/// Counter behind a Substitute reflects the REAL damage the hit dealt (to the doll),
/// not 0 — Gen-1 computes wDamage before the sub absorbs, so Counter still reads it.
/// The doll protects the mon's real HP; Counter (−1 priority) then reflects 2×.
#[test]
fn counter_behind_a_sub_reflects_the_real_damage() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Counter));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Tackle));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 2000, 50), MoveId::Counter)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 2000, 100), MoveId::Tackle)],
    );
    // The player holds a doll so the enemy's (NORMAL, counterable) Tackle is absorbed.
    let mut effects = vec![EffectState {
        id: EffectId(0x9600),
        host: BattlerRef::PLAYER,
        effect_order: 999,
        kind: PokeVolatile::SubstituteHp { hp: 1500 },
    }];
    let (p_hp0, e_hp0) = (state.player_battlers[0].hp, state.opponent_battlers[0].hp);
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Counter },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    assert_eq!(state.player_battlers[0].hp, p_hp0, "the doll protected the counter-user's real HP");
    assert!(
        state.opponent_battlers[0].hp < e_hp0,
        "Counter reflected the real (absorbed) damage, not 0"
    );
}

/// Gen-1 SLEEP goes THROUGH a Substitute (unlike poison/paralyze) — the oracle's
/// apply_sleep has no sub check, so the RON must not veto it.
#[test]
fn sleep_goes_through_a_substitute() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Hypnosis));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Gengar, 400, 200), MoveId::Hypnosis)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 2000, 50), MoveId::Splash)],
    );
    let mut effects = vec![EffectState {
        id: EffectId(0x9500), host: BattlerRef::OPPONENT, effect_order: 999,
        kind: PokeVolatile::SubstituteHp { hp: 500 },
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Hypnosis },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let (_r, log) = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    // A 1-turn sleep (all-zero rng) is ticked away when the (asleep) enemy acts, so
    // assert on the INFLICTION event: the sub did NOT veto it (it would abort the op).
    assert!(
        log.events.iter().any(|e| matches!(
            e,
            TurnEvent::StatusInflicted { target, status }
                if *target == BattlerRef::OPPONENT && matches!(status, LegacyStatus::Sleep(_))
        )),
        "sleep bypassed the sub and was inflicted; log={:?}",
        log.events
    );
    assert!(
        effects.iter().any(|e| e.host == BattlerRef::OPPONENT && matches!(e.kind, PokeVolatile::SubstituteHp { .. })),
        "the sub is still up (sleep does not break it)"
    );
}

/// Self-inflicted loss bypasses one's OWN doll: recoil damages the attacker's real HP
/// even while it holds a Substitute (the `who == source` exemption in redirect_hp_loss).
#[test]
fn recoil_bypasses_the_users_own_substitute() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::DoubleEdge));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 2000, 200), MoveId::DoubleEdge)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 2000, 50), MoveId::Splash)],
    );
    // The ATTACKER holds a doll; its recoil must still hit its real HP.
    let mut effects = vec![EffectState {
        id: EffectId(0x9400),
        host: BattlerRef::PLAYER,
        effect_order: 999,
        kind: PokeVolatile::SubstituteHp { hp: 1500 },
    }];
    let p_hp0 = state.player_battlers[0].hp;
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::DoubleEdge },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    assert!(state.player_battlers[0].hp < p_hp0, "recoil hit the attacker's REAL HP, not its own doll");
    let sub = effects.iter().find_map(|e| match &e.kind {
        PokeVolatile::SubstituteHp { hp } if e.host == BattlerRef::PLAYER => Some(*hp),
        _ => None,
    });
    assert_eq!(sub, Some(1500), "the user's own doll is untouched by its recoil");
}

// ── Secondary-effect battle text (residual HP-change cause tags) ──

/// A poisoned mon's residual tick narrates "is hurt by POISON!" — the driver tags the
/// residual Damaged with `Status(Poison)`, which translate_turn reads.
#[test]
fn residual_poison_narrates_hurt_by_poison() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Tackle));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 2000, 100), MoveId::Tackle)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 2000, 50), MoveId::Splash)],
    );
    state.player_battlers[0].status = Some(LegacyStatus::Poison);
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let (_r, log) = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
    let text = super::runtime::translate_turn(&log, &state, &effects);
    assert!(text.iter().any(|l| l.contains("is hurt by POISON!")), "poison residual narrated: {text:?}");
}

/// A burned mon's residual tick narrates "is hurt by its BURN!".
#[test]
fn residual_burn_narrates_hurt_by_burn() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Tackle));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 2000, 100), MoveId::Tackle)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 2000, 50), MoveId::Splash)],
    );
    state.player_battlers[0].status = Some(LegacyStatus::Burn);
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let (_r, log) = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
    let text = super::runtime::translate_turn(&log, &state, &effects);
    assert!(text.iter().any(|l| l.contains("is hurt by its BURN!")), "burn residual narrated: {text:?}");
}

/// A Leech Seed VOLATILE residual now narrates its own faithful line ("…sapped by LEECH
/// SEED!") via the per-volatile `HpChangeCause::Volatile(kind)` tag, and is NOT mislabeled
/// "hurt by POISON!". Here the seeder is at FULL HP so its heal clamps to 0 (no paired
/// Healed) — the sap on the seeded mon still narrates.
#[test]
fn leech_seed_residual_narrates_sapped() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Splash));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 2000, 100), MoveId::Splash)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 2000, 50), MoveId::Splash)],
    );
    // Player seeded; the seeder (opponent) is at FULL HP → its heal clamps to 0 (no
    // paired Healed). The sap still deals damage and reads as leech, never poison.
    let mut effects = vec![EffectState {
        id: EffectId(0x9700),
        host: BattlerRef::PLAYER,
        effect_order: 999,
        kind: PokeVolatile::LeechSeed,
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let (_r, log) = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
    let text = super::runtime::translate_turn(&log, &state, &effects);
    assert!(text.iter().any(|l| l.contains("LEECH SEED!")), "leech sap narrated: {text:?}");
    assert!(!text.iter().any(|l| l.contains("POISON")), "a leech sap is NOT mislabeled poison: {text:?}");
}

/// A badly-poisoned mon's Toxic ramp chips via the Toxic VOLATILE (the plain-Poison status
/// residual skips when Toxic is live — "one chip, not two"), and now narrates "is hurt by
/// POISON!" EXACTLY once (proving no status+volatile double-narration).
#[test]
fn toxic_residual_narrates_hurt_by_poison_once() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Tackle));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 2000, 100), MoveId::Tackle)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 2000, 50), MoveId::Splash)],
    );
    // Badly-poisoned = the Poison status PLUS a live Toxic volatile.
    state.player_battlers[0].status = Some(LegacyStatus::Poison);
    let mut effects = vec![EffectState {
        id: EffectId(0x9700),
        host: BattlerRef::PLAYER,
        effect_order: 999,
        kind: PokeVolatile::Toxic { counter: 1 },
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let (_r, log) = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
    let text = super::runtime::translate_turn(&log, &state, &effects);
    assert_eq!(
        text.iter().filter(|l| l.contains("is hurt by POISON!")).count(),
        1,
        "the Toxic ramp narrates one poison line (no status+volatile double): {text:?}"
    );
}

/// A mon carrying BOTH Toxic and Leech Seed gets BOTH residual lines, each correctly
/// attributed — precisely the case a positional/heuristic inference could not resolve
/// (the finer per-volatile cause makes it clean).
#[test]
fn toxic_and_leech_both_narrate_distinctly() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Tackle));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 2000, 100), MoveId::Tackle)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 2000, 50), MoveId::Splash)],
    );
    state.player_battlers[0].status = Some(LegacyStatus::Poison);
    // Toxic (lower id → fires first, matching legacy "toxic before leech") + Leech Seed.
    let mut effects = vec![
        EffectState {
            id: EffectId(0x9700),
            host: BattlerRef::PLAYER,
            effect_order: 998,
            kind: PokeVolatile::Toxic { counter: 1 },
        },
        EffectState {
            id: EffectId(0x9800),
            host: BattlerRef::PLAYER,
            effect_order: 999,
            kind: PokeVolatile::LeechSeed,
        },
    ];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let (_r, log) = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
    let text = super::runtime::translate_turn(&log, &state, &effects);
    assert!(text.iter().any(|l| l.contains("is hurt by POISON!")), "toxic line present: {text:?}");
    assert!(text.iter().any(|l| l.contains("LEECH SEED!")), "leech line present: {text:?}");
}

// ── Conversion (变身术) ──

/// Conversion copies the TARGET's types onto the user via a TypeOverride volatile
/// (matching legacy `apply_conversion`), leaving the user's species/stats untouched.
#[test]
fn conversion_copies_target_types() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Conversion));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    // Ditto (Normal) converts to match Gengar (Ghost/Poison).
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Ditto, 200, 100), MoveId::Conversion)],
        vec![engine_battler(&Mon::new(Species::Gengar, 200, 50), MoveId::Splash)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Conversion },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    let want = species_types(Species::Gengar);
    let got = effects.iter().find_map(|e| match &e.kind {
        PokeVolatile::TypeOverride { type1, type2 } if e.host == BattlerRef::PLAYER => Some((*type1, *type2)),
        _ => None,
    });
    assert_eq!(got, Some(want), "Conversion copies the target's types onto the user");
    assert_eq!(state.player_battlers[0].species, Species::Ditto, "only types change — species is untouched");
}

/// A TypeOverride (Conversion) is honoured by the damage formula: a move whose type
/// matches the overridden type gains STAB, dealing strictly more than without it.
#[test]
fn conversion_type_override_grants_stab() {
    install_canonical();
    let run = |with_override: bool| -> u16 {
        clear_current_moves();
        set_current_move(BattlerRef::PLAYER, real_move(MoveId::Surf));
        set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
        // Pikachu (Electric) Surfs a Normal-type Snorlax (neutral both runs); only the
        // Water TypeOverride (STAB) differs.
        let mut state = EngineState::new(
            vec![engine_battler(&Mon::new(Species::Pikachu, 400, 100), MoveId::Surf)],
            vec![engine_battler(&Mon::new(Species::Snorlax, 2000, 50), MoveId::Splash)],
        );
        let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
        if with_override {
            effects.push(EffectState {
                id: EffectId(0x9000),
                host: BattlerRef::PLAYER,
                effect_order: 999,
                kind: PokeVolatile::TypeOverride { type1: PokemonType::Water, type2: PokemonType::Water },
            });
        }
        let actions = [
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Surf },
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
        ];
        let mut rng = ScriptedRng::new(vec![0u8; 64]);
        let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
        2000 - state.opponent_battlers[0].hp
    };
    let no_stab = run(false);
    let stab = run(true);
    assert!(stab > no_stab, "the Water TypeOverride grants STAB: {stab} > {no_stab}");
}

// ── Disable (定身法) ──

/// Disable disables the TARGET's last-used move (its slot) for `(rng & 7) + 1` turns.
/// The Disable user is SLOWER so the target has already moved (its same-turn
/// decrement runs before the volatile exists) — isolating the install at full
/// duration. The faster-user same-turn decrement is covered by
/// [`disable_faster_user_target_decrements_same_turn`].
#[test]
fn disable_disables_targets_last_move() {
    install_canonical();
    clear_current_moves();
    clear_last_move_live();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Disable));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    // The opponent's PRIOR move (Tackle, its move slot 0) is disabled.
    set_last_move_live(BattlerRef::OPPONENT, MoveId::Tackle);
    let opp = {
        let mut stats = EnumMap::new();
        stats.set(StatIndex::Attack, 100);
        stats.set(StatIndex::Defense, 80);
        stats.set(StatIndex::Speed, 100); // faster → moves before the (slower) Disable
        stats.set(StatIndex::Special, 80);
        EngineBattler::<PokeredRules>::new(Species::Snorlax, 400, 400, stats, vec![MoveId::Tackle, MoveId::BodySlam])
    };
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Pikachu, 200, 50), MoveId::Disable)],
        vec![opp],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Disable },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    // All-zero bytes ⇒ accuracy passes and the duration byte is 0 ⇒ (0 & 7) + 1 = 1.
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    let got = effects.iter().find_map(|e| match &e.kind {
        PokeVolatile::Disable { slot, turns } if e.host == BattlerRef::OPPONENT => Some((*slot, *turns)),
        _ => None,
    });
    assert_eq!(got, Some((1, 1)), "Disable targets the last move (Tackle, slot 1) for 1 turn");
}

/// A disabled mon that still selects its disabled move loses the turn (the move deals
/// nothing), and the counter decrements even on that blocked turn.
#[test]
fn disable_veto_blocks_the_disabled_move() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Tackle));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let player = {
        let mut stats = EnumMap::new();
        stats.set(StatIndex::Attack, 100);
        stats.set(StatIndex::Defense, 80);
        stats.set(StatIndex::Speed, 100);
        stats.set(StatIndex::Special, 80);
        EngineBattler::<PokeredRules>::new(Species::Pikachu, 200, 200, stats, vec![MoveId::Tackle, MoveId::Thundershock])
    };
    let mut state = EngineState::new(
        vec![player],
        vec![engine_battler(&Mon::new(Species::Snorlax, 500, 50), MoveId::Splash)],
    );
    // Slot 1 (Tackle) disabled with 3 turns left.
    let mut effects = vec![EffectState {
        id: EffectId(0x8000),
        host: BattlerRef::PLAYER,
        effect_order: 999,
        kind: PokeVolatile::Disable { slot: 1, turns: 3 },
    }];
    let e_hp0 = state.opponent_battlers[0].hp;
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    assert_eq!(state.opponent_battlers[0].hp, e_hp0, "the disabled Tackle is blocked → no damage");
    let turns = effects.iter().find_map(|e| match &e.kind {
        PokeVolatile::Disable { turns, .. } if e.host == BattlerRef::PLAYER => Some(*turns),
        _ => None,
    });
    assert_eq!(turns, Some(2), "the counter ticks (3→2) even on the blocked turn");
}

/// The disable counter decrements each turn the mon acts (with a non-disabled move)
/// and the volatile is removed when it hits zero (the move becomes usable again).
#[test]
fn disable_counter_decrements_and_expires() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Thundershock)); // NOT the disabled move
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let player = {
        let mut stats = EnumMap::new();
        stats.set(StatIndex::Attack, 100);
        stats.set(StatIndex::Defense, 80);
        stats.set(StatIndex::Speed, 100);
        stats.set(StatIndex::Special, 80);
        EngineBattler::<PokeredRules>::new(Species::Pikachu, 200, 200, stats, vec![MoveId::Tackle, MoveId::Thundershock])
    };
    let mut state = EngineState::new(
        vec![player],
        vec![engine_battler(&Mon::new(Species::Snorlax, 500, 50), MoveId::Splash)],
    );
    // Slot 1 (Tackle) disabled with only 1 turn left → this turn it expires.
    let mut effects = vec![EffectState {
        id: EffectId(0x8000),
        host: BattlerRef::PLAYER,
        effect_order: 999,
        kind: PokeVolatile::Disable { slot: 1, turns: 1 },
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Thundershock },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    assert!(
        !effects.iter().any(|e| e.host == BattlerRef::PLAYER && matches!(e.kind, PokeVolatile::Disable { .. })),
        "the disable expired (counter hit 0) and the volatile was removed"
    );
}

/// The DEFENDER-side TypeOverride (Conversion) is honoured by the type chart: a mon
/// converted to Ghost is IMMUNE to a Normal move (Gen-1 Ghost↔Normal 0×), where the
/// same move without the override deals damage. Exercises effective_types on the
/// DEFENDER path (the offensive/STAB path is covered by conversion_type_override_grants_stab).
#[test]
fn conversion_defensive_override_confers_immunity() {
    install_canonical();
    let run = |with_override: bool| -> u16 {
        clear_current_moves();
        set_current_move(BattlerRef::PLAYER, real_move(MoveId::Tackle));
        set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
        let mut state = EngineState::new(
            vec![engine_battler(&Mon::new(Species::Tauros, 400, 100), MoveId::Tackle)],
            vec![engine_battler(&Mon::new(Species::Snorlax, 2000, 50), MoveId::Splash)],
        );
        let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
        if with_override {
            effects.push(EffectState {
                id: EffectId(0x9100),
                host: BattlerRef::OPPONENT,
                effect_order: 999,
                kind: PokeVolatile::TypeOverride { type1: PokemonType::Ghost, type2: PokemonType::Ghost },
            });
        }
        let actions = [
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
            BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
        ];
        let mut rng = ScriptedRng::new(vec![0u8; 64]);
        let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
        2000 - state.opponent_battlers[0].hp
    };
    assert!(run(false) > 0, "without the override the Normal Tackle damages the defender");
    assert_eq!(run(true), 0, "a Ghost TypeOverride on the defender makes it immune to Normal");
}

/// The PRODUCTION narration effectiveness line honours a defender's Conversion
/// TypeOverride (keeping the "super effective!" text in agreement with the
/// override-aware damage): Surf (Water) on a mon overridden to Ground/Rock announces
/// super-effective. Uses the real `runtime::translate_turn` (not the test-side copy).
#[test]
fn conversion_narration_honours_defender_override() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Surf));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Blastoise, 400, 100), MoveId::Surf)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 4000, 50), MoveId::Splash)],
    );
    // Snorlax overridden to Ground/Rock ⇒ Water is 4× super-effective.
    let mut effects = vec![EffectState {
        id: EffectId(0x9200),
        host: BattlerRef::OPPONENT,
        effect_order: 999,
        kind: PokeVolatile::TypeOverride { type1: PokemonType::Ground, type2: PokemonType::Rock },
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Surf },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let (_r, log) = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    let with_override = super::runtime::translate_turn(&log, &state, &effects);
    assert!(
        with_override.iter().any(|l| l.contains("super effective")),
        "narration honours the defender override: {with_override:?}"
    );
    // Without the arena (the old behaviour) the text would fall back to species types
    // (Normal) and omit the line — proving the fix depends on the override.
    let without = super::runtime::translate_turn(&log, &state, &[]);
    assert!(
        !without.iter().any(|l| l.contains("super effective")),
        "species-based fallback omits the line: {without:?}"
    );
}

/// A FASTER Disable user's target decrements its FRESH counter the SAME turn (the
/// target moves after the Disable, so its decrement gate ticks). With turns=1 the
/// same-turn tick removes the disable immediately — the Gen-1 behaviour the install
/// test deliberately sidesteps by making the user slower.
#[test]
fn disable_faster_user_target_decrements_same_turn() {
    install_canonical();
    clear_current_moves();
    clear_last_move_live();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Disable));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Tackle)); // NOT the disabled move
    set_last_move_live(BattlerRef::OPPONENT, MoveId::BodySlam); // the move to disable
    let opp = {
        let mut stats = EnumMap::new();
        stats.set(StatIndex::Attack, 100);
        stats.set(StatIndex::Defense, 80);
        stats.set(StatIndex::Speed, 50); // slower → moves AFTER the faster Disable
        stats.set(StatIndex::Special, 80);
        EngineBattler::<PokeredRules>::new(Species::Snorlax, 400, 400, stats, vec![MoveId::Tackle, MoveId::BodySlam])
    };
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Pikachu, 200, 100), MoveId::Disable)], // faster
        vec![opp],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Disable },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);

    assert!(
        !effects.iter().any(|e| e.host == BattlerRef::OPPONENT && matches!(e.kind, PokeVolatile::Disable { .. })),
        "the faster Disable user's target ticked its fresh counter (1→0) the same turn"
    );
}

// ── Struggle recoil + Dream Eater ──

/// Struggle deals normal damage then recoils HALF the damage dealt.
#[test]
fn struggle_recoils_half_the_damage_dealt() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Struggle));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 1000, 200), MoveId::Struggle)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 1000, 50), MoveId::Splash)],
    );
    let (p_hp0, e_hp0) = (state.player_battlers[0].hp, state.opponent_battlers[0].hp);
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Struggle },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    let dealt = e_hp0 - state.opponent_battlers[0].hp;
    let recoil = p_hp0 - state.player_battlers[0].hp;
    assert!(dealt > 0, "Struggle dealt damage");
    assert_eq!(recoil, dealt / 2, "Struggle recoils half the damage dealt");
}

/// Dream Eater drains (heals the user) when the target is ASLEEP.
#[test]
fn dream_eater_drains_from_sleeping_target() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::DreamEater));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Gengar, 1000, 200), MoveId::DreamEater)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 1000, 50), MoveId::Splash)],
    );
    state.player_battlers[0].hp = 400; // hurt, so the drain heal is observable
    state.opponent_battlers[0].status = Some(LegacyStatus::Sleep(3));
    let p_hp0 = state.player_battlers[0].hp;
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::DreamEater },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert!(state.opponent_battlers[0].hp < 1000, "Dream Eater dealt damage");
    assert!(state.player_battlers[0].hp > p_hp0, "Dream Eater drained (healed the user)");
}

/// Dream Eater FAILS ENTIRELY (a miss — no damage, no drain) when the target is
/// awake: core.asm:5240-5245, MoveHitTest's dreamEaterCheck → .moveMissed before
/// any damage/accuracy roll.
#[test]
fn dream_eater_fails_on_awake_target() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::DreamEater));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Gengar, 1000, 200), MoveId::DreamEater)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 1000, 50), MoveId::Splash)],
    );
    state.player_battlers[0].hp = 400;
    let p_hp0 = state.player_battlers[0].hp;
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::DreamEater },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let (_r, log) = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(state.opponent_battlers[0].hp, 1000, "Dream Eater deals NO damage to an awake target");
    assert_eq!(state.player_battlers[0].hp, p_hp0, "no drain when the target is awake");
    assert!(
        log.events.iter().any(|e| matches!(e, TurnEvent::Missed { actor } if *actor == BattlerRef::PLAYER)),
        "the failure is a miss (MoveHitTest .moveMissed)"
    );
}

/// CheckDefrost (effects.asm:312-330): a Fire-type burn-family move (Ember)
/// DEFROSTS a frozen target it hits — and can NOT then burn it (the original
/// skips the side-effect roll entirely on an already-statused target).
#[test]
fn fire_move_defrosts_frozen_target() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Ember));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Arcanine, 1000, 200), MoveId::Ember)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 1000, 50), MoveId::Splash)],
    );
    state.opponent_battlers[0].status = Some(LegacyStatus::Freeze);
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Ember },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    // All-zero stream: Ember's 26/256 burn roll WOULD pass — but the frozen
    // (already-statused) target can't be burned; it is defrosted instead.
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let (_r, log) = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(
        state.opponent_battlers[0].status, None,
        "frozen target defrosted by a Fire-type move (and NOT burned)"
    );
    assert!(
        log.events.iter().any(|e| matches!(e, TurnEvent::StatusCured { target, status }
            if *target == BattlerRef::OPPONENT && matches!(status, LegacyStatus::Freeze))),
        "the defrost surfaces as a Freeze cure"
    );
}

/// A NON-Fire move does NOT defrost (CheckDefrost checks the move's type).
#[test]
fn non_fire_move_does_not_defrost() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Tackle));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Tauros, 1000, 200), MoveId::Tackle)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 1000, 50), MoveId::Splash)],
    );
    state.opponent_battlers[0].status = Some(LegacyStatus::Freeze);
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(
        state.opponent_battlers[0].status,
        Some(LegacyStatus::Freeze),
        "Tackle leaves the frozen target frozen"
    );
}

/// A missed Jump Kick crashes the user for 1 HP (Gen-1's 1-HP crash).
#[test]
fn jump_kick_crashes_the_user_on_miss() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::JumpKick));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Hitmonlee, 1000, 200), MoveId::JumpKick)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 1000, 50), MoveId::Splash)],
    );
    let p_hp0 = state.player_battlers[0].hp;
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::JumpKick },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    // 0xFF stream → the accuracy roll misses.
    let mut rng = ScriptedRng::new(vec![0xFFu8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(state.player_battlers[0].hp, p_hp0 - 1, "a missed Jump Kick crashes for 1 HP");
    assert_eq!(state.opponent_battlers[0].hp, 1000, "the missed Jump Kick dealt no damage");
}

/// Haze resets both sides' stat stages + clears confusion, but PRESERVES the screens
/// (Reflect) — the selective-reset guarantee.
#[test]
fn haze_resets_stages_but_preserves_reflect() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Haze));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Gengar, 1000, 200), MoveId::Haze)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 1000, 50), MoveId::Splash)],
    );
    state.opponent_battlers[0].stat_stages.set(StatIndex::Attack, 4);
    let mut effects: Vec<EffectState<PokeredRules>> = vec![
        EffectState { id: EffectId(1), host: BattlerRef::OPPONENT, effect_order: 0, kind: PokeVolatile::Confused { turns: 3 } },
        EffectState { id: EffectId(2), host: BattlerRef::OPPONENT, effect_order: 1, kind: PokeVolatile::Reflect },
    ];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Haze },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let _ = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(
        state.opponent_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
        0,
        "Haze reset the foe's stat stages"
    );
    assert!(
        !effects.iter().any(|e| matches!(e.kind, PokeVolatile::Confused { .. })),
        "Haze cleared confusion"
    );
    assert!(
        effects.iter().any(|e| matches!(e.kind, PokeVolatile::Reflect)),
        "Haze PRESERVED Reflect (selective reset)"
    );
}

/// Net HP lost by `who` per the log (Damaged − Healed), as the legacy oracle's
/// `initial − final` should match.
fn log_net_damage(log: &TurnLog<PokeredRules>, who: BattlerRef) -> i32 {
    log.events.iter().fold(0i32, |acc, e| match e {
        TurnEvent::Damaged { target, amount, .. } if *target == who => acc + *amount as i32,
        TurnEvent::Healed { target, amount, .. } if *target == who => acc - *amount as i32,
        _ => acc,
    })
}

fn log_has_faint(log: &TurnLog<PokeredRules>, who: BattlerRef) -> bool {
    log.events.iter().any(|e| matches!(e, TurnEvent::Fainted { who: w } if *w == who))
}

fn log_has_status(log: &TurnLog<PokeredRules>, who: BattlerRef) -> bool {
    log.events.iter().any(|e| matches!(e, TurnEvent::StatusInflicted { target, .. } if *target == who))
}

fn log_has_crit(log: &TurnLog<PokeredRules>, who: BattlerRef) -> bool {
    log.events.iter().any(|e| matches!(e, TurnEvent::Crit { actor } if *actor == who))
}

fn log_stat_delta(log: &TurnLog<PokeredRules>, who: BattlerRef, st: StatIndex) -> i32 {
    log.events.iter().fold(0i32, |acc, e| match e {
        TurnEvent::StatChanged { target, stat, delta } if *target == who && *stat == st => acc + *delta as i32,
        _ => acc,
    })
}

fn log_move_used_count(log: &TurnLog<PokeredRules>) -> usize {
    log.events.iter().filter(|e| matches!(e, TurnEvent::MoveUsed { .. })).count()
}

/// The shadow contract: the logged stack turn matches the plain stack turn AND the
/// legacy oracle in final state, and the TurnLog narrates the same outcome.
fn shadow_validate(s: &Scenario) {
    let legacy = legacy_run(s);
    let (stack, log) = stack_run_logged(s);
    let (plain, _, _) = stack_run(s);

    let lp = legacy.player.active_mon();
    let le = legacy.enemy.active_mon();
    let sp = &stack.player_battlers[0];
    let se = &stack.opponent_battlers[0];

    // (a) logging is NON-PERTURBING on the real PokeredRules path.
    assert_eq!(sp.hp, plain.player_battlers[0].hp, "[{}] logged==plain PLAYER hp", s.name);
    assert_eq!(se.hp, plain.opponent_battlers[0].hp, "[{}] logged==plain ENEMY hp", s.name);

    // (b) parity with the production-representative legacy oracle.
    assert_eq!(lp.hp, sp.hp, "[{}] PLAYER hp parity", s.name);
    assert_eq!(le.hp, se.hp, "[{}] ENEMY hp parity", s.name);

    // (c) the TurnLog narrates the legacy turn.
    // net hp ↔ legacy initial−final.
    assert_eq!(log_net_damage(&log, BattlerRef::PLAYER), s.player.hp as i32 - lp.hp as i32, "[{}] log PLAYER net dmg", s.name);
    assert_eq!(log_net_damage(&log, BattlerRef::OPPONENT), s.enemy.hp as i32 - le.hp as i32, "[{}] log ENEMY net dmg", s.name);
    // faint ↔ legacy hp reached 0.
    assert_eq!(log_has_faint(&log, BattlerRef::PLAYER), lp.hp == 0 && s.player.hp > 0, "[{}] PLAYER faint event", s.name);
    assert_eq!(log_has_faint(&log, BattlerRef::OPPONENT), le.hp == 0 && s.enemy.hp > 0, "[{}] ENEMY faint event", s.name);
    // status ↔ legacy gained a non-volatile status.
    assert_eq!(log_has_status(&log, BattlerRef::PLAYER), lp.status != LegacyStatus::None, "[{}] PLAYER status event", s.name);
    assert_eq!(log_has_status(&log, BattlerRef::OPPONENT), le.status != LegacyStatus::None, "[{}] ENEMY status event", s.name);
    // stat-stage ↔ legacy stage delta (initial stages are 0).
    for st in [StatIndex::Attack, StatIndex::Defense, StatIndex::Speed, StatIndex::Special, StatIndex::Accuracy, StatIndex::Evasion] {
        assert_eq!(log_stat_delta(&log, BattlerRef::PLAYER, st), legacy_stage(&legacy.player.stat_stages, st) as i32, "[{}] PLAYER stage {:?}", s.name, st);
        assert_eq!(log_stat_delta(&log, BattlerRef::OPPONENT, st), legacy_stage(&legacy.enemy.stat_stages, st) as i32, "[{}] ENEMY stage {:?}", s.name, st);
    }
    // move-used: both movers act unless a faint cut the turn short.
    if lp.hp > 0 && le.hp > 0 {
        assert_eq!(log_move_used_count(&log), 2, "[{}] both movers logged MoveUsed", s.name);
    } else {
        assert!(log_move_used_count(&log) >= 1, "[{}] at least one MoveUsed", s.name);
    }
}

/// The shadow proof over a representative set of REAL Gen-1 turns spanning the
/// tiers the translator must render: pure damage, super-effective, crit, self-buff,
/// secondary status, and a KO (faint + second-move cancel).
#[test]
fn p6_shadow_turnlog_narrates_real_gen1_turns() {
    // 1. Pure neutral damage (Tackle: Normal vs Electric Pikachu).
    shadow_validate(&Scenario::base("shadow:pure", MoveId::Tackle));

    // 2. Super-effective (Earthquake: Ground 2× vs Electric).
    shadow_validate(&Scenario::base("shadow:super", MoveId::Earthquake));

    // 3. Forced crit (Tackle, crit byte 0 → crit) → a Crit event for the first mover.
    let mut crit = Scenario::base("shadow:crit", MoveId::Tackle);
    crit.first.crit = 0;
    shadow_validate(&crit);
    let (_st, log) = stack_run_logged(&crit);
    // base scenario: player (speed 100) is the first mover, so the crit byte (on
    // `s.first`) lands on the PLAYER.
    assert_eq!(first_mover(&crit), FirstMover::Player, "[shadow:crit] player is first mover");
    assert!(log_has_crit(&log, BattlerRef::PLAYER), "[shadow:crit] first mover logged a Crit");

    // 4. Self-buff (Swords Dance: +2 Attack on self, power 0 → no damage).
    shadow_validate(&Scenario::base("shadow:buff", MoveId::SwordsDance));

    // 5. Secondary status — Body Slam paralysis (Normal-type, so no
    //    MoveTypeIsDefenderType veto; side_effect byte 0 ⇒ paralysis fires on both).
    //    Paralysis carries NO end-of-turn residual, so it stays fully at parity
    //    (unlike burn/poison, whose 1/16 chip the stack DEFERS — see the gap note on
    //    `shadow_status_residual_gap_is_documented`). Validates the StatusInflicted
    //    narration on a real Gen-1 secondary.
    let mut par = Scenario::base("shadow:paralyze", MoveId::BodySlam);
    par.first.side_effect = 0;
    par.second.side_effect = 0;
    shadow_validate(&par);

    // 6. KO (Tackle into a 10-HP foe → faint + second move cancelled).
    let mut ko = Scenario::base("shadow:ko", MoveId::Tackle);
    ko.enemy = Mon::new(Species::Pikachu, 10, 50);
    shadow_validate(&ko);
}

/// P6b-prereq (GAP NOW CLOSED): non-volatile status residual is on the stack.
/// `PokeredRules::effect_for_status` returns flat burn/poison residual effects, so a
/// burned/poisoned mon takes its `(max/16).max(1)` chip through the driver's residual
/// aggregation — exactly as the legacy `apply_all_residual` does. This test was the
/// gap-pin (the shadow proof's max/16 divergence on a burn-firing Ember turn); it now
/// asserts FULL PARITY (the divergence is zero). A burned mon (second mover) chips
/// this turn; the first mover, burned after its own residual step, chips next turn.
#[test]
fn burn_residual_now_at_parity_on_stack() {
    let mut burn = Scenario::base("burn-residual-parity", MoveId::Ember);
    burn.first.side_effect = 0;
    burn.second.side_effect = 0;
    let legacy = legacy_run(&burn);
    let (stack, log) = stack_run_logged(&burn);
    // The burn is inflicted on both paths and narrated by the log.
    assert_eq!(stack.opponent_battlers[0].status, Some(LegacyStatus::Burn), "stack inflicted burn");
    assert!(log_has_status(&log, BattlerRef::OPPONENT), "log narrates the burn infliction");
    // FULL parity now: the burned enemy's residual chip lands on the stack too.
    assert_eq!(
        stack.opponent_battlers[0].hp,
        legacy.enemy.active_mon().hp,
        "burned ENEMY hp now matches legacy (status residual on the stack)"
    );
    assert_eq!(
        stack.player_battlers[0].hp,
        legacy.player.active_mon().hp,
        "PLAYER hp matches legacy (burned after its residual step → no chip this turn)"
    );
    // The log narrates the chip as a Damaged event on the enemy (move hit + burn).
    let chip = (burn.enemy.hp / 16).max(1);
    assert_eq!(
        log_net_damage(&log, BattlerRef::OPPONENT),
        burn.enemy.hp as i32 - stack.opponent_battlers[0].hp as i32,
        "log net damage == enemy hp lost (move + {chip} burn chip)"
    );
}

/// P6b-prereq: a badly-poisoned (Toxic) mon ticks the Toxic VOLATILE ramp through
/// the DRIVER's residual aggregation (effect_for_volatile), while the flat poison
/// status residual (effect_for_status(Poison)) SKIPS — so exactly ONE chip lands
/// (the ramp), not two. Pins the dedup that keeps a Toxic'd mon from double-chipping
/// when both the status byte and the volatile are live.
#[test]
fn toxic_ramp_through_driver_no_double_chip() {
    install_canonical();
    set_active_move(real_move(MoveId::Splash));
    // Player is badly poisoned: status = Poison AND a live Toxic volatile (counter 0).
    let mut pb = engine_battler(&Mon::new(Species::Snorlax, 320, 100), MoveId::Splash);
    pb.status = Some(LegacyStatus::Poison);
    let eb = engine_battler(&Mon::new(Species::Snorlax, 320, 50), MoveId::Splash);
    let mut state = EngineState::new(vec![pb], vec![eb]);
    let mut effects: Vec<EffectState<PokeredRules>> = vec![EffectState {
        id: EffectId(900),
        host: BattlerRef::PLAYER,
        effect_order: 0,
        kind: PokeVolatile::Toxic { counter: 0 },
    }];
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0, 0, 0, 0]);
    StackDriver::execute_turn(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
    // Ramp tick 1: counter 0→1, chip = (320/16)*1 = 20. NOT 40 (flat 20 + ramp 20).
    assert_eq!(
        state.player_battlers[0].hp, 300,
        "Toxic ramps 20 via the volatile; the flat poison status residual SKIPS (no double-chip)"
    );
    assert!(
        effects.iter().any(|e| e.host == BattlerRef::PLAYER
            && matches!(e.kind, PokeVolatile::Toxic { counter: 1 })),
        "the Toxic counter ramped 0 → 1 through the driver's residual aggregation"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// P6b-prereq stage 2 — the BeforeMove GATES fire on the REAL PokeredRules provider.
// Setup: the gated mover (player, faster) uses Tackle; the enemy does Nothing. So
// the enemy is unharmed IFF the gate BLOCKED the player's Tackle, and the player
// loses hp IFF a confusion self-hit fired. Drives StackDriver::execute_turn with
// explicit bytes. (The gate LOGIC is legacy-parity-proven on the POC; these pin
// that it is wired onto every PokeredRules move and fires.)
// ─────────────────────────────────────────────────────────────────────────────

fn gate_state(player_status: Option<LegacyStatus>) -> (EngineState<PokeredRules>, Vec<EffectState<PokeredRules>>) {
    install_canonical();
    set_active_move(real_move(MoveId::Tackle));
    let mut pb = engine_battler(&Mon::new(Species::Snorlax, 300, 100), MoveId::Tackle);
    pb.status = player_status;
    let eb = engine_battler(&Mon::new(Species::Snorlax, 300, 50), MoveId::Tackle);
    (EngineState::new(vec![pb], vec![eb]), Vec::new())
}

fn gate_actions() -> [BattleAction<PokeredRules>; 2] {
    [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
        BattleAction::<PokeredRules>::Nothing,
    ]
}

/// Sleep gate (#8): an asleep mon can't act and STILL loses the turn on the wake
/// tick. Draws no rng; counter decrements 2 → 1 → 0(awake).
#[test]
fn gate_sleep_blocks_and_wake_costs_a_turn() {
    let (mut state, mut effects) = gate_state(Some(LegacyStatus::Sleep(2)));
    let mut rng = ScriptedRng::new(vec![]);
    StackDriver::execute_turn(&PokeredRules, &mut state, &mut effects, gate_actions(), &mut rng);
    assert_eq!(state.opponent_battlers[0].hp, 300, "asleep → Tackle blocked → enemy unharmed");
    assert_eq!(state.player_battlers[0].status, Some(LegacyStatus::Sleep(1)), "counter 2 → 1");
    assert_eq!(rng.consumed(), 0, "sleep gate draws no rng");
    let mut rng2 = ScriptedRng::new(vec![]);
    StackDriver::execute_turn(&PokeredRules, &mut state, &mut effects, gate_actions(), &mut rng2);
    assert_eq!(state.opponent_battlers[0].hp, 300, "wake tick STILL loses the turn (#8)");
    assert_eq!(state.player_battlers[0].status, None, "woke up (counter 1 → 0)");
}

/// Freeze gate (#10): a frozen mon ALWAYS can't act; no thaw, no rng.
#[test]
fn gate_freeze_always_blocks() {
    let (mut state, mut effects) = gate_state(Some(LegacyStatus::Freeze));
    let mut rng = ScriptedRng::new(vec![]);
    StackDriver::execute_turn(&PokeredRules, &mut state, &mut effects, gate_actions(), &mut rng);
    assert_eq!(state.opponent_battlers[0].hp, 300, "frozen → Tackle blocked → enemy unharmed");
    assert_eq!(state.player_battlers[0].status, Some(LegacyStatus::Freeze), "still frozen (no thaw)");
    assert_eq!(rng.consumed(), 0, "freeze gate draws no rng");
}

/// Paralysis gate: 25% full-para (`byte < 63`). `< 63` blocks (draws 1 byte); `>= 63`
/// the mon acts (draws the para byte then crit/acc/damage).
#[test]
fn gate_paralysis_full_para_blocks_else_acts() {
    // byte 0 < 63 → fully paralyzed → blocked.
    let (mut s1, mut e1) = gate_state(Some(LegacyStatus::Paralysis));
    let mut r1 = ScriptedRng::new(vec![0]);
    StackDriver::execute_turn(&PokeredRules, &mut s1, &mut e1, gate_actions(), &mut r1);
    assert_eq!(s1.opponent_battlers[0].hp, 300, "para byte 0 < 63 → fully paralyzed → enemy unharmed");
    assert_eq!(r1.consumed(), 1, "only the paralysis gate byte is drawn on a full-para");
    // byte 63 !< 63 → acts → Tackle lands.
    let (mut s2, mut e2) = gate_state(Some(LegacyStatus::Paralysis));
    let mut r2 = ScriptedRng::new(vec![63, 255, 0, 255]); // para(63 act), crit(255), acc(0 hit), dmg(255)
    StackDriver::execute_turn(&PokeredRules, &mut s2, &mut e2, gate_actions(), &mut r2);
    assert!(s2.opponent_battlers[0].hp < 300, "para byte 63 ≥ 63 → acts → Tackle hits");
}

/// Confusion gate: a confused mon draws one byte — `< 128` ⇒ 40-power typeless
/// self-hit (move aborts), `>= 128` ⇒ acts.
#[test]
fn gate_confusion_self_hit_else_acts() {
    // byte 0 < 128 → self-hit: player loses hp, Tackle aborts (enemy unharmed).
    let (mut s1, mut e1) = gate_state(None);
    e1.push(EffectState { id: EffectId(800), host: BattlerRef::PLAYER, effect_order: 0, kind: PokeVolatile::Confused { turns: 3 } });
    let mut r1 = ScriptedRng::new(vec![0]);
    StackDriver::execute_turn(&PokeredRules, &mut s1, &mut e1, gate_actions(), &mut r1);
    assert_eq!(s1.opponent_battlers[0].hp, 300, "confusion self-hit → Tackle aborts → enemy unharmed");
    assert!(s1.player_battlers[0].hp < 300, "confusion self-hit damages the confused mon");
    assert_eq!(r1.consumed(), 1, "only the confusion gate byte is drawn on a self-hit");
    // byte 255 ≥ 128 → acts → Tackle lands.
    let (mut s2, mut e2) = gate_state(None);
    e2.push(EffectState { id: EffectId(801), host: BattlerRef::PLAYER, effect_order: 0, kind: PokeVolatile::Confused { turns: 3 } });
    let mut r2 = ScriptedRng::new(vec![255, 255, 0, 255]); // confusion(255 act), crit, acc, dmg
    StackDriver::execute_turn(&PokeredRules, &mut s2, &mut e2, gate_actions(), &mut r2);
    assert!(s2.opponent_battlers[0].hp < 300, "confusion byte 255 ≥ 128 → acts → Tackle hits");
    assert_eq!(s2.player_battlers[0].hp, 300, "no self-hit when the mon acts through confusion");
}

/// P6b: a BLOCKED move (here an asleep mover) logs `TurnEvent::Blocked{actor}` and
/// NO `MoveUsed` — the event a frontend translator needs to narrate "X is fast
/// asleep!" (the game derives the reason from the mover's status). Pins the engine's
/// additive Blocked emission end-to-end through the real PokeredRules gates.
#[test]
fn blocked_move_logs_blocked_event_via_driver() {
    let (mut state, mut effects) = gate_state(Some(LegacyStatus::Sleep(2)));
    let mut rng = ScriptedRng::new(vec![]);
    let (_r, log) =
        StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, gate_actions(), &mut rng);
    let p = BattlerRef::PLAYER;
    assert!(
        log.events.iter().any(|e| matches!(e, TurnEvent::Blocked { actor } if *actor == p)),
        "the asleep mover logs a Blocked event"
    );
    assert!(
        !log.events.iter().any(|e| matches!(e, TurnEvent::MoveUsed { actor, .. } if *actor == p)),
        "no MoveUsed is logged for a blocked move"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// P6b TRANSLATOR (slice 1) — TurnEvent → pokered battle text. The production loop
// will eventually call this instead of the scattered text sites; here it is proven
// (test-side) to reproduce `format_move_outcome`'s move-announcement block exactly.
// The TurnLog carries the structural facts (MoveUsed / Crit / Missed / Blocked);
// effectiveness is RE-DERIVED game-side (the engine reports only structural damage).
// ─────────────────────────────────────────────────────────────────────────────

/// The combined type-effectiveness category of `move_type` vs a (possibly dual-type)
/// defender, via the type chart — the game-side re-derivation the TurnLog omits.
/// Uses the integer discriminants (×10 units) so the dual-type product is exact and
/// float-free. A single-type mon has `def2 == def1` (counts once).
fn effectiveness_category(move_type: PokemonType, def1: PokemonType, def2: PokemonType) -> Effectiveness {
    use pokered_data::type_chart::get_effectiveness;
    let e1 = get_effectiveness(move_type, def1) as u32; // {0,5,10,20}
    let e2 = if def2 == def1 { 10 } else { get_effectiveness(move_type, def2) as u32 };
    let p = e1 * e2 / 10; // back to ×10 units: 10 == neutral
    match p {
        0 => Effectiveness::NoEffect,
        10 => Effectiveness::Normal,
        x if x > 10 => Effectiveness::SuperEffective,
        _ => Effectiveness::NotVeryEffective,
    }
}

/// The move-announcement text block for ONE mover, matching the production
/// `format_move_outcome` (battle/mod.rs:863). `blocked_reason` (Some when the mover
/// hit a `Blocked` gate) maps the mon's status via the same wording as
/// `format_cannot_move`; it CLEARS the "used" line (legacy parity).
fn move_announcement(
    side_name: &str,
    move_name: &str,
    crit: bool,
    missed: bool,
    eff: Effectiveness,
    blocked_reason: Option<&str>,
) -> Vec<String> {
    if let Some(reason) = blocked_reason {
        return vec![format!("{} {}", side_name, reason)];
    }
    let mut msgs = vec![format!("{} used {}!", side_name, move_name)];
    if missed {
        msgs.push(format!("{}'s attack missed!", side_name));
        return msgs;
    }
    if crit {
        msgs.push("Critical hit!".to_string());
    }
    match eff {
        Effectiveness::SuperEffective => msgs.push("It's super effective!".to_string()),
        Effectiveness::NotVeryEffective => msgs.push("It's not very effective...".to_string()),
        Effectiveness::NoEffect => msgs.push("It doesn't affect the enemy!".to_string()),
        Effectiveness::Normal => {}
    }
    msgs
}

/// The effectiveness derivation matches the chart for representative match-ups.
#[test]
fn translator_effectiveness_category_matches_chart() {
    use pokered_data::types::PokemonType as T;
    assert_eq!(effectiveness_category(T::Ground, T::Electric, T::Electric), Effectiveness::SuperEffective, "Ground 2× Electric");
    assert_eq!(effectiveness_category(T::Electric, T::Electric, T::Electric), Effectiveness::NotVeryEffective, "Electric 0.5× Electric");
    assert_eq!(effectiveness_category(T::Normal, T::Electric, T::Electric), Effectiveness::Normal, "Normal 1× Electric");
    assert_eq!(effectiveness_category(T::Ground, T::Electric, T::Flying), Effectiveness::NoEffect, "Ground 0× a Flying part (Electric/Flying)");
    assert_eq!(effectiveness_category(T::Water, T::Fire, T::Flying), Effectiveness::SuperEffective, "Water 2×Fire ×1Flying = super");
}

/// The announcement block reproduces `format_move_outcome`'s exact lines for each
/// outcome shape.
#[test]
fn translator_move_announcement_matches_production_wording() {
    // Neutral hit.
    assert_eq!(
        move_announcement("Pikachu", "Tackle", false, false, Effectiveness::Normal, None),
        vec!["Pikachu used Tackle!"]
    );
    // Crit + super-effective.
    assert_eq!(
        move_announcement("Gyarados", "Surf", true, false, Effectiveness::SuperEffective, None),
        vec!["Gyarados used Surf!", "Critical hit!", "It's super effective!"]
    );
    // Not very effective.
    assert_eq!(
        move_announcement("Pikachu", "Thunderbolt", false, false, Effectiveness::NotVeryEffective, None),
        vec!["Pikachu used Thunderbolt!", "It's not very effective..."]
    );
    // Miss ("used" then "missed").
    assert_eq!(
        move_announcement("Pikachu", "Tackle", false, true, Effectiveness::Normal, None),
        vec!["Pikachu used Tackle!", "Pikachu's attack missed!"]
    );
    // Blocked (asleep) — the "used" line is cleared; only the reason shows.
    assert_eq!(
        move_announcement("Snorlax", "Tackle", false, false, Effectiveness::Normal, Some("is fast asleep!")),
        vec!["Snorlax is fast asleep!"]
    );
}

/// End-to-end: a REAL TurnLog (super-effective Earthquake) → extract the first
/// mover's MoveUsed → re-derive effectiveness → produce the announcement. Proves the
/// translator consumes the engine log and reaches the production wording.
#[test]
fn translator_from_real_turnlog_super_effective() {
    use pokered_data::lang_data::{move_name, species_name};
    let s = Scenario::base("translate:earthquake", MoveId::Earthquake); // Ground vs Electric Pikachu
    let (_state, log) = stack_run_logged(&s);
    // The first event is the (faster) player's MoveUsed{Earthquake}.
    let (actor, mv) = log
        .events
        .iter()
        .find_map(|e| match e {
            TurnEvent::MoveUsed { actor, move_ } => Some((*actor, *move_)),
            _ => None,
        })
        .expect("a MoveUsed was logged");
    assert_eq!(actor, BattlerRef::PLAYER, "player is the first mover");
    let crit = log.events.iter().any(|e| matches!(e, TurnEvent::Crit { actor: a } if *a == actor));
    // Re-derive effectiveness from the move's type vs the DEFENDER's (opponent) types.
    let md = real_move(mv);
    let (d1, d2) = pokered_data::pokemon_data::get_base_stats(s.enemy.species)
        .map(|b| (b.type1, b.type2))
        .unwrap();
    let eff = effectiveness_category(md.move_type, d1, d2);
    let text = move_announcement(species_name(s.player.species, false), move_name(mv, false), crit, false, eff, None);
    assert_eq!(eff, Effectiveness::SuperEffective, "Ground Earthquake vs Electric = super");
    assert_eq!(text[0], format!("{} used {}!", species_name(s.player.species, false), move_name(mv, false)));
    assert!(text.contains(&"It's super effective!".to_string()), "super-effective line from the real log");
}

// ── P6b TRANSLATOR slice 2: the WHOLE-TURN walk → the production per-turn text ──
//
// The production loop (execute_turn_with_move, mod.rs:1795) emits per turn exactly:
// `format_move_outcome` (the move-announcement block, slice 1) + "{name} fainted!"
// for each faint — names are the UPPERCASE species, prefixed "Enemy " on side 1.
// `translate_turn` walks the full TurnLog and reproduces that, re-deriving
// effectiveness per move and the Blocked reason from the mover's status.

fn battler_at<'a>(state: &'a EngineState<PokeredRules>, who: BattlerRef) -> &'a EngineBattler<PokeredRules> {
    if who.side == 0 { &state.player_battlers[who.slot as usize] } else { &state.opponent_battlers[who.slot as usize] }
}

/// Display name matching the production loop: UPPERCASE species, "Enemy "-prefixed
/// for the opponent side.
fn display_name(state: &EngineState<PokeredRules>, who: BattlerRef) -> String {
    use pokered_data::lang_data::species_name;
    let up = species_name(battler_at(state, who).species, false).to_uppercase();
    if who.side == 0 { up } else { format!("Enemy {}", up) }
}

fn opp_ref(who: BattlerRef) -> BattlerRef {
    BattlerRef::new(if who.side == 0 { 1 } else { 0 }, who.slot)
}

/// The Blocked reason line for a mover, matching `format_cannot_move` wording,
/// derived from the mover's post-turn status / volatiles (best-effort: the wake /
/// snap-out edges read the already-cleared state).
fn blocked_reason(state: &EngineState<PokeredRules>, log: &TurnLog<PokeredRules>, who: BattlerRef) -> &'static str {
    match battler_at(state, who).status {
        Some(LegacyStatus::Sleep(_)) => "is fast asleep!",
        Some(LegacyStatus::Freeze) => "is frozen solid!",
        Some(LegacyStatus::Paralysis) => "is fully paralyzed!",
        _ => {
            // A confusion self-hit logs a Damaged on the mover alongside Blocked.
            if log.events.iter().any(|e| matches!(e, TurnEvent::Damaged { target, .. } if *target == who)) {
                "hurt itself in confusion!"
            } else {
                "can't move!"
            }
        }
    }
}

/// Walk the whole TurnLog → the production per-turn text lines.
fn translate_turn(log: &TurnLog<PokeredRules>, state: &EngineState<PokeredRules>) -> Vec<String> {
    use pokered_data::lang_data::move_name;
    let evs = &log.events;
    let mut msgs = Vec::new();
    for (i, e) in evs.iter().enumerate() {
        match e {
            TurnEvent::MoveUsed { actor, move_ } => {
                let actor = *actor;
                // This mover's outcome flags, gathered until the next mover's block.
                let (mut crit, mut missed) = (false, false);
                for follow in &evs[i + 1..] {
                    match follow {
                        TurnEvent::MoveUsed { .. } | TurnEvent::Blocked { .. } => break,
                        TurnEvent::Crit { actor: a } if *a == actor => crit = true,
                        TurnEvent::Missed { actor: a } if *a == actor => missed = true,
                        _ => {}
                    }
                }
                let md = real_move(*move_);
                let (d1, d2) = {
                    let bs = pokered_data::pokemon_data::get_base_stats(battler_at(state, opp_ref(actor)).species);
                    bs.map(|b| (b.type1, b.type2)).unwrap_or((PokemonType::Normal, PokemonType::Normal))
                };
                let eff = effectiveness_category(md.move_type, d1, d2);
                msgs.extend(move_announcement(&display_name(state, actor), move_name(*move_, false), crit, missed, eff, None));
            }
            TurnEvent::Blocked { actor } => {
                msgs.push(format!("{} {}", display_name(state, *actor), blocked_reason(state, log, *actor)));
            }
            TurnEvent::Fainted { who } => msgs.push(format!("{} fainted!", display_name(state, *who))),
            // Damaged / Healed / Status / StatChanged carry NO text in this loop.
            _ => {}
        }
    }
    msgs
}

/// End-to-end: a KO turn → the exact production lines ("X used Tackle!" + "Enemy X
/// fainted!"). The second mover is cancelled, so no second announcement.
#[test]
fn translate_turn_ko_matches_production_lines() {
    let mut ko = Scenario::base("translate:ko", MoveId::Tackle);
    ko.player = Mon::new(Species::Snorlax, 300, 100);
    ko.enemy = Mon::new(Species::Snorlax, 10, 50); // dies to the player's Tackle
    let (state, log) = stack_run_logged(&ko);
    let text = translate_turn(&log, &state);
    assert_eq!(
        text,
        vec!["SNORLAX used TACKLE!".to_string(), "Enemy SNORLAX fainted!".to_string()],
        "KO turn → move announcement + faint line, no cancelled-second announcement"
    );
}

/// A two-mover non-KO turn → both announcements, in mover order (no faint line).
#[test]
fn translate_turn_two_movers_both_announce() {
    let s = Scenario::base("translate:two", MoveId::Tackle); // both Pikachu, neutral
    let (state, log) = stack_run_logged(&s);
    let text = translate_turn(&log, &state);
    // Player (faster) first, then enemy. Tackle (Normal) vs Electric = neutral ⇒ no
    // effectiveness line either side.
    assert_eq!(
        text,
        vec!["PIKACHU used TACKLE!".to_string(), "Enemy PIKACHU used TACKLE!".to_string()],
        "both movers announce in order, no faint"
    );
}

/// A blocked (asleep) mover → the cannot-move line, derived from status; the awake
/// opponent still announces.
#[test]
fn translate_turn_blocked_sleeper() {
    install_canonical();
    set_active_move(real_move(MoveId::Tackle));
    let mut pb = engine_battler(&Mon::new(Species::Snorlax, 300, 100), MoveId::Tackle);
    pb.status = Some(LegacyStatus::Sleep(2));
    let eb = engine_battler(&Mon::new(Species::Snorlax, 300, 50), MoveId::Tackle);
    let mut state = EngineState::new(vec![pb], vec![eb]);
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Tackle },
        BattleAction::<PokeredRules>::Nothing,
    ];
    let mut rng = ScriptedRng::new(vec![]);
    let (_r, log) =
        StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
    let text = translate_turn(&log, &state);
    assert_eq!(text, vec!["SNORLAX is fast asleep!".to_string()], "asleep mover → cannot-move line, no 'used'");
}

// ═════════════════════════════════════════════════════════════════════════════
// 1. PURE-DAMAGE + the type chart (STAB super-effective / resisted / neutral),
//    crit, 1/256 miss — REAL Gen-1 numbers.
// ═════════════════════════════════════════════════════════════════════════════

/// STAB super-effective: Gyarados (Water/Flying) Surf vs Charizard (Fire/Flying).
/// Water STAB (×1.5) + Water vs Fire (2×) vs Flying (1×) = super-effective. Both
/// paths must deal the SAME real Gen-1 damage. We capture it and assert the exact
/// value so the doc is concrete.
#[test]
fn pure_damage_stab_super_effective() {
    let mut s = Scenario::base("Surf STAB super-effective (Gyarados vs Charizard)", MoveId::Surf);
    s.player = Mon::new(Species::Gyarados, 300, 100); // Water/Flying attacker (STAB)
    s.enemy = Mon::new(Species::Charizard, 300, 50);  // Fire/Flying → Water 2× super
    // Second mover (Charizard Surf, no STAB, vs Gyarados Water = resisted) keeps
    // both above 30 HP.
    run_scenario(&s);

    // Capture the exact damage the player's Surf dealt (the super-effective hit).
    let (stack, _c, _f) = stack_run(&s);
    let dealt = 300 - stack.opponent_battlers[0].hp;
    assert_eq!(
        dealt, 128,
        "STAB super-effective Surf (Gyarados→Charizard) deals the real Gen-1 128"
    );
}

/// Resisted: a Water Surf into a Water defender (Gyarados) = ×0.5. Attacker is
/// Charizard (Fire/Flying → no Water STAB) so the only modifier is the resist.
#[test]
fn pure_damage_resisted() {
    let mut s = Scenario::base("Surf resisted (Charizard vs Gyarados)", MoveId::Surf);
    s.player = Mon::new(Species::Charizard, 300, 100); // no Water STAB
    s.enemy = Mon::new(Species::Gyarados, 300, 50);    // Water/Flying → Water 0.5×
    run_scenario(&s);

    let (stack, _c, _f) = stack_run(&s);
    let dealt = 300 - stack.opponent_battlers[0].hp;
    assert_eq!(dealt, 21, "resisted Surf (Charizard→Gyarados) deals the real Gen-1 21");
}

/// Neutral: Surf (Water) from Charizard (no STAB) vs Snorlax (Normal) = ×1.
#[test]
fn pure_damage_neutral() {
    let mut s = Scenario::base("Surf neutral (Charizard vs Snorlax)", MoveId::Surf);
    s.player = Mon::new(Species::Charizard, 300, 100);
    s.enemy = Mon::new(Species::Snorlax, 300, 50);
    run_scenario(&s);

    let (stack, _c, _f) = stack_run(&s);
    let dealt = 300 - stack.opponent_battlers[0].hp;
    assert_eq!(dealt, 43, "neutral Surf (Charizard→Snorlax) deals the real Gen-1 43");
}

/// A crit: first mover's crit byte 0 (< the Pikachu base-speed/2 threshold) ⇒
/// guaranteed crit (crit doubles the level term). Both paths agree, and the crit
/// hit deals MORE than the same non-crit hit.
#[test]
fn pure_damage_crit() {
    let mut s = Scenario::base("Tackle crit (Snorlax vs Snorlax)", MoveId::Tackle);
    s.player = Mon::new(Species::Snorlax, 300, 100);
    s.enemy = Mon::new(Species::Snorlax, 300, 50);
    s.first = MoveBytes { confusion: 255, paralysis: 255, crit: 0, accuracy: 0, damage: 255, side_effect: 255, multi_hit: 0 };
    run_scenario(&s);

    // The crit hit value, and a non-crit control.
    let (crit_state, _c, _f) = stack_run(&s);
    let crit_dealt = 300 - crit_state.opponent_battlers[0].hp;
    let mut non_crit = s;
    non_crit.first.crit = 255; // above threshold ⇒ no crit
    let (nc_state, _c2, _f2) = stack_run(&non_crit);
    let nc_dealt = 300 - nc_state.opponent_battlers[0].hp;
    assert!(
        crit_dealt > nc_dealt,
        "crit Tackle ({crit_dealt}) must exceed non-crit Tackle ({nc_dealt})"
    );
    assert_eq!((crit_dealt, nc_dealt), (57, 31), "real Gen-1 Snorlax Tackle crit 57 vs non-crit 31");
}

/// A 1/256 miss: accuracy byte 255 vs a 100%-accuracy move scales to 255, and
/// `255 !< 255` ⇒ the deliberate Gen-1 1/256 miss (#2). The first mover misses ⇒
/// the enemy is untouched by it; and the stack draws NO damage byte for the miss.
#[test]
fn pure_damage_one_in_256_miss() {
    let mut s = Scenario::base("Surf 1/256 miss (first mover)", MoveId::Surf);
    s.player = Mon::new(Species::Snorlax, 300, 100);
    s.enemy = Mon::new(Species::Snorlax, 300, 50);
    s.first = MoveBytes { confusion: 255, paralysis: 255, crit: 255, accuracy: 255, damage: 255, side_effect: 255, multi_hit: 0 };
    run_scenario(&s);

    // Explicit: the enemy took ONLY the second mover's hit, not the missed first.
    let (one_hit, _c, _f) = stack_run(&s);
    let mut both_hit = s;
    both_hit.first.accuracy = 0; // now first mover hits too
    let (two_hit, _c2, _f2) = stack_run(&both_hit);
    assert!(
        one_hit.opponent_battlers[0].hp > two_hit.opponent_battlers[0].hp,
        "the missed first move must leave the enemy with MORE hp than a double hit"
    );
}

/// Swift never misses and draws NO accuracy byte (the SwiftEffect bypass) — the
/// stack stream omits the accuracy byte for both movers, and parity holds.
#[test]
fn swift_always_hits_and_draws_no_accuracy_byte() {
    let s = Scenario::base("Swift always-hit (no accuracy draw)", MoveId::Swift);
    run_scenario(&s);
    // Swift is power 60: per mover the stack draws crit + damage = 2 (NO acc).
    let first = first_mover(&s);
    let stream = build_stream(&s, first, order_is_tie(&s));
    assert_eq!(stream.len(), 4, "two movers × (crit, damage) = 4 bytes (no accuracy)");
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. SELF-BOOST (+1 / +2 with the −6..+6 clamp).
// ═════════════════════════════════════════════════════════════════════════════

/// +1 self-Boost (Growth = SpecialUp1Effect). Both movers raise their own Special
/// one stage; both paths agree. Power-0 ⇒ only the accuracy byte is drawn (no
/// crit, no damage) per mover.
#[test]
fn self_boost_plus_1_special() {
    let s = Scenario::base("Growth +1 Special (both movers)", MoveId::Growth);
    run_scenario(&s);

    let (stack, consumed, first) = stack_run(&s);
    assert_eq!(
        stack.player_battlers[0].stat_stages.get(StatIndex::Special).copied().unwrap_or(0),
        1,
        "Growth raised the player's Special to +1"
    );
    assert_eq!(
        stack.opponent_battlers[0].stat_stages.get(StatIndex::Special).copied().unwrap_or(0),
        1,
        "Growth raised the enemy's Special to +1 too"
    );
    // power-0: per mover only the accuracy byte. Two movers = 2 bytes.
    let _ = first;
    assert_eq!(consumed, 2, "two power-0 self-Boost movers draw 2 accuracy bytes total");
}

/// +2 self-Boost (SwordsDance = AttackUp2Effect): both movers' Attack to +2.
#[test]
fn self_boost_plus_2_attack() {
    let s = Scenario::base("Swords Dance +2 Attack", MoveId::SwordsDance);
    run_scenario(&s);

    let (stack, _c, _f) = stack_run(&s);
    assert_eq!(
        stack.player_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
        2,
        "Swords Dance raised the player's Attack to +2"
    );
}

/// The −6..+6 clamp (Gen-1 bug #30): start the player at +5 Attack; a +2 Swords
/// Dance clamps to +6, NOT +7. Both paths clamp identically. (Only the player
/// pre-boosted; the enemy goes 0 → +2.)
#[test]
fn self_boost_clamp_at_plus_6() {
    let s = Scenario::base("Swords Dance clamp +5 → +6", MoveId::SwordsDance);
    // Both paths pre-set the player's Attack stage to +5, then Swords Dance.
    let legacy = {
        let mut state = new_battle_state(
            BattleType::Wild,
            vec![poke(&s.player, s.move_id)],
            vec![poke(&s.enemy, s.move_id)],
        );
        state.player.selected_move = s.move_id;
        state.enemy.selected_move = s.move_id;
        state.player.stat_stages.attack = 5;
        let md = real_move(s.move_id);
        let randoms = TurnRandoms {
            order_random: s.order_byte,
            first_mover: to_move_randoms(s.first),
            second_mover: to_move_randoms(s.second),
        };
        execute_turn(&mut state, &md, &md, &randoms);
        state
    };
    let stack = {
        install_canonical();
        set_active_move(real_move(s.move_id));
        let mut pb = engine_battler(&s.player, s.move_id);
        pb.stat_stages.set(StatIndex::Attack, 5);
        let mut state = EngineState::new(vec![pb], vec![engine_battler(&s.enemy, s.move_id)]);
        let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
        let provider = PokeredRules;
        let actions = [
            BattleAction::<PokeredRules>::Fight { move_: s.move_id },
            BattleAction::<PokeredRules>::Fight { move_: s.move_id },
        ];
        let mut rng = ScriptedRng::new(build_stream(&s, first_mover(&s), order_is_tie(&s)));
        StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
        state
    };
    assert_eq!(legacy.player.stat_stages.attack, 6, "legacy clamps +5 +2 → +6");
    assert_eq!(
        stack.player_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
        6,
        "stack clamps +5 +2 → +6 (the −6..+6 bug #30)"
    );
    assert_eq!(
        legacy.player.stat_stages.attack,
        stack.player_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
        "legacy == stack clamp"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. SELF-HEAL (Recover / Softboiled).
// ═════════════════════════════════════════════════════════════════════════════

/// Recover heals 1/2 the user's max HP (capped). Start both movers at half HP so
/// the heal is fully observable; both paths heal identically.
#[test]
fn self_heal_recover_half_max_hp() {
    let mut s = Scenario::base("Recover heals 1/2 max HP", MoveId::Recover);
    s.player = Mon::new(Species::Snorlax, 200, 100);
    s.enemy = Mon::new(Species::Snorlax, 200, 50);
    // Both start at 100/200 (set below in the run helpers via a damaged start).
    // We damage both to 100 first on both paths.
    let legacy = {
        let mut state = new_battle_state(
            BattleType::Wild,
            vec![poke(&s.player, s.move_id)],
            vec![poke(&s.enemy, s.move_id)],
        );
        state.player.active_mon_mut().hp = 100;
        state.enemy.active_mon_mut().hp = 100;
        state.player.selected_move = s.move_id;
        state.enemy.selected_move = s.move_id;
        let md = real_move(s.move_id);
        let randoms = TurnRandoms {
            order_random: s.order_byte,
            first_mover: to_move_randoms(s.first),
            second_mover: to_move_randoms(s.second),
        };
        execute_turn(&mut state, &md, &md, &randoms);
        state
    };
    let stack = {
        install_canonical();
        set_active_move(real_move(s.move_id));
        let mut pb = engine_battler(&s.player, s.move_id);
        pb.hp = 100;
        let mut eb = engine_battler(&s.enemy, s.move_id);
        eb.hp = 100;
        let mut state = EngineState::new(vec![pb], vec![eb]);
        let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
        let provider = PokeredRules;
        let actions = [
            BattleAction::<PokeredRules>::Fight { move_: s.move_id },
            BattleAction::<PokeredRules>::Fight { move_: s.move_id },
        ];
        let mut rng = ScriptedRng::new(build_stream(&s, first_mover(&s), order_is_tie(&s)));
        StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
        state
    };
    // 100 + 200/2 = 200 (capped at max), both sides, both paths.
    assert_eq!(legacy.player.active_mon().hp, 200, "legacy Recover: 100 + 100 = 200");
    assert_eq!(stack.player_battlers[0].hp, 200, "stack Recover: 100 + 100 = 200");
    assert_eq!(legacy.enemy.active_mon().hp, stack.opponent_battlers[0].hp, "enemy heal parity");
    assert_eq!(legacy.player.active_mon().hp, stack.player_battlers[0].hp, "player heal parity");
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. Focus Energy + the /4 crit bug rides through the data path unchanged.
// ═════════════════════════════════════════════════════════════════════════════

/// Focus Energy makes a crit LESS likely (the Gen-1 ÷4 bug #1). With the focus
/// volatile set on the player and a crit byte just under the normal threshold,
/// the player does NOT crit (the ÷4 cut threshold) — both paths agree.
#[test]
fn focus_energy_div4_crit_bug_parity() {
    // Pikachu base speed 90 → normal threshold 45; with Focus Energy ÷4 → 11.
    // crit byte 20: < 45 (would crit normally) but >= 11 (no crit with focus).
    let mut s = Scenario::base("Focus Energy ÷4 crit bug", MoveId::Tackle);
    s.player = Mon { focus_energy: true, ..Mon::new(Species::Pikachu, 300, 100) };
    s.enemy = Mon::new(Species::Pikachu, 300, 50);
    s.first = MoveBytes { confusion: 255, paralysis: 255, crit: 20, accuracy: 0, damage: 255, side_effect: 255, multi_hit: 0 };
    run_scenario(&s);
}

// ═════════════════════════════════════════════════════════════════════════════
// 5. The whole bucket-A matrix (all authored moves, both orders + a speed tie).
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn bucket_a_matrix() {
    let damage_moves = [MoveId::Surf, MoveId::Earthquake, MoveId::Swift, MoveId::Tackle];
    let boost_moves = [
        MoveId::SwordsDance, MoveId::Amnesia, MoveId::Agility, MoveId::Growth,
        MoveId::Harden, MoveId::Meditate, MoveId::DoubleTeam, MoveId::AcidArmor,
    ];
    let mut scenarios: Vec<Scenario> = Vec::new();
    for m in damage_moves {
        let mut s = Scenario::base("dmg player-first", m);
        s.player = Mon::new(Species::Snorlax, 400, 100);
        s.enemy = Mon::new(Species::Snorlax, 400, 50);
        scenarios.push(s);
        let mut e = Scenario::base("dmg enemy-first", m);
        e.player = Mon::new(Species::Snorlax, 400, 50);
        e.enemy = Mon::new(Species::Snorlax, 400, 100);
        scenarios.push(e);
        let mut t = Scenario::base("dmg speed tie order 0", m);
        t.player = Mon::new(Species::Snorlax, 400, 80);
        t.enemy = Mon::new(Species::Snorlax, 400, 80);
        scenarios.push(t);
        let mut t2 = Scenario::base("dmg speed tie order 200", m);
        t2.player = Mon::new(Species::Snorlax, 400, 80);
        t2.enemy = Mon::new(Species::Snorlax, 400, 80);
        t2.order_byte = 200;
        scenarios.push(t2);
    }
    for m in boost_moves {
        scenarios.push(Scenario::base("boost player-first", m));
        let mut e = Scenario::base("boost enemy-first", m);
        e.player = Mon::new(Species::Pikachu, 200, 50);
        e.enemy = Mon::new(Species::Pikachu, 200, 100);
        scenarios.push(e);
    }
    // Recover (heal) at full HP ⇒ no change, parity (the StatBlocked path).
    scenarios.push(Scenario::base("recover at full hp", MoveId::Recover));
    // Splash ⇒ pure no-op, parity.
    scenarios.push(Scenario::base("splash no-op", MoveId::Splash));

    for s in &scenarios {
        run_scenario(s);
    }
    assert!(scenarios.len() >= 30, "matrix covers the bucket-A set ({} scenarios)", scenarios.len());
}

// ═════════════════════════════════════════════════════════════════════════════
// 6. Dual-mode (baked == disk) + hot-reload load proofs.
// ═════════════════════════════════════════════════════════════════════════════

/// Baked text and disk text compile to byte-identical registries (the dual-mode
/// invariant). Both load through the SAME `Ruleset::from_ron`.
#[test]
fn baked_and_disk_yield_identical_ruleset() {
    use super::{compile, load_ruleset, RULES_RON_PATH};
    use dotzuki_rules::RuleSource;
    let baked = load_ruleset(false).load().expect("baked parses");
    let disk = RuleSource::from_path(RULES_RON_PATH).load().expect("disk parses");
    let cb = compile(&baked).expect("baked compiles");
    let cd = compile(&disk).expect("disk compiles");
    assert_eq!(baked.effects.len(), disk.effects.len(), "same effect count");
    assert_eq!(cb.hooks.len(), cd.hooks.len(), "same compiled-hook count");
    assert_eq!(cb.types, cd.types, "same interned types");
    assert_eq!(cb.stats, cd.stats, "same interned stats");
    let mut a: Vec<_> = cb.hooks.values()
        .map(|h| (h.event, h.source_id.clone(), h.ops.clone(), h.order, h.chance)).collect();
    let mut b: Vec<_> = cd.hooks.values()
        .map(|h| (h.event, h.source_id.clone(), h.ops.clone(), h.order, h.chance)).collect();
    a.sort_by(|x, y| (x.1.clone(), format!("{:?}", x.0)).cmp(&(y.1.clone(), format!("{:?}", y.0))));
    b.sort_by(|x, y| (x.1.clone(), format!("{:?}", x.0)).cmp(&(y.1.clone(), format!("{:?}", y.0))));
    assert_eq!(a, b, "baked and disk compile to identical compiled hooks");
}

/// Drive the SAME bucket-A battle through a baked-built registry AND a disk-built
/// registry; assert identical outcome — the runtime ruleset is the same both ways.
#[test]
fn baked_and_disk_drive_identical_battle() {
    use super::{compile, install_compiled, load_ruleset, RULES_RON_PATH};
    use dotzuki_rules::RuleSource;
    let s = Scenario::base("Surf baked==disk", MoveId::Surf);
    let run_one = || {
        set_active_move(real_move(s.move_id));
        let mut state = EngineState::new(
            vec![engine_battler(&s.player, s.move_id)],
            vec![engine_battler(&s.enemy, s.move_id)],
        );
        let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
        let provider = PokeredRules;
        let actions = [
            BattleAction::<PokeredRules>::Fight { move_: s.move_id },
            BattleAction::<PokeredRules>::Fight { move_: s.move_id },
        ];
        let mut rng = ScriptedRng::new(build_stream(&s, first_mover(&s), order_is_tie(&s)));
        StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
        (200u16.saturating_sub(state.opponent_battlers[0].hp), rng.consumed())
    };
    install_compiled(compile(&load_ruleset(false).load().unwrap()).unwrap());
    let baked = run_one();
    install_compiled(compile(&RuleSource::from_path(RULES_RON_PATH).load().unwrap()).unwrap());
    let disk = run_one();
    assert_eq!(baked, disk, "baked and disk drive identical battle outcome + draws");
    install_canonical();
}

/// The combined per-move effect actually resolves for the authored bucket-A moves
/// (the registry is installed + indexed), and the DATA op-lists are the authored
/// ones (a sanity witness that these are genuinely data, not native).
#[test]
fn authored_moves_resolve_as_data() {
    install_canonical();
    use dotzuki_rules::{FractionOf, Op, Selector};
    // Pure-damage moves carry [DealMoveDamage] + [ApplyTypeChart].
    assert!(super::record_has_op("move.surf", &Op::DealMoveDamage), "surf is DealMoveDamage data");
    assert!(super::record_has_op("move.surf", &Op::ApplyTypeChart), "surf is ApplyTypeChart data");
    // Self-Boost carries Boost(Source).
    assert!(
        super::record_has_op("boost.attack_up_2", &Op::Boost {
            stat: "Attack".into(), stages: 2, target: Selector::Source
        }),
        "swords dance is a +2 Attack Boost on Source"
    );
    // Self-heal carries HealFraction(MaxHp, 1/2, Source).
    assert!(
        super::record_has_op("heal.recover", &Op::HealFraction {
            num: 1, den: 2, of: FractionOf::MaxHp, target: Selector::Source, unless: None
        }),
        "recover is a 1/2 MaxHp HealFraction on Source"
    );
    // Every authored move resolves to a combined effect.
    for m in [MoveId::Surf, MoveId::SwordsDance, MoveId::Recover, MoveId::Splash] {
        assert!(move_effect_for(m).is_some(), "{m:?} resolves to a combined effect");
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// P2 — SIDE-STATUS + DRAIN/RECOIL differential parity (blueprint 15 §5 P2).
//
// Reuses the SYMMETRIC two-mover harness (both movers use the SAME move + its real
// MoveData, so the active-move thread-local is correct for both). For each P2
// effect the SAME byte vector flows through (a) the LEGACY oracle (`execute_turn`)
// and (b) the STACK (`StackDriver::execute_turn`), asserting IDENTICAL hp + status
// BOTH sides AND identical `consumed()`.
//
// The side-effect byte is drawn at the legacy ordinal (after `damage`) whether or
// not the secondary fires (`build_stream` always pushes it for a side-status
// move), so `consumed()` is invariant fired-vs-not.
//
// Symmetry is SAFE for burn (Ember = special ⇒ burn's attack-halving never bites),
// poison, and paralysis (paralysis_roll = 255 ⇒ never fully paralyzed; speed is
// post-order): a just-inflicted status never perturbs the second mover's move. The
// guard tests (Substitute / #23 / poison-immunity) inflict NOTHING on EITHER mover
// (symmetric setup), so both act cleanly. Freeze is handled separately (a frozen
// mon can't act — P5 — so its FIRES test fains the defender to cancel the reply).
// ═════════════════════════════════════════════════════════════════════════════

/// Extra P2 setup atop a `Scenario`: a Substitute on BOTH movers + a pre-set HP on
/// BOTH movers (drain/recoil observability). Symmetric so both sides stay parity.
#[derive(Clone, Copy)]
struct P2Setup {
    substitute_both: bool,
    hp_both: Option<u16>,
}

impl P2Setup {
    fn none() -> Self {
        Self { substitute_both: false, hp_both: None }
    }
    fn sub() -> Self {
        Self { substitute_both: true, hp_both: None }
    }
    fn hp(hp: u16) -> Self {
        Self { substitute_both: false, hp_both: Some(hp) }
    }
}

/// LEGACY symmetric runner that drives the two movers via `execute_move` (per
/// mover) but DELIBERATELY SKIPS the end-of-turn residual phase. The P2 stack
/// fires `residual_and_faint` too, but P2 registers no status/volatile residual
/// handler (poison/burn chip is P5), so the stack residual is inert — this legacy
/// runner matches that by not chipping either. Faint short-circuit is preserved
/// (a fainted defender cancels the reply, exactly like the stack).
fn p2_legacy(s: &Scenario, setup: P2Setup) -> LegacyState {
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![poke(&s.player, s.move_id)],
        vec![poke(&s.enemy, s.move_id)],
    );
    state.player.selected_move = s.move_id;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = s.move_id;
    state.enemy.selected_move_index = 0;
    if let Some(hp) = setup.hp_both {
        state.player.active_mon_mut().hp = hp;
        state.enemy.active_mon_mut().hp = hp;
    }
    if setup.substitute_both {
        state.player.set_status2(status2::HAS_SUBSTITUTE_UP);
        state.player.substitute_hp = 50;
        state.enemy.set_status2(status2::HAS_SUBSTITUTE_UP);
        state.enemy.substitute_hp = 50;
    }
    let md = real_move(s.move_id);
    // Use the FULL legacy turn (ordering + per-mover residual + faint short-circuit),
    // matching `legacy_run` and the stack path. P6b-prereq put burn/poison residual on
    // the stack, so the oracle must include it too; for non-status moves residual is a
    // no-op, so every existing P2 scenario is unchanged.
    let randoms = TurnRandoms {
        order_random: s.order_byte,
        first_mover: to_move_randoms(s.first),
        second_mover: to_move_randoms(s.second),
    };
    execute_turn(&mut state, &md, &md, &randoms);
    state
}

fn p2_stack(s: &Scenario, setup: P2Setup) -> (EngineState<PokeredRules>, usize) {
    install_canonical();
    set_active_move(real_move(s.move_id));
    let mut pb = engine_battler(&s.player, s.move_id);
    let mut eb = engine_battler(&s.enemy, s.move_id);
    if let Some(hp) = setup.hp_both {
        pb.hp = hp;
        eb.hp = hp;
    }
    let mut state = EngineState::new(vec![pb], vec![eb]);
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    if setup.substitute_both {
        effects.push(EffectState {
            id: EffectId(200), host: BattlerRef::PLAYER, effect_order: 0,
            kind: PokeVolatile::Substitute,
        });
        effects.push(EffectState {
            id: EffectId(201), host: BattlerRef::OPPONENT, effect_order: 1,
            kind: PokeVolatile::Substitute,
        });
    }
    let provider = PokeredRules;
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
    ];
    let first = first_mover(s);
    let tie = order_is_tie(s);
    // A subbed second mover blocks a first-mover-inflicted paralysis ⇒ no para gate
    // byte; thread the substitute flag so the predictor matches.
    let bytes = build_stream_sub(s, first, tie, setup.substitute_both);
    let mut rng = ScriptedRng::new(bytes);
    StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
    (state, rng.consumed())
}

/// Assert IDENTICAL hp + status (both sides) AND the predicted consumed().
fn p2_assert(s: &Scenario, setup: P2Setup) {
    let legacy = p2_legacy(s, setup);
    let (stack, consumed) = p2_stack(s, setup);
    let lp = legacy.player.active_mon();
    let le = legacy.enemy.active_mon();
    let sp = &stack.player_battlers[0];
    let se = &stack.opponent_battlers[0];
    assert_eq!(lp.hp, sp.hp, "[{}] PLAYER hp: legacy={} stack={}", s.name, lp.hp, sp.hp);
    assert_eq!(le.hp, se.hp, "[{}] ENEMY hp: legacy={} stack={}", s.name, le.hp, se.hp);
    assert_eq!(lp.status, sp.status.unwrap_or(LegacyStatus::None), "[{}] PLAYER status", s.name);
    assert_eq!(le.status, se.status.unwrap_or(LegacyStatus::None), "[{}] ENEMY status", s.name);
    let expected = build_stream_sub(s, first_mover(s), order_is_tie(s), setup.substitute_both).len();
    assert_eq!(consumed, expected, "[{}] consumed() drift: got {} want {}", s.name, consumed, expected);
}

/// A side-status base scenario: both Pikachu (Electric — not Fire/Poison/Normal so
/// no #23 / poison immunity bites), enough HP that nobody faints.
fn side_base(name: &'static str, move_id: MoveId, side_effect: u8) -> Scenario {
    let mut s = Scenario::base(name, move_id);
    s.player = Mon::new(Species::Pikachu, 300, 100);
    s.enemy = Mon::new(Species::Pikachu, 300, 50);
    s.first.side_effect = side_effect;
    s.second.side_effect = side_effect;
    s
}

/// 10%-ish BURN side-status (Ember, BurnSideEffect1 26/256) FIRES: byte 0 < 26 ⇒
/// both movers burn each other. Burn is special-safe (Ember is Special).
#[test]
fn side_status_burn_fires() {
    let s = side_base("Ember burn FIRES (26/256, byte 0)", MoveId::Ember, 0);
    p2_assert(&s, P2Setup::none());
    let (stack, _c) = p2_stack(&s, P2Setup::none());
    assert_eq!(stack.opponent_battlers[0].status, Some(LegacyStatus::Burn), "byte 0 < 26 ⇒ enemy BURNED");
    assert_eq!(stack.player_battlers[0].status, Some(LegacyStatus::Burn), "byte 0 < 26 ⇒ player BURNED");
}

/// The SAME Ember that does NOT fire: byte 26 (!< 26) ⇒ no burn — but the byte is
/// STILL drawn at the legacy ordinal (consumed() invariant fired-vs-not).
#[test]
fn side_status_burn_does_not_fire_but_byte_drawn() {
    let s = side_base("Ember burn NO-FIRE (byte 26)", MoveId::Ember, 26);
    p2_assert(&s, P2Setup::none());
    let (stack, consumed) = p2_stack(&s, P2Setup::none());
    assert_eq!(stack.opponent_battlers[0].status, None, "byte 26 ⇒ NO burn");
    assert_eq!(stack.player_battlers[0].status, None, "byte 26 ⇒ NO burn");
    // Same consumed() as the FIRES case — the chance byte is drawn either way.
    let fired = side_base("ember fired", MoveId::Ember, 0);
    let (_st2, consumed_fired) = p2_stack(&fired, P2Setup::none());
    assert_eq!(consumed, consumed_fired,
        "consumed() identical whether the secondary fires (byte 0) or not (byte 26)");
}

/// 30%-ish PARALYZE side-status (Body Slam, ParalyzeSideEffect2 77/256) FIRES (byte
/// 76 < 77) and NOT (byte 77). Pikachu (Electric) ⇒ no #23 (Body Slam is Normal).
#[test]
fn side_status_paralyze_30pct_fires_and_not() {
    let fires = side_base("Body Slam paralyze FIRES (77/256, byte 76)", MoveId::BodySlam, 76);
    p2_assert(&fires, P2Setup::none());
    let (st, _c) = p2_stack(&fires, P2Setup::none());
    assert_eq!(st.opponent_battlers[0].status, Some(LegacyStatus::Paralysis), "byte 76 < 77 ⇒ paralyzed");
    let not = side_base("Body Slam paralyze NO-FIRE (byte 77)", MoveId::BodySlam, 77);
    p2_assert(&not, P2Setup::none());
    let (st2, _c2) = p2_stack(&not, P2Setup::none());
    assert_eq!(st2.opponent_battlers[0].status, None, "byte 77 ⇒ no paralysis");
}

/// The Substitute block: with a Substitute on BOTH movers, the status is vetoed.
/// Uses a power-0 primary-status move (Thunder Wave) so there is no damage to
/// redirect (Substitute damage absorption is P5/#28, not P2) — isolating the
/// status block cleanly. Snorlax movers (Normal ⇒ no #23 on the Electric TW).
#[test]
fn side_status_blocked_by_substitute() {
    let mut s = Scenario::base("Thunder Wave blocked by Substitute", MoveId::ThunderWave);
    s.player = Mon::new(Species::Snorlax, 300, 100);
    s.enemy = Mon::new(Species::Snorlax, 300, 50);
    p2_assert(&s, P2Setup::sub());
    let (stack, _c) = p2_stack(&s, P2Setup::sub());
    assert_eq!(stack.opponent_battlers[0].status, None, "Substitute blocks the paralysis (enemy)");
    assert_eq!(stack.player_battlers[0].status, None, "Substitute blocks the paralysis (player)");
    // Control: WITHOUT a Substitute, the same Thunder Wave DOES paralyze.
    let (open, _c2) = p2_stack(&s, P2Setup::none());
    assert_eq!(open.opponent_battlers[0].status, Some(LegacyStatus::Paralysis),
        "no Substitute ⇒ paralysis lands");
}

/// The type-immunity quirk #23: a Fire move can't burn a Fire-type defender, even
/// when the byte FIRES. Both movers Charmander (Fire) Ember ⇒ neither burns.
#[test]
fn side_status_type_immunity_quirk_23() {
    let mut s = side_base("Ember can't burn a Fire-type (#23)", MoveId::Ember, 0);
    s.player = Mon::new(Species::Charmander, 300, 100); // Fire
    s.enemy = Mon::new(Species::Charmander, 300, 50);   // Fire
    p2_assert(&s, P2Setup::none());
    let (stack, _c) = p2_stack(&s, P2Setup::none());
    assert_eq!(stack.opponent_battlers[0].status, None, "Fire-type immune to its own-type burn (#23)");
    assert_eq!(stack.player_battlers[0].status, None, "Fire-type immune to its own-type burn (#23)");
}

/// Poison-type immunity (poison-side): Poison Sting (PoisonSideEffect1 51/256)
/// can't poison a Poison-type. Both Nidoking (Poison/Ground) ⇒ neither poisoned;
/// a non-Poison pair (Pikachu) IS poisoned.
#[test]
fn side_status_poison_type_immunity() {
    let mut immune = side_base("Poison Sting can't poison a Poison-type", MoveId::PoisonSting, 0);
    immune.player = Mon::new(Species::Nidoking, 300, 100);
    immune.enemy = Mon::new(Species::Nidoking, 300, 50);
    p2_assert(&immune, P2Setup::none());
    let (st, _c) = p2_stack(&immune, P2Setup::none());
    assert_eq!(st.opponent_battlers[0].status, None, "Poison-type immune to poison");
    let bitten = side_base("Poison Sting poisons a non-Poison type", MoveId::PoisonSting, 0);
    p2_assert(&bitten, P2Setup::none());
    let (st2, _c2) = p2_stack(&bitten, P2Setup::none());
    assert_eq!(st2.opponent_battlers[0].status, Some(LegacyStatus::Poison), "non-Poison ⇒ poisoned");
}

/// FREEZE side-status (Ice Beam, FreezeSideEffect1 26/256) FIRES: byte 0 < 26. A
/// frozen mon can't act (P5), so we make the FIRST mover's Ice Beam FAINT the
/// defender — the reply is cancelled on BOTH paths, isolating the freeze. Player
/// (Charizard, fast) freezes a low-HP Charizard; the enemy faints + is frozen.
#[test]
fn side_status_freeze_fires_defender_faints() {
    let mut s = Scenario::base("Ice Beam freeze FIRES (defender faints)", MoveId::IceBeam);
    s.player = Mon::new(Species::Snorlax, 300, 100);
    s.enemy = Mon::new(Species::Charizard, 10, 50); // Fire/Flying: Ice 2× ⇒ faints; low HP
    s.first.side_effect = 0;  // freeze fires
    s.second.side_effect = 0;
    // Legacy + stack: player's Ice Beam faints the enemy, the reply is cancelled.
    let legacy = p2_legacy(&s, P2Setup::none());
    let (stack, consumed) = p2_stack(&s, P2Setup::none());
    assert_eq!(stack.opponent_battlers[0].hp, 0, "enemy fainted");
    assert_eq!(legacy.enemy.active_mon().hp, 0, "legacy enemy fainted too");
    assert_eq!(
        legacy.enemy.active_mon().status,
        stack.opponent_battlers[0].status.unwrap_or(LegacyStatus::None),
        "freeze parity on the fainted defender"
    );
    assert_eq!(stack.opponent_battlers[0].status, Some(LegacyStatus::Freeze), "the defender is FROZEN");
    // Only the first mover's bytes are drawn (the reply is cancelled): crit, acc,
    // damage, side_effect = 4.
    assert_eq!(consumed, 4, "defender faints ⇒ only first mover's 4 bytes (incl. the freeze byte)");
}

/// PRIMARY status (Thunder Wave, ParalyzeEffect): guaranteed paralysis on a hit,
/// no chance byte. Both Pikachu (Electric — not paralysis-#23-immune; TW is
/// Electric so a Electric defender WOULD be #23-immune!). Use non-Electric movers.
#[test]
fn primary_status_thunder_wave() {
    let mut s = Scenario::base("Thunder Wave paralyzes (primary)", MoveId::ThunderWave);
    s.player = Mon::new(Species::Snorlax, 300, 100); // Normal
    s.enemy = Mon::new(Species::Snorlax, 300, 50);   // Normal (TW Electric ⇒ no #23)
    p2_assert(&s, P2Setup::none());
    let (stack, _c) = p2_stack(&s, P2Setup::none());
    assert_eq!(stack.opponent_battlers[0].status, Some(LegacyStatus::Paralysis), "TW paralyzes enemy");
    assert_eq!(stack.player_battlers[0].status, Some(LegacyStatus::Paralysis), "TW paralyzes player too");
}

/// PRIMARY poison (Poisonpowder, PoisonEffect plain branch): guaranteed poison on a
/// hit. Both Pikachu (non-Poison). The Toxic badly-poisoned branch is DEFERRED.
#[test]
fn primary_status_poisonpowder() {
    let mut s = Scenario::base("Poisonpowder poisons (primary)", MoveId::Poisonpowder);
    s.player = Mon::new(Species::Pikachu, 200, 100);
    s.enemy = Mon::new(Species::Pikachu, 200, 50);
    p2_assert(&s, P2Setup::none());
    let (stack, _c) = p2_stack(&s, P2Setup::none());
    assert_eq!(stack.opponent_battlers[0].status, Some(LegacyStatus::Poison), "Poisonpowder poisons enemy");
}

/// DRAIN (Mega Drain) symmetric parity: both movers drain each other; legacy and
/// stack agree on hp + status both sides + consumed(). The exact heal value is
/// pinned in `drain_exact_value_isolated`.
#[test]
fn drain_heals_attacker_half_damage_dealt() {
    let mut s = Scenario::base("Mega Drain symmetric parity", MoveId::MegaDrain);
    s.player = Mon::new(Species::Snorlax, 300, 100);
    s.enemy = Mon::new(Species::Snorlax, 300, 50);
    p2_assert(&s, P2Setup::hp(150)); // both at 150/300 so the heal is visible
}

/// A one-sided drain/recoil witness helper: per-side HP override (so the defender
/// can be set to faint, cancelling the reply and isolating the first mover).
fn p2_stack_hp(s: &Scenario, php: Option<u16>, ehp: Option<u16>) -> (EngineState<PokeredRules>, usize) {
    install_canonical();
    set_active_move(real_move(s.move_id));
    let mut pb = engine_battler(&s.player, s.move_id);
    let mut eb = engine_battler(&s.enemy, s.move_id);
    if let Some(hp) = php { pb.hp = hp; }
    if let Some(hp) = ehp { eb.hp = hp; }
    let mut state = EngineState::new(vec![pb], vec![eb]);
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let provider = PokeredRules;
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
    ];
    let mut rng = ScriptedRng::new(build_stream(s, first_mover(s), order_is_tie(s)));
    StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
    (state, rng.consumed())
}

/// DRAIN exact value (one-sided, isolated): player Mega Drain faints a low-HP enemy
/// and heals half the damage dealt. Real Gen-1 number.
#[test]
fn drain_exact_value_isolated() {
    let mut s = Scenario::base("Mega Drain exact heal (isolated)", MoveId::MegaDrain);
    s.player = Mon::new(Species::Pikachu, 200, 100);
    s.enemy = Mon::new(Species::Snorlax, 300, 50);
    // Legacy single move (player only) for the exact heal.
    let mut legacy = new_battle_state(
        BattleType::Wild, vec![poke(&s.player, s.move_id)], vec![poke(&s.enemy, s.move_id)]);
    legacy.whose_turn = Side::Player;
    legacy.player.selected_move = s.move_id;
    legacy.player.active_mon_mut().hp = 100;
    crate::battle::move_execution::execute_move(&mut legacy, &real_move(s.move_id), &to_move_randoms(s.first));
    let dealt = 300 - legacy.enemy.active_mon().hp;
    let healed = legacy.player.active_mon().hp - 100;
    assert_eq!(healed, (dealt / 2).max(1), "legacy drain heals (dealt/2).max(1)");
    // Stack: faint the enemy so only the player's move runs; same heal math.
    let mut st = s;
    st.enemy = Mon::new(Species::Snorlax, dealt, 50); // exactly lethal so reply cancels
    let (stack, _c) = p2_stack_hp(&st, Some(100), Some(dealt));
    let stack_healed = stack.player_battlers[0].hp - 100;
    assert_eq!(stack_healed, (dealt / 2).max(1), "stack drain heals (dealt/2).max(1) too");
    assert_eq!(healed, stack_healed, "legacy == stack drain heal");
    // Concrete witness.
    assert_eq!((dealt, healed), (19, 9), "real Gen-1: Mega Drain Pikachu→Snorlax deals 19, drains 9");
}

/// RECOIL (Take Down): the attacker takes a QUARTER of the damage dealt. One-sided
/// isolated witness + the symmetric parity. Real Gen-1 number.
#[test]
fn recoil_hits_attacker_quarter_damage_dealt() {
    let mut s = Scenario::base("Take Down recoils a quarter the damage dealt", MoveId::TakeDown);
    s.player = Mon::new(Species::Snorlax, 400, 100);
    s.enemy = Mon::new(Species::Snorlax, 400, 50);
    p2_assert(&s, P2Setup::none()); // symmetric parity (both recoil)
    // Legacy single move for the exact recoil.
    let mut legacy = new_battle_state(
        BattleType::Wild, vec![poke(&s.player, s.move_id)], vec![poke(&s.enemy, s.move_id)]);
    legacy.whose_turn = Side::Player;
    legacy.player.selected_move = s.move_id;
    crate::battle::move_execution::execute_move(&mut legacy, &real_move(s.move_id), &to_move_randoms(s.first));
    let dealt = 400 - legacy.enemy.active_mon().hp;
    let recoil = 400 - legacy.player.active_mon().hp;
    assert_eq!(recoil, (dealt / 4).max(1), "legacy recoil is (dealt/4).max(1)");
    // Stack one-sided (enemy faints ⇒ reply cancelled).
    let mut st = s;
    st.enemy = Mon::new(Species::Snorlax, dealt, 50);
    let (stack, _c) = p2_stack_hp(&st, Some(400), Some(dealt));
    let stack_recoil = 400 - stack.player_battlers[0].hp;
    assert_eq!(stack_recoil, (dealt / 4).max(1), "stack recoil is (dealt/4).max(1)");
    assert_eq!(recoil, stack_recoil, "legacy == stack recoil");
    // Concrete witness.
    assert_eq!((dealt, recoil), (76, 19), "real Gen-1: Take Down Snorlax→Snorlax deals 76, recoils 19");
}

/// The data sanity witness: the P2 effects are genuinely DATA (the authored
/// op-lists), not native code.
#[test]
fn p2_authored_as_data() {
    install_canonical();
    use dotzuki_rules::{FractionOf, Op, Predicate, Selector};
    assert!(super::record_has_op("drain.absorb", &Op::HealFraction {
        num: 1, den: 2, of: FractionOf::LastDamage, target: Selector::Source, unless: None
    }), "drain is a 1/2 LastDamage HealFraction on Source");
    assert!(super::record_has_op("recoil.take_down", &Op::DamageFraction {
        num: 1, den: 4, of: FractionOf::LastDamage, target: Selector::Source, unless: None
    }), "recoil is a 1/4 LastDamage DamageFraction on Source");
    assert!(super::record_has_op("side.burn_1", &Op::VetoIf {
        cond: Predicate::HasVolatile("Substitute".into()), silent: false
    }), "burn-side vetoes on Substitute");
    assert!(super::record_has_op("side.burn_1", &Op::VetoIf {
        cond: Predicate::MoveTypeIsDefenderType, silent: false
    }), "burn-side vetoes on the #23 type immunity");
    assert!(super::record_has_op("side.burn_1", &Op::InflictStatus {
        status: "burn".into(), target: Selector::Target, amount: Default::default()
    }), "burn-side inflicts burn on the target");
    assert!(super::record_has_op("side.poison_1", &Op::VetoIf {
        cond: Predicate::HasType("Poison".into()), silent: false
    }), "poison-side vetoes on the Poison type immunity");
    for m in [MoveId::Ember, MoveId::PoisonSting, MoveId::BodySlam, MoveId::IceBeam,
              MoveId::ThunderWave, MoveId::Poisonpowder, MoveId::MegaDrain, MoveId::TakeDown] {
        assert!(move_effect_for(m).is_some(), "{m:?} resolves to a combined effect");
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// P3 — SPECIAL / FIXED / OHKO / SUPER FANG + FOE STAT-DOWN nested-veto cascade
//      (blueprint 15 §5 P3). The legacy ORACLE differs by family:
//
//   • SuperFang / SpecialDamage / OHKO have NO `execute_turn` wiring (production
//     `apply_move_effect` returns NoEffect; `calc_and_apply_damage` has no special
//     branch — an honest finding documented in stack_slice7). So the STANDALONE
//     `damage_effects::apply_super_fang` / `apply_special_damage` / `apply_ohko`
//     functions ARE the oracle (they take no rng). We diff the STACK's resulting
//     HP against the standalone oracle on a fresh legacy state, and assert the
//     stack's `consumed()` explicitly (accuracy [+ 1 Psywave byte]).
//   • Foe stat-down (primary AND side) IS wired in production via `apply_stat_down`
//     / `apply_stat_down_side` (the Mist + Substitute + −6 floor guards). So the
//     legacy `execute_move` per-mover path is the oracle, mirroring the P2 harness.
//     The side-variant draws its side_effect byte at the legacy ordinal even when
//     blocked by Mist/Substitute (the consumed() invariant), proven below.
// ═════════════════════════════════════════════════════════════════════════════

use super::{clear_levels, set_level, PokeVolatile as PV};
use crate::battle::effects::damage_effects::{apply_ohko, apply_special_damage, apply_super_fang};

/// Build a single-mover legacy state (player only acts) at a given level + HP.
fn p3_legacy_state(s: &Scenario) -> LegacyState {
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![poke(&s.player, s.move_id)],
        vec![poke(&s.enemy, s.move_id)],
    );
    state.whose_turn = Side::Player;
    state.player.selected_move = s.move_id;
    state.player.selected_move_index = 0;
    state.player.active_mon_mut().level = s.player.level as u8;
    state.enemy.active_mon_mut().level = s.enemy.level as u8;
    state
}

/// Run ONLY the player's special-damage move through the STACK (the enemy is set
/// to faint-on-reply so the second mover is cancelled — isolating the player's
/// special damage). Returns (state, consumed).
fn p3_special_stack(s: &Scenario, bytes: Vec<u8>) -> (EngineState<PokeredRules>, usize) {
    install_canonical();
    clear_levels();
    set_level(s.player.species, s.player.level);
    if s.enemy.species != s.player.species {
        set_level(s.enemy.species, s.enemy.level);
    }
    set_active_move(real_move(s.move_id));
    let mut state = EngineState::new(
        vec![engine_battler(&s.player, s.move_id)],
        vec![engine_battler(&s.enemy, s.move_id)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let provider = PokeredRules;
    // Only the player acts (the enemy "does nothing" — a Splash-shaped no-op move
    // would still draw; instead we run a SINGLE action by giving the enemy a move
    // that the driver will run second, but we faint it via the player's hit so the
    // reply is cancelled). For special damage we instead drive BOTH with the same
    // move and read the player→enemy result, keeping the enemy alive enough.
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
    ];
    let mut rng = ScriptedRng::new(bytes);
    StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
    (state, rng.consumed())
}

// ── extend Mon with a level field via a builder (defaults stay level 50) ──────

impl Mon {
    fn lvl(species: Species, hp: u16, speed: u16, level: u16) -> Self {
        let mut m = Mon::new(species, hp, speed);
        m.level = level;
        m
    }
}

/// SUPER FANG halves the target's CURRENT hp (real number). Oracle = the standalone
/// `apply_super_fang` on a fresh legacy state. The stack draws only the accuracy
/// byte (the formula is bypassed) — Super Fang has NO crit/damage/Psywave byte.
#[test]
fn super_fang_halves_current_hp() {
    let mut s = Scenario::base("Super Fang halves current hp", MoveId::SuperFang);
    s.player = Mon::lvl(Species::Pikachu, 200, 100, 50);
    s.enemy = Mon::lvl(Species::Snorlax, 264, 50, 50); // odd-ish current hp
    // Legacy oracle: apply_super_fang on a fresh state (player → enemy).
    let mut legacy = p3_legacy_state(&s);
    let result = apply_super_fang(&mut legacy);
    let legacy_dealt = 264 - legacy.enemy.active_mon().hp;
    assert_eq!(result, crate::battle::effects::EffectResult::SuperFangDamage { damage: 132 });
    assert_eq!(legacy_dealt, 132, "264 / 2 = 132 (real number)");
    // Stack: player Super Fang on the enemy. Both movers act; we read the enemy hp
    // after the player's hit (the enemy replies with Super Fang on the player too,
    // but we only assert the enemy's hp = the player→enemy Super Fang result).
    // Bytes: [player accuracy][enemy accuracy] (no crit/damage). Player faster.
    let (stack, consumed) = p3_special_stack(&s, vec![0, 0]);
    let stack_dealt = 264 - stack.opponent_battlers[0].hp;
    assert_eq!(stack_dealt, 132, "stack Super Fang halves the enemy's 264 → 132 dealt");
    assert_eq!(stack_dealt, legacy_dealt, "stack == standalone oracle");
    assert_eq!(consumed, 2, "two movers × accuracy only (formula bypassed)");
}

/// FIXED damage: Seismic Toss = the user's level (50). Oracle = standalone
/// `apply_special_damage`. Dragon Rage = 40 proven alongside.
#[test]
fn special_damage_seismic_toss_and_dragon_rage() {
    // Seismic Toss = user level (50).
    let mut s = Scenario::base("Seismic Toss = user level", MoveId::SeismicToss);
    s.player = Mon::lvl(Species::Machamp, 250, 100, 50);
    s.enemy = Mon::lvl(Species::Snorlax, 250, 50, 50);
    let mut legacy = p3_legacy_state(&s);
    let r = apply_special_damage(&mut legacy, MoveId::SeismicToss);
    assert_eq!(r, crate::battle::effects::EffectResult::SpecialDamageDealt { damage: 50 });
    assert_eq!(250 - legacy.enemy.active_mon().hp, 50, "Seismic Toss deals user level = 50");
    let (stack, consumed) = p3_special_stack(&s, vec![0, 0]);
    assert_eq!(250 - stack.opponent_battlers[0].hp, 50, "stack Seismic Toss deals 50 (user level)");
    assert_eq!(consumed, 2, "two movers × accuracy only");

    // Dragon Rage = fixed 40.
    let mut d = Scenario::base("Dragon Rage = 40", MoveId::DragonRage);
    d.player = Mon::lvl(Species::Dragonite, 300, 100, 50);
    d.enemy = Mon::lvl(Species::Snorlax, 300, 50, 50);
    let mut dleg = p3_legacy_state(&d);
    let dr = apply_special_damage(&mut dleg, MoveId::DragonRage);
    assert_eq!(dr, crate::battle::effects::EffectResult::SpecialDamageDealt { damage: 40 });
    let (dstack, _c) = p3_special_stack(&d, vec![0, 0]);
    assert_eq!(300 - dstack.opponent_battlers[0].hp, 40, "stack Dragon Rage deals fixed 40");
}

/// Psywave: rng·1.5·level/256. The stack draws ONE extra byte (the RngScaledLevel
/// byte) at the ModifyDamage ordinal. We pin both the value AND consumed().
#[test]
fn special_damage_psywave_draws_one_byte() {
    let mut s = Scenario::base("Psywave rng·1.5·lvl", MoveId::Psywave);
    s.player = Mon::lvl(Species::Alakazam, 300, 100, 50);
    s.enemy = Mon::lvl(Species::Snorlax, 300, 50, 50);
    // Player byte stream: [accuracy=0][psywave byte=255] then enemy [accuracy=0][psywave=255].
    // 255 * 3 / 2 = 382; 382 * 50 = 19100; 19100 / 256 = 74.
    let (stack, consumed) = p3_special_stack(&s, vec![0, 255, 0, 255]);
    assert_eq!(300 - stack.opponent_battlers[0].hp, 74, "Psywave: 255*3/2*50/256 = 74");
    assert_eq!(consumed, 4, "two movers × (accuracy + psywave byte)");
}

/// OHKO connects when user level ≥ target, and is IMMUNE when target level > user
/// (bug #19). Oracle = standalone `apply_ohko`. The stack uses distinct species so
/// the level map keys cleanly.
#[test]
fn ohko_level_gate_bug_19() {
    // CONNECTS: user (Machamp, level 50) ≥ foe (Snorlax, level 50). Equal connects.
    let mut connect = Scenario::base("OHKO connects (user >= foe)", MoveId::HornDrill);
    connect.player = Mon::lvl(Species::Machamp, 250, 100, 50);
    connect.enemy = Mon::lvl(Species::Snorlax, 250, 50, 50);
    let mut cleg = p3_legacy_state(&connect);
    let cr = apply_ohko(&mut cleg);
    assert_eq!(cr, crate::battle::effects::EffectResult::OhkoSuccess);
    assert_eq!(cleg.enemy.active_mon().hp, 0, "standalone OHKO connects (equal level)");
    let (cstack, cconsumed) = p3_special_stack(&connect, vec![0]);
    assert_eq!(cstack.opponent_battlers[0].hp, 0, "stack OHKO connects (user 50 >= foe 50)");
    // The player's OHKO faints the enemy ⇒ the reply is cancelled ⇒ only the
    // player's single accuracy byte is drawn (the faint short-circuit).
    assert_eq!(cconsumed, 1, "OHKO faints the foe ⇒ reply cancelled ⇒ 1 accuracy byte");

    // IMMUNE: user (Diglett, level 30) < foe (Snorlax, level 50). Bug #19.
    let mut immune = Scenario::base("OHKO immune (foe higher level)", MoveId::HornDrill);
    immune.player = Mon::lvl(Species::Diglett, 100, 100, 30);
    immune.enemy = Mon::lvl(Species::Snorlax, 250, 50, 50);
    let mut ileg = p3_legacy_state(&immune);
    let ir = apply_ohko(&mut ileg);
    assert_eq!(ir, crate::battle::effects::EffectResult::OhkoFailed);
    assert_eq!(ileg.enemy.active_mon().hp, 250, "standalone OHKO immune (foe higher level)");
    // Stack: the player's OHKO must NOT KO the higher-level enemy (the LevelGE gate
    // fails). The enemy stays at full hp (no SetHp applied).
    let (istack, _ic) = p3_special_stack(&immune, vec![0, 0]);
    assert_eq!(istack.opponent_battlers[0].hp, 250, "stack OHKO IMMUNE: foe level > user (bug #19)");
}

// ── FOE STAT-DOWN nested-veto: applies / Mist-veto / Substitute-veto ──────────

/// A symmetric foe-stat-down legacy runner via `execute_move` (the production
/// `apply_stat_down`/`apply_stat_down_side` oracle), optionally Mist/Substitute on
/// BOTH movers. Mirrors `p2_legacy` but with the foe-down setup.
fn p3_foedown_legacy(s: &Scenario, mist: bool, sub: bool) -> LegacyState {
    use crate::battle::move_execution::execute_move;
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![poke(&s.player, s.move_id)],
        vec![poke(&s.enemy, s.move_id)],
    );
    state.player.selected_move = s.move_id;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = s.move_id;
    state.enemy.selected_move_index = 0;
    if mist {
        state.player.set_status2(status2::PROTECTED_BY_MIST);
        state.enemy.set_status2(status2::PROTECTED_BY_MIST);
    }
    if sub {
        state.player.set_status2(status2::HAS_SUBSTITUTE_UP);
        state.player.substitute_hp = 50;
        state.enemy.set_status2(status2::HAS_SUBSTITUTE_UP);
        state.enemy.substitute_hp = 50;
    }
    let md = real_move(s.move_id);
    let (first_side, second_side, fb, sb) = match first_mover(s) {
        FirstMover::Player => (Side::Player, Side::Enemy, s.first, s.second),
        FirstMover::Opponent => (Side::Enemy, Side::Player, s.first, s.second),
    };
    state.whose_turn = first_side;
    execute_move(&mut state, &md, &to_move_randoms(fb));
    if state.side(second_side).active_mon().hp != 0 {
        state.whose_turn = second_side;
        execute_move(&mut state, &md, &to_move_randoms(sb));
    }
    state
}

/// The foe-stat-down STACK runner: optional Mist / Substitute volatiles on BOTH
/// movers (each fires the nested-veto cascade). Returns (state, consumed).
fn p3_foedown_stack(s: &Scenario, mist: bool, sub: bool) -> (EngineState<PokeredRules>, usize) {
    install_canonical();
    clear_levels();
    set_active_move(real_move(s.move_id));
    let mut state = EngineState::new(
        vec![engine_battler(&s.player, s.move_id)],
        vec![engine_battler(&s.enemy, s.move_id)],
    );
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let mut next_id = 300u32;
    let mut push_vol = |effects: &mut Vec<EffectState<PokeredRules>>, host, kind| {
        effects.push(EffectState { id: EffectId(next_id), host, effect_order: next_id as u64, kind });
        next_id += 1;
    };
    if mist {
        push_vol(&mut effects, BattlerRef::PLAYER, PV::Mist);
        push_vol(&mut effects, BattlerRef::OPPONENT, PV::Mist);
    }
    if sub {
        push_vol(&mut effects, BattlerRef::PLAYER, PV::Substitute);
        push_vol(&mut effects, BattlerRef::OPPONENT, PV::Substitute);
    }
    effects.sort_by(|a, b| a.id.cmp(&b.id));
    let provider = PokeredRules;
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
    ];
    let bytes = build_stream(s, first_mover(s), order_is_tie(s));
    let mut rng = ScriptedRng::new(bytes);
    StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
    (state, rng.consumed())
}

/// A primary foe-down (Growl, AttackDown1): both movers lower each other's Attack
/// by 1. Legacy `apply_stat_down` IS the production oracle; assert IDENTICAL stat
/// stages BOTH sides + consumed().
#[test]
fn foe_stat_down_applies() {
    let mut s = Scenario::base("Growl lowers foe Attack -1", MoveId::Growl);
    s.player = Mon::new(Species::Snorlax, 300, 100);
    s.enemy = Mon::new(Species::Snorlax, 300, 50);
    let legacy = p3_foedown_legacy(&s, false, false);
    let (stack, consumed) = p3_foedown_stack(&s, false, false);
    assert_eq!(legacy.enemy.stat_stages.attack, -1, "legacy: enemy Attack -1");
    assert_eq!(legacy.player.stat_stages.attack, -1, "legacy: player Attack -1");
    assert_eq!(
        stack.opponent_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
        legacy.enemy.stat_stages.attack, "enemy Attack stage parity"
    );
    assert_eq!(
        stack.player_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
        legacy.player.stat_stages.attack, "player Attack stage parity"
    );
    // power-0 primary foe-down ⇒ per mover only the accuracy byte. Two movers = 2.
    assert_eq!(consumed, 2, "two power-0 foe-down movers draw 2 accuracy bytes");
}

/// Mist VETO: with Mist on BOTH movers, the foe-down is vetoed by the nested
/// cascade (TryBoost → Mist handler Fails). NO stat change, both sides. The legacy
/// `apply_stat_down` Mist guard agrees.
#[test]
fn foe_stat_down_blocked_by_mist() {
    let mut s = Scenario::base("Growl blocked by Mist", MoveId::Growl);
    s.player = Mon::new(Species::Snorlax, 300, 100);
    s.enemy = Mon::new(Species::Snorlax, 300, 50);
    let legacy = p3_foedown_legacy(&s, true, false);
    let (stack, _c) = p3_foedown_stack(&s, true, false);
    assert_eq!(legacy.enemy.stat_stages.attack, 0, "legacy Mist blocks the drop (enemy)");
    assert_eq!(
        stack.opponent_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
        0, "stack: Mist VETOES the foe-down (enemy) — nested TryBoost cascade"
    );
    assert_eq!(
        stack.player_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
        0, "stack: Mist VETOES the foe-down (player)"
    );
    // Control: WITHOUT Mist the same Growl DOES lower Attack.
    let (open, _c2) = p3_foedown_stack(&s, false, false);
    assert_eq!(
        open.opponent_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
        -1, "no Mist ⇒ the drop lands"
    );
}

/// Substitute VETO: with a Substitute on BOTH movers, the foe-down is absorbed by
/// the nested cascade (TryBoost → Substitute handler Fails). Legacy agrees.
#[test]
fn foe_stat_down_blocked_by_substitute() {
    let mut s = Scenario::base("Growl blocked by Substitute", MoveId::Growl);
    s.player = Mon::new(Species::Snorlax, 300, 100);
    s.enemy = Mon::new(Species::Snorlax, 300, 50);
    let legacy = p3_foedown_legacy(&s, false, true);
    let (stack, _c) = p3_foedown_stack(&s, false, true);
    assert_eq!(legacy.enemy.stat_stages.attack, 0, "legacy Substitute blocks the drop (enemy)");
    assert_eq!(
        stack.opponent_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
        0, "stack: Substitute ABSORBS the foe-down (enemy) — nested TryBoost cascade"
    );
    assert_eq!(
        stack.player_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
        0, "stack: Substitute ABSORBS the foe-down (player)"
    );
}

/// SIDE foe-down (Aurora Beam, AttackDownSide 85/256): the side_effect byte is
/// drawn at the legacy ordinal EVEN WHEN blocked by Mist (consumed() invariant).
/// FIRES at byte 84 (< 85), and is still drawn (but vetoed) under Mist.
#[test]
fn foe_stat_down_side_byte_drawn_even_when_vetoed() {
    let mut s = Scenario::base("Aurora Beam -Atk side (85/256) under Mist", MoveId::AuroraBeam);
    s.player = Mon::new(Species::Snorlax, 400, 100);
    s.enemy = Mon::new(Species::Snorlax, 400, 50);
    s.first.side_effect = 84;  // < 85 ⇒ the secondary rolls
    s.second.side_effect = 84;
    // Under Mist: the drop is vetoed, but the side_effect byte is still drawn.
    let (mist_stack, mist_consumed) = p3_foedown_stack(&s, true, false);
    assert_eq!(
        mist_stack.opponent_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
        0, "Mist vetoes the side foe-down"
    );
    // WITHOUT Mist: the same byte 84 FIRES the drop.
    let (open_stack, open_consumed) = p3_foedown_stack(&s, false, false);
    assert_eq!(
        open_stack.opponent_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
        -1, "no Mist ⇒ the side foe-down lands at byte 84"
    );
    // consumed() IDENTICAL whether vetoed or not — the chance byte is drawn either
    // way (the side_effect-roll-at-legacy-ordinal invariant).
    assert_eq!(mist_consumed, open_consumed,
        "side_effect byte drawn at the legacy ordinal even when Mist vetoes");
    // Each mover draws crit + accuracy + damage + side_effect = 4; two movers = 8.
    assert_eq!(open_consumed, 8, "two movers × (crit, acc, damage, side_effect)");
}

/// The −6 floor (bug #30, the down-clamp): a foe already at −6 Attack cannot drop
/// further. Legacy `apply_stat_down` returns StatBlocked at the floor; the stack's
/// `apply_boost` clamp matches (the drop is a no-op, no veto needed).
#[test]
fn foe_stat_down_floor_at_minus_6() {
    let mut s = Scenario::base("Growl at -6 floor", MoveId::Growl);
    s.player = Mon::new(Species::Snorlax, 300, 100);
    s.enemy = Mon::new(Species::Snorlax, 300, 50);
    // Stack: pre-set the enemy's Attack stage to -6; Growl from the player cannot
    // lower further. (Symmetric: both at -6.)
    install_canonical();
    clear_levels();
    set_active_move(real_move(s.move_id));
    let mut pb = engine_battler(&s.player, s.move_id);
    pb.stat_stages.set(StatIndex::Attack, -6);
    let mut eb = engine_battler(&s.enemy, s.move_id);
    eb.stat_stages.set(StatIndex::Attack, -6);
    let mut state = EngineState::new(vec![pb], vec![eb]);
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let provider = PokeredRules;
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
    ];
    let mut rng = ScriptedRng::new(build_stream(&s, first_mover(&s), order_is_tie(&s)));
    StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
    assert_eq!(
        state.opponent_battlers[0].stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
        -6, "the −6 floor holds (no further drop)"
    );
}

/// The data sanity witness: the P3 special-damage + foe-down effects are genuinely
/// DATA (the authored op-lists), not native code.
#[test]
fn p3_authored_as_data() {
    install_canonical();
    use dotzuki_rules::{DamageValue, Op, Predicate, Selector};
    // Special damage: SetDamage(UserLevel) / Const(40) / RngScaledLevel; SuperFang;
    // OHKO SetHp+LevelGE.
    assert!(super::record_has_op("special.user_level", &Op::SetDamage {
        value: DamageValue::UserLevel, of: Selector::Source
    }), "seismic toss is SetDamage(UserLevel)");
    assert!(super::record_has_op("special.const_40", &Op::SetDamage {
        value: DamageValue::Const(40), of: Selector::Source
    }), "dragon rage is SetDamage(Const(40))");
    assert!(super::record_has_op("special.psywave", &Op::SetDamage {
        value: DamageValue::RngScaledLevel { num: 3, den: 2 }, of: Selector::Source
    }), "psywave is SetDamage(RngScaledLevel)");
    assert!(super::record_has_op("special.super_fang", &Op::DamageCurrentHpFraction {
        num: 1, den: 2, target: Selector::Target
    }), "super fang is DamageCurrentHpFraction(1/2, Target)");
    assert!(super::record_has_op("special.ohko", &Op::SetHp {
        target: Selector::Target, value: 0, when: vec![Predicate::LevelGE]
    }), "ohko is SetHp(Target, 0, when:[LevelGE])");
    // Foe stat-down: Boost(stat, -N, Target) routed through the nested-veto driver.
    assert!(super::record_has_op("foedown.attack_1", &Op::Boost {
        stat: "Attack".into(), stages: -1, target: Selector::Target
    }), "growl is a -1 Attack Boost on Target (foe-down)");
    assert!(super::record_has_op("foedown.defense_2", &Op::Boost {
        stat: "Defense".into(), stages: -2, target: Selector::Target
    }), "screech is a -2 Defense Boost on Target");
    for m in [MoveId::SuperFang, MoveId::SeismicToss, MoveId::DragonRage, MoveId::Psywave,
              MoveId::HornDrill, MoveId::Growl, MoveId::Screech, MoveId::AuroraBeam] {
        assert!(move_effect_for(m).is_some(), "{m:?} resolves to a combined effect");
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// P4 — MULTI-HIT via the RepeatHits seam (blueprint 15 §5 P4). The RepeatHits
// mechanism is a GAME-SIDE `dotzuki_rules::Op` (NO engine change): the StackDriver
// computes the per-hit damage once into `ctx.mv.damage`, checks accuracy once,
// deals the FIRST hit, then fires `DamagingHit`, where the `RepeatHits` op
// re-deals the SAME number `(N-1)` more times. So total = per-hit × N.
//
// The legacy ORACLE for multi-hit's TOTAL damage is, like P3 special-damage,
// NOT the atomic `execute_turn` (which calls `apply_two_to_five`/etc. — those set
// only the bookkeeping fields num_attacks_left/num_hits + the MULTI_HIT status +
// Twineedle's poison; they do NOT loop the damage — the frame-stepped production
// loop does, and that is not the oracle). So the per-hit oracle is a SINGLE legacy
// `execute_move` (which deals exactly ONE hit via the shared `calculate_damage`
// authority), and the total = per-hit × N where N is the legacy `determine_hit_count`
// (multi_hit_effects.rs) for TwoToFive or the fixed 2 for Double Kick / Twineedle.
// The stack draws ONE multi_hit byte (TwoToFive) + Twineedle's ONE side byte at the
// legacy ordinals; consumed() is asserted explicitly.
// ═════════════════════════════════════════════════════════════════════════════

/// Run ONE legacy `execute_move` (player only) and return the per-hit damage dealt
/// to the enemy — the per-hit oracle (the shared `calculate_damage` authority deals
/// exactly one hit; the legacy multi-hit fn only sets bookkeeping, not extra damage).
fn legacy_per_hit(s: &Scenario) -> u16 {
    let mut legacy = new_battle_state(
        BattleType::Wild, vec![poke(&s.player, s.move_id)], vec![poke(&s.enemy, s.move_id)]);
    legacy.whose_turn = Side::Player;
    legacy.player.selected_move = s.move_id;
    legacy.player.selected_move_index = 0;
    let before = legacy.enemy.active_mon().hp;
    crate::battle::move_execution::execute_move(&mut legacy, &real_move(s.move_id), &to_move_randoms(s.first));
    before - legacy.enemy.active_mon().hp
}

/// Run the player's multi-hit through the STACK (both movers act with the same
/// move; the enemy survives so the player→enemy total is readable). Returns
/// (player→enemy total dealt, consumed, enemy status).
fn p4_stack(s: &Scenario) -> (u16, usize, Option<LegacyStatus>) {
    install_canonical();
    set_active_move(real_move(s.move_id));
    let mut state = EngineState::new(
        vec![engine_battler(&s.player, s.move_id)],
        vec![engine_battler(&s.enemy, s.move_id)],
    );
    let before_enemy = state.opponent_battlers[0].hp;
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let provider = PokeredRules;
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
        BattleAction::<PokeredRules>::Fight { move_: s.move_id },
    ];
    let bytes = build_stream(s, first_mover(s), order_is_tie(s));
    let mut rng = ScriptedRng::new(bytes);
    StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
    let dealt = before_enemy - state.opponent_battlers[0].hp;
    (dealt, rng.consumed(), state.opponent_battlers[0].status)
}

/// A multi-hit base scenario: player faster + a big-HP enemy that survives all N
/// hits so the total is observable. Non-Poison movers (Snorlax) so Twineedle's
/// poison rider is not type-blocked unless a test chooses Poison defenders.
fn multi_base(name: &'static str, move_id: MoveId) -> Scenario {
    let mut s = Scenario::base(name, move_id);
    s.player = Mon::new(Species::Snorlax, 700, 100);
    s.enemy = Mon::new(Species::Snorlax, 700, 50);
    s
}

/// TWO-TO-FIVE, LOW roll = 2 hits: byte 50 (< 96) ⇒ 2 hits. Real total = per-hit×2.
#[test]
fn multi_hit_two_to_five_low_roll_2_hits() {
    let mut s = multi_base("Doubleslap 2-5 LOW roll = 2 hits", MoveId::Doubleslap);
    s.first.multi_hit = 50;  // < 96 ⇒ 2 hits
    s.second.multi_hit = 50;
    assert_eq!(determine_hit_count(50), 2, "byte 50 ⇒ 2 hits");
    let per_hit = legacy_per_hit(&s);
    let (dealt, consumed, _st) = p4_stack(&s);
    assert_eq!(dealt, per_hit * 2, "2-hit total = per-hit × 2 ({} × 2)", per_hit);
    // Concrete real Gen-1 number: Doubleslap (pow 15) Snorlax→Snorlax per-hit, ×2.
    assert_eq!((per_hit, dealt), (15, 30), "real Gen-1: Doubleslap per-hit 15, 2 hits = 30");
    // Per mover: crit, accuracy, damage, multi_hit count = 4; two movers = 8.
    assert_eq!(consumed, 8, "two movers × (crit, acc, damage, multi_hit) = 8");
}

/// TWO-TO-FIVE, HIGH roll = 5 hits: byte 240 (>= 224) ⇒ 5 hits. Total = per-hit×5.
#[test]
fn multi_hit_two_to_five_high_roll_5_hits() {
    let mut s = multi_base("Doubleslap 2-5 HIGH roll = 5 hits", MoveId::Doubleslap);
    s.first.multi_hit = 240; // >= 224 ⇒ 5 hits
    s.second.multi_hit = 240;
    assert_eq!(determine_hit_count(240), 5, "byte 240 ⇒ 5 hits");
    let per_hit = legacy_per_hit(&s);
    let (dealt, consumed, _st) = p4_stack(&s);
    assert_eq!(dealt, per_hit * 5, "5-hit total = per-hit × 5 ({} × 5)", per_hit);
    assert_eq!((per_hit, dealt), (15, 75), "real Gen-1: Doubleslap per-hit 15, 5 hits = 75");
    assert_eq!(consumed, 8, "two movers × (crit, acc, damage, multi_hit) = 8");
}

/// The full 2-5 distribution boundaries (the engine `determine_hit_count` parity):
/// the stack's hit count tracks the legacy distribution exactly across the cutoffs.
#[test]
fn multi_hit_two_to_five_distribution_boundaries() {
    for (byte, want_n) in [(0u8, 2u8), (95, 2), (96, 3), (191, 3), (192, 4), (223, 4), (224, 5), (255, 5)] {
        let mut s = multi_base("Fury Attack distribution", MoveId::FuryAttack);
        s.first.multi_hit = byte;
        s.second.multi_hit = byte;
        assert_eq!(determine_hit_count(byte), want_n, "byte {byte} ⇒ {want_n} hits");
        let per_hit = legacy_per_hit(&s);
        let (dealt, _c, _st) = p4_stack(&s);
        assert_eq!(dealt, per_hit * want_n as u16,
            "byte {byte}: total = per-hit({per_hit}) × {want_n}");
    }
}

/// DOUBLE KICK = exactly 2 hits (AttackTwiceEffect, Fixed(2)) — NO count byte.
/// Total = per-hit × 2; per mover draws crit + acc + damage = 3 (no multi_hit byte).
#[test]
fn multi_hit_double_kick_exactly_2() {
    let mut s = multi_base("Double Kick exactly 2 hits", MoveId::DoubleKick);
    // Machamp (Fighting STAB) into Snorlax (Normal): Fighting 2× super-effective.
    s.player = Mon::new(Species::Machamp, 700, 100);
    s.enemy = Mon::new(Species::Snorlax, 700, 50);
    let per_hit = legacy_per_hit(&s);
    let (dealt, consumed, _st) = p4_stack(&s);
    assert_eq!(dealt, per_hit * 2, "Double Kick total = per-hit × 2 ({} × 2)", per_hit);
    assert!(per_hit > 0, "Double Kick deals real damage per hit");
    // Concrete real Gen-1 number (Fighting STAB ×1.5 × super-effective ×2 vs Normal).
    assert_eq!((per_hit, dealt), (54, 108), "real Gen-1: Double Kick Machamp→Snorlax per-hit 54, ×2 = 108");
    // Fixed(2) draws NO count byte ⇒ per mover crit + acc + damage = 3; two = 6.
    assert_eq!(consumed, 6, "Fixed-count multi-hit draws no count byte (2 movers × 3 = 6)");
}

/// BONEMERANG = exactly 2 hits (AttackTwiceEffect, Fixed(2)).
#[test]
fn multi_hit_bonemerang_exactly_2() {
    let mut s = multi_base("Bonemerang exactly 2 hits", MoveId::Bonemerang);
    s.player = Mon::new(Species::Marowak, 700, 100); // Ground STAB
    s.enemy = Mon::new(Species::Snorlax, 700, 50);
    let per_hit = legacy_per_hit(&s);
    let (dealt, _c, _st) = p4_stack(&s);
    assert_eq!(dealt, per_hit * 2, "Bonemerang total = per-hit × 2");
}

/// TWINEEDLE = 2 hits AND poison on the 2nd hit at the legacy 20%+1 (52/256)
/// ordinal. Byte 51 (< 52) ⇒ poison FIRES on the final hit; byte 52 (!< 52) ⇒ no
/// poison. The side byte is drawn at the side_effect ordinal (after the count —
/// Twineedle is Fixed(2) so no count byte) — consumed() invariant either way.
#[test]
fn multi_hit_twineedle_two_hits_and_poison() {
    // FIRES: side byte 51 < 52 ⇒ poison on the 2nd hit. Snorlax (non-Poison) victim.
    let mut fires = multi_base("Twineedle 2 hits + poison FIRES (51/256)", MoveId::Twineedle);
    fires.first.side_effect = 51;  // < 52 ⇒ poison fires on the final hit
    fires.second.side_effect = 51;
    let per_hit = legacy_per_hit(&fires);
    let (dealt, consumed, status) = p4_stack(&fires);
    // P6b-prereq: the poisoned victim (the enemy, poisoned by the player's final
    // hit) now ALSO takes its `(max/16).max(1)` poison residual chip at the enemy's
    // residual step — so the enemy's full-turn hp loss is the 2-hit damage PLUS the
    // chip. (A nice integration check that multi-hit + final-hit poison + residual
    // compose correctly through the driver.)
    let chip = (fires.enemy.hp / 16).max(1);
    assert_eq!(dealt, per_hit * 2 + chip, "Twineedle 2 hits (per-hit × 2) + poison residual chip");
    assert_eq!(status, Some(LegacyStatus::Poison), "byte 51 < 52 ⇒ poison on the final hit");
    // Per mover: crit, acc, damage, side_effect(poison) = 4 (NO count byte, Fixed 2);
    // two movers = 8. Residual draws NO bytes, so consumed() is unchanged.
    assert_eq!(consumed, 8, "Twineedle: two movers × (crit, acc, damage, poison byte) = 8");
    // Concrete real Gen-1: Twineedle (Bug pow 25) Snorlax→Snorlax: per-hit 15, ×2 = 30,
    // + the 700/16 = 43 poison chip = 73 total hp lost.
    assert_eq!((per_hit, per_hit * 2), (15, 30), "real Gen-1: Twineedle per-hit 15, ×2 = 30");
    assert_eq!(dealt, 73, "30 move + 43 poison chip (Snorlax max 700)");

    // NO-FIRE: side byte 52 (!< 52) ⇒ no poison, but the byte is STILL drawn.
    let mut no_fire = multi_base("Twineedle poison NO-FIRE (byte 52)", MoveId::Twineedle);
    no_fire.first.side_effect = 52;
    no_fire.second.side_effect = 52;
    let (dealt2, consumed2, status2) = p4_stack(&no_fire);
    assert_eq!(dealt2, per_hit * 2, "still 2 hits when poison doesn't fire");
    assert_eq!(status2, None, "byte 52 ⇒ no poison");
    assert_eq!(consumed, consumed2,
        "consumed() identical whether the final-hit poison fires (51) or not (52)");
}

/// Twineedle's poison rider reuses the P2 side-status guards: a Poison-type victim
/// is IMMUNE even when the byte fires (the `VetoIf(HasType("Poison"))` guard).
#[test]
fn multi_hit_twineedle_poison_type_immune() {
    let mut s = multi_base("Twineedle can't poison a Poison-type", MoveId::Twineedle);
    s.player = Mon::new(Species::Nidoking, 700, 100); // Poison/Ground
    s.enemy = Mon::new(Species::Nidoking, 700, 50);   // Poison/Ground → immune
    s.first.side_effect = 0;  // poison would fire
    s.second.side_effect = 0;
    let (_dealt, _c, status) = p4_stack(&s);
    assert_eq!(status, None, "Poison-type immune to Twineedle's poison (reused #VetoIf guard)");
}

/// The data sanity witness: all 10 multi-hit moves are genuinely DATA (the authored
/// RepeatHits op-lists), not native code. NONE deferred — Twineedle's poison is the
/// composed `final_hit: OnFinal { chance, ops }` rider.
#[test]
fn p4_authored_as_data() {
    install_canonical();
    use dotzuki_rules::{FinalHitRider, HitCount, Op, Predicate, Rational, Selector};
    // The 7 TwoToFive moves carry RepeatHits(TwoToFive, Target).
    for sid in ["multi.doubleslap", "multi.comet_punch", "multi.fury_attack",
                "multi.pin_missile", "multi.spike_cannon", "multi.barrage", "multi.fury_swipes"] {
        assert!(super::record_has_op(sid, &Op::RepeatHits {
            count: HitCount::TwoToFive, target: Selector::Target, final_hit: FinalHitRider::None
        }), "{sid} is a TwoToFive RepeatHits");
    }
    // Double Kick + Bonemerang carry RepeatHits(Fixed(2), Target).
    for sid in ["multi.double_kick", "multi.bonemerang"] {
        assert!(super::record_has_op(sid, &Op::RepeatHits {
            count: HitCount::Fixed(2), target: Selector::Target, final_hit: FinalHitRider::None
        }), "{sid} is a Fixed(2) RepeatHits");
    }
    // Twineedle carries RepeatHits(Fixed(2)) + the OnFinal poison rider (the COMPOSED
    // P2 side-status machinery: poison-type immunity + Substitute block + InflictStatus).
    assert!(super::record_has_op("multi.twineedle", &Op::RepeatHits {
        count: HitCount::Fixed(2),
        target: Selector::Target,
        final_hit: FinalHitRider::OnFinal {
            chance: Rational { num: 52, den: 256 },
            ops: vec![
                Op::VetoIf { cond: Predicate::HasType("Poison".into()), silent: false },
                Op::VetoIf { cond: Predicate::HasVolatile("Substitute".into()), silent: false },
                Op::InflictStatus { status: "poison".into(), target: Selector::Target, amount: Default::default() },
            ],
        },
    }), "twineedle is Fixed(2) RepeatHits + the OnFinal 52/256 poison rider (NOT deferred)");
    for m in [MoveId::Doubleslap, MoveId::CometPunch, MoveId::FuryAttack, MoveId::PinMissile,
              MoveId::SpikeCannon, MoveId::Barrage, MoveId::FurySwipes,
              MoveId::DoubleKick, MoveId::Bonemerang, MoveId::Twineedle] {
        assert!(move_effect_for(m).is_some(), "{m:?} resolves to a combined effect");
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// DEMO — a full battle played ENTIRELY on the new stack engine (P6 flip preview).
// Two different species, different moves, real levels, leveled stats — driven turn
// by turn through StackDriver::execute_turn_logged + the production-capable
// PokeredRules provider, narrated by translate_turn. Deterministic (LCG rng) so the
// transcript is reproducible. This is the "test the new logic" entry point:
//   cargo test -p pokered-core --lib demo_stack_battle_transcript -- --nocapture
// ═════════════════════════════════════════════════════════════════════════════

/// A deterministic LCG `BattleRng` so the demo battle is reproducible (and varied).
struct DemoRng(u64);
impl dotzuki_engine::battle::rng::BattleRng for DemoRng {
    fn next_u8(&mut self) -> u8 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u8
    }
}

/// Build an engine battler from a freshly-created (leveled, stat-computed) legacy
/// Pokémon — the fresh-mon adapter (no mid-battle volatiles; the stack maintains
/// the arena across turns itself).
fn engine_from_pokemon(p: &Pokemon) -> EngineBattler<PokeredRules> {
    let mut stats = EnumMap::new();
    stats.set(StatIndex::Attack, p.attack);
    stats.set(StatIndex::Defense, p.defense);
    stats.set(StatIndex::Speed, p.speed);
    stats.set(StatIndex::Special, p.special);
    let moves: Vec<MoveId> = p.moves.iter().copied().filter(|m| *m != MoveId::None).collect();
    EngineBattler::new(p.species, p.hp, p.max_hp, stats, moves).with_level(p.level)
}

#[test]
fn demo_stack_battle_transcript() {
    use dotzuki_engine::battle::rng::BattleRng; // for rng.next_u8() at the call site
    install_canonical();
    super::clear_current_moves();
    let dvs = [0xFF, 0xFF];
    let p = crate::pokemon::stats::create_pokemon_with_moves(
        Species::Charizard, 40, dvs,
        [MoveId::Flamethrower, MoveId::Earthquake, MoveId::Slash, MoveId::Swift],
    ).expect("player mon");
    let e = crate::pokemon::stats::create_pokemon_with_moves(
        Species::Blastoise, 40, dvs,
        [MoveId::HydroPump, MoveId::IceBeam, MoveId::BodySlam, MoveId::Bite],
    ).expect("enemy mon");
    let (pmoves, emoves) = (p.moves, e.moves);
    let mut state = EngineState::new(vec![engine_from_pokemon(&p)], vec![engine_from_pokemon(&e)]);
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let mut rng = DemoRng(0x00C0_FFEE_1234_5678);

    println!("\n=== STACK-ENGINE BATTLE: L{} {:?} vs L{} {:?} ===", p.level, p.species, e.level, e.species);
    for turn in 1..=16u32 {
        if state.player_battlers[0].hp == 0 || state.opponent_battlers[0].hp == 0 {
            break;
        }
        let pmv = pmoves[((turn as usize) - 1) % 4];
        let emv = emoves[(rng.next_u8() as usize) % 4];
        let pmd = *MoveData::get(pmv).unwrap();
        let emd = *MoveData::get(emv).unwrap();
        // Production per-mover move (different moves each side — the slice-3 seam).
        super::set_current_move(BattlerRef::PLAYER, pmd);
        super::set_current_move(BattlerRef::OPPONENT, emd);
        let actions = [
            BattleAction::<PokeredRules>::Fight { move_: pmv },
            BattleAction::<PokeredRules>::Fight { move_: emv },
        ];
        let (_r, log) =
            StackDriver::execute_turn_logged(&PokeredRules, &mut state, &mut effects, actions, &mut rng);
        println!("--- Turn {turn} ---");
        for line in translate_turn(&log, &state) {
            println!("    {line}");
        }
        println!(
            "    [you {}/{} hp | foe {}/{} hp]",
            state.player_battlers[0].hp, state.player_battlers[0].max_hp,
            state.opponent_battlers[0].hp, state.opponent_battlers[0].max_hp,
        );
    }
    let outcome = if state.opponent_battlers[0].hp == 0 {
        "YOU WIN"
    } else if state.player_battlers[0].hp == 0 {
        "YOU LOSE"
    } else {
        "(turn limit)"
    };
    println!("=== {outcome} ===\n");
    // Sanity (not the point — the transcript is): the battle actually progressed.
    assert!(
        state.player_battlers[0].hp < state.player_battlers[0].max_hp
            || state.opponent_battlers[0].hp < state.opponent_battlers[0].max_hp,
        "the stack-driven battle dealt damage"
    );
}

/// Haze (haze.asm) cures the target's non-volatile status in Gen 1, but the
/// ONLY text it prints is StatusChangesEliminatedText — never CheckDefrost's
/// "Fire defrosted <TARGET>!" line (text_3.asm:91), and no per-stat lines
/// for the stage reset.
#[test]
fn haze_cure_narrates_eliminated_text_not_defrost() {
    install_canonical();
    clear_current_moves();
    set_current_move(BattlerRef::PLAYER, real_move(MoveId::Haze));
    set_current_move(BattlerRef::OPPONENT, real_move(MoveId::Splash));
    let mut state = EngineState::new(
        vec![engine_battler(&Mon::new(Species::Butterfree, 1000, 200), MoveId::Haze)],
        vec![engine_battler(&Mon::new(Species::Snorlax, 1000, 50), MoveId::Splash)],
    );
    // Frozen + boosted stages: Haze cures the freeze and resets the stages.
    state.opponent_battlers[0].status = Some(LegacyStatus::Freeze);
    state.opponent_battlers[0].stat_stages.set(StatIndex::Attack, 2);
    let mut effects: Vec<EffectState<PokeredRules>> = Vec::new();
    let actions = [
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Haze },
        BattleAction::<PokeredRules>::Fight { move_: MoveId::Splash },
    ];
    let mut rng = ScriptedRng::new(vec![0u8; 64]);
    let (_r, log) = StackDriver::execute_turn_logged(
        &PokeredRules, &mut state, &mut effects, actions, &mut rng,
    );
    assert_eq!(
        state.opponent_battlers[0].status, None,
        "Haze cures the frozen target (haze.asm .cureStatuses)"
    );
    assert_eq!(
        state.opponent_battlers[0]
            .stat_stages
            .get(StatIndex::Attack)
            .copied()
            .unwrap_or(0),
        0,
        "Haze resets stat stages"
    );
    let text = super::runtime::translate_turn(&log, &state, &effects);
    let joined = text.join("\n");
    assert!(
        joined.contains("All STATUS changes\nare eliminated!"),
        "Haze prints StatusChangesEliminatedText: {joined}"
    );
    assert!(
        !joined.contains("Fire defrosted"),
        "Haze's cure must NOT reuse CheckDefrost's defrost line: {joined}"
    );
    assert!(
        !joined.contains("ATTACK fell"),
        "no per-stat lines for Haze's stage reset: {joined}"
    );
}
