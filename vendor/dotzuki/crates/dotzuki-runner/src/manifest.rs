//! Serde model of `.dotzuki-editor.json`, the single manifest of a zero-Rust
//! game project. See `docs/reference/project-manifest.md` for the full
//! contract.
//!
//! Reads are lenient (unknown keys ignored, everything optional except the
//! fields every consumer needs) so old editor-written projects keep parsing.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Root-relative scene directory used when the manifest has no `game` section.
pub const DEFAULT_SCENES_DIR: &str = "assets/scenes";

/// Story-activity scenesDir fallback, mirroring the editor's SCENE_DEFAULT_DIR.
pub const DEFAULT_STORY_SCENES_DIR: &str = "maps";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Display name (not necessarily a slug).
    pub name: String,
    /// Game data root, relative to the project dir (e.g. "./data").
    #[serde(rename = "dataRoot")]
    pub data_root: String,
    /// Graphics root, relative to the project dir (e.g. "./gfx").
    #[serde(rename = "gfxRoot", default, skip_serializing_if = "Option::is_none")]
    pub gfx_root: Option<String>,
    #[serde(default)]
    pub activities: Vec<Activity>,
    /// Optional engine-facing section; absent in older editor projects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game: Option<GameSection>,
    /// Optional battle-system section; absent in projects without battles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battle: Option<BattleSection>,
    /// Optional shop/currency section; absent ⇒ the defaults below apply the
    /// moment money is first needed (a shop opens or the Bag shows money).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shop: Option<ShopSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub enabled: bool,
    /// Free-form per-type config (see the spec for the known keys).
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GameSection {
    /// Stem of the `.scene` file under `scenesDir` the game boots into.
    #[serde(
        rename = "entryScene",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub entry_scene: Option<String>,
    /// Map to spawn on (engine-specific; no default).
    #[serde(rename = "entryMap", default, skip_serializing_if = "Option::is_none")]
    pub entry_map: Option<String>,
    /// Root-relative directory of story scenes (default: "assets/scenes").
    #[serde(rename = "scenesDir", default, skip_serializing_if = "Option::is_none")]
    pub scenes_dir: Option<String>,
}

// ── battle section ──────────────────────────────────────────────────────────

/// Default record field holding a combatant's skill id list.
pub const DEFAULT_SKILLS_FIELD: &str = "skills";
/// Default skill-record field holding the skill category.
pub const DEFAULT_CATEGORY_FIELD: &str = "type";
/// Default skill-record field holding the resource (MP) cost.
pub const DEFAULT_COST_FIELD: &str = "mpCost";
/// Default rules file (project-root-relative), used when `battle.rules` is
/// absent; parsed only when the file exists.
pub const DEFAULT_RULES_FILE: &str = "data/rules.ron";

/// Default record field making an item battle-usable (heal amount).
pub const DEFAULT_HEAL_FIELD: &str = "healHp";

/// The optional top-level `battle` section: how project data tables map onto
/// the generic battle system. Every key is optional; the defaults match
/// the documented schema (see `docs/reference/project-manifest.md`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BattleSection {
    /// The player's party table (`{ "table": "<id>" }`); ALL records of the
    /// table form the party (sorted by record id), with switching in battle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub party: Option<BattleTableRef>,
    /// The enemy table; `startBattle("<id>")` names a record in it (a single
    /// wild enemy) when the id is not an encounter record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enemies: Option<BattleTableRef>,
    /// The encounters table (enemy parties + trainer battles): when set AND
    /// `startBattle("<id>")` names a record in it, the battle runs against
    /// the encounter's ordered enemy list (a queue), with its trainer flag
    /// and money reward. Absent ⇒ every battle is a single wild enemy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encounters: Option<BattleTableRef>,
    /// The skills table + field-name mapping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<BattleSkills>,
    /// Stat field mapping: stat role → record field name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<BattleStats>,
    /// Record field holding the combatant's resource pool (e.g. `"mp"`);
    /// absent ⇒ no resource gate (every skill is free).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    /// Project-root-relative rules file (dotzuki-rules `Ruleset` RON). Only the
    /// `type_chart` is consumed in v1. Default: `data/rules.ron` (used only
    /// when the file exists).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<String>,
    /// Battle-usable items (v2-b); absent ⇒ no Item menu in battle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<BattleItems>,
    /// EXP/level growth (v2-c); absent ⇒ no EXP is earned and records'
    /// `level` fields only feed RON level-ops (v1 behavior, unchanged).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<BattleLevels>,
}

/// The battle items block: the items table, which record field makes an item
/// battle-usable (a positive heal amount), and the starting inventory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleItems {
    /// Table id of the items table.
    pub table: String,
    /// Item record field holding the heal amount; an item is battle-usable
    /// iff its record has a positive number there (default `"healHp"`).
    /// Free-text `effect` fields are display-only.
    #[serde(rename = "healField", default = "default_heal_field")]
    pub heal_field: String,
    /// The starting inventory (record id → count), applied at first boot.
    #[serde(default)]
    pub starting: HashMap<String, u32>,
}

