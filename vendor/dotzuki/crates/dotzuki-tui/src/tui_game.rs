use dotzuki_renderer::FbSurface;

use crate::input::InputState;

/// A game that can be driven by the dotzuki-tui loop.
pub trait TuiGame {
    /// The button type used for input.
    type Button: Copy + PartialEq;

    /// The framebuffer type the game draws into (RGBA engine buffer or the
    /// indexed facade).
    type Fb: FbSurface;

    /// Called once per frame. Process input before returning.
    fn update(&mut self, input: &InputState<Self::Button>);

    /// Called once per frame. Draw the current screen into the framebuffer.
    fn draw(&mut self, fb: &mut Self::Fb);

    /// Should the loop exit?
    fn exit_requested(&self) -> bool;
}
