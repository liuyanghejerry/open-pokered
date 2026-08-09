pub mod schema;
pub mod types;

pub use schema::{
    Align, BoxDef, CursorDef, DynamicHeight, DynamicLabelDef, EdgeInsets, FlexItem, Justify,
    LabelDef, LayoutMode, PrimitiveDef, PrimitiveKind, ScreenLayout, SizeMode, VariantDef,
};
pub use types::{BracketSides, InkColor, TilePos, TileRect};

#[cfg(test)]
mod v2_registry_tests {
    use super::schema::{get_layout_json, get_screen_v2_json};

    #[test]
    fn v2_registry_serves_element_format_for_v1_covered_screen() {
        // `main` has a v1 (variants) JSON, so `get_layout_json` returns that;
        // the v2 registry must still expose `main.gui`'s element-format JSON
        // (with `"elements"`) so the migrated main menu can render via the
        // jrpg-renderer layout engine.
        let v2 = get_screen_v2_json("main").expect("main.gui must be in the v2 registry");
        assert!(v2.contains("\"elements\""), "v2 JSON should be element-format");
        assert!(v2.contains("\"schema_version\": 2"));

        let v1 = get_layout_json("main").expect("main has a v1 layout");
        assert!(v1.contains("\"variants\""), "get_layout_json keeps v1 precedence");
    }

    #[test]
    fn v2_registry_covers_gui_only_screens() {
        // Screens with no v1 JSON are served identically by both registries.
        assert!(get_screen_v2_json("mart_quantity").is_some());
        assert!(get_screen_v2_json("pokedex_list").is_some());
    }
}
