use pokered_data::map_flags::*;
use pokered_data::map_names::*;
use pokered_data::maps::MapId;
use pokered_data::sprite_set_data::*;
use pokered_data::town_map_data::*;

// ── map_names ──────────────────────────────────────────────────────

#[test]
fn all_53_map_names_present() {
    assert_eq!(MAP_NAME_STRINGS.len(), NUM_MAP_NAMES);
    assert_eq!(NUM_MAP_NAMES, 53);
}

#[test]
fn pallet_town_name() {
    assert_eq!(map_name_for_map(MapId::PalletTown, false), "PALLET TOWN");
}

#[test]
fn indoor_map_inherits_parent_name() {
    assert_eq!(map_name_for_map(MapId::OaksLab, false), "PALLET TOWN");
    assert_eq!(map_name_for_map(MapId::ViridianGym, false), "VIRIDIAN CITY");
    assert_eq!(map_name_for_map(MapId::PokemonTower1F, false), "POKéMON TOWER");
}

#[test]
fn route_names() {
    assert_eq!(map_name_for_map(MapId::Route1, false), "ROUTE 1");
    assert_eq!(map_name_for_map(MapId::Route25, false), "ROUTE 25");
}

#[test]
fn name_id_round_trip() {
    let id = map_to_name_id(MapId::CeruleanCity);
    assert_eq!(id, MapNameId::CeruleanCity);
    assert_eq!(map_name_str(id), "CERULEAN CITY");
}

// ── town_map_data ──────────────────────────────────────────────────

#[test]
fn outdoor_entries_count() {
    assert_eq!(OUTDOOR_TOWN_MAP_ENTRIES.len(), NUM_OUTDOOR_ENTRIES);
    assert_eq!(NUM_OUTDOOR_ENTRIES, 37);
}

#[test]
fn indoor_entries_count() {
    assert_eq!(INDOOR_TOWN_MAP_ENTRIES.len(), NUM_INDOOR_ENTRIES);
    assert_eq!(NUM_INDOOR_ENTRIES, 60);
}

#[test]
fn town_map_order_count() {
    assert_eq!(TOWN_MAP_ORDER.len(), NUM_TOWN_MAP_ORDER);
    assert_eq!(NUM_TOWN_MAP_ORDER, 47);
}

#[test]
fn pallet_town_in_outdoor_entries() {
    let entry = OUTDOOR_TOWN_MAP_ENTRIES
        .iter()
        .find(|e| e.name == MapNameId::PalletTown);
    assert!(entry.is_some(), "Pallet Town must be in outdoor entries");
}

// ── sprite_set_data ────────────────────────────────────────────────

#[test]
fn sprite_sets_count() {
    assert_eq!(SPRITE_SETS.len(), NUM_SPRITE_SETS);
    assert_eq!(NUM_SPRITE_SETS, 10);
}

#[test]
fn each_sprite_set_has_11_entries() {
    for (i, set) in SPRITE_SETS.iter().enumerate() {
        assert_eq!(set.len(), SPRITE_SET_LENGTH, "set {i} wrong length");
    }
}

#[test]
fn map_sprite_sets_count() {
    assert_eq!(MAP_SPRITE_SETS.len(), NUM_OUTDOOR_MAPS);
    assert_eq!(NUM_OUTDOOR_MAPS, 37);
}

#[test]
fn split_map_sprite_sets_count() {
    assert_eq!(SPLIT_MAP_SPRITE_SETS.len(), NUM_SPLIT_SETS);
    assert_eq!(NUM_SPLIT_SETS, 12);
}

// ── map_flags ──────────────────────────────────────────────────────

#[test]
fn viridian_forest_is_dungeon() {
    assert!(is_dungeon_map(MapId::ViridianForest));
}

#[test]
fn pallet_town_not_dungeon() {
    assert!(!is_dungeon_map(MapId::PalletTown));
}

#[test]
fn rock_tunnel_is_dungeon() {
    assert!(is_dungeon_map(MapId::RockTunnel1F));
    assert!(is_dungeon_map(MapId::RockTunnelB1F));
}

#[test]
fn mt_moon_is_dungeon() {
    assert!(is_dungeon_map(MapId::MtMoon1F));
    assert!(is_dungeon_map(MapId::MtMoonB2F));
}

#[test]
fn pewter_gym_badge() {
    assert_eq!(badge_for_gym(MapId::PewterGym), Some(1 << BIT_BOULDERBADGE));
}

#[test]
fn non_gym_no_badge() {
    assert_eq!(badge_for_gym(MapId::PalletTown), None);
}

#[test]
fn all_8_badges_mapped() {
    assert_eq!(BADGE_MAP_FLAGS.len(), 8);
    let bits: Vec<u8> = BADGE_MAP_FLAGS.iter().map(|e| e.badge_bit).collect();
    for i in 0..8u8 {
        assert!(bits.contains(&i), "missing badge bit {i}");
    }
}

#[test]
fn safari_rest_houses() {
    assert!(is_safari_rest_house(MapId::SafariZoneWestRestHouse));
    assert!(is_safari_rest_house(MapId::SafariZoneEastRestHouse));
    assert!(is_safari_rest_house(MapId::SafariZoneNorthRestHouse));
    assert!(!is_safari_rest_house(MapId::PalletTown));
}
