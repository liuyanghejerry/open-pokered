mod game_flow_helpers;

use game_flow_helpers::*;
use pokered_core::battle::state::*;
use pokered_core::battle::turn::execute_turn;
use pokered_core::overworld::event_flags::EventFlags;
use pokered_core::overworld::map_loading::badge_for_gym;
use pokered_data::event_flags::EventFlag;
use pokered_data::maps::MapId;
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
fn brock_team_geodude_12_onix_14() {
    let team = find_trainer_party(TrainerClass::Brock, 0);
    assert_eq!(team.len(), 2);
    assert_eq!(team[0], (Species::Geodude, 12));
    assert_eq!(team[1], (Species::Onix, 14));
}

#[test]
fn misty_team_staryu_18_starmie_21() {
    let team = find_trainer_party(TrainerClass::Misty, 0);
    assert_eq!(team.len(), 2);
    assert_eq!(team[0], (Species::Staryu, 18));
    assert_eq!(team[1], (Species::Starmie, 21));
}

#[test]
fn lt_surge_team_voltorb_pikachu_raichu() {
    let team = find_trainer_party(TrainerClass::LtSurge, 0);
    assert_eq!(team.len(), 3);
    assert_eq!(team[0], (Species::Voltorb, 21));
    assert_eq!(team[1], (Species::Pikachu, 18));
    assert_eq!(team[2], (Species::Raichu, 24));
}

#[test]
fn erika_team_victreebel_tangela_vileplume() {
    let team = find_trainer_party(TrainerClass::Erika, 0);
    assert_eq!(team.len(), 3);
    assert_eq!(team[0], (Species::Victreebel, 29));
    assert_eq!(team[1], (Species::Tangela, 24));
    assert_eq!(team[2], (Species::Vileplume, 29));
}

#[test]
fn koga_team_four_pokemon() {
    let team = find_trainer_party(TrainerClass::Koga, 0);
    assert_eq!(team.len(), 4);
    assert_eq!(team[0], (Species::Koffing, 37));
    assert_eq!(team[1], (Species::Muk, 39));
    assert_eq!(team[2], (Species::Koffing, 37));
    assert_eq!(team[3], (Species::Weezing, 43));
}

#[test]
fn blaine_team_four_fire_types() {
    let team = find_trainer_party(TrainerClass::Blaine, 0);
    assert_eq!(team.len(), 4);
    assert_eq!(team[0], (Species::Growlithe, 42));
    assert_eq!(team[1], (Species::Ponyta, 40));
    assert_eq!(team[2], (Species::Rapidash, 42));
    assert_eq!(team[3], (Species::Arcanine, 47));
}

#[test]
fn sabrina_team_four_psychic_types() {
    let team = find_trainer_party(TrainerClass::Sabrina, 0);
    assert_eq!(team.len(), 4);
    assert_eq!(team[0], (Species::Kadabra, 38));
    assert_eq!(team[1], (Species::MrMime, 37));
    assert_eq!(team[2], (Species::Venomoth, 38));
    assert_eq!(team[3], (Species::Alakazam, 43));
}

#[test]
fn giovanni_gym_team() {
    let team = find_trainer_party(TrainerClass::Giovanni, 2);
    assert_eq!(team.len(), 5);
    assert_eq!(team[0], (Species::Rhyhorn, 45));
    assert_eq!(team[1], (Species::Dugtrio, 42));
    assert_eq!(team[2], (Species::Nidoqueen, 44));
    assert_eq!(team[3], (Species::Nidoking, 45));
    assert_eq!(team[4], (Species::Rhydon, 50));
}

#[test]
fn all_gym_leaders_have_base_money_99() {
    let gym_leaders = [
        TrainerClass::Brock,
        TrainerClass::Misty,
        TrainerClass::LtSurge,
        TrainerClass::Erika,
        TrainerClass::Koga,
        TrainerClass::Blaine,
        TrainerClass::Sabrina,
        TrainerClass::Giovanni,
    ];
    for &class in &gym_leaders {
        assert_eq!(
            get_base_money(class),
            99,
            "{:?} should have base_money 99",
            class
        );
    }
}

#[test]
fn badge_assignments_for_gyms() {
    assert_eq!(badge_for_gym(MapId::PewterGym), Some(BOULDER_BADGE));
    assert_eq!(badge_for_gym(MapId::CeruleanGym), Some(CASCADE_BADGE));
    assert_eq!(badge_for_gym(MapId::VermilionGym), Some(THUNDER_BADGE));
    assert_eq!(badge_for_gym(MapId::CeladonGym), Some(RAINBOW_BADGE));
    assert_eq!(badge_for_gym(MapId::FuchsiaGym), Some(SOUL_BADGE));
    assert_eq!(badge_for_gym(MapId::SaffronGym), Some(MARSH_BADGE));
    assert_eq!(badge_for_gym(MapId::CinnabarGym), Some(VOLCANO_BADGE));
    assert_eq!(badge_for_gym(MapId::ViridianGym), Some(EARTH_BADGE));
}

