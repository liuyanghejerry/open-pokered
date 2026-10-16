//! Ordinary cold SRAM/Continue/START inputs with the real muted PCM owner.
//! No screen/menu/audio setter, debug injection, or fixed sound-duration oracle.

use super::*;
use pokered_core::pokemon::stats::create_pokemon;
use pokered_data::{event_flags::EventFlag, species::Species};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);
// These only bound a stuck test; completion is read from the real sequencer.
const BOOT_WATCHDOG: usize = 6000;
const SOUND_WATCHDOG: usize = 600;

#[derive(Clone, Copy, Debug)]
struct Sound {
    id: u8,
    busy: bool,
    owns_channel: bool,
    nonzero_pcm: bool,
}

struct Session {
    game: PokemonGame,
    input: InputState,
    path: PathBuf,
    bytes: Vec<u8>,
    pcm: Vec<f32>,
}

impl Drop for Session {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("preserving START audio fixture: {}", self.path.display());
        } else {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl Session {
    fn step(&mut self, buttons: &[GbButton]) -> Sound {
        // Keep the physical held mask between frames. A held key is not
        // released/re-pressed to manufacture another confirmation edge.
        self.input.begin_frame();
        for key in GbButton::ALL {
            if buttons.contains(&key) {
                self.input.press(key);
            } else {
                self.input.release(key);
            }
        }
        self.game.update(&self.input);
        let audio = self.game.audio.as_ref().expect("real muted PCM owner");
        let (id, busy, owns_channel) = {
            let manager = audio.manager.lock().unwrap();
            let id = manager.sequencer.current_sfx_id;
            (
                id,
                manager.is_sfx_playing(),
                (4..8).any(|c| {
                    let channel = &manager.sequencer.channels[c];
                    channel.active && channel.sound_id == id
                }),
            )
        };
        // The normal Game update has already advanced the audio frame once.
        // Render samples without a second audio.update_frame clock.
        audio.render_pcm(&mut self.pcm);
        Sound {
            id,
            busy,
            owns_channel,
            nonzero_pcm: self.pcm.iter().any(|v| v.abs() > 0.00001),
        }
    }

    fn boot(empty_party: bool, has_pokedex: bool) -> Self {
        // Fixture construction finishes before PokemonGame exists.
        let mut save = SaveData::new();
        save.player_name = pokered_data::charmap::encode_string("RED").unwrap();
        save.game_data.rival_name = pokered_data::charmap::encode_string("BLUE").unwrap();
        save.game_data.player_id = 1234;
        save.game_data.position.map_id = MapId::ViridianForest as u8;
        save.game_data.position.x = 2;
        save.game_data.position.y = 19;
        save.game_data.position.x_block = 1;
        save.game_data.position.y_block = 9;
        save.game_data.last_map = MapId::ViridianCity as u8;
        save.game_data.options.text_speed = pokered_core::options_menu::TextSpeed::Fast;
        if !empty_party {
            let mut mon = create_pokemon(Species::Geodude, 21, [0x9a, 0x78]).unwrap();
            mon.ot_id = 1234;
            save.party.add(mon).unwrap();
        }
        let dex = EventFlag::EVENT_GOT_POKEDEX;
        if has_pokedex {
            save.game_data.event_flags[dex.byte_offset()] |= dex.bit_mask();
            save.game_data.pokedex.set_seen(Species::Geodude);
        }
        let bytes = export_sram(&save);
        let restored = import_sram(&bytes).unwrap();
        assert!(!restored.imported_legacy_native);
        assert_eq!(restored.party.count(), usize::from(!empty_party));
        assert_eq!(
            restored.game_data.event_flags[dex.byte_offset()] & dex.bit_mask() != 0,
            has_pokedex,
        );
        let serial = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pokered-start-button-audio-{}-{serial}.sav",
            std::process::id(),
        ));
        assert!(!companion_path_for_save(&path).exists());
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap()
            .write_all(&bytes)
            .unwrap();
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
        assert_eq!(game.state.screen, GameScreen::GameFreakSplash);
        assert!(game.state.save_summary.is_some());
        let mut session = Self {
            game,
            input: InputState::new(),
            path,
            bytes,
            pcm: vec![0.0; 1470],
        };
        for frame in 0..BOOT_WATCHDOG {
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
            session.step(if frame % 8 == 0 { &[GbButton::A] } else { &[] });
        }
        assert_eq!(session.game.state.screen, GameScreen::Overworld);
        // Neutral field time also finishes the ordinary Continue handoff.
        for _ in 0..32 {
            session.step(&[]);
        }
        assert_eq!(
            session.game.overworld.state.current_map,
            MapId::ViridianForest
        );
        assert!(session.game.overworld.script_engine_idle());
        assert!(session
            .game
            .overworld
            .active_script_effect_label()
            .is_none());
        session.step(&[GbButton::Start]);
        session.step(&[GbButton::Start]);
        assert_eq!(session.game.state.screen, GameScreen::StartMenu);
        assert_eq!(session.game.start_menu.has_pokedex(), has_pokedex);
        assert!(!session.game.start_menu.is_link_connected());
        session.finish_sound(&[]);
        assert_eq!(session.game.state.screen, GameScreen::StartMenu);
        assert_eq!(session.game.start_menu.cursor(), 0);
        session
    }

    fn finish_sound(&mut self, held: &[GbButton]) {
        for _ in 0..SOUND_WATCHDOG {
            if !self.step(held).busy {
                return;
            }
        }
        panic!("real sound did not terminate");
    }

    fn assert_press_ab(&mut self, buttons: &[GbButton], held_after: &[GbButton]) {
        let mut sound = self.step(buttons);
        assert_eq!(sound.id, SfxId::PressAB as u8);
        assert!(sound.busy && sound.owns_channel, "{sound:?}");
        let mut pcm = sound.nonzero_pcm;
        for _ in 0..SOUND_WATCHDOG {
            sound = self.step(held_after);
            if sound.busy {
                assert_eq!(sound.id, SfxId::PressAB as u8);
                assert!(sound.owns_channel, "{sound:?}");
                pcm |= sound.nonzero_pcm;
            } else {
                assert!(pcm, "PRESS_AB must reach the actual PCM renderer");
                return;
            }
        }
        panic!("PRESS_AB did not terminate; held input may have restarted it");
    }

    fn assert_fixture_unchanged(&self) {
        assert_eq!(std::fs::read(&self.path).unwrap(), self.bytes);
        assert!(!companion_path_for_save(&self.path).exists());
    }
}

