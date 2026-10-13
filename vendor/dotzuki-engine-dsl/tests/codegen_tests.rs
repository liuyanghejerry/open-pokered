//! Comprehensive codegen roundtrip tests.
//!
//! Each test exercises the full compilation pipeline:
//! DSL string → Lexer::tokenize → Parser::parse → Codegen → verify output.
//!
//! Run with: `cargo test -p dotzuki-engine-dsl -- codegen_tests`

use dotzuki_engine_dsl::ast::Document;
use dotzuki_engine_dsl::codegen;
use dotzuki_engine_dsl::lexer::Lexer;
use dotzuki_engine_dsl::parser::Parser;
use dotzuki_engine_dsl::sourcemap::SourceMapBuilder;

// ── helpers ──────────────────────────────────────────────────────────────

/// Parse a DSL string into a `Document::Scene(GameScene)`, expecting success.
fn parse_scene(dsl: &str) -> Document {
    let mut lexer = Lexer::new(dsl, "test.scene");
    let tokens = lexer.tokenize().expect("Lexer should tokenize valid DSL");
    let (doc, errors) = Parser::new(tokens, dsl).parse();
    assert!(
        errors.is_empty(),
        "Parser errors for DSL:\n{}\nErrors: {:?}",
        dsl,
        errors
    );
    doc.expect("Parser should produce a Document")
}

/// Extract `GameScene` from a `Document`, panicking if not a scene.
fn expect_scene(doc: &Document) -> &dotzuki_engine_dsl::ast::GameScene {
    match doc {
        Document::Scene(scene) => scene,
        Document::Screen(_) => panic!("Expected GameScene, got Screen"),
        Document::Components(_) => panic!("Expected GameScene, got component declarations"),
    }
}

/// Helper: extract storylines from the document and compile with sourcemap.
fn compile_storylines(doc: &Document) -> (String, SourceMapBuilder) {
    let scene = expect_scene(doc);
    let storyline = &scene.storylines[0];
    let sb = dotzuki_engine_dsl::ast::StorylineBlock {
        statements: storyline.statements.clone(),
        span: storyline.span.clone(),
    };
    let mut sm = SourceMapBuilder::new("test.scene", "test.scene.js");
    let js = codegen::js_storyline::compile_storyline("main", &sb, &mut sm);
    (js, sm)
}

/// Helper: compile a specific storyline by index, using its actual name.
fn compile_storyline_by_index(doc: &Document, idx: usize) -> (String, SourceMapBuilder) {
    let scene = expect_scene(doc);
    let storyline = &scene.storylines[idx];
    let sb = dotzuki_engine_dsl::ast::StorylineBlock {
        statements: storyline.statements.clone(),
        span: storyline.span.clone(),
    };
    let mut sm = SourceMapBuilder::new("test.scene", "test.scene.js");
    let js = codegen::js_storyline::compile_storyline(&storyline.name, &sb, &mut sm);
    (js, sm)
}

/// Helper: compile ALL storylines in a scene into combined JS output.
fn compile_all_storylines(doc: &Document) -> String {
    let scene = expect_scene(doc);
    let mut sm = SourceMapBuilder::new("test.scene", "test.scene.js");
    let mut all_js = String::new();
    for storyline in &scene.storylines {
        let sb = dotzuki_engine_dsl::ast::StorylineBlock {
            statements: storyline.statements.clone(),
            span: storyline.span.clone(),
        };
        let js = codegen::js_storyline::compile_storyline(&storyline.name, &sb, &mut sm);
        all_js.push_str(&js);
        all_js.push('\n');
    }
    all_js
}

// ══════════════════════════════════════════════════════════════════════════════
// Storyline tests (10)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_codegen_speaker_single() {
    let dsl = "\
game_scene Test {
  @storylines {
    @speaker(\"Prof\") {
      \"Hello!\"
    }
  }
}";
    let doc = parse_scene(dsl);
    let (js, _sm) = compile_storylines(&doc);

    assert!(
        js.contains("await game.showText("),
        "Expected game.showText call"
    );
    assert!(js.contains("Prof:"), "Expected speaker prefix");
    assert!(js.contains("Hello!"), "Expected text content");
    assert!(
        js.contains("export async function storyline_main()"),
        "Expected function wrapper"
    );
}

