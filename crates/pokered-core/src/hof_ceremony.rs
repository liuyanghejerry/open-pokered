//! Hall of Fame roll-call movie — the post-Champion ceremony from
//! `AnimateHallOfFame` (engine/movie/hall_of_fame.asm:35-129), driven by
//! `HallOfFameResetEventsAndSaveScript` (scripts/HallOfFame.asm:22-56).
//!
//! Sequence (all timings in 60 fps frames, from the asm):
//! 1. [`HofPhase::FadeOut`] — `HoFFadeOutScreenAndMusic`
//!    (hall_of_fame.asm:284-288): music fades, screen whites out.
//! 2. [`HofPhase::Opening`] — screen cleared, 100-frame delay
//!    (`ld c, 100`, hall_of_fame.asm:39-40), then `MUSIC_HALL_OF_FAME` starts
//!    (hall_of_fame.asm:73-76).
//! 3. For each party mon (hall_of_fame.asm:77-110):
//!    - [`HofPhase::MonScroll`] — `HoFShowMonOrPlayer` scrolls the BACK pic
//!      in from the right along the bottom band (hSCX 192→160, 56 frames at
//!      4 px, hSCY=$d0) then the FRONT pic in from the left to its resting
//!      spot (hSCX 160→0, 40 frames at 4 px) — modelled as 96 frames.
//!    - [`HofPhase::MonInfo`] — `HoFDisplayAndRecordMonInfo`: nickname, level
//!      and types box + the mon's cry, then 80 frames (`ld c, 80`).
//!    - [`HofPhase::MonText`] — the "HALL OF FAME" text box, 180 frames
//!      (`ld c, 180`).
//!    - [`HofPhase::MonFade`] — `GBFadeOutToWhite`.
//! 4. [`HofPhase::PlayerScroll`] / [`HofPhase::PlayerStats`] — the player is
//!    shown (hall_of_fame.asm:117-123), then `HoFDisplayPlayerStats`
//!    (hall_of_fame.asm:203-228): name, PLAY TIME, MONEY, the #DEX
//!    seen/owned text (120 frames) and the rating text (120 frames).
//! 5. [`HofPhase::FinalFade`] — `HoFFadeOutScreenAndMusic` again; the
//!    frontend then starts the credits (engine/movie/credits.asm).
//!
//! The team is recorded into SRAM by the *caller* when the ceremony starts
//! (the original interleaves recording into the display loop —
//! `HoFRecordMonInfo`, hall_of_fame.asm:230-241 — but the net effect is one
//! `SaveHallOfFameTeams` after the last mon).

use pokered_data::species::Species;

/// Frames for the initial/final `GBFadeOutToWhite` (approximation of the
/// palette sweep).
pub const FADE_FRAMES: u16 = 16;
/// Post-clear delay before the music starts (`ld c, 100`, hall_of_fame.asm:39).
pub const OPENING_FRAMES: u16 = 100;
/// Back-pic scroll: hSCX 192 → 160 at +4 px/frame, 56 frames
/// (`HoFShowMonOrPlayer`'s `.ScrollPic` loop with d=$a0 / e=4,
/// hall_of_fame.asm:127-134). The pic wraps through the map, so it is
/// visible crossing the screen for the first ~40 of these frames.
pub const SCROLL_BACK_FRAMES: u16 = 56;
/// Front-pic scroll: hSCX 160 → 0 at −4 px/frame, 40 frames (d=0 / e=−4,
/// hall_of_fame.asm:135-140); the pic is visible for the last ~25 of these.
pub const SCROLL_FRONT_FRAMES: u16 = 40;
/// Total pic-scroll time (back + front) per mon / for the player.
pub const SCROLL_FRAMES: u16 = SCROLL_BACK_FRAMES + SCROLL_FRONT_FRAMES;
/// Info-box dwell per mon (`ld c, 80`, hall_of_fame.asm:94).
pub const MON_INFO_FRAMES: u16 = 80;
/// "HALL OF FAME" text dwell per mon (`ld c, 180`, hall_of_fame.asm:100).
pub const MON_TEXT_FRAMES: u16 = 180;
/// Player-stats dwell: seen/owned text (120) + rating text (120) + a final
/// read beat (`HoFPrintTextAndDelay`, hall_of_fame.asm:224-228).
pub const PLAYER_STATS_FRAMES: u16 = 360;

