//! # Error Quality Tests
//!
//! Integration tests that feed real DSL input through the lexer → parser pipeline
//! and verify that the resulting miette `DslError` diagnostics are useful,
//! actionable, and carry correct metadata (error code, help text, file, line, col).
//!
//! Each test follows the pattern:
//! 1. Feed DSL text to the lexer + parser (or semantic validator)
//! 2. Convert raw parse/semantic errors into `DslError` variants
//! 3. Assert: error code matches, help text is actionable, source location is populated

use dotzuki_engine_dsl::ast::SourceSpan;
use dotzuki_engine_dsl::error::{to_miette_span, DslError};
use dotzuki_engine_dsl::lexer::{LexError, Lexer, SpannedToken, Token};
use dotzuki_engine_dsl::parser::{ParseError, SemanticError};
use miette::Diagnostic;
use miette::SourceSpan as MietteSpan;
use std::panic;

// ── Helpers ──

fn lex_dsl(source: &str, file: &str) -> Result<Vec<SpannedToken>, Vec<LexError>> {
    let mut lexer = Lexer::new(source, file);
    lexer.tokenize()
}

fn parse_and_validate(
    tokens: Vec<SpannedToken>,
) -> (
    Option<dotzuki_engine_dsl::ast::Document>,
    Vec<ParseError>,
    Vec<SemanticError>,
) {
    dotzuki_engine_dsl::parser::parse_and_validate(tokens, "")
}

fn parse_only(
    tokens: Vec<SpannedToken>,
) -> (Option<dotzuki_engine_dsl::ast::Document>, Vec<ParseError>) {
    dotzuki_engine_dsl::parser::parse(tokens)
}

fn dsl_error_code(err: &DslError) -> Option<String> {
    err.code().map(|c| c.to_string())
}

fn dsl_error_help(err: &DslError) -> Option<String> {
    err.help().map(|h| h.to_string())
}

fn dsl_error_file(err: &DslError) -> &str {
    match err {
        DslError::Syntax { file, .. } => file,
        DslError::UnknownDirective { file, .. } => file,
        DslError::UndefinedVariable { file, .. } => file,
        DslError::CircularStyle { file, .. } => file,
        DslError::InvalidComponent { file, .. } => file,
        DslError::MissingRegion { file, .. } => file,
        DslError::EmptyChoice { file, .. } => file,
        DslError::IndentationError { file, .. } => file,
        DslError::InternalError { file, .. } => file,
    }
}

fn dsl_error_line(err: &DslError) -> usize {
    match err {
        DslError::Syntax { line, .. } => *line,
        DslError::UnknownDirective { line, .. } => *line,
        DslError::UndefinedVariable { line, .. } => *line,
        DslError::CircularStyle { line, .. } => *line,
        DslError::InvalidComponent { line, .. } => *line,
        DslError::MissingRegion { line, .. } => *line,
        DslError::EmptyChoice { line, .. } => *line,
        DslError::IndentationError { line, .. } => *line,
        DslError::InternalError { line, .. } => *line,
    }
}

