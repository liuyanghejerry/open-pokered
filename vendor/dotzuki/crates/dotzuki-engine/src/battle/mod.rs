//! Battle system trait definitions for a JRPG engine framework.
//!
//! This module defines the core abstractions for turn-based battle systems:
//! battle state, battler state, type charts, AI decision-making, and
//! move effect handling. All types are generic over a [`BattleProvider`]
//! implementation — the engine is game-agnostic.
//!
//! ## Architecture
//!
//! * **`BattleProvider`** — Central trait supplying battle data, damage formula,
//!   and factory methods. All methods take `&self` (read-only provider).
//! * **`TypeChart`** — N×N type effectiveness matrix, parameterized by game types.
//! * **`BattleAI`** — Move selection, switching, and item-use decisions.
//! * **`EffectHandler`** — Dispatches move effects (damage, healing, status, stat
//!   changes) on battler state.
//!
//! ## Design Principles
//!
//! * **Generic over game data** — No concrete game-specific, monster, or move types.
//!   All identifiers are associated types on `BattleProvider`.
//! * **Provider pattern** — Battle systems query the provider, which delegates
//!   to type charts, AI, and effect handlers internally.
//! * **No I/O, no platform** — Pure data and trait definitions only.

use core::fmt;

pub mod ai;
pub mod driver;
pub mod rng;
pub mod stack;

pub use ai::{BattleAi, BattleAiProvider};
pub use driver::{BattleDriver, BattleEnd, TurnEvent, TurnOutcome};
pub use rng::BattleRng;

// ─── EnumMap ───────────────────────────────────────────────────────────

/// A map from enum-like keys to values, backed by a `Vec` of `(K, V)` pairs.
///
/// Designed for use with small, game-specific enums (e.g. stat IDs, type IDs).
/// Lookups are O(n) linear scan — acceptable for N ≤ 15.
///
/// # Type Parameters
///
/// * `K` — Key type, typically a copyable game-enum variant (`Copy + PartialEq`).
/// * `V` — Value type.
pub struct EnumMap<K, V> {
    entries: Vec<(K, V)>,
}

impl<K: PartialEq, V> EnumMap<K, V> {
    /// Create an empty map.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Insert or update a key-value pair.
    pub fn set(&mut self, key: K, value: V) {
        if let Some(entry) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            entry.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    /// Look up a value by key. Returns `None` if the key is not present.
    pub fn get(&self, key: K) -> Option<&V> {
        self.entries.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    /// Look up a value by key. Returns `None` if the key is not present.
    pub fn get_mut(&mut self, key: K) -> Option<&mut V> {
        self.entries
            .iter_mut()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
    }

    /// Number of entries in the map.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the map contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over `(key, value)` pairs in insertion order. Used by the turn-event
    /// log to diff stat-stage maps before/after an action.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }
}

impl<K: PartialEq, V> Default for EnumMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Clone + PartialEq, V: Clone> Clone for EnumMap<K, V> {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
        }
    }
}

impl<K: fmt::Debug + PartialEq, V: fmt::Debug> fmt::Debug for EnumMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.entries.iter().map(|(k, v)| (k, v)))
            .finish()
    }
}

// ─── ResourcePool ──────────────────────────────────────────────────────

/// A per-battler pool of generic, game-defined **consumable resources** (MP / SP
/// / TP / mana / charge — the engine assigns the *concept* no name; doc 13 §4,
/// the highest-value STATE-SEAM).
///
/// ## Why a `u16`-keyed container, not `EnumMap<P::Resource>`
///
/// The doc's sketch suggested `EnumMap<P::Resource, u16>` on `BattlerState`. That
/// would require a new associated type `Resource` on [`BattleProvider`]. A
/// *non-defaulted* assoc type breaks all 16 existing `impl BattleProvider` blocks
/// (an unacceptable, non-additive change), and a *defaulted* assoc type
/// (`type Resource: … = …;`) is **unstable on stable Rust** (`E0658`,
/// `associated_type_defaults`, issue #29661) — so it cannot ship on the
/// workspace's stable toolchain either. The fully-additive choice is therefore a
/// **`P`-independent** container keyed by a small **opaque integer resource id**
/// (the game assigns ids; the engine never interprets them). This adds a single
/// field to `BattlerState` and **zero** changes to the `BattleProvider` trait or
/// any of its 16 impls.
///
/// Each entry tracks `(current, max)`. The pool **defaults to EMPTY**, so a
/// battler that declares no resource behaves byte-for-byte as before. Lookups are
/// an O(n) linear scan, like [`EnumMap`] — pools are tiny.
#[derive(Default)]
pub struct ResourcePool {
    /// `(resource_id, current, max)` triples. Game-assigned ids; opaque to the engine.
    entries: Vec<(u16, u16, u16)>,
}

impl ResourcePool {
    /// An empty pool (the default — a battler with no declared resources).
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Set `id`'s current value and max (inserting the entry if absent). The
    /// current value is clamped to `max`.
    pub fn set(&mut self, id: u16, current: u16, max: u16) {
        let current = current.min(max);
        if let Some(e) = self.entries.iter_mut().find(|(k, _, _)| *k == id) {
            e.1 = current;
            e.2 = max;
        } else {
            self.entries.push((id, current, max));
        }
    }

    /// The current value of resource `id`, or `None` if the battler has no such
    /// resource. A `None` resource is treated as "cannot pay any positive cost".
    pub fn current(&self, id: u16) -> Option<u16> {
        self.entries
            .iter()
            .find(|(k, _, _)| *k == id)
            .map(|(_, cur, _)| *cur)
    }

    /// The max value of resource `id`, or `None` if absent.
    pub fn max(&self, id: u16) -> Option<u16> {
        self.entries
            .iter()
            .find(|(k, _, _)| *k == id)
            .map(|(_, _, m)| *m)
    }

    /// Whether the battler can pay `amount` of resource `id`. A `0` cost is always
    /// payable (even with no such resource — it is inert). A positive cost on a
    /// resource the battler does not have is **not** payable.
    pub fn can_pay(&self, id: u16, amount: u16) -> bool {
        if amount == 0 {
            return true;
        }
        self.current(id).map(|cur| cur >= amount).unwrap_or(false)
    }

    /// Deduct `amount` of resource `id` (saturating at 0). No-op for a `0` amount
    /// or an absent resource. Returns `true` if a deduction was applied. **Pure
    /// arithmetic — consumes no randomness.**
    pub fn pay(&mut self, id: u16, amount: u16) -> bool {
        if amount == 0 {
            return false;
        }
        if let Some(e) = self.entries.iter_mut().find(|(k, _, _)| *k == id) {
            e.1 = e.1.saturating_sub(amount);
            true
        } else {
            false
        }
    }

    /// Restore `amount` of resource `id` (clamped to its max). No-op for an absent
    /// resource.
    pub fn restore(&mut self, id: u16, amount: u16) {
        if let Some(e) = self.entries.iter_mut().find(|(k, _, _)| *k == id) {
            e.1 = e.1.saturating_add(amount).min(e.2);
        }
    }

    /// Number of distinct resources in the pool.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the pool declares no resources (the default — fully inert).
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Clone for ResourcePool {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
        }
    }
}

impl fmt::Debug for ResourcePool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.entries.iter().map(|(k, cur, max)| (k, (cur, max))))
            .finish()
    }
}

// ─── DamageResult ──────────────────────────────────────────────────────

/// Result of a damage calculation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageResult {
    /// Final damage after all modifiers.
    pub damage: u16,
    /// Type effectiveness multiplier (1.0 = neutral, 2.0 = super effective).
    pub effectiveness: f32,
    /// Whether the move missed (effectiveness = 0 or accuracy failed).
    pub is_miss: bool,
}

// ─── MoveEffect ────────────────────────────────────────────────────────

/// Categories of additional effects a move can have.
///
/// This enum classifies post-damage or primary effects so that effect
/// handlers can dispatch to the appropriate logic. The concrete parameters
/// (power, type, status details) are obtained from the provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveEffect {
    /// Pure damage, no additional effect.
    Damage,
    /// Heals the user.
    Heal,
    /// Inflicts a status condition on the target.
    StatusCondition,
    /// Raises or lowers a stat stage.
    StatChange,
    /// Hits multiple times in one turn.
    MultiHit,
    /// User must recharge next turn.
    Recharge,
    /// Drains HP from the target.
    DrainHp,
    /// User takes recoil damage.
    Recoil,
    /// Has a chance to flinch the target.
    Flinch,
    /// Sets or changes field conditions (weather, terrain, screens).
    FieldEffect,
    /// Fixed-damage special move (ignores normal formula).
    SpecialDamage,
    /// One-hit KO attempt.
    Ohko,
    /// Multi-turn move (charge turn, trapping, etc.).
    MultiTurn,
}

// ─── EffectResult ──────────────────────────────────────────────────────

