//! Shared field LearnMove continuation. Rare Candy shows stats and checks
//! evolution before consuming; a machine consumes only on learned-text return.
use super::*;
use pokered_core::party_screen::PartyScreenPhase;
use pokered_core::pokemon::move_learning::{
    replace_move_guarded, try_learn_move, LearnMoveResult, ReplaceMoveError,
};
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RareCandyPhase {
    LevelAndStats,
    TryingToLearn(u8),
    ReplaceConfirm { yes: bool },
    ChooseMove,
    AbandonConfirm { yes: bool },
    Forgot { slot: usize },
    Learned(MoveId),
    DidNotLearn,
    Evolving,
}

#[derive(Clone, Copy)]
enum LearningOrigin {
    RareCandy,
    Machine { item: ItemId, learned: bool },
    PostBattleEvolution,
}

pub(super) struct RareCandyFlow {
    origin: LearningOrigin,
    party_index: usize,
    moves: VecDeque<(MoveId, bool)>,
    current_move: Option<MoveId>,
    pub(super) phase: RareCandyPhase,
    evolution_checked: bool,
    source_result: Option<pokered_core::pokemon::learn_terminal::LearnMoveTerminal>,
}

impl PokemonGame {
    pub(crate) fn rare_candy_phase(&self) -> Option<RareCandyPhase> {
        self.rare_candy_flow
            .as_ref()
            .filter(|flow| matches!(flow.origin, LearningOrigin::RareCandy))
            .map(|flow| flow.phase)
    }

    pub(crate) fn machine_learning_phase(&self) -> Option<RareCandyPhase> {
        self.rare_candy_flow
            .as_ref()
            .filter(|flow| matches!(flow.origin, LearningOrigin::Machine { .. }))
            .map(|flow| flow.phase)
    }

    pub(super) fn post_battle_learning_active(&self) -> bool {
        self.rare_candy_flow
            .as_ref()
            .is_some_and(|flow| matches!(flow.origin, LearningOrigin::PostBattleEvolution))
    }

    pub(super) fn begin_post_battle_evolution_learning(
        &mut self,
        party_index: usize,
        learning: Vec<pokered_core::pokemon::evolution::EvolutionMoveLearning>,
    ) {
        let moves = learning
            .into_iter()
            .filter_map(|entry| match entry.result {
                LearnMoveResult::AlreadyKnown => None,
                LearnMoveResult::Learned { .. } => Some((entry.move_id, true)),
                LearnMoveResult::MoveSlotsFull => Some((entry.move_id, false)),
            })
            .collect();
        let mut flow = RareCandyFlow {
            origin: LearningOrigin::PostBattleEvolution,
            party_index,
            moves,
            current_move: None,
            phase: RareCandyPhase::TryingToLearn(0),
            evolution_checked: true,
            source_result: None,
        };
        // Do not display an extra prompt for no new/already-known moves.
        if flow.moves.is_empty() {
            self.resume_post_battle_evolution();
            return;
        }
        self.handle_transition(GameScreen::PartyScreen);
        self.party_screen =
            PartyScreenState::new_for_move_choice(self.save_data.party.to_vec(), party_index);
        if self.candy_next_move(&mut flow) {
            self.rare_candy_flow = Some(flow);
        }
    }

    fn resume_post_battle_evolution(&mut self) {
        if let Some(anim) = self.evolution_anim.as_mut() {
            anim.resume_after_learning();
        }
        if self.evolution_anim.as_ref().is_some_and(|anim|
            anim.phase() == pokered_core::evolution_screen::EvolutionPhase::IsEvolving) {
            if let Some(owner) = self.overworld.source_foreground_mut() { owner.retire_history(); }
        }
        if self
            .evolution_anim
            .as_ref()
            .is_some_and(|anim| anim.is_done())
        {
            self.finish_evolution_cutscene();
        }
        // An empty learning list never left the movie. Only a real party
        // continuation needs a sub-screen return, never a second settlement.
        if self.state.screen == GameScreen::PartyScreen {
            self.handle_transition(GameScreen::Overworld);
        }
    }

    /// All learning origins share restore/reset cleanup.
    pub(crate) fn clear_rare_candy_continuation(&mut self) {
        self.rare_candy_flow = None;
    }

