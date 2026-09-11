//! Generic, game-agnostic battle **turn-execution driver** (P0b).
//!
//! [`BattleDriver::execute_turn`] sits above the existing
//! [`BattleProvider`]/[`BattlerState`](super::BattlerState)/[`BattleState`]
//! abstraction and owns the *control flow* of a single battle turn:
//!
//! 1. Ask the provider for each actor's [`OrderKey`] and **stable-sort** them
//!    ascending (turn order — C4).
//! 2. For each actor in order: run the [`before_move`](BattleProvider::before_move)
//!    pre-move status gate, then execute its action — for `Fight`, an
//!    [`accuracy_check`](BattleProvider::accuracy_check) followed by
//!    [`calculate_damage`](BattleProvider::calculate_damage); for `Switch`,
//!    [`apply_switch`](BattleDriver::apply_switch). Faints are detected after
//!    each hit.
//! 3. Run the [`end_of_turn`](BattleProvider::end_of_turn) residual hook.
//! 4. Fold everything into a [`TurnOutcome`] and detect a [`BattleEnd`].
//!
//! Every *number* and *rule decision* (the damage formula, accuracy/crit math,
//! status semantics, residual ordering, turn-order tie-breaks) lives in the
//! provider; the driver only sequences. All randomness is injected through
//! [`BattleRng`] so the engine never links `rand` and the game controls the
//! exact draw sequence (C2).
//!
//! The driver is **event-sourced**: it returns a `Vec<`[`TurnEvent`]`>` that the
//! game maps onto its own UI/animation state machine, keeping rendering out of
//! the engine and making the driver unit-testable without a screen.

use super::{
    BattleAction, BattleProvider, BattleRng, BattleState, BattlerRef, BattlerState as Battler,
    EffectResult, MoveGate, OrderKey,
};
use core::fmt;

/// A single observable event produced during turn execution.
///
/// Generic over the provider so move identifiers stay game-defined. The game
/// translates these into its own UI / animation steps.
pub enum TurnEvent<P: BattleProvider + ?Sized> {
    /// A battler began its action with the given chosen move.
    MoveUsed {
        /// The acting battler.
        who: BattlerRef,
        /// The move used.
        move_: P::Move,
    },
    /// A battler's action was prevented by the pre-move gate (sleep, freeze,
    /// flinch, full paralysis, recharge, …). Carries the gate's reason.
    ActionPrevented {
        /// The prevented battler.
        who: BattlerRef,
        /// Why it could not act.
        reason: EffectResult,
    },
    /// A move missed (failed [`accuracy_check`](BattleProvider::accuracy_check),
    /// including the Gen-1 1/256 miss).
    Missed {
        /// The attacker.
        who: BattlerRef,
        /// The intended target.
        target: BattlerRef,
    },
    /// Damage was dealt to `target`.
    Damage {
        /// The attacker.
        who: BattlerRef,
        /// The battler that took the damage.
        target: BattlerRef,
        /// Amount of HP lost.
        amount: u16,
        /// Whether this was a critical hit.
        critical: bool,
        /// Type-effectiveness multiplier from the damage calculation.
        effectiveness: f32,
    },
    /// A battler fainted (HP reached 0).
    Faint {
        /// The fainted battler.
        who: BattlerRef,
    },
    /// A battler switched to another party slot.
    Switched {
        /// The acting side.
        side: u8,
        /// Destination slot.
        to_slot: usize,
    },
    /// An end-of-turn residual effect resolved (poison/burn tick, leech, …).
    Residual {
        /// The residual result.
        result: EffectResult,
    },
    /// A generic effect result the game wishes to surface (catch/run handling,
    /// item use, forced actions, …).
    Effect {
        /// The effect result.
        result: EffectResult,
    },
}

