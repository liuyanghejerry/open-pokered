//! Rage requires a survived damaging action (even enemy Psywave's zero);
//! its activation is an on-hit effect, cancelled by a Substitute break.
use pokered_core::battle::state::{status2, BattlerState, StatusCondition};
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species, types::PokemonType};

const HITS: [MoveId; 10] = [
    MoveId::Tackle,
    MoveId::Rage,
    MoveId::HyperBeam,
    MoveId::DoubleEdge,
    MoveId::Struggle,
    MoveId::DragonRage,
    MoveId::Sonicboom,
    MoveId::SeismicToss,
    MoveId::NightShade,
    MoveId::SuperFang,
];

fn side(b: &mut BattleScreen, enemy: bool) -> &mut BattlerState {
    let s = b.battle_state.as_mut().unwrap();
    if enemy {
        &mut s.enemy
    } else {
        &mut s.player
    }
}

fn fixed(id: MoveId) -> bool {
    matches!(
        id,
        MoveId::DragonRage
            | MoveId::Sonicboom
            | MoveId::SeismicToss
            | MoveId::NightShade
            | MoveId::SuperFang
    )
}

fn setup(id: MoveId, enemy: bool, initial: i8, species: Species) -> BattleScreen {
    let a = create_pokemon_with_moves(
        Species::Mewtwo,
        20,
        [0x9a, 0x78],
        [id, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let mut t = create_pokemon_with_moves(
        species,
        100,
        [0x9a, 0x78],
        [MoveId::Rage, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    t.status = StatusCondition::Sleep(2);
    let (p, e) = if enemy { (t, a) } else { (a, t) };
    let mut b = BattleScreen::from_parties(true, &[p], &[e], None);
    b.link_enemy_move_override = Some((if enemy { id } else { MoveId::Rage }, 0));
    b.link_rng = Some(LinkRng::new(if fixed(id) {
        [0, 255, 255, 255, 255, 255, 255, 255, 255, 255]
    } else {
        [255, 255, 0, 255, 255, 255, 255, 255, 255, 255]
    }));
    side(&mut b, enemy).active_mon_mut().speed = 100;
    let target = side(&mut b, !enemy);
    target.active_mon_mut().speed = 200;
    target.battle_status2 |= status2::USING_RAGE;
    target.selected_move = MoveId::Rage;
    target.stat_stages.attack = initial;
    b.set_presentation_enabled(true);
    b.phase = BattlePhase::PlayerMenu;
    b
}

fn fight(b: &mut BattleScreen) -> Vec<String> {
    let mut a = BattleInput::none();
    a.a = true;
    for _ in 0..4 {
        b.update_frame(a);
        if let BattlePhase::ShowingText { messages, .. } = &b.phase {
            return messages.clone();
        }
    }
    panic!("FIGHT did not resolve: {:?}", b.phase);
}

fn normalized(messages: &[String]) -> Vec<String> {
    messages
        .iter()
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

fn target_rises(messages: &[String]) -> usize {
    normalized(messages)
        .iter()
        .filter(|s| {
            (s.contains("SNORLAX") || s.contains("DRAGONITE")) && s.contains("ATTACK rose!")
        })
        .count()
}

#[test]
fn ordinary_and_fixed_damage_hits_build_rage_once_until_the_stage_cap() {
    for id in HITS {
        for enemy in [false, true] {
            for initial in [0, 5, 6] {
                let mut b = setup(id, enemy, initial, Species::Snorlax);
                let hp = side(&mut b, !enemy).active_mon().hp;
                let messages = fight(&mut b);
                let gains = usize::from(initial < 6);
                assert!(
                    side(&mut b, !enemy).active_mon().hp < hp,
                    "{id:?} {messages:?}"
                );
                assert_eq!(
                    side(&mut b, !enemy).stat_stages.attack,
                    initial + gains as i8,
                    "{id:?} enemy={enemy} initial={initial} {messages:?}"
                );
                assert_eq!(
                    messages
                        .iter()
                        .filter(|s| s.contains("RAGE is building!"))
                        .count(),
                    gains,
                    "{id:?} {messages:?}"
                );
                assert_eq!(target_rises(&messages), gains, "{id:?} {messages:?}");
            }
        }
    }
}

#[test]
fn knockout_hits_cannot_build_rage() {
    for id in HITS {
        for enemy in [false, true] {
            let mut b = setup(id, enemy, 0, Species::Snorlax);
            side(&mut b, !enemy).active_mon_mut().hp = 1;
            let messages = fight(&mut b);
            assert_eq!(
                side(&mut b, !enemy).active_mon().hp,
                0,
                "{id:?} {messages:?}"
            );
            assert_eq!(
                side(&mut b, !enemy).stat_stages.attack,
                0,
                "{id:?} {messages:?}"
            );
            assert!(messages.iter().all(|s| !s.contains("RAGE is building!")));
            assert_eq!(target_rises(&messages), 0);
            if id == MoveId::Rage {
                assert_ne!(
                    side(&mut b, enemy).battle_status2 & status2::USING_RAGE,
                    0,
                    "A successful KO still installs the user's Rage lock"
                );
            }
        }
    }
}

#[test]
fn enemy_psywaves_successful_zero_damage_still_builds_rage() {
    for enemy in [false, true] {
        for substitute in [false, true] {
            let mut b = setup(MoveId::Psywave, enemy, 0, Species::Snorlax);
            // Player rejects damage zero then takes 1; enemy accepts zero.
            b.link_rng = Some(LinkRng::new([0, 0, 1, 255, 255, 255, 255, 255, 255, 255]));
            if substitute {
                side(&mut b, !enemy).battle_status2 |= status2::HAS_SUBSTITUTE_UP;
                side(&mut b, !enemy).substitute_hp = 10;
            }
            let hp = side(&mut b, !enemy).active_mon().hp;
            let messages = fight(&mut b);
            assert_eq!(
                side(&mut b, !enemy).active_mon().hp,
                hp - u16::from(!enemy && !substitute)
            );
            assert_eq!(side(&mut b, !enemy).stat_stages.attack, 1, "{messages:?}");
            assert_eq!(target_rises(&messages), 1);
            assert_eq!(
                messages
                    .iter()
                    .filter(|s| s.contains("RAGE is building!"))
                    .count(),
                1
            );
            assert_eq!(
                b.link_rng.as_ref().unwrap().consumed(),
                if enemy { 2 } else { 3 }
            );
        }
    }
}

#[test]
fn immune_and_missed_hits_cannot_build_rage() {
    for enemy in [false, true] {
        for (id, kind) in [
            (MoveId::Tackle, PokemonType::Ghost),
            (MoveId::Rage, PokemonType::Ghost),
            (MoveId::Thunderbolt, PokemonType::Ground),
            (MoveId::LowKick, PokemonType::Ghost),
        ] {
            for immune in [false, true] {
                let mut b = setup(id, enemy, 0, Species::Snorlax);
                if immune {
                    let target = side(&mut b, !enemy);
                    target.conversion_type1 = Some(kind);
                    target.conversion_type2 = Some(kind);
                } else {
                    b.link_rng = Some(LinkRng::new([255; 10]));
                }
                let hp = side(&mut b, !enemy).active_mon().hp;
                let messages = fight(&mut b);
                assert_eq!(side(&mut b, !enemy).active_mon().hp, hp);
                assert_eq!(side(&mut b, !enemy).stat_stages.attack, 0, "{messages:?}");
                assert_eq!(target_rises(&messages), 0);
                assert!(messages.iter().all(|s| !s.contains("RAGE is building!")));
            }
        }
    }
}

#[test]
fn status_and_charge_moves_do_not_build_rage_or_hide_growls_actual_drop() {
    for enemy in [false, true] {
        for id in [
            MoveId::Growl,
            MoveId::Leer,
            MoveId::TailWhip,
            MoveId::Splash,
            MoveId::SwordsDance,
            MoveId::FocusEnergy,
            MoveId::Fly,
            MoveId::Dig,
        ] {
            let mut b = setup(id, enemy, 0, Species::Snorlax);
            // Enemy stat-down moves pass their extra Gen-1 failure roll BEFORE
            // accuracy. Charge/status moves still do no damage.
            b.link_rng = Some(LinkRng::new(if enemy {
                [64, 0, 255, 255, 255, 255, 255, 255, 255, 255]
            } else {
                [0, 255, 255, 255, 255, 255, 255, 255, 255, 255]
            }));
            let hp = side(&mut b, !enemy).active_mon().hp;
            let messages = fight(&mut b);
            assert_eq!(side(&mut b, !enemy).active_mon().hp, hp, "{id:?}");
            assert_eq!(
                side(&mut b, !enemy).stat_stages.attack,
                if id == MoveId::Growl { -1 } else { 0 },
                "{id:?} {messages:?}"
            );
            assert_eq!(target_rises(&messages), 0, "{id:?} {messages:?}");
            assert!(
                messages.iter().all(|s| !s.contains("RAGE is building!")),
                "{id:?} {messages:?}"
            );
            if id == MoveId::Growl {
                assert!(normalized(&messages)
                    .iter()
                    .any(|s| s.contains("ATTACK fell!")));
            }
        }
    }
}

#[test]
fn attack_999_preserves_the_stage_and_does_not_repeat_burn_penalties() {
    for enemy in [false, true] {
        for initial in [3, 4, 5, 6] {
            for burned in [false, true] {
                let make = || setup(MoveId::Tackle, enemy, initial, Species::Dragonite);
                let mut control = make();
                side(&mut control, !enemy).battle_status2 &= !status2::USING_RAGE;
                if burned {
                    side(&mut control, enemy).active_mon_mut().status = StatusCondition::Burn;
                }
                fight(&mut control);
                let baseline = side(&mut control, enemy).staged_badge_stats.unwrap()[0];
                let mut b = make();
                if burned {
                    side(&mut b, enemy).active_mon_mut().status = StatusCondition::Burn;
                }
                let messages = fight(&mut b);
                let gain = initial < 5;
                assert_eq!(
                    side(&mut b, !enemy).stat_stages.attack,
                    initial + i8::from(gain)
                );
                assert_eq!(
                    side(&mut b, !enemy).staged_badge_stats.unwrap()[0],
                    if initial == 3 { 873 } else { 999 }
                );
                assert_eq!(
                    messages
                        .iter()
                        .filter(|s| s.contains("RAGE is building!"))
                        .count(),
                    usize::from(initial < 6),
                    "{messages:?}"
                );
                assert_eq!(target_rises(&messages), usize::from(gain));
                assert_eq!(
                    messages
                        .iter()
                        .filter(|s| s.as_str() == "Nothing happened!")
                        .count(),
                    usize::from(initial == 5)
                );
                assert_eq!(
                    side(&mut b, enemy).staged_badge_stats.unwrap()[0],
                    if gain && burned {
                        (baseline / 2).max(1)
                    } else {
                        baseline
                    }
                );
                assert_eq!(
                    b.link_rng.as_ref().unwrap().consumed(),
                    control.link_rng.as_ref().unwrap().consumed()
                );
            }
        }
    }
}

#[test]
fn substitute_hits_build_rage_but_a_break_cancels_new_rage_activation() {
    for enemy in [false, true] {
        for id in [
            MoveId::Tackle,
            MoveId::Rage,
            MoveId::DragonRage,
            MoveId::SuperFang,
        ] {
            for doll in [1, 200] {
                let mut b = setup(id, enemy, 0, Species::Snorlax);
                side(&mut b, !enemy).battle_status2 |= status2::HAS_SUBSTITUTE_UP;
                side(&mut b, !enemy).substitute_hp = doll;
                let hp = side(&mut b, !enemy).active_mon().hp;
                let messages = fight(&mut b);
                assert_eq!(side(&mut b, !enemy).active_mon().hp, hp);
                assert_eq!(side(&mut b, !enemy).stat_stages.attack, 1, "{messages:?}");
                assert_eq!(target_rises(&messages), 1);
                assert_eq!(
                    messages
                        .iter()
                        .filter(|s| s.contains("RAGE is building!"))
                        .count(),
                    1
                );
                assert_eq!(
                    side(&mut b, enemy).battle_status2 & status2::USING_RAGE != 0,
                    id == MoveId::Rage && doll > 1,
                    "{messages:?}"
                );
            }
        }
    }
}

#[test]
fn an_immune_rage_does_not_lock_a_new_user_or_gain_attack_from_splash() {
    for enemy in [false, true] {
        for already_raging in [false, true] {
            let mut b = setup(MoveId::Rage, enemy, 0, Species::Snorlax);
            if already_raging {
                let actor = side(&mut b, enemy);
                actor.battle_status2 |= status2::USING_RAGE;
                actor.selected_move = MoveId::Rage;
            }
            let target = side(&mut b, !enemy);
            target.battle_status2 &= !status2::USING_RAGE;
            target.active_mon_mut().status = StatusCondition::None;
            target.active_mon_mut().speed = 50;
            target.active_mon_mut().moves[0] = MoveId::Splash;
            target.conversion_type1 = Some(PokemonType::Ghost);
            target.conversion_type2 = Some(PokemonType::Ghost);
            if !enemy {
                b.link_enemy_move_override = Some((MoveId::Splash, 0));
            }
            let messages = fight(&mut b);
            assert!(
                messages.iter().any(|s| s.contains("SPLASH!")),
                "{messages:?}"
            );
            assert_eq!(
                side(&mut b, enemy).battle_status2 & status2::USING_RAGE != 0,
                already_raging,
                "{messages:?}"
            );
            assert_eq!(side(&mut b, enemy).stat_stages.attack, 0, "{messages:?}");
            assert!(
                messages
                    .iter()
                    .all(|s| !s.contains("RAGE is building!") && !s.contains("ATTACK")),
                "{messages:?}"
            );
        }
    }
}
