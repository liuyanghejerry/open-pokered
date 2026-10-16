//! Normal Game caller coverage: physical preboot SRAM, cold boot/CONTINUE,
//! front-tile interaction, and ordinary buttons with the real muted audio owner.
//! No script handler injection, state setter, manual sound flag or cue timer.

use super::*;
use pokered_core::overworld::script_bridge::ScriptEffect;
use pokered_core::pokemon::stats::create_pokemon;
use pokered_core::snapshot::OverworldSnapshot;
use pokered_data::event_flags::EventFlag;
use pokered_data::script_command::FieldTextPrompt;
use pokered_data::species::Species;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);
const GATE: &str = "EVENT_CINNABAR_GYM_GATE1_UNLOCKED";
// These are stuck-test bounds, never the implementation or expected SFX duration.
const WATCHDOG: usize = 6000;

struct Fixture {
    path: PathBuf,
    bytes: Vec<u8>,
}

impl Fixture {
    fn new(open: bool) -> Self {
        // This is the only state construction: it finishes before Game exists.
        // Gate1 is either still locked or already opened by a previous answer;
        // its trainer is unbeaten in both reachable cases. Other flags are zero.
        let mut save = SaveData::new();
        save.player_name = pokered_data::charmap::encode_string("RED").unwrap();
        save.game_data.rival_name = pokered_data::charmap::encode_string("BLUE").unwrap();
        save.game_data.player_id = 1234;
        save.game_data.position.map_id = MapId::CinnabarGym as u8;
        save.game_data.position.x = 15;
        save.game_data.position.y = 8;
        save.game_data.position.x_block = 7;
        save.game_data.position.y_block = 4;
        save.game_data.last_map = MapId::CinnabarIsland as u8;
        save.game_data.options.text_speed = pokered_core::options_menu::TextSpeed::Fast;
        let gate = EventFlag::EVENT_CINNABAR_GYM_GATE1_UNLOCKED;
        if open {
            save.game_data.event_flags[gate.byte_offset()] |= gate.bit_mask();
        }
        let mut mon = create_pokemon(Species::Bulbasaur, 30, [0x9a, 0x78]).unwrap();
        mon.ot_id = 1234;
        save.party.add(mon).unwrap();
        // Empty bag; no item/battle shortcuts are used by this scene.
        let bytes = export_sram(&save);
        let restored = import_sram(&bytes).unwrap();
        assert!(
            !restored.imported_legacy_native,
            "fixture must not enable legacy sidecar fallback"
        );
        assert_eq!(
            (
                restored.game_data.position.map_id,
                restored.game_data.position.x,
                restored.game_data.position.y
            ),
            (MapId::CinnabarGym as u8, 15, 8)
        );
        assert_eq!(
            restored.game_data.event_flags[gate.byte_offset()] & gate.bit_mask() != 0,
            open
        );
        let serial = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pokered-cinnabar-audio-{}-{serial}.sav",
            std::process::id(),
        ));
        assert!(
            !companion_path_for_save(&path).exists(),
            "fixture must have no pre-existing companion flags"
        );
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap()
            .write_all(&bytes)
            .unwrap();
        Self { path, bytes }
    }

    fn assert_unchanged(&self) {
        assert_eq!(std::fs::read(&self.path).unwrap(), self.bytes);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("preserving failed quiz fixture: {}", self.path.display());
            return;
        }
        // Remove only this test's exclusively created SRAM file.
        let _ = std::fs::remove_file(&self.path);
    }
}

struct Session {
    game: PokemonGame,
    input: InputState,
    fixture: Fixture,
    sfx_starts: Vec<u8>,
    previous_sfx: Option<u8>,
    pcm_sfx_ids: Vec<u8>,
    pcm: Vec<f32>,
}

