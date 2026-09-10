//! Locate (or build) the `dotzuki-runner-web` wasm package that
//! `dotzuki export --web` ships as the player runtime.
//!
//! Resolution order: an explicit `--runner-pkg` directory → the prebuilt
//! `pkg/` next to the crate source → build it with wasm-pack. The last two
//! need the dotzuki source tree (a `cargo install`ed CLI only has the
//! registry copy of dotzuki-cli, without the sibling crate), so a binary
//! built outside the repo must be given `--runner-pkg`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Result};

/// wasm-pack `--target web` glue file.
pub const JS_FILE: &str = "dotzuki_runner_web.js";
/// The wasm binary the glue file loads.
pub const WASM_FILE: &str = "dotzuki_runner_web_bg.wasm";

/// The dotzuki-runner-web crate directory, resolvable only when this CLI was
/// built inside the dotzuki workspace.
fn source_crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../dotzuki-runner-web")
}

fn is_complete_pkg(dir: &Path) -> bool {
    dir.join(JS_FILE).is_file() && dir.join(WASM_FILE).is_file()
}

/// Resolve the runner wasm package directory (one containing [`JS_FILE`] and
/// [`WASM_FILE`]). `rebuild` forces a fresh wasm-pack build even when a
/// prebuilt package exists.
pub fn locate(override_dir: Option<&Path>, rebuild: bool) -> Result<PathBuf> {
    if let Some(dir) = override_dir {
        if is_complete_pkg(dir) {
            return Ok(dir.to_path_buf());
        }
        bail!(
            "--runner-pkg {} does not contain {JS_FILE} and {WASM_FILE}",
            dir.display()
        );
    }

    let crate_dir = source_crate_dir();
    let pkg = crate_dir.join("pkg");
    if !rebuild && is_complete_pkg(&pkg) {
        return Ok(pkg);
    }
    if !crate_dir.join("Cargo.toml").is_file() {
        bail!(
            "no prebuilt dotzuki-runner-web wasm package found, and this `dotzuki` \
             binary was built outside the dotzuki source tree so it cannot build one.\n\
             In a dotzuki checkout, run:\n  \
             cd workspace/crates/dotzuki-runner-web && \
             wasm-pack build --target web --out-dir pkg --release -- --features modern-audio\n\
             then re-run with --runner-pkg workspace/crates/dotzuki-runner-web/pkg"
        );
    }
    build(&crate_dir)?;
    if is_complete_pkg(&pkg) {
        Ok(pkg)
    } else {
        bail!(
            "wasm-pack build did not produce {JS_FILE} and {WASM_FILE} in {}",
            pkg.display()
        )
    }
}

/// `wasm-pack build` the runner crate (release, web target — same invocation
/// as the editor's `build:wasm-runner:release` script). Output goes through
/// to the user: a release wasm build takes a minute.
fn build(crate_dir: &Path) -> Result<()> {
    println!("building dotzuki-runner-web wasm package (wasm-pack --release)…");
    let status = Command::new("wasm-pack")
        .args([
            "build", "--target", "web", "--out-dir", "pkg", "--release", "--", "--features",
            "modern-audio",
        ])
        .current_dir(crate_dir)
        .status()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => anyhow::anyhow!(
                "wasm-pack is not installed (https://rustwasm.github.io/wasm-pack/installer/) \
                 — install it, or pass --runner-pkg <dir>"
            ),
            _ => anyhow::anyhow!(e).context("failed to run wasm-pack"),
        })?;
    if !status.success() {
        bail!("wasm-pack build failed ({status})");
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
                "dotzuki-cli-runnerpkg-{test}-{}-{id}",
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
    fn override_dir_is_used_when_complete() {
        let tmp = TestDir::new("override-ok");
        fs::write(tmp.0.join(JS_FILE), "// stub").unwrap();
        fs::write(tmp.0.join(WASM_FILE), b"\0asm").unwrap();
        let found = locate(Some(&tmp.0), false).unwrap();
        assert_eq!(found, tmp.0);
    }

    #[test]
    fn override_dir_is_rejected_when_incomplete() {
        let tmp = TestDir::new("override-bad");
        fs::write(tmp.0.join(JS_FILE), "// stub").unwrap();
        let err = locate(Some(&tmp.0), false).unwrap_err();
        assert!(err.to_string().contains(WASM_FILE), "{err}");
    }
}
