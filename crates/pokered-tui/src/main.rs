mod game;

#[cfg(test)]
#[path = "../tests/common/visual_verify_zh_descriptions.rs"]
mod visual_verify_zh_descriptions;

use core::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::Parser;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use pokered_data::wild_data::GameVersion;
use pokered_renderer::input::GbButton;

use crate::game::TerminalGame;

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

    let mut wrapped = TerminalGame::new(GameVersion::Red, Arc::clone(&quit));

    dotzuki_tui::run(
        &mut wrapped,
        {
            let q = Arc::clone(&quit);
            move |ev| terminal_button(ev, &q)
        },
        cli.scale,
        cli.cell_ratio,
        160,
        144,
    )?;

    Ok(())
}

fn terminal_button(ev: KeyEvent, quit: &AtomicBool) -> Option<GbButton> {
    if ev.kind == KeyEventKind::Press && ev.code == KeyCode::Esc {
        quit.store(true, Ordering::Relaxed);
        None
    } else if ev.kind == KeyEventKind::Press || ev.kind == KeyEventKind::Repeat {
        keycode_to_gb_button(ev.code)
    } else {
        None
    }
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

#[cfg(test)]
mod input_tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    #[test]
    fn terminal_bindings_keep_all_existing_keys() {
        for (code, button) in [
            (KeyCode::Up, GbButton::Up),
            (KeyCode::Down, GbButton::Down),
            (KeyCode::Left, GbButton::Left),
            (KeyCode::Right, GbButton::Right),
            (KeyCode::Char('z'), GbButton::A),
            (KeyCode::Char('Z'), GbButton::A),
            (KeyCode::Char('x'), GbButton::B),
            (KeyCode::Char('X'), GbButton::B),
            (KeyCode::Enter, GbButton::Start),
            (KeyCode::Char(' '), GbButton::Start),
            (KeyCode::Backspace, GbButton::Select),
        ] {
            assert_eq!(keycode_to_gb_button(code), Some(button));
        }
        assert_eq!(keycode_to_gb_button(KeyCode::Char('q')), None);
    }

    #[test]
    fn key_events_accept_repeats_ignore_releases_and_escape_quits() {
        let quit = AtomicBool::new(false);
        for (kind, expected) in [
            (KeyEventKind::Press, Some(GbButton::A)),
            (KeyEventKind::Repeat, Some(GbButton::A)),
            (KeyEventKind::Release, None),
        ] {
            let event = KeyEvent::new_with_kind(KeyCode::Char('z'), KeyModifiers::NONE, kind);
            assert_eq!(terminal_button(event, &quit), expected);
            assert!(!quit.load(Ordering::Relaxed));
        }
        let escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(terminal_button(escape, &quit), None);
        assert!(quit.load(Ordering::Relaxed));
    }
}