fn default_heal_field() -> String {
    DEFAULT_HEAL_FIELD.to_string()
}

// ── levels (EXP / stat growth) ────────────────────────────────────────────────

/// Default enemy-record field holding the EXP reward.
pub const DEFAULT_EXP_FIELD: &str = "exp";
/// Default combatant-record field holding its starting level.
pub const DEFAULT_LEVEL_FIELD: &str = "level";
/// Default stat growth per level above 1 (+5%).
pub const DEFAULT_GROWTH: f64 = 0.05;
/// Default level cap.
pub const DEFAULT_MAX_LEVEL: u32 = 100;
/// Default exp-curve base (`exp_to_next(L) = base × L^exponent`).
pub const DEFAULT_CURVE_BASE: u32 = 8;
/// Default exp-curve exponent.
pub const DEFAULT_CURVE_EXPONENT: u32 = 3;

/// The battle `levels` block (v2-c): EXP rewards and level growth. Every key
/// is optional (the defaults shown in the spec); an absent block keeps the v1
/// behavior exactly — no EXP is earned, stats never grow, and a record's
/// `level` field only feeds level-based RON ops/predicates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleLevels {
    /// ENEMY record field holding the EXP reward (0 when absent on a record).
    #[serde(rename = "expField", default = "default_exp_field")]
    pub exp_field: String,
    /// Combatant record field holding its starting level (default 1).
    #[serde(rename = "levelField", default = "default_level_field")]
    pub level_field: String,
    /// The exp curve (`exp_to_next(L) = base × L^exponent`, integer).
    #[serde(default)]
    pub curve: ExpCurve,
    /// Stat growth per level above 1: effective stat =
    /// `floor(raw × (1 + growth × (level − 1)))`.
    #[serde(default = "default_growth")]
    pub growth: f64,
    /// Level cap (level-ups stop here; EXP keeps accumulating).
    #[serde(rename = "maxLevel", default = "default_max_level")]
    pub max_level: u32,
}

impl Default for BattleLevels {
    fn default() -> Self {
        Self {
            exp_field: default_exp_field(),
            level_field: default_level_field(),
            curve: ExpCurve::default(),
            growth: DEFAULT_GROWTH,
            max_level: DEFAULT_MAX_LEVEL,
        }
    }
}

/// The exp curve: `exp_to_next(L) = base × L^exponent` (integer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpCurve {
    #[serde(default = "default_curve_base")]
    pub base: u32,
    #[serde(default = "default_curve_exponent")]
    pub exponent: u32,
}

impl Default for ExpCurve {
    fn default() -> Self {
        Self {
            base: DEFAULT_CURVE_BASE,
            exponent: DEFAULT_CURVE_EXPONENT,
        }
    }
}

fn default_exp_field() -> String {
    DEFAULT_EXP_FIELD.to_string()
}
fn default_level_field() -> String {
    DEFAULT_LEVEL_FIELD.to_string()
}
fn default_growth() -> f64 {
    DEFAULT_GROWTH
}
fn default_max_level() -> u32 {
    DEFAULT_MAX_LEVEL
}
fn default_curve_base() -> u32 {
    DEFAULT_CURVE_BASE
}
fn default_curve_exponent() -> u32 {
    DEFAULT_CURVE_EXPONENT
}

/// A `{ "table": "<id>" }` reference to a data table of the data activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleTableRef {
    /// Table id (matched against the data activity's `config.tables[].id`).
    pub table: String,
}

/// The skills table reference + the record field names the battle reads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleSkills {
    /// Table id of the skills table.
    pub table: String,
    /// Combatant record field listing skill ids (default `"skills"`).
    #[serde(default = "default_skills_field")]
    pub field: String,
    /// Skill record field holding the category (default `"type"`).
    #[serde(rename = "categoryField", default = "default_category_field")]
    pub category_field: String,
    /// Skill record field holding the resource cost (default `"mpCost"`).
    #[serde(rename = "costField", default = "default_cost_field")]
    pub cost_field: String,
}

fn default_skills_field() -> String {
    DEFAULT_SKILLS_FIELD.to_string()
}
fn default_category_field() -> String {
    DEFAULT_CATEGORY_FIELD.to_string()
}
fn default_cost_field() -> String {
    DEFAULT_COST_FIELD.to_string()
}

/// Stat role → record field name. Defaults: hp/atk/def/spd.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleStats {
    #[serde(default = "default_hp_field")]
    pub hp: String,
    #[serde(default = "default_atk_field")]
    pub attack: String,
    #[serde(default = "default_def_field")]
    pub defense: String,
    #[serde(default = "default_spd_field")]
    pub speed: String,
}

impl Default for BattleStats {
    fn default() -> Self {
        Self {
            hp: default_hp_field(),
            attack: default_atk_field(),
            defense: default_def_field(),
            speed: default_spd_field(),
        }
    }
}

fn default_hp_field() -> String {
    "hp".to_string()
}
fn default_atk_field() -> String {
    "atk".to_string()
}
fn default_def_field() -> String {
    "def".to_string()
}
fn default_spd_field() -> String {
    "spd".to_string()
}

// ── shop section ────────────────────────────────────────────────────────────

