//! Handler collection, the Showdown `comparePriority` comparator, the speed-tie
//! draw, and the `run_event` dispatch fold (design §1.3, §3.4).

use core::cmp::Ordering;

use crate::battle::rng::BattleRng;
use crate::battle::BattlerRef;

use super::ctx::{BattleCtx, EffectProvider};
use super::event::{Effect, EffectId, Event, HandlerFn, HandlerResult, RelayVar};

/// One collected, sortable handler invocation (design §1.3). The comparator
/// orders these by the exact Showdown lexical order.
pub struct CollectedHandler<P: EffectProvider + ?Sized> {
    /// `on<Event>Order`; default `u32::MAX` fires last; LOW first.
    pub order: u32,
    /// `on<Event>Priority`; HIGH first.
    pub priority: i32,
    /// The host battler's current speed; HIGH first (speed-sort).
    pub speed: u32,
    /// Effect-type sub-order; LOW first.
    pub sub_order: u8,
    /// Monotonic creation counter — the final deterministic tiebreak; LOW first.
    pub effect_order: u64,
    /// The event target.
    pub target: BattlerRef,
    /// The event source.
    pub source: BattlerRef,
    /// Which effect registered this handler.
    pub source_effect: EffectId,
    /// The native handler.
    pub call: HandlerFn<P>,
}

impl<P: EffectProvider + ?Sized> Clone for CollectedHandler<P> {
    fn clone(&self) -> Self {
        Self {
            order: self.order,
            priority: self.priority,
            speed: self.speed,
            sub_order: self.sub_order,
            effect_order: self.effect_order,
            target: self.target,
            source: self.source,
            source_effect: self.source_effect,
            call: self.call,
        }
    }
}

/// The exact Showdown `comparePriority` lexical order (design §1.3):
/// **order → priority → speed → sub_order → effect_order**.
///
/// `order`/`sub_order`/`effect_order` are ascending (LOW first);
/// `priority`/`speed` are descending (HIGH first).
pub fn compare<P: EffectProvider + ?Sized>(
    a: &CollectedHandler<P>,
    b: &CollectedHandler<P>,
) -> Ordering {
    a.order
        .cmp(&b.order)
        .then(b.priority.cmp(&a.priority))
        .then(b.speed.cmp(&a.speed))
        .then(a.sub_order.cmp(&b.sub_order))
        .then(a.effect_order.cmp(&b.effect_order))
}

/// Collect, from a single known effect, the hooks that subscribe to `ev`,
/// wrapping each with its host's speed and `effect_order` from the arena.
///
/// Scoping note (design §1.3): the POC collects from the **one effect explicitly
/// passed by the driver** (the move's effect, the host's status effect) rather
/// than synthesizing `OnAny/OnFoe/OnSource/OnAlly` prefix variants across every
/// live effect — no Gen-1 effect registers a prefixed hook, so that seam stays
/// present-but-inert. This keeps the slice minimal per the doc's explicit
/// permission while preserving the comparator's full shape.
pub fn collect_from_effect<P: EffectProvider + ?Sized>(
    ctx: &BattleCtx<'_, P>,
    eff: &'static Effect<P>,
    ev: Event,
    target: BattlerRef,
    source: BattlerRef,
    out: &mut Vec<CollectedHandler<P>>,
) {
    // Delegates to the shared `push_matching` so the single-source slice path
    // and the multi-source `collect_handlers` path emit byte-identical
    // `CollectedHandler`s for the same effect (comparator tiers incl. the inert
    // `speed = 0` and the arena-or-id `effect_order` fallback). This identity is
    // what keeps the 88 Gen-1 slices' `consumed()` draw order unchanged.
    push_matching(ctx, eff, ev, target, source, out);
}

/// Push every hook in `eff` that subscribes to `ev` into `out`, stamping each
/// with the comparator tiers. Shared by [`collect_from_effect`] (single-source,
/// the slice path) and [`collect_handlers`] (multi-source, §2.2) so both paths
/// produce **byte-identical** `CollectedHandler`s for the same effect.
fn push_matching<P: EffectProvider + ?Sized>(
    ctx: &BattleCtx<'_, P>,
    eff: &'static Effect<P>,
    ev: Event,
    target: BattlerRef,
    source: BattlerRef,
    out: &mut Vec<CollectedHandler<P>>,
) {
    // `speed` tier: the engine cannot name a game-specific "speed" stat from the
    // opaque `P::Stat`, so it stays 0 (an inert tier, as for the slices).
    let speed = 0;
    // effect_order: prefer the live arena entry; fall back to the effect id so
    // moves/abilities/items (no arena entry) still get a deterministic,
    // RNG-free tiebreak.
    let effect_order = ctx
        .effect(eff.id)
        .map(|s| s.effect_order)
        .unwrap_or(eff.id.0 as u64);

    for hook in eff.hooks {
        if hook.event != ev {
            continue;
        }
        out.push(CollectedHandler {
            order: hook.order,
            priority: hook.priority,
            speed,
            sub_order: hook.sub_order.unwrap_or_else(|| eff.kind.sub_order()),
            effect_order,
            target,
            source,
            source_effect: eff.id,
            call: hook.call,
        });
    }
}

