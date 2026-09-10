// dotzuki-tui: Generic terminal TUI shell for JRPG games.
//
// Provides a generic game loop, halfblock framebuffer rendering widget,
// RAII terminal guard, and a [`TuiGame`] trait for games to implement.

mod input;
mod r#loop;
mod terminal;
mod tui_game;
mod widget;

pub use input::InputState;
pub use r#loop::{run, FRAME_DURATION};
pub use terminal::TerminalGuard;
pub use tui_game::TuiGame;
pub use widget::{auto_scale, HalfblockImage};
