use crate::ast::*;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Compile `@theme` → JSON tokens map.
///
/// Input:  `@theme dark { primary="#c9a03d"; background="#1a1a1e" }`
/// Output: `{"name":"dark","tokens":{"primary":"#c9a03d","background":"#1a1a1e"}}`
pub fn compile_theme(theme: &Theme) -> String {
    serde_json::to_string_pretty(&json!({
        "name": theme.name,
        "tokens": theme.tokens,
    }))
    .unwrap()
}

/// Compile `@style` → JSON with parent reference (no inheritance resolution).
///
/// Input:  `@style base { padding=12; color="red" }`
/// Output: `{"name":"base","properties":{"padding":{"NumberLit":12.0},"color":{"StringLit":"red"}}}`
///
/// NOTE: Full inheritance (merging parent props) requires the caller to provide a
/// style lookup. Here we just include the "extends" field for the caller to resolve.
pub fn compile_style(style: &Style) -> String {
    let mut obj = json!({
        "name": style.name,
        "properties": style.properties,
    });
    if let Some(ref parent) = style.extends {
        obj["extends"] = json!(parent);
    }
    serde_json::to_string_pretty(&obj).unwrap()
}

/// Compile multiple styles, resolving inheritance chains.
///
/// Walks up the `extends` chain for each style, merging ancestor properties
/// first (child overrides), and outputs a JSON array of resolved styles.
///
/// # Algorithm
/// 1. Build `HashMap<&str, &Style>` by name
/// 2. For each style, walk up the extends chain (stopping at `None` or cycle)
/// 3. Merge ancestor properties first, then child properties (child overrides)
/// 4. Return JSON array of resolved styles with clean value representations
pub fn compile_styles_resolved(styles: &[Style]) -> String {
    let style_map: HashMap<&str, &Style> = styles.iter().map(|s| (s.name.as_str(), s)).collect();

    let resolved: Vec<Value> = styles
        .iter()
        .map(|s| {
            let mut visited = Vec::new();
            let props = resolve_style(&s.name, &style_map, &mut visited);
            let mut obj = json!({
                "name": s.name,
                "properties": props,
            });
            if let Some(ref parent) = s.extends {
                obj["extends"] = json!(parent);
                let chain = build_inheritance_chain(&s.name, &style_map);
                if chain.len() > 1 {
                    obj["inheritance_chain"] = json!(chain);
                }
            }
            obj
        })
        .collect();

    serde_json::to_string_pretty(&json!(resolved)).unwrap()
}

/// Resolve style properties by walking inheritance chain.
///
/// Returns merged properties: ancestor keys first, child overrides.
fn resolve_style(
    name: &str,
    styles: &HashMap<&str, &Style>,
    visited: &mut Vec<String>,
) -> HashMap<String, Value> {
    let style = match styles.get(name) {
        Some(s) => s,
        None => return HashMap::new(),
    };

    // Cycle detection
    if visited.contains(&name.to_string()) {
        return HashMap::new();
    }
    visited.push(name.to_string());

    // Start with parent's resolved properties (ancestor first)
    let mut props = if let Some(ref parent) = &style.extends {
        resolve_style(parent, styles, visited)
    } else {
        HashMap::new()
    };

    // Merge child properties (child overrides ancestor)
    for (k, v) in &style.properties {
        props.insert(k.clone(), expression_to_value(v));
    }

    props
}

/// Build the inheritance chain from a style name up to the root.
/// Returns `[child, parent, grandparent, ...]`.
fn build_inheritance_chain(name: &str, styles: &HashMap<&str, &Style>) -> Vec<String> {
    let mut chain = Vec::new();
    let mut current = name.to_string();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    while seen.insert(current.clone()) {
        chain.push(current.clone());
        match styles.get(current.as_str()) {
            Some(style) => match &style.extends {
                Some(parent) => current = parent.clone(),
                None => break,
            },
            None => break,
        }
    }

    chain
}

