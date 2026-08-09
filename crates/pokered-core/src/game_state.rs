//! Top-level game screen state machine.
//!
//! Manages transitions between the title screen, main menu, Oak's intro,
//! the overworld, battles, and other top-level game screens.

use pokered_data::wild_data::GameVersion;

use crate::items::MartState;

/// Supported languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Zh,
}

impl Default for Lang {
    fn default() -> Self {
        Lang::En
    }
}

/// Top-level game screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameScreen {
    /// Game Freak shooting-star splash (boot; `PlayShootingStar`,
    /// engine/movie/intro.asm:305-341). Includes the copyright text.
    GameFreakSplash,
    CopyrightSplash,
    LanguageSelect,
    /// Gengar vs Nidorino intro battle animation (before title screen).
    IntroScene,
    TitleScreen,
    MainMenu,
    OakSpeech,
    Overworld,
    Battle,
    /// Poké Mart shop interface.
    Shop(MartState),
    StartMenu,
    OptionsMenu,
    SaveMenu,
    PartyScreen,
    PokemonStatsScreen(usize),
    /// Overworld ITEM bag (Start menu → ITEM).
    Bag,
    /// Town Map viewer (bag → TOWN MAP).
    TownMap,
    /// Game Corner slot-machine minigame.
    Slots,
    /// Elevator floor-selection menu (Celadon Mart / Silph Co / Rocket Hideout).
    Elevator,
    /// Filtered-bag menu (drink / fossil / badge-list selections).
    FilterBag,
    /// Full-screen diploma (completed-POKeDEX certificate).
    Diploma,
    /// Pokédex CONTENTS list + entry view (start menu → POKéDEX, and the
    /// post-capture "New DEX data" entry).
    Pokedex,
    /// Trainer card (start menu → player name): name, money, time, badges.
    TrainerCard,
    /// PC storage system: Bill's PC (#MON boxes), the player's item PC, and
    /// PROF.OAK's #DEX rating (engine/menus/pc.asm, players_pc.asm).
    PC,
}

/// Result of a single frame update for any screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenAction {
    /// Stay on the current screen.
    Continue,
    /// Transition to another screen.
    Transition(GameScreen),
}

/// Possible results from the main menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuChoice {
    /// Player chose CONTINUE (load existing save).
    Continue,
    /// Player chose NEW GAME.
    NewGame,
    /// Player chose OPTION (settings menu).
    Option,
    /// Player pressed B to go back to the title screen.
    BackToTitle,
}

/// Minimal save file summary shown on the CONTINUE screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveFileSummary {
    pub player_name: Vec<u8>,
    pub badges: u8,
    pub pokedex_owned: u8,
    pub play_time_hours: u16,
    pub play_time_minutes: u8,
    pub play_time_seconds: u8,
}

impl SaveFileSummary {
    pub fn badge_count(&self) -> u8 {
        self.badges.count_ones() as u8
    }
}

/// Game-wide configuration that persists across screens.
#[derive(Debug, Clone)]
pub struct GameConfig {
    pub version: GameVersion,
    pub language: Lang,
    pub text_speed: TextSpeed,
    pub battle_animation: bool,
    pub battle_style: BattleStyle,
}

impl GameConfig {
    pub fn new(version: GameVersion) -> Self {
        Self {
            version,
            language: Lang::default(),
            text_speed: TextSpeed::Medium,
            battle_animation: true,
            battle_style: BattleStyle::Shift,
        }
    }
}

impl Default for GameConfig {
    fn default() -> Self {
        Self::new(GameVersion::Red)
    }
}

/// Text printing speed option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSpeed {
    Fast,
    Medium,
    Slow,
}

impl TextSpeed {
    /// Frames between revealed typewriter characters — the original's
    /// `wOptions` text-delay value (`TEXT_DELAY_FAST/MEDIUM/SLOW` = 1/3/5,
    /// constants/ram_constants.asm), waited per letter by `PrintLetterDelay`
    /// (home/print_text.asm).
    pub fn delay_frames(self) -> u16 {
        match self {
            Self::Fast => 1,
            Self::Medium => 3,
            Self::Slow => 5,
        }
    }
}

/// Battle style option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleStyle {
    /// Player is prompted to switch Pokémon when opponent sends out a new one.
    Shift,
    /// No switching prompt.
    Set,
}

/// Top-level game state that owns the current screen and config.
#[derive(Debug, Clone)]
pub struct GameState {
    pub screen: GameScreen,
    pub config: GameConfig,
    pub save_summary: Option<SaveFileSummary>,
}

impl GameState {
    pub fn new(version: GameVersion) -> Self {
        Self {
            screen: GameScreen::GameFreakSplash,
            config: GameConfig::new(version),
            save_summary: None,
        }
    }

    /// Transition to a new screen.
    pub fn transition_to(&mut self, screen: GameScreen) {
        self.screen = screen;
    }

    /// Check if a save file exists.
    pub fn has_save_file(&self) -> bool {
        self.save_summary.is_some()
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self::new(GameVersion::Red)
    }
}
