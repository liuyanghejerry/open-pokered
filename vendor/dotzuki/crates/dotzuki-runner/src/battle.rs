//! The generic, data-driven battle system for `dotzuki run` (parties + items).
//!
//! A project opts in with a top-level `battle` section in its
//! `.dotzuki-editor.json` (see [`crate::manifest::BattleSection`] and
//! `docs/reference/project-manifest.md`). Combatants are plain data-table records
//! (`<dataRoot>/<tableDir>/<id>.json`); skills are records of the skills
//! table; the optional rules file (dotzuki-rules `Ruleset` RON) contributes the
//! type chart and — when it declares `effects` — **RON effect hooks** (v2-a):
//! `kind: Move` records take over the matching skills and `kind: Status`
//! records define statuses, all executed through the engine's effect-stack
//! interpreter (see [`hooks`]). This module owns everything battle-side:
//! config resolution, record loading, the damage formula, the turn loop, and
//! the placeholder battle screen.
//!
//! # The standard formula (v1)
//!
//! Per damaging hit, all integer math:
//!
//! 1. `eff = raw × stage_mult` per stat — stage ∈ −4..=+4, ×(4+stage)/4 for
//!    positive stages, ×4/(4−stage) for negative (+1 = ×1.25, −1 = ×0.8).
//! 2. `base = power × eff_atk / max(1, eff_def)`.
//! 3. variance: `× (85 + rng%16) / 100` (one rng byte).
//! 4. crit: one rng byte; `rng % 16 == 0` (1/16) ⇒ ×3/2.
//! 5. effectiveness: ×`num`/`den` from the type chart (skill `element` vs the
//!    defender's `element` field; no edge ⇒ 1×).
//! 6. `damage = max(1, …)`.
//!
//! Accuracy: one rng byte; the hit lands iff `rng % 100 < accuracy`. Every
//! skill use consumes the accuracy byte first; damaging skills then consume
//! the variance and crit bytes (heal/buff/debuff consume only accuracy).
//!
//! # Parties (v2-b)
//!
//! The player's party is EVERY record of the party table (sorted by record
//! id; a 1-record party behaves like v1). The runner owns the persistent
//! party state — each member's current HP/MP/status survives between
//! battles (a member at 0 HP stays fainted until healed) — while base stats
//! are rebuilt from the records at every battle start. The first LIVING
//! member leads. The root menu offers **Fight** (the skill menu), **Party**
//! (switch to a living non-active member — consumes the player's turn), and
//! **Item** (when the manifest has an `items` block). When the active member
//! faints the player is FORCED to pick a replacement (a free action; the
//! enemy's deferred action then resolves against the new member); with no
//! living member left, the battle is lost. Stat stages reset on switch-in;
//! statuses persist with the member (the RON mirror is re-built from the
//! member's current state on switch).
//!
//! # Items (v2-b)
//!
//! With a manifest `items` block (`{ table, healField, starting }`), records
//! of the items table with a positive `healField` number are battle-usable:
//! the Item menu lists them while the inventory count is positive, and using
//! one heals the active member (capped at max), decrements the count, and
//! consumes the player's turn. The runner owns the inventory between battles
//! (initialized from `starting`). Free-text `effect` fields are display-only.
//!
//! # Turn loop (v1, kept for v2)
//!
//! The loop is intentionally **not** the engine's `StackDriver`: the stack
//! engine has no Switch/Item/Run surface. The loop is a tiny phase machine
//! instead: root menu → submenu → narration → won/lost. Per round the player
//! picks a skill (unaffordable MP costs are unselectable), the enemy AI picks
//! its highest-power affordable skill (fallback: its first affordable skill,
//! else the built-in Attack); the faster side (eff speed) acts first, ties go
//! to the player; each action re-checks the MP gate, rolls accuracy, resolves
//! the skill and narrates. Switch/Item rounds act player-first.
//!
//! # RON effect hooks (v2-a)
//!
//! When the rules file declares `effects`, a `kind: Move` record whose `id`
//! matches a skill id **takes over that skill**: its `power`/`type`/
//! `accuracy`/`cost` fields override the table record (absent fields fall
//! back), and the action runs through the stack interpreter instead of the
//! built-in category behavior — MP gate → accuracy → damage precompute (the
//! v1 formula → `ctx.mv.damage`) → `BeforeMove` gate (if subscribed) →
//! `ModifyDamage` → `Effectiveness` → `Damage` → `DamagingHit` → `AfterMove`
//! (the minimon/wuxia fire order). When the record subscribes to
//! `Effectiveness`, the hooks own the scaling (author `ApplyTypeChart` for
//! the chart); otherwise the v1 direct chart application applies in the
//! precompute. A `kind: Status` record defines a status for
//! `InflictStatus{status}` ops; its `Residual` hooks run after the afflicted
//! combatant's action. Skills with NO matching RON record keep the v1
//! built-in category behavior byte-for-byte (full backwards compat). The
//! engine mirrors track the ACTIVE member — re-built on switch-in (stages
//! reset, status carried, the old battler's volatiles dropped).
//!
//! # EXP & levels (v2-c)
//!
//! With an optional `battle.levels` manifest block (all keys optional:
//! `{ expField: "exp", levelField: "level", curve: { base: 8, exponent: 3 },
//! growth: 0.05, maxLevel: 100 }`), a combatant's effective stat is
//! `floor(raw × (1 + growth × (level − 1)))` — applied wherever raw record
//! stats are read (battle build, the RON mirror rides the same values, the
//! menu Party view), with the level coming from the record's `levelField`
//! (default 1 ⇒ ×1, numerically identical to v1). On a win each NON-fainted
//! party member gains the enemy's `expField` value (0 when absent), then
//! levels up while `exp >= exp_to_next(level)` and `level < maxLevel`
//! (`exp_to_next(L) = curve.base × L^curve.exponent`); a level-up recomputes
//! the member's stats and heals the max-HP/MP DELTAS into the current pools.
//! Per-member `level` + `exp` ride the persistent party state and the save.
//! Without the block nothing changes: no EXP narration, no growth.
//!
//! # Encounters, trainer battles & Run (v2-d)
//!
//! An optional `battle.encounters` block (`{ "table": "encounters" }`) names
//! an ENCOUNTER table whose records describe enemy parties:
//! `{ "id", "name", "enemies": ["slime", "bat"], "trainer": true, "money": 80 }`.
//! `startBattle("x")` resolves in this order: an encounter record `x` (when
//! the block is set) → a single enemy record `x` (implicitly wild, v1
//! behavior) → the first enemy record + a warning. Unknown enemy ids INSIDE
//! an encounter record are a clear error at battle start (and a `dotzuki check`
//! schema diagnostic covers the block itself). In an encounter battle the
//! enemy side is a QUEUE: the active enemy faints → the next is sent out
//! (narrated `"Foe sent out Bat!"`, a fresh combatant with its own stats and
//! no status; the RON mirror is rebuilt and its volatiles dropped) — the
//! round then ends. The battle is won when the queue empties; the EXP award
//! is the SUM of every defeated enemy's `expField`. A trainer encounter
//! (`trainer: true`, default false) pays its `money` (default 0) on a win
//! (narrated `"Got 80 G for winning!"`) and BLOCKS the Run action.
//!
//! The root menu's **Run** entry (Fight/Party/Item/Run) ends a WILD battle
//! on the spot — narration `"Got away safely!"`, outcome `"run"` (no
//! EXP/money; the party state carries over as after any battle) — and is
//! blocked in trainer battles (`"Can't escape from a trainer battle!"`, the
//! turn NOT consumed). Scenes branching on `result == "win"` treat `"run"`
//! as not-won (the third outcome string).
//!
//! # Abilities, held items & weather (v2-e)
//!
//! The remaining RON kinds are live. A combatant record's optional `ability`
//! field names a `kind: Ability` record; its `SwitchIn` hooks fire at battle
//! start and on every switch-in of the ACTIVE combatant (benched members'
//! abilities are inert), narrated with an intro line (`"Aria's
//! Intimidate!"`), and its hooks also join the acting combatant's per-action
//! event sequence (an ability hooking `ModifyDamage` fires alongside the
//! skill's hooks). A record's optional `heldItem` field names a `kind: Item`
//! record: its hooks fire the same way, with `Residual` hooks running after
//! each of the holder's actions (Leftovers-style heal). Held items are
//! persistent flags — nothing consumes them (berries are out of scope). A
//! `kind: Weather` record's `FieldResidual` hooks fire on each combatant's
//! residual while the weather is active; a scene arms the weather with
//! `setWeather("sandstorm")` / clears it with `clearWeather()` before
//! `startBattle` — the weather is battle-local (narrated at battle start,
//! dropped at battle end, never saved).
//!
//! # Remaining limits (documented in the spec)
//!
//! Volatiles are basic (arena + `HasVolatile` only); HP/MP clamp into the
//! engine's `u16` pools for RON skills; items only heal (no status cures /
//! battle-only effects) and held items are never consumed; a defender's
//! ability/held-item hooks do not join the ATTACKER's per-action sequence
//! (they fire on their own switch-in/residual only); weather is armed from
//! scenes, not by in-battle ops. A lost battle returns `"lose"` to the scene
//! and then triggers the runner's game-over whiteout (see `game::menu`).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use anyhow::{Context, Result};
use dotzuki_engine::battle::stack::{collect_handlers, run_event, BattleCtx, Event, RelayVar};
use dotzuki_engine::battle::{BattleState, BattlerRef};
use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{FrameBuffer, Rgba, TileRect, Ui};
use dotzuki_renderer::embedded_font;
use dotzuki_renderer::input::{GbButton, InputState};
use dotzuki_ui::widgets::flex_menu::{draw_flex_menu, FlexMenuState};
use dotzuki_ui::FrameBufferPainter;

use crate::game::{draw_textbox, DIALOG_AREA, SCREEN_H, SCREEN_W};
use crate::manifest::{BattleLevels, BattleStats, DEFAULT_RULES_FILE};
use crate::project::LoadedProject;
use crate::vfs::{join_path, ProjectFiles};

use dotzuki_rules::RulesProvider;
use hooks::{GenericProvider, HookState};

pub mod hooks;

/// The built-in fallback skill every combatant can use (no cost).
pub const BASIC_ATTACK_NAME: &str = "Attack";
/// Built-in Attack power.
pub const BASIC_ATTACK_POWER: u32 = 40;
/// Built-in Attack accuracy.
pub const BASIC_ATTACK_ACCURACY: u32 = 100;
/// Stat stages clamp to −4..=+4.
const MAX_STAGE: i8 = 4;

// ── rng ─────────────────────────────────────────────────────────────────────

/// The battle's entropy source: a byte stream. Accuracy, variance and crit
/// all fold bytes, so a scripted stream replays a battle exactly (tests).
pub trait BattleRng {
    /// Draw the next byte.
    fn byte(&mut self) -> u8;
}

/// Production rng: a xorshift64\* PRNG seeded per battle.
pub struct XorshiftRng(u64);

impl XorshiftRng {
    /// Seed a generator (a zero seed is remapped — xorshift sticks at 0).
    pub fn seed(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }
}

impl BattleRng for XorshiftRng {
    fn byte(&mut self) -> u8 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 56) as u8
    }
}

/// Deterministic byte-scripted rng (tests / `RunnerOptions::rng_script`).
/// Bytes are consumed in order, cycling back to the start on exhaustion, so a
/// short script can drive an arbitrarily long battle. An empty script yields
/// zeros (always hits, min variance, always crits).
pub struct ScriptedRng {
    bytes: Vec<u8>,
    idx: usize,
}

impl ScriptedRng {
    /// A scripted stream cycling over `bytes`.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, idx: 0 }
    }
}

impl BattleRng for ScriptedRng {
    fn byte(&mut self) -> u8 {
        if self.bytes.is_empty() {
            return 0;
        }
        let b = self.bytes[self.idx];
        self.idx = (self.idx + 1) % self.bytes.len();
        b
    }
}

// ── type chart ──────────────────────────────────────────────────────────────

/// The `(attacking element, defending element) → [num, den]` relation parsed
/// from the rules file's `type_chart`. Element names match case-insensitively;
/// missing pairs are neutral (1×).
#[derive(Debug, Default, Clone)]
pub struct TypeChart {
    edges: HashMap<(String, String), (u32, u32)>,
}

impl TypeChart {
    /// Build a chart from a parsed dotzuki-rules [`dotzuki_rules::Ruleset`].
    pub fn from_ruleset(ruleset: &dotzuki_rules::Ruleset) -> Self {
        let mut edges = HashMap::new();
        for entry in &ruleset.type_chart {
            edges.insert(
                (entry.atk.to_lowercase(), entry.def.to_lowercase()),
                (entry.mult.num, entry.mult.den.max(1)),
            );
        }
        Self { edges }
    }

    /// The `[num, den]` multiplier for an attack of element `atk` against a
    /// defender of element `def`; `(1, 1)` when either side is untyped or the
    /// pair has no edge.
    pub fn mult(&self, atk: Option<&str>, def: Option<&str>) -> (u32, u32) {
        let (Some(atk), Some(def)) = (atk, def) else {
            return (1, 1);
        };
        self.edges
            .get(&(atk.to_lowercase(), def.to_lowercase()))
            .copied()
            .unwrap_or((1, 1))
    }
}

// ── skills ──────────────────────────────────────────────────────────────────

/// What a skill does when it connects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillCategory {
    /// Deal `power`-based damage to the target.
    Damage,
    /// Restore the user's HP by `power` (capped at max).
    Heal,
    /// Raise one of the user's stat stages by 1.
    Buff,
    /// Lower one of the target's stat stages by 1.
    Debuff,
}

