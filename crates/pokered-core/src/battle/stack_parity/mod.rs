//! Reusable **effect-stack parity harness** (strangler slice 1, design doc
//! `06-battle-engine-effect-stack-design.md` §4.1 + §7).
//!
//! This is the harness **every later slice (2–7) depends on**. It extracts the
//! generic plumbing that used to be trapped inside `#[cfg(test)] stack_poc.rs`
//! so sibling test modules can add scenarios *without* rebuilding the byte-
//! stream shim, the `ScriptedRng` builder, the consumed-count predictor, or the
//! `BattleState` differential oracle.
//!
//! It is **additive and test-only** (`#![cfg(test)]`, `pub(crate)`): it does NOT
//! touch the production battle loop (`turn.rs` / `mod.rs`). The legacy
//! [`execute_turn`](super::turn::execute_turn) stays the oracle.
//!
//! ## What lives here (reusable by sibling slices)
//!
//! * The POC [`EffectProvider`] ([`PocData`]) + its registered [`Effect`]s and
//!   zero-capture `fn`-pointer handlers — the re-homed pokered battle math that
//!   later slices extend (more events, more handlers).
//! * The byte-vector ⇄ [`TurnRandoms`] ⇄ [`ScriptedRng`] shim
//!   ([`Scenario`]/[`MoveBytes`], [`legacy_run`], [`stack_run`],
//!   [`build_stack_stream`]).
//! * The differential oracle [`assert_state_parity`] (compares legacy
//!   `execute_turn` `BattleState` vs `StackDriver` `BattleState`: hp + status,
//!   both sides) and the [`run_scenario`] driver (state parity **and** the
//!   `consumed()` draw-order assertion).
//! * The **standing draw-order guard** ([`assert_crit_drawn_before_accuracy`]
//!   and its `#[test]`) — makes a crit-before-accuracy regression in
//!   `driver.rs` fail loudly (§4, bug-critical invariant).
//!
//! The handlers **re-home** pokered's native battle math (same formulas, call
//! shape changed) so parity is by construction, not reimplementation.

#![cfg(test)]

use dotzuki_engine::battle::stack::{
    BattleCtx, Effect, EffectId, EffectProvider, EffectState, EffectType, Event, EventHook,
    FirstMover, HandlerResult, RelayVar, StackDriver,
};
use dotzuki_engine::battle::{
    BattleAction, BattleProvider, BattleState as EngineState, BattlerRef,
    BattlerState as EngineBattler, DamageResult, EffectResult, EnumMap,
    MoveEffect as EngineMoveEffect,
};

use pokered_data::move_data::MoveData;
use pokered_data::moves::{MoveEffect, MoveId};
use pokered_data::pokemon_data::get_base_stats;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

use dotzuki_engine::battle::rng::ScriptedRng;

use super::effects::EffectRandoms;
use super::move_execution::MoveRandoms;
use super::state::{
    new_battle_state, status1, status2, BattleState as LegacyState, BattleType, Pokemon,
    StatusCondition as LegacyStatus,
};
use super::turn::{execute_turn, TurnRandoms};

// ─── The POC game provider ───────────────────────────────────────────────
//
// A minimal `EffectProvider` over pokered types. It carries no state; every
// number flows from the engine `BattlerState` (stats EnumMap + status) or from
// `get_base_stats`. Volatile Focus Energy rides as an `EffectState` condition,
// exactly the design-intended home (§3.1).

/// Stat keys for the POC engine battler (mirrors `StatIndex` shape). Accuracy /
/// Evasion (slice 3) are the keys the accuracy stage ratios read; they live on
/// the engine `BattlerState.stat_stages` `EnumMap` exactly like the legacy
/// `BattlerState.stat_stages` (`accuracy.rs:54,61`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stat {
    Attack,
    Defense,
    Speed,
    Special,
    Level,
    MaxHp,
    Accuracy,
    Evasion,
}

/// The game-supplied typed effect-state enum (design §3.1). The slice-1 harness
/// needs only the Focus-Energy marker; richer volatiles (Toxic counter,
/// Substitute hp, partial-trap turns) are the same shape and land in later slices.
///
/// All variants are POKERED-side (the engine treats `EffectStateKind` opaquely —
/// design §3.1 "provider-supplied / concrete-for-now"), so adding Substitute/
/// Trapping here keeps `dotzuki-engine` 100% game-agnostic (no engine change).
#[derive(Clone)]
#[allow(dead_code)] // `None` is the inert variant of the typed-state shape (§3.1)
pub enum PocKind {
    /// Inert.
    None,
    /// Focus Energy volatile (drives the `/4` crit bug).
    FocusEnergy,
    /// Confusion volatile (slice 2). Carries the remaining turn counter, the
    /// design-intended home for confusion (the engine `BattlerState` models only
    /// the non-volatile `status`; confusion is a Gen-1 *volatile* — `status1`
    /// `CONFUSED` + `confused_turns_left` in legacy `state.rs:65,148`). Rides as
    /// an `EffectState` exactly like Focus Energy, so the engine needs no
    /// Pokémon-specific field.
    Confusion { turns_left: u8 },
    /// **Substitute** volatile (slice 4, design §3.1 `Substitute{hp}`). Hosted on
    /// the DEFENDER; carries the substitute's remaining hp. The damage path
    /// redirects a hit into this hp instead of the defender's real hp (absorb),
    /// breaking the sub at 0 on overkill — mirroring legacy
    /// `move_execution.rs:288-300` (`substitute_hp` + `HAS_SUBSTITUTE_UP`).
    Substitute { hp: u16 },
    /// **Partial-trap** volatile (slice 4, design §3.1 `Trapping{turns_left}`).
    /// Hosted on the TRAPPING mover (the one using Wrap/Bind/…); its presence on
    /// a mon's OPPONENT prevents that mon from acting — mirroring legacy
    /// `status_checks.rs:60-64` (`opponent.has_status1(USING_TRAPPING_MOVE)`) and
    /// the turn counter of `multi_turn_effects.rs:31-49` (`num_attacks_left`).
    Trapping { turns_left: u8 },
    /// **Toxic** volatile (slice 5, design §3.1 `Toxic{counter}` + §3.3 bug #6).
    /// Hosted on the badly-poisoned mon; carries the ever-incrementing toxic
    /// counter. The legacy home is the `status3::BADLY_POISONED` flag PLUS the
    /// `BattlerState.toxic_counter` byte (`state.rs:149`, `residual.rs:34-37`);
    /// here BOTH live in this one volatile (presence ≙ the flag, `counter` ≙ the
    /// byte). The residual deals `floor(maxHP/16).max(1) * counter`, the counter
    /// incrementing each tick, UNCAPPED (bug #6).
    Toxic { counter: u8 },
    /// **Leech Seed** volatile (slice 5). Hosted on the SEEDED mon (presence ≙
    /// the legacy `status2::SEEDED` flag, `state.rs:76`). Its residual is the only
    /// genuinely CROSS-BATTLER residual: it drains `floor(maxHP/16).max(1)` from
    /// the host and heals the host's OPPONENT by the drained amount
    /// (`residual.rs:53-78`) — a real `pair_mut` exercise (drain target + heal
    /// opponent through one paired `&mut`).
    Seeded,
    /// **Locked move** volatile (slice 6, design §3.1 `LockedMove`, bug #17).
    /// Hosted on the THRASHING mon (≙ legacy `status1::THRASHING_ABOUT` +
    /// `num_attacks_left`, `multi_turn_effects.rs:73-93`). `turns_left` counts the
    /// remaining FORCED repetitions; each turn the engine's `forced_action` seam
    /// re-issues `move_` (ignoring the chosen action — the cross-turn proof). When
    /// `turns_left` reaches 0 the mon self-confuses (`confuse_on_end`) — Gen-1's
    /// Thrash/Petal-Dance fatigue confusion.
    LockedMove {
        /// The locked move re-issued each turn (Thrash).
        move_: MoveId,
        /// Remaining forced repetitions (counts DOWN to 0 → end).
        turns_left: u8,
        /// On expiry, self-confuse (Thrash/Petal Dance fatigue, bug #17).
        confuse_on_end: bool,
    },
    /// **Two-turn** volatile (slice 6, design §3.1 `TwoTurn`, bug #15). Hosted on
    /// the charging mon (≙ legacy `status1::CHARGING_UP` (+`INVULNERABLE` for
    /// Fly/Dig), `multi_turn_effects.rs:7-29`). `charging == true` is the charge
    /// turn (no damage, the forced action is `Nothing` — "gathering energy");
    /// next turn `forced_action` re-issues `move_` to strike, then the volatile is
    /// cleared. `invulnerable` mirrors Fly/Dig semi-invuln (modeled as the flag).
    TwoTurn {
        /// The two-turn move re-issued to strike on turn 2 (Fly/Solar Beam).
        move_: MoveId,
        /// True on the charge turn (forces `Nothing`); false on the strike turn.
        charging: bool,
        /// Fly/Dig semi-invulnerability during the charge turn.
        invulnerable: bool,
    },
    /// **Recharge** volatile (slice 6, design §3.1 / bug #14). Hosted on the mon
    /// that just used Hyper Beam (≙ legacy `status2::NEEDS_TO_RECHARGE`,
    /// `multi_turn_effects.rs:101-109`). Its presence forces `Nothing` next turn
    /// (the recharge skip), after which it is cleared.
    Recharge,
    /// **Bide** volatile (slice 6, design §3.1 `Bide{accumulated,turns_left}`,
    /// bug #18). Hosted on the biding mon (≙ legacy `status1::STORING_ENERGY` +
    /// `num_attacks_left` + `bide_accumulated_damage`,
    /// `multi_turn_effects.rs:52-71`). `accumulated` sums the damage the host took
    /// while biding; `turns_left` counts the storing turns; on the unleash turn it
    /// releases `accumulated * 2` (NOT ×3, bug #18) onto the opponent, then clears.
    Bide {
        /// Damage accumulated while storing energy.
        accumulated: u16,
        /// Remaining storing turns (Gen-1 stores for 2 turns, unleashes on the 3rd).
        turns_left: u8,
    },
    /// **Damage-taken-this-turn** scratch (slice 6, the §9 open-question resolution
    /// for CROSS-ACTION reads). Hosted on a mon; `amount` is the damage it took
    /// from the OPPONENT's move EARLIER this turn. Counter reads its host's entry
    /// (the damage the Counter user took) and reflects `amount * 2`; Bide's tick
    /// also folds it into `accumulated`. This is the canonical proof (design §9)
    /// that the per-mover `MoveContext.last_damage` is insufficient: Counter needs
    /// the damage recorded when the OTHER mover acted (a different `MoveContext`),
    /// so it must live PER-BATTLER in the arena, reset at the start of the turn.
    DamageTaken {
        /// Damage this host took from the opponent's move this turn (physical).
        amount: u16,
        /// Whether that damage was PHYSICAL (Counter only reflects physical, bug #20).
        physical: bool,
    },
    /// **Flinch** volatile (slice 7). Hosted on the FLINCHED defender (≙ legacy
    /// `status1::FLINCHED`, `special_effects.rs:20`). Set by a flinch-on-hit
    /// secondary; consumed by the slice-7 `flinch_gate` (a `BeforeMove` handler) on
    /// the host's NEXT action — the host loses that turn and the flinch clears. A
    /// flinch only bites if the flincher moved FIRST (the flinched mon has not yet
    /// acted), exactly as Gen-1 `FLINCHED` is checked-then-cleared each turn.
    Flinch,
}

/// Stable effect ids. Moves and statuses each map to one `Effect`.
const EFF_DAMAGING_MOVE: EffectId = EffectId(1);
const EFF_POISON_STATUS: EffectId = EffectId(10);
/// Arena id for the Focus-Energy volatile (host = mover).
pub const EFF_FOCUS_ENERGY: EffectId = EffectId(100);
/// Arena id for the Confusion volatile (host = mover; slice 2).
pub const EFF_CONFUSION: EffectId = EffectId(110);
/// Arena id base for the Substitute volatile (host = defender; slice 4). Player's
/// sub is `120`, opponent's `121`, so the arena's sorted-by-id invariant holds.
pub const EFF_SUBSTITUTE: EffectId = EffectId(120);
/// Arena id base for the partial-trap volatile (host = trapping mover; slice 4).
/// Player's trap is `130`, opponent's `131`.
pub const EFF_TRAPPING: EffectId = EffectId(130);
/// The Burn status' residual effect (slice 5). Same flat `floor(maxHP/16).max(1)`
/// as Poison; routed via `effect_for_status` because burn lives in the
/// non-volatile `status` byte (legacy `StatusCondition::Burn`).
pub const EFF_BURN_STATUS: EffectId = EffectId(11);
/// Arena id base for the Toxic volatile (host = badly-poisoned mon; slice 5).
/// Player's is `140`, opponent's `141`. Toxic sorts BEFORE Leech Seed (`150`),
/// so when a mon is both badly-poisoned AND seeded the arena-residual pass fires
/// the status damage (toxic) before the leech drain — the Gen-1 ASM order
/// (`residual.rs:84` "status first, then leech").
pub const EFF_TOXIC: EffectId = EffectId(140);
/// Arena id base for the Leech Seed volatile (host = seeded mon; slice 5).
/// Player's is `150`, opponent's `151` — sorted AFTER Toxic (`140`), pinning the
/// ASM "status damage then leech" order in the arena pass.
pub const EFF_SEEDED: EffectId = EffectId(150);
/// Arena id base for the slice-6 lock-in volatiles (LockedMove / TwoTurn /
/// Recharge / Bide). Player `160`, opponent `161` — sorted after every slice-1-5
/// id so the arena stays ordered. (Handlers address by host+kind, not id, so the
/// exact value is immaterial — only ordering matters for the binary-search arena.)
pub const EFF_LOCKIN: EffectId = EffectId(160);
/// Arena id base for the per-turn DamageTaken scratch (the §9 cross-action read).
/// Player `170`, opponent `171`. Reset (removed) at the start of every turn.
pub const EFF_DAMAGE_TAKEN: EffectId = EffectId(170);
/// The Counter move's effect (slice 6, bug #20). Counter reads its host's
/// `DamageTaken` and reflects `amount * 2` onto the opponent via `pair_mut`.
pub const EFF_COUNTER: EffectId = EffectId(2);
/// Arena id base for the Flinch volatile (host = flinched defender; slice 7).
/// Player `180`, opponent `181` — sorted after every slice-1-6 id so the arena
/// stays ordered.
pub const EFF_FLINCH: EffectId = EffectId(180);
/// The Haze move's effect (slice 7): a power-0 global stat/status reset. Distinct
/// move-effect id (never an arena entry).
pub const EFF_HAZE: EffectId = EffectId(3);

/// The slice's `EffectProvider`. Reusable by later slices, which extend the
/// registered effects/handlers rather than re-declaring the provider.
pub struct PocData;

impl BattleProvider for PocData {
    type Monster = ();
    type Move = MoveId;
    type Ability = ();
    type Status = LegacyStatus;
    type Stat = Stat;
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
        // Damage is computed inside the `ModifyDamage` handler (re-homing the
        // legacy formula); this trait method is unused by the stack harness.
        DamageResult {
            damage: 0,
            effectiveness: 1.0,
            is_miss: false,
        }
    }

    fn select_move(&self, b: &EngineBattler<Self>, _s: &EngineState<Self>) -> Self::Move {
        b.moves.first().cloned().unwrap_or(MoveId::Thundershock)
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

impl EffectProvider for PocData {
    type EffectStateKind = PocKind;

    fn effect_for_move(&self, m: &Self::Move) -> Option<&'static Effect<Self>> {
        // Slice 6: Counter routes to its own reactive Effect (reflect 2× physical
        // damage taken); Bide routes to a no-damage move effect (its accumulate/
        // unleash all happens in the volatile's Residual); every other move shares
        // the damage pipeline (a forced Thrash/Fly strike deals normal damage).
        match m {
            MoveId::Counter => Some(&COUNTER_EFFECT),
            MoveId::Bide => Some(&BIDE_MOVE_EFFECT),
            // Slice 7: Haze is a power-0 GLOBAL effect — it routes to a dedicated
            // effect with NO crit/damage hooks (only the BeforeMove gate + accuracy +
            // the global reset on `DamagingHit`), matching the legacy power-0 branch's
            // draw order (accuracy roll only, `move_execution.rs:78-101`).
            MoveId::Haze => Some(&HAZE_EFFECT),
            _ => Some(&DAMAGING_MOVE_EFFECT),
        }
    }

    fn effect_for_status(&self, s: &Self::Status) -> Option<&'static Effect<Self>> {
        match s {
            // Burn/Poison both live in the non-volatile `status` byte and both
            // tick flat `floor(maxHP/16).max(1)` (legacy `residual.rs:29-31`), so
            // they route through the status residual. (Slice 1 registered Poison;
            // slice 5 adds Burn.)
            LegacyStatus::Poison => Some(&POISON_EFFECT),
            LegacyStatus::Burn => Some(&BURN_EFFECT),
            _ => None,
        }
    }

    /// Volatile residuals (slice 5): Toxic (badly-poisoned) and Leech Seed both
    /// live in Gen-1 `status2`/`status3` bit flags — NOT the non-volatile `status`
    /// byte — so they CANNOT route through `effect_for_status` (a badly-poisoned-
    /// only or seeded-only mon has `status == None`). They ride the engine's
    /// generic arena-residual pass instead (design §3.4: every live effect on a
    /// battler contributes its `Residual` handler). All Gen-1 semantics stay here.
    fn effect_for_volatile(&self, kind: &Self::EffectStateKind) -> Option<&'static Effect<Self>> {
        match kind {
            PocKind::Toxic { .. } => Some(&TOXIC_EFFECT),
            PocKind::Seeded => Some(&LEECH_EFFECT),
            // Slice 6: the lock-in volatiles drive their lifecycle via a per-mover
            // `Residual` handler (decrement lock / flip charge→strike / clear
            // recharge / advance Bide). All Gen-1 semantics live in these handlers.
            PocKind::LockedMove { .. } => Some(&LOCKEDMOVE_EFFECT),
            PocKind::TwoTurn { .. } => Some(&TWOTURN_EFFECT),
            PocKind::Recharge => Some(&RECHARGE_EFFECT),
            PocKind::Bide { .. } => Some(&BIDE_EFFECT),
            _ => None,
        }
    }

    /// Turn-order rank — re-homes `turn_order::determine_order` exactly
    /// (priority table → effective speed with paralysis ÷4), drawing NO rng.
    /// The driver breaks an exact tie with one coin-flip byte (§2/§4).
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

    /// **Cross-turn lock-in** (slice 6, the §9 proof). Re-homes the legacy
    /// multi-turn action-override logic (`multi_turn_effects.rs`): a live volatile
    /// recorded on a PRIOR turn forces THIS turn's action, IGNORING the chosen one.
    /// ALL Gen-1 semantics live here (the engine seam only swaps one `BattleAction`
    /// for another, names no volatile):
    ///   * `LockedMove` (Thrash) → force `Fight{move_}` while `turns_left > 0`;
    ///   * `TwoTurn` charge turn → force `Nothing` (gathering energy); strike turn
    ///     → force `Fight{move_}`;
    ///   * `Recharge` (Hyper Beam) → force `Nothing` (the recharge skip);
    ///   * `Bide` storing → force `Fight{Bide}` (keep biding/unleashing).
    /// Returns `None` (chosen action stands) when no forcing volatile is live.
    fn forced_action(
        &self,
        effects: &[EffectState<Self>],
        actor: BattlerRef,
        chosen: &BattleAction<Self>,
    ) -> Option<BattleAction<Self>> {
        for e in effects.iter().filter(|e| e.host == actor) {
            match &e.kind {
                PocKind::LockedMove { move_, turns_left, .. } if *turns_left > 0 => {
                    return Some(BattleAction::Fight { move_: *move_ });
                }
                PocKind::TwoTurn { move_, charging, .. } => {
                    return Some(if *charging {
                        BattleAction::Nothing // charge turn: gather energy
                    } else {
                        BattleAction::Fight { move_: *move_ } // strike turn
                    });
                }
                PocKind::Recharge => {
                    return Some(BattleAction::Nothing); // Hyper Beam recharge skip
                }
                PocKind::Bide { .. } => {
                    return Some(BattleAction::Fight { move_: MoveId::Bide });
                }
                _ => {}
            }
        }
        let _ = chosen;
        None
    }
}

// ─── Effect registrations (the slice-1 handlers) ─────────────────────────

/// The damaging move's pipeline: the BeforeMove status gate (slice 2:
/// sleep → freeze → confusion → paralysis, in ASM order) → Focus Energy /4
/// crit (ModifyCritRatio) → accuracy (Accuracy) → damage roll (ModifyDamage).
///
/// ## Slice 2: the BeforeMove status gate as ordered handlers
///
/// The legacy oracle [`check_status_conditions`](super::super::status_checks)
/// runs the Gen-1 ASM order
/// `Sleep → Freeze → Trapped → Flinch → Recharge → (Disabled-ctr) → Confusion →
/// (Disabled-move) → Paralysis` (`status_checks.rs:23-35`). This slice re-homes
/// the *implemented subset* (Sleep, Freeze, Confusion, Paralysis) as four
/// `BeforeMove` handlers, ordering them by the comparator's `order` field
/// (ASCENDING = fires first, `dispatch.rs:60`). The chosen `order` values
/// preserve the relative ASM positions of the implemented statuses **exactly**:
///
/// | status     | ASM position | handler `order` |
/// |------------|--------------|-----------------|
/// | Sleep      | 1            | 10              |
/// | Freeze     | 2            | 20              |
/// | *(Trapped 3, Flinch 4, Recharge 5, Disabled-ctr 6 — STUBBED, not wired)* | | |
/// | Confusion  | 7            | 70              |
/// | *(Disabled-move 8 — STUBBED)* | | |
/// | Paralysis  | 9            | 90              |
///
/// The gaps (30..60, 80) are intentionally left for the stubbed statuses, so a
/// later slice slots Trapped/Flinch/Recharge/Disabled in without renumbering and
/// the order remains ASM-faithful. The draw order this produces — confusion byte
/// (order 70) **before** paralysis byte (order 90) — matches both the ASM and
/// pokered's `MoveRandoms` field order (`confusion_roll` then `paralysis_roll`,
/// `move_execution.rs:30-34`), so `consumed()` parity holds by construction.
static DAMAGING_MOVE_EFFECT: Effect<PocData> = Effect {
    id: EFF_DAMAGING_MOVE,
    kind: EffectType::Move,
    hooks: &[
        EventHook {
            event: Event::BeforeMove,
            call: sleep_gate,
            order: 10,
            priority: 0,
            sub_order: None,
        },
        EventHook {
            event: Event::BeforeMove,
            call: freeze_gate,
            order: 20,
            priority: 0,
            sub_order: None,
        },
        // Slice 4: partial-trap gate (ASM position 3, `order:30`). Slots into the
        // gap the slice-2 table reserved for Trapped (between Freeze:20 and
        // Confusion:70), so the ASM order stays faithful with NO renumbering.
        EventHook {
            event: Event::BeforeMove,
            call: trapped_gate,
            order: 30,
            priority: 0,
            sub_order: None,
        },
        // Slice 7: flinch gate (ASM position 4, `order:40`). Slots into the gap the
        // slice-2 table reserved for Flinch (between Trapped:30 and Confusion:70), so
        // the ASM order stays faithful with NO renumbering. Draws NO rng (the flinch
        // check reads a flag, not a byte) → `consumed()` unaffected for slices 1-6.
        EventHook {
            event: Event::BeforeMove,
            call: flinch_gate,
            order: 40,
            priority: 0,
            sub_order: None,
        },
        EventHook {
            event: Event::BeforeMove,
            call: confusion_gate,
            order: 70,
            priority: 0,
            sub_order: None,
        },
        EventHook {
            event: Event::BeforeMove,
            call: paralysis_gate,
            order: 90,
            priority: 0,
            sub_order: None,
        },
        EventHook {
            event: Event::ModifyCritRatio,
            call: focus_energy_crit,
            order: u32::MAX,
            priority: 0,
            sub_order: None,
        },
        EventHook {
            event: Event::Accuracy,
            call: accuracy_handler,
            order: u32::MAX,
            priority: 0,
            sub_order: None,
        },
        // The base damage formula. Slice 4 lowers its `order` from `u32::MAX` to a
        // finite value so the Substitute interceptor (below) can fire DETERMINISTICALLY
        // AFTER it on the SAME event, WITHOUT a tie (a tie would force the dispatch
        // speed-tiebreak to draw an rng byte — breaking `consumed()` parity). With a
        // single `ModifyDamage` hook in slices 1-3 this order change is behaviorally
        // inert (no tie ever arose); slices 1-3 still produce identical bytes/state.
        EventHook {
            event: Event::ModifyDamage,
            call: damage_handler,
            order: 1000,
            priority: 0,
            sub_order: None,
        },
        // Slice 4: Substitute interceptor (`order:2000` > damage's `1000`). Runs
        // AFTER the damage is computed, redirects it into the DEFENDER's Substitute
        // volatile, and zeroes `ctx.mv.damage` so the driver's unconditional
        // `take_damage` (driver.rs:165) leaves the defender's REAL hp untouched.
        // Draws NO rng → `consumed()` is identical to a non-substitute hit.
        EventHook {
            event: Event::ModifyDamage,
            call: substitute_interceptor,
            order: 2000,
            priority: 0,
            sub_order: None,
        },
        // Slice 7: the post-damage SECONDARY-effect handler (the design §1.1
        // `AfterMoveSecondary` shape). The engine has NO `AfterMoveSecondary` event;
        // the existing `DamagingHit` IS that seam — it fires AFTER the driver applied
        // `ctx.mv.damage` and set `ctx.mv.last_damage` (driver.rs:177-188), exactly
        // where recoil/drain read the damage DEALT this move and the secondary
        // status/stat/flinch rolls land. `order:100` < `record_damage_taken`'s
        // `u32::MAX` so it fires first (immaterial — it reads `mv.last_damage`, set
        // before either). For a `NoAdditionalEffect` move (every slice 1-6 move) it
        // returns immediately and draws NO byte, so slices 1-6 stay byte-identical.
        EventHook {
            event: Event::DamagingHit,
            call: secondary_handler,
            order: 100,
            priority: 0,
            sub_order: None,
        },
        // Slice 6: record the damage the DEFENDER took this turn into a per-turn
        // `DamageTaken` volatile, so Counter (cross-action) and Bide can read it.
        // Fires on `DamagingHit` (after the driver applied `ctx.mv.damage` and set
        // `ctx.mv.last_damage`). Draws NO rng → `consumed()` is unchanged for
        // slices 1-5 (which never inspect `DamageTaken`; the entry is invisible to
        // their `PocKind`-filtered arena reads). This is the §6 #18/#20 "Damage/
        // DamagingHit — Bide/Counter read" hook.
        EventHook {
            event: Event::DamagingHit,
            call: record_damage_taken,
            order: u32::MAX,
            priority: 0,
            sub_order: None,
        },
    ],
};

