//! Project bundling for `dotzuki export`: pack a whole game project
//! directory into the `path → content` map a shipped game boots from, then
//! serialize it as a `.dzpk` binary pack (see `dotzuki_runner::pack`).
//!
//! This is a Rust port of the editor's play-bundle collector
//! (`tools/dotzuki-editor/server/api/routes/play.ts`, also mirrored by
//! dotzuki-cloud's `bundle.ts`). **Keep the exclusion rules and size caps in
//! sync across all three.**

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};

use dotzuki_runner::pack;

/// A single file larger than this is refused.
pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// Total uncompressed size cap for the whole bundle.
pub const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
/// Directories that never ship in a bundle.
const SKIP_DIRS: [&str; 4] = ["node_modules", ".git", "target", "dist"];
/// The one dotfile that must ship: the runner needs the project manifest.
const MANIFEST_FILE: &str = ".dotzuki-editor.json";

/// The pack file name every export target ships (`game.dzpk`).
pub const PACK_FILE: &str = "game.dzpk";

/// Recursively collect `root` into `posix rel path → raw content` with the
/// default size caps ([`MAX_FILE_BYTES`] / [`MAX_TOTAL_BYTES`]).
///
/// Sandbox rules (mirror the editor's):
/// - symlinks are never followed (skipped via `symlink_metadata`);
/// - `node_modules`/`.git`/`target`/`dist`, dot-directories, dotfiles and
///   `*.bak` are excluded — EXCEPT `.dotzuki-editor.json`;
/// - non-UTF-8 file names are skipped (they cannot be pack index keys).
///
/// Fails past the per-file / total size caps.
pub fn collect_project_files(root: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    collect_project_files_with_caps(root, MAX_FILE_BYTES, MAX_TOTAL_BYTES)
}

/// Serialize collected files into the `game.dzpk` bytes every export target
/// ships. The stamped metadata is informational only — nothing enforces it at
/// runtime.
pub fn serialize_pack(files: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let exported_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    pack::encode_pack(
        files,
        serde_json::json!({
            "tool": "dotzuki-cli",
            "version": env!("CARGO_PKG_VERSION"),
            "exportedAt": exported_at,
        }),
    )
}

/// [`collect_project_files`] with explicit caps (tests, future CLI flags).
pub fn collect_project_files_with_caps(
    root: &Path,
    max_file: u64,
    max_total: u64,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let mut total: u64 = 0;
    walk(root, "", &mut files, &mut total, max_file, max_total)?;
    Ok(files)
}

#[allow(clippy::too_many_arguments)] // walker state — caps + accumulator + output
fn walk(
    dir: &Path,
    rel: &str,
    out: &mut BTreeMap<String, Vec<u8>>,
    total: &mut u64,
    max_file: u64,
    max_total: u64,
) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("failed to list {}", dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("failed to list {}", dir.display()))?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name();
        let Ok(name) = name.into_string() else {
            continue; // non-UTF-8 name: cannot be an index key
        };
        let full = entry.path();
        let meta = std::fs::symlink_metadata(&full)
            .with_context(|| format!("failed to stat {}", full.display()))?;
        let child_rel = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) || name.starts_with('.') {
                continue;
            }
            walk(&full, &child_rel, out, total, max_file, max_total)?;
        } else if meta.is_file() {
            if name != MANIFEST_FILE && (name.starts_with('.') || name.ends_with(".bak")) {
                continue;
            }
            let size = meta.len();
            if size > max_file {
                bail!("File too large: {child_rel} ({size} bytes, cap is {max_file})");
            }
            *total += size;
            if *total > max_total {
                bail!("Project too large to bundle (>{max_total} bytes uncompressed)");
            }
            let bytes = std::fs::read(&full)
                .with_context(|| format!("failed to read {}", full.display()))?;
            out.insert(child_rel, bytes);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_runner::vfs::ProjectFiles as _;
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT_ID: AtomicU32 = AtomicU32::new(0);

    /// Unique temp directory, removed on drop.
    struct TestDir(std::path::PathBuf);

    impl TestDir {
        fn new(test: &str) -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!(
                "dotzuki-cli-bundle-{test}-{}-{id}",
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

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn collects_only_shippable_files_as_posix_raw_bytes() {
        let tmp = TestDir::new("rules");
        let root = tmp.0.as_path();
        write(root, ".dotzuki-editor.json", b"{}");
        write(root, "data/a.txt", b"hello");
        write(root, "data/deep/b.bin", &[0, 1, 255]);
        // excluded:
        write(root, ".hidden", b"x");
        write(root, ".hidden-dir/x", b"x");
        write(root, "data/x.bak", b"x");
        write(root, "node_modules/pkg/index.js", b"x");
        write(root, ".git/config", b"x");
        write(root, "target/debug/x", b"x");
        write(root, "dist/web/index.html", b"x");
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("data"), root.join("link")).unwrap();

        let files = collect_project_files(root).unwrap();
        let keys: Vec<&str> = files.keys().map(|k| k.as_str()).collect();
        assert_eq!(
            keys,
            [".dotzuki-editor.json", "data/a.txt", "data/deep/b.bin"]
        );
        assert_eq!(files["data/a.txt"], b"hello");
        assert_eq!(files["data/deep/b.bin"], vec![0, 1, 255]);
    }

    #[test]
    fn serialize_pack_round_trips_through_pack_files() {
        let files = BTreeMap::from([
            ("a/b.txt".to_string(), b"hello".to_vec()),
            ("c.bin".to_string(), vec![0, 1, 2, 255]),
        ]);
        let pack = pack::PackFiles::from_bytes(serialize_pack(&files)).unwrap();
        assert_eq!(pack.read("a/b.txt").unwrap(), b"hello");
        assert_eq!(pack.read("c.bin").unwrap(), vec![0, 1, 2, 255]);
    }

    #[test]
    fn refuses_a_file_past_the_per_file_cap() {
        let tmp = TestDir::new("filecap");
        write(&tmp.0, "big.bin", &[0u8; 9]);
        let err = collect_project_files_with_caps(&tmp.0, 8, 1024).unwrap_err();
        assert!(err.to_string().contains("File too large: big.bin"), "{err}");
    }

    #[test]
    fn refuses_a_project_past_the_total_cap() {
        let tmp = TestDir::new("totalcap");
        write(&tmp.0, "a.bin", &[0u8; 6]);
        write(&tmp.0, "b.bin", &[0u8; 6]);
        let err = collect_project_files_with_caps(&tmp.0, 1024, 10).unwrap_err();
        assert!(err.to_string().contains("Project too large"), "{err}");
    }
}
