use super::*;
use pokered_core::battle::state::StatusCondition;
use pokered_core::overworld::poison::FieldPoisonFlow;
use pokered_core::pokemon::party::Party;
use pokered_core::snapshot::OverworldSnapshot;
use pokered_data::{moves::MoveId, species::Species};

fn on_game_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

fn game(hp_and_poison: &[(u16, bool)]) -> PokemonGame {
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
    g.save_data = SaveData::new();
    g.state.screen = GameScreen::Overworld;
    g.state.config.text_speed = pokered_core::game_state::TextSpeed::Fast;
    g.overworld = OverworldScreen::new(MapId::RedsHouse2F, None, PokemonRedData);
    g.overworld.state.player.x = 3;
    g.overworld.state.player.y = 6;
    g.overworld.player_name = "NINTEN".into();
    let mons = hp_and_poison
        .iter()
        .map(|&(hp, poison)| {
            let mut m = pokered_core::pokemon::stats::create_pokemon_with_moves(
                Species::Rattata,
                12,
                [0x9A, 0x78],
                [
                    MoveId::Tackle,
                    MoveId::TailWhip,
                    MoveId::QuickAttack,
                    MoveId::None,
                ],
            )
            .unwrap();
            m.hp = hp;
            m.status = if poison {
                StatusCondition::Poison
            } else {
                StatusCondition::None
            };
            m
        })
        .collect::<Vec<_>>();
    g.save_data.party = Party::from(mons);
    g.save_data.game_data.player_money = 3001;
    g.save_data.game_data.last_blackout_map = MapId::PalletTown as u8;
    g
}

fn buttons(keys: &[GbButton]) -> InputState {
    let mut input = InputState::new();
    for &key in keys {
        input.press(key);
    }
    input
}

fn begin(g: &mut PokemonGame) {
    g.overworld
        .game_data_requests
        .push(OverworldGameDataRequest::PoisonStep);
    g.update(&InputState::new());
}

fn hp(g: &PokemonGame) -> Vec<u16> {
    g.save_data.party.iter().map(|m| m.hp).collect()
}

fn wait_for_prompt(g: &mut PokemonGame) {
    for _ in 0..200 {
        if g.overworld
            .pending_dialogue
            .as_ref()
            .is_some_and(|d| d.waiting_for_input())
        {
            return;
        }
        g.update(&InputState::new());
    }
    panic!("poison dialogue never reached its prompt");
}

fn acknowledge(g: &mut PokemonGame) {
    wait_for_prompt(g);
    g.update(&buttons(&[GbButton::A]));
    g.update(&InputState::new()); // HoldTextDisplayOpen returns on A release.
}

fn approach_fourth_step_door(g: &mut PokemonGame) {
    g.overworld = OverworldScreen::new(MapId::ViridianCity, None, PokemonRedData);
    g.overworld.state.player.x = 20;
    g.overworld.state.player.y = 26;
    g.overworld.state.player.facing = dotzuki_engine::overworld::types::Direction::Right;
    for button in [GbButton::Right, GbButton::Right, GbButton::Right, GbButton::Up] {
        for _ in 0..4 { g.update(&buttons(&[button])); }
        for _ in 0..28 { g.update(&InputState::new()); }
    }
}

#[test]
fn field_poison_door_fourth_step_damages_party_before_map_change() {
    on_game_stack(|| {
        let mut g = game(&[(10, true), (31, false)]);
        approach_fourth_step_door(&mut g);
        for _ in 0..120 { g.update(&InputState::new()); }
        assert_eq!(g.overworld.state.current_map, MapId::ViridianPokecenter);
        assert_eq!(hp(&g), vec![9, 31], "the door step still calls poison");
    });
}

#[test]
fn field_poison_door_waits_for_faint_ack_before_entering() {
    on_game_stack(|| {
        let mut g = game(&[(1, true), (31, false)]);
        approach_fourth_step_door(&mut g);
        wait_for_prompt(&mut g);
        assert_eq!(g.overworld.state.current_map, MapId::ViridianCity);
        assert_eq!(hp(&g), vec![0, 31]);
        assert_eq!(g.overworld.warp_fade_state, pokered_core::overworld::WarpFadeState::Idle);
        acknowledge(&mut g);
        for _ in 0..120 { g.update(&InputState::new()); }
        assert_eq!(g.overworld.state.current_map, MapId::ViridianPokecenter);
        assert_eq!(hp(&g), vec![0, 31]);
    });
}