/// Default currency label when the manifest has no `shop` section.
pub const DEFAULT_CURRENCY: &str = "G";
/// Default starting money when the manifest has no `shop` section.
pub const DEFAULT_START_MONEY: u32 = 100;

/// The optional top-level `shop` section: the currency label and the money
/// the player starts with. Both keys optional; the defaults are `G` / `100`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopSection {
    /// Currency label shown next to amounts (shop UI, Bag). Default `"G"`.
    #[serde(default = "default_currency")]
    pub currency: String,
    /// Money a fresh game starts with. Default `100`.
    #[serde(rename = "startMoney", default = "default_start_money")]
    pub start_money: u32,
}

fn default_currency() -> String {
    DEFAULT_CURRENCY.to_string()
}
fn default_start_money() -> u32 {
    DEFAULT_START_MONEY
}

/// A data-table definition lifted from the data activity's `config.tables[]`
/// (`{ id, dir, fields[] }`; field entries may be strings or objects — the
/// editor writes `{ "key": …, "type": … }`, hand-grown projects may use
/// `{ "id": … }`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDef {
    /// Table id (what the battle section's `table` values name).
    pub id: String,
    /// Records directory, relative to `dataRoot` (`<dir>/<record>.json`).
    pub dir: String,
    /// Field ids declared by the table schema.
    pub fields: Vec<String>,
}

impl Manifest {
    /// Every data table declared by the `data` activity (`config.tables[]`).
    pub fn data_tables(&self) -> Vec<TableDef> {
        let mut out = Vec::new();
        for activity in &self.activities {
            if activity.kind != "data" {
                continue;
            }
            let Some(tables) = activity.config.get("tables").and_then(|v| v.as_array()) else {
                continue;
            };
            for table in tables {
                let (Some(id), Some(dir)) = (
                    table.get("id").and_then(|v| v.as_str()),
                    table.get("dir").and_then(|v| v.as_str()),
                ) else {
                    continue;
                };
                let fields = table
                    .get("fields")
                    .and_then(|v| v.as_array())
                    .map(|fs| {
                        fs.iter()
                            .filter_map(|f| {
                                f.as_str().map(str::to_string).or_else(|| {
                                    // Editor schemas use `key`; some projects use `id`.
                                    f.get("key")
                                        .or_else(|| f.get("id"))
                                        .and_then(|v| v.as_str())
                                        .map(str::to_string)
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                out.push(TableDef {
                    id: id.to_string(),
                    dir: dir.to_string(),
                    fields,
                });
            }
        }
        out
    }

    /// The data table with id `table_id`, if declared.
    pub fn data_table(&self, table_id: &str) -> Option<TableDef> {
        self.data_tables().into_iter().find(|t| t.id == table_id)
    }
}

impl Manifest {
    /// Scene directory: `game.scenesDir`, or the default when absent.
    pub fn scenes_dir(&self) -> &str {
        self.game
            .as_ref()
            .and_then(|g| g.scenes_dir.as_deref())
            .unwrap_or(DEFAULT_SCENES_DIR)
    }

    /// Every directory that may hold DSL files, resolved against `root`:
    /// the [`dsl_dirs_rel`](Self::dsl_dirs_rel) paths joined onto `root`.
    ///
    /// Missing directories are kept in the list; callers decide how to treat
    /// them (`compile_dirs` silently skips them).
    pub fn dsl_dirs(&self, root: &Path) -> Vec<PathBuf> {
        self.dsl_dirs_rel().iter().map(|d| root.join(d)).collect()
    }

    /// Every directory that may hold DSL files, as project-relative POSIX
    /// paths (the VFS form of [`dsl_dirs`](Self::dsl_dirs)):
    ///
    /// - the scene directory (`game.scenesDir`, root-relative);
    /// - each `script` activity's `scriptsDir` (dataRoot-relative);
    /// - each `story` activity's `scenesDir` (dataRoot-relative, default "maps");
    /// - each `ui` activity's `guiRoot` (root-relative).
    pub fn dsl_dirs_rel(&self) -> Vec<String> {
        let data_root = crate::vfs::join_path("", &self.data_root);
        let mut dirs = vec![crate::vfs::join_path("", self.scenes_dir())];
        for activity in &self.activities {
            match activity.kind.as_str() {
                "script" => {
                    if let Some(dir) = config_str(&activity.config, "scriptsDir") {
                        dirs.push(crate::vfs::join_path(&data_root, dir));
                    }
                }
                "story" => {
                    let dir = config_str(&activity.config, "scenesDir")
                        .unwrap_or(DEFAULT_STORY_SCENES_DIR);
                    dirs.push(crate::vfs::join_path(&data_root, dir));
                }
                "ui" => {
                    if let Some(dir) = config_str(&activity.config, "guiRoot") {
                        dirs.push(crate::vfs::join_path("", dir));
                    }
                }
                _ => {}
            }
        }
        let mut seen = HashSet::new();
        dirs.retain(|d| seen.insert(d.clone()));
        dirs
    }
}

fn config_str<'a>(config: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    config.get(key).and_then(|v| v.as_str())
}
