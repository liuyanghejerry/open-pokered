//! Lower schema-v2 GUI output to a borrowed Rust layout expression.
//!
//! This intentionally supports a checked subset: flat default panels, unwrapped
//! text, literal tiles and grid cursors. Unsupported properties are build errors,
//! never silently discarded. The input is the normal GUI compiler's JSON output,
//! including expanded components. No game names or coordinates belong here.
use serde_json::Value;

fn fields(v: &Value, allowed: &[&str]) -> Result<(), String> {
    let obj = v.as_object().ok_or("expected object")?;
    for key in obj.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("unsupported static UI property: {key}"));
        }
    }
    Ok(())
}
fn binding(s: &str) -> Result<&str, String> {
    let key = s
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .ok_or("expected {binding}")?;
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!("unsupported static UI binding: {s}"));
    }
    Ok(key)
}
fn number(v: &Value) -> Result<u32, String> {
    v.as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| format!("expected u32: {v}"))
}
fn string_field<'a>(value: &'a Value, key: &str, default: &'a str) -> Result<&'a str, String> {
    match value.get(key) {
        None => Ok(default),
        Some(Value::String(s)) => Ok(s),
        Some(_) => Err(format!("{key} must be a string")),
    }
}
fn coord(v: Option<&Value>, default: u32) -> Result<String, String> {
    match v {
        None => Ok(format!("Coord::Literal({default})")),
        Some(Value::String(s)) => Ok(format!("Coord::Binding({:?})", binding(s)?)),
        Some(v) => Ok(format!("Coord::Literal({})", number(v)?)),
    }
}
fn text(v: &Value) -> Result<String, String> {
    match v {
        Value::String(s) if s.contains('{') || s.contains('}') => {
            Ok(format!("Text::Binding({:?})", binding(s)?))
        }
        Value::String(s) => Ok(format!("Text::Literal({s:?})")),
        Value::Object(entries) if !entries.is_empty() => {
            let mut out = String::from("Text::Localized(&[");
            for (locale, value) in entries {
                if !value.is_string() {
                    return Err("localized text must contain strings".into());
                }
                out.push_str(&format!("({locale:?}, {}),", text(value)?));
            }
            out.push_str("])");
            Ok(out)
        }
        _ => Err("expected literal, binding or localized text".into()),
    }
}
/// Emit a Rust expression of type `dotzuki_renderer::layout_engine::static_layout::Layout`.
/// Fail the caller's build with the returned diagnostic when a layout exceeds
/// the supported subset; the dynamic layout engine remains available for editors.
pub fn compile(json: &str) -> Result<String, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    fields(&root, &["schema_version", "screen", "elements"])?;
    if root["schema_version"] != 2 {
        return Err("static UI requires schema_version 2".into());
    }
    let mut elements: Vec<_> = root["elements"]
        .as_array()
        .ok_or("expected elements")?
        .iter()
        .collect();
    for e in &elements {
        if let Some(z) = e.get("z_index") {
            if z.as_i64().is_none() {
                return Err("z_index must be an integer".into());
            }
        }
    }
    elements.sort_by_key(|e| e.get("z_index").and_then(Value::as_i64).unwrap_or(0));
    let mut out = String::from("{ use dotzuki_renderer::layout_engine::static_layout::{Layout, Element, Coord, Text, Visible, Op}; use dotzuki_renderer::layout_engine::types::TextAlign; Layout { elements: &[\n");
    for (i, e) in elements.iter().enumerate() {
        let result =
            element(e).map_err(|err| format!("screen {} element {i}: {err}", root["screen"]))?;
        out.push_str(&result);
        out.push_str(",\n");
    }
    out.push_str("] } }");
    Ok(out)
}
fn element(e: &Value) -> Result<String, String> {
    let kind = e["type"].as_str().ok_or("expected element type")?;
    let specific: &[&str] = match kind {
        "border" => &["style"],
        "text" => &["value", "align"],
        "tile" => &["tile_id"],
        "cursor" => &["glyph", "col", "row", "col_step", "row_step"],
        _ => return Err(format!("unsupported static UI element: {kind}")),
    };
    let mut allowed = vec!["type", "id", "rect", "visible", "z_index"];
    allowed.extend_from_slice(specific);
    fields(e, &allowed)?;
    let rect = &e["rect"];
    fields(rect, &["tx", "ty", "tw", "th"])?;
    let x = coord(rect.get("tx"), 0)?;
    let y = coord(rect.get("ty"), 0)?;
    let width = rect
        .get("tw")
        .map(number)
        .transpose()?
        .unwrap_or(if kind == "text" { 20 } else { 1 });
    let height = rect
        .get("th")
        .map(number)
        .transpose()?
        .unwrap_or(if kind == "text" { 18 } else { 1 });
    let visible = match e.get("visible") {
        None => "Visible::Always(true)".into(),
        Some(Value::Bool(b)) => format!("Visible::Always({b})"),
        Some(Value::String(s)) => format!("Visible::Binding({:?})", binding(s)?),
        _ => return Err("unsupported visibility".into()),
    };
    let op = match kind {
        "border" => {
            if e.get("style").is_some_and(|v| v != "default") {
                return Err("only default panel style is supported".into());
            }
            "Op::Border".into()
        }
        "text" => {
            let align = match string_field(e, "align", "left")? {
                "left" => "Left",
                "center" => "Center",
                "right" => "Right",
                _ => return Err("unsupported alignment".into()),
            };
            format!(
                "Op::Text {{ value: {}, align: TextAlign::{align} }}",
                text(&e["value"])?
            )
        }
        "tile" => {
            let id = u8::try_from(number(&e["tile_id"])?).map_err(|_| "tile id exceeds u8")?;
            format!("Op::Tile {{ id: {id}, fallback: \"[{id}]\" }}")
        }
        "cursor" => {
            let glyph = string_field(e, "glyph", "▶")?;
            let mut chars = glyph.chars();
            let glyph = chars.next().ok_or("empty cursor glyph")?;
            if chars.next().is_some() {
                return Err("cursor must be one glyph".into());
            }
            format!(
                "Op::Cursor {{ glyph: {glyph:?}, col: {}, row: {}, col_step: {}, row_step: {} }}",
                coord(e.get("col"), 0)?,
                coord(e.get("row"), 0)?,
                e.get("col_step").map(number).transpose()?.unwrap_or(0),
                e.get("row_step").map(number).transpose()?.unwrap_or(0)
            )
        }
        _ => unreachable!(),
    };
    Ok(format!("Element {{ x: {x}, y: {y}, width: {width}, height: {height}, visible: {visible}, op: {op} }}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_features_instead_of_silently_changing_the_layout() {
        for property in [r#""wrap":"word""#, r#""color":"red""#, r#""children":[]"#] {
            let json = format!(
                r#"{{"schema_version":2,"elements":[{{"type":"text","rect":{{}},"value":"hello",{property}}}]}}"#
            );
            assert!(compile(&json)
                .unwrap_err()
                .contains("unsupported static UI property"));
        }
        assert!(text(&serde_json::json!("HP: {hp}")).is_err());
        assert!(coord(Some(&serde_json::json!("{index%4}")), 0).is_err());
    }
    #[test]
    fn source_changes_flow_into_the_compiled_layout() {
        let source = r#"screen Example { text(@t("Hello", "你好")) { rect = {tx: 3, ty: 4, tw: 8, th: 1} } }"#;
        let tokens = crate::lexer::Lexer::new(source, "example.gui")
            .tokenize()
            .unwrap();
        let (doc, errors) = crate::parser::parse(tokens);
        assert!(errors.is_empty());
        let crate::ast::Document::Screen(screen) = doc.unwrap() else {
            panic!()
        };
        let json = crate::codegen::json_ui::compile_screen(&screen).unwrap();
        let rust = compile(&json).unwrap();
        assert!(rust.contains("Coord::Literal(3)"));
        assert!(rust.contains("你好"));
        let edited = json.replace("Hello", "Goodbye");
        assert!(compile(&edited).unwrap().contains("Goodbye"));
    }
}
