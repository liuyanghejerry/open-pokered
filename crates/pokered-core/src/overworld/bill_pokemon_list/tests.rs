use super::*;
use crate::{foreground_input::ForegroundJoypad, snapshot::OverworldSnapshot};
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
type Field = OverworldScreen<PokemonRedData>;
fn step(f: &mut Field, keys: u8) {
    f.update_frame(ForegroundJoypad::as_field_input(keys));
    assert_eq!(f.source_foreground_error(), None);
}
fn phase(f: &Field) -> Phase {
    let Some(ScriptEffect::BillPokemonList { state }) = &f.active_script_effect else {
        panic!("Bill flow missing")
    };
    state.phase
}
fn roundtrip(f: &mut Field) {
    let json = serde_json::to_value(OverworldSnapshot::capture(f)).unwrap();
    let saved: OverworldSnapshot = serde_json::from_value(json.clone()).unwrap();
    saved.restore_into(f);
    assert_eq!(
        serde_json::to_value(OverworldSnapshot::capture(f)).unwrap(),
        json
    );
}
fn menu() -> Field {
    let mut f = Field::new(MapId::BillsHouse, None, PokemonRedData);
    // Component entry fixture supplies the ordinary field sample preceding talk.
    let owner = f.begin_source_field();
    owner.read_raw(A);
    owner.poll();
    f.script_engine.seed_text("playerFacing", "up");
    f.script_engine
        .set_flag("EVENT_LEFT_BILLS_HOUSE_AFTER_HELPING", true);
    let command = f
        .script_engine
        .call_function_no_args("pcMachine")
        .unwrap()
        .unwrap();
    f.active_script_effect = Some(super::super::script_bridge::dispatch_command(&command));
    let mut accepted = false;
    for _ in 0..1000 {
        if f.choice_accepts_input() {
            assert!(accepted);
            assert!(f.no_text_delay());
            assert!(f.displayed_field_dialogue().is_some());
            return f;
        }
        step(&mut f, 0);
        if !accepted
            && f.pending_dialogue
                .as_ref()
                .is_some_and(|d| d.waiting_for_input())
        {
            for _ in 0..5 {
                step(&mut f, 0);
            }
            step(&mut f, A);
            accepted = true;
        }
    }
    panic!("one favorite ACK must reach menu without a question ACK")
}
#[test]
fn actual_scene_has_one_prompt_then_done_question_and_three_unpolled_cursor_frames() {
    let mut f = menu();
    assert_eq!(
        f.pending_choice.as_ref().unwrap().options,
        ["EEVEE", "FLAREON", "JOLTEON", "VAPOREON", "CANCEL"]
    );
    let question = serde_json::to_value(f.displayed_field_dialogue()).unwrap();
    for (key, selected) in [
        (UP, 0),
        (DOWN, 1),
        (DOWN, 2),
        (DOWN, 3),
        (DOWN, 4),
        (DOWN, 4),
        (UP | DOWN, 3),
        (LEFT, 3),
        (START, 3),
    ] {
        step(&mut f, 0);
        step(&mut f, key);
        assert_eq!(phase(&f), Phase::CursorDelay(3));
        assert_eq!(f.pending_choice.as_ref().unwrap().selected, selected);
        let previous = f.foreground_observation().unwrap().last();
        for _ in 0..2 {
            step(&mut f, A | SELECT);
            roundtrip(&mut f);
            assert_eq!(f.foreground_observation().unwrap().last(), previous);
            assert!(!f.choice_accepts_input());
        }
        step(&mut f, 0);
        assert!(f.choice_accepts_input());
        assert_eq!(
            serde_json::to_value(f.displayed_field_dialogue()).unwrap(),
            question
        );
    }
}
#[test]
fn b_wins_over_a_and_outer_close_only_waits_for_a_release() {
    let mut f = menu();
    step(&mut f, 0);
    step(&mut f, A | B);
    assert_eq!(phase(&f), Phase::CancelHold);
    assert!(!f.no_text_delay());
    assert_eq!(f.sfx_event, OverworldSfxEvent::TextAdvance);
    for _ in 0..20 {
        step(&mut f, A);
        roundtrip(&mut f);
        assert_eq!(phase(&f), Phase::CancelHold);
    }
    step(&mut f, B);
    assert_eq!(phase(&f), Phase::CloseWait);
    assert!(f.pending_choice.is_none());
    assert!(f.displayed_field_dialogue().is_none());
    step(&mut f, 0);
    assert!(f.active_script_effect.is_none());
}
#[test]
fn selected_entry_has_six_white_frames_real_cry_gate_and_ten_frame_map_return() {
    let mut f = menu();
    step(&mut f, 0);
    step(&mut f, DOWN | A);
    assert_eq!(f.pending_pokedex_entry.as_ref().unwrap().species, "Flareon");
    assert_eq!(phase(&f), Phase::White(6));
    let last = f.foreground_observation().unwrap().last();
    for n in (1..=5).rev() {
        step(&mut f, START);
        roundtrip(&mut f);
        assert_eq!(phase(&f), Phase::White(n));
        assert_eq!(
            f.pending_pokedex_entry.as_ref().unwrap().presentation,
            PokedexEntryPresentation::White
        );
        assert_eq!(f.foreground_observation().unwrap().last(), last);
    }
    step(&mut f, B);
    assert_eq!(phase(&f), Phase::Cry);
    assert!(f.audio_requests.contains(&OverworldAudioRequest::PlayCry {
        species: "Flareon".into()
    }));
    assert_eq!(
        f.pending_pokedex_entry.as_ref().unwrap().presentation,
        PokedexEntryPresentation::CryHeader
    );
    // Core receives the actual host sequencer's sample; length is deliberately
    // varied here to prove no fixed-duration timer opens input/data.
    f.script_sfx_playing = true;
    for _ in 0..137 {
        step(&mut f, A | B);
        assert_eq!(phase(&f), Phase::Cry);
        assert_eq!(f.foreground_observation().unwrap().last(), last);
    }
    f.script_sfx_playing = false;
    step(&mut f, 0);
    assert_eq!(phase(&f), Phase::Data);
    assert_eq!(
        f.pending_pokedex_entry.as_ref().unwrap().presentation,
        PokedexEntryPresentation::Data
    );
    f.pending_pokedex_entry.as_mut().unwrap().total_pages = 1;
    step(&mut f, B);
    assert_eq!(phase(&f), Phase::ReturnMap(10));
    assert_eq!(f.sfx_event, OverworldSfxEvent::None);
    let last = f.foreground_observation().unwrap().last();
    for n in (1..=9).rev() {
        step(&mut f, A);
        roundtrip(&mut f);
        assert_eq!(phase(&f), Phase::ReturnMap(n));
        assert!(f.pending_pokedex_entry.is_some());
        assert_eq!(f.foreground_observation().unwrap().last(), last);
    }
    step(&mut f, 0);
    assert!(f.pending_pokedex_entry.is_none());
    assert_eq!(phase(&f), Phase::Question);
    for _ in 0..20 {
        step(&mut f, 0);
        if f.choice_accepts_input() {
            break;
        }
    }
    assert!(f.choice_accepts_input());
    assert_eq!(f.pending_choice.as_ref().unwrap().selected, 1);
    assert!(f.displayed_field_dialogue().is_some());
    assert!(f.no_text_delay());
}

