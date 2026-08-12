//! Regression tests for cutscene forced-movement fidelity.
//!
//! Companion to tests_oak_event.rs / tests_oaks_lab.rs: these cover the
//! systematic audit sweep — relative-step movement (movePlayerRelative),
//! per-trigger-tile branching (Route 22 rival), and push-back shoves.

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

/// Elite Four entry autowalk: LoreleisRoom @load must move the player six
/// tiles up from the entrance (4,11) to (4,5). This is the end-to-end
/// check for the movePlayerRelative command (relative deltas resolved
/// against the live player position).
#[test]
fn loreleis_room_entry_autowalks_up_six_tiles() {
    let mut screen = OverworldScreen::new(MapId::LoreleisRoom, None, PokemonRedData);
    screen.state.player.x = 4;
    screen.state.player.y = 11;
    screen.state.player.facing = Direction::Up;
    screen.run_on_load();

    for _ in 0..600 {
        screen.update_frame(neutral_input());
        if screen.state.player.y == 5 {
            break;
        }
    }

    assert_eq!(
        (screen.state.player.x, screen.state.player.y),
        (4, 5),
        "entry autowalk must end six tiles up from the entrance"
    );
}

/// Route 22 first rival battle: stepping onto the lower trigger tile
/// (29,5) must walk the rival from his (25,5) spawn to (28,5), adjacent
/// and facing the player (original: RIGHT x3, first movement byte skipped).
#[test]
fn route22_rival_approaches_lower_trigger_tile() {
    let mut screen = OverworldScreen::new(MapId::Route22, None, PokemonRedData);
    screen.set_flag_live("EVENT_ROUTE22_RIVAL_WANTS_BATTLE", true);
    screen.set_flag_live("EVENT_1ST_ROUTE22_RIVAL_BATTLE", true);
    screen.state.player.x = 29;
    screen.state.player.y = 6;
    screen.state.player.facing = Direction::Up;

    let mut rival_reached = false;
    for frame in 0..1200 {
        let input = if screen.active_script_effect.is_some() {
            if frame % 40 == 0 {
                a_input()
            } else {
                neutral_input()
            }
        } else {
            up_input()
        };
        screen.update_frame(input);
        let rival = &screen.npc_states[0];
        if rival.x == 28 && rival.y == 5 {
            rival_reached = true;
            break;
        }
    }

    assert!(
        rival_reached,
        "rival never walked to (28,5); at ({},{})",
        screen.npc_states[0].x, screen.npc_states[0].y
    );
}

/// Route 22 first rival battle, upper trigger tile (29,4): the rival must
/// walk RIGHT x4 to (29,5), directly below the player.
#[test]
fn route22_rival_approaches_upper_trigger_tile() {
    let mut screen = OverworldScreen::new(MapId::Route22, None, PokemonRedData);
    screen.set_flag_live("EVENT_ROUTE22_RIVAL_WANTS_BATTLE", true);
    screen.set_flag_live("EVENT_1ST_ROUTE22_RIVAL_BATTLE", true);
    screen.state.player.x = 29;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Up;

    let mut rival_reached = false;
    for frame in 0..1200 {
        let input = if screen.active_script_effect.is_some() {
            if frame % 40 == 0 {
                a_input()
            } else {
                neutral_input()
            }
        } else {
            up_input()
        };
        screen.update_frame(input);
        let rival = &screen.npc_states[0];
        if rival.x == 29 && rival.y == 5 {
            rival_reached = true;
            break;
        }
    }

    assert!(
        rival_reached,
        "rival never walked to (29,5); at ({},{})",
        screen.npc_states[0].x, screen.npc_states[0].y
    );
}

/// SS Anne 2F rival: stepping onto the (36,8) trigger walks the rival
/// from the stairs (36,4) DOWN x3 to (36,7) (orig SSAnne2FDefaultScript).
/// This also covers the script_config toggleId wiring — without it
/// moveNpc silently no-ops.
#[test]
fn ssanne2f_rival_walks_down_to_player() {
    let mut screen = OverworldScreen::new(MapId::SSAnne2F, None, PokemonRedData);
    screen.state.player.x = 36;
    screen.state.player.y = 9;
    screen.state.player.facing = Direction::Up;

    let mut rival_reached = false;
    for frame in 0..1200 {
        let input = if screen.active_script_effect.is_some() {
            if frame % 40 == 0 {
                a_input()
            } else {
                neutral_input()
            }
        } else {
            up_input()
        };
        screen.update_frame(input);
        let rival = &screen.npc_states[1];
        if rival.x == 36 && rival.y == 7 {
            rival_reached = true;
            break;
        }
    }

    assert!(
        rival_reached,
        "rival never walked to (36,7); at ({},{})",
        screen.npc_states[1].x, screen.npc_states[1].y
    );
}

