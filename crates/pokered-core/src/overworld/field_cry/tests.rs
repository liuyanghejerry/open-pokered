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

/// These are component fixtures at the actual map's live NPC position, not
/// physical-SAV traversal evidence. The embedded map bindings and ScriptVM
/// must produce FieldCry; the test never installs a synthetic effect/script.
fn normal_far_pet_talk(map: MapId, npc_text_id: u8) -> Field {
    let mut field = Field::new(map, None, PokemonRedData);
    field.set_text_delay_frames(3);
    if map == MapId::SaffronCity {
        // Real liberation precondition: use the map's @load show/hide writers,
        // not a forced visible NPC or a bypass of the hidden/toggle trigger.
        field.set_flag_live("EVENT_BEAT_SILPH_CO_GIOVANNI", true);
        field.run_on_load();
    }
    for _ in 0..512 {
        step(&mut field, 0);
        let npc = field
            .npc_states
            .iter()
            .find(|npc| npc.text_id == npc_text_id)
            .expect("the production map must contain the audited NPC");
        if field.active_script_effect.is_none()
            && npc.visible
            && npc.walk_counter == 0
            && field
                .foreground_observation()
                .is_some_and(|owner| owner.last().is_some())
        {
            break;
        }
    }
    let npc = field
        .npc_states
        .iter()
        .find(|npc| npc.text_id == npc_text_id)
        .unwrap();
    assert!(
        npc.visible && npc.walk_counter == 0,
        "map precondition must make the real pet available"
    );
    assert!(
        field
            .foreground_observation()
            .is_some_and(|owner| owner.last().is_some()),
        "the ordinary field must establish actual neutral input history"
    );
    field.state.player.x = npc.x;
    field.state.player.y = npc.y + 1;
    field.state.player.facing = Direction::Up;
    for _ in 0..8 {
        far_pet_step(&mut field, A);
        if phase(&field).is_some() {
            break;
        }
    }
    assert!(
        phase(&field).is_some(),
        "ordinary A must enter the embedded pet consumer: {map:?} npc{npc_text_id}"
    );
    field
}

fn far_pet_step(field: &mut Field, buttons: u8) -> Vec<String> {
    field.update_frame(ForegroundJoypad::as_field_input(buttons));
    assert_eq!(field.source_foreground_error(), None);
    field
        .audio_requests
        .iter()
        .filter_map(|request| match request {
            OverworldAudioRequest::PlayCry { species } => Some(species.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn audited_far_pet_npc_consumers_print_then_cry_once_and_wait_before_outer_ack() {
    let cases = [
        (MapId::CeladonCity, 7, "POLIWRATH"),
        (MapId::CeladonMansion1F, 1, "MEOWTH"),
        (MapId::CeladonMansion1F, 3, "CLEFAIRY"),
        (MapId::CeladonMansion1F, 4, "NIDORAN_F"),
        (MapId::CopycatsHouse1F, 3, "CHANSEY"),
        (MapId::LavenderCuboneHouse, 1, "CUBONE"),
        (MapId::MrFujisHouse, 3, "PSYDUCK"),
        (MapId::MrFujisHouse, 4, "NIDORINO"),
        (MapId::PewterNidoranHouse, 1, "NIDORAN_M"),
        (MapId::SSAnne1FRooms, 8, "WIGGLYTUFF"),
        (MapId::SSAnneB1FRooms, 8, "MACHOKE"),
        (MapId::SaffronCity, 12, "PIDGEOT"),
        (MapId::SaffronPidgeyHouse, 2, "PIDGEY"),
    ];
    for (map, npc, species) in cases {
        let mut field = normal_far_pet_talk(map, npc);
        let Some(ScriptEffect::FieldCry { state }) = &field.active_script_effect else {
            panic!("{map:?} npc{npc} did not dispatch the real typed consumer");
        };
        assert_eq!(state.species, species, "{map:?} npc{npc}");
        let mut cries = Vec::new();
        for _ in 0..2000 {
            if phase(&field) == Some(Phase::SoundWait) {
                break;
            }
            cries.extend(far_pet_step(&mut field, 0));
        }
        assert_eq!(
            phase(&field),
            Some(Phase::SoundWait),
            "{map:?} npc{npc} requires no final text ACK before cry"
        );
        assert_eq!(
            cries.len(),
            1,
            "{map:?} npc{npc} must request its cry exactly once"
        );
        assert_eq!(
            cries[0], species,
            "{map:?} npc{npc} must request the right species"
        );
        assert!(field.pending_dialogue.as_ref().unwrap().waiting_for_input());
        assert!(field.retain_field_text);
        let cursor = serde_json::to_value(field.pending_dialogue.as_ref().unwrap()).unwrap();
        let last = field.source_foreground().unwrap().last();
        field.script_sfx_playing = true; // Component sequencer input, not a guessed cry duration.
        for buttons in [A, B, A | B, 0] {
            assert!(far_pet_step(&mut field, buttons).is_empty());
            assert_eq!(phase(&field), Some(Phase::SoundWait));
            assert_eq!(field.source_foreground().unwrap().last(), last);
            assert_eq!(
                serde_json::to_value(field.pending_dialogue.as_ref().unwrap()).unwrap(),
                cursor
            );
        }
        field.script_sfx_playing = false;
        assert!(far_pet_step(&mut field, 0).is_empty());
        assert_eq!(phase(&field), Some(Phase::OuterAck));
        assert!(field.pending_dialogue.is_some() && field.retain_field_text);
        assert!(far_pet_step(&mut field, B).is_empty());
        assert_eq!(phase(&field), Some(Phase::CloseWait));
        assert!(field.displayed_field_dialogue().is_none());
        assert!(far_pet_step(&mut field, 0).is_empty());
        assert!(phase(&field).is_none());
    }
}
