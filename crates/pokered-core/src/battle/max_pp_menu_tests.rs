use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;

fn mon(species: Species, moves: [MoveId; 4]) -> state::Pokemon {
    create_pokemon_with_moves(species, 20, [0x9a, 0x78], moves).unwrap()
}

fn screen(player: Vec<state::Pokemon>) -> BattleScreen {
    let enemy = mon(
        Species::Weedle,
        [
            MoveId::PoisonSting,
            MoveId::StringShot,
            MoveId::None,
            MoveId::None,
        ],
    );
    let mut screen = BattleScreen::from_parties(false, &player, &[enemy], None);
    screen.rng = pokered_rules::runtime::StdBattleRng::from_seed(42);
    screen
}

#[test]
fn max_pp_menu_matches_gen1_bonus_and_seven_pp_cap() {
    // GetMaxPP/AddBonusPP: floor(base/5), capped at seven per PP Up.
    // Splash's base 40 must end at 61, rather than wrapping packed 64 to zero.
    for (move_id, expected) in [
        (MoveId::Thundershock, [30, 36, 42, 48]),
        (MoveId::Splash, [40, 47, 54, 61]),
    ] {
        for (ups, maximum) in expected.into_iter().enumerate() {
            let mut pokemon = mon(
                Species::Pikachu,
                [move_id, MoveId::None, MoveId::None, MoveId::None],
            );
            pokemon.pp_ups[0] = ups as u8;
            pokemon.pp[0] = 0;
            let screen = screen(vec![pokemon.clone()]);
            let fight =
                BattleScreen::build_move_menu_from_state(screen.battle_state.as_ref().unwrap());
            let item = BattleScreen::build_move_menu_for_mon(&pokemon);
            for menu in [fight, item] {
                assert_eq!(menu.moves()[0].max_pp, maximum);
                assert_eq!(
                    menu.moves()[0].current_pp,
                    0,
                    "empty PP does not erase PP Ups"
                );
            }
        }
    }
}

#[test]
fn max_pp_menu_reads_active_slot_without_compacting_pp_up_indices() {
    let mut active = mon(
        Species::Pikachu,
        [
            MoveId::Thundershock,
            MoveId::None,
            MoveId::PoisonSting,
            MoveId::Splash,
        ],
    );
    active.pp = [36, 0, 11, 61];
    active.pp_ups = [1, 0, 1, 3];
    let bench = mon(
        Species::Bulbasaur,
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    );
    let mut screen = screen(vec![bench, active.clone()]);
    let bs = screen.battle_state.as_mut().unwrap();
    bs.player.active_pokemon_index = 1;
    bs.player.disabled_move = 3;
    let fight = BattleScreen::build_move_menu_from_state(bs);
    let item = BattleScreen::build_move_menu_for_mon(&active);
    assert_eq!(
        fight
            .moves()
            .iter()
            .map(|m| (m.current_pp, m.max_pp, m.is_disabled))
            .collect::<Vec<_>>(),
        vec![(36, 36, false), (11, 42, true), (61, 61, false)]
    );
    assert_eq!(
        item.moves()
            .iter()
            .map(|m| (m.current_pp, m.max_pp, m.is_disabled))
            .collect::<Vec<_>>(),
        vec![(36, 36, false), (11, 42, false), (61, 61, false)]
    );
}

#[test]
fn max_pp_menu_uses_only_the_two_serialized_pp_up_bits() {
    // Save serialization keeps only two high bits of each packed PP byte.
    let mut pokemon = mon(Species::Pikachu, [MoveId::Thundershock; 4]);
    pokemon.pp_ups = [0xfc, 0xfd, 0xfe, 0xff];
    let screen = screen(vec![pokemon.clone()]);
    for menu in [
        BattleScreen::build_move_menu_from_state(screen.battle_state.as_ref().unwrap()),
        BattleScreen::build_move_menu_for_mon(&pokemon),
    ] {
        assert_eq!(
            menu.moves().iter().map(|m| m.max_pp).collect::<Vec<_>>(),
            vec![30, 36, 42, 48]
        );
    }
}

#[test]
fn max_pp_menu_transform_clears_working_ups_and_switch_restores_originals() {
    let mut ditto = mon(
        Species::Ditto,
        [MoveId::Transform, MoveId::None, MoveId::None, MoveId::None],
    );
    ditto.pp[0] = 12;
    ditto.pp_ups[0] = 1;
    let bench = mon(
        Species::Bulbasaur,
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    );
    let mut screen = screen(vec![ditto, bench]);
    screen.execute_turn_with_move(0);
    let bs = screen.battle_state.as_ref().unwrap();
    let working = bs.player.active_mon();
    assert_eq!(
        working.moves,
        [
            MoveId::PoisonSting,
            MoveId::StringShot,
            MoveId::None,
            MoveId::None
        ]
    );
    assert_eq!(working.pp, [5, 5, 0, 0]);
    assert_eq!(working.pp_ups, [0; 4]);
    assert_eq!(bs.player.party_mon(0).pp_ups, [1, 0, 0, 0]);
    assert_eq!(bs.player.party_mon(0).pp[0], 11);
    assert_eq!(
        BattleScreen::build_move_menu_from_state(bs).moves()[0].max_pp,
        35
    );
    screen.apply_player_switch(1);
    let restored = &screen.battle_state.as_ref().unwrap().player.party[0];
    assert_eq!(restored.species, Species::Ditto);
    assert_eq!(restored.moves[0], MoveId::Transform);
    assert_eq!(restored.pp_ups, [1, 0, 0, 0]);
    assert_eq!(restored.pp[0], 11);
}

#[test]
fn max_pp_menu_mimic_retains_slot_ups_and_switch_restores_original_move() {
    let mut pikachu = mon(
        Species::Pikachu,
        [
            MoveId::Mimic,
            MoveId::Thundershock,
            MoveId::Growl,
            MoveId::QuickAttack,
        ],
    );
    pikachu.pp[0] = 12;
    pikachu.pp_ups[0] = 1;
    let bench = mon(
        Species::Bulbasaur,
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    );
    let mut screen = screen(vec![pikachu, bench]);
    screen.execute_turn_with_move(0);
    assert_eq!(
        screen.phase,
        BattlePhase::MoveSelect,
        "real Mimic requests the foe move selector"
    );
    assert_eq!(
        screen.move_menu.as_ref().unwrap().moves()[0].move_id,
        MoveId::PoisonSting
    );
    screen.update_frame(BattleInput {
        a: true,
        ..BattleInput::none()
    });
    let bs = screen.battle_state.as_ref().unwrap();
    assert_eq!(bs.player.active_mon().moves[0], MoveId::PoisonSting);
    assert_eq!(bs.player.active_mon().pp[0], 11);
    assert_eq!(bs.player.active_mon().pp_ups, [1, 0, 0, 0]);
    assert_eq!(
        BattleScreen::build_move_menu_from_state(bs).moves()[0].max_pp,
        42
    );
    assert_eq!(bs.player.party_mon(0).moves[0], MoveId::Mimic);
    screen.apply_player_switch(1);
    let restored = &screen.battle_state.as_ref().unwrap().player.party[0];
    assert_eq!(restored.moves[0], MoveId::Mimic);
    assert_eq!(restored.pp[0], 11);
    assert_eq!(restored.pp_ups, [1, 0, 0, 0]);
}
