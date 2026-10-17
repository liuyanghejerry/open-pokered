//! DisplayPokemonCenterDialogue_: explicit inner PrintText terminators,
//! canonical used bit, HEAL/CANCEL, and the special DisplayTextID outer ACK.
use super::{
    field_yes_no::FieldYesNoState, screen::OverworldGameDataRequest, script_bridge::ScriptEffect,
    OverworldScreen,
};
use crate::{
    alloc_prelude::*,
    foreground_input::{ForegroundInputError, A, B},
};
use dotzuki_engine::GameData;
use pokered_data::{script_command::FieldTextPrompt, tilesets::TilesetId};

fn text(text: String, prompt: FieldTextPrompt) -> Box<ScriptEffect> {
    Box::new(ScriptEffect::PrintFieldText {
        text,
        retain_window: true,
        prompt,
        prompt_frames_remaining: 0,
        prompt_last_buttons: None,
        leading_sound: None,
        leading_sound_started: false,
    })
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum TextPhase {
    Start,
    Prepare,
    Pause(u8),
    Print,
    Done,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PausedText {
    effect: Box<ScriptEffect>,
    phase: TextPhase,
}
impl PausedText {
    fn new(value: String) -> Self {
        Self {
            effect: text(value, FieldTextPrompt::Done),
            phase: TextPhase::Start,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum OfferPhase {
    Welcome,
    ParagraphClear(u8),
    WelcomeLast,
    Question,
    Menu,
    Done,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NurseOfferState {
    used: bool,
    welcome: Box<ScriptEffect>,
    welcome_last: Box<ScriptEffect>,
    question: PausedText,
    menu: FieldYesNoState,
    phase: OfferPhase,
}
impl NurseOfferState {
    pub fn new(used: bool, texts: [String; 5]) -> Self {
        let [welcome, welcome_last, question, heal, cancel] = texts;
        Self {
            used,
            welcome: text(welcome, FieldTextPrompt::Protected),
            welcome_last: text(welcome_last, FieldTextPrompt::Protected),
            question: PausedText::new(question),
            menu: FieldYesNoState::preprinted([heal, cancel]),
            phase: OfferPhase::Welcome,
        }
    }
    pub fn accepts_input(&self) -> bool {
        self.phase == OfferPhase::Menu && self.menu.accepts_input()
    }
    pub fn result(&self) -> u8 {
        self.menu.result()
    }
    pub(super) fn needs_button(&self, native_page: bool) -> bool {
        matches!(self.phase, OfferPhase::Welcome | OfferPhase::WelcomeLast)
            || self.phase == OfferPhase::Question && native_page
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum FarewellPhase {
    Text,
    Ack,
    CloseWait,
    Done,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NurseFarewellState {
    text: PausedText,
    phase: FarewellPhase,
}
impl NurseFarewellState {
    pub(super) fn needs_button(&self, native_page: bool) -> bool {
        self.phase == FarewellPhase::Ack || self.phase == FarewellPhase::Text && native_page
    }
    pub(super) fn shows_arrow(&self, native_page: bool) -> bool {
        self.phase == FarewellPhase::Text && native_page
    }

    pub fn new(value: String) -> Self {
        Self {
            text: PausedText::new(value),
            phase: FarewellPhase::Text,
        }
    }
}
impl<G: GameData<Tileset = TilesetId>> OverworldScreen<G> {
    fn tick_nurse_paused_text(&mut self, state: &mut PausedText) -> bool {
        match state.phase {
            TextPhase::Start => {
                // Initialize PrintText's ordinary Delay3 without printing a glyph.
                self.tick_owned_field_text(&mut state.effect);
                state.phase = TextPhase::Prepare;
                return false;
            }
            TextPhase::Prepare => {
                if !self
                    .pending_dialogue
                    .as_mut()
                    .expect("nurse PrintText")
                    .source_field_preparation_ready()
                {
                    return false;
                }
                let Some(owner) = self.source_foreground_mut() else {
                    self.block_source_foreground(ForegroundInputError::MissingFieldOwner);
                    return false;
                };
                // TextCommand_PAUSE calls Joypad once. Held A/B skips it;
                // otherwise its thirty VBlanks do not poll foreground input.
                if owner.poll().held & (A | B) == 0 {
                    state.phase = TextPhase::Pause(30);
                    return false;
                }
                state.phase = TextPhase::Print;
            }
            TextPhase::Pause(n) if n > 1 => {
                state.phase = TextPhase::Pause(n - 1);
                return false;
            }
            TextPhase::Pause(_) => state.phase = TextPhase::Print,
            TextPhase::Print => {}
            TextPhase::Done => return true,
        }
        if self.tick_owned_field_text(&mut state.effect) {
            state.phase = TextPhase::Done;
            true
        } else {
            false
        }
    }
    pub(super) fn tick_nurse_offer(&mut self, state: &mut NurseOfferState) -> bool {
        self.begin_source_field();
        if state.phase == OfferPhase::Welcome {
            if !self.tick_owned_field_text(&mut state.welcome) {
                return false;
            }
            // Original PARA clears the rows and waits twenty VBlanks.
            self.last_script_dialogue = Some(super::screen::BedroomDialogue::from_pages(vec![
                super::DialoguePage {
                    line1: "".into(),
                    line2: "".into(),
                },
            ]));
            state.phase = OfferPhase::ParagraphClear(20);
            return false;
        }
        if let OfferPhase::ParagraphClear(n) = state.phase {
            if n > 1 {
                state.phase = OfferPhase::ParagraphClear(n - 1);
                return false;
            }
            let ScriptEffect::PrintFieldText { text, .. } = state.welcome_last.as_ref() else {
                unreachable!()
            };
            let names: Vec<&str> = core::iter::once(self.player_name.as_str())
                .chain(core::iter::once(self.rival_name.as_str()))
                .chain(self.script_dialogue_names.iter().map(String::as_str))
                .collect();
            let mut dialogue = super::script_bridge::text_to_dialogue_with_names(text, &names);
            dialogue.set_text_delay_frames(self.text_delay_frames);
            // NextChar after PARA continues the existing PrintText, without
            // another PrintText/Delay3 prologue.
            self.pending_dialogue = Some(dialogue);
            self.last_script_dialogue = None;
            state.phase = OfferPhase::WelcomeLast;
        }
        if state.phase == OfferPhase::WelcomeLast {
            if !self.tick_owned_field_text(&mut state.welcome_last) {
                return false;
            }
            self.game_data_requests
                .push(OverworldGameDataRequest::MarkPokecenterUsed);
            state.phase = if state.used {
                OfferPhase::Menu
            } else {
                OfferPhase::Question
            };
            if state.used {
                state.menu.open_preprinted(self);
                return false;
            }
        }
        if state.phase == OfferPhase::Question {
            if !self.tick_nurse_paused_text(&mut state.question) {
                return false;
            }
            state.menu.open_preprinted(self);
            state.phase = OfferPhase::Menu;
            return false;
        }
        if state.phase == OfferPhase::Menu {
            if !self.tick_field_yes_no(&mut state.menu) {
                return false;
            }
            if state.menu.result() == 0 {
                // SetLastBlackoutMap precedes NeedYourPokemonText, not HealParty.
                if let Some(map) = self.last_map {
                    self.game_data_requests
                        .push(OverworldGameDataRequest::SetBlackoutMap { map });
                }
            }
            state.phase = OfferPhase::Done;
        }
        state.phase == OfferPhase::Done
    }
    pub(super) fn tick_nurse_farewell(&mut self, state: &mut NurseFarewellState) -> bool {
        self.begin_source_field();
        match state.phase {
            FarewellPhase::Text => {
                if !self.tick_nurse_paused_text(&mut state.text) {
                    return false;
                }
                // The nurse's special DisplayTextID dispatch always executes
                // AfterDisplayingTextID, independently of inner PROMPT flags.
                // UpdateSprites rebuilds the nurse from her ordinary facing
                // before returning to DisplayTextID's outer confirmation.
                if let Some(nurse) = self.npc_states.iter_mut().find(|npc| npc.text_id == 1) {
                    nurse.facing = super::Direction::Down;
                    nurse.scripted_frame = None;
                }
                self.pending_dialogue = self.last_script_dialogue.take();
                state.phase = FarewellPhase::Ack;
                self.tick_nurse_farewell(state)
            }
            FarewellPhase::Ack => {
                if self.tick_retained_field_outer_ack() {
                    self.pending_dialogue = None;
                    self.last_script_dialogue = None;
                    self.retain_field_text = false;
                    state.phase = FarewellPhase::CloseWait;
                }
                false
            }
            FarewellPhase::CloseWait => {
                state.phase = FarewellPhase::Done;
                true
            }
            FarewellPhase::Done => true,
        }
    }
}

#[cfg(all(test, not(target_os = "none"), not(feature = "script-boa")))]
mod tests {
    use super::*;
    use crate::{
        foreground_input::{ForegroundJoypad, DOWN, UP},
        snapshot::OverworldSnapshot,
    };
    use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
    type Field = OverworldScreen<PokemonRedData>;
    const CENTERS: [MapId; 12] = [
        MapId::ViridianPokecenter,
        MapId::PewterPokecenter,
        MapId::CeruleanPokecenter,
        MapId::MtMoonPokecenter,
        MapId::RockTunnelPokecenter,
        MapId::VermilionPokecenter,
        MapId::CeladonPokecenter,
        MapId::LavenderPokecenter,
        MapId::FuchsiaPokecenter,
        MapId::CinnabarPokecenter,
        MapId::SaffronPokecenter,
        MapId::IndigoPlateauLobby,
    ];
    fn step(f: &mut Field, keys: u8) {
        f.update_frame(ForegroundJoypad::as_field_input(keys));
        assert_eq!(f.source_foreground_error(), None);
    }
    fn start(map: MapId, used: bool, lang: &str) -> Field {
        let mut f = Field::new(map, None, PokemonRedData);
        f.set_script_lang(lang);
        f.script_engine
            .seed_number("pokecenterUsed", f64::from(used));
        f.last_map = Some(MapId::CeruleanCity);
        // Explicit component entry with a real field poll, not a manufactured
        // previous-frame key. Native/cold-SRAM evidence is separate.
        step(&mut f, 0);
        f.begin_source_field();
        let cmd = f
            .script_engine
            .call_function_no_args("talkNurse")
            .unwrap()
            .unwrap();
        f.active_script_effect = Some(super::super::script_bridge::dispatch_command(&cmd));
        f
    }
    fn state(f: &Field) -> &NurseOfferState {
        let Some(ScriptEffect::PokecenterNurseOffer { state }) = &f.active_script_effect else {
            panic!("nurse offer expected")
        };
        state
    }
    fn snapshot(f: &mut Field) {
        let expected = serde_json::to_value(OverworldSnapshot::capture(f)).unwrap();
        let saved: OverworldSnapshot = serde_json::from_value(expected.clone()).unwrap();
        saved.restore_into(f);
        assert_eq!(
            serde_json::to_value(OverworldSnapshot::capture(f)).unwrap(),
            expected
        );
    }
    fn menu(f: &mut Field) -> (bool, usize) {
        let mut saw_question = false;
        let mut marked = 0;
        for frame in 0..8000 {
            saw_question |= state(f).phase == OfferPhase::Question;
            if f.choice_accepts_input() {
                return (saw_question, marked);
            }
            let ack = frame % 2 == 0
                && f.pending_dialogue
                    .as_ref()
                    .is_some_and(|d| d.waiting_for_input());
            step(f, if ack { A } else { 0 });
            marked += f
                .game_data_requests
                .iter()
                .filter(|r| matches!(r, OverworldGameDataRequest::MarkPokecenterUsed))
                .count();
            f.game_data_requests.clear();
        }
        panic!("nurse did not open menu")
    }
    #[test]
    fn nurse_all_twelve_bilingual_scenes_skip_only_the_used_question_and_record_destination_before_need(
    ) {
        for map in CENTERS {
            for lang in ["en", "zh"] {
                for used in [false, true] {
                    for answer in [0, 1] {
                        let mut f = start(map, used, lang);
                        let (question, marked) = menu(&mut f);
                        assert_eq!(question, !used, "{map:?}/{lang}");
                        assert_eq!(
                            marked, 1,
                            "both acceptance and decline mark the canonical bit"
                        );
                        assert_eq!(
                            f.pending_choice.as_ref().unwrap().options,
                            if lang == "en" {
                                vec!["HEAL", "CANCEL"]
                            } else {
                                vec!["治疗", "取消"]
                            }
                        );
                        step(&mut f, 0);
                        step(&mut f, if answer == 0 { A } else { B });
                        for _ in 0..14 {
                            step(&mut f, 0);
                            snapshot(&mut f);
                            assert!(f.game_data_requests.is_empty());
                        }
                        step(&mut f, 0);
                        let recorded = f
                            .game_data_requests
                            .iter()
                            .filter(|r| {
                                matches!(
                                    r,
                                    OverworldGameDataRequest::SetBlackoutMap {
                                        map: MapId::CeruleanCity
                                    }
                                )
                            })
                            .count();
                        assert_eq!(recorded, usize::from(answer == 0));
                        assert!(
                            !f.heal_requested,
                            "recording occurs before Need text and HealParty"
                        );
                        assert!(if answer == 0 {
                            matches!(
                                f.active_script_effect,
                                Some(ScriptEffect::PrintFieldText {
                                    prompt: FieldTextPrompt::Done,
                                    ..
                                })
                            )
                        } else {
                            matches!(
                                f.active_script_effect,
                                Some(ScriptEffect::PokecenterFarewell { .. })
                            )
                        });
                    }
                }
            }
        }
    }
    #[test]
    fn nurse_menu_clamps_and_preserves_three_frame_and_fifteen_frame_raw_waits() {
        let mut f = start(MapId::ViridianPokecenter, false, "en");
        menu(&mut f);
        step(&mut f, 0);
        step(&mut f, UP);
        assert_eq!(f.pending_choice.as_ref().unwrap().selected, 0);
        for _ in 0..3 {
            step(&mut f, 0);
        }
        step(&mut f, DOWN);
        assert_eq!(f.pending_choice.as_ref().unwrap().selected, 1);
        let last = f.foreground_observation().unwrap().last();
        step(&mut f, A);
        step(&mut f, 0);
        assert_eq!(f.foreground_observation().unwrap().last(), last);
        step(&mut f, 0);
        assert!(
            f.choice_accepts_input(),
            "A pressed/released inside Delay3 is lost"
        );
        assert!(state(&f).menu.accepts_input());
    }
    #[test]
    fn nurse_text_pause_polls_once_then_waits_thirty_raw_vblanks_or_skips_when_held() {
        for keys in [0, A, B] {
            let mut f = start(MapId::ViridianPokecenter, false, "en");
            let mut paused = PausedText::new("Question?".into());
            f.last_script_dialogue = Some(super::super::screen::BedroomDialogue::from_message(
                "Welcome",
            ));
            f.active_script_effect = None;
            assert!(!f.tick_nurse_paused_text(&mut paused));
            for n in 0..3 {
                f.source_foreground_mut().unwrap().read_raw(keys);
                f.tick_nurse_paused_text(&mut paused);
                if n < 2 {
                    assert_eq!(paused.phase, TextPhase::Prepare);
                }
            }
            if keys == 0 {
                assert_eq!(paused.phase, TextPhase::Pause(30));
                let last = f.foreground_observation().unwrap().last();
                for n in 0..29 {
                    f.source_foreground_mut()
                        .unwrap()
                        .read_raw(if n % 2 == 0 { A } else { 0 });
                    assert!(!f.tick_nurse_paused_text(&mut paused));
                    assert_eq!(f.foreground_observation().unwrap().last(), last);
                    assert_eq!(f.pending_dialogue.as_ref().unwrap().char_index(), 0);
                }
                f.tick_nurse_paused_text(&mut paused);
            }
            assert_eq!(paused.phase, TextPhase::Print);
            assert!(f.pending_dialogue.as_ref().unwrap().char_index() > 0);
        }
    }
    #[test]
    fn nurse_farewell_done_requires_silent_outer_ack_then_a_release_and_one_close_frame() {
        let mut f = start(MapId::ViridianPokecenter, true, "en");
        f.active_script_effect = None;
        let mut state = NurseFarewellState::new("Bye!".into());
        for _ in 0..300 {
            f.source_foreground_mut().unwrap().read_raw(0);
            f.tick_nurse_farewell(&mut state);
            if state.phase == FarewellPhase::Ack {
                break;
            }
        }
        assert_eq!(state.phase, FarewellPhase::Ack);
        assert!(f.pending_dialogue.is_some());
        f.source_foreground_mut().unwrap().read_raw(A);
        assert!(!f.tick_nurse_farewell(&mut state));
        assert_eq!(state.phase, FarewellPhase::Ack);
        assert!(f.pending_dialogue.as_ref().unwrap().holding_open());
        assert_eq!(f.sfx_event, super::super::screen::OverworldSfxEvent::None);
        f.source_foreground_mut().unwrap().read_raw(0);
        assert!(!f.tick_nurse_farewell(&mut state));
        assert_eq!(state.phase, FarewellPhase::CloseWait);
        assert!(f.displayed_field_dialogue().is_none());
        assert!(f.tick_nurse_farewell(&mut state));
    }
}

#[cfg(all(test, not(target_os = "none"), not(feature = "script-boa")))]
mod paragraph_tests {
    use super::*;
    use crate::foreground_input::ForegroundJoypad;
    use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
    #[test]
    fn nurse_welcome_para_has_twenty_raw_vblanks_before_continuing_same_printtext() {
        let mut f = OverworldScreen::new(MapId::ViridianPokecenter, None, PokemonRedData);
        f.update_frame(ForegroundJoypad::as_field_input(0));
        let mut state = NurseOfferState::new(
            false,
            [
                "Welcome!".into(),
                "We heal your Pokemon.".into(),
                "Heal?".into(),
                "HEAL".into(),
                "CANCEL".into(),
            ],
        );
        for frame in 0..1000 {
            let ack = frame % 2 == 0
                && f.pending_dialogue
                    .as_ref()
                    .is_some_and(|d| d.waiting_for_input());
            f.begin_source_field().read_raw(if ack { A } else { 0 });
            assert!(!f.tick_nurse_offer(&mut state));
            if state.phase == OfferPhase::ParagraphClear(20) {
                break;
            }
        }
        assert_eq!(state.phase, OfferPhase::ParagraphClear(20));
        assert!(
            f.game_data_requests.is_empty(),
            "mark only after the last welcome PROMPT"
        );
        let last = f.foreground_observation().unwrap().last();
        for n in 0..19 {
            f.source_foreground_mut()
                .unwrap()
                .read_raw(if n % 2 == 0 { A } else { B });
            assert!(!f.tick_nurse_offer(&mut state));
            assert_eq!(f.foreground_observation().unwrap().last(), last);
            assert!(f.pending_dialogue.is_none());
            assert!(f
                .displayed_field_dialogue()
                .unwrap()
                .get_display_text()
                .is_none_or(|(a, b)| a.trim().is_empty() && b.trim().is_empty()));
        }
        assert!(!f.tick_nurse_offer(&mut state));
        assert_eq!(state.phase, OfferPhase::WelcomeLast);
        assert_eq!(
            f.pending_dialogue.as_ref().unwrap().char_index(),
            1,
            "twentieth returning VBlank prints without a second Delay3"
        );
    }
}
