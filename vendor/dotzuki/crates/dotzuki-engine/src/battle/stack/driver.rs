//! The `StackDriver` — the fixed per-turn firing sequence (design §2).
//!
//! Replaces `BattleDriver::execute_turn` for stack-based games. Per turn it:
//! resolves order (with a speed-tie draw), then for each actor fires the move
//! pipeline as a sequence of [`Event`]s through the mover's registered
//! [`Effect`](super::event::Effect), and fires **per-mover** residual with the
//! first-mover-faint short-circuit (the Gen-1 structural choice, design §2 gap
//! #1). All randomness flows through `ctx.rng` at the points the events fire, so
//! the byte-stream draw order is auditable and pinnable to a legacy oracle
//! (design §4).

use crate::battle::rng::BattleRng;
use crate::battle::{BattleAction, BattleProvider, BattleState, BattlerRef, EnumMap};

use super::ctx::{BattleCtx, EffectProvider, EffectState, MoveContext};
use super::dispatch::{collect_from_effect, run_event, CollectedHandler};
use super::event::{EffectId, Event, RelayVar};
use super::log::{TurnEvent, TurnLog};

/// Which side moved first this turn, surfaced for the caller / parity oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstMover {
    /// The player (side 0) moved first.
    Player,
    /// The opponent (side 1) moved first.
    Opponent,
}

/// The outcome of one stack-driven turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StackTurnResult {
    /// Who moved first.
    pub first: FirstMover,
    /// Whether the second mover's action was cancelled (first-mover faint or the
    /// move KO'd the defender — design §2 step 2d).
    pub second_cancelled: bool,
}

/// The stateless stack-based turn driver (design §2).
pub struct StackDriver;

impl StackDriver {
    /// Execute exactly one full battle turn.
    ///
    /// `actions[0]` is the player's chosen action, `actions[1]` the opponent's.
    /// `effects` is the live per-effect-state arena (kept sorted by id); the
    /// driver does not allocate it so per-turn state (toxic counters, …)
    /// persists across turns.
    ///
    /// The firing sequence (design §2):
    /// 1. resolve order → speed-tie draw (via `turn_order_key`)
    /// 2. for actor in [first, second]:
    ///    a. `BeforeMove` gate (may abort — status draws here)
    ///    b. if it acts: `ModifyCritRatio`(+crit draw) → `Accuracy`(+acc draw)
    ///       → `ModifyDamage`(+dmg roll) → `Damage`/`DamagingHit`
    ///    c. fire `Residual` + faint-check FOR THIS ACTOR (per-mover)
    ///    d. if this actor's residual KO'd it, or the move KO'd the defender →
    ///       STOP (cancel the second move)
    pub fn execute_turn<P: EffectProvider>(
        provider: &P,
        state: &mut BattleState<P>,
        effects: &mut Vec<EffectState<P>>,
        actions: [BattleAction<P>; 2],
        rng: &mut dyn BattleRng,
    ) -> StackTurnResult {
        Self::execute_turn_inner(provider, state, effects, actions, rng, None)
    }

    /// Like [`execute_turn`](Self::execute_turn) but also returns a generic
    /// [`TurnLog`] narrating the turn (move used / miss / crit / damage / heal /
    /// status / stat change / faint, in order) for a frontend to render.
    ///
    /// **ADDITIVE + DEFAULTED:** this runs the SAME turn as `execute_turn` — the
    /// identical `rng` draw order and identical final [`BattleState`] — and merely
    /// records a structural before/after diff at the driver's existing event sites.
    /// `execute_turn` is `execute_turn_logged` with the log discarded; the no-log
    /// path is byte-identical (it allocates nothing and observes nothing).
    pub fn execute_turn_logged<P: EffectProvider>(
        provider: &P,
        state: &mut BattleState<P>,
        effects: &mut Vec<EffectState<P>>,
        actions: [BattleAction<P>; 2],
        rng: &mut dyn BattleRng,
    ) -> (StackTurnResult, TurnLog<P>) {
        let mut log = TurnLog::new();
        let result =
            Self::execute_turn_inner(provider, state, effects, actions, rng, Some(&mut log));
        (result, log)
    }

