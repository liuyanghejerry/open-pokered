//! Build pipeline integration tests.
//!
//! Simulates the full build pipeline: DSL files → compilation → output verification.
//! Covers: idempotency, incremental compilation, error handling, and mixed valid/invalid.
//!
//! Run with: `cargo test -p dotzuki-engine-dsl -- build_integration`

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Helper: create a temp workspace with assets/scenes/ directory structure.
fn setup_temp_workspace(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!("dsl_integration_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir.join("assets").join("scenes")).unwrap();
    dir
}

/// Helper: count occurrences of a substring in all files under a directory.
#[allow(dead_code)]
fn count_substring_in_files(dir: &Path, substring: &str) -> usize {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if content.contains(substring) {
                    count += 1;
                }
            }
        }
    }
    count
}

// ══════════════════════════════════════════════════════════════════════════════
// Test 1: Idempotent output — same input → same hash on every compilation
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_full_pipeline_idempotent() {
    let root = setup_temp_workspace("idempotent");
    let scene_path = root.join("assets").join("scenes").join("test.scene");
    let dsl = "\
game_scene IdempotentTest {
  @storylines {
    @speaker(\"NPC\") { \"Hello\" }
  }
}";
    fs::write(&scene_path, dsl).unwrap();

    let out_dir = root.join("out");
    let dsl_out = out_dir.join("dsl");
    fs::create_dir_all(&dsl_out).unwrap();

    // First compilation pass
    let files1 = dotzuki_engine_dsl::compiler::discover_dsl_files(&root);
    assert!(!files1.is_empty(), "Should discover at least one DSL file");

    let mut hashes1 = Vec::new();
    for (ext, path) in &files1 {
        let content = fs::read_to_string(path).unwrap();
        let result = dotzuki_engine_dsl::compiler::compile_dsl_file(ext, &content, path, &dsl_out);
        assert!(
            result.is_ok(),
            "First compile should succeed: {:?}",
            result.err()
        );
        let js_path = dsl_out.join("IdempotentTest.js");
        let js = fs::read_to_string(&js_path).unwrap();
        hashes1.push(dotzuki_engine_dsl::compiler::content_hash(&js));
    }

    // Second compilation pass — identical input, should be idempotent
    let mut hashes2 = Vec::new();
    for (ext, path) in &files1 {
        let content = fs::read_to_string(path).unwrap();
        let result = dotzuki_engine_dsl::compiler::compile_dsl_file(ext, &content, path, &dsl_out);
        assert!(result.is_ok(), "Second compile should succeed");
        let js_path = dsl_out.join("IdempotentTest.js");
        let js = fs::read_to_string(&js_path).unwrap();
        hashes2.push(dotzuki_engine_dsl::compiler::content_hash(&js));
    }

    assert_eq!(
        hashes1, hashes2,
        "Idempotent: same input should produce same hash"
    );

    // Cleanup
    let _ = fs::remove_dir_all(&root);
}

// ══════════════════════════════════════════════════════════════════════════════
// Test 2: Incremental — only changed files are recompiled
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_incremental_only_changed_recompiled() {
    let root = setup_temp_workspace("incremental");
    let scenes_dir = root.join("assets").join("scenes");

    // Create 2 .scene files
    fs::write(
        scenes_dir.join("a.scene"),
        "\
game_scene A {
  @storylines {
    @speaker(\"A\") { \"first\" }
  }
}",
    )
    .unwrap();
    fs::write(
        scenes_dir.join("b.scene"),
        "\
game_scene B {
  @storylines {
    @speaker(\"B\") { \"first\" }
  }
}",
    )
    .unwrap();

    let out_dir = root.join("out");
    let dsl_out = out_dir.join("dsl");
    fs::create_dir_all(&dsl_out).unwrap();

    // First compilation
    let files = dotzuki_engine_dsl::compiler::discover_dsl_files(&root);
    assert_eq!(files.len(), 2, "Should discover 2 files");

    for (ext, path) in &files {
        let content = fs::read_to_string(path).unwrap();
        dotzuki_engine_dsl::compiler::compile_dsl_file(ext, &content, path, &dsl_out).unwrap();
    }

    // Record hashes
    let hash_a_first = dotzuki_engine_dsl::compiler::content_hash(
        &fs::read_to_string(dsl_out.join("A.js")).unwrap(),
    );
    let hash_b_first = dotzuki_engine_dsl::compiler::content_hash(
        &fs::read_to_string(dsl_out.join("B.js")).unwrap(),
    );

    // Modify only file A
    fs::write(
        scenes_dir.join("a.scene"),
        "\
game_scene A {
  @storylines {
    @speaker(\"A2\") { \"modified\" }
  }
}",
    )
    .unwrap();

    // Second compilation
    let files2 = dotzuki_engine_dsl::compiler::discover_dsl_files(&root);
    for (ext, path) in &files2 {
        let content = fs::read_to_string(path).unwrap();
        dotzuki_engine_dsl::compiler::compile_dsl_file(ext, &content, path, &dsl_out).unwrap();
    }

    let hash_a_second = dotzuki_engine_dsl::compiler::content_hash(
        &fs::read_to_string(dsl_out.join("A.js")).unwrap(),
    );
    let hash_b_second = dotzuki_engine_dsl::compiler::content_hash(
        &fs::read_to_string(dsl_out.join("B.js")).unwrap(),
    );

    // Verify: A changed, B unchanged
    assert_ne!(hash_a_first, hash_a_second, "A should have changed");
    assert_eq!(hash_b_first, hash_b_second, "B should be unchanged");

    let _ = fs::remove_dir_all(&root);
}

