//! Game Freak shooting-star splash — the pre-title sequence from
//! `PlayShootingStar` (engine/movie/intro.asm:305-341) and
//! `AnimateShootingStar` (engine/movie/splash.asm:27-146).
//!
//! Sequence (all timings in 60 fps frames, from the asm):
//! 1. [`SplashPhase::BlackDelay`] — the copyright text on black, 180 frames
//!    (`ld c, 180 / call DelayFrames`, intro.asm:312). NOT skippable.
//! 2. [`SplashPhase::Setup`] — screen clears, black letterbox bars
//!    (`IntroDrawBlackBars`) + the Game Freak logo appear, 64 frames
//!    (`ld c, 64`, intro.asm:326). NOT skippable.
//! 3. [`SplashPhase::BigStar`] — `SFX_SHOOTING_STAR` plays
//!    (splash.asm:29-30) while a big star crosses the screen diagonally from
//!    the top-right corner at 4 px/frame (`Y += 4, X -= 4`, splash.asm:39-44)
//!    for 40 frames (until the first sprite's OAM Y = $a0; the
//!    `cp 80 / jr .bigStarLoop` quirk at splash.asm:54-57 makes it pass Y=80
//!    once, so the loop ends at 160, not 80).
//! 4. [`SplashPhase::LogoFlash`] — the logo flashes 3 times, 10 frames each
//!    (`ld b, 3` / `rrc [rOBP0]` ×2 / `ld c, 10`, splash.asm:72-82).
//! 5. [`SplashPhase::SmallStars`] — small stars fall from the logo in 6
//!    waves (4 populated + 2 empty, splash.asm:152-158); each wave step drops
//!    every star 8 px at 1 px per 3 frames (`MoveDownSmallStars`,
//!    splash.asm:186-209) = 24 frames, toggling `rOBP1 ^= %10100000` every
//!    3 frames so the lower star in the tile blinks.
//! 6. [`SplashPhase::PostDelay`] — 40 frames (intro.asm:329-332); the
//!    original skips this delay when the user interrupted the animation
//!    (`jr c, .next`), which we model by jumping straight to `Done` on skip.
//!
//! Skippability matches `CheckForUserInterruption`
//! (home/overworld.asm:2395-2424): during phases 3-5, A or Start (or the
//! Up+Select+B combo) aborts the animation immediately. The two long
//! `DelayFrames` waits (180/64) are NOT interruptible in the original.
//!
//! `MUSIC_INTRO_BATTLE` (intro.asm:333-338) is NOT started here — in this
//! port the intro-fight scene (`IntroScene`) already starts it on entry.

use crate::alloc_prelude::*;
use crate::game_state::{GameScreen, ScreenAction};

