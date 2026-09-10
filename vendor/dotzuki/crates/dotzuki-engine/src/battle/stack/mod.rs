//! Showdown-style **effect-stack** battle engine (Pattern C) — vertical-slice
//! POC (design doc `06-battle-engine-effect-stack-design.md`, §8).
//!
//! This is an **additive sibling** to [`crate::battle::driver`]; the existing
//! `BattleDriver` and every other game's code keep compiling unchanged. The
//! module is **100% game-agnostic**: no game-specific concrete types, no
//! `rand` (randomness only via the existing [`BattleRng`](crate::battle::rng)).
//!
//! ## Pieces
//!
//! * [`event`] — the closed [`Event`] taxonomy, [`RelayVar`]/[`HandlerResult`],
//!   the `fn`-pointer [`HandlerFn`] signature, and the [`Effect`]/[`EventHook`]
//!   registration shape.
//! * [`ctx`] — the [`EffectProvider`] trait, the typed [`EffectState`] arena,
//!   per-move scratch, and the split-borrow [`BattleCtx`] (`battler_mut` /
//!   `pair_mut` (both branches) / `effect_mut`).
//! * [`dispatch`] — the Showdown `comparePriority` comparator, the speed-tie
//!   draw, and the [`run_event`](dispatch::run_event) fold.
//! * [`driver`] — the [`StackDriver`](driver::StackDriver) firing the fixed §2
//!   per-turn sequence with the per-mover residual + first-mover-faint
//!   short-circuit.

#[macro_use]
pub mod authoring;
pub mod ctx;
pub mod dispatch;
pub mod driver;
pub mod event;
pub mod log;

pub use ctx::{BattleCtx, EffectHost, EffectProvider, EffectState, MoveContext};
pub use dispatch::{collect_handlers, compare, run_event, run_event_checked, CollectedHandler};
pub use driver::{FirstMover, StackDriver, StackTurnResult};
pub use event::{
    Effect, EffectId, EffectType, Event, EventHook, HandlerFn, HandlerResult, RelayVar,
};
pub use log::{HpChangeCause, TurnEvent, TurnLog};

#[cfg(test)]
pub(crate) mod tests_support;

