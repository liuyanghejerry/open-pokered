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
                if text.contains("fighting fit") {fit=true;assert_ne!(screen.npc_states[0].scripted_frame,Some(1),"{map:?}: bow must follow fighting-fit text");}
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

#[test]
fn healing_keeps_last_message_visible_during_machine_and_bow() {
    let mut screen=OverworldScreen::new(MapId::ViridianPokecenter,None,PokemonRedData);
    screen.state.player.x=3;screen.state.player.y=3;screen.state.player.facing=Direction::Up;
    let mut machine=false;let mut bow=false;let mut ended=false;
    for frame in 0..8000 {
        if let Some(d)=screen.pending_dialogue.as_mut() {d.skip_to_full_page();}
        screen.update_frame(input(frame%40==0));
        if screen.pending_healing_machine.is_some() {
            assert!(screen.pending_dialogue.is_none());
            assert!(screen.displayed_field_dialogue().is_some(),"machine must retain need-your-Pokemon message");
            machine=true;
        }
        if screen.npc_states[0].scripted_frame==Some(1) && screen.pending_dialogue.is_none() {
            assert!(screen.displayed_field_dialogue().is_some(),"bow wait must retain fighting-fit message");
            bow=true;
        }
        if bow && screen.active_script_effect.is_none() && screen.pending_dialogue.is_none() {
            assert!(!screen.retain_healing_text);
            assert!(screen.displayed_field_dialogue().is_none(),"clear text when routine returns");
            ended=true;break;
        }
    }
    assert!(machine && bow && ended);
}

#[test]
fn unrelated_script_delay_does_not_retain_old_dialogue() {
    let mut screen=OverworldScreen::new(MapId::PalletTown,None,PokemonRedData);
    screen.last_script_dialogue=Some(super::screen::BedroomDialogue::from_message("Old text"));
    screen.active_script_effect=Some(super::script_bridge::ScriptEffect::Delay{frames:10,frames_remaining:10});
    assert!(screen.displayed_field_dialogue().is_none());
}

#[test]
fn retained_healing_text_round_trips_snapshot_and_clears_on_map_load() {
    let mut screen=OverworldScreen::new(MapId::ViridianPokecenter,None,PokemonRedData);
    screen.retain_healing_text=true;
    let mut text=super::screen::BedroomDialogue::from_message("Fighting fit!");
    text.skip_to_full_page();screen.last_script_dialogue=Some(text);
    screen.active_script_effect=Some(super::script_bridge::ScriptEffect::Delay{frames:20,frames_remaining:12});
    let saved=crate::snapshot::OverworldSnapshot::capture(&screen);
    let mut restored=OverworldScreen::new(MapId::ViridianPokecenter,None,PokemonRedData);
    saved.restore_into(&mut restored);
    assert!(restored.retain_healing_text);
    assert_eq!(restored.displayed_field_dialogue().unwrap().get_display_text(),screen.displayed_field_dialogue().unwrap().get_display_text());
    restored.warp_to_map(MapId::PalletTown,5,6);
    for _ in 0..100 {restored.update_frame(input(false));}
    assert_eq!(restored.state.current_map,MapId::PalletTown);
    assert!(!restored.retain_healing_text);
    assert!(restored.displayed_field_dialogue().is_none());
    let mut legacy=serde_json::to_value(saved).unwrap();
    legacy.as_object_mut().unwrap().remove("retain_healing_text");
    let old:crate::snapshot::OverworldSnapshot=serde_json::from_value(legacy).unwrap();
    assert!(!old.retain_healing_text);
}


#[test]
fn healing_balls_follow_original_thirty_frame_intervals() {
    use super::script_bridge::{HealingMachinePhase, ScriptEffect};
    for count in 1..=6 {
        let mut screen = OverworldScreen::new(MapId::ViridianPokecenter, None, PokemonRedData);
        screen.party_count = count;
        screen.active_script_effect = Some(ScriptEffect::AnimateHealingMachine {
            phase: HealingMachinePhase::FadeOutMusic, frames_remaining: 0,
        });
        let mut ball_frames = Vec::new();
        let mut previous = 0;
        for frame in 0..240 {
            screen.update_frame(input(false));
            if let Some(machine) = &screen.pending_healing_machine {
                if machine.pokeballs_visible != previous {
                    ball_frames.push(frame);
                    previous = machine.pokeballs_visible;
                }
            }
        }
        assert_eq!(ball_frames.len(), count as usize);
        for pair in ball_frames.windows(2) { assert_eq!(pair[1] - pair[0], 30); }
    }
}


#[test]
fn healing_palette_has_eight_ten_frame_intervals() {
    use super::script_bridge::{HealingMachinePhase, ScriptEffect};
    let mut screen = OverworldScreen::new(MapId::ViridianPokecenter, None, PokemonRedData);
    screen.party_count = 5;
    screen.active_script_effect = Some(ScriptEffect::AnimateHealingMachine {
        phase: HealingMachinePhase::FadeOutMusic, frames_remaining: 0,
    });
    let mut toggles = Vec::new();
    let mut previous = false;
    let mut finished = None;
    for frame in 0..400 {
        screen.update_frame(input(false));
        if let Some(machine) = &screen.pending_healing_machine {
            if matches!(machine.phase, HealingMachinePhase::FlashSprite { .. }) {
                if machine.flash_active != previous { toggles.push(frame); }
                previous = machine.flash_active;
            } else if matches!(machine.phase, HealingMachinePhase::WaitForMusic) {
                finished = Some(frame); break;
            }
        }
    }
    assert_eq!(toggles.len(), 8, "Original FlashSprite8Times performs eight toggles");
    for pair in toggles.windows(2) { assert_eq!(pair[1] - pair[0], 10); }
    assert_eq!(finished.unwrap() - toggles[0], 80);
}


