use dotzuki_engine_dsl::ast::GameScene;
use dotzuki_engine_dsl::compiler::{compile_scene_to_ast, compile_scene_to_js};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct SceneFileMeta {
    pub path: PathBuf,
    pub modified: SystemTime,
}

/// Disk provider for the native AST interpreter: compiles `maps/*/script.scene`
/// (and `maps/shared/*.scene`) to `GameScene` ASTs at runtime, mirroring
/// [`SceneScriptProvider`] but for the AST path (`--scripts-dir` hot reload
/// without Boa).
pub struct SceneAstProvider {
    pub scenes: HashMap<String, GameScene>,
    pub file_meta: HashMap<String, SceneFileMeta>,
    /// True after [`load_from_directory`](Self::load_from_directory): the
    /// whole provider came from `--scripts-dir` and shadows the embedded
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
        let mut count = 0;
        if !dir.is_dir() {
            return Ok(0);
        }

        // Shared modules (`shared/*.scene` → key `shared/{name}`).
        let shared_dir = dir.join("shared");
        if shared_dir.is_dir() {
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
                let source = fs::read_to_string(&path)
                    .map_err(|e| format!("read {}: {}", path.display(), e))?;
                let ast = compile_scene_to_ast(&source, &path.to_string_lossy())
                    .map_err(|e| format!("compile {}: {}", path.display(), e))?;
                let modified = fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                self.scenes.insert(format!("shared/{}", name), ast);
                self.file_meta.insert(
                    format!("shared/{}", name),
                    SceneFileMeta { path, modified },
                );
                count += 1;
            }
        }

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

            let modified = fs::metadata(&scene_path)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);

            let source = fs::read_to_string(&scene_path)
                .map_err(|e| format!("read {}: {}", scene_path.display(), e))?;

            let ast = compile_scene_to_ast(&source, &scene_path.to_string_lossy())
                .map_err(|e| format!("compile {}: {}", scene_path.display(), e))?;

            self.scenes.insert(map_id.clone(), ast);
            self.file_meta.insert(map_id.clone(), SceneFileMeta {
                path: scene_path,
                modified,
            });
            count += 1;
        }

        Ok(count)
    }

    pub fn check_reload(&mut self) -> Vec<String> {
        let mut changed = Vec::new();
        for (map_id, meta) in &self.file_meta {
            if let Ok(new_meta) = fs::metadata(&meta.path) {
                if let Ok(new_modified) = new_meta.modified() {
                    if new_modified > meta.modified {
                        changed.push(map_id.clone());
                    }
                }
            }
        }
        for map_id in &changed {
            let meta = self.file_meta.get(map_id).unwrap();
            if let Ok(source) = fs::read_to_string(&meta.path) {
                if let Ok(ast) = compile_scene_to_ast(&source, &meta.path.to_string_lossy()) {
                    if let Ok(new_meta) = fs::metadata(&meta.path) {
                        if let Ok(new_modified) = new_meta.modified() {
                            self.scenes.insert(map_id.clone(), ast);
                            self.file_meta.insert(map_id.clone(), SceneFileMeta {
                                path: meta.path.clone(),
                                modified: new_modified,
                            });
                        }
                    }
                }
            }
        }
        changed
    }
}

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
        let mut count = 0;
        if !dir.is_dir() {
            return Ok(0);
        }

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

            let modified = fs::metadata(&scene_path)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);

            let source = fs::read_to_string(&scene_path)
                .map_err(|e| format!("read {}: {}", scene_path.display(), e))?;

            let js = compile_scene_to_js(&source, &scene_path.to_string_lossy())
                .map_err(|e| format!("compile {}: {}", scene_path.display(), e))?;

            self.scenes.insert(map_id.clone(), js);
            self.file_meta.insert(map_id.clone(), SceneFileMeta {
                path: scene_path,
                modified,
            });
            count += 1;
        }

        Ok(count)
    }

    pub fn check_reload(&mut self) -> Vec<String> {
        let mut changed = Vec::new();
        for (map_id, meta) in &self.file_meta {
            if let Ok(new_meta) = fs::metadata(&meta.path) {
                if let Ok(new_modified) = new_meta.modified() {
                    if new_modified > meta.modified {
                        changed.push(map_id.clone());
                    }
                }
            }
        }
        for map_id in &changed {
            let meta = self.file_meta.get(map_id).unwrap();
            if let Ok(source) = fs::read_to_string(&meta.path) {
                if let Ok(js) = compile_scene_to_js(&source, &meta.path.to_string_lossy()) {
                    if let Ok(new_meta) = fs::metadata(&meta.path) {
                        if let Ok(new_modified) = new_meta.modified() {
                            self.scenes.insert(map_id.clone(), js);
                            self.file_meta.insert(map_id.clone(), SceneFileMeta {
                                path: meta.path.clone(),
                                modified: new_modified,
                            });
                        }
                    }
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod ast_provider_tests {
    use super::*;

    #[test]
    fn disk_provider_compiles_scenes_to_asts() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("maps");
        let mut provider = SceneAstProvider::new();
        let count = provider.load_from_directory(&dir).unwrap();
        assert!(count >= 240, "expected ~248 scenes + shared, got {}", count);
        let pallet = provider.get_scene("PalletTown").expect("PalletTown AST");
        assert!(
            pallet.storylines.iter().any(|s| s.name == "coordNorthExit"),
            "PalletTown disk AST must carry coordNorthExit"
        );
        // Shared modules load under the `shared/{name}` key.
        let shared = provider.get_scene("shared/pokecenter").expect("shared AST");
        assert!(
            shared.storylines.iter().any(|s| s.name == "talkNurse"),
            "shared AST must carry talkNurse"
        );
    }
}
