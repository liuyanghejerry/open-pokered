//! Cold physical SRAM -> Continue/talk/FIGHT, real muted PCM and shared Game.
//! These are policy-driven caller tests, not a fixed-frame native replay.
use super::*;
use pokered_core::battle::{FaintCue, FaintSide, FaintSound};
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
const LIMIT: usize = 12000; // stuck bound, not a sound-length oracle

struct Session {
    game: PokemonGame,
    input: InputState,
    path: PathBuf,
    bytes: Vec<u8>,
    pcm: Vec<f32>,
    cues: Vec<FaintCue>,
    serial: Option<u32>,
    captions: Vec<String>,
    sfx: Vec<u8>,
    pcm_ids: Vec<u8>,
    previous_sfx: Option<u8>,
    silent_own_sound: bool,
}
impl Drop for Session {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("preserving faint fixture: {}", self.path.display());
        } else {
            let _ = std::fs::remove_file(&self.path);
        }
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
        if let Some((serial, cue)) = self.game.battle.presentation.current_faint() {
            if self.serial != Some(serial) {
                self.cues.push(cue);
                self.serial = Some(serial);
            }
        }
        if let Some(message) = self
            .game
            .battle
            .current_message
            .as_ref()
            .filter(|m| m.ends_with("fainted!"))
        {
            if self.captions.last() != Some(message) {
                self.captions.push(message.clone());
            }
        }
        let audio = self
            .game
            .audio
            .as_ref()
            .expect("muted retains actual PCM owner");
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
        if !self.cues.is_empty() && busy && self.previous_sfx != Some(id) {
            self.sfx.push(id);
        }
        self.previous_sfx = busy.then_some(id);
        audio.render_pcm(&mut self.pcm);
        if !self.cues.is_empty() && busy && channel && self.pcm.iter().any(|v| v.abs() > 0.00001) {
            if !self.pcm_ids.contains(&id) {
                self.pcm_ids.push(id);
            }
            if self
                .game
                .battle
                .presentation
                .current_faint()
                .is_some_and(|(_, cue)| cue.side == FaintSide::Player && cue.sound.is_none())
            {
                self.silent_own_sound = true;
            }
        }
    }
    fn boot(move_id: MoveId, reserve: bool) -> Self {
        let mut save = SaveData::new();
        save.player_name = pokered_data::charmap::encode_string("RED").unwrap();
        save.game_data.rival_name = pokered_data::charmap::encode_string("BLUE").unwrap();
        save.game_data.player_id = 1234;
        save.game_data.obtained_badges = 0xff;
        save.game_data.position.map_id = MapId::ViridianForest as u8;
        save.game_data.position.x = 2;
        save.game_data.position.y = 19;
        save.game_data.position.x_block = 1;
        save.game_data.position.y_block = 9;
        save.game_data.last_map = MapId::Route2 as u8;
        save.game_data.options.text_speed = pokered_core::options_menu::TextSpeed::Fast;
        let mut own = create_pokemon_with_moves(
            Species::Electrode,
            60,
            [0x9a, 0x78],
            [move_id, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap();
        own.hp = 1;
        own.ot_id = 1234;
        save.party.add(own).unwrap();
        if reserve {
            let mut bird = create_pokemon_with_moves(
                Species::Pidgey,
                5,
                [0x9a, 0x78],
                [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            )
            .unwrap();
            bird.ot_id = 1234;
            save.party.add(bird).unwrap();
        }
        let bytes = export_sram(&save);
        let restored = import_sram(&bytes).unwrap();
        assert!(!restored.imported_legacy_native);
        assert_eq!(restored.party.get(0).unwrap().hp, 1);
        assert_eq!(restored.party.get(0).unwrap().moves[0], move_id);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "faint-narration-{}-{stamp}-{id}.sav",
            std::process::id()
        ));
        assert!(!companion_path_for_save(&path).exists());
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
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
        game.set_seed(42); // one preboot CLI-equivalent seed; never changed after boot
        let mut session = Self {
            game,
            input: InputState::new(),
            path,
            bytes,
            pcm: vec![0.; 1470],
            cues: Vec::new(),
            serial: None,
            captions: Vec::new(),
            sfx: Vec::new(),
            pcm_ids: Vec::new(),
            previous_sfx: None,
            silent_own_sound: false,
        };
        for frame in 0..LIMIT {
            if session.game.state.screen == GameScreen::Overworld {
                break;
            }
            session.step(if frame % 8 == 0 { &[GbButton::A] } else { &[] });
        }
        assert_eq!(session.game.state.screen, GameScreen::Overworld);
        for _ in 0..16 {
            session.step(&[]);
        }
        // The source trainer is at (2,18); turning up is blocked by that NPC.
        for buttons in [
            &[GbButton::Up][..],
            &[GbButton::Up],
            &[],
            &[],
            &[GbButton::A],
            &[],
        ] {
            session.step(buttons);
        }
        for frame in 0..LIMIT {
            if session.game.state.screen == GameScreen::Battle
                && session.game.battle.phase == BattlePhase::PlayerMenu
            {
                break;
            }
            session.step(if frame % 8 == 0 { &[GbButton::B] } else { &[] });
        }
        assert_eq!(session.game.battle.phase, BattlePhase::PlayerMenu);
        session
    }
    fn fight_until_faint_returns(&mut self) {
        for frame in 0..LIMIT {
            if !self.cues.is_empty()
                && !matches!(self.game.battle.phase, BattlePhase::ShowingText { .. })
            {
                return;
            }
            let button = if matches!(
                self.game.battle.phase,
                BattlePhase::PlayerMenu | BattlePhase::MoveSelect
            ) {
                GbButton::A
            } else {
                GbButton::B
            };
            self.step(if frame % 8 == 0 {
                std::slice::from_ref(&button)
            } else {
                &[]
            });
        }
        panic!(
            "caller did not return from faint presentation: {:?}",
            self.game.battle.phase
        );
    }
}

