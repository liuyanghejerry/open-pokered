//! Real shared-runtime input tests for fidelity audit items 9–12.
use dotzuki_app::InputState;
use dotzuki_renderer::input::GbButton;
use pokered_app::PokemonGame;
use pokered_core::{
    game_state::{GameScreen, Lang},
    overworld::{Direction, OverworldScreen},
    pokemon::stats::create_pokemon,
    save::SaveData,
};
use pokered_data::{
    impl_traits::PokemonRedData, maps::MapId, species::Species, wild_data::GameVersion,
};

fn game(map: MapId, x: u16, y: u16) -> PokemonGame {
    let mut game = PokemonGame::new_with_options(
        GameVersion::Red,
        None,
        None,
        None,
        false,
        None,
        false,
        true,
        #[cfg(feature = "debug-server")]
        None,
    );
    game.save_data = SaveData::new();
    game.state.screen = GameScreen::Overworld;
    game.state.config.language = Lang::En;
    game.overworld = OverworldScreen::new(map, None, PokemonRedData);
    game.overworld.state.player.x = x;
    game.overworld.state.player.y = y;
    game.overworld.state.player.facing = Direction::Up;
    idle(&mut game, 2);
    game
}
fn idle(g: &mut PokemonGame, frames: usize) {
    for _ in 0..frames {
        g.update(&InputState::new());
    }
}
fn tap(g: &mut PokemonGame, button: GbButton) {
    let mut input = InputState::new();
    input.press(button);
    g.update(&input);
    idle(g, 1);
}
fn until(g: &mut PokemonGame, predicate: impl Fn(&PokemonGame) -> bool) {
    for _ in 0..2000 {
        if predicate(g) {
            return;
        }
        if g.overworld.pending_dialogue.is_some() {
            tap(g, GbButton::A);
        } else {
            idle(g, 1);
        }
    }
    panic!(
        "flow stalled: {:?}",
        g.overworld.active_script_effect_value()
    );
}
fn eevee(g: &mut PokemonGame) {
    tap(g, GbButton::A);
}
fn ask_name(g: &mut PokemonGame) {
    eevee(g);
    until(g, |g| g.overworld.pending_choice.is_some());
    assert_eq!(
        g.overworld.pending_choice.as_ref().unwrap().options,
        ["YES", "NO"]
    );
    assert_eq!(
        g.overworld.active_script_effect_label().as_deref(),
        Some("GivePokemon")
    );
    assert!(
        g.overworld.pending_dialogue.is_some(),
        "nickname question remains beneath YES/NO"
    );
    assert!(!g
        .overworld
        .script_flags()
        .get("EVENT_GOT_EEVEE")
        .copied()
        .unwrap_or(false));
}
#[test]
fn gift_accepts_nickname_and_commits_only_once_after_the_prompt() {
    let mut g = game(MapId::CeladonMansionRoofHouse, 4, 4);
    g.save_data.game_data.player_id = 12345;
    ask_name(&mut g);
    tap(&mut g, GbButton::A);
    until(&mut g, |g| g.overworld.is_naming_screen_active());
    idle(&mut g, 5);
    tap(&mut g, GbButton::A); // first naming-grid character, A
    tap(&mut g, GbButton::Start);
    until(&mut g, |g| g.save_data.party.count() == 1);
    idle(&mut g, 15);
    let mon = g.save_data.party.get(0).unwrap();
    assert_eq!(mon.species, Species::Eevee);
    let mut buf = [0; 64];
    assert_eq!(
        pokered_core::battle::state::decode_name(&mon.nickname, &mut buf),
        "A"
    );
    assert_eq!(mon.ot_id, 12345);
    assert!(g.save_data.game_data.pokedex.is_owned(Species::Eevee));
    assert!(g.overworld.script_flags()["EVENT_GOT_EEVEE"]);
    tap(&mut g, GbButton::A);
    idle(&mut g, 30);
    assert_eq!(g.save_data.party.count(), 1);
}
#[test]
fn declining_or_submitting_empty_nickname_keeps_species_name() {
    for name in [false, true] {
        let mut g = game(MapId::CeladonMansionRoofHouse, 4, 4);
        ask_name(&mut g);
        if name {
            tap(&mut g, GbButton::A);
            until(&mut g, |g| g.overworld.is_naming_screen_active());
            idle(&mut g, 5);
            tap(&mut g, GbButton::Start);
        } else {
            tap(&mut g, GbButton::B);
        }
        until(&mut g, |g| g.save_data.party.count() == 1);
        assert_eq!(
            g.save_data.party.get(0).unwrap().nickname,
            create_pokemon(Species::Eevee, 25, [0, 0]).unwrap().nickname
        );
    }
}
#[test]
fn full_party_still_asks_name_then_sends_to_current_box_and_full_box_retains_ball() {
    for full in [false, true] {
        let mut g = game(MapId::CeladonMansionRoofHouse, 4, 4);
        for _ in 0..6 {
            g.save_data
                .party
                .add(create_pokemon(Species::Pidgey, 5, [0, 0]).unwrap())
                .unwrap();
        }
        g.save_data.pc_storage.change_box(3).unwrap();
        if full {
            for _ in 0..20 {
                g.save_data
                    .pc_storage
                    .deposit_to_current(create_pokemon(Species::Rattata, 5, [0, 0]).unwrap())
                    .unwrap();
            }
        }
        g.save_data.current_box = g.save_data.pc_storage.current_box().clone();
        g.overworld.party_count = 6;
        g.overworld.box_count = g.save_data.current_box.count() as u8;
        eevee(&mut g);
        let mut saw_box_notice = false;
        let mut saw_nickname_prompt = false;
        for _ in 0..1500 {
            if g.overworld.pending_choice.is_some() {
                assert!(!full, "full box rejects before asking for a name");
                saw_nickname_prompt = true;
                tap(&mut g, GbButton::B);
            } else if let Some(d) = &g.overworld.pending_dialogue {
                saw_box_notice |= d
                    .pages()
                    .iter()
                    .any(|p| p.line1.contains("BOX 4") || p.line2.contains("BOX 4"));
                tap(&mut g, GbButton::A);
            } else {
                idle(&mut g, 1);
            }
            if g.overworld.active_script_effect_label().is_none() {
                break;
            }
        }
        idle(&mut g, 15);
        assert_eq!(g.save_data.party.count(), 6);
        assert_eq!(
            g.save_data.pc_storage.current_box().count(),
            if full { 20 } else { 1 }
        );
        assert_eq!(
            g.overworld
                .script_flags()
                .get("EVENT_GOT_EEVEE")
                .copied()
                .unwrap_or(false),
            !full
        );
        assert_eq!(saw_box_notice, !full);
        assert_eq!(saw_nickname_prompt, !full);
    }
}
#[test]
fn museum_back_counter_requires_a_and_ticket_gate_still_triggers_on_step() {
    let mut g = game(MapId::Museum1F, 13, 5);
    let mut up = InputState::new();
    up.press(GbButton::Up);
    for _ in 0..8 {
        g.update(&up);
    }
    idle(&mut g, 40);
    assert_eq!(
        (g.overworld.state.player.x, g.overworld.state.player.y),
        (13, 4)
    );
    assert!(g.overworld.pending_dialogue.is_none());
    g.overworld.state.player.facing = Direction::Left;
    tap(&mut g, GbButton::A);
    idle(&mut g, 8);
    assert!(
        g.overworld.pending_dialogue.is_some(),
        "manual scientist chat remains available"
    );
    let mut g = game(MapId::Museum1F, 9, 5);
    for _ in 0..8 {
        g.update(&up);
    }
    idle(&mut g, 40);
    assert!(
        g.overworld.pending_dialogue.is_some(),
        "front ticket gate remains automatic"
    );
}
#[test]
fn reading_menus_allow_repeated_headings_and_exit_with_b() {
    for map in [MapId::ViridianSchoolHouse, MapId::CeladonMansionRoofHouse] {
        let mut g = game(map, 3, 1);
        tap(&mut g, GbButton::A);
        until(&mut g, |g| g.overworld.pending_choice.is_some());
        let headings = g.overworld.pending_choice.as_ref().unwrap().options.clone();
        tap(&mut g, GbButton::Down);
        for _ in 0..4 {
            tap(&mut g, GbButton::A);
            until(&mut g, |g| g.overworld.pending_choice.is_some());
            assert_eq!(
                g.overworld.pending_choice.as_ref().unwrap().options,
                headings
            );
            assert_eq!(g.overworld.pending_choice.as_ref().unwrap().selected, 1);
        }
        tap(&mut g, GbButton::B);
        idle(&mut g, 10);
        assert!(g.overworld.pending_choice.is_none());
        assert!(g.overworld.active_script_effect_label().is_none());
    }
}
#[test]
fn vending_displays_prices_delivers_for_120_frames_and_charges_once() {
    for (selection, cost, item) in [
        (0, 200, "FRESH_WATER"),
        (1, 300, "SODA_POP"),
        (2, 350, "LEMONADE"),
    ] {
        let mut g = game(MapId::CeladonMartRoof, 10, 2);
        g.save_data.game_data.player_money = 1000;
        tap(&mut g, GbButton::A);
        until(&mut g, |g| g.overworld.pending_choice.is_some());
        let options = &g.overworld.pending_choice.as_ref().unwrap().options;
        for price in ["¥200", "¥300", "¥350"] {
            assert!(options.iter().any(|o| o.contains(price)));
        }
        assert_eq!(g.overworld.script_money_box, Some(1000));
        for _ in 0..selection {
            tap(&mut g, GbButton::Down);
        }
        tap(&mut g, GbButton::A);
        until(&mut g, |g| {
            g.overworld.active_script_effect_label().as_deref() == Some("VendingDelivery")
        });
        for _ in 0..119 {
            idle(&mut g, 1);
            assert_eq!(
                g.overworld.active_script_effect_label().as_deref(),
                Some("VendingDelivery")
            );
        }
        idle(&mut g, 1);
        assert_ne!(
            g.overworld.active_script_effect_label().as_deref(),
            Some("VendingDelivery")
        );
        assert_eq!(
            g.save_data.game_data.player_money, 1000,
            "charge follows delivery text"
        );
        until(&mut g, |g| {
            g.overworld.active_script_effect_label().is_none()
        });
        idle(&mut g, 5);
        assert_eq!(g.save_data.game_data.player_money, 1000 - cost);
        assert!(g.save_data.game_data.bag.has_item(
            pokered_data::items::ItemId::from_const_name(item).unwrap(),
            1
        ));
        assert!(g.overworld.script_money_box.is_none());
    }
}

