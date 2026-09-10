//! The runtime bridge + the closed primitive interpreter (doc 11 §2, §1.1).
//!
//! [`interpret`] is the **single** zero-capture `fn` every data hook's `call`
//! field points at. It looks up its op-list by the [`EffectId`] the engine
//! threads as `source_effect` (`dispatch.rs:128`), then folds it via [`run_ops`].
//!
//! [`run_ops`] is a **pure interpreter over `ctx` + the closed op enum**: it
//! mutates only through `ctx` (`battler_mut` / `effect_mut` / the binding), and
//! its **only entropy is `ctx.rng`** (the `chance` gate; doc 11 §4.1). No clock,
//! no pointer hashing, no `HashMap` iteration affecting draw order. A
//! [`ScriptedRng`](dotzuki_engine::battle::rng::ScriptedRng) therefore replays a
//! data ruleset identically.

use dotzuki_engine::battle::stack::{BattleCtx, EffectId, HandlerResult, RelayVar};
use dotzuki_engine::battle::BattlerRef;

use crate::bindings::RuleBindings;
use crate::model::{AmountSpec, DamageValue, FractionOf, Op, Predicate, Selector};
use crate::registry::{CompiledHook, RulesProvider};
use crate::trace;

/// **THE bridge** (doc 11 §2): the single zero-capture `fn` every data hook's
/// `call` points at. Keyed entirely off `source_effect` — the engine already
/// threads it to every handler (`dispatch.rs:128`). Looks the compiled hook up in
/// the game's `&'static` [`RulesHost`](crate::registry::RulesHost), applies the
/// `chance` gate (the only RNG), then folds the op-list.
///
/// Signature matches [`HandlerFn<P>`](dotzuki_engine::battle::stack::HandlerFn)
/// exactly so it is a valid `call` field.
pub fn interpret<P: RulesProvider>(
    ctx: &mut BattleCtx<'_, P>,
    relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    source_effect: EffectId,
) -> HandlerResult {
    let Some(host) = P::rules_host() else {
        // No registry installed ⇒ inert (relay passes through), identical to a
        // game with no data hooks. (Defensive; a wired game always installs one.)
        return HandlerResult::Unchanged;
    };
    let Some(hook) = host.hook(source_effect) else {
        return HandlerResult::Unchanged;
    };

    // The chance gate — the SOLE entropy (doc 11 §4.1). Drawn UNCONDITIONALLY so
    // draw count/order is a pure function of the op-list, not of the outcome.
    if let Some((num, den)) = hook.chance {
        let pass = ctx.rng.chance(num, den);
        if !pass {
            return HandlerResult::Unchanged;
        }
    }

    run_ops(ctx, relay, target, source, &host.bindings, hook)
}

/// The closed primitive interpreter (doc 11 §1.1). Folds the hook's op-list over
/// `relay`, returning the engine [`HandlerResult`]. Short-circuits on the first
/// `Fail`/`FailSilent` exactly like the native fold (`dispatch.rs:285`); a
/// numeric op produces `Set`; a side-effecting op produces `Unchanged` (the relay
/// is threaded through).
///
/// **Determinism**: this fn touches `ctx.rng` ONLY via the caller's `chance` gate
/// (it is not re-drawn here); every op below is a pure `ctx`/`RelayVar`/binding
/// operation. No entropy, no clock, no draw-order-affecting iteration.
pub fn run_ops<P: RulesProvider>(
    ctx: &mut BattleCtx<'_, P>,
    mut relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    bindings: &P::Bindings,
    hook: &CompiledHook,
) -> HandlerResult {
    let mut result = HandlerResult::Unchanged;
    for op in &hook.ops {
        let before = relay;
        match apply_op(ctx, relay, target, source, bindings, hook, op) {
            OpOutcome::Unchanged => {
                trace::record(hook.id, hook.event, op, before, before);
            }
            OpOutcome::Set(v) => {
                relay = v;
                result = HandlerResult::Set(v);
                trace::record(hook.id, hook.event, op, before, v);
            }
            OpOutcome::Fail => {
                trace::record(hook.id, hook.event, op, before, RelayVar::Bool(false));
                return HandlerResult::Fail;
            }
            OpOutcome::FailSilent => {
                trace::record(hook.id, hook.event, op, before, RelayVar::Unit);
                return HandlerResult::FailSilent;
            }
        }
    }
    result
}