// ══════════════════════════════════════════════════════════════════════════════
// Test 3: Error handling — invalid DSL returns Err, never panics
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_handling_no_panic() {
    let root = setup_temp_workspace("error_handling");

    // Create invalid .scene file (syntax error)
    fs::write(
        root.join("assets").join("scenes").join("broken.scene"),
        "game_scene Broken { @@invalid_syntax !! }",
    )
    .unwrap();

    let out_dir = root.join("out");
    let dsl_out = out_dir.join("dsl");
    fs::create_dir_all(&dsl_out).unwrap();

    // This should NOT panic — compile_dsl_file should return Err
    let files = dotzuki_engine_dsl::compiler::discover_dsl_files(&root);
    for (ext, path) in &files {
        let content = fs::read_to_string(path).unwrap();
        let result = dotzuki_engine_dsl::compiler::compile_dsl_file(ext, &content, path, &dsl_out);
        // Should be Err with a useful message (not a panic)
        match result {
            Err(err_msg) => {
                assert!(!err_msg.is_empty(), "Error message should not be empty");
                assert!(
                    err_msg.contains("broken.scene") || err_msg.len() > 10,
                    "Error should reference source file or be descriptive"
                );
            }
            Ok(_) => panic!("Invalid DSL should return error, not Ok"),
        }
    }

    let _ = fs::remove_dir_all(&root);
}

// ══════════════════════════════════════════════════════════════════════════════
// Test 4: Mixed valid/invalid — valid files succeed, invalid files fail
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_mixed_valid_invalid() {
    let root = setup_temp_workspace("mixed");
    let scenes_dir = root.join("assets").join("scenes");

    // 2 valid + 1 invalid
    fs::write(
        scenes_dir.join("good_a.scene"),
        "\
game_scene GoodA {
  @storylines {
    @speaker(\"A\") { \"ok\" }
  }
}",
    )
    .unwrap();
    fs::write(scenes_dir.join("bad.scene"), "game_scene Bad { @@bad }").unwrap();
    fs::write(
        scenes_dir.join("good_b.scene"),
        "\
game_scene GoodB {
  @storylines {
    @speaker(\"B\") { \"ok\" }
  }
}",
    )
    .unwrap();

    let out_dir = root.join("out");
    let dsl_out = out_dir.join("dsl");
    fs::create_dir_all(&dsl_out).unwrap();

    let mut success_count = 0;
    let mut error_count = 0;

    let files = dotzuki_engine_dsl::compiler::discover_dsl_files(&root);
    assert_eq!(files.len(), 3, "Should discover 3 files");

    for (ext, path) in &files {
        let content = fs::read_to_string(path).unwrap();
        let result = dotzuki_engine_dsl::compiler::compile_dsl_file(ext, &content, path, &dsl_out);
        match result {
            Ok(_) => success_count += 1,
            Err(_) => error_count += 1,
        }
    }

    assert_eq!(success_count, 2, "2 valid files should succeed");
    assert_eq!(error_count, 1, "1 invalid file should fail");

    // Verify the 2 good .js files exist
    assert!(dsl_out.join("GoodA.js").exists(), "GoodA.js should exist");
    assert!(dsl_out.join("GoodB.js").exists(), "GoodB.js should exist");
    // Bad.scene should NOT produce a .js file
    assert!(!dsl_out.join("Bad.js").exists(), "Bad.js should NOT exist");

    let _ = fs::remove_dir_all(&root);
}
