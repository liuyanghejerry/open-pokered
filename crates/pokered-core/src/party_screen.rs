use crate::battle::state::Pokemon;
use crate::overworld::hm_effects;
use pokered_data::moves::MoveId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartyScreenInput {
    pub up: bool,
    pub down: bool,
    pub a: bool,
    pub b: bool,
}

impl PartyScreenInput {
    pub fn none() -> Self {
        Self {
            up: false,
            down: false,
            a: false,
            b: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyScreenPhase {
    Browsing,
    /// Cursor into the dynamic action menu: any usable field moves of the
    /// selected mon come first (Gen-1 order), then STATS / SWITCH / CANCEL.
    ActionMenu { cursor: u8 },
    SwitchTarget { source_index: usize },
    /// "Which move should be forgotten?" — cursor over the selected mon's
    /// known moves, with a trailing CANCEL row (TM/HM replace-move flow).
    ChooseMove { cursor: u8 },
}

/// Why the party screen was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyScreenMode {
    /// Opened from the start menu: STATS / SWITCH / field moves.
    Normal,
    /// Opened from the bag to apply `item` to a chosen party member:
    /// pressing A on a Pokémon applies the item directly (no action menu),
    /// matching the Gen-1 party menu shown for medicine / stones / TM-HM.
    UseItem(pokered_data::items::ItemId),
    /// SOFTBOILED target pick: the field move was chosen for `usize`, the
    /// party menu reopens (Gen-1 `.softboiled` → `GoBackToPartyMenu`) and A
    /// on any other member heals it; A on the user itself is ignored (the
    /// original loops `ItemUseMedicine` until a different mon is chosen).
    SoftboiledTarget(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyScreenAction {
    Active,
    /// Party index for the caller to open a stats/details screen.
    ShowStats(usize),
    /// Player chose a field move from the party menu. The caller dispatches
    /// the overworld effect (badge-gated, per Gen-1 start_sub_menus.asm).
    UseFieldMove { party_index: usize, move_id: MoveId },
    /// In `UseItem` mode, the player picked a party member to apply the
    /// pending bag item to. The caller runs the item effect.
    ApplyItem { party_index: usize },
    /// In `SoftboiledTarget` mode, the player picked the member to heal. The
    /// caller runs the SOFTBOILED heal (user loses 1/5 max HP, target gains
    /// it) and shows the result text on the field.
    SoftboiledTargetChosen { target_index: usize },
    /// In the `ChooseMove` phase (TM/HM with a full moveset), the player
    /// picked the move slot to forget. The caller performs the replacement.
    MoveForgetChosen { party_index: usize, slot: usize },
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct PartyScreenState {
    party: Vec<Pokemon>,
    cursor: usize,
    phase: PartyScreenPhase,
    mode: PartyScreenMode,
    pending_swap: Option<(usize, usize)>,
}

impl PartyScreenState {
    pub fn new(party: Vec<Pokemon>) -> Self {
        Self {
            party,
            cursor: 0,
            phase: PartyScreenPhase::Browsing,
            mode: PartyScreenMode::Normal,
            pending_swap: None,
        }
    }

    /// Party screen opened from the bag to apply `item` to a chosen member.
    pub fn new_for_item(party: Vec<Pokemon>, item: pokered_data::items::ItemId) -> Self {
        Self {
            mode: PartyScreenMode::UseItem(item),
            ..Self::new(party)
        }
    }

    /// Party screen reopened to pick the SOFTBOILED target: the player chose
    /// the field move for `user_index` (Gen-1 `.softboiled` → the pseudo-item
    /// `GoBackToPartyMenu`); A on a different member heals it.
    pub fn new_for_softboiled_target(party: Vec<Pokemon>, user_index: usize) -> Self {
        Self {
            mode: PartyScreenMode::SoftboiledTarget(user_index),
            ..Self::new(party)
        }
    }

    /// Party screen opened straight into the "which move should be
    /// forgotten?" phase for `party_index` — used by the post-evolution
    /// full-moveset learn flow (Gen-1 `LearnMove`'s move-list menu).
    pub fn new_for_move_choice(party: Vec<Pokemon>, party_index: usize) -> Self {
        let mut s = Self::new(party);
        s.cursor = party_index.min(s.party.len().saturating_sub(1));
        s.phase = PartyScreenPhase::ChooseMove { cursor: 0 };
        s
    }

    pub fn mode(&self) -> PartyScreenMode {
        self.mode
    }

    pub fn party(&self) -> &[Pokemon] {
        &self.party
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn phase(&self) -> PartyScreenPhase {
        self.phase
    }

    pub fn selected_pokemon(&self) -> Option<&Pokemon> {
        self.party.get(self.cursor)
    }

    pub fn party_member(&self, idx: usize) -> Option<&Pokemon> {
        self.party.get(idx)
    }

    /// Returns Some((a, b)) if a swap was performed since last call, then clears the flag.
    pub fn take_pending_swap(&mut self) -> Option<(usize, usize)> {
        self.pending_swap.take()
    }

    /// Field moves listed for the currently selected mon (Gen-1
    /// GetMonFieldMoves): every HM/field move it knows, in moveset order.
    /// Badge gating happens when the move is *chosen*, not on display.
    pub fn selected_field_moves(&self) -> Vec<MoveId> {
        self.selected_pokemon()
            .map(|p| hm_effects::field_moves_of(&p.moves))
            .unwrap_or_default()
    }

    pub fn update_frame(&mut self, input: PartyScreenInput) -> PartyScreenAction {
        match self.phase {
            PartyScreenPhase::Browsing => self.update_browsing(input),
            PartyScreenPhase::ActionMenu { cursor } => self.update_action_menu(input, cursor),
            PartyScreenPhase::SwitchTarget { source_index } => {
                self.update_switch_target(input, source_index)
            }
            PartyScreenPhase::ChooseMove { cursor } => self.update_choose_move(input, cursor),
        }
    }

    /// Enter the "which move should be forgotten?" phase for the currently
    /// selected mon (TM/HM teaching when its moveset is full).
    pub fn enter_move_choice(&mut self) {
        self.phase = PartyScreenPhase::ChooseMove { cursor: 0 };
    }

    /// Known (non-empty) moves of the currently selected mon, in slot order.
    pub fn selected_known_moves(&self) -> Vec<MoveId> {
        self.selected_pokemon()
            .map(|p| p.moves.iter().copied().filter(|&m| m != MoveId::None).collect())
            .unwrap_or_default()
    }

    fn update_browsing(&mut self, input: PartyScreenInput) -> PartyScreenAction {
        if self.party.is_empty() {
            if input.a || input.b {
                return PartyScreenAction::Cancelled;
            }
            return PartyScreenAction::Active;
        }

        let count = self.party.len();

        if input.up && self.cursor > 0 {
            self.cursor -= 1;
        } else if input.down && self.cursor < count.saturating_sub(1) {
            self.cursor += 1;
        }

        if input.a {
            match self.mode {
                // Bag item use: A applies the pending item directly (Gen-1
                // medicine/stone/TM-HM party menu has no STATS/SWITCH submenu).
                PartyScreenMode::UseItem(_) => {
                    return PartyScreenAction::ApplyItem {
                        party_index: self.cursor,
                    };
                }
                // SOFTBOILED target pick: A on the user itself is ignored —
                // the original loops `ItemUseMedicine` (`jr z, ItemUseMedicine`,
                // item_effects.asm:854-856) until a different mon is chosen.
                PartyScreenMode::SoftboiledTarget(user) => {
                    if self.cursor != user {
                        self.phase = PartyScreenPhase::Browsing;
                        return PartyScreenAction::SoftboiledTargetChosen {
                            target_index: self.cursor,
                        };
                    }
                    return PartyScreenAction::Active;
                }
                PartyScreenMode::Normal => {}
            }
            self.phase = PartyScreenPhase::ActionMenu { cursor: 0 };
            return PartyScreenAction::Active;
        }

        if input.b {
            // SOFTBOILED target pick: B abandons the heal and returns to the
            // normal party menu (`.canceledItemUse` → the StartMenu_Pokemon
            // loop in the original).
            if matches!(self.mode, PartyScreenMode::SoftboiledTarget(_)) {
                self.mode = PartyScreenMode::Normal;
                self.phase = PartyScreenPhase::Browsing;
                return PartyScreenAction::Active;
            }
            return PartyScreenAction::Cancelled;
        }

        PartyScreenAction::Active
    }

    fn update_choose_move(&mut self, input: PartyScreenInput, mut cursor: u8) -> PartyScreenAction {
        // Rows: one per known move, plus a trailing CANCEL row.
        let num_moves = self.selected_known_moves().len() as u8;
        let max_cursor = num_moves;

        if input.up && cursor > 0 {
            cursor -= 1;
        } else if input.down && cursor < max_cursor {
            cursor += 1;
        }

        if input.a {
            self.phase = PartyScreenPhase::Browsing;
            if cursor < num_moves {
                return PartyScreenAction::MoveForgetChosen {
                    party_index: self.cursor,
                    slot: cursor as usize,
                };
            }
            // CANCEL row: give up teaching (the item is not used).
            return PartyScreenAction::Active;
        }

        if input.b {
            self.phase = PartyScreenPhase::Browsing;
            return PartyScreenAction::Active;
        }

        self.phase = PartyScreenPhase::ChooseMove { cursor };
        PartyScreenAction::Active
    }

    fn update_action_menu(
        &mut self,
        input: PartyScreenInput,
        mut menu_cursor: u8,
    ) -> PartyScreenAction {
        let field_moves = self.selected_field_moves();
        let num_field_moves = field_moves.len() as u8;
        // Menu layout: [field moves..., STATS, SWITCH, CANCEL].
        let max_cursor = num_field_moves + 2;

        if input.up && menu_cursor > 0 {
            menu_cursor -= 1;
        } else if input.down && menu_cursor < max_cursor {
            menu_cursor += 1;
        }

        if input.a {
            if menu_cursor < num_field_moves {
                let move_id = field_moves[menu_cursor as usize];
                self.phase = PartyScreenPhase::Browsing;
                return PartyScreenAction::UseFieldMove {
                    party_index: self.cursor,
                    move_id,
                };
            }
            match menu_cursor - num_field_moves {
                0 => {
                    self.phase = PartyScreenPhase::Browsing;
                    return PartyScreenAction::ShowStats(self.cursor);
                }
                1 => {
                    self.phase =
                        PartyScreenPhase::SwitchTarget { source_index: self.cursor };
                    return PartyScreenAction::Active;
                }
                2 => {
                    self.phase = PartyScreenPhase::Browsing;
                    return PartyScreenAction::Active;
                }
                _ => unreachable!(),
            }
        }

        if input.b {
            self.phase = PartyScreenPhase::Browsing;
            return PartyScreenAction::Active;
        }

        self.phase = PartyScreenPhase::ActionMenu {
            cursor: menu_cursor,
        };
        PartyScreenAction::Active
    }

    fn update_switch_target(
        &mut self,
        input: PartyScreenInput,
        source_index: usize,
    ) -> PartyScreenAction {
        let count = self.party.len();

        if input.up && self.cursor > 0 {
            self.cursor -= 1;
        } else if input.down && self.cursor < count.saturating_sub(1) {
            self.cursor += 1;
        }

        if input.a {
            if self.cursor != source_index {
                self.party.swap(source_index, self.cursor);
                self.pending_swap = Some((source_index, self.cursor));
            } else {
                // Pressing A on the same slot cancels the swap so the player
                // is never stuck with a single-mon party in SwitchTarget.
                self.cursor = source_index;
            }
            self.phase = PartyScreenPhase::Browsing;
            return PartyScreenAction::Active;
        }

        if input.b {
            self.cursor = source_index;
            self.phase = PartyScreenPhase::Browsing;
            return PartyScreenAction::Active;
        }

        PartyScreenAction::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_data::species::Species;

    fn make_test_pokemon(species: Species) -> Pokemon {
        crate::pokemon::stats::create_pokemon(species, 5, [0xFF, 0xFF]).unwrap()
    }

    fn party_of(n: usize) -> Vec<Pokemon> {
        let species = [Species::Bulbasaur, Species::Charmander, Species::Squirtle];
        species
            .iter()
            .cycle()
            .take(n)
            .map(|&s| make_test_pokemon(s))
            .collect()
    }

    #[test]
    fn empty_party_cancel() {
        let mut screen = PartyScreenState::new(vec![]);
        assert_eq!(
            screen.update_frame(PartyScreenInput {
                b: true,
                ..PartyScreenInput::none()
            }),
            PartyScreenAction::Cancelled
        );
    }

    #[test]
    fn single_pokemon_a_opens_action_menu() {
        let party = vec![make_test_pokemon(Species::Bulbasaur)];
        let mut screen = PartyScreenState::new(party);
        let result =
            screen.update_frame(PartyScreenInput {
                a: true,
                ..PartyScreenInput::none()
            });
        assert_eq!(result, PartyScreenAction::Active);
        assert_eq!(screen.phase(), PartyScreenPhase::ActionMenu { cursor: 0 });
    }

    #[test]
    fn cursor_navigation() {
        let party = vec![
            make_test_pokemon(Species::Bulbasaur),
            make_test_pokemon(Species::Charmander),
            make_test_pokemon(Species::Squirtle),
        ];
        let mut screen = PartyScreenState::new(party);

        assert_eq!(screen.cursor(), 0);

        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.cursor(), 1);

        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.cursor(), 2);

        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.cursor(), 2);

        screen.update_frame(PartyScreenInput {
            up: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.cursor(), 1);
    }

    #[test]
    fn select_stats_returns_show_stats_with_correct_index() {
        let party = vec![
            make_test_pokemon(Species::Bulbasaur),
            make_test_pokemon(Species::Charmander),
        ];
        let mut screen = PartyScreenState::new(party);

        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(
            screen.update_frame(PartyScreenInput {
                a: true,
                ..PartyScreenInput::none()
            }),
            PartyScreenAction::Active
        );
        assert_eq!(
            screen.update_frame(PartyScreenInput {
                a: true,
                ..PartyScreenInput::none()
            }),
            PartyScreenAction::ShowStats(1)
        );
    }

    #[test]
    fn cancel_returns_cancelled() {
        let party = vec![make_test_pokemon(Species::Bulbasaur)];
        let mut screen = PartyScreenState::new(party);
        assert_eq!(
            screen.update_frame(PartyScreenInput {
                b: true,
                ..PartyScreenInput::none()
            }),
            PartyScreenAction::Cancelled
        );
    }


    #[test]
    fn open_action_menu_with_a_in_browsing() {
        let party = party_of(3);
        let mut screen = PartyScreenState::new(party);
        let result =
            screen.update_frame(PartyScreenInput {
                a: true,
                ..PartyScreenInput::none()
            });
        assert_eq!(result, PartyScreenAction::Active);
        assert_eq!(screen.phase(), PartyScreenPhase::ActionMenu { cursor: 0 });
        assert_eq!(screen.cursor(), 0);
    }

    #[test]
    fn cancel_action_menu_with_b_returns_to_browsing() {
        let party = party_of(2);
        let mut screen = PartyScreenState::new(party);
        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        let result =
            screen.update_frame(PartyScreenInput {
                b: true,
                ..PartyScreenInput::none()
            });
        assert_eq!(result, PartyScreenAction::Active);
        assert_eq!(screen.phase(), PartyScreenPhase::Browsing);
        assert_eq!(screen.cursor(), 0);
    }

    #[test]
    fn select_stats_yields_show_stats() {
        let party = party_of(2);
        let mut screen = PartyScreenState::new(party);
        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        let result =
            screen.update_frame(PartyScreenInput {
                a: true,
                ..PartyScreenInput::none()
            });
        assert_eq!(result, PartyScreenAction::ShowStats(1));
        assert_eq!(screen.phase(), PartyScreenPhase::Browsing);
    }

    #[test]
    fn select_cancel_from_action_menu_returns_to_browsing() {
        let party = party_of(2);
        let mut screen = PartyScreenState::new(party);
        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        let result =
            screen.update_frame(PartyScreenInput {
                a: true,
                ..PartyScreenInput::none()
            });
        assert_eq!(result, PartyScreenAction::Active);
        assert_eq!(screen.phase(), PartyScreenPhase::Browsing);
        assert_eq!(screen.cursor(), 0);
    }

    #[test]
    fn switch_selects_target_and_swaps() {
        let party = party_of(3);
        let mut screen = PartyScreenState::new(party.clone());
        let species_before: Vec<Species> =
            screen.party().iter().map(|p| p.species).collect();

        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        let result =
            screen.update_frame(PartyScreenInput {
                a: true,
                ..PartyScreenInput::none()
            });
        assert_eq!(result, PartyScreenAction::Active);
        assert_eq!(
            screen.phase(),
            PartyScreenPhase::SwitchTarget { source_index: 0 }
        );

        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.cursor(), 2);

        let result =
            screen.update_frame(PartyScreenInput {
                a: true,
                ..PartyScreenInput::none()
            });
        assert_eq!(result, PartyScreenAction::Active);
        assert_eq!(screen.phase(), PartyScreenPhase::Browsing);
        assert_eq!(screen.cursor(), 2);

        let species_after: Vec<Species> =
            screen.party().iter().map(|p| p.species).collect();
        assert_ne!(species_before, species_after);
        assert_eq!(species_before[0], species_after[2]);
        assert_eq!(species_before[2], species_after[0]);

        let swap = screen.take_pending_swap();
        assert_eq!(swap, Some((0, 2)));
        assert_eq!(screen.take_pending_swap(), None);
    }

    #[test]
    fn switch_a_on_same_slot_cancels() {
        let party = party_of(2);
        let mut screen = PartyScreenState::new(party);
        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        let result =
            screen.update_frame(PartyScreenInput {
                a: true,
                ..PartyScreenInput::none()
            });
        assert_eq!(result, PartyScreenAction::Active);
        assert_eq!(screen.phase(), PartyScreenPhase::Browsing);
        assert_eq!(screen.cursor(), 0);
        assert_eq!(screen.take_pending_swap(), None);
    }

    #[test]
    fn cancel_switch_with_b_returns_to_browsing_no_mutation() {
        let party = party_of(3);
        let mut screen = PartyScreenState::new(party.clone());
        let original: Vec<Species> =
            screen.party().iter().map(|p| p.species).collect();

        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.cursor(), 1);

        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        screen.update_frame(PartyScreenInput {
            up: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.cursor(), 0);

        let result =
            screen.update_frame(PartyScreenInput {
                b: true,
                ..PartyScreenInput::none()
            });
        assert_eq!(result, PartyScreenAction::Active);
        assert_eq!(screen.phase(), PartyScreenPhase::Browsing);
        assert_eq!(screen.cursor(), 1);

        assert_eq!(screen.take_pending_swap(), None);

        let after: Vec<Species> =
            screen.party().iter().map(|p| p.species).collect();
        assert_eq!(original, after);
    }

    #[test]
    fn action_menu_cursor_wraps_correctly() {
        let party = party_of(1);
        let mut screen = PartyScreenState::new(party);
        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.phase(), PartyScreenPhase::ActionMenu { cursor: 0 });

        screen.update_frame(PartyScreenInput {
            up: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.phase(), PartyScreenPhase::ActionMenu { cursor: 0 });

        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.phase(), PartyScreenPhase::ActionMenu { cursor: 1 });

        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.phase(), PartyScreenPhase::ActionMenu { cursor: 2 });

        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.phase(), PartyScreenPhase::ActionMenu { cursor: 2 });
    }

    #[test]
    fn switch_target_empty_party_nop() {
        let mut screen = PartyScreenState::new(vec![]);
        screen.phase = PartyScreenPhase::SwitchTarget { source_index: 0 };
        let result =
            screen.update_frame(PartyScreenInput {
                a: true,
                ..PartyScreenInput::none()
            });
        assert_eq!(result, PartyScreenAction::Active);
        assert_eq!(screen.phase(), PartyScreenPhase::Browsing);
    }

    // ── Field moves (Gen-1 FIELD_MOVE_MON_MENU) ──────────────────────

    fn mon_with_moves(moves: [pokered_data::moves::MoveId; 4]) -> Pokemon {
        let mut p = make_test_pokemon(Species::Squirtle);
        p.moves = moves;
        p
    }

    #[test]
    fn field_moves_listed_for_mon_that_knows_them() {
        use pokered_data::moves::MoveId;
        let party = vec![mon_with_moves([
            MoveId::Cut,
            MoveId::Tackle,
            MoveId::Surf,
            MoveId::None,
        ])];
        let screen = PartyScreenState::new(party);
        // Moveset order is preserved (GetMonFieldMoves scans the mon's moves).
        assert_eq!(
            screen.selected_field_moves(),
            vec![MoveId::Cut, MoveId::Surf]
        );
    }

    #[test]
    fn no_field_moves_for_ordinary_moveset() {
        use pokered_data::moves::MoveId;
        let party = vec![mon_with_moves([
            MoveId::Tackle,
            MoveId::Growl,
            MoveId::None,
            MoveId::None,
        ])];
        let screen = PartyScreenState::new(party);
        assert!(screen.selected_field_moves().is_empty());
    }

    #[test]
    fn field_move_dispatches_use_field_move_action() {
        use pokered_data::moves::MoveId;
        let party = vec![mon_with_moves([
            MoveId::Cut,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ])];
        let mut screen = PartyScreenState::new(party);
        // Open the action menu; cursor 0 is the field move (listed first,
        // above STATS/SWITCH/CANCEL).
        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.phase(), PartyScreenPhase::ActionMenu { cursor: 0 });
        let result = screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(
            result,
            PartyScreenAction::UseFieldMove {
                party_index: 0,
                move_id: MoveId::Cut,
            }
        );
        assert_eq!(screen.phase(), PartyScreenPhase::Browsing);
    }