/// The Counter move's effect (slice 6, bug #20). On `ModifyDamage` it reads its
/// HOST's per-turn `DamageTaken` volatile (the damage the Counter user took from
/// the opponent's PHYSICAL move earlier this turn) and reflects `amount * 2` onto
/// the opponent — the canonical cross-action / `pair_mut` handler. Counter's −1
/// priority (always last) is already encoded in `move_priority`/`turn_order_rank`.
static COUNTER_EFFECT: Effect<PocData> = Effect {
    id: EFF_COUNTER,
    kind: EffectType::Move,
    hooks: &[
        // Counter reuses the standard BeforeMove status gate (a paralyzed/asleep
        // Counter user still cannot act) — share the same handlers as the damaging
        // move so the gate behaviour is identical.
        EventHook {
            event: Event::BeforeMove,
            call: paralysis_gate,
            order: 90,
            priority: 0,
            sub_order: None,
        },
        // Counter is a fixed-damage reactive move: NO crit, NO accuracy roll, NO
        // damage roll — it deals exactly 2× the physical damage taken (or fails).
        // So it registers ONLY a `ModifyDamage` handler (drawing no rng), and the
        // driver's crit/accuracy `fire` calls collect zero hooks → no bytes.
        EventHook {
            event: Event::ModifyDamage,
            call: counter_handler,
            order: 1000,
            priority: 0,
            sub_order: None,
        },
    ],
};

/// The Poison status' residual (`order:10`, min-1, per-mover — design §6 #7).
static POISON_EFFECT: Effect<PocData> = Effect {
    id: EFF_POISON_STATUS,
    kind: EffectType::Status,
    hooks: &[EventHook {
        event: Event::Residual,
        call: poison_residual,
        order: 10,
        priority: 0,
        sub_order: None,
    }],
};

/// The Burn status' residual (slice 5). Identical flat `floor(maxHP/16).max(1)`
/// tick as Poison (`residual.rs:29-31`), reusing the same handler — burn vs
/// poison differ only in the status byte they ride, not in the residual math.
/// `order:10` = the ASM "status damage" rank (design §6 #7).
static BURN_EFFECT: Effect<PocData> = Effect {
    id: EFF_BURN_STATUS,
    kind: EffectType::Status,
    hooks: &[EventHook {
        event: Event::Residual,
        call: poison_residual, // flat /16, min 1 — same math as poison
        order: 10,
        priority: 0,
        sub_order: None,
    }],
};

/// The Toxic (badly-poisoned) residual (slice 5, design §3.3 bug #6). Lives on a
/// `PocKind::Toxic` volatile (≙ legacy `status3::BADLY_POISONED` + the
/// `toxic_counter` byte). `order:10` = the ASM "status damage" rank — the SAME
/// rank as burn/poison, because a mon is never two status-damage sources at once
/// in Gen-1; it sorts before leech via its arena id (`140 < 150`).
static TOXIC_EFFECT: Effect<PocData> = Effect {
    id: EFF_TOXIC,
    kind: EffectType::Condition,
    hooks: &[EventHook {
        event: Event::Residual,
        call: toxic_residual,
        order: 10, // status damage rank (same as burn/poison)
        priority: 0,
        sub_order: None,
    }],
};

/// The Leech Seed residual (slice 5). Lives on a `PocKind::Seeded` volatile (≙
/// legacy `status2::SEEDED`). `order:30` (> the status-damage `10`), AND its
/// arena id (`150 > 140`) — both encode the ASM "status damage FIRST, then leech"
/// order (`residual.rs:84`). The ONLY cross-battler residual: drains the host and
/// heals the host's opponent via `ctx.pair_mut` (genuinely load-bearing).
static LEECH_EFFECT: Effect<PocData> = Effect {
    id: EFF_SEEDED,
    kind: EffectType::Condition,
    hooks: &[EventHook {
        event: Event::Residual,
        call: leech_residual,
        order: 30, // AFTER status damage (burn/poison/toxic at order 10)
        priority: 0,
        sub_order: None,
    }],
};

/// `BeforeMove` sleep gate (slice 2, ASM position 1, `order:10`). Re-homes
/// `status_checks.rs:35-52`: decrement the sleep counter; while it is still > 0
/// the mon stays asleep and cannot move; on the tick that reaches 0 the mon
/// wakes but **still loses the turn** (the deliberate Gen-1 "wake costs a turn"
/// behaviour, bug #8). Draws **NO** rng (sleep is counter-driven), so it adds
/// zero bytes to `consumed()` — matching the legacy gate which reads no byte
/// here. Mutates the engine `status` in place: `Sleep(n) → Sleep(n-1)` or
/// `Sleep(1) → None` (awake). Returns `Fail` whenever asleep this turn (both the
/// still-asleep and the just-woke tick), else `Unchanged`.
fn sleep_gate(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let counter = match ctx.battler(source).status {
        Some(LegacyStatus::Sleep(c)) => c,
        _ => return HandlerResult::Unchanged,
    };
    if counter == 0 {
        // Defensive: a `Sleep(0)` should not exist, but treat it as awake.
        ctx.battler_mut(source).status = None;
        return HandlerResult::Unchanged;
    }
    let new_counter = counter - 1;
    ctx.battler_mut(source).status = if new_counter == 0 {
        None // woke up …
    } else {
        Some(LegacyStatus::Sleep(new_counter))
    };
    // … but ASM forfeits the turn even on the wake tick (bug #8).
    HandlerResult::Fail
}

/// `BeforeMove` freeze gate (slice 2, ASM position 2, `order:20`). Re-homes
/// `status_checks.rs:54-58`: a frozen mon **always** cannot move and there is no
/// per-turn thaw roll in Gen 1 (bug #10) — so this draws **NO** rng and never
/// clears the status itself. Returns `Fail` while frozen, else `Unchanged`.
fn freeze_gate(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if ctx.battler(source).status == Some(LegacyStatus::Freeze) {
        HandlerResult::Fail // permanently blocked, no thaw draw
    } else {
        HandlerResult::Unchanged
    }
}

/// `BeforeMove` confusion gate (slice 2, ASM position 7, `order:70`). Re-homes
/// `status_checks.rs:88-101` + the self-hit damage application in
/// `move_execution.rs:62-69`. Confusion is a **volatile** carried in the
/// `EffectState` arena ([`PocKind::Confusion`]) — the engine `BattlerState`
/// models only non-volatile `status`. Behaviour:
///   * decrement `turns_left` (min 0);
///   * if it hits 0 → confusion ends (remove the volatile) and the mon may act,
///     drawing **NO** confusion byte (matches legacy "snap out" which skips the
///     `random < 128` read);
///   * otherwise draw ONE confusion byte; `< 128` → 50% self-hit: apply the
///     typeless 40-power self-damage to `source` and `Fail` (move aborted);
///     `>= 128` → the mon acts normally.
///
/// Drawing the byte ONLY when still confused-and-not-snapping-out keeps
/// `consumed()` identical to the legacy meaningful-read, and the `order:70`
/// places this draw BEFORE the paralysis draw (`order:90`) — the ASM /
/// `MoveRandoms` field order.
fn confusion_gate(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    // Find the confusion volatile on the mover, if any.
    let idx = ctx
        .effects
        .iter()
        .position(|e| e.host == source && matches!(e.kind, PocKind::Confusion { .. }));
    let Some(idx) = idx else {
        return HandlerResult::Unchanged; // not confused
    };

    // Decrement turns_left (the legacy gate decrements before the snap-out test).
    let turns_after = {
        let PocKind::Confusion { turns_left } = &mut ctx.effects[idx].kind else {
            return HandlerResult::Unchanged;
        };
        if *turns_left > 0 {
            *turns_left -= 1;
        }
        *turns_left
    };

    if turns_after == 0 {
        // Snap out of confusion: remove the volatile, no byte drawn, mon acts.
        ctx.effects.remove(idx);
        return HandlerResult::Unchanged;
    }

    // Still confused: 50% self-hit (legacy `random_confusion < 128`).
    let roll = ctx.rng.next_u8();
    if roll < 128 {
        let self_damage = confusion_self_hit_damage(ctx, source);
        ctx.battler_mut(source).take_damage(self_damage);
        return HandlerResult::Fail; // hit itself → move aborted
    }
    HandlerResult::Unchanged // confused but acts this turn
}

/// Confusion self-hit damage — re-homes `move_execution.rs:177-201`
/// (`calc_confusion_self_hit`): a typeless 40-power physical hit using the
/// mover's OWN Attack vs OWN Defense, no crit, max damage roll (255). Reads the
/// engine stats (which the harness sets to the same values as the legacy
/// `Pokemon`, so the computed damage is identical by construction).
fn confusion_self_hit_damage(ctx: &BattleCtx<'_, PocData>, who: BattlerRef) -> u16 {
    use super::damage::{calculate_damage, DamageParams};
    let b = ctx.battler(who);
    let level = b.stats.get(Stat::Level).copied().unwrap_or(50) as u8;
    let atk = b.stats.get(Stat::Attack).copied().unwrap_or(0);
    let def = b.stats.get(Stat::Defense).copied().unwrap_or(1);
    let params = DamageParams {
        attacker_level: level,
        move_power: 40,
        move_type: PokemonType::Normal,
        move_id: MoveId::None,
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
        attacker_burned: false,
    };
    calculate_damage(&params).damage
}

/// `BeforeMove` paralysis gate (slice 2, ASM position 9, `order:90`). Re-homes
/// `status_checks.rs:109-115`: 25% full paralysis (`paralysis_roll < 63`). Draws
/// a byte **only when paralyzed**, so the streamed `consumed()` matches the
/// legacy "meaningful read" exactly. Its `order:90` is AFTER confusion
/// (`order:70`), so the paralysis byte is always drawn after the confusion byte
/// — the ASM / `MoveRandoms` field order.
///
/// Returns `Fail` to abort the move (fully paralyzed) or `Unchanged` to proceed.
fn paralysis_gate(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    // `source` is the mover; the gate reads the mover's own status.
    let paralyzed = ctx.battler(source).status == Some(LegacyStatus::Paralysis);
    if paralyzed {
        let roll = ctx.rng.next_u8();
        if roll < 63 {
            return HandlerResult::Fail; // fully paralyzed → move aborted
        }
    }
    HandlerResult::Unchanged
}

/// `ModifyCritRatio` — the full Gen-1 crit pipeline (slice 3, generalizes slice
/// 1's Focus-Energy-only stand-in). Re-homes `damage.rs:25` (`is_high_crit_move`)
/// + `damage.rs:33-49` (`crit_chance`): threshold = base_speed/2 for a normal
/// move, ×8 for a high-crit move (Slash/Razor Leaf/Karate Chop/Crabhammer), then
/// the deliberate Focus-Energy `/4` BUG (#1), clamped to 255. This MIRRORS the
/// legacy [`test_critical_hit`](super::move_execution) exactly: same
/// `is_high_crit_move(move.id)`, same `crit_chance(base_speed, high_crit, focus)`,
/// same `crit_roll < threshold` test. base_speed is the SPECIES base speed (Gen-1
/// ignores the in-battle Speed stat/stage, bug #3); Focus Energy rides as an
/// `EffectState` volatile (NOT an `if gen == 1` in the engine). Draws the crit
/// byte here, BEFORE accuracy (design §4).
fn focus_energy_crit(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let species = ctx.battler(source).species;
    let base_speed = get_base_stats(species).map_or(0, |s| s.speed);
    // High-crit comes from the move id (Slash/Razor Leaf/…), exactly like legacy.
    let is_high_crit = super::damage::is_high_crit_move(poc_move_data().id);
    // Focus Energy rides as an EffectState condition hosted on the mover.
    let is_focus = ctx
        .effects
        .iter()
        .any(|e| e.host == source && matches!(e.kind, PocKind::FocusEnergy));
    let threshold = super::damage::crit_chance(base_speed, is_high_crit, is_focus);
    let crit_roll = ctx.rng.next_u8();
    ctx.mv.is_critical = crit_roll < threshold;
    HandlerResult::Unchanged
}

/// `Accuracy` — the full Gen-1 hit check (slice 3, generalizes slice 1's
/// base-accuracy-only stand-in). Re-homes `accuracy.rs:50-67`: percentage→0..255
/// (`acc*255/100`), then the attacker's ACCURACY stage ratio and the target's
/// EVASION stage ratio (inverted index, `14 - evasion`), clamped to 255, with the
/// `byte < accuracy` test — so byte 255 vs a 100% move is the deliberate Gen-1
/// 1/256 miss (#2). The stages are read from the engine `stat_stages` `EnumMap`
/// (Accuracy on the attacker, Evasion on the target), the SAME stages the legacy
/// `accuracy_check` reads off its `BattlerState.stat_stages`, so this MIRRORS the
/// legacy fn input-for-input. Returns `Bool(false)` to STOP the pipeline on a
/// miss. Draws the accuracy byte here, AFTER crit (design §4).
fn accuracy_handler(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let move_data = poc_move_data();
    let (accuracy, effect) = (move_data.accuracy, move_data.effect);
    if effect == MoveEffect::SwiftEffect {
        return HandlerResult::Unchanged; // never misses; draws nothing (parity)
    }
    let acc_stage = stage(ctx, source, Stat::Accuracy);
    let eva_stage = stage(ctx, target, Stat::Evasion);
    let scaled = scaled_accuracy(accuracy, acc_stage, eva_stage);
    let byte = ctx.rng.next_u8();
    if byte < scaled {
        HandlerResult::Unchanged
    } else {
        HandlerResult::Set(RelayVar::Bool(false)) // miss → STOP
    }
}

/// Read a stat stage from the engine battler (defaults to 0 → slices 1/2 see no
/// stages). The Accuracy/Evasion keys are set on the engine `stat_stages` by
/// `engine_battler_dmg`; Attack/Defense/Special stages likewise.
fn stage(ctx: &BattleCtx<'_, PocData>, who: BattlerRef, stat: Stat) -> i8 {
    ctx.battler(who).stat_stages.get(stat).copied().unwrap_or(0)
}

