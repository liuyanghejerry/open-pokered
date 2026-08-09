//! Title screen state machine.
//!
//! Replicates the title screen from `engine/movie/title.asm`:
//! 1. Copyright splash (brief)
//! 2. Pokémon logo loads, bounces down into place
//! 3. Game version text scrolls in from the right
//! 4. Title music starts
//! 5. Pokémon sprites cycle on screen while waiting for user input
//! 6. On any button press → play the current mon's cry → transition to MainMenu
//!
//! This module handles the logical state (animation phase, timers, current mon).
//! Rendering is handled by the renderer crate based on the state exposed here.

use pokered_data::species::Species;
use pokered_data::wild_data::GameVersion;

use crate::game_state::{GameScreen, ScreenAction};

/// Pokémon shown on the Red version title screen.
pub const TITLE_MONS_RED: [Species; 16] = [
    Species::Charmander, // STARTER1
    Species::Squirtle,   // STARTER2
    Species::Bulbasaur,  // STARTER3
    Species::Weedle,
    Species::NidoranM,
    Species::Scyther,
    Species::Pikachu,
    Species::Clefairy,
    Species::Rhydon,
    Species::Abra,
    Species::Gastly,
    Species::Ditto,
    Species::Pidgeotto,
    Species::Onix,
    Species::Ponyta,
    Species::Magikarp,
];

/// Pokémon shown on the Blue version title screen.
pub const TITLE_MONS_BLUE: [Species; 16] = [
    Species::Squirtle,   // STARTER2
    Species::Charmander, // STARTER1
    Species::Bulbasaur,  // STARTER3
    Species::Mankey,
    Species::Hitmonlee,
    Species::Vulpix,
    Species::Chansey,
    Species::Aerodactyl,
    Species::Jolteon,
    Species::Snorlax,
    Species::Gloom,
    Species::Poliwag,
    Species::Doduo,
    Species::Porygon,
    Species::Gengar,
    Species::Raichu,
];

/// Animation phases of the title screen, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitlePhase {
    /// Copyright splash screen (brief display before title).
    Copyright,
    /// Load graphics, prepare screen (instantaneous in our Rust version).
    Init,
    /// Pokémon logo bounces into position.
    LogoBounce,
    /// Brief pause after logo lands.
    LogoPause,
    /// Game version text scrolls in from the right.
    VersionScroll,
    /// Title music plays, Pokémon sprites cycle. Waiting for user input.
    WaitingForInput,
    /// Current mon is scrolling out to the left.
    ScrollOut,
    /// New mon is scrolling in from the right.
    ScrollIn,
    /// User pressed a button — playing the current mon's cry.
    PlayingCry,
    /// Fade to white, then transition to main menu.
    FadeOut,
    /// Done — ready to transition.
    Done,
}

/// Scroll speed table entry: (pixels_per_frame, frame_count).
/// Derived from the original ASM where each byte encodes hi_nibble=speed, lo_nibble=duration.
type ScrollEntry = (u8, u8);

/// Scroll the current mon out to the left (accelerating).
/// Original: TitleScroll_Out = $12,$22,$32,$42,$52,$62,$83,$93,0
pub const SCROLL_OUT_TABLE: &[ScrollEntry] = &[
    (1, 2),
    (2, 2),
    (3, 2),
    (4, 2),
    (5, 2),
    (6, 2),
    (8, 3),
    (9, 3),
];

/// Scroll a new mon in from the right (decelerating).
/// Original: TitleScroll_In = $a2,$94,$84,$63,$52,$31,$11,0
pub const SCROLL_IN_TABLE: &[ScrollEntry] =
    &[(10, 2), (9, 4), (8, 4), (6, 3), (5, 2), (3, 1), (1, 1)];

/// Compute total pixel displacement for a scroll table.
const fn scroll_table_total_pixels(table: &[ScrollEntry]) -> i32 {
    let mut total = 0i32;
    let mut i = 0;
    while i < table.len() {
        total += table[i].0 as i32 * table[i].1 as i32;
        i += 1;
    }
    total
}

/// Total pixels the scroll-out table covers (used to set initial scroll-in offset).
pub const SCROLL_OUT_TOTAL_PIXELS: i32 = scroll_table_total_pixels(SCROLL_OUT_TABLE);

/// Total pixels the scroll-in table covers.
pub const SCROLL_IN_TOTAL_PIXELS: i32 = scroll_table_total_pixels(SCROLL_IN_TABLE);

