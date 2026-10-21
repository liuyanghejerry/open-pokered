//! Original CheckSpriteAvailability gates movement as well as drawing.
//! Constructed unit actors/maps here are not actual game capture evidence.
use super::*;
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};

fn route() -> OverworldScreen {
    let mut field = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
    field.state.player.x = 2;
    field.state.player.y = 2;
    field.publish_npc_frame(false);
    field
}
#[test]
fn route1_offscreen_walkers_keep_positions_facing_and_all_counters_until_player_approaches() {
    let mut field = route();
    let before = serde_json::to_value(&field.npc_states).unwrap();
    for _ in 0..512 {
        field.update_frame(OverworldInput::none());
    }
    assert_eq!((field.state.player.x, field.state.player.y), (2, 2));
    assert_eq!(serde_json::to_value(&field.npc_states).unwrap(), before);
    field.state.player.x = 4;
    field.state.player.y = 23;
    field.update_frame(OverworldInput::none());
    assert!(
        field.npc_states[0].walk_counter > 0 || field.npc_states[0].delay_counter > 0,
        "entering the original sprite rectangle resumes the walker"
    );
    assert!(field.npc_states.iter().all(|npc| npc.visible));
}
fn movement_pass(field: &mut OverworldScreen) {
    let map = field.map_data.as_ref().unwrap();
    npc_movement::update_npc_movement(
        &mut field.npc_states,
        field.state.player.x,
        field.state.player.y,
        None,
        map.width,
        map.height,
        0,
        &map.blocks,
        map.tileset,
        &collision::PokemonCollisionProvider::new(map.id, map.tileset),
    );
}
#[test]
fn active_stride_uses_source_destination_for_both_rectangle_edges() {
    let mut field = route();
    field.state.player.x = 10;
    field.state.player.y = 10;
    field.npc_states.truncate(1);
    let npc = &mut field.npc_states[0];
    npc.x = 15;
    npc.y = 10;
    npc.walk_counter = 8;
    npc.delay_counter = 19;
    npc.step_direction = Some(Direction::Right);
    let before = serde_json::to_value(&field.npc_states).unwrap();
    movement_pass(&mut field);
    assert_eq!(
        serde_json::to_value(&field.npc_states).unwrap(),
        before,
        "rightward destination x16 is outside, even though origin x15 is inside"
    );
    let npc = &mut field.npc_states[0];
    npc.x = 16;
    npc.step_direction = Some(Direction::Left);
    movement_pass(&mut field);
    assert_eq!(
        field.npc_states[0].walk_counter, 7,
        "leftward destination x15 is inside, even though origin x16 is outside"
    );
    assert_eq!(field.npc_states[0].delay_counter, 19);
    assert!(field.npc_states[0].visible);
}
#[test]
fn scripted_paths_cross_offscreen_rectangle_and_hidden_objects_stay_hidden() {
    let mut field = route();
    let npc = &mut field.npc_states[0];
    let target = (npc.x + 1, npc.y);
    npc.scripted_path.push_back(target);
    field.npc_states[1].visible = false;
    let hidden = serde_json::to_value(&field.npc_states[1]).unwrap();
    for _ in 0..=16 {
        movement_pass(&mut field);
    }
    assert_eq!((field.npc_states[0].x, field.npc_states[0].y), target);
    assert!(field.npc_states[0].scripted_path.is_empty());
    assert!(field.npc_states[0].visible);
    assert_eq!(serde_json::to_value(&field.npc_states[1]).unwrap(), hidden);
}
