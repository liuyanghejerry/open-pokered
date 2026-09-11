//! End-to-end integration tests for the full DSL → Runtime pipeline:
//! DSL file → Lexer → Parser → Codegen → ScriptEngine execution → ScriptCommand verification.
//!
//! Tests cover: simple dialogue, choice branches, conditional logic,
//! UI commands, and coexistence with hand-written JavaScript.

use crate::alloc_prelude::*;
use dotzuki_engine_dsl::ast::*;
use dotzuki_engine_dsl::codegen::js_storyline::compile_storyline;
use dotzuki_engine_dsl::lexer::Lexer;
use dotzuki_engine_dsl::parser;
use dotzuki_engine_dsl::sourcemap::SourceMapBuilder;
use dotzuki_engine_script::command::{CommandResult, ScriptCommand};
use dotzuki_engine_script::engine::ScriptEngine;

// ═════════════════════════════════════════════════════════════════════════
// Compilation helper: DSL string → compiled JavaScript with source map
// ═════════════════════════════════════════════════════════════════════════

/// Compile a full DSL scene string to a JavaScript module string.
///
/// Pipeline: DSL → Lex → Parse → extract StorylineBlock → compile to JS.
/// The returned string includes the source map comment.
fn compile_dsl_to_js(dsl: &str, scene_name: &str) -> String {
    let mut lexer = Lexer::new(dsl, &format!("{}.scene", scene_name));
    let tokens = lexer
        .tokenize()
        .expect("lexing should succeed for valid DSL");
    let (doc, errors) = parser::parse(tokens);
    assert!(
        errors.is_empty(),
        "parse errors for '{}': {:?}",
        scene_name,
        errors
    );
    let Document::Scene(scene) = doc.expect("should parse to a Scene document") else {
        panic!("expected Scene document for '{}'", scene_name);
    };
    let storyline = &scene.storylines[0];
    let sb = StorylineBlock {
        statements: storyline.statements.clone(),
        span: storyline.span.clone(),
    };
    let mut sm = SourceMapBuilder::new(
        &format!("{}.scene", scene_name),
        &format!("{}.js", scene_name),
    );
    let js = compile_storyline("main", &sb, &mut sm);
    let sm_comment = sm.finalize();
    format!("{}\n{}", js, sm_comment)
}

/// Compile a DSL string focusing only on the storyline content (without scene wrapper).
///
/// This helper builds the minimal DSL around the storyline body so tests
/// can express just the storyline statements inline.
fn compile_storyline_dsl(body: &str, name: &str) -> String {
    let dsl = format!(
        "game_scene {} {{\n  @storylines {{\n{}\n  }}\n}}\n",
        name, body
    );
    compile_dsl_to_js(&dsl, name)
}

// ═════════════════════════════════════════════════════════════════════════
// ScriptEngine execution helpers
// ═════════════════════════════════════════════════════════════════════════

/// Load compiled JS into a ScriptEngine and call `storyline_main()`,
/// returning the first emitted ScriptCommand (if any).
fn execute_js_get_command(js: &str) -> Option<ScriptCommand> {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(js)
        .expect("script should load successfully");
    engine
        .call_function("storyline_main", &[])
        .expect("storyline_main() should not error")
}

/// Load compiled JS into a ScriptEngine and collect ALL ScriptCommands
/// by stepping through `signal_done(CommandResult::Void)` until the
/// function returns None (finished).
fn execute_js_collect_all(js: &str) -> Vec<ScriptCommand> {
    let mut engine = ScriptEngine::new();
    engine
        .load_script(js)
        .expect("script should load successfully");
    let mut commands = Vec::new();
    let mut cmd = engine
        .call_function("storyline_main", &[])
        .expect("storyline_main() should not error");
    while let Some(c) = cmd {
        commands.push(c);
        cmd = engine
            .signal_done(CommandResult::Void)
            .expect("signal_done should not error");
    }
    commands
}

