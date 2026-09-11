//! Event taxonomy, handler signature, and effect registration types for the
//! Showdown-style effect-stack battle engine (design doc §1).
//!
//! Everything here is **100% game-agnostic**: no game-specific concrete
//! types appear. The game registers `Effect`s carrying `HandlerFn` pointers via
//! the [`EffectProvider`](super::ctx::EffectProvider) trait.
//!
//! ## The broadened taxonomy (design §1, P0b)
//!
//! The enum below is the **multi-gen authoring surface** (design §1.4): the 6
//! groups / 31 kinds + the `Custom(u16)` escape hatch + the legacy `Residual`
//! kept for the Gen-1 slices (see the note on `Residual`). Adding a variant is a
//! **non-breaking engine change**: handlers subscribe by *listing*
//! `EventHook { event: Event::X, .. }` and never `match` the whole enum — the
//! only exhaustive matches live inside the engine driver, which we control. So
//! growing this enum only *offers* new subscription points; it forces no
//! existing handler or game to change. Kinds the engine driver does not yet
//! *fire* are simply never collected for, hence inert (zero behavioral change),
//! exactly as `Start`/`End`/`Faint` were in the POC.

/// The closed taxonomy of dispatch *kinds* (design §1.1, broadened §1.4).
///
/// Events are an enum of keys with **no payload** — the payload rides in a
/// typed [`RelayVar`] threaded through the fold. A closed enum (not a
/// string-keyed bus) keeps the comparator and the parity tests auditable, which
/// the Gen-1 quirks demand; [`Event::Custom`] is the open tail so a game is
/// never *blocked* by the closed set.
///
/// Grouped per design §1.2. Variants beyond the POC's `BeforeMove`/
/// `ModifyCritRatio`/`Accuracy`/`ModifyDamage`/`Damage`/`DamagingHit`/`Residual`
/// are present as **subscription seams** — inert until a driver extension fires
/// them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Event {
    // ── Group A — Turn lifecycle (3) ──────────────────────────────────────
    /// After speed-sort settled, before each move resolves.
    BeforeTurn,
    /// End-of-turn batch ordering (weather tick → status tick → leftovers → …);
    /// drives the Gen-2+ end-of-turn sequence.
    ResidualOrder,
    /// Post-residual cleanup (Gen-5 form reset, counters).
    AfterTurn,

    // ── Group B — Action / move pipeline (9) ──────────────────────────────
    /// Pre-move status gate (sleep/freeze/trap/flinch/recharge/confusion/para/
    /// Truant) — the veto point. In the POC slice, paralysis full-para is gated
    /// here.
    BeforeMove,
    /// Mutate the move in flight (multi-hit count, type override, Normalize).
    ModifyMove,
    /// Per-hit type override (Pixilate/Aerilate, Hidden Power) — split from
    /// `ModifyMove` because abilities re-type *after* the move's own type pick.
    ModifyType,
    /// Numeric fold producing the critical-hit threshold (Focus Energy `/4`,
    /// high-crit `×8`, Super Luck, Razor Claw). Crit is drawn here — *before*
    /// `Accuracy` (design §4).
    ModifyCritRatio,
    /// Accuracy fold returning a `0..=255` INT (the Gen-1 1/256 miss; Compound
    /// Eyes, Hustle, Sand Veil).
    Accuracy,
    /// Fly/Dig/Dive/Bounce gate; `fast_exit` veto.
    Invulnerability,
    /// Numeric fold scaling the final damage (the damage roll lands here; Life
    /// Orb, weather boost, STAB-via-Adaptability).
    ModifyDamage,
    /// Type-effectiveness fold (Gen-1 immunity-as-miss; Levitate, Scrappy,
    /// Tinted Lens, Wonder Guard).
    Effectiveness,
    /// Per-action cleanup (Hyper Beam recharge set, Life Orb recoil, Rocky
    /// Helmet contact damage via the contact flag).
    AfterMove,

    // ── Group C — Hit / damage application (5) ─────────────────────────────
    /// Pre-damage interception veto (Substitute swap-target, Protect/Detect,
    /// Magic Bounce redirect).
    TryHit,
    /// Damage-application fold (Substitute absorb, Disguise, Endure/Sturdy
    /// floor-to-1).
    Damage,
    /// A hit connected — secondary-effect + reactive fire-point (Counter/Bide
    /// read, Static/Flame Body, recoil, drain).
    DamagingHit,
    /// Healing fold (Heal Block veto, Big Root item boost).
    Heal,
    /// Post-KO (Moxie/Beast Boost, Aftermath, Destiny Bond resolution).
    AfterFaint,

    // ── Group D — Status & stat changes (6) ────────────────────────────────
    /// Veto setting a non-volatile status (type immunity, Immunity/Limber,
    /// Safeguard, Substitute block).
    TrySetStatus,
    /// Status applied (Synchronize, Toxic Orb self-status).
    AfterSetStatus,
    /// Veto/modify a stat-stage change (Clear Body, Hyper Cutter, White Smoke).
    TryBoost,
    /// Stat change applied (Defiant/Competitive).
    AfterBoost,
    /// Persistent stat fold for the damage-formula reads (Huge Power, Choice
    /// Band, para ÷4 speed, burn ÷2 atk) — one fold parameterized by `P::Stat`.
    ModifyStat,
    /// Weather/ability stat multipliers that layer *after* `ModifyStat` (Sand
    /// Force, Chlorophyll, Swift Swim).
    WeatherModifyStat,

    // ── Group E — Lifecycle / presence (5) ─────────────────────────────────
    /// An effect was added to a host (volatile applied; ability/item attached
    /// on switch-in).
    Start,
    /// An effect was removed (volatile expired → Thrash self-confuse; item
    /// consumed).
    End,
    /// A battler fainted.
    Faint,
    /// A battler entered — the cross-gen ability/item/hazard fire-point
    /// (Intimidate, Drizzle, Stealth Rock damage, Toxic Spikes).
    SwitchIn,
    /// A battler is leaving (Regenerator, Natural Cure, pursuit, Baton Pass).
    SwitchOut,

    // ── Group F — Field / side (3) ─────────────────────────────────────────
    /// Veto/replace a weather change (Air Lock/Cloud Nine suppress, Damp Rock
    /// duration).
    SetWeather,
    /// Field-hosted end-of-turn tick (weather chip damage, Trick Room countdown,
    /// terrain).
    FieldResidual,
    /// Side-hosted end-of-turn tick (Spikes, Wish, Reflect/Light Screen
    /// countdown).
    SideResidual,

    /// **Accuracy-miss reaction** (the one true Gen-1 core touch, blueprint `15`
    /// §3). Fired by the [`StackDriver`](super::driver::StackDriver) on the
    /// accuracy-miss branch — the point where a move whiffed. Gen-1 Jump Kick /
    /// Hi Jump Kick crash the user for 1 HP here. **Additive + DEFAULTED**: the
    /// driver fires it through the move's own effect, so it is INERT for a move
    /// (and a game) that registers no `OnMiss` hook — every existing slice/game
    /// collects zero handlers for it, so the fold is a no-op and `consumed()` /
    /// the byte stream are byte-identical. (`Custom` could not serve this: the
    /// fire-point is INSIDE the driver's miss branch, which only the engine drives.)
    OnMiss,

    // ── Legacy (kept for the Gen-1 regression slices) ──────────────────────
    /// PER-MOVER end-of-action residual (burn/psn/toxic → leech). Order is
    /// load-bearing (design §6 #7).
    ///
    /// The §1.4 taxonomy folds this into `ResidualOrder`/`FieldResidual`/
    /// `SideResidual`, but the 88 Gen-1 stack-parity slices fire `Residual`
    /// directly — it is kept as an existing variant so those slices compile and
    /// pass **unchanged** (the additive/non-breaking constraint trumps the
    /// rename). New games should prefer the §1.4 kinds.
    Residual,

    // ── The open tail (design §1.4) ────────────────────────────────────────
    /// Game-defined dispatch key. The engine dispatches it like any other event
    /// (collect → sort → fold) but assigns it no built-in meaning — a game's
    /// driver extension fires it. Lets a game add an interaction point WITHOUT
    /// an engine change, at the cost of the closed-set audit guarantee for that
    /// key.
    Custom(u16),
}

