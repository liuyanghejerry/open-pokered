//! Real muted App/SRAM/ordinary-restore consumers. Only the explicitly named
//! sampler component control alters an audio channel; normal cases do not.
use super::*;
use pokered_core::foreground_input::{A, B, UP};
use pokered_core::save::sram_export::export_sram;
use pokered_data::species::Species;

struct Session {
    game: PokemonGame,
    input: InputState,
    path: PathBuf,
    bytes: Vec<u8>,
}
impl Session {
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
    }
    fn phase(&self) -> Option<String> {
        self.game
            .overworld
            .active_script_effect_value()
            .filter(|value| value["effect"] == "JigglypuffSong")
            .and_then(|value| value["state"]["phase"].as_str().map(str::to_owned))
    }
    fn boot() -> Self {
        let mut save = SaveData::new();
        save.player_name = pokered_data::charmap::encode_string("RED").unwrap();
        save.game_data.player_id = 1234;
        save.game_data.position.map_id = MapId::PewterPokecenter as u8;
        save.game_data.position.x = 1;
        save.game_data.position.y = 4;
        save.game_data.position.x_block = 0;
        save.game_data.position.y_block = 2;
        save.game_data.last_map = MapId::PewterCity as u8;
        let mon = pokered_core::pokemon::stats::create_pokemon(Species::Bulbasaur, 5, [0x9a, 0x78])
            .unwrap();
        save.party.add(mon).unwrap();
        let bytes = export_sram(&save);
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("jigglypuff-song-{}-{unique}", std::process::id()));
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
        game.set_seed(42); // One preboot seed, never a seed search or runtime warp.
        assert_eq!(game.state.screen, GameScreen::GameFreakSplash);
        let mut session = Self {
            game,
            input: InputState::new(),
            path,
            bytes,
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
        for _ in 0..32 {
            session.step(0);
        }
        assert_eq!(session.game.state.screen, GameScreen::Overworld);
        assert_eq!(
            session.game.overworld.state.current_map,
            MapId::PewterPokecenter
        );
        assert_eq!(
            (
                session.game.overworld.state.player.x,
                session.game.overworld.state.player.y
            ),
            (1, 4)
        );
        assert_eq!(
            session.game.audio.as_ref().unwrap().last_music_id(),
            Some(MusicId::POKECENTER)
        );
        session
    }
    fn talk_to_song(&mut self) {
        for buttons in [UP, UP, 0, 0, A, A, 0, 0] {
            self.step(buttons);
        }
        assert!(
            self.phase().is_some(),
            "ordinary npc3 talk must enter the embedded song caller"
        );
        for _ in 0..2000 {
            if self.phase().as_deref() == Some("SongSpin") {
                break;
            }
            self.step(0);
        }
        assert_eq!(self.phase().as_deref(), Some("SongSpin"));
        assert_eq!(
            self.game.audio.as_ref().unwrap().last_music_id(),
            Some(MusicId::JIGGLYPUFF_SONG)
        );
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("preserving Jigglypuff fixture {}", self.path.display());
        } else {
            let _ = std::fs::remove_dir_all(self.path.parent().unwrap());
        }
    }
}
fn with_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn ordinary_jigglypuff_talk_waits_for_the_unmodified_real_song_and_emits_pcm() {
    with_stack(|| {
        let mut session = Session::boot();
        session.talk_to_song();
        let mut pcm = vec![0.; 1470];
        let mut audible = false;
        let mut saw_both_channels = false;
        let mut song_frames = 0;
        let cursor = serde_json::to_value(
            session
                .game
                .overworld
                .last_script_dialogue
                .as_ref()
                .unwrap(),
        )
        .unwrap();
        for _ in 0..4000 {
            let audio = session.game.audio.as_ref().unwrap();
            assert!(
                !audio.is_sfx_playing(),
                "no extra Jigglypuff cry during the song"
            );
            let playing = sample_jigglypuff_song(Some(audio));
            if playing {
                song_frames += 1;
                saw_both_channels |=
                    audio.is_music_channel_playing(0) && audio.is_music_channel_playing(1);
                audio.render_pcm(&mut pcm);
                audible |= pcm.iter().any(|sample| sample.abs() > 1e-7);
                assert_eq!(session.phase().as_deref(), Some("SongSpin"));
            }
            if session.phase().as_deref() == Some("PostSongWait") {
                assert!(!playing, "both real song channels must have ended");
                assert_eq!(
                    serde_json::to_value(
                        session
                            .game
                            .overworld
                            .last_script_dialogue
                            .as_ref()
                            .unwrap()
                    )
                    .unwrap(),
                    cursor
                );
            }
            if session.phase().is_none() && session.game.overworld.script_engine_idle() {
                assert!(song_frames > 24 && saw_both_channels && audible);
                assert_eq!(audio.last_music_id(), Some(MusicId::POKECENTER));
                assert!(session.game.overworld.displayed_field_dialogue().is_none());
                assert_eq!(std::fs::read(&session.path).unwrap(), session.bytes);
                return;
            }
            session.step(0); // No final ACK: DoNotWait still executes A-release.
        }
        panic!("actual unmodified Jigglypuff song did not return to the field");
    });
}

