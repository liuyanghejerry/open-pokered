//! Shared **game-agnostic mock game** for the engine-side stack tests
//! (design §6, the mock-game style). No game-specific concrete type, no `rand`, no
//! game concept leaks — `TProvider` is a tiny synthetic game whose `Stat` is the
//! 6-stat Gen-4 shape (proving the split is invisible to the engine) and whose
//! resolvers route opaque battler markers to `&'static Effect` hook tables.
//!
//! This module is `#[cfg(test)]` only and used by `authoring`, `dispatch`'s and
//! `mod`'s test modules to avoid re-declaring a provider per file.

use crate::battle::rng::ScriptedRng;
use crate::battle::stack::ctx::{BattleCtx, EffectProvider, EffectState, MoveContext};
use crate::battle::stack::event::Effect;
use crate::battle::{
    BattleProvider, BattleState, BattlerRef, BattlerState, DamageResult, EffectResult, EnumMap,
    MoveEffect,
};

/// 6-stat Gen-4 shape (design §5/§6.1): the physical/special split is just a
/// different `P::Stat` enum, invisible to the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TStat {
    Hp,
    Atk,
    Def,
    Spe,
    SpA,
    SpD,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TStatus {
    Poisoned,
    Burned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TType {
    Normal,
}

/// A "species" doubling as an opaque ability/item marker the resolvers read.
/// The engine never reads its meaning; the mock provider maps it to hook tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TSpecies {
    /// No ability / item.
    Plain,
    /// Hosts the mock "ability" effect (see [`MOCK_ABILITY`]).
    HasAbility,
    /// Hosts the mock "item" effect (see [`MOCK_ITEM`]).
    HasItem,
    /// Hosts both ability and item.
    HasBoth,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TMove {
    pub power: u8,
}

/// The game-supplied typed effect-state enum (design §3.1). `None` is the inert
/// variant; `Vol` marks a generic live volatile the collector can resolve.
#[derive(Clone)]
#[allow(dead_code)]
pub enum TKind {
    None,
    Vol,
    Counter { n: u8 },
}

/// The mock provider. `field_on` toggles whether a field effect is live.
pub struct TProvider {
    pub field_on: bool,
}

impl Default for TProvider {
    fn default() -> Self {
        Self { field_on: false }
    }
}

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
    fn effect_for_volatile(&self, kind: &Self::EffectStateKind) -> Option<&'static Effect<Self>> {
        match kind {
            TKind::Vol => Some(&MOCK_VOLATILE),
            _ => None,
        }
    }
    fn effect_for_ability(&self, b: &BattlerState<Self>) -> Option<&'static Effect<Self>> {
        matches!(b.species, TSpecies::HasAbility | TSpecies::HasBoth).then_some(&MOCK_ABILITY)
    }
    fn effect_for_item(&self, b: &BattlerState<Self>) -> Option<&'static Effect<Self>> {
        matches!(b.species, TSpecies::HasItem | TSpecies::HasBoth).then_some(&MOCK_ITEM)
    }
    fn field_effects(&self, _ctx: &BattleCtx<'_, Self>) -> &[&'static Effect<Self>] {
        if self.field_on {
            &MOCK_FIELD_LIST
        } else {
            &[]
        }
    }
    fn turn_order_rank(
        &self,
        _state: &BattleState<Self>,
        _who: BattlerRef,
        _action: &Self::Move,
    ) -> (i32, i32) {
        (0, 0)
    }
}

// ── Mock effects (registered hook tables) ────────────────────────────────────
//
// Each handler stamps a sentinel into the target/host's `stat_stages[Atk]` so a
// test can read which sources fired and in what order (a write-counter, not a
// game concept). They are `static` so the resolvers can return `&'static`.

use crate::battle::stack::event::{
    EffectId, EffectType, Event, EventHook, HandlerResult, RelayVar,
};

/// Append a marker for `who` by bumping `stat_stages[Atk]` by `tag`.
pub fn mark<P: EffectProvider<Stat = TStat> + ?Sized>(
    ctx: &mut BattleCtx<'_, P>,
    who: BattlerRef,
    tag: i8,
) {
    let b = ctx.battler_mut(who);
    let cur = b.stat_stages.get(TStat::Atk).copied().unwrap_or(0);
    b.stat_stages.set(TStat::Atk, cur + tag);
}

