//! Field Pokédex access through ordinary SRAM/CONTINUE and actual PC input.
use super::*;
use dotzuki_engine::overworld::Direction;
use dotzuki_engine::render_config::RenderConfig;
use pokered_core::pokemon::stats::create_pokemon;
use pokered_core::snapshot::OverworldSnapshot;
use pokered_data::{event_flags::EventFlag, species::Species};
use std::io::Write;

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_file(companion_path_for_save(&self.0));
    }
}
fn step(game: &mut PokemonGame, input: &mut InputState, buttons: &[GbButton]) {
    input.begin_frame();
    input.set_from_bitmask(0);
    for button in buttons {
        input.press(*button);
    }
    game.update(input);
}
fn same_pixels(a: &FrameBuffer, b: &FrameBuffer) -> bool {
    (0..144).all(|y| (0..160).all(|x| a.get_pixel(x, y) == b.get_pixel(x, y)))
}

#[test]
fn cold_bill_pokedex_uses_owned_flags_for_full_and_cached_render_and_only_registers_seen() {
    for (owned, seen) in [(true, true), (false, false), (false, true)] {
        let mut save = SaveData::new();
        save.player_name = pokered_data::charmap::encode_string("RED").unwrap();
        save.game_data.rival_name = pokered_data::charmap::encode_string("BLUE").unwrap();
        save.game_data.player_id = 1234;
        save.game_data.position.map_id = MapId::BillsHouse as u8;
        save.game_data.position.x = 1;
        save.game_data.position.y = 5;
        save.game_data.player_direction = 8;
        save.game_data.last_map = MapId::Route25 as u8;
        save.game_data.options.text_speed = pokered_core::options_menu::TextSpeed::Fast;
        for flag in [
            EventFlag::EVENT_MET_BILL,
            EventFlag::EVENT_MET_BILL_2,
            EventFlag::EVENT_USED_CELL_SEPARATOR_ON_BILL,
            EventFlag::EVENT_GOT_SS_TICKET,
            EventFlag::EVENT_LEFT_BILLS_HOUSE_AFTER_HELPING,
        ] {
            save.game_data.event_flags[flag.byte_offset()] |= flag.bit_mask();
        }
        for bit in [0x61usize, 0x62, 0x63] {
            save.game_data.toggleable_object_flags[bit / 8] |= 1 << (bit % 8);
        }
        save.party
            .add(create_pokemon(Species::Bulbasaur, 20, [0x9a, 0x78]).unwrap())
            .unwrap();
        save.game_data.pokedex.set_owned(Species::Bulbasaur);
        if owned {
            save.game_data.pokedex.set_owned(Species::Eevee);
        } else if seen {
            save.game_data.pokedex.set_seen(Species::Eevee);
        }
        let bytes = export_sram(&save);
        assert!(!import_sram(&bytes).unwrap().imported_legacy_native);
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "pokered-bill-dex-{}-{owned}-{seen}.sav",
            std::process::id()
        )));
        assert!(!companion_path_for_save(&fixture.0).exists());
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&fixture.0)
            .unwrap()
            .write_all(&bytes)
            .unwrap();
        let mut game = PokemonGame::new_with_options(
            GameVersion::Red,
            Some(fixture.0.clone()),
            None,
            None,
            false,
            None,
            false,
            true,
            #[cfg(feature = "debug-server")]
            None,
        );
        let mut input = InputState::new();
        for frame in 0..6000 {
            if game.state.screen == GameScreen::Overworld
                && game.overworld.script_engine_idle()
                && OverworldSnapshot::capture(&game.overworld)
                    .active_script_effect
                    .is_none()
            {
                break;
            }
            step(
                &mut game,
                &mut input,
                if frame % 8 == 0 { &[GbButton::A] } else { &[] },
            );
        }
        assert_eq!(game.state.screen, GameScreen::Overworld);
        for _ in 0..20 {
            step(&mut game, &mut input, &[]);
        }
        for _ in 0..2 {
            step(&mut game, &mut input, &[GbButton::Up]);
        }
        for _ in 0..4 {
            step(&mut game, &mut input, &[]);
        }
        assert_eq!(game.overworld.state.player.facing, Direction::Up);
        assert_eq!(
            (game.overworld.state.player.x, game.overworld.state.player.y),
            (1, 5)
        );
        for _ in 0..3 {
            step(&mut game, &mut input, &[GbButton::A]);
        }
        for frame in 0..6000 {
            if game.overworld.pending_choice.is_some() {
                break;
            }
            let read = frame % 4 == 0
                && game
                    .overworld
                    .pending_dialogue
                    .as_ref()
                    .is_some_and(|d| d.waiting_for_input());
            step(
                &mut game,
                &mut input,
                if read { &[GbButton::A] } else { &[] },
            );
        }
        assert_eq!(
            game.overworld
                .pending_choice
                .as_ref()
                .expect("Bill's list")
                .options[0],
            "EEVEE"
        );
        for _ in 0..4 {
            step(&mut game, &mut input, &[]);
        }
        step(&mut game, &mut input, &[GbButton::A]);
        for _ in 0..20 {
            if game.overworld.pending_pokedex_entry.is_some() {
                break;
            }
            step(&mut game, &mut input, &[]);
        }
        assert_eq!(
            game.overworld
                .pending_pokedex_entry
                .as_ref()
                .unwrap()
                .species,
            "Eevee"
        );
        assert_eq!(
            game.save_data.game_data.pokedex.is_owned(Species::Eevee),
            owned
        );
        assert_eq!(
            game.save_data.game_data.pokedex.is_seen(Species::Eevee),
            seen
        );
        let mut actual = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        game.draw(&mut actual);
        let mut expected = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        // The ordinary Pokédex already implements the owned-data boundary.
        // Mark the reference clone seen so it can select Eevee; this does not
        // grant ownership or change the actual game/save.
        let mut reference_dex = game.save_data.game_data.pokedex.clone();
        reference_dex.set_seen(Species::Eevee);
        let reference = pokered_core::pokedex_screen::PokedexScreenState::new_entry(
            reference_dex,
            Species::Eevee,
            game.state.config.version,
        );
        crate::render::draw_pokedex_screen(
            &reference,
            MapId::BillsHouse,
            false,
            &mut game.resources,
            &mut expected,
        );
        let pages = reference.entry_total_pages();
        assert!(
            same_pixels(&actual, &expected),
            "field dex must respect owned={owned}"
        );
        assert_eq!(
            game.overworld
                .pending_pokedex_entry
                .as_ref()
                .unwrap()
                .total_pages,
            pages
        );
        if !owned {
            assert_eq!(pages, 1);
        }
        let mut session = crate::render::session::RenderSession::new();
        let mut cached = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        let mut scroll = |_: &mut [u8], _: usize, _: usize, _: i32, _: i32, _: u8| {};
        for _ in 0..2 {
            let _ = session.render(&mut game, &mut cached, &mut scroll);
            assert!(
                same_pixels(&cached, &expected),
                "cached field dex must respect owned={owned}"
            );
        }
        for _ in 0..8 {
            if game.overworld.pending_pokedex_entry.is_none() {
                break;
            }
            for _ in 0..4 {
                step(&mut game, &mut input, &[]);
            }
            step(&mut game, &mut input, &[GbButton::B]);
            game.draw(&mut actual);
        }
        assert!(game.overworld.pending_pokedex_entry.is_none());
        assert!(game.save_data.game_data.pokedex.is_seen(Species::Eevee));
        assert_eq!(
            game.save_data.game_data.pokedex.is_owned(Species::Eevee),
            owned
        );
        assert_eq!(std::fs::read(&fixture.0).unwrap(), bytes);
    }
}