#[test]
fn test_codegen_speaker_multiple() {
    let dsl = "\
game_scene Test {
  @storylines {
    @speaker(\"Prof\") {
      \"Hello!\"
      \"Welcome to the lab!\"
    }
  }
}";
    let doc = parse_scene(dsl);
    let (js, _sm) = compile_storylines(&doc);

    assert!(js.contains("await game.showText("));
    assert!(js.contains("Prof: Hello!"));
    assert!(js.contains("Welcome to the lab!"));
    assert!(js.contains("\\n"));
}

#[test]
fn test_codegen_choice_two_options() {
    let dsl = "\
game_scene Test {
  @storylines {
    @choice {
      @option(\"Yes\") {
        @speaker(\"Prof\") {
          \"Great!\"
        }
      }
      @option(\"No\") {
        @speaker(\"Prof\") {
          \"Too bad.\"
        }
      }
    }
  }
}";
    let doc = parse_scene(dsl);
    let (js, _sm) = compile_storylines(&doc);

    assert!(js.contains("const choice = await game.showChoice([\"Yes\", \"No\"]);"));
    assert!(js.contains("if (choice === 0) {"));
    assert!(js.contains("} else {"));
    assert!(js.contains("Prof: Great!"));
    assert!(js.contains("Prof: Too bad."));
}

#[test]
fn test_codegen_choice_three_options() {
    let dsl = "\
game_scene Test {
  @storylines {
    @choice {
      @option(\"A\") {
        @speaker(\"Narrator\") {
          \"Option A\"
        }
      }
      @option(\"B\") {
        @speaker(\"Narrator\") {
          \"Option B\"
        }
      }
      @option(\"C\") {
        @speaker(\"Narrator\") {
          \"Option C\"
        }
      }
    }
  }
}";
    let doc = parse_scene(dsl);
    let (js, _sm) = compile_storylines(&doc);

    assert!(js.contains("if (choice === 0) {"));
    assert!(js.contains("else if (choice === 1) {"));
    assert!(js.contains("} else {"));
    assert!(js.contains("\"A\", \"B\", \"C\""));
}

#[test]
fn test_codegen_if_else() {
    let dsl = "\
game_scene Test {
  @variables {
    gold = 500
  }
  @storylines {
    @if (gold > 100) {
      @speaker(\"Shopkeeper\") {
        \"You have enough!\"
      }
    } @else {
      @speaker(\"Shopkeeper\") {
        \"Not enough.\"
      }
    }
  }
}";
    let doc = parse_scene(dsl);
    let (js, _sm) = compile_storylines(&doc);

    assert!(js.contains("if ((gold > 100)) {"));
    assert!(js.contains("Shopkeeper: You have enough!"));
    assert!(js.contains("} else {"));
    assert!(js.contains("Shopkeeper: Not enough."));
}

#[test]
fn test_codegen_if_no_else() {
    let dsl = "\
game_scene Test {
  @variables {
    hasKey = true
  }
  @storylines {
    @if (hasKey == true) {
      @speaker(\"Guard\") {
        \"Door opens.\"
      }
    }
  }
}";
    let doc = parse_scene(dsl);
    let (js, _sm) = compile_storylines(&doc);

    assert!(js.contains("if ((hasKey === true)) {"));
    assert!(js.contains("Guard: Door opens."));
    assert!(!js.contains("else"), "Should not contain else branch");
}

#[test]
fn test_codegen_nested_choice_in_if() {
    let dsl = "\
game_scene Test {
  @variables {
    hasStarter = true
  }
  @storylines {
    @if (hasStarter) {
      @speaker(\"Prof\") {
        \"You have a monster.\"
      }
    } @else {
      @choice {
        @option(\"Flambit\") {
          @speaker(\"Prof\") {
            \"Fiery!\"
          }
        }
        @option(\"Aquakit\") {
          @speaker(\"Prof\") {
            \"Water!\"
          }
        }
      }
    }
  }
}";
    let doc = parse_scene(dsl);
    let (js, _sm) = compile_storylines(&doc);

    assert!(js.contains("if (hasStarter) {"));
    assert!(js.contains("Prof: You have a monster."));
    assert!(js.contains("const choice = await game.showChoice"));
    assert!(js.contains("\"Flambit\""));
    assert!(js.contains("\"Aquakit\""));
    assert!(js.contains("Prof: Fiery!"));
    assert!(js.contains("Prof: Water!"));
}