#[test]
fn field_poison_door_blackout_cancels_the_door_before_recovery() {
    on_game_stack(|| {
        let mut g = game(&[(1, true), (1, true)]);
        approach_fourth_step_door(&mut g);
        wait_for_prompt(&mut g);
        assert_eq!(g.overworld.state.current_map, MapId::ViridianCity);
        acknowledge(&mut g);
        acknowledge(&mut g);
        assert_eq!(g.overworld.poison_flow, Some(FieldPoisonFlow::Blackout));
        assert!(g.overworld.pending_warp.is_none(), "abandon the pending clinic door");
        acknowledge(&mut g);
        for _ in 0..120 {
            g.update(&InputState::new());
            assert_ne!(g.overworld.state.current_map, MapId::ViridianPokecenter);
        }
        assert_eq!(g.overworld.state.current_map, MapId::PalletTown);
        assert_eq!(hp(&g), vec![31, 31]);
        assert_eq!(g.save_data.game_data.player_money, 1500);
    });
}

#[test]
fn field_poison_door_snapshot_resumes_the_original_destination_once() {
    on_game_stack(|| {
        let mut g = game(&[(1, true), (31, false)]);
        approach_fourth_step_door(&mut g);
        wait_for_prompt(&mut g);
        let saved_party = g.save_data.clone();
        let mut value = serde_json::to_value(OverworldSnapshot::capture(&g.overworld)).unwrap();
        let snapshot: OverworldSnapshot = serde_json::from_value(value.clone()).unwrap();
        let mut restored = game(&[(31, false)]);
        restored.save_data = saved_party;
        snapshot.restore_into(&mut restored.overworld);
        for _ in 0..60 {
            restored.update(&InputState::new());
            assert_eq!(restored.overworld.state.current_map, MapId::ViridianCity);
        }
        acknowledge(&mut restored);
        for _ in 0..180 { restored.update(&InputState::new()); }
        assert_eq!(restored.overworld.state.current_map, MapId::ViridianPokecenter);
        assert_eq!(hp(&restored), vec![0, 31]);
        assert!(restored.overworld.pending_warp.is_none());
        assert!(restored.overworld.poison_step_warp_sound.is_none());
        value.as_object_mut().unwrap().remove("poison_step_warp_sound");
        let legacy: OverworldSnapshot = serde_json::from_value(value).unwrap();
        assert!(legacy.poison_step_warp_sound.is_none());
    });
}

#[test]
fn field_poison_door_reset_abandons_the_suspended_destination() {
    on_game_stack(|| {
        let mut g = game(&[(1, true), (31, false)]);
        approach_fourth_step_door(&mut g);
        wait_for_prompt(&mut g);
        assert!(g.overworld.pending_warp.is_some());
        let missing = std::env::temp_dir().join(format!(
            "pokered-poison-door-907-missing-{}.sav", std::process::id()
        ));
        assert!(!missing.exists());
        g.save_path = Some(missing);
        g.soft_reset();
        assert_eq!(g.state.screen, GameScreen::TitleScreen);
        assert!(g.overworld.poison_flow.is_none());
        assert!(g.overworld.pending_dialogue.is_none());
        assert!(g.overworld.pending_warp.is_none());
        assert!(g.overworld.poison_step_warp_sound.is_none());
    });
}