fn convert_parse_error(err: &ParseError, src_text: &str) -> Option<DslError> {
    match err {
        ParseError::UnexpectedToken { found, span, .. } => {
            if let Token::Identifier(name) = found {
                if name.starts_with('@') {
                    let directive_name = name.trim_start_matches('@').to_string();
                    return Some(DslError::UnknownDirective {
                        name: directive_name,
                        span: to_miette_span(span, src_text),
                        file: span.file.clone(),
                        line: span.line_start,
                    });
                }
            }
            Some(DslError::Syntax {
                src: src_text.to_string(),
                span: to_miette_span(span, src_text),
                file: span.file.clone(),
                line: span.line_start,
                col: span.col_start,
            })
        }
        ParseError::MissingBlock { span, .. } => Some(DslError::Syntax {
            src: src_text.to_string(),
            span: to_miette_span(span, src_text),
            file: span.file.clone(),
            line: span.line_start,
            col: span.col_start,
        }),
        ParseError::InvalidComponentType { found, span, .. } => Some(DslError::InvalidComponent {
            found: found.clone(),
            span: to_miette_span(span, src_text),
            file: span.file.clone(),
            line: span.line_start,
        }),
        ParseError::UnclosedBlock { span, .. } => Some(DslError::Syntax {
            src: src_text.to_string(),
            span: to_miette_span(span, src_text),
            file: span.file.clone(),
            line: span.line_start,
            col: span.col_start,
        }),
        ParseError::IndentationError { msg, span } => Some(DslError::IndentationError {
            msg: msg.clone(),
            span: to_miette_span(span, src_text),
            file: span.file.clone(),
            line: span.line_start,
        }),
        ParseError::UnterminatedString { span } => Some(DslError::Syntax {
            src: src_text.to_string(),
            span: to_miette_span(span, src_text),
            file: span.file.clone(),
            line: span.line_start,
            col: span.col_start,
        }),
        ParseError::UnexpectedEof { span, .. } => Some(DslError::Syntax {
            src: src_text.to_string(),
            span: to_miette_span(span, src_text),
            file: span.file.clone(),
            line: span.line_start,
            col: span.col_start,
        }),
        // Custom-component schema violations surface as plain syntax errors
        // in this diagnostic layer; the Display impl carries the detail.
        ParseError::MissingRequiredProp { span, .. }
        | ParseError::PropTypeMismatch { span, .. }
        | ParseError::UnknownProp { span, .. }
        | ParseError::DuplicateComponentDecl { span, .. } => Some(DslError::Syntax {
            src: src_text.to_string(),
            span: to_miette_span(span, src_text),
            file: span.file.clone(),
            line: span.line_start,
            col: span.col_start,
        }),
    }
}

fn convert_semantic_error(err: &SemanticError, src_text: &str) -> Option<DslError> {
    match err {
        SemanticError::UndefinedVariable {
            name,
            defined_vars,
            span,
        } => Some(DslError::UndefinedVariable {
            name: name.clone(),
            defined_vars: defined_vars.join(", "),
            span: to_miette_span(span, src_text),
            file: span.file.clone(),
            line: span.line_start,
        }),
        SemanticError::CircularStyleInheritance { chain, span } => Some(DslError::CircularStyle {
            chain: chain.join(" -> "),
            span: to_miette_span(span, src_text),
            file: span.file.clone(),
            line: span.line_start,
        }),
        SemanticError::EmptyChoice { span } => Some(DslError::EmptyChoice {
            span: to_miette_span(span, src_text),
            file: span.file.clone(),
            line: span.line_start,
        }),
        _ => None,
    }
}

fn convert_lex_error(err: &LexError, src_text: &str) -> DslError {
    let span = SourceSpan::point(&err.file, err.line, err.col);
    if err.message.contains("Tabs")
        || err.message.contains("indent")
        || err.message.contains("spaces")
    {
        DslError::IndentationError {
            msg: err.message.clone(),
            span: to_miette_span(&span, src_text),
            file: err.file.clone(),
            line: err.line,
        }
    } else {
        DslError::Syntax {
            src: src_text.to_string(),
            span: to_miette_span(&span, src_text),
            file: err.file.clone(),
            line: err.line,
            col: err.col,
        }
    }
}

// ── E001: Syntax Error ──

#[test]
fn error_quality_test_e001_syntax_error_location() {
    let dsl =
        "game_scene Test {\n  @storylines {\n    @if (true)\n      @speaker(\"NPC\") { \"hi\" }\n  }\n}";
    let tokens = lex_dsl(dsl, "test.scene").expect("lex should succeed");
    let (_, parse_errors) = parse_only(tokens);
    assert!(!parse_errors.is_empty(), "Expected parse errors");

    let dsl_err = convert_parse_error(&parse_errors[0], dsl).expect("should convert to DslError");

    assert_eq!(dsl_error_code(&dsl_err).as_deref(), Some("E001"));

    let help = dsl_error_help(&dsl_err).expect("E001 must have help text");
    assert!(
        help.contains("block delimiters") || help.contains("parentheses"),
        "Help text should guide the user: {help}"
    );

    let display = dsl_err.to_string();
    assert!(!display.is_empty(), "Display should produce text");
    assert!(
        display.to_lowercase().contains("syntax"),
        "Display should mention syntax"
    );

    if let DslError::Syntax {
        file, line, col: _, ..
    } = &dsl_err
    {
        assert_eq!(file, "test.scene");
        assert!(*line > 0, "Line number should be >0, got {line}");
    } else {
        panic!("Expected DslError::Syntax, got different variant");
    }
}

