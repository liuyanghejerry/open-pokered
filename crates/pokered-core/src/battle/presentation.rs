//! Ordered visual handoff for a turn already resolved by the deterministic engine.
//! Metadata is attached before localization/pagination, never parsed from a page.
use super::{
    pokered_rules::{runtime, PokeVolatile, PokeredRules},
    BattleScreen, BattleSide,
};
use crate::alloc_prelude::*;
use dotzuki_engine::battle::{
    stack::{EffectState, HpChangeCause, TurnEvent, TurnLog},
    BattleState,
};
use pokered_data::{lang_data::move_name, moves::MoveId};

#[derive(Debug, Clone)]
struct MoveStep {
    announcement: String,
    move_id: Option<MoveId>,
    leech_seed: bool,
    charge: Option<MoveId>,
    missed: bool,
    player: bool,
    hp: [u16; 2],
    hits: Vec<[u16; 2]>,
}

#[derive(Debug, Clone, Default)]
pub struct BattlePresentation {
    /// Frontends opt in only when they can acknowledge visual completion.
    pub enabled: bool,
    pub waiting: bool,
    queued: Vec<MoveStep>,
    pages: Vec<Option<MoveStep>>,
    page: Option<usize>,
    serial: u32,
    current: Option<(u32, MoveId, bool)>,
    current_leech_seed: Option<(u32, bool)>,
    current_charge: Option<(u32, MoveId, bool)>,
    hit_index: usize,
}
impl BattlePresentation {
    pub fn current_leech_seed(&self) -> Option<(u32, bool)> {
        self.current_leech_seed
    }

    pub fn current_charge(&self) -> Option<(u32, MoveId, bool)> {
        self.current_charge
    }
    pub fn current_move_missed(&self) -> bool {
        self.page
            .and_then(|page| self.pages.get(page))
            .and_then(Option::as_ref)
            .is_some_and(|step| step.missed)
    }
    pub(super) fn charge_in_order(
        &mut self,
        messages: &mut Vec<String>,
        name: &str,
        move_id: MoveId,
        player: bool,
    ) {
        let used = format!("{} used {}!", name, move_name(move_id, false));
        let charged = format!("{} {}", name, super::charge_message(move_id));
        if let Some(index) = messages.iter().position(|message| message == &used) {
            messages[index] = charged.clone();
        } else {
            messages.push(charged.clone());
        }
        if let Some(step) = self
            .queued
            .iter_mut()
            .find(|step| step.player == player && step.move_id == Some(move_id))
        {
            step.announcement = charged;
            step.move_id = None;
            step.charge = Some(move_id);
        }
    }

    pub(super) fn called_move_in_order(
        &mut self,
        messages: &mut Vec<String>,
        name: &str,
        label: &str,
        resolved: MoveId,
        player: bool,
        failed: bool,
        initial_hp: [u16; 2],
    ) {
        let resolved_text = format!("{} used {}!", name, move_name(resolved, false));
        let index = messages
            .iter()
            .position(|message| message == &resolved_text);
        // A blocked actor never used the calling move either.
        if index.is_none() && !failed {
            return;
        }
        let index = index.unwrap_or(0);
        let announcement = format!("{name} used {label}!");
        messages.insert(index, announcement.clone());
        if failed {
            messages.insert(index + 1, "But it failed!".to_string());
        }
        if !self.enabled {
            return;
        }
        let step_index = self
            .queued
            .iter()
            .position(|step| step.player == player && step.move_id == Some(resolved))
            .unwrap_or(0);
        let hp = step_index
            .checked_sub(1)
            .map(|i| self.queued[i].hp)
            .unwrap_or(initial_hp);
        self.queued.insert(
            step_index,
            MoveStep {
                announcement,
                move_id: Some(if label == "METRONOME" {
                    MoveId::Metronome
                } else {
                    MoveId::MirrorMove
                }),
                charge: None,
                leech_seed: false,
                missed: failed,
                player,
                hp,
                hits: Vec::new(),
            },
        );
    }

    pub fn current_move(&self) -> Option<(u32, MoveId, bool)> {
        self.current
    }
    pub fn manages_moves(&self) -> bool {
        self.enabled
            && self
                .pages
                .iter()
                .flatten()
                .any(|step| step.move_id.is_some())
    }
    pub(super) fn holds_hp(&self) -> bool {
        self.enabled && (!self.queued.is_empty() || self.pages.iter().any(Option::is_some))
    }