/// The per-op verdict, before it is folded into the running [`HandlerResult`].
enum OpOutcome {
    Unchanged,
    Set(RelayVar),
    Fail,
    FailSilent,
}

/// Apply ONE op. Pure over `ctx` + binding; no entropy.
fn apply_op<P: RulesProvider>(
    ctx: &mut BattleCtx<'_, P>,
    relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    bindings: &P::Bindings,
    hook: &CompiledHook,
    op: &Op,
) -> OpOutcome {
    match op {
        // Damage was precomputed into `ctx.mv.damage` by the driver (the provider
        // isn't in BattleCtx); this is the ModifyDamage subscription marker.
        Op::DealMoveDamage => OpOutcome::Unchanged,

        Op::DamageFraction {
            num,
            den,
            of,
            target: sel,
            unless,
        } => {
            if pred_holds(ctx, bindings, relay, target, source, hook, unless.as_ref()) {
                return OpOutcome::Unchanged; // skipped (e.g. non-Rock chip's `unless: HasType(Rock)`)
            }
            let who = resolve(*sel, target, source);
            let amt = fraction_amount(ctx, who, *of, *num, *den);
            // A damage sink on `who` (Substitute / shield) may swallow the loss;
            // otherwise apply it. Defaulted-false ⇒ unchanged for every existing game.
            if !bindings.redirect_hp_loss(ctx, who, source, amt) {
                ctx.battler_mut(who).take_damage(amt);
            }
            OpOutcome::Unchanged
        }

        Op::HealFraction {
            num,
            den,
            of,
            target: sel,
            unless,
        } => {
            if pred_holds(ctx, bindings, relay, target, source, hook, unless.as_ref()) {
                return OpOutcome::Unchanged;
            }
            let who = resolve(*sel, target, source);
            let amt = fraction_amount(ctx, who, *of, *num, *den);
            ctx.battler_mut(who).heal(amt);
            OpOutcome::Unchanged
        }

        Op::InflictStatus {
            status,
            target: sel,
            amount,
        } => {
            let who = resolve(*sel, target, source);
            // Draw the amount first (unconditionally, at this op's ordinal) so
            // the rng stream is a pure function of the op-list. `Const` (the
            // default) draws nothing, so a plain InflictStatus is unchanged.
            let amt = resolve_amount(ctx, *amount);
            // Resolve the status name to an index via the binding. The loader
            // already validated this name at compile, so the index exists.
            if let Some(idx) = status_index::<P>(status) {
                let b = ctx.battler_mut(who);
                bindings.set_status_with_amount(b, idx, amt);
            }
            OpOutcome::Unchanged
        }

        Op::InflictVolatile {
            kind,
            target: sel,
            amount,
        } => {
            let who = resolve(*sel, target, source);
            let amt = resolve_amount(ctx, *amount);
            // The game builds its OPAQUE volatile kind for (name, amount); the
            // engine installs it generically. Unknown name ⇒ `None` ⇒ inert.
            if let Some(kind) = bindings.make_volatile(kind, amt) {
                ctx.install_effect(who, kind);
            }
            OpOutcome::Unchanged
        }

        Op::Boost {
            stat,
            stages,
            target: sel,
        } => {
            let who = resolve(*sel, target, source);
            if let Some(idx) = host_stat_index::<P>(stat) {
                let b = ctx.battler_mut(who);
                bindings.apply_boost(b, idx, *stages);
            }
            OpOutcome::Unchanged
        }

        Op::ScaleRelay { num, den, when } => {
            if when
                .iter()
                .all(|p| pred_holds(ctx, bindings, relay, target, source, hook, Some(p)))
            {
                OpOutcome::Set(relay.scale(*num, *den))
            } else {
                OpOutcome::Unchanged
            }
        }

        Op::SetRelay(v) => OpOutcome::Set(RelayVar::Int(*v)),
        Op::AddRelay(k) => OpOutcome::Set(RelayVar::Int(relay.as_int() + *k)),
        Op::ClampRelay { lo, hi } => {
            let v = relay.as_int().clamp(*lo, *hi);
            OpOutcome::Set(RelayVar::Int(v))
        }

        Op::VetoIf { cond, silent } => {
            if pred_holds(ctx, bindings, relay, target, source, hook, Some(cond)) {
                if *silent {
                    OpOutcome::FailSilent
                } else {
                    OpOutcome::Fail
                }
            } else {
                OpOutcome::Unchanged
            }
        }

        Op::ApplyTypeChart => {
            let Some(mti) = hook.move_type_index else {
                return OpOutcome::Unchanged; // untyped move ⇒ neutral 1×
            };
            // Fold the dual-type PRODUCT into ONE rational, then a single scale
            // (doc 12 §5.3). The binding owns the chart; pure, no RNG.
            let (num, den) = bindings.type_chart_mult(ctx, mti, target);
            OpOutcome::Set(relay.scale(num, den))
        }

        Op::PayResource {
            resource,
            amount,
            target: sel,
        } => {
            // The MP/SP/mana cost gate expressed in DATA (doc 13 §4). If the payer
            // cannot afford the cost, `Fail` (the move is prevented via the existing
            // veto path); otherwise deduct it. PURE ARITHMETIC — touches no rng.
            let who = resolve(*sel, target, source);
            let Some(idx) = host_resource_index::<P>(resource) else {
                // The loader validated the name at compile, so this is defensive.
                return OpOutcome::Unchanged;
            };
            if !bindings.can_pay_resource(ctx.battler(who), idx, *amount) {
                return OpOutcome::Fail; // insufficient ⇒ prevent the move
            }
            bindings.pay_resource(ctx.battler_mut(who), idx, *amount);
            OpOutcome::Unchanged
        }

        // OHKO (SetHp(Foe, 0, when:[LevelGE])) / Explode (SetHp(Source, 0)). An
        // ABSOLUTE set — not routed through `take_damage` — the faithful Gen-1
        // one-hit-KO / self-detonate. The `when` guard gates the write (ALL hold).
        // Pure write; no entropy.
        Op::SetHp {
            target: sel,
            value,
            when,
        } => {
            if !when
                .iter()
                .all(|p| pred_holds(ctx, bindings, relay, target, source, hook, Some(p)))
            {
                return OpOutcome::Unchanged; // gate failed (e.g. OHKO immune, bug #19)
            }
            let who = resolve(*sel, target, source);
            let cur = ctx.battler(who).hp;
            // Only a genuine LOSS (cur > value) may be routed to a damage sink. A KO
            // (value == 0) breaks the sink UNCONDITIONALLY — the oracle zeroes the
            // Substitute regardless of its HP vs the mon's — so pass a break-guaranteeing
            // amount; a partial set (unused in Gen-1) passes its real loss.
            if cur > *value {
                let sink_amount = if *value == 0 { u16::MAX } else { cur - *value };
                if bindings.redirect_hp_loss(ctx, who, source, sink_amount) {
                    return OpOutcome::Unchanged; // the sink took it; skip the absolute set
                }
            }
            ctx.battler_mut(who).hp = *value;
            OpOutcome::Unchanged
        }

        // Special/fixed damage that BYPASSES the type chart: write `ctx.mv.damage`
        // directly so it rides the SAME driver apply path as `DealMoveDamage`. The
        // level-based variants read the user's level via the binding; `RngScaledLevel`
        // draws exactly ONE `ctx.rng` byte (Psywave) — the SOLE entropy here.
        Op::SetDamage { value, of } => {
            let who = resolve(*of, target, source);
            let dmg = match *value {
                DamageValue::Const(c) => c,
                DamageValue::UserLevel => bindings.battler_level(ctx.battler(who)),
                DamageValue::RngScaledLevel { num, den } => {
                    let byte = ctx.rng.next_u8() as u32;
                    let level = bindings.battler_level(ctx.battler(who)) as u32;
                    let den = den.max(1);
                    let raw = (byte * num / den * level) / 256;
                    raw.min(u16::MAX as u32) as u16
                }
            };
            ctx.mv.damage = dmg;
            OpOutcome::Unchanged
        }

        // Super Fang: damage the selector by a fraction of its CURRENT HP, floored
        // at 1 for a non-zero base (the legacy `(curHP/2).max(1)`), AND write
        // `ctx.mv.damage` so a redirect / Counter read sees the real number. Pure.
        Op::DamageCurrentHpFraction {
            num,
            den,
            target: sel,
        } => {
            let who = resolve(*sel, target, source);
            let cur = ctx.battler(who).hp as u64;
            let den = (*den).max(1) as u64;
            let amt = ((cur * *num as u64) / den).min(u16::MAX as u64) as u16;
            let amt = if cur > 0 { amt.max(1) } else { 0 };
            // Super Fang deals a fraction of the MON's current HP (the oracle reads the
            // mon, not the doll), but a Substitute swallows the resulting number.
            if !bindings.redirect_hp_loss(ctx, who, source, amt) {
                ctx.battler_mut(who).take_damage(amt);
            }
            ctx.mv.damage = amt; // keep the real number for a Counter / informational read
            OpOutcome::Unchanged
        }

        // The Gen-1 multi-hit loop, GAME-SIDE (no engine seam). On `DamagingHit`
        // the driver has already dealt the FIRST hit (`take_damage(ctx.mv.damage)`),
        // so re-apply the SAME per-hit number `(N-1)` more times. N is drawn from
        // `count`; `TwoToFive` consumes ONE byte via `determine_hit_count` (the
        // legacy `multi_hit_roll`). The final-hit rider (Twineedle poison) draws its
        // `chance` byte UNCONDITIONALLY after the last hit (the consumed() invariant),
        // then runs its guard/inflict ops if the gate passes. Only `ctx.rng` is
        // touched (count byte + optional final-hit byte), at this op's ordinal.
        Op::RepeatHits {
            count,
            target: sel,
            final_hit,
        } => {
            let who = resolve(*sel, target, source);
            let per_hit = ctx.mv.damage;
            let n = match count {
                crate::model::HitCount::Fixed(k) => *k,
                crate::model::HitCount::TwoToFive => determine_hit_count(ctx.rng.next_u8()),
            };
            // The driver already dealt hit #1 (through the Event::Damage fold, so a
            // Substitute absorbed it); deal the remaining (N-1), each likewise routed
            // to the sink until it breaks, then to the mon.
            for _ in 1..n {
                if !bindings.redirect_hp_loss(ctx, who, source, per_hit) {
                    ctx.battler_mut(who).take_damage(per_hit);
                }
            }
            // Final-hit-only secondary (Twineedle 52/256 poison). The chance byte is
            // drawn UNCONDITIONALLY (consumed() invariant), then — if it passes — the
            // rider ops (VetoIf guards + InflictStatus) run exactly like a side-status
            // hook. Resolved against the same `target` (the defender).
            if let crate::model::FinalHitRider::OnFinal { chance, ops } = final_hit {
                let pass = ctx.rng.chance(chance.num, chance.den);
                if pass {
                    for op in ops {
                        match apply_op(ctx, relay, target, source, bindings, hook, op) {
                            // A VetoIf that fires aborts the rider (poison-type /
                            // Substitute immunity), exactly like the side-status
                            // VetoIf short-circuit — but it does NOT fail the whole
                            // multi-hit (the hits already landed). Stop running the
                            // rider; the op itself is otherwise side-effecting.
                            OpOutcome::Fail | OpOutcome::FailSilent => break,
                            OpOutcome::Unchanged | OpOutcome::Set(_) => {}
                        }
                    }
                }
            }
            OpOutcome::Unchanged
        }

        // The cleanse: clear the selector's non-volatile status (the wuxia 驱散
        // op). A generic engine-field write — `.status = None` — mirroring how
        // `SetHp` writes `.hp = value`; no binding, no entropy. The inverse of
        // `InflictStatus`.
        Op::RemoveStatus { target: sel } => {
            let who = resolve(*sel, target, source);
            ctx.battler_mut(who).status = None;
            OpOutcome::Unchanged
        }
    }
}

