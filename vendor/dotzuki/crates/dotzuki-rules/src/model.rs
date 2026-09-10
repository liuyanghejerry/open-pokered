//! The serde/RON data model + the closed primitive vocabulary (doc 11 §1, §1.1;
//! doc 12 §2). Every name in the data (`on:`, `kind:`, ops, selectors,
//! predicates) parses to a **closed** engine concept at LOAD time — an unknown
//! name is a [`LoadError`], never a battle-time surprise (doc 11 §4.2).

use dotzuki_engine::battle::stack::Event;
use serde::{Deserialize, Deserializer};

/// An integer rational `[num, den]` deserialized from the doc's bracket-pair RON
/// syntax (doc 11 `chance:[30,100]`, doc 12 `mult:[2,1]`). RON parses a fixed
/// `[u32; 2]` as a *tuple* (requiring `(..)`), so we deserialize a 2-element list
/// and validate the length — keeping the authored `[num, den]` form exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rational {
    /// Numerator.
    pub num: u32,
    /// Denominator.
    pub den: u32,
}

impl<'de> Deserialize<'de> for Rational {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v: Vec<u32> = Vec::deserialize(d)?;
        if v.len() != 2 {
            return Err(serde::de::Error::invalid_length(
                v.len(),
                &"a 2-element rational [num, den]",
            ));
        }
        Ok(Rational {
            num: v[0],
            den: v[1],
        })
    }
}

/// A statement of which thing in the data layer could not be bound to the closed
/// vocabulary. Every variant is a **load-time** error (doc 11 §4.2: "a malformed
/// record fails at load, never mid-battle").
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LoadError {
    /// The RON text failed to deserialize into the [`Ruleset`] shape.
    #[error("RON parse error: {0}")]
    Ron(String),
    /// A hook's `on:` named an event outside the closed [`Event`] taxonomy.
    #[error("unknown event name in `on:`: {0:?}")]
    UnknownEvent(String),
    /// A type name referenced in the chart / a `HasType` predicate is not in the
    /// ruleset's `types:` list.
    #[error("unknown type name: {0:?}")]
    UnknownType(String),
    /// A `chance:` fraction had a zero denominator (would never gate).
    #[error("invalid chance fraction {0}/{1} (denominator must be > 0)")]
    BadChance(u32, u32),
    /// A status name referenced by `InflictStatus` could not be resolved by the
    /// game's [`RuleBindings`](crate::RuleBindings) at compile time.
    #[error("unknown status name: {0:?}")]
    UnknownStatus(String),
    /// A stat name referenced by `Boost`/`StatIs` could not be resolved.
    #[error("unknown stat name: {0:?}")]
    UnknownStat(String),
    /// A resource name referenced by a `cost:` entry or a `PayResource` op is not
    /// in the ruleset's `resources:` list (the MP/SP/mana cost gate, doc 13 §4).
    #[error("unknown resource name: {0:?}")]
    UnknownResource(String),
}

/// A whole no-code ruleset (doc 11 §1). Flat table of effect records plus the
/// shared `types` vocabulary and the optional `type_chart` relation (doc 12 §2).
#[derive(Debug, Clone, Deserialize)]
pub struct Ruleset {
    /// The opaque stat names; the game's [`RuleBindings`](crate::RuleBindings)
    /// maps these ↔ `P::Stat`. Order is the interned stat index.
    #[serde(default)]
    pub stats: Vec<String>,
    /// The opaque type names; index = interned chart index (doc 12 §3.2).
    #[serde(default)]
    pub types: Vec<TypeName>,
    /// The opaque resource names (MP / SP / mana — doc 13 §4). Order is the
    /// interned resource index, which the game's
    /// [`RuleBindings`](crate::RuleBindings) maps ↔ the engine's opaque resource
    /// id. Empty by default ⇒ no game declares a resource ⇒ the cost gate is inert.
    #[serde(default)]
    pub resources: Vec<String>,
    /// The attacker-type → defender-type → `[num, den]` relation (doc 12 §2).
    #[serde(default)]
    pub type_chart: Vec<TypeChartEntry>,
    /// The effect records (moves / statuses / abilities / items / weather).
    #[serde(default)]
    pub effects: Vec<EffectRecord>,
}

/// A type name string (interned to a chart index by [`Ruleset::type_index`]).
pub type TypeName = String;

/// One `(atk, def, mult)` chart edge (doc 12 §2). `mult` is an integer rational
/// `[num, den]`; `[2,1]` = super-effective, `[1,2]` = resisted, `[0,1]` = immune.
/// Omitted pairs default to `[1,1]` (neutral) at lookup time.
#[derive(Debug, Clone, Deserialize)]
pub struct TypeChartEntry {
    /// Attacking type name (must be in `types:`).
    pub atk: String,
    /// Defending type name (must be in `types:`).
    pub def: String,
    /// `[num, den]` rational.
    pub mult: Rational,
}