#[test]
fn test_codegen_each_loop() {
    let dsl = "\
game_scene Test {
  @variables {
    inventory = \"items\"
  }
  @storylines {
    @each item in inventory {
      @speaker(\"Shopkeeper\") {
        \"Here you go.\"
      }
    }
  }
}";
    let doc = parse_scene(dsl);
    let (js, _sm) = compile_storylines(&doc);

    assert!(js.contains("for (const item of inventory) {"));
    assert!(js.contains("Shopkeeper: Here you go."));
}

#[test]
fn test_codegen_variable_assign() {
    let dsl = "\
game_scene Test {
  @storylines {
    gold = 500
    playerName = \"RED\"
  }
}";
    let doc = parse_scene(dsl);
    let (js, _sm) = compile_storylines(&doc);

    assert!(js.contains("let gold = 500;"));
    assert!(js.contains("let playerName = \"RED\";"));
    let gold_pos = js.find("let gold").unwrap();
    let player_pos = js.find("let playerName").unwrap();
    assert!(gold_pos < player_pos);
}

#[test]
fn test_codegen_expression_inline() {
    let dsl = "\
game_scene Test {
  @variables {
    npcName = \"Professor Maple\"
  }
  @storylines {
    @speaker(npcName) {
      \"Hello from a variable speaker!\"
    }
  }
}";
    let doc = parse_scene(dsl);
    let (js, _sm) = compile_storylines(&doc);

    assert!(js.contains("await game.showText("));
    assert!(js.contains("${npcName}"));
    assert!(js.contains("Hello from a variable speaker!"));
}

// ══════════════════════════════════════════════════════════════════════════════
// UI tests (6)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_codegen_ui_panel() {
    let dsl = "\
game_scene Test {
  ui {
    panel {
      text(\"Hello\") {
      }
    }
  }
}";
    let doc = parse_scene(dsl);
    let scene = expect_scene(&doc);
    let ui = scene.ui.as_ref().expect("Missing ui block");
    let json = codegen::json_ui::compile_ui(ui).expect("UI compile should succeed");

    assert!(json.contains("\"type\": \"border\""));
    assert!(json.contains("\"type\": \"text\""));
    assert!(json.contains("\"value\": \"Hello\""));
}

#[test]
fn test_codegen_ui_button() {
    let dsl = "\
game_scene Test {
  ui {
    panel {
      button(\"Buy\") {
        on_click = \"buy_item\"
      }
    }
  }
}";
    let doc = parse_scene(dsl);
    let scene = expect_scene(&doc);
    let ui = scene.ui.as_ref().expect("Missing ui block");
    let json = codegen::json_ui::compile_ui(ui).expect("UI compile should succeed");

    assert!(json.contains("\"type\": \"text\""));
    assert!(json.contains("\"value\": \"Buy\""));
    assert!(json.contains("\"interactive\": true"));
    assert!(json.contains("\"onClick\": \"buy_item\""));
}

#[test]
fn test_codegen_ui_list() {
    let dsl = "\
game_scene Test {
  ui {
    panel {
      list(\"items\") {
      }
    }
  }
}";
    let doc = parse_scene(dsl);
    let scene = expect_scene(&doc);
    let ui = scene.ui.as_ref().expect("Missing ui block");

    let json = codegen::json_ui::compile_ui(ui).expect("UI compile should succeed");
    assert!(json.contains("\"type\": \"list\""));
    assert!(json.contains("\"items\": \"{items}\""));
}

#[test]
fn test_codegen_ui_image() {
    let dsl = "\
game_scene Test {
  ui {
    panel {
      image(\"ui/panel.png\") {
        slice = \"[8,8,8,8]\"
      }
    }
  }
}";
    let doc = parse_scene(dsl);
    let scene = expect_scene(&doc);
    let ui = scene.ui.as_ref().expect("Missing ui block");
    let json = codegen::json_ui::compile_ui(ui).expect("UI compile should succeed");

    assert!(json.contains("\"type\": \"image\""));
    assert!(json.contains("\"src\": \"ui/panel.png\""));
    assert!(json.contains("\"nineSlice\": \"[8,8,8,8]\""));
}