/// Outcome of applying a move effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectResult {
    /// No effect triggered.
    NoEffect,
    /// Damage was dealt.
    DamageDealt { amount: u16 },
    /// HP was healed.
    Healed { amount: u16 },
    /// A status condition was inflicted.
    StatusInflicted,
    /// Status infliction failed (immune, already afflicted, etc.).
    StatusFailed,
    /// A stat stage was modified.
    StatModified { stages: i8 },
    /// Stat modification was blocked (e.g. by a protective effect).
    StatBlocked,
    /// HP was drained from the target and healed to the user.
    HpDrained { drained: u16 },
    /// The user took recoil damage.
    RecoilDamage { recoil: u16 },
    /// The target fainted.
    Fainted,
    /// The move missed.
    Miss,
    /// The move landed as a critical hit.
    CriticalHit,
    /// The move hit multiple times.
    MultiHit { hits: u8 },
    /// The user must recharge next turn.
    MustRecharge,
    /// A field effect was set up.
    FieldEffectSet,
}

// ─── Weather / Terrain ─────────────────────────────────────────────────

/// Field weather condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Weather {
    /// No special weather.
    #[default]
    Clear,
    /// Rain — may boost water moves, weaken fire moves.
    Rain,
    /// Intense sunlight — may boost fire moves, weaken water moves.
    Sun,
    /// Sandstorm — deals residual damage to non-rock/ground/steel types.
    Sandstorm,
    /// Hail/Snow — deals residual damage to non-ice types.
    Snow,
}

/// Field terrain condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Terrain {
    /// Normal ground.
    #[default]
    Normal,
    /// Electrified terrain.
    Electric,
    /// Overgrown terrain.
    Grassy,
    /// Mist-covered terrain.
    Misty,
    /// Psychic-charged terrain.
    Psychic,
}

// ─── Turn-driver support types (P0b) ───────────────────────────────────

/// A reference to one battler position on the battlefield.
///
/// In a 1v1 Gen-1-style battle `slot` is always `0`; the field is present so
/// the same type serves multi-slot formats. `side` is `0` for the player and
/// `1` for the opponent, matching `BattleState::player_battlers` /
/// `opponent_battlers`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BattlerRef {
    /// `0` = player side, `1` = opponent side.
    pub side: u8,
    /// Slot within the side (0-based). `0` for 1v1.
    pub slot: u8,
}

impl BattlerRef {
    /// Construct a battler reference.
    pub const fn new(side: u8, slot: u8) -> Self {
        Self { side, slot }
    }

    /// The player's lead battler (`side 0, slot 0`).
    pub const PLAYER: Self = Self { side: 0, slot: 0 };
    /// The opponent's lead battler (`side 1, slot 0`).
    pub const OPPONENT: Self = Self { side: 1, slot: 0 };
}

/// A turn-ordering sort key produced by [`BattleProvider::turn_order_key`].
///
/// The engine **stable-sorts actors ascending** by this key, so the game must
/// encode "acts earlier" as a *smaller* key. The tuple is ordered
/// `(priority, speed, tiebreak)`:
///
/// * `priority` — move/action priority bracket. Higher-priority actions act
///   first, so the game should store the *negated* bracket here (e.g. `-1` for
///   a +1 priority move) to make ascending order put them first.
/// * `speed` — likewise the *negated* effective speed, so the faster battler
///   (larger speed → more-negative key) sorts first. The game applies its own
///   speed modifiers (paralysis cut, badge boosts) before negating.
/// * `tiebreak` — a game-supplied tie-break value (e.g. a coin-flip drawn from
///   the injected RNG for the Gen-1 speed tie). Smaller acts first.
///
/// Stability of the sort guarantees that equal keys preserve submission order,
/// so a game that wants pure submission order on ties can leave `tiebreak` at
/// `0`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrderKey(pub i32, pub i32, pub u32);

/// The action a battler has chosen to take this turn.
///
/// Generic over the provider so the concrete move/item identifiers stay
/// game-defined. Catch/Run resolution is game-specific: a game models them as
/// `UseItem`/`Run` and decides the [`BattleEnd`](driver::BattleEnd) outcome
/// inside its hooks.
pub enum BattleAction<P: BattleProvider + ?Sized> {
    /// Use a move against the implied target (the opposing lead in 1v1).
    Fight {
        /// The chosen move.
        move_: P::Move,
    },
    /// Switch the active battler to another party slot on the same side.
    Switch {
        /// Destination slot within the acting side's party.
        to_slot: usize,
    },
    /// Use an item (potions, status cures, capture devices, …).
    UseItem {
        /// The chosen item.
        item: P::Item,
    },
    /// Attempt to flee the battle.
    Run,
    /// Do nothing this turn (e.g. recharging, forced inaction handled upstream).
    Nothing,
}

impl<P: BattleProvider + ?Sized> Clone for BattleAction<P> {
    fn clone(&self) -> Self {
        match self {
            BattleAction::Fight { move_ } => BattleAction::Fight {
                move_: move_.clone(),
            },
            BattleAction::Switch { to_slot } => BattleAction::Switch { to_slot: *to_slot },
            BattleAction::UseItem { item } => BattleAction::UseItem { item: item.clone() },
            BattleAction::Run => BattleAction::Run,
            BattleAction::Nothing => BattleAction::Nothing,
        }
    }
}

impl<P: BattleProvider + ?Sized> fmt::Debug for BattleAction<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BattleAction::Fight { move_ } => f.debug_struct("Fight").field("move_", move_).finish(),
            BattleAction::Switch { to_slot } => {
                f.debug_struct("Switch").field("to_slot", to_slot).finish()
            }
            BattleAction::UseItem { item } => {
                f.debug_struct("UseItem").field("item", item).finish()
            }
            BattleAction::Run => write!(f, "Run"),
            BattleAction::Nothing => write!(f, "Nothing"),
        }
    }
}

/// Result of [`BattleProvider::before_move`]: the pre-move status gate.
///
/// The game owns every Gen-1 quirk here — sleep counter decremented *before*
/// the move, freeze, the full-paralysis roll, flinch, Hyper Beam recharge,
/// confusion self-hit, and partial-trap continuation — and reports the outcome
/// to the engine, which only sequences it.
pub enum MoveGate<P: BattleProvider + ?Sized> {
    /// The battler may act with its chosen action.
    Acts,
    /// The battler is prevented from acting; the carried [`EffectResult`]
    /// (e.g. `Miss`, `NoEffect`) is surfaced as a [`TurnEvent`](driver::TurnEvent).
    Prevented(EffectResult),
    /// The chosen action is replaced by a forced one (confusion self-hit,
    /// thrash/petal-dance continuation, …). The engine executes the forced
    /// action instead.
    ForcedAction(BattleAction<P>),
}

impl<P: BattleProvider + ?Sized> fmt::Debug for MoveGate<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MoveGate::Acts => write!(f, "Acts"),
            MoveGate::Prevented(r) => f.debug_tuple("Prevented").field(r).finish(),
            MoveGate::ForcedAction(a) => f.debug_tuple("ForcedAction").field(a).finish(),
        }
    }
}

// ─── BattleProvider ────────────────────────────────────────────────────

/// Central trait for battle system data and formula implementations.
///
/// Implementations provide concrete types for monsters, moves, abilities,
/// statuses, stats, species, and types, along with methods for damage
/// calculation, monster creation, move selection, and effect application.
///
/// All methods take `&self` — the provider is a read-only data source.
///
/// # Associated Types
///
/// | Type       | Purpose                                              |
/// |------------|------------------------------------------------------|
/// | `Monster`  | Full monster/character data (species + stats + HP)   |
/// | `Move`     | Move identifier (ID or inline data struct)           |
/// | `Ability`  | Passive ability identifier                           |
/// | `Status`   | Status condition identifier (burn, sleep, etc.)      |
/// | `Stat`     | Stat identifier (HP, ATK, DEF, SPD, etc.)           |
/// | `Species`  | Species/monster-type identifier                      |
/// | `Type`     | Elemental type identifier                            |
/// | `Item`     | Usable item identifier                               |
pub trait BattleProvider {
    /// The monster data type.
    type Monster: Clone + fmt::Debug;
    /// The move identifier or data type.
    type Move: Clone + fmt::Debug;
    /// The ability identifier type.
    type Ability: Clone + fmt::Debug;
    /// The status condition type.
    type Status: Clone + PartialEq + fmt::Debug;
    /// The stat identifier type. Must be `Copy` + `PartialEq` for [`EnumMap`] key usage.
    type Stat: Copy + PartialEq + fmt::Debug;
    /// The species identifier type.
    type Species: Clone + fmt::Debug;
    /// The elemental type identifier type.
    type Type: Clone + PartialEq + fmt::Debug;
    /// The item identifier type.
    type Item: Clone + fmt::Debug;

    /// Calculate damage for a move against a defender.
    ///
    /// The provider is responsible for implementing the damage formula,
    /// consulting the type chart, and applying stat stage modifiers.
    fn calculate_damage(
        &self,
        move_: &Self::Move,
        attacker: &BattlerState<Self>,
        defender: &BattlerState<Self>,
        random: u8,
        is_critical: bool,
    ) -> DamageResult;

    /// Select a move for the given battler. Typically delegates to a
    /// [`BattleAI`] implementation.
    fn select_move(&self, battler: &BattlerState<Self>, state: &BattleState<Self>) -> Self::Move;

    /// Apply a move's effect after damage has been dealt. Typically
    /// delegates to an [`EffectHandler`] implementation.
    fn apply_move_effect(
        &self,
        effect: MoveEffect,
        user: &mut BattlerState<Self>,
        target: &mut BattlerState<Self>,
    ) -> EffectResult;

