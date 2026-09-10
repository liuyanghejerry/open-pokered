//! The effect provider, per-effect state arena, per-move scratch, and the
//! split-borrow [`BattleCtx`] (design §3).
//!
//! The borrow-checker crux: never hand a handler `&mut BattleState` *plus*
//! borrowed battler refs. Instead, hand it a `BattleCtx` of split accessors and
//! resolve via the relay fold (the dispatch loop in
//! [`dispatch`](super::dispatch) owns iteration and re-borrows per step).

use crate::battle::rng::BattleRng;
use crate::battle::{BattleAction, BattleProvider, BattleState, BattlerRef, BattlerState};

use super::event::{Effect, EffectId};

/// The game's effect registry, extending [`BattleProvider`] (design §1.5).
///
/// The engine ships the dispatch machinery with **zero game-specific types**; all
/// specifics (which move maps to which `Effect`, the volatile state enum) live
/// in the game via this trait. For the POC, `EffectStateKind` is a concrete
/// game-supplied associated type (design §3.1 — promote to richer generics when
/// a second game lands).
pub trait EffectProvider: BattleProvider + 'static {
    /// The game-supplied typed per-effect-kind state enum (design §3.1). The
    /// engine treats it opaquely (it only stamps `effect_order` and routes it to
    /// the host). pokered supplies the Gen-1 enum (Toxic counter, Substitute hp,
    /// …).
    type EffectStateKind: Clone;

    /// Resolve the [`Effect`] registered for a given move. Returns `None` if the
    /// move registers no stack hooks.
    fn effect_for_move(&self, m: &Self::Move) -> Option<&'static Effect<Self>>
    where
        Self: Sized;

    /// Resolve the [`Effect`] registered for a non-volatile status (e.g. the
    /// poison residual). Returns `None` if the status registers no hooks.
    fn effect_for_status(&self, s: &Self::Status) -> Option<&'static Effect<Self>>
    where
        Self: Sized;

    /// Resolve the [`Effect`] registered for a **live volatile** in the effect
    /// arena (design §3.4: *every* live effect on a battler contributes its
    /// handlers, not only the non-volatile status). Returns `None` (the default)
    /// when the volatile registers no hooks — so a provider that has no
    /// volatile-borne residuals (every game built on the engine so far) is
    /// completely unaffected and the driver's arena-residual pass is inert.
    ///
    /// This is the generic seam that lets a game host a residual on a *volatile*
    /// (Gen-1 Leech Seed / badly-poisoned both live in `status2`/`status3` bit
    /// flags, NOT the non-volatile `status` byte) without the engine knowing any
    /// game-specific semantics. The `/16`, the toxic counter, and the ASM "status then
    /// leech" order all live in the **game's** handlers; the engine only fires
    /// the hooks the game registers, in the order the game's `order` values dictate.
    fn effect_for_volatile(&self, kind: &Self::EffectStateKind) -> Option<&'static Effect<Self>>
    where
        Self: Sized,
    {
        let _ = kind;
        None
    }

    /// Turn-order rank for `who` — **drawing NO randomness** (design §1.3/§2).
    ///
    /// The driver compares the two ranks; on an exact tie it draws **one** byte
    /// to break it (mirroring pokered's single `order_random` coin flip,
    /// `turn_order.rs:41`, bug #22) — the only turn-order RNG site. This is why
    /// the stack does NOT reuse [`BattleProvider::turn_order_key`] (which draws
    /// per actor): a per-actor draw would consume the wrong number of bytes and
    /// break draw-order parity with the legacy oracle (design §4.1).
    ///
    /// Lower rank acts first; encode "acts earlier" as a *smaller* key
    /// (e.g. `(-priority, -effective_speed)`).
    fn turn_order_rank(
        &self,
        state: &BattleState<Self>,
        who: BattlerRef,
        action: &<Self as BattleProvider>::Move,
    ) -> (i32, i32)
    where
        Self: Sized;

    /// **Cross-turn action override** (design §3 / §9, the multi-turn lock-in
    /// seam). Before the driver executes `actor`'s chosen action, it asks the
    /// game whether a *live volatile* forces a different action this turn — Gen-1
    /// Thrash/Petal Dance and Wrap/Bind re-issue the locked move ignoring the
    /// player's choice, Fly/Dig/Solar Beam strike on the second turn, and Hyper
    /// Beam recharge forces inaction ([`BattleAction::Nothing`]). The game reads
    /// its own `effects` arena (the cross-turn home of the lock counter / charge
    /// flag / recharge flag) and returns `Some(forced)` to override `chosen`, or
    /// `None` to let the chosen action stand.
    ///
    /// This is the **canonical proof** (design §9) that a per-turn `[Action; 2]`
    /// input is insufficient: the locked volatile, recorded on a PRIOR turn,
    /// hijacks this turn's action. The seam is **generic** (the engine names no
    /// game-specific volatile — it only swaps one `BattleAction` for another) and
    /// **defaulted to `None`**, so it is completely INERT for every other game
    /// and for slices 1–5 (which never register a forcing volatile). All Gen-1
    /// lock-in semantics (which volatile forces which move, the lock counter, the
    /// recharge skip) live in the game's `forced_action` impl, never in the engine.
    fn forced_action(
        &self,
        effects: &[EffectState<Self>],
        actor: BattlerRef,
        chosen: &BattleAction<Self>,
    ) -> Option<BattleAction<Self>>
    where
        Self: Sized,
    {
        let _ = (effects, actor, chosen);
        None
    }

    // ── Multi-source collection resolvers (design §2.4, P0b) ─────────────────
    //
    // These four seams are what turns "abilities/items/weather/side-conditions
    // are just Effects" into a working collection pass. The broadened collector
    // (`dispatch::collect_handlers`) calls them so an effect hosted on a
    // battler's ability/item, on a side, or on the field gets a chance to
    // subscribe to an event alongside the move/volatile/status effects. **All
    // four default to `None`/empty**, so a game with no abilities/items/weather
    // (and every existing Gen-1 slice) sees the broadened collector reduce
    // *exactly* to today's single-source behavior — zero new handlers, zero
    // behavioral change, identical `consumed()` draw order.
    //
    // The engine never reads an ability/item's *meaning*; it only fetches the
    // hook table. This is the whole "abilities = effects" mechanism: a resolver
    // + a collection pass, no new engine enum, no `Ability` dispatcher.

    /// Resolve the [`Effect`] registered for a battler's **ability**, hosted on
    /// that battler (design §2.4). Returns `None` (the default) when the game
    /// has no abilities or this one registers no stack hooks.
    fn effect_for_ability(&self, b: &BattlerState<Self>) -> Option<&'static Effect<Self>>
    where
        Self: Sized,
    {
        let _ = b;
        None
    }

    /// Resolve the [`Effect`] registered for a battler's **held item**, hosted
    /// on that battler (design §2.4). Returns `None` (the default) when the game
    /// has no items or this one registers no stack hooks.
    fn effect_for_item(&self, b: &BattlerState<Self>) -> Option<&'static Effect<Self>>
    where
        Self: Sized,
    {
        let _ = b;
        None
    }

    /// Resolve the **side-hosted** effects for `side` (screens, hazards, Wish;
    /// design §2.4). Returns `&[]` (the default) when the game has no side
    /// conditions.
    ///
    /// The returned slice borrows from **`self`** (the provider owns the
    /// registry of `&'static Effect` tables); `ctx` is passed read-only so a
    /// game can decide *which* side conditions are currently live by consulting
    /// its arena/field state. The engine only fetches hook tables — it never
    /// reads a side condition's meaning.
    fn side_effects(&self, ctx: &BattleCtx<'_, Self>, side: u8) -> &[&'static Effect<Self>]
    where
        Self: Sized,
    {
        let _ = (ctx, side);
        &[]
    }

    /// Resolve the **field-hosted** effects (weather, terrain, Trick Room;
    /// design §2.4). Returns `&[]` (the default) when the game has no field
    /// conditions. The returned slice borrows from `self` (see
    /// [`side_effects`](EffectProvider::side_effects)).
    fn field_effects(&self, ctx: &BattleCtx<'_, Self>) -> &[&'static Effect<Self>]
    where
        Self: Sized,
    {
        let _ = ctx;
        &[]
    }
}

