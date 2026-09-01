use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "pokered", about = "Pokémon Red/Blue — Rust Rewrite")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Enable debug logging for specific modules (comma-separated).
    /// Available modules: save, overworld, battle, menu, audio, warp, event, render, all.
    /// Example: --debug-modules save,overworld
    #[arg(long, global = true)]
    pub debug_modules: Option<String>,

    /// Path to the maps directory containing per-map folders with script.js/script_config.json.
    /// Only used when compiled without the `embedded-scripts` feature.
    /// Defaults to crates/pokered-data/maps/ when not specified.
    #[arg(long, global = true)]
    pub scripts_dir: Option<PathBuf>,

    /// Start a TCP debug server on the given port (requires `debug-server` feature).
    /// Accepts JSON-line protocol commands for inspecting and controlling game state.
    #[arg(long, global = true)]
    pub debug_port: Option<u16>,

    /// Run the JRPG multi-layer demo map instead of Pokémon Red.
    /// Shows a ground layer, decoration layer, player entity, and camera follow.
    #[arg(long, global = true)]
    pub demo: bool,

    /// Enable file watching for asset hot-reloading.
    /// Watches .tmx, .png, .js files in the assets/ directory and reloads them
    /// on change during development. Only available in debug builds.
    #[arg(long, global = true)]
    pub watch: bool,

    /// Host a link-play (Cable Club) server on the given port. The game
    /// waits for one peer, then runs the link handshake.
    #[arg(long, global = true, conflicts_with = "link_connect")]
    pub link_listen: Option<u16>,

    /// Join a link-play game as the client: "host:port" (e.g. 127.0.0.1:5000).
    #[arg(long, global = true, conflicts_with = "link_listen")]
    pub link_connect: Option<String>,
}

/// Language selector for the capture commands (screenshots/battle). Maps to
/// `pokered_core::game_state::Lang` and additionally syncs the overworld
/// script engine ("en" / "zh") so `@t` dialogue renders bilingually.
#[derive(Clone, Copy, ValueEnum)]
pub enum CliLang {
    En,
    Zh,
}

