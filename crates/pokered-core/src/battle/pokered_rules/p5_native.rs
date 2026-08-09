//! # P5 — the NATIVE TIER (multi-turn / field / volatile / data-reach) as
//! game-side native `Event::Custom` handlers + `EffectStateKind` state (blueprint
//! `15` §5 P5; design `06` §6 bug catalog #1/#6/#14-18/#28).
//!
//! ## What P5 is (and is NOT)
//!
//! P5 is **ADDITIVE and DIFFERENTIAL-ONLY**. The legacy
//! [`field_effects`](crate::battle::effects::field_effects) /
//! [`special_effects`](crate::battle::effects::special_effects) /
//! [`multi_turn_effects`](crate::battle::effects::multi_turn_effects) `apply_*`
//! functions stay **THE ORACLE**, untouched; the production loop is untouched
//! (the swap is P6). This module adds **game-side native handlers** that
//! reproduce each effect's legacy semantics — including its specific Gen-1 bug —
//! operating on the engine [`BattleState`](EngineState) + the
//! [`PokeVolatile`](super::PokeVolatile) arena, and the differential tests
//! (`tests.rs`, the `p5_*` block) assert the native handler produces a state
//! IDENTICAL to the legacy `apply_*` oracle's, field by field.
//!
//! ## The engine stays 100% game-agnostic
//!
//! Every P5 native handler fires on a game-defined [`Event::Custom`] code — the
//! engine dispatches it like any event (collect → sort → fold) but assigns it NO
//! meaning. The [`PokeVolatile`] variants are opaque to the engine (it only
//! stamps `effect_order` and routes the arena entry to its host). The ONE
//! permitted engine touch is the additive, defaulted [`Event::OnMiss`] seam (for
//! the JumpKick crash, blueprint §3) — inert for every game that does not
//! subscribe.
//!
//! ## The Gen-1 bugs each handler preserves
//!
//! | Effect | Handler | Gen-1 bug preserved |
//! |---|---|---|
//! | Focus Energy | [`focus_energy_set`] | #1 — sets the `/4` crit volatile (the crit pipeline divides) |
//! | Toxic | [`toxic_set`] + [`toxic_residual`] | #6 — UNCAPPED counter ramp (`/16·counter`, no cap) |
//! | Substitute | [`substitute_create`] | #28 — `HP == cost` succeeds, leaving the user at 0 HP |
//! | Mist / LightScreen / Reflect | set-once flags | screen damage halving / Mist veto (reuses the P3 driver) |
//! | Leech Seed | [`leech_seed_set`] + [`leech_residual`] | residual drain-to-source |
//! | Haze | [`haze_reset`] | resets ALL stages + volatiles + status, both sides |
//! | Flinch / Confusion | [`flinch_set`] / [`confusion_set`] | the `BeforeMove` gate volatiles |
//! | Disable | [`disable_set`] | move-slot disable + countdown |
//! | PayDay | [`pay_day`] | session coin pool award (`level·2`) |
//! | Conversion / Transform / Mimic / Metronome / Mirror Move | the data-reach 5 | arbitrary logic over species / the move table |
//! | JumpKick crash | [`jump_kick_crash`] (on `Event::OnMiss`) | crash 1 HP on a miss (the on:Miss seam) |
//!
//! The cross-turn LOCK-IN multi-turn group (Charge/Fly/Trapping/Thrash/HyperBeam/
//! Bide) + the reactive Counter/Rage are **already POC-proven** in the
//! `stack_parity` slice 6 (the `forced_action` seam + the `EffectState` arena);
//! re-authoring them in this differential harness is deferred to **P5b** (see the
//! returned findings) — the lowest-value, highest-coupling part, already green.

// PRODUCTIONIZED (P6 flip): the native handlers compile in non-test builds. Test
// scratch helpers (reset_p5_scratch, set_last_move, …) become unused in production
// → covered by allow(dead_code).
#![allow(dead_code)]

use jrpg_engine::battle::stack::{
    BattleCtx, Effect, EffectId, EffectState, EffectType, Event, EventHook, HandlerResult, RelayVar,
};
use jrpg_engine::battle::BattlerRef;

use pokered_data::pokemon_data::get_base_stats;
use pokered_data::types::PokemonType;

use crate::battle::state::StatusCondition;

use super::{PokeVolatile, PokeredRules};

// ─────────────────────────────────────────────────────────────────────────────
// 0. The game-defined Event::Custom codes for the P5 native handlers. The engine
//    assigns these NO meaning; the harness fires the move's effect for the
//    matching custom event and the registered handler does the work. Distinct
//    codes keep each effect's hook addressable without an engine enum change.
// ─────────────────────────────────────────────────────────────────────────────