// ─── Engine unit tests (design §8: prove the three structural risks) ─────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::rng::ScriptedRng;
    use crate::battle::{
        BattleProvider, BattleState, BattlerRef, BattlerState, DamageResult, EffectResult, EnumMap,
        MoveEffect,
    };

    // ── A tiny game provider for the engine-side compile/run proofs ──────

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TStat {
        Hp,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[allow(dead_code)] // variants exist to satisfy the trait's assoc-type shape
    enum TStatus {
        Poisoned,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[allow(dead_code)]
    enum TType {
        N,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TSpecies {
        Mon,
    }
    #[derive(Debug, Clone, PartialEq)]
    struct TMove {
        power: u8,
    }

    /// The game-supplied typed effect-state enum (design §3.1).
    #[derive(Clone)]
    #[allow(dead_code)] // `None` is the inert variant of the typed-state shape
    enum TKind {
        None,
        Toxic { counter: u8 },
    }

    struct TProvider;

    impl BattleProvider for TProvider {
        type Monster = ();
        type Move = TMove;
        type Ability = ();
        type Status = TStatus;
        type Stat = TStat;
        type Species = TSpecies;
        type Type = TType;
        type Item = ();

        fn calculate_damage(
            &self,
            move_: &Self::Move,
            _a: &BattlerState<Self>,
            _d: &BattlerState<Self>,
            _r: u8,
            _c: bool,
        ) -> DamageResult {
            DamageResult {
                damage: move_.power as u16,
                effectiveness: 1.0,
                is_miss: false,
            }
        }
        fn select_move(&self, b: &BattlerState<Self>, _s: &BattleState<Self>) -> Self::Move {
            b.moves.first().cloned().unwrap()
        }
        fn apply_move_effect(
            &self,
            _e: MoveEffect,
            _u: &mut BattlerState<Self>,
            _t: &mut BattlerState<Self>,
        ) -> EffectResult {
            EffectResult::NoEffect
        }
        fn create_monster(&self, s: Self::Species, _l: u8) -> BattlerState<Self> {
            BattlerState::new(s, 100, 100, EnumMap::new(), vec![])
        }
    }

    impl EffectProvider for TProvider {
        type EffectStateKind = TKind;
        fn effect_for_move(&self, _m: &Self::Move) -> Option<&'static Effect<Self>> {
            None
        }
        fn effect_for_status(&self, _s: &Self::Status) -> Option<&'static Effect<Self>> {
            None
        }
        fn turn_order_rank(
            &self,
            _state: &BattleState<Self>,
            _who: BattlerRef,
            _action: &Self::Move,
        ) -> (i32, i32) {
            (0, 0) // engine unit tests do not exercise turn order
        }
    }

    fn mon(hp: u16) -> BattlerState<TProvider> {
        let mut stats = EnumMap::new();
        stats.set(TStat::Hp, hp);
        BattlerState::new(TSpecies::Mon, hp, hp, stats, vec![TMove { power: 10 }])
    }

    // ── Handlers used by the proofs (zero-capture fn pointers) ──────────

    /// A `ModifyDamage`-shaped handler: deals `relay` int as damage to target.
    fn deal_5<P: EffectProvider<EffectStateKind = TKind> + ?Sized>(
        ctx: &mut BattleCtx<'_, P>,
        _relay: RelayVar,
        target: BattlerRef,
        _source: BattlerRef,
        _eff: EffectId,
    ) -> HandlerResult {
        ctx.battler_mut(target).take_damage(5);
        HandlerResult::Unchanged
    }

    /// A second handler in the same fold — proves the fold chains multiple
    /// handlers and threads the relay.
    fn add_int<P: EffectProvider + ?Sized>(
        _ctx: &mut BattleCtx<'_, P>,
        relay: RelayVar,
        _t: BattlerRef,
        _s: BattlerRef,
        _e: EffectId,
    ) -> HandlerResult {
        let n = match relay {
            RelayVar::Int(n) => n,
            _ => 0,
        };
        HandlerResult::Set(RelayVar::Int(n + 1))
    }

    /// A **Counter-shaped** handler (design §8): mutate `target` while READING
    /// `source`'s host — the proof that `pair_mut` lets a handler touch both
    /// battlers with NO `RefCell`/`Rc`. It deals damage to `target` equal to
    /// `source`'s current hp / 10.
    fn counter_shaped<P: EffectProvider + ?Sized>(
        ctx: &mut BattleCtx<'_, P>,
        _relay: RelayVar,
        target: BattlerRef,
        source: BattlerRef,
        _eff: EffectId,
    ) -> HandlerResult {
        let (tgt, src) = ctx.pair_mut(target, source);
        let reflected = src.hp / 10; // READ source's host
        tgt.take_damage(reflected); // WRITE target
        HandlerResult::Unchanged
    }

    fn make_ctx_parts(
        hp_a: u16,
        hp_b: u16,
    ) -> (
        BattleState<TProvider>,
        Vec<EffectState<TProvider>>,
        MoveContext,
    ) {
        let state = BattleState::new(vec![mon(hp_a)], vec![mon(hp_b)]);
        (state, Vec::new(), MoveContext::default())
    }

    // ── BORROW proof 1: run_event fold compiles & runs, threads relay ──

    #[test]
    fn fold_dispatch_runs_and_threads_relay() {
        let (mut state, mut effects, mut mv) = make_ctx_parts(100, 100);
        let mut rng = ScriptedRng::new(vec![0]);
        let mut ctx = BattleCtx {
            state: &mut state,
            effects: &mut effects,
            mv: &mut mv,
            rng: &mut rng,
        };
        // Two handlers, both targeting the opponent; the second reads & bumps
        // the int relay set by nothing (starts at Int(0)).
        let hs = vec![
            CollectedHandler {
                order: 1,
                priority: 0,
                speed: 0,
                sub_order: 6,
                effect_order: 0,
                target: BattlerRef::OPPONENT,
                source: BattlerRef::PLAYER,
                source_effect: EffectId(1),
                call: add_int::<TProvider>,
            },
            CollectedHandler {
                order: 2,
                priority: 0,
                speed: 0,
                sub_order: 6,
                effect_order: 1,
                target: BattlerRef::OPPONENT,
                source: BattlerRef::PLAYER,
                source_effect: EffectId(2),
                call: deal_5::<TProvider>,
            },
        ];
        let out = run_event(&mut ctx, hs, RelayVar::Int(0), false);
        assert_eq!(out, RelayVar::Int(1), "relay threaded through add_int");
        assert_eq!(state.opponent_battlers[0].hp, 95, "deal_5 mutated target");
    }

    // ── BORROW proof 2: pair_mut CROSS-side branch (the one unsafe) ──

    #[test]
    fn pair_mut_cross_side_branch_works() {
        let (mut state, mut effects, mut mv) = make_ctx_parts(100, 50);
        let mut rng = ScriptedRng::new(vec![0]);
        let mut ctx = BattleCtx {
            state: &mut state,
            effects: &mut effects,
            mv: &mut mv,
            rng: &mut rng,
        };
        let (player, opp) = ctx.pair_mut(BattlerRef::PLAYER, BattlerRef::OPPONENT);
        assert_eq!(player.hp, 100);
        assert_eq!(opp.hp, 50);
        // Mutate one while reading the other — proves true disjoint &mut.
        let opp_hp = opp.hp;
        player.take_damage(opp_hp);
        assert_eq!(state.player_battlers[0].hp, 50);
        assert_eq!(state.opponent_battlers[0].hp, 50);
    }

    // ── BORROW proof 3: pair_mut SAME-side branch (split_at_mut, safe) ──

    #[test]
    fn pair_mut_same_side_branch_works() {
        // Two slots on the player side (doubles-shaped) to exercise split_at_mut.
        let mut state = BattleState::new(vec![mon(100), mon(40)], vec![mon(100)]);
        let mut effects: Vec<EffectState<TProvider>> = Vec::new();
        let mut mv = MoveContext::default();
        let mut rng = ScriptedRng::new(vec![0]);
        let mut ctx = BattleCtx {
            state: &mut state,
            effects: &mut effects,
            mv: &mut mv,
            rng: &mut rng,
        };
        // Higher slot first, to also prove the order-swap path.
        let a = BattlerRef::new(0, 1);
        let b = BattlerRef::new(0, 0);
        let (slot1, slot0) = ctx.pair_mut(a, b);
        assert_eq!(slot1.hp, 40);
        assert_eq!(slot0.hp, 100);
        let read = slot0.hp;
        slot1.take_damage(read / 4); // 100/4 = 25 → 40-25 = 15
        assert_eq!(state.player_battlers[1].hp, 15);
        assert_eq!(state.player_battlers[0].hp, 100);
    }

    // ── BORROW proof 4: Counter-shaped handler, NO RefCell ──

    #[test]
    fn counter_shaped_handler_compiles_no_refcell() {
        let (mut state, mut effects, mut mv) = make_ctx_parts(100, 70);
        let mut rng = ScriptedRng::new(vec![0]);
        let mut ctx = BattleCtx {
            state: &mut state,
            effects: &mut effects,
            mv: &mut mv,
            rng: &mut rng,
        };
        // Player's handler reflects opponent's hp/10 onto opponent? No: it
        // mutates `target` reading `source`. Here target=OPPONENT, source=PLAYER.
        let hs = vec![CollectedHandler {
            order: 1,
            priority: 0,
            speed: 0,
            sub_order: 6,
            effect_order: 0,
            target: BattlerRef::OPPONENT,
            source: BattlerRef::PLAYER,
            source_effect: EffectId(1),
            call: counter_shaped::<TProvider>,
        }];
        run_event(&mut ctx, hs, RelayVar::Unit, false);
        // opponent took player.hp/10 = 100/10 = 10 → 70-10 = 60
        assert_eq!(state.opponent_battlers[0].hp, 60);
    }

    // ── Typed EffectState proof: Toxic counter via effect_mut (design §3.3) ──

    #[test]
    fn typed_effect_state_toxic_counter() {
        let (mut state, mut effects, mut mv) = make_ctx_parts(100, 100);
        effects.push(EffectState {
            id: EffectId(7),
            host: BattlerRef::PLAYER,
            effect_order: 0,
            kind: TKind::Toxic { counter: 0 },
        });
        let mut rng = ScriptedRng::new(vec![0]);
        let mut ctx = BattleCtx {
            state: &mut state,
            effects: &mut effects,
            mv: &mut mv,
            rng: &mut rng,
        };
        let n = match &mut ctx.effect_mut(EffectId(7)).unwrap().kind {
            TKind::Toxic { counter } => {
                *counter = counter.saturating_add(1);
                *counter
            }
            TKind::None => 0,
        };
        assert_eq!(n, 1);
        // binary-search miss returns None.
        assert!(ctx.effect_mut(EffectId(99)).is_none());
    }

    // ── forced_action seam: a live volatile overrides the chosen action ──
    //
    // Proves the generic, defaulted cross-turn lock-in seam (design §3/§9): the
    // driver consults `EffectProvider::forced_action` BEFORE reading the per-turn
    // chosen action, so a volatile recorded earlier hijacks this turn. This is the
    // engine-side proof that a per-turn `[Action; 2]` input is insufficient; it is
    // game-agnostic (only swaps one `BattleAction` for another, names no game-specific
    // volatile) and inert by default (every other test/game gets `None`).

    use crate::battle::rng::ScriptedRng as EngineScriptedRng;
    use crate::battle::BattleAction;

    /// A damaging move effect for the forced-action proof: `ModifyDamage` deals
    /// `move.power` to the defender (the driver applies `ctx.mv.damage`).
    fn force_dmg<P: EffectProvider + ?Sized>(
        ctx: &mut BattleCtx<'_, P>,
        _relay: RelayVar,
        _target: BattlerRef,
        _source: BattlerRef,
        _eff: EffectId,
    ) -> HandlerResult {
        ctx.mv.damage = 10;
        HandlerResult::Unchanged
    }

    static FORCE_DMG_EFFECT: Effect<TForce> = Effect {
        id: EffectId(1),
        kind: EffectType::Move,
        hooks: &[EventHook {
            event: Event::ModifyDamage,
            call: force_dmg::<TForce>,
            order: 1000,
            priority: 0,
            sub_order: None,
        }],
    };

    /// A provider whose `forced_action` forces `Nothing` whenever the actor hosts
    /// a `TKind::Toxic` volatile (used here purely as a generic "is-locked" marker
    /// — the engine attaches NO game-specific meaning to it).
    struct TForce;

    impl BattleProvider for TForce {
        type Monster = ();
        type Move = TMove;
        type Ability = ();
        type Status = TStatus;
        type Stat = TStat;
        type Species = TSpecies;
        type Type = TType;
        type Item = ();
        fn calculate_damage(
            &self,
            _m: &Self::Move,
            _a: &BattlerState<Self>,
            _d: &BattlerState<Self>,
            _r: u8,
            _c: bool,
        ) -> DamageResult {
            DamageResult {
                damage: 0,
                effectiveness: 1.0,
                is_miss: false,
            }
        }
        fn select_move(&self, b: &BattlerState<Self>, _s: &BattleState<Self>) -> Self::Move {
            b.moves.first().cloned().unwrap()
        }
        fn apply_move_effect(
            &self,
            _e: MoveEffect,
            _u: &mut BattlerState<Self>,
            _t: &mut BattlerState<Self>,
        ) -> EffectResult {
            EffectResult::NoEffect
        }
        fn create_monster(&self, s: Self::Species, _l: u8) -> BattlerState<Self> {
            BattlerState::new(s, 100, 100, EnumMap::new(), vec![])
        }
    }

    impl EffectProvider for TForce {
        type EffectStateKind = TKind;
        fn effect_for_move(&self, _m: &Self::Move) -> Option<&'static Effect<Self>> {
            Some(&FORCE_DMG_EFFECT)
        }
        fn effect_for_status(&self, _s: &Self::Status) -> Option<&'static Effect<Self>> {
            None
        }
        fn turn_order_rank(
            &self,
            _state: &BattleState<Self>,
            who: BattlerRef,
            _action: &Self::Move,
        ) -> (i32, i32) {
            // Player always first (no tie → no order byte drawn).
            if who.side == 0 {
                (0, 0)
            } else {
                (1, 0)
            }
        }
        fn forced_action(
            &self,
            effects: &[EffectState<Self>],
            actor: BattlerRef,
            chosen: &BattleAction<Self>,
        ) -> Option<BattleAction<Self>> {
            // A `Toxic` volatile on the actor forces inaction (the recharge shape).
            let locked = effects
                .iter()
                .any(|e| e.host == actor && matches!(e.kind, TKind::Toxic { .. }));
            if locked {
                Some(BattleAction::Nothing)
            } else {
                let _ = chosen;
                None
            }
        }
    }

    fn force_mon(hp: u16) -> BattlerState<TForce> {
        let mut stats = EnumMap::new();
        stats.set(TStat::Hp, hp);
        BattlerState::new(TSpecies::Mon, hp, hp, stats, vec![TMove { power: 10 }])
    }

    #[test]
    fn forced_action_overrides_chosen_action() {
        // Player has a "lock" volatile → forced_action returns `Nothing` → its
        // chosen `Fight` is IGNORED (deals no damage). The opponent has no lock →
        // its `Fight` runs normally (deals 10). Proves the seam hijacks the
        // per-turn chosen action using cross-turn arena state.
        let mut state = BattleState::new(vec![force_mon(100)], vec![force_mon(100)]);
        let mut effects: Vec<EffectState<TForce>> = vec![EffectState {
            id: EffectId(50),
            host: BattlerRef::PLAYER,
            effect_order: 0,
            kind: TKind::Toxic { counter: 0 },
        }];
        let actions = [
            BattleAction::<TForce>::Fight {
                move_: TMove { power: 10 },
            },
            BattleAction::<TForce>::Fight {
                move_: TMove { power: 10 },
            },
        ];
        let mut rng = EngineScriptedRng::new(vec![]);
        let provider = TForce;
        StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
        // Player's chosen Fight was overridden to Nothing → opponent UNHARMED.
        assert_eq!(
            state.opponent_battlers[0].hp, 100,
            "locked player forced to Nothing"
        );
        // Opponent acted normally → player took 10.
        assert_eq!(
            state.player_battlers[0].hp, 90,
            "unlocked opponent's Fight ran"
        );
    }

    #[test]
    fn forced_action_default_is_inert() {
        // No lock volatile → forced_action returns None → both Fights run.
        let mut state = BattleState::new(vec![force_mon(100)], vec![force_mon(100)]);
        let mut effects: Vec<EffectState<TForce>> = Vec::new();
        let actions = [
            BattleAction::<TForce>::Fight {
                move_: TMove { power: 10 },
            },
            BattleAction::<TForce>::Fight {
                move_: TMove { power: 10 },
            },
        ];
        let mut rng = EngineScriptedRng::new(vec![]);
        let provider = TForce;
        StackDriver::execute_turn(&provider, &mut state, &mut effects, actions, &mut rng);
        assert_eq!(
            state.opponent_battlers[0].hp, 90,
            "player Fight ran (no lock)"
        );
        assert_eq!(
            state.player_battlers[0].hp, 90,
            "opponent Fight ran (no lock)"
        );
    }

    // ── P6a: the generic turn-event log (execute_turn_logged) ────────────────

    use super::log::TurnEvent;

    /// The ADDITIVE/DEFAULTED guarantee: `execute_turn_logged` runs the SAME turn
    /// as `execute_turn` — identical final state, identical `StackTurnResult`,
    /// identical `rng.consumed()`. The log is pure observation.
    #[test]
    fn logged_turn_is_behaviorally_identical_to_plain() {
        let actions = || {
            [
                BattleAction::<TForce>::Fight {
                    move_: TMove { power: 10 },
                },
                BattleAction::<TForce>::Fight {
                    move_: TMove { power: 10 },
                },
            ]
        };
        // Plain.
        let mut s_plain = BattleState::new(vec![force_mon(100)], vec![force_mon(100)]);
        let mut e_plain: Vec<EffectState<TForce>> = Vec::new();
        let mut rng_plain = EngineScriptedRng::new(vec![]);
        let r_plain = StackDriver::execute_turn(
            &TForce,
            &mut s_plain,
            &mut e_plain,
            actions(),
            &mut rng_plain,
        );
        // Logged.
        let mut s_log = BattleState::new(vec![force_mon(100)], vec![force_mon(100)]);
        let mut e_log: Vec<EffectState<TForce>> = Vec::new();
        let mut rng_log = EngineScriptedRng::new(vec![]);
        let (r_log, log) = StackDriver::execute_turn_logged(
            &TForce,
            &mut s_log,
            &mut e_log,
            actions(),
            &mut rng_log,
        );

        assert_eq!(r_plain, r_log, "same StackTurnResult");
        assert_eq!(
            rng_plain.consumed(),
            rng_log.consumed(),
            "same rng draw count"
        );
        assert_eq!(
            s_plain.player_battlers[0].hp, s_log.player_battlers[0].hp,
            "same player hp"
        );
        assert_eq!(
            s_plain.opponent_battlers[0].hp, s_log.opponent_battlers[0].hp,
            "same opponent hp"
        );
        assert!(!log.is_empty(), "the logged path recorded events");
    }

    /// The log captures `MoveUsed` + `Damaged` for both movers, in order. With
    /// `TForce`: player first (deals 10 → opp 90), then opponent (deals 10 → player
    /// 90). No crit/miss handlers ⇒ no `Crit`/`Missed`.
    #[test]
    fn log_records_move_used_and_damage_both_movers() {
        let mut state = BattleState::new(vec![force_mon(100)], vec![force_mon(100)]);
        let mut effects: Vec<EffectState<TForce>> = Vec::new();
        let actions = [
            BattleAction::<TForce>::Fight {
                move_: TMove { power: 10 },
            },
            BattleAction::<TForce>::Fight {
                move_: TMove { power: 10 },
            },
        ];
        let mut rng = EngineScriptedRng::new(vec![]);
        let (_r, log) =
            StackDriver::execute_turn_logged(&TForce, &mut state, &mut effects, actions, &mut rng);

        let p = BattlerRef::PLAYER;
        let o = BattlerRef::OPPONENT;
        let ev = &log.events;
        assert!(
            matches!(ev[0], TurnEvent::MoveUsed { actor, .. } if actor == p),
            "1: player MoveUsed"
        );
        assert!(
            matches!(ev[1], TurnEvent::Damaged { target, amount: 10, .. } if target == o),
            "2: opponent took 10"
        );
        assert!(
            matches!(ev[2], TurnEvent::MoveUsed { actor, .. } if actor == o),
            "3: opponent MoveUsed"
        );
        assert!(
            matches!(ev[3], TurnEvent::Damaged { target, amount: 10, .. } if target == p),
            "4: player took 10"
        );
        assert_eq!(
            ev.len(),
            4,
            "exactly MoveUsed+Damaged ×2 (no crit/miss/status)"
        );
    }

    /// A KO logs `Damaged` then `Fainted`, and the second mover is cancelled (so it
    /// never logs a `MoveUsed`). Opponent at 10 HP dies to the player's 10 damage.
    #[test]
    fn log_records_faint_and_cancels_second() {
        let mut state = BattleState::new(vec![force_mon(100)], vec![force_mon(10)]);
        let mut effects: Vec<EffectState<TForce>> = Vec::new();
        let actions = [
            BattleAction::<TForce>::Fight {
                move_: TMove { power: 10 },
            },
            BattleAction::<TForce>::Fight {
                move_: TMove { power: 10 },
            },
        ];
        let mut rng = EngineScriptedRng::new(vec![]);
        let (r, log) =
            StackDriver::execute_turn_logged(&TForce, &mut state, &mut effects, actions, &mut rng);

        assert!(r.second_cancelled, "defender KO'd → second move cancelled");
        let o = BattlerRef::OPPONENT;
        let ev = &log.events;
        assert!(
            matches!(ev[0], TurnEvent::MoveUsed { .. }),
            "player MoveUsed"
        );
        assert!(
            matches!(ev[1], TurnEvent::Damaged { target, amount: 10, .. } if target == o),
            "opponent took 10"
        );
        assert!(
            matches!(ev[2], TurnEvent::Fainted { who } if who == o),
            "opponent fainted"
        );
        assert!(
            !ev.iter()
                .any(|e| matches!(e, TurnEvent::MoveUsed { actor, .. } if *actor == o)),
            "cancelled opponent never logs a MoveUsed"
        );
    }

    /// A rich move effect exercises the `StatChanged` / `StatusInflicted` / `Healed`
    /// diff paths: it deals 10 to the target, boosts the ACTOR's stat stage +1,
    /// poisons the target, and heals the actor (who starts below max).
    fn log_rich(
        ctx: &mut BattleCtx<'_, TRich>,
        _relay: RelayVar,
        target: BattlerRef,
        source: BattlerRef,
        _eff: EffectId,
    ) -> HandlerResult {
        ctx.mv.damage = 10; // driver applies → Damaged{target,10}
        ctx.battler_mut(source).stat_stages.set(TStat::Hp, 1); // StatChanged{source,+1}
        ctx.battler_mut(target).status = Some(TStatus::Poisoned); // StatusInflicted{target}
        let b = ctx.battler_mut(source);
        b.hp = (b.hp + 20).min(b.max_hp); // Healed{source,20}
        HandlerResult::Unchanged
    }

    static LOG_RICH_EFFECT: Effect<TRich> = Effect {
        id: EffectId(1),
        kind: EffectType::Move,
        hooks: &[EventHook {
            event: Event::ModifyDamage,
            call: log_rich,
            order: 1000,
            priority: 0,
            sub_order: None,
        }],
    };

    /// A provider whose move both damages the foe and buffs/heals/poisons — only
    /// the player acts (opponent does `Nothing`) so the assertions are unambiguous.
    struct TRich;
    impl BattleProvider for TRich {
        type Monster = ();
        type Move = TMove;
        type Ability = ();
        type Status = TStatus;
        type Stat = TStat;
        type Species = TSpecies;
        type Type = TType;
        type Item = ();
        fn calculate_damage(
            &self,
            _m: &Self::Move,
            _a: &BattlerState<Self>,
            _d: &BattlerState<Self>,
            _r: u8,
            _c: bool,
        ) -> DamageResult {
            DamageResult {
                damage: 0,
                effectiveness: 1.0,
                is_miss: false,
            }
        }
        fn select_move(&self, b: &BattlerState<Self>, _s: &BattleState<Self>) -> Self::Move {
            b.moves.first().cloned().unwrap()
        }
        fn apply_move_effect(
            &self,
            _e: MoveEffect,
            _u: &mut BattlerState<Self>,
            _t: &mut BattlerState<Self>,
        ) -> EffectResult {
            EffectResult::NoEffect
        }
        fn create_monster(&self, s: Self::Species, _l: u8) -> BattlerState<Self> {
            BattlerState::new(s, 100, 100, EnumMap::new(), vec![])
        }
    }
    impl EffectProvider for TRich {
        type EffectStateKind = TKind;
        fn effect_for_move(&self, _m: &Self::Move) -> Option<&'static Effect<Self>> {
            Some(&LOG_RICH_EFFECT)
        }
        fn effect_for_status(&self, _s: &Self::Status) -> Option<&'static Effect<Self>> {
            None
        }
        fn turn_order_rank(
            &self,
            _state: &BattleState<Self>,
            who: BattlerRef,
            _action: &Self::Move,
        ) -> (i32, i32) {
            if who.side == 0 {
                (0, 0)
            } else {
                (1, 0)
            }
        }
    }

    #[test]
    fn log_records_stat_status_and_heal_diffs() {
        // Player at 50/100 so the +20 self-heal is observable; opponent does Nothing.
        let mut player = BattlerState::<TRich>::new(
            TSpecies::Mon,
            50,
            100,
            EnumMap::new(),
            vec![TMove { power: 10 }],
        );
        player.max_hp = 100;
        let opponent = BattlerState::<TRich>::new(
            TSpecies::Mon,
            100,
            100,
            EnumMap::new(),
            vec![TMove { power: 10 }],
        );
        let mut state = BattleState::new(vec![player], vec![opponent]);
        let mut effects: Vec<EffectState<TRich>> = Vec::new();
        let actions = [
            BattleAction::<TRich>::Fight {
                move_: TMove { power: 10 },
            },
            BattleAction::<TRich>::Nothing,
        ];
        let mut rng = EngineScriptedRng::new(vec![]);
        let (_r, log) =
            StackDriver::execute_turn_logged(&TRich, &mut state, &mut effects, actions, &mut rng);

        let p = BattlerRef::PLAYER;
        let o = BattlerRef::OPPONENT;
        let ev = &log.events;
        assert!(
            ev.iter()
                .any(|e| matches!(e, TurnEvent::MoveUsed { actor, .. } if *actor == p)),
            "MoveUsed"
        );
        assert!(
            ev.iter().any(
                |e| matches!(e, TurnEvent::Damaged { target, amount: 10, .. } if *target == o)
            ),
            "Damaged target"
        );
        assert!(
            ev.iter()
                .any(|e| matches!(e, TurnEvent::StatusInflicted { target, .. } if *target == o)),
            "StatusInflicted target"
        );
        assert!(
            ev.iter().any(
                |e| matches!(e, TurnEvent::StatChanged { target, delta: 1, .. } if *target == p)
            ),
            "StatChanged actor +1"
        );
        assert!(
            ev.iter()
                .any(|e| matches!(e, TurnEvent::Healed { target, amount: 20, .. } if *target == p)),
            "Healed actor +20"
        );
    }

    // ── RESOURCE COST GATE (doc 13 §4): the generic MP/SP/mana cost check ──
    //
    // A `TCost` provider declares a `move_cost` of 4 on resource id 0 (the engine
    // assigns this resource NO meaning — it is "MP" only to the game). The driver,
    // at `resolve_action`, checks the actor can pay it against its `ResourcePool`
    // (else prevents the move via the `Fail`/early-return path) and deducts on
    // success — PURE ARITHMETIC, consuming NO rng. The default empty cost + empty
    // pool is inert (the forced_action tests above, which declare no cost, prove
    // the no-op case end to end).

    const MP: u16 = 0; // the game's opaque resource id for "MP" — the engine never names it.

    /// A move-effect that deals 10 to the defender (so a paid move's effect is
    /// observable, and a prevented move's absence is observable).
    static COST_DMG_EFFECT: Effect<TCost> = Effect {
        id: EffectId(1),
        kind: EffectType::Move,
        hooks: &[EventHook {
            event: Event::ModifyDamage,
            call: force_dmg::<TCost>,
            order: 1000,
            priority: 0,
            sub_order: None,
        }],
    };

    /// A provider whose every move costs 4 of resource [`MP`].
    struct TCost;

    impl BattleProvider for TCost {
        type Monster = ();
        type Move = TMove;
        type Ability = ();
        type Status = TStatus;
        type Stat = TStat;
        type Species = TSpecies;
        type Type = TType;
        type Item = ();
        fn calculate_damage(
            &self,
            _m: &Self::Move,
            _a: &BattlerState<Self>,
            _d: &BattlerState<Self>,
            _r: u8,
            _c: bool,
        ) -> DamageResult {
            DamageResult {
                damage: 0,
                effectiveness: 1.0,
                is_miss: false,
            }
        }
        fn select_move(&self, b: &BattlerState<Self>, _s: &BattleState<Self>) -> Self::Move {
            b.moves.first().cloned().unwrap()
        }
        fn apply_move_effect(
            &self,
            _e: MoveEffect,
            _u: &mut BattlerState<Self>,
            _t: &mut BattlerState<Self>,
        ) -> EffectResult {
            EffectResult::NoEffect
        }
        fn create_monster(&self, s: Self::Species, _l: u8) -> BattlerState<Self> {
            BattlerState::new(s, 100, 100, EnumMap::new(), vec![])
        }
        /// The whole point: every move costs 4 MP. Defaulted-hook OVERRIDE.
        fn move_cost(&self, _move_: &Self::Move) -> &[(u16, u16)] {
            &[(MP, 4)]
        }
    }

    impl EffectProvider for TCost {
        type EffectStateKind = TKind;
        fn effect_for_move(&self, _m: &Self::Move) -> Option<&'static Effect<Self>> {
            Some(&COST_DMG_EFFECT)
        }
        fn effect_for_status(&self, _s: &Self::Status) -> Option<&'static Effect<Self>> {
            None
        }
        fn turn_order_rank(
            &self,
            _state: &BattleState<Self>,
            who: BattlerRef,
            _action: &Self::Move,
        ) -> (i32, i32) {
            // Player always first (no tie → no order byte drawn).
            if who.side == 0 {
                (0, 0)
            } else {
                (1, 0)
            }
        }
    }

    /// A `TCost` battler with `mp` MP (resource id 0) and `mp` max.
    fn cost_mon(hp: u16, mp: u16) -> BattlerState<TCost> {
        let mut stats = EnumMap::new();
        stats.set(TStat::Hp, hp);
        BattlerState::new(TSpecies::Mon, hp, hp, stats, vec![TMove { power: 10 }])
            .with_resource(MP, mp)
    }

    #[test]
    fn cost_gate_pays_and_deducts_when_affordable() {
        // Player has 10 MP; its move costs 4 → it acts (deals 10) and ends with 6.
        // Opponent has 0 MP (no resource declared at all) so its 4-MP move is
        // PREVENTED — but it moves SECOND, after the player KO check passes, and is
        // a clean control for the insufficient case in one turn.
        let mut state = BattleState::new(vec![cost_mon(100, 10)], vec![cost_mon(100, 0)]);
        let mut effects: Vec<EffectState<TCost>> = Vec::new();
        let actions = [
            BattleAction::<TCost>::Fight {
                move_: TMove { power: 10 },
            },
            BattleAction::<TCost>::Fight {
                move_: TMove { power: 10 },
            },
        ];
        let mut rng = EngineScriptedRng::new(vec![]);
        StackDriver::execute_turn(&TCost, &mut state, &mut effects, actions, &mut rng);

        // Player paid: 10 - 4 = 6 MP left, and its move connected (opp took 10).
        assert_eq!(
            state.player_battlers[0].resources.current(MP),
            Some(6),
            "affordable: 10 MP - 4 cost = 6 left"
        );
        assert_eq!(
            state.opponent_battlers[0].hp, 90,
            "paid move dealt its 10 damage"
        );

        // Opponent could NOT pay (0 MP, undeclared/zero) → move prevented → player
        // unharmed, opponent MP unchanged (still absent → current() None... it WAS
        // declared with max 0, so current is Some(0), and stays Some(0)).
        assert_eq!(
            state.player_battlers[0].hp, 100,
            "insufficient opp move prevented → no damage"
        );
        assert_eq!(
            state.opponent_battlers[0].resources.current(MP),
            Some(0),
            "insufficient: MP unchanged (no deduction on a prevented move)"
        );
    }

    #[test]
    fn cost_gate_prevents_when_unaffordable_and_leaves_mp_unchanged() {
        // Player has 3 MP, move costs 4 → CANNOT pay → move PREVENTED. The opponent
        // (here also short on MP) is irrelevant; assert the prevention + no deduction.
        let mut state = BattleState::new(vec![cost_mon(100, 3)], vec![cost_mon(100, 0)]);
        let mut effects: Vec<EffectState<TCost>> = Vec::new();
        let actions = [
            BattleAction::<TCost>::Fight {
                move_: TMove { power: 10 },
            },
            BattleAction::<TCost>::Fight {
                move_: TMove { power: 10 },
            },
        ];
        let mut rng = EngineScriptedRng::new(vec![]);
        StackDriver::execute_turn(&TCost, &mut state, &mut effects, actions, &mut rng);

        assert_eq!(
            state.opponent_battlers[0].hp, 100,
            "player could not pay → its move was prevented → opp unharmed"
        );
        assert_eq!(
            state.player_battlers[0].resources.current(MP),
            Some(3),
            "prevented move deducts NOTHING → MP unchanged at 3"
        );
    }

    #[test]
    fn cost_gate_consumes_no_rng() {
        // The cost path is pure arithmetic. Drive a turn where the player pays and
        // the opponent is prevented (the most code-covering case) with an EMPTY
        // scripted rng and assert `consumed() == 0`. (The driver draws an order
        // byte only on a tie; here turn_order_rank gives the player a strictly
        // lower rank, so even turn-order draws nothing — isolating the cost path.)
        let mut state = BattleState::new(vec![cost_mon(100, 10)], vec![cost_mon(100, 1)]);
        let mut effects: Vec<EffectState<TCost>> = Vec::new();
        let actions = [
            BattleAction::<TCost>::Fight {
                move_: TMove { power: 10 },
            },
            BattleAction::<TCost>::Fight {
                move_: TMove { power: 10 },
            },
        ];
        let mut rng = EngineScriptedRng::new(vec![]);
        StackDriver::execute_turn(&TCost, &mut state, &mut effects, actions, &mut rng);
        assert_eq!(
            rng.consumed(),
            0,
            "the resource cost check + deduction consume NO randomness"
        );
    }

    #[test]
    fn no_cost_no_resources_is_inert() {
        // The `TForce` provider declares NO `move_cost` (defaulted `&[]`) and its
        // battlers declare NO resources (empty `ResourcePool`). Driving a turn is
        // byte-identical to the pre-resource engine: both Fights run, dealing 10.
        // This is the additivity witness — the empty/default path is a pure no-op.
        let mut state = BattleState::new(vec![force_mon(100)], vec![force_mon(100)]);
        let mut effects: Vec<EffectState<TForce>> = Vec::new();
        let actions = [
            BattleAction::<TForce>::Fight {
                move_: TMove { power: 10 },
            },
            BattleAction::<TForce>::Fight {
                move_: TMove { power: 10 },
            },
        ];
        let mut rng = EngineScriptedRng::new(vec![]);
        StackDriver::execute_turn(&TForce, &mut state, &mut effects, actions, &mut rng);
        assert_eq!(
            state.opponent_battlers[0].hp, 90,
            "no-cost move ran (inert gate)"
        );
        assert_eq!(
            state.player_battlers[0].hp, 90,
            "no-cost move ran (inert gate)"
        );
        assert!(
            state.player_battlers[0].resources.is_empty(),
            "pool defaulted EMPTY"
        );
    }

    // ── speed_sort_tiebreak: draws a byte ONLY on a tie, permutes the run ──

    #[test]
    fn speed_tiebreak_draws_only_on_tie() {
        // Two fully-equal handlers → one tie comparison → one byte drawn.
        let mk = |eo: u64| CollectedHandler::<TProvider> {
            order: 1,
            priority: 0,
            speed: 0,
            sub_order: 6,
            effect_order: eo,
            target: BattlerRef::OPPONENT,
            source: BattlerRef::PLAYER,
            source_effect: EffectId(eo as u32),
            call: add_int::<TProvider>,
        };
        // effect_order differs → NOT equal → no draw.
        let mut distinct = vec![mk(0), mk(1)];
        let mut rng = ScriptedRng::new(vec![200, 200]);
        dispatch::speed_sort_tiebreak(&mut distinct, &mut rng);
        assert_eq!(
            rng.consumed(),
            0,
            "distinct effect_order ⇒ no tie ⇒ no draw"
        );

        // identical effect_order → equal → one byte drawn for the pair.
        let mut tied = vec![mk(0), mk(0)];
        let mut rng2 = ScriptedRng::new(vec![200]);
        dispatch::speed_sort_tiebreak(&mut tied, &mut rng2);
        assert_eq!(rng2.consumed(), 1, "one tied pair ⇒ exactly one draw");
    }

    // ── comparator: order asc, priority desc, effect_order asc tiebreak ──

    #[test]
    fn comparator_lexical_order() {
        use core::cmp::Ordering;
        let h = |order, priority, eo| CollectedHandler::<TProvider> {
            order,
            priority,
            speed: 0,
            sub_order: 6,
            effect_order: eo,
            target: BattlerRef::OPPONENT,
            source: BattlerRef::PLAYER,
            source_effect: EffectId(0),
            call: add_int::<TProvider>,
        };
        // lower order fires first
        assert_eq!(compare(&h(1, 0, 0), &h(2, 0, 0)), Ordering::Less);
        // equal order: higher priority fires first
        assert_eq!(compare(&h(1, 5, 0), &h(1, 1, 0)), Ordering::Less);
        // equal order+priority: lower effect_order fires first
        assert_eq!(compare(&h(1, 0, 3), &h(1, 0, 9)), Ordering::Less);
        // fully equal
        assert_eq!(compare(&h(1, 0, 0), &h(1, 0, 0)), Ordering::Equal);
    }
}

