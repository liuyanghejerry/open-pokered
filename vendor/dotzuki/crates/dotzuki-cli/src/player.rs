//! Locate (or build) the `dotzuki-player` native binary that
//! `dotzuki export --native` ships as the player runtime.
//!
//! Resolution order: an explicit `--player-bin` path → `cargo build` the
//! `dotzuki-player` bin target of `dotzuki-runner` in the source workspace
//! (incremental, so repeat exports are cheap). The build needs the dotzuki
//! source tree — a `cargo install`ed CLI has no sibling crates — so a binary
//! built outside the repo must be given `--player-bin`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Result};

/// The player binary file name on the host platform.
pub fn exe_name() -> &'static str {
    if cfg!(windows) {
        "dotzuki-player.exe"
    } else {
        "dotzuki-player"
    }
}

/// The workspace root, resolvable only when this CLI was built inside the
/// dotzuki workspace (`crates/dotzuki-cli` → `workspace/`).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Resolve the player binary. Without an override, build it with cargo and
/// return the release artifact's path.
pub fn locate(override_bin: Option<&Path>) -> Result<PathBuf> {
    if let Some(bin) = override_bin {
        if bin.is_file() {
            return Ok(bin.to_path_buf());
        }
        bail!("--player-bin {} is not a file", bin.display());
    }

    let workspace = workspace_root();
    if !workspace.join("Cargo.toml").is_file() {
        bail!(
            "no --player-bin given, and this `dotzuki` binary was built outside the dotzuki \
             source tree so it cannot build the player.\n\
             In a dotzuki checkout, run:\n  \
             cd workspace && cargo build --release -p dotzuki-runner --bin dotzuki-player\n\
             then re-run with --player-bin workspace/target/release/dotzuki-player"
        );
    }
    build(&workspace)?;

    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let bin = target.join("release").join(exe_name());
    if bin.is_file() {
        Ok(bin)
    } else {
        bail!("cargo build did not produce {}", bin.display())
    }
}

/// `cargo build --release` the player bin. Output goes through to the user:
/// a cold release build takes minutes (incremental rebuilds are seconds).
fn build(workspace: &Path) -> Result<()> {
    println!("building dotzuki-player (cargo build --release)…");
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "-p",
            "dotzuki-runner",
            "--bin",
            "dotzuki-player",
        ])
        .current_dir(workspace)
        .status()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => anyhow::anyhow!(
                "cargo is not installed (https://rustup.rs/) — install it, or pass \
                 --player-bin <path to a prebuilt dotzuki-player>"
            ),
            _ => anyhow::anyhow!(e).context("failed to run cargo"),
        })?;
    if !status.success() {
        bail!("cargo build failed ({status})");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT_ID: AtomicU32 = AtomicU32::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(test: &str) -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!(
                "dotzuki-cli-player-{test}-{}-{id}",
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

    #[test]
    fn override_bin_is_used_when_it_is_a_file() {
        let tmp = TestDir::new("override-ok");
        let bin = tmp.0.join(exe_name());
        fs::write(&bin, b"fake player").unwrap();
        let found = locate(Some(&bin)).unwrap();
        assert_eq!(found, bin);
    }

    #[test]
    fn override_bin_is_rejected_when_missing() {
        let tmp = TestDir::new("override-bad");
        let err = locate(Some(&tmp.0.join("nope"))).unwrap_err();
        assert!(err.to_string().contains("--player-bin"), "{err}");
    }
}