/// Silph Co 7F rival: stepping onto (3,3) walks the rival UP x3 from his
/// (3,7) spawn to (3,4) — the original skips the first movement byte on
/// this trigger. The "What kept you?" text displays BEFORE the walk.
#[test]
fn silphco7f_rival_walks_up_to_player() {
    let mut screen = OverworldScreen::new(MapId::SilphCo7F, None, PokemonRedData);
    screen.state.player.x = 3;
    screen.state.player.y = 4;
    screen.state.player.facing = Direction::Up;

    let mut rival_reached = false;
    for frame in 0..1200 {
        let input = if screen.active_script_effect.is_some() {
            if frame % 40 == 0 {
                a_input()
            } else {
                neutral_input()
            }
        } else {
            up_input()
        };
        screen.update_frame(input);
        let rival = &screen.npc_states[8];
        if rival.x == 3 && rival.y == 4 {
            rival_reached = true;
            break;
        }
    }

    assert!(
        rival_reached,
        "rival never walked to (3,4); at ({},{})",
        screen.npc_states[8].x, screen.npc_states[8].y
    );
}

/// Pokémon Tower 2F rival: no entry movement in the original — stepping
/// onto (15,5) only makes the rival (at (14,5)) face RIGHT toward the
/// player (orig EVENT_POKEMON_TOWER_RIVAL_ON_LEFT branch).
#[test]
fn pokemontower2f_rival_faces_player_on_left_trigger() {
    let mut screen = OverworldScreen::new(MapId::PokemonTower2F, None, PokemonRedData);
    screen.state.player.x = 15;
    screen.state.player.y = 6;
    screen.state.player.facing = Direction::Up;

    let mut faced = false;
    for frame in 0..1200 {
        let input = if screen.active_script_effect.is_some() {
            if frame % 40 == 0 {
                a_input()
            } else {
                neutral_input()
            }
        } else {
            up_input()
        };
        screen.update_frame(input);
        if screen.npc_states[0].facing == Direction::Right {
            faced = true;
            break;
        }
    }

    assert!(
        faced,
        "rival never faced right; facing {:?}",
        screen.npc_states[0].facing
    );
}

/// OaksLab "don't go away yet!": stepping down to row 6 before choosing a
/// starter shows Oak's line and shoves the player one tile back UP —
/// from the player's own column, not a hard-coded one.
#[test]
fn oaks_lab_dont_go_away_shoves_player_up() {
    let mut screen = OverworldScreen::new(MapId::OaksLab, None, PokemonRedData);
    screen.set_flag_live("EVENT_FOLLOWED_OAK_INTO_LAB", true);
    screen.set_flag_live("EVENT_OAK_ASKED_TO_CHOOSE_MON", true);
    screen.state.player.x = 5;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Down;

    // Walk down onto the trigger row (y=6).
    let down_input = || OverworldInput::new(false, true, false, false, false, false, false, false);
    let mut pushed = false;
    for frame in 0..1200 {
        let input = if screen.active_script_effect.is_some() {
            if frame % 40 == 0 {
                a_input()
            } else {
                neutral_input()
            }
        } else {
            down_input()
        };
        screen.update_frame(input);
        // The shove runs after the dialogue is dismissed: player back at y=5.
        if screen.state.player.x == 5 && screen.state.player.y == 5 && frame > 60 {
            pushed = true;
            break;
        }
    }

    assert!(
        pushed,
        "player was not shoved back to (5,5); at ({},{})",
        screen.state.player.x, screen.state.player.y
    );
}

/// Regression (review F2): injecting/editing ONE map's scene must not shadow
/// the other 247 embedded maps. The provider only carries the injected
/// override; misses fall back to the embedded ASTs.
#[test]
fn injected_scene_does_not_shadow_other_maps() {
    let mut screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    let injected = "game_scene PalletTown { @storyline(\"editorInjected\") { setFlag(\"TEST_FLAG\") } }";
    screen
        .reload_scene_with_config("PalletTown", injected, None)
        .expect("inject PalletTown scene");
    // The injected scene replaces PalletTown's own…
    let pallet = screen
        .map_scene_ast("PalletTown")
        .expect("PalletTown AST resolves");
    assert!(
        pallet.storylines.iter().any(|s| s.name == "editorInjected"),
        "injected storyline must be the resolved PalletTown scene"
    );
    // …but every other map still resolves from the embedded table.
    assert!(
        screen.map_scene_ast("ViridianCity").is_some(),
        "non-injected map must fall back to the embedded AST"
    );
    assert!(
        screen.shared_scene_ast().is_some(),
        "shared pokecenter AST must still resolve"
    );
}