/// **Multi-source** handler collection (design §2.2) — the broadened collector.
///
/// Gathers the hooks subscribing to `ev` from **every live source**, not just
/// the one effect the driver passes:
///
/// 1. the **source effect** (the move/volatile that triggered the dispatch),
/// 2. every **live volatile** on `target` and on `source` (arena scan →
///    `effect_for_volatile`),
/// 3. each relevant battler's **ability** and **held item**
///    (`effect_for_ability` / `effect_for_item`),
/// 4. the **side** effects of `target`'s and `source`'s sides (`side_effects`),
/// 5. the **field** effects (`field_effects`).
///
/// ## Reduces to identity (the non-breaking guarantee)
///
/// Steps 3–5 go through the four resolvers that **default to `None`/empty**
/// (§2.4); step 2 goes through `effect_for_volatile` (defaulted `None`). So for
/// a game with no abilities/items/weather/side-conditions and no live volatiles
/// — i.e. every existing Gen-1 slice scenario that fires a move event with an
/// empty arena — this collector pushes **exactly** what `push_matching(src_eff)`
/// alone pushes, in the same order, with byte-identical comparator tiers. The
/// broadened gather adds **read fan-out, never new handlers**, until a game
/// implements a resolver.
///
/// ## Borrow safety (design §2.3)
///
/// Takes only `&BattleCtx` (shared) and fills an **owned** `Vec` whose entries
/// hold the `HandlerFn` pointer + `EffectId` + `BattlerRef`s **by value** — no
/// borrows into the arena or battlers. Once collected, the snapshot is
/// independent of `ctx`, so the fold can hand each handler a `&mut BattleCtx`
/// without aliasing the iterator. No `RefCell`, no new `unsafe`.
pub fn collect_handlers<P: EffectProvider>(
    ctx: &BattleCtx<'_, P>,
    provider: &P,
    src_eff: Option<&'static Effect<P>>,
    ev: Event,
    target: BattlerRef,
    source: BattlerRef,
    out: &mut Vec<CollectedHandler<P>>,
) {
    // 1. The source effect (the move/volatile the driver resolved), if any.
    if let Some(eff) = src_eff {
        push_matching(ctx, eff, ev, target, source, out);
    }

    // 2. Live volatiles on target & source (arena scan → effect_for_volatile).
    //    Walk the arena in its stable `id` order so the gather is deterministic
    //    and RNG-free. We only *read* the arena here (shared borrow); the owned
    //    snapshot in `out` decouples this read from any later mutation.
    for e in ctx.effects.iter() {
        if e.host != target && e.host != source {
            continue;
        }
        if let Some(eff) = provider.effect_for_volatile(&e.kind) {
            push_matching(ctx, eff, ev, target, source, out);
        }
    }

    // 3. Ability + held item on each relevant battler. Defaulted resolvers ⇒
    //    None ⇒ skipped. `source`/`target` may coincide (a self-targeting
    //    event); dedup is unnecessary because the comparator + effect_order make
    //    the fold deterministic and a battler's own ability listing twice would
    //    be a game authoring choice, not an engine one — but we still avoid the
    //    obvious double when target == source.
    let battlers: &[BattlerRef] = if target == source {
        core::slice::from_ref(&source)
    } else {
        &[target, source]
    };
    for &who in battlers {
        let b = ctx.battler(who);
        if let Some(eff) = provider.effect_for_ability(b) {
            push_matching(ctx, eff, ev, who, source, out);
        }
        if let Some(eff) = provider.effect_for_item(b) {
            push_matching(ctx, eff, ev, who, source, out);
        }
    }

    // 4. Side effects of target's & source's sides. Defaulted ⇒ empty.
    for &eff in provider.side_effects(ctx, target.side) {
        push_matching(ctx, eff, ev, target, source, out);
    }
    if source.side != target.side {
        for &eff in provider.side_effects(ctx, source.side) {
            push_matching(ctx, eff, ev, target, source, out);
        }
    }

    // 5. Field effects. Defaulted ⇒ empty.
    for &eff in provider.field_effects(ctx) {
        push_matching(ctx, eff, ev, target, source, out);
    }
}

/// Whether `who` is still on the field (hp > 0). Game-agnostic: reads `hp` only.
fn is_alive<P: EffectProvider + ?Sized>(ctx: &BattleCtx<'_, P>, who: BattlerRef) -> bool {
    ctx.battler(who).hp > 0
}

