//! # P1 — the pokered `RulesProvider` + bucket-A moves authored as RON data
//! (migration blueprint `15` §2 / §5 P1).
//!
//! This module stands up a **game-side** [`PokeredRules`] provider that drives
//! the bucket-A Gen-1 moves through the engine's [`StackDriver`] using effects
//! authored in [`rules.ron`](./rules.ron) (loaded via the game-agnostic
//! `jrpg-rules` loader). It is **ADDITIVE and DIFFERENTIAL-ONLY** (`#![cfg(test)]`
//! at the module-decl site): the legacy [`apply_move_effect`](super::effects) /
//! [`execute_turn`](super::turn::execute_turn) dispatcher stays the **production
//! oracle**, untouched, and the frame-stepped production loop is NOT routed
//! through the stack (that is P6). All this module does is prove, side-by-side,
//! that the data-driven bucket-A effects produce an **IDENTICAL `BattleState`**
//! AND **identical `rng.consumed()`** vs the legacy oracle on real Gen-1 numbers.
//!
//! ## The damage authority (the load-bearing invariant)
//!
//! `pokered_core::battle::damage::calculate_damage` stays the **single damage
//! authority** — it folds STAB + the per-type effectiveness chart + the ×roll/255
//! ordering into ONE number (the real Gen-1 damage). The stack path precomputes
//! that full number into `ctx.mv.damage` inside the native [`pokered_damage`]
//! `ModifyDamage` handler (mirroring minimon's `move_damage_hook`, where the
//! number is provider-computed and the hook is the subscription marker). So:
//!
//!   * the move's `DealMoveDamage` op is the `ModifyDamage` **subscription
//!     marker** (resolves `Unchanged` — the number is already in `ctx.mv.damage`);
//!   * `ApplyTypeChart` rides the `Effectiveness` fold as the **declarative
//!     marker** but resolves **NEUTRAL (1×)** on purpose: the chart already rode
//!     the damage authority, so a second fold would double-count. The binding's
//!     [`PokeredBindings::type_chart_mult`] returns `(1, 1)` for exactly this.
//!
//! The crit / accuracy / damage-roll **draws** are native pipeline handlers
//! ([`pokered_crit`] / [`pokered_accuracy`] / [`pokered_damage`]) re-homing
//! pokered's own formula (the damage authority), so the stack's draw order is
//! byte-identical to the legacy `MoveRandoms` field order — exactly as the
//! existing `stack_parity` POC handlers do, with one addition: a **power-0
//! short-circuit** so a self-Boost / self-heal move draws ONLY the accuracy byte
//! (no crit, no damage), matching the legacy power-0 branch
//! (`move_execution.rs:78-101`).
//!
//! ## Honest bucket-A scope (data vs deferred)
//!
//! Authored as PURE DATA in P1 (today's op vocabulary + the type chart):
//!   * pure-damage moves (`NoAdditionalEffect`) + Swift → `[DealMoveDamage, ApplyTypeChart]`
//!   * the 9 *used* self-Boost effects (+1 / +2 stage on self) → `[Boost(stat, ±N, Host)]`
//!     (of the 11 self-Boost variants, AccuracyUp1/AccuracyUp2/EvasionUp2 have 0 moves in Gen-1)
//!   * Recover / Softboiled (`HealEffect`, heal 1/2 max HP) → `[HealFraction(Host, MaxHp, 1/2)]`
//!   * Splash (no-op) → no hooks.
//!
//! DEFERRED to a later phase — today's vocabulary lacks the needed primitive:
//!   * **Drain** (`DrainHpEffect`): needs `HealFraction` with `of: LastDamage`
//!     (heal 1/2 the damage DEALT). The `FractionOf` enum has only `MaxHp`/`CurHp`.
//!   * **Recoil** (`RecoilEffect`): needs `DamageFraction` with `of: LastDamage`
//!     (self-damage 1/4 the damage dealt). Same missing fraction base.
//!   * **PayDay** (`PayDayEffect`): needs an AWARD-resource / session-coin op;
//!     `PayResource` only **deducts** a cost (and fails if unaffordable). PayDay
//!     adds `level·2` to the coin pool (provider session state, blueprint §4) —
//!     no op expresses that today.
//!
//! These three are listed as "A" in the blueprint's summary table, but its own
//! per-effect op-spec (§2 rows `DrainHpEffect`/`RecoilEffect`/`PayDayEffect`)
//! reveals the `lastDamage` / coin-award reach. We MOVE them to a later phase
//! rather than sneak native logic into a "data" move (the §6 honesty mandate).
//!
//! ## Dual-mode sourcing (doc 11 §4.2)
//!
//! [`load_ruleset`] builds the dual-mode [`RuleSource`]: BAKED (`include_str!`,
//! the default build, zero file IO) or — with the `hot-reload` feature — from
//! DISK with a watcher. Both yield the SAME [`Ruleset`]. A test proves baked and
//! disk compile to byte-identical registries.

// PRODUCTIONIZED (P6 flip): the provider now compiles in non-test builds so the
// live battle loop can route through it. The differential tests in `tests` /
// `p5_tests` self-gate via their own `#![cfg(test)]`. `allow(dead_code)` covers the
// test-only setup helpers + the surface not yet called by the production loop.
#![allow(dead_code)]

use std::cell::RefCell;

use jrpg_engine::battle::stack::{
    BattleCtx, Effect, EffectId, EffectProvider, EffectState, EffectType, Event, EventHook,
    HandlerResult, RelayVar,
};
use jrpg_engine::battle::{
    BattleAction, BattleProvider, BattleState as EngineState, BattlerRef,
    BattlerState as EngineBattler, DamageResult, EffectResult, EnumMap, MoveEffect as EngineMoveEffect,
};

use jrpg_rules::{
    CompiledHook, CompiledRuleset, Op, RuleBindings, RuleSource, Ruleset, RulesHost, RulesProvider,
};

use pokered_data::move_data::MoveData;
use pokered_data::moves::{MoveEffect, MoveId};
use pokered_data::pokemon_data::get_base_stats;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

use crate::battle::stat_stages::StatIndex;
use crate::battle::state::StatusCondition as LegacyStatus;

// ─────────────────────────────────────────────────────────────────────────────
// 0. The canonical rules.ron — BAKED into the binary (default build) + the disk
//    path (dev / hot-reload). Both parse to the SAME Ruleset.
// ─────────────────────────────────────────────────────────────────────────────

/// The canonical `rules.ron` text, compiled into the binary via `include_str!`
/// (the BAKED dual-mode path; zero file IO). The DISK path reads the *same* file
/// from [`RULES_RON_PATH`]; both parse to an identical [`Ruleset`].
pub const RULES_RON_BAKED: &str = include_str!("rules.ron");

/// The on-disk path of the canonical `rules.ron` (DEV / hot-reload path),
/// resolved at compile time relative to this source file so the disk read targets
/// the *same* file the baked text was `include_str!`'d from.
pub const RULES_RON_PATH: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/src/battle/pokered_rules/rules.ron");

/// The `EffectId` base for the synthesized data hooks (well clear of any native
/// `stack_parity` POC ids, which top out around 180).
pub const DATA_ID_BASE: u32 = 0x20_000;

/// The stable per-move-record `EffectId` base. `effect_for_move` returns ONE
/// combined effect per move record whose `id` is `MOVE_EFFECT_ID_BASE + record
/// index`; the engine threads that id as `source_effect` to every hook, and the
/// data bridge recovers the record's op-lists by it. Kept distinct from
/// [`DATA_ID_BASE`] (the per-hook synthesized ids) so the two spaces never alias.
pub const MOVE_EFFECT_ID_BASE: u32 = 0x30_000;

// ─────────────────────────────────────────────────────────────────────────────
// 1. The minimal P1 volatile enum (EffectStateKind). Focus Energy rides as a
//    volatile for the crit `/4` bug, exactly like the stack_parity POC.
// ─────────────────────────────────────────────────────────────────────────────

