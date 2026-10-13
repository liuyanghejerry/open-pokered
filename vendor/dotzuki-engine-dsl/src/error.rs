use miette::{Diagnostic, SourceSpan as MietteSpan};
use thiserror::Error;

/// DSL compilation error with source location and actionable help.
///
/// Each variant carries a unique error code (searchable via `E0xx`),
/// a source span for pinpointing the error location, and actionable
/// help text to guide the user toward a fix.
#[derive(Error, Diagnostic, Debug)]
pub enum DslError {
    /// Syntax error — unexpected or malformed token at a specific location.
    #[error("Syntax error: unexpected token")]
    #[diagnostic(
        code("E001"),
        help("Check for missing block delimiters `{{}}` or mismatched parentheses")
    )]
    Syntax {
        /// The source code snippet for rich miette display.
        #[source_code]
        src: String,
        /// Byte-offset span pointing to the error location.
        #[label("here")]
        span: MietteSpan,
        /// Source file path.
        file: String,
        /// 1-based line number.
        line: usize,
        /// 0-based column offset.
        col: usize,
    },

    /// Unknown `@directive` encountered in source.
    #[error("Unknown directive `{name}`")]
    #[diagnostic(
        code("E002"),
        help("Valid directives: @variables, @theme, @style, @atlas, @storylines, @speaker, @choice, @option, @if, @else, @each")
    )]
    UnknownDirective {
        /// The unrecognized directive name (without `@` prefix).
        name: String,
        /// Byte-offset span of the directive token.
        #[label]
        span: MietteSpan,
        /// Source file path.
        file: String,
        /// 1-based line number.
        line: usize,
    },

    /// Reference to a variable that has not been declared in `@variables`.
    #[error("Undefined variable `{name}`")]
    #[diagnostic(
        code("E003"),
        help("Did you forget to declare `{name}` in `@variables`?")
    )]
    UndefinedVariable {
        /// The undefined variable name.
        name: String,
        /// Comma-separated list of variables that ARE defined (for suggestions).
        defined_vars: String,
        /// Byte-offset span of the variable reference.
        #[label]
        span: MietteSpan,
        /// Source file path.
        file: String,
        /// 1-based line number.
        line: usize,
    },

    /// A `@style` block has a circular inheritance chain.
    #[error("Circular style inheritance: {chain}")]
    #[diagnostic(
        code("E004"),
        help("Remove the circular reference in `@style` inheritance chains")
    )]
    CircularStyle {
        /// The cycle path, e.g. "A -> B -> C -> A".
        chain: String,
        /// Byte-offset span of the offending style.
        #[label]
        span: MietteSpan,
        /// Source file path.
        file: String,
        /// 1-based line number.
        line: usize,
    },

    /// A component type is not recognised.
    #[error("Invalid component type `{found}`")]
    #[diagnostic(
        code("E005"),
        help(
            "Valid component types: panel, container, text, button, list, image, input, dropdown"
        )
    )]
    InvalidComponent {
        /// The invalid type name that was found.
        found: String,
        /// Byte-offset span of the component type token.
        #[label]
        span: MietteSpan,
        /// Source file path.
        file: String,
        /// 1-based line number.
        line: usize,
    },

    /// Reference to an atlas region that doesn't exist.
    #[error("Missing atlas region `{region}`")]
    #[diagnostic(code("E006"), help("Available regions: {available}"))]
    MissingRegion {
        /// The region name that was requested but not found.
        region: String,
        /// Comma-separated list of available region names.
        available: String,
        /// Byte-offset span of the region reference.
        #[label]
        span: MietteSpan,
        /// Source file path.
        file: String,
        /// 1-based line number.
        line: usize,
    },

    /// A `@choice` block contains zero `@option` entries.
    #[error("Empty @choice block")]
    #[diagnostic(code("E007"), help("A @choice must have at least one @option"))]
    EmptyChoice {
        /// Byte-offset span of the choice block.
        #[label]
        span: MietteSpan,
        /// Source file path.
        file: String,
        /// 1-based line number.
        line: usize,
    },

    /// Indentation is inconsistent or uses tabs.
    #[error("Indentation error: {msg}")]
    #[diagnostic(
        code("E008"),
        help("Use consistent spaces (2 or 4) for indentation. Tabs are not allowed.")
    )]
    IndentationError {
        /// Human-readable description of the indentation problem.
        msg: String,
        /// Byte-offset span on the offending line.
        #[label]
        span: MietteSpan,
        /// Source file path.
        file: String,
        /// 1-based line number.
        line: usize,
    },

    /// Catch-all for internal compiler bugs (should never fire).
    #[error("Internal compiler error: {msg}")]
    #[diagnostic(code("E999"))]
    InternalError {
        /// Description of the internal failure.
        msg: String,
        /// Optional byte-offset span.
        span: Option<MietteSpan>,
        /// Source file path.
        file: String,
        /// 1-based line number.
        line: usize,
    },
}