    /// The shared turn body. `log` is `None` for the plain path (zero observation,
    /// byte-identical) and `Some` for the logged path (snapshot + diff at each
    /// event site). All recording is gated behind `log`, so the `None` path runs
    /// exactly the original sequence.
    fn execute_turn_inner<P: EffectProvider>(
        provider: &P,
        state: &mut BattleState<P>,
        effects: &mut Vec<EffectState<P>>,
        actions: [BattleAction<P>; 2],
        rng: &mut dyn BattleRng,
        mut log: Option<&mut TurnLog<P>>,
    ) -> StackTurnResult {
        // ── 1. Turn order (draws the order/speed-tie byte first, like pokered
        //       turn_order.rs:41 / move_execution draw order §4). ──
        let first = Self::resolve_first_mover(provider, state, &actions, rng);
        let (first_ref, second_ref) = match first {
            FirstMover::Player => (BattlerRef::PLAYER, BattlerRef::OPPONENT),
            FirstMover::Opponent => (BattlerRef::OPPONENT, BattlerRef::PLAYER),
        };
        let (first_action, second_action) = match first {
            FirstMover::Player => (&actions[0], &actions[1]),
            FirstMover::Opponent => (&actions[1], &actions[0]),
        };

        // ── 2. First mover acts, then per-mover residual + faint check. ──
        //
        // CROSS-TURN LOCK-IN (design §3/§9): before reading the per-turn chosen
        // action, ask the game whether a live volatile (recorded on a PRIOR turn)
        // forces a different action — Thrash re-issues the lock, Fly strikes on
        // turn 2, Hyper Beam recharge forces `Nothing`. This is the seam that
        // proves a per-turn `[Action; 2]` input is insufficient; it is defaulted
        // to `None` (inert) for every game that registers no forcing volatile.
        let first_effective = provider
            .forced_action(effects, first_ref, first_action)
            .unwrap_or_else(|| first_action.clone());
        // Snapshot before the action (target first, then actor → natural log order:
        // the hit precedes self-effects like recoil). Only when logging.
        let act_pre = Self::snap_pair(state, Self::opposing(first_ref), first_ref, &log);
        let mut mv = MoveContext::default();
        Self::resolve_action(
            provider,
            state,
            effects,
            &mut mv,
            first_ref,
            &first_effective,
            rng,
            log.as_deref_mut(),
        );
        if let Some(l) = log.as_deref_mut() {
            Self::diff_pair(l, state, [Self::opposing(first_ref), first_ref], act_pre);
        }
        // Residual: `residual_and_faint` snapshot-diffs EACH source itself (so each HP
        // change is cause-tagged for narration — status tick precedes a cross-battler
        // drain heal in its per-source order).
        let first_residual_faint = Self::residual_and_faint(
            provider,
            state,
            effects,
            &mut mv,
            first_ref,
            rng,
            log.as_deref_mut(),
        );

        // Step 2d: short-circuit. If this actor's residual KO'd it OR the move
        // KO'd the defender, cancel the second move (design §2 / turn.rs:48-60).
        let defender_dead = Self::is_fainted(state, second_ref);
        if first_residual_faint || defender_dead {
            return StackTurnResult {
                first,
                second_cancelled: true,
            };
        }

        // ── Second mover acts, then its own per-mover residual. ──
        let second_effective = provider
            .forced_action(effects, second_ref, second_action)
            .unwrap_or_else(|| second_action.clone());
        let act_pre2 = Self::snap_pair(state, Self::opposing(second_ref), second_ref, &log);
        let mut mv2 = MoveContext::default();
        Self::resolve_action(
            provider,
            state,
            effects,
            &mut mv2,
            second_ref,
            &second_effective,
            rng,
            log.as_deref_mut(),
        );
        if let Some(l) = log.as_deref_mut() {
            Self::diff_pair(l, state, [Self::opposing(second_ref), second_ref], act_pre2);
        }
        Self::residual_and_faint(
            provider,
            state,
            effects,
            &mut mv2,
            second_ref,
            rng,
            log.as_deref_mut(),
        );

        StackTurnResult {
            first,
            second_cancelled: false,
        }
    }