/// The game-supplied typed per-effect-state enum for the P1 stack path. Minimal:
/// only the Focus-Energy marker the crit pipeline reads. Richer volatiles (the
/// B/C tiers) land in later phases — adding them here keeps `jrpg-engine` 100%
/// game-agnostic (the engine treats `EffectStateKind` opaquely).
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum PokeVolatile {
    /// Inert.
    None,
    /// Focus Energy volatile (drives the Gen-1 `/4` crit bug, #1).
    FocusEnergy,
    /// Substitute up (drives the side-status Substitute block, P2; and the P3
    /// foe-stat-down absorption via the nested-veto driver). The engine treats
    /// this opaquely; only [`PokeredBindings::has_volatile`] maps the name
    /// `"Substitute"` ↔ this arena entry, and [`PokeredRules::effect_for_volatile`]
    /// hands back its `TryBoost`-veto effect.
    Substitute,
    /// Mist up (P3): the foe-stat-down veto. A battler protected by Mist cannot
    /// have its stats lowered by the foe (legacy `apply_stat_down`'s
    /// `PROTECTED_BY_MIST` guard). Drives the nested-veto cascade: the pokered-side
    /// driver fires `Event::TryBoost`, and this volatile's registered `TryBoost`
    /// handler returns `Fail` to veto the stat drop.
    Mist,

    // ── P5 native-tier volatiles (blueprint 15 §5 P5 / "New game-side
    //    EffectStateKind variants"). Each mirrors a legacy `status1/2/3` bit or a
    //    scalar counter; the engine treats them OPAQUELY (zero engine change). The
    //    P5 native handlers (fired on `Event::Custom(_)`) set / read them, exactly
    //    reproducing the legacy `apply_*` semantics + the specific Gen-1 bug. ──

    /// **Substitute with HP** (#28; legacy `substitute_hp` + `HAS_SUBSTITUTE_UP`).
    /// The P5 substitute-creation handler sets this with `hp = max_hp/4` (and the
    /// Gen-1 0-HP bug: HP == cost succeeds, leaving the user at 0). It also serves
    /// the P2/P3 Substitute veto/block: [`PokeredBindings::has_volatile`] and
    /// [`PokeredRules::effect_for_volatile`] match BOTH this and the bare
    /// [`PokeVolatile::Substitute`] marker, so the absorb/veto is unchanged.
    SubstituteHp { hp: u16 },
    /// **Light Screen up** (legacy `status3::HAS_LIGHT_SCREEN_UP`). Halves the
    /// special damage the host takes — the screen `ModifyDamage` resolver reads it.
    LightScreen,
    /// **Reflect up** (legacy `status3::HAS_REFLECT_UP`). Halves the physical
    /// damage the host takes.
    Reflect,
    /// **Leech Seed** (legacy `status2::SEEDED`). The end-of-turn residual drains
    /// the seeded host to its seeder.
    LeechSeed,
    /// **Toxic / badly-poisoned counter** (Gen-1 bug #6 — UNCAPPED ramp; legacy
    /// `status3::BADLY_POISONED` + `toxic_counter`). The residual chips
    /// `(max/16).max(1) * counter` with the counter incremented each tick and
    /// NEVER capped.
    Toxic { counter: u8 },
    /// **Flinched** (legacy `status1::FLINCHED`). Consumed by the `BeforeMove`
    /// gate (a flinched mon can't act this turn).
    Flinched,
    /// **Confused** with a turn counter (legacy `status1::CONFUSED` +
    /// `confused_turns_left`). The `BeforeMove` gate decrements it; at 0 the
    /// confusion ends.
    Confused { turns: u8 },
    /// **Disabled** move (legacy `disabled_move` 1-based slot + `disabled_turns_left`).
    /// The action-selection veto blocks the disabled slot until the counter hits 0.
    Disable { slot: u8, turns: u8 },

    /// **Per-turn damage-taken scratch** — the cross-action read home for Counter
    /// (and, later, Bide) resolving the design §9 open question. NOT backed by any
    /// legacy status bit: it is created fresh inside a SINGLE turn's arena by
    /// [`record_damage_taken`] and dropped at write-back (`apply_engine_to_legacy`'s
    /// `_ => {}` catch-all), so it never persists across turns and needs no explicit
    /// reset. `amount` is the damage the host took this turn; `counterable` records
    /// whether the dealing move was a Gen-1 Counter-eligible move — a NORMAL- or
    /// FIGHTING-type move (NOT merely any physical move: Counter does not reflect
    /// Ground/Rock/etc., bug #20). Opaque to the engine like every other kind.
    DamageTaken { amount: u16, counterable: bool },

    /// **Must-recharge** (Hyper Beam, legacy `status2::NEEDS_TO_RECHARGE`). Set on the
    /// user the turn Hyper Beam connects (unless the target faints — the Gen-1 quirk),
    /// it makes [`PokeredRules::forced_action`] return `Nothing` next turn so the mon
    /// skips (the recharge turn). The lifecycle is managed game-side: the flag
    /// round-trips through the legacy bit and is consumed after the skip turn (see
    /// `execute_turn_with_move`). The engine only sees "this actor is forced to do
    /// Nothing", never "Hyper Beam".
    Recharge,

    /// **Charging a two-turn move** (Fly/Dig/Solar Beam/Razor Wind/Skull Bash/Sky
    /// Attack; legacy `status1::CHARGING_UP` + `INVULNERABLE`). Installed on the
    /// CHARGE turn (the move deals no damage and draws nothing); on the STRIKE turn
    /// [`PokeredRules::forced_action`] forces `move_` and the native pipeline removes
    /// this volatile and lands the hit. `invulnerable` (Fly/Dig only) makes the
    /// opponent's moves miss while charging — bar the Gen-1 exceptions (Gust/Thunder
    /// vs Fly, Earthquake/Fissure vs Dig). The engine only sees an opaque kind + a
    /// forced action; the charge/strike meaning is entirely game-side.
    Charging { move_: MoveId, invulnerable: bool },

    /// **Locked into a rampage move** (Thrash / Petal Dance; legacy
    /// `status1::THRASHING_ABOUT` + `num_attacks_left`). Installed on first use with a
    /// 2–3 use counter; [`PokeredRules::forced_action`] re-issues `move_` until it runs
    /// out, then the user self-confuses (`confuse_on_end`, the Gen-1 fatigue). The
    /// engine only sees an opaque kind + a forced action.
    LockedMove { move_: MoveId, turns_left: u8, confuse_on_end: bool },

    /// **Locked into Rage** (legacy `status2::USING_RAGE`). Once used, Rage is
    /// re-issued every turn ([`PokeredRules::forced_action`]) and the user's Attack
    /// rises one stage each time it is hit by a damaging move — the Gen-1 "rage is
    /// building". No counter (persists until the mon switches/faints, which the
    /// legacy round-trip handles). Opaque to the engine.
    Rage,

    /// **Trapping move in progress** (Wrap/Bind/Fire Spin/Clamp; legacy
    /// `status1::USING_TRAPPING_MOVE` + `num_attacks_left`). The user is locked into
    /// `move_` for 2–5 turns ([`PokeredRules::forced_action`]) — it re-hits each turn
    /// — and the FOE is bound: while this volatile is live on one side,
    /// `forced_action` returns `Nothing` for the other side (it can't act). Opaque to
    /// the engine.
    Trapping { move_: MoveId, turns_left: u8 },

    /// **Bide storing energy** (legacy `status1::STORING_ENERGY` + `num_attacks_left`
    /// + `bide_accumulated_damage`). The user is forced to Bide for 2 more turns,
    /// dealing no damage while it accumulates the damage it takes; when the counter
    /// runs out it unleashes `accumulated × 2` (Gen-1 bug #18: ×2, not ×3). Opaque
    /// to the engine.
    Bide { turns_left: u8, accumulated: u16 },

    /// **Transformed** marker (legacy `status3::TRANSFORMED`). A one-shot write
    /// carrier: `transform_install` copies the target's species/stats/stat-stages/
    /// moves onto the user's ENGINE battler this turn and pushes this marker;
    /// `write_party` sees it and persists the copy (species/stats/types/moves/PP=5 +
    /// the TRANSFORMED bit) into the legacy Pokémon exactly ONCE. Not re-created by
    /// `build_volatiles`, so PP depletes normally afterwards. Opaque to the engine.
    Transformed,

    /// **Type override** (Conversion; legacy `apply_conversion` copies the target's
    /// `type1`/`type2` onto the user). The engine `BattlerState` derives types SOLELY
    /// from `species`, so a type change that must NOT change species/stats/moves needs
    /// this side-carrier: [`effective_types`] consults it before falling back to
    /// [`species_types`], so STAB (attacker types), the defensive type chart, and the
    /// self-type-immunity quirk all honour the converted type. It round-trips through
    /// the BATTLE-only `BattlerState::conversion_type1/2` fields (never the persistent
    /// `Pokemon`, so a Conversion can never leak into the save) — `build_volatiles`
    /// re-creates it each turn, `write_party` persists it. Opaque to the engine.
    TypeOverride { type1: PokemonType, type2: PokemonType },
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. The provider — BattleProvider + EffectProvider + RulesProvider over the REAL
//    pokered Species / MoveId / StatIndex / PokemonType / StatusCondition.
// ─────────────────────────────────────────────────────────────────────────────

/// The pokered P1 `RulesProvider`. Carries no per-battle state; the move-effect
/// resolver returns a combined `&'static Effect` per move record (built once from
/// the compiled registry and leaked). `calculate_damage` stays the damage
/// authority — the native [`pokered_damage`] handler precomputes the full Gen-1
/// number into `ctx.mv.damage` via [`crate::battle::damage::calculate_damage`].
pub struct PokeredRules;

impl BattleProvider for PokeredRules {
    type Monster = ();
    type Move = MoveId;
    type Ability = ();
    type Status = LegacyStatus;
    type Stat = StatIndex;
    type Species = Species;
    type Type = PokemonType;
    type Item = ();

    fn calculate_damage(
        &self,
        _m: &Self::Move,
        _a: &EngineBattler<Self>,
        _d: &EngineBattler<Self>,
        _r: u8,
        _c: bool,
    ) -> DamageResult {
        // The damage number is computed inside the `ModifyDamage` handler
        // (`pokered_damage`) re-homing `calculate_damage` — the single authority.
        // This trait method is unused by the stack path (the StackDriver relies on
        // the ModifyDamage handler, not this hook), mirroring the POC provider.
        DamageResult { damage: 0, effectiveness: 1.0, is_miss: false }
    }

    fn select_move(&self, b: &EngineBattler<Self>, _s: &EngineState<Self>) -> Self::Move {
        b.moves.first().cloned().unwrap_or(MoveId::Tackle)
    }

    fn apply_move_effect(
        &self,
        _e: EngineMoveEffect,
        _u: &mut EngineBattler<Self>,
        _t: &mut EngineBattler<Self>,
    ) -> EffectResult {
        EffectResult::NoEffect
    }

    fn create_monster(&self, s: Self::Species, _l: u8) -> EngineBattler<Self> {
        EngineBattler::new(s, 100, 100, EnumMap::new(), vec![])
    }
}

impl EffectProvider for PokeredRules {
    type EffectStateKind = PokeVolatile;

    /// Every bucket-A move routes to its combined per-record `&'static Effect`,
    /// built once from the compiled registry (the move's MoveData governs the
    /// pipeline-handler draws; the move's data hooks ride the effect events).
    fn effect_for_move(&self, m: &Self::Move) -> Option<&'static Effect<Self>> {
        move_effect_for(*m)
    }

    /// Non-volatile status RESIDUAL (P6b-prereq). Burn and plain Poison each chip a
    /// flat `(max/16).max(1)` at end of the burned/poisoned mon's turn (legacy
    /// `residual.rs:29-31`); a BADLY-poisoned mon's ramp is owned by the Toxic
    /// VOLATILE (see [`effect_for_volatile`]), and `poison_residual` skips the flat
    /// tick when a Toxic volatile is live (one chip, not two). Sleep/Freeze/Paralysis
    /// carry NO residual (their cost is the BeforeMove gate, P6b-prereq stage 2), so
    /// they register no hook here.
    fn effect_for_status(&self, s: &Self::Status) -> Option<&'static Effect<Self>> {
        match s {
            LegacyStatus::Burn => Some(p5_native::burn_residual_effect()),
            LegacyStatus::Poison => Some(p5_native::poison_residual_effect()),
            _ => None,
        }
    }

    /// **P3: the nested-veto absorbers.** When the pokered-side foe-stat-down
    /// driver fires `Event::TryBoost`, the engine's [`collect_handlers`] scans the
    /// live volatiles on the boost target and asks each for its effect. A `Mist`
    /// or `Substitute` volatile hands back an effect whose single `TryBoost` hook
    /// returns `Fail` — that veto (a `Bool(false)` from `run_event_checked`) tells
    /// the driver NOT to apply the stat drop, mirroring the legacy
    /// `apply_stat_down` Mist + Substitute guards. Every other volatile registers
    /// no hook here, so this is inert for them (and the engine stays unaware of
    /// what "Mist"/"Substitute" mean — it only fetches a hook table).
    fn effect_for_volatile(&self, kind: &Self::EffectStateKind) -> Option<&'static Effect<Self>> {
        match kind {
            PokeVolatile::Mist => Some(mist_try_boost_effect()),
            PokeVolatile::Substitute | PokeVolatile::SubstituteHp { .. } => {
                Some(substitute_try_boost_effect())
            }
            // VOLATILE residuals (P6b-prereq): the driver's per-mover residual
            // aggregation ticks these on `Event::Residual` after the status residual
            // (status first, then volatiles in arena-id order — legacy
            // `apply_all_residual`: poison/burn/toxic before leech). Toxic carries the
            // uncapped ramp (#6); LeechSeed drains to the seeder. Each handler
            // self-guards on a fainted host (the legacy early-return).
            PokeVolatile::Toxic { .. } => Some(p5_native::toxic_residual_effect()),
            PokeVolatile::LeechSeed => Some(p5_native::leech_residual_effect()),
            PokeVolatile::Bide { .. } => Some(bide_residual_effect()),
            _ => None,
        }
    }

    /// Turn-order rank — re-homes the priority table → effective speed (paralysis
    /// ÷4), drawing NO rng. The driver breaks an exact tie with one coin-flip byte.
    fn turn_order_rank(
        &self,
        state: &EngineState<Self>,
        who: BattlerRef,
        action: &MoveId,
    ) -> (i32, i32) {
        let b = battler(state, who);
        let priority = move_priority(*action) as i32;
        let speed = effective_speed(b) as i32;
        (-priority, -speed)
    }

    /// Cross-turn action override (the multi-turn lock-in seam). A live volatile
    /// recorded on a PRIOR turn hijacks this turn's chosen action. Currently: a
    /// `Recharge` volatile (Hyper Beam) forces `Nothing` — the mon skips this turn.
    /// The engine names no Pokémon volatile; it only swaps one `BattleAction` for
    /// another. Charge (Fly/Dig) and lock-in (Thrash) will extend this match.
    fn forced_action(
        &self,
        effects: &[EffectState<Self>],
        actor: BattlerRef,
        chosen: &BattleAction<Self>,
    ) -> Option<BattleAction<Self>> {
        // Bound by the FOE's trapping move (Wrap/Bind/…) → this actor can't act.
        let foe = BattlerRef::new(if actor.side == 0 { 1 } else { 0 }, actor.slot);
        if effects.iter().any(|e| {
            e.host == foe
                && matches!(e.kind, PokeVolatile::Trapping { turns_left, .. } if turns_left > 0)
        }) {
            return Some(BattleAction::Nothing);
        }
        for e in effects.iter().filter(|e| e.host == actor) {
            match &e.kind {
                PokeVolatile::Recharge => {
                    return Some(BattleAction::Nothing); // Hyper Beam recharge: skip
                }
                // Trapping user → re-issue the trapping move until it runs out.
                PokeVolatile::Trapping { move_, turns_left } if *turns_left > 0 => {
                    return Some(BattleAction::Fight { move_: *move_ });
                }
                // Charge move mid-flight → the STRIKE turn is forced to re-use the
                // charging move (turn 1's gather already installed the volatile).
                PokeVolatile::Charging { move_, .. } => {
                    return Some(BattleAction::Fight { move_: *move_ });
                }
                // Rampage lock-in (Thrash/Petal Dance) → re-issue the locked move.
                PokeVolatile::LockedMove { move_, turns_left, .. } if *turns_left > 0 => {
                    return Some(BattleAction::Fight { move_: *move_ });
                }
                // Rage locks the user into Rage until it switches/faints.
                PokeVolatile::Rage => {
                    return Some(BattleAction::Fight { move_: MoveId::Rage });
                }
                // Bide re-issues while storing energy.
                PokeVolatile::Bide { .. } => {
                    return Some(BattleAction::Fight { move_: MoveId::Bide });
                }
                _ => {}
            }
        }
        let _ = chosen;
        None
    }
}

impl RulesProvider for PokeredRules {
    type Bindings = PokeredBindings;

    fn compiled(&self) -> &CompiledRuleset {
        &Self::rules_host().expect("pokered rules host installed").compiled
    }
    fn bindings(&self) -> &Self::Bindings {
        &Self::rules_host().expect("pokered rules host installed").bindings
    }
    fn rules_host() -> Option<&'static RulesHost<PokeredRules>> {
        HOST.with(|h| *h.borrow())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. The game binding — names ↔ pokered's concrete StatIndex / StatusCondition /
//    PokemonType. The type-chart fold is NEUTRAL (the damage authority owns it).
// ─────────────────────────────────────────────────────────────────────────────

/// pokered's [`RuleBindings`]: resolves the ruleset's interned stat/status/type
/// indices to concrete pokered types. All methods are pure / RNG-free (doc 11
/// §4.1).
pub struct PokeredBindings;

impl PokeredBindings {
    /// Map a ruleset stat index (the `rules.ron` `stats:` order) ↔ pokered
    /// `StatIndex`. Order MUST match `rules.ron`:
    /// `["Attack","Defense","Speed","Special","Accuracy","Evasion"]`.
    fn stat_for_index(idx: usize) -> Option<StatIndex> {
        Some(match idx {
            0 => StatIndex::Attack,
            1 => StatIndex::Defense,
            2 => StatIndex::Speed,
            3 => StatIndex::Special,
            4 => StatIndex::Accuracy,
            5 => StatIndex::Evasion,
            _ => return None,
        })
    }

    /// Map an interned status index ([`status_index_of`] order) ↔ pokered
    /// `LegacyStatus`. P2 expresses only the simple, duration-less non-volatile
    /// statuses (Poison/Burn/Freeze/Paralysis). Sleep is DEFERRED (it carries a
    /// `(rng&7).max(1)` turn count — needs the `StatusWithDuration` op, not P2).
    fn status_for_index(idx: usize) -> Option<LegacyStatus> {
        Some(match idx {
            0 => LegacyStatus::Poison,
            1 => LegacyStatus::Burn,
            2 => LegacyStatus::Freeze,
            3 => LegacyStatus::Paralysis,
            _ => return None,
        })
    }

    /// Map a `rules.ron` interned TYPE index (the `types:` list order, sequential
    /// 0..15) ↔ pokered `PokemonType` (whose discriminants are NOT sequential —
    /// Fire = 0x14, etc.). The `types:` list MUST stay in this exact order. Used by
    /// `has_type` / `move_type_is_defender_type` so the Gen-1 self-type-immunity
    /// quirk #23 compares the right type. Returns `None` for an out-of-range index.
    fn type_for_index(idx: usize) -> Option<PokemonType> {
        Some(match idx {
            0 => PokemonType::Normal,
            1 => PokemonType::Fighting,
            2 => PokemonType::Flying,
            3 => PokemonType::Poison,
            4 => PokemonType::Ground,
            5 => PokemonType::Rock,
            6 => PokemonType::Bird,
            7 => PokemonType::Bug,
            8 => PokemonType::Ghost,
            9 => PokemonType::Fire,
            10 => PokemonType::Water,
            11 => PokemonType::Grass,
            12 => PokemonType::Electric,
            13 => PokemonType::Psychic,
            14 => PokemonType::Ice,
            15 => PokemonType::Dragon,
            _ => return None,
        })
    }
}

/// Map a `rules.ron` / predicate volatile NAME to whether a live `PokeVolatile`
/// arena entry IS that volatile. The single game-side source of truth for the
/// volatile vocabulary — used by `has_volatile` for `HasVolatile(...)` de-dup /
/// immunity guards. The engine treats the arena opaquely; only this map knows
/// which entry a name refers to.
fn volatile_name_matches(name: &str, kind: &PokeVolatile) -> bool {
    match name {
        "Substitute" => matches!(
            kind,
            PokeVolatile::Substitute | PokeVolatile::SubstituteHp { .. }
        ),
        "confusion" => matches!(kind, PokeVolatile::Confused { .. }),
        "leechseed" => matches!(kind, PokeVolatile::LeechSeed),
        "toxic" => matches!(kind, PokeVolatile::Toxic { .. }),
        "flinch" => matches!(kind, PokeVolatile::Flinched),
        "focusenergy" => matches!(kind, PokeVolatile::FocusEnergy),
        "lightscreen" => matches!(kind, PokeVolatile::LightScreen),
        "reflect" => matches!(kind, PokeVolatile::Reflect),
        "mist" => matches!(kind, PokeVolatile::Mist),
        _ => false,
    }
}

impl RuleBindings<PokeredRules> for PokeredBindings {
    /// Apply a signed stat-stage delta to `b` for the interned `stat_index`, with
    /// the engine's standing `-6..=+6` stage clamp (Gen-1 bug #30) — exactly the
    /// clamp the legacy `StatStages::modify` enforces, so the boost is parity by
    /// construction. Returns `false` for an unknown index (defense-in-depth).
    fn apply_boost(&self, b: &mut EngineBattler<PokeredRules>, stat_index: usize, stages: i8) -> bool {
        let Some(stat) = Self::stat_for_index(stat_index) else { return false };
        let cur = b.stat_stages.get(stat).copied().unwrap_or(0);
        let next = (cur as i16 + stages as i16).clamp(-6, 6) as i8;
        b.stat_stages.set(stat, next);
        // The stat-up glitch (effects.asm:499/689): whenever a stat stage ACTUALLY
        // changed, `ApplyBadgeStatBoosts` re-applies the badge boosts to ALL of
        // the player's working stats. Every stage change in the production stack
        // funnels through here; the hook is inert unless `b` carries the seeded
        // player badge context (enemy battlers and badge-less games: zero change).
        // (A clamped no-change — "+6 already" / Mist-vetoed — takes the asm's
        // "nothing happened" path, which does NOT re-apply the boosts.)
        if next != cur {
            crate::battle::badge_boosts::reapply_on_stage_change(b, stat);
        }
        true
    }

    /// Set `b`'s non-volatile status for the interned `status_index` (P2 side-status
    /// + primary-status). **Mirrors the legacy "no-status" guard**: a target that
    /// already has ANY non-volatile status is NOT re-statused (the
    /// `if !d_mon.status.is_none() { return StatusFailed }` branch in
    /// `status_effects.rs`). This keeps the no-status guard game-side (the engine
    /// stays status-agnostic) so no `HasAnyStatus` predicate is needed. Returns
    /// `false` if the index is unknown or the target is already statused.
    fn set_status(&self, b: &mut EngineBattler<PokeredRules>, status_index: usize) -> bool {
        let Some(status) = Self::status_for_index(status_index) else { return false };
        // The legacy no-status guard (status_effects.rs): only inflict on a
        // status-free target. `None` / no status ⇒ inflict; else fail (no-op).
        if b.status.is_some() {
            return false;
        }
        b.status = Some(status);
        true
    }

