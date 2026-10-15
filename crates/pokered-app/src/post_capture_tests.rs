//! Capture continuation regression and actual-input recorder.
use super::*;
use pokered_core::battle::settlement::{BattleOutcome, BattleSettlement};
use pokered_core::pokemon::stats::create_pokemon;
use pokered_core::start_menu::StartMenuItem;
use pokered_data::species::Species;

fn mon(species: Species, level: u8) -> pokered_core::battle::state::Pokemon {
    create_pokemon(species, level, [0x9a, 0x78]).unwrap()
}
fn game(owned: bool, boxed: bool, met_bill: bool) -> PokemonGame {
    let mut g = PokemonGame::new_with_options(
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
    g.state.config.language = pokered_core::game_state::Lang::En;
    g.state.config.text_speed = pokered_core::game_state::TextSpeed::Fast;
    g.save_data.party.clear();
    for _ in 0..if boxed { 6 } else { 1 } {
        g.save_data.party.add(mon(Species::Bulbasaur, 12)).unwrap();
    }
    let mut old = mon(Species::Weedle, 9);
    old.set_nickname("OLD");
    g.save_data.current_box.deposit(old).unwrap();
    g.save_data.sync_current_box_to_storage();
    if owned {
        g.save_data.game_data.pokedex.set_owned(Species::Weedle);
    }
    if met_bill {
        g.overworld
            .set_event_flag_live(pokered_data::event_flags::EventFlag::EVENT_MET_BILL);
    }
    g.battle = BattleScreen::from_parties(
        true,
        &g.save_data.party.to_vec(),
        &[mon(Species::Weedle, 4)],
        None,
    );
    g.battle.player_bag = g.save_data.game_data.bag.clone();
    g.battle.captured_mon = Some(mon(Species::Weedle, 4));
    g.battle.settlement = Some(BattleSettlement {
        outcome: BattleOutcome::Captured,
        money_gained: 0,
        money_lost: 0,
        payday_bonus: 0,
        exp_entries: vec![],
        level_ups: vec![],
        evolutions: vec![],
    });
    g.state.screen = GameScreen::Battle;
    g.handle_transition(GameScreen::Overworld);
    g
}
fn idle(g: &mut PokemonGame, frames: usize) {
    for _ in 0..frames {
        g.update(&InputState::new());
    }
}
fn tap(g: &mut PokemonGame, button: GbButton) {
    let mut input = InputState::new();
    input.press(button);
    g.update(&input);
    g.update(&InputState::new());
}
fn choice(g: &mut PokemonGame) {
    for frame in 0..2000 {
        if g.post_capture_choice().is_some() {
            return;
        }
        if frame % 30 == 0 {
            tap(g, GbButton::A);
        } else {
            idle(g, 1);
        }
    }
    panic!("DATA caller did not return to AskName");
}
fn finish(g: &mut PokemonGame) {
    for frame in 0..1000 {
        if !g.post_capture_active() {
            return;
        }
        if frame % 30 == 0 {
            tap(g, GbButton::A);
        } else {
            idle(g, 1);
        }
    }
    panic!("capture continuation did not return");
}

#[test]
fn actual_settlement_receipt_drives_owned_and_unowned_party_and_box_answers() {
    for owned in [false, true] {
        for boxed in [false, true] {
            for yes in [false, true] {
                let mut g = game(owned, boxed, false);
                let old = *g
                    .save_data
                    .current_box
                    .get(if boxed { 1 } else { 0 })
                    .unwrap();
                assert_eq!(
                    g.post_capture_flow
                        .as_ref()
                        .unwrap()
                        .receipt
                        .new_pokedex_entry,
                    !owned
                );
                choice(&mut g);
                assert_eq!(
                    g.post_capture_flow.as_ref().unwrap().backdrop,
                    if owned {
                        CaptureBackdrop::PlayerHud
                    } else {
                        CaptureBackdrop::Blank
                    }
                );
                assert_eq!(
                    g.post_capture_choice().unwrap().selected,
                    0,
                    "opening A does not choose YES"
                );
                if yes {
                    tap(&mut g, GbButton::A);
                    idle(&mut g, NAMING_FLASH_FRAMES as usize + 1);
                    tap(&mut g, GbButton::A);
                    tap(&mut g, GbButton::Start);
                    idle(&mut g, NAMING_FLASH_FRAMES as usize + 1);
                } else {
                    tap(&mut g, GbButton::Down);
                    tap(&mut g, GbButton::A);
                }
                if boxed {
                    assert_eq!(
                        g.post_capture_flow.as_ref().unwrap().backdrop,
                        if !owned {
                            CaptureBackdrop::Blank
                        } else if yes {
                            CaptureBackdrop::Battle
                        } else {
                            CaptureBackdrop::PlayerHud
                        }
                    );
                }
                finish(&mut g);
                let new = if boxed {
                    g.save_data.current_box.get(0).unwrap()
                } else {
                    g.save_data.party.get(1).unwrap()
                };
                let mut buf = [0; pokered_core::battle::state::NAME_TEXT_BUF];
                assert_eq!(new.display_name(&mut buf), if yes { "A" } else { "WEEDLE" });
                assert_eq!(
                    *g.save_data
                        .current_box
                        .get(if boxed { 1 } else { 0 })
                        .unwrap(),
                    old
                );
                assert_eq!(g.save_data.party.count(), if boxed { 6 } else { 2 });
                assert_eq!(g.save_data.current_box.count(), if boxed { 2 } else { 1 });
                assert_eq!(
                    g.save_data.current_box,
                    *g.save_data.pc_storage.current_box()
                );
                assert_eq!(g.state.screen, GameScreen::Overworld);
            }
        }
    }
}

#[test]
fn transferred_text_uses_the_saved_nickname_and_met_bill_recipient() {
    for met_bill in [false, true] {
        let mut g = game(true, true, met_bill);
        choice(&mut g);
        tap(&mut g, GbButton::A);
        idle(&mut g, 4);
        tap(&mut g, GbButton::A);
        tap(&mut g, GbButton::Start);
        idle(&mut g, 4);
        let flow = g.post_capture_flow.as_ref().unwrap();
        assert_eq!(flow.phase, CapturePhase::Transfer);
        let text = flow
            .dialogue
            .as_ref()
            .unwrap()
            .pages()
            .iter()
            .map(|p| format!("{} {}", p.line1, p.line2))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.starts_with("A was"));
        assert!(text.contains(if met_bill {
            "BILL's PC!"
        } else {
            "someone's PC!"
        }));
    }
}