    pub(super) fn begin_rare_candy(&mut self, party_index: usize, level: u8, moves: Vec<MoveId>) {
        self.pending_bag_item = None;
        self.overworld.party_lead_level = self.save_data.party.leader_level();
        self.party_screen =
            PartyScreenState::new_for_move_choice(self.save_data.party.to_vec(), party_index);
        let flow = RareCandyFlow {
            origin: LearningOrigin::RareCandy,
            party_index,
            moves: moves.into_iter().map(|move_id| (move_id, false)).collect(),
            current_move: None,
            phase: RareCandyPhase::LevelAndStats,
            evolution_checked: false,
            source_result: None,
        };
        let name = self.candy_mon_name(&flow);
        self.candy_notice(format!("{name} grew to\nlevel {level}!"));
        self.rare_candy_flow = Some(flow);
    }

    pub(super) fn begin_machine_learning(
        &mut self,
        party_index: usize,
        item: ItemId,
        move_id: MoveId,
        already_learned: bool,
    ) {
        self.pending_bag_item = None;
        // Keep the machine's party mode/background through LearnMove, including
        // its compatibility hints; only the active continuation owns the input.
        self.party_screen = PartyScreenState::new_for_item(self.save_data.party.to_vec(), item);
        self.party_screen.set_cursor(party_index);
        self.party_screen.enter_move_choice();
        let mut flow = RareCandyFlow {
            origin: LearningOrigin::Machine {
                item,
                learned: false,
            },
            party_index,
            moves: [(move_id, already_learned)].into(),
            current_move: None,
            phase: RareCandyPhase::TryingToLearn(0),
            evolution_checked: true,
            source_result: None,
        };
        // apply_item_to_pokemon already writes a free slot. Full-slot machines
        // use the same Trying/Delete/Abandon/HM-guard flow as level-up learning.
        if self.candy_next_move(&mut flow) {
            self.rare_candy_flow = Some(flow);
        }
    }

    fn candy_mon_name(&self, flow: &RareCandyFlow) -> String {
        let mut buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
        self.save_data
            .party
            .get(flow.party_index)
            .map(|mon| mon.display_name(&mut buf).to_string())
            .unwrap_or_default()
    }