    /// Construct a battler state from species and level data.
    ///
    /// The provider looks up base stats, learnable moves, and type
    /// information for the given species at the given level.
    fn create_monster(&self, species: Self::Species, level: u8) -> BattlerState<Self>;

    /// Check whether a battler has fainted (HP = 0).
    fn check_faint(&self, battler: &BattlerState<Self>) -> bool {
        battler.hp == 0
    }

    // ── Resource cost hook (doc 13 §4 — the MP/SP/mana cost gate) ─────
    //
    // DEFAULTED so all 16 existing `impl BattleProvider` blocks compile
    // UNCHANGED. With the default empty slice (and the empty-by-default
    // [`ResourcePool`]), the engine's cost gate is a pure NO-OP: every existing
    // battle is byte-identical and the draw sequence is untouched (the gate is
    // pure arithmetic — it consumes NO randomness).

    /// The resource cost of `move_` as a list of `(resource_id, amount)` pairs.
    ///
    /// The engine assigns the resources no meaning (it is not "MP" to the engine
    /// — the ids are game-defined and opaque). Before resolving a `Fight` action,
    /// the [`StackDriver`](crate::battle::stack::StackDriver) checks the actor can
    /// pay **all** of these against its [`ResourcePool`]; if it cannot, the move is
    /// **prevented** (the existing `BeforeMove`/`Fail` prevention path), and if it
    /// can, the engine deducts each cost. The check and deduction are **pure
    /// arithmetic and consume no `rng`**, so a game that declares costs keeps its
    /// exact draw order.
    ///
    /// The default is `&[]` ⇒ no cost ⇒ the gate is inert, so a battler with no
    /// resources behaves exactly as today.
    fn move_cost(&self, _move_: &Self::Move) -> &[(u16, u16)] {
        &[]
    }

    // ── Turn-driver hooks (P0b) ──────────────────────────────────────
    //
    // All four are *defaulted* so pre-existing providers (and other
    // games) keep compiling unchanged. The engine driver supplies the
    // *sequencing*; these hooks supply the game's *numbers and rule
    // decisions* (C3/C4). Randomness is injected via `&mut dyn BattleRng`
    // so the engine never links `rand` (C2) and the game owns the exact
    // draw order (critical for Gen-1 quirks).

    /// Return the [`OrderKey`] used to sequence `who`'s `action` this turn.
    ///
    /// The engine stable-sorts actors ascending by this key (C4). The game
    /// encodes the priority bracket, effective speed (after paralysis cut,
    /// badge boosts, …), and any tie-break — e.g. the Gen-1 speed-tie coin
    /// flip drawn from `rng`. The default treats every actor as equal, so the
    /// driver falls back to stable submission order.
    fn turn_order_key(
        &self,
        _state: &BattleState<Self>,
        _who: BattlerRef,
        _action: &BattleAction<Self>,
        _rng: &mut dyn BattleRng,
    ) -> OrderKey
    where
        Self: Sized,
    {
        OrderKey::default()
    }

    /// Pre-move status gate: may `who` act with `action` this turn?
    ///
    /// The game owns sleep (counter decremented *before* the move), freeze,
    /// the full-paralysis roll, flinch, Hyper Beam recharge, confusion
    /// self-hit, partial-trap continuation, etc. The default always allows
    /// the action.
    fn before_move(
        &self,
        _state: &mut BattleState<Self>,
        _who: BattlerRef,
        _action: &BattleAction<Self>,
        _rng: &mut dyn BattleRng,
    ) -> MoveGate<Self>
    where
        Self: Sized,
    {
        MoveGate::Acts
    }

    /// Accuracy check for `who`'s `move_` against `target` (incl. the Gen-1
    /// 1/256 miss). Returns `true` if the move connects. Default: always hits.
    fn accuracy_check(
        &self,
        _state: &BattleState<Self>,
        _who: BattlerRef,
        _target: BattlerRef,
        _move_: &Self::Move,
        _rng: &mut dyn BattleRng,
    ) -> bool
    where
        Self: Sized,
    {
        true
    }

    /// End-of-turn residual hook: poison/burn tick, leech seed, wrap damage,
    /// weather, etc. The game owns ordering and numbers; it returns the
    /// resulting [`EffectResult`]s for the UI. Default: no residuals.
    fn end_of_turn(
        &self,
        _state: &mut BattleState<Self>,
        _rng: &mut dyn BattleRng,
    ) -> Vec<EffectResult>
    where
        Self: Sized,
    {
        Vec::new()
    }

    /// Roll whether `who`'s `move_` against `target` lands a **critical hit**.
    ///
    /// The driver calls this *before* [`calculate_damage`](Self::calculate_damage)
    /// and feeds the result back in as its `is_critical` argument, then surfaces
    /// it into the [`TurnEvent::Damage`](crate::battle::driver::TurnEvent::Damage)
    /// event's `critical` flag — so a provider-computed crit reaches both the
    /// damage formula and the UI/event stream.
    ///
    /// The game owns the crit-rate math (base speed / Focus Energy / high-crit
    /// moves and every Gen-1 quirk), drawing from `rng` as needed. The default
    /// never crits and draws **no** randomness, so pre-existing providers and
    /// other games keep their exact draw sequence unchanged.
    fn roll_critical(
        &self,
        _state: &BattleState<Self>,
        _who: BattlerRef,
        _target: BattlerRef,
        _move_: &Self::Move,
        _rng: &mut dyn BattleRng,
    ) -> bool
    where
        Self: Sized,
    {
        false
    }
}

// ─── BattlerState ──────────────────────────────────────────────────────

/// The battle state of a single monster/character.
///
/// Tracks HP, stats, stat-stage modifiers, status condition, and known
/// moves. Generic over the [`BattleProvider`] that supplies the concrete
/// type identifiers.
pub struct BattlerState<P: BattleProvider + ?Sized> {
    /// Species of this monster.
    pub species: P::Species,
    /// Current hit points.
    pub hp: u16,
    /// Maximum hit points.
    pub max_hp: u16,
    /// The battler's level. **Defaults to `50`** in [`new`](Self::new) (set it via
    /// [`with_level`](Self::with_level) or the field directly). The engine never
    /// interprets it; a provider's damage/effect logic reads it as needed (e.g. the
    /// level term in a damage formula). Additive — existing callers that ignore it
    /// keep the prior fixed-50 behaviour.
    pub level: u8,
    /// Base stat values, keyed by stat ID.
    pub stats: EnumMap<P::Stat, u16>,
    /// Stat-stage modifiers (-6 to +6), keyed by stat ID.
    pub stat_stages: EnumMap<P::Stat, i8>,
    /// Current status condition, if any.
    pub status: Option<P::Status>,
    /// Known moves.
    pub moves: Vec<P::Move>,
    /// Generic, game-defined consumable resources (MP / SP / mana / charge —
    /// doc 13 §4). **Defaults to EMPTY**, so a battler that declares no resource
    /// behaves exactly as before. Keyed by an opaque game-assigned `u16` id; the
    /// engine never interprets a resource's *meaning* (it is not "MP" to the
    /// engine). See [`ResourcePool`].
    pub resources: ResourcePool,
}

impl<P: BattleProvider + ?Sized> BattlerState<P> {
    /// Create a new battler state.
    pub fn new(
        species: P::Species,
        hp: u16,
        max_hp: u16,
        stats: EnumMap<P::Stat, u16>,
        moves: Vec<P::Move>,
    ) -> Self {
        Self {
            species,
            hp,
            max_hp,
            level: 50,
            stats,
            stat_stages: EnumMap::default(),
            status: None,
            moves,
            resources: ResourcePool::new(),
        }
    }

    /// Builder: set the battler's [`level`](Self::level).
    pub fn with_level(mut self, level: u8) -> Self {
        self.level = level;
        self
    }

    /// Apply damage, clamping HP to zero (never negative).
    pub fn take_damage(&mut self, amount: u16) {
        self.hp = self.hp.saturating_sub(amount);
    }

    /// Heal HP, clamping to max_hp.
    pub fn heal(&mut self, amount: u16) {
        self.hp = self.hp.saturating_add(amount).min(self.max_hp);
    }

    /// Builder: declare a generic resource (id, current = max) on this battler.
    /// Returns `self` so it chains after [`new`](Self::new) without changing the
    /// constructor's signature (the additivity invariant).
    pub fn with_resource(mut self, id: u16, max: u16) -> Self {
        self.resources.set(id, max, max);
        self
    }

    /// Whether this battler can pay `amount` of resource `id` (delegates to
    /// [`ResourcePool::can_pay`]). A `0` cost is always payable; a positive cost
    /// on an undeclared resource is not. **Pure — consumes no randomness.**
    pub fn can_pay_resource(&self, id: u16, amount: u16) -> bool {
        self.resources.can_pay(id, amount)
    }

    /// Deduct `amount` of resource `id` (saturating). **Pure arithmetic — no rng.**
    pub fn pay_resource(&mut self, id: u16, amount: u16) -> bool {
        self.resources.pay(id, amount)
    }
}

impl<P: BattleProvider + ?Sized> fmt::Debug for BattlerState<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BattlerState")
            .field("species", &self.species)
            .field("hp", &self.hp)
            .field("max_hp", &self.max_hp)
            .field("stats", &self.stats)
            .field("stat_stages", &self.stat_stages)
            .field("status", &self.status)
            .field("moves", &self.moves)
            .field("resources", &self.resources)
            .finish()
    }
}