#[test]
fn boxed_gift_can_be_named_and_chinese_prompt_is_localized() {
    for boxed in [false, true] {
        let mut g = game(MapId::CeladonMansionRoofHouse, 4, 4);
        g.state.config.language = Lang::Zh;
        g.overworld.set_script_lang("zh");
        if boxed {
            for _ in 0..6 {
                g.save_data
                    .party
                    .add(create_pokemon(Species::Pidgey, 5, [0, 0]).unwrap())
                    .unwrap();
            }
            g.overworld.party_count = 6;
        }
        eevee(&mut g);
        until(&mut g, |g| g.overworld.pending_choice.is_some());
        assert_eq!(
            g.overworld.pending_choice.as_ref().unwrap().options,
            ["是", "否"]
        );
        assert!(g
            .overworld
            .pending_dialogue
            .as_ref()
            .unwrap()
            .pages()
            .iter()
            .any(|p| p.line1.contains("昵称") || p.line2.contains("昵称")));
        tap(&mut g, GbButton::A);
        until(&mut g, |g| g.overworld.is_naming_screen_active());
        idle(&mut g, 5);
        tap(&mut g, GbButton::A);
        tap(&mut g, GbButton::Start);
        until(&mut g, |g| {
            g.overworld.active_script_effect_label().is_none()
        });
        let mon = if boxed {
            g.save_data.pc_storage.current_box().get(0).unwrap()
        } else {
            g.save_data.party.get(0).unwrap()
        };
        assert_eq!(mon.species, Species::Eevee);
        assert_ne!(
            mon.nickname,
            create_pokemon(Species::Eevee, 25, [0, 0]).unwrap().nickname
        );
    }
}