impl Session {
    fn step(&mut self, buttons: &[GbButton]) {
        self.input.begin_frame();
        self.input.set_from_bitmask(0);
        for button in buttons {
            self.input.press(*button);
        }
        self.game.update(&self.input);
        let (id, playing, owns_channel) = {
            let audio = self
                .game
                .audio
                .as_ref()
                .expect("--no-audio keeps a logical PCM owner");
            let manager = audio.manager.lock().unwrap();
            let id = manager.sequencer.current_sfx_id;
            (
                id,
                manager.is_sfx_playing(),
                (4..8).any(|channel| {
                    let channel = &manager.sequencer.channels[channel];
                    channel.active && channel.sound_id == id
                }),
            )
        };
        if playing && self.previous_sfx != Some(id) {
            self.sfx_starts.push(id);
        }
        self.previous_sfx = playing.then_some(id);
        // Render the real manager's PCM. Music remains normal; sound identity
        // is proved by its actual active SFX channel, not PCM amplitude alone.
        self.game.audio.as_ref().unwrap().render_pcm(&mut self.pcm);
        if playing
            && owns_channel
            && self.pcm.iter().any(|sample| sample.abs() > 0.00001)
            && !self.pcm_sfx_ids.contains(&id)
        {
            self.pcm_sfx_ids.push(id);
        }
    }

    fn sound(&self) -> (u8, bool) {
        let manager = self.game.audio.as_ref().unwrap().manager.lock().unwrap();
        (manager.sequencer.current_sfx_id, manager.is_sfx_playing())
    }

    fn effect(&self) -> Option<ScriptEffect> {
        // An immutable observation copy only: never restore or resume Game.
        OverworldSnapshot::capture(&self.game.overworld).active_script_effect
    }

    fn gate_open(&self) -> bool {
        self.game
            .overworld
            .script_flags()
            .get(GATE)
            .copied()
            .unwrap_or(false)
    }

    fn visible(&self) -> serde_json::Value {
        let value = serde_json::to_value(self.game.overworld.displayed_field_dialogue()).unwrap();
        assert!(
            !value.is_null(),
            "the caller still owns an open field window"
        );
        value
    }

    fn boot_at_machine(open: bool) -> Self {
        let fixture = Fixture::new(open);
        let game = PokemonGame::new_with_options(
            GameVersion::Red,
            Some(fixture.path.clone()),
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
            fixture,
            sfx_starts: Vec::new(),
            previous_sfx: None,
            pcm_sfx_ids: Vec::new(),
            pcm: vec![0.0; 1470],
        };
        // Normal splash/title/menu/CONTINUE inputs; no skip-intro or screen setter.
        for frame in 0..WATCHDOG {
            if session.game.state.screen == GameScreen::Overworld
                && session.game.overworld.script_engine_idle()
                && session.effect().is_none()
            {
                break;
            }
            session.step(if frame % 8 == 0 { &[GbButton::A] } else { &[] });
        }
        assert_eq!(session.game.state.screen, GameScreen::Overworld);
        assert_eq!(session.game.overworld.state.current_map, MapId::CinnabarGym);
        assert!(session.game.overworld.script_engine_idle() && session.effect().is_none());
        assert_eq!(
            (
                session.game.overworld.state.player.x,
                session.game.overworld.state.player.y
            ),
            (15, 8)
        );
        assert_eq!(session.gate_open(), open);
        // Continue directly runs @load, then the first ordinary field update
        // dispatches its OnEnter hook. Allow that dispatch, all six gate
        // effects, and the field-loop wait to finish before the unchanged UP2.
        for _ in 0..8 {
            session.step(&[]);
        }
        assert!(session.game.overworld.script_engine_idle() && session.effect().is_none());
        assert!(session.game.overworld.pending_dialogue.is_none());
        assert!(session.game.overworld.pending_choice.is_none());
        #[cfg(feature = "debug-server")]
        assert!(session.game.debug_condition_met("control_ready"));
        // UP faces the adjacent solid machine; A interacts with its real tile.
        session.step(&[GbButton::Up]);
        session.step(&[GbButton::Up]);
        session.step(&[]);
        session.step(&[]);
        assert_eq!(
            (
                session.game.overworld.state.player.x,
                session.game.overworld.state.player.y
            ),
            (15, 8)
        );
        assert_eq!(
            session.game.overworld.state.player.facing,
            pokered_core::overworld::Direction::Up
        );
        session.sfx_starts.clear();
        session.pcm_sfx_ids.clear();
        session.step(&[GbButton::A]);
        session.step(&[GbButton::A]);
        session.step(&[]);
        session.step(&[]);
        assert!(matches!(
            session.effect(),
            Some(ScriptEffect::PrintFieldText {
                prompt: FieldTextPrompt::Protected,
                leading_sound: None,
                ..
            })
        ));
        session
    }

