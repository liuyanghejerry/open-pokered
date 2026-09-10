//! `dotzuki export --web` — pack a zero-Rust game project into a static web
//! directory that any static file server can host:
//!
//! ```text
//! <out>/
//! ├── index.html                      (player page: canvas + input + audio + saves)
//! ├── game.dzpk                       (binary pack: index + raw file bytes)
//! └── wasm/
//!     ├── dotzuki_runner_web.js       (wasm-pack glue)
//!     └── dotzuki_runner_web_bg.wasm  (the runner itself)
//! ```
//!
//! The page boots the same `WasmRunner` (dotzuki-runner-web) the editor's Play
//! activity uses — via `WasmRunner.fromPack`, so no base64 is involved — so an
//! exported game plays identically to the in-editor playtest. Pipeline:
//! validate (`dotzuki check` diagnostics; `--force` overrides) → collect the
//! pack → locate/build the runner wasm package → write the three artifacts.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::{bundle, check, runner_pkg};

/// CLI arguments for `dotzuki export --web`.
pub struct ExportArgs {
    /// Project root containing `.dotzuki-editor.json`.
    pub dir: PathBuf,
    /// Output directory (default: `<project>/dist/web` — `dist` is excluded
    /// from bundles, so re-exporting never packs a previous export).
    pub out: Option<PathBuf>,
    /// Prebuilt dotzuki-runner-web wasm package directory override.
    pub runner_pkg: Option<PathBuf>,
    /// Rebuild the wasm package with wasm-pack even when a prebuilt one exists.
    pub rebuild_runner: bool,
    /// Export despite DSL/battle diagnostics.
    pub force: bool,
    /// localStorage key the player page persists saves under (default
    /// `dotzuki-save:<title>`). Hosts embedding the export (dotzuki-cloud)
    /// pin their own key to keep existing players' saves valid.
    pub save_key: Option<String>,
    /// Player page UI language (`en` / `zh`) — loading/status/hint strings.
    pub lang: String,
}

const INDEX_TEMPLATE: &str = include_str!("../templates/web-player.html");

/// Export the project and return the output directory.
pub fn run(args: &ExportArgs) -> Result<PathBuf> {
    // 1. Validate — the same diagnostics `dotzuki check` reports.
    let diags = gate_diagnostics(&args.dir, args.force)?;

    // 2. Bundle the project files.
    let files = bundle::collect_project_files(&args.dir).context("failed to collect project files")?;

    // 3. Locate (or wasm-pack build) the runner package.
    let pkg = runner_pkg::locate(args.runner_pkg.as_deref(), args.rebuild_runner)?;

    // 4. Write the static site.
    let out = args
        .out
        .clone()
        .unwrap_or_else(|| args.dir.join("dist").join("web"));
    write_site(
        &out,
        &diags.manifest.name,
        args.save_key.as_deref(),
        &args.lang,
        &files,
        &pkg,
    )?;

    let bundle_bytes = fs::metadata(out.join(bundle::PACK_FILE)).map(|m| m.len()).unwrap_or(0);
    println!(
        "exported {} file(s) ({:.1} MiB pack) to {}",
        files.len(),
        bundle_bytes as f64 / (1024.0 * 1024.0),
        out.display()
    );
    println!("serve it with any static file server, e.g.:");
    println!("  python3 -m http.server --directory {}", out.display());
    Ok(out)
}

/// Validate a project for export: the same diagnostics `dotzuki check`
/// reports, blocking the export unless `force` — shared by every export
/// target (`--web`, `--native`).
pub fn gate_diagnostics(dir: &Path, force: bool) -> Result<check::ProjectDiagnostics> {
    let diags = check::diagnose(dir)?;
    if diags.total() > 0 {
        for d in diags.all() {
            eprintln!("error: {d}");
        }
        if !force {
            bail!(
                "export aborted: {} diagnostic(s) — fix them (see `dotzuki check`) or pass --force",
                diags.total()
            );
        }
        eprintln!(
            "warning: exporting despite {} diagnostic(s) (--force)",
            diags.total()
        );
    }
    Ok(diags)
}

/// Write `index.html` + `game.dzpk` + `wasm/*` into `out`.
fn write_site(
    out: &Path,
    title: &str,
    save_key: Option<&str>,
    lang: &str,
    files: &BTreeMap<String, Vec<u8>>,
    pkg: &Path,
) -> Result<()> {
    let wasm_dir = out.join("wasm");
    fs::create_dir_all(&wasm_dir)
        .with_context(|| format!("failed to create {}", wasm_dir.display()))?;
    for file in [runner_pkg::JS_FILE, runner_pkg::WASM_FILE] {
        fs::copy(pkg.join(file), wasm_dir.join(file))
            .with_context(|| format!("failed to copy {file} from {}", pkg.display()))?;
    }

    fs::write(out.join(bundle::PACK_FILE), bundle::serialize_pack(files))
        .context("failed to write game.dzpk")?;

    let key = save_key
        .map(str::to_string)
        .unwrap_or_else(|| default_save_key(title));
    let html = render_index_html(title, &key, lang);
    fs::write(out.join("index.html"), html).context("failed to write index.html")?;
    Ok(())
}

