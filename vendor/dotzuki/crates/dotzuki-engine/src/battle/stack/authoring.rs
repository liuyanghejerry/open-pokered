//! Developer-facing **effect-authoring** helpers (design §4.1).
//!
//! This is the second of the two real gaps the generalization closes (design
//! §0): today the only `HandlerFn`s that exist are buried in pokered's
//! `#[cfg(test)]` parity harness, with no ergonomic, documented way to author a
//! move / ability / item / weather. This module is **concept-free**: it adds no
//! new runtime type — it is pure constructors over the existing
//! [`Effect`](super::event::Effect) / [`EventHook`](super::event::EventHook)
//! shape (plus the typed [`RelayVar`](super::event::RelayVar) accessors, which
//! live on `RelayVar` itself). It introduces **no** game concept and **no**
//! `rand`; a game authors a `const Effect` with zero-capture `fn` handlers.
//!
//! ## The `effect!` macro
//!
//! Builds the same `&'static [EventHook]` table the engine folds, in a
//! `const`/`static` context, so registrations stay zero-alloc constants:
//!
//! ```
//! # use dotzuki_engine::battle::stack::event::{Effect, EffectId, EffectType,
//! #     Event, EventHook, HandlerFn, HandlerResult, RelayVar};
//! # use dotzuki_engine::battle::stack::ctx::{BattleCtx, EffectProvider};
//! # use dotzuki_engine::battle::BattlerRef;
//! # use dotzuki_engine::effect;
//! # fn my_hit<P: EffectProvider + ?Sized>(
//! #     _c: &mut BattleCtx<'_, P>, _r: RelayVar,
//! #     _t: BattlerRef, _s: BattlerRef, _e: EffectId) -> HandlerResult {
//! #     HandlerResult::Unchanged
//! # }
//! # fn my_residual<P: EffectProvider + ?Sized>(
//! #     _c: &mut BattleCtx<'_, P>, _r: RelayVar,
//! #     _t: BattlerRef, _s: BattlerRef, _e: EffectId) -> HandlerResult {
//! #     HandlerResult::Unchanged
//! # }
//! # trait MyGame: EffectProvider {}
//! fn flamethrower<P: EffectProvider + ?Sized>() -> Effect<P> {
//!     effect!(EffectId(0x10), EffectType::Move, {
//!         DamagingHit       => my_hit::<P>,
//!         Residual(20)      => my_residual::<P>,  // explicit on<Event>Order
//!     })
//! }
//! ```
//!
//! The first form (`Event => fn`) defaults `order` to `u32::MAX` (fires last,
//! matching the engine's `EventHook` default); the second form
//! (`Event(order) => fn`) sets `on<Event>Order` explicitly. `priority` defaults
//! to `0` and `sub_order` to `None` (derive from the effect's `EffectType`),
//! exactly as a hand-written `EventHook` would.

/// Declarative effect builder (design §4.1). Expands to an
/// [`Effect`](super::event::Effect) whose `hooks` is a `&'static` slice of
/// [`EventHook`](super::event::EventHook)s. Usable anywhere an `Effect`
/// expression is expected (incl. the initializer of a `const`/`static`).
///
/// Syntax: `effect!(<id expr>, <EffectType expr>, { <Event> [(<order>)] => <fn path>, ... })`
///
/// * `<Event>` is bare (e.g. `DamagingHit`) — it is qualified to
///   `$crate::battle::stack::event::Event::<Event>` by the macro.
/// * `(<order>)` is optional; omitted ⇒ `u32::MAX` (fires last).
/// * `<fn path>` is any path to a [`HandlerFn`](super::event::HandlerFn)
///   (a zero-capture `fn`), e.g. `my_handler::<P>`.
#[macro_export]
macro_rules! effect {
    ($id:expr, $kind:expr, { $( $ev:ident $( ( $ord:expr ) )? => $fn:path ),* $(,)? }) => {
        $crate::battle::stack::event::Effect {
            id: $id,
            kind: $kind,
            hooks: &[ $(
                $crate::battle::stack::event::EventHook {
                    event: $crate::battle::stack::event::Event::$ev,
                    call: $fn,
                    order: $crate::effect!(@ord $( $ord )?),
                    priority: 0,
                    sub_order: None,
                },
            )* ],
        }
    };
    // order helpers: explicit value, or the `u32::MAX` default (fires last).
    (@ord $o:expr) => { $o };
    (@ord) => { u32::MAX };
}

#[cfg(test)]
mod tests {
    use crate::battle::stack::ctx::{BattleCtx, EffectProvider};
    use crate::battle::stack::event::{EffectId, EffectType, Event, HandlerResult, RelayVar};
    use crate::battle::BattlerRef;

    // A generic zero-capture handler usable by any provider.
    fn noop<P: EffectProvider + ?Sized>(
        _c: &mut BattleCtx<'_, P>,
        _r: RelayVar,
        _t: BattlerRef,
        _s: BattlerRef,
        _e: EffectId,
    ) -> HandlerResult {
        HandlerResult::Unchanged
    }

    // A provider just to monomorphize the macro output.
    use crate::battle::stack::tests_support::TProvider;

    #[test]
    fn effect_macro_builds_expected_hooks() {
        let eff = effect!(EffectId(0x10), EffectType::Move, {
            DamagingHit       => noop::<TProvider>,
            Residual(20)      => noop::<TProvider>,
        });
        assert_eq!(eff.id, EffectId(0x10));
        assert_eq!(eff.kind, EffectType::Move);
        assert_eq!(eff.hooks.len(), 2);
        // First hook: default order = u32::MAX (fires last).
        assert_eq!(eff.hooks[0].event, Event::DamagingHit);
        assert_eq!(eff.hooks[0].order, u32::MAX);
        assert_eq!(eff.hooks[0].priority, 0);
        assert_eq!(eff.hooks[0].sub_order, None);
        // Second hook: explicit order = 20.
        assert_eq!(eff.hooks[1].event, Event::Residual);
        assert_eq!(eff.hooks[1].order, 20);
    }

    #[test]
    fn relay_typed_accessors_and_scale() {
        assert_eq!(RelayVar::Int(7).as_int(), 7);
        assert_eq!(RelayVar::Unit.as_int(), 0);
        assert_eq!(RelayVar::Damage(40).as_damage(), 40);
        assert_eq!(RelayVar::Int(0).as_damage(), 0);
        assert_eq!(RelayVar::Accuracy(200).as_accuracy(), 200);
        assert!(RelayVar::Bool(true).as_bool());
        assert!(!RelayVar::Bool(false).as_bool());
        assert!(!RelayVar::Unit.as_bool());
        // scale keeps the lane and applies num/den.
        assert_eq!(RelayVar::Int(10).scale(3, 2), RelayVar::Int(15));
        assert_eq!(RelayVar::Damage(40).scale(3, 2), RelayVar::Damage(60));
        assert_eq!(RelayVar::Damage(100).scale(1, 2), RelayVar::Damage(50));
        // div-by-zero is clamped to /1 (no panic).
        assert_eq!(RelayVar::Int(10).scale(2, 0), RelayVar::Int(20));
        // non-numeric relays pass through.
        assert_eq!(RelayVar::Unit.scale(3, 2), RelayVar::Unit);
    }
}
