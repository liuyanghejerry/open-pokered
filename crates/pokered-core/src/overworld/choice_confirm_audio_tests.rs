use super::*;
use pokered_data::impl_traits::PokemonRedData;

type Field = OverworldScreen<PokemonRedData>;

fn input(up: bool, down: bool, a: bool, b: bool) -> OverworldInput {
    OverworldInput::new(up, down, false, false, a, b, false, false)
}

pub(super) fn generic_choice() -> Field {
    let mut field = Field::new(MapId::BillsHouse, None, PokemonRedData);
    // Bill now uses the source-owned YesNoChoice flow. Keep this regression
    // on generic ShowChoice, including its result and confirmation audio.
    let scene = dotzuki_engine_dsl::compiler::compile_scene_to_ast(
        r#"game_scene ChoiceAudio {
  @storyline("ask") {
    @speaker("") { @t("Question?", "问题？") }
    @choice {
      @option(@t("YES", "是")) { @speaker("") { @t("Accepted.", "已接受。") } }
      @option(@t("NO", "否")) { @speaker("") { @t("Cancelled.", "已取消。") } }
    }
  }
}"#,
        "ChoiceAudio",
    ).unwrap();
    let mut engine = crate::overworld::native_script::NativeScriptEngine::new();
    engine.load_map("ChoiceAudio", &scene);
    field.script_engine = crate::overworld::native_script::OverworldScriptEngine::Native(engine);
    let command = field
        .script_engine
        .call_function_no_args("ask")
        .unwrap()
        .unwrap();
    field.active_script_effect = Some(script_bridge::dispatch_command(&command));

    // Enter the fixture's choice through its existing text/input flow.
    // Only ordinary field updates reveal pages and acknowledge them.
    for frame in 0..4000 {
        let a = frame % 2 == 0
            && field
                .pending_dialogue
                .as_ref()
                .is_some_and(|dialogue| dialogue.waiting_for_input());
        field.update_frame(input(false, false, a, false));
        if field.pending_choice.is_some() {
            break;
        }
    }
    assert_eq!(
        field.pending_choice.as_ref().unwrap().options,
        ["YES", "NO"]
    );
    field.update_frame(input(false, false, false, false));
    field
}

#[test]
fn choice_idle_and_cursor_movement_do_not_play_confirmation() {
    let mut field = generic_choice();
    for _ in 0..16 {
        field.update_frame(input(false, false, false, false));
        assert_eq!(field.sfx_event, OverworldSfxEvent::None);
        assert_eq!(field.pending_choice.as_ref().unwrap().selected, 0);
    }
    field.update_frame(input(false, true, false, false));
    assert_eq!(field.pending_choice.as_ref().unwrap().selected, 1);
    assert_eq!(field.sfx_event, OverworldSfxEvent::None);
    field.update_frame(input(false, false, false, false));
    field.update_frame(input(true, false, false, false));
    assert_eq!(field.pending_choice.as_ref().unwrap().selected, 0);
    assert_eq!(field.sfx_event, OverworldSfxEvent::None);
}

#[test]
fn choice_a_b_and_combined_confirmation_play_once_with_b_priority() {
    for (a, b, response) in [
        (true, false, "Accepted."),
        (false, true, "Cancelled."),
        // HandleMenuInput callers test B before accepting a selection.
        (true, true, "Cancelled."),
    ] {
        let mut field = generic_choice();
        field.update_frame(input(false, false, a, b));
        assert_eq!(field.sfx_event, OverworldSfxEvent::TextAdvance);
        assert!(field.pending_choice.is_none());
        assert!(matches!(
            &field.active_script_effect,
            Some(script_bridge::ScriptEffect::ShowDialogue { text })
                if text.starts_with(response)
        ));

        // The following text is still driven by the existing physical-key
        // edges: holding the confirmation must not replay a menu sound.
        for _ in 0..60 {
            field.update_frame(input(false, false, a, b));
            assert_eq!(field.sfx_event, OverworldSfxEvent::None);
            assert!(field.pending_choice.is_none());
        }
    }
}

#[test]
fn nested_gift_choice_emits_the_common_confirmation_once() {
    let mut field = Field::new(MapId::BillsHouse, None, PokemonRedData);
    // Exercise the existing GivePokemon owner, including its normal prompt
    // and child-choice construction, rather than injecting a child result.
    field.active_script_effect = Some(script_bridge::ScriptEffect::GivePokemon {
        species: "EEVEE".into(),
        nickname: None,
        level: 5,
        flow: None,
    });
    for frame in 0..4000 {
        let a = frame % 2 == 0
            && field
                .pending_dialogue
                .as_ref()
                .is_some_and(|dialogue| dialogue.waiting_for_input());
        field.update_frame(input(false, false, a, false));
        if field.pending_choice.is_some() {
            break;
        }
    }
    assert!(field.pending_choice.is_some());
    assert!(field.active_script_effect.as_ref().unwrap().is_choice());
    field.update_frame(input(false, false, false, false));
    assert_eq!(field.sfx_event, OverworldSfxEvent::None);
    field.update_frame(input(false, false, false, true));
    assert_eq!(field.sfx_event, OverworldSfxEvent::TextAdvance);
    assert!(field.pending_choice.is_none());
    for _ in 0..16 {
        field.update_frame(input(false, false, false, true));
        assert_eq!(field.sfx_event, OverworldSfxEvent::None);
    }
}

#[test]
fn nested_reading_choice_emits_the_common_confirmation_once() {
    let mut field = Field::new(MapId::BillsHouse, None, PokemonRedData);
    field.active_script_effect = Some(script_bridge::ScriptEffect::ReadingMenu {
        menu: super::super::script_interactions::ReadingMenu::new(
            vec!["HEADER".into(), "CANCEL".into()],
            vec!["The selected page.".into()],
        ),
    });
    field.update_frame(input(false, false, false, false));
    assert!(field.pending_choice.is_some());
    assert_eq!(field.sfx_event, OverworldSfxEvent::None);
    field.update_frame(input(false, false, true, false));
    assert_eq!(field.sfx_event, OverworldSfxEvent::TextAdvance);
    assert!(field.pending_choice.is_none());
    assert!(matches!(
        &field.active_script_effect,
        Some(script_bridge::ScriptEffect::ReadingMenu { menu })
            if matches!(menu.child.as_ref(),
                script_bridge::ScriptEffect::ShowDialogue { text }
                    if text == "The selected page.")
    ));
    for _ in 0..60 {
        field.update_frame(input(false, false, true, false));
        assert_eq!(field.sfx_event, OverworldSfxEvent::None);
    }
}
