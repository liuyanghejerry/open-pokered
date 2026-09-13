//! Thin re-export of the generic disk scene providers from
//! `dotzuki_engine_dsl::disk_loader`.
//!
//! The providers compile `maps/<MapName>/script.scene` (and
//! `maps/shared/*.scene`) to `GameScene` ASTs or JS at runtime, with
//! mtime-based hot reload — the pokered flavor is only the `maps/` directory
//! the caller passes in (`--scripts-dir`); all mechanics live in the engine
//! crate. This module keeps the `pokered_data::scene_loader::*` paths stable
//! so existing call sites don't change.

// The disk providers (`--scripts-dir` hot-reload path) are host-only: they
// read `.scene` files from the filesystem. Bare metal always uses the
// build-time embedded scenes (crate::embedded_scenes).

#[cfg(not(target_os = "none"))]
pub use dotzuki_engine_dsl::disk_loader::{SceneAstProvider, SceneFileMeta, SceneScriptProvider};

// Bare-metal stand-ins with the same minimal surface the overworld screen
// touches: `new()`, the `scenes`/`disk_mode` fields and `get_scene`. They
// never hold data (disk_mode is always false, so every lookup falls back to
// the embedded scene tables), but keep the screen's field types and the
// `--scripts-dir` code paths (compiled out on the host path) type-checking.
#[cfg(target_os = "none")]
mod bare_metal_providers {
    use alloc::string::String;
    use dotzuki_engine_dsl::ast::GameScene;
    use crate::hash_compat::HashMap;

    #[derive(Default)]
    pub struct SceneAstProvider {
        pub scenes: HashMap<String, GameScene>,
        pub disk_mode: bool,
    }

    impl SceneAstProvider {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn get_scene(&self, map_id: &str) -> Option<&GameScene> {
            self.scenes.get(map_id)
        }
    }

    #[derive(Default)]
    pub struct SceneScriptProvider {
        pub scenes: HashMap<String, String>,
    }

    impl SceneScriptProvider {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn get_scene(&self, map_id: &str) -> Option<&String> {
            self.scenes.get(map_id)
        }
    }
}

#[cfg(target_os = "none")]
pub use bare_metal_providers::{SceneAstProvider, SceneScriptProvider};

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