    /// Set a status carrying a turn count — the Gen-1 Sleep case (the engine
    /// resolved the turns from the op's `AmountSpec`, drawing its own rng). Sleep
    /// is the only status whose value depends on the amount; everything else
    /// delegates to [`set_status`](Self::set_status). Same no-status guard.
    fn set_status_with_amount(
        &self,
        b: &mut EngineBattler<PokeredRules>,
        status_index: usize,
        amount: u16,
    ) -> bool {
        if status_index == SLEEP_STATUS_INDEX {
            if b.status.is_some() {
                return false;
            }
            b.status = Some(LegacyStatus::Sleep(amount as u8));
            true
        } else {
            self.set_status(b, status_index)
        }
    }

    /// Build the pokered `PokeVolatile` for a `rules.ron` volatile name + the
    /// engine-resolved amount (turns / counter seed) — the `InflictVolatile` op.
    /// This is the ONLY place the volatile vocabulary maps to the game enum; the
    /// engine installs whatever is returned opaquely. Unknown name ⇒ `None`
    /// (inert). Pure — the engine already drew any rng.
    fn make_volatile(&self, name: &str, amount: u16) -> Option<PokeVolatile> {
        Some(match name {
            "confusion" => PokeVolatile::Confused {
                turns: amount as u8,
            },
            "leechseed" => PokeVolatile::LeechSeed,
            "toxic" => PokeVolatile::Toxic {
                counter: amount as u8,
            },
            "flinch" => PokeVolatile::Flinched,
            "focusenergy" => PokeVolatile::FocusEnergy,
            "lightscreen" => PokeVolatile::LightScreen,
            "reflect" => PokeVolatile::Reflect,
            "mist" => PokeVolatile::Mist,
            _ => return None,
        })
    }

    /// `HasType` membership (the `VetoIf(HasType("Poison"))` poison-side immunity).
    /// `type_index` is the `rules.ron` interned `types:` index; map it to the
    /// concrete `PokemonType` (the discriminants are non-sequential) before
    /// comparing the species' two types. Pure read.
    fn has_type(&self, b: &EngineBattler<PokeredRules>, type_index: usize) -> bool {
        let Some(want) = Self::type_for_index(type_index) else { return false };
        let (t1, t2) = species_types(b.species);
        want == t1 || want == t2
    }

    /// `HasVolatile` — the side-status Substitute block. Maps the name
    /// `"Substitute"` ↔ a live [`PokeVolatile::Substitute`] arena entry hosted on
    /// `who`. The engine treats the arena opaquely; only the game knows which entry
    /// IS the Substitute. Pure read; no entropy.
    fn has_volatile(
        &self,
        ctx: &BattleCtx<'_, PokeredRules>,
        who: BattlerRef,
        name: &str,
    ) -> bool {
        ctx.effects
            .iter()
            .any(|e| e.host == who && volatile_name_matches(name, &e.kind))
    }

    /// `redirect_hp_loss` — Substitute vs the DIRECT-MUTATE ops (Super Fang / OHKO /
    /// multi-hit). The formula path already routes through `Event::Damage`; these ops
    /// apply HP outside it, so the interpreter asks here whether `who`'s doll should
    /// swallow the loss. It does — EXCEPT self-inflicted loss (recoil / Explosion
    /// self-KO, where `who == source`): the mover's OWN doll never absorbs its own
    /// recoil/detonation (Gen-1). Matches the oracle absorb (mon-based damage number
    /// routed into the doll, e.g. Super Fang's `curHP/2` from the MON hits the doll;
    /// OHKO's full-HP loss breaks it). Mutates the arena via [`absorb_into_substitute`].
    fn redirect_hp_loss(
        &self,
        ctx: &mut BattleCtx<'_, PokeredRules>,
        who: BattlerRef,
        source: BattlerRef,
        amount: u16,
    ) -> bool {
        if who == source {
            return false; // self-inflicted (recoil / self-KO) bypasses one's own doll
        }
        absorb_into_substitute(ctx, who, amount)
    }

    /// `MoveTypeIsDefenderType` — the Gen-1 burn/freeze/paralyze self-type-immunity
    /// quirk #23 (`status_effects.rs:85/110/135`): a defender whose own type matches
    /// the move's type cannot be afflicted. `move_type_index` is the record's `type:`
    /// interned index. Pure read.
    fn move_type_is_defender_type(
        &self,
        ctx: &BattleCtx<'_, PokeredRules>,
        move_type_index: usize,
        who: BattlerRef,
    ) -> bool {
        let Some(mt) = Self::type_for_index(move_type_index) else { return false };
        // Honour Conversion: the defender's overridden types drive the quirk.
        let (t1, t2) = effective_types(ctx, who);
        mt == t1 || mt == t2
    }

    /// `TargetHasStatus` — the Dream Eater sleep gate. P2 maps only the simple
    /// statuses; Sleep is deferred (so a `TargetHasStatus("sleep")` would need the
    /// deferred sleep mapping). Pure read.
    fn has_status(&self, b: &EngineBattler<PokeredRules>, status_index: usize) -> bool {
        if status_index == SLEEP_STATUS_INDEX {
            // Sleep carries a turn count, so match ANY Sleep(_) (the Dream Eater
            // "target is asleep" gate).
            return matches!(b.status, Some(LegacyStatus::Sleep(_)));
        }
        match Self::status_for_index(status_index) {
            Some(want) => b.status == Some(want),
            None => false,
        }
    }

    /// `TargetHasAnyStatus` — the Toxic "already-statused ⇒ nothing happens"
    /// guard. Any non-volatile status counts. Pure read.
    fn has_any_status(&self, b: &EngineBattler<PokeredRules>) -> bool {
        b.status.is_some()
    }

    /// `battler_level` (P3): the per-battler level the OHKO `LevelGE` gate + the
    /// level-based `SetDamage` sources (Seismic Toss / Night Shade / Psywave) read.
    /// The engine [`BattlerState`] carries no level field (the engine is
    /// level-agnostic), so the harness records each species' level in a thread-local
    /// (`set_level`); we look it up by species here. Defaults to 50 (the P1/P2
    /// fixed level) when the harness set nothing. Pure read; no entropy.
    fn battler_level(&self, b: &EngineBattler<PokeredRules>) -> u16 {
        level_for_species(b.species)
    }