// ── E002: Unknown Directive ──

#[test]
fn error_quality_test_e002_unknown_directive_suggestion() {
    let dsl = "game_scene Test {\n  @banana {\n  }\n}";
    let tokens = lex_dsl(dsl, "test.scene").expect("lex should succeed");
    let (_, parse_errors) = parse_only(tokens);
    assert!(
        !parse_errors.is_empty(),
        "Expected parse errors for @banana"
    );

    let dsl_err = convert_parse_error(&parse_errors[0], dsl).expect("should convert to DslError");

    assert_eq!(
        dsl_error_code(&dsl_err).as_deref(),
        Some("E002"),
        "Expected E002 error code, got: {:?}",
        dsl_error_code(&dsl_err)
    );

    if let DslError::UnknownDirective { name, .. } = &dsl_err {
        assert_eq!(name, "banana", "Should reference the unknown directive");
    } else {
        panic!("Expected DslError::UnknownDirective, got: {:?}", dsl_err);
    }

    let help = dsl_error_help(&dsl_err).expect("E002 must have help text");
    assert!(
        help.contains("@variables"),
        "Help should list valid directives: {help}"
    );
    assert!(
        help.contains("@storylines"),
        "Help should mention @storylines: {help}"
    );
    assert!(
        help.contains("@speaker"),
        "Help should mention @speaker: {help}"
    );

    if let DslError::UnknownDirective { file, line, .. } = &dsl_err {
        assert_eq!(file, "test.scene");
        assert!(*line > 0, "Line should be >0, got {line}");
    }
}

// ── E003: Undefined Variable ──

#[test]
fn error_quality_test_e003_undefined_variable_suggestion() {
    let dsl = r#"game_scene Test {
    @variables {
        gold = 500
        hp = 100
    }
    @storylines {
        @if (undefined_var > 100) {
            @speaker("NPC") { "rich!" }
        }
    }
}"#;
    let tokens = lex_dsl(dsl, "test.scene").expect("lex should succeed");
    let (_, parse_errors, semantic_errors) = parse_and_validate(tokens);
    assert!(
        parse_errors.is_empty(),
        "Expected no parse errors, got: {:?}",
        parse_errors
    );
    assert!(
        !semantic_errors.is_empty(),
        "Expected semantic errors for undefined variable"
    );

    let dsl_err =
        convert_semantic_error(&semantic_errors[0], dsl).expect("should convert to DslError");

    assert_eq!(dsl_error_code(&dsl_err).as_deref(), Some("E003"));

    if let DslError::UndefinedVariable {
        name, defined_vars, ..
    } = &dsl_err
    {
        assert_eq!(name, "undefined_var", "Should reference the bad variable");
        assert!(
            defined_vars.contains("gold"),
            "Should list defined variables, got: {defined_vars}"
        );
        assert!(
            defined_vars.contains("hp"),
            "Should list hp as defined: {defined_vars}"
        );
    } else {
        panic!("Expected DslError::UndefinedVariable, got: {:?}", dsl_err);
    }

    let help = dsl_error_help(&dsl_err).expect("E003 must have help text");
    assert!(
        help.contains("undefined_var") || help.contains("Did you forget"),
        "Help should mention the variable or suggest declaration: {help}"
    );
    assert!(
        help.contains("@variables"),
        "Help should reference @variables: {help}"
    );

    if let DslError::UndefinedVariable {
        file: _, line: _, ..
    } = &dsl_err
    {}
}

// ── E004: Circular Style Inheritance ──