#[test]
fn vending_cancel_no_money_and_full_bag_do_not_deliver_or_charge() {
    for mode in [0, 1, 2] {
        let mut g = game(MapId::CeladonMartRoof, 10, 2);
        let money = if mode == 1 { 0 } else { 1000 };
        g.save_data.game_data.player_money = money;
        if mode == 2 {
            for id in 1..=255 {
                {
                    let item = pokered_data::items::ItemId::from_id(id);
                    if !["FRESH_WATER", "SODA_POP", "LEMONADE"].iter().any(|name| {
                        Some(item) == pokered_data::items::ItemId::from_const_name(name)
                    }) {
                        let _ = g.save_data.game_data.bag.add_item(item, 1);
                    }
                    if g.save_data.game_data.bag.count() == 20 {
                        break;
                    }
                }
            }
            assert_eq!(g.save_data.game_data.bag.count(), 20);
        }
        let bag = g.save_data.game_data.bag.clone();
        tap(&mut g, GbButton::A);
        until(&mut g, |g| g.overworld.pending_choice.is_some());
        tap(&mut g, if mode == 0 { GbButton::B } else { GbButton::A });
        for _ in 0..500 {
            assert_ne!(
                g.overworld.active_script_effect_label().as_deref(),
                Some("VendingDelivery")
            );
            if g.overworld.pending_dialogue.is_some() {
                tap(&mut g, GbButton::A);
            } else {
                idle(&mut g, 1);
            }
            if g.overworld.active_script_effect_label().is_none() {
                break;
            }
        }
        assert_eq!(g.save_data.game_data.player_money, money);
        assert_eq!(g.save_data.game_data.bag, bag);
        assert!(g.overworld.script_money_box.is_none());
    }
}

#[test]
fn snapshot_roundtrip_preserves_reading_and_money_overlays() {
    for map in [MapId::ViridianSchoolHouse, MapId::CeladonMartRoof] {
        let mut g = game(
            map,
            if map == MapId::ViridianSchoolHouse {
                3
            } else {
                10
            },
            if map == MapId::ViridianSchoolHouse {
                1
            } else {
                2
            },
        );
        tap(&mut g, GbButton::A);
        until(&mut g, |g| g.overworld.pending_choice.is_some());
        let saved = pokered_core::snapshot::OverworldSnapshot::capture(&g.overworld);
        let json = serde_json::to_vec(&saved).unwrap();
        let restored: pokered_core::snapshot::OverworldSnapshot =
            serde_json::from_slice(&json).unwrap();
        let before = g.overworld.active_script_effect_value();
        g.overworld.script_money_box = None;
        restored.restore_into(&mut g.overworld);
        assert_eq!(g.overworld.active_script_effect_value(), before);
        assert_eq!(g.overworld.script_money_box, saved.script_money_box);
        tap(&mut g, GbButton::B);
        until(&mut g, |g| {
            g.overworld.active_script_effect_label().is_none()
        });
    }
}
