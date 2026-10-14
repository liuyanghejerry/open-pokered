//! ItemUseBall's continuation after the caught mon has been committed once.
//! The exact insertion slot belongs to settlement, not a species search or
//! the box's last row. Field scripts and Safari ejection stay paused here.
use super::*;
use pokered_core::battle::settlement::{CapturedLocation, CapturedMonReceipt};
use pokered_core::naming_screen::{
    NamingScreenResult, NamingScreenState, NamingScreenType, NAMING_FLASH_FRAMES,
};
use pokered_core::overworld::script_bridge::PendingChoice;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CapturePhase {
    NewDexMessage,
    NewDexSound,
    NewDexConfirm,
    Entry,
    Question,
    Choice,
    Naming,
    SubmitFlash,
    Transfer,
}

pub(super) struct PostCaptureFlow {
    pub(super) receipt: CapturedMonReceipt,
    pub(super) phase: CapturePhase,
    pub(super) dialogue: Option<BedroomDialogue>,
    pub(super) choice: Option<PendingChoice>,
    pub(super) naming: Option<NamingScreenState>,
    flash_frames: u8,
    met_bill: bool,
}

impl PostCaptureFlow {
    fn new(
        receipt: CapturedMonReceipt,
        met_bill: bool,
        language: pokered_core::game_state::Lang,
    ) -> Self {
        let mut flow = Self {
            receipt,
            phase: CapturePhase::Question,
            dialogue: None,
            choice: None,
            naming: None,
            flash_frames: 0,
            met_bill,
        };
        if receipt.new_pokedex_entry {
            flow.phase = CapturePhase::NewDexMessage;
            let species = pokered_data::lang_data::species_name(
                receipt.species,
                language == pokered_core::game_state::Lang::Zh,
            );
            flow.dialogue = Some(BedroomDialogue::from_message(&if language
                == pokered_core::game_state::Lang::Zh
            {
                format!("将为{species}添加新的图鉴资料！")
            } else {
                format!(
                    "New POKéDEX data\nwill be added for\n{}!",
                    species.to_uppercase()
                )
            }));
        } else {
            flow.question(language);
        }
        flow
    }

    fn question(&mut self, language: pokered_core::game_state::Lang) {
        let species = pokered_data::lang_data::species_name(
            self.receipt.species,
            language == pokered_core::game_state::Lang::Zh,
        );
        self.phase = CapturePhase::Question;
        self.choice = None;
        self.dialogue = Some(BedroomDialogue::from_message(&if language
            == pokered_core::game_state::Lang::Zh
        {
            format!("要给{species}起昵称吗？")
        } else {
            format!("Give a nickname\nto {}?", species.to_uppercase())
        }));
    }

    pub(super) fn awaiting_entry(&self) -> bool {
        self.phase == CapturePhase::Entry
    }
}

impl PokemonGame {
    pub(super) fn begin_post_capture(&mut self, receipt: CapturedMonReceipt) {
        let met_bill = self.overworld.unified_flags().get_flag("EVENT_MET_BILL");
        self.post_capture_flow = Some(Box::new(PostCaptureFlow::new(
            receipt,
            met_bill,
            self.state.config.language,
        )));
    }

    pub(crate) fn clear_post_capture_continuation(&mut self) {
        self.post_capture_flow = None;
    }

    pub(crate) fn post_capture_active(&self) -> bool {
        self.post_capture_flow.is_some()
    }

    pub(super) fn post_capture_choice(&self) -> Option<&PendingChoice> {
        self.post_capture_flow
            .as_ref()
            .and_then(|flow| flow.choice.as_ref())
    }

    pub(super) fn post_capture_dialogue(&self) -> Option<&BedroomDialogue> {
        self.post_capture_flow
            .as_ref()
            .and_then(|flow| flow.dialogue.as_ref())
    }

    pub(super) fn close_post_capture_entry(&mut self) -> bool {
        let Some(flow) = self.post_capture_flow.as_mut() else {
            return false;
        };
        if !flow.awaiting_entry() {
            return false;
        }
        flow.question(self.state.config.language);
        // Settlement already ran before the DATA caller: do not settle again.
        self.state.transition_to(GameScreen::Battle);
        true
    }

    fn post_capture_name(&self, flow: &PostCaptureFlow) -> String {
        let mon = match flow.receipt.location {
            CapturedLocation::Party(index) => self.save_data.party.get(index),
            CapturedLocation::CurrentBox(index) => self.save_data.current_box.get(index),
        };
        let mut buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
        mon.map(|mon| mon.display_name(&mut buf).to_string())
            .unwrap_or_default()
    }

