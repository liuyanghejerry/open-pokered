//! Phase-1 unit tests (doc 11 §6 POC spec, scoped to the new crate — minimon
//! wiring is Phase 2). Each primitive is proved ONCE through the real engine
//! `run_event` fold; load-time vs battle-time error handling is proved; and a
//! `ScriptedRng` is proved to replay a data ruleset identically.
//!
//! The test provider here is the ONLY concrete game type in this crate and lives
//! entirely under `#[cfg(test)]` — the non-test crate stays 100% game-agnostic.

use std::cell::RefCell;

use dotzuki_engine::battle::rng::{BattleRng, ScriptedRng};
use dotzuki_engine::battle::stack::{
    collect_handlers, run_event, BattleCtx, Effect, EffectId, EffectProvider, EffectState, Event,
    MoveContext, RelayVar,
};
use dotzuki_engine::battle::{
    BattleProvider, BattleState, BattlerRef, BattlerState, DamageResult, EffectResult, EnumMap,
    MoveEffect,
};

use crate::bindings::RuleBindings;
use crate::model::{LoadError, Ruleset};
use crate::registry::{CompiledRuleset, RulesHost, RulesProvider};

// ─────────────────────────────────────────────────────────────────────────────
// A minimal concrete game provider for the proofs (test-only).
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stat {
    Hp,
    Atk,
    SpD,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Burn,
    Poison,
    Sleep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MType {
    Normal,
    Rock,
    Fire,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Species {
    mtype: MType,
    /// Per-battler level (the `LevelGE` predicate + level-based `SetDamage`
    /// sources read this via the binding; P3). Defaults to 50 in the harness.
    level: u16,
}

#[derive(Debug, Clone, PartialEq)]
struct Move {
    power: u8,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)] // the inert variant of the `EffectStateKind` shape
enum Kind {
    None,
    /// A Substitute volatile (exercises the `HasVolatile` predicate).
    Substitute,
}

struct TestProvider;

impl BattleProvider for TestProvider {
    type Monster = ();
    type Move = Move;
    type Ability = ();
    type Status = Status;
    type Stat = Stat;
    type Species = Species;
    type Type = MType;
    type Item = ();

    fn calculate_damage(
        &self,
        m: &Self::Move,
        _a: &BattlerState<Self>,
        _d: &BattlerState<Self>,
        _r: u8,
        _c: bool,
    ) -> DamageResult {
        DamageResult {
            damage: m.power as u16,
            effectiveness: 1.0,
            is_miss: false,
        }
    }
    fn select_move(&self, b: &BattlerState<Self>, _s: &BattleState<Self>) -> Self::Move {
        b.moves.first().cloned().unwrap_or(Move { power: 0 })
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

impl EffectProvider for TestProvider {
    type EffectStateKind = Kind;
    fn effect_for_move(&self, _m: &Self::Move) -> Option<&'static Effect<Self>> {
        None
    }
    fn effect_for_status(&self, _s: &Self::Status) -> Option<&'static Effect<Self>> {
        None
    }
    fn turn_order_rank(
        &self,
        _s: &BattleState<Self>,
        _w: BattlerRef,
        _a: &Self::Move,
    ) -> (i32, i32) {
        (0, 0)
    }
}

// ── The game binding: maps interned indices ↔ concrete Stat/Status/Type. ──────

struct TestBindings;

impl RuleBindings<TestProvider> for TestBindings {
    fn apply_boost(
        &self,
        b: &mut BattlerState<TestProvider>,
        stat_index: usize,
        stages: i8,
    ) -> bool {
        let stat = match stat_index {
            0 => Stat::Hp,
            1 => Stat::Atk,
            2 => Stat::SpD,
            _ => return false,
        };
        let cur = b.stat_stages.get(stat).copied().unwrap_or(0);
        b.stat_stages.set(stat, cur + stages);
        true
    }
    fn set_status(&self, b: &mut BattlerState<TestProvider>, status_index: usize) -> bool {
        let s = match status_index {
            0 => Status::Burn,
            1 => Status::Poison,
            2 => Status::Sleep,
            _ => return false,
        };
        b.status = Some(s);
        true
    }
    fn has_type(&self, b: &BattlerState<TestProvider>, type_index: usize) -> bool {
        b.species.mtype.chart_index() == type_index
    }
    /// The `HasVolatile` predicate: does `who` host a live `Kind::Substitute` arena
    /// entry under the name `"Substitute"`? The game owns the arena ↔ name mapping.
    fn has_volatile(&self, ctx: &BattleCtx<'_, TestProvider>, who: BattlerRef, name: &str) -> bool {
        name == "Substitute"
            && ctx
                .effects
                .iter()
                .any(|e| e.host == who && e.kind == Kind::Substitute)
    }
    /// The `TargetHasStatus` predicate: does `b` currently have `status_index`?
    fn has_status(&self, b: &BattlerState<TestProvider>, status_index: usize) -> bool {
        let want = match status_index {
            0 => Status::Burn,
            1 => Status::Poison,
            2 => Status::Sleep,
            _ => return false,
        };
        b.status == Some(want)
    }
    /// The `LevelGE` gate + level-based `SetDamage` sources read the battler's
    /// level here (P3). The engine carries no level; the game supplies it.
    fn battler_level(&self, b: &BattlerState<TestProvider>) -> u16 {
        b.species.level
    }
    fn type_chart_mult(
        &self,
        ctx: &BattleCtx<'_, TestProvider>,
        move_type_index: usize,
        defender: BattlerRef,
    ) -> (u32, u32) {
        // A tiny chart: Fire(2) → Rock(1) is 1/2 (resisted); Fire(2) → Fire(2)
        // is 2/1 (super-effective, contrived) — enough to prove the fold.
        let def = ctx.battler(defender).species.mtype.chart_index();
        match (move_type_index, def) {
            (2, 1) => (1, 2), // Fire vs Rock → resisted
            (2, 2) => (2, 1), // Fire vs Fire → super-effective (contrived)
            _ => (1, 1),
        }
    }
}

impl MType {
    fn chart_index(self) -> usize {
        match self {
            MType::Normal => 0,
            MType::Rock => 1,
            MType::Fire => 2,
        }
    }
}

// ── The RulesProvider bridge: a thread-local `&'static RulesHost`. ────────────
//
// `rules_host()` must return `&'static`. Tests install a fresh leaked host per
// test (each test calls `install_host`), so tests stay isolated; the leak is a
// deliberate, bounded test-only cost.

thread_local! {
    static HOST: RefCell<Option<&'static RulesHost<TestProvider>>> = const { RefCell::new(None) };
}

fn install_host(host: RulesHost<TestProvider>) {
    let leaked: &'static RulesHost<TestProvider> = Box::leak(Box::new(host));
    HOST.with(|h| *h.borrow_mut() = Some(leaked));
}

impl RulesProvider for TestProvider {
    type Bindings = TestBindings;
    fn compiled(&self) -> &CompiledRuleset {
        &Self::rules_host().expect("host installed").compiled
    }
    fn bindings(&self) -> &Self::Bindings {
        &Self::rules_host().expect("host installed").bindings
    }
    fn rules_host() -> Option<&'static RulesHost<TestProvider>> {
        HOST.with(|h| *h.borrow())
    }
}