    /// **The chart fold is NEUTRAL by design (the damage authority owns it).** The
    /// real Gen-1 super-effective / resisted / STAB numbers come out of
    /// `calculate_damage` (folded into `ctx.mv.damage` before the `Effectiveness`
    /// event). `ApplyTypeChart` therefore resolves `(1,1)` here — re-applying the
    /// chart would double-count. (When P-later moves a chart relation INTO the
    /// data layer, this returns the compiled `chart_mult`; for P1 it is `(1,1)`.)
    fn type_chart_mult(
        &self,
        _ctx: &BattleCtx<'_, PokeredRules>,
        _move_type_index: usize,
        _defender: BattlerRef,
    ) -> (u32, u32) {
        (1, 1)
    }
}

/// status-name → pokered status index (the closed status vocabulary the loader
/// validates `InflictStatus` / `TargetHasStatus` against; P2 side-status +
/// primary-status). Order MUST match [`PokeredBindings::status_for_index`]. Sleep
/// is DEFERRED (needs the `StatusWithDuration` op for its turn count), so no
/// `"sleep"` name is bound here.
/// Interned index of the Sleep status. Sleep carries a turn count, so it is set
/// ONLY through [`RuleBindings::set_status_with_amount`] (never the plain
/// `set_status`); `status_for_index` deliberately returns `None` for it.
pub const SLEEP_STATUS_INDEX: usize = 4;

pub fn status_index_of(name: &str) -> Option<usize> {
    Some(match name {
        "poison" => 0,
        "burn" => 1,
        "freeze" => 2,
        "paralysis" => 3,
        "sleep" => SLEEP_STATUS_INDEX,
        _ => return None,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. The RulesProvider bridge — a thread-local `&'static RulesHost`.
// ─────────────────────────────────────────────────────────────────────────────

thread_local! {
    static HOST: RefCell<Option<&'static RulesHost<PokeredRules>>> = const { RefCell::new(None) };
}

/// Install (or hot-swap) the compiled registry the interpreter reads. Leaks a
/// fresh `&'static RulesHost`; a reload points the slot at the new one. Safe
/// mid-battle because live `EffectState` is in the engine arena, not here.
/// Thread-local so the parallel test harness stays isolated.
pub fn install_compiled(compiled: CompiledRuleset) {
    let host = RulesHost::new(compiled, PokeredBindings);
    let leaked: &'static RulesHost<PokeredRules> = Box::leak(Box::new(host));
    HOST.with(|h| *h.borrow_mut() = Some(leaked));
    // Rebuild the per-record op-list index + combined move effects on this thread.
    rebuild_move_index();
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Dual-mode loading (doc 11 §4.2): one rules.ron, two access modes.
// ─────────────────────────────────────────────────────────────────────────────

/// Build the dual-mode [`RuleSource`]. `hot=false` (or no `hot-reload` feature) ⇒
/// BAKED (`include_str!`, zero file IO); `hot=true` (with the feature) ⇒ DISK
/// (read + watch). Both yield the SAME [`Ruleset`] when the on-disk file matches
/// the baked text (the dual-mode invariant).
pub fn load_ruleset(hot: bool) -> RuleSource {
    if hot {
        RuleSource::from_path(RULES_RON_PATH)
    } else {
        RuleSource::baked(RULES_RON_BAKED)
    }
}

/// Compile a [`Ruleset`] into pokered's registry (names→indices + status
/// vocabulary). Validates every name against the closed vocabulary NOW; an
/// unknown name is a load error, never a battle-time surprise (doc 11 §4.2).
pub fn compile(ruleset: &Ruleset) -> Result<CompiledRuleset, jrpg_rules::LoadError> {
    CompiledRuleset::compile::<PokeredRules, PokeredBindings>(
        ruleset,
        DATA_ID_BASE,
        &PokeredBindings,
        status_index_of,
    )
}

/// Install the canonical compiled ruleset on the current thread (idempotent;
/// rebuilds the combined move-effect index). The harness calls this FIRST on each
/// test thread (the thread-local host is per-thread; test threads are pooled).
pub fn install_canonical() {
    let ruleset = load_ruleset(false).load().expect("baked pokered rules.ron parses");
    let compiled = compile(&ruleset).expect("pokered rules.ron compiles");
    install_compiled(compiled);
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. The per-move combined effect + the (record, event) op-list index.
//
//    The StackDriver fires ONE move effect via `effect_for_move`, threading that
//    effect's id to EVERY hook as `source_effect` (dispatch.rs:128). The
//    `jrpg_rules::interpret` bridge keys an op-list by `source_effect` ALONE — so
//    a move with hooks on several events would collapse to one op-list. We
//    therefore build OUR OWN per-record index keyed by `(record_index, Event)`
//    and a small set of per-event bridge fns (one per event the bucket-A moves
//    use) that look the op-list up by the combined effect's id (= the record) +
//    the bridge's own static event, then fold it via `jrpg_rules::run_ops`. This
//    is the same Option-A "data is read by source_effect" shape minimon uses,
//    extended to disambiguate by event so a single StackDriver move-effect call
//    drives all of a move's data hooks.
// ─────────────────────────────────────────────────────────────────────────────

/// One compiled move record: its `MoveData`-shaped pipeline inputs (recovered by
/// the harness from the REAL `MoveData::get`) + its per-event op-lists.
struct MoveRecord {
    /// The rules.ron record id string (e.g. `"move.surf"`).
    source_id: String,
    /// Per-event compiled hooks (the op-list + chance gate) for this record.
    hooks: Vec<CompiledHook>,
}

thread_local! {
    /// `record_index → MoveRecord` (op-lists), and `MoveId → record_index`
    /// resolution is via the harness binding ([`record_for_move`]). Rebuilt on
    /// every `install_compiled` so a hot-reload swaps the index in lockstep.
    static MOVE_RECORDS: RefCell<Vec<MoveRecord>> = const { RefCell::new(Vec::new()) };
    /// The leaked combined `&'static Effect` per record index (built once per
    /// install; the engine requires `&'static`).
    static MOVE_EFFECTS: RefCell<Vec<&'static Effect<PokeredRules>>> =
        const { RefCell::new(Vec::new()) };
    /// The per-record active MoveData (power/type/accuracy/effect) the pipeline
    /// handlers read. Set by the harness before driving a turn (mirrors the
    /// stack_parity active-move thread-local).
    static ACTIVE_MOVE: RefCell<Option<MoveData>> = const { RefCell::new(None) };
}

/// Map a `MoveId` to its rules.ron record id (the bucket-A authored moves). The
/// harness's scenarios pick from these; an unmapped move falls through to a
/// pure-damage default (so any pure-damage `MoveId` works with the right MoveData).
fn record_id_for_move(m: MoveId) -> &'static str {
    match m {
        MoveId::Surf => "move.surf",
        MoveId::Earthquake => "move.earthquake",
        MoveId::Swift => "move.swift",
        MoveId::SwordsDance => "boost.attack_up_2",
        MoveId::Amnesia => "boost.special_up_2",
        MoveId::Agility => "boost.speed_up_2",
        MoveId::Growth => "boost.special_up_1",
        MoveId::Harden | MoveId::Withdraw | MoveId::DefenseCurl => "boost.defense_up_1",
        MoveId::Meditate | MoveId::Sharpen => "boost.attack_up_1",
        MoveId::DoubleTeam | MoveId::Minimize => "boost.evasion_up_1",
        MoveId::AcidArmor | MoveId::Barrier => "boost.defense_up_2",
        MoveId::Recover | MoveId::Softboiled => "heal.recover",
        MoveId::Splash => "move.splash",
        // ── P2 side-status moves (chance-gated InflictStatus) ──
        MoveId::PoisonSting => "side.poison_1",   // PoisonSideEffect1, 51/256
        MoveId::Sludge | MoveId::Smog => "side.poison_2", // PoisonSideEffect2, 102/256
        MoveId::Ember => "side.burn_1",           // BurnSideEffect1, 26/256
        MoveId::FireBlast => "side.burn_2",       // BurnSideEffect2, 77/256
        MoveId::IceBeam | MoveId::Blizzard => "side.freeze_1", // FreezeSideEffect1, 26/256
        MoveId::Thunder => "side.paralyze_1",     // ParalyzeSideEffect1, 26/256
        MoveId::BodySlam => "side.paralyze_2",    // ParalyzeSideEffect2, 77/256
        // Group-A additions (P1B): more damaging side-effect riders reusing the
        // existing side.* records (10% = _1, 30% = _2).
        MoveId::Flamethrower | MoveId::FirePunch => "side.burn_1",
        MoveId::IcePunch => "side.freeze_1",
        MoveId::Thunderbolt | MoveId::Thundershock | MoveId::Thunderpunch => "side.paralyze_1",
        MoveId::Lick => "side.paralyze_2",
        // ── P2 primary-status moves (guaranteed, power-0) ──
        MoveId::ThunderWave => "status.paralyze", // ParalyzeEffect
        MoveId::StunSpore | MoveId::Glare => "status.paralyze",
        MoveId::Poisonpowder => "status.poison",  // PoisonEffect (plain branch)
        MoveId::PoisonGas => "status.poison",
        // ── P1B batch 4: sleep / confusion / leech-seed via the decoupled
        //    InflictStatus(amount) / InflictVolatile ops (engine stays generic).
        MoveId::Hypnosis | MoveId::Sing | MoveId::SleepPowder | MoveId::LovelyKiss
        | MoveId::Spore => "status.sleep",
        MoveId::ConfuseRay | MoveId::Supersonic => "status.confuse",
        MoveId::Confusion | MoveId::Psybeam | MoveId::DizzyPunch => "side.confuse",
        MoveId::LeechSeed => "status.leechseed",
        MoveId::Bite | MoveId::BoneClub | MoveId::HyperFang => "side.flinch_1",
        MoveId::Stomp | MoveId::RollingKick | MoveId::Headbutt | MoveId::LowKick => "side.flinch_2",
        MoveId::Toxic => "status.toxic",
        MoveId::Rest => "status.rest",
        MoveId::FocusEnergy => "status.focus_energy",
        MoveId::Explosion | MoveId::Selfdestruct => "special.explosion",
        MoveId::LightScreen => "status.lightscreen",
        MoveId::Reflect => "status.reflect",
        MoveId::Mist => "status.mist",
        MoveId::Counter => "special.counter",
        MoveId::Transform => "status.transform",
        MoveId::Mimic => "move.mimic",
        MoveId::Haze => "status.haze",
        MoveId::Substitute => "status.substitute",
        MoveId::Conversion => "status.conversion",
        MoveId::Disable => "status.disable",
        MoveId::Whirlwind | MoveId::Roar | MoveId::Teleport => "special.switch_teleport",
        // ── P2 drain / recoil ──
        MoveId::MegaDrain | MoveId::Absorb | MoveId::LeechLife => "drain.absorb", // DrainHpEffect
        MoveId::TakeDown | MoveId::DoubleEdge | MoveId::Submission => "recoil.take_down", // RecoilEffect
        MoveId::Struggle => "recoil.struggle",
        MoveId::DreamEater => "dream.eater",
        // ── P3 special / fixed / OHKO / Super Fang ──
        MoveId::SeismicToss | MoveId::NightShade => "special.user_level", // SpecialDamageEffect (level)
        MoveId::DragonRage => "special.const_40",  // SpecialDamageEffect (40)
        MoveId::Sonicboom => "special.const_20",   // SpecialDamageEffect (20)
        MoveId::Psywave => "special.psywave",      // SpecialDamageEffect (rng·1.5·lvl)
        MoveId::SuperFang => "special.super_fang", // SuperFangEffect (curHP/2)
        MoveId::HornDrill | MoveId::Guillotine | MoveId::Fissure => "special.ohko", // OhkoEffect (#19)
        // ── P3 foe stat-down (nested-veto driver fires TryBoost → Mist/Sub veto) ──
        MoveId::Growl => "foedown.attack_1",       // AttackDown1Effect
        MoveId::Leer | MoveId::TailWhip => "foedown.defense_1", // DefenseDown1Effect
        MoveId::StringShot => "foedown.speed_1",   // SpeedDown1Effect
        MoveId::SandAttack | MoveId::Flash | MoveId::Kinesis | MoveId::Smokescreen => {
            "foedown.accuracy_1" // AccuracyDown1Effect
        }
        MoveId::Screech => "foedown.defense_2",    // DefenseDown2Effect (-2)
        MoveId::AuroraBeam => "foedown.attack_side",   // AttackDownSideEffect
        MoveId::Acid => "foedown.defense_side",        // DefenseDownSideEffect
        MoveId::Bubble | MoveId::Bubblebeam | MoveId::Constrict => "foedown.speed_side", // SpeedDownSideEffect
        MoveId::PsychicM => "foedown.special_side",    // SpecialDownSideEffect
        // ── P4 multi-hit (RepeatHits seam) ──
        MoveId::Doubleslap => "multi.doubleslap",     // TwoToFiveAttacksEffect
        MoveId::CometPunch => "multi.comet_punch",    // TwoToFiveAttacksEffect
        MoveId::FuryAttack => "multi.fury_attack",    // TwoToFiveAttacksEffect
        MoveId::PinMissile => "multi.pin_missile",    // TwoToFiveAttacksEffect
        MoveId::SpikeCannon => "multi.spike_cannon",  // TwoToFiveAttacksEffect
        MoveId::Barrage => "multi.barrage",           // TwoToFiveAttacksEffect
        MoveId::FurySwipes => "multi.fury_swipes",    // TwoToFiveAttacksEffect
        MoveId::DoubleKick => "multi.double_kick",    // AttackTwiceEffect (Fixed 2)
        MoveId::Bonemerang => "multi.bonemerang",     // AttackTwiceEffect (Fixed 2)
        MoveId::Twineedle => "multi.twineedle",       // TwineedleEffect (Fixed 2 + poison)
        // Any other move id is treated as a pure-damage move record (Tackle's
        // shape); the active MoveData governs the actual power/type/accuracy.
        _ => "move.tackle",
    }
}

/// Rebuild the per-record op-list index + the leaked combined move effects from
/// the installed compiled registry. Called by [`install_compiled`].
fn rebuild_move_index() {
    let host = PokeredRules::rules_host().expect("pokered rules host installed");
    // Group the compiled hooks by their owning record source_id, in a stable,
    // deterministic order (sorted by source_id, then by synthesized hook id).
    let mut by_source: std::collections::BTreeMap<String, Vec<CompiledHook>> =
        std::collections::BTreeMap::new();
    let mut hooks: Vec<CompiledHook> = host.compiled.hooks.values().cloned().collect();
    hooks.sort_by_key(|h| h.id.0);
    for h in hooks {
        by_source.entry(h.source_id.clone()).or_default().push(h);
    }
    // Also register the no-op records (Splash) that carry no hooks, so a MoveId →
    // record_id mapping always resolves.
    let mut records: Vec<MoveRecord> = Vec::new();
    for (source_id, hs) in by_source {
        records.push(MoveRecord { source_id, hooks: hs });
    }
    // Ensure every authored record id has a slot even if hook-less (Splash), plus
    // the fully-native records whose behaviour is a native handler, not data ops
    // (Counter reflects via `counter_handler`, registered in the rebuild loop below).
    for sid in ["move.splash", "special.counter", "status.transform", "move.mimic", "status.haze", "special.switch_teleport", "status.substitute", "status.conversion", "status.disable"] {
        if !records.iter().any(|r| r.source_id == sid) {
            records.push(MoveRecord { source_id: sid.to_string(), hooks: Vec::new() });
        }
    }
    records.sort_by(|a, b| a.source_id.cmp(&b.source_id));

    // Build a combined `&'static Effect` per record. Each record's effect id is
    // `MOVE_EFFECT_ID_BASE + record_index`; the bridge recovers the record by
    // subtracting the base. Every record gets the FULL pipeline hooks (crit /
    // accuracy / damage = the native draws) PLUS one bridge hook per event the
    // record subscribes to (ModifyDamage / Effectiveness / AfterMove). Power-0
    // records still register the pipeline hooks; the handlers short-circuit on
    // power-0 (drawing only the accuracy byte) to match the legacy power-0 branch.
    let mut effects: Vec<&'static Effect<PokeredRules>> = Vec::new();
    for (idx, rec) in records.iter().enumerate() {
        let id = EffectId(MOVE_EFFECT_ID_BASE + idx as u32);
        let mut event_hooks: Vec<EventHook<PokeredRules>> = Vec::new();
        // ── Native pipeline (the DRAW structure, re-homing pokered's formula). ──
        event_hooks.push(EventHook {
            event: Event::ModifyCritRatio,
            call: pokered_crit,
            order: u32::MAX,
            priority: 0,
            sub_order: None,
        });
        event_hooks.push(EventHook {
            event: Event::Accuracy,
            call: pokered_accuracy,
            order: u32::MAX,
            priority: 0,
            sub_order: None,
        });
        event_hooks.push(EventHook {
            event: Event::ModifyDamage,
            call: pokered_damage,
            order: 1000,
            priority: 0,
            sub_order: None,
        });
        // ── BeforeMove status/volatile GATES (P6b-prereq stage 2). Every move
        //    carries the four Gen-1 pre-move gates; each reads the MOVER's own
        //    status/volatile and is INERT (no draw) when absent, so a no-status
        //    mover is byte-identical. Orders sleep(10) < freeze(20) < confusion(70)
        //    < paralysis(90) reproduce the ASM / MoveRandoms field order (confusion
        //    byte before paralysis byte, both before crit); run_event's short-circuit
        //    on the first Fail makes a confusion self-hit stop before the paralysis
        //    gate fires.
        for (order, call) in [
            (10u32, p5_native::sleep_gate as jrpg_engine::battle::stack::HandlerFn<PokeredRules>),
            (20, p5_native::freeze_gate),
            (30, p5_native::flinch_gate),
            // Disable: the ASM decrements the counter (step 6) then blocks the disabled
            // move (step 8), straddling confusion (step 7) — orders 50 / 80 reproduce
            // that, and run_event's short-circuit means a sleeping/frozen mover never
            // reaches the decrement (matching the ASM early-return).
            (50, disable_decrement_gate),
            (70, p5_native::confusion_gate),
            (80, disable_veto_gate),
            (90, p5_native::paralysis_gate),
        ] {
            event_hooks.push(EventHook {
                event: Event::BeforeMove,
                call,
                order,
                priority: 0,
                sub_order: None,
            });
        }
        // ── Counter's per-turn damage-taken recorder (bug #20). On DamagingHit,
        //    stamp the DEFENDER's `DamageTaken` scratch so a Counter user (moving
        //    last, −1 priority) can read the physical damage it took this turn. High
        //    order → runs after any damage-adjusting DamagingHit hook. INERT unless a
        //    Counter follows (the entry is dropped at write-back). ──
        event_hooks.push(EventHook {
            event: Event::DamagingHit,
            call: record_damage_taken,
            order: u32::MAX - 1,
            priority: 0,
            sub_order: None,
        });
        // ── Hyper Beam recharge install (bug #14). On DamagingHit, a connecting
        //    Hyper Beam sets the user's Recharge volatile (unless it KO'd). Keyed on
        //    the active move's effect → inert for every other move. ──
        event_hooks.push(EventHook {
            event: Event::DamagingHit,
            call: hyperbeam_recharge_install,
            order: u32::MAX - 2,
            priority: 0,
            sub_order: None,
        });
        // ── Thrash / Petal Dance lock-in (install/decrement + end-confuse). Keyed on
        //    ThrashPetalDanceEffect → inert for other moves. ──
        event_hooks.push(EventHook {
            event: Event::DamagingHit,
            call: thrash_lockin,
            order: u32::MAX - 3,
            priority: 0,
            sub_order: None,
        });
        // ── Rage: lock-in on use + Attack-up when the raging mon is hit. ──
        event_hooks.push(EventHook {
            event: Event::DamagingHit,
            call: rage_manage,
            order: u32::MAX - 4,
            priority: 0,
            sub_order: None,
        });
        // ── Trapping moves (Wrap/Bind/Fire Spin/Clamp): lock-in + counter. ──
        event_hooks.push(EventHook {
            event: Event::DamagingHit,
            call: trapping_lockin,
            order: u32::MAX - 5,
            priority: 0,
            sub_order: None,
        });
        // ── Transform: copy the target's identity onto the user. ──
        event_hooks.push(EventHook {
            event: Event::DamagingHit,
            call: transform_install,
            order: u32::MAX - 6,
            priority: 0,
            sub_order: None,
        });
        // ── Haze: reset both sides' stat stages/status + wipe select volatiles. ──
        event_hooks.push(EventHook {
            event: Event::DamagingHit,
            call: haze_reset,
            order: u32::MAX - 7,
            priority: 0,
            sub_order: None,
        });
        // ── Conversion: copy the target's types onto the user (TypeOverride). ──
        event_hooks.push(EventHook {
            event: Event::DamagingHit,
            call: conversion_install,
            order: u32::MAX - 8,
            priority: 0,
            sub_order: None,
        });
        // ── Disable: disable the target's last-used move slot. ──
        event_hooks.push(EventHook {
            event: Event::DamagingHit,
            call: disable_install,
            order: u32::MAX - 9,
            priority: 0,
            sub_order: None,
        });
        // ── Substitute: create the doll on use (self, keyed on SubstituteEffect). ──
        event_hooks.push(EventHook {
            event: Event::DamagingHit,
            call: substitute_install,
            order: u32::MAX - 10,
            priority: 0,
            sub_order: None,
        });
        // ── CheckDefrost (effects.asm:312-330): a Fire-type FreezeBurnParalyze-
        //    family move defrosts a frozen target it hits. Fires AFTER the RON
        //    side-status hooks (same order u32::MAX, priority −1 ⇒ strictly
        //    later): those hooks CAN'T inflict on an already-statused target (the
        //    binding guards), so curing afterwards exactly reproduces the
        //    original's outcome — defrosted, never burned. Keyed on move
        //    type+effect → inert for every other move; draws no rng. ──
        event_hooks.push(EventHook {
            event: Event::DamagingHit,
            call: pokered_defrost,
            order: u32::MAX,
            priority: -1,
            sub_order: None,
        });
        // ── Substitute: absorb the incoming hit into the doll's HP pool (the reserved
        //    Event::Damage seam, fired by the driver before the hp write). Inert unless
        //    the defender holds a Substitute. ──
        event_hooks.push(EventHook {
            event: Event::Damage,
            call: substitute_absorb,
            order: 100,
            priority: 0,
            sub_order: None,
        });
        // ── A charge move that misses on its strike turn still ends the charge. ──
        event_hooks.push(EventHook {
            event: Event::OnMiss,
            call: charge_miss_land,
            order: 100,
            priority: 0,
            sub_order: None,
        });
        // ── Jump Kick / Hi Jump Kick crash (1 HP) on a miss. ──
        event_hooks.push(EventHook {
            event: Event::OnMiss,
            call: jump_kick_crash,
            order: 110,
            priority: 0,
            sub_order: None,
        });
        // ── Counter itself: the reflect-2×-physical handler on ModifyDamage (only
        //    the special.counter record). power-0 record → the native crit/damage
        //    draws already short-circuit; pokered_accuracy skips Counter; this native
        //    handler is Counter's sole damage authority (reads its own DamageTaken). ──
        if rec.source_id == "special.counter" {
            event_hooks.push(EventHook {
                event: Event::ModifyDamage,
                call: counter_handler,
                order: 1500, // after the native pokered_damage (1000, which no-ops on power-0)
                priority: 0,
                sub_order: None,
            });
        }
        // ── Data bridge hooks (the EFFECT op-lists, by event). ──
        for h in &rec.hooks {
            // A DamagingHit hook whose op-list carries a foe-directed `Boost`
            // (target Target/Foe, NOT Source) is a foe stat-down — route it to the
            // pokered-side nested-veto driver (`bridge_foe_stat_down`) which fires
            // Event::TryBoost so Mist/Substitute can veto. A self-Boost (Source)
            // and every other DamagingHit hook take the plain data bridge.
            let is_foe_stat_down = h.event == Event::DamagingHit
                && h.ops.iter().any(|op| {
                    matches!(
                        op,
                        jrpg_rules::Op::Boost {
                            target: jrpg_rules::Selector::Target | jrpg_rules::Selector::Foe,
                            ..
                        }
                    )
                });
            let call: jrpg_engine::battle::stack::HandlerFn<PokeredRules> = match h.event {
                Event::ModifyDamage => bridge_modify_damage,
                Event::Effectiveness => bridge_effectiveness,
                Event::AfterMove => bridge_after_move,
                Event::Accuracy => bridge_accuracy,
                Event::DamagingHit if is_foe_stat_down => bridge_foe_stat_down,
                Event::DamagingHit => bridge_damaging_hit,
                // Any other event the bucket-A set does not use falls back to the
                // generic after-move bridge (still keyed by (record, event)).
                _ => bridge_after_move,
            };
            // ModifyDamage's DealMoveDamage op is a marker the native
            // `pokered_damage` already realizes; we still register the bridge so
            // the op-list is genuinely read (it resolves Unchanged), but at a LOW
            // order so it never perturbs the native damage write. For other events
            // the bridge is the sole subscriber.
            let order = match h.event {
                Event::ModifyDamage => 2000, // after the native damage write (1000)
                Event::Effectiveness => h.order,
                _ => h.order,
            };
            event_hooks.push(EventHook {
                event: h.event,
                call,
                order,
                priority: h.priority,
                sub_order: None,
            });
        }
        let leaked_hooks: &'static [EventHook<PokeredRules>] =
            Box::leak(event_hooks.into_boxed_slice());
        let eff: &'static Effect<PokeredRules> = Box::leak(Box::new(Effect {
            id,
            kind: EffectType::Move,
            hooks: leaked_hooks,
        }));
        effects.push(eff);
    }

    MOVE_RECORDS.with(|r| *r.borrow_mut() = records);
    MOVE_EFFECTS.with(|e| *e.borrow_mut() = effects);
}

/// The record index for a `MoveId` (via its record id string). `None` if no
/// registry is installed.
fn record_index_for_move(m: MoveId) -> Option<usize> {
    let sid = record_id_for_move(m);
    MOVE_RECORDS.with(|r| r.borrow().iter().position(|rec| rec.source_id == sid))
}

/// The combined `&'static Effect` for a `MoveId` (its record). `None` if no
/// registry is installed or the move maps to no record.
pub fn move_effect_for(m: MoveId) -> Option<&'static Effect<PokeredRules>> {
    let idx = record_index_for_move(m)?;
    MOVE_EFFECTS.with(|e| e.borrow().get(idx).copied())
}

/// Look up a record's compiled hook for a given event (the bridge's lookup),
/// recovering the record from the combined effect's `source_effect` id.
fn hook_for(source_effect: EffectId, event: Event) -> Option<CompiledHook> {
    let rec_idx = source_effect.0.checked_sub(MOVE_EFFECT_ID_BASE)? as usize;
    MOVE_RECORDS.with(|r| {
        r.borrow()
            .get(rec_idx)
            .and_then(|rec| rec.hooks.iter().find(|h| h.event == event).cloned())
    })
}

thread_local! {
    /// Per-species level the P3 `battler_level` binding reads (the OHKO `LevelGE`
    /// gate + level-based `SetDamage`). The harness records each species' level;
    /// an unrecorded species defaults to 50 (the P1/P2 fixed level). Keyed by
    /// species so the player and opponent can differ when they use distinct species
    /// (the OHKO / Seismic Toss differential scenarios). Thread-local so the
    /// parallel test harness stays isolated.
    static LEVELS: RefCell<std::collections::HashMap<Species, u16>> =
        RefCell::new(std::collections::HashMap::new());
}

/// Record a species' level for the P3 `battler_level` binding (harness-only).
pub fn set_level(species: Species, level: u16) {
    LEVELS.with(|m| {
        m.borrow_mut().insert(species, level);
    });
}

/// Clear all recorded levels (harness resets before each P3 scenario).
pub fn clear_levels() {
    LEVELS.with(|m| m.borrow_mut().clear());
}

/// The level recorded for a species (defaults to 50 — the P1/P2 fixed level).
fn level_for_species(species: Species) -> u16 {
    LEVELS.with(|m| m.borrow().get(&species).copied().unwrap_or(50))
}

/// Public re-export of [`level_for_species`] for the P5 native PayDay handler
/// (the coin award is `level·2`). Harness-only.
pub fn level_for_species_pub(species: Species) -> u16 {
    level_for_species(species)
}

/// Set the per-turn active MoveData (power/type/accuracy/effect) the pipeline
/// handlers read. Mirrors the stack_parity active-move thread-local.
pub fn set_active_move(m: MoveData) {
    ACTIVE_MOVE.with(|c| *c.borrow_mut() = Some(m));
}

/// Read the active MoveData (defaults to Tackle if unset — defensive).
fn active_move() -> MoveData {
    ACTIVE_MOVE.with(|c| c.borrow().unwrap_or(TACKLE_FALLBACK))
}

const TACKLE_FALLBACK: MoveData = MoveData {
    id: MoveId::Tackle,
    effect: MoveEffect::NoAdditionalEffect,
    power: 35,
    move_type: PokemonType::Normal,
    accuracy: 95,
    pp: 35,
};

thread_local! {
    /// The move each SIDE is executing this action. A real turn has TWO different
    /// moves, so the single [`ACTIVE_MOVE`] is insufficient in production — this is
    /// indexed by `BattlerRef.side`. The production routing sets both sides before
    /// `StackDriver::execute_turn`; the pipeline handlers read it by `source`,
    /// falling back to `ACTIVE_MOVE` (the same-move differential harness) when unset.
    static CURRENT_MOVES: RefCell<[Option<MoveData>; 2]> = const { RefCell::new([None, None]) };

    /// Per-side "raised a Substitute doll THIS turn" flag — so the game can narrate
    /// "put in a SUBSTITUTE!" even when the doll is broken again the SAME turn (the
    /// pre/post `HAS_SUBSTITUTE_UP` flags can't distinguish that). Set by
    /// `substitute_install`, cleared with [`clear_current_moves`] before each turn.
    static SUB_CREATED: RefCell<[bool; 2]> = const { RefCell::new([false, false]) };
}

/// Set the move the battler on `who`'s side is executing this action (production).
pub fn set_current_move(who: BattlerRef, m: MoveData) {
    CURRENT_MOVES.with(|c| c.borrow_mut()[who.side as usize] = Some(m));
}

/// Clear both sides' current moves + the per-turn Substitute-created flags (call
/// before a fresh turn).
pub fn clear_current_moves() {
    CURRENT_MOVES.with(|c| *c.borrow_mut() = [None, None]);
    SUB_CREATED.with(|c| *c.borrow_mut() = [false, false]);
}

/// Mark that `who` raised a Substitute doll this turn (called by `substitute_install`).
fn mark_sub_created(who: BattlerRef) {
    SUB_CREATED.with(|c| c.borrow_mut()[who.side as usize] = true);
}

/// Whether `who` raised a doll this turn (read game-side for the creation narration).
pub fn sub_created_this_turn(who: BattlerRef) -> bool {
    SUB_CREATED.with(|c| c.borrow()[who.side as usize])
}

/// The `MoveData` for the mover at `source`: the per-side [`CURRENT_MOVES`] if set
/// (production, different-move turns), else the single [`active_move`] (the same-move
/// differential harness).
fn current_move_for(source: BattlerRef) -> MoveData {
    CURRENT_MOVES
        .with(|c| c.borrow()[source.side as usize])
        .unwrap_or_else(active_move)
}

thread_local! {
    /// Each side's last move used AS OF THE TURN START — what a Disable this turn
    /// disables on its target. The engine `BattlerState` carries no last-move field,
    /// so the production loop primes this from `bs.{player,enemy}.last_move_used`
    /// before `execute_turn` (symmetric with [`CURRENT_MOVES`]); decoupled tests set
    /// it directly. Read by `disable_install`. NOTE: this is the PRE-turn value, so a
    /// Disable user that moves AFTER its target reads the target's PRIOR move, not the
    /// one it just used this turn. Two consequences vs the oracle (which reads the
    /// target's live `last_move_used` at effect time): (a) a slower Disable user whose
    /// target VARIED its move this turn disables the prior move; (b) on turn 1 a slower
    /// Disable is a no-op (the target's prior move is `None`) where the oracle would
    /// disable the just-used move. Both are narrow speed-order cases — Disable is
    /// normally used to lock a REPEATED move, where prior and just-used agree; a faster
    /// Disable user (the common intent) is always exact.
    static LAST_MOVE_LIVE: RefCell<[MoveId; 2]> = const { RefCell::new([MoveId::None, MoveId::None]) };
}

/// Prime the turn-start last-move-used for `who`'s side (production / tests).
pub fn set_last_move_live(who: BattlerRef, m: MoveId) {
    LAST_MOVE_LIVE.with(|c| c.borrow_mut()[who.side as usize] = m);
}

/// Clear both sides' turn-start last-move-used (call before a fresh turn / scenario).
pub fn clear_last_move_live() {
    LAST_MOVE_LIVE.with(|c| *c.borrow_mut() = [MoveId::None, MoveId::None]);
}

/// The turn-start last-move-used for `who`'s side (defaults to `None`).
fn last_move_live(who: BattlerRef) -> MoveId {
    LAST_MOVE_LIVE.with(|c| c.borrow()[who.side as usize])
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. The native pipeline handlers (the DRAW structure). Re-home pokered's own
//    crit / accuracy / damage formula — the damage authority — with a power-0
//    short-circuit so self-Boost / self-heal moves draw ONLY accuracy.
// ─────────────────────────────────────────────────────────────────────────────

/// `ModifyCritRatio` — draws the crit byte ONLY for a power>0 move (the legacy
/// power-0 branch draws no crit). Re-homes `crit_chance` + the Focus-Energy `/4`
/// bug (#1); base-speed from the species (Gen-1 ignores in-battle Speed, #3).
fn pokered_crit(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let pm = current_move_for(source);
    if pm.power == 0
        || is_special_damage_effect(pm.effect)
        || (is_charge_move(pm.effect) && charging_of(ctx, source).is_none())
    {
        // power-0 move OR a special/fixed/OHKO/SuperFang move OR a charge move's
        // gather turn (not yet charging): no crit draw. The
        // special-damage moves bypass the damage formula entirely (the SetDamage /
        // DamageCurrentHpFraction / SetHp op writes the number), and the legacy
        // standalone oracle (`apply_special_damage`/`apply_super_fang`/`apply_ohko`)
        // draws no crit byte — so neither does the stack (P3 parity).
        return HandlerResult::Unchanged;
    }
    let species = ctx.battler(source).species;
    let base_speed = get_base_stats(species).map_or(0, |s| s.speed);
    let is_high_crit = crate::battle::damage::is_high_crit_move(pm.id);
    let is_focus = ctx
        .effects
        .iter()
        .any(|e| e.host == source && matches!(e.kind, PokeVolatile::FocusEnergy));
    let threshold = crate::battle::damage::crit_chance(base_speed, is_high_crit, is_focus);
    let crit_roll = ctx.rng.next_u8();
    ctx.mv.is_critical = crit_roll < threshold;
    HandlerResult::Unchanged
}

/// `Accuracy` — the full Gen-1 hit check (re-homing `accuracy.rs`). SwiftEffect
/// never misses and draws NOTHING (parity). Otherwise draws the accuracy byte and
/// applies the accuracy/evasion stage ratios; byte 255 vs a 100% move is the
/// deliberate 1/256 miss (#2). Returns `Bool(false)` to STOP on a miss.
fn pokered_accuracy(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let pm = current_move_for(source);
    if pm.effect == MoveEffect::SwiftEffect || pm.id == MoveId::Counter {
        // Swift never misses; Counter is a fixed-damage reactive move that draws no
        // accuracy byte (it fails via `counter_handler`, not the accuracy roll).
        return HandlerResult::Unchanged;
    }
    // A charge move's GATHER turn draws no accuracy byte (it never "hits" turn 1).
    if is_charge_move(pm.effect) && charging_of(ctx, source).is_none() {
        return HandlerResult::Unchanged;
    }
    // Bide never rolls accuracy (it stores, then bide_residual unleashes fixed damage).
    if pm.effect == MoveEffect::BideEffect {
        return HandlerResult::Unchanged;
    }
    // Semi-invulnerability: a mid-charge Fly/Dig target can't be hit, PERIOD.
    // Gen 1 has NO Gust/Thunder/Earthquake/Fissure exceptions (those are Gen 2+);
    // only Swift bypasses, via the early `ret z` above (core.asm:5246-5248 —
    // the swiftCheck precedes .checkForDigOrFlyStatus). Missing here draws no
    // accuracy byte (the invulnerability check precedes the roll).
    if let Some((_charge_move, invulnerable)) = charging_of(ctx, target) {
        if invulnerable {
            return HandlerResult::Set(RelayVar::Bool(false));
        }
    }
    let acc_stage = ctx.battler(source).stat_stages.get(StatIndex::Accuracy).copied().unwrap_or(0);
    let eva_stage = ctx.battler(target).stat_stages.get(StatIndex::Evasion).copied().unwrap_or(0);
    let scaled = scaled_accuracy(pm.accuracy, acc_stage, eva_stage);
    let byte = ctx.rng.next_u8();
    if byte < scaled {
        HandlerResult::Unchanged
    } else {
        HandlerResult::Set(RelayVar::Bool(false)) // miss → STOP
    }
}

/// `ModifyDamage` — the damage authority. Power-0 moves draw NOTHING and leave
/// `ctx.mv.damage` at 0 (legacy power-0 branch). For a power>0 move it draws the
/// damage byte and writes the full Gen-1 `calculate_damage` number (STAB + type
/// chart + ×roll/255) into `ctx.mv.damage` — the single source of truth the
/// `DealMoveDamage` op marks and the driver applies. On a type-immunity "miss" it
/// returns `Bool(false)` to short-circuit (#4).
fn pokered_damage(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    use crate::battle::damage::{calculate_damage, is_physical, DamageParams};
    let pm = current_move_for(source);
    // Two-turn charge moves: the GATHER turn installs the Charging volatile and deals
    // no damage; the STRIKE turn (forced by forced_action) consumes it and lands the
    // hit through the normal formula below. Removing the volatile here also drops the
    // semi-invulnerability at the moment the mon commits to striking.
    if is_charge_move(pm.effect) {
        if let Some(idx) = ctx
            .effects
            .iter()
            .position(|e| e.host == source && matches!(e.kind, PokeVolatile::Charging { .. }))
        {
            ctx.effects.remove(idx); // STRIKE turn → fall through to the damage formula
        } else {
            ctx.effects.push(EffectState {
                id: EffectId(0x50_020 + if source.side == 0 { 0 } else { 1 }),
                host: source,
                effect_order: 960,
                kind: PokeVolatile::Charging {
                    move_: pm.id,
                    invulnerable: charge_is_invulnerable(pm.id),
                },
            });
            ctx.mv.damage = 0; // GATHER turn → no damage, no draw
            return HandlerResult::Unchanged;
        }
    }
    // Bide deals no damage while storing (the release is dealt by bide_residual as
    // 2× accumulated). First use installs the store for 2 or 3 turns at random
    // (effects.asm:782-786 — `(BattleRandom & 1) + 2`).
    if pm.effect == MoveEffect::BideEffect {
        if !ctx
            .effects
            .iter()
            .any(|e| e.host == source && matches!(e.kind, PokeVolatile::Bide { .. }))
        {
            let turns = (ctx.rng.next_u8() & 1) + 2;
            ctx.effects.push(EffectState {
                id: EffectId(0x50_038 + if source.side == 0 { 0 } else { 1 }),
                host: source,
                effect_order: 962,
                kind: PokeVolatile::Bide { turns_left: turns, accumulated: 0 },
            });
        }
        ctx.mv.damage = 0;
        return HandlerResult::Unchanged;
    }
    if pm.power == 0 || is_special_damage_effect(pm.effect) {
        // power-0 OR special/fixed/OHKO/SuperFang: the formula does NOT run here.
        // For a special-damage move the SetDamage / DamageCurrentHpFraction / SetHp
        // op (on ModifyDamage / DamagingHit) writes `ctx.mv.damage` directly,
        // bypassing the type chart — and draws no formula damage byte (P3 parity
        // vs the standalone oracle, which takes no rng).
        return HandlerResult::Unchanged;
    }
    // Gen-1 damage spread: reject bytes below 217 so the multiplier lands in
    // [217, 255] (~85%..100% of computed damage). The rejection is BOUNDED so a
    // degenerate or exhausted rng (e.g. a scripted all-zero test stream) can
    // never spin forever — a real rng terminates in ~6 tries, far below the cap,
    // so the production draw count/stream is unchanged.
    let damage_roll = {
        let mut b = ctx.rng.next_u8();
        let mut tries = 0;
        while b < 217 && tries < 64 {
            b = ctx.rng.next_u8();
            tries += 1;
        }
        b.max(217)
    };
    let is_crit = ctx.mv.is_critical;
    let physical = is_physical(pm.move_type);

    let a = ctx.battler(source);
    let d = ctx.battler(target);
    let (atk, def) = if physical {
        (a.stats.get(StatIndex::Attack).copied().unwrap_or(0),
         d.stats.get(StatIndex::Defense).copied().unwrap_or(1))
    } else {
        (a.stats.get(StatIndex::Special).copied().unwrap_or(0),
         d.stats.get(StatIndex::Special).copied().unwrap_or(1))
    };
    let (atk_stage, def_stage) = if physical {
        (a.stat_stages.get(StatIndex::Attack).copied().unwrap_or(0),
         d.stat_stages.get(StatIndex::Defense).copied().unwrap_or(0))
    } else {
        (a.stat_stages.get(StatIndex::Special).copied().unwrap_or(0),
         d.stat_stages.get(StatIndex::Special).copied().unwrap_or(0))
    };
    let level = level_of(a);
    // STAB (attacker) + type effectiveness/immunity (defender) honour Conversion via
    // the arena TypeOverride (effective_types), falling back to the species types.
    let (atype1, atype2) = effective_types(ctx, source);
    let (dtype1, dtype2) = effective_types(ctx, target);
    // Reflect halves physical damage, Light Screen halves special — modelled as
    // doubling the relevant defence when the DEFENDER holds the matching screen
    // volatile (the setter is InflictVolatile on the screen records).
    let has_screen = ctx.effects.iter().any(|e| {
        e.host == target
            && match e.kind {
                PokeVolatile::Reflect => physical,
                PokeVolatile::LightScreen => !physical,
                _ => false,
            }
    });
    let params = DamageParams {
        attacker_level: level,
        move_power: pm.power,
        move_type: pm.move_type,
        move_id: pm.id,
        attack_stat: atk,
        defense_stat: def,
        attack_stage: atk_stage,
        defense_stage: def_stage,
        attacker_type1: atype1,
        attacker_type2: atype2,
        defender_type1: dtype1,
        defender_type2: dtype2,
        is_critical: is_crit,
        random_value: damage_roll,
        has_reflect_or_light_screen: has_screen,
        // Explosion / Self-Destruct halve the target's Defense (the self-KO is a
        // data op on the record).
        is_explode_effect: pm.effect == MoveEffect::ExplodeEffect,
        attacker_burned: a.status == Some(LegacyStatus::Burn),
    };
    let result = calculate_damage(&params);
    ctx.mv.damage = if result.is_miss { 0 } else { result.damage };
    if result.is_miss {
        ctx.mv.move_missed = true;
        return HandlerResult::Set(RelayVar::Bool(false)); // type-immunity → "miss"
    }
    HandlerResult::Unchanged
}

// ─────────────────────────────────────────────────────────────────────────────
// 7a′. COUNTER (Gen-1 bug #20) — the cross-action reactive read (design §9), now on
//      the LIVE path. `ctx.mv.last_damage` is reset PER MOVER, so Counter (−1
//      priority, always last) cannot read it across the opponent's move; the damage
//      must live PER-BATTLER in the arena. Two native handlers realize this, exactly
//      re-homing the `stack_parity` POC (`record_damage_taken` / `counter_handler`,
//      slice 6): the recorder stamps every defender's per-turn `DamageTaken`; Counter
//      reads its OWN scratch and reflects 2× the NORMAL/FIGHTING damage taken. Because the
//      arena is rebuilt fresh each turn from the legacy state (`engine_state_from_legacy`)
//      and `DamageTaken` has no legacy backing, the scratch is inherently per-turn —
//      no reset plumbing. The engine stays Pokémon-unaware (it only sees an opaque
//      `EffectStateKind` and fires the same events).
// ─────────────────────────────────────────────────────────────────────────────

/// `DamagingHit` recorder (every move record). After the driver applied a move's
/// damage, stamp the DEFENDER's per-turn `DamageTaken` scratch with the amount +
/// whether the dealing move is Counter-eligible (NORMAL / FIGHTING type — bug #20).
/// Replaces any prior entry for the defender this turn (Gen-1 Counter reads the LAST
/// damage taken). Draws NO rng; inert for every move that isn't followed by a Counter
/// (the entry is simply dropped at write-back).
fn record_damage_taken(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let amount = match relay {
        RelayVar::Damage(d) => d,
        _ => ctx.mv.last_damage,
    };
    if amount == 0 {
        return HandlerResult::Unchanged; // no damage → nothing to record
    }
    // Gen-1 Counter reflects only NORMAL / FIGHTING damage — not every physical-type
    // move (Earthquake, Rock Slide, … are physical but NOT counterable).
    let counterable = matches!(
        current_move_for(source).move_type,
        PokemonType::Normal | PokemonType::Fighting
    );
    if let Some(idx) = ctx
        .effects
        .iter()
        .position(|e| e.host == target && matches!(e.kind, PokeVolatile::DamageTaken { .. }))
    {
        ctx.effects[idx].kind = PokeVolatile::DamageTaken { amount, counterable };
    } else {
        // A high, per-side id keeps it distinct from the data / move-effect id
        // spaces; the arena is sorted by id so this lands after the volatiles.
        ctx.effects.push(EffectState {
            id: EffectId(0x50_000 + if target.side == 0 { 0 } else { 1 }),
            host: target,
            effect_order: 900,
            kind: PokeVolatile::DamageTaken { amount, counterable },
        });
    }
    HandlerResult::Unchanged
}

/// `ModifyDamage` handler for the Counter record (bug #20). Reads the Counter
/// user's (`source`) own per-turn `DamageTaken` scratch and reflects `amount * 2`
/// onto the opponent (`target`) via the load-bearing `pair_mut` — "mutate target
/// while reading source" (design §3.2) — then zeroes `ctx.mv.damage` so the driver's
/// own `take_damage(target, mv.damage)` does not double-apply, and returns
/// `Set(Bool(false))` to stop the ModifyDamage chain. Counter FAILS (deals 0) when the
/// user took no NORMAL/FIGHTING damage this turn (special / non-N/F / status / none),
/// and a fainted Counter user reflects nothing. Draws NO rng.
fn counter_handler(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let (amount, counterable) = match ctx
        .effects
        .iter()
        .find(|e| e.host == source && matches!(e.kind, PokeVolatile::DamageTaken { .. }))
        .map(|e| &e.kind)
    {
        Some(PokeVolatile::DamageTaken { amount, counterable }) => (*amount, *counterable),
        _ => (0, false),
    };
    if amount == 0 || !counterable {
        ctx.mv.damage = 0;
        return HandlerResult::Set(RelayVar::Bool(false)); // Counter fails
    }
    let reflected = amount.saturating_mul(2);
    let (counter_user, opponent) = ctx.pair_mut(source, target);
    if counter_user.hp == 0 {
        ctx.mv.damage = 0;
        return HandlerResult::Set(RelayVar::Bool(false)); // dead user reflects nothing
    }
    opponent.take_damage(reflected);
    ctx.mv.damage = 0; // applied via pair_mut → don't let the driver re-apply
    HandlerResult::Set(RelayVar::Bool(false)) // STOP the ModifyDamage chain
}

/// `DamagingHit` hook (every record): after a Hyper Beam connects, install the
/// `Recharge` volatile on the user so it is forced to skip next turn — UNLESS the
/// target fainted (Gen-1: a KO skips the recharge, the well-known quirk). Keyed on
/// the active move's `HyperBeamEffect`, so it is inert for every other move. Draws
/// no rng; the volatile round-trips through `status2::NEEDS_TO_RECHARGE`.
fn hyperbeam_recharge_install(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if current_move_for(source).effect != MoveEffect::HyperBeamEffect {
        return HandlerResult::Unchanged;
    }
    // Gen-1: a Hyper Beam that KO'd the target needs no recharge.
    if ctx.battler(target).hp == 0 {
        return HandlerResult::Unchanged;
    }
    if ctx
        .effects
        .iter()
        .any(|e| e.host == source && matches!(e.kind, PokeVolatile::Recharge))
    {
        return HandlerResult::Unchanged; // idempotent
    }
    ctx.effects.push(EffectState {
        id: EffectId(0x50_010 + if source.side == 0 { 0 } else { 1 }),
        host: source,
        effect_order: 950,
        kind: PokeVolatile::Recharge,
    });
    HandlerResult::Unchanged
}

/// `DamagingHit` hook (every record): Thrash / Petal Dance lock-in. Re-homes
/// `apply_thrash` (`multi_turn_effects.rs`): first use rolls a 2–3 counter and
/// installs `LockedMove`; each forced re-use decrements it and, on exhaustion, the
/// user self-confuses ((rng & 7).max(1) turns — the Gen-1 fatigue). Keyed on
/// ThrashPetalDanceEffect → inert for every other move (draws no rng then).
fn thrash_lockin(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if current_move_for(source).effect != MoveEffect::ThrashPetalDanceEffect {
        return HandlerResult::Unchanged;
    }
    let existing = ctx
        .effects
        .iter()
        .position(|e| e.host == source && matches!(e.kind, PokeVolatile::LockedMove { .. }));
    if let Some(idx) = existing {
        let ended = {
            let PokeVolatile::LockedMove { turns_left, .. } = &mut ctx.effects[idx].kind else {
                return HandlerResult::Unchanged;
            };
            if *turns_left > 0 {
                *turns_left -= 1;
            }
            *turns_left == 0
        };
        if ended {
            ctx.effects.remove(idx);
            let turns = (ctx.rng.next_u8() & 0x07).max(1);
            if !ctx
                .effects
                .iter()
                .any(|e| e.host == source && matches!(e.kind, PokeVolatile::Confused { .. }))
            {
                ctx.effects.push(EffectState {
                    id: EffectId(0x50_030 + if source.side == 0 { 0 } else { 1 }),
                    host: source,
                    effect_order: 970,
                    kind: PokeVolatile::Confused { turns },
                });
            }
        }
    } else {
        let counter = (ctx.rng.next_u8() & 0x01) + 2;
        ctx.effects.push(EffectState {
            id: EffectId(0x50_032 + if source.side == 0 { 0 } else { 1 }),
            host: source,
            effect_order: 965,
            kind: PokeVolatile::LockedMove {
                move_: current_move_for(source).id,
                turns_left: counter,
                confuse_on_end: true,
            },
        });
    }
    HandlerResult::Unchanged
}

/// `DamagingHit` hook (every record): Rage. Two independent effects, cleanly split
/// by role — the MOVER using Rage locks into it (install on `source`); a RAGING mon
/// that just took a damaging hit gains +1 Attack (boost `target`). Re-homes
/// `apply_rage` + the Gen-1 "rage is building". Draws no rng; inert otherwise.
fn rage_manage(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    // The mover used Rage → lock in (idempotent).
    if current_move_for(source).effect == MoveEffect::RageEffect
        && !ctx.effects.iter().any(|e| e.host == source && matches!(e.kind, PokeVolatile::Rage))
    {
        ctx.effects.push(EffectState {
            id: EffectId(0x50_034 + if source.side == 0 { 0 } else { 1 }),
            host: source,
            effect_order: 968,
            kind: PokeVolatile::Rage,
        });
    }
    // The DEFENDER is raging and just got hit → +1 Attack stage (clamped to +6).
    if ctx.effects.iter().any(|e| e.host == target && matches!(e.kind, PokeVolatile::Rage)) {
        let b = ctx.battler_mut(target);
        let cur = b.stat_stages.get(StatIndex::Attack).copied().unwrap_or(0);
        if cur < 6 {
            b.stat_stages.set(StatIndex::Attack, cur + 1);
        }
    }
    HandlerResult::Unchanged
}

/// `DamagingHit` hook (every record): trapping moves (Wrap/Bind/Fire Spin/Clamp).
/// Re-homes `apply_trapping`: first HIT rolls 2–5 turns and installs `Trapping`
/// (`turns_left = turns − 1`, the legacy `num_attacks_left`); each forced re-use
/// decrements it and clears at 0. The forced re-hit deals the per-turn damage; the
/// foe is bound via `forced_action`. Keyed on TrappingEffect → inert otherwise.
fn trapping_lockin(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if current_move_for(source).effect != MoveEffect::TrappingEffect {
        return HandlerResult::Unchanged;
    }
    let existing = ctx
        .effects
        .iter()
        .position(|e| e.host == source && matches!(e.kind, PokeVolatile::Trapping { .. }));
    if let Some(idx) = existing {
        let ended = {
            let PokeVolatile::Trapping { turns_left, .. } = &mut ctx.effects[idx].kind else {
                return HandlerResult::Unchanged;
            };
            if *turns_left > 0 {
                *turns_left -= 1;
            }
            *turns_left == 0
        };
        if ended {
            ctx.effects.remove(idx);
        }
    } else {
        // Gen-1: 2–5 turns with the multi-hit WEIGHTS (3/8 each for 2/3, 1/8
        // each for 4/5 — effects.asm TrappingEffect re-rolls `& 3` when the
        // first draw ≥ 2; `determine_hit_count` is the single-byte equivalent);
        // store turns − 1.
        let turns = crate::battle::effects::multi_hit_effects::determine_hit_count(ctx.rng.next_u8());
        ctx.effects.push(EffectState {
            id: EffectId(0x50_036 + if source.side == 0 { 0 } else { 1 }),
            host: source,
            effect_order: 966,
            kind: PokeVolatile::Trapping {
                move_: current_move_for(source).id,
                turns_left: turns - 1,
            },
        });
    }
    HandlerResult::Unchanged
}

/// `DamagingHit` hook (every record): CheckDefrost (effects.asm:312-330). Any
/// Fire-type move in the FreezeBurnParalyze family — in Gen 1 the burn
/// side-effect moves (Ember / Fire Punch / Flamethrower / Fire Blast) — DEFROSTS
/// a frozen target it hits ("Fire defrosted <TARGET>!"). The original reaches
/// CheckDefrost when the side-effect handler finds the target already statused,
/// BEFORE the probability roll, so the defrost is unconditional on a hit. This
/// hook is ordered after the RON side-status hooks, which cannot inflict on an
/// already-statused target (the binding guard) — so the net outcome matches the
/// original exactly: defrosted, never burned. Keyed on move type+effect → inert
/// for every other move; draws no rng.
fn pokered_defrost(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let pm = current_move_for(source);
    if pm.move_type != PokemonType::Fire
        || !matches!(
            pm.effect,
            MoveEffect::BurnSideEffect1
                | MoveEffect::BurnSideEffect2
                | MoveEffect::FreezeSideEffect1
                | MoveEffect::FreezeSideEffect2
                | MoveEffect::ParalyzeSideEffect1
                | MoveEffect::ParalyzeSideEffect2
        )
    {
        return HandlerResult::Unchanged;
    }
    if ctx.battler(target).status == Some(LegacyStatus::Freeze) {
        ctx.battler_mut(target).status = None;
    }
    HandlerResult::Unchanged
}

/// `DamagingHit` hook (every record): Transform. Copies the target's identity
/// (species / stats / stat-stages / moves) onto the user's ENGINE battler and pushes
/// the one-shot `Transformed` marker so `write_party` persists it to the legacy
/// Pokémon. Types follow automatically from the copied species. Keyed on
/// TransformEffect → inert for every other move; draws no rng.
fn transform_install(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if current_move_for(source).effect != MoveEffect::TransformEffect {
        return HandlerResult::Unchanged;
    }
    let (species, stats, stages, moves) = {
        let d = ctx.battler(target);
        (d.species, d.stats.clone(), d.stat_stages.clone(), d.moves.clone())
    };
    {
        let a = ctx.battler_mut(source);
        a.species = species;
        a.stats = stats;
        a.stat_stages = stages;
        a.moves = moves;
    }
    if !ctx
        .effects
        .iter()
        .any(|e| e.host == source && matches!(e.kind, PokeVolatile::Transformed))
    {
        ctx.effects.push(EffectState {
            id: EffectId(0x50_040 + if source.side == 0 { 0 } else { 1 }),
            host: source,
            effect_order: 972,
            kind: PokeVolatile::Transformed,
        });
    }
    HandlerResult::Unchanged
}

/// `DamagingHit` hook (every record): Conversion. Copies the TARGET's effective types
/// onto the user via a [`PokeVolatile::TypeOverride`] (the engine derives types from
/// species, so the change lives in the arena, round-tripped through the battle-only
/// `conversion_type1/2` fields). Mirrors legacy `apply_conversion` (which copies the
/// defender's `type1`/`type2` onto the attacker). Keyed on ConversionEffect → inert
/// for every other move. Fires only on a connecting Conversion (a missed status move
/// returns before DamagingHit), so a missed Conversion does nothing — faithful.
fn conversion_install(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if current_move_for(source).effect != MoveEffect::ConversionEffect {
        return HandlerResult::Unchanged;
    }
    let (type1, type2) = effective_types(ctx, target);
    // Replace any prior override on the user (a re-Conversion re-copies).
    ctx.effects
        .retain(|e| !(e.host == source && matches!(e.kind, PokeVolatile::TypeOverride { .. })));
    ctx.effects.push(EffectState {
        id: EffectId(0x50_050 + if source.side == 0 { 0 } else { 1 }),
        host: source,
        effect_order: 973,
        kind: PokeVolatile::TypeOverride { type1, type2 },
    });
    HandlerResult::Unchanged
}

/// `DamagingHit` hook (every record): Disable. Disables the TARGET's last-used move
/// for `(rng & 7) + 1` turns (legacy `apply_disable`). The target's prior move rides
/// [`last_move_live`] (the engine battler carries no last-move field); the slot is its
/// position in the compacted engine `moves` (1-based, == the legacy `disabled_move`
/// full-array slot for a gapless moveset — the Gen-1 norm). Fails (no-op) if the target
/// is already disabled or has no last move in a slot — matching the oracle's
/// `StatusFailed`. Keyed on DisableEffect; fires only on a connecting Disable.
///
/// The oracle's `pp[i] > 0` guard is enforced UPSTREAM: the production loop primes
/// [`last_move_live`] via `disable_target_last_move`, which yields `None` for an
/// out-of-PP last move — so this handler naturally no-ops on it, exactly like
/// `apply_disable`. (The decoupled harness primes `last_move_live` directly.)
fn disable_install(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if current_move_for(source).effect != MoveEffect::DisableEffect {
        return HandlerResult::Unchanged;
    }
    // Already disabled → the legacy StatusFailed no-op.
    if ctx
        .effects
        .iter()
        .any(|e| e.host == target && matches!(e.kind, PokeVolatile::Disable { .. }))
    {
        return HandlerResult::Unchanged;
    }
    let last = last_move_live(target);
    if last == MoveId::None {
        return HandlerResult::Unchanged;
    }
    let Some(slot) = ctx.battler(target).moves.iter().position(|m| *m == last) else {
        return HandlerResult::Unchanged;
    };
    let turns = (ctx.rng.next_u8() & 0x07) + 1; // (roll & 7) + 1, min 1 — matches apply_disable
    ctx.effects.push(EffectState {
        id: EffectId(0x50_060 + if target.side == 0 { 0 } else { 1 }),
        host: target,
        effect_order: 974,
        kind: PokeVolatile::Disable {
            slot: (slot + 1) as u8, // 1-based
            turns,
        },
    });
    HandlerResult::Unchanged
}

/// `BeforeMove` gate (order 50, ASM step 6): decrement the MOVER's Disable countdown
/// each turn it acts; at 0 the disable ends (the volatile is removed). Never blocks —
/// the block is the separate `disable_veto_gate` (order 80). run_event's short-circuit
/// means a sleeping/frozen/flinched mover (gates 10/20/30) never reaches here, so the
/// counter does not tick on a turn the mon couldn't act — matching the ASM early-return.
fn disable_decrement_gate(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if let Some(idx) = ctx
        .effects
        .iter()
        .position(|e| e.host == source && matches!(e.kind, PokeVolatile::Disable { .. }))
    {
        let expired = if let PokeVolatile::Disable { turns, .. } = &mut ctx.effects[idx].kind {
            *turns = turns.saturating_sub(1);
            *turns == 0
        } else {
            false
        };
        if expired {
            ctx.effects.remove(idx);
        }
    }
    HandlerResult::Unchanged
}

/// `BeforeMove` gate (order 80, ASM step 8): block the move if the MOVER chose its
/// DISABLED move — the mon loses the turn ("can't move!"). Runs AFTER the decrement
/// (50) and confusion (70), so a disable that expired THIS turn (removed at 50) no
/// longer blocks. In normal play the disabled move is unselectable (the player menu +
/// smart trainer AI exclude it), so this fires only when a disabled move is chosen
/// anyway (a wild / no-AI-layer mon's random pick, or a forced move) — the faithful
/// Gen-1 wasted turn.
fn disable_veto_gate(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let disabled_slot = ctx.effects.iter().find_map(|e| match &e.kind {
        PokeVolatile::Disable { slot, .. } if e.host == source => Some(*slot),
        _ => None,
    });
    if let Some(slot) = disabled_slot {
        if let Some(disabled_move) = ctx.battler(source).moves.get(slot.saturating_sub(1) as usize) {
            if *disabled_move == current_move_for(source).id {
                return HandlerResult::Fail; // move is disabled → wasted turn
            }
        }
    }
    HandlerResult::Unchanged
}

/// `OnMiss` hook (every record): a charge move that MISSES on its STRIKE turn still
/// ends the charge — the mon comes down / surfaces, so it must not stay invulnerable.
/// On a HIT this removal is done in `pokered_damage`; the accuracy-miss branch returns
/// before ModifyDamage, so it is cleared here. Inert for non-charge moves.
fn charge_miss_land(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if is_charge_move(current_move_for(source).effect) {
        if let Some(idx) = ctx
            .effects
            .iter()
            .position(|e| e.host == source && matches!(e.kind, PokeVolatile::Charging { .. }))
        {
            ctx.effects.remove(idx);
        }
    }
    HandlerResult::Unchanged
}

/// `OnMiss` hook (every record): Jump Kick / Hi Jump Kick crash. A missed (Hi) Jump
/// Kick hurts the user for 1 HP (Gen-1's famous 1-HP crash). Gated on JumpKickEffect
/// → inert for every other missed move. Fires only on the accuracy-miss branch (not
/// type-immunity), matching the OnMiss seam.
fn jump_kick_crash(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if current_move_for(source).effect == MoveEffect::JumpKickEffect {
        ctx.battler_mut(source).take_damage(1);
    }
    HandlerResult::Unchanged
}

/// `DamagingHit` hook (every record): Haze. Resets BOTH sides' stat stages to 0 and
/// cures their status, and clears the volatiles Gen-1 Haze wipes (confusion, Leech
/// Seed, Toxic, Focus Energy) — SELECTIVELY: Light Screen / Reflect / Mist /
/// Substitute / lock-ins are PRESERVED (apply_haze keeps them). Keyed on HazeEffect.
fn haze_reset(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if current_move_for(source).effect != MoveEffect::HazeEffect {
        return HandlerResult::Unchanged;
    }
    for who in [BattlerRef::PLAYER, BattlerRef::OPPONENT] {
        let b = ctx.battler_mut(who);
        b.stat_stages = EnumMap::new();
        b.status = None;
    }
    ctx.effects.retain(|e| {
        !matches!(
            e.kind,
            PokeVolatile::Confused { .. }
                | PokeVolatile::LeechSeed
                | PokeVolatile::Toxic { .. }
                | PokeVolatile::FocusEnergy
        )
    });
    HandlerResult::Unchanged
}

/// `DamagingHit` hook (every record): Substitute creation. Re-homes legacy
/// `apply_substitute` — the user spends `max_hp/4` HP to raise a doll with that many
/// HP (`SubstituteHp`). Fails (no-op) if a Substitute is already up, the cost is 0, or
/// the user has less than the cost — EXCEPT the preserved Gen-1 bug #28: `hp == cost`
/// SUCCEEDS, leaving the user at 0 HP. Keyed on SubstituteEffect (self-effect; installs
/// on `source`, ignoring the nominal target).
fn substitute_install(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if current_move_for(source).effect != MoveEffect::SubstituteEffect {
        return HandlerResult::Unchanged;
    }
    if ctx.effects.iter().any(|e| {
        e.host == source
            && matches!(e.kind, PokeVolatile::Substitute | PokeVolatile::SubstituteHp { .. })
    }) {
        return HandlerResult::Unchanged; // already up (legacy SubstituteFailed)
    }
    let (max_hp, hp) = {
        let b = ctx.battler(source);
        (b.max_hp, b.hp)
    };
    let cost = max_hp / 4;
    if cost == 0 || hp < cost {
        return HandlerResult::Unchanged; // legacy SubstituteFailed
    }
    ctx.battler_mut(source).hp -= cost; // BUG #28: hp == cost leaves the user at 0
    ctx.effects.push(EffectState {
        id: EffectId(0x50_070 + if source.side == 0 { 0 } else { 1 }),
        host: source,
        effect_order: 975,
        kind: PokeVolatile::SubstituteHp { hp: cost },
    });
    mark_sub_created(source); // for the "put in a SUBSTITUTE!" narration
    HandlerResult::Unchanged
}

/// `Event::Damage` hook (every record): Substitute absorb — the reserved damage-
/// application seam (driver.rs fires it AFTER the effectiveness fold, BEFORE the hp
/// write). Re-homes legacy `move_execution.rs` absorb: if the DEFENDER holds a
/// Substitute, the incoming damage hits the doll instead of the mon — the doll BREAKS
/// (volatile removed) when the hit is >= its HP (Gen-1 does NOT spill the overkill
/// into the mon), else its HP drops by the damage. Returns `Set(Damage(0))` so the mon
/// takes nothing. Inert (Unchanged) when the target has no Substitute or the hit is 0.
fn substitute_absorb(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let dmg = relay.as_damage();
    if dmg == 0 {
        return HandlerResult::Unchanged;
    }
    if absorb_into_substitute(ctx, target, dmg) {
        HandlerResult::Set(RelayVar::Damage(0)) // the mon takes nothing
    } else {
        HandlerResult::Unchanged
    }
}

/// Route `dmg` into `who`'s Substitute doll if it has one: the doll BREAKS (volatile
/// removed) when `dmg >= its HP` (Gen-1 does NOT spill the overkill into the mon), else
/// its HP drops by `dmg`. Returns `true` iff a doll absorbed the hit — the shared core
/// of both the `Event::Damage` absorb (formula damage) and the `redirect_hp_loss`
/// binding (the direct-mutate ops: Super Fang / OHKO / multi-hit).
fn absorb_into_substitute(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    who: BattlerRef,
    dmg: u16,
) -> bool {
    let Some(idx) = ctx
        .effects
        .iter()
        .position(|e| e.host == who && matches!(e.kind, PokeVolatile::SubstituteHp { .. }))
    else {
        return false;
    };
    let PokeVolatile::SubstituteHp { hp } = ctx.effects[idx].kind else {
        return false;
    };
    if dmg >= hp {
        ctx.effects.remove(idx); // the doll broke (no overflow to the mon)
    } else {
        ctx.effects[idx].kind = PokeVolatile::SubstituteHp { hp: hp - dmg };
    }
    true
}

/// The damage the host took this turn (its per-turn `DamageTaken` scratch, 0 if
/// none). Shared by Bide's accumulator.
fn damage_taken_this_turn(ctx: &BattleCtx<'_, PokeredRules>, who: BattlerRef) -> u16 {
    ctx.effects
        .iter()
        .find_map(|e| match &e.kind {
            PokeVolatile::DamageTaken { amount, .. } if e.host == who => Some(*amount),
            _ => None,
        })
        .unwrap_or(0)
}

/// `Residual` handler for the Bide volatile: fold the damage taken this turn into the
/// accumulator, decrement the store counter, and on exhaustion unleash `accumulated
/// × 2` (Gen-1 bug #18: ×2, not ×3) onto the opponent via the load-bearing `pair_mut`.
/// Draws no rng; self-guards on a fainted host.
fn bide_residual(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let host = target;
    let Some(idx) = ctx
        .effects
        .iter()
        .position(|e| e.host == host && matches!(e.kind, PokeVolatile::Bide { .. }))
    else {
        return HandlerResult::Unchanged;
    };
    let taken = damage_taken_this_turn(ctx, host);
    let (unleash, total) = {
        let PokeVolatile::Bide { accumulated, turns_left } = &mut ctx.effects[idx].kind else {
            return HandlerResult::Unchanged;
        };
        *accumulated = accumulated.saturating_add(taken);
        if *turns_left > 0 {
            *turns_left -= 1;
        }
        (*turns_left == 0, *accumulated)
    };
    if unleash {
        ctx.effects.remove(idx);
        let release = total.saturating_mul(2);
        let opponent = BattlerRef::new(if host.side == 0 { 1 } else { 0 }, host.slot);
        let (host_mon, opp_mon) = ctx.pair_mut(host, opponent);
        if host_mon.hp > 0 {
            opp_mon.take_damage(release);
        }
    }
    HandlerResult::Unchanged
}

/// The leaked `&'static` Bide residual effect (one `Residual` hook). Returned by
/// `effect_for_volatile` for a `Bide` volatile so the driver ticks it each turn.
fn bide_residual_effect() -> &'static Effect<PokeredRules> {
    use std::sync::OnceLock;
    static EFF: OnceLock<&'static Effect<PokeredRules>> = OnceLock::new();
    EFF.get_or_init(|| {
        let hooks: &'static [EventHook<PokeredRules>] = Box::leak(
            vec![EventHook {
                event: Event::Residual,
                call: bide_residual,
                order: 100,
                priority: 0,
                sub_order: None,
            }]
            .into_boxed_slice(),
        );
        Box::leak(Box::new(Effect {
            id: EffectId(0x40_010),
            kind: EffectType::Move,
            hooks,
        }))
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 7b. P3 — the nested-veto cascade (the ONE shared native orchestration; blueprint
//     15 §3 "Shared native orchestration"). A foe stat-down move CANNOT be pure
//     data: the StackDriver never fires `Event::TryBoost`. So the GAME side fires
//     it. `bridge_foe_stat_down` rides `DamagingHit`, reads the authored
//     `Boost(stat, -N, Foe)` op-list, fires `TryBoost` via the engine's exposed
//     `collect_handlers` + `run_event_checked` seams (the Intimidate→Clear-Body
//     shape), and applies the drop ONLY if no handler vetoed. Mist + Substitute
//     register `TryBoost` handlers (via `effect_for_volatile`) that return `Fail`,
//     so they absorb the drop — mirroring the legacy `apply_stat_down` guards.
//     This unblocks all 13 foe-down arms + Mist + Substitute stat absorption at
//     once. NO ENGINE CHANGE: `collect_handlers`/`run_event_checked`/`Event::TryBoost`
//     are already public engine seams the StackDriver simply does not fire itself.
// ─────────────────────────────────────────────────────────────────────────────

/// A `TryBoost` veto handler: returns `Fail`, vetoing ANY stat-stage change on
/// its host. Registered by the `Mist` and `Substitute` volatiles. When the
/// pokered-side driver fires `Event::TryBoost` on a foe protected by Mist/Sub,
/// `run_event_checked` collects this and returns `Bool(false)` (the `Fail` fold
/// result), so the driver skips the drop. Pure; draws no rng.
fn try_boost_veto(
    _ctx: &mut BattleCtx<'_, PokeredRules>,
    _relay: RelayVar,
    _target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    HandlerResult::Fail
}

/// The leaked `&'static` Mist veto effect (one `TryBoost` hook ⇒ `Fail`). Built
/// once per process. `EffectId`s are well clear of the data / move-effect spaces.
fn mist_try_boost_effect() -> &'static Effect<PokeredRules> {
    use std::sync::OnceLock;
    static EFF: OnceLock<&'static Effect<PokeredRules>> = OnceLock::new();
    EFF.get_or_init(|| build_try_boost_veto_effect(EffectId(0x40_001)))
}

/// The leaked `&'static` Substitute veto effect (one `TryBoost` hook ⇒ `Fail`).
fn substitute_try_boost_effect() -> &'static Effect<PokeredRules> {
    use std::sync::OnceLock;
    static EFF: OnceLock<&'static Effect<PokeredRules>> = OnceLock::new();
    EFF.get_or_init(|| build_try_boost_veto_effect(EffectId(0x40_002)))
}

/// Build a one-hook `TryBoost`→`Fail` effect with the given id (leaked `&'static`).
fn build_try_boost_veto_effect(id: EffectId) -> &'static Effect<PokeredRules> {
    let hooks: &'static [EventHook<PokeredRules>] = Box::leak(
        vec![EventHook {
            event: Event::TryBoost,
            call: try_boost_veto,
            order: 0,
            priority: 0,
            sub_order: None,
        }]
        .into_boxed_slice(),
    );
    Box::leak(Box::new(Effect { id, kind: EffectType::Condition, hooks }))
}