    /// Resolve a single actor's `Fight` action through the event chain
    /// (design §2 step 2a/2b). Switch/UseItem/Run/Nothing are no-ops for the
    /// POC slice (one damaging move per side).
    fn resolve_action<P: EffectProvider>(
        provider: &P,
        state: &mut BattleState<P>,
        effects: &mut Vec<EffectState<P>>,
        mv: &mut MoveContext,
        actor: BattlerRef,
        action: &BattleAction<P>,
        rng: &mut dyn BattleRng,
        mut log: Option<&mut TurnLog<P>>,
    ) {
        let move_ = match action {
            BattleAction::Fight { move_ } => move_.clone(),
            _ => return,
        };
        let Some(eff) = provider.effect_for_move(&move_) else {
            return;
        };
        let target = Self::opposing(actor);

        let mut ctx = BattleCtx {
            state,
            effects,
            mv,
            rng,
        };

        // 2a. BeforeMove gate (status draws here — para full-para in the POC).
        // The gate handler returns `Fail`/`FailSilent` to abort the move; a
        // `Bool(false)` relay means "cannot act".
        let gate = Self::fire(
            &mut ctx,
            eff,
            Event::BeforeMove,
            target,
            actor,
            RelayVar::Bool(true),
        );
        if matches!(gate, RelayVar::Bool(false) | RelayVar::Unit) {
            // The move was PREVENTED by a BeforeMove gate (asleep / frozen / fully
            // paralyzed / a confusion self-hit). Record it so a frontend can narrate
            // "X is fast asleep!" etc.; no MoveUsed is logged for a blocked move.
            if let Some(l) = log.as_deref_mut() {
                l.push(TurnEvent::Blocked { actor });
            }
            return; // move aborted (e.g. fully paralyzed)
        }

        // 2a′. RESOURCE COST GATE (doc 13 §4 — the MP/SP/mana cost check). Fires
        // AFTER the `BeforeMove` status gate has allowed the move, and BEFORE the
        // crit/accuracy/damage draws, so a move the actor CANNOT pay is prevented
        // (the existing `BeforeMove`/`Fail` prevention path: an early `return`,
        // identical in shape to a fully-paralyzed abort) and consumes NONE of the
        // crit/acc/damage rng either. The check and the deduction are PURE
        // ARITHMETIC — they touch no `rng`. With the default empty cost and the
        // empty-by-default `ResourcePool`, this whole block is INERT (an empty
        // loop), so every existing battle and the stack-parity draw sequence are
        // byte-identical.
        let costs = provider.move_cost(&move_);
        if !costs.is_empty() {
            let actor_b = ctx.battler(actor);
            if !costs
                .iter()
                .all(|(id, amt)| actor_b.can_pay_resource(*id, *amt))
            {
                if let Some(l) = log.as_deref_mut() {
                    l.push(TurnEvent::Blocked { actor });
                }
                return; // cannot pay → move prevented (no rng consumed)
            }
            for (id, amt) in costs {
                ctx.battler_mut(actor).pay_resource(*id, *amt);
            }
        }

        // The move passed the BeforeMove gate + the resource cost → it executes.
        // Record it (the *effective* move; a lock-in override is already resolved
        // by the driver before resolve_action is called). Gated behind `log`.
        if let Some(l) = log.as_deref_mut() {
            l.push(TurnEvent::MoveUsed {
                actor,
                move_: move_.clone(),
            });
        }

        // 2b. crit → accuracy → damage. Crit is drawn BEFORE accuracy
        // (design §4, bug-critical), guaranteed by the FIRE ORDER here — not by
        // handler priority.
        //
        // INVARIANT (DO NOT SWAP THESE TWO `fire` CALLS): `ModifyCritRatio`
        // MUST fire before `Accuracy`, so the crit byte is drawn before the
        // accuracy byte (matching pokered's `MoveRandoms` field order). An audit
        // agent previously swapped these and broke Gen-1 fidelity silently. This
        // ordering is pinned by a STANDING DRAW-ORDER GUARD in pokered-core:
        // `battle::stack_parity::assert_crit_drawn_before_accuracy` (test
        // `crit_is_drawn_before_accuracy`). Swapping these lines makes that test
        // FAIL loudly.
        ctx.mv.is_critical = false;
        Self::fire(
            &mut ctx,
            eff,
            Event::ModifyCritRatio,
            target,
            actor,
            RelayVar::Unit,
        );

        let acc = Self::fire(
            &mut ctx,
            eff,
            Event::Accuracy,
            target,
            actor,
            RelayVar::Bool(true),
        );
        if matches!(acc, RelayVar::Bool(false)) {
            ctx.mv.move_missed = true;
            if let Some(l) = log.as_deref_mut() {
                l.push(TurnEvent::Missed { actor });
            }
            // ── on:Miss seam (blueprint 15 §3, the one true core touch). Fire the
            //    move's own `OnMiss` hook on the accuracy-miss branch — Gen-1 Jump
            //    Kick / Hi Jump Kick crash the user here. ADDITIVE + DEFAULTED: a
            //    move with no `OnMiss` hook collects ZERO handlers, so the fold is a
            //    no-op (no rng drawn, no state change) and the byte stream /
            //    `consumed()` of every existing slice + game stays byte-identical.
            Self::fire(&mut ctx, eff, Event::OnMiss, target, actor, RelayVar::Unit);
            return; // missed
        }

        Self::fire(
            &mut ctx,
            eff,
            Event::ModifyDamage,
            target,
            actor,
            RelayVar::Unit,
        );

        // ── Effectiveness fold (design doc 12 §1.1). Inert at 1× when no handler
        //    subscribes (the empty-`hs` `run_event` returns the relay unchanged,
        //    dispatch.rs `for h in hs` never runs), so the write-back is an
        //    identity no-op and every existing game/test is byte-identical.
        //    - lift the formula-computed number into the Damage lane,
        //    - let handlers fold it via `RelayVar::scale` (chart 2×, 1/2×; 0× = immune),
        //    - write back so the apply at the next line stays the single source of
        //      truth (the number still lives in `ctx.mv.damage`).
        //    Fires AFTER `ModifyDamage` (screen/item/weather precede the chart) and
        //    BEFORE `DamagingHit` (on-hit reactions see the post-effectiveness number).
        let eff_in = RelayVar::Damage(ctx.mv.damage);
        let eff_out = Self::fire(&mut ctx, eff, Event::Effectiveness, target, actor, eff_in);
        ctx.mv.damage = eff_out.as_damage(); // non-Damage relay ⇒ 0 (event.rs as_damage)

        // The hit landed → record a crit if the crit pipeline flagged one. ("Critical
        // hit!" text shows only on a connecting hit, never on a miss — hence here,
        // past the miss return, not at ModifyCritRatio.)
        if ctx.mv.is_critical {
            if let Some(l) = log.as_deref_mut() {
                l.push(TurnEvent::Crit { actor });
            }
        }

        // ── Damage-application fold (`Event::Damage`, taxonomy Group C). A defender
        //    effect may INTERCEPT the incoming HP loss before it lands — Substitute
        //    absorbs it into a proxy HP pool (returning `Set(Damage(0))`), Endure /
        //    Sturdy floor it to 1, Disguise zeroes the first hit. Fires AFTER the
        //    effectiveness fold (the number is final) and BEFORE the hp write.
        //
        //    The FOLDED number is what lands on hp — but `ctx.mv.{damage,last_damage}`
        //    keep the MOVE's real (pre-fold) damage, because the DamagingHit riders read
        //    the number the MOVE dealt, not the post-sink residual: recoil/drain via
        //    `last_damage`, multi-hit re-hits via `damage`, Counter's recorder. (A sub
        //    that absorbs 120 still owes 30 recoil / 60 drain / a 120 second hit.)
        //
        //    ADDITIVE + DEFAULTED: with NO `Damage` subscriber the fold returns the relay
        //    unchanged, so `folded == pre` and this is byte-identical to a plain apply
        //    (`ctx.mv.damage` is never rewritten, no rng drawn, no state change).
        let pre = ctx.mv.damage;
        let folded = Self::fire(
            &mut ctx,
            eff,
            Event::Damage,
            target,
            actor,
            RelayVar::Damage(pre),
        )
        .as_damage();
        if folded > 0 {
            ctx.battler_mut(target).take_damage(folded);
        }
        // `last_damage` = the damage the MOVE dealt (to the mon OR its sink), for the
        // recoil/drain `LastDamage` reads — matching the oracle, which bases them on the
        // real formula number even against a Substitute.
        if pre > 0 {
            ctx.mv.last_damage = pre;
        }
        Self::fire(
            &mut ctx,
            eff,
            Event::DamagingHit,
            target,
            actor,
            RelayVar::Damage(pre),
        );
    }

