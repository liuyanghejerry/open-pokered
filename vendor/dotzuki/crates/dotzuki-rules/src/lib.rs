//! # dotzuki-rules — no-code RON authoring for the battle effect-stack (Phase 1)
//!
//! A **game-side** loader that turns a
//! declarative `rules.ron` into runtime [`Effect`](dotzuki_engine::battle::stack::Effect)s
//! dispatched through **ONE** zero-capture interpreter-bridge `fn`
//! ([`interpret`]) plus a **closed** primitive-op interpreter ([`run_ops`]).
//!
//! ## What this crate is (and is NOT)
//!
//! * It depends on the game-agnostic [`dotzuki_engine`] **only** — zero
//!   pokered / pokered-core / pokered-data / minimon, zero concrete game type in
//!   non-test code, no `rand`.
//! * It is a **consumer** of the engine's closed primitive vocabulary
//!   (doc 11 §1.1 + doc 12 §3). It **amortizes content** (one `InflictStatus`
//!   covers every secondary-status move) — it does **not extend mechanics**.
//!   A genuinely new mechanic still needs a Rust primitive + test (doc 11 §5).
//!
//! ## The bridge (doc 11 §2 — Option A, ZERO engine change)
//!
//! The fold's only handler call site is a zero-capture `fn` pointer
//! ([`HandlerFn`](dotzuki_engine::battle::stack::HandlerFn)); **data cannot *be* a
//! `fn` pointer**. So every data hook points its `call` field at the single
//! generic [`interpret`] `fn`, which on each call looks up its op-list **by the
//! [`EffectId`] the engine already threads as `source_effect`**
//! (`dispatch.rs:128`). The loader mints one distinct `EffectId` per
//! `(effect, event)` hook and registers each as its own tiny runtime
//! [`Effect`](dotzuki_engine::battle::stack::Effect) **through the existing
//! defaulted resolvers** — exactly the Option-A shape doc 11 §2.2 recommends,
//! and the shape minimon already proves with `effectiveness_chart_hook`. **No
//! engine edit, no new trait method on an engine trait.**
//!
//! ## Determinism (doc 11 §4)
//!
//! The interpreter has **NO entropy except `ctx.rng`** (a `&mut dyn BattleRng`).
//! The `chance` gate compiles to `ctx.rng.chance(num, den)`; there is no clock,
//! no pointer hashing, no `HashMap` iteration affecting draw order. A
//! [`ScriptedRng`](dotzuki_engine::battle::rng::ScriptedRng) replays a data ruleset
//! identically (same draw count and order) as the native path — a **structural**
//! guarantee, proved by [`tests::scripted_rng_replays_identically`].
//!
//! ## Dual-mode sourcing (Phase 2, doc 11 §4.2)
//!
//! [`RuleSource`] yields the **same** runtime [`Ruleset`] from either a **baked**
//! `include_str!`'d text (RELEASE; the default build, zero file IO) or a **disk**
//! path (DEV; behind the `hot-reload` feature it also watches the file and
//! [`RuleSource::poll_changed`] signals an edit so the game rebuilds the registry
//! **between turns**). A mid-battle reload is safe because effects are addressed
//! by [`EffectId`](dotzuki_engine::battle::stack::EffectId) and live state lives in
//! the engine's `EffectState` arena, not the data — the reload swaps the
//! *vocabulary*, never the *in-flight state*.

// `forbid(unsafe_code)` holds on hosted builds; the bare-metal build uses one
// audited `static mut` for the trace sink (std::thread_local! is unavailable
// there), so it downgrades to `warn`.
#![cfg_attr(not(target_os = "none"), forbid(unsafe_code))]
#![cfg_attr(target_os = "none", warn(unsafe_code))]

// no_std port (GBA / thumbv4t): the default build (baked rules, no
// hot-reload) runs bare-metal — the disk source and `notify` watcher are
// hosted-only, and RON parsing uses the vendored no_std ron.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", feature(prelude_import))]
#![cfg_attr(target_os = "none", allow(internal_features))]

extern crate alloc;

#[allow(unused_imports)]
mod alloc_prelude {
    pub use core::prelude::v1::*;
    pub use core::convert::{TryFrom, TryInto};
    pub use alloc::borrow::ToOwned;
    pub use core::iter::FromIterator;
    pub use alloc::boxed::Box;
    pub use alloc::format;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
    pub use core::{assert_eq, assert_ne, matches, todo, unimplemented, write, writeln};
    pub use core::debug_assert;
}

#[cfg_attr(target_os = "none", prelude_import)]
#[allow(unused_imports)]
use alloc_prelude::*;

mod bindings;
mod interp;
mod model;
mod registry;
mod source;
mod trace;

#[cfg(feature = "compile-time")]
pub use dotzuki_rules_macro::rules_ron;

pub use bindings::RuleBindings;
pub use interp::{interpret, run_ops};
pub use model::{
    parse_event, parse_kind, DamageValue, EffectKind, EffectRecord, FinalHitRider, FractionOf,
    HitCount, HookRecord, LoadError, Op, Predicate, Rational, ResourceCost, Ruleset, Selector,
    StatRef, TypeChartEntry, TypeName,
};
pub use registry::{CompiledHook, CompiledRuleset, ResolverKind, RulesHost, RulesProvider};
pub use source::RuleSource;
pub use trace::{enable_trace, take_trace, TraceEvent, TraceSink};

#[cfg(test)]
mod tests;