/// The typed value threaded through a dispatch fold (design §1.2). Mirrors
/// Showdown's relay variable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RelayVar {
    /// No meaningful payload (side-effecting events such as `Residual`).
    Unit,
    /// A signed integer fold value.
    Int(i64),
    /// Accumulated/last damage.
    Damage(u16),
    /// An accuracy threshold in `0..=255`.
    Accuracy(u8),
    /// A boolean verdict (e.g. "did the gate allow the move?").
    Bool(bool),
}

impl RelayVar {
    /// Read the relay as a signed integer (design §4.1). Non-`Int` ⇒ `0`, so a
    /// handler can fold an int relay without hand-matching every variant.
    pub fn as_int(self) -> i64 {
        if let RelayVar::Int(v) = self {
            v
        } else {
            0
        }
    }

    /// Read the relay as a damage value (design §4.1). Non-`Damage` ⇒ `0`.
    pub fn as_damage(self) -> u16 {
        if let RelayVar::Damage(v) = self {
            v
        } else {
            0
        }
    }

    /// Read the relay as an accuracy threshold (design §4.1). Non-`Accuracy`
    /// ⇒ `0`.
    pub fn as_accuracy(self) -> u8 {
        if let RelayVar::Accuracy(v) = self {
            v
        } else {
            0
        }
    }