/// Gen-1 two-to-five hit distribution (`3/8` each for 2/3, `1/8` each for 4/5),
/// bit-identical to the legacy pokered `determine_hit_count` (multi_hit_effects.rs):
/// `roll<96⇒2, <192⇒3, <224⇒4, else⇒5`. Pure; the SOLE caller draws the byte once.
fn determine_hit_count(roll: u8) -> u8 {
    if roll < 96 {
        2
    } else if roll < 192 {
        3
    } else if roll < 224 {
        4
    } else {
        5
    }
}

/// Resolve an [`AmountSpec`] to a number, drawing from the `ctx.rng` byte stream
/// (at the op's ordinal — the sole entropy). `Const` draws nothing, so a
/// duration-less op preserves the pre-existing rng stream. `RngMask` draws
/// exactly ONE byte; `RngRange` REJECTION-samples (redrawing the skewed tail) so
/// the span is uniform — see below.
fn resolve_amount<P: RulesProvider>(ctx: &mut BattleCtx<'_, P>, spec: AmountSpec) -> u16 {
    match spec {
        AmountSpec::Const(c) => c,
        AmountSpec::RngMask { mask, plus } => {
            let byte = ctx.rng.next_u8();
            (byte & mask) as u16 + plus as u16
        }
        AmountSpec::RngRange { lo, hi } => {
            let span = hi.saturating_sub(lo).saturating_add(1).max(1);
            if span >= 256 {
                // One byte can't cover the span; degrade to a single modulo draw.
                let byte = ctx.rng.next_u8() as u16;
                return lo + (byte % span);
            }
            // Rejection sampling (Gen-1 sleep counter style: the asm re-rolls a
            // 0 rather than skewing low values): accept only bytes below the
            // largest multiple of `span` that fits in a byte, so `byte % span`
            // is uniform. BOUNDED like the production damage-roll rejection so a
            // degenerate scripted rng stream can never spin forever.
            let limit = 256 - (256 % span);
            let mut byte = ctx.rng.next_u8() as u16;
            let mut tries = 0;
            while byte >= limit && tries < 64 {
                byte = ctx.rng.next_u8() as u16;
                tries += 1;
            }
            lo + (byte % span)
        }
    }
}

