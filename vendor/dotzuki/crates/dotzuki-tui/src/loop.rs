use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use dotzuki_renderer::FbSurface;

use crate::input::InputState;
use crate::terminal::TerminalGuard;
use crate::tui_game::TuiGame;
use crate::widget::{auto_scale, HalfblockImage};

/// Target frame duration for ~59.7 Hz (matching classic JRPG timing).
///
/// 16,742,706 ns ≈ 59.73 fps.
pub const FRAME_DURATION: Duration = Duration::from_nanos(16_742_706);

/// Run the TUI game loop.
///
/// # Parameters
/// - `game`: Your game implementing [`TuiGame`].
/// - `key_handler`: Maps a [`crossterm::event::KeyEvent`] to your game's `Button` type.
///   Return `None` if the key should be ignored.
/// - `scale`: Fixed integer scale factor. If `None`, auto-detected from terminal size.
/// - `cell_ratio`: Terminal cell aspect ratio (width/height). Typical: `0.8`.
///
/// # Panics
///
/// Panics if terminal initialization fails (raw mode, alternate screen, etc.).
pub fn run<T: TuiGame>(
    game: &mut T,
    key_handler: impl Fn(crossterm::event::KeyEvent) -> Option<T::Button>,
    scale: Option<u32>,
    cell_ratio: f64,
    fb_width: u32,
    fb_height: u32,
) -> io::Result<()> {
    let _guard = TerminalGuard::new()?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut input = InputState::new();
    let mut fb = T::Fb::new_screen(fb_width, fb_height);
    let mut last_frame = Instant::now();

    loop {
        if game.exit_requested() {
            return Ok(());
        }

        let now = Instant::now();
        let elapsed = now.duration_since(last_frame);

        if elapsed >= FRAME_DURATION {
            last_frame = now;

            // Terminal emulators typically don't send Release events for
            // character keys, so we clear all held buttons each frame and
            // re-press only those that appear in this frame's event queue.
            input.begin_frame();
            input.clear();

            while event::poll(Duration::ZERO)? {
                if let Event::Key(key_event) = event::read()? {
                    if key_event.kind == KeyEventKind::Press
                        || key_event.kind == KeyEventKind::Repeat
                    {
                        if let Some(button) = key_handler(key_event) {
                            input.press(button);
                        }
                    }
                }
            }

            game.update(&input);
            game.draw(&mut fb);

            terminal.draw(|frame| {
                let area = frame.area();
                let scale = scale.unwrap_or_else(|| {
                    auto_scale(area.width, area.height, cell_ratio, fb.width(), fb.height())
                });
                let widget = HalfblockImage {
                    fb: &fb,
                    scale,
                    cell_ratio,
                };
                frame.render_widget(widget, area);
            })?;
        } else {
            let remaining = FRAME_DURATION - elapsed;
            std::thread::sleep(remaining.min(Duration::from_millis(1)));
        }
    }
}
