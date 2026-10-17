//! Component fixtures enter via ordinary talk, use actual audio, and exercise
//! the real App battle-entry owner. The capture test explicitly injects only
//! the settlement outcome; it is not a successful catch/playthrough witness.
use super::*;
use pokered_core::foreground_input::{A, B};
use pokered_core::overworld::Direction;
use pokered_data::species::Species;

struct Run {
    game: PokemonGame,
    input: InputState,
}
impl Run {
    fn step(&mut self, keys: u8) {
        self.input.begin_frame();
        for (bit, key) in [(A, GbButton::A), (B, GbButton::B)] {
            if keys & bit != 0 {
                self.input.press(key);
            } else {
                self.input.release(key);
            }
        }
        self.game.update(&self.input);
    }
    fn phase(&self) -> serde_json::Value {
        self.game
            .overworld
            .active_script_effect_value()
            .map(|v| v["state"]["text"]["phase"].clone())
            .unwrap_or(serde_json::Value::Null)
    }
    fn fixture(map: MapId, x: u16, y: u16, facing: Direction) -> Self {
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
        let mon =
            pokered_core::pokemon::stats::create_pokemon(Species::Bulbasaur, 80, [0x9a, 0x78])
                .unwrap();
        game.save_data.party.add(mon).unwrap();
        game.overworld = OverworldScreen::new(map, None, PokemonRedData);
        game.overworld.state.player.x = x;
        game.overworld.state.player.y = y;
        game.overworld.state.player.facing = facing;
        game.state.screen = GameScreen::Overworld;
        game.set_seed(42);
        game.audio.as_ref().unwrap().stop_all();
        let mut r = Self {
            game,
            input: InputState::new(),
        };
        r.step(0);
        r.step(0);
        r.step(A);
        r.step(0);
        r
    }
    fn to_outer_ack(&mut self, species: Species) {
        let cry = pokered_data::cries::cry_data(species).sfx;
        let mut heard = false;
        let mut starts = 0;
        let mut prev = false;
        let mut audible = false;
        for _ in 0..3000 {
            self.step(0);
            assert_eq!(self.game.state.screen, GameScreen::Overworld);
            assert!(self.game.overworld.pending_wild_encounter.is_none());
            let a = self.game.audio.as_ref().unwrap();
            let playing = {
                let m = a.manager.lock().unwrap();
                m.sequencer.current_sfx_id == cry && m.is_sfx_playing()
            };
            if playing && !prev {
                starts += 1;
                heard = true;
            }
            prev = playing;
            if playing {
                let mut pcm = [0.; 1470];
                a.render_pcm(&mut pcm);
                audible |= pcm.iter().any(|s| s.abs() > 1e-7);
            }
            if self.phase() == serde_json::json!("OuterAck") {
                break;
            }
        }
        assert!(heard && audible);
        assert_eq!(starts, 1);
        assert_eq!(self.phase(), serde_json::json!("OuterAck"));
        assert_eq!(
            self.game.audio.as_ref().unwrap().last_music_id(),
            Some(MusicId::MEET_MALE_TRAINER)
        );
    }
    fn start_battle(&mut self) {
        self.step(A);
        for _ in 0..12 {
            self.step(A);
            assert_eq!(self.game.state.screen, GameScreen::Overworld);
            assert!(self.game.overworld.pending_dialogue.is_some());
        }
        self.step(0);
        assert_eq!(self.game.state.screen, GameScreen::Overworld);
        for _ in 0..4 {
            self.step(0);
            if self.game.state.screen == GameScreen::Battle {
                break;
            }
        }
        assert_eq!(self.game.state.screen, GameScreen::Battle);
        assert!(self.game.overworld.script_awaiting_battle);
    }
}