    /// Fire one event through one effect's hooks and return the folded relay.
    fn fire<P: EffectProvider>(
        ctx: &mut BattleCtx<'_, P>,
        eff: &'static super::event::Effect<P>,
        ev: Event,
        target: BattlerRef,
        source: BattlerRef,
        relay: RelayVar,
    ) -> RelayVar {
        let mut hs: Vec<CollectedHandler<P>> = Vec::new();
        collect_from_effect(ctx, eff, ev, target, source, &mut hs);
        run_event(ctx, hs, relay, false)
    }

    /// Per-mover residual (design §2 step 2c): fire `Residual` on the acting
    /// side's host, then report whether the acting battler fainted.
    ///
    /// Two generic, game-agnostic residual sources fire, in this fixed order:
    ///   1. the actor's **non-volatile status** effect (`effect_for_status`) — the
    ///      slice-1 route (Gen-1 burn/poison live in the `status` byte);
    ///   2. each **live volatile** hosted on the actor (`effect_for_volatile`),
    ///      walked in the arena's stable `id` order (design §3.4: every live
    ///      effect contributes its handlers).
    ///
    /// The engine fixes only this *source* order (status sources, then volatile
    /// sources in arena order) and the per-effect hook `order` within each
    /// `run_event`; it knows NOTHING of which volatile is toxic vs leech, the
    /// `/16`, or the "status damage then leech" Gen-1 sequencing. The game makes
    /// the ASM order hold by stamping its volatiles' arena ids (toxic < leech) and
    /// hook `order` values — so a single `if gen==1` never enters the engine.
    fn residual_and_faint<P: EffectProvider>(
        provider: &P,
        state: &mut BattleState<P>,
        effects: &mut Vec<EffectState<P>>,
        mv: &mut MoveContext,
        actor: BattlerRef,
        rng: &mut dyn BattleRng,
        mut log: Option<&mut TurnLog<P>>,
    ) -> bool {
        use super::log::HpChangeCause;
        let opp = Self::opposing(actor);
        // Each residual SOURCE is snapshot-diffed on its OWN (not once for the whole
        // phase), so the per-source HP change carries its cause — the game narrates
        // "hurt by POISON!" (status) vs "sapped by LEECH SEED!" (volatile) as distinct
        // lines. Byte-identical to the old single-diff for state + consumed (the parity
        // oracle checks those, not the event list); only MULTI-source residual logs
        // split into multiple tagged events.

        // 1. Non-volatile status residual (burn/poison) → `Status(s)`.
        let status = state_status(state, actor);
        if let Some(s) = status {
            if let Some(eff) = provider.effect_for_status(&s) {
                let pre = Self::snap_pair(state, actor, opp, &log);
                {
                    let mut ctx = BattleCtx {
                        state,
                        effects,
                        mv,
                        rng,
                    };
                    // Residual fires "on" the acting battler (host == target == source).
                    Self::fire(&mut ctx, eff, Event::Residual, actor, actor, RelayVar::Unit);
                }
                if let Some(l) = log.as_deref_mut() {
                    let cause = HpChangeCause::Status(s.clone());
                    if let Some(p) = &pre[0] {
                        Self::emit_diff(l, state, actor, p, Some(cause.clone()));
                    }
                    if let Some(p) = &pre[1] {
                        Self::emit_diff(l, state, opp, p, Some(cause));
                    }
                }
            }
        }

        // 2. Volatile residuals, in the arena's stable id order → `Volatile`. Snapshot
        //    the resolved effects FIRST (sorted by arena id, the deterministic RNG-free
        //    order) so a handler that mutates the arena (e.g. a leech KO removing a
        //    volatile) cannot perturb the iteration.
        // Capture each source's opaque `kind` alongside its effect so the per-source HP
        // diff can be tagged `Volatile(kind)` — the game maps it back to Toxic / Leech Seed.
        let mut volatiles: Vec<(
            EffectId,
            P::EffectStateKind,
            &'static super::event::Effect<P>,
        )> = effects
            .iter()
            .filter(|e| e.host == actor)
            .filter_map(|e| {
                provider
                    .effect_for_volatile(&e.kind)
                    .map(|eff| (e.id, e.kind.clone(), eff))
            })
            .collect();
        volatiles.sort_by_key(|(id, _, _)| *id);
        for (_id, kind, eff) in volatiles {
            let pre = Self::snap_pair(state, actor, opp, &log);
            {
                // A prior volatile (e.g. Toxic) may have KO'd the actor this tick. The
                // engine does NOT skip the later volatile (it fires unconditionally);
                // each game handler is responsible for its own post-faint guard.
                let mut ctx = BattleCtx {
                    state,
                    effects,
                    mv,
                    rng,
                };
                Self::fire(&mut ctx, eff, Event::Residual, actor, actor, RelayVar::Unit);
            }
            if let Some(l) = log.as_deref_mut() {
                // Both the actor's Damaged AND the opponent's paired Healed (Leech Seed's
                // drain-to-source) carry the SAME volatile kind.
                if let Some(p) = &pre[0] {
                    Self::emit_diff(
                        l,
                        state,
                        actor,
                        p,
                        Some(HpChangeCause::Volatile(kind.clone())),
                    );
                }
                if let Some(p) = &pre[1] {
                    Self::emit_diff(
                        l,
                        state,
                        opp,
                        p,
                        Some(HpChangeCause::Volatile(kind.clone())),
                    );
                }
            }
        }

        Self::is_fainted(state, actor)
    }

