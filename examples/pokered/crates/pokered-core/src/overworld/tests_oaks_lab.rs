//! Reproduction tests for the OaksLab entry cutscene.
//!
//! Original Gen I behavior (scripts/OaksLab.asm): after following Oak in,
//! Oak (OAK2) walks up 3 from (5,10) to (5,7), swaps to his desk sprite
//! (OAK1 at (5,2)), then the player is force-walked UP 8 from the door
//! (5,11) to (5,3) — right in front of Oak's desk — and only THEN does
//! the "Gramps! I'm fed up with waiting!" / choose-mon dialogue run.

use super::screen::OverworldScreen;
use super::{Direction, OverworldInput};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;

fn neutral_input() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

fn a_input() -> OverworldInput {
    OverworldInput::new(false, false, false, false, true, false, false, false)
}

/// Entering the lab after the Pallet interception must auto-walk the
/// player up to (5,3), in front of Oak, before any dialogue shows.
#[test]
fn oaks_lab_entry_walks_player_to_oak_before_dialogue() {
    let mut screen = OverworldScreen::new(MapId::OaksLab, None, PokemonRedData);
    screen.set_flag_live("EVENT_OAK_APPEARED_IN_PALLET", true);
    screen.state.player.x = 5;
    screen.state.player.y = 11;
    screen.state.player.facing = Direction::Up;
    // Mirrors the app: on_load re-fires after warp-in with flags seeded.
    screen.run_on_load();

    let mut player_pos_at_first_dialogue: Option<(u16, u16)> = None;
    for frame in 0..3000 {
        let input = if frame % 40 == 0 { a_input() } else { neutral_input() };
        screen.update_frame(input);
        if player_pos_at_first_dialogue.is_none() && screen.pending_dialogue.is_some() {
            player_pos_at_first_dialogue = Some((screen.state.player.x, screen.state.player.y));
        }
        if screen.pending_dialogue.is_some() && frame > 200 {
            break;
        }
    }

    assert_eq!(
        player_pos_at_first_dialogue,
        Some((5, 3)),
        "dialogue must start only after the player is walked up to Oak's desk"
    );
}
