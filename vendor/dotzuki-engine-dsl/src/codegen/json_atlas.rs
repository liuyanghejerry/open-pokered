// Atlas → JSON codegen for the Game DSL compiler.
//
// Transforms AST `@atlas` definitions into structured JSON for the
// renderer's 9-slice atlas system.

use crate::ast::*;
use serde_json::{json, Value};

/// Compile an `@atlas` AST node to JSON.
///
/// # Example
///
/// ```
/// use dotzuki_engine_dsl::ast::*;
/// use dotzuki_engine_dsl::codegen::json_atlas::compile_atlas;
///
/// let atlas = Atlas {
///     name: "ui".into(),
///     source: "atlas.png".into(),
///     regions: vec![
///         AtlasRegion {
///             name: "btn".into(),
///             x: 0, y: 0, w: 64, h: 64,
///             nine_slice: Some([8, 8, 8, 8]),
///             span: SourceSpan::point("test.scene", 1, 1),
///         }
///     ],
///     span: SourceSpan::point("test.scene", 1, 1),
/// };
///
/// let json = compile_atlas(&atlas);
/// assert!(json.contains("\"ui\""));
/// assert!(json.contains("nineSlice"));
/// ```
pub fn compile_atlas(atlas: &Atlas) -> String {
    let regions: Vec<Value> = atlas.regions.iter().map(compile_region).collect();
    serde_json::to_string_pretty(&json!({
        "name": atlas.name,
        "source": atlas.source,
        "regions": regions,
    }))
    .unwrap()
}