/// **The pokered-side nested-veto driver** (blueprint 15 §3). For a foe stat-down
/// move, this rides `DamagingHit`, applies the authored chance gate (side-variant)
/// UNCONDITIONALLY (so the byte is drawn at the legacy ordinal even when blocked —
/// the `consumed()` invariant), reads the record's `Boost(stat, stages, Foe)` op,
/// FIRES `Event::TryBoost` on the foe, and applies the drop only if no Mist /
/// Substitute handler vetoed.
///
/// Firing `TryBoost` = the same shape minimon's Intimidate→Clear-Body cascade
/// uses: build the handler set via the engine's [`collect_handlers`] (which scans
/// the foe's live volatiles → `effect_for_volatile`), then re-enter
/// [`run_event_checked`]. A `Bool(false)` result (a handler returned `Fail`) is
/// the veto. The StackDriver itself never fires `TryBoost`; the game does — using
/// engine seams that are already public, so NO engine change is needed.
fn bridge_foe_stat_down(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    source_effect: EffectId,
) -> HandlerResult {
    use jrpg_engine::battle::stack::{collect_handlers, run_event_checked};
    use jrpg_rules::Op;

    let Some(hook) = hook_for(source_effect, Event::DamagingHit) else {
        return HandlerResult::Unchanged;
    };
    // The chance gate (side-variant 85/256). Drawn UNCONDITIONALLY so the
    // side_effect byte is consumed at the legacy ordinal even when the drop is
    // blocked by Mist/Substitute — the consumed() invariant. A primary foe-down
    // (no chance) skips this with no byte.
    if let Some((num, den)) = hook.chance {
        if !ctx.rng.chance(num, den) {
            return HandlerResult::Unchanged; // secondary did not roll ⇒ no drop
        }
    }

    // Recover the authored Boost op (stat index + stages). The op targets `Foe`;
    // the foe of the DamagingHit `target` is the move's actual defender — but on
    // DamagingHit `target` IS the defender, so the foe-of-target is the attacker.
    // We resolve the boost selector exactly like the interpreter would.
    let Some((stat_name, stages, sel)) = hook.ops.iter().find_map(|op| match op {
        Op::Boost { stat, stages, target } => Some((stat.clone(), *stages, *target)),
        _ => None,
    }) else {
        return HandlerResult::Unchanged;
    };
    // `Foe` selector ⇒ foe-of-target. For a foe stat-down authored as `Foe`, this
    // resolves to the defender being lowered: with DamagingHit target=defender and
    // a `Target` selector pointing at the defender, but the legacy "down on the
    // OPPONENT of the mover" = the defender = `target`. We author the down as
    // `target: Target` (the defender) so the resolved battler is the defender.
    let lowered = match sel {
        jrpg_rules::Selector::Target | jrpg_rules::Selector::Host => target,
        jrpg_rules::Selector::Source => source,
        jrpg_rules::Selector::Foe => {
            BattlerRef::new(if target.side == 0 { 1 } else { 0 }, target.slot)
        }
    };

    // FIRE Event::TryBoost on the lowered battler. The relay carries the (signed)
    // stage delta so a future modify-style handler could fold it; Mist/Substitute
    // only need to `Fail`. `collect_handlers` gathers the lowered battler's live
    // volatiles' TryBoost hooks (Mist/Substitute → veto); re-enter the checked
    // fold. A `Bool(false)` is the veto.
    //
    // BOTH target AND source are `lowered` here: a stat drop's veto is hosted on
    // the battler BEING lowered (legacy `apply_stat_down` reads only the defender's
    // Mist/Substitute flags). Passing `source = lowered` confines the volatile scan
    // to that one battler, so the OTHER side's Mist/Substitute is NOT collected —
    // which is correct (their Mist protects them, not the foe) AND keeps the draw
    // count exact: collecting two tied TryBoost handlers would make the engine's
    // speed-tie tiebreak draw a stray byte, breaking the consumed() invariant.
    let provider = PokeredRules;
    let mut hs = Vec::new();
    collect_handlers(
        ctx,
        &provider,
        None, // no source effect: only the lowered battler's volatile handlers veto
        Event::TryBoost,
        lowered,
        lowered,
        &mut hs,
    );
    let verdict = run_event_checked(ctx, hs, RelayVar::Int(stages as i64), false);
    if matches!(verdict, RelayVar::Bool(false)) {
        // Vetoed by Mist / Substitute (the legacy StatBlocked path) — no drop, but
        // the chance byte was already consumed above (consumed() invariant).
        let _ = relay;
        return HandlerResult::Unchanged;
    }

    // Not vetoed ⇒ apply the stat drop via the binding (the engine's −6..+6 clamp,
    // matching legacy `StatStages::modify`).
    if let Some(idx) = host_stat_index(&stat_name) {
        let host = PokeredRules::rules_host().expect("pokered rules host installed");
        host.bindings.apply_boost(ctx.battler_mut(lowered), idx, stages);
    }
    HandlerResult::Unchanged
}