/// Logo bounce data: (scroll_delta, repeat_count) pairs.
/// Matches .TitleScreenPokemonLogoYScrolls from title.asm
pub const LOGO_BOUNCE_TABLE: &[(i8, u8)] =
    &[(-4, 16), (3, 4), (-3, 4), (2, 2), (-2, 2), (1, 2), (-1, 2)];

/// Total frames the logo bounce takes.
pub const LOGO_BOUNCE_TOTAL_FRAMES: u32 = {
    let mut total = 0u32;
    let mut i = 0;
    while i < 7 {
        total += LOGO_BOUNCE_TABLE[i].1 as u32;
        i += 1;
    }
    total
};

/// Frames to pause after logo lands (matches: ld c, 36 / call DelayFrames).
pub const LOGO_PAUSE_FRAMES: u32 = 36;

/// Frames for the version text scroll animation.
pub const VERSION_SCROLL_FRAMES: u32 = 36;

/// Frames between Pokémon switches on the title screen.
/// In the original: `ld c, 200 / call CheckForUserInterruption`.
pub const MON_DISPLAY_FRAMES: u32 = 200;

/// Frames for playing a cry before transitioning.
pub const CRY_PLAY_FRAMES: u32 = 60;

/// Frames for the fade-out effect.
pub const FADE_OUT_FRAMES: u32 = 16;

/// Frames to display the copyright splash before transitioning to title screen.
/// The original game shows copyright during the boot sequence.
pub const COPYRIGHT_FRAMES: u32 = 120; // ~2 seconds at 60fps

/// Title screen state.
#[derive(Debug, Clone)]
pub struct TitleScreenState {
    /// Current animation phase.
    pub phase: TitlePhase,
    /// Frame counter within the current phase.
    pub frame_counter: u32,
    /// Current background scroll Y (for logo bounce animation).
    pub scroll_y: i32,
    /// Which Pokémon is currently displayed.
    pub current_mon: Species,
    /// Index into the title mons list for the current mon.
    pub current_mon_index: usize,
    /// Game version (affects mon list and version text).
    pub version: GameVersion,
    /// Whether the player character sprite is visible.
    pub player_visible: bool,
    /// Whether the Pokémon logo is visible.
    pub logo_visible: bool,
    /// Whether the version text is visible.
    pub version_text_visible: bool,
    /// Progress of the version text scroll (0.0 = offscreen, 1.0 = in place).
    pub version_scroll_progress: f32,
    /// Current logo bounce step index.
    bounce_step: usize,
    /// Frames remaining in the current bounce step.
    bounce_frames_remaining: u8,
    /// Simple RNG state for picking random mons.
    rng_state: u32,
    /// Pixel offset applied to the Pokémon sprite during scroll transitions.
    /// 0 = normal position. Negative = shifted left (scroll out). Positive = shifted right (scroll in).
    pub mon_scroll_offset: i32,
    /// Index into the current scroll speed table.
    scroll_table_index: usize,
    /// Frames remaining at the current scroll speed.
    scroll_frames_remaining: u8,
}

impl TitleScreenState {
    pub fn new(version: GameVersion) -> Self {
        let first_mon = match version {
            GameVersion::Red => TITLE_MONS_RED[0],
            GameVersion::Blue => TITLE_MONS_BLUE[0],
        };
        Self {
            phase: TitlePhase::Copyright,
            frame_counter: 0,
            scroll_y: 64,
            current_mon: first_mon,
            current_mon_index: 0,
            version,
            player_visible: false,
            logo_visible: false,
            version_text_visible: false,
            version_scroll_progress: 0.0,
            bounce_step: 0,
            bounce_frames_remaining: 0,
            rng_state: 0x12345678,
            mon_scroll_offset: 0,
            scroll_table_index: 0,
            scroll_frames_remaining: 0,
        }
    }

    /// Seed the RNG (for deterministic testing).
    pub fn set_rng_seed(&mut self, seed: u32) {
        self.rng_state = seed;
    }

    /// Get the title mons list for the current version.
    pub fn title_mons(&self) -> &[Species; 16] {
        match self.version {
            GameVersion::Red => &TITLE_MONS_RED,
            GameVersion::Blue => &TITLE_MONS_BLUE,
        }
    }

