use crate::alloc_prelude::*;
use crate::overworld::screen::OverworldGameDataRequest;
use crate::overworld::{OverworldInput, OverworldScreen};
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
type Field = OverworldScreen<PokemonRedData>;
fn keys(up: bool, down: bool, a: bool, b: bool) -> OverworldInput {
    OverworldInput::new(up, down, false, false, a, b, false, false)
}
fn menu(count: usize) -> Field {
    let mut field = Field::new(MapId::PalletTown, None, PokemonRedData);
    let options = (0..count)
        .map(|n| format!("@option(\"ENTRY{n}\") {{giveMoney({})}}", 10 + n))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("game_scene PalletTown {{@load {{@choice {{{options}}}}}}}");
    field
        .reload_scene_with_config(
            "PalletTown",
            &source,
            Some(r#"{"onLoad":"PalletTownOnLoad"}"#),
        )
        .unwrap();
    field.run_on_load();
    for _ in 0..10 {
        field.update_frame(keys(false, false, false, false));
        if field.pending_choice.is_some() {
            return field;
        }
    }
    panic!("real native scene did not open its choice");
}
fn pulse(field: &mut Field, up: bool, down: bool) {
    field.update_frame(keys(up, down, false, false));
    field.update_frame(keys(false, false, false, false));
}
#[test]
fn field_choice_endpoints_clamp_for_real_native_scene_continuations() {
    for count in [1, 2, 3] {
        let mut field = menu(count);
        pulse(&mut field, true, false);
        assert_eq!(field.pending_choice.as_ref().unwrap().selected, 0);
        for _ in 0..count + 2 {
            pulse(&mut field, false, true);
        }
        assert_eq!(
            field.pending_choice.as_ref().unwrap().selected,
            (count - 1) as u32
        );
        for _ in 0..count + 2 {
            pulse(&mut field, true, false);
        }
        assert_eq!(field.pending_choice.as_ref().unwrap().selected, 0);
    }
}
#[test]
fn simultaneous_ab_runs_only_the_cancel_scene_branch() {
    for count in [2, 3] {
        let mut field = menu(count);
        field.update_frame(keys(false, false, true, true));
        let mut receipts = Vec::new();
        for _ in 0..32 {
            field.update_frame(keys(false, false, false, false));
            receipts.extend(field.game_data_requests.drain(..).filter_map(
                |request| match request {
                    OverworldGameDataRequest::GiveMoney { amount } => Some(amount),
                    _ => None,
                },
            ));
        }
        assert_eq!(receipts, vec![10 + (count - 1) as u32]);
        assert!(field.pending_choice.is_none());
        assert!(field.script_engine.is_idle());
    }
}
