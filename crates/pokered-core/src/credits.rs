//! End-credits sequence — `Credits` / `DisplayCreditsMon`
//! (engine/movie/credits.asm:184-273), started by `HallOfFamePC`
//! (credits.asm:1-41) after the Hall of Fame roll call.
//!
//! The original streams `CreditsOrder` (data/credits/credits_order.asm): each
//! "screen" is a set of text lines placed at hlcoord 9,6 (each string carries
//! a signed x offset, data/credits/credits_text.asm) followed by a command:
//!
//! - `CRED_TEXT_FADE_MON` — palette fade-in (`FadeInCredits`, 4 steps × 5
//!   frames), 90-frame hold, then the next mon from `CreditsMons`
//!   (data/credits/credits_mons.asm) scrolls left across the screen as a
//!   black silhouette (7 + 20 tile scrolls ≈ 54 frames).
//! - `CRED_TEXT_MON` — same without the fade, 110-frame hold.
//! - `CRED_TEXT_FADE` — fade + 120-frame hold, no mon.
//! - `CRED_TEXT` — 140-frame hold, no mon.
//!
//! `CRED_COPYRIGHT` shows the copyright screen, `CRED_THE_END` shows
//! "THE END" (16-frame delay, then fade-in). After the credits the original
//! waits 600 frames and a button press before `jp Init`
//! (scripts/HallOfFame.asm:50-56).
//!
//! Scoped deviations from the original:
//! - The copyright screen shows no scrolling mon. The original's 16th
//!   `DisplayCreditsMon` reads one byte past `CreditsMons` (a stray opcode
//!   byte) — an original bug we don't reproduce.
//! - The mon silhouette's per-scanline scroll (`ScrollCreditsMonLeft`) is
//!   modelled as 27 discrete 8 px steps (`mon_scroll_step`); the renderer
//!   applies them to the middle band only, like the original's SCX writes on
//!   scanlines 32-111.
//! - The credits are not skippable, matching the original (no
//!   `CheckForUserInterruption` in `Credits`).

use pokered_data::species::Species;
use pokered_data::wild_data::GameVersion;

/// Hold frames per screen command (credits.asm:237-253).
pub const HOLD_FADE_MON: u16 = 90;
pub const HOLD_MON: u16 = 110;
pub const HOLD_FADE: u16 = 120;
pub const HOLD_TEXT: u16 = 140;
/// `FadeInCredits`: 4 palette steps × 5 frames (credits.asm:43-54).
pub const FADE_IN_FRAMES: u16 = 20;
/// Mon silhouette scroll: 7 + 20 tile scrolls ≈ 2 frames each
/// (`DisplayCreditsMon`, credits.asm:56-133).
pub const MON_SCROLL_FRAMES: u16 = 54;
/// `CRED_THE_END`: 16-frame delay before "THE END" appears (credits.asm:255-263).
pub const THE_END_DELAY_FRAMES: u16 = 16;
/// Post-credits delay before the button wait (scripts/HallOfFame.asm:50-54:
/// 5 × 120 frames).
pub const POST_END_FRAMES: u16 = 600;

/// One text line of a credits screen. `x_off` is the signed offset from
/// hlcoord 9,6 carried by every `CreditsTextPointers` string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreditsLine {
    pub text: &'static str,
    pub x_off: i8,
}

/// What closes a screen (the `CreditsOrder` command).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditsScreenKind {
    /// CRED_TEXT — plain, 140-frame hold.
    Text,
    /// CRED_TEXT_FADE — fade-in, 120-frame hold.
    TextFade,
    /// CRED_TEXT_MON — 110-frame hold, then a mon scrolls by.
    TextMon(Species),
    /// CRED_TEXT_FADE_MON — fade-in, 90-frame hold, then a mon scrolls by.
    /// `None` only for the copyright screen (see module docs).
    TextFadeMon(Option<Species>),
}

/// One screen of the credits roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreditsScreen {
    pub lines: &'static [CreditsLine],
    pub kind: CreditsScreenKind,
}

impl CreditsScreen {
    /// Frames the text is held before the mon scroll / next screen.
    pub fn hold_frames(&self) -> u16 {
        match self.kind {
            CreditsScreenKind::Text => HOLD_TEXT,
            CreditsScreenKind::TextFade => HOLD_FADE,
            CreditsScreenKind::TextMon(_) => HOLD_MON,
            CreditsScreenKind::TextFadeMon(_) => HOLD_FADE_MON,
        }
    }