/// Where an effect is hosted (design §3.1, the broadened addressing). A
/// cross-gen battle hosts effects not only on a battler (volatiles, ability,
/// item) but on a **side** (screens, hazards, Wish) or the **field** (weather,
/// terrain, Trick Room). `EffectHost` is the 3-way scope the engine routes by.
///
/// ## Non-breaking note (design §3.1, §7)
///
/// The design's §3.1 sketch proposed *widening the `EffectState.host` field*
/// from `BattlerRef` to `EffectHost` "via `From<BattlerRef>` so existing
/// constructors compile unchanged." In Rust that does **not** hold for
/// struct-literal field initialization (`EffectState { host: who, .. }` and the
/// field-shorthand `host,`): field init requires the *exact* field type and
/// never invokes `From`/`Into`. Widening the field would therefore force an edit
/// to all 40+ Gen-1 slice construction sites — a NO-GO on the design's own
/// "Non-breaking" axis (§6.2). See the returned findings for the documented
/// NO-GO.
///
/// So `EffectHost` ships as an **additive type**: `EffectState.host` stays
/// `BattlerRef` (battler-hosted effects, the only kind the engine fires today —
/// every slice compiles verbatim), and [`EffectState::host_scope`] projects it
/// to `EffectHost::Battler`. Side- and field-hosted state is addressed through
/// the `EffectHost::Side`/`Field` cases and the defaulted `side_effects`/
/// `field_effects` resolvers (the game owns that mutable state). `From` and
/// `PartialEq` cross-impls let routing code treat a `BattlerRef` and an
/// `EffectHost::Battler` interchangeably.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectHost {
    /// Hosted on one battler (volatile, ability, item).
    Battler(BattlerRef),
    /// Hosted on a side (screens, hazards, Wish). `0` = player, `1` = opponent.
    Side(u8),
    /// Hosted on the field (weather, terrain, Trick Room).
    Field,
}

