//! Round-trip integration test: compile representative `.gui` DSL layouts into
//! v2 `ScreenLayout` JSON and verify each element's `type` string deserializes
//! into the *matching* [`ElementParams`] variant.
//!
//! This guards against codegen drift between `dotzuki-engine-dsl`'s JSON output
//! and the renderer's deserializer. The classic failure it catches: a `text`
//! element whose JSON omits the canonical `value` field falls through the
//! `#[serde(untagged)]` enum to `ElementParams::Custom`, so the renderer's
//! `text` handler bails and the glyphs never draw — i.e. "blank text in the
//! LayoutEditor preview".
//!
//! Fixtures are inline engine-owned samples (see `FIXTURES` below) so the
//! contract test runs without any game data; the original walked pokered's
//! `ui_layouts` directory.

use crate::alloc_prelude::*;
use dotzuki_renderer::layout_engine::deserialize::parse_layout;
use dotzuki_renderer::layout_engine::types::{ElementParams, LayoutElement};

/// Compile `.gui` DSL source into v2 ScreenLayout JSON, mirroring the path the
/// WASM editor bridge (`compile_screen_source`) and the build pipeline use —
/// including seeding the parser with the `components.gui` prelude.
fn compile_gui(
    src: &str,
    path: &str,
    prelude: &[dotzuki_engine_dsl::ast::ComponentDecl],
) -> Result<String, String> {
    let tokens = dotzuki_engine_dsl::lexer::Lexer::new(src, path)
        .tokenize()
        .map_err(|errs| format!("lex errors: {errs:?}"))?;
    let (doc, parse_errors) = dotzuki_engine_dsl::parser::Parser::new(tokens, src)
        .with_components(prelude)
        .parse();
    if !parse_errors.is_empty() {
        return Err(format!("parse errors: {parse_errors:?}"));
    }
    match doc {
        Some(dotzuki_engine_dsl::ast::Document::Screen(screen)) => {
            dotzuki_engine_dsl::codegen::json_ui::compile_screen(&screen)
        }
        _ => Err("expected a `screen { ... }` document".to_string()),
    }
}

/// Parse the `components.gui` declarations prelude used by the fixtures.
/// An empty prelude yields no declarations.
fn load_prelude(prelude_src: &str) -> Vec<dotzuki_engine_dsl::ast::ComponentDecl> {
    if prelude_src.trim().is_empty() {
        return Vec::new();
    }
    let tokens = dotzuki_engine_dsl::lexer::Lexer::new(prelude_src, "components.gui")
        .tokenize()
        .expect("lex components.gui");
    let (doc, errors) = dotzuki_engine_dsl::parser::Parser::new(tokens, prelude_src).parse();
    assert!(errors.is_empty(), "components.gui parse errors: {errors:?}");
    match doc {
        Some(dotzuki_engine_dsl::ast::Document::Components(decls)) => decls,
        other => panic!("components.gui must contain only declarations, got {other:?}"),
    }
}

fn variant_name(params: &ElementParams) -> &'static str {
    match params {
        ElementParams::Border(_) => "Border",
        ElementParams::Text(_) => "Text",
        ElementParams::Tile(_) => "Tile",
        ElementParams::Divider(_) => "Divider",
        ElementParams::Image(_) => "Image",
        ElementParams::List(_) => "List",
        ElementParams::FlexList(_) => "FlexList",
        ElementParams::Group(_) => "Group",
        ElementParams::Cursor(_) => "Cursor",
        ElementParams::Bracket(_) => "Bracket",
        ElementParams::PixelRect(_) => "PixelRect",
        ElementParams::Custom(_) => "Custom",
    }
}

/// The params variant the renderer's dispatcher requires for a given `type`.
fn expected_variant(element_type: &str) -> Option<&'static str> {
    match element_type {
        "border" => Some("Border"),
        "text" => Some("Text"),
        "tile" => Some("Tile"),
        "divider" => Some("Divider"),
        "image" => Some("Image"),
        "list" => Some("List"),
        "flex_list" => Some("FlexList"),
        "group" => Some("Group"),
        "cursor" => Some("Cursor"),
        "bracket" => Some("Bracket"),
        "pixel_rect" => Some("PixelRect"),
        // custom:* legitimately deserializes to Custom — not a mismatch.
        t if t.starts_with("custom:") => Some("Custom"),
        _ => None,
    }
}

/// Walk an element (recursing into group/border children) and record every
/// element whose `type` string does not match its deserialized params variant.
fn collect_mismatches(el: &LayoutElement, out: &mut Vec<String>) {
    let actual = variant_name(&el.params);
    match expected_variant(&el.element_type) {
        Some(expected) if expected != actual => out.push(format!(
            "type=\"{}\" deserialized as ElementParams::{} (expected {})",
            el.element_type, actual, expected
        )),
        None => out.push(format!("unknown element type \"{}\"", el.element_type)),
        _ => {}
    }
    match &el.params {
        ElementParams::Group(g) => {
            for child in &g.children {
                collect_mismatches(child, out);
            }
        }
        // Panels (borders) may nest children (e.g. a dialog box's text). Verify
        // those round-trip too — they are what made dialog/prof_speech/battle_text
        // render a blank box.
        ElementParams::Border(b) => {
            for child in &b.children {
                collect_mismatches(child, out);
            }
        }
        _ => {}
    }
}