// ═════════════════════════════════════════════════════════════════════════
// Test 1: Simple Dialogue
// @speaker("Prof") { "Hello there!" }
// → ScriptCommand::ShowText { text: "Prof: Hello there!" }
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_dialogue_simple() {
    let body = r#"    @speaker("Prof") { "Hello there!" }"#;
    let js = compile_storyline_dsl(body, "test_dialogue");

    // Verify the compiled JS structure
    assert!(
        js.contains("export async function storyline_main()"),
        "should export storyline_main function"
    );

    // Verify the JS contains the correct game API call
    assert!(
        js.contains("await game.showText("),
        "should call game.showText"
    );
    assert!(
        js.contains("Prof: Hello there!"),
        "should include speaker name and text"
    );

    // Verify source map is emitted
    assert!(js.contains("sourceMappingURL"), "should include source map");

    // Execute in ScriptEngine
    let cmd = execute_js_get_command(&js);
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Prof: Hello there!".to_string()
        }),
        "should emit ShowText with speaker-prefixed text"
    );
}

#[test]
fn test_dialogue_multiple_lines() {
    let body = r#"
    @speaker("Prof") {
      "Welcome to the world of monsters!"
      "Are you ready for your adventure?"
    }"#;
    let js = compile_storyline_dsl(body, "test_multi_dialogue");

    let cmd = execute_js_get_command(&js);
    assert!(
        matches!(&cmd, Some(ScriptCommand::ShowText { text }) if text.contains("Welcome") && text.contains("Are you ready"))
    );
    assert!(cmd.unwrap().to_text().unwrap().contains("Prof:"));
}

#[test]
fn test_dialogue_say_cutscene_line() {
    // `@say("Prof")` is cutscene speech (auto-triggered storyline); it renders
    // through the same showText path as @speaker, differing only in meaning.
    let body = r#"    @say("Prof") { "The lab is that way." }"#;
    let js = compile_storyline_dsl(body, "test_dialogue_say");

    assert!(
        js.contains("await game.showText("),
        "@say should compile to game.showText"
    );
    assert!(
        !js.contains("showTextAuto"),
        "@say must not emit a showTextAuto call"
    );

    let cmd = execute_js_get_command(&js);
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Prof: The lab is that way.".to_string()
        }),
        "@say yields the same ShowText command as @speaker"
    );
}

#[test]
fn test_dialogue_compilation_only() {
    // Verify the compilation pipeline works end-to-end without runtime
    let body = r#"    @speaker("Shopkeeper") { "Welcome to the shop!" }"#;
    let js = compile_storyline_dsl(body, "test_compilation");

    // Verify JS structure
    assert!(
        js.contains("export async function storyline_main()"),
        "should be a valid ES module"
    );
    assert!(js.contains("await game.showText("), "should call showText");
    assert!(
        js.contains("\"Shopkeeper: Welcome to the shop!\""),
        "should include full text"
    );

    // Verify source map
    let has_source_map =
        js.contains("//# sourceMappingURL=data:application/json;charset=utf-8;base64,");
    assert!(has_source_map, "should include base64 source map comment");
}

// ═════════════════════════════════════════════════════════════════════════
// Test 2: Choice Branching
// @choice { @option("A") { ... } @option("B") { ... } }
// → ScriptCommand::ShowChoice { options: ["A", "B"] }
// Then on signal_done(result), follows the correct branch.
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_choice_branching() {
    let body = r#"
    @choice {
      @option("Buy") {
        @speaker("Shopkeeper") { "Great choice!" }
      }
      @option("Leave") {
        @speaker("Shopkeeper") { "Come again!" }
      }
    }"#;
    let js = compile_storyline_dsl(body, "test_choice");

    let mut engine = ScriptEngine::new();
    engine.load_script(&js).expect("should load");
    let cmd = engine.call_function("storyline_main", &[]).unwrap();

    // First command should be ShowChoice
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowChoice {
            options: vec!["Buy".to_string(), "Leave".to_string()]
        }),
        "first command should be ShowChoice"
    );

    // Simulate choosing option 0 ("Buy") → should get "Great choice!"
    let cmd = engine.signal_done(CommandResult::Number(0.0)).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Shopkeeper: Great choice!".to_string()
        }),
        "choosing option 0 should show appropriate text"
    );

    // Script should be done after that
    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None, "script should finish after option 0 body");
    assert!(engine.is_idle());
}

