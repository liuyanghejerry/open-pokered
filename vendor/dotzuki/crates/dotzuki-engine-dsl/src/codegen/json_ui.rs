// JSON codegen for UI blocks — transforms AST UiBlock/UiComponent nodes into
// ScreenLayout-compatible JSON strings for the dotzuki-renderer.

use crate::ast::*;
use serde_json::{json, Value};

/// Render author-facing text as a JSON value for the `value` field.
///
/// `Plain` text emits a JSON string (unchanged from the monolingual path);
/// `@t("en", "中文")` emits a `{"en": …, "zh": …}` object that the renderer
/// resolves against the active language at draw time.
fn localized_to_json_value(text: &LocalizedText) -> Value {
    match text {
        LocalizedText::Plain(s) => Value::String(s.clone()),
        LocalizedText::Localized(pairs) => {
            let map: serde_json::Map<String, Value> = pairs
                .iter()
                .map(|(locale, t)| (locale.clone(), Value::String(t.clone())))
                .collect();
            Value::Object(map)
        }
    }
}

/// Compile a UI block into a ScreenLayout-compatible JSON string.
///
/// All components are wrapped in a top-level `group` element with `children`.
pub fn compile_ui(ui: &UiBlock) -> Result<String, String> {
    let children: Vec<Value> = ui.components.iter().map(compile_component).collect();
    let root = json!({
        "type": "group",
        "children": children,
    });
    serde_json::to_string_pretty(&root).map_err(|e| format!("JSON serialization failed: {e}"))
}

/// Compile a ScreenLayout into a pokered schema v2 JSON string.
///
/// Outputs: `{"schema_version": 2, "screen": "<name>", "elements": [...]}`
pub fn compile_screen(screen: &ScreenLayout) -> Result<String, String> {
    let version = screen.schema_version.unwrap_or(2);
    let elements: Vec<Value> = screen.components.iter().map(compile_component).collect();
    let root = json!({
        "schema_version": version,
        "screen": screen.name,
        "elements": elements,
    });
    serde_json::to_string_pretty(&root).map_err(|e| format!("JSON serialization failed: {e}"))
}