/// Permute *only* the tied runs (`compare == Equal`) by drawing one byte from
/// the rng per tie — the single source of true handler-order randomness
/// (design §1.3, mirroring pokered's order coin-flip, bug #22).
///
/// `hs` must already be sorted by [`compare`]. Within each maximal run of equal
/// entries, adjacent pairs are conditionally swapped on a `byte < 128` flip —
/// the same comparison shape as `turn_order.rs:41`.
pub fn speed_sort_tiebreak<P: EffectProvider + ?Sized>(
    hs: &mut [CollectedHandler<P>],
    rng: &mut dyn BattleRng,
) {
    let mut i = 0;
    while i < hs.len() {
        let mut j = i + 1;
        while j < hs.len() && compare(&hs[i], &hs[j]) == Ordering::Equal {
            j += 1;
        }
        // [i, j) is a tied run. For a run of length >= 2, flip each adjacent
        // pair (bubble one pass) using a coin per comparison — the same
        // single-byte `< 128` flip pokered uses for the order tie.
        if j - i >= 2 {
            for k in i..(j - 1) {
                if rng.next_u8() >= 128 {
                    hs.swap(k, k + 1);
                }
            }
        }
        i = j;
    }
}

/// The dispatch fold (design §3.4) — the workhorse.
///
/// Collects handlers (already done by the caller via `collect_from_effect` into
/// `hs`), sorts by [`compare`], permutes ties via the rng, then folds each
/// handler over the `relay`. The loop **owns `hs`** (collected before the fold),
/// so no handler can invalidate another mid-fold. Handlers take `&mut BattleCtx`
/// — never borrowed battler refs — so the `&mut` borrow lives only inside each
/// call. No `RefCell`, no `unsafe` here (the only `unsafe` is the
/// provably-disjoint cross-side `pair_mut`).
///
/// `fast_exit` returns on the first `Set` (redirection / first-blood, the
/// `priority_event` shape).
pub fn run_event<P: EffectProvider + ?Sized>(
    ctx: &mut BattleCtx<'_, P>,
    mut hs: Vec<CollectedHandler<P>>,
    mut relay: RelayVar,
    fast_exit: bool,
) -> RelayVar {
    hs.sort_by(compare);
    speed_sort_tiebreak(&mut hs, ctx.rng);
    for h in hs {
        match (h.call)(ctx, relay, h.target, h.source, h.source_effect) {
            HandlerResult::Unchanged => {}
            HandlerResult::Set(v) => {
                relay = v;
                if fast_exit {
                    return relay;
                }
            }
            HandlerResult::Fail => return RelayVar::Bool(false),
            HandlerResult::FailSilent => return RelayVar::Unit,
        }
    }
    relay
}

/// The dispatch fold **with the §2.3 per-step liveness re-check** — the
/// multi-source variant.
///
/// Identical to [`run_event`] (same sort, same tie-break draw, same `fast_exit`
/// fold) except that **before each handler fires** it re-checks that the
/// handler's `target` is still alive — because a multi-source fold can collect
/// handlers from several effects, and an earlier handler (e.g. a weather chip,
/// or a contact-ability) can KO the `target` an later handler was about to act
/// on. The re-check is a pure **read** between calls while the loop holds the
/// sole `&mut`, so it never aliases the snapshot.
///
/// Source-effect removal mid-fold (a handler removing another live volatile) is
/// left to each game handler's own post-mutation guard, matching the existing
/// slice contract (`driver.rs`: "each game handler is responsible for its own
/// post-faint guard") — `CollectedHandler` carries no arena borrow, so a
/// removed source effect simply means `ctx.effect(source_effect)` returns
/// `None`, which the handler reads defensively.
///
/// `run_event` is kept separate and unchanged so the 88 Gen-1 slices' fold is
/// byte-identical; this variant is for the broadened multi-source path.
pub fn run_event_checked<P: EffectProvider + ?Sized>(
    ctx: &mut BattleCtx<'_, P>,
    mut hs: Vec<CollectedHandler<P>>,
    mut relay: RelayVar,
    fast_exit: bool,
) -> RelayVar {
    hs.sort_by(compare);
    speed_sort_tiebreak(&mut hs, ctx.rng);
    for h in hs {
        // §2.3 re-check: a prior handler may have KO'd this handler's target.
        if !is_alive(ctx, h.target) {
            continue;
        }
        match (h.call)(ctx, relay, h.target, h.source, h.source_effect) {
            HandlerResult::Unchanged => {}
            HandlerResult::Set(v) => {
                relay = v;
                if fast_exit {
                    return relay;
                }
            }
            HandlerResult::Fail => return RelayVar::Bool(false),
            HandlerResult::FailSilent => return RelayVar::Unit,
        }
    }
    relay
}