/// Engine-owned `.gui` fixture screens covering every element type the DSL can
/// emit. Kept inline so this codegen↔deserializer contract test runs without
/// any game data — the original walked pokered's `ui_layouts` directory, which
/// no longer exists in the standalone engine repository.
const FIXTURE_PRELUDE: &str = "";

const FIXTURES: &[(&str, &str)] = &[
    (
        "dialog.gui",
        r#"screen Dialog {
  panel {
    rect = {tx: 0, ty: 12, tw: 20, th: 6}
    style = "default"
    text("{text}") {
      rect = {tx: 1, ty: 13, tw: 18, th: 4}
      wrap = "word"
      line_spacing = 1
    }
    tile(31) {
      rect = {tx: 18, ty: 16, tw: 1, th: 1}
    }
  }
}"#,
    ),
    (
        "dex.gui",
        r#"screen Dex {
  panel {
    rect = {tx: 0, ty: 0, tw: 20, th: 18}
    style = {corner_tl: 99, edge_top: 100, corner_tr: 101, edge_left: 102, edge_right: 103, corner_bl: 108, edge_bottom: 111, corner_br: 110}
  }
  divider {
    rect = {tx: 1, ty: 9, tw: 18, th: 1}
    tiles = [122]
    repeat = 17
  }
  text("{name}") {
    rect = {tx: 9, ty: 2, tw: 10, th: 1}
  }
  tile(31) {
    rect = {tx: 18, ty: 16, tw: 1, th: 1}
  }
}"#,
    ),
    (
        "party.gui",
        r#"screen Party {
  container {
    rect = {tx: 0, ty: 0, tw: 20, th: 18}
    layout = {gap: 0}
    clip = false
    text("{mon1_name}") {
      rect = {tx: 4, ty: 0, tw: 10, th: 1}
    }
    text("L{mon1_level}") {
      rect = {tx: 14, ty: 0, tw: 3, th: 1}
    }
  }
  text(@t("B:Cancel", "B：取消")) {
    rect = {tx: 2, ty: 16, tw: 10, th: 1}
    color = "DarkGray"
  }
}"#,
    ),
    (
        "menu.gui",
        r#"screen Start {
  panel { rect = {tx: 10, ty: 0, tw: 10, th: 15} style = "default" }
  list {
    rect = {tx: 11, ty: 1, tw: 8, th: 13}
    source = "{items}"
    item_template = {height: 1, gap: 1}
    cursor = {tile: 223, position: "left"}
    max_visible = 7
  }
}"#,
    ),
    (
        "battle.gui",
        r#"screen Battle {
  image("ui/panel.png") { rect = {tx: 0, ty: 0, tw: 1, th: 1} }
  flex_list("{bag_items}") {
    rect = {tx: 1, ty: 4, tw: 18, th: 13}
    item_layout = [{field: "name", width: 14, align: "left"}, {field: "qty", width: 3, align: "right", prefix: "x"}]
    padding = {top: 1, left: 1}
    gap = 1
    cursor = {tile: 223, position: "left"}
  }
  cursor {
    rect = {tx: 9, ty: 14, tw: 1, th: 1}
    col_step = 6
    row_step = 2
  }
}"#,
    ),
];

// NOTE: `bracket` and `pixel_rect` are deliberately not in the fixture set —
// no v2 `.gui` file in the games uses them (they exist only in the legacy v1
// layout JSON and the codegen/renderer unit tests), and their DSL prop surface
// (`rect`) does not round-trip through the params structs today.

#[test]
fn all_gui_layouts_round_trip_to_matching_params() {
    let prelude = load_prelude(FIXTURE_PRELUDE);
    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for (name, src) in FIXTURES {
        let json = match compile_gui(src, name, &prelude) {
            Ok(j) => j,
            Err(e) => {
                failures.push(format!("[{name}] compile failed: {e}"));
                continue;
            }
        };
        let layout = match parse_layout(&json) {
            Ok(l) => l,
            Err(e) => {
                failures.push(format!("[{name}] parse_layout failed: {e:?}\n{json}"));
                continue;
            }
        };

        checked += 1;
        let mut file_mismatches = Vec::new();
        for el in &layout.elements {
            collect_mismatches(el, &mut file_mismatches);
        }
        for m in file_mismatches {
            failures.push(format!("[{name}] {m}"));
        }
    }

    assert!(checked > 0, "no .gui fixtures were checked");
    assert!(
        failures.is_empty(),
        "{} .gui element(s) did not round-trip to the expected params:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