/// Compile a single UI component into its JSON representation.
pub fn compile_component(comp: &UiComponent) -> Value {
    match comp {
        // ── Panel → border + children ──────────────────────────────────
        UiComponent::Panel {
            props, children, ..
        } => {
            let mut obj = json!({ "type": "border" });
            let child_vals: Vec<Value> = children.iter().map(compile_component).collect();
            if !child_vals.is_empty() {
                obj["children"] = Value::Array(child_vals);
            }
            merge_props(&mut obj, props);
            obj
        }

        // ── Container → group + children ───────────────────────────────
        UiComponent::Container {
            props, children, ..
        } => {
            let mut obj = json!({ "type": "group" });
            let child_vals: Vec<Value> = children.iter().map(compile_component).collect();
            if !child_vals.is_empty() {
                obj["children"] = Value::Array(child_vals);
            }
            merge_props(&mut obj, props);
            obj
        }

        // ── Text ───────────────────────────────────────────────────────
        UiComponent::Text { content, props, .. } => {
            // The renderer's `TextParams` reads the string from the canonical
            // `value` field (with `deny_unknown_fields`); emitting `content`
            // instead made text elements fall through to `Custom` and render
            // blank. A `value = …` prop (a template binding) overrides the
            // positional argument; otherwise the positional text is emitted,
            // either as a plain string or — for `@t("en", "中文")` — as a
            // `{"en": …, "zh": …}` object the renderer resolves by language.
            let value = match props.value.as_deref() {
                Some(v) => Value::String(v.to_string()),
                None => localized_to_json_value(content),
            };
            let mut obj = json!({
                "type": "text",
                "value": value,
            });
            merge_props(&mut obj, props);
            obj
        }

        // ── Button → text + interactive + onClick ──────────────────────
        UiComponent::Button { label, props, .. } => {
            let mut obj = json!({
                "type": "text",
                "value": localized_to_json_value(label),
                "interactive": true,
            });
            if let Some(ref handler) = props.on_click {
                obj["onClick"] = json!(handler);
            }
            merge_props(&mut obj, props);
            obj
        }

        // ── List ───────────────────────────────────────────────────────
        UiComponent::List {
            source,
            format,
            props,
            ..
        } => {
            // `list` maps to the renderer's single-column `list` element
            // (`ListParams`); `flex_list` is the multi-column variant below.
            let mut obj = json!({
                "type": "list",
                "items": expr_to_json(source),
            });
            if let Some(ref fmt) = format {
                obj["format"] = json!(fmt);
            }
            merge_props(&mut obj, props);
            obj
        }

        // ── Image ──────────────────────────────────────────────────────
        UiComponent::Image { src, props, .. } => {
            let mut obj = json!({
                "type": "image",
                "src": src,
            });
            if let Some(slice) = props.custom.get("slice") {
                obj["nineSlice"] = expr_to_json(slice);
            }
            merge_props(&mut obj, props);
            if props.custom.contains_key("slice") {
                obj.as_object_mut().unwrap().remove("slice");
            }
            obj
        }

        // ── Input → custom:input ───────────────────────────────────────
        UiComponent::Input { props, .. } => {
            let mut obj = json!({ "type": "custom:input" });
            merge_props(&mut obj, props);
            obj
        }

        // ── Dropdown → custom:dropdown ─────────────────────────────────
        UiComponent::Dropdown { props, .. } => {
            let mut obj = json!({ "type": "custom:dropdown" });
            merge_props(&mut obj, props);
            obj
        }

        // ── Tile → tile ────────────────────────────────────────────────
        UiComponent::Tile { tile_id, props, .. } => {
            let mut obj = json!({
                "type": "tile",
                "tile_id": expr_to_json(tile_id),
            });
            merge_props(&mut obj, props);
            if props.custom.contains_key("tile_id") {
                obj.as_object_mut().unwrap().remove("tile_id");
            }
            obj
        }

        // ── Divider → divider ──────────────────────────────────────────
        UiComponent::Divider { tiles, props, .. } => {
            let mut obj = json!({ "type": "divider" });
            if !tiles.is_empty() {
                obj["tiles"] = Value::Array(tiles.iter().map(expr_to_json).collect());
            }
            merge_props(&mut obj, props);
            if props.custom.contains_key("tiles") {
                obj.as_object_mut().unwrap().remove("tiles");
            }
            obj
        }

        // ── Cursor → cursor (selection arrow) ──────────────────────────
        UiComponent::Cursor { props, .. } => {
            let mut obj = json!({ "type": "cursor" });
            merge_props(&mut obj, props);
            obj
        }

        // ── Primitives: bracket / pixel_rect ───────────────────────────
        UiComponent::Bracket { props, .. } => {
            let mut obj = json!({ "type": "bracket" });
            merge_props(&mut obj, props);
            obj
        }
        UiComponent::PixelRect { props, .. } => {
            let mut obj = json!({ "type": "pixel_rect" });
            merge_props(&mut obj, props);
            obj
        }

        // ── Declared custom component → custom:<name> ──────────────────
        // Dispatched at runtime to the game's registered `CustomElement`.
        UiComponent::Custom { name, props, .. } => {
            let mut obj = json!({ "type": format!("custom:{name}") });
            merge_props(&mut obj, props);
            obj
        }

        // ── FlexList → flex_list ───────────────────────────────────────
        UiComponent::FlexList {
            source,
            format,
            props,
            ..
        } => {
            // The renderer's `FlexListParams` reads the data binding from
            // `items` (same as `list`); emitting `source` left it without the
            // required field and it fell through to `Custom` (blank list).
            let mut obj = json!({
                "type": "flex_list",
                "items": expr_to_json(source),
            });
            if let Some(ref fmt) = format {
                obj["format"] = json!(fmt);
            }
            merge_props(&mut obj, props);
            obj
        }
    }
}