impl<P: BattleProvider + ?Sized> fmt::Debug for TurnEvent<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TurnEvent::MoveUsed { who, move_ } => f
                .debug_struct("MoveUsed")
                .field("who", who)
                .field("move_", move_)
                .finish(),
            TurnEvent::ActionPrevented { who, reason } => f
                .debug_struct("ActionPrevented")
                .field("who", who)
                .field("reason", reason)
                .finish(),
            TurnEvent::Missed { who, target } => f
                .debug_struct("Missed")
                .field("who", who)
                .field("target", target)
                .finish(),
            TurnEvent::Damage {
                who,
                target,
                amount,
                critical,
                effectiveness,
            } => f
                .debug_struct("Damage")
                .field("who", who)
                .field("target", target)
                .field("amount", amount)
                .field("critical", critical)
                .field("effectiveness", effectiveness)
                .finish(),
            TurnEvent::Faint { who } => f.debug_struct("Faint").field("who", who).finish(),
            TurnEvent::Switched { side, to_slot } => f
                .debug_struct("Switched")
                .field("side", side)
                .field("to_slot", to_slot)
                .finish(),
            TurnEvent::Residual { result } => {
                f.debug_struct("Residual").field("result", result).finish()
            }
            TurnEvent::Effect { result } => {
                f.debug_struct("Effect").field("result", result).finish()
            }
        }
    }
}

impl<P: BattleProvider + ?Sized> PartialEq for TurnEvent<P>
where
    P::Move: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        use TurnEvent::*;
        match (self, other) {
            (MoveUsed { who: a, move_: ma }, MoveUsed { who: b, move_: mb }) => a == b && ma == mb,
            (ActionPrevented { who: a, reason: ra }, ActionPrevented { who: b, reason: rb }) => {
                a == b && ra == rb
            }
            (Missed { who: a, target: ta }, Missed { who: b, target: tb }) => a == b && ta == tb,
            (
                Damage {
                    who: a,
                    target: ta,
                    amount: am,
                    critical: ca,
                    effectiveness: ea,
                },
                Damage {
                    who: b,
                    target: tb,
                    amount: bm,
                    critical: cb,
                    effectiveness: eb,
                },
            ) => a == b && ta == tb && am == bm && ca == cb && ea == eb,
            (Faint { who: a }, Faint { who: b }) => a == b,
            (
                Switched {
                    side: a,
                    to_slot: ta,
                },
                Switched {
                    side: b,
                    to_slot: tb,
                },
            ) => a == b && ta == tb,
            (Residual { result: a }, Residual { result: b }) => a == b,
            (Effect { result: a }, Effect { result: b }) => a == b,
            _ => false,
        }
    }
}

/// How a battle ended, if it ended this turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattleEnd {
    /// All opponent battlers fainted.
    PlayerWin,
    /// All player battlers fainted.
    PlayerLoss,
    /// The player (or opponent) fled.
    Fled,
    /// A wild monster was caught.
    Caught,
}

/// The result of executing one turn.
pub struct TurnOutcome<P: BattleProvider + ?Sized> {
    /// Ordered events produced this turn, ready to drive the game's UI.
    pub events: Vec<TurnEvent<P>>,
    /// `Some` if the battle ended this turn.
    pub battle_over: Option<BattleEnd>,
}

impl<P: BattleProvider + ?Sized> fmt::Debug for TurnOutcome<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurnOutcome")
            .field("events", &self.events)
            .field("battle_over", &self.battle_over)
            .finish()
    }
}

/// The generic turn-execution driver. Stateless: state goes in, events come out.
pub struct BattleDriver;

