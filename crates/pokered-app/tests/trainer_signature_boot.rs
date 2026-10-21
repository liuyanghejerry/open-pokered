//! SRAM is prepared offline; after construction only physical input mutates the game.
//! Observe the real map-script -> trainer-battle roster, before any move is selected.
use pokered_app::PokemonGame;
use pokered_core::{
    game_state::GameScreen,
    pokemon::stats::create_pokemon,
    save::{game_data::MapPosition, sram_export::export_sram, SaveData},
};
use pokered_data::{maps::MapId, species::Species, wild_data::GameVersion};
use pokered_renderer::input::{GbButton, InputState};
use serde_json::{json, Value};

#[test]
fn normal_continue_and_map_dialogue_install_original_trainer_moves() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(replay)
        .unwrap()
        .join()
        .unwrap();
}

fn replay() {
    let reference: Value = serde_json::from_str(include_str!(
        "../../pokered-core/tests/fixtures/original_trainer_signature_moves.json"
    ))
    .unwrap();
    let cases = [
        (MapId::PewterGym, 4, 2, Species::Bulbasaur, "Brock"),
        (MapId::CeruleanGym, 4, 1, Species::Bulbasaur, "Misty"),
        (MapId::VermilionGym, 5, 2, Species::Bulbasaur, "LtSurge"),
        (MapId::CeladonGym, 4, 4, Species::Bulbasaur, "Erika"),
        (MapId::FuchsiaGym, 4, 11, Species::Bulbasaur, "Koga"),
        (MapId::SaffronGym, 9, 9, Species::Bulbasaur, "Sabrina"),
        (MapId::CinnabarGym, 3, 4, Species::Bulbasaur, "Blaine"),
        (MapId::ViridianGym, 2, 2, Species::Bulbasaur, "Giovanni"),
        (
            MapId::RocketHideoutB4F,
            25,
            4,
            Species::Bulbasaur,
            "HideoutGiovanni",
        ),
        (
            MapId::SilphCo11F,
            6,
            10,
            Species::Bulbasaur,
            "SilphGiovanni",
        ),
        (MapId::ChampionsRoom, 3, 7, Species::Bulbasaur, "Charizard"),
        (MapId::ChampionsRoom, 3, 7, Species::Charmander, "Blastoise"),
        (MapId::ChampionsRoom, 3, 7, Species::Squirtle, "Venusaur"),
    ];
    let root = std::env::temp_dir().join(format!("trainer-signature-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut failures = Vec::new();
    for (map, x, y, starter, name) in cases {
        let mut save = SaveData::new();
        save.player_name = vec![0x91, 0x84, 0x83, 0x50];
        save.game_data.rival_name = vec![0x81, 0x8b, 0x94, 0x84, 0x50];
        save.game_data.player_starter = starter as u8;
        save.game_data.position = MapPosition {
            map_id: map as u8,
            x,
            y,
            x_block: x % 2,
            y_block: y % 2,
        };
        save.game_data.player_direction = 4;
        save.party
            .add(create_pokemon(starter, 50, [0xff; 2]).unwrap())
            .unwrap();
        let path = root.join(format!("{name}.sav"));
        std::fs::write(&path, export_sram(&save)).unwrap();
        #[cfg(feature = "debug-server")]
        let mut game = PokemonGame::new_with_options(
            GameVersion::Red,
            Some(path.clone()),
            None,
            None,
            false,
            None,
            false,
            true,
            None,
        );
        #[cfg(not(feature = "debug-server"))]
        let mut game = PokemonGame::new_with_options(
            GameVersion::Red,
            Some(path.clone()),
            None,
            None,
            false,
            None,
            false,
            true,
        );
        let mut input = InputState::new();
        let mut entered_overworld = false;
        let mut battle_frame = None;
        let mut overworld_frames = 0;
        for frame in 0..16000 {
            if game.state.screen == GameScreen::Overworld {
                if !entered_overworld {
                    assert_eq!(
                        (game.overworld.state.player.x, game.overworld.state.player.y),
                        (u16::from(x), u16::from(y)),
                        "{name}: SRAM position"
                    );
                }
                entered_overworld = true;
                overworld_frames += 1;
            }
            if game.state.screen == GameScreen::Battle && game.battle.battle_state.is_some() {
                battle_frame = Some(frame);
                break;
            }
            input.begin_frame();
            input.release(GbButton::A);
            input.release(GbButton::Up);
            input.release(GbButton::Down);
            // Continue intentionally faces Down, matching the original. Turn
            // toward the adjacent boss using actual movement input.
            if map != MapId::ChampionsRoom && (64..67).contains(&overworld_frames) {
                input.press(if map == MapId::CeruleanGym {
                    GbButton::Down
                } else {
                    GbButton::Up
                });
            } else if frame % 64 < 3 {
                input.press(GbButton::A);
            }
            game.update(&input);
        }
        assert!(
            entered_overworld,
            "{name}: CONTINUE did not reach overworld; {:?}",
            game.state.screen
        );
        assert!(
            battle_frame.is_some(),
            "{name}: physical dialogue did not enter battle; {:?}",
            game.state.screen
        );
        let expected_class = if map == MapId::ChampionsRoom {
            "Rival3"
        } else if name.ends_with("Giovanni") {
            "Giovanni"
        } else {
            name
        };
        assert_eq!(
            format!("{:?}", game.battle.trainer_class.unwrap()),
            expected_class,
            "{name}: entered another trainer battle"
        );
        let mons: Vec<_> = game
            .battle
            .battle_state
            .as_ref()
            .unwrap()
            .enemy
            .party
            .iter()
            .map(|mon| json!({"species":mon.species,"moves":mon.moves}))
            .collect();
        println!(
            "SIGNATURE_BOOT {}",
            json!({"case": name, "map":format!("{map:?}"), "battle_frame":battle_frame, "party":mons})
        );
        if map == MapId::ChampionsRoom {
            let row = reference["champion_moves"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["species"] == name)
                .unwrap();
            if mons.len() != 6
                || mons[5]["species"] != row["species"]
                || mons[5]["moves"][2] != row["move"]
                || mons[0]["moves"][2] != "SkyAttack"
            {
                failures.push(name);
            }
        } else if name == "HideoutGiovanni" || name == "SilphGiovanni" {
            if mons.iter().any(|m| {
                m["moves"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|mv| mv == "Fissure")
            }) {
                failures.push(name);
            }
        } else {
            let row = reference["lone_moves"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["class"] == name)
                .unwrap();
            let target = row["target"].as_u64().unwrap() as usize;
            if mons
                .get(target)
                .is_none_or(|m| m["species"] != row["species"] || m["moves"][2] != row["move"])
            {
                failures.push(name);
            }
        }
        drop(game);
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir_all(root).unwrap();
    assert!(
        failures.is_empty(),
        "original signature mismatch: {failures:?}"
    );
}
