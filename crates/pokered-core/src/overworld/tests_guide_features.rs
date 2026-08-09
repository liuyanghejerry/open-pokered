//! Regression tests for the scripted-guide / current-sweep features:
//! Pewter City escorts (followNpc), the OaksLab rival challenge flow, and
//! the Seafoam Islands B4F water-current sweeps.

use super::screen::OverworldScreen;
use super::{Direction, OverworldInput};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;

fn neutral_input() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

fn up_input() -> OverworldInput {
    OverworldInput::new(true, false, false, false, false, false, false, false)
}

fn a_input() -> OverworldInput {
    OverworldInput::new(false, false, false, false, true, false, false, false)
}

fn flag_set(screen: &OverworldScreen<PokemonRedData>, name: &str) -> bool {
    screen.script_flags().get(name).copied().unwrap_or(false)
}

/// Drive a cutscene frame: tap A periodically while a script effect runs.
fn cutscene_input(screen: &OverworldScreen<PokemonRedData>, frame: u32) -> OverworldInput {
    if screen.active_script_effect.is_some() && frame % 40 == 0 {
        a_input()
    } else {
        neutral_input()
    }
}

/// Pewter City east-exit interception: before Brock is beaten, stepping
/// onto (36,17) makes the gym guide escort the player to the gym front.
/// Original ends with the player at (11,18) and the guide at (12,18) in
/// front of the gym sign; followNpc lands both on the guide's final tile
/// (12,18). Afterwards the guide resets to his (35,16) spawn and is shown
/// again (orig PewterCityResetYoungsterScript).
#[test]
fn pewter_gym_guy_escorts_player_to_gym() {
    let mut screen = OverworldScreen::new(MapId::PewterCity, None, PokemonRedData);
    screen.state.player.x = 36;
    screen.state.player.y = 18;
    screen.state.player.facing = Direction::Up;

    let mut arrived = false;
    for frame in 0..6000 {
        let input = if screen.active_script_effect.is_some() {
            cutscene_input(&screen, frame)
        } else if arrived {
            // The storyline is between effects; stand still and let the
            // walk-off + respawn play out.
            neutral_input()
        } else {
            up_input()
        };
        screen.update_frame(input);
        if (screen.state.player.x, screen.state.player.y) == (12, 18) {
            arrived = true;
        }
        let guy = &screen.npc_states[4];
        if arrived && guy.visible && guy.x == 35 && guy.y == 16 {
            break;
        }
    }

    assert_eq!(
        screen.state.current_map,
        MapId::PewterCity,
        "the escort must not warp the player into the gym mid-cutscene"
    );
    assert!(
        arrived,
        "escort never delivered the player to the gym front (12,18); at ({},{})",
        screen.state.player.x, screen.state.player.y
    );
    // The guide (npc 5) resets to his spawn and is shown again.
    let guy = &screen.npc_states[4];
    assert!(
        guy.visible && guy.x == 35 && guy.y == 16,
        "gym guide must reset to his (35,16) spawn after the escort, got ({},{}) vis={}",
        guy.x,
        guy.y,
        guy.visible
    );
}

