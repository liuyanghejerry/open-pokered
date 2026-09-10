//! `dotzuki new` — lay out a fresh zero-Rust game project.
//!
//! The layout mirrors the editor's scaffolder
//! (`tools/dotzuki-editor/server/scaffold.ts`, "empty" template) plus the `game`
//! section from `docs/reference/project-manifest.md`, so projects round-trip: the
//! editor opens `dotzuki new` output, and editor-wizard projects pass
//! `dotzuki check`. `--template your-first-game` writes the embedded tutorial
//! project (`templates::YOUR_FIRST_GAME`, a byte-for-byte copy of
//! `examples/your-first-game/`).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::json;

use crate::manifest::{Activity, GameSection, Manifest};
use crate::templates;

// A minimal scene so a fresh project has something openable on day one.
// Same structure as the editor's starter scene.
const MAIN_SCENE: &str = r#"// main.scene — your game's first scene, written in the jrpg Game DSL.
// Scenes compile to JavaScript and run on the dotzuki-engine runtime; ask the
// in-editor AI assistant (✨) to sketch characters, quests and scenes for you.

game_scene Main {
    @storylines {
        @speaker("Guide") {
            "Welcome to your new JRPG project!"
            "Edit assets/scenes/main.scene to make it yours."
        }
    }
}
"#;

fn readme(title: &str) -> String {
    format!(
        r#"# {title}

A JRPG project created with `dotzuki new`.

## Layout

- `.dotzuki-editor.json` — editor project config (activities, data roots)
- `data/` — game data: maps, data tables, the shared tile library, the
  narrative bible (`data/stories/`)
- `gfx/` — graphics assets (tilesets, sprites)
- `assets/scenes/` — Game DSL scene scripts (`.scene`)

## Editing

Reopen this folder from the editor's welcome screen (**Open Project**), or
start the editor with `DOTZUKI_PROJECT_ROOT=<this folder>`.

## Checking

Run `dotzuki check <this folder>` to compile-check every DSL file in the project.
"#
    )
}

/// Activity list written into `.dotzuki-editor.json` — the same six the editor
/// scaffolds (map/script/data/story/assets/tiles), with the same config
/// shapes and order.
fn activities() -> Vec<Activity> {
    let activity =
        |id: &str, kind: &str, label: &str, icon: &str, config: serde_json::Value| Activity {
            id: id.to_string(),
            kind: kind.to_string(),
            label: label.to_string(),
            icon: icon.to_string(),
            enabled: true,
            config,
        };
    vec![
        activity("maps", "map", "Maps", "map", json!({ "mapsDir": "maps" })),
        activity(
            "scripts",
            "script",
            "Scripts",
            "code",
            json!({ "scriptsDir": "maps", "extension": ".scene" }),
        ),
        activity("play", "play", "Play", "play", json!({})),
        activity("data", "data", "Data", "database", json!({ "tables": [] })),
        activity(
            "story",
            "story",
            "Story",
            "book",
            json!({ "storiesDir": "stories", "scenesDir": "maps", "locales": ["en", "zh"] }),
        ),
        activity(
            "assets",
            "assets",
            "Assets",
            "image",
            json!({ "roots": ["gfx"] }),
        ),
        activity(
            "tiles",
            "tiles",
            "Tiles",
            "tiles",
            json!({ "tilesDir": "tiles", "tileSize": 16, "backdropMapsDir": "maps" }),
        ),
    ]
}

fn manifest(title: &str) -> Manifest {
    Manifest {
        name: title.to_string(),
        data_root: "./data".to_string(),
        gfx_root: Some("./gfx".to_string()),
        activities: activities(),
        game: Some(GameSection {
            entry_scene: Some("main".to_string()),
            entry_map: None,
            scenes_dir: Some("assets/scenes".to_string()),
        }),
        battle: None,
        shop: None,
    }
}

/// Directory names must be slugs: `^[a-z0-9][a-z0-9-]*$`.
pub fn is_slug(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
}

/// Scaffold a new project `<parent>/<name>`; returns the project directory.
///
/// `template` selects the layout: `None` (or `"empty"`) writes the empty
/// starter project; `"your-first-game"` writes the embedded tutorial project
/// with the project name substituted into its manifest.
pub fn run(
    name: &str,
    parent: Option<&Path>,
    title: Option<&str>,
    template: Option<&str>,
) -> Result<PathBuf> {
    if !is_slug(name) {
        bail!(
            "invalid project name '{}': must match [a-z0-9][a-z0-9-]* \
             (use --title for a free-form display name)",
            name
        );
    }
    let parent = parent.unwrap_or_else(|| Path::new("."));
    let target = parent.join(name);
    if target.is_file() {
        bail!("target '{}' already exists and is a file", target.display());
    }
    if target.is_dir() && target.read_dir()?.next().is_some() {
        bail!(
            "target directory '{}' already exists and is not empty",
            target.display()
        );
    }

    let title = title.unwrap_or(name);
    match template.unwrap_or("empty") {
        "empty" => scaffold_empty(&target, &title)?,
        "your-first-game" => scaffold_template(&target, &title)?,
        other => bail!(
            "unknown template '{}': available templates are {}",
            other,
            templates::TEMPLATE_NAMES.join(", ")
        ),
    }

    println!(
        "Created new JRPG project '{}' in {}",
        title,
        target.display()
    );
    println!(
        "Next: open the folder in dotzuki-editor, or run `dotzuki check {}`.",
        target.display()
    );

    Ok(target)
}

