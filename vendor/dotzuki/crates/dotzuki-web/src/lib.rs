use std::collections::HashMap;

use wasm_bindgen::prelude::*;

use dotzuki_renderer::layout_engine::deserialize::parse_layout;
use dotzuki_renderer::layout_engine::registry::ElementRegistry;
use dotzuki_renderer::layout_engine::renderer::render_layout as render_screen;
use dotzuki_renderer::layout_engine::types::{DataContext, DataValue, RenderContext, Theme};
use dotzuki_renderer::{FrameBuffer, RenderConfig};

/// Real-engine audio playback (`render_audio_pcm`, `audio_sample_rate`).
mod audio;

/// pixels + winit game shell (feature `game-shell`): browser canvas + native
/// fallback window around a [`GameLoop`].
#[cfg(feature = "game-shell")]
pub mod game_shell;

/// BroadcastChannel link transport (feature `link`): serverless tab-to-tab
/// link play over the game's own message type.
#[cfg(feature = "link")]
pub mod link;

#[cfg(feature = "game-shell")]
pub use game_shell::{GameLoop, GameShellConfig, GameShellError, run_game};

use dotzuki_engine::render::Rgba;
use dotzuki_ui::FrameBufferPainter;

/// Log a warning message (goes to stderr; in WASM this reaches the browser
/// console when using `wasm-bindgen` test runner or `console_log`).
fn log_warn(msg: &str) {
    eprintln!("[dotzuki-web] WARN: {}", msg);
}

/// Log an error message.
fn log_error(msg: &str) {
    eprintln!("[dotzuki-web] ERROR: {}", msg);
}

#[cfg(feature = "debug-panic-hook")]
#[wasm_bindgen]
pub fn install_panic_hook() {
    console_error_panic_hook::set_once();
}

/// Generic, **game-agnostic** layout preview for editors.
///
/// Compiles raw `.gui` source, renders at an arbitrary `width`×`height`, lets
/// the editor inject the screen `theme` (so high-resolution / proportional /
/// themed layouts preview exactly as in-game) and supplies the data bindings
/// itself as JSON. Game-specific `custom:*` elements are NOT registered here
/// (the engine cannot know them); games that have any ship their own preview
/// WASM built on `dotzuki-renderer`'s layout engine.
///
/// * `source`     — raw `.gui` DSL text.
/// * `width/height` — framebuffer size in pixels (e.g. 426×240 for wuxia).
/// * `theme_json` — a `Theme` object (`{bg_color, text_mode, ink, …}`); empty
///   string keeps the layout's own theme (default GB white/tile).
/// * `data_json`  — an object of template bindings; values may be nested arrays
///   (→ `DataValue::List`) to feed `list`/`flex_list` rows.
/// * `lang`       — 0=en, 1=zh for `@t(...)` text.
///
/// Returns a `width*height*4` RGBA buffer, or empty on compile/parse error.
#[wasm_bindgen]
pub fn render_gui(
    source: &str,
    width: u32,
    height: u32,
    theme_json: &str,
    data_json: &str,
    lang: u32,
) -> Vec<u8> {
    // 1. Compile the `.gui` source → schema-v2 JSON → ScreenLayout.
    //    A declarations-only component prelude (`component Foo { ... }`) is a
    //    valid `.gui` file but has no screen to render — return an empty buffer
    //    silently instead of logging a compile error.
    let json = match parse_gui_doc(source) {
        Ok(dotzuki_engine_dsl::ast::Document::Screen(screen)) => {
            match dotzuki_engine_dsl::codegen::json_ui::compile_screen(&screen) {
                Ok(j) => j,
                Err(e) => {
                    log_error(&format!("render_gui: compile failed: {e}"));
                    return Vec::new();
                }
            }
        }
        Ok(dotzuki_engine_dsl::ast::Document::Components(_)) => return Vec::new(),
        Ok(_) => {
            log_error("render_gui: expected a screen layout (screen { ... })");
            return Vec::new();
        }
        Err(e) => {
            log_error(&format!("render_gui: compile failed: {e}"));
            return Vec::new();
        }
    };
    let mut layout = match parse_layout(&json) {
        Ok(l) => l,
        Err(e) => {
            log_error(&format!("render_gui: parse failed: {e:?}"));
            return Vec::new();
        }
    };

    // 2. Editor-supplied theme override (the DSL emits no theme block).
    if !theme_json.is_empty() {
        match serde_json::from_str::<Theme>(theme_json) {
            Ok(t) => layout.theme = t,
            Err(e) => log_warn(&format!("render_gui: theme parse failed: {e}")),
        }
    }

    // 3. Data context from the editor's mock bindings (recursive: nested arrays
    //    become DataValue::List for flex_list/list rows).
    let mut ctx = DataContext::new();
    ctx.set("__lang", if lang == 1 { "zh" } else { "en" });
    if !data_json.is_empty() {
        match serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(data_json) {
            Ok(map) => {
                for (key, value) in map {
                    ctx.set(&key, json_to_data_value(&value));
                }
            }
            Err(e) => log_warn(&format!("render_gui: data parse failed: {e}")),
        }
    }

    // 4. Render at the requested size (render_layout clears to the theme bg).
    let fonts: HashMap<String, ()> = HashMap::new();
    let tilesets: HashMap<String, ()> = HashMap::new();
    let render_ctx = RenderContext::new(&layout.screen, &layout.theme, &fonts, &tilesets);
    let mut fb = FrameBuffer::new(RenderConfig::new(width, height), Rgba::WHITE);
    {
        let mut painter = FrameBufferPainter::new(&mut fb);
        let registry = ElementRegistry::new();
        if let Err(e) = render_screen(&layout, &ctx, &render_ctx, &registry, &mut painter) {
            log_error(&format!("render_gui: render failed: {e:?}"));
        }
    }
    fb.data
}

