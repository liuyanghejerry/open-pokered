//! Virtual file system for project content: [`ProjectFiles`].
//!
//! Every project file the runner reads (manifest, DSL sources, maps,
//! tilesets, data-table records, audio tracks, sprites) goes through this
//! trait instead of `std::fs`, so the same loading code runs on native disk
//! projects ([`DiskFiles`]) and on in-memory projects ([`MemoryFiles`]) —
//! the browser/WASM shell fetches the project into a `MemoryFiles` and boots
//! it with [`crate::project::LoadedProject::load_with_files`].
//!
//! Paths are **project-relative POSIX strings** (`'/'`-separated, no leading
//! slash, `"./"` prefixes tolerated and stripped): `"data/maps/Town/map.tmx.json"`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Read/list access to a game project's files.
pub trait ProjectFiles {
    /// Read the file at project-relative POSIX `path`.
    ///
    /// # Errors
    ///
    /// Fails when the file does not exist or cannot be read.
    fn read(&self, path: &str) -> Result<Vec<u8>>;

    /// Recursively list every file under `prefix` (a project-relative POSIX
    /// directory; `""` lists the whole project), as project-relative POSIX
    /// paths, sorted lexicographically (stable). An absent `prefix` yields an
    /// empty list, not an error.
    fn list(&self, prefix: &str) -> Vec<String>;

    /// Whether a readable file exists at `path`.
    fn exists(&self, path: &str) -> bool {
        self.read(path).is_ok()
    }

    /// The on-disk project root, when backed by a real directory. Native
    /// conveniences (the save-file default location, hot-reload watching)
    /// use this; in-memory projects return `None`.
    fn root(&self) -> Option<&Path> {
        None
    }
}

/// A project backed by a real directory tree (the native case).
pub struct DiskFiles {
    root: PathBuf,
}

impl DiskFiles {
    /// A view over the directory `root` (the project dir).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl ProjectFiles for DiskFiles {
    fn read(&self, path: &str) -> Result<Vec<u8>> {
        let full = self.root.join(path);
        std::fs::read(&full).with_context(|| format!("failed to read {}", full.display()))
    }

    fn list(&self, prefix: &str) -> Vec<String> {
        let base = if prefix.is_empty() {
            self.root.clone()
        } else {
            self.root.join(prefix)
        };
        let mut out = Vec::new();
        if base.is_dir() {
            walk(&base, &self.root, &mut out);
        }
        out.sort();
        out
    }

    fn root(&self) -> Option<&Path> {
        Some(&self.root)
    }
}

/// A project held fully in memory (WASM shell, tests).
pub struct MemoryFiles {
    map: HashMap<String, Vec<u8>>,
}

impl MemoryFiles {
    /// A view over an in-memory `path → content` map.
    pub fn new(map: HashMap<String, Vec<u8>>) -> Self {
        Self { map }
    }
}

impl From<HashMap<String, Vec<u8>>> for MemoryFiles {
    fn from(map: HashMap<String, Vec<u8>>) -> Self {
        Self::new(map)
    }
}

impl ProjectFiles for MemoryFiles {
    fn read(&self, path: &str) -> Result<Vec<u8>> {
        self.map
            .get(path)
            .cloned()
            .with_context(|| format!("no such file '{path}'"))
    }

    fn list(&self, prefix: &str) -> Vec<String> {
        let mut out: Vec<String> = self
            .map
            .keys()
            .filter(|k| {
                prefix.is_empty()
                    || k.as_str() == prefix
                    || k.strip_prefix(prefix)
                        .is_some_and(|rest| rest.starts_with('/'))
            })
            .cloned()
            .collect();
        out.sort();
        out
    }
}

/// Join project-relative POSIX path segments, stripping a leading `"./"`
/// from `rel` (the editor's `"./data"` style). An empty `base` returns the
/// normalized `rel`.
pub fn join_path(base: &str, rel: &str) -> String {
    let rel = rel.strip_prefix("./").unwrap_or(rel);
    let rel = rel.strip_suffix('/').unwrap_or(rel);
    if base.is_empty() {
        rel.to_string()
    } else if rel.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{rel}")
    }
}

/// Recursive walk helper for [`DiskFiles::list`]: every regular file under
/// `dir`, as a `root`-relative POSIX path.
fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, root, out);
        } else if path.is_file() {
            if let Ok(rel) = path.strip_prefix(root) {
                let posix = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push(posix);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> MemoryFiles {
        MemoryFiles::new(HashMap::from([
            ("data/maps/Town/map.tmx.json".to_string(), b"{}".to_vec()),
            ("data/maps/Town/objects.json".to_string(), b"{}".to_vec()),
            ("data/maps/Field/map.tmx.json".to_string(), b"{}".to_vec()),
            ("data/rules.ron".to_string(), b"()".to_vec()),
            (".dotzuki-editor.json".to_string(), b"{}".to_vec()),
        ]))
    }

    #[test]
    fn memory_list_is_recursive_sorted_and_prefix_bounded() {
        let files = mem();
        assert_eq!(
            files.list("data/maps"),
            vec![
                "data/maps/Field/map.tmx.json".to_string(),
                "data/maps/Town/map.tmx.json".to_string(),
                "data/maps/Town/objects.json".to_string(),
            ]
        );
        // "data/map" must NOT match prefix "data/maps" (boundary-aware).
        assert_eq!(files.list("data/mapss"), Vec::<String>::new());
        assert_eq!(files.list("").len(), 5);
    }

    #[test]
    fn memory_read_and_exists() {
        let files = mem();
        assert_eq!(files.read("data/rules.ron").unwrap(), b"()".to_vec());
        assert!(files.read("nope.json").is_err());
        assert!(files.exists("data/rules.ron"));
        assert!(!files.exists("nope.json"));
    }

    #[test]
    fn join_path_normalizes() {
        assert_eq!(join_path("", "data"), "data");
        assert_eq!(join_path("", "./data"), "data");
        assert_eq!(join_path("data", "maps"), "data/maps");
        assert_eq!(join_path("data", "./maps"), "data/maps");
        assert_eq!(join_path("data", ""), "data");
    }
}