/// Re-homes the `accuracy.rs:50-65` scaling chain (percentage→255, accuracy
/// stage ratio, inverted evasion stage ratio, clamp 255). Shared by the handler
/// and the slice-3 stream predictor so both compute the SAME scaled threshold.
fn scaled_accuracy(move_accuracy: u8, acc_stage: i8, eva_stage: i8) -> u8 {
    // ASM StatModifierRatios — same table as accuracy.rs:7-21.
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

/// `ModifyDamage` — the full Gen-1 damage formula (slice 3, generalizes slice 1's
/// fixed-power stand-in). Re-homes `damage.rs:calculate_damage` end-to-end via the
/// legacy fn itself: stat overflow `>>2` when either stat > 255 (#5), crit doubles
/// the level term (#... `damage.rs:103-107`), type chart → STAB / super-effective /
/// not-very-effective / immunity-as-miss (#4), and the final `type_damage * roll /
/// 255` with the 217..=255 roll range (#29) — drawing the damage byte at THIS site.
/// It MIRRORS the legacy [`calc_and_apply_damage`](super::move_execution): same
/// physical/special atk-def selection, same attack/defense STAGES (read from the
/// engine `stat_stages`, the same the legacy path sets), same move power/type/id and
/// attacker/defender types, fed into the SAME `calculate_damage`. On a type-immunity
/// "miss" it returns `Bool(false)` to short-circuit the Hit chain (#4). Draws the
/// damage byte here, after accuracy (design §4).
fn damage_handler(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    use super::damage::{calculate_damage, is_physical, DamageParams};

    let damage_roll = ctx.rng.next_u8();
    let is_crit = ctx.mv.is_critical;

    let pm = poc_move_data();
    let move_id = pm.id;
    let move_type = pm.move_type;
    let physical = is_physical(move_type);
    let (atk, def, atk_stage, def_stage, atype1, atype2, dtype1, dtype2, level) = {
        let a = ctx.battler(source);
        let d = ctx.battler(target);
        let (atk, def) = if physical {
            (
                a.stats.get(Stat::Attack).copied().unwrap_or(0),
                d.stats.get(Stat::Defense).copied().unwrap_or(1),
            )
        } else {
            (
                a.stats.get(Stat::Special).copied().unwrap_or(0),
                d.stats.get(Stat::Special).copied().unwrap_or(1),
            )
        };
        // Stages: physical uses Attack/Defense, special uses Special on both —
        // exactly like `calc_and_apply_damage` (`move_execution.rs:232-236`).
        let (atk_stage, def_stage) = if physical {
            (
                a.stat_stages.get(Stat::Attack).copied().unwrap_or(0),
                d.stat_stages.get(Stat::Defense).copied().unwrap_or(0),
            )
        } else {
            (
                a.stat_stages.get(Stat::Special).copied().unwrap_or(0),
                d.stat_stages.get(Stat::Special).copied().unwrap_or(0),
            )
        };
        (
            atk,
            def,
            atk_stage,
            def_stage,
            species_types(a.species).0,
            species_types(a.species).1,
            species_types(d.species).0,
            species_types(d.species).1,
            a.stats.get(Stat::Level).copied().unwrap_or(50) as u8,
        )
    };

    let params = DamageParams {
        attacker_level: level,
        move_power: pm.power,
        move_type,
        move_id,
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
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        attacker_burned: false,
    };
    let result = calculate_damage(&params);
    ctx.mv.damage = if result.is_miss { 0 } else { result.damage };
    if result.is_miss {
        ctx.mv.move_missed = true;
        return HandlerResult::Set(RelayVar::Bool(false)); // type-immunity → "miss"
    }
    HandlerResult::Unchanged
}

/// `ModifyDamage` — **the slice-4 Substitute interceptor** (`order:2000`, AFTER
/// `damage_handler`'s `1000`). Re-homes the legacy absorb/break at
/// `move_execution.rs:288-300`: when the DEFENDER has a Substitute up, the computed
/// damage reduces the sub's hp instead of the defender's real hp; on overkill
/// (`damage >= sub_hp`) the sub breaks (hp → 0, volatile removed); the defender's
/// REAL hp is never touched while a sub stands.
///
/// ## The cross-battler exercise (`pair_mut`) — and an HONEST scoping note
///
/// This is the slice's stress goal: drive the engine's cross-battler
/// [`pair_mut`](BattleCtx::pair_mut) with a REAL handler. The interceptor takes
/// `ctx.pair_mut(source, target)` to hold the **attacker** and **defender** as two
/// `&mut` simultaneously. Because mover and defender are on OPPOSITE sides in single
/// battle, this hits the **cross-side raw-pointer branch** (ctx.rs:150-158) — the
/// sole engine `unsafe` — exercised here under genuine load (not the synthetic
/// slice-1 probe). The borrow does real work both ways: it READS the attacker's
/// `hp` (a live-attacker guard a real interceptor performs) and, via the SAME paired
/// `&mut`, captures and re-pins the defender's real `hp` so the redirect is proven
/// not to spill into it. This compiles with NO `RefCell`/`Rc` — the design's borrow
/// claim (§6 "Borrow") holds for a real handler.
///
/// **Honest finding (the slice was designed to surface this):** the Substitute
/// absorb does NOT *strictly require* `pair_mut`. The sub hp lives in the
/// `EffectState` arena (`PocKind::Substitute` on `target`) and the rolled damage is
/// in `ctx.mv` scratch — so the minimal absorb needs only `ctx.effects` + `ctx.mv`,
/// not both battler refs. `pair_mut` becomes load-bearing only when the volatile
/// lives ON the battler (legacy's `BattlerState.substitute_hp`) or when a Counter-
/// shaped handler must mutate `target` while reading `source`'s host fields. We
/// model the volatile in the arena (the engine `BattlerState` is game-agnostic and
/// has no `substitute_hp` field), so this handler uses `pair_mut` to *prove the seam
/// works for a real cross-side handler*, while the absorb itself routes through the
/// arena. See the slice-4 return notes for the full borrow-friction report.
///
/// Draws NO rng — the damage byte was already drawn by `damage_handler` — so
/// `consumed()` is identical to a non-sub hit. Returns `Set(Bool(false))` after
/// redirecting and zeroes `ctx.mv.damage` so the driver's unconditional
/// `take_damage(target, dmg)` at driver.rs:165 is skipped (`dmg == 0`) — the
/// defender's real hp stays intact.
fn substitute_interceptor(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    // Is there a Substitute volatile hosted on the DEFENDER (target)?
    let sub_idx = ctx
        .effects
        .iter()
        .position(|e| e.host == target && matches!(e.kind, PocKind::Substitute { .. }));
    let Some(sub_idx) = sub_idx else {
        return HandlerResult::Unchanged; // no sub → normal damage path
    };

    let damage = ctx.mv.damage;
    if damage == 0 {
        return HandlerResult::Unchanged; // nothing to absorb (e.g. immunity-miss)
    }

    // ── The REAL cross-battler borrow: hold attacker + defender `&mut` together.
    //    Cross-side (mover vs defender) → the raw-pointer branch of pair_mut. We
    //    read the attacker's live state THROUGH the pair while re-pinning the
    //    defender's real hp through the SAME borrow — the Counter/Substitute-shaped
    //    "mutate target while reading source" the design (§3.2) says pair_mut is for.
    let real_hp_preserved = {
        let (atk, def) = ctx.pair_mut(source, target);
        // Live-attacker guard: a fainted attacker deals no absorb (read `source`).
        if atk.hp == 0 {
            return HandlerResult::Unchanged;
        }
        // Re-pin the defender's real hp through the paired `&mut` (write `target`):
        // the sub absorbs ALL of this hit, so real hp is explicitly held constant.
        let before = def.hp;
        def.hp = before; // no-op by intent; proves the paired write path is live
        before
    };

    // Absorb into the volatile (legacy move_execution.rs:289-296).
    let PocKind::Substitute { hp } = &mut ctx.effects[sub_idx].kind else {
        return HandlerResult::Unchanged;
    };
    let sub_hp = *hp;
    if damage >= sub_hp {
        // Overkill → the substitute BREAKS: hp 0 + volatile removed.
        ctx.effects.remove(sub_idx);
    } else {
        *hp = sub_hp - damage;
    }

    // Redirect complete: zero the move damage so the driver does NOT touch real hp.
    // (The defender's real hp must equal what we re-pinned above — the driver's
    // `take_damage` is skipped because `dmg == 0`.)
    debug_assert_eq!(ctx.battler(target).hp, real_hp_preserved);
    ctx.mv.damage = 0;
    HandlerResult::Set(RelayVar::Bool(false)) // STOP the ModifyDamage chain
}

/// `BeforeMove` partial-trap gate (slice 4, ASM position 3, `order:30`). Re-homes
/// `status_checks.rs:60-64`: if the mover's OPPONENT is currently using a trapping
/// move (Wrap/Bind/FireSpin/Clamp), the mover CANNOT act this turn. The trap turns
/// live in a `PocKind::Trapping` volatile hosted on the TRAPPING mon; the gate
/// checks for that volatile on `target` (the mover's opponent). Draws NO rng (the
/// trapped check reads no byte in legacy), so `consumed()` is unaffected.
///
/// ## Scope (in-turn now; cross-turn deferred to slice 6)
///
/// This implements the IN-TURN interception part legacy `execute_turn` actually
/// exercises: a PRE-EXISTING trap flag on the opponent forfeits the trapped mon's
/// action. The turn-counter lifecycle (`apply_trapping` setting `num_attacks_left`
/// on hit, decrementing it across turns, auto-repeating the trapping move, and the
/// end-of-turn trap chip) flows through the move-EFFECT dispatch + multi-turn
/// machinery — cross-turn state that is slice 6 territory (the harness move is
/// `NoAdditionalEffect`, so `execute_turn` never invokes `apply_trapping` here).
/// The scenarios therefore PRE-SET the trap volatile + turn counter and assert the
/// in-turn forfeit + counter at parity with the legacy trapped gate.
///
/// Returns `Fail` (move aborted) when trapped, else `Unchanged`.
fn trapped_gate(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let _ = source;
    // Trapped iff the mover's OPPONENT (target) hosts an active Trapping volatile.
    let trapped = ctx
        .effects
        .iter()
        .any(|e| e.host == target && matches!(e.kind, PocKind::Trapping { turns_left } if turns_left > 0));
    if trapped {
        HandlerResult::Fail // opponent's trap forfeits this mon's action (no draw)
    } else {
        HandlerResult::Unchanged
    }
}

/// `Residual` — Poison/Burn tick. Re-homes the flat arm of the FIXED
/// `residual.rs::apply_residual_status_damage`: `floor(maxHP/16)`, min 1, can
/// KO. A BADLY-poisoned host (live `PocKind::Toxic` volatile ≙ the legacy
/// `BADLY_POISONED` flag) is ticked by the Toxic ramp INSTEAD — the oracle's
/// flag check runs first, so exactly ONE chip lands. Side-effecting; relay
/// untouched. Draws NO rng.
fn poison_residual(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    // Badly-poisoned → the Toxic volatile ramp owns the tick (avoid double-chip).
    let badly = ctx
        .effects
        .iter()
        .any(|e| e.host == target && matches!(e.kind, PocKind::Toxic { .. }));
    if badly {
        return HandlerResult::Unchanged;
    }
    let max_hp = ctx.battler(target).max_hp;
    let dmg = (max_hp / 16).max(1);
    ctx.battler_mut(target).take_damage(dmg);
    HandlerResult::Unchanged
}

/// `Residual` — Toxic tick (slice 5, design §3.3 bug #6). Re-homes the ramp arm
/// of the FIXED `residual.rs::apply_residual_status_damage`: the oracle now
/// checks the `BADLY_POISONED` flag FIRST (a badly-poisoned mon's non-volatile
/// status IS `Poison`, and previously matched the flat Burn|Poison arm, leaving
/// this ramp dead — D7). So this handler ramps UNCONDITIONALLY whenever the
/// `PocKind::Toxic` volatile (≙ legacy `BADLY_POISONED` + `toxic_counter`) is
/// live: increment the counter and deal `floor(maxHP/16).max(1) * counter`,
/// UNCAPPED (`saturating_mul`, bug #6). The flat Poison status residual skips
/// while this volatile is live (one chip, not two — see `poison_residual`).
/// Draws NO rng (fixed `/16` math — Gen-1 residuals are deterministic), so it
/// adds ZERO bytes to `consumed()`.
fn toxic_residual(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    // Increment the toxic counter held in the volatile (≙ legacy `toxic_counter`).
    let counter = {
        let idx = ctx
            .effects
            .iter()
            .position(|e| e.host == target && matches!(e.kind, PocKind::Toxic { .. }));
        let Some(idx) = idx else {
            return HandlerResult::Unchanged; // not badly poisoned
        };
        let PocKind::Toxic { counter } = &mut ctx.effects[idx].kind else {
            return HandlerResult::Unchanged;
        };
        *counter = counter.saturating_add(1);
        *counter as u16
    };
    let max_hp = ctx.battler(target).max_hp;
    let base = (max_hp / 16).max(1);
    let dmg = base.saturating_mul(counter); // bug #6: uncapped multiply
    ctx.battler_mut(target).take_damage(dmg);
    HandlerResult::Unchanged
}

/// `Residual` — Leech Seed tick (slice 5). Re-homes the FIXED
/// `residual.rs::apply_leech_seed`: drain `floor(maxHP/16).max(1)` (capped at
/// the host's hp) from the SEEDED host, and heal the host's OPPONENT by the
/// actually-drained amount (capped at the opponent's `max_hp`) — PLUS the Gen-1
/// Toxic × Leech Seed interaction (D11): a badly-poisoned host's Toxic counter
/// increments again here and scales the drain. Fires AFTER status damage
/// (`order:30` + arena id `150`), so a status-damage KO this tick suppresses it
/// — which this handler ALSO honors directly by short-circuiting on a dead host,
/// matching the legacy `apply_all_residual` early-return after a `Fainted`
/// status result (`residual.rs`: leech is not applied, opponent is not healed).
///
/// ## The genuinely load-bearing `pair_mut` (vs slice 4's note)
///
/// Slice 4 honestly reported its Substitute absorb did NOT *require* `pair_mut`
/// (the sub hp lived in the arena, the damage in `ctx.mv` scratch). Leech Seed is
/// different: the drain reduces the HOST's real hp AND the heal raises the
/// OPPONENT's real hp — two distinct battlers' real `hp`, mutated in one tick. We
/// take `ctx.pair_mut(host, opponent)` to hold BOTH as `&mut` simultaneously and
/// move hp across them. Because host and opponent are on opposite sides in single
/// battle, this is the cross-side raw-pointer branch (ctx.rs:150-158) under REAL
/// load: both writes are essential (drop either and the residual is wrong), so
/// `pair_mut` here is load-bearing in a way slice 4's never was. Draws NO rng.
fn leech_residual(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    // The seeded host is the residual `target` (host == target == source here).
    let host = target;
    // Verify the Seeded volatile is still live on the host.
    let seeded = ctx
        .effects
        .iter()
        .any(|e| e.host == host && matches!(e.kind, PocKind::Seeded));
    if !seeded {
        return HandlerResult::Unchanged;
    }
    // ASM/legacy short-circuit: a status-damage KO earlier this tick suppresses
    // leech entirely (no drain, no heal) — `residual.rs:90-97`.
    if ctx.battler(host).hp == 0 {
        return HandlerResult::Unchanged;
    }

    let opponent = BattlerRef::new(if host.side == 0 { 1 } else { 0 }, host.slot);
    let max_hp = ctx.battler(host).max_hp;
    let base = (max_hp / 16).max(1);
    // Gen-1 TOXIC × LEECH SEED bug (reproduced in the fixed oracle, D11): the
    // drain runs through the SAME HP-decrease routine as poison
    // (HandlePoisonBurnLeechSeed_DecreaseOwnHP, core.asm:550+), so a
    // badly-poisoned host's Toxic counter increments a SECOND time this turn
    // and the drain scales with it.
    let tox_idx = ctx
        .effects
        .iter()
        .position(|e| e.host == host && matches!(e.kind, PocKind::Toxic { .. }));
    let drain = match tox_idx {
        Some(idx) => {
            let n = match &mut ctx.effects[idx].kind {
                PocKind::Toxic { counter } => {
                    *counter = counter.saturating_add(1);
                    *counter as u16
                }
                _ => return HandlerResult::Unchanged,
            };
            base.saturating_mul(n)
        }
        None => base,
    };

    // ── The load-bearing cross-battler borrow: drain host + heal opponent
    //    through ONE paired &mut (cross-side → the raw-pointer branch). ──
    let (host_mon, opp_mon) = ctx.pair_mut(host, opponent);
    let actual_drain = drain.min(host_mon.hp); // legacy caps the heal at hp drained
    host_mon.take_damage(drain);
    // Legacy `residual.rs:65-67`: if the drain itself KO'd the host, return
    // `Fainted` BEFORE healing — the opponent is NOT healed on a leech KO.
    if host_mon.hp == 0 {
        return HandlerResult::Unchanged;
    }
    // Heal the opponent by the drained amount (engine `heal` clamps to max_hp,
    // matching legacy `(hp + actual_drain).min(max_hp)`).
    opp_mon.heal(actual_drain);

    HandlerResult::Unchanged
}

// ════════════════════════════════════════════════════════════════════════════
// SLICE 6 — multi-turn lock-in (Thrash/Fly/Hyper-Beam-recharge) + Counter + Bide.
//
// Lock-in volatiles live in the arena (cross-turn). The engine's `forced_action`
// seam (defaulted, generic) consults `PocData::forced_action` each turn so a
// volatile recorded on a PRIOR turn overrides the chosen action. The volatile
// lifecycle (decrement the lock, flip charge→strike, clear recharge, advance
// Bide) is driven by `Residual` handlers via `effect_for_volatile` (the slice-5
// arena-residual pass) — fired per-mover AFTER the move, every turn.
// ════════════════════════════════════════════════════════════════════════════

/// The Bide move's effect (slice 6). While storing, Bide does NOT attack on its
/// move turn — the accumulate/unleash all happens in the Bide volatile's Residual
/// handler. So this effect registers only the BeforeMove gate (a paralyzed Bide
/// user still loses the turn) and NO damage hooks (draws no crit/acc/dmg bytes).
static BIDE_MOVE_EFFECT: Effect<PocData> = Effect {
    id: EffectId(EFF_LOCKIN.0 + 100), // distinct move-effect id, never an arena entry
    kind: EffectType::Move,
    hooks: &[EventHook {
        event: Event::BeforeMove,
        call: paralysis_gate,
        order: 90,
        priority: 0,
        sub_order: None,
    }],
};

/// `Residual` lifecycle handler for the **LockedMove** (Thrash) volatile. Re-homes
/// `apply_thrash`'s decrement + end-confusion (`multi_turn_effects.rs:73-93`): each
/// turn (fired after the forced move) decrement `turns_left`; when it reaches 0,
/// self-confuse (Gen-1 fatigue, bug #17) and remove the lock. Draws NO rng (the
/// lock duration was rolled when the lock was created — here pre-seeded by the
/// scenario, matching how slices 4/5 pre-seed trap/sub state).
fn lockedmove_residual(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let host = target; // residual host == actor
    let idx = ctx
        .effects
        .iter()
        .position(|e| e.host == host && matches!(e.kind, PocKind::LockedMove { .. }));
    let Some(idx) = idx else {
        return HandlerResult::Unchanged;
    };
    let (now_zero, confuse) = {
        let PocKind::LockedMove { turns_left, confuse_on_end, .. } = &mut ctx.effects[idx].kind
        else {
            return HandlerResult::Unchanged;
        };
        if *turns_left > 0 {
            *turns_left -= 1;
        }
        (*turns_left == 0, *confuse_on_end)
    };
    if now_zero {
        ctx.effects.remove(idx);
        if confuse {
            // Gen-1 Thrash fatigue: self-confuse. Add a Confusion volatile on the
            // host (≙ legacy `status1::CONFUSED` + `confused_turns_left`). The
            // duration is deterministic here (pre-rolled / fixed) — no rng draw,
            // matching the residual-no-draw contract.
            ctx.effects.push(EffectState {
                id: EffectId(EFF_CONFUSION.0 + if host.side == 0 { 0 } else { 1 }),
                host,
                effect_order: 900,
                kind: PocKind::Confusion { turns_left: 2 },
            });
        }
    }
    HandlerResult::Unchanged
}

/// `Residual` lifecycle handler for the **TwoTurn** (Fly/Solar Beam) volatile.
/// Re-homes `apply_fly`/`apply_charge` (`multi_turn_effects.rs:7-29`): on the
/// CHARGE turn (`charging == true`, the move was forced to `Nothing`) flip to the
/// strike turn (`charging = false`, clear invulnerability); on the STRIKE turn
/// (`charging == false`, the move struck this turn) remove the volatile. Draws NO
/// rng.
fn twoturn_residual(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let host = target;
    let idx = ctx
        .effects
        .iter()
        .position(|e| e.host == host && matches!(e.kind, PocKind::TwoTurn { .. }));
    let Some(idx) = idx else {
        return HandlerResult::Unchanged;
    };
    let charging = match &mut ctx.effects[idx].kind {
        PocKind::TwoTurn { charging, invulnerable, .. } => {
            let was = *charging;
            if was {
                *charging = false; // charge → strike next turn
                *invulnerable = false; // semi-invuln only during the charge turn
            }
            was
        }
        _ => return HandlerResult::Unchanged,
    };
    if !charging {
        // We were on the strike turn this turn → the two-turn move is complete.
        ctx.effects.remove(idx);
    }
    HandlerResult::Unchanged
}

/// `Residual` lifecycle handler for the **Recharge** (Hyper Beam) volatile.
/// Re-homes `apply_hyper_beam`'s recharge skip (`multi_turn_effects.rs:101-109`):
/// the recharge volatile forced this turn's action to `Nothing` (the skip); now
/// remove it so the mon can act again next turn. Draws NO rng.
fn recharge_residual(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let host = target;
    if let Some(idx) = ctx
        .effects
        .iter()
        .position(|e| e.host == host && matches!(e.kind, PocKind::Recharge))
    {
        ctx.effects.remove(idx);
    }
    HandlerResult::Unchanged
}

/// `Residual` lifecycle handler for the **Bide** volatile. Re-homes `apply_bide`
/// (`multi_turn_effects.rs:52-71`): each storing turn, fold the damage the host
/// took this turn (its per-turn `DamageTaken` scratch) into `accumulated` and
/// decrement `turns_left`; on the unleash turn (`turns_left` reaches 0) deal
/// `accumulated * 2` (NOT ×3, bug #18) to the OPPONENT via `pair_mut`, then remove
/// the volatile. Draws NO rng. The damage-taken read is the §9 cross-action proof:
/// Bide accumulates damage recorded when the OPPONENT moved (a different mover's
/// `MoveContext`), so it must read the per-battler arena scratch, not `mv`.
fn bide_residual(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let host = target;
    let idx = ctx
        .effects
        .iter()
        .position(|e| e.host == host && matches!(e.kind, PocKind::Bide { .. }));
    let Some(idx) = idx else {
        return HandlerResult::Unchanged;
    };
    // Damage the host took this turn (from the opponent's move): the per-battler
    // `DamageTaken` scratch (the §9 cross-action home).
    let taken = damage_taken_this_turn(ctx, host);
    let (unleash, total) = {
        let PocKind::Bide { accumulated, turns_left } = &mut ctx.effects[idx].kind else {
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
        let release = total.saturating_mul(2); // bug #18: ×2, not ×3
        let opponent = BattlerRef::new(if host.side == 0 { 1 } else { 0 }, host.slot);
        // pair_mut: read host (alive guard) while writing the opponent's hp.
        let (host_mon, opp_mon) = ctx.pair_mut(host, opponent);
        if host_mon.hp > 0 {
            opp_mon.take_damage(release);
        }
    }
    HandlerResult::Unchanged
}

/// `DamagingHit` recorder (slice 6). After the driver applied a move's damage,
/// stamp the DEFENDER's per-turn `DamageTaken` scratch with the amount + whether
/// the move was PHYSICAL (Counter only reflects physical, bug #20). Replaces any
/// existing entry for the defender this turn (Gen-1 Counter/Bide read the LAST
/// damage taken). Draws NO rng; invisible to slices 1-5 (they never read it).
fn record_damage_taken(
    ctx: &mut BattleCtx<'_, PocData>,
    relay: RelayVar,
    target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    let amount = match relay {
        RelayVar::Damage(d) => d,
        _ => ctx.mv.last_damage,
    };
    if amount == 0 {
        return HandlerResult::Unchanged; // no damage → nothing to record
    }
    let physical = super::damage::is_physical(poc_move_data().move_type);
    // Remove any prior entry for this defender this turn (keep only the last).
    if let Some(idx) = ctx
        .effects
        .iter()
        .position(|e| e.host == target && matches!(e.kind, PocKind::DamageTaken { .. }))
    {
        ctx.effects[idx].kind = PocKind::DamageTaken { amount, physical };
    } else {
        ctx.effects.push(EffectState {
            id: EffectId(EFF_DAMAGE_TAKEN.0 + if target.side == 0 { 0 } else { 1 }),
            host: target,
            effect_order: 700,
            kind: PocKind::DamageTaken { amount, physical },
        });
    }
    HandlerResult::Unchanged
}

// ════════════════════════════════════════════════════════════════════════════
// SLICE 7 — representative Gen-1 SECONDARY / SPECIAL effects as `DamagingHit`
// (the design §1.1 `AfterMoveSecondary`) handlers, one per category, re-homing
// the legacy `apply_move_effect` oracle fn for each shape.
//
// ## The post-damage event-chain (no engine change)
//
// The engine has NO `AfterMoveSecondary` event; the existing `DamagingHit` IS
// that seam (driver.rs:181 — fired after `take_damage` set `mv.last_damage`).
// `secondary_handler` rides it for the DAMAGING-move effect, dispatching on the
// active move's `MoveEffect` to the right re-homed legacy handler. Haze (a power-0
// GLOBAL move) routes to its own `HAZE_EFFECT` (no crit/damage hooks) so its
// draw order matches the legacy power-0 branch (accuracy only).
//
// ## Determinism — the side-effect roll draw order / count (the slice's crux)
//
// In the legacy oracle `effect_randoms.side_effect_roll` is a STRUCT FIELD, read
// unconditionally by every side-effect handler AFTER damage (`apply_poison_side`
// etc. compare `roll < threshold`). The stack mirrors that by drawing ONE byte
// at the `DamagingHit` site — AFTER crit/acc/dmg, matching the `MoveRandoms`
// field order (`effect_randoms` last). The byte is drawn iff the move HIT (the
// DamagingHit fired) AND the move HAS a side-effect that reads the roll — exactly
// the legacy "apply_move_effect called only on a hit, side-effect handlers read
// the roll". A secondary that does NOT fire (roll >= threshold) STILL drew the
// byte (legacy still read the field) → `consumed()` parity holds either way. A
// `NoAdditionalEffect` move (every slice 1-6 move) draws NO byte (the handler
// returns immediately) → slices 1-6 stay byte-identical.

/// `DamagingHit` SECONDARY dispatcher (slice 7). Routes the active move's
/// `MoveEffect` to the re-homed legacy handler, drawing the `side_effect_roll`
/// byte for side-effect shapes (status/stat/flinch) and reading `mv.last_damage`
/// for the damage-variant shapes (recoil/drain). Inert (no byte, no effect) for
/// `NoAdditionalEffect`.
fn secondary_handler(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    use pokered_data::moves::MoveEffect as ME;
    match poc_move_data().effect {
        // ── status-on-hit side-effect (`apply_poison_side`, threshold 52/103) ──
        ME::PoisonSideEffect1 => poison_side(ctx, target, 52),
        ME::PoisonSideEffect2 => poison_side(ctx, target, 103),
        // ── stat-drop-on-hit (`apply_stat_down_side`, Special, 33% = thr 85) ──
        ME::SpecialDownSideEffect => stat_down_side(ctx, target, Stat::Special, 85),
        // ── flinch-on-hit (`apply_flinch_side`, threshold 26/77) ──
        ME::FlinchSideEffect1 => flinch_side(ctx, target, 26),
        ME::FlinchSideEffect2 => flinch_side(ctx, target, 77),
        // ── recoil (`apply_recoil`, damage/4 of damage DEALT; reads last_damage) ──
        ME::RecoilEffect => recoil(ctx, source),
        // ── drain (`apply_drain`, heal attacker damage/2; cross-battler) ──
        ME::DrainHpEffect => drain(ctx, source),
        // No side-effect → draw nothing, do nothing (slices 1-6 stay identical).
        _ => HandlerResult::Unchanged,
    }
}

/// Status-on-hit side-effect — re-homes `status_effects::apply_poison_side`
/// (`status_effects.rs:45-69`). Draws ONE `side_effect_roll` byte (legacy reads
/// the struct field), inflicts Poison iff `roll < threshold` AND the target has
/// NO status AND is not Poison-type. The byte is drawn UNCONDITIONALLY here
/// (mirroring the always-read legacy field) so `consumed()` is identical whether
/// or not the poison lands — the boundary pin (threshold-1 fires / threshold not).
fn poison_side(ctx: &mut BattleCtx<'_, PocData>, target: BattlerRef, threshold: u8) -> HandlerResult {
    let roll = ctx.rng.next_u8(); // side_effect_roll, drawn after damage
    if roll >= threshold {
        return HandlerResult::Unchanged; // chance failed (byte still consumed)
    }
    // Already-statused → fail (legacy `status.is_none()` guard).
    if ctx.battler(target).status.is_some() {
        return HandlerResult::Unchanged;
    }
    // Poison-type immunity (legacy type1/type2 == Poison guard).
    let (t1, t2) = species_types(ctx.battler(target).species);
    if t1 == PokemonType::Poison || t2 == PokemonType::Poison {
        return HandlerResult::Unchanged;
    }
    ctx.battler_mut(target).status = Some(LegacyStatus::Poison);
    HandlerResult::Unchanged
}

/// Stat-drop-on-hit — re-homes `stat_effects::apply_stat_down_side` +
/// `apply_stat_down` (`stat_effects.rs:31-63`, 33% = threshold 85). Draws ONE
/// `side_effect_roll` byte; on `roll < threshold` drops the target's stat stage by
/// 1 (clamped at -6, the legacy `stat_stages.modify` floor). The byte is drawn
/// unconditionally (legacy always reads the field), so `consumed()` is identical
/// fire vs no-fire.
fn stat_down_side(
    ctx: &mut BattleCtx<'_, PocData>,
    target: BattlerRef,
    stat: Stat,
    threshold: u8,
) -> HandlerResult {
    let roll = ctx.rng.next_u8(); // side_effect_roll
    if roll >= threshold {
        return HandlerResult::Unchanged; // chance failed (byte consumed)
    }
    let cur = ctx.battler(target).stat_stages.get(stat).copied().unwrap_or(0);
    if cur > -6 {
        ctx.battler_mut(target).stat_stages.set(stat, cur - 1);
    }
    HandlerResult::Unchanged
}

/// Flinch-on-hit — re-homes `special_effects::apply_flinch_side`
/// (`special_effects.rs:7-22`, threshold 26/77). Draws ONE `side_effect_roll`
/// byte; on `roll < threshold` adds a `PocKind::Flinch` volatile on the DEFENDER
/// (≙ legacy `status1::FLINCHED`). The flinch is consumed by `flinch_gate` on the
/// defender's NEXT action — it only bites if the flincher moved FIRST. The byte is
/// drawn unconditionally → `consumed()` identical fire vs no-fire.
fn flinch_side(ctx: &mut BattleCtx<'_, PocData>, target: BattlerRef, threshold: u8) -> HandlerResult {
    let roll = ctx.rng.next_u8(); // side_effect_roll
    if roll >= threshold {
        return HandlerResult::Unchanged; // chance failed (byte consumed)
    }
    // Don't double-add (legacy sets the flag idempotently).
    let already = ctx
        .effects
        .iter()
        .any(|e| e.host == target && matches!(e.kind, PocKind::Flinch));
    if !already {
        ctx.effects.push(EffectState {
            id: EffectId(EFF_FLINCH.0 + if target.side == 0 { 0 } else { 1 }),
            host: target,
            effect_order: 800,
            kind: PocKind::Flinch,
        });
    }
    HandlerResult::Unchanged
}

/// Recoil — re-homes `damage_effects::apply_recoil` (`damage_effects.rs:28-40`).
/// Recoil = `(damage_dealt / 4).max(1)` for a normal recoil move (Take Down /
/// Double-Edge), reading the damage DEALT THIS MOVE via `mv.last_damage` (the
/// slice-6 same-action placement — VALIDATED here: recoil reads the SAME mover's
/// just-dealt damage). Draws NO rng. Subtracts from the ATTACKER's hp.
fn recoil(ctx: &mut BattleCtx<'_, PocData>, source: BattlerRef) -> HandlerResult {
    let dealt = ctx.mv.last_damage;
    if dealt == 0 {
        return HandlerResult::Unchanged;
    }
    let recoil = (dealt / 4).max(1); // non-Struggle recoil = damage/4
    ctx.battler_mut(source).take_damage(recoil);
    HandlerResult::Unchanged
}

/// Drain — re-homes `damage_effects::apply_drain` (`damage_effects.rs:6-13`).
/// Heals the ATTACKER by `(damage_dealt / 2).max(1)` (Absorb/Mega Drain), reading
/// the damage DEALT THIS MOVE via `mv.last_damage` (slice-6 same-action read,
/// VALIDATED). Cross-battler in spirit (drained FROM the target, healed TO the
/// attacker); the heal target is the attacker, capped at its max_hp by
/// `BattlerState::heal`. Draws NO rng.
fn drain(ctx: &mut BattleCtx<'_, PocData>, source: BattlerRef) -> HandlerResult {
    let dealt = ctx.mv.last_damage;
    if dealt == 0 {
        return HandlerResult::Unchanged;
    }
    let drain = (dealt / 2).max(1);
    ctx.battler_mut(source).heal(drain);
    HandlerResult::Unchanged
}

/// `BeforeMove` flinch gate (slice 7, ASM position 4, `order:40`). Re-homes the
/// Gen-1 `FLINCHED` check: if the mover hosts a `PocKind::Flinch` volatile it
/// loses this turn and the flinch clears (consume-on-check). Draws NO rng (the
/// flinch is a flag, not a roll) → `consumed()` unaffected. Returns `Fail` when
/// flinched, else `Unchanged`.
fn flinch_gate(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    _target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    if let Some(idx) = ctx
        .effects
        .iter()
        .position(|e| e.host == source && matches!(e.kind, PocKind::Flinch))
    {
        ctx.effects.remove(idx); // consume the flinch (one-turn flag)
        HandlerResult::Fail // flinched → move aborted
    } else {
        HandlerResult::Unchanged
    }
}

/// **Haze** (`DamagingHit`, slice 7) — re-homes `field_effects::apply_haze`
/// (`field_effects.rs:66-89`): reset BOTH sides' stat stages to 0, clear all
/// non-volatile status, clear confusion / seeded / toxic / focus-energy volatiles.
/// A power-0 GLOBAL effect — it routes via `HAZE_EFFECT` (no crit/damage hooks),
/// firing only after the accuracy gate passes (so a Haze MISS does nothing, like
/// the legacy power-0 miss). Draws NO rng. The single representative "global
/// effect" shape.
fn haze_handler(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    _target: BattlerRef,
    _source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    // Reset both sides' stat stages (Attack/Defense/Special/Accuracy/Evasion).
    for who in [BattlerRef::PLAYER, BattlerRef::OPPONENT] {
        let b = ctx.battler_mut(who);
        b.stat_stages.set(Stat::Attack, 0);
        b.stat_stages.set(Stat::Defense, 0);
        b.stat_stages.set(Stat::Special, 0);
        b.stat_stages.set(Stat::Accuracy, 0);
        b.stat_stages.set(Stat::Evasion, 0);
        // Haze cures all non-volatile status in Gen-1 (legacy `field_effects.rs:86`).
        b.status = None;
    }
    // Clear the volatiles Haze wipes: confusion, seeded, toxic, focus energy.
    ctx.effects.retain(|e| {
        !matches!(
            e.kind,
            PocKind::Confusion { .. }
                | PocKind::Seeded
                | PocKind::Toxic { .. }
                | PocKind::FocusEnergy
        )
    });
    HandlerResult::Unchanged
}

/// **Counter** (`ModifyDamage`, slice 6, bug #20). Reads the HOST's per-turn
/// `DamageTaken` scratch — the damage the Counter user took from the opponent's
/// PHYSICAL move EARLIER this turn — and reflects `amount * 2` onto the opponent.
///
/// ## The MANDATORY load-bearing `pair_mut`
///
/// Counter is the slice that makes `pair_mut` genuinely required: it must READ the
/// SOURCE's (Counter user's) own per-turn damage-taken state *and* its live hp,
/// while WRITING the TARGET's (opponent's) hp — "mutate target while reading
/// source" (design §3.2). The reflect is applied THROUGH the paired `&mut` (not
/// via the driver's `ctx.mv.damage` auto-apply), so `ctx.mv.damage` is zeroed and
/// the handler returns `Set(Bool(false))` to stop the chain — the driver's
/// unconditional `take_damage(target, mv.damage)` is then a no-op. Draws NO rng.
///
/// Counter fails (deals 0) when the host took no PHYSICAL damage this turn
/// (special move, status move, or no damage) — the Gen-1 behaviour.
fn counter_handler(
    ctx: &mut BattleCtx<'_, PocData>,
    _relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    _eff: EffectId,
) -> HandlerResult {
    // The Counter user is the `source` (the mover); the opponent is `target`.
    let opponent = target;

    let (amount, physical) = match ctx
        .effects
        .iter()
        .find(|e| e.host == source && matches!(e.kind, PocKind::DamageTaken { .. }))
        .map(|e| &e.kind)
    {
        Some(PocKind::DamageTaken { amount, physical }) => (*amount, *physical),
        _ => (0, false),
    };

    if amount == 0 || !physical {
        // Counter fails: no physical damage taken this turn → deal 0 (bug #20).
        ctx.mv.damage = 0;
        return HandlerResult::Set(RelayVar::Bool(false));
    }

    let reflected = amount.saturating_mul(2);

    // ── The MANDATORY load-bearing pair_mut: read SOURCE (Counter user) host
    //    while WRITING the TARGET (opponent). Cross-side → the raw-pointer branch.
    let (counter_user, opp_mon) = ctx.pair_mut(source, opponent);
    // Live-attacker guard: a fainted Counter user reflects nothing (read source).
    if counter_user.hp == 0 {
        ctx.mv.damage = 0;
        return HandlerResult::Set(RelayVar::Bool(false));
    }
    opp_mon.take_damage(reflected); // WRITE target through the SAME paired &mut

    // Damage applied directly through pair_mut → zero mv.damage so the driver's
    // own `take_damage(target, mv.damage)` does not double-apply.
    ctx.mv.damage = 0;
    HandlerResult::Set(RelayVar::Bool(false)) // STOP the ModifyDamage chain
}

/// Read the host's per-turn `DamageTaken.amount` (0 if none). Shared by Bide.
fn damage_taken_this_turn(ctx: &BattleCtx<'_, PocData>, who: BattlerRef) -> u16 {
    ctx.effects
        .iter()
        .find_map(|e| match (&e.kind, e.host == who) {
            (PocKind::DamageTaken { amount, .. }, true) => Some(*amount),
            _ => None,
        })
        .unwrap_or(0)
}

/// **Haze** move effect (slice 7): a power-0 GLOBAL reset. Registers ONLY the
/// BeforeMove paralysis gate + accuracy + the global `haze_handler` on
/// `DamagingHit` — NO crit, NO damage hooks — so its draw order is `[para?] acc`,
/// matching the legacy power-0 branch (accuracy roll only, no crit/damage).
static HAZE_EFFECT: Effect<PocData> = Effect {
    id: EFF_HAZE,
    kind: EffectType::Move,
    hooks: &[
        EventHook {
            event: Event::BeforeMove,
            call: paralysis_gate,
            order: 90,
            priority: 0,
            sub_order: None,
        },
        EventHook {
            event: Event::Accuracy,
            call: accuracy_handler,
            order: u32::MAX,
            priority: 0,
            sub_order: None,
        },
        EventHook {
            event: Event::DamagingHit,
            call: haze_handler,
            order: 100,
            priority: 0,
            sub_order: None,
        },
    ],
};

/// The LockedMove (Thrash) volatile's residual effect (lifecycle: decrement +
/// end-confuse). `order` is irrelevant (one handler per volatile per event).
static LOCKEDMOVE_EFFECT: Effect<PocData> = Effect {
    id: EFF_LOCKIN,
    kind: EffectType::Condition,
    hooks: &[EventHook {
        event: Event::Residual,
        call: lockedmove_residual,
        order: 50,
        priority: 0,
        sub_order: None,
    }],
};

/// The TwoTurn (Fly/Solar Beam) volatile's residual effect (charge→strike→clear).
static TWOTURN_EFFECT: Effect<PocData> = Effect {
    id: EFF_LOCKIN,
    kind: EffectType::Condition,
    hooks: &[EventHook {
        event: Event::Residual,
        call: twoturn_residual,
        order: 50,
        priority: 0,
        sub_order: None,
    }],
};

/// The Recharge (Hyper Beam) volatile's residual effect (clear after the skip).
static RECHARGE_EFFECT: Effect<PocData> = Effect {
    id: EFF_LOCKIN,
    kind: EffectType::Condition,
    hooks: &[EventHook {
        event: Event::Residual,
        call: recharge_residual,
        order: 50,
        priority: 0,
        sub_order: None,
    }],
};

/// The Bide volatile's residual effect (accumulate / unleash ×2). `order:60` >
/// the status-damage `10` so any same-turn status tick lands first; Bide draws no
/// rng either way.
static BIDE_EFFECT: Effect<PocData> = Effect {
    id: EFF_LOCKIN,
    kind: EffectType::Condition,
    hooks: &[EventHook {
        event: Event::Residual,
        call: bide_residual,
        order: 60,
        priority: 0,
        sub_order: None,
    }],
};

// ─── helpers shared by provider + handlers ───────────────────────────────

fn battler<'a>(state: &'a EngineState<PocData>, who: BattlerRef) -> &'a EngineBattler<PocData> {
    if who.side == 0 {
        &state.player_battlers[who.slot as usize]
    } else {
        &state.opponent_battlers[who.slot as usize]
    }
}

