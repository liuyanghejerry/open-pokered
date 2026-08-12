mod game_flow_helpers;

use game_flow_helpers::*;
use pokered_core::battle::state::*;
use pokered_core::battle::turn::execute_turn;
use pokered_core::overworld::event_flags::EventFlags;
use pokered_core::save::hall_of_fame::{HallOfFame, HofMon, HofTeam};
use pokered_data::event_flags::EventFlag;
use pokered_data::move_data::MoveData;
use pokered_data::moves::{MoveEffect, MoveId};
use pokered_data::species::Species;
use pokered_data::trainer_data::{get_base_money, trainer_data, TrainerClass};
use pokered_data::types::PokemonType;

fn find_trainer_party(class: TrainerClass, party_index: usize) -> Vec<(Species, u8)> {
    let data = trainer_data();
    let class_data = data.iter().find(|d| d.class == class).unwrap();
    class_data.parties[party_index]
        .pokemon
        .iter()
        .map(|m| (m.species, m.level))
        .collect()
}

#[test]
fn lorelei_team_five_pokemon() {
    let team = find_trainer_party(TrainerClass::Lorelei, 0);
    assert_eq!(team.len(), 5);
    assert_eq!(team[0], (Species::Dewgong, 54));
    assert_eq!(team[1], (Species::Cloyster, 53));
    assert_eq!(team[2], (Species::Slowbro, 54));
    assert_eq!(team[3], (Species::Jynx, 56));
    assert_eq!(team[4], (Species::Lapras, 56));
}

#[test]
fn bruno_team_five_pokemon() {
    let team = find_trainer_party(TrainerClass::Bruno, 0);
    assert_eq!(team.len(), 5);
    assert_eq!(team[0], (Species::Onix, 53));
    assert_eq!(team[1], (Species::Hitmonchan, 55));
    assert_eq!(team[2], (Species::Hitmonlee, 55));
    assert_eq!(team[3], (Species::Onix, 56));
    assert_eq!(team[4], (Species::Machamp, 58));
}

#[test]
fn agatha_team_five_pokemon() {
    let team = find_trainer_party(TrainerClass::Agatha, 0);
    assert_eq!(team.len(), 5);
    assert_eq!(team[0], (Species::Gengar, 56));
    assert_eq!(team[1], (Species::Golbat, 56));
    assert_eq!(team[2], (Species::Haunter, 55));
    assert_eq!(team[3], (Species::Arbok, 58));
    assert_eq!(team[4], (Species::Gengar, 60));
}

#[test]
fn lance_team_five_pokemon() {
    let team = find_trainer_party(TrainerClass::Lance, 0);
    assert_eq!(team.len(), 5);
    assert_eq!(team[0], (Species::Gyarados, 58));
    assert_eq!(team[1], (Species::Dragonair, 56));
    assert_eq!(team[2], (Species::Dragonair, 56));
    assert_eq!(team[3], (Species::Aerodactyl, 60));
    assert_eq!(team[4], (Species::Dragonite, 62));
}

#[test]
fn champion_variant0_player_chose_bulbasaur() {
    let team = find_trainer_party(TrainerClass::Rival3, 0);
    assert_eq!(team.len(), 6);
    assert_eq!(team[0], (Species::Pidgeot, 61));
    assert_eq!(team[1], (Species::Alakazam, 59));
    assert_eq!(team[2], (Species::Rhydon, 61));
    assert_eq!(team[3], (Species::Arcanine, 61));
    assert_eq!(team[4], (Species::Exeggutor, 63));
    assert_eq!(team[5], (Species::Blastoise, 65));
}

#[test]
fn champion_variant1_player_chose_charmander() {
    let team = find_trainer_party(TrainerClass::Rival3, 1);
    assert_eq!(team.len(), 6);
    assert_eq!(team[0], (Species::Pidgeot, 61));
    assert_eq!(team[5], (Species::Venusaur, 65));
}

#[test]
fn champion_variant2_player_chose_squirtle() {
    let team = find_trainer_party(TrainerClass::Rival3, 2);
    assert_eq!(team.len(), 6);
    assert_eq!(team[0], (Species::Pidgeot, 61));
    assert_eq!(team[5], (Species::Charizard, 65));
}

#[test]
fn all_elite_four_have_base_money_99() {
    let e4 = [
        TrainerClass::Lorelei,
        TrainerClass::Bruno,
        TrainerClass::Agatha,
        TrainerClass::Lance,
        TrainerClass::Rival3,
    ];
    for &class in &e4 {
        assert_eq!(
            get_base_money(class),
            99,
            "{:?} should have base_money 99",
            class
        );
    }
}

#[test]
fn elite_four_levels_increase_across_members() {
    let lorelei_max = find_trainer_party(TrainerClass::Lorelei, 0)
        .iter()
        .map(|&(_, lvl)| lvl)
        .max()
        .unwrap();
    let bruno_max = find_trainer_party(TrainerClass::Bruno, 0)
        .iter()
        .map(|&(_, lvl)| lvl)
        .max()
        .unwrap();
    let agatha_max = find_trainer_party(TrainerClass::Agatha, 0)
        .iter()
        .map(|&(_, lvl)| lvl)
        .max()
        .unwrap();
    let lance_max = find_trainer_party(TrainerClass::Lance, 0)
        .iter()
        .map(|&(_, lvl)| lvl)
        .max()
        .unwrap();
    let champion_max = find_trainer_party(TrainerClass::Rival3, 0)
        .iter()
        .map(|&(_, lvl)| lvl)
        .max()
        .unwrap();

    assert!(lorelei_max <= bruno_max);
    assert!(bruno_max <= agatha_max);
    assert!(agatha_max <= lance_max);
    assert!(lance_max <= champion_max);
}

