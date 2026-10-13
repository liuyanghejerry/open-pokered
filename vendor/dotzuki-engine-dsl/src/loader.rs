use std::collections::HashMap;

use crate::compiler::CompileReport;

/// A compiled DSL scene, ready for registration with a script engine.
#[derive(Debug, Clone)]
pub struct CompiledScene {
    pub name: String,
    pub js_source: String,
}

/// A compiled UI layout JSON string.
#[derive(Debug, Clone)]
pub struct CompiledUILayout {
    pub name: String,
    pub json: String,
}

/// Registry that accepts compiled DSL artefacts.
///
/// Implement this trait on your script-loader or engine type so that
/// `load_dsl_scenes` can push compiled scenes directly into it.
pub trait DslSceneRegistrar {
    fn register_scene_js(&mut self, name: &str, js: &str);
    fn register_ui_layout(&mut self, name: &str, json: &str);
}

include!(concat!(env!("OUT_DIR"), "/embedded_scenes.rs"));

/// Convenience adapter: implements `DslSceneRegistrar` by collecting
/// everything into `Vec`s — useful for testing or when you want to
/// inspect the compiled outputs before registration.
#[derive(Debug, Default)]
pub struct CollectingRegistrar {
    pub scenes: Vec<CompiledScene>,
    pub layouts: Vec<CompiledUILayout>,
    pub other: HashMap<String, String>,
}

impl DslSceneRegistrar for CollectingRegistrar {
    fn register_scene_js(&mut self, name: &str, js: &str) {
        self.scenes.push(CompiledScene {
            name: name.to_string(),
            js_source: js.to_string(),
        });
    }

    fn register_ui_layout(&mut self, name: &str, json: &str) {
        self.layouts.push(CompiledUILayout {
            name: name.to_string(),
            json: json.to_string(),
        });
    }
}

/// Load all DSL-compiled scenes that were embedded at build time.
///
/// Call this once during engine initialisation.
pub fn load_dsl_scenes(registrar: &mut impl DslSceneRegistrar) {
    load_embedded_scenes(registrar);
}

/// Register the scenes and UI layouts of an in-memory `CompileReport`
/// (produced by `compiler::compile_dirs`) with a scene registrar.
///
/// This is the runtime counterpart of `load_dsl_scenes`: instead of the
/// build-time embedded artifacts it registers artifacts compiled on the
/// fly, so a standalone game project can compile its own DSL directories
/// without a build step. Themes and styles have no registrar entry point
/// (same as the embedded path); access them via `report.themes` /
/// `report.styles`.
pub fn register_compiled(registrar: &mut impl DslSceneRegistrar, report: &CompileReport) {
    for (name, js, _source_path) in &report.scenes {
        registrar.register_scene_js(name, js);
    }
    for (name, json, _source_path) in &report.ui_layouts {
        registrar.register_ui_layout(name, json);
    }
}

/// Bridge adapter: wraps a `dotzuki_engine_script::loader::ScriptLoader`
/// so it can receive DSL-compiled scenes through the `DslSceneRegistrar`
/// trait.
///
/// # Usage
///
/// ```ignore
/// let mut loader = ScriptLoader::new();
/// let mut adapter = ScriptLoaderAdapter(&mut loader);
/// dotzuki_engine_dsl::loader::load_dsl_scenes(&mut adapter);
/// ```
pub struct ScriptLoaderAdapter<'a>(pub &'a mut dyn ScriptLoaderLike);

/// Minimal trait that abstracts the `register_script` method from
/// `dotzuki-engine-script`'s `ScriptLoader`.  Implementors only need to
/// provide `register_script`.
pub trait ScriptLoaderLike {
    fn register_script(&mut self, map_id: &str, source: &str);
}

impl<'a> DslSceneRegistrar for ScriptLoaderAdapter<'a> {
    fn register_scene_js(&mut self, name: &str, js: &str) {
        self.0.register_script(name, js);
    }

    fn register_ui_layout(&mut self, _name: &str, _json: &str) {
        // UI layouts are data artefacts — they are stored separately
        // from the script engine. The `_json` value is available via
        // the `embedded_scenes` module for direct inclusion.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collecting_registrar_basic() {
        let mut reg = CollectingRegistrar::default();
        reg.register_scene_js("shop", "export async function storyline_main() {}");
        reg.register_ui_layout("shop", r#"{"type":"group","children":[]}"#);

        assert_eq!(reg.scenes.len(), 1);
        assert_eq!(reg.scenes[0].name, "shop");
        assert!(reg.scenes[0].js_source.contains("storyline_main"));

        assert_eq!(reg.layouts.len(), 1);
        assert_eq!(reg.layouts[0].name, "shop");
        assert!(reg.layouts[0].json.contains("\"group\""));
    }

    #[test]
    fn test_collecting_registrar_empty() {
        let reg = CollectingRegistrar::default();
        assert!(reg.scenes.is_empty());
        assert!(reg.layouts.is_empty());
    }

    #[test]
    fn test_register_compiled_pushes_report_contents() {
        let report = CompileReport {
            scenes: vec![
                ("intro".into(), "js_intro".into(), "intro.scene".into()),
                ("shop".into(), "js_shop".into(), "shop.scene".into()),
            ],
            ui_layouts: vec![("menu".into(), "{}".into(), "menu.gui".into())],
            ..Default::default()
        };

        let mut reg = CollectingRegistrar::default();
        register_compiled(&mut reg, &report);

        assert_eq!(reg.scenes.len(), 2);
        assert_eq!(reg.scenes[0].name, "intro");
        assert_eq!(reg.scenes[0].js_source, "js_intro");
        assert_eq!(reg.scenes[1].name, "shop");
        assert_eq!(reg.layouts.len(), 1);
        assert_eq!(reg.layouts[0].name, "menu");
        assert_eq!(reg.layouts[0].json, "{}");
    }

    #[test]
    fn test_register_compiled_empty_report() {
        let report = CompileReport::default();
        let mut reg = CollectingRegistrar::default();
        register_compiled(&mut reg, &report);
        assert!(reg.scenes.is_empty());
        assert!(reg.layouts.is_empty());
    }
}