impl<P: BattleProvider + ?Sized> Clone for BattlerState<P> {
    fn clone(&self) -> Self {
        Self {
            species: self.species.clone(),
            hp: self.hp,
            max_hp: self.max_hp,
            level: self.level,
            stats: self.stats.clone(),
            stat_stages: self.stat_stages.clone(),
            status: self.status.clone(),
            moves: self.moves.clone(),
            resources: self.resources.clone(),
        }
    }
}

// ─── BattleState ───────────────────────────────────────────────────────

/// The complete state of an ongoing battle.
///
/// Tracks both sides' parties, turn order, field conditions, and turn
/// counter. Generic over the [`BattleProvider`] that supplies the concrete
/// type identifiers.
pub struct BattleState<P: BattleProvider + ?Sized> {
    /// The player's party (one or more battlers).
    pub player_battlers: Vec<BattlerState<P>>,
    /// The opponent's party (one or more battlers).
    pub opponent_battlers: Vec<BattlerState<P>>,
    /// Turn order — indices into the combined battler list.
    pub turn_order: Vec<usize>,
    /// Current weather condition.
    pub weather: Weather,
    /// Current terrain condition.
    pub terrain: Terrain,
    /// Number of turns elapsed.
    pub turn_count: u32,
}

impl<P: BattleProvider + ?Sized> BattleState<P> {
    /// Create a new battle state.
    pub fn new(
        player_battlers: Vec<BattlerState<P>>,
        opponent_battlers: Vec<BattlerState<P>>,
    ) -> Self {
        Self {
            player_battlers,
            opponent_battlers,
            turn_order: Vec::new(),
            weather: Weather::default(),
            terrain: Terrain::default(),
            turn_count: 0,
        }
    }

    /// Get a reference to the active player battler (first in party).
    pub fn active_player(&self) -> Option<&BattlerState<P>> {
        self.player_battlers.first()
    }

    /// Get a mutable reference to the active player battler.
    pub fn active_player_mut(&mut self) -> Option<&mut BattlerState<P>> {
        self.player_battlers.first_mut()
    }

    /// Get a reference to the active opponent battler.
    pub fn active_opponent(&self) -> Option<&BattlerState<P>> {
        self.opponent_battlers.first()
    }

    /// Get a mutable reference to the active opponent battler.
    pub fn active_opponent_mut(&mut self) -> Option<&mut BattlerState<P>> {
        self.opponent_battlers.first_mut()
    }
}

impl<P: BattleProvider + ?Sized> fmt::Debug for BattleState<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BattleState")
            .field("player_battlers", &self.player_battlers)
            .field("opponent_battlers", &self.opponent_battlers)
            .field("turn_order", &self.turn_order)
            .field("weather", &self.weather)
            .field("terrain", &self.terrain)
            .field("turn_count", &self.turn_count)
            .finish()
    }
}

impl<P: BattleProvider + ?Sized> Clone for BattleState<P> {
    fn clone(&self) -> Self {
        Self {
            player_battlers: self.player_battlers.clone(),
            opponent_battlers: self.opponent_battlers.clone(),
            turn_order: self.turn_order.clone(),
            weather: self.weather,
            terrain: self.terrain,
            turn_count: self.turn_count,
        }
    }
}

// ─── TypeChart ─────────────────────────────────────────────────────────

/// N×N type effectiveness matrix.
///
/// Returns a multiplier for an attacking type against a set of defending
/// types. The implementation can be a lookup table, a procedural rule,
/// or any combination thereof.
pub trait TypeChart {
    /// The elemental type identifier.
    type Type: PartialEq + fmt::Debug;

    /// Compute the effectiveness of an attacking type against one or more
    /// defending types.
    ///
    /// The result is a multiplier: `1.0` = neutral, `2.0` = super effective,
    /// `0.5` = not very effective, `0.0` = immune.
    ///
    /// For dual-type defenders, the caller passes both types in the
    /// `defending` slice and the chart computes the combined multiplier.
    fn effectiveness(attacking: &Self::Type, defending: &[Self::Type]) -> f32;
}

// ─── BattleAI ──────────────────────────────────────────────────────────

/// AI decision-making for battle actions.
///
/// Implementations decide which move to use, whether to switch monsters,
/// and whether to use items — all based on the current battle state.
pub trait BattleAI<P: BattleProvider + ?Sized> {
    /// Select a move for the given battler.
    fn select_move(&self, battler: &BattlerState<P>, state: &BattleState<P>) -> P::Move;

    /// Decide whether the battler should switch to a different monster.
    fn should_switch(&self, battler: &BattlerState<P>) -> bool;

    /// Decide whether the battler should use an item, and which one.
    fn should_use_item(&self, battler: &BattlerState<P>) -> Option<P::Item>;
}

// ─── EffectHandler ─────────────────────────────────────────────────────

/// Handles move effects on battler state.
///
/// After damage has been dealt, the effect handler applies additional
/// effects such as status infliction, stat modification, healing, and
/// field changes.
pub trait EffectHandler<P: BattleProvider + ?Sized> {
    /// Apply a move effect to the user and/or target.
    ///
    /// * `effect` — The category of effect to apply.
    /// * `user` — The battler that used the move.
    /// * `target` — The battler being targeted.
    /// * `provider` — The battle data provider for lookups.
    fn handle_effect(
        &self,
        effect: MoveEffect,
        user: &mut BattlerState<P>,
        target: &mut BattlerState<P>,
        provider: &P,
    ) -> EffectResult;
}

