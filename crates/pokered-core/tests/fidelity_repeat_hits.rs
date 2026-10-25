//! Original-source multi-hit outcomes through the public production FIGHT path.
//! Count sampling: effects.asm:925-969. KO exits: core.asm:3238-3255,5638-5654.

use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};

const VARIABLE_MOVES: [MoveId; 7] = [
    MoveId::Doubleslap,
    MoveId::CometPunch,
    MoveId::FuryAttack,
    MoveId::FurySwipes,
    MoveId::Barrage,
    MoveId::PinMissile,
    MoveId::SpikeCannon,
];

fn turn(
    move_id: MoveId,
    enemy: bool,
    first: u8,
    second: u8,
    hp: Option<u16>,
) -> (BattleScreen, u16, u64) {
    let attacker = create_pokemon_with_moves(
        Species::Mewtwo,
        20,
        [0x9a, 0x78],
        [move_id, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let mut target = create_pokemon_with_moves(
        Species::Snorlax,
        100,
        [0x9a, 0x78],
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    if let Some(hp) = hp {
        target.hp = hp;
    }
    let initial_hp = target.hp;
    let (player, foe) = if enemy {
        (target, attacker)
    } else {
        (attacker, target)
    };
    let mut battle = BattleScreen::from_parties(true, &[player], &[foe], None);
    battle.link_enemy_move_override = Some((if enemy { move_id } else { MoveId::Splash }, 0));
    battle.link_rng = Some(LinkRng::new([
        255, 255, 0, first, second, 82, 73, 64, 55, 46,
    ]));
    let state = battle.battle_state.as_mut().unwrap();
    assert!(!state.link_battle && !battle.link_mode);
    let (attacker, target) = if enemy {
        (&mut state.enemy, &mut state.player)
    } else {
        (&mut state.player, &mut state.enemy)
    };
    attacker.active_mon_mut().speed = 100;
    target.active_mon_mut().speed = 200;
    battle.set_presentation_enabled(true);
    battle.phase = BattlePhase::PlayerMenu;
    let mut a = BattleInput::none();
    a.a = true;
    battle.update_frame(a);
    battle.update_frame(a);
    let state = battle.battle_state.as_ref().unwrap();
    let target = if enemy { &state.player } else { &state.enemy };
    let damage = initial_hp - target.active_mon().hp;
    let draws = battle.link_rng.as_ref().unwrap().consumed();
    (battle, damage, draws)
}

fn finish_presentation(battle: &mut BattleScreen, move_id: MoveId, enemy: bool) -> usize {
    let mut serials = Vec::new();
    let mut a = BattleInput::none();
    a.a = true;
    for frame in 0..2500 {
        if let Some((serial, id, player)) = battle.presentation.current_move() {
            if id == move_id && player != enemy && !serials.contains(&serial) {
                serials.push(serial);
            }
        }
        if battle.presentation.waiting {
            battle.complete_move_presentation();
        }
        if matches!(
            battle.phase,
            BattlePhase::PlayerMenu | BattlePhase::BattleOver { .. }
        ) {
            return serials.len();
        }
        battle.update_frame(if frame % 2 == 0 {
            a
        } else {
            BattleInput::none()
        });
    }
    panic!(
        "presentation did not complete: {move_id:?}, enemy={enemy}, {:?}",
        battle.phase
    );
}

#[test]
fn seven_variable_moves_preserve_original_count_and_conditional_rng_draws() {
    // Original source: first low bits 0/1 finish at 2/3; 2/3 reroll.
    // High-bit aliases exercise actual input bytes through the game adapter.
    let counts = [[2, 2, 2, 2], [3, 3, 3, 3], [2, 3, 4, 5], [2, 3, 4, 5]];
    let count_draws = [1, 1, 2, 2];
    for move_id in VARIABLE_MOVES {
        for enemy in [false, true] {
            let (_, twice, _) = turn(move_id, enemy, 0, 0, None);
            assert!(twice > 0 && twice % 2 == 0);
            let per_hit = twice / 2;
            for first in [0, 1, 2, 3, 252, 253, 254, 255] {
                for second in [0, 1, 2, 3, 252, 253, 254, 255] {
                    let row = usize::from(first & 3);
                    let count = counts[row][usize::from(second & 3)];
                    let (mut battle, damage, draws) = turn(move_id, enemy, first, second, None);
                    assert_eq!(
                        damage,
                        per_hit * count,
                        "{move_id:?}, enemy={enemy}, {first}/{second}"
                    );
                    assert_eq!(
                        draws,
                        3 + count_draws[row],
                        "{move_id:?}, enemy={enemy}, {first}/{second}"
                    );
                    assert_eq!(
                        finish_presentation(&mut battle, move_id, enemy),
                        count as usize
                    );
                }
            }
        }
    }
}

#[test]
fn ten_multi_hit_moves_stop_animation_receipts_and_extra_draws_at_knockout() {
    for move_id in VARIABLE_MOVES.into_iter().chain([
        MoveId::DoubleKick,
        MoveId::Bonemerang,
        MoveId::Twineedle,
    ]) {
        let variable = VARIABLE_MOVES.contains(&move_id);
        let hits = if variable { 5 } else { 2 };
        for enemy in [false, true] {
            // 255/3 chooses five hits in both old and original samplers;
            // for Twineedle 255 instead rejects its final poison chance.
            let (mut survivor, damage, draws) = turn(move_id, enemy, 255, 3, None);
            assert!(damage > 0 && damage % hits == 0);
            let per_hit = damage / hits;
            assert_eq!(
                draws,
                if variable {
                    5
                } else if move_id == MoveId::Twineedle {
                    4
                } else {
                    3
                }
            );
            assert_eq!(
                finish_presentation(&mut survivor, move_id, enemy),
                hits as usize
            );
            for (hp, expected_hits) in [(1, 1), (per_hit + 1, 2), (damage, hits)] {
                let (mut battle, dealt, draws) = turn(move_id, enemy, 255, 3, Some(hp));
                assert_eq!(dealt, hp);
                assert_eq!(
                    draws,
                    if variable && hp > per_hit { 5 } else { 3 },
                    "{move_id:?}, enemy={enemy}, HP={hp}"
                );
                assert_eq!(
                    finish_presentation(&mut battle, move_id, enemy),
                    expected_hits as usize,
                    "no attack animation after KO: {move_id:?}, enemy={enemy}, HP={hp}"
                );
            }
        }
    }
}
