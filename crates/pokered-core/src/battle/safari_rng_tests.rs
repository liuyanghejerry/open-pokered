//! Decision-draw boundaries from ItemUseBall/BaitRockCommon and the Safari
//! caller in core.asm; the original's background VBlank Random is out of scope.

use super::*;
use crate::battle::capture::{try_capture, CaptureContext, CaptureRandoms, CaptureResult};
use crate::battle::pokered_rules::runtime::StdBattleRng;
use crate::battle::safari::SafariState;
use crate::pokemon::stats::create_pokemon;
use crate::snapshot::BattleSnapshot;

fn battle(rate: u8, balls: u8, speed: u16) -> BattleScreen {
    let player = create_pokemon(Species::Rattata, 12, [0x9a, 0x78]).unwrap();
    let mut enemy = create_pokemon(Species::NidoranM, 22, [0x9a, 0x78]).unwrap();
    enemy.speed = speed;
    let mut s = BattleScreen::from_parties(true, &[player], &[enemy], None);
    s.phase = BattlePhase::PlayerMenu;
    s.is_safari = true;
    s.safari = Some(SafariState::new(rate, balls));
    s.safari_menu = menu::SafariBattleMenuState::new(balls);
    s
}

fn has_flee_text(s: &BattleScreen) -> bool {
    matches!(&s.phase, BattlePhase::ShowingText { messages, .. }
        if messages.iter().any(|m| m.contains("ran away!")))
}

fn rng_value(rng: &StdBattleRng) -> serde_json::Value {
    serde_json::to_value(rng).unwrap()
}

fn ball_rolls(
    rate: u8,
    max_hp: u16,
    hp: u16,
    status: StatusCondition,
    rolls: &[u8],
) -> CaptureResult {
    let mut s = SafariState::new(rate, 30);
    let mut bytes = rolls.iter().copied();
    let result = s.throw_ball_with_rng(max_hp, hp, status, &mut || {
        bytes.next().expect("extra Safari capture draw")
    });
    assert!(bytes.next().is_none(), "expected draw was skipped");
    assert_eq!(s.balls, 29);
    result
}

#[test]
fn safari_rng_ball_rejects_rand1_then_conditionally_draws_rand2() {
    assert_eq!(
        ball_rolls(45, 100, 100, StatusCondition::None, &[255, 151, 45, 85]),
        CaptureResult::Captured
    );
    assert!(matches!(
        ball_rolls(45, 100, 100, StatusCondition::None, &[45, 86]),
        CaptureResult::Failed { .. }
    ));
    assert!(matches!(
        ball_rolls(149, 100, 100, StatusCondition::None, &[255, 151, 150]),
        CaptureResult::Failed { .. }
    ));
    assert_eq!(
        ball_rolls(255, 100, 100, StatusCondition::Sleep(3), &[255, 24]),
        CaptureResult::Captured
    );
    assert_eq!(
        ball_rolls(255, 100, 1, StatusCondition::None, &[150]),
        CaptureResult::Captured
    );
    assert_eq!(
        ball_rolls(255, 12, 4, StatusCondition::None, &[0, 255]),
        CaptureResult::Captured
    );
    assert_eq!(
        ball_rolls(255, 13, 4, StatusCondition::None, &[0]),
        CaptureResult::Captured
    );
}

#[test]
fn safari_rng_injected_ball_and_flee_apis_keep_their_results() {
    for rate in [3, 45, 150, 255] {
        for rand1 in 0..=150 {
            for rand2 in [0, 85, 86, 255] {
                let mut old = SafariState::new(rate, 30);
                let old_result =
                    old.throw_ball(100, 100, StatusCondition::None, rand2, &mut || rand1);
                let ctx = CaptureContext {
                    ball: ItemId::SafariBall,
                    wild_max_hp: 100,
                    wild_current_hp: 100,
                    wild_catch_rate: rate,
                    wild_status: StatusCondition::None,
                };
                assert_eq!(
                    old_result,
                    try_capture(&ctx, &CaptureRandoms { rand1, rand2 })
                );
                assert_eq!(old.balls, 29);
            }
        }
    }
    let s = SafariState::new(100, 30);
    for speed in [0, 50, 127, 128, 200, 306] {
        for byte in [0, 99, 100, 254, 255] {
            assert_eq!(
                s.flee_roll(speed, byte),
                s.flee_roll_with_rng(speed, || byte)
            );
        }
    }
}

