//! Reproduction tests for the Pallet Town north-exit Oak cutscene.
//!
//! Original Gen I behavior: stepping on (10,1) or (11,1) without having
//! followed Oak into the lab triggers the "Hey! Wait! Don't go out!" event.
//!
//! These tests drive a real `OverworldScreen` with the on-disk `.scene`
//! scripts (the same sources the game compiles at runtime).

use super::screen::OverworldScreen;
use super::{Direction, OverworldInput};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;

fn maps_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../pokered-data/maps")
}

fn up_input() -> OverworldInput {
    OverworldInput::new(true, false, false, false, false, false, false, false)
}

fn neutral_input() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

fn flag_set(screen: &OverworldScreen<PokemonRedData>, name: &str) -> bool {
    screen.script_flags().get(name).copied().unwrap_or(false)
}

/// Sanity: with an explicit scripts dir the PalletTown `.scene` compiles and
/// registers, so the coord-event function actually exists.
#[test]
fn pallet_scene_loads_with_scripts_dir() {
    let screen = OverworldScreen::new(MapId::PalletTown, Some(maps_dir()), PokemonRedData);
    assert!(
        screen.map_script_config.coord_event_fn(10, 1).is_some(),
        "script_config must bind a coord event at (10,1)"
    );
}

/// Stepping onto (10,1) must fire the Oak cutscene: the
/// EVENT_OAK_APPEARED_IN_PALLET flag is set by the first block of the
/// coordNorthExit storyline, before any text shows.
#[test]
fn oak_stops_player_leaving_north() {
    let mut screen = OverworldScreen::new(MapId::PalletTown, Some(maps_dir()), PokemonRedData);
    screen.state.player.x = 10;
    screen.state.player.y = 3;
    screen.state.player.facing = Direction::Up;

    let mut oak_flag_frame = None;
    for frame in 0..600 {
        // Once a script effect is running, release the d-pad (the cutscene
        // owns the joypad via setJoyIgnore anyway).
        let input = if screen.active_script_effect.is_some() {
            neutral_input()
        } else {
            up_input()
        };
        screen.update_frame(input);
        if oak_flag_frame.is_none() && flag_set(&screen, "EVENT_OAK_APPEARED_IN_PALLET") {
            oak_flag_frame = Some(frame);
            break;
        }
    }

    assert!(
        oak_flag_frame.is_some(),
        "Oak event never fired: player at ({},{}) on {:?}",
        screen.state.player.x, screen.state.player.y, screen.state.current_map,
    );
    assert_eq!(
        screen.state.current_map,
        MapId::PalletTown,
        "player must not reach Route 1 before the Oak event"
    );
}

/// Full-cutscene regression test: Oak must walk up to the player, and
/// the escort route into the lab must never cross unwalkable tiles.
/// Run for both north-exit trigger tiles (x = 10 and x = 11).
fn run_oak_escort(player_x: u16) {
    let mut screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    screen.state.player.x = player_x;
    screen.state.player.y = 3;
    screen.state.player.facing = Direction::Up;

    let a_input = || OverworldInput::new(false, false, false, false, true, false, false, false);

    let mut player_trail: Vec<(u16, u16)> = vec![(player_x, 3)];
    let mut oak_adjacent_when_follow_started = false;
    let mut oak_stood_below_player = false;
    let mut cutscene_started = false;
    let mut saw_dialogue = false;
    let mut escort_trail: Vec<(u16, u16)> = Vec::new();
    let mut frames = 0;
    for frame in 0..6000 {
        frames = frame;
        let input = if screen.active_script_effect.is_some() {
            // During cutscene: tap A periodically to advance dialogue.
            if frame % 40 == 0 {
                a_input()
            } else {
                neutral_input()
            }
        } else {
            up_input()
        };
        screen.update_frame(input);

        if screen.active_script_effect.is_some() {
            cutscene_started = true;
        }
        if screen.pending_dialogue.is_some() {
            saw_dialogue = true;
        }
        let p = (screen.state.player.x, screen.state.player.y);
        if screen.state.current_map == MapId::PalletTown && player_trail.last() != Some(&p) {
            player_trail.push(p);
            if cutscene_started {
                escort_trail.push(p);
            }
        }
        let oak = &screen.npc_states[0];
        if oak.visible && oak.x == player_x && oak.y == 2 && p == (player_x, 1) {
            oak_stood_below_player = true;
        }
        if matches!(
            screen.active_script_effect,
            Some(super::script_bridge::ScriptEffect::FollowNpc { .. })
        ) && !oak_adjacent_when_follow_started
        {
            // Follow phase must begin with Oak right next to the player;
            // otherwise the player's shadow-walk starts tiles away and
            // cuts straight through buildings.
            let dist = oak.x.abs_diff(p.0) + oak.y.abs_diff(p.1);
            oak_adjacent_when_follow_started = dist == 1;
        }
        if screen.state.current_map != MapId::PalletTown {
            break;
        }
    }

    assert!(
        saw_dialogue,
        "Oak's dialogue never showed during the cutscene (x={})",
        player_x
    );
    assert!(
        oak_stood_below_player,
        "Oak never walked up to the player at ({},1)",
        player_x
    );
    assert!(
        oak_adjacent_when_follow_started,
        "follow phase started with Oak away from the player (x={})",
        player_x
    );
    assert_eq!(
        screen.state.current_map,
        MapId::OaksLab,
        "escort must warp the player into Oak's lab (x={}, stopped at {:?} after {} frames)",
        player_x,
        player_trail.last(),
        frames
    );

    // Every escort-phase tile the player walked on (except the lab door
    // warp tile) must be genuinely walkable on the Pallet Town map —
    // no cutting through the lab building. (screen.map_data has already
    // been swapped to OaksLab by the post-warp break, so reload Pallet
    // Town explicitly.)
    let (pallet_map, _) =
        crate::overworld::map_data_loading::load_full_map_data_concrete(MapId::PalletTown);
    for &(x, y) in &escort_trail {
        if (x, y) == (12, 11) {
            continue; // door warp tile: passable only via the warp itself
        }
        assert!(
            super::update::is_script_walkable_tile(&pallet_map, x, y),
            "player walked onto unwalkable tile ({},{}) (start x={}); escort trail={:?}",
            x,
            y,
            player_x,
            escort_trail
        );
    }
}