/// A usable skill (a record of the skills table, or the built-in Attack).
#[derive(Debug, Clone)]
pub struct Skill {
    /// Record id (`"basic"` for the built-in Attack).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Base power (damage, or heal amount); 0 for pure stage skills.
    pub power: u32,
    /// Accuracy percent (hit iff `rng % 100 < accuracy`).
    pub accuracy: u32,
    /// Optional attacking element (type-chart lookups).
    pub element: Option<String>,
    /// What the skill does (v1 built-in behavior; unused when `ron` is set).
    pub category: SkillCategory,
    /// Stat key (`"attack"` / `"defense"` / `"speed"` / `"hp"`) a buff/debuff
    /// moves; default `"attack"`.
    pub stat: String,
    /// Resource (MP) cost.
    pub cost: u32,
    /// Whether a `kind: Move` RON record took this skill over (its hooks run
    /// through the stack interpreter instead of the built-in category).
    pub ron: bool,
}

/// The built-in basic Attack every combatant falls back to.
pub fn basic_attack() -> Skill {
    Skill {
        id: "basic".to_string(),
        name: BASIC_ATTACK_NAME.to_string(),
        power: BASIC_ATTACK_POWER,
        accuracy: BASIC_ATTACK_ACCURACY,
        element: None,
        category: SkillCategory::Damage,
        stat: "attack".to_string(),
        cost: 0,
        ron: false,
    }
}

/// Parse a skill record. `category` comes from the configured category field
/// (case-insensitive): `attack`/`damage` → Damage, `heal` → Heal, `buff` →
/// Buff, `debuff` → Debuff; anything unrecognized → Damage. A matching
/// `kind: Move` RON record then overrides `power`/`accuracy`/`element`/`cost`
/// and marks the skill RON-driven.
fn skill_from_record(id: &str, record: &serde_json::Value, setup: &BattleSetup) -> Skill {
    let category = match get_str(record, &setup.category_field)
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "heal" => SkillCategory::Heal,
        "buff" => SkillCategory::Buff,
        "debuff" => SkillCategory::Debuff,
        // "attack" | "damage" | unrecognized → damage.
        _ => SkillCategory::Damage,
    };
    let stat = normalize_stat_key(get_str(record, "stat").unwrap_or("attack"));
    let mut skill = Skill {
        id: id.to_string(),
        name: get_str(record, "name").unwrap_or(id).to_string(),
        power: get_num(record, "power").unwrap_or(0),
        accuracy: get_num(record, "accuracy").unwrap_or(100),
        element: get_str(record, "element").map(str::to_string),
        category,
        stat,
        cost: get_num(record, &setup.cost_field).unwrap_or(0),
        ron: false,
    };
    if let Some(ron_move) = setup.ron_move(id) {
        skill.ron = true;
        if let Some(power) = ron_move.power {
            skill.power = power;
        }
        if let Some(accuracy) = ron_move.accuracy {
            skill.accuracy = accuracy;
        }
        if let Some(mtype) = &ron_move.mtype {
            skill.element = Some(mtype.clone());
        }
        if let Some(cost) = ron_move.cost {
            skill.cost = cost;
        }
    }
    skill
}

/// Map an arbitrary `stat` field value onto a known stat key.
fn normalize_stat_key(stat: &str) -> String {
    match stat.to_lowercase().as_str() {
        "hp" => "hp",
        "defense" | "def" => "defense",
        "speed" | "spd" => "speed",
        // "attack" | "atk" | unknown → attack.
        _ => "attack",
    }
    .to_string()
}

/// Display label for a stat key ("Aria's Attack rose!").
fn stat_label(stat: &str) -> &str {
    match stat {
        "hp" => "HP",
        "defense" => "Defense",
        "speed" => "Speed",
        _ => "Attack",
    }
}

// ── combatants ──────────────────────────────────────────────────────────────

/// Stat stages (−4..=+4) for the four stat roles. `hp` is tracked for
/// completeness but unused by the v1 formula.
#[derive(Debug, Clone, Default)]
pub struct Stages {
    /// HP stage (unused by the v1 formula).
    pub hp: i8,
    /// Attack stage.
    pub attack: i8,
    /// Defense stage.
    pub defense: i8,
    /// Speed stage (affects turn order).
    pub speed: i8,
}

impl Stages {
    fn bump(&mut self, stat: &str, delta: i8) {
        let stage = match stat {
            "hp" => &mut self.hp,
            "defense" => &mut self.defense,
            "speed" => &mut self.speed,
            _ => &mut self.attack,
        };
        *stage = (*stage + delta).clamp(-MAX_STAGE, MAX_STAGE);
    }

    /// The stage of a stat (RON stat names accepted — aliases normalize).
    pub fn get(&self, stat: &str) -> i8 {
        match normalize_stat_key(stat).as_str() {
            "hp" => self.hp,
            "defense" => self.defense,
            "speed" => self.speed,
            _ => self.attack,
        }
    }

    /// Set a stat's stage absolutely (clamped to ±4; the mirror sync-back).
    pub fn set(&mut self, stat: &str, value: i8) {
        let value = value.clamp(-MAX_STAGE, MAX_STAGE);
        match normalize_stat_key(stat).as_str() {
            "hp" => self.hp = value,
            "defense" => self.defense = value,
            "speed" => self.speed = value,
            _ => self.attack = value,
        }
    }
}

/// The stage multiplier: ×(4+stage)/4 above 0, ×4/(4−stage) below (+1 =
/// ×1.25, −1 = ×0.8). Stages clamp to ±4.
pub fn stage_multiplier(raw: u32, stage: i8) -> u32 {
    match stage.clamp(-MAX_STAGE, MAX_STAGE) {
        s if s >= 0 => raw * (4 + s as u32) / 4,
        s => raw * 4 / (4 - s) as u32,
    }
}

/// A combatant's raw record stats, before the level-growth multiplier
/// (v2-c). Equal to the effective stats when levels are off or the level is
/// 1; the level-up recompute reads these.
#[derive(Debug, Clone, Copy)]
pub struct BaseStats {
    /// Raw max HP.
    pub max_hp: u32,
    /// Raw max resource.
    pub max_mp: u32,
    /// Raw attack.
    pub attack: u32,
    /// Raw defense.
    pub defense: u32,
    /// Raw speed.
    pub speed: u32,
}

/// The level-growth multiplier (v2-c): `floor(raw × (1 + growth × (level −
/// 1)))`. Level 1 (or 0) is always ×1 — records without a level field are
/// numerically identical to v1.
pub fn growth_stat(raw: u32, level: u8, growth: f64) -> u32 {
    let mult = 1.0 + growth * f64::from(level.saturating_sub(1));
    // The tiny epsilon keeps exact products (e.g. ×1.05 of 100) from
    // flooring one short on float representation error.
    ((raw as f64 * mult) + 1e-9).floor().max(0.0) as u32
}

/// The exp curve (v2-c): `exp_to_next(L) = base × L^exponent` (integer,
/// saturating).
pub fn exp_to_next(base: u32, exponent: u32, level: u8) -> u32 {
    u64::from(base)
        .saturating_mul(u64::from(level).saturating_pow(exponent))
        .min(u64::from(u32::MAX)) as u32
}

/// A live combatant: one data-table record plus per-battle state (HP/MP
/// pools, stat stages). Rebuilt from its record for every battle (base stats
/// always fresh); the runner re-applies the persistent party state (current
/// HP/MP/status, v2-b; level/exp, v2-c) on top.
#[derive(Debug, Clone)]
pub struct Combatant {
    /// Record id (filename stem).
    pub id: String,
    /// Display name (`name` field, else the id).
    pub name: String,
    /// Optional element (`element` field) — the defending side of chart lookups.
    pub element: Option<String>,
    /// HP pool.
    pub hp: u32,
    /// Max HP (growth-applied when the levels block is present).
    pub max_hp: u32,
    /// Attack (growth-applied when the levels block is present).
    pub attack: u32,
    /// Defense (growth-applied when the levels block is present).
    pub defense: u32,
    /// Speed (growth-applied when the levels block is present).
    pub speed: u32,
    /// The record's level field (default 1; read by level-based RON ops
    /// and the `LevelGE` predicate). With a `levels` block it also drives
    /// the stat-growth multiplier and level-ups; the v1 formula ignores it.
    pub level: u8,
    /// Raw record stats (pre-growth), for the level-up recompute.
    pub base: BaseStats,
    /// EXP progress toward the next level (v2-c; persists on party members).
    pub exp: u32,
    /// The EXP this combatant awards when defeated (its `expField`; 0 when
    /// the levels block is absent or the field missing).
    pub exp_reward: u32,
    /// Stat stages (reset when the combatant switches in, v2-b).
    pub stages: Stages,
    /// Resource pool (0 when the manifest maps no `resource` field).
    pub mp: u32,
    /// Max resource.
    pub max_mp: u32,
    /// Move list (never empty: falls back to the built-in Attack).
    pub skills: Vec<Skill>,
    /// The persistent non-volatile status (a `kind: Status` RON record id),
    /// carried between battles by the party state (v2-b).
    pub status: Option<String>,
    /// The combatant's ability: a `kind: Ability` RON record id (the record's
    /// `ability` field; v2-e). Fires at battle start / switch-in (`SwitchIn`)
    /// and joins the ACTIVE combatant's per-action event sequence. Benched
    /// members' abilities are inert.
    pub ability: Option<String>,
    /// The combatant's held item: a `kind: Item` RON record id (the record's
    /// `heldItem` field; v2-e). Fires like an ability — its `Residual` hooks
    /// run after each of the holder's actions (Leftovers-style). Items are
    /// persistent flags: nothing consumes them (no berries).
    pub held_item: Option<String>,
}

impl Combatant {
    /// Effective attack (stat × stage multiplier).
    pub fn eff_attack(&self) -> u32 {
        stage_multiplier(self.attack, self.stages.attack)
    }
    /// Effective defense.
    pub fn eff_defense(&self) -> u32 {
        stage_multiplier(self.defense, self.stages.defense)
    }
    /// Effective speed (turn order).
    pub fn eff_speed(&self) -> u32 {
        stage_multiplier(self.speed, self.stages.speed)
    }

    /// Recompute every effective stat from the raw record stats at the
    /// current level (v2-c level-up): max HP/MP move with the growth
    /// multiplier and the DELTAS heal into the current pools.
    pub fn recompute_stats(&mut self, levels: &LevelsSetup) {
        let grown = |raw: u32| growth_stat(raw, self.level, levels.growth);
        let (old_hp, old_mp) = (self.max_hp, self.max_mp);
        self.max_hp = grown(self.base.max_hp);
        self.max_mp = grown(self.base.max_mp);
        self.attack = grown(self.base.attack);
        self.defense = grown(self.base.defense);
        self.speed = grown(self.base.speed);
        self.hp = (self.hp + self.max_hp.saturating_sub(old_hp)).min(self.max_hp);
        self.mp = (self.mp + self.max_mp.saturating_sub(old_mp)).min(self.max_mp);
    }
}

// ── persistent party state + items (v2-b) ───────────────────────────────────

/// A party member's persistent state, owned by the runner between battles:
/// current HP/MP, status, and (v2-c) level/exp. Base stats are rebuilt from
/// the record at every battle start; these pools carry over (a member at 0
/// HP stays fainted until healed).
#[derive(Debug, Clone)]
pub struct PartyMemberState {
    /// Party record id.
    pub id: String,
    /// Current HP (0 = fainted).
    pub hp: u32,
    /// Current MP (resource pool).
    pub mp: u32,
    /// The persistent status (a `kind: Status` RON record id), if any.
    pub status: Option<String>,
    /// Current level (v2-c; 1 when levels are off or the save predates them).
    pub level: u8,
    /// EXP progress toward the next level (v2-c).
    pub exp: u32,
}

/// A battle-usable item: a record of the items table whose heal field holds
/// a positive number. Free-text `effect` fields are display-only.
#[derive(Debug, Clone)]
pub struct BattleItem {
    /// Record id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// HP restored on use (capped at max).
    pub heal: u32,
}

// ── setup (manifest + rules resolution) ─────────────────────────────────────

/// The resolved battle configuration: record directories, field mapping and
/// the type chart. Built lazily on the first `startBattle` (projects without
/// a `battle` section never pay for it, and projects that never battle never
/// fail on a broken section).
pub struct BattleSetup {
    /// The project file backend every record read goes through.
    files: Arc<dyn ProjectFiles>,
    /// Record dirs are project-relative POSIX paths (VFS key prefixes).
    party_dir: String,
    enemies_dir: String,
    /// The encounters table's record dir (v2-d): `None` when the manifest
    /// has no `encounters` block (every battle is a single wild enemy).
    encounters_dir: Option<String>,
    skills_dir: Option<String>,
    skills_field: String,
    category_field: String,
    cost_field: String,
    stats: BattleStats,
    resource: Option<String>,
    chart: TypeChart,
    ron: Option<RonSetup>,
    /// Battle-usable items (v2-b): `None` when the manifest has no `items`
    /// block (no Item menu).
    items: Option<ItemsSetup>,
    /// EXP/level growth (v2-c): `None` when the manifest has no `levels`
    /// block (v1 behavior — no EXP, stats never grow).
    levels: Option<LevelsSetup>,
}

/// The resolved levels half of a [`BattleSetup`] (v2-c).
#[derive(Debug, Clone)]
pub struct LevelsSetup {
    /// Enemy record field holding the EXP reward.
    pub exp_field: String,
    /// Combatant record field holding its starting level.
    pub level_field: String,
    /// Curve base (`exp_to_next(L) = base × L^exponent`).
    pub curve_base: u32,
    /// Curve exponent.
    pub curve_exponent: u32,
    /// Stat growth per level above 1 (0.05 ⇒ +5% per level).
    pub growth: f64,
    /// Level cap.
    pub max_level: u8,
}