/// Resolve a [`Selector`] to a concrete [`BattlerRef`] (doc 11 §1.1).
fn resolve(sel: Selector, target: BattlerRef, source: BattlerRef) -> BattlerRef {
    match sel {
        Selector::Target | Selector::Host => target,
        Selector::Source => source,
        Selector::Foe => BattlerRef::new(if target.side == 0 { 1 } else { 0 }, target.slot),
    }
}

/// `of` base × num / den, integer-truncated (mirrors `RelayVar::scale`). Div-by-0
/// clamps den to 1 (doc 11 §4.2).
///
/// [`FractionOf::LastDamage`] bases off `ctx.mv.last_damage` (the damage the
/// in-flight move just dealt) and **floors the non-zero result at 1** — the legacy
/// Gen-1 Drain `(dealt/2).max(1)` / Recoil `(dealt/4).max(1)`
/// (`damage_effects.rs`). A 0-damage event yields 0 (no drain / no recoil); any
/// positive dealt yields at least 1. `MaxHp`/`CurHp` keep their plain truncation
/// (Recover has no min). Pure read of `ctx.mv`; no entropy.
fn fraction_amount<P: RulesProvider>(
    ctx: &BattleCtx<'_, P>,
    who: BattlerRef,
    of: FractionOf,
    num: u32,
    den: u32,
) -> u16 {
    let den = den.max(1) as u64;
    match of {
        FractionOf::MaxHp => {
            let base = ctx.battler(who).max_hp as u64;
            ((base * num as u64) / den).min(u16::MAX as u64) as u16
        }
        FractionOf::CurHp => {
            let base = ctx.battler(who).hp as u64;
            ((base * num as u64) / den).min(u16::MAX as u64) as u16
        }
        FractionOf::LastDamage => {
            let base = ctx.mv.last_damage as u64;
            let amt = ((base * num as u64) / den).min(u16::MAX as u64) as u16;
            // Legacy `.max(1)` floor: a non-zero damage event always moves ≥1 HP.
            if base > 0 {
                amt.max(1)
            } else {
                0
            }
        }
    }
}