    fn candy_move_name(&self, flow: &RareCandyFlow) -> &'static str {
        pokered_data::lang_data::move_name(
            flow.current_move.unwrap_or(MoveId::None),
            self.state.config.language == pokered_core::game_state::Lang::Zh,
        )
    }

    fn candy_notice(&mut self, text: String) {
        self.party_screen
            .refresh_party(self.save_data.party.to_vec());
        self.party_screen.show_item_use_notice(
            self.localize_dialogue(&text),
            0,
            PartyNoticeReturn::Bag,
        );
    }

    fn candy_trying_page(&mut self, flow: &mut RareCandyFlow, page: u8) {
        // The existing earlier learn/forget menu is an unmodeled owner. Its
        // final real result PrintText will establish history; no physical-edge
        // snapshot here is promoted to a source foreground poll.
        if matches!(flow.origin, LearningOrigin::PostBattleEvolution) {
            if let Some(owner) = self.overworld.source_foreground_mut() {
                owner.retire_history();
            }
        }
        let name = self.candy_mon_name(flow);
        let move_name = self.candy_move_name(flow);
        let text = match page {
            0 => format!("{name} is\ntrying to learn\n{move_name}!"),
            1 => format!("But, {name}\ncan't learn more\nthan 4 moves!"),
            _ => format!("Delete an older\nmove to make room\nfor {move_name}?"),
        };
        flow.phase = if page >= 2 {
            RareCandyPhase::ReplaceConfirm { yes: true }
        } else {
            RareCandyPhase::TryingToLearn(page)
        };
        self.candy_notice(text);
    }

    fn candy_abandon_prompt(&mut self, flow: &mut RareCandyFlow) {
        flow.phase = RareCandyPhase::AbandonConfirm { yes: true };
        self.candy_notice(format!("Abandon learning\n{}?", self.candy_move_name(flow)));
    }

    fn candy_learned_notice(&mut self, flow: &mut RareCandyFlow) {
        if let LearningOrigin::Machine { item, .. } = flow.origin {
            flow.origin = LearningOrigin::Machine {
                item,
                learned: true,
            };
        }
        flow.phase = RareCandyPhase::Learned(flow.current_move.unwrap());
        let text = format!(
            "{} learned\n{}!",
            self.candy_mon_name(flow),
            self.candy_move_name(flow)
        );
        if matches!(flow.origin, LearningOrigin::PostBattleEvolution) {
            flow.source_result = Some(
                pokered_core::pokemon::learn_terminal::LearnMoveTerminal::learned_text(
                    self.localize_dialogue(&text),
                    self.state.config.text_speed.delay_frames() as u8,
                ),
            );
            self.candy_notice(String::new());
            return;
        }
        self.candy_notice(text);
        if let Some(audio) = &self.audio {
            audio.play_sfx(SfxId::GetItem1);
        }
    }

    /// Returns true while this continuation still owns the selected party.
    fn candy_next_move(&mut self, flow: &mut RareCandyFlow) -> bool {
        while let Some((move_id, already_learned)) = flow.moves.pop_front() {
            flow.current_move = Some(move_id);
            if already_learned {
                self.candy_learned_notice(flow);
                return true;
            }
            let result = self
                .save_data
                .party
                .get_mut(flow.party_index)
                .map(|mon| try_learn_move(mon, move_id));
            match result {
                Some(LearnMoveResult::AlreadyKnown) | None => continue,
                Some(LearnMoveResult::Learned { .. }) => self.candy_learned_notice(flow),
                Some(LearnMoveResult::MoveSlotsFull) => self.candy_trying_page(flow, 0),
            }
            return true;
        }
        flow.current_move = None;
        if matches!(flow.origin, LearningOrigin::PostBattleEvolution) {
            self.resume_post_battle_evolution();
            return false;
        }
        if let LearningOrigin::Machine { item, learned } = flow.origin {
            // ItemUseTMHM tests LearnMove's success flag only after its learned
            // text (and sound) return. Abandon never consumes, nor does any HM.
            if learned
                && pokered_core::items::bag_use::machine_of(item)
                    .is_some_and(|machine| machine.is_tm())
            {
                let _ = self.save_data.game_data.bag.remove_item(item, 1);
            }
            self.handle_transition(GameScreen::Bag);
            return false;
        }
        if !flow.evolution_checked {
            flow.evolution_checked = true;
            let evolution = self.save_data.party.get(flow.party_index).and_then(|mon| {
                pokered_core::pokemon::evolution::check_evolution(
                    mon,
                    pokered_core::pokemon::evolution::EvolutionTrigger::LevelUp,
                )
                .map(|to| (mon.species, to))
            });
            if let Some((from, to)) = evolution {
                flow.phase = RareCandyPhase::Evolving;
                self.queue_item_evolution(flow.party_index, from, to, None, false);
                self.handle_transition(GameScreen::Overworld);
                return true;
            }
        }
        // Declining/cancelling learning or evolution still consumes the candy,
        // at the original final RemoveUsedItem, never once per dialog/page.
        let _ = self
            .save_data
            .game_data
            .bag
            .remove_item(ItemId::RareCandy, 1);
        self.handle_transition(GameScreen::Bag);
        false
    }

    pub(super) fn update_rare_candy(&mut self, input: &InputState) {
        let Some(mut flow) = self.rare_candy_flow.take() else {
            return;
        };
        if let Some(work) = flow.source_result.as_mut() {
            let Some(owner) = self.overworld.source_foreground_mut() else {
                self.overworld.block_source_foreground(
                    pokered_core::text::terminal_input::TerminalInputError::MissingLearningOwner,
                );
                self.rare_candy_flow = Some(flow);
                return;
            };
            let step = work.tick_with_input(
                input.raw_current(),
                self.audio
                    .as_ref()
                    .is_some_and(|audio| audio.is_sfx_playing()),
                owner,
            );
            let visible = work.visible_text();
            if let Some(error) = step.input_error {
                self.overworld.block_source_foreground(error);
            }
            // Display is a projection of the work cursor, never a second clock.
            self.candy_notice(visible);
            if let Some(audio) = &self.audio {
                if step.learned_sound {
                    audio.play_sfx(SfxId::GetItem1);
                }
                if step.press_ab {
                    audio.play_sfx(SfxId::PressAB);
                }
            }
            let keep = if step.complete {
                flow.source_result = None;
                self.candy_next_move(&mut flow)
            } else {
                true
            };
            if keep {
                self.rare_candy_flow = Some(flow);
            }
            return;
        }
        if matches!(flow.origin, LearningOrigin::PostBattleEvolution) {
            if let Some(owner) = self.overworld.source_foreground_mut() {
                owner.read_raw(input.raw_current());
            }
        }
        let controls = PartyScreenInput {
            up: input.is_just_pressed(GbButton::Up),
            down: input.is_just_pressed(GbButton::Down),
            a: input.is_just_pressed(GbButton::A),
            b: input.is_just_pressed(GbButton::B),
        };
        let ack = controls.a || controls.b;
        let mut keep = true;
        match flow.phase {
            RareCandyPhase::LevelAndStats | RareCandyPhase::DidNotLearn => {
                if ack {
                    keep = self.candy_next_move(&mut flow);
                }
            }
            RareCandyPhase::Learned(_) => {
                let sound_busy = self
                    .audio
                    .as_ref()
                    .is_some_and(|audio| audio.is_sfx_playing());
                if ack && !sound_busy {
                    keep = self.candy_next_move(&mut flow);
                }
            }
            RareCandyPhase::TryingToLearn(page) => {
                if ack {
                    self.candy_trying_page(&mut flow, page + 1);
                }
            }
            RareCandyPhase::ReplaceConfirm { mut yes }
            | RareCandyPhase::AbandonConfirm { mut yes } => {
                let abandoning = matches!(flow.phase, RareCandyPhase::AbandonConfirm { .. });
                if controls.up {
                    yes = true;
                }
                if controls.down || controls.b {
                    yes = false;
                }
                flow.phase = if abandoning {
                    RareCandyPhase::AbandonConfirm { yes }
                } else {
                    RareCandyPhase::ReplaceConfirm { yes }
                };
                if ack {
                    if abandoning {
                        if yes {
                            flow.phase = RareCandyPhase::DidNotLearn;
                            let text = self.localize_dialogue(&format!(
                                "{}\ndid not learn\n{}!",
                                self.candy_mon_name(&flow),
                                self.candy_move_name(&flow)
                            ));
                            if matches!(flow.origin, LearningOrigin::PostBattleEvolution) {
                                // This explicit DidNotLearn descriptor owns CONT;
                                // translation resolves text, never command identity.
                                let (intro, final_line) =
                                    text.rsplit_once('\n').expect("authored DidNotLearn CONT");
                                let retained = intro.rsplit('\n').next().unwrap_or(intro);
                                flow.source_result = Some(pokered_core::pokemon::learn_terminal::LearnMoveTerminal::abandoned_parts(
                                    intro.into(), retained.into(), final_line.into(),
                                    self.state.config.text_speed.delay_frames() as u8));
                                self.candy_notice(String::new());
                            } else {
                                self.candy_notice(text);
                            }
                        } else {
                            self.candy_trying_page(&mut flow, 0);
                        }
                    } else if yes {
                        flow.phase = RareCandyPhase::ChooseMove;
                        self.party_screen.enter_move_choice();
                    } else {
                        self.candy_abandon_prompt(&mut flow);
                    }
                }
            }
            RareCandyPhase::ChooseMove => {
                let action = self.party_screen.update_frame(controls);
                if let PartyScreenAction::MoveForgetChosen { slot, .. } = action {
                    let old_move = self
                        .save_data
                        .party
                        .get(flow.party_index)
                        .map(|mon| mon.moves[slot]);
                    if old_move.is_some_and(pokered_core::pokemon::move_learning::is_hm_move) {
                        self.party_screen.enter_move_choice();
                        self.party_screen.show_move_choice_notice(
                            self.localize_dialogue("HM techniques\ncan't be deleted!"),
                        );
                    } else if let Some(old_move) = old_move {
                        flow.phase = RareCandyPhase::Forgot { slot };
                        self.candy_notice(format!(
                            "1, 2 and... Poof!\n{} forgot\n{}!\nAnd...",
                            self.candy_mon_name(&flow),
                            pokered_data::lang_data::move_name(
                                old_move,
                                self.state.config.language == pokered_core::game_state::Lang::Zh
                            )
                        ));
                        if let Some(audio) = &self.audio {
                            audio.play_sfx(SfxId::Swap);
                        }
                    }
                } else if self.party_screen.phase() == PartyScreenPhase::Browsing && ack {
                    self.candy_abandon_prompt(&mut flow);
                }
            }
            RareCandyPhase::Forgot { slot } => {
                if ack {
                    let result = self
                        .save_data
                        .party
                        .get_mut(flow.party_index)
                        .map(|mon| replace_move_guarded(mon, slot, flow.current_move.unwrap()));
                    match result {
                        Some(Ok(_)) => self.candy_learned_notice(&mut flow),
                        Some(Err(ReplaceMoveError::HmCantDelete)) => {
                            flow.phase = RareCandyPhase::ChooseMove;
                            self.party_screen.enter_move_choice();
                            self.party_screen.show_move_choice_notice(
                                self.localize_dialogue("HM techniques\ncan't be deleted!"),
                            );
                        }
                        _ => self.candy_abandon_prompt(&mut flow),
                    }
                }
            }
            RareCandyPhase::Evolving => {}
        }
        if keep {
            self.rare_candy_flow = Some(flow);
        }
    }

    pub(super) fn apply_rare_candy_evolution(
        &mut self,
        outcome: &pokered_core::evolution_screen::EvolutionOutcome,
    ) -> bool {
        let Some(flow) = self.rare_candy_flow.as_mut() else {
            return false;
        };
        if flow.phase != RareCandyPhase::Evolving || flow.party_index != outcome.party_index {
            return false;
        }
        if let Some(mon) = self.save_data.party.get_mut(outcome.party_index) {
            let candidate = pokered_core::pokemon::move_learning::first_move_at_level_for(
                outcome.to,
                mon.level,
                &mon.moves,
            );
            pokered_core::pokemon::evolution::finalize_evolution(
                mon,
                &mut self.save_data.game_data.pokedex,
                outcome.to,
            );
            // Species-change learning is applied by the existing evolution
            // helper (`LearnMoveFromLevelUp`: first matching level entry only).
            if let Some(id) = candidate {
                if mon.moves.contains(&id) {
                    flow.moves.push_back((id, true));
                }
            }
        }
        true
    }

    pub(super) fn resume_rare_candy_after_evolution(&mut self) -> bool {
        if self.rare_candy_phase() != Some(RareCandyPhase::Evolving) {
            return false;
        }
        let mut flow = self.rare_candy_flow.take().unwrap();
        // Reuse evolution's deferred full-slot result without replaying either
        // the level-up or evolution; finish the item after that learning choice.
        if let Some((party_index, move_id)) = self.pending_evolve_move_replace.take() {
            if party_index == flow.party_index {
                flow.moves.push_back((move_id, false));
            }
        }
        self.handle_transition(GameScreen::PartyScreen);
        self.party_screen =
            PartyScreenState::new_for_move_choice(self.save_data.party.to_vec(), flow.party_index);
        if self.candy_next_move(&mut flow) {
            self.rare_candy_flow = Some(flow);
        }
        true
    }

    pub(super) fn draw_rare_candy(&mut self, fb: &mut FrameBuffer) {
        use pokered_ui::backends::FrameBufferPainter;
        use pokered_ui::{menus, InkColor, TileRect, Ui};
        let flow = self.rare_candy_flow.as_ref().unwrap();
        let lang = self.state.config.language;
        crate::render::draw_party_screen_base(
            &self.party_screen,
            self.resources.as_mut(),
            self.frame_count,
            fb,
            lang,
        );
        let mut painter = FrameBufferPainter::new(fb).with_lang(lang);
        let mut ui = Ui::new(&mut painter);
        if let Some(mon) = self
            .save_data
            .party
            .get(flow.party_index)
            .filter(|_| matches!(flow.origin, LearningOrigin::RareCandy))
        {
            ui.text_box(
                TileRect::new(10, 3, 10, 10),
                InkColor::Black,
                true,
                |frame| {
                    for (row, (label, value)) in [
                        ("ATTACK", mon.attack),
                        ("DEFENSE", mon.defense),
                        ("SPEED", mon.speed),
                        ("SPECIAL", mon.special),
                    ]
                    .iter()
                    .enumerate()
                    {
                        frame.label(
                            0,
                            row as u32 * 2,
                            pokered_data::lang_data::ui_label(
                                label,
                                lang == pokered_core::game_state::Lang::Zh,
                            ),
                            InkColor::Black,
                        );
                        let value = value.to_string();
                        frame.label(
                            7u32.saturating_sub(value.len() as u32),
                            row as u32 * 2 + 1,
                            &value,
                            InkColor::Black,
                        );
                    }
                },
            );
        }
        menus::party::draw_overlay(&self.party_screen, &mut ui, lang);
        if let RareCandyPhase::ReplaceConfirm { yes } | RareCandyPhase::AbandonConfirm { yes } =
            flow.phase
        {
            let options = vec![
                pokered_data::lang_data::ui_label(
                    "YES",
                    lang == pokered_core::game_state::Lang::Zh,
                )
                .to_string(),
                pokered_data::lang_data::ui_label("NO", lang == pokered_core::game_state::Lang::Zh)
                    .to_string(),
            ];
            menus::yes_no::draw(
                &options,
                if yes { 0 } else { 1 },
                &pokered_data::ui_layout::schema::YES_NO_DEFAULT_LAYOUT,
                &mut ui,
            );
        }
    }
}