    pub(super) fn record_turn(
        &mut self,
        log: &TurnLog<PokeredRules>,
        state: &BattleState<PokeredRules>,
        effects: &[EffectState<PokeredRules>],
        mut hp: [u16; 2],
    ) {
        if !self.enabled {
            return;
        }
        self.queued.clear();
        for event in &log.events {
            if let Some(announcement) = runtime::presentation_hp_message(event, state, log) {
                self.queued.push(MoveStep {
                    announcement,
                    hits: Vec::new(),
                    move_id: None,
                    charge: None,
                    missed: false,
                    leech_seed: matches!(
                        event,
                        TurnEvent::Damaged {
                            cause: Some(HpChangeCause::Volatile(PokeVolatile::LeechSeed)),
                            ..
                        }
                    ),
                    // The absorbing animation originates from the OTHER side.
                    player: matches!(event, TurnEvent::Damaged { target, .. } if target.side != 0),
                    hp,
                });
            }
            match event {
                TurnEvent::MoveUsed { actor, move_ } => self.queued.push(MoveStep {
                    announcement: format!(
                        "{} used {}!",
                        runtime::display_name(state, *actor),
                        move_name(*move_, false)
                    ),
                    hits: {
                        let recorded = super::pokered_rules::presented_hits(actor.side == 0);
                        let mut staged = hp;
                        let mut hits = Vec::new();
                        if recorded[5] > 1 {
                            for loss in recorded.iter().take(recorded[5] as usize) {
                                let target = usize::from(actor.side == 0);
                                staged[target] = staged[target].saturating_sub(*loss);
                                hits.push(staged);
                            }
                        }
                        hits
                    },
                    move_id: Some(*move_),
                    leech_seed: false,
                    charge: None,
                    missed: runtime::move_is_immune(state, effects, *actor, *move_),
                    player: actor.side == 0,
                    hp,
                }),
                TurnEvent::Missed { actor } => {
                    if let Some(step) = self
                        .queued
                        .iter_mut()
                        .rev()
                        .find(|step| step.move_id.is_some() && step.player == (actor.side == 0))
                    {
                        step.missed = true;
                    }
                }
                TurnEvent::Damaged { target, amount, .. } => {
                    hp[target.side as usize] = hp[target.side as usize].saturating_sub(*amount)
                }
                TurnEvent::Healed { target, amount, .. } => {
                    hp[target.side as usize] = hp[target.side as usize].saturating_add(*amount)
                }
                _ => {}
            }
            if let Some(step) = self.queued.last_mut() {
                step.hp = hp;
            }
        }
    }

    pub(super) fn begin_pages(&mut self) {
        self.pages.clear();
        self.page = None;
        self.current = None;
        self.current_leech_seed = None;
        self.current_charge = None;
        self.waiting = false;
    }
    pub(super) fn add_pages(&mut self, message: &str, count: usize) {
        let step = self
            .queued
            .iter()
            .position(|s| s.announcement == message)
            .map(|i| self.queued.remove(i));
        self.pages.extend((0..count).map(|_| None));
        if count > 0 {
            *self.pages.last_mut().unwrap() = step;
        }
    }
    pub(super) fn activate(&mut self, page: usize) {
        if !self.enabled || self.page == Some(page) {
            return;
        }
        self.page = Some(page);
        let step = self.pages.get(page).and_then(Option::as_ref);
        self.waiting = step.is_some();
        self.hit_index = 0;
        self.current_charge = step.and_then(|step| {
            step.charge.map(|move_id| {
                self.serial = self.serial.wrapping_add(1);
                (self.serial, move_id, step.player)
            })
        });
        self.current_leech_seed = step.filter(|step| step.leech_seed).map(|step| {
            self.serial = self.serial.wrapping_add(1);
            (self.serial, step.player)
        });
        self.current = step.and_then(|step| {
            step.move_id.map(|move_id| {
                self.serial = self.serial.wrapping_add(1);
                (self.serial, move_id, step.player)
            })
        });
    }
    pub(super) fn next_hit(&mut self) -> bool {
        let Some(step) = self
            .page
            .and_then(|page| self.pages.get(page))
            .and_then(Option::as_ref)
        else {
            return false;
        };
        if !self.waiting && self.hit_index > 0 && self.hit_index < step.hits.len() {
            self.serial = self.serial.wrapping_add(1);
            self.current = step.move_id.map(|id| (self.serial, id, step.player));
            self.waiting = true;
            return true;
        }
        false
    }
    pub(super) fn finish_pages(&mut self) {
        self.queued.clear();
        self.begin_pages();
    }
    pub(super) fn finish_preparing(&mut self) {
        self.queued.clear();
    }
}