impl LevelsSetup {
    /// Resolve the manifest `levels` block.
    fn from_manifest(levels: &BattleLevels) -> Self {
        Self {
            exp_field: levels.exp_field.clone(),
            level_field: levels.level_field.clone(),
            curve_base: levels.curve.base,
            curve_exponent: levels.curve.exponent,
            growth: levels.growth,
            max_level: levels.max_level.clamp(1, 255) as u8,
        }
    }

    /// EXP needed to advance from `level` (`base × level^exponent`).
    pub fn exp_to_next(&self, level: u8) -> u32 {
        exp_to_next(self.curve_base, self.curve_exponent, level)
    }

    /// The growth multiplier applied to a raw stat at `level`.
    pub fn growth(&self, raw: u32, level: u8) -> u32 {
        growth_stat(raw, level, self.growth)
    }
}

/// The resolved items half of a [`BattleSetup`] (v2-b).
struct ItemsSetup {
    /// Records directory of the items table (project-relative POSIX path).
    dir: String,
    /// The record field holding the heal amount (usability gate).
    heal_field: String,
    /// The starting inventory (record id → count).
    starting: HashMap<String, u32>,
}

/// A parsed encounter record (v2-d): an ordered enemy party plus the
/// trainer flag and money reward.
struct Encounter {
    /// Display name (log/diagnostics only — battles narrate the enemies).
    name: String,
    /// Ordered enemy-table record ids (validated non-empty and known).
    enemy_ids: Vec<String>,
    /// Whether this is a trainer battle (blocks Run; pays `money` on a win).
    trainer: bool,
    /// The money reward on a win (0 default; paid only when `trainer`).
    money: u32,
}

/// The compiled RON-hooks half of a [`BattleSetup`] (v2-a): present when the
/// rules file declares `effects`. Built once per setup; the thread-local
/// `RulesHost` is (re-)installed from it on every battle start.
struct RonSetup {
    /// The compiled registry (hooks + interned vocabularies + chart).
    compiled: dotzuki_rules::CompiledRuleset,
    /// One leaked `Effect` per compiled hook (the deliberate one-time leak,
    /// minimon/wuxia precedent).
    registry: Vec<&'static dotzuki_engine::battle::stack::Effect<GenericProvider>>,
    /// Skill id → its `kind: Move` RON record's overrides.
    move_records: HashMap<String, hooks::RonMove>,
    /// `StatusId(idx)` → status record id (declaration order).
    status_names: Vec<String>,
    /// The ruleset's interned stat names, in order.
    stat_names: Vec<String>,
    /// Whether the manifest maps a resource field (the MP pool mirror).
    has_resource: bool,
}

impl BattleSetup {
    /// Resolve the manifest's `battle` section against the project's data
    /// tables and rules file.
    ///
    /// # Errors
    ///
    /// Fails when the section is absent, a referenced table id names no
    /// declared data table, or the rules file exists but does not parse.
    pub fn from_project(project: &LoadedProject) -> Result<Self> {
        let section = project
            .manifest()
            .battle
            .as_ref()
            .context("manifest has no battle section")?;

        let table_dir =
            |reference: Option<&crate::manifest::BattleTableRef>, what: &str| -> Result<String> {
                let id = reference.map(|r| r.table.as_str()).with_context(|| {
                    format!("battle.{what}.table is required when a battle starts")
                })?;
                project.table_dir_rel(id).with_context(|| {
                    format!("battle.{what}.table '{id}' is not a declared data table")
                })
            };
        let party_dir = table_dir(section.party.as_ref(), "party")?;
        let enemies_dir = table_dir(section.enemies.as_ref(), "enemies")?;
        let encounters_dir = match &section.encounters {
            Some(encounters) => {
                Some(project.table_dir_rel(&encounters.table).with_context(|| {
                    format!(
                        "battle.encounters.table '{}' is not a declared data table",
                        encounters.table
                    )
                })?)
            }
            None => None,
        };
        let skills_dir = match &section.skills {
            Some(skills) => Some(project.table_dir_rel(&skills.table).with_context(|| {
                format!(
                    "battle.skills.table '{}' is not a declared data table",
                    skills.table
                )
            })?),
            None => None,
        };
        let items = match &section.items {
            Some(items) => Some(ItemsSetup {
                dir: project.table_dir_rel(&items.table).with_context(|| {
                    format!(
                        "battle.items.table '{}' is not a declared data table",
                        items.table
                    )
                })?,
                heal_field: items.heal_field.clone(),
                starting: items.starting.clone(),
            }),
            None => None,
        };

        let (skills_field, category_field, cost_field) = match &section.skills {
            Some(s) => (
                s.field.clone(),
                s.category_field.clone(),
                s.cost_field.clone(),
            ),
            None => (
                crate::manifest::DEFAULT_SKILLS_FIELD.to_string(),
                crate::manifest::DEFAULT_CATEGORY_FIELD.to_string(),
                crate::manifest::DEFAULT_COST_FIELD.to_string(),
            ),
        };

        // The rules file contributes the type chart and — when it declares
        // `effects` — the compiled RON hook registry (v2-a). It is parsed
        // only when it exists; a malformed registry is a boot-time
        // error at battle start (and a `dotzuki check` diagnostic).
        let rules_rel = section.rules.as_deref().unwrap_or(DEFAULT_RULES_FILE);
        let rules_rel = join_path("", rules_rel);
        let mut ron = None;
        let chart = match project.files().read(&rules_rel) {
            Ok(bytes) => {
                let text = String::from_utf8(bytes)
                    .with_context(|| format!("{rules_rel} is not UTF-8"))?;
                let ruleset = dotzuki_rules::Ruleset::from_ron(&text)
                    .map_err(|e| anyhow::anyhow!("failed to parse {rules_rel}: {e}"))?;
                let chart = TypeChart::from_ruleset(&ruleset);
                if !ruleset.effects.is_empty() {
                    let compiled = hooks::compile_ruleset(&ruleset)
                        .map_err(|e| anyhow::anyhow!("failed to compile {rules_rel}: {e}"))?;
                    let registry = compiled.build_effects::<GenericProvider>();
                    let move_records = hooks::ron_moves(&ruleset, section.resource.as_deref());
                    ron = Some(RonSetup {
                        compiled,
                        registry,
                        move_records,
                        status_names: hooks::status_names(&ruleset),
                        stat_names: ruleset.stats.clone(),
                        has_resource: section.resource.is_some(),
                    });
                }
                chart
            }
            Err(e) => {
                // An absent default file (battle.rules unset) is a legal
                // project — no type chart. A manifest-named file that can't
                // be read is a configuration slip: warn loudly so the chart's
                // silent absence is visible (`dotzuki check` errors on it).
                if section.rules.is_some() {
                    log::warn!(
                        "battle.rules '{rules_rel}' could not be read ({e:#}); battles run with an empty type chart"
                    );
                }
                TypeChart::default()
            }
        };

        Ok(Self {
            files: Arc::clone(project.files()),
            party_dir,
            enemies_dir,
            encounters_dir,
            skills_dir,
            skills_field,
            category_field,
            cost_field,
            stats: section.stats.clone().unwrap_or_default(),
            resource: section.resource.clone(),
            chart,
            ron,
            items,
            levels: section.levels.as_ref().map(LevelsSetup::from_manifest),
        })
    }

    /// Build a battle with a FRESH party state (every member at full HP/MP,
    /// no status) and the manifest's starting inventory. Convenience for
    /// tests and tools; the runner calls [`start_with`](Self::start_with).
    ///
    /// # Errors
    ///
    /// Fails when the party table holds no readable record.
    pub fn start(&self, enemy_id: &str, rng: Box<dyn BattleRng>) -> Result<Battle> {
        self.start_with(enemy_id, rng, None, None)
    }

    /// Build a battle: the WHOLE party table (sorted by record id) against
    /// `enemy_id`. Resolution order (v2-d): when the manifest has an
    /// `encounters` block AND `enemy_id` names an encounter record, the
    /// enemy side is that record's ordered enemy party (a queue) with its
    /// trainer flag and money reward; otherwise `enemy_id` names an enemy
    /// record (a single implicitly-wild enemy); an id in NEITHER table falls
    /// back to the first enemy record with a warning. `party_state`
    /// (runner-owned, from the previous battle or a save) re-applies current
    /// HP/MP/status per member id — base stats always come fresh from the
    /// records; `inventory` overrides the manifest's `items.starting`
    /// counts. The first LIVING member leads; a party with no living member
    /// arms an immediate loss.
    ///
    /// # Errors
    ///
    /// Fails when the party table holds no readable record, or when the
    /// encounter record is malformed (no `enemies` list, or an enemy id in
    /// it that names no enemy record).
    pub fn start_with(
        &self,
        enemy_id: &str,
        rng: Box<dyn BattleRng>,
        party_state: Option<&[PartyMemberState]>,
        inventory: Option<&HashMap<String, u32>>,
    ) -> Result<Battle> {
        let ids = record_ids(self.files.as_ref(), &self.party_dir);
        if ids.is_empty() {
            anyhow::bail!("party table {} has no records", self.party_dir);
        }
        let mut party = Vec::with_capacity(ids.len());
        for id in &ids {
            let mut c = self.load_combatant(&self.party_dir, id)?;
            if let Some(state) = party_state.and_then(|states| states.iter().find(|s| &s.id == id))
            {
                // v2-c: the persistent level/exp overrides the record's —
                // the stats are re-grown first, then the pools clamp in.
                if let Some(levels) = &self.levels {
                    c.level = state.level.max(1);
                    c.exp = state.exp;
                    c.recompute_stats(levels);
                }
                c.hp = state.hp.min(c.max_hp);
                c.mp = state.mp.min(c.max_mp);
                c.status = state.status.clone();
            }
            party.push(c);
        }
        let active = party.iter().position(|c| c.hp > 0).unwrap_or(0);

        // v2-d: an encounter record takes precedence over a single-enemy
        // lookup; both miss ⇒ the v1 first-record fallback.
        let (enemy, rest, trainer, money) = match self.load_encounter(enemy_id)? {
            Some(encounter) => {
                let mut enemies = Vec::with_capacity(encounter.enemy_ids.len());
                for id in &encounter.enemy_ids {
                    enemies.push(self.load_combatant(&self.enemies_dir, id)?);
                }
                let mut enemies = enemies.into_iter();
                let first = enemies.next().expect("encounter enemies non-empty");
                let money = if encounter.trainer {
                    encounter.money
                } else {
                    0
                };
                log::info!(
                    "encounter '{}' ({}): {} enemies, trainer {}",
                    enemy_id,
                    encounter.name,
                    encounter.enemy_ids.len(),
                    encounter.trainer
                );
                (first, enemies.collect(), encounter.trainer, money)
            }
            None => {
                let enemy_id = if self
                    .files
                    .exists(&format!("{}/{enemy_id}.json", self.enemies_dir))
                {
                    enemy_id.to_string()
                } else {
                    let fallback = first_record_id(self.files.as_ref(), &self.enemies_dir)
                        .with_context(|| {
                            format!("enemies table {} has no records", self.enemies_dir)
                        })?;
                    log::warn!(
                        "unknown enemy id '{enemy_id}' — using first enemy record '{fallback}'"
                    );
                    fallback
                };
                (
                    self.load_combatant(&self.enemies_dir, &enemy_id)?,
                    Vec::new(),
                    false,
                    0,
                )
            }
        };

        let items = self.load_items();
        let inventory = inventory.cloned().unwrap_or_else(|| {
            self.items
                .as_ref()
                .map(|i| i.starting.clone())
                .unwrap_or_default()
        });

        log::info!(
            "battle: {} (hp {}, party of {}) vs {} (hp {})",
            party[active].name,
            party[active].max_hp,
            party.len(),
            enemy.name,
            enemy.max_hp
        );
        // RON hooks: (re-)install the thread-local rules host (the parallel
        // test harness runs battles on many threads) and mirror the ACTIVE
        // member + the enemy into the engine battle state.
        let hook_state = self.ron.as_ref().map(|ron| {
            hooks::install_compiled(ron.compiled.clone());
            HookState {
                state: BattleState::new(
                    vec![hooks::mirror_of(
                        &party[active],
                        &ron.stat_names,
                        &ron.status_names,
                        ron.has_resource,
                    )],
                    vec![hooks::mirror_of(
                        &enemy,
                        &ron.stat_names,
                        &ron.status_names,
                        ron.has_resource,
                    )],
                ),
                effects: Vec::new(),
                mv: dotzuki_engine::battle::stack::MoveContext::default(),
                registry: ron.registry.clone(),
                move_records: ron.move_records.clone(),
                status_names: ron.status_names.clone(),
                stat_names: ron.stat_names.clone(),
                has_resource: ron.has_resource,
            }
        });
        let mut battle = Battle::full(
            party,
            active,
            enemy,
            items,
            inventory,
            self.chart.clone(),
            rng,
            hook_state,
        );
        battle.set_levels(self.levels.clone());
        battle.set_enemy_party(rest, trainer, money);
        if battle.party.iter().all(|c| c.hp == 0) {
            battle.arm_loss();
        }
        Ok(battle)
    }

