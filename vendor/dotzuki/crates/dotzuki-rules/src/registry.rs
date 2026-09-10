//! The side registry (doc 11 §2): compile a [`Ruleset`] into runtime
//! [`Effect`](dotzuki_engine::battle::stack::Effect)s + a map from the synthesized
//! [`EffectId`] to the compiled op-list the [`interpret`](crate::interpret)
//! bridge reads.
//!
//! ## Option A — zero engine change (doc 11 §2.2)
//!
//! The loader mints one distinct [`EffectId`] per `(effect, event)` hook and
//! registers each as its own tiny runtime `Effect` **through the existing
//! defaulted resolvers** (`effect_for_move` / `_status` / `_ability` / `_item` /
//! `field_effects`). Each such `Effect` has hooks whose `call` is `interpret::<P>`
//! and whose **owning effect id** keys the op-list. The engine never learns "data
//! exists"; it sees an ordinary `Effect` with a `fn`-pointer hook and threads its
//! `source_effect` to the handler (`dispatch.rs:128`) — which is exactly the key
//! the interpreter reads. **No engine edit, no new engine-trait method.**
//!
//! The game reaches the compiled registry through the **game-side**
//! [`RulesProvider`] trait (which *extends* `EffectProvider`); the engine is
//! untouched.

use dotzuki_engine::hash::HashMap;

use dotzuki_engine::battle::stack::{
    Effect, EffectId, EffectProvider, EffectType, Event, EventHook,
};

use crate::bindings::RuleBindings;
use crate::interp::interpret;
use crate::model::{EffectKind, LoadError, Op, Ruleset};

/// Which defaulted resolver hosts a compiled effect (doc 11 §1, §2.2 Option A).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverKind {
    /// `effect_for_move`.
    Move,
    /// `effect_for_status`.
    Status,
    /// `effect_for_ability`.
    Ability,
    /// `effect_for_item`.
    Item,
    /// `field_effects`.
    Weather,
}

impl ResolverKind {
    /// The resolver host for an [`EffectKind`].
    pub fn from_kind(kind: EffectKind) -> Self {
        match kind {
            EffectKind::Move => ResolverKind::Move,
            EffectKind::Status => ResolverKind::Status,
            EffectKind::Ability => ResolverKind::Ability,
            EffectKind::Item => ResolverKind::Item,
            EffectKind::Weather => ResolverKind::Weather,
        }
    }
}

/// One compiled hook program (doc 11 §2): the closed op-list + its ordering +
/// the optional RNG gate. The driver / interpreter reads this by `EffectId`.
#[derive(Debug, Clone)]
pub struct CompiledHook {
    /// The synthesized id keying this hook (the engine threads it as
    /// `source_effect`).
    pub id: EffectId,
    /// Which closed event this hook fires on.
    pub event: Event,
    /// `on<Event>Order`; LOW first.
    pub order: u32,
    /// `on<Event>Priority`; HIGH first.
    pub priority: i32,
    /// Optional `(num, den)` RNG gate — drawn unconditionally so draw order is a
    /// pure function of the op-list (doc 11 §4.1).
    pub chance: Option<(u32, u32)>,
    /// The interned, bound op-list (chart/stat/status names already validated to
    /// indices at compile).
    pub ops: Vec<Op>,
    /// The in-flight move type chart index for this effect (for `ApplyTypeChart`,
    /// recovered from `source_effect`), if the record carried a `type:`.
    pub move_type_index: Option<usize>,
    /// The engine effect category (sub_order feed).
    pub effect_type: EffectType,
    /// The original effect id string (for tracing/diagnostics).
    pub source_id: String,
    /// Which resolver hosts the owning effect.
    pub resolver: ResolverKind,
}

