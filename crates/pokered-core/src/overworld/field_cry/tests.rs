//! Source-caller component controls. These do not substitute for actual App
//! PCM or ordinary physical-SRAM before/after input recordings.
use super::*;
use crate::foreground_input::{ForegroundJoypad, LEFT, SELECT, START, UP};
use crate::overworld::script_bridge::ScriptEffect;
use crate::overworld::Direction;
use crate::snapshot::OverworldSnapshot;
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
type Field = OverworldScreen<PokemonRedData>;

fn phase(field: &Field) -> Option<Phase> {
    match &field.active_script_effect {
        Some(ScriptEffect::FieldCry { state }) => Some(state.phase),
        _ => None,
    }
}
fn step(field: &mut Field, buttons: u8) -> usize {
    field.update_frame(ForegroundJoypad::as_field_input(buttons));
    assert_eq!(field.source_foreground_error(), None);
    field.audio_requests.iter().filter(|request| {
        matches!(request, OverworldAudioRequest::PlayCry { species } if species == "PIDGEY")
    }).count()
}
fn normal_talk() -> Field {
    let mut field = Field::new(MapId::VermilionPidgeyHouse, None, PokemonRedData);
    // Unit preboot position only. The real Bird remains Wander/range2; this
    // is not a native playthrough or a frozen-NPC physical-SAV witness.
    field.state.player.x = 3;
    field.state.player.y = 6;
    field.state.player.facing = Direction::Up;
    field.set_text_delay_frames(3);
    for _ in 0..2 {
        step(&mut field, 0);
    }
    for _ in 0..8 {
        step(&mut field, A);
        if phase(&field).is_some() {
            break;
        }
    }
    assert!(
        phase(&field).is_some(),
        "ordinary npc2 A must reach the embedded fieldCry caller"
    );
    field
}
fn to_sound_wait(field: &mut Field, buttons: u8) {
    let mut cries = 0;
    for _ in 0..2000 {
        if phase(field) == Some(Phase::SoundWait) {
            assert_eq!(
                cries, 1,
                "literal @ prints once and starts one cry without a final ACK"
            );
            assert!(field.pending_dialogue.as_ref().unwrap().waiting_for_input());
            assert!(field.retain_field_text);
            return;
        }
        cries += step(field, buttons);
    }
    panic!("actual embedded Pidgey caller did not reach sound wait");
}
fn roundtrip(field: &mut Field) {
    let before = serde_json::to_value(OverworldSnapshot::capture(field)).unwrap();
    let snapshot: OverworldSnapshot = serde_json::from_value(before.clone()).unwrap();
    snapshot.restore_into(field);
    assert_eq!(
        serde_json::to_value(OverworldSnapshot::capture(field)).unwrap(),
        before
    );
}

#[test]
fn far_entry_keeps_background_three_frames_and_omits_inner_print_delay() {
    let mut dialogue = BedroomDialogue::from_message("x");
    dialogue.begin_far_field_display();
    let mut owner = ForegroundJoypad::default();
    owner.read_raw(0);
    owner.poll();
    owner.read_raw(A); // Any accidental preparation poll would now change last.
                       // CopyScreenTileBuffer3 + LoadFont128/8+1 =20 source VBlanks.
    for frame in 1..20 {
        assert_eq!(dialogue.get_display_text().is_none(), frame <= 3);
        dialogue.reveal_next_char_with_source(false, &mut owner);
        assert_eq!(dialogue.char_index(), 0);
        assert_eq!(owner.last(), Some(0));
    }
    dialogue.reveal_next_char_with_source(false, &mut owner);
    assert_eq!(
        dialogue.char_index(),
        1,
        "far entry has no additional inner PrintText3"
    );
    assert_eq!(
        owner.last(),
        Some(A),
        "the first glyph performs its real poll"
    );
}

#[test]
fn embedded_pidgey_keeps_same_window_and_last_poll_until_actual_sound_completion() {
    let mut field = normal_talk();
    to_sound_wait(&mut field, 0);
    // PlayCry has been queued while the pre-dispatch sample is still false;
    // the same update must yield SoundWait instead of consuming that old false.
    assert!(!field.script_sfx_playing);
    assert_eq!(phase(&field), Some(Phase::SoundWait));
    let cursor = serde_json::to_value(field.pending_dialogue.as_ref().unwrap()).unwrap();
    let last = field.source_foreground().unwrap().last();
    let position = (field.state.player.x, field.state.player.y);
    field.script_sfx_playing = true; // Component's actual-sequencer input, not a duration.
    for buttons in [A, B, LEFT | SELECT | START | UP, 0]
        .into_iter()
        .cycle()
        .take(24)
    {
        assert_eq!(step(&mut field, buttons), 0, "no repeated cry");
        assert_eq!(phase(&field), Some(Phase::SoundWait));
        assert_eq!(
            serde_json::to_value(field.pending_dialogue.as_ref().unwrap()).unwrap(),
            cursor
        );
        assert_eq!(field.source_foreground().unwrap().last(), last);
        assert_eq!((field.state.player.x, field.state.player.y), position);
        assert_eq!(
            field.sfx_event,
            super::super::screen::OverworldSfxEvent::None
        );
    }
    field.script_sfx_playing = false;
    assert_eq!(step(&mut field, 0), 0);
    assert_eq!(phase(&field), Some(Phase::OuterAck));
    assert!(field.pending_dialogue.is_some() && field.retain_field_text);
}