    /// Parse the encounter record `id` (v2-d): `None` when the manifest has
    /// no encounters table or the id names no record in it. An `enemies`
    /// list that is missing/empty or references an unknown enemy record is a
    /// hard error (a clear battle-start failure, never a silent fallback).
    fn load_encounter(&self, id: &str) -> Result<Option<Encounter>> {
        let Some(dir) = &self.encounters_dir else {
            return Ok(None);
        };
        if !self.files.exists(&format!("{dir}/{id}.json")) {
            return Ok(None);
        }
        let record = read_record(self.files.as_ref(), dir, id)?;
        let enemy_ids: Vec<String> = record
            .get("enemies")
            .and_then(|v| v.as_array())
            .map(|ids| {
                ids.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if enemy_ids.is_empty() {
            anyhow::bail!("encounter '{id}' has no 'enemies' list (or it is empty)");
        }
        for enemy_id in &enemy_ids {
            if !self
                .files
                .exists(&format!("{}/{enemy_id}.json", self.enemies_dir))
            {
                anyhow::bail!(
                    "encounter '{id}' references unknown enemy id '{enemy_id}' \
                     (no record in the enemies table)"
                );
            }
        }
        Ok(Some(Encounter {
            name: get_str(&record, "name").unwrap_or(id).to_string(),
            enemy_ids,
            trainer: record
                .get("trainer")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            money: get_num(&record, "money").unwrap_or(0),
        }))
    }

    /// The battle-usable items: every record of the items table whose heal
    /// field holds a positive number (sorted by record id). Empty when the
    /// manifest has no `items` block.
    fn load_items(&self) -> Vec<BattleItem> {
        let Some(items) = &self.items else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for id in record_ids(self.files.as_ref(), &items.dir) {
            let record = match read_record(self.files.as_ref(), &items.dir, &id) {
                Ok(record) => record,
                Err(e) => {
                    log::warn!("item record '{id}' skipped: {e:#}");
                    continue;
                }
            };
            let heal = get_num(&record, &items.heal_field).unwrap_or(0);
            if heal > 0 {
                out.push(BattleItem {
                    name: get_str(&record, "name").unwrap_or(&id).to_string(),
                    id,
                    heal,
                });
            }
        }
        out
    }

    /// The `kind: Move` RON record overriding skill `id`, if any.
    fn ron_move(&self, id: &str) -> Option<&hooks::RonMove> {
        self.ron.as_ref()?.move_records.get(id)
    }

    /// Load one combatant record from `dir` (stats via the field mapping,
    /// skills via the skills table; full HP/MP). With a `levels` block the
    /// stats carry the level-growth multiplier (the record's `levelField`,
    /// default 1 ⇒ ×1) and the enemy side reads its `expField` reward.
    fn load_combatant(&self, dir: &str, id: &str) -> Result<Combatant> {
        let record = read_record(self.files.as_ref(), dir, id)?;
        let level_field = self
            .levels
            .as_ref()
            .map(|l| l.level_field.as_str())
            .unwrap_or("level");
        let level = get_num(&record, level_field).unwrap_or(1).min(255) as u8;
        let stat = |field: &str| get_num(&record, field).unwrap_or(1);
        let base = BaseStats {
            max_hp: stat(&self.stats.hp),
            max_mp: self
                .resource
                .as_deref()
                .map(|f| get_num(&record, f).unwrap_or(0))
                .unwrap_or(0),
            attack: stat(&self.stats.attack),
            defense: stat(&self.stats.defense),
            speed: stat(&self.stats.speed),
        };
        let grown = |raw: u32| match &self.levels {
            Some(levels) => levels.growth(raw, level),
            None => raw,
        };
        let (max_hp, mp) = (grown(base.max_hp), grown(base.max_mp));
        let exp_reward = self
            .levels
            .as_ref()
            .and_then(|l| get_num(&record, &l.exp_field))
            .unwrap_or(0);
        Ok(Combatant {
            id: id.to_string(),
            name: get_str(&record, "name").unwrap_or(id).to_string(),
            element: get_str(&record, "element").map(str::to_string),
            hp: max_hp,
            max_hp,
            attack: grown(base.attack),
            defense: grown(base.defense),
            speed: grown(base.speed),
            level,
            base,
            exp: 0,
            exp_reward,
            stages: Stages::default(),
            mp,
            max_mp: mp,
            skills: self.load_skills(&record),
            status: None,
            ability: get_str(&record, "ability").map(str::to_string),
            held_item: get_str(&record, "heldItem").map(str::to_string),
        })
    }

    /// A combatant's move list: the configured skills field (an array of
    /// skill ids looked up in the skills table; unknown ids are skipped with
    /// a warning) — or just the built-in Attack when no skills table is
    /// configured or the list is empty/missing.
    fn load_skills(&self, record: &serde_json::Value) -> Vec<Skill> {
        let Some(skills_dir) = &self.skills_dir else {
            return vec![basic_attack()];
        };
        let mut skills = Vec::new();
        if let Some(ids) = record.get(&self.skills_field).and_then(|v| v.as_array()) {
            for id in ids {
                let Some(id) = id.as_str() else {
                    continue;
                };
                match read_record(self.files.as_ref(), skills_dir, id) {
                    Ok(rec) => skills.push(skill_from_record(id, &rec, self)),
                    Err(e) => log::warn!("unknown skill id '{id}' skipped: {e:#}"),
                }
            }
        }
        if skills.is_empty() {
            skills.push(basic_attack());
        }
        skills
    }
}

/// Every record id (sorted `.json` filename stems) directly in a table dir
/// (a project-relative POSIX path read through the VFS).
pub(crate) fn record_ids(files: &dyn ProjectFiles, dir: &str) -> Vec<String> {
    let prefix = format!("{dir}/");
    let mut ids: Vec<String> = files
        .list(dir)
        .into_iter()
        .filter_map(|p| {
            // Direct children only (records live flat in the table dir).
            let rest = p.strip_prefix(&prefix)?;
            if rest.contains('/') {
                return None;
            }
            rest.strip_suffix(".json").map(str::to_string)
        })
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// The first record id (sorted `.json` filename stem) in a table dir.
fn first_record_id(files: &dyn ProjectFiles, dir: &str) -> Option<String> {
    record_ids(files, dir).into_iter().next()
}

/// Read and parse `<dir>/<id>.json`.
pub(crate) fn read_record(
    files: &dyn ProjectFiles,
    dir: &str,
    id: &str,
) -> Result<serde_json::Value> {
    let rel = format!("{dir}/{id}.json");
    let bytes = files
        .read(&rel)
        .with_context(|| format!("failed to read {rel}"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("failed to parse {rel}"))
}

/// A record's numeric field (accepts ints and floats); `None` when missing
/// or not a number.
pub(crate) fn get_num(record: &serde_json::Value, field: &str) -> Option<u32> {
    match record.get(field)? {
        serde_json::Value::Number(n) => n
            .as_u64()
            .or_else(|| n.as_f64().map(|f| f.max(0.0) as u64))
            .map(|v| v.min(u32::MAX as u64) as u32),
        _ => None,
    }
}

/// A record's string field; `None` when missing or not a string.
pub(crate) fn get_str<'a>(record: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    record.get(field).and_then(|v| v.as_str())
}

// ── the formula ─────────────────────────────────────────────────────────────

/// Accuracy roll: the hit lands iff `rng % 100 < accuracy`.
pub fn accuracy_roll(accuracy: u32, rng: &mut dyn BattleRng) -> bool {
    u32::from(rng.byte() % 100) < accuracy
}

/// The outcome of one damaging hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageRoll {
    /// Final damage (≥ 1).
    pub damage: u32,
    /// Whether the hit was critical (×1.5).
    pub crit: bool,
    /// Effectiveness numerator.
    pub mult_num: u32,
    /// Effectiveness denominator.
    pub mult_den: u32,
}

/// The standard damage roll: `power × eff_atk / max(1, eff_def)`, then
/// variance ×(85+rng%16)/100, crit (rng%16==0) ×3/2, then the type-chart
/// multiplier; floored at 1. Consumes exactly two rng bytes (variance, crit)
/// — accuracy is rolled separately by the caller.
pub fn damage_roll(
    power: u32,
    eff_atk: u32,
    eff_def: u32,
    mult: (u32, u32),
    rng: &mut dyn BattleRng,
) -> DamageRoll {
    let base = power as u64 * eff_atk as u64 / eff_def.max(1) as u64;
    let varied = base * (85 + (rng.byte() % 16) as u64) / 100;
    let crit = rng.byte().is_multiple_of(16);
    let after_crit = if crit { varied * 3 / 2 } else { varied };
    let damage = (after_crit * mult.0 as u64 / mult.1.max(1) as u64)
        .max(1)
        .min(u32::MAX as u64) as u32;
    DamageRoll {
        damage,
        crit,
        mult_num: mult.0,
        mult_den: mult.1,
    }
}

// ── the battle (turn loop + screen) ─────────────────────────────────────────

/// The battle's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleOutcome {
    /// The enemy fainted.
    Win,
    /// The player fainted.
    Lose,
    /// The player ran from a wild battle (v2-d): no EXP/money, the party
    /// state carries over. Reaches the scene as the `"run"` string — scenes
    /// branching on `== "win"` treat it as not-won.
    Run,
}

/// Which combatant is acting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Player,
    Enemy,
}

/// What follows the current narration queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum After {
    /// Back to the root menu.
    Menu,
    /// The active member fainted and a replacement must be picked (a free
    /// action); the enemy's deferred action then resolves against the new
    /// member.
    ForcedSwitch,
    End(BattleOutcome),
}

/// The turn-loop phase.
#[derive(Debug)]
enum Phase {
    /// The root menu: Fight / Party (/ Item when items are configured).
    Root,
    /// The skill menu (Fight).
    Skills,
    /// The party list from the root menu (B backs out; a legal pick consumes
    /// the player's turn).
    Party,
    /// The item list (B backs out; using one consumes the player's turn).
    Items,
    /// Forced replacement after a faint (no backing out; the pick is free).
    ForcedSwitch,
    /// Narration lines are showing; A advances, `after` runs when drained.
    Narrate {
        lines: VecDeque<String>,
        after: After,
    },
}

/// The outcome of a faint check after an action or residual.
enum FaintFlow {
    /// Both sides stand.
    Continue,
    /// The active enemy fainted and the encounter queue sent out the next
    /// one (v2-d): the round ends, back to the root menu.
    SentOut,
    /// The active member fainted but other members live: a replacement must
    /// be picked before play resumes.
    Switch,
    /// The battle ended.
    End(BattleOutcome),
}

/// A live battle: the player's party (the active member fights) against one
/// enemy, the item inventory, the type chart, and the phase machine. Drive
/// with [`update`](Self::update) + [`draw`](Self::draw);
/// [`outcome`](Self::outcome) reports the end (the runner then resumes the
/// suspended scene with `"win"`/`"lose"` and harvests the party state).
pub struct Battle {
    /// The whole party table (sorted by record id); `active` fights.
    party: Vec<Combatant>,
    /// Index of the active (fighting) member.
    active: usize,
    enemy: Combatant,
    /// The enemies waiting behind the active one (v2-d encounters); empty
    /// for a single-enemy (wild) battle.
    enemies: VecDeque<Combatant>,
    /// Whether this is a trainer battle (v2-d): blocks Run.
    trainer: bool,
    /// The money the runner pays the player on a win (0 for wild battles).
    trainer_money: u32,
    /// The SUM of every defeated enemy's EXP reward (v2-d; equals the single
    /// enemy's `expField` in a wild battle — identical to v1).
    exp_pool: u32,
    /// The currency label for the trainer-money narration (runner's
    /// `shop.currency`, default "G").
    currency: String,
    /// Every battle-usable item record (heal field > 0); the counts live in
    /// `inventory`.
    items: Vec<BattleItem>,
    /// The battle inventory (record id → count), written back to the runner.
    inventory: HashMap<String, u32>,
    chart: TypeChart,
    rng: Box<dyn BattleRng>,
    /// The RON hook machinery (v2-a): `Some` when the project's rules file
    /// compiled a non-empty `effects` registry. The player mirror tracks the
    /// ACTIVE member (re-built on switch).
    hooks: Option<HookState>,
    /// EXP/level growth (v2-c): `None` without a manifest `levels` block —
    /// the win then awards nothing (v1 behavior).
    levels: Option<LevelsSetup>,
    /// Narration language (`"en"`/`"zh"`) for the EXP/level-up lines.
    lang: String,
    phase: Phase,
    cursor: usize,
    outcome: Option<BattleOutcome>,
    /// The active weather: a `kind: Weather` RON record id (v2-e), armed by a
    /// scene's `setWeather` before the battle started. Battle-local: dropped
    /// with the battle, never saved. Its `FieldResidual` hooks fire on each
    /// combatant's residual while set.
    weather: Option<String>,
    /// The enemy's skill when its action was deferred by a forced switch.
    pending_enemy: Option<Skill>,
    /// Every narration line produced so far (acceptance log, tests).
    log: Vec<String>,
}

impl Battle {
    /// A 1v1 battle between two already-built combatants (no RON hooks, no
    /// items — the unit-test shape).
    pub fn new(
        player: Combatant,
        enemy: Combatant,
        chart: TypeChart,
        rng: Box<dyn BattleRng>,
    ) -> Self {
        Self::with_hooks(player, enemy, chart, rng, None)
    }

    /// A battle between two already-built combatants, with the RON hook
    /// state when the project compiled one.
    pub fn with_hooks(
        player: Combatant,
        enemy: Combatant,
        chart: TypeChart,
        rng: Box<dyn BattleRng>,
        hooks: Option<HookState>,
    ) -> Self {
        Self::full(
            vec![player],
            0,
            enemy,
            Vec::new(),
            HashMap::new(),
            chart,
            rng,
            hooks,
        )
    }

