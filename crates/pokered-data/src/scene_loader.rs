use dotzuki_engine_dsl::compiler::compile_scene_to_js;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct SceneFileMeta {
    pub path: PathBuf,
    pub modified: SystemTime,
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