#[test]
fn newly_held_a_during_cry_is_fresh_outer_ack_then_waits_for_a_release() {
    let mut field = normal_talk();
    to_sound_wait(&mut field, 0);
    assert_eq!(field.source_foreground().unwrap().last(), Some(0));
    field.script_sfx_playing = true;
    for _ in 0..12 {
        assert_eq!(step(&mut field, A), 0);
    }
    assert_eq!(field.source_foreground().unwrap().last(), Some(0));
    field.script_sfx_playing = false;
    step(&mut field, A);
    assert!(field.pending_dialogue.as_ref().unwrap().holding_open());
    assert_eq!(
        field.sfx_event,
        super::super::screen::OverworldSfxEvent::None,
        "outer ACK is silent"
    );
    for _ in 0..4 {
        step(&mut field, A | B);
        assert!(field.pending_dialogue.is_some(), "B cannot release held A");
    }
    step(&mut field, B);
    assert_eq!(phase(&field), Some(Phase::CloseWait));
    assert!(field.displayed_field_dialogue().is_none());
    step(&mut field, B);
    assert!(phase(&field).is_none());
}

#[test]
fn a_already_polled_while_typing_cannot_be_a_fresh_ack_after_cry() {
    let mut field = normal_talk();
    to_sound_wait(&mut field, A);
    assert_eq!(field.source_foreground().unwrap().last(), Some(A));
    field.script_sfx_playing = true;
    for _ in 0..8 {
        step(&mut field, A);
    }
    field.script_sfx_playing = false;
    for _ in 0..4 {
        step(&mut field, A);
        assert_eq!(phase(&field), Some(Phase::OuterAck));
        assert!(!field.pending_dialogue.as_ref().unwrap().holding_open());
    }
    step(&mut field, 0);
    step(&mut field, B);
    assert_eq!(
        phase(&field),
        Some(Phase::CloseWait),
        "fresh B ACK closes without an A hold"
    );
}

#[test]
fn snapshot_preserves_print_sound_outer_ack_and_a_hold_without_recry() {
    let mut field = normal_talk();
    for _ in 0..6 {
        step(&mut field, 0);
    }
    roundtrip(&mut field); // Printing, including the far entry preparation.
    to_sound_wait(&mut field, 0);
    field.script_sfx_playing = true;
    roundtrip(&mut field);
    for _ in 0..3 {
        assert_eq!(step(&mut field, 0), 0);
    }
    field.script_sfx_playing = false;
    step(&mut field, 0);
    assert_eq!(phase(&field), Some(Phase::OuterAck));
    roundtrip(&mut field);
    step(&mut field, A);
    assert!(field.pending_dialogue.as_ref().unwrap().holding_open());
    roundtrip(&mut field);
    assert_eq!(step(&mut field, A), 0);
    assert!(field.pending_dialogue.as_ref().unwrap().holding_open());
    assert_eq!(step(&mut field, 0), 0);
    assert_eq!(phase(&field), Some(Phase::CloseWait));
}

#[test]
fn no_delay_keeps_the_real_talk_poll_and_still_waits_for_cry_then_outer_ack() {
    let mut field = normal_talk();
    field.set_no_text_delay(true); // Unit source-status writer, not native evidence.
    let last = field
        .source_foreground()
        .or(field.foreground_observation())
        .unwrap()
        .last();
    to_sound_wait(&mut field, 0);
    assert_eq!(
        field.source_foreground().unwrap().last(),
        last,
        "NoTextDelay prints without Joypad"
    );
    field.script_sfx_playing = true;
    for _ in 0..4 {
        assert_eq!(step(&mut field, B), 0);
    }
    field.script_sfx_playing = false;
    step(&mut field, B);
    assert_eq!(phase(&field), Some(Phase::CloseWait));
    assert!(
        field.no_text_delay(),
        "closing a pet dialogue is not a writer of status bit6"
    );
}

#[test]
fn legacy_missing_owner_cannot_manufacture_an_ack_after_sound_wait() {
    let mut field = normal_talk();
    to_sound_wait(&mut field, 0);
    let mut legacy = serde_json::to_value(OverworldSnapshot::capture(&field)).unwrap();
    for key in [
        "source_foreground",
        "source_foreground_mode",
        "source_foreground_error",
    ] {
        legacy.as_object_mut().unwrap().remove(key);
    }
    let snapshot: OverworldSnapshot = serde_json::from_value(legacy).unwrap();
    snapshot.restore_into(&mut field);
    field.script_sfx_playing = false;
    field.update_frame(ForegroundJoypad::as_field_input(B));
    assert_eq!(
        field.source_foreground_error(),
        Some(crate::foreground_input::ForegroundInputError::MissingFieldOwner)
    );
    assert_eq!(phase(&field), Some(Phase::OuterAck));
    assert!(field.pending_dialogue.is_some() && field.retain_field_text);
}