    /// The full constructor: the party + its active member, the enemy, the
    /// usable items and the inventory counts.
    #[allow(clippy::too_many_arguments)]
    pub fn full(
        party: Vec<Combatant>,
        active: usize,
        enemy: Combatant,
        items: Vec<BattleItem>,
        inventory: HashMap<String, u32>,
        chart: TypeChart,
        rng: Box<dyn BattleRng>,
        hooks: Option<HookState>,
    ) -> Self {
        Self {
            party,
            active,
            enemy,
            enemies: VecDeque::new(),
            trainer: false,
            trainer_money: 0,
            exp_pool: 0,
            currency: "G".to_string(),
            items,
            inventory,
            chart,
            rng,
            hooks,
            levels: None,
            lang: "en".to_string(),
            weather: None,
            phase: Phase::Root,
            cursor: 0,
            outcome: None,
            pending_enemy: None,
            log: Vec::new(),
        }
    }

    /// A party with no living member loses before the first frame.
    pub(crate) fn arm_loss(&mut self) {
        self.outcome = Some(BattleOutcome::Lose);
    }

    /// Arm the levels config (v2-c; [`BattleSetup::start_with`] calls this).
    pub fn set_levels(&mut self, levels: Option<LevelsSetup>) {
        self.levels = levels;
    }

    /// Set the narration language (`"en"`/`"zh"`) for the EXP/level-up
    /// lines; the runner passes its `--lang` here.
    pub fn set_lang(&mut self, lang: &str) {
        self.lang = lang.to_string();
    }

    /// Set the currency label for the trainer-money narration (the runner's
    /// `shop.currency`); default "G".
    pub fn set_currency(&mut self, currency: &str) {
        self.currency = currency.to_string();
    }

    /// Arm the enemy party extras (v2-d): the enemies queued behind the
    /// active one plus the trainer flag and the win's money reward
    /// ([`BattleSetup::start_with`] calls this; the plain constructors leave
    /// a single wild enemy).
    pub fn set_enemy_party(&mut self, rest: Vec<Combatant>, trainer: bool, money: u32) {
        self.enemies = rest.into();
        self.trainer = trainer;
        self.trainer_money = money;
    }

    /// Arm the battle-local weather (v2-e): a `kind: Weather` RON record id,
    /// `None` to clear. The runner sets this from a scene's `setWeather` /
    /// `clearWeather` before the battle begins; it is never saved and dies
    /// with the battle.
    pub fn set_weather(&mut self, weather: Option<String>) {
        self.weather = weather;
    }

    /// The armed weather record id, if any (tests, introspection).
    pub fn weather(&self) -> Option<&str> {
        self.weather.as_deref()
    }

    /// Battle-start hook pass (v2-e): narrate the armed weather's intro,
    /// then fire both active combatants' ability `SwitchIn` hooks (player
    /// first). Any produced lines queue as the battle's opening narration;
    /// with nothing to say the battle opens on the root menu exactly as v1.
    /// The runner calls this once per battle, after [`set_weather`]; the
    /// plain constructors leave it to tests. No-op without hooks or once the
    /// battle is already decided.
    pub fn begin(&mut self) {
        if self.outcome.is_some() || self.hooks.is_none() {
            return;
        }
        let mut lines = VecDeque::new();
        if let Some(weather) = self.weather.clone() {
            if self.record_has_hooks(&weather) {
                narrate(
                    &mut self.log,
                    &mut lines,
                    weather_start_line(&self.lang, &weather),
                );
            } else {
                log::warn!("weather '{weather}' names no rules.ron record — ignored");
                self.weather = None;
            }
        }
        for side in [Side::Player, Side::Enemy] {
            self.fire_switch_in(side, &mut lines);
        }
        if !lines.is_empty() {
            self.phase = Phase::Narrate {
                lines,
                after: After::Menu,
            };
        }
    }

    /// Whether any compiled hook is sourced from record `id` (any kind).
    fn record_has_hooks(&self, id: &str) -> bool {
        GenericProvider::rules_host()
            .is_some_and(|host| host.compiled.hooks.values().any(|h| h.source_id == id))
    }

    /// Fire one side's ability `SwitchIn` hooks (v2-e): at battle start, on a
    /// voluntary/forced switch-in, and when an encounter sends out the next
    /// enemy. An intro line (`"Aria's Intimidate!"`) narrates first when the
    /// ability record subscribes to `SwitchIn`; the state changes ride the
    /// snapshot-diff narration. No-op without an ability or a subscription.
    fn fire_switch_in(&mut self, side: Side, lines: &mut VecDeque<String>) {
        if self.hooks.is_none() {
            return;
        }
        let combatant = match side {
            Side::Player => &self.party[self.active],
            Side::Enemy => &self.enemy,
        };
        let Some(ability) = combatant.ability.clone() else {
            return;
        };
        if !self.subscribes(&ability, Event::SwitchIn) {
            return;
        }
        let name = combatant.name.clone();
        narrate(
            &mut self.log,
            lines,
            ability_intro_line(&self.lang, &name, &ability),
        );
        self.sync_to_mirrors();
        let who = HookState::battler_ref(side);
        let before = snap_mirrors(self.hooks.as_ref().unwrap());
        self.fire(Event::SwitchIn, &[&ability], who, who, RelayVar::Unit);
        self.narrate_diffs(&before, lines, false);
        self.sync_from_mirrors();
    }

    // ── introspection (runner, tests) ───────────────────────────────────────

    /// The active player combatant.
    pub fn player(&self) -> &Combatant {
        &self.party[self.active]
    }
    /// The whole party (index 0 fights unless switched).
    pub fn party(&self) -> &[Combatant] {
        &self.party
    }
    /// The active member's index in [`party`](Self::party).
    pub fn active_index(&self) -> usize {
        self.active
    }
    /// The enemy combatant.
    pub fn enemy(&self) -> &Combatant {
        &self.enemy
    }
    /// How many enemies still wait behind the active one (v2-d encounters).
    pub fn enemies_remaining(&self) -> usize {
        self.enemies.len()
    }
    /// Whether this is a trainer battle (Run is blocked).
    pub fn is_trainer(&self) -> bool {
        self.trainer
    }
    /// The money the runner pays the player on a win (0 for wild battles).
    pub fn trainer_money(&self) -> u32 {
        self.trainer_money
    }
    /// The result, once the battle has ended (set when the last narration
    /// line is dismissed).
    pub fn outcome(&self) -> Option<BattleOutcome> {
        self.outcome
    }
    /// The full narration history.
    pub fn log(&self) -> &[String] {
        &self.log
    }
    /// The RON hook state, when this battle compiled one (tests, debug).
    pub fn hooks(&self) -> Option<&HookState> {
        self.hooks.as_ref()
    }
    /// The live inventory (record id → count).
    pub fn inventory(&self) -> &HashMap<String, u32> {
        &self.inventory
    }
    /// The persistent party state (v2-b) for the runner to keep between
    /// battles: every member's current HP/MP and status — plus level/exp
    /// (v2-c).
    pub fn party_state(&self) -> Vec<PartyMemberState> {
        self.party
            .iter()
            .map(|c| PartyMemberState {
                id: c.id.clone(),
                hp: c.hp,
                mp: c.mp,
                status: c.status.clone(),
                level: c.level,
                exp: c.exp,
            })
            .collect()
    }
    /// `true` while a player menu owns input.
    pub fn in_menu(&self) -> bool {
        !matches!(self.phase, Phase::Narrate { .. }) && self.outcome.is_none()
    }
    /// The narration line currently on screen.
    pub fn current_line(&self) -> Option<&str> {
        match &self.phase {
            Phase::Narrate { lines, .. } => lines.front().map(String::as_str),
            _ => None,
        }
    }
    /// The current menu's labels (marked `×` entries are unselectable).
    pub fn menu_items(&self) -> Vec<String> {
        match &self.phase {
            Phase::Root => self.root_items(),
            Phase::Skills => self.skill_items(),
            Phase::Party | Phase::ForcedSwitch => self.party_items(),
            Phase::Items => self.item_items(),
            Phase::Narrate { .. } => Vec::new(),
        }
    }

    /// The root menu (Item only when the project configures usable items;
    /// Run always — v2-d).
    fn root_items(&self) -> Vec<String> {
        let mut items = vec!["Fight".to_string(), "Party".to_string()];
        if !self.items.is_empty() {
            items.push("Item".to_string());
        }
        items.push("Run".to_string());
        items
    }

    /// The skill-menu labels (name + cost; unaffordable entries are marked).
    fn skill_items(&self) -> Vec<String> {
        self.player()
            .skills
            .iter()
            .map(|s| {
                let label = if s.cost > 0 {
                    format!("{} {}MP", s.name, s.cost)
                } else {
                    s.name.clone()
                };
                if s.cost > self.player().mp {
                    format!("× {label}")
                } else {
                    label
                }
            })
            .collect()
    }

