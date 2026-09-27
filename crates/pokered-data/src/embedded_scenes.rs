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

/// Serialized statement lists keyed by `(map, function)`. Unlike a full
/// [`GameScene`](dotzuki_engine_dsl::ast::GameScene), these can remain in ROM
/// until the function is actually called, avoiding a large map-wide heap spike
/// on GBA transitions.
pub fn scene_functions() -> &'static [(&'static str, &'static str, &'static [u8])] {
    SCENE_FUNCTIONS
}

/// Locate a map's contiguous function range without scanning every scene.
pub fn scene_functions_for_map(
    map: &str,
) -> &'static [(&'static str, &'static str, &'static [u8])] {
    let start = SCENE_FUNCTIONS.partition_point(|(name, _, _)| *name < map);
    let end = start + SCENE_FUNCTIONS[start..].partition_point(|(name, _, _)| *name == map);
    &SCENE_FUNCTIONS[start..end]
}

/// The prefixed spelling lives in ROM too; registering a function need not
/// allocate a second name on a constrained target.
pub fn scene_function_alias(name: &str) -> Option<&'static str> {
    SCENE_FUNCTION_ALIASES
        .binary_search_by_key(&name, |(name, _)| *name)
        .ok()
        .map(|index| SCENE_FUNCTION_ALIASES[index].1)
}

/// Number of embedded, independently decodable script functions.
pub fn scene_function_count() -> usize {
    SCENE_FUNCTION_COUNT
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
    use crate::alloc_prelude::*;

    #[test]
    fn compiled_bindings_and_indexed_functions_match_embedded_sources() {
        for &(map, json) in scene_configs() {
            let parsed: dotzuki_engine_script::MapScriptConfig =
                serde_json::from_str(json).unwrap();
            let compiled = create_scene_config(map).unwrap();
            assert_eq!(format!("{compiled:?}"), format!("{parsed:?}"), "{map}");
            let expected: Vec<_> = scene_functions()
                .iter()
                .filter(|(name, _, _)| *name == map)
                .copied()
                .collect();
            assert_eq!(scene_functions_for_map(map), expected, "{map}");
        }
        for &(_, name, _) in scene_functions() {
            assert_eq!(
                scene_function_alias(name),
                Some(format!("storyline_{name}").as_str())
            );
        }
        assert!(create_scene_config("missing").is_none());
        assert!(scene_functions_for_map("missing").is_empty());
    }

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

    #[test]
    fn oaks_lab_early_oak_dialogue_has_small_lazy_branches() {
        for name in [
            "__native_talkOak1_battled",
            "__native_talkOak1_starter",
            "__native_talkOak1_choose",
        ] {
            let bytes = scene_functions()
                .iter()
                .find(|(map, function, _)| *map == "OaksLab" && *function == name)
                .map(|(_, _, bytes)| *bytes)
                .unwrap_or_else(|| panic!("missing lazy OaksLab branch {name}"));
            let statements: Vec<dotzuki_engine_dsl::ast::StoryStmt> =
                serde_json::from_slice(bytes).expect("lazy branch must deserialize");
            assert_eq!(statements.len(), 1, "{name} stays allocation-bounded");
        }
    }

    #[test]
    fn oaks_lab_late_dialogue_does_not_retain_unselected_ratings() {
        for band in 0..=15 {
            let name = format!("__native_talkOak1_rating_{band}");
            let (_, _, bytes) = scene_functions()
                .iter()
                .find(|(map, function, _)| *map == "OaksLab" && *function == name)
                .unwrap();
            assert!(
                bytes.len() < 12 * 1024,
                "rating {band} exceeded its memory budget"
            );
            let statements: Vec<dotzuki_engine_dsl::ast::StoryStmt> =
                serde_json::from_slice(bytes).unwrap();
            // Rating comparisons are gone; the original trailing early-game
            // conditional remains so this is identical to the compiled scene.
            assert!(statements.len() >= 4);
            for statement in &statements[2..statements.len() - 2] {
                assert!(matches!(
                    statement,
                    dotzuki_engine_dsl::ast::StoryStmt::Speaker { .. }
                ));
            }
        }
        for name in ["__native_talkOak1_parcel", "__native_talkOak1_dex_other"] {
            let (_, _, bytes) = scene_functions()
                .iter()
                .find(|(map, function, _)| *map == "OaksLab" && *function == name)
                .unwrap();
            assert!(bytes.len() < 12 * 1024, "{name} exceeded its memory budget");
        }
    }

    #[test]
    fn every_lazy_function_deserializes_without_host_source_paths() {
        fn assert_spans_are_portable(value: &serde_json::Value, context: &str) {
            match value {
                serde_json::Value::Object(fields) => {
                    if let Some(serde_json::Value::Object(span)) = fields.get("span") {
                        assert_eq!(
                            span.get("file").and_then(serde_json::Value::as_str),
                            Some(""),
                            "{context} embeds a build-host source path"
                        );
                    }
                    for child in fields.values() {
                        assert_spans_are_portable(child, context);
                    }
                }
                serde_json::Value::Array(values) => {
                    for child in values {
                        assert_spans_are_portable(child, context);
                    }
                }
                _ => {}
            }
        }

        for (map, function, bytes) in scene_functions() {
            let context = format!("{map}::{function}");
            let value: serde_json::Value = serde_json::from_slice(bytes)
                .unwrap_or_else(|error| panic!("{context} must deserialize: {error}"));
            assert_spans_are_portable(&value, &context);
        }
    }

    #[test]
    fn route22_encounters_fit_individually_without_cloning_the_outer_guard() {
        for suffix in ["early", "late", "noop"] {
            let name = format!("__native_coordRivalBattle_{suffix}");
            let bytes = scene_functions()
                .iter()
                .find(|(map, function, _)| *map == "Route22" && *function == name)
                .map(|(_, _, bytes)| *bytes)
                .unwrap();
            assert!(bytes.len() < 9 * 1024, "{name}: {} bytes", bytes.len());
            let statements: Vec<dotzuki_engine_dsl::ast::StoryStmt> =
                serde_json::from_slice(bytes).unwrap();
            assert!(!matches!(
                statements.first(),
                Some(dotzuki_engine_dsl::ast::StoryStmt::If { .. })
            ));
            assert_eq!(statements.is_empty(), suffix == "noop");
        }
    }

    #[test]
    fn oaks_lab_rival_battle_is_split_at_the_heap_peak() {
        for name in [
            "__native_coordDontGoAway_dont_go",
            "__native_coordDontGoAway_battle_before",
            "__native_coordDontGoAway_battle_win",
            "__native_coordDontGoAway_battle_loss",
            "__native_coordDontGoAway_noop",
        ] {
            let bytes = scene_functions()
                .iter()
                .find(|(map, function, _)| *map == "OaksLab" && *function == name)
                .map(|(_, _, bytes)| *bytes)
                .unwrap_or_else(|| panic!("missing split OaksLab branch {name}"));
            let _: Vec<dotzuki_engine_dsl::ast::StoryStmt> = serde_json::from_slice(bytes)
                .unwrap_or_else(|error| panic!("{name} must deserialize: {error}"));
            assert!(
                bytes.len() < 8 * 1024,
                "{name} must stay below the GBA allocation budget, got {} bytes",
                bytes.len()
            );
        }
    }
}