/// One effect record (doc 11 §1). `kind` selects which resolver hosts it and the
/// engine [`EffectType`](dotzuki_engine::battle::stack::EffectType); the optional
/// `category`/`power`/`type`/`accuracy` are per-move data the provider's damage
/// formula reads (the engine never sees them).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename = "Effect")]
pub struct EffectRecord {
    /// Opaque effect id string (e.g. `"move.ember"`). Maps to a synthesized
    /// [`EffectId`](dotzuki_engine::battle::stack::EffectId) per hook at compile.
    pub id: String,
    /// Which resolver hosts this effect + the engine `EffectType`.
    pub kind: EffectKind,
    /// Optional per-move category (`Physical`/`Special`) — provider-read data.
    #[serde(default)]
    pub category: Option<String>,
    /// Optional base power — provider-read data.
    #[serde(default)]
    pub power: Option<u32>,
    /// Optional attacking type name — used by `ApplyTypeChart` to recover the
    /// in-flight move's type from `source_effect` (doc 12 §3.3).
    #[serde(rename = "type", default)]
    pub mtype: Option<String>,
    /// Optional accuracy — provider-read data.
    #[serde(default)]
    pub accuracy: Option<u32>,
    /// Optional resource cost of this move (MP / SP / mana — doc 13 §4). Each
    /// entry names a resource from the ruleset's `resources:` list and the amount
    /// to pay. The loader interns the names to resource ids; the engine's cost gate
    /// (the `move_cost` provider hook) reads them before `BeforeMove`. Empty by
    /// default ⇒ no cost ⇒ the gate is inert (the move always costs nothing).
    #[serde(default)]
    pub cost: Vec<ResourceCost>,
    /// The event hooks.
    #[serde(default)]
    pub hooks: Vec<HookRecord>,
}

/// One `(resource, amount)` cost entry on a move (doc 13 §4). `resource` names a
/// resource from the ruleset's `resources:` list; an unknown name is a
/// [`LoadError::UnknownResource`] at LOAD.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename = "Cost")]
pub struct ResourceCost {
    /// The resource name (must be in the ruleset's `resources:` list).
    pub resource: String,
    /// The amount of the resource to pay.
    pub amount: u16,
}

/// One hook = one `(event, ordering, gate, op-list)` (doc 11 §1).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename = "Hook")]
pub struct HookRecord {
    /// The event name; parsed to the closed [`Event`] at load.
    pub on: String,
    /// `on<Event>Order`; LOW first. Default `u32::MAX` (fires last).
    #[serde(default = "default_order")]
    pub order: u32,
    /// `on<Event>Priority`; HIGH first. Default 0.
    #[serde(default)]
    pub priority: i32,
    /// Optional RNG gate `[num, den]`: the op-list runs only if
    /// `ctx.rng.chance(num, den)` (doc 11 §4.1). The draw is consumed
    /// unconditionally so draw order is a pure function of the op-list.
    #[serde(default)]
    pub chance: Option<Rational>,
    /// The closed primitive op-list.
    #[serde(rename = "do", default)]
    pub ops: Vec<Op>,
}

fn default_order() -> u32 {
    u32::MAX
}

/// The five effect kinds (doc 11 §1). Each maps to an
/// [`EffectType`](dotzuki_engine::battle::stack::EffectType) AND to which provider
/// resolver hosts it ([`crate::ResolverKind`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum EffectKind {
    /// A damaging/utility move → `effect_for_move`, `EffectType::Move`.
    Move,
    /// A non-volatile status → `effect_for_status`, `EffectType::Status`.
    Status,
    /// An ability → `effect_for_ability`, `EffectType::Condition`.
    Ability,
    /// A held item → `effect_for_item`, `EffectType::Condition`.
    Item,
    /// Weather/field → `field_effects`, `EffectType::Condition`.
    Weather,
}

/// A target/host selector (doc 11 §1.1). Resolved against the hook's
/// `target`/`source` [`BattlerRef`](dotzuki_engine::battle::BattlerRef)s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Selector {
    /// The dispatch target.
    Target,
    /// The foe of the target (the other side, same slot).
    Foe,
    /// The effect host (alias of `Target` for battler-hosted effects).
    Host,
    /// The dispatch source.
    Source,
}