/// Merge layout / interaction props from a `ComponentProps` into an existing JSON object.
///
/// Only non-`None` values are emitted.  Property names follow the renderer convention
/// (camelCase for JS-side keys, lowercase for layout tokens).
fn merge_props(obj: &mut Value, props: &ComponentProps) {
    if let Some(ref id) = props.id {
        obj["id"] = json!(id);
    }
    if let Some(ref w) = props.width {
        obj["width"] = expr_to_json(w);
    }
    if let Some(ref h) = props.height {
        obj["height"] = expr_to_json(h);
    }
    if let Some(ref padding) = props.padding {
        obj["padding"] = compile_edge_values(padding);
    }
    if let Some(ref margin) = props.margin {
        obj["margin"] = compile_edge_values(margin);
    }
    if let Some(ref align) = props.align {
        obj["align"] = json!(align);
    }
    if let Some(grow) = props.flex_grow {
        obj["flexGrow"] = json!(grow);
    }
    if let Some(vis) = props.visible {
        obj["visible"] = json!(vis);
    }

    // Pokered-specific layout properties
    if let Some(ref rect) = props.rect {
        obj["rect"] = json!({
            "tx": expr_to_json(&rect.tx),
            "ty": expr_to_json(&rect.ty),
            "tw": expr_to_json(&rect.tw),
            "th": expr_to_json(&rect.th),
        });
    }
    if let Some(ref style) = props.style {
        obj["style"] = json!(style);
    }
    if let Some(ref color) = props.color {
        obj["color"] = json!(color);
    }
    if let Some(ref font) = props.font {
        obj["font"] = json!(font);
    }
    if let Some(ref wrap) = props.wrap {
        obj["wrap"] = json!(wrap);
    }
    if let Some(ls) = props.line_spacing {
        obj["line_spacing"] = json!(ls);
    }
    if let Some(sc) = props.scale {
        obj["scale"] = json!(sc);
    }
    if let Some(ref tile_id) = props.tile_id {
        obj["tile_id"] = expr_to_json(tile_id);
    }
    if let Some(ref tiles) = props.tiles {
        obj["tiles"] = Value::Array(tiles.iter().map(expr_to_json).collect());
    }
    if let Some(rpt) = props.repeat {
        obj["repeat"] = json!(rpt);
    }
    if let Some(ref orientation) = props.orientation {
        obj["orientation"] = json!(orientation);
    }
    if let Some(ref cursor) = props.cursor {
        obj["cursor"] = expr_to_json(cursor);
    }
    if let Some(ref selected) = props.selected {
        obj["selected"] = expr_to_json(selected);
    }
    if let Some(mv) = props.max_visible {
        obj["max_visible"] = json!(mv);
    }
    if let Some(ref footer) = props.footer {
        obj["footer"] = json!(footer);
    }
    if let Some(ref item_template) = props.item_template {
        obj["item_template"] = expr_to_json(item_template);
    }
    if let Some(ref item_layout) = props.item_layout {
        obj["item_layout"] = Value::Array(item_layout.iter().map(expr_to_json).collect());
    }
    if let Some(gap) = props.gap {
        obj["gap"] = json!(gap);
    }
    if let Some(clip) = props.clip {
        obj["clip"] = json!(clip);
    }
    if let Some(flip_x) = props.flip_x {
        obj["flip_x"] = json!(flip_x);
    }
    if let Some(flip_y) = props.flip_y {
        obj["flip_y"] = json!(flip_y);
    }
    if let Some(ref palette) = props.palette {
        obj["palette"] = json!(palette);
    }

    // Custom / extension properties (e.g. nine-slice, placeholder, …)
    for (key, val) in &props.custom {
        obj[key] = expr_to_json(val);
    }
}

/// Convert an edge-value list (`[top, right, bottom, left]` or a single uniform value)
/// into the JSON representation expected by the renderer.
fn compile_edge_values(values: &[Expression]) -> Value {
    if values.len() == 1 {
        expr_to_json(&values[0])
    } else {
        Value::Array(values.iter().map(expr_to_json).collect())
    }
}

/// Convert an AST `Expression` into its closest JSON-value equivalent.
///
/// - Literals → their JSON counterparts.
/// - `Variable("foo")` → `"{foo}"`  (binding string).
/// - Compound expressions (`BinaryOp`, `TernaryOp`) → `"{…}"` binding string.
pub fn expr_to_json(expr: &Expression) -> Value {
    match expr {
        Expression::StringLit(s) => Value::String(s.clone()),
        Expression::Localized(pairs) => {
            let map: serde_json::Map<String, Value> = pairs
                .iter()
                .map(|(locale, t)| (locale.clone(), Value::String(t.clone())))
                .collect();
            Value::Object(map)
        }
        Expression::NumberLit(n) => {
            // Preserve integer-ness when possible so JSON doesn't get spurious `.0`
            if n.fract() == 0.0 && n.is_finite() {
                json!(*n as i64)
            } else {
                json!(n)
            }
        }
        Expression::BoolLit(b) => Value::Bool(*b),
        Expression::Variable(v) => Value::String(format!("{{{v}}}")),
        Expression::BinaryOp { op, left, right } => {
            let left_str = expr_to_template_str(left);
            let right_str = expr_to_template_str(right);
            let op_str = binop_str(*op);
            Value::String(format!("{{{left_str} {op_str} {right_str}}}"))
        }
        Expression::TernaryOp {
            condition,
            then_expr,
            else_expr,
        } => {
            let cond_str = expr_to_template_str(condition);
            let then_str = expr_to_template_str(then_expr);
            let else_str = expr_to_template_str(else_expr);
            Value::String(format!("{{{cond_str} ? {then_str} : {else_str}}}"))
        }
        Expression::Call { callee, args } => {
            let args_str: Vec<String> = args.iter().map(expr_to_template_str).collect();
            Value::String(format!("{{{callee}({})}}", args_str.join(", ")))
        }
        Expression::ArrayLit(elements) => {
            let parts: Vec<String> = elements.iter().map(expr_to_template_str).collect();
            Value::String(format!("[{}]", parts.join(", ")))
        }
        Expression::UnaryOp { op, operand } => {
            let inner = expr_to_template_str(operand);
            let sym = match op {
                UnaryOp::Not => "!",
                UnaryOp::Neg => "-",
            };
            Value::String(format!("{{{sym}{inner}}}"))
        }
        Expression::ObjectLit(fields) => {
            let obj: serde_json::Map<String, Value> = fields
                .iter()
                .map(|(k, v)| (k.clone(), expr_to_json(v)))
                .collect();
            Value::Object(obj)
        }
    }
}

