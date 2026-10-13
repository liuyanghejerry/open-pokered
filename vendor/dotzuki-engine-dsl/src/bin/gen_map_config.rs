//! Regenerate every map's `script_config.json` FROM its `.scene` source.
//!
//! This realizes the "DSL is the single source of truth" design: the `.scene`
//! carries all routing/binding data via `@trigger`, and the runtime contract
//! (`script_config.json`) is a generated artifact. A round-trip test then keeps
//! them from drifting.
//!
//! Usage: `cargo run -p dotzuki-engine-dsl --bin gen_map_config -- <maps_dir>`

use dotzuki_engine_dsl::config_gen::compile_scene_to_config;
use std::fs;
use std::path::PathBuf;

fn main() {
    let maps_dir = match std::env::args().nth(1) {
        Some(d) => PathBuf::from(d),
        None => {
            eprintln!("usage: gen_map_config <maps_dir>");
            std::process::exit(2);
        }
    };

    let mut entries: Vec<PathBuf> = fs::read_dir(&maps_dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {}", maps_dir.display(), e))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    entries.sort();

    let (mut ok, mut fail) = (0u32, 0u32);
    for path in entries {
        if !path.is_dir() {
            continue;
        }
        let scene = path.join("script.scene");
        if !scene.is_file() {
            continue;
        }
        let src = match fs::read_to_string(&scene) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("read {}: {}", scene.display(), e);
                fail += 1;
                continue;
            }
        };
        let fname = format!(
            "{}/script.scene",
            path.file_name().unwrap().to_string_lossy()
        );
        match compile_scene_to_config(&src, &fname) {
            Ok(json) => {
                fs::write(path.join("script_config.json"), format!("{}\n", json))
                    .expect("write config");
                ok += 1;
            }
            Err(e) => {
                eprintln!("FAIL {}: {}", path.display(), e);
                fail += 1;
            }
        }
    }
    println!("gen_map_config: {} ok, {} failed", ok, fail);
    if fail > 0 {
        std::process::exit(1);
    }
}