    fn last_page(&mut self, prompt: FieldTextPrompt) {
        for frame in 0..WATCHDOG {
            if matches!(self.effect(), Some(ScriptEffect::PrintFieldText {
                prompt: actual, leading_sound: None, ..
            }) if actual == prompt)
                && self
                    .game
                    .overworld
                    .pending_dialogue
                    .as_ref()
                    .is_some_and(|dialogue| dialogue.waiting_for_input() && dialogue.is_last_page())
            {
                return;
            }
            let next_page = frame % 4 == 0
                && self
                    .game
                    .overworld
                    .pending_dialogue
                    .as_ref()
                    .is_some_and(|dialogue| {
                        dialogue.waiting_for_input() && dialogue.has_more_pages()
                    });
            self.step(if next_page { &[GbButton::A] } else { &[] });
        }
        panic!(
            "normal reading did not reach {prompt:?}: {:?}",
            self.effect()
        );
    }

    fn protected_ack(&mut self, button: GbButton) {
        assert!(matches!(
            self.effect(),
            Some(ScriptEffect::PrintFieldText {
                prompt: FieldTextPrompt::Protected,
                prompt_frames_remaining: 3,
                prompt_last_buttons: Some(0),
                ..
            })
        ));
        for remaining in [2, 1] {
            self.step(&[button]);
            assert!(matches!(self.effect(), Some(ScriptEffect::PrintFieldText {
                prompt: FieldTextPrompt::Protected, prompt_frames_remaining, ..
            }) if prompt_frames_remaining == remaining));
            assert_eq!(self.game.overworld.sfx_event, OverworldSfxEvent::None);
        }
        self.step(&[button]);
        assert_eq!(
            self.game.overworld.sfx_event,
            OverworldSfxEvent::TextAdvance
        );
        assert_eq!(self.sound(), (SfxId::PressAB as u8, true));
        assert!(self.game.overworld.retain_field_text);
        assert!(self.game.overworld.pending_dialogue.is_none());
        assert!(self.game.overworld.last_script_dialogue.is_some());
    }

    fn question(&mut self, intro_button: GbButton) {
        self.last_page(FieldTextPrompt::Protected);
        self.protected_ack(intro_button);
        assert!(matches!(
            self.effect(),
            Some(ScriptEffect::PrintFieldText {
                prompt: FieldTextPrompt::Done,
                ..
            })
        ));
        self.step(&[intro_button]);
        assert!(
            self.game.overworld.pending_dialogue.is_some(),
            "inner PROMPT does not wait for A release before question"
        );
        // Actual machine1 question ends DONE. It needs no final-page A/B.
        for _ in 0..WATCHDOG {
            self.step(&[]);
            if self.game.overworld.pending_choice.is_some() {
                break;
            }
        }
        assert!(self.game.overworld.pending_choice.is_some());
        assert!(self.game.overworld.retain_field_text);
    }