    /// Read the relay as a boolean verdict (design §4.1). Non-`Bool` ⇒ `false`.
    pub fn as_bool(self) -> bool {
        matches!(self, RelayVar::Bool(true))
    }

    /// Fold-friendly multiply: scale a numeric relay (`Int`/`Damage`/`Accuracy`)
    /// by `num/den` (design §4.1, the `×1.5` / `×0.5` modifier shape). Damage and
    /// accuracy clamp into their lanes; the result keeps the relay's lane so a
    /// `ModifyDamage`/`Accuracy`/`ModifyStat` fold composes. A non-numeric relay
    /// passes through untouched.
    pub fn scale(self, num: u32, den: u32) -> RelayVar {
        let den = den.max(1);
        match self {
            RelayVar::Int(v) => {
                let scaled = (v as i128) * (num as i128) / (den as i128);
                RelayVar::Int(scaled as i64)
            }
            RelayVar::Damage(v) => {
                let scaled = (v as u64) * (num as u64) / (den as u64);
                RelayVar::Damage(scaled.min(u16::MAX as u64) as u16)
            }
            RelayVar::Accuracy(v) => {
                let scaled = (v as u32) * num / den;
                RelayVar::Accuracy(scaled.min(u8::MAX as u32) as u8)
            }
            other => other,
        }
    }
}

/// The verdict a handler returns, mirroring Showdown's
/// `undefined / value / false / null` (design §1.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HandlerResult {
    /// Relay passes through untouched, continue (Showdown `undefined`).
    Unchanged,
    /// `relay` becomes this value, continue (Showdown returns a value).
    Set(RelayVar),
    /// Relay → falsy: STOP, show "but it failed!" (Showdown `false`).
    Fail,
    /// Relay → falsy: STOP, no message (Showdown `null`).
    FailSilent,
}

/// An effect identifier. The engine treats it opaquely; the game assigns ids to
/// moves/statuses/volatiles. Used as the binary-search key into the
/// `EffectState` arena and to address handler `source_effect`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EffectId(pub u32);

/// The effect category, feeding the comparator's `sub_order` default
/// (design §1.3). Gen-1 leaves this defaulting; kept for generality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectType {
    /// A move's own effect.
    Move,
    /// A non-volatile status (burn/poison/…).
    Status,
    /// A volatile condition (Focus Energy, Substitute, …).
    Condition,
}

impl EffectType {
    /// The Showdown `effectType` table sub-order (design §1.3): lower fires
    /// first. Gen-1 does not exercise this seam, but it is wired for review.
    pub fn sub_order(self) -> u8 {
        match self {
            EffectType::Condition => 2,
            EffectType::Status => 4,
            EffectType::Move => 6,
        }
    }
}

/// A handler is a **zero-capture `fn` pointer** (design §1.2): it cannot capture
/// or alias battle state, so the only mutable path is through `ctx`. Per-effect
/// counter state lives in `EffectState`, not a closure.
///
/// `P` is the game's [`EffectProvider`](super::ctx::EffectProvider).
pub type HandlerFn<P> = fn(
    ctx: &mut super::ctx::BattleCtx<'_, P>,
    relay: RelayVar,
    target: crate::battle::BattlerRef,
    source: crate::battle::BattlerRef,
    source_effect: EffectId,
) -> HandlerResult;

/// One `(Event → HandlerFn)` subscription with its ordering metadata
/// (design §1.5). `order`/`priority`/`sub_order` mirror Showdown's
/// `on<Event>Order` / `on<Event>Priority` / effect-type sub-order.
pub struct EventHook<P: super::ctx::EffectProvider + ?Sized> {
    /// Which event this hook subscribes to.
    pub event: Event,
    /// The native handler.
    pub call: HandlerFn<P>,
    /// `on<Event>Order`; default `u32::MAX` fires last; LOW first.
    pub order: u32,
    /// `on<Event>Priority`; HIGH first.
    pub priority: i32,
    /// Optional explicit sub-order override; `None` ⇒ derive from effect type.
    pub sub_order: Option<u8>,
}

impl<P: super::ctx::EffectProvider + ?Sized> Clone for EventHook<P> {
    fn clone(&self) -> Self {
        Self {
            event: self.event,
            call: self.call,
            order: self.order,
            priority: self.priority,
            sub_order: self.sub_order,
        }
    }
}

impl<P: super::ctx::EffectProvider + ?Sized> Copy for EventHook<P> {}

/// An effect = id/type + a sparse table of hooks (design §1.5). Moves,
/// statuses, abilities, items all share this shape (Showdown's `BasicEffect`).
/// The hook table is `'static` so registrations are zero-alloc constants.
pub struct Effect<P: super::ctx::EffectProvider + ?Sized> {
    /// The effect's id.
    pub id: EffectId,
    /// The effect category (feeds `sub_order`).
    pub kind: EffectType,
    /// The sparse `(Event → HandlerFn)` table.
    pub hooks: &'static [EventHook<P>],
}