/// A compiled ruleset: the synthesized-id → [`CompiledHook`] map plus the
/// interned `types`/`stats` vocabularies (doc 11 §2 side registry). Built once at
/// load, addressed by `EffectId`, so a hot-reload swaps the map between turns
/// without invalidating in-flight engine `EffectState` (doc 11 §4.2).
#[derive(Debug, Clone)]
pub struct CompiledRuleset {
    /// `EffectId` → compiled hook (the interpreter's lookup; doc 11 §2).
    pub hooks: HashMap<EffectId, CompiledHook>,
    /// Interned type names (index = chart index).
    pub types: Vec<String>,
    /// Interned stat names (index = stat index).
    pub stats: Vec<String>,
    /// Interned resource names (index = resource index; the MP/SP/mana cost gate,
    /// doc 13 §4). The game binding maps each ↔ the engine's opaque resource id.
    pub resources: Vec<String>,
    /// Per-move resource cost: the move record's `source_id` → the list of
    /// `(resource_index, amount)` it costs (doc 13 §4). The game reads this to
    /// implement [`BattleProvider::move_cost`](dotzuki_engine::battle::BattleProvider::move_cost)
    /// so the engine's cost gate fires. Empty for a move with no `cost:`.
    pub move_costs: HashMap<String, Vec<(usize, u16)>>,
    /// Status-name → the GAME's status index (resolved at compile via the
    /// game-supplied `status_index_of`). The RON `stats:` list does NOT carry
    /// statuses; this map is the closed status vocabulary the interpreter reads.
    pub statuses: HashMap<String, usize>,
    /// The interned type chart (doc 12 §2): `(atk_type_index, def_type_index)` →
    /// `(num, den)`, keyed by the ruleset's `types:` intern indices. Omitted pairs
    /// default to `(1, 1)` at lookup via [`chart_mult`](CompiledRuleset::chart_mult).
    /// The DATA layer OWNS the chart here — a hot-reload of the RON `type_chart`
    /// rebuilds this map and the binding reads it, so the relation is genuinely
    /// data-driven (not a hardcoded native const). Built once at compile; an
    /// unknown type name in an edge is a [`LoadError`](crate::LoadError::UnknownType).
    pub type_chart: HashMap<(usize, usize), (u32, u32)>,
}

impl CompiledRuleset {
    /// The game status index for a status name (the interpreter's `InflictStatus`
    /// lookup), validated at compile.
    pub fn status_index(&self, name: &str) -> Option<usize> {
        self.statuses.get(name).copied()
    }

    /// The interned chart multiplier `(num, den)` for an attacker-type index vs a
    /// defender-type index (the indices are the ruleset's `types:` positions).
    /// Omitted pairs default to `(1, 1)` (neutral) — doc 12 §2. Pure; no RNG. The
    /// game binding folds the product over a defender's type(s) and applies ONE
    /// `scale` (doc 12 §5.3), so the data path owns the relation end to end.
    pub fn chart_mult(&self, atk_type_index: usize, def_type_index: usize) -> (u32, u32) {
        self.type_chart
            .get(&(atk_type_index, def_type_index))
            .copied()
            .unwrap_or((1, 1))
    }
}