/// OaksLab rival challenge: with a starter chosen, walking down to row 6
/// makes the rival walk up to the player, demand a battle, then exit and
/// hide mid-map (never on the door tile).
#[test]
fn oaks_lab_rival_challenges_on_exit_row() {
    let mut screen = OverworldScreen::new(MapId::OaksLab, None, PokemonRedData);
    screen.set_flag_live("EVENT_FOLLOWED_OAK_INTO_LAB", true);
    screen.set_flag_live("EVENT_OAK_ASKED_TO_CHOOSE_MON", true);
    screen.set_flag_live("EVENT_GOT_STARTER", true);
    screen.state.player.x = 5;
    screen.state.player.y = 7;
    screen.state.player.facing = Direction::Up;

    let mut rival_confronted = false;
    let mut battle_started = false;
    for frame in 0..6000 {
        if screen.script_awaiting_battle {
            battle_started = true;
            screen.resume_script_after_battle("win");
        }
        let input = if screen.active_script_effect.is_some() {
            cutscene_input(&screen, frame)
        } else {
            up_input()
        };
        screen.update_frame(input);

        let rival = &screen.npc_states[0];
        if rival.x == 5 && rival.y == 5 {
            rival_confronted = true;
        }
        if flag_set(&screen, "EVENT_BATTLED_RIVAL_IN_OAKS_LAB")
            && !rival.visible
            && screen.active_script_effect.is_none()
        {
            break;
        }
    }

    assert!(
        rival_confronted,
        "rival never walked up to (5,5) to confront the player; at ({},{})",
        screen.npc_states[0].x, screen.npc_states[0].y
    );
    assert!(battle_started, "the challenge battle never started");
    assert!(
        flag_set(&screen, "EVENT_BATTLED_RIVAL_IN_OAKS_LAB"),
        "EVENT_BATTLED_RIVAL_IN_OAKS_LAB must be set after the battle"
    );
    let rival = &screen.npc_states[0];
    assert!(
        !rival.visible && rival.y == 10 && rival.x != 5,
        "rival must exit with a sidestep and hide mid-map at y=10, got ({},{}) vis={}",
        rival.x,
        rival.y,
        rival.visible
    );
}

/// Seafoam B4F south-east current: while the B3F boulders are not both
/// down, standing on the ladder row (20,17) shoves the player UP x2.
/// Once both boulders are down the shove stops.
#[test]
fn seafoam_southeast_current_shoves_until_boulders_down() {
    let mut screen = OverworldScreen::new(MapId::SeafoamIslandsB4F, None, PokemonRedData);
    // Warp-arrival path: load_map_script registers the coord triggers.
    screen.load_map_script(MapId::SeafoamIslandsB4F);
    screen.state.player.x = 20;
    screen.state.player.y = 17;
    screen.state.player.facing = Direction::Up;

    for frame in 0..600 {
        let input = cutscene_input(&screen, frame);
        screen.update_frame(input);
        if screen.state.player.y == 15 {
            break;
        }
    }
    assert_eq!(
        (screen.state.player.x, screen.state.player.y),
        (20, 15),
        "current must shove the player from (20,17) to (20,15)"
    );

    // With both B3F boulders down, stepping there again is safe.
    let mut screen = OverworldScreen::new(MapId::SeafoamIslandsB4F, None, PokemonRedData);
    screen.load_map_script(MapId::SeafoamIslandsB4F);
    screen.set_flag_live("EVENT_SEAFOAM3_BOULDER1_DOWN_HOLE", true);
    screen.set_flag_live("EVENT_SEAFOAM3_BOULDER2_DOWN_HOLE", true);
    screen.state.player.x = 20;
    screen.state.player.y = 17;
    screen.state.player.facing = Direction::Up;

    for frame in 0..300 {
        let input = cutscene_input(&screen, frame);
        screen.update_frame(input);
    }
    assert_eq!(
        (screen.state.player.x, screen.state.player.y),
        (20, 17),
        "no shove expected once both B3F boulders are down"
    );
}

/// Seafoam B4F west-channel sweep: while this floor's boulders are not
/// both down, surfing onto (4,14) sweeps the player UP, RIGHT x3, UP x3
/// to (7,10); (5,14) sweeps UP, RIGHT x2, UP x3 — also to (7,10)
/// (orig RLEs consumed LIFO).
#[test]
fn seafoam_west_channel_current_sweeps_player() {
    for start_x in [4u16, 5u16] {
        let mut screen = OverworldScreen::new(MapId::SeafoamIslandsB4F, None, PokemonRedData);
        screen.load_map_script(MapId::SeafoamIslandsB4F);
        screen.state.player.x = start_x;
        screen.state.player.y = 14;
        screen.state.player.facing = Direction::Up;

        for frame in 0..1200 {
            let input = cutscene_input(&screen, frame);
            screen.update_frame(input);
            if (screen.state.player.x, screen.state.player.y) == (7, 10) {
                break;
            }
        }
        assert_eq!(
            (screen.state.player.x, screen.state.player.y),
            (7, 10),
            "west channel must sweep the player from ({},14) to (7,10)",
            start_x
        );
    }
}
