//! WASM layout-preview bridge for the pokered editor (`tools/pokered-editor`).
//!
//! This is the pokered-flavoured counterpart of the engine's generic
//! `dotzuki-web` preview: it renders compiled `.gui` layout JSON on the fixed
//! 160×144 Game Boy canvas with per-menu pokered mock data and registers
//! pokered's `custom:*` elements (see `preview_elements`), plus thin wrappers
//! around the `dotzuki-engine-dsl` compiler so the editor can validate `.scene`
//! / `.gui` sources inline. It was extracted from `dotzuki-web` during the
//! engine/game repo split — the engine crate now only ships the game-agnostic
//! `render_gui` path.

use std::collections::HashMap;

use wasm_bindgen::prelude::*;

use dotzuki_renderer::{FrameBuffer, RenderConfig};
use dotzuki_renderer::layout_engine::deserialize::{parse_layout, load_layout};
use dotzuki_renderer::layout_engine::types::{DataContext, RenderContext};
use dotzuki_renderer::layout_engine::renderer::render_layout as render_screen;

mod mock_data;
mod preview_elements;

use dotzuki_ui::FrameBufferPainter;
use dotzuki_engine::render::{Painter, Rgba};

/// Log a warning message (goes to stderr; in WASM this reaches the browser
/// console when using `wasm-bindgen` test runner or `console_log`).
fn log_warn(msg: &str) {
    eprintln!("[pokered-layout-preview] WARN: {}", msg);
}

/// Log an error message.
fn log_error(msg: &str) {
    eprintln!("[pokered-layout-preview] ERROR: {}", msg);
}

fn render_with<F>(draw_fn: F) -> Vec<u8>
where
    F: FnOnce(&mut FrameBufferPainter),
{
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    {
        let mut painter = FrameBufferPainter::new(&mut fb);
        painter.clear(Rgba::INK_WHITE);
        draw_fn(&mut painter);
    }
    fb.data
}

#[wasm_bindgen]
pub fn render_layout(menu_name: &str, layout_json: &str, mock_state_id: u32, lang: u32, overrides_json: &str) -> Vec<u8> {
    // 1. Try to parse the layout JSON string from the editor
    let layout = match parse_layout(layout_json) {
        Ok(l) => l,
        Err(e) => {
            log_warn(&format!(
                "parse_layout failed for '{}': {:?}. Falling back to file load.",
                menu_name, e
            ));
            // Fallback: try loading from file
            match load_layout(menu_name) {
                Ok(l) => l,
                Err(e) => {
                    log_error(&format!(
                        "load_layout also failed for '{}': {:?}. Returning empty preview.",
                        menu_name, e
                    ));
                    return Vec::new(); // empty = no preview
                }
            }
        }
    };

    // 2. Create DataContext with mock data
    let mut ctx = DataContext::new();
    mock_data::fill_mock_data(&mut ctx, menu_name, mock_state_id);

    // Select the active language for `@t("en", "中文")` text elements.
    // 0 = English, 1 = Chinese (matches the editor's language dropdown).
    let lang_code = if lang == 1 { "zh" } else { "en" };
    ctx.set("__lang", lang_code);

    // Merge user overrides from the editor
    if !overrides_json.is_empty() {
        if let Ok(overrides) = serde_json::from_str::<HashMap<String, serde_json::Value>>(overrides_json) {
            for (key, value) in overrides {
                match value {
                    serde_json::Value::String(s) => { ctx.set(&key, s.as_str()); }
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() { ctx.set(&key, i); }
                    }
                    serde_json::Value::Bool(b) => { ctx.set(&key, b); }
                    _ => { ctx.set(&key, value.to_string()); }
                }
            }
        }
    }

    // 3. Create RenderContext with default font/tileset registries
    let fonts: HashMap<String, ()> = HashMap::new();
    let tilesets: HashMap<String, ()> = HashMap::new();
    let render_ctx = RenderContext::new(menu_name, &layout.theme, &fonts, &tilesets);

    // 4. Render via the layout engine — do NOT silently swallow errors
    render_with(|painter| {
        let registry = preview_elements::preview_registry();
        if let Err(e) = render_screen(&layout, &ctx, &render_ctx, &registry, painter) {
            log_error(&format!(
                "render_screen failed for layout '{}': {:?}",
                menu_name, e
            ));
        }
    })
}

// ── DSL (.scene) compile bridge ───────────────────────────────────────────
//
// These exports wrap the `dotzuki-engine-dsl` compiler so the game-editor can
// validate `.scene` files inline (CM6 linter) and show a compiled-JS preview.
//
// They return a **JSON string** rather than a structured `JsValue` so we avoid
// pulling in `serde-wasm-bindgen` (not currently a workspace dependency). The
// TS side calls `JSON.parse` on the result. Shapes:
//   success: { "ok": true,  "js": "<compiled JS or JSON config>" }
//   failure: { "ok": false, "error": "<message>", "line": <n>, "col": <n> }

