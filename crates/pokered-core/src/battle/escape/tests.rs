use super::*;
use crate::battle::state::*;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

fn make_pokemon(level: u8, speed: u16) -> Pokemon {
    Pokemon {
        species: Species::Pikachu,
        nickname: [0x50; 11],
        level,
        hp: 50,
        max_hp: 50,
        attack: 50,
        defense: 50,
        speed,
        special: 50,
        type1: PokemonType::Electric,
        type2: PokemonType::Electric,
        moves: [
            MoveId::Thundershock,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ],
        pp: [30, 0, 0, 0],
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

fn make_state(player_speed: u16, enemy_speed: u16) -> BattleState {
    new_battle_state(
        BattleType::Wild,
        vec![make_pokemon(25, player_speed)],
        vec![make_pokemon(25, enemy_speed)],
    )
}

// ==================== try_run_from_battle ====================

#[test]
fn run_trainer_battle_always_fails() {
    let mut state = new_battle_state(
        BattleType::Trainer,
        vec![make_pokemon(25, 100)],
        vec![make_pokemon(25, 50)],
    );
    assert_eq!(try_run_from_battle(&mut state, 0), RunResult::CannotRun);
    assert!(!state.escaped);
}

#[test]
fn run_player_faster_always_escapes() {
    let mut state = make_state(100, 50);
    assert_eq!(try_run_from_battle(&mut state, 255), RunResult::Escaped);
    assert!(state.escaped);
    assert_eq!(state.num_run_attempts, 1);
}

#[test]
fn run_equal_speed_always_escapes() {
    let mut state = make_state(80, 80);
    assert_eq!(try_run_from_battle(&mut state, 255), RunResult::Escaped);
    assert!(state.escaped);
}

#[test]
fn run_enemy_faster_can_fail() {
    // player_speed=50, enemy_speed=200
    // divisor = (200/4) % 256 = 50
    // quotient = (50*32)/50 = 32
    // first attempt: escape_odds = 32, random_byte = 100
    // 32 < 100 → fail
    let mut state = make_state(50, 200);
    assert_eq!(
        try_run_from_battle(&mut state, 100),
        RunResult::FailedToEscape
    );
    assert!(!state.escaped);
    assert_eq!(state.num_run_attempts, 1);
}

#[test]
fn run_enemy_faster_can_succeed_with_low_random() {
    // same setup, but random_byte = 10
    // escape_odds = 32 >= 10 → escape
    let mut state = make_state(50, 200);
    assert_eq!(try_run_from_battle(&mut state, 10), RunResult::Escaped);
    assert!(state.escaped);
}

#[test]
fn run_attempts_add_30_each() {
    // player_speed=50, enemy_speed=200
    // quotient = 32
    // attempt 1: odds=32, random=60 → fail (32 < 60)
    // attempt 2: odds=32+30=62, random=60 → succeed (62 >= 60)
    let mut state = make_state(50, 200);
    assert_eq!(
        try_run_from_battle(&mut state, 60),
        RunResult::FailedToEscape
    );
    assert_eq!(state.num_run_attempts, 1);

    assert_eq!(try_run_from_battle(&mut state, 60), RunResult::Escaped);
    assert_eq!(state.num_run_attempts, 2);
}

#[test]
fn run_overflow_guarantees_escape() {
    // player_speed=50, enemy_speed=200, quotient=32
    // After ~8 attempts, 32 + 30*7 = 242, then 242+30 overflows → escape
    let mut state = make_state(50, 200);
    for _ in 0..7 {
        let _ = try_run_from_battle(&mut state, 255);
    }
    // 8th attempt: 32 + 30*7 = 242, random=255 → 242 < 255 → fail
    assert_eq!(
        try_run_from_battle(&mut state, 255),
        RunResult::FailedToEscape
    );
    // 9th attempt: 32 + 30*8 = 272 → overflow → escape
    assert_eq!(try_run_from_battle(&mut state, 255), RunResult::Escaped);
}

#[test]
fn run_divisor_zero_escapes() {
    // enemy_speed=1024: (1024/4)%256 = 256%256 = 0 → always escape
    let mut state = make_state(50, 1024);
    assert_eq!(try_run_from_battle(&mut state, 255), RunResult::Escaped);
}

#[test]
fn run_quotient_overflow_escapes() {
    // enemy_speed=1028, player_speed=1027
    // divisor = (1028/4)%256 = 257%256 = 1
    // quotient = 1027*32/1 = 32864 ≥ 256 → escape
    let mut state = make_state(1027, 1028);
    assert_eq!(try_run_from_battle(&mut state, 255), RunResult::Escaped);
}

// ==================== try_escape_move ====================

#[test]
fn escape_move_trainer_battle_fails() {
    let mut state = new_battle_state(
        BattleType::Trainer,
        vec![make_pokemon(50, 100)],
        vec![make_pokemon(25, 50)],
    );
    assert_eq!(
        try_escape_move(&mut state, Side::Player, 0),
        TeleportResult::Failed
    );
    assert!(!state.escaped);
}

#[test]
fn escape_move_user_higher_level_succeeds() {
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![make_pokemon(50, 100)],
        vec![make_pokemon(25, 50)],
    );
    assert_eq!(
        try_escape_move(&mut state, Side::Player, 255),
        TeleportResult::Success
    );
    assert!(state.escaped);
}

#[test]
fn escape_move_equal_level_succeeds() {
    let mut state = make_state(100, 100);
    assert_eq!(
        try_escape_move(&mut state, Side::Player, 255),
        TeleportResult::Success
    );
}

#[test]
fn escape_move_lower_level_can_succeed() {
    // player_level=10, enemy_level=20
    // range = 10 + 20 + 1 = 31
    // threshold = 20/4 = 5
    // sampled = random_value % 31
    // need sampled >= 5
    // random_value=20: 20%31=20 >= 5 → success
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![make_pokemon(10, 100)],
        vec![make_pokemon(20, 50)],
    );
    assert_eq!(
        try_escape_move(&mut state, Side::Player, 20),
        TeleportResult::Success
    );
}

