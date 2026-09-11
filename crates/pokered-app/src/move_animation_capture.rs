//! Isolated, deterministic move-animation frame capture for ROM comparison.

use std::fs::File;
use std::path::Path;

use dotzuki_engine::render_config::RenderConfig;
use pokered_audio::sfx_data::SfxId;
use pokered_core::battle::state::Pokemon;
use pokered_core::battle::{BallAnimOutcome, BattleAnimEvent, BattlePhase, BattleScreen};
use pokered_core::game_state::Lang;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::items::ItemId;
use pokered_data::move_sfx::{get_move_sound, MoveSound};
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_renderer::resource::{AssetRoot, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba};
use serde_json::json;

use crate::cli::CliItemAnimationScenario;
use crate::render::{draw_battle, BattleVisualEffects};

const WIDTH: usize = 160;
const HEIGHT: usize = 144;

fn crc32(bytes: &[u8]) -> String {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    format!("{:08x}", !crc)
}

fn adler32(bytes: &[u8]) -> String {
    const MODULUS: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in bytes {
        a = (a + u32::from(*byte)) % MODULUS;
        b = (b + a) % MODULUS;
    }
    format!("{:08x}", (b << 16) | a)
}

fn rgba(framebuffer: &FrameBuffer) -> Vec<u8> {
    let mut result = vec![0; WIDTH * HEIGHT * 4];
    assert!(framebuffer.to_rgba(&mut result));
    result
}

fn pixel_observation(pixels: &[u8], baseline: &[u8]) -> serde_json::Value {
    let mut mask = vec![0u8; (WIDTH * HEIGHT).div_ceil(8)];
    let mut changed_pixels = 0usize;
    let mut min_x = WIDTH;
    let mut min_y = HEIGHT;
    let mut max_x = 0usize;
    let mut max_y = 0usize;
    for index in 0..WIDTH * HEIGHT {
        let offset = index * 4;
        if pixels[offset..offset + 3] == baseline[offset..offset + 3] {
            continue;
        }
        mask[index / 8] |= 1 << (7 - index % 8);
        changed_pixels += 1;
        let x = index % WIDTH;
        let y = index / WIDTH;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x + 1);
        max_y = max_y.max(y + 1);
    }
    json!({
        "delta_crc32": crc32(&mask),
        "delta_adler32": adler32(&mask),
        "changed_pixels": changed_pixels,
        "bbox": (changed_pixels > 0).then_some([min_x, min_y, max_x, max_y]),
    })
}

fn save_frame(framebuffer: &FrameBuffer, path: &Path, enabled: bool) -> Result<(), String> {
    if enabled {
        framebuffer
            .save_png(path)
            .map_err(|error| format!("cannot save {}: {error}", path.display()))?;
    }
    Ok(())
}

fn rhydon() -> Result<Pokemon, String> {
    create_pokemon_with_moves(
        Species::Rhydon,
        20,
        [0xff, 0xff],
        [MoveId::Pound, MoveId::None, MoveId::None, MoveId::None],
    )
    .ok_or_else(|| "failed to construct audit Rhydon".to_string())
}

fn require_empty_output(path: &Path) -> Result<(), String> {
    if path.exists() {
        let mut entries = path
            .read_dir()
            .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
        if entries.next().is_some() {
            return Err(format!(
                "output directory must be empty to preserve evidence: {}",
                path.display()
            ));
        }
    } else {
        std::fs::create_dir_all(path)
            .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
    }
    Ok(())
}

