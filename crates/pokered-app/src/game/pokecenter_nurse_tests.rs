//! Cold physical SRAM + normal Continue/talk/input; no runtime state setters.
use super::*;
use pokered_core::foreground_input::{A, B, UP};
use pokered_core::save::{sram_export::export_sram, sram_import::import_sram};
use pokered_data::species::Species;
struct NurseSession {
    game: PokemonGame,
    input: InputState,
    path: PathBuf,
}
impl NurseSession {
    fn step(&mut self, keys: u8) {
        self.input.begin_frame();
        for (bit, button) in [(A, GbButton::A), (B, GbButton::B), (UP, GbButton::Up)] {
            if keys & bit != 0 {
                self.input.press(button);
            } else {
                self.input.release(button);
            }
        }
        self.game.update(&self.input);
        if let Some(audio) = self.game.audio.as_ref() {
            audio.render_pcm(&mut [0.; 1470]);
        }
    }
    fn boot(used: bool) -> Self {
        let mut save = SaveData::new();
        save.player_name = pokered_data::charmap::encode_string("RED").unwrap();
        save.game_data.player_id = 1234;
        save.game_data.position.map_id = MapId::ViridianPokecenter as u8;
        save.game_data.position.x = 3;
        save.game_data.position.y = 3;
        save.game_data.position.x_block = 1;
        save.game_data.position.y_block = 1;
        save.game_data.map_header.tileset = 6;
        save.game_data.map_header.width = 7;
        save.game_data.map_header.height = 4;
        save.game_data.last_map = MapId::ViridianCity as u8;
        save.game_data.last_blackout_map = MapId::PalletTown as u8;
        save.game_data.status_flags[4] = if used { 0x84 } else { 0x80 };
        let mut mon =
            pokered_core::pokemon::stats::create_pokemon(Species::Bulbasaur, 5, [0x9a, 0x78])
                .unwrap();
        mon.hp = 1;
        mon.pp = [0; 4];
        save.party.add(mon).unwrap();
        let bytes = export_sram(&save);
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("nurse-{}-{unique}", std::process::id()));
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
        game.set_seed(42);
        assert_eq!(game.state.screen, GameScreen::GameFreakSplash);
        let mut s = Self {
            game,
            input: InputState::new(),
            path,
        };
        for frame in 0..8000 {
            if s.game.state.screen == GameScreen::Overworld && s.game.overworld.script_engine_idle()
            {
                break;
            }
            s.step(if frame % 8 < 2 { A } else { 0 });
        }
        for _ in 0..32 {
            s.step(0);
        }
        assert_eq!(s.game.state.screen, GameScreen::Overworld);
        assert_eq!(
            s.game.overworld.state.current_map,
            MapId::ViridianPokecenter
        );
        assert_eq!(
            (
                s.game.overworld.state.player.x,
                s.game.overworld.state.player.y
            ),
            (3, 3)
        );
        s
    }
    fn menu(&mut self) -> bool {
        for _ in 0..16 {
            self.step(UP);
        }
        for _ in 0..16 {
            self.step(0);
        }
        // The normal field loop samples Joypad every other VBlank. Use
        // the same two-frame talk hold as the independent normal CLI control.
        self.step(A);
        self.step(A);
        self.step(0);
        let mut question = false;
        for frame in 0..8000 {
            if let Some(value) = self.game.overworld.active_script_effect_value() {
                question |= value["effect"] == "PokecenterNurseOffer"
                    && value["state"]["phase"] == "Question";
            }
            if self.game.overworld.choice_accepts_input() {
                return question;
            }
            let ack = frame % 2 == 0
                && self.game.overworld.dialogue_needs_button()
                && self
                    .game
                    .overworld
                    .pending_dialogue
                    .as_ref()
                    .is_some_and(|d| d.waiting_for_input());
            self.step(if ack { A } else { 0 });
        }
        panic!("ordinary nurse talk did not reach its menu: screen={:?} pos={:?} effect={:?} foreground={:?} dialogue={:?}",
            self.game.state.screen, self.game.overworld.state.player,
            self.game.overworld.active_script_effect_value(), self.game.overworld.source_foreground_error(),
            self.game.overworld.pending_dialogue.as_ref().map(|d| (d.get_display_text(), d.waiting_for_input())))
    }
}
impl Drop for NurseSession {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = std::fs::remove_dir_all(self.path.parent().unwrap());
        }
    }
}
#[test]
fn cold_sram_nurse_marks_used_on_decline_and_records_destination_before_actual_healing() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            for used in [false, true] {
                for heal in [false, true] {
                    let mut s = NurseSession::boot(used);
                    assert_eq!(s.menu(), !used);
                    assert_eq!(
                        s.game.overworld.pending_choice.as_ref().unwrap().options,
                        vec!["HEAL", "CANCEL"]
                    );
                    assert_eq!(s.game.save_data.game_data.status_flags[4], 0x86);
                    assert_eq!(
                        s.game.save_data.game_data.last_blackout_map,
                        MapId::PalletTown as u8
                    );
                    let bytes = export_sram(&s.game.save_data);
                    assert!(import_sram(&bytes).unwrap().game_data.used_pokecenter());
                    s.step(0);
                    s.step(if heal { A } else { B });
                    for _ in 0..15 {
                        s.step(0);
                    }
                    assert_eq!(
                        s.game.save_data.game_data.last_blackout_map,
                        if heal {
                            MapId::ViridianCity
                        } else {
                            MapId::PalletTown
                        } as u8
                    );
                    assert_eq!(
                        s.game.save_data.party.get(0).unwrap().hp,
                        1,
                        "SetLastBlackoutMap precedes NeedYourPokemonText, turn and Delay3"
                    );
                    assert_eq!(
                        s.game.overworld.active_script_effect_label().as_deref(),
                        Some(if heal {
                            "PrintFieldText"
                        } else {
                            "PokecenterFarewell"
                        })
                    );
                    if heal {
                        // Need/DONE must progress into HealParty/machine without
                        // any extra ACK. Advance actual muted audio naturally.
                        for _ in 0..10000 {
                            if s.game.overworld.pending_healing_machine.is_some() {
                                break;
                            }
                            s.step(0);
                        }
                        assert!(s.game.overworld.pending_healing_machine.is_some());
                        let mon = s.game.save_data.party.get(0).unwrap();
                        assert_eq!(mon.hp, mon.max_hp);
                    }
                    let mut machine = false;
                    for frame in 0..10000 {
                        machine |= s.game.overworld.pending_healing_machine.is_some();
                        if s.game.overworld.script_engine_idle()
                            && s.game.overworld.active_script_effect_label().is_none()
                        {
                            break;
                        }
                        s.step(if frame % 40 == 0 { A } else { 0 });
                    }
                    assert!(s.game.overworld.script_engine_idle());
                    assert!(s.game.overworld.active_script_effect_label().is_none());
                    assert_eq!(machine, heal);
                    let mon = s.game.save_data.party.get(0).unwrap();
                    assert_eq!(mon.hp, if heal { mon.max_hp } else { 1 });
                    assert_eq!(mon.pp.iter().any(|pp| *pp != 0), heal);
                    assert!(
                        !s.menu(),
                        "repeat talk uses canonical flag without another question"
                    );
                    assert!(!companion_path_for_save(&s.path).exists());
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
