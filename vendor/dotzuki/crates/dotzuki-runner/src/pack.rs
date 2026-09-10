//! The `.dzpk` binary game pack: one file a shipped game boots from.
//!
//! `game.bundle.json` (see [`crate::bundle`]) ships every asset as a base64
//! JSON string — a ~1.33× size inflation plus a full base64 decode on boot,
//! both painful once a project's graphics and audio grow. A `.dzpk` pack is
//! the same `path → content` map as raw bytes:
//!
//! ```text
//! 0..4    magic "DZPK"
//! 4..8    u32 LE  format version (currently 1)
//! 8..12   u32 LE  index JSON byte length
//! 12..    index JSON (UTF-8)
//! …       data section: every file's raw bytes, concatenated
//! ```
//!
//! The index JSON is
//! `{ "dotzuki": {…export metadata…}, "files": { "<path>": { "offset", "size" } } }`
//! with `offset` relative to the start of the data section. The `dotzuki`
//! metadata is informational only — nothing enforces it at runtime.
//!
//! [`PackFiles`] reads a pack through the [`ProjectFiles`] trait, so a packed
//! project boots through the exact same `LoadedProject::load_with_files` path
//! as a disk or in-memory project — native (`dotzuki-player`) and web
//! (`WasmRunner.fromPack`) alike.

use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};

use crate::vfs::ProjectFiles;

/// Pack magic bytes at offset 0.
pub const MAGIC: [u8; 4] = *b"DZPK";
/// The format version this crate reads and writes.
pub const FORMAT_VERSION: u32 = 1;
/// Header size: magic + version + index length.
const HEADER_LEN: usize = 12;

/// One index entry: a file's byte range within the data section.
#[derive(serde::Deserialize, serde::Serialize)]
struct IndexEntry {
    offset: u64,
    size: u64,
}

/// The parsed index JSON document.
#[derive(serde::Deserialize, serde::Serialize)]
struct Index {
    /// Export metadata (`{ tool, version, exportedAt }`); informational only.
    #[serde(default)]
    #[allow(dead_code)]
    dotzuki: serde_json::Value,
    files: BTreeMap<String, IndexEntry>,
}

/// Encode `files` (`path → raw content`, iterated in sorted order) into a
/// `.dzpk` pack. `dotzuki_meta` rides along as the informational
/// `"dotzuki"` index member (the CLI stamps tool/version/export time).
pub fn encode_pack(files: &BTreeMap<String, Vec<u8>>, dotzuki_meta: serde_json::Value) -> Vec<u8> {
    let mut index_files = BTreeMap::new();
    let mut data_len: u64 = 0;
    let mut offset: u64 = 0;
    for (path, bytes) in files {
        index_files.insert(
            path.clone(),
            IndexEntry {
                offset,
                size: bytes.len() as u64,
            },
        );
        offset += bytes.len() as u64;
        data_len = offset;
    }
    let index = Index {
        dotzuki: dotzuki_meta,
        files: index_files,
    };
    // Serializing plain maps of integers/strings cannot fail.
    let index_json = serde_json::to_vec(&index).expect("pack index serialization is infallible");

    let mut out = Vec::with_capacity(HEADER_LEN + index_json.len() + data_len as usize);
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&(index_json.len() as u32).to_le_bytes());
    out.extend_from_slice(&index_json);
    for bytes in files.values() {
        out.extend_from_slice(bytes);
    }
    out
}

/// A `.dzpk` pack held in memory, read through [`ProjectFiles`].
///
/// `read` slices the data section — no per-file decode, no copies beyond the
/// returned `Vec`.
pub struct PackFiles {
    bytes: Vec<u8>,
    /// Absolute offset of the data section within `bytes`.
    data_start: usize,
    /// `path → (offset, size)` within the data section.
    entries: BTreeMap<String, (u64, u64)>,
}

impl PackFiles {
    /// Parse and validate a `.dzpk` pack. Every index entry is bounds-checked
    /// against the data section here, so `read` can slice unchecked.
    ///
    /// # Errors
    ///
    /// Fails on a bad magic, an unsupported format version, a truncated
    /// header/index, a malformed index, or an entry pointing outside the data
    /// section (the error names the offending file).
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() < HEADER_LEN {
            bail!("not a .dzpk pack: file is shorter than the {HEADER_LEN}-byte header");
        }
        if bytes[0..4] != MAGIC {
            bail!("not a .dzpk pack: bad magic (expected DZPK)");
        }
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != FORMAT_VERSION {
            bail!("unsupported .dzpk format version {version} (this player reads {FORMAT_VERSION})");
        }
        let index_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let data_start = HEADER_LEN + index_len;
        if bytes.len() < data_start {
            bail!("corrupt .dzpk pack: index is truncated");
        }
        let index: Index = serde_json::from_slice(&bytes[HEADER_LEN..data_start])
            .context("corrupt .dzpk pack: index is not valid JSON")?;
        let data_len = (bytes.len() - data_start) as u64;
        let mut entries = BTreeMap::new();
        for (path, entry) in index.files {
            let end = entry.offset.checked_add(entry.size).with_context(|| {
                format!("corrupt .dzpk pack: file '{path}' range overflows")
            })?;
            if end > data_len {
                bail!(
                    "corrupt .dzpk pack: file '{path}' (offset {}, size {}) points past the data section ({data_len} bytes)",
                    entry.offset,
                    entry.size
                );
            }
            entries.insert(path, (entry.offset, entry.size));
        }
        Ok(Self {
            bytes,
            data_start,
            entries,
        })
    }
}

