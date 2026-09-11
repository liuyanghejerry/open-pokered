//! Map layer definitions, rendering state, colour types, geometry types,
//! and the pixel framebuffer.
//!
//! This module defines:
//!
//! * **Layer compositing** — [`MapLayer`], [`MapRenderState`], [`BlendMode`] for
//!   multi-layer tilemap rendering.
//! * **Colour** — [`Rgba`] for representing pixel colours.
//! * **Framebuffer** — [`FrameBuffer`], [`DirtyRegion`], screen-dimension constants.
//! * **Geometry** — [`TilePos`], [`TileRect`], [`BracketSides`] for tile-grid
//!   positioning.
//!
//! The actual compositing logic lives in `pokered-renderer`; this crate
//! only provides the **data model**.

pub mod color;
pub mod framebuffer;
pub mod geometry;
pub mod painter;

pub use color::Rgba;
pub use framebuffer::{DirtyRegion, FrameBuffer, BYTES_PER_PIXEL, TILE_SIZE};
pub use geometry::{BracketSides, TilePos, TileRect};
pub use painter::{Frame, LabelValue, Painter, Ui};

use crate::tilemap::Tilemap;

/// How a layer blends with layers below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    /// Source over destination (standard alpha compositing).
    Normal,
    /// Add source and destination colours (for glow / fire / flash effects).
    Additive,
    /// Multiply source and destination colours (for shadows / dark overlays).
    Multiply,
}

/// A single layer in the map rendering stack.
///
/// Each layer owns a [`Tilemap`] and carries display properties that
/// control how it is positioned and blended relative to other layers.
#[derive(Debug, Clone)]
pub struct MapLayer {
    /// The tilemap data for this layer.
    pub tilemap: Tilemap,
    /// Whether this layer should be drawn.  Invisible layers are skipped
    /// entirely during compositing.
    pub visible: bool,
    /// Global opacity: `0.0` = fully transparent, `1.0` = fully opaque.
    pub opacity: f32,
    /// Parallax scroll factor.
    ///
    /// `(1.0, 1.0)` means the layer scrolls at the same rate as the camera.
    /// `(0.5, 0.5)` means the layer scrolls at half the camera speed (distant
    /// background), and `(2.0, 2.0)` means it scrolls faster (foreground).
    pub scroll_factor: (f32, f32),
    /// How this layer blends with layers rendered below it.
    pub blend_mode: BlendMode,
    /// Sort order.  Lower values are rendered first (behind).
    pub z_index: i32,
    /// When true, the layer's content does not animate frame-to-frame.
    /// Renderers may cache pre-rendered tiles for performance.
    pub no_animation: bool,
    /// Elevation level this layer belongs to (0 = ground, the default).
    /// Multi-level maps split rendering at the player's elevation: layers
    /// with `level <= player_elevation` render below sprites, layers above
    /// render over them.
    pub level: i32,
}

impl MapLayer {
    /// Create a new layer with sensible defaults.
    ///
    /// * `tilemap` – the tile data for this layer.
    /// * `z_index` – render order (lower = behind).
    pub fn new(tilemap: Tilemap, z_index: i32) -> Self {
        Self {
            tilemap,
            visible: true,
            opacity: 1.0,
            scroll_factor: (1.0, 1.0),
            blend_mode: BlendMode::Normal,
            z_index,
            no_animation: false,
            level: 0,
        }
    }
}

/// The complete rendering state for a map (all layers).
#[derive(Debug, Clone)]
pub struct MapRenderState {
    /// Ordered stack of map layers.  The compositor sorts them by
    /// [`MapLayer::z_index`] before drawing.
    pub layers: Vec<MapLayer>,
    /// RGBA background colour used to fill the framebuffer before any layer
    /// is drawn.
    pub background_color: (u8, u8, u8, u8),
}

impl MapRenderState {
    /// Create an empty render state with a transparent black background.
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            background_color: (0, 0, 0, 255),
        }
    }

    /// Add a layer to the stack.
    pub fn add_layer(&mut self, layer: MapLayer) {
        self.layers.push(layer);
    }

    /// Count how many layers are currently visible.
    pub fn visible_layer_count(&self) -> usize {
        self.layers.iter().filter(|l| l.visible).count()
    }
}

impl Default for MapRenderState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tilemap::Tilemap;

    fn make_tilemap(w: u16, h: u16) -> Tilemap {
        Tilemap::new(w, h)
    }

    #[test]
    fn blend_mode_equality() {
        assert_eq!(BlendMode::Normal, BlendMode::Normal);
        assert_ne!(BlendMode::Normal, BlendMode::Additive);
        assert_ne!(BlendMode::Additive, BlendMode::Multiply);
    }

    #[test]
    fn map_layer_defaults() {
        let tm = make_tilemap(10, 10);
        let layer = MapLayer::new(tm, 0);
        assert!(layer.visible);
        assert!((layer.opacity - 1.0).abs() < f32::EPSILON);
        assert_eq!(layer.scroll_factor, (1.0, 1.0));
        assert_eq!(layer.blend_mode, BlendMode::Normal);
        assert_eq!(layer.z_index, 0);
        assert_eq!(layer.level, 0);
    }

    #[test]
    fn map_layer_custom() {
        let tm = make_tilemap(8, 8);
        let layer = MapLayer {
            tilemap: tm,
            visible: false,
            opacity: 0.5,
            scroll_factor: (0.5, 0.5),
            blend_mode: BlendMode::Additive,
            z_index: 5,
            no_animation: false,
            level: 1,
        };
        assert!(!layer.visible);
        assert!((layer.opacity - 0.5).abs() < f32::EPSILON);
        assert_eq!(layer.scroll_factor, (0.5, 0.5));
        assert_eq!(layer.blend_mode, BlendMode::Additive);
        assert_eq!(layer.z_index, 5);
        assert_eq!(layer.level, 1);
    }

    #[test]
    fn map_render_state_default() {
        let state = MapRenderState::default();
        assert!(state.layers.is_empty());
        assert_eq!(state.background_color, (0, 0, 0, 255));
    }

    #[test]
    fn map_render_state_add_layer() {
        let mut state = MapRenderState::new();
        let tm = make_tilemap(32, 32);
        state.add_layer(MapLayer::new(tm, 0));
        assert_eq!(state.layers.len(), 1);
    }

    #[test]
    fn visible_layer_count() {
        let mut state = MapRenderState::new();
        assert_eq!(state.visible_layer_count(), 0);

        let mut l1 = MapLayer::new(make_tilemap(4, 4), 0);
        l1.visible = true;
        state.add_layer(l1);

        let mut l2 = MapLayer::new(make_tilemap(4, 4), 1);
        l2.visible = false;
        state.add_layer(l2);

        let mut l3 = MapLayer::new(make_tilemap(4, 4), 2);
        l3.visible = true;
        state.add_layer(l3);

        assert_eq!(state.visible_layer_count(), 2);
    }

    #[test]
    fn map_render_state_background_color() {
        let state = MapRenderState {
            layers: vec![],
            background_color: (0x12, 0x34, 0x56, 0xFF),
        };
        assert_eq!(state.background_color, (0x12, 0x34, 0x56, 0xFF));
    }

    #[test]
    fn layer_clone_independent() {
        let tm = make_tilemap(4, 4);
        let mut l1 = MapLayer::new(tm, 0);
        l1.opacity = 0.7;
        let l2 = l1.clone();
        assert!((l2.opacity - 0.7).abs() < f32::EPSILON);
        // l2.tilemap should be a clone, not a reference
        assert_eq!(l2.tilemap.width, 4);
    }
}
