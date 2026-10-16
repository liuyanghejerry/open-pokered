//! Common external witness: ordinary cold SRAM, fixed physical inputs, public Game.
//! Link the SAME bytes separately to reviewed actual-base and integrated-head App libs.
//! No debug RPC, restore, warp, injected battle/result, or runtime seed change.
//! Usage: witness SAV FIXED_BUTTONS NEW_OUTPUT before|after only-geodude|healthy-reserve
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
use pokered_core::snapshot::BattleSnapshot;
use pokered_data::species::Species;
use pokered_renderer::input::{GbButton, InputState};
use pokered_renderer::{FrameBuffer, Rgba};
use serde_json::{json, Value};

const FRAMES: usize = 3572;
const TRAINER_FLAG: &str = "EVENT_BEAT_VIRIDIAN_FOREST_TRAINER_2";
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
fn observation(game: &PokemonGame) -> Result<Value, String> {
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
fn capture(frame: usize) -> bool {
    (1973..=2400).contains(&frame) || matches!(frame, 1428 | 1460 | 1900 | 1972 | 3572)
}
fn fnv1a(bytes: &[u8]) -> String {
    let mut value = 0xcbf29ce484222325u64;
    for byte in bytes {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x100000001b3);
    }
    format!("{value:016x}")
}
fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 5 {
        return Err(
            "expected SAV BUTTONS NEW_OUTPUT before|after only-geodude|healthy-reserve".into(),
        );
    }
    let fixture = PathBuf::from(&args[0]);
    let buttons = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    let candidate = match args[3].as_str() {
        "before" => false,
        "after" => true,
        _ => return Err("invalid provider role".into()),
    };
    let reserve = match args[4].as_str() {
        "only-geodude" => false,
        "healthy-reserve" => true,
        _ => return Err("invalid fixture case".into()),
    };
    check(
        !output.exists(),
        0,
        "fresh output required; never overwrite/resume",
    )?;
    let bytes = fs::read(&fixture).map_err(|e| e.to_string())?;
    check(bytes.len() == 32768, 0, "physical SRAM must be 32768 bytes")?;
    let input_bytes = fs::read(&buttons).map_err(|e| e.to_string())?;
    let timeline: Vec<Vec<String>> =
        serde_json::from_slice(&input_bytes).map_err(|e| e.to_string())?;
    check(
        timeline.len() == FRAMES,
        0,
        "literal input must contain exactly 3572 frames",
    )?;
    // Parse and reject every malformed item before constructing a Game.
    for keys in &timeline {
        check(
            keys.len() <= 1,
            0,
            "original vector contains only zero or one held button",
        )?;
        for key in keys {
            button(key)?;
        }
    }
    fs::create_dir(&output).map_err(|e| e.to_string())?;
    let pngs = output.join("png");
    fs::create_dir(&pngs).map_err(|e| e.to_string())?;
    let save = output.join("ordinary.sav");
    fs::write(&save, &bytes).map_err(|e| e.to_string())?;
    fs::write(output.join("fixed-buttons.json"), &input_bytes).map_err(|e| e.to_string())?;
    let mut states = writer(output.join("states.jsonl"))?;
    let mut inputs = writer(output.join("inputs.jsonl"))?;
    let mut captures = writer(output.join("captures.jsonl"))?;
    let mut pcm_rows = writer(output.join("pcm.jsonl"))?;
    let mut pcm_raw = writer(output.join("pcm-f32le.raw"))?;
    let mut events = writer(output.join("events.jsonl"))?;
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
    game.set_seed(42); // single CLI-equivalent preboot seed, before first observation/update
    let mut input = InputState::new();
    let mut framebuffer = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut pcm = vec![0f32; PCM_SAMPLES];
    let mut cues: Vec<Value> = Vec::new();
    let mut serial: Option<Value> = None;
    let mut captions: Vec<String> = Vec::new();
    let mut previous_sfx: Option<u8> = None;
    let mut faint_sfx: Vec<u8> = Vec::new();
    let mut faint_pcm: Vec<u8> = Vec::new();
    let mut faint_started = false;
    let mut both_zero = false;
    let mut outcome_seen = false;
    let own_cry = pokered_data::cries::cry_data(Species::Geodude).sfx;
    let initial = observation(&game)?;
    check(game.frame_count == 0, 0, "constructor frame zero")?;
    line(
        &mut states,
        &json!({"frame":0,"state":initial,"visual_stable":game.battle_vfx.is_frame_stable()}),
    )?;
    for (index, keys) in timeline.iter().enumerate() {
        let frame = index + 1;
        input.begin_frame();
        let next = keys
            .iter()
            .map(|key| button(key))
            .collect::<Result<Vec<_>, _>>()?;
        for physical in [
            GbButton::A,
            GbButton::B,
            GbButton::Start,
            GbButton::Select,
            GbButton::Up,
            GbButton::Down,
            GbButton::Left,
            GbButton::Right,
        ] {
            if next.contains(&physical) {
                if !input.is_held(physical) {
                    input.press(physical);
                }
            } else if input.is_held(physical) {
                input.release(physical);
            }
        }
        line(
            &mut inputs,
            &json!({"frame":frame,"buttons":keys,"raw_current":input.raw_current(),"raw_previous":input.raw_previous()}),
        )?;
        game.update(&input); // exactly ONE real shared Game update
        let before_draw = observation(&game)?;
        game.draw(&mut framebuffer); // exactly ONE ordinary draw per update
        let state = observation(&game)?;
        check(
            before_draw == state,
            frame,
            "draw must preserve recorded core/script/sequence state",
        )?;
        check(
            game.frame_count == frame as u64,
            frame,
            "one update per physical frame",
        )?;
        let visual_stable = game.battle_vfx.is_frame_stable();
        let audio_before = audio_state(&game)?;
        game.audio
            .as_ref()
            .ok_or("PCM owner absent")?
            .render_pcm(&mut pcm);
        check(
            audio_state(&game)? == audio_before,
            frame,
            "APU pull must not advance recorded sequencer fields",
        )?;
        check(
            pcm.iter().all(|v| v.is_finite()),
            frame,
            "finite PCM samples",
        )?;
        let raw: Vec<u8> = pcm.iter().flat_map(|sample| sample.to_le_bytes()).collect();
        pcm_raw.write_all(&raw).map_err(|e| e.to_string())?;
        pcm_raw.flush().map_err(|e| e.to_string())?;
        let nonzero = pcm.iter().filter(|v| v.abs() > 0.00001).count();
        let rms =
            (pcm.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / PCM_SAMPLES as f64).sqrt();
        let peak = pcm.iter().map(|v| v.abs()).fold(0f32, f32::max);
        line(
            &mut pcm_rows,
            &json!({"frame":frame,"samples":PCM_SAMPLES,"raw_offset_bytes":(frame-1)*PCM_SAMPLES*4,
            "fnv1a64_f32le":fnv1a(&raw),"rms":rms,"peak":peak,"nonzero_above_1e-5":nonzero,"audio":audio_before}),
        )?;
        line(
            &mut states,
            &json!({"frame":frame,"state":state,"visual_stable":visual_stable}),
        )?;
        if capture(frame) {
            let name = format!("frame-{frame:04}.png");
            framebuffer
                .save_png(&pngs.join(&name))
                .map_err(|e| e.to_string())?;
            line(
                &mut captures,
                &json!({"frame":frame,"path":format!("png/{name}"),"width":160,"height":144,"state":state,"visual_stable":visual_stable}),
            )?;
        }
        if frame == 1428 {
            check(
                game.state.screen == GameScreen::Overworld
                    && game.overworld.state.current_map == MapId::ViridianForest,
                frame,
                "ordinary Continue reaches physical fixture map",
            )?;
            check(
                (game.overworld.state.player.x, game.overworld.state.player.y) == (2, 19)
                    && game.overworld.state.player.facing == Direction::Down,
                frame,
                "physical Continue position and source Down direction",
            )?;
        }
        if state["battle_live"]["player_hp"] == 0 && state["battle_live"]["enemy_hp"] == 0 {
            if !both_zero {
                line(
                    &mut events,
                    &json!({"frame":frame,"kind":"both_active_hp_zero","state":state}),
                )?;
            }
            both_zero = true;
        }
        if let Some(active) = state["text_publication"]["active_faint"].as_array() {
            check(active.len() == 2, frame, "active faint snapshot tuple")?;
            if serial.as_ref() != Some(&active[0]) {
                serial = Some(active[0].clone());
                cues.push(active[1].clone());
                line(
                    &mut events,
                    &json!({"frame":frame,"kind":"typed_faint","serial":active[0],"cue":active[1],"visual_stable":visual_stable}),
                )?;
            }
        }
        if let Some(message) = state["battle_message"]
            .as_str()
            .filter(|s| s.contains("fainted!"))
        {
            if captions.last().map(String::as_str) != Some(message) {
                captions.push(message.to_owned());
                line(
                    &mut events,
                    &json!({"frame":frame,"kind":"faint_caption","message":message,"state":state,"visual_stable":visual_stable}),
                )?;
                if candidate {
                    check(
                        reserve && cues.len() == 2 && visual_stable,
                        frame,
                        "enemy narration only after both typed structural actions and stable VFX",
                    )?;
                    check(
                        state["text_publication"]["faint_waiting"] == false,
                        frame,
                        "both structural actions completed before enemy caption",
                    )?;
                }
            }
        }
        let id = audio_before["id"].as_u64().ok_or("sfx ID missing")? as u8;
        let busy = audio_before["busy"] == true;
        if busy && previous_sfx != Some(id) {
            line(
                &mut events,
                &json!({"frame":frame,"kind":"sfx_start","id":id,"state":state}),
            )?;
            if id == SfxId::FaintFall as u8 {
                faint_started = true;
            }
            if faint_started && (id == SfxId::FaintFall as u8 || id == SfxId::FaintThud as u8) {
                faint_sfx.push(id);
            }
        }
        previous_sfx = busy.then_some(id);
        if candidate && faint_started && game.state.screen == GameScreen::Battle {
            check(
                !(busy && id == own_cry),
                frame,
                "silent own Remove must not start Geodude cry",
            )?;
        }
        let own_channel = audio_before["channels"]
            .as_array()
            .ok_or("channel metadata missing")?
            .iter()
            .any(|c| {
                c["index"].as_u64().is_some_and(|i| i >= 4)
                    && c["active"] == true
                    && c["sound_id"] == id
            });
        if faint_started
            && busy
            && own_channel
            && nonzero > 0
            && !faint_pcm.contains(&id)
            && (id == SfxId::FaintFall as u8 || id == SfxId::FaintThud as u8)
        {
            faint_pcm.push(id);
        }
        // Game consumes settlement synchronously in its transition/writeback.
        // Observe the actual typed victory/loss phase before that transition;
        // final wallet/flags/map/HP/PP below independently verify its writeback.
        if let pokered_core::battle::BattlePhase::TrainerVictory { player_won, .. } = &game.battle.phase {
            if *player_won == reserve {
                outcome_seen = true;
            }
        }
    }
    let final_state = observation(&game)?;
    check(
        both_zero && outcome_seen && game.battle_count == 1,
        FRAMES,
        "one real Selfdestruct double KO and typed victory/loss phase observed",
    )?;
    check(
        game.state.screen == GameScreen::Overworld,
        FRAMES,
        "ordinary fixed tail returns to field",
    )?;
    check(
        game.save_data.game_data.player_money == if reserve { 1090 } else { 500 },
        FRAMES,
        "source win prize or blackout half-money",
    )?;
    check(
        game.overworld
            .script_flags()
            .get(TRAINER_FLAG)
            .copied()
            .unwrap_or(false)
            == reserve,
        FRAMES,
        "trainer win flag follows real outcome",
    )?;
    check(
        game.save_data.game_data.player_coins == 0,
        FRAMES,
        "coins unrelated and unchanged",
    )?;
    check(
        game.save_data.party.count() == if reserve { 2 } else { 1 },
        FRAMES,
        "party size preserved",
    )?;
    let geo = game.save_data.party.get(0).ok_or("Geodude missing")?;
    check(
        geo.species == Species::Geodude && geo.hp == if reserve { 0 } else { 54 },
        FRAMES,
        "KO retained with reserve; ordinary blackout heals sole Geodude",
    )?;
    check(
        geo.pp[3] == if reserve { 4 } else { 5 },
        FRAMES,
        "one natural Selfdestruct PP; blackout restores PP",
    )?;
    if reserve {
        check(
            game.overworld.state.current_map == MapId::ViridianForest
                && (game.overworld.state.player.x, game.overworld.state.player.y) == (2, 19),
            FRAMES,
            "win returns to same trainer map",
        )?;
        let bird = game.save_data.party.get(1).ok_or("Pidgey missing")?;
        check(
            bird.species == Species::Pidgey && bird.hp == 20,
            FRAMES,
            "healthy reserve preserved",
        )?;
    } else {
        check(
            game.overworld.state.current_map == MapId::PalletTown
                && (game.overworld.state.player.x, game.overworld.state.player.y) == (5, 6),
            FRAMES,
            "ordinary blackout destination",
        )?;
    }
    if candidate {
        check(
            cues == vec![
                json!({"side":"Enemy","sound":"EnemyFall","text":null}),
                json!({"side":"Player","sound":null,"text":if reserve{Some("Enemy")}else{None}}),
            ],
            FRAMES,
            "source EnemySlide then silent OwnSlide typed cue order",
        )?;
        check(
            captions.len() == usize::from(reserve)
                && captions.iter().all(|m| m.starts_with("Enemy ")),
            FRAMES,
            "source caption eligibility",
        )?;
    } else {
        check(
            captions.len() == 2
                && captions[0].starts_with("Enemy ")
                && !captions[1].starts_with("Enemy "),
            FRAMES,
            "baseline extra faint captions remain recorded",
        )?;
    }
    check(
        faint_sfx == vec![SfxId::FaintFall as u8, SfxId::FaintThud as u8],
        FRAMES,
        "one real Fall then Thud",
    )?;
    check(
        faint_pcm.contains(&(SfxId::FaintFall as u8))
            && faint_pcm.contains(&(SfxId::FaintThud as u8)),
        FRAMES,
        "both faint sound owners coincide with nonzero actual mixed PCM",
    )?;
    check(
        fs::read(&save).map_err(|e| e.to_string())? == bytes,
        FRAMES,
        "ordinary physical save unchanged",
    )?;
    let save_named_files = fs::read_dir(&output)
        .map_err(|e| e.to_string())?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    check(
        save_named_files
            .iter()
            .filter(|name| name.starts_with("ordinary"))
            .count()
            == 1,
        FRAMES,
        "no runtime save sidecar emitted",
    )?;
    fs::write(output.join("result.json"),serde_json::to_vec_pretty(&json!({
        "complete":true,"all_pass":true,"frames":FRAMES,"states":FRAMES+1,
        "case":args[4],"role":args[3],"captures":433,"PCM_samples_per_update":PCM_SAMPLES,
        "PCM_format":"interleaved stereo f32le 44100Hz; mixed APU output; not isolated waveform/device audibility",
        "PCM_frame_hash":"FNV-1a64 over f32le bytes; outer runner adds full raw SHA256",
        "constructor":"ordinary cold SRAM/no skip/no warp/no snapshot/no device PCM",
        "preboot_seed":42,"preboot_seed_configuration_calls":1,"runtime_seed_changes":0,
        "other_state_setters":0,"debug_RPC":0,"restores":0,"extra_sequence_ticks":0,
        "draws":FRAMES,"cues":cues,"captions":captions,"faint_sfx":faint_sfx,"faint_pcm":faint_pcm,
        "final_state":final_state,"hidden_RNG_parity_claim":false,"DirectBattle_PCM_executions":0,
    })).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("ordinary faint witness failed: {error}");
        std::process::exit(1);
    }
}