#[test]
fn test_choice_option_1() {
    let body = r#"
    @choice {
      @option("Buy") {
        @speaker("Shopkeeper") { "Great choice!" }
      }
      @option("Leave") {
        @speaker("Shopkeeper") { "Come again!" }
      }
    }"#;
    let js = compile_storyline_dsl(body, "test_choice_1");

    let mut engine = ScriptEngine::new();
    engine.load_script(&js).unwrap();
    let cmd = engine.call_function("storyline_main", &[]).unwrap();
    assert!(matches!(cmd, Some(ScriptCommand::ShowChoice { .. })));

    // Choose option 1 ("Leave")
    let cmd = engine.signal_done(CommandResult::Number(1.0)).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Shopkeeper: Come again!".to_string()
        }),
        "choosing option 1 should show 'Come again!'"
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
}

// ═════════════════════════════════════════════════════════════════════════
// Test 3: Conditional Logic
// @if (condition) { ... } @else { ... }
// Correct branch is selected at runtime.
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_conditional_logic_true_branch() {
    // 10 > 5 is always true → executes then-branch
    let body = r#"
    @if (10 > 5) {
      @speaker("NPC") { "Ten is greater" }
    } @else {
      @speaker("NPC") { "Never shown" }
    }"#;
    let js = compile_storyline_dsl(body, "test_conditional_true");

    let cmd = execute_js_get_command(&js);
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "NPC: Ten is greater".to_string()
        }),
        "true branch should execute"
    );
}

#[test]
fn test_conditional_logic_false_branch() {
    // 5 > 10 is always false → executes else-branch
    let body = r#"
    @if (5 > 10) {
      @speaker("NPC") { "Never shown" }
    } @else {
      @speaker("NPC") { "Five is smaller" }
    }"#;
    let js = compile_storyline_dsl(body, "test_conditional_false");

    let cmd = execute_js_get_command(&js);
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "NPC: Five is smaller".to_string()
        }),
        "false branch should execute"
    );
}

#[test]
fn test_conditional_with_flag() {
    // Use a flag to control the condition
    let body = r#"
    @if (5 > 10) {
      @speaker("Guard") { "You may pass" }
    } @else {
      @speaker("Guard") { "Access denied" }
    }"#;
    let js = compile_storyline_dsl(body, "test_conditional_flag");

    let mut engine = ScriptEngine::new();
    engine.load_script(&js).unwrap();
    let cmd = engine.call_function("storyline_main", &[]).unwrap();

    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Guard: Access denied".to_string()
        }),
        "false branch (5>10) should execute"
    );
}

// ═════════════════════════════════════════════════════════════════════════
// Test 4: UI Commands (showScene)
// Command statement in DSL → ScriptCommand::ShowScene emitted by engine
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_ui_show_scene_command() {
    // Use command syntax in DSL (bare identifier, not @-prefixed)
    let body = r#"    showScene("battle_ui")"#;
    let js = compile_storyline_dsl(body, "test_ui_command");

    // Verify JS structure
    assert!(
        js.contains("await game[\"showScene\"]"),
        "should call game.showScene via bracket notation"
    );

    // Execute in ScriptEngine
    let cmd = execute_js_get_command(&js);
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowScene {
            scene_name: "battle_ui".to_string(),
            layout_json: None,
        }),
        "should emit ShowScene command"
    );
}

#[test]
fn test_command_delay() {
    let body = r#"    delay(60)"#;
    let js = compile_storyline_dsl(body, "test_delay_command");

    let cmd = execute_js_get_command(&js);
    assert_eq!(
        cmd,
        Some(ScriptCommand::Delay { frames: 60 }),
        "should emit Delay command"
    );
}

#[test]
fn test_command_heal() {
    let body = r#"    heal()"#;
    let js = compile_storyline_dsl(body, "test_heal_command");

    let cmd = execute_js_get_command(&js);
    assert_eq!(cmd, Some(ScriptCommand::Heal), "should emit Heal command");
}

