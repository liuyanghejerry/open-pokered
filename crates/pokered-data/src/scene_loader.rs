//! Thin re-export of the generic disk scene providers from
//! `dotzuki_engine_dsl::disk_loader`.
//!
//! The providers compile `maps/<MapName>/script.scene` (and
//! `maps/shared/*.scene`) to `GameScene` ASTs or JS at runtime, with
//! mtime-based hot reload — the pokered flavor is only the `maps/` directory
//! the caller passes in (`--scripts-dir`); all mechanics live in the engine
//! crate. This module keeps the `pokered_data::scene_loader::*` paths stable
//! so existing call sites don't change.

pub use dotzuki_engine_dsl::disk_loader::{SceneAstProvider, SceneFileMeta, SceneScriptProvider};

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