    /// The mon scrolling by after the hold, if any.
    pub fn mon(&self) -> Option<Species> {
        match self.kind {
            CreditsScreenKind::TextMon(sp) => Some(sp),
            CreditsScreenKind::TextFadeMon(sp) => sp,
            _ => None,
        }
    }

    /// True for the fade-in commands (CRED_TEXT_FADE{,_MON}).
    pub fn fades_in(&self) -> bool {
        matches!(
            self.kind,
            CreditsScreenKind::TextFade | CreditsScreenKind::TextFadeMon(_)
        )
    }
}

macro_rules! line {
    ($off:expr, $text:expr) => {
        CreditsLine {
            text: $text,
            x_off: $off,
        }
    };
}

/// The full credits roll, in `CreditsOrder` order (credits_order.asm:1-39).
/// The first screen's version line is version-dependent ("RED VERSION STAFF"
/// / "BLUE VERSION STAFF", credits_text.asm CredVersion).
pub fn credits_screens(version: GameVersion) -> Vec<CreditsScreen> {
    let version_line: &[CreditsLine] = match version {
        GameVersion::Blue => &[
            line!(-3, "#MON"),
            line!(-8, "BLUE VERSION STAFF"),
        ],
        _ => &[
            line!(-3, "#MON"),
            line!(-8, "RED VERSION STAFF"),
        ],
    };
    vec![
        CreditsScreen { lines: version_line, kind: CreditsScreenKind::TextFadeMon(Some(Species::Venusaur)) },
        CreditsScreen { lines: &[line!(-3, "DIRECTOR"), line!(-6, "SATOSHI TAJIRI")], kind: CreditsScreenKind::TextFadeMon(Some(Species::Arbok)) },
        CreditsScreen { lines: &[line!(-5, "PROGRAMMERS"), line!(-6, "TAKENORI OOTA"), line!(-7, "SHIGEKI MORIMOTO")], kind: CreditsScreenKind::TextFade },
        CreditsScreen { lines: &[line!(-5, "PROGRAMMERS"), line!(-7, "TETSUYA WATANABE"), line!(-6, "JUNICHI MASUDA"), line!(-6, "SOUSUKE TAMADA")], kind: CreditsScreenKind::TextMon(Species::Rhyhorn) },
        CreditsScreen { lines: &[line!(-7, "CHARACTER DESIGN"), line!(-5, "KEN SUGIMORI"), line!(-6, "ATSUKO NISHIDA")], kind: CreditsScreenKind::TextFadeMon(Some(Species::Fearow)) },
        CreditsScreen { lines: &[line!(-2, "MUSIC"), line!(-6, "JUNICHI MASUDA")], kind: CreditsScreenKind::TextFade },
        CreditsScreen { lines: &[line!(-6, "SOUND EFFECTS"), line!(-6, "JUNICHI MASUDA")], kind: CreditsScreenKind::TextMon(Species::Abra) },
        CreditsScreen { lines: &[line!(-5, "GAME DESIGN"), line!(-6, "SATOSHI TAJIRI")], kind: CreditsScreenKind::TextFadeMon(Some(Species::Graveler)) },
        CreditsScreen { lines: &[line!(-6, "MONSTER DESIGN"), line!(-5, "KEN SUGIMORI"), line!(-6, "ATSUKO NISHIDA"), line!(-7, "MOTOFUMI FUZIWARA")], kind: CreditsScreenKind::TextFade },
        CreditsScreen { lines: &[line!(-6, "MONSTER DESIGN"), line!(-7, "SHIGEKI MORIMOTO"), line!(-5, "SATOSHI OOTA"), line!(-6, "RENA YOSHIKAWA")], kind: CreditsScreenKind::TextMon(Species::Hitmonlee) },
        CreditsScreen { lines: &[line!(-6, "GAME SCENARIO"), line!(-6, "SATOSHI TAJIRI")], kind: CreditsScreenKind::TextFadeMon(Some(Species::Tangela)) },
        CreditsScreen { lines: &[line!(-6, "GAME SCENARIO"), line!(-8, "RYOHSUKE TANIGUCHI"), line!(-8, "FUMIHIRO NONOMURA"), line!(-7, "HIROYUKI ZINNAI")], kind: CreditsScreenKind::TextMon(Species::Starmie) },
        CreditsScreen { lines: &[line!(-8, "PARAMETRIC DESIGN"), line!(-5, "KOHJI NISINO"), line!(-6, "TAKEO NAKAMURA")], kind: CreditsScreenKind::TextFadeMon(Some(Species::Gyarados)) },
        CreditsScreen { lines: &[line!(-4, "MAP DESIGN"), line!(-6, "SATOSHI TAJIRI"), line!(-5, "KOHJI NISINO")], kind: CreditsScreenKind::TextFade },
        CreditsScreen { lines: &[line!(-4, "MAP DESIGN"), line!(-7, "KENJI MATSUSIMA"), line!(-8, "FUMIHIRO NONOMURA"), line!(-8, "RYOHSUKE TANIGUCHI")], kind: CreditsScreenKind::TextMon(Species::Ditto) },
        CreditsScreen { lines: &[line!(-7, "PRODUCT TESTING"), line!(-6, "AKIYOSHI KAKEI"), line!(-7, "KAZUKI TSUCHIYA")], kind: CreditsScreenKind::TextFade },
        CreditsScreen { lines: &[line!(-7, "PRODUCT TESTING"), line!(-6, "TAKEO NAKAMURA"), line!(-6, "MASAMITSU YUDA")], kind: CreditsScreenKind::TextMon(Species::Omastar) },
        CreditsScreen { lines: &[line!(-6, "SPECIAL THANKS"), line!(-7, "TATSUYA HISHIDA"), line!(-6, "YASUHIRO SAKAI")], kind: CreditsScreenKind::TextFade },
        CreditsScreen { lines: &[line!(-6, "SPECIAL THANKS"), line!(-7, "WATARU YAMAGUCHI"), line!(-8, "KAZUYUKI YAMAMOTO")], kind: CreditsScreenKind::Text },
        CreditsScreen { lines: &[line!(-6, "SPECIAL THANKS"), line!(-7, "AKIHITO TOMISAWA"), line!(-7, "HIROSHI KAWAMOTO"), line!(-6, "TOMOMICHI OOTA")], kind: CreditsScreenKind::TextMon(Species::Vileplume) },
        CreditsScreen { lines: &[line!(-4, "PRODUCERS"), line!(-7, "SHIGERU MIYAMOTO")], kind: CreditsScreenKind::TextFade },
        CreditsScreen { lines: &[line!(-4, "PRODUCERS"), line!(-8, "TAKASHI KAWAGUCHI")], kind: CreditsScreenKind::Text },
        CreditsScreen { lines: &[line!(-4, "PRODUCERS"), line!(-8, "TSUNEKAZU ISHIHARA")], kind: CreditsScreenKind::TextMon(Species::Nidoking) },
        CreditsScreen { lines: &[line!(-7, "US VERSION STAFF")], kind: CreditsScreenKind::TextFade },
        CreditsScreen { lines: &[line!(-7, "US COORDINATION"), line!(-5, "GAIL TILDEN")], kind: CreditsScreenKind::TextFade },
        CreditsScreen { lines: &[line!(-7, "US COORDINATION"), line!(-6, "NAOKO KAWAKAMI"), line!(-6, "HIRO NAKAMURA")], kind: CreditsScreenKind::Text },
        CreditsScreen { lines: &[line!(-7, "US COORDINATION"), line!(-6, "WILLIAM GIESE"), line!(-5, "SARA OSBORNE")], kind: CreditsScreenKind::Text },
        CreditsScreen { lines: &[line!(-7, "TEXT TRANSLATION"), line!(-6, "NOB OGASAWARA")], kind: CreditsScreenKind::TextFade },
        CreditsScreen { lines: &[line!(-5, "PROGRAMMERS"), line!(-7, "TERUKI MURAKAWA"), line!(-7, "KOHTA FUKUI")], kind: CreditsScreenKind::TextFade },
        CreditsScreen { lines: &[line!(-6, "SPECIAL THANKS"), line!(-5, "SATORU IWATA")], kind: CreditsScreenKind::TextFade },
        CreditsScreen { lines: &[line!(-6, "SPECIAL THANKS"), line!(-7, "TAKAHIRO HARADA")], kind: CreditsScreenKind::Text },
        CreditsScreen { lines: &[line!(-7, "PRODUCT TESTING"), line!(-5, "PAAD TESTING"), line!(-9, "NCL SUPER MARIO CLUB")], kind: CreditsScreenKind::TextFade },
        CreditsScreen { lines: &[line!(-4, "PRODUCER"), line!(-7, "TAKEHIRO IZUSHI")], kind: CreditsScreenKind::TextFade },
        CreditsScreen { lines: &[line!(-8, "EXECUTIVE PRODUCER"), line!(-7, "HIROSHI YAMAUCHI")], kind: CreditsScreenKind::TextFadeMon(Some(Species::Parasect)) },
        // CRED_COPYRIGHT — no mon (see module docs for the deviation).
        CreditsScreen { lines: &[line!(-7, "©1995-98 NINTENDO"), line!(-7, "©1995-98 CREATURES inc."), line!(-7, "©1995-98 GAME FREAK inc.")], kind: CreditsScreenKind::TextFadeMon(None) },
    ]
}