#[test]
fn owned_page_turn_clears_body_for_twenty_vblanks_without_polling_and_final_ack_is_silent() {
    let mut f = menu();
    step(&mut f, 0);
    step(&mut f, A);
    f.pending_pokedex_entry.as_mut().unwrap().owned = true;
    for _ in 0..6 {
        step(&mut f, 0);
    }
    assert_eq!(phase(&f), Phase::Cry);
    step(&mut f, 0);
    assert_eq!(phase(&f), Phase::DataDelay(3));
    // Renderer feedback fixture: the owned English entry has two pages.
    f.pending_pokedex_entry.as_mut().unwrap().total_pages = 2;
    for _ in 0..3 {
        step(&mut f, 0);
    }
    step(&mut f, B);
    assert_eq!(phase(&f), Phase::PageClear(20));
    assert_eq!(f.sfx_event, OverworldSfxEvent::TextAdvance);
    let last = f.foreground_observation().unwrap().last();
    for n in (1..=19).rev() {
        step(&mut f, A | B);
        roundtrip(&mut f);
        assert_eq!(phase(&f), Phase::PageClear(n));
        assert_eq!(f.foreground_observation().unwrap().last(), last);
        let entry = f.pending_pokedex_entry.as_ref().unwrap();
        assert_eq!(entry.page, 0);
        assert_eq!(entry.presentation, PokedexEntryPresentation::PageClear);
    }
    step(&mut f, 0);
    assert_eq!(phase(&f), Phase::Data);
    assert_eq!(f.pending_pokedex_entry.as_ref().unwrap().page, 1);
    step(&mut f, A);
    assert_eq!(phase(&f), Phase::ReturnMap(10));
    assert_eq!(f.sfx_event, OverworldSfxEvent::None);
}

#[test]
fn choosing_cancel_has_no_yes_no_fifteen_frame_completion_wait() {
    let mut f = menu();
    for _ in 0..4 {
        step(&mut f, DOWN);
        for _ in 0..3 {
            step(&mut f, 0);
        }
    }
    assert_eq!(f.pending_choice.as_ref().unwrap().selected, 4);
    step(&mut f, A);
    assert_eq!(phase(&f), Phase::CancelHold);
    step(&mut f, 0);
    assert_eq!(phase(&f), Phase::CloseWait);
    step(&mut f, 0);
    assert!(f.active_script_effect.is_none());
}
