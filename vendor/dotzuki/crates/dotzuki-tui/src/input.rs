/// Tracks button state across frames.
///
/// Terminal emulators typically don't send Release events for
/// character keys, so call `begin_frame()` and `clear()` each frame,
/// then re-press only the buttons that are still held.

#[derive(Debug, Clone)]
pub struct InputState<B> {
    current: Vec<B>,
    previous: Vec<B>,
}

impl<B: Copy + PartialEq> InputState<B> {
    /// Create a new empty input state.
    pub fn new() -> Self {
        Self {
            current: Vec::new(),
            previous: Vec::new(),
        }
    }

    /// Copy current state to previous and clear current.
    ///
    /// Call this at the start of each frame before processing events.
    pub fn begin_frame(&mut self) {
        self.previous = self.current.clone();
    }

    /// Clear both current and previous state.
    ///
    /// Call this after `begin_frame()` to start with a clean slate,
    /// then re-press only buttons that appear in this frame's event queue.
    pub fn clear(&mut self) {
        self.current.clear();
        self.previous.clear();
    }

    /// Register a button press for this frame.
    pub fn press(&mut self, button: B) {
        if !self.current.contains(&button) {
            self.current.push(button);
        }
    }

    /// Check if a button is held this frame.
    pub fn is_held(&self, button: B) -> bool {
        self.current.contains(&button)
    }

    /// Check if a button was just pressed this frame (pressed now, not pressed last frame).
    pub fn is_just_pressed(&self, button: B) -> bool {
        self.current.contains(&button) && !self.previous.contains(&button)
    }

    /// Check if any button was just pressed this frame.
    pub fn any_just_pressed(&self) -> bool {
        self.current.iter().any(|b| !self.previous.contains(b))
    }

    /// Check if any button is held this frame.
    pub fn any_held(&self) -> bool {
        !self.current.is_empty()
    }
}

impl<B: Copy + PartialEq> Default for InputState<B> {
    fn default() -> Self {
        Self::new()
    }
}
