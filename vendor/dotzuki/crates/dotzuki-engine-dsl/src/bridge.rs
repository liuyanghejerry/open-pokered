use crate::loader::load_dsl_scenes;
use crate::loader::DslSceneRegistrar;

impl DslSceneRegistrar for dotzuki_engine_script::loader::ScriptLoader {
    fn register_scene_js(&mut self, name: &str, js: &str) {
        self.register_script(name, js);
    }

    fn register_ui_layout(&mut self, _name: &str, _json: &str) {
        // UI layouts are data artefacts stored separately from the script engine.
        // Access via the embedded_scenes module for direct inclusion.
    }
}

/// Register all DSL-compiled scenes with a `dotzuki-engine-script` `ScriptLoader`.
pub fn register_dsl_scenes(loader: &mut dotzuki_engine_script::loader::ScriptLoader) {
    load_dsl_scenes(loader);
}