/// Phases within the roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditsPhase {
    /// Text of the current screen is held (with a fade-in ramp on fade
    /// screens).
    Hold,
    /// The screen's mon silhouette scrolls left.
    MonScroll,
    /// "THE END" — 16-frame delay, text, 600-frame dwell, then a button
    /// press finishes (scripts/HallOfFame.asm:50-56).
    TheEnd,
    /// Roll complete — the frontend resets to the title screen.
    Done,
}

/// Input forwarded to the credits each frame (only used at "THE END").
#[derive(Debug, Clone, Copy, Default)]
pub struct CreditsInput {
    pub a: bool,
    pub b: bool,
}

impl CreditsInput {
    pub fn none() -> Self {
        Self::default()
    }
}

/// Logical state of the credits roll. Rendering reads `current_screen()`,
/// `phase()`, `fade_step()`, `mon_scroll_progress()` and `the_end_visible()`.
#[derive(Debug, Clone)]
pub struct CreditsState {
    screens: Vec<CreditsScreen>,
    screen_idx: usize,
    phase: CreditsPhase,
    frame: u16,
}

impl CreditsState {
    pub fn new(version: GameVersion) -> Self {
        Self {
            screens: credits_screens(version),
            screen_idx: 0,
            phase: CreditsPhase::Hold,
            frame: 0,
        }
    }