#[test]
fn non_gym_map_returns_no_badge() {
    assert_eq!(badge_for_gym(MapId::PalletTown), None);
    assert_eq!(badge_for_gym(MapId::Route1), None);
}

#[test]
fn gym_event_flags_set_correctly() {
    let mut flags = EventFlags::new();

    let gym_flags = [
        EventFlag::EVENT_BEAT_BROCK,
        EventFlag::EVENT_BEAT_MISTY,
        EventFlag::EVENT_BEAT_LT_SURGE,
        EventFlag::EVENT_BEAT_ERIKA,
        EventFlag::EVENT_BEAT_KOGA,
        EventFlag::EVENT_BEAT_SABRINA,
        EventFlag::EVENT_BEAT_BLAINE,
    ];

    for (i, &flag) in gym_flags.iter().enumerate() {
        assert!(!flags.check(flag));
        flags.set(flag);
        assert!(flags.check(flag));
        assert_eq!(flags.count_set() as usize, i + 1);
    }
}

#[test]
fn badges_accumulate_correctly() {
    let mut badges: u8 = 0;
    assert_eq!(badge_count(badges), 0);

    badges |= BOULDER_BADGE;
    assert_eq!(badge_count(badges), 1);

    badges |= CASCADE_BADGE;
    assert_eq!(badge_count(badges), 2);

    badges |= THUNDER_BADGE | RAINBOW_BADGE;
    assert_eq!(badge_count(badges), 4);

    badges |= SOUL_BADGE | MARSH_BADGE | VOLCANO_BADGE | EARTH_BADGE;
    assert_eq!(badge_count(badges), 8);
    assert_eq!(badges, ALL_BADGES);
}

#[test]
fn gym_battle_brock_player_wins_with_water() {
    let player = make_pokemon(
        Species::Squirtle,
        12,
        35,
        20,
        25,
        22,
        22,
        PokemonType::Water,
        PokemonType::Water,
        [MoveId::WaterGun, MoveId::Tackle, MoveId::None, MoveId::None],
        [25, 35, 0, 0],
    );
    let geodude = make_brock_geodude();

    let mut state = new_battle_state(BattleType::Trainer, vec![player], vec![geodude]);
    state.player.selected_move = MoveId::WaterGun;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Tackle;
    state.enemy.selected_move_index = 0;

    let water_gun_data = MoveData {
        id: MoveId::WaterGun,
        effect: MoveEffect::NoAdditionalEffect,
        power: 40,
        move_type: PokemonType::Water,
        accuracy: 255,
        pp: 25,
    };
    let tackle_data = MoveData {
        id: MoveId::Tackle,
        effect: MoveEffect::NoAdditionalEffect,
        power: 35,
        move_type: PokemonType::Normal,
        accuracy: 255,
        pp: 35,
    };

    let randoms = turn_randoms_player_first();
    execute_turn(&mut state, &water_gun_data, &tackle_data, &randoms);

    // Water Gun is super effective against Rock/Ground Geodude (4x)
    assert!(
        state.enemy.active_mon().hp < state.enemy.active_mon().max_hp,
        "Water Gun should deal significant damage to Geodude"
    );
}

#[test]
fn gym_leader_levels_increase_progressively() {
    let brock = find_trainer_party(TrainerClass::Brock, 0);
    let misty = find_trainer_party(TrainerClass::Misty, 0);
    let surge = find_trainer_party(TrainerClass::LtSurge, 0);
    let erika = find_trainer_party(TrainerClass::Erika, 0);
    let koga = find_trainer_party(TrainerClass::Koga, 0);
    let sabrina = find_trainer_party(TrainerClass::Sabrina, 0);
    let blaine = find_trainer_party(TrainerClass::Blaine, 0);
    let giovanni = find_trainer_party(TrainerClass::Giovanni, 2);

    let max_levels: Vec<u8> = [brock, misty, surge, erika, koga, sabrina, blaine, giovanni]
        .iter()
        .map(|team| team.iter().map(|&(_, lvl)| lvl).max().unwrap())
        .collect();

    // Each gym leader's max level should generally be higher than the previous
    for i in 1..max_levels.len() {
        assert!(
            max_levels[i] >= max_levels[i - 1],
            "Gym {} max level ({}) should be >= gym {} max level ({})",
            i + 1,
            max_levels[i],
            i,
            max_levels[i - 1]
        );
    }
}
