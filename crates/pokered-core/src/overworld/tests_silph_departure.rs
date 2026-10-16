//! Host/caller contracts against fbcf7d SilphCo11F.asm and home/fade.asm.
use super::screen::OverworldScreen;
use super::{script_bridge::ScriptEffect, OverworldInput};
use crate::alloc_prelude::*;
use crate::snapshot::OverworldSnapshot;
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;
use pokered_data::toggleable_objects::{
    is_object_hidden, set_object_hidden, set_object_shown, toggle_id_to_bit_index,
};

fn neutral() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

fn original_roster() -> (Vec<String>, Vec<String>) {
    let golden: serde_json::Value = serde_json::from_str(include_str!(
        "../../../pokered-data/src/silph_liberation_golden.json"
    ))
    .unwrap();
    let mut hidden: Vec<String> = [1, 2, 3, 4, 5, 6, 7, 14, 15]
        .into_iter()
        .map(|id| format!("SAFFRON_CITY_OBJ_{id}"))
        .collect();
    hidden.extend(
        golden["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|case| case["toggle"].as_str().unwrap().to_string()),
    );
    (
        hidden,
        (8..=13)
            .map(|id| format!("SAFFRON_CITY_OBJ_{id}"))
            .collect(),
    )
}

fn start() -> OverworldScreen<PokemonRedData> {
    let mut screen = OverworldScreen::new(MapId::SilphCo11F, None, PokemonRedData);
    screen.state.player.x = 6;
    screen.state.player.y = 14;
    // A small actual DSL caller isolates the complete local effect from the
    // preceding battle/dialogue. Real embedded win/lose paths have separate tests.
    screen.reload_scene_source("SilphCo11F", "game_scene SilphCo11F {\n  @load {\n    silphRocketDeparture()\n    setFlag(\"AFTER_DEPARTURE\")\n  }\n}").unwrap();
    let (hidden, shown) = original_roster();
    for id in &hidden {
        screen.set_flag_live(&format!("__OBJ_HIDDEN_{id}"), false);
        screen.set_flag_live(&format!("__OBJ_SHOWN_{id}"), true);
        set_object_shown(
            &mut screen.toggleable_object_flags,
            toggle_id_to_bit_index(id).unwrap(),
        );
    }
    for id in &shown {
        screen.set_flag_live(&format!("__OBJ_HIDDEN_{id}"), true);
        screen.set_flag_live(&format!("__OBJ_SHOWN_{id}"), false);
        set_object_hidden(
            &mut screen.toggleable_object_flags,
            toggle_id_to_bit_index(id).unwrap(),
        );
    }
    screen.run_on_load();
    assert!(matches!(
        screen.active_script_effect,
        Some(ScriptEffect::SilphRocketDeparture { .. })
    ));
    screen
}

fn assert_roster(screen: &OverworldScreen<PokemonRedData>, applied: bool) {
    let (hidden, shown) = original_roster();
    let flags = screen.script_flags();
    for (ids, hidden_after) in [(&hidden, true), (&shown, false)] {
        for id in ids {
            let expected = if applied { hidden_after } else { !hidden_after };
            assert_eq!(
                is_object_hidden(
                    &screen.toggleable_object_flags,
                    toggle_id_to_bit_index(id).unwrap()
                ),
                expected,
                "SRAM bit {id}"
            );
            assert_eq!(
                flags.get(&format!("__OBJ_HIDDEN_{id}")).copied(),
                Some(expected),
                "hidden flag {id}"
            );
            assert_eq!(
                flags.get(&format!("__OBJ_SHOWN_{id}")).copied(),
                Some(!expected),
                "shown flag {id}"
            );
        }
    }
    for npc_id in [3, 4, 5] {
        assert_eq!(
            screen
                .npc_states
                .iter()
                .find(|npc| npc.text_id == npc_id)
                .unwrap()
                .visible,
            !applied
        );
    }
}

#[test]
fn silph_departure_host_waits_32_batch_3_32_and_normal_palette_does_not_resume_early() {
    let mut screen = start();
    // Literal original boundaries: the final normal stage starts at 59 but
    // still waits eight successor VBlanks. No render/query consumes a wait.
    for elapsed in 0..=67 {
        let expected = match elapsed {
            0..=7 => Some(3),
            8..=15 => Some(2),
            16..=23 => Some(1),
            24..=42 => Some(0),
            43..=50 => Some(1),
            51..=58 => Some(2),
            59..=66 => Some(3),
            _ => None,
        };
        for _ in 0..2 {
            assert_eq!(
                screen.silph_rocket_departure_palette_index(),
                expected,
                "wait {elapsed}"
            );
        }
        assert_roster(&screen, elapsed >= 32);
        assert_eq!(
            screen
                .script_flags()
                .get("AFTER_DEPARTURE")
                .copied()
                .unwrap_or(false),
            elapsed == 67
        );
        assert_eq!(screen.script_engine_idle(), elapsed == 67);
        if elapsed < 67 {
            screen.update_frame(neutral());
        }
    }
    let mut save = crate::save::SaveData::new();
    save.game_data.toggleable_object_flags = screen.toggleable_object_flags;
    let bytes = crate::save::sram_export::export_sram(&save);
    let imported = crate::save::sram_import::import_sram(&bytes).unwrap();
    assert_eq!(
        imported.game_data.toggleable_object_flags,
        screen.toggleable_object_flags
    );
}

#[cfg(not(feature = "script-boa"))]
#[test]
fn silph_departure_native_vm_and_effect_round_trip_at_every_wait_without_reapplying_batch() {
    let mut uninterrupted = start();
    for elapsed in 0..67 {
        // Snapshot includes native VM, effect phase, flags, live actors and SRAM
        // bits. Restoring before/after the atomic batch must not replay it.
        let encoded = serde_json::to_vec(&OverworldSnapshot::capture(&uninterrupted)).unwrap();
        let saved: OverworldSnapshot = serde_json::from_slice(&encoded).unwrap();
        let mut resumed = start();
        saved.restore_into(&mut resumed);
        assert_eq!(
            resumed.active_script_effect_value(),
            uninterrupted.active_script_effect_value()
        );
        uninterrupted.update_frame(neutral());
        resumed.update_frame(neutral());
        assert_eq!(
            resumed.active_script_effect_value(),
            uninterrupted.active_script_effect_value(),
            "wait {elapsed}"
        );
        assert_eq!(
            resumed.script_flags(),
            uninterrupted.script_flags(),
            "VM wait {elapsed}"
        );
        assert_eq!(
            resumed.toggleable_object_flags,
            uninterrupted.toggleable_object_flags
        );
        assert_roster(&resumed, elapsed + 1 >= 32);
    }
}
