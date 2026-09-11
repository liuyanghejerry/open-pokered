//! P6a — a generic, additive per-turn **event log** for stack-driven battles.
//!
//! [`StackDriver::execute_turn`](super::driver::StackDriver::execute_turn) returns
//! only [`StackTurnResult`](super::driver::StackTurnResult) `{ first,
//! second_cancelled }` — enough to *sequence* a turn, but NOT enough for a frontend
//! to *narrate* it ("X used Y!", "Critical hit!", "X fainted!"). A game that renders
//! a turn needs a structured record of WHAT HAPPENED.
//!
//! This module adds that record as a **generic, game-agnostic** [`TurnLog`] of
//! [`TurnEvent`]s, populated by
//! [`StackDriver::execute_turn_logged`](super::driver::StackDriver::execute_turn_logged).
//! It is **ADDITIVE + DEFAULTED**: the plain `execute_turn` is unchanged and draws
//! no log; the logging path runs the SAME turn (identical `rng` draw order,
//! identical final [`BattleState`](crate::battle::BattleState)) and merely records a
//! structural before/after diff at the driver's existing event sites. Every existing
//! battle / test that calls `execute_turn` is byte-identical.
//!
//! The vocabulary is the universal JRPG turn surface — move used, miss, crit,
//! damage, heal, status inflicted/cured, stat-stage change, faint — keyed by the
//! engine's existing generic associated types (`P::Move` / `P::Status` / `P::Stat`)
//! and [`BattlerRef`]. Game-specific PRESENTATION (effectiveness wording, exact
//! phrasing, animation choice) is the frontend's job: the engine reports the
//! structural truth in order, the game translates it to text/animation.

use core::fmt;

use crate::battle::stack::ctx::EffectProvider;
use crate::battle::BattlerRef;

/// One structural event observed during a stack-driven turn (see module docs).
///
/// Generic over the game's [`BattleProvider`](crate::battle::BattleProvider) associated types; the engine never
/// interprets the move/status/stat values — it only records them, in order.
pub enum TurnEvent<P: EffectProvider + ?Sized> {
    /// `actor` began executing `move_` (it passed the `BeforeMove` gate + cost).
    MoveUsed {
        /// The battler whose move executed.
        actor: BattlerRef,
        /// The move that executed (the *effective* move — a lock-in override is
        /// already resolved by the driver before this is recorded).
        move_: P::Move,
    },
    /// `actor`'s move missed (the accuracy / immunity miss branch).
    Missed {
        /// The battler whose move missed.
        actor: BattlerRef,
    },
    /// `actor`'s move was PREVENTED before it executed — a `BeforeMove` gate aborted
    /// it (e.g. asleep / frozen / fully paralyzed / a confusion self-hit) or it could
    /// not pay its resource cost. No `MoveUsed` is logged for a blocked move. The
    /// engine reports only THAT the move was prevented; the game derives the *reason*
    /// from the battler's status / volatiles (e.g. "is fast asleep!").
    Blocked {
        /// The battler whose move was prevented.
        actor: BattlerRef,
    },
    /// `actor`'s move landed a critical hit.
    Crit {
        /// The battler who landed the crit.
        actor: BattlerRef,
    },
    /// `target` lost `amount` HP this step.
    Damaged {
        /// The battler that lost HP.
        target: BattlerRef,
        /// HP lost (> 0).
        amount: u16,
        /// Why the HP was lost — `None` for move damage; `Some(..)` for a residual
        /// tick (burn/poison/toxic/leech). See [`HpChangeCause`].
        cause: Option<HpChangeCause<P>>,
    },
    /// `target` recovered `amount` HP this step.
    Healed {
        /// The battler that gained HP.
        target: BattlerRef,
        /// HP gained (> 0).
        amount: u16,
        /// Why the HP was gained — `None` for a move heal (drain/Recover); `Some(..)`
        /// for a residual drain-to-source (Leech Seed's seeder gain).
        cause: Option<HpChangeCause<P>>,
    },
    /// `target` gained the non-volatile `status`.
    StatusInflicted {
        /// The battler that gained a status.
        target: BattlerRef,
        /// The new non-volatile status.
        status: P::Status,
    },
    /// `target`'s non-volatile status was cleared / cured.
    StatusCured {
        /// The battler whose status was cleared.
        target: BattlerRef,
        /// The status that was cleared.
        status: P::Status,
    },
    /// `target`'s `stat` stage changed by `delta` signed steps.
    StatChanged {
        /// The battler whose stat stage changed.
        target: BattlerRef,
        /// The stat that changed.
        stat: P::Stat,
        /// The signed change in stage (e.g. `+1`, `-2`).
        delta: i8,
    },
    /// `who` fainted (HP reached 0 this step).
    Fainted {
        /// The battler that fainted.
        who: BattlerRef,
    },
}

