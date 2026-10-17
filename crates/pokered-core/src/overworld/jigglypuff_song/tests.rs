//! Actual embedded-map/ScriptVM component controls. The song sample here is
//! a component input; App tests separately exercise the real audio sequencer.
use super::*;
use crate::foreground_input::{ForegroundJoypad, A, B, SELECT, START};
use crate::overworld::BedroomDialogue;
use crate::snapshot::OverworldSnapshot;
use pokered_data::impl_traits::PokemonRedData;
type Field = OverworldScreen<PokemonRedData>;

fn state(field: &Field) -> &JigglypuffSongState {
    match field.active_script_effect.as_ref().unwrap() {
        ScriptEffect::JigglypuffSong { state } => state,
        other => panic!("expected the real npc3 song caller, got {other:?}"),
    }
}
fn step(field: &mut Field, buttons: u8) -> Vec<OverworldAudioRequest> {
    field.update_frame(ForegroundJoypad::as_field_input(buttons));
    assert_eq!(field.source_foreground_error(), None);
    let audio = core::mem::take(&mut field.audio_requests);
    assert!(
        !audio
            .iter()
            .any(|request| matches!(request, OverworldAudioRequest::PlayCry { .. })),
        "the original Jigglypuff caller has no cry"
    );
    audio
}
fn talk(npc_facing: Direction) -> Field {
    let mut field = Field::new(MapId::PewterPokecenter, None, PokemonRedData);
    let npc = field
        .npc_states
        .iter()
        .find(|npc| npc.text_id == 3)
        .unwrap();
    let (x, y) = (npc.x, npc.y);
    // Only a component's preboot player placement; the original NPC, map
    // trigger and scene VM all remain live. No synthetic effect/script.
    let (dx, dy, facing) = match npc_facing {
        Direction::Down => (0, 1, Direction::Up),
        Direction::Left => (-1, 0, Direction::Right),
        Direction::Up => (0, -1, Direction::Down),
        Direction::Right => (1, 0, Direction::Left),
    };
    field.state.player.x = (i32::from(x) + dx) as u16;
    field.state.player.y = (i32::from(y) + dy) as u16;
    field.state.player.facing = facing;
    for _ in 0..2 {
        step(&mut field, 0);
    }
    for _ in 0..8 {
        step(&mut field, A);
        if matches!(
            field.active_script_effect,
            Some(ScriptEffect::JigglypuffSong { .. })
        ) {
            break;
        }
    }
    assert!(
        matches!(
            field.active_script_effect,
            Some(ScriptEffect::JigglypuffSong { .. })
        ),
        "ordinary Pewter npc3 talk must reach the typed song"
    );
    field
}
fn to_song(field: &mut Field, buttons: u8) {
    let mut stops = 0;
    let mut songs = 0;
    for _ in 0..2000 {
        let audio = step(field, buttons);
        stops += audio
            .iter()
            .filter(|request| {
                matches!(request,
            OverworldAudioRequest::PlaySound { sound_id } if sound_id == "SFX_STOP_ALL_MUSIC")
            })
            .count();
        songs += audio
            .iter()
            .filter(|request| {
                matches!(request,
            OverworldAudioRequest::PlayMusic { music_id } if music_id == "MUSIC_JIGGLYPUFF_SONG")
            })
            .count();
        if state(field).phase == Phase::SongSpin {
            assert_eq!((stops, songs), (1, 1));
            assert_eq!(state(field).frames_remaining, 24);
            assert!(field.pending_dialogue.is_none());
            assert!(field
                .last_script_dialogue
                .as_ref()
                .unwrap()
                .waiting_for_input());
            assert!(field.retain_field_text);
            return;
        }
    }
    panic!("embedded Jigglypuff song never started");
}
fn roundtrip(field: &mut Field) {
    let json = serde_json::to_value(OverworldSnapshot::capture(field)).unwrap();
    let snapshot: OverworldSnapshot = serde_json::from_value(json.clone()).unwrap();
    snapshot.restore_into(field);
    assert_eq!(
        serde_json::to_value(OverworldSnapshot::capture(field)).unwrap(),
        json
    );
}

#[test]
fn inner_print_keeps_bg3_font_transfer_and_inner_delay3_without_joypad() {
    let mut dialogue = BedroomDialogue::from_message("x");
    dialogue.begin_field_display(true);
    let mut owner = ForegroundJoypad::default();
    owner.read_raw(0);
    owner.poll();
    owner.read_raw(A);
    // BG3 + CopyVideoDataDouble(128/8+1) + inner PrintText Delay3 =23.
    for frame in 1..23 {
        assert_eq!(dialogue.get_display_text().is_none(), frame <= 3);
        dialogue.reveal_next_char_with_source(false, &mut owner);
        assert_eq!(dialogue.char_index(), 0);
        assert_eq!(owner.last(), Some(0));
    }
    dialogue.reveal_next_char_with_source(false, &mut owner);
    assert_eq!(dialogue.char_index(), 1);
    assert_eq!(owner.last(), Some(A));
}