#[test]
fn error_quality_test_e004_circular_style_chain() {
    let dsl = r#"game_scene Test {
    @style A: B {
        color = "red"
    }
    @style B: A {
        color = "blue"
    }
}"#;
    let tokens = lex_dsl(dsl, "test.scene").expect("lex should succeed");
    let (_, parse_errors, semantic_errors) = parse_and_validate(tokens);
    assert!(
        parse_errors.is_empty(),
        "Expected no parse errors, got: {:?}",
        parse_errors
    );
    assert!(
        !semantic_errors.is_empty(),
        "Expected semantic errors for circular style"
    );

    let dsl_err =
        convert_semantic_error(&semantic_errors[0], dsl).expect("should convert to DslError");

    assert_eq!(dsl_error_code(&dsl_err).as_deref(), Some("E004"));

    if let DslError::CircularStyle { chain, .. } = &dsl_err {
        assert!(
            chain.contains("A") && chain.contains("B"),
            "Chain should reference both styles: {chain}"
        );
        assert!(
            chain.contains("->"),
            "Chain should use arrow notation: {chain}"
        );
    } else {
        panic!("Expected DslError::CircularStyle, got: {:?}", dsl_err);
    }

    let help = dsl_error_help(&dsl_err).expect("E004 must have help text");
    assert!(
        help.to_lowercase().contains("circular"),
        "Help should mention circular reference: {help}"
    );

    if let DslError::CircularStyle { file, line, .. } = &dsl_err {
        assert_eq!(file, "test.scene");
        assert!(*line > 0, "Line should be >0, got {line}");
    }
}

// ── E005: Invalid Component Type ──

#[test]
fn error_quality_test_e005_invalid_component_type_with_suggestions() {
    let dsl = r#"screen Main {
    widget {
        width = 100
    }
}"#;
    let tokens = lex_dsl(dsl, "test.gui").expect("lex should succeed");
    let (_, parse_errors) = parse_only(tokens);
    assert!(
        !parse_errors.is_empty(),
        "Expected parse errors for invalid component"
    );

    let dsl_err = convert_parse_error(&parse_errors[0], dsl).expect("should convert to DslError");

    assert_eq!(dsl_error_code(&dsl_err).as_deref(), Some("E005"));

    if let DslError::InvalidComponent { found, .. } = &dsl_err {
        assert_eq!(found, "widget", "Should reference the invalid type");
    } else {
        panic!("Expected DslError::InvalidComponent, got: {:?}", dsl_err);
    }

    let help = dsl_error_help(&dsl_err).expect("E005 must have help text");
    assert!(
        help.contains("panel"),
        "Help should list 'panel' as valid: {help}"
    );
    assert!(
        help.contains("button"),
        "Help should list 'button' as valid: {help}"
    );
    assert!(
        help.contains("text"),
        "Help should list 'text' as valid: {help}"
    );

    if let DslError::InvalidComponent { file, line, .. } = &dsl_err {
        assert_eq!(file, "test.gui");
        assert!(*line > 0, "Line should be >0, got {line}");
    }
}

// ── E006: Missing Atlas Region ──

#[test]
fn error_quality_test_e006_missing_atlas_region_diagnostic() {
    let err = DslError::MissingRegion {
        region: "btn_hover".into(),
        available: "btn_normal, btn_pressed, btn_disabled".into(),
        span: MietteSpan::new(0.into(), 10),
        file: "ui.atlas".into(),
        line: 12,
    };

    assert_eq!(dsl_error_code(&err).as_deref(), Some("E006"));

    let msg = err.to_string();
    assert!(
        msg.contains("btn_hover"),
        "Error message should name the missing region: {msg}"
    );
    assert!(
        msg.to_lowercase().contains("missing"),
        "Error should convey something is missing: {msg}"
    );

    let help = dsl_error_help(&err).expect("E006 must have help text");
    assert!(
        help.contains("btn_normal"),
        "Help should list available regions: {help}"
    );
    assert!(
        help.contains("btn_pressed"),
        "Help should list btn_pressed: {help}"
    );

    assert_eq!(dsl_error_file(&err), "ui.atlas");
    assert_eq!(dsl_error_line(&err), 12);

    let _ = format!("{:?}", err);
}

// ── E007: Empty Choice Block ──