#[test]
fn test_codegen_ui_full_layout() {
    let dsl = "\
game_scene Test {
  ui {
    panel {
      text(\"Title\") {
      }
      button(\"Close\") {
        on_click = \"close_screen\"
      }
    }
    panel {
      text(\"Footer\") {
      }
    }
  }
}";
    let doc = parse_scene(dsl);
    let scene = expect_scene(&doc);
    let ui = scene.ui.as_ref().expect("Missing ui block");
    let json = codegen::json_ui::compile_ui(ui).expect("UI compile should succeed");

    assert!(json.contains("\"type\": \"group\""));
    assert!(json.contains("\"children\""));
    assert!(json.contains("\"type\": \"border\""));
    assert!(json.contains("\"Title\""));
    assert!(json.contains("\"Close\""));
    assert!(json.contains("\"Footer\""));
}

#[test]
fn test_codegen_ui_layout_props() {
    let dsl = "\
game_scene Test {
  ui {
    panel {
      width = 200
      height = 100
      padding = 8
      margin = 4
      align = \"center\"
      text(\"Styled\") {
      }
    }
  }
}";
    let doc = parse_scene(dsl);
    let scene = expect_scene(&doc);
    let ui = scene.ui.as_ref().expect("Missing ui block");
    let json = codegen::json_ui::compile_ui(ui).expect("UI compile should succeed");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("Must be valid JSON");

    let panel = &parsed["children"][0];
    assert_eq!(panel["width"], 200);
    assert_eq!(panel["height"], 100);
    assert_eq!(panel["padding"][0], 8);
    assert_eq!(panel["padding"][1], 8);
    assert_eq!(panel["padding"][2], 8);
    assert_eq!(panel["padding"][3], 8);
    assert_eq!(panel["margin"][0], 4);
    assert_eq!(panel["margin"][1], 4);
    assert_eq!(panel["margin"][2], 4);
    assert_eq!(panel["margin"][3], 4);
    assert_eq!(panel["align"], "center");
}

// ══════════════════════════════════════════════════════════════════════════════
// Theme tests (4)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_codegen_theme_simple() {
    let dsl = "\
game_scene Test {
  @theme dark {
    primary = \"#c9a03d\"
    background = \"#1a1a1e\"
  }
}";
    let doc = parse_scene(dsl);
    let scene = expect_scene(&doc);
    assert_eq!(scene.themes.len(), 1);
    let json = codegen::json_theme::compile_theme(&scene.themes[0]);

    assert!(json.contains("\"dark\""));
    assert!(json.contains("\"#c9a03d\""));
    assert!(json.contains("\"#1a1a1e\""));
}

#[test]
fn test_codegen_style_no_extends() {
    let dsl = "\
game_scene Test {
  @style base {
    padding = 12
    color = \"red\"
  }
}";
    let doc = parse_scene(dsl);
    let scene = expect_scene(&doc);
    assert_eq!(scene.styles.len(), 1);
    let json = codegen::json_theme::compile_style(&scene.styles[0]);

    assert!(json.contains("\"base\""));
    assert!(json.contains("\"padding\""));
    assert!(json.contains("\"color\""));
    assert!(!json.contains("\"extends\""));
}

#[test]
fn test_codegen_style_with_extends() {
    let dsl = "\
game_scene Test {
  @style base {
    padding = 12
  }
  @style derived : base {
    margin = 8
  }
}";
    let doc = parse_scene(dsl);
    let scene = expect_scene(&doc);
    assert_eq!(scene.styles.len(), 2);

    let json = codegen::json_theme::compile_styles_resolved(&scene.styles);

    assert!(json.contains("\"derived\""));
    assert!(json.contains("\"base\""));
    assert!(json.contains("\"extends\""));
    assert!(json.contains("\"padding\""));
}

#[test]
fn test_codegen_style_inheritance_chain() {
    let dsl = "\
game_scene Test {
  @style grandparent {
    font = \"Arial\"
    size = 14
  }
  @style parent : grandparent {
    size = 16
    weight = \"bold\"
  }
  @style child : parent {
    color = \"blue\"
  }
}";
    let doc = parse_scene(dsl);
    let scene = expect_scene(&doc);
    assert_eq!(scene.styles.len(), 3);

    let json = codegen::json_theme::compile_styles_resolved(&scene.styles);

    assert!(json.contains("\"font\": \"Arial\""));
    assert!(json.contains("16"));
    assert!(json.contains("\"bold\""));
    assert!(json.contains("\"blue\""));
    assert!(json.contains("\"inheritance_chain\""));
    assert!(json.contains("\"grandparent\""));
}

