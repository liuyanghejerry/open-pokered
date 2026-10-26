//! Cold SRAM -> CONTINUE -> ordinary gift dialogue with real audio/PCM.
use super::*;
use pokered_core::foreground_input::{A, B, DOWN, UP};
use pokered_core::save::{sram_export::export_sram, sram_import::import_sram, SaveData};
use pokered_data::{
    event_flags::EventFlag, items::ItemId, maps::MapId, moves::MoveId, species::Species,
};

#[derive(Clone, Copy)]
struct Case {
    map: MapId,
    x: u8,
    y: u8,
    flag: EventFlag,
    item: ItemId,
}
const CASES: [Case; 2] = [
    Case {
        map: MapId::CinnabarLabMetronomeRoom,
        x: 7,
        y: 3,
        flag: EventFlag::EVENT_GOT_TM35,
        item: ItemId::Tm35,
    },
    Case {
        map: MapId::SafariZoneSecretHouse,
        x: 3,
        y: 4,
        flag: EventFlag::EVENT_GOT_HM03,
        item: ItemId::Hm03,
    },
];

struct Session {
    game: PokemonGame,
    input: InputState,
    path: std::path::PathBuf,
    original_sram: Vec<u8>,
    pcm: Vec<f32>,
    case: Case,
}

impl Session {
    fn step(&mut self, buttons: u8) -> bool {
        self.input.begin_frame();
        for (bit, key) in [
            (A, GbButton::A),
            (B, GbButton::B),
            (DOWN, GbButton::Down),
            (UP, GbButton::Up),
        ] {
            if buttons & bit != 0 {
                self.input.press(key);
            } else {
                self.input.release(key);
            }
        }
        self.game.update(&self.input);
        assert_eq!(self.game.overworld.source_foreground_error(), None);
        let audio = self
            .game
            .audio
            .as_ref()
            .expect("no-device mode retains the sequencer");
        let active = {
            let manager = audio.manager.lock().unwrap();
            let id = manager.sequencer.current_sfx_id;
            manager.is_sfx_playing()
                && (4..8).any(|channel| {
                    let state = &manager.sequencer.channels[channel];
                    state.active && state.sound_id == id
                })
        };
        audio.render_pcm(&mut self.pcm);
        active && self.pcm.iter().any(|sample| sample.abs() > 1e-7)
    }

