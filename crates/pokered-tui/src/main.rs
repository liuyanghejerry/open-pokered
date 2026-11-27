mod game;

#[cfg(test)]
#[path = "../tests/common/visual_verify_zh_descriptions.rs"]
mod visual_verify_zh_descriptions;

use core::sync::atomic::{AtomicBool, Ordering};
use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, ValueEnum};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use pokered_app::PokemonGame;
use pokered_core::game_state::Lang;
use pokered_data::wild_data::GameVersion;
use pokered_renderer::input::GbButton;

use crate::game::TerminalGame;

#[derive(Clone, Copy, ValueEnum)]
enum CliLang {
    En,
    Zh,
}

impl CliLang {
    fn to_lang(self) -> Lang {
        match self {
            CliLang::En => Lang::En,
            CliLang::Zh => Lang::Zh,
        }
    }
}

#[derive(Parser)]
#[command(name = "pokered-tui", about = "Pokémon Red — Terminal UI")]
struct Cli {
    /// Fixed integer scale factor (auto-detected from terminal size if omitted)
    #[arg(short, long)]
    scale: Option<u32>,

    /// Terminal cell width:height ratio, e.g. 0.5 means cells are half as wide
    /// as they are tall. Adjust if the image looks stretched or squashed.
    #[arg(long, default_value_t = 0.8)]
    cell_ratio: f64,

    /// Load/save a specific .sav file instead of the default save slot
    #[arg(long)]
    save: Option<PathBuf>,

    /// Boot from a JSON state snapshot
    #[arg(long)]
    snapshot: Option<PathBuf>,

    /// Initial text language (en or zh)
    #[arg(long, value_enum)]
    lang: Option<CliLang>,

    /// Skip the intro sequence and enter the overworld directly
    #[arg(long)]
    skip_intro: bool,

    /// Warp directly to MapName[,x,y] on startup
    #[arg(long)]
    warp: Option<String>,

    /// Disable audio output
    #[arg(long)]
    no_audio: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let quit = Arc::new(AtomicBool::new(false));
    let mut game = PokemonGame::new_with_options(
        GameVersion::Red,
        cli.save,
        cli.snapshot,
        None,
        cli.skip_intro,
        cli.warp,
        false,
        cli.no_audio,
    );
    if let Some(lang) = cli.lang {
        let l = lang.to_lang();
        game.state.config.language = l;
        game.overworld.set_script_lang(match l {
            Lang::En => "en",
            Lang::Zh => "zh",
        });
    }

    // Keep runtime diagnostic stderr prints from corrupting the alternate screen.
    pokered_app::set_quiet_stderr(true);
    let mut wrapped = TerminalGame::with_game(game, Arc::clone(&quit));

    let res = dotzuki_tui::run(
        &mut wrapped,
        {
            let q = Arc::clone(&quit);
            move |ev| terminal_button(ev, &q)
        },
        cli.scale,
        cli.cell_ratio,
        160,
        144,
    );
    pokered_app::set_quiet_stderr(false);
    res?;

    Ok(())
}

fn terminal_button(ev: KeyEvent, quit: &AtomicBool) -> Option<GbButton> {
    if ev.kind == KeyEventKind::Press
        && (ev.code == KeyCode::Esc
            || (ev.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(ev.code, KeyCode::Char('c' | 'C'))))
    {
        quit.store(true, Ordering::Relaxed);
        return None;
    }
    if ev
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return None;
    }
    if ev.kind == KeyEventKind::Press || ev.kind == KeyEventKind::Repeat {
        keycode_to_gb_button(ev.code)
    } else {
        None
    }
}

fn keycode_to_gb_button(keycode: KeyCode) -> Option<GbButton> {
    match keycode {
        KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') => Some(GbButton::Up),
        KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') => Some(GbButton::Down),
        KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => Some(GbButton::Left),
        KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => Some(GbButton::Right),
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
            (KeyCode::Char('w'), GbButton::Up),
            (KeyCode::Char('W'), GbButton::Up),
            (KeyCode::Char('s'), GbButton::Down),
            (KeyCode::Char('S'), GbButton::Down),
            (KeyCode::Char('a'), GbButton::Left),
            (KeyCode::Char('A'), GbButton::Left),
            (KeyCode::Char('d'), GbButton::Right),
            (KeyCode::Char('D'), GbButton::Right),
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
    fn key_events_accept_repeats_ignore_releases_and_escape_or_ctrl_c_quits() {
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
        // Ctrl/Alt modifiers must not trigger game buttons.
        let ctrl_a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(terminal_button(ctrl_a, &quit), None);
        assert!(!quit.load(Ordering::Relaxed));

        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(terminal_button(ctrl_c, &quit), None);
        assert!(quit.load(Ordering::Relaxed));

        quit.store(false, Ordering::Relaxed);
        let escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(terminal_button(escape, &quit), None);
        assert!(quit.load(Ordering::Relaxed));
    }
}