#[test]
fn healing_waits_for_live_music_then_thirty_two_frames() {
    use super::script_bridge::{HealingMachinePhase, ScriptEffect};
    let mut screen = OverworldScreen::new(MapId::ViridianPokecenter, None, PokemonRedData);
    screen.party_count = 5;
    screen.active_script_effect = Some(ScriptEffect::AnimateHealingMachine {
        phase: HealingMachinePhase::FadeOutMusic, frames_remaining: 0,
    });
    screen.script_music_playing = true;
    for _ in 0..500 { screen.update_frame(input(false)); }
    assert!(matches!(screen.pending_healing_machine.as_ref().unwrap().phase,
        HealingMachinePhase::WaitForMusic), "must not interrupt a still-playing song");
    screen.script_music_playing = false;
    screen.update_frame(input(false));
    assert!(matches!(screen.pending_healing_machine.as_ref().unwrap().phase,
        HealingMachinePhase::PostMusicDelay));
    for _ in 0..31 {
        screen.update_frame(input(false));
        assert!(screen.pending_healing_machine.is_some());
    }
    screen.update_frame(input(false));
    assert!(screen.pending_healing_machine.is_none());
    assert!(screen.audio_requests.iter().any(|request| matches!(request,
        super::screen::OverworldAudioRequest::RestoreMapMusic { map: MapId::ViridianPokecenter })));
}


#[test]
fn healing_waits_for_actual_audio_fade_and_snapshots_its_state() {
    use super::script_bridge::{HealingMachinePhase, ScriptEffect};
    let mut screen = OverworldScreen::new(MapId::ViridianPokecenter, None, PokemonRedData);
    screen.party_count = 5;
    screen.active_script_effect = Some(ScriptEffect::AnimateHealingMachine {
        phase: HealingMachinePhase::FadeOutMusic, frames_remaining: 0,
    });
    screen.update_frame(input(false));
    screen.set_healing_audio_fading(true);
    for _ in 0..80 { screen.update_frame(input(false)); }
    assert_eq!(screen.pending_healing_machine.as_ref().unwrap().pokeballs_visible, 0);
    let saved = crate::snapshot::OverworldSnapshot::capture(&screen);
    let mut restored = OverworldScreen::new(MapId::ViridianPokecenter, None, PokemonRedData);
    saved.restore_into(&mut restored);
    assert!(matches!(restored.active_script_effect, Some(ScriptEffect::AnimateHealingMachine {
        phase: HealingMachinePhase::WaitForFadeOut, frames_remaining: 1 })));
    restored.update_frame(input(false));
    assert_eq!(restored.pending_healing_machine.as_ref().unwrap().pokeballs_visible, 0);
    restored.set_healing_audio_fading(false);
    restored.update_frame(input(false));
    assert_eq!(restored.pending_healing_machine.as_ref().unwrap().pokeballs_visible, 1);
    // A snapshot written by the initial fade implementation may contain
    // its now-unused global input; serde ignores it and preserves the effect.
    let mut legacy = serde_json::to_value(saved).unwrap();
    legacy.as_object_mut().unwrap().insert("script_audio_fading".into(), serde_json::json!(true));
    let legacy: crate::snapshot::OverworldSnapshot = serde_json::from_value(legacy).unwrap();
    let mut old = OverworldScreen::new(MapId::ViridianPokecenter, None, PokemonRedData);
    legacy.restore_into(&mut old);
    old.set_healing_audio_fading(false);
    old.update_frame(input(false));
    assert_eq!(old.pending_healing_machine.as_ref().unwrap().pokeballs_visible, 1);
}


#[test]
fn all_centers_wait_three_frames_after_nurse_turns() {
    use super::script_bridge::ScriptEffect;
    for map in [MapId::ViridianPokecenter, MapId::PewterPokecenter, MapId::CeruleanPokecenter,
        MapId::VermilionPokecenter, MapId::LavenderPokecenter, MapId::CeladonPokecenter,
        MapId::FuchsiaPokecenter, MapId::SaffronPokecenter, MapId::CinnabarPokecenter,
        MapId::MtMoonPokecenter, MapId::RockTunnelPokecenter, MapId::IndigoPlateauLobby] {
        let mut screen = OverworldScreen::new(map, None, PokemonRedData);
        screen.party_count = 1;
        screen.state.player.x = screen.npc_states[0].x;
        screen.state.player.y = screen.npc_states[0].y + 2;
        screen.state.player.facing = Direction::Up;
        let mut countdown = Vec::new();
        for frame in 0..2000 {
            if let Some(d) = screen.pending_dialogue.as_mut() { d.skip_to_full_page(); }
            screen.update_frame(input(frame % 40 == 0));
            if screen.pending_healing_machine.is_some() { break; }
            if screen.npc_states[0].facing == Direction::Left {
                if let Some(ScriptEffect::Delay { frames_remaining, .. }) = &screen.active_script_effect {
                    countdown.push(*frames_remaining);
                }
            }
        }
        assert_eq!(countdown, vec![3, 2, 1], "{map:?}: original Delay3 before healing");
        assert!(screen.pending_healing_machine.is_some());
    }
}
