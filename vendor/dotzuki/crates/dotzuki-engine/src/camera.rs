//! Camera system — viewport positioning with smooth follow, zoom, and boundary clamping.
//!
//! A general-purpose camera that supports arbitrary world sizes, fractional pixel
//! positions, smooth interpolation, and zoom. It supersedes the older fixed
//! Game Boy-style scroll/viewport approach.
//!
//! ## Usage
//!
//! ```ignore
//! use dotzuki_engine::camera::{Camera, Vec2, Rect};
//!
//! let mut cam = Camera::new(160.0, 144.0);
//! cam.clamp_to_bounds(Rect::new(0.0, 0.0, 2048.0, 2048.0));
//! cam.follow_target(Vec2::new(player_x, player_y));
//!
//! // Every frame:
//! cam.update(dt);
//! let screen_pos = cam.world_to_screen(Vec2::new(npc_x, npc_y));
//! ```

/// Two-dimensional vector (floating-point).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Axis-aligned rectangle with floating-point coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
}

/// Tile size in pixels used for coordinate calculations.
const TILE_SIZE: f32 = 8.0;

/// Default smooth-follow speed multiplier. Higher values produce snappier movement.
const DEFAULT_FOLLOW_SPEED: f32 = 12.0;

/// Camera controlling which portion of the world is visible on screen.
///
/// The camera's `position` represents the **top-left corner** of the viewport in
/// world-space pixels. All coordinate conversions account for `zoom`.
#[derive(Debug, Clone, PartialEq)]
pub struct Camera {
    /// Current camera position in world pixels (top-left corner of the viewport).
    pub position: Vec2,
    /// Desired position for smooth follow (world pixels). `None` means no follow target.
    pub target: Option<Vec2>,
    /// World boundaries that clamp the camera. `None` means unbounded.
    pub bounds: Option<Rect>,
    /// Smooth-follow factor: 0.0 = instant snap, 1.0 = no movement at all.
    pub smooth_factor: f32,
    /// Zoom level: 1.0 = normal, > 1.0 = zoomed in, < 1.0 = zoomed out.
    pub zoom: f32,
    /// Size of the viewport / screen in pixels.
    pub viewport_size: Vec2,
}

impl Camera {
    // ---------------------------------------------------------------------------
    // Construction
    // ---------------------------------------------------------------------------