/// Evaluate a closed [`Predicate`] (doc 11 §1.1). Pure read; no RNG.
///
/// `hook` is threaded so `MoveTypeIsDefenderType` can recover the in-flight move's
/// `move_type_index` (the record's `type:`). The predicates evaluate against the
/// `target` selector (the dispatch target — the defender on `DamagingHit`).
fn pred_holds<P: RulesProvider>(
    ctx: &BattleCtx<'_, P>,
    bindings: &P::Bindings,
    relay: RelayVar,
    target: BattlerRef,
    source: BattlerRef,
    hook: &CompiledHook,
    pred: Option<&Predicate>,
) -> bool {
    let Some(pred) = pred else { return false };
    match pred {
        Predicate::HasType(name) => match host_type_index::<P>(name) {
            Some(idx) => bindings.has_type(ctx.battler(target), idx),
            None => false,
        },
        Predicate::StatIs(name) => {
            match (host_stat_index::<P>(name), bindings.current_stat_index(ctx)) {
                (Some(want), Some(cur)) => want == cur,
                _ => false,
            }
        }
        Predicate::RelayIntLt(n) => relay.as_int() < *n,
        // The Substitute block: does the defender have the named live volatile?
        // The game inspects its own arena (the engine treats EffectStateKind
        // opaquely). Pure read; no entropy.
        Predicate::HasVolatile(name) => bindings.has_volatile(ctx, target, name),
        // The Gen-1 self-type immunity quirk #23: the move's type == one of the
        // defender's types. The move type is the compiled hook's move_type_index;
        // an untyped record (no `type:`) ⇒ never matches.
        Predicate::MoveTypeIsDefenderType => match hook.move_type_index {
            Some(mti) => bindings.move_type_is_defender_type(ctx, mti, target),
            None => false,
        },
        // The Dream Eater sleep gate: the defender currently has the named status.
        Predicate::TargetHasStatus(name) => match status_index::<P>(name) {
            Some(idx) => bindings.has_status(ctx.battler(target), idx),
            None => false,
        },
        // Logical negation of any inner predicate (Dream Eater: NOT asleep).
        Predicate::Not(inner) => {
            !pred_holds(ctx, bindings, relay, target, source, hook, Some(inner))
        }
        // The Toxic guard: the defender already has any non-volatile status.
        Predicate::TargetHasAnyStatus => bindings.has_any_status(ctx.battler(target)),
        // The OHKO gate (bug #19): the SOURCE (user) level ≥ the TARGET (foe)
        // level. The level is the game's per-battler quantity (the binding answers
        // it; the engine carries no level). Pure read.
        Predicate::LevelGE => {
            bindings.battler_level(ctx.battler(source))
                >= bindings.battler_level(ctx.battler(target))
        }
        // The wuxia 「血越低攻越高」 gate: the SOURCE (the acting battler) HP fraction
        // strictly below num/den. `hp`/`max_hp` are engine fields, read directly off
        // ctx (no binding). Pure read; no entropy.
        Predicate::SelfHpBelow { num, den } => {
            let b = ctx.battler(source);
            let den = (*den).max(1) as u64;
            (b.hp as u64) * den < (b.max_hp as u64) * (*num as u64)
        }
        // Like `TargetHasStatus` but on the SOURCE (the acting battler) — the wuxia
        // 眩晕/控制 BeforeMove veto gate. Reuses the EXISTING `has_status` binding.
        Predicate::SourceHasStatus(name) => match status_index::<P>(name) {
            Some(idx) => bindings.has_status(ctx.battler(source), idx),
            None => false,
        },
    }
}

