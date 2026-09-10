//! dotzuki-engine-dsl — Game DSL compiler for JRPG content authoring.
//!
//! Compiles `.scene`, `.gui`, `.theme`, and `.style` files into
//! executable JavaScript (via Boa engine) and JSON (for UI data).
//!
//! # no_std split
//!
//! The crate has two halves:
//!
//! * **Native AST interpreter** (`ast`, `lexer`, `parser`, `interpreter`) —
//!   always compiled, including bare-metal targets (GBA / thumbv4t). This is
//!   the runtime scene executor downstream games drive on device.
//! * **Compiler** (`compiler`, `codegen`, `config_gen`, `sourcemap`,
//!   `disk_loader`, `error`, `bridge`, `loader`, bins) — hosted only: it uses
//!   miette diagnostics, sourcemap generation, and the filesystem. Hosted
//!   builds (including build-dependency consumers such as pokered-data's
//!   build.rs, which compiles for the host where `target_os != "none"`) get
//!   the full crate.

// no_std port (GBA / thumbv4t): see the module-level cfg gates below.
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

pub mod ast;
pub mod hash;
pub mod interpreter;
pub mod lexer;
pub mod parser;

// Host-only compiler half (miette / sourcemap / fs / process access).
#[cfg(not(target_os = "none"))]
pub mod bridge;
#[cfg(not(target_os = "none"))]
pub mod codegen;
#[cfg(not(target_os = "none"))]
pub mod compiler;
#[cfg(not(target_os = "none"))]
pub mod config_gen;
#[cfg(not(target_os = "none"))]
pub mod conflict;
#[cfg(not(target_os = "none"))]
pub mod disk_loader;
#[cfg(not(target_os = "none"))]
pub mod error;
#[cfg(not(target_os = "none"))]
pub mod loader;
#[cfg(not(target_os = "none"))]
pub mod sourcemap;
