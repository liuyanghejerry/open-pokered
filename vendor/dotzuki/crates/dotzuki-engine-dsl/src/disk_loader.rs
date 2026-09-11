//! Disk-backed scene providers: compile `.scene` files from a directory
//! tree at runtime, with mtime-based hot reload.
//!
//! This is the runtime counterpart of the build-time embedded path in
//! [`crate::loader`]: instead of `include!`-ing pre-compiled artifacts it
//! compiles `.scene` sources on the fly, so a game can point the engine at a
//! scripts directory (e.g. a `--scripts-dir` CLI flag) and iterate on scenes
//! without rebuilding.
//!
//! Directory layout contract (the root directory itself is injected by the
//! caller):
//!
//! - `<dir>/<scene-id>/script.scene` — one scene per subdirectory; the scene
//!   id is the subdirectory name.
//! - `<dir>/shared/<name>.scene` — shared modules, registered under the
//!   scene id `shared/<name>` (AST provider only).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::ast::GameScene;
use crate::compiler::{compile_scene_to_ast, compile_scene_to_js};

/// Filesystem metadata tracked for one loaded scene file, used by
/// `check_reload` to detect on-disk changes.
pub struct SceneFileMeta {
    pub path: PathBuf,
    pub modified: SystemTime,
}

/// A scene file discovered on disk: its scene id, path, and mtime.
struct SceneFile {
    id: String,
    path: PathBuf,
    modified: SystemTime,
}

fn modified_or_epoch(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

/// Collect `<dir>/shared/<name>.scene` files; scene id is `shared/<name>`.
fn collect_shared_files(dir: &Path) -> Result<Vec<SceneFile>, String> {
    let mut files = Vec::new();
    let shared_dir = dir.join("shared");
    if !shared_dir.is_dir() {
        return Ok(files);
    }
    let entries = fs::read_dir(&shared_dir).map_err(|e| format!("read_dir: {}", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("dir entry: {}", e))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("scene") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        files.push(SceneFile {
            id: format!("shared/{}", name),
            modified: modified_or_epoch(&path),
            path,
        });
    }
    Ok(files)
}

/// Collect `<dir>/<scene-id>/script.scene` files; scene id is the
/// subdirectory name.
fn collect_map_files(dir: &Path) -> Result<Vec<SceneFile>, String> {
    let mut files = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| format!("read_dir: {}", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("dir entry: {}", e))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let scene_path = path.join("script.scene");
        if !scene_path.is_file() {
            continue;
        }

        let map_id = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if map_id.is_empty() {
            continue;
        }

        files.push(SceneFile {
            id: map_id,
            modified: modified_or_epoch(&scene_path),
            path: scene_path,
        });
    }
    Ok(files)
}

/// Read, compile, and register each discovered scene file. Returns the
/// number of scenes successfully loaded; the first read/compile error
/// aborts the batch.
fn compile_files<T>(
    scenes: &mut HashMap<String, T>,
    file_meta: &mut HashMap<String, SceneFileMeta>,
    files: Vec<SceneFile>,
    compile: impl Fn(&str, &str) -> Result<T, String>,
) -> Result<usize, String> {
    let mut count = 0;
    for file in files {
        let source = fs::read_to_string(&file.path)
            .map_err(|e| format!("read {}: {}", file.path.display(), e))?;
        let compiled = compile(&source, &file.path.to_string_lossy())
            .map_err(|e| format!("compile {}: {}", file.path.display(), e))?;
        scenes.insert(file.id.clone(), compiled);
        file_meta.insert(
            file.id,
            SceneFileMeta {
                path: file.path,
                modified: file.modified,
            },
        );
        count += 1;
    }
    Ok(count)
}

/// Recompile every tracked file whose mtime advanced since it was loaded.
/// Files that fail to read or compile keep their previous version. Returns
/// the ids of the scenes that changed on disk (whether or not the recompile
/// succeeded).
fn reload_changed<T>(
    scenes: &mut HashMap<String, T>,
    file_meta: &mut HashMap<String, SceneFileMeta>,
    compile: impl Fn(&str, &str) -> Result<T, String>,
) -> Vec<String> {
    let mut changed = Vec::new();
    for (scene_id, meta) in file_meta.iter() {
        if let Ok(new_meta) = fs::metadata(&meta.path) {
            if let Ok(new_modified) = new_meta.modified() {
                if new_modified > meta.modified {
                    changed.push(scene_id.clone());
                }
            }
        }
    }
    for scene_id in &changed {
        let meta = file_meta.get(scene_id).unwrap();
        if let Ok(source) = fs::read_to_string(&meta.path) {
            if let Ok(compiled) = compile(&source, &meta.path.to_string_lossy()) {
                if let Ok(new_meta) = fs::metadata(&meta.path) {
                    if let Ok(new_modified) = new_meta.modified() {
                        scenes.insert(scene_id.clone(), compiled);
                        file_meta.insert(
                            scene_id.clone(),
                            SceneFileMeta {
                                path: meta.path.clone(),
                                modified: new_modified,
                            },
                        );
                    }
                }
            }
        }
    }
    changed
}