    /// The party-list labels (name + HP + status; the active member and
    /// fainted members are marked `×` and cannot be picked).
    fn party_items(&self) -> Vec<String> {
        self.party
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let mut label = format!("{} {}/{}", c.name, c.hp, c.max_hp);
                if let Some(status) = &c.status {
                    label.push_str(&format!(" ({status})"));
                }
                if i == self.active || c.hp == 0 {
                    format!("× {label}")
                } else {
                    label
                }
            })
            .collect()
    }

    /// The item-list labels (name + count) for items still in the inventory.
    fn item_items(&self) -> Vec<String> {
        self.usable_items()
            .iter()
            .map(|&i| {
                let item = &self.items[i];
                let count = self.inventory.get(&item.id).copied().unwrap_or(0);
                format!("{} ×{count}", item.name)
            })
            .collect()
    }

    /// Indexes into `items` of the items with a positive inventory count.
    fn usable_items(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| self.inventory.get(&item.id).copied().unwrap_or(0) > 0)
            .map(|(i, _)| i)
            .collect()
    }

    /// The first party index a switch may target (living, not active).
    fn first_switchable(&self) -> usize {
        self.party
            .iter()
            .enumerate()
            .position(|(i, c)| i != self.active && c.hp > 0)
            .unwrap_or(0)
    }

    /// Move a cursor over `n` entries with Up/Down.
    fn move_cursor(&mut self, input: &InputState, n: usize) {
        let n = n.max(1);
        if input.is_just_pressed(GbButton::Up) {
            self.cursor = (self.cursor + n - 1) % n;
        } else if input.is_just_pressed(GbButton::Down) {
            self.cursor = (self.cursor + 1) % n;
        }
    }

    // ── per-frame update ────────────────────────────────────────────────────

    /// Advance the battle one frame: menu cursor / confirm / cancel, or
    /// narration paging. Sets [`outcome`](Self::outcome) when the battle
    /// resolves.
    pub fn update(&mut self, input: &InputState) {
        match std::mem::replace(&mut self.phase, Phase::Root) {
            Phase::Root => {
                let n = self.root_items().len();
                self.move_cursor(input, n);
                if input.is_just_pressed(GbButton::A) {
                    if self.cursor == n - 1 {
                        // The last entry is always Run (v2-d).
                        self.try_run();
                        return;
                    }
                    self.phase = match self.cursor {
                        0 => Phase::Skills,
                        1 => {
                            self.cursor = self.first_switchable();
                            Phase::Party
                        }
                        _ => Phase::Items,
                    };
                    if !matches!(self.phase, Phase::Party) {
                        self.cursor = 0;
                    }
                }
            }
            Phase::Skills => {
                if input.is_just_pressed(GbButton::B) {
                    self.phase = Phase::Root;
                    self.cursor = 0;
                    return;
                }
                let n = self.player().skills.len();
                self.move_cursor(input, n);
                if input.is_just_pressed(GbButton::A) {
                    // Unaffordable skills are unselectable.
                    if self.player().skills[self.cursor].cost <= self.player().mp {
                        let pick = self.cursor;
                        self.execute_round(pick);
                        return;
                    }
                }
                self.phase = Phase::Skills;
            }
            Phase::Party => {
                if input.is_just_pressed(GbButton::B) {
                    self.phase = Phase::Root;
                    self.cursor = 0;
                    return;
                }
                let n = self.party.len();
                self.move_cursor(input, n);
                if input.is_just_pressed(GbButton::A) && self.switch_legal(self.cursor) {
                    let pick = self.cursor;
                    self.execute_switch_round(pick);
                    return;
                }
                self.phase = Phase::Party;
            }
            Phase::Items => {
                if input.is_just_pressed(GbButton::B) {
                    self.phase = Phase::Root;
                    self.cursor = 0;
                    return;
                }
                let n = self.usable_items().len();
                self.move_cursor(input, n);
                if input.is_just_pressed(GbButton::A) && n > 0 {
                    let pick = self.cursor;
                    self.execute_item_round(pick);
                    return;
                }
                self.phase = Phase::Items;
            }
            Phase::ForcedSwitch => {
                let n = self.party.len();
                self.move_cursor(input, n);
                if input.is_just_pressed(GbButton::A) && self.switch_legal(self.cursor) {
                    let pick = self.cursor;
                    self.forced_switch_to(pick);
                    return;
                }
                self.phase = Phase::ForcedSwitch;
            }
            Phase::Narrate { mut lines, after } => {
                if input.is_just_pressed(GbButton::A) {
                    lines.pop_front();
                }
                if lines.is_empty() && input.is_just_pressed(GbButton::A) {
                    match after {
                        After::Menu => {
                            self.phase = Phase::Root;
                            self.cursor = 0;
                        }
                        After::ForcedSwitch => {
                            self.cursor = self.first_switchable();
                            self.phase = Phase::ForcedSwitch;
                        }
                        After::End(o) => self.outcome = Some(o),
                    }
                } else {
                    self.phase = Phase::Narrate { lines, after };
                }
            }
        }
    }

    /// Whether party index `idx` is a legal switch target (living, not the
    /// active member).
    fn switch_legal(&self, idx: usize) -> bool {
        idx != self.active && self.party.get(idx).is_some_and(|c| c.hp > 0)
    }

    /// The Run root entry (v2-d): a wild battle ends on the spot with the
    /// `"run"` outcome (no EXP/money; the party state carries over); a
    /// trainer battle REFUSES — the line narrates and the turn is NOT
    /// consumed (back to the root menu).
    fn try_run(&mut self) {
        let mut lines = VecDeque::new();
        if self.trainer {
            narrate(&mut self.log, &mut lines, run_blocked_line(&self.lang));
            self.phase = Phase::Narrate {
                lines,
                after: After::Menu,
            };
        } else {
            narrate(&mut self.log, &mut lines, run_safe_line(&self.lang));
            self.phase = Phase::Narrate {
                lines,
                after: After::End(BattleOutcome::Run),
            };
        }
    }

    /// One full round: the player's pick vs the enemy AI's pick, faster side
    /// first (eff speed; ties go to the player), resolving each action in
    /// order and queueing the narration. After each action the acting side's
    /// status residuals fire (RON hooks), then the faint checks run.
    fn execute_round(&mut self, player_pick: usize) {
        let player_skill = self.player().skills[player_pick].clone();
        let enemy_skill = ai_pick(&self.enemy);
        let player_first = self.player().eff_speed() >= self.enemy.eff_speed();
        let order = if player_first {
            [Side::Player, Side::Enemy]
        } else {
            [Side::Enemy, Side::Player]
        };

        let mut lines = VecDeque::new();
        let mut after = After::Menu;
        for (pos, side) in order.iter().enumerate() {
            if self.player().hp == 0 || self.enemy.hp == 0 {
                break; // a faint mid-round cancels the remaining action
            }
            let skill = match side {
                Side::Player => player_skill.clone(),
                Side::Enemy => enemy_skill.clone(),
            };
            self.perform(*side, &skill, &mut lines);
            match self.faint_flow(&mut lines) {
                FaintFlow::Continue => {}
                FaintFlow::SentOut => {
                    // The replacement never acts the turn it comes in.
                    after = After::Menu;
                    break;
                }
                FaintFlow::Switch => {
                    // The enemy's action still resolves — against the
                    // replacement, once picked.
                    if order.get(pos + 1) == Some(&Side::Enemy) {
                        self.pending_enemy = Some(enemy_skill);
                    }
                    after = After::ForcedSwitch;
                    break;
                }
                FaintFlow::End(o) => {
                    after = After::End(o);
                    break;
                }
            }
            self.residual(*side, &mut lines);
            match self.faint_flow(&mut lines) {
                FaintFlow::Continue => {}
                FaintFlow::SentOut => {
                    after = After::Menu;
                    break;
                }
                FaintFlow::Switch => {
                    if order.get(pos + 1) == Some(&Side::Enemy) {
                        self.pending_enemy = Some(enemy_skill);
                    }
                    after = After::ForcedSwitch;
                    break;
                }
                FaintFlow::End(o) => {
                    after = After::End(o);
                    break;
                }
            }
        }
        self.phase = Phase::Narrate { lines, after };
    }

    /// The faint check after an action or residual: narrates the faint and
    /// decides what follows — the next queued enemy (v2-d), a forced
    /// replacement while the party has living members, else the win/lose
    /// ending.
    fn faint_flow(&mut self, lines: &mut VecDeque<String>) -> FaintFlow {
        if self.enemy.hp == 0 {
            narrate(
                &mut self.log,
                lines,
                format!("{} fainted!", self.enemy.name),
            );
            // v2-d: the EXP of every defeated enemy accumulates into the
            // end-of-battle award.
            self.exp_pool = self.exp_pool.saturating_add(self.enemy.exp_reward);
            if let Some(next) = self.enemies.pop_front() {
                self.send_out(next, lines);
                return FaintFlow::SentOut;
            }
            narrate(&mut self.log, lines, "You won the battle!".to_string());
            self.award_exp(lines);
            self.award_trainer_money(lines);
            FaintFlow::End(BattleOutcome::Win)
        } else if self.player().hp == 0 {
            let name = self.player().name.clone();
            narrate(&mut self.log, lines, format!("{name} fainted!"));
            if self.party.iter().any(|c| c.hp > 0) {
                FaintFlow::Switch
            } else {
                narrate(&mut self.log, lines, "You lost the battle…".to_string());
                FaintFlow::End(BattleOutcome::Lose)
            }
        } else {
            FaintFlow::Continue
        }
    }

    /// Send out the next queued enemy (v2-d): a fresh combatant (its own
    /// stats/level, no status); the RON opponent mirror is rebuilt and the
    /// old enemy's volatiles drop. The round then ends (the replacement
    /// never acts the turn it comes in).
    fn send_out(&mut self, next: Combatant, lines: &mut VecDeque<String>) {
        self.enemy = next;
        if let Some(hooks) = &mut self.hooks {
            hooks.state.opponent_battlers[0] = hooks::mirror_of(
                &self.enemy,
                &hooks.stat_names,
                &hooks.status_names,
                hooks.has_resource,
            );
            let enemy_ref = HookState::battler_ref(Side::Enemy);
            hooks.effects.retain(|e| e.host != enemy_ref);
        }
        let name = self.enemy.name.clone();
        narrate(&mut self.log, lines, sent_out_line(&self.lang, &name));
        // The incoming enemy's ability fires on switch-in (v2-e).
        self.fire_switch_in(Side::Enemy, lines);
    }

    /// The EXP award on a win (v2-c): every NON-fainted party member gains
    /// the SUM of every defeated enemy's `expField` value (v2-d; a wild
    /// battle's single enemy, identical to v1), then levels up while
    /// its progress covers the curve (`exp_to_next(L) = base × L^exponent`,
    /// capped at `maxLevel`), each level-up recomputing its stats and
    /// healing the max-HP/MP deltas. No `levels` block ⇒ nothing happens
    /// (v1 behavior, byte-for-byte).
    fn award_exp(&mut self, lines: &mut VecDeque<String>) {
        let Some(levels) = self.levels.clone() else {
            return;
        };
        let reward = self.exp_pool;
        let lang = self.lang.clone();
        for i in 0..self.party.len() {
            if self.party[i].hp == 0 {
                continue; // fainted members gain nothing
            }
            let name = self.party[i].name.clone();
            narrate(&mut self.log, lines, gained_exp_line(&lang, &name, reward));
            let c = &mut self.party[i];
            c.exp = c.exp.saturating_add(reward);
            loop {
                let need = levels.exp_to_next(c.level);
                if c.exp < need || c.level >= levels.max_level {
                    break;
                }
                c.exp -= need;
                c.level += 1;
                c.recompute_stats(&levels);
                narrate(&mut self.log, lines, level_up_line(&lang, &name, c.level));
            }
        }
    }

    /// The trainer-money narration on a win (v2-d): the runner reads
    /// [`trainer_money`](Self::trainer_money) and pays it when the battle
    /// ends in a win; here we only narrate. Wild battles award nothing.
    fn award_trainer_money(&mut self, lines: &mut VecDeque<String>) {
        if self.trainer_money > 0 {
            let line = trainer_money_line(&self.lang, self.trainer_money, &self.currency);
            narrate(&mut self.log, lines, line);
        }
    }

    /// A voluntary switch (the Party menu): costs the player's turn — the
    /// enemy acts after the new member comes in.
    fn execute_switch_round(&mut self, idx: usize) {
        let mut lines = VecDeque::new();
        let old_name = self.player().name.clone();
        narrate(&mut self.log, &mut lines, format!("Come back, {old_name}!"));
        self.switch_to(idx, &mut lines);
        let after = self.enemy_turn(&mut lines);
        self.phase = Phase::Narrate { lines, after };
    }

    /// An item use (the Item menu): heals the active member (capped at max),
    /// decrements the inventory, and costs the player's turn.
    fn execute_item_round(&mut self, pick: usize) {
        let usable = self.usable_items();
        let Some(&item_idx) = usable.get(pick) else {
            return;
        };
        let item = self.items[item_idx].clone();
        let mut lines = VecDeque::new();
        let before = self.player().hp;
        let healed = (before + item.heal).min(self.player().max_hp);
        self.party[self.active].hp = healed;
        if let Some(count) = self.inventory.get_mut(&item.id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.inventory.remove(&item.id);
            }
        }
        let name = self.player().name.clone();
        narrate(
            &mut self.log,
            &mut lines,
            format!("{name} used {}!", item.name),
        );
        narrate(
            &mut self.log,
            &mut lines,
            format!("{name} recovered {} HP!", healed - before),
        );
        let after = self.enemy_turn(&mut lines);
        self.phase = Phase::Narrate { lines, after };
    }

    /// The enemy's half of a switch/item round: its AI pick, then its
    /// residuals, with the faint checks between.
    fn enemy_turn(&mut self, lines: &mut VecDeque<String>) -> After {
        let skill = ai_pick(&self.enemy);
        self.perform(Side::Enemy, &skill, lines);
        match self.faint_flow(lines) {
            FaintFlow::SentOut => After::Menu,
            FaintFlow::Switch => After::ForcedSwitch,
            FaintFlow::End(o) => After::End(o),
            FaintFlow::Continue => {
                self.residual(Side::Enemy, lines);
                match self.faint_flow(lines) {
                    FaintFlow::SentOut => After::Menu,
                    FaintFlow::Switch => After::ForcedSwitch,
                    FaintFlow::End(o) => After::End(o),
                    FaintFlow::Continue => After::Menu,
                }
            }
        }
    }

    /// A forced replacement after a faint (a free action): the new member
    /// comes in, then the enemy's deferred action (if any) resolves.
    fn forced_switch_to(&mut self, idx: usize) {
        let mut lines = VecDeque::new();
        self.switch_to(idx, &mut lines);
        let mut after = After::Menu;
        if let Some(skill) = self.pending_enemy.take() {
            self.perform(Side::Enemy, &skill, &mut lines);
            match self.faint_flow(&mut lines) {
                FaintFlow::SentOut => after = After::Menu,
                FaintFlow::Switch => after = After::ForcedSwitch,
                FaintFlow::End(o) => after = After::End(o),
                FaintFlow::Continue => {
                    self.residual(Side::Enemy, &mut lines);
                    after = match self.faint_flow(&mut lines) {
                        FaintFlow::SentOut => After::Menu,
                        FaintFlow::Switch => After::ForcedSwitch,
                        FaintFlow::End(o) => After::End(o),
                        FaintFlow::Continue => After::Menu,
                    };
                }
            }
        }
        self.phase = Phase::Narrate { lines, after };
    }

    /// Bring party member `idx` in: its stat stages reset (documented), the
    /// RON mirror is re-built from the member's CURRENT state (status
    /// persists with the member), and the old battler's volatiles drop.
    fn switch_to(&mut self, idx: usize, lines: &mut VecDeque<String>) {
        self.active = idx;
        self.party[idx].stages = Stages::default();
        if let Some(hooks) = &mut self.hooks {
            hooks.state.player_battlers[0] = hooks::mirror_of(
                &self.party[idx],
                &hooks.stat_names,
                &hooks.status_names,
                hooks.has_resource,
            );
            let player_ref = HookState::battler_ref(Side::Player);
            hooks.effects.retain(|e| e.host != player_ref);
        }
        let name = self.party[idx].name.clone();
        narrate(&mut self.log, lines, format!("Go, {name}!"));
        // The incoming member's ability fires on switch-in (v2-e).
        self.fire_switch_in(Side::Player, lines);
    }

    /// Resolve one action: the MP gate (re-checked), the accuracy roll, then
    /// the skill's effect (damage / heal / stage change), narrating each step.
    /// A RON-taken-over skill runs through the stack interpreter instead
    /// ([`perform_ron`](Self::perform_ron)).
    fn perform(&mut self, side: Side, skill: &Skill, lines: &mut VecDeque<String>) {
        if skill.ron && self.hooks.is_some() {
            self.perform_ron(side, skill, lines);
            return;
        }
        let (attacker, defender) = match side {
            Side::Player => (&mut self.party[self.active], &mut self.enemy),
            Side::Enemy => (&mut self.enemy, &mut self.party[self.active]),
        };

        // The MP gate is re-checked at resolution time.
        if skill.cost > attacker.mp {
            narrate(
                &mut self.log,
                lines,
                format!("{} tried to use {}!", attacker.name, skill.name),
            );
            narrate(
                &mut self.log,
                lines,
                "But there wasn't enough MP!".to_string(),
            );
            return;
        }
        attacker.mp -= skill.cost;
        narrate(
            &mut self.log,
            lines,
            format!("{} used {}!", attacker.name, skill.name),
        );

        if !accuracy_roll(skill.accuracy, self.rng.as_mut()) {
            narrate(&mut self.log, lines, "But it missed!".to_string());
            return;
        }

        match skill.category {
            SkillCategory::Damage => {
                let mult = self
                    .chart
                    .mult(skill.element.as_deref(), defender.element.as_deref());
                let roll = damage_roll(
                    skill.power,
                    attacker.eff_attack(),
                    defender.eff_defense(),
                    mult,
                    self.rng.as_mut(),
                );
                defender.hp = defender.hp.saturating_sub(roll.damage);
                if roll.crit {
                    narrate(&mut self.log, lines, "Critical hit!".to_string());
                }
                if roll.mult_num > roll.mult_den {
                    narrate(&mut self.log, lines, "It's super effective!".to_string());
                } else if roll.mult_num < roll.mult_den {
                    narrate(&mut self.log, lines, "It's not very effective…".to_string());
                }
                narrate(&mut self.log, lines, format!("{} damage!", roll.damage));
            }
            SkillCategory::Heal => {
                let before = attacker.hp;
                attacker.hp = (attacker.hp + skill.power).min(attacker.max_hp);
                narrate(
                    &mut self.log,
                    lines,
                    format!("{} recovered {} HP!", attacker.name, attacker.hp - before),
                );
            }
            SkillCategory::Buff => {
                attacker.stages.bump(&skill.stat, 1);
                narrate(
                    &mut self.log,
                    lines,
                    format!("{}'s {} rose!", attacker.name, stat_label(&skill.stat)),
                );
            }
            SkillCategory::Debuff => {
                defender.stages.bump(&skill.stat, -1);
                narrate(
                    &mut self.log,
                    lines,
                    format!("{}'s {} fell!", defender.name, stat_label(&skill.stat)),
                );
            }
        }
    }

    // ── RON effect hooks (v2-a) ─────────────────────────────────────────────

    /// Fire one stack event for the hooks sourced from ANY of `source_ids`
    /// (a skill id plus — v2-e — the acting combatant's ability / held-item
    /// record ids, or a status / weather record id), threading `relay`
    /// through the fold (the minimon/wuxia harness shape: per-record filter →
    /// `collect_handlers` → `run_event`). Returns the fold's output relay.
    fn fire(
        &mut self,
        event: Event,
        source_ids: &[&str],
        target: BattlerRef,
        source: BattlerRef,
        relay: RelayVar,
    ) -> RelayVar {
        let hooks = self.hooks.as_mut().expect("fire requires hook state");
        let host = GenericProvider::rules_host().expect("rules host installed");
        let provider = GenericProvider;
        let mut adapter = hooks::RngAdapter(self.rng.as_mut());
        let mut ctx = BattleCtx {
            state: &mut hooks.state,
            effects: &mut hooks.effects,
            mv: &mut hooks.mv,
            rng: &mut adapter,
        };
        let mut hs = Vec::new();
        for eff in &hooks.registry {
            let matches = host
                .compiled
                .hook(eff.id)
                .map(|h| source_ids.contains(&h.source_id.as_str()))
                .unwrap_or(false);
            if matches {
                collect_handlers(&ctx, &provider, Some(eff), event, target, source, &mut hs);
            }
        }
        run_event(&mut ctx, hs, relay, false)
    }

    /// Copy the live pools (HP/MP/stats/stages/status) into the engine mirrors.
    fn sync_to_mirrors(&mut self) {
        let Some(hooks) = &mut self.hooks else { return };
        let (stat_names, status_names, has_resource) =
            (&hooks.stat_names, &hooks.status_names, hooks.has_resource);
        hooks::sync_to_mirror(
            &self.party[self.active],
            &mut hooks.state.player_battlers[0],
            stat_names,
            status_names,
            has_resource,
        );
        hooks::sync_to_mirror(
            &self.enemy,
            &mut hooks.state.opponent_battlers[0],
            stat_names,
            status_names,
            has_resource,
        );
    }

    /// Copy the pools back from the engine mirrors after a fire.
    fn sync_from_mirrors(&mut self) {
        let Some(hooks) = &mut self.hooks else { return };
        let (stat_names, status_names, has_resource) =
            (&hooks.stat_names, &hooks.status_names, hooks.has_resource);
        hooks::sync_from_mirror(
            &hooks.state.player_battlers[0],
            &mut self.party[self.active],
            stat_names,
            status_names,
            has_resource,
        );
        hooks::sync_from_mirror(
            &hooks.state.opponent_battlers[0],
            &mut self.enemy,
            stat_names,
            status_names,
            has_resource,
        );
    }

    /// Narrate the state changes a fire produced (status inflicted/cured,
    /// stat stages, HP/MP moved) by diffing the mirror snapshots. `residual`
    /// flavors HP loss as the status chip ("… is hurt by poison!").
    fn narrate_diffs(
        &mut self,
        before: &[MirrorSnap; 2],
        lines: &mut VecDeque<String>,
        residual: bool,
    ) {
        let names = [self.player().name.clone(), self.enemy.name.clone()];
        let Some(hooks) = &self.hooks else { return };
        let produced = diff_lines(hooks, before, [&names[0], &names[1]], residual);
        for line in produced {
            narrate(&mut self.log, lines, line);
        }
    }

    /// Resolve one RON-taken-over skill: the v1 MP gate + accuracy roll, then
    /// the stack event sequence over the mirrored battlers — `BeforeMove`
    /// gate (when subscribed) → damage precompute (the v1 formula into
    /// `ctx.mv.damage`) → `ModifyDamage` → `Effectiveness` → `Damage` → apply
    /// → `DamagingHit` → `AfterMove` (the minimon/wuxia fire order).
    fn perform_ron(&mut self, side: Side, skill: &Skill, lines: &mut VecDeque<String>) {
        let (attacker, defender) = match side {
            Side::Player => (&mut self.party[self.active], &mut self.enemy),
            Side::Enemy => (&mut self.enemy, &mut self.party[self.active]),
        };

        // The MP gate is re-checked at resolution time (v1 parity); the RON
        // record's `cost:` already fed `skill.cost` at load.
        if skill.cost > attacker.mp {
            narrate(
                &mut self.log,
                lines,
                format!("{} tried to use {}!", attacker.name, skill.name),
            );
            narrate(
                &mut self.log,
                lines,
                "But there wasn't enough MP!".to_string(),
            );
            return;
        }
        attacker.mp -= skill.cost;
        narrate(
            &mut self.log,
            lines,
            format!("{} used {}!", attacker.name, skill.name),
        );

        if !accuracy_roll(skill.accuracy, self.rng.as_mut()) {
            narrate(&mut self.log, lines, "But it missed!".to_string());
            return;
        }

        let eff_atk = attacker.eff_attack();
        let eff_def = defender.eff_defense();
        let def_element = defender.element.clone();
        // v2-e: the acting combatant's ability and held-item records join its
        // per-action event sequence (an ability hooking `ModifyDamage` etc.
        // fires alongside the skill's own hooks).
        let ability = attacker.ability.clone();
        let held_item = attacker.held_item.clone();
        let mut ids: Vec<&str> = vec![skill.id.as_str()];
        if let Some(a) = &ability {
            ids.push(a);
        }
        if let Some(i) = &held_item {
            ids.push(i);
        }
        let source = HookState::battler_ref(side);
        let target = HookState::battler_ref(match side {
            Side::Player => Side::Enemy,
            Side::Enemy => Side::Player,
        });

        self.sync_to_mirrors();

        // BeforeMove gate — only when the record subscribes. Relay starts
        // `Bool(true)`; a `Fail` (`VetoIf` / unaffordable `PayResource`)
        // yields `Bool(false)`, a silent veto `Unit`.
        if self.subscribes_any(&ids, Event::BeforeMove) {
            let before = snap_mirrors(self.hooks.as_ref().unwrap());
            let out = self.fire(
                Event::BeforeMove,
                &ids,
                target,
                source,
                RelayVar::Bool(true),
            );
            self.narrate_diffs(&before, lines, false);
            match out {
                RelayVar::Bool(false) => {
                    narrate(&mut self.log, lines, "But it failed!".to_string());
                    self.sync_from_mirrors();
                    return;
                }
                RelayVar::Unit => {
                    self.sync_from_mirrors();
                    return;
                }
                _ => {}
            }
        }

        // When the record subscribes to `Effectiveness` the hooks own the
        // scaling (author `ApplyTypeChart` for the chart); otherwise the v1
        // direct chart application applies in the precompute.
        let has_effectiveness_hooks = self.subscribes_any(&ids, Event::Effectiveness);

        if skill.power > 0 {
            let mult = if has_effectiveness_hooks {
                (1, 1)
            } else {
                self.chart
                    .mult(skill.element.as_deref(), def_element.as_deref())
            };
            let roll = damage_roll(skill.power, eff_atk, eff_def, mult, self.rng.as_mut());
            self.hooks.as_mut().unwrap().mv.damage = roll.damage.min(u32::from(u16::MAX)) as u16;
            if roll.crit {
                narrate(&mut self.log, lines, "Critical hit!".to_string());
            }
            if !has_effectiveness_hooks {
                if roll.mult_num > roll.mult_den {
                    narrate(&mut self.log, lines, "It's super effective!".to_string());
                } else if roll.mult_num < roll.mult_den {
                    narrate(&mut self.log, lines, "It's not very effective…".to_string());
                }
            }

            // ModifyDamage fold (ScaleRelay/SetDamage ride here).
            let before = snap_mirrors(self.hooks.as_ref().unwrap());
            let in_damage = self.hooks.as_ref().unwrap().mv.damage;
            let out = self.fire(
                Event::ModifyDamage,
                &ids,
                target,
                source,
                RelayVar::Damage(in_damage),
            );
            self.narrate_diffs(&before, lines, false);
            match out {
                RelayVar::Damage(d) => self.hooks.as_mut().unwrap().mv.damage = d,
                RelayVar::Bool(false) => {
                    narrate(&mut self.log, lines, "But it failed!".to_string());
                    self.sync_from_mirrors();
                    return;
                }
                RelayVar::Unit => {
                    self.sync_from_mirrors();
                    return;
                }
                _ => {}
            }

            // Effectiveness fold (ApplyTypeChart; effectiveness narrated from
            // what the fold actually did to the number).
            if has_effectiveness_hooks {
                let before = snap_mirrors(self.hooks.as_ref().unwrap());
                let in_damage = self.hooks.as_ref().unwrap().mv.damage;
                let out = self.fire(
                    Event::Effectiveness,
                    &ids,
                    target,
                    source,
                    RelayVar::Damage(in_damage),
                );
                self.narrate_diffs(&before, lines, false);
                match out {
                    RelayVar::Damage(d) => {
                        self.hooks.as_mut().unwrap().mv.damage = d;
                        if d > in_damage {
                            narrate(&mut self.log, lines, "It's super effective!".to_string());
                        } else if d < in_damage {
                            narrate(&mut self.log, lines, "It's not very effective…".to_string());
                        }
                    }
                    RelayVar::Bool(false) => {
                        narrate(&mut self.log, lines, "But it failed!".to_string());
                        self.sync_from_mirrors();
                        return;
                    }
                    RelayVar::Unit => {
                        self.sync_from_mirrors();
                        return;
                    }
                    _ => {}
                }
            }

            // The Damage fold (absorb / floor / veto hooks), then apply.
            let before = snap_mirrors(self.hooks.as_ref().unwrap());
            let in_damage = self.hooks.as_ref().unwrap().mv.damage;
            let out = self.fire(
                Event::Damage,
                &ids,
                target,
                source,
                RelayVar::Damage(in_damage),
            );
            self.narrate_diffs(&before, lines, false);
            let final_damage = match out {
                RelayVar::Damage(d) => d,
                RelayVar::Bool(false) => {
                    narrate(&mut self.log, lines, "But it failed!".to_string());
                    self.sync_from_mirrors();
                    return;
                }
                RelayVar::Unit => {
                    self.sync_from_mirrors();
                    return;
                }
                _ => in_damage,
            };
            {
                let hooks = self.hooks.as_mut().unwrap();
                let b = if target.side == 0 {
                    &mut hooks.state.player_battlers[target.slot as usize]
                } else {
                    &mut hooks.state.opponent_battlers[target.slot as usize]
                };
                b.take_damage(final_damage);
                hooks.mv.last_damage = final_damage;
            }
            narrate(&mut self.log, lines, format!("{final_damage} damage!"));
            // Keep the pools synced so the faint checks read live HP.
            self.sync_from_mirrors();
        }

        // DamagingHit (secondary effects: InflictStatus riders etc.) — fired
        // after any landed hit, damaging or not, so a power-0 status skill's
        // riders still run.
        let before = snap_mirrors(self.hooks.as_ref().unwrap());
        let last_damage = self.hooks.as_ref().unwrap().mv.last_damage;
        self.fire(
            Event::DamagingHit,
            &ids,
            target,
            source,
            RelayVar::Damage(last_damage),
        );
        self.narrate_diffs(&before, lines, false);

        // AfterMove (per-action cleanup: self-chips, volatiles).
        let before = snap_mirrors(self.hooks.as_ref().unwrap());
        self.fire(Event::AfterMove, &ids, target, source, RelayVar::Unit);
        self.narrate_diffs(&before, lines, false);

        self.sync_from_mirrors();
    }

    /// Whether the skill's RON record subscribes to `event`.
    fn subscribes(&self, skill_id: &str, event: Event) -> bool {
        self.hooks
            .as_ref()
            .is_some_and(|h| h.subscribes(skill_id, event))
    }

    /// Whether ANY of `ids` (a skill plus the acting combatant's ability /
    /// held-item records, v2-e) subscribes to `event`.
    fn subscribes_any(&self, ids: &[&str], event: Event) -> bool {
        ids.iter().any(|id| self.subscribes(id, event))
    }

    /// The end-of-action residual: the acting combatant's status record's
    /// `Residual` hooks (poison chip etc., v2-a), its held-item record's
    /// `Residual` hooks (Leftovers-style heal, v2-e), and the active
    /// weather's `FieldResidual` hooks with this side as the target (v2-e —
    /// so on a full round each side ticks once). No-op without hooks.
    fn residual(&mut self, side: Side, lines: &mut VecDeque<String>) {
        if self.hooks.is_none() {
            return;
        }
        // The mirror must see the action's results first (a v1-path action
        // mutates the Combatant directly).
        self.sync_to_mirrors();
        let who = HookState::battler_ref(side);

        // 1. The persistent status's residual (v2-a).
        let status_id = {
            let hooks = self.hooks.as_ref().unwrap();
            hooks
                .battler(side)
                .status
                .clone()
                .and_then(|hooks::StatusId(idx)| hooks.status_names.get(idx as usize).cloned())
        };
        if let Some(source_id) = status_id {
            let before = snap_mirrors(self.hooks.as_ref().unwrap());
            self.fire(Event::Residual, &[&source_id], who, who, RelayVar::Unit);
            self.narrate_diffs(&before, lines, true);
        }

        // 2. The held item's residual (v2-e): persistent — never consumed.
        let held_item = match side {
            Side::Player => self.party[self.active].held_item.clone(),
            Side::Enemy => self.enemy.held_item.clone(),
        };
        if let Some(item_id) = held_item {
            let before = snap_mirrors(self.hooks.as_ref().unwrap());
            self.fire(Event::Residual, &[&item_id], who, who, RelayVar::Unit);
            self.narrate_diffs(&before, lines, false);
        }

        // 3. The weather's field residual (v2-e).
        if let Some(weather) = self.weather.clone() {
            let before = snap_mirrors(self.hooks.as_ref().unwrap());
            self.fire(Event::FieldResidual, &[&weather], who, who, RelayVar::Unit);
            self.narrate_diffs(&before, lines, false);
        }

        self.sync_from_mirrors();
    }

    // ── rendering ───────────────────────────────────────────────────────────

    /// Draw the battle screen: a two-tone field with placeholder combatant
    /// blobs, an enemy panel (name, element, HP bar) up top, a player panel
    /// (name, HP bar, MP) below, and the current menu or narration line at
    /// the bottom.
    pub fn draw(&self, fb: &mut FrameBuffer) {
        // Field.
        fb.fill_rect(
            0,
            0,
            SCREEN_W as u32,
            SCREEN_H as u32,
            Rgba::rgb(0x18, 0x18, 0x28),
        );
        fb.fill_rect(
            0,
            130,
            SCREEN_W as u32,
            SCREEN_H as u32 - 130,
            Rgba::rgb(0x20, 0x2A, 0x20),
        );

        // Placeholder combatants (colored blobs, palette from the record id).
        draw_blob(fb, 228, 28, 56, blob_color(&self.enemy.id));
        draw_blob(fb, 52, 108, 64, blob_color(&self.player().id));

        // Enemy panel: name + element + HP bar.
        draw_panel(fb, 8, 8, 184, 46);
        text(fb, &self.enemy.name, 16, 14, Rgba::rgb(0xF0, 0xF0, 0xF0));
        if let Some(element) = &self.enemy.element {
            text(fb, element, 16, 26, Rgba::rgb(0x90, 0xA8, 0xC8));
        }
        draw_bar(fb, 80, 28, 104, 6, self.enemy.hp, self.enemy.max_hp);

        // Player panel (the ACTIVE member): name + HP bar + numbers + MP.
        draw_panel(fb, 132, 136, 180, 46);
        text(
            fb,
            &self.player().name,
            140,
            142,
            Rgba::rgb(0xF0, 0xF0, 0xF0),
        );
        draw_bar(fb, 140, 156, 104, 6, self.player().hp, self.player().max_hp);
        text(
            fb,
            &format!("{}/{}", self.player().hp, self.player().max_hp),
            250,
            154,
            Rgba::rgb(0xC8, 0xC8, 0xC8),
        );
        text(
            fb,
            &format!("MP {}/{}", self.player().mp, self.player().max_mp),
            140,
            168,
            Rgba::rgb(0x90, 0xA8, 0xC8),
        );

        // Bottom: the current menu (+prompt) or the current narration line.
        match &self.phase {
            Phase::Root => {
                draw_textbox(fb, "What will you do?");
                self.draw_menu(fb);
            }
            Phase::Skills => {
                draw_textbox(fb, "Choose a skill!");
                self.draw_menu(fb);
            }
            Phase::Party => {
                draw_textbox(fb, "Switch to whom?");
                self.draw_menu(fb);
            }
            Phase::Items => {
                draw_textbox(fb, "Use which item?");
                self.draw_menu(fb);
            }
            Phase::ForcedSwitch => {
                draw_textbox(fb, "Choose your next fighter!");
                self.draw_menu(fb);
            }
            Phase::Narrate { lines, .. } => {
                if let Some(line) = lines.front() {
                    draw_textbox(fb, line);
                }
            }
        }
    }

    /// The current menu above the dialogue area (the cursor marks the
    /// selection; entries marked `×` cannot be confirmed).
    fn draw_menu(&self, fb: &mut FrameBuffer) {
        let items = self.menu_items();
        let n = items.len() as u32;
        if n == 0 {
            return;
        }
        let max_len = items.iter().map(|o| o.chars().count()).max().unwrap_or(1) as u32;
        // +4: left/right border, cursor column, one padding column.
        let w = (max_len + 4).clamp(10, 24);
        let h = n + 2;
        let tx = (40 - w) as i32;
        let ty = DIALOG_AREA.ty as i32 - h as i32;
        let config = MenuConfig::new(
            TileRect::new(tx.max(0) as u32, ty.max(0) as u32, w, h),
            None,
            TileRect::new(tx.max(0) as u32 + 1, ty.max(0) as u32 + 1, w - 2, n),
            Default::default(),
        );
        let state = FlexMenuState {
            cursor: self.cursor,
            scroll_offset: 0,
        };
        let mut painter = FrameBufferPainter::new(fb);
        let mut ui = Ui::new(&mut painter);
        draw_flex_menu(&items, &[config], &state, items.len(), &mut ui);
    }
}