#[test]
fn safari_rng_flee_carry_skips_draw_but_zero_speed_draws() {
    let s = SafariState::new(100, 30);
    assert!(s.flee_roll_with_rng(128, || panic!("carry must skip Random")));
    assert!(s.flee_roll_with_rng(200, || panic!("carry must skip Random")));
    let mut draws = 0;
    assert!(!s.flee_roll_with_rng(0, || {
        draws += 1;
        0
    }));
    assert_eq!(draws, 1, "zero threshold still reaches Random");
    assert!(
        !s.flee_roll_with_rng(306, || 100),
        "only the speed low byte is doubled"
    );
    assert!(s.flee_roll_with_rng(50, || 99));
    assert!(
        !s.flee_roll_with_rng(50, || 100),
        "comparison is strictly less than"
    );
}

fn reference_turn(
    s: &BattleScreen,
    action: menu::SafariMenuAction,
) -> (StdBattleRng, SafariState, bool, bool) {
    use menu::SafariMenuAction;
    let mut rng = s.rng.clone();
    let mut safari = s.safari.clone().unwrap();
    if action == SafariMenuAction::Run {
        return (rng, safari, false, false);
    }
    let enemy = s.battle_state.as_ref().unwrap().enemy.active_mon();
    let mut caught = false;
    match action {
        SafariMenuAction::Ball => {
            safari.balls -= 1;
            let rand1 = loop {
                let byte = rng.next_u8();
                if byte <= 150 {
                    break byte;
                }
            };
            let w = u32::from(enemy.max_hp) * 255 / 12 / u32::from((enemy.hp / 4).max(1));
            assert_eq!(enemy.status, StatusCondition::None);
            let rand2 = if rand1 <= safari.catch_rate && w <= 255 {
                rng.next_u8()
            } else {
                0
            };
            let ctx = CaptureContext {
                ball: ItemId::SafariBall,
                wild_max_hp: enemy.max_hp,
                wild_current_hp: enemy.hp,
                wild_catch_rate: safari.catch_rate,
                wild_status: enemy.status,
            };
            caught = try_capture(&ctx, &CaptureRandoms { rand1, rand2 }) == CaptureResult::Captured;
        }
        SafariMenuAction::Bait | SafariMenuAction::Rock => {
            let amount = loop {
                let v = rng.next_u8() & 7;
                if v < 5 {
                    break v + 1;
                }
            };
            if action == SafariMenuAction::Bait {
                safari.apply_bait(amount);
            } else {
                safari.apply_rock(amount);
            }
        }
        SafariMenuAction::Run => unreachable!(),
    }
    if caught || safari.balls == 0 {
        return (rng, safari, caught, false);
    }
    safari.upkeep();
    let fled = if enemy.speed as u8 > 127 {
        true
    } else {
        safari.flee_roll(enemy.speed, rng.next_u8())
    };
    (rng, safari, caught, fled)
}

#[test]
fn safari_rng_canonical_actions_advance_owned_stream_and_replay_serialized_snapshot() {
    use menu::SafariMenuAction;
    let mut outcomes = [false; 2];
    for entropy in [false, true] {
        for seed in 0..16 {
            for action in [
                SafariMenuAction::Ball,
                SafariMenuAction::Bait,
                SafariMenuAction::Rock,
            ] {
                let mut s = battle(255, 30, 27);
                s.rng = if entropy {
                    StdBattleRng::from_entropy()
                } else {
                    StdBattleRng::from_seed(seed)
                };
                let (rng, expected, caught, fled) = reference_turn(&s, action);
                if !entropy && action == SafariMenuAction::Ball {
                    outcomes[usize::from(caught)] = true;
                }
                let bytes = serde_json::to_vec(&BattleSnapshot::capture(&s)).unwrap();
                let before: BattleSnapshot = serde_json::from_slice(&bytes).unwrap();
                let mut first = None;
                for _ in 0..4 {
                    before.restore_into(&mut s);
                    s.resolve_safari_action(action);
                    assert_eq!(rng_value(&s.rng), rng_value(&rng));
                    assert_eq!(s.safari.as_ref(), Some(&expected));
                    assert_eq!(s.captured_mon.is_some(), caught);
                    assert_eq!(has_flee_text(&s), fled);
                    let after = serde_json::to_value(BattleSnapshot::capture(&s)).unwrap();
                    if let Some(first) = &first {
                        assert_eq!(&after, first);
                    } else {
                        first = Some(after);
                    }
                }
            }
        }
    }
    assert_eq!(
        outcomes,
        [true, true],
        "seeded capture success and failure both covered"
    );
}