/// Convert our AST [`SourceSpan`](crate::ast::SourceSpan) (line/col-based) into
/// miette's [`SourceSpan`](miette::SourceSpan) (byte-offset + length).
///
/// The conversion walks the source text to find the byte offset corresponding
/// to the given line and column.  `col` is treated as a **character** index
/// (not a byte index), so the function correctly handles multi-byte UTF-8
/// characters.
///
/// # Panics
///
/// Does **not** panic, but returns a zero-length span at offset 0 if the
/// span coordinates fall outside the source text (e.g. an empty source string
/// or out-of-bounds line).
pub fn to_miette_span(span: &crate::ast::SourceSpan, source_text: &str) -> MietteSpan {
    let start_offset = line_col_to_byte_offset(source_text, span.line_start, span.col_start);
    let end_offset = line_col_to_byte_offset(source_text, span.line_end, span.col_end);
    let len = end_offset.saturating_sub(start_offset);
    MietteSpan::new(start_offset.into(), len)
}

/// Map a 1-based line + 0-based character column to a byte offset within `source`.
///
/// `col` counts characters (not bytes), so multi-byte UTF-8 sequences are
/// handled correctly.  The returned offset is the byte position in `source`
/// where the given (line, col) begins.
fn line_col_to_byte_offset(source: &str, target_line: usize, target_col: usize) -> usize {
    let mut byte_offset: usize = 0;
    let mut current_line: usize = 1;
    let mut current_col: usize = 0; // 0-based, matching the lexer's col

    for ch in source.chars() {
        // Stop once we've reached the target position.
        if current_line == target_line && current_col >= target_col {
            break;
        }
        if ch == '\n' {
            current_line += 1;
            current_col = 0;
        } else {
            current_col += 1;
        }
        byte_offset += ch.len_utf8();
    }
    byte_offset
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // to_miette_span tests
    // ------------------------------------------------------------------

    #[test]
    fn test_to_miette_span_single_line() {
        let src = "hello world";
        // The span covers "hello" on line 1, cols 0..5.
        let span = crate::ast::SourceSpan::new("test.dsl", 1, 0, 1, 5, 0, 0);
        let mspan = to_miette_span(&span, src);
        assert_eq!(mspan.offset(), 0);
        assert_eq!(mspan.len(), 5); // "hello" is 5 bytes
    }

    #[test]
    fn test_to_miette_span_multi_line() {
        let src = "line1\nline2\nline3";
        // Span covering "line2" — line 2, cols 0..5.
        let span = crate::ast::SourceSpan::new("test.dsl", 2, 0, 2, 5, 0, 0);
        let mspan = to_miette_span(&span, src);
        // "line1\n" = 6 bytes, so offset should be 6.
        assert_eq!(mspan.offset(), 6);
        assert_eq!(mspan.len(), 5);
        // Verify the bytes at that offset spell "line2".
        assert_eq!(&src.as_bytes()[6..11], b"line2");
    }

    #[test]
    fn test_to_miette_span_point_span() {
        let src = "abc\ndef";
        // Point span at line 2, col 0 (start of "def").
        let span = crate::ast::SourceSpan::point("test.dsl", 2, 0);
        let mspan = to_miette_span(&span, src);
        assert_eq!(mspan.offset(), 4); // "abc\n" = 4 bytes
        assert_eq!(mspan.len(), 0); // zero-width
    }

    #[test]
    fn test_to_miette_span_utf8_multibyte() {
        let src = "héllo wörld";
        // "héllo" on line 1 — é is 2 bytes in UTF-8, so 5 chars but 6 bytes.
        let span = crate::ast::SourceSpan::new("test.dsl", 1, 0, 1, 5, 0, 0);
        let mspan = to_miette_span(&span, src);
        assert_eq!(mspan.offset(), 0);
        assert_eq!(mspan.len(), 6); // 'h'(1) + 'é'(2) + 'l'(1) + 'l'(1) + 'o'(1) = 6
    }

    // ------------------------------------------------------------------
    // DslError variant tests — error message + diagnostic metadata
    // ------------------------------------------------------------------

    /// Helper: extract the `.code()` from a DslError via miette's `Diagnostic` trait.
    fn error_code(err: &DslError) -> Option<String> {
        use miette::Diagnostic;
        err.code().map(|c| c.to_string())
    }

    /// Helper: extract the `.help()` text from a DslError.
    fn error_help(err: &DslError) -> Option<String> {
        use miette::Diagnostic;
        err.help().map(|h| h.to_string())
    }

    fn dummy_span() -> MietteSpan {
        MietteSpan::new(0.into(), 5)
    }

    #[test]
    fn test_syntax_error_with_location() {
        let err = DslError::Syntax {
            src: "game_scene {".into(),
            span: dummy_span(),
            file: "test.scene".into(),
            line: 1,
            col: 13,
        };
        assert_eq!(err.to_string(), "Syntax error: unexpected token");
        assert_eq!(error_code(&err).as_deref(), Some("E001"));
        assert!(error_help(&err).unwrap().contains("block delimiters"));
        // Metadata fields are preserved.
        if let DslError::Syntax {
            file, line, col, ..
        } = &err
        {
            assert_eq!(file, "test.scene");
            assert_eq!(*line, 1);
            assert_eq!(*col, 13);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn test_unknown_directive_error() {
        let err = DslError::UnknownDirective {
            name: "banana".into(),
            span: dummy_span(),
            file: "test.scene".into(),
            line: 5,
        };
        assert_eq!(err.to_string(), "Unknown directive `banana`");
        assert_eq!(error_code(&err).as_deref(), Some("E002"));
        let help = error_help(&err).unwrap();
        assert!(
            help.contains("@variables"),
            "help should list valid directives, got: {help}"
        );
    }

    #[test]
    fn test_undefined_variable_error() {
        let err = DslError::UndefinedVariable {
            name: "gold".into(),
            defined_vars: "hp, mp, xp".into(),
            span: dummy_span(),
            file: "test.scene".into(),
            line: 10,
        };
        assert_eq!(err.to_string(), "Undefined variable `gold`");
        assert_eq!(error_code(&err).as_deref(), Some("E003"));
        let help = error_help(&err).unwrap();
        assert!(
            help.contains("gold"),
            "help should mention the variable name"
        );
        assert!(
            help.contains("@variables"),
            "help should reference @variables"
        );
    }

    #[test]
    fn test_circular_style_error() {
        let err = DslError::CircularStyle {
            chain: "A -> B -> C -> A".into(),
            span: dummy_span(),
            file: "test.scene".into(),
            line: 20,
        };
        assert_eq!(
            err.to_string(),
            "Circular style inheritance: A -> B -> C -> A"
        );
        assert_eq!(error_code(&err).as_deref(), Some("E004"));
        assert!(error_help(&err).unwrap().contains("circular"));
    }

    #[test]
    fn test_invalid_component_error() {
        let err = DslError::InvalidComponent {
            found: "widget".into(),
            span: dummy_span(),
            file: "test.gui".into(),
            line: 3,
        };
        assert_eq!(err.to_string(), "Invalid component type `widget`");
        assert_eq!(error_code(&err).as_deref(), Some("E005"));
        let help = error_help(&err).unwrap();
        assert!(help.contains("panel"), "help should list valid components");
        assert!(help.contains("button"), "help should list valid components");
    }

    #[test]
    fn test_indentation_error() {
        let err = DslError::IndentationError {
            msg: "Tabs are not allowed".into(),
            span: dummy_span(),
            file: "test.scene".into(),
            line: 7,
        };
        assert_eq!(err.to_string(), "Indentation error: Tabs are not allowed");
        assert_eq!(error_code(&err).as_deref(), Some("E008"));
        assert!(error_help(&err).unwrap().contains("spaces"));
    }

    #[test]
    fn test_empty_choice_error() {
        let err = DslError::EmptyChoice {
            span: dummy_span(),
            file: "test.scene".into(),
            line: 12,
        };
        assert_eq!(err.to_string(), "Empty @choice block");
        assert_eq!(error_code(&err).as_deref(), Some("E007"));
        assert!(error_help(&err).unwrap().contains("@choice"));
        assert!(error_help(&err).unwrap().contains("@option"));
    }

    #[test]
    fn test_missing_region_error() {
        let err = DslError::MissingRegion {
            region: "btn_hover".into(),
            available: "btn_normal, btn_pressed, btn_disabled".into(),
            span: dummy_span(),
            file: "test.scene".into(),
            line: 15,
        };
        assert_eq!(err.to_string(), "Missing atlas region `btn_hover`");
        assert_eq!(error_code(&err).as_deref(), Some("E006"));
        assert!(error_help(&err).unwrap().contains("btn_normal"));
    }

    #[test]
    fn test_internal_error() {
        let err = DslError::InternalError {
            msg: "unexpected state in codegen".into(),
            span: Some(dummy_span()),
            file: "test.scene".into(),
            line: 42,
        };
        assert_eq!(
            err.to_string(),
            "Internal compiler error: unexpected state in codegen"
        );
        assert_eq!(error_code(&err).as_deref(), Some("E999"));
        // Internal errors have no help text by design.
        assert!(error_help(&err).is_none());
    }

    #[test]
    fn test_all_error_codes_are_unique() {
        use std::collections::HashSet;
        let mut codes = HashSet::new();

        // Construct one of each variant and collect error codes.
        let errors: Vec<DslError> = vec![
            DslError::Syntax {
                src: "".into(),
                span: dummy_span(),
                file: "".into(),
                line: 1,
                col: 0,
            },
            DslError::UnknownDirective {
                name: "".into(),
                span: dummy_span(),
                file: "".into(),
                line: 1,
            },
            DslError::UndefinedVariable {
                name: "".into(),
                defined_vars: "".into(),
                span: dummy_span(),
                file: "".into(),
                line: 1,
            },
            DslError::CircularStyle {
                chain: "".into(),
                span: dummy_span(),
                file: "".into(),
                line: 1,
            },
            DslError::InvalidComponent {
                found: "".into(),
                span: dummy_span(),
                file: "".into(),
                line: 1,
            },
            DslError::MissingRegion {
                region: "".into(),
                available: "".into(),
                span: dummy_span(),
                file: "".into(),
                line: 1,
            },
            DslError::EmptyChoice {
                span: dummy_span(),
                file: "".into(),
                line: 1,
            },
            DslError::IndentationError {
                msg: "".into(),
                span: dummy_span(),
                file: "".into(),
                line: 1,
            },
            DslError::InternalError {
                msg: "".into(),
                span: Some(dummy_span()),
                file: "".into(),
                line: 1,
            },
        ];

        for err in &errors {
            let code = error_code(err).expect("every variant must have an error code");
            assert!(codes.insert(code.clone()), "duplicate error code: {code}");
        }
        assert_eq!(codes.len(), errors.len(), "all codes must be unique");
    }
}
