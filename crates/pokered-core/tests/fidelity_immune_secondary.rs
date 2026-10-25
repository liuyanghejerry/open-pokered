//! The missed-move return precedes ordinary secondary-effect dispatch.
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::battle::state::{status2, BattlerState, StatusCondition};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species, types::PokemonType};

const MOVES: [MoveId; 12] = [MoveId::BodySlam, MoveId::Lick, MoveId::Thundershock,
    MoveId::Thunderbolt, MoveId::Thunder, MoveId::Thunderpunch, MoveId::Stomp,
    MoveId::Bite, MoveId::Headbutt, MoveId::LowKick, MoveId::HyperFang, MoveId::BoneClub];

fn side(b: &mut BattleScreen, enemy: bool) -> &mut BattlerState {
    let s = b.battle_state.as_mut().unwrap();
    if enemy { &mut s.enemy } else { &mut s.player }
}

fn immune_type(id: MoveId) -> PokemonType {
    match id {
        MoveId::Lick => PokemonType::Normal,
        MoveId::Thundershock | MoveId::Thunderbolt | MoveId::Thunder | MoveId::Thunderpunch => PokemonType::Ground,
        MoveId::BoneClub => PokemonType::Flying,
        _ => PokemonType::Ghost,
    }
}

fn setup(id: MoveId, enemy: bool, kind: PokemonType, doll: bool) -> BattleScreen {
    let a = create_pokemon_with_moves(Species::Mewtwo, 20, [0x9a, 0x78],
        [id, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    let t = create_pokemon_with_moves(Species::Snorlax, 100, [0x9a, 0x78],
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    let (p, e) = if enemy { (t, a) } else { (a, t) };
    let mut b = BattleScreen::from_parties(true, &[p], &[e], None);
    b.link_enemy_move_override = Some((if enemy { id } else { MoveId::Splash }, 0));
    b.link_rng = Some(LinkRng::new(if kind == immune_type(id) {
        [255, 0, 255, 0, 255, 255, 255, 255, 255, 255]
    } else {
        [255, 255, 0, 0, 255, 255, 255, 255, 255, 255]
    }));
    side(&mut b, enemy).active_mon_mut().speed = 200;
    let target = side(&mut b, !enemy);
    target.active_mon_mut().speed = 100;
    target.conversion_type1 = Some(kind);
    target.conversion_type2 = Some(kind);
    if doll { target.battle_status2 |= status2::HAS_SUBSTITUTE_UP; target.substitute_hp = 200; }
    b.set_presentation_enabled(true);
    b.phase = BattlePhase::PlayerMenu;
    b
}

fn fight(b: &mut BattleScreen) -> Vec<String> {
    let mut a = BattleInput::none(); a.a = true;
    for _ in 0..4 {
        b.update_frame(a);
        if let BattlePhase::ShowingText { messages, .. } = &b.phase { return messages.clone(); }
    }
    panic!("FIGHT did not resolve: {:?}", b.phase);
}

fn drain(b: &mut BattleScreen) {
    for frame in 0..4000 {
        if b.presentation.waiting { b.complete_move_presentation(); }
        if matches!(b.phase, BattlePhase::PlayerMenu) { return; }
        let mut a = BattleInput::none(); a.a = frame % 2 == 0; b.update_frame(a);
    }
    panic!("presentation did not finish: {:?}", b.phase);
}

#[test]
fn immune_targets_cannot_be_paralyzed_or_flinched_or_consume_secondary_rng() {
    for id in MOVES {
        for enemy in [false, true] {
            for doll in [false, true] {
                // An immune ordinary move supplies a relative RNG control;
                // the overall original RNG pipeline is audited separately.
                let mut control = setup(MoveId::Tackle, enemy, PokemonType::Ghost, doll);
                fight(&mut control);
                let draws = control.link_rng.as_ref().unwrap().consumed();
                drain(&mut control);
                let mut b = setup(id, enemy, immune_type(id), doll);
                let hp = side(&mut b, !enemy).active_mon().hp;
                let messages = fight(&mut b);
                assert_eq!(side(&mut b, !enemy).active_mon().hp, hp);
                assert_eq!(side(&mut b, !enemy).active_mon().status, StatusCondition::None, "{id:?} {messages:?}");
                assert_eq!(side(&mut b, !enemy).substitute_hp, if doll { 200 } else { 0 });
                assert!(messages.iter().any(|s| s.contains("doesn't affect")), "{messages:?}");
                assert!(messages.iter().any(|s| s.contains("SPLASH!")), "target must act: {messages:?}");
                assert!(messages.iter().all(|s| !s.contains("paralyzed") && !s.contains("flinched")), "{messages:?}");
                assert_eq!(b.link_rng.as_ref().unwrap().consumed(), draws, "{id:?} enemy={enemy} doll={doll}");
                drain(&mut b);
            }
        }
    }
}

#[test]
fn ordinary_misses_still_leave_the_target_healthy_and_able_to_act() {
    for id in MOVES {
        for enemy in [false, true] {
            let mut b = setup(id, enemy, PokemonType::Water, false);
            b.link_rng = Some(LinkRng::new([255; 10]));
            let hp = side(&mut b, !enemy).active_mon().hp;
            let messages = fight(&mut b);
            assert_eq!(side(&mut b, !enemy).active_mon().hp, hp);
            assert_eq!(side(&mut b, !enemy).active_mon().status, StatusCondition::None);
            assert!(messages.iter().any(|s| s.contains("missed")), "{messages:?}");
            assert!(messages.iter().any(|s| s.contains("SPLASH!")), "{messages:?}");
            assert!(messages.iter().all(|s| !s.contains("paralyzed") && !s.contains("flinched")));
            drain(&mut b);
        }
    }
}

#[test]
fn successful_hits_keep_their_paralysis_and_flinch_effects() {
    for id in [MoveId::BodySlam, MoveId::Thunderbolt, MoveId::LowKick] {
        for enemy in [false, true] {
            let mut b = setup(id, enemy, PokemonType::Water, false);
            let hp = side(&mut b, !enemy).active_mon().hp;
            let messages = fight(&mut b);
            assert!(side(&mut b, !enemy).active_mon().hp < hp);
            if id == MoveId::LowKick {
                assert!(messages.iter().any(|s| s.contains("flinched")), "{messages:?}");
                assert!(messages.iter().all(|s| !s.contains("SPLASH!")), "{messages:?}");
            } else {
                assert_eq!(side(&mut b, !enemy).active_mon().status, StatusCondition::Paralysis);
                assert!(messages.iter().any(|s| s.contains("paralyzed")), "{messages:?}");
            }
            drain(&mut b);
        }
    }
}