    fn boot(case: Case, full_bag: bool) -> Self {
        let mut save = SaveData::new();
        save.player_name = pokered_data::charmap::encode_string("RED").unwrap();
        save.game_data.player_id = 1234;
        save.game_data.position.map_id = case.map as u8;
        save.game_data.position.x = case.x;
        save.game_data.position.y = case.y;
        save.game_data.position.x_block = case.x / 2;
        save.game_data.position.y_block = case.y / 2;
        save.game_data.options.text_speed = pokered_core::options_menu::TextSpeed::Fast;
        if full_bag {
            for id in 1..=20 {
                save.game_data.bag.add_item(ItemId::from_id(id), 1).unwrap();
            }
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
        let original_sram = export_sram(&save);
        assert_eq!(
            import_sram(&original_sram)
                .unwrap()
                .game_data
                .position
                .map_id,
            case.map as u8
        );
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("field-reward-{}-{unique}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("fixture.sav");
        std::fs::write(&path, &original_sram).unwrap();
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
            original_sram,
            pcm: vec![0.; 1470],
            case,
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
        assert_eq!(session.game.overworld.state.current_map, case.map);
        assert_eq!(
            (
                session.game.overworld.state.player.x,
                session.game.overworld.state.player.y
            ),
            (case.x.into(), case.y.into())
        );
        session.talk();
        session
    }

    fn phase(&self, expected: &str) -> bool {
        self.game
            .overworld
            .active_script_effect_value()
            .is_some_and(|value| value["phase"] == expected)
    }

    fn received(&self) -> bool {
        self.game.overworld.unified_flags().as_bytes()[self.case.flag.byte_offset()]
            & self.case.flag.bit_mask()
            != 0
    }

    fn quantity(&self) -> u16 {
        self.game
            .save_data
            .game_data
            .bag
            .item_quantity(self.case.item)
    }

    fn talk(&mut self) {
        for _ in 0..16 {
            self.step(UP);
        }
        for _ in 0..16 {
            self.step(0);
        }
        for _ in 0..6 {
            self.step(A);
        }
        for _ in 0..6 {
            self.step(0);
        }
        assert_eq!(
            self.game.overworld.active_script_effect_label().as_deref(),
            Some("FieldRewardText")
        );
    }

    fn reach(&mut self, target: &str) {
        for _ in 0..8000 {
            if self.phase(target) {
                return;
            }
            let keys = if self.game.overworld.dialogue_needs_button() {
                B
            } else {
                0
            };
            self.step(keys);
        }
        panic!(
            "did not reach {target}: {:?}",
            self.game.overworld.active_script_effect_value()
        );
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = std::fs::remove_dir_all(self.path.parent().unwrap());
        } else {
            eprintln!("preserving gift fixture {}", self.path.display());
        }
    }
}

#[test]
fn cold_continue_gifts_wait_for_actual_fanfare_and_outer_ack_then_cannot_repeat() {
    for case in CASES {
        let mut session = Session::boot(case, false);
        session.reach("SoundWait");
        assert_eq!(session.quantity(), 1);
        assert!(!session.received());
        let mut audible = 0;
        for tick in 0..600 {
            if session.phase("OuterAck") {
                break;
            }
            if session.step(if (8..11).contains(&tick) { B | DOWN } else { 0 }) {
                audible += 1;
            }
        }
        assert!(
            audible > 10,
            "the real fanfare must render PCM through its owned SFX channel"
        );
        assert!(session.phase("OuterAck"));
        assert!(session.received());
        for _ in 0..180 {
            session.step(0);
        }
        assert!(session.game.overworld.displayed_field_dialogue().is_some());
        for _ in 0..32 {
            session.step(A);
        }
        assert!(session
            .game
            .overworld
            .pending_dialogue
            .as_ref()
            .unwrap()
            .holding_open());
        for _ in 0..32 {
            session.step(0);
        }
        assert!(session.game.overworld.displayed_field_dialogue().is_none());
        assert_eq!(session.quantity(), 1);
        let saved = import_sram(&export_sram(&session.game.build_save_data())).unwrap();
        assert_ne!(
            saved.game_data.event_flags[case.flag.byte_offset()] & case.flag.bit_mask(),
            0
        );
        assert_eq!(saved.game_data.bag.item_quantity(case.item), 1);
        session.talk();
        for _ in 0..8000 {
            assert!(
                !session.phase("SoundWait"),
                "repeat dialogue must not replay the gift fanfare"
            );
            if session.phase("OuterAck") {
                break;
            }
            session.step(if session.game.overworld.dialogue_needs_button() {
                B
            } else {
                0
            });
        }
        assert!(session.phase("OuterAck"));
        assert_eq!(session.quantity(), 1);
        session.step(B);
        for _ in 0..8 {
            session.step(0);
        }
        assert!(session.game.overworld.displayed_field_dialogue().is_none());
        assert_eq!(std::fs::read(&session.path).unwrap(), session.original_sram);
    }
}

#[test]
fn cold_continue_full_bag_never_marks_the_gift_received() {
    for case in CASES {
        let mut session = Session::boot(case, true);
        session.reach("OuterAck");
        assert!(!session.received());
        assert_eq!(session.quantity(), 0);
        assert_eq!(session.game.save_data.game_data.bag.count(), 20);
        for _ in 0..60 {
            session.step(0);
        }
        assert!(session.game.overworld.displayed_field_dialogue().is_some());
        session.step(B);
        for _ in 0..8 {
            session.step(0);
        }
        assert!(session.game.overworld.displayed_field_dialogue().is_none());
        assert!(!session.received());
        assert_eq!(std::fs::read(&session.path).unwrap(), session.original_sram);
    }
}
