//! Constructed component fixtures; normal-boot recordings are kept separately.
use super::*;
use crate::overworld::{npc_movement::load_map_npcs, OverworldInput, OverworldScreen};
use pokered_data::{impl_traits::PokemonRedData, npc_data::get_map_npcs};

#[test]
fn in_flight_stride_roundtrip_preserves_origin_destination_and_independent_facing() {
    for direction in [
        Direction::Down,
        Direction::Up,
        Direction::Left,
        Direction::Right,
    ] {
        for counter in [1, 8, 16] {
            let mut npcs = load_map_npcs(get_map_npcs(MapId::Route1));
            npcs[0].x = 14;
            npcs[0].y = 20;
            npcs[0].facing = Direction::Right;
            npcs[0].step_direction = Some(direction);
            npcs[0].walk_counter = counter;
            npcs[0].delay_counter = 63;
            let mut source = NpcSavedState::default();
            source.prepare(MapId::Route1, &npcs);
            source.data.second[1][2] = 3;
            source.data.second[1][3] = 0;
            source.data.first[15][15] = 99;
            let saved = source.capture(MapId::Route1, &npcs, (14, 20));
            let mut restored = load_map_npcs(get_map_npcs(MapId::Route1));
            let mut owner = NpcSavedState::default();
            owner.restore(MapId::Route1, &saved, &mut restored);
            let n = &restored[0];
            assert_eq!((n.x, n.y, n.walk_counter), (14, 20, counter));
            assert_eq!(n.step_direction, Some(direction));
            assert_eq!(n.facing, Direction::Right);
            assert_eq!(n.delay_counter, 63);
            assert_eq!(&owner.data.second[1][2..4], &[3, 0]);
            assert_eq!(owner.data.first[15][15], 99);
        }
    }
}

#[test]
fn delayed_zero_byte_restores_a_full_wrapping_wait_and_offscreen_is_not_hidden() {
    let mut npcs = load_map_npcs(get_map_npcs(MapId::Route1));
    npcs[0].x = 5;
    npcs[0].y = 25;
    npcs[0].delay_counter = 256;
    let mut saved = NpcSavedState::default().capture(MapId::Route1, &npcs, (2, 2));
    saved.first[1][2] = 255;
    let mut restored = load_map_npcs(get_map_npcs(MapId::Route1));
    NpcSavedState::default().restore(MapId::Route1, &saved, &mut restored);
    assert_eq!((restored[0].x, restored[0].y), (5, 25));
    assert_eq!(restored[0].delay_counter, 256);
    assert!(restored[0].visible);
}

#[test]
fn old_zero_sprite_saves_leave_runtime_map_defaults_intact() {
    let mut npcs = load_map_npcs(get_map_npcs(MapId::Route1));
    let before = serde_json::to_value(&npcs).unwrap();
    let mut owner = NpcSavedState::default();
    owner.restore(MapId::Route1, &SpriteData::default(), &mut npcs);
    assert_eq!(serde_json::to_value(&npcs).unwrap(), before);
    assert_eq!(&owner.data.second[1][2..4], &[8, 8]);
}

#[test]
fn legacy_zero_sprite_table_does_not_restore_stale_object_metadata_from_another_map() {
    let mut npcs = load_map_npcs(get_map_npcs(MapId::Route1));
    let before = serde_json::to_value(&npcs).unwrap();
    let mut save = crate::save::SaveData::new();
    save.game_data.position.map_id = MapId::Route1 as u8;
    save.game_data.num_sprites = 7;
    save.game_data.map_sprite_data[0] = (0, 17);
    let mut owner = NpcSavedState::default();
    owner.restore_from_save(MapId::Route1, &save, &mut npcs);
    assert_eq!(serde_json::to_value(&npcs).unwrap(), before);
    save.sprite_data = owner.capture(MapId::Route1, &npcs, (5, 21));
    owner.restore_from_save(MapId::Route1, &save, &mut npcs);
    assert_eq!(npcs[0].range, 0);
    assert_eq!(npcs[0].text_id, 17);
    let mut other_map_npcs = load_map_npcs(get_map_npcs(MapId::PalletTown));
    let defaults = serde_json::to_value(&other_map_npcs).unwrap();
    owner.restore_from_save(MapId::PalletTown, &save, &mut other_map_npcs);
    assert_eq!(serde_json::to_value(&other_map_npcs).unwrap(), defaults);
}