impl BattleDriver {
    /// Execute exactly one full battle turn given each side's chosen action
    /// (`actions[0]` = player, `actions[1]` = opponent).
    ///
    /// See the [module docs](self) for the sequencing. The provider supplies all
    /// numbers/rules via its hooks; `rng` supplies all randomness.
    pub fn execute_turn<P: BattleProvider>(
        provider: &P,
        state: &mut BattleState<P>,
        actions: [BattleAction<P>; 2],
        rng: &mut dyn BattleRng,
    ) -> TurnOutcome<P> {
        let mut events: Vec<TurnEvent<P>> = Vec::new();

        // Each actor: (BattlerRef, action). Player first, then opponent — this
        // is also the stable fallback order on a tie.
        let [player_action, opponent_action] = actions;
        let actors = [
            (BattlerRef::PLAYER, player_action),
            (BattlerRef::OPPONENT, opponent_action),
        ];

        // ── 1. Turn order: provider key → engine stable-sort (C4). ──
        // RNG is drawn here (e.g. speed-tie flip) in submission order so the
        // draw sequence is deterministic and game-controlled.
        let mut keyed: Vec<(OrderKey, usize)> = actors
            .iter()
            .enumerate()
            .map(|(idx, (who, action))| (provider.turn_order_key(state, *who, action, rng), idx))
            .collect();
        // Stable sort: equal keys keep submission (player-before-opponent) order.
        keyed.sort_by(|a, b| a.0.cmp(&b.0));

        // ── 2. Resolve each actor in order. ──
        for (_key, idx) in keyed {
            let (who, action) = &actors[idx];
            // Skip actors whose side is already wiped (lead fainted earlier
            // this turn) — they cannot act.
            if Self::side_all_fainted(state, who.side) {
                continue;
            }
            Self::resolve_action(provider, state, *who, action, rng, &mut events);

            // Short-circuit if the battle is already decided mid-turn so we
            // don't run residuals on a finished battle.
            if let Some(battle_over) = Self::detect_end(state) {
                return TurnOutcome {
                    events,
                    battle_over: Some(battle_over),
                };
            }
        }

        // ── 3. End-of-turn residuals (poison/burn/leech/wrap/weather). ──
        for result in provider.end_of_turn(state, rng) {
            events.push(TurnEvent::Residual { result });
        }
        // Surface faints triggered by residual damage.
        Self::push_faints(state, &mut events);

        // ── 4. Battle-end detection. ──
        state.turn_count = state.turn_count.saturating_add(1);
        let battle_over = Self::detect_end(state);

        TurnOutcome {
            events,
            battle_over,
        }
    }

    /// Resolve a single actor's action, appending events.
    fn resolve_action<P: BattleProvider>(
        provider: &P,
        state: &mut BattleState<P>,
        who: BattlerRef,
        action: &BattleAction<P>,
        rng: &mut dyn BattleRng,
        events: &mut Vec<TurnEvent<P>>,
    ) {
        // Pre-move status gate (sleep/freeze/flinch/para/recharge/confusion…).
        let gate = provider.before_move(state, who, action, rng);
        let effective_action: BattleAction<P> = match gate {
            MoveGate::Acts => action.clone(),
            MoveGate::Prevented(reason) => {
                events.push(TurnEvent::ActionPrevented { who, reason });
                return;
            }
            MoveGate::ForcedAction(forced) => forced,
        };

        match effective_action {
            BattleAction::Fight { move_ } => {
                Self::resolve_fight(provider, state, who, move_, rng, events);
            }
            BattleAction::Switch { to_slot } => {
                if Self::apply_switch(state, who.side, to_slot) {
                    events.push(TurnEvent::Switched {
                        side: who.side,
                        to_slot,
                    });
                }
            }
            // Item/Run/Catch resolution is game-specific: the game models it via
            // `before_move`/`end_of_turn` or surfaces the result through the
            // action; the engine has nothing generic to do beyond noting it.
            BattleAction::UseItem { .. } | BattleAction::Run | BattleAction::Nothing => {}
        }
    }