impl CompiledRuleset {
    /// Compile a [`Ruleset`], minting one [`EffectId`] per hook starting at
    /// `id_base` (doc 11 §2.2 Option A). **All closed-vocabulary binding happens
    /// here**: every `on:` event, every `HasType`/`StatIs`/chart type name, every
    /// `chance` fraction is validated NOW — an unknown name/op is a [`LoadError`]
    /// at LOAD, never at battle time (doc 11 §4.2).
    ///
    /// `bindings` resolves status/stat names to indices (defense-in-depth: the
    /// loader also pre-validates names against the ruleset's `stats:` list, but
    /// status names are the game's vocabulary, so the binding is the authority).
    pub fn compile<P, B>(
        ruleset: &Ruleset,
        id_base: u32,
        bindings: &B,
        status_index_of: impl Fn(&str) -> Option<usize>,
    ) -> Result<Self, LoadError>
    where
        P: EffectProvider,
        B: RuleBindings<P>,
    {
        let _ = bindings; // bindings authority is exercised via status_index_of/stat list
        let mut hooks = HashMap::default();
        let mut statuses: HashMap<String, usize> = HashMap::default();
        let mut move_costs: HashMap<String, Vec<(usize, u16)>> = HashMap::default();
        let mut next_id = id_base;

        // Intern the type chart NOW (doc 12 §2): every edge's atk/def names must
        // be in `types:` — an unknown name is a LOAD error, never a battle-time
        // surprise. The DATA layer owns the chart; a hot-reload rebuilds this map.
        let mut type_chart: HashMap<(usize, usize), (u32, u32)> = HashMap::default();
        for edge in &ruleset.type_chart {
            let a = ruleset
                .type_index(&edge.atk)
                .ok_or_else(|| LoadError::UnknownType(edge.atk.clone()))?;
            let d = ruleset
                .type_index(&edge.def)
                .ok_or_else(|| LoadError::UnknownType(edge.def.clone()))?;
            type_chart.insert((a, d), (edge.mult.num, edge.mult.den));
        }

        for rec in &ruleset.effects {
            let effect_type = crate::model::parse_kind(rec.kind);
            let resolver = ResolverKind::from_kind(rec.kind);
            let move_type_index = match &rec.mtype {
                Some(name) => Some(
                    ruleset
                        .type_index(name)
                        .ok_or_else(|| LoadError::UnknownType(name.clone()))?,
                ),
                None => None,
            };

            // Intern the move's resource cost NOW (doc 13 §4): each `cost:` entry's
            // resource name must be in `resources:` — an unknown name is a LOAD
            // error, never a battle-time surprise. Stored by the record's id so the
            // game's `move_cost` hook can read it.
            if !rec.cost.is_empty() {
                let mut costs = Vec::with_capacity(rec.cost.len());
                for c in &rec.cost {
                    let idx = ruleset
                        .resource_index(&c.resource)
                        .ok_or_else(|| LoadError::UnknownResource(c.resource.clone()))?;
                    costs.push((idx, c.amount));
                }
                move_costs.insert(rec.id.clone(), costs);
            }

            for hook in &rec.hooks {
                // Parse the event to the closed enum NOW (unknown ⇒ load error).
                let event = crate::model::parse_event(&hook.on)?;

                // Validate the chance fraction NOW.
                let chance = match hook.chance {
                    Some(r) => {
                        if r.den == 0 {
                            return Err(LoadError::BadChance(r.num, r.den));
                        }
                        Some((r.num, r.den))
                    }
                    None => None,
                };

                // Validate every name referenced by an op NOW, and intern any
                // status names into the closed status vocabulary — both
                // `InflictStatus` ops AND `TargetHasStatus` predicates (the
                // interpreter resolves both through the same `statuses` map).
                for op in &hook.ops {
                    validate_op::<P, B>(op, ruleset, &status_index_of)?;
                    if let Op::InflictStatus { status, .. } = op {
                        let idx = status_index_of(status)
                            .ok_or_else(|| LoadError::UnknownStatus(status.clone()))?;
                        statuses.insert(status.clone(), idx);
                    }
                    for pred in op_predicates(op) {
                        // Both `TargetHasStatus` and `SourceHasStatus` name the game's
                        // status vocabulary; intern each into the closed status map so
                        // the interpreter's `status_index` lookup resolves at runtime.
                        let status_name = match pred {
                            crate::model::Predicate::TargetHasStatus(s)
                            | crate::model::Predicate::SourceHasStatus(s) => Some(s),
                            _ => None,
                        };
                        if let Some(s) = status_name {
                            let idx = status_index_of(s)
                                .ok_or_else(|| LoadError::UnknownStatus(s.clone()))?;
                            statuses.insert(s.clone(), idx);
                        }
                    }
                }

                let id = EffectId(next_id);
                next_id += 1;
                hooks.insert(
                    id,
                    CompiledHook {
                        id,
                        event,
                        order: hook.order,
                        priority: hook.priority,
                        chance,
                        ops: hook.ops.clone(),
                        move_type_index,
                        effect_type,
                        source_id: rec.id.clone(),
                        resolver,
                    },
                );
            }
        }

        Ok(CompiledRuleset {
            hooks,
            types: ruleset.types.clone(),
            stats: ruleset.stats.clone(),
            resources: ruleset.resources.clone(),
            move_costs,
            statuses,
            type_chart,
        })
    }

    /// The compiled hook for an `EffectId` (the interpreter's lookup).
    pub fn hook(&self, id: EffectId) -> Option<&CompiledHook> {
        self.hooks.get(&id)
    }

    /// The resource cost of the move with record id `source_id`, as interned
    /// `(resource_index, amount)` pairs (doc 13 §4). Empty for a move with no
    /// `cost:`. The game maps each `resource_index` to its engine resource id (via
    /// the binding) to build [`BattleProvider::move_cost`](dotzuki_engine::battle::BattleProvider::move_cost).
    pub fn move_cost(&self, source_id: &str) -> &[(usize, u16)] {
        self.move_costs
            .get(source_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Build the `&'static`-shaped per-hook [`Effect`] registry that the game's
    /// defaulted resolvers hand back to the engine. Because the engine requires
    /// `&'static [EventHook]`, the caller leaks these once at load (a deliberate
    /// one-time leak — the registry lives for the whole battle; doc 11 §4.2 notes
    /// a reload swaps the map, not in-flight state). Each `Effect` has ONE hook
    /// whose `call` is `interpret::<P>` and whose id keys the op-list.
    pub fn build_effects<P>(&self) -> Vec<&'static Effect<P>>
    where
        P: RulesProvider,
    {
        let mut out = Vec::with_capacity(self.hooks.len());
        for h in self.hooks.values() {
            let hook: EventHook<P> = EventHook {
                event: h.event,
                call: interpret::<P>,
                order: h.order,
                priority: h.priority,
                sub_order: None,
            };
            let leaked_hooks: &'static [EventHook<P>] = Box::leak(vec![hook].into_boxed_slice());
            let eff: &'static Effect<P> = Box::leak(Box::new(Effect {
                id: h.id,
                kind: h.effect_type,
                hooks: leaked_hooks,
            }));
            out.push(eff);
        }
        out
    }
}

