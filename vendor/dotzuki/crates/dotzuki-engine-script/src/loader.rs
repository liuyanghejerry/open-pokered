use std::collections::HashMap;

use crate::MapScriptConfig;

#[derive(Debug, Clone)]
pub struct ScriptSource {
    pub map_id: String,
    pub source: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
struct ScriptFileMeta {
    path: std::path::PathBuf,
    modified: std::time::SystemTime,
}

pub struct ScriptLoader {
    scripts: HashMap<String, String>,
    configs: HashMap<String, MapScriptConfig>,
    #[cfg(not(target_arch = "wasm32"))]
    file_meta: HashMap<String, ScriptFileMeta>,
}

impl ScriptLoader {
    pub fn new() -> Self {
        Self {
            scripts: HashMap::new(),
            configs: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            file_meta: HashMap::new(),
        }
    }

    pub fn register_script(&mut self, map_id: &str, source: &str) {
        self.scripts.insert(map_id.to_string(), source.to_string());
    }

    pub fn register_config(&mut self, map_id: &str, config: MapScriptConfig) {
        self.configs.insert(map_id.to_string(), config);
    }

    pub fn register_config_json(&mut self, map_id: &str, json: &str) -> Result<(), String> {
        let config: MapScriptConfig = serde_json::from_str(json)
            .map_err(|e| format!("JSON parse error for {}: {}", map_id, e))?;
        self.configs.insert(map_id.to_string(), config);
        Ok(())
    }

    pub fn get_script(&self, map_id: &str) -> Option<&str> {
        self.scripts.get(map_id).map(|s| s.as_str())
    }

    pub fn get_config(&self, map_id: &str) -> Option<&MapScriptConfig> {
        self.configs.get(map_id)
    }

    pub fn has_script(&self, map_id: &str) -> bool {
        self.scripts.contains_key(map_id)
    }

    pub fn has_config(&self, map_id: &str) -> bool {
        self.configs.contains_key(map_id)
    }

    pub fn loaded_maps(&self) -> Vec<&str> {
        self.scripts.keys().map(|s| s.as_str()).collect()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_from_directory(
        &mut self,
        dir: &std::path::Path,
    ) -> Result<usize, ScriptLoaderError> {
        use std::fs;

        if !dir.is_dir() {
            return Err(ScriptLoaderError::NotADirectory(
                dir.to_string_lossy().to_string(),
            ));
        }

        let shared_dir = dir.join("shared");
        if shared_dir.is_dir() {
            log::info!(target: "dotzuki::overworld", "[ScriptLoader] Loading shared modules from {:?}", shared_dir);
            for entry in fs::read_dir(&shared_dir).map_err(|e| {
                ScriptLoaderError::IoError(shared_dir.to_string_lossy().to_string(), e)
            })? {
                let entry = entry.map_err(|e| {
                    ScriptLoaderError::IoError(shared_dir.to_string_lossy().to_string(), e)
                })?;
                let path = entry.path();
                if path.is_file() {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        if let Ok(content) = fs::read_to_string(&path) {
                            let key = format!("shared/{}", name);
                            log::info!(target: "dotzuki::overworld", "[ScriptLoader] Registered shared module: {} ({} bytes)", key, content.len());
                            self.scripts.insert(key, content);
                        }
                    }
                }
            }
        }

        let mut count = 0;
        for entry in fs::read_dir(dir)
            .map_err(|e| ScriptLoaderError::IoError(dir.to_string_lossy().to_string(), e))?
        {
            let entry = entry
                .map_err(|e| ScriptLoaderError::IoError(dir.to_string_lossy().to_string(), e))?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let map_id = path
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| {
                    ScriptLoaderError::InvalidFileName(path.to_string_lossy().to_string())
                })?
                .to_string();

            let js_path = path.join("script.js");
            if js_path.is_file() {
                let content = fs::read_to_string(&js_path).map_err(|e| {
                    ScriptLoaderError::IoError(js_path.to_string_lossy().to_string(), e)
                })?;

                let modified = fs::metadata(&js_path)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

                self.scripts.insert(map_id.clone(), content);
                self.file_meta.insert(
                    format!("{}:js", map_id),
                    ScriptFileMeta {
                        path: js_path,
                        modified,
                    },
                );
                count += 1;
            }

            let config_path = path.join("script_config.json");
            if config_path.is_file() {
                let content = fs::read_to_string(&config_path).map_err(|e| {
                    ScriptLoaderError::IoError(config_path.to_string_lossy().to_string(), e)
                })?;

                let modified = fs::metadata(&config_path)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

                let config: MapScriptConfig = serde_json::from_str(&content).map_err(|e| {
                    ScriptLoaderError::IoError(
                        config_path.to_string_lossy().to_string(),
                        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
                    )
                })?;
                self.configs.insert(map_id.clone(), config);
                self.file_meta.insert(
                    format!("{}:json", map_id),
                    ScriptFileMeta {
                        path: config_path,
                        modified,
                    },
                );
                count += 1;
            }
        }

