//! Regression tests for the Pokécenter nurse heal sequence.
//!
//! The nurse scripts turn her toward the healing machine (RIGHT) while it
//! runs and back DOWN afterwards via `faceNpc("1", ...)`. The numeric id is
//! her text id (1) — she is object 0 on every Pokécenter map, so resolving
//! "1" as the 0-based object index silently turned the SECOND NPC instead
//! and the nurse never moved.

use crate::alloc_prelude::Vec;
use super::screen::OverworldScreen;
use super::{Direction, OverworldInput};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;

fn maps_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../pokered-data/maps")
}

fn input(a: bool) -> OverworldInput {
    OverworldInput::new(false, false, false, false, a, false, false, false)
}

/// Talking to the nurse and answering YES must face her LEFT (toward the
/// healing machine, original image index $18 & $f = $8 = StandingLeft) for
/// the whole animation, then back DOWN toward the player. The gentleman
/// (object 1, map-facing LEFT) must never move — `faceNpc("1")` used to hit
/// him instead of the nurse.
#[test]
fn nurse_faces_machine_during_heal_then_turns_back() {
    let mut screen =
        OverworldScreen::new(MapId::PewterPokecenter, Some(maps_dir()), PokemonRedData);
    screen.state.player.x = 3;
    screen.state.player.y = 3;
    screen.state.player.facing = Direction::Up;

    let mut saw_machine = false;
    let mut nurse_faced_machine = false;
    let mut nurse_turned_back = false;
    let mut gentleman_moved = false;

    for frame in 0..6000 {
        // A rising edge every 40 frames: advances dialogue, confirms the
        // YES choice (cursor starts on it), closes the closing lines.
        let a = frame % 40 == 0;
        screen.update_frame(input(a));

        if screen.pending_healing_machine.is_some() {
            saw_machine = true;
            if screen.npc_states[0].facing == Direction::Left {
                nurse_faced_machine = true;
            }
            if screen.npc_states[1].facing != Direction::Left {
                gentleman_moved = true;
            }
        }

        // Machine overlay gone + script fully ended (no pending effect and
        // no dialogue box): the nurse must face the player again.
        if saw_machine
            && screen.pending_healing_machine.is_none()
            && screen.active_script_effect.is_none()
            && screen.pending_dialogue.is_none()
        {
            if screen.npc_states[0].facing == Direction::Down {
                nurse_turned_back = true;
            }
            if screen.npc_states[1].facing != Direction::Left {
                gentleman_moved = true;
            }
            break;
        }
    }

    assert!(saw_machine, "healing machine animation never ran");
    assert!(
        nurse_faced_machine,
        "nurse never faced the machine (left) during healing; facing {:?}",
        screen.npc_states[0].facing
    );
    assert!(
        nurse_turned_back,
        "nurse did not turn back down after healing; facing {:?}",
        screen.npc_states[0].facing
    );
    assert!(
        !gentleman_moved,
        "faceNpc(\"1\") must target the nurse (text id 1), not the gentleman (object 1)"
    );
}

#[test]
fn nurse_bows_after_fighting_fit_and_restores_pose_after_farewell() {
    for map in [MapId::ViridianPokecenter,MapId::PewterPokecenter,MapId::CeruleanPokecenter,
        MapId::VermilionPokecenter,MapId::LavenderPokecenter,MapId::CeladonPokecenter,
        MapId::FuchsiaPokecenter,MapId::SaffronPokecenter,MapId::CinnabarPokecenter,
        MapId::MtMoonPokecenter,MapId::RockTunnelPokecenter,MapId::IndigoPlateauLobby] {
        let mut screen=OverworldScreen::new(map,None,PokemonRedData);
        screen.state.player.x=screen.npc_states[0].x;
        screen.state.player.y=screen.npc_states[0].y+2;
        screen.state.player.facing=Direction::Up;
        let mut fit=false;let mut bow=false;let mut farewell=false;let mut completed=false;
        for frame in 0..8000 {
            if let Some(dialogue)=screen.pending_dialogue.as_mut() {dialogue.skip_to_full_page();}
            screen.update_frame(input(frame%40==0));
            if let Some(dialogue)=&screen.pending_dialogue {
                let text=dialogue.pages().iter().map(|p|format!("{} {}",p.line1,p.line2)).collect::<Vec<_>>().join(" ");
                let text=text.split_whitespace().collect::<Vec<_>>().join(" ");
                if text.contains("fighting fit") || text.contains("fully healed") {fit=true;assert_ne!(screen.npc_states[0].scripted_frame,Some(1),"{map:?}: bow must follow fighting-fit text");}
                if text.contains("hope to see") {
                    farewell=true;assert_eq!(screen.npc_states[0].scripted_frame,Some(1),"{map:?}: keep bow through farewell");
                }
            }
            if screen.npc_states[0].scripted_frame==Some(1) {
                assert!(fit,"{map:?}: bow before fighting-fit text");
                assert!(screen.pending_healing_machine.is_none(),"{map:?}: bow while machine active");
                bow=true;
            }
            if farewell && screen.pending_dialogue.is_none() && screen.active_script_effect.is_none() {
                assert_eq!(screen.npc_states[0].scripted_frame,None,"{map:?}: clear bow override");
                assert_eq!(screen.npc_states[0].facing,Direction::Down);
                completed=true;break;
            }
        }
        assert!(fit && bow && farewell && completed,"{map:?}: incomplete healing/bow flow");
    }
}