#[test]
fn actual_trainer_selfdestruct_silent_own_slide_keeps_enemy_fall_thud_pcm_in_both_party_shapes() {
    for reserve in [false, true] {
        let mut session = Session::boot(MoveId::Selfdestruct, reserve);
        session.fight_until_faint_returns();
        assert_eq!(
            session.cues,
            vec![
                FaintCue {
                    side: FaintSide::Enemy,
                    sound: Some(FaintSound::EnemyFall),
                    text: None
                },
                FaintCue {
                    side: FaintSide::Player,
                    sound: None,
                    text: reserve.then_some(FaintSide::Enemy)
                },
            ]
        );
        assert_eq!(session.captions.len(), usize::from(reserve));
        assert!(session
            .captions
            .iter()
            .all(|text| text.starts_with("Enemy ")));
        let faint_sfx: Vec<_> = session
            .sfx
            .iter()
            .copied()
            .filter(|id| *id == SfxId::FaintFall as u8 || *id == SfxId::FaintThud as u8)
            .collect();
        assert_eq!(
            faint_sfx,
            vec![SfxId::FaintFall as u8, SfxId::FaintThud as u8]
        );
        assert!(session.pcm_ids.contains(&(SfxId::FaintFall as u8)));
        assert!(session.pcm_ids.contains(&(SfxId::FaintThud as u8)));
        assert!(
            !session.silent_own_sound,
            "silent Remove must not start an own cry"
        );
        let battle = session.game.battle.battle_state.as_ref().unwrap();
        assert_eq!(
            (battle.player.active_mon().hp, battle.enemy.active_mon().hp),
            (0, 0)
        );
        assert_eq!(battle.player.active_mon().pp[0], 4);
        assert_eq!(std::fs::read(&session.path).unwrap(), session.bytes);
    }
}

#[test]
fn ordinary_enemy_attack_single_own_faint_still_emits_the_real_own_cry_pcm() {
    let mut session = Session::boot(MoveId::Splash, true);
    session.fight_until_faint_returns();
    assert_eq!(
        session.cues,
        vec![FaintCue {
            side: FaintSide::Player,
            sound: Some(FaintSound::OwnCry),
            text: Some(FaintSide::Player)
        }]
    );
    assert_eq!(session.captions.len(), 1);
    let cry = pokered_data::cries::cry_data(Species::Electrode).sfx;
    assert!(session.sfx.contains(&cry));
    assert!(session.pcm_ids.contains(&cry));
    assert!(!session.sfx.contains(&(SfxId::FaintFall as u8)));
    assert!(!session.sfx.contains(&(SfxId::FaintThud as u8)));
    assert_eq!(std::fs::read(&session.path).unwrap(), session.bytes);
}
