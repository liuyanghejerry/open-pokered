//! Counter-boundary fixtures for UpdateNPCSprite's ready/wWalkCounter gate.
//! These constructed core states are separate from normal-boot evidence.
use super::*;
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};

fn field(moving: bool) -> OverworldScreen {
    let mut field = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
    // Both (6,24) and the rightward destination (7,24) are walkable. The
    // ordinary walker at (5,24) is in view and does not occupy that stride.
    field.state.player.x = 6;
    field.state.player.y = 24;
    field.state.player.facing = Direction::Right;
    if moving {
        field.state.player.movement_state = MovementState::Walking;
        field.state.walk_counter = 8;
    }
    field.publish_npc_frame(false);
    field
}
#[test]
fn ready_walker_keeps_its_state_during_a_player_stride() {
    let mut field = field(true);
    let before = serde_json::to_value(&field.npc_states[0]).unwrap();
    field.update_frame(OverworldInput::none());
    assert_eq!(field.state.walk_counter, 7);
    assert_eq!(field.state.player.movement_state, MovementState::Walking);
    assert_eq!(
        serde_json::to_value(&field.npc_states[0]).unwrap(),
        before,
        "ready ordinary NPCs must not roll a direction or start a delay midstride"
    );
}
#[test]
fn idle_player_allows_the_ready_walker_to_decide() {
    let mut field = field(false);
    field.update_frame(OverworldInput::none());
    let npc = &field.npc_states[0];
    assert!(npc.walk_counter != 0 || npc.delay_counter != 0);
    assert_eq!(npc.movement_type, NpcMovementType::Wander);
    assert!(npc.visible);
}
#[test]
fn existing_npc_stride_continues_while_the_player_walks() {
    let mut field = field(true);
    let npc = &mut field.npc_states[0];
    npc.walk_counter = 4;
    npc.step_direction = Some(Direction::Down);
    field.update_frame(OverworldInput::none());
    assert_eq!(field.state.walk_counter, 7);
    assert_eq!(field.npc_states[0].walk_counter, 3);
    assert_eq!(field.npc_states[0].movement_type, NpcMovementType::Wander);
    assert!(field.npc_states[0].visible);
}
#[test]
fn delay_expiration_waits_for_a_zero_player_counter_before_new_movement() {
    let mut field = field(true);
    field.npc_states[0].delay_counter = 1;
    field.update_frame(OverworldInput::none());
    assert_eq!(field.npc_states[0].delay_counter, 0);
    assert_eq!(field.npc_states[0].walk_counter, 0);
    // A rendered update may be the field loop's wait frame. Reach the next
    // real player/NPC foreground tick before checking the ready transition.
    for _ in 0..8 {
        field.update_frame(OverworldInput::none());
        if field.state.walk_counter == 6 {
            break;
        }
    }
    assert_eq!(field.state.walk_counter, 6);
    assert_eq!(field.npc_states[0].delay_counter, 0);
    assert_eq!(field.npc_states[0].walk_counter, 0);
    // A separate constructed zero-counter boundary must permit a new roll.
    field.state.player.movement_state = MovementState::Idle;
    field.state.walk_counter = 0;
    field.player_moving_direction = 0;
    for _ in 0..8 {
        field.update_frame(OverworldInput::none());
        if field.npc_states[0].walk_counter != 0 || field.npc_states[0].delay_counter != 0 {
            break;
        }
    }
    assert!(field.npc_states[0].walk_counter != 0 || field.npc_states[0].delay_counter != 0);
    assert_eq!(field.npc_states[0].movement_type, NpcMovementType::Wander);
}
#[test]
fn offscreen_scripted_path_can_start_while_the_player_walks() {
    let mut field = field(true);
    // Route1's second object is outside the first object's/player's range.
    field.npc_states[1].scripted_path.push_back((16, 13));
    field.update_frame(OverworldInput::none());
    assert_eq!(field.state.walk_counter, 7);
    assert_eq!(
        field.npc_states[1].walk_counter,
        npc_movement::NPC_WALK_FRAMES
    );
    assert_eq!(field.npc_states[1].scripted_path.len(), 1);
    assert_eq!(field.npc_states[1].movement_type, NpcMovementType::Wander);
}