#[test]
fn field_poison_door_healthy_fourth_step_keeps_normal_transition_timing() {
    on_game_stack(|| {
        let run = |counter| {
            let mut g = game(&[(31, false), (31, false)]);
            g.overworld = OverworldScreen::new(MapId::ViridianCity, None, PokemonRedData);
            g.overworld.state.player.x = 23;
            g.overworld.state.player.y = 26;
            g.overworld.state.player.facing = dotzuki_engine::overworld::types::Direction::Up;
            g.overworld.poison_step_counter = counter;
            let mut transition = Vec::new();
            for frame in 0..120 {
                g.update(&if frame < 4 { buttons(&[GbButton::Up]) } else { InputState::new() });
                assert!(g.overworld.pending_dialogue.is_none());
                assert!(g.overworld.poison_flow.is_none());
                transition.push((g.overworld.state.current_map, g.overworld.warp_fade_state));
            }
            assert_eq!(g.overworld.state.current_map, MapId::ViridianPokecenter);
            assert_eq!(hp(&g), vec![31, 31]);
            transition
        };
        assert_eq!(run(0), run(3), "a healthy fourth step must not add a wait");
    });
}

#[test]
fn field_poison_door_safari_timeout_takes_priority_over_poison_and_rest_house() {
    on_game_stack(|| {
        let mut g = game(&[(1, true), (31, false)]);
        g.overworld = OverworldScreen::new(MapId::SafariZoneCenter, None, PokemonRedData);
        g.overworld.state.player.x = 17;
        g.overworld.state.player.y = 20;
        g.overworld.state.player.facing = dotzuki_engine::overworld::types::Direction::Up;
        g.overworld.start_safari_game();
        let mut snapshot = OverworldSnapshot::capture(&g.overworld);
        snapshot.safari_steps = 0;
        snapshot.poison_step_counter = 3;
        snapshot.restore_into(&mut g.overworld);
        for _ in 0..4 { g.update(&buttons(&[GbButton::Up])); }
        for _ in 0..28 { g.update(&InputState::new()); }
        assert_eq!((g.overworld.state.player.x, g.overworld.state.player.y), (17, 19));
        assert!(OverworldSnapshot::capture(&g.overworld).safari_eject_pending.is_some());
        assert!(g.overworld.pending_warp.is_none());
        assert!(g.overworld.poison_flow.is_none());
        assert!(g.overworld.poison_step_warp_sound.is_none());
        assert_eq!(hp(&g), vec![1, 31]);
        for _ in 0..6 {
            if g.overworld.pending_dialogue.is_none() { break; }
            acknowledge(&mut g);
        }
        for _ in 0..180 {
            g.update(&InputState::new());
            assert_ne!(g.overworld.state.current_map, MapId::SafariZoneCenterRestHouse);
        }
        assert_eq!(g.overworld.state.current_map, MapId::SafariZoneGate);
        assert_eq!(hp(&g), vec![1, 31]);
    });
}

#[test]
fn field_poison_damage_waits_for_each_real_faint_ack() {
    on_game_stack(|| {
        let mut g = game(&[(1, true), (1, true), (31, false)]);
        let before = g.save_data.party.to_vec();
        begin(&mut g);
        assert_eq!(hp(&g), vec![0, 1, 31]);
        assert_eq!(
            g.save_data.party.get(0).unwrap().status,
            StatusCondition::None
        );
        assert_eq!(
            g.save_data.party.get(1).unwrap().status,
            StatusCondition::Poison
        );
        // A held during font transfer/typewriter cannot ACK the next Pokemon.
        for _ in 0..100 {
            g.update(&buttons(&[GbButton::A, GbButton::Right, GbButton::Start]));
        }
        assert_eq!(hp(&g), vec![0, 1, 31]);
        assert_eq!(g.state.screen, GameScreen::Overworld);
        assert_eq!(
            (g.overworld.state.player.x, g.overworld.state.player.y),
            (3, 6)
        );
        g.update(&InputState::new());
        acknowledge(&mut g);
        assert_eq!(hp(&g), vec![0, 0, 31]);
        assert!(
            g.overworld.pending_dialogue.is_some(),
            "second faint has its own caller"
        );
        assert!(matches!(
            g.overworld.poison_flow,
            Some(FieldPoisonFlow::Faint { next_index: 2 })
        ));
        acknowledge(&mut g);
        assert!(g.overworld.poison_flow.is_none());
        assert!(g.overworld.pending_warp.is_none());
        for (old, new) in before.iter().zip(g.save_data.party.iter()) {
            assert_eq!(old.pp, new.pp);
            assert_eq!(old.pp_ups, new.pp_ups);
            assert_eq!(old.total_exp, new.total_exp);
            assert_eq!(old.stat_exp, new.stat_exp);
        }
        assert_eq!(g.save_data.game_data.player_money, 3001);
    });
}

