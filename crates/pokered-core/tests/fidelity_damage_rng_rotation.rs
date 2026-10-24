//! Expected damage and draw counts come from executing original RandomizeDamage.
use pokered_core::battle::state::StatusCondition;
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};

// Generated from observed ROM results, with initial damage 200 and fallback 255.
const ORIGINAL_DAMAGE: [u16; 256] = [
    200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200,
    200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200,
    200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200,
    200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200,
    200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200,
    200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200,
    200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200,
    200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200,
    200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200,
    200, 200, 200, 200, 200, 200, 200, 200, 170, 200, 170, 200, 171, 200, 172, 200, 173, 200, 174,
    200, 174, 200, 175, 200, 176, 200, 177, 200, 178, 200, 178, 200, 179, 200, 180, 200, 181, 200,
    181, 200, 182, 200, 183, 200, 184, 200, 185, 200, 185, 200, 186, 200, 187, 200, 188, 200, 189,
    200, 189, 200, 190, 200, 191, 200, 192, 200, 192, 200, 193, 200, 194, 200, 195, 200, 196, 200,
    196, 200, 197, 200, 198, 200, 199, 200, 200,
];
const ORIGINAL_DRAWS: [u64; 256] = [
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1,
    2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1,
    2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1,
];

fn setup(enemy: bool, variance_bytes: &[u8]) -> BattleScreen {
    let attacker = create_pokemon_with_moves(
        Species::Mewtwo,
        100,
        [0x9a, 0x78],
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let mut defender = create_pokemon_with_moves(
        Species::Snorlax,
        100,
        [0x9a, 0x78],
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    defender.status = StatusCondition::Sleep(7);
    let (player, foe) = if enemy {
        (defender, attacker)
    } else {
        (attacker, defender)
    };
    let mut battle = BattleScreen::from_parties(true, &[player], &[foe], None);
    battle.link_enemy_move_override = Some((
        if enemy {
            MoveId::Tackle
        } else {
            MoveId::Splash
        },
        0,
    ));
    let mut bytes = [255; 10];
    bytes[1] = 0;
    bytes[2..2 + variance_bytes.len()].copy_from_slice(variance_bytes);
    battle.link_rng = Some(LinkRng::new(bytes));
    let state = battle.battle_state.as_mut().unwrap();
    let (actor, target) = if enemy {
        (&mut state.enemy, &mut state.player)
    } else {
        (&mut state.player, &mut state.enemy)
    };
    // Prepared working stats isolate a pre-variance damage of exactly 200.
    actor.active_mon_mut().attack = 203;
    actor.active_mon_mut().speed = 100;
    target.active_mon_mut().defense = 30;
    target.active_mon_mut().speed = 200;
    battle.set_presentation_enabled(true);
    battle.phase = BattlePhase::PlayerMenu;
    battle
}

fn target_hp(battle: &BattleScreen, enemy: bool) -> u16 {
    let state = battle.battle_state.as_ref().unwrap();
    if enemy {
        state.player.active_mon().hp
    } else {
        state.enemy.active_mon().hp
    }
}

fn fight_and_drain(mut battle: BattleScreen, enemy: bool) -> (u16, u64) {
    let hp = target_hp(&battle, enemy);
    let mut input = BattleInput::none();
    input.a = true;
    for _ in 0..4 {
        battle.update_frame(input);
        if matches!(battle.phase, BattlePhase::ShowingText { .. }) {
            break;
        }
    }
    let BattlePhase::ShowingText { messages, .. } = &battle.phase else {
        panic!("FIGHT did not resolve");
    };
    assert!(messages
        .iter()
        .all(|s| !s.contains("Critical hit!") && !s.contains("missed!")));
    let outcome = (
        hp - target_hp(&battle, enemy),
        battle.link_rng.as_ref().unwrap().consumed(),
    );
    for frame in 0..4000 {
        if battle.presentation.waiting {
            battle.complete_move_presentation();
        }
        if matches!(battle.phase, BattlePhase::PlayerMenu) {
            return outcome;
        }
        let mut input = BattleInput::none();
        input.a = frame % 2 == 0;
        battle.update_frame(input);
    }
    panic!("presentation did not finish: {:?}", battle.phase);
}

#[test]
fn all_256_original_damage_bytes_match_on_both_sides() {
    for enemy in [false, true] {
        for raw in 0..=255u8 {
            // Crit then accuracy remain separate from the observed variance draws.
            assert_eq!(
                fight_and_drain(setup(enemy, &[raw]), enemy),
                (
                    ORIGINAL_DAMAGE[raw as usize],
                    2 + ORIGINAL_DRAWS[raw as usize]
                ),
                "enemy={enemy} raw={raw}"
            );
        }
    }
}

#[test]
fn each_rejection_retry_rotates_its_new_byte() {
    for enemy in [false, true] {
        // These three sequences were separately executed in the original ROM.
        for (seeds, damage, draws) in [
            (&[0, 179][..], 170, 2),
            (&[218, 217][..], 185, 2),
            (&[0, 0, 179][..], 170, 3),
        ] {
            assert_eq!(
                fight_and_drain(setup(enemy, seeds), enemy),
                (damage, draws + 2),
                "enemy={enemy} seeds={seeds:?}"
            );
        }
    }
}