#[test]
fn four_real_talk_sides_spin_from_the_initial_image_in_24_blocks_without_logical_turns() {
    for initial in FACINGS {
        let mut field = talk(initial);
        to_song(&mut field, 0);
        let slot = state(&field).npc_index.unwrap();
        assert_eq!(field.npc_states[slot].text_id, 3);
        assert_eq!(field.npc_states[slot].facing, initial);
        let base = FACINGS
            .iter()
            .position(|&direction| direction == initial)
            .unwrap();
        assert_eq!(usize::from(state(&field).facing_index), base);
        let last = field.source_foreground().unwrap().last();
        field.jigglypuff_song_playing = true; // Component input, not actual audio evidence.
        for elapsed in 1_usize..=96 {
            assert!(step(&mut field, A | B | SELECT | START).is_empty());
            assert_eq!(
                usize::from(state(&field).facing_index),
                (base + elapsed / 24) % 4
            );
            let pose = field
                .npc_presentation
                .pose(MapId::PewterPokecenter, slot)
                .unwrap();
            // Preserve the existing foreground→shadow-OAM→published pipeline.
            assert_eq!(
                pose.image.facing,
                FACINGS[(base + elapsed.saturating_sub(2) / 24) % 4]
            );
            assert_eq!(pose.image.phase, 0);
            assert_eq!(field.npc_states[slot].facing, initial);
            assert_eq!(field.npc_states[slot].scripted_frame, None);
            assert_eq!(field.source_foreground().unwrap().last(), last);
        }
    }
}

#[test]
fn done_has_no_ack_stop_wait_is_32_song_completion_is_24_and_tail_is_48() {
    let mut field = talk(Direction::Down);
    for _ in 0..2000 {
        step(&mut field, 0);
        if state(&field).phase == Phase::StopAllWait {
            break;
        }
    }
    assert_eq!(state(&field).frames_remaining, 32);
    for remaining in (1..32).rev() {
        assert!(step(&mut field, B).is_empty());
        assert_eq!(state(&field).frames_remaining, remaining);
        assert_eq!(state(&field).phase, Phase::StopAllWait);
    }
    assert!(matches!(step(&mut field, 0).as_slice(),
        [OverworldAudioRequest::PlayMusic { music_id }] if music_id == "MUSIC_JIGGLYPUFF_SONG"));
    // Pre-dispatch false cannot skip the first full block. Generic CH1 can
    // remain active (restored map music); only the scoped song sample counts.
    field.script_music_playing = true;
    field.jigglypuff_song_playing = false;
    let last = field.source_foreground().unwrap().last();
    for _ in 0..23 {
        assert!(step(&mut field, SELECT | START).is_empty());
        assert_eq!(state(&field).phase, Phase::SongSpin);
        assert_eq!(field.source_foreground().unwrap().last(), last);
    }
    step(&mut field, 0);
    assert_eq!(state(&field).phase, Phase::PostSongWait);
    assert_eq!(state(&field).frames_remaining, 48);
    for _ in 0..47 {
        assert!(step(&mut field, 0).is_empty());
        assert_eq!(state(&field).phase, Phase::PostSongWait);
    }
    assert!(matches!(
        step(&mut field, 0).as_slice(),
        [OverworldAudioRequest::RestoreMapMusic {
            map: MapId::PewterPokecenter
        }]
    ));
    assert_eq!(
        state(&field).phase,
        Phase::CloseWait,
        "DoNotWait needs no fresh ACK"
    );
    assert!(field.displayed_field_dialogue().is_none());
    step(&mut field, 0);
    assert!(field.active_script_effect.is_none());
    assert!(field.script_engine_idle());
}

#[test]
fn text_speed_no_delay_and_old_or_new_held_a_preserve_the_window_until_release() {
    for (speed, no_delay, typing_buttons) in
        [(1, false, A), (3, false, 0), (5, false, A), (3, true, 0)]
    {
        let mut field = talk(Direction::Right);
        field.set_text_delay_frames(speed);
        field.set_no_text_delay(no_delay);
        to_song(&mut field, typing_buttons);
        let text = serde_json::to_value(field.last_script_dialogue.as_ref().unwrap()).unwrap();
        let last = field.source_foreground().unwrap().last();
        // Simulates completed song input; no guessed duration in the effect.
        field.jigglypuff_song_playing = false;
        for _ in 0..71 {
            step(&mut field, A | B);
            assert_eq!(
                serde_json::to_value(field.last_script_dialogue.as_ref().unwrap()).unwrap(),
                text
            );
            assert_eq!(field.source_foreground().unwrap().last(), last);
        }
        step(&mut field, A | B);
        assert_eq!(state(&field).phase, Phase::HoldARelease);
        assert!(field.pending_dialogue.as_ref().unwrap().holding_open());
        for _ in 0..4 {
            assert!(step(&mut field, A | B).is_empty());
            assert!(field.pending_dialogue.is_some());
        }
        step(&mut field, B);
        assert_eq!(state(&field).phase, Phase::CloseWait);
        assert_eq!(
            field.sfx_event,
            super::super::screen::OverworldSfxEvent::None
        );
        assert_eq!(field.no_text_delay(), no_delay);
    }
}