    fn correct_text(&mut self) {
        let question = self.visible();
        self.step(&[GbButton::A]); // YES: the actual selected option, not a VM result injection.
        assert_eq!(
            self.game.overworld.sfx_event,
            OverworldSfxEvent::TextAdvance
        );
        assert_eq!(self.sound(), (SfxId::PressAB as u8, true));
        assert_eq!(self.visible(), question);
        assert!(matches!(self.effect(), Some(ScriptEffect::PrintFieldText {
            leading_sound: Some(sound), leading_sound_started: false, ..
        }) if sound == "SFX_GET_ITEM_1"));
        self.step(&[]);
        assert!(self.game.overworld.last_script_dialogue.is_none());
        assert_eq!(
            self.game
                .overworld
                .pending_dialogue
                .as_ref()
                .unwrap()
                .get_display_text(),
            Some((String::new(), String::new()))
        );
        for _ in 0..3 {
            self.step(&[GbButton::A, GbButton::B]);
            assert_eq!(
                self.game
                    .overworld
                    .pending_dialogue
                    .as_ref()
                    .unwrap()
                    .char_index(),
                0
            );
            assert!(matches!(
                self.effect(),
                Some(ScriptEffect::PrintFieldText {
                    leading_sound_started: false,
                    ..
                })
            ));
        }
        self.step(&[]);
        assert_eq!(self.sound(), (SfxId::GetItem1 as u8, true));
        let mut waited = 0;
        for frame in 0..WATCHDOG {
            if !self.sound().1 {
                break;
            }
            assert_eq!(self.sound().0, SfxId::GetItem1 as u8);
            assert_eq!(
                self.game
                    .overworld
                    .pending_dialogue
                    .as_ref()
                    .unwrap()
                    .char_index(),
                0
            );
            assert_eq!(
                self.game
                    .overworld
                    .pending_dialogue
                    .as_ref()
                    .unwrap()
                    .get_display_text(),
                Some((String::new(), String::new()))
            );
            assert!(self.game.overworld.retain_field_text);
            self.step(if frame % 2 == 0 {
                &[GbButton::A, GbButton::B]
            } else {
                &[]
            });
            waited += 1;
        }
        assert!(
            waited > 0 && !self.sound().1,
            "real GET_ITEM_1 must both play and finish"
        );
        assert!(self.pcm_sfx_ids.contains(&(SfxId::GetItem1 as u8)));
        assert_eq!(
            self.game
                .overworld
                .pending_dialogue
                .as_ref()
                .unwrap()
                .char_index(),
            1,
            "first glyph follows the real sequencer becoming idle"
        );
        assert!(matches!(
            self.effect(),
            Some(ScriptEffect::PrintFieldText {
                leading_sound: None,
                leading_sound_started: true,
                ..
            })
        ));
        self.last_page(FieldTextPrompt::Button);
        self.step(&[]);
    }
}

#[test]
fn muted_quiz_correct_uses_real_get_item_press_ab_and_door_sounds_then_outer_a_release() {
    for button in [GbButton::A, GbButton::B] {
        let mut session = Session::boot_at_machine(false);
        session.question(if button == GbButton::A {
            GbButton::B
        } else {
            GbButton::A
        });
        session.correct_text();
        session.step(&[button]); // TX_PROMPT_BUTTON has no additional protected3.
        assert_eq!(
            session.game.overworld.sfx_event,
            OverworldSfxEvent::TextAdvance
        );
        assert_eq!(session.sound(), (SfxId::PressAB as u8, true));
        assert!(matches!(session.effect(), Some(ScriptEffect::WaitSound)));
        let result = session.visible();
        let mut press_waited = 0;
        let mut door_waited = 0;
        for _ in 0..WATCHDOG {
            if matches!(session.effect(), Some(ScriptEffect::FinishFieldText)) {
                break;
            }
            let (id, playing) = session.sound();
            if playing && id == SfxId::PressAB as u8 {
                press_waited += 1;
                assert!(matches!(session.effect(), Some(ScriptEffect::WaitSound)));
                assert!(!session.gate_open());
            }
            if playing && id == SfxId::GoInside as u8 {
                door_waited += 1;
                assert!(matches!(session.effect(), Some(ScriptEffect::WaitSound)));
                assert!(!session.gate_open());
            }
            assert_eq!(session.visible(), result);
            session.step(&[GbButton::A]); // ignored by sound waits; A still holds outer caller.
        }
        assert!(press_waited > 0 && door_waited > 0);
        assert!(session.pcm_sfx_ids.contains(&(SfxId::PressAB as u8)));
        assert!(session.pcm_sfx_ids.contains(&(SfxId::GoInside as u8)));
        assert!(matches!(
            session.effect(),
            Some(ScriptEffect::FinishFieldText)
        ));
        assert!(session.gate_open());
        for _ in 0..11 {
            session.step(&[GbButton::A]);
            assert!(matches!(
                session.effect(),
                Some(ScriptEffect::FinishFieldText)
            ));
            assert_eq!(session.visible(), result);
        }
        session.step(&[]);
        assert!(session.game.overworld.script_engine_idle() && session.effect().is_none());
        assert!(!session.game.overworld.retain_field_text);
        assert!(session.game.overworld.displayed_field_dialogue().is_none());
        assert_eq!(
            session
                .sfx_starts
                .iter()
                .filter(|&&id| id == SfxId::GetItem1 as u8)
                .count(),
            1
        );
        assert_eq!(
            session
                .sfx_starts
                .iter()
                .filter(|&&id| id == SfxId::GoInside as u8)
                .count(),
            1
        );
        session.fixture.assert_unchanged();
        println!("quiz correct {button:?}: actual GET1/PressAB/GoInside gates passed; PRESS={press_waited}, door={door_waited}");
    }
}