impl From<BattlerRef> for EffectHost {
    fn from(r: BattlerRef) -> Self {
        EffectHost::Battler(r)
    }
}

impl PartialEq<BattlerRef> for EffectHost {
    fn eq(&self, other: &BattlerRef) -> bool {
        matches!(self, EffectHost::Battler(r) if r == other)
    }
}

impl PartialEq<EffectHost> for BattlerRef {
    fn eq(&self, other: &EffectHost) -> bool {
        matches!(other, EffectHost::Battler(r) if r == self)
    }
}

/// One live effect's mutable per-instance state, held in an arena keyed by id
/// (design §3.1). `kind` is the game's typed counter enum, so the compiler
/// checks every counter (no positional slot bag).
pub struct EffectState<P: EffectProvider + ?Sized> {
    /// The effect id (arena key; the arena is kept sorted for binary search).
    pub id: EffectId,
    /// The battler this effect is attached to.
    ///
    /// Kept `BattlerRef` (not `EffectHost`) so every existing struct-literal
    /// constructor compiles unchanged (see [`EffectHost`]'s non-breaking note).
    /// Project to the 3-way scope via [`EffectState::host_scope`].
    pub host: BattlerRef,
    /// Monotonic creation counter — the final deterministic tiebreak in the
    /// comparator (design §1.3), stamped at creation, consumes NO rng.
    pub effect_order: u64,
    /// The game's typed counter state.
    pub kind: P::EffectStateKind,
}

impl<P: EffectProvider + ?Sized> EffectState<P> {
    /// The 3-way host scope of this effect (design §3.1). Arena effects are
    /// always battler-hosted today, so this returns `EffectHost::Battler`; the
    /// method is the routing seam the driver uses so a future side/field-hosted
    /// state path can branch on scope without callers caring how `host` is
    /// stored.
    pub fn host_scope(&self) -> EffectHost {
        EffectHost::Battler(self.host)
    }
}

impl<P: EffectProvider + ?Sized> Clone for EffectState<P> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            host: self.host,
            effect_order: self.effect_order,
            kind: self.kind.clone(),
        }
    }
}

/// Per-move scratch shared across a move's event chain (design §3.2): the crit
/// flag, the rolled damage, the miss flag, and the last damage dealt (the
/// canonical home for Counter/Bide reads — design §9 open question).
#[derive(Clone, Copy, Debug, Default)]
pub struct MoveContext {
    /// Whether the in-flight move is a critical hit.
    pub is_critical: bool,
    /// The damage computed for the in-flight move.
    pub damage: u16,
    /// Whether the in-flight move missed.
    pub move_missed: bool,
    /// The last damage actually dealt this turn (Counter/Bide read this).
    pub last_damage: u16,
}

/// Split-borrow context handed to every handler (design §3.2). A handler's only
/// mutable path into the battle is through this struct's accessors.
pub struct BattleCtx<'a, P: EffectProvider + ?Sized> {
    /// The shared battle state (the two party `Vec`s live here).
    pub state: &'a mut BattleState<P>,
    /// The per-effect-instance arena, kept sorted by `id`.
    pub effects: &'a mut Vec<EffectState<P>>,
    /// Per-move scratch.
    pub mv: &'a mut MoveContext,
    /// The ONLY randomness source (design §4).
    pub rng: &'a mut dyn BattleRng,
}