/// The default localStorage key the player page persists saves under.
fn default_save_key(title: &str) -> String {
    format!("dotzuki-save:{title}")
}

/// Player-page UI strings for a `lang` (`en` default, `zh` available).
/// Hint may contain HTML entities (it is injected as markup); the status
/// strings are plain text. All are authored constants — never user input.
struct PageStrings {
    loading: &'static str,
    loading_runtime: &'static str,
    downloading: &'static str,
    starting: &'static str,
    failed_prefix: &'static str,
    hint: &'static str,
}

fn page_strings(lang: &str) -> PageStrings {
    match lang {
        "zh" => PageStrings {
            loading: "加载中…",
            loading_runtime: "加载运行时…",
            downloading: "下载游戏包…",
            starting: "启动中…",
            failed_prefix: "加载失败: ",
            hint: "方向键 / WASD — 移动 &nbsp;·&nbsp; Z — A &nbsp;·&nbsp; X — B &nbsp;·&nbsp; Enter — Start &nbsp;·&nbsp; Backspace — Select &nbsp;·&nbsp; M — 静音",
        },
        _ => PageStrings {
            loading: "Loading…",
            loading_runtime: "Loading runtime…",
            downloading: "Downloading game…",
            starting: "Starting…",
            failed_prefix: "Failed to load: ",
            hint: "Arrows / WASD — move &nbsp;·&nbsp; Z — A &nbsp;·&nbsp; X — B &nbsp;·&nbsp; Enter — Start &nbsp;·&nbsp; Backspace — Select &nbsp;·&nbsp; M — mute",
        },
    }
}

