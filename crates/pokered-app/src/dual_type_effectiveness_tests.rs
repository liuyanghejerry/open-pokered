use super::*;
use pokered_core::battle::BattlePhase;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};

fn on_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

type BattleTrace = Vec<(u32, Option<String>, u16, u8, u16, bool)>;

fn ice_beam_battle(limit: Option<u32>) -> (PokemonGame, u32, BattleTrace) {
    attack_battle(
        MoveId::IceBeam,
        Species::Gyarados,
        Some("It's super effective!"),
        limit,
    )
}

fn attack_battle(
    player_move: MoveId,
    defender: Species,
    stop_message: Option<&str>,
    limit: Option<u32>,
) -> (PokemonGame, u32, BattleTrace) {
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
    let mon = |species, level, move_id| {
        create_pokemon_with_moves(
            species,
            level,
            [0x99, 0x88],
            [move_id, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap()
    };
    let lead = mon(Species::Mew, 30, player_move);
    game.save_data.party.add(lead.clone()).unwrap();
    game.battle =
        BattleScreen::from_parties(true, &[lead], &[mon(defender, 5, MoveId::Splash)], None);
    game.battle.phase = BattlePhase::PlayerMenu;
    game.state.screen = GameScreen::Battle;
    game.set_seed(42);
    let mut input = InputState::new();
    let mut trace = Vec::new();
    let mut framebuffer = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut pcm = vec![0.0f32; 1470];
    for tick in 0..1200 {
        input.begin_frame();
        if tick % 16 == 0 {
            input.press(GbButton::A);
        } else {
            input.release(GbButton::A);
        }
        game.update(&input);
        game.draw(&mut framebuffer);
        let audio = game.audio.as_ref().unwrap();
        let (id, tempo) = {
            let manager = audio.manager.lock().unwrap();
            (
                manager.sequencer.current_sfx_id,
                manager.sequencer.sfx_tempo,
            )
        };
        audio.render_pcm(&mut pcm);
        let audible = pcm.iter().any(|sample| sample.abs() > 0.00001);
        let message = game.battle.current_message.clone();
        if trace
            .last()
            .is_none_or(|(_, last, hp, sound, last_tempo, last_audible)| {
                last != &message
                    || *hp != game.battle.enemy_hp
                    || *sound != id
                    || *last_tempo != tempo
                    || *last_audible != audible
            })
        {
            trace.push((
                tick,
                message.clone(),
                game.battle.enemy_hp,
                id,
                tempo,
                audible,
            ));
        }
        if limit == Some(tick)
            || (limit.is_none()
                && stop_message.is_some_and(|expected| message.as_deref() == Some(expected)))
        {
            return (game, tick, trace);
        }
        if limit.is_none() && game.state.screen != GameScreen::Battle {
            if stop_message.is_none() {
                return (game, tick, trace);
            }
            break;
        }
    }
    panic!("ICE BEAM never reached the original effectiveness caption: {trace:?}");
}

#[test]
fn actual_ice_beam_against_gyarados_prints_original_dual_type_caption() {
    on_stack(|| {
        let (game, _, _) = ice_beam_battle(None);
        assert_eq!(
            game.battle.current_message.as_deref(),
            Some("It's super effective!")
        );
        assert!(game.battle.enemy_hp < game.battle.enemy_max_hp);
    });
}

#[test]
fn actual_move_commands_keep_their_own_sound_table_modifiers_and_pcm() {
    on_stack(|| {
        for (move_id, expected) in [
            (MoveId::PayDay, vec![(SfxId::Pound, 0x100), (SfxId::SilphScope, 0x17f)]),
            (MoveId::HornAttack, vec![(SfxId::Battle1E, 0xe0)]),
            (MoveId::FirePunch, vec![(SfxId::Battle0D, 0xe0)]),
        ] {
            let (_, _, trace) = attack_battle(move_id, Species::Rattata, None, None);
            let mut previous = None;
            for (sound, tempo) in expected {
                let tick = trace.iter().find(|row| row.3 == sound as u8 && row.4 == tempo && row.5)
                    .unwrap_or_else(|| panic!("{move_id:?} missing audible {sound:?}/{tempo}: {trace:?}")).0;
                if let Some(before) = previous { assert!(before < tick); }
                previous = Some(tick);
            }
        }
    });
}

#[test]
#[ignore = "matched master/branch captures; FIDELITY_DUAL_TYPE_CAPTURE, optional FIDELITY_DUAL_TYPE_TICK"]
fn capture_dual_type_effectiveness() {
    on_stack(|| {
        let directory = PathBuf::from(std::env::var("FIDELITY_DUAL_TYPE_CAPTURE").unwrap());
        std::fs::create_dir_all(&directory).unwrap();
        let limit = std::env::var("FIDELITY_DUAL_TYPE_TICK")
            .ok()
            .map(|s| s.parse().unwrap());
        let (mut initial, _, _) = ice_beam_battle(Some(0));
        let mut first_frame = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        initial.draw(&mut first_frame);
        first_frame
            .save_png(&directory.join("initial.png"))
            .unwrap();
        std::fs::write(
            directory.join("initial.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "battle_state": &initial.battle.battle_state,
                "field_party": &initial.save_data.party,
                "player_display_hp": initial.battle.player_hp,
            }))
            .unwrap(),
        )
        .unwrap();
        drop(initial);
        let (mut game, tick, trace) = ice_beam_battle(limit);
        let mut framebuffer = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        game.draw(&mut framebuffer);
        framebuffer.save_png(&directory.join("frame.png")).unwrap();
        std::fs::write(directory.join("trace.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "capture_tick": tick, "messages_hp_sfx": trace,
                "battle_state": &game.battle.battle_state,
                "field_party": &game.save_data.party,
                "player_display_hp": game.battle.player_hp,
                "phase": format!("{:?}", game.battle.phase),
            "scope": "Seeded subsystem battle, actual Game A updates/draw/muted PCM; no whole playthrough or source-frame alignment claim",
        })).unwrap()).unwrap();
    });
}