#[test]
fn field_poison_blackout_text_owns_ack_before_heal_money_and_warp() {
    on_game_stack(|| {
        let mut g = game(&[(1, true), (1, true)]);
        begin(&mut g);
        acknowledge(&mut g);
        acknowledge(&mut g);
        assert_eq!(g.overworld.poison_flow, Some(FieldPoisonFlow::Blackout));
        assert_eq!(hp(&g), vec![0, 0]);
        for _ in 0..200 {
            g.update(&InputState::new());
        }
        assert_eq!(hp(&g), vec![0, 0]);
        assert_eq!(g.save_data.game_data.player_money, 3001);
        assert!(g.overworld.pending_warp.is_none());
        assert_eq!(g.overworld.state.current_map, MapId::RedsHouse2F);
        acknowledge(&mut g);
        assert_eq!(hp(&g), vec![31, 31]);
        assert_eq!(g.save_data.game_data.player_money, 1500);
        assert!(g.overworld.pending_warp.is_some());
        assert!(g.overworld.poison_flow.is_none());
        for _ in 0..100 {
            g.update(&InputState::new());
        }
        assert_eq!(g.overworld.state.current_map, MapId::PalletTown);
        assert_eq!(g.save_data.game_data.player_money, 1500);
    });
}

#[test]
fn field_poison_flash_has_four_render_frames_and_preserves_obj_white() {
    on_game_stack(|| {
        let mut g = game(&[(3, true), (31, false)]);
        let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);
        let mut before = FrameBuffer::new(config.clone(), Rgba::WHITE);
        crate::render::draw_overworld(
            &mut g.overworld,
            &mut g.resources,
            &mut before,
            pokered_core::game_state::Lang::En,
        );
        assert!(
            g.resources.is_some(),
            "composed map and OBJ assets are required"
        );
        begin(&mut g);
        let mut cached = FrameBuffer::new(config.clone(), Rgba::WHITE);
        let mut cache = crate::render::OverworldBackgroundCache::new(160, 144);
        for remaining in [4, 3, 2, 1, 0] {
            let active = g
                .overworld
                .poison_flow
                .is_some_and(|flow| flow.bg_flash_active());
            assert_eq!(active, remaining != 0);
            assert_eq!(
                g.audio.as_ref().unwrap().is_sfx_playing(),
                remaining == 0,
                "emulated SFX starts only after BGP has been restored"
            );
            let mut full = FrameBuffer::new(config.clone(), Rgba::WHITE);
            crate::render::draw_overworld(
                &mut g.overworld,
                &mut g.resources,
                &mut full,
                pokered_core::game_state::Lang::En,
            );
            crate::render::draw_overworld_cached(
                &mut g.overworld,
                &mut g.resources,
                &mut cached,
                pokered_core::game_state::Lang::En,
                &mut cache,
            );
            assert_eq!(
                full.packed(),
                cached.packed(),
                "cached effect/restore frame {remaining}"
            );
            assert_eq!(full.display_palette(), cached.display_palette());
            if active {
                let mut dimmed_bg_white = 0;
                let mut unchanged_obj_white = 0;
                for y in 0..144 {
                    for x in 0..160 {
                        let old = before.get_pixel(x, y).unwrap();
                        let new = full.get_pixel(x, y).unwrap();
                        if old == Rgba::WHITE && new == Rgba::rgb(0x55, 0x55, 0x55) {
                            dimmed_bg_white += 1;
                        }
                        if old == Rgba::WHITE && new == Rgba::WHITE {
                            unchanged_obj_white += 1;
                        }
                        if old != Rgba::WHITE {
                            assert_eq!(new, old, "only BGP color 0 changes at {x},{y}");
                        }
                    }
                }
                assert!(dimmed_bg_white > 100);
                assert!(
                    unchanged_obj_white > 0,
                    "OBP white must not be recolored by a global palette"
                );
                assert_eq!(g.state.screen, GameScreen::Overworld);
                assert_eq!(
                    (g.overworld.state.player.x, g.overworld.state.player.y),
                    (3, 6)
                );
            } else {
                assert_eq!(full.packed(), before.packed(), "normal BG restored");
            }
            assert_eq!(hp(&g), vec![2, 31]);
            if remaining != 0 {
                g.update(&buttons(&[GbButton::Right, GbButton::Start]));
            }
        }
    });
}

