//! Original RandomizeDamage must keep sampling until it accepts a byte.
use pokered_core::battle::state::{status2, StatusCondition};
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species, types::PokemonType};

// Observed original-ROM RandomizeDamage, typed damage 2 and initial index 2,
// for each uniform 10-byte link seed list. These are oracle outputs.
const ORIGINAL_DAMAGE: [u16; 256] = [
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2,
];
const ORIGINAL_DRAWS: [u64; 256] = [
    98, 89, 170, 35, 44, 35, 80, 17, 44, 17, 116, 161, 26, 53, 26, 53, 26, 53, 26, 17, 26, 35, 26,
    35, 26, 143, 26, 179, 26, 17, 26, 71, 26, 89, 26, 89, 8, 35, 8, 17, 8, 35, 8, 53, 8, 53, 8, 53,
    8, 17, 8, 107, 44, 125, 62, 35, 152, 35, 98, 17, 62, 17, 44, 71, 98, 89, 44, 89, 134, 17, 80,
    17, 44, 35, 116, 35, 44, 53, 152, 17, 80, 17, 62, 107, 116, 125, 44, 71, 8, 17, 8, 17, 8, 197,
    8, 71, 8, 71, 8, 53, 8, 17, 8, 53, 26, 35, 26, 35, 26, 35, 26, 17, 26, 71, 26, 107, 26, 125,
    26, 125, 26, 17, 62, 35, 44, 35, 134, 53, 62, 53, 44, 17, 116, 53, 44, 143, 170, 71, 8, 35, 8,
    17, 8, 17, 8, 71, 8, 89, 8, 107, 8, 17, 8, 17, 44, 53, 62, 35, 44, 35, 98, 17, 62, 17, 134,
    107, 80, 143, 44, 161, 116, 17, 206, 17, 62, 35, 80, 35, 44, 1, 62, 1, 26, 1, 26, 1, 26, 1, 26,
    1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 26, 1, 62, 1, 188, 1, 44, 1, 62, 1, 44, 1,
    98, 1, 62, 1, 80, 1, 80, 1, 44, 1, 134, 1, 62, 1, 80, 1, 98, 1, 44, 1, 152, 1, 44, 1, 8, 1, 8,
    1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1,
];
fn setup(enemy: bool, id: MoveId, types: [PokemonType; 2], raw: u8) -> BattleScreen {
    let attacker = create_pokemon_with_moves(
        Species::Mewtwo,
        1,
        [0x9a, 0x78],
        [id, MoveId::None, MoveId::None, MoveId::None],
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
    battle.link_enemy_move_override = Some((if enemy { id } else { MoveId::Splash }, 0));
    let bytes = [raw; 10];
    let mut rng = LinkRng::new(bytes);
    // The isolated original oracle starts variance at index 2. Prepare index 1
    // so the critical byte leads into exactly that same stream position.
    rng.next_stream_byte();
    battle.link_rng = Some(rng);
    let state = battle.battle_state.as_mut().unwrap();
    let (actor, target) = if enemy {
        (&mut state.enemy, &mut state.player)
    } else {
        (&mut state.player, &mut state.enemy)
    };
    // Prepared stats give base damage 2; converted types reduce it to 0/1.
    // Isolate variance from the subsequent accuracy byte, including raw 255.
    actor.battle_status2 |= status2::USING_X_ACCURACY;
    actor.active_mon_mut().attack = 1;
    actor.active_mon_mut().special = 1;
    actor.active_mon_mut().speed = 100;
    target.active_mon_mut().defense = 250;
    target.active_mon_mut().special = 250;
    target.conversion_type1 = Some(types[0]);
    target.conversion_type2 = Some(types[1]);
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
    let initial_draws = battle.link_rng.as_ref().unwrap().consumed();
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
    assert!(messages.iter().any(|s| s.contains("used POUND!")));
    let outcome = (
        hp - target_hp(&battle, enemy),
        battle.link_rng.as_ref().unwrap().consumed() - initial_draws,
    );
    for frame in 0..4000 {
        if battle.presentation.waiting {
            battle.complete_move_presentation();
        }
        if matches!(battle.phase, BattlePhase::PlayerMenu) {
            assert_eq!(battle.link_rng.as_ref().unwrap().consumed() - initial_draws, outcome.1);
            return outcome;
        }
        let mut input = BattleInput::none();
        input.a = frame % 2 == 0;
        battle.update_frame(input);
    }
    panic!("presentation did not finish: {:?}", battle.phase);
}

fn verify_sequences(long: bool) {
    for enemy in [false, true] {
        for raw in 0..=255u8 {
            let expected_draws = ORIGINAL_DRAWS[raw as usize];
            if (expected_draws > 65) != long {
                continue;
            }
            assert_eq!(
                fight_and_drain(
                    setup(enemy, MoveId::Pound, [PokemonType::Normal; 2], raw),
                    enemy
                ),
                (ORIGINAL_DAMAGE[raw as usize], 1 + expected_draws),
                "enemy={enemy} raw={raw}"
            );
        }
    }
}

#[test]
fn long_rejection_sequences_keep_the_original_rng_position() {
    verify_sequences(true);
}

#[test]
fn shorter_rejection_sequences_preserve_damage_and_rng_position() {
    verify_sequences(false);
}
