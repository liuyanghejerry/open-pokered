use std::collections::hash_map::DefaultHasher;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use serde_json;

use crate::ast;
use crate::codegen::js_storyline;
use crate::codegen::js_variables;
use crate::codegen::json_atlas;
use crate::codegen::json_theme;
use crate::codegen::json_ui;
use crate::lexer;
use crate::parser;
use crate::sourcemap;

// ── Routing table entry ────────────────────────────────────────────────────

/// A single routing entry mapping (map, npc/onEnter) → storyline function.
/// This is the dispatch table the engine uses to decide which storyline
/// to activate when a player interacts with an NPC or enters a map.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RouteEntry {
    pub map: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npc: Option<String>,
    #[serde(rename = "onEnter", skip_serializing_if = "is_false", default)]
    pub on_enter: bool,
    pub storyline: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

// ── Compile output variants ────────────────────────────────────────────────

#[derive(Debug)]
pub enum CompileOutput {
    Scene {
        name: String,
        js: String,
        /// Routing entries extracted from storylines with @trigger declarations
        routes: Vec<RouteEntry>,
        source_path: String,
    },
    UILayout {
        name: String,
        json: String,
        source_path: String,
    },
    Theme {
        name: String,
        json: String,
        source_path: String,
    },
    Style {
        name: String,
        json: String,
        source_path: String,
    },
    /// A declarations-only `.gui` prelude (e.g. `components.gui`): no
    /// artifact is emitted — the declarations are collected by
    /// [`compile_dirs`] and pre-registered when compiling screen files.
    ComponentDecls,
}

// ── File discovery ──────────────────────────────────────────────────────────

pub const DSL_EXTENSIONS: &[&str] = &["scene", "gui", "theme", "style"];

pub fn find_search_dirs(manifest_path: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    let examples_dir = manifest_path.join("..").join("..").join("examples");
    if examples_dir.exists() {
        if let Ok(entries) = fs::read_dir(&examples_dir) {
            for entry in entries.flatten() {
                let assets = entry.path().join("assets");
                if assets.exists() {
                    dirs.push(assets);
                }
            }
        }
    }

    // The `examples/*/assets` and `dotzuki-template/assets` entries below are
    // generic search dirs shared by every game project. Games that need extra
    // dirs (e.g. their own data tree) inject them via `DOTZUKI_DSL_DIRS`.
    let template_dir = manifest_path
        .join("..")
        .join("dotzuki-template")
        .join("assets");
    if template_dir.exists() {
        dirs.push(template_dir);
    }

    let crate_assets = manifest_path.join("assets");
    if crate_assets.exists() {
        dirs.push(crate_assets);
    }

    dirs
}

/// Merge extra search dirs (a `:`-separated list such as the `DOTZUKI_DSL_DIRS`
/// environment variable) with the built-in `find_search_dirs` results.
/// Extra dirs come first; duplicates are removed preserving order.
pub fn merge_search_dirs(extra: Option<&str>, manifest_path: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(value) = extra {
        for part in value.split(':') {
            let part = part.trim();
            if !part.is_empty() {
                dirs.push(PathBuf::from(part));
            }
        }
    }
    dirs.extend(find_search_dirs(manifest_path));
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));
    dirs
}

/// Public API: discover all DSL files recursively under `root`.
pub fn discover_dsl_files(root: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    if root.exists() {
        scan_dsl_files(root, &mut files);
    }
    files
}

fn scan_dsl_files(dir: &Path, files: &mut Vec<(String, String)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();

    for path in paths {
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        if file_name.starts_with('.') || file_name == "node_modules" || file_name == "target" {
            continue;
        }

        if path.is_dir() {
            scan_dsl_files(&path, files);
        } else if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if DSL_EXTENSIONS.contains(&ext) {
                    files.push((ext.to_string(), path.to_string_lossy().to_string()));
                }
            }
        }
    }
}

// ── DSL compilation ─────────────────────────────────────────────────────────

pub fn compile_dsl_file(
    ext: &str,
    content: &str,
    file_path: &str,
    out_dir: &Path,
) -> Result<CompileOutput, String> {
    compile_dsl_file_opt(ext, content, file_path, Some(out_dir))
}

/// Internal form of `compile_dsl_file` that accepts an optional output
/// directory: `None` skips all artifact writes (runtime/in-memory use).
fn compile_dsl_file_opt(
    ext: &str,
    content: &str,
    file_path: &str,
    out_dir: Option<&Path>,
) -> Result<CompileOutput, String> {
    compile_dsl_file_opt_with_components(ext, content, file_path, out_dir, &[])
}