#[test]
fn event_flag_beat_lance() {
    let mut flags = EventFlags::new();
    flags.set(EventFlag::EVENT_BEAT_LANCE);
    assert!(flags.check(EventFlag::EVENT_BEAT_LANCE));
}

#[test]
fn event_flag_beat_champion() {
    let mut flags = EventFlags::new();
    flags.set(EventFlag::EVENT_BEAT_CHAMPION_RIVAL);
    assert!(flags.check(EventFlag::EVENT_BEAT_CHAMPION_RIVAL));
}

#[test]
fn hall_of_fame_records_winning_team() {
    let mut hof = HallOfFame::new();
    assert_eq!(hof.team_count(), 0);

    let mut team = HofTeam::new();
    team.add_mon(HofMon::new(
        Species::Charizard as u8,
        63,
        &[0x82, 0x87, 0x80, 0x91, 0x88, 0x99, 0x80, 0x91, 0x83, 0x50],
    ));
    team.add_mon(HofMon::new(
        Species::Alakazam as u8,
        60,
        &[0x80, 0x8B, 0x80, 0x8A, 0x80, 0x99, 0x80, 0x8C, 0x50],
    ));
    team.add_mon(HofMon::new(
        Species::Lapras as u8,
        58,
        &[0x8B, 0x80, 0x8F, 0x91, 0x80, 0x92, 0x50],
    ));

    hof.push_team(team);
    assert_eq!(hof.team_count(), 1);

    let recorded = hof.get_team(0).unwrap();
    assert_eq!(recorded.mons().len(), 3);
    assert_eq!(recorded.mons()[0].species, Species::Charizard as u8);
    assert_eq!(recorded.mons()[0].level, 63);
}

#[test]
fn hall_of_fame_max_50_entries() {
    let mut hof = HallOfFame::new();

    for i in 0..50 {
        let mut team = HofTeam::new();
        team.add_mon(HofMon::new(i as u8, 50, &[0x50]));
        hof.push_team(team);
    }
    assert_eq!(hof.team_count(), 50);

    let mut overflow_team = HofTeam::new();
    overflow_team.add_mon(HofMon::new(200, 99, &[0x50]));
    hof.push_team(overflow_team);

    assert_eq!(hof.team_count(), 50, "Should not exceed 50 entries");
    let last = hof.get_team(49).unwrap();
    assert_eq!(
        last.mons()[0].species, 200,
        "Most recent entry should be at end"
    );
}

#[test]
fn champion_battle_with_strong_party() {
    let player = make_pokemon(
        Species::Charizard,
        65,
        200,
        120,
        95,
        120,
        105,
        PokemonType::Fire,
        PokemonType::Flying,
        [
            MoveId::Slash,
            MoveId::FireBlast,
            MoveId::Earthquake,
            MoveId::Strength,
        ],
        [20, 5, 10, 15],
    );
    let rival_pidgeot = make_pokemon(
        Species::Pidgeot,
        61,
        170,
        90,
        80,
        101,
        75,
        PokemonType::Normal,
        PokemonType::Flying,
        [
            MoveId::WingAttack,
            MoveId::MirrorMove,
            MoveId::None,
            MoveId::None,
        ],
        [35, 20, 0, 0],
    );

    let mut state = new_battle_state(BattleType::Trainer, vec![player], vec![rival_pidgeot]);
    state.player.selected_move = MoveId::Slash;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::WingAttack;
    state.enemy.selected_move_index = 0;

    let slash_data = MoveData {
        id: MoveId::Slash,
        effect: MoveEffect::NoAdditionalEffect,
        power: 70,
        move_type: PokemonType::Normal,
        accuracy: 255,
        pp: 20,
    };
    let wing_data = MoveData {
        id: MoveId::WingAttack,
        effect: MoveEffect::NoAdditionalEffect,
        power: 35,
        move_type: PokemonType::Flying,
        accuracy: 255,
        pp: 35,
    };

    let randoms = turn_randoms_player_first();
    execute_turn(&mut state, &slash_data, &wing_data, &randoms);

    assert!(
        state.enemy.active_mon().hp < state.enemy.active_mon().max_hp,
        "Slash should deal damage to Pidgeot"
    );
}

#[test]
fn champion_has_three_team_variants() {
    let data = trainer_data();
    let rival3 = data
        .iter()
        .find(|d| d.class == TrainerClass::Rival3)
        .unwrap();
    assert_eq!(
        rival3.parties.len(),
        3,
        "Champion should have 3 party variants"
    );

    for party in &rival3.parties {
        assert_eq!(
            party.pokemon.len(),
            6,
            "Each champion party should have 6 pokemon"
        );
    }
}

#[test]
fn champion_ace_is_always_level_65() {
    for variant in 0..3 {
        let team = find_trainer_party(TrainerClass::Rival3, variant);
        let ace = team.last().unwrap();
        assert_eq!(
            ace.1, 65,
            "Champion's ace (variant {}) should be level 65",
            variant
        );
    }
}

#[test]
fn champion_ace_matches_starter_triangle() {
    let team0 = find_trainer_party(TrainerClass::Rival3, 0);
    let team1 = find_trainer_party(TrainerClass::Rival3, 1);
    let team2 = find_trainer_party(TrainerClass::Rival3, 2);

    assert_eq!(team0.last().unwrap().0, Species::Blastoise);
    assert_eq!(team1.last().unwrap().0, Species::Venusaur);
    assert_eq!(team2.last().unwrap().0, Species::Charizard);
}