impl BattleScreen {
    pub fn set_presentation_enabled(&mut self, enabled: bool) {
        if self.presentation.enabled == enabled {
            return;
        }
        self.presentation.enabled = enabled;
        if enabled {
            self.sync_display_from_state();
        }
    }

    /// Called once the frontend has finished the move AND its impact effects.
    pub fn complete_move_presentation(&mut self) {
        if !self.presentation.waiting {
            return;
        }
        self.presentation.waiting = false;
        let Some(step) = self
            .presentation
            .page
            .and_then(|page| self.presentation.pages.get(page))
            .and_then(Option::as_ref)
        else {
            return;
        };
        let hp = step
            .hits
            .get(self.presentation.hit_index)
            .copied()
            .unwrap_or(step.hp);
        self.presentation.hit_index += 1;
        self.hp_bar_anim.set_target(
            BattleSide::Player,
            hp[0],
            self.player_max_hp,
            self.player_species,
            &mut self.player_hp,
        );
        self.hp_bar_anim.set_target(
            BattleSide::Enemy,
            hp[1],
            self.enemy_max_hp,
            self.enemy_species,
            &mut self.enemy_hp,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        battle::{BattleInput, BattlePhase},
        pokemon::stats::create_pokemon_with_moves,
    };
    use pokered_data::species::Species;

    #[test]
    fn both_speed_orders_wait_for_each_animation_then_drain_only_its_damage() {
        for player_first in [true, false] {
            for chinese in [false, true] {
                let make = |species| {
                    create_pokemon_with_moves(
                        species,
                        30,
                        [0xff; 2],
                        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
                    )
                    .unwrap()
                };
                let mut battle = BattleScreen::from_parties(
                    true,
                    &[make(Species::Charmander)],
                    &[make(Species::Bulbasaur)],
                    None,
                );
                battle.rng = runtime::StdBattleRng::from_seed(42);
                let bs = battle.battle_state.as_mut().unwrap();
                bs.player.active_mon_mut().speed = if player_first { 200 } else { 10 };
                bs.enemy.active_mon_mut().speed = if player_first { 10 } else { 200 };
                battle.is_zh = chinese;
                battle.set_presentation_enabled(true);
                let original = [battle.player_hp, battle.enemy_hp];
                battle.execute_turn_with_move(0);
                assert_eq!(
                    [battle.player_hp, battle.enemy_hp],
                    original,
                    "resolution must not expose final HP"
                );
                let mut sides = Vec::new();
                for _ in 0..1000 {
                    if battle.presentation.waiting {
                        let action = battle.presentation.current_move().unwrap();
                        sides.push(action.2);
                        let hp = [battle.player_hp, battle.enemy_hp];
                        for _ in 0..40 {
                            battle.update_frame(BattleInput {
                                a: true,
                                ..BattleInput::none()
                            });
                        }
                        assert_eq!(battle.presentation.current_move(), Some(action));
                        assert_eq!(
                            [battle.player_hp, battle.enemy_hp],
                            hp,
                            "even mashing A must wait for animation"
                        );
                        battle.complete_move_presentation();
                        for _ in 0..150 {
                            battle.update_frame(BattleInput::none());
                        }
                        if sides.len() == 1 {
                            let attacker = if player_first { 0 } else { 1 };
                            assert_eq!(
                                [battle.player_hp, battle.enemy_hp][attacker],
                                original[attacker],
                                "opponent's damage must still wait"
                            );
                        }
                    }
                    battle.update_frame(BattleInput {
                        a: true,
                        ..BattleInput::none()
                    });
                    if matches!(battle.phase, BattlePhase::PlayerMenu) {
                        break;
                    }
                }
                assert_eq!(sides, vec![player_first, !player_first]);
                let bs = battle.battle_state.as_ref().unwrap();
                assert_eq!(
                    [battle.player_hp, battle.enemy_hp],
                    [bs.player.active_mon().hp, bs.enemy.active_mon().hp]
                );
            }
        }
    }
    #[test]
    fn enemy_free_turn_stages_damage_and_poison_at_distinct_pages() {
        use crate::battle::state::StatusCondition;
        let make = |species| {
            create_pokemon_with_moves(
                species,
                30,
                [0xff; 2],
                [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            )
            .unwrap()
        };
        let mut battle = BattleScreen::from_parties(
            true,
            &[make(Species::Charmander)],
            &[make(Species::Bulbasaur)],
            None,
        );
        battle
            .battle_state
            .as_mut()
            .unwrap()
            .enemy
            .active_mon_mut()
            .status = StatusCondition::Poison;
        battle.rng = runtime::StdBattleRng::from_seed(42);
        battle.set_presentation_enabled(true);
        let original = [battle.player_hp, battle.enemy_hp];
        battle.run_enemy_free_turn_stack(Vec::new(), None);
        assert_eq!([battle.player_hp, battle.enemy_hp], original);
        let steps: Vec<_> = battle.presentation.pages.iter().flatten().collect();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].move_id, Some(MoveId::Tackle));
        assert!(!steps[0].player);
        assert_eq!(
            steps[0].hp[1], original[1],
            "poison must not be folded into the move"
        );
        assert!(steps[0].hp[0] < original[0]);
        assert_eq!(steps[1].move_id, None);
        assert!(steps[1].announcement.contains("POISON"));
        assert!(steps[1].hp[1] < original[1]);
    }
    #[test]
    fn knockout_has_one_animation_and_hides_damage_until_it_finishes() {
        let make = |species, level| {
            create_pokemon_with_moves(
                species,
                level,
                [0xff; 2],
                [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            )
            .unwrap()
        };
        let mut battle = BattleScreen::from_parties(
            true,
            &[make(Species::Charizard, 80)],
            &[make(Species::Bulbasaur, 5)],
            None,
        );
        battle.rng = runtime::StdBattleRng::from_seed(42);
        battle.set_presentation_enabled(true);
        let hp = battle.enemy_hp;
        battle.execute_turn_with_move(0);
        assert_eq!(battle.enemy_hp, hp);
        let actions: Vec<_> = battle
            .presentation
            .pages
            .iter()
            .flatten()
            .filter(|step| step.move_id.is_some())
            .collect();
        assert_eq!(actions.len(), 1);
        assert!(actions[0].player);
        assert_eq!(actions[0].hp[1], 0);
        assert!(battle.presentation.waiting);
        battle.complete_move_presentation();
        for _ in 0..200 {
            battle.update_frame(BattleInput::none());
        }
        assert_eq!(battle.enemy_hp, 0);
    }

    #[test]
    fn charge_turn_is_not_a_strike_and_stays_before_the_other_move() {
        for player in [true, false] {
            for mv in [
                MoveId::Fly,
                MoveId::Dig,
                MoveId::Solarbeam,
                MoveId::RazorWind,
                MoveId::SkullBash,
                MoveId::SkyAttack,
            ] {
                let make = |id| {
                    create_pokemon_with_moves(
                        Species::Mew,
                        50,
                        [0xff; 2],
                        [id, MoveId::None, MoveId::None, MoveId::None],
                    )
                    .unwrap()
                };
                let mut battle = BattleScreen::from_parties(
                    true,
                    &[make(if player { mv } else { MoveId::Splash })],
                    &[make(if player { MoveId::Splash } else { mv })],
                    None,
                );
                let bs = battle.battle_state.as_mut().unwrap();
                bs.player.active_mon_mut().speed = if player { 200 } else { 10 };
                bs.enemy.active_mon_mut().speed = if player { 10 } else { 200 };
                battle.set_presentation_enabled(true);
                battle.execute_turn_with_move(0);
                let first = battle.presentation.pages.iter().flatten().next().unwrap();
                assert_eq!(first.charge, Some(mv));
                assert_eq!(first.move_id, None);
                assert_eq!(first.player, player);
                assert!(!first.announcement.contains(" used "));
            }
        }
    }

    #[test]
    fn misses_are_known_before_the_move_presentation_starts() {
        let make = |species| {
            create_pokemon_with_moves(
                species,
                30,
                [0xff; 2],
                [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            )
            .unwrap()
        };
        let mut battle = BattleScreen::from_parties(
            true,
            &[make(Species::Charmander)],
            &[make(Species::Gastly)],
            None,
        );
        battle.set_presentation_enabled(true);
        battle.execute_turn_with_move(0);
        let steps: Vec<_> = battle
            .presentation
            .pages
            .iter()
            .flatten()
            .filter(|step| step.move_id.is_some())
            .collect();
        assert!(
            steps.iter().any(|step| step.player && step.missed),
            "Normal attack into Ghost is a miss"
        );
    }

    #[test]
    fn called_move_plays_before_its_strike_and_after_the_faster_opponent() {
        for player in [true, false] {
            let make = |mv| {
                create_pokemon_with_moves(
                    Species::Mew,
                    50,
                    [0xff; 2],
                    [mv, MoveId::None, MoveId::None, MoveId::None],
                )
                .unwrap()
            };
            let mut battle = BattleScreen::from_parties(
                true,
                &[make(if player {
                    MoveId::MirrorMove
                } else {
                    MoveId::Tackle
                })],
                &[make(if player {
                    MoveId::Tackle
                } else {
                    MoveId::MirrorMove
                })],
                None,
            );
            let bs = battle.battle_state.as_mut().unwrap();
            bs.player.last_move_used = MoveId::Tackle;
            bs.enemy.last_move_used = MoveId::Tackle;
            bs.player.active_mon_mut().speed = if player { 10 } else { 200 };
            bs.enemy.active_mon_mut().speed = if player { 200 } else { 10 };
            battle.rng = runtime::StdBattleRng::from_seed(42);
            battle.set_presentation_enabled(true);
            battle.execute_turn_with_move(0);
            let steps: Vec<_> = battle.presentation.pages.iter().flatten().collect();
            assert_eq!(steps.len(), 3);
            assert_eq!(steps[0].player, !player);
            assert_eq!(steps[1].player, player);
            assert_eq!(steps[1].move_id, Some(MoveId::MirrorMove));
            assert_eq!(steps[2].move_id, Some(MoveId::Tackle));
            assert_eq!(
                steps[1].hp, steps[0].hp,
                "calling animation cannot apply strike damage"
            );
        }
    }

    #[test]
    fn multi_hit_repeats_animation_and_drains_each_hit_before_next() {
        let make = |mv| {
            create_pokemon_with_moves(
                Species::Mew,
                50,
                [0xff; 2],
                [mv, MoveId::None, MoveId::None, MoveId::None],
            )
            .unwrap()
        };
        let mut battle = BattleScreen::from_parties(
            true,
            &[make(MoveId::DoubleKick)],
            &[make(MoveId::Splash)],
            None,
        );
        battle.rng = runtime::StdBattleRng::from_seed(42);
        battle
            .battle_state
            .as_mut()
            .unwrap()
            .player
            .active_mon_mut()
            .speed = 200;
        battle.set_presentation_enabled(true);
        let start = battle.enemy_hp;
        battle.execute_turn_with_move(0);
        let step = battle
            .presentation
            .pages
            .iter()
            .flatten()
            .find(|step| step.player)
            .unwrap();
        assert_eq!(step.hits.len(), 2);
        let hp = step.hits.clone();
        assert!(hp[0][1] < start && hp[1][1] < hp[0][1]);
        let first = battle.presentation.current_move().unwrap().0;
        battle.complete_move_presentation();
        for _ in 0..200 {
            battle.update_frame(BattleInput::none());
            if battle.presentation.waiting {
                break;
            }
        }
        assert_eq!(battle.enemy_hp, hp[0][1]);
        assert!(battle.presentation.waiting);
        assert_ne!(battle.presentation.current_move().unwrap().0, first);
        battle.complete_move_presentation();
        for _ in 0..200 {
            battle.update_frame(BattleInput::none());
        }
        assert_eq!(battle.enemy_hp, hp[1][1]);
    }
}
