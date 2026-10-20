//! Cold ordinary SRAM/CONTINUE and actual buttons with the muted PCM backend.
//! No script injection, runtime state setters or simulated sound-completion flag.
use super::*;
use dotzuki_engine::overworld::Direction;
use pokered_core::overworld::script_bridge::ScriptEffect;
use pokered_core::pokemon::stats::create_pokemon;
use pokered_core::snapshot::OverworldSnapshot;
use pokered_data::{event_flags::EventFlag, species::Species};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const USED: &str = "EVENT_USED_CELL_SEPARATOR_ON_BILL";
const MET: &str = "EVENT_MET_BILL";
const WATCHDOG: usize = 6000;

struct Session {
    game: PokemonGame,
    input: InputState,
    path: PathBuf,
    original: Vec<u8>,
    sounds: Vec<(u8, bool)>,
    previous_sound: Option<u8>,
    pcm: Vec<f32>,
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(companion_path_for_save(&self.path));
    }
}
impl Session {
    fn step(&mut self, buttons: &[GbButton]) {
        self.input.begin_frame();
        self.input.set_from_bitmask(0);
        for button in buttons {
            self.input.press(*button);
        }
        self.game.update(&self.input);
        let audio = self
            .game
            .audio
            .as_ref()
            .expect("muted output retains the sequencer");
        let (id, playing, owns_channel) = {
            let manager = audio.manager.lock().unwrap();
            let id = manager.sequencer.current_sfx_id;
            (
                id,
                manager.is_sfx_playing(),
                (4..8).any(|index| {
                    let channel = &manager.sequencer.channels[index];
                    channel.active && channel.sound_id == id
                }),
            )
        };
        if playing && self.previous_sound != Some(id) {
            self.sounds.push((id, false));
        }
        self.previous_sound = playing.then_some(id);
        audio.render_pcm(&mut self.pcm);
        if playing && owns_channel && self.pcm.iter().any(|sample| sample.abs() > 0.00001) {
            if let Some(last) = self.sounds.last_mut() {
                last.1 = true;
            }
        }
    }
    fn flag(&self, name: &str) -> bool {
        self.game
            .overworld
            .script_flags()
            .get(name)
            .copied()
            .unwrap_or(false)
    }
    fn boot(side: bool) -> Self {
        let mut save = SaveData::new();
        save.player_name = pokered_data::charmap::encode_string("RED").unwrap();
        save.game_data.rival_name = pokered_data::charmap::encode_string("BLUE").unwrap();
        save.game_data.player_id = 1234;
        save.game_data.position.map_id = MapId::BillsHouse as u8;
        save.game_data.position.x = if side { 2 } else { 1 };
        save.game_data.position.y = if side { 4 } else { 5 };
        save.game_data.player_direction = if side { 2 } else { 8 };
        save.game_data.last_map = MapId::Route25 as u8;
        save.game_data.options.text_speed = pokered_core::options_menu::TextSpeed::Fast;
        let agreed = EventFlag::EVENT_BILL_SAID_USE_CELL_SEPARATOR;
        save.game_data.event_flags[agreed.byte_offset()] |= agreed.bit_mask();
        for bit in [0x61usize, 0x62, 0x63] {
            save.game_data.toggleable_object_flags[bit / 8] |= 1 << (bit % 8);
        }
        save.party
            .add(create_pokemon(Species::Bulbasaur, 20, [0x9a, 0x78]).unwrap())
            .unwrap();
        let original = export_sram(&save);
        assert!(!import_sram(&original).unwrap().imported_legacy_native);
        let path = std::env::temp_dir().join(format!(
            "pokered-bill-separator-{}-{}.sav",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(!companion_path_for_save(&path).exists());
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap()
            .write_all(&original)
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
        assert!(game.state.save_summary.is_some());
        let mut session = Self {
            game,
            input: InputState::new(),
            path,
            original,
            sounds: Vec::new(),
            previous_sound: None,
            pcm: vec![0.0; 1470],
        };
        for frame in 0..WATCHDOG {
            if session.game.state.screen == GameScreen::Overworld
                && session.game.overworld.script_engine_idle()
                && OverworldSnapshot::capture(&session.game.overworld)
                    .active_script_effect
                    .is_none()
            {
                break;
            }
            session.step(if frame % 8 == 0 { &[GbButton::A] } else { &[] });
        }
        assert_eq!(session.game.state.screen, GameScreen::Overworld);
        assert_eq!(session.game.overworld.state.current_map, MapId::BillsHouse);
        for _ in 0..20 {
            session.step(&[]);
        }
        assert!(!session.flag(USED) && !session.flag(MET));
        // Continue restores the player; face the real front tile with input,
        // rather than assuming one of the saved direction bytes is facing.
        let direction = if side { GbButton::Left } else { GbButton::Up };
        for _ in 0..2 {
            session.step(&[direction]);
        }
        for _ in 0..2 {
            session.step(&[]);
        }
        assert_eq!(
            session.game.overworld.state.player.facing,
            if side { Direction::Left } else { Direction::Up }
        );
        assert_eq!(
            (
                session.game.overworld.state.player.x,
                session.game.overworld.state.player.y
            ),
            if side { (2, 4) } else { (1, 5) }
        );
        session.sounds.clear();
        session.previous_sound = None;
        for _ in 0..3 {
            session.step(&[GbButton::A]);
        }
        session
    }
}

#[test]
fn cold_bill_separator_plays_real_sounds_restores_music_then_shows_and_walks_bill() {
    let mut session = Session::boot(false);
    let mut saw_stopped_music = false;
    let mut saw_visible_delay = false;
    for frame in 0..WATCHDOG {
        let snapshot = OverworldSnapshot::capture(&session.game.overworld);
        if let Some(ScriptEffect::Delay { frames: 8, .. }) = snapshot.active_script_effect.as_ref()
        {
            let bill = &session.game.overworld.npc_states[1];
            assert!(bill.visible);
            assert_eq!(
                (bill.x, bill.y),
                (1, 2),
                "Bill appears inside the machine before the eight-frame delay"
            );
            assert!(session.flag(USED) && !session.flag(MET));
            saw_visible_delay = true;
        }
        let audio = session.game.audio.as_ref().unwrap();
        if audio.last_music_id().is_none() {
            saw_stopped_music = true;
        }
        if session.flag(USED) {
            assert_eq!(audio.last_music_id(), Some(MusicId::CITIES2));
            assert!(audio.is_music_channel_playing(0));
            assert!(
                !audio.is_sfx_playing(),
                "completion waits for the last item sound"
            );
        }
        if session.flag(MET) {
            break;
        }
        let read = frame % 4 == 0
            && session.game.overworld.dialogue_needs_button()
            && session
                .game
                .overworld
                .pending_dialogue
                .as_ref()
                .is_some_and(|d| d.waiting_for_input());
        session.step(if read { &[GbButton::A] } else { &[] });
    }
    assert!(
        session.flag(MET) && session.flag(USED),
        "effect {:?}; dialogue {:?}; needs_button {}; facing {:?}; sounds {:?}",
        OverworldSnapshot::capture(&session.game.overworld).active_script_effect,
        session.game.overworld.pending_dialogue,
        session.game.overworld.dialogue_needs_button(),
        session.game.overworld.state.player.facing,
        session.sounds
    );
    assert!(saw_stopped_music && saw_visible_delay);
    let actual: Vec<_> = session
        .sounds
        .iter()
        .copied()
        .filter(|(id, _)| *id != SfxId::PressAB as u8)
        .collect();
    let expected: Vec<_> = [
        SfxId::Switch,
        SfxId::Tink,
        SfxId::Shrink,
        SfxId::Tink,
        SfxId::GetItem1,
    ]
    .into_iter()
    .map(|id| (id as u8, true))
    .collect();
    assert_eq!(
        actual, expected,
        "each real cue owns an active channel and produces PCM, including both Tinks"
    );
    let bill = &session.game.overworld.npc_states[1];
    assert!(bill.visible);
    assert_eq!((bill.x, bill.y), (4, 4));
    assert!(
        !session.game.overworld.npc_states[0].visible
            && !session.game.overworld.npc_states[2].visible
    );
    assert_eq!(
        std::fs::read(&session.path).unwrap(),
        session.original,
        "the fixture is only read at boot"
    );
}

#[test]
fn cold_bill_pc_side_interaction_is_silent_and_does_not_run_separator() {
    let mut session = Session::boot(true);
    for _ in 0..300 {
        session.step(&[]);
        assert!(session.game.overworld.pending_dialogue.is_none());
        assert!(session.game.overworld.displayed_field_dialogue().is_none());
        assert!(!session.flag(USED) && !session.flag(MET));
    }
    assert!(session
        .sounds
        .iter()
        .all(|(id, _)| *id == SfxId::PressAB as u8));
    assert!(session
        .game
        .overworld
        .npc_states
        .iter()
        .all(|npc| !npc.visible));
}