/// One party member shown in the roll call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HofEntry {
    pub species: Species,
    pub level: u8,
    /// Decoded nickname (display-ready).
    pub nickname: String,
}

/// Player stats shown after the last mon (`HoFDisplayPlayerStats`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HofPlayerStats {
    /// Decoded player name.
    pub name: String,
    pub play_time_hours: u16,
    pub play_time_minutes: u8,
    pub money: u32,
    pub dex_seen: u16,
    pub dex_owned: u16,
    /// #DEX rating text (same table as PROF.OAK's PC rating).
    pub rating: &'static str,
}

/// Animation phases of the roll call, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HofPhase {
    /// Music + screen fade out (entering the ceremony).
    FadeOut,
    /// Cleared screen, pre-music delay.
    Opening,
    /// Mon pics scrolling in.
    MonScroll,
    /// Nickname / LEVEL / TYPE1 / TYPE2 box + cry.
    MonInfo,
    /// "HALL OF FAME" text box.
    MonText,
    /// Fade to white between mons.
    MonFade,
    /// Player pic scrolling in.
    PlayerScroll,
    /// Player name / PLAY TIME / MONEY / #DEX seen-owned + rating.
    PlayerStats,
    /// Final music + screen fade (into the credits).
    FinalFade,
    /// Ceremony over — the frontend rolls the credits.
    Done,
}

/// One-shot sound requests for the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HofSfx {
    /// `PlayCry` for the mon being displayed (hall_of_fame.asm:200).
    Cry(Species),
}

/// Which pic is scrolling during `MonScroll` / `PlayerScroll`
/// (`HoFShowMonOrPlayer`, hall_of_fame.asm:97-157): the BACK pic scrolls
/// first (hSCX 192 → 160, +4 px/frame, with hSCY=$d0 so it runs along the
/// bottom band), then the FRONT pic (hSCX 160 → 0, −4 px/frame, hSCY=0,
/// sliding in from the left edge to its resting spot).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HofScrollStage {
    Back,
    Front,
}

/// Logical state of the roll call. Rendering reads `phase()`, `mon_index()`,
/// `current_entry()`, `scroll_progress()` and `stats()`.
#[derive(Debug, Clone)]
pub struct HofCeremonyState {
    entries: Vec<HofEntry>,
    stats: HofPlayerStats,
    phase: HofPhase,
    /// Frames elapsed in the current phase.
    frame: u16,
    /// Party index being shown (Mon* phases).
    mon_index: usize,
    pending_sfx: Vec<HofSfx>,
    /// Set when [`HofPhase::Opening`] ends — the frontend starts
    /// `MUSIC_HALL_OF_FAME` (hall_of_fame.asm:73-76).
    music_pending: bool,
    /// Set when [`HofPhase::FinalFade`] is entered — the frontend fades the
    /// music out (`HoFFadeOutScreenAndMusic`, hall_of_fame.asm:284-288).
    music_fade_pending: bool,
}

impl HofCeremonyState {
    pub fn new(entries: Vec<HofEntry>, stats: HofPlayerStats) -> Self {
        Self {
            entries,
            stats,
            phase: HofPhase::FadeOut,
            frame: 0,
            mon_index: 0,
            pending_sfx: Vec::new(),
            music_pending: false,
            music_fade_pending: false,
        }
    }

    pub fn phase(&self) -> HofPhase {
        self.phase
    }

    /// Frames elapsed in the current phase.
    pub fn phase_frame(&self) -> u16 {
        self.frame
    }

    /// Index of the mon being shown (valid in the `Mon*` phases).
    pub fn mon_index(&self) -> usize {
        self.mon_index
    }

    /// The mon currently on screen (`Mon*` phases only).
    pub fn current_entry(&self) -> Option<&HofEntry> {
        if matches!(
            self.phase,
            HofPhase::MonScroll | HofPhase::MonInfo | HofPhase::MonText | HofPhase::MonFade
        ) {
            self.entries.get(self.mon_index)
        } else {
            None
        }
    }

    pub fn stats(&self) -> &HofPlayerStats {
        &self.stats
    }