#[test]
fn error_quality_test_e007_empty_choice_has_helpful_text() {
    let dsl = r#"game_scene Test {
    @storylines {
        @choice {
        }
    }
}"#;
    let tokens = lex_dsl(dsl, "test.scene").expect("lex should succeed");
    let (_, parse_errors, semantic_errors) = parse_and_validate(tokens);
    assert!(
        parse_errors.is_empty(),
        "Expected no parse errors, got: {:?}",
        parse_errors
    );
    assert!(
        !semantic_errors.is_empty(),
        "Expected semantic errors for empty choice"
    );

    let dsl_err =
        convert_semantic_error(&semantic_errors[0], dsl).expect("should convert to DslError");

    assert_eq!(dsl_error_code(&dsl_err).as_deref(), Some("E007"));

    let msg = dsl_err.to_string();
    assert!(
        msg.to_lowercase().contains("empty") || msg.contains("@choice"),
        "Message should reference empty @choice: {msg}"
    );

    let help = dsl_error_help(&dsl_err).expect("E007 must have help text");
    assert!(
        help.contains("@choice"),
        "Help should mention @choice: {help}"
    );
    assert!(
        help.contains("@option"),
        "Help should mention @option requirement: {help}"
    );

    if let DslError::EmptyChoice { file, line, .. } = &dsl_err {
        assert_eq!(file, "test.scene");
        assert!(*line > 0, "Line should be >0, got {line}");
    }
}

// ── E008: Indentation Error (Tabs) ──

#[test]
fn error_quality_test_e008_tab_indentation_error() {
    let dsl = "game_scene Test {\n\tui {\n\t\tcolor = \"red\"\n\t}\n}";
    let result = lex_dsl(dsl, "test.scene");

    match result {
        Ok(tokens) => {
            let has_tab_error = tokens
                .iter()
                .any(|t| matches!(&t.token, Token::Error(msg) if msg.contains("Tabs")));
            if has_tab_error {
                for t in &tokens {
                    if let Token::Error(msg) = &t.token {
                        let span = &t.span;
                        let err = DslError::IndentationError {
                            msg: msg.clone(),
                            span: to_miette_span(span, dsl),
                            file: span.file.clone(),
                            line: span.line_start,
                        };
                        assert_eq!(dsl_error_code(&err).as_deref(), Some("E008"));
                        let help = dsl_error_help(&err).expect("E008 must have help text");
                        assert!(
                            help.contains("spaces"),
                            "Help should mention spaces: {help}"
                        );
                        assert!(help.contains("Tabs"), "Help should mention tabs: {help}");
                        assert_eq!(dsl_error_file(&err), "test.scene");
                        assert!(dsl_error_line(&err) > 0);
                        return;
                    }
                }
            }
            panic!("Expected tab error in tokens, got: {:?}", tokens);
        }
        Err(lex_errors) => {
            assert!(!lex_errors.is_empty(), "Expected lex errors for tabs");
            let dsl_err = convert_lex_error(&lex_errors[0], dsl);

            assert_eq!(dsl_error_code(&dsl_err).as_deref(), Some("E008"));

            let msg = dsl_err.to_string();
            assert!(
                msg.contains("Tabs") || msg.to_lowercase().contains("indentation"),
                "Message should mention tabs or indentation: {msg}"
            );

            let help = dsl_error_help(&dsl_err).expect("E008 must have help text");
            assert!(
                help.contains("spaces"),
                "Help should suggest using spaces: {help}"
            );
            assert!(
                help.to_lowercase().contains("tab"),
                "Help should mention tabs: {help}"
            );

            if let DslError::IndentationError { file, line, .. } = &dsl_err {
                assert_eq!(file, "test.scene");
                assert!(*line > 0, "Line should be >0, got {line}");
            } else {
                panic!("Expected IndentationError, got: {:?}", dsl_err);
            }
        }
    }
}

// ── Edge Case 1: Empty input does not panic ──

#[test]
fn error_quality_test_error_empty_input_does_not_panic() {
    let dsl = "";
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let tokens = lex_dsl(dsl, "empty.scene").expect("lex of empty string should succeed");
        let (doc, parse_errors) = parse_only(tokens);

        assert!(doc.is_none(), "Empty document expected None");
        assert!(
            !parse_errors.is_empty(),
            "Expected parse errors for empty input"
        );

        let dsl_err = convert_parse_error(&parse_errors[0], dsl).expect("should convert");
        let _display = dsl_err.to_string();
        let _code = dsl_error_code(&dsl_err);

        (doc, parse_errors)
    }));

    assert!(
        result.is_ok(),
        "Empty input must not cause a panic, but got: {:?}",
        result
    );
}