    #[test]
    fn stats_switch_cancel_follow_field_moves() {
        use pokered_data::moves::MoveId;
        let party = vec![
            mon_with_moves([MoveId::Cut, MoveId::None, MoveId::None, MoveId::None]),
            make_test_pokemon(Species::Charmander),
        ];
        let mut screen = PartyScreenState::new(party);
        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        // Menu: [CUT, STATS, SWITCH, CANCEL] — max cursor = 3.
        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.phase(), PartyScreenPhase::ActionMenu { cursor: 1 });
        // STATS
        let result = screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(result, PartyScreenAction::ShowStats(0));

        // SWITCH at cursor 2.
        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.phase(), PartyScreenPhase::ActionMenu { cursor: 2 });
        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(
            screen.phase(),
            PartyScreenPhase::SwitchTarget { source_index: 0 }
        );
    }

    #[test]
    fn action_menu_cursor_clamps_at_cancel_with_field_moves() {
        use pokered_data::moves::MoveId;
        let party = vec![mon_with_moves([
            MoveId::Fly,
            MoveId::Strength,
            MoveId::None,
            MoveId::None,
        ])];
        let mut screen = PartyScreenState::new(party);
        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        // Menu: [FLY, STRENGTH, STATS, SWITCH, CANCEL] — max cursor = 4.
        for _ in 0..10 {
            screen.update_frame(PartyScreenInput {
                down: true,
                ..PartyScreenInput::none()
            });
        }
        assert_eq!(screen.phase(), PartyScreenPhase::ActionMenu { cursor: 4 });
        // CANCEL returns to browsing.
        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(screen.phase(), PartyScreenPhase::Browsing);
    }

    #[test]
    fn field_move_uses_selected_mon_not_first() {
        use pokered_data::moves::MoveId;
        let party = vec![
            make_test_pokemon(Species::Bulbasaur),
            mon_with_moves([MoveId::Teleport, MoveId::None, MoveId::None, MoveId::None]),
        ];
        let mut screen = PartyScreenState::new(party);
        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        let result = screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(
            result,
            PartyScreenAction::UseFieldMove {
                party_index: 1,
                move_id: MoveId::Teleport,
            }
        );
    }

    // -- SOFTBOILED (9th Gen-1 field move) ---------------------------------

    #[test]
    fn softboiled_listed_for_mon_that_knows_it() {
        use pokered_data::moves::MoveId;
        let party = vec![mon_with_moves([
            MoveId::Tackle,
            MoveId::Softboiled,
            MoveId::Cut,
            MoveId::None,
        ])];
        let screen = PartyScreenState::new(party);
        // GetMonFieldMoves scans the moveset: SOFTBOILED appears in place.
        assert_eq!(
            screen.selected_field_moves(),
            vec![MoveId::Softboiled, MoveId::Cut]
        );
    }

    #[test]
    fn softboiled_dispatches_like_other_field_moves() {
        use pokered_data::moves::MoveId;
        let party = vec![mon_with_moves([
            MoveId::Softboiled,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ])];
        let mut screen = PartyScreenState::new(party);
        screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        let result = screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(
            result,
            PartyScreenAction::UseFieldMove {
                party_index: 0,
                move_id: MoveId::Softboiled,
            }
        );
    }

    #[test]
    fn softboiled_target_pick_heals_another_member() {
        use pokered_data::moves::MoveId;
        let party = vec![
            mon_with_moves([MoveId::Softboiled, MoveId::None, MoveId::None, MoveId::None]),
            make_test_pokemon(Species::Charmander),
        ];
        let mut screen = PartyScreenState::new_for_softboiled_target(party, 0);
        assert_eq!(screen.mode(), PartyScreenMode::SoftboiledTarget(0));

        // A on the user itself: ignored (the original loops the pick).
        let result = screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(result, PartyScreenAction::Active);
        assert_eq!(screen.mode(), PartyScreenMode::SoftboiledTarget(0));

        // A on the other member: target chosen.
        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        let result = screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(
            result,
            PartyScreenAction::SoftboiledTargetChosen { target_index: 1 }
        );
    }

    #[test]
    fn softboiled_target_pick_b_returns_to_normal_menu() {
        use pokered_data::moves::MoveId;
        let party = vec![
            mon_with_moves([MoveId::Softboiled, MoveId::None, MoveId::None, MoveId::None]),
            make_test_pokemon(Species::Charmander),
        ];
        let mut screen = PartyScreenState::new_for_softboiled_target(party, 0);
        let result = screen.update_frame(PartyScreenInput {
            b: true,
            ..PartyScreenInput::none()
        });
        // B cancels the heal; the party menu stays open in normal mode
        // (StartMenu_Pokemon loop in the original).
        assert_eq!(result, PartyScreenAction::Active);
        assert_eq!(screen.mode(), PartyScreenMode::Normal);
        assert_eq!(screen.phase(), PartyScreenPhase::Browsing);
    }

    // -- bag item-use mode --------------------------------------------------

    #[test]
    fn item_mode_a_applies_item_without_action_menu() {
        let party = party_of(2);
        let mut screen =
            PartyScreenState::new_for_item(party, pokered_data::items::ItemId::Potion);
        assert_eq!(
            screen.mode(),
            PartyScreenMode::UseItem(pokered_data::items::ItemId::Potion)
        );
        screen.update_frame(PartyScreenInput {
            down: true,
            ..PartyScreenInput::none()
        });
        let result = screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(result, PartyScreenAction::ApplyItem { party_index: 1 });
        // No action menu was opened.
        assert_eq!(screen.phase(), PartyScreenPhase::Browsing);
    }

    #[test]
    fn item_mode_b_cancels() {
        let party = party_of(1);
        let mut screen =
            PartyScreenState::new_for_item(party, pokered_data::items::ItemId::FireStone);
        let result = screen.update_frame(PartyScreenInput {
            b: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(result, PartyScreenAction::Cancelled);
    }

    #[test]
    fn choose_move_picks_slot_and_cancel_row() {
        use pokered_data::moves::MoveId;
        let party = vec![mon_with_moves([
            MoveId::Tackle,
            MoveId::Growl,
            MoveId::VineWhip,
            MoveId::RazorLeaf,
        ])];
        let mut screen = PartyScreenState::new_for_item(party, pokered_data::items::ItemId::Tm01);
        screen.enter_move_choice();
        assert_eq!(screen.phase(), PartyScreenPhase::ChooseMove { cursor: 0 });

        // Move down twice, pick slot 2.
        for _ in 0..2 {
            screen.update_frame(PartyScreenInput {
                down: true,
                ..PartyScreenInput::none()
            });
        }
        let result = screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(
            result,
            PartyScreenAction::MoveForgetChosen {
                party_index: 0,
                slot: 2
            }
        );
        assert_eq!(screen.phase(), PartyScreenPhase::Browsing);
    }

    #[test]
    fn choose_move_cancel_row_gives_up() {
        use pokered_data::moves::MoveId;
        let party = vec![mon_with_moves([
            MoveId::Tackle,
            MoveId::Growl,
            MoveId::VineWhip,
            MoveId::RazorLeaf,
        ])];
        let mut screen = PartyScreenState::new_for_item(party, pokered_data::items::ItemId::Tm01);
        screen.enter_move_choice();
        // 4 known moves + CANCEL row: cursor clamps at 4.
        for _ in 0..10 {
            screen.update_frame(PartyScreenInput {
                down: true,
                ..PartyScreenInput::none()
            });
        }
        assert_eq!(screen.phase(), PartyScreenPhase::ChooseMove { cursor: 4 });
        let result = screen.update_frame(PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(result, PartyScreenAction::Active);
        assert_eq!(screen.phase(), PartyScreenPhase::Browsing);
    }

    #[test]
    fn choose_move_b_returns_to_browsing() {
        use pokered_data::moves::MoveId;
        let party = vec![mon_with_moves([
            MoveId::Tackle,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ])];
        let mut screen = PartyScreenState::new_for_item(party, pokered_data::items::ItemId::Tm01);
        screen.enter_move_choice();
        let result = screen.update_frame(PartyScreenInput {
            b: true,
            ..PartyScreenInput::none()
        });
        assert_eq!(result, PartyScreenAction::Active);
        assert_eq!(screen.phase(), PartyScreenPhase::Browsing);
    }
}