/// Collect every [`Predicate`](crate::model::Predicate) an op carries (its
/// `unless`/`when`/`cond` guards), so the compiler can intern any status names a
/// `TargetHasStatus` predicate references. Pure; allocation-light.
fn op_predicates(op: &Op) -> Vec<&crate::model::Predicate> {
    match op {
        Op::DamageFraction { unless, .. } | Op::HealFraction { unless, .. } => {
            unless.iter().collect()
        }
        Op::ScaleRelay { when, .. } | Op::SetHp { when, .. } => when.iter().collect(),
        Op::VetoIf { cond, .. } => vec![cond],
        // RepeatHits' final-hit rider carries its own ops (VetoIf guards +
        // InflictStatus); recurse so a `TargetHasStatus`/`HasType` guard's name is
        // interned and an `InflictStatus` status is validated (done in validate_op).
        Op::RepeatHits {
            final_hit: crate::model::FinalHitRider::OnFinal { ops, .. },
            ..
        } => ops.iter().flat_map(op_predicates).collect(),
        _ => Vec::new(),
    }
}

/// Validate every name an op references against the closed vocabulary, NOW (load
/// time). Returns the first [`LoadError`].
fn validate_op<P, B>(
    op: &Op,
    ruleset: &Ruleset,
    status_index_of: &impl Fn(&str) -> Option<usize>,
) -> Result<(), LoadError>
where
    P: EffectProvider,
    B: RuleBindings<P>,
{
    use crate::model::Predicate;
    let check_type = |name: &str| -> Result<(), LoadError> {
        ruleset
            .type_index(name)
            .map(|_| ())
            .ok_or_else(|| LoadError::UnknownType(name.to_string()))
    };
    let check_stat = |name: &str| -> Result<(), LoadError> {
        ruleset
            .stat_index(name)
            .map(|_| ())
            .ok_or_else(|| LoadError::UnknownStat(name.to_string()))
    };
    let check_resource = |name: &str| -> Result<(), LoadError> {
        ruleset
            .resource_index(name)
            .map(|_| ())
            .ok_or_else(|| LoadError::UnknownResource(name.to_string()))
    };
    let check_status = |name: &str| -> Result<(), LoadError> {
        status_index_of(name)
            .map(|_| ())
            .ok_or_else(|| LoadError::UnknownStatus(name.to_string()))
    };
    let check_pred = |p: &Predicate| -> Result<(), LoadError> {
        match p {
            Predicate::HasType(t) => check_type(t),
            Predicate::StatIs(s) => check_stat(s),
            Predicate::RelayIntLt(_) => Ok(()),
            // `HasVolatile`'s name is the game's volatile vocabulary (resolved by
            // the binding's `has_volatile` against the live arena), not the closed
            // `types`/`stats`/`resources` lists — so it is NOT load-validated here
            // (the binding owns it). `MoveTypeIsDefenderType` references no name.
            Predicate::HasVolatile(_) | Predicate::MoveTypeIsDefenderType => Ok(()),
            // `TargetHasStatus`'s name IS the game's status vocabulary — validate it
            // NOW against `status_index_of`, exactly like `InflictStatus`.
            Predicate::TargetHasStatus(s) => check_status(s),
            // `LevelGE` references no name (the binding supplies both levels).
            Predicate::LevelGE => Ok(()),
            // `SelfHpBelow` references no name (it reads engine HP fields).
            Predicate::SelfHpBelow { .. } => Ok(()),
            // `SourceHasStatus`'s name IS the game's status vocabulary — validate it
            // NOW, exactly like `TargetHasStatus`.
            Predicate::SourceHasStatus(s) => check_status(s),
            // `Not` validates its inner's name (one level; nested Not isn't
            // authored). Name-free / binding-resolved inners need no validation.
            Predicate::Not(inner) => match inner.as_ref() {
                Predicate::HasType(t) => check_type(t),
                Predicate::StatIs(s) => check_stat(s),
                Predicate::TargetHasStatus(s) | Predicate::SourceHasStatus(s) => check_status(s),
                _ => Ok(()),
            },
            // `TargetHasAnyStatus` references no name (the binding answers it).
            Predicate::TargetHasAnyStatus => Ok(()),
        }
    };
    match op {
        Op::DealMoveDamage | Op::ApplyTypeChart | Op::SetRelay(_) | Op::AddRelay(_) => Ok(()),
        Op::ClampRelay { .. } => Ok(()),
        Op::DamageFraction { unless, .. } | Op::HealFraction { unless, .. } => {
            if let Some(p) = unless {
                check_pred(p)?;
            }
            Ok(())
        }
        Op::InflictStatus { status, .. } => status_index_of(status)
            .map(|_| ())
            .ok_or_else(|| LoadError::UnknownStatus(status.clone())),
        // The volatile `kind` is the game's RUNTIME vocabulary (resolved by the
        // binding's `make_volatile`, exactly like `HasVolatile` is resolved by
        // `has_volatile`) — not an interned name, so nothing to validate at load.
        Op::InflictVolatile { .. } => Ok(()),
        Op::Boost { stat, .. } => check_stat(stat),
        Op::ScaleRelay { when, .. } => {
            for p in when {
                check_pred(p)?;
            }
            Ok(())
        }
        Op::VetoIf { cond, .. } => check_pred(cond),
        Op::PayResource { resource, .. } => check_resource(resource),
        // SetHp's `when` guards may carry predicates (OHKO's `LevelGE`).
        Op::SetHp { when, .. } => {
            for p in when {
                check_pred(p)?;
            }
            Ok(())
        }
        // SetDamage / DamageCurrentHpFraction / RemoveStatus reference no
        // closed-vocabulary name (the level reach is the binding's `battler_level`,
        // validated by type; RemoveStatus clears whatever status is held by name).
        Op::SetDamage { .. } | Op::DamageCurrentHpFraction { .. } | Op::RemoveStatus { .. } => {
            Ok(())
        }
        // RepeatHits: the count source is a plain number (no name). Validate the
        // final-hit rider's nested ops (its InflictStatus status + guard names) NOW,
        // recursively, so an unknown name in Twineedle's poison rider is a LOAD error.
        Op::RepeatHits { final_hit, .. } => {
            if let crate::model::FinalHitRider::OnFinal { ops, .. } = final_hit {
                for op in ops {
                    validate_op::<P, B>(op, ruleset, status_index_of)?;
                }
            }
            Ok(())
        }
    }
}