/// Compile a single `AtlasRegion` to a JSON object.
///
/// Regions without `nine_slice` omit the field entirely.
fn compile_region(region: &AtlasRegion) -> Value {
    let mut obj = json!({
        "name": region.name,
        "x": region.x,
        "y": region.y,
        "w": region.w,
        "h": region.h,
    });
    if let Some(slice) = &region.nine_slice {
        obj["nineSlice"] = json!({
            "top": slice[0],
            "right": slice[1],
            "bottom": slice[2],
            "left": slice[3],
        });
    }
    obj
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> SourceSpan {
        SourceSpan::point("test.scene", 1, 1)
    }

    fn make_region(
        name: &str,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        nine_slice: Option<[u32; 4]>,
    ) -> AtlasRegion {
        AtlasRegion {
            name: name.into(),
            x,
            y,
            w,
            h,
            nine_slice,
            span: span(),
        }
    }

    #[test]
    fn test_atlas_single_slice() {
        // slice=8 → AST stores Some([8,8,8,8]) → JSON nineSlice with uniform edges
        let atlas = Atlas {
            name: "ui".into(),
            source: "atlas.png".into(),
            regions: vec![make_region("btn", 0, 0, 64, 64, Some([8, 8, 8, 8]))],
            span: span(),
        };
        let json = compile_atlas(&atlas);
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["name"], "ui");
        assert_eq!(parsed["source"], "atlas.png");
        assert_eq!(parsed["regions"][0]["name"], "btn");
        assert_eq!(parsed["regions"][0]["x"], 0);
        assert_eq!(parsed["regions"][0]["y"], 0);
        assert_eq!(parsed["regions"][0]["w"], 64);
        assert_eq!(parsed["regions"][0]["h"], 64);

        let ns = &parsed["regions"][0]["nineSlice"];
        assert_eq!(ns["top"], 8);
        assert_eq!(ns["right"], 8);
        assert_eq!(ns["bottom"], 8);
        assert_eq!(ns["left"], 8);
    }

    #[test]
    fn test_atlas_array_slice() {
        // slice=[12,16,12,16] → per-edge slice values
        let atlas = Atlas {
            name: "ui".into(),
            source: "atlas.png".into(),
            regions: vec![make_region("panel", 0, 0, 128, 128, Some([12, 16, 12, 16]))],
            span: span(),
        };
        let json = compile_atlas(&atlas);
        let parsed: Value = serde_json::from_str(&json).unwrap();

        let ns = &parsed["regions"][0]["nineSlice"];
        assert_eq!(ns["top"], 12);
        assert_eq!(ns["right"], 16);
        assert_eq!(ns["bottom"], 12);
        assert_eq!(ns["left"], 16);
    }

    #[test]
    fn test_atlas_multiple_regions() {
        let atlas = Atlas {
            name: "ui".into(),
            source: "ui_atlas.png".into(),
            regions: vec![
                make_region("btn_normal", 0, 0, 64, 64, Some([8, 8, 8, 8])),
                make_region("btn_hover", 64, 0, 64, 64, Some([8, 8, 8, 8])),
                make_region("btn_pressed", 128, 0, 64, 64, Some([8, 8, 8, 8])),
            ],
            span: span(),
        };
        let json = compile_atlas(&atlas);
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["regions"].as_array().unwrap().len(), 3);
        assert_eq!(parsed["regions"][0]["name"], "btn_normal");
        assert_eq!(parsed["regions"][1]["name"], "btn_hover");
        assert_eq!(parsed["regions"][2]["name"], "btn_pressed");
        assert_eq!(parsed["regions"][1]["x"], 64);
    }

    #[test]
    fn test_atlas_no_slice() {
        // Region without nine_slice omits the field entirely
        let atlas = Atlas {
            name: "ui".into(),
            source: "atlas.png".into(),
            regions: vec![make_region("icon", 0, 0, 32, 32, None)],
            span: span(),
        };
        let json = compile_atlas(&atlas);
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["regions"][0]["name"], "icon");
        assert_eq!(parsed["regions"][0]["x"], 0);
        assert_eq!(parsed["regions"][0]["y"], 0);
        assert_eq!(parsed["regions"][0]["w"], 32);
        assert_eq!(parsed["regions"][0]["h"], 32);
        // nineSlice field must not be present
        assert!(
            parsed["regions"][0].get("nineSlice").is_none(),
            "region without slice should omit nineSlice"
        );
    }

    #[test]
    fn test_full_atlas_definition() {
        // Complete atlas: source path + multiple regions with mixed slice/no-slice
        let atlas = Atlas {
            name: "battle_ui".into(),
            source: "assets/battle/atlas.png".into(),
            regions: vec![
                make_region("health_bar_bg", 0, 0, 256, 32, Some([4, 4, 4, 4])),
                make_region("health_bar_fill", 0, 32, 256, 32, None),
                make_region("text_box", 0, 64, 320, 96, Some([8, 8, 8, 8])),
            ],
            span: span(),
        };
        let json = compile_atlas(&atlas);
        let parsed: Value = serde_json::from_str(&json).unwrap();

        // Top-level fields
        assert_eq!(parsed["name"], "battle_ui");
        assert_eq!(parsed["source"], "assets/battle/atlas.png");
        assert_eq!(parsed["regions"].as_array().unwrap().len(), 3);

        // Region 0: has nineSlice
        assert_eq!(parsed["regions"][0]["name"], "health_bar_bg");
        assert_eq!(parsed["regions"][0]["nineSlice"]["top"], 4);

        // Region 1: no nineSlice
        assert_eq!(parsed["regions"][1]["name"], "health_bar_fill");
        assert!(parsed["regions"][1].get("nineSlice").is_none());

        // Region 2: has nineSlice
        assert_eq!(parsed["regions"][2]["name"], "text_box");
        assert_eq!(parsed["regions"][2]["nineSlice"]["left"], 8);
        assert_eq!(parsed["regions"][2]["h"], 96);
    }

    #[test]
    fn test_atlas_slice_values_preserved() {
        // Verify all four slice values are independently preserved
        let atlas = Atlas {
            name: "frame".into(),
            source: "frame.png".into(),
            regions: vec![make_region(
                "border",
                0,
                0,
                200,
                100,
                Some([10, 15, 20, 25]),
            )],
            span: span(),
        };
        let json = compile_atlas(&atlas);
        let parsed: Value = serde_json::from_str(&json).unwrap();

        let ns = &parsed["regions"][0]["nineSlice"];
        assert_eq!(ns["top"], 10);
        assert_eq!(ns["right"], 15);
        assert_eq!(ns["bottom"], 20);
        assert_eq!(ns["left"], 25);
    }
}