/// Convert an `Expression` to a JSON value, unwrapping literals for cleaner output.
///
/// - `StringLit`, `NumberLit`, `BoolLit` → JSON primitives
/// - `Variable` → `{"$var": "name"}` (distinguished from string literals)
/// - Complex expressions (`BinaryOp`, `TernaryOp`) → serde representation
fn expression_to_value(expr: &Expression) -> Value {
    match expr {
        Expression::StringLit(s) => Value::String(s.clone()),
        Expression::NumberLit(n) => json!(n),
        Expression::BoolLit(b) => Value::Bool(*b),
        Expression::Variable(v) => json!({"$var": v}),
        // Complex expressions: use serde representation
        other => serde_json::to_value(other).unwrap_or(Value::Null),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn span() -> SourceSpan {
        SourceSpan::point("test.scene", 0, 0)
    }

    // -- compile_theme ------------------------------------------------------

    #[test]
    fn theme_codegen_test_theme_simple() {
        let theme = Theme {
            name: "dark".into(),
            tokens: {
                let mut m = HashMap::new();
                m.insert("primary".into(), "#c9a03d".into());
                m.insert("background".into(), "#1a1a1e".into());
                m
            },
            span: span(),
        };
        let json = compile_theme(&theme);
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["name"], "dark");
        assert_eq!(parsed["tokens"]["primary"], "#c9a03d");
        assert_eq!(parsed["tokens"]["background"], "#1a1a1e");
    }

    // -- compile_style ------------------------------------------------------

    #[test]
    fn theme_codegen_test_style_no_extends() {
        let style = Style {
            name: "base".into(),
            extends: None,
            properties: {
                let mut m = HashMap::new();
                m.insert("padding".into(), Expression::NumberLit(12.0));
                m.insert("color".into(), Expression::StringLit("red".into()));
                m
            },
            span: span(),
        };
        let json = compile_style(&style);
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["name"], "base");
        assert!(parsed.get("extends").is_none());
        // Properties are serialized as-is (Expression enums)
        assert!(parsed["properties"]["padding"].is_object());
    }

    #[test]
    fn theme_codegen_test_style_with_extends() {
        let style = Style {
            name: "derived".into(),
            extends: Some("base".into()),
            properties: {
                let mut m = HashMap::new();
                m.insert("margin".into(), Expression::NumberLit(8.0));
                m
            },
            span: span(),
        };
        let json = compile_style(&style);
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["name"], "derived");
        assert_eq!(parsed["extends"], "base");
    }

    // -- compile_styles_resolved --------------------------------------------

    #[test]
    fn theme_codegen_test_style_inheritance_resolved() {
        let base = Style {
            name: "base".into(),
            extends: None,
            properties: {
                let mut m = HashMap::new();
                m.insert("padding".into(), Expression::NumberLit(12.0));
                m.insert("color".into(), Expression::StringLit("red".into()));
                m
            },
            span: span(),
        };
        let child = Style {
            name: "child".into(),
            extends: Some("base".into()),
            properties: {
                let mut m = HashMap::new();
                m.insert("margin".into(), Expression::NumberLit(4.0));
                m
            },
            span: span(),
        };

        let styles = vec![base, child];
        let json = compile_styles_resolved(&styles);
        let parsed: Value = serde_json::from_str(&json).unwrap();

        let arr = parsed.as_array().unwrap();
        let child_obj = arr.iter().find(|s| s["name"] == "child").unwrap();

        assert_eq!(child_obj["extends"], "base");
        assert_eq!(child_obj["properties"]["padding"], json!(12.0));
        assert_eq!(child_obj["properties"]["color"], "red");
        assert_eq!(child_obj["properties"]["margin"], json!(4.0));
    }

    #[test]
    fn theme_codegen_test_styles_multi_level_chain() {
        let grandparent = Style {
            name: "grandparent".into(),
            extends: None,
            properties: {
                let mut m = HashMap::new();
                m.insert("font".into(), Expression::StringLit("Arial".into()));
                m.insert("size".into(), Expression::NumberLit(14.0));
                m
            },
            span: span(),
        };
        let parent = Style {
            name: "parent".into(),
            extends: Some("grandparent".into()),
            properties: {
                let mut m = HashMap::new();
                m.insert("size".into(), Expression::NumberLit(16.0)); // override
                m.insert("weight".into(), Expression::StringLit("bold".into()));
                m
            },
            span: span(),
        };
        let child = Style {
            name: "child".into(),
            extends: Some("parent".into()),
            properties: {
                let mut m = HashMap::new();
                m.insert("color".into(), Expression::StringLit("blue".into()));
                m
            },
            span: span(),
        };

        let styles = vec![grandparent, parent, child];
        let json = compile_styles_resolved(&styles);
        let parsed: Value = serde_json::from_str(&json).unwrap();

        let arr = parsed.as_array().unwrap();
        let child_obj = arr.iter().find(|s| s["name"] == "child").unwrap();

        assert_eq!(child_obj["properties"]["font"], "Arial");
        assert_eq!(child_obj["properties"]["size"], json!(16.0));
        assert_eq!(child_obj["properties"]["weight"], "bold");
        assert_eq!(child_obj["properties"]["color"], "blue");

        // Inheritance chain
        assert_eq!(
            child_obj["inheritance_chain"],
            json!(["child", "parent", "grandparent"])
        );
    }

    #[test]
    fn theme_codegen_test_expression_values() {
        let style = Style {
            name: "expr_test".into(),
            extends: None,
            properties: {
                let mut m = HashMap::new();
                m.insert("str_val".into(), Expression::StringLit("hello".into()));
                m.insert("num_val".into(), Expression::NumberLit(42.0));
                m.insert("bool_val".into(), Expression::BoolLit(true));
                m.insert("var_ref".into(), Expression::Variable("someVar".into()));
                m
            },
            span: span(),
        };

        let styles = vec![style];
        let json = compile_styles_resolved(&styles);
        let parsed: Value = serde_json::from_str(&json).unwrap();

        let props = &parsed[0]["properties"];
        assert_eq!(props["str_val"], "hello");
        assert_eq!(props["num_val"], json!(42.0));
        assert_eq!(props["bool_val"], true);
        assert_eq!(props["var_ref"]["$var"], "someVar");
    }

    #[test]
    fn theme_codegen_test_cycle_detection() {
        let a = Style {
            name: "a".into(),
            extends: Some("b".into()),
            properties: HashMap::new(),
            span: span(),
        };
        let b = Style {
            name: "b".into(),
            extends: Some("a".into()),
            properties: HashMap::new(),
            span: span(),
        };

        let styles = vec![a, b];
        let json = compile_styles_resolved(&styles);
        // Should not loop infinitely — should produce valid JSON
        let _parsed: Value = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn theme_codegen_test_child_override() {
        let base = Style {
            name: "base".into(),
            extends: None,
            properties: {
                let mut m = HashMap::new();
                m.insert("color".into(), Expression::StringLit("red".into()));
                m.insert("padding".into(), Expression::NumberLit(10.0));
                m
            },
            span: span(),
        };
        let child = Style {
            name: "child".into(),
            extends: Some("base".into()),
            properties: {
                let mut m = HashMap::new();
                m.insert("color".into(), Expression::StringLit("blue".into())); // override
                m
            },
            span: span(),
        };

        let styles = vec![base, child];
        let json = compile_styles_resolved(&styles);
        let parsed: Value = serde_json::from_str(&json).unwrap();

        let arr = parsed.as_array().unwrap();
        let child_obj = arr.iter().find(|s| s["name"] == "child").unwrap();
        assert_eq!(child_obj["properties"]["color"], "blue");
        assert_eq!(child_obj["properties"]["padding"], json!(10.0));
    }
}
