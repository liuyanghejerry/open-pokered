//! End-to-end tests for the `dotzuki` binary: scaffold a project into a temp
//! dir, then compile-check it. Uses only std (no assert_cmd/tempfile — the
//! workspace has no such dev-dependency convention).

use crate::alloc_prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

/// Unique temp directory, removed on drop.
struct TestDir(PathBuf);

impl TestDir {
    fn new(test: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("dotzuki-cli-{test}-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        TestDir(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn dotzuki(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dotzuki"))
        .args(args)
        .output()
        .expect("failed to run dotzuki binary")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Scaffold a default project named `name` under `parent`; asserts success.
fn scaffold(parent: &Path, name: &str) -> PathBuf {
    let out = dotzuki(&["new", name, "--dir", parent.to_str().unwrap()]);
    assert!(out.status.success(), "dotzuki new failed: {}", stderr(&out));
    parent.join(name)
}

#[test]
fn new_creates_expected_tree_and_manifest() {
    let tmp = TestDir::new("tree");
    let project = scaffold(tmp.path(), "my-game");

    for entry in [
        ".dotzuki-editor.json",
        "data/maps",
        "data/tiles",
        "gfx",
        "assets/scenes/main.scene",
        "README.md",
    ] {
        assert!(project.join(entry).exists(), "missing {}", entry);
    }

    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(project.join(".dotzuki-editor.json")).unwrap())
            .unwrap();

    assert_eq!(manifest["name"], "my-game");
    assert_eq!(manifest["dataRoot"], "./data");
    assert_eq!(manifest["gfxRoot"], "./gfx");

    // Same seven activities the editor scaffolds, in the same order.
    let activities = manifest["activities"].as_array().unwrap();
    let ids: Vec<&str> = activities
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        ["maps", "scripts", "play", "data", "story", "assets", "tiles"]
    );
    let by_id = |id: &str| activities.iter().find(|a| a["id"] == id).unwrap().clone();
    assert_eq!(by_id("maps")["type"], "map");
    assert_eq!(by_id("maps")["config"]["mapsDir"], "maps");
    assert_eq!(by_id("scripts")["type"], "script");
    assert_eq!(by_id("scripts")["config"]["scriptsDir"], "maps");
    assert_eq!(by_id("scripts")["config"]["extension"], ".scene");
    assert_eq!(by_id("play")["type"], "play");
    assert_eq!(by_id("data")["type"], "data");
    assert_eq!(
        by_id("data")["config"]["tables"].as_array().unwrap().len(),
        0
    );
    assert_eq!(by_id("story")["type"], "story");
    assert_eq!(by_id("story")["config"]["storiesDir"], "stories");
    assert_eq!(by_id("story")["config"]["scenesDir"], "maps");
    assert_eq!(by_id("assets")["type"], "assets");
    assert_eq!(by_id("assets")["config"]["roots"][0], "gfx");
    assert_eq!(by_id("tiles")["type"], "tiles");
    assert_eq!(by_id("tiles")["config"]["tilesDir"], "tiles");
    assert_eq!(by_id("tiles")["config"]["tileSize"], 16);
    assert_eq!(by_id("tiles")["config"]["backdropMapsDir"], "maps");

    // Engine-facing section added by the CLI.
    assert_eq!(manifest["game"]["entryScene"], "main");
    assert_eq!(manifest["game"]["scenesDir"], "assets/scenes");

    // Starter scene matches the editor's template structure.
    let scene = fs::read_to_string(project.join("assets/scenes/main.scene")).unwrap();
    assert!(scene.contains("game_scene Main {"));
    assert!(scene.contains("@storylines {"));
    assert!(scene.contains("@speaker(\"Guide\")"));
}

#[test]
fn new_honors_title_override() {
    let tmp = TestDir::new("title");
    let out = dotzuki(&[
        "new",
        "my-game",
        "--dir",
        tmp.path().to_str().unwrap(),
        "--title",
        "My Cool Game",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(tmp.path().join("my-game/.dotzuki-editor.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["name"], "My Cool Game");
}

#[test]
fn new_rejects_non_slug_names() {
    let tmp = TestDir::new("slug");
    for bad in ["My Game", "Bad", "bad_name", ""] {
        let out = dotzuki(&["new", bad, "--dir", tmp.path().to_str().unwrap()]);
        assert!(!out.status.success(), "expected failure for '{}'", bad);
        assert!(stderr(&out).contains("invalid project name"), "{}", bad);
    }
}

#[test]
fn new_rejects_non_empty_target() {
    let tmp = TestDir::new("nonempty");
    let target = tmp.path().join("my-game");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("keep.txt"), "occupied").unwrap();

    let out = dotzuki(&["new", "my-game", "--dir", tmp.path().to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("already exists and is not empty"));
    assert_eq!(
        fs::read_to_string(target.join("keep.txt")).unwrap(),
        "occupied"
    );
}

#[test]
fn new_scaffolds_into_existing_empty_dir() {
    let tmp = TestDir::new("emptydir");
    fs::create_dir_all(tmp.path().join("my-game")).unwrap();
    let project = scaffold(tmp.path(), "my-game");
    assert!(project.join(".dotzuki-editor.json").is_file());
}

#[test]
fn check_passes_on_fresh_project() {
    let tmp = TestDir::new("checkok");
    let project = scaffold(tmp.path(), "my-game");

    let out = dotzuki(&["check", project.to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("1 scene(s)"), "{}", text);
    assert!(text.contains("OK"), "{}", text);
}

#[test]
fn check_fails_without_manifest() {
    let tmp = TestDir::new("nomanifest");
    let out = dotzuki(&["check", tmp.path().to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("No .dotzuki-editor.json found"));
}

#[test]
fn check_reports_dsl_errors_with_exit_1() {
    let tmp = TestDir::new("checkbad");
    let project = scaffold(tmp.path(), "my-game");
    fs::write(
        project.join("assets/scenes/broken.scene"),
        "game_scene Broken {\n    @storylines {\n        @@@ not valid dsl @@@\n    }\n}\n",
    )
    .unwrap();

    let out = dotzuki(&["check", project.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let combined = format!("{}{}", stdout(&out), stderr(&out));
    assert!(combined.contains("broken.scene"), "{}", combined);
    assert!(combined.contains("diagnostic"), "{}", combined);
}