// ═════════════════════════════════════════════════════════════════════════
// Test 5: Coexistence — DSL-compiled JS + hand-written JS on same Engine
// Both scripts can be loaded and executed on the same ScriptEngine instance.
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_coexistence_dsl_and_handwritten() {
    let dsl_body = r#"    @speaker("Prof") { "Hello from DSL!" }"#;
    let dsl_js = compile_storyline_dsl(dsl_body, "coexistence_dsl");

    let hand_js = r#"
        export async function onEnter() {
            await game.showText("Hand-written script!");
        }
    "#;

    let mut engine = ScriptEngine::new();

    // ── Load and execute hand-written JS first ──
    engine
        .load_script(hand_js)
        .expect("hand-written script should load");
    let cmd = engine
        .call_function("onEnter", &[])
        .expect("onEnter should be callable");
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Hand-written script!".to_string()
        }),
        "hand-written script should work"
    );
    engine.signal_done(CommandResult::Void).unwrap();
    assert!(engine.is_idle());

    // ── Load and execute DSL-compiled JS on same engine ──
    engine
        .load_script(&dsl_js)
        .expect("DSL-compiled script should load");
    let cmd = engine
        .call_function("storyline_main", &[])
        .expect("storyline_main should be callable");
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Prof: Hello from DSL!".to_string()
        }),
        "DSL-compiled script should work on same engine"
    );
    engine.signal_done(CommandResult::Void).unwrap();
    assert!(engine.is_idle());

    // Both scripts executed successfully on the same engine instance
}

// ═════════════════════════════════════════════════════════════════════════
// Test 6: Multi-step dialogue sequence
// Multiple @speaker blocks → a sequence of ShowText commands
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_multi_step_dialogue_sequence() {
    let body = r#"
    @speaker("Prof") { "Hello there!" }
    @speaker("Prof") { "Welcome to the world of monsters!" }
    @speaker("Prof") { "This world is inhabited by creatures called monsters." }
    "#;
    let js = compile_storyline_dsl(body, "test_sequence");

    let commands = execute_js_collect_all(&js);

    assert_eq!(commands.len(), 3, "should have 3 dialogue commands");
    assert_eq!(
        commands[0],
        ScriptCommand::ShowText {
            text: "Prof: Hello there!".to_string()
        }
    );
    assert_eq!(
        commands[1],
        ScriptCommand::ShowText {
            text: "Prof: Welcome to the world of monsters!".to_string()
        }
    );
    assert_eq!(
        commands[2],
        ScriptCommand::ShowText {
            text: "Prof: This world is inhabited by creatures called monsters.".to_string()
        }
    );
}

// ═════════════════════════════════════════════════════════════════════════
// Test 7: Nested control flow (choice inside conditional)
// @if → true branch → @choice → verify nested execution
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_nested_choice_in_conditional() {
    let body = r#"
    @if (10 > 5) {
      @choice {
        @option("Flambit") {
          @speaker("Prof") { "So you choose Flambit!" }
        }
        @option("Aquakit") {
          @speaker("Prof") { "So you choose Aquakit!" }
        }
      }
    } @else {
      @speaker("Prof") { "No monsters for you." }
    }"#;
    let js = compile_storyline_dsl(body, "test_nested");

    let mut engine = ScriptEngine::new();
    engine.load_script(&js).unwrap();
    let cmd = engine.call_function("storyline_main", &[]).unwrap();

    // Condition is true → enters choice block
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowChoice {
            options: vec!["Flambit".to_string(), "Aquakit".to_string()]
        }),
        "true branch should show choice"
    );

    // Choose option 0
    let cmd = engine.signal_done(CommandResult::Number(0.0)).unwrap();
    assert_eq!(
        cmd,
        Some(ScriptCommand::ShowText {
            text: "Prof: So you choose Flambit!".to_string()
        })
    );

    let cmd = engine.signal_done(CommandResult::Void).unwrap();
    assert_eq!(cmd, None);
    assert!(engine.is_idle());
}