#[cfg(feature = "debug-server")]
#[test]
fn field_poison_control_ready_waits_until_the_flash_caller_returns() {
    on_game_stack(|| {
        let mut g = game(&[(3, true), (31, false)]);
        assert!(g.debug_condition_met("control_ready"));
        begin(&mut g);
        for _ in 0..4 {
            assert!(g.overworld.pending_dialogue.is_none());
            assert!(g.overworld.poison_flow.is_some());
            assert!(!g.debug_condition_met("control_ready"));
            g.update(&InputState::new());
        }
        assert!(g.overworld.poison_flow.is_none());
        assert!(g.debug_condition_met("control_ready"));
        assert_eq!(hp(&g), vec![2, 31]);
    });
}

#[test]
fn field_poison_snapshot_resumes_next_slot_once_and_accepts_legacy_missing_field() {
    on_game_stack(|| {
        let mut g = game(&[(1, true), (1, true), (31, false)]);
        begin(&mut g);
        let party = g.save_data.party.clone();
        let snap = OverworldSnapshot::capture(&g.overworld);
        let mut value = serde_json::to_value(&snap).unwrap();
        let roundtrip: OverworldSnapshot = serde_json::from_value(value.clone()).unwrap();
        acknowledge(&mut g);
        acknowledge(&mut g);
        let expected = g.save_data.party.clone();
        g.save_data.party = party;
        roundtrip.restore_into(&mut g.overworld);
        assert_eq!(hp(&g), vec![0, 1, 31]);
        acknowledge(&mut g);
        acknowledge(&mut g);
        assert_eq!(g.save_data.party.to_vec(), expected.to_vec());
        assert!(g.overworld.poison_flow.is_none());
        value.as_object_mut().unwrap().remove("poison_flow");
        let legacy: OverworldSnapshot = serde_json::from_value(value).unwrap();
        assert!(legacy.poison_flow.is_none());
    });
}

#[test]
fn field_poison_soft_reset_drops_its_dialogue_and_continuation() {
    on_game_stack(|| {
        let mut g = game(&[(1, true), (31, false)]);
        begin(&mut g);
        let missing = std::env::temp_dir().join(format!(
            "pokered-poison-896-missing-{}.sav",
            std::process::id()
        ));
        assert!(!missing.exists());
        g.save_path = Some(missing);
        g.soft_reset();
        assert_eq!(g.state.screen, GameScreen::TitleScreen);
        assert!(g.overworld.poison_flow.is_none());
        assert!(g.overworld.pending_dialogue.is_none());
        assert_eq!(g.save_data.game_data.player_money, 3001);
    });
}

#[test]
fn field_poison_partial_blackout_snapshot_does_not_settle_early_or_twice() {
    on_game_stack(|| {
        let mut g = game(&[(1, true)]);
        begin(&mut g);
        acknowledge(&mut g);
        wait_for_prompt(&mut g);
        assert_eq!(g.overworld.poison_flow, Some(FieldPoisonFlow::Blackout));
        let saved_party = g.save_data.clone();
        let snap = OverworldSnapshot::capture(&g.overworld);
        acknowledge(&mut g);
        assert_eq!(g.save_data.game_data.player_money, 1500);
        g.save_data = saved_party;
        snap.restore_into(&mut g.overworld);
        assert_eq!(hp(&g), vec![0]);
        assert_eq!(g.save_data.game_data.player_money, 3001);
        assert!(g.overworld.pending_warp.is_none());
        acknowledge(&mut g);
        assert_eq!(hp(&g), vec![31]);
        assert_eq!(g.save_data.game_data.player_money, 1500);
    });
}