/// Resolve a stat name to the installed registry's interned stat index (the
/// driver's `Boost` application). Pure.
fn host_stat_index(name: &str) -> Option<usize> {
    PokeredRules::rules_host().and_then(|h| h.compiled.stats.iter().position(|s| s == name))
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. The data bridge fns (one per event). Each looks up its record's op-list by
//    (source_effect, its own static event), applies the chance gate, and folds
//    via `jrpg_rules::run_ops`. This is the genuine DATA path: the op-list comes
//    from rules.ron, not from native code.
// ─────────────────────────────────────────────────────────────────────────────

fn run_bridge(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    source_effect: EffectId,
    event: Event,
) -> HandlerResult {
    let Some(hook) = hook_for(source_effect, event) else {
        return HandlerResult::Unchanged;
    };
    // The chance gate — drawn UNCONDITIONALLY so draw order is a pure function of
    // the op-list (doc 11 §4.1). Bucket-A authors NO chance gate, so this is inert.
    if let Some((num, den)) = hook.chance {
        if !ctx.rng.chance(num, den) {
            return HandlerResult::Unchanged;
        }
    }
    let host = PokeredRules::rules_host().expect("pokered rules host installed");
    jrpg_rules::run_ops(ctx, relay, target, source, &host.bindings, &hook)
}

fn bridge_modify_damage(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    source_effect: EffectId,
) -> HandlerResult {
    run_bridge(ctx, relay, target, source, source_effect, Event::ModifyDamage)
}

fn bridge_effectiveness(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    source_effect: EffectId,
) -> HandlerResult {
    run_bridge(ctx, relay, target, source, source_effect, Event::Effectiveness)
}

fn bridge_after_move(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    source_effect: EffectId,
) -> HandlerResult {
    run_bridge(ctx, relay, target, source, source_effect, Event::AfterMove)
}

/// The `Accuracy`-event data bridge (Dream Eater's pre-hit sleep veto,
/// core.asm:5240-5245). A `VetoIf` here `Fail`s the fold → `Bool(false)` → the
/// driver marks the move missed BEFORE any accuracy/damage byte is drawn.
fn bridge_accuracy(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    source_effect: EffectId,
) -> HandlerResult {
    run_bridge(ctx, relay, target, source, source_effect, Event::Accuracy)
}

fn bridge_damaging_hit(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    source_effect: EffectId,
) -> HandlerResult {
    run_bridge(ctx, relay, target, source, source_effect, Event::DamagingHit)
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Shared helpers (re-homed from the stack_parity POC / pokered formula).
// ─────────────────────────────────────────────────────────────────────────────

fn battler<'a>(state: &'a EngineState<PokeredRules>, who: BattlerRef) -> &'a EngineBattler<PokeredRules> {
    if who.side == 0 {
        &state.player_battlers[who.slot as usize]
    } else {
        &state.opponent_battlers[who.slot as usize]
    }
}

