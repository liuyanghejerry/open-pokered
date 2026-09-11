//! Game bundle decoding: the legacy JSON sibling of the `.dzpk` pack.
//!
//! Older `dotzuki export` builds wrote `game.bundle.json` as
//! `{ "dotzuki": {…export metadata…}, "files": { "<path>": "<base64>" } }`.
//! Current exports ship a binary `.dzpk` pack instead (see [`crate::pack`]) —
//! base64 inflates assets ~1.33× and costs a full decode on boot. This module
//! stays so players can still boot an old export: [`decode_bundle_files`]
//! accepts BOTH shapes — a top-level object with a `files` object member is
//! unwrapped, anything else must itself be the files map. The `dotzuki`
//! metadata is informational only.

use std::collections::HashMap;

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

/// Decode a bundle JSON string into the `path → content` map a
/// [`MemoryFiles`](crate::vfs::MemoryFiles) boots from.
///
/// Accepts the full `game.bundle.json` (`{ "dotzuki": …, "files": … }`) or a
/// bare `{ path: base64 }` files map.
///
/// # Errors
///
/// Fails when the JSON is malformed, when neither shape yields a
/// `path → base64-string` object, or when a value is not valid base64 (the
/// error names the offending file).
pub fn decode_bundle_files(bundle_json: &str) -> Result<HashMap<String, Vec<u8>>> {
    let value: serde_json::Value =
        serde_json::from_str(bundle_json).context("bundle is not valid JSON")?;
    let files_value = match &value {
        serde_json::Value::Object(map) => match map.get("files") {
            Some(files @ serde_json::Value::Object(_)) => files,
            _ => &value,
        },
        _ => &value,
    };
    let encoded: HashMap<String, String> = serde_json::from_value(files_value.clone())
        .context("bundle is not an object of path→base64 strings")?;
    let mut files = HashMap::with_capacity(encoded.len());
    for (path, b64) in encoded {
        let bytes = BASE64
            .decode(&b64)
            .with_context(|| format!("file '{path}': invalid base64"))?;
        files.insert(path, bytes);
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_a_bare_files_map() {
        let json = serde_json::json!({
            "a/b.txt": BASE64.encode(b"hello"),
            "c.bin": BASE64.encode([0, 1, 2, 255]),
        })
        .to_string();
        let files = decode_bundle_files(&json).unwrap();
        assert_eq!(files["a/b.txt"], b"hello");
        assert_eq!(files["c.bin"], vec![0, 1, 2, 255]);
    }

    #[test]
    fn unwraps_a_full_bundle_and_ignores_metadata() {
        let json = serde_json::json!({
            "dotzuki": { "tool": "dotzuki-cli", "version": "0.0.0", "exportedAt": 0 },
            "files": { "a.txt": BASE64.encode(b"hi") },
        })
        .to_string();
        let files = decode_bundle_files(&json).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files["a.txt"], b"hi");
    }

    #[test]
    fn reports_the_file_with_bad_base64() {
        let json = serde_json::json!({ "data/x.png": "!!! not base64 !!!" }).to_string();
        let err = decode_bundle_files(&json).unwrap_err();
        assert!(err.to_string().contains("data/x.png"), "{err}");
    }

    #[test]
    fn rejects_non_object_json() {
        let err = decode_bundle_files(r#"["not", "an", "object"]"#).unwrap_err();
        assert!(err.to_string().contains("path→base64"), "{err}");
    }

    #[test]
    fn a_non_object_files_member_falls_back_to_the_whole_object() {
        // `{ "files": 42 }` is not a bundle wrapper, so the whole object is
        // treated as the files map — and fails because 42 is not base64.
        let err = decode_bundle_files(r#"{ "files": 42 }"#).unwrap_err();
        assert!(err.to_string().contains("path→base64"), "{err}");
    }
}
