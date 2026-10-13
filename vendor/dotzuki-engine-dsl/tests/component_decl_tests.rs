//! Tests for `component` declarations — the build-time schema for
//! game-registered custom elements.
//!
//! Pipeline under test: DSL string → Lexer → Parser (decl registration +
//! use-site validation) → json_ui codegen (`custom:<name>` emission).

use dotzuki_engine_dsl::ast::{Document, PropKind};
use dotzuki_engine_dsl::codegen::json_ui;
use dotzuki_engine_dsl::lexer::Lexer;
use dotzuki_engine_dsl::parser::{self, ParseError, Parser};

fn lex(src: &str) -> Vec<dotzuki_engine_dsl::lexer::SpannedToken> {
    Lexer::new(src, "test.gui")
        .tokenize()
        .expect("lexer should accept source")
}

fn parse_ok(src: &str) -> Document {
    let (doc, errors) = Parser::new(lex(src), src).parse();
    assert!(errors.is_empty(), "unexpected parse errors: {errors:?}");
    doc.expect("document")
}

fn parse_err(src: &str) -> Vec<ParseError> {
    let (_, errors) = Parser::new(lex(src), src).parse();
    assert!(!errors.is_empty(), "expected parse errors, got none");
    errors
}

const GAUGE_DECL: &str = r#"
component gauge {
  current: expr required
  max: expr required
  label: string
  segments: int
  fill: color
}
"#;

// ── declarations ──────────────────────────────────────────────────────────

#[test]
fn declarations_only_file_parses_to_components_document() {
    let Document::Components(decls) = parse_ok(GAUGE_DECL) else {
        panic!("expected Document::Components");
    };
    assert_eq!(decls.len(), 1);
    let d = &decls[0];
    assert_eq!(d.name, "gauge");
    assert_eq!(d.props.len(), 5);
    assert_eq!(d.props[0].name, "current");
    assert_eq!(d.props[0].kind, PropKind::Expr);
    assert!(d.props[0].required);
    assert_eq!(d.props[2].name, "label");
    assert_eq!(d.props[2].kind, PropKind::String);
    assert!(!d.props[2].required);
    assert_eq!(d.props[4].kind, PropKind::Color);
}

#[test]
fn duplicate_declaration_is_an_error() {
    let src = "component a {\n}\ncomponent a {\n}\n";
    let errors = parse_err(src);
    assert!(
        matches!(errors[0], ParseError::DuplicateComponentDecl { ref name, .. } if name == "a")
    );
}

// ── use-site validation ───────────────────────────────────────────────────

fn screen_with(decl: &str, body: &str) -> String {
    format!("{decl}\nscreen test {{\n{body}\n}}\n")
}

#[test]
fn declared_component_compiles_to_custom_type() {
    let src = screen_with(
        GAUGE_DECL,
        r#"  gauge {
    rect = {tx: 1, ty: 2, tw: 6, th: 1}
    current = "{hp}"
    max = "{max_hp}"
  }"#,
    );
    let Document::Screen(screen) = parse_ok(&src) else {
        panic!("expected screen")
    };
    let json = json_ui::compile_screen(&screen).expect("codegen");
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let el = &v["elements"][0];
    assert_eq!(el["type"], "custom:gauge");
    assert_eq!(el["current"], "{hp}");
    assert_eq!(el["max"], "{max_hp}");
    assert_eq!(el["rect"]["tw"], 6);
}

#[test]
fn missing_required_prop_is_an_error() {
    let src = screen_with(
        GAUGE_DECL,
        r#"  gauge {
    current = "{hp}"
  }"#,
    );
    let errors = parse_err(&src);
    assert!(
        matches!(errors[0], ParseError::MissingRequiredProp { ref prop, .. } if prop == "max"),
        "got: {errors:?}"
    );
}

#[test]
fn undeclared_prop_is_an_error() {
    let src = screen_with(
        GAUGE_DECL,
        r#"  gauge {
    current = "{hp}"
    max = "{max_hp}"
    colour = "black"
  }"#,
    );
    let errors = parse_err(&src);
    assert!(
        matches!(errors[0], ParseError::UnknownProp { ref prop, .. } if prop == "colour"),
        "got: {errors:?}"
    );
}

#[test]
fn prop_kind_mismatch_is_an_error() {
    let src = screen_with(
        GAUGE_DECL,
        r#"  gauge {
    current = "{hp}"
    max = "{max_hp}"
    segments = "four"
  }"#,
    );
    let errors = parse_err(&src);
    assert!(
        matches!(
            errors[0],
            ParseError::PropTypeMismatch { ref prop, ref expected, .. }
                if prop == "segments" && expected == "int"
        ),
        "got: {errors:?}"
    );
}

#[test]
fn undeclared_component_type_still_rejected_and_lists_declared_names() {
    let src = screen_with(GAUGE_DECL, "  sparkline {\n  }");
    let errors = parse_err(&src);
    match &errors[0] {
        ParseError::InvalidComponentType { found, valid, .. } => {
            assert_eq!(found, "sparkline");
            assert!(
                valid.iter().any(|v| v == "gauge"),
                "valid list should include declared components"
            );
        }
        other => panic!("expected InvalidComponentType, got {other:?}"),
    }
}

// ── prelude (pre-registered declarations) ─────────────────────────────────

#[test]
fn parse_with_components_accepts_prelude_declared_component() {
    let Document::Components(decls) = parse_ok(GAUGE_DECL) else {
        panic!()
    };

    let screen_src = r#"screen test {
  gauge {
    current = "{hp}"
    max = "{max_hp}"
  }
}"#;
    let (doc, errors) = parser::parse_with_components(lex(screen_src), &decls);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let Some(Document::Screen(screen)) = doc else {
        panic!("expected screen")
    };
    let json = json_ui::compile_screen(&screen).expect("codegen");
    assert!(json.contains("custom:gauge"));

    // The prelude still validates: a missing required prop is caught.
    let bad_src = "screen test {\n  gauge {\n  }\n}";
    let (_, errors) = parser::parse_with_components(lex(bad_src), &decls);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ParseError::MissingRequiredProp { .. })),
        "got: {errors:?}"
    );
}