        log::info!("ScriptLoader: loaded {} files from {:?}", count, dir);
        Ok(count)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn check_reload(&mut self) -> Vec<String> {
        use std::fs;

        let mut reloaded = Vec::new();

        let entries: Vec<(String, std::path::PathBuf, std::time::SystemTime)> = self
            .file_meta
            .iter()
            .map(|(id, meta)| (id.clone(), meta.path.clone(), meta.modified))
            .collect();

        for (meta_key, path, old_modified) in entries {
            let current_modified = match fs::metadata(&path).and_then(|m| m.modified()) {
                Ok(t) => t,
                Err(_) => continue,
            };

            if current_modified > old_modified {
                match fs::read_to_string(&path) {
                    Ok(content) => {
                        let ext = path.extension().and_then(|e| e.to_str());
                        let map_id = path
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_string();

                        match ext {
                            Some("js") => {
                                self.scripts.insert(map_id.clone(), content);
                            }
                            Some("json") => {
                                if let Ok(config) =
                                    serde_json::from_str::<MapScriptConfig>(&content)
                                {
                                    self.configs.insert(map_id.clone(), config);
                                }
                            }
                            _ => {}
                        }

                        if let Some(meta) = self.file_meta.get_mut(&meta_key) {
                            meta.modified = current_modified;
                        }
                        log::info!("ScriptLoader: hot-reloaded {:?}", path);
                        reloaded.push(map_id);
                    }
                    Err(e) => {
                        log::warn!("ScriptLoader: failed to reload {:?}: {}", path, e);
                    }
                }
            }
        }

        reloaded
    }

    #[cfg(feature = "embedded-scripts")]
    pub fn load_embedded(&mut self) -> usize {
        crate::embedded_scripts::load_embedded_scripts(self);
        self.scripts.len()
    }

    #[cfg(feature = "embedded-scripts")]
    pub fn load_auto(
        &mut self,
        _scripts_dir: Option<&std::path::Path>,
    ) -> Result<usize, ScriptLoaderError> {
        let count = self.load_embedded();
        Ok(count)
    }

    #[cfg(all(not(feature = "embedded-scripts"), not(target_arch = "wasm32")))]
    pub fn load_auto(
        &mut self,
        scripts_dir: Option<&std::path::Path>,
    ) -> Result<usize, ScriptLoaderError> {
        if let Some(dir) = scripts_dir {
            return self.load_from_directory(dir);
        }

        Err(ScriptLoaderError::NotADirectory(
            "no scripts directory provided (auto-detection is no longer \
             baked into the engine; games pass their own --scripts-dir or use \
             their own embedded scene provider)"
                .to_string(),
        ))
    }

    #[cfg(all(not(feature = "embedded-scripts"), target_arch = "wasm32"))]
    pub fn load_auto(
        &mut self,
        _scripts_dir: Option<&std::path::Path>,
    ) -> Result<usize, ScriptLoaderError> {
        // wasm32 cannot load scripts from disk; embedded-scripts feature required for runtime use.
        // Stub returns Ok(0) so wasm builds compile; real wasm consumers (preview crate) must use embedded-scripts.
        Ok(0)
    }
}

impl Default for ScriptLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum ScriptLoaderError {
    NotADirectory(String),
    IoError(String, std::io::Error),
    InvalidFileName(String),
}

impl std::fmt::Display for ScriptLoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotADirectory(path) => write!(f, "Not a directory: {}", path),
            Self::IoError(path, err) => write!(f, "IO error at {}: {}", path, err),
            Self::InvalidFileName(path) => write!(f, "Invalid file name: {}", path),
        }
    }
}