    fn finish_capture_naming(&mut self, flow: &mut PostCaptureFlow) -> bool {
        flow.choice = None;
        flow.naming = None;
        if matches!(flow.receipt.location, CapturedLocation::CurrentBox(_)) {
            let name = self.post_capture_name(flow);
            let text = if self.state.config.language == pokered_core::game_state::Lang::Zh {
                let recipient = if flow.met_bill { "正辉" } else { "某人" };
                format!("{name}被传送到了{recipient}的电脑！")
            } else {
                let recipient = if flow.met_bill { "BILL's" } else { "someone's" };
                format!("{name} was\ntransferred to\n{recipient} PC!")
            };
            flow.phase = CapturePhase::Transfer;
            flow.dialogue = Some(BedroomDialogue::from_message(&text));
            true
        } else {
            false
        }
    }

    fn set_captured_nickname(&mut self, flow: &PostCaptureFlow, name: &str) {
        match flow.receipt.location {
            CapturedLocation::Party(index) => {
                let _ = self.save_data.party.set_nickname(index, name);
            }
            CapturedLocation::CurrentBox(index) => {
                if let Some(mon) = self.save_data.current_box.get_mut(index) {
                    mon.set_nickname(name);
                }
                self.save_data.sync_current_box_to_storage();
            }
        }
    }

    /// A separate modal caller: no map/NPC/Safari update runs until it returns.
    pub(super) fn update_post_capture(&mut self, input: &InputState) {
        let Some(mut flow) = self.post_capture_flow.take() else {
            return;
        };
        let ack = input.is_just_pressed(GbButton::A) || input.is_just_pressed(GbButton::B);
        let held = input.is_held(GbButton::A) || input.is_held(GbButton::B);
        let sound_busy = self
            .audio
            .as_ref()
            .is_some_and(|audio| audio.is_sfx_playing());
        let mut keep = true;
        match flow.phase {
            CapturePhase::NewDexMessage | CapturePhase::Question | CapturePhase::Transfer => {
                let dialogue = flow
                    .dialogue
                    .as_mut()
                    .expect("capture text owns its dialogue");
                dialogue.set_text_delay_frames(self.state.config.text_speed.delay_frames());
                if !dialogue.waiting_for_input() {
                    dialogue.reveal_next_char_with_buttons(held);
                } else if flow.phase == CapturePhase::Question && !dialogue.has_more_pages() {
                    // AskName's question ends in DONE; its final print wait
                    // enters the menu automatically. Opening A cannot pick YES.
                    flow.phase = CapturePhase::Choice;
                    flow.choice = Some(PendingChoice::new(vec![
                        "YES".to_string(),
                        "NO".to_string(),
                    ]));
                } else if flow.phase == CapturePhase::NewDexMessage && !dialogue.has_more_pages() {
                    flow.phase = CapturePhase::NewDexSound;
                    if let Some(audio) = &self.audio {
                        audio.play_sfx(SfxId::DexPageAdded);
                    }
                } else if ack {
                    if dialogue.has_more_pages() {
                        dialogue.advance();
                        if let Some(audio) = &self.audio {
                            audio.play_sfx(SfxId::PressAB);
                        }
                    } else {
                        keep = false;
                    }
                }
            }
            CapturePhase::NewDexSound => {
                if !sound_busy {
                    flow.phase = CapturePhase::NewDexConfirm;
                }
            }
            CapturePhase::NewDexConfirm => {
                if ack {
                    flow.phase = CapturePhase::Entry;
                    flow.dialogue = None;
                    self.prepare_gba_screen_resources(&GameScreen::Pokedex);
                    self.pokedex_screen = PokedexScreenState::new_entry(
                        self.save_data.game_data.pokedex.clone(),
                        flow.receipt.species,
                        self.state.config.version,
                    );
                    self.pokedex_entry_cry_waiting = false;
                    self.state.transition_to(GameScreen::Pokedex);
                }
            }
            CapturePhase::Choice => {
                let choice = flow.choice.as_mut().expect("AskName owns its choice");
                if input.is_just_pressed(GbButton::Up) {
                    choice.move_up();
                } else if input.is_just_pressed(GbButton::Down) {
                    choice.move_down();
                }
                if ack {
                    if let Some(audio) = &self.audio {
                        audio.play_sfx(SfxId::PressAB);
                    }
                    if input.is_just_pressed(GbButton::B) || choice.selected == 1 {
                        keep = self.finish_capture_naming(&mut flow);
                    } else {
                        let mut naming = NamingScreenState::new(NamingScreenType::Pokemon);
                        naming.species = Some(flow.receipt.species);
                        flow.phase = CapturePhase::Naming;
                        flow.choice = None;
                        flow.dialogue = None;
                        flow.naming = Some(naming);
                        flow.flash_frames = NAMING_FLASH_FRAMES;
                        self.prepare_gba_screen_resources(&GameScreen::Overworld);
                    }
                }
            }
            CapturePhase::Naming => {
                if flow.flash_frames > 0 {
                    flow.flash_frames -= 1;
                } else {
                    let controls = NamingInput {
                        up: input.is_just_pressed(GbButton::Up),
                        down: input.is_just_pressed(GbButton::Down),
                        left: input.is_just_pressed(GbButton::Left),
                        right: input.is_just_pressed(GbButton::Right),
                        a: input.is_just_pressed(GbButton::A),
                        b: input.is_just_pressed(GbButton::B),
                        start: input.is_just_pressed(GbButton::Start),
                        select: input.is_just_pressed(GbButton::Select),
                    };
                    match flow.naming.as_mut().unwrap().update_frame(
                        controls,
                        self.state.config.language == pokered_core::game_state::Lang::Zh,
                    ) {
                        NamingScreenResult::Editing => {}
                        result => {
                            if let NamingScreenResult::Submitted(name) = result {
                                self.set_captured_nickname(&flow, &name);
                            }
                            flow.phase = CapturePhase::SubmitFlash;
                            flow.flash_frames = NAMING_FLASH_FRAMES;
                        }
                    }
                }
            }
            CapturePhase::SubmitFlash => {
                if flow.flash_frames > 0 {
                    flow.flash_frames -= 1;
                } else {
                    keep = self.finish_capture_naming(&mut flow);
                }
            }
            CapturePhase::Entry => {}
        }
        if keep {
            self.post_capture_flow = Some(flow);
        } else {
            self.prepare_gba_screen_resources(&GameScreen::Overworld);
            self.state.transition_to(GameScreen::Overworld);
            self.battle_vfx = BattleVisualEffects::default();
            self.battle_prev_message = None;
            self.overworld.synchronize_player_buttons(
                input.is_held(GbButton::A),
                input.is_held(GbButton::Start),
            );
            if let Some(audio) = &self.audio {
                let music = pokered_core::overworld::map_loading::get_map_music(
                    self.overworld.state.current_map,
                );
                if let Some(id) = MusicId::from_u8(music as u8) {
                    audio.play_music_with_fade(id, 8);
                }
            }
        }
    }

