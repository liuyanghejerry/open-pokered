//! Proc-macro for compile-time rules.ron to Rust code generation.
//!
//! This crate provides a `rules_ron!` proc-macro that reads a `rules.ron` file
//! at compile time and generates Rust code that directly constructs the
//! `Ruleset` struct, eliminating the need for runtime RON parsing.
//!
//! # Usage
//!
//! ```rust,ignore
//! use dotzuki_rules_macro::rules_ron;
//!
//! // Generate code from rules.ron at compile time
//! const RULESET: Ruleset = rules_ron!("path/to/rules.ron");
//! ```
//!
//! # Benefits
//!
//! - Eliminates runtime RON parsing overhead
//! - Removes `ron` dependency from release builds
//! - Compile-time validation of rules.ron syntax
//! - Type-safe generated code

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

mod generator;
mod parser;

/// Generate Rust code from a rules.ron file at compile time.
///
/// This proc-macro reads the specified `rules.ron` file, parses it, and
/// generates Rust code that directly constructs a `Ruleset` struct.
///
/// # Arguments
///
/// * `path` - Path to the rules.ron file, relative to the crate root
///
/// # Returns
///
/// An expression that evaluates to a `Ruleset` struct.
///
/// # Example
///
/// ```rust,ignore
/// use dotzuki_rules_macro::rules_ron;
///
/// let ruleset: Ruleset = rules_ron!("rules.ron");
/// ```
#[proc_macro]
pub fn rules_ron(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::LitStr);
    let path = input.value();

    match generator::generate_ruleset_code(&path) {
        Ok(code) => code.into(),
        Err(err) => {
            let error_msg = format!("Failed to generate rules from '{}': {}", path, err);
            quote! {
                compile_error!(#error_msg)
            }
            .into()
        }
    }
}
