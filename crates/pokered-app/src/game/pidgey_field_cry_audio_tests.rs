//! One ordinary cold SRAM caller with the real muted App audio owner.
//! Construct only the preboot save; never replace scripts or sound flags.
use super::*;
use pokered_core::foreground_input::{A, B, UP};
use pokered_core::save::{sram_export::export_sram, sram_import::import_sram};
use pokered_data::species::Species;
use serde_json::json;

struct Session {
    game: PokemonGame,
    input: InputState,
    path: PathBuf,
    bytes: Vec<u8>,
    pcm: Vec<f32>,
    audible_cry: bool,
    cry_starts: usize,
    previous_cry: bool,
}

impl Session {
    fn sound(&self) -> (u8, bool) {
        let manager = self.game.audio.as_ref().unwrap().manager.lock().unwrap();
        (manager.sequencer.current_sfx_id, manager.is_sfx_playing())
    }

    fn step(&mut self, buttons: u8) {
        self.input.begin_frame();
        for (bit, key) in [(A, GbButton::A), (B, GbButton::B), (UP, GbButton::Up)] {
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
            .expect("muted mode still owns logical audio");
        let cry_id = pokered_data::cries::cry_data(Species::Pidgey).sfx;
        let (cry, owned_channel) = {
            let manager = audio.manager.lock().unwrap();
            let cry = manager.is_sfx_playing() && manager.sequencer.current_sfx_id == cry_id;
            (
                cry,
                cry && (4..8).any(|index| {
                    let channel = &manager.sequencer.channels[index];
                    channel.active && channel.sound_id == cry_id
                }),
            )
        };
        if cry && !self.previous_cry {
            self.cry_starts += 1;
        }
        self.previous_cry = cry;
        audio.render_pcm(&mut self.pcm);
        self.audible_cry |= owned_channel && self.pcm.iter().any(|sample| sample.abs() > 1e-7);
    }

    fn phase(&self) -> Option<serde_json::Value> {
        self.game
            .overworld
            .active_script_effect_value()
            .filter(|_| {
                self.game.overworld.active_script_effect_label().as_deref() == Some("FieldCry")
            })
            .map(|value| value["state"]["phase"].clone())
    }

    fn boot() -> Self {
        let mut save = SaveData::new();
        save.player_name = pokered_data::charmap::encode_string("RED").unwrap();
        save.game_data.player_id = 1234;
        save.game_data.position.map_id = MapId::VermilionPidgeyHouse as u8;
        save.game_data.position.x = 3;
        save.game_data.position.y = 6;
        save.game_data.position.x_block = 1;
        save.game_data.position.y_block = 3;
        save.game_data.last_map = MapId::VermilionCity as u8;
        save.game_data.options.text_speed = pokered_core::options_menu::TextSpeed::Medium;
        let mut mon =
            pokered_core::pokemon::stats::create_pokemon(Species::Bulbasaur, 5, [0x9a, 0x78])
                .unwrap();
        mon.ot_id = 1234;
        save.party.add(mon).unwrap();
        let bytes = export_sram(&save);
        let restored = import_sram(&bytes).unwrap();
        assert!(!restored.imported_legacy_native);
        assert_eq!(
            restored.game_data.position.map_id,
            MapId::VermilionPidgeyHouse as u8
        );
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("pidgey-field-cry-{}-{unique}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("fixture.sav");
        std::fs::write(&path, &bytes).unwrap();
        assert!(!companion_path_for_save(&path).exists());
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
        game.set_seed(42); // Once before the first update; never search seeds.
        assert_eq!(game.state.screen, GameScreen::GameFreakSplash);
        let mut session = Self {
            game,
            input: InputState::new(),
            path,
            bytes,
            pcm: vec![0.; 1470],
            audible_cry: false,
            cry_starts: 0,
            previous_cry: false,
        };
        for frame in 0..8000 {
            // Stuck-test bound, never a cry duration.
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
            (3, 6)
        );
        // The actual Bird keeps its normal Wander/range2 behavior. These are
        // ordinary facing/talk inputs; no NPC freeze, position or VM setter.
        for buttons in [UP, UP, 0, 0, A, A, 0, 0] {
            session.step(buttons);
        }
        assert!(
            session.phase().is_some(),
            "normal npc2 interaction must reach embedded fieldCry"
        );
        session
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("preserving Pidgey fixture {}", self.path.display());
        } else {
            let _ = std::fs::remove_dir_all(self.path.parent().unwrap());
        }
    }
}

#[test]
fn cold_pidgey_talk_keeps_window_through_real_cry_then_silent_ack_and_a_release() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let mut session = Session::boot();
            for _ in 0..2000 {
                if session.phase() == Some(json!("SoundWait")) {
                    break;
                }
                session.step(0);
            }
            assert_eq!(session.phase(), Some(json!("SoundWait")));
            assert_eq!(
                session.sound(),
                (pokered_data::cries::cry_data(Species::Pidgey).sfx, true),
                "the first SoundWait update must start the actual App sequencer"
            );
            assert_eq!(session.cry_starts, 1);
            assert_eq!(
                session.game.overworld.source_foreground().unwrap().last(),
                Some(0)
            );
            let cursor =
                serde_json::to_value(session.game.overworld.pending_dialogue.as_ref().unwrap())
                    .unwrap();
            let mut waited = 0;
            for _ in 0..2000 {
                session.step(A); // First pressed during the sound wait: no Joypad poll yet.
                if !session.sound().1 {
                    break;
                }
                waited += 1;
                assert_eq!(session.phase(), Some(json!("SoundWait")));
                assert_eq!(
                    serde_json::to_value(session.game.overworld.pending_dialogue.as_ref().unwrap())
                        .unwrap(),
                    cursor
                );
                assert!(session.game.overworld.retain_field_text);
                assert_eq!(
                    session.game.overworld.source_foreground().unwrap().last(),
                    Some(0)
                );
                assert_eq!(session.game.overworld.sfx_event, OverworldSfxEvent::None);
            }
            assert!(
                waited > 0 && session.audible_cry,
                "real cry must own a channel and emit PCM"
            );
            assert!(
                !session.sound().1,
                "sequencer must complete without an estimated timer"
            );
            assert_eq!(session.phase(), Some(json!("OuterAck")));
            assert!(session
                .game
                .overworld
                .pending_dialogue
                .as_ref()
                .unwrap()
                .holding_open());
            for _ in 0..8 {
                session.step(A | B); // B does not release the physical held A.
                assert!(session
                    .game
                    .overworld
                    .pending_dialogue
                    .as_ref()
                    .unwrap()
                    .holding_open());
                assert_eq!(session.game.overworld.sfx_event, OverworldSfxEvent::None);
                assert!(
                    !session.sound().1,
                    "silent outer ACK must not play PressAB or restart the cry"
                );
            }
            session.step(B);
            assert_eq!(session.phase(), Some(json!("CloseWait")));
            assert!(session.game.overworld.displayed_field_dialogue().is_none());
            session.step(B);
            assert!(session
                .game
                .overworld
                .active_script_effect_label()
                .is_none());
            assert!(session.game.overworld.script_engine_idle());
            assert_eq!(session.cry_starts, 1);
            assert_eq!(std::fs::read(&session.path).unwrap(), session.bytes);
            assert!(!companion_path_for_save(&session.path).exists());
        })
        .unwrap()
        .join()
        .unwrap();
}