#[test]
fn escape_move_lower_level_can_fail() {
    // player_level=10, enemy_level=20
    // threshold = 5
    // random_value=3: 3%31=3 < 5 → fail
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![make_pokemon(10, 100)],
        vec![make_pokemon(20, 50)],
    );
    assert_eq!(
        try_escape_move(&mut state, Side::Player, 3),
        TeleportResult::Failed
    );
    assert!(!state.escaped);
}

#[test]
fn escape_move_enemy_side_works() {
    // Enemy uses Whirlwind. enemy_level=30, player_level=20 → enemy >= player → success
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![make_pokemon(20, 100)],
        vec![make_pokemon(30, 50)],
    );
    assert_eq!(
        try_escape_move(&mut state, Side::Enemy, 255),
        TeleportResult::Success
    );
    assert!(state.escaped);
}

#[test]
fn escape_move_enemy_lower_level_can_fail() {
    // Enemy level=10, player level=20
    // threshold = player_level/4 = 5
    // range = 10+20+1 = 31
    // random_value=1: 1%31=1 < 5 → fail
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![make_pokemon(20, 100)],
        vec![make_pokemon(10, 50)],
    );
    assert_eq!(
        try_escape_move(&mut state, Side::Enemy, 1),
        TeleportResult::Failed
    );
}

#[test]
fn run_increments_attempt_counter() {
    let mut state = make_state(50, 200);
    let _ = try_run_from_battle(&mut state, 255);
    assert_eq!(state.num_run_attempts, 1);
    let _ = try_run_from_battle(&mut state, 255);
    assert_eq!(state.num_run_attempts, 2);
    let _ = try_run_from_battle(&mut state, 255);
    assert_eq!(state.num_run_attempts, 3);
}