/// The enemy AI's skill pick: highest-power affordable (ties → earliest in
/// the list); with no affordable skill, the built-in Attack.
fn ai_pick(combatant: &Combatant) -> Skill {
    combatant
        .skills
        .iter()
        .filter(|s| s.cost <= combatant.mp)
        .max_by_key(|s| s.power)
        .cloned()
        .unwrap_or_else(basic_attack)
}

/// Push a narration line onto the queue and the battle's running log.
fn narrate(log: &mut Vec<String>, lines: &mut VecDeque<String>, line: String) {
    log.push(line.clone());
    lines.push_back(line);
}

/// "<name> gained <n> EXP!" (interpolated, so it can't sit in a label table).
fn gained_exp_line(lang: &str, name: &str, n: u32) -> String {
    if lang == "zh" {
        format!("{name} 获得了 {n} 点经验！")
    } else {
        format!("{name} gained {n} EXP!")
    }
}

/// "<name> grew to level <n>!" (interpolated).
fn level_up_line(lang: &str, name: &str, level: u8) -> String {
    if lang == "zh" {
        format!("{name} 升到了 {level} 级！")
    } else {
        format!("{name} grew to level {level}!")
    }
}

/// "Foe sent out <name>!" (v2-d encounter queue; interpolated).
fn sent_out_line(lang: &str, name: &str) -> String {
    if lang == "zh" {
        format!("对方派出了 {name}！")
    } else {
        format!("Foe sent out {name}!")
    }
}

