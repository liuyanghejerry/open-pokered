//! Compiled `.scene` scripts and `script_config.json` files, embedded at
//! build time by `build.rs` (`generate_scene_scripts`).
//!
//! This is the default script source for every frontend (native app, TUI,
//! web) when no `--scripts-dir` override is given: `pokered-core` registers
//! these tables with the script loader at startup. Unlike
//! [`crate::embedded_assets`] the tables are populated in **debug and
//! release** builds alike — hot-reload during development is handled by
//! passing a scripts directory, not by emptying the tables.

// ── Include generated data ──────────────────────────────────────────────────

include!(concat!(env!("OUT_DIR"), "/scene_scripts_gen.rs"));

// ── Public accessors ────────────────────────────────────────────────────────

/// Every compiled scene script as `(map_name, js_module)` pairs, sorted by
/// map name (e.g. `("PalletTown", "export async function …")`).
pub fn scene_scripts() -> &'static [(&'static str, &'static str)] {
    SCENE_SCRIPTS
}

/// Every serialized scene AST as `(map_name, bytes)` pairs, sorted by map
/// name. The bytes are serde_json of `dotzuki_engine_dsl::ast::GameScene` and
/// feed the native AST interpreter (`pokered-core::overworld::native_script`).
/// Also includes shared modules under the `shared/{name}` key
/// (e.g. `("shared/pokecenter", …)`).
pub fn scene_asts() -> &'static [(&'static str, &'static [u8])] {
    SCENE_ASTS
}

/// Deserialize the scene AST for `map` (e.g. `"PalletTown"` or
/// `"shared/pokecenter"`), or `None` when the map has no `script.scene` or
/// the bytes fail to deserialize.
pub fn get_scene_ast(map: &str) -> Option<dotzuki_engine_dsl::ast::GameScene> {
    let bytes = get_scene_ast_bytes(map)?;
    serde_json::from_slice(bytes).ok()
}

/// Return the raw serialized AST bytes for `map`, or `None` when absent.
pub fn get_scene_ast_bytes(map: &str) -> Option<&'static [u8]> {
    SCENE_ASTS
        .iter()
        .find(|(key, _)| *key == map)
        .map(|(_, bytes)| *bytes)
}

/// Every raw `script_config.json` as `(map_name, json)` pairs, sorted by
/// map name.
pub fn scene_configs() -> &'static [(&'static str, &'static str)] {
    SCENE_CONFIGS
}

/// Return the compiled JS module for `map` (e.g. `"PalletTown"`), or `None`
/// when the map has no `script.scene`.
pub fn get_scene_script(map: &str) -> Option<&'static str> {
    SCENE_SCRIPTS
        .iter()
        .find(|(key, _)| *key == map)
        .map(|(_, content)| *content)
}

/// Return the raw `script_config.json` for `map`, or `None` when absent.
pub fn get_scene_config(map: &str) -> Option<&'static str> {
    SCENE_CONFIGS
        .iter()
        .find(|(key, _)| *key == map)
        .map(|(_, content)| *content)
}

/// Number of embedded scene scripts.
pub fn scene_script_count() -> usize {
    SCENE_SCRIPT_COUNT
}

/// Number of embedded scene ASTs (maps + shared modules).
pub fn scene_ast_count() -> usize {
    SCENE_AST_COUNT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_maps_have_embedded_scene_and_config() {
        assert!(
            scene_script_count() >= 240,
            "expected ~248 embedded scenes, got {}",
            scene_script_count()
        );
        assert_eq!(scene_scripts().len(), scene_script_count());
        assert_eq!(scene_configs().len(), scene_script_count());
    }

    #[test]
    fn all_scenes_have_embedded_asts() {
        assert!(
            scene_ast_count() >= 240,
            "expected ~248 embedded scene ASTs, got {}",
            scene_ast_count()
        );
        assert_eq!(scene_asts().len(), scene_ast_count());
        // Every map scene has a deserializable AST with matching storylines.
        let pallet = get_scene_ast("PalletTown").expect("PalletTown AST embedded");
        assert!(
            pallet.storylines.iter().any(|s| s.name == "coordNorthExit"),
            "PalletTown AST must carry the north-exit coord event storyline"
        );
    }

    #[test]
    fn shared_pokecenter_scene_embedded_as_ast() {
        let shared = get_scene_ast("shared/pokecenter").expect("shared/pokecenter AST embedded");
        let names: Vec<&str> = shared.storylines.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"talkNurse"), "shared AST must export talkNurse, got {names:?}");
        assert!(
            names.contains(&"talkLinkReceptionist"),
            "shared AST must export talkLinkReceptionist, got {names:?}"
        );
    }

    #[test]
    fn pallet_town_scene_exports_coord_event() {
        let js = get_scene_script("PalletTown").expect("PalletTown scene embedded");
        assert!(
            js.contains("storyline_coordNorthExit"),
            "PalletTown JS must export the north-exit coord event"
        );
        let cfg = get_scene_config("PalletTown").expect("PalletTown config embedded");
        assert!(cfg.contains("northExit1"), "config binds northExit1");
    }
}