/// The denominator base for a fraction op (doc 11 §1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum FractionOf {
    /// Fraction of the selector's `max_hp`.
    MaxHp,
    /// Fraction of the selector's current `hp`.
    CurHp,
    /// Fraction of the **damage the in-flight move just dealt** (the move
    /// execution's `ctx.mv.last_damage`; blueprint `15` §2 P2). This is the base
    /// Gen-1 Drain (`HealFraction` of half the damage dealt) and Recoil
    /// (`DamageFraction` of a quarter the damage dealt) read. **Floors at 1** like
    /// the legacy `(dealt / d).max(1)` — a non-zero damage event always drains /
    /// recoils at least 1 (a 0-damage event yields 0). Pure read of `ctx.mv`; no
    /// entropy. Selecting `LastDamage` for a non-on-hit hook yields whatever
    /// `last_damage` holds (0 before any hit), so author it only on `DamagingHit`.
    LastDamage,
}

impl Default for FractionOf {
    fn default() -> Self {
        FractionOf::MaxHp
    }
}

/// A stat reference: a name string, interned to a stat index by the loader and
/// resolved to `P::Stat` by the game binding.
pub type StatRef = String;

/// The source of a [`Op::SetDamage`] value (blueprint `15` §2/§3 — the
/// special/fixed damage moves that **bypass the type chart**: Seismic Toss /
/// Night Shade = the user's level, Dragon Rage = 40, Sonic Boom = 20, Psywave =
/// `rng·(num/den)·level`). Every variant is **pure** (no entropy except the
/// explicit [`RngScaledLevel`](DamageValue::RngScaledLevel) which draws ONE
/// `ctx.rng` byte at the op's ordinal). Game-agnostic: the only game reach is the
/// per-battler level, supplied by
/// [`RuleBindings::battler_level`](crate::RuleBindings::battler_level).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum DamageValue {
    /// A fixed constant (Dragon Rage = 40, Sonic Boom = 20).
    Const(u16),
    /// The `source` selector's level (Seismic Toss / Night Shade).
    UserLevel,
    /// `(rng_byte * num / den * level)` then floored at 1 — the Psywave shape
    /// (`rng · 1.5 · level / 256`, authored as `num=…, den=…`). Draws exactly ONE
    /// `ctx.rng` byte (the SOLE entropy in this op), at the op's ordinal, so the
    /// stream stays a pure function of the op-list.
    RngScaledLevel {
        /// Numerator (Psywave: the ×1.5 → `num=3`).
        num: u32,
        /// Denominator (Psywave: `den=2`, combined with the /256 byte scale).
        den: u32,
    },
}

/// How a status/volatile op computes its **numeric amount** — the sleep or
/// confusion duration, the Toxic counter seed, etc. Every variant is
/// **game-agnostic**: the engine resolves the number (drawing exactly ONE
/// `ctx.rng` byte for the roll variants, at the op's ordinal — the sole
/// entropy), then hands the resolved `u16` to the game's binding, which alone
/// decides what it MEANS (sleep turns vs. confusion turns vs. a counter seed).
/// The game never touches the RNG; the engine never learns the meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum AmountSpec {
    /// A fixed constant (Rest = `2` sleep turns; a Toxic counter seed = `0`).
    Const(u16),
    /// `(rng_byte & mask) + plus` — one byte (Gen-1 confusion = `mask:3, plus:2`
    /// ⇒ 2–5 turns; a `mask:7` sleep-style roll ⇒ 0–7).
    RngMask {
        /// Bit mask applied to the drawn byte.
        mask: u8,
        /// Constant added after masking.
        plus: u8,
    },
    /// Uniformly random in `[lo, hi]`, drawn from the engine's byte stream by
    /// REJECTION sampling (bytes in the skewed tail are re-drawn) so every value
    /// in the span is equiprobable — the Gen-1 sleep counter (1–7) rejects a 0
    /// roll rather than taking `byte % 7` (which oversamples low values).
    RngRange {
        /// Inclusive lower bound.
        lo: u16,
        /// Inclusive upper bound.
        hi: u16,
    },
}

impl Default for AmountSpec {
    /// `Const(0)` — the inert default so an `InflictStatus` authored without an
    /// `amount:` keeps its pre-existing (duration-less) behaviour.
    fn default() -> Self {
        AmountSpec::Const(0)
    }
}