// ── status-name → game status index (the game's vocabulary). ──────────────────

fn status_index_of(name: &str) -> Option<usize> {
    match name {
        "burn" => Some(0),
        "poison" => Some(1),
        "sleep" => Some(2),
        _ => None,
    }
}

// ── Test harness: compile a ruleset, install it, build a battle, fire one event.

fn compile_and_install(ron: &str) -> Result<(), LoadError> {
    let ruleset = Ruleset::from_ron(ron)?;
    let compiled = CompiledRuleset::compile::<TestProvider, TestBindings>(
        &ruleset,
        0x1000,
        &TestBindings,
        status_index_of,
    )?;
    install_host(RulesHost::new(compiled, TestBindings));
    Ok(())
}

struct Harness {
    state: BattleState<TestProvider>,
    effects: Vec<EffectState<TestProvider>>,
    mv: MoveContext,
}

impl Harness {
    fn new(player_type: MType, opp_type: MType) -> Self {
        let p = BattlerState::new(
            Species {
                mtype: player_type,
                level: 50,
            },
            100,
            100,
            EnumMap::new(),
            vec![],
        );
        let o = BattlerState::new(
            Species {
                mtype: opp_type,
                level: 50,
            },
            100,
            100,
            EnumMap::new(),
            vec![],
        );
        Self {
            state: BattleState::new(vec![p], vec![o]),
            effects: Vec::new(),
            mv: MoveContext::default(),
        }
    }

    /// Fire `ev` collecting only the synthesized data effects, with `relay`, and
    /// return the folded relay. The data effects are built from the installed
    /// registry and routed exactly like the engine would route a move's effect.
    fn fire(&mut self, ev: Event, relay: RelayVar, rng: &mut dyn BattleRng) -> RelayVar {
        let provider = TestProvider;
        let data_effects: Vec<&'static Effect<TestProvider>> = TestProvider::rules_host()
            .unwrap()
            .compiled
            .build_effects::<TestProvider>();
        let mut ctx = BattleCtx {
            state: &mut self.state,
            effects: &mut self.effects,
            mv: &mut self.mv,
            rng,
        };
        let mut hs = Vec::new();
        for eff in &data_effects {
            collect_handlers(
                &ctx,
                &provider,
                Some(eff),
                ev,
                BattlerRef::OPPONENT,
                BattlerRef::PLAYER,
                &mut hs,
            );
        }
        run_event(&mut ctx, hs, relay, false)
    }

