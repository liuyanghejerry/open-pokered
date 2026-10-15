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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureBackdrop {
    Blank,
    PlayerHud,
    Battle,
}

pub(super) struct PostCaptureFlow {
    pub(super) receipt: CapturedMonReceipt,
    pub(super) phase: CapturePhase,
    pub(super) dialogue: Option<BedroomDialogue>,
    pub(super) choice: Option<PendingChoice>,
    pub(super) naming: Option<NamingScreenState>,
    flash_frames: u8,
    met_bill: bool,
    backdrop: CaptureBackdrop,
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
            // ShowPokedexData leaves a blank tilemap. Without DATA, AskName
            // clears the enemy HUD and sprites but retains the player's HUD.
            backdrop: if receipt.new_pokedex_entry {
                CaptureBackdrop::Blank
            } else {
                CaptureBackdrop::PlayerHud
            },
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
        self.battle_vfx.clear_ball_continuation();
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
                        // DisplayNamingScreen restores the saved battle tiles
                        // when it returns, including the enemy HUD/back picture.
                        if !flow.receipt.new_pokedex_entry {
                            flow.backdrop = CaptureBackdrop::Battle;
                        } else {
                            self.prepare_gba_screen_resources(&GameScreen::Overworld);
                        }
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
        let announcement = matches!(
            flow.phase,
            CapturePhase::NewDexMessage | CapturePhase::NewDexSound | CapturePhase::NewDexConfirm
        );
        let backdrop = if announcement {
            CaptureBackdrop::Battle
        } else {
            flow.backdrop
        };
        if backdrop != CaptureBackdrop::Blank && flow.naming.is_none() {
            crate::render::draw_battle(
                &self.battle,
                &mut self.resources,
                fb,
                &mut self.battle_vfx,
                language,
            );
            if backdrop == CaptureBackdrop::PlayerHud {
                // Keep the existing renderer's player HUD (name begins at
                // 72,56); ClearSprites/AskName remove the other scene regions.
                // No framebuffer snapshot is retained on the small-memory host.
                fb.fill_rect(0, 0, 160, 56, Rgba::WHITE);
                fb.fill_rect(0, 56, 72, 56, Rgba::WHITE);
            }
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
#[path = "post_capture_tests.rs"]
mod tests;