/// Render an expression as a template-string fragment (no wrapping braces).
fn expr_to_template_str(expr: &Expression) -> String {
    match expr {
        Expression::StringLit(s) => format!("\"{s}\""),
        // `@t(...)` in a template position is unusual; fall back to the base
        // (`en`) text so the binding string stays well-formed.
        Expression::Localized(pairs) => {
            format!("\"{}\"", crate::codegen::i18n::locale_text(pairs, "en"))
        }
        Expression::NumberLit(n) => {
            if n.fract() == 0.0 && n.is_finite() {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        Expression::BoolLit(b) => b.to_string(),
        Expression::Variable(v) => v.clone(),
        Expression::BinaryOp { op, left, right } => {
            let left_str = expr_to_template_str(left);
            let right_str = expr_to_template_str(right);
            format!("{} {} {}", left_str, binop_str(*op), right_str)
        }
        Expression::TernaryOp {
            condition,
            then_expr,
            else_expr,
        } => {
            let cond_str = expr_to_template_str(condition);
            let then_str = expr_to_template_str(then_expr);
            let else_str = expr_to_template_str(else_expr);
            format!("{cond_str} ? {then_str} : {else_str}")
        }
        Expression::Call { callee, args } => {
            let args_str: Vec<String> = args.iter().map(expr_to_template_str).collect();
            format!("{}({})", callee, args_str.join(", "))
        }
        Expression::ArrayLit(elements) => {
            let parts: Vec<String> = elements.iter().map(expr_to_template_str).collect();
            format!("[{}]", parts.join(", "))
        }
        Expression::UnaryOp { op, operand } => {
            let inner = expr_to_template_str(operand);
            let sym = match op {
                UnaryOp::Not => "!",
                UnaryOp::Neg => "-",
            };
            format!("{sym}{inner}")
        }
        Expression::ObjectLit(fields) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{k}: {}", expr_to_template_str(v)))
                .collect();
            format!("{{ {} }}", parts.join(", "))
        }
    }
}

fn binop_str(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Eq => "==",
        BinOp::Neq => "!=",
        BinOp::Gt => ">",
        BinOp::Lt => "<",
        BinOp::Gte => ">=",
        BinOp::Lte => "<=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Helper: create a minimal `ComponentProps` with the given span.
    fn props_with(
        id: Option<&str>,
        width: Option<Expression>,
        height: Option<Expression>,
        padding: Option<Vec<Expression>>,
        margin: Option<Vec<Expression>>,
        align: Option<&str>,
        on_click: Option<&str>,
        flex_grow: Option<u32>,
        visible: Option<bool>,
        custom: HashMap<String, Expression>,
    ) -> ComponentProps {
        ComponentProps {
            id: id.map(|s| s.to_string()),
            width,
            height,
            padding,
            margin,
            align: align.map(|s| s.to_string()),
            on_click: on_click.map(|s| s.to_string()),
            flex_grow,
            visible,
            custom,
            span: SourceSpan::point("test", 1, 1),
            rect: None,
            style: None,
            value: None,
            color: None,
            font: None,
            wrap: None,
            line_spacing: None,
            scale: None,
            tile_id: None,
            tiles: None,
            repeat: None,
            orientation: None,
            cursor: None,
            selected: None,
            max_visible: None,
            footer: None,
            item_template: None,
            item_layout: None,
            gap: None,
            clip: None,
            flip_x: None,
            flip_y: None,
            palette: None,
        }
    }

    fn default_props() -> ComponentProps {
        ComponentProps {
            id: None,
            width: None,
            height: None,
            padding: None,
            margin: None,
            align: None,
            on_click: None,
            flex_grow: None,
            visible: None,
            custom: HashMap::new(),
            span: SourceSpan::point("test", 1, 1),
            rect: None,
            style: None,
            value: None,
            color: None,
            font: None,
            wrap: None,
            line_spacing: None,
            scale: None,
            tile_id: None,
            tiles: None,
            repeat: None,
            orientation: None,
            cursor: None,
            selected: None,
            max_visible: None,
            footer: None,
            item_template: None,
            item_layout: None,
            gap: None,
            clip: None,
            flip_x: None,
            flip_y: None,
            palette: None,
        }
    }

    /// Assert that `json_str` parses as valid JSON and matches `expected` structurally.
    fn assert_json_eq(json_str: &Result<String, String>, expected: &Value) {
        let s = json_str.as_ref().expect("compile_ui should succeed");
        let parsed: Value = serde_json::from_str(s).expect("output must be valid JSON");
        assert_eq!(
            parsed, *expected,
            "JSON mismatch.\nExpected: {expected}\nGot:      {parsed}"
        );
    }

    // ── expr_to_json ────────────────────────────────────────────────────────

    #[test]
    fn expr_string_lit() {
        let v = expr_to_json(&Expression::StringLit("hello".into()));
        assert_eq!(v, json!("hello"));
    }

    #[test]
    fn expr_number_lit_integer() {
        let v = expr_to_json(&Expression::NumberLit(42.0));
        assert_eq!(v, json!(42));
    }

    #[test]
    fn expr_number_lit_float() {
        let v = expr_to_json(&Expression::NumberLit(3.14));
        assert_eq!(v, json!(3.14));
    }

    #[test]
    fn expr_bool_lit() {
        let v = expr_to_json(&Expression::BoolLit(true));
        assert_eq!(v, json!(true));
    }

    #[test]
    fn expr_variable() {
        let v = expr_to_json(&Expression::Variable("count".into()));
        assert_eq!(v, json!("{count}"));
    }

    #[test]
    fn expr_binary_op() {
        let v = expr_to_json(&Expression::BinaryOp {
            op: BinOp::Add,
            left: Box::new(Expression::Variable("a".into())),
            right: Box::new(Expression::NumberLit(1.0)),
        });
        assert_eq!(v, json!("{a + 1}"));
    }

    #[test]
    fn expr_ternary_op() {
        let v = expr_to_json(&Expression::TernaryOp {
            condition: Box::new(Expression::Variable("visible".into())),
            then_expr: Box::new(Expression::NumberLit(10.0)),
            else_expr: Box::new(Expression::NumberLit(0.0)),
        });
        assert_eq!(v, json!("{visible ? 10 : 0}"));
    }

    // ── Component compilation ───────────────────────────────────────────────

    #[test]
    fn test_panel_with_text_child() {
        let text = UiComponent::Text {
            content: "Hello".into(),
            props: default_props(),
            span: SourceSpan::point("test", 2, 3),
        };
        let panel = UiComponent::Panel {
            props: default_props(),
            children: vec![text],
            span: SourceSpan::point("test", 1, 1),
        };
        let v = compile_component(&panel);
        let expected = json!({
            "type": "border",
            "children": [
                { "type": "text", "value": "Hello" }
            ]
        });
        assert_eq!(v, expected);
    }

    #[test]
    fn test_container_with_multiple_children() {
        let text = UiComponent::Text {
            content: "Label".into(),
            props: default_props(),
            span: SourceSpan::point("test", 2, 1),
        };
        let button = UiComponent::Button {
            label: "OK".into(),
            props: props_with(
                None,
                None,
                None,
                None,
                None,
                None,
                Some("on_ok"),
                None,
                None,
                HashMap::new(),
            ),
            span: SourceSpan::point("test", 3, 1),
        };
        let container = UiComponent::Container {
            props: default_props(),
            children: vec![text, button],
            span: SourceSpan::point("test", 1, 1),
        };
        let v = compile_component(&container);
        let expected = json!({
            "type": "group",
            "children": [
                { "type": "text", "value": "Label" },
                { "type": "text", "value": "OK", "interactive": true, "onClick": "on_ok" }
            ]
        });
        assert_eq!(v, expected);
    }

    #[test]
    fn test_button_with_on_click() {
        let button = UiComponent::Button {
            label: "Buy".into(),
            props: props_with(
                None,
                None,
                None,
                None,
                None,
                None,
                Some("buy_item"),
                None,
                None,
                HashMap::new(),
            ),
            span: SourceSpan::point("test", 1, 1),
        };
        let v = compile_component(&button);
        let expected = json!({
            "type": "text",
            "value": "Buy",
            "interactive": true,
            "onClick": "buy_item"
        });
        assert_eq!(v, expected);
    }

    #[test]
    fn test_list_component() {
        let list = UiComponent::List {
            source: Expression::Variable("shop_items".into()),
            format: Some("{icon} {name} - ${price}".into()),
            props: default_props(),
            span: SourceSpan::point("test", 1, 1),
        };
        let v = compile_component(&list);
        let expected = json!({
            "type": "list",
            "items": "{shop_items}",
            "format": "{icon} {name} - ${price}"
        });
        assert_eq!(v, expected);
    }

    #[test]
    fn test_image_with_slice() {
        let mut custom = HashMap::new();
        custom.insert("slice".into(), Expression::StringLit("[8,8,8,8]".into()));
        let image = UiComponent::Image {
            src: "ui/panel.png".into(),
            props: props_with(None, None, None, None, None, None, None, None, None, custom),
            span: SourceSpan::point("test", 1, 1),
        };
        let v = compile_component(&image);
        let expected = json!({
            "type": "image",
            "src": "ui/panel.png",
            "nineSlice": "[8,8,8,8]"
        });
        assert_eq!(v, expected);
    }

    #[test]
    fn test_component_with_layout_props() {
        let text = UiComponent::Text {
            content: "Styled".into(),
            props: props_with(
                Some("my_text"),
                Some(Expression::NumberLit(200.0)),
                Some(Expression::StringLit("auto".into())),
                Some(vec![
                    Expression::NumberLit(8.0),
                    Expression::NumberLit(12.0),
                    Expression::NumberLit(8.0),
                    Expression::NumberLit(12.0),
                ]),
                Some(vec![Expression::NumberLit(4.0)]),
                Some("center"),
                None,
                Some(1),
                Some(true),
                HashMap::new(),
            ),
            span: SourceSpan::point("test", 1, 1),
        };
        let v = compile_component(&text);
        let expected = json!({
            "type": "text",
            "value": "Styled",
            "id": "my_text",
            "width": 200,
            "height": "auto",
            "padding": [8, 12, 8, 12],
            "margin": 4,
            "align": "center",
            "flexGrow": 1,
            "visible": true
        });
        assert_eq!(v, expected);
    }

    #[test]
    fn test_full_ui_block() {
        let mut custom = HashMap::new();
        custom.insert("slice".into(), Expression::StringLit("[8,8,8,8]".into()));

        let ui = UiBlock {
            components: vec![
                // Panel wrapping a button
                UiComponent::Panel {
                    props: props_with(
                        Some("main_panel"),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        HashMap::new(),
                    ),
                    children: vec![
                        UiComponent::Text {
                            content: "Title".into(),
                            props: default_props(),
                            span: SourceSpan::point("test", 2, 1),
                        },
                        UiComponent::Button {
                            label: "Close".into(),
                            props: props_with(
                                None,
                                None,
                                None,
                                None,
                                None,
                                None,
                                Some("close_screen"),
                                None,
                                None,
                                HashMap::new(),
                            ),
                            span: SourceSpan::point("test", 3, 1),
                        },
                    ],
                    span: SourceSpan::point("test", 1, 1),
                },
                // Standalone image
                UiComponent::Image {
                    src: "bg.png".into(),
                    props: props_with(None, None, None, None, None, None, None, None, None, custom),
                    span: SourceSpan::point("test", 5, 1),
                },
            ],
            span: SourceSpan::point("test", 0, 0),
        };

        let expected = json!({
            "type": "group",
            "children": [
                {
                    "type": "border",
                    "id": "main_panel",
                    "children": [
                        { "type": "text", "value": "Title" },
                        { "type": "text", "value": "Close", "interactive": true, "onClick": "close_screen" }
                    ]
                },
                {
                    "type": "image",
                    "src": "bg.png",
                    "nineSlice": "[8,8,8,8]"
                }
            ]
        });

        assert_json_eq(&compile_ui(&ui), &expected);
    }

    // ── JSON validity ───────────────────────────────────────────────────────

    #[test]
    fn generated_json_is_valid() {
        let ui = UiBlock {
            components: vec![
                UiComponent::Text {
                    content: "hello".into(),
                    props: default_props(),
                    span: SourceSpan::point("test", 1, 1),
                },
                UiComponent::Container {
                    props: default_props(),
                    children: vec![UiComponent::Button {
                        label: "ok".into(),
                        props: props_with(
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            Some("handle"),
                            None,
                            None,
                            HashMap::new(),
                        ),
                        span: SourceSpan::point("test", 3, 1),
                    }],
                    span: SourceSpan::point("test", 2, 1),
                },
            ],
            span: SourceSpan::point("test", 0, 0),
        };
        let result = compile_ui(&ui);
        assert!(result.is_ok(), "compile_ui should succeed");
        let s = result.unwrap();
        let parsed: Value = serde_json::from_str(&s).expect("must be valid JSON");
        assert!(parsed.is_object());
        assert_eq!(parsed["type"], "group");
        assert!(parsed["children"].is_array());
    }

    #[test]
    fn input_and_dropdown_custom_types() {
        let input = UiComponent::Input {
            props: props_with(
                Some("name_input"),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                HashMap::new(),
            ),
            span: SourceSpan::point("test", 1, 1),
        };
        let dropdown = UiComponent::Dropdown {
            props: props_with(
                Some("lang_select"),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                HashMap::new(),
            ),
            span: SourceSpan::point("test", 2, 1),
        };

        assert_eq!(
            compile_component(&input),
            json!({
                "type": "custom:input",
                "id": "name_input"
            })
        );
        assert_eq!(
            compile_component(&dropdown),
            json!({
                "type": "custom:dropdown",
                "id": "lang_select"
            })
        );
    }

    // ── Pokered-specific codegen tests ────────────────────────────────

    fn pokered_props() -> ComponentProps {
        ComponentProps {
            id: None,
            width: None,
            height: None,
            padding: None,
            margin: None,
            align: None,
            on_click: None,
            flex_grow: None,
            visible: None,
            custom: HashMap::new(),
            span: SourceSpan::point("test", 1, 1),
            rect: None,
            style: None,
            value: None,
            color: None,
            font: None,
            wrap: None,
            line_spacing: None,
            scale: None,
            tile_id: None,
            tiles: None,
            repeat: None,
            orientation: None,
            cursor: None,
            selected: None,
            max_visible: None,
            footer: None,
            item_template: None,
            item_layout: None,
            gap: None,
            clip: None,
            flip_x: None,
            flip_y: None,
            palette: None,
        }
    }

    #[test]
    fn test_tile_component() {
        let tile = UiComponent::Tile {
            tile_id: Expression::NumberLit(31.0),
            props: {
                let mut p = pokered_props();
                p.rect = Some(RectDef {
                    tx: Expression::NumberLit(18.0),
                    ty: Expression::NumberLit(16.0),
                    tw: Expression::NumberLit(1.0),
                    th: Expression::NumberLit(1.0),
                    span: SourceSpan::point("test", 1, 1),
                });
                p
            },
            span: SourceSpan::point("test", 1, 1),
        };
        let v = compile_component(&tile);
        let expected = json!({
            "type": "tile",
            "tile_id": 31,
            "rect": { "tx": 18, "ty": 16, "tw": 1, "th": 1 }
        });
        assert_eq!(v, expected);
    }

    #[test]
    fn test_divider_component() {
        let divider = UiComponent::Divider {
            tiles: vec![Expression::NumberLit(122.0)],
            props: {
                let mut p = pokered_props();
                p.repeat = Some(17);
                p.orientation = Some("horizontal".into());
                p
            },
            span: SourceSpan::point("test", 1, 1),
        };
        let v = compile_component(&divider);
        let expected = json!({
            "type": "divider",
            "tiles": [122],
            "repeat": 17,
            "orientation": "horizontal"
        });
        assert_eq!(v, expected);
    }

    #[test]
    fn test_flex_list_component() {
        let flex = UiComponent::FlexList {
            source: Expression::Variable("monster_list".into()),
            format: None,
            props: {
                let mut p = pokered_props();
                p.cursor = Some(Expression::ObjectLit(vec![
                    ("tile".into(), Expression::NumberLit(223.0)),
                    ("position".into(), Expression::StringLit("left".into())),
                ]));
                p.max_visible = Some(4);
                p.gap = Some(1);
                p
            },
            span: SourceSpan::point("test", 1, 1),
        };
        let v = compile_component(&flex);
        let expected = json!({
            "type": "flex_list",
            "items": "{monster_list}",
            "cursor": { "tile": 223, "position": "left" },
            "max_visible": 4,
            "gap": 1
        });
        assert_eq!(v, expected);
    }

    #[test]
    fn test_text_localized_emits_value_object() {
        // `@t("CANCEL", "取消")` → `"value": {"en": "CANCEL", "zh": "取消"}`.
        let text = UiComponent::Text {
            content: LocalizedText::Localized(vec![
                ("en".into(), "CANCEL".into()),
                ("zh".into(), "取消".into()),
            ]),
            props: pokered_props(),
            span: SourceSpan::point("test", 1, 1),
        };
        let v = compile_component(&text);
        assert_eq!(v["type"], "text");
        assert_eq!(v["value"], json!({ "en": "CANCEL", "zh": "取消" }));
    }

    #[test]
    fn test_button_localized_emits_value_object() {
        let button = UiComponent::Button {
            label: LocalizedText::Localized(vec![
                ("en".into(), "BUY".into()),
                ("zh".into(), "购买".into()),
            ]),
            props: pokered_props(),
            span: SourceSpan::point("test", 1, 1),
        };
        let v = compile_component(&button);
        assert_eq!(v["value"], json!({ "en": "BUY", "zh": "购买" }));
        assert_eq!(v["interactive"], true);
    }

    #[test]
    fn test_text_plain_still_emits_string() {
        // Non-localized text is unchanged: a plain JSON string.
        let text = UiComponent::Text {
            content: LocalizedText::Plain("HELLO".into()),
            props: pokered_props(),
            span: SourceSpan::point("test", 1, 1),
        };
        let v = compile_component(&text);
        assert_eq!(v["value"], json!("HELLO"));
    }

    #[test]
    fn test_text_with_value_and_wrap() {
        let text = UiComponent::Text {
            content: "Hello".into(),
            props: {
                let mut p = pokered_props();
                p.value = Some("{text}".into());
                p.wrap = Some("word".into());
                p.color = Some("Black".into());
                p.font = Some("Monster".into());
                p
            },
            span: SourceSpan::point("test", 1, 1),
        };
        let v = compile_component(&text);
        let expected = json!({
            "type": "text",
            "value": "{text}",
            "wrap": "word",
            "color": "Black",
            "font": "Monster"
        });
        assert_eq!(v, expected);
    }

    #[test]
    fn test_panel_with_style_and_rect() {
        let panel = UiComponent::Panel {
            props: {
                let mut p = pokered_props();
                p.style = Some("default".into());
                p.rect = Some(RectDef {
                    tx: Expression::NumberLit(0.0),
                    ty: Expression::NumberLit(12.0),
                    tw: Expression::NumberLit(20.0),
                    th: Expression::NumberLit(6.0),
                    span: SourceSpan::point("test", 1, 1),
                });
                p
            },
            children: vec![],
            span: SourceSpan::point("test", 1, 1),
        };
        let v = compile_component(&panel);
        let expected = json!({
            "type": "border",
            "style": "default",
            "rect": { "tx": 0, "ty": 12, "tw": 20, "th": 6 }
        });
        assert_eq!(v, expected);
    }

    #[test]
    fn test_compile_screen_schema_v2() {
        let screen = ScreenLayout {
            name: "dialog".into(),
            theme: None,
            components: vec![
                UiComponent::Panel {
                    props: {
                        let mut p = pokered_props();
                        p.style = Some("default".into());
                        p.rect = Some(RectDef {
                            tx: Expression::NumberLit(0.0),
                            ty: Expression::NumberLit(12.0),
                            tw: Expression::NumberLit(20.0),
                            th: Expression::NumberLit(6.0),
                            span: SourceSpan::point("test", 1, 1),
                        });
                        p
                    },
                    children: vec![],
                    span: SourceSpan::point("test", 1, 1),
                },
                UiComponent::Text {
                    content: "{text}".into(),
                    props: {
                        let mut p = pokered_props();
                        p.value = Some("{text}".into());
                        p.wrap = Some("word".into());
                        p.rect = Some(RectDef {
                            tx: Expression::NumberLit(1.0),
                            ty: Expression::NumberLit(13.0),
                            tw: Expression::NumberLit(18.0),
                            th: Expression::NumberLit(4.0),
                            span: SourceSpan::point("test", 1, 1),
                        });
                        p
                    },
                    span: SourceSpan::point("test", 2, 1),
                },
                UiComponent::Tile {
                    tile_id: Expression::NumberLit(31.0),
                    props: {
                        let mut p = pokered_props();
                        p.rect = Some(RectDef {
                            tx: Expression::NumberLit(18.0),
                            ty: Expression::NumberLit(16.0),
                            tw: Expression::NumberLit(1.0),
                            th: Expression::NumberLit(1.0),
                            span: SourceSpan::point("test", 1, 1),
                        });
                        p
                    },
                    span: SourceSpan::point("test", 3, 1),
                },
            ],
            schema_version: Some(2),
            span: SourceSpan::point("test", 0, 0),
        };

        let result = compile_screen(&screen);
        assert!(
            result.is_ok(),
            "compile_screen should succeed: {:?}",
            result.err()
        );
        let json_str = result.unwrap();
        let parsed: Value = serde_json::from_str(&json_str).expect("must be valid JSON");

        assert_eq!(parsed["schema_version"], 2);
        assert_eq!(parsed["screen"], "dialog");
        let elements = parsed["elements"]
            .as_array()
            .expect("elements should be an array");
        assert_eq!(elements.len(), 3);
        assert_eq!(elements[0]["type"], "border");
        assert_eq!(elements[0]["style"], "default");
        assert_eq!(elements[1]["type"], "text");
        assert_eq!(elements[1]["wrap"], "word");
        assert_eq!(elements[2]["type"], "tile");
    }

    #[test]
    fn test_expr_to_json_object_lit() {
        let expr = Expression::ObjectLit(vec![
            ("tile".into(), Expression::NumberLit(223.0)),
            ("position".into(), Expression::StringLit("left".into())),
        ]);
        let v = expr_to_json(&expr);
        let expected = json!({ "tile": 223, "position": "left" });
        assert_eq!(v, expected);
    }
}
