//! Map-entry scripts have one direct caller, independent of position triggers.
use super::screen::{OverworldGameDataRequest, PendingWarp};
use super::{script_bridge::ScriptEffect, Direction, OverworldInput, OverworldScreen};
use crate::alloc_prelude::*;
use dotzuki_engine::metatile::TriggerType;
use dotzuki_engine::trigger_manager::Trigger;
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};

fn neutral() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

fn money(screen: &mut OverworldScreen<PokemonRedData>) -> Vec<u32> {
    screen.game_data_requests.drain(..).filter_map(|request| match request {
        OverworldGameDataRequest::GiveMoney { amount } => Some(amount),
        _ => None,
    }).collect()
}

fn ordinary_frames(screen: &mut OverworldScreen<PokemonRedData>, frames: usize) -> Vec<u32> {
    let mut trace = Vec::new();
    for _ in 0..frames {
        screen.update_frame(neutral());
        trace.extend(money(screen));
    }
    trace
}

fn counter_source(key: &str) -> String {
    format!("game_scene {key} {{\n @load {{\n giveMoney(37)\n giveMoney(83)\n }}\n}}")
}

fn register_counter(screen: &mut OverworldScreen<PokemonRedData>, map: MapId) {
    let key = super::script_bridge::map_id_to_script_key(map);
    let source = counter_source(&key);
    let config = format!(r#"{{"onLoad":"{key}OnLoad"}}"#);
    screen.reload_scene_with_config(&key, &source, Some(&config)).unwrap();
}

fn counter_screen() -> OverworldScreen<PokemonRedData> {
    let mut screen = OverworldScreen::new(MapId::RedsHouse2F, None, PokemonRedData);
    screen.state.player.x = 3;
    screen.state.player.y = 6;
    // Function-only registration does not start or discard an entry script.
    screen.reload_scene_source("RedsHouse2F", &counter_source("RedsHouse2F")).unwrap();
    screen.load_map_script_ex(MapId::RedsHouse2F, false);
    screen
}

#[test]
fn direct_load_runs_non_idempotent_effects_once_across_ordinary_loops() {
    let mut screen = counter_screen();
    screen.run_on_load();
    assert_eq!(ordinary_frames(&mut screen, 32), [37, 83]);
    assert!(screen.script_engine.is_idle());
    assert!(screen.active_script_effect.is_none());
    assert!(ordinary_frames(&mut screen, 32).is_empty());
}

#[test]
fn rebuilding_functions_without_load_never_defers_load_to_on_enter() {
    let mut screen = counter_screen();
    assert!(ordinary_frames(&mut screen, 32).is_empty());
    screen.load_map_script_ex(MapId::RedsHouse2F, false);
    assert!(ordinary_frames(&mut screen, 32).is_empty());
}

#[test]
fn explicit_same_map_hot_reload_runs_load_once() {
    let mut screen = counter_screen();
    register_counter(&mut screen, MapId::RedsHouse2F);
    assert_eq!(ordinary_frames(&mut screen, 32), [37, 83]);
}

#[test]
fn cinnabar_direct_load_and_first_ordinary_loop_apply_exactly_six_gate_writes() {
    let mut screen = OverworldScreen::new(MapId::CinnabarGym, None, PokemonRedData);
    screen.state.player.x = 15;
    screen.state.player.y = 8;
    screen.state.player.facing = Direction::Down;
    screen.run_on_load();
    let mut writes = Vec::new();
    for _ in 0..6 {
        match screen.active_script_effect {
            Some(ScriptEffect::ReplaceTileBlock { x, y, block_id }) => writes.push((x, y, block_id)),
            ref other => panic!("gate load stopped early: {other:?}"),
        }
        screen.update_frame(neutral());
    }
    assert_eq!(writes, [(9, 3, 84), (6, 3, 84), (6, 6, 84),
                        (3, 8, 95), (2, 6, 84), (2, 3, 84)]);
    assert!(screen.script_engine.is_idle());
    assert!(screen.active_script_effect.is_none());
    // Cross the first ordinary field-loop check, where the old OnEnter
    // binding started the same six commands again after Continue.
    for _ in 0..16 {
        screen.update_frame(neutral());
        assert!(screen.active_script_effect.is_none(), "gate initialization replayed");
        assert!(screen.script_engine.is_idle());
    }
    assert_eq!((screen.state.player.x, screen.state.player.y), (15, 8));
}

fn warp(screen: &mut OverworldScreen<PokemonRedData>, map: MapId, x: u8, y: u8) {
    screen.pending_warp = Some(PendingWarp {
        dest_map: map, dest_x: x, dest_y: y,
        save_last_map: false, arrival_spin: false,
    });
    screen.commit_pending_warp();
}

#[test]
fn warp_and_same_map_reentry_each_run_the_load_phase_once() {
    let mut screen = counter_screen();
    for _ in 0..2 {
        warp(&mut screen, MapId::RedsHouse2F, 3, 6);
        assert_eq!(ordinary_frames(&mut screen, 32), [37, 83]);
    }
    warp(&mut screen, MapId::PalletTown, 10, 10);
    ordinary_frames(&mut screen, 32);
    warp(&mut screen, MapId::RedsHouse2F, 3, 6);
    assert_eq!(ordinary_frames(&mut screen, 32), [37, 83]);
}

#[test]
fn crossing_a_real_connection_runs_the_destination_load_once() {
    let mut screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    register_counter(&mut screen, MapId::Route1);
    screen.set_event_flag_live(pokered_data::event_flags::EventFlag::EVENT_GOT_STARTER);
    screen.state.player.x = 10;
    screen.state.player.y = 0;
    screen.state.player.facing = Direction::Up;
    screen.state.encounter_cooldown = 255;
    let up = OverworldInput::new(true, false, false, false, false, false, false, false);
    let mut trace = Vec::new();
    for _ in 0..64 {
        screen.update_frame(up);
        trace.extend(money(&mut screen));
        if screen.state.current_map == MapId::Route1 { break; }
    }
    assert_eq!(screen.state.current_map, MapId::Route1);
    trace.extend(ordinary_frames(&mut screen, 32));
    assert_eq!(trace, [37, 83]);
}

#[test]
fn explicit_post_battle_refresh_still_runs_once_and_respects_suspended_battle() {
    let mut screen = counter_screen();
    screen.run_on_load();
    assert_eq!(ordinary_frames(&mut screen, 32), [37, 83]);
    screen.rerun_map_on_load_script();
    assert_eq!(ordinary_frames(&mut screen, 32), [37, 83]);
    screen.script_awaiting_battle = true;
    screen.rerun_map_on_load_script();
    assert!(screen.active_script_effect.is_none());
    assert!(screen.script_engine.is_idle());
}

#[test]
fn restore_removes_only_legacy_load_binding_and_preserves_every_manager_field() {
    for (prior_map, legacy_fired) in [
        ("RedsHouse2F", false), ("RedsHouse2F", true),
        ("PalletTown", false), ("PalletTown", true),
    ] {
        let mut screen = counter_screen();
        screen.run_on_load();
        assert_eq!(ordinary_frames(&mut screen, 32), [37, 83]);
        screen.trigger_manager.check_triggers(prior_map, 9, 13);
        let handler = screen.map_script_config.on_load().unwrap().to_string();
        let id = format!("enter_{handler}");
        let mut legacy = Trigger::single_tile(&id, "RedsHouse2F", TriggerType::OnEnter,
            3, 6, &handler, true);
        legacy.fired = legacy_fired;
        let mut used = Trigger::single_tile("used_interact", "RedsHouse2F", TriggerType::OnInteract,
            99, 99, "usedTalk", true);
        used.fired = true;
        let mut used_step = Trigger::single_tile("used_step", "RedsHouse2F", TriggerType::OnStep,
            99, 99, "usedStep", true);
        used_step.fired = true;
        let mut manual = Trigger::single_tile("manual_entry", "RedsHouse2F", TriggerType::OnEnter,
            99, 99, &handler, false);
        manual.width = 3;
        manual.height = 5;
        // Similar IDs, other handlers/types/maps and hand-authored OnEnter
        // bindings are not the old automatic load binding.
        screen.trigger_manager.add_triggers([
            used,
            used_step,
            Trigger::single_tile(&id, "PalletTown", TriggerType::OnEnter, 99, 99, &handler, true),
            legacy.clone(),
            Trigger::single_tile(format!("{id}_extra"), "RedsHouse2F", TriggerType::OnEnter, 99, 99, &handler, true),
            Trigger::single_tile(&id, "RedsHouse2F", TriggerType::OnInteract, 99, 99, &handler, true),
            Trigger::single_tile(&id, "RedsHouse2F", TriggerType::OnEnter, 99, 99, "otherHandler", true),
            manual,
        ]);
        let mut expected = serde_json::to_value(&screen.trigger_manager).unwrap();
        let entries = expected["triggers"].as_array_mut().unwrap();
        let serialized_legacy = serde_json::to_value(&legacy).unwrap();
        entries.retain(|entry| entry != &serialized_legacy);
        let saved = crate::snapshot::OverworldSnapshot::capture(&screen);
        let bytes = serde_json::to_vec(&saved).unwrap();
        let decoded: crate::snapshot::OverworldSnapshot = serde_json::from_slice(&bytes).unwrap();
        decoded.restore_into(&mut screen);
        // Whole manager equality covers trigger ordering/all struct fields,
        // fired/oneShot state AND private prev_player_map/X/Y, not just IDs.
        assert_eq!(serde_json::to_value(&screen.trigger_manager).unwrap(), expected);
        assert!(ordinary_frames(&mut screen, 32).is_empty());
        assert!(screen.script_engine.is_idle());
    }
}

#[cfg(not(feature = "script-boa"))]
#[test]
fn native_snapshot_mid_load_resumes_remaining_effect_without_restarting() {
    let mut screen = counter_screen();
    screen.run_on_load();
    screen.update_frame(neutral());
    assert_eq!(money(&mut screen), [37]);
    assert!(matches!(screen.active_script_effect, Some(ScriptEffect::GiveMoney { amount: 83 })));
    let saved = crate::snapshot::OverworldSnapshot::capture(&screen);
    saved.restore_into(&mut screen);
    assert_eq!(ordinary_frames(&mut screen, 32), [83]);
}
