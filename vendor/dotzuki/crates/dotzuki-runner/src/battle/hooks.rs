//! RON effect hooks (battle v2-a + v2-e): skill, status, ability, held-item
//! and weather effects authored in the project's `rules.ron` (the dotzuki-rules
//! closed `Op`/`Predicate` vocabulary) and executed through the engine's
//! effect-stack interpreter, instead of the runner's hardcoded skill
//! categories. This is the "no-Rust game features" milestone for battles: a
//! new status, skill effect, ability, held item or weather is pure data.
//!
//! The shape mirrors the proven minimon/wuxia harnesses
//! (`examples/minimon/src/data.rs`, `examples/wuxia/.../data.rs`):
//!
//! * [`GenericProvider`] / [`GenericBindings`] — the game side of the stack:
//!   dynamic id types ([`StatId`]/[`StatusId`]/[`TypeId`], interned from the
//!   RON vocabularies) and the pure name↔index bindings (boosts, statuses,
//!   the type chart, volatiles, levels, the MP resource pool).
//! * A **thread-local** [`RulesHost`] (re-installed per battle — the parallel
//!   test harness stays isolated) the zero-capture `interpret` bridge reads.
//! * [`HookState`] — the per-battle mirror: both combatants as engine
//!   [`BattlerState`]s (1v1: side 0 = player, side 1 = enemy), the effect
//!   arena, and the per-action scratch. The runner's own turn loop (v1,
//!   unchanged) fires the event sequence per action through
//!   `collect_handlers` + `run_event`; it does **not** use `StackDriver`.
//!
//! Naming conventions (documented in `docs/reference/project-manifest.md`):
//!
//! * RON `stats` names map onto the manifest `battle.stats` KEYS
//!   (`"hp"|"attack"|"defense"|"speed"`; the usual aliases `atk`/`def`/`spd`
//!   also resolve), so `Boost { stat: "attack" }` needs no per-game code.
//! * RON `resources` names = the manifest `battle.resource` field name (e.g.
//!   `"mp"`); the FIRST declared resource is mirrored onto the combatant's MP
//!   pool (engine resource id 0), so `cost:` / `PayResource` flow through the
//!   MP gate.
//! * RON `types` names = the `element` strings on records (as the chart
//!   already required), matched case-insensitively.
//! * The closed status vocabulary = the ids of the ruleset's `kind: Status`
//!   records, in declaration order (a `StatusId(u16)` interned per record).

use std::cell::RefCell;
use std::collections::HashMap;

use dotzuki_engine::battle::rng::BattleRng as EngineRng;
use dotzuki_engine::battle::stack::{
    BattleCtx, Effect, EffectProvider, EffectState, Event, MoveContext,
};
use dotzuki_engine::battle::{
    BattleProvider, BattleState, BattlerRef, BattlerState, DamageResult, EffectResult, EnumMap,
    MoveEffect,
};
use dotzuki_rules::{
    CompiledRuleset, EffectKind, LoadError, RuleBindings, RulesHost, RulesProvider, Ruleset,
};

use super::{basic_attack, normalize_stat_key, stage_multiplier, Combatant, Skill, MAX_STAGE};

/// The `EffectId` base for the synthesized data hooks (well clear of the
/// arena-allocated volatile ids, which count up from 1; minimon/wuxia use the
/// same base).
pub const DATA_ID_BASE: u32 = 0x10_000;

// ── dynamic id types ────────────────────────────────────────────────────────

/// A stat interned from the ruleset's `stats:` list (index = list position).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatId(pub u16);

/// A status interned from the ruleset's `kind: Status` records (index =
/// declaration order among those records).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusId(pub u16);

/// A type interned from the ruleset's `types:` list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeId(pub u16);

/// A game-opaque volatile (`InflictVolatile { kind, amount }`): the name is
/// the RON vocabulary, the engine never interprets it.
#[derive(Debug, Clone)]
pub struct VolatileKind {
    /// The volatile's RON name.
    pub name: String,
    /// The resolved numeric parameter (turns / counter seed).
    pub amount: u16,
}