#[test]
fn snapshot_keeps_continuation_window_owner_and_rotation_without_restarting_audio() {
    let mut field = talk(Direction::Left);
    step(&mut field, 0);
    roundtrip(&mut field);
    to_song(&mut field, 0);
    field.jigglypuff_song_playing = true;
    for _ in 0..29 {
        step(&mut field, 0);
    }
    roundtrip(&mut field);
    assert!(
        step(&mut field, A).is_empty(),
        "restore must not restart the song"
    );
    field.jigglypuff_song_playing = false;
    for _ in 0..24 {
        if state(&field).phase == Phase::PostSongWait {
            break;
        }
        step(&mut field, A);
    }
    roundtrip(&mut field);
    for _ in 0..48 {
        step(&mut field, A);
    }
    assert_eq!(state(&field).phase, Phase::HoldARelease);
    roundtrip(&mut field);
    step(&mut field, 0);
    assert_eq!(state(&field).phase, Phase::CloseWait);
}

#[test]
fn missing_owner_snapshot_defaults_song_sample_but_cannot_manufacture_release_history() {
    let mut field = talk(Direction::Down);
    to_song(&mut field, 0);
    let mut json = serde_json::to_value(OverworldSnapshot::capture(&field)).unwrap();
    for key in [
        "source_foreground",
        "source_foreground_mode",
        "source_foreground_error",
        "jigglypuff_song_playing",
    ] {
        json.as_object_mut().unwrap().remove(key);
    }
    let snapshot: OverworldSnapshot = serde_json::from_value(json).unwrap();
    assert!(!snapshot.jigglypuff_song_playing);
    snapshot.restore_into(&mut field);
    for _ in 0..72 {
        field.update_frame(ForegroundJoypad::as_field_input(0));
    }
    assert_eq!(state(&field).phase, Phase::HoldARelease);
    assert_eq!(
        field.source_foreground_error(),
        Some(crate::foreground_input::ForegroundInputError::MissingFieldOwner)
    );
    assert!(field.pending_dialogue.is_some() && field.retain_field_text);
}

#[test]
fn image_override_does_not_survive_close_map_script_replacement_or_new_game() {
    let mut field = talk(Direction::Down);
    to_song(&mut field, 0);
    field.jigglypuff_song_playing = true;
    for _ in 0..26 {
        step(&mut field, 0);
    }
    let slot = state(&field).npc_index.unwrap();
    assert_eq!(
        field
            .npc_presentation
            .pose(MapId::PewterPokecenter, slot)
            .unwrap()
            .image
            .facing,
        Direction::Left
    );
    assert_eq!(
        state(&field).image_override(MapId::ViridianPokecenter),
        None
    );
    let mut closing = Field::new(MapId::PewterPokecenter, None, PokemonRedData);
    OverworldSnapshot::capture(&field).restore_into(&mut closing);
    closing.jigglypuff_song_playing = false;
    for _ in 0..75 {
        step(&mut closing, 0);
        if closing.active_script_effect.is_none() {
            break;
        }
    }
    assert!(closing.active_script_effect.is_none());
    for _ in 0..2 {
        closing.publish_npc_frame(false);
    }
    assert_eq!(
        closing
            .npc_presentation
            .pose(MapId::PewterPokecenter, slot)
            .unwrap()
            .image
            .facing,
        closing.npc_states[slot].facing
    );
    // Existing map/script replacement drops this effect; no separate global
    // override or mutated logical NPC needs repair on the replacement map.
    field.load_map_script(MapId::PewterPokecenter);
    assert!(field.active_script_effect.is_none());
    for _ in 0..2 {
        field.publish_npc_frame(false);
    }
    assert_eq!(
        field
            .npc_presentation
            .pose(MapId::PewterPokecenter, slot)
            .unwrap()
            .image
            .facing,
        field.npc_states[slot].facing
    );
    let fresh = Field::new(MapId::PewterPokecenter, None, PokemonRedData);
    assert!(fresh.active_script_effect.is_none());
    assert!(!fresh.jigglypuff_song_playing);
}