#[test]
fn no_and_empty_name_leave_species_name_and_choice_does_not_consume_start() {
    for cancel_by_b in [false, true] {
        let mut g = game(true, false, false);
        choice(&mut g);
        if cancel_by_b {
            tap(&mut g, GbButton::B);
        } else {
            tap(&mut g, GbButton::A);
            idle(&mut g, 4);
            tap(&mut g, GbButton::Start);
            idle(&mut g, 4);
        }
        finish(&mut g);
        let mut buf = [0; pokered_core::battle::state::NAME_TEXT_BUF];
        assert_eq!(
            g.save_data.party.get(1).unwrap().display_name(&mut buf),
            "WEEDLE"
        );
    }
}

#[test]
fn modal_pauses_field_clock_and_safari_zero_ball_announcement() {
    let mut g = game(true, true, false);
    choice(&mut g);
    g.overworld.start_safari_game();
    for _ in 0..30 {
        g.overworld.use_safari_ball();
    }
    let frame = g.overworld.frame_counter;
    idle(&mut g, 120);
    assert_eq!(g.overworld.frame_counter, frame);
    assert!(g.overworld.pending_dialogue.is_none());
    assert!(g.post_capture_choice().is_some());
}

#[test]
fn new_dex_sound_finishes_before_fresh_confirmation_opens_data() {
    let mut g = game(false, false, false);
    g.audio = Some(AudioOutput::new_pcm());
    for frame in 0..1000 {
        if g.post_capture_flow.as_ref().unwrap().phase == CapturePhase::NewDexSound {
            break;
        }
        if frame % 30 == 0 {
            tap(&mut g, GbButton::A);
        } else {
            idle(&mut g, 1);
        }
    }
    assert_eq!(
        g.post_capture_flow.as_ref().unwrap().phase,
        CapturePhase::NewDexSound
    );
    assert!(g.audio.as_ref().unwrap().is_sfx_playing());
    tap(&mut g, GbButton::A);
    assert_eq!(
        g.state.screen,
        GameScreen::Battle,
        "the sound's text_promptbutton has not returned"
    );
    for _ in 0..1000 {
        if g.post_capture_flow.as_ref().unwrap().phase == CapturePhase::NewDexConfirm {
            break;
        }
        idle(&mut g, 1);
    }
    assert_eq!(
        g.post_capture_flow.as_ref().unwrap().phase,
        CapturePhase::NewDexConfirm
    );
    assert_eq!(g.state.screen, GameScreen::Battle);
    tap(&mut g, GbButton::A);
    assert_eq!(g.state.screen, GameScreen::Pokedex);
}