#[test]
fn muted_quiz_wrong_keeps_question_during_real_denied_then_one_protected_a_or_b_prompt() {
    for button in [GbButton::A, GbButton::B] {
        let mut session = Session::boot_at_machine(false);
        session.question(GbButton::A);
        let question = session.visible();
        session.step(&[GbButton::B]); // Ordinary NO; the common choice supplies its sound.
        assert_eq!(
            session.game.overworld.sfx_event,
            OverworldSfxEvent::TextAdvance
        );
        assert_eq!(session.sound(), (SfxId::PressAB as u8, true));
        assert!(matches!(session.effect(), Some(ScriptEffect::WaitSound)));
        for _ in 0..WATCHDOG {
            if session.sound() == (SfxId::Denied as u8, true) {
                break;
            }
            assert_eq!(session.visible(), question);
            session.step(&[]);
        }
        assert_eq!(session.sound(), (SfxId::Denied as u8, true));
        let mut denied_waited = 0;
        for frame in 0..WATCHDOG {
            if !session.sound().1 {
                break;
            }
            assert_eq!(session.sound().0, SfxId::Denied as u8);
            assert_eq!(session.visible(), question);
            assert!(session.game.overworld.pending_dialogue.is_none());
            assert!(matches!(session.effect(), Some(ScriptEffect::WaitSound)));
            session.step(if frame % 2 == 0 {
                &[GbButton::A, GbButton::B]
            } else {
                &[]
            });
            denied_waited += 1;
        }
        assert!(denied_waited > 0 && !session.sound().1);
        assert!(session.pcm_sfx_ids.contains(&(SfxId::Denied as u8)));
        assert!(matches!(
            session.effect(),
            Some(ScriptEffect::PrintFieldText {
                prompt: FieldTextPrompt::Protected,
                leading_sound: None,
                ..
            })
        ));
        assert!(session.game.overworld.pending_dialogue.is_none());
        session.step(&[]);
        assert!(session.game.overworld.last_script_dialogue.is_none());
        assert_eq!(
            session
                .game
                .overworld
                .pending_dialogue
                .as_ref()
                .unwrap()
                .char_index(),
            0
        );
        for _ in 0..3 {
            session.step(&[]);
            assert_eq!(
                session
                    .game
                    .overworld
                    .pending_dialogue
                    .as_ref()
                    .unwrap()
                    .char_index(),
                0
            );
        }
        session.step(&[]);
        assert_eq!(
            session
                .game
                .overworld
                .pending_dialogue
                .as_ref()
                .unwrap()
                .char_index(),
            1
        );
        session.last_page(FieldTextPrompt::Protected);
        session.protected_ack(button);
        assert!(matches!(
            session.effect(),
            Some(ScriptEffect::FinishFieldText)
        ));
        let wrong = session.visible();
        for _ in 0..7 {
            session.step(&[GbButton::A]);
            assert!(matches!(
                session.effect(),
                Some(ScriptEffect::FinishFieldText)
            ));
            assert_eq!(session.visible(), wrong);
        }
        session.step(&[GbButton::B]); // Releasing A closes; no second confirmation is consumed.
        assert_eq!(session.game.overworld.sfx_event, OverworldSfxEvent::None);
        assert!(!session.game.overworld.retain_field_text);
        assert!(session.game.overworld.displayed_field_dialogue().is_none());
        assert!(!session.gate_open());
        // Stop here: trainer approach/prebattle text and shared menu15 are
        // separate outstanding fixes, and are not claimed covered by this test.
        session.fixture.assert_unchanged();
        println!("quiz wrong {button:?}: actual Denied gate={denied_waited}; one protected prompt + release passed");
    }
}