/// A closed predicate (doc 11 §1.1). Used by `unless`/`when`/`cond` guards.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum Predicate {
    /// The selector has the named type (chart membership).
    HasType(String),
    /// The in-flight folded stat equals the named stat (the Sandstorm
    /// `WeatherModifyStat` SpD case, doc 11 §1). Requires the driver to stash the
    /// in-flight stat index in scratch via the binding; see [`crate::run_ops`].
    StatIs(String),
    /// The current relay-as-int is strictly less than `n` (the Clear-Body
    /// `VetoIf(RelayIntLt(0))` pattern, doc 11 §1).
    RelayIntLt(i64),
    /// **The `target` selector has the named volatile live** (blueprint `15` §2 /
    /// §3, the new predicate). The Gen-1 Substitute block on side-status:
    /// `VetoIf(HasVolatile("Substitute"))` (status fails through a Substitute,
    /// `status_effects.rs`). The volatile name is the game's vocabulary; the binding
    /// resolves it against the live `ctx.effects` arena
    /// ([`RuleBindings::has_volatile`](crate::RuleBindings::has_volatile)). Pure
    /// read; no entropy. Game-agnostic: the rules crate names no concrete volatile.
    HasVolatile(String),
    /// **The in-flight move's type equals one of the `target` selector's types**
    /// (blueprint `15` §2 / §3). The Gen-1 burn/freeze/paralyze self-type-immunity
    /// quirk #23: `VetoIf(MoveTypeIsDefenderType)` — a Fire-type can't be burned by
    /// a Fire move, etc. (`status_effects.rs:85/110/135`). The move type is
    /// recovered from the compiled hook's `move_type_index` (the record's `type:`);
    /// the binding answers membership
    /// ([`RuleBindings::move_type_is_defender_type`](crate::RuleBindings::move_type_is_defender_type)).
    /// Pure read; no entropy.
    MoveTypeIsDefenderType,
    /// **The `target` selector currently has the named non-volatile status**
    /// (blueprint `15` §2, the Dream Eater sleep gate
    /// `VetoIf(!TargetHasStatus("sleep"))`). The status name is the game's
    /// vocabulary, resolved by the binding's `status_index_of` (compiled into the
    /// status map) + [`RuleBindings::has_status`](crate::RuleBindings::has_status).
    /// Pure read; no entropy.
    TargetHasStatus(String),
    /// **Logical negation** of a closed predicate — `VetoIf(Not(TargetHasStatus(
    /// "sleep")))` fires when the target is NOT asleep (the Dream Eater gate:
    /// drain only a sleeping target). Generic over any inner predicate; pure.
    Not(Box<Predicate>),
    /// **The `target` selector has ANY non-volatile status.** Lets a compound
    /// status move (`VetoIf(TargetHasAnyStatus)` then `InflictStatus` +
    /// `InflictVolatile`) apply atomically or not at all — the Gen-1 Toxic
    /// "already-statused ⇒ nothing happens" rule. The engine knows no concrete
    /// status; the binding answers via
    /// [`RuleBindings::has_any_status`](crate::RuleBindings::has_any_status).
    /// Pure read; no entropy.
    TargetHasAnyStatus,
    /// **The `source` selector's level is ≥ the `target` selector's level**
    /// (blueprint `15` §2/§3, the OHKO gate). The Gen-1 one-hit-KO connects only
    /// when the user's level is at least the foe's (bug #19: a foe of strictly
    /// higher level is immune). The level is the game's per-battler quantity —
    /// answered by [`RuleBindings::battler_level`](crate::RuleBindings::battler_level)
    /// (the rules crate stays game-agnostic: it reads level only through the binding,
    /// never off `BattlerState` directly), which **defaults to `0`** so a game that
    /// never authors `LevelGE` is unaffected.
    /// Pure read; no entropy. Game-agnostic: the rules crate names no
    /// game-specific level concept — only "a number the binding supplies per battler".
    LevelGE,
    /// **The `source` selector's HP fraction is strictly below `num/den`** (the
    /// wuxia 「血越低攻越高」 self-HP-threshold gate, affinity.md §四 吕醉仙/苏夜).
    /// True iff `source_hp * den < source_max_hp * num` (so `< num/den`); `den`
    /// clamps to ≥1. Reads the SOURCE (the acting battler) directly off `ctx`
    /// (`hp`/`max_hp` are engine fields, no binding needed), so a `ModifyDamage`
    /// hook can scale outgoing damage only when the actor's own HP is low. Pure
    /// read; no entropy. Game-agnostic: a fraction of an engine HP field — no
    /// game-specific/wuxia concept named here.
    SelfHpBelow {
        /// Numerator of the threshold fraction (e.g. `1` for `< 1/2`).
        num: u32,
        /// Denominator of the threshold fraction (clamped to ≥1).
        den: u32,
    },
    /// **The `source` selector currently has the named non-volatile status** —
    /// exactly [`TargetHasStatus`](Predicate::TargetHasStatus) but on the SOURCE
    /// (the acting battler) rather than the dispatch target. Lets a `BeforeMove`
    /// `VetoIf(SourceHasStatus("..."))` skip the HOLDER's OWN move (the wuxia
    /// 眩晕/控制 gate, affinity.md §四). The status name is the game's vocabulary,
    /// resolved by the same `status_index_of` map + the EXISTING
    /// [`RuleBindings::has_status`](crate::RuleBindings::has_status) binding (no new
    /// binding). Pure read; no entropy.
    SourceHasStatus(String),
}