    #[inline(never)]
    pub(super) fn draw_post_capture(&mut self, fb: &mut FrameBuffer) {
        use pokered_data::ui_layout::schema::{DIALOG_DEFAULT_LAYOUT, YES_NO_DEFAULT_LAYOUT};
        use pokered_ui::backends::FrameBufferPainter;
        use pokered_ui::{menus, Ui};
        let flow = self.post_capture_flow.as_ref().unwrap();
        let language = self.state.config.language;
        if matches!(
            flow.phase,
            CapturePhase::NewDexMessage | CapturePhase::NewDexSound | CapturePhase::NewDexConfirm
        ) {
            crate::render::draw_battle(
                &self.battle,
                &mut self.resources,
                fb,
                &mut self.battle_vfx,
                language,
            );
        } else {
            fb.clear(Rgba::WHITE);
        }
        if flow.flash_frames > 0 {
            return;
        }
        if let Some(naming) = &flow.naming {
            crate::render::draw_naming_screen(naming, &mut self.resources, fb, language);
            return;
        }
        if let Some(dialogue) = &flow.dialogue {
            if let Some((first, second)) = dialogue.get_display_text() {
                let text = if second.is_empty() {
                    first
                } else {
                    format!("{first}\n{second}")
                };
                let arrow = dialogue.waiting_for_input()
                    && flow.phase != CapturePhase::Choice
                    && flow.phase != CapturePhase::Question
                    && (self.frame_count / 16) % 2 == 0;
                let mut painter = FrameBufferPainter::new(fb).with_lang(language);
                let mut ui = Ui::new(&mut painter);
                menus::dialog::draw_paginated(
                    &text,
                    arrow,
                    &DIALOG_DEFAULT_LAYOUT,
                    &mut ui,
                    language,
                );
                if let Some(choice) = &flow.choice {
                    menus::yes_no::draw(
                        &choice.options,
                        choice.selected,
                        &YES_NO_DEFAULT_LAYOUT,
                        &mut ui,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_core::battle::settlement::{BattleOutcome, BattleSettlement};
    use pokered_core::pokemon::stats::create_pokemon;
    use pokered_data::species::Species;

    fn mon(species: Species, level: u8) -> pokered_core::battle::state::Pokemon {
        create_pokemon(species, level, [0x9a, 0x78]).unwrap()
    }
    fn game(owned: bool, boxed: bool, met_bill: bool) -> PokemonGame {
        let mut g = PokemonGame::new_with_options(
            GameVersion::Red,
            None,
            None,
            None,
            false,
            None,
            false,
            true,
            #[cfg(feature = "debug-server")]
            None,
        );
        g.state.config.language = pokered_core::game_state::Lang::En;
        g.state.config.text_speed = pokered_core::game_state::TextSpeed::Fast;
        g.save_data.party.clear();
        for _ in 0..if boxed { 6 } else { 1 } {
            g.save_data.party.add(mon(Species::Bulbasaur, 12)).unwrap();
        }
        let mut old = mon(Species::Weedle, 9);
        old.set_nickname("OLD");
        g.save_data.current_box.deposit(old).unwrap();
        g.save_data.sync_current_box_to_storage();
        if owned {
            g.save_data.game_data.pokedex.set_owned(Species::Weedle);
        }
        if met_bill {
            g.overworld
                .set_event_flag_live(pokered_data::event_flags::EventFlag::EVENT_MET_BILL);
        }
        g.battle = BattleScreen::from_parties(
            true,
            &g.save_data.party.to_vec(),
            &[mon(Species::Weedle, 4)],
            None,
        );
        g.battle.player_bag = g.save_data.game_data.bag.clone();
        g.battle.captured_mon = Some(mon(Species::Weedle, 4));
        g.battle.settlement = Some(BattleSettlement {
            outcome: BattleOutcome::Captured,
            money_gained: 0,
            money_lost: 0,
            payday_bonus: 0,
            exp_entries: vec![],
            level_ups: vec![],
            evolutions: vec![],
        });
        g.state.screen = GameScreen::Battle;
        g.handle_transition(GameScreen::Overworld);
        g
    }
    fn idle(g: &mut PokemonGame, frames: usize) {
        for _ in 0..frames {
            g.update(&InputState::new());
        }
    }
    fn tap(g: &mut PokemonGame, button: GbButton) {
        let mut input = InputState::new();
        input.press(button);
        g.update(&input);
        g.update(&InputState::new());
    }
    fn choice(g: &mut PokemonGame) {
        for frame in 0..2000 {
            if g.post_capture_choice().is_some() {
                return;
            }
            if frame % 30 == 0 {
                tap(g, GbButton::A);
            } else {
                idle(g, 1);
            }
        }
        panic!("DATA caller did not return to AskName");
    }
    fn finish(g: &mut PokemonGame) {
        for frame in 0..1000 {
            if !g.post_capture_active() {
                return;
            }
            if frame % 30 == 0 {
                tap(g, GbButton::A);
            } else {
                idle(g, 1);
            }
        }
        panic!("capture continuation did not return");
    }

    #[test]
    fn actual_settlement_receipt_drives_owned_and_unowned_party_and_box_answers() {
        for owned in [false, true] {
            for boxed in [false, true] {
                for yes in [false, true] {
                    let mut g = game(owned, boxed, false);
                    let old = *g
                        .save_data
                        .current_box
                        .get(if boxed { 1 } else { 0 })
                        .unwrap();
                    assert_eq!(
                        g.post_capture_flow
                            .as_ref()
                            .unwrap()
                            .receipt
                            .new_pokedex_entry,
                        !owned
                    );
                    choice(&mut g);
                    assert_eq!(
                        g.post_capture_choice().unwrap().selected,
                        0,
                        "opening A does not choose YES"
                    );
                    if yes {
                        tap(&mut g, GbButton::A);
                        idle(&mut g, NAMING_FLASH_FRAMES as usize + 1);
                        tap(&mut g, GbButton::A);
                        tap(&mut g, GbButton::Start);
                        idle(&mut g, NAMING_FLASH_FRAMES as usize + 1);
                    } else {
                        tap(&mut g, GbButton::Down);
                        tap(&mut g, GbButton::A);
                    }
                    finish(&mut g);
                    let new = if boxed {
                        g.save_data.current_box.get(0).unwrap()
                    } else {
                        g.save_data.party.get(1).unwrap()
                    };
                    let mut buf = [0; pokered_core::battle::state::NAME_TEXT_BUF];
                    assert_eq!(new.display_name(&mut buf), if yes { "A" } else { "WEEDLE" });
                    assert_eq!(
                        *g.save_data
                            .current_box
                            .get(if boxed { 1 } else { 0 })
                            .unwrap(),
                        old
                    );
                    assert_eq!(g.save_data.party.count(), if boxed { 6 } else { 2 });
                    assert_eq!(g.save_data.current_box.count(), if boxed { 2 } else { 1 });
                    assert_eq!(
                        g.save_data.current_box,
                        *g.save_data.pc_storage.current_box()
                    );
                    assert_eq!(g.state.screen, GameScreen::Overworld);
                }
            }
        }
    }

    #[test]
    fn transferred_text_uses_the_saved_nickname_and_met_bill_recipient() {
        for met_bill in [false, true] {
            let mut g = game(true, true, met_bill);
            choice(&mut g);
            tap(&mut g, GbButton::A);
            idle(&mut g, 4);
            tap(&mut g, GbButton::A);
            tap(&mut g, GbButton::Start);
            idle(&mut g, 4);
            let flow = g.post_capture_flow.as_ref().unwrap();
            assert_eq!(flow.phase, CapturePhase::Transfer);
            let text = flow
                .dialogue
                .as_ref()
                .unwrap()
                .pages()
                .iter()
                .map(|p| format!("{} {}", p.line1, p.line2))
                .collect::<Vec<_>>()
                .join(" ");
            assert!(text.starts_with("A was"));
            assert!(text.contains(if met_bill {
                "BILL's PC!"
            } else {
                "someone's PC!"
            }));
        }
    }

    #[test]
    fn no_and_empty_name_leave_species_name_and_choice_does_not_consume_start() {
        for cancel_by_b in [false, true] {
            let mut g = game(true, false, false);
            choice(&mut g);
            if cancel_by_b {
                tap(&mut g, GbButton::B);
            } else {
                tap(&mut g, GbButton::A);
                idle(&mut g, 4);
                tap(&mut g, GbButton::Start);
                idle(&mut g, 4);
            }
            finish(&mut g);
            let mut buf = [0; pokered_core::battle::state::NAME_TEXT_BUF];
            assert_eq!(
                g.save_data.party.get(1).unwrap().display_name(&mut buf),
                "WEEDLE"
            );
        }
    }

    #[test]
    fn modal_pauses_field_clock_and_safari_zero_ball_announcement() {
        let mut g = game(true, true, false);
        choice(&mut g);
        g.overworld.start_safari_game();
        for _ in 0..30 {
            g.overworld.use_safari_ball();
        }
        let frame = g.overworld.frame_counter;
        idle(&mut g, 120);
        assert_eq!(g.overworld.frame_counter, frame);
        assert!(g.overworld.pending_dialogue.is_none());
        assert!(g.post_capture_choice().is_some());
    }

    #[test]
    fn snapshot_restore_and_title_reset_discard_capture_modal_and_naming_state() {
        let mut g = game(true, true, false);
        choice(&mut g);
        tap(&mut g, GbButton::A);
        g.handle_transition(GameScreen::TitleScreen);
        assert!(!g.post_capture_active());
        assert!(g.post_capture_choice().is_none());
        let mut g = game(true, false, false);
        assert!(
            g.agent_save_state().is_err(),
            "modal receipt must not be silently omitted from a snapshot"
        );
        g.clear_post_capture_continuation();
        g.state.screen = GameScreen::Overworld;
        let saved = g.agent_save_state().unwrap();
        g.begin_post_capture(CapturedMonReceipt {
            location: CapturedLocation::Party(1),
            species: Species::Weedle,
            new_pokedex_entry: false,
        });
        g.agent_restore_state(&saved).unwrap();
        assert!(!g.post_capture_active());
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    #[test]
    fn ordinary_save_load_discards_modal_without_old_battle_party_writeback() {
        let mut g = game(true, false, false);
        choice(&mut g);
        tap(&mut g, GbButton::A);
        let mut replacement = pokered_core::save::SaveData::new();
        replacement.party.add(mon(Species::Pikachu, 22)).unwrap();
        let bytes = pokered_core::save::sram_export::export_sram(&replacement);
        let path = std::env::temp_dir().join(format!(
            "pokered-post-capture-load-{}.sav",
            std::process::id()
        ));
        std::fs::write(&path, bytes).unwrap();
        g.load_from_path(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        idle(&mut g, 120);
        assert!(!g.post_capture_active());
        assert_eq!(g.save_data.party.count(), 1);
        assert_eq!(g.save_data.party.get(0).unwrap().species, Species::Pikachu);
    }
}