impl CliLang {
    pub fn to_lang(self) -> pokered_core::game_state::Lang {
        match self {
            CliLang::En => pokered_core::game_state::Lang::En,
            CliLang::Zh => pokered_core::game_state::Lang::Zh,
        }
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run the game in windowed mode (default)
    Run {
        /// Path to a regular SRAM .sav file to load at startup
        #[arg(long)]
        save: Option<PathBuf>,
        /// Path to a JSON snapshot of SaveData to load at startup (for debug/testing)
        #[arg(long)]
        snapshot: Option<PathBuf>,
        /// Skip intro sequence (Copyright→Title→MainMenu→OakSpeech) and start at Overworld
        #[arg(long)]
        skip_intro: bool,
        /// Disable audio output. Recommended for headless/debug-server
        /// driving: synchronous step_frames bursts otherwise pace against
        /// the real-time audio stream, slowing scripted runs enormously.
        #[arg(long)]
        no_audio: bool,
        /// Warp to a specific map and coordinates on startup. Format: "MapName[,x,y]"
        /// e.g. --warp PalletTown,10,14 or --warp CeruleanCity,14,8
        #[arg(long)]
        warp: Option<String>,
        /// Start a TCP debug server on the given port.
        #[arg(long)]
        debug_port: Option<u16>,
        /// Run without creating a window (no rendering, deterministic frame
        /// loop). Intended for CI/debug-server driving: combine with
        /// --debug-port and the step_frames command.
        #[arg(long)]
        headless: bool,
        /// Render the state after startup (honoring --save/--snapshot/
        /// --skip-intro/--warp) to a PNG and exit, without opening a window.
        /// This is how driven/saved states get before/after captures —
        /// `screenshot --screen` only reaches fixed boot screens.
        #[arg(long)]
        screenshot: Option<PathBuf>,
        /// Frames to advance (neutral input) before the --screenshot
        /// capture, letting warp fades and arrival animations settle.
        #[arg(long, default_value = "30")]
        screenshot_frames: u32,
        /// Record every frame to DIR/frame-NNNNNN.png (headless or windowed).
        /// Driven runs (debug-server step_frames bursts included) can then be
        /// assembled into video offline, e.g.:
        /// `ffmpeg -framerate 240 -i frame-%06d.png -r 60 out.mp4`.
        /// For full-run video prefer --record-video, which needs no
        /// intermediate files.
        #[arg(long)]
        record_frames: Option<PathBuf>,
        /// Record every frame to FILE as H.264 video by streaming raw frames
        /// into a spawned ffmpeg process (must be on PATH). Same capture
        /// cadence as --record-frames but without the per-frame PNG encode
        /// or the thousands of intermediate files: the .mp4 is finalized
        /// when the game exits.
        #[arg(long, conflicts_with = "record_frames")]
        record_video: Option<PathBuf>,
        /// Game frames per second of --record-video playback time. The game
        /// runs at 60 fps, so 240 plays back at 4× real-time; the output is
        /// resampled to 60 fps.
        #[arg(long, default_value = "240", requires = "record_video")]
        record_video_fps: u32,
        /// Text language (menus, battle text, scene dialogue). Defaults to
        /// the language-select screen flow; explicit values skip choosing.
        #[arg(long, value_enum, default_value_t = CliLang::En)]
        lang: CliLang,
    },
    /// Export the current save file (or a specified .sav) as a JSON snapshot
    ExportSnapshot {
        /// Input SRAM .sav file path (defaults to the auto-detected pokered.sav)
        #[arg(long)]
        input: Option<PathBuf>,
        /// Output JSON snapshot file path
        #[arg(short, long, default_value = "snapshot.json")]
        output: PathBuf,
    },
    /// Dump a .sav file to a JSON snapshot (same output as `export-snapshot`,
    /// input required). The binary has no JSON→.sav write-back — load a
    /// snapshot with `run --snapshot`.
    ImportSnapshot {
        /// Input .sav file path
        #[arg(short, long)]
        input: PathBuf,
        /// Output JSON snapshot file path
        #[arg(short, long, default_value = "snapshot.json")]
        output: PathBuf,
    },
    /// Capture a screenshot of a specific game screen
    Screenshot {
        /// Which screen to capture
        #[arg(short, long)]
        screen: ScreenTarget,
        /// Output PNG file path
        #[arg(short, long, default_value = "screenshot.png")]
        output: PathBuf,
        /// Number of frames to advance before capturing (for animation).
        /// Keep this generous: several screens typewriter their text or
        /// animate in, and too few frames captures a half-loaded picture.
        #[arg(short, long, default_value_t = 5)]
        frames: u32,
        /// Text language for the capture (dialogue, menus)
        #[arg(long, value_enum, default_value_t = CliLang::En)]
        lang: CliLang,
    },
    /// Capture screenshots of all game screens
    ScreenshotAll {
        /// Output directory for PNG files
        #[arg(short, long, default_value = "screenshots")]
        output_dir: PathBuf,
        /// Number of frames to advance before capturing each screen
        #[arg(short, long, default_value_t = 5)]
        frames: u32,
        /// Text language for the capture (dialogue, menus)
        #[arg(long, value_enum, default_value_t = CliLang::En)]
        lang: CliLang,
    },
    /// Dump game state as JSON to stdout (for comparison with PyBoy WRAM reads)
    DumpState {
        /// Which screen to transition to before dumping state
        #[arg(short, long)]
        screen: ScreenTarget,
        /// Number of frames to advance before dumping state
        #[arg(short, long, default_value_t = 0)]
        frames: u32,
    },
    /// Start a direct battle from a JSON config file (bypasses all menus/story)
    Battle {
        /// Path to the battle configuration JSON file
        #[arg(short, long)]
        config: PathBuf,
        /// Capture a screenshot to a PNG file instead of opening a window
        #[arg(short, long)]
        screenshot: Option<PathBuf>,
        /// Number of frames to advance before capturing the screenshot (default: 5)
        #[arg(long, default_value_t = 5)]
        frames: u32,
        /// Text language for the capture (menus, messages)
        #[arg(long, value_enum, default_value_t = CliLang::En)]
        lang: CliLang,
    },
}

#[derive(Clone, ValueEnum)]
pub enum ScreenTarget {
    GamefreakSplash,
    Copyright,
    Title,
    MainMenu,
    Oak,
    Overworld,
    Battle,
    StartMenu,
    Options,
    Save,
    TownMap,
    Slots,
    Elevator,
    FilterBag,
    Diploma,
    Pokedex,
    TrainerCard,
    Pc,
    Naming,
}

pub fn screen_target_to_game_screen(target: &ScreenTarget) -> pokered_core::game_state::GameScreen {
    use pokered_core::game_state::GameScreen;
    use ScreenTarget::*;

    match target {
        GamefreakSplash => GameScreen::GameFreakSplash,
        Copyright => GameScreen::CopyrightSplash,
        Title => GameScreen::TitleScreen,
        MainMenu => GameScreen::MainMenu,
        Oak => GameScreen::OakSpeech,
        Overworld => GameScreen::Overworld,
        Battle => GameScreen::Battle,
        StartMenu => GameScreen::StartMenu,
        Options => GameScreen::OptionsMenu,
        Save => GameScreen::SaveMenu,
        TownMap => GameScreen::TownMap,
        Slots => GameScreen::Slots,
        Elevator => GameScreen::Elevator,
        FilterBag => GameScreen::FilterBag,
        Diploma => GameScreen::Diploma,
        Pokedex => GameScreen::Pokedex,
        TrainerCard => GameScreen::TrainerCard,
        Pc => GameScreen::PC,
        Naming => GameScreen::OakSpeech,
    }
}

pub fn screen_name(screen: &pokered_core::game_state::GameScreen) -> &'static str {
    use pokered_core::game_state::GameScreen;