impl std::error::Error for ScriptLoaderError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NpcBinding;

    #[test]
    fn test_register_and_get() {
        let mut loader = ScriptLoader::new();
        loader.register_script("TestMap", "function onEnter() {}");
        assert!(loader.has_script("TestMap"));
        assert_eq!(loader.get_script("TestMap"), Some("function onEnter() {}"));
    }

    #[test]
    fn test_register_config_json() {
        let mut loader = ScriptLoader::new();
        let json = r#"{
            "npcs": [{"id": 1, "talk": "talkProf"}],
            "signs": [{"id": 1, "talk": "signLab"}],
            "coordEvents": [{"name": "enterRoute1", "position": [4, 1], "trigger": "enterRoute1"}]
        }"#;
        loader.register_config_json("TestMap", json).unwrap();
        assert!(loader.has_config("TestMap"));
        let config = loader.get_config("TestMap").unwrap();
        assert_eq!(config.npcs.len(), 1);
        assert_eq!(config.npc_talk_fn(1), Some("talkProf"));
        assert_eq!(config.sign_talk_fn(1), Some("signLab"));
        assert_eq!(config.coord_event_fn(4, 1), Some("enterRoute1"));
    }

    #[test]
    fn test_get_script_missing() {
        let loader = ScriptLoader::new();
        assert_eq!(loader.get_script("NonExistentMap"), None);
    }

    #[test]
    fn test_get_config_missing() {
        let loader = ScriptLoader::new();
        assert!(loader.get_config("NonExistentMap").is_none());
    }

    #[test]
    fn test_has_script_false() {
        let loader = ScriptLoader::new();
        assert!(!loader.has_script("anything"));
    }

    #[test]
    fn test_has_config_false() {
        let loader = ScriptLoader::new();
        assert!(!loader.has_config("anything"));
    }

    #[test]
    fn test_loaded_maps_empty() {
        let loader = ScriptLoader::new();
        assert!(loader.loaded_maps().is_empty());
    }

    #[test]
    fn test_loaded_maps_multiple() {
        let mut loader = ScriptLoader::new();
        loader.register_script("MapA", "script A");
        loader.register_script("MapB", "script B");
        loader.register_script("MapC", "script C");

        let mut maps: Vec<&str> = loader.loaded_maps();
        maps.sort();
        assert_eq!(maps, vec!["MapA", "MapB", "MapC"]);
    }

    #[test]
    fn test_register_config_direct() {
        let mut loader = ScriptLoader::new();
        let config = MapScriptConfig {
            on_load: Some("onEnter".into()),
            npcs: vec![NpcBinding {
                id: 1,
                talk: Some("talkProf".into()),
                toggle_id: None,
                script_id: None,
                default_hidden: false,
            }],
            signs: vec![],
            coord_events: vec![],
        };
        loader.register_config("TestMap", config);
        assert!(loader.has_config("TestMap"));
        let loaded = loader.get_config("TestMap").unwrap();
        assert_eq!(loaded.on_load(), Some("onEnter"));
        assert_eq!(loaded.npc_talk_fn(1), Some("talkProf"));
    }

    #[test]
    fn test_register_config_json_invalid() {
        let mut loader = ScriptLoader::new();
        let result = loader.register_config_json("BadMap", "not valid json");
        assert!(result.is_err());
        assert!(!loader.has_config("BadMap"));
    }

    #[test]
    fn test_register_and_overwrite_script() {
        let mut loader = ScriptLoader::new();
        loader.register_script("Map", "version1");
        assert_eq!(loader.get_script("Map"), Some("version1"));

        loader.register_script("Map", "version2");
        assert_eq!(loader.get_script("Map"), Some("version2"));
    }

    #[test]
    fn test_loader_default() {
        let loader: ScriptLoader = Default::default();
        assert!(loader.loaded_maps().is_empty());
    }
}
