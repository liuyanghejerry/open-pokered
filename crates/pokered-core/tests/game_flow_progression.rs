#[path = "game_flow_helpers.rs"]
mod game_flow_helpers;

use game_flow_helpers::*;

use pokered_core::battle::state::Pokemon;
use pokered_core::overworld::event_flags::EventFlags;
use pokered_core::pokemon::party::Party;
use pokered_core::pokemon::pokedex::Pokedex;
use pokered_data::event_flags::EventFlag;
use pokered_data::species::Species;
use pokered_data::wild_data::GameVersion;

#[test]
fn badge_count_starts_at_zero() {
    assert_eq!(badge_count(0), 0);
}

#[test]
fn one_badge_after_brock() {
    let badges = BOULDER_BADGE;
    assert_eq!(badge_count(badges), 1);
    assert!(badges & BOULDER_BADGE != 0);
    assert!(badges & CASCADE_BADGE == 0);
}

#[test]
fn two_badges_after_misty() {
    let badges = BOULDER_BADGE | CASCADE_BADGE;
    assert_eq!(badge_count(badges), 2);
}

#[test]
fn four_badges_mid_game() {
    let badges = BOULDER_BADGE | CASCADE_BADGE | THUNDER_BADGE | RAINBOW_BADGE;
    assert_eq!(badge_count(badges), 4);
}

#[test]
fn all_eight_badges() {
    assert_eq!(badge_count(ALL_BADGES), 8);
    assert_eq!(ALL_BADGES, 0xFF);
}

#[test]
fn no_event_flags_at_start() {
    let flags = EventFlags::new();
    assert!(!flags.check(EventFlag::EVENT_GOT_STARTER));
    assert!(!flags.check(EventFlag::EVENT_GOT_POKEDEX));
    assert!(!flags.check(EventFlag::EVENT_BEAT_BROCK));
}

#[test]
fn early_game_event_flags() {
    let mut flags = EventFlags::new();
    flags.set(EventFlag::EVENT_GOT_STARTER);
    flags.set(EventFlag::EVENT_BATTLED_RIVAL_IN_OAKS_LAB);
    flags.set(EventFlag::EVENT_GOT_POKEDEX);

    assert!(flags.check(EventFlag::EVENT_GOT_STARTER));
    assert!(flags.check(EventFlag::EVENT_BATTLED_RIVAL_IN_OAKS_LAB));
    assert!(flags.check(EventFlag::EVENT_GOT_POKEDEX));
    assert!(!flags.check(EventFlag::EVENT_BEAT_BROCK));
}

#[test]
fn mid_game_event_flags_accumulate() {
    let mut flags = EventFlags::new();
    flags.set(EventFlag::EVENT_GOT_STARTER);
    flags.set(EventFlag::EVENT_GOT_POKEDEX);
    flags.set(EventFlag::EVENT_BEAT_BROCK);
    flags.set(EventFlag::EVENT_BEAT_MISTY);
    flags.set(EventFlag::EVENT_BEAT_LT_SURGE);
    flags.set(EventFlag::EVENT_BEAT_ERIKA);

    assert!(flags.check(EventFlag::EVENT_BEAT_BROCK));
    assert!(flags.check(EventFlag::EVENT_BEAT_MISTY));
    assert!(flags.check(EventFlag::EVENT_BEAT_LT_SURGE));
    assert!(flags.check(EventFlag::EVENT_BEAT_ERIKA));
    assert!(!flags.check(EventFlag::EVENT_BEAT_KOGA));
}

#[test]
fn end_game_all_gym_flags_set() {
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
    for &flag in &gym_flags {
        flags.set(flag);
    }
    for &flag in &gym_flags {
        assert!(flags.check(flag));
    }
    assert!(!flags.check(EventFlag::EVENT_BEAT_LANCE));
}

#[test]
fn event_flags_can_be_reset() {
    let mut flags = EventFlags::new();
    flags.set(EventFlag::EVENT_GOT_STARTER);
    assert!(flags.check(EventFlag::EVENT_GOT_STARTER));
    flags.reset(EventFlag::EVENT_GOT_STARTER);
    assert!(!flags.check(EventFlag::EVENT_GOT_STARTER));
}

#[test]
fn party_starts_with_one_pokemon() {
    let party = party_at_start(make_starter_charmander());
    assert_eq!(party.count(), 1);
    assert_eq!(party.get(0).unwrap().species, Species::Charmander);
    assert_eq!(party.get(0).unwrap().level, 5);
}

#[test]
fn party_grows_during_game() {
    let party = party_mid_game();
    assert_eq!(party.count(), 3);
    assert_eq!(party.get(0).unwrap().species, Species::Charmeleon);
    assert!(party.get(0).unwrap().level > 5);
}

#[test]
fn party_full_for_elite_four() {
    let party = party_elite_four();
    assert_eq!(party.count(), 6);
    for i in 0..6 {
        assert!(party.get(i).unwrap().level >= 57);
    }
}

#[test]
fn party_max_size_is_six() {
    let party = party_elite_four();
    assert_eq!(party.count(), 6);
    let extra = make_starter_bulbasaur();
    let mut party_vec: Vec<Pokemon> = (0..6).map(|i| party.get(i).unwrap().clone()).collect();
    party_vec.push(extra);
    let result = Party::from_pokemon(party_vec);
    assert!(result.is_err());
}

#[test]
fn empty_pokedex_at_start() {
    let dex = Pokedex::new();
    assert_eq!(dex.owned_count(), 0);
    assert_eq!(dex.seen_count(), 0);
}

#[test]
fn pokedex_early_game_counts() {
    let dex = pokedex_early_game();
    assert!(dex.owned_count() >= 2);
    assert!(dex.seen_count() >= 3);
    assert!(dex.is_owned(Species::Charmander));
    assert!(dex.is_seen(Species::Pidgey));
}

#[test]
fn pokedex_mid_game_has_many_entries() {
    let dex = pokedex_mid_game();
    assert!(dex.owned_count() >= 15);
    assert!(dex.seen_count() >= 30);
    assert!(dex.is_owned(Species::Charmeleon));
    assert!(dex.is_owned(Species::Kadabra));
}

#[test]
fn owned_implies_seen() {
    let mut dex = Pokedex::new();
    dex.set_owned(Species::Pikachu);
    assert!(dex.is_owned(Species::Pikachu));
    assert!(dex.is_seen(Species::Pikachu));
}

#[test]
fn red_and_blue_are_different_versions() {
    let red = new_game_state_red();
    let blue = new_game_state_blue();
    assert_eq!(red.config.version, GameVersion::Red);
    assert_eq!(blue.config.version, GameVersion::Blue);
}

#[test]
fn game_config_stores_version() {
    let config = new_game_config_red();
    assert_eq!(config.version, GameVersion::Red);
}