/// The **game-side** bridge trait (doc 11 §2.2 Option A) — extends
/// [`EffectProvider`] with the two things the zero-capture
/// [`interpret`](crate::interpret) `fn` needs but cannot capture: the compiled
/// op-list registry and the game binding. **This adds NO method to any engine
/// trait** — it is an additive game-side super-trait, so the engine is untouched.
pub trait RulesProvider: EffectProvider {
    /// The game binding resolving interned indices ↔ concrete `P::Stat`/`P::Status`
    /// and supplying the chart fold.
    type Bindings: RuleBindings<Self>;

    /// The compiled ruleset (the side registry the interpreter reads by
    /// `source_effect`). Typically a `&'static` built once via `OnceLock`.
    fn compiled(&self) -> &CompiledRuleset;

    /// The game binding instance.
    fn bindings(&self) -> &Self::Bindings;

    /// Read the provider from `ctx` for the interpreter. The interpreter only has
    /// `&mut BattleCtx`, which does NOT carry `&P` (the engine's borrow
    /// discipline). A game whose chart/registry is a `&'static` (the common case)
    /// returns it here without touching `ctx`; the default points at that static.
    /// Returns `None` if no static is installed.
    fn rules_host() -> Option<&'static RulesHost<Self>>
    where
        Self: Sized;
}

/// A `&'static` bundle of the compiled registry + binding the zero-capture
/// [`interpret`](crate::interpret) `fn` reaches without capturing (doc 11 §2.2).
/// A game installs one once (e.g. in a `OnceLock`) and returns it from
/// [`RulesProvider::rules_host`].
pub struct RulesHost<P: RulesProvider> {
    /// The compiled side registry.
    pub compiled: CompiledRuleset,
    /// The game binding.
    pub bindings: P::Bindings,
}

impl<P: RulesProvider> RulesHost<P> {
    /// Bundle a compiled registry + binding.
    pub fn new(compiled: CompiledRuleset, bindings: P::Bindings) -> Self {
        Self { compiled, bindings }
    }

    /// The compiled hook for an `EffectId`.
    pub fn hook(&self, id: EffectId) -> Option<&CompiledHook> {
        self.compiled.hook(id)
    }
}