/// The provider's species payload: just the defending element (type-chart and
/// `HasType` lookups). Everything else lives on the engine `BattlerState`.
#[derive(Debug, Clone, Default)]
pub struct SpeciesData {
    /// The combatant's `element` field (defending side of chart lookups).
    pub element: Option<String>,
}

// ── the provider ────────────────────────────────────────────────────────────

/// The generic battle provider: dynamic ids + the runner's standard formula.
/// The stack never drives a turn loop here (the runner's own loop fires the
/// events), so `select_move`/`apply_move_effect`/`create_monster` are inert
/// stubs; `calculate_damage` mirrors the runner formula for completeness.
#[derive(Debug, Default, Clone, Copy)]
pub struct GenericProvider;

/// One battler's effective stat (raw × stage multiplier) by canonical stat key
/// (`"attack"` etc.), resolved through the installed registry's stat names.
fn eff_stat(b: &BattlerState<GenericProvider>, key: &str) -> u32 {
    let Some(host) = GenericProvider::rules_host() else {
        return 1;
    };
    let Some(idx) = host
        .compiled
        .stats
        .iter()
        .position(|s| normalize_stat_key(s) == key)
    else {
        return 1;
    };
    let id = StatId(idx as u16);
    let raw = u32::from(b.stats.get(id).copied().unwrap_or(1));
    let stage = b.stat_stages.get(id).copied().unwrap_or(0);
    stage_multiplier(raw, stage)
}

impl BattleProvider for GenericProvider {
    type Monster = ();
    type Move = Skill;
    type Ability = ();
    type Status = StatusId;
    type Stat = StatId;
    type Species = SpeciesData;
    type Type = TypeId;
    type Item = ();

    /// The runner's standard formula with the variance byte and crit flag
    /// given (the stack driver's shape). The runner's own loop precomputes
    /// damage itself (`damage_roll` → `ctx.mv.damage`); this mirrors it.
    fn calculate_damage(
        &self,
        move_: &Skill,
        attacker: &BattlerState<Self>,
        defender: &BattlerState<Self>,
        random: u8,
        is_critical: bool,
    ) -> DamageResult {
        let base = move_.power as u64 * eff_stat(attacker, "attack") as u64
            / eff_stat(defender, "defense").max(1) as u64;
        let varied = base * (85 + u64::from(random % 16)) / 100;
        let after_crit = if is_critical { varied * 3 / 2 } else { varied };
        DamageResult {
            damage: after_crit.max(1).min(u64::from(u16::MAX)) as u16,
            effectiveness: 1.0,
            is_miss: false,
        }
    }

    fn select_move(&self, battler: &BattlerState<Self>, _state: &BattleState<Self>) -> Self::Move {
        battler.moves.first().cloned().unwrap_or_else(basic_attack)
    }

    fn apply_move_effect(
        &self,
        _effect: MoveEffect,
        _user: &mut BattlerState<Self>,
        _target: &mut BattlerState<Self>,
    ) -> EffectResult {
        EffectResult::NoEffect
    }

    fn create_monster(&self, species: Self::Species, level: u8) -> BattlerState<Self> {
        BattlerState::new(species, 1, 1, EnumMap::default(), Vec::new()).with_level(level)
    }
}

impl EffectProvider for GenericProvider {
    type EffectStateKind = VolatileKind;

    /// The runner collects from the compiled registry directly (per-record),
    /// never through the resolvers.
    fn effect_for_move(&self, _m: &Self::Move) -> Option<&'static Effect<Self>> {
        None
    }

    /// See [`effect_for_move`](Self::effect_for_move).
    fn effect_for_status(&self, _s: &Self::Status) -> Option<&'static Effect<Self>> {
        None
    }

    /// `(-priority, -effective speed)`; skills carry no priority (v1), so the
    /// priority tier is 0.
    fn turn_order_rank(
        &self,
        state: &BattleState<Self>,
        who: BattlerRef,
        _action: &Self::Move,
    ) -> (i32, i32) {
        let b = if who.side == 0 {
            &state.player_battlers[who.slot as usize]
        } else {
            &state.opponent_battlers[who.slot as usize]
        };
        (0, -(eff_stat(b, "speed") as i32))
    }
}