    /// Determine the first mover from the provider's RNG-free
    /// [`turn_order_rank`](EffectProvider::turn_order_rank), drawing **exactly
    /// one** byte iff the ranks tie (the single turn-order RNG site, mirroring
    /// pokered's `order_random` coin flip — design §2/§4). On a tie, `byte < 128`
    /// keeps the player first (matching `turn_order.rs:41`).
    fn resolve_first_mover<P: EffectProvider>(
        provider: &P,
        state: &BattleState<P>,
        actions: &[BattleAction<P>; 2],
        rng: &mut dyn BattleRng,
    ) -> FirstMover {
        let player_move = move_of(&actions[0]);
        let enemy_move = move_of(&actions[1]);
        let player_rank = match player_move {
            Some(m) => provider.turn_order_rank(state, BattlerRef::PLAYER, &m),
            None => (0, 0),
        };
        let enemy_rank = match enemy_move {
            Some(m) => provider.turn_order_rank(state, BattlerRef::OPPONENT, &m),
            None => (0, 0),
        };
        match player_rank.cmp(&enemy_rank) {
            core::cmp::Ordering::Less => FirstMover::Player,
            core::cmp::Ordering::Greater => FirstMover::Opponent,
            core::cmp::Ordering::Equal => {
                // Exact tie → ONE coin-flip byte (the only turn-order draw).
                if rng.next_u8() < 128 {
                    FirstMover::Player
                } else {
                    FirstMover::Opponent
                }
            }
        }
    }