/// `compile_dsl_file_opt` with a shared component prelude: `.gui` files are
/// parsed with `decls` pre-registered, so screens may use custom component
/// types declared in a `components.gui` file elsewhere in the project.
fn compile_dsl_file_opt_with_components(
    ext: &str,
    content: &str,
    file_path: &str,
    out_dir: Option<&Path>,
    component_decls: &[ast::ComponentDecl],
) -> Result<CompileOutput, String> {
    let (src, effective_ext) = match ext {
        "theme" | "style" => {
            let wrapped = format!("game_scene _auto {{ {} }}", content);
            (wrapped, ext.to_string())
        }
        _ => (content.to_string(), ext.to_string()),
    };

    let tokens = lexer::Lexer::new(&src, file_path)
        .tokenize()
        .map_err(|errors| {
            let msgs: Vec<String> = errors
                .iter()
                .map(|e| format!("{}:{}: {}", e.line, e.col, e.message))
                .collect();
            msgs.join("; ")
        })?;

    let (doc, parse_errors, semantic_errors) = if component_decls.is_empty() {
        parser::parse_and_validate(tokens, &src)
    } else {
        parser::parse_and_validate_with_components(tokens, &src, component_decls)
    };
    if !parse_errors.is_empty() {
        let msgs: Vec<String> = parse_errors.iter().map(|e| e.to_string()).collect();
        return Err(msgs.join("; "));
    }
    if !semantic_errors.is_empty() {
        let msgs: Vec<String> = semantic_errors.iter().map(|e| e.to_string()).collect();
        return Err(msgs.join("; "));
    }

    let doc = doc.ok_or_else(|| "parser returned no document".to_string())?;

    match (&doc, effective_ext.as_str()) {
        (ast::Document::Scene(scene), "scene") => {
            let name = scene.name.clone();
            let mut sm = sourcemap::SourceMapBuilder::new(file_path, &format!("{}.js", name));

            let mut js = String::new();
            js.push_str("// @generated by dotzuki-engine-dsl — do not edit.\n");
            js.push_str(&format!("// Source: {}\n\n", file_path));

            if let Some(ref vars) = scene.variables {
                let vars_js = js_variables::compile_variables(vars, &mut sm);
                if !vars_js.is_empty() {
                    js.push_str(&vars_js);
                    js.push('\n');
                }
            }

            let mut routes = Vec::new();
            let module_vars: Vec<String> = scene
                .variables
                .as_ref()
                .map(|v| v.decls.iter().map(|d| d.name.clone()).collect())
                .unwrap_or_default();
            for storyline in &scene.storylines {
                let sb = ast::StorylineBlock {
                    statements: storyline.statements.clone(),
                    span: storyline.span.clone(),
                };
                let story_js = js_storyline::compile_storyline_with_vars(
                    &storyline.name,
                    &sb,
                    &mut sm,
                    &module_vars,
                );
                js.push_str(&story_js);
                js.push('\n');

                for trigger in &storyline.triggers {
                    routes.push(RouteEntry {
                        map: trigger.map.clone(),
                        npc: trigger.npc.clone(),
                        on_enter: trigger.on_enter,
                        storyline: storyline.name.clone(),
                        after: trigger.after.clone(),
                    });
                }
            }

            if let Some(ref onload) = scene.on_load {
                let onload_js = js_storyline::compile_onload(&name, onload, &mut sm);
                js.push_str(&onload_js);
                js.push('\n');
            }

            js.push_str(&sm.finalize());
            js.push('\n');

            if let Some(out_dir) = out_dir {
                let js_path = out_dir.join(format!("{}.js", name));
                write_if_changed(&js_path, &js);
            }

            if let Some(ref ui) = scene.ui {
                let ui_json =
                    json_ui::compile_ui(ui).map_err(|e| format!("UI codegen error: {}", e))?;
                if let Some(out_dir) = out_dir {
                    let ui_path = out_dir.join(format!("{}_ui.json", name));
                    let ui_with_header = format!(
                        "// @generated by dotzuki-engine-dsl — do not edit.\n// Source: {}\n\n{}",
                        file_path, ui_json
                    );
                    write_if_changed(&ui_path, &ui_with_header);
                }
            }

            if let Some(out_dir) = out_dir {
                for (i, theme) in scene.themes.iter().enumerate() {
                    let theme_json = json_theme::compile_theme(theme);
                    let theme_path = out_dir.join(format!("{}_theme_{}.json", name, i));
                    let theme_with_header = format!(
                        "// @generated by dotzuki-engine-dsl — do not edit.\n// Source: {}\n\n{}",
                        file_path, theme_json
                    );
                    write_if_changed(&theme_path, &theme_with_header);
                }
            }

            if !scene.styles.is_empty() {
                let styles_json = json_theme::compile_styles_resolved(&scene.styles);
                if let Some(out_dir) = out_dir {
                    let styles_path = out_dir.join(format!("{}_styles.json", name));
                    let styles_with_header = format!(
                        "// @generated by dotzuki-engine-dsl — do not edit.\n// Source: {}\n\n{}",
                        file_path, styles_json
                    );
                    write_if_changed(&styles_path, &styles_with_header);
                }
            }

            if let Some(out_dir) = out_dir {
                for (i, atlas) in scene.atlases.iter().enumerate() {
                    let atlas_json = json_atlas::compile_atlas(atlas);
                    let atlas_path = out_dir.join(format!("{}_atlas_{}.json", name, i));
                    let atlas_with_header = format!(
                        "// @generated by dotzuki-engine-dsl — do not edit.\n// Source: {}\n\n{}",
                        file_path, atlas_json
                    );
                    write_if_changed(&atlas_path, &atlas_with_header);
                }
            }

            Ok(CompileOutput::Scene {
                name,
                js,
                routes,
                source_path: file_path.to_string(),
            })
        }

        (ast::Document::Screen(screen), "gui") => {
            let name = screen.name.clone();
            let ui_json =
                json_ui::compile_screen(screen).map_err(|e| format!("UI codegen error: {}", e))?;
            if let Some(out_dir) = out_dir {
                let ui_path = out_dir.join(format!("{}.json", name));
                let ui_with_header = format!(
                    "// @generated by dotzuki-engine-dsl — do not edit.\n// Source: {}\n\n{}",
                    file_path, ui_json
                );
                write_if_changed(&ui_path, &ui_with_header);
            }

            Ok(CompileOutput::UILayout {
                name,
                json: ui_json,
                source_path: file_path.to_string(),
            })
        }

        // Declarations-only `.gui` prelude (e.g. `components.gui`): not an
        // error and no artifact — `compile_dirs` collects these declarations
        // and pre-registers them when compiling the project's screen files.
        (ast::Document::Components(_), "gui") => Ok(CompileOutput::ComponentDecls),

        (ast::Document::Scene(scene), "theme") => {
            if scene.themes.is_empty() {
                return Err("no @theme blocks found in .theme file".to_string());
            }
            for theme in &scene.themes {
                let name = theme.name.clone();
                let theme_json = json_theme::compile_theme(theme);
                if let Some(out_dir) = out_dir {
                    let theme_path = out_dir.join(format!("{}.json", name));
                    let theme_with_header = format!(
                        "// @generated by dotzuki-engine-dsl — do not edit.\n// Source: {}\n\n{}",
                        file_path, theme_json
                    );
                    write_if_changed(&theme_path, &theme_with_header);
                }
            }
            Ok(CompileOutput::Theme {
                name: scene.name.clone(),
                json: String::new(),
                source_path: file_path.to_string(),
            })
        }

        (ast::Document::Scene(scene), "style") => {
            if scene.styles.is_empty() {
                return Err("no @style blocks found in .style file".to_string());
            }
            let name = scene.name.clone();
            let styles_json = json_theme::compile_styles_resolved(&scene.styles);
            if let Some(out_dir) = out_dir {
                let styles_path = out_dir.join(format!("{}_styles.json", name));
                let styles_with_header = format!(
                    "// @generated by dotzuki-engine-dsl — do not edit.\n// Source: {}\n\n{}",
                    file_path, styles_json
                );
                write_if_changed(&styles_path, &styles_with_header);
            }

            Ok(CompileOutput::Style {
                name,
                json: String::new(),
                source_path: file_path.to_string(),
            })
        }

        (doc, ext) => {
            let kind = match doc {
                ast::Document::Scene(_) => "game_scene",
                ast::Document::Screen(_) => "screen",
                ast::Document::Components(_) => "component declarations",
            };
            Err(format!(
                "unexpected document variant ({}) for .{} file",
                kind, ext
            ))
        }
    }
}