    /// Resolve a `Fight` action: accuracy → damage → faint.
    fn resolve_fight<P: BattleProvider>(
        provider: &P,
        state: &mut BattleState<P>,
        who: BattlerRef,
        move_: P::Move,
        rng: &mut dyn BattleRng,
        events: &mut Vec<TurnEvent<P>>,
    ) {
        let target = Self::opposing(who);

        events.push(TurnEvent::MoveUsed {
            who,
            move_: move_.clone(),
        });

        // Accuracy (incl. the 1/256 miss) — game-owned.
        if !provider.accuracy_check(state, who, target, &move_, rng) {
            events.push(TurnEvent::Missed { who, target });
            return;
        }

        // Critical-hit roll — game-owned (crit rate, Focus Energy, high-crit
        // moves, every Gen-1 quirk). Rolled *before* damage so the result both
        // feeds the formula and surfaces into the `Damage` event below. The
        // default `roll_critical` never crits and draws no rng, so providers
        // that don't override it keep their exact draw sequence.
        let critical = provider.roll_critical(state, who, target, &move_, rng);

        // Damage — game-owned formula (C3). The driver supplies a `random` byte
        // from the injected rng and consumes the returned result; STAB/type and
        // every quirk live inside the provider's `calculate_damage`, which is
        // told whether this is a critical hit.
        let random = rng.next_u8();
        let (Some(attacker), Some(defender)) = (
            Self::battler(state, who).cloned(),
            Self::battler(state, target).cloned(),
        ) else {
            return;
        };
        let dmg = provider.calculate_damage(&move_, &attacker, &defender, random, critical);

        if dmg.is_miss {
            events.push(TurnEvent::Missed { who, target });
            return;
        }

        if let Some(def_mut) = Self::battler_mut(state, target) {
            def_mut.take_damage(dmg.damage);
        }
        events.push(TurnEvent::Damage {
            who,
            target,
            amount: dmg.damage,
            critical,
            effectiveness: dmg.effectiveness,
        });

        if let Some(def) = Self::battler(state, target) {
            if def.hp == 0 {
                events.push(TurnEvent::Faint { who: target });
            }
        }
    }

    /// Switch the active battler on `side` to `to_slot`. Returns `true` on a
    /// successful, in-bounds switch to a non-fainted member.
    pub fn apply_switch<P: BattleProvider>(
        state: &mut BattleState<P>,
        side: u8,
        to_slot: usize,
    ) -> bool {
        let party = match side {
            0 => &mut state.player_battlers,
            _ => &mut state.opponent_battlers,
        };
        if to_slot == 0 || to_slot >= party.len() || party[to_slot].hp == 0 {
            return false;
        }
        party.swap(0, to_slot);
        true
    }

    // ── helpers ──────────────────────────────────────────────────────

    fn opposing(who: BattlerRef) -> BattlerRef {
        BattlerRef::new(if who.side == 0 { 1 } else { 0 }, who.slot)
    }

    fn battler<P: BattleProvider>(state: &BattleState<P>, who: BattlerRef) -> Option<&Battler<P>> {
        let party = if who.side == 0 {
            &state.player_battlers
        } else {
            &state.opponent_battlers
        };
        party.get(who.slot as usize)
    }

    fn battler_mut<P: BattleProvider>(
        state: &mut BattleState<P>,
        who: BattlerRef,
    ) -> Option<&mut Battler<P>> {
        let party = if who.side == 0 {
            &mut state.player_battlers
        } else {
            &mut state.opponent_battlers
        };
        party.get_mut(who.slot as usize)
    }

    /// Is every battler of `side` fainted (or the side empty)?
    fn side_all_fainted<P: BattleProvider>(state: &BattleState<P>, side: u8) -> bool {
        let party = if side == 0 {
            &state.player_battlers
        } else {
            &state.opponent_battlers
        };
        party.is_empty() || party.iter().all(|b| b.hp == 0)
    }

    /// Emit `Faint` events for any lead battler at 0 HP not yet reported.
    fn push_faints<P: BattleProvider>(state: &BattleState<P>, events: &mut Vec<TurnEvent<P>>) {
        for who in [BattlerRef::PLAYER, BattlerRef::OPPONENT] {
            if let Some(b) = Self::battler(state, who) {
                if b.hp == 0 {
                    let already = events
                        .iter()
                        .any(|e| matches!(e, TurnEvent::Faint { who: w } if *w == who));
                    if !already {
                        events.push(TurnEvent::Faint { who });
                    }
                }
            }
        }
    }

    /// Detect battle end from current HP totals.
    fn detect_end<P: BattleProvider>(state: &BattleState<P>) -> Option<BattleEnd> {
        let player_wiped =
            !state.player_battlers.is_empty() && state.player_battlers.iter().all(|b| b.hp == 0);
        let opponent_wiped = !state.opponent_battlers.is_empty()
            && state.opponent_battlers.iter().all(|b| b.hp == 0);
        match (player_wiped, opponent_wiped) {
            (_, true) => Some(BattleEnd::PlayerWin),
            (true, false) => Some(BattleEnd::PlayerLoss),
            (false, false) => None,
        }
    }
}
