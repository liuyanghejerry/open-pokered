//! Cold physical SRAM/CONTINUE and actual PCM sequencer; no runtime state/audio setters.
use super::*;
use pokered_core::{
    foreground_input::{A, B, DOWN, UP},
    pc_screen::{PcPhase, PcScreen},
};
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
    }
    fn pc(&self) -> &PcScreen {
        self.game.pc_screen.as_ref().expect("normal PC entry")
    }
    fn species(&self) -> Species {
        self.pc().league_hof_mon().unwrap().1.species
    }
    fn sound(&self) -> (u8, bool) {
        let audio = self.game.audio.as_ref().unwrap().manager.lock().unwrap();
        (audio.sequencer.current_sfx_id, audio.is_sfx_playing())
    }
    fn boot() -> Self {
        Self::boot_bytes(include_bytes!("../../../../docs/audits/hof-pc-cry/fixture.sav").to_vec())
    }
    fn boot_bytes(bytes: Vec<u8>) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("hof-pc-cry-{}-{unique}", std::process::id()));
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
        assert_eq!(s.game.overworld.state.current_map, MapId::PewterPokecenter);
        for _ in 0..32 {
            s.step(0);
        }
        for keys in [UP, UP, UP, UP, 0, 0, 0, 0, A, A, A, A, 0, 0] {
            s.step(keys);
        }
        for frame in 0..2000 {
            if s.game
                .pc_screen
                .as_ref()
                .is_some_and(|pc| pc.phase() == PcPhase::LeagueHoF)
            {
                break;
            }
            let keys = if frame % 8 != 0 {
                0
            } else {
                match s.game.pc_screen.as_ref().map(|pc| pc.phase()) {
                    Some(PcPhase::MainMenu) if s.pc().main_menu().cursor() < 3 => DOWN,
                    Some(PcPhase::MainMenu | PcPhase::Message) => A,
                    _ => 0,
                }
            };
            s.step(keys);
        }
        assert_eq!(s.pc().phase(), PcPhase::LeagueHoF);
        assert!(s.pc().waiting_for_sound() && s.sound().1);
        let mut pcm = [0.; 1470];
        s.game.audio.as_ref().unwrap().render_pcm(&mut pcm);
        assert!(
            pcm.iter().any(|sample| sample.abs() > 1e-7),
            "actual cry PCM"
        );
        s
    }
    fn unchanged_save(&self) {
        assert_eq!(std::fs::read(&self.path).unwrap(), self.bytes);
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
fn cold_hof_pc_cry_ignores_short_a_b_then_allows_the_next_mon() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            for key in [A, B] {
                let mut s = Session::boot();
                s.step(0);
                let sound = s.sound();
                s.step(key);
                assert_eq!(s.species(), Species::Zapdos);
                assert_eq!(s.sound(), sound);
                s.step(0);
                for _ in 0..256 {
                    if !s.pc().waiting_for_sound() {
                        break;
                    }
                    s.step(0);
                    assert_eq!(s.species(), Species::Zapdos);
                }
                assert!(!s.pc().waiting_for_sound());
                assert!(!s.sound().1);
                s.step(A);
                assert_eq!(s.species(), Species::Lapras);
                assert!(s.pc().waiting_for_sound() && s.sound().1);
                assert_ne!(s.sound().0, sound.0);
                s.unchanged_save();
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn cold_hof_pc_retains_sample_history_during_the_actual_cry() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let mut s = Session::boot();
            s.step(0); // The raw release during PlayCry is not a foreground poll.
            for _ in 0..100 {
                s.step(A);
            }
            assert_eq!(s.species(), Species::Zapdos);
            assert!(!s.pc().waiting_for_sound());
            s.step(0);
            s.step(A);
            assert_eq!(s.species(), Species::Lapras);
            s.unchanged_save();

            let mut s = Session::boot();
            for _ in 0..256 {
                s.step(B);
                if s.pc().phase() == PcPhase::MainMenu {
                    break;
                }
                assert_eq!(s.species(), Species::Zapdos);
                assert!(s.pc().waiting_for_sound());
            }
            assert_eq!(s.pc().phase(), PcPhase::MainMenu);
            assert!(!s.sound().1);
            s.unchanged_save();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn cold_full_hof_pc_visits_all_three_hundred_records_with_actual_cries() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            use pokered_core::save::{
                hall_of_fame::{HofMon, HofTeam},
                sram_export::export_sram,
                sram_import::import_sram,
            };
            let mut save = import_sram(include_bytes!(
                "../../../../docs/audits/hof-pc-cry/fixture.sav"
            ))
            .unwrap();
            save.hall_of_fame.clear();
            // Same 50-team/6-member capacity as the GBA stress fixture. Only the
            // offline SRAM is constructed; the viewer and sound run normally.
            for team_no in 0..50 {
                let mut team = HofTeam::new();
                for n in 0..6 {
                    team.add_mon(HofMon::new(
                        ((team_no * 6 + n) % 151 + 1) as u8,
                        80,
                        &[0x80; 10],
                    ));
                }
                save.hall_of_fame.push_team(team);
            }
            let expected: Vec<_> = save
                .hall_of_fame
                .iter()
                .flat_map(|team| {
                    team.mons()
                        .iter()
                        .map(|mon| Species::from_index_id(mon.species))
                })
                .collect();
            let mut s = Session::boot_bytes(export_sram(&save));
            let mut visited = 0;
            let mut cooldown = 0;
            let mut elapsed = 0;
            for frame in 0..18000 {
                elapsed = frame;
                if s.pc().phase() == PcPhase::MainMenu {
                    break;
                }
                assert_eq!(s.pc().phase(), PcPhase::LeagueHoF);
                if cooldown > 0 {
                    cooldown -= 1;
                }
                let mut keys = 0;
                if !s.pc().waiting_for_sound() && cooldown == 0 {
                    assert_eq!(s.pc().league_hof_progress(), (visited / 6, 50));
                    assert_eq!(s.species(), expected[visited]);
                    visited += 1;
                    keys = A;
                    cooldown = 16;
                }
                s.step(keys);
            }
            assert_eq!(visited, 300, "completed full HoF store in {elapsed} frames");
            assert_eq!(s.pc().phase(), PcPhase::MainMenu);
            s.unchanged_save();
            eprintln!("full HoF: 300 records, {elapsed} frames");
        })
        .unwrap()
        .join()
        .unwrap();
}