/// Disk provider for the native AST interpreter: compiles
/// `<scene-id>/script.scene` (and `shared/*.scene`) to [`GameScene`] ASTs at
/// runtime, mirroring [`SceneScriptProvider`] but for the AST path
/// (`--scripts-dir`-style hot reload without a JavaScript engine).
pub struct SceneAstProvider {
    pub scenes: HashMap<String, GameScene>,
    pub file_meta: HashMap<String, SceneFileMeta>,
    /// True after [`load_from_directory`](Self::load_from_directory): the
    /// whole provider came from a scripts directory and shadows the embedded
    /// ASTs entirely (mirrors the JS loader's all-or-nothing convention).
    /// When false, `scenes` only holds runtime injections/overrides and
    /// misses fall back to the embedded ASTs.
    pub disk_mode: bool,
}

impl SceneAstProvider {
    pub fn new() -> Self {
        Self {
            scenes: HashMap::new(),
            file_meta: HashMap::new(),
            disk_mode: false,
        }
    }

    pub fn get_scene(&self, map_id: &str) -> Option<&GameScene> {
        self.scenes.get(map_id)
    }

    pub fn has_scene(&self, map_id: &str) -> bool {
        self.scenes.contains_key(map_id)
    }

    pub fn load_from_directory(&mut self, dir: &Path) -> Result<usize, String> {
        self.disk_mode = true;
        if !dir.is_dir() {
            return Ok(0);
        }
        let mut count = compile_files(
            &mut self.scenes,
            &mut self.file_meta,
            collect_shared_files(dir)?,
            compile_scene_to_ast,
        )?;
        count += compile_files(
            &mut self.scenes,
            &mut self.file_meta,
            collect_map_files(dir)?,
            compile_scene_to_ast,
        )?;
        Ok(count)
    }

    pub fn check_reload(&mut self) -> Vec<String> {
        reload_changed(&mut self.scenes, &mut self.file_meta, compile_scene_to_ast)
    }
}

/// Disk provider for the JavaScript script path: compiles
/// `<scene-id>/script.scene` to JS source at runtime, for engines that run
/// scenes through a JS interpreter.
pub struct SceneScriptProvider {
    pub scenes: HashMap<String, String>,
    pub file_meta: HashMap<String, SceneFileMeta>,
}

impl SceneScriptProvider {
    pub fn new() -> Self {
        Self {
            scenes: HashMap::new(),
            file_meta: HashMap::new(),
        }
    }

    pub fn get_script(&self, map_id: &str) -> Option<&str> {
        self.scenes.get(map_id).map(|s| s.as_str())
    }

    pub fn has_script(&self, map_id: &str) -> bool {
        self.scenes.contains_key(map_id)
    }

    pub fn load_from_directory(&mut self, dir: &Path) -> Result<usize, String> {
        if !dir.is_dir() {
            return Ok(0);
        }
        compile_files(
            &mut self.scenes,
            &mut self.file_meta,
            collect_map_files(dir)?,
            compile_scene_to_js,
        )
    }

