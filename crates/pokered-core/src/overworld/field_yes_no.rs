//! Explicit field question/DONE + YesNoChoice, using the shared foreground ledger.
use super::{
    screen::OverworldSfxEvent,
    script_bridge::{PendingChoice, ScriptEffect},
    OverworldScreen,
};
use crate::{
    alloc_prelude::*,
    foreground_input::{ForegroundInputError, A, B, DOWN, LEFT, RIGHT, SELECT, START, UP},
};
use dotzuki_engine::GameData;
use pokered_data::{script_command::FieldTextPrompt, tilesets::TilesetId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Phase {
    Question,
    CursorDelay(u8),
    Poll,
    Completion(u8),
    Done,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FieldYesNoState {
    question: Box<ScriptEffect>,
    labels: [String; 2],
    phase: Phase,
    logical: u8,
    displayed: u8,
    result: Option<u8>,
    started: bool,
}
impl FieldYesNoState {
    pub fn new(question: String, labels: [String; 2]) -> Self {
        Self {
            question: Box::new(ScriptEffect::PrintFieldText {
                text: question,
                retain_window: true,
                prompt: FieldTextPrompt::Done,
                prompt_frames_remaining: 0,
                prompt_last_buttons: None,
                leading_sound: None,
                leading_sound_started: false,
            }),
            labels,
            phase: Phase::Question,
            logical: 0,
            displayed: 0,
            result: None,
            started: false,
        }
    }
    /// The caller already printed the parent text. Do not print or ACK it again.
    pub(super) fn preprinted(labels: [String; 2]) -> Self {
        let mut state = Self::new(String::new(), labels);
        state.phase = Phase::CursorDelay(3);
        state
    }
    pub(super) fn open_preprinted<G: GameData<Tileset = TilesetId>>(&self, field: &mut OverworldScreen<G>) {
        field.set_no_text_delay(false);
        let mut choice = PendingChoice::new(self.labels.to_vec());
        choice.input_enabled = false;
        field.pending_choice = Some(choice);
    }
    pub fn accepts_input(&self) -> bool {
        self.phase == Phase::Poll
    }
    pub fn result(&self) -> u8 {
        self.result.expect("completed YesNoChoice")
    }
}
impl<G: GameData<Tileset = TilesetId>> OverworldScreen<G> {
    pub(super) fn tick_field_yes_no(&mut self, state: &mut FieldYesNoState) -> bool {
        match state.phase {
            Phase::Question => {
                if !state.started {
                    self.begin_source_field();
                    state.started = true;
                }
                // PrintLetterDelay performs real held-only polls. The final DONE
                // returns without an extra ACK and leaves its question visible.
                if self.tick_owned_field_text(&mut state.question) {
                    // DisplayTwoOptionMenu prints its labels without delay,
                    // then explicitly clears BIT_NO_TEXT_DELAY before input.
                    self.set_no_text_delay(false);
                    let mut choice = PendingChoice::new(state.labels.to_vec());
                    choice.input_enabled = false;
                    self.pending_choice = Some(choice);
                    state.phase = Phase::CursorDelay(3);
                }
                return false;
            }
            Phase::CursorDelay(n) if n > 1 => {
                state.phase = Phase::CursorDelay(n - 1);
                return false;
            }
            Phase::CursorDelay(_) => state.phase = Phase::Poll,
            Phase::Completion(n) if n > 1 => {
                state.phase = Phase::Completion(n - 1);
                return false;
            }
            Phase::Completion(_) => {
                // Both answers restore the saved parent on the fifteenth next
                // VBlank, before the numeric result resumes the scene.
                self.pending_choice = None;
                state.phase = Phase::Done;
                return true;
            }
            Phase::Done => return true,
            Phase::Poll => {}
        }
        let Some(owner) = self.source_foreground.as_mut() else {
            self.block_source_foreground(ForegroundInputError::MissingFieldOwner);
            return false;
        };
        let Some(pressed) = owner.poll().pressed else {
            self.block_source_foreground(ForegroundInputError::UnknownBeforePrompt);
            return false;
        };
        // HandleMenuInput processes UP before DOWN, and does not wrap these
        // two-option menus. Then DisplayTwoOptionMenu gives B priority over A.
        if pressed & UP != 0 {
            state.logical = state.logical.saturating_sub(1);
        } else if pressed & DOWN != 0 {
            state.logical = (state.logical + 1).min(1);
        }
        if pressed & (A | B) != 0 {
            if pressed & B != 0 {
                state.logical = 1;
            }
            state.result = Some(state.logical);
            state.phase = Phase::Completion(15);
            self.sfx_event = OverworldSfxEvent::TextAdvance;
            // Acceptance skips PlaceMenuCursor: retain the previously drawn
            // arrow even for direction+A or B forcing the second result.
        } else if pressed & (UP | DOWN | LEFT | RIGHT | START | SELECT) != 0 {
            state.displayed = state.logical;
            state.phase = Phase::CursorDelay(3);
        }
        if let Some(choice) = self.pending_choice.as_mut() {
            choice.selected = u32::from(state.displayed);
            choice.input_enabled = state.accepts_input();
        }
        false
    }
}

#[cfg(all(test, not(target_os = "none"), not(feature = "script-boa")))]
mod tests {
    use super::*;
    use crate::foreground_input::ForegroundJoypad;
    use crate::overworld::script_bridge;
    use crate::snapshot::OverworldSnapshot;
    use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
    type Field = OverworldScreen<PokemonRedData>;
    fn step(f: &mut Field, keys: u8) {
        f.update_frame(ForegroundJoypad::as_field_input(keys));
        assert_eq!(f.source_foreground_error(), None);
    }
    fn state(f: &Field) -> &FieldYesNoState {
        let Some(ScriptEffect::AskFieldYesNo { state }) = &f.active_script_effect else {
            panic!("expected YES/NO")
        };
        state
    }
    fn question(number: usize) -> Field {
        let mut f = Field::new(MapId::CinnabarGym, None, PokemonRedData);
        f.script_engine.seed_text("playerFacing", "up");
        let command = f
            .script_engine
            .call_function_no_args(&format!("quizMachine{number}"))
            .unwrap()
            .unwrap();
        f.active_script_effect = Some(script_bridge::dispatch_command(&command));
        for frame in 0..5000 {
            if f.choice_accepts_input() {
                return f;
            }
            let ack = f
                .pending_dialogue
                .as_ref()
                .is_some_and(|d| d.waiting_for_input())
                && frame % 2 == 0;
            step(&mut f, if ack { A } else { 0 });
        }
        panic!("real quiz did not reach a menu")
    }
    fn restore(f: &mut Field) {
        let expected = serde_json::to_value(OverworldSnapshot::capture(f)).unwrap();
        let snapshot: OverworldSnapshot = serde_json::from_value(expected.clone()).unwrap();
        snapshot.restore_into(f);
        assert_eq!(
            serde_json::to_value(OverworldSnapshot::capture(f)).unwrap(),
            expected
        );
    }
    #[test]
    fn all_six_questions_clamp_both_ends_and_retain_the_question_during_three_frame_cursor_waits() {
        for number in 1..=6 {
            let mut f = question(number);
            step(&mut f, 0);
            let text = serde_json::to_value(f.displayed_field_dialogue()).unwrap();
            for (key, expected) in [(UP, 0), (DOWN, 1), (DOWN, 1), (UP, 0)] {
                step(&mut f, key);
                assert_eq!(f.pending_choice.as_ref().unwrap().selected, expected);
                assert_eq!(state(&f).phase, Phase::CursorDelay(3));
                let last = f.foreground_observation().unwrap().last();
                for _ in 0..2 {
                    step(&mut f, 0);
                    restore(&mut f);
                    assert_eq!(f.foreground_observation().unwrap().last(), last);
                    assert!(!f.choice_accepts_input());
                }
                step(&mut f, 0);
                assert!(f.choice_accepts_input());
                assert_eq!(
                    serde_json::to_value(f.displayed_field_dialogue()).unwrap(),
                    text
                );
            }
        }
    }
    #[test]
    fn confirm_priorities_keep_drawn_arrow_and_wait_fifteen_frames_before_resuming_actual_branch() {
        for (keys, result) in [
            (A | B, 1),
            (DOWN | A, 1),
            (UP | DOWN | A, 0),
            (B, 1),
            (A, 0),
        ] {
            let mut f = question(1);
            step(&mut f, 0);
            step(&mut f, keys);
            assert_eq!(state(&f).result(), result);
            assert_eq!(state(&f).phase, Phase::Completion(15));
            assert_eq!(f.sfx_event, OverworldSfxEvent::TextAdvance);
            assert_eq!(
                f.pending_choice.as_ref().unwrap().selected,
                0,
                "confirmation does not redraw the cursor"
            );
            let last = f.foreground_observation().unwrap().last();
            for _ in 0..14 {
                step(&mut f, START | SELECT);
                restore(&mut f);
                assert_eq!(f.foreground_observation().unwrap().last(), last);
                assert!(!f.choice_accepts_input());
                assert!(f.pending_choice.is_some());
            }
            step(&mut f, 0);
            assert!(f.pending_choice.is_none());
            assert!(
                matches!(
                    &f.active_script_effect,
                    Some(ScriptEffect::PrintFieldText { .. })
                ) && result == 0
                    || matches!(&f.active_script_effect, Some(ScriptEffect::WaitSound))
                        && result == 1
            );
        }
    }
    #[test]
    fn ignored_keys_take_cursor_delay_and_raw_waits_do_not_create_a_fresh_press() {
        for key in [LEFT, RIGHT, START, SELECT] {
            let mut f = question(1);
            step(&mut f, 0);
            step(&mut f, key);
            assert_eq!(state(&f).phase, Phase::CursorDelay(3));
            step(&mut f, A);
            step(&mut f, 0);
            step(&mut f, 0);
            assert!(f.choice_accepts_input());
            assert_eq!(
                state(&f).result,
                None,
                "press/release entirely inside Delay3 is not sampled"
            );
        }
    }
}

#[cfg(all(test, not(target_os = "none"), not(feature = "script-boa")))]
mod status_bit_tests {
    use super::*;
    use crate::foreground_input::ForegroundJoypad;
    use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
    #[test]
    fn actual_yes_no_entry_clears_the_original_no_text_delay_writer() {
        let mut f = OverworldScreen::new(MapId::CinnabarGym, None, PokemonRedData);
        // Component entry fixture: a known field Joypad sample precedes the
        // explicit script. The bit can persist from another actual writer.
        let owner = f.begin_source_field();
        owner.read_raw(0);
        owner.poll();
        f.set_no_text_delay(true);
        f.script_engine.seed_text("playerFacing", "up");
        let command = f
            .script_engine
            .call_function_no_args("quizMachine1")
            .unwrap()
            .unwrap();
        f.active_script_effect = Some(super::super::script_bridge::dispatch_command(&command));
        for frame in 0..5000 {
            if f.pending_choice.is_some() {
                assert!(!f.no_text_delay());
                return;
            }
            let ack = f
                .pending_dialogue
                .as_ref()
                .is_some_and(|d| d.waiting_for_input())
                && frame % 2 == 0;
            f.update_frame(ForegroundJoypad::as_field_input(if ack { A } else { 0 }));
            assert_eq!(f.source_foreground_error(), None);
        }
        panic!("actual quiz failed to reach the menu")
    }
}
