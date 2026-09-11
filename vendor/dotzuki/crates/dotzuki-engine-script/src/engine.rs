use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use boa_engine::builtins::promise::PromiseState;
#[cfg(target_arch = "wasm32")]
use boa_engine::module::IdleModuleLoader;
#[cfg(not(target_arch = "wasm32"))]
use boa_engine::module::SimpleModuleLoader;
use boa_engine::object::builtins::{JsFunction, JsPromise};
use boa_engine::property::Attribute;
use boa_engine::{
    js_string, Context, JsArgs, JsNativeError, JsResult, JsValue, Module, NativeFunction, Source,
};

use crate::api_registrar::ScriptApiRegistrar;
use crate::command::{CommandResult, ScriptCommand};

#[derive(Debug, thiserror::Error)]
pub enum ScriptEngineError {
    #[error("JS error: {0}")]
    JsError(String),
    #[error("Script not found for map: {0}")]
    ScriptNotFound(String),
    #[error("Function not found: {0}")]
    FunctionNotFound(String),
    #[error("Engine not initialized")]
    NotInitialized,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineState {
    Idle,
    Running,
    WaitingForCommand,
    Finished,
}

struct PendingResolve {
    resolve_fn: JsFunction,
}

/// Shared state between the JS runtime and the Rust game loop.
/// Commands issued by JS `await game.showText(...)` are placed here;
/// the game loop reads them, executes the operation, then calls `signal_done`.
pub struct SharedBridge {
    pending_command: Option<ScriptCommand>,
    pending_resolve: Option<PendingResolve>,
    flags: std::collections::HashMap<String, bool>,
    /// Generic, game-agnostic seeded query state read by synchronous JS
    /// query functions (registered via `register_sync_fn`). The core engine
    /// does not know what these keys mean — the game layer seeds them and
    /// registers named query functions that interpret them.
    numbers: std::collections::HashMap<String, f64>,
    texts: std::collections::HashMap<String, String>,
    sets: std::collections::HashMap<String, std::collections::HashSet<String>>,
    player_x: u8,
    player_y: u8,
    pub lang: String,
    /// State for the script-side RNG used by `game.showRandomText(...)` (and any
    /// future `randInt`-style primitives). Game scripts have no `Math.random` /
    /// `Date.now`, so all randomness must originate on the Rust side: the game
    /// layer mixes real entropy in via [`ScriptEngine::mix_rng`], and tests can
    /// pin a deterministic stream via [`ScriptEngine::seed_rng`].
    rng_state: u64,
}

/// Non-zero default seed (a common splitmix64/golden-ratio constant). Keeping the
/// state non-zero matters because xorshift64 is stuck at 0.
const DEFAULT_RNG_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

impl SharedBridge {
    fn new() -> Self {
        Self {
            pending_command: None,
            pending_resolve: None,
            flags: std::collections::HashMap::new(),
            numbers: std::collections::HashMap::new(),
            texts: std::collections::HashMap::new(),
            sets: std::collections::HashMap::new(),
            player_x: 0,
            player_y: 0,
            lang: "en".to_string(),
            rng_state: DEFAULT_RNG_SEED,
        }
    }

    /// Advance the internal xorshift64 RNG and return the next 64-bit value.
    fn next_rand(&mut self) -> u64 {
        let mut x = self.rng_state;
        if x == 0 {
            x = DEFAULT_RNG_SEED;
        }
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        x
    }
}

/// Read-only view over the seeded query state of a [`SharedBridge`].
///
/// Passed to synchronous query closures registered via
/// [`ScriptEngine::register_sync_fn`] so they can answer `@if`-style
/// conditions without issuing a command or awaiting a promise.
pub struct BridgeView<'a> {
    inner: &'a SharedBridge,
}

impl<'a> BridgeView<'a> {
    /// Numeric seeded value (defaults to `0.0`).
    pub fn number(&self, k: &str) -> f64 {
        self.inner.numbers.get(k).copied().unwrap_or(0.0)
    }
    /// Text seeded value (defaults to empty string).
    pub fn text(&self, k: &str) -> String {
        self.inner.texts.get(k).cloned().unwrap_or_default()
    }
    /// Whether the seeded set `k` contains `v`.
    pub fn set_contains(&self, k: &str, v: &str) -> bool {
        self.inner.sets.get(k).is_some_and(|s| s.contains(v))
    }
    /// Boolean flag value (defaults to `false`).
    pub fn flag(&self, k: &str) -> bool {
        self.inner.flags.get(k).copied().unwrap_or(false)
    }
}

pub struct ScriptEngine {
    context: Context,
    bridge: Rc<RefCell<SharedBridge>>,
    state: EngineState,
    /// The currently loaded ES6 module (holds exported function bindings).
    current_module: Option<Module>,
}

impl ScriptEngine {
    pub fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        let mut context = Context::builder()
            .module_loader(Rc::new(IdleModuleLoader))
            .build()
            .expect("failed to build JS context");

        #[cfg(not(target_arch = "wasm32"))]
        let mut context = Context::builder()
            .module_loader(Rc::new(SimpleModuleLoader::new(".").expect(
                "failed to create module loader (current directory must exist)",
            )))
            .build()
            .expect("failed to build JS context");
        let bridge = Rc::new(RefCell::new(SharedBridge::new()));

        register_core_game_api(&mut context, bridge.clone());

