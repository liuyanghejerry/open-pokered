//! JSON deserialization for screen layout definitions.
//!
//! Provides [`load_layout`] (from file) and [`parse_layout`] (from string)
//! entry points that deserialise JSON into [`ScreenLayout`] and run basic
//! validation.

use crate::layout_engine::types::{RenderError, ScreenLayout};
#[cfg(not(target_os = "none"))]
use std::path::Path;

// ── Public API ──────────────────────────────────────────────────────────────

/// Load a layout by name from the game's data directory.
///
/// Tries to load from `data/ui_layouts/{name}.json` relative to the current
/// working directory (a generic fallback; callers that know their own data
/// root pass a full path instead).
///
/// # Errors
///
/// Returns [`RenderError::InvalidLayout`] if the file cannot be read or
/// the JSON is malformed.
// Hosted only (filesystem access); bare-metal targets embed layouts or
// call [`parse_layout`] directly.
#[cfg(not(target_os = "none"))]
pub fn load_layout(name: &str) -> Result<ScreenLayout, RenderError> {
    let candidate_paths = [format!("data/ui_layouts/{}.json", name)];

    for path in &candidate_paths {
        if Path::new(path).exists() {
            let json = std::fs::read_to_string(path).map_err(|e| {
                log::warn!("Failed to read layout file '{}': {}", path, e);
                RenderError::InvalidLayout
            })?;
            return parse_layout(&json);
        }
    }

    log::warn!(
        "Layout file not found for '{}' (tried: {:?})",
        name,
        candidate_paths
    );
    Err(RenderError::InvalidLayout)
}

/// Parse a layout from a JSON string.
///
/// This is the primary entry point for testing and for programmatic layout
/// creation. After deserialisation the layout is validated for common
/// issues (zero-sized elements, unknown types, etc.).
///
/// # Errors
///
/// Returns [`RenderError::InvalidLayout`] if the JSON cannot be parsed
/// into a valid [`ScreenLayout`].
pub fn parse_layout(json: &str) -> Result<ScreenLayout, RenderError> {
    let layout: ScreenLayout = serde_json::from_str(json).map_err(|e| {
        log::warn!("Failed to parse layout JSON: {}", e);
        RenderError::InvalidLayout
    })?;

    validate_layout(&layout);
    Ok(layout)
}

// ── Validation ─────────────────────────────────────────────────────────────

