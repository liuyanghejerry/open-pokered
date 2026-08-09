//! DSL-as-source ↔ runtime-contract no-drift guarantee.
//!
//! Every map's `.scene` is the single source of truth for routing/binding data
//! (`@trigger`). This test regenerates each map's `script_config.json` FROM its
//! `.scene` and asserts it reproduces the committed config's bindings
//! (npcs / signs / coordEvents). If they ever diverge — someone edited a
//! `.scene` trigger without regenerating, or vice-versa — this test fails.
//!
//! `onLoad` is excluded from *this* binding comparison: the 240-odd maps whose
//! `@load {}` body is empty keep a legacy no-op name (`enterMap`) that compiles
//! to a different name (`<Scene>OnLoad`) — harmless, since the body is empty.
//! Maps with a *non-empty* `@load` (where the handler must actually fire) are
//! guarded instead by `scene_onload_resolves_at_runtime` below, which asserts
//! the committed `onLoad` equals the compiled `<Scene>OnLoad` export. All other
//! binding data must match exactly.

use jrpg_engine_dsl::compiler::compile_scene_to_js;
use jrpg_engine_dsl::config_gen::{compile_scene_to_config, normalize_config};
use jrpg_engine_dsl::{ast, lexer, parser};
use serde_json::Value;
use std::path::PathBuf;

fn maps_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("maps")
}

/// Normalize a config and drop `onLoad`/`coordEvents` so only npc/sign bindings remain.
///
/// `coordEvents` are one-way committed→runtime: the `.scene` no longer carries
/// `coords` (position lives in the map JSON alone), so regenerated config has
/// an empty array and can never match the committed array. We skip it here.
fn bindings_only(v: &Value) -> Value {
    let mut n = normalize_config(v);
    if let Some(obj) = n.as_object_mut() {
        obj.remove("onLoad");
        obj.remove("coordEvents");
    }
    n
}

#[test]
fn scene_regenerates_committed_bindings() {
    let dir = maps_dir();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("maps dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    entries.sort();

    let mut checked = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for path in entries {
        if !path.is_dir() {
            continue;
        }
        let scene = path.join("script.scene");
        let cfg = path.join("script_config.json");
        if !scene.is_file() || !cfg.is_file() {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(&scene).unwrap();

        let generated = match compile_scene_to_config(&src, &format!("{}/script.scene", name)) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{name}: compile error: {e}"));
                continue;
            }
        };
        let gen_v: Value = serde_json::from_str(&generated).unwrap();
        let committed_v: Value =
            serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();

        checked += 1;
        let g = bindings_only(&gen_v);
        let c = bindings_only(&committed_v);
        if g != c {
            failures.push(format!(
                "{name}: bindings differ\n   committed: {c}\n   generated: {g}"
            ));
        }
    }

    assert!(checked > 200, "expected >200 maps checked, got {checked}");
    assert!(
        failures.is_empty(),
        "{} / {} maps failed the .scene→config binding round-trip:\n{}",
        failures.len(),
        checked,
        failures.join("\n")
    );
    eprintln!("config round-trip OK for {checked} maps");
}

/// Parse a `.scene` source into its `GameScene` AST (or an error string).
fn parse_scene(src: &str, path: &str) -> Result<ast::GameScene, String> {
    let tokens = lexer::Lexer::new(src, path).tokenize().map_err(|errs| {
        errs.iter()
            .map(|e| format!("{}:{}: {}", e.line, e.col, e.message))
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let (doc, parse_errors, semantic_errors) = parser::parse_and_validate(tokens, src);
    if !parse_errors.is_empty() {
        return Err(parse_errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; "));
    }
    if !semantic_errors.is_empty() {
        return Err(semantic_errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; "));
    }
    match doc.ok_or_else(|| "parser returned no document".to_string())? {
        ast::Document::Scene(scene) => Ok(scene),
        _ => Err("not a .scene document".to_string()),
    }
}

/// Map-entry (`@load`) handlers must actually fire at runtime.
///
/// The overworld loads a map and calls `script_engine.has_function(cfg.onLoad)`
/// (overworld/update.rs); if the name isn't an export of the compiled `.scene`
/// module it logs "on_load function '…' not found in module" and the handler is
/// silently skipped. A `.scene`'s `@load` block compiles to `<Scene>OnLoad`
/// (js_storyline.rs), so the committed `script_config.json` `onLoad` MUST equal
/// that name — otherwise the map-entry script is dead.
///
/// This regression actually shipped: the migration renamed the compiled fn to
/// PascalCase `<Scene>OnLoad` while the configs kept legacy names
/// (`palletTownOnLoad`, `enterMap`, `scriptDefault`), so the early-game story
/// maps' onLoad never fired. This test guards it.
///
/// Empty `@load {}` bodies are exempt: they compile to a no-op, so a legacy
/// unresolved name is harmless. Only maps with a NON-EMPTY `@load` are checked.
#[test]
fn scene_onload_resolves_at_runtime() {
    let dir = maps_dir();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("maps dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    entries.sort();

    let mut checked = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for path in entries {
        if !path.is_dir() {
            continue;
        }
        let scene = path.join("script.scene");
        let cfg = path.join("script_config.json");
        if !scene.is_file() || !cfg.is_file() {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(&scene).unwrap();

        let parsed = match parse_scene(&src, &format!("{}/script.scene", name)) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{name}: parse error: {e}"));
                continue;
            }
        };

        // Only a non-empty @load body has logic that must fire.
        let has_real_onload = parsed
            .on_load
            .as_ref()
            .map(|b| !b.statements.is_empty())
            .unwrap_or(false);
        if !has_real_onload {
            continue;
        }

        checked += 1;
        let expected = format!("{}OnLoad", parsed.name);

        let committed: Value =
            serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        let committed_onload = committed.get("onLoad").and_then(|v| v.as_str());
        if committed_onload != Some(expected.as_str()) {
            failures.push(format!(
                "{name}: config onLoad = {committed_onload:?} but the compiled @load fn is \
                 {expected:?} — runtime has_function() will miss it, so the map-entry script \
                 won't fire. Set onLoad to {expected:?}."
            ));
        }

        // Belt-and-suspenders: the compiled module must actually export it.
        match compile_scene_to_js(&src, &format!("{}/script.scene", name)) {
            Ok(js) => {
                let needle = format!("export async function {}(", expected);
                if !js.contains(&needle) {
                    failures.push(format!("{name}: compiled JS does not export `{expected}`"));
                }
            }
            Err(e) => failures.push(format!("{name}: compile error: {e}")),
        }
    }

    assert!(
        checked >= 5,
        "expected at least the 5 story maps with a non-empty @load, got {checked}"
    );
    assert!(
        failures.is_empty(),
        "{} map(s) have an onLoad that won't resolve at runtime:\n{}",
        failures.len(),
        failures.join("\n")
    );
    eprintln!("onLoad resolves for all {checked} maps with a non-empty @load body");
}