#[test]
fn safari_rng_run_cancel_and_full_box_do_not_draw_or_spend() {
    for case in ["run", "cancel", "full-box"] {
        let mut s = battle(255, 30, 27);
        s.rng = StdBattleRng::from_seed(3);
        let before = rng_value(&s.rng);
        let safari = s.safari.clone();
        match case {
            "run" => s.resolve_safari_action(menu::SafariMenuAction::Run),
            "cancel" => {
                s.update_frame(BattleInput {
                    b: true,
                    ..BattleInput::none()
                });
            }
            _ => {
                let mon = s.battle_state.as_ref().unwrap().player.party[0];
                s.battle_state.as_mut().unwrap().player.party = vec![mon; 6];
                s.player_box_full = true;
                s.resolve_safari_action(menu::SafariMenuAction::Ball);
            }
        }
        assert_eq!(rng_value(&s.rng), before, "{case}");
        assert_eq!(s.safari, safari, "{case}");
        assert!(s.captured_mon.is_none());
    }
}

#[test]
fn safari_rng_caught_and_last_ball_stop_before_upkeep_or_flee() {
    for caught in [false, true] {
        let mut s = battle(if caught { 255 } else { 0 }, 1, 27);
        if caught {
            s.battle_state.as_mut().unwrap().enemy.active_mon_mut().hp = 1;
        }
        let seed = (0..100)
            .find(|seed| {
                let mut rng = StdBattleRng::from_seed(*seed);
                loop {
                    let byte = rng.next_u8();
                    if byte <= 150 {
                        break byte > 0;
                    }
                }
            })
            .unwrap();
        s.rng = StdBattleRng::from_seed(seed);
        s.safari.as_mut().unwrap().bait_factor = 2;
        let (rng, expected, expected_caught, fled) =
            reference_turn(&s, menu::SafariMenuAction::Ball);
        assert_eq!(caught, expected_caught);
        assert!(!fled);
        s.resolve_safari_action(menu::SafariMenuAction::Ball);
        assert_eq!(rng_value(&s.rng), rng_value(&rng));
        assert_eq!(s.safari.as_ref(), Some(&expected));
        assert_eq!(
            s.safari.as_ref().unwrap().bait_factor,
            2,
            "no upkeep on final ball/catch"
        );
        assert_eq!(s.captured_mon.is_some(), caught);
    }
}

#[test]
fn safari_rng_upkeep_expires_counters_before_flee_and_carry_never_draws() {
    for counter in ["bait1", "anger1", "carry", "zero"] {
        let speed = if counter == "carry" {
            128
        } else if counter == "zero" {
            0
        } else {
            50
        };
        let mut s = battle(0, 30, speed);
        let seed = (0..1000)
            .find(|seed| {
                let mut rng = StdBattleRng::from_seed(*seed);
                let rand1 = loop {
                    let v = rng.next_u8();
                    if v <= 150 {
                        break v;
                    }
                };
                let flee = rng.next_u8();
                rand1 > 1
                    && match counter {
                        "bait1" => (25..100).contains(&flee),
                        "anger1" => (100..200).contains(&flee),
                        _ => true,
                    }
            })
            .unwrap();
        s.rng = StdBattleRng::from_seed(seed);
        let safari = s.safari.as_mut().unwrap();
        if counter == "bait1" {
            safari.bait_factor = 1;
        }
        if counter == "anger1" {
            safari.catch_rate = 1;
            safari.escape_factor = 1;
        }
        let (rng, expected, caught, fled) = reference_turn(&s, menu::SafariMenuAction::Ball);
        assert!(!caught);
        s.resolve_safari_action(menu::SafariMenuAction::Ball);
        assert_eq!(rng_value(&s.rng), rng_value(&rng), "{counter}");
        assert_eq!(s.safari.as_ref(), Some(&expected), "{counter}");
        assert_eq!(has_flee_text(&s), fled, "{counter}");
        if counter == "bait1" {
            assert_eq!(expected.bait_factor, 0);
        }
        if counter == "anger1" {
            assert_eq!((expected.escape_factor, expected.catch_rate), (0, 0));
        }
    }
}
