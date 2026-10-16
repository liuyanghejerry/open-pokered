//! Common external witness: ordinary cold SRAM, fixed physical inputs, public Game.
//! Link the SAME bytes separately to reviewed actual-base and integrated-head App libs.
//! No debug RPC, restore, warp, injected battle/result, or runtime seed change.
//! Usage: witness SAV FIXED_BUTTONS NEW_OUTPUT before|after main|reload
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use dotzuki_engine::render_config::RenderConfig;
use pokered_app::PokemonGame;
use pokered_audio::sfx_data::SfxId;
use pokered_core::data::maps::MapId;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::GameScreen;
use pokered_core::overworld::Direction;
use pokered_core::save::{sram_import::import_sram, SaveData};
use pokered_core::save_menu::SavePhase;
use pokered_core::snapshot::{BattleSnapshot, OverworldSnapshot};
use pokered_core::start_menu::StartMenuItem;
use pokered_data::species::Species;
use pokered_renderer::input::{GbButton, InputState};
use pokered_renderer::{FrameBuffer, Rgba};
use serde_json::{json, Value};

const ORIGINAL_FRAMES: usize = 7000;
const MAIN_FRAMES: usize = 7928;
const RELOAD_FRAMES: usize = 3972;
const ROUTE_ANCHORS_JSON: &str = include_str!("route-anchors.json");
const ROSTER_JSON: &str = include_str!("roster-oracle.json");
const TRAINER_FLAG: &str = "EVENT_BEAT_SILPH_CO_GIOVANNI";
const PCM_SAMPLES: usize = 1470; // interleaved stereo, 44.1 kHz / 60 updates