#[test]
fn unsaved_oak_name_input_supplies_the_raw_captured_ot() {
    let mut g = game(true, false, false);
    g.handle_transition(GameScreen::OakSpeech);
    assert!(g.save_data.player_name.is_empty());
    // Drive the complete ordinary speech to its name menu. This tests
    // the real PlayerNameSet consumer, rather than a Game-name setter.
    for frame in 0..4000 {
        if matches!(g.oak_speech.phase, OakSpeechPhase::PlayerNameChoice { .. }) {
            break;
        }
        if frame % 30 == 0 {
            tap(&mut g, GbButton::A);
        } else {
            idle(&mut g, 1);
        }
    }
    assert!(matches!(
        g.oak_speech.phase,
        OakSpeechPhase::PlayerNameChoice { .. }
    ));
    tap(&mut g, GbButton::Down);
    tap(&mut g, GbButton::A);
    assert_eq!(g.player_name, "ASH");
    let mut expected = [0x50; 11];
    let raw = pokered_data::charmap::encode_string("ASH").unwrap();
    expected[..raw.len()].copy_from_slice(&raw);
    assert_eq!(
        g.save_data.player_name, raw,
        "no SAVE has synchronized the name"
    );
    for frame in 0..4000 {
        if g.state.screen == GameScreen::Overworld {
            break;
        }
        if frame % 30 == 0 {
            tap(&mut g, GbButton::A);
        } else {
            idle(&mut g, 1);
        }
    }
    assert_eq!(g.state.screen, GameScreen::Overworld);
    // Unit-level catch settlement, separate from the actual natural-
    // encounter input controls recorded by the external audit driver.
    g.battle = BattleScreen::from_parties(
        true,
        &[mon(Species::Bulbasaur, 5)],
        &[mon(Species::Weedle, 4)],
        None,
    );
    g.battle.captured_mon = Some(mon(Species::Weedle, 4));
    g.battle.settlement = Some(BattleSettlement {
        outcome: BattleOutcome::Captured,
        money_gained: 0,
        money_lost: 0,
        payday_bonus: 0,
        exp_entries: vec![],
        level_ups: vec![],
        evolutions: vec![],
    });
    g.state.screen = GameScreen::Battle;
    g.handle_transition(GameScreen::Overworld);
    assert_eq!(g.save_data.party.get(1).unwrap().ot_name, expected);
}

