//! scene_check <file.scene>
//!
//! Compile-check a single `.scene` file WITHOUT side effects — no config
//! regeneration, no hardcoded project directory (unlike `scene_apply`, which
//! targets the pokered maps and rewrites `script_config.json`). The machine-
//! readable result goes to STDOUT ("OK: …" or "COMPILE ERROR: …") and the exit
//! code signals status (0 = compiles, 1 = does not, 2 = bad usage/IO), so a
//! caller can discard STDERR — which carries this workspace's build.rs rebuild
//! warnings — with `2>/dev/null` and still get a clean result. Editors point
//! their draft-check command at this (dotzuki-editor `scene.checkCmd`), so the AI
//! assistant can verify a draft before proposing it.
//!
//!   cargo run -q -p dotzuki-engine-dsl --bin scene_check -- path/to/script.scene

use dotzuki_engine_dsl::compiler::compile_scene_to_js;
use std::process::exit;

fn main() {
    let path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            println!("usage: scene_check <file.scene>");
            exit(2);
        }
    };
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            println!("cannot read {path}: {e}");
            exit(2);
        }
    };
    match compile_scene_to_js(&src, &path) {
        Ok(_) => println!("OK: scene compiles."),
        Err(e) => {
            // to STDOUT so `2>/dev/null` keeps the real error but drops build noise
            println!("COMPILE ERROR: {e}");
            exit(1);
        }
    }
}