// ── the RulesProvider bridge — a thread-local `&'static RulesHost` ──────────
//
// Mirrors minimon/wuxia exactly: install (or re-install) the compiled registry
// per battle by leaking a fresh host; the previous leak is abandoned (a
// bounded, deliberate cost). Thread-local so the parallel test harness stays
// isolated — each test re-installs on its own thread first.

thread_local! {
    static HOST: RefCell<Option<&'static RulesHost<GenericProvider>>> =
        const { RefCell::new(None) };
}

/// Install (or hot-swap) the compiled registry the interpreter reads.
pub fn install_compiled(compiled: CompiledRuleset) {
    let host = RulesHost::new(compiled, GenericBindings);
    let leaked: &'static RulesHost<GenericProvider> = Box::leak(Box::new(host));
    HOST.with(|h| *h.borrow_mut() = Some(leaked));
}

impl RulesProvider for GenericProvider {
    type Bindings = GenericBindings;

    fn compiled(&self) -> &CompiledRuleset {
        &Self::rules_host().expect("rules host installed").compiled
    }
    fn bindings(&self) -> &Self::Bindings {
        &Self::rules_host().expect("rules host installed").bindings
    }
    fn rules_host() -> Option<&'static RulesHost<GenericProvider>> {
        HOST.with(|h| *h.borrow())
    }
}

// ── the bindings ────────────────────────────────────────────────────────────

/// The generic [`RuleBindings`]: resolves interned indices against the
/// installed registry's vocabularies and applies them to the engine
/// `BattlerState`. All methods are pure / RNG-free (the trait contract).
#[derive(Debug, Default, Clone, Copy)]
pub struct GenericBindings;

impl GenericBindings {
    /// The canonical stat key (`"attack"` …) for an interned stat index.
    fn stat_key(stat_index: usize) -> Option<String> {
        let host = GenericProvider::rules_host()?;
        let name = host.compiled.stats.get(stat_index)?;
        Some(normalize_stat_key(name))
    }

    /// The ruleset's type name for an interned chart index.
    fn type_name(type_index: usize) -> Option<String> {
        let host = GenericProvider::rules_host()?;
        host.compiled.types.get(type_index).cloned()
    }
}

impl RuleBindings<GenericProvider> for GenericBindings {
    fn apply_boost(
        &self,
        b: &mut BattlerState<GenericProvider>,
        stat_index: usize,
        stages: i8,
    ) -> bool {
        if Self::stat_key(stat_index).is_none() {
            return false;
        }
        let id = StatId(stat_index as u16);
        let cur = b.stat_stages.get(id).copied().unwrap_or(0);
        b.stat_stages
            .set(id, (cur + stages).clamp(-MAX_STAGE, MAX_STAGE));
        true
    }

    fn set_status(&self, b: &mut BattlerState<GenericProvider>, status_index: usize) -> bool {
        b.status = Some(StatusId(status_index as u16));
        true
    }

    fn has_type(&self, b: &BattlerState<GenericProvider>, type_index: usize) -> bool {
        match (Self::type_name(type_index), &b.species.element) {
            (Some(name), Some(element)) => name.eq_ignore_ascii_case(element),
            _ => false,
        }
    }