#[test]
fn all_four_static_talks_emit_real_cry_and_meet_music_before_actual_battle_entry() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            for (map, x, y, dir, species) in [
                (
                    MapId::CeruleanCaveB1F,
                    27,
                    14,
                    Direction::Up,
                    Species::Mewtwo,
                ),
                (MapId::PowerPlant, 4, 8, Direction::Down, Species::Zapdos),
                (
                    MapId::SeafoamIslandsB4F,
                    6,
                    2,
                    Direction::Up,
                    Species::Articuno,
                ),
                (
                    MapId::VictoryRoad2F,
                    11,
                    4,
                    Direction::Down,
                    Species::Moltres,
                ),
            ] {
                let mut r = Run::fixture(map, x, y, dir);
                r.to_outer_ack(species);
                for _ in 0..20 {
                    r.step(0);
                    assert_eq!(
                        r.game.audio.as_ref().unwrap().last_music_id(),
                        Some(MusicId::MEET_MALE_TRAINER)
                    );
                    assert_eq!(r.game.state.screen, GameScreen::Overworld);
                }
                r.start_battle();
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn static_capture_result_stays_suspended_through_dex_and_naming_then_consumes_once() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            use pokered_core::battle::settlement::{BattleOutcome, BattleSettlement};
            for already_owned in [false, true] {
                for name in [false, true] {
                    let mut r = Run::fixture(MapId::PowerPlant, 4, 8, Direction::Down);
                    r.to_outer_ack(Species::Zapdos);
                    r.start_battle();
                    if already_owned {
                        r.game
                            .save_data
                            .game_data
                            .pokedex
                            .set_owned(Species::Zapdos);
                    }
                    // Explicit component seam: inject a caught settlement on the real
                    // script-owned battle. Catch probability/throw animation are not
                    // represented as verified by this test.
                    r.game.battle.captured_mon = Some(
                        pokered_core::pokemon::stats::create_pokemon(
                            Species::Zapdos,
                            50,
                            [0x9a, 0x78],
                        )
                        .unwrap(),
                    );
                    r.game.battle.settlement = Some(BattleSettlement {
                        outcome: BattleOutcome::Captured,
                        money_gained: 0,
                        money_lost: 0,
                        payday_bonus: 0,
                        exp_entries: vec![],
                        level_ups: vec![],
                        evolutions: vec![],
                    });
                    r.game.handle_transition(GameScreen::Overworld);
                    assert!(r.game.post_capture_active());
                    assert!(r.game.overworld.script_awaiting_battle);
                    assert!(!r
                        .game
                        .overworld
                        .unified_flags()
                        .get_flag("EVENT_BEAT_ZAPDOS"));
                    for frame in 0..4000 {
                        assert!(!r
                            .game
                            .overworld
                            .unified_flags()
                            .get_flag("EVENT_BEAT_ZAPDOS"));
                        if r.game.post_capture_choice().is_some() {
                            break;
                        }
                        r.step(if frame % 30 == 0 { A } else { 0 });
                    }
                    assert!(r.game.post_capture_choice().is_some());
                    // Existing snapshot API rejects capture movies; do not pretend
                    // a partial field snapshot can resume the unsaved modal state.
                    assert!(r.game.agent_save_state().is_err());
                    r.step(0);
                    r.step(if name { A } else { B });
                    if name {
                        for _ in 0..30 {
                            r.step(0);
                            assert!(!r
                                .game
                                .overworld
                                .unified_flags()
                                .get_flag("EVENT_BEAT_ZAPDOS"));
                        }
                        assert_eq!(
                            r.game.post_capture_flow.as_ref().unwrap().phase,
                            post_capture::CapturePhase::Naming
                        );
                        r.step(A); // Select the first naming-grid letter, A.
                        r.step(0);
                        let mut start = InputState::new();
                        start.press(GbButton::Start);
                        r.game.update(&start);
                    }
                    for frame in 0..3000 {
                        if !r.game.post_capture_active() {
                            break;
                        }
                        assert!(!r
                            .game
                            .overworld
                            .unified_flags()
                            .get_flag("EVENT_BEAT_ZAPDOS"));
                        r.step(if frame % 30 == 0 { A } else { 0 });
                    }
                    assert!(!r.game.post_capture_active());
                    assert!(!r.game.overworld.script_awaiting_battle);
                    assert!(r
                        .game
                        .overworld
                        .unified_flags()
                        .get_flag("EVENT_BEAT_ZAPDOS"));
                    assert!(
                        !r.game
                            .overworld
                            .npc_states
                            .iter()
                            .find(|n| n.text_id == 9)
                            .unwrap()
                            .visible
                    );
                    assert_eq!(r.game.save_data.party.count(), 2);
                    if name { assert_eq!(pokered_data::charmap::decode_string(&r.game.save_data.party.get(1).unwrap().nickname), "A"); }
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