// ─── P0b: broadened multi-source collection + EffectHost engine proofs ────────
//
// Game-agnostic, mock-game style (design §6). Each test uses the shared
// `tests_support::TProvider` (6-stat Gen-4 shape, opaque ability/item/field
// markers). The handlers stamp `stat_stages[Atk]` so a test can read which
// sources fired and in what order.

#[cfg(test)]
mod multi_source_tests {
    use super::dispatch::{collect_handlers, run_event, run_event_checked};
    use super::tests_support::{marker, mon, parts, TKind, TProvider, TSpecies, MOCK_VOLATILE};
    use super::*;
    use crate::battle::stack::ctx::EffectHost;
    use crate::battle::{BattlerRef, EffectResult};

    fn ctx_from<'a>(
        state: &'a mut crate::battle::BattleState<TProvider>,
        effects: &'a mut Vec<EffectState<TProvider>>,
        mv: &'a mut MoveContext,
        rng: &'a mut crate::battle::rng::ScriptedRng,
    ) -> BattleCtx<'a, TProvider> {
        BattleCtx {
            state,
            effects,
            mv,
            rng,
        }
    }

    // ── Proof 1: multi-source collection gathers from ability + item + field,
    //    firing them in comparator (`order`) order. ──
    #[test]
    fn collects_ability_item_field_in_order() {
        // Player has BOTH ability+item; field is ON. Event targets the OPPONENT
        // (source = player). The opponent's own ability/item are absent (Plain).
        let provider = TProvider { field_on: true };
        let (mut state, mut effects, mut mv, mut rng) =
            parts(mon(100, TSpecies::HasBoth), mon(100, TSpecies::Plain));
        let mut ctx = ctx_from(&mut state, &mut effects, &mut mv, &mut rng);

        let mut hs = Vec::new();
        collect_handlers(
            &ctx,
            &provider,
            None, // no source move effect
            Event::DamagingHit,
            BattlerRef::OPPONENT, // target
            BattlerRef::PLAYER,   // source
            &mut hs,
        );
        // Player's ability (order 10) + item (order 20) + field (order 30) = 3.
        assert_eq!(hs.len(), 3, "ability + item + field collected");

        run_event(&mut ctx, hs, RelayVar::Unit, false);
        // ability marks player (target of the ability hook is the host = player),
        // item marks player, field marks OPPONENT (target). Read both:
        // ability(1) + item(10) on player → 11; field(100) on opponent → 100.
        assert_eq!(
            marker(&state, BattlerRef::PLAYER),
            11,
            "ability+item fired on host"
        );
        assert_eq!(
            marker(&state, BattlerRef::OPPONENT),
            100,
            "field fired on target"
        );
    }

    // ── Proof 1b: multi-source ordering is by the comparator. Manually verify
    //    the collected order is ability(10) < volatile(15) < item(20) < field(30).
    #[test]
    fn multi_source_comparator_order() {
        let provider = TProvider { field_on: true };
        let (mut state, mut effects, mut mv, mut rng) =
            parts(mon(100, TSpecies::HasBoth), mon(100, TSpecies::Plain));
        // Add a live volatile on the source (player) → MOCK_VOLATILE (order 15).
        effects.push(EffectState {
            id: MOCK_VOLATILE.id,
            host: BattlerRef::PLAYER,
            effect_order: 0,
            kind: TKind::Vol,
        });
        let ctx = ctx_from(&mut state, &mut effects, &mut mv, &mut rng);

        let mut hs = Vec::new();
        collect_handlers(
            &ctx,
            &provider,
            None,
            Event::DamagingHit,
            BattlerRef::OPPONENT,
            BattlerRef::PLAYER,
            &mut hs,
        );
        hs.sort_by(super::dispatch::compare);
        let orders: Vec<u32> = hs.iter().map(|h| h.order).collect();
        assert_eq!(orders, vec![10, 15, 20, 30], "ability<volatile<item<field");
    }

    // ── Proof 2: snapshot + per-step re-check handles a handler that KOs the
    //    target another handler was about to act on (mid-fold mutation). ──
    fn ko_target<P: EffectProvider + ?Sized>(
        ctx: &mut BattleCtx<'_, P>,
        _r: RelayVar,
        target: BattlerRef,
        _s: BattlerRef,
        _e: EffectId,
    ) -> HandlerResult {
        ctx.battler_mut(target).hp = 0; // KO the target
        HandlerResult::Unchanged
    }
    fn touch_target<P: EffectProvider<Stat = super::tests_support::TStat> + ?Sized>(
        ctx: &mut BattleCtx<'_, P>,
        _r: RelayVar,
        target: BattlerRef,
        _s: BattlerRef,
        _e: EffectId,
    ) -> HandlerResult {
        super::tests_support::mark(ctx, target, 7); // would mark — but target is dead
        HandlerResult::Unchanged
    }

    #[test]
    fn re_check_skips_handler_whose_target_was_koed_midfold() {
        let (mut state, mut effects, mut mv, mut rng) =
            parts(mon(100, TSpecies::Plain), mon(100, TSpecies::Plain));
        let mut ctx = ctx_from(&mut state, &mut effects, &mut mv, &mut rng);
        // First handler (order 1) KOs OPPONENT; second (order 2) would mark it.
        let hs = vec![
            CollectedHandler {
                order: 1,
                priority: 0,
                speed: 0,
                sub_order: 6,
                effect_order: 0,
                target: BattlerRef::OPPONENT,
                source: BattlerRef::PLAYER,
                source_effect: EffectId(1),
                call: ko_target::<TProvider>,
            },
            CollectedHandler {
                order: 2,
                priority: 0,
                speed: 0,
                sub_order: 6,
                effect_order: 1,
                target: BattlerRef::OPPONENT,
                source: BattlerRef::PLAYER,
                source_effect: EffectId(2),
                call: touch_target::<TProvider>,
            },
        ];
        run_event_checked(&mut ctx, hs, RelayVar::Unit, false);
        assert_eq!(
            state.opponent_battlers[0].hp, 0,
            "first handler KO'd target"
        );
        assert_eq!(
            marker(&state, BattlerRef::OPPONENT),
            0,
            "re-check SKIPPED the second handler (dead target)"
        );
    }

    // The plain `run_event` (slice path) does NOT re-check → the second handler
    // would still fire. This documents the behavioral difference precisely.
    #[test]
    fn plain_run_event_does_not_re_check() {
        let provider = TProvider::default();
        let _ = &provider; // unused: hs built directly
        let (mut state, mut effects, mut mv, mut rng) =
            parts(mon(100, TSpecies::Plain), mon(100, TSpecies::Plain));
        let mut ctx = ctx_from(&mut state, &mut effects, &mut mv, &mut rng);
        let hs = vec![
            CollectedHandler {
                order: 1,
                priority: 0,
                speed: 0,
                sub_order: 6,
                effect_order: 0,
                target: BattlerRef::OPPONENT,
                source: BattlerRef::PLAYER,
                source_effect: EffectId(1),
                call: ko_target::<TProvider>,
            },
            CollectedHandler {
                order: 2,
                priority: 0,
                speed: 0,
                sub_order: 6,
                effect_order: 1,
                target: BattlerRef::OPPONENT,
                source: BattlerRef::PLAYER,
                source_effect: EffectId(2),
                call: touch_target::<TProvider>,
            },
        ];
        run_event(&mut ctx, hs, RelayVar::Unit, false);
        assert_eq!(
            marker(&state, BattlerRef::OPPONENT),
            7,
            "plain run_event fired the second handler (no re-check) — slice contract"
        );
    }

    // ── Proof 2b: a handler that REMOVES another live effect mid-fold does not
    //    corrupt the owned snapshot (no iterator invalidation, no RefCell). ──
    fn remove_other_effect<P: EffectProvider + ?Sized>(
        ctx: &mut BattleCtx<'_, P>,
        _r: RelayVar,
        _t: BattlerRef,
        _s: BattlerRef,
        _e: EffectId,
    ) -> HandlerResult {
        // Remove the volatile (id == MOCK_VOLATILE.id) from the arena.
        ctx.effects.retain(|e| e.id != MOCK_VOLATILE.id);
        HandlerResult::Unchanged
    }

    #[test]
    fn handler_removing_effect_midfold_is_snapshot_safe() {
        let provider = TProvider::default();
        let _ = &provider; // unused: hs built directly
        let (mut state, mut effects, mut mv, mut rng) =
            parts(mon(100, TSpecies::Plain), mon(100, TSpecies::Plain));
        effects.push(EffectState {
            id: MOCK_VOLATILE.id,
            host: BattlerRef::PLAYER,
            effect_order: 0,
            kind: TKind::Vol,
        });
        let mut ctx = ctx_from(&mut state, &mut effects, &mut mv, &mut rng);
        // First handler removes the volatile; second still runs from the snapshot.
        let hs = vec![
            CollectedHandler {
                order: 1,
                priority: 0,
                speed: 0,
                sub_order: 6,
                effect_order: 0,
                target: BattlerRef::PLAYER,
                source: BattlerRef::PLAYER,
                source_effect: EffectId(1),
                call: remove_other_effect::<TProvider>,
            },
            CollectedHandler {
                order: 2,
                priority: 0,
                speed: 0,
                sub_order: 6,
                effect_order: 1,
                target: BattlerRef::PLAYER,
                source: BattlerRef::PLAYER,
                source_effect: EffectId(2),
                call: touch_target::<TProvider>,
            },
        ];
        run_event_checked(&mut ctx, hs, RelayVar::Unit, false);
        assert!(effects.is_empty(), "volatile removed mid-fold");
        assert_eq!(
            marker(&state, BattlerRef::PLAYER),
            7,
            "second handler still fired from the owned snapshot (no invalidation)"
        );
    }

    // ── Proof 3: EffectHost routes a Field-hosted residual / scope projection. ──
    #[test]
    fn effect_host_routes_field_and_battler() {
        // Battler-hosted arena entry projects to EffectHost::Battler.
        let es = EffectState::<TProvider> {
            id: EffectId(1),
            host: BattlerRef::PLAYER,
            effect_order: 0,
            kind: TKind::None,
        };
        assert_eq!(es.host_scope(), EffectHost::Battler(BattlerRef::PLAYER));

        // From / PartialEq cross-impls (non-breaking widening proof).
        let h: EffectHost = BattlerRef::OPPONENT.into();
        assert_eq!(h, EffectHost::Battler(BattlerRef::OPPONENT));
        assert!(h == BattlerRef::OPPONENT, "EffectHost == BattlerRef");
        assert!(BattlerRef::OPPONENT == h, "BattlerRef == EffectHost");
        assert_ne!(EffectHost::Side(1), EffectHost::Field);

        // A Field-hosted effect fires via the field_effects resolver path:
        // collect_handlers with field_on=true yields the field hook, and running
        // it marks the target — i.e. a Field-hosted residual is routed.
        let provider = TProvider { field_on: true };
        let (mut state, mut effects, mut mv, mut rng) =
            parts(mon(100, TSpecies::Plain), mon(100, TSpecies::Plain));
        let mut ctx = ctx_from(&mut state, &mut effects, &mut mv, &mut rng);
        let mut hs = Vec::new();
        collect_handlers(
            &ctx,
            &provider,
            None,
            Event::DamagingHit,
            BattlerRef::PLAYER,
            BattlerRef::PLAYER,
            &mut hs,
        );
        assert_eq!(hs.len(), 1, "only the field effect (no ability/item)");
        run_event_checked(&mut ctx, hs, RelayVar::Unit, false);
        assert_eq!(
            marker(&state, BattlerRef::PLAYER),
            100,
            "field residual routed"
        );
    }

    // ── Proof 4: an event with no subscribers is a no-op. ──
    #[test]
    fn event_with_no_subscribers_is_noop() {
        let provider = TProvider::default(); // field off, both Plain
        let (mut state, mut effects, mut mv, mut rng) =
            parts(mon(100, TSpecies::Plain), mon(100, TSpecies::Plain));
        let ctx = ctx_from(&mut state, &mut effects, &mut mv, &mut rng);
        let mut hs = Vec::new();
        collect_handlers(
            &ctx,
            &provider,
            None,
            Event::DamagingHit,
            BattlerRef::OPPONENT,
            BattlerRef::PLAYER,
            &mut hs,
        );
        assert!(hs.is_empty(), "no live source ⇒ no handlers ⇒ no-op");
        drop(ctx);
        assert_eq!(marker(&state, BattlerRef::PLAYER), 0);
        assert_eq!(marker(&state, BattlerRef::OPPONENT), 0);
        let _ = EffectResult::NoEffect; // keep the import meaningful
    }

    // ── Proof 5: default resolvers ⇒ collect_handlers reduces to identity with
    //    the single-source collect_from_effect (byte-identical CollectedHandlers).
    #[test]
    fn default_resolvers_reduce_to_single_source_identity() {
        // A move effect with one DamagingHit hook; both battlers Plain; field off;
        // empty arena ⇒ every resolver default. collect_handlers(src) MUST equal
        // collect_from_effect(src).
        use super::dispatch::collect_from_effect;
        static SRC: Effect<TProvider> = Effect {
            id: EffectId(0x99),
            kind: EffectType::Move,
            hooks: &[EventHook {
                event: Event::DamagingHit,
                call: super::tests_support::mock_src_hit::<TProvider>,
                order: 5,
                priority: 0,
                sub_order: None,
            }],
        };
        let provider = TProvider::default();
        let (mut state, mut effects, mut mv, mut rng) =
            parts(mon(100, TSpecies::Plain), mon(100, TSpecies::Plain));
        let ctx = ctx_from(&mut state, &mut effects, &mut mv, &mut rng);

        let mut single = Vec::new();
        collect_from_effect(
            &ctx,
            &SRC,
            Event::DamagingHit,
            BattlerRef::OPPONENT,
            BattlerRef::PLAYER,
            &mut single,
        );
        let mut multi = Vec::new();
        collect_handlers(
            &ctx,
            &provider,
            Some(&SRC),
            Event::DamagingHit,
            BattlerRef::OPPONENT,
            BattlerRef::PLAYER,
            &mut multi,
        );
        assert_eq!(single.len(), 1);
        assert_eq!(multi.len(), single.len(), "identity: same count");
        for (a, b) in single.iter().zip(multi.iter()) {
            assert_eq!(a.order, b.order);
            assert_eq!(a.priority, b.priority);
            assert_eq!(a.speed, b.speed);
            assert_eq!(a.sub_order, b.sub_order);
            assert_eq!(a.effect_order, b.effect_order);
            assert_eq!(a.target, b.target);
            assert_eq!(a.source, b.source);
            assert_eq!(a.source_effect, b.source_effect);
        }
    }
}