    match screen {
        GameScreen::GameFreakSplash => "gamefreak-splash",
        GameScreen::CopyrightSplash => "copyright",
        GameScreen::LanguageSelect => "language-select",
        GameScreen::TitleScreen => "title",
        GameScreen::MainMenu => "main-menu",
        GameScreen::OakSpeech => "oak",
        GameScreen::Overworld => "overworld",
        GameScreen::Battle => "battle",
        GameScreen::StartMenu => "start-menu",
        GameScreen::IntroScene => "intro",
        GameScreen::OptionsMenu => "options",
        GameScreen::SaveMenu => "save",
        GameScreen::PartyScreen => "party",
        GameScreen::PokemonStatsScreen(_) => "stats",
        GameScreen::Shop(_) => "shop",
        GameScreen::Bag => "bag",
        GameScreen::TownMap => "town-map",
        GameScreen::Slots => "slots",
        GameScreen::Elevator => "elevator",
        GameScreen::FilterBag => "filter-bag",
        GameScreen::Diploma => "diploma",
        GameScreen::Pokedex => "pokedex",
        GameScreen::TrainerCard => "trainer-card",
        GameScreen::PC => "pc",
    }
}

pub const ALL_SCREENS: &[pokered_core::game_state::GameScreen] = &[
    pokered_core::game_state::GameScreen::CopyrightSplash,
    pokered_core::game_state::GameScreen::TitleScreen,
    pokered_core::game_state::GameScreen::MainMenu,
    pokered_core::game_state::GameScreen::OakSpeech,
    pokered_core::game_state::GameScreen::Overworld,
    pokered_core::game_state::GameScreen::Battle,
    pokered_core::game_state::GameScreen::StartMenu,
    pokered_core::game_state::GameScreen::IntroScene,
    pokered_core::game_state::GameScreen::OptionsMenu,
    pokered_core::game_state::GameScreen::SaveMenu,
    pokered_core::game_state::GameScreen::PartyScreen,
    pokered_core::game_state::GameScreen::PokemonStatsScreen(0),
];