    /// The chart fold for the in-flight move type vs the defender's element,
    /// read from the COMPILED RON `type_chart` (the data layer owns the
    /// relation). An untyped defender is neutral.
    fn type_chart_mult(
        &self,
        ctx: &BattleCtx<'_, GenericProvider>,
        move_type_index: usize,
        defender: BattlerRef,
    ) -> (u32, u32) {
        let Some(host) = GenericProvider::rules_host() else {
            return (1, 1);
        };
        let Some(element) = &ctx.battler(defender).species.element else {
            return (1, 1);
        };
        let Some(def_index) = host
            .compiled
            .types
            .iter()
            .position(|t| t.eq_ignore_ascii_case(element))
        else {
            return (1, 1);
        };
        host.compiled.chart_mult(move_type_index, def_index)
    }

    fn make_volatile(&self, name: &str, amount: u16) -> Option<VolatileKind> {
        Some(VolatileKind {
            name: name.to_string(),
            amount,
        })
    }

    fn has_volatile(
        &self,
        ctx: &BattleCtx<'_, GenericProvider>,
        who: BattlerRef,
        name: &str,
    ) -> bool {
        ctx.effects
            .iter()
            .any(|e| e.host == who && e.kind.name == name)
    }

    fn battler_level(&self, b: &BattlerState<GenericProvider>) -> u16 {
        u16::from(b.level)
    }

    fn has_status(&self, b: &BattlerState<GenericProvider>, status_index: usize) -> bool {
        b.status == Some(StatusId(status_index as u16))
    }

    fn has_any_status(&self, b: &BattlerState<GenericProvider>) -> bool {
        b.status.is_some()
    }

    // Resource ops use the defaults: resource index ↔ engine id is the
    // identity, and the pool lives on `BattlerState.resources` (id 0 = the
    // manifest resource — mirrored from the combatant's MP by the runner).
}

// ── compile / validate ──────────────────────────────────────────────────────

/// The closed status vocabulary: the ruleset's `kind: Status` record ids, in
/// declaration order. Supplied to the loader at compile so
/// `InflictStatus(status: "...")` / `TargetHasStatus("...")` validate at LOAD.
pub fn status_index_of(ruleset: &Ruleset, name: &str) -> Option<usize> {
    ruleset
        .effects
        .iter()
        .filter(|r| r.kind == EffectKind::Status)
        .position(|r| r.id == name)
}

/// The status record ids in declaration order (`StatusId(idx)` → record id —
/// the residual pass's effect source and the narrator's display name).
pub fn status_names(ruleset: &Ruleset) -> Vec<String> {
    ruleset
        .effects
        .iter()
        .filter(|r| r.kind == EffectKind::Status)
        .map(|r| r.id.clone())
        .collect()
}

/// Compile a [`Ruleset`] into the generic registry, validating every name
/// against the closed vocabulary NOW (unknown event/op/stat/type/resource/
/// status ⇒ [`LoadError`] at load, never mid-battle).
pub fn compile_ruleset(ruleset: &Ruleset) -> Result<CompiledRuleset, LoadError> {
    CompiledRuleset::compile::<GenericProvider, GenericBindings>(
        ruleset,
        DATA_ID_BASE,
        &GenericBindings,
        |name| status_index_of(ruleset, name),
    )
}

/// The `kind: Move` records' override map (skill id → [`RonMove`]). `cost:`
/// entries naming the manifest `resource` become the skill's MP cost
/// (summed); a record with no `cost:` leaves the table record's cost in
/// place.
pub fn ron_moves(ruleset: &Ruleset, resource: Option<&str>) -> HashMap<String, RonMove> {
    ruleset
        .effects
        .iter()
        .filter(|r| r.kind == EffectKind::Move)
        .map(|rec| {
            let cost = if rec.cost.is_empty() {
                None
            } else {
                resource.map(|res| {
                    rec.cost
                        .iter()
                        .filter(|c| c.resource == res)
                        .map(|c| u32::from(c.amount))
                        .sum()
                })
            };
            (
                rec.id.clone(),
                RonMove {
                    power: rec.power,
                    accuracy: rec.accuracy,
                    mtype: rec.mtype.clone(),
                    cost,
                },
            )
        })
        .collect()
}