/// The mover's level. The harness stores level 50 in the stat block via a
/// dedicated key absent from `StatIndex`; we store it as `max_hp`-adjacent? No —
/// the engine `BattlerState` has no level field, so the harness sets a Level via
/// a stat key. We use a fixed 50 (every P1 scenario is level 50, matching the
/// stack_parity POC) read defensively.
fn level_of(b: &EngineBattler<PokeredRules>) -> u8 {
    // The battler's real level (defaults to 50 via `BattlerState::new`, so the
    // existing level-50 differential scenarios are unchanged; the production
    // adapter sets it from the legacy `Pokemon.level`).
    b.level
}

/// Whether `effect` is a P3 special/fixed/OHKO/SuperFang damage effect — these
/// BYPASS the damage formula (the SetDamage / DamageCurrentHpFraction / SetHp op
/// writes `ctx.mv.damage`), so the native crit/damage handlers draw nothing for
/// them (only the accuracy byte), matching the standalone oracle's zero-rng path.
fn is_special_damage_effect(effect: MoveEffect) -> bool {
    matches!(
        effect,
        MoveEffect::SpecialDamageEffect | MoveEffect::SuperFangEffect | MoveEffect::OhkoEffect
    )
}

// ── Two-turn charge moves (Fly/Dig/Solar Beam/Razor Wind/Skull Bash/Sky Attack) ──