/// Fixed placeholder path used when compiling from the editor (no real file).
const EDITOR_SCENE_PATH: &str = "editor/script.scene";

/// Parse the leading `line:col:` prefix out of a compiler error string.
///
/// The DSL compiler formats lexer errors as `"<line>:<col>: <message>; ..."`
/// but parser/semantic errors carry no positional prefix. When no prefix is
/// present we fall back to line 1, col 1 so the diagnostic still anchors
/// somewhere sensible.
fn parse_error_location(err: &str) -> (u32, u32, String) {
    // Only consider the first error (segments are joined with "; ").
    let first = err.split("; ").next().unwrap_or(err);
    let mut parts = first.splitn(3, ':');
    if let (Some(l), Some(c), Some(rest)) = (parts.next(), parts.next(), parts.next()) {
        if let (Ok(line), Ok(col)) = (l.trim().parse::<u32>(), c.trim().parse::<u32>()) {
            return (line, col, rest.trim().to_string());
        }
    }
    (1, 1, err.to_string())
}

/// Build the JSON success payload `{ "ok": true, "<field>": "<output>" }`.
fn dsl_ok_json(field: &str, output: &str) -> String {
    serde_json::json!({ "ok": true, field: output }).to_string()
}

/// Build the JSON failure payload `{ "ok": false, "error", "line", "col" }`.
fn dsl_err_json(err: &str) -> String {
    let (line, col, message) = parse_error_location(err);
    serde_json::json!({
        "ok": false,
        "error": message,
        // Keep the full (possibly multi-error) message available too.
        "raw": err,
        "line": line,
        "col": col,
    })
    .to_string()
}

/// Compile `.scene` DSL source to JavaScript.
///
/// Returns a JSON string (parse with `JSON.parse`):
///   `{ ok: true, js: "<compiled JS>" }` on success
///   `{ ok: false, error, raw, line, col }` on failure
#[wasm_bindgen]
pub fn compile_scene(source: &str) -> String {
    match dotzuki_engine_dsl::compiler::compile_scene_to_js(source, EDITOR_SCENE_PATH) {
        Ok(js) => dsl_ok_json("js", &js),
        Err(e) => dsl_err_json(&e),
    }
}

/// Compile `.scene` DSL source to its `script_config.json` representation.
///
/// Returns a JSON string (parse with `JSON.parse`):
///   `{ ok: true, config: "<compiled JSON config>" }` on success
///   `{ ok: false, error, raw, line, col }` on failure
#[wasm_bindgen]
pub fn compile_scene_config(source: &str) -> String {
    match dotzuki_engine_dsl::config_gen::compile_scene_to_config(source, EDITOR_SCENE_PATH) {
        Ok(config) => dsl_ok_json("config", &config),
        Err(e) => dsl_err_json(&e),
    }
}

/// Compile `.gui` DSL source (screen layout) to v2 ScreenLayout JSON.
///
/// Returns a JSON string (parse with `JSON.parse`):
///   `{ ok: true, js: "<compiled JSON>" }` on success
///   `{ ok: false, error, raw, line, col }` on failure
#[wasm_bindgen]
pub fn compile_screen_source(source: &str) -> String {
    match compile_gui_inner(source) {
        Ok(json) => dsl_ok_json("js", &json),
        Err(e) => dsl_err_json(&e),
    }
}

/// Compile `.gui` DSL source → schema-v2 ScreenLayout JSON, or a `line:col: msg`
/// error string. Shared by [`compile_screen_source`] and the layout preview.
fn compile_gui_inner(source: &str) -> Result<String, String> {
    let tokens = dotzuki_engine_dsl::lexer::Lexer::new(source, "editor/screen.gui")
        .tokenize()
        .map_err(|errors| {
            errors
                .iter()
                .map(|e| format!("{}:{}: {}", e.line, e.col, e.message))
                .collect::<Vec<_>>()
                .join("; ")
        })?;

    let (doc, parse_errors) = dotzuki_engine_dsl::parser::Parser::new(tokens, source).parse();
    if !parse_errors.is_empty() {
        return Err(parse_errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; "));
    }

    match doc.ok_or_else(|| "parser returned no document".to_string())? {
        dotzuki_engine_dsl::ast::Document::Screen(screen) => {
            dotzuki_engine_dsl::codegen::json_ui::compile_screen(&screen).map_err(|e| e.to_string())
        }
        _ => Err("expected a screen layout (screen { ... })".to_string()),
    }
}