// ─── Effectiveness fold: the engine fires Event::Effectiveness in resolve_action
//     between ModifyDamage and the damage apply, folding RelayVar::Damage via
//     scale(num,den). Game-agnostic, mock-game style (design doc 12 §1.1/§1.3).
//     Proves: (a) no Effectiveness subscriber ⇒ damage unchanged (inert);
//     (b) a subscriber that scale(2,1) doubles, scale(1,2) halves, scale(0,1)
//     zeroes the damage. No game-specific type, no element name, no `rand`. ──────────
#[cfg(test)]
mod effectiveness_fold_tests {
    use super::*;
    use crate::battle::rng::ScriptedRng as EngineScriptedRng;
    use crate::battle::stack::ctx::{BattleCtx, EffectProvider, EffectState};
    use crate::battle::stack::event::{
        Effect, EffectId, EffectType, Event, EventHook, HandlerResult, RelayVar,
    };
    use crate::battle::{
        BattleAction, BattleProvider, BattleState, BattlerRef, BattlerState, DamageResult,
        EffectResult, EnumMap, MoveEffect,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum EStat {
        Hp,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[allow(dead_code)]
    enum EStatus {
        Poisoned,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[allow(dead_code)]
    enum EType {
        N,
    }
    /// The attacker's species encodes WHICH Effectiveness handler the move effect
    /// registers (the engine learns nothing of its meaning — it is an opaque
    /// marker the provider maps to an `&'static Effect`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ESpecies {
        /// Move effect has NO Effectiveness hook ⇒ the fold is inert (1×).
        NoSub,
        /// Move effect's Effectiveness hook does `scale(2,1)` (super-effective).
        Double,
        /// `scale(1,2)` (resisted).
        Half,
        /// `scale(0,1)` (immune).
        Zero,
    }
    #[derive(Debug, Clone, PartialEq)]
    struct EMove {
        power: u8,
    }
    #[derive(Clone)]
    #[allow(dead_code)]
    enum EKind {
        None,
    }

    struct EProvider;

    impl BattleProvider for EProvider {
        type Monster = ();
        type Move = EMove;
        type Ability = ();
        type Status = EStatus;
        type Stat = EStat;
        type Species = ESpecies;
        type Type = EType;
        type Item = ();
        fn calculate_damage(
            &self,
            _m: &Self::Move,
            _a: &BattlerState<Self>,
            _d: &BattlerState<Self>,
            _r: u8,
            _c: bool,
        ) -> DamageResult {
            DamageResult {
                damage: 0,
                effectiveness: 1.0,
                is_miss: false,
            }
        }
        fn select_move(&self, b: &BattlerState<Self>, _s: &BattleState<Self>) -> Self::Move {
            b.moves.first().cloned().unwrap()
        }
        fn apply_move_effect(
            &self,
            _e: MoveEffect,
            _u: &mut BattlerState<Self>,
            _t: &mut BattlerState<Self>,
        ) -> EffectResult {
            EffectResult::NoEffect
        }
        fn create_monster(&self, s: Self::Species, _l: u8) -> BattlerState<Self> {
            BattlerState::new(s, 100, 100, EnumMap::new(), vec![])
        }
    }

    impl EffectProvider for EProvider {
        type EffectStateKind = EKind;
        /// Route the attacker's species marker to the matching move effect. Only
        /// the player (side 0) attacks in these tests, so the player's species
        /// selects the effect; the opponent is the defender (its move is inert
        /// `NoSub` and never resolves because the player KO-or-acts first).
        fn effect_for_move(&self, _m: &Self::Move) -> Option<&'static Effect<Self>> {
            // The driver does not pass the actor's species here; the effect is the
            // SAME static for every move, and its Effectiveness handler reads the
            // SOURCE battler's species off `ctx` to pick the fold (below).
            Some(&EFF_MOVE_EFFECT)
        }
        fn effect_for_status(&self, _s: &Self::Status) -> Option<&'static Effect<Self>> {
            None
        }
        fn turn_order_rank(
            &self,
            _state: &BattleState<Self>,
            who: BattlerRef,
            _action: &Self::Move,
        ) -> (i32, i32) {
            // Player always first (no tie ⇒ no order byte drawn).
            if who.side == 0 {
                (0, 0)
            } else {
                (1, 0)
            }
        }
    }