/// "Got away safely!" — a successful Run from a wild battle.
fn run_safe_line(lang: &str) -> String {
    if lang == "zh" {
        "顺利逃走了！".to_string()
    } else {
        "Got away safely!".to_string()
    }
}

/// "Can't escape from a trainer battle!" — Run blocked (turn not consumed).
fn run_blocked_line(lang: &str) -> String {
    if lang == "zh" {
        "无法从训练家的战斗中逃走！".to_string()
    } else {
        "Can't escape from a trainer battle!".to_string()
    }
}

/// "Got <n> <currency> for winning!" — the trainer-money award.
fn trainer_money_line(lang: &str, money: u32, currency: &str) -> String {
    if lang == "zh" {
        format!("赢得了 {money} {currency}！")
    } else {
        format!("Got {money} {currency} for winning!")
    }
}

/// "<name>'s <Ability>!" — the switch-in intro when an ability fires (v2-e).
/// The RON record has no display name, so the id is prettified
/// (`swift-swim` → `Swift Swim`).
fn ability_intro_line(lang: &str, name: &str, ability: &str) -> String {
    let label = prettify_id(ability);
    if lang == "zh" {
        format!("{name} 的 {label}！")
    } else {
        format!("{name}'s {label}!")
    }
}

/// "A <weather> rages!" — the battle-start weather intro (v2-e; the id
/// stays as authored).
fn weather_start_line(lang: &str, weather: &str) -> String {
    if lang == "zh" {
        format!("{weather} 开始了！")
    } else {
        format!("A {weather} rages!")
    }
}

/// Prettify a record id for narration: `kebab-case`/`snake_case` → `Title Case`.
fn prettify_id(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ── hook-fire narration (v2-a) ──────────────────────────────────────────────

/// A snapshot of one mirrored battler's narratable state (diffed across a
/// stack fire).
#[derive(Clone)]
struct MirrorSnap {
    hp: u16,
    status: Option<hooks::StatusId>,
    stages: Vec<i8>,
}

/// Snapshot both engine mirrors (index 0 = player, 1 = enemy).
fn snap_mirrors(hooks: &HookState) -> [MirrorSnap; 2] {
    [Side::Player, Side::Enemy].map(|side| {
        let b = hooks.battler(side);
        MirrorSnap {
            hp: b.hp,
            status: b.status.clone(),
            stages: (0..hooks.stat_names.len())
                .map(|i| {
                    b.stat_stages
                        .get(hooks::StatId(i as u16))
                        .copied()
                        .unwrap_or(0)
                })
                .collect(),
        }
    })
}

/// The narration lines for the difference between `before` and the mirrors'
/// current state: status inflicted/cured, stat stages, HP movement. With
/// `residual`, HP loss on a statused combatant reads as the status chip.
fn diff_lines(
    hooks: &HookState,
    before: &[MirrorSnap; 2],
    names: [&str; 2],
    residual: bool,
) -> Vec<String> {
    let after = snap_mirrors(hooks);
    let mut out = Vec::new();
    for i in 0..2 {
        let (b, a, name) = (&before[i], &after[i], names[i]);
        let status_name = |id: &hooks::StatusId| {
            hooks
                .status_names
                .get(id.0 as usize)
                .map(String::as_str)
                .unwrap_or("?")
        };
        match (&b.status, &a.status) {
            (None, Some(id)) => out.push(format!("{name} was afflicted with {}!", status_name(id))),
            (Some(id), None) => out.push(format!("{name} is no longer {}!", status_name(id))),
            _ => {}
        }
        for (s, stat) in hooks.stat_names.iter().enumerate() {
            let (bv, av) = (b.stages[s], a.stages[s]);
            if av > bv {
                out.push(format!(
                    "{name}'s {} rose!",
                    stat_label(&normalize_stat_key(stat))
                ));
            } else if av < bv {
                out.push(format!(
                    "{name}'s {} fell!",
                    stat_label(&normalize_stat_key(stat))
                ));
            }
        }
        if a.hp < b.hp {
            match (&a.status, &b.status, residual) {
                (Some(id), _, true) | (_, Some(id), true) => {
                    out.push(format!("{name} is hurt by {}!", status_name(id)));
                }
                _ => out.push(format!("{name} lost {} HP!", b.hp - a.hp)),
            }
        } else if a.hp > b.hp {
            out.push(format!("{name} recovered {} HP!", a.hp - b.hp));
        }
    }
    out
}

// ── drawing helpers ─────────────────────────────────────────────────────────

/// Text via the embedded font.
pub(crate) fn text(fb: &mut FrameBuffer, s: &str, x: u32, y: u32, color: Rgba) {
    embedded_font::draw_text(s, x, y, color, fb);
}

/// A bordered info panel.
pub(crate) fn draw_panel(fb: &mut FrameBuffer, x: u32, y: u32, w: u32, h: u32) {
    fb.fill_rect(x, y, w, h, Rgba::rgb(0xC8, 0xC8, 0xD0));
    fb.fill_rect(x + 1, y + 1, w - 2, h - 2, Rgba::rgb(0x28, 0x28, 0x38));
}

/// A proportional HP bar (green, turning red below a quarter).
fn draw_bar(fb: &mut FrameBuffer, x: u32, y: u32, w: u32, h: u32, hp: u32, max_hp: u32) {
    let frac = if max_hp == 0 {
        0.0
    } else {
        hp as f32 / max_hp as f32
    };
    fb.fill_rect(x, y, w, h, Rgba::rgb(0x10, 0x10, 0x18));
    let fill = if frac < 0.25 {
        Rgba::rgb(0xD0, 0x40, 0x38)
    } else {
        Rgba::rgb(0x40, 0xC0, 0x58)
    };
    let inner = ((w - 2) as f32 * frac.clamp(0.0, 1.0)) as u32;
    if inner > 0 {
        fb.fill_rect(x + 1, y + 1, inner, h - 2, fill);
    }
}

/// A blob color derived from a record id (distinct ids read as distinct monsters).
fn blob_color(id: &str) -> Rgba {
    let hash = id.bytes().fold(0x811C_9DC5_u32, |h, b| {
        h.wrapping_mul(16_777_619) ^ b as u32
    });
    let hue = (hash >> 8) as u8;
    Rgba::rgb(
        0x60 + hue % 0x70,
        0x60 + (hue / 3) % 0x70,
        0x60 + (hue / 7) % 0x70,
    )
}

/// A round-ish solid placeholder combatant blob.
fn draw_blob(fb: &mut FrameBuffer, x: i32, y: i32, size: i32, color: Rgba) {
    let r = (size / 7).max(2);
    for dy in 0..size {
        for dx in 0..size {
            let (px, py) = (x + dx, y + dy);
            if px < 0 || py < 0 || px >= SCREEN_W || py >= SCREEN_H {
                continue;
            }
            if (dx < r || dx >= size - r) && (dy < r || dy >= size - r) {
                continue;
            }
            fb.set_pixel(px as u32, py as u32, color);
        }
    }
}

#[cfg(test)]
mod tests;