    /// Simple xorshift RNG.
    fn next_random(&mut self) -> u32 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng_state = x;
        x
    }

    /// Pick a random mon different from the current one (matching original logic).
    fn pick_new_mon(&mut self) -> Species {
        let mons = match self.version {
            GameVersion::Red => &TITLE_MONS_RED,
            GameVersion::Blue => &TITLE_MONS_BLUE,
        };
        loop {
            let index = (self.next_random() & 0xF) as usize;
            let candidate = mons[index];
            if candidate != self.current_mon {
                self.current_mon_index = index;
                return candidate;
            }
        }
    }

    /// Begin scrolling out: reset scroll state to iterate SCROLL_OUT_TABLE.
    fn begin_scroll_out(&mut self) {
        self.phase = TitlePhase::ScrollOut;
        self.mon_scroll_offset = 0;
        self.scroll_table_index = 0;
        self.scroll_frames_remaining = SCROLL_OUT_TABLE[0].1;
    }

    /// Begin scrolling in: pick new mon, set offset far right, iterate SCROLL_IN_TABLE.
    fn begin_scroll_in(&mut self) {
        self.current_mon = self.pick_new_mon();
        self.phase = TitlePhase::ScrollIn;
        self.mon_scroll_offset = SCROLL_IN_TOTAL_PIXELS;
        self.scroll_table_index = 0;
        self.scroll_frames_remaining = SCROLL_IN_TABLE[0].1;
    }

    /// Advance one frame of a scroll table. Returns true when the table is exhausted.
    fn advance_scroll(&mut self, table: &[ScrollEntry], direction: i32) -> bool {
        if self.scroll_table_index >= table.len() {
            return true;
        }
        let (speed, _) = table[self.scroll_table_index];
        self.mon_scroll_offset += speed as i32 * direction;
        self.scroll_frames_remaining -= 1;
        if self.scroll_frames_remaining == 0 {
            self.scroll_table_index += 1;
            if self.scroll_table_index < table.len() {
                self.scroll_frames_remaining = table[self.scroll_table_index].1;
            }
        }
        self.scroll_table_index >= table.len()
    }

    /// Update one frame. Returns what action to take.
    ///
    /// `any_button_pressed`: true if the player pressed any button this frame.
    pub fn update_frame(&mut self, any_button_pressed: bool) -> ScreenAction {
        match self.phase {
            TitlePhase::Copyright => {
                self.frame_counter += 1;
                if self.frame_counter >= COPYRIGHT_FRAMES || any_button_pressed {
                    self.phase = TitlePhase::Init;
                    self.frame_counter = 0;
                }
                ScreenAction::Continue
            }

            TitlePhase::Init => {
                self.logo_visible = true;
                self.player_visible = true;
                self.phase = TitlePhase::LogoBounce;
                self.frame_counter = 0;
                self.bounce_step = 0;
                if !LOGO_BOUNCE_TABLE.is_empty() {
                    self.bounce_frames_remaining = LOGO_BOUNCE_TABLE[0].1;
                }
                ScreenAction::Continue
            }

            TitlePhase::LogoBounce => {
                if self.bounce_step < LOGO_BOUNCE_TABLE.len() {
                    let (delta, _count) = LOGO_BOUNCE_TABLE[self.bounce_step];
                    self.scroll_y += delta as i32;
                    self.bounce_frames_remaining -= 1;
                    if self.bounce_frames_remaining == 0 {
                        self.bounce_step += 1;
                        if self.bounce_step < LOGO_BOUNCE_TABLE.len() {
                            self.bounce_frames_remaining = LOGO_BOUNCE_TABLE[self.bounce_step].1;
                        }
                    }
                    self.frame_counter += 1;
                }

                if self.bounce_step >= LOGO_BOUNCE_TABLE.len() {
                    self.phase = TitlePhase::LogoPause;
                    self.frame_counter = 0;
                }
                ScreenAction::Continue
            }

            TitlePhase::LogoPause => {
                self.frame_counter += 1;
                if self.frame_counter >= LOGO_PAUSE_FRAMES {
                    self.phase = TitlePhase::VersionScroll;
                    self.frame_counter = 0;
                    self.version_text_visible = true;
                }
                ScreenAction::Continue
            }

            TitlePhase::VersionScroll => {
                self.frame_counter += 1;
                self.version_scroll_progress =
                    (self.frame_counter as f32) / (VERSION_SCROLL_FRAMES as f32);
                if self.version_scroll_progress > 1.0 {
                    self.version_scroll_progress = 1.0;
                }
                if self.frame_counter >= VERSION_SCROLL_FRAMES {
                    self.phase = TitlePhase::WaitingForInput;
                    self.frame_counter = 0;
                }
                ScreenAction::Continue
            }

            TitlePhase::WaitingForInput => {
                if any_button_pressed {
                    self.phase = TitlePhase::PlayingCry;
                    self.frame_counter = 0;
                    return ScreenAction::Continue;
                }

                self.frame_counter += 1;
                if self.frame_counter >= MON_DISPLAY_FRAMES {
                    self.begin_scroll_out();
                }
                ScreenAction::Continue
            }

            TitlePhase::ScrollOut => {
                if any_button_pressed {
                    // Snap back to normal position and play cry
                    self.mon_scroll_offset = 0;
                    self.phase = TitlePhase::PlayingCry;
                    self.frame_counter = 0;
                    return ScreenAction::Continue;
                }
                // direction = -1: offset goes negative (sprite moves left)
                let done = self.advance_scroll(SCROLL_OUT_TABLE, -1);
                if done {
                    self.begin_scroll_in();
                }
                ScreenAction::Continue
            }

            TitlePhase::ScrollIn => {
                if any_button_pressed {
                    // Snap to final position and play cry
                    self.mon_scroll_offset = 0;
                    self.phase = TitlePhase::PlayingCry;
                    self.frame_counter = 0;
                    return ScreenAction::Continue;
                }
                // direction = -1: offset decreases from positive toward 0 (sprite slides in from right)
                let done = self.advance_scroll(SCROLL_IN_TABLE, -1);
                if done {
                    self.mon_scroll_offset = 0;
                    self.phase = TitlePhase::WaitingForInput;
                    self.frame_counter = 0;
                }
                ScreenAction::Continue
            }

            TitlePhase::PlayingCry => {
                self.frame_counter += 1;
                if self.frame_counter >= CRY_PLAY_FRAMES {
                    self.phase = TitlePhase::FadeOut;
                    self.frame_counter = 0;
                }
                ScreenAction::Continue
            }

            TitlePhase::FadeOut => {
                self.frame_counter += 1;
                if self.frame_counter >= FADE_OUT_FRAMES {
                    self.phase = TitlePhase::Done;
                    return ScreenAction::Transition(GameScreen::MainMenu);
                }
                ScreenAction::Continue
            }

            TitlePhase::Done => ScreenAction::Transition(GameScreen::MainMenu),
        }
    }

    /// Skip directly to WaitingForInput phase (for comparison/testing).
    /// This bypasses copyright, logo bounce, and version scroll animations.
    pub fn skip_to_waiting_for_input(&mut self) {
        self.phase = TitlePhase::WaitingForInput;
        self.frame_counter = 0;
        self.scroll_y = 0;
        self.logo_visible = true;
        self.player_visible = true;
        self.version_text_visible = true;
        self.version_scroll_progress = 1.0;
        self.bounce_step = LOGO_BOUNCE_TABLE.len();
        self.bounce_frames_remaining = 0;
        self.mon_scroll_offset = 0;
        self.scroll_table_index = 0;
        self.scroll_frames_remaining = 0;
    }

    /// Skip to WaitingForInput with a specific Pokemon (for comparison/testing).
    pub fn skip_to_waiting_for_input_with_mon(&mut self, mon: Species) {
        self.skip_to_waiting_for_input();
        self.current_mon = mon;
    }

    /// Reset to initial state (for returning from main menu).
    pub fn reset(&mut self) {
        let first_mon = match self.version {
            GameVersion::Red => TITLE_MONS_RED[0],
            GameVersion::Blue => TITLE_MONS_BLUE[0],
        };
        self.phase = TitlePhase::Copyright;
        self.frame_counter = 0;
        self.scroll_y = 64;
        self.current_mon = first_mon;
        self.current_mon_index = 0;
        self.player_visible = false;
        self.logo_visible = false;
        self.version_text_visible = false;
        self.version_scroll_progress = 0.0;
        self.bounce_step = 0;
        self.bounce_frames_remaining = 0;
        self.mon_scroll_offset = 0;
        self.scroll_table_index = 0;
        self.scroll_frames_remaining = 0;
    }

    /// Check if currently showing copyright screen.
    pub fn is_copyright(&self) -> bool {
        self.phase == TitlePhase::Copyright
    }

    /// Get fade progress (0.0 = no fade, 1.0 = fully white).
    pub fn fade_progress(&self) -> f32 {
        if self.phase == TitlePhase::FadeOut {
            (self.frame_counter as f32) / (FADE_OUT_FRAMES as f32)
        } else {
            0.0
        }
    }
}