    fn opposing(who: BattlerRef) -> BattlerRef {
        BattlerRef::new(if who.side == 0 { 1 } else { 0 }, who.slot)
    }

    // ── Turn-event log helpers (P6a). All read-only over `state`; only invoked on
    //    the logged path (`execute_turn_logged`). The plain `execute_turn` passes
    //    `None`, so `snap_pair` returns empty snapshots and `diff_pair`/`emit_diff`
    //    are never reached — the no-log path is byte-identical. ──

    /// Snapshot a battler's loggable state, or `None` if the slot is absent.
    fn snapshot<P: EffectProvider>(state: &BattleState<P>, who: BattlerRef) -> Option<Snap<P>> {
        let party = if who.side == 0 {
            &state.player_battlers
        } else {
            &state.opponent_battlers
        };
        party.get(who.slot as usize).map(|b| Snap {
            hp: b.hp,
            status: b.status.clone(),
            stages: b.stat_stages.clone(),
        })
    }

    /// Snapshot an ordered pair of battlers iff `log` is active; else two `None`s
    /// (so the plain path allocates/clones nothing).
    fn snap_pair<P: EffectProvider>(
        state: &BattleState<P>,
        a: BattlerRef,
        b: BattlerRef,
        log: &Option<&mut TurnLog<P>>,
    ) -> [Option<Snap<P>>; 2] {
        if log.is_none() {
            return [None, None];
        }
        [Self::snapshot(state, a), Self::snapshot(state, b)]
    }

