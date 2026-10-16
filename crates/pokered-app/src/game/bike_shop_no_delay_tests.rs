//! Real cold SRAM -> CONTINUE -> ordinary talk -> shared audio/PCM controls.
//! No script/phase/flag/last setter, snapshot restore or debug RPC is used.
use super::*;
use pokered_core::foreground_input::{A, B, DOWN};
use pokered_core::save::{export_sram, import_sram, SaveData};
use pokered_data::{moves::MoveId, species::Species};
use serde_json::json;

struct Session {
    game: PokemonGame,
    input: InputState,
    path: std::path::PathBuf,
    bytes: Vec<u8>,
    pcm: Vec<f32>,
}
impl Session {
    fn step(&mut self, buttons: u8) -> (u8, bool) {
        self.input.begin_frame();
        for (bit, key) in [
            (A, GbButton::A),
            (B, GbButton::B),
            (DOWN, GbButton::Down),
            (0x40, GbButton::Up),
            (8, GbButton::Start),
        ] {
            if buttons & bit != 0 {
                self.input.press(key);
            } else {
                self.input.release(key);
            }
        }
        self.game.update(&self.input);
        let audio = self
            .game
            .audio
            .as_ref()
            .expect("no-device still has logical audio");
        let (id, owned_channel) = {
            let manager = audio.manager.lock().unwrap();
            let id = manager.sequencer.current_sfx_id;
            (
                id,
                manager.is_sfx_playing()
                    && (4..8).any(|channel| {
                        let state = &manager.sequencer.channels[channel];
                        state.active && state.sound_id == id
                    }),
            )
        };
        audio.render_pcm(&mut self.pcm);
        (
            id,
            owned_channel && self.pcm.iter().any(|sample| sample.abs() > 1e-7),
        )
    }
    fn boot(no_delay: bool) -> Self {
        let mut save = SaveData::new();
        save.player_name = pokered_data::charmap::encode_string("RED").unwrap();
        save.game_data.player_id = 1234;
        save.game_data.player_money = 999_999;
        save.game_data.position.map_id = pokered_data::maps::MapId::BikeShop as u8;
        save.game_data.position.x = 6;
        save.game_data.position.y = 4;
        save.game_data.position.x_block = 3;
        save.game_data.position.y_block = 2;
        save.game_data.last_map = pokered_data::maps::MapId::CeruleanCity as u8;
        save.game_data.options.text_speed = pokered_core::options_menu::TextSpeed::Fast;
        if no_delay {
            save.game_data.status_flags[5] |= 0x40;
        }
        let mut mon = pokered_core::pokemon::stats::create_pokemon_with_moves(
            Species::Bulbasaur,
            5,
            [0x9a, 0x78],
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap();
        mon.ot_id = 1234;
        save.party.add(mon).unwrap();
        let bytes = export_sram(&save);
        assert_eq!(
            import_sram(&bytes).unwrap().game_data.status_flags[5] & 0x40,
            if no_delay { 0x40 } else { 0 }
        );
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("bike-no-delay-{}-{unique}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("fixture.sav");
        std::fs::write(&path, &bytes).unwrap();
        let mut game = PokemonGame::new_with_options(
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
        game.set_seed(42);
        let mut session = Self {
            game,
            input: InputState::new(),
            path,
            bytes,
            pcm: vec![0.; 1470],
        };
        for frame in 0..8000 {
            if session.game.state.screen == GameScreen::Overworld
                && session.game.overworld.script_engine_idle()
                && session
                    .game
                    .overworld
                    .active_script_effect_label()
                    .is_none()
            {
                break;
            }
            session.step(if frame % 8 < 2 { A } else { 0 });
        }
        for _ in 0..16 {
            session.step(0);
        }
        assert_eq!(session.game.state.screen, GameScreen::Overworld);
        assert_eq!(
            (
                session.game.overworld.state.player.x,
                session.game.overworld.state.player.y
            ),
            (6, 4)
        );
        assert!(
            !session.game.overworld.no_text_delay(),
            "ordinary MainMenu clears even a saved true byte before Continue"
        );
        for buttons in [0x40, 0x40, 0, 0, A, A, 0, 0] {
            session.step(buttons);
        }
        assert_eq!(
            session
                .game
                .overworld
                .active_script_effect_label()
                .as_deref(),
            Some("BikeShopOffer")
        );
        session
    }
    fn phase(&self) -> serde_json::Value {
        self.game.overworld.active_script_effect_value().unwrap()["state"]["phase"].clone()
    }
    fn reach_final_welcome_prompt(&mut self) {
        for frame in 0..8000 {
            if self.phase() == json!({"Prompt":"Question"}) {
                return;
            }
            self.step(if frame % 16 < 2 { A } else { 0 });
        }
        panic!("ordinary Welcome/PARA/PROMPT did not reach question producer");
    }
    fn state(&self) -> serde_json::Value {
        self.game.overworld.active_script_effect_value().unwrap()["state"].clone()
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("preserving Bike fixture {}", self.path.display());
        } else {
            let _ = std::fs::remove_dir_all(self.path.parent().unwrap());
        }
    }
}

#[test]
fn ordinary_cold_bike_consumer_owns_fresh_mask_pcm_and_real_start_clear() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            // Both physical saved bit values load the canonical byte. MainMenu's
            // actual clear writer executes before either ordinary talk path.
            for (saved_true, prepared, pressed, expected_mask, selected, keep) in [
                (false, A, A, A, 0, false),
                (true, A, A, A, 0, false),
                (false, 0, A | B, A | B, 0, true),
                (false, B, A | B, A, 0, false),
                (false, 0, DOWN | A, DOWN | A, 1, false),
                (false, 0, B, B, 0, true),
            ] {
                let mut s = Session::boot(saved_true);
                s.reach_final_welcome_prompt();
                s.step(0);
                s.step(B);
                assert_eq!(s.phase(), json!({"Text":"Question"}));
                for _ in 0..5 {
                    s.step(prepared);
                    assert_eq!(
                        s.game.overworld.source_foreground().unwrap().last(),
                        Some(B),
                        "no-poll preparation must retain actual Welcome B"
                    );
                }
                let (id, pcm) = s.step(if prepared == A { pressed } else { prepared });
                if prepared == A {
                    assert_eq!(id, SfxId::PressAB as u8);
                    assert!(pcm);
                } else {
                    assert_eq!(s.phase(), json!("Menu"));
                    let (id, pcm) = s.step(pressed);
                    assert_eq!(id, SfxId::PressAB as u8);
                    assert!(pcm, "actual accepted menu key emits PCM");
                }
                assert_eq!(s.state()["accepted_buttons"], json!(expected_mask));
                assert_eq!(s.state()["selected"], json!(selected));
                assert_eq!(s.game.overworld.no_text_delay(), keep);
                assert_eq!(
                    s.game.save_data.game_data.player_money, 999_999,
                    "sale never compares or debits the impossible price"
                );
                if keep {
                    for _ in 0..20 {
                        s.step(B);
                    }
                    assert_eq!(
                        s.phase(),
                        json!("OuterPrompt"),
                        "held menu B must not close instant goodbye"
                    );
                    s.step(0);
                    s.step(A);
                    s.step(A);
                    assert_eq!(s.phase(), json!("OuterHold"));
                    s.step(0);
                    s.step(0);
                }
                for frame in 0..8000 {
                    if s.game.overworld.script_engine_idle()
                        && s.game.overworld.active_script_effect_label().is_none()
                    {
                        break;
                    }
                    s.step(if frame % 16 < 2 { A } else { 0 });
                }
                assert!(s.game.overworld.script_engine_idle());
                assert_eq!(
                    s.game.overworld.no_text_delay(),
                    keep,
                    "ordinary text close is not a bit writer"
                );
                assert!(!s
                    .game
                    .overworld
                    .script_flags()
                    .get("EVENT_GOT_BICYCLE")
                    .copied()
                    .unwrap_or(false));
                assert!(!s.game.save_data.game_data.bag.has_item_const("BICYCLE"));
                assert_eq!(
                    std::fs::read(&s.path).unwrap(),
                    s.bytes,
                    "no SAVE occurs in this test"
                );
                for buttons in [0, 0, 8, 8, 0, 0] {
                    s.step(buttons);
                }
                assert_eq!(s.game.state.screen, GameScreen::StartMenu);
                assert!(
                    !s.game.overworld.no_text_delay(),
                    "real DrawStartMenu unconditionally clears before menu input"
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