#[test]
fn field_poison_flash_snapshot_preserves_remaining_frames_and_sound_boundary() {
    on_game_stack(|| {
        let mut g = game(&[(3, true), (31, false)]);
        begin(&mut g);
        g.update(&InputState::new());
        g.update(&InputState::new());
        assert_eq!(
            g.overworld.poison_flow,
            Some(FieldPoisonFlow::Flash {
                frames_remaining: 2
            })
        );
        let snap = OverworldSnapshot::capture(&g.overworld);
        for _ in 0..2 {
            g.update(&InputState::new());
        }
        assert!(g.overworld.poison_flow.is_none());
        // A field snapshot restores into a fresh caller; audio is a
        // frontend sequencer, not a second party or field-loop owner.
        let mut restored = game(&[(3, true), (31, false)]);
        restored.save_data = g.save_data.clone();
        snap.restore_into(&mut restored.overworld);
        restored.update(&InputState::new());
        assert_eq!(
            restored.overworld.poison_flow,
            Some(FieldPoisonFlow::Flash {
                frames_remaining: 1
            })
        );
        assert!(!restored.audio.as_ref().unwrap().is_sfx_playing());
        restored.update(&InputState::new());
        assert!(restored.overworld.poison_flow.is_none());
        assert!(restored.audio.as_ref().unwrap().is_sfx_playing());
        assert_eq!(
            hp(&restored),
            vec![2, 31],
            "snapshot continuation does not damage again"
        );
    });
}