// ── interned-name lookups against the installed registry vocabularies ─────────
//
// The compiled registry owns the interned `types`/`stats` lists; status indices
// are the game's vocabulary (resolved by the binding through a game-supplied
// `status_index_of` at compile, and looked up here against the same vocabulary).
// All pure, no RNG.

fn host_type_index<P: RulesProvider>(name: &str) -> Option<usize> {
    P::rules_host().and_then(|h| h.compiled.types.iter().position(|t| t == name))
}

fn host_stat_index<P: RulesProvider>(name: &str) -> Option<usize> {
    P::rules_host().and_then(|h| h.compiled.stats.iter().position(|s| s == name))
}

/// Resource index against the installed registry's interned `resources:` list
/// (the MP/SP/mana cost gate, doc 13 §4). Pure, no RNG.
fn host_resource_index<P: RulesProvider>(name: &str) -> Option<usize> {
    P::rules_host().and_then(|h| h.compiled.resources.iter().position(|r| r == name))
}

/// Status index: the game's status vocabulary is NOT part of the RON `stats:`
/// list (it is the game's own enum). The loader interns each `InflictStatus`
/// status name → the game's status index at compile (via the game-supplied
/// `status_index_of`), storing it in [`CompiledRuleset::statuses`]. The
/// interpreter recovers it here, then hands the index to `bindings.set_status`.
/// Pure, no RNG.
fn status_index<P: RulesProvider>(name: &str) -> Option<usize> {
    P::rules_host().and_then(|h| h.compiled.status_index(name))
}