// ══════════════════════════════════════════════════════════════════════════════
// Atlas tests (4)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_codegen_atlas_single_slice() {
    let dsl = "\
game_scene Test {
  @atlas \"ui\" {
    source = \"atlas.png\"
    regions = {
      btn = [0, 0, 64, 64, slice=8]
    }
  }
}";
    let doc = parse_scene(dsl);
    let scene = expect_scene(&doc);
    assert_eq!(scene.atlases.len(), 1);
    let json = codegen::json_atlas::compile_atlas(&scene.atlases[0]);

    assert!(json.contains("\"ui\""));
    assert!(json.contains("\"atlas.png\""));
    assert!(json.contains("\"btn\""));
    assert!(json.contains("\"nineSlice\""));
    assert!(json.contains("\"top\": 8"));
    assert!(json.contains("\"right\": 8"));
    assert!(json.contains("\"bottom\": 8"));
    assert!(json.contains("\"left\": 8"));
}

#[test]
fn test_codegen_atlas_array_slice() {
    let dsl = "\
game_scene Test {
  @atlas \"ui\" {
    source = \"atlas.png\"
    regions = {
      panel = [0, 0, 128, 128, slice=[12, 16, 12, 16]]
    }
  }
}";
    let doc = parse_scene(dsl);
    let scene = expect_scene(&doc);
    assert_eq!(scene.atlases.len(), 1);
    let json = codegen::json_atlas::compile_atlas(&scene.atlases[0]);

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let ns = &parsed["regions"][0]["nineSlice"];
    assert_eq!(ns["top"], 12);
    assert_eq!(ns["right"], 16);
    assert_eq!(ns["bottom"], 12);
    assert_eq!(ns["left"], 16);
}

#[test]
fn test_codegen_atlas_no_slice() {
    let dsl = "\
game_scene Test {
  @atlas \"ui\" {
    source = \"atlas.png\"
    regions = {
      icon = [0, 0, 32, 32]
    }
  }
}";
    let doc = parse_scene(dsl);
    let scene = expect_scene(&doc);
    assert_eq!(scene.atlases.len(), 1);
    let json = codegen::json_atlas::compile_atlas(&scene.atlases[0]);

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let region = &parsed["regions"][0];
    assert_eq!(region["name"], "icon");
    assert!(
        region.get("nineSlice").is_none(),
        "Should omit nineSlice for no-slice regions"
    );
}

#[test]
fn test_codegen_atlas_multiple_regions() {
    let dsl = "\
game_scene Test {
  @atlas \"battle_ui\" {
    source = \"assets/battle/atlas.png\"
    regions = {
      btn_normal = [0, 0, 64, 64, slice=8]
      btn_hover = [64, 0, 64, 64, slice=8]
      btn_pressed = [128, 0, 64, 64, slice=8]
      icon = [0, 128, 32, 32]
    }
  }
}";
    let doc = parse_scene(dsl);
    let scene = expect_scene(&doc);
    assert_eq!(scene.atlases.len(), 1);
    let json = codegen::json_atlas::compile_atlas(&scene.atlases[0]);

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let regions = parsed["regions"].as_array().unwrap();
    assert_eq!(regions.len(), 4);
    assert_eq!(regions[0]["name"], "btn_normal");
    assert_eq!(regions[1]["name"], "btn_hover");
    assert_eq!(regions[2]["name"], "btn_pressed");
    assert_eq!(regions[3]["name"], "icon");
    assert!(regions[0].get("nineSlice").is_some());
    assert!(regions[3].get("nineSlice").is_none());
}

// ══════════════════════════════════════════════════════════════════════════════
// Source map tests (3)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_codegen_sourcemap_in_output() {
    let dsl = "\
game_scene Test {
  @storylines {
    @speaker(\"Prof\") {
      \"Hello!\"
    }
  }
}";
    let doc = parse_scene(dsl);
    let (js, sm) = compile_storylines(&doc);
    let comment = sm.finalize();

    let combined = format!("{}{}", js, comment);
    assert!(
        combined.contains("//# sourceMappingURL="),
        "Output should contain sourceMappingURL"
    );
    assert!(
        combined.contains("data:application/json;charset=utf-8;base64,"),
        "Should use inline base64 data URL format"
    );
}