pub fn capture_move_animation(
    move_id: u8,
    player_is_attacker: bool,
    output_dir: &Path,
    max_frames: u32,
    save_pngs: bool,
) -> Result<(), String> {
    require_empty_output(output_dir)?;
    let move_id = MoveId::from_id(move_id);
    if move_id == MoveId::None {
        return Err("move id must resolve to a canonical move".to_string());
    }
    if max_frames == 0 {
        return Err("max-frames must be greater than zero".to_string());
    }

    let player = rhydon()?;
    let enemy = rhydon()?;
    let mut screen = BattleScreen::from_parties(true, &[player], &[enemy], None);
    // Match the retail-ROM oracle's static battle scene. The DEBUG ROM enters
    // MoveAnimation from POUND on the player side and FURY ATTACK on the enemy
    // side, then the harness replaces only wAnimationID. Matching that text
    // and its level-20 HP avoids false dynamic-mask failures when a palette or
    // sprite effect touches the otherwise-static HUD/text-box pixels.
    let message = if player_is_attacker {
        "RHYDON\nused POUND!"
    } else {
        "Enemy RHYDON\nused FURY ATTACK!"
    }
    .to_string();
    screen.player_hp = 72;
    screen.player_max_hp = 72;
    screen.enemy_hp = 72;
    screen.enemy_max_hp = 72;
    screen.phase = BattlePhase::ShowingText {
        messages: vec![message.clone()],
        current: 0,
        wait_frames: 0,
        next_phase: Box::new(BattlePhase::PlayerMenu),
    };
    screen.current_message = Some(message);

    let root =
        AssetRoot::auto_detect().map_err(|error| format!("cannot locate gfx assets: {error}"))?;
    let mut resources = Some(ResourceManager::new(root));
    let mut effects = BattleVisualEffects::default();
    effects.prime_move_animation_capture_scene(&screen);

    let mut framebuffer = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_battle(
        &screen,
        &mut resources,
        &mut framebuffer,
        &mut effects,
        Lang::En,
    );
    save_frame(
        &framebuffer,
        &output_dir.join("frame-000000.png"),
        save_pngs,
    )?;
    let baseline = rgba(&framebuffer);

    let mut frames = vec![json!({
        "capture_index": 0,
        "png": save_pngs.then_some("frame-000000.png"),
        "pixel": pixel_observation(&baseline, &baseline),
        "state": effects.move_animation_capture_state(),
    })];
    effects.start_move_animation_capture(move_id, player_is_attacker);

    let mut completed_at = None;
    for capture_index in 1..=max_frames {
        effects.update(&screen);
        draw_battle(
            &screen,
            &mut resources,
            &mut framebuffer,
            &mut effects,
            Lang::En,
        );
        let filename = format!("frame-{capture_index:06}.png");
        save_frame(&framebuffer, &output_dir.join(&filename), save_pngs)?;
        let pixels = rgba(&framebuffer);
        frames.push(json!({
            "capture_index": capture_index,
            "png": save_pngs.then_some(filename),
            "pixel": pixel_observation(&pixels, &baseline),
            "state": effects.move_animation_capture_state(),
        }));
        if effects.move_animation_capture_finished() {
            completed_at = Some(capture_index);
            break;
        }
    }

    let Some(completed_at) = completed_at else {
        return Err(format!(
            "animation {move_id:?} did not finish within {max_frames} frames"
        ));
    };
    let manifest = json!({
        "schema": 2,
        "implementation": "current",
        "capture": "isolated production BattleVisualEffects + draw_battle",
        "move_id": move_id as u8,
        "move": format!("{move_id:?}"),
        "attacker": if player_is_attacker { "player" } else { "enemy" },
        "baseline_capture_index": 0,
        "first_update_capture_index": 1,
        "completed_capture_index": completed_at,
        "applying_attack_feedback": false,
        "pngs_saved": save_pngs,
        "frames": frames,
    });
    let file = File::create(output_dir.join("manifest.json"))
        .map_err(|error| format!("cannot create manifest: {error}"))?;
    serde_json::to_writer_pretty(file, &manifest)
        .map_err(|error| format!("cannot write manifest: {error}"))?;
    println!(
        "Captured {} ({}) in {} frames: {}",
        format!("{move_id:?}"),
        if player_is_attacker {
            "player"
        } else {
            "enemy"
        },
        completed_at,
        output_dir.display()
    );
    Ok(())
}

