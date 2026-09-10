// Variables → JavaScript codegen.
//
// Transforms AST `@variables` declarations and inline variable expressions
// into JavaScript `let` declarations and references.

use crate::ast::*;
use crate::sourcemap::SourceMapBuilder;

/// Compile a `@variables` block into JavaScript `let` declarations.
///
/// Each declaration gets a source map entry mapping the DSL variable span
/// back to the generated JS line.
///
/// # Example
/// ```
/// // Input:  @variables { gold = 500; name = "RED"; has_potion = true }
/// // Output: let gold = 500;
/// //         let name = "RED";
/// //         let has_potion = true;
/// ```
pub fn compile_variables(vars: &VariablesBlock, sourcemap: &mut SourceMapBuilder) -> String {
    let mut output = String::new();
    let mut generated_line: u32 = 0;

    for decl in &vars.decls {
        let value_js = compile_expression(&decl.value);
        let stmt = format!("let {} = {};", decl.name, value_js);

        // Record source mapping: generated line 0-based → DSL source position
        sourcemap.add_mapping(
            generated_line,
            0,
            decl.span.line_start as u32,
            decl.span.col_start as u32,
        );

        output.push_str(&stmt);
        output.push('\n');
        generated_line += 1;
    }

    output
}

/// Compile a DSL expression into a JavaScript expression string.
///
/// Handles all expression types supported in the MVP AST:
/// - String literals (escaped via `serde_json::to_string`)
/// - Number literals
/// - Boolean literals
/// - Variable references
/// - Binary operations (arithmetic, comparison, logical)
/// - Ternary conditional expressions
///
/// # String escaping
///
/// Uses `serde_json::to_string()` which correctly handles:
/// - Embedded double quotes → `\"`
/// - Newlines → `\n`
/// - Backslashes → `\\`
/// - Unicode characters
pub fn compile_expression(expr: &Expression) -> String {
    match expr {
        Expression::StringLit(s) => {
            // serde_json handles all JSON string escaping, which is compatible
            // with JavaScript string literals (both use \", \\, \n, etc.)
            serde_json::to_string(s).unwrap_or_else(|_| format!("\"{}\"", s))
        }

        Expression::Localized(pairs) => crate::codegen::i18n::localized_pairs_to_js_t(pairs),

        Expression::NumberLit(n) => {
            // Rust's f64 Display strips trailing zeros, producing clean
            // JS-compatible numbers (e.g. 500.0 → "500", 3.14 → "3.14")
            format!("{}", n)
        }

        Expression::BoolLit(true) => "true".to_string(),
        Expression::BoolLit(false) => "false".to_string(),

        Expression::Variable(name) => name.clone(),

        Expression::ArrayLit(elements) => {
            let parts: Vec<String> = elements.iter().map(compile_expression).collect();
            format!("[{}]", parts.join(", "))
        }

        Expression::Call { callee, args } => {
            let args_js: Vec<String> = args.iter().map(compile_expression).collect();
            format!("{}({})", callee, args_js.join(", "))
        }

        Expression::UnaryOp { op, operand } => {
            let inner = compile_expression(operand);
            match op {
                UnaryOp::Not => format!("(!{inner})"),
                UnaryOp::Neg => format!("(-{inner})"),
            }
        }

        Expression::BinaryOp { op, left, right } => {
            let left_js = compile_expression(left);
            let right_js = compile_expression(right);

            let op_str = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Eq => "===",
                BinOp::Neq => "!==",
                BinOp::Gt => ">",
                BinOp::Lt => "<",
                BinOp::Gte => ">=",
                BinOp::Lte => "<=",
                BinOp::And => "&&",
                BinOp::BitAnd => "&",
                BinOp::BitOr => "|",
                BinOp::Or => "||",
            };

            // Wrap in parens to preserve precedence in all contexts.
            // Redundant parens are semantically harmless in JS.
            format!("({} {} {})", left_js, op_str, right_js)
        }

        Expression::TernaryOp {
            condition,
            then_expr,
            else_expr,
        } => {
            let cond_js = compile_expression(condition);
            let then_js = compile_expression(then_expr);
            let else_js = compile_expression(else_expr);

            // Wrap in parens to ensure correct precedence when used inside
            // larger expressions.
            format!("({} ? {} : {})", cond_js, then_js, else_js)
        }

        Expression::ObjectLit(fields) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{}: {}", k, compile_expression(v)))
                .collect();
            format!("{{ {} }}", parts.join(", "))
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: construct a SourceSpan for test purposes.
    fn test_span(line: usize, col: usize) -> SourceSpan {
        SourceSpan {
            file: "test.scene".to_string(),
            line_start: line,
            col_start: col,
            line_end: line,
            col_end: col + 1,
            byte_start: 0,
            byte_end: 0,
        }
    }

    /// Helper: create a SourceMapBuilder for tests.
    fn test_sourcemap() -> SourceMapBuilder {
        SourceMapBuilder::new("test.scene", "test.scene.js")
    }

    // -----------------------------------------------------------------------
    // Variable declaration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_simple_variable_declaration() {
        let vars = VariablesBlock {
            decls: vec![VariableDecl {
                name: "gold".to_string(),
                value: Expression::NumberLit(500.0),
                span: test_span(2, 4),
            }],
            span: test_span(1, 0),
        };

        let mut sm = test_sourcemap();
        let output = compile_variables(&vars, &mut sm);

        assert_eq!(output, "let gold = 500;\n");
        assert_eq!(sm.mappings().len(), 1);
        assert_eq!(sm.mappings()[0].generated_line, 0);
        assert_eq!(sm.mappings()[0].source_line, 2);
    }

    #[test]
    fn test_string_variable() {
        let vars = VariablesBlock {
            decls: vec![VariableDecl {
                name: "name".to_string(),
                value: Expression::StringLit("RED".to_string()),
                span: test_span(2, 4),
            }],
            span: test_span(1, 0),
        };

        let mut sm = test_sourcemap();
        let output = compile_variables(&vars, &mut sm);

        assert_eq!(output, "let name = \"RED\";\n");
    }

    #[test]
    fn test_boolean_variable() {
        let vars = VariablesBlock {
            decls: vec![VariableDecl {
                name: "has_potion".to_string(),
                value: Expression::BoolLit(true),
                span: test_span(2, 4),
            }],
            span: test_span(1, 0),
        };

        let mut sm = test_sourcemap();
        let output = compile_variables(&vars, &mut sm);

        assert_eq!(output, "let has_potion = true;\n");
    }

    #[test]
    fn test_boolean_false_variable() {
        let vars = VariablesBlock {
            decls: vec![VariableDecl {
                name: "is_defeated".to_string(),
                value: Expression::BoolLit(false),
                span: test_span(2, 4),
            }],
            span: test_span(1, 0),
        };

        let mut sm = test_sourcemap();
        let output = compile_variables(&vars, &mut sm);

        assert_eq!(output, "let is_defeated = false;\n");
    }

    #[test]
    fn test_multiple_variables() {
        let vars = VariablesBlock {
            decls: vec![
                VariableDecl {
                    name: "gold".to_string(),
                    value: Expression::NumberLit(500.0),
                    span: test_span(2, 4),
                },
                VariableDecl {
                    name: "name".to_string(),
                    value: Expression::StringLit("RED".to_string()),
                    span: test_span(3, 4),
                },
                VariableDecl {
                    name: "has_potion".to_string(),
                    value: Expression::BoolLit(true),
                    span: test_span(4, 4),
                },
            ],
            span: test_span(1, 0),
        };

        let mut sm = test_sourcemap();
        let output = compile_variables(&vars, &mut sm);

        let expected = [
            "let gold = 500;",
            "let name = \"RED\";",
            "let has_potion = true;",
            "", // trailing newline
        ]
        .join("\n");

        assert_eq!(output, expected);
        assert_eq!(sm.mappings().len(), 3);

        // Each mapping increments the generated line
        for (i, mapping) in sm.mappings().iter().enumerate() {
            assert_eq!(
                mapping.generated_line, i as u32,
                "mapping {i} should have generated_line = {i}"
            );
        }
    }

    #[test]
    fn test_empty_variables_block() {
        let vars = VariablesBlock {
            decls: vec![],
            span: test_span(1, 0),
        };

        let mut sm = test_sourcemap();
        let output = compile_variables(&vars, &mut sm);

        assert_eq!(output, "");
        assert_eq!(sm.mappings().len(), 0);
    }

    // -----------------------------------------------------------------------
    // Expression compilation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_expression_string_literal() {
        let result = compile_expression(&Expression::StringLit("Hello, World!".to_string()));
        assert_eq!(result, "\"Hello, World!\"");
    }

    #[test]
    fn test_expression_string_with_embedded_quotes() {
        let result = compile_expression(&Expression::StringLit("He said \"Hi\"".to_string()));
        assert_eq!(result, "\"He said \\\"Hi\\\"\"");
    }

    #[test]
    fn test_expression_string_with_newlines() {
        let result = compile_expression(&Expression::StringLit("line1\nline2".to_string()));
        assert_eq!(result, "\"line1\\nline2\"");
    }

    #[test]
    fn test_expression_number_integer() {
        let result = compile_expression(&Expression::NumberLit(42.0));
        assert_eq!(result, "42");
    }

    #[test]
    fn test_expression_number_float() {
        let result = compile_expression(&Expression::NumberLit(3.14));
        assert_eq!(result, "3.14");
    }

    #[test]
    fn test_expression_boolean_true() {
        let result = compile_expression(&Expression::BoolLit(true));
        assert_eq!(result, "true");
    }

    #[test]
    fn test_expression_boolean_false() {
        let result = compile_expression(&Expression::BoolLit(false));
        assert_eq!(result, "false");
    }

    #[test]
    fn test_expression_variable() {
        let result = compile_expression(&Expression::Variable("gold".to_string()));
        assert_eq!(result, "gold");
    }

    #[test]
    fn test_expression_binary_add() {
        let expr = Expression::BinaryOp {
            op: BinOp::Add,
            left: Box::new(Expression::NumberLit(100.0)),
            right: Box::new(Expression::NumberLit(50.0)),
        };
        assert_eq!(compile_expression(&expr), "(100 + 50)");
    }

    #[test]
    fn test_expression_binary_sub() {
        let expr = Expression::BinaryOp {
            op: BinOp::Sub,
            left: Box::new(Expression::Variable("gold".to_string())),
            right: Box::new(Expression::NumberLit(100.0)),
        };
        assert_eq!(compile_expression(&expr), "(gold - 100)");
    }

    #[test]
    fn test_expression_binary_mul() {
        let expr = Expression::BinaryOp {
            op: BinOp::Mul,
            left: Box::new(Expression::NumberLit(3.0)),
            right: Box::new(Expression::NumberLit(7.0)),
        };
        assert_eq!(compile_expression(&expr), "(3 * 7)");
    }

    #[test]
    fn test_expression_binary_div() {
        let expr = Expression::BinaryOp {
            op: BinOp::Div,
            left: Box::new(Expression::Variable("total".to_string())),
            right: Box::new(Expression::NumberLit(2.0)),
        };
        assert_eq!(compile_expression(&expr), "(total / 2)");
    }

    #[test]
    fn test_expression_binary_eq() {
        let expr = Expression::BinaryOp {
            op: BinOp::Eq,
            left: Box::new(Expression::Variable("x".to_string())),
            right: Box::new(Expression::NumberLit(10.0)),
        };
        assert_eq!(compile_expression(&expr), "(x === 10)");
    }

    #[test]
    fn test_expression_binary_neq() {
        let expr = Expression::BinaryOp {
            op: BinOp::Neq,
            left: Box::new(Expression::Variable("y".to_string())),
            right: Box::new(Expression::BoolLit(false)),
        };
        assert_eq!(compile_expression(&expr), "(y !== false)");
    }

    #[test]
    fn test_expression_binary_gt() {
        let expr = Expression::BinaryOp {
            op: BinOp::Gt,
            left: Box::new(Expression::Variable("gold".to_string())),
            right: Box::new(Expression::NumberLit(100.0)),
        };
        assert_eq!(compile_expression(&expr), "(gold > 100)");
    }

    #[test]
    fn test_expression_binary_lt() {
        let expr = Expression::BinaryOp {
            op: BinOp::Lt,
            left: Box::new(Expression::NumberLit(50.0)),
            right: Box::new(Expression::Variable("max".to_string())),
        };
        assert_eq!(compile_expression(&expr), "(50 < max)");
    }

    #[test]
    fn test_expression_binary_gte() {
        let expr = Expression::BinaryOp {
            op: BinOp::Gte,
            left: Box::new(Expression::Variable("level".to_string())),
            right: Box::new(Expression::NumberLit(5.0)),
        };
        assert_eq!(compile_expression(&expr), "(level >= 5)");
    }

    #[test]
    fn test_expression_binary_lte() {
        let expr = Expression::BinaryOp {
            op: BinOp::Lte,
            left: Box::new(Expression::NumberLit(100.0)),
            right: Box::new(Expression::Variable("cap".to_string())),
        };
        assert_eq!(compile_expression(&expr), "(100 <= cap)");
    }

    #[test]
    fn test_expression_binary_and() {
        let expr = Expression::BinaryOp {
            op: BinOp::And,
            left: Box::new(Expression::BoolLit(true)),
            right: Box::new(Expression::BoolLit(false)),
        };
        assert_eq!(compile_expression(&expr), "(true && false)");
    }

    #[test]
    fn test_expression_binary_or() {
        let expr = Expression::BinaryOp {
            op: BinOp::Or,
            left: Box::new(Expression::Variable("a".to_string())),
            right: Box::new(Expression::Variable("b".to_string())),
        };
        assert_eq!(compile_expression(&expr), "(a || b)");
    }

    #[test]
    fn test_expression_ternary() {
        let expr = Expression::TernaryOp {
            condition: Box::new(Expression::BinaryOp {
                op: BinOp::Gt,
                left: Box::new(Expression::Variable("gold".to_string())),
                right: Box::new(Expression::NumberLit(100.0)),
            }),
            then_expr: Box::new(Expression::StringLit("rich".to_string())),
            else_expr: Box::new(Expression::StringLit("poor".to_string())),
        };
        assert_eq!(
            compile_expression(&expr),
            "((gold > 100) ? \"rich\" : \"poor\")"
        );
    }

    #[test]
    fn test_expression_nested_binary() {
        // (a + b) * c
        let expr = Expression::BinaryOp {
            op: BinOp::Mul,
            left: Box::new(Expression::BinaryOp {
                op: BinOp::Add,
                left: Box::new(Expression::Variable("a".to_string())),
                right: Box::new(Expression::Variable("b".to_string())),
            }),
            right: Box::new(Expression::Variable("c".to_string())),
        };
        assert_eq!(compile_expression(&expr), "((a + b) * c)");
    }

    #[test]
    fn test_expression_nested_ternary() {
        // a ? (b ? "x" : "y") : "z"
        let expr = Expression::TernaryOp {
            condition: Box::new(Expression::Variable("a".to_string())),
            then_expr: Box::new(Expression::TernaryOp {
                condition: Box::new(Expression::Variable("b".to_string())),
                then_expr: Box::new(Expression::StringLit("x".to_string())),
                else_expr: Box::new(Expression::StringLit("y".to_string())),
            }),
            else_expr: Box::new(Expression::StringLit("z".to_string())),
        };
        assert_eq!(
            compile_expression(&expr),
            "(a ? (b ? \"x\" : \"y\") : \"z\")"
        );
    }

    // -----------------------------------------------------------------------
    // Variable with expression-initializer tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_variable_with_binary_expression() {
        let vars = VariablesBlock {
            decls: vec![VariableDecl {
                name: "total".to_string(),
                value: Expression::BinaryOp {
                    op: BinOp::Add,
                    left: Box::new(Expression::NumberLit(100.0)),
                    right: Box::new(Expression::NumberLit(50.0)),
                },
                span: test_span(2, 4),
            }],
            span: test_span(1, 0),
        };

        let mut sm = test_sourcemap();
        let output = compile_variables(&vars, &mut sm);

        assert_eq!(output, "let total = (100 + 50);\n");
    }

    #[test]
    fn test_variable_with_ternary_expression() {
        let vars = VariablesBlock {
            decls: vec![VariableDecl {
                name: "status".to_string(),
                value: Expression::TernaryOp {
                    condition: Box::new(Expression::BinaryOp {
                        op: BinOp::Gt,
                        left: Box::new(Expression::Variable("gold".to_string())),
                        right: Box::new(Expression::NumberLit(100.0)),
                    }),
                    then_expr: Box::new(Expression::StringLit("wealthy".to_string())),
                    else_expr: Box::new(Expression::StringLit("broke".to_string())),
                },
                span: test_span(2, 4),
            }],
            span: test_span(1, 0),
        };

        let mut sm = test_sourcemap();
        let output = compile_variables(&vars, &mut sm);

        assert_eq!(
            output,
            "let status = ((gold > 100) ? \"wealthy\" : \"broke\");\n"
        );
    }

    // -----------------------------------------------------------------------
    // Source map coverage tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_sourcemap_multiple_mappings() {
        let vars = VariablesBlock {
            decls: vec![
                VariableDecl {
                    name: "a".to_string(),
                    value: Expression::NumberLit(1.0),
                    span: SourceSpan::new("test.scene", 2, 4, 2, 5, 0, 0),
                },
                VariableDecl {
                    name: "b".to_string(),
                    value: Expression::NumberLit(2.0),
                    span: SourceSpan::new("test.scene", 3, 4, 3, 5, 0, 0),
                },
                VariableDecl {
                    name: "c".to_string(),
                    value: Expression::NumberLit(3.0),
                    span: SourceSpan::new("test.scene", 4, 4, 4, 5, 0, 0),
                },
            ],
            span: test_span(1, 0),
        };

        let mut sm = test_sourcemap();
        let _ = compile_variables(&vars, &mut sm);

        assert_eq!(sm.mappings().len(), 3);
        assert_eq!(sm.mappings()[0].source_line, 2);
        assert_eq!(sm.mappings()[1].source_line, 3);
        assert_eq!(sm.mappings()[2].source_line, 4);
    }
}