/// The closed primitive op vocabulary (doc 11 §1.1 + doc 12 §3.1). Each variant
/// maps 1:1 to an existing `ctx`/`RelayVar` op. **This closed set is the entire
/// expressiveness budget** (doc 11 §5).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Op {
    /// Writes `ctx.mv.damage` from the provider formula. The provider isn't in
    /// `BattleCtx`, so (exactly like minimon's `move_damage_hook`) the number is
    /// precomputed by the driver into `ctx.mv.damage` before the fold; this op is
    /// the `ModifyDamage` subscription marker and resolves `Unchanged`.
    DealMoveDamage,
    /// `battler_mut(t).take_damage(of * num/den)`. `unless` skips when the
    /// predicate holds (the Sandstorm "non-Rock chip" case).
    DamageFraction {
        /// Numerator.
        num: u32,
        /// Denominator (clamped to ≥1).
        den: u32,
        /// What the fraction is of.
        #[serde(default)]
        of: FractionOf,
        /// The selector to damage.
        target: Selector,
        /// Skip when this predicate holds.
        #[serde(default)]
        unless: Option<Predicate>,
    },
    /// `battler_mut(t).heal(of * num/den)`.
    HealFraction {
        /// Numerator.
        num: u32,
        /// Denominator (clamped to ≥1).
        den: u32,
        /// What the fraction is of.
        #[serde(default)]
        of: FractionOf,
        /// The selector to heal.
        target: Selector,
        /// Skip when this predicate holds.
        #[serde(default)]
        unless: Option<Predicate>,
    },
    /// Set the selector's non-volatile status (the on-hit secondary). For Phase 1
    /// this directly sets `BattlerState.status` via the game binding; the nested
    /// `TrySetStatus` veto cascade is driver orchestration (doc 11 §3, Phase 2).
    InflictStatus {
        /// Status name (resolved by the binding).
        status: String,
        /// The selector to afflict.
        target: Selector,
        /// Optional numeric parameter handed to the binding (e.g. sleep turns).
        /// Defaults to `Const(0)`, so a plain `InflictStatus` is unchanged and
        /// the binding's `set_status_with_amount` default ignores it.
        #[serde(default)]
        amount: AmountSpec,
    },
    /// **Install a game-defined live volatile on the selector** — the generic
    /// counterpart to [`InflictStatus`] for effects that live in the effect
    /// arena rather than the non-volatile status slot (Gen-1 confusion, Leech
    /// Seed, Toxic-counter, flinch). The engine resolves `amount` (one
    /// `ctx.rng` byte for the roll variants), then asks the game's binding to
    /// build the OPAQUE `P::EffectStateKind` for `kind` + `amount`; if the
    /// binding returns one, the engine installs it generically (fresh id, kept
    /// sorted). The engine never learns what the volatile means — only the game
    /// (via `make_volatile`) does. A game with no volatiles returns `None` and
    /// the op is inert.
    InflictVolatile {
        /// The volatile's game-vocabulary name (e.g. `"confusion"`), passed to
        /// the binding — NOT interned (like [`Predicate::HasVolatile`]).
        kind: String,
        /// The selector to afflict.
        target: Selector,
        /// The numeric parameter (turns / counter seed) handed to the binding.
        #[serde(default)]
        amount: AmountSpec,
    },
    /// Apply a stat-stage delta to the selector (the Intimidate request). For
    /// Phase 1 this applies directly via the binding; the nested `TryBoost` veto
    /// (Clear Body) is driver orchestration (doc 11 §3, Phase 2).
    Boost {
        /// Stat name (resolved by the binding).
        stat: String,
        /// Signed stage delta.
        stages: i8,
        /// The selector to boost.
        target: Selector,
    },
    /// `Set(relay.scale(num, den))` (doc 11 §1.1; minimon Sandstorm). `when`
    /// gates the scale on a predicate (else `Unchanged`).
    ScaleRelay {
        /// Numerator.
        num: u32,
        /// Denominator (clamped to ≥1 by `RelayVar::scale`).
        den: u32,
        /// Apply the scale only when ALL these predicates hold.
        #[serde(default)]
        when: Vec<Predicate>,
    },
    /// `Set(Int(v))` — overwrite the relay with a constant int.
    SetRelay(i64),
    /// `Set(Int(relay.as_int() + k))`.
    AddRelay(i64),
    /// `Set(Int(relay.as_int().clamp(lo, hi)))`.
    ClampRelay {
        /// Lower bound.
        lo: i64,
        /// Upper bound.
        hi: i64,
    },
    /// `Fail` when the predicate holds (the Clear-Body veto). `silent` ⇒
    /// `FailSilent` (no "but it failed!" message).
    VetoIf {
        /// The veto condition.
        cond: Predicate,
        /// Suppress the failure message.
        #[serde(default)]
        silent: bool,
    },
    /// `Set(relay.scale(num, den))` from the type chart (doc 12 §3.1). Folds the
    /// dual-type PRODUCT into ONE rational, then a single `scale` (doc 12 §5.3).
    /// The in-flight move's type is recovered from `source_effect`.
    ApplyTypeChart,
    /// **Pay a resource cost (MP / SP / mana — doc 13 §4).** If the selector cannot
    /// pay `amount` of the named resource, the op `Fail`s (the move is prevented via
    /// the existing veto path, exactly like `VetoIf`); otherwise it deducts the
    /// amount and passes the relay through. This expresses the cost gate **in data**
    /// for a hook on `BeforeMove`. The deduction is pure arithmetic — no rng. The
    /// resource name is interned to an id at LOAD (unknown ⇒ [`LoadError`]).
    PayResource {
        /// The resource name (must be in the ruleset's `resources:` list).
        resource: String,
        /// The amount to pay.
        amount: u16,
        /// Who pays (typically `Source` — the acting battler).
        target: Selector,
    },
    /// **Set the selector's HP to a constant** (blueprint `15` §2/§3, the new op).
    /// OHKO writes `SetHp(Foe, 0)`; Explode writes `SetHp(Source, 0)`. The
    /// `when` guard (ALL predicates hold) gates the write — OHKO authors
    /// `when: [LevelGE]` so the KO lands only when the user's level ≥ the foe's
    /// (bug #19); an empty `when` always applies. Pure write of `ctx.battler.hp`;
    /// no entropy. (Unlike `DamageFraction`, this does NOT route through
    /// `take_damage` — it is an absolute set, the faithful Gen-1 OHKO/Explode.)
    SetHp {
        /// The selector whose HP to set.
        target: Selector,
        /// The HP value to set (typically `0`).
        value: u16,
        /// Apply only when ALL these predicates hold (empty ⇒ always).
        #[serde(default)]
        when: Vec<Predicate>,
    },
    /// **Set `ctx.mv.damage` from a [`DamageValue`] source, bypassing the type
    /// chart** (blueprint `15` §2/§3, the new op). The special/fixed damage moves
    /// (Seismic Toss = user level, Dragon Rage = 40, Sonic Boom = 20, Psywave =
    /// `rng·num/den·level`) write the move's damage directly so it rides the same
    /// driver apply path as `DealMoveDamage` (the driver applies `ctx.mv.damage`
    /// to the target after the `ModifyDamage`/`Effectiveness` fold). Authored on
    /// `ModifyDamage` at a HIGH order so it overwrites the formula number. Pure
    /// except [`DamageValue::RngScaledLevel`], which draws ONE `ctx.rng` byte.
    SetDamage {
        /// The damage value source.
        value: DamageValue,
        /// The selector whose level the level-based variants read (the user —
        /// typically `Source`).
        #[serde(default = "default_source_selector")]
        of: Selector,
    },
    /// **Damage the selector by a fraction of its CURRENT HP** (blueprint `15`
    /// §2/§3, the new op). Super Fang = `curHP/2` (floored at 1). Differs from
    /// `DamageFraction { of: CurHp }` in that it ALSO writes `ctx.mv.damage` (so a
    /// Substitute redirect / Counter read sees the real number) and floors a
    /// non-zero result at 1 like the legacy `(curHP/2).max(1)`. Pure read +
    /// `take_damage`; no entropy.
    DamageCurrentHpFraction {
        /// Numerator (Super Fang: `num=1`).
        num: u32,
        /// Denominator (Super Fang: `den=2`, clamped to ≥1).
        den: u32,
        /// The selector to damage.
        target: Selector,
    },
    /// **Re-apply the in-flight move's damage N times — the Gen-1 multi-hit loop,
    /// driven GAME-SIDE with NO engine change** (blueprint `15` §2/§3 "RepeatHits",
    /// P4). Authored on `DamagingHit`, which the StackDriver fires AFTER the FIRST
    /// hit's `take_damage` (driver.rs `resolve_action`). So `ctx.mv.damage` is the
    /// per-hit number the driver already applied ONCE; this op re-applies the SAME
    /// number to `target` `(N-1)` MORE times — the faithful Gen-1 "compute damage
    /// once, deal it N times" (no per-hit recompute). N comes from [`count`]:
    ///   * `Fixed(k)` ⇒ exactly k hits, NO byte (Double Kick / Bonemerang / the
    ///     Twineedle double-hit);
    ///   * `TwoToFive` ⇒ ONE byte folded by the legacy `determine_hit_count`
    ///     distribution (the `multi_hit_roll`).
    /// [`final_hit`] runs Twineedle's final-hit-only secondary (one `chance` byte +
    /// guarded `InflictStatus`) AFTER the last hit, at the legacy `side_effect`
    /// ordinal. This op needs NO engine seam: it loops `take_damage` on the existing
    /// `BattleCtx`, drawing only `ctx.rng` (one count byte + the optional final-hit
    /// chance byte) at its ordinal — so a `ScriptedRng` replays it identically and
    /// `consumed()` is a pure function of the op-list.
    RepeatHits {
        /// Where N comes from (fixed, or the 2-5 distribution draw).
        count: HitCount,
        /// The selector to deal the repeated hits to (the defender — `Target` on a
        /// `DamagingHit` hook).
        target: Selector,
        /// The final-hit secondary (Twineedle poison); `None` for a plain multi-hit.
        #[serde(default)]
        final_hit: FinalHitRider,
    },
    /// **Clear the selector's non-volatile status** (the wuxia cleanse / 驱散
    /// 静心咒·天玑回春 op, affinity.md §六). Resolves the selector, then writes
    /// `ctx.battler_mut(who).status = None` — a generic engine-field write, exactly
    /// like [`SetHp`](Op::SetHp) writes `.hp = value`; no binding, no entropy. The
    /// inverse of [`InflictStatus`](Op::InflictStatus): where that sets a status,
    /// this removes whatever non-volatile status is held. Resolves `Unchanged` (the
    /// relay threads through). A game that never authors `RemoveStatus` is unaffected.
    RemoveStatus {
        /// The selector whose non-volatile status to clear.
        target: Selector,
    },
}

