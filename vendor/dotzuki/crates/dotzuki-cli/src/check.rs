//! `dotzuki check` — compile every DSL file in a game project (in memory) and
//! report diagnostics. Exit code 0 when clean, 1 when any diagnostic fires.
//!
//! When the manifest carries a `battle` section, it is validated too: the
//! referenced table ids must exist in the data activity, the referenced
//! stat/skill fields must exist in the table schemas, an explicitly declared
//! rules file must exist on disk, and the rules file — whenever it exists —
//! must parse as a dotzuki-rules `Ruleset`. Record JSONs are not loaded — the
//! manifest's table definitions are enough.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use dotzuki_engine_dsl::compiler::{compile_dirs, CompileReport};
use dotzuki_runner::manifest::{Manifest, TableDef, DEFAULT_RULES_FILE};

/// A project's full diagnostic picture: the parsed manifest, the DSL dirs that
/// were compiled, the compile report, and the battle-section diagnostics.
/// Shared by `dotzuki check` (prints and exits) and `dotzuki export --web`
/// (blocks the export unless `--force`).
pub struct ProjectDiagnostics {
    pub manifest: Manifest,
    pub dsl_dirs: Vec<PathBuf>,
    pub report: CompileReport,
    pub battle_diags: Vec<String>,
}

impl ProjectDiagnostics {
    /// Total diagnostic count (DSL compile + battle section).
    pub fn total(&self) -> usize {
        self.report.diagnostics.len() + self.battle_diags.len()
    }

    /// Every diagnostic message, DSL-compile first then battle-section.
    pub fn all(&self) -> impl Iterator<Item = &String> {
        self.report.diagnostics.iter().chain(self.battle_diags.iter())
    }
}

/// Load the project manifest and compute all diagnostics without printing.
pub fn diagnose(dir: &Path) -> Result<ProjectDiagnostics> {
    let manifest_path = dir.join(".dotzuki-editor.json");
    if !manifest_path.is_file() {
        bail!(
            "No .dotzuki-editor.json found in {} — not a jrpg game project",
            dir.display()
        );
    }
    let text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: Manifest = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;

    let dirs = manifest.dsl_dirs(dir);
    let dir_refs: Vec<&Path> = dirs.iter().map(|d| d.as_path()).collect();
    let report = compile_dirs(&dir_refs, None);
    let battle_diags = battle_diagnostics(&manifest, dir);
    Ok(ProjectDiagnostics {
        manifest,
        dsl_dirs: dirs,
        report,
        battle_diags,
    })
}

pub fn run(dir: &Path) -> Result<()> {
    let diags = diagnose(dir)?;

    println!("Project: {} ({})", diags.manifest.name, dir.display());
    println!("DSL dirs:");
    for d in &diags.dsl_dirs {
        if d.is_dir() {
            println!("  {}", d.display());
        } else {
            println!("  {} (missing, skipped)", d.display());
        }
    }
    println!(
        "Compiled: {} scene(s), {} layout(s), {} theme(s), {} style(s) from {} file(s)",
        diags.report.scenes.len(),
        diags.report.ui_layouts.len(),
        diags.report.themes.len(),
        diags.report.styles.len(),
        diags.report.files.len()
    );

    let total = diags.total();
    if total == 0 {
        println!("OK — no diagnostics");
        return Ok(());
    }
    for diag in diags.all() {
        eprintln!("error: {}", diag);
    }
    eprintln!("check failed: {} diagnostic(s)", total);
    std::process::exit(1);
}

