//! BikeShop's actual no-voucher text_asm caller. It borrows the field's one
//! Joypad ledger; text/menu phases never carry another last-button sample.
use super::script_bridge::PendingChoice;
use super::{BedroomDialogue, OverworldScreen};
use crate::alloc_prelude::*;
use crate::foreground_input::{A, B, DOWN, UP};
use dotzuki_engine::GameData;
use pokered_data::tilesets::TilesetId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum TextPart {
    WelcomeFirst,
    WelcomeLast,
    Question,
    CantAfford,
    Goodbye,
}

#[derive(Clone, Copy)]
enum TextEntry {
    Display,
    Print,
    ContinueParagraph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum AfterPrompt {
    WelcomeParagraph,
    Question,
    Goodbye,
    NativePage(TextPart),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Phase {
    Start,
    Text(TextPart),
    Protected { remaining: u8, next: AfterPrompt },
    Prompt(AfterPrompt),
    ParagraphClear { remaining: u8 },
    MenuPreparation { remaining: u8 },
    Menu,
    OuterPrompt,
    OuterHold,
    CloseWait,
    Done,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BikeShopOfferState {
    texts: Vec<String>,
    phase: Phase,
    pub selected: u8,
    /// Actual returned hJoy5, not raw held and not an inferred cancel row.
    pub accepted_buttons: Option<u8>,
}

impl BikeShopOfferState {
    pub fn new(texts: Vec<String>) -> Self {
        assert_eq!(texts.len(), 7, "validated BikeShopOffer payload");
        Self {
            texts,
            phase: Phase::Start,
            selected: 0,
            accepted_buttons: None,
        }
    }
    pub fn accepts_input(&self) -> bool {
        self.phase == Phase::Menu
    }
    pub fn is_done(&self) -> bool {
        self.phase == Phase::Done
    }
}

impl<G: GameData<Tileset = TilesetId>> OverworldScreen<G> {
    fn bike_text(&mut self, state: &mut BikeShopOfferState, part: TextPart, entry: TextEntry) {
        let index = match part {
            TextPart::WelcomeFirst => 0,
            TextPart::WelcomeLast => 1,
            TextPart::Question => 2,
            TextPart::CantAfford => 3,
            TextPart::Goodbye => 4,
        };
        let mut dialogue = BedroomDialogue::from_message(&state.texts[index]);
        dialogue.set_text_delay_frames(self.text_delay_frames);
        match entry {
            TextEntry::Display => dialogue.begin_field_display(true),
            TextEntry::Print => dialogue.begin_field_display(false),
            // PARA is inside this PlaceString. After clear+Delay20 it jumps
            // NextChar; there is no second PrintText prologue/Delay3.
            TextEntry::ContinueParagraph => {}
        }
        self.pending_dialogue = Some(dialogue);
        self.last_script_dialogue = None;
        self.retain_field_text = true;
        state.phase = Phase::Text(part);
    }

    fn bike_pressed(&mut self) -> Option<u8> {
        let Some(owner) = self.source_foreground_mut() else {
            self.block_source_foreground(
                crate::foreground_input::ForegroundInputError::MissingFieldOwner,
            );
            return None;
        };
        let pressed = owner.poll().pressed;
        if pressed.is_none() {
            self.block_source_foreground(
                crate::foreground_input::ForegroundInputError::UnknownBeforePrompt,
            );
        }
        pressed
    }

    pub(super) fn tick_bike_shop_offer(&mut self, state: &mut BikeShopOfferState) -> bool {
        match state.phase {
            Phase::Start => {
                // Ordinary talk has just executed JoypadOverworld. Reuse its
                // real full-byte poll. A manually-started unsupported snapshot
                // must not invent preceding history from physical prev_A.
                self.begin_source_field();
                self.bike_text(state, TextPart::WelcomeFirst, TextEntry::Display);
            }
            Phase::Text(part) => {
                let no_delay = self.no_text_delay;
                let (dialogue, owner) = (&mut self.pending_dialogue, &mut self.source_foreground);
                if let (Some(dialogue), Some(owner)) = (dialogue.as_mut(), owner.as_mut()) {
                    if !dialogue.waiting_for_input() {
                        dialogue.reveal_next_char_with_source(no_delay, owner);
                    }
                    if !dialogue.waiting_for_input() {
                        return false;
                    }
                    // Extra native layout pages retain their existing explicit
                    // ACK; source PARA provenance is the separate payload part.
                    if dialogue.has_more_pages() {
                        state.phase = Phase::Protected {
                            remaining: 3,
                            next: AfterPrompt::NativePage(part),
                        };
                        return false;
                    }
                }
                state.phase = match part {
                    TextPart::WelcomeFirst => Phase::Protected {
                        remaining: 3,
                        next: AfterPrompt::WelcomeParagraph,
                    },
                    TextPart::WelcomeLast => Phase::Protected {
                        remaining: 3,
                        next: AfterPrompt::Question,
                    },
                    TextPart::CantAfford => Phase::Protected {
                        remaining: 3,
                        next: AfterPrompt::Goodbye,
                    },
                    TextPart::Goodbye => Phase::OuterPrompt,
                    TextPart::Question => {
                        self.last_script_dialogue = self.pending_dialogue.take();
                        if let Some(choice) = self.pending_choice.as_mut() {
                            choice.cursor_visible = true;
                        }
                        Phase::MenuPreparation { remaining: 3 }
                    }
                };
            }
            Phase::Protected { remaining, next } => {
                if remaining > 1 {
                    state.phase = Phase::Protected {
                        remaining: remaining - 1,
                        next,
                    };
                } else {
                    state.phase = Phase::Prompt(next);
                    return self.tick_bike_shop_offer(state);
                }
            }
            Phase::Prompt(next) => {
                let Some(pressed) = self.bike_pressed() else {
                    return false;
                };
                if pressed & (A | B) == 0 {
                    return false;
                }
                // Only ManualTextScroll emits this. Outer final ACK is silent.
                self.sfx_event = super::screen::OverworldSfxEvent::TextAdvance;
                match next {
                    AfterPrompt::WelcomeParagraph => {
                        self.pending_dialogue = None;
                        // Paragraph clears the text rows, not the enclosing
                        // window. Reuse the retained display owner while its
                        // Delay20 neither prints nor accepts another button.
                        self.last_script_dialogue =
                            Some(BedroomDialogue::from_pages(vec![super::DialoguePage {
                                line1: "".into(),
                                line2: "".into(),
                            }]));
                        state.phase = Phase::ParagraphClear { remaining: 20 };
                    }
                    AfterPrompt::Question => {
                        // Manual list, not generic DisplayTwoOptionMenu.
                        self.no_text_delay = true;
                        let mut choice = PendingChoice::new(state.texts[5..7].to_vec());
                        choice.cursor_visible = false;
                        choice.input_enabled = false;
                        self.pending_choice = Some(choice);
                        self.bike_text(state, TextPart::Question, TextEntry::Print);
                    }
                    AfterPrompt::Goodbye => {
                        self.bike_text(state, TextPart::Goodbye, TextEntry::Print)
                    }
                    AfterPrompt::NativePage(part) => {
                        if let Some(dialogue) = self.pending_dialogue.as_mut() {
                            dialogue.advance();
                        }
                        state.phase = Phase::Text(part);
                    }
                }
            }
            Phase::ParagraphClear { remaining } => {
                if remaining > 1 {
                    state.phase = Phase::ParagraphClear {
                        remaining: remaining - 1,
                    };
                } else {
                    self.bike_text(state, TextPart::WelcomeLast, TextEntry::ContinueParagraph);
                    // The twentieth successor VBlank returns to NextChar and
                    // can print now, rather than adding a creation-only frame.
                    return self.tick_bike_shop_offer(state);
                }
            }
            Phase::MenuPreparation { remaining } => {
                if remaining > 1 {
                    state.phase = Phase::MenuPreparation {
                        remaining: remaining - 1,
                    };
                } else {
                    state.phase = Phase::Menu;
                    if let Some(choice) = self.pending_choice.as_mut() {
                        choice.input_enabled = true;
                    }
                    return self.tick_bike_shop_offer(state);
                }
            }
            Phase::Menu => {
                // Normal ordinary-field profile is hJoy7=0 (newly pressed).
                // No-poll preparation preserves presses made while waiting.
                let Some(accepted) = self.bike_pressed() else {
                    return false;
                };
                if accepted == 0 {
                    return false;
                }
                if accepted & UP != 0 {
                    state.selected = state.selected.saturating_sub(1);
                } else if accepted & DOWN != 0 {
                    state.selected = (state.selected + 1).min(1);
                }
                if accepted & (A | B) == 0 {
                    if let Some(choice) = self.pending_choice.as_mut() {
                        choice.selected = state.selected.into();
                        choice.input_enabled = false;
                    }
                    state.phase = Phase::MenuPreparation { remaining: 3 };
                    return false;
                }
                state.accepted_buttons = Some(accepted);
                self.sfx_event = super::screen::OverworldSfxEvent::TextAdvance;
                if let Some(choice) = self.pending_choice.as_mut() {
                    choice.input_enabled = false;
                }
                // Test returned accepted B, not raw held B and not row==CANCEL.
                if accepted & B != 0 {
                    self.bike_text(state, TextPart::Goodbye, TextEntry::Print);
                } else {
                    self.no_text_delay = false;
                    self.bike_text(
                        state,
                        if state.selected == 0 {
                            TextPart::CantAfford
                        } else {
                            TextPart::Goodbye
                        },
                        TextEntry::Print,
                    );
                }
            }
            Phase::OuterPrompt => {
                let Some(pressed) = self.bike_pressed() else {
                    return false;
                };
                if pressed & (A | B) != 0 {
                    state.phase = Phase::OuterHold;
                    return self.tick_bike_shop_offer(state);
                }
            }
            Phase::OuterHold => {
                // HoldTextDisplayOpen itself really calls Joypad. It is not a
                // VBlank-only/raw check, and only A delays CloseTextDisplay.
                let Some(owner) = self.source_foreground_mut() else {
                    return false;
                };
                if owner.poll().held & A == 0 {
                    self.pending_dialogue = None;
                    self.last_script_dialogue = None;
                    self.pending_choice = None;
                    self.retain_field_text = false;
                    state.phase = Phase::CloseWait;
                }
            }
            Phase::CloseWait => state.phase = Phase::Done,
            Phase::Done => {}
        }
        state.is_done()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foreground_input::{ForegroundJoypad, LEFT, SELECT, START, UP};
    use crate::overworld::script_bridge::ScriptEffect;
    use crate::overworld::Direction;
    use crate::snapshot::OverworldSnapshot;
    use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
    type Field = OverworldScreen<PokemonRedData>;

    fn step(field: &mut Field, buttons: u8) {
        field.update_frame(ForegroundJoypad::as_field_input(buttons));
        assert!(
            field.source_foreground_error().is_none(),
            "{:?}",
            field.source_foreground_error()
        );
    }
    fn offer(field: &Field) -> Option<&BikeShopOfferState> {
        match &field.active_script_effect {
            Some(ScriptEffect::BikeShopOffer { state }) => Some(state),
            _ => None,
        }
    }
    fn normal_talk(no_delay: bool, speed: u16) -> Field {
        let mut field = Field::new(MapId::BikeShop, None, PokemonRedData);
        // Unit preboot map fixture, not an App runtime setter/witness.
        field.state.player.x = 6;
        field.state.player.y = 4;
        field.state.player.facing = Direction::Up;
        field.text_delay_frames = speed;
        let mut data = crate::save::game_data::GameData::new();
        // Ordinary shop origin has no ALWAYS_ON_BIKE/other status fixture.
        // Exhaustive unrelated-bit preservation is a separate test below.
        if no_delay {
            data.status_flags[5] |= 0x40;
        } else {
            data.status_flags[5] &= !0x40;
        }
        field.restore_system_save_state(&data);
        for _ in 0..8 {
            step(&mut field, 0);
        }
        assert_eq!(field.foreground_observation().unwrap().last(), Some(0));
        for _ in 0..2 {
            step(&mut field, A);
        }
        for _ in 0..8 {
            if offer(&field).is_some() {
                break;
            }
            step(&mut field, 0);
        }
        assert!(
            offer(&field).is_some(),
            "ordinary front-of-clerk A must execute the embedded typed caller"
        );
        field
    }
    fn welcome_prompt(field: &mut Field) {
        let initial = (field.text_delay_frames, field.no_text_delay());
        let mut first_question = None;
        let mut first_answer = None;
        for frame in 0..8000 {
            if let Some(state) = offer(field) {
                if state.phase == Phase::Text(TextPart::Question) && first_question.is_none() {
                    first_question = Some(frame);
                }
                if state.accepted_buttons.is_some() && first_answer.is_none() {
                    first_answer = Some((frame, state.accepted_buttons));
                }
            }
            if offer(field).is_some_and(|state| state.phase == Phase::Prompt(AfterPrompt::Question))
            {
                return;
            }
            step(field, if frame % 16 == 0 { A } else { 0 });
        }
        panic!("ordinary Welcome/PARA/PROMPT never completed: initial={initial:?}, first_question={first_question:?}, first_answer={first_answer:?}, final_effect={:?}, page={:?}, owner={:?}",
            field.active_script_effect_value(), field.pending_dialogue.as_ref().map(|d|
                (d.current_page(), d.char_index(), d.total_chars(), d.waiting_for_input())),
            field.foreground_observation());
    }
    fn begin_question_with_b(field: &mut Field) {
        welcome_prompt(field);
        step(field, 0);
        step(field, B);
        assert_eq!(offer(field).unwrap().phase, Phase::Text(TextPart::Question));
        assert_eq!(field.source_foreground().unwrap().last(), Some(B));
        assert!(field.no_text_delay());
    }

    #[test]
    fn welcome_para_continues_place_string_after_twenty_waits_without_another_printtext_delay() {
        for speed in [1, 3, 5] {
            for no_delay in [false, true] {
                let mut field = normal_talk(no_delay, speed);
                for _ in 0..8000 {
                    if offer(&field).unwrap().phase == Phase::Prompt(AfterPrompt::WelcomeParagraph)
                    {
                        break;
                    }
                    step(&mut field, 0);
                }
                assert_eq!(
                    offer(&field).unwrap().phase,
                    Phase::Prompt(AfterPrompt::WelcomeParagraph)
                );
                step(&mut field, A);
                assert_eq!(
                    offer(&field).unwrap().phase,
                    Phase::ParagraphClear { remaining: 20 }
                );
                assert_eq!(field.source_foreground().unwrap().last(), Some(A));
                for elapsed in 1..20 {
                    step(&mut field, 0);
                    assert!(
                        field.pending_dialogue.is_none(),
                        "no next paragraph before wait20: {elapsed}"
                    );
                    assert!(field.retain_field_text);
                    assert_eq!(
                        field.displayed_field_dialogue().unwrap().get_display_text(),
                        Some((String::new(), String::new())),
                        "PARA retains a blank window"
                    );
                    assert!(
                        !field.dialogue_needs_button(),
                        "Delay20 is not another prompt"
                    );
                    assert_eq!(
                        field.source_foreground().unwrap().last(),
                        Some(A),
                        "Delay20 never polls"
                    );
                    if elapsed == 10 {
                        let saved =
                            serde_json::to_string(&OverworldSnapshot::capture(&field)).unwrap();
                        let saved: OverworldSnapshot = serde_json::from_str(&saved).unwrap();
                        saved.restore_into(&mut field);
                        assert_eq!(field.source_foreground().unwrap().last(), Some(A));
                    }
                }
                step(&mut field, 0);
                let dialogue = field
                    .pending_dialogue
                    .as_ref()
                    .expect("wait20 resumes the next paragraph");
                assert!(dialogue.current().unwrap().line1.starts_with("Have we got"));
                assert_eq!(
                    dialogue.char_index(),
                    if no_delay { dialogue.total_chars() } else { 1 },
                    "NextChar resumes in the same update, without another inner PrintText3"
                );
                assert_eq!(
                    field.source_foreground().unwrap().last(),
                    if no_delay { Some(A) } else { Some(0) },
                    "only a real non-NoDelay glyph poll can change last"
                );
            }
        }
    }

    #[test]
    fn ordinary_bike_b_prompt_then_a_held_across_both_preparations_is_fresh() {
        for speed in [1, 3, 5] {
            let mut field = normal_talk(false, speed);
            begin_question_with_b(&mut field);
            for _ in 0..5 {
                step(&mut field, A | SELECT);
                assert_eq!(
                    field.source_foreground().unwrap().last(),
                    Some(B),
                    "PrintText3 and cursor3 do not poll"
                );
            }
            let mid = OverworldSnapshot::capture(&field);
            let mid: OverworldSnapshot =
                serde_json::from_str(&serde_json::to_string(&mid).unwrap()).unwrap();
            mid.restore_into(&mut field);
            assert!(
                field.prev_a_pressed,
                "physical A was held before the snapshot"
            );
            let restored = field.source_foreground().unwrap();
            assert_eq!(
                restored.raw(),
                A | SELECT,
                "raw changed during no-poll preparation"
            );
            assert_eq!(
                restored.last(),
                Some(B),
                "snapshot must not rebase last to physical held A"
            );
            step(&mut field, A | SELECT);
            let state = offer(&field).unwrap();
            assert_eq!(state.accepted_buttons, Some(A | SELECT));
            assert_eq!(state.phase, Phase::Text(TextPart::CantAfford));
            assert!(
                !field.no_text_delay(),
                "accepted A clears before refusal text"
            );
            assert_eq!(field.source_foreground().unwrap().last(), Some(A | SELECT));
        }
    }

    #[test]
    fn bike_tests_accepted_mask_b_priority_and_direction_before_selection() {
        for (prepare, press, accepted, selected, kept, text) in [
            (0, A | B | START, A | B | START, 0, true, TextPart::Goodbye),
            (B, A | B, A, 0, false, TextPart::CantAfford),
            (0, DOWN | A, DOWN | A, 1, false, TextPart::Goodbye),
        ] {
            let mut field = normal_talk(false, 1);
            begin_question_with_b(&mut field);
            for _ in 0..6 {
                step(&mut field, prepare);
            }
            assert_eq!(offer(&field).unwrap().phase, Phase::Menu);
            assert!(offer(&field).unwrap().accepted_buttons.is_none());
            assert!(
                field.pending_dialogue.is_none(),
                "DONE returned without a final-page ACK"
            );
            assert!(!field.dialogue_needs_button());
            let (top, bottom) = field
                .displayed_field_dialogue()
                .unwrap()
                .get_display_text()
                .unwrap();
            assert!(format!("{top} {bottom}").contains("want it?"));
            assert!(field.game_data_requests.is_empty());
            step(&mut field, press);
            let state = offer(&field).unwrap();
            assert_eq!(state.accepted_buttons, Some(accepted));
            assert_eq!(state.selected, selected);
            assert_eq!(state.phase, Phase::Text(text));
            assert_eq!(field.no_text_delay(), kept);
        }
    }

    #[test]
    fn b_cancel_keeps_bit_through_real_outer_ack_hold_and_next_ordinary_text() {
        let mut field = normal_talk(false, 1);
        begin_question_with_b(&mut field);
        for _ in 0..6 {
            step(&mut field, 0);
        }
        step(&mut field, B);
        assert_eq!(offer(&field).unwrap().accepted_buttons, Some(B));
        for _ in 0..3 {
            step(&mut field, B);
        }
        assert_eq!(offer(&field).unwrap().phase, Phase::OuterPrompt);
        assert_eq!(
            field.source_foreground().unwrap().last(),
            Some(B),
            "instant goodbye did not poll"
        );
        for _ in 0..10 {
            step(&mut field, B);
        }
        assert_eq!(
            offer(&field).unwrap().phase,
            Phase::OuterPrompt,
            "held menu B is not outer confirmation"
        );
        step(&mut field, 0);
        step(&mut field, A);
        assert_eq!(offer(&field).unwrap().phase, Phase::OuterHold);
        for _ in 0..2 {
            step(&mut field, A);
        }
        assert_eq!(field.source_foreground().unwrap().last(), Some(A));
        step(&mut field, 0);
        step(&mut field, 0);
        for _ in 0..8 {
            step(&mut field, 0);
        }
        assert!(field.no_text_delay());
        assert!(
            field.source_foreground().is_some(),
            "ordinary Bike owner must persist after its display closes"
        );
        // Ordinary walking reaches the youngster from (2,3), facing left.
        // Row4 x2..6 and row3 x2 are floor0x1f. Column1 on row4 is a
        // displayed bike0x1c, so approach from the right, not from below.
        // No private script call or runtime position/history setter is used.
        for _ in 0..400 {
            step(&mut field, LEFT);
            if field.state.player.x == 2
                && field.state.player.movement_state == crate::overworld::MovementState::Idle
            {
                break;
            }
        }
        assert_eq!((field.state.player.x, field.state.player.y), (2, 4));
        for _ in 0..8 {
            step(&mut field, 0);
        }
        for _ in 0..120 {
            step(&mut field, UP);
            if field.state.player.y == 3
                && field.state.player.movement_state == crate::overworld::MovementState::Idle
            {
                break;
            }
        }
        assert_eq!((field.state.player.x, field.state.player.y), (2, 3));
        for _ in 0..8 {
            step(&mut field, 0);
        }
        for buttons in [LEFT, LEFT, 0, 0, A, A] {
            step(&mut field, buttons);
        }
        assert!(
            matches!(&field.active_script_effect, Some(ScriptEffect::ShowDialogue { text })
            if text.starts_with("These BIKEs"))
        );
        assert_eq!(
            field.source_foreground().unwrap().last(),
            Some(A),
            "actual talk poll owns A"
        );
        for _ in 0..80 {
            step(&mut field, 0);
            if field
                .pending_dialogue
                .as_ref()
                .is_some_and(|d| d.waiting_for_input())
            {
                break;
            }
        }
        let dialogue = field.pending_dialogue.as_ref().unwrap();
        assert_eq!(dialogue.char_index(), dialogue.total_chars());
        assert_eq!(
            field.source_foreground().unwrap().last(),
            Some(0),
            "instant glyphs return to the real outer ACK, whose neutral poll owns last"
        );
        assert!(field.no_text_delay());
    }

    #[test]
    fn canonical_saved_bit_and_legacy_snapshot_preserve_all_other_status_bits() {
        for byte in 0..=u8::MAX {
            let mut field = Field::new(MapId::BikeShop, None, PokemonRedData);
            let mut data = crate::save::game_data::GameData::new();
            data.status_flags = [0xa5; 12];
            data.status_flags[5] = byte;
            field.restore_system_save_state(&data);
            assert_eq!(field.no_text_delay(), byte & 0x40 != 0);
            let snapshot = OverworldSnapshot::capture(&field);
            let mut legacy = serde_json::to_value(&snapshot).unwrap();
            legacy.as_object_mut().unwrap().remove("no_text_delay");
            let legacy: OverworldSnapshot = serde_json::from_value(legacy).unwrap();
            legacy.restore_into(&mut field);
            assert_eq!(
                field.no_text_delay(),
                byte & 0x40 != 0,
                "absent sidecar cannot clear canonical bit"
            );
            for enabled in [false, true] {
                field.set_no_text_delay(enabled);
                let mut actual = data.clone();
                field.write_system_save_state(&mut actual);
                let mut expected = data.status_flags;
                expected[5] = (byte & !0x40) | if enabled { 0x40 } else { 0 };
                assert_eq!(actual.status_flags, expected);
                let mut save = crate::save::SaveData::new();
                save.game_data = actual;
                let bytes = crate::save::sram_export::export_sram(&save);
                let restored = crate::save::sram_import::import_sram(&bytes).unwrap();
                // Original SRAM stores ten named status bytes. Container
                // spare slots10/11 have no source slot and import as zero.
                // The separate live RMW assertion above still checks all12.
                assert_eq!(&restored.game_data.status_flags[..10], &expected[..10]);
                assert_eq!(&restored.game_data.status_flags[10..], &[0, 0]);
            }
        }
    }

    #[test]
    fn frontend_control_filter_never_rewrites_the_canonical_physical_byte() {
        let mut field = normal_talk(false, 1);
        begin_question_with_b(&mut field);
        for _ in 0..6 {
            step(&mut field, 0);
        }
        let physical = A | B | SELECT | START;
        let logical = physical & !START; // existing soft-reset START suppression
        field.update_frame_with_physical_buttons(
            ForegroundJoypad::as_field_input(logical),
            physical,
        );
        assert_eq!(offer(&field).unwrap().accepted_buttons, Some(physical));
        assert_eq!(field.source_foreground().unwrap().last(), Some(physical));
        assert!(field.no_text_delay());
    }

    #[test]
    fn restored_true_no_delay_still_uses_real_field_poll_and_full_welcome_prompt() {
        let mut field = normal_talk(true, 5);
        begin_question_with_b(&mut field);
        for _ in 0..5 {
            step(&mut field, A);
        }
        assert_eq!(field.source_foreground().unwrap().last(), Some(B));
        step(&mut field, A);
        assert_eq!(offer(&field).unwrap().accepted_buttons, Some(A));
        assert!(!field.no_text_delay());
    }
}
