//! Physical preboot SRAM -> ordinary Continue/talk/FIGHT -> actual victory.
//! No battle/phase/outcome setter or script injection; PCM is the normal muted
//! Game audio owner. The companion flags file is never used.
use super::*;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{event_flags::EventFlag, moves::MoveId, species::Species};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
const LIMIT: usize = 12000; // stuck-test bound, never a sound-duration oracle
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
            eprintln!("preserving victory fixture: {}", self.path.display());
        } else {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
impl Session {
    fn step(&mut self, buttons: &[GbButton]) -> (u8, bool, bool) {
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
            .expect("--no-audio retains real logical PCM");
        let (id, busy, channel) = {
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
        audio.render_pcm(&mut self.pcm);
        (
            id,
            busy,
            channel && self.pcm.iter().any(|v| v.abs() > 0.00001),
        )
    }
    fn flag(&self, name: &str) -> bool {
        self.game
            .overworld
            .script_flags()
            .get(name)
            .copied()
            .unwrap_or(false)
    }
    fn boot(trainer: usize, open: bool) -> Self {
        // All assistance is a valid physical save constructed before Game.
        let mut save = SaveData::new();
        save.player_name = pokered_data::charmap::encode_string("RED").unwrap();
        save.game_data.rival_name = pokered_data::charmap::encode_string("BLUE").unwrap();
        save.game_data.player_id = 1234;
        save.game_data.position.map_id = MapId::CinnabarGym as u8;
        let y = if trainer == 0 { 3 } else { 9 };
        save.game_data.position.x = 17;
        save.game_data.position.y = y;
        save.game_data.position.x_block = 8;
        save.game_data.position.y_block = y / 2;
        save.game_data.last_map = MapId::CinnabarIsland as u8;
        save.game_data.options.text_speed = pokered_core::options_menu::TextSpeed::Fast;
        if open {
            let flag = EventFlag::EVENT_CINNABAR_GYM_GATE1_UNLOCKED;
            save.game_data.event_flags[flag.byte_offset()] |= flag.bit_mask();
        }
        let mut mon = create_pokemon_with_moves(
            Species::Blastoise,
            100,
            [0x9a, 0x78],
            [MoveId::Surf, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap();
        mon.ot_id = 1234;
        save.party.add(mon).unwrap();
        let bytes = export_sram(&save);
        let decoded = import_sram(&bytes).unwrap();
        assert!(!decoded.imported_legacy_native);
        assert_eq!(decoded.party.get(0).unwrap().level, 100);
        assert_eq!(decoded.party.get(0).unwrap().moves[0], MoveId::Surf);
        assert_eq!(decoded.party.get(0).unwrap().ot_id, 1234);
        assert_eq!(
            (decoded.game_data.position.x, decoded.game_data.position.y),
            (17, y)
        );
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cinnabar-victory-{}-{unique}-{serial}.sav",
            std::process::id()
        ));
        assert!(!companion_path_for_save(&path).exists());
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap()
            .write_all(&bytes)
            .unwrap();
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
        assert_eq!(game.state.screen, GameScreen::GameFreakSplash);
        game.set_seed(42); // Same one-time preboot configuration as CLI --seed.
        let mut s = Self {
            game,
            input: InputState::new(),
            path,
            bytes,
            pcm: vec![0.; 1470],
        };
        for frame in 0..LIMIT {
            if s.game.state.screen == GameScreen::Overworld
                && s.game.overworld.script_engine_idle()
                && s.game.overworld.active_script_effect_label().is_none()
            {
                break;
            }
            s.step(if frame % 8 == 0 { &[GbButton::A] } else { &[] });
        }
        // Initial six gate effects and ordinary field-loop control settle;
        // this is fixed neutral input, not a setter or a loop finding a seed.
        for _ in 0..16 {
            s.step(&[]);
        }
        assert_eq!(s.game.state.screen, GameScreen::Overworld);
        assert!(
            s.game.overworld.script_engine_idle()
                && s.game.overworld.active_script_effect_label().is_none()
        );
        assert_eq!(
            (
                s.game.overworld.state.player.x,
                s.game.overworld.state.player.y
            ),
            (17, u16::from(y))
        );
        for buttons in [
            &[GbButton::Up][..],
            &[GbButton::Up],
            &[],
            &[],
            &[GbButton::A],
            &[],
        ] {
            s.step(buttons);
        }
        assert_eq!(
            (
                s.game.overworld.state.player.x,
                s.game.overworld.state.player.y
            ),
            (17, u16::from(y))
        );
        assert_eq!(
            s.game.overworld.state.player.facing,
            pokered_core::overworld::Direction::Up
        );
        s
    }
}
#[test]
fn cold_saved_game_trainer_victory_shows_quote_before_money_then_waits_for_real_go_inside() {
    for (trainer, open) in [(0, false), (1, false), (1, true)] {
        let mut s = Session::boot(trainer, open);
        let beaten = format!("EVENT_BEAT_CINNABAR_GYM_TRAINER_{trainer}");
        let gate = format!("EVENT_CINNABAR_GYM_GATE{trainer}_UNLOCKED");
        let quote = if trainer == 0 {
            "Yow!\nHot, hot, hot!"
        } else {
            "I\nsurrender!"
        };
        let after = if trainer == 0 {
            "combustion..."
        } else {
            "I can't help stealing"
        };
        assert!(!s.flag(&beaten));
        assert_eq!(s.flag(&gate), open);
        let map = s.game.overworld.map_data.as_ref().unwrap();
        let initial_blocks = map.blocks.clone();
        let gate_index = (trainer == 1).then_some(3 * map.width as usize + 9);
        if let Some(index) = gate_index {
            assert_eq!(initial_blocks[index], if open { 14 } else { 84 });
        }
        let mut battle_seen = false;
        let mut go_count = 0;
        let mut previous_go = false;
        let mut go_pcm = false;
        let mut messages = Vec::<String>::new();
        let mut completed = false;
        for frame in 0..LIMIT {
            let (id, busy, pcm) = s.step(if frame % 32 == 0 { &[GbButton::A] } else { &[] });
            if s.game.state.screen == GameScreen::Battle {
                if !battle_seen {
                    assert_eq!(s.game.battle.end_battle_text.as_deref(), Some(quote));
                    assert_eq!(
                        s.game.battle.trainer_npc_index, None,
                        "scene owns trainer flags after audio"
                    );
                    battle_seen = true;
                }
                if let Some(message) = &s.game.battle.current_message {
                    if messages.last() != Some(message) {
                        messages.push(message.clone());
                    }
                }
            }
            let go = id == 86 && busy; // Actual SFX_GO_INSIDE, not a guessed duration.
            if go {
                if !previous_go {
                    go_count += 1;
                }
                go_pcm |= pcm;
                assert_eq!(s.game.state.screen, GameScreen::Overworld);
                assert_eq!(
                    s.game.overworld.active_script_effect_label().as_deref(),
                    Some("WaitSound")
                );
                assert!(!s.flag(&beaten));
                assert_eq!(s.flag(&gate), open);
                let blocks = &s.game.overworld.map_data.as_ref().unwrap().blocks;
                if let Some(index) = gate_index {
                    assert_eq!(blocks[index], initial_blocks[index]);
                } else {
                    assert_eq!(blocks, &initial_blocks, "trainer 0 has no physical gate");
                }
            }
            previous_go = go;
            if battle_seen && s.flag(&beaten) && s.game.overworld.script_engine_idle() {
                assert!(!busy || id != 86);
                completed = true;
                break;
            }
        }
        assert!(
            battle_seen && completed,
            "ordinary Surf/FIGHT inputs must finish trainer={trainer}: screen={:?}, battle_phase={:?}, effect={:?}",
            s.game.state.screen,
            s.game.battle.phase,
            s.game.overworld.active_script_effect_label()
        );
        assert_eq!(go_count, 1);
        assert!(go_pcm);
        assert_eq!(messages.iter().filter(|m| m.as_str() == quote).count(), 1);
        let quote_at = messages.iter().position(|m| m == quote).unwrap();
        let money_at = messages
            .iter()
            .position(|m| m.contains("for\nwinning!"))
            .unwrap();
        assert!(quote_at < money_at);
        assert!(!messages.iter().any(|m| m.contains(after)));
        assert!(s.flag(&gate));
        let blocks = &s.game.overworld.map_data.as_ref().unwrap().blocks;
        if let Some(index) = gate_index {
            assert_eq!(blocks[index], 14);
        } else {
            assert_eq!(blocks, &initial_blocks, "trainer 0 must not change map blocks");
        }
        assert!(s.game.overworld.displayed_field_dialogue().is_none());
        // Later defeated talk still owns AfterBattleText; no stale battle request.
        for _ in 0..32 {
            s.step(&[]);
        }
        s.step(&[GbButton::A]);
        let mut later_talk = false;
        for frame in 0..1000 {
            s.step(if frame % 32 == 0 { &[GbButton::A] } else { &[] });
            assert_eq!(s.game.state.screen, GameScreen::Overworld);
            if let Some(dialogue) = s.game.overworld.displayed_field_dialogue() {
                let text = dialogue
                    .pages()
                    .iter()
                    .map(|p| format!("{}\n{}", p.line1, p.line2))
                    .collect::<Vec<_>>()
                    .join("\n");
                if text.contains(after) {
                    later_talk = true;
                    break;
                }
            }
        }
        assert!(
            later_talk,
            "defeated talk must retain its original AfterBattleText"
        );
        assert_eq!(std::fs::read(&s.path).unwrap(), s.bytes);
    }
}