/// Mist set (legacy `apply_mist`).
pub const EV_MIST: u16 = 0x500;
/// Focus Energy set (legacy `apply_focus_energy`, bug #1).
pub const EV_FOCUS_ENERGY: u16 = 0x501;
/// Light Screen set (legacy `apply_light_screen`).
pub const EV_LIGHT_SCREEN: u16 = 0x502;
/// Reflect set (legacy `apply_reflect`).
pub const EV_REFLECT: u16 = 0x503;
/// Leech Seed set (legacy `apply_leech_seed`).
pub const EV_LEECH_SEED: u16 = 0x504;
/// Haze reset (legacy `apply_haze`).
pub const EV_HAZE: u16 = 0x505;
/// Substitute create (legacy `apply_substitute`, bug #28).
pub const EV_SUBSTITUTE: u16 = 0x506;
/// Conversion (data-reach; legacy `apply_conversion`).
pub const EV_CONVERSION: u16 = 0x507;
/// Rest (full-heal + self-sleep + cure; legacy `apply_heal` Rest branch).
pub const EV_REST: u16 = 0x508;
/// Flinch set (legacy `apply_flinch_side`).
pub const EV_FLINCH: u16 = 0x509;
/// Confusion set (legacy `apply_confusion_primary`).
pub const EV_CONFUSION: u16 = 0x50A;
/// Disable set (legacy `apply_disable`).
pub const EV_DISABLE: u16 = 0x50B;
/// Pay Day (session coin pool; legacy `apply_pay_day`).
pub const EV_PAY_DAY: u16 = 0x50C;
/// Toxic set (badly-poisoned; legacy `PoisonEffect` Toxic branch, bug #6).
pub const EV_TOXIC: u16 = 0x50D;
/// Transform (data-reach; legacy `apply_transform`).
pub const EV_TRANSFORM: u16 = 0x50E;
/// Mimic (data-reach; legacy `apply_mimic`).
pub const EV_MIMIC: u16 = 0x50F;
/// Metronome (data-reach; legacy `apply_metronome`).
pub const EV_METRONOME: u16 = 0x510;
/// Mirror Move (data-reach; legacy `apply_mirror_move`).
pub const EV_MIRROR_MOVE: u16 = 0x511;

/// Distinct `EffectId`s for the P5 native move effects (clear of the data /
/// move-effect / veto id spaces, which top out around `0x40_002`).
const P5_ID_BASE: u32 = 0x50_000;

// ─────────────────────────────────────────────────────────────────────────────
// 1. Per-battler scratch the data-reach handlers + the harness share (the same
//    thread-local shape the harness uses for level): a TYPE OVERRIDE (Conversion/
//    Transform change a mon's types, which the engine derives from species), the
//    DISABLE slot, the COIN POOL (PayDay session state), the LAST-MOVE-USED (the
//    data-reach Mimic/MirrorMove/Disable read it), and the MIMIC slot. The engine
//    `BattlerState` has no type/last-move field, so these live game-side — exactly
//    the "provider session state" the blueprint §4 prescribes.
// ─────────────────────────────────────────────────────────────────────────────

