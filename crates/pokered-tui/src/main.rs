mod audio;
mod game;
mod render;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::Parser;
use crossterm::event::{KeyCode, KeyEventKind};
use pokered_core::data::wild_data::GameVersion;
use pokered_renderer::input::GbButton;

use crate::game::PokemonGame;

/// Wraps a PokemonGame with a quit flag so Esc exits the TUI loop.
struct QuittableGame {
    game: PokemonGame,
    quit: Arc<AtomicBool>,
}

impl jrpg_tui::TuiGame for QuittableGame {
    type Button = GbButton;

    fn update(&mut self, input: &jrpg_tui::InputState<Self::Button>) {
        self.game.update(input);
    }

    fn draw(&mut self, fb: &mut jrpg_renderer::FrameBuffer) {
        self.game.draw(fb);
    }

    fn exit_requested(&self) -> bool {
        self.quit.load(Ordering::Relaxed)
    }
}

#[derive(Parser)]
#[command(name = "pokered-tui", about = "Pokémon Red/Blue — Terminal UI")]
struct Cli {
    /// Fixed integer scale factor (auto-detected from terminal size if omitted)
    #[arg(short, long)]
    scale: Option<u32>,

    /// Terminal cell width:height ratio, e.g. 0.5 means cells are half as wide
    /// as they are tall. Adjust if the image looks stretched or squashed.
    #[arg(long, default_value_t = 0.8)]
    cell_ratio: f64,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let quit = Arc::new(AtomicBool::new(false));

    let mut wrapped = QuittableGame {
        game: PokemonGame::new(GameVersion::Red),
        quit: Arc::clone(&quit),
    };

    jrpg_tui::run(
        &mut wrapped,
        {
            let q = Arc::clone(&quit);
            move |ev| {
                if ev.kind == KeyEventKind::Press && ev.code == KeyCode::Esc {
                    q.store(true, Ordering::Relaxed);
                    None
                } else if ev.kind == KeyEventKind::Press
                    || ev.kind == KeyEventKind::Repeat
                {
                    keycode_to_gb_button(ev.code)
                } else {
                    None
                }
            }
        },
        cli.scale,
        cli.cell_ratio,
        160,
        144,
    )?;

    Ok(())
}

fn keycode_to_gb_button(keycode: KeyCode) -> Option<GbButton> {
    match keycode {
        KeyCode::Up => Some(GbButton::Up),
        KeyCode::Down => Some(GbButton::Down),
        KeyCode::Left => Some(GbButton::Left),
        KeyCode::Right => Some(GbButton::Right),
        KeyCode::Char('z') | KeyCode::Char('Z') => Some(GbButton::A),
        KeyCode::Char('x') | KeyCode::Char('X') => Some(GbButton::B),
        KeyCode::Enter | KeyCode::Char(' ') => Some(GbButton::Start),
        KeyCode::Backspace => Some(GbButton::Select),
        _ => None,
    }
}