fn default_source_selector() -> Selector {
    Selector::Source
}

/// The number of times a [`Op::RepeatHits`] re-applies the in-flight move's damage
/// (blueprint `15` §2/§3, the new game-side multi-hit construct). Gen-1 multi-hit
/// checks accuracy ONCE then deals the SAME computed damage N times; this enum is
/// the **source of N**. Every variant is game-agnostic — the rules crate names no
/// game-specific concept, only "a count, optionally drawn from one byte".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum HitCount {
    /// A fixed number of hits (Double Kick / Bonemerang / Twineedle = `Fixed(2)`).
    /// Draws NO rng.
    Fixed(u8),
    /// The Gen-1 two-to-five distribution (`TwoToFiveAttacksEffect`): draws ONE
    /// byte and folds it `3/8·3/8·1/8·1/8` over `{2,3,4,5}` — bit-identical to the
    /// legacy `determine_hit_count` (`roll<96⇒2, <192⇒3, <224⇒4, else⇒5`). The byte
    /// is the legacy `multi_hit_roll`; it is the SOLE entropy of this variant, drawn
    /// at the op's ordinal so the stream stays a pure function of the op-list.
    TwoToFive,
}

/// What [`Op::RepeatHits`] does AFTER the final hit lands (Twineedle's
/// final-hit-only poison, blueprint `15` §2). `None` ⇒ a plain multi-hit (Double
/// Kick / Fury Attack). `InflictOnFinal` ⇒ on the LAST hit only, draw ONE
/// `chance` byte and (if it passes the gate) apply the named status to the target —
/// the Twineedle 20%+1 (52/256) poison at the legacy `side_effect` ordinal. The
/// guards (poison-type immunity, Substitute block) are authored as the SAME
/// `VetoIf` ops a side-status move uses, evaluated game-side by the interpreter.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum FinalHitRider {
    /// No final-hit secondary.
    None,
    /// On the final hit only: draw the `chance` byte, then (if it passes) run the
    /// rider ops (`VetoIf` guards + `InflictStatus`) exactly like a side-status
    /// hook. `chance` is `[num, den]`; the byte is drawn UNCONDITIONALLY of whether
    /// the secondary fires (the `consumed()` invariant).
    OnFinal {
        /// The `[num, den]` chance gate (Twineedle = `[52, 256]`).
        chance: Rational,
        /// The rider op-list (guards + `InflictStatus`), run on the final hit if the
        /// gate passes.
        ops: Vec<Op>,
    },
}

