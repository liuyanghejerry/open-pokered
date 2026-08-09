//! scene_apply <MapName> [<MapName>...]
//!
//! For each map: compile its `script.scene` to JS (fail with the error if it
//! doesn't compile) and regenerate `script_config.json` FROM the `.scene` so the
//! runtime binding contract stays in lock-step with the source. Used by the
//! story-translation workflow so each translated map self-verifies + keeps its
//! config consistent (config round-trip then passes).
//!
//!   cargo run -p scene_apply -- PewterGym CeruleanGym

use jrpg_engine_dsl::compiler::compile_scene_to_js;
use jrpg_engine_dsl::config_gen::compile_scene_to_config;
use std::path::PathBuf;
use std::process::exit;

fn maps_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("pokered")
        .join("crates")
        .join("pokered-data")
        .join("maps")
}

fn main() {
    let names: Vec<String> = std::env::args().skip(1).collect();
    if names.is_empty() {
        eprintln!("usage: scene_apply <MapName> [<MapName>...]");
        exit(2);
    }
    let dir = maps_dir();
    let mut failed = false;
    for name in &names {
        let scene = dir.join(name).join("script.scene");
        let cfg = dir.join(name).join("script_config.json");
        let src = match std::fs::read_to_string(&scene) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{name}: cannot read {}: {e}", scene.display());
                failed = true;
                continue;
            }
        };
        let rel = format!("{name}/script.scene");
        // 1. compile-check
        if let Err(e) = compile_scene_to_js(&src, &rel) {
            eprintln!("{name}: COMPILE ERROR: {e}");
            failed = true;
            continue;
        }
        // 2. regenerate config from the .scene
        match compile_scene_to_config(&src, &rel) {
            Ok(json) => {
                let mut json = json;
                if !json.ends_with('\n') {
                    json.push('\n');
                }
                if let Err(e) = std::fs::write(&cfg, json) {
                    eprintln!("{name}: cannot write config: {e}");
                    failed = true;
                    continue;
                }
                println!("{name}: ok");
            }
            Err(e) => {
                eprintln!("{name}: CONFIG GEN ERROR: {e}");
                failed = true;
            }
        }
    }
    if failed {
        exit(1);
    }
}