    /// 0.0..=1.0 progress of the pic scroll (`MonScroll` / `PlayerScroll`).
    pub fn scroll_progress(&self) -> f32 {
        if matches!(self.phase, HofPhase::MonScroll | HofPhase::PlayerScroll) {
            (self.frame as f32 / SCROLL_FRAMES as f32).min(1.0)
        } else {
            0.0
        }
    }

    /// Which pic is scrolling in `MonScroll` / `PlayerScroll`, or `None` in
    /// other phases.
    pub fn scroll_stage(&self) -> Option<HofScrollStage> {
        match self.phase {
            HofPhase::MonScroll | HofPhase::PlayerScroll if self.frame <= SCROLL_BACK_FRAMES => {
                Some(HofScrollStage::Back)
            }
            HofPhase::MonScroll | HofPhase::PlayerScroll => Some(HofScrollStage::Front),
            _ => None,
        }
    }

    /// Screen x of the scrolling pic (0..160 = on screen; ≥160 = off the
    /// right edge — the pics enter from the edges and never go negative).
    ///
    /// Back stage: hSCX = (192 + 4·frame) mod 256, pic at (96 − hSCX) mod 256
    /// (map column 12, hlcoord 12,5). Front stage: hSCX = 160 − 4·(frame−56),
    /// pic at (96 − hSCX) mod 256.
    pub fn scroll_pic_x(&self) -> u32 {
        match self.scroll_stage() {
            Some(HofScrollStage::Back) => {
                (96u32.wrapping_sub((192 + 4 * self.frame as u32) & 0xFF)) & 0xFF
            }
            Some(HofScrollStage::Front) => {
                let k = (self.frame - SCROLL_BACK_FRAMES) as u32;
                (96u32.wrapping_sub(160u32.wrapping_sub(4 * k))) & 0xFF
            }
            None => 96,
        }
    }

    /// Drain one-shot SFX requests (mon cries).
    pub fn take_sfx(&mut self) -> Vec<HofSfx> {
        std::mem::take(&mut self.pending_sfx)
    }

    /// Take the pending `MUSIC_HALL_OF_FAME` request (fires once, at the end
    /// of [`HofPhase::Opening`]).
    pub fn take_music_pending(&mut self) -> bool {
        std::mem::take(&mut self.music_pending)
    }

    /// Take the pending music-fade request (fires once, at the start of
    /// [`HofPhase::FinalFade`]).
    pub fn take_music_fade_pending(&mut self) -> bool {
        std::mem::take(&mut self.music_fade_pending)
    }

    fn enter(&mut self, phase: HofPhase) {
        self.phase = phase;
        self.frame = 0;
        match phase {
            HofPhase::MonInfo => {
                if let Some(entry) = self.entries.get(self.mon_index) {
                    self.pending_sfx.push(HofSfx::Cry(entry.species));
                }
            }
            HofPhase::MonScroll if self.mon_index == 0 => {
                self.music_pending = true;
            }
            HofPhase::FinalFade => {
                self.music_fade_pending = true;
            }
            _ => {}
        }
    }