fn move_priority(move_id: MoveId) -> i8 {
    match move_id {
        MoveId::QuickAttack => 1,
        MoveId::Counter => -1,
        _ => 0,
    }
}

/// Re-homes `turn_order::effective_speed` (paralysis ÷4, min 1).
fn effective_speed(b: &EngineBattler<PocData>) -> u16 {
    let base = b.stats.get(Stat::Speed).copied().unwrap_or(0);
    if b.status == Some(LegacyStatus::Paralysis) {
        (base / 4).max(1)
    } else {
        base
    }
}

fn species_types(s: Species) -> (PokemonType, PokemonType) {
    get_base_stats(s)
        .map(|bs| (bs.type1, bs.type2))
        .unwrap_or((PokemonType::Normal, PokemonType::Normal))
}

/// The single move used by the slice, shared by BOTH the legacy oracle and the
/// stack handlers so their accuracy/power/type/effect are identical (parity by
/// construction). A `NoAdditionalEffect` Electric move (power 40, acc 100) — the
/// secondary-effect machinery is out of slice 1's scope.
pub fn poc_move_data() -> MoveData {
    active_move()
}

// ─── Slice 3: the per-scenario active move + stages (read by the handlers) ──
//
// The upgraded crit/accuracy/damage handlers (slice 3) need per-scenario move
// data (power/type/accuracy/high-crit move id) and the attacker accuracy / target
// evasion stages — none of which the zero-capture `fn`-pointer handlers can carry
// themselves. Rather than change the engine, the harness stashes the active move
// in a **test-only thread-local** that `stack_run`/`stack_run_dmg` sets before
// driving the turn and the handlers read. Thread-locals are the *correct*
// isolation here: tests run in parallel on separate threads, so each thread sees
// its own active move (no cross-test contamination). The default (when unset) is
// the slice-1/2 Thundershock spec with zero stages, so slices 1 and 2 behave
// EXACTLY as before (`poc_move_data` returns it; stage reads are 0).
//
// The accuracy/defense stat STAGES ride on the engine `BattlerState.stat_stages`
// `EnumMap` (slice-3 sets them in `engine_battler_dmg`); the legacy path sets the
// same stages on its `BattlerState.stat_stages`. So both `calculate_damage` and
// `accuracy_check` see identical inputs — parity by construction.

thread_local! {
    static ACTIVE_MOVE: std::cell::Cell<MoveData> = const { std::cell::Cell::new(SLICE12_MOVE) };
}

/// The slice-1/2 default move (Electric, power 40, acc 100).
const SLICE12_MOVE: MoveData = MoveData {
    id: MoveId::Thundershock,
    effect: MoveEffect::NoAdditionalEffect,
    power: 40,
    move_type: PokemonType::Electric,
    accuracy: 100,
    pp: 30,
};

/// Read the per-scenario active move (defaults to the slice-1/2 Thundershock).
fn active_move() -> MoveData {
    ACTIVE_MOVE.with(|m| m.get())
}

/// Set the per-scenario active move for the current thread (slice 3). The
/// harness sets this before each stack run and resets it after.
fn set_active_move(m: MoveData) {
    ACTIVE_MOVE.with(|c| c.set(m));
}

/// RAII guard that restores the previous active move on drop — keeps the
/// thread-local clean across scenarios on the same test thread.
struct ActiveMoveGuard(MoveData);
impl Drop for ActiveMoveGuard {
    fn drop(&mut self) {
        set_active_move(self.0);
    }
}
fn with_active_move(m: MoveData) -> ActiveMoveGuard {
    let prev = active_move();
    set_active_move(m);
    ActiveMoveGuard(prev)
}

// ─── The §4.1 RNG-shim parity machinery (the reusable harness) ───────────
//
//                  ┌──────────────────────────────────────────┐
//   byte vector ──►│  b0 (order)  b1 b2 b3 …                   │
//                  └───────┬───────────────────┬──────────────┘
//                          │                   │
//           lay into TurnRandoms fields   feed ScriptedRng in stack
//           (pokered struct order)        FIRE order (order, then per
//                          │              mover: [para?] crit acc dmg)
//                  legacy execute_turn      StackDriver::execute_turn
//                          │                   │
//                   BattleState_A  ===diff=== BattleState_B  (identical)
//                                  AND stack rng.consumed() == expected N
//
// The shim proves the stack's fire order equals the struct field order for the
// slice under test, by construction. The honest oracle (design §4.1) is "same
// BattleState given the same bytes" + a per-scenario `consumed()` assertion on
// the stack (the legacy struct is not a stream, so consumed() is not compared
// to it). Crit-drawn-before-accuracy is guaranteed by the FIRE ORDER (§4) and
// pinned by the standing guard below.

/// A self-describing scenario shared by both code paths. Reusable: later slices
/// add scenarios as values of this type and call [`run_scenario`].
#[derive(Clone)]
pub struct Scenario {
    pub name: &'static str,
    pub player_speed: u16,
    pub enemy_speed: u16,
    pub player_hp: u16,
    pub enemy_hp: u16,
    pub player_status: LegacyStatus,
    pub enemy_status: LegacyStatus,
    pub player_focus_energy: bool,
    /// Confusion remaining-turn counters (slice 2). `0` = not confused. Confusion
    /// is a *volatile* (`status1` + `confused_turns_left` in legacy), so it rides
    /// separately from the non-volatile `*_status` field — exactly as the engine
    /// carries it (an `EffectState`, not the `BattlerState.status`).
    pub player_confused_turns: u8,
    pub enemy_confused_turns: u8,
    /// `order_random` (one byte, drawn first by both paths).
    pub order_byte: u8,
    /// Per-mover bytes in pokered FIELD order. Only the meaningful ones are
    /// drawn by the stack; the rest are harmless struct padding.
    pub first: MoveBytes,
    pub second: MoveBytes,
}

/// Per-mover RNG bytes in pokered's `MoveRandoms` FIELD order
/// (confusion → paralysis → crit → accuracy → damage). This is the canonical
/// byte layout the harness lays into both the legacy struct and the stack
/// stream — the single source of truth for draw order.
#[derive(Clone, Copy)]
pub struct MoveBytes {
    pub confusion: u8,
    pub paralysis: u8,
    pub crit: u8,
    pub accuracy: u8,
    pub damage: u8,
}

impl MoveBytes {
    /// confusion 255 (no self-hit), paralysis 255 (not full-para), crit 255 (no
    /// crit at low threshold), accuracy 0 (always hits), damage 255 (max roll).
    pub fn always_hit() -> Self {
        Self {
            confusion: 255,
            paralysis: 255,
            crit: 255,
            accuracy: 0,
            damage: 255,
        }
    }
}

