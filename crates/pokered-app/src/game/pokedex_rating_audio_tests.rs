//! Cold physical SRAM, normal Continue and PC input, actual shared audio owner.
use super::*;
use pokered_core::foreground_input::{A, B, DOWN, UP};
use pokered_core::pc_screen::PcPhase;
use pokered_core::pokedex_rating::DexRatingSound;
use pokered_core::save::{sram_export::export_sram, sram_import::import_sram};
use pokered_data::species::Species;

struct Session {
    game: PokemonGame,
    input: InputState,
    path: PathBuf,
    bytes: Vec<u8>,
    renderer: crate::render::session::RenderSession,
    retained: FrameBuffer,
}
impl Session {
    fn step(&mut self, buttons: u8) {
        self.input.begin_frame();
        for (bit, key) in [
            (A, GbButton::A),
            (B, GbButton::B),
            (UP, GbButton::Up),
            (DOWN, GbButton::Down),
        ] {
            if buttons & bit != 0 {
                self.input.press(key);
            } else {
                self.input.release(key);
            }
        }
        self.game.update(&self.input);
        if self.game.state.screen == GameScreen::PC {
            let mut scroll = |_: &mut [u8], _: usize, _: usize, _: i32, _: i32, _: u8| {};
            self.renderer
                .render(&mut self.game, &mut self.retained, &mut scroll);
            let mut full = FrameBuffer::new(
                dotzuki_engine::render_config::RenderConfig::new(160, 144),
                pokered_renderer::Rgba::WHITE,
            );
            self.game.draw(&mut full);
            assert_eq!(
                self.retained.packed(),
                full.packed(),
                "PC incremental rendering must follow the actual letter cursor"
            );
        }
    }
    fn pc(&self) -> &pokered_core::pc_screen::PcScreen {
        self.game.pc_screen.as_ref().expect("actual PC interaction")
    }
    fn sound(&self) -> (u8, bool) {
        let m = self.game.audio.as_ref().unwrap().manager.lock().unwrap();
        (m.sequencer.current_sfx_id, m.is_sfx_playing())
    }
    fn boot(owned: u32) -> Self {
        Self::boot_at(owned, false)
    }
    fn boot_at(owned: u32, lab: bool) -> Self {
        let mut save = SaveData::new();
        save.player_name = pokered_data::charmap::encode_string("RED").unwrap();
        save.game_data.player_id = 1234;
        save.game_data.position.map_id = if lab {
            MapId::OaksLab
        } else {
            MapId::ViridianPokecenter
        } as u8;
        let (x, y) = if lab { (5, 3) } else { (13, 4) };
        save.game_data.position.x = x;
        save.game_data.position.y = y;
        save.game_data.position.x_block = x / 2;
        save.game_data.position.y_block = y / 2;
        save.game_data.player_direction = 4; // sprite facing up
        save.game_data.last_map = MapId::ViridianCity as u8;
        save.game_data.options.text_speed = pokered_core::options_menu::TextSpeed::Medium;
        use pokered_data::event_flags::EventFlag;
        save.game_data.event_flags.resize(320, 0);
        for flag in [
            EventFlag::EVENT_GOT_POKEDEX,
            EventFlag::EVENT_GOT_STARTER,
            EventFlag::EVENT_BATTLED_RIVAL_IN_OAKS_LAB,
            EventFlag::EVENT_PALLET_AFTER_GETTING_POKEBALLS,
            EventFlag::EVENT_OAK_GOT_PARCEL,
            EventFlag::EVENT_FOLLOWED_OAK_INTO_LAB,
        ] {
            let flag = flag as usize;
            save.game_data.event_flags[flag / 8] |= 1 << (flag % 8);
        }
        if lab {
            save.game_data.last_map = MapId::PalletTown as u8;
            let bit = pokered_data::toggleable_objects::toggle_id_to_bit_index("OAKS_LAB_OBJ_5")
                .unwrap() as usize;
            save.game_data.toggleable_object_flags[bit / 8] &= !(1 << (bit % 8));
            for id in [
                "OAKS_LAB_OBJ_1",
                "OAKS_LAB_OBJ_2",
                "OAKS_LAB_OBJ_3",
                "OAKS_LAB_OBJ_4",
                "OAKS_LAB_OBJ_6",
                "OAKS_LAB_OBJ_7",
                "OAKS_LAB_OBJ_8",
            ] {
                let bit =
                    pokered_data::toggleable_objects::toggle_id_to_bit_index(id).unwrap() as usize;
                save.game_data.toggleable_object_flags[bit / 8] |= 1 << (bit % 8);
            }
        }
        for n in 1..=owned {
            save.game_data
                .pokedex
                .set_owned(Species::from_index_id(n as u8));
        }
        let mut mon =
            pokered_core::pokemon::stats::create_pokemon(Species::Bulbasaur, 5, [0x9a, 0x78])
                .unwrap();
        mon.ot_id = 1234;
        save.party.add(mon).unwrap();
        let bytes = export_sram(&save);
        let restored = import_sram(&bytes).unwrap();
        assert!(!restored.imported_legacy_native);
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("pokedex-rating-{}-{unique}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("fixture.sav");
        std::fs::write(&path, &bytes).unwrap();
        let game = PokemonGame::new_with_options(
            GameVersion::Red,
            Some(path.clone()),
            None,
            None,
            false,
            None,
            false,
            true,
            #[cfg(feature = "debug-server")]
            None,
        );
        let mut s = Self {
            game,
            input: InputState::new(),
            path,
            bytes,
            renderer: crate::render::session::RenderSession::new(),
            retained: FrameBuffer::new(
                dotzuki_engine::render_config::RenderConfig::new(160, 144),
                pokered_renderer::Rgba::WHITE,
            ),
        };
        s.game.set_seed(42);
        for frame in 0..8000 {
            if s.game.state.screen == GameScreen::Overworld && s.game.overworld.script_engine_idle()
            {
                break;
            }
            s.step(if frame % 8 < 2 { A } else { 0 });
        }
        assert_eq!(s.game.state.screen, GameScreen::Overworld);
        for _ in 0..32 {
            s.step(0);
        }
        if lab {
            return s;
        }
        for keys in [UP, UP, 0, 0, A, A, 0, 0] {
            s.step(keys);
        }
        for frame in 0..2000 {
            if s.game
                .pc_screen
                .as_ref()
                .is_some_and(|pc| pc.phase() == PcPhase::OaksConfirm)
            {
                break;
            }
            let keys = if frame % 8 != 0 {
                0
            } else {
                match s.game.pc_screen.as_ref().map(|pc| pc.phase()) {
                    Some(PcPhase::MainMenu) if s.pc().main_menu().cursor() < 2 => DOWN,
                    Some(PcPhase::MainMenu) | Some(PcPhase::Message) => A,
                    _ => 0,
                }
            };
            s.step(keys);
        }
        assert_eq!(s.pc().phase(), PcPhase::OaksConfirm);
        s
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("preserving {}", self.path.display());
        } else {
            let _ = std::fs::remove_dir_all(self.path.parent().unwrap());
        }
    }
}