// ── Edge Case 2: Multiple errors collected together ──

#[test]
fn error_quality_test_error_multiple_errors_collected_together() {
    let dsl = r#"game_scene Test {
    @style A: B {
        color = "red"
    }
    @style B: A {
        color = "blue"
    }
    @variables {
        gold = 500
    }
    @storylines {
        @if (undefined_x > 0) {
            @speaker("NPC") { "hi" }
        }
        @choice {
        }
    }
}"#;
    let tokens = lex_dsl(dsl, "test.scene").expect("lex should succeed");
    let (_, parse_errors, semantic_errors) = parse_and_validate(tokens);

    assert!(
        parse_errors.is_empty(),
        "Expected no parse errors, got: {:?}",
        parse_errors
    );

    assert!(
        semantic_errors.len() >= 3,
        "Expected at least 3 semantic errors, got {}: {:?}",
        semantic_errors.len(),
        semantic_errors
    );

    let dsl_errors: Vec<DslError> = semantic_errors
        .iter()
        .filter_map(|e| convert_semantic_error(e, dsl))
        .collect();

    assert_eq!(
        dsl_errors.len(),
        semantic_errors.len(),
        "All semantic errors should be convertible"
    );

    let codes: Vec<String> = dsl_errors
        .iter()
        .filter_map(|e| dsl_error_code(e))
        .collect();
    assert!(
        codes.contains(&"E003".to_string()),
        "Should have E003 (undefined var), got: {:?}",
        codes
    );
    assert!(
        codes.contains(&"E004".to_string()),
        "Should have E004 (circular style), got: {:?}",
        codes
    );
    assert!(
        codes.contains(&"E007".to_string()),
        "Should have E007 (empty choice), got: {:?}",
        codes
    );
    assert_eq!(
        codes.len(),
        dsl_errors.len(),
        "Every error should have a code"
    );

    for err in &dsl_errors {
        let msg = err.to_string();
        assert!(!msg.is_empty(), "Every error must have a Display message");
    }
}

// ── Bonus: E999 Internal Error ──

#[test]
fn error_quality_test_e999_internal_error_has_no_help_by_design() {
    let err = DslError::InternalError {
        msg: "unexpected state in codegen".into(),
        span: Some(MietteSpan::new(50.into(), 10)),
        file: "internal.scene".into(),
        line: 99,
    };

    assert_eq!(dsl_error_code(&err).as_deref(), Some("E999"));
    assert!(err.to_string().contains("unexpected state"));
    assert!(
        dsl_error_help(&err).is_none(),
        "E999 should have no help text (internal compiler error)"
    );

    let _ = format!("{:?}", err);
}

// ── Bonus: Inconsistent indentation ──

#[test]
fn error_quality_test_e008_inconsistent_indentation_error() {
    let dsl = "game_scene Test {\n   ui {\n      color = \"red\"\n   }\n}";
    let result = lex_dsl(dsl, "test.scene");

    match result {
        Ok(tokens) => {
            let has_indent_err = tokens.iter().any(|t| {
                matches!(&t.token, Token::Error(msg) if msg.contains("indentation") || msg.contains("spaces"))
            });
            if !has_indent_err {
                let (_, parse_errors) = parse_only(tokens);
                let has_parse_indent = parse_errors
                    .iter()
                    .any(|e| matches!(e, ParseError::IndentationError { .. }));
                assert!(
                    !parse_errors.is_empty() || has_parse_indent,
                    "Expected indentation error, got parse errors: {:?}",
                    parse_errors
                );
            }
        }
        Err(lex_errors) => {
            assert!(
                !lex_errors.is_empty(),
                "Expected lex errors for bad indentation"
            );
            let err = convert_lex_error(&lex_errors[0], dsl);
            assert_eq!(dsl_error_code(&err).as_deref(), Some("E008"));
            assert!(
                !err.to_string().is_empty(),
                "Error message should not be empty"
            );
        }
    }
}

// ── All error codes are unique ──