// Manual `Clone` / `Debug` (NOT derived): a derive would wrongly add `P: Clone` /
// `P: Debug` bounds — `P` is a provider, never `Clone`. The fields are all `Clone` +
// `Debug` via the trait's assoc-type bounds. (Same pattern as `EffectState<P>`.)
impl<P: EffectProvider + ?Sized> Clone for TurnEvent<P> {
    fn clone(&self) -> Self {
        match self {
            Self::MoveUsed { actor, move_ } => Self::MoveUsed {
                actor: *actor,
                move_: move_.clone(),
            },
            Self::Missed { actor } => Self::Missed { actor: *actor },
            Self::Blocked { actor } => Self::Blocked { actor: *actor },
            Self::Crit { actor } => Self::Crit { actor: *actor },
            Self::Damaged {
                target,
                amount,
                cause,
            } => Self::Damaged {
                target: *target,
                amount: *amount,
                cause: cause.clone(),
            },
            Self::Healed {
                target,
                amount,
                cause,
            } => Self::Healed {
                target: *target,
                amount: *amount,
                cause: cause.clone(),
            },
            Self::StatusInflicted { target, status } => Self::StatusInflicted {
                target: *target,
                status: status.clone(),
            },
            Self::StatusCured { target, status } => Self::StatusCured {
                target: *target,
                status: status.clone(),
            },
            Self::StatChanged {
                target,
                stat,
                delta,
            } => Self::StatChanged {
                target: *target,
                stat: *stat,
                delta: *delta,
            },
            Self::Fainted { who } => Self::Fainted { who: *who },
        }
    }
}

impl<P: EffectProvider + ?Sized> fmt::Debug for TurnEvent<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MoveUsed { actor, move_ } => f
                .debug_struct("MoveUsed")
                .field("actor", actor)
                .field("move_", move_)
                .finish(),
            Self::Missed { actor } => f.debug_struct("Missed").field("actor", actor).finish(),
            Self::Blocked { actor } => f.debug_struct("Blocked").field("actor", actor).finish(),
            Self::Crit { actor } => f.debug_struct("Crit").field("actor", actor).finish(),
            Self::Damaged {
                target,
                amount,
                cause,
            } => f
                .debug_struct("Damaged")
                .field("target", target)
                .field("amount", amount)
                .field("cause", cause)
                .finish(),
            Self::Healed {
                target,
                amount,
                cause,
            } => f
                .debug_struct("Healed")
                .field("target", target)
                .field("amount", amount)
                .field("cause", cause)
                .finish(),
            Self::StatusInflicted { target, status } => f
                .debug_struct("StatusInflicted")
                .field("target", target)
                .field("status", status)
                .finish(),
            Self::StatusCured { target, status } => f
                .debug_struct("StatusCured")
                .field("target", target)
                .field("status", status)
                .finish(),
            Self::StatChanged {
                target,
                stat,
                delta,
            } => f
                .debug_struct("StatChanged")
                .field("target", target)
                .field("stat", stat)
                .field("delta", delta)
                .finish(),
            Self::Fainted { who } => f.debug_struct("Fainted").field("who", who).finish(),
        }
    }
}

/// Why a battler's HP changed — carried on [`TurnEvent::Damaged`] / [`TurnEvent::Healed`]
/// so a game can narrate residual ticks distinctly ("hurt by POISON!", "sapped by LEECH
/// SEED!"). `None` (the default) means ordinary move damage/heal — today's behaviour,
/// unchanged. The engine records only what it holds at the residual fire site: the
/// non-volatile [`Status`](crate::battle::BattleProvider::Status), or the volatile's opaque
/// [`EffectStateKind`](EffectProvider::EffectStateKind) for a volatile-hosted residual —
/// the game maps that token back to the concrete volatile (Toxic / Leech Seed / …).
/// Engine-agnostic: the engine stores the token but never interprets it (symmetric with
/// the `Status` variant).
pub enum HpChangeCause<P: EffectProvider + ?Sized> {
    /// A non-volatile status residual (Gen-1 burn / poison chip).
    Status(P::Status),
    /// A volatile-effect residual, carrying the game's opaque per-volatile token
    /// ([`EffectStateKind`](EffectProvider::EffectStateKind)) so the game can tell
    /// WHICH volatile ticked — Toxic ramp vs Leech Seed sap vs Bide unleash.
    Volatile(P::EffectStateKind),
}

// Manual Clone/Debug (a derive would wrongly demand `P: Clone`/`P: Debug`). The kind
// is `Clone` via the `EffectStateKind: Clone` bound; Debug omits it (it has no Debug
// bound — and the parity oracle checks state/consumed, never the log's Debug text).
impl<P: EffectProvider + ?Sized> Clone for HpChangeCause<P> {
    fn clone(&self) -> Self {
        match self {
            Self::Status(s) => Self::Status(s.clone()),
            Self::Volatile(k) => Self::Volatile(k.clone()),
        }
    }
}
impl<P: EffectProvider + ?Sized> fmt::Debug for HpChangeCause<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Status(s) => f.debug_tuple("Status").field(s).finish(),
            Self::Volatile(_) => f.write_str("Volatile(..)"),
        }
    }
}

/// An ordered log of [`TurnEvent`]s for one stack-driven turn.
pub struct TurnLog<P: EffectProvider + ?Sized> {
    /// The events, in the order they occurred this turn.
    pub events: Vec<TurnEvent<P>>,
}

impl<P: EffectProvider + ?Sized> TurnLog<P> {
    /// An empty log.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Append one event.
    pub fn push(&mut self, ev: TurnEvent<P>) {
        self.events.push(ev);
    }

    /// The number of recorded events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether no events were recorded.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl<P: EffectProvider + ?Sized> Default for TurnLog<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: EffectProvider + ?Sized> Clone for TurnLog<P> {
    fn clone(&self) -> Self {
        Self {
            events: self.events.clone(),
        }
    }
}

impl<P: EffectProvider + ?Sized> fmt::Debug for TurnLog<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurnLog")
            .field("events", &self.events)
            .finish()
    }
}