/// Validate the manifest's `battle` section against the data activity's table
/// definitions and the rules file. Empty when there is no battle section (or
/// the section is valid).
fn battle_diagnostics(manifest: &Manifest, root: &Path) -> Vec<String> {
    let Some(battle) = &manifest.battle else {
        return Vec::new();
    };
    let mut diags = Vec::new();

    // Referenced table ids must exist in the data activity.
    let party = battle
        .party
        .as_ref()
        .and_then(|r| resolve_table(manifest, "battle.party.table", &r.table, &mut diags));
    let enemies = battle
        .enemies
        .as_ref()
        .and_then(|r| resolve_table(manifest, "battle.enemies.table", &r.table, &mut diags));
    let skills = battle
        .skills
        .as_ref()
        .and_then(|s| resolve_table(manifest, "battle.skills.table", &s.table, &mut diags));
    let items = battle
        .items
        .as_ref()
        .and_then(|i| resolve_table(manifest, "battle.items.table", &i.table, &mut diags));
    let encounters = battle
        .encounters
        .as_ref()
        .and_then(|e| resolve_table(manifest, "battle.encounters.table", &e.table, &mut diags));

    // The mapped stat fields must exist in the combatant table schemas.
    let stats = battle.stats.clone().unwrap_or_default();
    for (what, table) in [("party", &party), ("enemies", &enemies)] {
        let Some(table) = table.as_ref() else {
            continue;
        };
        for (role, field) in [
            ("hp", &stats.hp),
            ("attack", &stats.attack),
            ("defense", &stats.defense),
            ("speed", &stats.speed),
        ] {
            require_field(
                &mut diags,
                &format!("battle.stats.{role}"),
                field.as_str(),
                table,
                what,
            );
        }
        // The combatant resource field.
        if let Some(resource) = &battle.resource {
            require_field(
                &mut diags,
                "battle.resource",
                resource.as_str(),
                table,
                what,
            );
        }
        // The combatant skill-list field.
        let skills_field = battle
            .skills
            .as_ref()
            .map(|s| s.field.as_str())
            .unwrap_or(dotzuki_runner::manifest::DEFAULT_SKILLS_FIELD);
        if battle.skills.is_some() {
            require_field(&mut diags, "battle.skills.field", skills_field, table, what);
        }
    }

    // The skill-record fields must exist in the skills table schema.
    if let (Some(skill_cfg), Some(table)) = (&battle.skills, &skills) {
        require_field(
            &mut diags,
            "battle.skills.categoryField",
            skill_cfg.category_field.as_str(),
            table,
            "skills",
        );
        require_field(
            &mut diags,
            "battle.skills.costField",
            skill_cfg.cost_field.as_str(),
            table,
            "skills",
        );
    }

    // The item heal field must exist in the items table schema.
    if let (Some(item_cfg), Some(table)) = (&battle.items, &items) {
        require_field(
            &mut diags,
            "battle.items.healField",
            item_cfg.heal_field.as_str(),
            table,
            "items",
        );
    }

    // The encounters table schema must declare the `enemies` list field
    // (encounter records are `{ id, name, enemies[], trainer, money }`).
    if let Some(table) = &encounters {
        require_field(
            &mut diags,
            "battle.encounters",
            "enemies",
            table,
            "encounters",
        );
    }

    // The rules file, when the manifest names one explicitly, must exist on
    // disk — a missing file would silently leave battles with an empty type
    // chart at runtime. Whenever the file exists, it must parse as a Ruleset
    // AND compile against the closed vocabulary (unknown events/ops/stat/
    // type/resource/status names in hooks are load-time errors, never
    // mid-battle). An absent default file (battle.rules unset) is legal: such
    // projects run without a type chart.
    let rules_rel = battle.rules.as_deref().unwrap_or(DEFAULT_RULES_FILE);
    let rules_path = root.join(rules_rel.strip_prefix("./").unwrap_or(rules_rel));
    if rules_path.is_file() {
        match fs::read_to_string(&rules_path) {
            Ok(text) => {
                for d in dotzuki_runner::validate_ruleset(&text) {
                    diags.push(format!("battle.rules '{}': {d}", rules_path.display()));
                }
            }
            Err(e) => diags.push(format!("failed to read {}: {e}", rules_path.display())),
        }
    } else if battle.rules.is_some() {
        diags.push(format!(
            "battle.rules '{}' not found — battles would run with an empty type chart",
            rules_path.display()
        ));
    }

    diags
}

/// Resolve a referenced table id to its definition, recording a diagnostic
/// when the id names no declared data table.
fn resolve_table(
    manifest: &Manifest,
    key: &str,
    table_id: &str,
    diags: &mut Vec<String>,
) -> Option<TableDef> {
    let table = manifest.data_table(table_id);
    if table.is_none() {
        diags.push(format!("{key} '{table_id}' is not a declared data table"));
    }
    table
}

