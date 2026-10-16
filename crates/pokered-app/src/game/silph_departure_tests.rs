//! Exercise the production live-session -> physical SRAM writer at the
//! departure's publication boundaries, without synthesizing a second writer.
use super::*;
use pokered_core::overworld::OverworldInput;
use pokered_data::toggleable_objects::{is_object_hidden, toggle_id_to_bit_index};

#[test]
fn silph_departure_game_save_writer_preserves_black_batch_and_delays_the_beat_flag() {
    let mut screen = OverworldScreen::new(MapId::SilphCo11F, None, PokemonRedData);
    screen.state.player.x = 6;
    screen.state.player.y = 14;
    screen.reload_scene_source("SilphCo11F", "game_scene SilphCo11F {\n  @load {\n    silphRocketDeparture()\n    setFlag(\"EVENT_BEAT_SILPH_CO_GIOVANNI\")\n  }\n}").unwrap();
    screen.run_on_load();
    let golden: serde_json::Value = serde_json::from_str(include_str!(
        "../../../pokered-data/src/silph_liberation_golden.json"
    ))
    .unwrap();
    let mut hides: Vec<String> = [1, 2, 3, 4, 5, 6, 7, 14, 15]
        .into_iter()
        .map(|id| format!("SAFFRON_CITY_OBJ_{id}"))
        .collect();
    hides.extend(
        golden["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|case| case["toggle"].as_str().unwrap().to_owned()),
    );
    for elapsed in 0..=67 {
        if [31, 32, 59, 66, 67].contains(&elapsed) {
            let mut save = SaveData::new();
            PokemonGame::apply_live_state_to_save(&mut save, &screen, "RED", "BLUE");
            let imported = pokered_core::save::sram_import::import_sram(
                &pokered_core::save::sram_export::export_sram(&save),
            )
            .unwrap();
            assert_eq!(
                imported.game_data.toggleable_object_flags,
                *screen.toggleable_object_flags()
            );
            let mut restored = OverworldScreen::new(MapId::SilphCo11F, None, PokemonRedData);
            restored.restore_loaded_save_flags(&imported, None);
            restored.set_toggleable_object_flags(imported.game_data.toggleable_object_flags);
            restored.apply_hidden_object_flags();
            assert_eq!(
                restored
                    .unified_flags()
                    .get_flag("EVENT_BEAT_SILPH_CO_GIOVANNI"),
                elapsed == 67
            );
            for id in &hides {
                let bit = toggle_id_to_bit_index(id).unwrap();
                assert_eq!(
                    is_object_hidden(restored.toggleable_object_flags(), bit),
                    elapsed >= 32,
                    "wait {elapsed} {id}"
                );
            }
            for id in 8..=13 {
                let bit = toggle_id_to_bit_index(&format!("SAFFRON_CITY_OBJ_{id}")).unwrap();
                assert_eq!(
                    is_object_hidden(restored.toggleable_object_flags(), bit),
                    elapsed < 32,
                    "citizen wait {elapsed}"
                );
            }
            for text_id in [3, 4, 5] {
                let live = restored
                    .npc_states
                    .iter()
                    .find(|npc| npc.text_id == text_id)
                    .unwrap();
                assert_eq!(
                    live.visible,
                    elapsed < 32,
                    "live object after actual SRAM import"
                );
            }
        }
        if elapsed < 67 {
            screen.update_frame(OverworldInput::new(
                false, false, false, false, false, false, false, false,
            ));
        }
    }
}