#[test]
fn field_poison_ordinary_sram_load_clears_old_party_continuation() {
    on_game_stack(|| {
        struct PrivateSave(std::path::PathBuf);
        impl Drop for PrivateSave {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let dir = std::env::temp_dir().join(format!(
            "pokered-field-poison-load-896-{}",
            std::process::id()
        ));
        assert!(!dir.exists());
        std::fs::create_dir(&dir).unwrap();
        let private = PrivateSave(dir);
        let path = private.0.join("ordinary.sav");
        let mut g = game(&[(1, true), (31, false)]);
        g.save_to_path(&path).unwrap();
        approach_fourth_step_door(&mut g);
        assert_eq!(hp(&g), vec![0, 31]);
        assert!(g.overworld.pending_dialogue.is_some());
        g.load_from_path(&path).unwrap();
        assert_eq!(hp(&g), vec![1, 31]);
        assert_eq!(
            g.save_data.party.get(0).unwrap().status,
            StatusCondition::Poison
        );
        assert!(g.overworld.poison_flow.is_none());
        assert!(g.overworld.pending_dialogue.is_none());
        for _ in 0..10 {
            g.update(&InputState::new());
        }
        assert_eq!(hp(&g), vec![1, 31]);
        assert!(g.overworld.pending_warp.is_none());
        assert_eq!(g.save_data.game_data.player_money, 3001);
    });
}

fn queue_step_encounter(g: &mut PokemonGame) {
    g.overworld.pending_wild_encounter = Some(pokered_core::overworld::PendingWildEncounter {
        species: Species::Rattata,
        level: 3,
        old_man: false,
        hooked: false,
    });
}

#[test]
fn field_poison_encounter_waits_for_four_flash_renders_then_uses_damaged_party() {
    on_game_stack(|| {
        let mut g = game(&[(10, true), (31, false)]);
        queue_step_encounter(&mut g);
        begin(&mut g);
        for remaining in [4, 3, 2, 1] {
            assert_eq!(g.state.screen, GameScreen::Overworld);
            assert_eq!(g.overworld.poison_flow, Some(FieldPoisonFlow::Flash { frames_remaining: remaining }));
            assert!(g.overworld.pending_wild_encounter.is_some());
            assert_eq!(hp(&g), vec![9, 31]);
            g.update(&InputState::new());
        }
        assert!(g.overworld.poison_flow.is_none());
        assert!(g.overworld.pending_wild_encounter.is_none());
        assert_eq!(g.state.screen, GameScreen::Battle);
        let live = g.battle.battle_state.as_ref().unwrap();
        assert_eq!(live.player.party[0].hp, 9);
        assert_eq!(live.player.party[1].hp, 31);
    });
}

#[test]
fn field_poison_encounter_waits_for_faint_ack_and_sends_healthy_member() {
    on_game_stack(|| {
        let mut g = game(&[(1, true), (31, false)]);
        queue_step_encounter(&mut g);
        begin(&mut g);
        wait_for_prompt(&mut g);
        assert_eq!(g.state.screen, GameScreen::Overworld);
        assert_eq!(hp(&g), vec![0, 31]);
        assert!(g.overworld.pending_wild_encounter.is_some());
        acknowledge(&mut g);
        assert!(g.overworld.poison_flow.is_none());
        assert_eq!(g.state.screen, GameScreen::Battle);
        let live = g.battle.battle_state.as_ref().unwrap();
        assert_eq!(live.player.party[0].hp, 0);
        assert_eq!(live.player.party[1].hp, 31);
        assert_eq!(live.player.active_mon().hp, 31);
    });
}

#[test]
fn field_poison_encounter_is_discarded_before_blackout_ack_and_never_reappears() {
    on_game_stack(|| {
        let mut g = game(&[(1, true), (1, true)]);
        queue_step_encounter(&mut g);
        begin(&mut g);
        acknowledge(&mut g);
        assert!(g.overworld.pending_wild_encounter.is_some());
        acknowledge(&mut g);
        assert_eq!(g.overworld.poison_flow, Some(FieldPoisonFlow::Blackout));
        assert!(g.overworld.pending_wild_encounter.is_none());
        assert_eq!(hp(&g), vec![0, 0]);
        assert_eq!(g.state.screen, GameScreen::Overworld);
        acknowledge(&mut g);
        for _ in 0..120 {
            g.update(&InputState::new());
            assert_ne!(g.state.screen, GameScreen::Battle);
        }
        assert_eq!(g.overworld.state.current_map, MapId::PalletTown);
        assert_eq!(hp(&g), vec![31, 31]);
        assert_eq!(g.save_data.game_data.player_money, 1500);
    });
}

#[test]
fn field_poison_pending_trainer_waits_for_caller_then_resumes_with_committed_party() {
    on_game_stack(|| {
        use pokered_data::trainer_data::{make_trainer_id, TrainerClass};
        for initial_hp in [1, 10] {
            let mut g = game(&[(initial_hp, true), (31, false)]);
            g.overworld.pending_trainer_battle = Some(
                pokered_core::overworld::PendingTrainerBattle {
                    trainer_id: make_trainer_id(TrainerClass::BugCatcher, 1),
                    npc_index: 7,
                    end_battle_text: Some("The trainer's own victory text.".into()),
                    rival_triplet_base: None,
                },
            );
            begin(&mut g);
            assert_eq!(g.state.screen, GameScreen::Overworld);
            assert!(g.overworld.pending_trainer_battle.is_some());
            if initial_hp == 1 {
                wait_for_prompt(&mut g);
                assert_eq!(g.state.screen, GameScreen::Overworld);
                assert_eq!(hp(&g), vec![0, 31]);
                assert!(g.overworld.pending_trainer_battle.is_some());
                acknowledge(&mut g);
            } else {
                for remaining in [4, 3, 2, 1] {
                    assert_eq!(g.state.screen, GameScreen::Overworld);
                    assert_eq!(g.overworld.poison_flow, Some(FieldPoisonFlow::Flash { frames_remaining: remaining }));
                    assert!(g.overworld.pending_trainer_battle.is_some());
                    g.update(&InputState::new());
                }
            }
            assert!(g.overworld.poison_flow.is_none());
            assert!(g.overworld.pending_trainer_battle.is_none());
            assert_eq!(g.state.screen, GameScreen::Battle);
            assert!(!g.battle.is_wild);
            assert_eq!(g.battle.trainer_class, Some(TrainerClass::BugCatcher));
            assert_eq!(g.battle.trainer_npc_index, Some(7));
            assert_eq!(g.battle.end_battle_text.as_deref(), Some("The trainer's own victory text."));
            let live = g.battle.battle_state.as_ref().unwrap();
            assert_eq!(live.player.party[0].hp, initial_hp - 1);
            assert_eq!(live.player.party[1].hp, 31);
            assert_eq!(live.player.active_mon().hp, if initial_hp == 1 { 31 } else { 9 });
        }
    });
}