    pub fn check_reload(&mut self) -> Vec<String> {
        reload_changed(&mut self.scenes, &mut self.file_meta, compile_scene_to_js)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "dotzuki-dsl-disk-loader-{}-{}",
                name,
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn scene_source(scene_name: &str, storyline: &str, line: &str) -> String {
        format!(
            "game_scene {scene_name} {{\n    @storyline(\"{storyline}\") {{\n        @trigger(map = \"{scene_name}\", on_enter = true)\n        @speaker(\"Guide\") {{\n            \"{line}\"\n        }}\n    }}\n}}\n"
        )
    }

    /// Write `<dir>/<scene_id>/script.scene`.
    fn write_map_scene(dir: &Path, scene_id: &str, storyline: &str, line: &str) -> PathBuf {
        let map_dir = dir.join(scene_id);
        fs::create_dir_all(&map_dir).unwrap();
        let path = map_dir.join("script.scene");
        fs::write(&path, scene_source(scene_id, storyline, line)).unwrap();
        path
    }

    #[test]
    fn ast_provider_loads_map_and_shared_scenes() {
        let tmp = TempDir::new("ast-load");
        write_map_scene(&tmp.0, "StartTown", "intro", "Welcome!");
        let shared_dir = tmp.0.join("shared");
        fs::create_dir_all(&shared_dir).unwrap();
        fs::write(
            shared_dir.join("center.scene"),
            scene_source("center", "talkClerk", "Hello!"),
        )
        .unwrap();
        // Non-scene files and directories without script.scene are ignored.
        fs::write(tmp.0.join("notes.txt"), "not a scene").unwrap();
        fs::create_dir_all(tmp.0.join("EmptyDir")).unwrap();

        let mut provider = SceneAstProvider::new();
        let count = provider.load_from_directory(&tmp.0).unwrap();
        assert_eq!(count, 2);
        assert!(provider.disk_mode);
        assert!(provider.has_scene("StartTown"));
        assert!(provider.has_scene("shared/center"));
        assert!(!provider.has_scene("EmptyDir"));

        let town = provider.get_scene("StartTown").expect("StartTown AST");
        assert!(town.storylines.iter().any(|s| s.name == "intro"));
        let shared = provider.get_scene("shared/center").expect("shared AST");
        assert!(shared.storylines.iter().any(|s| s.name == "talkClerk"));
    }

    #[test]
    fn ast_provider_missing_dir_loads_zero_but_enters_disk_mode() {
        let mut provider = SceneAstProvider::new();
        let count = provider
            .load_from_directory(Path::new("/nonexistent/dotzuki-scenes"))
            .unwrap();
        assert_eq!(count, 0);
        assert!(provider.disk_mode);
    }

    #[test]
    fn script_provider_loads_js_and_ignores_shared_dir() {
        let tmp = TempDir::new("js-load");
        write_map_scene(&tmp.0, "StartTown", "intro", "Welcome!");
        let shared_dir = tmp.0.join("shared");
        fs::create_dir_all(&shared_dir).unwrap();
        fs::write(
            shared_dir.join("center.scene"),
            scene_source("center", "talkClerk", "Hello!"),
        )
        .unwrap();

        let mut provider = SceneScriptProvider::new();
        let count = provider.load_from_directory(&tmp.0).unwrap();
        assert_eq!(count, 1);
        assert!(provider.has_script("StartTown"));
        // The JS path has no shared-module convention.
        assert!(!provider.has_script("shared/center"));
        let js = provider.get_script("StartTown").expect("StartTown JS");
        assert!(js.contains("storyline_intro"), "generated JS: {}", js);
    }

    #[test]
    fn check_reload_is_quiet_until_a_file_changes() {
        let tmp = TempDir::new("reload-quiet");
        write_map_scene(&tmp.0, "StartTown", "intro", "Welcome!");
        let mut provider = SceneAstProvider::new();
        provider.load_from_directory(&tmp.0).unwrap();
        assert!(provider.check_reload().is_empty());
    }

    #[test]
    fn ast_provider_check_reload_recompiles_changed_files() {
        let tmp = TempDir::new("ast-reload");
        let path = write_map_scene(&tmp.0, "StartTown", "intro", "Welcome!");
        let mut provider = SceneAstProvider::new();
        provider.load_from_directory(&tmp.0).unwrap();

        // Force the tracked mtime into the past so the rewrite is detected
        // regardless of filesystem timestamp granularity.
        provider
            .file_meta
            .get_mut("StartTown")
            .unwrap()
            .modified = SystemTime::UNIX_EPOCH;
        fs::write(&path, scene_source("StartTown", "intro_v2", "Welcome back!")).unwrap();

        let changed = provider.check_reload();
        assert_eq!(changed, vec!["StartTown".to_string()]);
        let town = provider.get_scene("StartTown").expect("StartTown AST");
        assert!(town.storylines.iter().any(|s| s.name == "intro_v2"));
        // After the reload the tracked mtime matches the file again.
        assert!(provider.check_reload().is_empty());
    }

    #[test]
    fn script_provider_check_reload_recompiles_changed_files() {
        let tmp = TempDir::new("js-reload");
        let path = write_map_scene(&tmp.0, "StartTown", "intro", "Welcome!");
        let mut provider = SceneScriptProvider::new();
        provider.load_from_directory(&tmp.0).unwrap();

        provider
            .file_meta
            .get_mut("StartTown")
            .unwrap()
            .modified = SystemTime::UNIX_EPOCH;
        fs::write(&path, scene_source("StartTown", "intro_v2", "Welcome back!")).unwrap();

        let changed = provider.check_reload();
        assert_eq!(changed, vec!["StartTown".to_string()]);
        let js = provider.get_script("StartTown").expect("StartTown JS");
        assert!(js.contains("storyline_intro_v2"), "generated JS: {}", js);
    }
}