/// Record a diagnostic when `field` is not declared in the table schema.
fn require_field(diags: &mut Vec<String>, key: &str, field: &str, table: &TableDef, what: &str) {
    if !table.fields.iter().any(|f| f == field) {
        diags.push(format!(
            "{key} '{field}' is not a field of the {what} table '{}'",
            table.id
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT_ID: AtomicU32 = AtomicU32::new(0);

    /// Unique temp directory, removed on drop.
    struct TestDir(std::path::PathBuf);

    impl TestDir {
        fn new(test: &str) -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!(
                "dotzuki-cli-check-{test}-{}-{id}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            TestDir(dir)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    // Mirrors the editor's real manifest shape: field entries are objects
    // with `key`/`type` (string fields are also accepted for hand-grown
    // projects — covered by the runner's manifest unit tests).
    const TABLES: &str = r#"[
        { "id": "heroes", "dir": "heroes",
          "fields": [ {"key": "name", "type": "string"}, {"key": "hp", "type": "number"},
                      {"key": "atk", "type": "number"}, {"key": "def", "type": "number"},
                      {"key": "spd", "type": "number"}, {"key": "mp", "type": "number"},
                      {"key": "skills", "type": "array"} ] },
        { "id": "monsters", "dir": "monsters",
          "fields": [ {"key": "name"}, {"key": "hp"}, {"key": "atk"}, {"key": "def"},
                      {"key": "spd"}, {"key": "mp"}, {"key": "skills"} ] },
        { "id": "spells", "dir": "spells",
          "fields": ["name", "type", "power", "mpCost"] },
        { "id": "items", "dir": "items",
          "fields": ["name", "healHp", "effect"] },
        { "id": "encounters", "dir": "encounters",
          "fields": ["name", "enemies", "trainer", "money"] }
    ]"#;

    /// Write a project with a data activity (`TABLES`) and a battle section
    /// assembled from the given JSON fragments.
    fn write_project(
        test: &str,
        battle: serde_json::Value,
        rules_ron: Option<&str>,
    ) -> (TestDir, Manifest) {
        let tmp = TestDir::new(test);
        let root = tmp.0.join("proj");
        fs::create_dir_all(&root).unwrap();
        let manifest = serde_json::json!({
            "name": "proj",
            "dataRoot": "./data",
            "activities": [
                { "id": "data", "type": "data", "config": { "tables": serde_json::from_str::<serde_json::Value>(TABLES).unwrap() } }
            ],
            "battle": battle,
        });
        fs::write(
            root.join(".dotzuki-editor.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        if let Some(ron) = rules_ron {
            fs::create_dir_all(root.join("data")).unwrap();
            fs::write(root.join("data/rules.ron"), ron).unwrap();
        }
        let parsed: Manifest = serde_json::from_value(manifest).unwrap();
        (tmp, parsed)
    }

    fn valid_battle() -> serde_json::Value {
        serde_json::json!({
            "party": { "table": "heroes" },
            "enemies": { "table": "monsters" },
            "skills": { "table": "spells" },
            "stats": { "hp": "hp", "attack": "atk", "defense": "def", "speed": "spd" },
            "resource": "mp",
        })
    }

    #[test]
    fn valid_battle_section_passes() {
        let (tmp, manifest) = write_project(
            "valid",
            valid_battle(),
            Some("Ruleset(types: [\"fire\"], type_chart: [])"),
        );
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn no_battle_section_is_not_checked() {
        let tmp = TestDir::new("none");
        let manifest: Manifest = serde_json::from_value(serde_json::json!({
            "name": "proj",
            "dataRoot": "./data",
        }))
        .unwrap();
        assert!(battle_diagnostics(&manifest, tmp.0.as_path()).is_empty());
    }

    #[test]
    fn unknown_table_id_is_a_diagnostic() {
        let mut battle = valid_battle();
        battle["enemies"] = serde_json::json!({ "table": "beasts" });
        let (tmp, manifest) = write_project("badtable", battle, None);
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(
            diags
                .iter()
                .any(|d| d.contains("beasts") && d.contains("battle.enemies.table")),
            "{diags:?}"
        );
    }

    #[test]
    fn unknown_stat_field_is_a_diagnostic() {
        let mut battle = valid_battle();
        battle["stats"] = serde_json::json!({ "attack": "attack" }); // tables declare "atk"
        let (tmp, manifest) = write_project("badstat", battle, None);
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(
            diags
                .iter()
                .any(|d| d.contains("battle.stats.attack") && d.contains("attack")),
            "{diags:?}"
        );
    }

    #[test]
    fn unknown_skill_cost_field_is_a_diagnostic() {
        let mut battle = valid_battle();
        battle["skills"] = serde_json::json!({ "table": "spells", "costField": "cost" });
        let (tmp, manifest) = write_project("badcost", battle, None);
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(
            diags
                .iter()
                .any(|d| d.contains("battle.skills.costField") && d.contains("cost")),
            "{diags:?}"
        );
    }

    #[test]
    fn valid_items_block_passes() {
        let mut battle = valid_battle();
        battle["items"] = serde_json::json!({ "table": "items", "healField": "healHp", "starting": { "potion": 3 } });
        let (tmp, manifest) = write_project("gooditems", battle, None);
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn unknown_items_table_is_a_diagnostic() {
        let mut battle = valid_battle();
        battle["items"] = serde_json::json!({ "table": "goods" });
        let (tmp, manifest) = write_project("baditemtable", battle, None);
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(
            diags
                .iter()
                .any(|d| d.contains("goods") && d.contains("battle.items.table")),
            "{diags:?}"
        );
    }

    #[test]
    fn unknown_items_heal_field_is_a_diagnostic() {
        let mut battle = valid_battle();
        battle["items"] = serde_json::json!({ "table": "items", "healField": "healAmount" });
        let (tmp, manifest) = write_project("badhealfield", battle, None);
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(
            diags
                .iter()
                .any(|d| d.contains("battle.items.healField") && d.contains("healAmount")),
            "{diags:?}"
        );
    }

    #[test]
    fn valid_encounters_block_passes() {
        let mut battle = valid_battle();
        battle["encounters"] = serde_json::json!({ "table": "encounters" });
        let (tmp, manifest) = write_project("goodencounters", battle, None);
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn unknown_encounters_table_is_a_diagnostic() {
        let mut battle = valid_battle();
        battle["encounters"] = serde_json::json!({ "table": "trainers" });
        let (tmp, manifest) = write_project("badenctable", battle, None);
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(
            diags
                .iter()
                .any(|d| d.contains("trainers") && d.contains("battle.encounters.table")),
            "{diags:?}"
        );
    }

    #[test]
    fn encounters_table_without_enemies_field_is_a_diagnostic() {
        // "monsters" exists but declares no `enemies` list field.
        let mut battle = valid_battle();
        battle["encounters"] = serde_json::json!({ "table": "monsters" });
        let (tmp, manifest) = write_project("badencfield", battle, None);
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(
            diags.iter().any(|d| d.contains("battle.encounters")
                && d.contains("enemies")
                && d.contains("monsters")),
            "{diags:?}"
        );
    }

    #[test]
    fn malformed_rules_ron_is_a_diagnostic() {
        let (tmp, manifest) = write_project("badron", valid_battle(), Some("Ruleset(types: ["));
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(
            diags
                .iter()
                .any(|d| d.contains("battle.rules") && d.contains("rules.ron")),
            "{diags:?}"
        );
    }

    #[test]
    fn declared_missing_rules_path_is_a_diagnostic() {
        let mut battle = valid_battle();
        battle["rules"] = serde_json::json!("data/typo.ron");
        let (tmp, manifest) = write_project("missingrules", battle, None);
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(
            diags
                .iter()
                .any(|d| d.contains("battle.rules") && d.contains("typo.ron")),
            "{diags:?}"
        );
    }

    #[test]
    fn undeclared_rules_with_missing_default_file_passes() {
        // battle.rules unset + no data/rules.ron on disk is a legal project
        // (battles run with an empty type chart) — never a diagnostic.
        let (tmp, manifest) = write_project("nodefault", valid_battle(), None);
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn declared_existing_rules_path_validates_content() {
        let mut battle = valid_battle();
        battle["rules"] = serde_json::json!("data/rules.ron");
        let (tmp, manifest) = write_project("declaredbad", battle, Some("Ruleset(types: ["));
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(
            diags
                .iter()
                .any(|d| d.contains("battle.rules") && d.contains("rules.ron")),
            "{diags:?}"
        );
    }

    #[test]
    fn valid_hooks_pass() {
        let ron = r#"Ruleset(
            stats: ["hp", "attack", "defense", "speed"],
            resources: ["mp"],
            effects: [
                Effect(id: "venom-sting", kind: Move, power: 15, hooks: [
                    Hook(on: "DamagingHit", chance: [30, 100], do: [
                        InflictStatus(status: "poison", target: Target),
                    ]),
                ]),
                Effect(id: "poison", kind: Status, hooks: [
                    Hook(on: "Residual", do: [
                        DamageFraction(num: 1, den: 8, of: MaxHp, target: Target),
                    ]),
                ]),
            ],
        )"#;
        let (tmp, manifest) = write_project("goodhooks", valid_battle(), Some(ron));
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn unknown_hook_names_are_diagnostics() {
        // An unknown op stat name…
        let (tmp, manifest) = write_project(
            "badstatname",
            valid_battle(),
            Some(
                r#"Ruleset(
                    stats: ["attack"],
                    effects: [Effect(id: "x", kind: Move, hooks: [
                        Hook(on: "DamagingHit", do: [Boost(stat: "speed", stages: 1, target: Source)]),
                    ])],
                )"#,
            ),
        );
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(
            diags
                .iter()
                .any(|d| d.contains("battle.rules") && d.contains("speed")),
            "{diags:?}"
        );

        // …an unknown event name…
        let (tmp, manifest) = write_project(
            "badevent",
            valid_battle(),
            Some(r#"Ruleset(effects: [Effect(id: "x", kind: Move, hooks: [Hook(on: "OnHit")])])"#),
        );
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(
            diags
                .iter()
                .any(|d| d.contains("battle.rules") && d.contains("OnHit")),
            "{diags:?}"
        );

        // …an unknown chart type name…
        let (tmp, manifest) = write_project(
            "badtype",
            valid_battle(),
            Some(
                r#"Ruleset(types: ["fire"], type_chart: [(atk: "fire", def: "grass", mult: [2, 1])])"#,
            ),
        );
        let diags = battle_diagnostics(&manifest, &tmp.0.join("proj"));
        assert!(
            diags
                .iter()
                .any(|d| d.contains("battle.rules") && d.contains("grass")),
            "{diags:?}"
        );
    }
}