/// Fill the player-page template. The title is HTML-escaped into `<title>` and
/// `<h1>`; the save key and the status strings are JSON-encoded (valid JS
/// string literals); the hint is authored markup, injected raw.
fn render_index_html(title: &str, save_key: &str, lang: &str) -> String {
    let escaped = html_escape(title);
    let key_json = serde_json::to_string(save_key).unwrap_or_else(|_| "\"dotzuki-save\"".into());
    let s = page_strings(lang);
    let js = |v: &str| serde_json::to_string(v).unwrap();
    INDEX_TEMPLATE
        .replace("__DOTZUKI_TITLE__", &escaped)
        .replace("__DOTZUKI_SAVE_KEY__", &key_json)
        .replace("__DOTZUKI_STR_LOADING__", s.loading)
        .replace("__DOTZUKI_STR_LOADING_RUNTIME__", &js(s.loading_runtime))
        .replace("__DOTZUKI_STR_DOWNLOADING__", &js(s.downloading))
        .replace("__DOTZUKI_STR_STARTING__", &js(s.starting))
        .replace("__DOTZUKI_STR_FAILED_PREFIX__", &js(s.failed_prefix))
        .replace("__DOTZUKI_STR_HINT__", s.hint)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_runner::vfs::ProjectFiles as _;
    use dotzuki_runner::{run_headless, HeadlessOptions, LoadedProject, RunnerGame, RunnerOptions};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    static NEXT_ID: AtomicU32 = AtomicU32::new(0);

    /// Unique temp directory, removed on drop.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new(test: &str) -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!(
                "dotzuki-cli-export-{test}-{}-{id}",
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

    /// A fake runner wasm package (two placeholder files) for tests that must
    /// not depend on wasm-pack.
    fn fake_runner_pkg(test: &str) -> TestDir {
        let tmp = TestDir::new(test);
        fs::write(tmp.0.join(runner_pkg::JS_FILE), "// test stub").unwrap();
        fs::write(tmp.0.join(runner_pkg::WASM_FILE), b"\0asm").unwrap();
        tmp
    }

    /// The tutorial project shipped in this repo.
    fn example_project() -> Option<PathBuf> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/your-first-game");
        dir.is_dir().then_some(dir)
    }

    fn args(dir: PathBuf, out: PathBuf, pkg: &Path, force: bool) -> ExportArgs {
        ExportArgs {
            dir,
            out: Some(out),
            runner_pkg: Some(pkg.to_path_buf()),
            rebuild_runner: false,
            force,
            save_key: None,
            lang: "en".to_string(),
        }
    }

    #[test]
    fn export_produces_a_bootable_site() {
        let Some(project) = example_project() else {
            eprintln!("skipping: examples/your-first-game not present");
            return;
        };
        let pkg = fake_runner_pkg("site-pkg");
        let tmp = TestDir::new("site");
        let out = export_dir(&tmp);
        let produced = run(&args(project, out.clone(), &pkg.0, false)).unwrap();
        assert_eq!(produced, out);

        // The three artifacts exist.
        let html = fs::read_to_string(out.join("index.html")).unwrap();
        let pack_bytes = fs::read(out.join(bundle::PACK_FILE)).unwrap();
        assert!(out.join("wasm").join(runner_pkg::JS_FILE).is_file());
        assert!(out.join("wasm").join(runner_pkg::WASM_FILE).is_file());

        // The page wired the manifest name into title and save key.
        assert!(html.contains("Your First Game"), "{html}");
        assert!(html.contains("dotzuki-save:Your First Game"), "{html}");

        // The pack boots through the exact WasmRunner.fromPack path: parse it
        // into PackFiles and drive a few frames headless.
        let pack = dotzuki_runner::pack::PackFiles::from_bytes(pack_bytes).unwrap();
        assert!(pack.exists(".dotzuki-editor.json"));
        let project = LoadedProject::load_with_files(Arc::new(pack)).unwrap();
        let mut game = RunnerGame::new(
            project,
            RunnerOptions {
                headless: true,
                pcm_audio: true,
                fresh: true,
                ..RunnerOptions::default()
            },
        )
        .unwrap();
        run_headless(&mut game, &HeadlessOptions {
            frames: 30,
            ..HeadlessOptions::default()
        })
        .unwrap();
    }

    #[test]
    fn save_key_and_lang_customize_the_player_page() {
        let Some(project) = example_project() else {
            eprintln!("skipping: examples/your-first-game not present");
            return;
        };
        let pkg = fake_runner_pkg("custom-pkg");
        let tmp = TestDir::new("custom");
        let mut a = args(project, export_dir(&tmp), &pkg.0, false);
        a.save_key = Some("dotzuki-cloud-save:my-game".to_string());
        a.lang = "zh".to_string();
        let out = run(&a).unwrap();

        let html = fs::read_to_string(out.join("index.html")).unwrap();
        // The embedding host's save key lands as a JS string literal…
        assert!(html.contains("\"dotzuki-cloud-save:my-game\""), "{html}");
        // …the page speaks Chinese…
        assert!(html.contains("加载中…"), "{html}");
        assert!(html.contains("加载失败"), "{html}");
        assert!(html.contains("静音"), "{html}");
        // …and every template placeholder is filled.
        assert!(!html.contains("__DOTZUKI_"), "unreplaced placeholder: {html}");
    }

    #[test]
    fn default_page_is_english_with_the_title_save_key() {
        let Some(project) = example_project() else {
            eprintln!("skipping: examples/your-first-game not present");
            return;
        };
        let pkg = fake_runner_pkg("default-pkg");
        let tmp = TestDir::new("default");
        let out = run(&args(project, export_dir(&tmp), &pkg.0, false)).unwrap();
        let html = fs::read_to_string(out.join("index.html")).unwrap();
        assert!(html.contains("Loading…"), "{html}");
        assert!(html.contains("\"dotzuki-save:Your First Game\""), "{html}");
        assert!(!html.contains("__DOTZUKI_"), "unreplaced placeholder: {html}");
    }

    #[test]
    fn dsl_errors_block_export_unless_forced() {
        let tmp = TestDir::new("broken");
        let root = tmp.0.join("proj");
        fs::create_dir_all(root.join("assets/scenes")).unwrap();
        fs::write(
            root.join(".dotzuki-editor.json"),
            r#"{ "name": "broken", "dataRoot": "./data" }"#,
        )
        .unwrap();
        fs::write(root.join("assets/scenes/bad.scene"), "this is not a scene {{{").unwrap();

        let pkg = fake_runner_pkg("broken-pkg");
        let err = run(&args(root.clone(), export_dir(&tmp), &pkg.0, false)).unwrap_err();
        assert!(err.to_string().contains("--force"), "{err}");

        // --force exports anyway.
        let out = run(&args(root, export_dir(&tmp), &pkg.0, true)).unwrap();
        assert!(out.join(bundle::PACK_FILE).is_file());
    }

    #[test]
    fn missing_project_is_an_error() {
        let tmp = TestDir::new("missing");
        let pkg = fake_runner_pkg("missing-pkg");
        let err = run(&args(tmp.0.join("nope"), export_dir(&tmp), &pkg.0, false)).unwrap_err();
        assert!(err.to_string().contains(".dotzuki-editor.json"), "{err}");
    }

    fn export_dir(tmp: &TestDir) -> PathBuf {
        tmp.0.join(format!("out-{}", NEXT_ID.fetch_add(1, Ordering::SeqCst)))
    }
}
