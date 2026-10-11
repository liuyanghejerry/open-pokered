//! Original input fixtures343: menu position persists across ordinary callers.
//! Inventory depletion also resets RAM menu memory across equal-count exchanges.
use pokered_app::PokemonGame;
use pokered_core::{
    game_state::{GameScreen, Lang, TextSpeed},
    items::inventory::Inventory,
    start_menu::StartMenuState,
};
use pokered_data::{items::ItemId, species::Species, wild_data::GameVersion};
use pokered_renderer::input::{GbButton, InputState};

fn fixture() -> PokemonGame {
    #[cfg(feature = "debug-server")]
    let mut g = PokemonGame::new_with_options(
        GameVersion::Red,
        None,
        None,
        None,
        false,
        None,
        false,
        true,
        None,
    );
    #[cfg(not(feature = "debug-server"))]
    let mut g =
        PokemonGame::new_with_options(GameVersion::Red, None, None, None, false, None, false, true);
    g.audio = None;
    g.set_seed(42);
    g.state.config.language = Lang::En;
    g.state.config.text_speed = TextSpeed::Slow;
    g.save_data.game_data.bag = Inventory::new_bag();
    g.save_data.game_data.pc_items = Inventory::new_pc();
    g.save_data
        .game_data
        .bag
        .replace_item_slots(&[
            (ItemId::Potion, 4),
            (ItemId::Antidote, 3),
            (ItemId::PokeBall, 2),
            (ItemId::Revive, 2),
            (ItemId::EscapeRope, 1),
            (ItemId::Repel, 2),
            (ItemId::HyperPotion, 3),
            (ItemId::SuperPotion, 4),
        ])
        .unwrap();
    g.save_data.party.clear();
    for (id, level) in [(10, 90), (21, 5), (104, 56), (118, 56), (74, 57), (84, 5)] {
        g.save_data
            .party
            .add(
                pokered_core::pokemon::stats::create_pokemon(
                    Species::from_rom_id(id),
                    level,
                    [0x99; 2],
                )
                .unwrap(),
            )
            .unwrap();
    }
    g.overworld
        .set_event_flag_live(pokered_data::event_flags::EventFlag::EVENT_GOT_POKEDEX);
    g.start_menu = StartMenuState::new(true, true, false);
    g.state.screen = GameScreen::StartMenu;
    g
}
fn drive(g: &mut PokemonGame, schedule: &serde_json::Value, end: usize) {
    let keys: Vec<(usize, GbButton)> = schedule
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r[0].as_u64().unwrap() as usize,
                match r[1].as_str().unwrap() {
                    "a" => GbButton::A,
                    "b" => GbButton::B,
                    "down" => GbButton::Down,
                    "up" => GbButton::Up,
                    "start" => GbButton::Start,
                    "select" => GbButton::Select,
                    _ => unreachable!(),
                },
            )
        })
        .collect();
    let mut input = InputState::new();
    let mut held = None;
    for t in 0..=end {
        input.begin_frame();
        let key = keys
            .iter()
            .find(|(at, _)| (*at..*at + 2).contains(&t))
            .map(|(_, k)| *k);
        if key != held {
            if let Some(k) = held {
                input.release(k);
            }
            if let Some(k) = key {
                input.press(k);
            }
            held = key;
        }
        g.update(&input);
    }
}
#[test]
fn ordinary_bag_reopen_preserves_original_row_and_window() {
    std::thread::Builder::new().stack_size(16*1024*1024).spawn(|| {
        let schedules: serde_json::Value=serde_json::from_str(r#"{"cancel_b":[[70,"down",2],[110,"down",2],[150,"a",2],[230,"down",2],[270,"down",2],[310,"down",2],[350,"down",2],[390,"down",2],[430,"down",2],[470,"down",2],[510,"down",2],[600,"b",2],[700,"a",2]],"cancel_a":[[70,"down",2],[110,"down",2],[150,"a",2],[230,"down",2],[270,"down",2],[310,"down",2],[350,"down",2],[390,"down",2],[430,"down",2],[470,"down",2],[510,"down",2],[600,"a",2],[700,"a",2]],"select_b":[[70,"down",2],[110,"down",2],[150,"a",2],[230,"down",2],[270,"down",2],[310,"down",2],[350,"down",2],[390,"down",2],[430,"down",2],[500,"select",2],[600,"b",2],[700,"a",2]],"full_close":[[70,"down",2],[110,"down",2],[150,"a",2],[230,"down",2],[270,"down",2],[310,"down",2],[350,"down",2],[390,"down",2],[430,"down",2],[600,"b",2],[650,"b",2],[700,"start",2],[850,"a",2]],"party":[[70,"down",2],[110,"down",2],[150,"a",2],[230,"down",2],[270,"down",2],[310,"down",2],[350,"down",2],[390,"down",2],[430,"down",2],[600,"b",2],[650,"up",2],[700,"a",2],[1000,"b",2],[1050,"down",2],[1100,"a",2]],"dex":[[70,"down",2],[110,"down",2],[150,"a",2],[230,"down",2],[270,"down",2],[310,"down",2],[350,"down",2],[390,"down",2],[430,"down",2],[600,"b",2],[650,"up",2],[690,"up",2],[740,"a",2],[1100,"b",2],[1200,"down",2],[1240,"down",2],[1300,"a",2]],"options":[[70,"down",2],[110,"down",2],[150,"a",2],[230,"down",2],[270,"down",2],[310,"down",2],[350,"down",2],[390,"down",2],[430,"down",2],[600,"b",2],[650,"down",2],[690,"down",2],[730,"down",2],[780,"a",2],[1100,"b",2],[1200,"up",2],[1240,"up",2],[1280,"up",2],[1340,"a",2]],"card":[[70,"down",2],[110,"down",2],[150,"a",2],[230,"down",2],[270,"down",2],[310,"down",2],[350,"down",2],[390,"down",2],[430,"down",2],[600,"b",2],[650,"down",2],[700,"a",2],[1100,"b",2],[1200,"up",2],[1240,"a",2]]}"#).unwrap();
        for case in ["cancel_b","cancel_a","select_b","full_close","party","dex","options","card"] {
            let mut g=fixture();let stock=g.save_data.game_data.bag.items();
            drive(&mut g,&schedules[case],1700);
            assert_eq!(g.state.screen,GameScreen::Bag,"{case}");
            let expected=if case.starts_with("cancel_"){(8,6)}else{(6,4)};
            assert_eq!((g.bag_screen.cursor(),g.bag_screen.scroll()),expected,"Source {case}");
            assert_eq!(g.bag_screen.phase(),pokered_core::bag_screen::BagPhase::Browsing,"SELECT/submenu clears: {case}");
            assert_eq!(g.save_data.game_data.bag.items(),stock,"{case}");
        }
    }).unwrap().join().unwrap();
}
#[test]
fn physical_slot_depletion_resets_saved_bag_position_even_after_equal_count_exchange() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let setup = serde_json::json!([
                [70, "down"],
                [110, "down"],
                [150, "a"],
                [230, "down"],
                [270, "down"],
                [310, "down"],
                [350, "down"],
                [390, "down"],
                [430, "down"],
                [600, "b"],
                [650, "b"]
            ]);
            for pc in [false, true] {
                for full in [false, true] {
                    let mut g = fixture();
                    drive(&mut g, &setup, 700);
                    assert_eq!(g.state.screen, GameScreen::Overworld);
                    assert_eq!((g.bag_screen.cursor(), g.bag_screen.scroll()), (6, 4));
                    fn exchange<const N: usize>(inventory: &mut Inventory<N>, full: bool) {
                        let count = inventory.count();
                        inventory
                            .remove_item(ItemId::Potion, if full { 4 } else { 1 })
                            .unwrap();
                        if full {
                            inventory.add_item(ItemId::Carbos, 1).unwrap();
                        }
                        assert_eq!(inventory.count(), count, "same slot count");
                    }
                    if pc {
                        g.save_data
                            .game_data
                            .pc_items
                            .add_item(ItemId::Potion, 4)
                            .unwrap();
                        exchange(&mut g.save_data.game_data.pc_items, full);
                    } else {
                        exchange(&mut g.save_data.game_data.bag, full);
                    }
                    g.update(&InputState::new());
                    drive(&mut g, &serde_json::json!([[0, "start"], [40, "a"]]), 100);
                    assert_eq!(g.state.screen, GameScreen::Bag);
                    assert_eq!(
                        (g.bag_screen.cursor(), g.bag_screen.scroll()),
                        if full { (0, 0) } else { (6, 4) },
                        "pc={pc} full={full}"
                    );
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