// ── Batch compilation (library form of the build-script pipeline) ──────────

/// Parse `content` (a `.gui` file) and return its declarations when it is a
/// declarations-only component prelude (e.g. `components.gui`). Returns
/// `None` for screens and for files that fail to parse — the main
/// compilation pass reports those errors as diagnostics.
fn parse_component_prelude(content: &str, file_path: &str) -> Option<Vec<ast::ComponentDecl>> {
    let tokens = lexer::Lexer::new(content, file_path).tokenize().ok()?;
    let (doc, parse_errors, _) = parser::parse_and_validate(tokens, content);
    if !parse_errors.is_empty() {
        return None;
    }
    match doc {
        Some(ast::Document::Components(decls)) => Some(decls),
        _ => None,
    }
}

/// In-memory result of compiling every DSL file found under a set of
/// search directories.
///
/// This is the runtime-usable counterpart of the build script: the same
/// discovery → compile → sort → conflict-detection pipeline, with warnings
/// collected into `diagnostics` instead of being printed.
#[derive(Debug, Default)]
pub struct CompileReport {
    /// `(name, js, source_path)` for each compiled `.scene`, sorted by name.
    pub scenes: Vec<(String, String, String)>,
    /// `(name, json, source_path)` for each compiled `.gui`, sorted by name.
    pub ui_layouts: Vec<(String, String, String)>,
    /// `(name, json, source_path)` for each compiled `.theme`, sorted by name.
    /// Note: per-theme JSON is a disk artifact; the in-memory `json` is empty.
    pub themes: Vec<(String, String, String)>,
    /// `(name, json, source_path)` for each compiled `.style`, sorted by name.
    /// Note: styles JSON is a disk artifact; the in-memory `json` is empty.
    pub styles: Vec<(String, String, String)>,
    /// Routing entries collected from all scenes, in compile order.
    pub routes: Vec<RouteEntry>,
    /// All discovered `(extension, path)` pairs that were attempted,
    /// sorted by path.
    pub files: Vec<(String, String)>,
    /// Non-fatal problems: unreadable files, compile failures, route
    /// conflicts (same wording the build script emits as `cargo:warning`).
    pub diagnostics: Vec<String>,
}

/// Compile every DSL file discoverable under `dirs` into a `CompileReport`.
///
/// When `dsl_out_dir` is `Some`, compiled artifacts (`.js` / `.json` files)
/// are written there exactly as the build script does; `None` skips all
/// disk writes (runtime/in-memory use).
pub fn compile_dirs(dirs: &[&Path], dsl_out_dir: Option<&Path>) -> CompileReport {
    let mut discovered: Vec<(String, String)> = Vec::new();
    for dir in dirs {
        if dir.exists() {
            discovered.extend(discover_dsl_files(dir));
        }
    }

    let mut files: Vec<(String, String, String)> = Vec::new();
    let mut read_errors: Vec<String> = Vec::new();
    for (ext, path) in discovered {
        match fs::read_to_string(&path) {
            Ok(content) => files.push((ext, path, content)),
            Err(e) => read_errors.push(format!("Failed to read {}: {}", path, e)),
        }
    }

    let mut report = compile_files(&files, dsl_out_dir);
    read_errors.extend(std::mem::take(&mut report.diagnostics));
    report.diagnostics = read_errors;
    report
}

/// Compile an in-memory set of DSL files into a `CompileReport`.
///
/// `files` is a list of `(extension, path, content)` triples; `path` is used
/// only for diagnostics, `source_path` bookkeeping and route/map matching —
/// nothing is read from disk. This is the WASM-friendly counterpart of
/// [`compile_dirs`], which is a thin discover+read wrapper around it.
pub fn compile_files(
    files: &[(String, String, String)],
    dsl_out_dir: Option<&Path>,
) -> CompileReport {
    let mut report = CompileReport::default();

    let mut files: Vec<(String, String, String)> = files.to_vec();
    files.sort_by(|a, b| a.1.cmp(&b.1));
    report.files = files
        .iter()
        .map(|(ext, path, _)| (ext.clone(), path.clone()))
        .collect();

    if let Some(out_dir) = dsl_out_dir {
        fs::create_dir_all(out_dir).ok();
    }

    // Pre-pass: collect shared component declarations from declarations-only
    // `.gui` files (e.g. `components.gui`). Screen files are then compiled
    // with these pre-registered so custom component types resolve across
    // files — mirroring how game build scripts seed their component prelude.
    // Prelude files themselves compile WITHOUT the seed: their local
    // declarations would otherwise collide with the seeded copy of the same
    // name (DuplicateComponentDecl).
    let mut component_prelude: Vec<ast::ComponentDecl> = Vec::new();
    let mut prelude_files: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (ext, path_str, content) in &files {
        if ext != "gui" {
            continue;
        }
        if let Some(decls) = parse_component_prelude(content, path_str) {
            component_prelude.extend(decls);
            prelude_files.insert(path_str.as_str());
        }
    }

    for (ext, path_str, content) in &files {
        let seed: &[ast::ComponentDecl] = if prelude_files.contains(path_str.as_str()) {
            &[]
        } else {
            &component_prelude
        };
        match compile_dsl_file_opt_with_components(ext, content, path_str, dsl_out_dir, seed) {
            Ok(CompileOutput::Scene {
                name,
                js,
                routes,
                source_path,
            }) => {
                report.scenes.push((name, js, source_path));
                report.routes.extend(routes);
            }
            Ok(CompileOutput::UILayout {
                name,
                json,
                source_path,
            }) => {
                report.ui_layouts.push((name, json, source_path));
            }
            Ok(CompileOutput::Theme {
                name,
                json,
                source_path,
            }) => {
                report.themes.push((name, json, source_path));
            }
            Ok(CompileOutput::Style {
                name,
                json,
                source_path,
            }) => {
                report.styles.push((name, json, source_path));
            }
            // Declarations-only prelude: consumed by the pre-pass above.
            Ok(CompileOutput::ComponentDecls) => {}
            Err(msg) => {
                report
                    .diagnostics
                    .push(format!("Failed to compile {}: {}", path_str, msg));
            }
        }
    }

    report.scenes.sort_by(|a, b| a.0.cmp(&b.0));
    report.ui_layouts.sort_by(|a, b| a.0.cmp(&b.0));
    report.themes.sort_by(|a, b| a.0.cmp(&b.0));
    report.styles.sort_by(|a, b| a.0.cmp(&b.0));

    let conflict_result = crate::conflict::detect_conflicts(&report.routes);
    report.diagnostics.extend(conflict_result.warnings);

    report
}