/// Recursively convert an editor JSON binding value to a [`DataValue`].
fn json_to_data_value(v: &serde_json::Value) -> DataValue {
    match v {
        serde_json::Value::String(s) => DataValue::Str(s.clone()),
        serde_json::Value::Bool(b) => DataValue::Bool(*b),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(DataValue::Int)
            .unwrap_or_else(|| DataValue::Float(n.as_f64().unwrap_or(0.0))),
        serde_json::Value::Array(a) => DataValue::List(a.iter().map(json_to_data_value).collect()),
        serde_json::Value::Object(_) => DataValue::Str(v.to_string()),
        serde_json::Value::Null => DataValue::Str(String::new()),
    }
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
///   `{ ok: true, kind: "screen", js: "<compiled JSON>" }` for a screen layout
///   `{ ok: true, kind: "components", names: ["Foo", ...] }` for a
///     declarations-only component prelude (a valid `.gui` file with no screen
///     to preview — the editor shows an informational state, not an error)
///   `{ ok: false, error, raw, line, col }` on failure
#[wasm_bindgen]
pub fn compile_screen_source(source: &str) -> String {
    match parse_gui_doc(source) {
        Ok(dotzuki_engine_dsl::ast::Document::Screen(screen)) => {
            match dotzuki_engine_dsl::codegen::json_ui::compile_screen(&screen) {
                Ok(json) => serde_json::json!({ "ok": true, "kind": "screen", "js": json }).to_string(),
                Err(e) => dsl_err_json(&e.to_string()),
            }
        }
        Ok(dotzuki_engine_dsl::ast::Document::Components(decls)) => {
            let names: Vec<&str> = decls.iter().map(|d| d.name.as_str()).collect();
            serde_json::json!({ "ok": true, "kind": "components", "names": names }).to_string()
        }
        Ok(_) => dsl_err_json("expected a screen layout (screen { ... })"),
        Err(e) => dsl_err_json(&e),
    }
}

/// Parse `.gui` source into a DSL [`Document`](dotzuki_engine_dsl::ast::Document),
/// or a `line:col: msg` error string. Shared by [`compile_screen_source`] and
/// [`render_gui`].
fn parse_gui_doc(source: &str) -> Result<dotzuki_engine_dsl::ast::Document, String> {
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

    doc.ok_or_else(|| "parser returned no document".to_string())
}

#[cfg(test)]
mod render_gui_tests {
    use super::*;

    /// End-to-end smoke test of the generic editor preview path: compile a wuxia
    /// `.gui`, inject the parchment theme, bind list rows, render at 426×240, and
    /// assert real (non-background) pixels were drawn.
    #[test]
    fn render_gui_draws_themed_proportional_layout() {
        let src = r##"screen Party {
  text("队伍") { rect = {tx: 2, ty: 1, tw: 20, th: 2} color = "#F0D070" }
  flex_list("{members}") {
    rect = {tx: 2, ty: 4, tw: 50, th: 22}
    item_layout = [
      {field: "name", width: 26, align: "left"},
      {field: "hp", width: 13, align: "right"}
    ]
    padding = {top: 0, left: 0}
    gap = 1
    selected = "{cursor}"
    cursor = {tile: 223, position: "left"}
  }
}"##;
        let theme = r##"{"bg_color":"#18140F","default_font":"default","text_mode":"proportional","ink":"#F4ECD8","cursor_color":"#F0D070"}"##;
        let data = r##"{"members":[["陈墨  [土]主角","气血120"],["吕醉仙  [火]侠客","气血130"]],"cursor":0}"##;

        let bytes = render_gui(src, 426, 240, theme, data, 1);
        assert_eq!(bytes.len(), 426 * 240 * 4, "RGBA buffer size");

        // Background is #18140F; require a meaningful number of ink/cursor pixels.
        let non_bg = bytes
            .chunks_exact(4)
            .filter(|p| p[0..3] != [0x18, 0x14, 0x0F])
            .count();
        assert!(non_bg > 200, "expected drawn glyph pixels, got {non_bg}");
    }

    /// Bad source compiles to nothing → empty buffer (editor shows blank, no panic).
    #[test]
    fn render_gui_bad_source_is_empty() {
        assert!(render_gui("not a screen", 100, 100, "", "{}", 0).is_empty());
    }

    /// A declarations-only component prelude is a valid `.gui` file:
    /// `compile_screen_source` reports it as `kind: "components"` (not an error)
    /// so the editor can show an informational state instead of
    /// "expected a screen layout (screen { ... })".
    #[test]
    fn compile_screen_source_accepts_component_prelude() {
        let src = r##"component HpGauge {
  current: int required
  max: int required
  color: color
}

component NamePlate {
  title: string required
}"##;
        let raw = compile_screen_source(src);
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed["ok"], true, "component prelude must not be an error: {raw}");
        assert_eq!(parsed["kind"], "components");
        assert_eq!(parsed["names"], serde_json::json!(["HpGauge", "NamePlate"]));
        // Nothing to render — empty buffer, and no error logged.
        assert!(render_gui(src, 100, 100, "", "{}", 0).is_empty());
    }

    /// Screen sources keep the original success shape, plus `kind: "screen"`.
    #[test]
    fn compile_screen_source_marks_screens() {
        let raw = compile_screen_source(r##"screen Main { text("hi") { rect = {tx: 1, ty: 1, tw: 4, th: 1} } }"##);
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed["ok"], true, "screen must compile: {raw}");
        assert_eq!(parsed["kind"], "screen");
        assert!(parsed["js"].as_str().unwrap().contains("\"screen\""));
    }
}