#[test]
fn test_codegen_sourcemap_line_mapping() {
    let dsl = "\
game_scene Test {
  @storylines {
    @speaker(\"Prof\") {
      \"Hello!\"
      \"Welcome!\"
    }
  }
}";
    let doc = parse_scene(dsl);
    let (_js, sm) = compile_storylines(&doc);

    // At least one mapping should exist (from speaker compilation)
    let mappings = sm.mappings();
    assert!(
        !mappings.is_empty(),
        "Should have at least one source mapping"
    );

    for m in mappings {
        assert!(
            m.generated_line < 100,
            "Generated line should be reasonable"
        );
    }
}

#[test]
fn test_codegen_sourcemap_roundtrip() {
    let mut builder = SourceMapBuilder::new("roundtrip.scene", "roundtrip.scene.js");

    let mappings = vec![
        (0u32, 0u32, 1u32, 0u32),
        (0, 5, 1, 10),
        (1, 0, 3, 2),
        (2, 8, 7, 0),
        (5, 0, 12, 0),
    ];

    for &(gl, gc, sl, sc) in &mappings {
        builder.add_mapping(gl, gc, sl, sc);
    }

    let comment = builder.finalize();

    let data_url = comment
        .strip_prefix("//# sourceMappingURL=")
        .expect("Missing sourceMappingURL prefix");
    let payload = data_url
        .strip_prefix("data:application/json;charset=utf-8;base64,")
        .expect("Not a base64 data URL");
    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, payload)
        .expect("Base64 decode failed");

    let sm = sourcemap::SourceMap::from_reader(decoded.as_slice()).expect("SourceMap parse failed");

    assert_eq!(sm.get_token_count() as usize, mappings.len());

    for (gl, gc, sl, sc) in &mappings {
        let token = sm
            .lookup_token(*gl, *gc)
            .unwrap_or_else(|| panic!("No token at generated ({},{})", gl, gc));
        assert_eq!(
            token.get_src_line(),
            *sl,
            "Generated ({},{}) → source line mismatch",
            gl,
            gc
        );
        assert_eq!(
            token.get_src_col(),
            *sc,
            "Generated ({},{}) → source column mismatch",
            gl,
            gc
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Command tests (1)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_directive_command_compiles_to_js() {
    let dsl = "\
game_scene CmdTest {
  @storylines {
    @command(\"heal\")
    @command(\"giveMonster\", \"SPARKIT\", 5)
  }
}";
    let doc = parse_scene(dsl);
    let (js, sm) = compile_storylines(&doc);
    let comment = sm.finalize();
    let combined = format!("{}{}", js, comment);

    assert!(
        js.contains("await game[\"heal\"]()"),
        "should emit game[\"heal\"]()"
    );
    assert!(
        js.contains("await game[\"giveMonster\"](\"SPARKIT\", 5)"),
        "should emit game[\"giveMonster\"]"
    );
    assert!(
        combined.contains("sourceMappingURL"),
        "should include source map"
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Named storyline function name tests (3)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_named_storyline_function_name() {
    let dsl = "\
game_scene Test {
  @storyline(\"delivery\") {
    @speaker(\"Prof\") {
      \"Package arrived!\"
    }
  }
}";
    let doc = parse_scene(dsl);
    let (js, _sm) = compile_storyline_by_index(&doc, 0);

    assert!(
        js.contains("export async function storyline_delivery()"),
        "Named storyline should generate storyline_delivery(), got:\n{}",
        js
    );
    assert!(
        js.contains("Prof: Package arrived!"),
        "Should contain speaker content"
    );
}

#[test]
fn test_unnamed_storyline_function_name() {
    let dsl = "\
game_scene Test {
  @storylines {
    @speaker(\"Prof\") {
      \"Hello!\"
    }
  }
}";
    let doc = parse_scene(dsl);
    let (js, _sm) = compile_storyline_by_index(&doc, 0);

    assert!(
        js.contains("export async function storyline_main()"),
        "Unnamed storyline should generate storyline_main(), got:\n{}",
        js
    );
    assert!(
        js.contains("Prof: Hello!"),
        "Should contain speaker content"
    );
}

#[test]
fn test_multiple_storylines() {
    let dsl = "\
game_scene Test {
  @storyline(\"intro\") {
    @speaker(\"Prof\") {
      \"Welcome!\"
    }
  }
  @storyline(\"delivery\") {
    @speaker(\"Mailman\") {
      \"Package!\"
    }
  }
}";
    let doc = parse_scene(dsl);
    let all_js = compile_all_storylines(&doc);

    assert!(
        all_js.contains("export async function storyline_intro()"),
        "Should contain storyline_intro(), got:\n{}",
        all_js
    );
    assert!(
        all_js.contains("export async function storyline_delivery()"),
        "Should contain storyline_delivery(), got:\n{}",
        all_js
    );
    assert!(
        all_js.contains("Prof: Welcome!"),
        "Should contain intro content"
    );
    assert!(
        all_js.contains("Mailman: Package!"),
        "Should contain delivery content"
    );

    // Both functions should appear in the output
    let intro_pos = all_js.find("storyline_intro").unwrap();
    let delivery_pos = all_js.find("storyline_delivery").unwrap();
    assert!(
        intro_pos < delivery_pos,
        "storyline_intro should appear before storyline_delivery"
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Condition / argument call namespacing (1)
// ══════════════════════════════════════════════════════════════════════════════

/// Regression: API calls inside conditions/arguments must be namespaced to the
/// `game` object. The engine exposes APIs only as methods on `game` — there are
/// no bare globals — so a bare `getFlag(...)` throws `ReferenceError` at runtime
/// (and the engine silently swallows the rejection). See
/// `js_storyline::compile_expression`.
#[test]
fn test_condition_calls_are_game_namespaced() {
    let dsl = "\
game_scene Test {
  @storyline(\"checkStarter\") {
    @if (getFlag(\"GOT_STARTER\")) {
      @speaker(\"Prof\") { \"Already got one.\" }
    } @else {
      @speaker(\"Prof\") { \"Pick one.\" }
    }
  }
}";
    let doc = parse_scene(dsl);
    let (js, _sm) = compile_storyline_by_index(&doc, 0);
    assert!(
        js.contains("if (game.getFlag(\"GOT_STARTER\"))"),
        "condition call must be game-namespaced, got:\n{}",
        js
    );
    // The bare, unnamespaced form must never appear — it would ReferenceError.
    assert!(
        !js.contains("if (getFlag("),
        "bare unnamespaced getFlag( must not appear, got:\n{}",
        js
    );
}

/// Regression: assigning a game-API call must `await` it, so
/// `result = startBattle(...)` captures the resolved outcome (e.g. "win") rather
/// than a pending Promise — otherwise `@if (result == "win")` can never be true.
#[test]
fn test_assignment_from_call_is_awaited() {
    let dsl = "\
game_scene Test {
  @storyline(\"fight\") {
    result = startBattle(\"OPP_ROCCO1\")
    @if (result == \"win\") {
      @speaker(\"\") { \"You won!\" }
    }
  }
}";
    let doc = parse_scene(dsl);
    let (js, _sm) = compile_storyline_by_index(&doc, 0);
    assert!(
        js.contains("result = await game.startBattle("),
        "assignment from a game-API call must await, got:\n{}",
        js
    );
    // The binding is declared at function scope (`let result;`) so branches
    // can read it — see the block-scoping regression test in js_storyline.
    assert!(
        js.contains("let result;"),
        "function-scoped decl, got:\n{}",
        js
    );
}

#[test]
fn test_codegen_ui_visible_template_and_bool() {
    // `visible` must support both forms: a bool literal and a `"{binding}"`
    // template string. The template form used to be silently dropped by the
    // parser (options.gui's active-row ▶ cursors rendered on every row).
    let dsl = "\
game_scene Test {
  ui {
    panel {
      cursor {
        glyph = \"▶\"
        visible = \"{r0_active}\"
      }
      text(\"Hi\") {
        visible = false
      }
    }
  }
}";
    let doc = parse_scene(dsl);
    let scene = expect_scene(&doc);
    let ui = scene.ui.as_ref().expect("Missing ui block");
    let json = codegen::json_ui::compile_ui(ui).expect("UI compile should succeed");

    assert!(
        json.contains("\"visible\": \"{r0_active}\""),
        "template visible must survive: {json}"
    );
    assert!(
        json.contains("\"visible\": false"),
        "bool visible must survive: {json}"
    );
}