    /// Create a new camera with default settings.
    ///
    /// Position starts at (0, 0), zoom is 1.0, and smooth follow is disabled
    /// (smooth_factor = 0.0 — instant snap).
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            position: Vec2::new(0.0, 0.0),
            target: None,
            bounds: None,
            smooth_factor: 0.0,
            zoom: 1.0,
            viewport_size: Vec2::new(viewport_width, viewport_height),
        }
    }

    // ---------------------------------------------------------------------------
    // Targeting
    // ---------------------------------------------------------------------------

    /// Set (or replace) the target world position for smooth follow.
    ///
    /// Call [`update`](Self::update) every frame to interpolate `position` toward
    /// this target.
    pub fn follow_target(&mut self, target: Vec2) {
        self.target = Some(target);
    }

    /// Stop following any target. The camera stays at its current position.
    pub fn stop_follow(&mut self) {
        self.target = None;
    }

    // ---------------------------------------------------------------------------
    // Per-frame update
    // ---------------------------------------------------------------------------

    /// Advance the camera by `dt` seconds.
    ///
    /// If a [`target`](Self::target) is set, `position` is interpolated toward it
    /// using exponential decay that is frame-rate-independent. After interpolation,
    /// `position` is clamped to [`bounds`](Self::bounds) (if set).
    ///
    /// When `smooth_factor` is 0.0 the position snaps to the target instantly.
    ///
    /// # Panics
    ///
    /// Panics if `dt` is negative.
    pub fn update(&mut self, dt: f32) {
        assert!(dt >= 0.0, "dt must be non-negative, got {dt}");

        if let Some(ref target) = self.target {
            if self.smooth_factor <= 0.0 {
                // Instant snap
                self.position = *target;
            } else {
                let follow_strength = (1.0 - self.smooth_factor) * DEFAULT_FOLLOW_SPEED;
                // f32::exp is not available on no_std targets; libm's expf is
                // used there (bit-identical inputs, same decay curve).
                #[cfg(target_os = "none")]
                let decay = libm::expf(-follow_strength * dt);
                #[cfg(not(target_os = "none"))]
                let decay = (-follow_strength * dt).exp();
                let t = 1.0 - decay;
                self.position.x += (target.x - self.position.x) * t;
                self.position.y += (target.y - self.position.y) * t;
            }
        }

        self.apply_bounds();
    }

    // ---------------------------------------------------------------------------
    // Coordinate conversion
    // ---------------------------------------------------------------------------

    /// Convert a world-space position to a screen-space position, accounting for
    /// the camera's current position and zoom.
    ///
    /// ```
    /// # use dotzuki_engine::camera::{Camera, Vec2};
    /// let cam = Camera::new(160.0, 144.0);
    /// let screen = cam.world_to_screen(Vec2::new(80.0, 72.0));
    /// assert_eq!(screen.x, 80.0);
    /// assert_eq!(screen.y, 72.0);
    /// ```
    pub fn world_to_screen(&self, world_pos: Vec2) -> Vec2 {
        Vec2::new(
            (world_pos.x - self.position.x) * self.zoom,
            (world_pos.y - self.position.y) * self.zoom,
        )
    }

    /// Convert a screen-space position to a world-space position.
    ///
    /// This is the inverse of [`world_to_screen`](Self::world_to_screen).
    ///
    /// ```
    /// # use dotzuki_engine::camera::{Camera, Vec2};
    /// let cam = Camera::new(160.0, 144.0);
    /// let world = cam.screen_to_world(Vec2::new(160.0, 144.0));
    /// assert_eq!(world.x, 160.0);
    /// assert_eq!(world.y, 144.0);
    /// ```
    pub fn screen_to_world(&self, screen_pos: Vec2) -> Vec2 {
        Vec2::new(
            screen_pos.x / self.zoom + self.position.x,
            screen_pos.y / self.zoom + self.position.y,
        )
    }

    // ---------------------------------------------------------------------------
    // Bounds
    // ---------------------------------------------------------------------------

    /// Set the world boundary rectangle and immediately clamp the camera to it.
    ///
    /// The camera will never show content outside these bounds. The visible area
    /// (viewport_size / zoom) must fit within the bounds; if the viewport is larger
    /// than the bounds, the camera is centered on the bounds area.
    pub fn clamp_to_bounds(&mut self, bounds: Rect) {
        self.bounds = Some(bounds);
        self.apply_bounds();
    }

    /// Remove world boundaries. The camera can scroll anywhere.
    pub fn clear_bounds(&mut self) {
        self.bounds = None;
    }

    /// Clamp `position` so the viewport stays within [`bounds`](Self::bounds).
    fn apply_bounds(&mut self) {
        let Some(ref bounds) = self.bounds else {
            return;
        };

        let visible_w = self.viewport_size.x / self.zoom;
        let visible_h = self.viewport_size.y / self.zoom;

        // If the viewport is larger than the bounds, centre on the bounds area.
        if visible_w >= bounds.w {
            self.position.x = bounds.x + (bounds.w - visible_w) * 0.5;
        } else {
            self.position.x = self
                .position
                .x
                .clamp(bounds.x, bounds.x + bounds.w - visible_w);
        }

        if visible_h >= bounds.h {
            self.position.y = bounds.y + (bounds.h - visible_h) * 0.5;
        } else {
            self.position.y = self
                .position
                .y
                .clamp(bounds.y, bounds.y + bounds.h - visible_h);
        }
    }

    // ---------------------------------------------------------------------------
    // Zoom
    // ---------------------------------------------------------------------------

    /// Set the zoom level, clamped to the valid range `[0.1, 10.0]`.
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(0.1, 10.0);
        // Re-clamp after zoom change because the visible area changed.
        self.apply_bounds();
    }

    // ---------------------------------------------------------------------------
    // Tile helpers
    // ---------------------------------------------------------------------------

    /// Which tile column is at the camera's left edge.
    ///
    /// Divides the camera's world x-position by `TILE_SIZE` (8 px) and floors.
    pub fn tile_x(&self) -> i32 {
        // f32::floor is std-only; libm's floorf on no_std targets.
        #[cfg(target_os = "none")]
        {
            libm::floorf(self.position.x / TILE_SIZE) as i32
        }
        #[cfg(not(target_os = "none"))]
        {
            (self.position.x / TILE_SIZE).floor() as i32
        }
    }

    /// Which tile row is at the camera's top edge.
    ///
    /// Divides the camera's world y-position by `TILE_SIZE` (8 px) and floors.
    pub fn tile_y(&self) -> i32 {
        #[cfg(target_os = "none")]
        {
            libm::floorf(self.position.y / TILE_SIZE) as i32
        }
        #[cfg(not(target_os = "none"))]
        {
            (self.position.y / TILE_SIZE).floor() as i32
        }
    }

    // ---------------------------------------------------------------------------
    // Visibility checks
    // ---------------------------------------------------------------------------

    /// Returns `true` if a world-space point is visible on screen (within the
    /// viewport), accounting for the camera position and zoom.
    pub fn is_visible(&self, world_pos: Vec2) -> bool {
        let screen = self.world_to_screen(world_pos);
        screen.x >= -TILE_SIZE
            && screen.y >= -TILE_SIZE
            && screen.x < self.viewport_size.x + TILE_SIZE
            && screen.y < self.viewport_size.y + TILE_SIZE
    }

    /// Returns `true` if any part of a world-space axis-aligned rectangle is
    /// visible on screen.
    pub fn is_rect_visible(&self, rect: Rect) -> bool {
        let visible_w = self.viewport_size.x / self.zoom;
        let visible_h = self.viewport_size.y / self.zoom;

        // Expand viewport by one tile in each direction for culling margin.
        let margin = TILE_SIZE / self.zoom;

        !(rect.x + rect.w < self.position.x - margin
            || rect.x > self.position.x + visible_w + margin
            || rect.y + rect.h < self.position.y - margin
            || rect.y > self.position.y + visible_h + margin)
    }
}