/// Capture one semantic item-use battle animation through the same production
/// `BattleVisualEffects` and renderer used by live play.
pub fn capture_item_animation(
    scenario: CliItemAnimationScenario,
    ball: ItemId,
    requested_shakes: u8,
    output_dir: &Path,
    max_frames: u32,
    save_pngs: bool,
) -> Result<(), String> {
    require_empty_output(output_dir)?;
    if max_frames == 0 {
        return Err("max-frames must be greater than zero".to_string());
    }

    let (outcome, shakes) = match scenario {
        CliItemAnimationScenario::BallCaught => (Some(BallAnimOutcome::Caught), 3),
        CliItemAnimationScenario::BallBrokeFree => {
            (Some(BallAnimOutcome::BrokeFree), requested_shakes)
        }
        CliItemAnimationScenario::BallDodged => (Some(BallAnimOutcome::Dodged), 0),
        CliItemAnimationScenario::BallBlocked => (Some(BallAnimOutcome::Blocked), 0),
        _ => (None, 0),
    };

    let player = rhydon()?;
    let enemy = rhydon()?;
    let mut screen = BattleScreen::from_parties(true, &[player], &[enemy], None);
    // Ball screenshots use semantic item text so the saved PR evidence is
    // self-explanatory. X-stat palette comparisons retain the oracle's static
    // text because the full-screen palette makes text geometry part of the
    // dynamic-pixel mask.
    let message = match scenario {
        CliItemAnimationScenario::XStatEnemy => {
            "Enemy RHYDON\nused FURY ATTACK!".to_string()
        }
        CliItemAnimationScenario::BallCaught
        | CliItemAnimationScenario::BallBrokeFree
        | CliItemAnimationScenario::BallDodged
        | CliItemAnimationScenario::BallBlocked => {
            let name = pokered_data::item_data::get_item_data(ball)
                .map(|data| data.name)
                .unwrap_or("BALL");
            format!("RED used {}!", name)
        }
        _ => "RHYDON\nused POUND!".to_string(),
    };
    screen.player_hp = 72;
    screen.player_max_hp = 72;
    screen.enemy_hp = 72;
    screen.enemy_max_hp = 72;
    screen.phase = BattlePhase::ShowingText {
        messages: vec![message.clone()],
        current: 0,
        wait_frames: 0,
        next_phase: Box::new(BattlePhase::PlayerMenu),
    };
    screen.current_message = Some(message);

    let root =
        AssetRoot::auto_detect().map_err(|error| format!("cannot locate gfx assets: {error}"))?;
    let mut resources = Some(ResourceManager::new(root));
    let mut effects = BattleVisualEffects::default();
    effects.prime_move_animation_capture_scene(&screen);

    let mut framebuffer = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_battle(
        &screen,
        &mut resources,
        &mut framebuffer,
        &mut effects,
        Lang::En,
    );
    save_frame(
        &framebuffer,
        &output_dir.join("frame-000000.png"),
        save_pngs,
    )?;
    let baseline = rgba(&framebuffer);
    let mut frames = vec![json!({
        "capture_index": 0,
        "png": save_pngs.then_some("frame-000000.png"),
        "pixel": pixel_observation(&baseline, &baseline),
        "state": effects.move_animation_capture_state(),
        "sfx": [],
    })];

    match scenario {
        CliItemAnimationScenario::XStatPlayer => {
            effects.start_non_move_animation_capture(0xAE, true);
        }
        CliItemAnimationScenario::XStatEnemy => {
            effects.start_non_move_animation_capture(0xAF, false);
        }
        CliItemAnimationScenario::SafariBait => {
            effects.start_non_move_animation_capture(0xCA, true);
        }
        CliItemAnimationScenario::SafariRock => {
            effects.start_non_move_animation_capture(0xC9, true);
        }
        _ => effects.on_anim_event(BattleAnimEvent::Ball {
            ball,
            shakes,
            outcome: outcome.expect("ball scenarios have an outcome"),
        }),
    }

    let mut completed_at = None;
    for capture_index in 1..=max_frames {
        effects.update(&screen);
        draw_battle(
            &screen,
            &mut resources,
            &mut framebuffer,
            &mut effects,
            Lang::En,
        );
        let filename = format!("frame-{capture_index:06}.png");
        save_frame(&framebuffer, &output_dir.join(&filename), save_pngs)?;
        let pixels = rgba(&framebuffer);
        let mut sfx = Vec::new();
        while let Some(id) = effects.take_ball_sfx() {
            sfx.push(json!({"kind": "ball", "id": id as u8, "name": format!("{id:?}")}));
        }
        if let Some(request) = effects.take_move_sfx() {
            match get_move_sound(
                request.anim_move,
                request.sound_move,
                request.attacker_species,
            ) {
                Some(MoveSound::Sfx(raw)) => {
                    let name = SfxId::from_u8(raw)
                        .map(|id| format!("{id:?}"))
                        .unwrap_or_else(|| format!("Unknown{raw}"));
                    sfx.push(json!({
                        "kind": "animation-command",
                        "id": raw,
                        "name": name,
                        "sound_move": request.sound_move,
                    }));
                }
                Some(MoveSound::Cry { species, .. }) => {
                    sfx.push(json!({
                        "kind": "animation-command-cry",
                        "name": format!("Cry::{species:?}"),
                        "sound_move": request.sound_move,
                    }));
                }
                None => {}
            }
        }
        frames.push(json!({
            "capture_index": capture_index,
            "png": save_pngs.then_some(filename),
            "pixel": pixel_observation(&pixels, &baseline),
            "state": effects.move_animation_capture_state(),
            "sfx": sfx,
        }));
        if effects.item_animation_capture_finished() {
            completed_at = Some(capture_index);
            break;
        }
    }

    let Some(completed_at) = completed_at else {
        return Err(format!(
            "item animation {scenario:?} did not finish within {max_frames} frames"
        ));
    };
    let manifest = json!({
        "schema": 1,
        "implementation": "current",
        "capture": "isolated production BattleVisualEffects + draw_battle",
        "scenario": format!("{scenario:?}"),
        "ball": outcome.map(|_| format!("{ball:?}")),
        "shakes": outcome.map(|_| shakes),
        "outcome": outcome.map(|value| format!("{value:?}")),
        "baseline_capture_index": 0,
        "first_update_capture_index": 1,
        "completed_capture_index": completed_at,
        "pngs_saved": save_pngs,
        "frames": frames,
    });
    let file = File::create(output_dir.join("manifest.json"))
        .map_err(|error| format!("cannot create manifest: {error}"))?;
    serde_json::to_writer_pretty(file, &manifest)
        .map_err(|error| format!("cannot write manifest: {error}"))?;
    println!(
        "Captured item animation {scenario:?} in {completed_at} frames: {}",
        output_dir.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_checksums_match_standard_vectors() {
        assert_eq!(crc32(b"123456789"), "cbf43926");
        assert_eq!(adler32(b"123456789"), "091e01de");
    }

    #[test]
    fn unchanged_frame_has_empty_delta_mask() {
        let pixels = vec![0xff; WIDTH * HEIGHT * 4];
        let observation = pixel_observation(&pixels, &pixels);
        assert_eq!(observation["changed_pixels"], 0);
        assert!(observation["bbox"].is_null());
        assert_eq!(observation["delta_crc32"], "044d19c2");
        assert_eq!(observation["delta_adler32"], "0b400001");
    }
}