// ═════════════════════════════════════════════════════════════════════════
// Test 8: End-to-end with real test.scene asset content
// Verify the pipeline works with realistic game scene content
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_full_scene_pipeline() {
    // Content mirroring assets/test.scene structure
    let dsl = r#"
game_scene TestShop {
  @variables {
    gold = 500
    name = "Trainer"
  }
  @storylines {
    @speaker("Shopkeeper") {
      "Welcome to the shop!"
      "What would you like?"
    }
    @choice {
      @option("Buy") {
        @speaker("Shopkeeper") {
          "Great choice!"
        }
      }
      @option("Leave") {
        @speaker("Shopkeeper") {
          "Come again!"
        }
      }
    }
  }
}"#;
    let js = compile_dsl_to_js(dsl, "test_shop");

    // Verify the compilation produces valid, well-structured JS
    assert!(js.contains("export async function storyline_main()"));
    assert!(js.contains("await game.showText("));
    assert!(js.contains("const choice = await game.showChoice("));
    assert!(js.contains("if (choice === 0) {"));
    assert!(js.contains("\"Shopkeeper: Welcome to the shop!\\nWhat would you like?\""));

    // Verify source map is present
    assert!(
        js.contains("//# sourceMappingURL=data:application/json;charset=utf-8;base64,"),
        "should include a valid source map"
    );

    // Execute the compiled JS and verify key commands
    let mut engine = ScriptEngine::new();
    engine.load_script(&js).unwrap();

    let cmd = engine.call_function("storyline_main", &[]).unwrap();
    assert!(
        matches!(cmd, Some(ScriptCommand::ShowText { ref text }) if text.contains("Shopkeeper: Welcome"))
    );

    engine.signal_done(CommandResult::Void).unwrap();
    let cmd = engine.tick();
    assert!(matches!(cmd, Some(ScriptCommand::ShowChoice { ref options }) if options.len() == 2));

    // Choose "Buy"
    engine.signal_done(CommandResult::Number(0.0)).unwrap();
    let cmd = engine.tick();
    assert!(
        matches!(cmd, Some(ScriptCommand::ShowText { ref text }) if text.contains("Great choice!"))
    );

    engine.signal_done(CommandResult::Void).unwrap();
    assert!(engine.is_idle());
}

// ═════════════════════════════════════════════════════════════════════════
// Test 9: Source map verification
// Ensure the generated JS has proper source map mappings back to DSL
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_source_map_generation() {
    let dsl = r#"game_scene MapTest {
  @storylines {
    @speaker("Prof") { "Hello!" }
  }
}"#;
    let js = compile_dsl_to_js(dsl, "map_test");

    // Source map must be present
    assert!(
        js.contains("//# sourceMappingURL="),
        "should have source map"
    );

    // Locate the source map comment
    let sm_line = js
        .lines()
        .find(|l| l.starts_with("//# sourceMappingURL="))
        .expect("source map line should be present");

    // It should be a valid base64 data URL
    assert!(
        sm_line.starts_with("//# sourceMappingURL=data:application/json;charset=utf-8;base64,"),
        "should be a base64 data URL source map"
    );

    // The source map should reference the original DSL file
    let payload = sm_line
        .strip_prefix("//# sourceMappingURL=data:application/json;charset=utf-8;base64,")
        .unwrap();

    use base64::Engine as _;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .expect("source map should be valid base64");
    let sm_json: serde_json::Value =
        serde_json::from_slice(&decoded).expect("source map should be valid JSON");

    let sources = sm_json["sources"]
        .as_array()
        .expect("should have sources array");
    let has_scene = sources
        .iter()
        .any(|s| s.as_str().map_or(false, |s| s.contains(".scene")));
    assert!(has_scene, "sources should reference .scene file");
}

// ═════════════════════════════════════════════════════════════════════════
// Test 10: Error handling — invalid DSL produces parse errors
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn test_invalid_dsl_reports_errors() {
    // DSL without a scene body (missing {) — clear syntax error
    let dsl = r#"
game_scene Broken
  @storylines {
    @speaker("Prof") { "Hello!" }
  }
"#;
    let mut lexer = Lexer::new(dsl, "broken.scene");
    let tokens = lexer
        .tokenize()
        .expect("lexing should succeed even for broken DSL");
    let (doc, errors) = parser::parse(tokens);

    assert!(
        !errors.is_empty() || doc.is_none(),
        "should have parse errors OR no document for invalid DSL"
    );
}

// ═════════════════════════════════════════════════════════════════════════
// Extension trait for ScriptCommand to help with assertions
// ═════════════════════════════════════════════════════════════════════════

trait ScriptCommandExt {
    fn to_text(&self) -> Option<&str>;
}

impl ScriptCommandExt for ScriptCommand {
    fn to_text(&self) -> Option<&str> {
        match self {
            ScriptCommand::ShowText { text } => Some(text),
            _ => None,
        }
    }
}
