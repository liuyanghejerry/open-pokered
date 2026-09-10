//! Render configuration for the engine.
//!
//! Defines screen dimensions and other renderer settings that can be
//! customized per-game. Each game must provide its own dimensions —
//! there is no default resolution.

/// Configuration for the rendering pipeline.
///
/// Stores screen dimensions and any other renderer-level settings.
/// Each game must construct this with its own resolution via
/// [`RenderConfig::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderConfig {
    /// Width of the screen in pixels.
    pub screen_width: u32,
    /// Height of the screen in pixels.
    pub screen_height: u32,
}

impl RenderConfig {
    /// Create a new render config with the given dimensions.
    pub const fn new(screen_width: u32, screen_height: u32) -> Self {
        Self {
            screen_width,
            screen_height,
        }
    }
}
