//! Primary-source raw-byte contract through the actual BattleScreen turn caller.
//! The other mon sleeps, isolating confusion from AI, damage and speed-tie RNG.
use super::*;
use crate::link::rng::LinkRng;
use crate::pokemon::stats::create_pokemon_with_moves;

fn screen(confused_player: bool) -> BattleScreen {
    let make = || {
        create_pokemon_with_moves(
            Species::Snorlax,
            50,
            [0x9a, 0x78],
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap()
    };
    let mut screen = BattleScreen::from_parties(true, &[make()], &[make()], None);
    screen.link_mode = true;
    screen.link_enemy_move_override = Some((MoveId::Tackle, 0));
    let state = screen.battle_state.as_mut().unwrap();
    state.player.active_mon_mut().speed = 100;
    state.enemy.active_mon_mut().speed = 50;
    let (confused, other) = if confused_player {
        (&mut state.player, &mut state.enemy)
    } else {
        (&mut state.enemy, &mut state.player)
    };
    confused.set_status1(state::status1::CONFUSED);
    confused.confused_turns_left = 3;
    other.active_mon_mut().status = StatusCondition::Sleep(2);
    screen
}

#[test]
fn real_turn_caller_uses_uncomplemented_link_confusion_bytes_on_both_sides() {
    for player in [true, false] {
        // Expected outcomes come from CP $80 / JR C, independently of either
        // native/legacy implementation. Same 50% rate, opposite byte polarity.
        for (byte, self_hit) in [(0, false), (127, false), (128, true), (255, true)] {
            let mut screen = screen(player);
            let before = screen.battle_state.as_ref().unwrap().clone();
            let list = [byte, 255, 0, 255, 91, 82, 73, 64, 55, 46];
            screen.link_rng = Some(LinkRng::new(list));
            screen.execute_turn_with_move(0);
            let after = screen.battle_state.as_ref().unwrap();
            let (actor, target, old_actor, old_target) = if player {
                (&after.player, &after.enemy, &before.player, &before.enemy)
            } else {
                (&after.enemy, &after.player, &before.enemy, &before.player)
            };
            assert_eq!(actor.active_mon().hp < old_actor.active_mon().hp, self_hit);
            assert_eq!(
                target.active_mon().hp < old_target.active_mon().hp,
                !self_hit
            );
            assert_eq!(actor.confused_turns_left, 2);
            assert_eq!(actor.last_move_used == MoveId::Tackle, !self_hit);
            if player {
                assert_eq!(
                    actor.active_mon().pp[0],
                    old_actor.active_mon().pp[0] - u8::from(!self_hit),
                    "existing PP gate: only a move passing confusion spends PP"
                );
            }
            let rng = screen.link_rng.as_ref().unwrap();
            assert_eq!(rng.consumed(), if self_hit { 1 } else { 4 });
            assert_eq!(rng.index(), if self_hit { 1 } else { 4 });
            assert_eq!(rng.list(), list);
        }
    }
}