impl Default for FinalHitRider {
    fn default() -> Self {
        FinalHitRider::None
    }
}

/// Parse a hook's `on:` string to the closed [`Event`] enum (doc 11 §3). An
/// unknown name is a **load** error. `Custom(N)` is the open tail.
pub fn parse_event(name: &str) -> Result<Event, LoadError> {
    let ev = match name {
        // Group A
        "BeforeTurn" => Event::BeforeTurn,
        "ResidualOrder" => Event::ResidualOrder,
        "AfterTurn" => Event::AfterTurn,
        // Group B
        "BeforeMove" => Event::BeforeMove,
        "ModifyMove" => Event::ModifyMove,
        "ModifyType" => Event::ModifyType,
        "ModifyCritRatio" => Event::ModifyCritRatio,
        "Accuracy" => Event::Accuracy,
        "Invulnerability" => Event::Invulnerability,
        "ModifyDamage" => Event::ModifyDamage,
        "Effectiveness" => Event::Effectiveness,
        "AfterMove" => Event::AfterMove,
        // Group C
        "TryHit" => Event::TryHit,
        "Damage" => Event::Damage,
        "DamagingHit" => Event::DamagingHit,
        "Heal" => Event::Heal,
        "AfterFaint" => Event::AfterFaint,
        // Group D
        "TrySetStatus" => Event::TrySetStatus,
        "AfterSetStatus" => Event::AfterSetStatus,
        "TryBoost" => Event::TryBoost,
        "AfterBoost" => Event::AfterBoost,
        "ModifyStat" => Event::ModifyStat,
        "WeatherModifyStat" => Event::WeatherModifyStat,
        // Group E
        "Start" => Event::Start,
        "End" => Event::End,
        "Faint" => Event::Faint,
        "SwitchIn" => Event::SwitchIn,
        "SwitchOut" => Event::SwitchOut,
        // Group F
        "SetWeather" => Event::SetWeather,
        "FieldResidual" => Event::FieldResidual,
        "SideResidual" => Event::SideResidual,
        // Legacy
        "Residual" => Event::Residual,
        // The open tail: `Custom(N)`.
        other => {
            if let Some(rest) = other
                .strip_prefix("Custom(")
                .and_then(|s| s.strip_suffix(')'))
            {
                rest.trim()
                    .parse::<u16>()
                    .map(Event::Custom)
                    .map_err(|_| LoadError::UnknownEvent(name.to_string()))?
            } else {
                return Err(LoadError::UnknownEvent(name.to_string()));
            }
        }
    };
    Ok(ev)
}