#[test]
fn uninitialized_source_slot_initializes_counters_but_ready_zero_is_preserved() {
    let mut npcs = load_map_npcs(get_map_npcs(MapId::Route1));
    let mut raw = SpriteData::default();
    raw.first[1][0] = 4;
    raw.second[1][4] = 28;
    raw.second[1][5] = 9;
    raw.second[1][6] = 0xfe;
    let mut owner = NpcSavedState::default();
    owner.restore(MapId::Route1, &raw, &mut npcs);
    assert_eq!(&owner.data.second[1][2..4], &[8, 8]);
    raw.first[1][1] = 1;
    owner.restore(MapId::Route1, &raw, &mut npcs);
    assert_eq!(&owner.data.second[1][2..4], &[0, 0]);
}

fn field() -> OverworldScreen {
    let mut field = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
    field.state.player.x = 6;
    field.state.player.y = 24;
    field.npc_states[0].range = 0xD0;
    field
        .npc_saved_state
        .prepare(MapId::Route1, &field.npc_states);
    field.publish_npc_frame(false);
    field
}

#[test]
fn exhausted_displacement_rejects_live_ready_walker_without_replacing_facing_or_delay() {
    let mut field = field();
    // The source movement selector forces Down; (5,25) is known walkable and within the screen.
    field.npc_saved_state.data.second[1][2] = 0;
    field.update_frame(OverworldInput::none());
    assert_eq!(field.npc_states[0].walk_counter, 0);
    assert!(field.npc_states[0].delay_counter > 0);
    assert_eq!(field.npc_states[0].facing, Direction::Down);
    assert_eq!(field.npc_saved_state.data.second[1][2], 0);
}

#[test]
fn successful_live_stride_updates_displacement_once_not_each_pixel() {
    let mut field = field();
    field.update_frame(OverworldInput::none());
    assert_eq!(field.npc_states[0].walk_counter, 16);
    assert_eq!(field.npc_saved_state.data.second[1][2], 9);
    for _ in 0..8 {
        field.update_frame(OverworldInput::none());
    }
    assert!(field.npc_states[0].walk_counter < 16);
    assert_eq!(field.npc_saved_state.data.second[1][2], 9);
}

#[test]
fn scripted_live_stride_bypasses_displacements_and_does_not_store_them() {
    let mut field = field();
    field.npc_saved_state.data.second[1][2] = 0;
    field.npc_saved_state.data.second[1][3] = 0;
    field.npc_states[0].scripted_path.push_back((5, 25));
    field.update_frame(OverworldInput::none());
    assert_eq!(field.npc_states[0].walk_counter, 16);
    assert_eq!(&field.npc_saved_state.data.second[1][2..4], &[0, 0]);
}

#[test]
fn snapshot_preserves_owner_and_map_replacement_resets_it() {
    let mut field = field();
    field.npc_saved_state.data.second[1][2] = 3;
    let value = serde_json::to_value(crate::snapshot::OverworldSnapshot::capture(&field)).unwrap();
    let snapshot: crate::snapshot::OverworldSnapshot = serde_json::from_value(value).unwrap();
    let mut restored = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
    snapshot.restore_into(&mut restored);
    assert_eq!(restored.npc_saved_state.data.second[1][2], 3);
    // Constructed unit warp exercises a real map replacement, not gameplay proof.
    restored.warp_to_map(MapId::Route1, 5, 23);
    restored.commit_pending_warp();
    restored
        .npc_saved_state
        .prepare(restored.state.current_map, &restored.npc_states);
    assert_eq!(restored.npc_saved_state.data.second[1][2], 8);
    restored.npc_saved_state.data.second[1][2] = 1;
    restored.warp_to_map(MapId::PalletTown, 5, 6);
    restored.commit_pending_warp();
    restored
        .npc_saved_state
        .prepare(restored.state.current_map, &restored.npc_states);
    assert_eq!(restored.state.current_map, MapId::PalletTown);
    assert_eq!(restored.npc_saved_state.data.second[1][2], 8);
}