/// A move that charges one turn, strikes the next (FlyEffect = Fly; ChargeEffect =
/// Dig/Solar Beam/Razor Wind/Skull Bash/Sky Attack).
fn is_charge_move(effect: MoveEffect) -> bool {
    matches!(effect, MoveEffect::FlyEffect | MoveEffect::ChargeEffect)
}

/// Which charge moves make the user semi-invulnerable while charging (Fly/Dig only —
/// Solar Beam & the rest stay hittable). Keyed on the MOVE, since Dig shares
/// `ChargeEffect` with the non-invulnerable chargers.
fn charge_is_invulnerable(move_id: MoveId) -> bool {
    matches!(move_id, MoveId::Fly | MoveId::Dig)
}

/// The `(move, invulnerable)` of a `Charging` volatile on `who`, if any.
fn charging_of(ctx: &BattleCtx<'_, PokeredRules>, who: BattlerRef) -> Option<(MoveId, bool)> {
    ctx.effects.iter().find_map(|e| match &e.kind {
        PokeVolatile::Charging { move_, invulnerable } if e.host == who => {
            Some((*move_, *invulnerable))
        }
        _ => None,
    })
}

fn move_priority(move_id: MoveId) -> i8 {
    match move_id {
        MoveId::QuickAttack => 1,
        MoveId::Counter => -1,
        _ => 0,
    }
}

fn effective_speed(b: &EngineBattler<PokeredRules>) -> u16 {
    // Speed stat stage (Agility, String Shot, …) applies to turn order, then
    // paralysis quarters the result. Kept as pure u16 to preserve the Gen-1
    // speed-overflow behaviour (no 255/999 clamp).
    let base = b.stats.get(StatIndex::Speed).copied().unwrap_or(0);
    let stage = b.stat_stages.get(StatIndex::Speed).copied().unwrap_or(0);
    let staged = crate::battle::stat_stages::apply_stage(base, stage);
    if b.status == Some(LegacyStatus::Paralysis) {
        (staged / 4).max(1)
    } else {
        staged
    }
}

fn species_types(s: Species) -> (PokemonType, PokemonType) {
    get_base_stats(s)
        .map(|bs| (bs.type1, bs.type2))
        .unwrap_or((PokemonType::Normal, PokemonType::Normal))
}

/// The battler's EFFECTIVE types: a live [`PokeVolatile::TypeOverride`] (Conversion)
/// if present on `who`, else the species-derived types. Every in-turn type read that
/// must honour Conversion routes through here — the damage formula's attacker types
/// (STAB) and defender types (effectiveness / type-immunity), plus the
/// self-type-immunity quirk (`move_type_is_defender_type`).
///
/// KNOWN LIMITATION: the `HasType` binding (`has_type`, the `VetoIf(HasType(..))`
/// status-move type-immunity predicate) receives only `&EngineBattler` — no
/// `BattleCtx` and no `BattlerRef` — so it cannot reach the arena override and stays
/// species-based. A Conversion therefore does not alter *status-move* type immunity
/// (e.g. becoming Poison-type mid-battle to dodge a poison), an extremely narrow
/// interaction; covering it would need an engine trait-signature change.
fn effective_types(ctx: &BattleCtx<'_, PokeredRules>, who: BattlerRef) -> (PokemonType, PokemonType) {
    if let Some((t1, t2)) = ctx.effects.iter().find_map(|e| match &e.kind {
        PokeVolatile::TypeOverride { type1, type2 } if e.host == who => Some((*type1, *type2)),
        _ => None,
    }) {
        return (t1, t2);
    }
    species_types(ctx.battler(who).species)
}

/// Re-homes the `accuracy.rs` scaling chain (percentage→255, accuracy stage
/// ratio, inverted evasion stage ratio, clamp 255). Identical table to the POC.
fn scaled_accuracy(move_accuracy: u8, acc_stage: i8, eva_stage: i8) -> u8 {
    const STAGE_RATIOS: [(u32, u32); 13] = [
        (25, 100), (28, 100), (33, 100), (40, 100), (50, 100), (66, 100),
        (100, 100), (150, 100), (200, 100), (250, 100), (300, 100), (350, 100), (400, 100),
    ];
    let mut accuracy = (move_accuracy as u32 * 255 / 100).min(255);
    let (an, ad) = STAGE_RATIOS[(acc_stage + 6) as usize];
    accuracy = accuracy * an / ad;
    let (en, ed) = STAGE_RATIOS[((-eva_stage) + 6) as usize];
    accuracy = accuracy * en / ed;
    accuracy.min(255) as u8
}

/// Sanity helper for tests: assert the op-list of a record id contains an op.
#[allow(dead_code)]
pub fn record_has_op(source_id: &str, want: &Op) -> bool {
    MOVE_RECORDS.with(|r| {
        r.borrow()
            .iter()
            .find(|rec| rec.source_id == source_id)
            .map(|rec| rec.hooks.iter().any(|h| h.ops.iter().any(|o| o == want)))
            .unwrap_or(false)
    })
}

mod tests;

pub mod p5_native;
mod p5_tests;

/// P6 production runtime — drive a real battle through the stack (RNG, translator,
/// legacy↔engine adapter). Production (NOT test-gated).
pub mod runtime;
