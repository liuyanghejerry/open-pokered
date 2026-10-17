//! Real embedded NPC/VM consumer, with explicit component audio samples.
//! These tests are not physical-SAV/native playback or PCM evidence.
use super::*;
use crate::foreground_input::{ForegroundJoypad, LEFT, SELECT, START, UP};
use crate::overworld::Direction;
use crate::snapshot::OverworldSnapshot;
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};

type Field = OverworldScreen<PokemonRedData>;
fn phase(field: &Field) -> Option<Phase> {
    match &field.active_script_effect {
        Some(ScriptEffect::FieldCryContinuation { state }) => Some(state.phase),
        _ => None,
    }
}
fn step(field: &mut Field, buttons: u8) -> usize {
    field.update_frame(ForegroundJoypad::as_field_input(buttons));
    assert_eq!(field.source_foreground_error(), None);
    let cries = field
        .audio_requests
        .iter()
        .filter(|request| {
            matches!(request,
        OverworldAudioRequest::PlayCry { species } if species == "MACHOP")
        })
        .count();
    // Drain requests as App does after this update. Snapshot restore clears
    // transient output queues; resumed state must not dispatch this cry again.
    field.audio_requests.clear();
    cries
}
fn talk() -> Field {
    let mut field = Field::new(MapId::VermilionCity, None, PokemonRedData);
    field.set_text_delay_frames(3);
    for _ in 0..512 {
        step(&mut field, 0);
        let npc = field
            .npc_states
            .iter()
            .find(|npc| npc.text_id == 5)
            .unwrap();
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
        .find(|npc| npc.text_id == 5)
        .unwrap();
    assert!(npc.visible && npc.walk_counter == 0);
    // Component preboot stand beside the actual live Wander NPC. No frozen
    // NPC, replacement scene, direct script call or injected effect.
    field.state.player.x = npc.x;
    field.state.player.y = npc.y + 1;
    field.state.player.facing = Direction::Up;
    for _ in 0..8 {
        step(&mut field, A);
        if phase(&field).is_some() {
            break;
        }
    }
    let Some(ScriptEffect::FieldCryContinuation { state }) = &field.active_script_effect else {
        panic!("ordinary npc5 A must dispatch the actual embedded continuation");
    };
    assert_eq!(state.species, "MACHOP");
    assert_eq!(
        state.texts,
        [
            "MACHOP: Guoh! Gogogoh!",
            "A MACHOP is\nstomping the land",
            "flat."
        ]
    );
    field
}
fn reach(field: &mut Field, target: Phase, buttons: u8) -> usize {
    let mut cries = 0;
    for _ in 0..2000 {
        if phase(field) == Some(target) {
            return cries;
        }
        cries += step(field, buttons);
    }
    panic!(
        "continuation never reached {target:?}; now {:?}",
        phase(field)
    );
}
fn roundtrip(field: &mut Field) {
    let before = serde_json::to_value(OverworldSnapshot::capture(field)).unwrap();
    let snapshot: OverworldSnapshot = serde_json::from_value(before.clone()).unwrap();
    let mut restored = Field::new(field.state.current_map, None, PokemonRedData);
    snapshot.restore_into(&mut restored);
    assert_eq!(
        serde_json::to_value(OverworldSnapshot::capture(&restored)).unwrap(),
        before
    );
    *field = restored;
}
fn display(field: &Field) -> (String, String) {
    field
        .displayed_field_dialogue()
        .unwrap()
        .get_display_text()
        .unwrap()
}

#[test]
fn embedded_machop_far_cry_para_cont_done_keeps_one_display_owner() {
    let mut field = talk();
    let position = (field.state.player.x, field.state.player.y);
    assert_eq!(
        reach(&mut field, Phase::SoundWait, 0),
        1,
        "literal @ returns and starts the cry without first closing/ACKing"
    );
    assert!(field.pending_dialogue.is_some() && field.retain_field_text);
    assert!(
        !field.dialogue_needs_button(),
        "the cry is not an outer ACK"
    );
    assert!(
        !field.dialogue_shows_prompt_arrow(),
        "literal @ has no arrow"
    );
    let cursor = serde_json::to_value(field.pending_dialogue.as_ref().unwrap()).unwrap();
    let last = field.source_foreground().unwrap().last();
    roundtrip(&mut field);
    field.script_sfx_playing = true; // Component sequencer input, not a duration.
    for buttons in [A, B, LEFT | SELECT | START | UP, 0]
        .into_iter()
        .cycle()
        .take(32)
    {
        assert_eq!(step(&mut field, buttons), 0, "no second cry");
        assert_eq!(phase(&field), Some(Phase::SoundWait));
        assert_eq!(field.source_foreground().unwrap().last(), last);
        assert_eq!(
            serde_json::to_value(field.pending_dialogue.as_ref().unwrap()).unwrap(),
            cursor
        );
        assert_eq!((field.state.player.x, field.state.player.y), position);
    }
    field.script_sfx_playing = false;
    step(&mut field, 0);
    assert_eq!(
        phase(&field),
        Some(Phase::Protected {
            remaining: 3,
            next: Prompt::Paragraph
        })
    );
    assert!(
        field.dialogue_shows_prompt_arrow(),
        "PARA writes its arrow before Protected3"
    );
    roundtrip(&mut field);
    // Source text_start/PARA keeps the first message through its protection.
    assert_eq!(
        serde_json::to_value(field.pending_dialogue.as_ref().unwrap()).unwrap(),
        cursor
    );
    for _ in 0..2 {
        step(&mut field, 0);
    }
    assert_eq!(field.source_foreground().unwrap().last(), last);
    step(&mut field, B);
    assert_eq!(phase(&field), Some(Phase::ParagraphClear { remaining: 20 }));
    assert!(
        !field.dialogue_shows_prompt_arrow(),
        "PARA clears its arrow with the text"
    );
    assert_eq!(display(&field), ("".into(), "".into()));
    assert!(
        field.retain_field_text,
        "PARA clears rows, not the enclosing window"
    );
    roundtrip(&mut field);
    let para_last = field.source_foreground().unwrap().last();
    for _ in 0..19 {
        step(&mut field, A | B | START);
        assert_eq!(display(&field), ("".into(), "".into()));
        assert_eq!(
            field.source_foreground().unwrap().last(),
            para_last,
            "Delay20 neither polls nor prints"
        );
    }
    step(&mut field, 0);
    assert_eq!(
        field.pending_dialogue.as_ref().unwrap().char_index(),
        1,
        "twentieth successor returns to NextChar, with no extra PrintText3"
    );
    assert_eq!(
        reach(
            &mut field,
            Phase::Protected {
                remaining: 3,
                next: Prompt::Continue
            },
            0
        ),
        0
    );
    assert_eq!(
        display(&field),
        ("A MACHOP is".into(), "stomping the land".into())
    );
    for _ in 0..3 {
        step(&mut field, 0);
    }
    assert_eq!(phase(&field), Some(Phase::Prompt(Prompt::Continue)));
    assert!(
        field.dialogue_shows_prompt_arrow(),
        "authored CONT has its arrow"
    );
    step(&mut field, B);
    assert_eq!(phase(&field), Some(Phase::ScrollFirst { remaining: 5 }));
    assert!(field.field_text_half_scroll());
    assert!(!field.dialogue_needs_button());
    assert!(
        !field.dialogue_shows_prompt_arrow(),
        "CONT removes the arrow before scrolling"
    );
    roundtrip(&mut field);
    let cont_last = field.source_foreground().unwrap().last();
    for _ in 0..4 {
        step(&mut field, A | B);
        assert!(field.field_text_half_scroll());
        assert_eq!(field.source_foreground().unwrap().last(), cont_last);
    }
    step(&mut field, A | B);
    assert_eq!(phase(&field), Some(Phase::ScrollSecond { remaining: 5 }));
    assert!(!field.field_text_half_scroll());
    assert_eq!(display(&field), ("stomping the land".into(), "".into()));
    roundtrip(&mut field);
    let retained_chars = field.pending_dialogue.as_ref().unwrap().char_index();
    for _ in 0..4 {
        step(&mut field, A | B);
        assert_eq!(
            field.pending_dialogue.as_ref().unwrap().char_index(),
            retained_chars
        );
        assert_eq!(
            field.source_foreground().unwrap().last(),
            cont_last,
            "both Scroll5 calls preserve the same input poll"
        );
    }
    step(&mut field, 0);
    assert_eq!(
        field.pending_dialogue.as_ref().unwrap().char_index(),
        retained_chars + 1
    );
    assert_eq!(reach(&mut field, Phase::OuterAck, 0), 0);
    assert!(
        field.dialogue_needs_button(),
        "DONE still accepts its outer ACK"
    );
    assert!(
        !field.dialogue_shows_prompt_arrow(),
        "DONE never authors another arrow"
    );
    assert_eq!(
        display(&field),
        ("stomping the land".into(), "flat.".into())
    );
    roundtrip(&mut field);
    step(&mut field, A);
    assert!(field.pending_dialogue.as_ref().unwrap().holding_open());
    assert_eq!(
        field.sfx_event,
        super::super::screen::OverworldSfxEvent::None,
        "the sole outer ACK is silent"
    );
    for _ in 0..4 {
        step(&mut field, A | B);
        assert_eq!(phase(&field), Some(Phase::OuterAck));
    }
    step(&mut field, B);
    assert_eq!(phase(&field), Some(Phase::CloseWait));
    assert!(field.displayed_field_dialogue().is_none());
    step(&mut field, 0);
    assert!(phase(&field).is_none());
    assert_eq!((field.state.player.x, field.state.player.y), position);
}

#[test]
fn typing_held_a_is_not_reused_after_sound_or_as_continuation_ack() {
    let mut field = talk();
    assert_eq!(reach(&mut field, Phase::SoundWait, A), 1);
    assert_eq!(field.source_foreground().unwrap().last(), Some(A));
    field.script_sfx_playing = true;
    for _ in 0..8 {
        step(&mut field, A);
    }
    field.script_sfx_playing = false;
    step(&mut field, A);
    for _ in 0..6 {
        step(&mut field, A);
    }
    assert_eq!(
        phase(&field),
        Some(Phase::Prompt(Prompt::Paragraph)),
        "already polled typing A is not a new PARA confirmation"
    );
    assert!(field.pending_dialogue.is_some() && field.retain_field_text);
    assert!(
        !field.pending_dialogue.as_ref().unwrap().holding_open(),
        "the continuation must not accidentally enter fieldCry's outer hold"
    );
    step(&mut field, 0);
    step(&mut field, A);
    assert_eq!(phase(&field), Some(Phase::ParagraphClear { remaining: 20 }));
    let last = field.source_foreground().unwrap().last();
    for _ in 0..19 {
        step(&mut field, A);
        assert_eq!(field.source_foreground().unwrap().last(), last);
    }
    step(&mut field, A);
    assert_eq!(field.pending_dialogue.as_ref().unwrap().char_index(), 1);
    // This is the same real ledger: a long-held A can accelerate letters,
    // but cannot confirm the authored CONT until a subsequent fresh press.
    reach(
        &mut field,
        Phase::Protected {
            remaining: 3,
            next: Prompt::Continue,
        },
        A,
    );
    for _ in 0..6 {
        step(&mut field, A);
    }
    assert_eq!(phase(&field), Some(Phase::Prompt(Prompt::Continue)));
    step(&mut field, 0);
    step(&mut field, B);
    assert_eq!(phase(&field), Some(Phase::ScrollFirst { remaining: 5 }));
}
