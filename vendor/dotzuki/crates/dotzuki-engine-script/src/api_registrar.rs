//! ScriptApiRegistrar trait — enables game-agnostic JS API registration.
//!
//! The core engine registers generic APIs (showText, moveNpc, getFlag, warpTo, etc.)
//! on the `game` global object. Game-specific APIs (giveMonster, startBattle, etc.)
//! are registered by implementors of this trait via `ScriptEngine::with_api()`.
//!
//! DSL scene management APIs (showScene, hideScene, updateUI) are registered as
//! core APIs during `ScriptEngine::new()`.

use crate::command::ScriptCommand;
use crate::engine::ScriptEngine;
use boa_engine::JsArgs;

/// Trait for registering game-specific JavaScript APIs.
///
/// Implementations add function properties to the `game` global JS object
/// (already created by the core engine) during `ScriptEngine` initialization.
///
/// # Example
///
/// ```ignore
/// struct MyGameApi;
/// impl ScriptApiRegistrar for MyGameApi {
///     fn register_api(&self, engine: &mut ScriptEngine) {
///         engine.register_async_fn("myCommand", |args, ctx| {
///             // ... build ScriptCommand ...
///         });
///     }
/// }
///
/// let engine = ScriptEngine::with_api(&MyGameApi);
/// ```
pub trait ScriptApiRegistrar {
    /// Register functions on the `game` JS object.
    /// Called during `ScriptEngine` initialization, after core APIs
    /// have been registered and the `game` global object is available.
    fn register_api(&self, engine: &mut ScriptEngine);
}

/// Register the `game.showScene()` JS API on the given engine.
///
/// Callable from JS as: `await game.showScene("shop")`
/// Produces `ScriptCommand::ShowScene { scene_name, layout_json: None }`.
pub fn register_show_scene(engine: &mut ScriptEngine) {
    engine.register_async_fn("showScene", |args, ctx| {
        let scene_name = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_lossy();
        Ok(ScriptCommand::ShowScene {
            scene_name,
            layout_json: None,
        })
    });
}

/// Register the `game.hideScene()` JS API on the given engine.
///
/// Callable from JS as: `await game.hideScene("shop")`
/// Produces `ScriptCommand::HideScene { scene_name }`.
pub fn register_hide_scene(engine: &mut ScriptEngine) {
    engine.register_async_fn("hideScene", |args, ctx| {
        let scene_name = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_lossy();
        Ok(ScriptCommand::HideScene { scene_name })
    });
}

/// Register the `game.updateUI()` JS API on the given engine.
///
/// Callable from JS as: `await game.updateUI("shop", { gold: 999 })`
/// The `data` argument is serialized via `JSON.stringify` by the Boa runtime.
/// Produces `ScriptCommand::UpdateUI { scene_name, data_json }`.
pub fn register_update_ui(engine: &mut ScriptEngine) {
    engine.register_async_fn("updateUI", |args, ctx| {
        let scene_name = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_lossy();
        let data_val = args.get_or_undefined(1);
        let json_val = data_val.to_json(ctx)?;
        let data_json = json_val.to_string();
        Ok(ScriptCommand::UpdateUI {
            scene_name,
            data_json,
        })
    });
}