    pub fn phase(&self) -> CreditsPhase {
        self.phase
    }

    pub fn screen_index(&self) -> usize {
        self.screen_idx
    }

    /// The screen being held/scrolled (`None` once "THE END" is reached).
    pub fn current_screen(&self) -> Option<&CreditsScreen> {
        if self.phase == CreditsPhase::TheEnd || self.phase == CreditsPhase::Done {
            return None;
        }
        self.screens.get(self.screen_idx)
    }

    /// Fade-in ramp for fade screens: 0..=4 palette steps
    /// (`HoFGBPalettes`, credits.asm:135-140); 4 (fully visible) afterwards
    /// and on non-fade screens.
    pub fn fade_step(&self) -> u8 {
        match self.current_screen() {
            Some(screen) if self.phase == CreditsPhase::Hold && screen.fades_in() => {
                ((self.frame / 5) as u8).min(4)
            }
            _ => 4,
        }
    }

    /// 0.0..=1.0 progress of the mon silhouette scroll.
    pub fn mon_scroll_progress(&self) -> f32 {
        if self.phase == CreditsPhase::MonScroll {
            (self.frame as f32 / MON_SCROLL_FRAMES as f32).min(1.0)
        } else {
            0.0
        }
    }

    /// The mon-scroll step 0..=27 — one 8 px tile-scroll of the middle band
    /// (`ScrollCreditsMonLeft` runs 7 + 20 times, credits.asm:89-114).
    /// The renderer shifts the band by `step * 8` px; from step 7 on the
    /// white window sweep covers everything right of `216 - step * 8` px
    /// (the mon's right edge, credits.asm:107-114).
    pub fn mon_scroll_step(&self) -> u8 {
        if self.phase == CreditsPhase::MonScroll {
            ((self.frame / 2) as u8).min(27)
        } else {
            0
        }
    }

    /// "THE END" is visible (after the 16-frame delay, credits.asm:255).
    pub fn the_end_visible(&self) -> bool {
        self.phase == CreditsPhase::TheEnd && self.frame >= THE_END_DELAY_FRAMES
    }

    /// True once the 600-frame dwell has elapsed and a button dismisses the
    /// roll (the original's `WaitForTextScrollButtonPress`).
    pub fn awaiting_final_button(&self) -> bool {
        self.phase == CreditsPhase::TheEnd && self.frame >= THE_END_DELAY_FRAMES + POST_END_FRAMES
    }

    /// Advance one frame. Returns `true` on the frame the roll completes.
    pub fn update_frame(&mut self, input: CreditsInput) -> bool {
        self.frame += 1;
        match self.phase {
            CreditsPhase::Hold => {
                let screen = self.screens[self.screen_idx];
                if self.frame >= screen.hold_frames() {
                    self.frame = 0;
                    if screen.mon().is_some() {
                        self.phase = CreditsPhase::MonScroll;
                    } else {
                        self.advance_screen();
                    }
                }
            }
            CreditsPhase::MonScroll => {
                if self.frame >= MON_SCROLL_FRAMES {
                    self.frame = 0;
                    self.advance_screen();
                }
            }
            CreditsPhase::TheEnd => {
                if self.awaiting_final_button() && (input.a || input.b) {
                    self.phase = CreditsPhase::Done;
                    return true;
                }
            }
            CreditsPhase::Done => return true,
        }
        false
    }