#[test]
fn cold_pc_ratings_play_all_seven_real_fanfares_then_silent_ack_and_closed_text() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            for owned in [0, 10, 40, 60, 90, 120, 150] {
                let mut s = Session::boot(owned);
                s.step(0);
                s.step(A);
                // Navigate only completion pages. Once rating text owns its
                // cursor, leave it to the actual PrintLetterDelay/audio owner.
                for frame in 0..4000 {
                    if s.pc().rating_visible_chars().is_some() {
                        break;
                    }
                    s.step(if frame % 8 == 0 { A } else { 0 });
                }
                assert!(s.pc().rating_visible_chars().is_some());
                assert_eq!(s.pc().dex_owned(), owned);
                let expected =
                    parse_sfx_id(DexRatingSound::for_owned(owned).script_id()).unwrap() as u8;
                let mut started = false;
                let mut completed = false;
                let mut audible = false;
                let mut starts = 0;
                let mut previous = false;
                for _ in 0..6000 {
                    s.step(0);
                    let playing = s.sound() == (expected, true);
                    if playing && !previous {
                        starts += 1;
                        started = true;
                    }
                    previous = playing;
                    if playing {
                        let mut pcm = [0.; 1470];
                        s.game.audio.as_ref().unwrap().render_pcm(&mut pcm);
                        audible |= pcm.iter().any(|x| x.abs() > 1e-7);
                        assert!(s.pc().rating_waiting_for_audio());
                    }
                    if started && !s.pc().rating_waiting_for_audio() {
                        completed = true;
                        break;
                    }
                }
                assert!(
                    started && completed && audible,
                    "count {owned}: actual sound completion/PCM"
                );
                assert_eq!(starts, 1);
                assert_eq!(
                    s.game.audio.as_ref().unwrap().last_music_id(),
                    Some(MusicId::POKECENTER)
                );
                let lines = s.pc().message_lines().to_vec();
                for _ in 0..30 {
                    s.step(0);
                    assert_eq!(s.pc().message_lines(), lines);
                }
                s.step(A); // silent rating ACK; PC caller does not HoldA.
                assert_eq!(s.pc().message_lines(), ["Closed link to", "PROF.OAK's PC."]);
                assert_ne!(s.sound(), (SfxId::PressAB as u8, true));
                for _ in 0..300 {
                    s.step(A);
                }
                assert_eq!(
                    s.pc().phase(),
                    PcPhase::Message,
                    "held A cannot acknowledge closed-link text"
                );
                s.step(0);
                s.step(A);
                assert_eq!(s.pc().phase(), PcPhase::MainMenu);
                assert_eq!(s.sound(), (SfxId::PressAB as u8, true));
                assert_eq!(
                    std::fs::read(&s.path).unwrap(),
                    s.bytes,
                    "rating must not mutate physical SRAM"
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn cold_pc_rating_refusal_closes_with_confirmation_and_no_rating_fanfare() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let mut s = Session::boot(40);
            s.step(0);
            s.step(DOWN);
            s.step(0);
            s.step(A);
            let ids = [0, 10, 40, 60, 90, 120, 150]
                .map(|n| parse_sfx_id(DexRatingSound::for_owned(n).script_id()).unwrap() as u8);
            let mut closed = false;
            for frame in 0..3000 {
                s.step(if frame % 8 == 0 { A } else { 0 });
                assert!(!s.pc().rating_waiting_for_audio());
                let (id, playing) = s.sound();
                assert!(!playing || !ids.contains(&id));
                if s.pc().message_lines() == ["Closed link to", "PROF.OAK's PC."] {
                    closed = true;
                }
                if s.pc().phase() == PcPhase::MainMenu {
                    break;
                }
            }
            assert!(closed);
            assert_eq!(s.pc().phase(), PcPhase::MainMenu);
            assert_eq!(s.sound(), (SfxId::PressAB as u8, true));
            assert_eq!(std::fs::read(&s.path).unwrap(), s.bytes);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn cold_lab_rating_waits_for_actual_sound_then_silent_ack_holds_a_before_closing() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let mut s = Session::boot_at(150, true);
            for keys in [UP, UP, 0, 0, A, A, 0, 0] {
                s.step(keys);
            }
            for frame in 0..5000 {
                if s.game.overworld.active_script_effect_label().as_deref() == Some("PokedexRating")
                {
                    break;
                }
                s.step(if frame % 8 == 0 { A } else { 0 });
            }
            assert_eq!(
                s.game.overworld.active_script_effect_label().as_deref(),
                Some("PokedexRating")
            );
            let phase = |s: &Session| {
                s.game.overworld.active_script_effect_value().unwrap()["state"]["phase"].clone()
            };
            let expected = parse_sfx_id(DexRatingSound::GetItem2.script_id()).unwrap() as u8;
            let mut started = false;
            let mut audible = false;
            let mut starts = 0;
            let mut previous = false;
            for _ in 0..6000 {
                s.step(0);
                let playing = s.sound() == (expected, true);
                if playing && !previous {
                    starts += 1;
                    started = true;
                }
                previous = playing;
                if playing {
                    let mut pcm = [0.; 1470];
                    s.game.audio.as_ref().unwrap().render_pcm(&mut pcm);
                    audible |= pcm.iter().any(|x| x.abs() > 1e-7);
                    assert!(s.game.overworld.retain_field_text);
                }
                if phase(&s) == serde_json::json!("Ack") {
                    break;
                }
            }
            assert!(started && audible);
            assert_eq!(starts, 1);
            assert_eq!(phase(&s), serde_json::json!("Ack"));
            assert!(!s.sound().1);
            assert_eq!(
                s.game.audio.as_ref().unwrap().last_music_id(),
                MusicId::from_u8(
                    pokered_core::overworld::map_loading::get_map_music(MapId::OaksLab) as u8
                )
            );
            s.step(A);
            assert_eq!(phase(&s), serde_json::json!("ReleaseA"));
            assert_ne!(s.sound(), (SfxId::PressAB as u8, true));
            for _ in 0..30 {
                s.step(A);
                assert!(s.game.overworld.pending_dialogue.is_some());
                assert_eq!(phase(&s), serde_json::json!("ReleaseA"));
            }
            s.step(0);
            assert!(s.game.overworld.pending_dialogue.is_none());
            for _ in 0..4 {
                s.step(0);
            }
            assert!(s.game.overworld.active_script_effect_label().is_none());
            assert!(s.game.overworld.script_engine_idle());
            assert_eq!(std::fs::read(&s.path).unwrap(), s.bytes);
        })
        .unwrap()
        .join()
        .unwrap();
}