#[test]
fn actual_hit_sound_precedes_hp_drain_once_with_original_tempo() {
    on_stack(|| {
        for (move_id, defender, caption, sound, tempo) in [
            (MoveId::Tackle, Species::Rattata, None, SfxId::Damage, 0xb0),
            (
                MoveId::IceBeam,
                Species::Gyarados,
                Some("It's super effective!"),
                SfxId::SuperEffective,
                0x17f,
            ),
            (
                MoveId::Flamethrower,
                Species::Dewgong,
                Some("It's not very effective..."),
                SfxId::NotVeryEffective,
                0x81,
            ),
        ] {
            let (_, _, trace) = attack_battle(move_id, defender, caption, None);
            let mut previous = (0u8, 0u16);
            let mut starts = Vec::new();
            for row in &trace {
                let current = (row.3, row.4);
                if current != previous && current == (sound as u8, tempo) {
                    starts.push(row.0);
                }
                previous = current;
            }
            let first_hp_loss = trace.iter().find(|row| row.2 < trace[0].2).unwrap().0;
            assert!(
                trace
                    .iter()
                    .any(|row| row.3 == sound as u8 && row.4 == tempo && row.5),
                "the applying sound must reach the real PCM renderer: {trace:?}"
            );
            assert_eq!(starts.len(), 1, "{move_id:?}: {trace:?}");
            assert!(
                starts[0] < first_hp_loss,
                "sound belongs to applying feedback, before HP drain: {trace:?}"
            );
            if sound != SfxId::Damage {
                assert!(
                    !trace.iter().any(|row| row.3 == SfxId::Damage as u8),
                    "no generic HP-drain sound: {trace:?}"
                );
            }
        }
    });
}