fn check(ok: bool, frame: usize, message: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("frame {frame}: {message}"))
    }
}
fn line(w: &mut BufWriter<File>, value: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *w, value).map_err(|e| e.to_string())?;
    w.write_all(b"\n").map_err(|e| e.to_string())?;
    w.flush().map_err(|e| e.to_string())
}
fn writer(path: PathBuf) -> Result<BufWriter<File>, String> {
    File::options()
        .create_new(true)
        .write(true)
        .open(path)
        .map(BufWriter::new)
        .map_err(|e| e.to_string())
}
fn button(s: &str) -> Result<GbButton, String> {
    match s {
        "A" => Ok(GbButton::A),
        "B" => Ok(GbButton::B),
        "START" => Ok(GbButton::Start),
        "SELECT" => Ok(GbButton::Select),
        "UP" => Ok(GbButton::Up),
        "DOWN" => Ok(GbButton::Down),
        "LEFT" => Ok(GbButton::Left),
        "RIGHT" => Ok(GbButton::Right),
        _ => Err(format!("unknown physical button {s:?}")),
    }
}
fn audio_state(game: &PokemonGame) -> Result<Value, String> {
    let audio = game
        .audio
        .as_ref()
        .ok_or("no-device logical PCM owner absent")?;
    let manager = audio.manager.lock().map_err(|e| e.to_string())?;
    Ok(json!({
        "id": manager.sequencer.current_sfx_id,
        "busy": manager.is_sfx_playing(),
        "channels": manager.sequencer.channels.iter().enumerate().map(|(i,c)| json!({
            "index": i, "active": c.active, "sound_id": c.sound_id,
            "ptr": c.ptr, "delay_counter": c.delay_counter, "delay_frac": c.delay_frac,
            "note_length": c.note_length, "frequency": c.frequency,
        })).collect::<Vec<_>>(),
    }))
}
fn base_observation(game: &PokemonGame) -> Result<Value, String> {
    // Immutable capture only. NEVER call restore. Select existing common fields,
    // avoiding a candidate-only typed cue API or any claim of complete RNG state.
    let snapshot = BattleSnapshot::capture(&game.battle);
    let live = game.battle.battle_state.as_ref().map(|bs| {
        json!({
            "player_party": bs.player.party.iter().map(|m| json!({
                "species": format!("{:?}",m.species), "level":m.level,
                "hp":m.hp, "max_hp":m.max_hp, "pp":m.pp,
                "moves":m.moves.iter().map(|v|format!("{:?}",v)).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "enemy_party": bs.enemy.party.iter().map(|m| json!({
                "species":format!("{:?}",m.species),"level":m.level,
                "hp":m.hp,"max_hp":m.max_hp,"pp":m.pp,
            })).collect::<Vec<_>>(),
            "player_hp":bs.player.active_mon().hp, "enemy_hp":bs.enemy.active_mon().hp,
        })
    });
    Ok(json!({
        "frame":game.frame_count, "screen":format!("{:?}",game.state.screen),
        "map":format!("{:?}",game.overworld.state.current_map),
        "position":[game.overworld.state.player.x,game.overworld.state.player.y],
        "facing":format!("{:?}",game.overworld.state.player.facing),
        "movement":format!("{:?}",game.overworld.state.player.movement_state),
        "money":game.save_data.game_data.player_money,
        "coins":game.save_data.game_data.player_coins,
        "party":&game.save_data.party, "bag":&game.save_data.game_data.bag,
        "flags":game.overworld.script_flags(), "script_idle":game.overworld.script_engine_idle(),
        "battle_count":game.battle_count, "battle_phase":format!("{:?}",game.battle.phase),
        "battle_message":&game.battle.current_message,
        "text_publication":snapshot.text_publication,
        "settlement":snapshot.settlement, "battle_live":live,
        "audio":audio_state(game)?,
    }))
}

fn observation(game: &PokemonGame) -> Result<Value, String> {
    let mut s = base_observation(game)?;
    s["active_script_effect"] = json!(game.overworld.active_script_effect_value());
    s["npcs"] = json!(game
        .overworld
        .npc_states
        .iter()
        .enumerate()
        .map(|(slot, n)| json!({"slot":slot,"runtime":n}))
        .collect::<Vec<_>>());
    s["live_toggleable_object_flags"] = json!(game.overworld.toggleable_object_flags().to_vec());
    s["save_toggleable_object_flags"] =
        json!(game.save_data.game_data.toggleable_object_flags.to_vec());
    s["save_event_flags"] = json!(game.save_data.game_data.event_flags.to_vec());
    s["dialogue_display"] = json!(game
        .overworld
        .pending_dialogue
        .as_ref()
        .and_then(|d| d.get_display_text())
        .map(|(a, b)| (a.to_string(), b.to_string())));
    // Immutable public capture only; never restore or mutate it. The native
    // VM/host flags are serialized by the existing supported snapshot owner.
    let native = serde_json::to_value(OverworldSnapshot::capture(&game.overworld).script_engine)
        .map_err(|e| e.to_string())?;
    s["native_vm_state"] = native["state"].clone();
    s["native_vm_flags"] = native
        .pointer("/interp/host/flags")
        .cloned()
        .unwrap_or(Value::Null);
    s["live_event_flags"] = json!(game.overworld.unified_flags().as_bytes().to_vec());
    s["runtime_extras_count"] = json!(game.overworld.unified_flags().extras().len());
    s["start_menu"] = json!({"cursor":game.start_menu.cursor(),"item":format!("{:?}",game.start_menu.current_item()),"has_pokedex":game.start_menu.has_pokedex(),"saved_cursor":game.start_menu.saved_cursor()});
    s["save_menu"] = json!({"phase":format!("{:?}",game.save_menu.phase),"cursor":format!("{:?}",game.save_menu.cursor),"has_previous":game.save_menu.has_previous_save,"is_different_player":game.save_menu.is_different_player});
    s["departure_palette"] = departure_palette(&s["active_script_effect"])?;
    Ok(s)
}
fn fnv(bytes: &[u8]) -> String {
    let mut h = 0xcbf29ce484222325u64;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn departure_palette(effect: &Value) -> Result<Value, String> {
    if effect["effect"] != "SilphRocketDeparture" {
        return Ok(Value::Null);
    }
    let state = &effect["state"];
    let phase = state["phase"].as_str().ok_or("departure phase missing")?;
    let step = state["step"].as_u64().ok_or("departure step missing")? as usize;
    if step > 3 {
        return Err("departure step out of range".into());
    }
    let index = match phase {
        "FadeOut" => 3 - step,
        "Settle" => 0,
        "FadeIn" => step,
        "Done" => 3,
        _ => return Err(format!("unknown departure phase {phase}")),
    };
    let p = pokered_renderer::transition::FADE_PALETTES[index];
    Ok(
        json!({"phase":phase,"step":step,"frames_remaining":state["frames_remaining"],"index":index,"BGP":p.bgp,"OBP0":p.obp0,"OBP1":p.obp1,"source":"actual typed ScriptEffect debug serialization + pinned original FadePalette table; no pixel inference"}),
    )
}
fn bit(flags: &[u8], n: usize) -> bool {
    flags[n / 8] & (1 << (n % 8)) != 0
}
fn roster() -> Result<Vec<Value>, String> {
    let value: Value = serde_json::from_str(ROSTER_JSON).map_err(|e| e.to_string())?;
    let rows = value["roster"].as_array().ok_or("roster missing")?.clone();
    if rows.len() != 46 {
        return Err("independent roster must contain46 calls".into());
    }
    Ok(rows)
}
fn assert_roster(
    game: &PokemonGame,
    observation: &Value,
    applied: bool,
    explicit_flags: bool,
    frame: usize,
) -> Result<(), String> {
    for row in roster()? {
        let id = row["toggle"].as_str().ok_or("toggle missing")?;
        let n = row["bit"].as_u64().ok_or("bit missing")? as usize;
        let hidden = if applied {
            row["action"] == "hide"
        } else {
            row["original_initial_state"] == "OFF"
        };
        check(
            bit(game.overworld.toggleable_object_flags(), n) == hidden,
            frame,
            &format!("live original toggle {id}"),
        )?;
        if explicit_flags {
            for flags in [&observation["flags"], &observation["native_vm_flags"]] {
                check(
                    flags[format!("__OBJ_HIDDEN_{id}")] == hidden,
                    frame,
                    &format!("explicit hidden flag {id}"),
                )?;
                check(
                    flags[format!("__OBJ_SHOWN_{id}")] == !hidden,
                    frame,
                    &format!("explicit shown flag {id}"),
                )?;
            }
        }
    }
    if applied && game.overworld.state.current_map == MapId::SilphCo11F {
        for id in [1, 2] {
            let npc = game
                .overworld
                .npc_states
                .iter()
                .find(|n| n.text_id == id)
                .ok_or("untargeted Silph actor missing")?;
            check(npc.visible, frame, "president and assistant remain visible")?;
        }
        for id in [3, 4, 5] {
            let npc = game
                .overworld
                .npc_states
                .iter()
                .find(|n| n.text_id == id)
                .ok_or("current Silph actor missing")?;
            check(
                !npc.visible,
                frame,
                "all three original11F Rocket actors hidden",
            )?;
        }
    }
    if applied && game.overworld.state.current_map == MapId::SilphCo7F {
        for id in [5, 6, 7, 8] {
            let npc = game
                .overworld
                .npc_states
                .iter()
                .find(|n| n.text_id == id)
                .ok_or("original7F Rocket/Scientist actor missing")?;
            check(
                !npc.visible,
                frame,
                "all four original7F enemy actors remain hidden",
            )?;
        }
    }
    Ok(())
}
fn assert_route_anchor(game: &PokemonGame, s: &Value, frame: usize) -> Result<(), String> {
    let anchors: Vec<Value> =
        serde_json::from_str(ROUTE_ANCHORS_JSON).map_err(|e| e.to_string())?;
    if let Some(a) = anchors.iter().find(|a| a["frame"] == frame) {
        check(
            s["screen"] == "Overworld"
                && s["map"] == a["map"]
                && s["position"] == a["position"]
                && s["movement"] == "Idle"
                && s["script_idle"] == true,
            frame,
            "source-derived fixed route anchor",
        )?;
        if let Some(facing) = a["facing"].as_str() {
            check(
                s["facing"] == facing,
                frame,
                "ordinary Continue Down / final route Up facing",
            )?;
        }
        check(
            game.overworld.unified_flags().get_flag(TRAINER_FLAG)
                && s["departure_palette"].is_null(),
            frame,
            "route keeps liberation flag and has no departure owner",
        )?;
        assert_roster(game, s, true, true, frame)?;
        check(
            game.battle_count == 0 && game.save_data.game_data.player_money == 14059,
            frame,
            "normal re-entry without battle or money change",
        )?;
        let mon = game.save_data.party.get(0).ok_or("route party absent")?;
        check(
            mon.hp == 174 && mon.pp == [11, 25, 40, 15],
            frame,
            "route preserves actual saved HP/PP",
        )?;
    }
    Ok(())
}
fn assert_saved(save: &SaveData, frame: usize) -> Result<(), String> {
    let flag = pokered_data::event_flags::EventFlag::EVENT_BEAT_SILPH_CO_GIOVANNI as usize;
    check(
        bit(&save.game_data.event_flags, flag),
        frame,
        "physical SRAM has canonical Giovanni beat bit",
    )?;
    for row in roster()? {
        check(
            bit(
                &save.game_data.toggleable_object_flags,
                row["bit"].as_u64().unwrap() as usize,
            ) == (row["action"] == "hide"),
            frame,
            "physical SRAM contains complete original object roster",
        )?;
    }
    check(
        save.game_data.position.map_id == MapId::SilphCo11F as u8
            && save.game_data.position.x == 6
            && save.game_data.position.y == 13,
        frame,
        "physical saved position remains natural postbattle11F(6,13)",
    )?;
    check(
        save.game_data.player_money == 14059,
        frame,
        "normal win wallet14059",
    )?;
    let mon = save.party.get(0).ok_or("saved party missing")?;
    check(
        save.party.count() == 1
            && mon.species == Species::Blastoise
            && mon.level == 60
            && mon.hp == 174
            && mon.pp == [11, 25, 40, 15],
        frame,
        "physical battle outcome party/PP",
    )
}
fn expected_departure(relative: usize) -> (&'static str, usize, u64, usize) {
    match relative {
        0..=31 => (
            "FadeOut",
            relative / 8,
            (8 - relative % 8) as u64,
            3 - relative / 8,
        ),
        32..=34 => ("Settle", 3, (35 - relative) as u64, 0),
        35..=66 => (
            "FadeIn",
            (relative - 35) / 8,
            (8 - (relative - 35) % 8) as u64,
            (relative - 35) / 8,
        ),
        _ => unreachable!(),
    }
}
fn archive_save(save: &PathBuf, out: &PathBuf) -> Result<Value, String> {
    fs::create_dir(out).map_err(|e| e.to_string())?;
    let bytes = fs::read(save).map_err(|e| e.to_string())?;
    fs::write(out.join("ordinary.sav"), &bytes).map_err(|e| e.to_string())?;
    let companion = save.with_extension("script_flags.json");
    let mut metadata = json!({"SRAM_bytes":bytes.len(),"SRAM_fnv1a64":fnv(&bytes),"companion_exists":companion.exists()});
    if companion.exists() {
        let extra = fs::read(&companion).map_err(|e| e.to_string())?;
        let parsed: Value = serde_json::from_slice(&extra).map_err(|e| e.to_string())?;
        fs::write(out.join("ordinary.script_flags.json"), &extra).map_err(|e| e.to_string())?;
        metadata["companion_bytes"] = json!(extra.len());
        metadata["companion_fnv1a64"] = json!(fnv(&extra));
        metadata["companion_semantics"] = parsed;
    }
    Ok(metadata)
}
fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 5 {
        return Err(
            "expected PHYSICAL_SAV FIXED_BUTTONS NEW_OUTPUT before|after main|reload".into(),
        );
    }
    let fixture = PathBuf::from(&args[0]);
    let tape = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    let role = &args[3];
    let mode = &args[4];
    check(
        role == "before" || role == "after",
        0,
        "explicit provider role",
    )?;
    check(mode == "main" || mode == "reload", 0, "explicit run mode")?;
    let frame_budget = if mode == "main" {
        MAIN_FRAMES
    } else {
        RELOAD_FRAMES
    };
    check(!out.exists(), 0, "new output only")?;
    let original = fs::read(&fixture).map_err(|e| e.to_string())?;
    check(original.len() == 32768, 0, "actual physical SRAM32KiB")?;
    let parsed = import_sram(&original).map_err(|e| format!("physical input import: {e:?}"))?;
    if mode == "reload" {
        assert_saved(&parsed, 0)?;
    }
    let tape_bytes = fs::read(&tape).map_err(|e| e.to_string())?;
    let timeline: Vec<Vec<String>> =
        serde_json::from_slice(&tape_bytes).map_err(|e| e.to_string())?;
    check(
        timeline.len() == frame_budget,
        0,
        "frozen literal frame budget",
    )?;
    for keys in &timeline {
        check(keys.len() <= 1, 0, "zero or one ordinary held input")?;
        for key in keys {
            button(key)?;
        }
    }
    fs::create_dir(&out).map_err(|e| e.to_string())?;
    fs::create_dir(out.join("png")).map_err(|e| e.to_string())?;
    let save = out.join("ordinary.sav");
    fs::write(&save, &original).map_err(|e| e.to_string())?;
    let source_companion = fixture.with_extension("script_flags.json");
    let initial_companion = if source_companion.exists() {
        Some(fs::read(&source_companion).map_err(|e| e.to_string())?)
    } else {
        None
    };
    if let Some(bytes) = &initial_companion {
        fs::write(save.with_extension("script_flags.json"), bytes).map_err(|e| e.to_string())?;
    }
    fs::write(out.join("fixed-buttons.json"), &tape_bytes).map_err(|e| e.to_string())?;
    let mut states = writer(out.join("states.jsonl"))?;
    let mut inputs = writer(out.join("inputs.jsonl"))?;
    let mut events = writer(out.join("events.jsonl"))?;
    let mut captures = writer(out.join("captures.jsonl"))?;
    let mut frames = writer(out.join("framebuffers-packed.bin"))?;
    let mut frame_meta = writer(out.join("framebuffers.jsonl"))?;
    let mut game = PokemonGame::new_with_options(
        GameVersion::Red,
        Some(save.clone()),
        None,
        None,
        false,
        None,
        false,
        true,
        None,
    );
    game.set_seed(42); // Exactly one explicit public preboot seed, before observation/update.
    let mut input = InputState::new();
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut pcm = vec![0f32; PCM_SAMPLES];
    let mut previous = observation(&game)?;
    check(game.frame_count == 0, 0, "ordinary constructor frame0")?;
    line(&mut states, &json!({"frame":0,"state":previous}))?;
    let mut packed_offset = 0usize;
    let mut captured = 0usize;
    let mut departure_start = None;
    let mut departure_completed = None;
    let mut first_battle = None;
    let mut first_beat = None;
    let mut first_disk_write = None;
    let mut last_disk = original.clone();
    let mut black_frames = 0usize;
    let mut last_phase = String::new();
    let mut saw_save_sfx = false;
    for (index, keys) in timeline.iter().enumerate() {
        let f = index + 1;
        input.begin_frame();
        let held = keys
            .iter()
            .map(|key| button(key))
            .collect::<Result<Vec<_>, _>>()?;
        for key in GbButton::ALL {
            if held.contains(&key) {
                input.press(key);
            } else {
                input.release(key);
            }
        }
        line(
            &mut inputs,
            &json!({"frame":f,"buttons":keys,"raw_current":input.raw_current(),"raw_previous":input.raw_previous()}),
        )?;
        game.update(&input); // Exactly once per fixed physical input frame.
        let before_draw = observation(&game)?;
        game.draw(&mut fb);
        let s = observation(&game)?;
        check(
            game.frame_count == f as u64,
            f,
            "one Game update per physical frame",
        )?;
        let seq = audio_state(&game)?;
        game.audio
            .as_ref()
            .ok_or("real no-device owner absent")?
            .render_pcm(&mut pcm);
        check(
            audio_state(&game)? == seq,
            f,
            "1470sampleAPUpull adds no sequencer tick",
        )?;
        check(pcm.iter().all(|v| v.is_finite()), f, "finite mixed PCM")?;
        line(
            &mut states,
            &json!({"frame":f,"state":s,"draw_changed_public_state":before_draw!=s}),
        )?;
        let palette = fb
            .display_palette()
            .colors
            .iter()
            .map(|c| vec![c.r, c.g, c.b, c.a])
            .collect::<Vec<_>>();
        let packed = fb.packed();
        frames.write_all(packed).map_err(|e| e.to_string())?;
        line(
            &mut frame_meta,
            &json!({"frame":f,"width":160,"height":144,"packed_offset_bytes":packed_offset,"packed_bytes":packed.len(),"fnv1a64_packed":fnv(packed),"display_palette_RGBA":palette,"PCM_pull_samples":PCM_SAMPLES,"mixed_PCM_nonzero":pcm.iter().filter(|v|v.abs()>0.00001).count(),"PCM_raw_saved":false}),
        )?;
        packed_offset += packed.len();
        let disk = fs::read(&save).map_err(|e| e.to_string())?;
        if disk != last_disk {
            check(
                mode == "main" && f > 7128,
                f,
                "disk write only after real SAVE confirmation",
            )?;
            check(first_disk_write.is_none(), f, "exactly one real SAVE")?;
            first_disk_write = Some(f);
            let saved =
                import_sram(&disk).map_err(|e| format!("actual SAVE checksum/import: {e:?}"))?;
            assert_saved(&saved, f)?;
            line(
                &mut events,
                &json!({"frame":f,"disk_SAVE_write":true,"bytes":disk.len(),"fnv1a64":fnv(&disk),"screen":s["screen"],"save_phase":s["save_menu"]}),
            )?;
            last_disk = disk;
        }
        if game.state.screen == GameScreen::Battle && first_battle.is_none() {
            first_battle = Some(f);
        }
        if game.state.screen == GameScreen::Overworld
            && game.overworld.unified_flags().get_flag(TRAINER_FLAG)
            && first_beat.is_none()
        {
            first_beat = Some(f);
        }
        let has_departure = !s["departure_palette"].is_null();
        if has_departure {
            check(
                mode == "main" && role == "after",
                f,
                "only candidate winning caller owns new departure",
            )?;
            let start = *departure_start.get_or_insert(f);
            let relative = f - start;
            check(
                relative < 67,
                f,
                "departure must finish after its67explicit successor waits",
            )?;
            let (phase, step, remaining, palette_index) = expected_departure(relative);
            check(
                s["departure_palette"]["phase"] == phase
                    && s["departure_palette"]["step"] == step
                    && s["departure_palette"]["frames_remaining"] == remaining
                    && s["departure_palette"]["index"] == palette_index,
                f,
                "actual typed serialized phase/countdown/palette follows original waits",
            )?;
            check(
                !game.overworld.script_engine_idle()
                    && !game.overworld.unified_flags().get_flag(TRAINER_FLAG),
                f,
                "normal palette at59 still blocks through66",
            )?;
            check(
                s["native_vm_state"] == "WaitingForCommand",
                f,
                "real native VM remains suspended on departure",
            )?;
            assert_roster(&game, &s, relative >= 32, relative >= 32, f)?;
            if relative == 32 {
                let old = previous["live_toggleable_object_flags"]
                    .as_array()
                    .ok_or("previous bits missing")?;
                let old_flags = old
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect::<Vec<_>>();
                let now = game.overworld.toggleable_object_flags();
                let changed = (0..256)
                    .filter(|n| bit(now, *n) != bit(&old_flags, *n))
                    .count();
                check(changed==45,f,"same host update publishes45bit changes for46calls; Saffron15was alreadyhidden")?;
                line(
                    &mut events,
                    &json!({"frame":f,"departure_batch_relative":32,"observed_changed_toggle_bits":changed,"source_roster_calls":46,"all46_VM_and_unified_postconditions":true,"single_update_black_publication":true}),
                )?;
            }
            if (24..=42).contains(&relative) {
                check(
                    packed.iter().all(|byte| *byte == 0xff) && palette[3] == vec![0, 0, 0, 255],
                    f,
                    "all black including textbox/money region",
                )?;
                black_frames += 1;
            }
        } else if let Some(start) = departure_start {
            if departure_completed.is_none() {
                check(
                    f - start == 67,
                    f,
                    "departure return at exactly67 successor waits",
                )?;
                check(
                    game.overworld.script_engine_idle()
                        && game.overworld.unified_flags().get_flag(TRAINER_FLAG),
                    f,
                    "beat flag only after final wait",
                )?;
                assert_roster(&game, &s, true, true, f)?;
                departure_completed = Some(f);
            }
        }
        if mode == "main" {
            if f == 1460 {
                check(
                    game.state.screen == GameScreen::Overworld
                        && game.overworld.state.current_map == MapId::SilphCo11F
                        && (game.overworld.state.player.x, game.overworld.state.player.y)
                            == (6, 14),
                    f,
                    "original coldContinue anchor",
                )?;
                assert_roster(&game, &s, false, false, f)?;
            }
            if f == ORIGINAL_FRAMES {
                check(
                    game.state.screen == GameScreen::Overworld
                        && game.overworld.script_engine_idle()
                        && game.battle_count == 1,
                    f,
                    "fixed original7000 finishes the normal battle/script",
                )?;
                assert_roster(&game, &s, true, true, f)?;
                check(
                    first_disk_write.is_none() && last_disk == original,
                    f,
                    "original7000 contains no SAVE input/write",
                )?;
            }
            if f == 7032 {
                check(
                    game.state.screen == GameScreen::StartMenu
                        && game.start_menu.cursor() == 0
                        && game.start_menu.has_pokedex(),
                    f,
                    "source-fixed START starts at Pokédex row0",
                )?;
            }
            if f == 7096 {
                check(
                    game.state.screen == GameScreen::StartMenu
                        && game.start_menu.current_item() == StartMenuItem::Save,
                    f,
                    "four distinct DOWN edges select SAVE",
                )?;
            }
            if f == 7128 {
                check(
                    game.state.screen == GameScreen::SaveMenu
                        && game.save_menu.phase == SavePhase::AskSave
                        && !game.save_menu.is_different_player
                        && game.save_menu.has_previous_save,
                    f,
                    "sameplayer originalsave asks only SAVE prompt, no overwrite",
                )?;
            }
            if game.state.screen == GameScreen::SaveMenu
                && game.save_menu.sfx_event == pokered_core::save_menu::SaveSfxEvent::Save
            {
                saw_save_sfx = true;
                line(
                    &mut events,
                    &json!({"frame":f,"actual_SaveSfx92_event":true,"save_phase":s["save_menu"],"audio":s["audio"]}),
                )?;
            }
            if f == 7896 {
                check(
                    game.state.screen == GameScreen::StartMenu
                        && game.start_menu.current_item() == StartMenuItem::Save
                        && first_disk_write.is_some(),
                    f,
                    "fixed neutral budget completes actual SAVE and returns to START",
                )?;
            }
        }
        if mode == "reload" {
            assert_route_anchor(&game, &s, f)?;
            let anchors: Vec<Value> =
                serde_json::from_str(ROUTE_ANCHORS_JSON).map_err(|e| e.to_string())?;
            if anchors.iter().any(|a| a["frame"] == f) {
                check(
                    fb.display_palette().colors[..4]
                        == pokered_renderer::palette::GRAYSCALE_PALETTE.colors[..4],
                    f,
                    "settled re-entry normal display palette",
                )?;
            }
        }
        let phase = format!("{:?}", game.save_menu.phase);
        if game.state.screen == GameScreen::SaveMenu && phase != last_phase {
            line(
                &mut events,
                &json!({"frame":f,"actual_save_phase":phase,"audio":s["audio"]}),
            )?;
        }
        last_phase = phase;
        let fixed_capture = if mode == "main" {
            (4900..=5800).contains(&f)
                || [
                    1428, 1460, 1500, 7000, 7002, 7032, 7048, 7064, 7080, 7096, 7098, 7128, 7130,
                    7248, 7392, 7896, 7898, 7928,
                ]
                .contains(&f)
        } else {
            [1428, 1460, 1524, 2676, 3268, 3908, 3972].contains(&f)
        };
        if fixed_capture || f % 128 == 0 {
            let name = format!("frame-{f:04}.png");
            fb.save_png(&out.join("png").join(&name))
                .map_err(|e| e.to_string())?;
            check(
                observation(&game)? == s,
                f,
                "capture is read-only and adds no draw/update",
            )?;
            line(
                &mut captures,
                &json!({"frame":f,"path":format!("png/{name}"),"state":s}),
            )?;
            captured += 1;
        }
        let watch = [
            "screen",
            "map",
            "position",
            "flags",
            "native_vm_flags",
            "npcs",
            "live_toggleable_object_flags",
            "save_toggleable_object_flags",
            "dialogue_display",
            "battle_message",
            "departure_palette",
        ];
        let changed = watch
            .iter()
            .filter(|key| previous[**key] != s[**key])
            .copied()
            .collect::<Vec<_>>();
        if !changed.is_empty() {
            line(
                &mut events,
                &json!({"frame":f,"changed":changed,"active_script_effect":s["active_script_effect"],"battle_phase":s["battle_phase"]}),
            )?;
        }
        previous = s;
    }
    frames.flush().map_err(|e| e.to_string())?;
    check(
        game.state.screen == GameScreen::Overworld
            && game.overworld.state.current_map == MapId::SilphCo11F
            && (game.overworld.state.player.x, game.overworld.state.player.y) == (6, 13)
            && game.overworld.script_engine_idle(),
        frame_budget,
        "normal terminal field/no pending script",
    )?;
    check(
        previous["departure_palette"].is_null(),
        frame_budget,
        "no stale departure owner",
    )?;
    assert_roster(&game, &previous, true, true, frame_budget)?;
    let saved_bytes = fs::read(&save).map_err(|e| e.to_string())?;
    let saved =
        import_sram(&saved_bytes).map_err(|e| format!("final physical SRAM import: {e:?}"))?;
    assert_saved(&saved, frame_budget)?;
    if mode == "main" {
        check(
            first_battle.is_some() && first_disk_write.is_some() && saw_save_sfx,
            frame_budget,
            "actual normal battle + SAVE sequence observed",
        )?;
        if role == "after" {
            check(
                departure_start.is_some() && departure_completed.is_some() && black_frames == 19,
                frame_budget,
                "one complete new departure with19black frames",
            )?;
        } else {
            check(
                departure_start.is_none(),
                frame_budget,
                "actualM9baseline never uses candidate command",
            )?;
        }
        check(
            saved_bytes != original,
            frame_budget,
            "physical SAVE changed initial file",
        )?;
        check(
            game.overworld.unified_flags().extras().is_empty()
                || save.with_extension("script_flags.json").is_file(),
            frame_budget,
            "actual nonempty extras companion written and retained",
        )?;
    } else {
        check(
            first_battle.is_none()
                && game.battle_count == 0
                && departure_start.is_none()
                && first_disk_write.is_none(),
            frame_budget,
            "coldContinue plus normal11F/7F re-entry does not replay battle/departure or save",
        )?;
        check(
            saved_bytes == original,
            frame_budget,
            "reload never rewrites real main-run save",
        )?;
        check(
            game.overworld.state.player.facing == Direction::Up,
            frame_budget,
            "fixed route ends with normal UP movement; Continue Down was checked at1460",
        )?;
        let actual_companion = if save.with_extension("script_flags.json").exists() {
            Some(fs::read(save.with_extension("script_flags.json")).map_err(|e| e.to_string())?)
        } else {
            None
        };
        check(
            actual_companion == initial_companion,
            frame_budget,
            "reload preserves actual companion exact bytes",
        )?;
    }
    let palette = fb.display_palette();
    check(
        palette.colors[..4] == pokered_renderer::palette::GRAYSCALE_PALETTE.colors[..4],
        frame_budget,
        "normal final display palette",
    )?;
    let archived = archive_save(&save, &out.join("saved-artifacts"))?;
    let result = json!({"complete":true,"role":role,"mode":mode,"frames":frame_budget,"states":frame_budget+1,"draws":frame_budget,"captures":captured,"preboot_seed":42,"explicit_preboot_seed_calls":1,"runtime_seed_calls":0,"state_setters":0,"RPC":0,"restores":0,"skip_intro":false,"first_battle":first_battle,"first_beat":first_beat,"departure_start":departure_start,"departure_completed":departure_completed,"actual_black_frames":black_frames,"first_physical_SAVE_write":first_disk_write,"actual_SaveSfx92_event":saw_save_sfx,"saved_artifacts":archived,"final_state":previous,"framebuffer_full_packed_bytes":packed_offset,"audio_scope":"real no-device owner,1470mixedf32APUpull once per update after one draw; summary/actualsequencer only, no isolated/deviceaudibility or historicalpilotPCMclaim","source_scope":"formal actualM9/H fixed inputs; source-derived wait contract only, no completeCPU/OAM/hardware/NPC-RNG parity claim; coldreload verifies real SAVE output+actual companion then fixed11F/7F return route; no Saffron/elevator visit, not injected snapshot"});
    fs::write(
        out.join("result.json"),
        serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Silph formal witness first failure: {error}");
        std::process::exit(1);
    }
}