fn ability_hit<P: EffectProvider<Stat = TStat> + ?Sized>(
    ctx: &mut BattleCtx<'_, P>,
    _r: RelayVar,
    target: BattlerRef,
    _s: BattlerRef,
    _e: EffectId,
) -> HandlerResult {
    mark(ctx, target, 1);
    HandlerResult::Unchanged
}
fn item_hit<P: EffectProvider<Stat = TStat> + ?Sized>(
    ctx: &mut BattleCtx<'_, P>,
    _r: RelayVar,
    target: BattlerRef,
    _s: BattlerRef,
    _e: EffectId,
) -> HandlerResult {
    mark(ctx, target, 10);
    HandlerResult::Unchanged
}
fn field_hit<P: EffectProvider<Stat = TStat> + ?Sized>(
    ctx: &mut BattleCtx<'_, P>,
    _r: RelayVar,
    target: BattlerRef,
    _s: BattlerRef,
    _e: EffectId,
) -> HandlerResult {
    mark(ctx, target, 100);
    HandlerResult::Unchanged
}
/// A generic source-effect handler (used by the reduce-to-identity proof).
pub fn mock_src_hit<P: EffectProvider<Stat = TStat> + ?Sized>(
    ctx: &mut BattleCtx<'_, P>,
    _r: RelayVar,
    target: BattlerRef,
    _s: BattlerRef,
    _e: EffectId,
) -> HandlerResult {
    mark(ctx, target, 5);
    HandlerResult::Unchanged
}

fn volatile_hit<P: EffectProvider<Stat = TStat> + ?Sized>(
    ctx: &mut BattleCtx<'_, P>,
    _r: RelayVar,
    target: BattlerRef,
    _s: BattlerRef,
    _e: EffectId,
) -> HandlerResult {
    mark(ctx, target, 50);
    HandlerResult::Unchanged
}

/// Ability: order 10 (fires earlier).
pub static MOCK_ABILITY: Effect<TProvider> = Effect {
    id: EffectId(0xA1),
    kind: EffectType::Condition,
    hooks: &[EventHook {
        event: Event::DamagingHit,
        call: ability_hit::<TProvider>,
        order: 10,
        priority: 0,
        sub_order: None,
    }],
};

/// Item: order 20 (fires after the ability).
pub static MOCK_ITEM: Effect<TProvider> = Effect {
    id: EffectId(0xB1),
    kind: EffectType::Condition,
    hooks: &[EventHook {
        event: Event::DamagingHit,
        call: item_hit::<TProvider>,
        order: 20,
        priority: 0,
        sub_order: None,
    }],
};

/// Field: order 30 (fires last).
pub static MOCK_FIELD: Effect<TProvider> = Effect {
    id: EffectId(0xF1),
    kind: EffectType::Condition,
    hooks: &[EventHook {
        event: Event::DamagingHit,
        call: field_hit::<TProvider>,
        order: 30,
        priority: 0,
        sub_order: None,
    }],
};

/// Volatile (resolved from a live `TKind::Vol` arena entry): order 15.
pub static MOCK_VOLATILE: Effect<TProvider> = Effect {
    id: EffectId(0xC1),
    kind: EffectType::Condition,
    hooks: &[EventHook {
        event: Event::DamagingHit,
        call: volatile_hit::<TProvider>,
        order: 15,
        priority: 0,
        sub_order: None,
    }],
};

/// The provider-owned single-element list `field_effects` borrows from.
pub static MOCK_FIELD_LIST: [&Effect<TProvider>; 1] = [&MOCK_FIELD];

// ── Builders ─────────────────────────────────────────────────────────────────

/// A battler with the given hp and species marker.
pub fn mon(hp: u16, species: TSpecies) -> BattlerState<TProvider> {
    let mut stats = EnumMap::new();
    stats.set(TStat::Hp, hp);
    BattlerState::new(species, hp, hp, stats, vec![TMove { power: 10 }])
}

/// Owned `(state, effects, mv, rng)` parts for a `BattleCtx` in a test.
pub fn parts(
    player: BattlerState<TProvider>,
    opp: BattlerState<TProvider>,
) -> (
    BattleState<TProvider>,
    Vec<EffectState<TProvider>>,
    MoveContext,
    ScriptedRng,
) {
    (
        BattleState::new(vec![player], vec![opp]),
        Vec::new(),
        MoveContext::default(),
        ScriptedRng::new(vec![]),
    )
}

/// Read a battler's accumulated marker (`stat_stages[Atk]`).
pub fn marker(state: &BattleState<TProvider>, who: BattlerRef) -> i8 {
    let b = if who.side == 0 {
        &state.player_battlers[who.slot as usize]
    } else {
        &state.opponent_battlers[who.slot as usize]
    };
    b.stat_stages.get(TStat::Atk).copied().unwrap_or(0)
}