    fn advance_screen(&mut self) {
        self.screen_idx += 1;
        self.phase = if self.screen_idx >= self.screens.len() {
            CreditsPhase::TheEnd
        } else {
            CreditsPhase::Hold
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The screen sequence reproduces `CreditsOrder` (credits_order.asm):
    /// 35 screens, 15 of them with mons matching `CreditsMons` in order.
    #[test]
    fn screen_order_matches_asm() {
        let screens = credits_screens(GameVersion::Red);
        assert_eq!(screens.len(), 35);
        let mons: Vec<Species> = screens.iter().filter_map(|s| s.mon()).collect();
        assert_eq!(
            mons,
            vec![
                Species::Venusaur,
                Species::Arbok,
                Species::Rhyhorn,
                Species::Fearow,
                Species::Abra,
                Species::Graveler,
                Species::Hitmonlee,
                Species::Tangela,
                Species::Starmie,
                Species::Gyarados,
                Species::Ditto,
                Species::Omastar,
                Species::Vileplume,
                Species::Nidoking,
                Species::Parasect,
            ],
            "data/credits/credits_mons.asm order"
        );
        assert_eq!(screens[0].lines[1].text, "RED VERSION STAFF");
        assert_eq!(screens[0].lines[0].text, "#MON");
        assert_eq!(screens[1].lines[0].text, "DIRECTOR");
        assert_eq!(screens[1].lines[1].text, "SATOSHI TAJIRI");
        assert!(screens[1].fades_in());
        assert!(!screens[3].fades_in());
        // Hold durations from credits.asm:237-253.
        assert_eq!(screens[0].hold_frames(), 90);
        assert_eq!(screens[2].hold_frames(), 120);
        assert_eq!(screens[3].hold_frames(), 110);
        assert_eq!(screens[18].hold_frames(), 140);
    }

    #[test]
    fn blue_version_string() {
        let screens = credits_screens(GameVersion::Blue);
        assert_eq!(screens[0].lines[1].text, "BLUE VERSION STAFF");
    }

    /// Per-screen flow: hold → mon scroll → next; copyright ends in THE END.
    #[test]
    fn flow_through_screens_to_the_end() {
        let mut s = CreditsState::new(GameVersion::Red);
        // Screen 0: fade-in ramp, then hold, then mon scroll.
        assert_eq!(s.phase(), CreditsPhase::Hold);
        assert_eq!(s.fade_step(), 0);
        for _ in 0..5 {
            s.update_frame(CreditsInput::none());
        }
        assert_eq!(s.fade_step(), 1);
        for _ in 0..(HOLD_FADE_MON - 5) {
            s.update_frame(CreditsInput::none());
        }
        assert_eq!(s.phase(), CreditsPhase::MonScroll);
        assert_eq!(s.fade_step(), 4);
        for _ in 0..MON_SCROLL_FRAMES {
            s.update_frame(CreditsInput::none());
        }
        assert_eq!(s.phase(), CreditsPhase::Hold);
        assert_eq!(s.screen_index(), 1);

        // Run the whole roll to THE END.
        let mut guard = 0;
        while s.phase() != CreditsPhase::TheEnd {
            s.update_frame(CreditsInput::none());
            guard += 1;
            assert!(guard < 100_000, "no infinite loop");
        }
        assert_eq!(s.screen_index(), 35);
        assert!(!s.the_end_visible(), "16-frame delay first");
        for _ in 0..THE_END_DELAY_FRAMES {
            s.update_frame(CreditsInput::none());
        }
        assert!(s.the_end_visible());
        assert!(!s.awaiting_final_button());
        // Buttons during the 600-frame dwell do nothing (not skippable).
        s.update_frame(CreditsInput { a: true, b: false });
        assert_eq!(s.phase(), CreditsPhase::TheEnd);
        for _ in 0..POST_END_FRAMES {
            s.update_frame(CreditsInput::none());
        }
        assert!(s.awaiting_final_button());
        assert!(s.update_frame(CreditsInput { a: true, b: false }));
        assert_eq!(s.phase(), CreditsPhase::Done);
    }
}
