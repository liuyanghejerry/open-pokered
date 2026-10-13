//! Snapshot tests for DSL compilation outputs.
//!
//! Each test compiles a DSL string through the full pipeline
//! and captures the compiled JS/JSON as a snapshot using the `insta` crate.
//!
//! Run with: `cargo test -p dotzuki-engine-dsl --test compiler_snapshots`
//!
//! First run auto-accepts snapshots with `INSTA_UPDATE=new`.
//! Use `cargo insta review` to review changes.

use std::fs;

use dotzuki_engine_dsl::compiler::{compile_dsl_file, CompileOutput};

// ── helpers ──────────────────────────────────────────────────────────────

/// Compile a DSL scene string and return the generated JavaScript.
/// Uses a unique temp directory per test to avoid cross-test contamination.
fn compile_scene(dsl: &str, name: &str) -> String {
    let out_dir = std::env::temp_dir().join("dsl_snap").join(name);
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).unwrap();

    let ext = "scene";
    let result = compile_dsl_file(ext, dsl, &format!("{}.scene", name), &out_dir)
        .expect("Scene compilation should succeed");

    match result {
        CompileOutput::Scene { js, .. } => js,
        _ => panic!("Expected Scene output, got another variant"),
    }
}

/// Clean and recreate a temp dir for tests that read generated JSON files.
fn setup_temp_dir(name: &str) -> std::path::PathBuf {
    let out_dir = std::env::temp_dir().join("dsl_snap").join(name);
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).unwrap();
    out_dir
}

// ═══════════════════════════════════════════════════════════════════════════
// Snapshot tests (8)
// ═══════════════════════════════════════════════════════════════════════════

/// 1. Shop scene — speakers, choice, and variable mutation in JS.
#[test]
fn snapshot_shop_scene_js() {
    let dsl = r#"
game_scene Shop {
    @variables { gold = 500 }
    @storylines {
        @speaker("Shopkeeper") { "Welcome!" "What would you like?" }
        @choice {
            @option("Buy") { gold = gold - 100 }
            @option("Leave") { }
        }
    }
}
"#;
    let js = compile_scene(dsl, "shop_scene");
    insta::assert_snapshot!("shop_scene_js", js);
}

/// 2. Theme block — verify the generated theme JSON file.
#[test]
fn snapshot_theme_json() {
    let dsl = r###"
game_scene Colors {
    @theme default {
        primary = "#c9a03d"
        background = "#1a1a2e"
    }
}
"###;
    let out_dir = setup_temp_dir("theme_test");
    let _result = compile_dsl_file("scene", dsl, "colors.scene", &out_dir)
        .expect("Theme compilation should succeed");
    let theme_path = out_dir.join("Colors_theme_0.json");
    let json = fs::read_to_string(&theme_path).expect("Theme JSON file should exist");
    insta::assert_snapshot!("theme_json", json);
}

/// 3. Atlas with 9-slice region — verify the generated atlas JSON.
#[test]
fn snapshot_atlas_slice_json() {
    let dsl = r#"
game_scene Atlas {
    @atlas "ui" {
        source = "atlas.png"
        regions = {
            btn = [0, 0, 64, 64, slice=8]
        }
    }
}
"#;
    let out_dir = setup_temp_dir("atlas_test");
    let _result = compile_dsl_file("scene", dsl, "atlas.scene", &out_dir)
        .expect("Atlas compilation should succeed");
    let atlas_path = out_dir.join("Atlas_atlas_0.json");
    let json = fs::read_to_string(&atlas_path).expect("Atlas JSON file should exist");
    insta::assert_snapshot!("atlas_slice_json", json);
}

/// 4. UI panel with identified components — verify the generated UI JSON.
#[test]
fn snapshot_ui_panel_json() {
    let dsl = r#"
game_scene ShopUI {
    ui {
        panel {
            title = text("Shop") { }
            buy_btn = button("Buy") { on_click = "buy_item" }
        }
    }
}
"#;
    let out_dir = setup_temp_dir("ui_test");
    let _result = compile_dsl_file("scene", dsl, "shop_ui.scene", &out_dir)
        .expect("UI compilation should succeed");
    let ui_path = out_dir.join("ShopUI_ui.json");
    let json = fs::read_to_string(&ui_path).expect("UI JSON file should exist");
    insta::assert_snapshot!("ui_panel_json", json);
}

/// 5. Broken scene — capture parse error via Debug snapshot.
#[test]
fn snapshot_broken_scene_error() {
    let dsl = "game_scene Broken { @@invalid }";
    let out_dir = setup_temp_dir("error_test");
    let result = compile_dsl_file("scene", dsl, "broken.scene", &out_dir);
    insta::assert_debug_snapshot!("broken_scene_error", result);
}

/// 6. Nested choice — choice-within-choice produces indented JS branches.
#[test]
fn snapshot_nested_choice_js() {
    let dsl = r#"
game_scene Nested {
    @storylines {
        @choice {
            @option("A") {
                @choice {
                    @option("A1") { }
                    @option("A2") { }
                }
            }
            @option("B") { }
        }
    }
}
"#;
    let js = compile_scene(dsl, "nested_choice");
    insta::assert_snapshot!("nested_choice_js", js);
}

/// 7. Variable interpolation — inline `{gold}` and `{name}` in speaker text.
#[test]
fn snapshot_variables_inline_js() {
    let dsl = r#"
game_scene Vars {
    @variables {
        gold = 500
        name = "RED"
    }
    @storylines {
        @speaker("Prof") { "You have {gold} gold, {name}!" }
    }
}
"#;
    let js = compile_scene(dsl, "vars_test");
    insta::assert_snapshot!("variables_inline_js", js);
}

/// 8. Empty scene — minimal valid scene produces minimal JS with source map.
#[test]
fn snapshot_empty_scene_js() {
    let dsl = "game_scene Empty {}";
    let js = compile_scene(dsl, "empty_scene");
    insta::assert_snapshot!("empty_scene_js", js);
}
