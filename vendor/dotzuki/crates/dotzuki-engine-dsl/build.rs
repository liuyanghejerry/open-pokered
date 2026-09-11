//! Build script for `dotzuki-engine-dsl`: discovers DSL files across the
//! workspace's built-in search dirs, compiles them, and embeds the results.
//!
//! The compiler modules below are `include!`d wholesale from `src/` so the
//! build script runs the exact same code as the library, but the script only
//! drives the batch pipeline (`compile_dirs` & friends) — most of the API is
//! unused in this compilation unit, hence the crate-level `dead_code` allow.
#![allow(dead_code)]

use std::env;
use std::path::Path;

mod ast {
    include!("src/ast.rs");
}
mod hash {
    include!("src/hash.rs");
}
mod lexer {
    include!("src/lexer.rs");
}
mod parser {
    include!("src/parser.rs");
}
mod sourcemap {
    include!("src/sourcemap.rs");
}
mod codegen {
    pub mod i18n {
        include!("src/codegen/i18n.rs");
    }
    pub mod js_storyline {
        include!("src/codegen/js_storyline.rs");
    }
    pub mod js_variables {
        include!("src/codegen/js_variables.rs");
    }
    pub mod json_ui {
        include!("src/codegen/json_ui.rs");
    }
    pub mod json_theme {
        include!("src/codegen/json_theme.rs");
    }
    pub mod json_atlas {
        include!("src/codegen/json_atlas.rs");
    }
}
mod compiler {
    include!("src/compiler.rs");
}
mod conflict {
    include!("src/conflict.rs");
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = Path::new(&manifest_dir);

    // Extra search dirs from DOTZUKI_DSL_DIRS (":"-separated absolute paths)
    // take precedence over the built-in monorepo locations.
    println!("cargo:rerun-if-env-changed=DOTZUKI_DSL_DIRS");
    let search_dirs =
        compiler::merge_search_dirs(env::var("DOTZUKI_DSL_DIRS").ok().as_deref(), manifest_path);

    let dsl_out_dir = out_path.join("dsl");
    let dir_refs: Vec<&Path> = search_dirs.iter().map(|d| d.as_path()).collect();
    let report = compiler::compile_dirs(&dir_refs, Some(&dsl_out_dir));

    for (_, path) in &report.files {
        println!("cargo:rerun-if-changed={}", path);
    }
    for dir in &search_dirs {
        if dir.exists() {
            println!("cargo:rerun-if-changed={}", dir.display());
        }
    }

    compiler::generate_embedded_module(
        out_path,
        &report.scenes,
        &report.ui_layouts,
        &report.themes,
        &report.styles,
    );
    compiler::generate_routing_table(out_path, &report.routes);

    for diagnostic in &report.diagnostics {
        println!("cargo:warning={}", diagnostic);
    }
}