/// Build a [`Pokemon`] for the legacy oracle path.
pub fn poke(species: Species, hp: u16, speed: u16, status: LegacyStatus) -> Pokemon {
    let base = get_base_stats(species).expect("species base stats");
    Pokemon {
        species,
        nickname: [0x50; 11],
        level: 50,
        hp,
        max_hp: hp,
        attack: 100,
        defense: 80,
        speed,
        special: 80,
        type1: base.type1,
        type2: base.type2,
        moves: [MoveId::Thundershock, MoveId::None, MoveId::None, MoveId::None],
        pp: [30, 0, 0, 0],
        pp_ups: [0; 4],
        status,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

// ── Legacy path: build pokered-core state + TurnRandoms, run execute_turn ──

/// Run the legacy oracle: lay the scenario bytes into a [`TurnRandoms`] struct
/// (pokered field order) and call [`execute_turn`].
pub fn legacy_run(s: &Scenario) -> LegacyState {
    let species = Species::Pikachu;
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![poke(species, s.player_hp, s.player_speed, s.player_status)],
        vec![poke(species, s.enemy_hp, s.enemy_speed, s.enemy_status)],
    );
    state.player.selected_move = MoveId::Thundershock;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Thundershock;
    state.enemy.selected_move_index = 0;
    if s.player_focus_energy {
        state.player.set_status2(status2::GETTING_PUMPED);
    }
    // Slice 2: confusion is a volatile (status1 CONFUSED + confused_turns_left in
    // the legacy state). Set it on whichever side the scenario marks confused.
    if s.player_confused_turns > 0 {
        state.player.set_status1(status1::CONFUSED);
        state.player.confused_turns_left = s.player_confused_turns;
    }
    if s.enemy_confused_turns > 0 {
        state.enemy.set_status1(status1::CONFUSED);
        state.enemy.confused_turns_left = s.enemy_confused_turns;
    }

    let move_data = poc_move_data();

    let randoms = TurnRandoms {
        order_random: s.order_byte,
        first_mover: to_move_randoms(s.first),
        second_mover: to_move_randoms(s.second),
    };
    execute_turn(&mut state, &move_data, &move_data, &randoms);
    state
}

fn to_move_randoms(b: MoveBytes) -> MoveRandoms {
    MoveRandoms {
        confusion_roll: b.confusion,
        paralysis_roll: b.paralysis,
        crit_roll: b.crit,
        accuracy_roll: b.accuracy,
        damage_roll: b.damage,
        effect_randoms: EffectRandoms {
            side_effect_roll: 255,
            duration_roll: 0,
            multi_hit_roll: 0,
            stat_down_miss_roll: 255,
        },
    }
}

// ── Stack path: build engine state + arena, stream bytes in FIRE order ──

/// Build an engine [`BattlerState`](EngineBattler) for the stack path.
pub fn engine_battler(
    species: Species,
    hp: u16,
    speed: u16,
    status: LegacyStatus,
) -> EngineBattler<PocData> {
    let mut stats = EnumMap::new();
    stats.set(Stat::Attack, 100);
    stats.set(Stat::Defense, 80);
    stats.set(Stat::Speed, speed);
    stats.set(Stat::Special, 80);
    stats.set(Stat::Level, 50);
    stats.set(Stat::MaxHp, hp);
    let mut b = EngineBattler::new(species, hp, hp, stats, vec![MoveId::Thundershock]);
    if status != LegacyStatus::None {
        b.status = Some(status);
    }
    b
}

/// Build the byte stream in the EXACT order the StackDriver draws it, and the
/// expected `consumed()` count. Returns `(bytes, expected_consumed)`.
///
/// Draw order per turn (design §4, extended for slice 2's BeforeMove gate):
///   order_byte (only on an exact rank tie)
///   first  mover BeforeMove gate (ASM order):
///       sleep (no draw, may abort) → freeze (no draw, may abort)
///       → confusion (draw iff confused & not snapping out; may abort on self-hit)
///       → paralysis (draw iff paralyzed; may abort on full-para)
///   first  mover (if it acts): crit → accuracy → [damage if hit]
///   second mover: same (only if not cancelled)
///
/// The confusion/paralysis bytes draw ONLY when their gate meaningfully reads —
/// confusion BEFORE paralysis (`order:70` < `order:90`), matching the ASM and
/// `MoveRandoms` field order (`confusion_roll` then `paralysis_roll`). Sleep and
/// freeze draw nothing. The shim mirrors the StackDriver's control flow to
/// predict the count; this predictor is the heart of the draw-order parity
/// contract every later slice extends.
pub fn build_stack_stream(s: &Scenario, first: FirstMover, tie: bool) -> (Vec<u8>, usize) {
    // The order byte is drawn by the stack ONLY on an exact rank tie.
    let mut bytes = if tie { vec![s.order_byte] } else { Vec::new() };
    // map mover → (status, confused_turns, MoveBytes)
    let (first_status, first_conf, first_bytes, second_status, second_conf, second_bytes) =
        match first {
            FirstMover::Player => (
                s.player_status,
                s.player_confused_turns,
                s.first,
                s.enemy_status,
                s.enemy_confused_turns,
                s.second,
            ),
            FirstMover::Opponent => (
                s.enemy_status,
                s.enemy_confused_turns,
                s.first,
                s.player_status,
                s.player_confused_turns,
                s.second,
            ),
        };

    let push_mover =
        |bytes: &mut Vec<u8>, status: LegacyStatus, confused_turns: u8, mb: MoveBytes| -> bool {
            // ── BeforeMove gate, in ASM / handler-`order` sequence ──
            // 1. Sleep (order 10): no draw; if asleep, aborts the move.
            if let LegacyStatus::Sleep(_) = status {
                return true; // asleep (or waking) → aborts; no byte, no faint
            }
            // 2. Freeze (order 20): no draw; always aborts.
            if status == LegacyStatus::Freeze {
                return true;
            }
            // 3. Confusion (order 70): draws ONE byte iff confused AND not
            //    snapping out this turn (turns_left decrements to >0). The legacy
            //    gate decrements first, then snaps out at 0 WITHOUT a draw.
            if confused_turns > 0 {
                let turns_after = confused_turns - 1;
                if turns_after > 0 {
                    bytes.push(mb.confusion);
                    if mb.confusion < 128 {
                        return true; // confusion self-hit → aborts; no further draws
                    }
                }
                // turns_after == 0 → snap out, no byte, fall through.
            }
            // 4. Paralysis (order 90): draws iff paralyzed.
            if status == LegacyStatus::Paralysis {
                bytes.push(mb.paralysis);
                if mb.paralysis < 63 {
                    return true; // fully paralyzed → aborts; no further draws
                }
            }
            // ── move acts: ModifyCritRatio always draws crit. ──
            bytes.push(mb.crit);
            // Accuracy: always draws.
            bytes.push(mb.accuracy);
            // ModifyDamage: draws iff accuracy hit. Scaled = move_acc*255/100
            // clamped (the slice move is 100% → 255; only byte 255 misses = the
            // Gen-1 1/256 bug).
            let scaled = (poc_move_data().accuracy as u32 * 255 / 100).min(255) as u8;
            if mb.accuracy < scaled {
                bytes.push(mb.damage);
            }
            false
        };

    let _first_aborted = push_mover(&mut bytes, first_status, first_conf, first_bytes);

    // Whether the second mover acts depends on faint short-circuit, which
    // depends on resulting HP — we can't predict damage cheaply here, so the
    // predictor assumes no mid-turn KO. The matrix scenarios pick HP so the
    // second mover always acts (no KO); the dedicated KO/faint scenario asserts
    // `consumed()` directly against the stack run instead.
    let second_acts = s.enemy_hp > 30 && s.player_hp > 30; // generous: no KO
    if second_acts {
        push_mover(&mut bytes, second_status, second_conf, second_bytes);
    }
    let consumed = bytes.len();
    (bytes, consumed)
}

/// Run the stack path: build engine state + the Focus-Energy arena, stream the
/// bytes in FIRE order, and return `(state, consumed, first_mover)`.
pub fn stack_run(s: &Scenario) -> (EngineState<PocData>, usize, FirstMover) {
    let species = Species::Pikachu;
    let mut state = EngineState::new(
        vec![engine_battler(species, s.player_hp, s.player_speed, s.player_status)],
        vec![engine_battler(species, s.enemy_hp, s.enemy_speed, s.enemy_status)],
    );
    let mut effects: Vec<EffectState<PocData>> = Vec::new();
    if s.player_focus_energy {
        effects.push(EffectState {
            id: EFF_FOCUS_ENERGY,
            host: BattlerRef::PLAYER,
            effect_order: 0,
            kind: PocKind::FocusEnergy,
        });
    }
    // Slice 2: confusion volatiles. Distinct ids per host keep the arena's
    // sorted-by-id invariant intact (the confusion handler addresses them by
    // `host` + `kind`, not by id, so the exact id is immaterial). Ids 110/111
    // sort after Focus Energy (100), so the arena stays ordered.
    if s.player_confused_turns > 0 {
        effects.push(EffectState {
            id: EFF_CONFUSION,
            host: BattlerRef::PLAYER,
            effect_order: 1,
            kind: PocKind::Confusion {
                turns_left: s.player_confused_turns,
            },
        });
    }
    if s.enemy_confused_turns > 0 {
        effects.push(EffectState {
            id: EffectId(EFF_CONFUSION.0 + 1),
            host: BattlerRef::OPPONENT,
            effect_order: 2,
            kind: PocKind::Confusion {
                turns_left: s.enemy_confused_turns,
            },
        });
    }

    let provider = PocData;
    let actions = [
        BattleAction::<PocData>::Fight { move_: MoveId::Thundershock },
        BattleAction::<PocData>::Fight { move_: MoveId::Thundershock },
    ];

    // Determine the first mover via the provider's RNG-free ranking; tie → one
    // coin-flip byte. This must agree with the driver's own choice.
    let first = first_mover(s);

    let tie = order_is_tie(s);
    let (bytes, _expected) = build_stack_stream(s, first, tie);
    let mut rng = ScriptedRng::new(bytes);
    let result = StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
    assert_eq!(result.first, first, "[{}] first-mover probe disagreed", s.name);
    (state, rng.consumed(), first)
}

/// The first mover for a scenario, via the provider's RNG-free ranking (tie
/// broken by `order_byte < 128`). Re-uses the same logic the driver does.
pub fn first_mover(s: &Scenario) -> FirstMover {
    let provider = PocData;
    let state = EngineState::new(
        vec![engine_battler(Species::Pikachu, s.player_hp, s.player_speed, s.player_status)],
        vec![engine_battler(Species::Pikachu, s.enemy_hp, s.enemy_speed, s.enemy_status)],
    );
    let pr = provider.turn_order_rank(&state, BattlerRef::PLAYER, &MoveId::Thundershock);
    let er = provider.turn_order_rank(&state, BattlerRef::OPPONENT, &MoveId::Thundershock);
    match pr.cmp(&er) {
        std::cmp::Ordering::Less => FirstMover::Player,
        std::cmp::Ordering::Greater => FirstMover::Opponent,
        std::cmp::Ordering::Equal => {
            if s.order_byte < 128 {
                FirstMover::Player
            } else {
                FirstMover::Opponent
            }
        }
    }
}

/// Whether the two movers have an exact turn-order rank tie (so the stack draws
/// the one order byte). Re-uses the provider's RNG-free ranking.
pub fn order_is_tie(s: &Scenario) -> bool {
    let provider = PocData;
    let state = EngineState::new(
        vec![engine_battler(Species::Pikachu, s.player_hp, s.player_speed, s.player_status)],
        vec![engine_battler(Species::Pikachu, s.enemy_hp, s.enemy_speed, s.enemy_status)],
    );
    let pr = provider.turn_order_rank(&state, BattlerRef::PLAYER, &MoveId::Thundershock);
    let er = provider.turn_order_rank(&state, BattlerRef::OPPONENT, &MoveId::Thundershock);
    pr == er
}

// ── The diff: compare hp/status of both sides after the turn. ──

/// The differential oracle (design §4.1): legacy `execute_turn` `BattleState` vs
/// `StackDriver` `BattleState` — hp + status, BOTH sides. The single comparison
/// every slice asserts.
pub fn assert_state_parity(s: &Scenario, legacy: &LegacyState, stack: &EngineState<PocData>) {
    let lp = legacy.player.active_mon();
    let le = legacy.enemy.active_mon();
    let sp = &stack.player_battlers[0];
    let se = &stack.opponent_battlers[0];
    assert_eq!(
        lp.hp, sp.hp,
        "[{}] PLAYER hp mismatch: legacy={} stack={}",
        s.name, lp.hp, sp.hp
    );
    assert_eq!(
        le.hp, se.hp,
        "[{}] ENEMY hp mismatch: legacy={} stack={}",
        s.name, le.hp, se.hp
    );
    // status (poison persists; the slice does not inflict new status).
    let l_pstatus = lp.status;
    let s_pstatus = sp.status.unwrap_or(LegacyStatus::None);
    assert_eq!(l_pstatus, s_pstatus, "[{}] PLAYER status mismatch", s.name);
    let l_estatus = le.status;
    let s_estatus = se.status.unwrap_or(LegacyStatus::None);
    assert_eq!(l_estatus, s_estatus, "[{}] ENEMY status mismatch", s.name);
}

/// The full per-scenario assertion: run both paths, assert `BattleState` parity
/// AND that the stack consumed exactly the predicted byte count (draw-order
/// drift detector). This is the one call later slices add scenarios against.
pub fn run_scenario(s: &Scenario) {
    let legacy = legacy_run(s);
    let (stack, consumed, first) = stack_run(s);
    assert_state_parity(s, &legacy, &stack);
    // consumed() parity: assert the stack drew exactly the predicted count.
    let (_b, expected) = build_stack_stream(s, first, order_is_tie(s));
    assert_eq!(
        consumed, expected,
        "[{}] stack consumed {} bytes, expected {} (draw-order drift!)",
        s.name, consumed, expected
    );
}

// ─── The STANDING DRAW-ORDER GUARD (crit BEFORE accuracy, §4) ────────────
//
// An audit agent previously broke the crit-before-accuracy invariant by
// swapping the ModifyCritRatio/Accuracy fire order in `driver.rs`. The whole
// point of slice 1 is to make that class of regression impossible to merge
// silently. This guard pins the invariant via a stream-position trap: the crit
// byte and the accuracy byte are at distinct stream offsets, and the crit byte
// only lands as a crit if it is the one drawn at the CRIT offset (before the
// accuracy byte is consumed). If `driver.rs` fired Accuracy before
// ModifyCritRatio, the stack would consume the bytes in the wrong slots and the
// crit would NOT land — producing less damage than the legacy oracle (which
// always reads crit before accuracy from its struct), and the assertion fails.

/// Assert the [`StackDriver`] draws crit BEFORE accuracy (design §4). Returns
/// the (legacy_hp, stack_hp, no_crit_legacy_hp) triple so callers can also see
/// the crit actually bit. Fails loudly if the fire order in `driver.rs` is
/// swapped — see the module comment.
pub fn assert_crit_drawn_before_accuracy() {
    // base_speed of Pikachu / 2 = threshold; pick a crit byte below it.
    let base_speed = get_base_stats(Species::Pikachu).unwrap().speed;
    let crit_threshold = super::damage::crit_chance(base_speed, false, false);
    assert!(crit_threshold > 1, "need a crit-able threshold for the proof");

    // The trap: crit byte (drawn 2nd, at the CRIT offset) is 0 → a guaranteed
    // crit IF drawn at that offset. The accuracy byte (drawn 3rd) is 255 → a
    // *miss* IF drawn at the ACCURACY offset (the 1/256 Gen-1 bug). So:
    //   correct order (crit, then accuracy): crit byte 0 → CRIT, accuracy byte
    //       255 → MISS. Net: a critical MISS (no damage). Legacy agrees.
    //   swapped order (accuracy, then crit): accuracy reads 0 → HIT, crit reads
    //       255 → NO crit. Net: a normal NON-crit HIT (damage dealt).
    // The two outcomes are unmistakably different state (0 vs >0 damage), and
    // the stack must match the legacy oracle (which reads crit-before-accuracy
    // from its struct, giving the critical MISS).
    let s = Scenario {
        name: "crit-before-accuracy guard",
        player_speed: 100,
        enemy_speed: 50,
        player_hp: 200,
        enemy_hp: 200,
        player_status: LegacyStatus::None,
        enemy_status: LegacyStatus::None,
        player_focus_energy: false,
        player_confused_turns: 0,
        enemy_confused_turns: 0,
        order_byte: 0,
        first: MoveBytes {
            confusion: 255,
            paralysis: 255,
            crit: 0,       // CRIT offset: 0 < threshold → crit (if drawn here)
            accuracy: 255, // ACCURACY offset: 255 → the 1/256 miss (if drawn here)
            damage: 255,
        },
        second: MoveBytes::always_hit(),
    };

    let legacy = legacy_run(&s);
    let (stack, _consumed, _first) = stack_run(&s);

    // 1. Strong invariant: the stack matches the legacy oracle exactly. With
    //    correct fire order this is a critical MISS → enemy at full HP both
    //    sides. With swapped order the stack would HIT (enemy < full) while the
    //    legacy oracle still MISSES → mismatch → this assertion fires.
    assert_eq!(
        legacy.enemy.active_mon().hp,
        stack.opponent_battlers[0].hp,
        "DRAW-ORDER GUARD: stack vs legacy enemy hp diverged — crit/accuracy \
         fire order in driver.rs is likely swapped (see driver.rs §2 step 2b)"
    );

    // 2. Explicit semantic check: correct order makes this a critical MISS, so
    //    the enemy must be untouched. If accuracy were drawn first (reading the
    //    0 byte as a HIT) the enemy would have taken damage and this fails.
    assert_eq!(
        stack.opponent_battlers[0].hp, s.enemy_hp,
        "DRAW-ORDER GUARD: enemy took damage — accuracy byte (255=miss) was NOT \
         consumed at the accuracy offset, so crit was drawn AFTER accuracy"
    );

    // 3. Cross-check the offsets are genuinely distinct by flipping ONLY the
    //    crit byte to a non-crit value: with correct order the move STILL misses
    //    (accuracy byte 255), proving the accuracy byte governs hit/miss at its
    //    own offset independent of the crit byte.
    let mut crit_high = s.clone();
    crit_high.first.crit = 255; // no crit
    let (stack_nc, _c2, _f2) = stack_run(&crit_high);
    assert_eq!(
        stack_nc.opponent_battlers[0].hp, s.enemy_hp,
        "DRAW-ORDER GUARD: changing only the crit byte changed hit/miss — the \
         accuracy draw is not at a stable offset (fire order suspect)"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// SLICE 3 — the Gen-1 crit → accuracy → damage pipeline at parity.
//
// Slice 1's damage path was a minimal stand-in (fixed power/type, no stages, no
// high-crit, fixed species). Slice 3 upgrades the three handlers (above) to the
// REAL Gen-1 formulas (delegating to / mirroring the legacy `crit_chance` /
// `is_high_crit_move` / `accuracy_check` / `calculate_damage`) and proves them at
// parity across a real matrix: per-side species (→ base-speed crit threshold,
// STAB, type chart), per-side stats (→ stat-scaling >255), per-side stat STAGES
// (atk/def/special for damage, accuracy/evasion for hit), and per-scenario move
// power/type/id/accuracy (→ high-crit, super-/not-very-effective, 1/256 miss).
//
// It is **additive**: a NEW richer scenario type ([`DamageScenario`]) with its own
// run functions ([`legacy_run_dmg`]/[`stack_run_dmg`]/[`run_scenario_dmg`]). The
// slice-1/2 [`Scenario`] and its runners are untouched, so those slices' literal
// scenario constructions still compile and behave identically (the upgraded
// handlers default — via the thread-local — to the slice-1/2 Thundershock with
// zero stages).
// ════════════════════════════════════════════════════════════════════════════

/// Per-side mon config for the slice-3 damage matrix. Carries the everything the
/// crit/accuracy/damage formulas read so both paths get IDENTICAL inputs: species
/// (→ base-speed crit threshold + types for STAB/effectiveness), the physical &
/// special attack/defense stats (→ stat-scaling >255), and the four stat stages
/// the pipeline uses (atk/def/special for damage, accuracy/evasion for hit).
#[derive(Clone, Copy)]
pub struct MonSpec {
    pub species: Species,
    pub hp: u16,
    pub speed: u16,
    pub attack: u16,
    pub defense: u16,
    pub special: u16,
    pub atk_stage: i8,
    pub def_stage: i8,
    pub spc_stage: i8,
    pub acc_stage: i8,
    pub eva_stage: i8,
    pub status: LegacyStatus,
    pub focus_energy: bool,
    /// Slice 4: this mon has a Substitute up with this many hp (`0` = none). Set
    /// on BOTH paths identically (legacy `substitute_hp`+`HAS_SUBSTITUTE_UP`; the
    /// stack `PocKind::Substitute` volatile), so the absorb/break is parity by
    /// construction.
    pub substitute_hp: u16,
    /// Slice 4: this mon is using a partial-trap move with this many turns left
    /// (`0` = none). Set on BOTH paths (legacy `USING_TRAPPING_MOVE`+
    /// `num_attacks_left`; the stack `PocKind::Trapping`). Its presence forfeits
    /// the OPPONENT's action this turn (the in-turn trapped gate).
    pub trapping_turns: u8,
}

impl MonSpec {
    /// A neutral Pikachu (atk 100, def 80, spc 80, no stages/status) — the
    /// slice-1/2 default stat block, so a `DamageScenario` left at defaults
    /// reproduces the slice-1/2 numbers.
    pub fn pikachu(hp: u16, speed: u16) -> Self {
        Self {
            species: Species::Pikachu,
            hp,
            speed,
            attack: 100,
            defense: 80,
            special: 80,
            atk_stage: 0,
            def_stage: 0,
            spc_stage: 0,
            acc_stage: 0,
            eva_stage: 0,
            status: LegacyStatus::None,
            focus_energy: false,
            substitute_hp: 0,
            trapping_turns: 0,
        }
    }
}

/// The slice-3 scenario: two [`MonSpec`]s, the move spec (same for both movers),
/// and the per-mover [`MoveBytes`] in `MoveRandoms` field order. Distinct from the
/// slice-1/2 [`Scenario`] so neither disturbs the other.
#[derive(Clone)]
pub struct DamageScenario {
    pub name: &'static str,
    pub player: MonSpec,
    pub enemy: MonSpec,
    /// The move both movers use (power/type/id/accuracy). High-crit is derived
    /// from `move.id` via `is_high_crit_move`, exactly like legacy.
    pub move_data: MoveData,
    pub order_byte: u8,
    pub first: MoveBytes,
    pub second: MoveBytes,
}

/// Build a legacy [`Pokemon`] from a [`MonSpec`] (sets the physical/special stats
/// and types from the species; stages/status are set on the `BattlerState`).
fn poke_spec(m: &MonSpec) -> Pokemon {
    let base = get_base_stats(m.species).expect("species base stats");
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
        moves: [MoveId::Thundershock, MoveId::None, MoveId::None, MoveId::None],
        pp: [30, 0, 0, 0],
        pp_ups: [0; 4],
        status: m.status,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

/// Run the legacy oracle for a [`DamageScenario`]: build pokered-core state with
/// the per-side stats + STAGES + focus energy, lay the bytes into `TurnRandoms`,
/// and call [`execute_turn`] with the scenario's move data on BOTH movers.
pub fn legacy_run_dmg(s: &DamageScenario) -> LegacyState {
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![poke_spec(&s.player)],
        vec![poke_spec(&s.enemy)],
    );
    state.player.selected_move = MoveId::Thundershock;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Thundershock;
    state.enemy.selected_move_index = 0;
    // Stat stages live on the BattlerState (the same place accuracy_check /
    // calc_and_apply_damage read them).
    set_legacy_stages(&mut state.player.stat_stages, &s.player);
    set_legacy_stages(&mut state.enemy.stat_stages, &s.enemy);
    if s.player.focus_energy {
        state.player.set_status2(status2::GETTING_PUMPED);
    }
    if s.enemy.focus_energy {
        state.enemy.set_status2(status2::GETTING_PUMPED);
    }
    // Slice 4: Substitute up (flag + hp) — the absorb/break target in
    // `move_execution.rs:288-300`.
    if s.player.substitute_hp > 0 {
        state.player.set_status2(status2::HAS_SUBSTITUTE_UP);
        state.player.substitute_hp = s.player.substitute_hp as u8;
    }
    if s.enemy.substitute_hp > 0 {
        state.enemy.set_status2(status2::HAS_SUBSTITUTE_UP);
        state.enemy.substitute_hp = s.enemy.substitute_hp as u8;
    }
    // Slice 4: partial-trap in flight (flag + turn counter) — the trapped gate in
    // `status_checks.rs:60-64` reads the OPPONENT's `USING_TRAPPING_MOVE`.
    if s.player.trapping_turns > 0 {
        state.player.set_status1(status1::USING_TRAPPING_MOVE);
        state.player.num_attacks_left = s.player.trapping_turns;
    }
    if s.enemy.trapping_turns > 0 {
        state.enemy.set_status1(status1::USING_TRAPPING_MOVE);
        state.enemy.num_attacks_left = s.enemy.trapping_turns;
    }

    let randoms = TurnRandoms {
        order_random: s.order_byte,
        first_mover: to_move_randoms(s.first),
        second_mover: to_move_randoms(s.second),
    };
    execute_turn(&mut state, &s.move_data, &s.move_data, &randoms);
    state
}

fn set_legacy_stages(stages: &mut super::stat_stages::StatStages, m: &MonSpec) {
    stages.attack = m.atk_stage;
    stages.defense = m.def_stage;
    stages.special = m.spc_stage;
    stages.accuracy = m.acc_stage;
    stages.evasion = m.eva_stage;
}

/// Build an engine battler from a [`MonSpec`] (stats + stat_stages incl.
/// Accuracy/Evasion — the keys the slice-3 handlers read).
pub fn engine_battler_dmg(m: &MonSpec) -> EngineBattler<PocData> {
    let mut stats = EnumMap::new();
    stats.set(Stat::Attack, m.attack);
    stats.set(Stat::Defense, m.defense);
    stats.set(Stat::Speed, m.speed);
    stats.set(Stat::Special, m.special);
    stats.set(Stat::Level, 50);
    stats.set(Stat::MaxHp, m.hp);
    let mut b = EngineBattler::new(m.species, m.hp, m.hp, stats, vec![MoveId::Thundershock]);
    if m.status != LegacyStatus::None {
        b.status = Some(m.status);
    }
    b.stat_stages.set(Stat::Attack, m.atk_stage);
    b.stat_stages.set(Stat::Defense, m.def_stage);
    b.stat_stages.set(Stat::Special, m.spc_stage);
    b.stat_stages.set(Stat::Accuracy, m.acc_stage);
    b.stat_stages.set(Stat::Evasion, m.eva_stage);
    b
}

/// First mover for a [`DamageScenario`] (RNG-free rank, tie → `order_byte<128`).
fn first_mover_dmg(s: &DamageScenario) -> FirstMover {
    let provider = PocData;
    let state = EngineState::new(
        vec![engine_battler_dmg(&s.player)],
        vec![engine_battler_dmg(&s.enemy)],
    );
    let pr = provider.turn_order_rank(&state, BattlerRef::PLAYER, &MoveId::Thundershock);
    let er = provider.turn_order_rank(&state, BattlerRef::OPPONENT, &MoveId::Thundershock);
    match pr.cmp(&er) {
        std::cmp::Ordering::Less => FirstMover::Player,
        std::cmp::Ordering::Greater => FirstMover::Opponent,
        std::cmp::Ordering::Equal => {
            if s.order_byte < 128 {
                FirstMover::Player
            } else {
                FirstMover::Opponent
            }
        }
    }
}

fn order_is_tie_dmg(s: &DamageScenario) -> bool {
    let provider = PocData;
    let state = EngineState::new(
        vec![engine_battler_dmg(&s.player)],
        vec![engine_battler_dmg(&s.enemy)],
    );
    let pr = provider.turn_order_rank(&state, BattlerRef::PLAYER, &MoveId::Thundershock);
    let er = provider.turn_order_rank(&state, BattlerRef::OPPONENT, &MoveId::Thundershock);
    pr == er
}

/// The slice-3 consumed-byte predictor. Same control flow as
/// [`build_stack_stream`] (status gate → crit → accuracy → damage), but the
/// accuracy threshold is the STAGE-AWARE [`scaled_accuracy`] for the scenario's
/// move + the mover's accuracy stage + the target's evasion stage (so the 1/256
/// miss and stage-driven misses are predicted exactly). The damage byte is drawn
/// whenever accuracy passes — including a type-immunity "miss" (the damage handler
/// draws the byte before returning the immunity short-circuit), matching legacy.
pub fn build_stack_stream_dmg(s: &DamageScenario, first: FirstMover, tie: bool) -> (Vec<u8>, usize) {
    let mut bytes = if tie { vec![s.order_byte] } else { Vec::new() };
    // mover → (attacker MonSpec, defender MonSpec, MoveBytes)
    let (fa, fd, fb, sa, sd, sb) = match first {
        FirstMover::Player => (&s.player, &s.enemy, s.first, &s.enemy, &s.player, s.second),
        FirstMover::Opponent => (&s.enemy, &s.player, s.first, &s.player, &s.enemy, s.second),
    };

    let push_mover = |bytes: &mut Vec<u8>, atk: &MonSpec, def: &MonSpec, mb: MoveBytes| {
        // Slice 4: partial-trap gate (ASM #3, `order:30`) fires BEFORE paralysis.
        // If the mover's OPPONENT (`def`) is using a trapping move, the mover
        // CANNOT act — and the trapped check draws NO byte (legacy reads none).
        if def.trapping_turns > 0 {
            return; // trapped by opponent → no draws at all
        }
        // BeforeMove gate (slice 3 matrix uses no sleep/freeze/confusion; only
        // paralysis can gate). Confusion is not exercised in slice 3.
        if atk.status == LegacyStatus::Paralysis {
            bytes.push(mb.paralysis);
            if mb.paralysis < 63 {
                return; // fully paralyzed → aborts, no further draws
            }
        }
        // crit always drawn; accuracy always drawn.
        bytes.push(mb.crit);
        bytes.push(mb.accuracy);
        // damage drawn iff accuracy passes (stage-aware threshold). The Substitute
        // interceptor draws NO byte (it redirects the ALREADY-drawn damage), so the
        // damage byte is still drawn here whether or not the defender has a sub.
        let scaled = scaled_accuracy(s.move_data.accuracy, atk.acc_stage, def.eva_stage);
        if mb.accuracy < scaled {
            bytes.push(mb.damage);
        }
    };

    push_mover(&mut bytes, fa, fd, fb);
    // The matrix keeps both HP high enough that no mid-turn KO cancels the second
    // move; the KO-specific scenario asserts consumed() directly instead.
    let second_acts = s.player.hp > 30 && s.enemy.hp > 30;
    if second_acts {
        push_mover(&mut bytes, sa, sd, sb);
    }
    let consumed = bytes.len();
    (bytes, consumed)
}

/// Run the stack path for a [`DamageScenario`]: set the active move (thread-local,
/// restored on drop), build the engine state with per-side stats + stages + focus
/// energy + the slice-4 sub/trap volatiles, stream the bytes in FIRE order, and
/// return `(state, consumed, first)`. Delegates to [`stack_run_sub`] (the single
/// builder for both slices) and drops the post-turn arena — NO plumbing duplicated.
pub fn stack_run_dmg(s: &DamageScenario) -> (EngineState<PocData>, usize, FirstMover) {
    let (state, consumed, first, _effects) = stack_run_sub(s);
    (state, consumed, first)
}

/// Differential oracle for a [`DamageScenario`]: legacy `execute_turn` vs
/// `StackDriver` — hp + status, BOTH sides.
pub fn assert_state_parity_dmg(s: &DamageScenario, legacy: &LegacyState, stack: &EngineState<PocData>) {
    let lp = legacy.player.active_mon();
    let le = legacy.enemy.active_mon();
    let sp = &stack.player_battlers[0];
    let se = &stack.opponent_battlers[0];
    assert_eq!(lp.hp, sp.hp, "[{}] PLAYER hp: legacy={} stack={}", s.name, lp.hp, sp.hp);
    assert_eq!(le.hp, se.hp, "[{}] ENEMY hp: legacy={} stack={}", s.name, le.hp, se.hp);
    assert_eq!(lp.status, sp.status.unwrap_or(LegacyStatus::None), "[{}] PLAYER status", s.name);
    assert_eq!(le.status, se.status.unwrap_or(LegacyStatus::None), "[{}] ENEMY status", s.name);
}

/// The full per-scenario slice-3 assertion: run BOTH paths, assert `BattleState`
/// parity AND that the stack consumed exactly the predicted byte count.
pub fn run_scenario_dmg(s: &DamageScenario) {
    let legacy = legacy_run_dmg(s);
    let (stack, consumed, first) = stack_run_dmg(s);
    assert_state_parity_dmg(s, &legacy, &stack);
    let (_b, expected) = build_stack_stream_dmg(s, first, order_is_tie_dmg(s));
    assert_eq!(
        consumed, expected,
        "[{}] stack consumed {} bytes, expected {} (draw-order drift!)",
        s.name, consumed, expected
    );
}

// ════════════════════════════════════════════════════════════════════════════
// SLICE 4 — Substitute + partial-trap volatile parity (the FIRST real
// cross-battler handler, exercising the engine's `pair_mut`).
//
// These helpers EXTEND the slice-3 [`DamageScenario`] machinery (no plumbing
// rebuilt): they reuse `legacy_run_dmg` / `stack_run_dmg` / `build_stack_stream_dmg`
// (which already understand the slice-4 `substitute_hp`/`trapping_turns` fields)
// and add the VOLATILE assertion the slice needs — the substitute hp + the
// substitute-up flag + the trap counter, both sides, legacy vs stack.
// ════════════════════════════════════════════════════════════════════════════

/// Read the stack's Substitute hp for `who` from the effect arena (`0` if none).
fn stack_sub_hp(stack_effects: &[EffectState<PocData>], who: BattlerRef) -> u16 {
    stack_effects
        .iter()
        .find_map(|e| match (&e.kind, e.host == who) {
            (PocKind::Substitute { hp }, true) => Some(*hp),
            _ => None,
        })
        .unwrap_or(0)
}

/// Whether the stack still has a Substitute volatile for `who` (the engine
/// equivalent of the legacy `HAS_SUBSTITUTE_UP` flag — the volatile's PRESENCE).
fn stack_sub_up(stack_effects: &[EffectState<PocData>], who: BattlerRef) -> bool {
    stack_effects
        .iter()
        .any(|e| e.host == who && matches!(e.kind, PocKind::Substitute { .. }))
}

/// Read the stack's partial-trap turns for `who` (`0` if not trapping).
fn stack_trap_turns(stack_effects: &[EffectState<PocData>], who: BattlerRef) -> u8 {
    stack_effects
        .iter()
        .find_map(|e| match (&e.kind, e.host == who) {
            (PocKind::Trapping { turns_left }, true) => Some(*turns_left),
            _ => None,
        })
        .unwrap_or(0)
}

/// Run a slice-4 Substitute/partial-trap scenario and assert FULL parity:
/// `BattleState` (both sides' hp + status) AND the **volatiles** (each side's
/// substitute hp + substitute-up flag + trap turns) AND `consumed()`.
///
/// The legacy volatile lives in the `BattlerState` (`substitute_hp` +
/// `HAS_SUBSTITUTE_UP`; `num_attacks_left` + `USING_TRAPPING_MOVE`); the stack
/// volatile lives in the `EffectState` arena (`PocKind::Substitute`/`Trapping`).
/// This asserts the two agree value-for-value AND flag-for-presence — so the
/// absorb/break and the trap counter are proven equal to the legacy oracle.
pub fn run_scenario_sub(s: &DamageScenario) {
    let legacy = legacy_run_dmg(s);
    let (stack, consumed, first, stack_effects) = stack_run_sub(s);

    // 1. Core BattleState: both sides' hp + status (the existing oracle).
    assert_state_parity_dmg(s, &legacy, &stack);

    // 2. Substitute hp + up-flag parity, both sides.
    for (who, lb) in [
        (BattlerRef::PLAYER, &legacy.player),
        (BattlerRef::OPPONENT, &legacy.enemy),
    ] {
        let legacy_sub_hp = lb.substitute_hp as u16;
        let legacy_sub_up = lb.has_status2(status2::HAS_SUBSTITUTE_UP);
        let s_hp = stack_sub_hp(&stack_effects, who);
        let s_up = stack_sub_up(&stack_effects, who);
        assert_eq!(
            legacy_sub_hp, s_hp,
            "[{}] side {} substitute_hp: legacy={} stack={}",
            s.name, who.side, legacy_sub_hp, s_hp
        );
        assert_eq!(
            legacy_sub_up, s_up,
            "[{}] side {} substitute-up flag: legacy={} stack={} (break parity)",
            s.name, who.side, legacy_sub_up, s_up
        );
    }

    // 3. Partial-trap turn-counter parity, both sides.
    for (who, lb) in [
        (BattlerRef::PLAYER, &legacy.player),
        (BattlerRef::OPPONENT, &legacy.enemy),
    ] {
        let legacy_trap = if lb.has_status1(status1::USING_TRAPPING_MOVE) {
            lb.num_attacks_left
        } else {
            0
        };
        let s_trap = stack_trap_turns(&stack_effects, who);
        assert_eq!(
            legacy_trap, s_trap,
            "[{}] side {} trap turns: legacy={} stack={}",
            s.name, who.side, legacy_trap, s_trap
        );
    }

    // 4. consumed() parity (draw-order drift detector — same as slice 3).
    let (_b, expected) = build_stack_stream_dmg(s, first, order_is_tie_dmg(s));
    assert_eq!(
        consumed, expected,
        "[{}] stack consumed {} bytes, expected {} (draw-order drift!)",
        s.name, consumed, expected
    );
}

/// Like [`stack_run_dmg`] but ALSO returns the post-turn effect arena so the
/// slice-4 oracle can inspect the surviving Substitute/Trapping volatiles. Reuses
/// the exact same setup as `stack_run_dmg` (no plumbing duplicated beyond the
/// arena hand-back).
pub fn stack_run_sub(
    s: &DamageScenario,
) -> (EngineState<PocData>, usize, FirstMover, Vec<EffectState<PocData>>) {
    let _guard = with_active_move(s.move_data);

    let mut state = EngineState::new(
        vec![engine_battler_dmg(&s.player)],
        vec![engine_battler_dmg(&s.enemy)],
    );
    let mut effects: Vec<EffectState<PocData>> = Vec::new();
    if s.player.focus_energy {
        effects.push(EffectState {
            id: EFF_FOCUS_ENERGY,
            host: BattlerRef::PLAYER,
            effect_order: 0,
            kind: PocKind::FocusEnergy,
        });
    }
    if s.enemy.focus_energy {
        effects.push(EffectState {
            id: EffectId(EFF_FOCUS_ENERGY.0 + 1),
            host: BattlerRef::OPPONENT,
            effect_order: 1,
            kind: PocKind::FocusEnergy,
        });
    }
    if s.player.substitute_hp > 0 {
        effects.push(EffectState {
            id: EFF_SUBSTITUTE,
            host: BattlerRef::PLAYER,
            effect_order: 2,
            kind: PocKind::Substitute { hp: s.player.substitute_hp },
        });
    }
    if s.enemy.substitute_hp > 0 {
        effects.push(EffectState {
            id: EffectId(EFF_SUBSTITUTE.0 + 1),
            host: BattlerRef::OPPONENT,
            effect_order: 3,
            kind: PocKind::Substitute { hp: s.enemy.substitute_hp },
        });
    }
    if s.player.trapping_turns > 0 {
        effects.push(EffectState {
            id: EFF_TRAPPING,
            host: BattlerRef::PLAYER,
            effect_order: 4,
            kind: PocKind::Trapping { turns_left: s.player.trapping_turns },
        });
    }
    if s.enemy.trapping_turns > 0 {
        effects.push(EffectState {
            id: EffectId(EFF_TRAPPING.0 + 1),
            host: BattlerRef::OPPONENT,
            effect_order: 5,
            kind: PocKind::Trapping { turns_left: s.enemy.trapping_turns },
        });
    }

    let provider = PocData;
    let actions = [
        BattleAction::<PocData>::Fight { move_: MoveId::Thundershock },
        BattleAction::<PocData>::Fight { move_: MoveId::Thundershock },
    ];
    let first = first_mover_dmg(s);
    let tie = order_is_tie_dmg(s);
    let (bytes, _expected) = build_stack_stream_dmg(s, first, tie);
    let mut rng = ScriptedRng::new(bytes);
    let result = StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
    assert_eq!(result.first, first, "[{}] first-mover probe disagreed", s.name);
    (state, rng.consumed(), first, effects)
}

// ════════════════════════════════════════════════════════════════════════════
// SLICE 5 — Gen-1 end-of-turn RESIDUALS as effect handlers (burn/poison flat,
// Toxic uncapped ramp, Leech Seed cross-battler drain+heal), proven at parity
// with the legacy `apply_all_residual` oracle (via `execute_turn`).
//
// Residuals draw NO rng in Gen-1 (fixed `/16` math), so adding them must leave
// `consumed()` UNCHANGED vs the no-residual run — every scenario asserts that.
// The handlers fire in ASM order (status damage `order:10`, THEN leech `order:30`
// + arena id `150 > 140`), per-mover, with the first-mover-faint short-circuit
// (a residual KO of the first mover cancels the second move) already enforced by
// the `StackDriver` from slice 1 — re-proven here under residual KOs.
//
// This is the FIRST genuinely load-bearing `pair_mut` use: Leech Seed drains the
// host's real hp AND heals the opponent's real hp in one tick (two battlers'
// real `hp`, both writes essential), unlike slice 4's Substitute whose absorb
// routed through the arena. `ResidualScenario` runs MULTIPLE turns reusing ONE
// persistent effect arena so the Toxic counter ramps across turns (1,2,3,…) —
// the cross-turn state a single `execute_turn` cannot drive.
// ════════════════════════════════════════════════════════════════════════════

/// Per-side residual config for a [`ResidualScenario`]. Sets exactly the inputs
/// the legacy `apply_all_residual` reads: the non-volatile `status` (Burn/Poison
/// → flat tick), the badly-poisoned flag + initial toxic counter (→ the ramp),
/// and the seeded flag (→ leech). `max_hp == hp` initially (full hp).
#[derive(Clone, Copy)]
pub struct ResidualMon {
    /// Current hp. `clean` sets `max_hp == hp`; set `hp` AFTER `clean` to make a
    /// damaged mon (so `max_hp` stays the full value the residual `/16` uses).
    pub hp: u16,
    /// Max hp — the `/16` base for every residual. Kept distinct from `hp` so leech
    /// heal-room and exact-faint boundaries can be set without shrinking `max_hp`.
    pub max_hp: u16,
    pub speed: u16,
    /// Non-volatile status. Burn/Poison tick flat `floor(maxHP/16).max(1)`.
    pub status: LegacyStatus,
    /// Badly-poisoned (Toxic). The ramp fires ONLY when `status ∉ {Burn,Poison}`
    /// (the oracle's `_` arm), matching `residual.rs:33-42`.
    pub badly_poisoned: bool,
    /// Initial toxic counter (legacy `toxic_counter`; the volatile's `counter`).
    pub toxic_counter: u8,
    /// Leech-seeded (drains this mon, heals its opponent each tick it acts).
    pub seeded: bool,
}

impl ResidualMon {
    /// A clean, status-free, full-hp mon (`hp == max_hp`). Set `.hp` afterwards to
    /// make a damaged mon while keeping `max_hp` (the residual `/16` base) intact.
    pub fn clean(hp: u16, speed: u16) -> Self {
        Self {
            hp,
            max_hp: hp,
            speed,
            status: LegacyStatus::None,
            badly_poisoned: false,
            toxic_counter: 0,
            seeded: false,
        }
    }
}

/// The slice-5 scenario: two [`ResidualMon`]s, the per-mover [`MoveBytes`], the
/// move both use, and a turn count (for the multi-turn Toxic ramp). Distinct from
/// the slice-1/3 scenario types so none disturbs the others.
#[derive(Clone)]
pub struct ResidualScenario {
    pub name: &'static str,
    pub player: ResidualMon,
    pub enemy: ResidualMon,
    pub move_data: MoveData,
    pub order_byte: u8,
    pub first: MoveBytes,
    pub second: MoveBytes,
    /// How many full turns to run (>=1). Each turn reuses the persistent arena, so
    /// the Toxic counter ramps across turns exactly like the legacy `toxic_counter`.
    pub turns: u32,
}

/// Build a legacy [`Pokemon`] from a [`ResidualMon`] (just hp/speed/types; status
/// + toxic + seeded ride on the `BattlerState`).
fn poke_residual(m: &ResidualMon) -> Pokemon {
    let base = get_base_stats(Species::Pikachu).expect("species base stats");
    Pokemon {
        species: Species::Pikachu,
        nickname: [0x50; 11],
        level: 50,
        hp: m.hp,
        max_hp: m.max_hp,
        attack: 100,
        defense: 80,
        speed: m.speed,
        special: 80,
        type1: base.type1,
        type2: base.type2,
        moves: [MoveId::Thundershock, MoveId::None, MoveId::None, MoveId::None],
        pp: [30, 0, 0, 0],
        pp_ups: [0; 4],
        status: m.status,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

/// Build an engine battler from a [`ResidualMon`] (hp may be < max_hp; the
/// residual `/16` uses `max_hp`).
fn engine_battler_residual(m: &ResidualMon) -> EngineBattler<PocData> {
    let mut stats = EnumMap::new();
    stats.set(Stat::Attack, 100);
    stats.set(Stat::Defense, 80);
    stats.set(Stat::Speed, m.speed);
    stats.set(Stat::Special, 80);
    stats.set(Stat::Level, 50);
    stats.set(Stat::MaxHp, m.max_hp);
    let mut b = EngineBattler::new(Species::Pikachu, m.hp, m.max_hp, stats, vec![MoveId::Thundershock]);
    if m.status != LegacyStatus::None {
        b.status = Some(m.status);
    }
    b
}

/// Apply the residual flags (badly-poisoned + counter, seeded) onto a legacy
/// `BattlerState`, exactly where the oracle reads them.
fn set_legacy_residual(b: &mut super::state::BattlerState, m: &ResidualMon) {
    use super::state::{status2, status3};
    if m.badly_poisoned {
        b.set_status3(status3::BADLY_POISONED);
        b.toxic_counter = m.toxic_counter;
    }
    if m.seeded {
        b.set_status2(status2::SEEDED);
    }
}

/// Run the legacy oracle for a [`ResidualScenario`] across `turns` turns, reusing
/// ONE `BattleState` so the `toxic_counter` ramps across turns. Returns the final
/// state.
pub fn legacy_run_residual(s: &ResidualScenario) -> LegacyState {
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![poke_residual(&s.player)],
        vec![poke_residual(&s.enemy)],
    );
    state.player.selected_move = MoveId::Thundershock;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Thundershock;
    state.enemy.selected_move_index = 0;
    set_legacy_residual(&mut state.player, &s.player);
    set_legacy_residual(&mut state.enemy, &s.enemy);

    let randoms = TurnRandoms {
        order_random: s.order_byte,
        first_mover: to_move_randoms(s.first),
        second_mover: to_move_randoms(s.second),
    };
    for _ in 0..s.turns.max(1) {
        // Stop once a side has fainted (the oracle's caller would end the battle;
        // continuing to call execute_turn on a 0-hp mon is not meaningful).
        if state.player.active_mon().hp == 0 || state.enemy.active_mon().hp == 0 {
            break;
        }
        execute_turn(&mut state, &s.move_data, &s.move_data, &randoms);
    }
    state
}

/// Build the engine arena for a [`ResidualScenario`]: a Toxic volatile (≙ badly-
/// poisoned + counter) and/or a Seeded volatile per side, with ids that pin the
/// ASM order (toxic `140 < 150` leech) and keep the arena sorted.
fn build_residual_effects(s: &ResidualScenario) -> Vec<EffectState<PocData>> {
    let mut effects: Vec<EffectState<PocData>> = Vec::new();
    let mut order: u64 = 0;
    if s.player.badly_poisoned {
        effects.push(EffectState {
            id: EFF_TOXIC,
            host: BattlerRef::PLAYER,
            effect_order: order,
            kind: PocKind::Toxic { counter: s.player.toxic_counter },
        });
        order += 1;
    }
    if s.enemy.badly_poisoned {
        effects.push(EffectState {
            id: EffectId(EFF_TOXIC.0 + 1),
            host: BattlerRef::OPPONENT,
            effect_order: order,
            kind: PocKind::Toxic { counter: s.enemy.toxic_counter },
        });
        order += 1;
    }
    if s.player.seeded {
        effects.push(EffectState {
            id: EFF_SEEDED,
            host: BattlerRef::PLAYER,
            effect_order: order,
            kind: PocKind::Seeded,
        });
        order += 1;
    }
    if s.enemy.seeded {
        effects.push(EffectState {
            id: EffectId(EFF_SEEDED.0 + 1),
            host: BattlerRef::OPPONENT,
            effect_order: order,
            kind: PocKind::Seeded,
        });
    }
    // Arena MUST stay sorted by id for the engine's binary-search + the driver's
    // arena-order residual pass (toxic ids 140/141 < leech ids 150/151).
    effects.sort_by_key(|e| e.id);
    effects
}

/// First mover for a [`ResidualScenario`] (RNG-free rank, tie → `order_byte<128`).
fn first_mover_residual(s: &ResidualScenario) -> FirstMover {
    let provider = PocData;
    let state = EngineState::new(
        vec![engine_battler_residual(&s.player)],
        vec![engine_battler_residual(&s.enemy)],
    );
    let pr = provider.turn_order_rank(&state, BattlerRef::PLAYER, &MoveId::Thundershock);
    let er = provider.turn_order_rank(&state, BattlerRef::OPPONENT, &MoveId::Thundershock);
    match pr.cmp(&er) {
        std::cmp::Ordering::Less => FirstMover::Player,
        std::cmp::Ordering::Greater => FirstMover::Opponent,
        std::cmp::Ordering::Equal => {
            if s.order_byte < 128 {
                FirstMover::Player
            } else {
                FirstMover::Opponent
            }
        }
    }
}

fn order_is_tie_residual(s: &ResidualScenario) -> bool {
    let provider = PocData;
    let state = EngineState::new(
        vec![engine_battler_residual(&s.player)],
        vec![engine_battler_residual(&s.enemy)],
    );
    let pr = provider.turn_order_rank(&state, BattlerRef::PLAYER, &MoveId::Thundershock);
    let er = provider.turn_order_rank(&state, BattlerRef::OPPONENT, &MoveId::Thundershock);
    pr == er
}

/// The slice-5 consumed-byte predictor for ONE turn. Residuals draw NO bytes, so
/// this is exactly the BeforeMove-gate (paralysis only here) → crit → accuracy →
/// damage predictor — the SAME shape as [`build_stack_stream_dmg`], proving by
/// construction that adding residuals adds ZERO bytes.
fn build_residual_stream(s: &ResidualScenario, first: FirstMover, tie: bool) -> (Vec<u8>, usize) {
    let mut bytes = if tie { vec![s.order_byte] } else { Vec::new() };
    let (fa, fb, sa, sb) = match first {
        FirstMover::Player => (&s.player, s.first, &s.enemy, s.second),
        FirstMover::Opponent => (&s.enemy, s.first, &s.player, s.second),
    };
    let push_mover = |bytes: &mut Vec<u8>, atk: &ResidualMon, mb: MoveBytes| {
        // The BeforeMove gate: Sleep/Freeze abort with no draw; Paralysis draws one
        // byte and may abort. Burn/Poison/None do not gate. (Slice-5 scenarios use
        // Burn/Poison/None as the carrier so per-turn byte count is constant.)
        match atk.status {
            LegacyStatus::Sleep(_) => return, // asleep → abort, no draw
            LegacyStatus::Freeze => return,   // frozen → abort, no draw
            LegacyStatus::Paralysis => {
                bytes.push(mb.paralysis);
                if mb.paralysis < 63 {
                    return; // fully paralyzed → abort, no further draws
                }
            }
            _ => {}
        }
        bytes.push(mb.crit);
        bytes.push(mb.accuracy);
        let scaled = (s.move_data.accuracy as u32 * 255 / 100).min(255) as u8;
        if mb.accuracy < scaled {
            bytes.push(mb.damage);
        }
    };
    push_mover(&mut bytes, fa, fb);
    // The scenarios keep hp high enough that the MOVE never KOs mid-turn (residual
    // KO scenarios assert consumed() directly), so the second mover always acts —
    // EXCEPT when a residual KO of the first mover cancels it. Those are asserted
    // directly, not via this predictor (callers pass turns=1 + direct consumed).
    push_mover(&mut bytes, sa, sb);
    let consumed = bytes.len();
    (bytes, consumed)
}

/// Run the stack path for a [`ResidualScenario`] across `turns` turns, reusing ONE
/// persistent effect arena (so the Toxic counter ramps across turns). Returns the
/// final `(state, total_consumed, first, effects)`. `total_consumed` is the SUM
/// across turns; per-turn parity is asserted separately by the caller for turn 1.
pub fn stack_run_residual(
    s: &ResidualScenario,
) -> (EngineState<PocData>, usize, FirstMover, Vec<EffectState<PocData>>) {
    let _guard = with_active_move(s.move_data);
    let mut state = EngineState::new(
        vec![engine_battler_residual(&s.player)],
        vec![engine_battler_residual(&s.enemy)],
    );
    let mut effects = build_residual_effects(s);
    let provider = PocData;
    let first = first_mover_residual(s);
    let tie = order_is_tie_residual(s);

    let mut total_consumed = 0usize;
    for _ in 0..s.turns.max(1) {
        if state.player_battlers[0].hp == 0 || state.opponent_battlers[0].hp == 0 {
            break;
        }
        // `execute_turn` consumes `actions` by value → rebuild each turn.
        let actions = [
            BattleAction::<PocData>::Fight { move_: MoveId::Thundershock },
            BattleAction::<PocData>::Fight { move_: MoveId::Thundershock },
        ];
        let (bytes, _e) = build_residual_stream(s, first, tie);
        let mut rng = ScriptedRng::new(bytes);
        let result =
            StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
        assert_eq!(result.first, first, "[{}] first-mover probe disagreed", s.name);
        total_consumed += rng.consumed();
    }
    (state, total_consumed, first, effects)
}

/// Read the stack Toxic counter for `who` (`None` if not badly poisoned).
pub fn stack_toxic_counter(effects: &[EffectState<PocData>], who: BattlerRef) -> Option<u8> {
    effects.iter().find_map(|e| match (&e.kind, e.host == who) {
        (PocKind::Toxic { counter }, true) => Some(*counter),
        _ => None,
    })
}

/// Whether the stack still hosts a Seeded volatile for `who`.
pub fn stack_seeded(effects: &[EffectState<PocData>], who: BattlerRef) -> bool {
    effects
        .iter()
        .any(|e| e.host == who && matches!(e.kind, PocKind::Seeded))
}

/// Differential oracle for a [`ResidualScenario`]: legacy `execute_turn` (with
/// `apply_all_residual`) vs `StackDriver` — final hp + status, BOTH sides, AND
/// the Toxic counter ramp (both sides), AND `consumed()` (residuals add NO bytes).
///
/// `consumed()` is asserted against a SINGLE-turn no-residual baseline run of the
/// SAME move bytes: the stack drew exactly that many bytes per turn × turns, so a
/// residual that secretly drew a byte would diverge. This pins "residuals draw no
/// rng" as a hard, diffed claim — not an assumption.
pub fn run_scenario_residual(s: &ResidualScenario) {
    let legacy = legacy_run_residual(s);
    let (stack, consumed, first, effects) = stack_run_residual(s);

    // 1. Final BattleState: both sides' hp + status, legacy vs stack.
    let lp = legacy.player.active_mon();
    let le = legacy.enemy.active_mon();
    let sp = &stack.player_battlers[0];
    let se = &stack.opponent_battlers[0];
    assert_eq!(lp.hp, sp.hp, "[{}] PLAYER hp: legacy={} stack={}", s.name, lp.hp, sp.hp);
    assert_eq!(le.hp, se.hp, "[{}] ENEMY hp: legacy={} stack={}", s.name, le.hp, se.hp);
    assert_eq!(
        lp.status,
        sp.status.unwrap_or(LegacyStatus::None),
        "[{}] PLAYER status",
        s.name
    );
    assert_eq!(
        le.status,
        se.status.unwrap_or(LegacyStatus::None),
        "[{}] ENEMY status",
        s.name
    );

    // 2. Toxic counter ramp parity, both sides (legacy `toxic_counter` vs the
    //    stack volatile's `counter`). Only meaningful while the mon is alive AND
    //    badly poisoned in BOTH paths; a faint clears neither here (we ran until a
    //    faint), so compare the surviving counters.
    for (who, lb, alive) in [
        (BattlerRef::PLAYER, &legacy.player, sp.hp > 0),
        (BattlerRef::OPPONENT, &legacy.enemy, se.hp > 0),
    ] {
        if !alive {
            continue; // a fainted mon's counter is not asserted (battle would end)
        }
        if let Some(stack_ctr) = stack_toxic_counter(&effects, who) {
            assert_eq!(
                lb.toxic_counter, stack_ctr,
                "[{}] side {} toxic_counter ramp: legacy={} stack={}",
                s.name, who.side, lb.toxic_counter, stack_ctr
            );
        }
    }

    // 3. consumed() parity: residuals add NO bytes. `run_scenario_residual` is for
    //    FAINT-FREE multi-turn scenarios (the residual-KO case is asserted directly
    //    in its own test), so every turn is a full both-movers turn with a CONSTANT
    //    per-turn byte count. The stack must have drawn exactly `per_turn × turns`.
    //    A residual that secretly drew a byte would diverge from this baseline.
    assert!(
        sp.hp > 0 && se.hp > 0,
        "[{}] run_scenario_residual is for faint-free runs (use a direct consumed() \
         assert for residual-KO scenarios); a side fainted",
        s.name
    );
    let tie = order_is_tie_residual(s);
    let (_b, per_turn) = build_residual_stream(s, first, tie);
    let expected = per_turn * (s.turns.max(1) as usize);
    assert_eq!(
        consumed, expected,
        "[{}] stack consumed {} bytes, expected {} ({} bytes/turn × {} turns) — \
         residuals must draw NO rng (draw-order drift!)",
        s.name, consumed, expected, per_turn, s.turns
    );
}

// ════════════════════════════════════════════════════════════════════════════
// SLICE 6 — multi-turn lock-in + Counter + Bide harness.
//
// Runs the STACK across N turns reusing ONE persistent effect arena (the cross-
// turn home of every lock-in counter / charge flag / recharge flag / Bide
// accumulator). Each turn the harness FIRST clears the per-turn `DamageTaken`
// scratch (so it holds only THIS turn's damage), then drives `execute_turn` with
// the per-turn CHOSEN actions — which a live lock-in volatile overrides via the
// engine's `forced_action` seam.
//
// ## Oracle scope — an HONEST design finding (the slice was meant to surface it)
//
// The legacy `execute_turn` (turn.rs) does NOT drive multi-turn lock-in: it takes
// a FIXED `(player_move, enemy_move)` each turn and never re-issues a locked move,
// never skips a recharge turn, never accumulates Bide across turns. The lock-in
// LIFECYCLE lives at a higher layer than `execute_turn`. AND there is NO Counter
// damage in the legacy battle code at all (`MoveEffect` has no `CounterEffect`;
// Counter exists only as a −1-priority `MoveId` in `turn_order.rs`). So:
//   * For the per-turn DAMAGE MATH (a forced Thrash/Fly strike = a normal move),
//     the harness DIFFS the stack vs the legacy `execute_turn` single-turn oracle
//     (same move → identical damage), via [`legacy_single_turn_damage`].
//   * For the lock-in EFFECT MATH (Bide ×2, Thrash end-confusion, Trapping/Fly/
//     recharge counters), the oracle is the legacy `multi_turn_effects.rs` unit
//     tests (`apply_bide`/`apply_thrash`/… — re-homed here, same constants), which
//     the slice-6 assertions reproduce by construction and pin directly.
//   * Counter's 2× reflection has NO legacy oracle; it is DIRECT-PINNED (expected
//     = 2× the diffable opponent damage), and the diffable part (the opponent's
//     hit on the Counter user) IS asserted vs the legacy oracle.
// This is the §9 finding made concrete: Counter/lock-in cannot be diffed against
// `execute_turn` because `execute_turn`'s per-turn `[move;2]` input cannot express
// them — exactly why the engine needed the (generic, defaulted) `forced_action`
// cross-turn seam.
// ════════════════════════════════════════════════════════════════════════════

/// Which lock-in volatile a side starts with (slice 6).
#[derive(Clone, Copy)]
pub enum Lockin {
    /// No lock-in (normal mover).
    None,
    /// Thrash/Petal Dance: forced `move_` for `turns_left` turns, then self-confuse.
    Locked { move_: MoveId, turns_left: u8, confuse_on_end: bool },
    /// Fly/Solar Beam: charge turn (`Nothing`) then strike `move_`.
    TwoTurn { move_: MoveId, invulnerable: bool },
    /// Hyper Beam recharge: this side skips THIS turn (forced `Nothing`).
    Recharge,
    /// Bide: store for `turns_left` turns (accumulate damage taken), unleash ×2.
    Bide { turns_left: u8, accumulated: u16 },
}

/// A slice-6 scenario: two mons (hp/speed/status), each side's lock-in start
/// state, the chosen move each side picks each turn (overridden by lock-in), the
/// move data both use for damage math, per-mover bytes, and a turn count.
// `name`/`order_byte` mirror the other slices' scenario structs for uniformity;
// slice-6's lock-in tests don't read them (lock-in draws no order tie byte), hence allow.
#[derive(Clone)]
#[allow(dead_code)]
pub struct LockinScenario {
    pub name: &'static str,
    pub player_hp: u16,
    pub enemy_hp: u16,
    pub player_speed: u16,
    pub enemy_speed: u16,
    pub player_status: LegacyStatus,
    pub enemy_status: LegacyStatus,
    /// The action each side CHOOSES each turn (a lock-in overrides it).
    pub player_choice: MoveId,
    pub enemy_choice: MoveId,
    /// Lock-in start state per side.
    pub player_lockin: Lockin,
    pub enemy_lockin: Lockin,
    /// The move data used for the damage pipeline (power/type/accuracy).
    pub move_data: MoveData,
    pub order_byte: u8,
    pub first: MoveBytes,
    pub second: MoveBytes,
    pub turns: u32,
}

impl LockinScenario {
    /// Two clean fast/slow Pikachus, both choose Thundershock, always hit, 1 turn.
    pub fn base(name: &'static str) -> Self {
        Self {
            name,
            player_hp: 5000,
            enemy_hp: 5000,
            player_speed: 200,
            enemy_speed: 50,
            player_status: LegacyStatus::None,
            enemy_status: LegacyStatus::None,
            player_choice: MoveId::Thundershock,
            enemy_choice: MoveId::Thundershock,
            player_lockin: Lockin::None,
            enemy_lockin: Lockin::None,
            move_data: SLICE12_MOVE,
            order_byte: 0,
            first: MoveBytes::always_hit(),
            second: MoveBytes::always_hit(),
            turns: 1,
        }
    }
}

fn build_lockin_effect(host: BattlerRef, lockin: Lockin, order: u64) -> Option<EffectState<PocData>> {
    let id_off = if host.side == 0 { 0 } else { 1 };
    let kind = match lockin {
        Lockin::None => return None,
        Lockin::Locked { move_, turns_left, confuse_on_end } => {
            PocKind::LockedMove { move_, turns_left, confuse_on_end }
        }
        Lockin::TwoTurn { move_, invulnerable } => {
            PocKind::TwoTurn { move_, charging: true, invulnerable }
        }
        Lockin::Recharge => PocKind::Recharge,
        Lockin::Bide { turns_left, accumulated } => PocKind::Bide { accumulated, turns_left },
    };
    Some(EffectState {
        id: EffectId(EFF_LOCKIN.0 + id_off),
        host,
        effect_order: order,
        kind,
    })
}

/// Run the STACK across `turns` turns reusing ONE persistent arena (so lock-in
/// counters / charge flags / Bide accumulators persist). Resets the per-turn
/// `DamageTaken` scratch at the start of each turn. Returns the final
/// `(state, total_consumed, effects)`.
pub fn stack_run_lockin(s: &LockinScenario) -> (EngineState<PocData>, usize, Vec<EffectState<PocData>>) {
    let _guard = with_active_move(s.move_data);
    let mut state = EngineState::new(
        vec![engine_battler(Species::Pikachu, s.player_hp, s.player_speed, s.player_status)],
        vec![engine_battler(Species::Pikachu, s.enemy_hp, s.enemy_speed, s.enemy_status)],
    );
    let mut effects: Vec<EffectState<PocData>> = Vec::new();
    if let Some(e) = build_lockin_effect(BattlerRef::PLAYER, s.player_lockin, 0) {
        effects.push(e);
    }
    if let Some(e) = build_lockin_effect(BattlerRef::OPPONENT, s.enemy_lockin, 1) {
        effects.push(e);
    }
    effects.sort_by_key(|e| e.id);

    let provider = PocData;
    let mut total_consumed = 0usize;
    for _ in 0..s.turns.max(1) {
        if state.player_battlers[0].hp == 0 || state.opponent_battlers[0].hp == 0 {
            break;
        }
        // Reset the per-turn DamageTaken scratch (it holds only THIS turn's damage).
        effects.retain(|e| !matches!(e.kind, PocKind::DamageTaken { .. }));
        let actions = [
            BattleAction::<PocData>::Fight { move_: s.player_choice },
            BattleAction::<PocData>::Fight { move_: s.enemy_choice },
        ];
        // Build the rng stream: turn order has no tie (speeds differ), and the
        // forced/locked actions determine which movers draw crit/acc/dmg. We feed
        // a generous stream and assert consumed() directly per scenario.
        let bytes = vec![
            s.first.crit, s.first.accuracy, s.first.damage,
            s.second.crit, s.second.accuracy, s.second.damage,
        ];
        let mut rng = ScriptedRng::new(bytes);
        StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
        total_consumed += rng.consumed();
    }
    (state, total_consumed, effects)
}

/// The damage a single normal move deals in the LEGACY oracle (one turn, player
/// hits a clean enemy). The cross-check for "a forced lock-in strike deals the
/// SAME damage as a normal move" — the diffable part of the lock-in math.
pub fn legacy_single_turn_damage(move_data: &MoveData, attacker_hp: u16, defender_hp: u16) -> u16 {
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![poke(Species::Pikachu, attacker_hp, 200, LegacyStatus::None)],
        vec![poke(Species::Pikachu, defender_hp, 50, LegacyStatus::None)],
    );
    state.player.selected_move = move_data.id;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Thundershock;
    state.enemy.selected_move_index = 0;
    // Player faster → moves first; enemy at huge hp so no KO cancels nothing here.
    // We read the enemy hp delta after the player's move only (single isolated hit):
    // run with the enemy missing so only the player's hit lands.
    let randoms = TurnRandoms {
        order_random: 0,
        first_mover: to_move_randoms(MoveBytes::always_hit()),
        second_mover: to_move_randoms(MoveBytes {
            confusion: 255,
            paralysis: 255,
            crit: 255,
            accuracy: 255, // enemy MISSES → only the player's hit lands
            damage: 255,
        }),
    };
    execute_turn(&mut state, move_data, &move_data_thundershock(), &randoms);
    defender_hp - state.enemy.active_mon().hp
}

fn move_data_thundershock() -> MoveData {
    SLICE12_MOVE
}

/// Read the stack's LockedMove `turns_left` for `who` (`None` if not locked).
pub fn stack_lock_turns(effects: &[EffectState<PocData>], who: BattlerRef) -> Option<u8> {
    effects.iter().find_map(|e| match (&e.kind, e.host == who) {
        (PocKind::LockedMove { turns_left, .. }, true) => Some(*turns_left),
        _ => None,
    })
}

/// Whether the stack still hosts a Recharge volatile for `who`.
pub fn stack_recharging(effects: &[EffectState<PocData>], who: BattlerRef) -> bool {
    effects.iter().any(|e| e.host == who && matches!(e.kind, PocKind::Recharge))
}

/// Read the stack's TwoTurn `charging` flag for `who` (`None` if no two-turn move).
pub fn stack_twoturn_charging(effects: &[EffectState<PocData>], who: BattlerRef) -> Option<bool> {
    effects.iter().find_map(|e| match (&e.kind, e.host == who) {
        (PocKind::TwoTurn { charging, .. }, true) => Some(*charging),
        _ => None,
    })
}

/// Read the stack's Bide `(accumulated, turns_left)` for `who` (`None` if no Bide).
pub fn stack_bide(effects: &[EffectState<PocData>], who: BattlerRef) -> Option<(u16, u8)> {
    effects.iter().find_map(|e| match (&e.kind, e.host == who) {
        (PocKind::Bide { accumulated, turns_left }, true) => Some((*accumulated, *turns_left)),
        _ => None,
    })
}

/// Whether the stack hosts a Confusion volatile for `who` (Thrash end-confusion).
pub fn stack_confused(effects: &[EffectState<PocData>], who: BattlerRef) -> bool {
    effects.iter().any(|e| e.host == who && matches!(e.kind, PocKind::Confusion { .. }))
}

// ════════════════════════════════════════════════════════════════════════════
// SLICE 7 — representative Gen-1 SECONDARY / SPECIAL effects parity harness.
//
// One representative per category, each DIFFED against the legacy oracle:
//   * status-on-hit (PoisonSideEffect2) ─┐
//   * stat-drop-on-hit (SpecialDownSideEffect) ├ vs `execute_turn`
//   * flinch (FlinchSideEffect2) ─┤  (apply_move_effect via the live pipeline)
//   * recoil (RecoilEffect) ──────┤
//   * drain (DrainHpEffect) ──────┘
//   * Haze (HazeEffect, power-0 global) vs `execute_turn`
//
// The side-effect roll (`side_effect_roll`) is a STRUCT FIELD in the legacy
// oracle (always read after damage). The stack draws ONE byte at `DamagingHit`
// for a side-effect move that HIT — matching legacy's always-read-the-field
// semantics: the byte is consumed whether or not the secondary FIRES, so
// `consumed()` parity holds at the threshold boundary (thr-1 fires / thr does
// not), which the scenarios pin directly.
//
// ## Determinism — the `MoveRandoms` field order
//
// `MoveRandoms.effect_randoms` is the LAST field (after confusion/para/crit/acc/
// damage), so the stack draws the side_effect byte LAST per mover — the byte
// stream is `[para?] crit acc dmg [side_effect?]`, matching the field order.
// EXTENDS the slice-1/3 [`MoveBytes`] shim with one extra per-mover byte carried
// SEPARATELY (in `SecondaryScenario`) so the 26 existing `MoveBytes` literals in
// slices 1-6 are untouched (additive, like slices 3/5/6).
// ════════════════════════════════════════════════════════════════════════════

/// Per-side mon config for a [`SecondaryScenario`]. Carries the inputs the
/// secondary handlers read: hp/speed/status (status gates `apply_*_side`), the
/// SpecialDown stat stage (so the drop is observable), the species (Poison-type
/// immunity), and a pre-existing flinch flag.
#[derive(Clone, Copy)]
pub struct SecondaryMon {
    pub species: Species,
    pub hp: u16,
    pub max_hp: u16,
    pub speed: u16,
    pub status: LegacyStatus,
    /// Pre-set Special stat stage (so a stat-drop / Haze reset is observable).
    pub spc_stage: i8,
    /// Pre-set Attack stat stage (so a Haze reset is observable).
    pub atk_stage: i8,
    /// Pre-set a flinch flag (legacy `status1::FLINCHED`; the stack `PocKind::Flinch`).
    pub flinched: bool,
}

impl SecondaryMon {
    /// A clean full-hp Pikachu (no status/stages/flinch). Set fields afterwards.
    pub fn clean(hp: u16, speed: u16) -> Self {
        Self {
            species: Species::Pikachu,
            hp,
            max_hp: hp,
            speed,
            status: LegacyStatus::None,
            spc_stage: 0,
            atk_stage: 0,
            flinched: false,
        }
    }
}

/// The slice-7 scenario: two [`SecondaryMon`]s, the move both movers use (carries
/// the `MoveEffect` that selects the secondary), per-mover damage bytes, and the
/// per-mover `side_effect_roll` (carried separately from [`MoveBytes`]).
#[derive(Clone)]
pub struct SecondaryScenario {
    pub name: &'static str,
    pub player: SecondaryMon,
    pub enemy: SecondaryMon,
    /// The move both movers use (power/type/accuracy/EFFECT). The `effect` field
    /// drives which secondary fires (PoisonSide / SpecialDown / Flinch / Recoil /
    /// Drain / Haze).
    pub move_data: MoveData,
    pub order_byte: u8,
    pub first: MoveBytes,
    pub second: MoveBytes,
    /// Per-mover `side_effect_roll` byte (the LAST per-mover draw, after damage).
    pub first_side_effect: u8,
    pub second_side_effect: u8,
}

impl SecondaryScenario {
    /// Two clean Pikachus, player faster (acts first), always-hit, no secondary
    /// roll fires (side_effect 255). Caller sets `move_data.effect` + the bytes.
    pub fn base(name: &'static str, move_data: MoveData) -> Self {
        Self {
            name,
            player: SecondaryMon::clean(5000, 200),
            enemy: SecondaryMon::clean(5000, 50),
            move_data,
            order_byte: 0,
            first: MoveBytes::always_hit(),
            second: MoveBytes::always_hit(),
            first_side_effect: 255,
            second_side_effect: 255,
        }
    }
}

fn to_move_randoms_se(b: MoveBytes, side_effect: u8) -> MoveRandoms {
    MoveRandoms {
        confusion_roll: b.confusion,
        paralysis_roll: b.paralysis,
        crit_roll: b.crit,
        accuracy_roll: b.accuracy,
        damage_roll: b.damage,
        effect_randoms: EffectRandoms {
            side_effect_roll: side_effect,
            duration_roll: 0,
            multi_hit_roll: 0,
            stat_down_miss_roll: 255,
        },
    }
}

fn poke_secondary(m: &SecondaryMon) -> Pokemon {
    let base = get_base_stats(m.species).expect("species base stats");
    Pokemon {
        species: m.species,
        nickname: [0x50; 11],
        level: 50,
        hp: m.hp,
        max_hp: m.max_hp,
        attack: 100,
        defense: 80,
        speed: m.speed,
        special: 80,
        type1: base.type1,
        type2: base.type2,
        moves: [MoveId::Thundershock, MoveId::None, MoveId::None, MoveId::None],
        pp: [30, 0, 0, 0],
        pp_ups: [0; 4],
        status: m.status,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

fn engine_battler_secondary(m: &SecondaryMon) -> EngineBattler<PocData> {
    let mut stats = EnumMap::new();
    stats.set(Stat::Attack, 100);
    stats.set(Stat::Defense, 80);
    stats.set(Stat::Speed, m.speed);
    stats.set(Stat::Special, 80);
    stats.set(Stat::Level, 50);
    stats.set(Stat::MaxHp, m.max_hp);
    let mut b = EngineBattler::new(m.species, m.hp, m.max_hp, stats, vec![MoveId::Thundershock]);
    if m.status != LegacyStatus::None {
        b.status = Some(m.status);
    }
    b.stat_stages.set(Stat::Special, m.spc_stage);
    b.stat_stages.set(Stat::Attack, m.atk_stage);
    b
}

/// Run the legacy oracle for a [`SecondaryScenario`]: build state with per-side
/// status/stages/flinch, lay bytes (incl. the `side_effect_roll`) into
/// `TurnRandoms`, run `execute_turn` with the scenario's move on BOTH movers.
pub fn legacy_run_secondary(s: &SecondaryScenario) -> LegacyState {
    use super::state::status1;
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![poke_secondary(&s.player)],
        vec![poke_secondary(&s.enemy)],
    );
    state.player.selected_move = s.move_data.id;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = s.move_data.id;
    state.enemy.selected_move_index = 0;
    state.player.stat_stages.special = s.player.spc_stage;
    state.player.stat_stages.attack = s.player.atk_stage;
    state.enemy.stat_stages.special = s.enemy.spc_stage;
    state.enemy.stat_stages.attack = s.enemy.atk_stage;
    if s.player.flinched {
        state.player.set_status1(status1::FLINCHED);
    }
    if s.enemy.flinched {
        state.enemy.set_status1(status1::FLINCHED);
    }

    let first = first_mover_secondary(s);
    // `side_effect_roll` rides per-mover in the SAME field order the stack draws.
    let (first_se, second_se) = match first {
        FirstMover::Player => (s.first_side_effect, s.second_side_effect),
        FirstMover::Opponent => (s.first_side_effect, s.second_side_effect),
    };
    // `TurnRandoms.first_mover`/`second_mover` are keyed by move ORDER, so the
    // first/second side-effect bytes attach to the first/second mover directly.
    let randoms = TurnRandoms {
        order_random: s.order_byte,
        first_mover: to_move_randoms_se(s.first, first_se),
        second_mover: to_move_randoms_se(s.second, second_se),
    };
    execute_turn(&mut state, &s.move_data, &s.move_data, &randoms);
    state
}

fn first_mover_secondary(s: &SecondaryScenario) -> FirstMover {
    let provider = PocData;
    let state = EngineState::new(
        vec![engine_battler_secondary(&s.player)],
        vec![engine_battler_secondary(&s.enemy)],
    );
    let pr = provider.turn_order_rank(&state, BattlerRef::PLAYER, &MoveId::Thundershock);
    let er = provider.turn_order_rank(&state, BattlerRef::OPPONENT, &MoveId::Thundershock);
    match pr.cmp(&er) {
        std::cmp::Ordering::Less => FirstMover::Player,
        std::cmp::Ordering::Greater => FirstMover::Opponent,
        std::cmp::Ordering::Equal => {
            if s.order_byte < 128 {
                FirstMover::Player
            } else {
                FirstMover::Opponent
            }
        }
    }
}

fn order_is_tie_secondary(s: &SecondaryScenario) -> bool {
    let provider = PocData;
    let state = EngineState::new(
        vec![engine_battler_secondary(&s.player)],
        vec![engine_battler_secondary(&s.enemy)],
    );
    let pr = provider.turn_order_rank(&state, BattlerRef::PLAYER, &MoveId::Thundershock);
    let er = provider.turn_order_rank(&state, BattlerRef::OPPONENT, &MoveId::Thundershock);
    pr == er
}

/// Whether the active move HAS a side-effect that reads the `side_effect_roll`
/// (status/stat/flinch shapes). Recoil/Drain/Haze read NO roll → no extra byte.
fn move_draws_side_effect(effect: pokered_data::moves::MoveEffect) -> bool {
    use pokered_data::moves::MoveEffect as ME;
    matches!(
        effect,
        ME::PoisonSideEffect1
            | ME::PoisonSideEffect2
            | ME::SpecialDownSideEffect
            | ME::FlinchSideEffect1
            | ME::FlinchSideEffect2
    )
}

/// The slice-7 consumed-byte predictor for ONE turn. Same crit→accuracy→damage
/// shape as slices 3/5, PLUS the LAST per-mover `side_effect` byte for a
/// side-effect move that HIT. A flinched mover aborts (no draws); a Haze (power-0)
/// move draws only `[para?] acc` (no crit/damage), modeled here.
fn build_secondary_stream(s: &SecondaryScenario, first: FirstMover, tie: bool) -> (Vec<u8>, usize) {
    use pokered_data::moves::MoveEffect as ME;
    let is_haze = s.move_data.effect == ME::HazeEffect;
    let draws_se = move_draws_side_effect(s.move_data.effect);

    let mut bytes = if tie { vec![s.order_byte] } else { Vec::new() };
    let (fa, fb, f_se, sa, sb, s_se) = match first {
        FirstMover::Player => (
            &s.player, s.first, s.first_side_effect, &s.enemy, s.second, s.second_side_effect,
        ),
        FirstMover::Opponent => (
            &s.enemy, s.first, s.first_side_effect, &s.player, s.second, s.second_side_effect,
        ),
    };

    // `flinched_by_first`: did the FIRST mover's secondary land a flinch on the
    // SECOND mover this turn (so the second mover aborts before any draw)?
    let mut flinched_second = sa.flinched; // pre-existing flinch on the 2nd mover

    let push_mover =
        |bytes: &mut Vec<u8>, atk: &SecondaryMon, mb: MoveBytes, se: u8, flinched: bool| -> bool {
            // Flinch gate (order 40) fires before crit; a flinched mover aborts.
            if flinched {
                return false; // aborted, no flinch landed by this mover
            }
            // Status gate: Sleep/Freeze abort (no draw); Paralysis draws + may abort.
            match atk.status {
                LegacyStatus::Sleep(_) | LegacyStatus::Freeze => return false,
                LegacyStatus::Paralysis => {
                    bytes.push(mb.paralysis);
                    if mb.paralysis < 63 {
                        return false; // fully paralyzed → abort, no further draws
                    }
                }
                _ => {}
            }
            if is_haze {
                // Power-0 Haze: only the accuracy byte (no crit/damage).
                bytes.push(mb.accuracy);
                return false; // Haze inflicts no flinch
            }
            // Damaging move: crit, accuracy, [damage iff hit].
            bytes.push(mb.crit);
            bytes.push(mb.accuracy);
            let scaled = (s.move_data.accuracy as u32 * 255 / 100).min(255) as u8;
            let hit = mb.accuracy < scaled;
            if hit {
                bytes.push(mb.damage);
                // Side-effect byte drawn LAST iff the move has a roll-reading
                // secondary (legacy always reads the field on a hit).
                if draws_se {
                    bytes.push(se);
                    // Did a FLINCH land? (only flinch shapes set the flag).
                    let is_flinch = matches!(
                        s.move_data.effect,
                        ME::FlinchSideEffect1 | ME::FlinchSideEffect2
                    );
                    let thr = match s.move_data.effect {
                        ME::FlinchSideEffect1 => 26,
                        ME::FlinchSideEffect2 => 77,
                        _ => 0,
                    };
                    if is_flinch && se < thr {
                        return true; // flinch landed on the defender
                    }
                }
            }
            false
        };

    // The first mover may carry a PRE-EXISTING flinch (set before the turn) → it
    // aborts before any draw, exactly like the stack's `flinch_gate`.
    let first_flinch_landed = push_mover(&mut bytes, fa, fb, f_se, fa.flinched);
    if first_flinch_landed {
        flinched_second = true;
    }
    // The matrix keeps hp high so the move never KOs mid-turn; the second mover
    // acts unless it was flinched (by a landed flinch or a pre-existing flag).
    let _ = push_mover(&mut bytes, sa, sb, s_se, flinched_second);
    let consumed = bytes.len();
    (bytes, consumed)
}

/// Run the stack path for a [`SecondaryScenario`]: set the active move (thread-
/// local), build engine state with per-side status/stages, pre-seed flinch
/// volatiles, stream the bytes in FIRE order, return `(state, consumed, first,
/// effects)`.
pub fn stack_run_secondary(
    s: &SecondaryScenario,
) -> (EngineState<PocData>, usize, FirstMover, Vec<EffectState<PocData>>) {
    let _guard = with_active_move(s.move_data);
    let mut state = EngineState::new(
        vec![engine_battler_secondary(&s.player)],
        vec![engine_battler_secondary(&s.enemy)],
    );
    let mut effects: Vec<EffectState<PocData>> = Vec::new();
    if s.player.flinched {
        effects.push(EffectState {
            id: EFF_FLINCH,
            host: BattlerRef::PLAYER,
            effect_order: 0,
            kind: PocKind::Flinch,
        });
    }
    if s.enemy.flinched {
        effects.push(EffectState {
            id: EffectId(EFF_FLINCH.0 + 1),
            host: BattlerRef::OPPONENT,
            effect_order: 1,
            kind: PocKind::Flinch,
        });
    }
    effects.sort_by_key(|e| e.id);

    let provider = PocData;
    let move_id = s.move_data.id;
    let actions = [
        BattleAction::<PocData>::Fight { move_: move_id },
        BattleAction::<PocData>::Fight { move_: move_id },
    ];
    let first = first_mover_secondary(s);
    let tie = order_is_tie_secondary(s);
    let (bytes, _expected) = build_secondary_stream(s, first, tie);
    let mut rng = ScriptedRng::new(bytes);
    let result = StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
    assert_eq!(result.first, first, "[{}] first-mover probe disagreed", s.name);
    (state, rng.consumed(), first, effects)
}

/// Read the stack's Special stat stage for `who`.
pub fn stack_spc_stage(stack: &EngineState<PocData>, who: BattlerRef) -> i8 {
    let b = if who.side == 0 {
        &stack.player_battlers[who.slot as usize]
    } else {
        &stack.opponent_battlers[who.slot as usize]
    };
    b.stat_stages.get(Stat::Special).copied().unwrap_or(0)
}

/// Read the stack's Attack stat stage for `who`.
pub fn stack_atk_stage(stack: &EngineState<PocData>, who: BattlerRef) -> i8 {
    let b = if who.side == 0 {
        &stack.player_battlers[who.slot as usize]
    } else {
        &stack.opponent_battlers[who.slot as usize]
    };
    b.stat_stages.get(Stat::Attack).copied().unwrap_or(0)
}

/// Whether the stack still hosts a Flinch volatile for `who`.
pub fn stack_flinched(effects: &[EffectState<PocData>], who: BattlerRef) -> bool {
    effects.iter().any(|e| e.host == who && matches!(e.kind, PocKind::Flinch))
}

/// The full slice-7 assertion: run BOTH paths, assert `BattleState` (both sides'
/// hp + status), the stat stages (Special + Attack, both sides), the flinch state
/// (legacy `status1::FLINCHED` vs the stack `PocKind::Flinch`), AND `consumed()`.
pub fn run_scenario_secondary(s: &SecondaryScenario) {
    use super::state::status1;
    let legacy = legacy_run_secondary(s);
    let (stack, consumed, first, effects) = stack_run_secondary(s);

    // 1. hp + non-volatile status, both sides.
    let lp = legacy.player.active_mon();
    let le = legacy.enemy.active_mon();
    let sp = &stack.player_battlers[0];
    let se = &stack.opponent_battlers[0];
    assert_eq!(lp.hp, sp.hp, "[{}] PLAYER hp: legacy={} stack={}", s.name, lp.hp, sp.hp);
    assert_eq!(le.hp, se.hp, "[{}] ENEMY hp: legacy={} stack={}", s.name, le.hp, se.hp);
    assert_eq!(
        lp.status,
        sp.status.unwrap_or(LegacyStatus::None),
        "[{}] PLAYER status",
        s.name
    );
    assert_eq!(
        le.status,
        se.status.unwrap_or(LegacyStatus::None),
        "[{}] ENEMY status",
        s.name
    );

    // 2. Special + Attack stat stages, both sides (stat-drop / Haze reset).
    assert_eq!(
        legacy.player.stat_stages.special,
        stack_spc_stage(&stack, BattlerRef::PLAYER),
        "[{}] PLAYER special stage",
        s.name
    );
    assert_eq!(
        legacy.enemy.stat_stages.special,
        stack_spc_stage(&stack, BattlerRef::OPPONENT),
        "[{}] ENEMY special stage",
        s.name
    );
    assert_eq!(
        legacy.player.stat_stages.attack,
        stack_atk_stage(&stack, BattlerRef::PLAYER),
        "[{}] PLAYER attack stage",
        s.name
    );
    assert_eq!(
        legacy.enemy.stat_stages.attack,
        stack_atk_stage(&stack, BattlerRef::OPPONENT),
        "[{}] ENEMY attack stage",
        s.name
    );

    // 3. Flinch state, both sides (legacy clears FLINCHED on the check; the stack
    //    consumes the volatile in `flinch_gate`, so a flinched-then-acted mon ends
    //    with NO flinch in BOTH paths).
    assert_eq!(
        legacy.player.has_status1(status1::FLINCHED),
        stack_flinched(&effects, BattlerRef::PLAYER),
        "[{}] PLAYER flinch state",
        s.name
    );
    assert_eq!(
        legacy.enemy.has_status1(status1::FLINCHED),
        stack_flinched(&effects, BattlerRef::OPPONENT),
        "[{}] ENEMY flinch state",
        s.name
    );

    // 4. consumed() parity (draw-order drift detector incl. the side_effect byte).
    let (_b, expected) = build_secondary_stream(s, first, order_is_tie_secondary(s));
    assert_eq!(
        consumed, expected,
        "[{}] stack consumed {} bytes, expected {} (draw-order drift!)",
        s.name, consumed, expected
    );
}

// ════════════════════════════════════════════════════════════════════════════
// P0 — the PRODUCTION RNG SHIM + the two-mover + AI differential harness.
//
// ## Why this exists (the migration prerequisite, blueprint `15` §4 / §5 P0)
//
// Every slice above (1-7) drives a two-mover turn whose enemy move is FIXED
// (`MoveId::Thundershock` on both sides). That proves the move/residual/volatile
// pipeline, but it NEVER exercises the one draw the production loop interleaves
// that the slices skipped: the AI `pick_enemy_move` `rand::random()`.
//
// The production live loop (`mod.rs::execute_turn_with_move`, the frame-stepped
// path that is the REAL battle, NOT the dead `turn::execute_turn` oracle) draws
// its RNG in this exact ordinal:
//
//   1. `pick_enemy_move(bs, trainer_class)`  (mod.rs:1766) — drawn FIRST:
//        * trainer w/ non-empty layers: `result.pick_move(rand::random::<u8>())`
//          (mod.rs:794) — ONE byte; on a valid pick it RETURNS here.
//        * else / on a None or invalid pick: `rand::random::<usize>() % len`
//          (mod.rs:803) — the wild/fallback draw.
//   2. `generate_turn_randoms()`             (mod.rs:1784) — the pre-roll:
//        order_random, then first_mover MoveRandoms (confusion, paralysis, crit,
//        accuracy, damage + EffectRandoms), then second_mover MoveRandoms.
//   3. `determine_order` + `execute_move`(×2) consume the pre-rolled struct.
//
// ## The HAZARD the blueprint flagged — and the FINDING this harness proves
//
// Blueprint `15` §4 / §6 risk #1 hypothesized the AI draw happens "BETWEEN the
// pre-roll and execution" and warned it "can silently desync every turn". The
// code (mod.rs:1766 vs 1784) shows the AI draw is strictly BEFORE the whole
// pre-roll — a clean prefix, NOT interleaved mid-`TurnRandoms`. That is the
// MIGRATION-SAFE outcome: a prefixed draw IS reproducible by a streamed RNG.
// This harness PROVES it: it lays the AI byte(s) at the FRONT of one shared byte
// vector, draws them (via the SAME re-homed AI code) on BOTH the legacy-oracle
// path and the stack path, then runs the rest of the turn on the remaining
// bytes, and asserts IDENTICAL resulting BattleState AND identical consumed().
//
// ## Faithfulness scope (honest note)
//
// Production draws the AI via `rand::random()` of the AI byte's WIDTH (`u8` for
// the trainer pick; `usize` for the wild fallback). The harness routes BOTH the
// legacy and stack AI draws through the SAME `harness_pick_enemy_move` over the
// SHARED `BattleRng`, so the picked move AND the AI byte-count are identical on
// the two paths BY CONSTRUCTION — the property P0 must prove (the shared stream's
// ordinal reproduces). The harness models the wild fallback as ONE `range(len)`
// byte (the engine's `range` is `byte % len`, matching `rand::random::<usize>()
// % len`'s reduction); the production `usize` entropy width is irrelevant to the
// ordinal claim because both harness paths consume the SAME shim. The re-home
// uses the REAL AI code (`move_choice_layers` / `choose_moves` / `pick_move`),
// not a reimplementation, exactly like every other re-home in this harness.
// ════════════════════════════════════════════════════════════════════════════

use super::trainer_ai::move_choice::choose_moves as ai_choose_moves;
use super::trainer_ai::move_choice_layers as ai_move_choice_layers;
use dotzuki_engine::battle::rng::BattleRng;
use pokered_data::trainer_data::TrainerClass;

/// Re-home of the production `BattleScreen::pick_enemy_move` (`mod.rs:777-805`),
/// drawing from a `BattleRng` stream instead of `rand::random()`. The control
/// flow + the AI building blocks (`move_choice_layers` / `choose_moves` /
/// `MoveChoiceResult::pick_move`) are the PRODUCTION ones — only the entropy
/// source is swapped (the harness invariant: re-home, don't reimplement).
///
/// Draw structure (the production ordinal, the P0 crux):
///   * no available moves → `Struggle`, draws NOTHING (production mod.rs:786-788);
///   * trainer w/ non-empty layers → `pick_move(rng.next_u8())` = ONE byte; a
///     valid pick RETURNS here (mod.rs:790-800);
///   * else → the fallback `range(len)` = ONE byte (mod.rs:803).
///
/// Returns `(move_id, move_index)` exactly like production.
pub fn harness_pick_enemy_move(
    bs: &LegacyState,
    trainer_class: Option<TrainerClass>,
    rng: &mut dyn BattleRng,
) -> (MoveId, u8) {
    let mon = bs.enemy.active_mon();
    let available: Vec<(MoveId, u8)> = mon
        .moves
        .iter()
        .enumerate()
        .filter(|(i, m)| **m != MoveId::None && mon.pp[*i] > 0)
        .map(|(i, m)| (*m, i as u8))
        .collect();
    if available.is_empty() {
        return (MoveId::Struggle, 0); // no draw (matches mod.rs:786-788)
    }

    if let Some(tc) = trainer_class {
        let layers = ai_move_choice_layers(tc);
        if !layers.is_empty() {
            // `choose_moves` draws NO rng (pure scoring) — same as production.
            let result = ai_choose_moves(layers, &bs.enemy, &bs.player, 0);
            // The single trainer AI draw (mod.rs:794).
            if let Some(slot) = result.pick_move(rng.next_u8()) {
                let move_id = mon.moves[slot];
                if move_id != MoveId::None && mon.pp[slot] > 0 {
                    return (move_id, slot as u8); // valid pick → RETURN here
                }
            }
            // else: fall through to the fallback (production mod.rs:801-804).
        }
    }

    // The wild / fallback draw (mod.rs:803). `range(len)` == `byte % len`, the
    // same reduction `rand::random::<usize>() % len` performs.
    let idx = rng.range(available.len() as u32) as usize;
    available[idx]
}

/// Predict how many bytes `harness_pick_enemy_move` will draw for a scenario,
/// for the consumed() predictor. Mirrors the control flow exactly (no rng).
fn ai_draw_count(
    enemy_moves: [MoveId; 4],
    enemy_pp: [u8; 4],
    trainer_class: Option<TrainerClass>,
    ai_byte: u8,
    bs: &LegacyState,
) -> usize {
    let available: Vec<usize> = enemy_moves
        .iter()
        .enumerate()
        .filter(|(i, m)| **m != MoveId::None && enemy_pp[*i] > 0)
        .map(|(i, _)| i)
        .collect();
    if available.is_empty() {
        return 0; // Struggle, no draw
    }
    if let Some(tc) = trainer_class {
        let layers = ai_move_choice_layers(tc);
        if !layers.is_empty() {
            let result = ai_choose_moves(layers, &bs.enemy, &bs.player, 0);
            if let Some(slot) = result.pick_move(ai_byte) {
                let move_id = enemy_moves[slot];
                if move_id != MoveId::None && enemy_pp[slot] > 0 {
                    return 1; // trainer pick drew one byte and returned
                }
            }
            return 1 + 1; // pick byte + fallback byte (None / invalid pick)
        }
    }
    1 // fallback only
}

/// Per-side mon config for an [`AiScenario`]. The ENEMY carries a FULL 4-move
/// set + pp (so the AI has real choices to pick among); the PLAYER's move is
/// fixed (the human's selection in production).
#[derive(Clone, Copy)]
pub struct AiMonSpec {
    pub species: Species,
    pub hp: u16,
    pub speed: u16,
    pub status: LegacyStatus,
    pub moves: [MoveId; 4],
    pub pp: [u8; 4],
}

impl AiMonSpec {
    /// A clean Pikachu with a single Thundershock (pp 30), the others empty.
    pub fn solo(hp: u16, speed: u16) -> Self {
        Self {
            species: Species::Pikachu,
            hp,
            speed,
            status: LegacyStatus::None,
            moves: [MoveId::Thundershock, MoveId::None, MoveId::None, MoveId::None],
            pp: [30, 0, 0, 0],
        }
    }
    /// A clean Pikachu with a multi-move set (for real AI choice).
    pub fn multi(hp: u16, speed: u16, moves: [MoveId; 4], pp: [u8; 4]) -> Self {
        Self { species: Species::Pikachu, hp, speed, status: LegacyStatus::None, moves, pp }
    }
}

/// A P0 two-mover + AI scenario: the player picks `player_move` (the human
/// selection); the ENEMY move is chosen by the REAL AI (`harness_pick_enemy_move`)
/// drawing the AI byte FIRST from the shared stream. Both paths (legacy
/// `execute_turn` oracle + `StackDriver`) run on the SAME byte vector and must
/// agree on resulting state AND consumed().
#[derive(Clone)]
pub struct AiScenario {
    pub name: &'static str,
    pub player: AiMonSpec,
    pub enemy: AiMonSpec,
    /// The player's chosen move (production: the human's menu selection).
    pub player_move: MoveId,
    /// `None` = wild battle (fallback AI draw); `Some(tc)` = trainer (AI pick).
    pub trainer_class: Option<TrainerClass>,
    /// The AI byte drawn FIRST (production ordinal: before the pre-roll).
    pub ai_byte: u8,
    /// `order_random` (drawn after the AI byte; used on a tie).
    pub order_byte: u8,
    /// Per-mover bytes in `MoveRandoms` field order.
    pub first: MoveBytes,
    pub second: MoveBytes,
}

/// Build a legacy [`Pokemon`] for an [`AiMonSpec`] (full move set + pp).
fn poke_ai(m: &AiMonSpec) -> Pokemon {
    let base = get_base_stats(m.species).expect("species base stats");
    Pokemon {
        species: m.species,
        nickname: [0x50; 11],
        level: 50,
        hp: m.hp,
        max_hp: m.hp,
        attack: 100,
        defense: 80,
        speed: m.speed,
        special: 80,
        type1: base.type1,
        type2: base.type2,
        moves: m.moves,
        pp: m.pp,
        pp_ups: [0; 4],
        status: m.status,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

fn engine_battler_ai(m: &AiMonSpec) -> EngineBattler<PocData> {
    let mut stats = EnumMap::new();
    stats.set(Stat::Attack, 100);
    stats.set(Stat::Defense, 80);
    stats.set(Stat::Speed, m.speed);
    stats.set(Stat::Special, 80);
    stats.set(Stat::Level, 50);
    stats.set(Stat::MaxHp, m.hp);
    let mut b = EngineBattler::new(m.species, m.hp, m.hp, stats, m.moves.to_vec());
    if m.status != LegacyStatus::None {
        b.status = Some(m.status);
    }
    b
}

/// Build the base legacy `BattleState` for an [`AiScenario`] (no move selected
/// yet — the AI step sets the enemy's, the caller sets the player's).
fn ai_base_state(s: &AiScenario) -> LegacyState {
    new_battle_state(
        BattleType::Wild,
        vec![poke_ai(&s.player)],
        vec![poke_ai(&s.enemy)],
    )
}

/// Run the LEGACY oracle path for an [`AiScenario`], reproducing the PRODUCTION
/// draw ordinal exactly:
///   1. draw the AI byte(s) FIRST via `harness_pick_enemy_move` (from the shared
///      stream) → the enemy's selected move;
///   2. lay the REMAINING bytes into a `TurnRandoms` struct (order + the two
///      MoveRandoms, pokered field order);
///   3. call `execute_turn` (the atomic two-mover oracle, untouched).
/// Returns `(final_state, ai_bytes_consumed, enemy_move)`.
pub fn legacy_run_ai(s: &AiScenario) -> (LegacyState, usize, MoveId) {
    let mut state = ai_base_state(s);

    // ── Step 1: the AI draw, FIRST, from the shared stream. ──
    let mut ai_rng = ScriptedRng::new(vec![s.ai_byte, s.ai_byte]); // ≤2 AI bytes
    let (enemy_move_id, enemy_move_idx) =
        harness_pick_enemy_move(&state, s.trainer_class, &mut ai_rng);
    let ai_consumed = ai_rng.consumed();

    // ── Step 2/3: set both movers, lay the pre-roll, run the oracle. ──
    state.player.selected_move = s.player_move;
    state.player.selected_move_index = player_move_index(&s.player, s.player_move);
    state.enemy.selected_move = enemy_move_id;
    state.enemy.selected_move_index = enemy_move_idx;

    // Both movers run the SAME synthetic damaging-move spec (`SLICE12_MOVE`,
    // power 40 / Electric / acc 100 / NoAdditionalEffect) the existing slices use
    // — NOT the real `MoveData::get` table entry (whose `effect`/power differ).
    // The AI-chosen MoveId governs ONLY `selected_move` (turn-order priority +
    // the chosen index), exactly as the stack path uses it; the per-hit math is
    // the synthetic spec on BOTH paths, so damage parity holds by construction.
    let md = SLICE12_MOVE;

    let randoms = TurnRandoms {
        order_random: s.order_byte,
        first_mover: to_move_randoms(s.first),
        second_mover: to_move_randoms(s.second),
    };
    execute_turn(&mut state, &md, &md, &randoms);
    (state, ai_consumed, enemy_move_id)
}

fn player_move_index(m: &AiMonSpec, mv: MoveId) -> u8 {
    m.moves.iter().position(|x| *x == mv).unwrap_or(0) as u8
}

/// Run the STACK path for an [`AiScenario`], reproducing the SAME draw ordinal:
///   1. draw the AI byte(s) FIRST via the SAME `harness_pick_enemy_move` (from
///      the shared stream) → the enemy's `BattleAction`;
///   2. feed the REMAINING bytes to a `ScriptedRng` in FIRE order;
///   3. run `StackDriver::execute_turn` with `[player_action, enemy_action]`.
/// The single shared `BattleRng` is the same vector for both AI + pre-roll, so
/// the AI draw and the turn draws are one continuous stream (the production
/// `rand` stream's faithful replay). Returns `(state, total_consumed, first)`.
pub fn stack_run_ai(s: &AiScenario) -> (EngineState<PocData>, usize, FirstMover, MoveId) {
    // The stack uses the per-scenario move for BOTH movers' damage math via the
    // thread-local active move. The P0 AI scenarios use a single damaging-move
    // spec (the active-move shim carries power/type/acc); the enemy's CHOSEN move
    // id only affects which move the AI returns (turn-order priority + the chosen
    // index), not the per-hit damage formula, which the active-move shim governs.
    let mut state = EngineState::new(
        vec![engine_battler_ai(&s.player)],
        vec![engine_battler_ai(&s.enemy)],
    );
    let mut effects: Vec<EffectState<PocData>> = Vec::new();

    // ── Step 1: the AI draw, FIRST, on the SHARED stream. Build the full byte
    //    vector = [AI bytes..] ++ [pre-roll/fire-order bytes..], then drive ONE
    //    ScriptedRng through it so the AI + turn draws are one continuous stream.
    let legacy_for_ai = ai_base_state(s); // same data the legacy path reads
    let ai_bytes = ai_byte_vec(s, &legacy_for_ai);
    let ai_count = ai_bytes.len();
    // Re-draw the AI move via the shim to get the enemy action (deterministic on
    // the same bytes the legacy path drew).
    let mut ai_probe = ScriptedRng::new(ai_bytes.clone());
    let (enemy_move_id, _eidx) =
        harness_pick_enemy_move(&legacy_for_ai, s.trainer_class, &mut ai_probe);
    debug_assert_eq!(ai_probe.consumed(), ai_count, "AI probe drew unexpected count");

    let _guard = with_active_move(ai_active_move(s));

    let first = first_mover_ai(s, enemy_move_id);
    let tie = order_is_tie_ai(s, enemy_move_id);
    let (turn_bytes, _turn_expected) = build_ai_turn_stream(s, first, tie);

    // The one shared stream: AI prefix ++ turn fire-order bytes.
    let mut full = ai_bytes;
    full.extend(turn_bytes);
    let mut rng = ScriptedRng::new(full);

    // Re-consume the AI prefix on the SAME rng (production draws it from the same
    // stream before the pre-roll). The shim's draw advances `rng` exactly as
    // production's `pick_enemy_move` advances the global `rand` stream.
    let (probe2, eidx2) = harness_pick_enemy_move(&legacy_for_ai, s.trainer_class, &mut rng);
    debug_assert_eq!(probe2, enemy_move_id, "AI re-draw disagreed on the shared stream");

    let provider = PocData;
    let actions = [
        BattleAction::<PocData>::Fight { move_: s.player_move },
        BattleAction::<PocData>::Fight { move_: enemy_move_id },
    ];
    let _ = eidx2;
    let result = StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
    assert_eq!(result.first, first, "[{}] first-mover probe disagreed", s.name);
    (state, rng.consumed(), first, enemy_move_id)
}

/// The byte vector the AI draw consumes for a scenario (the production prefix).
fn ai_byte_vec(s: &AiScenario, bs: &LegacyState) -> Vec<u8> {
    let n = ai_draw_count(s.enemy.moves, s.enemy.pp, s.trainer_class, s.ai_byte, bs);
    vec![s.ai_byte; n]
}

/// The active move spec the stack uses for the per-hit damage math in P0 AI
/// scenarios: a 100%-accuracy damaging move (the same shape the legacy oracle
/// uses for `MoveId::Thundershock`). Carried via the thread-local active-move shim.
fn ai_active_move(_s: &AiScenario) -> MoveData {
    SLICE12_MOVE
}

/// First mover for an [`AiScenario`] given the AI-chosen enemy move (RNG-free
/// rank using each mover's chosen move's priority; tie → `order_byte < 128`).
fn first_mover_ai(s: &AiScenario, enemy_move: MoveId) -> FirstMover {
    let provider = PocData;
    let state = EngineState::new(
        vec![engine_battler_ai(&s.player)],
        vec![engine_battler_ai(&s.enemy)],
    );
    let pr = provider.turn_order_rank(&state, BattlerRef::PLAYER, &s.player_move);
    let er = provider.turn_order_rank(&state, BattlerRef::OPPONENT, &enemy_move);
    match pr.cmp(&er) {
        std::cmp::Ordering::Less => FirstMover::Player,
        std::cmp::Ordering::Greater => FirstMover::Opponent,
        std::cmp::Ordering::Equal => {
            if s.order_byte < 128 {
                FirstMover::Player
            } else {
                FirstMover::Opponent
            }
        }
    }
}

fn order_is_tie_ai(s: &AiScenario, enemy_move: MoveId) -> bool {
    let provider = PocData;
    let state = EngineState::new(
        vec![engine_battler_ai(&s.player)],
        vec![engine_battler_ai(&s.enemy)],
    );
    let pr = provider.turn_order_rank(&state, BattlerRef::PLAYER, &s.player_move);
    let er = provider.turn_order_rank(&state, BattlerRef::OPPONENT, &enemy_move);
    pr == er
}

/// Predict the turn (non-AI) fire-order stream + count for an [`AiScenario`].
/// Same control flow as [`build_stack_stream`] (status gate → crit → accuracy →
/// damage), specialized to the P0 AI scenarios (no confusion; paralysis only).
fn build_ai_turn_stream(s: &AiScenario, first: FirstMover, tie: bool) -> (Vec<u8>, usize) {
    let mut bytes = if tie { vec![s.order_byte] } else { Vec::new() };
    let (first_status, first_bytes, second_status, second_bytes) = match first {
        FirstMover::Player => (s.player.status, s.first, s.enemy.status, s.second),
        FirstMover::Opponent => (s.enemy.status, s.first, s.player.status, s.second),
    };
    let push_mover = |bytes: &mut Vec<u8>, status: LegacyStatus, mb: MoveBytes| -> bool {
        match status {
            LegacyStatus::Sleep(_) => return true, // abort, no draw
            LegacyStatus::Freeze => return true,
            LegacyStatus::Paralysis => {
                bytes.push(mb.paralysis);
                if mb.paralysis < 63 {
                    return true; // fully paralyzed, abort
                }
            }
            _ => {}
        }
        bytes.push(mb.crit);
        bytes.push(mb.accuracy);
        let scaled = (SLICE12_MOVE.accuracy as u32 * 255 / 100).min(255) as u8;
        if mb.accuracy < scaled {
            bytes.push(mb.damage);
        }
        false
    };
    let _first_aborted = push_mover(&mut bytes, first_status, first_bytes);
    // The matrix keeps both above 30 HP (no mid-turn KO) so the second mover
    // acts; the faint-short-circuit AI scenario asserts consumed() directly.
    let second_acts = s.player.hp > 30 && s.enemy.hp > 30;
    if second_acts {
        push_mover(&mut bytes, second_status, second_bytes);
    }
    let consumed = bytes.len();
    (bytes, consumed)
}

/// The full P0 two-mover + AI assertion: run BOTH paths on the SAME shared byte
/// vector (AI prefix ++ turn fire order), assert
///   * IDENTICAL resulting `BattleState` (both sides hp + status), AND
///   * the SAME enemy move was AI-chosen on both paths, AND
///   * IDENTICAL total consumed (AI byte count + turn byte count) — the draw
///     ordinal / interleave proof.
pub fn run_scenario_ai(s: &AiScenario) {
    let (legacy, legacy_ai_consumed, legacy_enemy_move) = legacy_run_ai(s);
    let (stack, stack_consumed, first, stack_enemy_move) = stack_run_ai(s);

    // 1. The AI chose the SAME move on both paths (the shared-stream proof).
    assert_eq!(
        legacy_enemy_move, stack_enemy_move,
        "[{}] AI picked different moves: legacy={:?} stack={:?} (shared-stream desync!)",
        s.name, legacy_enemy_move, stack_enemy_move
    );

    // 2. Resulting BattleState parity: both sides hp + status.
    let lp = legacy.player.active_mon();
    let le = legacy.enemy.active_mon();
    let sp = &stack.player_battlers[0];
    let se = &stack.opponent_battlers[0];
    assert_eq!(lp.hp, sp.hp, "[{}] PLAYER hp: legacy={} stack={}", s.name, lp.hp, sp.hp);
    assert_eq!(le.hp, se.hp, "[{}] ENEMY hp: legacy={} stack={}", s.name, le.hp, se.hp);
    assert_eq!(
        lp.status,
        sp.status.unwrap_or(LegacyStatus::None),
        "[{}] PLAYER status",
        s.name
    );
    assert_eq!(
        le.status,
        se.status.unwrap_or(LegacyStatus::None),
        "[{}] ENEMY status",
        s.name
    );

    // 3. consumed() parity — the AI-draw INTERLEAVE proof. The stack's TOTAL
    //    draw count (AI prefix + turn) must equal the predicted AI count + the
    //    predicted turn count. A drift in EITHER (the AI byte at the wrong
    //    ordinal, or a turn byte miscounted because the AI prefix shifted the
    //    stream) makes this fail.
    let bs_for_ai = ai_base_state(s);
    let expected_ai =
        ai_draw_count(s.enemy.moves, s.enemy.pp, s.trainer_class, s.ai_byte, &bs_for_ai);
    let tie = order_is_tie_ai(s, stack_enemy_move);
    let (_b, expected_turn) = build_ai_turn_stream(s, first, tie);
    let expected_total = expected_ai + expected_turn;
    assert_eq!(
        stack_consumed, expected_total,
        "[{}] stack consumed {} bytes, expected {} ({} AI + {} turn) — AI-draw \
         interleave drift!",
        s.name, stack_consumed, expected_total, expected_ai, expected_turn
    );
    // The legacy AI step drew exactly the same AI count (both call the shim).
    assert_eq!(
        legacy_ai_consumed, expected_ai,
        "[{}] legacy AI step consumed {} bytes, expected {} — AI-draw count \
         disagreement between legacy and stack",
        s.name, legacy_ai_consumed, expected_ai
    );
}

// ── Public wrappers so the sibling P0 test module can pin the draw-order
//    invariants directly (the order byte / crit-before-accuracy / inert count).

/// Public wrapper over [`first_mover_ai`] (the RNG-free first-mover rank).
pub fn first_mover_ai_pub(s: &AiScenario, enemy_move: MoveId) -> FirstMover {
    first_mover_ai(s, enemy_move)
}

/// Public wrapper over [`order_is_tie_ai`] (exact turn-order rank tie).
pub fn order_is_tie_ai_pub(s: &AiScenario, enemy_move: MoveId) -> bool {
    order_is_tie_ai(s, enemy_move)
}

/// Public wrapper over [`build_ai_turn_stream`] (the post-AI fire-order stream).
pub fn build_ai_turn_stream_pub(s: &AiScenario, first: FirstMover, tie: bool) -> (Vec<u8>, usize) {
    build_ai_turn_stream(s, first, tie)
}

/// Public wrapper over [`ai_draw_count`]: predicts the AI prefix byte count for a
/// scenario (builds the base legacy state the AI scorer reads).
pub fn ai_draw_count_pub(
    enemy_moves: [MoveId; 4],
    enemy_pp: [u8; 4],
    trainer_class: Option<TrainerClass>,
    ai_byte: u8,
    s: &AiScenario,
) -> usize {
    let bs = ai_base_state(s);
    ai_draw_count(enemy_moves, enemy_pp, trainer_class, ai_byte, &bs)
}