/// The default "empty" template: data roots + gfx + the story-bible skeleton
/// + one starter scene + README.
fn scaffold_empty(target: &Path, title: &str) -> Result<()> {
    let manifest_json = serde_json::to_string_pretty(&manifest(title))?;

    // Same footprint as the editor's empty template.
    for dir in [
        "data/maps",
        "data/tiles",
        "data/stories/characters",
        "data/stories/quests",
        "data/stories/arcs",
        "gfx",
        "assets/scenes",
    ] {
        fs::create_dir_all(target.join(dir))
            .with_context(|| format!("failed to create {}/{}", target.display(), dir))?;
    }
    fs::write(target.join(".dotzuki-editor.json"), manifest_json)?;
    fs::write(target.join("assets/scenes/main.scene"), MAIN_SCENE)?;
    fs::write(target.join("README.md"), readme(title))?;

    println!("  .dotzuki-editor.json");
    println!("  data/maps/  data/tiles/  gfx/");
    println!("  assets/scenes/main.scene");
    println!("  README.md");
    Ok(())
}

/// Write the embedded `your-first-game` template into `target`, substituting
/// the project name into its manifest.
fn scaffold_template(target: &Path, title: &str) -> Result<()> {
    for (rel, bytes) in templates::YOUR_FIRST_GAME {
        let path = target.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        if *rel == ".dotzuki-editor.json" {
            let mut value: serde_json::Value =
                serde_json::from_slice(bytes).context("template manifest is not valid JSON")?;
            value["name"] = serde_json::Value::String(title.to_string());
            let text = serde_json::to_string_pretty(&value)?;
            fs::write(&path, text)
                .with_context(|| format!("failed to write {}", path.display()))?;
        } else {
            fs::write(&path, bytes)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT_ID: AtomicU32 = AtomicU32::new(0);

    /// Unique temp directory, removed on drop.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new(test: &str) -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!(
                "dotzuki-cli-scaffold-{test}-{}-{id}",
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

    fn collect_files(dir: &Path, base: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect_files(&path, base, out);
            } else {
                out.push(
                    path.strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }

    /// The vendored template must be a byte-for-byte copy of the canonical
    /// tutorial project. Skips (with a warning) when the examples directory
    /// is absent — e.g. a packaged crate build outside the repository.
    #[test]
    fn vendored_template_matches_examples() {
        let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/your-first-game");
        if !examples.is_dir() {
            eprintln!("skipping: examples/ not present next to the crate");
            return;
        }
        let mut missing = Vec::new();
        let mut drifted = Vec::new();
        for (rel, bytes) in templates::YOUR_FIRST_GAME {
            let src = examples.join(rel);
            match fs::read(&src) {
                Ok(d) if d == *bytes => {}
                Ok(_) => drifted.push(*rel),
                Err(_) => missing.push(*rel),
            }
        }
        let mut example_files = Vec::new();
        collect_files(&examples, &examples, &mut example_files);
        let vendored: Vec<&str> = templates::YOUR_FIRST_GAME.iter().map(|(r, _)| *r).collect();
        let extra: Vec<&String> = example_files
            .iter()
            .filter(|f| !vendored.contains(&f.as_str()))
            .collect();
        assert!(
            missing.is_empty(),
            "vendored files missing from examples/: {missing:?}"
        );
        assert!(
            drifted.is_empty(),
            "vendored files drifted from examples/: {drifted:?}"
        );
        assert!(extra.is_empty(), "example files not vendored: {extra:?}");
    }

    /// A scaffolded `your-first-game` copy must pass `dotzuki check`.
    #[test]
    fn your_first_game_template_scaffolds_and_checks() {
        let tmp = TestDir::new("tpl-check");
        let target = run("first-game", Some(&tmp.0), None, Some("your-first-game")).unwrap();
        assert!(target.join(".dotzuki-editor.json").is_file());
        assert!(target.join("data/maps/Hometown/tileset.png").is_file());
        assert!(target.join("assets/scenes/main.scene").is_file());
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(target.join(".dotzuki-editor.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["name"], "first-game");
        crate::check::run(&target).unwrap();
    }

    /// The empty template is still the default and still checks clean.
    #[test]
    fn empty_template_is_default_and_checks() {
        let tmp = TestDir::new("tpl-empty");
        let target = run("blank-game", Some(&tmp.0), None, None).unwrap();
        assert!(target.join("assets/scenes/main.scene").is_file());
        crate::check::run(&target).unwrap();
    }

    #[test]
    fn unknown_template_is_an_error() {
        let tmp = TestDir::new("tpl-bad");
        let err = run("x-game", Some(&tmp.0), None, Some("nope")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown template"), "{msg}");
        assert!(msg.contains("your-first-game"), "{msg}");
    }
}