/// Frames for the initial copyright-on-black delay (intro.asm:312).
pub const BLACK_DELAY_FRAMES: u16 = 180;
/// Frames between the logo/bars appearing and the star animation
/// (intro.asm:326).
pub const SETUP_DELAY_FRAMES: u16 = 64;
/// Big-star frames: 4 px/frame from OAM (X=160, Y=0) until Y = $a0
/// (splash.asm:34-60).
pub const BIG_STAR_FRAMES: u16 = 40;
/// Big-star speed, px per frame, along both axes (splash.asm:40-43).
pub const BIG_STAR_PX_PER_FRAME: i32 = 4;
/// Big-star top-left sprite OAM position at start (`dbsprite 20, 0`,
/// splash.asm:231): OAM X=160, Y=0.
pub const BIG_STAR_START_OAM_X: i32 = 160;
pub const BIG_STAR_START_OAM_Y: i32 = 0;
/// Logo flashes (`ld b, 3`, splash.asm:73) at 10 frames each (splash.asm:78).
pub const LOGO_FLASH_COUNT: u16 = 3;
pub const LOGO_FLASH_FRAMES: u16 = 10;
/// rOBP0 value at load (splash.asm:2-3); each flash rotates it right twice
/// (`rrc [rOBP0]` ×2, splash.asm:75-77).
pub const LOGO_OBP0: u8 = 0xf9;
/// Small-star waves (`ld c, 6`, splash.asm:101) — 4 populated + 2 empty.
pub const SMALL_STAR_WAVES: u16 = 6;
/// Frames per wave step: 8 iterations × 3 frames (splash.asm:187-208).
pub const SMALL_STAR_WAVE_FRAMES: u16 = 24;
/// Pixels each star falls per wave step (`ld b, 8`, splash.asm:187).
pub const SMALL_STAR_FALL_PX_PER_WAVE: i32 = 8;
/// Small stars spawn at OAM Y = $68 (splash.asm:163-182).
pub const SMALL_STAR_SPAWN_OAM_Y: i32 = 0x68;
/// OAM X coords of the 4 populated waves (splash.asm:163-182);
/// waves 5-6 are empty (`SmallStarsEmptyWave`, splash.asm:183-184).
pub const SMALL_STAR_WAVE_OAM_X: [[i32; 4]; 4] = [
    [0x30, 0x40, 0x58, 0x78],
    [0x38, 0x48, 0x60, 0x70],
    [0x34, 0x4c, 0x54, 0x64],
    [0x3c, 0x5c, 0x6c, 0x74],
];
/// Frames of post-animation delay, skipped on user interruption
/// (intro.asm:329-332).
pub const POST_DELAY_FRAMES: u16 = 40;

/// Animation phases of the Game Freak splash, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplashPhase {
    /// Copyright text on black (180 frames, not skippable).
    BlackDelay,
    /// Black bars + logo visible (64 frames, not skippable).
    Setup,
    /// Big star crossing the screen diagonally (40 frames, SFX plays).
    BigStar,
    /// Logo flashing (3 × 10 frames).
    LogoFlash,
    /// Small stars falling from the logo (6 waves × 24 frames).
    SmallStars,
    /// Post-animation delay (40 frames; skipped on interruption).
    PostDelay,
    /// Done — ready to transition.
    Done,
}

/// Input forwarded to the splash each frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct SplashInput {
    pub a: bool,
    pub b: bool,
    pub start: bool,
    pub select: bool,
    pub up: bool,
}

impl SplashInput {
    pub fn none() -> Self {
        Self::default()
    }

    /// `CheckForUserInterruption` (home/overworld.asm:2395-2424): the
    /// Up+Select+B held combo, or a fresh Start/A press.
    fn interrupts(self) -> bool {
        self.a || self.start || (self.up && self.select && self.b)
    }
}

/// Logical state of the Game Freak shooting-star splash. Rendering reads the
/// phase plus the `*_oam`/`logo_obp0`/`small_stars_oam` accessors.
#[derive(Debug, Clone)]
pub struct GameFreakSplashState {
    pub phase: SplashPhase,
    /// Frames elapsed in the current phase.
    frame: u16,
    /// Big-star top-left sprite OAM position (only meaningful in
    /// [`SplashPhase::BigStar`]).
    big_star_oam: (i32, i32),
    /// Current rOBP0 register, retained after the final logo flash and on skip.
    logo_obp0: u8,
    /// Set once when [`SplashPhase::BigStar`] is entered — the frontend
    /// plays `SFX_SHOOTING_STAR` (splash.asm:29-30).
    sfx_pending: bool,
}

impl Default for GameFreakSplashState {
    fn default() -> Self {
        Self::new()
    }
}