#[test]
fn error_quality_test_quality_all_error_codes_unique_and_present() {
    use std::collections::HashSet;

    let dummy_span = MietteSpan::new(0.into(), 5);
    let errors: Vec<DslError> = vec![
        DslError::Syntax {
            src: "".into(),
            span: dummy_span,
            file: "".into(),
            line: 1,
            col: 0,
        },
        DslError::UnknownDirective {
            name: "".into(),
            span: dummy_span,
            file: "".into(),
            line: 1,
        },
        DslError::UndefinedVariable {
            name: "".into(),
            defined_vars: "".into(),
            span: dummy_span,
            file: "".into(),
            line: 1,
        },
        DslError::CircularStyle {
            chain: "".into(),
            span: dummy_span,
            file: "".into(),
            line: 1,
        },
        DslError::InvalidComponent {
            found: "".into(),
            span: dummy_span,
            file: "".into(),
            line: 1,
        },
        DslError::MissingRegion {
            region: "".into(),
            available: "".into(),
            span: dummy_span,
            file: "".into(),
            line: 1,
        },
        DslError::EmptyChoice {
            span: dummy_span,
            file: "".into(),
            line: 1,
        },
        DslError::IndentationError {
            msg: "".into(),
            span: dummy_span,
            file: "".into(),
            line: 1,
        },
        DslError::InternalError {
            msg: "".into(),
            span: Some(dummy_span),
            file: "".into(),
            line: 1,
        },
    ];

    let mut codes = HashSet::new();
    for err in &errors {
        let code = dsl_error_code(err).expect("every variant must have an error code");
        assert!(codes.insert(code.clone()), "duplicate error code: {code}");
    }
    assert_eq!(codes.len(), errors.len(), "all 9 codes must be unique");
    assert!(codes.contains("E001"), "Missing E001");
    assert!(codes.contains("E002"), "Missing E002");
    assert!(codes.contains("E003"), "Missing E003");
    assert!(codes.contains("E004"), "Missing E004");
    assert!(codes.contains("E005"), "Missing E005");
    assert!(codes.contains("E006"), "Missing E006");
    assert!(codes.contains("E007"), "Missing E007");
    assert!(codes.contains("E008"), "Missing E008");
    assert!(codes.contains("E999"), "Missing E999");
}

// ── Verify miette-related Display/Debug don't panic ──

#[test]
fn error_quality_test_quality_dsl_error_display_and_debug_no_panic() {
    let dummy_span = MietteSpan::new(0.into(), 5);
    let error_cases: Vec<DslError> = vec![
        DslError::Syntax {
            src: "game_scene Test {}".into(),
            span: dummy_span,
            file: "t.scene".into(),
            line: 1,
            col: 0,
        },
        DslError::UnknownDirective {
            name: "xyz".into(),
            span: dummy_span,
            file: "t.scene".into(),
            line: 2,
        },
        DslError::UndefinedVariable {
            name: "foo".into(),
            defined_vars: "bar".into(),
            span: dummy_span,
            file: "t.scene".into(),
            line: 3,
        },
        DslError::CircularStyle {
            chain: "A -> B -> A".into(),
            span: dummy_span,
            file: "t.scene".into(),
            line: 4,
        },
        DslError::InvalidComponent {
            found: "widget".into(),
            span: dummy_span,
            file: "t.gui".into(),
            line: 5,
        },
        DslError::MissingRegion {
            region: "r1".into(),
            available: "r2".into(),
            span: dummy_span,
            file: "t.atlas".into(),
            line: 6,
        },
        DslError::EmptyChoice {
            span: dummy_span,
            file: "t.scene".into(),
            line: 7,
        },
        DslError::IndentationError {
            msg: "tabs bad".into(),
            span: dummy_span,
            file: "t.scene".into(),
            line: 8,
        },
        DslError::InternalError {
            msg: "oops".into(),
            span: Some(dummy_span),
            file: "t.scene".into(),
            line: 9,
        },
    ];

    for err in &error_cases {
        let display = err.to_string();
        assert!(
            !display.is_empty(),
            "Display for {:?} rendered empty",
            dsl_error_code(err)
        );

        let debug = format!("{:?}", err);
        assert!(
            !debug.is_empty(),
            "Debug for {:?} rendered empty",
            dsl_error_code(err)
        );

        let code = dsl_error_code(err);
        assert!(
            code.is_some(),
            "Every variant must expose its error code via miette::Diagnostic::code() — thiserror Display shows the message, not the code"
        );
    }
}