#[cfg(feature = "debug-server")]
#[test]
#[ignore]
fn record_unsaved_oak_natural_capture() {
    let root = std::path::PathBuf::from(std::env::var("FIDELITY_UNSAVED_OAK_CAPTURE").unwrap());
    std::fs::create_dir_all(&root).unwrap();
    for repeat in [1, 2] {
        let trial = root.join(format!("repeat{repeat}"));
        std::fs::create_dir(&trial).unwrap();
        let mut g = PokemonGame::new_with_options(
            GameVersion::Red,
            Some(trial.join("ordinary.sav")),
            None,
            None,
            false,
            Some("ViridianForest,7,6".to_string()),
            false,
            true,
            None,
        );
        g.handle_transition(GameScreen::OakSpeech);
        g.main_menu.last_choice = Some(pokered_core::game_state::MainMenuChoice::NewGame);
        g.state.config.language = pokered_core::game_state::Lang::En;
        g.state.config.text_speed = pokered_core::game_state::TextSpeed::Fast;
        g.seed = Some(12);
        g.overworld.set_rng_seed(12);
        // Accelerated pre-input fixture only: supply a starting party and
        // Master Ball, then use the configured startup ViridianForest location.
        // No validation-time battle/map/party writes follow this boundary.
        g.save_data.party.add(mon(Species::Bulbasaur, 12)).unwrap();
        g.save_data
            .game_data
            .bag
            .add_item(pokered_data::items::ItemId::MasterBall, 2)
            .unwrap();
        g.save_data.game_data.pokedex.set_owned(Species::Bulbasaur);
        g.save_data.game_data.pokedex.set_seen(Species::Bulbasaur);
        std::fs::write(
            trial.join("construction.json"),
            serde_json::to_vec_pretty(&g.save_data).unwrap(),
        )
        .unwrap();
        assert!(g.save_data.player_name.is_empty());
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        struct TraceLog {
            path: std::path::PathBuf,
            frames: Vec<serde_json::Value>,
        }
        impl core::ops::Deref for TraceLog {
            type Target = Vec<serde_json::Value>;
            fn deref(&self) -> &Self::Target {
                &self.frames
            }
        }
        impl core::ops::DerefMut for TraceLog {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.frames
            }
        }
        impl Drop for TraceLog {
            fn drop(&mut self) {
                let _ = std::fs::write(&self.path, serde_json::to_vec(&self.frames).unwrap());
            }
        }
        let mut records = TraceLog {
            path: trial.join("frames.json"),
            frames: Vec::new(),
        };
        let mut physical_input = InputState::new();
        let mut previous_key = None;
        let mut frame = |g: &mut PokemonGame,
                         key: Option<GbButton>,
                         records: &mut Vec<serde_json::Value>,
                         fb: &mut FrameBuffer| {
            physical_input.begin_frame();
            if previous_key != key {
                if let Some(previous) = previous_key {
                    physical_input.release(previous);
                }
                if let Some(key) = key {
                    physical_input.press(key);
                }
            }
            previous_key = key;
            g.update(&physical_input);
            g.draw(fb);
            let state = g.debug_state_snapshot();
            records
                .push(serde_json::json!({"input": key.map(|k| format!("{k:?}")), "input_bits": physical_input.raw_current(), "state": state}));
        };
        for count in 0..4000 {
            if matches!(g.oak_speech.phase, OakSpeechPhase::PlayerNameChoice { .. }) {
                break;
            }
            frame(
                &mut g,
                (count % 30 == 0).then_some(GbButton::A),
                &mut records,
                &mut fb,
            );
        }
        assert!(matches!(
            g.oak_speech.phase,
            OakSpeechPhase::PlayerNameChoice { .. }
        ));
        frame(&mut g, Some(GbButton::Down), &mut records, &mut fb);
        frame(&mut g, None, &mut records, &mut fb);
        frame(&mut g, Some(GbButton::A), &mut records, &mut fb);
        frame(&mut g, None, &mut records, &mut fb);
        assert_eq!(g.player_name, "ASH");
        fb.save_png(&trial.join("oak-ASH-unsaved.png")).unwrap();
        for count in 0..4000 {
            if g.state.screen == GameScreen::Overworld {
                break;
            }
            frame(
                &mut g,
                (count % 30 == 0).then_some(GbButton::A),
                &mut records,
                &mut fb,
            );
        }
        assert_eq!(g.state.screen, GameScreen::Overworld);
        assert_eq!(g.overworld.state.current_map, MapId::ViridianForest);
        assert!(!trial.join("ordinary.sav").exists());
        fb.save_png(&trial.join("field-unsaved.png")).unwrap();
        'walk: for count in 0..1000 {
            let key = if count % 2 == 0 {
                GbButton::Right
            } else {
                GbButton::Left
            };
            for _ in 0..16 {
                frame(&mut g, Some(key), &mut records, &mut fb);
                if g.state.screen == GameScreen::Battle {
                    break 'walk;
                }
            }
            for _ in 0..16 {
                frame(&mut g, None, &mut records, &mut fb);
            }
        }
        assert_eq!(
            g.state.screen,
            GameScreen::Battle,
            "ordinary grass encounter required"
        );
        assert!(g.battle.is_wild);
        for count in 0..4000 {
            if g.battle.phase == BattlePhase::PlayerMenu {
                break;
            }
            frame(
                &mut g,
                (count % 64 == 0).then_some(GbButton::A),
                &mut records,
                &mut fb,
            );
        }
        assert_eq!(g.battle.phase, BattlePhase::PlayerMenu);
        for key in [GbButton::Up, GbButton::Left, GbButton::Down, GbButton::A] {
            frame(&mut g, Some(key), &mut records, &mut fb);
            frame(&mut g, None, &mut records, &mut fb);
        }
        assert_eq!(g.battle.phase, BattlePhase::BagSelect);
        frame(&mut g, Some(GbButton::A), &mut records, &mut fb);
        frame(&mut g, None, &mut records, &mut fb);
        for count in 0..4000 {
            if g.post_capture_choice().is_some() {
                break;
            }
            frame(
                &mut g,
                (count % 64 == 0).then_some(GbButton::A),
                &mut records,
                &mut fb,
            );
        }
        assert!(g.post_capture_choice().is_some());
        fb.save_png(&trial.join("capture-name-choice.png")).unwrap();
        frame(&mut g, Some(GbButton::Down), &mut records, &mut fb);
        frame(&mut g, None, &mut records, &mut fb);
        frame(&mut g, Some(GbButton::A), &mut records, &mut fb);
        frame(&mut g, None, &mut records, &mut fb);
        for count in 0..4000 {
            if g.state.screen == GameScreen::Overworld
                && !g.post_capture_active()
                && matches!(
                    g.overworld.warp_fade_state,
                    pokered_core::overworld::WarpFadeState::Idle
                )
            {
                break;
            }
            frame(
                &mut g,
                (count % 64 == 0).then_some(GbButton::A),
                &mut records,
                &mut fb,
            );
        }
        assert_eq!(g.save_data.party.count(), 2);
        let mut expected = [0x50; 11];
        let raw = pokered_data::charmap::encode_string("ASH").unwrap();
        expected[..raw.len()].copy_from_slice(&raw);
        assert_eq!(g.save_data.party.get(1).unwrap().ot_name, expected);
        assert_eq!(
            g.save_data.party.get(1).unwrap().ot_id,
            g.save_data.game_data.player_id
        );
        assert!(!trial.join("ordinary.sav").exists());
        for _ in 0..64 {
            frame(&mut g, None, &mut records, &mut fb);
        }
        for _ in 0..4 {
            frame(&mut g, Some(GbButton::Start), &mut records, &mut fb);
        }
        frame(&mut g, None, &mut records, &mut fb);
        for _ in 0..60 {
            frame(&mut g, None, &mut records, &mut fb);
        }
        assert_eq!(g.state.screen, GameScreen::StartMenu);
        for _ in 0..8 {
            if g.start_menu.current_item() == StartMenuItem::Save {
                break;
            }
            frame(&mut g, Some(GbButton::Down), &mut records, &mut fb);
            for _ in 0..32 {
                frame(&mut g, None, &mut records, &mut fb);
            }
        }
        assert_eq!(g.start_menu.current_item(), StartMenuItem::Save);
        frame(&mut g, Some(GbButton::A), &mut records, &mut fb);
        frame(&mut g, None, &mut records, &mut fb);
        for count in 0..2000 {
            if trial.join("ordinary.sav").exists() {
                break;
            }
            frame(
                &mut g,
                (count % 64 == 0).then_some(GbButton::A),
                &mut records,
                &mut fb,
            );
        }
        let saved = import_sram(&std::fs::read(trial.join("ordinary.sav")).unwrap()).unwrap();
        assert_eq!(saved.party.get(1).unwrap().ot_name, expected);
        assert_eq!(saved.party.get(1).unwrap().ot_id, saved.game_data.player_id);
        fb.save_png(&trial.join("first-ordinary-save.png")).unwrap();
        std::fs::write(
            trial.join("frames.json"),
            serde_json::to_vec(&records.frames).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn snapshot_restore_and_title_reset_discard_capture_modal_and_naming_state() {
    let mut g = game(true, true, false);
    choice(&mut g);
    tap(&mut g, GbButton::A);
    g.handle_transition(GameScreen::TitleScreen);
    assert!(!g.post_capture_active());
    assert!(g.post_capture_choice().is_none());
    let mut g = game(true, false, false);
    assert!(
        g.agent_save_state().is_err(),
        "modal receipt must not be silently omitted from a snapshot"
    );
    g.clear_post_capture_continuation();
    g.state.screen = GameScreen::Overworld;
    let saved = g.agent_save_state().unwrap();
    g.begin_post_capture(CapturedMonReceipt {
        location: CapturedLocation::Party(1),
        species: Species::Weedle,
        new_pokedex_entry: false,
    });
    g.agent_restore_state(&saved).unwrap();
    assert!(!g.post_capture_active());
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
#[test]
fn ordinary_save_load_discards_modal_without_old_battle_party_writeback() {
    let mut g = game(true, false, false);
    choice(&mut g);
    tap(&mut g, GbButton::A);
    let mut replacement = pokered_core::save::SaveData::new();
    replacement.party.add(mon(Species::Pikachu, 22)).unwrap();
    let bytes = pokered_core::save::sram_export::export_sram(&replacement);
    let path = std::env::temp_dir().join(format!(
        "pokered-post-capture-load-{}.sav",
        std::process::id()
    ));
    std::fs::write(&path, bytes).unwrap();
    g.load_from_path(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    idle(&mut g, 120);
    assert!(!g.post_capture_active());
    assert_eq!(g.save_data.party.count(), 1);
    assert_eq!(g.save_data.party.get(0).unwrap().species, Species::Pikachu);
}

#[test]
fn capture_ball_finishes_before_success_sound_and_caption_survives_return() {
    for boxed in [false, true] {
        for low_hp in [false, true] {
            for safari in [false, true] {
                verify_capture_presentation(boxed, low_hp, safari);
            }
        }
    }
}

fn verify_capture_presentation(boxed: bool, low_hp: bool, safari: bool) {
    use pokered_core::battle::{BattlePhase, menu::BagMenuState};
    use pokered_data::items::ItemId;
    let mut g = PokemonGame::new_with_options(
        GameVersion::Red, None, None, None, false, None, false, true,
        #[cfg(feature = "debug-server")] None,
    );
    g.state.config.language = pokered_core::game_state::Lang::En;
    g.save_data.party.clear();
    for index in 0..if boxed { 6 } else { 1 } {
        let mut player = mon(Species::Bulbasaur, 12);
        if low_hp && index == 0 { player.hp = 1; }
        g.save_data.party.add(player).unwrap();
    }
    let protected = g.save_data.party.to_vec();
    g.save_data.game_data.pokedex.set_owned(Species::Weedle);
    g.save_data.game_data.bag.add_item(ItemId::MasterBall, 2).unwrap();
    let mut enemy = mon(Species::Weedle, 4);
    if safari { enemy.hp = 1; }
    g.battle = BattleScreen::from_parties(true, &protected, &[enemy], None);
    g.battle.player_bag = g.save_data.game_data.bag.clone();
    g.battle.bag_menu = Some(BagMenuState::new(vec![(ItemId::MasterBall, 2)]));
    g.battle.phase = if safari { BattlePhase::PlayerMenu } else { BattlePhase::BagSelect };
    if safari {
        // Natural WEEDLE catch rate255 + legal HP1 makes W>255, so the
        // actual Safari path captures for every legal Rand1/Rand2 draw.
        g.battle.is_safari = true;
        g.battle.safari = Some(pokered_core::battle::safari::SafariState::new(255, 1));
    }
    g.state.screen = GameScreen::Battle;
    g.audio = Some(AudioOutput::new_pcm());
    let mut seen = false;
    let mut finished = false;
    let mut saw_battle_over = false;
    let mut active_frames = [0usize; 3];
    let mut pcm = vec![0.0f32; 1470];
    let mut audible = false;
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    for frame in 0..2400 {
        let mut input = InputState::new();
        // Throw once, then send no A while the unprompted used-ball page
        // owns the animation. Aggressive later A tests the sound wait.
        if frame == 0 || (frame % 8 == 0 && g.battle.current_message.as_deref()
            .is_some_and(|text| text.starts_with("All right!"))) {
            input.press(GbButton::A);
        }
        g.update(&input);
        g.draw(&mut fb);
        let channels = {
            let manager = g.audio.as_ref().unwrap().manager.lock().unwrap();
            core::array::from_fn::<_, 3, _>(|i| {
                let channel = &manager.sequencer.channels[4 + i];
                (channel.active, channel.sound_id)
            })
        };
        g.audio.as_ref().unwrap().render_pcm(&mut pcm);
        if channels.iter().any(|&(active, id)| active && id == SfxId::CaughtMon as u8) {
            assert!(!low_hp || safari, "the ordinary low-health alarm rejects this SFX");
            assert!(g.battle_vfx.item_animation_capture_finished(), "a later Tink must not replace the jingle");
            if !seen {
                assert_eq!(channels, [(true, SfxId::CaughtMon as u8); 3]);
                seen = true;
            }
        }
        if seen && !finished {
            audible |= pcm.iter().any(|sample| sample.abs() > 0.00001);
            for (i, &(active, id)) in channels.iter().enumerate() {
                if active {
                    assert_eq!(id, SfxId::CaughtMon as u8, "caught voice {i} was replaced");
                    active_frames[i] += 1;
                }
            }
            if channels.iter().all(|&(active, _)| !active) { finished = true; }
            else {
                assert_eq!(g.state.screen, GameScreen::Battle);
                assert!(g.battle.current_message.as_deref().unwrap_or_default().contains("was caught!"));
            }
        }
        if matches!(g.battle.phase, BattlePhase::BattleOver { .. }) {
            saw_battle_over = true;
            assert_eq!(g.battle.current_message.as_deref(), Some("All right!\nWEEDLE was caught!"));
            if !low_hp || safari { assert!(seen && finished, "sound must return before the text caller"); }
            let actual = fb.clone();
            let phase = g.battle.phase.clone();
            g.battle.phase = BattlePhase::ShowingText {
                messages: vec!["All right!\nWEEDLE was caught!".to_string()], current: 0,
                wait_frames: 0, next_phase: Box::new(phase.clone()),
            };
            g.draw(&mut fb);
            for y in 96..144 { for x in 0..160 {
                assert_eq!(actual.get_pixel(x, y), fb.get_pixel(x, y), "catch return must render its success caption");
            } }
            g.battle.phase = phase;
        }
        if g.post_capture_active() { break; }
    }
    assert!(saw_battle_over && g.post_capture_active());
    if low_hp && !safari { assert!(!seen); }
    else { assert!(seen && finished && audible); assert!(active_frames.iter().all(|&n| n > 4)); }
    for (index, player) in protected.iter().enumerate() {
        assert_eq!(serde_json::to_value(g.save_data.party.get(index).unwrap()).unwrap(), serde_json::to_value(player).unwrap());
    }
    assert_eq!(g.save_data.party.count(), if boxed { 6 } else { 2 });
    assert_eq!(g.save_data.current_box.count(), usize::from(boxed));
    if safari { assert_eq!(g.battle.safari.as_ref().unwrap().balls, 0); }
}

#[test]
fn failed_ball_result_waits_for_the_complete_throw() {
    use pokered_core::battle::{BallAnimOutcome, BattleAnimEvent, BattlePhase};
    use pokered_data::items::ItemId;
    let mut g = PokemonGame::new_with_options(
        GameVersion::Red, None, None, None, false, None, false, true,
        #[cfg(feature = "debug-server")] None,
    );
    // Isolate the already-resolved failed throw's presentation. Whole failed
    // Safari throws and their ball economy are checked by the actual controls.
    g.battle = BattleScreen::from_parties(true, &[mon(Species::Bulbasaur, 12)], &[mon(Species::Venonat, 22)], None);
    g.battle.is_safari = true;
    g.battle.safari = Some(pokered_core::battle::safari::SafariState::new(190, 29));
    g.battle.phase = BattlePhase::ShowingText {
        messages: vec!["RED used\nSAFARI BALL!".to_string(), "Aww! It appeared\nto be caught!".to_string()],
        current: 0, wait_frames: 0, next_phase: Box::new(BattlePhase::PlayerMenu),
    };
    g.battle_vfx.on_anim_event(BattleAnimEvent::Ball {
        ball: ItemId::SafariBall, shakes: 2, outcome: BallAnimOutcome::BrokeFree,
    });
    g.state.screen = GameScreen::Battle;
    g.audio = Some(AudioOutput::new_pcm());
    let mut saw_active = false;
    let mut saw_result = false;
    for frame in 0..1200 {
        let was_active = g.battle_vfx.ball_animation_active();
        saw_active |= was_active;
        let mut input = InputState::new();
        if frame % 8 == 0 { input.press(GbButton::A); }
        g.update(&input);
        if let BattlePhase::ShowingText { current, .. } = g.battle.phase {
            if was_active { assert_eq!(current, 0, "result text cannot interrupt the ball"); }
            saw_result |= current == 1;
        }
        assert_eq!(g.battle.safari.as_ref().unwrap().balls, 29);
        assert!(g.battle.captured_mon.is_none());
        if g.battle.phase == BattlePhase::PlayerMenu {
            assert!(saw_active && saw_result);
            return;
        }
    }
    panic!("failed ball presentation did not return to its caller");
}

#[test]
fn ball_caller_gate_returns_once_and_keeps_trainer_pages_prompted() {
    use pokered_core::battle::{BallAnimOutcome, BattleAnimEvent, BattleInput, BattlePhase};
    use pokered_data::items::ItemId;
    for outcome in [
        BallAnimOutcome::Caught,
        BallAnimOutcome::BrokeFree,
        BallAnimOutcome::Dodged,
        BallAnimOutcome::Blocked,
    ] {
        let mut g = game(true, false, false);
        g.clear_post_capture_continuation();
        assert!(!g.battle_vfx.has_ball_text_continuation());
        // Isolate an already-resolved ball's presentation, including the
        // trainer-block and ghost/Safari dodge callers. No RNG is overridden.
        g.battle.phase = BattlePhase::ShowingText {
            messages: vec!["Used ball".to_string(), "Result".to_string()],
            current: 0, wait_frames: 1, next_phase: Box::new(BattlePhase::PlayerMenu),
        };
        g.battle_vfx.on_anim_event(BattleAnimEvent::Ball {
            ball: ItemId::PokeBall, shakes: 1, outcome,
        });
        for _ in 0..1200 {
            assert!(g.battle_vfx.has_ball_text_continuation());
            let mut input = BattleInput { a: true, b: true, ..BattleInput::none() };
            g.apply_ball_text_continuation(&mut input);
            assert!(!input.a && !input.b, "A/B cannot interrupt {outcome:?}");
            g.battle_vfx.update(&g.battle);
            if !g.battle_vfx.ball_animation_active() { break; }
        }
        assert!(!g.battle_vfx.ball_animation_active());
        let wild_return = outcome != BallAnimOutcome::Blocked;
        assert_eq!(g.battle_vfx.has_ball_text_continuation(), wild_return);
        assert_eq!(g.battle_vfx.ball_result_ready(), wild_return);
        let mut input = BattleInput::none();
        g.apply_ball_text_continuation(&mut input);
        assert!(!input.a, "completion cannot skip the remaining text delay");
        assert_eq!(g.battle_vfx.ball_result_ready(), wild_return);
        if let BattlePhase::ShowingText { wait_frames, .. } = &mut g.battle.phase {
            *wait_frames = 0;
        }
        g.apply_ball_text_continuation(&mut input);
        assert_eq!(input.a, wild_return, "trainer pages still require real input");
        assert!(!g.battle_vfx.has_ball_text_continuation());
        let mut next_input = BattleInput::none();
        g.apply_ball_text_continuation(&mut next_input);
        assert!(!next_input.a && !next_input.b, "the completed throw returns once");
    }
}


#[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
#[test]
fn completed_ball_event_does_not_cross_load_snapshot_or_title_reset() {
    use pokered_core::battle::{BallAnimOutcome, BattleAnimEvent, BattlePhase};
    use pokered_data::items::ItemId;
    for boundary in 0..3 {
        let mut g = game(true, false, false);
        g.clear_post_capture_continuation();
        g.state.screen = GameScreen::Overworld;
        let snapshot = g.agent_save_state().unwrap();
        g.battle_vfx.on_anim_event(BattleAnimEvent::Ball {
            ball: ItemId::PokeBall, shakes: 0, outcome: BallAnimOutcome::Dodged,
        });
        for _ in 0..1200 {
            g.battle_vfx.update(&g.battle);
            if !g.battle_vfx.ball_animation_active() { break; }
        }
        assert!(!g.battle_vfx.ball_animation_active());
        match boundary {
            0 => g.handle_transition(GameScreen::TitleScreen),
            1 => { g.agent_restore_state(&snapshot).unwrap(); }
            _ => {
                let path = std::env::temp_dir().join(format!("capture-ball-cleanup-{}.sav", std::process::id()));
                std::fs::write(&path, pokered_core::save::sram_export::export_sram(&g.save_data)).unwrap();
                g.load_from_path(&path).unwrap();
                std::fs::remove_file(path).unwrap();
            }
        }
        // A different two-page caller must not receive the abandoned ball's
        // automatic continuation, even if it starts at the same text position.
        g.state.screen = GameScreen::Battle;
        g.battle.captured_mon = None;
        g.battle.phase = BattlePhase::ShowingText {
            messages: vec!["First prompt".to_string(), "Second prompt".to_string()],
            current: 0, wait_frames: 0, next_phase: Box::new(BattlePhase::PlayerMenu),
        };
        g.update(&InputState::new());
        assert!(matches!(g.battle.phase, BattlePhase::ShowingText { current: 0, .. }));
        assert!(!g.battle_vfx.take_ball_result_ready());
    }
}
