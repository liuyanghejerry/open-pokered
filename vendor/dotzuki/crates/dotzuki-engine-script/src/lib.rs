//! dotzuki-engine-script — an async JavaScript scripting engine (Boa-based) for games.
//!
//! Provides an async/await-based scripting system using Boa (a pure-Rust JS engine)
//! that replaces the hardcoded ScriptAction queue. Map scripts are written in
//! JavaScript and can `await` game operations like showText(), moveNpc(), etc.
//!
//! # Architecture
//!
//! ```text
//! JS Script (async fn)
//!     │
//!     ├─ await game.showText("...")  ──► ScriptCommand::ShowText
//!     │       ↑ Rust resolves promise when text dismissed
//!     │
//!     ├─ game.getFlag("GOT_STARTER") ──► synchronous bool return
//!     │
//!     └─ await game.startBattle("RIVAL") ──► ScriptCommand::StartBattle
//!             ↑ Rust resolves promise with battle result
//! ```
//!
//! The game loop calls `ScriptEngine::tick()` each frame:
//! 1. If a pending command was resolved by Rust, `run_jobs()` drains the JS
//!    microtask queue so the async function continues to its next `await`.
//! 2. If the script issues a new command, it's returned to the caller for dispatch.
//! 3. If no script is active, returns `None`.

// no_std port (GBA / thumbv4t): without the `script-boa` feature this crate
// provides only the boa-free runtime protocol (`ScriptCommand`,
// `CommandResult`, `MapScriptConfig`, `CutsceneManager`), which is what
// bare-metal downstreams consume. See the cfg gates below for what drops out.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", feature(prelude_import))]
#![cfg_attr(target_os = "none", allow(internal_features))]

extern crate alloc;

#[allow(unused_imports)]
mod alloc_prelude {
    pub use core::prelude::v1::*;
    pub use core::convert::{TryFrom, TryInto};
    pub use alloc::borrow::ToOwned;
    pub use core::iter::FromIterator;
    pub use alloc::boxed::Box;
    pub use alloc::format;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
    pub use core::{assert_eq, assert_ne, matches, todo, unimplemented, write, writeln};
    pub use core::debug_assert;
}

#[cfg_attr(target_os = "none", prelude_import)]
#[allow(unused_imports)]
use alloc_prelude::*;

pub mod command;
pub mod config;
pub mod cutscene;

// Boa-backed runtime (hosted JS engine) — excluded from bare-metal builds.
#[cfg(feature = "script-boa")]
pub mod api_registrar;
#[cfg(feature = "script-boa")]
pub mod engine;
#[cfg(feature = "script-boa")]
pub mod game_api;
// Filesystem-backed script/config loading — hosted only.
#[cfg(not(target_os = "none"))]
pub mod loader;

// Bare-metal stub with the same surface: no filesystem, no JS engine. The
// overworld's native AST interpreter path (embedded scene tables) never
// reads through this loader; the shim exists so downstream crates compile
// unchanged on target_os = "none".
#[cfg(target_os = "none")]
pub mod loader {
    use crate::MapScriptConfig;

    #[derive(Debug, Clone, Default)]
    pub struct ScriptLoader;

    #[derive(Debug)]
    pub struct ScriptLoaderError;

    impl ScriptLoader {
        pub fn new() -> Self {
            Self
        }
        pub fn register_script(&mut self, _map_id: &str, _source: &str) {}
        pub fn register_config(&mut self, _map_id: &str, _config: MapScriptConfig) {}
        pub fn register_config_json(
            &mut self,
            _map_id: &str,
            _json: &str,
        ) -> Result<(), String> {
            Ok(())
        }
        pub fn get_script(&self, _map_id: &str) -> Option<&str> {
            None
        }
        pub fn get_config(&self, _map_id: &str) -> Option<&MapScriptConfig> {
            None
        }
        pub fn has_script(&self, _map_id: &str) -> bool {
            false
        }
        pub fn has_config(&self, _map_id: &str) -> bool {
            false
        }
        pub fn loaded_maps(&self) -> Vec<&str> {
            Vec::new()
        }
        pub fn load_auto<T>(&mut self, _dirs: Option<T>) -> Result<usize, ScriptLoaderError> {
            Ok(0)
        }
    }
}

#[cfg(feature = "embedded-scripts")]
mod embedded_scripts {
    include!(concat!(env!("OUT_DIR"), "/embedded_scripts.rs"));
}

#[cfg(all(test, feature = "script-boa"))]
mod tests;

pub use command::{CommandResult, ScriptCommand};
pub use config::MapScriptConfig;
pub use cutscene::CutsceneManager;
#[cfg(feature = "script-boa")]
pub use api_registrar::ScriptApiRegistrar;
#[cfg(feature = "script-boa")]
pub use engine::{BridgeView, ScriptEngine, ScriptEngineError};
#[cfg(not(target_os = "none"))]
pub use loader::{ScriptLoader, ScriptLoaderError};
#[cfg(target_os = "none")]
pub use loader::{ScriptLoader, ScriptLoaderError};