/// Full closed-vocabulary validation of a `rules.ron` text (the `dotzuki check`
/// path): parse + compile. Returns one diagnostic per problem (empty = clean).
pub fn validate_ruleset(rules_text: &str) -> Vec<String> {
    match Ruleset::from_ron(rules_text) {
        Err(e) => vec![e.to_string()],
        Ok(ruleset) => match compile_ruleset(&ruleset) {
            Ok(_) => Vec::new(),
            Err(e) => vec![e.to_string()],
        },
    }
}

// ── the per-battle hook state ───────────────────────────────────────────────

/// A `kind: Move` RON record's overrides for the matching skill-table record.
/// Fields left `None` fall back to the table record.
#[derive(Debug, Clone, Default)]
pub struct RonMove {
    /// Base power override.
    pub power: Option<u32>,
    /// Accuracy override.
    pub accuracy: Option<u32>,
    /// Attacking element override (the record's `type:`).
    pub mtype: Option<String>,
    /// Resource cost override (the record's `cost:` entries naming the
    /// manifest resource, summed).
    pub cost: Option<u32>,
}

/// The per-battle hook machinery: both combatants mirrored as engine
/// [`BattlerState`]s (side 0 = player, side 1 = enemy), the effect arena, the
/// per-action scratch, and the built effect registry (one leaked `Effect` per
/// compiled hook — the deliberate one-time leak, minimon/wuxia precedent).
pub struct HookState {
    /// The engine-side battle state (the interpreter's mutation target).
    pub state: BattleState<GenericProvider>,
    /// The live volatile arena.
    pub effects: Vec<EffectState<GenericProvider>>,
    /// Per-action scratch (`damage`, `last_damage`).
    pub mv: MoveContext,
    /// Every synthesized per-hook effect (filtered by source record id at
    /// fire time).
    pub registry: Vec<&'static Effect<GenericProvider>>,
    /// Skill id → its `kind: Move` RON record's overrides.
    pub move_records: HashMap<String, RonMove>,
    /// `StatusId(idx)` → the status record id (residual source + narration).
    pub status_names: Vec<String>,
    /// The interned stat names (the ruleset's `stats:` list, in order).
    pub stat_names: Vec<String>,
    /// Whether the manifest maps a resource field (the MP pool mirror).
    pub has_resource: bool,
}

impl HookState {
    /// The battler ref of one side (1v1: slot 0).
    pub fn battler_ref(side: super::Side) -> BattlerRef {
        match side {
            super::Side::Player => BattlerRef::PLAYER,
            super::Side::Enemy => BattlerRef::OPPONENT,
        }
    }

    /// The engine battler for a side.
    pub fn battler(&self, side: super::Side) -> &BattlerState<GenericProvider> {
        let r = Self::battler_ref(side);
        if r.side == 0 {
            &self.state.player_battlers[r.slot as usize]
        } else {
            &self.state.opponent_battlers[r.slot as usize]
        }
    }

    /// Whether the skill's RON record subscribes to `event` (used to decide
    /// gate/fold firing and the no-hooks chart fallback).
    pub fn subscribes(&self, skill_id: &str, event: Event) -> bool {
        let Some(host) = GenericProvider::rules_host() else {
            return false;
        };
        host.compiled
            .hooks
            .values()
            .any(|h| h.source_id == skill_id && h.event == event)
    }
}

/// The runner's byte-stream rng as the engine's rng trait (one `next_u8` per
/// `byte`, so a scripted stream replays a battle exactly — accuracy, variance,
/// crit, then the hooks' `chance` gates, in fire order).
pub struct RngAdapter<'a>(pub &'a mut dyn super::BattleRng);

impl EngineRng for RngAdapter<'_> {
    fn next_u8(&mut self) -> u8 {
        self.0.byte()
    }
}