impl GameFreakSplashState {
    pub fn new() -> Self {
        Self {
            phase: SplashPhase::BlackDelay,
            frame: 0,
            big_star_oam: (BIG_STAR_START_OAM_X, BIG_STAR_START_OAM_Y),
            logo_obp0: LOGO_OBP0,
            sfx_pending: false,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Take the pending `SFX_SHOOTING_STAR` request. Fires at most once per
    /// run, at the start of [`SplashPhase::BigStar`].
    pub fn take_sfx_pending(&mut self) -> bool {
        core::mem::take(&mut self.sfx_pending)
    }

    /// Big-star top-left OAM position while [`SplashPhase::BigStar`] is
    /// active (the star is 2×2 sprites = 16×16 px).
    pub fn big_star_oam(&self) -> Option<(i32, i32)> {
        (self.phase == SplashPhase::BigStar).then_some(self.big_star_oam)
    }

    /// Current rOBP0 value: rotate before each 10-frame wait, then retain
    /// the final value (`$e7`) while the small stars fall and afterwards.
    pub fn logo_obp0(&self) -> u8 {
        self.logo_obp0
    }

    /// Small-star OAM positions (tile `$a2`, 8×8 px each) while
    /// [`SplashPhase::SmallStars`] is active. Wave `w` spawns at
    /// Y = $68 and falls 8 px per wave step, 1 px per 3 frames
    /// (`MoveDownSmallStars`, splash.asm:186-209). The first increment is
    /// performed before the first 3-frame wait, so the first visible Y is $69.
    pub fn small_stars_oam(&self) -> Vec<(i32, i32)> {
        if self.phase != SplashPhase::SmallStars {
            return Vec::new();
        }
        let wave = self.frame / SMALL_STAR_WAVE_FRAMES;
        let sub = self.frame % SMALL_STAR_WAVE_FRAMES;
        let partial = (sub / 3 + 1) as i32; // increment before each 3-frame wait
        let mut out = Vec::new();
        let last_populated = wave.min(SMALL_STAR_WAVE_OAM_X.len() as u16 - 1);
        for (w, xs) in SMALL_STAR_WAVE_OAM_X.iter().enumerate() {
            if w as u16 > last_populated {
                break;
            }
            let y = SMALL_STAR_SPAWN_OAM_Y
                + SMALL_STAR_FALL_PX_PER_WAVE * (wave - w as u16) as i32
                + partial;
            for &x in xs {
                out.push((x, y));
            }
        }
        out
    }

    /// True while the small-star palette is toggled (`rOBP1 ^= %10100000`
    /// every 3 frames, splash.asm:199-202) — the lower star in the tile
    /// blinks. The first wait already uses the toggled palette ($04).
    pub fn small_star_blink(&self) -> bool {
        if self.phase != SplashPhase::SmallStars {
            return false;
        }
        let sub = self.frame % SMALL_STAR_WAVE_FRAMES;
        (sub / 3) % 2 == 0
    }

    fn enter(&mut self, phase: SplashPhase) {
        self.phase = phase;
        self.frame = 0;
        if phase == SplashPhase::BigStar {
            // The asm moves all four sprites before its first 1-frame wait.
            self.big_star_oam = (
                BIG_STAR_START_OAM_X - BIG_STAR_PX_PER_FRAME,
                BIG_STAR_START_OAM_Y + BIG_STAR_PX_PER_FRAME,
            );
            self.sfx_pending = true;
        } else if phase == SplashPhase::LogoFlash {
            self.logo_obp0 = LOGO_OBP0.rotate_right(2);
        }
    }

    /// Advance one frame. Returns `Transition(LanguageSelect)` once the
    /// sequence is done (or was skipped); the copyright text is part of the
    /// splash itself ([`SplashPhase::BlackDelay`]), matching the original
    /// where the © screen is the first 180 frames of `PlayShootingStar`.
    pub fn update_frame(&mut self, input: SplashInput) -> ScreenAction {
        let interruptible = matches!(
            self.phase,
            SplashPhase::BigStar | SplashPhase::LogoFlash | SplashPhase::SmallStars
        );
        if interruptible && input.interrupts() {
            // `ret c` out of AnimateShootingStar; `jr c, .next` then skips
            // the 40-frame post delay (intro.asm:329-331).
            self.enter(SplashPhase::Done);
        }

        match self.phase {
            SplashPhase::BlackDelay => {
                self.frame += 1;
                if self.frame >= BLACK_DELAY_FRAMES {
                    self.enter(SplashPhase::Setup);
                }
                ScreenAction::Continue
            }
            SplashPhase::Setup => {
                self.frame += 1;
                if self.frame >= SETUP_DELAY_FRAMES {
                    self.enter(SplashPhase::BigStar);
                }
                ScreenAction::Continue
            }
            SplashPhase::BigStar => {
                // The current position has completed its 1-frame wait.
                self.frame += 1;
                if self.frame >= BIG_STAR_FRAMES {
                    self.enter(SplashPhase::LogoFlash);
                } else {
                    self.big_star_oam.0 -= BIG_STAR_PX_PER_FRAME;
                    self.big_star_oam.1 += BIG_STAR_PX_PER_FRAME;
                }
                ScreenAction::Continue
            }
            SplashPhase::LogoFlash => {
                self.frame += 1;
                if self.frame >= LOGO_FLASH_COUNT * LOGO_FLASH_FRAMES {
                    self.enter(SplashPhase::SmallStars);
                } else if self.frame % LOGO_FLASH_FRAMES == 0 {
                    self.logo_obp0 = self.logo_obp0.rotate_right(2);
                }
                ScreenAction::Continue
            }
            SplashPhase::SmallStars => {
                self.frame += 1;
                if self.frame >= SMALL_STAR_WAVES * SMALL_STAR_WAVE_FRAMES {
                    self.enter(SplashPhase::PostDelay);
                }
                ScreenAction::Continue
            }
            SplashPhase::PostDelay => {
                self.frame += 1;
                if self.frame >= POST_DELAY_FRAMES {
                    self.enter(SplashPhase::Done);
                }
                ScreenAction::Continue
            }
            SplashPhase::Done => ScreenAction::Transition(GameScreen::LanguageSelect),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press_a() -> SplashInput {
        SplashInput {
            a: true,
            ..SplashInput::none()
        }
    }

    /// The two long delays are plain `DelayFrames` in the original — NOT
    /// interruptible.
    #[test]
    fn delays_are_not_skippable() {
        let mut s = GameFreakSplashState::new();
        assert_eq!(s.phase, SplashPhase::BlackDelay);
        for _ in 0..BLACK_DELAY_FRAMES {
            assert_eq!(
                s.update_frame(press_a()),
                ScreenAction::Continue,
                "BlackDelay must ignore input"
            );
        }
        assert_eq!(s.phase, SplashPhase::Setup);
        for _ in 0..SETUP_DELAY_FRAMES {
            s.update_frame(press_a());
        }
        assert_eq!(s.phase, SplashPhase::BigStar);
        assert!(s.take_sfx_pending(), "SFX_SHOOTING_STAR at BigStar start");
        assert!(!s.take_sfx_pending(), "one-shot");
    }

    /// Big star: 40 frames at 4 px/frame from OAM (160, 0) to (0, 160)
    /// (splash.asm:34-60 — the `cp 80` quirk means the loop runs to Y=$a0).
    #[test]
    fn big_star_path_and_timing() {
        let mut s = GameFreakSplashState::new();
        s.enter(SplashPhase::BigStar);
        assert_eq!(s.big_star_oam(), Some((156, 4)), "moves before first wait");
        s.update_frame(SplashInput::none());
        assert_eq!(s.big_star_oam(), Some((152, 8)));
        for _ in 1..BIG_STAR_FRAMES {
            s.update_frame(SplashInput::none());
        }
        assert_eq!(s.phase, SplashPhase::LogoFlash);
    }

    /// Logo flash: 3 × 10 frames, rOBP0 rotated right twice per flash
    /// (splash.asm:72-82).
    #[test]
    fn logo_flash_timing_and_palette() {
        let mut s = GameFreakSplashState::new();
        s.enter(SplashPhase::LogoFlash);
        assert_eq!(s.logo_obp0(), 0x7e, "rotate before first wait");
        for _ in 0..LOGO_FLASH_FRAMES {
            s.update_frame(SplashInput::none());
        }
        assert_eq!(s.logo_obp0(), 0x9f);
        for _ in 0..LOGO_FLASH_FRAMES {
            s.update_frame(SplashInput::none());
        }
        assert_eq!(s.logo_obp0(), 0xe7);
        for _ in 0..LOGO_FLASH_FRAMES {
            s.update_frame(SplashInput::none());
        }
        assert_eq!(s.phase, SplashPhase::SmallStars);
        assert_eq!(s.logo_obp0(), 0xe7, "final palette persists after flashes");
    }

    /// Small stars: wave 1 spawns at Y=$68 with the asm X coords; each wave
    /// step drops every star 8 px at 1 px per 3 frames; the blink toggles
    /// every 3 frames (splash.asm:163-209).
    #[test]
    fn small_stars_waves_fall_and_blink() {
        let mut s = GameFreakSplashState::new();
        s.enter(SplashPhase::SmallStars);
        assert_eq!(
            s.small_stars_oam(),
            vec![(0x30, 0x69), (0x40, 0x69), (0x58, 0x69), (0x78, 0x69)]
        );
        assert!(s.small_star_blink(), "toggle before first wait");
        // The next 3-frame wait starts with another increment and toggle.
        for _ in 0..3 {
            s.update_frame(SplashInput::none());
        }
        assert_eq!(s.small_stars_oam()[0], (0x30, 0x6a));
        assert!(!s.small_star_blink());
        // Finish wave step 1 (24 frames total): wave 2 spawns, wave 1 is 8 px down.
        for _ in 3..SMALL_STAR_WAVE_FRAMES {
            s.update_frame(SplashInput::none());
        }
        let stars = s.small_stars_oam();
        assert_eq!(stars.len(), 8, "waves 1+2 visible");
        assert_eq!(stars[0], (0x30, 0x69 + 8));
        assert_eq!(stars[4], (0x38, 0x69));
        // All 6 wave steps (144 frames total; 24 elapsed) → PostDelay.
        for _ in 0..(SMALL_STAR_WAVES - 1) * SMALL_STAR_WAVE_FRAMES {
            s.update_frame(SplashInput::none());
        }
        assert_eq!(s.phase, SplashPhase::PostDelay);
    }

    /// Full uninterrupted run: 180+64+40+30+144+40 frames, then transition.
    #[test]
    fn full_run_transitions_to_language_select() {
        let mut s = GameFreakSplashState::new();
        let total = BLACK_DELAY_FRAMES
            + SETUP_DELAY_FRAMES
            + BIG_STAR_FRAMES
            + LOGO_FLASH_COUNT * LOGO_FLASH_FRAMES
            + SMALL_STAR_WAVES * SMALL_STAR_WAVE_FRAMES
            + POST_DELAY_FRAMES;
        for _ in 0..total {
            assert_eq!(s.update_frame(SplashInput::none()), ScreenAction::Continue);
        }
        assert_eq!(s.phase, SplashPhase::Done);
        assert_eq!(
            s.update_frame(SplashInput::none()),
            ScreenAction::Transition(GameScreen::LanguageSelect)
        );
    }

    /// Literal asm wait trace: mutate registers/OAM, then observe each frame
    /// of DelayFrames/CheckForUserInterruption. No production phase math.
    #[test]
    fn every_wait_frame_matches_original_instruction_order() {
        let mut s = GameFreakSplashState::new();
        let mut check = |phase, logo, big, small: &[(i32, i32)], blink| {
            assert_eq!(s.phase, phase);
            assert_eq!(s.logo_obp0(), logo);
            assert_eq!(s.big_star_oam(), big);
            assert_eq!(s.small_stars_oam(), small);
            assert_eq!(s.small_star_blink(), blink);
            assert_eq!(s.update_frame(SplashInput::none()), ScreenAction::Continue);
        };
        for _ in 0..180 {
            check(SplashPhase::BlackDelay, 0xf9, None, &[], false);
        }
        for _ in 0..64 {
            check(SplashPhase::Setup, 0xf9, None, &[], false);
        }
        let (mut x, mut y) = (160, 0);
        for _ in 0..40 {
            x -= 4;
            y += 4;
            check(SplashPhase::BigStar, 0xf9, Some((x, y)), &[], false);
        }
        // rrc twice precedes each ten-frame wait; there is no restore.
        for palette in [0x7e, 0x9f, 0xe7] {
            for _ in 0..10 {
                check(SplashPhase::LogoFlash, palette, None, &[], false);
            }
        }
        let mut stars = Vec::new();
        let mut obp1 = 0xa4;
        let waves: [&[i32]; 6] = [
            &[0x30, 0x40, 0x58, 0x78],
            &[0x38, 0x48, 0x60, 0x70],
            &[0x34, 0x4c, 0x54, 0x64],
            &[0x3c, 0x5c, 0x6c, 0x74],
            &[],
            &[],
        ];
        for wave in waves {
            for &x in wave {
                stars.push((x, 0x68));
            }
            for _ in 0..8 {
                for (_, y) in &mut stars {
                    *y += 1;
                }
                obp1 ^= 0xa0;
                for _ in 0..3 {
                    check(SplashPhase::SmallStars, 0xe7, None, &stars, obp1 == 0x04);
                }
            }
        }
        // The last wave has fallen entirely behind the bottom black bar.
        assert!(stars.iter().all(|&(_, y)| y >= 128));
        assert_eq!(obp1, 0xa4);
        for _ in 0..40 {
            check(SplashPhase::PostDelay, 0xe7, None, &[], false);
        }
        assert_eq!(s.phase, SplashPhase::Done);
        assert_eq!(s.logo_obp0(), 0xe7);
    }

    #[test]
    fn interruption_keeps_the_current_logo_palette_and_reset_restores_it() {
        let mut s = GameFreakSplashState::new();
        s.enter(SplashPhase::LogoFlash);
        for _ in 0..10 {
            s.update_frame(SplashInput::none());
        }
        assert_eq!(s.logo_obp0(), 0x9f);
        s.update_frame(press_a());
        assert_eq!(s.phase, SplashPhase::Done);
        assert_eq!(s.logo_obp0(), 0x9f);
        s.reset();
        assert_eq!(s.logo_obp0(), 0xf9);
    }

    /// A/Start (or Up+Select+B) during the animation aborts it and skips the
    /// 40-frame post delay (`ret c` / `jr c, .next`, intro.asm:329-331).
    #[test]
    fn skip_during_animation_goes_straight_to_done() {
        for input in [
            press_a(),
            SplashInput {
                start: true,
                ..SplashInput::none()
            },
            SplashInput {
                up: true,
                select: true,
                b: true,
                ..SplashInput::none()
            },
        ] {
            let mut s = GameFreakSplashState::new();
            s.enter(SplashPhase::BigStar);
            s.update_frame(input);
            assert_eq!(s.phase, SplashPhase::Done, "input {input:?}");
            assert_eq!(
                s.update_frame(SplashInput::none()),
                ScreenAction::Transition(GameScreen::LanguageSelect),
                "PostDelay skipped on interruption"
            );
        }
        // Also during LogoFlash and SmallStars.
        let mut s = GameFreakSplashState::new();
        s.enter(SplashPhase::LogoFlash);
        s.update_frame(press_a());
        assert_eq!(s.phase, SplashPhase::Done);
        let mut s = GameFreakSplashState::new();
        s.enter(SplashPhase::SmallStars);
        s.update_frame(press_a());
        assert_eq!(s.phase, SplashPhase::Done);
    }
}