#[test]
fn muted_quiz_already_open_still_plays_get_item_and_accepts_b_without_replaying_door() {
    let mut session = Session::boot_at_machine(true);
    session.question(GbButton::B);
    session.correct_text();
    session.step(&[GbButton::B]);
    assert_eq!(
        session.game.overworld.sfx_event,
        OverworldSfxEvent::TextAdvance
    );
    assert_eq!(session.sound(), (SfxId::PressAB as u8, true));
    assert!(matches!(
        session.effect(),
        Some(ScriptEffect::ReplaceTileBlock { .. })
    ));
    assert!(session.game.overworld.retain_field_text);
    session.step(&[GbButton::B]);
    assert!(matches!(
        session.effect(),
        Some(ScriptEffect::FinishFieldText)
    ));
    session.step(&[GbButton::B]);
    assert!(session.game.overworld.script_engine_idle() && session.effect().is_none());
    assert!(session.game.overworld.displayed_field_dialogue().is_none());
    assert!(!session.game.overworld.retain_field_text);
    assert_eq!(
        session
            .sfx_starts
            .iter()
            .filter(|&&id| id == SfxId::GetItem1 as u8)
            .count(),
        1
    );
    assert!(!session.sfx_starts.contains(&(SfxId::GoInside as u8)));
    session.fixture.assert_unchanged();
}

#[test]
fn muted_choice_confirmation_starts_real_press_ab_once_while_held() {
    for (button, following) in [
        (GbButton::A, SfxId::GetItem1),
        (GbButton::B, SfxId::Denied),
    ] {
        let mut session = Session::boot_at_machine(false);
        session.question(GbButton::A);
        let starts_before = session.sfx_starts.len();
        session.step(&[button]);
        assert_eq!(
            session.game.overworld.sfx_event,
            OverworldSfxEvent::TextAdvance
        );
        assert_eq!(session.sound(), (SfxId::PressAB as u8, true));

        // Keep the same physical button down while the real PCM owner runs
        // the existing GET_ITEM or DENIED path. This bound is a stuck-test
        // watchdog, not a replacement cue duration or extra neutral padding.
        let mut following_started = false;
        for _ in 0..WATCHDOG {
            let (id, playing) = session.sound();
            following_started |= id == following as u8 && playing;
            if following_started && !playing {
                break;
            }
            session.step(&[button]);
            assert_eq!(session.game.overworld.sfx_event, OverworldSfxEvent::None);
        }
        assert!(following_started && !session.sound().1);
        assert!(session.pcm_sfx_ids.contains(&(following as u8)));
        assert_eq!(
            session.sfx_starts[starts_before..]
                .iter()
                .filter(|&&id| id == SfxId::PressAB as u8)
                .count(),
            1
        );
        assert!(!session.gate_open());
        session.fixture.assert_unchanged();
    }
}