    /// `ModifyDamage`: set the formula-computed damage to 80 (the base the chart
    /// folds on top of, mirroring doc 12 §4's `atk==def ⇒ base==power` setup).
    fn eff_modify_damage(
        ctx: &mut BattleCtx<'_, EProvider>,
        _r: RelayVar,
        _t: BattlerRef,
        _s: BattlerRef,
        _e: EffectId,
    ) -> HandlerResult {
        ctx.mv.damage = 80;
        HandlerResult::Unchanged
    }

    /// `Effectiveness`: fold the `RelayVar::Damage` relay by the rational the
    /// SOURCE (attacker) battler's species selects. `NoSub` registers no hook so
    /// it never reaches here (proving inertness through a separate effect).
    fn eff_effectiveness(
        ctx: &mut BattleCtx<'_, EProvider>,
        relay: RelayVar,
        _target: BattlerRef,
        source: BattlerRef,
        _e: EffectId,
    ) -> HandlerResult {
        let sp = ctx.battler(source).species;
        let (num, den) = match sp {
            ESpecies::Double => (2u32, 1u32),
            ESpecies::Half => (1, 2),
            ESpecies::Zero => (0, 1),
            ESpecies::NoSub => (1, 1), // never reached (no hook), defensive identity
        };
        HandlerResult::Set(relay.scale(num, den))
    }