thread_local! {
    /// `BattlerRef → (type1, type2)` override (Conversion / Transform). `species_types`
    /// is unaffected; the data-reach tests read this override directly.
    static TYPE_OVERRIDE: std::cell::RefCell<std::collections::HashMap<(u8, u8), (PokemonType, PokemonType)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// The session coin pool (PayDay). One scalar per process-thread.
    static COIN_POOL: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// `BattlerRef → last move used` (the data-reach Mimic/MirrorMove/Disable read).
    static LAST_MOVE: std::cell::RefCell<std::collections::HashMap<(u8, u8), pokered_data::moves::MoveId>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// `BattlerRef → the move slot index` the Mimic handler overwrites.
    static MIMIC_SLOT: std::cell::RefCell<std::collections::HashMap<(u8, u8), usize>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Reset all P5 thread-local scratch (the harness calls this before each scenario).
pub fn reset_p5_scratch() {
    TYPE_OVERRIDE.with(|m| m.borrow_mut().clear());
    COIN_POOL.with(|c| c.set(0));
    LAST_MOVE.with(|m| m.borrow_mut().clear());
    MIMIC_SLOT.with(|m| m.borrow_mut().clear());
}

fn key(r: BattlerRef) -> (u8, u8) {
    (r.side, r.slot)
}

/// The current (overridden, else species-derived) types of the battler at `who`.
pub fn battler_types(ctx: &BattleCtx<'_, PokeredRules>, who: BattlerRef) -> (PokemonType, PokemonType) {
    if let Some(t) = TYPE_OVERRIDE.with(|m| m.borrow().get(&key(who)).copied()) {
        return t;
    }
    let s = ctx.battler(who).species;
    get_base_stats(s)
        .map(|bs| (bs.type1, bs.type2))
        .unwrap_or((PokemonType::Normal, PokemonType::Normal))
}

/// The coin pool (PayDay session state).
pub fn coin_pool() -> u32 {
    COIN_POOL.with(|c| c.get())
}

/// Set a battler's last-move-used (the harness primes this for the data-reach 5).
pub fn set_last_move(who: BattlerRef, m: pokered_data::moves::MoveId) {
    LAST_MOVE.with(|map| {
        map.borrow_mut().insert(key(who), m);
    });
}

/// Set the Mimic slot the handler will overwrite (the harness primes this).
pub fn set_mimic_slot(who: BattlerRef, slot: usize) {
    MIMIC_SLOT.with(|map| {
        map.borrow_mut().insert(key(who), slot);
    });
}

fn type_override(who: BattlerRef) -> Option<(PokemonType, PokemonType)> {
    TYPE_OVERRIDE.with(|m| m.borrow().get(&key(who)).copied())
}

/// The override types for `who` (test read).
pub fn read_type_override(who: BattlerRef) -> Option<(PokemonType, PokemonType)> {
    type_override(who)
}

fn opposing(who: BattlerRef) -> BattlerRef {
    BattlerRef::new(if who.side == 0 { 1 } else { 0 }, who.slot)
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Arena helpers — set / read / clear a volatile on a host (keyed by id).
// ─────────────────────────────────────────────────────────────────────────────

/// The next free arena id (so two volatiles never collide).
fn next_id(ctx: &BattleCtx<'_, PokeredRules>) -> EffectId {
    let max = ctx.effects.iter().map(|e| e.id.0).max().unwrap_or(0);
    EffectId(max.max(P5_ID_BASE) + 1)
}

/// Whether `host` has a volatile satisfying `pred`.
fn has_vol(
    ctx: &BattleCtx<'_, PokeredRules>,
    host: BattlerRef,
    pred: impl Fn(&PokeVolatile) -> bool,
) -> bool {
    ctx.effects.iter().any(|e| e.host == host && pred(&e.kind))
}

/// Insert a volatile on `host` (kept sorted by id for the arena's binary search).
fn set_vol(ctx: &mut BattleCtx<'_, PokeredRules>, host: BattlerRef, kind: PokeVolatile) -> EffectId {
    let id = next_id(ctx);
    ctx.effects.push(EffectState {
        id,
        host,
        effect_order: id.0 as u64,
        kind,
    });
    ctx.effects.sort_by_key(|e| e.id.0);
    id
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. FIELD / SCREEN handlers (legacy `field_effects.rs`). Each is set-ONCE: a
//    second application is a no-op (the legacy `FieldEffectAlreadyActive`). They
//    fire on `Event::Custom(EV_*)` and operate on the ATTACKER = the `source`
//    (mirroring legacy `state.attacker()`).
// ─────────────────────────────────────────────────────────────────────────────

/// Mist (#none): set the `Mist` veto volatile on the mover (legacy `apply_mist`).
fn mist_set(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if !has_vol(ctx, source, |k| matches!(k, PokeVolatile::Mist)) {
        set_vol(ctx, source, PokeVolatile::Mist);
    }
    HandlerResult::Unchanged
}

/// Focus Energy (#1): set the `FocusEnergy` `/4`-crit volatile on the mover.
fn focus_energy_set(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if !has_vol(ctx, source, |k| matches!(k, PokeVolatile::FocusEnergy)) {
        set_vol(ctx, source, PokeVolatile::FocusEnergy);
    }
    HandlerResult::Unchanged
}

/// Light Screen: set the `LightScreen` damage-halving volatile on the mover.
fn light_screen_set(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if !has_vol(ctx, source, |k| matches!(k, PokeVolatile::LightScreen)) {
        set_vol(ctx, source, PokeVolatile::LightScreen);
    }
    HandlerResult::Unchanged
}

/// Reflect: set the `Reflect` damage-halving volatile on the mover.
fn reflect_set(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if !has_vol(ctx, source, |k| matches!(k, PokeVolatile::Reflect)) {
        set_vol(ctx, source, PokeVolatile::Reflect);
    }
    HandlerResult::Unchanged
}

/// Leech Seed: seed the DEFENDER (legacy `apply_leech_seed`). Fails if the
/// defender is already seeded, Grass-type (immune), or holds a Substitute.
fn leech_seed_set(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if has_vol(ctx, target, |k| matches!(k, PokeVolatile::LeechSeed)) {
        return HandlerResult::Unchanged; // already seeded
    }
    let (t1, t2) = battler_types(ctx, target);
    if t1 == PokemonType::Grass || t2 == PokemonType::Grass {
        return HandlerResult::Unchanged; // Grass immune (legacy StatusFailed)
    }
    if has_vol(ctx, target, |k| {
        matches!(k, PokeVolatile::Substitute | PokeVolatile::SubstituteHp { .. })
    }) {
        return HandlerResult::Unchanged; // Substitute blocks (legacy StatusFailed)
    }
    set_vol(ctx, target, PokeVolatile::LeechSeed);
    HandlerResult::Unchanged
}

/// Haze: reset ALL stat stages + volatiles + non-volatile status on BOTH sides
/// (legacy `apply_haze`). Game-agnostic reach? No — this is the one effect with
/// no selector (blueprint §2 "ResetAll broadcast"), so it is a native broadcast.
fn haze_reset(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    for who in [BattlerRef::PLAYER, BattlerRef::OPPONENT] {
        let b = ctx.battler_mut(who);
        b.stat_stages = jrpg_engine::battle::EnumMap::new();
        b.status = None;
        // Haze copies the UNMODIFIED stats over the battle stats
        // (engine/battle/move_effects/haze.asm `ResetStats`) WITHOUT re-applying
        // badge boosts — wiping the accumulated stat-up-glitch boosts on the
        // player (inert without the seeded badge context).
        crate::battle::badge_boosts::wipe_boosts(b);
    }
    // Clear EVERY volatile both sides (Confused/Seeded/Toxic/FocusEnergy/Disable/…).
    ctx.effects.clear();
    HandlerResult::Unchanged
}

/// Substitute (#28): create a Substitute on the mover costing `max_hp/4` HP.
/// **Gen-1 bug #28 preserved**: HP == cost SUCCEEDS (leaving the user at 0 HP);
/// only HP < cost fails. Also fails if already up or cost == 0.
fn substitute_create(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if has_vol(ctx, source, |k| {
        matches!(k, PokeVolatile::Substitute | PokeVolatile::SubstituteHp { .. })
    }) {
        return HandlerResult::Unchanged; // already up (legacy SubstituteFailed)
    }
    let b = ctx.battler(source);
    let cost = b.max_hp / 4;
    if cost == 0 || b.hp < cost {
        return HandlerResult::Unchanged; // legacy SubstituteFailed
    }
    ctx.battler_mut(source).hp -= cost; // BUG #28: hp == cost leaves the user at 0
    set_vol(ctx, source, PokeVolatile::SubstituteHp { hp: cost });
    HandlerResult::Unchanged
}

/// Rest (HealEffect Rest branch): full-heal + self-sleep(2) + cure badly-poisoned.
/// (Recover/Softboiled = the P1 `heal.recover` data path; only Rest is native.)
fn rest_heal(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let max = ctx.battler(source).max_hp;
    let b = ctx.battler_mut(source);
    b.hp = max;
    b.status = Some(crate::battle::state::StatusCondition::Sleep(2));
    // Cure badly-poisoned (legacy clears BADLY_POISONED + toxic_counter on Rest).
    ctx.effects
        .retain(|e| !(e.host == source && matches!(e.kind, PokeVolatile::Toxic { .. })));
    HandlerResult::Unchanged
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. VOLATILE-SET handlers (legacy `special_effects.rs`). Flinch / Confusion set
//    a `BeforeMove`-gate volatile on the DEFENDER; Disable disables a foe move
//    slot; PayDay awards coins; Toxic sets the badly-poisoned counter.
// ─────────────────────────────────────────────────────────────────────────────

/// Flinch (side): set `Flinched` on the defender (legacy `apply_flinch_side`).
/// Blocked by a Substitute on the defender. The chance gate is the harness's
/// (drawn at the legacy ordinal); this handler is the APPLY only.
fn flinch_set(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if has_vol(ctx, target, |k| {
        matches!(k, PokeVolatile::Substitute | PokeVolatile::SubstituteHp { .. })
    }) {
        return HandlerResult::Unchanged; // legacy StatusFailed (Substitute)
    }
    if !has_vol(ctx, target, |k| matches!(k, PokeVolatile::Flinched)) {
        set_vol(ctx, target, PokeVolatile::Flinched);
    }
    HandlerResult::Unchanged
}

/// Confusion (primary): set `Confused{turns}` on the defender (legacy
/// `apply_confusion_primary`). `turns = (relay & 3) + 2` (the harness lays the
/// duration byte into the relay). Fails if already confused or Substitute-up.
fn confusion_set(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if has_vol(ctx, target, |k| matches!(k, PokeVolatile::Confused { .. })) {
        return HandlerResult::Unchanged; // already confused (legacy StatusFailed)
    }
    if has_vol(ctx, target, |k| {
        matches!(k, PokeVolatile::Substitute | PokeVolatile::SubstituteHp { .. })
    }) {
        return HandlerResult::Unchanged; // legacy StatusFailed (Substitute)
    }
    let duration = (relay.as_int() as u8) & 0x03;
    let turns = duration + 2;
    set_vol(ctx, target, PokeVolatile::Confused { turns });
    HandlerResult::Unchanged
}

/// Disable: disable the defender's last-used move slot (legacy `apply_disable`).
/// `turns = (relay & 7) + 1` (min 1). Fails if already disabled, no last move,
/// or the last move is not in a slot with PP. The harness primes `LAST_MOVE` +
/// the defender's move list; we resolve the slot from the engine `moves` Vec.
fn disable_set(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if has_vol(ctx, target, |k| matches!(k, PokeVolatile::Disable { .. })) {
        return HandlerResult::Unchanged; // already disabled (legacy StatusFailed)
    }
    let last = LAST_MOVE.with(|m| m.borrow().get(&key(target)).copied());
    let Some(last) = last else {
        return HandlerResult::Unchanged; // no last move (legacy StatusFailed)
    };
    if last == pokered_data::moves::MoveId::None {
        return HandlerResult::Unchanged;
    }
    let slot = ctx.battler(target).moves.iter().position(|m| *m == last);
    let Some(slot) = slot else {
        return HandlerResult::Unchanged; // last move not in a slot (legacy StatusFailed)
    };
    let turns = ((relay.as_int() as u8) & 0x07).wrapping_add(1).max(1);
    set_vol(
        ctx,
        target,
        PokeVolatile::Disable {
            slot: (slot + 1) as u8, // 1-based, mirroring legacy `disabled_move`
            turns,
        },
    );
    HandlerResult::Unchanged
}

/// Pay Day: award `level·2` coins to the session pool (legacy `apply_pay_day`).
/// The mover's level rides the harness `set_level`; we read it via the binding.
fn pay_day(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let level = super::level_for_species_pub(ctx.battler(source).species);
    let coins = level * 2;
    COIN_POOL.with(|c| c.set(c.get() + coins as u32));
    HandlerResult::Unchanged
}

/// Toxic (#6): set `Toxic{counter:0}` (badly-poisoned) on the defender + the
/// non-volatile Poison status (legacy sets `StatusCondition::Poison` + the
/// `BADLY_POISONED` flag). Fails if the defender already has a status. The
/// UNCAPPED ramp is in [`toxic_residual`].
fn toxic_set(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if ctx.battler(target).status.is_some() {
        return HandlerResult::Unchanged; // already statused (legacy guard)
    }
    if has_vol(ctx, target, |k| {
        matches!(k, PokeVolatile::Substitute | PokeVolatile::SubstituteHp { .. })
    }) {
        return HandlerResult::Unchanged; // Substitute blocks
    }
    ctx.battler_mut(target).status = Some(crate::battle::state::StatusCondition::Poison);
    set_vol(ctx, target, PokeVolatile::Toxic { counter: 0 });
    HandlerResult::Unchanged
}

/// Toxic residual (#6, UNCAPPED): chip `(max/16).max(1) * counter` after
/// incrementing the counter — NEVER capped (the Gen-1 bug; legacy `residual.rs`).
/// Fires on `Event::Residual`, hosted on the badly-poisoned battler.
fn toxic_residual(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    // Find the Toxic volatile by HOST (it is the arena entry hosted on the
    // residual's `target`, NOT keyed by `source_effect` — the driver passes the
    // residual EFFECT's id, not the arena entry's). Increment + read the counter.
    let idx = ctx
        .effects
        .iter()
        .position(|e| e.host == target && matches!(e.kind, PokeVolatile::Toxic { .. }));
    let Some(idx) = idx else {
        return HandlerResult::Unchanged;
    };
    let n = match &mut ctx.effects[idx].kind {
        PokeVolatile::Toxic { counter } => {
            *counter = counter.saturating_add(1);
            *counter as u16
        }
        _ => return HandlerResult::Unchanged,
    };
    let max = ctx.battler(target).max_hp;
    let dmg = (max / 16).max(1) * n; // BUG #6: uncapped multiply
    ctx.battler_mut(target).take_damage(dmg);
    HandlerResult::Unchanged
}

/// Leech Seed residual: drain from the seeded host to its seeder (legacy
/// `residual.rs` leech). Fires on `Event::Residual`, hosted on the seeded
/// battler; the seeder is the opposing battler.
///
/// Gen-1 TOXIC × LEECH SEED interaction (deliberate bug, reproduced): the asm
/// runs the leech drain through the SAME HP-decrease routine as poison
/// (`HandlePoisonBurnLeechSeed_DecreaseOwnHP`, core.asm:550+), so when the host
/// is BADLY POISONED the Toxic counter is incremented AGAIN and the drain scales
/// with it — drain = `(max/16).max(1) × new_counter` (and the seeder heals the
/// full amount). When the Toxic volatile ticks first (the usual install order),
/// a seeded + badly-poisoned host takes `base×N` poison and `base×(N+1)` leech
/// per turn, exactly like the original.
fn leech_residual(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if ctx.battler(target).hp == 0 {
        return HandlerResult::Unchanged; // dead host (legacy early-return)
    }
    let max = ctx.battler(target).max_hp;
    let base = (max / 16).max(1);
    // Badly-poisoned host → the shared-routine bug: bump the Toxic counter and
    // scale the drain by it (mirrors `toxic_residual`'s UNCAPPED multiply).
    let tox_idx = ctx
        .effects
        .iter()
        .position(|e| e.host == target && matches!(e.kind, PokeVolatile::Toxic { .. }));
    let drain = match tox_idx {
        Some(idx) => {
            let n = match &mut ctx.effects[idx].kind {
                PokeVolatile::Toxic { counter } => {
                    *counter = counter.saturating_add(1);
                    *counter as u16
                }
                _ => return HandlerResult::Unchanged,
            };
            base * n
        }
        None => base,
    };
    let seeder = opposing(target);
    let (host, seeder_b) = ctx.pair_mut(target, seeder);
    let actual = drain.min(host.hp);
    host.take_damage(actual);
    seeder_b.heal(actual);
    HandlerResult::Unchanged
}

/// Burn (non-volatile status) residual: chip `(max/16).max(1)` flat each turn
/// (legacy `residual.rs:29-31`). Fires on `Event::Residual`, hosted on the burned
/// battler; draws NO rng. Self-guards on an already-fainted host (legacy
/// `apply_all_residual` early-returns after a faint).
fn burn_residual(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if ctx.battler(target).hp == 0 {
        return HandlerResult::Unchanged;
    }
    let dmg = (ctx.battler(target).max_hp / 16).max(1);
    ctx.battler_mut(target).take_damage(dmg);
    HandlerResult::Unchanged
}

/// Poison (non-volatile status) residual: chip `(max/16).max(1)` flat. A
/// BADLY-poisoned (Toxic) battler is ticked by the Toxic VOLATILE ramp instead, so
/// if a `Toxic` volatile is live on the host this flat tick SKIPS — the legacy
/// `apply_residual_status_damage` ramps-OR-flats in ONE place, so exactly one chip
/// must land. Fires on `Event::Residual`; draws NO rng; self-guards on faint.
fn poison_residual(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if ctx.battler(target).hp == 0 {
        return HandlerResult::Unchanged;
    }
    // Badly-poisoned → the Toxic volatile ramp owns the tick (avoid double-chip).
    let badly = ctx
        .effects
        .iter()
        .any(|e| e.host == target && matches!(e.kind, PokeVolatile::Toxic { .. }));
    if badly {
        return HandlerResult::Unchanged;
    }
    let dmg = (ctx.battler(target).max_hp / 16).max(1);
    ctx.battler_mut(target).take_damage(dmg);
    HandlerResult::Unchanged
}

// ─────────────────────────────────────────────────────────────────────────────
// 4b. BeforeMove status / volatile GATES (P6b-prereq stage 2). Re-homes the proven
//     `stack_parity` POC gates onto PokeredRules. Attached to EVERY move effect by
//     the per-move builder (`mod.rs`), fired on `Event::BeforeMove` in `order`
//     sequence — sleep(10) → freeze(20) → confusion(70) → paralysis(90), the legacy
//     ASM / `MoveRandoms` field order (confusion byte before paralysis byte, both
//     before the crit/accuracy/damage draws). Each reads the MOVER's OWN
//     status/volatile and is INERT (no rng drawn, `Unchanged`) when absent — so a
//     no-status mover is byte-identical and every existing P1-P5 scenario is
//     unaffected. `Fail` aborts the move; `run_event` short-circuits on the first
//     `Fail`, so a confusion self-hit (order 70) stops before the paralysis gate
//     (order 90) ever fires — matching the legacy "confusion checked before para".
// ─────────────────────────────────────────────────────────────────────────────

/// Sleep gate (order 10, bug #8): decrement the counter; the mon cannot act while
/// asleep AND still forfeits the turn on the tick it wakes. Draws NO rng.
pub fn sleep_gate(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let counter = match ctx.battler(source).status {
        Some(StatusCondition::Sleep(c)) => c,
        _ => return HandlerResult::Unchanged,
    };
    if counter == 0 {
        ctx.battler_mut(source).status = None; // defensive: Sleep(0) ⇒ awake
        return HandlerResult::Unchanged;
    }
    let new_counter = counter - 1;
    ctx.battler_mut(source).status = if new_counter == 0 {
        None // woke up …
    } else {
        Some(StatusCondition::Sleep(new_counter))
    };
    HandlerResult::Fail // … but the wake tick still forfeits the turn (#8)
}

/// Freeze gate (order 20, bug #10): a frozen mon ALWAYS cannot move; Gen-1 has no
/// per-turn thaw roll, so this draws NO rng and never clears the status itself.
pub fn freeze_gate(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if ctx.battler(source).status == Some(StatusCondition::Freeze) {
        HandlerResult::Fail
    } else {
        HandlerResult::Unchanged
    }
}

/// Flinch gate (order 30, after freeze, before confusion): a flinched mon cannot
/// move this turn. The `Flinched` volatile is CONSUMED (removed) when the gate
/// runs, so it never persists past the mon's own move attempt (legacy
/// `status1::FLINCHED` cleared each turn). No rng.
pub fn flinch_gate(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let idx = ctx
        .effects
        .iter()
        .position(|e| e.host == source && matches!(e.kind, PokeVolatile::Flinched));
    match idx {
        Some(i) => {
            ctx.effects.remove(i); // consume the flinch
            HandlerResult::Fail // move aborted this turn
        }
        None => HandlerResult::Unchanged,
    }
}

/// Confusion gate (order 70): confusion is the `Confused` VOLATILE. Decrement
/// `turns`; on reaching 0 snap out (remove the volatile, NO byte, the mon acts);
/// else draw ONE byte — `< 128` ⇒ 50% self-hit (typeless 40-power, `Fail`), `>= 128`
/// ⇒ the mon acts. The byte is drawn ONLY while still-confused-and-not-snapping, so
/// `consumed()` matches the legacy meaningful read.
pub fn confusion_gate(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let idx = ctx
        .effects
        .iter()
        .position(|e| e.host == source && matches!(e.kind, PokeVolatile::Confused { .. }));
    let Some(idx) = idx else {
        return HandlerResult::Unchanged; // not confused
    };
    let turns_after = {
        let PokeVolatile::Confused { turns } = &mut ctx.effects[idx].kind else {
            return HandlerResult::Unchanged;
        };
        if *turns > 0 {
            *turns -= 1;
        }
        *turns
    };
    if turns_after == 0 {
        ctx.effects.remove(idx); // snap out: no byte, mon acts
        return HandlerResult::Unchanged;
    }
    let roll = ctx.rng.next_u8();
    if roll < 128 {
        let self_damage = confusion_self_hit_damage(ctx, source);
        ctx.battler_mut(source).take_damage(self_damage);
        return HandlerResult::Fail; // hit itself → move aborted
    }
    HandlerResult::Unchanged // confused but acts this turn
}

/// Paralysis gate (order 90): 25% full paralysis (`byte < 63`). Draws a byte ONLY
/// when paralyzed, AFTER the confusion byte (order 90 > 70).
pub fn paralysis_gate(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if ctx.battler(source).status == Some(StatusCondition::Paralysis) {
        let roll = ctx.rng.next_u8();
        if roll < 63 {
            return HandlerResult::Fail; // fully paralyzed → move aborted
        }
    }
    HandlerResult::Unchanged
}

/// Confusion self-hit damage — typeless 40-power physical hit using the mover's OWN
/// Attack vs OWN Defense, no crit, max roll (re-homes legacy
/// `move_execution` `calc_confusion_self_hit`) through pokered's damage authority.
fn confusion_self_hit_damage(ctx: &BattleCtx<'_, PokeredRules>, who: BattlerRef) -> u16 {
    use crate::battle::damage::{calculate_damage, DamageParams};
    use crate::battle::stat_stages::StatIndex;
    let b = ctx.battler(who);
    let atk = b.stats.get(StatIndex::Attack).copied().unwrap_or(0);
    let def = b.stats.get(StatIndex::Defense).copied().unwrap_or(1);
    let params = DamageParams {
        attacker_level: super::level_of(b),
        move_power: 40,
        move_type: PokemonType::Normal,
        move_id: pokered_data::moves::MoveId::None,
        attack_stat: atk,
        defense_stat: def,
        attack_stage: 0,
        defense_stage: 0,
        attacker_type1: PokemonType::Normal,
        attacker_type2: PokemonType::Normal,
        defender_type1: PokemonType::Normal,
        defender_type2: PokemonType::Normal,
        is_critical: false,
        random_value: 255,
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        // Confusion self-hit is typeless and unaffected by burn.
        attacker_burned: false,
    };
    calculate_damage(&params).damage
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. THE DATA-REACH 5 (blueprint §2 — kept NATIVE as `Event::Custom`, NOT script:
//    arbitrary logic over `P::Species` / the move table / a foe's last move). The
//    engine treats the custom event opaquely; all the data-reach logic is here.
// ─────────────────────────────────────────────────────────────────────────────

/// Conversion (data-reach): copy the DEFENDER's types onto the mover (legacy
/// `apply_conversion`). The engine derives types from species, so the copy lands
/// in the [`TYPE_OVERRIDE`] arena (provider session state).
fn conversion(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let (t1, t2) = battler_types(ctx, target);
    TYPE_OVERRIDE.with(|m| {
        m.borrow_mut().insert(key(source), (t1, t2));
    });
    HandlerResult::Unchanged
}

/// Transform (data-reach): copy the DEFENDER's species/types/stats/stages/moves
/// onto the mover, PP→5 (legacy `apply_transform`).
fn transform(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let (species, stats, stages, moves, types) = {
        let d = ctx.battler(target);
        (
            d.species,
            d.stats.clone(),
            d.stat_stages.clone(),
            d.moves.clone(),
            battler_types(ctx, target),
        )
    };
    let a = ctx.battler_mut(source);
    a.species = species;
    a.stats = stats;
    a.stat_stages = stages;
    a.moves = moves;
    TYPE_OVERRIDE.with(|m| {
        m.borrow_mut().insert(key(source), types);
    });
    HandlerResult::Unchanged
}

/// Mimic (data-reach): overwrite the mover's chosen slot with the DEFENDER's
/// last-used move, PP→5 (legacy `apply_mimic`). Fails if the foe has no last move.
fn mimic(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let last = LAST_MOVE.with(|m| m.borrow().get(&key(target)).copied());
    let Some(last) = last else {
        return HandlerResult::Unchanged;
    };
    if last == pokered_data::moves::MoveId::None {
        return HandlerResult::Unchanged; // legacy StatusFailed
    }
    let slot = MIMIC_SLOT.with(|m| m.borrow().get(&key(source)).copied()).unwrap_or(0);
    let b = ctx.battler_mut(source);
    if slot < b.moves.len() {
        b.moves[slot] = last;
    }
    HandlerResult::Unchanged
}

/// Metronome (data-reach): pick a random move 1-165, skipping Metronome (0x76),
/// from the duration byte (legacy `apply_metronome`). The picked move rides the
/// relay back to the harness as an `Int`.
fn metronome(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    relay: RelayVar,
    _target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let _ = ctx;
    let duration = relay.as_int() as u16;
    let raw = (duration % 163) + 1;
    let move_val = if raw >= 0x76 { raw + 1 } else { raw }; // skip Metronome (0x76)
    let picked = pokered_data::moves::move_id_from_u8(move_val as u8);
    HandlerResult::Set(RelayVar::Int(picked as i64))
}

/// Mirror Move (data-reach): re-dispatch the DEFENDER's last move (legacy
/// `apply_mirror_move`). The mirrored move rides the relay back to the harness.
fn mirror_move(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let _ = ctx;
    let last = LAST_MOVE.with(|m| m.borrow().get(&key(target)).copied());
    match last {
        Some(m) if m != pokered_data::moves::MoveId::None => {
            HandlerResult::Set(RelayVar::Int(m as i64))
        }
        _ => HandlerResult::Fail, // legacy StatusFailed
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. THE on:Miss SEAM (JumpKick crash, blueprint §3 "the one true core touch").
//    The engine fires `Event::OnMiss` on the accuracy-miss branch (additive +
//    DEFAULTED — inert unless a game subscribes). pokered subscribes here: a
//    JumpKick / Hi Jump Kick that MISSES crashes the user for 1 HP (Gen-1).
// ─────────────────────────────────────────────────────────────────────────────

/// JumpKick crash-on-miss (Gen-1): on a miss, the USER (the `source` of the move)
/// takes 1 HP crash damage. Fires on the new `Event::OnMiss` seam.
fn jump_kick_crash(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    ctx.battler_mut(source).take_damage(1);
    HandlerResult::Unchanged
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. The leaked `&'static Effect`s — one per P5 native effect, each a single hook
//    on its custom event (or `Residual` / `OnMiss`). Built once per process via
//    `OnceLock`. The harness fires these directly through the dispatch fold.
// ─────────────────────────────────────────────────────────────────────────────

macro_rules! p5_effect {
    ($fnname:ident, $id:expr, $ev:expr, $call:expr) => {
        pub fn $fnname() -> &'static Effect<PokeredRules> {
            use std::sync::OnceLock;
            static EFF: OnceLock<&'static Effect<PokeredRules>> = OnceLock::new();
            EFF.get_or_init(|| {
                let hooks: &'static [EventHook<PokeredRules>] = Box::leak(
                    vec![EventHook {
                        event: $ev,
                        call: $call,
                        order: 100,
                        priority: 0,
                        sub_order: None,
                    }]
                    .into_boxed_slice(),
                );
                Box::leak(Box::new(Effect {
                    id: EffectId($id),
                    kind: EffectType::Move,
                    hooks,
                }))
            })
        }
    };
}

p5_effect!(mist_effect, P5_ID_BASE + 1, Event::Custom(EV_MIST), mist_set);
p5_effect!(focus_energy_effect, P5_ID_BASE + 2, Event::Custom(EV_FOCUS_ENERGY), focus_energy_set);
p5_effect!(light_screen_effect, P5_ID_BASE + 3, Event::Custom(EV_LIGHT_SCREEN), light_screen_set);
p5_effect!(reflect_effect, P5_ID_BASE + 4, Event::Custom(EV_REFLECT), reflect_set);
p5_effect!(leech_seed_effect, P5_ID_BASE + 5, Event::Custom(EV_LEECH_SEED), leech_seed_set);
p5_effect!(haze_effect, P5_ID_BASE + 6, Event::Custom(EV_HAZE), haze_reset);
p5_effect!(substitute_effect, P5_ID_BASE + 7, Event::Custom(EV_SUBSTITUTE), substitute_create);
p5_effect!(conversion_effect, P5_ID_BASE + 8, Event::Custom(EV_CONVERSION), conversion);
p5_effect!(rest_effect, P5_ID_BASE + 9, Event::Custom(EV_REST), rest_heal);
p5_effect!(flinch_effect, P5_ID_BASE + 10, Event::Custom(EV_FLINCH), flinch_set);
p5_effect!(confusion_effect, P5_ID_BASE + 11, Event::Custom(EV_CONFUSION), confusion_set);
p5_effect!(disable_effect, P5_ID_BASE + 12, Event::Custom(EV_DISABLE), disable_set);
p5_effect!(pay_day_effect, P5_ID_BASE + 13, Event::Custom(EV_PAY_DAY), pay_day);
p5_effect!(toxic_effect, P5_ID_BASE + 14, Event::Custom(EV_TOXIC), toxic_set);
p5_effect!(transform_effect, P5_ID_BASE + 15, Event::Custom(EV_TRANSFORM), transform);
p5_effect!(mimic_effect, P5_ID_BASE + 16, Event::Custom(EV_MIMIC), mimic);
p5_effect!(metronome_effect, P5_ID_BASE + 17, Event::Custom(EV_METRONOME), metronome);
p5_effect!(mirror_move_effect, P5_ID_BASE + 18, Event::Custom(EV_MIRROR_MOVE), mirror_move);
p5_effect!(toxic_residual_effect, P5_ID_BASE + 19, Event::Residual, toxic_residual);
p5_effect!(leech_residual_effect, P5_ID_BASE + 20, Event::Residual, leech_residual);
p5_effect!(jump_kick_crash_effect, P5_ID_BASE + 21, Event::OnMiss, jump_kick_crash);
p5_effect!(burn_residual_effect, P5_ID_BASE + 22, Event::Residual, burn_residual);
p5_effect!(poison_residual_effect, P5_ID_BASE + 23, Event::Residual, poison_residual);
