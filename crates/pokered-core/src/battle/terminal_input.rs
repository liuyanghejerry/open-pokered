//! Typed terminal PrintText calls. This is independent of FIGHT refusal text.

use super::*;
use crate::foreground_input::ForegroundJoypad;
use crate::text::terminal_input::{TerminalTextEnd, TerminalTextKind, TerminalTextWork};

impl BattleScreen {
    /// The renderer reads this work's cursor, never an independent full page.
    pub fn terminal_text_work(&self) -> Option<&TerminalTextWork> {
        match &self.phase {
            BattlePhase::TerminalText { work, .. } => Some(work),
            _ => None,
        }
    }

    /// Bind a call identity before localization/pagination. The caller has
    /// verified the normal PrintText delay flags; no substring identifies it.
    pub(super) fn terminal_text_phase(
        &self,
        messages: Vec<String>,
        kind: TerminalTextKind,
        options_delay: u8,
        next: BattlePhase,
    ) -> BattlePhase {
        let messages: Vec<String> = messages
            .into_iter()
            .flat_map(|message| {
                paginate_battle_text(&pokered_data::battle_text::localize(&message, self.is_zh))
            })
            .collect();
        if messages.is_empty() {
            return next;
        }
        let work = TerminalTextWork::new(
            kind,
            messages[0].clone(),
            TerminalTextEnd::Prompt,
            options_delay,
            3,
        );
        BattlePhase::TerminalText {
            messages,
            current: 0,
            work,
            next_phase: Box::new(next),
        }
    }

    pub(super) fn update_terminal_text(
        &mut self,
        messages: Vec<String>,
        current: usize,
        mut work: TerminalTextWork,
        next_phase: Box<BattlePhase>,
        owner: Option<&mut ForegroundJoypad>,
        receipt: &mut FightTextReceipt,
    ) -> ScreenAction {
        // Old edge-only wrappers cannot resume a source owner with a made-up
        // six-key sample. They never create this phase themselves.
        let Some(owner) = owner else {
            return ScreenAction::Continue;
        };
        if self.hp_bar_anim.is_active() {
            return ScreenAction::Continue;
        }
        let step = work.tick(owner, false);
        receipt.press_ab |= step.press_ab;
        self.current_message = Some(work.visible_text());
        if step.complete {
            let next = current + 1;
            if next == messages.len() {
                self.current_message = None;
                self.phase = *next_phase;
                self.post_text_transition();
                return ScreenAction::Continue;
            }
            work = TerminalTextWork::new(
                work.kind,
                messages[next].clone(),
                TerminalTextEnd::Prompt,
                work.options_delay(),
                3,
            );
            self.phase = BattlePhase::TerminalText {
                messages,
                current: next,
                work,
                next_phase,
            };
        } else {
            self.phase = BattlePhase::TerminalText {
                messages,
                current,
                work,
                next_phase,
            };
        }
        ScreenAction::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foreground_input::{A, B, SELECT, START};

    fn trainer_prize() -> BattleScreen {
        let mon = crate::pokemon::stats::create_pokemon(
            pokered_data::species::Species::Bulbasaur,
            34,
            [0x88, 0x88],
        )
        .unwrap();
        let mut battle = BattleScreen::from_parties(
            false,
            &[mon.clone()],
            &[mon],
            Some(pokered_data::trainer_data::TrainerClass::Burglar),
        );
        battle.end_battle_text = Some("You beat me!".into());
        battle.phase = BattlePhase::TrainerVictory {
            phase: VictoryPhase::EndBattleText,
            wait_frames: 60,
            player_won: true,
        };
        battle
    }

    #[test]
    fn actual_trainer_victory_creates_prize_owner_and_keeps_quote_separate() {
        let mut battle = trainer_prize();
        let mut owner = ForegroundJoypad::default();
        let mut receipt = FightTextReceipt::default();
        battle.update_frame_with_terminal_input(
            BattleInput::none(),
            BattleTextFrame {
                held_buttons: START | SELECT,
                settings: Some(BattleTextSettings::fight_menu(1, false)),
            },
            &mut receipt,
            &mut owner,
        );
        let BattlePhase::ShowingText {
            messages,
            next_phase,
            ..
        } = &battle.phase
        else {
            panic!("quote must remain its independent existing text call");
        };
        assert_eq!(messages, &["You beat me!".to_string()]);
        assert!(
            matches!(&**next_phase, BattlePhase::TerminalText { work, .. }
            if work.kind == TerminalTextKind::Prize)
        );
        assert_eq!(
            owner.last(),
            None,
            "creating a prize continuation is not a poll"
        );
    }

    #[test]
    fn old_edge_only_update_keeps_the_existing_combined_showing_text_api() {
        let mut battle = trainer_prize();
        battle.update_frame(BattleInput::none());
        let BattlePhase::ShowingText {
            messages,
            next_phase,
            ..
        } = &battle.phase
        else {
            panic!("None wrapper must retain the existing public sequence");
        };
        assert!(messages[0].contains("You beat me!"));
        assert!(messages[1].contains("for\nwinning!"));
        assert!(matches!(
            &**next_phase,
            BattlePhase::TrainerVictory { wait_frames: 0, .. }
        ));
    }

    #[test]
    fn terminal_prize_uses_real_letter_and_prompt_polls_after_snapshot() {
        let mut battle = BattleScreen::new(false);
        battle.phase = battle.terminal_text_phase(
            vec!["X".into()],
            TerminalTextKind::Prize,
            1,
            BattlePhase::TrainerVictory {
                phase: VictoryPhase::EndBattleText,
                wait_frames: 0,
                player_won: true,
            },
        );
        let mut owner = ForegroundJoypad::default();
        let mut receipt = FightTextReceipt::default();
        let update = |battle: &mut BattleScreen,
                      owner: &mut ForegroundJoypad,
                      receipt: &mut FightTextReceipt,
                      held| {
            battle.update_frame_with_terminal_input(
                BattleInput::none(),
                BattleTextFrame {
                    held_buttons: held,
                    settings: None,
                },
                receipt,
                owner,
            )
        };
        for _ in 0..3 {
            update(&mut battle, &mut owner, &mut receipt, A | SELECT);
        }
        assert_eq!(owner.last(), Some(A | SELECT));
        update(&mut battle, &mut owner, &mut receipt, START);
        for _ in 0..2 {
            update(&mut battle, &mut owner, &mut receipt, START);
        }
        let snapshot = crate::snapshot::BattleSnapshot::capture(&battle);
        let json = serde_json::to_string(&(snapshot, owner)).unwrap();
        let (snapshot, mut owner): (crate::snapshot::BattleSnapshot, ForegroundJoypad) =
            serde_json::from_str(&json).unwrap();
        snapshot.restore_into(&mut battle);
        update(&mut battle, &mut owner, &mut receipt, B | START);
        assert!(receipt.press_ab);
        assert_eq!(owner.last(), Some(B | START));
        assert!(matches!(
            battle.phase,
            BattlePhase::TrainerVictory { wait_frames: 0, .. }
        ));
        // The non-input settlement call must not stamp the current raw mask.
        update(&mut battle, &mut owner, &mut receipt, SELECT);
        assert_eq!(owner.last(), Some(B | START));
        assert!(!receipt.press_ab);
    }
}
