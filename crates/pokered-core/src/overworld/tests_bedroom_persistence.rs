use super::{screen::PendingWarp, Direction, OverworldInput, OverworldScreen};
use crate::save::{game_data::REDS_HOUSE_2F_SCRIPT_INDEX, SaveData};
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};

const NOOP: &str = "SCRIPT_REDSHOUSE2F_NOOP";

fn bedroom(script: u8) -> OverworldScreen<PokemonRedData> {
    let mut save = SaveData::new();
    save.game_data.game_progress_flags[REDS_HOUSE_2F_SCRIPT_INDEX] = script;
    let mut screen = OverworldScreen::new(MapId::RedsHouse2F, None, PokemonRedData);
    screen.state.player.x = 3;
    screen.state.player.y = 6;
    screen.state.player.facing = Direction::Down;
    screen.restore_loaded_save_flags(&save, None);
    screen
}

fn settle(screen: &mut OverworldScreen<PokemonRedData>) {
    for _ in 0..32 {
        screen.update_frame(OverworldInput::new(
            false, false, false, false, false, false, false, false,
        ));
    }
    assert!(screen.script_engine_idle());
    assert!(screen.active_script_effect.is_none());
}

fn enter(screen: &mut OverworldScreen<PokemonRedData>, map: MapId, x: u8, y: u8) {
    screen.pending_warp = Some(PendingWarp {
        dest_map: map,
        dest_x: x,
        dest_y: y,
        save_last_map: false,
        arrival_spin: false,
    });
    screen.commit_pending_warp();
}

#[test]
fn bedroom_initializes_once_across_post_battle_refresh_and_map_reentry() {
    let mut screen = bedroom(0);
    screen.run_on_load();
    settle(&mut screen);
    assert_eq!(screen.state.player.facing, Direction::Up);
    assert!(screen.unified_flags().get_flag(NOOP));

    screen.state.player.facing = Direction::Down;
    screen.rerun_map_on_load_script();
    assert!(screen.active_script_effect.is_none());
    settle(&mut screen);
    assert_eq!(screen.state.player.facing, Direction::Down);

    enter(&mut screen, MapId::RedsHouse1F, 7, 1);
    settle(&mut screen);
    enter(&mut screen, MapId::RedsHouse2F, 3, 6);
    let arrival_facing = screen.state.player.facing;
    assert!(screen.active_script_effect.is_none());
    settle(&mut screen);
    assert_eq!(screen.state.player.facing, arrival_facing);
    assert!(screen.unified_flags().get_flag(NOOP));
}

#[test]
fn original_saved_noop_keeps_continue_facing() {
    let mut screen = bedroom(1);
    screen.run_on_load();
    assert!(screen.active_script_effect.is_none());
    settle(&mut screen);
    assert_eq!(screen.state.player.facing, Direction::Down);
    assert!(screen.unified_flags().get_flag(NOOP));
}

#[cfg(not(feature = "script-boa"))]
#[test]
fn bedroom_snapshot_resumes_initialization_and_keeps_the_saved_noop() {
    use crate::snapshot::OverworldSnapshot;

    let mut screen = bedroom(0);
    screen.run_on_load();
    assert!(screen.active_script_effect.is_some());
    let snapshot = OverworldSnapshot::capture(&screen);
    let mut resumed = bedroom(0);
    snapshot.restore_into(&mut resumed);
    settle(&mut resumed);
    assert_eq!(resumed.state.player.facing, Direction::Up);
    assert!(resumed.unified_flags().get_flag(NOOP));

    resumed.state.player.facing = Direction::Down;
    let snapshot = OverworldSnapshot::capture(&resumed);
    let mut continued = bedroom(0);
    snapshot.restore_into(&mut continued);
    continued.run_on_load();
    assert!(continued.active_script_effect.is_none());
    settle(&mut continued);
    assert_eq!(continued.state.player.facing, Direction::Down);
    assert!(continued.unified_flags().get_flag(NOOP));
}