impl<'a, P: EffectProvider + ?Sized> BattleCtx<'a, P> {
    /// One battler as `&mut`. Cross-side is trivial (two different `Vec`s).
    pub fn battler_mut(&mut self, r: BattlerRef) -> &mut BattlerState<P> {
        match r.side {
            0 => &mut self.state.player_battlers[r.slot as usize],
            _ => &mut self.state.opponent_battlers[r.slot as usize],
        }
    }

    /// One battler as `&` (read-only).
    pub fn battler(&self, r: BattlerRef) -> &BattlerState<P> {
        match r.side {
            0 => &self.state.player_battlers[r.slot as usize],
            _ => &self.state.opponent_battlers[r.slot as usize],
        }
    }

    /// Two **disjoint** battler refs as two `&mut` (design §3.2). This is the
    /// borrow-checker crux that lets a Counter-shaped handler mutate `target`
    /// while reading `source`'s host.
    ///
    /// * Cross-side (`a.side != b.side`): the two sides are separate `Vec`s, so
    ///   the refs are *provably* non-aliasing. This is the **one** localized,
    ///   documented `unsafe` in the engine hot path (design §3.2).
    /// * Same-side: a real disjoint split via `split_at_mut` — **fully safe**,
    ///   no raw pointer. (Once MSRV permits, `<[T]>::get_disjoint_mut`.)
    ///
    /// # Panics
    /// Debug-asserts `a != b` (the two refs must address distinct battlers).
    pub fn pair_mut(
        &mut self,
        a: BattlerRef,
        b: BattlerRef,
    ) -> (&mut BattlerState<P>, &mut BattlerState<P>) {
        debug_assert!(a != b, "pair_mut requires two distinct battlers");
        if a.side != b.side {
            // Cross-side: disjoint `Vec`s ⇒ two independent `&mut`.
            let pa: *mut BattlerState<P> = self.battler_mut(a);
            let pb: *mut BattlerState<P> = self.battler_mut(b);
            // SAFETY: `a.side != b.side` ⇒ the two refs index DIFFERENT `Vec`s
            // (`player_battlers` vs `opponent_battlers`), so the resulting
            // `&mut`s can never alias. This is the sole engine `unsafe` and its
            // disjointness is structural, not a runtime invariant.
            unsafe { (&mut *pa, &mut *pb) }
        } else {
            // Same-side: disjoint slots in ONE slice → split_at_mut (safe).
            let v = if a.side == 0 {
                &mut self.state.player_battlers
            } else {
                &mut self.state.opponent_battlers
            };
            let (lo, hi) = (a.slot.min(b.slot) as usize, a.slot.max(b.slot) as usize);
            let (left, right) = v.split_at_mut(hi);
            let (first, second) = (&mut left[lo], &mut right[0]);
            if a.slot < b.slot {
                (first, second)
            } else {
                (second, first)
            }
        }
    }

    /// Mutable access to a live effect's state by id (binary search; the arena
    /// is kept sorted). Returns `None` if no such effect is live.
    pub fn effect_mut(&mut self, id: EffectId) -> Option<&mut EffectState<P>> {
        match self.effects.binary_search_by(|e| e.id.cmp(&id)) {
            Ok(idx) => Some(&mut self.effects[idx]),
            Err(_) => None,
        }
    }

    /// Read access to a live effect's state by id.
    pub fn effect(&self, id: EffectId) -> Option<&EffectState<P>> {
        match self.effects.binary_search_by(|e| e.id.cmp(&id)) {
            Ok(idx) => Some(&self.effects[idx]),
            Err(_) => None,
        }
    }

    /// Install a new game-defined effect (volatile) on `host`, allocating a
    /// fresh arena id + creation order and keeping the arena sorted by id.
    /// Returns the new id. The engine treats `kind` OPAQUELY — this is the
    /// generic seam a data `InflictVolatile` op uses; the game constructs the
    /// `P::EffectStateKind` (via its binding) and the engine only stores it.
    pub fn install_effect(&mut self, host: BattlerRef, kind: P::EffectStateKind) -> EffectId {
        let id = EffectId(self.effects.iter().map(|e| e.id.0).max().unwrap_or(0) + 1);
        self.effects.push(EffectState {
            id,
            host,
            effect_order: id.0 as u64,
            kind,
        });
        self.effects.sort_by_key(|e| e.id.0);
        id
    }
}