        Self {
            context,
            bridge,
            state: EngineState::Idle,
            current_module: None,
        }
    }

    pub fn state(&self) -> &EngineState {
        &self.state
    }

    pub fn is_idle(&self) -> bool {
        self.state == EngineState::Idle
    }

    pub fn is_waiting(&self) -> bool {
        self.state == EngineState::WaitingForCommand
    }

    pub fn set_flag(&mut self, flag: &str, value: bool) {
        self.bridge
            .borrow_mut()
            .flags
            .insert(flag.to_string(), value);
    }

    pub fn get_flag(&self, flag: &str) -> bool {
        self.bridge
            .borrow()
            .flags
            .get(flag)
            .copied()
            .unwrap_or(false)
    }

    /// Return a snapshot of all flags currently held in the bridge.
    /// Used by the overworld to persist flags across map transitions.
    pub fn get_all_flags(&self) -> std::collections::HashMap<String, bool> {
        self.bridge.borrow().flags.clone()
    }

    /// Bulk-insert flags into the bridge (additive — does not clear existing).
    /// Called after creating a new ScriptEngine to restore persistent flags.
    pub fn seed_flags(&mut self, flags: &std::collections::HashMap<String, bool>) {
        let mut b = self.bridge.borrow_mut();
        for (k, v) in flags {
            b.flags.insert(k.clone(), *v);
        }
    }

    /// Pin the script-side RNG to a deterministic starting state. Intended for
    /// tests; a value of `0` is treated as the default non-zero seed.
    pub fn seed_rng(&mut self, seed: u64) {
        self.bridge.borrow_mut().rng_state = if seed == 0 { DEFAULT_RNG_SEED } else { seed };
    }

    /// Mix externally-sourced entropy into the script-side RNG. The game layer
    /// calls this (e.g. once per frame with a draw from the overworld RNG) so
    /// `game.showRandomText(...)` picks vary between playthroughs even though
    /// scripts themselves have no access to `Math.random`/`Date.now`.
    pub fn mix_rng(&mut self, entropy: u64) {
        let mut b = self.bridge.borrow_mut();
        b.rng_state ^= entropy.wrapping_mul(0x2545_F491_4F6C_DD1D);
        if b.rng_state == 0 {
            b.rng_state = DEFAULT_RNG_SEED;
        }
    }

    /// Seed a numeric value read by synchronous query functions.
    pub fn seed_number(&mut self, k: &str, v: f64) {
        self.bridge.borrow_mut().numbers.insert(k.into(), v);
    }

    /// Seed a text value read by synchronous query functions.
    pub fn seed_text(&mut self, k: &str, v: &str) {
        self.bridge.borrow_mut().texts.insert(k.into(), v.into());
    }

    /// Seed a string set read by synchronous query functions
    /// (e.g. the player's bag, as a set of item constant names).
    pub fn seed_set(&mut self, k: &str, vals: &[String]) {
        self.bridge
            .borrow_mut()
            .sets
            .insert(k.into(), vals.iter().cloned().collect());
    }

    pub fn set_player_position(&mut self, x: u8, y: u8) {
        self.bridge.borrow_mut().player_x = x;
        self.bridge.borrow_mut().player_y = y;
    }

    pub fn set_lang(&mut self, lang: &str) {
        self.bridge.borrow_mut().lang = lang.to_string();
    }

    pub fn load_script(&mut self, source: &str) -> Result<(), ScriptEngineError> {
        log::info!(target: "dotzuki::overworld", "[ScriptEngine] load_script: {} bytes", source.len());
        let src = Source::from_reader(source.as_bytes(), Some(Path::new("script.mjs")));
        let module = Module::parse(src, None, &mut self.context).map_err(|e| {
            log::warn!(target: "dotzuki::overworld", "[ScriptEngine] Module parse failed: {}", e);
            ScriptEngineError::JsError(e.to_string())
        })?;

        self.context
            .module_loader()
            .register_module(js_string!("script.mjs"), module.clone());

        let promise = module.load_link_evaluate(&mut self.context);
        self.context.run_jobs();

        match promise.state() {
            PromiseState::Fulfilled(_) => {
                log::info!(target: "dotzuki::overworld", "[ScriptEngine] Module evaluated OK");
            }
            PromiseState::Rejected(err) => {
                log::warn!(target: "dotzuki::overworld", "[ScriptEngine] Module evaluation rejected: {:?}", err);
                return Err(ScriptEngineError::JsError(format!(
                    "Module evaluation failed: {:?}",
                    err
                )));
            }
            PromiseState::Pending => {
                log::warn!(target: "dotzuki::overworld", "[ScriptEngine] Module evaluation stuck pending");
                return Err(ScriptEngineError::JsError(
                    "Module evaluation stuck in pending state".to_string(),
                ));
            }
        }

        self.current_module = Some(module);

        if let Some(ref m) = self.current_module {
            for name in &[
                "enterMap",
                "talkNurse",
                "talkLinkReceptionist",
                "talkGentleman",
            ] {
                let has = m
                    .get_value(js_string!(*name), &mut self.context)
                    .map(|v| v.is_callable())
                    .unwrap_or(false);
                log::info!(target: "dotzuki::overworld", "[ScriptEngine] Export check: {} = {}", name, has);
            }
        }

        Ok(())
    }

    pub fn load_shared_module(
        &mut self,
        name: &str,
        source: &str,
    ) -> Result<(), ScriptEngineError> {
        log::info!(target: "dotzuki::overworld", "[ScriptEngine] load_shared_module '{}': {} bytes", name, source.len());
        let src = Source::from_reader(source.as_bytes(), Some(Path::new(name)));
        let module = Module::parse(src, None, &mut self.context)
            .map_err(|e| {
                log::warn!(target: "dotzuki::overworld", "[ScriptEngine] Shared module parse failed: {}", e);
                ScriptEngineError::JsError(e.to_string())
            })?;

        self.context
            .module_loader()
            .register_module(js_string!(name), module.clone());

        let promise = module.load_link_evaluate(&mut self.context);
        self.context.run_jobs();

        match promise.state() {
            PromiseState::Fulfilled(_) => {
                log::info!(target: "dotzuki::overworld", "[ScriptEngine] Shared module '{}' evaluated OK", name);
                if let Ok(val) = module.get_value(js_string!("talkNurse"), &mut self.context) {
                    log::info!(target: "dotzuki::overworld", "[ScriptEngine] Shared module talkNurse callable: {}", val.is_callable());
                }
            }
            PromiseState::Rejected(err) => {
                log::warn!(target: "dotzuki::overworld", "[ScriptEngine] Shared module '{}' rejected: {:?}", name, err);
                return Err(ScriptEngineError::JsError(format!(
                    "Shared module evaluation failed: {:?}",
                    err
                )));
            }
            PromiseState::Pending => {
                log::warn!(target: "dotzuki::overworld", "[ScriptEngine] Shared module '{}' stuck pending", name);
                return Err(ScriptEngineError::JsError(
                    "Shared module evaluation stuck in pending state".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Call a JS async function by name (e.g., "scriptDefault", "talkProf").
    /// The function must be `export`-ed from the loaded module.
    /// Returns the first ScriptCommand if the function immediately awaits one.
    pub fn call_function(
        &mut self,
        fn_name: &str,
        args: &[JsValue],
    ) -> Result<Option<ScriptCommand>, ScriptEngineError> {
        // Resolve `talkMom` → `storyline_talkMom` etc. (see `resolved_fn_name`).
        let resolved = self
            .resolved_fn_name(fn_name)
            .unwrap_or_else(|| fn_name.to_string());
        let fn_name = resolved.as_str();
        log::info!(target: "dotzuki::overworld", "[ScriptEngine] call_function: {}", fn_name);
        let module = self
            .current_module
            .as_ref()
            .ok_or(ScriptEngineError::NotInitialized)?;

        let func = module
            .get_value(js_string!(fn_name), &mut self.context)
            .map_err(|e| {
                log::warn!(target: "dotzuki::overworld", "[ScriptEngine] get_value error for {}: {}", fn_name, e);
                ScriptEngineError::JsError(e.to_string())
            })?;

        if func.is_undefined() || func.is_null() {
            log::warn!(target: "dotzuki::overworld", "[ScriptEngine] Function '{}' is undefined or null", fn_name);
            return Err(ScriptEngineError::FunctionNotFound(fn_name.to_string()));
        }

        let func_obj = func
            .as_callable()
            .ok_or_else(|| {
                log::warn!(target: "dotzuki::overworld", "[ScriptEngine] Function '{}' is not callable", fn_name);
                ScriptEngineError::FunctionNotFound(fn_name.to_string())
            })?;

        log::info!(target: "dotzuki::overworld", "[ScriptEngine] Calling function '{}'...", fn_name);
        let result = func_obj.call(&JsValue::undefined(), args, &mut self.context);

        match result {
            Ok(_) => {
                log::info!(target: "dotzuki::overworld", "[ScriptEngine] Function '{}' call succeeded", fn_name);
            }
            Err(e) => {
                log::warn!(target: "dotzuki::overworld", "[ScriptEngine] Function '{}' call failed: {}", fn_name, e);
                return Err(ScriptEngineError::JsError(e.to_string()));
            }
        }

        self.context.run_jobs();

        self.state = EngineState::Running;
        let cmd = self.check_pending_command()?;
        log::info!(target: "dotzuki::overworld", "[ScriptEngine] After call_function '{}': pending_command = {:?}", fn_name, cmd.is_some());
        Ok(cmd)
    }

    /// Called each frame by the game loop.
    /// Returns the current pending command if the script is waiting.
    pub fn tick(&mut self) -> Option<ScriptCommand> {
        match self.state {
            EngineState::WaitingForCommand => self.bridge.borrow().pending_command.clone(),
            EngineState::Idle | EngineState::Finished => None,
            EngineState::Running => match self.check_pending_command() {
                Ok(cmd) => cmd,
                Err(_) => {
                    self.state = EngineState::Finished;
                    None
                }
            },
        }
    }

    /// Signal that the game has completed the pending command.
    /// Resolves the JS promise so the async function can continue.
    pub fn signal_done(
        &mut self,
        result: CommandResult,
    ) -> Result<Option<ScriptCommand>, ScriptEngineError> {
        if self.state != EngineState::WaitingForCommand {
            return Ok(None);
        }

        let resolve = self.bridge.borrow_mut().pending_resolve.take();
        self.bridge.borrow_mut().pending_command = None;

        if let Some(pending) = resolve {
            let js_result = command_result_to_js(&result, &mut self.context);
            pending
                .resolve_fn
                .call(&JsValue::undefined(), &[js_result], &mut self.context)
                .map_err(|e| ScriptEngineError::JsError(e.to_string()))?;

            self.context.run_jobs();
        }

        self.state = EngineState::Running;
        self.check_pending_command()
    }

    fn check_pending_command(&mut self) -> Result<Option<ScriptCommand>, ScriptEngineError> {
        let cmd = self.bridge.borrow().pending_command.clone();
        if cmd.is_some() {
            self.state = EngineState::WaitingForCommand;
        } else if self.state == EngineState::Running {
            self.state = EngineState::Idle;
        }
        Ok(cmd)
    }

    /// Register an async command function on the `game` global JS object.
    ///
    /// The `builder` closure receives JS arguments and returns a `ScriptCommand`.
    /// The engine automatically creates a Promise, stores the command + resolve
    /// function in the bridge, and returns the Promise to JS.
    ///
    /// This is the building block for `ScriptApiRegistrar` implementations.
    pub fn register_async_fn(
        &mut self,
        name: &str,
        builder: impl Fn(&[JsValue], &mut Context) -> JsResult<ScriptCommand> + 'static,
    ) {
        let bridge = self.bridge.clone();
        let func = unsafe {
            NativeFunction::from_closure(move |_this, args, ctx| {
                let (promise, resolvers) = JsPromise::new_pending(ctx);
                let cmd = builder(args, ctx)?;
                let mut b = bridge.borrow_mut();
                b.pending_command = Some(cmd);
                b.pending_resolve = Some(PendingResolve {
                    resolve_fn: resolvers.resolve,
                });
                Ok(promise.into())
            })
        };
        let game_obj = self
            .context
            .global_object()
            .get(js_string!("game"), &mut self.context)
            .expect("game global not found")
            .to_object(&mut self.context)
            .expect("game global is not an object");
        game_obj
            .set(
                js_string!(name),
                func.to_js_function(self.context.realm()),
                true,
                &mut self.context,
            )
            .unwrap_or_else(|_| panic!("failed to register game.{}", name));
    }

    /// Register a *synchronous* query function on the `game` global JS object.
    ///
    /// Unlike [`register_async_fn`](Self::register_async_fn), the closure returns
    /// a `JsValue` directly (no promise, no pending command). It is handed a
    /// read-only [`BridgeView`] over the seeded query state so it can answer
    /// `@if`-style conditions immediately.
    pub fn register_sync_fn<F>(&mut self, name: &str, f: F)
    where
        F: Fn(&[JsValue], &mut Context, &BridgeView) -> JsResult<JsValue> + 'static,
    {
        let bridge = self.bridge.clone();
        // SAFETY: closure captures only `Rc<RefCell<SharedBridge>>` which holds no
        // GC-traced (boa `Trace`) types, so it cannot cause use-after-free.
        let func = unsafe {
            NativeFunction::from_closure(move |_this, args, ctx| {
                let b = bridge.borrow();
                let view = BridgeView { inner: &b };
                f(args, ctx, &view)
            })
        };
        let game_obj = self
            .context
            .global_object()
            .get(js_string!("game"), &mut self.context)
            .expect("game global not found")
            .to_object(&mut self.context)
            .expect("game global is not an object");
        game_obj
            .set(
                js_string!(name),
                func.to_js_function(self.context.realm()),
                true,
                &mut self.context,
            )
            .unwrap_or_else(|_| panic!("failed to register game.{}", name));
    }

    /// Construct a `ScriptEngine` with a game-specific API registrar.
    ///
    /// Core APIs (showText, moveNpc, getFlag, warpTo, playMusic, etc.) are always
    /// registered. The `registrar` adds game-specific APIs such as `giveMonster`,
    /// `startBattle`, etc.
    pub fn with_api(registrar: &dyn ScriptApiRegistrar) -> Self {
        let mut engine = Self::new();
        registrar.register_api(&mut engine);
        engine
    }
}

impl Default for ScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Convenience call methods ─────────────────────────────────────
// These allow pokered-core to call JS functions without depending on boa_engine directly.

impl ScriptEngine {
    /// Call a JS function with no arguments.
    pub fn call_function_no_args(
        &mut self,
        fn_name: &str,
    ) -> Result<Option<ScriptCommand>, ScriptEngineError> {
        self.call_function(fn_name, &[])
    }

    /// Call a JS function with a single u8 argument (e.g., npc text_id lookup).
    pub fn call_function_with_u8(
        &mut self,
        fn_name: &str,
        arg: u8,
    ) -> Result<Option<ScriptCommand>, ScriptEngineError> {
        self.call_function(fn_name, &[JsValue::from(arg as i32)])
    }

    /// Call a JS function with two u16 arguments (e.g., coord event trigger).
    pub fn call_function_with_xy(
        &mut self,
        fn_name: &str,
        x: u16,
        y: u16,
    ) -> Result<Option<ScriptCommand>, ScriptEngineError> {
        self.call_function(fn_name, &[JsValue::from(x as i32), JsValue::from(y as i32)])
    }

    /// Call a JS function with a single string argument.
    pub fn call_function_with_str(
        &mut self,
        fn_name: &str,
        arg: &str,
    ) -> Result<Option<ScriptCommand>, ScriptEngineError> {
        self.call_function(fn_name, &[JsValue::from(js_string!(arg))])
    }

    /// Resolve a trigger/binding name to the exported function that actually
    /// exists in the current module. Configs bind the *bare* name (e.g.
    /// `talkMom`, `SeafoamIslandsB4FOnLoad`) but the DSL compiler exports
    /// `@storyline` blocks under a `storyline_`-prefixed name
    /// (`storyline_talkMom`). Try the exact name first (so `onLoad` names and
    /// any bare `.js` functions still win), then the `storyline_` fallback.
    fn resolved_fn_name(&mut self, fn_name: &str) -> Option<String> {
        let module = self.current_module.clone()?;
        if matches!(module.get_value(js_string!(fn_name), &mut self.context), Ok(v) if v.is_callable())
        {
            return Some(fn_name.to_string());
        }
        let prefixed = format!("storyline_{fn_name}");
        if matches!(module.get_value(js_string!(prefixed.as_str()), &mut self.context), Ok(v) if v.is_callable())
        {
            return Some(prefixed);
        }
        None
    }

    /// Check if a JS function exists in the module's exports (matching the
    /// `storyline_` resolution used by [`Self::call_function`]).
    pub fn has_function(&mut self, fn_name: &str) -> bool {
        self.resolved_fn_name(fn_name).is_some()
    }
}

fn command_result_to_js(result: &CommandResult, _context: &mut Context) -> JsValue {
    match result {
        CommandResult::Void => JsValue::undefined(),
        CommandResult::Bool(b) => JsValue::from(*b),
        CommandResult::Number(n) => JsValue::from(*n),
        CommandResult::Text(s) => JsValue::from(js_string!(s.as_str())),
    }
}

fn register_core_game_api(context: &mut Context, bridge: Rc<RefCell<SharedBridge>>) {
    let mut game_obj = boa_engine::object::ObjectInitializer::new(context);
    let game_obj = game_obj.build();

    let lang_bridge = bridge.clone();
    let lang_fn = unsafe {
        NativeFunction::from_closure(
            move |_this: &JsValue, _args: &[JsValue], _ctx: &mut Context| -> JsResult<JsValue> {
                Ok(JsValue::from(js_string!(lang_bridge
                    .borrow()
                    .lang
                    .as_str())))
            },
        )
    };
    game_obj
        .set(
            js_string!("lang"),
            lang_fn.to_js_function(context.realm()),
            true,
            context,
        )
        .expect("failed to register game.lang");

    let t_bridge = bridge.clone();
    let t_fn = unsafe {
        NativeFunction::from_closure(
            move |_this: &JsValue, args: &[JsValue], ctx: &mut Context| -> JsResult<JsValue> {
                let en = args
                    .get_or_undefined(0)
                    .to_string(ctx)
                    .map_or(String::new(), |s| s.to_std_string_lossy());
                let zh = args
                    .get_or_undefined(1)
                    .to_string(ctx)
                    .map_or(String::new(), |s| s.to_std_string_lossy());
                let result = if t_bridge.borrow().lang == "zh" {
                    zh
                } else {
                    en
                };
                Ok(JsValue::from(js_string!(result)))
            },
        )
    };
    game_obj
        .set(
            js_string!("t"),
            t_fn.to_js_function(context.realm()),
            true,
            context,
        )
        .expect("failed to register game.t");

    // game.showRandomText(a, b, c, ...) OR game.showRandomText([a, b, c])
    //   -> Promise<void>
    // Picks one line at random (Rust-side RNG) and shows it, exactly like
    // game.showText. Used for original flavor-text pools where an NPC/sign picks
    // a line from a set each interaction (e.g. gossip NPCs, the cruise ship
    // chefs). Resolves like showText once the box is dismissed.
    let rand_text_bridge = bridge.clone();
    // SAFETY: captures only `Rc<RefCell<SharedBridge>>`, which holds no
    // GC-traced (boa `Trace`) types, so it cannot cause use-after-free.
    let rand_text_fn = unsafe {
        NativeFunction::from_closure(
            move |_this: &JsValue, args: &[JsValue], ctx: &mut Context| -> JsResult<JsValue> {
                // Accept a single array argument, or a variadic list of strings.
                let mut options: Vec<String> = Vec::new();
                if args.len() == 1 && args[0].is_object() {
                    let obj = args[0].to_object(ctx)?;
                    let len = obj.get(js_string!("length"), ctx)?.to_u32(ctx)?;
                    for i in 0..len {
                        options.push(obj.get(i, ctx)?.to_string(ctx)?.to_std_string_lossy());
                    }
                } else {
                    for a in args {
                        options.push(a.to_string(ctx)?.to_std_string_lossy());
                    }
                }

                let (promise, resolvers) = JsPromise::new_pending(ctx);

                let mut b = rand_text_bridge.borrow_mut();
                let text = if options.is_empty() {
                    String::new()
                } else {
                    let idx = (b.next_rand() % options.len() as u64) as usize;
                    options.swap_remove(idx)
                };
                b.pending_command = Some(ScriptCommand::ShowText { text });
                b.pending_resolve = Some(PendingResolve {
                    resolve_fn: resolvers.resolve,
                });

                Ok(promise.into())
            },
        )
    };
    game_obj
        .set(
            js_string!("showRandomText"),
            rand_text_fn.to_js_function(context.realm()),
            true,
            context,
        )
        .expect("failed to register game.showRandomText");

    macro_rules! register_async_command {
        ($name:expr, $bridge:expr, $context:expr, $game_obj:expr, $cmd_builder:expr) => {{
            let bridge = $bridge.clone();
            // SAFETY: The closure captures only `Rc<RefCell<SharedBridge>>` which contains no
            // GC-traced (boa `Trace`) types, so it cannot cause use-after-free with the GC.
            let func = unsafe {
                NativeFunction::from_closure(move |_this, args, ctx| {
                    let (promise, resolvers) = JsPromise::new_pending(ctx);

                    let cmd = ($cmd_builder)(args, ctx)?;

                    let mut b = bridge.borrow_mut();
                    b.pending_command = Some(cmd);
                    b.pending_resolve = Some(PendingResolve {
                        resolve_fn: resolvers.resolve,
                    });

                    Ok(promise.into())
                })
            };
            $game_obj
                .set(
                    js_string!($name),
                    func.to_js_function($context.realm()),
                    true,
                    $context,
                )
                .expect(concat!("failed to register game.", $name));
        }};
    }

    // game.showText(text: string) -> Promise<void>
    register_async_command!(
        "showText",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let text = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            Ok(ScriptCommand::ShowText { text })
        }
    );

    // game.showChoice(options: string[]) -> Promise<number>
    register_async_command!(
        "showChoice",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let arr = args.get_or_undefined(0).to_object(ctx)?;
            let len = arr.get(js_string!("length"), ctx)?.to_u32(ctx)?;
            let mut options = Vec::new();
            for i in 0..len {
                let val = arr.get(i, ctx)?;
                options.push(val.to_string(ctx)?.to_std_string_lossy());
            }
            Ok(ScriptCommand::ShowChoice { options })
        }
    );

    // game.moveNpc(npcId: string, path: [number, number][]) -> Promise<void>
    register_async_command!(
        "moveNpc",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let npc_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            let arr = args.get_or_undefined(1).to_object(ctx)?;
            let len = arr.get(js_string!("length"), ctx)?.to_u32(ctx)?;
            let mut path = Vec::new();
            for i in 0..len {
                let point = arr.get(i, ctx)?.to_object(ctx)?;
                let x = point.get(0, ctx)?.to_u32(ctx)? as u8;
                let y = point.get(1, ctx)?.to_u32(ctx)? as u8;
                path.push((x, y));
            }
            Ok(ScriptCommand::MoveNpc { npc_id, path })
        }
    );

    // game.startNpcMove(npcId: string, path: [number, number][]) -> Promise<void>
    // Fire-and-forget: starts NPC moving along path, resolves immediately.
    register_async_command!(
        "startNpcMove",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let npc_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            let arr = args.get_or_undefined(1).to_object(ctx)?;
            let len = arr.get(js_string!("length"), ctx)?.to_u32(ctx)?;
            let mut path = Vec::new();
            for i in 0..len {
                let point = arr.get(i, ctx)?.to_object(ctx)?;
                let x = point.get(0, ctx)?.to_u32(ctx)? as u8;
                let y = point.get(1, ctx)?.to_u32(ctx)? as u8;
                path.push((x, y));
            }
            Ok(ScriptCommand::StartNpcMove { npc_id, path })
        }
    );

    // game.awaitNpcMove(npcId: string) -> Promise<void>
    // Blocks until the NPC's scripted path is complete.
    register_async_command!(
        "awaitNpcMove",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let npc_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            Ok(ScriptCommand::AwaitNpcMove { npc_id })
        }
    );

    // game.movePlayer(path: [number, number][]) -> Promise<void>
    // Blocks until the player finishes walking the path.
    register_async_command!(
        "movePlayer",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let arr = args.get_or_undefined(0).to_object(ctx)?;
            let len = arr.get(js_string!("length"), ctx)?.to_u32(ctx)?;
            let mut path = Vec::new();
            for i in 0..len {
                let point = arr.get(i, ctx)?.to_object(ctx)?;
                let x = point.get(0, ctx)?.to_u32(ctx)? as u8;
                let y = point.get(1, ctx)?.to_u32(ctx)? as u8;
                path.push((x, y));
            }
            Ok(ScriptCommand::MovePlayer { path })
        }
    );

    // game.movePlayerRelative(steps: ([number, number] | DirectionString)[]) -> Promise<void>
    // Each entry is a (dx, dy) delta (or a direction string) applied
    // cumulatively from the player's current position; the deltas are
    // resolved to absolute waypoints by the game core when the command
    // runs. Blocks until the player finishes walking.
    register_async_command!(
        "movePlayerRelative",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let arr = args.get_or_undefined(0).to_object(ctx)?;
            let len = arr.get(js_string!("length"), ctx)?.to_u32(ctx)?;
            let mut steps = Vec::new();
            for i in 0..len {
                let entry = arr.get(i, ctx)?;
                if entry.is_string() {
                    let dir = entry.to_string(ctx)?.to_std_string_lossy();
                    let delta = match dir.to_ascii_lowercase().as_str() {
                        "up" | "north" => (0i16, -1i16),
                        "down" | "south" => (0, 1),
                        "left" | "west" => (-1, 0),
                        "right" | "east" => (1, 0),
                        other => {
                            return Err(JsNativeError::typ()
                                .with_message(format!(
                                    "movePlayerRelative: unknown direction '{other}'"
                                ))
                                .into())
                        }
                    };
                    steps.push(delta);
                } else {
                    let point = entry.to_object(ctx)?;
                    let dx = point.get(0, ctx)?.to_i32(ctx)? as i16;
                    let dy = point.get(1, ctx)?.to_i32(ctx)? as i16;
                    steps.push((dx, dy));
                }
            }
            Ok(ScriptCommand::MovePlayerRelative { steps })
        }
    );

    // game.moveNpcTo(npcId: string, x: number, y: number) -> Promise<void>
    // Plans a terrain-aware path and resolves when movement is done.
    register_async_command!(
        "moveNpcTo",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let npc_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            let x = args.get_or_undefined(1).to_u32(ctx)? as u8;
            let y = args.get_or_undefined(2).to_u32(ctx)? as u8;
            Ok(ScriptCommand::MoveNpcTo { npc_id, x, y })
        }
    );

    // game.startNpcMoveTo(npcId: string, x: number, y: number) -> Promise<void>
    // Plans a terrain-aware path, starts movement immediately and resolves at once.
    register_async_command!(
        "startNpcMoveTo",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let npc_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            let x = args.get_or_undefined(1).to_u32(ctx)? as u8;
            let y = args.get_or_undefined(2).to_u32(ctx)? as u8;
            Ok(ScriptCommand::StartNpcMoveTo { npc_id, x, y })
        }
    );

    // game.movePlayerTo(x: number, y: number) -> Promise<void>
    // Plans a terrain-aware path and resolves when movement is done.
    register_async_command!(
        "movePlayerTo",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let x = args.get_or_undefined(0).to_u32(ctx)? as u8;
            let y = args.get_or_undefined(1).to_u32(ctx)? as u8;
            Ok(ScriptCommand::MovePlayerTo { x, y })
        }
    );

    // game.faceNpc(npcId: string, direction: string) -> Promise<void>
    register_async_command!(
        "faceNpc",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let npc_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            let direction = args
                .get_or_undefined(1)
                .to_string(ctx)?
                .to_std_string_lossy();
            Ok(ScriptCommand::FaceNpc { npc_id, direction })
        }
    );

    // game.facePlayer(direction: string) -> Promise<void>
    register_async_command!(
        "facePlayer",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let direction = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            Ok(ScriptCommand::FacePlayer { direction })
        }
    );

    // game.setNpcFrame(npcId: string, frame: number) -> Promise<void>
    register_async_command!(
        "setNpcFrame",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let npc_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            let frame = args.get_or_undefined(1).to_number(ctx)? as u8;
            Ok(ScriptCommand::SetNpcFrame { npc_id, frame })
        }
    );

    // game.playMusic(musicId: string) -> Promise<void>
    register_async_command!(
        "playMusic",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let music_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            Ok(ScriptCommand::PlayMusic { music_id })
        }
    );

    // game.playSound(soundId: string) -> Promise<void>
    register_async_command!(
        "playSound",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let sound_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            Ok(ScriptCommand::PlaySound { sound_id })
        }
    );

    // game.stopMusic() -> Promise<void>
    register_async_command!(
        "stopMusic",
        bridge,
        context,
        game_obj,
        |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
            Ok(ScriptCommand::StopMusic)
        }
    );

    // game.fadeOutMusic() -> Promise<void>
    register_async_command!(
        "fadeOutMusic",
        bridge,
        context,
        game_obj,
        |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
            Ok(ScriptCommand::FadeOutMusic)
        }
    );

    // game.delay(frames: number) -> Promise<void>
    register_async_command!("delay", bridge, context, game_obj, |args: &[JsValue],
                                                                 ctx: &mut Context|
     -> JsResult<
        ScriptCommand,
    > {
        let frames = args.get_or_undefined(0).to_u32(ctx)? as u16;
        Ok(ScriptCommand::Delay { frames })
    });

    // game.warpTo(map: string, x: number, y: number) -> Promise<void>
    register_async_command!("warpTo", bridge, context, game_obj, |args: &[JsValue],
                                                                  ctx: &mut Context|
     -> JsResult<
        ScriptCommand,
    > {
        let map = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_lossy();
        let x = args.get_or_undefined(1).to_u32(ctx)? as u8;
        let y = args.get_or_undefined(2).to_u32(ctx)? as u8;
        Ok(ScriptCommand::WarpTo { map, x, y })
    });

    // game.heal() -> Promise<void>
    register_async_command!("heal", bridge, context, game_obj, |_args: &[JsValue],
                                                                _ctx: &mut Context|
     -> JsResult<
        ScriptCommand,
    > {
        Ok(ScriptCommand::Heal)
    });

    // game.fadeScreen(fadeType: string) -> Promise<void>
    register_async_command!(
        "fadeScreen",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let fade_type = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            Ok(ScriptCommand::FadeScreen { fade_type })
        }
    );

    // game.showObject(objectIndexOrToggleId: number | string) -> Promise<void>
    register_async_command!(
        "showObject",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let arg = args.get_or_undefined(0);
            if arg.is_string() {
                let toggle_id = arg.to_string(ctx)?.to_std_string_lossy();
                Ok(ScriptCommand::ShowObjectByName { toggle_id })
            } else {
                let object_index = arg.to_u32(ctx)? as u8;
                Ok(ScriptCommand::ShowObject { object_index })
            }
        }
    );

    // game.hideObject(objectIndexOrToggleId: number | string) -> Promise<void>
    register_async_command!(
        "hideObject",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let arg = args.get_or_undefined(0);
            if arg.is_string() {
                let toggle_id = arg.to_string(ctx)?.to_std_string_lossy();
                Ok(ScriptCommand::HideObjectByName { toggle_id })
            } else {
                let object_index = arg.to_u32(ctx)? as u8;
                Ok(ScriptCommand::HideObject { object_index })
            }
        }
    );

    // game.showObjectByName(toggleId: string) -> Promise<void>
    // Explicit string-only alias used by many .scene files (e.g. `@load` guards).
    // Without this the call is `undefined` and the handler throws before the
    // object is ever toggled.
    register_async_command!(
        "showObjectByName",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let toggle_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            Ok(ScriptCommand::ShowObjectByName { toggle_id })
        }
    );

    // game.hideObjectByName(toggleId: string) -> Promise<void>
    register_async_command!(
        "hideObjectByName",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let toggle_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            Ok(ScriptCommand::HideObjectByName { toggle_id })
        }
    );

    // game.setJoyIgnore(mask: number) -> Promise<void>
    register_async_command!(
        "setJoyIgnore",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let mask = args.get_or_undefined(0).to_u32(ctx)? as u8;
            Ok(ScriptCommand::SetJoyIgnore { mask })
        }
    );

    // game.clearJoyIgnore() -> Promise<void>
    register_async_command!(
        "clearJoyIgnore",
        bridge,
        context,
        game_obj,
        |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
            Ok(ScriptCommand::ClearJoyIgnore)
        }
    );

    // game.followNpc(npcId: string, targetX: number, targetY: number) -> Promise<void>
    register_async_command!(
        "followNpc",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let npc_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            let target_x = args.get_or_undefined(1).to_u32(ctx)? as u8;
            let target_y = args.get_or_undefined(2).to_u32(ctx)? as u8;
            Ok(ScriptCommand::FollowNpc {
                npc_id,
                target_x,
                target_y,
            })
        }
    );

    // game.openShop(items: string[]) -> Promise<void>
    register_async_command!(
        "openShop",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let arr = args.get_or_undefined(0).to_object(ctx)?;
            let len = arr.get(js_string!("length"), ctx)?.to_u32(ctx)?;
            let mut items = Vec::new();
            for i in 0..len {
                let val = arr.get(i, ctx)?;
                items.push(val.to_string(ctx)?.to_std_string_lossy());
            }
            Ok(ScriptCommand::OpenShop { items })
        }
    );

    // game.showEmotionBubble(npcId: string, emotion: string) -> Promise<void>
    register_async_command!(
        "showEmotionBubble",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let npc_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            let emotion = args
                .get_or_undefined(1)
                .to_string(ctx)?
                .to_std_string_lossy();
            Ok(ScriptCommand::ShowEmotionBubble { npc_id, emotion })
        }
    );

    // game.setNpcPosition(npcId: string, x: number, y: number) -> Promise<void>
    register_async_command!(
        "setNpcPosition",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let npc_id = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            let x = args.get_or_undefined(1).to_u32(ctx)? as u8;
            let y = args.get_or_undefined(2).to_u32(ctx)? as u8;
            Ok(ScriptCommand::SetNpcPosition { npc_id, x, y })
        }
    );

    // game.showScene(sceneName: string) -> Promise<void>
    register_async_command!(
        "showScene",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let scene_name = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            Ok(ScriptCommand::ShowScene {
                scene_name,
                layout_json: None,
            })
        }
    );

    // game.hideScene(sceneName: string) -> Promise<void>
    register_async_command!(
        "hideScene",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
            let scene_name = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_lossy();
            Ok(ScriptCommand::HideScene { scene_name })
        }
    );

    // game.updateUI(sceneName: string, data: any) -> Promise<void>
    register_async_command!(
        "updateUI",
        bridge,
        context,
        game_obj,
        |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
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
        }
    );

    // game.getFlag(flag: string) -> boolean
    {
        let bridge = bridge.clone();
        // SAFETY: closure captures Rc<RefCell<SharedBridge>> — no GC-traced types.
        let func = unsafe {
            NativeFunction::from_closure(move |_this, args, ctx| {
                let flag = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                let val = bridge.borrow().flags.get(&flag).copied().unwrap_or(false);
                Ok(JsValue::from(val))
            })
        };
        game_obj
            .set(
                js_string!("getFlag"),
                func.to_js_function(context.realm()),
                true,
                context,
            )
            .expect("failed to register game.getFlag");
    }

    // game.setFlag(flag: string) -> void
    {
        let bridge = bridge.clone();
        // SAFETY: closure captures Rc<RefCell<SharedBridge>> — no GC-traced types.
        let func = unsafe {
            NativeFunction::from_closure(move |_this, args, ctx| {
                let flag = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                bridge.borrow_mut().flags.insert(flag, true);
                Ok(JsValue::undefined())
            })
        };
        game_obj
            .set(
                js_string!("setFlag"),
                func.to_js_function(context.realm()),
                true,
                context,
            )
            .expect("failed to register game.setFlag");
    }

    // game.resetFlag(flag: string) -> void
    {
        let bridge = bridge.clone();
        // SAFETY: closure captures Rc<RefCell<SharedBridge>> — no GC-traced types.
        let func = unsafe {
            NativeFunction::from_closure(move |_this, args, ctx| {
                let flag = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                bridge.borrow_mut().flags.insert(flag, false);
                Ok(JsValue::undefined())
            })
        };
        game_obj
            .set(
                js_string!("resetFlag"),
                func.to_js_function(context.realm()),
                true,
                context,
            )
            .expect("failed to register game.resetFlag");
    }

    // game.getPlayerPosition() -> {x: number, y: number}
    {
        let bridge = bridge.clone();
        let func = unsafe {
            NativeFunction::from_closure(move |_this, _args, ctx| {
                let b = bridge.borrow();
                let pos = boa_engine::object::ObjectInitializer::new(ctx)
                    .property(
                        js_string!("x"),
                        JsValue::from(b.player_x as i32),
                        Attribute::all(),
                    )
                    .property(
                        js_string!("y"),
                        JsValue::from(b.player_y as i32),
                        Attribute::all(),
                    )
                    .build();
                Ok(pos.into())
            })
        };
        game_obj
            .set(
                js_string!("getPlayerPosition"),
                func.to_js_function(context.realm()),
                true,
                context,
            )
            .expect("failed to register game.getPlayerPosition");
    }

    // game.getPlayerX() -> number
    {
        let bridge = bridge.clone();
        let func = unsafe {
            NativeFunction::from_closure(move |_this, _args, _ctx| {
                Ok(JsValue::from(bridge.borrow().player_x as i32))
            })
        };
        game_obj
            .set(
                js_string!("getPlayerX"),
                func.to_js_function(context.realm()),
                true,
                context,
            )
            .expect("failed to register game.getPlayerX");
    }

    // game.getPlayerY() -> number
    {
        let bridge = bridge.clone();
        let func = unsafe {
            NativeFunction::from_closure(move |_this, _args, _ctx| {
                Ok(JsValue::from(bridge.borrow().player_y as i32))
            })
        };
        game_obj
            .set(
                js_string!("getPlayerY"),
                func.to_js_function(context.realm()),
                true,
                context,
            )
            .expect("failed to register game.getPlayerY");
    }

    context
        .register_global_property(js_string!("game"), game_obj, Attribute::all())
        .expect("failed to register global game object");
}