// ── Combatant ↔ BattlerState mirroring ──────────────────────────────────────
//
// The v1 `Combatant` stays the loop/UI authority for HP/MP/stages; the mirror
// is the interpreter's mutation target. The runner syncs Combatant → mirror
// before an action's event sequence and mirror → Combatant after, so v1 paths
// (a non-RON skill mid-battle) and hook paths never diverge. The non-volatile
// status rides the same sync as the record-id string on the Combatant (v2-b:
// statuses persist on the party member across switches and battles); an id
// outside the RON `kind: Status` vocabulary reads as no status. HP/MP clamp
// into the engine's u16.

/// A combatant's raw stat by RON stat name (canonical key mapping).
fn raw_stat(c: &Combatant, name: &str) -> u32 {
    match normalize_stat_key(name).as_str() {
        "hp" => c.max_hp,
        "defense" => c.defense,
        "speed" => c.speed,
        _ => c.attack,
    }
}

/// The mirror status for a combatant's status record id (unknown ids drop —
/// a status name outside the ruleset's `kind: Status` vocabulary can't be
/// interpreted).
fn status_id_of(status: &Option<String>, status_names: &[String]) -> Option<StatusId> {
    status
        .as_ref()
        .and_then(|name| status_names.iter().position(|n| n == name))
        .map(|idx| StatusId(idx as u16))
}

/// Build the engine mirror of a combatant (its CURRENT pools — HP/MP/stages
/// and status; used at battle start and on switch-in).
pub fn mirror_of(
    c: &Combatant,
    stat_names: &[String],
    status_names: &[String],
    has_resource: bool,
) -> BattlerState<GenericProvider> {
    let mut b = BattlerState::new(
        SpeciesData {
            element: c.element.clone(),
        },
        c.hp.min(u32::from(u16::MAX)) as u16,
        c.max_hp.min(u32::from(u16::MAX)) as u16,
        EnumMap::default(),
        c.skills.clone(),
    )
    .with_level(c.level);
    b.status = status_id_of(&c.status, status_names);
    sync_to_mirror(c, &mut b, stat_names, status_names, has_resource);
    b
}

/// Copy the mutable pools (HP/MP/levels/stats/stages/status) Combatant →
/// mirror.
pub fn sync_to_mirror(
    c: &Combatant,
    b: &mut BattlerState<GenericProvider>,
    stat_names: &[String],
    status_names: &[String],
    has_resource: bool,
) {
    b.hp = c.hp.min(u32::from(u16::MAX)) as u16;
    b.max_hp = c.max_hp.min(u32::from(u16::MAX)) as u16;
    b.level = c.level;
    b.status = status_id_of(&c.status, status_names);
    for (i, name) in stat_names.iter().enumerate() {
        let id = StatId(i as u16);
        b.stats
            .set(id, raw_stat(c, name).min(u32::from(u16::MAX)) as u16);
        b.stat_stages.set(id, c.stages.get(name));
    }
    if has_resource {
        b.resources.set(
            0,
            c.mp.min(u32::from(u16::MAX)) as u16,
            c.max_mp.min(u32::from(u16::MAX)) as u16,
        );
    }
}

/// Copy the pools back mirror → Combatant (after each event fire).
pub fn sync_from_mirror(
    b: &BattlerState<GenericProvider>,
    c: &mut Combatant,
    stat_names: &[String],
    status_names: &[String],
    has_resource: bool,
) {
    c.hp = u32::from(b.hp);
    c.max_hp = u32::from(b.max_hp);
    c.status = b
        .status
        .as_ref()
        .and_then(|id| status_names.get(id.0 as usize).cloned());
    for (i, name) in stat_names.iter().enumerate() {
        let id = StatId(i as u16);
        if let Some(stage) = b.stat_stages.get(id) {
            c.stages.set(name, *stage);
        }
    }
    if has_resource {
        c.mp = u32::from(b.resources.current(0).unwrap_or(0));
        c.max_mp = u32::from(b.resources.max(0).unwrap_or(0));
    }
}