#[test]
fn oak_escorts_player_to_lab_from_left_exit() {
    run_oak_escort(10);
}

#[test]
fn oak_escorts_player_to_lab_from_right_exit() {
    run_oak_escort(11);
}

/// Mirror of the app path: `pokered-app` passes `scripts_dir = None` unless
/// `--scripts-dir` is given. The embedded scenes baked into pokered-data at
/// build time must make the Oak event fire on this path too — this is the
/// regression test for the original bug (player walked onto Route 1
/// unstopped because no map script was ever loaded).
#[test]
fn app_default_path_fires_oak_event() {
    let mut screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    screen.state.player.x = 10;
    screen.state.player.y = 3;
    screen.state.player.facing = Direction::Up;

    assert!(
        screen.script_engine.has_function("coordNorthExit"),
        "embedded PalletTown scene must provide coordNorthExit"
    );

    let mut fired = false;
    for _ in 0..600 {
        let input = if screen.active_script_effect.is_some() {
            neutral_input()
        } else {
            up_input()
        };
        screen.update_frame(input);
        if flag_set(&screen, "EVENT_OAK_APPEARED_IN_PALLET") {
            fired = true;
            break;
        }
    }

    assert!(
        fired,
        "Oak event must fire on the default (embedded) script path"
    );
    assert_eq!(
        screen.state.current_map,
        MapId::PalletTown,
        "player must not reach Route 1 before the Oak event"
    );
}

/// Chinese-mode smoke test: with the script language set to "zh", the Oak
/// cutscene dialogue must resolve through `game.t` to the Chinese text
/// (the `@t("en", "中文")` bilingual blocks in PalletTown/script.scene).
#[test]
fn oak_dialogue_is_chinese_in_zh_mode() {
    let mut screen = OverworldScreen::new(MapId::PalletTown, Some(maps_dir()), PokemonRedData);
    screen.set_script_lang("zh");
    screen.state.player.x = 10;
    screen.state.player.y = 3;
    screen.state.player.facing = Direction::Up;

    for _ in 0..900 {
        let input = if screen.active_script_effect.is_some() {
            neutral_input()
        } else {
            up_input()
        };
        screen.update_frame(input);
        if let Some(dlg) = &screen.pending_dialogue {
            if let Some((d1, _d2)) = dlg.get_display_text() {
                if d1.is_empty() {
                    continue; // typewriter hasn't revealed any characters yet
                }
                assert!(
                    d1.contains('大'),
                    "zh-mode dialogue must be Chinese, got: {d1:?}"
                );
                assert!(
                    !d1.contains("OAK"),
                    "zh-mode dialogue leaked English, got: {d1:?}"
                );
                return;
            }
        }
    }
    panic!("zh-mode Oak dialogue never appeared");
}