impl std::fmt::Debug for PackFiles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PackFiles")
            .field("files", &self.entries.len())
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

impl ProjectFiles for PackFiles {
    fn read(&self, path: &str) -> Result<Vec<u8>> {
        let (offset, size) = self
            .entries
            .get(path)
            .with_context(|| format!("no such file '{path}'"))?;
        let start = self.data_start + *offset as usize;
        Ok(self.bytes[start..start + *size as usize].to_vec())
    }

    fn list(&self, prefix: &str) -> Vec<String> {
        // BTreeMap iteration is already sorted.
        self.entries
            .keys()
            .filter(|k| {
                prefix.is_empty()
                    || k.as_str() == prefix
                    || k.strip_prefix(prefix)
                        .is_some_and(|rest| rest.starts_with('/'))
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_files() -> BTreeMap<String, Vec<u8>> {
        BTreeMap::from([
            ("data/a.txt".to_string(), b"hello".to_vec()),
            ("data/deep/b.bin".to_string(), vec![0, 1, 2, 255]),
            (".dotzuki-editor.json".to_string(), b"{}".to_vec()),
        ])
    }

    fn sample_pack() -> Vec<u8> {
        encode_pack(&sample_files(), serde_json::json!({"tool": "test", "version": "0.0.0"}))
    }

    #[test]
    fn round_trip_reads_every_file_byte_exact() {
        let pack = PackFiles::from_bytes(sample_pack()).unwrap();
        for (path, bytes) in sample_files() {
            assert_eq!(pack.read(&path).unwrap(), bytes, "{path}");
        }
        assert!(pack.read("nope").is_err());
        assert!(!pack.exists("nope"));
        assert!(pack.exists("data/a.txt"));
    }

    #[test]
    fn list_is_sorted_and_prefix_bounded() {
        let pack = PackFiles::from_bytes(sample_pack()).unwrap();
        assert_eq!(
            pack.list("data"),
            vec!["data/a.txt".to_string(), "data/deep/b.bin".to_string()]
        );
        assert_eq!(pack.list("datas"), Vec::<String>::new());
        assert_eq!(pack.list("").len(), 3);
    }

    #[test]
    fn encoding_is_deterministic() {
        assert_eq!(sample_pack(), sample_pack());
    }

    #[test]
    fn rejects_a_short_buffer() {
        let err = PackFiles::from_bytes(b"DZ".to_vec()).unwrap_err();
        assert!(err.to_string().contains("shorter than"), "{err}");
    }

    #[test]
    fn rejects_bad_magic() {
        let mut pack = sample_pack();
        pack[0] = b'X';
        let err = PackFiles::from_bytes(pack).unwrap_err();
        assert!(err.to_string().contains("bad magic"), "{err}");
    }

    #[test]
    fn rejects_an_unknown_version() {
        let mut pack = sample_pack();
        pack[4..8].copy_from_slice(&99u32.to_le_bytes());
        let err = PackFiles::from_bytes(pack).unwrap_err();
        assert!(err.to_string().contains("version 99"), "{err}");
    }

    #[test]
    fn rejects_a_truncated_index() {
        let mut pack = sample_pack();
        let index_len = u32::from_le_bytes(pack[8..12].try_into().unwrap()) as usize;
        pack[8..12].copy_from_slice(&(index_len as u32 + 16).to_le_bytes());
        let err = PackFiles::from_bytes(pack).unwrap_err();
        assert!(err.to_string().contains("truncated"), "{err}");
    }

    #[test]
    fn rejects_an_entry_past_the_data_section() {
        let files = BTreeMap::from([("a.txt".to_string(), b"hi".to_vec())]);
        let mut pack = encode_pack(&files, serde_json::json!({}));
        // Rewrite the index with an inflated size for a.txt.
        let index_json = br#"{"dotzuki":{},"files":{"a.txt":{"offset":0,"size":100}}}"#;
        pack.truncate(HEADER_LEN);
        pack[8..12].copy_from_slice(&(index_json.len() as u32).to_le_bytes());
        pack.extend_from_slice(index_json);
        pack.extend_from_slice(b"hi");
        let err = PackFiles::from_bytes(pack).unwrap_err();
        assert!(err.to_string().contains("a.txt"), "{err}");
        assert!(err.to_string().contains("past the data section"), "{err}");
    }

    #[test]
    fn an_empty_pack_boots_an_empty_file_set() {
        let pack = PackFiles::from_bytes(encode_pack(&BTreeMap::new(), serde_json::json!({})))
            .unwrap();
        assert_eq!(pack.list(""), Vec::<String>::new());
        assert!(pack.read("anything").is_err());
    }
}