    /// The move effect WITHOUT an Effectiveness hook ⇒ the fold is provably inert.
    static EFF_MOVE_EFFECT: Effect<EProvider> = Effect {
        id: EffectId(1),
        kind: EffectType::Move,
        hooks: &[
            EventHook {
                event: Event::ModifyDamage,
                call: eff_modify_damage,
                order: 1000,
                priority: 0,
                sub_order: None,
            },
            EventHook {
                event: Event::Effectiveness,
                call: eff_effectiveness,
                order: 1000,
                priority: 0,
                sub_order: None,
            },
        ],
    };

    /// A provider variant whose move effect has NO Effectiveness subscriber (the
    /// inert case: the Effectiveness fire collects zero handlers).
    struct EProviderNoSub;
    impl BattleProvider for EProviderNoSub {
        type Monster = ();
        type Move = EMove;
        type Ability = ();
        type Status = EStatus;
        type Stat = EStat;
        type Species = ESpecies;
        type Type = EType;
        type Item = ();
        fn calculate_damage(
            &self,
            _m: &Self::Move,
            _a: &BattlerState<Self>,
            _d: &BattlerState<Self>,
            _r: u8,
            _c: bool,
        ) -> DamageResult {
            DamageResult {
                damage: 0,
                effectiveness: 1.0,
                is_miss: false,
            }
        }
        fn select_move(&self, b: &BattlerState<Self>, _s: &BattleState<Self>) -> Self::Move {
            b.moves.first().cloned().unwrap()
        }
        fn apply_move_effect(
            &self,
            _e: MoveEffect,
            _u: &mut BattlerState<Self>,
            _t: &mut BattlerState<Self>,
        ) -> EffectResult {
            EffectResult::NoEffect
        }
        fn create_monster(&self, s: Self::Species, _l: u8) -> BattlerState<Self> {
            BattlerState::new(s, 100, 100, EnumMap::new(), vec![])
        }
    }
    impl EffectProvider for EProviderNoSub {
        type EffectStateKind = EKind;
        fn effect_for_move(&self, _m: &Self::Move) -> Option<&'static Effect<Self>> {
            Some(&EFF_MOVE_NO_SUB_NS)
        }
        fn effect_for_status(&self, _s: &Self::Status) -> Option<&'static Effect<Self>> {
            None
        }
        fn turn_order_rank(
            &self,
            _state: &BattleState<Self>,
            who: BattlerRef,
            _action: &Self::Move,
        ) -> (i32, i32) {
            if who.side == 0 {
                (0, 0)
            } else {
                (1, 0)
            }
        }
    }
    fn eff_modify_damage_ns(
        ctx: &mut BattleCtx<'_, EProviderNoSub>,
        _r: RelayVar,
        _t: BattlerRef,
        _s: BattlerRef,
        _e: EffectId,
    ) -> HandlerResult {
        ctx.mv.damage = 80;
        HandlerResult::Unchanged
    }
    static EFF_MOVE_NO_SUB_NS: Effect<EProviderNoSub> = Effect {
        id: EffectId(2),
        kind: EffectType::Move,
        hooks: &[EventHook {
            event: Event::ModifyDamage,
            call: eff_modify_damage_ns,
            order: 1000,
            priority: 0,
            sub_order: None,
        }],
    };

    fn mon_eff(species: ESpecies) -> BattlerState<EProvider> {
        let mut stats = EnumMap::new();
        stats.set(EStat::Hp, 200);
        BattlerState::new(species, 200, 200, stats, vec![EMove { power: 80 }])
    }
    fn mon_eff_ns(species: ESpecies) -> BattlerState<EProviderNoSub> {
        let mut stats = EnumMap::new();
        stats.set(EStat::Hp, 200);
        BattlerState::new(species, 200, 200, stats, vec![EMove { power: 80 }])
    }

    /// Run one player turn and return the damage the opponent took (200 - hp).
    fn run_player_turn(attacker: ESpecies) -> u16 {
        let mut state = BattleState::new(vec![mon_eff(attacker)], vec![mon_eff(ESpecies::NoSub)]);
        let mut effects: Vec<EffectState<EProvider>> = Vec::new();
        let actions = [
            BattleAction::<EProvider>::Fight {
                move_: EMove { power: 80 },
            },
            BattleAction::<EProvider>::Fight {
                move_: EMove { power: 80 },
            },
        ];
        let mut rng = EngineScriptedRng::new(vec![]);
        StackDriver::execute_turn(&EProvider, &mut state, &mut effects, actions, &mut rng);
        200 - state.opponent_battlers[0].hp
    }

    #[test]
    fn effectiveness_fold_is_inert_with_no_subscriber() {
        // The move effect has ONLY a ModifyDamage hook (no Effectiveness hook), so
        // the engine's Effectiveness fire collects zero handlers and run_event
        // returns RelayVar::Damage(80) unchanged ⇒ identity write-back ⇒ 80.
        let mut state = BattleState::new(
            vec![mon_eff_ns(ESpecies::NoSub)],
            vec![mon_eff_ns(ESpecies::NoSub)],
        );
        let mut effects: Vec<EffectState<EProviderNoSub>> = Vec::new();
        let actions = [
            BattleAction::<EProviderNoSub>::Fight {
                move_: EMove { power: 80 },
            },
            BattleAction::<EProviderNoSub>::Fight {
                move_: EMove { power: 80 },
            },
        ];
        let mut rng = EngineScriptedRng::new(vec![]);
        StackDriver::execute_turn(&EProviderNoSub, &mut state, &mut effects, actions, &mut rng);
        let dmg = 200 - state.opponent_battlers[0].hp;
        assert_eq!(
            dmg, 80,
            "no Effectiveness subscriber ⇒ damage unchanged (inert 1×)"
        );
    }

    #[test]
    fn effectiveness_subscriber_scale_2_1_doubles() {
        assert_eq!(
            run_player_turn(ESpecies::Double),
            160,
            "scale(2,1): 80 → 160"
        );
    }

    #[test]
    fn effectiveness_subscriber_scale_1_2_halves() {
        assert_eq!(run_player_turn(ESpecies::Half), 40, "scale(1,2): 80 → 40");
    }

    #[test]
    fn effectiveness_subscriber_scale_0_1_zeroes() {
        assert_eq!(
            run_player_turn(ESpecies::Zero),
            0,
            "scale(0,1): 80 → 0 (immune)"
        );
    }
}