/// Run checks on a deserialised layout and log warnings for recoverable
/// issues.
fn validate_layout(layout: &ScreenLayout) {
    for element in &layout.elements {
        let id = if element.id.is_empty() {
            format!("<type:{}>", element.element_type)
        } else {
            element.id.clone()
        };

        // Warn on zero-sized elements (tw=0 or th=0 means nothing renders)
        if element.rect.tw == Some(0) {
            log::warn!(
                "Element '{}' has tw=0 — nothing will be drawn horizontally",
                id
            );
        }
        if element.rect.th == Some(0) {
            log::warn!(
                "Element '{}' has th=0 — nothing will be drawn vertically",
                id
            );
        }

        // Warn on unknown element types so the user knows the renderer
        // will skip them.
        match element.element_type.as_str() {
            "group" | "border" | "text" | "tile" | "divider" | "image" | "list" | "flex_list" => {}
            t if t.starts_with("custom:") => {}
            _ => {
                log::warn!(
                    "Unknown element type '{}' in element '{}' — will be skipped at render time",
                    element.element_type,
                    id,
                );
            }
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── test_parse_basic_layout ───────────────────────────────────────

    #[test]
    fn test_parse_basic_layout() {
        let json = r##"{
            "schema_version": 1,
            "screen": "test",
            "theme": { "bg_color": "#FFFFFF", "default_font": "default" },
            "elements": [
                {
                    "type": "text",
                    "rect": { "tx": 0, "ty": 0, "tw": 10, "th": 2 },
                    "value": "Hello World",
                    "color": "black"
                },
                {
                    "type": "border",
                    "rect": { "tx": 0, "ty": 0, "tw": 10, "th": 5 },
                    "style": "Single"
                },
                {
                    "type": "tile",
                    "rect": { "tx": 5, "ty": 5, "tw": 1, "th": 1 },
                    "tile_id": 42
                }
            ]
        }"##;

        let layout = parse_layout(json).expect("should parse");
        assert_eq!(layout.schema_version, 1);
        assert_eq!(layout.screen, "test");
        assert_eq!(layout.elements.len(), 3);

        // First element — text
        let e0 = &layout.elements[0];
        assert_eq!(e0.element_type, "text");
        assert_eq!(e0.rect.tx.as_literal(), Some(0));
        assert_eq!(e0.rect.ty.as_literal(), Some(0));

        // Second element — border
        let e1 = &layout.elements[1];
        assert_eq!(e1.element_type, "border");
        assert_eq!(e1.rect.tw, Some(10));

        // Third element — tile
        let e2 = &layout.elements[2];
        assert_eq!(e2.element_type, "tile");
        assert_eq!(e2.rect.tx.as_literal(), Some(5));
    }

    // ── test_parse_empty_elements ─────────────────────────────────────

    #[test]
    fn test_parse_empty_elements() {
        let json = r##"{
            "schema_version": 1,
            "screen": "empty",
            "theme": { "bg_color": "#000000", "default_font": "default" },
            "elements": []
        }"##;

        let layout = parse_layout(json).expect("should parse even with zero elements");
        assert_eq!(layout.screen, "empty");
        assert!(layout.elements.is_empty());
    }

    // ── test_parse_invalid_json ───────────────────────────────────────

    #[test]
    fn test_parse_invalid_json() {
        // Missing closing brace
        let result = parse_layout(r##"{"schema_version": 1, "screen": "bad""##);
        assert!(result.is_err(), "invalid JSON should return error");
        match result {
            Err(RenderError::InvalidLayout) => {} // expected
            other => panic!("expected InvalidLayout, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_missing_required_field() {
        // Missing required "screen" field — serde should fail
        let json = r##"{
            "schema_version": 1,
            "theme": { "bg_color": "#FFFFFF", "default_font": "default" },
            "elements": []
        }"##;
        let result = parse_layout(json);
        assert!(result.is_err(), "missing required field should error");
    }

    #[test]
    fn test_parse_wrong_types() {
        // elements should be an array, not an object
        let json = r##"{
            "schema_version": 1,
            "screen": "test",
            "elements": { "not": "an array" }
        }"##;
        let result = parse_layout(json);
        assert!(result.is_err(), "wrong type should error");
    }

    // ── test_parse_unknown_type ───────────────────────────────────────

    #[test]
    fn test_parse_unknown_type() {
        // Unknown element types should parse successfully (no error)
        // but a warning should be logged by validate_layout.
        let json = r##"{
            "schema_version": 1,
            "screen": "test",
            "theme": { "bg_color": "#FFFFFF", "default_font": "default" },
            "elements": [
                {
                    "type": "foobar_unknown",
                    "rect": { "tx": 0, "ty": 0, "tw": 5, "th": 5 },
                    "some_param": "hello"
                }
            ]
        }"##;

        let layout = parse_layout(json).expect("should parse unknown types");
        assert_eq!(layout.elements.len(), 1);
        assert_eq!(layout.elements[0].element_type, "foobar_unknown");
        // Validation logs a warning but does not error — the element is kept.
    }

    // ── test_parse_dex_layout ─────────────────────────────────────────

    #[test]
    fn test_parse_dex_layout() {
        let json = r##"{
            "schema_version": 1,
            "screen": "dex",
            "theme": { "bg_color": "#FFFFFF", "default_font": "default" },
            "elements": [
                {
                    "id": "dex_border",
                    "type": "border",
                    "rect": { "tx": 0, "ty": 0, "tw": 20, "th": 18 },
                    "style": "Single"
                },
                {
                    "id": "dex_title",
                    "type": "text",
                    "rect": { "tx": 1, "ty": 1, "tw": 18, "th": 2 },
                    "value": "DEX",
                    "color": "black",
                    "align": "Center"
                },
                {
                    "id": "separator",
                    "type": "divider",
                    "rect": { "tx": 1, "ty": 2, "tw": 18, "th": 1 },
                    "tiles": [122],
                    "repeat": 18
                },
                {
                    "id": "dex_image",
                    "type": "image",
                    "rect": { "tx": 1, "ty": 3, "tw": 7, "th": 7 },
                    "source": "{monster_sprite}"
                },
                {
                    "id": "dex_num",
                    "type": "text",
                    "rect": { "tx": 9, "ty": 3, "tw": 10, "th": 1 },
                    "value": "\u2116\u2022{dex_num:03}",
                    "color": "black"
                },
                {
                    "id": "dex_name",
                    "type": "text",
                    "rect": { "tx": 9, "ty": 4, "tw": 10, "th": 1 },
                    "value": "{name}",
                    "color": "black"
                },
                {
                    "id": "dex_height",
                    "type": "text",
                    "rect": { "tx": 9, "ty": 5, "tw": 10, "th": 1 },
                    "value": "HT {feet}\u2032{inches:02}\u2033",
                    "color": "black"
                },
                {
                    "id": "dex_weight",
                    "type": "text",
                    "rect": { "tx": 9, "ty": 6, "tw": 10, "th": 1 },
                    "value": "WT {weight%10}.{weight%10} lb",
                    "color": "black"
                },
                {
                    "id": "dex_desc",
                    "type": "text",
                    "rect": { "tx": 1, "ty": 11, "tw": 18, "th": 6 },
                    "value": "{description}",
                    "color": "black",
                    "wrap": true
                }
            ]
        }"##;

        let layout = parse_layout(json).expect("dex layout should parse");
        assert_eq!(layout.screen, "dex");
        assert_eq!(layout.elements.len(), 9);

        // Verify key elements by id
        let ids: Vec<&str> = layout.elements.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains(&"dex_border"));
        assert!(ids.contains(&"dex_title"));
        assert!(ids.contains(&"dex_image"));
        assert!(ids.contains(&"dex_name"));
        assert!(ids.contains(&"dex_desc"));
    }

    // ── test_load_layout_not_found ────────────────────────────────────

    #[test]
    fn test_load_layout_not_found() {
        let result = load_layout("__nonexistent_layout_xyzzy__");
        assert!(result.is_err(), "missing file should error");
        match result {
            Err(RenderError::InvalidLayout) => {} // expected
            other => panic!("expected InvalidLayout, got {:?}", other),
        }
    }

    // ── test_parse_minimal_layout ─────────────────────────────────────

    #[test]
    fn test_parse_minimal_layout() {
        // Schema with all defaults — theme defaults, elements empty, no ids
        let json = r##"{
            "schema_version": 1,
            "screen": "minimal",
            "elements": []
        }"##;

        let layout = parse_layout(json).expect("minimal should parse");
        assert_eq!(layout.screen, "minimal");
        assert_eq!(layout.elements.len(), 0);
    }

    // ── test_parse_group_layout ───────────────────────────────────────

    #[test]
    fn test_parse_group_layout() {
        let json = r##"{
            "schema_version": 1,
            "screen": "grouped",
            "theme": { "bg_color": "#FFFFFF", "default_font": "default" },
            "elements": [
                {
                    "id": "container",
                    "type": "group",
                    "rect": { "tx": 2, "ty": 2, "tw": 16, "th": 14 },
                    "layout": {
                        "direction": "Vertical",
                        "gap": 2,
                        "padding": { "top": 1, "bottom": 1, "left": 1, "right": 1 }
                    },
                    "clip": true,
                    "children": [
                        {
                            "type": "text",
                            "rect": { "tx": 0, "ty": 0, "tw": 14, "th": 2 },
                            "value": "ITEM",
                            "color": "black"
                        },
                        {
                            "type": "text",
                            "rect": { "tx": 0, "ty": 2, "tw": 14, "th": 2 },
                            "value": "DESCRIPTION",
                            "color": "darkgray",
                            "wrap": true
                        }
                    ]
                }
            ]
        }"##;

        let layout = parse_layout(json).expect("group layout should parse");
        assert_eq!(layout.elements.len(), 1);
        assert_eq!(layout.elements[0].element_type, "group");
        assert_eq!(layout.elements[0].id, "container");

        // Verify group params
        if let crate::layout_engine::types::ElementParams::Group(ref gp) = layout.elements[0].params
        {
            assert_eq!(gp.children.len(), 2);
            assert!(gp.clip);
            assert!(matches!(
                gp.layout.direction,
                Some(crate::layout_engine::types::Direction::Vertical)
            ));
            assert_eq!(gp.layout.gap, 2);
        } else {
            panic!("expected Group params");
        }
    }

    // ── test_parse_custom_element ─────────────────────────────────────

    #[test]
    fn test_parse_custom_element() {
        let json = r##"{
            "schema_version": 1,
            "screen": "custom",
            "elements": [
                {
                    "type": "custom:monster_sprite",
                    "rect": { "tx": 0, "ty": 0, "tw": 7, "th": 7 },
                    "sprite_id": 25,
                    "palette": "default"
                }
            ]
        }"##;

        let layout = parse_layout(json).expect("custom element should parse");
        assert_eq!(layout.elements.len(), 1);
        assert_eq!(layout.elements[0].element_type, "custom:monster_sprite");
    }

    // ── test_parse_z_index ────────────────────────────────────────────

    #[test]
    fn test_parse_z_index_default() {
        let json = r##"{
            "schema_version": 1,
            "screen": "z_test",
            "elements": [
                { "type": "text", "rect": { "tx": 0, "ty": 0 }, "value": "bg" },
                { "type": "text", "rect": { "tx": 0, "ty": 0 }, "value": "fg", "z_index": 10 }
            ]
        }"##;

        let layout = parse_layout(json).expect("should parse");
        assert_eq!(layout.elements[0].z_index, 0); // default
        assert_eq!(layout.elements[1].z_index, 10);
    }

    // ── test_parse_visible ────────────────────────────────────────────

    #[test]
    fn test_parse_visible_default_and_explicit() {
        let json = r##"{
            "schema_version": 1,
            "screen": "vis_test",
            "elements": [
                { "type": "text", "rect": { "tx": 0, "ty": 0 }, "value": "visible" },
                { "type": "text", "rect": { "tx": 0, "ty": 0 }, "value": "hidden", "visible": false }
            ]
        }"##;

        let layout = parse_layout(json).expect("should parse");
        let ctx = crate::layout_engine::types::DataContext::new();
        assert!(layout.elements[0].visible.eval(&ctx)); // default true
        assert!(!layout.elements[1].visible.eval(&ctx)); // explicit false
    }

    // ── test_parse_flex_list_layout ───────────────────────────────────

    #[test]
    fn test_parse_flex_list_layout() {
        let json = r##"{
            "schema_version": 1,
            "screen": "mart",
            "elements": [
                {
                    "type": "flex_list",
                    "rect": { "tx": 1, "ty": 3, "tw": 18, "th": 12 },
                    "items": "{inventory}",
                    "cursor": 0,
                    "gap": 0,
                    "padding": { "top": 0, "bottom": 0, "left": 1, "right": 0 },
                    "item_layout": [
                        { "field": "name", "width": 12, "align": "Left" },
                        { "field": "price", "width": 5, "align": "Right", "prefix": "$" }
                    ]
                }
            ]
        }"##;

        let layout = parse_layout(json).expect("flex_list should parse");
        assert_eq!(layout.elements.len(), 1);
        assert_eq!(layout.elements[0].element_type, "flex_list");
    }
}
