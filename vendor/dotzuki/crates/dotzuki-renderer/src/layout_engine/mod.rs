//! JSON layout engine — declarative layout system for JRPG screen composition.
//!
//! This module provides a declarative, JSON-driven layout engine that composes
//! screens from a tree of layout elements. Layout definitions are deserialized
//! from JSON and rendered by element-specific renderers.
//!
//! ## Architecture
//!
//! ```text
//! JSON Layout → Deserialize → ScreenLayout (element tree)
//!                               ↓
//!                          LayoutEngine::render()
//!                               ↓
//!                    Element renderers (border, text, tile, …)
//!                               ↓
//!                          Framebuffer
//! ```
//!
//! ## Sub-modules
//!
//! - [`types`] — Core type definitions (ScreenLayout, LayoutElement,
//!   ElementParams, etc.)

pub mod data_context;
pub mod deserialize;
pub mod elements;
pub mod registry;
pub mod renderer;
pub mod types;