#[test]
fn actual_start_menu_a_b_and_direction_combinations_play_press_ab_pcm() {
    let cases: &[(&[GbButton], bool, Option<GameScreen>)] = &[
        (&[GbButton::Up, GbButton::A], false, None),
        (&[GbButton::Up, GbButton::B], false, None),
        (
            &[GbButton::A, GbButton::B],
            false,
            Some(GameScreen::Overworld),
        ),
        (&[GbButton::A], false, Some(GameScreen::PartyScreen)),
        (&[GbButton::B], true, Some(GameScreen::Overworld)),
        (&[GbButton::A], true, Some(GameScreen::Pokedex)),
    ];
    for &(buttons, dex, ref expected_screen) in cases {
        let mut session = Session::boot(false, dex);
        session.assert_press_ab(buttons, &[]);
        if let Some(expected) = expected_screen {
            assert_eq!(&session.game.state.screen, expected, "{buttons:?}");
        }
        // Direction-priority behavior is covered by its separate core fix;
        // this sound caller must work regardless of the returned action.
        session.assert_fixture_unchanged();
    }
}

#[test]
fn actual_empty_party_start_a_uses_fresh_edges_without_held_restarts() {
    let mut session = Session::boot(true, false);
    assert_eq!(
        session.game.start_menu.current_item(),
        pokered_core::start_menu::StartMenuItem::Pokemon
    );
    session.assert_press_ab(&[GbButton::A], &[GbButton::A]);
    assert_eq!(session.game.state.screen, GameScreen::StartMenu);
    assert_eq!(session.game.save_data.party.count(), 0);
    for _ in 0..16 {
        let sound = session.step(&[GbButton::A]);
        assert!(!sound.busy, "held A must not restart PRESS_AB");
        assert!(!session.input.is_just_pressed(GbButton::A));
    }
    // A new direction while A remains held does not manufacture a fresh A.
    let sound = session.step(&[GbButton::A, GbButton::Down]);
    assert!(!sound.busy);
    assert_eq!(session.game.state.screen, GameScreen::StartMenu);
    assert_eq!(session.game.start_menu.cursor(), 1);
    let sound = session.step(&[GbButton::A, GbButton::Up]);
    assert!(!sound.busy);
    assert_eq!(session.game.start_menu.cursor(), 0);
    session.step(&[]);
    session.assert_press_ab(&[GbButton::A], &[GbButton::A]);
    assert_eq!(session.game.state.screen, GameScreen::StartMenu);
    session.assert_fixture_unchanged();
}

#[test]
fn actual_start_menu_directions_and_start_do_not_play_press_ab() {
    let mut session = Session::boot(false, false);
    let last = session.game.start_menu.item_count() - 1;
    let sound = session.step(&[GbButton::Up]);
    assert!(!sound.busy);
    assert_eq!(session.game.state.screen, GameScreen::StartMenu);
    assert_eq!(session.game.start_menu.cursor(), last);
    session.step(&[]);
    let sound = session.step(&[GbButton::Down]);
    assert!(!sound.busy);
    assert_eq!(session.game.start_menu.cursor(), 0);
    session.step(&[]);
    let sound = session.step(&[GbButton::Start]);
    assert!(!sound.busy);
    assert_eq!(session.game.state.screen, GameScreen::Overworld);
    session.assert_fixture_unchanged();
}