/// Parse an [`EffectKind`] to the engine [`EffectType`] (doc 11 §1). The
/// resolver-host mapping is in [`crate::ResolverKind::from_kind`].
pub fn parse_kind(kind: EffectKind) -> dotzuki_engine::battle::stack::EffectType {
    use dotzuki_engine::battle::stack::EffectType;
    match kind {
        EffectKind::Move => EffectType::Move,
        EffectKind::Status => EffectType::Status,
        // Abilities/items/weather are conditions in the comparator's sub_order.
        EffectKind::Ability | EffectKind::Item | EffectKind::Weather => EffectType::Condition,
    }
}

impl Ruleset {
    /// Parse a `rules.ron` text into a [`Ruleset`]. Pure deserialization; the
    /// closed-vocabulary binding (events, types, stats) happens in
    /// [`crate::CompiledRuleset::compile`].
    ///
    /// The `implicit_some` RON extension is enabled so authors write the
    /// ergonomic `chance:[30,100]` / `unless:HasType("Rock")` (doc 11 §1) rather
    /// than the verbose `Some(...)`. This affects parsing only; it adds no
    /// entropy and no nondeterminism.
    pub fn from_ron(text: &str) -> Result<Self, LoadError> {
        let opts = ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
        opts.from_str(text)
            .map_err(|e| LoadError::Ron(e.to_string()))
    }

    /// Intern a type name to its chart index, or `None` if not in `types:`.
    pub fn type_index(&self, name: &str) -> Option<usize> {
        self.types.iter().position(|t| t == name)
    }

    /// Intern a stat name to its index, or `None` if not in `stats:`.
    pub fn stat_index(&self, name: &str) -> Option<usize> {
        self.stats.iter().position(|s| s == name)
    }

    /// Intern a resource name to its index, or `None` if not in `resources:`
    /// (the MP/SP/mana cost gate, doc 13 §4).
    pub fn resource_index(&self, name: &str) -> Option<usize> {
        self.resources.iter().position(|r| r == name)
    }
}
