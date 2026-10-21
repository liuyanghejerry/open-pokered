//! Constructed component inputs to the real live-session SRAM writer.
use super::*;
use pokered_core::overworld::{npc_saved_state::NpcSavedState, Direction};
use pokered_core::save::{sram_export::export_sram, sram_import::import_sram};

#[test]
fn production_save_writer_preserves_actor_stride_and_displacements_in_original_tables() {
    let mut field = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
    field.state.player.x = 5;
    field.state.player.y = 21;
    field
        .npc_saved_state
        .prepare(MapId::Route1, &field.npc_states);
    let npc = &mut field.npc_states[0];
    npc.y = 25;
    npc.facing = Direction::Left;
    npc.walk_counter = 8;
    npc.step_direction = Some(Direction::Down);
    npc.delay_counter = 73;
    field.npc_saved_state.data.second[1][2] = 4;
    field.npc_saved_state.data.second[1][3] = 1;
    let mut save = SaveData::new();
    PokemonGame::apply_live_state_to_save(&mut save, &field, "RED", "BLUE");
    let imported = import_sram(&export_sram(&save)).unwrap();
    assert_eq!(imported.game_data.num_sprites, 2);
    assert_eq!(imported.game_data.map_sprite_data[0], (1, 1));
    assert_eq!(imported.sprite_data.first[1][1], 3);
    assert_eq!(&imported.sprite_data.second[1][4..6], &[30, 9]);
    assert_eq!(&imported.sprite_data.second[1][2..4], &[4, 1]);
    let mut restored = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
    NpcSavedState::default().restore(
        MapId::Route1,
        &imported.sprite_data,
        &mut restored.npc_states,
    );
    let npc = &restored.npc_states[0];
    assert_eq!(
        (npc.x, npc.y, npc.walk_counter, npc.delay_counter),
        (5, 25, 8, 73)
    );
    assert_eq!(npc.facing, Direction::Left);
    assert_eq!(npc.step_direction, Some(Direction::Down));
}

#[test]
fn production_save_writer_exports_a_waiting_actor_at_its_live_position() {
    let mut field = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
    field.npc_states[0].y = 25;
    field.npc_states[0].delay_counter = 256;
    let mut save = SaveData::new();
    PokemonGame::apply_live_state_to_save(&mut save, &field, "RED", "BLUE");
    let imported = import_sram(&export_sram(&save)).unwrap();
    assert_eq!(imported.sprite_data.first[1][1], 2);
    assert_eq!(imported.sprite_data.second[1][8], 0);
    assert_eq!(imported.sprite_data.second[1][4], 29);
    assert_eq!(&imported.sprite_data.second[1][2..4], &[8, 8]);
}
