//! The FIGHT caller's PrintText / PROMPT / menu-return continuation.
//! There is one countdown: the enclosing ShowingText.wait_frames. Physical
//! input and configuration are supplied by the host; only actual Joypad
//! observations, glyph progress and the suspended call stage are saved here.
use super::*;
use crate::text::letter_delay::{poll_wait, sample_pressed};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BattleTextSettings {
    pub options_delay: u8,
    pub letter_flags_before_processor: u8,
    pub no_text_delay: bool,
}

impl BattleTextSettings {
    /// TextCommandProcessor sets BIT_TEXT_DELAY even after InitBattleCommon
    /// cleared it. The ordinary battle caller is outside the Pokedex's
    /// temporary hClearLetterPrintingDelayFlags=2 lifetime.
    pub fn effective_letter_flags(self) -> u8 {
        self.letter_flags_before_processor | 2
    }

    fn glyph_delay(self, held_ab: bool) -> u16 {
        if self.no_text_delay {
            0
        } else if held_ab || self.effective_letter_flags() & 1 == 0 {
            1
        } else {
            u16::from(self.options_delay & 0x0f)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BattleTextFrame {
    /// GB logical bits: A/B/SELECT/START/RIGHT/LEFT/UP/DOWN = bits 0..7.
    pub held_buttons: u8,
    /// Explicit entry policy. None retains the legacy public API until a
    /// frontend has supplied a verified native entry mapping. An already
    /// pending source call always resumes its saved settings.
    pub settings: Option<BattleTextSettings>,
}

impl BattleTextFrame {
    pub fn from_edges(input: BattleInput) -> Self {
        Self {
            held_buttons: u8::from(input.a)
                | (u8::from(input.b) << 1)
                | (u8::from(input.right) << 4)
                | (u8::from(input.left) << 5)
                | (u8::from(input.up) << 6)
                | (u8::from(input.down) << 7),
            settings: None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FightTextReceipt {
    /// This update executed a source selection or ManualTextScroll ACK.
    pub press_ab: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FightTextKind {
    ManualPrompt,
    LinkPrompt,
    NoMovesDone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FightTextStage {
    BoxSync,
    Printing,
    PromptSync,
    ManualScroll,
    LinkScroll,
    MenuInfoSync,
    MenuCursorSync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FightTextWork {
    pub kind: FightTextKind,
    pub stage: FightTextStage,
    pub visible_chars: u16,
    pub short_exit_scheduled: bool,
    pub last_polled_buttons: u8,
    pub settings: BattleTextSettings,
}

impl FightTextWork {
    pub fn returning_menu(self) -> bool {
        matches!(
            self.stage,
            FightTextStage::MenuInfoSync | FightTextStage::MenuCursorSync
        )
    }

    pub fn arrow_visible(self) -> bool {
        self.kind == FightTextKind::ManualPrompt
            && matches!(
                self.stage,
                FightTextStage::PromptSync | FightTextStage::ManualScroll
            )
    }
}

impl BattleScreen {
    pub fn fight_text_work(&self) -> Option<FightTextWork> {
        match &self.phase {
            BattlePhase::ShowingText {
                backdrop:
                    BattleTextBackdrop::FightMoveNotice {
                        wait: FightNoticeWait::Source(work),
                    },
                ..
            } => Some(*work),
            _ => None,
        }
    }

    /// Derived display only. The phase's messages remain the only full text.
    pub fn fight_text_visible_message(&self) -> Option<String> {
        let work = self.fight_text_work()?;
        if work.returning_menu() {
            return None;
        }
        let BattlePhase::ShowingText {
            messages, current, ..
        } = &self.phase
        else {
            return None;
        };
        let mut glyphs = 0u16;
        Some(
            messages
                .get(*current)?
                .chars()
                .take_while(|ch| {
                    if *ch == '\n' {
                        return true;
                    }
                    glyphs = glyphs.saturating_add(1);
                    glyphs <= work.visible_chars
                })
                .collect(),
        )
    }

    fn keep_fight_text(
        &mut self,
        messages: Vec<String>,
        current: usize,
        remaining: u16,
        next_phase: Box<BattlePhase>,
        work: FightTextWork,
    ) -> ScreenAction {
        self.phase = BattlePhase::ShowingText {
            messages,
            current,
            wait_frames: remaining,
            next_phase,
            backdrop: BattleTextBackdrop::FightMoveNotice {
                wait: FightNoticeWait::Source(work),
            },
        };
        ScreenAction::Continue
    }

    fn start_fight_text(
        &mut self,
        message: &str,
        kind: FightTextKind,
        settings: BattleTextSettings,
        held: u8,
    ) {
        let next = if kind == FightTextKind::NoMovesDone {
            BattlePhase::ForcedStruggle { wait_frames: 60 }
        } else {
            BattlePhase::MoveSelect
        };
        self.show_text_then(vec![message.to_string()], next);
        // The source caller supplies one authored page. Keep native layout's
        // existing multi-page continuation intact rather than dropping its
        // later pages; that layout-generated path is not a source-time claim.
        if matches!(&self.phase, BattlePhase::ShowingText { messages, .. } if messages.len() != 1) {
            if kind == FightTextKind::NoMovesDone {
                self.enter_fight_move_menu(None);
            } else {
                self.show_fight_move_notice(message);
            }
            return;
        }
        if let BattlePhase::ShowingText {
            wait_frames,
            backdrop,
            ..
        } = &mut self.phase
        {
            *wait_frames = 3;
            *backdrop = BattleTextBackdrop::FightMoveNotice {
                wait: FightNoticeWait::Source(FightTextWork {
                    kind,
                    stage: FightTextStage::BoxSync,
                    visible_chars: 0,
                    short_exit_scheduled: false,
                    last_polled_buttons: held,
                    settings,
                }),
            };
        }
    }

    pub(super) fn enter_fight_move_menu_with_text(
        &mut self,
        row: Option<usize>,
        text: BattleTextFrame,
        receipt: &mut FightTextReceipt,
    ) {
        let Some(settings) = text.settings else {
            self.enter_fight_move_menu(row);
            return;
        };
        if self
            .battle_state
            .as_ref()
            .is_some_and(|bs| !Self::any_move_to_select(&bs.player))
        {
            self.move_menu = None;
            let message = format!("{} has no\nmoves left!", self.player_display_name(false));
            self.start_fight_text(
                &message,
                FightTextKind::NoMovesDone,
                settings,
                text.held_buttons,
            );
            receipt.press_ab = true;
        } else {
            self.rebuild_fight_move_menu(row);
            self.phase = BattlePhase::MoveSelect;
        }
    }

    pub(super) fn show_fight_move_notice_with_text(
        &mut self,
        message: &str,
        text: BattleTextFrame,
        receipt: &mut FightTextReceipt,
    ) {
        let Some(settings) = text.settings else {
            self.show_fight_move_notice(message);
            return;
        };
        let kind = if self.link_mode {
            FightTextKind::LinkPrompt
        } else {
            FightTextKind::ManualPrompt
        };
        self.start_fight_text(message, kind, settings, text.held_buttons);
        receipt.press_ab = true;
    }

    /// Executes only the active continuation on this frame. Entering a later
    /// stage may fall through to its immediate source work, but never calls
    /// update_frame recursively or advances another clock.
    pub(super) fn update_fight_text(
        &mut self,
        messages: Vec<String>,
        current: usize,
        mut remaining: u16,
        next_phase: Box<BattlePhase>,
        mut work: FightTextWork,
        held: u8,
        receipt: &mut FightTextReceipt,
    ) -> ScreenAction {
        let total = messages
            .get(current)
            .map_or(0, |s| s.chars().filter(|c| *c != '\n').count())
            .min(u16::MAX as usize) as u16;
        loop {
            match work.stage {
                FightTextStage::BoxSync => {
                    remaining = remaining.saturating_sub(1);
                    if remaining != 0 {
                        break;
                    }
                    work.stage = FightTextStage::Printing;
                }
                FightTextStage::Printing => {
                    if work.settings.no_text_delay {
                        work.visible_chars = total;
                    } else {
                        // PrintLetterDelay calls Joypad, unlike either Delay3.
                        sample_pressed(&mut work.last_polled_buttons, held);
                        if !poll_wait(
                            &mut remaining,
                            &mut work.short_exit_scheduled,
                            work.visible_chars == total,
                            held & 3 != 0,
                        ) {
                            break;
                        }
                        if work.visible_chars < total {
                            let delay = work.settings.glyph_delay(held & 3 != 0);
                            if delay == 0 {
                                // A raw zero options delay has no hardware wait.
                                // The bounded string still returns through DONE/PROMPT.
                                work.visible_chars = total;
                            } else {
                                work.visible_chars += 1;
                                remaining = delay;
                                work.short_exit_scheduled = held & 3 != 0;
                                break;
                            }
                        }
                    }
                    if work.kind == FightTextKind::NoMovesDone {
                        self.presentation.finish_pages();
                        self.phase = *next_phase;
                        return ScreenAction::Continue;
                    }
                    work.stage = FightTextStage::PromptSync;
                    remaining = 3; // PromptText writes its arrow before this wait.
                    break;
                }
                FightTextStage::PromptSync => {
                    remaining = remaining.saturating_sub(1);
                    if remaining != 0 {
                        break;
                    }
                    work.stage = if work.kind == FightTextKind::LinkPrompt {
                        FightTextStage::LinkScroll
                    } else {
                        FightTextStage::ManualScroll
                    };
                    if work.stage == FightTextStage::LinkScroll {
                        remaining = 65;
                        break;
                    }
                }
                FightTextStage::ManualScroll | FightTextStage::LinkScroll => {
                    if work.stage == FightTextStage::ManualScroll {
                        if sample_pressed(&mut work.last_polled_buttons, held) & 3 == 0 {
                            break;
                        }
                        receipt.press_ab = true;
                    } else {
                        remaining = remaining.saturating_sub(1);
                        if remaining != 0 {
                            break;
                        }
                    }
                    // PrintText returned. Restore the existing caller once.
                    self.current_message = None;
                    self.presentation.finish_pages();
                    self.sync_display_from_state();
                    let row = self.move_menu.as_ref().map(|menu| menu.cursor());
                    if self
                        .battle_state
                        .as_ref()
                        .is_some_and(|bs| !Self::any_move_to_select(&bs.player))
                    {
                        let message =
                            format!("{} has no\nmoves left!", self.player_display_name(false));
                        self.move_menu = None;
                        self.start_fight_text(
                            &message,
                            FightTextKind::NoMovesDone,
                            work.settings,
                            held,
                        );
                        return ScreenAction::Continue;
                    }
                    self.rebuild_fight_move_menu(row);
                    work.stage = FightTextStage::MenuInfoSync;
                    remaining = 3; // PrintMenuItem's Delay3.
                    break;
                }
                FightTextStage::MenuInfoSync => {
                    remaining = remaining.saturating_sub(1);
                    if remaining != 0 {
                        break;
                    }
                    work.stage = FightTextStage::MenuCursorSync;
                    remaining = 3; // HandleMenuInput's PlaceMenuCursor/Delay3.
                    break;
                }
                FightTextStage::MenuCursorSync => {
                    remaining = remaining.saturating_sub(1);
                    if remaining != 0 {
                        break;
                    }
                    let pressed = sample_pressed(&mut work.last_polled_buttons, held);
                    self.phase = BattlePhase::MoveSelect;
                    let input = BattleInput {
                        a: pressed & 1 != 0,
                        b: pressed & 2 != 0,
                        right: pressed & 16 != 0,
                        left: pressed & 32 != 0,
                        up: pressed & 64 != 0,
                        down: pressed & 128 != 0,
                    };
                    receipt.press_ab |= pressed & 3 != 0;
                    return self.update_move_select(
                        input,
                        BattleTextFrame {
                            held_buttons: held,
                            settings: Some(work.settings),
                        },
                        receipt,
                    );
                }
            }
        }
        self.keep_fight_text(messages, current, remaining, next_phase, work)
    }
}