// ── Idempotent output ───────────────────────────────────────────────────────

/// Compute a stable u64 hash for a string slice.
pub fn content_hash(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

pub fn write_if_changed(path: &Path, content: &str) {
    let new_hash = content_hash(content);

    if path.exists() {
        if let Ok(existing) = fs::read_to_string(path) {
            if content_hash(&existing) == new_hash {
                return;
            }
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }

    let mut file = BufWriter::new(File::create(path).unwrap());
    file.write_all(content.as_bytes()).unwrap();
    file.flush().unwrap();
}

// ── Routing table JSON generation ──────────────────────────────────────────

/// Serialize all collected route entries to `storyline_routes.json` in `out_dir`.
pub fn generate_routing_table(out_dir: &Path, routes: &[RouteEntry]) {
    let dest_path = out_dir.join("storyline_routes.json");
    let payload = serde_json::json!({ "routes": routes });
    let json = serde_json::to_string_pretty(&payload).unwrap();
    write_if_changed(&dest_path, &json);
}

// ── embedded module generator ───────────────────────────────────────────────

pub fn generate_embedded_module(
    out_dir: &Path,
    scenes: &[(String, String, String)],
    ui_layouts: &[(String, String, String)],
    _themes: &[(String, String, String)],
    _styles: &[(String, String, String)],
) {
    let dest_path = out_dir.join("embedded_scenes.rs");
    let mut file = BufWriter::new(File::create(&dest_path).unwrap());

    writeln!(file, "// @generated by dotzuki-engine-dsl — do not edit.").unwrap();
    writeln!(file).unwrap();

    for (i, (name, _js, _src)) in scenes.iter().enumerate() {
        let js_abs = out_dir.join("dsl").join(format!("{}.js", name));
        writeln!(
            file,
            "static SCENE_JS_{}: &str = include_str!(r\"{}\");",
            i,
            js_abs.display()
        )
        .unwrap();
    }
    writeln!(file).unwrap();

    for (i, (name, _json, _src)) in ui_layouts.iter().enumerate() {
        let json_abs = out_dir.join("dsl").join(format!("{}.json", name));
        writeln!(
            file,
            "static UI_LAYOUT_{}: &str = include_str!(r\"{}\");",
            i,
            json_abs.display()
        )
        .unwrap();
    }
    writeln!(file).unwrap();

    writeln!(
        file,
        "/// Register all DSL-compiled scenes with a scene registrar."
    )
    .unwrap();
    writeln!(
        file,
        "/// The registrar must provide `register_scene_js(name: &str, js: &str)`."
    )
    .unwrap();
    let param_name = if scenes.is_empty() && ui_layouts.is_empty() {
        "_registrar"
    } else {
        "registrar"
    };
    writeln!(
        file,
        "pub fn load_embedded_scenes({}: &mut impl DslSceneRegistrar) {{",
        param_name
    )
    .unwrap();

    for (i, (name, _js, _src)) in scenes.iter().enumerate() {
        writeln!(
            file,
            "    registrar.register_scene_js(\"{}\", SCENE_JS_{});",
            name, i
        )
        .unwrap();
    }

    for (i, (name, _json, _src)) in ui_layouts.iter().enumerate() {
        writeln!(
            file,
            "    registrar.register_ui_layout(\"{}\", UI_LAYOUT_{});",
            name, i
        )
        .unwrap();
    }

    writeln!(file, "}}").unwrap();

    file.flush().unwrap();
}

/// Compile a `.scene` source string directly to JS (no disk I/O).
///
/// Used at runtime for hot-reload: when a `.scene` file changes,
/// the engine can re-compile it on the fly and feed the resulting
/// JS to the Boa script engine without needing a build step.
/// Parse and validate a `.scene` source file into its AST ([`ast::GameScene`]),
/// running the same lexer/parser/semantic-validation pipeline as
/// [`compile_scene_to_js`]. This is the entry point for the native AST
/// interpreter (`crate::interpreter`) and for build-time AST embedding.
pub fn compile_scene_to_ast(source: &str, file_path: &str) -> Result<ast::GameScene, String> {
    let doc = parse_scene_document(source, file_path)?;
    match doc {
        ast::Document::Scene(scene) => Ok(scene),
        _ => Err("only .scene files are supported by this function".to_string()),
    }
}

fn parse_scene_document(source: &str, file_path: &str) -> Result<ast::Document, String> {
    let tokens = lexer::Lexer::new(source, file_path)
        .tokenize()
        .map_err(|errors| {
            errors
                .iter()
                .map(|e| format!("{}:{}: {}", e.line, e.col, e.message))
                .collect::<Vec<_>>()
                .join("; ")
        })?;

    let (doc, parse_errors, semantic_errors) = parser::parse_and_validate(tokens, source);
    if !parse_errors.is_empty() {
        return Err(parse_errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; "));
    }
    if !semantic_errors.is_empty() {
        return Err(semantic_errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; "));
    }

    doc.ok_or_else(|| "parser returned no document".to_string())
}

pub fn compile_scene_to_js(source: &str, file_path: &str) -> Result<String, String> {
    let doc = parse_scene_document(source, file_path)?;
    match doc {
        ast::Document::Scene(scene) => {
            let mut sm = sourcemap::SourceMapBuilder::new(file_path, &format!("{}.js", scene.name));
            let mut js = String::new();
            js.push_str("// @generated by dotzuki-engine-dsl\n");
            js.push_str(&format!("// Source: {}\n\n", file_path));

            if let Some(ref vars) = scene.variables {
                let vars_js = js_variables::compile_variables(vars, &mut sm);
                if !vars_js.is_empty() {
                    js.push_str(&vars_js);
                    js.push('\n');
                }
            }

            for storyline in &scene.storylines {
                let sb = ast::StorylineBlock {
                    statements: storyline.statements.clone(),
                    span: storyline.span.clone(),
                };
                let module_vars: Vec<String> = scene
                    .variables
                    .as_ref()
                    .map(|v| v.decls.iter().map(|d| d.name.clone()).collect())
                    .unwrap_or_default();
                let story_js = js_storyline::compile_storyline_with_vars(
                    &storyline.name,
                    &sb,
                    &mut sm,
                    &module_vars,
                );
                js.push_str(&story_js);
                js.push('\n');
            }

            if let Some(ref onload) = scene.on_load {
                let onload_js = js_storyline::compile_onload(&scene.name, onload, &mut sm);
                js.push_str(&onload_js);
                js.push('\n');
            }

            js.push_str(&sm.finalize());
            js.push('\n');
            Ok(js)
        }
        _ => Err("only .scene files are supported by this function".to_string()),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Unit tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;

    // ── helpers ────────────────────────────────────────────────────────────

    fn tmp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dsl_test_{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &PathBuf) {
        let _ = fs::remove_dir_all(dir);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // discover_dsl_files tests (5)
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_discover_finds_scene_files() {
        let dir = tmp_dir("discover_scene_files");
        fs::write(dir.join("a.scene"), "game_scene A {}").unwrap();
        fs::write(dir.join("b.scene"), "game_scene B {}").unwrap();
        fs::write(dir.join("c.scene"), "game_scene C {}").unwrap();

        let files = discover_dsl_files(&dir);
        assert_eq!(files.len(), 3, "Should find 3 .scene files");
        assert_eq!(files[0].0, "scene");
        assert_eq!(files[1].0, "scene");
        assert_eq!(files[2].0, "scene");
        // Verify sorted order
        assert!(files[0].1.ends_with("a.scene"));
        assert!(files[1].1.ends_with("b.scene"));
        assert!(files[2].1.ends_with("c.scene"));

        cleanup(&dir);
    }

    #[test]
    fn test_discover_empty_dir() {
        let dir = tmp_dir("discover_empty");
        let files = discover_dsl_files(&dir);
        assert!(files.is_empty(), "Empty dir should return empty Vec");
        cleanup(&dir);
    }

    #[test]
    fn test_discover_nonexistent_dir() {
        let dir = PathBuf::from("/tmp/dsl_test_nonexistent_xyz123");
        let files = discover_dsl_files(&dir);
        assert!(
            files.is_empty(),
            "Nonexistent dir should return empty Vec, not panic"
        );
    }

    #[test]
    fn test_discover_skips_hidden() {
        let dir = tmp_dir("discover_hidden");
        // Create a visible directory containing a hidden .scene file
        let subdir = dir.join("visible");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join(".hidden.scene"), "game_scene Hidden {}").unwrap();
        fs::write(subdir.join("normal.scene"), "game_scene Normal {}").unwrap();

        let files = discover_dsl_files(&dir);
        assert_eq!(files.len(), 1, "Should find only 1 non-hidden .scene file");
        assert!(
            files[0].1.ends_with("normal.scene"),
            "Should be normal.scene"
        );
        cleanup(&dir);
    }

    #[test]
    fn test_discover_mixed_extensions() {
        let dir = tmp_dir("discover_mixed");
        fs::write(dir.join("intro.scene"), "game_scene Intro {}").unwrap();
        fs::write(dir.join("menu.gui"), "screen Menu {}").unwrap();
        fs::write(dir.join("dark.theme"), "@theme dark {}").unwrap();
        fs::write(dir.join("retro.style"), "@style retro {}").unwrap();
        fs::write(dir.join("readme.txt"), "not a dsl file").unwrap();

        let files = discover_dsl_files(&dir);
        assert_eq!(files.len(), 4, "Should find 4 DSL files, skip .txt");

        let exts: Vec<&str> = files.iter().map(|(ext, _)| ext.as_str()).collect();
        assert!(exts.contains(&"scene"));
        assert!(exts.contains(&"gui"));
        assert!(exts.contains(&"theme"));
        assert!(exts.contains(&"style"));
        cleanup(&dir);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // content_hash tests (4)
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_content_hash_same_input() {
        let s = "Hello, world! This is a test string.\nWith multiple lines.\n";
        let h1 = content_hash(s);
        let h2 = content_hash(s);
        assert_eq!(h1, h2, "Same input should produce same hash");
    }

    #[test]
    fn test_content_hash_different_input() {
        let h1 = content_hash("alpha");
        let h2 = content_hash("beta");
        assert_ne!(h1, h2, "Different inputs should produce different hashes");
    }

    #[test]
    fn test_content_hash_empty() {
        let h = content_hash("");
        // Empty string should produce a valid u64 (no panic)
        assert!(h > 0 || h == 0, "Empty string should produce a hash");
    }

    #[test]
    fn test_content_hash_non_ascii() {
        let h = content_hash("こんにちは世界 🌍 — em dash and Unicode");
        // Non-ASCII should not panic
        assert!(h > 0 || h == 0, "Non-ASCII string should produce a hash");
    }

    // ═══════════════════════════════════════════════════════════════════════
    // compile_dsl_file tests (3)
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_compile_scene_to_js() {
        let out_dir = tmp_dir("compile_scene_js");
        let content = "\
game_scene Test {
  @storylines {
    @speaker(\"NPC\") { \"Hello\" }
  }
}";
        let result = compile_dsl_file("scene", content, "test.scene", &out_dir);
        assert!(
            result.is_ok(),
            "Compilation should succeed: {:?}",
            result.err()
        );
        if let Ok(CompileOutput::Scene { js, .. }) = result {
            assert!(
                js.contains("storyline_main"),
                "JS should contain storyline_main, got:\n{}",
                js
            );
            assert!(
                js.contains("game.showText"),
                "JS should contain game.showText, got:\n{}",
                js
            );
        } else {
            panic!("Expected Scene output");
        }
        cleanup(&out_dir);
    }

    #[test]
    fn test_compile_scene_error() {
        let out_dir = tmp_dir("compile_scene_error");
        // Completely invalid content that the lexer cannot parse
        let content = "not valid dsl content @ # $ % ^";
        let result = compile_dsl_file("scene", content, "bad.scene", &out_dir);
        assert!(result.is_err(), "Invalid DSL should produce an Err");
        assert!(
            !result.unwrap_err().is_empty(),
            "Error message should be non-empty"
        );
        cleanup(&out_dir);
    }

    #[test]
    fn test_compile_scene_with_onload_to_js() {
        let out_dir = tmp_dir("compile_scene_onload");
        let content = "\
game_scene StartTown {
  @load {
    @if (game.getFlag(\"EVENT_X\")) {
      game.setFlag(\"EVENT_Y\")
    }
  }
  @storyline(\"talkProf\") {
    @trigger(map = \"StartTown\", npc = \"Prof\")
    @speaker(\"Prof\") { \"Hello!\" }
  }
}";
        let result = compile_dsl_file("scene", content, "start.scene", &out_dir);
        assert!(
            result.is_ok(),
            "Compilation should succeed: {:?}",
            result.err()
        );
        if let Ok(CompileOutput::Scene { js, .. }) = result {
            assert!(
                js.contains("export async function StartTownOnLoad()"),
                "JS should contain StartTownOnLoad, got:\n{}",
                js
            );
            assert!(
                js.contains("export async function storyline_talkProf()"),
                "JS should contain storyline_talkProf, got:\n{}",
                js
            );
            assert!(
                js.contains("game.getFlag"),
                "JS should contain the getFlag call, got:\n{}",
                js
            );
        } else {
            panic!("Expected Scene output");
        }
        cleanup(&out_dir);
    }

    #[test]
    fn test_compile_gui_to_json() {
        let out_dir = tmp_dir("compile_gui_json");
        let content = "\
screen MainMenu {
  panel {
    text(\"Hello\") {}
  }
}";
        let result = compile_dsl_file("gui", content, "test.gui", &out_dir);
        assert!(
            result.is_ok(),
            "Compilation should succeed: {:?}",
            result.err()
        );
        if let Ok(CompileOutput::UILayout { json, .. }) = result {
            assert!(json.contains("Hello"), "JSON should contain text content");
            assert!(json.contains("\"type\""), "JSON should contain type field");
        } else {
            panic!("Expected UILayout output");
        }
        cleanup(&out_dir);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // generate_embedded_module tests (2)
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_generate_one_scene() {
        let out_dir = tmp_dir("generate_one_scene");
        let scenes: Vec<(String, String, String)> = vec![(
            "intro".to_string(),
            "fake_js".to_string(),
            "intro.scene".to_string(),
        )];
        generate_embedded_module(&out_dir, &scenes, &[], &[], &[]);

        let content = fs::read_to_string(out_dir.join("embedded_scenes.rs"))
            .expect("Should write embedded_scenes.rs");
        assert!(
            content.contains("include_str!"),
            "Output should contain include_str!"
        );
        assert!(
            content.contains("SCENE_JS_0"),
            "Output should contain SCENE_JS_0"
        );
        assert!(
            content.contains("register_scene_js"),
            "Output should contain register_scene_js"
        );
        cleanup(&out_dir);
    }

    #[test]
    fn test_generate_zero_scenes() {
        let out_dir = tmp_dir("generate_zero_scenes");
        generate_embedded_module(&out_dir, &[], &[], &[], &[]);

        let content = fs::read_to_string(out_dir.join("embedded_scenes.rs"))
            .expect("Should write embedded_scenes.rs");
        // When all vecs are empty, the function uses `_registrar` as param name
        assert!(
            content.contains("_registrar"),
            "Empty output should use _registrar parameter"
        );
        assert!(
            !content.contains("include_str!"),
            "Empty output should not contain include_str!"
        );
        assert!(
            !content.contains("SCENE_JS_"),
            "Empty output should not contain SCENE_JS_"
        );
        cleanup(&out_dir);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // write_if_changed tests (3)
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_write_if_changed_new_file() {
        let dir = tmp_dir("write_new_file");
        let path = dir.join("output.txt");
        let content = "hello world\n";

        write_if_changed(&path, content);
        assert!(path.exists(), "File should be created");
        let written = fs::read_to_string(&path).unwrap();
        assert_eq!(written, content, "File content should match");
        cleanup(&dir);
    }

    #[test]
    fn test_write_if_changed_same_content() {
        let dir = tmp_dir("write_same");
        let path = dir.join("output.txt");
        let content = "identical content\nsecond line\n";

        // First write
        write_if_changed(&path, content);
        let meta_before = fs::metadata(&path).unwrap();
        let modified_before = meta_before.modified().unwrap();

        // Small sleep to ensure timestamp would differ if file were rewritten
        std::thread::sleep(Duration::from_millis(10));

        // Second write with same content — should be a no-op
        write_if_changed(&path, content);
        let meta_after = fs::metadata(&path).unwrap();
        let modified_after = meta_after.modified().unwrap();

        assert_eq!(
            modified_before, modified_after,
            "File should NOT be rewritten when content is identical"
        );
        let written = fs::read_to_string(&path).unwrap();
        assert_eq!(written, content, "Content should still match");
        cleanup(&dir);
    }

    #[test]
    fn test_write_if_changed_different_content() {
        let dir = tmp_dir("write_different");
        let path = dir.join("output.txt");
        let content_a = "first version\n";
        let content_b = "second version — different!\n";

        write_if_changed(&path, content_a);
        write_if_changed(&path, content_b);

        let written = fs::read_to_string(&path).unwrap();
        assert_eq!(written, content_b, "File should contain updated content");
        cleanup(&dir);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // routing table tests (5)
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_routing_table_with_npc_trigger() {
        let out_dir = tmp_dir("routing_npc");
        let content = r#"
game_scene ProfLab {
  @storyline("prof_ask") {
    @trigger(map = "ProfLab", npc = "Prof")
    @speaker("Prof") { "Hello!" }
  }
}
"#;
        let result = compile_dsl_file("scene", content, "prof_lab.scene", &out_dir);
        assert!(
            result.is_ok(),
            "Compilation should succeed: {:?}",
            result.err()
        );
        if let Ok(CompileOutput::Scene { routes, .. }) = result {
            assert_eq!(routes.len(), 1);
            assert_eq!(routes[0].map, "ProfLab");
            assert_eq!(routes[0].npc, Some("Prof".to_string()));
            assert!(!routes[0].on_enter);
            assert_eq!(routes[0].storyline, "prof_ask");
            assert_eq!(routes[0].after, None);
        }
        cleanup(&out_dir);
    }

    #[test]
    fn test_routing_table_on_enter_trigger() {
        let out_dir = tmp_dir("routing_enter");
        let content = r#"
game_scene MartScene {
  @storyline("mart_pickup") {
    @trigger(map = "CityMart", on_enter = true)
    @speaker("Clerk") { "Welcome!" }
  }
}
"#;
        let result = compile_dsl_file("scene", content, "mart.scene", &out_dir);
        assert!(
            result.is_ok(),
            "Compilation should succeed: {:?}",
            result.err()
        );
        if let Ok(CompileOutput::Scene { routes, .. }) = result {
            assert_eq!(routes.len(), 1);
            assert_eq!(routes[0].map, "CityMart");
            assert_eq!(routes[0].npc, None);
            assert!(routes[0].on_enter);
            assert_eq!(routes[0].storyline, "mart_pickup");
        }
        cleanup(&out_dir);
    }

    #[test]
    fn test_routing_table_with_after() {
        let out_dir = tmp_dir("routing_after");
        let content = r#"
game_scene RivalScene {
  @storyline("rival_fight") {
    @trigger(map = "NorthRoute", npc = "Rival", after = "prof_ask")
    @speaker("Rival") { "Let's battle!" }
  }
}
"#;
        let result = compile_dsl_file("scene", content, "rival.scene", &out_dir);
        assert!(
            result.is_ok(),
            "Compilation should succeed: {:?}",
            result.err()
        );
        if let Ok(CompileOutput::Scene { routes, .. }) = result {
            assert_eq!(routes.len(), 1);
            assert_eq!(routes[0].after, Some("prof_ask".to_string()));
        }
        cleanup(&out_dir);
    }

    #[test]
    fn test_routing_table_multiple_storylines() {
        let out_dir = tmp_dir("routing_multi");
        let content = r#"
game_scene MultiScene {
  @storyline("prof_ask") {
    @trigger(map = "ProfLab", npc = "Prof")
    @speaker("Prof") { "Hello!" }
  }
  @storyline("nurse_heal") {
    @trigger(map = "CityCenter", npc = "Nurse")
    @speaker("Nurse") { "Welcome!" }
  }
  @storyline("main") {
    @speaker("NPC") { "Legacy dialog" }
  }
}
"#;
        let result = compile_dsl_file("scene", content, "multi.scene", &out_dir);
        assert!(
            result.is_ok(),
            "Compilation should succeed: {:?}",
            result.err()
        );
        if let Ok(CompileOutput::Scene { routes, .. }) = result {
            assert_eq!(
                routes.len(),
                2,
                "Only triggered storylines should have routes"
            );
            let storylines: Vec<&str> = routes.iter().map(|r| r.storyline.as_str()).collect();
            assert!(storylines.contains(&"prof_ask"));
            assert!(storylines.contains(&"nurse_heal"));
            assert!(
                !storylines.contains(&"main"),
                "main storyline has no trigger, should be excluded"
            );
        }
        cleanup(&out_dir);
    }

    #[test]
    fn test_generate_routing_table_writes_json() {
        let out_dir = tmp_dir("routing_json");
        let routes = vec![
            RouteEntry {
                map: "ProfLab".into(),
                npc: Some("Prof".into()),
                on_enter: false,
                storyline: "prof_ask".into(),
                after: None,
            },
            RouteEntry {
                map: "CityMart".into(),
                npc: None,
                on_enter: true,
                storyline: "mart_pickup".into(),
                after: Some("prof_ask".into()),
            },
        ];
        generate_routing_table(&out_dir, &routes);

        let json_path = out_dir.join("storyline_routes.json");
        assert!(
            json_path.exists(),
            "storyline_routes.json should be created"
        );
        let json_str = fs::read_to_string(&json_path).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let arr = parsed["routes"].as_array().unwrap();
        assert_eq!(arr.len(), 2);

        assert_eq!(arr[0]["map"], "ProfLab");
        assert_eq!(arr[0]["npc"], "Prof");
        assert_eq!(arr[0]["storyline"], "prof_ask");
        assert!(arr[0]["after"].is_null());

        assert_eq!(arr[1]["map"], "CityMart");
        assert!(arr[1]["npc"].is_null());
        assert_eq!(arr[1]["onEnter"], true);
        assert_eq!(arr[1]["after"], "prof_ask");

        cleanup(&out_dir);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // merge_search_dirs tests (3)
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_merge_search_dirs_env_first() {
        let manifest = PathBuf::from("/nonexistent_manifest_xyz");
        let dirs = merge_search_dirs(Some("/tmp/extra_a:/tmp/extra_b"), &manifest);
        assert!(dirs.len() >= 2, "Should contain the two extra dirs");
        assert_eq!(dirs[0], PathBuf::from("/tmp/extra_a"));
        assert_eq!(dirs[1], PathBuf::from("/tmp/extra_b"));
    }

    #[test]
    fn test_merge_search_dirs_skips_empty_segments() {
        let manifest = PathBuf::from("/nonexistent_manifest_xyz");
        let dirs = merge_search_dirs(Some(":/tmp/extra_a::/tmp/extra_b:"), &manifest);
        assert_eq!(dirs.len(), 2, "Empty segments should be skipped");
    }

    #[test]
    fn test_merge_search_dirs_dedup() {
        let dir = tmp_dir("merge_dedup");
        let assets = dir.join("assets");
        fs::create_dir_all(&assets).unwrap();
        // The crate-assets dir is found by find_search_dirs AND passed via env
        let extra = format!("{}", assets.display());
        let dirs = merge_search_dirs(Some(&extra), &dir);
        let count = dirs.iter().filter(|d| **d == assets).count();
        assert_eq!(count, 1, "Duplicate dirs should be removed");
        cleanup(&dir);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // compile_dirs tests (5)
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_compile_dirs_mixed_content() {
        let dir = tmp_dir("compile_dirs_mixed");
        fs::write(
            dir.join("intro.scene"),
            "\
game_scene Intro {
  @storyline(\"prof_ask\") {
    @trigger(map = \"ProfLab\", npc = \"Prof\")
    @speaker(\"Prof\") { \"Hello!\" }
  }
}",
        )
        .unwrap();
        fs::write(
            dir.join("menu.gui"),
            "screen MainMenu {\n  panel {\n    text(\"Hello\") {}\n  }\n}",
        )
        .unwrap();
        fs::write(dir.join("broken.scene"), "game_scene Broken { @@bad }").unwrap();

        let report = compile_dirs(&[dir.as_path()], None);

        assert_eq!(report.scenes.len(), 1, "1 valid scene should compile");
        assert_eq!(report.ui_layouts.len(), 1, "1 gui should compile");
        assert_eq!(
            report.files.len(),
            3,
            "All 3 DSL files should be discovered"
        );
        assert_eq!(report.routes.len(), 1, "Scene should yield 1 route");
        assert_eq!(report.routes[0].storyline, "prof_ask");
        assert!(
            report.scenes[0].1.contains("storyline_prof_ask"),
            "Scene JS should contain storyline_prof_ask, got:\n{}",
            report.scenes[0].1
        );
        assert!(
            report.ui_layouts[0].1.contains("Hello"),
            "UI JSON should contain the text content"
        );
        assert_eq!(
            report.diagnostics.len(),
            1,
            "Broken file should produce 1 diagnostic: {:?}",
            report.diagnostics
        );
        assert!(report.diagnostics[0].contains("Failed to compile"));
        assert!(report.diagnostics[0].contains("broken.scene"));
        cleanup(&dir);
    }

    #[test]
    fn test_compile_dirs_no_disk_writes_when_none() {
        let dir = tmp_dir("compile_dirs_none");
        fs::write(
            dir.join("a.scene"),
            "game_scene A {\n  @storylines {\n    @speaker(\"A\") { \"hi\" }\n  }\n}",
        )
        .unwrap();

        let report = compile_dirs(&[dir.as_path()], None);
        assert_eq!(report.scenes.len(), 1);
        // Nothing may be written next to the sources
        assert!(
            !dir.join("A.js").exists(),
            "None out dir must skip all disk writes"
        );
        cleanup(&dir);
    }

    #[test]
    fn test_compile_dirs_writes_artifacts_when_some() {
        let dir = tmp_dir("compile_dirs_some_src");
        let out_dir = tmp_dir("compile_dirs_some_out");
        fs::write(
            dir.join("a.scene"),
            "game_scene A {\n  @storylines {\n    @speaker(\"A\") { \"hi\" }\n  }\n}",
        )
        .unwrap();

        let report = compile_dirs(&[dir.as_path()], Some(out_dir.as_path()));
        assert_eq!(report.scenes.len(), 1);
        let js_path = out_dir.join("A.js");
        assert!(js_path.exists(), "A.js should be written to the out dir");
        let on_disk = fs::read_to_string(&js_path).unwrap();
        assert_eq!(
            on_disk, report.scenes[0].1,
            "On-disk artifact should match the in-memory JS"
        );
        cleanup(&dir);
        cleanup(&out_dir);
    }

    #[test]
    fn test_compile_dirs_conflicts_in_diagnostics() {
        let dir = tmp_dir("compile_dirs_conflict");
        fs::write(
            dir.join("conflict.scene"),
            "\
game_scene Conflict {
  @storyline(\"prof_ask\") {
    @trigger(map = \"ProfLab\", npc = \"Prof\")
    @speaker(\"Prof\") { \"Hello!\" }
  }
  @storyline(\"rival_challenge\") {
    @trigger(map = \"ProfLab\", npc = \"Prof\")
    @speaker(\"Prof\") { \"Fight!\" }
  }
}",
        )
        .unwrap();

        let report = compile_dirs(&[dir.as_path()], None);
        assert_eq!(report.routes.len(), 2);
        assert_eq!(
            report.diagnostics.len(),
            1,
            "Conflict should produce 1 diagnostic: {:?}",
            report.diagnostics
        );
        assert!(report.diagnostics[0].contains("CONFLICT"));
        assert!(report.diagnostics[0].contains("prof_ask"));
        assert!(report.diagnostics[0].contains("rival_challenge"));
        cleanup(&dir);
    }

    #[test]
    fn test_compile_dirs_nonexistent_dir() {
        let dir = PathBuf::from("/tmp/dsl_test_compile_dirs_nonexistent_xyz123");
        let report = compile_dirs(&[dir.as_path()], None);
        assert!(report.scenes.is_empty());
        assert!(report.files.is_empty());
        assert!(report.diagnostics.is_empty(), "Should not panic or warn");
    }

    #[test]
    fn test_compile_dirs_component_prelude() {
        let dir = tmp_dir("compile_dirs_prelude");
        // Declarations-only prelude: must compile silently (no artifact, no
        // diagnostic) and seed custom component types for screen files.
        fs::write(
            dir.join("components.gui"),
            "component hp_bar {\n  current: expr required\n  max: expr required\n}",
        )
        .unwrap();
        // A screen using the custom component from the prelude: must compile
        // without "'hp_bar' is not a valid component type".
        fs::write(
            dir.join("stats.gui"),
            "screen Stats {\n  hp_bar {\n    rect = {tx: 13, ty: 3, tw: 6, th: 1}\n    current = \"{hp}\"\n    max = \"{max_hp}\"\n  }\n}",
        )
        .unwrap();

        let report = compile_dirs(&[dir.as_path()], None);
        assert_eq!(
            report.diagnostics.len(),
            0,
            "prelude + screen should compile without diagnostics: {:?}",
            report.diagnostics
        );
        assert_eq!(
            report.ui_layouts.len(),
            1,
            "only the screen yields a layout"
        );
        assert_eq!(report.ui_layouts[0].0, "Stats");
        cleanup(&dir);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // compile_files tests (1)
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_compile_files_matches_compile_dirs() {
        let dir = tmp_dir("compile_files_equiv");
        fs::write(
            dir.join("intro.scene"),
            "\
game_scene Intro {
  @storyline(\"prof_ask\") {
    @trigger(map = \"ProfLab\", npc = \"Prof\")
    @speaker(\"Prof\") { \"Hello!\" }
  }
}",
        )
        .unwrap();
        fs::write(
            dir.join("menu.gui"),
            "screen MainMenu {\n  panel {\n    text(\"Hello\") {}\n  }\n}",
        )
        .unwrap();

        let disk_report = compile_dirs(&[dir.as_path()], None);

        let files: Vec<(String, String, String)> = vec![
            (
                "scene".to_string(),
                dir.join("intro.scene").to_string_lossy().into_owned(),
                fs::read_to_string(dir.join("intro.scene")).unwrap(),
            ),
            (
                "gui".to_string(),
                dir.join("menu.gui").to_string_lossy().into_owned(),
                fs::read_to_string(dir.join("menu.gui")).unwrap(),
            ),
        ];
        let mem_report = compile_files(&files, None);

        assert_eq!(mem_report.scenes, disk_report.scenes);
        assert_eq!(mem_report.ui_layouts, disk_report.ui_layouts);
        assert_eq!(mem_report.files, disk_report.files);
        assert_eq!(mem_report.diagnostics, disk_report.diagnostics);
        assert_eq!(mem_report.routes.len(), disk_report.routes.len());
        cleanup(&dir);
    }
}