    /// Advance one frame. Returns `true` on the frame the ceremony completes
    /// ([`HofPhase::Done`] is entered); the frontend should then start the
    /// credits.
    pub fn update_frame(&mut self) -> bool {
        self.frame += 1;
        let limit = match self.phase {
            HofPhase::FadeOut | HofPhase::MonFade | HofPhase::FinalFade => FADE_FRAMES,
            HofPhase::Opening => OPENING_FRAMES,
            HofPhase::MonScroll | HofPhase::PlayerScroll => SCROLL_FRAMES,
            HofPhase::MonInfo => MON_INFO_FRAMES,
            HofPhase::MonText => MON_TEXT_FRAMES,
            HofPhase::PlayerStats => PLAYER_STATS_FRAMES,
            HofPhase::Done => return true,
        };
        if self.frame < limit {
            return false;
        }
        match self.phase {
            HofPhase::FadeOut => self.enter(HofPhase::Opening),
            HofPhase::Opening => {
                if self.entries.is_empty() {
                    self.enter(HofPhase::PlayerScroll);
                } else {
                    self.enter(HofPhase::MonScroll);
                }
            }
            HofPhase::MonScroll => self.enter(HofPhase::MonInfo),
            HofPhase::MonInfo => self.enter(HofPhase::MonText),
            HofPhase::MonText => self.enter(HofPhase::MonFade),
            HofPhase::MonFade => {
                self.mon_index += 1;
                if self.mon_index < self.entries.len() {
                    self.enter(HofPhase::MonScroll);
                } else {
                    self.enter(HofPhase::PlayerScroll);
                }
            }
            HofPhase::PlayerScroll => self.enter(HofPhase::PlayerStats),
            HofPhase::PlayerStats => self.enter(HofPhase::FinalFade),
            HofPhase::FinalFade => self.enter(HofPhase::Done),
            HofPhase::Done => return true,
        }
        matches!(self.phase, HofPhase::Done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state(n_mons: usize) -> HofCeremonyState {
        let entries = (0..n_mons)
            .map(|i| HofEntry {
                species: Species::Pikachu,
                level: 50 + i as u8,
                nickname: format!("MON{}", i),
            })
            .collect();
        HofCeremonyState::new(
            entries,
            HofPlayerStats {
                name: "RED".into(),
                play_time_hours: 12,
                play_time_minutes: 34,
                money: 99999,
                dex_seen: 80,
                dex_owned: 60,
                rating: "Great!",
            },
        )
    }

    fn run(state: &mut HofCeremonyState, frames: u16) {
        for _ in 0..frames {
            state.update_frame();
        }
    }

    /// Phase order and per-phase timing follow the asm delays
    /// (hall_of_fame.asm:39-110).
    #[test]
    fn phase_order_and_timing() {
        let mut s = test_state(2);
        assert_eq!(s.phase(), HofPhase::FadeOut);
        run(&mut s, FADE_FRAMES);
        assert_eq!(s.phase(), HofPhase::Opening);
        run(&mut s, OPENING_FRAMES);
        assert_eq!(s.phase(), HofPhase::MonScroll);
        assert!(s.take_music_pending(), "music starts after the opening");
        assert!(!s.take_music_pending(), "one-shot");
        for mon in 0..2 {
            assert_eq!(s.mon_index(), mon);
            assert_eq!(s.current_entry().unwrap().level, 50 + mon as u8);
            run(&mut s, SCROLL_FRAMES);
            assert_eq!(s.phase(), HofPhase::MonInfo);
            let sfx = s.take_sfx();
            assert_eq!(sfx, vec![HofSfx::Cry(Species::Pikachu)], "cry per mon");
            run(&mut s, MON_INFO_FRAMES);
            assert_eq!(s.phase(), HofPhase::MonText);
            assert!(s.take_sfx().is_empty(), "one cry per mon");
            run(&mut s, MON_TEXT_FRAMES);
            assert_eq!(s.phase(), HofPhase::MonFade);
            run(&mut s, FADE_FRAMES);
        }
        assert_eq!(s.phase(), HofPhase::PlayerScroll, "after the last mon");
        assert_eq!(s.current_entry(), None);
        run(&mut s, SCROLL_FRAMES);
        assert_eq!(s.phase(), HofPhase::PlayerStats);
        run(&mut s, PLAYER_STATS_FRAMES);
        assert_eq!(s.phase(), HofPhase::FinalFade);
        assert!(s.take_music_fade_pending(), "music fades at the end");
        run(&mut s, FADE_FRAMES);
        assert_eq!(s.phase(), HofPhase::Done);
        assert!(s.update_frame(), "reports completion");
    }

    /// An empty party (degenerate) skips straight to the player.
    #[test]
    fn empty_party_skips_to_player() {
        let mut s = test_state(0);
        run(&mut s, FADE_FRAMES + OPENING_FRAMES);
        assert_eq!(s.phase(), HofPhase::PlayerScroll);
    }

    /// Scroll progress ramps 0 → 1 across the scroll phase.
    #[test]
    fn scroll_progress_ramps() {
        let mut s = test_state(1);
        run(&mut s, FADE_FRAMES + OPENING_FRAMES);
        assert_eq!(s.scroll_progress(), 0.0);
        run(&mut s, SCROLL_FRAMES / 2);
        let mid = s.scroll_progress();
        assert!(mid > 0.4 && mid < 0.6, "mid-scroll {mid}");
        run(&mut s, SCROLL_FRAMES / 2);
        assert_eq!(s.phase(), HofPhase::MonInfo);
        assert_eq!(s.scroll_progress(), 0.0, "no scroll outside scroll phases");
    }
}