// ─── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Mock Types ────────────────────────────────────────────────────

    /// A simple three-type rock-paper-scissors system:
    ///   TypeA beats TypeB, TypeB beats TypeC, TypeC beats TypeA.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MockType {
        TypeA,
        TypeB,
        TypeC,
    }

    /// Stats for the mock battle system.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MockStat {
        Hp,
        Attack,
        Defense,
    }

    /// Status conditions.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[allow(dead_code)]
    enum MockStatus {
        Poison,
        Burn,
        Sleep,
    }

    /// Species identifiers.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[allow(dead_code)]
    enum MockSpecies {
        Alpha,
        Beta,
        Gamma,
    }

    /// A move with power, type, and accuracy.
    #[derive(Debug, Clone, PartialEq)]
    struct MockMove {
        name: String,
        power: u8,
        move_type: MockType,
        accuracy: u8,
    }

    /// A monster (used as Monster associated type).
    #[derive(Debug, Clone, PartialEq)]
    struct MockMonster {
        name: String,
        monster_type: MockType,
        hp: u16,
        attack: u16,
        defense: u16,
    }

    // ── Mock TypeChart ───────────────────────────────────────────────

    struct MockTypeChart;

    impl TypeChart for MockTypeChart {
        type Type = MockType;

        fn effectiveness(attacking: &Self::Type, defending: &[Self::Type]) -> f32 {
            // Single-type check against the first defending type.
            let def = defending.first().copied().unwrap_or(MockType::TypeA);
            match (attacking, def) {
                // TypeA beats TypeB (super effective, 2x)
                (MockType::TypeA, MockType::TypeB) => 2.0,
                // TypeB beats TypeC (super effective, 2x)
                (MockType::TypeB, MockType::TypeC) => 2.0,
                // TypeC beats TypeA (super effective, 2x)
                (MockType::TypeC, MockType::TypeA) => 2.0,
                // Reversed: not very effective (0.5x)
                (MockType::TypeA, MockType::TypeC) => 0.5,
                (MockType::TypeB, MockType::TypeA) => 0.5,
                (MockType::TypeC, MockType::TypeB) => 0.5,
                // Same type: not very effective (0.5x)
                (a, d) if *a == d => 0.5,
                // Default: neutral
                _ => 1.0,
            }
        }
    }

    // ── Mock Provider ─────────────────────────────────────────────────

    struct MockProvider;

    impl BattleProvider for MockProvider {
        type Monster = MockMonster;
        type Move = MockMove;
        type Ability = String;
        type Status = MockStatus;
        type Stat = MockStat;
        type Species = MockSpecies;
        type Type = MockType;
        type Item = String;

        fn calculate_damage(
            &self,
            move_: &Self::Move,
            attacker: &BattlerState<Self>,
            defender: &BattlerState<Self>,
            _random: u8,
            _is_critical: bool,
        ) -> DamageResult {
            let atk = attacker.stats.get(MockStat::Attack).copied().unwrap_or(0);
            let def = defender
                .stats
                .get(MockStat::Defense)
                .copied()
                .unwrap_or(1)
                .max(1);
            let defender_types: Vec<MockType> = vec![]; // Simplified: uses move_type only

            let effectiveness = MockTypeChart::effectiveness(&move_.move_type, &defender_types);

            if effectiveness == 0.0 {
                return DamageResult {
                    damage: 0,
                    effectiveness,
                    is_miss: true,
                };
            }

            // Simple formula: (power * attack / defense) * effectiveness
            let base = (move_.power as u32 * atk as u32 / def as u32) as u16;
            let damage = ((base as f32) * effectiveness) as u16;

            DamageResult {
                damage: damage.max(1),
                effectiveness,
                is_miss: false,
            }
        }

        fn select_move(
            &self,
            battler: &BattlerState<Self>,
            _state: &BattleState<Self>,
        ) -> Self::Move {
            // Always pick the first move.
            battler.moves.first().cloned().unwrap()
        }

        fn apply_move_effect(
            &self,
            _effect: MoveEffect,
            user: &mut BattlerState<Self>,
            target: &mut BattlerState<Self>,
        ) -> EffectResult {
            // Simplified: always deal damage based on first move's power.
            let power = user.moves.first().map(|m| m.power).unwrap_or(0);
            if power == 0 {
                return EffectResult::NoEffect;
            }
            target.take_damage(power as u16);
            EffectResult::DamageDealt {
                amount: power as u16,
            }
        }

        fn create_monster(&self, species: Self::Species, _level: u8) -> BattlerState<Self> {
            // Simplified: hardcoded stats per species.
            let (hp, atk, def, _monster_type) = match species {
                MockSpecies::Alpha => (100, 50, 30, MockType::TypeA),
                MockSpecies::Beta => (120, 40, 40, MockType::TypeB),
                MockSpecies::Gamma => (90, 60, 20, MockType::TypeC),
            };
            let mut stats = EnumMap::new();
            stats.set(MockStat::Hp, hp);
            stats.set(MockStat::Attack, atk);
            stats.set(MockStat::Defense, def);
            BattlerState::new(species, hp, hp, stats, Vec::new())
        }
    }

    // ── Mock AI ───────────────────────────────────────────────────────

    struct MockAI;

    impl BattleAI<MockProvider> for MockAI {
        fn select_move(
            &self,
            battler: &BattlerState<MockProvider>,
            _state: &BattleState<MockProvider>,
        ) -> MockMove {
            battler.moves.first().cloned().unwrap()
        }

        fn should_switch(&self, _battler: &BattlerState<MockProvider>) -> bool {
            false
        }

        fn should_use_item(&self, _battler: &BattlerState<MockProvider>) -> Option<String> {
            None
        }
    }

    // ── Mock EffectHandler ────────────────────────────────────────────

    struct MockEffectHandler;

    impl EffectHandler<MockProvider> for MockEffectHandler {
        fn handle_effect(
            &self,
            effect: MoveEffect,
            _user: &mut BattlerState<MockProvider>,
            target: &mut BattlerState<MockProvider>,
            _provider: &MockProvider,
        ) -> EffectResult {
            match effect {
                MoveEffect::Damage => {
                    target.take_damage(10);
                    EffectResult::DamageDealt { amount: 10 }
                }
                MoveEffect::Heal => {
                    target.heal(10);
                    EffectResult::Healed { amount: 10 }
                }
                MoveEffect::StatusCondition => EffectResult::StatusInflicted,
                MoveEffect::StatChange => EffectResult::StatModified { stages: 1 },
                MoveEffect::MultiHit => EffectResult::MultiHit { hits: 3 },
                MoveEffect::Recharge => EffectResult::MustRecharge,
                _ => EffectResult::NoEffect,
            }
        }
    }

    // ── Helper ────────────────────────────────────────────────────────

    /// Create a mock battler with given type and stats.
    fn make_battler(
        species: MockSpecies,
        hp: u16,
        atk: u16,
        def: u16,
    ) -> BattlerState<MockProvider> {
        let mut stats = EnumMap::new();
        stats.set(MockStat::Hp, hp);
        stats.set(MockStat::Attack, atk);
        stats.set(MockStat::Defense, def);
        BattlerState::new(species, hp, hp, stats, Vec::new())
    }

    fn make_move(name: &str, power: u8, move_type: MockType) -> MockMove {
        MockMove {
            name: name.to_string(),
            power,
            move_type,
            accuracy: 255,
        }
    }

    // ── Tests: TypeChart ──────────────────────────────────────────────

    #[test]
    fn type_a_beats_type_b() {
        let eff = MockTypeChart::effectiveness(&MockType::TypeA, &[MockType::TypeB]);
        assert!(
            (eff - 2.0).abs() < f32::EPSILON,
            "TypeA should be 2x vs TypeB, got {eff}"
        );
    }

    #[test]
    fn type_a_vs_type_a_is_half() {
        let eff = MockTypeChart::effectiveness(&MockType::TypeA, &[MockType::TypeA]);
        assert!(
            (eff - 0.5).abs() < f32::EPSILON,
            "TypeA vs TypeA should be 0.5x, got {eff}"
        );
    }

    #[test]
    fn type_b_beats_type_c() {
        let eff = MockTypeChart::effectiveness(&MockType::TypeB, &[MockType::TypeC]);
        assert!(
            (eff - 2.0).abs() < f32::EPSILON,
            "TypeB should be 2x vs TypeC, got {eff}"
        );
    }

    #[test]
    fn type_c_beats_type_a() {
        let eff = MockTypeChart::effectiveness(&MockType::TypeC, &[MockType::TypeA]);
        assert!(
            (eff - 2.0).abs() < f32::EPSILON,
            "TypeC should be 2x vs TypeA, got {eff}"
        );
    }

    #[test]
    fn type_b_vs_type_a_is_half() {
        let eff = MockTypeChart::effectiveness(&MockType::TypeB, &[MockType::TypeA]);
        assert!(
            (eff - 0.5).abs() < f32::EPSILON,
            "TypeB vs TypeA should be 0.5x, got {eff}"
        );
    }

    // ── Tests: BattleProvider ─────────────────────────────────────────

    #[test]
    fn provider_can_be_implemented() {
        let provider = MockProvider;
        let battler = make_battler(MockSpecies::Alpha, 100, 50, 30);
        assert!(!provider.check_faint(&battler));
    }

    #[test]
    fn create_monster_returns_battler() {
        let provider = MockProvider;
        let battler = provider.create_monster(MockSpecies::Alpha, 50);
        assert_eq!(battler.hp, 100);
        assert_eq!(battler.max_hp, 100);
    }

    #[test]
    fn calculate_damage_with_type_advantage() {
        let provider = MockProvider;
        let attacker = make_battler(MockSpecies::Alpha, 100, 50, 30);
        let defender = make_battler(MockSpecies::Beta, 100, 40, 40);
        let mv = make_move("SuperPunch", 60, MockType::TypeA);

        let result = provider.calculate_damage(&mv, &attacker, &defender, 255, false);

        // TypeA vs defender... but our simplified formula uses move_type only.
        // The effectiveness for TypeA in MockTypeChart depends on defender's type.
        // Since we don't pass defender type to the calculator, it defaults to neutral.
        // The damage = (60 * 50 / 40) * 1.0 = 75
        assert!(result.damage > 0);
        assert!(!result.is_miss);
    }

    #[test]
    fn select_move_returns_first_move() {
        let provider = MockProvider;
        let mut battler = make_battler(MockSpecies::Alpha, 100, 50, 30);
        let mv = make_move("Tackle", 40, MockType::TypeA);
        battler.moves = vec![mv.clone()];

        let state = BattleState::<MockProvider>::new(
            vec![battler.clone()],
            vec![make_battler(MockSpecies::Beta, 100, 40, 40)],
        );

        let selected = provider.select_move(&battler, &state);
        assert_eq!(selected.name, "Tackle");
    }

    #[test]
    fn apply_move_effect_deals_damage() {
        let provider = MockProvider;
        let mut user = make_battler(MockSpecies::Alpha, 100, 50, 30);
        let mv = make_move("Tackle", 40, MockType::TypeA);
        user.moves = vec![mv];
        let mut target = make_battler(MockSpecies::Beta, 100, 40, 40);

        let result = provider.apply_move_effect(MoveEffect::Damage, &mut user, &mut target);
        assert!(matches!(result, EffectResult::DamageDealt { .. }));
        assert_eq!(target.hp, 60); // 100 - 40
    }

    #[test]
    fn check_faint_detects_zero_hp() {
        let provider = MockProvider;
        let mut battler = make_battler(MockSpecies::Alpha, 100, 50, 30);
        assert!(!provider.check_faint(&battler));

        battler.hp = 0;
        assert!(provider.check_faint(&battler));
    }

    // ── Tests: BattleAI trait ─────────────────────────────────────────

    #[test]
    fn ai_trait_can_be_implemented() {
        let ai = MockAI;
        let battler = make_battler(MockSpecies::Alpha, 100, 50, 30);
        let _state = BattleState::<MockProvider>::new(
            vec![battler.clone()],
            vec![make_battler(MockSpecies::Beta, 100, 40, 40)],
        );

        assert!(!ai.should_switch(&battler));
        assert!(ai.should_use_item(&battler).is_none());
    }

    #[test]
    fn ai_select_move_works() {
        let ai = MockAI;
        let mut battler = make_battler(MockSpecies::Alpha, 100, 50, 30);
        let mv = make_move("Fireball", 50, MockType::TypeA);
        battler.moves = vec![mv.clone()];

        let state = BattleState::<MockProvider>::new(
            vec![battler.clone()],
            vec![make_battler(MockSpecies::Beta, 100, 40, 40)],
        );

        let chosen = ai.select_move(&battler, &state);
        assert_eq!(chosen.name, "Fireball");
    }

    // ── Tests: EffectHandler trait ────────────────────────────────────

    #[test]
    fn effect_handler_trait_can_be_implemented() {
        let handler = MockEffectHandler;
        let provider = MockProvider;
        let mut user = make_battler(MockSpecies::Alpha, 100, 50, 30);
        let mut target = make_battler(MockSpecies::Beta, 100, 40, 40);

        let result = handler.handle_effect(MoveEffect::Damage, &mut user, &mut target, &provider);
        assert!(matches!(result, EffectResult::DamageDealt { amount: 10 }));
        assert_eq!(target.hp, 90);
    }

    #[test]
    fn effect_handler_heal_works() {
        let handler = MockEffectHandler;
        let provider = MockProvider;
        let mut user = make_battler(MockSpecies::Alpha, 50, 50, 30);
        user.hp = 50;
        let mut target = make_battler(MockSpecies::Beta, 100, 40, 40);

        let result = handler.handle_effect(MoveEffect::Heal, &mut user, &mut target, &provider);
        assert!(matches!(result, EffectResult::Healed { amount: 10 }));
        assert_eq!(target.hp, 100); // 100 + capped at max
    }

    #[test]
    fn effect_handler_status_and_stat() {
        let handler = MockEffectHandler;
        let provider = MockProvider;
        let mut user = make_battler(MockSpecies::Alpha, 100, 50, 30);
        let mut target = make_battler(MockSpecies::Beta, 100, 40, 40);

        let r1 = handler.handle_effect(
            MoveEffect::StatusCondition,
            &mut user,
            &mut target,
            &provider,
        );
        assert_eq!(r1, EffectResult::StatusInflicted);

        let r2 = handler.handle_effect(MoveEffect::StatChange, &mut user, &mut target, &provider);
        assert_eq!(r2, EffectResult::StatModified { stages: 1 });
    }

    // ── Tests: BattlerState ───────────────────────────────────────────

    #[test]
    fn battler_state_take_damage() {
        let mut battler = make_battler(MockSpecies::Alpha, 100, 50, 30);
        battler.take_damage(30);
        assert_eq!(battler.hp, 70);
    }

    #[test]
    fn battler_state_take_damage_no_underflow() {
        let mut battler = make_battler(MockSpecies::Alpha, 10, 50, 30);
        battler.take_damage(999);
        assert_eq!(battler.hp, 0);
    }

    #[test]
    fn battler_state_heal() {
        let mut battler = make_battler(MockSpecies::Alpha, 100, 50, 30);
        battler.hp = 50;
        battler.heal(30);
        assert_eq!(battler.hp, 80);
    }

    #[test]
    fn battler_state_heal_caps_at_max() {
        let mut battler = make_battler(MockSpecies::Alpha, 100, 50, 30);
        battler.hp = 90;
        battler.heal(50);
        assert_eq!(battler.hp, 100);
    }

    // ── Tests: BattleState ────────────────────────────────────────────

    #[test]
    fn battle_state_defaults() {
        let p1 = make_battler(MockSpecies::Alpha, 100, 50, 30);
        let o1 = make_battler(MockSpecies::Beta, 100, 40, 40);
        let state = BattleState::<MockProvider>::new(vec![p1.clone()], vec![o1.clone()]);

        assert_eq!(state.player_battlers.len(), 1);
        assert_eq!(state.opponent_battlers.len(), 1);
        assert_eq!(state.turn_count, 0);
        assert!(matches!(state.weather, Weather::Clear));
        assert!(matches!(state.terrain, Terrain::Normal));
    }

    #[test]
    fn battle_state_active_player() {
        let p1 = make_battler(MockSpecies::Alpha, 100, 50, 30);
        let o1 = make_battler(MockSpecies::Beta, 100, 40, 40);
        let state = BattleState::<MockProvider>::new(vec![p1], vec![o1]);

        let active = state.active_player().unwrap();
        assert_eq!(active.hp, 100);
    }

    // ── Tests: EnumMap ────────────────────────────────────────────────

    #[test]
    fn enum_map_set_and_get() {
        let mut map: EnumMap<MockStat, u16> = EnumMap::new();
        map.set(MockStat::Attack, 50);
        map.set(MockStat::Defense, 30);

        assert_eq!(map.get(MockStat::Attack), Some(&50));
        assert_eq!(map.get(MockStat::Defense), Some(&30));
        assert_eq!(map.get(MockStat::Hp), None);
    }

    #[test]
    fn enum_map_overwrite() {
        let mut map: EnumMap<MockStat, u16> = EnumMap::new();
        map.set(MockStat::Hp, 100);
        map.set(MockStat::Hp, 200);

        assert_eq!(map.get(MockStat::Hp), Some(&200));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn enum_map_default_is_empty() {
        let map: EnumMap<MockStat, u16> = EnumMap::default();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    // ── Tests: ResourcePool (the generic MP/SP/mana pool, doc 13 §4) ──────

    #[test]
    fn resource_pool_default_is_empty_and_inert() {
        let pool = ResourcePool::default();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        // An empty pool: a 0 cost is always payable (inert); any positive cost on
        // an undeclared resource is NOT payable.
        assert!(pool.can_pay(0, 0), "0 cost is always payable");
        assert!(
            !pool.can_pay(0, 1),
            "positive cost on undeclared resource ⇒ not payable"
        );
        assert_eq!(pool.current(0), None);
    }

    #[test]
    fn resource_pool_set_can_pay_and_pay() {
        let mut pool = ResourcePool::new();
        pool.set(7, 10, 10); // resource id 7, current 10, max 10
        assert_eq!(pool.current(7), Some(10));
        assert_eq!(pool.max(7), Some(10));
        assert!(pool.can_pay(7, 4));
        assert!(pool.can_pay(7, 10), "exact balance is payable");
        assert!(!pool.can_pay(7, 11), "over balance is not payable");

        assert!(pool.pay(7, 4), "deduction applied");
        assert_eq!(pool.current(7), Some(6), "10 - 4 = 6");
        assert!(!pool.pay(7, 0), "0 deduction is a no-op (returns false)");
        assert_eq!(pool.current(7), Some(6), "0 deduction left it unchanged");
    }

    #[test]
    fn resource_pool_pay_saturates_and_restore_clamps() {
        let mut pool = ResourcePool::new();
        pool.set(0, 3, 10);
        pool.pay(0, 100); // over-pay saturates at 0
        assert_eq!(pool.current(0), Some(0));
        pool.restore(0, 4);
        assert_eq!(pool.current(0), Some(4));
        pool.restore(0, 1000); // restore clamps to max
        assert_eq!(pool.current(0), Some(10));
    }

    #[test]
    fn resource_pool_set_clamps_current_to_max() {
        let mut pool = ResourcePool::new();
        pool.set(0, 50, 20); // current > max
        assert_eq!(pool.current(0), Some(20), "current clamped to max");
    }

    #[test]
    fn battler_state_resources_default_empty() {
        // A battler built via `new` (the constructor all 16 impls call) declares NO
        // resources — the additivity invariant.
        let b: BattlerState<MockProvider> =
            BattlerState::new(MockSpecies::Alpha, 100, 100, EnumMap::new(), vec![]);
        assert!(
            b.resources.is_empty(),
            "default battler has an empty resource pool"
        );
        assert!(b.can_pay_resource(0, 0), "0 cost payable on an empty pool");
        assert!(
            !b.can_pay_resource(0, 5),
            "positive cost unpayable on an empty pool"
        );

        // `with_resource` declares one and the pay helpers work end to end.
        let mut b = b.with_resource(0, 8);
        assert!(b.can_pay_resource(0, 5));
        assert!(b.pay_resource(0, 5));
        assert_eq!(b.resources.current(0), Some(3));
    }

    // ── Tests: Weather / Terrain ──────────────────────────────────────

    #[test]
    fn weather_default_is_clear() {
        assert_eq!(Weather::default(), Weather::Clear);
    }

    #[test]
    fn terrain_default_is_normal() {
        assert_eq!(Terrain::default(), Terrain::Normal);
    }

    // ── Tests: MoveEffect / EffectResult ──────────────────────────────

    #[test]
    fn move_effect_variants_are_available() {
        // Ensure all variants can be constructed.
        let _effects = [
            MoveEffect::Damage,
            MoveEffect::Heal,
            MoveEffect::StatusCondition,
            MoveEffect::StatChange,
            MoveEffect::MultiHit,
            MoveEffect::Recharge,
            MoveEffect::DrainHp,
            MoveEffect::Recoil,
            MoveEffect::Flinch,
            MoveEffect::FieldEffect,
            MoveEffect::SpecialDamage,
            MoveEffect::Ohko,
            MoveEffect::MultiTurn,
        ];
    }

    #[test]
    fn effect_result_variants_are_available() {
        let _results = [
            EffectResult::NoEffect,
            EffectResult::DamageDealt { amount: 0 },
            EffectResult::Healed { amount: 0 },
            EffectResult::StatusInflicted,
            EffectResult::StatusFailed,
            EffectResult::StatModified { stages: 1 },
            EffectResult::StatBlocked,
            EffectResult::HpDrained { drained: 0 },
            EffectResult::RecoilDamage { recoil: 0 },
            EffectResult::Fainted,
            EffectResult::Miss,
            EffectResult::CriticalHit,
            EffectResult::MultiHit { hits: 2 },
            EffectResult::MustRecharge,
            EffectResult::FieldEffectSet,
        ];
    }
}

// ─── Driver tests (P0b) ────────────────────────────────────────────────

#[cfg(test)]
mod driver_tests {
    use super::driver::{BattleDriver, BattleEnd, TurnEvent};
    use super::rng::{BattleRng, ScriptedRng};
    use super::*;

    // ── Mock world for the driver ────────────────────────────────────

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DStat {
        Speed,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DStatus {
        Sleep,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DType {
        Normal,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DSpecies {
        Mon,
    }
    #[derive(Debug, Clone, PartialEq)]
    struct DMove {
        power: u8,
        accuracy: u8,
    }

    /// A deterministic provider exercising every driver hook.
    ///
    /// * `turn_order_key` ranks by Speed stat (faster acts first), with a
    ///   coin-flip tie-break drawn from the rng (Gen-1-style speed tie).
    /// * `before_move` skips a battler whose status is `Sleep`.
    /// * `accuracy_check` consults a draw vs the move's accuracy.
    /// * `calculate_damage` returns the move's `power` as flat damage.
    /// * `end_of_turn` applies `residual` damage to every living battler.
    struct DProvider {
        residual: u16,
    }

    impl BattleProvider for DProvider {
        type Monster = ();
        type Move = DMove;
        type Ability = ();
        type Status = DStatus;
        type Stat = DStat;
        type Species = DSpecies;
        type Type = DType;
        type Item = ();

        fn calculate_damage(
            &self,
            move_: &Self::Move,
            _attacker: &BattlerState<Self>,
            _defender: &BattlerState<Self>,
            _random: u8,
            _is_critical: bool,
        ) -> DamageResult {
            DamageResult {
                damage: move_.power as u16,
                effectiveness: 1.0,
                is_miss: false,
            }
        }

        fn select_move(
            &self,
            battler: &BattlerState<Self>,
            _state: &BattleState<Self>,
        ) -> Self::Move {
            battler.moves.first().cloned().unwrap()
        }

        fn apply_move_effect(
            &self,
            _effect: MoveEffect,
            _user: &mut BattlerState<Self>,
            _target: &mut BattlerState<Self>,
        ) -> EffectResult {
            EffectResult::NoEffect
        }

        fn create_monster(&self, species: Self::Species, _level: u8) -> BattlerState<Self> {
            BattlerState::new(species, 100, 100, EnumMap::new(), Vec::new())
        }

        // ── New P0b hooks ──

        fn turn_order_key(
            &self,
            state: &BattleState<Self>,
            who: BattlerRef,
            _action: &BattleAction<Self>,
            rng: &mut dyn BattleRng,
        ) -> OrderKey {
            let party = if who.side == 0 {
                &state.player_battlers
            } else {
                &state.opponent_battlers
            };
            let speed = party
                .get(who.slot as usize)
                .and_then(|b| b.stats.get(DStat::Speed).copied())
                .unwrap_or(0) as i32;
            // Negate speed so the faster battler sorts first; draw a coin-flip
            // tie-break from the rng (the draw happens for *every* actor so the
            // sequence is deterministic).
            let tiebreak = rng.next_u8() as u32;
            OrderKey(0, -speed, tiebreak)
        }

        fn before_move(
            &self,
            state: &mut BattleState<Self>,
            who: BattlerRef,
            _action: &BattleAction<Self>,
            _rng: &mut dyn BattleRng,
        ) -> MoveGate<Self> {
            let party = if who.side == 0 {
                &state.player_battlers
            } else {
                &state.opponent_battlers
            };
            if let Some(b) = party.get(who.slot as usize) {
                if b.status == Some(DStatus::Sleep) {
                    return MoveGate::Prevented(EffectResult::NoEffect);
                }
            }
            MoveGate::Acts
        }

        fn accuracy_check(
            &self,
            _state: &BattleState<Self>,
            _who: BattlerRef,
            _target: BattlerRef,
            move_: &Self::Move,
            rng: &mut dyn BattleRng,
        ) -> bool {
            // Hits if the drawn byte is below the move's accuracy.
            (rng.next_u8() as u32) < move_.accuracy as u32
        }

        fn end_of_turn(
            &self,
            state: &mut BattleState<Self>,
            _rng: &mut dyn BattleRng,
        ) -> Vec<EffectResult> {
            let mut out = Vec::new();
            for party in [&mut state.player_battlers, &mut state.opponent_battlers] {
                for b in party.iter_mut() {
                    if b.hp > 0 {
                        b.take_damage(self.residual);
                        out.push(EffectResult::DamageDealt {
                            amount: self.residual,
                        });
                    }
                }
            }
            out
        }
    }

    /// A provider that *always* lands a critical hit, drawing **no** rng in
    /// `roll_critical`, and doubles damage when told `is_critical`. Used to
    /// prove a provider-reported crit reaches both the damage formula and the
    /// `Damage` event's `critical` flag.
    struct CritProvider;

    impl BattleProvider for CritProvider {
        type Monster = ();
        type Move = DMove;
        type Ability = ();
        type Status = DStatus;
        type Stat = DStat;
        type Species = DSpecies;
        type Type = DType;
        type Item = ();

        fn calculate_damage(
            &self,
            move_: &Self::Move,
            _attacker: &BattlerState<Self>,
            _defender: &BattlerState<Self>,
            _random: u8,
            is_critical: bool,
        ) -> DamageResult {
            let base = move_.power as u16;
            DamageResult {
                damage: if is_critical { base * 2 } else { base },
                effectiveness: 1.0,
                is_miss: false,
            }
        }

        fn select_move(
            &self,
            battler: &BattlerState<Self>,
            _state: &BattleState<Self>,
        ) -> Self::Move {
            battler.moves.first().cloned().unwrap()
        }

        fn apply_move_effect(
            &self,
            _effect: MoveEffect,
            _user: &mut BattlerState<Self>,
            _target: &mut BattlerState<Self>,
        ) -> EffectResult {
            EffectResult::NoEffect
        }

        fn create_monster(&self, species: Self::Species, _level: u8) -> BattlerState<Self> {
            BattlerState::new(species, 100, 100, EnumMap::new(), Vec::new())
        }

        /// Always crit; draw no rng so byte accounting stays predictable.
        fn roll_critical(
            &self,
            _state: &BattleState<Self>,
            _who: BattlerRef,
            _target: BattlerRef,
            _move_: &Self::Move,
            _rng: &mut dyn BattleRng,
        ) -> bool {
            true
        }
    }

    fn crit_mon(hp: u16, speed: u16, power: u8) -> BattlerState<CritProvider> {
        let mut stats = EnumMap::new();
        stats.set(DStat::Speed, speed);
        BattlerState::new(
            DSpecies::Mon,
            hp,
            hp,
            stats,
            vec![DMove {
                power,
                accuracy: 255,
            }],
        )
    }

    fn mon(hp: u16, speed: u16, accuracy: u8, power: u8) -> BattlerState<DProvider> {
        let mut stats = EnumMap::new();
        stats.set(DStat::Speed, speed);
        BattlerState::new(
            DSpecies::Mon,
            hp,
            hp,
            stats,
            vec![DMove { power, accuracy }],
        )
    }

    /// A connecting (accuracy 255) move action of the given power.
    fn fight_pow(power: u8) -> BattleAction<DProvider> {
        BattleAction::Fight {
            move_: DMove {
                power,
                accuracy: 255,
            },
        }
    }

    /// A connecting move action of power 20.
    fn fight() -> BattleAction<DProvider> {
        fight_pow(20)
    }

    fn first_mover(out: &TurnOutcome<DProvider>) -> BattlerRef {
        out.events
            .iter()
            .find_map(|e| match e {
                TurnEvent::MoveUsed { who, .. } => Some(*who),
                _ => None,
            })
            .unwrap()
    }

    // ── Tests ────────────────────────────────────────────────────────

    #[test]
    fn turn_order_faster_battler_acts_first() {
        let provider = DProvider { residual: 0 };
        // Player slow (speed 10), opponent fast (speed 99). Always-hit moves.
        let mut state = BattleState::new(vec![mon(100, 10, 255, 20)], vec![mon(100, 99, 255, 20)]);
        // RNG: turn_order_key draws one byte per actor (2), then accuracy + damage
        // bytes per fight. Keep accuracy bytes below 255 so moves connect.
        let mut rng = ScriptedRng::new(vec![5, 5, 0, 0, 0, 0]);
        let out = BattleDriver::execute_turn(&provider, &mut state, [fight(), fight()], &mut rng);
        assert_eq!(
            first_mover(&out),
            BattlerRef::OPPONENT,
            "faster opponent acts first"
        );
    }

    #[test]
    fn turn_order_tie_uses_rng_tiebreak() {
        let provider = DProvider { residual: 0 };
        // Equal speed → tie broken by the rng draw. Player draws 9, opp draws 1,
        // so opp's tiebreak is smaller → opp first.
        let mut state = BattleState::new(vec![mon(100, 50, 255, 20)], vec![mon(100, 50, 255, 20)]);
        let mut rng = ScriptedRng::new(vec![9, 1, 0, 0, 0, 0]);
        let out = BattleDriver::execute_turn(&provider, &mut state, [fight(), fight()], &mut rng);
        assert_eq!(
            first_mover(&out),
            BattlerRef::OPPONENT,
            "smaller tiebreak acts first"
        );
    }

    #[test]
    fn before_move_gate_skips_asleep_battler() {
        let provider = DProvider { residual: 0 };
        let mut player = mon(100, 99, 255, 20); // fast so it would act first
        player.status = Some(DStatus::Sleep);
        let mut state = BattleState::new(vec![player], vec![mon(100, 10, 255, 20)]);
        let mut rng = ScriptedRng::new(vec![0, 0, 0, 0, 0, 0]);
        let out = BattleDriver::execute_turn(&provider, &mut state, [fight(), fight()], &mut rng);

        assert!(
            out.events.iter().any(|e| matches!(
                e,
                TurnEvent::ActionPrevented { who, .. } if *who == BattlerRef::PLAYER
            )),
            "asleep player should be prevented"
        );
        assert!(
            out.events.iter().any(|e| matches!(
                e,
                TurnEvent::MoveUsed { who, .. } if *who == BattlerRef::OPPONENT
            )),
            "opponent should still act"
        );
    }

    #[test]
    fn move_execution_applies_damage_via_hook() {
        let provider = DProvider { residual: 0 };
        // The driver uses the *action's* move (power 30), passed to the
        // provider's calculate_damage hook.
        let mut state = BattleState::new(
            vec![mon(100, 99, 255, 0)], // player fast
            vec![mon(100, 10, 255, 0)],
        );
        let mut rng = ScriptedRng::new(vec![0, 0, 0, 0, 0, 0]);
        let _ = BattleDriver::execute_turn(
            &provider,
            &mut state,
            [fight_pow(30), fight_pow(30)],
            &mut rng,
        );
        // Opponent took 30 (player's move); player took 30 (opponent's move).
        assert_eq!(state.opponent_battlers[0].hp, 70);
        assert_eq!(state.player_battlers[0].hp, 70);
    }

    #[test]
    fn accuracy_check_can_miss() {
        let provider = DProvider { residual: 0 };
        let mut state = BattleState::new(vec![mon(100, 99, 255, 20)], vec![mon(100, 10, 255, 20)]);
        // Player's move has accuracy 0 → always misses; opponent's connects.
        let player_fight = BattleAction::Fight {
            move_: DMove {
                power: 20,
                accuracy: 0,
            },
        };
        let opp_fight = BattleAction::Fight {
            move_: DMove {
                power: 20,
                accuracy: 255,
            },
        };
        let mut rng = ScriptedRng::new(vec![0, 0, 100, 0, 100, 0]);
        let out =
            BattleDriver::execute_turn(&provider, &mut state, [player_fight, opp_fight], &mut rng);
        // Opponent untouched (player missed); player took 20.
        assert_eq!(state.opponent_battlers[0].hp, 100);
        assert_eq!(state.player_battlers[0].hp, 80);
        assert!(out.events.iter().any(|e| matches!(
            e,
            TurnEvent::Missed { who, .. } if *who == BattlerRef::PLAYER
        )));
    }

    #[test]
    fn end_of_turn_residual_ticks() {
        let provider = DProvider { residual: 5 };
        let mut state = BattleState::new(vec![mon(100, 50, 255, 0)], vec![mon(100, 50, 255, 0)]);
        // Power-0 moves so only the residual changes HP.
        let mut rng = ScriptedRng::new(vec![1, 2, 0, 0, 0, 0]);
        let out = BattleDriver::execute_turn(
            &provider,
            &mut state,
            [fight_pow(0), fight_pow(0)],
            &mut rng,
        );
        assert_eq!(state.player_battlers[0].hp, 95);
        assert_eq!(state.opponent_battlers[0].hp, 95);
        assert_eq!(
            out.events
                .iter()
                .filter(|e| matches!(e, TurnEvent::Residual { .. }))
                .count(),
            2
        );
    }

    #[test]
    fn faint_leads_to_battle_end_player_win() {
        let provider = DProvider { residual: 0 };
        // Player fast; its power-200 move faints the opponent (100 hp) → PlayerWin.
        let mut state = BattleState::new(vec![mon(100, 99, 255, 0)], vec![mon(100, 10, 255, 0)]);
        let mut rng = ScriptedRng::new(vec![0, 0, 0, 0, 0, 0]);
        let out =
            BattleDriver::execute_turn(&provider, &mut state, [fight_pow(200), fight()], &mut rng);
        assert_eq!(state.opponent_battlers[0].hp, 0);
        assert_eq!(out.battle_over, Some(BattleEnd::PlayerWin));
        assert!(out
            .events
            .iter()
            .any(|e| matches!(e, TurnEvent::Faint { who } if *who == BattlerRef::OPPONENT)));
        // Opponent should NOT have acted (battle short-circuited after faint).
        assert!(!out.events.iter().any(|e| matches!(
            e,
            TurnEvent::MoveUsed { who, .. } if *who == BattlerRef::OPPONENT
        )));
    }

    #[test]
    fn faint_leads_to_battle_end_player_loss() {
        let provider = DProvider { residual: 0 };
        // Opponent fast; its power-200 move faints the player first → PlayerLoss.
        let mut state = BattleState::new(vec![mon(100, 10, 255, 0)], vec![mon(100, 99, 255, 0)]);
        let mut rng = ScriptedRng::new(vec![0, 0, 0, 0, 0, 0]);
        let out =
            BattleDriver::execute_turn(&provider, &mut state, [fight(), fight_pow(200)], &mut rng);
        assert_eq!(state.player_battlers[0].hp, 0);
        assert_eq!(out.battle_over, Some(BattleEnd::PlayerLoss));
    }

    #[test]
    fn switch_swaps_active_slot() {
        let provider = DProvider { residual: 0 };
        let mut state = BattleState::new(
            vec![mon(100, 50, 255, 20), mon(80, 50, 255, 20)],
            vec![mon(100, 50, 255, 20)],
        );
        let switch = BattleAction::Switch { to_slot: 1 };
        // Equal speed: tie keeps submission order, so player's switch resolves
        // first, then opponent fights the newly-active slot.
        let mut rng = ScriptedRng::new(vec![5, 5, 0, 0]);
        let _ = BattleDriver::execute_turn(&provider, &mut state, [switch, fight()], &mut rng);
        // Slot 0 is now the former slot-1 mon (80 max hp), which took 20 from the
        // opponent's fight → 60.
        assert_eq!(state.player_battlers[0].max_hp, 80);
        assert_eq!(state.player_battlers[0].hp, 60);
    }

    #[test]
    fn scripted_rng_draw_order_is_deterministic() {
        // Two identical runs with the same script produce identical outcomes,
        // proving the draw order is stable / game-controlled.
        let run = || {
            let provider = DProvider { residual: 3 };
            let mut state =
                BattleState::new(vec![mon(100, 50, 200, 15)], vec![mon(100, 50, 200, 15)]);
            let mut rng = ScriptedRng::new(vec![7, 2, 10, 0, 10, 0]);
            let _ = BattleDriver::execute_turn(&provider, &mut state, [fight(), fight()], &mut rng);
            (
                state.player_battlers[0].hp,
                state.opponent_battlers[0].hp,
                rng.consumed(),
            )
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn provider_critical_hit_surfaces_into_damage_event() {
        // `CritProvider::roll_critical` always crits (drawing no rng) and its
        // `calculate_damage` doubles damage on a crit. We assert the crit both
        // (a) flows into the formula (doubled damage / hp) and (b) surfaces as
        // `critical: true` on the player's `Damage` event.
        let provider = CritProvider;
        // Player fast (acts first), power-10 move → 20 damage on a crit.
        let mut state = BattleState::new(vec![crit_mon(100, 99, 10)], vec![crit_mon(100, 10, 10)]);
        // Default turn_order/accuracy/roll_critical draw no rng; only the two
        // `calculate_damage` random bytes are consumed.
        let mut rng = ScriptedRng::new(vec![0, 0]);
        let crit_fight = || BattleAction::<CritProvider>::Fight {
            move_: DMove {
                power: 10,
                accuracy: 255,
            },
        };
        let out = BattleDriver::execute_turn(
            &provider,
            &mut state,
            [crit_fight(), crit_fight()],
            &mut rng,
        );

        // (a) Crit doubled the damage in the formula.
        assert_eq!(
            state.opponent_battlers[0].hp, 80,
            "crit doubled 10 → 20 dmg"
        );

        // (b) The player's Damage event reports the crit.
        let player_dmg = out
            .events
            .iter()
            .find_map(|e| match e {
                TurnEvent::Damage {
                    who,
                    critical,
                    amount,
                    ..
                } if *who == BattlerRef::PLAYER => Some((*critical, *amount)),
                _ => None,
            })
            .expect("player should have a Damage event");
        assert!(player_dmg.0, "provider crit must surface as critical: true");
        assert_eq!(player_dmg.1, 20, "Damage event amount reflects the crit");
    }

    #[test]
    fn default_roll_critical_yields_non_critical_damage_event() {
        // `DProvider` does not override `roll_critical`, so the default (never
        // crit, no rng) keeps `critical: false` on the Damage event.
        let provider = DProvider { residual: 0 };
        let mut state = BattleState::new(vec![mon(100, 99, 255, 30)], vec![mon(100, 10, 255, 0)]);
        let mut rng = ScriptedRng::new(vec![0, 0, 0, 0, 0, 0]);
        let out = BattleDriver::execute_turn(
            &provider,
            &mut state,
            [fight_pow(30), fight_pow(0)],
            &mut rng,
        );
        assert!(out.events.iter().any(|e| matches!(
            e,
            TurnEvent::Damage { who, critical: false, .. } if *who == BattlerRef::PLAYER
        )));
    }
}