#[test]
fn ordinary_game_snapshot_load_into_live_map_bgm_finishes_without_replaying_the_song() {
    with_stack(|| {
        let mut source = Session::boot();
        source.talk_to_song();
        for _ in 0..7 {
            source.step(0);
        }
        let snapshot = source.game.agent_save_state().unwrap();
        let json = serde_json::to_string(&snapshot).unwrap();
        let loaded: crate::agent_state::GameSnapshot = serde_json::from_str(&json).unwrap();
        // Fresh ordinary cold App is already playing looping Pokecenter BGM.
        // Use the production JSON restore path, not OverworldSnapshot directly
        // and never a setter for the core's audio-completion bool.
        let mut receiving = Session::boot();
        let audio = receiving.game.audio.as_ref().unwrap();
        assert!(audio.is_music_channel_playing(0) || audio.is_music_channel_playing(1));
        receiving.game.agent_restore_state(&loaded).unwrap();
        assert_eq!(receiving.phase().as_deref(), Some("SongSpin"));
        for elapsed in 1..=100 {
            receiving.step(0);
            let audio = receiving.game.audio.as_ref().unwrap();
            assert_eq!(
                audio.last_music_id(),
                Some(MusicId::POKECENTER),
                "no sequencer/PCM snapshot means no song replay"
            );
            assert!(!receiving.game.overworld.jigglypuff_song_playing);
            if elapsed == 24 {
                assert_ne!(
                    receiving.phase().as_deref(),
                    Some("SongSpin"),
                    "restored looping CH1/2 BGM cannot hold the song wait"
                );
            }
            if receiving.phase().is_none() && receiving.game.overworld.script_engine_idle() {
                assert!(receiving
                    .game
                    .overworld
                    .displayed_field_dialogue()
                    .is_none());
                assert_eq!(std::fs::read(&receiving.path).unwrap(), receiving.bytes);
                return;
            }
        }
        panic!("ordinary snapshot load softlocked the Jigglypuff continuation");
    });
}

#[test]
fn sampler_component_control_observes_ch2_only_and_rejects_other_tracks() {
    // Explicit synthetic audio-component boundary, NOT a natural CH2 tail or
    // an ordinary App playthrough. Keep CH2's original live track stream and
    // terminate CH1 early to check the real sampler's second-channel branch.
    let audio = AudioOutput::new_pcm();
    audio.play_music(MusicId::JIGGLYPUFF_SONG);
    assert!(audio.is_music_channel_playing(1));
    {
        let mut manager = audio.manager.lock().unwrap();
        manager.sequencer.channels[0].active = false;
        manager.sequencer.channels[0].sound_id = 0;
    }
    assert!(!audio.is_music_channel_playing(0));
    assert!(
        sample_jigglypuff_song(Some(&audio)),
        "CH2 alone must hold the wait"
    );
    audio.play_music(MusicId::POKECENTER);
    assert!(audio.is_music_channel_playing(0) || audio.is_music_channel_playing(1));
    assert!(
        !sample_jigglypuff_song(Some(&audio)),
        "other live music is not the song"
    );
    assert!(!sample_jigglypuff_song(None));
}