// =============================================================================
// Unit tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    #[test]
    fn camera_starts_at_origin() {
        let cam = Camera::new(160.0, 144.0);
        assert_eq!(cam.position, Vec2::new(0.0, 0.0));
        assert_eq!(cam.zoom, 1.0);
        assert_eq!(cam.smooth_factor, 0.0);
        assert!(cam.target.is_none());
        assert!(cam.bounds.is_none());
    }

    #[test]
    fn new_is_gameboy_screen() {
        let cam = Camera::new(160.0, 144.0);
        assert_eq!(cam.viewport_size, Vec2::new(160.0, 144.0));
    }

    #[test]
    fn custom_viewport_size() {
        let cam = Camera::new(640.0, 480.0);
        assert_eq!(cam.viewport_size, Vec2::new(640.0, 480.0));
    }

    // -----------------------------------------------------------------------
    // Follow target
    // -----------------------------------------------------------------------

    #[test]
    fn follow_target_sets_target() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.follow_target(Vec2::new(100.0, 50.0));
        assert_eq!(cam.target, Some(Vec2::new(100.0, 50.0)));
    }

    #[test]
    fn follow_target_overwrites_previous() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.follow_target(Vec2::new(10.0, 20.0));
        cam.follow_target(Vec2::new(30.0, 40.0));
        assert_eq!(cam.target, Some(Vec2::new(30.0, 40.0)));
    }

    #[test]
    fn stop_follow_clears_target() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.follow_target(Vec2::new(100.0, 50.0));
        cam.stop_follow();
        assert!(cam.target.is_none());
    }

    #[test]
    fn update_snaps_when_smooth_factor_zero() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.smooth_factor = 0.0;
        cam.follow_target(Vec2::new(100.0, 50.0));
        cam.update(0.016);
        assert_eq!(cam.position, Vec2::new(100.0, 50.0));
    }

    #[test]
    fn update_moves_toward_target_with_smooth() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.smooth_factor = 0.5;
        cam.follow_target(Vec2::new(100.0, 0.0));
        let start = cam.position;
        cam.update(0.016);
        assert!(cam.position.x > start.x);
        assert!(cam.position.x < 100.0);
    }

    #[test]
    fn update_no_movement_when_smooth_factor_one() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.smooth_factor = 1.0;
        cam.position = Vec2::new(50.0, 50.0);
        cam.follow_target(Vec2::new(200.0, 200.0));
        cam.update(0.016);
        assert_eq!(cam.position.x, 50.0);
        assert_eq!(cam.position.y, 50.0);
    }

    #[test]
    fn update_no_target_does_nothing() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.position = Vec2::new(30.0, 40.0);
        cam.update(0.016);
        assert_eq!(cam.position, Vec2::new(30.0, 40.0));
    }

    #[test]
    fn update_converges_after_many_frames() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.smooth_factor = 0.2;
        cam.follow_target(Vec2::new(200.0, 0.0));
        for _ in 0..200 {
            cam.update(0.016);
        }
        assert!((cam.position.x - 200.0).abs() < 1.0);
    }

    // -----------------------------------------------------------------------
    // Coordinate conversion
    // -----------------------------------------------------------------------

    #[test]
    fn world_to_screen_at_origin() {
        let cam = Camera::new(160.0, 144.0);
        let screen = cam.world_to_screen(Vec2::new(50.0, 30.0));
        assert_eq!(screen, Vec2::new(50.0, 30.0));
    }

    #[test]
    fn world_to_screen_offset() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.position = Vec2::new(100.0, 50.0);
        let screen = cam.world_to_screen(Vec2::new(150.0, 100.0));
        assert_eq!(screen, Vec2::new(50.0, 50.0));
    }

    #[test]
    fn world_to_screen_negative_when_world_is_left_of_camera() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.position = Vec2::new(100.0, 50.0);
        let screen = cam.world_to_screen(Vec2::new(50.0, 20.0));
        assert_eq!(screen, Vec2::new(-50.0, -30.0));
    }

    #[test]
    fn screen_to_world_at_origin() {
        let cam = Camera::new(160.0, 144.0);
        let world = cam.screen_to_world(Vec2::new(80.0, 72.0));
        assert_eq!(world, Vec2::new(80.0, 72.0));
    }

    #[test]
    fn screen_to_world_offset() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.position = Vec2::new(100.0, 50.0);
        let world = cam.screen_to_world(Vec2::new(20.0, 30.0));
        assert_eq!(world, Vec2::new(120.0, 80.0));
    }

    #[test]
    fn round_trip_world_to_screen_to_world() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.position = Vec2::new(42.5, 17.3);
        cam.zoom = 1.5;

        let world = Vec2::new(123.4, 56.7);
        let screen = cam.world_to_screen(world);
        let round_tripped = cam.screen_to_world(screen);

        assert!((round_tripped.x - world.x).abs() < 0.001);
        assert!((round_tripped.y - world.y).abs() < 0.001);
    }

    #[test]
    fn round_trip_screen_to_world_to_screen() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.position = Vec2::new(50.0, 30.0);
        cam.zoom = 2.0;

        let screen = Vec2::new(80.0, 72.0);
        let world = cam.screen_to_world(screen);
        let round_tripped = cam.world_to_screen(world);

        assert!((round_tripped.x - screen.x).abs() < 0.001);
        assert!((round_tripped.y - screen.y).abs() < 0.001);
    }

    // -----------------------------------------------------------------------
    // Zoom
    // -----------------------------------------------------------------------

    #[test]
    fn zoom_affects_world_to_screen() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.position = Vec2::new(100.0, 100.0);

        // At zoom 2.0, world point 200,200 should appear at screen 200,200
        // (world - position) * zoom = (200-100)*2, (200-100)*2 = 200, 200
        cam.zoom = 2.0;
        let screen = cam.world_to_screen(Vec2::new(200.0, 200.0));
        assert_eq!(screen, Vec2::new(200.0, 200.0));
    }

    #[test]
    fn zoom_affects_screen_to_world() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.position = Vec2::new(100.0, 100.0);
        cam.zoom = 2.0;

        // screen 80,72 → world: 80/2+100, 72/2+100 = 140, 136
        let world = cam.screen_to_world(Vec2::new(80.0, 72.0));
        assert_eq!(world, Vec2::new(140.0, 136.0));
    }

    #[test]
    fn set_zoom_clamps_to_range() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.set_zoom(0.0);
        assert_eq!(cam.zoom, 0.1);
        cam.set_zoom(100.0);
        assert_eq!(cam.zoom, 10.0);
        cam.set_zoom(1.5);
        assert_eq!(cam.zoom, 1.5);
    }

    #[test]
    fn set_zoom_clamps_bounds() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.position = Vec2::new(0.0, 0.0);
        cam.clamp_to_bounds(Rect::new(0.0, 0.0, 320.0, 288.0));

        // At zoom 1.0, visible area is 160×144, max position is 160,144
        assert_eq!(cam.position, Vec2::new(0.0, 0.0));
        cam.position = Vec2::new(200.0, 200.0);
        cam.set_zoom(1.0);
        assert_eq!(cam.position, Vec2::new(160.0, 144.0));
    }

    // -----------------------------------------------------------------------
    // Boundary clamping
    // -----------------------------------------------------------------------

    #[test]
    fn clamp_keeps_camera_in_bounds() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.position = Vec2::new(500.0, 500.0);
        // 256 - 160 = 96 max x, 256 - 144 = 112 max y
        cam.clamp_to_bounds(Rect::new(0.0, 0.0, 256.0, 256.0));
        assert_eq!(cam.position, Vec2::new(96.0, 112.0));
    }

    #[test]
    fn clamp_prevents_negative_position() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.position = Vec2::new(-100.0, -50.0);
        cam.clamp_to_bounds(Rect::new(0.0, 0.0, 256.0, 256.0));
        assert_eq!(cam.position, Vec2::new(0.0, 0.0));
    }

    #[test]
    fn clamp_centers_when_viewport_larger_than_bounds() {
        let mut cam = Camera::new(320.0, 240.0);
        cam.position = Vec2::new(0.0, 0.0);
        // Bounds are smaller than viewport → camera should center
        cam.clamp_to_bounds(Rect::new(0.0, 0.0, 160.0, 120.0));
        // center: (160 - 320) / 2 = -80, (120 - 240) / 2 = -60
        assert_eq!(cam.position, Vec2::new(-80.0, -60.0));
    }

    #[test]
    fn clear_bounds_allows_any_position() {
        let mut cam = Camera::new(160.0, 144.0);
        // viewport > bounds: centres to (-30, -22)
        cam.clamp_to_bounds(Rect::new(0.0, 0.0, 100.0, 100.0));
        assert_eq!(cam.position, Vec2::new(-30.0, -22.0));

        cam.position = Vec2::new(500.0, 500.0);
        cam.clear_bounds();
        assert_eq!(cam.position, Vec2::new(500.0, 500.0));
    }

    #[test]
    fn update_clamps_after_follow() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.smooth_factor = 0.0;
        // 200 - 160 = 40 max x, 200 - 144 = 56 max y
        cam.clamp_to_bounds(Rect::new(0.0, 0.0, 200.0, 200.0));
        cam.follow_target(Vec2::new(500.0, 500.0));
        cam.update(0.016);
        assert_eq!(cam.position, Vec2::new(40.0, 56.0));
    }

    // -----------------------------------------------------------------------
    // Tile helpers
    // -----------------------------------------------------------------------

    #[test]
    fn tile_x_returns_correct_column() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.position = Vec2::new(0.0, 0.0);
        assert_eq!(cam.tile_x(), 0);

        cam.position = Vec2::new(8.0, 0.0);
        assert_eq!(cam.tile_x(), 1);

        cam.position = Vec2::new(15.0, 0.0);
        assert_eq!(cam.tile_x(), 1); // floor(15/8) = 1

        cam.position = Vec2::new(16.0, 0.0);
        assert_eq!(cam.tile_x(), 2);

        cam.position = Vec2::new(-5.0, 0.0);
        assert_eq!(cam.tile_x(), -1); // floor(-5/8) = -1
    }

    #[test]
    fn tile_y_returns_correct_row() {
        let mut cam = Camera::new(160.0, 144.0);
        cam.position = Vec2::new(0.0, 0.0);
        assert_eq!(cam.tile_y(), 0);

        cam.position = Vec2::new(0.0, 8.0);
        assert_eq!(cam.tile_y(), 1);

        cam.position = Vec2::new(0.0, 72.0);
        assert_eq!(cam.tile_y(), 9); // floor(72/8) = 9
    }

    // -----------------------------------------------------------------------
    // Visibility checks
    // -----------------------------------------------------------------------

    #[test]
    fn is_visible_in_viewport() {
        let cam = Camera::new(160.0, 144.0);
        // Point in centre of screen
        assert!(cam.is_visible(Vec2::new(80.0, 72.0)));
        // Point at top-left corner
        assert!(cam.is_visible(Vec2::new(0.0, 0.0)));
        // Point just outside
        assert!(cam.is_visible(Vec2::new(-7.0, 0.0))); // within margin
                                                       // Point far outside
        assert!(!cam.is_visible(Vec2::new(1000.0, 1000.0)));
    }

    #[test]
    fn is_rect_visible_intersects() {
        let cam = Camera::new(160.0, 144.0);
        assert!(cam.is_rect_visible(Rect::new(0.0, 0.0, 32.0, 32.0)));
        assert!(cam.is_rect_visible(Rect::new(140.0, 120.0, 32.0, 32.0)));
    }

    #[test]
    fn is_rect_visible_far_away() {
        let cam = Camera::new(160.0, 144.0);
        assert!(!cam.is_rect_visible(Rect::new(1000.0, 1000.0, 32.0, 32.0)));
    }

    // -----------------------------------------------------------------------
    // Vec2 / Rect
    // -----------------------------------------------------------------------

    #[test]
    fn vec2_new() {
        let v = Vec2::new(3.0, 4.0);
        assert_eq!(v.x, 3.0);
        assert_eq!(v.y, 4.0);
    }

    #[test]
    fn rect_new() {
        let r = Rect::new(1.0, 2.0, 10.0, 20.0);
        assert_eq!(r.x, 1.0);
        assert_eq!(r.y, 2.0);
        assert_eq!(r.w, 10.0);
        assert_eq!(r.h, 20.0);
    }
}
