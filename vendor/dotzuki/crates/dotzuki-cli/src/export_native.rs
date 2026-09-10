//! `dotzuki export --native` — pack a zero-Rust game project into a native
//! app directory:
//!
//! ```text
//! <out>/
//! ├── <project-dir-name>[.exe]  (the dotzuki-player binary, renamed)
//! └── game.dzpk                 (binary pack: index + raw file bytes)
//! ```
//!
//! The player binary is game-agnostic: it boots the `game.dzpk` sitting next
//! to the executable through the same `RunnerGame` + `dotzuki-app` window
//! `dotzuki run` uses, and writes its save next to the pack as
//! `.dotzuki-save.json`. Pipeline mirrors the web export: validate (`dotzuki
//! check` diagnostics; `--force` overrides) → collect the pack → locate/build
//! the player binary → write the two artifacts.
//!
//! The build targets the host platform only — cross-compiling a Windows or
//! Linux app from another OS is out of scope (build on the target OS, or in
//! CI with one runner per OS).

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::{bundle, export, player};

/// CLI arguments for `dotzuki export --native`.
pub struct NativeExportArgs {
    /// Project root containing `.dotzuki-editor.json`.
    pub dir: PathBuf,
    /// Output directory (default: `<project>/dist/native` — `dist` is
    /// excluded from bundles, so re-exporting never packs a previous export).
    pub out: Option<PathBuf>,
    /// Prebuilt dotzuki-player binary override (skips the cargo build).
    pub player_bin: Option<PathBuf>,
    /// Export despite DSL/battle diagnostics.
    pub force: bool,
}

/// Export the project and return the output directory.
pub fn run(args: &NativeExportArgs) -> Result<PathBuf> {
    // 1. Validate — the same diagnostics `dotzuki check` reports.
    export::gate_diagnostics(&args.dir, args.force)?;

    // 2. Bundle the project files.
    let files = bundle::collect_project_files(&args.dir).context("failed to collect project files")?;

    // 3. Locate (or cargo build) the player binary.
    let player_bin = player::locate(args.player_bin.as_deref())?;

    // 4. Write the app directory.
    let out = args
        .out
        .clone()
        .unwrap_or_else(|| args.dir.join("dist").join("native"));
    let exe = exe_file_name(&args.dir);
    fs::create_dir_all(&out).with_context(|| format!("failed to create {}", out.display()))?;
    fs::copy(&player_bin, out.join(&exe)).with_context(|| {
        format!("failed to copy the player binary from {}", player_bin.display())
    })?;
    fs::write(out.join(bundle::PACK_FILE), bundle::serialize_pack(&files))
        .context("failed to write game.dzpk")?;

    let bundle_bytes = fs::metadata(out.join(bundle::PACK_FILE)).map(|m| m.len()).unwrap_or(0);
    println!(
        "exported {} file(s) ({:.1} MiB pack) to {}",
        files.len(),
        bundle_bytes as f64 / (1024.0 * 1024.0),
        out.display()
    );
    println!("run it with: {}", out.join(&exe).display());
    Ok(out)
}

/// The shipped executable name: the project directory's name (a slug by
/// convention), reduced to `[a-zA-Z0-9-_]`; `.exe` on Windows.
fn exe_file_name(dir: &std::path::Path) -> String {
    let slug = dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("game")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>();
    let mut name = if slug.is_empty() { "game".to_string() } else { slug };
    if cfg!(windows) {
        name.push_str(".exe");
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_runner::vfs::ProjectFiles as _;
    use dotzuki_runner::{run_headless, HeadlessOptions, LoadedProject, RunnerGame, RunnerOptions};
    use std::path::Path;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    static NEXT_ID: AtomicU32 = AtomicU32::new(0);

    /// Unique temp directory, removed on drop.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new(test: &str) -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!(
                "dotzuki-cli-export-native-{test}-{}-{id}",
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

    /// A fake player binary for tests that must not depend on a cargo build.
    fn fake_player_bin(test: &str) -> (TestDir, PathBuf) {
        let tmp = TestDir::new(test);
        let bin = tmp.0.join(player::exe_name());
        fs::write(&bin, b"fake player").unwrap();
        (tmp, bin)
    }

    /// The tutorial project shipped in this repo.
    fn example_project() -> Option<PathBuf> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/your-first-game");
        dir.is_dir().then_some(dir)
    }

    fn args(dir: PathBuf, out: PathBuf, player_bin: &Path, force: bool) -> NativeExportArgs {
        NativeExportArgs {
            dir,
            out: Some(out),
            player_bin: Some(player_bin.to_path_buf()),
            force,
        }
    }

    #[test]
    fn export_produces_a_bootable_app_dir() {
        let Some(project) = example_project() else {
            eprintln!("skipping: examples/your-first-game not present");
            return;
        };
        let (_pkg_tmp, bin) = fake_player_bin("app-pkg");
        let tmp = TestDir::new("app");
        let out = export_dir(&tmp);
        let produced = run(&args(project.clone(), out.clone(), &bin, false)).unwrap();
        assert_eq!(produced, out);

        // The two artifacts exist, the binary renamed after the project dir.
        assert!(out.join("your-first-game").is_file());
        let pack_bytes = fs::read(out.join(bundle::PACK_FILE)).unwrap();

        // The pack boots through the exact player path: parse into PackFiles
        // and drive a few frames headless.
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

        let (_pkg_tmp, bin) = fake_player_bin("broken-pkg");
        let err = run(&args(root.clone(), export_dir(&tmp), &bin, false)).unwrap_err();
        assert!(err.to_string().contains("--force"), "{err}");

        // --force exports anyway.
        let out = run(&args(root, export_dir(&tmp), &bin, true)).unwrap();
        assert!(out.join(bundle::PACK_FILE).is_file());
    }

    #[test]
    fn missing_project_is_an_error() {
        let tmp = TestDir::new("missing");
        let (_pkg_tmp, bin) = fake_player_bin("missing-pkg");
        let err = run(&args(tmp.0.join("nope"), export_dir(&tmp), &bin, false)).unwrap_err();
        assert!(err.to_string().contains(".dotzuki-editor.json"), "{err}");
    }

    #[test]
    fn exe_name_is_a_safe_slug() {
        let (name, slug) = if cfg!(windows) {
            ("My-Game-.exe", "your-first-game.exe")
        } else {
            ("My-Game-", "your-first-game")
        };
        assert_eq!(exe_file_name(Path::new("/tmp/My Game!")), name);
        assert_eq!(exe_file_name(Path::new("your-first-game")), slug);
    }

    fn export_dir(tmp: &TestDir) -> PathBuf {
        tmp.0.join(format!("out-{}", NEXT_ID.fetch_add(1, Ordering::SeqCst)))
    }
}