    /// Set the in-flight move's `last_damage` (the `FractionOf::LastDamage` base).
    fn set_last_damage(&mut self, dmg: u16) {
        self.mv.last_damage = dmg;
    }
    /// Give `who` a live Substitute volatile (exercises `HasVolatile`).
    fn give_substitute(&mut self, who: BattlerRef) {
        self.effects.push(EffectState {
            id: EffectId(900 + self.effects.len() as u32),
            host: who,
            effect_order: self.effects.len() as u64,
            kind: Kind::Substitute,
        });
        self.effects.sort_by(|a, b| a.id.cmp(&b.id));
    }
    fn opp_hp(&self) -> u16 {
        self.state.opponent_battlers[0].hp
    }
    fn player_hp(&self) -> u16 {
        self.state.player_battlers[0].hp
    }
    fn opp_status(&self) -> Option<Status> {
        self.state.opponent_battlers[0].status
    }
    fn opp_atk_stage(&self) -> i8 {
        self.state.opponent_battlers[0]
            .stat_stages
            .get(Stat::Atk)
            .copied()
            .unwrap_or(0)
    }
    /// Set the player (source) and opponent (target) levels (P3 `LevelGE` / level
    /// `SetDamage` proofs).
    fn set_levels(&mut self, player: u16, opp: u16) {
        self.state.player_battlers[0].species.level = player;
        self.state.opponent_battlers[0].species.level = opp;
    }
    /// Set the opponent (target) current HP (P3 `SetHp` / `DamageCurrentHpFraction`).
    fn set_opp_hp(&mut self, hp: u16) {
        self.state.opponent_battlers[0].hp = hp;
    }
    /// The in-flight move's computed damage (`ctx.mv.damage`) — the `SetDamage` /
    /// `DamageCurrentHpFraction` write target.
    fn mv_damage(&self) -> u16 {
        self.mv.damage
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 1. ScaleRelay doubles a Damage relay.
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn scale_relay_doubles_damage() {
    let ron = r#"Ruleset(
        types: ["Normal"],
        effects: [ Effect(id:"x", kind:Move, hooks:[
            Hook(on:"ModifyDamage", do:[ ScaleRelay(num:2, den:1) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    let mut rng = ScriptedRng::new(vec![]);
    let out = h.fire(Event::ModifyDamage, RelayVar::Damage(50), &mut rng);
    assert_eq!(out, RelayVar::Damage(100));
    // No RNG was consumed (no chance gate, no tie).
    assert_eq!(rng.consumed(), 0);
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. VetoIf gives Fail (the Clear-Body pattern).
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn veto_if_relay_int_lt_fails() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"clearbody", kind:Ability, hooks:[
            Hook(on:"TryBoost", order:5, do:[ VetoIf(cond:RelayIntLt(0)) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    let mut rng = ScriptedRng::new(vec![]);
    // A negative boost delta → veto → Fail → fold returns Bool(false).
    let out = h.fire(Event::TryBoost, RelayVar::Int(-1), &mut rng);
    assert_eq!(out, RelayVar::Bool(false));
    // A non-negative delta passes through unchanged.
    let out2 = h.fire(Event::TryBoost, RelayVar::Int(1), &mut rng);
    assert_eq!(out2, RelayVar::Int(1));
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. DamageFraction reduces hp (the Sandstorm chip / poison chip).
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn damage_fraction_reduces_hp() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"psn", kind:Status, hooks:[
            Hook(on:"Residual", order:10, do:[
                DamageFraction(num:1, den:8, of:MaxHp, target:Target) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    let mut rng = ScriptedRng::new(vec![]);
    // Target is OPPONENT (see Harness::fire). 100 max_hp / 8 = 12.
    h.fire(Event::Residual, RelayVar::Unit, &mut rng);
    assert_eq!(h.opp_hp(), 88);
}

#[test]
fn damage_fraction_unless_predicate_skips() {
    // The Sandstorm "chip non-Rock" case: `unless: HasType(Rock)`.
    let ron = r#"Ruleset(
        types: ["Normal","Rock","Fire"],
        effects: [ Effect(id:"sand", kind:Weather, hooks:[
            Hook(on:"FieldResidual", order:50, do:[
                DamageFraction(num:1, den:16, of:MaxHp, target:Target, unless:HasType("Rock")) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    // Rock defender ⇒ skipped (no chip).
    let mut rock = Harness::new(MType::Normal, MType::Rock);
    let mut rng = ScriptedRng::new(vec![]);
    rock.fire(Event::FieldResidual, RelayVar::Unit, &mut rng);
    assert_eq!(rock.opp_hp(), 100);
    // Non-Rock defender ⇒ chipped 100/16 = 6.
    let mut normal = Harness::new(MType::Normal, MType::Normal);
    normal.fire(Event::FieldResidual, RelayVar::Unit, &mut rng);
    assert_eq!(normal.opp_hp(), 94);
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. HealFraction restores hp (Leftovers).
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn heal_fraction_restores_hp() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"left", kind:Item, hooks:[
            Hook(on:"Residual", order:20, do:[
                HealFraction(num:1, den:16, of:MaxHp, target:Target) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    h.state.opponent_battlers[0].hp = 50;
    let mut rng = ScriptedRng::new(vec![]);
    h.fire(Event::Residual, RelayVar::Unit, &mut rng);
    // 100/16 = 6 → 50 + 6 = 56.
    assert_eq!(h.opp_hp(), 56);
}

// ═════════════════════════════════════════════════════════════════════════════
// 5. InflictStatus sets status.
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn inflict_status_sets_status() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"ember", kind:Move, hooks:[
            Hook(on:"DamagingHit", order:10, do:[
                InflictStatus(status:"burn", target:Target) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    let mut rng = ScriptedRng::new(vec![]);
    assert_eq!(h.opp_status(), None);
    h.fire(Event::DamagingHit, RelayVar::Damage(40), &mut rng);
    assert_eq!(h.opp_status(), Some(Status::Burn));
}

/// `RngRange` REJECTION-samples instead of `byte % span` (Gen-1 sleep counter:
/// the asm re-rolls a 0 for a uniform 1–7; modulo oversamples low values). For
/// span 7 the skewed tail is bytes ≥ 252, so a 255 first byte is redrawn.
#[test]
fn rng_range_rejection_samples_skewed_tail() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"hypnosis", kind:Move, hooks:[
            Hook(on:"DamagingHit", order:10, do:[
                InflictStatus(status:"sleep", target:Target, amount: RngRange(lo:1, hi:7)) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();

    // Tail byte 255 (≥ 252 for span 7) is REJECTED → a second byte is drawn.
    let mut h = Harness::new(MType::Normal, MType::Normal);
    let mut rng = ScriptedRng::new(vec![255u8, 3]);
    h.fire(Event::DamagingHit, RelayVar::Damage(0), &mut rng);
    assert_eq!(rng.consumed(), 2, "255 is in the skewed tail → re-drawn");
    assert_eq!(h.opp_status(), Some(Status::Sleep));

    // An in-range byte (100 < 252) is accepted on the FIRST draw.
    let mut h2 = Harness::new(MType::Normal, MType::Normal);
    let mut rng2 = ScriptedRng::new(vec![100u8, 200]);
    h2.fire(Event::DamagingHit, RelayVar::Damage(0), &mut rng2);
    assert_eq!(rng2.consumed(), 1, "in-range byte → single draw");
    assert_eq!(h2.opp_status(), Some(Status::Sleep));
}

// ═════════════════════════════════════════════════════════════════════════════
// 6. Boost applies a stat-stage delta (Intimidate).
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn boost_applies_stage_delta() {
    let ron = r#"Ruleset(
        stats: ["Hp","Atk","SpD"],
        effects: [ Effect(id:"intimidate", kind:Ability, hooks:[
            Hook(on:"SwitchIn", order:10, do:[
                Boost(stat:"Atk", stages:-1, target:Target) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    let mut rng = ScriptedRng::new(vec![]);
    h.fire(Event::SwitchIn, RelayVar::Unit, &mut rng);
    assert_eq!(h.opp_atk_stage(), -1);
}

// ═════════════════════════════════════════════════════════════════════════════
// 7. The chance gate consumes EXACTLY ONE rng draw, unconditionally.
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn chance_gate_consumes_exactly_one_draw() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"ember", kind:Move, hooks:[
            Hook(on:"DamagingHit", order:10, chance:[30,100], do:[
                InflictStatus(status:"burn", target:Target) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();

    // chance(30,100): range(100) ≤ 256 ⇒ ONE byte. byte 0 ⇒ 0 < 30 ⇒ PASS.
    let mut h = Harness::new(MType::Normal, MType::Normal);
    let mut rng = ScriptedRng::new(vec![0u8]);
    h.fire(Event::DamagingHit, RelayVar::Damage(40), &mut rng);
    assert_eq!(rng.consumed(), 1, "exactly one draw on PASS");
    assert_eq!(h.opp_status(), Some(Status::Burn));

    // byte 200 ⇒ 200 % 100 = 0 ... pick byte 99 ⇒ 99 ≥ 30 ⇒ FAIL, but still ONE draw.
    let mut h2 = Harness::new(MType::Normal, MType::Normal);
    let mut rng2 = ScriptedRng::new(vec![99u8]);
    h2.fire(Event::DamagingHit, RelayVar::Damage(40), &mut rng2);
    assert_eq!(
        rng2.consumed(),
        1,
        "exactly one draw on FAIL too (drawn unconditionally)"
    );
    assert_eq!(h2.opp_status(), None, "gate failed ⇒ status NOT set");
}

// ═════════════════════════════════════════════════════════════════════════════
// 8. SetRelay / AddRelay / ClampRelay numeric folds.
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn numeric_relay_ops() {
    let ron = r#"Ruleset(
        effects: [
            Effect(id:"set", kind:Move, hooks:[ Hook(on:"ModifyStat", do:[ SetRelay(7) ]) ]),
            Effect(id:"add", kind:Move, hooks:[ Hook(on:"Accuracy", do:[ AddRelay(5) ]) ]),
            Effect(id:"clamp", kind:Move, hooks:[ Hook(on:"ModifyCritRatio", do:[ ClampRelay(lo:0, hi:10) ]) ]),
        ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    let mut rng = ScriptedRng::new(vec![]);
    assert_eq!(
        h.fire(Event::ModifyStat, RelayVar::Int(1), &mut rng),
        RelayVar::Int(7)
    );
    assert_eq!(
        h.fire(Event::Accuracy, RelayVar::Int(10), &mut rng),
        RelayVar::Int(15)
    );
    assert_eq!(
        h.fire(Event::ModifyCritRatio, RelayVar::Int(99), &mut rng),
        RelayVar::Int(10)
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// 9. ApplyTypeChart folds the move's chart multiplier (doc 12).
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn apply_type_chart_folds_multiplier() {
    let ron = r#"Ruleset(
        types: ["Normal","Rock","Fire"],
        effects: [ Effect(id:"ember", kind:Move, type:"Fire", hooks:[
            Hook(on:"Effectiveness", order:100, do:[ ApplyTypeChart ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    // Fire vs Rock ⇒ resisted 1/2: 80 → 40.
    let mut resisted = Harness::new(MType::Normal, MType::Rock);
    let mut rng = ScriptedRng::new(vec![]);
    let out = resisted.fire(Event::Effectiveness, RelayVar::Damage(80), &mut rng);
    assert_eq!(out, RelayVar::Damage(40));
    // Fire vs Fire ⇒ super-effective 2/1 (contrived): 80 → 160.
    let mut se = Harness::new(MType::Normal, MType::Fire);
    let out2 = se.fire(Event::Effectiveness, RelayVar::Damage(80), &mut rng);
    assert_eq!(out2, RelayVar::Damage(160));
    // Untyped (omitted/neutral) defender ⇒ 1×.
    let mut neutral = Harness::new(MType::Normal, MType::Normal);
    let out3 = neutral.fire(Event::Effectiveness, RelayVar::Damage(80), &mut rng);
    assert_eq!(out3, RelayVar::Damage(80));
}

// ═════════════════════════════════════════════════════════════════════════════
// 10. ScaleRelay with `when` predicates (Sandstorm WeatherModifyStat ×1.5 Rock).
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn scale_relay_when_has_type() {
    let ron = r#"Ruleset(
        types: ["Normal","Rock","Fire"],
        effects: [ Effect(id:"sand", kind:Weather, hooks:[
            Hook(on:"WeatherModifyStat", order:50, do:[
                ScaleRelay(num:3, den:2, when:[ HasType("Rock") ]) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    // Rock target ⇒ ×3/2: 100 → 150.
    let mut rock = Harness::new(MType::Normal, MType::Rock);
    let mut rng = ScriptedRng::new(vec![]);
    let out = rock.fire(Event::WeatherModifyStat, RelayVar::Int(100), &mut rng);
    assert_eq!(out, RelayVar::Int(150));
    // Non-Rock target ⇒ unchanged.
    let mut normal = Harness::new(MType::Normal, MType::Normal);
    let out2 = normal.fire(Event::WeatherModifyStat, RelayVar::Int(100), &mut rng);
    assert_eq!(out2, RelayVar::Int(100));
}

// ═════════════════════════════════════════════════════════════════════════════
// 11. Load-time errors: unknown event, unknown op, unknown type/status/stat,
//     bad chance — all fail at LOAD (compile), NOT at battle.
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn unknown_event_fails_at_load() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"x", kind:Move, hooks:[
            Hook(on:"NotAnEvent", do:[ DealMoveDamage ]) ]) ],
    )"#;
    let err = compile_and_install(ron).unwrap_err();
    assert!(matches!(err, LoadError::UnknownEvent(_)), "got {err:?}");
}

#[test]
fn unknown_op_fails_at_load() {
    // `Frobnicate` is not a closed Op variant ⇒ RON deserialize (LOAD) fails.
    let ron = r#"Ruleset(
        effects: [ Effect(id:"x", kind:Move, hooks:[
            Hook(on:"ModifyDamage", do:[ Frobnicate ]) ]) ],
    )"#;
    let err = compile_and_install(ron).unwrap_err();
    assert!(matches!(err, LoadError::Ron(_)), "got {err:?}");
}

#[test]
fn unknown_status_fails_at_load() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"x", kind:Move, hooks:[
            Hook(on:"DamagingHit", do:[ InflictStatus(status:"sparkle", target:Target) ]) ]) ],
    )"#;
    let err = compile_and_install(ron).unwrap_err();
    assert!(matches!(err, LoadError::UnknownStatus(_)), "got {err:?}");
}

#[test]
fn unknown_type_fails_at_load() {
    let ron = r#"Ruleset(
        types: ["Normal"],
        effects: [ Effect(id:"x", kind:Weather, hooks:[
            Hook(on:"FieldResidual", do:[
                DamageFraction(num:1, den:16, of:MaxHp, target:Target, unless:HasType("Lava")) ]) ]) ],
    )"#;
    let err = compile_and_install(ron).unwrap_err();
    assert!(matches!(err, LoadError::UnknownType(_)), "got {err:?}");
}

#[test]
fn unknown_stat_fails_at_load() {
    let ron = r#"Ruleset(
        stats: ["Hp","Atk"],
        effects: [ Effect(id:"x", kind:Ability, hooks:[
            Hook(on:"SwitchIn", do:[ Boost(stat:"Sass", stages:-1, target:Foe) ]) ]) ],
    )"#;
    let err = compile_and_install(ron).unwrap_err();
    assert!(matches!(err, LoadError::UnknownStat(_)), "got {err:?}");
}

#[test]
fn bad_chance_fails_at_load() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"x", kind:Move, hooks:[
            Hook(on:"DamagingHit", chance:[1,0], do:[ DealMoveDamage ]) ]) ],
    )"#;
    let err = compile_and_install(ron).unwrap_err();
    assert!(matches!(err, LoadError::BadChance(1, 0)), "got {err:?}");
}

#[test]
fn custom_event_parses() {
    // The open tail: `Custom(7)` is reachable as data with no engine change.
    let ron = r#"Ruleset(
        effects: [ Effect(id:"x", kind:Move, hooks:[
            Hook(on:"Custom(7)", do:[ SetRelay(1) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let host = TestProvider::rules_host().unwrap();
    assert!(host
        .compiled
        .hooks
        .values()
        .any(|h| h.event == Event::Custom(7)));
}

// ═════════════════════════════════════════════════════════════════════════════
// 12. ScriptedRng replays a data ruleset IDENTICALLY (same draw count & order).
//     The decisive determinism guarantee (doc 11 §4.1).
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn scripted_rng_replays_identically() {
    // Two chance-gated hooks on the same event ⇒ two draws, in op-list order.
    let ron = r#"Ruleset(
        effects: [
            Effect(id:"a", kind:Move, hooks:[
                Hook(on:"DamagingHit", order:10, chance:[50,100], do:[
                    InflictStatus(status:"burn", target:Target) ]) ]),
            Effect(id:"b", kind:Move, hooks:[
                Hook(on:"DamagingHit", order:20, chance:[50,100], do:[
                    InflictStatus(status:"poison", target:Source) ]) ]),
        ],
    )"#;
    compile_and_install(ron).unwrap();

    let script = vec![10u8, 200u8]; // draw1=10 (<50 PASS burn), draw2=200%100=0 (<50 PASS poison)

    // Run #1.
    let mut h1 = Harness::new(MType::Normal, MType::Normal);
    let mut rng1 = ScriptedRng::new(script.clone());
    h1.fire(Event::DamagingHit, RelayVar::Damage(40), &mut rng1);
    let draws1 = rng1.consumed();
    let burn1 = h1.opp_status();
    let psn1 = h1.state.player_battlers[0].status;

    // Run #2 — same ruleset, same script ⇒ MUST be byte-identical.
    let mut h2 = Harness::new(MType::Normal, MType::Normal);
    let mut rng2 = ScriptedRng::new(script.clone());
    h2.fire(Event::DamagingHit, RelayVar::Damage(40), &mut rng2);
    let draws2 = rng2.consumed();
    let burn2 = h2.opp_status();
    let psn2 = h2.state.player_battlers[0].status;

    // Identical draw COUNT, identical ORDER (same outcomes), identical state.
    assert_eq!(draws1, 2, "two chance gates ⇒ exactly two draws");
    assert_eq!(draws1, draws2, "draw count identical across replays");
    assert_eq!(burn1, burn2);
    assert_eq!(psn1, psn2);
    assert_eq!(burn1, Some(Status::Burn));
    assert_eq!(psn1, Some(Status::Poison));
}

// ═════════════════════════════════════════════════════════════════════════════
// 13. The determinism trace records (EffectId, Event, op, relay before/after).
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn trace_records_op_steps() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"x", kind:Move, hooks:[
            Hook(on:"ModifyDamage", do:[ ScaleRelay(num:2, den:1) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    let mut rng = ScriptedRng::new(vec![]);

    crate::enable_trace();
    h.fire(Event::ModifyDamage, RelayVar::Damage(50), &mut rng);
    let trace = crate::take_trace().expect("trace enabled");

    assert_eq!(trace.events.len(), 1);
    let e = &trace.events[0];
    assert_eq!(e.event, Event::ModifyDamage);
    assert_eq!(e.before, RelayVar::Damage(50));
    assert_eq!(e.after, RelayVar::Damage(100));
}

// ═════════════════════════════════════════════════════════════════════════════
// 14. The interpreter keys off `source_effect`: two effects on the SAME event
//     run their OWN op-lists, distinguished only by the threaded EffectId.
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn interpreter_keys_off_source_effect() {
    let ron = r#"Ruleset(
        effects: [
            Effect(id:"chip", kind:Status, hooks:[
                Hook(on:"Residual", order:10, do:[
                    DamageFraction(num:1, den:8, of:MaxHp, target:Target) ]) ]),
            Effect(id:"heal", kind:Item, hooks:[
                Hook(on:"Residual", order:20, do:[
                    HealFraction(num:1, den:16, of:MaxHp, target:Target) ]) ]),
        ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    let mut rng = ScriptedRng::new(vec![]);
    // order 10 chip (-12) BEFORE order 20 heal (+6): 100 - 12 + 6 = 94.
    h.fire(Event::Residual, RelayVar::Unit, &mut rng);
    assert_eq!(
        h.opp_hp(),
        94,
        "cross-source residual ordering via source_effect keys"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// 15. The canonical `rules.ron` (doc 11 §1 five systems + doc 12 §2 chart) loads
//     and compiles end-to-end — every name binds to the closed vocabulary.
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn canonical_rules_ron_loads_and_compiles() {
    let ron = include_str!("../rules.ron");
    let ruleset = Ruleset::from_ron(ron).expect("rules.ron parses");
    // Sanity: the five systems + two demo moves = 7 effect records.
    assert_eq!(ruleset.effects.len(), 7);
    assert_eq!(ruleset.type_chart.len(), 11);
    let compiled = CompiledRuleset::compile::<TestProvider, TestBindings>(
        &ruleset,
        0x2000,
        &TestBindings,
        status_index_of,
    )
    .expect("rules.ron compiles — every name binds to the closed vocabulary");
    // Every hook minted a distinct EffectId keying its op-list.
    assert!(!compiled.hooks.is_empty());
    let ids: std::collections::HashSet<_> = compiled.hooks.keys().collect();
    assert_eq!(
        ids.len(),
        compiled.hooks.len(),
        "EffectIds are distinct per hook"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// 16. The compiled `type_chart` is interned + queryable, and an omitted pair
//     defaults to neutral (1,1). The DATA layer owns the chart (doc 12 §2).
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn compiled_chart_mult_interns_and_defaults_neutral() {
    let ron = r#"Ruleset(
        types: ["Metal","Wood","Fire"],
        type_chart: [
            ( atk: "Metal", def: "Wood", mult: [2, 1] ),   // super-effective
            ( atk: "Wood",  def: "Metal", mult: [1, 2] ),  // resisted
            ( atk: "Fire",  def: "Wood", mult: [0, 1] ),   // immune
        ],
        effects: [],
    )"#;
    let ruleset = Ruleset::from_ron(ron).unwrap();
    let compiled = CompiledRuleset::compile::<TestProvider, TestBindings>(
        &ruleset,
        0x3000,
        &TestBindings,
        status_index_of,
    )
    .unwrap();
    // Indices are the `types:` positions: Metal=0, Wood=1, Fire=2.
    assert_eq!(
        compiled.chart_mult(0, 1),
        (2, 1),
        "Metal→Wood super-effective"
    );
    assert_eq!(compiled.chart_mult(1, 0), (1, 2), "Wood→Metal resisted");
    assert_eq!(compiled.chart_mult(2, 1), (0, 1), "Fire→Wood immune");
    assert_eq!(
        compiled.chart_mult(1, 2),
        (1, 1),
        "omitted Wood→Fire defaults neutral"
    );
    assert_eq!(
        compiled.chart_mult(0, 2),
        (1, 1),
        "omitted Metal→Fire defaults neutral"
    );
}

#[test]
fn unknown_chart_type_fails_at_load() {
    // A chart edge naming a type NOT in `types:` is a LOAD error (never battle).
    let ron = r#"Ruleset(
        types: ["Metal"],
        type_chart: [ ( atk: "Metal", def: "Plasma", mult: [2, 1] ) ],
        effects: [],
    )"#;
    let ruleset = Ruleset::from_ron(ron).unwrap();
    let err = CompiledRuleset::compile::<TestProvider, TestBindings>(
        &ruleset,
        0x4000,
        &TestBindings,
        status_index_of,
    )
    .unwrap_err();
    assert!(matches!(err, LoadError::UnknownType(_)), "got {err:?}");
}

// ═════════════════════════════════════════════════════════════════════════════
// 13. RESOURCE / MP cost gate (doc 13 §4): the `cost:` field + `PayResource` op.
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn move_cost_field_interns_resource_index_and_amount() {
    // A move declares `cost: [(resource:"MP", amount:4)]`; the loader interns the
    // resource name to its `resources:` index and exposes it via `move_cost`.
    let ron = r#"Ruleset(
        resources: ["MP"],
        effects: [ Effect(id:"move.fireball", kind:Move, cost:[ Cost(resource:"MP", amount:4) ],
            hooks:[ Hook(on:"ModifyDamage", do:[ DealMoveDamage ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let host = TestProvider::rules_host().unwrap();
    assert_eq!(
        host.compiled.move_cost("move.fireball"),
        &[(0usize, 4u16)],
        "MP interned to resource index 0, amount 4"
    );
    assert_eq!(
        host.compiled.move_cost("move.nonexistent"),
        &[],
        "a move with no cost: returns an empty cost slice (inert)"
    );
}

#[test]
fn unknown_resource_in_cost_fails_at_load() {
    // A `cost:` naming a resource NOT in `resources:` is a LOAD error.
    let ron = r#"Ruleset(
        resources: ["MP"],
        effects: [ Effect(id:"x", kind:Move, cost:[ Cost(resource:"Stamina", amount:2) ],
            hooks:[ Hook(on:"ModifyDamage", do:[ DealMoveDamage ]) ]) ],
    )"#;
    let err = compile_and_install(ron).unwrap_err();
    assert!(matches!(err, LoadError::UnknownResource(_)), "got {err:?}");
}

#[test]
fn unknown_resource_in_pay_resource_op_fails_at_load() {
    // A `PayResource` op naming an unknown resource is a LOAD error too.
    let ron = r#"Ruleset(
        resources: ["MP"],
        effects: [ Effect(id:"x", kind:Move, hooks:[
            Hook(on:"BeforeMove", do:[ PayResource(resource:"Mana", amount:1, target:Source) ]) ]) ],
    )"#;
    let err = compile_and_install(ron).unwrap_err();
    assert!(matches!(err, LoadError::UnknownResource(_)), "got {err:?}");
}

#[test]
fn pay_resource_op_deducts_when_affordable_and_consumes_no_rng() {
    // A `BeforeMove` hook pays 4 MP from the Source (the actor). With 10 MP the
    // payer affords it ⇒ the op passes (Unchanged) and 10 - 4 = 6 MP remain.
    let ron = r#"Ruleset(
        resources: ["MP"],
        effects: [ Effect(id:"x", kind:Move, hooks:[
            Hook(on:"BeforeMove", do:[ PayResource(resource:"MP", amount:4, target:Source) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    // The actor is the Source = PLAYER (the harness fires source=PLAYER).
    h.state.player_battlers[0].resources.set(0, 10, 10); // resource id 0 = MP
    let mut rng = ScriptedRng::new(vec![]);
    let out = h.fire(Event::BeforeMove, RelayVar::Bool(true), &mut rng);
    assert_eq!(
        out,
        RelayVar::Bool(true),
        "affordable ⇒ relay passes (move allowed)"
    );
    assert_eq!(
        h.state.player_battlers[0].resources.current(0),
        Some(6),
        "10 MP - 4 cost = 6 left"
    );
    assert_eq!(
        rng.consumed(),
        0,
        "the PayResource op consumes NO randomness"
    );
}

#[test]
fn pay_resource_op_vetoes_when_unaffordable_and_leaves_mp_unchanged() {
    // With only 3 MP the payer cannot afford the 4-MP cost ⇒ the op Fails ⇒ the
    // fold returns Bool(false) (the move is prevented) ⇒ MP unchanged at 3.
    let ron = r#"Ruleset(
        resources: ["MP"],
        effects: [ Effect(id:"x", kind:Move, hooks:[
            Hook(on:"BeforeMove", do:[ PayResource(resource:"MP", amount:4, target:Source) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    h.state.player_battlers[0].resources.set(0, 3, 10);
    let mut rng = ScriptedRng::new(vec![]);
    let out = h.fire(Event::BeforeMove, RelayVar::Bool(true), &mut rng);
    assert_eq!(
        out,
        RelayVar::Bool(false),
        "unaffordable ⇒ Fail ⇒ move prevented"
    );
    assert_eq!(
        h.state.player_battlers[0].resources.current(0),
        Some(3),
        "prevented move deducts NOTHING ⇒ MP unchanged at 3"
    );
    assert_eq!(rng.consumed(), 0, "the veto path consumes NO randomness");
}

// ═════════════════════════════════════════════════════════════════════════════
// P2 — the new FractionOf::LastDamage base + the new predicates HasVolatile,
//      MoveTypeIsDefenderType, TargetHasStatus (blueprint 15 §2/§3).
// ═════════════════════════════════════════════════════════════════════════════

/// `HealFraction(of: LastDamage, 1/2, Source)` heals the mover by HALF the damage
/// just dealt (the Gen-1 Drain base) with the `.max(1)` floor. Source = PLAYER.
#[test]
fn heal_fraction_of_last_damage_drains_half() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"absorb", kind:Move, hooks:[
            Hook(on:"DamagingHit", do:[ HealFraction(num:1, den:2, of:LastDamage, target:Source) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    h.state.player_battlers[0].hp = 50; // damaged mover
    h.set_last_damage(60); // the move just dealt 60
    let mut rng = ScriptedRng::new(vec![]);
    h.fire(Event::DamagingHit, RelayVar::Damage(60), &mut rng);
    // 60/2 = 30 → 50 + 30 = 80 (Source = PLAYER).
    assert_eq!(h.player_hp(), 80, "drain heals half the dealt damage");
    assert_eq!(rng.consumed(), 0, "LastDamage is a pure ctx read, no rng");
}

/// The `.max(1)` floor: a 1-damage hit still drains 1 (1/2 truncates to 0, floored
/// to 1) — matching the legacy `(dealt/2).max(1)`.
#[test]
fn heal_fraction_of_last_damage_floors_at_one() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"absorb", kind:Move, hooks:[
            Hook(on:"DamagingHit", do:[ HealFraction(num:1, den:2, of:LastDamage, target:Source) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    h.state.player_battlers[0].hp = 50;
    h.set_last_damage(1);
    let mut rng = ScriptedRng::new(vec![]);
    h.fire(Event::DamagingHit, RelayVar::Damage(1), &mut rng);
    assert_eq!(h.player_hp(), 51, "1 dmg drains the floored 1");
    // A 0-damage event drains NOTHING (no floor).
    let mut h0 = Harness::new(MType::Normal, MType::Normal);
    h0.state.player_battlers[0].hp = 50;
    h0.set_last_damage(0);
    let mut rng0 = ScriptedRng::new(vec![]);
    h0.fire(Event::DamagingHit, RelayVar::Damage(0), &mut rng0);
    assert_eq!(h0.player_hp(), 50, "0 dmg drains nothing");
}

/// `DamageFraction(of: LastDamage, 1/4, Source)` deals a QUARTER of the dealt
/// damage back to the mover (the Gen-1 Recoil base). Source = PLAYER.
#[test]
fn damage_fraction_of_last_damage_recoils_quarter() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"recoil", kind:Move, hooks:[
            Hook(on:"DamagingHit", do:[ DamageFraction(num:1, den:4, of:LastDamage, target:Source) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    h.set_last_damage(100); // the move just dealt 100
    let mut rng = ScriptedRng::new(vec![]);
    h.fire(Event::DamagingHit, RelayVar::Damage(100), &mut rng);
    // 100/4 = 25 → 100 - 25 = 75 (Source = PLAYER).
    assert_eq!(
        h.player_hp(),
        75,
        "recoil hits the mover for a quarter the dealt"
    );
}

/// `VetoIf(HasVolatile("Substitute"))` blocks an `InflictStatus` when the target
/// has a Substitute up — the Gen-1 side-status Substitute block.
#[test]
fn has_volatile_substitute_vetoes_status() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"psnpwd", kind:Move, hooks:[
            Hook(on:"DamagingHit", do:[
                VetoIf(cond:HasVolatile("Substitute")),
                InflictStatus(status:"poison", target:Target) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    // With a Substitute on the target (OPPONENT) ⇒ status vetoed.
    let mut blocked = Harness::new(MType::Normal, MType::Normal);
    blocked.give_substitute(BattlerRef::OPPONENT);
    let mut rng = ScriptedRng::new(vec![]);
    blocked.fire(Event::DamagingHit, RelayVar::Damage(20), &mut rng);
    assert_eq!(blocked.opp_status(), None, "Substitute blocks the status");
    // Without a Substitute ⇒ status applies.
    let mut open = Harness::new(MType::Normal, MType::Normal);
    open.fire(Event::DamagingHit, RelayVar::Damage(20), &mut rng);
    assert_eq!(
        open.opp_status(),
        Some(Status::Poison),
        "no Substitute ⇒ poison sticks"
    );
}

/// `VetoIf(MoveTypeIsDefenderType)` blocks a same-type secondary — the Gen-1
/// burn/freeze/paralyze self-type-immunity quirk #23. The record's `type:` is the
/// move type; the OPPONENT (target) carries that type ⇒ veto.
#[test]
fn move_type_is_defender_type_vetoes_same_type() {
    let ron = r#"Ruleset(
        types: ["Normal","Rock","Fire"],
        effects: [ Effect(id:"ember", kind:Move, type:"Fire", hooks:[
            Hook(on:"DamagingHit", do:[
                VetoIf(cond:MoveTypeIsDefenderType),
                InflictStatus(status:"burn", target:Target) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    // Fire move vs a Fire defender ⇒ quirk #23 ⇒ no burn.
    let mut immune = Harness::new(MType::Normal, MType::Fire);
    let mut rng = ScriptedRng::new(vec![]);
    immune.fire(Event::DamagingHit, RelayVar::Damage(40), &mut rng);
    assert_eq!(
        immune.opp_status(),
        None,
        "Fire can't be burned by a Fire move (#23)"
    );
    // Fire move vs a non-Fire defender ⇒ burn applies.
    let mut burnt = Harness::new(MType::Normal, MType::Rock);
    burnt.fire(Event::DamagingHit, RelayVar::Damage(40), &mut rng);
    assert_eq!(
        burnt.opp_status(),
        Some(Status::Burn),
        "non-Fire ⇒ burn sticks"
    );
}

/// `VetoIf(TargetHasStatus("sleep"))` (or its negation via op ordering): here the
/// predicate matches when the target is asleep — the Dream-Eater-shaped gate. We
/// prove the predicate reads the target's status correctly. The status name is
/// interned at LOAD even though no `InflictStatus` references it.
#[test]
fn target_has_status_reads_target_status() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"gate", kind:Move, hooks:[
            Hook(on:"DamagingHit", do:[
                VetoIf(cond:TargetHasStatus("sleep")),
                DamageFraction(num:1, den:2, of:CurHp, target:Target) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    // Awake target ⇒ predicate false ⇒ the DamageFraction runs (100/2 = 50).
    let mut awake = Harness::new(MType::Normal, MType::Normal);
    let mut rng = ScriptedRng::new(vec![]);
    awake.fire(Event::DamagingHit, RelayVar::Damage(0), &mut rng);
    assert_eq!(awake.opp_hp(), 50, "awake ⇒ predicate false ⇒ op runs");
    // Asleep target ⇒ predicate true ⇒ veto ⇒ no further op.
    let mut asleep = Harness::new(MType::Normal, MType::Normal);
    asleep.state.opponent_battlers[0].status = Some(Status::Sleep);
    asleep.fire(Event::DamagingHit, RelayVar::Damage(0), &mut rng);
    assert_eq!(asleep.opp_hp(), 100, "asleep ⇒ TargetHasStatus true ⇒ veto");
}

/// `TargetHasStatus` with an unknown status name fails at LOAD (the status
/// vocabulary is validated, exactly like `InflictStatus`).
#[test]
fn target_has_status_unknown_name_fails_at_load() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"x", kind:Move, hooks:[
            Hook(on:"DamagingHit", do:[ VetoIf(cond:TargetHasStatus("dazzle")) ]) ]) ],
    )"#;
    let err = compile_and_install(ron).unwrap_err();
    assert!(matches!(err, LoadError::UnknownStatus(_)), "got {err:?}");
}

// ═════════════════════════════════════════════════════════════════════════════
// P3 — SetHp / SetDamage / DamageCurrentHpFraction ops + LevelGE predicate.
// ═════════════════════════════════════════════════════════════════════════════

/// `SetHp(target, 0)` with an empty `when` always sets the target's HP (Explode /
/// unconditional KO shape). It is an ABSOLUTE set (not `take_damage`).
#[test]
fn set_hp_zeroes_target() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"ko", kind:Move, hooks:[
            Hook(on:"DamagingHit", do:[ SetHp(target:Target, value:0) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    let mut rng = ScriptedRng::new(vec![]);
    h.fire(Event::DamagingHit, RelayVar::Unit, &mut rng);
    assert_eq!(h.opp_hp(), 0, "SetHp(0) zeroes the target");
}

/// OHKO gate (bug #19): `SetHp(Foe, 0, when:[LevelGE])` KOs only when the SOURCE
/// (user) level ≥ the TARGET (foe) level. Source=player, target=opponent. Equal
/// level connects (the `>=`); a strictly-higher foe is IMMUNE (no HP change).
#[test]
fn set_hp_when_levelge_gates_ohko() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"ohko", kind:Move, hooks:[
            Hook(on:"DamagingHit", do:[ SetHp(target:Foe, value:0, when:[LevelGE]) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    // The fire target is OPPONENT; Foe-of-OPPONENT is PLAYER. To keep the gate
    // about the foe's HP cleanly, fire with target=PLAYER so Foe=OPPONENT. The
    // Harness::fire hard-codes target=OPPONENT/source=PLAYER, so here Foe = PLAYER
    // and LevelGE compares source(PLAYER) >= target(OPPONENT). We assert the GATE.
    // user(50) >= foe(50) ⇒ KO lands (player is Foe-of-OPPONENT).
    let mut equal = Harness::new(MType::Normal, MType::Normal);
    equal.set_levels(50, 50);
    let mut rng = ScriptedRng::new(vec![]);
    equal.fire(Event::DamagingHit, RelayVar::Unit, &mut rng);
    assert_eq!(
        equal.player_hp(),
        0,
        "user level == foe level ⇒ OHKO connects (>=)"
    );
    // user(30) < foe(50) ⇒ immune (the bug-#19 gate fails ⇒ no HP change).
    let mut lower = Harness::new(MType::Normal, MType::Normal);
    lower.set_levels(30, 50);
    lower.fire(Event::DamagingHit, RelayVar::Unit, &mut rng);
    assert_eq!(
        lower.player_hp(),
        100,
        "user level < foe level ⇒ IMMUNE (bug #19)"
    );
}

/// `SetDamage(Const)` writes a fixed `ctx.mv.damage`, bypassing the type chart
/// (Dragon Rage = 40, Sonic Boom = 20). It draws NO rng byte.
#[test]
fn set_damage_const_writes_fixed_damage() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"dragonrage", kind:Move, hooks:[
            Hook(on:"ModifyDamage", do:[ SetDamage(value:Const(40)) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    let mut rng = ScriptedRng::new(vec![]);
    h.fire(Event::ModifyDamage, RelayVar::Unit, &mut rng);
    assert_eq!(h.mv_damage(), 40, "SetDamage(Const(40)) ⇒ mv.damage = 40");
    assert_eq!(rng.consumed(), 0, "Const draws no rng byte");
}

/// `SetDamage(UserLevel)` writes the SOURCE's level (Seismic Toss / Night Shade).
/// Source = PLAYER at level 50.
#[test]
fn set_damage_user_level() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"seismic", kind:Move, hooks:[
            Hook(on:"ModifyDamage", do:[ SetDamage(value:UserLevel, of:Source) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    h.set_levels(50, 99); // source(player)=50 ⇒ damage 50; foe level irrelevant
    let mut rng = ScriptedRng::new(vec![]);
    h.fire(Event::ModifyDamage, RelayVar::Unit, &mut rng);
    assert_eq!(
        h.mv_damage(),
        50,
        "SetDamage(UserLevel) ⇒ mv.damage = source level (50)"
    );
}

/// `SetDamage(RngScaledLevel)` draws EXACTLY ONE rng byte and writes
/// `byte * num/den * level / 256` (the Psywave shape). byte=255, num=3, den=2,
/// level=50 ⇒ 255*3/2*50/256 = 382*50/256 = 19100/256 = 74.
#[test]
fn set_damage_rng_scaled_level_draws_one_byte() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"psywave", kind:Move, hooks:[
            Hook(on:"ModifyDamage", do:[ SetDamage(value:RngScaledLevel(num:3, den:2), of:Source) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    h.set_levels(50, 50);
    let mut rng = ScriptedRng::new(vec![255]);
    h.fire(Event::ModifyDamage, RelayVar::Unit, &mut rng);
    // 255 * 3 / 2 = 382; 382 * 50 = 19100; 19100 / 256 = 74.
    assert_eq!(h.mv_damage(), 74, "Psywave-shape: 255*3/2*50/256 = 74");
    assert_eq!(rng.consumed(), 1, "RngScaledLevel draws EXACTLY one byte");
}

/// `DamageCurrentHpFraction(1/2, Target)` deals half the target's CURRENT HP
/// (Super Fang), floored at 1, AND writes `ctx.mv.damage`. 80 cur ⇒ 40 dealt.
#[test]
fn damage_current_hp_fraction_halves_current_hp() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"superfang", kind:Move, hooks:[
            Hook(on:"DamagingHit", do:[ DamageCurrentHpFraction(num:1, den:2, target:Target) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    h.set_opp_hp(80);
    let mut rng = ScriptedRng::new(vec![]);
    h.fire(Event::DamagingHit, RelayVar::Unit, &mut rng);
    assert_eq!(h.opp_hp(), 40, "Super Fang: 80 cur → 40 dealt → 40 left");
    assert_eq!(h.mv_damage(), 40, "Super Fang writes mv.damage = 40");
}

/// Super Fang floors a non-zero result at 1: 1 cur HP ⇒ 1/2 = 0, floored to 1 ⇒
/// the target faints (the legacy `(curHP/2).max(1)`).
#[test]
fn damage_current_hp_fraction_floors_at_one() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"superfang", kind:Move, hooks:[
            Hook(on:"DamagingHit", do:[ DamageCurrentHpFraction(num:1, den:2, target:Target) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    h.set_opp_hp(1);
    let mut rng = ScriptedRng::new(vec![]);
    h.fire(Event::DamagingHit, RelayVar::Unit, &mut rng);
    assert_eq!(h.opp_hp(), 0, "1 cur → max(1) → faints");
    assert_eq!(h.mv_damage(), 1, "floored damage is 1");
}

// ═════════════════════════════════════════════════════════════════════════════
// P4 (additive, inert) — SelfHpBelow / SourceHasStatus predicates + RemoveStatus
//     op (the wuxia 血越低攻越高 / 眩晕控制 / 驱散 gaps). Each is proved ONCE through
//     the real `run_event` fold; existing variants untouched.
// ═════════════════════════════════════════════════════════════════════════════

/// `ScaleRelay(when: [SelfHpBelow])` scales the relay ONLY when the SOURCE (the
/// acting battler = PLAYER) HP fraction is strictly below num/den. The wuxia
/// 「血越低攻越高」 outgoing-damage gate. 100/100 max ⇒ not below 1/2 (no scale);
/// 40/100 ⇒ below 1/2 (×3/2). Pure read; no rng.
#[test]
fn self_hp_below_gates_scale_on_source_hp() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"berserk", kind:Ability, hooks:[
            Hook(on:"ModifyDamage", do:[
                ScaleRelay(num:3, den:2, when:[ SelfHpBelow(num:1, den:2) ]) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    // Source (PLAYER) at full HP (100/100) ⇒ 100*2 < 100*1 is false ⇒ no scale.
    let mut full = Harness::new(MType::Normal, MType::Normal);
    let mut rng = ScriptedRng::new(vec![]);
    let out = full.fire(Event::ModifyDamage, RelayVar::Damage(50), &mut rng);
    assert_eq!(
        out,
        RelayVar::Damage(50),
        "full HP ⇒ not below 1/2 ⇒ unchanged"
    );
    // Source (PLAYER) at 40/100 ⇒ 40*2=80 < 100*1=100 true ⇒ ×3/2: 50 → 75.
    let mut low = Harness::new(MType::Normal, MType::Normal);
    low.state.player_battlers[0].hp = 40;
    let out2 = low.fire(Event::ModifyDamage, RelayVar::Damage(50), &mut rng);
    assert_eq!(out2, RelayVar::Damage(75), "HP 40<50% ⇒ ×3/2 ⇒ 75");
    assert_eq!(rng.consumed(), 0, "SelfHpBelow is a pure ctx read, no rng");
}

/// `VetoIf(SourceHasStatus("sleep"))` on `BeforeMove` skips the HOLDER's OWN move
/// when the SOURCE (the acting battler = PLAYER) is afflicted — the wuxia 眩晕
/// control gate. Reuses the EXISTING `has_status` binding on the source.
#[test]
fn source_has_status_vetoes_holders_own_move() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"stunned", kind:Status, hooks:[
            Hook(on:"BeforeMove", do:[ VetoIf(cond:SourceHasStatus("sleep")) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    // Source (PLAYER) NOT afflicted ⇒ predicate false ⇒ the move proceeds.
    let mut awake = Harness::new(MType::Normal, MType::Normal);
    let mut rng = ScriptedRng::new(vec![]);
    let out = awake.fire(Event::BeforeMove, RelayVar::Bool(true), &mut rng);
    assert_eq!(out, RelayVar::Bool(true), "awake source ⇒ move proceeds");
    // Source (PLAYER) asleep ⇒ predicate true ⇒ Fail (the move is prevented).
    let mut asleep = Harness::new(MType::Normal, MType::Normal);
    asleep.state.player_battlers[0].status = Some(Status::Sleep);
    let out2 = asleep.fire(Event::BeforeMove, RelayVar::Bool(true), &mut rng);
    assert_eq!(
        out2,
        RelayVar::Bool(false),
        "asleep source ⇒ VetoIf fires ⇒ Fail"
    );
}

/// `SourceHasStatus` with an unknown status name fails at LOAD (its name is the
/// game's status vocabulary, validated exactly like `TargetHasStatus`).
#[test]
fn source_has_status_unknown_name_fails_at_load() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"x", kind:Status, hooks:[
            Hook(on:"BeforeMove", do:[ VetoIf(cond:SourceHasStatus("dazzle")) ]) ]) ],
    )"#;
    let err = compile_and_install(ron).unwrap_err();
    assert!(matches!(err, LoadError::UnknownStatus(_)), "got {err:?}");
}

/// `RemoveStatus(Target)` clears the target's non-volatile status (the wuxia
/// 驱散/cleanse op). Apply a status, fire it, assert status == None.
#[test]
fn remove_status_clears_target_status() {
    let ron = r#"Ruleset(
        effects: [ Effect(id:"cleanse", kind:Move, hooks:[
            Hook(on:"DamagingHit", do:[ RemoveStatus(target:Target) ]) ]) ],
    )"#;
    compile_and_install(ron).unwrap();
    let mut h = Harness::new(MType::Normal, MType::Normal);
    h.state.opponent_battlers[0].status = Some(Status::Poison);
    assert_eq!(h.opp_status(), Some(Status::Poison));
    let mut rng = ScriptedRng::new(vec![]);
    h.fire(Event::DamagingHit, RelayVar::Damage(10), &mut rng);
    assert_eq!(h.opp_status(), None, "RemoveStatus cleared the poison");
    assert_eq!(
        rng.consumed(),
        0,
        "RemoveStatus is a pure field write, no rng"
    );
}
