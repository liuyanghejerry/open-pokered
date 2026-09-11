//! The game binding seam (doc 11 §1, doc 12 §3.2).
//!
//! `dotzuki-rules` stays game-agnostic: it never names a concrete `P::Stat` /
//! `P::Status` / `P::Type`. The data uses **names**, interned to `usize` indices
//! at load. To actually apply a `Boost`/`InflictStatus`/`HasType`/`ApplyTypeChart`
//! against a [`BattlerState<P>`](dotzuki_engine::battle::BattlerState) the interpreter
//! asks the **game** to resolve those indices via this trait. This mirrors doc 12
//! §3.2's two defaulted provider seams (`defender_types`, `type_chart_mult`), but
//! lives game-side (no engine edit), keyed by the interned index the loader owns.
//!
//! All methods are **pure / RNG-free** (determinism, doc 11 §4.1): a binding may
//! read battler state and the interned chart but must never draw randomness. The
//! interpreter's only entropy is `ctx.rng`.

use dotzuki_engine::battle::stack::{BattleCtx, EffectProvider};
use dotzuki_engine::battle::{BattlerRef, BattlerState};

/// Resolves interned data-layer indices to concrete `P::Stat`/`P::Status` and
/// supplies the type-chart fold + defender-type membership, all pure.
///
/// A game implements this once for its provider; the loader carries it so the
/// zero-capture [`interpret`](crate::interpret) bridge can reach it. The trait is
/// generic over `P: EffectProvider`, so the engine learns nothing.
pub trait RuleBindings<P: EffectProvider + ?Sized>: 'static {
    /// Apply a signed stat-stage delta to `who` for the interned `stat_index`.
    /// Returns `false` if the index is unknown (a no-op; the loader validates
    /// names at compile, so this is defense-in-depth). Phase 1 applies directly;
    /// the nested-`TryBoost` veto is driver orchestration (doc 11 §3).
    fn apply_boost(&self, b: &mut BattlerState<P>, stat_index: usize, stages: i8) -> bool;

    /// Set `who`'s non-volatile status for the interned `status_index`. Returns
    /// `false` if the index is unknown.
    fn set_status(&self, b: &mut BattlerState<P>, status_index: usize) -> bool;

    /// Set `who`'s non-volatile status carrying a game-interpreted numeric
    /// `amount` (e.g. Gen-1 sleep turns). The engine resolves `amount` from the
    /// op's [`AmountSpec`](crate::AmountSpec) — drawing its OWN rng — and hands
    /// the pure number here. **Defaulted** to delegate to
    /// [`set_status`](Self::set_status) and ignore the amount, so a game whose
    /// statuses carry no duration is unaffected. Pure; no entropy.
    fn set_status_with_amount(
        &self,
        b: &mut BattlerState<P>,
        status_index: usize,
        _amount: u16,
    ) -> bool {
        self.set_status(b, status_index)
    }

    /// Build the game's OPAQUE `P::EffectStateKind` volatile for the vocabulary
    /// `name` + already-resolved `amount` (the [`InflictVolatile`](crate::Op::InflictVolatile)
    /// op). The engine installs whatever is returned generically (fresh arena
    /// id) and never learns what the volatile means — only the game does.
    /// **Defaulted to `None`** ⇒ a game with no volatiles (or that doesn't
    /// recognise `name`) makes the op inert. Pure — the engine already drew any
    /// rng needed for `amount`.
    fn make_volatile(&self, _name: &str, _amount: u16) -> Option<P::EffectStateKind> {
        None
    }

    /// Whether `who` has the type with interned chart `type_index` (the `HasType`
    /// predicate, doc 11 §1.1). Pure read.
    fn has_type(&self, b: &BattlerState<P>, type_index: usize) -> bool;

    /// The chart fold for the in-flight `move_type_index` against `defender`'s
    /// type(s), as ONE pre-combined integer rational `(num, den)` (doc 12 §3.2,
    /// §5.3 — one rational ⇒ exactly one `scale`, avoiding per-step truncation).
    /// Default `(1, 1)` ⇒ inert (no chart). Pure / RNG-free.
    fn type_chart_mult(
        &self,
        _ctx: &BattleCtx<'_, P>,
        _move_type_index: usize,
        _defender: BattlerRef,
    ) -> (u32, u32) {
        (1, 1)
    }

    /// The in-flight folded stat index, if the driver stashed one for a
    /// `StatIs` predicate (the Sandstorm `WeatherModifyStat` case, doc 11 §1).
    /// Default `None` ⇒ `StatIs` never matches. Pure.
    fn current_stat_index(&self, _ctx: &BattleCtx<'_, P>) -> Option<usize> {
        None
    }

    /// Whether `who` currently has the live volatile named by `name` (the
    /// `HasVolatile` predicate, blueprint `15` §2/§3 — the Substitute block on
    /// side-status). The game inspects its own `ctx.effects` arena (the engine
    /// treats `EffectStateKind` opaquely, so only the game can tell which arena
    /// entry IS "Substitute"). **Defaulted to `false`** so a game with no volatiles
    /// (every game built so far) is unaffected and the predicate never matches.
    /// Pure read; no entropy.
    fn has_volatile(&self, _ctx: &BattleCtx<'_, P>, _who: BattlerRef, _name: &str) -> bool {
        false
    }

    /// **Damage-redirection seam** for the DIRECT-MUTATE ops (`SetHp` / `DamageFraction`
    /// / `DamageCurrentHpFraction` / `RepeatHits`) that apply HP OUTSIDE the driver's
    /// `Event::Damage` fold. Before such an op subtracts `amount` HP from `who`
    /// (attributed to `source`), the interpreter asks the game whether a **damage sink**
    /// on `who` should swallow it instead — a monster Substitute doll, a cross-game
    /// shield/ward/decoy. Returning `true` means the game HANDLED the loss (it mutated
    /// its own sink via `ctx`); the interpreter then SKIPS the direct HP write. Returning
    /// `false` (the default) leaves the op to apply HP exactly as before.
    ///
    /// This is the ONLY binding permitted to MUTATE through `ctx` (every other is a pure
    /// read) — it is the redirect analogue of the `TryBoost`/`Event::Damage` interception
    /// the driver already fires for formula damage, extended to the ops the driver never
    /// routes. **Defaulted to `false`** so every existing game (and every op) is
    /// byte-identical: the loss applies unredirected, no `ctx` mutation, no entropy.
    /// `source` lets a game exempt self-inflicted loss (recoil / self-KO) from its own
    /// sink. Draws NO randomness.
    fn redirect_hp_loss(
        &self,
        _ctx: &mut BattleCtx<'_, P>,
        _who: BattlerRef,
        _source: BattlerRef,
        _amount: u16,
    ) -> bool {
        false
    }

    /// Whether the in-flight move's type (`move_type_index`, recovered from the
    /// record's `type:`) equals one of `who`'s types (the `MoveTypeIsDefenderType`
    /// predicate — Gen-1 burn/freeze/paralyze self-type-immunity quirk #23,
    /// blueprint `15` §2/§3). **Defaulted to `false`** ⇒ the quirk never fires for a
    /// game that does not implement it. Pure read; no entropy. The default body
    /// delegates to [`has_type`](Self::has_type) so a game whose `has_type` already
    /// answers chart membership gets the quirk for free by overriding nothing — but
    /// the engine has no `move_type_index` for a generic predicate, so the loader
    /// passes it through and the binding decides.
    fn move_type_is_defender_type(
        &self,
        ctx: &BattleCtx<'_, P>,
        move_type_index: usize,
        who: BattlerRef,
    ) -> bool {
        self.has_type(ctx.battler(who), move_type_index)
    }

    /// Whether `who` currently has the non-volatile status at interned
    /// `status_index` (the `TargetHasStatus` predicate — the Dream Eater sleep
    /// gate, blueprint `15` §2). The status index is the game's vocabulary (the same
    /// indices `set_status` consumes). **Defaulted to `false`** ⇒ a game that does
    /// not implement it never matches. Pure read; no entropy.
    fn has_status(&self, _b: &BattlerState<P>, _status_index: usize) -> bool {
        false
    }

    /// Whether `b` has ANY non-volatile status (the `TargetHasAnyStatus`
    /// predicate — the Toxic "already-statused ⇒ fail" guard). The engine knows
    /// no concrete status, so the game answers. **Defaulted to `false`** ⇒ a game
    /// that doesn't implement it never matches. Pure read; no entropy.
    fn has_any_status(&self, _b: &BattlerState<P>) -> bool {
        false
    }

    /// The level of battler `b` (the `LevelGE` predicate's gate + the level-based
    /// [`SetDamage`](crate::Op::SetDamage) sources — Seismic Toss / Night Shade /
    /// Psywave; blueprint `15` §2/§3). [`BattlerState<P>`](dotzuki_engine::battle::BattlerState)
    /// carries no `level` field (the engine is level-agnostic), so the game answers
    /// it here. **Defaulted to `0`** ⇒ a game that authors no level-gated op is
    /// unaffected (it never calls this) and `LevelGE` is `0 >= 0 == true`. Pure
    /// read; no entropy. Game-agnostic: "a number the binding supplies per battler".
    fn battler_level(&self, _b: &BattlerState<P>) -> u16 {
        0
    }

    /// Map an interned resource index (the ruleset's `resources:` order) to the
    /// engine's opaque resource id used in [`ResourcePool`](dotzuki_engine::battle::ResourcePool)
    /// / [`BattleProvider::move_cost`](dotzuki_engine::battle::BattleProvider::move_cost)
    /// (the MP/SP/mana cost gate, doc 13 §4). **Defaulted** so games with no
    /// resources need not implement it — they never reference a resource, so it is
    /// never called. Default: identity (`index as u16`). Pure.
    fn resource_id(&self, resource_index: usize) -> u16 {
        resource_index as u16
    }

    /// Whether `b` can pay `amount` of the resource at interned `resource_index`
    /// (the `PayResource` op's gate, doc 13 §4). Pure read. The default delegates
    /// to the engine's [`ResourcePool`](dotzuki_engine::battle::ResourcePool) via
    /// [`resource_id`](Self::resource_id) — a game that stores its pool on
    /// `BattlerState.resources` (the engine default) gets correct behavior for free.
    fn can_pay_resource(&self, b: &BattlerState<P>, resource_index: usize, amount: u16) -> bool {
        b.can_pay_resource(self.resource_id(resource_index), amount)
    }

    /// Deduct `amount` of the resource at interned `resource_index` from `b` (the
    /// `PayResource` op's deduction). **Pure arithmetic — no rng.** Default
    /// delegates to the engine `ResourcePool`.
    fn pay_resource(&self, b: &mut BattlerState<P>, resource_index: usize, amount: u16) {
        b.pay_resource(self.resource_id(resource_index), amount);
    }
}