    /// Emit the diff for an ordered pair against their pre-action snapshots.
    fn diff_pair<P: EffectProvider>(
        log: &mut TurnLog<P>,
        state: &BattleState<P>,
        order: [BattlerRef; 2],
        pre: [Option<Snap<P>>; 2],
    ) {
        for (who, p) in order.into_iter().zip(pre.into_iter()) {
            if let Some(p) = p {
                Self::emit_diff(log, state, who, &p, None); // move-phase HP change: no cause
            }
        }
    }

    /// Push the structural [`TurnEvent`]s implied by `who`'s state change since
    /// `pre`: HP delta (damage/heal), status delta, stat-stage deltas, then faint.
    fn emit_diff<P: EffectProvider>(
        log: &mut TurnLog<P>,
        state: &BattleState<P>,
        who: BattlerRef,
        pre: &Snap<P>,
        cause: Option<super::log::HpChangeCause<P>>,
    ) {
        let Some(post) = Self::snapshot(state, who) else {
            return;
        };
        // HP delta — tagged with `cause` (None for move damage/heal, Some for a residual).
        if post.hp < pre.hp {
            log.push(TurnEvent::Damaged {
                target: who,
                amount: pre.hp - post.hp,
                cause,
            });
        } else if post.hp > pre.hp {
            log.push(TurnEvent::Healed {
                target: who,
                amount: post.hp - pre.hp,
                cause,
            });
        }
        // Non-volatile status delta.
        if pre.status != post.status {
            match (&pre.status, &post.status) {
                (None, Some(s)) => log.push(TurnEvent::StatusInflicted {
                    target: who,
                    status: s.clone(),
                }),
                (Some(s), None) => log.push(TurnEvent::StatusCured {
                    target: who,
                    status: s.clone(),
                }),
                // A status replaced by a different one: report the new one inflicted.
                (Some(_), Some(s)) => log.push(TurnEvent::StatusInflicted {
                    target: who,
                    status: s.clone(),
                }),
                (None, None) => {}
            }
        }
        // Stat-stage deltas: keys present after, then keys that were cleared.
        for (stat, &after) in post.stages.iter() {
            let before = pre.stages.get(*stat).copied().unwrap_or(0);
            if after != before {
                log.push(TurnEvent::StatChanged {
                    target: who,
                    stat: *stat,
                    delta: after - before,
                });
            }
        }
        for (stat, &before) in pre.stages.iter() {
            if post.stages.get(*stat).is_none() && before != 0 {
                log.push(TurnEvent::StatChanged {
                    target: who,
                    stat: *stat,
                    delta: -before,
                });
            }
        }
        // Faint last — after the damage that caused it.
        if pre.hp > 0 && post.hp == 0 {
            log.push(TurnEvent::Fainted { who });
        }
    }

    fn is_fainted<P: EffectProvider>(state: &BattleState<P>, who: BattlerRef) -> bool {
        let party = if who.side == 0 {
            &state.player_battlers
        } else {
            &state.opponent_battlers
        };
        party
            .get(who.slot as usize)
            .map(|b| b.hp == 0)
            .unwrap_or(true)
    }
}

/// A before-action snapshot of one battler's loggable state (P6a). Taken only on
/// the logged path; diffed against the post-action state to derive [`TurnEvent`]s.
struct Snap<P: BattleProvider + ?Sized> {
    hp: u16,
    status: Option<P::Status>,
    stages: EnumMap<P::Stat, i8>,
}

/// Extract the chosen move from a `Fight` action (other actions have no move).
fn move_of<P: BattleProvider + ?Sized>(action: &BattleAction<P>) -> Option<P::Move> {
    match action {
        BattleAction::Fight { move_ } => Some(move_.clone()),
        _ => None,
    }
}

fn state_status<P: EffectProvider>(state: &BattleState<P>, who: BattlerRef) -> Option<P::Status> {
    let party = if who.side == 0 {
        &state.player_battlers
    } else {
        &state.opponent_battlers
    };
    party.get(who.slot as usize).and_then(|b| b.status.clone())
}
