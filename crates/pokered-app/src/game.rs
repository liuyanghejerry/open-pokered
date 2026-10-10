//! Local cross-target `dbg_eprintln!` (must precede its uses below).
#[cfg(not(target_os = "none"))]
macro_rules! dbg_eprintln {
    ($($arg:tt)*) => {
        eprintln!($($arg)*)
    };
}
#[cfg(target_os = "none")]
macro_rules! dbg_eprintln {
    ($($arg:tt)*) => {{
        let _ = core::format_args!($($arg)*);
    }};
}

use crate::alloc_prelude::*;

// Link play, save files and the recorders are hosted-only (std fs/net/time).
#[cfg(not(target_os = "none"))]
use std::path::{Path, PathBuf};

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
use crate::link::LinkServer;
#[cfg(not(target_os = "none"))]
use crate::link::{CableClubFlow, CableClubPhase, FlowNeed, LinkKind, LinkSession, LinkStatus};

#[cfg(not(target_os = "none"))]
use pokered_core::battle::link_battle_driver::{LinkBattleDriver, LinkDriverEvent};
#[cfg(not(target_os = "none"))]
use pokered_core::link::link_trade::{LinkTradeDriver, LinkTradePollResult};
#[cfg(not(target_os = "none"))]
use pokered_core::link::protocol::NetworkMessage;
#[cfg(not(target_os = "none"))]
use pokered_core::link::transport::NetworkTransport;
#[cfg(not(target_os = "none"))]
use pokered_core::link::LinkRole;

use pokered_audio::music_data::MusicId;
use pokered_audio::sfx_data::SfxId;
use pokered_core::bag_screen::{BagScreenAction, BagScreenInput, BagScreenState};
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::data::impl_traits::PokemonRedData;
use pokered_core::data::maps::MapId;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::{GameScreen, GameState, SaveFileSummary, ScreenAction};
use pokered_core::gamefreak_splash::{GameFreakSplashState, SplashInput};
use pokered_core::intro_scene::IntroSceneState;
use pokered_core::intro_scene::IntroSfxEvent;
use pokered_core::items::bag_use::{self, ItemApplyOutcome};
use pokered_core::items::{MartUpdate, PlayerData, SoundId};
use pokered_core::main_menu::{MainMenuState, MenuInput};
use pokered_core::naming_screen::NamingInput;
use pokered_core::oak_speech::{OakSpeechInput, OakSpeechPhase, OakSpeechResult, OakSpeechState};
use pokered_core::options_menu::{
    BattleAnimation, GameOptions, OptionsInput, OptionsMenuResult, OptionsMenuState,
};
use pokered_core::overworld::{
    BedroomDialogue, OverworldAudioRequest, OverworldGameDataRequest, OverworldInput,
    OverworldScreen, OverworldSfxEvent,
};
use pokered_core::party_screen::{
    PartyNoticeReturn, PartyScreenAction, PartyScreenInput, PartyScreenState,
};
use pokered_core::pokedex_screen::{PokedexScreenAction, PokedexScreenInput, PokedexScreenState};
use pokered_core::save::sram_export::{export_sram, export_sram_into};
use pokered_core::stats_screen::{StatsScreenAction, StatsScreenInput, StatsScreenState};
use pokered_core::town_map_screen::{TownMapScreenAction, TownMapScreenInput, TownMapScreenState};
use pokered_core::trainer_card_screen::{
    TrainerCardAction, TrainerCardInput, TrainerCardScreenState,
};

use pokered_core::elevator_screen::{ElevatorAction, ElevatorInput, ElevatorScreen};
use pokered_core::pc_screen::{PcContext, PcEntry, PcOpenContext, PcScreen, PcScreenAction, PcSfx};
#[cfg(not(target_arch = "wasm32"))]
use pokered_core::save::sram_import::import_sram;
#[cfg(target_os = "none")]
use pokered_core::save::sram_import::import_sram_banks_into;
use pokered_core::save::SaveData;
use pokered_core::save_menu::{
    SaveMenuResult, SaveMenuState, SavePhase, SaveScreenInfo, SaveSfxEvent, YesNoInput,
};
use pokered_core::slots_screen::{SlotsAction, SlotsInput, SlotsScreen};
use pokered_core::start_menu::{StartMenuAction, StartMenuInput, StartMenuState};
use pokered_core::title_screen::{TitlePhase, TitleScreenState};
use pokered_renderer::input::{GbButton, InputState};
use pokered_renderer::resource::ResourceManager;

use pokered_renderer::resource::AssetRoot;

use dotzuki_engine::render_config::RenderConfig;
#[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
use pokered_renderer::window::GameLoop;
use pokered_renderer::{FrameBuffer, Rgba};

#[cfg(all(feature = "desktop", debug_assertions, not(target_arch = "wasm32"), not(target_os = "none")))]
use crate::hot_reload::AssetWatcher;

use crate::audio::{play_species_cry, AudioOutput};
use crate::render::{
    draw_bag, draw_battle, draw_credits, draw_diploma, draw_elevator, draw_evolution,
    draw_filter_bag, draw_gamefreak_splash, draw_hof_ceremony, draw_intro_scene, draw_main_menu,
    draw_mart, draw_oak_speech, draw_options_menu, draw_overworld, draw_party_screen, draw_pc,
    draw_pokedex_screen, draw_save_menu, draw_slots, draw_start_menu, draw_stats_screen,
    draw_title_screen, draw_town_map, draw_trade, draw_trainer_card, BattleVisualEffects,
};

#[derive(serde::Serialize, serde::Deserialize)]
struct MobileSave {
    version: u32,
    data: SaveData,
    flags: pokered_core::hash_compat::HashMap<String, bool>,
}

const SAVE_FILE_NAME: &str = "pokered.sav";
const SCRIPT_FLAGS_FILE_NAME: &str = "pokered.script_flags.json";

/// Restore the options stored in the loaded save file into the live config
/// (original: `wOptions` is part of SRAM, so a CONTINUE'd game keeps the
/// saved text speed / battle animation / battle style).
fn apply_saved_options(config: &mut pokered_core::game_state::GameConfig, options: &GameOptions) {
    use pokered_core::game_state as gs;
    use pokered_core::options_menu as om;
    config.text_speed = match options.text_speed {
        om::TextSpeed::Fast => gs::TextSpeed::Fast,
        om::TextSpeed::Medium => gs::TextSpeed::Medium,
        om::TextSpeed::Slow => gs::TextSpeed::Slow,
    };
    config.battle_animation = options.battle_animation == BattleAnimation::On;
    config.battle_style = match options.battle_style {
        om::BattleStyle::Shift => gs::BattleStyle::Shift,
        om::BattleStyle::Set => gs::BattleStyle::Set,
    };
}

fn oak_phase_tag(phase: &OakSpeechPhase) -> u8 {
    match phase {
        OakSpeechPhase::Greeting { .. } => 1,
        OakSpeechPhase::ShowNidorino { .. } => 2,
        OakSpeechPhase::Explanation { .. } => 3,
        OakSpeechPhase::IntroducePlayer { .. } => 4,
        OakSpeechPhase::PlayerNameChoice { .. } => 5,
        OakSpeechPhase::PlayerNaming => 6,
        OakSpeechPhase::IntroduceRival { .. } => 7,
        OakSpeechPhase::RivalNameChoice { .. } => 8,
        OakSpeechPhase::RivalNaming => 9,
        OakSpeechPhase::FinalSpeech { .. } => 10,
        OakSpeechPhase::ShrinkPlayer { .. } => 11,
        OakSpeechPhase::Done => 12,
        OakSpeechPhase::SlidePic { .. } => 13,
    }
}

#[cfg(target_os = "ios")]
fn save_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(".")
}

#[cfg(target_os = "android")]
fn save_dir() -> std::path::PathBuf {
    std::env::current_dir()
        .ok()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

#[cfg(all(
    not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")),
    not(target_os = "none")
))]
fn save_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
fn save_file_path() -> std::path::PathBuf {
    save_dir().join(SAVE_FILE_NAME)
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
fn companion_path_for_save(path: &Path) -> PathBuf {
    path.with_extension("script_flags.json")
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
fn companion_lookup_paths(path: Option<&Path>, legacy_native: bool) -> Vec<PathBuf> {
    let old=script_flags_file_path();
    let own=path.map(companion_path_for_save).unwrap_or_else(||old.clone());
    let mut paths=vec![own.clone()];
    // Old --save files shared the executable's sidecar. Only the positively
    // identified old layout can recover that unbound file, once on migration.
    if legacy_native && own!=old { paths.push(old); }
    paths
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
fn script_flags_file_path() -> std::path::PathBuf {
    save_dir().join(SCRIPT_FLAGS_FILE_NAME)
}

/// Key under which the save file is stored in `window.localStorage`
/// when running in a browser (wasm32 build).
#[cfg(target_arch = "wasm32")]
const WEB_SAVE_STORAGE_KEY: &str = "pokered.save";

/// Key under which the runtime-only script-flag extras (dynamic keys with
/// no bit in the fixed SRAM event-flags region, e.g. `__OBJ_HIDDEN_*`) are
/// stored on web — the wasm equivalent of the native companion sidecar
/// file `pokered.script_flags.json`.
#[cfg(target_arch = "wasm32")]
const WEB_SCRIPT_FLAGS_STORAGE_KEY: &str = "pokered.script_flags";

/// Returns a handle to `window.localStorage`, or `None` if it isn't
/// available (e.g. private mode rejecting storage access, or the API
/// being disabled).
#[cfg(target_arch = "wasm32")]
fn web_local_storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

/// Inspect the original JSON before serde defaults fill its progress tail.
/// Only the real browser save reader grants companion-alias migration;
/// debug snapshots continue to deserialize without import provenance.
#[cfg(any(target_arch = "wasm32", test))]
fn decode_web_save(raw: &str) -> Result<SaveData, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(raw)?;
    let legacy = value.get("game_data").and_then(serde_json::Value::as_object)
        .is_some_and(|data| !data.contains_key("game_progress_tail"));
    let mut save: SaveData = serde_json::from_value(value)?;
    save.imported_legacy_json = legacy;
    // Older browser JSON may contain the former OT-ID-0 derived flag.
    pokered_core::save::sram_import::derive_traded_flags(&mut save);
    Ok(save)
}

/// Attempts to load a previously persisted [`SaveData`] from the
/// browser's `localStorage`. The save is stored as a JSON serialization
/// of [`SaveData`] (whose `game_data.event_flags` carries the event-flag
/// bit array; runtime-only extras live in a separate storage key), so this
/// is the wasm equivalent of reading the `pokered.sav` file on native.
#[cfg(target_arch = "wasm32")]
fn try_load_save_from_local_storage() -> (SaveData, Option<SaveFileSummary>) {
    let storage = match web_local_storage() {
        Some(s) => s,
        None => {
            log::warn!("localStorage is unavailable; starting with an empty save");
            return (SaveData::new(), None);
        }
    };
    let raw = match storage.get_item(WEB_SAVE_STORAGE_KEY) {
        Ok(Some(s)) => s,
        Ok(None) => return (SaveData::new(), None),
        Err(e) => {
            log::warn!("failed to read save from localStorage: {:?}", e);
            return (SaveData::new(), None);
        }
    };
    match decode_web_save(&raw) {
        Ok(save) => {
            let summary = save_summary_from_data(&save);
            log::info!(
                "save loaded from localStorage (key={}, {} bytes)",
                WEB_SAVE_STORAGE_KEY,
                raw.len()
            );
            (save, Some(summary))
        }
        Err(e) => {
            log::warn!("save in localStorage is invalid JSON ({}); ignoring", e);
            (SaveData::new(), None)
        }
    }
}

fn save_summary_from_data(save: &SaveData) -> SaveFileSummary {
    SaveFileSummary {
        player_name: save.player_name.clone(),
        badges: save.game_data.obtained_badges,
        pokedex_owned: save.game_data.pokedex.owned_count() as u8,
        play_time_hours: save.game_data.play_time.hours as u16,
        play_time_minutes: save.game_data.play_time.minutes,
        play_time_seconds: save.game_data.play_time.seconds,
        player_id: save.game_data.player_id,
    }
}

/// Recorded Hall of Fame teams for the #MON LEAGUE PC viewer
/// (`LoadHallOfFameTeams` + `wHoFTeamNo`, engine/menus/league_pc.asm:16-35):
/// oldest first, numbered by their all-time index so teams recorded before
/// the 50-team SRAM window wrapped keep their true number.
fn hof_team_records(save: &SaveData) -> Vec<pokered_core::pc_screen::HofTeamRecord> {
    use pokered_core::pc_screen::{HofMonView, HofTeamRecord};
    let count = save.hall_of_fame.team_count();
    let first_no = save.game_data.num_hof_teams.saturating_sub(count as u8);
    save.hall_of_fame
        .iter()
        .enumerate()
        .map(|(i, team)| HofTeamRecord {
            team_no: first_no.wrapping_add(i as u8).wrapping_add(1),
            mons: team
                .mons()
                .iter()
                .map(|m| HofMonView {
                    species: pokered_data::species::Species::from_index_id(m.species),
                    level: m.level,
                    nickname: pokered_data::charmap::decode_string(&m.nickname),
                })
                .collect(),
        })
        .collect()
}

fn script_string_to_music_id(s: &str) -> Option<MusicId> {
    let pascal = s
        .strip_prefix("MUSIC_")
        .unwrap_or(s)
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().to_string() + &c.as_str().to_lowercase(),
            }
        })
        .collect::<String>();
    pokered_data::music::MusicId::from_name(&pascal)
        .map(|dm| MusicId::from_u8(dm as u8).unwrap_or(MusicId::PALLET_TOWN))
}

/// An in-game NPC trade whose cutscene is playing; the party mutation is
/// applied when the animation completes (`apply_npc_trade`).
struct PendingTrade {
    party_index: usize,
    ready_to_animate: bool,
    give: pokered_data::species::Species,
    receive: pokered_data::species::Species,
    /// Table-authoritative nickname (pokered_data::trades), script arg as
    /// fallback for pairs not in the TradeMons table.
    nickname: String,
}

/// Writes each rendered frame to `dir/frame-NNNNNN.png` — the capture half
/// of `run --record-frames`. The buffer is reused across frames; PNG
/// encoding is the only per-frame cost. For full-run video prefer
/// `--record-video`, which streams raw frames to ffmpeg and leaves no
/// intermediate files behind.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
pub struct FrameRecorder {
    dir: PathBuf,
    next: u64,
    fb: FrameBuffer,
    /// Compact state sampled in the same capture call as each PNG.
    manifest: std::fs::File,
    manifest_broken: bool,
}

#[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
impl FrameRecorder {
    #[cfg(not(target_os = "none"))]
    pub fn new(dir: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let manifest = std::fs::File::create(dir.join("frame-manifest.jsonl"))?;
        Ok(Self {
            dir,
            next: 0,
            fb: FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE),
            manifest,
            manifest_broken: false,
        })
    }

    fn capture(&mut self, game: &mut PokemonGame) {
        game.draw(&mut self.fb);
        let filename = format!("frame-{:06}.png", self.next);
        let path = self.dir.join(&filename);
        let png_written = match self.fb.save_png(&path) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("frame recorder: failed to write {}: {}", path.display(), e);
                false
            }
        };

        if !self.manifest_broken {
            let fly_arrival = game.overworld.enter_map_fly_anim.as_ref().map(|state| {
                let (bird_y, bird_x) = state.bird_pos();
                serde_json::json!({
                    "frame": state.frame,
                    "bird_x": bird_x,
                    "bird_y": bird_y,
                    "flap": state.flap_frame(),
                })
            });
            let fly_departure = game.overworld.fly_departure.as_ref().map(|state| {
                let bird = state.bird_pose().map(|(bird_y, bird_x, flap)| {
                    serde_json::json!({
                        "bird_x": bird_x,
                        "bird_y": bird_y,
                        "flap": flap,
                    })
                });
                serde_json::json!({
                    "frame": state.frame,
                    "force_white": state.force_white(),
                    "player_visible": state.player_visible(),
                    "bird": bird,
                })
            });
            let ledge_jump = game.overworld.ledge_jump.as_ref().map(|state| {
                serde_json::json!({
                    "frame": state.frame,
                    "origin": [state.origin_x, state.origin_y],
                    "logical_position": state.player_position(),
                    "camera_progress_px": state.camera_progress_px(),
                    "camera_residual_px": state.camera_residual_px(),
                    "walk_counter": state.walk_counter(),
                    "player_y_offset": state.player_y_offset(),
                })
            });
            let field_move_step = game.overworld.field_move_step.as_ref().map(|state| {
                serde_json::json!({
                    "frame": state.frame,
                    "origin": [state.origin_x, state.origin_y],
                    "logical_position": state.player_position(),
                    "camera_progress_px": state.camera_progress_px(),
                    "camera_residual_px": state.camera_residual_px(),
                    "walk_counter": state.walk_counter(),
                })
            });
            let field_move_restore = game.overworld.field_move_restore.as_ref().map(|state| {
                serde_json::json!({
                    "frame": state.frame,
                    "force_white": state.force_white(),
                    "sprites_visible": false,
                })
            });
            let cut_anim = game.overworld.cut_anim.as_ref().map(|state| {
                serde_json::json!({
                    "frame": state.frame,
                    "kind": format!("{:?}", state.kind),
                    "facing": format!("{:?}", state.facing),
                    "tree_spread_px": state.tree_spread_px(),
                    "palette_flipped": state.palette_flipped(),
                    "base_offset": state.base_offset(),
                })
            });
            let dialogue = game
                .overworld
                .displayed_field_dialogue()
                .and_then(|state| state.get_display_text())
                .map(|(top, bottom)| format!("{} {}", top, bottom).trim().to_string());
            let entry = serde_json::json!({
                "capture_index": self.next,
                "png": filename,
                "png_written": png_written,
                "frame_count": game.frame_count,
                "screen": crate::cli::screen_name(&game.state.screen),
                "map": format!("{:?}", game.overworld.state.current_map),
                "player_x": game.overworld.state.player.x,
                "player_y": game.overworld.state.player.y,
                "player_facing": format!("{:?}", game.overworld.state.player.facing),
                "player_transport": format!("{:?}", game.overworld.state.player.transport),
                "player_movement_state": format!("{:?}", game.overworld.state.player.movement_state),
                "walk_counter": game.overworld.state.walk_counter,
                "enter_map_fly": fly_arrival,
                "fly_departure": fly_departure,
                "fly_arrival_delay_frames": game.overworld.fly_arrival_delay_frames,
                "pending_fly_arrival": game.overworld.pending_fly_arrival,
                "ledge_jump": ledge_jump,
                "field_move_step": field_move_step,
                "field_move_restore": field_move_restore,
                "cut_anim": cut_anim,
                "cut_retained_dialogue": game.overworld.cut_retained_dialogue.is_some(),
                "warp_fade": format!("{:?}", game.overworld.warp_fade_state),
                "dialogue": dialogue,
                "battle_phase": format!("{:?}", game.battle.phase),
                "battle_message": game.battle.current_message.clone(),
            });
            use std::io::Write;
            if let Err(e) = writeln!(self.manifest, "{entry}")
                .and_then(|_| self.manifest.flush())
            {
                log::warn!("frame recorder: failed to write frame manifest: {}", e);
                self.manifest_broken = true;
            }
        }
        self.next += 1;
    }
}

/// Streams raw RGBA frames into a spawned `ffmpeg` process — the capture
/// half of `run --record-video`. Same every-update cadence as
/// `FrameRecorder`, but skips the per-frame PNG encode and the thousands of
/// intermediate files: ffmpeg reads `pipe:0` and encodes H.264 as the game
/// runs, so the .mp4 is finished when the game exits. ffmpeg's stderr is
/// inherited at `-loglevel error`, so only real errors surface.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
pub struct VideoRecorder {
    child: std::process::Child,
    /// Option solely so Drop can close the pipe before waiting on ffmpeg.
    stdin: Option<std::process::ChildStdin>,
    fb: FrameBuffer,
    /// Scratch for the display-palette RGBA expansion (`to_rgba`), reused
    /// across frames.
    rgba: Vec<u8>,
    frames: u64,
    /// Set once the pipe breaks (ffmpeg died): recording stops but the game
    /// keeps running.
    broken: bool,
}

#[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
impl VideoRecorder {
    pub fn new(path: &Path, fps: u32) -> std::io::Result<Self> {
        if fps == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "--record-video-fps must be > 0",
            ));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut child = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-loglevel",
                "error",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "-s",
                "160x144",
                "-framerate",
                &fps.to_string(),
                "-i",
                "pipe:0",
                "-an",
                "-r",
                "60",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-crf",
                "20",
                "-movflags",
                "+faststart",
            ])
            .arg(path)
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("failed to open ffmpeg stdin"))?;
        Ok(Self {
            child,
            stdin: Some(stdin),
            fb: FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE),
            rgba: vec![0; 160 * 144 * 4],
            frames: 0,
            broken: false,
        })
    }

    fn capture(&mut self, game: &mut PokemonGame) {
        if self.broken {
            return;
        }
        game.draw(&mut self.fb);
        self.fb.to_rgba(&mut self.rgba);
        use std::io::Write;
        let stdin = self.stdin.as_mut().expect("stdin open until Drop");
        if let Err(e) = stdin.write_all(&self.rgba) {
            eprintln!(
                "VideoRecorder: ffmpeg pipe broke after {} frames: {}; recording stopped",
                self.frames, e
            );
            self.broken = true;
            return;
        }
        self.frames += 1;
    }
}

#[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
impl Drop for VideoRecorder {
    fn drop(&mut self) {
        // Closing stdin signals EOF; ffmpeg then flushes the encoder and
        // rewrites the moov atom (faststart) before exiting.
        drop(self.stdin.take());
        match self.child.wait() {
            Ok(status) if status.success() => eprintln!(
                "VideoRecorder: finalized video ({} frames written)",
                self.frames
            ),
            Ok(status) => eprintln!(
                "VideoRecorder: ffmpeg exited with {} after {} frames",
                status, self.frames
            ),
            Err(e) => eprintln!("VideoRecorder: failed to wait on ffmpeg: {}", e),
        }
    }
}

/// Largest currently-allocatable block (fallible `try_reserve_exact`
/// bisection — safe to probe from production states). Diagnostic only.
#[cfg(feature = "repro-markers")]
#[inline(never)]
pub fn largest_free_block() -> usize {
    let mut lo = 0usize;
    let mut hi = 256 * 1024usize;
    while lo < hi {
        let mid = lo + (hi - lo + 1) / 2;
        let mut v: Vec<u8> = Vec::new();
        if v.try_reserve_exact(mid).is_ok() {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

pub struct PokemonGame {
    pub state: GameState,
    pub title_screen: TitleScreenState,
    /// Game Freak shooting-star splash (boot; `PlayShootingStar`,
    /// engine/movie/intro.asm:305-341 + engine/movie/splash.asm).
    pub gamefreak_splash: GameFreakSplashState,
    pub intro_scene: IntroSceneState,
    pub main_menu: MainMenuState,
    pub oak_speech: OakSpeechState,
    pub overworld: OverworldScreen,
    pub battle: BattleScreen,
    pub battle_vfx: BattleVisualEffects,
    pub start_menu: StartMenuState,
    pub options_menu: OptionsMenuState,
    pub save_menu: SaveMenuState,
    pub party_screen: PartyScreenState,
    pub bag_screen: BagScreenState,
    pub town_map_screen: TownMapScreenState,
    pub pokedex_screen: PokedexScreenState,
    pub trainer_card_screen: TrainerCardScreenState,
    /// Set when the town map should open in FLY destination-picker mode
    /// (party-menu FLY) instead of the read-only viewer.
    pending_fly_map: bool,
    /// The original keeps the selected town-map frame visible for eight raw
    /// frames while `_LeaveMapAnim` begins. During this countdown the hidden
    /// overworld FLY state still advances once per frame.
    fly_departure_screen_frames: u8,
    /// StatusScreen2 clears the display before the PC restores its saved
    /// tilemap and reloads the tileset. Input resumes on frame 6; the middle
    /// display third finishes transferring on frame 8.
    pub(crate) pc_stats_return_frame: Option<u8>,
    /// Bag item awaiting a party-member target: set when the bag's USE opens
    /// the party screen (potions, stones, TM/HM…), cleared when the item is
    /// applied or the selection is cancelled.
    pending_bag_item: Option<pokered_data::items::ItemId>,
    /// The SOFTBOILED user (party index): set when the party menu chose the
    /// field move for it; the party screen reopens in target-pick mode
    /// (Gen-1 `.softboiled` → `GoBackToPartyMenu`), cleared when the heal is
    /// applied or the pick is cancelled.
    pending_softboiled_user: Option<usize>,
    /// A move the party screen must offer to forget: a level-up/evolution
    /// move that could not be learned because the moveset is full (Gen-1
    /// `LearnMove`'s forget-a-move prompt, learn_move.asm:98-184). Holds
    /// (party index, move). Consumed by `MoveForgetChosen`; cleared on
    /// CANCEL (the move is then not learned, like AbandonLearning).
    pending_evolve_move_replace: Option<(usize, pokered_data::moves::MoveId)>,
    pub stats_screen: Option<StatsScreenState>,
    pub slots_screen: Option<SlotsScreen>,
    pub elevator_screen: Option<ElevatorScreen>,
    /// PC storage screen (Bill's PC / item PC / Oak's rating), opened by
    /// `game.openPC()` / `game.openItemPC()` via `pending_pc`.
    pub pc_screen: Option<PcScreen>,
    /// Active in-game NPC trade cutscene (engine/movie/trade.asm); while Some,
    /// it takes over update + render from the overworld.
    pub trade_anim: Option<pokered_core::trade::TradeAnim>,
    /// The trade being animated. The party mutation is applied only when the
    /// cutscene completes (original order: InternalClockTradeAnim →
    /// RemovePokemon/AddPartyMon), then the script resumes with `true`.
    pending_trade: Option<PendingTrade>,
    /// Active evolution cutscene (`pokered_core::evolution_screen`,
    /// engine/movie/evolution.asm); while Some, it takes over update + render
    /// from the overworld. Queued by the post-battle writeback (level-ups),
    /// evolution stones and Rare Candy.
    pub evolution_anim: Option<pokered_core::evolution_screen::EvolutionScreenState>,
    /// Hall of Fame roll-call takeover (engine/movie/hall_of_fame.asm),
    /// started when the HallOfFame scene calls `game.enterHallOfFame()` —
    /// the team is recorded at that moment; on completion the credits roll.
    pub hof_ceremony: Option<pokered_core::hof_ceremony::HofCeremonyState>,
    /// End-credits takeover (engine/movie/credits.asm), started when the
    /// roll call completes; on completion the game saves and resets to the
    /// title screen (scripts/HallOfFame.asm:45-56).
    pub credits: Option<pokered_core::credits::CreditsState>,
    pub save_data: SaveData,
    #[cfg(not(target_os = "none"))]
    external_saves: bool,
    #[cfg(not(target_os = "none"))]
    committed_save: Option<String>,
    #[cfg(not(target_os = "none"))]
    mobile_flags: pokered_core::hash_compat::HashMap<String, bool>,
    pub player_name: String,
    pub rival_name: String,
    pub frame_count: u64,
    pub exit_requested: bool,
    pub resources: Option<ResourceManager>,
    prev_title_phase: Option<TitlePhase>,
    prev_oak_phase_tag: u8,
    battle_prev_message: Option<String>,
    /// SFX_FAINT_FALL has played for an enemy faint in a trainer battle;
    /// SFX_FAINT_THUD follows once the fall finishes (engine/battle/core.asm:782-791).
    faint_thud_pending: bool,
    pub black_screen_frames: u32,
    pub pending_screen: Option<GameScreen>,
    #[cfg(not(target_os = "none"))]
    pub scripts_dir: Option<PathBuf>,
    pub audio: Option<AudioOutput>,
    startup_warp: Option<(MapId, u16, u16)>,
    #[cfg(all(feature = "desktop", debug_assertions, not(target_arch = "wasm32"), not(target_os = "none")))]
    pub asset_watcher: Option<AssetWatcher>,
    #[cfg(feature = "debug-server")]
    pub debug_handle: Option<pokered_debug_server::DebugServerHandle>,
    pub(crate) pending_debug_inputs: Vec<Option<GbButton>>,
    pub(crate) pending_debug_frames: u32,
    /// Active determinism seed (agent M5), set by `--seed` / `set_seed`.
    pub seed: Option<u64>,
    /// Battles started since boot — feeds the per-battle RNG seed so
    /// successive battles draw distinct (but reproducible) streams.
    pub battle_count: u64,
    /// In-memory `save_state` snapshots (JSON strings).
    pub agent_state_slots: crate::alloc_prelude::BTreeMap<u8, String>,
    /// Persistent state for debug-server injected input. A queued button must
    /// read as HELD across consecutive frames (fresh `InputState` per frame
    /// looks like repeated taps, so d-pad walking never starts).
    debug_input: InputState,
    /// Per-frame PNG recorder (`--record-frames`): captures every update —
    /// real-time loop and synchronous step_frames bursts alike — so driven
    /// runs can be assembled into video offline.
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
    pub frame_recorder: Option<FrameRecorder>,
    /// Per-frame video recorder (`--record-video`): same capture cadence as
    /// `frame_recorder`, but streams raw RGBA into a spawned ffmpeg process
    /// instead of writing one PNG per frame.
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
    pub video_recorder: Option<VideoRecorder>,
    /// Consecutive frames A+B+Start+Select have all been held — the original's
    /// soft-reset combo (engine/joypad.asm `_Joypad`/`TrySoftReset`, 16 frames
    /// of PAD_BUTTONS held → `SoftReset`).
    soft_reset_frames: u8,
    /// Whether `overworld.update_frame` ran on the previous frame. The
    /// overworld owns per-button edge detectors (`prev_*_pressed` in
    /// update.rs); any frame it doesn't run — sub-screens, naming screen,
    /// party select, link modals, cutscenes, black-screen fades — leaves them
    /// stale. The first overworld frame after such a gap re-baselines them
    /// (see `sync_overworld_input_edges`).
    ow_ran_last_frame: bool,
    /// Save file the game was started with, kept so a soft reset can reload
    /// it from disk (the original re-reads SRAM on reset).
    #[cfg(not(target_os = "none"))]
    save_path: Option<PathBuf>,
    /// Link play (Cable Club): pending server while waiting for one peer.
    /// `--link-listen` sets this (native only); `poll_link` accepts the peer
    /// into `link_session` and drops the server.
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    pub link_server: Option<LinkServer>,
    /// Active link session: owns the transport and routes wire messages
    /// into the per-activity sub-transports consumed by the core drivers
    /// below. Created at connect (`--link-connect`, an accepted peer, or the
    /// wasm BroadcastChannel entry). Routed once per frame by `poll_link`.
    #[cfg(not(target_os = "none"))]
    pub link_session: Option<LinkSession>,
    /// High-level link status for the UI (waiting / connected / "Player2
    /// disconnected" …), kept in sync by `poll_link`.
    #[cfg(not(target_os = "none"))]
    pub link_status: LinkStatus,
    /// Cable Club clock role — the host (`--link-listen`, or `?linkHost=1`
    /// on wasm) is the internal clock ("player" side), the client/guest is
    /// the external clock ("friend" side). Set by `attach_link` in main.rs
    /// or `attach_link_transport`; decides the remote player's sprite
    /// placement in the rooms, the simultaneous-gameboy tie-break and whose
    /// random list feeds the shared battle RNG.
    #[cfg(not(target_os = "none"))]
    pub link_role: pokered_core::link::LinkRole,
    /// In-room Cable Club link UI: presence, the gameboy flow, prompts and
    /// the trade selection. Fed every frame from `poll_link` events.
    #[cfg(not(target_os = "none"))]
    pub link_cable: CableClubFlow,
    /// The CANONICAL link battle driver (owns the handshake → request →
    /// party exchange → battle lifecycle, the battle screen and the shared
    /// RNG stream). Created when the connection comes up (the party is
    /// refreshed at the cable-club table); `self.battle` mirrors its screen
    /// each frame for the render/vfx/audio/settle machinery.
    #[cfg(not(target_os = "none"))]
    pub link_battle: Option<LinkBattleDriver>,
    /// The CANONICAL link trade driver (owns the party, the selection →
    /// confirm → exchange lifecycle and trade evolution). Created when the
    /// connection comes up (the party is refreshed at the cable-club table).
    #[cfg(not(target_os = "none"))]
    pub link_trade: Option<LinkTradeDriver>,
    /// POD fingerprint of the inputs that feed the script query seeds (bag,
    /// party, money, dex, daycare…). The seed rebuild — hash-map sets plus
    /// `Vec<String>` clones and `format!` loops — runs only when this
    /// changes; doing it unconditionally cost ~1,200 GBA timer ticks per
    /// overworld frame in allocator churn on an otherwise static screen.
    query_seed: QuerySeedSnapshot,
}

/// POD snapshot of every input consumed by the script/day-care query seeding
/// in the overworld frame loop. Variable-length inputs (bag, party, day-care
/// mon) are folded into FNV-1a fingerprints annotated with the scalar inputs;
/// comparing two snapshots is a fixed-size memcmp with no allocation.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
struct QuerySeedSnapshot {
    money: u32,
    coins: u16,
    dex_owned: u8,
    dex_seen: u8,
    rival_starter: u8,
    player_starter: u8,
    badges: u8,
    version: u8,
    facing: u8,
    bag_h: u64,
    party_h: u64,
    daycare_h: u64,
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv_mix(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h = (*h ^ b as u64).wrapping_mul(FNV_PRIME);
    }
}

impl QuerySeedSnapshot {
    fn hash_u8(h: &mut u64, v: u8) {
        fnv_mix(h, &[v]);
    }

    fn hash_u32(h: &mut u64, v: u32) {
        fnv_mix(h, &v.to_le_bytes());
    }
}

impl PokemonGame {
    /// Fingerprint the script/day-care query inputs. Allocation-free: bag and
    /// party are walked by reference (a `Vec`-returning `items()`/`to_vec()`
    /// here would itself allocate once per frame).
    fn query_seed_snapshot(&self) -> QuerySeedSnapshot {
        use dotzuki_engine::overworld::Direction;
        let gd = &self.save_data.game_data;
        let mut snap = QuerySeedSnapshot {
            money: gd.player_money,
            coins: gd.player_coins,
            dex_owned: gd.pokedex.owned_count() as u8,
            dex_seen: gd.pokedex.seen_count() as u8,
            rival_starter: gd.rival_starter,
            player_starter: gd.player_starter,
            badges: gd.obtained_badges,
            version: match self.state.config.version {
                GameVersion::Red => 0,
                GameVersion::Blue => 1,
            },
            facing: match self.overworld.state.player.facing {
                Direction::Up => 0,
                Direction::Down => 1,
                Direction::Left => 2,
                Direction::Right => 3,
            },
            bag_h: 0,
            party_h: 0,
            daycare_h: 0,
        };

        let mut h = FNV_OFFSET;
        for i in 0..gd.bag.count() {
            if let Some((id, qty)) = gd.bag.get(i) {
                QuerySeedSnapshot::hash_u8(&mut h, id as u8);
                QuerySeedSnapshot::hash_u8(&mut h, qty);
            }
        }
        snap.bag_h = h;

        let mut h = FNV_OFFSET;
        fnv_mix(&mut h, &gd.player_id.to_le_bytes());
        fnv_mix(&mut h, &self.save_data.player_name);
        let mut name_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
        for mon in self.save_data.party.iter() {
            fnv_mix(&mut h, &mon.ot_id.to_le_bytes());
            fnv_mix(&mut h, &mon.ot_name);
            QuerySeedSnapshot::hash_u8(&mut h, mon.species as u8);
            QuerySeedSnapshot::hash_u8(&mut h, mon.level);
            for mv in &mon.moves {
                QuerySeedSnapshot::hash_u8(&mut h, *mv as u8);
            }
            fnv_mix(&mut h, mon.display_name(&mut name_buf).as_bytes());
        }
        snap.party_h = h;

        let dc = &gd.daycare;
        let mut h = FNV_OFFSET;
        QuerySeedSnapshot::hash_u8(&mut h, dc.in_use as u8);
        QuerySeedSnapshot::hash_u8(&mut h, dc.species);
        QuerySeedSnapshot::hash_u32(&mut h, dc.exp);
        QuerySeedSnapshot::hash_u8(&mut h, dc.box_level);
        fnv_mix(&mut h, &gd.daycare_mon_name);
        snap.daycare_h = h;

        snap
    }
}

/// Normalize the trade driver's errors onto the transport error type so the
/// flow-need handler treats both drivers uniformly.
#[cfg(not(target_os = "none"))]
fn link_trade_err_to_transport(
    e: pokered_core::link::link_trade::LinkTradeError,
) -> pokered_core::link::transport::TransportError {
    match e {
        pokered_core::link::link_trade::LinkTradeError::Transport(t) => t,
        other => pokered_core::link::transport::TransportError::IoError(other.to_string()),
    }
}

/// Seed for the host's 10-byte random list: wall-clock time natively;
/// `Math.random()` on wasm, where `std::time::SystemTime` is unavailable at
/// runtime (it compiles but panics). The values only need to be
/// host-known — both sides consume the host's list — not unpredictable.
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
fn link_random_seed() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() ^ (d.as_secs() as u32))
        .unwrap_or(0x9E3779B9)
}

#[cfg(target_arch = "wasm32")]
fn link_random_seed() -> u32 {
    (js_sys::Math::random() * u32::MAX as f64) as u32
}

pub fn parse_warp_arg(s: &str) -> Result<(MapId, Option<u16>, Option<u16>), String> {
    use pokered_core::data::maps::NUM_MAPS;

    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
    if parts.is_empty() {
        return Err("warp argument is empty".to_string());
    }

    let map_name = parts[0];
    let mut map_id: Option<MapId> = None;
    for i in 0..NUM_MAPS {
        if let Some(m) = MapId::from_u8(i as u8) {
            if format!("{:?}", m) == map_name {
                map_id = Some(m);
                break;
            }
        }
    }
    let map_id = map_id.ok_or_else(|| format!("unknown map name: '{}'", map_name))?;

    let x = if parts.len() >= 2 {
        Some(
            parts[1]
                .parse::<u16>()
                .map_err(|e| format!("invalid x coordinate: {}", e))?,
        )
    } else {
        None
    };

    let y = if parts.len() >= 3 {
        Some(
            parts[2]
                .parse::<u16>()
                .map_err(|e| format!("invalid y coordinate: {}", e))?,
        )
    } else {
        None
    };

    Ok((map_id, x, y))
}

/// Screens that must NOT re-enter the MainMenu Continue/NewGame re-entry
/// arms: everything that lives INSIDE a play session (the overworld plus every
/// overlay opened from it). Re-entering would rebuild the overworld from the
/// save and teleport the player (the "shop exit warps you home" bug — carried
/// variants like `Shop(_)`/`PokemonStatsScreen(_)` can't be compared with `!=`,
/// which is how they were missed).
fn is_ingame_session_screen(s: &GameScreen) -> bool {
    matches!(
        s,
        GameScreen::Overworld
            | GameScreen::Battle
            | GameScreen::Shop(_)
            | GameScreen::StartMenu
            | GameScreen::OptionsMenu
            | GameScreen::SaveMenu
            | GameScreen::PartyScreen
            | GameScreen::PokemonStatsScreen(_)
            | GameScreen::Bag
            | GameScreen::TownMap
            | GameScreen::Slots
            | GameScreen::Elevator
            | GameScreen::FilterBag
            | GameScreen::Diploma
            | GameScreen::PC
            | GameScreen::Pokedex
            | GameScreen::TrainerCard
    )
}

impl PokemonGame {
    /// Attach a CONNECTED link transport and start the Cable Club link
    /// session: sets the clock role and creates the session; the CORE
    /// battle/trade drivers are created on the first `poll_link` frame, and
    /// the `LinkRole::Guest` side starts the asymmetric Hello/HelloAck
    /// handshake then (the `LinkRole::Host` side auto-acks from its `Idle`
    /// state).
    ///
    /// This is the transport-agnostic seam the native binary's
    /// `--link-connect` path (main.rs `attach_link`) and the wasm
    /// BroadcastChannel entry (pokered-web, `?link=<channel>`) both call.
    #[cfg(not(target_os = "none"))]
    pub fn attach_link_transport(
        &mut self,
        transport: Box<dyn NetworkTransport<NetworkMessage>>,
        role: LinkRole,
    ) {
        self.link_role = role;
        self.link_session = Some(LinkSession::new(
            transport,
            crate::link::link_activity,
            NetworkMessage::Disconnect,
        ));
        self.link_status = LinkStatus::Connecting;
    }

    /// Drop the link session and drivers (the wasm host page's
    /// `linkLeave()`), returning the Cable Club UI to idle. The transport is
    /// dropped with the session — closing the channel or socket — so the
    /// peer sees a disconnect. Call between activities; detaching mid-battle
    /// leaves the (mirrored) link battle screen frozen.
    #[cfg(not(target_os = "none"))]
    pub fn detach_link(&mut self) {
        self.link_session = None;
        self.link_battle = None;
        self.link_trade = None;
        self.link_status = LinkStatus::Disabled;
        self.link_cable = CableClubFlow::new();
    }

    /// Creates a new game with default settings (no save file, no scripts dir).
    /// This is the primary constructor used by both web and native builds.
    #[cfg(all(
        not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")),
        not(target_os = "none")
    ))]
    pub fn new(version: GameVersion) -> Self {
        #[cfg(feature = "debug-server")]
        return Self::new_with_options(version, None, None, None, false, None, false, false, None);
        #[cfg(not(feature = "debug-server"))]
        return Self::new_with_options(version, None, None, None, false, None, false, false);
    }

    /// Creates a new game with optional save file, snapshot, and scripts directory.
    /// Only available for native builds (wasm doesn't support file system operations).
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    #[allow(unused_variables)]
    pub fn new_with_options(
        version: GameVersion,
        save_path: Option<PathBuf>,
        snapshot_path: Option<PathBuf>,
        scripts_dir: Option<PathBuf>,
        skip_intro: bool,
        warp: Option<String>,
        watch: bool,
        no_audio: bool,
        #[cfg(feature = "debug-server")] debug_handle: Option<
            pokered_debug_server::DebugServerHandle,
        >,
    ) -> Self {
        let (save_data, save_summary) = if let Some(ref path) = snapshot_path {
            Self::load_snapshot_from_path(path)
        } else if let Some(ref path) = save_path {
            Self::load_sram_from_path(path)
        } else {
            Self::try_load_default_save()
        };

        // Parse warp argument if provided
        let startup_warp = if let Some(ref warp_str) = warp {
            match parse_warp_arg(warp_str) {
                Ok((map_id, x, y)) => {
                    let x = x.unwrap_or(10);
                    let y = y.unwrap_or(8);
                    Some((map_id, x, y))
                }
                Err(e) => {
                    dbg_eprintln!(
                        "Warning: invalid --warp argument '{}': {}. Ignoring warp.",
                        warp_str,
                        e
                    );
                    None
                }
            }
        } else {
            None
        };

        // Determine initial screen and overworld configuration
        let initial_screen;
        let mut overworld;
        let mut player_name = "RED".to_string();
        let mut rival_name = "BLUE".to_string();

        if skip_intro {
            initial_screen = GameScreen::Overworld;
            let (map_id, requested) = if let Some((m, x, y)) = startup_warp {
                (m, Some((x.min(255) as u8, y.min(255) as u8)))
            } else {
                // No warp: boot at the save's position. An empty save's
                // default position is tile (0, 0) — the unreachable top-left
                // corner of every map — so treat it as "no position" and let
                // the resolver pick a walkable spot.
                let pos = save_data.game_data.position;
                let map_id = MapId::from_u8(pos.map_id)
                    .filter(|m| m.dimensions().0 > 0)
                    .unwrap_or(MapId::PalletTown);
                let requested = if pos.x == 0 && pos.y == 0 {
                    None
                } else {
                    Some((pos.x, pos.y))
                };
                (map_id, requested)
            };

            overworld = OverworldScreen::new(map_id, scripts_dir.clone(), PokemonRedData);
            if startup_warp.is_none() {
                overworld.restore_saved_last_map(save_data.game_data.last_map);
            }
            // Resolve a walkable landing spot: a valid requested position is
            // honored unchanged, a blocked/out-of-bounds one snaps to the
            // nearest walkable tile, and no request picks the map's warp
            // spots / center — the player never boots stuck (both the empty
            // save's (0, 0) and Pallet Town's nominal (10, 8) are wall tiles).
            let (px, py) = overworld.resolve_editor_warp_position(map_id, requested);
            overworld.state.player.x = px as u16;
            overworld.state.player.y = py as u16;
            overworld.state.player.facing = pokered_core::overworld::Direction::Down;

            // Load save-related data into overworld
            overworld.party_count = save_data.party.count() as u8;
            overworld.box_count = save_data.current_box.count() as u8;
            overworld.party_lead_level = save_data.party.leader_level();
            player_name = pokered_data::charmap::decode_string(&save_data.player_name);
            rival_name = pokered_data::charmap::decode_string(&save_data.game_data.rival_name);
            overworld.player_name = player_name.clone();
            overworld.rival_name = rival_name.clone();
            // Seed the event-flag bitset from SRAM bytes, then merge any
            // runtime-only extras (companion sidecar) on top.
            overworld.restore_loaded_save_flags(&save_data, Self::read_companion_script_flags(save_path.as_deref(), save_data.imported_legacy_native));
            overworld.set_toggleable_object_flags(save_data.game_data.toggleable_object_flags);
            overworld.set_hidden_item_flags(save_data.game_data.obtained_hidden_items);
            overworld.set_hidden_coin_flags(save_data.game_data.obtained_hidden_coins);
            overworld.apply_hidden_object_flags();
            overworld.run_on_load();

            dbg_eprintln!(
                "Skip-intro: starting at {:?} ({} x={}, y={})",
                map_id,
                map_id as u8,
                px,
                py
            );
        } else {
            initial_screen = GameScreen::GameFreakSplash;
            overworld =
                OverworldScreen::new(MapId::PalletTown, scripts_dir.clone(), PokemonRedData);
        }

        let mut state = GameState {
            screen: initial_screen,
            config: pokered_core::game_state::GameConfig::new(version),
            save_summary: save_summary.clone(),
        };
        apply_saved_options(&mut state.config, &save_data.game_data.options);
        let title_screen = TitleScreenState::new(version);
        let main_menu = MainMenuState::new(save_summary);
        let oak_speech = OakSpeechState::new();
        let battle = BattleScreen::new(true);
        let battle_vfx = BattleVisualEffects::default();
        let start_menu = StartMenuState::new(false, false, false);
        let options_menu = OptionsMenuState::new(GameOptions::default());
        let save_menu = SaveMenuState::new(
            SaveScreenInfo {
                player_name: "RED".to_string(),
                num_badges: 0,
                pokedex_owned: 0,
                play_time_hours: 0,
                play_time_minutes: 0,
            },
            false,
            false,
        );

        let resources = match AssetRoot::auto_detect() {
            Ok(root) => {
                dbg_eprintln!("Asset root found: {:?}", root.gfx_dir());
                Some(ResourceManager::new(root))
            }
            Err(e) => {
                dbg_eprintln!("Warning: Could not find gfx/ directory: {}", e);
                dbg_eprintln!("Falling back to text-only placeholder rendering.");
                None
            }
        };

        let audio = if no_audio {
            dbg_eprintln!("Audio output disabled (--no-audio).");
            Some(AudioOutput::new_pcm())
        } else {
            match AudioOutput::new() {
                Some(ao) => {
                    dbg_eprintln!("Audio output initialized (cpal 44100 Hz stereo)");
                    Some(ao)
                }
                None => {
                    dbg_eprintln!("Warning: Could not initialize audio output.");
                    Some(AudioOutput::new_pcm())
                }
            }
        };

        #[cfg(all(feature = "desktop", debug_assertions, not(target_arch = "wasm32")))]
        let asset_watcher = if watch {
            let mut dirs = Vec::new();

            // Watch the gfx/ parent directory for .png changes
            if let Ok(root) = AssetRoot::auto_detect() {
                if let Some(parent) = root.gfx_dir().parent() {
                    dirs.push(parent.to_path_buf());
                }
            }

            // Watch assets/ for .tmx files
            if let Ok(cwd) = std::env::current_dir() {
                let assets_dir = cwd.join("assets");
                if assets_dir.is_dir() {
                    dirs.push(assets_dir);
                }
            }

            // Watch scripts directory for .js files
            if let Some(ref sd) = scripts_dir {
                if sd.is_dir() {
                    dirs.push(sd.clone());
                }
            }

            match AssetWatcher::new(&dirs) {
                Ok(w) => {
                    dbg_eprintln!("[hot-reload] Asset watcher active");
                    Some(w)
                }
                Err(e) => {
                    dbg_eprintln!("[hot-reload] Failed to start watcher: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // The script engine language starts from the saved/default config;
        // the LanguageSelect screen re-syncs it whenever the choice changes.
        overworld.set_script_lang(
            if state.config.language == pokered_core::game_state::Lang::Zh {
                "zh"
            } else {
                "en"
            },
        );

        Self {
            state,
            title_screen,
            intro_scene: IntroSceneState::new(),
            gamefreak_splash: GameFreakSplashState::new(),
            main_menu,
            oak_speech,
            overworld,
            battle,
            battle_vfx,
            start_menu,
            options_menu,
            save_menu,
            party_screen: PartyScreenState::new(vec![]),
            bag_screen: BagScreenState::new(vec![]),
            town_map_screen: TownMapScreenState::new(MapId::PalletTown),
            pokedex_screen: PokedexScreenState::new(
                pokered_core::pokemon::pokedex::Pokedex::new(),
                version,
            ),
            trainer_card_screen: TrainerCardScreenState::new(),
            pending_evolve_move_replace: None,
            pending_fly_map: false,
            fly_departure_screen_frames: 0,
            pc_stats_return_frame: None,
            pending_bag_item: None,
            pending_softboiled_user: None,
            stats_screen: None,
            slots_screen: None,
            elevator_screen: None,
            pc_screen: None,
            trade_anim: None,
            evolution_anim: None,
            hof_ceremony: None,
            credits: None,
            pending_trade: None,
            save_data,
            external_saves: false,
            committed_save: None,
            mobile_flags: Default::default(),
            player_name,
            rival_name,
            frame_count: 0,
            exit_requested: false,
            resources,
            prev_title_phase: None,
            prev_oak_phase_tag: 0,
            battle_prev_message: None,
            faint_thud_pending: false,
            black_screen_frames: 0,
            pending_screen: None,
            #[cfg(not(target_os = "none"))]
            scripts_dir,
            audio,
            startup_warp,
            #[cfg(feature = "debug-server")]
            debug_handle,
            #[cfg(all(feature = "desktop", debug_assertions, not(target_arch = "wasm32")))]
            asset_watcher,
            pending_debug_inputs: Vec::new(),
            pending_debug_frames: 0,
            seed: None,
            battle_count: 0,
            agent_state_slots: crate::alloc_prelude::BTreeMap::new(),
            debug_input: InputState::new(),
            #[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
            frame_recorder: None,
            #[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
            video_recorder: None,
            soft_reset_frames: 0,
            ow_ran_last_frame: false,
            #[cfg(not(target_os = "none"))]
            save_path,
            #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
            link_server: None,
            #[cfg(not(target_os = "none"))]
            link_session: None,
            #[cfg(not(target_os = "none"))]
            link_status: LinkStatus::Disabled,
            #[cfg(not(target_os = "none"))]
            link_role: pokered_core::link::LinkRole::Host,
            #[cfg(not(target_os = "none"))]
            link_cable: CableClubFlow::new(),
            #[cfg(not(target_os = "none"))]
            link_battle: None,
            #[cfg(not(target_os = "none"))]
            link_trade: None,
            query_seed: QuerySeedSnapshot::default(),
        }
    }

    /// Bare-metal (GBA) constructor: a fresh new game — no save file, no
    /// snapshot, no `--scripts-dir`, no warp, no watcher and no debug server.
    /// Audio uses the hardware PSG. Scenes and map data come
    /// from the build-time embedded tables; graphics come from the
    /// pre-converted 2bpp registry. Called by `pokered-gba`'s `main.rs`.
    #[cfg(target_os = "none")]
    // Never inlined: the constructor's ~29 KB return slot plus its own temps
    // must live in a shallow call frame, not in `game_main`'s prologue
    // reservation (fat LTO merged them, ballooning the boot frame to ~55 KB
    // of the 64 KiB EWRAM stack).
    #[inline(never)]
    pub fn new_for_gba(version: GameVersion) -> Self {
        // NOTE: bare-metal callers keep `PokemonGame` in EWRAM (~29 KB, most
        // of the IWRAM stack budget). Large fields are constructed inline in
        // the struct literal below so no big temporary lives on the stack.
        log::info!("gba:ctor overworld");
        let title_screen = TitleScreenState::new(version);
        let main_menu = MainMenuState::new(None);
        let oak_speech = OakSpeechState::new();
        // The bare-metal build has no saved-game path, so prebuild the exact
        // first overworld map while the entry-point stack is shallow. Building
        // a second complete OverworldScreen during the Oak transition would
        // temporarily retain both script registries and exhaust EWRAM.
        let mut overworld = OverworldScreen::new(
            pokered_core::data::fly_warp_data::NEW_GAME_WARP.map_id,
            PokemonRedData,
        );
        overworld.set_script_lang(
            if pokered_core::game_state::GameConfig::new(version).language
                == pokered_core::game_state::Lang::Zh
            {
                "zh"
            } else {
                "en"
            },
        );
        let battle = BattleScreen::new(true);
        let state = GameState {
            screen: GameScreen::GameFreakSplash,
            config: {
                let mut c = pokered_core::game_state::GameConfig::new(version);
                apply_saved_options(&mut c, &GameOptions::default());
                c
            },
            save_summary: None,
        };
        let battle_vfx = BattleVisualEffects::default();
        let start_menu = StartMenuState::new(false, false, false);
        let options_menu = OptionsMenuState::new(GameOptions::default());
        let save_menu = SaveMenuState::new(
            SaveScreenInfo {
                player_name: "RED".to_string(),
                num_badges: 0,
                pokedex_owned: 0,
                play_time_hours: 0,
                play_time_minutes: 0,
            },
            false,
            false,
        );

        // Graphics from the build-time pre-converted registry.
        log::info!("gba:ctor resources");
        let resources = Some(ResourceManager::new(AssetRoot::new()));
        // Shared sequencer with hardware PSG output.
        let audio = AudioOutput::new();

        log::info!("gba:ctor pre-intro");
        let f_intro = IntroSceneState::new();
        log::info!("gba:ctor pre-splash");
        let f_splash = GameFreakSplashState::new();
        log::info!("gba:ctor pre-party");
        let f_party = PartyScreenState::new(vec![]);
        log::info!("gba:ctor pre-dex");
        let f_dex =
            PokedexScreenState::new(pokered_core::pokemon::pokedex::Pokedex::new(), version);
        log::info!("gba:ctor pre-towns");
        let f_town = TownMapScreenState::new(MapId::PalletTown);
        log::info!("gba:ctor pre-bag");
        let f_bag = BagScreenState::new(vec![]);
        log::info!("gba:ctor pre-card");
        let f_card = TrainerCardScreenState::new();
        log::info!("gba:ctor pre-save");
        let f_save = SaveData::new();
        log::info!("gba:ctor fields built");
        let built = Self {
            state,
            title_screen,
            intro_scene: f_intro,
            gamefreak_splash: f_splash,
            main_menu,
            oak_speech,
            overworld,
            battle,
            battle_vfx,
            start_menu,
            options_menu,
            save_menu,
            party_screen: f_party,
            bag_screen: f_bag,
            town_map_screen: f_town,
            pokedex_screen: f_dex,
            trainer_card_screen: f_card,
            pending_evolve_move_replace: None,
            pending_fly_map: false,
            fly_departure_screen_frames: 0,
            pc_stats_return_frame: None,
            pending_bag_item: None,
            pending_softboiled_user: None,
            stats_screen: None,
            slots_screen: None,
            elevator_screen: None,
            pc_screen: None,
            trade_anim: None,
            evolution_anim: None,
            hof_ceremony: None,
            credits: None,
            pending_trade: None,
            save_data: f_save,
            player_name: "RED".to_string(),
            rival_name: "BLUE".to_string(),
            frame_count: 0,
            exit_requested: false,
            resources,
            prev_title_phase: None,
            prev_oak_phase_tag: 0,
            battle_prev_message: None,
            faint_thud_pending: false,
            black_screen_frames: 0,
            pending_screen: None,
            audio,
            pending_debug_inputs: Vec::new(),
            pending_debug_frames: 0,
            seed: None,
            battle_count: 0,
            agent_state_slots: crate::alloc_prelude::BTreeMap::new(),
            debug_input: InputState::new(),
            startup_warp: None,
            soft_reset_frames: 0,
            ow_ran_last_frame: false,
            query_seed: QuerySeedSnapshot::default(),
        };
        log::info!("gba:ctor literal built");
        built
    }

    #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
    pub fn new(version: GameVersion) -> Self {
        #[cfg(target_arch = "wasm32")]
        let (save_data, save_summary) = try_load_save_from_local_storage();
        #[cfg(target_os = "android")]
        let (save_data, save_summary) = Self::try_load_default_save();
        #[cfg(target_os = "ios")]
        let (save_data, save_summary) = Self::try_load_default_save();
        Self::new_portable(version, save_data, save_summary, AudioOutput::new())
    }

    #[cfg(not(target_os = "none"))]
    fn new_portable(
        version: GameVersion,
        save_data: SaveData,
        save_summary: Option<SaveFileSummary>,
        audio: Option<AudioOutput>,
    ) -> Self {
        let audio = Some(audio.unwrap_or_else(AudioOutput::new_pcm));
        let mut state = GameState {
            screen: GameScreen::GameFreakSplash,
            config: pokered_core::game_state::GameConfig::new(version),
            save_summary: save_summary.clone(),
        };
        apply_saved_options(&mut state.config, &save_data.game_data.options);
        let title_screen = TitleScreenState::new(version);
        let main_menu = MainMenuState::new(save_summary);
        let oak_speech = OakSpeechState::new();
        let mut overworld = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
        overworld.set_script_lang(
            if state.config.language == pokered_core::game_state::Lang::Zh {
                "zh"
            } else {
                "en"
            },
        );
        let battle = BattleScreen::new(true);
        let battle_vfx = BattleVisualEffects::default();
        let start_menu = StartMenuState::new(false, false, false);
        let options_menu = OptionsMenuState::new(GameOptions::default());
        let save_menu = SaveMenuState::new(
            SaveScreenInfo {
                player_name: "RED".to_string(),
                num_badges: 0,
                pokedex_owned: 0,
                play_time_hours: 0,
                play_time_minutes: 0,
            },
            false,
            false,
        );

        let resources = Some(ResourceManager::new(AssetRoot::new_wasm()));

        Self {
            #[cfg(all(feature = "desktop", debug_assertions, not(target_arch = "wasm32")))]
            asset_watcher: None,
            #[cfg(feature = "debug-server")]
            debug_handle: None,
            state,
            title_screen,
            intro_scene: IntroSceneState::new(),
            gamefreak_splash: GameFreakSplashState::new(),
            main_menu,
            oak_speech,
            overworld,
            battle,
            battle_vfx,
            start_menu,
            options_menu,
            save_menu,
            party_screen: PartyScreenState::new(vec![]),
            bag_screen: BagScreenState::new(vec![]),
            town_map_screen: TownMapScreenState::new(MapId::PalletTown),
            pokedex_screen: PokedexScreenState::new(
                pokered_core::pokemon::pokedex::Pokedex::new(),
                version,
            ),
            trainer_card_screen: TrainerCardScreenState::new(),
            pending_evolve_move_replace: None,
            pending_fly_map: false,
            fly_departure_screen_frames: 0,
            pc_stats_return_frame: None,
            pending_bag_item: None,
            pending_softboiled_user: None,
            stats_screen: None,
            slots_screen: None,
            elevator_screen: None,
            pc_screen: None,
            trade_anim: None,
            evolution_anim: None,
            hof_ceremony: None,
            credits: None,
            pending_trade: None,
            save_data,
            external_saves: false,
            committed_save: None,
            mobile_flags: Default::default(),
            player_name: "RED".to_string(),
            rival_name: "BLUE".to_string(),
            frame_count: 0,
            exit_requested: false,
            resources,
            prev_title_phase: None,
            prev_oak_phase_tag: 0,
            battle_prev_message: None,
            faint_thud_pending: false,
            black_screen_frames: 0,
            pending_screen: None,
            #[cfg(not(target_os = "none"))]
            scripts_dir: None,
            audio,
            pending_debug_inputs: Vec::new(),
            pending_debug_frames: 0,
            seed: None,
            battle_count: 0,
            agent_state_slots: crate::alloc_prelude::BTreeMap::new(),
            debug_input: InputState::new(),
            #[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
            frame_recorder: None,
            #[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
            video_recorder: None,
            startup_warp: None,
            soft_reset_frames: 0,
            ow_ran_last_frame: false,
            #[cfg(not(target_os = "none"))]
            save_path: None,
            #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
            link_server: None,
            #[cfg(not(target_os = "none"))]
            link_session: None,
            #[cfg(not(target_os = "none"))]
            link_status: LinkStatus::Disabled,
            #[cfg(not(target_os = "none"))]
            link_role: pokered_core::link::LinkRole::Host,
            #[cfg(not(target_os = "none"))]
            link_cable: CableClubFlow::new(),
            #[cfg(not(target_os = "none"))]
            link_battle: None,
            #[cfg(not(target_os = "none"))]
            link_trade: None,
            query_seed: QuerySeedSnapshot::default(),
        }
    }

    /// Boot an embedded game without opening a device or reading desktop saves.
    #[cfg(not(target_os = "none"))]
    pub fn new_mobile(version: GameVersion, save: Option<&str>) -> Result<Self, String> {
        let parsed = save
            .map(serde_json::from_str::<MobileSave>)
            .transpose()
            .map_err(|e| e.to_string())?;
        if parsed.as_ref().is_some_and(|s| s.version != 1) {
            return Err("unsupported mobile save version".into());
        }
        let data = parsed
            .as_ref()
            .map(|s| s.data.clone())
            .unwrap_or_else(SaveData::new);
        let summary = parsed.as_ref().map(|s| save_summary_from_data(&s.data));
        let mut game = Self::new_portable(version, data, summary, Some(AudioOutput::new_pcm()));
        game.external_saves = true;
        game.committed_save = save.map(str::to_owned);
        game.mobile_flags = parsed.map(|s| s.flags).unwrap_or_default();
        Ok(game)
    }
    #[cfg(not(target_os = "none"))]
    pub fn export_mobile_save(&self) -> Option<String> {
        self.committed_save.clone()
    }
    #[cfg(not(target_os = "none"))]
    pub fn import_mobile_save(&mut self, save: &str) -> Result<(), String> {
        let replacement = Self::new_mobile(self.state.config.version, Some(save))?;
        *self = replacement;
        Ok(())
    }
    #[cfg(not(target_os = "none"))]
    fn companion_flags(&self) -> Option<pokered_core::hash_compat::HashMap<String, bool>> {
        if self.external_saves {
            Some(self.mobile_flags.clone())
        } else {
            Self::read_companion_script_flags(self.save_path.as_deref(), self.save_data.imported_legacy_native)
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    fn try_load_default_save() -> (SaveData, Option<SaveFileSummary>) {
        let path = save_file_path();
        let (save, summary) = match std::fs::read(&path) {
            Ok(data) => Self::parse_sram(&path, &data),
            Err(_) => (SaveData::new(), None),
        };
        (save, summary)
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    fn load_sram_from_path(path: &Path) -> (SaveData, Option<SaveFileSummary>) {
        let (save, summary) = match std::fs::read(path) {
            Ok(data) => Self::parse_sram(path, &data),
            Err(e) => {
                dbg_eprintln!("Error: failed to read save file {:?}: {}", path, e);
                (SaveData::new(), None)
            }
        };
        (save, summary)
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    fn load_snapshot_from_path(path: &Path) -> (SaveData, Option<SaveFileSummary>) {
        match std::fs::read(path) {
            Ok(data) => match serde_json::from_slice::<SaveData>(&data) {
                Ok(save) => {
                    let summary = save_summary_from_data(&save);
                    dbg_eprintln!("Snapshot loaded: {:?}", path);
                    (save, Some(summary))
                }
                Err(e) => {
                    dbg_eprintln!("Error: failed to parse snapshot {:?}: {}", path, e);
                    (SaveData::new(), None)
                }
            },
            Err(e) => {
                dbg_eprintln!("Error: failed to read snapshot {:?}: {}", path, e);
                (SaveData::new(), None)
            }
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    fn parse_sram(path: &Path, data: &[u8]) -> (SaveData, Option<SaveFileSummary>) {
        match import_sram(data) {
            Ok(save) => {
                let summary = save_summary_from_data(&save);
                dbg_eprintln!("Save file loaded: {:?}", path);
                pokered_core::log_save!(
                    "position: map_id={}, x={}, y={}, dir={}",
                    save.game_data.position.map_id,
                    save.game_data.position.x,
                    save.game_data.position.y,
                    save.game_data.player_direction
                );
                (save, Some(summary))
            }
            Err(e) => {
                dbg_eprintln!("Warning: save file {:?} failed to load: {:?}", path, e);
                (SaveData::new(), None)
            }
        }
    }

    /// Read the companion script-flags file (native sidecar for the
    /// runtime-only dynamic keys — e.g. `__OBJ_HIDDEN_*` — that have no bit
    /// in the fixed SRAM event-flags region). Named event flags in old
    /// sidecars are harmless: `set_script_flags` routes them to the bitset.
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    fn read_companion_script_flags(save_path: Option<&Path>, legacy_native: bool) -> Option<pokered_core::hash_compat::HashMap<String, bool>> {
        let paths=companion_lookup_paths(save_path,legacy_native);
        let (flags_path,data)=paths.into_iter().find_map(|path|std::fs::read(&path).ok().map(|data|(path,data)))?;
        match serde_json::from_slice::<pokered_core::hash_compat::HashMap<String, bool>>(&data) {
            Ok(flags) => Some(flags),
            Err(e) => {
                dbg_eprintln!(
                    "Warning: failed to parse script flags {:?}: {}",
                    flags_path,
                    e
                );
                None
            }
        }
    }

    /// Same companion store on web: the runtime-only extras live in a
    /// separate `localStorage` key next to the SaveData JSON.
    #[cfg(target_arch = "wasm32")]
    fn read_companion_script_flags(_save_path: Option<&Path>, _legacy_native: bool) -> Option<pokered_core::hash_compat::HashMap<String, bool>> {
        let storage = web_local_storage()?;
        let data = storage.get_item(WEB_SCRIPT_FLAGS_STORAGE_KEY).ok()??;
        match serde_json::from_str::<pokered_core::hash_compat::HashMap<String, bool>>(&data) {
            Ok(flags) => Some(flags),
            Err(e) => {
                log::warn!("failed to parse script flags from localStorage: {}", e);
                None
            }
        }
    }

    /// Persist the runtime-only extras (companion sidecar) if any exist;
    /// remove a stale sidecar when none do, so a previous save's extras
    /// can't re-merge onto a different save on next load.
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    fn save_companion_script_flags(overworld: &OverworldScreen<PokemonRedData>, save_path: &Path) {
        let extras = overworld.unified_flags().extras();
        let flags_path = companion_path_for_save(save_path);
        if extras.is_empty() {
            if let Err(e) = std::fs::remove_file(&flags_path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    dbg_eprintln!("Error: failed to remove script flags file: {}", e);
                }
            }
            return;
        }
        match serde_json::to_string(extras) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&flags_path, json.as_bytes()) {
                    dbg_eprintln!("Error: failed to write script flags file: {}", e);
                }
            }
            Err(e) => {
                dbg_eprintln!("Error: failed to serialize script flags: {}", e);
            }
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    pub fn export_snapshot_from_sav(
        input_path: Option<&Path>,
        output_path: &Path,
    ) -> Result<(), String> {
        let sav_path = input_path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(save_file_path);
        let data = std::fs::read(&sav_path)
            .map_err(|e| format!("Failed to read {:?}: {}", sav_path, e))?;
        let save = import_sram(&data)
            .map_err(|e| format!("Failed to parse SRAM from {:?}: {:?}", sav_path, e))?;
        let json = serde_json::to_string_pretty(&save)
            .map_err(|e| format!("Failed to serialize snapshot: {}", e))?;
        std::fs::write(output_path, json.as_bytes())
            .map_err(|e| format!("Failed to write {:?}: {}", output_path, e))?;
        dbg_eprintln!(
            "Exported snapshot: {:?} -> {:?} ({} bytes)",
            sav_path,
            output_path,
            json.len()
        );
        Ok(())
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    pub fn import_snapshot_from_sav(input_path: &Path, output_path: &Path) -> Result<(), String> {
        let data = std::fs::read(input_path)
            .map_err(|e| format!("Failed to read {:?}: {}", input_path, e))?;
        let save = import_sram(&data)
            .map_err(|e| format!("Failed to parse SRAM from {:?}: {:?}", input_path, e))?;
        let json = serde_json::to_string_pretty(&save)
            .map_err(|e| format!("Failed to serialize snapshot: {}", e))?;
        std::fs::write(output_path, json.as_bytes())
            .map_err(|e| format!("Failed to write {:?}: {}", output_path, e))?;
        dbg_eprintln!(
            "Imported snapshot: {:?} -> {:?} ({} bytes)",
            input_path,
            output_path,
            json.len()
        );
        Ok(())
    }

    /// Apply live session state (names, player position/map, flags, tile
    /// animations) onto `save`. Shared by the hosted `build_save_data` clone
    /// path and the bare-metal in-place path, which cannot afford a 29 KB
    /// `SaveData` clone inside the update loop.
    fn apply_live_state_to_save(
        save: &mut SaveData,
        overworld: &OverworldScreen,
        player_name: &str,
        rival_name: &str,
    ) {
    if let Some(encoded) = pokered_data::charmap::encode_string(&player_name) {
        save.player_name = encoded;
    }
    if let Some(encoded) = pokered_data::charmap::encode_string(&rival_name) {
        save.game_data.rival_name = encoded;
    }

    let player = &overworld.state.player;
    let current_map = overworld.state.current_map;

    save.game_data.position.map_id = current_map as u8;
    save.game_data.position.x = player.x as u8;
    save.game_data.position.y = player.y as u8;
    save.game_data.position.x_block = (player.x % 2) as u8;
    save.game_data.position.y_block = (player.y % 2) as u8;
    if let Some(last_map) = overworld.last_map {
        save.game_data.last_map = last_map as u8;
    }

    // These SRAM fields store PLAYER_DIR_* bitmasks, not the sprite-facing
    // values (0/4/8/12). Original sprite_data_constants.asm: right/left/down/up
    // are bits 0/1/2/3. OverworldLoop clears moving direction while stopped.
    let facing = match player.facing {
        pokered_core::overworld::Direction::Down => 4u8,
        pokered_core::overworld::Direction::Up => 8u8,
        pokered_core::overworld::Direction::Left => 2u8,
        pokered_core::overworld::Direction::Right => 1u8,
    };
    save.game_data.player_direction = facing;
    // Movement history is exported by write_system_save_state below.

    pokered_core::log_save!(
        "build_save_data: map_id={}, x={}, y={}, dir={}, player.x={}, player.y={}",
        save.game_data.position.map_id,
        save.game_data.position.x,
        save.game_data.position.y,
        facing,
        player.x,
        player.y
    );

    // wCurrentMapHeight2/Width2 = block dimensions × 2
    let (map_w, map_h) = current_map.dimensions();
    save.game_data.current_map_height2 = map_h * 2;
    save.game_data.current_map_width2 = map_w * 2;

    if let Some(ref map_data) = overworld.map_data {
        save.game_data.map_header.tileset = map_data.tileset.to_u8();
        save.game_data.map_header.height = map_data.height;
        save.game_data.map_header.width = map_data.width;
    }

    // engine/menus/save.asm: hTileAnimations is stored into sTileAnimations
    // on save. It carries the current tileset's animation byte
    // (TILEANIM_*); map loads refresh it from the tileset header.
    save.tile_animations = match overworld.tile_anim.kind() {
        pokered_core::overworld::presentation::TileAnimKind::None => 0,
        pokered_core::overworld::presentation::TileAnimKind::Water => 1,
        pokered_core::overworld::presentation::TileAnimKind::WaterFlower => 2,
    };

    // The event-flag bitset serializes directly into the original
    // 320-byte SRAM region (wEventFlags, NUM_EVENTS = $A00 bits).
    save.game_data.event_flags = overworld.unified_flags().as_bytes().to_vec();
    overworld.write_system_save_state(&mut save.game_data);

    save.game_data.toggleable_object_flags = *overworld.toggleable_object_flags();
    save.game_data.obtained_hidden_items = *overworld.hidden_item_flags();
    save.game_data.obtained_hidden_coins = *overworld.hidden_coin_flags();

    }

    fn build_save_data(&self) -> SaveData {
        let mut save = self.save_data.clone();
        save.imported_legacy_native=false;
        save.imported_legacy_json=false;
        Self::apply_live_state_to_save(&mut save, &self.overworld, &self.player_name, &self.rival_name);
        save
    }

    /// `game.enterHallOfFame()` (drained from `overworld.pending_hof_ceremony`):
    /// record the party into the Hall of Fame (AnimateHallOfFame records each
    /// mon as it is shown — `HoFRecordMonInfo`, engine/movie/hall_of_fame.asm:
    /// 230-241 — then `SaveHallOfFameTeams` persists; net effect = one team
    /// pushed per League victory), reset `wLastBlackoutMap` to PALLET_TOWN
    /// (scripts/HallOfFame.asm:48-49), then start the roll-call takeover.
    fn start_hof_ceremony(&mut self) {
        use pokered_core::hof_ceremony::{HofCeremonyState, HofEntry, HofPlayerStats};
        use pokered_core::save::hall_of_fame::{HofMon, HofTeam};

        let mut team = HofTeam::new();
        let mut entries = Vec::new();
        for mon in self.save_data.party.iter() {
            let mut name_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
            let name = mon.display_name(&mut name_buf);
            entries.push(HofEntry {
                species: mon.species,
                level: mon.level,
                nickname: name.to_string(),
            });
            let encoded = pokered_data::charmap::encode_string(name).unwrap_or_default();
            team.add_mon(HofMon::new(mon.species as u8, mon.level, &encoded));
        }
        self.save_data.hall_of_fame.push_team(team);
        // wNumHoFTeams: incremented unless it would wrap to 0
        // (hall_of_fame.asm:66-70).
        self.save_data.game_data.num_hof_teams =
            self.save_data.game_data.num_hof_teams.saturating_add(1);
        // ld a, PALLET_TOWN / ld [wLastBlackoutMap], a (HallOfFame.asm:48-49).
        self.save_data.game_data.last_blackout_map = MapId::PalletTown as u8;

        let dex_seen = self.save_data.game_data.pokedex.seen_count();
        let dex_owned = self.save_data.game_data.pokedex.owned_count();
        let stats = HofPlayerStats {
            name: self.player_name.clone(),
            play_time_hours: self.save_data.game_data.play_time.hours as u16,
            play_time_minutes: self.save_data.game_data.play_time.minutes,
            money: self.save_data.game_data.player_money,
            dex_seen: dex_seen as u16,
            dex_owned: dex_owned as u16,
            rating: pokered_core::pc_screen::dex_rating_text(dex_owned),
        };
        self.hof_ceremony = Some(HofCeremonyState::new(entries, stats));
        // HoFFadeOutScreenAndMusic (hall_of_fame.asm:284-288).
        if let Some(ref audio) = self.audio {
            audio.fade_out(10);
        }
    }

    /// Post-credits: `SaveGameData` + `Init` (scripts/HallOfFame.asm:45-56).
    /// The original saves on the HallOfFame map and the main-menu CONTINUE
    /// handler special-warps the player back out (engine/menus/main_menu.asm:
    /// 114-125); we instead reposition the save to Pallet Town (the fly
    /// point, in front of the player's house) before saving — the
    /// player-visible result is the same, and loading can't replay the
    /// HallOfFame @load ceremony.
    fn finish_hof_ceremony(&mut self) {
        self.overworld.state.current_map = MapId::PalletTown;
        self.overworld.state.player.x = 5;
        self.overworld.state.player.y = 6;
        self.overworld.state.player.facing = pokered_core::overworld::Direction::Down;
        self.save_to_file();
        self.state.save_summary = Some(save_summary_from_data(&self.save_data));
        self.handle_transition(GameScreen::TitleScreen);
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    fn save_to_file(&mut self) {
        let save = self.build_save_data();
        if self.external_saves {
            let flags = self.overworld.script_flags();
            let envelope = MobileSave {
                version: 1,
                data: save.clone(),
                flags: flags.clone(),
            };
            if let Ok(json) = serde_json::to_string(&envelope) {
                self.save_data = save;
                self.mobile_flags = flags;
                self.committed_save = Some(json);
            }
            return;
        }
        let sram = export_sram(&save);
        // Explicit --save path wins (headless/driver runs); normal play
        // falls back to the default location next to the executable.
        let path = self.save_path.clone().unwrap_or_else(save_file_path);
        match std::fs::write(&path, &sram) {
            Ok(()) => {
                pokered_core::log_save!("game saved to {:?} ({} bytes)", path, sram.len());
                self.save_data = save;
            }
            Err(e) => {
                dbg_eprintln!("Error: failed to write save file: {}", e);
            }
        }
        Self::save_companion_script_flags(&self.overworld, &path);
    }

    /// Bare metal: persist the 32 KiB SRAM image directly onto the cartridge
    /// SRAM (memory-mapped at 0x0E00_0000). The live state is synced in place —
    /// no `SaveData` clone — because this runs inside the update loop, whose
    /// stack headroom cannot absorb a 29 KB temporary.
    /// Diagnostic hook: run the production bare-metal save path on demand
    /// (`--features repro-rival`), so hardware repros can save from states the
    /// autopilot cannot navigate to.
    #[cfg(all(target_os = "none", feature = "repro-markers"))]
    pub fn debug_save_now(&mut self) {
        self.save_to_file();
    }

    /// Exercise every visual command stream on the actual small-memory
    /// target, including moves whose battle effect would otherwise miss or
    /// end the encounter. Reuses the differential recorder's animation seam.
    #[cfg(all(target_os = "none", feature = "repro-markers"))]
    pub fn debug_start_memory_move(&mut self, move_id: pokered_data::moves::MoveId, player: bool) {
        self.battle.phase = pokered_core::battle::BattlePhase::PlayerMenu;
        self.battle_vfx = BattleVisualEffects::default();
        self.battle_vfx.prime_move_animation_capture_scene(&self.battle);
        self.battle_vfx.start_move_animation_capture(move_id, player);
    }

    #[cfg(all(target_os = "none", feature = "repro-markers"))]
    pub fn debug_memory_move_finished(&self) -> bool {
        self.battle_vfx.move_animation_capture_finished()
    }

    #[cfg(target_os = "none")]
    #[inline(never)]
    fn save_to_file(&mut self) {
        Self::apply_live_state_to_save(
            &mut self.save_data,
            &self.overworld,
            &self.player_name,
            &self.rival_name,
        );
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        // Stream bank by bank (8 KiB staging) — a single 32 KiB image
        // allocation cannot be satisfied at the lab/battle heap peaks
        // (largest free block measured at ~30 KiB right after the first
        // rival battle; a failed allocation is an invisible halt on
        // hardware).
        let mut bank = [0u8; 0x2000];
        for index in 0..4 {
            pokered_core::save::sram_export::export_sram_bank_into(
                &self.save_data,
                index,
                &mut bank,
            );
            pokered_core::save::gba_sram::write_bytes(index * 0x2000, &bank);
        }
        log::info!("gba: save written to cartridge SRAM");
        // Keep the in-memory summary in step so the save-overwrite prompt and
        // a soft reset see the just-written save.
        self.state.save_summary = Some(save_summary_from_data(&self.save_data));
    }

    /// Bare metal boot: load a saved game from cartridge SRAM when one is
    /// present and checksum-valid. Blank/corrupt media (all-zero or 0xFF) fails
    /// the region checksums, so a fresh cart naturally reports NEW GAME.
    /// A checksum-valid image with no player id is also treated as empty.
    ///
    /// Imports straight into the resident save slot: `import_sram`'s 29 KB
    /// return value does not fit the 64 KiB EWRAM stack (with fat-LTO
    /// inlining it once exploded `game_main`'s frame outright).
    #[cfg(target_os = "none")]
    #[inline(never)]
    pub fn try_load_sram_save(&mut self) {
        // Loading starts a new session. Release the previous sequencer,
        // resume caches and decoded graphics before parsing the new save.
        self.audio = None;
        if let Some(resources) = self.resources.as_mut() {
            resources.clear_cache();
        }
        // SRAM only supports byte-wide loads. Stream 8 KiB banks: the full
        // 32 KiB image cannot coexist with a full PC on the GBA heap.
        let result = import_sram_banks_into(
            |index, bank| pokered_core::save::gba_sram::read_bytes(index * bank.len(), bank),
            &mut self.save_data,
        );
        match result {
            Ok(()) if self.save_data.game_data.player_id != 0 => {
                let summary = save_summary_from_data(&self.save_data);
                self.state.save_summary = Some(summary.clone());
                self.main_menu = MainMenuState::new(Some(summary));
                apply_saved_options(&mut self.state.config, &self.save_data.game_data.options);
                log::info!("gba: loaded SRAM save");
            }
            other => {
                let rb = pokered_core::save::gba_sram::sram();
                let off = pokered_core::save::sram_layout::SRAM_BANK_SIZE_LAYOUT
                    + pokered_core::save::sram_layout::GAME_DATA_OFFSET;
                log::info!(
                    "gba: no valid SRAM save ({:?}); probe {:02x?}; region_len={} bank1_len={}; continuing as NEW GAME",
                    other.err(),
                    &rb[off..off + 8],
                    pokered_core::save::sram_import::canonical_region_len(),
                    pokered_core::save::sram_layout::SRAM_BANK_SIZE_LAYOUT
                );
                // Drop whatever a partial parse may have written (in place —
                // no temporary).
                self.save_data.clear();
            }
        }
        self.audio = AudioOutput::new();
    }

    #[cfg(target_arch = "wasm32")]
    fn save_to_file(&mut self) {
        let save = self.build_save_data();
        if self.external_saves {
            let flags = self.overworld.script_flags();
            let envelope = MobileSave {
                version: 1,
                data: save.clone(),
                flags: flags.clone(),
            };
            if let Ok(json) = serde_json::to_string(&envelope) {
                self.save_data = save;
                self.mobile_flags = flags;
                self.committed_save = Some(json);
            }
            return;
        }
        // Keep the SRAM round-trip on web for debug builds: this validates
        // that the in-memory state can be encoded into the canonical SRAM
        // layout (catches regressions identical to the native build).
        // We persist the higher-level JSON form (so the runtime-only flag
        // extras, which live outside the SRAM region, are preserved in a
        // separate storage key), so the SRAM bytes themselves are unused
        // at runtime.
        #[cfg(debug_assertions)]
        {
            let _sram = export_sram(&save);
        }

        let storage = match web_local_storage() {
            Some(s) => s,
            None => {
                log::warn!("cannot save: localStorage is unavailable");
                return;
            }
        };
        let json = match serde_json::to_string(&save) {
            Ok(j) => j,
            Err(e) => {
                log::error!("failed to serialize save: {}", e);
                return;
            }
        };
        match storage.set_item(WEB_SAVE_STORAGE_KEY, &json) {
            Ok(()) => {
                log::info!(
                    "game saved to localStorage (key={}, {} bytes)",
                    WEB_SAVE_STORAGE_KEY,
                    json.len()
                );
                let summary = save_summary_from_data(&save);
                self.state.save_summary = Some(summary);
                self.save_data = save;
            }
            Err(e) => {
                // Most commonly QuotaExceededError or SecurityError.
                log::error!("failed to write save to localStorage: {:?}", e);
            }
        }
        // Companion store for runtime-only extras (no SRAM bits); a stale
        // entry is removed when none exist so a previous save's extras
        // can't re-merge onto a different save on next load.
        let extras = self.overworld.unified_flags().extras();
        if extras.is_empty() {
            let _ = storage.remove_item(WEB_SCRIPT_FLAGS_STORAGE_KEY);
        } else if let Ok(json) = serde_json::to_string(extras) {
            let _ = storage.set_item(WEB_SCRIPT_FLAGS_STORAGE_KEY, &json);
        }
    }

    /// Export the current game state as SRAM bytes and write to an explicit path.
    /// Uses `build_save_data()` internally to capture current overworld state.
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    pub fn save_to_path(&mut self, path: &Path) -> Result<(), String> {
        let save = self.build_save_data();
        let sram = export_sram(&save);
        std::fs::write(path, &sram)
            .map_err(|e| format!("failed to write save to {:?}: {}", path, e))?;
        self.save_data = save;
        pokered_core::log_save!("game saved to {:?} ({} bytes)", path, sram.len());
        Ok(())
    }

    /// Load game state from SRAM bytes read from the given path.
    /// Updates `save_data` in-place; the caller should arrange for overlay
    /// reconstruction (the next `update()` frame will pick up the new data).
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
    pub fn load_from_path(&mut self, path: &Path) -> Result<(), String> {
        let data = std::fs::read(path)
            .map_err(|e| format!("failed to read save from {:?}: {}", path, e))?;
        let save = import_sram(&data)
            .map_err(|e| format!("failed to parse save from {:?}: {:?}", path, e))?;
        pokered_core::log_save!("save loaded from {:?}", path);
        self.save_data = save;
        Ok(())
    }

    pub fn handle_transition(&mut self, screen: GameScreen) {
        #[cfg(feature = "repro-markers")]
        log::info!(
            "mk: transition to {:?} free={}B",
            screen,
            largest_free_block()
        );
        self.prepare_gba_screen_resources(&screen);
        // Set in the Battle→Overworld settle below when a caught species was
        // newly added to the Pokédex — the post-capture "New DEX data will be
        // added…" entry then opens instead of the overworld
        // (engine/items/item_effects.asm:521-546).
        let mut post_catch_species: Option<pokered_data::species::Species> = None;
        match screen {
            GameScreen::IntroScene => {
                self.intro_scene.reset();
                if let Some(ref audio) = self.audio {
                    audio.play_music(MusicId::INTRO_BATTLE);
                }
            }
            GameScreen::TitleScreen => {
                let coming_from_intro = self.state.screen == GameScreen::IntroScene;
                self.title_screen.reset();
                if coming_from_intro {
                    // Skip copyright — go straight to Init (logo bounce etc.)
                    self.title_screen.phase = TitlePhase::Init;
                }
                self.prev_title_phase = Some(self.title_screen.phase);
            }
            GameScreen::MainMenu => {
                self.main_menu = MainMenuState::new(self.state.save_summary.clone());
            }
            GameScreen::OakSpeech => {
                // NEW GAME starts a fresh in-memory save before choosing a starter.
                // Keep the disk save/summary for Continue and overwrite confirmation.
                self.save_data.clear();
                self.oak_speech = OakSpeechState::new();
                if let Some(ref audio) = self.audio {
                    audio.stop_all();
                    audio.play_music(MusicId::ROUTES2);
                }
            }
            GameScreen::Overworld => {
                use pokered_core::data::fly_warp_data::NEW_GAME_WARP;
                use pokered_core::game_state::MainMenuChoice;

                // Boot/title/Oak graphics are dead once play begins. Reclaim
                // their decoded tiles before the overworld starts allocating
                // transient render and script data in EWRAM.
                #[cfg(target_os = "none")]
                if let Some(resources) = self.resources.as_mut() {
                    resources.clear_cache();
                }

                // Only create a new OverworldScreen when entering from the main menu
                // (Continue or New Game). When returning from sub-screens (Start menu,
                // Options, Save, Battle), keep the existing overworld state intact.
                //
                // Bag / PartyScreen / TownMap must NOT rebuild: field-item use,
                // medicine, TM teaching and FLY return to the overworld *in place*
                // — a rebuild from the (save-time) position would teleport the
                // player and drop the pending result dialogue (mirrors the TUI
                // fix from the same finding).
                match self.main_menu.last_choice {
                    Some(MainMenuChoice::Continue)
                        if !is_ingame_session_screen(&self.state.screen) =>
                    {
                        let (map_id, px, py, facing) =
                            if let Some((warp_map, warp_x, warp_y)) = self.startup_warp.take() {
                                dbg_eprintln!("Warping to {:?} ({}, {})", warp_map, warp_x, warp_y);
                                (
                                    warp_map,
                                    warp_x,
                                    warp_y,
                                    pokered_core::overworld::Direction::Down,
                                )
                            } else {
                                let pos = &self.save_data.game_data.position;
                                pokered_core::log_save!(
                                    "continue: loading from save: map_id={}, x={}, y={}, dir={}",
                                    pos.map_id,
                                    pos.x,
                                    pos.y,
                                    self.save_data.game_data.player_direction
                                );
                                let map_id = pokered_core::data::maps::MapId::from_u8(pos.map_id)
                                    .unwrap_or(NEW_GAME_WARP.map_id);
                                // Original Continue sets wPlayerDirection to
                                // PLAYER_DIR_DOWN before SpecialEnterMap; it
                                // does not restore the saved movement byte as
                                // a sprite-facing value.
                                let facing = pokered_core::overworld::Direction::Down;
                                (map_id, pos.x as u16, pos.y as u16, facing)
                            };
                        #[cfg(not(target_os = "none"))]
                        let mut overworld =
                            OverworldScreen::new(map_id, self.scripts_dir.clone(), PokemonRedData);
                        #[cfg(target_os = "none")]
                        let mut overworld = OverworldScreen::new(map_id, PokemonRedData);
                        overworld.restore_saved_last_map(self.save_data.game_data.last_map);
                        overworld.state.player.x = px;
                        overworld.state.player.y = py;
                        overworld.state.player.facing = facing;
                        self.player_name =
                            pokered_data::charmap::decode_string(&self.save_data.player_name);
                        self.rival_name = pokered_data::charmap::decode_string(
                            &self.save_data.game_data.rival_name,
                        );
                        // Seed the event-flag bitset from SRAM bytes, then
                        // merge any runtime-only extras (companion sidecar)
                        // on top.
                        #[cfg(not(target_os = "none"))]
                        overworld.restore_loaded_save_flags(&self.save_data, self.companion_flags());
                        #[cfg(target_os = "none")]
                        overworld.restore_loaded_save_flags(&self.save_data, None);
                        overworld.set_toggleable_object_flags(
                            self.save_data.game_data.toggleable_object_flags,
                        );
                        overworld
                            .set_hidden_item_flags(self.save_data.game_data.obtained_hidden_items);
                        overworld
                            .set_hidden_coin_flags(self.save_data.game_data.obtained_hidden_coins);
                        overworld.apply_hidden_object_flags();
                        overworld.player_name = self.player_name.clone();
                        overworld.rival_name = self.rival_name.clone();
                        overworld.party_count = self.save_data.party.count() as u8;
                        overworld.party_lead_level = self.save_data.party.leader_level();
                        overworld.run_on_load();
                        overworld.set_script_lang(
                            if self.state.config.language == pokered_core::game_state::Lang::Zh {
                                "zh"
                            } else {
                                "en"
                            },
                        );
                        self.overworld = overworld;
                        pokered_core::log_save!(
                            "continue: overworld created: player x={}, y={}, map={:?}",
                            self.overworld.state.player.x,
                            self.overworld.state.player.y,
                            self.overworld.state.current_map
                        );
                        if let Some(ref audio) = self.audio {
                            audio.play_music(MusicId::PALLET_TOWN);
                        }
                    }
                    Some(MainMenuChoice::NewGame)
                        if !is_ingame_session_screen(&self.state.screen) =>
                    {
                        // InitOptions (engine/menus/main_menu.asm): a NEW GAME
                        // resets wOptions to defaults (medium text, animation
                        // on, shift style), discarding any save-file options.
                        let defaults =
                            pokered_core::game_state::GameConfig::new(self.state.config.version);
                        self.state.config.text_speed = defaults.text_speed;
                        self.state.config.battle_animation = defaults.battle_animation;
                        self.state.config.battle_style = defaults.battle_style;
                        let (map_id, px, py) =
                            if let Some((warp_map, warp_x, warp_y)) = self.startup_warp.take() {
                                dbg_eprintln!("Warping to {:?} ({}, {})", warp_map, warp_x, warp_y);
                                (warp_map, warp_x, warp_y)
                            } else {
                                (
                                    NEW_GAME_WARP.map_id,
                                    NEW_GAME_WARP.coords.x as u16,
                                    NEW_GAME_WARP.coords.y as u16,
                                )
                            };
                        #[cfg(not(target_os = "none"))]
                        {
                            let mut overworld = OverworldScreen::new(
                                map_id,
                                self.scripts_dir.clone(),
                                PokemonRedData,
                            );
                            overworld.state.player.x = px;
                            overworld.state.player.y = py;
                            // NEW GAME installs the freshly-reset save's
                            // toggleable-object flags
                            // (InitializeToggleableObjectsFlags: story-gated
                            // objects like Pallet Town Oak start hidden).
                            // Without this the screen's all-zero default makes
                            // apply_hidden_object_flags re-show them on the
                            // first warp.
                            overworld.set_toggleable_object_flags(
                                self.save_data.game_data.toggleable_object_flags,
                            );
                            overworld.apply_hidden_object_flags();
                            overworld.player_name = self.player_name.clone();
                            overworld.rival_name = self.rival_name.clone();
                            overworld.party_count = self.save_data.party.count() as u8;
                            overworld.party_lead_level = self.save_data.party.leader_level();
                            overworld.set_script_lang(
                                if self.state.config.language == pokered_core::game_state::Lang::Zh
                                {
                                    "zh"
                                } else {
                                    "en"
                                },
                            );
                            self.overworld = overworld;
                        }
                        #[cfg(target_os = "none")]
                        {
                            debug_assert_eq!(self.overworld.state.current_map, map_id);
                            self.overworld.state.player.x = px;
                            self.overworld.state.player.y = py;
                            // Same NEW GAME seeding as the native arm: install
                            // the fresh save's toggleable-object flags so
                            // story-gated objects (Pallet Town Oak) start
                            // hidden.
                            self.overworld.set_toggleable_object_flags(
                                self.save_data.game_data.toggleable_object_flags,
                            );
                            self.overworld.apply_hidden_object_flags();
                            self.overworld.player_name = self.player_name.clone();
                            self.overworld.rival_name = self.rival_name.clone();
                            self.overworld.party_count = self.save_data.party.count() as u8;
                            self.overworld.party_lead_level = self.save_data.party.leader_level();
                            self.overworld.set_script_lang(
                                if self.state.config.language == pokered_core::game_state::Lang::Zh
                                {
                                    "zh"
                                } else {
                                    "en"
                                },
                            );
                        }
                        if let Some(ref audio) = self.audio {
                            audio.play_music(MusicId::PALLET_TOWN);
                        }
                    }
                    _ => {
                        if self.state.screen == GameScreen::Battle {
                            // Was the caught species new to the Pokédex? Checked
                            // BEFORE the settle flips the owned bit (the original
                            // tests wPokedexOwned before setting it).
                            post_catch_species = self
                                .battle
                                .captured_mon
                                .as_ref()
                                .map(|c| c.species)
                                .filter(|sp| !self.save_data.game_data.pokedex.is_owned(*sp));
                            // Fold the finished battle into the save (money / blackout /
                            // party writeback / catch / Pokédex / bag / encounter state).
                            // Shared verbatim with the TUI frontend.
                            let writeback =
                                pokered_core::battle::settlement::settle_battle_into_save(
                                    &mut self.battle,
                                    &mut self.save_data,
                                    &mut self.overworld,
                                );
                            let battle_outcome = writeback.outcome;
                            // EvolutionAfterBattle (engine/pokemon/evos_moves.asm):
                            // level-up evolutions detected at battle end play as the
                            // cutscene in the overworld, BEFORE the map music restarts
                            // (EndOfBattle runs it before the map reload; the original
                            // ends it with PlayDefaultMusic, evos_moves.asm:257-259).
                            if !writeback.pending_evolutions.is_empty() {
                                self.queue_evolution_cutscene(writeback.pending_evolutions, None);
                            }
                            // Safari: fold the balls thrown this battle back into the
                            // overworld game (its zero-ball game-over / eject keys off this).
                            if self.battle.is_safari {
                                let remaining = self.battle.safari.as_ref().map_or(0, |s| s.balls);
                                while self.overworld.safari_balls_remaining() > remaining {
                                    self.overworld.use_safari_ball();
                                }
                            }
                            if let Some(ref audio) = self.audio {
                                // Battle end clears the low-health alarm
                                // (engine/battle/end_of_battle.asm:48).
                                audio.set_low_health_alarm(false);
                                // When an evolution cutscene is queued it owns
                                // the music (stop-all → SFX_TINK →
                                // MUSIC_SAFARI_ZONE); the map music restarts
                                // when it finishes (PlayDefaultMusic).
                                if self.evolution_anim.is_none() {
                                    let map = self.overworld.state.current_map;
                                    let data_id =
                                        pokered_core::overworld::map_loading::get_map_music(map);
                                    if let Some(id) = MusicId::from_u8(data_id as u8) {
                                        // Original uses fade_speed=8 after battle
                                        audio.play_music_with_fade(id, 8);
                                    }
                                }
                            }

                            // If a script was suspended on `await
                            // game.startBattle(...)`, resume it now with the
                            // outcome. No-op for non-script (sight-engaged)
                            // battles. Safe on a loss: the win-branch is skipped
                            // and the subsequent map load clears the script.
                            if let Some(outcome) = battle_outcome {
                                self.overworld.resume_script_after_battle(outcome);
                            }
                            // MapEntryAfterBattle (home/overworld.asm): the
                            // overworld fades back in from white after a battle.
                            // In a dark cave the original uses LoadGBPal instead
                            // (instant dark palette) — the renderer's dark-cave
                            // priority reproduces that without a special case here.
                            // A queued blackout warp (battle loss) already started
                            // its own fade-out; don't clobber it or the warp would
                            // never commit.
                            if self.overworld.pending_warp.is_none() {
                                self.overworld.warp_fade_state =
                                    pokered_core::overworld::screen::WarpFadeState::FadingIn {
                                        frames_remaining:
                                            pokered_core::overworld::screen::WARP_FADE_IN_FRAMES,
                                    };
                            }
                        }
                    }
                }
            }
            GameScreen::Battle => {
                if self.battle.battle_state.is_none() {
                    self.battle = BattleScreen::new(true);
                    self.battle.is_zh =
                        self.state.config.language == pokered_core::game_state::Lang::Zh;
                    self.battle.player_money = self.save_data.game_data.player_money;
                    self.battle_vfx = BattleVisualEffects::default();
                    self.battle_prev_message = None;
                    self.faint_thud_pending = false;
                    if let Some(ref audio) = self.audio {
                        audio.play_music(MusicId::WILD_BATTLE);
                    }
                }
            }
            GameScreen::StartMenu => {
                // Read the LIVE overworld flag store (unified_flags), not
                // the `save_data` snapshot which is only synced at save time.
                let has_pokedex = self.overworld.unified_flags().get_flag("EVENT_GOT_POKEDEX");
                let has_pokemon = self.save_data.party.count() > 0;
                self.start_menu.open(has_pokedex, has_pokemon, false);
                // PrintSafariZoneSteps (player_state.asm:219-255): while a
                // Safari run is live, the START menu shows steps/balls.
                self.start_menu.safari_info = if self.overworld.is_safari_game_active() {
                    Some(pokered_core::start_menu::SafariZoneInfo {
                        steps: self.overworld.safari_steps_remaining(),
                        balls: self.overworld.safari_balls_remaining(),
                    })
                } else {
                    None
                };
                if self.state.screen == GameScreen::Overworld {
                    let previous=self.overworld.sampled_player_input();
                    self.overworld.prepare_start_menu_sprite(has_pokedex);
                    self.start_menu.begin_field_initialization_with_portion(StartMenuInput {
                        up:previous.up,down:previous.down,a:previous.a,b:previous.b,start:previous.start,
                    }, self.overworld.bg_transfer_portion);
                } else {
                    if self.state.screen == GameScreen::PartyScreen {
                        // This lifecycle now blocks the short DOWN/A pulses
                        // while sprites/font/background are restored. The
                        // global CPU/PPU carry is not tracked yet: a zero work
                        // phase remains provisional and is audited separately.
                        self.overworld.begin_party_menu_restore(0, 16);
                    }
                    let previous = self.overworld.sampled_player_input();
                    self.start_menu.begin_redisplay_initialization(StartMenuInput {
                        up: previous.up, down: previous.down, a: previous.a,
                        b: previous.b, start: previous.start,
                    });
                }
            }
            GameScreen::OptionsMenu => {
                // Seed the menu from the live config so every row shows the
                // current setting.
                use pokered_core::game_state as gs;
                use pokered_core::options_menu as om;
                self.options_menu = OptionsMenuState::new(GameOptions {
                    text_speed: match self.state.config.text_speed {
                        gs::TextSpeed::Fast => om::TextSpeed::Fast,
                        gs::TextSpeed::Medium => om::TextSpeed::Medium,
                        gs::TextSpeed::Slow => om::TextSpeed::Slow,
                    },
                    battle_animation: if self.state.config.battle_animation {
                        BattleAnimation::On
                    } else {
                        BattleAnimation::Off
                    },
                    battle_style: match self.state.config.battle_style {
                        gs::BattleStyle::Shift => om::BattleStyle::Shift,
                        gs::BattleStyle::Set => om::BattleStyle::Set,
                    },
                });
            }
            GameScreen::SaveMenu => {
                let has_previous = self.state.has_save_file();
                // CheckPreviousSaveFile (engine/menus/save.asm:156-164,
                // 622-653): when the stored file belongs to a DIFFERENT
                // trainer ID, saving first asks "The older file will be
                // erased. Is that okay?" — compare the disk summary's ID
                // against the in-memory one.
                let is_different_player = self.state.save_summary.as_ref().map_or(false, |s| {
                    s.player_id != 0 && s.player_id != self.save_data.game_data.player_id
                });
                self.save_menu = SaveMenuState::new(
                    SaveScreenInfo {
                        player_name: self.player_name.clone(),
                        num_badges: self.save_data.game_data.badge_count(),
                        pokedex_owned: self.save_data.game_data.pokedex.owned_count() as u16,
                        play_time_hours: self.save_data.game_data.play_time.hours as u16,
                        play_time_minutes: self.save_data.game_data.play_time.minutes,
                    },
                    has_previous,
                    is_different_player,
                );
            }
            GameScreen::PartyScreen => {
                // Opened from the bag to apply an item → item-use mode (A on a
                // Pokémon applies the pending item, no STATS/SWITCH menu).
                // Post-evolution full-moveset learn → straight into the
                // "which move should be forgotten?" phase for that member.
                // SOFTBOILED chosen in the action menu → target-pick mode.
                let saved_cursor = self.party_screen.cursor();
                self.party_screen = match (
                    self.pending_bag_item,
                    self.pending_evolve_move_replace,
                    self.pending_softboiled_user,
                ) {
                    (Some(item), _, _) => {
                        PartyScreenState::new_for_item(self.save_data.party.to_vec(), item)
                    }
                    (None, Some((party_index, _)), _) => PartyScreenState::new_for_move_choice(
                        self.save_data.party.to_vec(),
                        party_index,
                    ),
                    (None, None, Some(user)) => PartyScreenState::new_for_softboiled_target(
                        self.save_data.party.to_vec(),
                        user,
                    ),
                    (None, None, None) => PartyScreenState::new(self.save_data.party.to_vec()),
                };
                if self.pending_evolve_move_replace.is_none() {
                    self.party_screen.set_cursor(saved_cursor);
                }
            }
            GameScreen::PokemonStatsScreen(_) => {
                self.pc_stats_return_frame = None;
                if let Some(stats) = &mut self.stats_screen { stats.start_entry(); }
                if let Some(audio) = &self.audio {
                    audio.play_sfx(SfxId::PressAB);
                    audio.set_master_volume(3, 3);
                }
            }
            GameScreen::LanguageSelect => {}
            GameScreen::GameFreakSplash => {
                self.gamefreak_splash.reset();
            }
            GameScreen::CopyrightSplash => {
                self.title_screen.reset();
            }
            GameScreen::Shop(_) => {}
            GameScreen::PC => {
                // The PC screen is initialized when `pending_pc` is consumed
                // (entry kind from the scene script + flags snapshot).
            }
            GameScreen::Bag => {
                self.bag_screen =
                    BagScreenState::new(self.save_data.game_data.bag.items().to_vec());
            }
            GameScreen::TownMap => {
                self.town_map_screen = if self.pending_fly_map {
                    self.pending_fly_map = false;
                    TownMapScreenState::new_fly(
                        self.overworld.state.current_map,
                        self.save_data.game_data.fly_destinations(),
                    )
                } else {
                    TownMapScreenState::new(self.overworld.state.current_map)
                };
            }
            GameScreen::Slots => {
                // The slots screen is initialized when `pending_slots` is
                // consumed (so it captures the live coin balance + seed).
            }
            GameScreen::Elevator => {
                // The elevator screen is initialized when `pending_elevator`
                // is consumed (with the floor list from the scene script).
            }
            GameScreen::FilterBag => {
                // The filtered-bag screen is initialized when
                // `pending_filter_bag` is consumed (carried candidates only).
            }
            GameScreen::Diploma => {
                // Full-screen certificate; closed by A/B in the update loop.
            }
            GameScreen::Pokedex => {
                // Start-menu POKéDEX: open the CONTENTS list. (The post-capture
                // entry builds its own state below and never reaches this arm.)
                self.pokedex_screen = PokedexScreenState::new(
                    self.save_data.game_data.pokedex.clone(),
                    self.state.config.version,
                );
            }
            GameScreen::TrainerCard => {
                self.trainer_card_screen = TrainerCardScreenState::new();
            }
        }
        if let Some(species) = post_catch_species {
            // "New DEX data will be added…": show the new species' entry before
            // returning to the overworld (item_effects.asm:541-546). The jingle
            // plays here; the cry fires from the update loop via cry_pending.
            self.pokedex_screen = PokedexScreenState::new_entry(
                self.save_data.game_data.pokedex.clone(),
                species,
                self.state.config.version,
            );
            if let Some(ref audio) = self.audio {
                audio.play_sfx(SfxId::DexPageAdded);
            }
            self.state.transition_to(GameScreen::Pokedex);
            return;
        }
        self.state.transition_to(screen);
    }

    /// Reclaim decoded graphics at GBA full-screen boundaries. Leaving battle
    /// must drop its combined tileset before another full-screen renderer
    /// starts allocating. The Overworld→Battle boundary is handled when the
    /// transition snapshot stops being visible, immediately before battle
    /// graphics are allocated.
    fn prepare_gba_screen_resources(&mut self, next: &GameScreen) {
        #[cfg(target_os = "none")]
        {
            if self.state.screen == GameScreen::Battle && *next != GameScreen::Battle {
                self.battle_vfx = BattleVisualEffects::default();
            }
            if matches!(next, GameScreen::Overworld | GameScreen::Pokedex) {
                if let Some(resources) = self.resources.as_mut() {
                    resources.clear_cache();
                }
            }
        }
        #[cfg(not(target_os = "none"))]
        let _ = next;
    }

    /// Restore the previous physical sample after a menu/cutscene gap.
    /// A held confirmation must not become a fresh field press; a genuinely
    /// new press on the first returning frame must remain detectable.
    fn sync_overworld_input_edges(&mut self, input: &InputState) {
        let previously_held = |b| input.is_just_released(b)
            || (input.is_held(b) && !input.is_just_pressed(b));
        self.overworld.sync_prev_input(
            previously_held(GbButton::A),
            previously_held(GbButton::B),
            previously_held(GbButton::Up),
            previously_held(GbButton::Down),
        );
    }

    /// A+B+Start+Select held for 16 frames — the original's soft reset
    /// (engine/joypad.asm `TrySoftReset` → home/init.asm `SoftReset`): stop
    /// all sounds, reload the save from disk (unsaved progress is lost, as on
    /// hardware), and return to the title screen.
    fn soft_reset(&mut self) {
        #[cfg(not(target_os = "none"))]
        {
            if self.external_saves {
                let save = self.committed_save.clone();
                if let Ok(mut replacement) =
                    Self::new_mobile(self.state.config.version, save.as_deref())
                {
                    replacement.handle_transition(GameScreen::TitleScreen);
                    *self = replacement;
                }
                return;
            }
        }
        if let Some(ref audio) = self.audio {
            audio.stop_all();
        }
        #[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
        {
            let path = self
                .save_path
                .clone()
                .or_else(|| save_file_path().exists().then(save_file_path));
            if let Some(path) = path {
                let _ = self.load_from_path(&path);
            }
        }
        self.handle_transition(GameScreen::TitleScreen);
    }

    fn game_timer_active(&self) -> bool {
        // TrackPlayTime runs from the VBlank interrupt every frame
        // (home/vblank.asm:75); its only gate is BIT_GAME_TIMER_COUNTING,
        // set once at SpecialEnterMap (main_menu.asm:333-334) and never
        // cleared anywhere in the original — so the clock keeps running in
        // battles, menus, and dialogs.
        let counting_started = !matches!(
            self.state.screen,
            GameScreen::GameFreakSplash
                | GameScreen::CopyrightSplash
                | GameScreen::TitleScreen
                | GameScreen::MainMenu
                | GameScreen::OakSpeech
                | GameScreen::LanguageSelect
        );
        counting_started
    }

    /// Localize a core-generated English message before queuing it as an
    /// overworld dialogue (Chinese selected via the language screen). The
    /// overworld's own scene text is already bilingual and passes through.
    fn localize_dialogue(&self, text: &str) -> String {
        if self.state.config.language == pokered_core::game_state::Lang::Zh {
            pokered_data::dialog_text::localize(text)
        } else {
            text.to_string()
        }
    }

    /// Compute the per-battle RNG stream for a battle about to start
    /// (agent M5): deterministic from `seed + battle_count` — successive
    /// battles differ, replays match. The caller assigns it onto the
    /// freshly constructed BattleScreen (constructors seed from entropy,
    /// so assigning before construction would discard it). Advances the
    /// counter only when a seed is pinned.
    fn next_battle_rng(
        &mut self,
    ) -> Option<pokered_core::battle::pokered_rules::runtime::StdBattleRng> {
        self.seed.map(|seed| {
            self.battle_count += 1;
            pokered_core::battle::pokered_rules::runtime::StdBattleRng::from_seed(
                seed.wrapping_add(0x9E3779B97F4A7C15)
                    .wrapping_add(self.battle_count),
            )
        })
    }

    /// Drop decoded overworld graphics before constructing a battle. On GBA,
    /// the battle parties and state are allocated while the script that
    /// requested the fight is still suspended; retaining the map cache across
    /// that heap peak can exhaust EWRAM before the transition renderer gets a
    /// chance to clear it.
    fn prepare_gba_battle_allocation(&mut self) {
        #[cfg(target_os = "none")]
        if let Some(resources) = self.resources.as_mut() {
            resources.clear_cache();
        }
    }

    fn start_wild_battle(&mut self, species: pokered_data::species::Species, level: u8) {
        // InitBattleVariables clears wPartyAndBillsPCSavedMenuItem.
        self.party_screen.set_cursor(0);
        use pokered_core::pokemon::stats::create_pokemon;

        self.prepare_gba_battle_allocation();
        let mut battle_rng = self.next_battle_rng();
        // Wild DVs are two random bytes (core.asm:6012-6019) — drawn from
        // the (possibly seeded) battle stream so encounters replay. The
        // BattleRng trait keeps this call site free of a direct rand dep
        // (rand is hosted-only for pokered-app).
        let dvs = match battle_rng.as_mut() {
            Some(rng) => [
                dotzuki_engine::battle::rng::BattleRng::next_u8(rng),
                dotzuki_engine::battle::rng::BattleRng::next_u8(rng),
            ],
            None => pokered_core::pokemon::stats::roll_random_dvs(),
        };
        let enemy_mon = create_pokemon(species, level, dvs);
        let player_party = self.save_data.party.to_vec();

        if let Some(enemy) = enemy_mon {
            if !player_party.is_empty() {
                self.battle = pokered_core::battle::BattleScreen::from_parties(
                    true,
                    &player_party,
                    &[enemy],
                    None,
                );
            } else {
                self.battle = pokered_core::battle::BattleScreen::new(true);
            }
        } else {
            self.battle = pokered_core::battle::BattleScreen::new(true);
        }
        if let Some(rng) = battle_rng {
            self.battle.rng = rng;
        }
        self.battle.is_zh = self.state.config.language == pokered_core::game_state::Lang::Zh;
        self.battle.player_money = self.save_data.game_data.player_money;
        // Badge stat boosts + traded-mon obedience context (wObtainedBadges /
        // wPlayerID) — the battle reads them from these fields every turn.
        self.battle.player_badges = self.save_data.game_data.obtained_badges;
        self.battle.player_id = self.save_data.game_data.player_id;
        // BoxFullCannotThrowBall guard context (app-side snapshot).
        self.battle.player_box_full = self.save_data.current_box.is_full();
        self.battle.map_id = self.overworld.state.current_map as u8;
        // Give the battle a copy of the bag so balls/items are usable in-battle;
        // synced back afterwards so consumed items are deducted from the save.
        self.battle.player_bag = self.save_data.game_data.bag.clone();
        // Pokémon Tower without the Silph Scope: the wild mon appears as an unidentified,
        // uncatchable "GHOST" (name + sprite override in the renderer; use_ball dodged).
        let in_pokemon_tower = {
            use pokered_core::data::maps::MapId;
            let m = self.overworld.state.current_map as u8;
            m >= MapId::PokemonTower1F as u8 && m <= MapId::PokemonTower7F as u8
        };
        let has_silph_scope = self
            .save_data
            .game_data
            .bag
            .has_item(pokered_data::items::ItemId::SilphScope, 1);
        let is_ghost = in_pokemon_tower && !has_silph_scope;
        self.battle.is_ghost = is_ghost;
        // The scripted RESTLESS_SOUL battle (6F) fought WITH the scope: the original
        // checks `cp RESTLESS_SOUL` (constants/pokemon_constants.asm:209 —
        // `RESTLESS_SOUL EQU MAROWAK`) and runs the SILPH SCOPE unveil text +
        // MarowakAnim, after which it's a normal Marowak fight.
        self.battle.ghost_marowak_reveal = in_pokemon_tower
            && has_silph_scope
            && species == pokered_data::species::Species::Marowak;
        self.battle_vfx = BattleVisualEffects::default();
        self.battle_prev_message = None;
        self.faint_thud_pending = false;
        // Encountering a wild Pokémon registers it as seen — but NOT a GHOST (the
        // species stays unidentified until the Silph Scope reveals it).
        if !is_ghost {
            self.save_data.game_data.pokedex.set_seen(species);
        }

        // Safari Zone during an active Safari Game → the BALL/BAIT/ROCK/RUN Safari mode
        // (no attacking; the ball economy + bait/rock catch-flee mechanics take over).
        if pokered_data::map_flags::is_safari_zone_map(self.overworld.state.current_map)
            && self.overworld.is_safari_game_active()
        {
            let base_catch = pokered_data::pokemon_data::get_base_stats(species)
                .map(|s| s.catch_rate)
                .unwrap_or(255);
            let balls = self.overworld.safari_balls_remaining();
            self.battle.is_safari = true;
            self.battle.safari = Some(pokered_core::battle::safari::SafariState::new(
                base_catch, balls,
            ));
            self.battle.safari_menu = pokered_core::battle::menu::SafariBattleMenuState::new(balls);
        }

        if let Some(ref audio) = self.audio {
            if let Some(id) = MusicId::from_u8(self.battle.battle_music_id()) {
                audio.play_music(id);
            }
        }
    }

    /// Editor quick-entry: start a wild battle against `species` at `level`
    /// (the WYSIWYG "test this Pokémon in battle" flow from pokered-runner-web).
    /// Seeds the player a starter when the party is empty so the battle always
    /// has a battler to send out; otherwise it reuses the party exactly like a
    /// normal wild encounter (including the seen-Pokédex registration).
    pub fn debug_start_wild_battle(&mut self, species: pokered_data::species::Species, level: u8) {
        use pokered_core::pokemon::stats::create_pokemon;
        use pokered_data::species::Species;
        if self.save_data.party.is_empty() {
            if let Some(starter) = create_pokemon(Species::Bulbasaur, 5, [0x9A, 0x78]) {
                let _ = self.save_data.party.add(starter);
            }
        }
        self.start_wild_battle(species, level);
        self.handle_transition(GameScreen::Battle);
    }

    /// Editor quick-entry: open the Pokédex directly on `species`' entry (the
    /// post-capture style — full data + cry, see `PokedexScreenState::new_entry`).
    /// The species is registered as seen AND owned so the entry always shows its
    /// flavor text; closing the Pokédex returns to the overworld.
    pub fn debug_open_pokedex(&mut self, species: pokered_data::species::Species) {
        use pokered_core::pokedex_screen::PokedexScreenState;
        self.prepare_gba_screen_resources(&GameScreen::Pokedex);
        self.save_data.game_data.pokedex.set_seen(species);
        self.save_data.game_data.pokedex.set_owned(species);
        self.pokedex_screen = PokedexScreenState::new_entry(
            self.save_data.game_data.pokedex.clone(),
            species,
            self.state.config.version,
        );
        self.state.transition_to(GameScreen::Pokedex);
    }

    /// Editor quick-entry: start a trainer battle against `class` using its
    /// `party_index`-th party (0-based) — the WYSIWYG "test this trainer"
    /// flow from pokered-runner-web. Reuses the normal trainer-battle setup
    /// (rival type advantage, money/badge context, seen registration) via
    /// `start_trainer_battle`; an out-of-range party index falls back to the
    /// class's first party.
    pub fn debug_start_trainer_battle(
        &mut self,
        class: pokered_data::trainer_data::TrainerClass,
        party_index: usize,
    ) {
        // make_trainer_id takes the 1-based set number.
        let index = (party_index + 1).min(255) as u8;
        let trainer_id = pokered_data::trainer_data::make_trainer_id(class, index);
        self.start_trainer_battle(&trainer_id, None);
        self.handle_transition(GameScreen::Battle);
    }

    /// Editor quick-entry: verify a move in battle — a Lv25 Pikachu tester
    /// that knows `move_id` is staged as the party lead against a Lv25 wild
    /// Pidgey, then a normal wild battle starts (so every battle-side effect
    /// of the move — power/accuracy/PP/effect/type — is exercisable). Runs on
    /// the scratch session, so staging the tester never touches a real save.
    pub fn debug_start_move_test(&mut self, move_id: pokered_data::moves::MoveId) {
        use pokered_core::pokemon::party::Party;
        use pokered_core::pokemon::stats::create_pokemon_with_moves;
        use pokered_data::species::Species;

        let tester = create_pokemon_with_moves(
            Species::Pikachu,
            25,
            [0xFF, 0xFF],
            [
                move_id,
                pokered_data::moves::MoveId::None,
                pokered_data::moves::MoveId::None,
                pokered_data::moves::MoveId::None,
            ],
        )
        .expect("Pikachu Lv25 is a valid tester");

        // Lead the tester (keep the rest of the scratch party, max 6 total).
        let mut party = self.save_data.party.to_vec();
        party.truncate(5);
        party.insert(0, tester);
        if let Ok(p) = Party::from_pokemon(party) {
            self.save_data.party = p;
        }

        self.start_wild_battle(Species::Pidgey, 25);
        self.handle_transition(GameScreen::Battle);
    }

    /// Editor quick-entry: play the evolution animation `from` → `to` (the
    /// WYSIWYG "test this evolution" flow). The animation takeover renders
    /// until it finishes; the staged party slot is the morph's target.
    pub fn debug_play_evolution(
        &mut self,
        from: pokered_data::species::Species,
        to: pokered_data::species::Species,
    ) {
        use pokered_core::evolution_screen::{EvolutionScreenState, PendingEvolution};
        use pokered_core::pokemon::stats::create_pokemon;

        // The animation reads the party slot's display name; ensure a slot
        // exists (a fresh test session may have an empty party).
        if self.save_data.party.is_empty() {
            if let Some(mon) = create_pokemon(from, 25, [0x9A, 0x78]) {
                let _ = self.save_data.party.add(mon);
            }
        }
        let mut name_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
        let name = self
            .save_data
            .party
            .get(0)
            .map(|m| m.display_name(&mut name_buf))
            .unwrap_or("")
            .to_string();
        let queue = vec![PendingEvolution {
            party_index: 0,
            from,
            to,
            name,
            // Level-up style (B cancels) — the more common authoring flow.
            force: false,
        }];
        self.evolution_anim = Some(EvolutionScreenState::new(
            queue,
            None,
            self.state.config.language == pokered_core::game_state::Lang::Zh,
        ));
    }

    /// Editor debug: fully restore the player — the overworld party is healed
    /// (HP/status/PP) and, mid-battle, the active battler state is reset to
    /// full (HP/status/PP, stat stages, confusion/toxic/disable/substitute).
    pub fn debug_full_heal(&mut self) {
        use pokered_core::battle::state::StatusCondition;
        use pokered_core::pokemon::move_learning::get_move_max_pp;

        self.save_data.party.heal_all();
        if let Some(state) = self.battle.battle_state.as_mut() {
            for mon in state.player.party.iter_mut() {
                mon.hp = mon.max_hp;
                mon.status = StatusCondition::None;
                mon.pp = [
                    get_move_max_pp(mon.moves[0]),
                    get_move_max_pp(mon.moves[1]),
                    get_move_max_pp(mon.moves[2]),
                    get_move_max_pp(mon.moves[3]),
                ];
            }
            let p = &mut state.player;
            p.stat_stages = pokered_core::battle::stat_stages::StatStages::default();
            p.battle_status1 = 0;
            p.battle_status2 = 0;
            p.battle_status3 = 0;
            p.substitute_hp = 0;
            p.confused_turns_left = 0;
            p.toxic_counter = 0;
            p.disabled_move = 0;
            p.disabled_turns_left = 0;
        }
    }

    /// ReadTrainer's special-move pass — delegates to the CORE implementation
    /// (battle::special_moves, shared with the TUI).
    fn apply_trainer_special_moves(
        class: pokered_data::trainer_data::TrainerClass,
        party: &mut [pokered_core::battle::state::Pokemon],
    ) {
        pokered_core::battle::special_moves::apply_trainer_special_moves(class, party);
    }

    fn start_trainer_battle(&mut self, trainer_id: &str, rival_triplet_base: Option<u8>) {
        self.party_screen.set_cursor(0);
        use pokered_core::pokemon::stats::create_pokemon;

        self.prepare_gba_battle_allocation();
        let battle_rng = self.next_battle_rng();
        use pokered_data::species::Species;
        use pokered_data::trainer_data::{get_trainer_party_mons, parse_trainer_id, TrainerClass};

        let player_party = self.save_data.party.to_vec();

        let parsed = parse_trainer_id(trainer_id);
        let is_rival = parsed.as_ref().map_or(false, |(class, _)| {
            matches!(
                class,
                TrainerClass::Rival1 | TrainerClass::Rival2 | TrainerClass::Rival3
            )
        });

        let enemy_party = if let Some((class, default_index)) = parsed {
            // Rival gets type advantage: Grass→Fire, Fire→Water, Water→Grass.
            // Each triplet is ordered Squirtle/Bulbasaur/Charmander; the scene
            // supplies the triplet base (scripts/{Map}.asm StarterTable).
            let party_index = if is_rival {
                let base = rival_triplet_base.unwrap_or(0) as usize;
                let starter = pokered_data::trainer_data::resolve_player_starter(
                    self.save_data.game_data.player_starter,
                    player_party.first().map(|mon| mon.species),
                );
                let offset = starter
                    .map(pokered_data::trainer_data::rival_starter_offset)
                    .unwrap_or(0);
                if self.save_data.game_data.player_starter == 0 {
                    if let Some(starter) = starter {
                        self.save_data.game_data.player_starter = starter as u8;
                        self.save_data.game_data.rival_starter =
                            [Species::Squirtle, Species::Bulbasaur, Species::Charmander][offset]
                                as u8;
                    }
                }
                base + offset
            } else {
                default_index
            };

            if let Some(party) = get_trainer_party_mons(class, party_index) {
                let mut mons: Vec<_> = party
                    .iter()
                    .filter_map(|mon| {
                        create_pokemon(
                            mon.species,
                            mon.level,
                            pokered_core::pokemon::stats::TRAINER_DV_BYTES,
                        )
                    })
                    .collect();
                Self::apply_trainer_special_moves(class, &mut mons);
                mons
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        // A trainer's Pokémon are registered as seen in the Pokédex.
        for m in &enemy_party {
            self.save_data.game_data.pokedex.set_seen(m.species);
        }

        if !player_party.is_empty() && !enemy_party.is_empty() {
            let tc = parsed.map(|(c, _)| c);
            self.battle = pokered_core::battle::BattleScreen::from_parties(
                false,
                &player_party,
                &enemy_party,
                tc,
            );
            if is_rival {
                self.battle.trainer_name = Some(self.overworld.rival_name.clone());
            }
        } else {
            self.battle = pokered_core::battle::BattleScreen::new(false);
        }
        if let Some(rng) = battle_rng {
            self.battle.rng = rng;
        }
        self.battle.is_zh = self.state.config.language == pokered_core::game_state::Lang::Zh;
        self.battle.player_money = self.save_data.game_data.player_money;
        // Badge stat boosts + traded-mon obedience context (wObtainedBadges /
        // wPlayerID) — the battle reads them from these fields every turn.
        self.battle.player_badges = self.save_data.game_data.obtained_badges;
        self.battle.player_id = self.save_data.game_data.player_id;
        // BoxFullCannotThrowBall guard context (app-side snapshot).
        self.battle.player_box_full = self.save_data.current_box.is_full();
        self.battle.map_id = self.overworld.state.current_map as u8;
        // Copy of the bag so items are usable in-battle (synced back afterwards).
        self.battle.player_bag = self.save_data.game_data.bag.clone();
        self.battle_vfx = BattleVisualEffects::default();
        self.battle_prev_message = None;
        self.faint_thud_pending = false;

        if let Some(ref audio) = self.audio {
            if let Some(id) = MusicId::from_u8(self.battle.battle_music_id()) {
                audio.play_music(id);
            }
        }
    }

    pub fn update(&mut self, input: &InputState) {
        self.update_inner(input);
        // Per-frame recorders (`--record-frames` / `--record-video`):
        // capture AFTER the frame's logic ran, and do it here rather than
        // inside the update body so every frame lands — early returns,
        // real-time loop and synchronous step_frames bursts alike.
        #[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
        if let Some(mut rec) = self.frame_recorder.take() {
            rec.capture(self);
            self.frame_recorder = Some(rec);
        }
        #[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
        if let Some(mut rec) = self.video_recorder.take() {
            rec.capture(self);
            self.video_recorder = Some(rec);
        }
    }

    /// Poll and execute pending debug-server commands WITHOUT advancing a
    /// frame; returns how many ran. The normal update path calls this at
    /// the top of every frame; driven-only headless mode (`--speed 0`)
    /// calls it in a low-latency loop so game frames advance ONLY inside
    /// synchronous commands (step_frames/move_to/...) — driven runs become
    /// wall-clock-insensitive (exact frame-count determinism) and avoid
    /// the 16.7ms tick of command latency.
    #[cfg(feature = "debug-server")]
    pub fn poll_debug_commands(&mut self) -> usize {
        let commands = self
            .debug_handle
            .as_ref()
            .map(|h| h.poll_commands())
            .unwrap_or_default();
        let count = commands.len();
        for cmd in commands {
            let response = self.handle_debug_command(cmd);
            if let Some(ref handle) = self.debug_handle {
                handle.send_response(response);
            }
        }
        count
    }

    /// Debug work queued by earlier commands: pressed buttons waiting to
    /// be consumed one-per-frame, or RunFrames bursts. Driven-only mode
    /// drains these through normal updates (same one-per-frame semantics
    /// as StepFrames bursts).
    #[cfg(feature = "debug-server")]
    pub fn debug_work_pending(&self) -> bool {
        self.pending_debug_frames > 0 || !self.pending_debug_inputs.is_empty()
    }

    fn update_inner(&mut self, input: &InputState) {
        use pokered_core::game_state::Lang;
        self.frame_count += 1;
        if !matches!(self.state.screen, GameScreen::Overworld) {
            self.overworld.tick_boulder_presentation_during_ui();
            self.overworld.tick_player_presentation_during_ui();
            // Window transfers are suspended during DisplayTextIDInit's
            // font copy; other UI frames keep cycling the retained third.
            if (!matches!(self.state.screen, GameScreen::StartMenu) || self.start_menu.field_presentation_stage() >= 19)
                && self.overworld.field_text_restore.as_ref().is_none_or(|restore| restore.bg_transfer_enabled()) {
                self.overworld.tick_ui_background_transfer();
            }
        }
        if let Some(frame) = self.pc_stats_return_frame {
            self.pc_stats_return_frame = (frame < 8).then_some(frame + 1);
        }
        // Snapshot whether the overworld ran on the previous frame, before
        // this frame's state overwrites it (see the update_frame call site).
        let ow_gapped_last_frame = !self.ow_ran_last_frame;
        self.ow_ran_last_frame = false;

        #[cfg(feature = "debug-server")]
        self.poll_debug_commands();

        // Link play: accept a pending peer and drive the link session
        // (battle/trade state machines) every frame, before any early
        // returns so network progress never stalls.
        #[cfg(not(target_os = "none"))]
        self.poll_link();

        #[cfg(all(feature = "desktop", debug_assertions, not(target_arch = "wasm32"), not(target_os = "none")))]
        {
            let changes = self
                .asset_watcher
                .as_mut()
                .map(|w| w.poll_events())
                .unwrap_or_default();
            for change in &changes {
                dbg_eprintln!("[hot-reload] Changed: {}", change.path.display());

                if change.path.extension().and_then(|e| e.to_str()) == Some("scene") {
                    use std::fs;
                    if let Ok(source) = fs::read_to_string(&change.path) {
                        let map_key = change
                            .path
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("");
                        // Reload from the raw .scene source: the native engine
                        // recompiles it to an AST, the Boa path to JS — one
                        // seam for both engines.
                        match self.overworld.reload_scene_source(map_key, &source) {
                            Ok(()) => log::info!("[hot-reload] Recompiled .scene: {}", map_key),
                            Err(e) => dbg_eprintln!(
                                "[hot-reload] Failed to compile {}: {}",
                                change.path.display(),
                                e
                            ),
                        }
                    }
                }
            }
        }

        // Debug-driven input (debug-server): a queued Press/PressSequence button
        // is injected via the persistent `debug_input` so it reads as HELD
        // across consecutive frames (overriding window input); otherwise
        // RunFrames advances the game with empty input. Both fall through to
        // the normal update below so scripts/animations/battles actually
        // progress — the previous RunFrames early-return froze the game, so
        // injected interactions (e.g. talking to an NPC) never ran.
        let mut _modified_input;
        let input: &InputState = if !self.pending_debug_inputs.is_empty() {
            let button = self.pending_debug_inputs.remove(0);
            self.debug_input.begin_frame();
            self.debug_input.set_from_bitmask(0);
            if let Some(button) = button {
                self.debug_input.press(button);
            }
            _modified_input = self.debug_input.clone();
            &_modified_input
        } else if self.debug_input.raw_current() != 0 {
            // Queue just drained: emit one all-released frame so whatever the
            // debug script was holding (e.g. a d-pad walk) stops cleanly.
            self.debug_input.begin_frame();
            self.debug_input.set_from_bitmask(0);
            _modified_input = self.debug_input.clone();
            &_modified_input
        } else if self.pending_debug_frames > 0 {
            self.pending_debug_frames -= 1;
            _modified_input = InputState::new();
            &_modified_input
        } else {
            input
        };

        // Soft reset: hold A+B+Start+Select for 16 consecutive frames
        // (engine/joypad.asm `_Joypad`: hJoyInput == PAD_BUTTONS → TrySoftReset
        // decrements hSoftReset from 16 while the combo stays held).
        if input.is_held(GbButton::A)
            && input.is_held(GbButton::B)
            && input.is_held(GbButton::Start)
            && input.is_held(GbButton::Select)
        {
            self.soft_reset_frames = self.soft_reset_frames.saturating_add(1);
            if self.soft_reset_frames >= SOFT_RESET_HOLD_FRAMES {
                self.soft_reset_frames = 0;
                self.soft_reset();
                return;
            }
        } else {
            self.soft_reset_frames = 0;
        }

        // Tick play time every frame once the game proper has started
        // (TrackPlayTime from VBlank — see game_timer_active).
        if self.game_timer_active() {
            self.save_data.game_data.play_time.tick();
        }

        if let Some(ref audio) = self.audio {
            audio.update_frame();
        }

        if self.black_screen_frames > 0 {
            self.black_screen_frames -= 1;
            if self.black_screen_frames == 0 {
                #[cfg(not(target_os = "none"))]
                if let Some(screen) = self.pending_screen.take() {
                    self.handle_transition(screen);
                }
            }
            return;
        }

        // DisplayPartyMenu precedes ConnectCableText, then the animation.
        // Keep the scene suspended until the selected mon has been exchanged.
        if self.pending_trade.as_ref().is_some_and(|trade| trade.ready_to_animate)
            && self.overworld.pending_dialogue.is_none() {
            let trade=self.pending_trade.as_mut().unwrap();
            trade.ready_to_animate=false;
            self.trade_anim=Some(pokered_core::trade::TradeAnim::new(trade.give,trade.receive,
                self.player_name.clone(),self.state.config.language==pokered_core::game_state::Lang::Zh));
        }

        // In-game NPC trade cutscene (engine/movie/trade.asm): takes over the
        // frame while active. The party mutation is applied only when the
        // animation completes (original order: InternalClockTradeAnim →
        // RemovePokemon/AddPartyMon), then the suspended script resumes.
        if self.trade_anim.is_some() {
            let done = {
                let anim = self.trade_anim.as_mut().unwrap();
                let (give, receive) = (anim.give, anim.receive);
                let done = anim.tick();
                for sfx in anim.pending_sfx.drain(..) {
                    if let Some(ref audio) = self.audio {
                        use pokered_core::trade::TradeSfx;
                        match sfx {
                            TradeSfx::CableConnect => audio.play_sfx(SfxId::HealHP),
                            TradeSfx::BallTravel => audio.play_sfx(SfxId::Tink),
                            TradeSfx::GiveMonCry => play_species_cry(audio, give),
                            TradeSfx::ReceiveMonCry => play_species_cry(audio, receive),
                        }
                    }
                }
                done
            };
            if done {
                self.trade_anim = None;
                let mut npc_ok = true;
                if let Some(trade) = self.pending_trade.take() {
                    npc_ok = self.apply_npc_trade(trade);
                }
                #[cfg(not(target_os = "none"))]
                if self.link_cable.phase() == &CableClubPhase::TradeAnim {
                    // Link trade: the driver applies the exchange
                    // (remove-then-add, traded flag, Pokédex, forced trade
                    // evolution), then the flow shows "Trade completed!" and
                    // returns to the selection screen.
                    self.apply_link_trade();
                }
                self.overworld.resume_script_after_trade(npc_ok);
            }
            return;
        }

        // Evolution cutscene (engine/movie/evolution.asm +
        // engine/pokemon/evos_moves.asm): takes over the frame while active.
        // Each evolution's mutation is applied only when its morph resolves —
        // a B-cancel applies nothing (CancelledEvolution, evos_moves.asm:293).
        if self.evolution_anim.is_some() {
            let done = {
                let anim = self.evolution_anim.as_mut().unwrap();
                let evo_input = pokered_core::evolution_screen::EvolutionInput {
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                };
                let done = anim.tick_with_sound(
                    evo_input,
                    self.audio
                        .as_ref()
                        .is_some_and(|audio| audio.is_sfx_playing()),
                );
                for sfx in anim.pending_sfx.drain(..) {
                    if let Some(ref audio) = self.audio {
                        use pokered_core::evolution_screen::EvolutionSfx;
                        match sfx {
                            EvolutionSfx::StopMusic => audio.stop_all(),
                            EvolutionSfx::Tink => audio.play_sfx(SfxId::Tink),
                            // MUSIC_SAFARI_ZONE is the original's morph music
                            // (evolution.asm:44-46).
                            EvolutionSfx::MorphMusic => audio.play_music(MusicId::SAFARI_ZONE),
                            EvolutionSfx::GetItem2 => audio.play_sfx(SfxId::GetItem2),
                            EvolutionSfx::Cry(species) => play_species_cry(audio, species),
                        }
                    }
                }
                done
            };
            // Apply resolved evolutions (success: species swap + dex; cancel:
            // nothing — the mon retries on its next level-up).
            let mut outcomes = Vec::new();
            while let Some(outcome) = self.evolution_anim.as_mut().unwrap().take_outcome() {
                outcomes.push(outcome);
            }
            for outcome in outcomes {
                self.apply_evolution_outcome(&outcome);
            }
            if done {
                self.evolution_anim = None;
                // PlayDefaultMusic (evos_moves.asm:257-259): restart the map
                // theme after the cutscene.
                if let Some(ref audio) = self.audio {
                    let map = self.overworld.state.current_map;
                    let data_id = pokered_core::overworld::map_loading::get_map_music(map);
                    if let Some(id) = MusicId::from_u8(data_id as u8) {
                        audio.play_music(id);
                    }
                }
                self.overworld.party_count = self.save_data.party.count() as u8;
                self.overworld.box_count = self.save_data.current_box.count() as u8;
                self.overworld.gift_box_number = self.save_data.pc_storage.current_box_index() as u8 + 1;
                self.overworld.party_lead_level = self.save_data.party.leader_level();
                // A full-moveset level-up move couldn't be learned: open the
                // party screen's forget-a-move prompt, exactly where the
                // original's `predef LearnMove` would (evos_moves.asm:212).
                if self.pending_evolve_move_replace.is_some() {
                    self.handle_transition(GameScreen::PartyScreen);
                    return;
                }
            }
            return;
        }

        // Hall of Fame roll-call (engine/movie/hall_of_fame.asm): endgame
        // takeover after the Champion, started by `game.enterHallOfFame()`
        // (the team was already recorded when the pending flag was drained).
        if self.hof_ceremony.is_some() {
            let done = {
                let hof = self.hof_ceremony.as_mut().unwrap();
                // HoFDisplayMonInfo's PlayCry is blocking before the 80-frame
                // information dwell. Keep the current pic while its cry plays.
                let crying = hof.phase() == pokered_core::hof_ceremony::HofPhase::MonInfo
                    && self
                        .audio
                        .as_ref()
                        .is_some_and(|audio| audio.is_sfx_playing());
                let done = if crying { false } else { hof.update_frame()};
                for sfx in hof.take_sfx() {
                    if let Some(ref audio) = self.audio {
                        let pokered_core::hof_ceremony::HofSfx::Cry(species) = sfx;
                        play_species_cry(audio, species);
                    }
                }
                // MUSIC_HALL_OF_FAME (hall_of_fame.asm:73-76).
                if hof.take_music_pending() {
                    if let Some(ref audio) = self.audio {
                        audio.play_music(MusicId::HALL_OF_FAME);
                    }
                }
                // HoFFadeOutScreenAndMusic (hall_of_fame.asm:284-288).
                if hof.take_music_fade_pending() {
                    if let Some(ref audio) = self.audio {
                        audio.fade_out(10);
                    }
                }
                done
            };
            if done {
                self.hof_ceremony = None;
                self.credits = Some(pokered_core::credits::CreditsState::new_with_opening(
                    self.state.config.version,
                ));
                }
            return;
        }

        // End credits (engine/movie/credits.asm): runs to "THE END", then a
        // button press saves + resets to the title screen.
        if self.credits.is_some() {
            let done = {
                let roll = self.credits.as_mut().unwrap();
                let done = roll.update_frame(pokered_core::credits::CreditsInput {
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                })
            ;
                if roll.take_music_pending() {
                    if let Some(ref audio) = self.audio {
                        audio.stop_music();
                        audio.play_music(MusicId::CREDITS);
                    }
                }
                done
            };
            if done {
                self.credits = None;
                self.finish_hof_ceremony();
            }
            return;
        }

        let action = match self.state.screen {
            GameScreen::GameFreakSplash => {
                // PlayShootingStar (engine/movie/intro.asm:305-341): the
                // shooting-star splash runs before everything else; input
                // skips it like the original's CheckForUserInterruption.
                let splash_input = SplashInput {
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_held(GbButton::B),
                    start: input.is_just_pressed(GbButton::Start),
                    select: input.is_held(GbButton::Select),
                    up: input.is_held(GbButton::Up),
                };
                let action = self.gamefreak_splash.update_frame(splash_input);
                // SFX_SHOOTING_STAR at the start of AnimateShootingStar
                // (engine/movie/splash.asm:29-30).
                if self.gamefreak_splash.take_sfx_pending() {
                    if let Some(ref audio) = self.audio {
                        audio.play_sfx(SfxId::ShootingStar);
                    }
                }
                action
            }
            GameScreen::CopyrightSplash => {
                let any_pressed = input.any_just_pressed();
                let action = self.title_screen.update_frame(any_pressed);
                if self.title_screen.phase == TitlePhase::Init {
                    ScreenAction::Transition(GameScreen::LanguageSelect)
                } else {
                    action
                }
            }
            GameScreen::LanguageSelect => {
                use pokered_core::game_state::Lang;
                let action = if input.is_just_pressed(GbButton::Up)
                    || input.is_just_pressed(GbButton::Down)
                {
                    self.state.config.language = match self.state.config.language {
                        Lang::En => Lang::Zh,
                        Lang::Zh => Lang::En,
                    };
                    ScreenAction::Continue
                } else if input.is_just_pressed(GbButton::A)
                    || input.is_just_pressed(GbButton::Start)
                {
                    ScreenAction::Transition(GameScreen::IntroScene)
                } else {
                    ScreenAction::Continue
                };
                // Keep the overworld script engine in sync with the chosen
                // language so NPC dialogue (`@t` literals) renders in it, and
                // localize battle messages the same way.
                self.overworld
                    .set_script_lang(if self.state.config.language == Lang::Zh {
                        "zh"
                    } else {
                        "en"
                    });
                self.battle.is_zh = self.state.config.language == Lang::Zh;
                action
            }
            GameScreen::IntroScene => {
                let any_pressed = input.any_just_pressed();
                let action = self.intro_scene.update_frame(any_pressed);
                if let Some(ref audio) = self.audio {
                    match self.intro_scene.sfx_event {
                        IntroSfxEvent::IntroHip => audio.play_sfx(SfxId::IntroHip),
                        IntroSfxEvent::IntroHop => audio.play_sfx(SfxId::IntroHop),
                        IntroSfxEvent::IntroRaise => audio.play_sfx(SfxId::IntroRaise),
                        IntroSfxEvent::IntroCrash => audio.play_sfx(SfxId::IntroCrash),
                        IntroSfxEvent::IntroLunge => audio.play_sfx(SfxId::IntroLunge),
                        IntroSfxEvent::None => {}
                    }
                }
                action
            }
            GameScreen::TitleScreen => {
                let prev_phase = self.title_screen.phase;
                let any_pressed = input.any_just_pressed();
                let action = if let Some(ref audio) = self.audio {
                    self.title_screen.update_frame_with_sound(any_pressed, audio.is_sfx_playing())
                } else {
                    self.title_screen.update_frame(any_pressed)};
                let new_phase = self.title_screen.phase;

                // Original crash: start of the -3 rebound, after 16 down and
                // four up frames; whoosh: after the full 36-frame pause.
                if new_phase == TitlePhase::LogoBounce && self.title_screen.frame_counter == 20 {
                    if let Some(ref audio) = self.audio {
                        audio.play_sfx(SfxId::IntroCrash);
                    }
                }

                if prev_phase != new_phase {
                    if let Some(ref audio) = self.audio {
                        match new_phase {
                            TitlePhase::VersionScroll => {
                                audio.play_sfx(SfxId::IntroWhoosh);
                            }
                            TitlePhase::WaitingForInput
                                if prev_phase == TitlePhase::VersionWait =>
                            {
                                audio.play_music(MusicId::TITLE_SCREEN);
                            }
                            TitlePhase::PlayingCry => {
                                play_species_cry(audio, self.title_screen.current_mon);
                            }
                            _ => {}
                        }
                    }
                }
                action
            }
            GameScreen::MainMenu => {
                let menu_input = MenuInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    a: input.is_just_pressed(GbButton::A) || input.is_just_pressed(GbButton::Start),
                    b: input.is_just_pressed(GbButton::B),
                };
                if input.is_just_pressed(GbButton::A) || input.is_just_pressed(GbButton::Start) {
                    if let Some(ref audio) = self.audio {
                        audio.play_sfx(SfxId::PressAB);
                    }
                }
                self.main_menu.update_frame(menu_input)
            }
            GameScreen::OakSpeech => {
                let prev_tag = self.prev_oak_phase_tag;
                self.oak_speech.language = self.state.config.language;
                let result = if self.oak_speech.is_naming_active() {
                    let naming_input = NamingInput {
                        up: input.is_just_pressed(GbButton::Up),
                        down: input.is_just_pressed(GbButton::Down),
                        left: input.is_just_pressed(GbButton::Left),
                        right: input.is_just_pressed(GbButton::Right),
                        a: input.is_just_pressed(GbButton::A),
                        b: input.is_just_pressed(GbButton::B),
                        start: input.is_just_pressed(GbButton::Start),
                        select: input.is_just_pressed(GbButton::Select),
                    };
                    self.oak_speech
                        .update_naming_frame(naming_input, self.state.config.language == Lang::Zh)
                } else {
                    let oak_input = OakSpeechInput {
                        up: input.is_just_pressed(GbButton::Up),
                        down: input.is_just_pressed(GbButton::Down),
                        a: input.is_just_pressed(GbButton::A),
                        b: input.is_just_pressed(GbButton::B),
                    };
                    self.oak_speech.update_frame(oak_input)
                };
                let new_tag = oak_phase_tag(&self.oak_speech.phase);

                if prev_tag != new_tag {
                    if let Some(ref audio) = self.audio {
                        match &self.oak_speech.phase {
                            OakSpeechPhase::ShowNidorino { .. } if prev_tag != new_tag => {
                                play_species_cry(audio, pokered_data::species::Species::Nidorino);
                            }
                            OakSpeechPhase::ShrinkPlayer { .. } => {
                                audio.play_sfx(SfxId::Shrink);
                            }
                            _ => {}
                        }
                    }
                    self.prev_oak_phase_tag = new_tag;
                }

                if (input.is_just_pressed(GbButton::A) || input.is_just_pressed(GbButton::B))
                    && !matches!(
                        &self.oak_speech.phase,
                        OakSpeechPhase::ShrinkPlayer { .. }
                            | OakSpeechPhase::SlidePic { .. }
                            | OakSpeechPhase::Done
                    )
                    && !self.oak_speech.is_flashing()
                {
                    if let Some(ref audio) = self.audio {
                        audio.play_sfx(SfxId::PressAB);
                    }
                }

                match result {
                    OakSpeechResult::PlayerNameSet(name) => {
                        self.player_name = name;
                        ScreenAction::Continue
                    }
                    OakSpeechResult::RivalNameSet(name) => {
                        self.rival_name = name;
                        ScreenAction::Continue
                    }
                    OakSpeechResult::Finished => {
                        // GenRandomTrainerID (oak_speech65.asm): a NEW GAME
                        // rolls the player's trainer ID — the save-overwrite
                        // "different player?" check compares against it.
                        if self.save_data.game_data.player_id == 0 {
                            // Seeded overworld stream when determinism is
                            // pinned (agent M5); entropy otherwise.
                            let dvs = [
                                self.overworld.next_rng_u8(),
                                self.overworld.next_rng_u8(),
                            ];
                            self.save_data.game_data.player_id =
                                dvs[0] as u16 | ((dvs[1] as u16) << 8);
                        }
                        ScreenAction::Transition(GameScreen::Overworld)
                    }
                    OakSpeechResult::Active => ScreenAction::Continue,
                }
            }
            GameScreen::Overworld => {
                // ── Link presence (Cable Club) ──────────────────────────
                // While a link session is connected and the player is inside
                // Colosseum/TradeCenter, the room's opponent NPC is pinned to
                // the remote player's spot (the original's TradeCenter_Script
                // placement); otherwise the map keeps its placeholder NPC.
                #[cfg(not(target_os = "none"))]
                let link_action: Option<ScreenAction> = {
                    let current_map = self.overworld.state.current_map;
                    let linked = matches!(self.link_status, LinkStatus::Connected)
                        && self.link_session.is_some();
                    let in_cable_room = crate::link::cable_club::is_cable_room(current_map);
                    self.overworld.link_opponent = if linked && in_cable_room {
                        Some(pokered_core::link::LinkOpponentPresence::for_role(
                            self.link_role,
                        ))
                    } else {
                        None
                    };
                    self.link_cable.note_presence(linked, in_cable_room);
                    if self.link_cable.phase()==&CableClubPhase::ReceptionText && self.overworld.pending_dialogue.is_none() {
                        self.link_cable.on_reception_text_done();
                    }

                    // The gameboy on the table: the map scene calls
                    // `game.linkStart()`; the flow starts the room's request
                    // (LINK BATTLE in the Colosseum, LINK TRADE in the Trade
                    // Center — the original's CableClubLeftGameboy/
                    // CableClubRightGameboy, engine/pokemon/bills_pc.asm).
                    let mut gameboy_started_this_frame = false;
                    if self.overworld.take_link_start_request() {
                        if !in_cable_room && !self.overworld.unified_flags().get_flag("EVENT_GOT_POKEDEX") {
                            self.overworld.pending_dialogue = Some(pokered_core::overworld::BedroomDialogue::from_message(
                                &self.localize_dialogue("We're making\npreparations.\nPlease wait.")));
                        } else if linked {
                            if in_cable_room {
                                let player = &self.overworld.state.player;
                                if pokered_core::link::can_use_gameboy(self.link_role, player.x, player.y, player.facing) {
                                    gameboy_started_this_frame = true;
                                    let need = self.link_cable.on_gameboy_used(current_map);
                                    self.handle_flow_need(need);
                                }
                            } else {
                                self.link_cable.on_receptionist_used();
                                self.overworld.pending_dialogue=Some(pokered_core::overworld::BedroomDialogue::from_message(
                                    if self.state.config.language==pokered_core::game_state::Lang::Zh { "请在这里申请。\n\n开启联机前，\n必须保存游戏。" }
                                    else { "Please apply here.\n\nBefore opening\nthe link, we have\nto save the game." }));
                            }
                        } else {
                            // Offline (no link session): the original's
                            // "Just a moment." (JustAMomentText) and nothing
                            // happens — the room has no cable partner.
                            self.overworld.pending_dialogue =
                                Some(pokered_core::overworld::BedroomDialogue::from_message(
                                    &self.localize_dialogue(
                                        if in_cable_room { crate::link::cable_club::TEXT_JUST_A_MOMENT } else { "This area is\nreserved for 2\nfriends who are\nlinked by cable." },
                                    ),
                                ));
                        }
                    }

                    // Trade cutscene start: both sides confirmed, the wire mon
                    // arrived (TradeExecute) — play the exchange animation. The
                    // exchange data lives in the trade driver (`received_mon`).
                    if self.link_cable.phase() == &CableClubPhase::TradeAnim
                        && self.trade_anim.is_none()
                    {
                        if self
                            .link_trade
                            .as_ref()
                            .is_some_and(|d| d.has_pending_exchange())
                        {
                            self.start_link_trade_anim();
                        } else if self.pending_evolve_move_replace.is_none() {
                            self.finish_link_trade();
                            return;
                        }
                    }

                    // Modal link screens (prompts, party select, wait boxes):
                    // the game freezes like the original's link screens and the
                    // input goes to the flow. `BattleSetup` (both parties
                    // exchanged) builds the battle and transitions into it.
                    if self.link_cable.is_modal()
                        || self.link_cable.phase() == &CableClubPhase::BattleSetup
                    {
                        if self.link_cable.is_modal() {
                            // StatusScreen calls the blocking PlayCry before waiting
                            // for footer input. Keep the logical audio clock in
                            // this path even when sound output is muted.
                            let cry_holds_input = self.link_cable.stats().is_some()
                                && self.audio.as_ref().is_some_and(|audio| audio.is_sfx_playing());
                            let psi = PartyScreenInput {
                                up: input.is_just_pressed(GbButton::Up),
                                down: input.is_just_pressed(GbButton::Down),
                                a: !gameboy_started_this_frame && !cry_holds_input && input.is_just_pressed(GbButton::A),
                                b: !gameboy_started_this_frame && !cry_holds_input && input.is_just_pressed(GbButton::B),
                            };
                            if self.link_cable.menu_button_sound(
                                psi,
                                input.is_just_pressed(GbButton::Left),
                                input.is_just_pressed(GbButton::Right),
                            ) {
                                if let Some(audio) = self.audio.as_ref() {
                                    audio.play_sfx(SfxId::PressAB);
                                }
                            }
                            let was_viewing_stats = self.link_cable.stats().is_some();
                            let need = self.link_cable.update_with_horizontal_navigation(
                                psi,
                                &self.save_data.party.to_vec(),
                                input.is_just_pressed(GbButton::Left),
                                input.is_just_pressed(GbButton::Right),
                            );
                            if let Some(species) = self.link_cable.take_stats_entry_cry() {
                                if let Some(audio) = self.audio.as_ref() { play_species_cry(audio, species); }
                            }
                            if let Some(audio) = self.audio.as_ref() {
                                if let Some(stats) = self.link_cable.stats() {
                                    let volume = if !stats.entry_blocks_input() && audio.is_sfx_playing()
                                        || stats.entry_frame() == Some(stats.entry_cry_frame()) { 7 } else { 3 };
                                    audio.set_master_volume(volume, volume);
                                } else if was_viewing_stats {
                                    audio.set_master_volume(7, 7);
                                }
                            }
                            self.handle_flow_need(need);
                        }
                        if self.link_cable.phase() == &CableClubPhase::BattleSetup {
                            self.start_link_battle();
                            Some(ScreenAction::Transition(GameScreen::Battle))
                        } else {
                            Some(ScreenAction::Continue)
                        }
                    } else {
                        None
                    }
                };
                #[cfg(target_os = "none")]
                let link_action: Option<ScreenAction> = None;
                if let Some(action) = link_action {
                    action
                } else if self.overworld.is_party_select_active() {
                    let psi = PartyScreenInput {
                        up: input.is_just_pressed(GbButton::Up),
                        down: input.is_just_pressed(GbButton::Down),
                        a: input.is_just_pressed(GbButton::A),
                        b: input.is_just_pressed(GbButton::B),
                    };
                    if self.pending_trade.is_some() {
                        use pokered_core::party_select::PartySelectResult;
                        let result = self.overworld.pending_party_select.as_mut().unwrap().update_frame(psi);
                        match result {
                            PartySelectResult::Active => {}
                            PartySelectResult::Cancelled => {
                                self.overworld.pending_party_select = None;
                                self.pending_trade = None;
                                self.overworld.set_flag_live("NPC_TRADE_CANCELLED", true);
                                self.overworld.resume_script_after_trade(false);
                            }
                            PartySelectResult::Selected(idx) => {
                                self.overworld.pending_party_select = None;
                                let trade = self.pending_trade.as_mut().unwrap();
                                if self.save_data.party.get(idx).is_some_and(|m| m.species == trade.give) {
                                    trade.party_index = idx;
                                    trade.ready_to_animate = true;
                                    // InGameTrade_DoTrade sets the completed bit after
                                    // a valid pick, before ConnectCableText/animation.
                                    self.overworld.mark_npc_trade_completed(&trade.nickname);
                                    self.overworld.pending_dialogue=Some(pokered_core::overworld::BedroomDialogue::from_message(
                                        if self.state.config.language==pokered_core::game_state::Lang::Zh {
                                            "好，请把通信线接上！"
                                        } else { "Okay, connect the\ncable like so!" }));
                                } else {
                                    self.pending_trade = None;
                                    self.overworld.resume_script_after_trade(false);
                                }
                            }
                        }
                    } else {
                        self.overworld.update_party_select_input(psi);
                    }
                    ScreenAction::Continue
                } else if self.overworld.is_naming_screen_active() {
                    let naming_input = NamingInput {
                        up: input.is_just_pressed(GbButton::Up),
                        down: input.is_just_pressed(GbButton::Down),
                        left: input.is_just_pressed(GbButton::Left),
                        right: input.is_just_pressed(GbButton::Right),
                        a: input.is_just_pressed(GbButton::A),
                        b: input.is_just_pressed(GbButton::B),
                        start: input.is_just_pressed(GbButton::Start),
                        select: input.is_just_pressed(GbButton::Select),
                    };
                    self.overworld
                        .update_naming_input(naming_input, self.state.config.language == Lang::Zh);
                    ScreenAction::Continue
                } else {
                    // While the A+B+Start+Select soft-reset combo is held, the
                    // START press is consumed by TrySoftReset (engine/joypad.asm
                    // `_Joypad` routes PAD_BUTTONS to TrySoftReset before the
                    // menu handlers) — the start menu must not open or a held
                    // combo would never reach the 16-frame reset.
                    let soft_reset_combo_held = input.is_held(GbButton::A)
                        && input.is_held(GbButton::B)
                        && input.is_held(GbButton::Start)
                        && input.is_held(GbButton::Select);
                    if ow_gapped_last_frame {
                        let previously_held = |b| input.is_just_released(b)
                            || (input.is_held(b) && !input.is_just_pressed(b));
                        self.overworld.synchronize_player_input(OverworldInput::new(
                            previously_held(GbButton::Up), previously_held(GbButton::Down),
                            previously_held(GbButton::Left), previously_held(GbButton::Right),
                            previously_held(GbButton::A), previously_held(GbButton::B),
                            previously_held(GbButton::Start), previously_held(GbButton::Select),
                        ));
                    }
                    let ow_input = OverworldInput::new(
                        input.is_held(GbButton::Up),
                        input.is_held(GbButton::Down),
                        input.is_held(GbButton::Left),
                        input.is_held(GbButton::Right),
                        input.is_held(GbButton::A),
                        input.is_held(GbButton::B),
                        input.is_held(GbButton::Start) && !soft_reset_combo_held,
                        input.is_held(GbButton::Select),
                    );
                    // Seed synchronous script-query state from persistent game
                    // data BEFORE update_frame so `@if` conditions (hasItem,
                    // getMoney, dex, rival starter, facing) read current
                    // values. The seed rebuild allocates (hash-map sets,
                    // `Vec<String>` clones) so it runs only when the
                    // fingerprinted inputs actually change — on a static
                    // overworld frame this used to be the single largest
                    // per-frame cost on GBA.
                    let seed_snapshot = self.query_seed_snapshot();
                    if self.overworld.script_queries_need_seed() || seed_snapshot != self.query_seed {
                        self.query_seed = seed_snapshot;
                        let bag_names: Vec<String> = self
                            .save_data
                            .game_data
                            .bag
                            .items()
                            .iter()
                            .map(|(id, _)| id.const_name())
                            .collect();
                        let party_species: Vec<String> = self
                            .save_data
                            .party
                            .species_list()
                            .iter()
                            .map(|s| s.pascal_name())
                            .collect();
                        self.overworld.seed_script_bag_quantities(&self.save_data.game_data.bag);
                        self.overworld.gift_box_number = self.save_data.pc_storage.current_box_index() as u8 + 1;
                        self.overworld.seed_script_query_state(
                            self.save_data.game_data.player_money,
                            &bag_names,
                            self.save_data.game_data.pokedex.owned_count() as u8,
                            self.save_data.game_data.pokedex.seen_count() as u8,
                            self.save_data.game_data.rival_starter,
                            self.save_data.game_data.player_starter,
                            &party_species,
                            self.save_data.game_data.player_coins,
                            self.save_data.game_data.obtained_badges,
                            match self.state.config.version {
                                GameVersion::Red => 0,
                                GameVersion::Blue => 1,
                            },
                        );
                        // Day Care + per-party query state (for the Day Care
                        // scene), refreshed with the same change gating.
                        {
                            use pokered_core::battle::experience::growth::level_from_exp;
                            use pokered_core::pokemon::move_learning::is_hm_move;
                            use pokered_data::pokemon_data::get_base_stats;
                            use pokered_data::species::Species;
                            let dc = &self.save_data.game_data.daycare;
                            let (levels_grown, cost) = if dc.in_use {
                                let species = Species::from_index_id(dc.species);
                                let new_level = get_base_stats(species)
                                    .map(|b| level_from_exp(b.growth_rate, dc.exp).min(100))
                                    .unwrap_or(dc.box_level);
                                let grown = new_level.saturating_sub(dc.box_level);
                                (grown, 100u32 * (grown as u32 + 1))
                            } else {
                                (0, 0)
                            };
                            self.overworld.seed_daycare_species(&Species::from_index_id(dc.species).pascal_name());
                            let dc_name = pokered_data::charmap::decode_string(
                                &self.save_data.game_data.daycare_mon_name,
                            );
                            let mut name_buf =
                                [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
                            let party_names: Vec<String> = self
                                .save_data
                                .party
                                .iter()
                                .map(|m| m.display_name(&mut name_buf).to_string())
                                .collect();
                            let party_knows_hm: Vec<bool> = self
                                .save_data
                                .party
                                .iter()
                                .map(|m| m.moves.iter().any(|mv| is_hm_move(*mv)))
                                .collect();
                            let can_rename: Vec<bool> = (0..self.save_data.party.count())
                                .map(|i| self.save_data.party_mon_can_rename(i))
                                .collect();
                            self.overworld.seed_party_rename_query_state(&can_rename);
                            self.overworld.seed_daycare_query_state(
                                dc.in_use,
                                &dc_name,
                                levels_grown,
                                cost,
                                &party_names,
                                &party_knows_hm,
                            );
                        }
                    } else {
                        // Seeding already mixes once. Consume exactly one
                        // RNG draw per tick, independent of cache history.
                        self.overworld.mix_script_rng();
                    }

                    self.overworld.script_sfx_playing = self.audio.as_ref()
                        .is_some_and(|audio| audio.is_sfx_playing() && !audio.low_health_alarm_active());
                    // wOptions text delay — pushed every frame so the dialogue
                    // typewriter honors the configured TEXT SPEED.
                    self.overworld
                        .set_text_delay_frames(self.state.config.text_speed.delay_frames());
                    // First overworld frame after a gap (sub-screen, naming
                    // screen, cutscene…): `update_frame` didn't run in between,
                    // so its per-button edge detectors are stale. Re-baseline
                    // them — the press that drove whatever ran in between is
                    // still held and must not re-fire here (e.g. A on
                    // START-menu EXIT instantly talking to a facing NPC).
                    if ow_gapped_last_frame {
                        self.sync_overworld_input_edges(input);
                    }
                    self.ow_ran_last_frame = true;
                    // A black warp frame no longer needs the decoded assets
                    // from the source map. Release them before core commits
                    // the destination map and deserializes its scene AST;
                    // retaining both sets can exhaust GBA EWRAM (notably the
                    // Pallet Town Oak escort into Oak's Lab).
                    #[cfg(target_os = "none")]
                    if matches!(
                        self.overworld.warp_fade_state,
                        pokered_core::overworld::WarpFadeState::BlackScreen
                    ) {
                        if let Some(resources) = self.resources.as_mut() {
                            resources.clear_cache();
                        }
                    }
                    // SSAnneCaptainsRoom.asm waits on music channel 1.
                    self.overworld.script_music_playing = self.audio.as_ref()
                        .is_some_and(|audio| audio.is_music_channel_playing(0));
                    // Original DisplayPokedex records the species as seen after viewing.
                    let viewed_species = self.overworld.pending_pokedex_entry.as_ref()
                        .and_then(|entry| pokered_data::species::Species::from_scene_name(&entry.species));
                    let action = self.overworld.update_frame(ow_input);
                    if self.overworld.pending_pokedex_entry.is_none() {
                        if let Some(species) = viewed_species {
                            self.save_data.game_data.pokedex.set_seen(species);
                        }
                    }

                    self.apply_overworld_game_data_requests();
                    // The money box follows the committed balance, including
                    // mutations queued just before the next script query.
                    if self.overworld.script_money_box.is_some() {
                        self.overworld.script_money_box = Some(self.save_data.game_data.player_money);
                    }

                    if let Some(ref audio) = self.audio {
                        match self.overworld.sfx_event {
                            OverworldSfxEvent::GoInside => audio.play_sfx(SfxId::GoInside),
                            OverworldSfxEvent::GoOutside => audio.play_sfx(SfxId::GoOutside),
                            OverworldSfxEvent::Collision => audio.play_sfx(SfxId::Collision),
                            OverworldSfxEvent::Ledge => audio.play_sfx(SfxId::Ledge),
                            OverworldSfxEvent::ArrowTiles => audio.play_sfx(SfxId::ArrowTiles),
                            OverworldSfxEvent::TextAdvance => audio.play_sfx(SfxId::PressAB),
                            OverworldSfxEvent::None => {}
                        }
                        for req in self.overworld.audio_requests.drain(..) {
                            match req {
                                OverworldAudioRequest::PlayMusic { music_id } => {
                                    // Alternate tempo/start variants
                                    // (audio/alternate_tempo.asm) are dispatched
                                    // by name; ordinary IDs map to MusicId.
                                    if audio.play_script_music(&music_id) {
                                    } else if let Some(id) = script_string_to_music_id(&music_id) {
                                        if audio.last_music_id() != Some(id) {
                                            audio.clear_saved_music_states();
                                            audio.play_music(id);
                                        }
                                    }
                                }
                                OverworldAudioRequest::PlaySound { sound_id } => {
                                    if sound_id == "SFX_STOP_ALL_MUSIC" {
                                        audio.stop_all();
                                    } else if sound_id == "SFX_BADGE_BANK_QUIRK" {
                                        audio.play_badge_bank_quirk();
                                    } else if let Some(sfx) = parse_sfx_id(&sound_id) {
                                        audio.play_sfx(sfx);
                                    } else {
                                        log::warn!("Unknown SFX: {}", sound_id);
                                    }
                                }
                                OverworldAudioRequest::StopMusic => {
                                    audio.stop_music();
                                }
                                OverworldAudioRequest::FadeOutMusic => {
                                    // Original uses fade_speed=4 for healing machine
                                    audio.fade_out(4);
                                }
                                OverworldAudioRequest::PlayMapMusic { map } => {
                                    use pokered_core::overworld::TransportMode;
                                    let data_id = match self.overworld.state.player.transport {
                                        TransportMode::Biking => 32,  // MUSIC_BIKE_RIDING
                                        TransportMode::Surfing => 33, // MUSIC_SURFING
                                        TransportMode::Walking => {
                                            pokered_core::overworld::map_loading::get_map_music(map)
                                                as u8
                                        }
                                    };
                                    if let Some(id) = MusicId::from_u8(data_id) {
                                        if audio.last_music_id() != Some(id) {
                                            audio.play_music_with_fade(id, 10);
                                        }
                                    }
                                }
                                OverworldAudioRequest::PlayCry { species } => {
                                    if let Some(sp) =
                                        pokered_data::species::Species::from_scene_name(&species)
                                    {
                                        play_species_cry(audio, sp);
                                    } else {
                                        log::warn!("Unknown cry species: {}", species);
                                    }
                                }
                                OverworldAudioRequest::PlayPokeFlute { map } => {
                                    use pokered_core::overworld::TransportMode;
                                    let data_id = match self.overworld.state.player.transport {
                                        TransportMode::Biking => 32,
                                        TransportMode::Surfing => 33,
                                        TransportMode::Walking => {
                                            pokered_core::overworld::map_loading::get_map_music(map)
                                                as u8
                                        }
                                    };
                                    if let Some(id) = MusicId::from_u8(data_id) {
                                        audio.play_flute_overworld(id);
                                    }
                                }
                            }
                        }
                    }

                    if self.overworld.heal_requested {
                        self.overworld.heal_requested = false;
                        self.save_data.party.heal_all();
                        if let Some(ref audio) = self.audio {
                            audio.play_sfx(SfxId::HealingMachine);
                        }
                    }

                    // A script (e.g. the Name Rater) asked to open the party
                    // selector — hand it the current party (the overworld does
                    // not own it).
                    if self.overworld.take_party_select_request() {
                        self.overworld
                            .begin_party_select(self.save_data.party.to_vec());
                    }

                    // Apply a script-requested nickname change to the party.
                    if let Some((idx, name)) = self.overworld.pending_set_nickname.take() {
                        let _ = self.save_data.party.set_nickname(idx as usize, &name);
                    }

                    if let Some(pending) = self.overworld.pending_give_pokemon.take() {
                        // Gifted mons get random DVs (AddPartyMon Random ×2,
                        // add_mon.asm:95-101).
                        if let Some(mut pokemon) = pokered_core::pokemon::stats::create_pokemon(
                            pending.species,
                            pending.level,
                            [
                                self.overworld.next_rng_u8(),
                                self.overworld.next_rng_u8(),
                            ],
                        ) {
                            pokemon.ot_id = self.save_data.game_data.player_id;
                            pokemon.ot_name = pokered_core::battle::state::encode_name(&self.player_name);
                            if let Some(nick) = pending.nickname {
                                pokemon.set_nickname(&nick);
                            }
                            // _GivePokemon: party first, else the CURRENT PC box
                            // ("sent to BOX!"); failure was already reported to the
                            // scene via the givePokemon result (both full).
                            if self.save_data.party.count() < 6 {
                                let _ = self.save_data.party.add(pokemon);
                            } else {
                                let _ = self.save_data.pc_storage.deposit_to_current(pokemon);
                                self.save_data.current_box =
                                    self.save_data.pc_storage.current_box().clone();
                            }
                            // A received Pokémon enters the Pokédex as seen + owned.
                            self.save_data.game_data.pokedex.set_seen(pending.species);
                            self.save_data.game_data.pokedex.set_owned(pending.species);
                            self.overworld.party_count = self.save_data.party.count() as u8;
                            self.overworld.box_count = self.save_data.current_box.count() as u8;
                            self.overworld.gift_box_number = self.save_data.pc_storage.current_box_index() as u8 + 1;
                            self.overworld.party_lead_level = self.save_data.party.leader_level();
                        }
                    }

                    if let Some(shop_items) = self.overworld.pending_shop.take() {
                        // The clerk's greeting box is dismissed once the mart
                        // menus take over (pokemart.asm redraws the screen for
                        // MONEY_BOX + BUY_SELL_QUIT_MENU) — drop it so the
                        // map-only backdrop doesn't render it underneath.
                        self.overworld.pending_dialogue = None;
                        match pokered_core::items::shop_stock_from_script_names(&shop_items) {
                            Ok(inv) => {
                                let mart = pokered_core::items::MartState::new(inv);
                                ScreenAction::Transition(GameScreen::Shop(mart))
                            }
                            Err(bad) => {
                                log::warn!("OpenShop: unknown item id '{}', skipping shop", bad);
                                action
                            }
                        }
                    } else if let Some(lucky) = self.overworld.pending_slots.take() {
                        let coins = self.save_data.game_data.player_coins;
                        let seed = (self.frame_count as u32)
                            .wrapping_mul(2654435761)
                            .wrapping_add(1);
                        self.slots_screen = Some(SlotsScreen::new(lucky, coins, seed));
                        ScreenAction::Transition(GameScreen::Slots)
                    } else if let Some(floors) = self.overworld.pending_elevator.take() {
                        self.elevator_screen = Some(ElevatorScreen::new(floors));
                        ScreenAction::Transition(GameScreen::Elevator)
                    } else if let Some(candidates) = self.overworld.pending_filter_bag.take() {
                        // Show only the candidate items the player actually carries.
                        let carried: Vec<String> = candidates
                            .into_iter()
                            .filter(|name| self.save_data.game_data.bag.has_item_const(name))
                            .collect();
                        self.elevator_screen = Some(ElevatorScreen::new_filtered(carried));
                        ScreenAction::Transition(GameScreen::FilterBag)
                    } else if self.overworld.pending_diploma {
                        self.overworld.pending_diploma = false;
                        ScreenAction::Transition(GameScreen::Diploma)
                    } else if self.overworld.pending_town_map
                        && self.overworld.pending_dialogue.is_none()
                    {
                        // Wall TOWN MAP (bookshelf table House $3D): the
                        // "A TOWN MAP." text closed → open the map screen
                        // (TownMapText → DisplayTownMap).
                        self.overworld.pending_town_map = false;
                        self.pending_fly_map = false;
                        ScreenAction::Transition(GameScreen::TownMap)
                    } else if let Some(pc_kind) = self.overworld.pending_pc.take() {
                        // game.openPC() / game.openItemPC() — engine/menus/
                        // pc.asm (Pokémon Center) / players_pc.asm (bedroom).
                        let entry = match pc_kind.as_str() {
                            "items" => PcEntry::PlayersPc,
                            "bills" => PcEntry::BillsPc,
                            _ => PcEntry::PokemonCenter,
                        };
                        let flag = |name: &str| {
                            self.overworld
                                .script_flags()
                                .get(name)
                                .copied()
                                .unwrap_or(false)
                        };
                        let open = PcOpenContext {
                            has_pokedex: flag("EVENT_GOT_POKEDEX"),
                            met_bill: flag("EVENT_MET_BILL"),
                            beaten_league: self.save_data.game_data.num_hof_teams > 0,
                            player_name: self.player_name.clone(),
                            hof_teams: hof_team_records(&self.save_data),
                        };
                        self.pc_screen = Some(PcScreen::new_with_language(entry, &open, self.state.config.language));
                        self.pc_screen.as_mut().unwrap().configure_field_text(self.state.config.text_speed.delay_frames(), self.overworld.text_delay_disabled);
                        if let Some(disabled) = self.pc_screen.as_mut().unwrap().take_field_text_delay_change() {
                            self.overworld.text_delay_disabled = disabled;
                        }
                        ScreenAction::Transition(GameScreen::PC)
                    } else if self.overworld.pending_hof_ceremony {
                        // game.enterHallOfFame() — record the team and start
                        // the roll-call takeover (scripts/HallOfFame.asm).
                        self.overworld.pending_hof_ceremony = false;
                        self.start_hof_ceremony();
                        ScreenAction::Continue
                    } else if let Some(encounter) = self.overworld.pending_wild_encounter.take() {
                        self.start_wild_battle(encounter.species, encounter.level);
                        // The Old-Man catch tutorial auto-plays a guaranteed-catch demo.
                        if encounter.old_man {
                            self.battle.is_old_man = true;
                        }
                        // A rod bite: "The hooked X attacked!" replaces "Wild X
                        // appeared!" (HookedMonAttackedText, wMoveMissed = 1).
                        self.battle.hooked = encounter.hooked;
                        ScreenAction::Transition(GameScreen::Battle)
                    } else if let Some(trainer) = self.overworld.pending_trainer_battle.take() {
                        self.start_trainer_battle(&trainer.trainer_id, trainer.rival_triplet_base);
                        if trainer.npc_index < u8::MAX {
                            self.battle.trainer_npc_index = Some(trainer.npc_index);
                        }
                        self.battle.end_battle_text = trainer.end_battle_text;
                        ScreenAction::Transition(GameScreen::Battle)
                    } else {
                        action
                    }
                }
            }
            GameScreen::Battle => {
                // Keep deterministic turn resolution separate from presentation:
                // finish this move's visuals, then drain its HP, then allow the
                // next narration/action. Core-only consumers remain unblocked.
                self.battle.set_presentation_enabled(!self.battle.link_mode);
                if self.battle.presentation.waiting && self.battle_vfx.is_frame_stable() {
                    self.battle.complete_move_presentation();
                }
                let battle_input = BattleInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    left: input.is_just_pressed(GbButton::Left),
                    right: input.is_just_pressed(GbButton::Right),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                };
                // ── Link battle driving ─────────────────────────────────
                // The CORE `LinkBattleDriver` owns the battle screen (turn
                // resolution, RNG, end detection); `self.battle` is a
                // per-frame MIRROR of the driver's screen so the render /
                // vfx / audio / settle machinery below stays untouched.
                #[cfg_attr(target_os = "none", allow(unused_mut))]
                let mut link_abort = false;
                #[cfg_attr(target_os = "none", allow(unused_mut, unused_variables))]
                let mut battle_over = false;
                #[cfg(not(target_os = "none"))]
                if self.battle.link_mode {
                    if let Some(driver) = self.link_battle.as_mut() {
                        // 1. Turn resolution + disconnect detection (the
                        //    events were already fed to the flow by
                        //    `poll_link`).
                        let _ = driver.poll();
                        // 2. Forward input; the driver swallows the
                        //    end-of-battle transition — link battles return
                        //    to the lobby.
                        let _ = driver.update(battle_input);
                        // 3. Push per-frame config onto the canonical
                        //    screen.
                        if let Some(screen) = driver.screen_mut() {
                            screen.battle_style = self.state.config.battle_style;
                            screen.player_name = Some(self.player_name.clone());
                        }
                        // 4. Mirror the canonical screen into `self.battle`
                        //    for the render/vfx/audio blocks below and the
                        //    settle at the transition. Once the battle is
                        //    over the driver's screen is FROZEN (its `update`
                        //    only runs while `Battling`) — the mirror
                        //    advances through the end-of-battle text below,
                        //    so it must not be re-cloned over that progress.
                        let result = driver.result();
                        // Copy the terminal canonical screen once as well:
                        // a KO can set the result while our previous mirror
                        // is still LinkWaiting. The mirror's result marks that
                        // final copy, so later frames keep its narration progress.
                        if result.is_none() || self.battle.link_result.is_none() {
                            if let Some(screen) = driver.screen() {
                                self.battle = screen.clone();
                                self.battle.link_result = result;
                            }
                        }
                        // 5. The link dropped mid-battle: settle what we
                        //    have (no settlement → no money/exp; the party
                        //    is written back as-is) and return to the room,
                        //    where the flow shows the error box.
                        if matches!(self.link_cable.phase(), CableClubPhase::Error { .. }) {
                            link_abort = true;
                        } else if result.is_some() {
                            // The battle is over. The mirror advances
                            // through the end narration below — exactly like
                            // the app drove the screen before the
                            // consolidation — and its own
                            // Transition(Overworld) triggers the settle and
                            // the rematch reset.
                            battle_over = true;
                        }
                    }
                }
                // wOptions BIT_BATTLE_SHIFT + the player name used by the
                // "Will <PLAYER> change #MON?" prompt — pushed every frame like
                // battle_animation below. (For link battles `self.battle` is
                // the mirror; the canonical screen gets the same pushes above.)
                self.battle.battle_style = self.state.config.battle_style;
                self.battle.player_name = Some(self.player_name.clone());
                let action = if self.battle.link_mode && battle_over {
                    // The driver's screen is frozen at the end narration;
                    // advance the mirror through it (the driver would
                    // swallow the transition anyway).
                    self.battle.update_frame(battle_input)
                } else if self.battle.link_mode {
                    // The driver owns the canonical screen; the mirror is
                    // only ever cloned, never updated.
                    ScreenAction::Continue
                } else {
                    self.battle.update_frame(battle_input)
                };
                // Non-move battle animation requests (ball throws, X-stat
                // items) queued by the core this frame.
                while let Some(event) = self.battle.take_anim_event() {
                    self.battle_vfx.on_anim_event(event);
                }
                // wOptions BIT_BATTLE_ANIMATION, checked at MoveAnimation time.
                self.battle_vfx.animations_enabled = self.state.config.battle_animation;
                // MoveAnimation's opening WaitForSoundToFinish: the visual
                // layer defers the animation start while an SFX is playing —
                // but WaitForSoundToFinish returns immediately while the
                // low-health alarm bit is set (home/delay.asm:15-18).
                self.battle_vfx.sfx_playing = self
                    .audio
                    .as_ref()
                    .is_some_and(|a| a.is_sfx_playing() && !a.low_health_alarm_active());
                self.battle_vfx.update(&self.battle);

                if let Some(ref audio) = self.audio {
                    // wLowHealthAlarm: re-evaluated every frame from the
                    // player mon's HP (DrawPlayerHUDAndHPBar,
                    // engine/battle/core.asm:1851-1875).
                    audio.set_low_health_alarm(self.battle.low_health_alarm());
                    // In-battle POKé FLUTE jingle (Music_PokeFluteInBattle,
                    // audio/poke_flute.asm) — requested by use_poke_flute when
                    // the flute wakes at least one sleeping Pokémon
                    // (engine/items/item_effects.asm:1732-1739).
                    if self.battle.take_poke_flute_sfx_pending() {
                        audio.play_flute_in_battle();
                    }
                    if let Some(sfx) = self.battle.take_item_sfx_pending() {
                        audio.play_sfx(match sfx {
                            pokered_core::battle::BattleItemSfx::HealHp => SfxId::HealHP,
                            pokered_core::battle::BattleItemSfx::HealAilment => SfxId::HealAilment,
                        });
                    }
                    // HP-bar drain starting: play the damage SFX once per
                    // drain. (Deviation: Gen 1's UpdateHPBar drain itself is
                    // silent — hp_bar.asm has no SFX; the task spec asks for
                    // SFX_DAMAGE while draining.)
                    if self.battle.take_hp_drain_sfx_pending() {
                        audio.play_sfx(SfxId::Damage);
                    }
                    // Pokémon cries (tracked by visual-effects layer)
                    if let Some(species) = self.battle_vfx.take_cry_pending() {
                        play_species_cry(audio, species);
                    }
                    // SFX_SILPH_SCOPE as the ghost-Marowak reveal completes
                    // (engine/battle/common_text.asm's `.playSFX`, reached right
                    // after MarowakAnim + the "Wild MAROWAK appeared!" text).
                    if self.battle_vfx.take_silph_scope_sfx_pending() {
                        audio.play_sfx(SfxId::SilphScope);
                    }
                    // Trainer-appear SFX at the start of a trainer intro
                    // (PrintBeginningBattleText's .trainerBattle → .playSFX;
                    // the wTempoModifier write is dead for non-cries, so the
                    // plain SFX_SILPH_SCOPE plays).
                    if self.battle_vfx.take_trainer_appear_sfx_pending() {
                        audio.play_sfx(SfxId::SilphScope);
                    }
                    // Ball-flow SFX (SFX_BALL_TOSS / SFX_TINK per shake /
                    // SFX_BALL_POOF — the BallToss/BallShake/Poof frame hooks
                    // of the original).
                    while let Some(sfx) = self.battle_vfx.take_ball_sfx() {
                        audio.play_sfx(sfx);
                    }

                    // Per-command move animation SFX (GetMoveSound in
                    // PlayAnimation/PlaySubanimation — one play per command).
                    if let Some(req) = self.battle_vfx.take_move_sfx() {
                        use pokered_data::move_sfx::{get_move_sound, MoveSound};
                        match get_move_sound(req.anim_move, req.sound_move, req.attacker_species) {
                            Some(MoveSound::Sfx(raw)) => {
                                if let Some(id) = SfxId::from_u8(raw) {
                                    audio.play_sfx(id);
                                }
                            }
                            Some(MoveSound::Cry {
                                species,
                                pitch_mod,
                                tempo_mod,
                            }) => {
                                // GetCryData sets the cry's modifiers, then
                                // GetMoveSound adds the command's table bytes.
                                let c = pokered_data::cries::cry_data(species);
                                if let Some(id) = SfxId::from_u8(c.sfx) {
                                    audio.play_cry(
                                        id,
                                        c.pitch.wrapping_add(pitch_mod),
                                        c.length.wrapping_add(tempo_mod),
                                    );
                                }
                            }
                            None => {}
                        }
                    }

                    // Victory music
                    if self.battle.is_victory_phase() && !self.battle_vfx.victory_music_played {
                        if let Some(id) = MusicId::from_u8(self.battle.victory_music_id()) {
                            audio.play_music(id);
                        }
                        self.battle_vfx.victory_music_played = true;
                    }

                    // A/B button-press SFX in menu phases
                    let in_menu = matches!(
                        self.battle.phase,
                        BattlePhase::PlayerMenu
                            | BattlePhase::MoveSelect
                            | BattlePhase::PartySelect
                            | BattlePhase::PlayerFaintSwitch
                    );
                    if in_menu
                        && (input.is_just_pressed(GbButton::A)
                            || input.is_just_pressed(GbButton::B))
                    {
                        audio.play_sfx(SfxId::PressAB);
                    }

                    // Message-based battle SFX. Move-use SFX are NOT played
                    // here: in the original they are per-command sounds of the
                    // move animation (see take_move_sfx above).
                    // SFX_FAINT_THUD follows SFX_FAINT_FALL once the fall has
                    // finished (PlaySoundWaitForCurrent → wait → PlaySound,
                    // engine/battle/core.asm:782-791).
                    if self.faint_thud_pending && !audio.is_sfx_playing() {
                        audio.play_sfx(SfxId::FaintThud);
                        self.faint_thud_pending = false;
                    }
                    let cur_message = self.battle.current_message.clone();
                    if cur_message != self.battle_prev_message {
                        if let Some(ref msg) = cur_message {
                            let msg_lower = msg.to_lowercase();
                            if msg_lower.contains("super effective") {
                                audio.play_sfx(SfxId::SuperEffective);
                            } else if msg_lower.contains("not very effective") {
                                audio.play_sfx(SfxId::NotVeryEffective);
                            } else if msg_lower.ends_with("fainted!") {
                                // HandleEnemyMonFainted: trainer battles play
                                // SFX_FAINT_FALL then SFX_FAINT_THUD; wild
                                // battles play the victory music instead (via
                                // is_victory_phase above). A PLAYER faint plays
                                // the mon's own cry, not the fall SFX — that is
                                // queued by battle_vfx (cry_pending).
                                if msg_lower.starts_with("enemy ") && !self.battle.is_wild {
                                    audio.play_sfx(SfxId::FaintFall);
                                    self.faint_thud_pending = true;
                                }
                            } else if msg_lower.contains("come back")
                                || msg_lower.contains("enough")
                            {
                                audio.play_sfx(SfxId::WithdrawDeposit);
                            } else if msg_lower.contains("critical hit") {
                                audio.play_sfx(SfxId::Damage);
                            }
                        }
                        self.battle_prev_message = cur_message;
                    }
                }

                // Link battle over: return to the room and reset the driver
                // for a rematch (the original stays in the Cable Club after
                // the battle — EndOfBattle → overworld → gameboy again). The
                // settle at the transition reads `self.battle` (the final
                // mirror), so the driver reset happens here only.
                #[cfg(not(target_os = "none"))]
                if self.battle.link_mode {
                    if matches!(action, ScreenAction::Transition(GameScreen::Overworld)) {
                        self.link_cable.on_battle_ended();
                        if let Some(driver) = self.link_battle.as_mut() {
                            driver.reset_for_rematch();
                        }
                    }
                }

                if link_abort {
                    ScreenAction::Transition(GameScreen::Overworld)
                } else {
                    action
                }
            }
            GameScreen::StartMenu => {
                if self.overworld.field_text_restore.is_some() {
                    let return_to_menu = self.overworld.field_text_restore.as_ref()
                        .is_some_and(|restore| restore.submenu_reload.is_some());
                    if self.overworld.tick_field_text_restore() && !return_to_menu {
                        ScreenAction::Transition(GameScreen::Overworld)
                    } else { ScreenAction::Continue }
                } else {
                let sm_input = StartMenuInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                    start: input.is_just_pressed(GbButton::Start),
                };
                let sampled=if self.start_menu.field_initialization_active() {
                    if self.start_menu.field_initialization_sound_due() {
                        if let Some(ref audio)=self.audio {audio.play_sfx(SfxId::StartMenu);}
                    }
                    self.start_menu.sample_field_initialization(StartMenuInput {
                        up:input.is_held(GbButton::Up),down:input.is_held(GbButton::Down),
                        a:input.is_held(GbButton::A),b:input.is_held(GbButton::B),start:input.is_held(GbButton::Start),
                    })
                } else {Some(sm_input)};
                if sampled.is_some_and(|input|input.a || input.b) {
                    if let Some(ref audio)=self.audio {audio.play_sfx(SfxId::PressAB);}
                }
                let action = sampled.map(|input| self.start_menu.update_frame(input))
                    .unwrap_or(StartMenuAction::Redisplay);
                if sampled.is_some_and(|i| i.up || i.down) {
                    self.start_menu.begin_direction_delay(StartMenuInput {
                        up: input.is_held(GbButton::Up),
                        down: input.is_held(GbButton::Down),
                        a: input.is_held(GbButton::A),
                        b: input.is_held(GbButton::B),
                        start: input.is_held(GbButton::Start),
                    });
                }
                match action {
                    StartMenuAction::Redisplay if sampled.is_some_and(|i| i.a && !i.up && !i.down)
                        && self.start_menu.current_item() == pokered_core::start_menu::StartMenuItem::Pokemon
                        && self.save_data.party.count() == 0 => {
                        // StartMenu_Pokemon jumps directly to RedisplayStartMenu
                        // on an empty party, with the usual three-frame redraw.
                        ScreenAction::Transition(GameScreen::StartMenu)
                    }
                    StartMenuAction::Close => {
                        self.overworld.begin_start_menu_restore();
                        ScreenAction::Continue
                    },
                    StartMenuAction::OpenOption => {
                        ScreenAction::Transition(GameScreen::OptionsMenu)
                    }
                    StartMenuAction::OpenSave => ScreenAction::Transition(GameScreen::SaveMenu),
                    StartMenuAction::OpenPokemon => ScreenAction::Transition(GameScreen::PartyScreen),
                    StartMenuAction::OpenItem => {
                        let items: Vec<(pokered_data::items::ItemId, u32)> =
                            self.save_data.game_data.bag.items().to_vec();
                        self.bag_screen = BagScreenState::new(items);
                        ScreenAction::Transition(GameScreen::Bag)
                    }
                    StartMenuAction::OpenPokedex => ScreenAction::Transition(GameScreen::Pokedex),
                    StartMenuAction::OpenTrainerInfo => {
                        ScreenAction::Transition(GameScreen::TrainerCard)
                    }
                    _ => ScreenAction::Continue,
                }
                }
            }
            GameScreen::OptionsMenu => {
                let opt_input = OptionsInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    left: input.is_just_pressed(GbButton::Left),
                    right: input.is_just_pressed(GbButton::Right),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                    start: input.is_just_pressed(GbButton::Start),
                };
                let action = match self.options_menu.tick(opt_input) {
                    OptionsMenuResult::Closed => {
                        // Persist the selection into the save data so the next
                        // save keeps it (original: wOptions is part of SRAM).
                        self.save_data.game_data.options = self.options_menu.options;
                        // Entered from the main menu (before a game is loaded)?
                        // Return there instead of the in-game Start menu.
                        if self.main_menu.last_choice
                            == Some(pokered_core::game_state::MainMenuChoice::Option)
                        {
                            self.main_menu.return_from_options();
                            ScreenAction::Transition(GameScreen::MainMenu)
                        } else {
                            ScreenAction::Transition(GameScreen::StartMenu)
                        }
                    }
                    OptionsMenuResult::Active => ScreenAction::Continue,
                };
                // Apply immediately: MoveAnimation checks wOptions each time.
                self.state.config.battle_animation =
                    self.options_menu.options.battle_animation == BattleAnimation::On;
                self.state.config.text_speed = match self.options_menu.options.text_speed {
                    pokered_core::options_menu::TextSpeed::Fast => {
                        pokered_core::game_state::TextSpeed::Fast
                    }
                    pokered_core::options_menu::TextSpeed::Medium => {
                        pokered_core::game_state::TextSpeed::Medium
                    }
                    pokered_core::options_menu::TextSpeed::Slow => {
                        pokered_core::game_state::TextSpeed::Slow
                    }
                };
                self.state.config.battle_style = match self.options_menu.options.battle_style {
                    pokered_core::options_menu::BattleStyle::Shift => {
                        pokered_core::game_state::BattleStyle::Shift
                    }
                    pokered_core::options_menu::BattleStyle::Set => {
                        pokered_core::game_state::BattleStyle::Set
                    }
                };
                action
            }
            GameScreen::SaveMenu => {
                let save_input = YesNoInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                };
                if self.save_menu.phase == SavePhase::SaveComplete {
                    let sfx_done = match self.audio {
                        Some(ref audio) => !audio.is_sfx_playing(),
                        None => true,
                    };
                    if sfx_done {
                        self.save_menu.notify_sfx_done();
                    }
                }
                let result = self.save_menu.tick(save_input);
                if let Some(ref audio) = self.audio {
                    if self.save_menu.sfx_event == SaveSfxEvent::Save {
                        audio.play_sfx(SfxId::Save);
                    }
                }
                match result {
                    SaveMenuResult::Saved => {
                        self.save_to_file();
                        ScreenAction::Transition(GameScreen::StartMenu)
                    }
                    SaveMenuResult::Cancelled => ScreenAction::Transition(GameScreen::StartMenu),
                    SaveMenuResult::Active => ScreenAction::Continue,
                }
            }
            GameScreen::PartyScreen => {
                let party_input = PartyScreenInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                };
                let prior_menu = party_input.a.then(|| self.party_screen.clone());
                let action = self.party_screen.update_frame(party_input);

                // Mirror any in-screen swap back into the canonical save data.
                if let Some((a, b)) = self.party_screen.take_pending_swap() {
                    if let Err(e) = self.save_data.party.swap(a, b) {
                        log::warn!("party swap {a}<->{b} failed: {e:?}");
                    }
                    if let Some(ref audio) = self.audio {
                        audio.play_sfx(SfxId::Swap);
                    }
                }

                match action {
                    PartyScreenAction::ItemUseFinished => {
                        self.pending_bag_item = None;
                        ScreenAction::Transition(GameScreen::Bag)
                    }
                    PartyScreenAction::Cancelled => {
                        // A SOFTBOILED target pick that got back to the normal
                        // menu and was cancelled abandons the heal entirely.
                        self.pending_softboiled_user.take();
                        if self.pending_evolve_move_replace.take().is_some() {
                            // AbandonLearning (learn_move.asm:76-90): backing
                            // out of the forget prompt leaves the move
                            // unlearned; return to the overworld.
                            ScreenAction::Transition(GameScreen::Overworld)
                        } else if self.pending_bag_item.take().is_some() {
                            // Bag item use cancelled: back to the bag
                            // (rebuilt from the live inventory on entry).
                            ScreenAction::Transition(GameScreen::Bag)
                        } else {
                            ScreenAction::Transition(GameScreen::StartMenu)
                        }
                    }
                    PartyScreenAction::ShowStats(idx) => {
                        self.stats_screen = Some(StatsScreenState::new(
                            self.party_screen.party_member(idx).cloned().unwrap(),
                        ));
                        if let Some(prior_menu) = prior_menu { self.party_screen = prior_menu; }
                        ScreenAction::Transition(GameScreen::PokemonStatsScreen(idx))
                    }
                    PartyScreenAction::ApplyItem { party_index } => {
                        // Bag USE → party select: apply the pending item to the
                        // chosen member (Gen-1 medicine / stone / TM-HM party
                        // menu). Success consumes the item and shows the result
                        // text back on the field.
                        match self.pending_bag_item {
                            None => ScreenAction::Continue,
                            Some(item) => {
                                let old_hp = self
                                    .save_data
                                    .party
                                    .get(party_index)
                                    .map(|mon| mon.hp)
                                    .unwrap_or(0);
                                let outcome = match self.save_data.party.get_mut(party_index) {
                                    Some(mon) => bag_use::apply_item_to_pokemon(
                                        item,
                                        mon,
                                        &mut self.save_data.game_data.pokedex,
                                    ),
                                    None => ItemApplyOutcome::NoEffect {
                                        message: bag_use::NO_EFFECT_MESSAGE.to_string(),
                                    },
                                };
                                match outcome {
                                    ItemApplyOutcome::Used { message, consume } => {
                                        if consume {
                                            let _ =
                                                self.save_data.game_data.bag.remove_item(item, 1);
                                        }
                                        // Rare Candy / stone evolution can change
                                        // the lead's level (repel checks it).
                                        self.overworld.party_lead_level =
                                            self.save_data.party.leader_level();
                                        self.party_screen
                                            .refresh_party(self.save_data.party.to_vec());
                                        if let (Some(audio), Some(sfx)) =
                                            (self.audio.as_ref(), bag_use::success_sfx(item))
                                        {
                                            audio.play_sfx(match sfx {
                                                bag_use::ItemUseSfx::HealHp => SfxId::HealHP,
                                                bag_use::ItemUseSfx::HealAilment => {
                                                    SfxId::HealAilment
                                                }
                                            });
                                        }
                                        let wait = if matches!(
                                            bag_use::success_sfx(item),
                                            Some(bag_use::ItemUseSfx::HealHp)
                                                | Some(bag_use::ItemUseSfx::HealAilment)
                                        ) {
                                            50
                                        } else {
                                            0
                                        };
                                        let message = self.localize_dialogue(&message);
                                        if matches!(
                                            bag_use::success_sfx(item),
                                            Some(bag_use::ItemUseSfx::HealHp)
                                        ) {
                                            self.party_screen
                                                .show_item_use_notice_with_hp_animation(
                                                    party_index,
                                                    old_hp,
                                                    message,
                                                    wait,
                                                    PartyNoticeReturn::Bag,
                                                );
                                        } else {
                                            self.party_screen.show_item_use_notice(
                                                message,
                                                wait,
                                                PartyNoticeReturn::Bag,
                                            );
                                        }
                                        self.pending_bag_item = None;
                                        ScreenAction::Continue
                                    }
                                    ItemApplyOutcome::NoEffect { message } => {
                                        let return_to = if bag_use::machine_of(item).is_some() {
                                            PartyNoticeReturn::Party
                                        } else {
                                            self.pending_bag_item = None;
                                            PartyNoticeReturn::Bag
                                        };
                                        self.party_screen.show_item_use_notice(
                                            self.localize_dialogue(&message),
                                            0,
                                            return_to,
                                        );
                                        ScreenAction::Continue
                                    }
                                    ItemApplyOutcome::NeedsMoveReplace { .. } => {
                                        // TM/HM on a full moveset: ask which
                                        // move to forget (stays on this screen).
                                        self.party_screen.enter_move_choice();
                                        ScreenAction::Continue
                                    }
                                    ItemApplyOutcome::EvolutionPending {
                                        pre_text,
                                        from,
                                        to,
                                        force,
                                        consume,
                                    } => {
                                        // Stone / Rare Candy evolution: play
                                        // the evolution cutscene
                                        // (engine/movie/evolution.asm); the
                                        // species swap lands only when it
                                        // confirms (stones set wForceEvolution
                                        // → no B-cancel).
                                        if consume {
                                            let _ =
                                                self.save_data.game_data.bag.remove_item(item, 1);
                                        }
                                        // ItemUseEvoStone plays SFX_HEAL_AILMENT
                                        // before TryEvolvingMon
                                        // (item_effects.asm:779-782).
                                        if force {
                                            if let Some(ref audio) = self.audio {
                                                audio.play_sfx(SfxId::HealAilment);
                                            }
                                        }
                                        self.queue_item_evolution(
                                            party_index,
                                            from,
                                            to,
                                            pre_text,
                                            force,
                                        );
                                        self.overworld.party_lead_level =
                                            self.save_data.party.leader_level();
                                        self.pending_bag_item = None;
                                        ScreenAction::Transition(GameScreen::Overworld)
                                    }
                                }
                            }
                        }
                    }
                    PartyScreenAction::MoveForgetChosen { party_index, slot } => {
                        // Post-evolution full-moveset learn (Gen-1 `LearnMove`,
                        // learn_move.asm:98-184): replace a move with the
                        // level-up move that could not be learned. An HM pick
                        // displays HMCantDeleteText and resumes this same move
                        // list; the pending move is kept until a deletable move
                        // is chosen or the player cancels.
                        if let Some((_, move_id)) = self.pending_evolve_move_replace {
                            let outcome = match self.save_data.party.get_mut(party_index) {
                                Some(mon) => {
                                    use pokered_core::pokemon::move_learning::{
                                        replace_move_guarded, ReplaceMoveError,
                                    };
                                    match replace_move_guarded(mon, slot, move_id) {
                                        Ok(old_move) => {
                                            let mut name_buf =
                                                [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
                                            Ok(format!(
                                                "{} forgot\n{}...\nand learned\n{}!",
                                                mon.display_name(&mut name_buf),
                                                pokered_data::lang_data::move_name(old_move, false),
                                                pokered_data::lang_data::move_name(move_id, false)
                                            ))
                                        }
                                        Err(ReplaceMoveError::HmCantDelete) => Err(true),
                                        Err(ReplaceMoveError::InvalidSlot) => Err(false),
                                    }
                                }
                                None => Err(false),
                            };
                            match outcome {
                                Err(true) => {
                                    let message = self.localize_dialogue(
                                        "HM techniques\ncan't be deleted!",
                                    );
                                    self.party_screen.show_move_choice_notice(message);
                                    ScreenAction::Continue
                                }
                                Ok(message) => {
                                    self.pending_evolve_move_replace = None;
                                    let message = self.localize_dialogue(&message);
                                    self.overworld.pending_dialogue =
                                        Some(BedroomDialogue::from_message(&message));
                                    ScreenAction::Transition(GameScreen::Overworld)
                                }
                                Err(false) => {
                                    self.pending_evolve_move_replace = None;
                                    let message = self.localize_dialogue(
                                        bag_use::NO_EFFECT_MESSAGE,
                                    );
                                    self.overworld.pending_dialogue =
                                        Some(BedroomDialogue::from_message(&message));
                                    ScreenAction::Transition(GameScreen::Overworld)
                                }
                            }
                        } else {
                            // Replace-move confirmation for the pending TM/HM.
                            match self.pending_bag_item {
                            None => ScreenAction::Continue,
                            Some(item) => {
                                let outcome = match self.save_data.party.get_mut(party_index) {
                                    Some(mon) => bag_use::finish_move_choice(item, mon, slot),
                                    None => ItemApplyOutcome::NoEffect {
                                        message: bag_use::NO_EFFECT_MESSAGE.to_string(),
                                    },
                                };
                                let hm_notice = match &outcome {
                                    ItemApplyOutcome::NoEffect { message }
                                        if message.starts_with("HM techniques") =>
                                    {
                                        Some(self.localize_dialogue(message))
                                    }
                                    _ => None,
                                };
                                if let Some(message) = hm_notice {
                                    self.party_screen.show_move_choice_notice(message);
                                    ScreenAction::Continue
                                } else {
                                let (message, consume) = match outcome {
                                    ItemApplyOutcome::Used { message, consume } => {
                                        (message, consume)
                                    }
                                    ItemApplyOutcome::NoEffect { message } => (message, false),
                                    // Invalid/non-machine fallbacks end the flow.
                                    ItemApplyOutcome::NeedsMoveReplace { .. } => {
                                        (bag_use::NO_EFFECT_MESSAGE.to_string(), false)
                                    }
                                    // …nor starts an evolution.
                                    ItemApplyOutcome::EvolutionPending { .. } => {
                                        (bag_use::NO_EFFECT_MESSAGE.to_string(), false)
                                    }
                                };
                                if consume {
                                    let _ = self.save_data.game_data.bag.remove_item(item, 1);
                                }
                                let message = self.localize_dialogue(&message);
                                self.overworld.pending_dialogue =
                                    Some(BedroomDialogue::from_message(&message));
                                self.pending_bag_item = None;
                                ScreenAction::Transition(GameScreen::Overworld)
                                }
                            }
                            }
                        }
                    }
                    PartyScreenAction::UseFieldMove {
                        party_index,
                        move_id,
                    } => {
                        // Party-menu HM use (Gen-1 start_sub_menus.asm): the
                        // overworld applies the effect and queues any result
                        // text; FLY hands off to the town map picker instead.
                        let outcome = match self.party_screen.party_member(party_index) {
                            Some(mon) => {
                                let mon = mon.clone();
                                self.overworld.use_field_move(
                                    move_id,
                                    &mon,
                                    self.save_data.game_data.obtained_badges,
                                    pokered_data::maps::MapId::from_u8(
                                        self.save_data.game_data.last_blackout_map,
                                    )
                                    .unwrap_or(pokered_data::maps::MapId::PalletTown),
                                )
                            }
                            None => pokered_core::overworld::field_moves::FieldMoveOutcome::Done,
                        };
                        match outcome {
                            pokered_core::overworld::field_moves::FieldMoveOutcome::Done => {
                                ScreenAction::Transition(GameScreen::Overworld)
                            }
                            pokered_core::overworld::field_moves::FieldMoveOutcome::OpenFlyMap => {
                                self.pending_fly_map = true;
                                ScreenAction::Transition(GameScreen::TownMap)
                            }
                            // SOFTBOILED: reopen the party menu to pick the
                            // target (start_sub_menus.asm `.softboiled` →
                            // GoBackToPartyMenu). The user stays on the party
                            // screen; the entry hook swaps it into
                            // SoftboiledTarget mode.
                            pokered_core::overworld::field_moves::FieldMoveOutcome::ChooseSoftboiledTarget => {
                                self.pending_softboiled_user = Some(party_index);
                                ScreenAction::Transition(GameScreen::PartyScreen)
                            }
                        }
                    }
                    // SOFTBOILED target picked: the user loses 1/5 max HP, the
                    // target gains it (capped) — ItemUseMedicine's pseudo-item
                    // path, engine/items/item_effects.asm:1003-1074.
                    PartyScreenAction::SoftboiledTargetChosen { target_index } => {
                        let user_index = self.pending_softboiled_user.take();
                        let outcome = match user_index {
                            Some(user_index) => match self
                                .save_data
                                .party
                                .get_two_mut(user_index, target_index)
                            {
                                Some((user, target)) => {
                                    pokered_core::items::bag_use::apply_softboiled(user, target)
                                }
                                None => pokered_core::items::bag_use::ItemApplyOutcome::NoEffect {
                                    message: pokered_core::items::bag_use::NO_EFFECT_MESSAGE
                                        .to_string(),
                                },
                            },
                            None => pokered_core::items::bag_use::ItemApplyOutcome::NoEffect {
                                message: pokered_core::items::bag_use::NO_EFFECT_MESSAGE
                                    .to_string(),
                            },
                        };
                        let message = match outcome {
                            pokered_core::items::bag_use::ItemApplyOutcome::Used {
                                message,
                                ..
                            } => message,
                            pokered_core::items::bag_use::ItemApplyOutcome::NoEffect {
                                message,
                            } => message,
                            // apply_softboiled never asks for a move replace or
                            // an evolution.
                            _ => pokered_core::items::bag_use::NO_EFFECT_MESSAGE.to_string(),
                        };
                        let message = self.localize_dialogue(&message);
                        self.overworld.pending_dialogue =
                            Some(BedroomDialogue::from_message(&message));
                        ScreenAction::Transition(GameScreen::Overworld)
                    }
                    PartyScreenAction::Active => ScreenAction::Continue,
                }
            }
            GameScreen::Bag => {
                let bag_input = BagScreenInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    left: input.is_just_pressed(GbButton::Left),
                    right: input.is_just_pressed(GbButton::Right),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                    select: input.is_just_pressed(GbButton::Select),
                };
                match self.bag_screen.update_frame(bag_input) {
                    BagScreenAction::Cancelled => ScreenAction::Transition(GameScreen::StartMenu),
                    BagScreenAction::TossItem {
                        item,
                        index,
                        quantity,
                    } => {
                        // Key items (incl. HMs) refuse: "That's too important
                        // to toss!" (_TooImportantToTossText). Everything else
                        // is removed and the bag view rebuilt.
                        if pokered_core::items::inventory::is_tossable(item) {
                            let _ = self
                                .save_data
                                .game_data
                                .bag
                                .toss_item(index, quantity.min(99) as u8);
                            self.bag_screen
                                .set_items(self.save_data.game_data.bag.items().to_vec());
                            ScreenAction::Continue
                        } else {
                            self.overworld.pending_dialogue = Some(BedroomDialogue::from_message(
                                &self.localize_dialogue("That's too impor-\ntant to toss!"),
                            ));
                            ScreenAction::Transition(GameScreen::Overworld)
                        }
                    }
                    BagScreenAction::UseItem { item, .. } => {
                        // TOWN MAP opens its own viewer screen. Party-targeted
                        // items (potions, stones, TM/HM…) open the party screen
                        // in item-use mode; every other field item dispatches
                        // an overworld effect (POKe FLUTE wakes the Snorlax,
                        // BICYCLE toggles riding, REPEL/ESCAPE ROPE…) and shows
                        // its message. Consumed items leave the bag.
                        if item == pokered_data::items::ItemId::TownMap {
                            ScreenAction::Transition(GameScreen::TownMap)
                        } else if item == pokered_data::items::ItemId::Pokedex {
                            ScreenAction::Transition(GameScreen::Pokedex)
                        } else {
                            match bag_use::classify_bag_use(item) {
                                bag_use::BagUseKind::OnPokemon => {
                                    self.pending_bag_item = Some(item);
                                    ScreenAction::Transition(GameScreen::PartyScreen)
                                }
                                bag_use::BagUseKind::Field | bag_use::BagUseKind::NotTime => {
                                    // ESCAPE ROPE warps to the last Pokémon
                                    // Center's fly point (wLastBlackoutMap) —
                                    // the same target DIG/TELEPORT use.
                                    let last_blackout = pokered_data::maps::MapId::from_u8(
                                        self.save_data.game_data.last_blackout_map,
                                    )
                                    .unwrap_or(pokered_data::maps::MapId::PalletTown);
                                    let consumed =
                                        self.overworld.use_field_item(item, last_blackout);
                                    if consumed {
                                        let _ = self.save_data.game_data.bag.remove_item(item, 1);
                                    }
                                    ScreenAction::Transition(GameScreen::Overworld)
                                }
                            }
                        }
                    }
                    BagScreenAction::Active => ScreenAction::Continue,
                }
            }
            GameScreen::Pokedex => {
                // The cry plays when an entry opens (PlayCry in
                // ShowPokedexDataInternal).
                self.pokedex_screen.language = self.state.config.language;
                if self.pokedex_screen.take_cry_pending() {
                    if let Some(ref audio) = self.audio {
                        play_species_cry(audio, self.pokedex_screen.cursor_species());
                    }
                }
                let dex_input = PokedexScreenInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    left: input.is_just_pressed(GbButton::Left),
                    right: input.is_just_pressed(GbButton::Right),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                };
                match self.pokedex_screen.update_frame(dex_input) {
                    // List-B / post-capture close: the original returns to the
                    // start menu (RedisplayStartMenu); the post-capture entry
                    // returns to the overworld.
                    PokedexScreenAction::Closed => {
                        if self.pokedex_screen.from_list() {
                            ScreenAction::Transition(GameScreen::StartMenu)
                        } else {
                            ScreenAction::Transition(GameScreen::Overworld)
                        }
                    }
                    PokedexScreenAction::Active => ScreenAction::Continue,
                }
            }
            GameScreen::TrainerCard => {
                let tc_input = TrainerCardInput {
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                };
                match self.trainer_card_screen.update_frame(tc_input) {
                    // RedisplayStartMenu (start_sub_menus.asm:475).
                    TrainerCardAction::Closed => ScreenAction::Transition(GameScreen::StartMenu),
                    TrainerCardAction::Active => ScreenAction::Continue,
                }
            }
            GameScreen::TownMap => {
                if self.fly_departure_screen_frames > 0 {
                    self.overworld.update_frame(
                        pokered_core::overworld::OverworldInput::new(
                            false, false, false, false, false, false, false, false,
                        ),
                    );
                    self.fly_departure_screen_frames -= 1;
                    if self.fly_departure_screen_frames == 0 {
                        ScreenAction::Transition(GameScreen::Overworld)
                    } else {
                        ScreenAction::Continue
                    }
                } else {
                    let tm_input = TownMapScreenInput {
                        up: input.is_just_pressed(GbButton::Up),
                        down: input.is_just_pressed(GbButton::Down),
                        a: input.is_just_pressed(GbButton::A),
                        b: input.is_just_pressed(GbButton::B),
                    };
                    match self.town_map_screen.update_frame(tm_input) {
                        TownMapScreenAction::Closed => {
                            // FLY cancel returns to the party menu (Gen-1 flow);
                            // the bag's TOWN MAP viewer returns to the overworld.
                            if self.town_map_screen.mode()
                                == pokered_core::town_map_screen::TownMapMode::Fly
                            {
                                ScreenAction::Transition(GameScreen::PartyScreen)
                            } else {
                                ScreenAction::Transition(GameScreen::Overworld)
                            }
                        }
                        TownMapScreenAction::FlyTo(dest) => {
                            // BIT_FLY_WARP: leave the selected town-map frame
                            // up while `_LeaveMapAnim` starts, then hand drawing
                            // back to the overworld for the bird departure.
                            let point =
                                pokered_core::overworld::hm_effects::fly_destination_for_map(dest);
                            if let Some(point) = point {
                                self.overworld.fly_warp_to(point.map, point.x, point.y);
                                self.fly_departure_screen_frames = pokered_core::overworld::presentation::FLY_DEPARTURE_TOWN_MAP_FRAMES;
                            }
                            ScreenAction::Continue
                        }
                        TownMapScreenAction::Active => ScreenAction::Continue,
                    }
                }
            }
            // PlayCry waits for the sound to finish before either stats
            // page accepts input (party and PC share this screen).
            GameScreen::PokemonStatsScreen(_idx) => {
                let cry_holds_input = self.audio.as_ref().is_some_and(|audio| audio.is_sfx_playing());
                if let Some(ref mut ss) = self.stats_screen {
                    let input = StatsScreenInput {
                        a: !cry_holds_input && input.is_just_pressed(GbButton::A),
                        b: !cry_holds_input && input.is_just_pressed(GbButton::B),
                    };
                    let action = ss.update(input);
                    if ss.take_entry_cry() {
                        if let Some(audio) = &self.audio { play_species_cry(audio, ss.pokemon().species); }
                    }
                    if let Some(audio) = &self.audio {
                        let volume = if !ss.entry_blocks_input() && audio.is_sfx_playing()
                            || ss.entry_frame() == Some(ss.entry_cry_frame()) { 7 } else { 3 };
                        audio.set_master_volume(volume, volume);
                    }
                    match action {
                        StatsScreenAction::Continue => ScreenAction::Continue,
                        StatsScreenAction::BackToParty => {
                            if let Some(audio) = &self.audio { audio.set_master_volume(7, 7); }
                            self.stats_screen = None;
                            // STATS opened from the PC's mon list returns to
                            // the PC (its state is still in `pc_screen`).
                            if self.pc_screen.is_some() {
                                self.pc_stats_return_frame = Some(0);
                                ScreenAction::Transition(GameScreen::PC)
                            } else {
                                ScreenAction::Transition(GameScreen::PartyScreen)
                            }
                        }
                    }
                } else {
                    ScreenAction::Transition(GameScreen::PartyScreen)
                }
            }
            GameScreen::Slots => {
                let slots_input = SlotsInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    left: input.is_just_pressed(GbButton::Left),
                    right: input.is_just_pressed(GbButton::Right),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                };
                let mut result = SlotsAction::Continue;
                let mut coins_out = None;
                if let Some(ref mut slots) = self.slots_screen {
                    result = slots.update_frame(slots_input);
                    coins_out = Some(slots.coins);
                    // The slots' own cues (slot_machine.asm: :120 spin start,
                    // :842 each reel stop, :694 per-coin payout tick, :588
                    // bar stinger, :599 seven stinger).
                    let sfx = slots.take_sfx();
                    if let Some(ref audio) = self.audio {
                        use pokered_core::slots_screen::SlotsSfx;
                        for cue in sfx {
                            let id = match cue {
                                SlotsSfx::NewSpin => SfxId::SlotsNewSpin,
                                SlotsSfx::StopWheel => SfxId::SlotsStopWheel,
                                SlotsSfx::Reward => SfxId::SlotsReward,
                                SlotsSfx::GetKeyItem => SfxId::GetKeyItem,
                                SlotsSfx::GetItem2 => SfxId::GetItem2,
                            };
                            audio.play_sfx(id);
                        }
                    }
                }
                // Persist the running coin balance every frame.
                if let Some(coins) = coins_out {
                    self.save_data.game_data.player_coins = coins;
                }
                match result {
                    SlotsAction::Continue => ScreenAction::Continue,
                    SlotsAction::Exit => {
                        self.slots_screen = None;
                        ScreenAction::Transition(GameScreen::Overworld)
                    }
                }
            }
            GameScreen::Elevator => {
                let elevator_input = ElevatorInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                };
                let mut resume_floor: Option<i32> = None;
                if let Some(ref mut elevator) = self.elevator_screen {
                    if let Some(audio) = self.audio.as_ref() {
                        if elevator_input.a {
                            audio.play_sfx(SfxId::PressAB);
                        }
                    }
                    match elevator.update_frame(elevator_input) {
                        ElevatorAction::Continue => {}
                        ElevatorAction::Select(idx) => {
                            resume_floor = Some(idx as i32);
                        }
                        ElevatorAction::Cancel => {
                            resume_floor = Some(-1);
                        }
                    }
                }
                if let Some(floor) = resume_floor {
                    self.elevator_screen = None;
                    self.overworld.resume_script_after_elevator(floor);
                    ScreenAction::Transition(GameScreen::Overworld)
                } else {
                    ScreenAction::Continue
                }
            }
            GameScreen::FilterBag => {
                let filter_input = ElevatorInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                };
                let mut resume_item: Option<String> = None;
                if let Some(ref mut filter) = self.elevator_screen {
                    match filter.update_frame(filter_input) {
                        ElevatorAction::Continue => {}
                        ElevatorAction::Select(idx) => {
                            resume_item = filter.floors().get(idx).cloned();
                        }
                        ElevatorAction::Cancel => {
                            resume_item = Some(String::new());
                        }
                    }
                }
                if let Some(item) = resume_item {
                    self.elevator_screen = None;
                    self.overworld.resume_script_after_filter_bag(&item);
                    ScreenAction::Transition(GameScreen::Overworld)
                } else {
                    ScreenAction::Continue
                }
            }
            GameScreen::Diploma => {
                // A/B closes the certificate back to the overworld.
                if input.is_just_pressed(GbButton::A) || input.is_just_pressed(GbButton::B) {
                    ScreenAction::Transition(GameScreen::Overworld)
                } else {
                    ScreenAction::Continue
                }
            }
            GameScreen::PC => {
                let restoring_tiles = self.pc_stats_return_frame.is_some_and(|f| f < 6);
                let menu_input = MenuInput {
                    up: !restoring_tiles && input.is_just_pressed(GbButton::Up),
                    down: !restoring_tiles && input.is_just_pressed(GbButton::Down),
                    a: !restoring_tiles && input.is_just_pressed(GbButton::A),
                    b: !restoring_tiles && input.is_just_pressed(GbButton::B),
                };
                if self.pc_screen.is_none() {
                    // Screen state lost (shouldn't happen) — bail out cleanly.
                    ScreenAction::Transition(GameScreen::Overworld)
                } else {
                    let pc = self.pc_screen.as_mut().unwrap();
                    // PCMainMenu / PlayerPCMenu set BIT_NO_MENU_BUTTON_SOUND.
                    // The standalone BillsPc entry retains its ordinary key sound.
                    if menu_input.a && pc.entry() == PcEntry::BillsPc && pc.phase() != pokered_core::pc_screen::PcPhase::Message && !pc.waiting_for_sound() {
                        if let Some(ref audio) = self.audio { audio.play_sfx(SfxId::PressAB); }
                    }
                    let pc_action = {
                        let mut ctx = PcContext {
                            party: &mut self.save_data.party,
                            pc_storage: &mut self.save_data.pc_storage,
                            bag: &mut self.save_data.game_data.bag,
                            pc_items: &mut self.save_data.game_data.pc_items,
                            pokedex: &self.save_data.game_data.pokedex,
                        };
                        if let Some(ref audio) = self.audio {
                            pc.update_frame_with_text_input(menu_input, &mut ctx, audio.is_sfx_playing(), input.is_held(GbButton::A) || input.is_held(GbButton::B))
                        } else { pc.update_frame_with_text_input(menu_input, &mut ctx, false, input.is_held(GbButton::A) || input.is_held(GbButton::B)) }
                    };
                    if let Some(disabled) = pc.take_field_text_delay_change() {
                        self.overworld.text_delay_disabled = disabled;
                    }
                    // Every mutation must refresh the live bank-1 box, not only CHANGE BOX.
                    self.save_data.current_box = self.save_data.pc_storage.current_box().clone();
                    for sfx in pc.take_sfx() {
                        if let Some(ref audio) = self.audio {
                            let id = match sfx {
                                PcSfx::TurnOn => SfxId::TurnOnPC,
                                PcSfx::TextAdvance => SfxId::PressAB,
                                PcSfx::PokedexRating {tier} => [SfxId::Denied,SfxId::PokedexRating,SfxId::GetItem1,SfxId::CaughtMon,SfxId::LevelUp,SfxId::GetKeyItem,SfxId::GetItem2][usize::from(tier)],
                                PcSfx::TurnOff => SfxId::TurnOffPC,
                                PcSfx::Enter => SfxId::EnterPC,
                                PcSfx::WithdrawDeposit => SfxId::WithdrawDeposit,
                                PcSfx::Save => SfxId::Save,
                                PcSfx::Cry(species) => { play_species_cry(audio, species); continue; }
                            };
                            audio.play_sfx(id);
                        }
                    }
                    if pc.phase() == pokered_core::pc_screen::PcPhase::BillsMenu {
                        // BillsPCMenu also clears the shared party selection.
                        self.party_screen.set_cursor(0);
                    }
                    if pc.take_save_request() {
                        // CHANGE BOX saves the game (save.asm ChangeBox →
                        // SaveGameData); keep the SRAM box-num byte in sync
                        // (bit 7 = "has changed boxes", wCurrentBoxNum).
                        self.save_data.game_data.current_box_num =
                            self.save_data.pc_storage.current_box_index() as u8 | 0x80;
                        // Keep the sCurBoxData mirror (the bank-1 copy of the
                        // active box the original rewrites on every save,
                        // save.asm:229-233) in sync with the storage.
                        self.save_data.current_box =
                            self.save_data.pc_storage.current_box().clone();
                        self.save_to_file();
                    }
                    // Party membership may have changed (deposit/withdraw) — keep
                    // the overworld mirrors in sync (repel checks, scripts).
                    self.overworld.party_count = self.save_data.party.count() as u8;
                    self.overworld.box_count = self.save_data.current_box.count() as u8;
                    self.overworld.gift_box_number = self.save_data.pc_storage.current_box_index() as u8 + 1;
                    self.overworld.party_lead_level = self.save_data.party.leader_level();
                    match pc_action {
                        PcScreenAction::Continue => ScreenAction::Continue,
                        PcScreenAction::Exit => {
                            self.pc_screen = None;
                            ScreenAction::Transition(GameScreen::Overworld)
                        }
                        PcScreenAction::ShowStats { from_box, index } => {
                            let mon = if from_box {
                                self.save_data.pc_storage.current_box().get(index).cloned()
                            } else {
                                self.save_data.party.get(index).cloned()
                            };
                            match mon {
                                Some(mon) => {
                                    self.stats_screen = Some(if from_box {
                                        StatsScreenState::from_box(mon)
                                    } else { StatsScreenState::new(mon) });
                                    ScreenAction::Transition(GameScreen::PokemonStatsScreen(index))
                                }
                                None => ScreenAction::Continue,
                            }
                        }
                    }
                }
            }
            GameScreen::Shop(ref mut mart_state) => {
                let menu_input = MenuInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                };
                let mut player = PlayerData {
                    money: self.save_data.game_data.player_money,
                    bag: self.save_data.game_data.bag.clone(),
                };
                let update = mart_state.update_frame(menu_input, &mut player);
                self.save_data.game_data.player_money = player.money;
                self.save_data.game_data.bag = player.bag;
                match update {
                    MartUpdate::Continue => ScreenAction::Continue,
                    MartUpdate::PlaySound(SoundId::Purchase) => {
                        if let Some(ref audio) = self.audio {
                            audio.play_sfx(SfxId::Purchase);
                        }
                        ScreenAction::Continue
                    }
                    MartUpdate::Exit => ScreenAction::Transition(GameScreen::Overworld),
                }
            }
        };

        if let ScreenAction::Transition(new_screen) = action {
            // RedisplayStartMenu compares its first Joypad with the sample
            // which closed the submenu, including a still-held return B.
            if new_screen == GameScreen::StartMenu && self.state.screen != GameScreen::Overworld {
                self.overworld.synchronize_player_input(OverworldInput::new(
                    input.is_held(GbButton::Up), input.is_held(GbButton::Down),
                    input.is_held(GbButton::Left), input.is_held(GbButton::Right),
                    input.is_held(GbButton::A), input.is_held(GbButton::B),
                    input.is_held(GbButton::Start), input.is_held(GbButton::Select),
                ));
            }
            use pokered_core::game_state::MainMenuChoice;
            let needs_black_screen = new_screen == GameScreen::Overworld
                && self.state.screen == GameScreen::MainMenu
                && self.main_menu.last_choice == Some(MainMenuChoice::Continue);

            if needs_black_screen {
                self.black_screen_frames = BLACK_SCREEN_DURATION;
                self.pending_screen = Some(new_screen);
            } else {
                // Some transition constructors contain large inline arrays.
                // Let the bare-metal entry point run them after `update_inner`
                // has returned so their stack frames do not overlap.
                #[cfg(target_os = "none")]
                {
                    self.pending_screen = Some(new_screen);
                }
                #[cfg(not(target_os = "none"))]
                {
                    self.handle_transition(new_screen);
                }
            }
        }
    }

    /// Complete a transition after the main update stack frame has unwound.
    /// Large inline save/overworld values otherwise overflow the GBA's small
    /// software stack when their constructors are nested in `update_inner`.
    #[cfg(target_os = "none")]
    pub fn flush_deferred_transition(&mut self) {
        if self.black_screen_frames == 0 {
            if let Some(screen) = self.pending_screen.take() {
                self.handle_transition(screen);
            }
        }
    }

    /// Poll the link layer once per frame: accept a pending peer, route
    /// transport messages into the per-activity queues, drive the CORE
    /// battle/trade drivers, and keep `link_status` in sync for the Cable
    /// Club UI.
    #[cfg(not(target_os = "none"))]
    fn poll_link(&mut self) {
        use pokered_core::link::protocol::LINK_RANDOM_LIST_SIZE;

        // Server pending: try a non-blocking accept each frame. (Native
        // only — the wasm transport is created by the entry point and
        // attached through `attach_link_transport`.)
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(server) = self.link_server.take() {
            match server.accept() {
                Ok(Some(transport)) => {
                    dbg_eprintln!("[link] peer connected, waiting for its Hello");
                    // The acceptor side does NOT start the handshake: the
                    // core Hello/HelloAck exchange is asymmetric (initiator
                    // sends Hello, receiver auto-acks from its Idle state), so
                    // only the `--link-connect` client starts it (below).
                    self.link_session = Some(LinkSession::new(
                        Box::new(transport),
                        crate::link::link_activity,
                        NetworkMessage::Disconnect,
                    ));
                    self.link_status = LinkStatus::Connecting;
                }
                Ok(None) => {
                    // Still waiting for the peer; keep the server for next frame.
                    self.link_server = Some(server);
                }
                Err(e) => {
                    dbg_eprintln!("[link] accept failed: {}", e);
                    self.link_status = LinkStatus::Disconnected(e.to_string());
                }
            }
        }

        let Some(session) = self.link_session.as_mut() else {
            return;
        };

        // Route everything the transport has into the per-activity queues.
        if let Some(reason) = session.poll() {
            dbg_eprintln!("[link] transport closed: {}", reason);
        }

        // Lazily create the CORE drivers on the routed sub-transports. They
        // are created at connect (they own the handshake), with the party as
        // it is NOW — the snapshot is refreshed at the cable-club table
        // (see `handle_flow_need`).
        if self.link_battle.is_none() {
            // The 10-byte random list (SERIAL_RNS_LENGTH): generated with a
            // tiny xorshift seeded from the clock (pokered-app has no rand
            // dependency; the values only need to be host-known — both sides
            // consume the host's list).
            let mut seed = link_random_seed() | 1;
            let mut next_byte = move || {
                // xorshift32
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                (seed >> 16) as u8
            };
            let random_numbers: [u8; LINK_RANDOM_LIST_SIZE] = core::array::from_fn(|_| next_byte());
            let mut driver = LinkBattleDriver::new(
                session.battle_transport(),
                self.save_data.party.clone(),
                self.player_name.clone(),
            )
            .with_role(self.link_role)
            .with_host_random_list(random_numbers);
            // The client (`--link-connect`, the "guest" side) starts the
            // asymmetric Hello/HelloAck exchange; the server auto-acks from
            // the driver's Idle state.
            if self.link_role == LinkRole::Guest {
                if let Err(e) = driver.start_handshake() {
                    dbg_eprintln!("[link] handshake failed: {}", e);
                    self.link_status = LinkStatus::Disconnected(e.to_string());
                }
            }
            self.link_battle = Some(driver);
        }
        if self.link_trade.is_none() {
            let driver = LinkTradeDriver::new(
                self.save_data.party.clone(),
                self.save_data.game_data.player_id,
            )
            .with_role(self.link_role)
            .with_trainer_name(self.player_name.clone());
            self.link_trade = Some(driver);
        }

        // Drive the battle driver; feed its events to the Cable Club flow
        // and keep the link status in sync.
        let mut needs = Vec::new();
        if let Some(driver) = self.link_battle.as_mut() {
            for ev in driver.poll() {
                match &ev {
                    LinkDriverEvent::Connected => {
                        dbg_eprintln!("[link] handshake complete — connected");
                        self.link_status = LinkStatus::Connected;
                    }
                    LinkDriverEvent::Disconnected(reason) => {
                        dbg_eprintln!("[link] disconnected: {}", reason);
                        self.link_status = LinkStatus::Disconnected("Player2 disconnected".into());
                    }
                    LinkDriverEvent::BattleRequested => {
                        // The peer's request can arrive before our party was
                        // refreshed: sync the driver's snapshot now so the
                        // accept-exchange sends the CURRENT party (the
                        // original's HealParty at the table,
                        // cable_club.asm:292).
                        if let Some(d) = self.link_battle.as_mut() {
                            d.set_local_party(self.save_data.party.clone());
                        }
                    }
                    _ => {}
                }
                let need = self.link_cable.on_battle_event(&ev);
                if need != FlowNeed::None {
                    needs.push(need);
                }
            }
        }

        // Drive the trade driver the same way.
        if let Some(driver) = self.link_trade.as_mut() {
            let result = driver.poll(&mut *session.trade_transport());
            self.link_cable
                .set_trainer_names(&self.player_name, driver.remote_name());
            if let Some(party) = driver.remote_party() {
                self.link_cable.set_remote_party(&party.to_vec());
            } else {
                self.link_cable.set_remote_party(&[]);
            }
            match &result {
                LinkTradePollResult::Disconnected => {
                    self.link_status = LinkStatus::Disconnected("Player2 disconnected".into());
                }
                LinkTradePollResult::Error(e) => {
                    self.link_status = LinkStatus::Disconnected(format!("link error: {}", e));
                }
                LinkTradePollResult::TradeRequested => {
                    // Same party refresh as the battle path above.
                    if let Some(d) = self.link_trade.as_mut() {
                        d.set_party(self.save_data.party.clone());
                    }
                }
                _ => {}
            }
            if self.link_trade.as_ref().is_some_and(|d| d.both_presentations_ready()) {
                self.link_cable.on_trade_presentations_ready();
            }
            let need = self.link_cable.on_trade_event(&result);
            if need != FlowNeed::None {
                needs.push(need);
            }
        }
        // Execute any flow request (currently the flow's event arms are
        // bookkeeping-only; the action needs come from modal input).
        for need in needs {
            self.handle_flow_need(need);
        }
    }

    /// Execute a [`FlowNeed`] issued by the Cable Club flow. The flow does
    /// not own the drivers or the save party, so the game loop performs the
    /// actual driver calls and party snapshots here.
    #[cfg(not(target_os = "none"))]
    fn handle_flow_need(&mut self, need: FlowNeed) {
        let result = match &need {
            FlowNeed::None => return,
            FlowNeed::CancelReception => {
                self.overworld.pending_dialogue=Some(pokered_core::overworld::BedroomDialogue::from_message(
                    if self.state.config.language==pokered_core::game_state::Lang::Zh { "欢迎下次再来！" } else { "Please come again!" }));
                return;
            }
            FlowNeed::CancelRoomSelection => {
                self.overworld.pending_dialogue = Some(pokered_core::overworld::BedroomDialogue::from_message(
                    if self.state.config.language == pokered_core::game_state::Lang::Zh {
                        "联机被\n取消了。"
                    } else {
                        "The link was\ncanceled."
                    }));
                return;
            }
            FlowNeed::SaveReception => {
                self.save_to_file();
                if let Some(audio) = &self.audio { audio.play_sfx(SfxId::Save); }
                return;
            }
            FlowNeed::EnterRoom(kind) => {
                self.warp_to_cable_room(match kind { LinkKind::Trade => MapId::TradeCenter, LinkKind::Battle => MapId::Colosseum });
                return;
            }
            FlowNeed::RequestLink(kind) => {
                let Some(session) = self.link_session.as_mut() else {
                    return;
                };
                match kind {
                    LinkKind::Battle => {
                        let Some(driver) = self.link_battle.as_mut() else {
                            return;
                        };
                        // Exchange the current party. Original HealParty runs
                        // after the link battle, not before the exchange.
                        driver.set_local_party(self.save_data.party.clone());
                        driver.request_battle()
                    }
                    LinkKind::Trade => {
                        let Some(driver) = self.link_trade.as_mut() else {
                            return;
                        };
                        // Same table-time snapshot for the trade flow.
                        driver.set_party(self.save_data.party.clone());
                        driver
                            .request_trade(&mut *session.trade_transport())
                            .map_err(link_trade_err_to_transport)
                    }
                }
            }
            FlowNeed::ReplyRequest { kind, accept } => {
                let Some(session) = self.link_session.as_mut() else {
                    return;
                };
                match (kind, accept) {
                    (LinkKind::Battle, true) => {
                        let Some(driver) = self.link_battle.as_mut() else { return; };
                        driver.set_local_party(self.save_data.party.clone());
                        driver.accept_battle()
                    }
                    (LinkKind::Battle, false) => {
                        let Some(driver) = self.link_battle.as_mut() else {
                            return;
                        };
                        driver.decline_battle()
                    }
                    (LinkKind::Trade, true) => {
                        let Some(driver) = self.link_trade.as_mut() else { return; };
                        driver.set_party(self.save_data.party.clone());
                        driver
                            .accept_trade(&mut *session.trade_transport())
                            .map_err(link_trade_err_to_transport)
                    }
                    (LinkKind::Trade, false) => {
                        let Some(driver) = self.link_trade.as_mut() else {
                            return;
                        };
                        driver
                            .decline_trade(&mut *session.trade_transport())
                            .map_err(link_trade_err_to_transport)
                    }
                }
            }
            FlowNeed::CompleteTrade => {
                self.save_link_party_and_dex();
                Ok(())
            }
            FlowNeed::LeaveTrade => {
                if let Some(driver) = self.link_trade.as_mut() {
                    driver.leave_trade();
                }
                Ok(())
            }
            FlowNeed::ContinueTrade => {
                let Some(session) = self.link_session.as_mut() else {
                    return;
                };
                let Some(driver) = self.link_trade.as_mut() else {
                    return;
                };
                driver.set_party(self.save_data.party.clone());
                driver
                    .continue_trade(&mut *session.trade_transport())
                    .map_err(link_trade_err_to_transport)
            }
            FlowNeed::SelectMon(idx) => {
                let Some(session) = self.link_session.as_mut() else {
                    return;
                };
                let Some(driver) = self.link_trade.as_mut() else {
                    return;
                };
                driver
                    .select_mon(&mut *session.trade_transport(), *idx)
                    .map_err(link_trade_err_to_transport)
            }
            FlowNeed::ResumeSelection => {
                if let Some(driver) = self.link_trade.as_mut() { driver.resume_selection(); }
                Ok(())
            }
            FlowNeed::SelectMonAgainstCancel(idx) => {
                let Some(session) = self.link_session.as_mut() else { return; };
                let Some(driver) = self.link_trade.as_mut() else { return; };
                driver.select_mon_against_cancel(&mut *session.trade_transport(), *idx)
                    .map_err(link_trade_err_to_transport)
            }
            FlowNeed::RejectTrade => {
                let Some(session) = self.link_session.as_mut() else { return; };
                let Some(driver) = self.link_trade.as_mut() else { return; };
                driver.reject_trade(&mut *session.trade_transport()).map_err(link_trade_err_to_transport)
            }
            FlowNeed::CancelTrade | FlowNeed::CancelTradeAndLeave => {
                let Some(session) = self.link_session.as_mut() else {
                    return;
                };
                let Some(driver) = self.link_trade.as_mut() else {
                    return;
                };
                let result = driver.cancel_trade(&mut *session.trade_transport()).map_err(link_trade_err_to_transport);
                if result.is_ok() && need == FlowNeed::CancelTradeAndLeave { driver.leave_trade(); }
                result
            }
            FlowNeed::ConfirmTrade => {
                let Some(session) = self.link_session.as_mut() else {
                    return;
                };
                let Some(driver) = self.link_trade.as_mut() else {
                    return;
                };
                driver
                    .confirm_trade(&mut *session.trade_transport())
                    .map_err(link_trade_err_to_transport)
            }
        };
        match result {
            Ok(()) => self.link_cable.on_need_done(&need),
            Err(e) => {
                dbg_eprintln!("[link] flow action failed: {}", e);
                self.link_cable
                    .on_session_error(format!("link error: {}", e));
            }
        }
    }

    /// Enter the link battle: the CORE driver already built the battle
    /// screen (both parties exchanged, shared RNG stream, link mode on).
    /// Here the app pushes the save-derived fields the driver cannot know,
    /// mirrors the screen into `self.battle` for the render/vfx/audio/settle
    /// machinery, and starts the battle music.
    #[cfg(not(target_os = "none"))]
    fn start_link_battle(&mut self) {
        use pokered_data::trainer_data::TrainerClass;

        let Some(driver) = self.link_battle.as_mut() else {
            return;
        };
        if driver.screen().is_none() {
            dbg_eprintln!("[link] battle started without a battle screen");
            self.link_cable
                .on_session_error("link error: battle screen missing".into());
            return;
        }
        // Save-derived screen fields the driver cannot know. The original
        // fights under the RIVAL1 trainer class (`wCurOpponent = OPP_RIVAL1`,
        // engine/link/cable_club.asm:280-287) — it drives the intro trainer
        // sprite and the end-of-battle music lookups.
        if let Some(screen) = driver.screen_mut() {
            screen.player_money = self.save_data.game_data.player_money;
            screen.player_badges = self.save_data.game_data.obtained_badges;
            screen.player_id = self.save_data.game_data.player_id;
            screen.map_id = self.overworld.state.current_map as u8;
            screen.player_bag = self.save_data.game_data.bag.clone();
            screen.trainer_class = Some(TrainerClass::Rival1);
        }
        // Mirror the canonical screen into `self.battle` (per-frame updates
        // happen in the Battle arm).
        if let Some(screen) = driver.screen() {
            self.battle = screen.clone();
        }
        self.battle_vfx = BattleVisualEffects::default();
        self.battle_prev_message = None;
        self.faint_thud_pending = false;

        if let Some(ref audio) = self.audio {
            if let Some(id) = MusicId::from_u8(self.battle.battle_music_id()) {
                audio.play_music(id);
            }
        }
        self.link_cable.on_battle_started();
    }

    /// Start the trade cutscene for a completed link exchange
    /// (`TradeExecute`): the animation plays via the app's `trade_anim`
    /// machinery; the driver applies the exchange when it finishes.
    #[cfg(not(target_os = "none"))]
    fn start_link_trade_anim(&mut self) {
        use pokered_core::trade::TradeAnim;
        let is_zh = matches!(
            self.state.config.language,
            pokered_core::game_state::Lang::Zh
        );
        let Some(driver) = self.link_trade.as_ref() else {
            return;
        };
        let give = driver
            .given_mon()
            .map(|m| m.species)
            .unwrap_or(pokered_data::species::Species::Pikachu);
        let receive = driver
            .received_mon()
            .map(|m| m.species)
            .unwrap_or(pokered_data::species::Species::Pikachu);
        // TradeParty carries the same peer name displayed by the party
        // selection screen (wLinkEnemyTrainerName in the original movie).
        self.trade_anim = Some(TradeAnim::new(
            give,
            receive,
            self.player_name.clone(),
            is_zh,
        ).with_partner_name(driver.remote_name().to_string()));
        self.link_cable.on_trade_anim_started();
    }

    /// Apply a completed link trade to the save via the CORE trade driver:
    /// remove-then-add (Gen 1 has no last-mon guard — `RemovePokemon` then
    /// `AddEnemyMonToPlayerParty`, engine/link/cable_club.asm:800-817), the
    /// traded/obedience flag against our ID, Pokédex owned+seen, and the
    /// forced trade evolution detection (`TryEvolvingMon`, cable_club.asm:
    /// 851 — preserves wForceEvolution, so B cannot cancel this cutscene).
    #[cfg(not(target_os = "none"))]
    fn apply_link_trade(&mut self) {
        use pokered_core::evolution_screen::EvolutionScreenState;

        // Driver calls first (they borrow disjoint fields of `self`); the
        // pending evolution and the exchanged party come out owned.
        let (pending, new_party) = {
            let Some(driver) = self.link_trade.as_mut() else {
                return;
            };
            let pending = match driver.apply_exchange(&mut self.save_data.game_data.pokedex) {
                Ok(pending) => pending,
                Err(e) => {
                    dbg_eprintln!("[link] trade exchange failed: {}", e);
                    None
                }
            };
            (pending, driver.party().clone())
        };
        if let Some(p) = pending {
            // Keep the driver's received-mon name and force flag. Rebuilding
            // a level-up event reads the old party and enables B cancellation.
            let is_zh = matches!(self.state.config.language, pokered_core::game_state::Lang::Zh);
            self.evolution_anim = Some(EvolutionScreenState::new(vec![p], None, is_zh));
        }
        // The driver's working party IS the save's new party (it was
        // snapshotted at the table and mutated by the exchange).
        self.save_data.party = new_party;
        self.overworld.party_count = self.save_data.party.count() as u8;
        self.overworld.box_count = self.save_data.current_box.count() as u8;
        self.overworld.party_lead_level = self.save_data.party.leader_level();
        if self.evolution_anim.is_none() {
            self.finish_link_trade();
        }
    }

    /// SavePartyAndDexData runs after the movie, evolution and move learning.
    /// It preserves the receptionist's committed Pokemon Center position.
    #[cfg(not(target_os = "none"))]
    fn finish_link_trade(&mut self) {
        if let (Some(driver), Some(session)) = (self.link_trade.as_mut(), self.link_session.as_mut()) {
            if let Err(error) = driver.finish_presentation(&mut *session.trade_transport()) {
                self.link_cable.on_session_error(format!("link error: {error}"));
                return;
            }
        }
        self.link_cable.on_trade_anim_done();
    }

    #[cfg(not(target_os = "none"))]
    fn save_link_party_and_dex(&mut self) {
        if self.external_saves {
            let Some(raw) = self.committed_save.as_deref() else { return; };
            let Ok(mut saved) = serde_json::from_str::<MobileSave>(raw) else { return; };
            saved.data.party = self.save_data.party.clone();
            saved.data.game_data.pokedex = self.save_data.game_data.pokedex.clone();
            if let Ok(raw) = serde_json::to_string(&saved) {
                self.committed_save = Some(raw);
            }
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            use pokered_core::save::{sram_layout::*, ser_pokemon::serialize_party_into};
            let path = self.save_path.clone().unwrap_or_else(save_file_path);
            let Ok(mut sram) = std::fs::read(&path) else { return; };
            let Ok(previous) = import_sram(&sram) else { return; };
            // Reception performs a canonical full save before room entry.
            if previous.imported_legacy_native { return; }
            let mut main_data = Vec::new();
            self.save_data.game_data.serialize_into(&mut main_data);
            let party_at = SRAM_BANK_SIZE_LAYOUT + MAIN_DATA_OFFSET + main_data.len() + SPRITE_DATA_REGION_SIZE;
            let dex_at = SRAM_BANK_SIZE_LAYOUT + MAIN_DATA_OFFSET;
            let owned = self.save_data.game_data.pokedex.owned_flags();
            let seen = self.save_data.game_data.pokedex.seen_flags();
            let mut party = Vec::new();
            serialize_party_into(&self.save_data.party, &mut party);
            let game_at = SRAM_BANK_SIZE_LAYOUT + GAME_DATA_OFFSET;
            let checksum_at = game_at + self.save_data.serialize_checksummed_region().len();
            if sram.len() <= checksum_at || party_at + party.len() > checksum_at { return; }
            sram[dex_at..dex_at + owned.len()].copy_from_slice(owned);
            sram[dex_at + owned.len()..dex_at + owned.len() + seen.len()].copy_from_slice(seen);
            sram[party_at..party_at + party.len()].copy_from_slice(&party);
            sram[checksum_at] = pokered_core::save_menu::calc_checksum(&sram[game_at..checksum_at]);
            if let Err(error) = std::fs::write(&path, &sram) {
                log::error!("failed to save traded party: {}", error);
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let Some(storage) = web_local_storage() else { return; };
            let Ok(Some(raw)) = storage.get_item(WEB_SAVE_STORAGE_KEY) else { return; };
            let Ok(mut saved) = decode_web_save(&raw) else { return; };
            saved.party = self.save_data.party.clone();
            saved.game_data.pokedex = self.save_data.game_data.pokedex.clone();
            if let Ok(raw) = serde_json::to_string(&saved) {
                if let Err(error) = storage.set_item(WEB_SAVE_STORAGE_KEY, &raw) {
                    log::error!("failed to save traded party: {:?}", error);
                }
            }
        }
    }

    /// Warp the player into a Cable Club room (used by the link CLI after a
    /// connection is established; the original warps into the room via
    /// `SpecialEnterMap` after the receptionist handshake).
    #[cfg(not(target_os = "none"))]
    pub fn warp_to_cable_room(&mut self, map: MapId) {
        let (x, y) = pokered_core::link::cable_room_entry(self.link_role);
        self.overworld.pending_warp = Some(pokered_core::overworld::PendingWarp {
            dest_map: map,
            dest_x: x as u8,
            dest_y: y as u8,
            save_last_map: false,
            // Cable Club entry: plain fade-in, no EnterMapAnim spin.
            arrival_spin: false,
        });
        self.overworld.warp_fade_state=pokered_core::overworld::WarpFadeState::FadingOut {
            frames_remaining:pokered_core::overworld::WARP_FADE_OUT_FRAMES,
        };
    }

    /// Drain script-requested bag/money mutations and apply them to the
    /// persistent game data (the overworld is pure logic and cannot reach
    /// SaveData itself). Runs every frame from the overworld update path —
    /// and once before `agent_save_state` captures, so no committed effect
    /// is lost sitting in the queue (agent M5).
    pub(crate) fn apply_overworld_game_data_requests(&mut self) {
// Drain script-requested bag/money mutations and apply them
                    // to the persistent game data (the overworld is pure logic
                    // and cannot reach SaveData itself).
                    // A failed tradePokemon resumes the suspended script AFTER
                    // the drain (the drain borrows self.overworld).
                    let mut trade_rejected = false;
                    // Drained into a local first so request handlers can take
                    // `&mut self.overworld` (the poison tick mutates the
                    // screen's pending dialogue / warp state).
                    let game_data_requests: Vec<_> =
                        self.overworld.game_data_requests.drain(..).collect();
                    for req in game_data_requests {
                        match req {
                            OverworldGameDataRequest::GiveItem { item, quantity } => {
                                if let Some(id) =
                                    pokered_data::items::ItemId::from_const_name(&item)
                                {
                                    let _ = self.save_data.game_data.bag.add_item(id, quantity);
                                } else {
                                    log::warn!("giveItem: unknown item const '{}'", item);
                                }
                            }
                            OverworldGameDataRequest::TakeItem { item, quantity } => {
                                if let Some(id) =
                                    pokered_data::items::ItemId::from_const_name(&item)
                                {
                                    let _ = self.save_data.game_data.bag.remove_item(id, quantity);
                                } else {
                                    log::warn!("takeItem: unknown item const '{}'", item);
                                }
                            }
                            OverworldGameDataRequest::GiveMoney { amount } => {
                                self.save_data.game_data.player_money = self
                                    .save_data
                                    .game_data
                                    .player_money
                                    .saturating_add(amount)
                                    .min(999_999);
                            }
                            OverworldGameDataRequest::TakeMoney { amount } => {
                                self.save_data.game_data.player_money = self
                                    .save_data
                                    .game_data
                                    .player_money
                                    .saturating_sub(amount);
                            }
                            OverworldGameDataRequest::GiveBadge { badge } => {
                                self.save_data.game_data.set_badge(badge);
                            }
                            OverworldGameDataRequest::MarkTownVisited { map } => {
                                self.save_data.game_data.mark_town_visited(map);
                            }
                            // SetLastBlackoutMap (engine/events/set_blackout_map.asm):
                            // a script-driven heal records the blackout/Teleport
                            // target.
                            OverworldGameDataRequest::SetBlackoutMap { map } => {
                                self.save_data.game_data.last_blackout_map = map as u8;
                            }
                            OverworldGameDataRequest::TradePokemon {
                                offered,
                                received,
                                nickname,
                            } => {
                                self.overworld.set_flag_live("NPC_TRADE_CANCELLED", false);
                                use pokered_data::species::Species;
                                use pokered_data::trades::find_npc_trade;
                                let pair = (
                                    Species::from_scene_name(&offered),
                                    Species::from_scene_name(&received),
                                );
                                // The script is suspended on this await; it
                                // resumes via resume_script_after_trade.
                                let ok = if let (Some(off_sp), Some(rec_sp)) = pair {
                                    self.trade_anim.is_none()
                                        && {
                                            // Nickname: the TradeMons table is
                                            // authoritative (the original stores
                                            // it per-trade); the script arg is
                                            // the fallback for non-table pairs.
                                            let nick = find_npc_trade(off_sp, rec_sp)
                                                .map(|t| t.nickname.to_string())
                                                .unwrap_or(nickname);
                                            self.pending_trade = Some(PendingTrade {
                                                party_index: 0,
                                                ready_to_animate: false,
                                                give: off_sp,
                                                receive: rec_sp,
                                                nickname: nick,
                                            });
                                            self.overworld.begin_party_select(self.save_data.party.to_vec());
                                            true
                                        }
                                } else {
                                    false
                                };
                                if !ok {
                                    // Offered mon not in the party (or bad
                                    // species): the scene takes its no-trade
                                    // branch, no cutscene. Deferred to after
                                    // the drain loop (borrow conflict).
                                    trade_rejected = true;
                                }
                            }
                            OverworldGameDataRequest::GiveCoins { amount } => {
                                self.save_data.game_data.give_coins(amount);
                                if self.overworld.script_coin_box.is_some() {
                                    self.overworld.script_coin_box = Some(self.save_data.game_data.player_coins);
                                }
                            }
                            OverworldGameDataRequest::TakeCoins { amount } => {
                                self.save_data.game_data.take_coins(amount);
                                if self.overworld.script_coin_box.is_some() {
                                    self.overworld.script_coin_box = Some(self.save_data.game_data.player_coins);
                                }
                            }
                            OverworldGameDataRequest::TickDaycareExp => {
                                self.save_data.game_data.tick_daycare_exp();
                            }
                            OverworldGameDataRequest::PoisonStep => {
                                pokered_core::overworld::poison::apply_out_of_battle_poison_damage(
                                    &mut self.save_data,
                                    &mut self.overworld,
                                );
                            }
                            OverworldGameDataRequest::DepositDaycare { index } => {
                                self.save_data.deposit_daycare(index);
                                self.overworld.party_count =
                                    self.save_data.party.count() as u8;
                                self.overworld.party_lead_level =
                                    self.save_data.party.leader_level();
                            }
                            OverworldGameDataRequest::WithdrawDaycare => {
                                self.save_data.withdraw_daycare();
                                self.overworld.party_count =
                                    self.save_data.party.count() as u8;
                                self.overworld.party_lead_level =
                                    self.save_data.party.leader_level();
                            }
                        }
                    }
                    if trade_rejected {
                        self.overworld.resume_script_after_trade(false);
                    }
    }

    /// Full structured state snapshot for the debug protocol's `get_state`
    /// (and the payload of `wait_until` / `skip_dialogue` responses).
    #[cfg(feature = "debug-server")]
    fn debug_state_snapshot(&self) -> serde_json::Value {
        let map_id = self.overworld.state.current_map;
        // Current overworld dialogue text (script @speaker / talk), so a
        // driver can observe interactions that don't change `screen`.
        let dialogue = self
            .overworld
            .displayed_field_dialogue()
            .and_then(|d| d.get_display_text())
            .map(|(a, b)| format!("{} {}", a, b).trim().to_string());
        // Structured dialogue-machine state: page progress, typewriter
        // position, and whether the current page is fully revealed and
        // waiting for A — the driver can align A presses exactly.
        let dialogue_state = self.overworld.pending_dialogue.as_ref().map(|d| {
            let (a, b) = d.get_display_text().unwrap_or_default();
            serde_json::json!({
                "text": format!("{} {}", a, b).trim(),
                "page": d.current_page() + 1,
                "total_pages": d.pages().len(),
                "char_index": d.char_index(),
                "total_chars": d.total_chars(),
                "waiting_for_input": d.waiting_for_input(),
                "holding_open": d.holding_open(),
                "done": d.is_done(),
            })
        });
        // Choice-menu cursor (script `@choice`), so a driver can move the
        // highlighted option without guessing.
        let choice = self
            .overworld
            .pending_choice
            .as_ref()
            .map(|c| serde_json::json!({ "options": c.options, "selected": c.selected }));
        let mut snapshot = serde_json::json!({
            "screen": crate::cli::screen_name(&self.state.screen).to_string(),
            "map_id": map_id as u8,
            "map_name": format!("{:?}", map_id),
            "last_outside_map": self.overworld.last_map.map(|map| format!("{map:?}")),
            "map_blocks": self.overworld.map_data.as_ref().map(|map| &map.blocks),
            "player_x": self.overworld.state.player.x,
            "player_y": self.overworld.state.player.y,
            "player_facing": format!("{:?}", self.overworld.state.player.facing),
            "player_transport": format!("{:?}", self.overworld.state.player.transport),
            "player_name": self.player_name.clone(),
            "frame_count": self.frame_count,
            "party_count": self.overworld.party_count,
            "badges": self.save_data.game_data.obtained_badges,
            "hall_of_fame_count": self.save_data.hall_of_fame.team_count(),
            "hof_phase": self.hof_ceremony.as_ref().map(|hof| format!("{:?}", hof.phase())),
            "credits_phase": self.credits.as_ref().map(|credits| format!("{:?}", credits.phase())),
            "credits_final_button": self.credits.as_ref().map(|credits| credits.awaiting_final_button()),
            // Full party roster (species/level/HP/moves/PP) so a driver
            // can plan healing, training and switch strategy offline.
            "party": self
                .save_data
                .party
                .iter()
                .map(|mon| {
                    serde_json::json!({
                        "species": format!("{:?}", mon.species),
                        "level": mon.level,
                        "hp": mon.hp,
                        "max_hp": mon.max_hp,
                        "status": format!("{:?}", mon.status),
                        "moves": mon.moves.iter()
                            .map(|m| format!("{:?}", m))
                            .collect::<Vec<_>>(),
                        "pp": mon.pp,
                    })
                })
                .collect::<Vec<_>>(),
            "dialogue": dialogue,
            "dialogue_state": dialogue_state,
            "choice": choice,
            // Link (Cable Club) session and in-room flow phase. The flow's
            // modal boxes (Just a moment. / prompts / trade select) are not
            // part of the overworld dialogue/choice machinery, so a driver
            // needs this to observe the link trade/battle flow.
            "link": serde_json::json!({
                "status": match &self.link_status {
                    LinkStatus::Disabled => "disabled".to_string(),
                    LinkStatus::WaitingForPeer => "waiting_for_peer".to_string(),
                    LinkStatus::Connecting => "connecting".to_string(),
                    LinkStatus::Connected => "connected".to_string(),
                    LinkStatus::Disconnected(reason) => format!("disconnected: {reason}"),
                },
                "role": format!("{:?}", self.link_role),
                "cable_phase": format!("{:?}", self.link_cable.phase()),
                "trade": self.link_trade.as_ref().map(|d| serde_json::json!({
                    "state": format!("{:?}", d.state()),
                    "given": d.given_mon().map(|m| format!("{:?}", m.species)),
                    "received": d.received_mon().map(|m| format!("{:?}", m.species)),
                })),
            }),
            // True while a storyline script owns the game (cutscene in
            // progress), false once control is back with the player.
            "script_running": !self.overworld.script_engine_idle(),
            // Current battle text box message (e.g. the post-victory
            // EndBattleText quip), so a driver can observe battle flow.
            "battle_message": self.battle.current_message.clone(),
            // Current battle phase (Debug form), e.g. "PlayerMenu",
            // "BagSelect", so a driver knows when a menu is ready.
            "battle_phase": format!("{:?}", self.battle.phase),
            "battle_party_cursor": self.battle.party_cursor,
            // Simulation HP, rather than the save snapshot or animated HUD.
            "battle_live": self.battle.battle_state.as_ref().map(|bs| {
                let player = bs.player.active_mon();
                let enemy = bs.enemy.active_mon();
                serde_json::json!({
                    "is_ghost": self.battle.is_ghost,
                    "player_party": bs.player.party.iter().map(|mon| serde_json::json!({
                        "species": format!("{:?}", mon.species), "level": mon.level,
                        "hp": mon.hp, "max_hp": mon.max_hp,
                    })).collect::<Vec<_>>(),
                    "player": { "species": format!("{:?}", player.species), "level": player.level, "hp": player.hp,
                        "max_hp": player.max_hp, "status": format!("{:?}", player.status) },
                    "enemy": { "species": format!("{:?}", enemy.species), "level": enemy.level, "hp": enemy.hp,
                        "max_hp": enemy.max_hp, "status": format!("{:?}", enemy.status) },
                    "enemy_party": bs.enemy.party.iter().map(|mon| serde_json::json!({
                        "species": format!("{:?}", mon.species), "level": mon.level, "hp": mon.hp,
                    })).collect::<Vec<_>>(),
                })
            }),
            "battle_bag": self.battle.bag_menu.as_ref().map(|bag| serde_json::json!({
                "cursor": bag.cursor(),
                "items": bag.items().iter().map(|(id, qty)| serde_json::json!({
                    "item": format!("{:?}", id), "qty": qty,
                })).collect::<Vec<_>>(),
            })),
            "battle_inventory": self.battle.player_bag.items().iter().map(|(id, qty)| {
                serde_json::json!({ "item": format!("{:?}", id), "qty": qty })
            }).collect::<Vec<_>>(),
            "shop_phase": match &self.state.screen {
                GameScreen::Shop(mart) => Some(format!("{:?}", mart.phase)),
                _ => None,
            },
            // Read-only menu observations for real-input HM/item/field-move
            // playthroughs. Expose the active menu only, never a stale cursor.
            "field_menu": match &self.state.screen {
                GameScreen::StartMenu => Some(serde_json::json!({
                    "kind": "start", "cursor": self.start_menu.cursor(),
                    "input_ready": !self.start_menu.field_initialization_active(),
                    "items": self.start_menu.items().iter().map(|item| format!("{:?}", item)).collect::<Vec<_>>(),
                })),
                GameScreen::Bag => Some(serde_json::json!({
                    "kind": "bag", "cursor": self.bag_screen.cursor(),
                    "phase": format!("{:?}", self.bag_screen.phase()),
                    "items": self.bag_screen.items().iter().map(|(id, qty)| serde_json::json!({
                        "item": format!("{:?}", id), "qty": qty,
                    })).collect::<Vec<_>>(),
                })),
                GameScreen::PartyScreen => Some(serde_json::json!({
                    "kind": "party", "cursor": self.party_screen.cursor(),
                    "phase": format!("{:?}", self.party_screen.phase()),
                    "mode": format!("{:?}", self.party_screen.mode()),
                    "field_moves": self.party_screen.selected_field_moves().iter().map(|m| format!("{:?}", m)).collect::<Vec<_>>(),
                    "known_moves": self.party_screen.selected_known_moves().iter().map(|m| format!("{:?}", m)).collect::<Vec<_>>(),
                })),
                GameScreen::Elevator => self.elevator_screen.as_ref().map(|lift| serde_json::json!({
                    "kind": "elevator", "cursor": lift.selected_index(), "items": lift.floors(),
                })),
                GameScreen::TownMap => Some(serde_json::json!({
                    "kind": "town_map", "cursor": self.town_map_screen.cursor(),
                    "mode": format!("{:?}", self.town_map_screen.mode()),
                    "selected_map": format!("{:?}", self.town_map_screen.selected_map()),
                })),
                _ => None,
            },
            // Live move menu while it is open (FIGHT selection): cursor and
            // per-slot PP. The save-data party is a battle-start snapshot —
            // mid-battle PP drain only exists here, so a closed-loop driver
            // must read PP (and the cursor) from this menu.
            "battle_moves": self.battle.move_menu.as_ref().map(|mm| {
                serde_json::json!({
                    "cursor": mm.cursor(),
                    "moves": mm.moves().iter().map(|m| {
                        serde_json::json!({
                            "move": format!("{:?}", m.move_id),
                            "pp": m.current_pp,
                            "disabled": m.is_disabled,
                        })
                    }).collect::<Vec<_>>(),
                })
            }),
            "money": self.save_data.game_data.player_money,
            "coins": self.save_data.game_data.player_coins,
            // Current PC-screen phase (Debug form), so a driver can
            // observe storage-system navigation.
            "pc_phase": self
                .pc_screen
                .as_ref()
                .map(|pc| format!("{:?}", pc.phase())),
            // Script-effect currently being processed (e.g.
            // "ShowDialogue", "FollowNpc"), null when idle — lets a
            // driver follow cutscene progress deterministically.
            "active_script_effect": self.overworld.active_script_effect_label(),
            // Full script-effect payload with progress fields (Delay
            // countdown, move-path state, FollowNpc phase, choice
            // cursor, …), or null when idle.
            "script_effect": self.overworld.active_script_effect_value(),
            // True while a storyline is suspended on startBattle/
            // startWildBattle — a driver must play out the battle
            // before the script resumes.
            "script_awaiting_battle": self.overworld.script_awaiting_battle,
            "player_movement_state": format!("{:?}", self.overworld.state.player.movement_state),
            // Configured typewriter speed (chars-per-frame pacing), so a
            // driver can estimate text reveal time offline.
            "text_speed_delay_frames": self.state.config.text_speed.delay_frames(),
            // Warp transition state ("Idle"/"FadingOut { .. }"/…), so a
            // driver knows when a warp is still settling.
            "warp_fade": format!("{:?}", self.overworld.warp_fade_state),
        });
        // Keep this separate from the large snapshot macro's recursion budget.
        let live = self.battle.battle_state.as_ref()
            .filter(|_| matches!(self.state.screen, GameScreen::Battle));
        let party: Vec<_> = if let Some(bs) = live {
            bs.player.party.iter().collect()
        } else {
            self.save_data.party.iter().collect()
        };
        snapshot["evaluation"] = serde_json::json!({
            "party_source": if live.is_some() { "battle_live" } else { "save_data" },
            "party": party.iter().map(|mon| serde_json::json!({
                "species": format!("{:?}", mon.species), "level": mon.level,
                "hp": mon.hp, "max_hp": mon.max_hp, "total_exp": mon.total_exp,
                "status": format!("{:?}", mon.status), "pp": mon.pp,
                "moves": mon.moves.iter().map(|m| format!("{:?}", m)).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "pokedex": {
                "seen": self.save_data.game_data.pokedex.seen_count(),
                "owned": self.save_data.game_data.pokedex.owned_count(),
                "total": pokered_core::pokemon::pokedex::NUM_POKEMON,
                "owned_numbers": (1u8..=151).filter(|id| self.save_data.game_data.pokedex
                    .is_owned(pokered_data::species::Species::from_index_id(*id))).collect::<Vec<_>>(),
            },
        });
        snapshot
    }

    /// Typed semantic observation snapshot (the `pokered-agent` M1 layer),
    /// built from the same live sources as [`Self::debug_state_snapshot`].
    /// Pure observation: reads state, never steps frames. Not gated on
    /// `debug-server` — the observation logic is pure and reusable; only
    /// the debug-command surface is feature-gated. Hosted-only: the
    /// pokered-agent layer is excluded from bare-metal (GBA) builds.
    #[cfg(not(target_os = "none"))]
    pub fn agent_snapshot(
        &self,
        profile: &pokered_agent::ObservationProfile,
    ) -> pokered_agent::AgentSnapshot {
        use pokered_agent::{ObservationSource, OverworldObs};
        let map_id = self.overworld.state.current_map;
        let bag = self.save_data.game_data.bag.items();
        let hidden = pokered_agent::hidden_item_spots(map_id, self.overworld.hidden_item_flags());
        let in_battle = matches!(self.state.screen, GameScreen::Battle);
        let map_handle = pokered_data::map_data_loader::get_map_json(map_id);
        let src = ObservationSource {
            screen: &self.state.screen,
            map_id,
            player_x: self.overworld.state.player.x,
            player_y: self.overworld.state.player.y,
            player_facing: self.overworld.state.player.facing,
            overworld: OverworldObs {
                dialogue_open: self.overworld.pending_dialogue.is_some(),
                choice_open: self.overworld.pending_choice.is_some(),
                script_running: !self.overworld.script_engine_idle()
                    || self.overworld.active_script_effect_label().is_some()
                    || self.overworld.script_awaiting_battle,
                warp_transition: self.overworld.pending_warp.is_some()
                    || !matches!(
                        self.overworld.warp_fade_state,
                        pokered_core::overworld::WarpFadeState::Idle
                    ),
                trainer_encounter_pending: self.overworld.trainer_encounter_pending(),
            },
            battle_phase: in_battle.then_some(&self.battle.phase),
            party: &self.save_data.party,
            bag: &bag,
            obtained_badges: self.save_data.game_data.obtained_badges,
            dialogue: self.overworld.pending_dialogue.as_ref(),
            choice: self.overworld.pending_choice.as_ref(),
            battle: in_battle.then_some(&self.battle),
            npc_states: &self.overworld.npc_states,
            map_json: map_handle.as_ref().map(|h| &**h),
            hidden_items: &hidden,
            nearby_radius: profile.nearby_radius,
        };
        pokered_agent::build_agent_snapshot(&src, profile)
    }

    /// Entities near the player within `radius` step units, nearest first
    /// (same aggregation as the snapshot's `nearby` section).
    #[cfg(not(target_os = "none"))]
    pub fn agent_nearby(&self, radius: i32) -> Vec<pokered_agent::NearbyEntity> {
        let map_id = self.overworld.state.current_map;
        let player = pokered_agent::Position {
            x: self.overworld.state.player.x as i32,
            y: self.overworld.state.player.y as i32,
        };
        let npcs: Vec<pokered_agent::NpcObs> = self
            .overworld
            .npc_states
            .iter()
            .map(pokered_agent::NpcObs::from)
            .collect();
        let hidden = pokered_agent::hidden_item_spots(map_id, self.overworld.hidden_item_flags());
        let map_handle = pokered_data::map_data_loader::get_map_json(map_id);
        pokered_agent::nearby_entities(
            player,
            &npcs,
            map_handle.as_ref().map(|h| &**h),
            &hidden,
            radius,
        )
    }

    /// Predicate used by the debug `wait_until` command. Returns whether
    /// the named condition currently holds. Named conditions are the
    /// driver-facing vocabulary; `screen=` / `battle_phase=` /
    /// `script_effect=` compare against Debug variant names.
    #[cfg(feature = "debug-server")]
    fn debug_condition_met(&self, condition: &str) -> bool {
        match condition {
            "dialogue_done" => self.overworld.pending_dialogue.is_none(),
            "dialogue_ready" => self
                .overworld
                .pending_dialogue
                .as_ref()
                .map_or(false, |d| d.waiting_for_input()),
            "choice_open" => self.overworld.pending_choice.is_some(),
            "choice_closed" => self.overworld.pending_choice.is_none(),
            "script_idle" => self.overworld.script_engine_idle(),
            "not_battle" => self.state.screen != pokered_core::game_state::GameScreen::Battle,
            // Player control back after a cutscene: overworld, no dialogue /
            // choice / script effect, script engine idle, warp settled,
            // no pending automatic door step, and no battle suspended on the script.
            "control_ready" => {
                crate::cli::screen_name(&self.state.screen) == "overworld"
                    && self.overworld.pending_dialogue.is_none()
                    && self.overworld.pending_choice.is_none()
                    && self.overworld.active_script_effect_value().is_none()
                    && self.overworld.script_engine_idle()
                    && !self.overworld.boulder_blocks_control()
                    && self.overworld.pending_warp.is_none()
                    && matches!(
                        self.overworld.warp_fade_state,
                        pokered_core::overworld::WarpFadeState::Idle
                    )
                    && !self.overworld.state.standing_on_door
                    && !self.overworld.state.exiting_door
                    && !self.overworld.script_awaiting_battle
            }
            other => {
                if let Some(name) = other.strip_prefix("screen=") {
                    crate::cli::screen_name(&self.state.screen) == name
                } else if let Some(name) = other.strip_prefix("battle_phase=") {
                    format!("{:?}", self.battle.phase) == name
                } else if let Some(name) = other.strip_prefix("script_effect=") {
                    self.overworld.active_script_effect_label().as_deref() == Some(name)
                } else {
                    false
                }
            }
        }
    }

    /// Whether `condition` is a recognized `wait_until` condition (named
    /// predicate or a known `key=value` prefix form). Unknown names are
    /// rejected up front so a typo fails fast instead of silently burning
    /// the whole frame budget and reporting `reached: false`.
    #[cfg(feature = "debug-server")]
    fn debug_condition_known(condition: &str) -> bool {
        matches!(
            condition,
            "dialogue_done"
                | "dialogue_ready"
                | "choice_open"
                | "choice_closed"
                | "script_idle"
                | "not_battle"
                | "control_ready"
        ) || condition.starts_with("screen=")
            || condition.starts_with("battle_phase=")
            || condition.starts_with("script_effect=")
    }

    #[cfg(feature = "debug-server")]
    fn handle_debug_command(
        &mut self,
        cmd: pokered_debug_server::DebugCommand,
    ) -> pokered_debug_server::DebugResponse {
        use pokered_debug_server::{
            CoreDebugCommand, DebugCommand, DebugResponse, GameDebugCommand,
        };

        match cmd {
            DebugCommand::Core(CoreDebugCommand::GetState) => {
                DebugResponse::ok_with_data(self.debug_state_snapshot())
            }
            DebugCommand::Game(GameDebugCommand::GetAgentState { level, profile }) => {
                // Validate params up front: `level` (1-4) selects a canned
                // profile, `profile` supplies one verbatim; both is a
                // caller error. No frames are stepped — pure observation.
                let profile = match (level, profile) {
                    (Some(_), Some(_)) => {
                        return DebugResponse::err(
                            "pass either `level` or `profile`, not both".to_string(),
                        )
                    }
                    (Some(level), None) => {
                        match pokered_agent::ObservationLevel::from_u8(level) {
                            Some(level) => pokered_agent::ObservationProfile::for_level(level),
                            None => {
                                return DebugResponse::err(format!(
                                    "unknown observation level: {level} (expected 1-4)"
                                ))
                            }
                        }
                    }
                    (None, Some(profile)) => {
                        match serde_json::from_value::<pokered_agent::ObservationProfile>(profile)
                        {
                            Ok(profile) => profile,
                            Err(err) => {
                                return DebugResponse::err(format!("invalid profile: {err}"))
                            }
                        }
                    }
                    (None, None) => pokered_agent::ObservationProfile::default(),
                };
                DebugResponse::ok_with_data(
                    serde_json::to_value(self.agent_snapshot(&profile)).unwrap_or_default(),
                )
            }
            DebugCommand::Game(GameDebugCommand::GetNearby { radius }) => {
                let radius = radius
                    .unwrap_or(pokered_agent::DEFAULT_NEARBY_RADIUS as u32)
                    as i32;
                let map_id = self.overworld.state.current_map;
                let entities = self.agent_nearby(radius);
                DebugResponse::ok_with_data(serde_json::json!({
                    "map": { "id": map_id as u8, "name": format!("{:?}", map_id) },
                    "position": {
                        "x": self.overworld.state.player.x,
                        "y": self.overworld.state.player.y,
                    },
                    "radius": radius,
                    "entities": entities,
                }))
            }
            DebugCommand::Game(GameDebugCommand::MoveTo { x, y }) => {
                // Synchronous closed-loop walk (see agent_nav.rs): the
                // outcome plus fresh snapshots, same convention as
                // wait_until. Param bounds are inherent (u16); map-bounds
                // and reachability report as `blocked` outcomes.
                let outcome = self.agent_move_to(x, y);
                let mut data = serde_json::to_value(&outcome).unwrap_or_default();
                if let Some(obj) = data.as_object_mut() {
                    obj.insert(
                        "map".to_string(),
                        serde_json::json!(format!("{:?}", self.overworld.state.current_map)),
                    );
                    obj.insert("state".to_string(), self.debug_state_snapshot());
                    obj.insert(
                        "agent_state".to_string(),
                        serde_json::to_value(
                            self.agent_snapshot(&pokered_agent::ObservationProfile::default()),
                        )
                        .unwrap_or_default(),
                    );
                }
                DebugResponse::ok_with_data(data)
            }
            DebugCommand::Game(GameDebugCommand::Interact) => {
                let outcome = self.agent_interact();
                let mut data = serde_json::to_value(&outcome).unwrap_or_default();
                if let Some(obj) = data.as_object_mut() {
                    obj.insert(
                        "map".to_string(),
                        serde_json::json!(format!("{:?}", self.overworld.state.current_map)),
                    );
                    obj.insert("state".to_string(), self.debug_state_snapshot());
                    obj.insert(
                        "agent_state".to_string(),
                        serde_json::to_value(
                            self.agent_snapshot(&pokered_agent::ObservationProfile::default()),
                        )
                        .unwrap_or_default(),
                    );
                }
                DebugResponse::ok_with_data(data)
            }
            DebugCommand::Game(GameDebugCommand::InteractWith { ref id }) => {
                let outcome = self.agent_interact_with(id);
                let mut data = serde_json::to_value(&outcome).unwrap_or_default();
                if let Some(obj) = data.as_object_mut() {
                    obj.insert(
                        "map".to_string(),
                        serde_json::json!(format!("{:?}", self.overworld.state.current_map)),
                    );
                    obj.insert("state".to_string(), self.debug_state_snapshot());
                    obj.insert(
                        "agent_state".to_string(),
                        serde_json::to_value(
                            self.agent_snapshot(&pokered_agent::ObservationProfile::default()),
                        )
                        .unwrap_or_default(),
                    );
                }
                DebugResponse::ok_with_data(data)
            }
            DebugCommand::Game(GameDebugCommand::GetWorldGraph { ref maps }) => {
                // Validate the scope filter up front.
                let scope: Option<Vec<pokered_data::maps::MapId>> = match maps {
                    Some(names) => {
                        let mut resolved = Vec::with_capacity(names.len());
                        for name in names {
                            match pokered_data::map_data_loader::resolve_map_id(name) {
                                Some(id) => resolved.push(id),
                                None => {
                                    return DebugResponse::err(format!(
                                        "unknown map: '{name}'"
                                    ))
                                }
                            }
                        }
                        Some(resolved)
                    }
                    None => None,
                };
                let graph = pokered_agent::WorldGraph::shared();
                let edges: Vec<&pokered_agent::WorldEdge> = match scope {
                    Some(ids) => ids
                        .iter()
                        .flat_map(|&id| graph.edges_from(id))
                        .collect(),
                    None => graph.edges().iter().collect(),
                };
                DebugResponse::ok_with_data(serde_json::json!({
                    "edge_count": edges.len(),
                    "edges": edges,
                }))
            }
            DebugCommand::Game(GameDebugCommand::FindWorldRoute { ref from, ref to }) => {
                let from_id = pokered_data::map_data_loader::resolve_map_id(from);
                let to_id = pokered_data::map_data_loader::resolve_map_id(to);
                let (from_id, to_id) = match (from_id, to_id) {
                    (Some(from_id), Some(to_id)) => (from_id, to_id),
                    (None, _) => return DebugResponse::err(format!("unknown map: '{from}'")),
                    (_, None) => return DebugResponse::err(format!("unknown map: '{to}'")),
                };
                let route = pokered_agent::WorldGraph::shared().find_route(from_id, to_id);
                DebugResponse::ok_with_data(serde_json::json!({
                    "from": format!("{:?}", from_id),
                    "to": format!("{:?}", to_id),
                    "found": route.is_some(),
                    "legs": route.unwrap_or_default(),
                }))
            }
            DebugCommand::Game(GameDebugCommand::TravelTo { ref map }) => {
                let Some(dest) = pokered_data::map_data_loader::resolve_map_id(map) else {
                    return DebugResponse::err(format!("unknown map: '{map}'"));
                };
                let outcome = self.agent_travel_to(dest);
                let mut data = serde_json::to_value(&outcome).unwrap_or_default();
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("state".to_string(), self.debug_state_snapshot());
                    obj.insert(
                        "agent_state".to_string(),
                        serde_json::to_value(
                            self.agent_snapshot(&pokered_agent::ObservationProfile::default()),
                        )
                        .unwrap_or_default(),
                    );
                }
                DebugResponse::ok_with_data(data)
            }
            DebugCommand::Game(GameDebugCommand::SetSeed { seed }) => {
                self.set_seed(seed);
                DebugResponse::ok_with_data(serde_json::json!({ "seed": seed }))
            }
            DebugCommand::Game(GameDebugCommand::SaveState { slot }) => {
                match self.agent_save_state_slot(slot) {
                    Ok(hash) => DebugResponse::ok_with_data(serde_json::json!({
                        "slot": slot,
                        "hash": hash,
                        "screen": crate::cli::screen_name(&self.state.screen),
                        "frame_count": self.frame_count,
                        "seed": self.seed,
                    })),
                    Err(err) => DebugResponse::err(err.message()),
                }
            }
            DebugCommand::Game(GameDebugCommand::RestoreState { slot }) => {
                match self.agent_restore_state_slot(slot) {
                    Ok(hash) => DebugResponse::ok_with_data(serde_json::json!({
                        "slot": slot,
                        "hash": hash,
                        "restored": true,
                        "state": self.debug_state_snapshot(),
                        "agent_state": serde_json::to_value(
                            self.agent_snapshot(&pokered_agent::ObservationProfile::default()),
                        )
                        .unwrap_or_default(),
                    })),
                    Err(err) => DebugResponse::err(err.message()),
                }
            }
            DebugCommand::Game(GameDebugCommand::GetScriptSemantics { ref map }) => {
                match map {
                    Some(name) => {
                        // Validate the map exists in world data first so a
                        // typo is a clean error, not an empty payload.
                        if pokered_data::map_data_loader::resolve_map_id(name).is_none()
                            && !name.starts_with("shared/")
                        {
                            return DebugResponse::err(format!("unknown map: '{name}'"));
                        }
                        match pokered_agent::extract_map_semantics(name) {
                            Some(semantics) => DebugResponse::ok_with_data(
                                serde_json::to_value(semantics).unwrap_or_default(),
                            ),
                            None => DebugResponse::err(format!(
                                "no scene script for map: '{name}'"
                            )),
                        }
                    }
                    None => {
                        let world = pokered_agent::generate_world_semantics();
                        let maps: Vec<&str> =
                            world.maps.iter().map(|m| m.map.as_str()).collect();
                        DebugResponse::ok_with_data(serde_json::json!({
                            "coverage": world.coverage,
                            "maps": maps,
                        }))
                    }
                }
            }
            DebugCommand::Game(GameDebugCommand::WaitUntil {
                ref condition,
                max_frames,
            }) => {
                // Reject unknown condition names up front (same treatment as
                // `press` with an unknown button) — otherwise a typo would
                // silently burn the whole budget and report reached=false.
                if !Self::debug_condition_known(condition) {
                    return DebugResponse::err(format!(
                        "unknown condition: '{}' (see wait_until docs in protocol.rs)",
                        condition
                    ));
                }
                // Synchronous condition-driven stepping: drive update() until
                // the predicate holds (checked after every frame), or the
                // budget elapses. One round trip replaces the driver's
                // poll-every-N-frames loop. Queued press/press_sequence
                // inputs are consumed one per stepped frame, as with
                // step_frames.
                let mut stepped: u32 = 0;
                while stepped < max_frames && !self.debug_condition_met(condition) {
                    let input = InputState::new();
                    self.update(&input);
                    stepped += 1;
                }
                let reached = self.debug_condition_met(condition);
                DebugResponse::ok_with_data(serde_json::json!({
                    "condition": condition,
                    "reached": reached,
                    "stepped": stepped,
                    "state": self.debug_state_snapshot(),
                }))
            }
            DebugCommand::Game(GameDebugCommand::SkipDialogue) => {
                // Engine-internal A taps: skip typing, advance every page,
                // and close the box exactly like a player pressing A through
                // it (a last-page tap starts holding-open; a release frame
                // closes it), so a script suspended on ShowDialogue resumes.
                // Release/tap frames alternate on purpose: the overworld
                // tracks `prev_a_pressed` across frames
                // (update.rs `a_just_pressed`), so consecutive tap frames
                // would read as a held button and never advance. Queued
                // press/press_sequence inputs are dropped first — inside
                // update() they override the passed-in frame and would
                // break the alternation.
                self.pending_debug_inputs.clear();
                let mut stepped: u32 = 0;
                const MAX_SKIP_FRAMES: u32 = 900;
                // First frame is a release so the very first tap is a
                // rising edge regardless of the preceding frame.
                let mut release = true;
                // A question may remain visible below a choice menu. Stop
                // when it opens: skip_dialogue must never answer it for us.
                while self.overworld.pending_dialogue.is_some()
                    && self.overworld.pending_choice.is_none()
                    && stepped < MAX_SKIP_FRAMES
                {
                    let mut input = InputState::new();
                    if !release {
                        input.press(GbButton::A);
                    }
                    self.update(&input);
                    stepped += 1;
                    release = !release;
                }
                DebugResponse::ok_with_data(serde_json::json!({
                    "stepped": stepped,
                    "dialogue_closed": self.overworld.pending_dialogue.is_none(),
                    "state": self.debug_state_snapshot(),
                }))
            }
            DebugCommand::Core(CoreDebugCommand::GetPosition) => {
                let map_id = self.overworld.state.current_map;
                let data = serde_json::json!({
                    "map_id": map_id as u8,
                    "map_name": format!("{:?}", map_id),
                    "x": self.overworld.state.player.x,
                    "y": self.overworld.state.player.y,
                    "facing": format!("{:?}", self.overworld.state.player.facing),
                });
                DebugResponse::ok_with_data(data)
            }
            DebugCommand::Game(GameDebugCommand::GetMap) => {
                let map = self.overworld.map_data.as_ref().map(|m| {
                    serde_json::json!({
                        "map_name": format!("{:?}", self.overworld.state.current_map),
                        "width": m.width, "height": m.height,
                        "tileset": format!("{:?}", m.tileset), "blocks": m.blocks,
                        "transport": format!("{:?}", self.overworld.state.player.transport),
                    })
                });
                DebugResponse::ok_with_data(serde_json::json!(map))
            }
            DebugCommand::Game(GameDebugCommand::CaptureFrame { ref path }) => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                    self.draw(&mut fb);
                    match fb.save_png(std::path::Path::new(path)) {
                        Ok(()) => DebugResponse::ok_with_data(serde_json::json!({
                            "path": path,
                            "state": self.debug_state_snapshot(),
                        })),
                        Err(err) => DebugResponse::err(format!("capture failed: {err}")),
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let _ = path;
                    DebugResponse::err("capture_frame requires a native frontend".to_string())
                }
            }
            DebugCommand::Game(GameDebugCommand::PressTimeline {
                ref buttons,
                start_at_frame,
                advance,
            }) => {
                let parsed = buttons
                    .iter()
                    .map(|button| match button.as_deref().map(str::to_lowercase) {
                        None => Ok(None),
                        Some(button) => match button.as_str() {
                            "a" => Ok(Some(GbButton::A)),
                            "b" => Ok(Some(GbButton::B)),
                            "start" => Ok(Some(GbButton::Start)),
                            "select" => Ok(Some(GbButton::Select)),
                            "up" => Ok(Some(GbButton::Up)),
                            "down" => Ok(Some(GbButton::Down)),
                            "left" => Ok(Some(GbButton::Left)),
                            "right" => Ok(Some(GbButton::Right)),
                            _ => Err(format!("unknown button: '{}'", button)),
                        },
                    })
                    .collect::<Result<Vec<_>, _>>();
                match parsed {
                    Ok(_parsed)
                        if start_at_frame.is_some_and(|frame| frame < self.frame_count) =>
                    {
                        DebugResponse::err(format!(
                            "start_at_frame {} is before current frame {}",
                            start_at_frame.unwrap(),
                            self.frame_count
                        ))
                    }
                    Ok(parsed) => {
                        if advance && !self.pending_debug_inputs.is_empty() {
                            return DebugResponse::err(
                                "synchronous timeline requires an empty input queue".to_string(),
                            );
                        }
                        let queue_start_frame = self.frame_count;
                        let start_frame = start_at_frame.unwrap_or(queue_start_frame);
                        let padding = start_frame.saturating_sub(queue_start_frame) as usize;
                        let frames = parsed.len() as u64;
                        self.pending_debug_inputs
                            .extend(std::iter::repeat_n(None, padding));
                        self.pending_debug_inputs.extend(parsed);
                        if advance {
                            let input = InputState::new();
                            for _ in 0..padding as u64 + frames {
                                self.update(&input);
                            }
                        }
                        DebugResponse::ok_with_data(serde_json::json!({
                            "queue_start_frame": queue_start_frame,
                            "start_frame": start_frame,
                            "end_frame": start_frame + frames.saturating_sub(1),
                            "frames": frames,
                            "padding_frames": padding,
                            "advanced": advance,
                            "frame_count": self.frame_count,
                        }))
                    }
                    Err(error) => DebugResponse::err(error),
                }
            }
            DebugCommand::Game(GameDebugCommand::GetParty) => {
                let party: Vec<serde_json::Value> = self
                    .save_data
                    .party
                    .to_vec()
                    .iter()
                    .map(|p| {
                        serde_json::json!({
                            "species": format!("{:?}", p.species),
                            "level": p.level,
                            "experience": p.total_exp,
                            "current_hp": p.hp,
                            "max_hp": p.max_hp,
                            "status": format!("{:?}", p.status),
                            "moves": p.moves.iter().map(|m| format!("{:?}", m)).collect::<Vec<_>>(),
                        })
                    })
                    .collect();
                DebugResponse::ok_with_data(serde_json::json!(party))
            }
            DebugCommand::Core(CoreDebugCommand::GetBag) => {
                let items: Vec<serde_json::Value> = self
                    .save_data
                    .game_data
                    .bag
                    .items()
                    .iter()
                    .map(|(id, qty)| {
                        serde_json::json!({
                            "item": format!("{:?}", id),
                            "qty": qty,
                        })
                    })
                    .collect();
                DebugResponse::ok_with_data(serde_json::json!(items))
            }
            DebugCommand::Core(CoreDebugCommand::GetFlags) => {
                // Read the LIVE overworld flag store (unified_flags), not the
                // `save_data` snapshot which is only synced at save time — so
                // flags flipped this session (e.g. via `set_flag_live`) show up.
                let flags: serde_json::Value =
                    serde_json::to_value(self.overworld.script_flags()).unwrap_or_default();
                DebugResponse::ok_with_data(flags)
            }
            DebugCommand::Core(CoreDebugCommand::Warp { ref map, x, y }) => {
                match parse_warp_arg(map) {
                    Ok((map_id, _, _)) => {
                        // Go through the real warp commit path so the destination
                        // map's script, triggers, and NPC states are (re)loaded —
                        // otherwise coord/NPC interactions wouldn't fire after a
                        // debug warp. The BlackScreen fade state makes the next
                        // update() frame call `commit_pending_warp`.
                        self.overworld.pending_warp =
                            Some(pokered_core::overworld::screen::PendingWarp {
                                dest_map: map_id,
                                dest_x: x as u8,
                                dest_y: y as u8,
                                save_last_map: false,
                                arrival_spin: false,
                            });
                        self.overworld.warp_fade_state =
                            pokered_core::overworld::screen::WarpFadeState::BlackScreen;
                        DebugResponse::ok()
                    }
                    Err(e) => DebugResponse::err(e),
                }
            }
            DebugCommand::Core(CoreDebugCommand::Press { ref button }) => {
                let gb_button = match button.to_lowercase().as_str() {
                    "a" => GbButton::A,
                    "b" => GbButton::B,
                    "start" => GbButton::Start,
                    "select" => GbButton::Select,
                    "up" => GbButton::Up,
                    "down" => GbButton::Down,
                    "left" => GbButton::Left,
                    "right" => GbButton::Right,
                    _ => {
                        return DebugResponse::err(format!("unknown button: '{}'", button));
                    }
                };
                self.pending_debug_inputs.push(Some(gb_button));
                DebugResponse::ok()
            }
            DebugCommand::Core(CoreDebugCommand::PressSequence { ref buttons }) => {
                for b in buttons {
                    let gb_button = match b.to_lowercase().as_str() {
                        "a" => GbButton::A,
                        "b" => GbButton::B,
                        "start" => GbButton::Start,
                        "select" => GbButton::Select,
                        "up" => GbButton::Up,
                        "down" => GbButton::Down,
                        "left" => GbButton::Left,
                        "right" => GbButton::Right,
                        _ => {
                            return DebugResponse::err(format!("unknown button: '{}'", b));
                        }
                    };
                    self.pending_debug_inputs.push(Some(gb_button));
                }
                DebugResponse::ok()
            }
            DebugCommand::Core(CoreDebugCommand::RunFrames { count }) => {
                self.pending_debug_frames += count;
                DebugResponse::ok()
            }
            DebugCommand::Core(CoreDebugCommand::StepFrames { count }) => {
                // Synchronous stepping: drive update() in a tight loop so the
                // game state is fully advanced when the response arrives.
                // Queued Press/PressSequence inputs are consumed one per
                // stepped frame by update()'s debug-input path. Audio ticks
                // along inside update(); rendering is skipped.
                let input = InputState::new();
                for _ in 0..count {
                    self.update(&input);
                }
                DebugResponse::ok_with_data(serde_json::json!({
                    "stepped": count,
                    "frame_count": self.frame_count,
                }))
            }
            DebugCommand::Core(CoreDebugCommand::GetNpcs) => {
                let npcs: Vec<serde_json::Value> = self
                    .overworld
                    .npc_states
                    .iter()
                    .map(|n| {
                        serde_json::json!({
                            "npc_index": n.npc_index,
                            "text_id": n.text_id,
                            "sprite_id": n.sprite_id,
                            "x": n.x,
                            "y": n.y,
                            "home_x": n.home_x,
                            "home_y": n.home_y,
                            "visible": n.visible,
                            "facing": format!("{:?}", n.facing),
                            "walk_counter": n.walk_counter,
                            "scripted_path_remaining": n.scripted_path.len(),
                        })
                    })
                    .collect();
                DebugResponse::ok_with_data(serde_json::json!(npcs))
            }
            DebugCommand::Core(CoreDebugCommand::Save) => {
                #[cfg(not(target_arch = "wasm32"))]
                self.save_to_file();
                DebugResponse::ok()
            }
            DebugCommand::Core(CoreDebugCommand::SetFlag { ref name, value }) => {
                // Set on the live overworld store; the next save-to-file
                // persists it (named bits → SRAM, extras → companion file).
                self.overworld.set_flag_live(name, value);
                DebugResponse::ok()
            }
            DebugCommand::Core(CoreDebugCommand::GiveItem { ref item, qty }) => {
                match pokered_data::items::ItemId::from_const_name(item) {
                    Some(id) => {
                        let _ = self.save_data.game_data.bag.add_item(id, qty as u8);
                        DebugResponse::ok()
                    }
                    None => DebugResponse::err(format!("unknown item: '{}'", item)),
                }
            }
            DebugCommand::Game(GameDebugCommand::GivePokemon { ref species, level }) => {
                let normalized = species
                    .chars()
                    .enumerate()
                    .map(|(i, c)| {
                        if i == 0 {
                            c.to_ascii_uppercase()
                        } else {
                            c.to_ascii_lowercase()
                        }
                    })
                    .collect::<String>();
                match normalized.parse::<pokered_data::species::Species>() {
                    Ok(sp) => {
                        match pokered_core::pokemon::stats::create_pokemon(sp, level, [0x9A, 0x78])
                        {
                            Some(mon) => {
                                let _ = self.save_data.party.add(mon);
                                self.overworld.party_count = self.save_data.party.count() as u8;
                                self.overworld.party_lead_level =
                                    self.save_data.party.leader_level();
                                DebugResponse::ok()
                            }
                            None => DebugResponse::err(format!("failed to create '{}'", species)),
                        }
                    }
                    Err(_) => DebugResponse::err(format!("unknown species: '{}'", species)),
                }
            }
            DebugCommand::Game(GameDebugCommand::StartWildBattle {
                ref species,
                level,
                start_at_frame,
            }) => {
                if let Some(target) = start_at_frame {
                    if target < self.frame_count {
                        return DebugResponse::err(format!(
                            "start_at_frame {} is behind current frame {}",
                            target, self.frame_count
                        ));
                    }
                    while self.frame_count < target {
                        self.update(&InputState::new());
                    }
                }
                let normalized = species
                    .chars()
                    .enumerate()
                    .map(|(i, c)| {
                        if i == 0 {
                            c.to_ascii_uppercase()
                        } else {
                            c.to_ascii_lowercase()
                        }
                    })
                    .collect::<String>();
                match normalized.parse::<pokered_data::species::Species>() {
                    Ok(sp) => {
                        if self.save_data.party.is_empty() {
                            DebugResponse::err("party is empty; give a Pokémon first".to_string())
                        } else {
                            self.start_wild_battle(sp, level);
                            self.state.screen = GameScreen::Battle;
                            DebugResponse::ok_with_data(serde_json::json!({
                                "frame_count": self.frame_count,
                                "state": self.debug_state_snapshot(),
                            }))
                        }
                    }
                    Err(_) => DebugResponse::err(format!("unknown species: '{}'", species)),
                }
            }
        }
    }

    /// Apply an in-game NPC trade's party mutation once its cutscene has
    /// finished (engine/events/in_game_trades.asm `InGameTrade_DoTrade`:
    /// RemovePokemon → AddPartyMon → CopyDataToReceivedMon). The received mon
    /// is built from the TradeMons table data: fixed nickname, OT `<TRAINER>`,
    /// random OT ID + DVs, and the given mon's level; it then enters the
    /// Pokédex as seen+owned (AddPartyMon sets both flags for player-party
    /// adds).
    fn apply_npc_trade(&mut self, trade: PendingTrade) -> bool {
        use pokered_core::trade::{assemble_npc_trade_mon, roll_npc_trade_randoms_thread};
        let Some(offered) = self.save_data.party.get(trade.party_index) else { return false; };
        if offered.species != trade.give { return false; }
        let (dv_bytes, ot_id) = roll_npc_trade_randoms_thread();
        let Some(mon) = assemble_npc_trade_mon(trade.receive, offered.level, &trade.nickname,
            dv_bytes, ot_id, self.save_data.game_data.player_id) else { return false; };
        // Construct first: every failure leaves the original party intact.
        if self.save_data.party.remove_for_trade(trade.party_index).is_err() { return false; }
        if self.save_data.party.add(mon).is_err() { return false; }
        self.save_data.game_data.pokedex.set_seen(trade.receive);
        self.save_data.game_data.pokedex.set_owned(trade.receive);
        self.overworld.party_count = self.save_data.party.count() as u8;
        self.overworld.party_lead_level = self.save_data.party.leader_level();
        true
    }

    /// Queue the evolution cutscene for a batch of detected evolutions
    /// (post-battle level-ups from the writeback, or a single stone / Rare
    /// Candy evolution from the bag). `pre_text` is Rare Candy's "grew to
    /// level X!" message, shown before "What? X is evolving!" (the original
    /// prints it in ItemUseRareCandy before TryEvolvingMon).
    fn queue_evolution_cutscene(
        &mut self,
        events: Vec<pokered_core::battle::settlement::EvolutionEvent>,
        pre_text: Option<String>,
    ) {
        use pokered_core::evolution_screen::{EvolutionScreenState, PendingEvolution};
        let is_zh = matches!(
            self.state.config.language,
            pokered_core::game_state::Lang::Zh
        );
        let queue: Vec<PendingEvolution> = events
            .into_iter()
            .map(|e| {
                let mut name_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
                let name = self
                    .save_data
                    .party
                    .get(e.party_index)
                    .map(|m| m.display_name(&mut name_buf))
                    .unwrap_or("")
                    .to_string();
                PendingEvolution {
                    party_index: e.party_index,
                    from: e.old_species,
                    to: e.new_species,
                    name,
                    // Post-battle evolutions are cancellable: EndOfBattle
                    // clears wForceEvolution (end_of_battle.asm:43-44).
                    force: false,
                }
            })
            .collect();
        if !queue.is_empty() {
            self.evolution_anim = Some(EvolutionScreenState::new(queue, pre_text, is_zh));
        }
    }

    /// Queue a single bag-triggered evolution (stone / Rare Candy).
    /// `force` mirrors wForceEvolution (stones: uncancellable).
    fn queue_item_evolution(
        &mut self,
        party_index: usize,
        from: pokered_data::species::Species,
        to: pokered_data::species::Species,
        pre_text: Option<String>,
        force: bool,
    ) {
        use pokered_core::evolution_screen::{EvolutionScreenState, PendingEvolution};
        let is_zh = matches!(
            self.state.config.language,
            pokered_core::game_state::Lang::Zh
        );
        let mut name_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
        let name = self
            .save_data
            .party
            .get(party_index)
            .map(|m| m.display_name(&mut name_buf))
            .unwrap_or("")
            .to_string();
        self.evolution_anim = Some(EvolutionScreenState::new(
            vec![PendingEvolution {
                party_index,
                from,
                to,
                name,
                force,
            }],
            pre_text,
            is_zh,
        ));
    }

    /// Apply one resolved evolution from the cutscene. On success the species
    /// swap, stat recalc, move learning and Pokédex updates land
    /// (`finalize_evolution`); on a B-cancel nothing happens — the original
    /// keeps the mon unchanged and retries on its next level-up
    /// (wCanEvolveFlags). Level-up moves that could not be learned because
    /// the moveset is full are queued for the forget-a-move prompt (Gen-1
    /// `LearnMove` runs it inline after `LearnMoveFromLevelUp`).
    fn apply_evolution_outcome(
        &mut self,
        outcome: &pokered_core::evolution_screen::EvolutionOutcome,
    ) {
        use pokered_core::evolution_screen::EvolutionOutcomeKind;
        if outcome.kind != EvolutionOutcomeKind::Evolved {
            return;
        }
        if let Some(mon) = self.save_data.party.get_mut(outcome.party_index) {
            let blocked = pokered_core::pokemon::evolution::finalize_evolution(
                mon,
                &mut self.save_data.game_data.pokedex,
                outcome.to,
            );
            if let Some(&move_id) = blocked.first() {
                self.pending_evolve_move_replace = Some((outcome.party_index, move_id));
            }
        }
    }


    /// Draw game state to frame buffer. Available for both wasm and native builds.
    pub fn draw(&mut self, frame_buffer: &mut FrameBuffer) {
        if self.black_screen_frames > 0 {
            frame_buffer.clear(Rgba::BLACK);
            return;
        }

        // The trade cutscene takes over the whole screen while it plays.
        if let Some(ref anim) = self.trade_anim {
            draw_trade(anim, &mut self.resources, frame_buffer);
            return;
        }

        // The evolution cutscene takes over the whole screen while it plays.
        if let Some(ref anim) = self.evolution_anim {
            draw_evolution(anim, &mut self.resources, frame_buffer);
            return;
        }

        // The Hall of Fame roll call and the end credits take over the whole
        // screen while they play.
        if let Some(ref hof) = self.hof_ceremony {
            if hof.phase() == pokered_core::hof_ceremony::HofPhase::FadeOut {
                draw_overworld(
                    &mut self.overworld,
                    &mut self.resources,
                    frame_buffer,
                    self.state.config.language,
                );
            }
            draw_hof_ceremony(
                hof,
                &mut self.resources,
                frame_buffer,
                self.state.config.language,
            );
            return;
        }
        if let Some(ref roll) = self.credits {
            draw_credits(roll, &mut self.resources, frame_buffer);
            return;
        }

        // These renderers own their backgrounds and clear the full screen.
        // Avoid clearing the GBA's software framebuffer twice every frame.
        if !matches!(
            self.state.screen,
            GameScreen::GameFreakSplash
                | GameScreen::LanguageSelect
                | GameScreen::IntroScene
                | GameScreen::TitleScreen
                | GameScreen::OakSpeech
                | GameScreen::Overworld
                | GameScreen::Battle
        ) {
            frame_buffer.clear(Rgba::WHITE);
        }

        match self.state.screen {
            GameScreen::GameFreakSplash => {
                draw_gamefreak_splash(&self.gamefreak_splash, &mut self.resources, frame_buffer);
            }
            GameScreen::CopyrightSplash => {
                draw_title_screen(&self.title_screen, true, &mut self.resources, frame_buffer);
            }
            GameScreen::LanguageSelect => {
                draw_language_select(frame_buffer, self.state.config.language);
            }
            GameScreen::IntroScene => {
                draw_intro_scene(&self.intro_scene, &mut self.resources, frame_buffer);
            }
            GameScreen::TitleScreen => {
                draw_title_screen(&self.title_screen, false, &mut self.resources, frame_buffer);
            }
            GameScreen::MainMenu => {
                draw_main_menu(&self.main_menu, frame_buffer, self.state.config.language);
            }
            GameScreen::OakSpeech => {
                draw_oak_speech(
                    &self.oak_speech,
                    &mut self.resources,
                    frame_buffer,
                    self.state.config.language,
                );
            }
            GameScreen::Overworld => {
                draw_overworld(
                    &mut self.overworld,
                    &mut self.resources,
                    frame_buffer,
                    self.state.config.language,
                );
                // Cable Club link overlay (text boxes / prompts / trade list)
                // over the frozen room.
                #[cfg(not(target_os = "none"))]
                if self.link_cable.is_active() {
                    crate::render::draw_link_flow(
                        &self.link_cable,
                        frame_buffer,
                        matches!(
                            self.state.config.language,
                            pokered_core::game_state::Lang::Zh
                        ),
                        self.resources.as_mut(),
                    );
                }
            }
            GameScreen::Battle => {
                let uses_overworld_snapshot = matches!(
                    &self.battle.phase,
                    BattlePhase::Intro {
                        phase: pokered_core::battle::IntroPhase::TransitionFlash
                            | pokered_core::battle::IntroPhase::BattleTransitionWipe(_),
                        ..
                    }
                );
                if uses_overworld_snapshot && self.battle_vfx.overworld_snapshot.is_none() {
                    // On GBA the current software framebuffer already is the
                    // final overworld frame. Release its decoded map assets
                    // before allocating the transition snapshot; redrawing
                    // the same frame while retaining those assets exhausts
                    // EWRAM at the first grass encounter.
                    #[cfg(target_os = "none")]
                    if let Some(resources) = self.resources.as_mut() {
                        resources.clear_cache();
                    }
                    #[cfg(target_os = "none")]
                    let snapshot =
                        pokered_renderer::transition_blit::CompactSnapshot::capture(frame_buffer);
                    #[cfg(not(target_os = "none"))]
                    let mut snapshot = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::BLACK);
                    #[cfg(not(target_os = "none"))]
                    draw_overworld(
                        &mut self.overworld,
                        &mut self.resources,
                        &mut snapshot,
                        self.state.config.language,
                    );
                    self.battle_vfx.overworld_snapshot = Some(snapshot);
                }
                // The first non-transition battle frame builds the combined
                // battle tileset and sprite caches. Drop the full 160×144
                // snapshot and its decoded Overworld assets *before* those
                // allocations; doing this after draw exceeds GBA EWRAM at the
                // WildReveal→HUD boundary.
                if !uses_overworld_snapshot && self.battle_vfx.overworld_snapshot.is_some() {
                    self.battle_vfx.clear_snapshot();
                    #[cfg(target_os = "none")]
                    if let Some(resources) = self.resources.as_mut() {
                        resources.clear_cache();
                    }
                }
                draw_battle(
                    &self.battle,
                    &mut self.resources,
                    frame_buffer,
                    &mut self.battle_vfx,
                    self.state.config.language,
                );
            }
            GameScreen::StartMenu => {
                if let Some(restore) = self.overworld.field_text_restore.as_ref()
                    .filter(|restore| restore.submenu_reload.is_some()) {
                    if restore.elapsed == 0 {
                        let first_white_line = restore.submenu_reload.as_ref().unwrap().white_start_line;
                        // Palette writes happen during the current frame. Party
                        // sprites have already advanced, so render this frame's
                        // party pose rather than freezing the previous image.
                        draw_party_screen(&self.party_screen, self.resources.as_mut(),
                            self.frame_count, frame_buffer, self.state.config.language);
                        for y in u32::from(first_white_line)..144 {
                            for x in 0..160 { frame_buffer.set_pixel(x, y, Rgba::WHITE); }
                        }
                    } else {
                        frame_buffer.clear(Rgba::WHITE);
                    }
                    return;
                }
                draw_overworld(
                    &mut self.overworld,
                    &mut self.resources,
                    frame_buffer,
                    self.state.config.language,
                );
                if self.overworld.field_text_window_visible() { draw_start_menu(
                    &self.start_menu,
                    &self.player_name,
                    frame_buffer,
                    self.state.config.language,
                ); }
            }
            GameScreen::OptionsMenu => {
                draw_options_menu(&self.options_menu, frame_buffer, self.state.config.language);
            }
            GameScreen::SaveMenu => {
                // save.gui clears and covers the complete 160x144 viewport;
                // drawing the live map first only creates pixels immediately
                // discarded by the save card.
                draw_save_menu(&self.save_menu, frame_buffer, self.state.config.language);
            }
            GameScreen::PartyScreen => {
                draw_party_screen(
                    &self.party_screen,
                    self.resources.as_mut(),
                    self.frame_count,
                    frame_buffer,
                    self.state.config.language,
                );
            }
            GameScreen::PokemonStatsScreen(_) => {
                if self.stats_screen.as_ref().is_some_and(|s| s.entry_frame() == Some(0)) {
                    if let Some(pc) = self.pc_screen.as_ref() {
                        draw_pc(pc, &self.save_data, &mut self.resources, frame_buffer, self.state.config.language);
                    } else {
                        draw_party_screen(&self.party_screen, self.resources.as_mut(), self.frame_count, frame_buffer, self.state.config.language);
                    }
                } else if let Some(ref ss) = self.stats_screen {
                    draw_stats_screen(
                        ss,
                        self.resources.as_mut(),
                        frame_buffer,
                        self.state.config.language,
                    );
                }
            }
            GameScreen::Shop(ref mart_state) => {
                // The mart is an overworld overlay sequence: pokemart.asm
                // saves and restores the screen tiles around every menu, so
                // the live map (clerk included) stays visible as the backdrop.
                draw_overworld(
                    &mut self.overworld,
                    &mut self.resources,
                    frame_buffer,
                    self.state.config.language,
                );
                let money = self.save_data.game_data.player_money;
                let bag_slice = self.save_data.game_data.bag.items();
                draw_mart(
                    mart_state,
                    money,
                    &bag_slice[..],
                    frame_buffer,
                    self.state.config.language,
                );
            }
            GameScreen::Bag => {
                draw_bag(&self.bag_screen, frame_buffer, self.state.config.language);
            }
            GameScreen::TownMap => {
                draw_town_map(
                    &self.town_map_screen,
                    &mut self.resources,
                    self.frame_count,
                    frame_buffer,
                    self.state.config.language,
                );
            }
            GameScreen::Slots => {
                if let Some(ref slots) = self.slots_screen {
                    draw_slots(slots, frame_buffer, self.state.config.language);
                }
            }
            GameScreen::Elevator => {
                if let Some(ref elevator) = self.elevator_screen {
                draw_overworld(
                    &mut self.overworld,
                    &mut self.resources,
                    frame_buffer,
                    self.state.config.language,
                );
                    draw_elevator(elevator, frame_buffer, self.state.config.language);
                }
            }
            GameScreen::FilterBag => {
                if let Some(ref filter) = self.elevator_screen {
                    draw_filter_bag(filter, frame_buffer, self.state.config.language);
                }
            }
            GameScreen::Diploma => {
                draw_diploma(&self.player_name, &mut self.resources, frame_buffer, self.state.config.language);
            }
            GameScreen::Pokedex => {
                let is_zh = matches!(
                    self.state.config.language,
                    pokered_core::game_state::Lang::Zh
                );
                draw_pokedex_screen(
                    &self.pokedex_screen,
                    self.overworld.state.current_map,
                    is_zh,
                    &mut self.resources,
                    frame_buffer,
                );
            }
            GameScreen::TrainerCard => {
                draw_trainer_card(
                    &self.player_name,
                    self.save_data.game_data.player_money,
                    self.save_data.game_data.play_time.hours,
                    self.save_data.game_data.play_time.minutes,
                    self.save_data.game_data.obtained_badges,
                    &mut self.resources,
                    frame_buffer,
                    self.state.config.language,
                );
            }
            GameScreen::PC => {
                if let Some(ref pc) = self.pc_screen {
                    draw_pc(
                        pc,
                        &self.save_data,
                        &mut self.resources,
                        frame_buffer,
                        self.state.config.language,
                    );
                }
                match self.pc_stats_return_frame {
                    Some(0..=6) => frame_buffer.clear(Rgba::WHITE),
                    Some(7) => {
                        for y in 48..96 { for x in 0..160 {
                            frame_buffer.set_pixel(x, y, Rgba::WHITE);
                        } }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Check if game should exit. Available for both wasm and native builds.
    pub fn should_exit(&self) -> bool {
        self.exit_requested
    }
}

const BLACK_SCREEN_DURATION: u32 = 30;

/// Frames A+B+Start+Select must be held to trigger a soft reset
/// (`hSoftReset` starts at 16 in home/init.asm).
const SOFT_RESET_HOLD_FRAMES: u8 = 16;

#[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
impl GameLoop for PokemonGame {
    type Fb = FrameBuffer;

    fn update(&mut self, input: &InputState) {
        #[cfg(feature = "ewram-audit")]
        {
            static AUDIT_RESET: core::sync::atomic::AtomicBool =
                core::sync::atomic::AtomicBool::new(false);
            if !AUDIT_RESET.swap(true, core::sync::atomic::Ordering::SeqCst) {
                crate::mem_audit::reset();
            }
        }
        self.update(input);
    }

    fn draw(&mut self, frame_buffer: &mut FrameBuffer) {
        self.draw(frame_buffer);
    }

    fn should_exit(&self) -> bool {
        self.should_exit()
    }
}

fn parse_sfx_id(name: &str) -> Option<SfxId> {
    match name {
        "SFX_GET_ITEM_1" | "SFX_GET_ITEM1" => Some(SfxId::GetItem1),
        "SFX_GET_ITEM_2" | "SFX_GET_ITEM2" => Some(SfxId::GetItem2),
        "SFX_GET_KEY_ITEM" => Some(SfxId::GetKeyItem),
        "SFX_TINK" => Some(SfxId::Tink),
        "SFX_HEAL_HP" => Some(SfxId::HealHP),
        "SFX_HEAL_AILMENT" => Some(SfxId::HealAilment),
        "SFX_START_MENU" => Some(SfxId::StartMenu),
        "SFX_PRESS_AB" => Some(SfxId::PressAB),
        "SFX_POKEDEX_RATING" => Some(SfxId::PokedexRating),
        "SFX_POISONED" => Some(SfxId::Poisoned),
        "SFX_TRADE_MACHINE" => Some(SfxId::TradeMachine),
        "SFX_TURN_ON_PC" => Some(SfxId::TurnOnPC),
        "SFX_TURN_OFF_PC" => Some(SfxId::TurnOffPC),
        "SFX_ENTER_PC" => Some(SfxId::EnterPC),
        "SFX_SHRINK" => Some(SfxId::Shrink),
        "SFX_SWITCH" => Some(SfxId::Switch),
        "SFX_HEALING_MACHINE" => Some(SfxId::HealingMachine),
        "SFX_TELEPORT_EXIT_1" => Some(SfxId::TeleportExit1),
        "SFX_TELEPORT_ENTER_1" => Some(SfxId::TeleportEnter1),
        "SFX_TELEPORT_EXIT_2" => Some(SfxId::TeleportExit2),
        "SFX_LEDGE" => Some(SfxId::Ledge),
        "SFX_TELEPORT_ENTER_2" => Some(SfxId::TeleportEnter2),
        "SFX_FLY" => Some(SfxId::Fly),
        "SFX_DENIED" => Some(SfxId::Denied),
        "SFX_ARROW_TILES" => Some(SfxId::ArrowTiles),
        "SFX_PUSH_BOULDER" => Some(SfxId::PushBoulder),
        "SFX_SS_ANNE_HORN" => Some(SfxId::SSAnneHorn),
        "SFX_WITHDRAW_DEPOSIT" => Some(SfxId::WithdrawDeposit),
        "SFX_CUT" => Some(SfxId::Cut),
        "SFX_GO_INSIDE" => Some(SfxId::GoInside),
        "SFX_SWAP" => Some(SfxId::Swap),
        "SFX_PURCHASE" => Some(SfxId::Purchase),
        "SFX_COLLISION" => Some(SfxId::Collision),
        "SFX_GO_OUTSIDE" => Some(SfxId::GoOutside),
        "SFX_SAVE" => Some(SfxId::Save),
        "SFX_POKEFLUTE" => Some(SfxId::Pokeflute),
        "SFX_SAFARI_ZONE_PA" => Some(SfxId::SafariZonePA),
        "SFX_LEVEL_UP" => Some(SfxId::LevelUp),
        "SFX_BALL_TOSS" => Some(SfxId::BallToss),
        "SFX_BALL_POOF" => Some(SfxId::BallPoof),
        "SFX_FAINT_THUD" => Some(SfxId::FaintThud),
        "SFX_RUN" => Some(SfxId::Run),
        "SFX_DEX_PAGE_ADDED" => Some(SfxId::DexPageAdded),
        "SFX_CAUGHT_MON" => Some(SfxId::CaughtMon),
        "SFX_SHOOTING_STAR" => Some(SfxId::ShootingStar),
        _ => None,
    }
}

fn draw_language_select(fb: &mut FrameBuffer, current: pokered_core::game_state::Lang) {
    use pokered_core::game_state::Lang;
    use pokered_renderer::embedded_font::draw_text;
    use pokered_renderer::Rgba;
    fb.clear(Rgba::WHITE);
    draw_text("Select Language / 选择语言", 16, 48, Rgba::BLACK, fb);
    let gray = Rgba::rgb(0x80, 0x80, 0x80);
    let (en_pre, en_color) = if current == Lang::En {
        ("> ", Rgba::BLACK)
    } else {
        ("  ", gray)
    };
    let (zh_pre, zh_color) = if current == Lang::Zh {
        ("> ", Rgba::BLACK)
    } else {
        ("  ", gray)
    };
    let en_line = format!("{}English", en_pre);
    let zh_line = format!("{}中文", zh_pre);
    draw_text(&en_line, 16, 72, en_color, fb);
    draw_text(&zh_line, 16, 90, zh_color, fb);
}

#[cfg(test)]
mod session_guard_tests {
    use super::*;

    /// The shop-exit teleport bug: `GameScreen::Shop(_)` (a carried variant
    /// that cannot be compared with `!=`) was missing from the Continue/NewGame
    /// re-entry guards, so exiting a mart rebuilt the overworld from the save
    /// and teleported the player. The helper must classify EVERY in-session
    /// screen — including the carried ones — as in-session.
    #[test]
    fn shop_and_carried_screens_are_in_session() {
        assert!(is_ingame_session_screen(&GameScreen::Overworld));
        assert!(is_ingame_session_screen(&GameScreen::Shop(
            pokered_core::items::MartState::new(pokered_core::items::ShopInventory::new(vec![
                pokered_data::items::ItemId::Potion
            ]),)
        )));
        assert!(is_ingame_session_screen(&GameScreen::PokemonStatsScreen(0)));
        assert!(is_ingame_session_screen(&GameScreen::Bag));
        assert!(is_ingame_session_screen(&GameScreen::PartyScreen));
        assert!(is_ingame_session_screen(&GameScreen::TownMap));
        // A NEW GAME session using the bag/fly must not be treated as
        // "outside a session" either (the NewGame arm's old table missed these).
        assert!(is_ingame_session_screen(&GameScreen::PC));
    }

    #[test]
    fn pre_session_screens_are_not_in_session() {
        assert!(!is_ingame_session_screen(&GameScreen::MainMenu));
        assert!(!is_ingame_session_screen(&GameScreen::TitleScreen));
        assert!(!is_ingame_session_screen(&GameScreen::OakSpeech));
    }
}

#[cfg(all(test, feature = "debug-server"))]
mod synchronous_input_tests {
    use super::*;

    #[test]
    fn evaluation_telemetry_reads_dex_and_experience_without_advancing() {
        use pokered_data::species::Species;
        let mut game = PokemonGame::new_with_options(
            GameVersion::Red, None, None, None, false, None, false, true, None,
        );
        game.save_data.game_data.pokedex.set_seen(Species::Pikachu);
        game.save_data.game_data.pokedex.set_owned(Species::Bulbasaur);
        game.save_data.game_data.pokedex.set_owned(Species::Zapdos);
        let mon = pokered_core::pokemon::stats::create_pokemon(Species::Bulbasaur, 10, [0x9a, 0x78]).unwrap();
        let experience = mon.total_exp;
        game.save_data.party.add(mon.clone()).unwrap();
        let before = game.frame_count;
        let command = serde_json::from_value(serde_json::json!({"cmd": "get_state"})).unwrap();
        let response = serde_json::to_value(game.handle_debug_command(command)).unwrap();
        assert_eq!(response["ok"], true);
        assert_eq!(response["data"]["evaluation"]["pokedex"]["seen"], 3);
        assert_eq!(response["data"]["evaluation"]["pokedex"]["owned"], 2);
        assert_eq!(response["data"]["evaluation"]["pokedex"]["owned_numbers"], serde_json::json!([1, 145]));
        assert_eq!(response["data"]["evaluation"]["party"][0]["total_exp"], experience);
        assert!(response["data"]["party"][0].get("total_exp").is_none());
        assert_eq!(game.frame_count, before);

        // During battle the persistent party is stale: use the actual roster,
        // including experience and PP, then return to save data on the field.
        let mut live_mon = mon;
        live_mon.hp = 1;
        live_mon.pp[0] = 0;
        live_mon.total_exp += 40;
        game.battle.battle_state = Some(pokered_core::battle::state::new_battle_state(
            pokered_core::battle::state::BattleType::Wild,
            vec![live_mon.clone()], vec![live_mon],
        ));
        game.state.screen = GameScreen::Battle;
        let state = game.debug_state_snapshot();
        assert_eq!(state["evaluation"]["party_source"], "battle_live");
        assert_eq!(state["evaluation"]["party"][0]["hp"], 1);
        assert_eq!(state["evaluation"]["party"][0]["pp"][0], 0);
        assert_eq!(state["evaluation"]["party"][0]["total_exp"], experience + 40);
        game.state.screen = GameScreen::Overworld;
        let state = game.debug_state_snapshot();
        assert_eq!(state["evaluation"]["party_source"], "save_data");
        assert_eq!(state["evaluation"]["party"][0]["total_exp"], experience);
        assert_eq!(game.frame_count, before);
    }

    #[test]
    fn timeline_advances_exact_frames_and_leaves_no_background_work() {
        let mut game = PokemonGame::new_with_options(
            GameVersion::Red,
            None,
            None,
            None,
            false,
            None,
            false,
            true,
            None,
        );
        let before = game.frame_count;
        for _ in 0..2 {
            let command = serde_json::from_value(serde_json::json!({
                "cmd": "press_timeline", "buttons": ["b", "b", null], "advance": true,
            }))
            .unwrap();
            let response = serde_json::to_value(game.handle_debug_command(command)).unwrap();
            assert_eq!(response["ok"], true);
            assert!(!game.debug_work_pending());
        }
        assert_eq!(game.frame_count, before + 6);

        // A queued legacy input must never silently shift a synchronous one.
        let queued = serde_json::from_value(serde_json::json!({
            "cmd": "press_timeline", "buttons": [null],
        }))
        .unwrap();
        game.handle_debug_command(queued);
        let synchronous = serde_json::from_value(serde_json::json!({
            "cmd": "press_timeline", "buttons": ["b"], "advance": true,
        }))
        .unwrap();
        let response = serde_json::to_value(game.handle_debug_command(synchronous)).unwrap();
        assert_eq!(response["ok"], false);
        assert_eq!(game.frame_count, before + 6);
    }
}

#[cfg(test)]
mod fidelity_systems_npc_tests {
    use super::*;
    use pokered_core::pokemon::stats::create_pokemon;
    use pokered_data::species::Species;

    fn game() -> PokemonGame {
        PokemonGame::new_with_options(GameVersion::Red, None, None, None, false, None,
            false, true, #[cfg(feature = "debug-server")] None)
    }

    #[test]
    #[cfg(not(target_arch="wasm32"))]
    fn fidelity_companions_are_bound_to_save_path_with_legacy_only_fallback() {
        let first=Path::new("/tmp/fidelity/slot-one.sav");
        let second=Path::new("/tmp/fidelity/slot-two.sav");
        assert_eq!(companion_path_for_save(first),Path::new("/tmp/fidelity/slot-one.script_flags.json"));
        assert_ne!(companion_path_for_save(first),companion_path_for_save(second));
        assert_eq!(companion_lookup_paths(Some(first),false),vec![companion_path_for_save(first)]);
        assert_eq!(companion_lookup_paths(Some(first),true),vec![companion_path_for_save(first),script_flags_file_path()]);
        assert_eq!(companion_lookup_paths(None,true),vec![script_flags_file_path()]);
        assert_eq!(companion_path_for_save(&save_file_path()),script_flags_file_path());
    }

    #[test]
    fn fidelity_cable_room_selection_completes_actual_warp() {
        for (kind,map) in [(LinkKind::Trade,MapId::TradeCenter),(LinkKind::Battle,MapId::Colosseum)] {
            let mut game=game();
            game.state.screen=GameScreen::Overworld;
            game.overworld=OverworldScreen::new(MapId::CeruleanPokecenter,None,PokemonRedData);
            game.handle_flow_need(FlowNeed::EnterRoom(kind));
            assert!(matches!(game.overworld.warp_fade_state,pokered_core::overworld::WarpFadeState::FadingOut { .. }));
            for _ in 0..90 { game.update(&InputState::new()); }
            assert_eq!(game.overworld.state.current_map,map);
        }
    }

    #[test]
    fn fidelity_npc_trade_accepts_last_mon_and_selected_duplicate() {
        for (count, selected) in [(1,0), (2,1), (6,5)] {
            let mut game=game();
            for index in 0..count {
                game.save_data.party.add(create_pokemon(Species::Abra,15+index as u8,[0x99,0x88]).unwrap()).unwrap();
            }
            assert!(game.apply_npc_trade(PendingTrade { party_index:selected, ready_to_animate:false, give:Species::Abra,
                receive:Species::MrMime,nickname:"MARCEL".to_string() }));
            assert_eq!(game.save_data.party.count(),count);
            let received=game.save_data.party.get(count-1).unwrap();
            assert_eq!(received.species,Species::MrMime);
            assert_eq!(received.level,15+selected as u8);
            assert!(game.save_data.game_data.pokedex.is_owned(Species::MrMime));
            assert_eq!(game.save_data.party.iter().filter(|m|m.species==Species::Abra).count(),count-1);
        }
    }

    #[test]
    fn fidelity_npc_wrong_selection_preserves_party_and_pokedex() {
        let mut game=game();let mon=create_pokemon(Species::Pikachu,15,[0x99,0x88]).unwrap();
        game.save_data.party.add(mon).unwrap();
        for index in [0,1] {
            assert!(!game.apply_npc_trade(PendingTrade { party_index:index, ready_to_animate:false, give:Species::Abra,
                receive:Species::MrMime,nickname:"MARCEL".to_string() }));
            assert_eq!(*game.save_data.party.get(0).unwrap(),mon);
            assert!(!game.save_data.game_data.pokedex.is_owned(Species::MrMime));
        }
    }

    #[test]
    fn fidelity_npc_completed_flag_precedes_connect_text_only_for_valid_pick() {
        for (offered, button, completed) in [
            (Species::Abra, GbButton::A, true),
            (Species::Abra, GbButton::B, false),
            (Species::Pikachu, GbButton::A, false),
        ] {
            let mut game = game();
            game.state.screen = GameScreen::Overworld;
            game.overworld = OverworldScreen::new(MapId::Route2TradeHouse, None, PokemonRedData);
            game.save_data.party.clear();
            game.save_data.party.add(create_pokemon(offered, 15, [0x99, 0x88]).unwrap()).unwrap();
            game.pending_trade = Some(PendingTrade { party_index: 0, ready_to_animate: false,
                give: Species::Abra, receive: Species::MrMime, nickname: "MARCEL".to_string() });
            game.overworld.begin_party_select(game.save_data.party.to_vec());
            let mut input = InputState::new();
            input.press(button);
            game.update(&input);
            assert_eq!(game.overworld.unified_flags().get_flag("EVENT_TRADED_FOR_MARCEL"), completed);
            assert_eq!(game.save_data.party.get(0).unwrap().species, offered);
            assert!(game.trade_anim.is_none());
            if completed {
                assert!(game.overworld.pending_dialogue.is_some());
                assert!(game.pending_trade.as_ref().unwrap().ready_to_animate);
            }
        }
    }
}

#[cfg(test)]
mod save_overwrite_tests {
    use super::*;
    use pokered_core::game_state::SaveFileSummary;

    fn summary_with(id: u16) -> SaveFileSummary {
        SaveFileSummary {
            player_name: vec![0x50; 11],
            badges: 0,
            pokedex_owned: 0,
            play_time_hours: 0,
            play_time_minutes: 0,
            play_time_seconds: 0,
            player_id: id,
        }
    }

    /// CheckPreviousSaveFile (engine/menus/save.asm:622-653): overwriting a
    /// file from a DIFFERENT trainer ID must prompt first. The predicate the
    /// SaveMenu arm computes: different (non-zero) disk ID vs in-memory ID.
    #[test]
    fn different_disk_player_id_demands_confirmation() {
        let disk = summary_with(0x1234);
        let memory_id: u16 = 0xBEEF;
        let different = disk.player_id != 0 && disk.player_id != memory_id;
        assert!(different, "a foreign ID is a different player");

        let same = summary_with(memory_id);
        assert!(!(same.player_id != 0 && same.player_id != memory_id));

        // Legacy summaries (pre-field) carry 0 and never trigger the prompt.
        let legacy = summary_with(0);
        assert!(!(legacy.player_id != 0 && legacy.player_id != memory_id));
    }

    #[test]
    fn constructor_preserves_loaded_save_summary() {
        let mut save = SaveData::new();
        save.game_data.player_id = 0x1234;
        let path = std::env::temp_dir().join(format!(
            "pokered-constructor-save-{}-{}.sav",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::write(&path, export_sram(&save)).unwrap();

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

        assert!(game.state.has_save_file());
        assert!(game.main_menu.has_save);
        assert_eq!(game.state.save_summary.unwrap().player_id, 0x1234);
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod wall_town_map_tests {
    use super::*;
    use pokered_core::overworld::Direction;

    #[test]
    fn wall_town_map_opens_after_dialogue_and_returns_in_place() {
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
        game.state.screen = GameScreen::Overworld;
        game.overworld = OverworldScreen::new(MapId::BluesHouse, None, PokemonRedData);
        game.overworld.state.player.x = 3;
        game.overworld.state.player.y = 1;
        game.overworld.state.player.facing = Direction::Up;
        game.main_menu.last_choice = Some(pokered_core::game_state::MainMenuChoice::Continue);
        let idle = InputState::new();
        let mut a = InputState::new();
        a.press(GbButton::A);
        let mut b = InputState::new();
        b.press(GbButton::B);

        for _ in 0..40 {
            game.update(&idle);
        }
        for _ in 0..2 { game.update(&a); }
        assert!(game.overworld.pending_town_map);
        assert!(game.overworld.pending_dialogue.is_some());
        assert_eq!(game.state.screen, GameScreen::Overworld);
        for _ in 0..80 {
            game.update(&idle);
        }
        assert_eq!(
            game.state.screen,
            GameScreen::Overworld,
            "wait for dialogue dismissal"
        );
        game.update(&b);
        for _ in 0..20 {
            game.update(&idle);
        }
        // Optional deterministic frame capture; also works on the unfixed base.
        if let Ok(path) = std::env::var("WALL_MAP_SCREENSHOT") {
            let mut fb = FrameBuffer::new(
                dotzuki_engine::render_config::RenderConfig::new(160, 144),
                pokered_renderer::Rgba::WHITE,
            );
            game.draw(&mut fb);
            let img = image::RgbaImage::from_fn(160, 144, |x, y| {
                image::Rgba(fb.get_pixel(x, y).unwrap().to_array())
            });
            img.save(path).unwrap();
        }
        assert_eq!(game.state.screen, GameScreen::TownMap);
        assert!(!game.overworld.pending_town_map);
        assert!(game.overworld.pending_dialogue.is_none());
        assert_eq!(
            game.town_map_screen.mode(),
            pokered_core::town_map_screen::TownMapMode::View
        );

        game.update(&b);
        for _ in 0..20 {
            game.update(&idle);
        }
        assert_eq!(game.state.screen, GameScreen::Overworld);
        assert_eq!(game.overworld.state.current_map, MapId::BluesHouse);
        assert_eq!(
            (game.overworld.state.player.x, game.overworld.state.player.y),
            (3, 1)
        );
        assert_eq!(game.overworld.state.player.facing, Direction::Up);
        for _ in 0..2 { game.update(&a); }
        assert!(
            game.overworld.pending_town_map,
            "wall map can be inspected again"
        );
    }
}

#[cfg(test)]
mod captain_music_wait_fidelity_tests {
    use super::*;
    #[test]
    fn no_audio_frontend_resumes_wait_music_when_the_real_healed_channel_ends() {
        let mut game = PokemonGame::new_with_options(GameVersion::Red, None, None, None,
            false, None, false, true, #[cfg(feature = "debug-server")] None);
        game.state.screen = GameScreen::Overworld;
        game.overworld = OverworldScreen::new(MapId::SSAnneCaptainsRoom, None, PokemonRedData);
        game.overworld.state.player.x = 4;
        game.overworld.state.player.y = 3;
        game.audio.as_ref().unwrap().play_music(MusicId::PKMNHEALED);
        // Use the public scene-loading seam: the native VM must suspend on
        // waitMusic and continue to the flag command after CHAN1 finishes.
        game.overworld.reload_scene_with_config(
            "SSAnneCaptainsRoom",
            r#"game_scene SSAnneCaptainsRoom {
  @storyline("captainMusicProbe") {
    waitMusic()
    setFlag("CAPTAIN_MUSIC_PROBE_DONE")
  }
}"#,
            Some(r#"{"onLoad":"captainMusicProbe"}"#),
        ).unwrap();
        assert_eq!(game.overworld.active_script_effect_label().as_deref(), Some("WaitMusic"));
        game.update(&InputState::new());
        assert_eq!(game.overworld.active_script_effect_label().as_deref(), Some("WaitMusic"));
        assert!(!game.overworld.unified_flags().get_flag("CAPTAIN_MUSIC_PROBE_DONE"));
        for _ in 0..1024 {
            game.update(&InputState::new());
            if game.overworld.unified_flags().get_flag("CAPTAIN_MUSIC_PROBE_DONE") {
                assert!(game.overworld.script_engine_idle());
                assert_eq!(game.overworld.active_script_effect_label(), None);
                assert!(!game.audio.as_ref().unwrap().is_music_channel_playing(0));
                return;
            }
        }
        panic!("no-audio WaitMusic remained blocked after the healed jingle");
    }
}

#[cfg(test)]
mod web_legacy_save_fidelity_tests {
    use super::*;
    #[test]
    fn web_reader_migrates_same_slot_extras_only_for_original_json_without_tail() {
        let aliases = ["EVENT_TRADED_FOR_MARCEL", "EVENT_GOT_OLD_ROD", "EVENT_GOT_GOOD_ROD", "EVENT_GOT_SUPER_ROD"];
        for old in [false, true] {
            let mut value = serde_json::to_value(SaveData::new()).unwrap();
            if old { value["game_data"].as_object_mut().unwrap().remove("game_progress_tail"); }
            let raw = serde_json::to_string(&value).unwrap();
            let save = decode_web_save(&raw).unwrap();
            assert_eq!(save.imported_legacy_json, old);
            let extras = aliases.into_iter().map(|name| (name.to_string(), true)).collect();
            let mut overworld = OverworldScreen::new(MapId::ViridianCity, None, PokemonRedData);
            overworld.restore_loaded_save_flags(&save, Some(extras));
            for name in aliases { assert_eq!(overworld.unified_flags().get_flag(name), old, "{name}"); }
            // The debug snapshot parser must not acquire the real-reader provenance.
            let snapshot: SaveData = serde_json::from_str(&raw).unwrap();
            assert!(!snapshot.imported_legacy_json);
        }
    }
    #[test]
    fn invalid_web_json_does_not_grant_legacy_import() {
        for raw in ["not json", "{}", r#"{"game_data":{"game_progress_tail":null}}"#] {
            assert!(decode_web_save(raw).is_err());
        }
    }

    #[test]
    fn web_reader_recomputes_named_zero_ot_trade_identity() {
        for (owner, expected) in [(1234, true), (0, false)] {
            let mut save = SaveData::new();
            save.game_data.player_id = owner;
            let mut mon = pokered_core::trade::assemble_npc_trade_mon(
                pokered_data::species::Species::MrMime, 15, "MARCEL", [0x99, 0x88], 0, owner,
            ).unwrap();
            mon.is_traded = !expected; // stale derived flag from prior JSON
            save.party.add(mon).unwrap();
            let raw = serde_json::to_string(&save).unwrap();
            assert_eq!(decode_web_save(&raw).unwrap().party.get(0).unwrap().is_traded, expected);
        }
    }
}

#[cfg(all(test, not(target_os = "none")))]
mod asynchronous_colosseum_fidelity_tests {
    use super::*;
    use pokered_core::battle::state::StatusCondition;
    use pokered_core::battle::BattlePhase;
    use pokered_core::link::transport::ChannelTransport;
    use pokered_core::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::{moves::MoveId, species::Species};

    fn game(species: Species, level: u8, attack: MoveId, hp: u16) -> PokemonGame {
        let mut game = PokemonGame::new_with_options(GameVersion::Red, None, None, None,
            false, None, false, true, #[cfg(feature = "debug-server")] None);
        let mut mon = create_pokemon_with_moves(species, level, [0x99, 0x88],
            [attack, MoveId::None, MoveId::None, MoveId::None]).unwrap();
        mon.hp = hp;
        mon.status = StatusCondition::Burn;
        mon.pp[0] = 2;
        let mut backup = create_pokemon_with_moves(Species::Pikachu, 10, [0x99, 0x88],
            [MoveId::Thundershock, MoveId::None, MoveId::None, MoveId::None]).unwrap();
        backup.hp = 0;
        backup.status = StatusCondition::Poison;
        backup.pp[0] = 0;
        game.save_data.party = pokered_core::pokemon::party::Party::from(vec![mon, backup]);
        game.state.screen = GameScreen::Overworld;
        game.main_menu.last_choice = Some(pokered_core::game_state::MainMenuChoice::Continue);
        game.overworld = OverworldScreen::new(MapId::Colosseum, None, PokemonRedData);
        game
    }

    #[test]
    fn actual_frontend_asynchronous_ko_returns_both_sides_to_room_and_heals_all() {
        let mut host = game(Species::Blastoise, 100, MoveId::Surf, 100);
        let mut peer = game(Species::Rattata, 5, MoveId::Tackle, 1);
        let (ta, tb) = ChannelTransport::new_pair();
        host.attach_link_transport(Box::new(ta), LinkRole::Host);
        peer.attach_link_transport(Box::new(tb), LinkRole::Guest);
        let idle = InputState::new();
        let mut a = InputState::new(); a.press(GbButton::A);
        for _ in 0..20 { host.update(&idle); peer.update(&idle); }
        let request = host.link_cable.on_gameboy_used(MapId::Colosseum);
        host.handle_flow_need(request);
        host.update(&a);
        for _ in 0..100 { host.update(&idle); peer.update(&idle); }
        assert_eq!(peer.link_cable.phase(), &CableClubPhase::InRoom);
        let request = peer.link_cable.on_gameboy_used(MapId::Colosseum);
        peer.handle_flow_need(request);
        peer.update(&a);
        for frame in 0..1200 {
            for g in [&mut host, &mut peer] {
                let ready = matches!(g.state.screen, GameScreen::Battle)
                    && g.battle.phase == BattlePhase::PlayerMenu;
                g.update(if !ready && frame % 2 == 0 { &a } else { &idle });
            }
            if host.battle.phase == BattlePhase::PlayerMenu && peer.battle.phase == BattlePhase::PlayerMenu { break; }
        }
        for g in [&mut host, &mut peer] {
            assert_eq!(g.state.screen, GameScreen::Battle);
            assert_eq!(g.battle.phase, BattlePhase::PlayerMenu);
            let party = &g.battle.battle_state.as_ref().unwrap().player.party;
            assert_eq!(party[0].status, StatusCondition::Burn);
            assert_eq!(party[0].pp[0], 2, "no prebattle PP heal");
            assert_eq!(party[1].hp, 0, "no prebattle revival");
            g.update(&a); // FIGHT -> move menu
        }
        host.update(&a); // Host commits, guest still choosing.
        for _ in 0..100 { host.update(&idle); peer.update(&idle); }
        assert_eq!(host.battle.phase, BattlePhase::LinkWaiting);
        assert_eq!(peer.battle.phase, BattlePhase::MoveSelect);
        peer.update(&a);
        for frame in 0..2400 {
            for g in [&mut host, &mut peer] {
                g.update(if matches!(g.state.screen, GameScreen::Battle) && frame % 2 == 0 { &a } else { &idle });
            }
            if host.state.screen == GameScreen::Overworld && peer.state.screen == GameScreen::Overworld { break; }
        }
        // A queued ordinary blackout can leave the room only AFTER the
        // Battle -> Overworld transition. Let the real fade/warp loop run.
        for _ in 0..120 { host.update(&idle); peer.update(&idle); }
        for g in [&host, &peer] {
            assert_eq!(g.state.screen, GameScreen::Overworld, "terminal mirror must leave LinkWaiting");
            assert_eq!(g.overworld.state.current_map, MapId::Colosseum);
            assert_eq!(g.link_cable.phase(), &CableClubPhase::InRoom);
            assert!(g.overworld.pending_warp.is_none());
            for mon in g.save_data.party.iter() {
                assert_eq!(mon.hp, mon.max_hp);
                assert_eq!(mon.status, StatusCondition::None);
                assert_eq!(mon.pp[0], pokered_core::pokemon::move_learning::get_move_max_pp(mon.moves[0]));
            }
        }
    }
}


#[cfg(all(test, not(target_os = "none"), not(target_arch = "wasm32")))]
mod link_trade_movie_name_fidelity_tests {
    use super::*;
    use pokered_core::link::link_trade::{LinkTradeDriver, LinkTradePollResult};
    use pokered_core::link::protocol::NetworkMessage;
    use pokered_core::link::transport::ChannelTransport;
    use pokered_core::pokemon::party::Party;
    use pokered_core::trade::TradeAnimPhase;
    use pokered_data::species::Species;

    #[test]
    fn completed_channel_trade_movie_uses_the_received_peer_name() {
        let (mut local_wire, mut remote_wire) = ChannelTransport::<NetworkMessage>::new_pair();
        let pikachu = pokered_core::pokemon::stats::create_pokemon(Species::Pikachu, 20, [0x99, 0x88]).unwrap();
        let charmander = pokered_core::pokemon::stats::create_pokemon(Species::Charmander, 20, [0x99, 0x88]).unwrap();
        let mut local = LinkTradeDriver::new(Party::from(vec![pikachu]), 1)
            .with_trainer_name("RED".to_string());
        let mut remote = LinkTradeDriver::new(Party::from(vec![charmander]), 2)
            .with_trainer_name("GREEN".to_string());
        local.request_trade(&mut local_wire).unwrap();
        assert_eq!(remote.poll(&mut remote_wire), LinkTradePollResult::TradeRequested);
        remote.accept_trade(&mut remote_wire).unwrap();
        assert_eq!(local.poll(&mut local_wire), LinkTradePollResult::TradeAccepted);
        assert_eq!(local.remote_name(), "GREEN");
        local.select_mon(&mut local_wire, 0).unwrap();
        assert_eq!(remote.poll(&mut remote_wire), LinkTradePollResult::PeerSelectedMon(0));
        remote.select_mon(&mut remote_wire, 0).unwrap();
        assert!(matches!(local.poll(&mut local_wire), LinkTradePollResult::BothSelected { .. }));
        local.confirm_trade(&mut local_wire).unwrap();
        assert_eq!(remote.poll(&mut remote_wire), LinkTradePollResult::PeerConfirmed);
        remote.confirm_trade(&mut remote_wire).unwrap();
        assert_eq!(local.poll(&mut local_wire), LinkTradePollResult::PeerConfirmed);
        assert!(matches!(remote.poll(&mut remote_wire), LinkTradePollResult::TradeExecute { .. }));
        assert!(matches!(local.poll(&mut local_wire), LinkTradePollResult::TradeExecute { .. }));

        let mut game = PokemonGame::new(GameVersion::Red);
        game.audio = None;
        game.state.config.language = pokered_core::game_state::Lang::En;
        game.player_name = "RED".to_string();
        game.link_trade = Some(local);
        game.start_link_trade_anim();
        let anim = game.trade_anim.as_mut().unwrap();
        assert_eq!((anim.give, anim.receive), (Species::Pikachu, Species::Charmander));
        while anim.phase() != TradeAnimPhase::TextWentTo { anim.tick(); }
        assert_eq!(anim.text_lines(), Some(("PIKACHU went".to_string(), "to GREEN.".to_string())));
        while anim.phase() != TradeAnimPhase::TextForSends { anim.tick(); }
        for _ in 0..80 { anim.tick(); }
        assert_eq!(anim.text_lines(), Some(("GREEN sends".to_string(), "CHARMANDER.".to_string())));
        while anim.phase() != TradeAnimPhase::TextFarewell { anim.tick(); }
        assert_eq!(anim.text_lines(), Some(("GREEN waves".to_string(), "farewell as".to_string())));
    }
}

#[cfg(test)]
#[path = "game/shared_runtime_regressions.rs"]
mod tui_runtime_regressions;

#[cfg(all(test, feature = "debug-server"))]
mod gift_dialogue_debug_tests {
    use super::*;
    #[test]
    fn control_ready_waits_for_boulder_slide_dust_and_graphics_restore() {
        use pokered_core::overworld::{Direction,OverworldScreen};
        use pokered_data::impl_traits::PokemonRedData;
        let mut game=PokemonGame::new_with_options(
            GameVersion::Red,None,None,None,false,None,false,true,None,
        );
        game.state.screen=GameScreen::Overworld;
        game.overworld=OverworldScreen::new(MapId::SeafoamIslands1F,None,PokemonRedData);
        game.overworld.run_on_load();
        let idle=InputState::new();
        for _ in 0..120 {game.update(&idle);}
        game.overworld.state.player.x=18;game.overworld.state.player.y=9;
        game.overworld.state.player.facing=Direction::Down;
        game.overworld.strength_active=true;
        // This fixture tests push/control timing, not the cave's random
        // encounter roll during a turn. Keep it encounter-free, as in the
        // original-ROM boulder timing probe, without bypassing the push.
        game.overworld.state.encounter_cooldown=255;
        game.overworld.set_rng_seed(0);
        let mut down=InputState::new();down.press(GbButton::Down);
        for _ in 0..10 {
            game.update(&down);
            if game.overworld.boulder_push.is_some() {break;}
        }
        assert!(game.overworld.boulder_push.is_some());
        let mut elapsed=0;
        while game.overworld.boulder_push.is_some() {
            let completed=game.overworld.boulder_push.unwrap().frame>=
                pokered_core::overworld::presentation::BoulderPushState::COMPLETION_FRAME;
            assert_eq!(game.debug_condition_met("control_ready"),completed,
                "logical control resumes before the last LCD image");
            game.update(&idle);elapsed+=1;assert!(elapsed<=72);
        }
        assert!(elapsed>=71);
        assert!(game.debug_condition_met("control_ready"));
    }

    #[test]
    fn control_ready_waits_for_the_entire_arrival_door_step() {
        let mut game = PokemonGame::new_with_options(
            GameVersion::Red, None, None, None, false, None, false, true, None,
        );
        game.state.screen = GameScreen::Overworld;
        game.overworld = OverworldScreen::new(
            pokered_data::maps::MapId::RocketHideoutB3F,
            None,
            pokered_data::impl_traits::PokemonRedData,
        );
        game.overworld.state.player.x = 19;
        game.overworld.state.player.y = 18;
        game.overworld.state.standing_on_door = true;
        assert!(!game.debug_condition_met("control_ready"));
        game.update(&InputState::new());
        assert!(game.overworld.state.exiting_door);
        assert!(!game.debug_condition_met("control_ready"));
        for _ in 0..32 {
            game.update(&InputState::new());
        }
        assert_eq!((game.overworld.state.player.x, game.overworld.state.player.y), (19, 19));
        assert!(game.debug_condition_met("control_ready"));
    }

    #[test]
    fn skip_dialogue_stops_at_gift_question_without_selecting_yes() {
        let mut game = PokemonGame::new_with_options(
            GameVersion::Red,
            None,
            None,
            None,
            false,
            None,
            false,
            true,
            None,
        );
        game.state.screen = GameScreen::Overworld;
        game.save_data = SaveData::new();
        game.overworld = OverworldScreen::new(
            pokered_data::maps::MapId::CeladonMansionRoofHouse,
            None,
            pokered_data::impl_traits::PokemonRedData,
        );
        game.overworld.state.player.x = 4;
        game.overworld.state.player.y = 4;
        game.overworld.state.player.facing = pokered_core::overworld::Direction::Up;
        game.update(&InputState::new());
        let mut a = InputState::new();
        a.press(GbButton::A);
        for _ in 0..2 { game.update(&a); }
        for _ in 0..10 {
            for _ in 0..8 {
                game.update(&InputState::new());
            }
            let response = game.handle_debug_command(pokered_debug_server::DebugCommand::Game(
                pokered_debug_server::GameDebugCommand::SkipDialogue,
            ));
            assert!(response.ok);
            if game.overworld.pending_choice.is_some() {
                break;
            }
        }
        assert_eq!(
            game.overworld
                .pending_choice
                .as_ref()
                .expect("nickname prompt")
                .options,
            ["YES", "NO"]
        );
        assert!(!game.overworld.is_naming_screen_active());
        assert!(game.save_data.party.is_empty());
    }
}

#[cfg(all(test, feature = "debug-server"))]
#[path = "fidelity_stdio.rs"]
mod fidelity_stdio;

#[cfg(all(test, not(target_os = "none"), not(target_arch = "wasm32")))]
mod link_stats_cry_fidelity_tests {
    use super::*;
    use pokered_core::link::transport::ChannelTransport;
    use pokered_core::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::{moves::MoveId, species::Species};

    // These fixtures hold two full games plus persisted snapshots (and a
    // third game when checking Continue). The unoptimized test build needs
    // more stack than Rust's default 2 MiB worker; production is unchanged.
    fn run_link_save_fixture(test: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .name("linked-game-save-regression".to_string())
            .stack_size(16 * 1024 * 1024)
            .spawn(test)
            .unwrap()
            .join()
            .unwrap();
    }

    fn fixture(
        species: Species,
        x: u16,
        facing: pokered_core::overworld::Direction,
    ) -> PokemonGame {
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
        g.audio = Some(AudioOutput::new_pcm());
        let first_move = if species == Species::Pikachu { MoveId::Thundershock } else { MoveId::Tackle };
        let mon = create_pokemon_with_moves(
            species,
            25,
            [0x99, 0x88],
            [first_move, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap();
        g.save_data.party = pokered_core::pokemon::party::Party::from(vec![mon]);
        g.state.screen = GameScreen::Overworld;
        g.main_menu.last_choice = Some(pokered_core::game_state::MainMenuChoice::Continue);
        g.overworld = OverworldScreen::new(MapId::TradeCenter, None, PokemonRedData);
        g.overworld.run_on_load();
        // Room fixtures satisfy the receptionist's Pokédex prerequisite.
        g.overworld.set_event_flag_live(pokered_data::event_flags::EventFlag::EVENT_GOT_POKEDEX);
        g.save_data.game_data.event_flags = g.overworld.unified_flags().as_bytes().to_vec();
        g.overworld.set_rng_seed(0);
        g.external_saves = true;
        let mut committed = g.save_data.clone();
        committed.game_data.position.map_id = MapId::ViridianPokecenter as u8;
        committed.game_data.position.x = 11;
        committed.game_data.position.y = 3;
        g.committed_save = Some(serde_json::to_string(&MobileSave {
            version: 1, data: committed, flags: g.overworld.script_flags()
        }).unwrap());
        g.overworld.state.player.x = x;
        g.overworld.state.player.y = 4;
        g.overworld.state.player.facing = facing;
        g
    }
    fn button(b: GbButton) -> InputState {
        let mut input = InputState::new();
        input.press(b);
        input
    }
    fn channels(g: &PokemonGame) -> Vec<bool> {
        let m = g.audio.as_ref().unwrap().manager.lock().unwrap();
        (0..4)
            .map(|ch| m.sequencer.is_sfx_channel_active(ch))
            .collect()
    }

    fn paired_trade_room() -> (PokemonGame, PokemonGame) {
        use pokered_core::overworld::Direction;
        linked_trade_room(
            fixture(Species::Bulbasaur, 3, Direction::Right),
            fixture(Species::Pikachu, 6, Direction::Left),
        )
    }

    fn linked_trade_room(
        mut host: PokemonGame,
        mut peer: PokemonGame,
    ) -> (PokemonGame, PokemonGame) {
        let (a, b) = ChannelTransport::new_pair();
        host.attach_link_transport(Box::new(a), LinkRole::Host);
        peer.attach_link_transport(Box::new(b), LinkRole::Guest);
        let idle = InputState::new();
        for _ in 0..120 {
            host.update(&idle);
            peer.update(&idle);
        }
        assert_eq!(host.link_cable.phase(), &CableClubPhase::InRoom);
        host.update(&button(GbButton::A)); // Actual table/sign interaction.
        host.update(&idle); host.update(&idle);
        assert!(matches!(host.link_cable.phase(), CableClubPhase::JustAMoment { .. }));
        host.update(&button(GbButton::A)); // Close Just a moment.
        for _ in 0..120 { host.update(&idle); peer.update(&idle); }
        assert_eq!(peer.link_cable.phase(), &CableClubPhase::InRoom);
        peer.update(&button(GbButton::A)); // The guest's own gameboy.
        peer.update(&idle); peer.update(&idle); // Drain the real scene's linkStart.
        assert!(matches!(peer.link_cable.phase(), CableClubPhase::JustAMoment { .. }),
            "phase={:?}, position=({},{}), facing={:?}, dialogue={}, choice={}, request={}",
            peer.link_cable.phase(), peer.overworld.state.player.x, peer.overworld.state.player.y,
            peer.overworld.state.player.facing, peer.overworld.pending_dialogue.is_some(),
            peer.overworld.pending_choice.is_some(), peer.overworld.link_start_requested);
        peer.update(&button(GbButton::A));
        for _ in 0..120 { host.update(&idle); peer.update(&idle); }
        assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeSelect);
        assert_eq!(peer.link_cable.phase(), &CableClubPhase::TradeSelect);
        (host, peer)
    }

    #[test]
    fn actual_trade_peer_request_does_not_open_menu_away_from_gameboy() {
        run_link_save_fixture(|| {
            use pokered_core::overworld::Direction;
            let mut host = fixture(Species::Bulbasaur, 3, Direction::Right);
            let mut peer = fixture(Species::Pikachu, 7, Direction::Down);
            peer.overworld.state.player.y = 6;
            let (a, b) = ChannelTransport::new_pair();
            host.attach_link_transport(Box::new(a), LinkRole::Host);
            peer.attach_link_transport(Box::new(b), LinkRole::Guest);
            let idle = InputState::new();
            for _ in 0..120 { host.update(&idle); peer.update(&idle); }
            assert_eq!(peer.link_cable.phase(), &CableClubPhase::InRoom);
            host.update(&button(GbButton::A));
            host.update(&idle); host.update(&idle); host.update(&button(GbButton::A));
            for _ in 0..120 { host.update(&idle); peer.update(&idle); }
            assert_eq!(host.link_cable.phase(), &CableClubPhase::WaitingResponse { kind: crate::link::cable_club::LinkKind::Trade });
            assert_eq!(peer.link_cable.phase(), &CableClubPhase::InRoom,
                "original waits for each player to use their gameboy; peer request cannot seize overworld input");
            assert_eq!(peer.link_cable.text_box(), None);
            assert_eq!(peer.link_trade.as_ref().unwrap().state(), &pokered_core::link::link_trade::LinkTradeState::PeerRequestedTrade,
                "request really arrived; absence of a prompt is not a missing network message");
            peer.update(&button(GbButton::A));
            for _ in 0..3 { peer.update(&idle); host.update(&idle); }
            assert_eq!(peer.link_cable.phase(), &CableClubPhase::InRoom,
                "A away from the gameboy cannot accept the pending request");
            for (key, target) in [(GbButton::Left, (6, 6)), (GbButton::Up, (6, 4))] {
                for _ in 0..80 {
                    let moving = peer.overworld.state.player.movement_state != dotzuki_engine::overworld::MovementState::Idle;
                    let walk_input = button(key);
                    peer.update(if moving { &idle } else { &walk_input }); host.update(&idle);
                    if (peer.overworld.state.player.x, peer.overworld.state.player.y) == target
                        && peer.overworld.state.player.movement_state == dotzuki_engine::overworld::MovementState::Idle { break; }
                }
                for _ in 0..20 { peer.update(&idle); host.update(&idle); }
                assert_eq!((peer.overworld.state.player.x, peer.overworld.state.player.y), target,
                    "incoming request must leave real walking input available");
            }
            for _ in 0..2 { peer.update(&button(GbButton::Left)); host.update(&idle); }
            for _ in 0..20 { peer.update(&idle); host.update(&idle); }
            assert_eq!((peer.overworld.state.player.x, peer.overworld.state.player.y), (6, 4));
            assert_eq!(peer.overworld.state.player.facing,pokered_core::overworld::Direction::Left);
            let mut held_a=button(GbButton::A);
            peer.update(&held_a);host.update(&idle);
            held_a.begin_frame();
            peer.update(&held_a);host.update(&idle);
            for _ in 0..2 { peer.update(&idle); host.update(&idle); }
            assert!(matches!(peer.link_cable.phase(), CableClubPhase::JustAMoment { .. }));
            peer.update(&button(GbButton::A)); host.update(&idle);
            for _ in 0..79 { peer.update(&idle); host.update(&idle); }
            assert!(matches!(peer.link_cable.phase(), CableClubPhase::GameboyDelay { frames_left: 1, .. }),
                "serial exchange must not start before the original 80-frame wait");
            assert_eq!(peer.link_trade.as_ref().unwrap().state(), &pokered_core::link::link_trade::LinkTradeState::PeerRequestedTrade);
            peer.update(&idle); host.update(&idle);
            for _ in 0..20 { peer.update(&idle); host.update(&idle); }
            assert_eq!(peer.link_cable.phase(), &CableClubPhase::TradeSelect);
            assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeSelect);
        });
    }

    fn actual_npc_trade_messages(map: MapId, x: u16, y: u16, species: Species, done_flag: Option<&str>, refuse: bool) -> (Vec<String>, Species) {
        use pokered_core::overworld::Direction;
        let mut game = fixture(species, x, Direction::Up);
        game.audio = None;
        game.overworld = OverworldScreen::new(map, None, PokemonRedData);
        game.overworld.state.player.x = x;
        game.overworld.state.player.y = y;
        game.overworld.state.player.facing = Direction::Up;
        game.overworld.run_on_load();
        for npc in &mut game.overworld.npc_states {
            npc.movement_type = pokered_core::overworld::NpcMovementType::Stationary;
        }
        if let Some(flag) = done_flag { game.overworld.set_flag_live(flag, true); }
        let idle = InputState::new();
        for _ in 0..120 { game.update(&idle); }
        game.update(&button(GbButton::A));
        let mut messages = Vec::new();
        let mut last = None;
        let mut started = false;
        for frame in 0..16000 {
            if let Some(dialogue) = &game.overworld.pending_dialogue {
                started = true;
                let text = dialogue.pages().iter().map(|p|format!("{} {}",p.line1,p.line2)).collect::<Vec<_>>().join(" ");
                if last.as_ref() != Some(&text) { messages.push(text.clone()); last = Some(text); }
            } else { last = None; }
            if started && game.overworld.script_engine_idle() && game.overworld.pending_dialogue.is_none()
                && game.overworld.pending_choice.is_none() && game.pending_trade.is_none() && game.trade_anim.is_none() {
                return (messages, game.save_data.party.get(0).unwrap().species);
            }
            let input = if frame % 20 == 0 { button(if refuse && game.overworld.pending_choice.is_some() { GbButton::B } else { GbButton::A }) } else { InputState::new() };
            game.update(&input);
        }
        panic!("actual NPC trade did not finish: {messages:?}");
    }

    #[test]
    fn actual_npc_trade_summary_precedes_npc_thanks() {
        run_link_save_fixture(|| {
            let (messages, received) = actual_npc_trade_messages(MapId::Route2TradeHouse, 4, 2, Species::Abra, None, false);
            assert_eq!(received, Species::MrMime, "trade completes through the real party selector and movie");
            let summary = messages.iter().position(|m|m.contains("traded ABRA")).unwrap();
            let thanks = messages.iter().position(|m|m.contains("Hey thanks!")).unwrap();
            assert!(summary < thanks, "DoInGameTradeDialogue prints TradedForText then TRADETEXT_THANKS: {messages:?}");
        });
    }

    #[test]
    fn actual_all_npc_trade_branches_match_original_dialog_sets() {
        run_link_save_fixture(|| {
            let mut results = Vec::new();
            for (map,x,y,give,receive,style,flag) in [
                (MapId::Route11Gate2F,4,3,Species::Nidorino,Species::Nidorina,1,"EVENT_TRADED_FOR_TERRY"),
                (MapId::Route2TradeHouse,4,2,Species::Abra,Species::MrMime,1,"EVENT_TRADED_FOR_MARCEL"),
                (MapId::CinnabarLabFossilRoom,7,7,Species::Ponyta,Species::Seel,1,"EVENT_TRADED_FOR_SAILOR"),
                (MapId::VermilionTradeHouse,3,6,Species::Spearow,Species::Farfetchd,3,"EVENT_TRADED_FOR_DUX"),
                (MapId::Route18Gate2F,4,3,Species::Slowbro,Species::Lickitung,1,"EVENT_GOT_LICKITUNG_FROM_TRADE"),
                (MapId::CeruleanTradeHouse,1,3,Species::Poliwhirl,Species::Jynx,2,"EVENT_TRADED_FOR_LOLA"),
                (MapId::CinnabarLabTradeRoom,1,5,Species::Raichu,Species::Electrode,2,"EVENT_TRADED_FOR_DORIS"),
                (MapId::CinnabarLabTradeRoom,5,6,Species::Venonat,Species::Tangela,3,"EVENT_TRADED_FOR_CRINKLES"),
                (MapId::UndergroundPathRoute5,2,4,Species::NidoranM,Species::NidoranF,3,"EVENT_TRADED_FOR_SPOT"),
            ] {
                let (intro,thanks,no,wrong,after) = match style {
                    1 => ("I'm looking for", "Hey thanks!", "Awww!", "What? That's not", "Isn't my old"),
                    2 => ("Hello there!", "Thanks!", "Well, if you don't want", "Hmmm? This isn't", "went and evolved!"),
                    _ => ("Hi! Do you have", "Thanks pal!", "That's too bad.", "...This is no", "How is my old"),
                };
                let (success,received) = actual_npc_trade_messages(map,x,y,give,None,false);
                let summary = success.iter().position(|m|m.contains(" traded "));
                let gratitude = success.iter().position(|m|m.contains(thanks));
                let (bad,unchanged) = actual_npc_trade_messages(map,x,y,Species::Bulbasaur,None,false);
                let (declined,untraded) = actual_npc_trade_messages(map,x,y,give,None,true);
                let (later,_) = actual_npc_trade_messages(map,x,y,give,Some(flag),false);
                let ok = received == receive && success[0].contains(intro)
                    && summary.zip(gratitude).is_some_and(|(a,b)|a<b)
                    && unchanged == Species::Bulbasaur && bad.iter().any(|m|m.contains(wrong))
                    && untraded == give && declined.iter().any(|m|m.contains(no))
                    && later.iter().any(|m|m.contains(after)) && !later.iter().any(|m|m.contains(intro))
                    && (give != Species::NidoranM || (success[0].contains("NIDORAN♂") && success[0].contains("NIDORAN♀")));
                results.push((map,give,ok,success,bad,declined,later));
            }
            assert!(results.iter().all(|r|r.2), "original nine trade dialog sets / input branches: {results:?}");
        });
    }

    #[test]
    fn actual_gameboy_rejects_wrong_role_and_vertical_facing() {
        run_link_save_fixture(|| {
            use pokered_core::overworld::Direction;
            let mut observed = Vec::new();
            for (role, x, y, facing) in [
                (LinkRole::Host, 6, 4, Direction::Left),
                (LinkRole::Guest, 3, 4, Direction::Right),
                (LinkRole::Host, 4, 5, Direction::Up),
                (LinkRole::Guest, 5, 5, Direction::Up),
            ] {
                let mut local = fixture(Species::Bulbasaur, x, facing);
                local.overworld.state.player.y = y;
                let mut peer = fixture(Species::Pikachu, 7, Direction::Down);
                peer.overworld.state.player.y = 6;
                let (a, b) = ChannelTransport::new_pair();
                local.attach_link_transport(Box::new(a), role);
                peer.attach_link_transport(Box::new(b), if role == LinkRole::Host { LinkRole::Guest } else { LinkRole::Host });
                let idle = InputState::new();
                for _ in 0..120 { local.update(&idle); peer.update(&idle); }
                local.update(&button(GbButton::A));
                for _ in 0..20 { local.update(&idle); peer.update(&idle); }
                observed.push((role, x, y, facing, local.link_cable.phase().clone(),
                    peer.link_cable.phase().clone(), local.link_cable.text_box()));
            }
            assert!(observed.iter().all(|r| r.4 == CableClubPhase::InRoom && r.5 == CableClubPhase::InRoom && r.6.is_none()),
                "wrong gameboy/facing results: {observed:?}");
        });
    }

    #[test]
    fn actual_receptionist_enters_role_specific_room_coordinates() {
        run_link_save_fixture(|| {
            use pokered_core::overworld::Direction;
            for role in [LinkRole::Host, LinkRole::Guest] {
                let mut local = fixture(Species::Bulbasaur, 11, Direction::Up);
                let mut peer = fixture(Species::Pikachu, 11, Direction::Up);
                for g in [&mut local, &mut peer] {
                    g.overworld = OverworldScreen::new(MapId::ViridianPokecenter, None, PokemonRedData);
                    g.overworld.set_event_flag_live(pokered_data::event_flags::EventFlag::EVENT_GOT_POKEDEX);
                    g.overworld.state.player.x = 11; g.overworld.state.player.y = 3;
                    g.overworld.state.player.facing = Direction::Up;
                    g.overworld.run_on_load();
                }
                let (a,b) = ChannelTransport::new_pair();
                local.attach_link_transport(Box::new(a), role);
                peer.attach_link_transport(Box::new(b), if role == LinkRole::Host { LinkRole::Guest } else { LinkRole::Host });
                let idle = InputState::new();
                for _ in 0..120 { local.update(&idle); peer.update(&idle); }
                local.update(&button(GbButton::A)); peer.update(&idle);
                for frame in 0..2000 {
                    if matches!(local.link_cable.phase(), CableClubPhase::ReceptionSave { .. }) { break; }
                    let advance = button(GbButton::A);
                    local.update(if frame % 20 == 19 { &advance } else { &idle });
                    peer.update(&idle);
                }
                assert!(matches!(local.link_cable.phase(), CableClubPhase::ReceptionSave { .. }), "real receptionist save prompt");
                local.update(&button(GbButton::A)); peer.update(&idle);
                assert!(matches!(local.link_cable.phase(), CableClubPhase::ReceptionMenu { selected: 0 }));
                local.update(&idle); local.update(&button(GbButton::A)); peer.update(&idle);
                for _ in 0..1000 {
                    local.update(&idle); peer.update(&idle);
                    if local.overworld.state.current_map == MapId::TradeCenter { break; }
                }
                assert_eq!(local.overworld.state.current_map, MapId::TradeCenter);
                for _ in 0..120 { local.update(&idle); peer.update(&idle); }
                let expected = if role == LinkRole::Host { (3,4) } else { (6,4) };
                assert_eq!((local.overworld.state.player.x, local.overworld.state.player.y), expected,
                    "special_warps.asm assigns each clocking role its own table end");
                let committed: MobileSave = serde_json::from_str(&local.export_mobile_save().unwrap()).unwrap();
                assert_eq!(committed.data.game_data.position.map_id, MapId::ViridianPokecenter as u8);
                assert_eq!((committed.data.game_data.position.x, committed.data.game_data.position.y), (11, 3),
                    "room entry preserves the receptionist's committed save position");
                let npc = local.overworld.npc_states.iter().find(|n| n.text_id == 1).unwrap();
                let opposite = if role == LinkRole::Host { (6,4) } else { (3,4) };
                assert_eq!((npc.x,npc.y), opposite, "TradeCenter_Script NPC bytes subtract the object +4 offset");
            }

        });
    }

    #[test]
    fn actual_trade_room_peer_stats_plays_selected_species_cry_once() {
        let (mut host, mut peer) = paired_trade_room();
        let idle = InputState::new();
        host.update(&button(GbButton::Right));
        host.update(&idle);
        assert_eq!(host.link_cable.peer_cursor(), Some(0));
        assert_eq!(channels(&host), vec![false; 4]);
        host.update(&button(GbButton::A));
        assert_eq!(
            host.link_cable.stats().unwrap().pokemon().species,
            Species::Pikachu
        );
        assert_eq!(host.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id, SfxId::PressAB as u8);
        for frame in 1..=47 {
            host.update(&idle);
            if frame < 47 { assert_ne!(host.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id,
                pokered_data::cries::cry_data(Species::Pikachu).sfx, "cry must follow picture loading"); }
        }
        host.update(&idle);
        let expected = AudioOutput::new_pcm();
        play_species_cry(&expected, Species::Pikachu);
        expected.update_frame();
        let expected_channels = {
            let m = expected.manager.lock().unwrap();
            (0..4)
                .map(|ch| m.sequencer.is_sfx_channel_active(ch))
                .collect::<Vec<_>>()
        };
        println!(
            "link stats first tick actual={:?}, expected={:?}",
            channels(&host),
            expected_channels
        );
        assert_eq!(channels(&host), expected_channels);
        // Compare cry channels' APU registers, excluding the background wave
        // channel. The selected species controls both pitch and duration.
        for address in (0xFF10..=0xFF19).chain(0xFF20..=0xFF23) {
            let actual = host
                .audio
                .as_ref()
                .unwrap()
                .manager
                .lock()
                .unwrap()
                .apu
                .read_register(address);
            let reference = expected.manager.lock().unwrap().apu.read_register(address);
            assert_eq!(actual, reference, "APU {address:#x}");
        }
        let mut actual_ticks = 1;
        while host.audio.as_ref().unwrap().is_sfx_playing() && actual_ticks < 600 {
            host.update(&idle);
            peer.update(&idle);
            actual_ticks += 1;
        }
        let mut expected_ticks = 1;
        while expected.is_sfx_playing() && expected_ticks < 600 {
            expected.update_frame();
            expected_ticks += 1;
        }
        assert_eq!(actual_ticks, expected_ticks);
        println!("selected Pikachu cry ticks={actual_ticks}");
        assert_eq!(channels(&host), vec![false; 4]);
        host.update(&button(GbButton::B));
        assert_eq!(
            host.link_cable.stats().unwrap().page(),
            pokered_core::stats_screen::StatsPage::Moves
        );
        host.update(&idle);
        assert_eq!(
            channels(&host),
            vec![false; 4],
            "page advance must not repeat cry"
        );
        host.update(&button(GbButton::A));
        assert!(host.link_cable.stats().is_none());
        assert_eq!(host.link_cable.peer_cursor(), Some(0));
        assert_eq!(channels(&host), vec![false; 4], "exit must not repeat cry");
    }

    #[test]
    fn actual_own_mon_menu_views_cancels_and_only_trade_sends_selection() {
        let (mut host, mut peer) = paired_trade_room();
        let idle = InputState::new();
        host.update(&button(GbButton::A));
        assert_eq!(host.link_cable.local_action(), Some((0, false)));
        assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeSelect);
        // LEFT on STATS stays on STATS; B cancels only this action menu.
        host.update(&button(GbButton::Left));
        assert_eq!(host.link_cable.local_action(), Some((0, false)));
        host.update(&button(GbButton::B));
        assert_eq!(host.link_cable.local_action(), None);
        peer.update(&idle);
        assert_eq!(peer.link_cable.phase(), &CableClubPhase::TradeSelect);
        host.update(&idle);
        host.update(&button(GbButton::A));
        host.update(&idle);
        host.update(&button(GbButton::A));
        assert_eq!(host.link_cable.local_action(), None);
        assert_eq!(
            host.link_cable.stats().unwrap().pokemon().species,
            Species::Bulbasaur
        );
        assert_eq!(host.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id, SfxId::PressAB as u8);
        for _ in 0..49 { host.update(&idle); }
        assert_eq!(channels(&host), vec![true, true, false, true]);
        for _ in 0..120 {
            host.update(&idle);
            peer.update(&idle);
        }
        host.update(&button(GbButton::B));
        assert_eq!(
            host.link_cable.stats().unwrap().page(),
            pokered_core::stats_screen::StatsPage::Moves
        );
        host.update(&button(GbButton::A));
        assert!(host.link_cable.stats().is_none());
        assert_eq!(host.link_cable.peer_cursor(), None);
        assert_eq!(host.link_cable.party_select().unwrap().cursor(), 0);
        host.update(&idle);
        host.update(&button(GbButton::A));
        host.update(&button(GbButton::Right));
        assert_eq!(host.link_cable.local_action(), Some((0, true)));
        host.update(&idle);
        host.update(&button(GbButton::Right));
        assert_eq!(
            host.link_cable.local_action(),
            Some((0, true)),
            "RIGHT is idempotent"
        );
        host.update(&button(GbButton::Left));
        assert_eq!(host.link_cable.local_action(), Some((0, false)));
        host.update(&button(GbButton::Right));
        host.update(&button(GbButton::A));
        assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeWaitingPeer);
        peer.update(&idle);
        assert_eq!(peer.link_cable.phase(), &CableClubPhase::TradeSelect);
        peer.update(&button(GbButton::A));
        assert_eq!(peer.link_cable.local_action(), Some((0, false)));
        peer.update(&button(GbButton::Right));
        peer.update(&button(GbButton::A));
        for _ in 0..20 {
            host.update(&idle);
            peer.update(&idle);
        }
        assert!(matches!(
            host.link_cable.phase(),
            CableClubPhase::TradeConfirm {
                local_index: 0,
                remote_index: 0,
                ..
            }
        ));
        assert!(matches!(
            peer.link_cable.phase(),
            CableClubPhase::TradeConfirm {
                local_index: 0,
                remote_index: 0,
                ..
            }
        ));
    }

    fn choose_actual_trade(g: &mut PokemonGame) {
        g.update(&button(GbButton::A));
        g.update(&button(GbButton::Right));
        g.update(&button(GbButton::A));
    }

    fn both_selected_pair() -> (PokemonGame, PokemonGame) {
        let (mut host, mut peer) = paired_trade_room();
        choose_actual_trade(&mut host); choose_actual_trade(&mut peer);
        let idle = InputState::new();
        for _ in 0..20 { host.update(&idle); peer.update(&idle); }
        assert!(matches!(host.link_cable.phase(), CableClubPhase::TradeConfirm { .. }));
        assert!(matches!(peer.link_cable.phase(), CableClubPhase::TradeConfirm { .. }));
        (host, peer)
    }

    #[test]
    fn actual_link_trade_evolution_ignores_b_during_morph() {
        run_link_save_fixture(|| {
            use pokered_core::evolution_screen::EvolutionPhase;
            use pokered_core::overworld::Direction;
            let host = fixture(Species::Bulbasaur, 3, Direction::Right);
            let mut peer = fixture(Species::Pikachu, 6, Direction::Left);
            peer.save_data.party = pokered_core::pokemon::party::Party::from(vec![
                create_pokemon_with_moves(Species::Kadabra, 25, [0x99, 0x88],
                    [MoveId::Confusion, MoveId::None, MoveId::None, MoveId::None]).unwrap()
            ]);
            let (mut host, mut peer) = linked_trade_room(host, peer);
            choose_actual_trade(&mut host); choose_actual_trade(&mut peer);
            let idle = InputState::new();
            for _ in 0..20 { host.update(&idle); peer.update(&idle); }
            assert!(matches!(host.link_cable.phase(), CableClubPhase::TradeConfirm { .. }));
            assert!(matches!(peer.link_cable.phase(), CableClubPhase::TradeConfirm { .. }));
            host.update(&button(GbButton::A)); peer.update(&button(GbButton::A));
            let mut saw_morph = false;
            for _ in 0..12000 {
                let phase = host.evolution_anim.as_ref().map(|a| a.phase());
                if phase == Some(EvolutionPhase::Morph) {
                    saw_morph = true;
                    let event = host.evolution_anim.as_ref().unwrap().current().unwrap();
                    assert_eq!(event.name, "KADABRA", "use received mon's name");
                    assert!(event.force, "preserve original wForceEvolution");
                    host.update(&button(GbButton::B));
                } else { host.update(&idle); }
                peer.update(&idle);
                assert_ne!(host.evolution_anim.as_ref().map(|a| a.phase()),
                    Some(EvolutionPhase::StoppedText), "original trade evolution ignores B");
                if saw_morph && host.evolution_anim.is_none() { break; }
            }
            assert!(saw_morph, "real exchange must reach evolution cutscene");
            assert!(host.evolution_anim.is_none(), "cutscene must finish");
            assert_eq!(host.save_data.party.get(0).unwrap().species, Species::Alakazam);
            assert_eq!(peer.save_data.party.get(0).unwrap().species, Species::Bulbasaur);
            assert!(host.save_data.game_data.pokedex.is_owned(Species::Alakazam));
            let before_completion: MobileSave = serde_json::from_str(&host.export_mobile_save().unwrap()).unwrap();
            assert_eq!(before_completion.data.party.get(0).unwrap().species, Species::Bulbasaur,
                "the original partial save follows serial synchronization and completion delay");
            for _ in 0..120 {
                host.update(&idle); peer.update(&idle);
                if host.link_cable.phase() == &CableClubPhase::TradeCompleted { break; }
            }
            assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeCompleted);
            let persisted: MobileSave = serde_json::from_str(&host.export_mobile_save().unwrap()).unwrap();
            assert_eq!(persisted.data.party.get(0).unwrap().species, Species::Alakazam,
                "save occurs after forced evolution, not before: screen={:?}, phase={:?}, move={:?}",
                host.state.screen, host.link_cable.phase(), host.pending_evolve_move_replace);
            assert!(persisted.data.game_data.pokedex.is_owned(Species::Alakazam));
            });
    }

    #[test]
    fn actual_trade_rejection_waits_for_both_choices_then_100_frames() {
        run_link_save_fixture(|| {
            let (mut host, mut peer) = both_selected_pair();
            let host_party = serde_json::to_value(&host.save_data.party).unwrap();
            let peer_party = serde_json::to_value(&peer.save_data.party).unwrap();
            let idle = InputState::new();
            host.update(&button(GbButton::B));
            for _ in 0..120 { host.update(&idle); peer.update(&idle); }
            assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeWaitingConfirm,
                "NO must synchronize with peer's confirmation choice");
            assert!(matches!(peer.link_cable.phase(), CableClubPhase::TradeConfirm { .. }),
                "received NO must not dismiss an unanswered confirmation menu");
            peer.update(&button(GbButton::A));
            host.update(&idle);
            // The confirmed rejection text is a timed delay, not an A/B prompt.
            for _ in 0..10 { host.update(&button(GbButton::A)); peer.update(&button(GbButton::B)); }
            assert_eq!(host.link_cable.text_box(), Some(crate::link::cable_club::TEXT_TRADE_CANCELED.to_string()));
            for _ in 0..89 { host.update(&idle); peer.update(&idle); }
            assert_eq!(host.link_cable.text_box(), Some(crate::link::cable_club::TEXT_TRADE_CANCELED.to_string()),
                "rejection still shows after 99 frames");
            host.update(&idle); peer.update(&idle);
            for g in [&host, &peer] {
                assert_eq!(g.link_cable.phase(), &CableClubPhase::TradeSelect);
                assert_eq!(g.link_cable.text_box(), None, "original DelayFrames100 returns automatically");
            }
            assert_eq!(serde_json::to_value(&host.save_data.party).unwrap(), host_party);
            assert_eq!(serde_json::to_value(&peer.save_data.party).unwrap(), peer_party);
            });
    }

    fn completed_actual_trade() -> (PokemonGame, PokemonGame, String) {
        let (mut host, mut peer) = both_selected_pair();
        // Reception saved in the Pokemon Center before room entry. This
        // fixture retains that committed snapshot while live gameplay trades.
        let mut saved = host.save_data.clone();
        saved.game_data.position.map_id = MapId::ViridianPokecenter as u8;
        saved.game_data.position.x = 11; saved.game_data.position.y = 3;
        let envelope = MobileSave { version: 1, data: saved, flags: host.overworld.script_flags() };
        let before = serde_json::to_string(&envelope).unwrap();
        host.external_saves = true; host.committed_save = Some(before.clone());
        peer.external_saves = true;
        peer.committed_save = Some(serde_json::to_string(&MobileSave {
            version: 1, data: peer.save_data.clone(), flags: peer.overworld.script_flags()
        }).unwrap());
        host.update(&button(GbButton::A)); peer.update(&button(GbButton::A));
        let idle = InputState::new();
        for _ in 0..12000 {
            host.update(&idle); peer.update(&idle);
            if host.link_cable.phase() == &CableClubPhase::TradeCompleted { return (host, peer, before); }
        }
        panic!("actual successful exchange must finish its movie");
    }

    #[test]
    fn actual_trade_completion_waits_for_peer_evolution() {
        run_link_save_fixture(|| {
            use pokered_core::overworld::Direction;
            let host = fixture(Species::Bulbasaur, 3, Direction::Right);
            let peer = fixture(Species::Kadabra, 6, Direction::Left);
            let (mut host, mut peer) = linked_trade_room(host, peer);
            choose_actual_trade(&mut host); choose_actual_trade(&mut peer);
            let idle = InputState::new();
            for _ in 0..20 { host.update(&idle); peer.update(&idle); }
            assert!(matches!(host.link_cable.phase(), CableClubPhase::TradeConfirm { .. }));
            assert!(matches!(peer.link_cable.phase(), CableClubPhase::TradeConfirm { .. }));
            host.update(&button(GbButton::A)); peer.update(&button(GbButton::A));
            let mut saw_evolution = false;
            for _ in 0..12000 {
                host.update(&idle); peer.update(&idle);
                if host.evolution_anim.is_some() {
                    if !saw_evolution {
                        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                        peer.draw(&mut fb);
                        for y in 0..80 { for x in 0..160 {
                            assert_eq!(fb.get_pixel(x,y), Some(Rgba::WHITE),
                                "ClearScreen after the movie must not repaint stale party rows");
                        }}
                    }
                    saw_evolution = true;
                    assert!(!matches!(peer.link_cable.phase(), CableClubPhase::TradeCompleted | CableClubPhase::TradeSelect),
                        "serial completion barrier must hold the non-evolving peer: host evolution={:?}, peer={:?}",
                        host.evolution_anim.as_ref().unwrap().phase(), peer.link_cable.phase());
                }
                if saw_evolution && host.evolution_anim.is_none() { break; }
            }
            assert!(saw_evolution, "actual trade must reach Kadabra evolution");
            for _ in 0..120 {
                host.update(&idle); peer.update(&idle);
                if matches!(host.link_cable.phase(), CableClubPhase::TradeCompletionDelay { frames_left: 40 }) { break; }
            }
            assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeCompletionDelay { frames_left: 40 });
            let persisted: MobileSave = serde_json::from_str(&host.export_mobile_save().unwrap()).unwrap();
            assert_eq!(persisted.data.party.get(0).unwrap().species, Species::Bulbasaur);
            for n in 1..=39 {
                host.update(&button(GbButton::A)); peer.update(&button(GbButton::B));
                assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeCompletionDelay { frames_left: 40-n },
                    "buttons cannot shorten the original forty-frame delay");
            }
            host.update(&idle); peer.update(&idle);
            assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeCompleted);
            let persisted: MobileSave = serde_json::from_str(&host.export_mobile_save().unwrap()).unwrap();
            assert_eq!(persisted.data.party.get(0).unwrap().species, Species::Alakazam,
                "save exactly after the synchronized completion delay");
        });
    }

    #[test]
    fn actual_trade_post_completed_returns_after_50_frames_without_button() {
        run_link_save_fixture(|| {
            let (mut host, mut peer, _) = completed_actual_trade();
            let idle = InputState::new();
            assert_eq!(host.save_data.party.get(0).unwrap().species, Species::Pikachu);
            for _ in 0..49 { host.update(&idle); peer.update(&idle); }
            assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeCompleted);
            host.update(&idle); peer.update(&idle);
            assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeSelect,
                "original DelayFrames50 returns without A/B");
            });
    }

    #[test]
    fn start_close_is_silent_and_direction_with_b_only_moves_cursor() {
        run_link_save_fixture(|| {
            let mut g = fixture(Species::Bulbasaur, 3, pokered_core::overworld::Direction::Down);
            let idle = InputState::new();
            g.handle_transition(GameScreen::StartMenu);
            for _ in 0..23 { g.update(&idle); }
            g.update(&button(GbButton::Down));
            let mut combined = button(GbButton::Down); combined.press(GbButton::B);
            // Release the preceding Down so both buttons are fresh.
            for _ in 0..3 { g.update(&idle); } g.update(&combined);
            assert_eq!(g.state.screen, GameScreen::StartMenu);
            assert_eq!(g.start_menu.current_item(), pokered_core::start_menu::StartMenuItem::Item);
            assert_eq!(g.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id, SfxId::PressAB as u8);
            for _ in 0..3 { g.update(&idle); }
            g.audio = Some(AudioOutput::new_pcm());
            let previous = g.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id;
            g.update(&button(GbButton::Start));
            assert_eq!(g.state.screen, GameScreen::StartMenu);
            assert!(g.overworld.field_text_restore.is_some());
            for frame in 1..=12 {
                g.update(&idle);
                assert_eq!(g.state.screen, if frame < 12 { GameScreen::StartMenu } else { GameScreen::Overworld });
            }
            assert_eq!(g.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id, previous);
        });
    }

    #[test]
    fn player_walk_defers_ready_random_npcs_but_advances_delay_and_running_steps() {
        use pokered_core::overworld::Direction;
        use pokered_core::snapshot::OverworldSnapshot;
        run_link_save_fixture(|| {
            let oracle: serde_json::Value = serde_json::from_str(
                include_str!("../tests/fixtures/npc-player-walk-132.json")).unwrap();
            for (case, moving, initial_delay) in [("ready", false, 3), ("moving", true, 54), ("ready_zero", false, 2)] {
                let mut g = fixture(Species::Bulbasaur, 3, Direction::Down);
                g.overworld.warp_to_map(MapId::ViridianCity, 20, 30);
                let idle = InputState::new();
                for _ in 0..120 { g.update(&idle); }
                let mut controlled = g.save_data.game_data.clone();
                controlled.player_last_stop_direction = 2;
                controlled.player_moving_direction = 0;
                g.overworld.restore_system_save_state(&controlled);
                // Stage the original pretrigger field-loop phase and turn
                // flag; this room-derived unit fixture has no CPU phase.
                let mut phase = OverworldSnapshot::capture(&g.overworld);
                phase.field_loop_wait = 0;
                phase.check_player_turn = true;
                phase.restore_into(&mut g.overworld);
                let npc = &mut g.overworld.npc_states[0];
                assert_eq!(npc.sprite_id, 4);
                npc.x = 19; npc.y = 29; npc.facing = Direction::Down;
                npc.walk_counter = if moving { 12 } else { 0 };
                // The running-step fixture includes the native preloaded
                // future idle wait. It is unused throughout this first walk.
                npc.delay_counter = initial_delay;
                npc.visible = true;
                if case == "ready_zero" {
                    use dotzuki_engine::overworld::collision::CollisionProvider as _;
                    let map = g.overworld.map_data.as_ref().unwrap();
                    let provider = pokered_core::overworld::collision::PokemonCollisionProvider::new(MapId::ViridianCity, map.tileset);
                    for (x,y) in [(18,29),(20,29),(19,28),(19,30)] {
                        let tile = provider.get_tile_at_position(map.tileset, &map.blocks, map.width, x, y);
                        assert!(pokered_data::collision::is_tile_passable(map.tileset, tile));
                        assert!(!g.overworld.npc_states.iter().any(|n|n.visible && (n.x,n.y)==(x,y)));
                    }
                }
                for _ in 0..4 { g.update(&idle); }
                let mut input = InputState::new();
                for row in oracle["cases"][case].as_array().unwrap() {
                    let t = row["t"].as_i64().unwrap();
                    if t >= 0 {
                        input.begin_frame();
                        if t == 0 { input.press(GbButton::Down); }
                        if t == 16 { input.release(GbButton::Down); }
                        g.update(&input);
                    }
                    let snap = OverworldSnapshot::capture(&g.overworld);
                    let npc = &snap.npc_states[0];
                    let sprite = &snap.npc_sprite_states[0];
                    assert_eq!(u64::from(g.overworld.state.walk_counter), row["player_counter"].as_u64().unwrap(), "player moving={moving} t={t}");
                    assert_eq!(u64::from(npc.walk_counter), row["remaining"].as_u64().unwrap(), "NPC moving={moving} t={t}");
                    // ready_zero permits a random first-step direction.
                    // This test compares gates/counters, not the PRNG stream.
                    if case != "ready_zero" || t < 2 {
                        assert_eq!(npc.facing, Direction::Down, "facing case={case} t={t}");
                    }
                    assert_eq!((npc.x, npc.y), (19, 29), "origin moving={moving} t={t}");
                    if case == "ready" { assert_eq!(u64::from(npc.delay_counter), row["delay"].as_u64().unwrap(), "delay t={t}"); }
                    assert_eq!(u64::from(sprite.phase), row["phase"].as_u64().unwrap(), "phase moving={moving} t={t}");
                    assert_eq!(u64::from(sprite.intra_frame), row["intra"].as_u64().unwrap(), "intra moving={moving} t={t}");
                    if case != "ready_zero" || t < 2 {
                        assert_eq!(u64::from(sprite.pending[1].image), row["raw_image"].as_u64().unwrap(), "image case={case} t={t}");
                    }
                    if t == 10 {
                        // Snapshot the frame where BG has just advanced but
                        // NPC OAM still uses its previous viewport.
                        let decoded: OverworldSnapshot = serde_json::from_str(&serde_json::to_string(&snap).unwrap()).unwrap();
                        let mut live = OverworldScreen::new(MapId::ViridianCity, None, PokemonRedData);
                        let mut restored = OverworldScreen::new(MapId::ViridianCity, None, PokemonRedData);
                        snap.restore_into(&mut live);
                        decoded.restore_into(&mut restored);
                        assert_eq!(restored.ordinary_npc_sprite_pose(0), g.overworld.ordinary_npc_sprite_pose(0));
                        for _ in 0..30 {
                            for screen in [&mut live, &mut restored] {
                                screen.update_frame(dotzuki_engine::overworld::OverworldInput::new(false,false,false,false,false,false,false,false));
                            }
                            assert_eq!(live.ordinary_npc_sprite_pose(0), restored.ordinary_npc_sprite_pose(0));
                            assert_eq!(serde_json::to_value(OverworldSnapshot::capture(&live).npc_camera_state).unwrap(),
                                serde_json::to_value(OverworldSnapshot::capture(&restored).npc_camera_state).unwrap());
                        }
                    }
                }
            }
        });
    }

    #[test]
    fn npc_finished_step_restores_standing_image_while_player_keeps_walking() {
        use pokered_core::overworld::Direction;
        use pokered_core::snapshot::OverworldSnapshot;
        run_link_save_fixture(|| {
            let oracle: serde_json::Value = serde_json::from_str(
                include_str!("../tests/fixtures/npc-finishing-walk-135.json")).unwrap();
            let mut g = fixture(Species::Bulbasaur, 3, Direction::Down);
            g.overworld.warp_to_map(MapId::ViridianCity, 20, 30);
            let idle = InputState::new();
            for _ in 0..120 { g.update(&idle); }
            let mut phase = OverworldSnapshot::capture(&g.overworld);
            phase.field_loop_wait = 0;
            phase.check_player_turn = true;
            phase.player_last_stop_direction = 2;
            phase.player_moving_direction = 0;
            phase.restore_into(&mut g.overworld);
            let npc = &mut g.overworld.npc_states[0];
            assert_eq!(npc.sprite_id, 4);
            npc.x = 19; npc.y = 29; npc.facing = Direction::Down;
            npc.walk_counter = 6;
            // The original chooses 13 after completion; native preloads it.
            npc.delay_counter = 13;
            npc.visible = true;
            for _ in 0..4 { g.update(&idle); }
            let mut input = InputState::new();
            for row in oracle["frames"].as_array().unwrap() {
                let t = row["t"].as_i64().unwrap();
                if t >= 0 {
                    input.begin_frame();
                    if t == 0 { input.press(GbButton::Down); }
                    if t == 16 { input.release(GbButton::Down); }
                    g.update(&input);
                }
                let snap = OverworldSnapshot::capture(&g.overworld);
                let npc = &snap.npc_states[0];
                let sprite = &snap.npc_sprite_states[0];
                assert_eq!(u64::from(g.overworld.state.walk_counter), row["player_counter"].as_u64().unwrap(), "player t={t}");
                assert_eq!(u64::from(npc.walk_counter), row["remaining"].as_u64().unwrap(), "NPC t={t}");
                assert_eq!((npc.x,npc.y), (19,if t < 9 {29} else {30}), "position t={t}");
                if t >= 9 { assert_eq!(u64::from(npc.delay_counter), row["delay"].as_u64().unwrap(), "delay t={t}"); }
                assert_eq!(u64::from(sprite.phase), row["phase"].as_u64().unwrap(), "phase t={t}");
                assert_eq!(u64::from(sprite.intra_frame), row["intra"].as_u64().unwrap(), "intra t={t}");
                assert_eq!(u64::from(sprite.pending[1].image), row["raw_image"].as_u64().unwrap(), "image t={t}");
                if t == 10 {
                    for final_delay in [13, 1] {
                        let mut saved = snap.clone();
                        saved.npc_states[0].delay_counter = final_delay;
                        // Bootstrap the controlled idle delay so its cache
                        // describes the state before the following update.
                        saved.npc_sprite_states[0] = pokered_core::overworld::presentation::NpcSpriteState::from_npc(&saved.npc_states[0]);
                        saved.npc_sprite_states[0].pending[1].image = 3;
                        let mut old = serde_json::to_value(&saved).unwrap();
                        for sprite in old["npc_sprite_states"].as_array_mut().unwrap() {
                            assert!(sprite.as_object_mut().unwrap().remove("last_delay_counter").is_some());
                        }
                        let legacy: OverworldSnapshot = serde_json::from_value(old).unwrap();
                        let mut live = OverworldScreen::new(MapId::ViridianCity, None, PokemonRedData);
                        let mut restored = OverworldScreen::new(MapId::ViridianCity, None, PokemonRedData);
                        saved.restore_into(&mut live);
                        legacy.restore_into(&mut restored);
                        for frame in 0..30 {
                            for screen in [&mut live, &mut restored] {
                                screen.update_frame(dotzuki_engine::overworld::OverworldInput::new(false,false,false,false,false,false,false,false));
                            }
                            let current = OverworldSnapshot::capture(&live);
                            let decoded = OverworldSnapshot::capture(&restored);
                            assert_eq!(serde_json::to_value(&current.npc_sprite_states).unwrap(), serde_json::to_value(&decoded.npc_sprite_states).unwrap(), "legacy delay={final_delay} frame={frame}");
                            assert_eq!(serde_json::to_value(&current.npc_states).unwrap(), serde_json::to_value(&decoded.npc_states).unwrap());
                            if frame == 0 { assert_eq!(current.npc_sprite_states[0].pending[1].image, 0); }
                        }
                    }
                }
            }
        });
    }

    #[test]
    fn npc_field_font_and_start_close_match_original_ram_and_lcd_frames() {
        use pokered_core::overworld::Direction;
        use pokered_core::snapshot::OverworldSnapshot;
        run_link_save_fixture(|| {
          for (x, fixture_json) in [
            (19, include_str!("../tests/fixtures/npc-field-font-outside-127.json")),
            (22, include_str!("../tests/fixtures/npc-field-font-boxed-127.json")),
            (27, include_str!("../tests/fixtures/npc-field-font-offscreen-127.json")),
          ] {
            let oracle: serde_json::Value = serde_json::from_str(fixture_json).unwrap();
            let mut g = fixture(Species::Bulbasaur, 3, Direction::Down);
            g.overworld.warp_to_map(MapId::ViridianCity, 20, 30);
            let idle = InputState::new();
            for _ in 0..120 { g.update(&idle); }
            let mut snapshot = OverworldSnapshot::capture(&g.overworld);
            snapshot.field_loop_wait = 0;
            snapshot.player_last_stop_direction = 2;
            snapshot.player_moving_direction = 0;
            snapshot.restore_into(&mut g.overworld);
            let npc = &mut g.overworld.npc_states[0];
            assert_eq!(npc.sprite_id, 4);
            npc.x = x;
            npc.y = 29;
            npc.facing = Direction::Down;
            npc.walk_counter = 12;
            // Native wandering preloads the next idle delay when a step starts;
            // the original chooses this same 54-tick delay at step completion.
            npc.delay_counter = oracle["future_delay"].as_u64().unwrap() as u16;
            npc.visible = true;
            for _ in 0..4 { g.update(&idle); }
            let mut input = InputState::new();
            for row in oracle["frames"].as_array().unwrap() {
                let t = row["t"].as_i64().unwrap();
                if t >= 0 {
                    input.begin_frame();
                    if t == 0 { input.press(GbButton::Start); }
                    if t == 40 { input.release(GbButton::Start); }
                    if t == 50 { input.press(GbButton::B); }
                    if t == 52 { input.release(GbButton::B); }
                    g.update(&input);
                }
                let snap = OverworldSnapshot::capture(&g.overworld);
                let sprite = &snap.npc_sprite_states[0];
                assert_eq!(u64::from(g.overworld.npc_states[0].walk_counter), row["remaining"].as_u64().unwrap(), "counter t={t}");
                assert_eq!(u64::from(sprite.phase), row["phase"].as_u64().unwrap(), "phase t={t}");
                assert_eq!(u64::from(sprite.intra_frame), row["intra"].as_u64().unwrap(), "intra t={t}");
                assert_eq!(u64::from(sprite.pending[1].image), row["raw_image"].as_u64().unwrap(), "image t={t}");
                let pose = g.overworld.ordinary_npc_sprite_pose(0).unwrap();
                assert_eq!(pose.image != 0xff, row["lcd_visible"].as_bool().unwrap(), "LCD visibility x={x} t={t}");
                if pose.image != 0xff {
                    assert_eq!(i64::from(pose.y - 420), row["lcd_y"].as_i64().unwrap(), "LCD y x={x} t={t}");
                    assert_eq!(pose.rendered_frame(), (row["lcd_frame"].as_u64().unwrap() as usize, row["lcd_flip"].as_bool().unwrap()), "LCD pose x={x} t={t}");
                }
                if t >= 0 {
                    assert_eq!(g.state.screen, if t < 78 { GameScreen::StartMenu } else { GameScreen::Overworld }, "restore t={t}");
                    if (50..78).contains(&t) { assert_eq!(g.overworld.field_text_window_visible(), t < 56, "WY t={t}"); }
                }
                if t == 60 {
                    // Mid-restore JSON must retain independent NPC phase,
                    // intra-frame progress and both pending OAM images.
                    let decoded: OverworldSnapshot = serde_json::from_str(&serde_json::to_string(&snap).unwrap()).unwrap();
                    let mut live = pokered_core::overworld::screen::OverworldScreen::new(MapId::ViridianCity, None, pokered_data::impl_traits::PokemonRedData);
                    let mut restored = pokered_core::overworld::screen::OverworldScreen::new(MapId::ViridianCity, None, pokered_data::impl_traits::PokemonRedData);
                    snap.restore_into(&mut live);
                    decoded.restore_into(&mut restored);
                    for _ in 0..100 {
                        for screen in [&mut live, &mut restored] {
                            if screen.field_text_restore.is_some() {
                                screen.tick_player_presentation_during_ui();
                                screen.tick_field_text_restore();
                            } else { screen.update_frame(dotzuki_engine::overworld::OverworldInput::new(false, false, false, false, false, false, false, false)); }
                        }
                        let a = OverworldSnapshot::capture(&live);
                        let b = OverworldSnapshot::capture(&restored);
                        assert_eq!(serde_json::to_value(a.npc_sprite_states).unwrap(), serde_json::to_value(b.npc_sprite_states).unwrap());
                        assert_eq!(serde_json::to_value(a.npc_states).unwrap(), serde_json::to_value(b.npc_states).unwrap());
                        assert_eq!(serde_json::to_value(a.field_text_restore).unwrap(), serde_json::to_value(b.field_text_restore).unwrap());
                        assert_eq!(serde_json::to_value(a.npc_camera_state).unwrap(), serde_json::to_value(b.npc_camera_state).unwrap());
                    }
                }
            }
          }
        });
    }

    #[test]
    fn walking_and_bicycle_start_menu_drain_sprite_before_standing() {
        use pokered_core::overworld::Direction;
        use pokered_core::snapshot::OverworldSnapshot;
        use dotzuki_engine::overworld::types::TransportMode;
        run_link_save_fixture(|| {
            for bike in [false,true] {
                let mut g=fixture(Species::Bulbasaur,3,Direction::Down);
                g.overworld.warp_to_map(MapId::Route1,12,22);
                // Original Route1 is 20 x 36 walk cells; (12,22..24) is grass.
                let (width, height) = MapId::Route1.dimensions();
                assert!(12 < u16::from(width) * 2 && 24 < u16::from(height) * 2);
                let idle=InputState::new();
                for _ in 0..120 {g.update(&idle);}
                g.overworld.state.player.transport=if bike {TransportMode::Biking} else {TransportMode::Walking};
                g.overworld.state.encounter_cooldown=255;
                // Match the original Continue / actual Bicycle preparation
                // field phase; input below is real hardware-frame input.
                let mut snapshot=OverworldSnapshot::capture(&g.overworld);
                snapshot.field_loop_wait=if bike {1} else {0};
                snapshot.player_last_stop_direction=2;
                snapshot.player_moving_direction=0;
                snapshot.check_player_turn=true;
                snapshot.restore_into(&mut g.overworld);
                let mut input=InputState::new();
                let first_menu=if bike {12} else {19};
                for t in 0..100 {
                    input.begin_frame();
                    if t==0 {input.press(GbButton::Down);}
                    if t==5 {input.press(GbButton::Start);}
                    if t==if bike {16} else {32} {input.release(GbButton::Down);}
                    if t==45 {input.release(GbButton::Start);}
                    g.update(&input);
                    assert_eq!(g.overworld.state.current_map, MapId::Route1);
                    assert!(g.overworld.state.player.x < u16::from(width) * 2);
                    assert!(g.overworld.state.player.y < u16::from(height) * 2);
                    assert_eq!(g.state.screen,if t<first_menu {GameScreen::Overworld} else {GameScreen::StartMenu},"bike={bike} t{t}");
                    if t>=first_menu {
                        // Original opaque sprite capture: odd bicycle step
                        // remains visible for two LCD frames, then stands.
                        let frame=if bike && t<first_menu+2 {3} else {0};
                        assert_eq!(g.overworld.ordinary_player_sprite_frame(),Some((frame,false)),"bike={bike} t{t}");
                    }
                }
            }
        });
    }

    #[test]
    fn empty_party_pokemon_selection_redraws_without_opening_party_or_replaying_start() {
        run_link_save_fixture(|| {
            let mut g = fixture(Species::Bulbasaur, 3, pokered_core::overworld::Direction::Down);
            g.save_data.party = pokered_core::pokemon::party::Party::default();
            let idle = InputState::new();
            g.handle_transition(GameScreen::StartMenu);
            for _ in 0..23 { g.update(&idle); }
            g.update(&button(GbButton::Down));
            for _ in 0..3 { g.update(&idle); }
            assert_eq!(g.start_menu.current_item(), pokered_core::start_menu::StartMenuItem::Pokemon);
            let mut input = button(GbButton::A);
            g.update(&input);
            assert_eq!(g.state.screen, GameScreen::StartMenu);
            assert!(g.start_menu.field_initialization_active());
            assert_eq!(g.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id, SfxId::PressAB as u8);
            for _ in 0..3 { input.begin_frame(); g.update(&input); }
            assert_eq!(g.state.screen, GameScreen::StartMenu);
            assert!(!g.start_menu.field_initialization_active(), "held selecting A must not restart redraw");
        });
    }

    #[test]
    fn party_restore_discards_original_short_down_a_probe_and_accepts_held_down() {
        use pokered_core::start_menu::StartMenuItem;
        run_link_save_fixture(|| {
            for held_down in [false, true] {
                let path = std::env::temp_dir().join(format!(
                    "pokered-party-restore-144-{}-{held_down}.sav", std::process::id()));
                std::fs::write(&path, include_bytes!("../tests/fixtures/party-restore-road-140.sav")).unwrap();
                let mut g = PokemonGame::new_with_options(GameVersion::Red, Some(path.clone()), None, None,
                    false, None, false, true, #[cfg(feature = "debug-server")] None);
                g.audio = Some(AudioOutput::new_pcm());
                let idle = InputState::new();
                let mut saw_continue = false;
                for t in 0..2000 {
                    saw_continue |= g.state.screen == GameScreen::MainMenu;
                    if g.state.screen == GameScreen::Overworld { break; }
                    let a = button(GbButton::A);
                    g.update(if t % 20 == 19 { &a } else { &idle });
                }
                assert!(saw_continue);
                assert_eq!(g.overworld.state.current_map, MapId::ViridianCity);
                assert_eq!((g.overworld.state.player.x, g.overworld.state.player.y), (20, 30));
                for _ in 0..120 { g.update(&idle); }
                let mut input = button(GbButton::Start);
                for _ in 0..40 { g.update(&input); input.begin_frame(); }
                for _ in 0..120 { g.update(&idle); }
                for _ in 0..8 {
                    if g.start_menu.current_item() == StartMenuItem::Pokemon { break; }
                    g.update(&button(GbButton::Up));
                    for _ in 0..121 { g.update(&idle); }
                }
                assert_eq!(g.start_menu.current_item(), StartMenuItem::Pokemon);
                input = button(GbButton::A);
                for _ in 0..2 { g.update(&input); input.begin_frame(); }
                for _ in 0..120 { g.update(&idle); }
                assert_eq!(g.state.screen, GameScreen::PartyScreen);
                // Actual original probe: B 0..1, DOWN 10..11, A 16..17.
                // Short pulses disappear; DOWN held from 10 reaches first
                // Joypad at 40. Closing B must never close START again.
                let mut session = crate::render::session::RenderSession::new();
                let mut retained = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                let mut full = retained.clone();
                let mut scroll = |_: &mut [u8], _: usize, _: usize, _: i32, _: i32, _: u8| {};
                session.render(&mut g, &mut retained, &mut scroll);
                let mut reused = 0;
                input = InputState::new();
                for t in 0..=40 {
                    input.begin_frame();
                    if t == 0 { input.press(GbButton::B); }
                    if t == 2 { input.release(GbButton::B); }
                    if t == 10 { input.press(GbButton::Down); }
                    if t == 12 && !held_down { input.release(GbButton::Down); }
                    if t == 16 { input.press(GbButton::A); }
                    if t == 18 { input.release(GbButton::A); }
                    g.update(&input);
                    assert_eq!(g.state.screen, GameScreen::StartMenu, "t={t}");
                    assert_eq!(g.overworld.field_text_restore.is_some(), t < 37, "t={t}");
                    assert_eq!(g.start_menu.field_initialization_active(), t < 40 || held_down,
                        "held DOWN starts MenuJoypad's next Delay3 at t={t}");
                    assert_eq!(g.start_menu.current_item(),
                        if held_down && t == 40 { StartMenuItem::Item } else { StartMenuItem::Pokemon }, "t={t}");
                    reused += usize::from(matches!(session.render(&mut g, &mut retained, &mut scroll),
                        crate::render::session::FrameUpdate::Reuse));
                    g.draw(&mut full);
                    for y in 0..144 { for x in 0..160 {
                        assert_eq!(retained.get_pixel(x,y), full.get_pixel(x,y),
                            "cached restore: held_down={held_down} t={t} x={x} y={y}");
                    }}
                    assert_eq!(retained.display_palette(), full.display_palette());
                    if t <= 1 {
                        let mut prefix_visible = false;
                        for y in 0..144 { for x in 0..160 {
                            let pixel = full.get_pixel(x,y).unwrap();
                            if t == 0 && y < 16 { prefix_visible |= pixel != Rgba::WHITE; }
                            else { assert_eq!(pixel, Rgba::WHITE, "palette boundary t={t} x={x} y={y}"); }
                        }}
                        if t == 0 { assert!(prefix_visible, "retain initiating frame's party prefix"); }
                    }
                    assert_ne!(g.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id,
                        SfxId::StartMenu as u8, "return must not replay START sound: t={t}");
                }
                assert!(reused > 0, "exercise white-frame cache reuse");
                drop(g);
                std::fs::remove_file(path).unwrap();
            }
        });
    }

    #[test]
    fn field_menu_wait_discards_pulse_then_reads_held_b_and_plays_source_sounds() {
        run_link_save_fixture(|| {
            let mut g=fixture(Species::Bulbasaur,3,pokered_core::overworld::Direction::Down);
            g.handle_transition(GameScreen::StartMenu);
            let mut input=InputState::new();
            for frame in 1..=23 {
                input.begin_frame();
                if frame==11 || frame==15 {input.press(GbButton::B);}
                if frame==12 {input.release(GbButton::B);}
                g.update(&input);
                assert_eq!(g.state.screen, GameScreen::StartMenu,
                    "held B starts the blocking graphic reload at the first menu Joypad");
                assert_eq!(g.overworld.field_text_restore.is_some(), frame == 23);
                let id=g.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id;
                if frame==19 {assert_ne!(id,SfxId::StartMenu as u8);}
                if frame==20 {assert_eq!(id,SfxId::StartMenu as u8);}
                if frame==23 {assert_eq!(id,SfxId::PressAB as u8);}
            }
            for frame in 1..=12 {
                input.begin_frame();
                g.update(&input);
                assert_eq!(g.state.screen, if frame < 12 { GameScreen::StartMenu } else { GameScreen::Overworld },
                    "held closing B cannot shortcut the original graphic reload");
            }
        });
    }


    #[test]
    #[ignore = "START submenu return comparison capture"]
    fn capture_party_return_menu_105() {
        use pokered_core::start_menu::StartMenuItem;
        run_link_save_fixture(|| {
            let dir = std::path::PathBuf::from(std::env::var("FIDELITY_MENU_RETURN_CAPTURE").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let mut g = fixture(Species::Bulbasaur, 3, pokered_core::overworld::Direction::Down);
            let idle = InputState::new();
            g.handle_transition(GameScreen::StartMenu);
            for _ in 0..23 { g.update(&idle); }
            g.update(&button(GbButton::Down));
            for _ in 0..3 { g.update(&idle); }
            assert_eq!(g.start_menu.current_item(), StartMenuItem::Pokemon);
            g.update(&button(GbButton::A));
            assert_eq!(g.state.screen, GameScreen::PartyScreen);
            let mut input = button(GbButton::B);
            g.update(&input);
            assert_eq!(g.state.screen, GameScreen::StartMenu);
            let mut rows = Vec::new();
            for t in -1i32..8 {
                if t >= 0 {
                    input.begin_frame();
                    if t == 0 { input.press(GbButton::Down); }
                    if t == 1 { input.release(GbButton::Down); }
                    g.update(&input);
                }
                let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                g.draw(&mut fb);
                fb.save_png(&dir.join(format!("frame-{:04}.png", t + 1))).unwrap();
                rows.push(serde_json::json!({"t":t,"input_bits":input.raw_current(),
                    "screen":format!("{:?}",g.state.screen),"item":format!("{:?}",g.start_menu.current_item()),
                    "sfx_id":g.audio.as_ref().map(|a|a.manager.lock().unwrap().sequencer.current_sfx_id)}));
            }
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&rows).unwrap()).unwrap();
        });
    }


    #[test]
    #[ignore = "empty-party START menu comparison capture"]
    fn capture_empty_party_menu_106() {
        run_link_save_fixture(|| {
            let dir = std::path::PathBuf::from(std::env::var("FIDELITY_EMPTY_MENU_CAPTURE").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let mut g = fixture(Species::Bulbasaur, 3, pokered_core::overworld::Direction::Down);
            g.save_data.party = pokered_core::pokemon::party::Party::default();
            g.overworld.set_flag_live("EVENT_GOT_POKEDEX", false);
            let mut field = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            g.draw(&mut field); field.save_png(&dir.join("field-before-menu.png")).unwrap();
            let idle = InputState::new();
            g.handle_transition(GameScreen::StartMenu);
            for _ in 0..23 { g.update(&idle); }
            let mut input = InputState::new();
            let mut rows = Vec::new();
            for t in -1i32..6 {
                if t >= 0 {
                    input.begin_frame();
                    if t == 0 { input.press(GbButton::A); }
                    if t == 1 { input.release(GbButton::A); }
                    g.update(&input);
                }
                let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                g.draw(&mut fb); fb.save_png(&dir.join(format!("frame-{:04}.png",t+1))).unwrap();
                rows.push(serde_json::json!({"t":t,"input_bits":input.raw_current(),"party_count":g.save_data.party.count(),
                    "screen":format!("{:?}",g.state.screen),"item":format!("{:?}",g.start_menu.current_item()),
                    "items":g.start_menu.items().iter().map(|i|format!("{:?}",i)).collect::<Vec<_>>(),
                    "sfx_id":g.audio.as_ref().map(|a|a.manager.lock().unwrap().sequencer.current_sfx_id)}));
            }
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&rows).unwrap()).unwrap();
        });
    }


    #[test]
    #[ignore = "START direction plus B comparison capture"]
    fn capture_combined_menu_input_107() {
        run_link_save_fixture(|| {
            let dir = std::path::PathBuf::from(std::env::var("FIDELITY_COMBINED_MENU_CAPTURE").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let mut g = fixture(Species::Bulbasaur, 3, pokered_core::overworld::Direction::Down);
            let idle = InputState::new();
            g.handle_transition(GameScreen::StartMenu);
            for _ in 0..23 { g.update(&idle); }
            g.update(&button(GbButton::Down));
            for _ in 0..3 { g.update(&idle); }
            let mut input = InputState::new(); let mut rows = Vec::new();
            for t in -1i32..8 {
                if t >= 0 {
                    input.begin_frame();
                    if t == 0 { input.press(GbButton::Down); input.press(GbButton::B); }
                    if t == 2 { input.release(GbButton::Down); input.release(GbButton::B); }
                    g.update(&input);
                }
                let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                g.draw(&mut fb); fb.save_png(&dir.join(format!("frame-{:04}.png",t+1))).unwrap();
                rows.push(serde_json::json!({"t":t,"input_bits":input.raw_current(),
                    "screen":format!("{:?}",g.state.screen),"item":format!("{:?}",g.start_menu.current_item()),
                    "sfx_id":g.audio.as_ref().map(|a|a.manager.lock().unwrap().sequencer.current_sfx_id)}));
            }
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&rows).unwrap()).unwrap();
        });
    }

    #[test]
    fn actual_save_menu_writes_original_direction_masks_89() {
        run_link_save_fixture(|| {
            use pokered_core::overworld::Direction;
            use pokered_core::start_menu::StartMenuItem;
            for (facing,key,mask) in [(Direction::Down,GbButton::Down,4u8),
                (Direction::Up,GbButton::Up,8),(Direction::Left,GbButton::Left,2),
                (Direction::Right,GbButton::Right,1)] {
                let dir=std::env::temp_dir().join(format!("pokered-direction-89-{}-{mask}",std::process::id()));
                std::fs::create_dir_all(&dir).unwrap();
                let path=dir.join("fixture.sav");
                let mut g=fixture(Species::Pikachu,10,Direction::Down);
                g.audio=None; g.external_saves=false; g.save_path=Some(path.clone());
                g.overworld=OverworldScreen::new(MapId::PalletTown,None,PokemonRedData);
                g.overworld.run_on_load(); g.overworld.state.player.x=10;g.overworld.state.player.y=10;
                let idle=InputState::new();
                for _ in 0..20 {g.update(&button(key));}
                for _ in 0..64 {g.update(&idle);}
                assert_eq!(g.overworld.state.player.facing,facing);
                assert_eq!(g.overworld.state.player.movement_state,pokered_core::overworld::MovementState::Idle);
                let position=(g.overworld.state.player.x,g.overworld.state.player.y);
                g.update(&button(GbButton::Start));g.update(&idle);
                assert_eq!(g.state.screen,GameScreen::StartMenu);
                for _ in 0..23 {g.update(&idle);}
                for _ in 0..7 {
                    if g.start_menu.current_item()==StartMenuItem::Save {break;}
                    g.update(&button(GbButton::Down));
                    for _ in 0..3 { g.update(&idle); }
                }
                assert_eq!(g.start_menu.current_item(),StartMenuItem::Save);
                g.update(&button(GbButton::A));g.update(&idle);
                assert_eq!(g.state.screen,GameScreen::SaveMenu);
                for t in 0..1000 {
                    if g.state.screen!=GameScreen::SaveMenu {break;}
                    let advance=button(GbButton::A); g.update(if t%20==0 {&advance} else {&idle});
                }
                assert_eq!(g.state.screen,GameScreen::StartMenu);
                let bytes=std::fs::read(&path).unwrap();
                // Independent original symbols: bank1 sMainData=$a5a3,
                // wMainDataStart=$d2f7; moving/last-stop/current=$d528/29/2a.
                assert_eq!(bytes[0x27d6],mask,"{facing:?} saved wPlayerDirection");
                assert_eq!(bytes[0x27d5],mask,"{facing:?} saved wPlayerLastStopDirection");
                assert_eq!(bytes[0x27d4],0,"idle must save no moving direction");
                let saved=import_sram(&bytes).unwrap();
                assert_eq!((u16::from(saved.game_data.position.x),u16::from(saved.game_data.position.y)),position);
                assert_eq!(saved.party,g.save_data.party);
                std::fs::remove_dir_all(dir).unwrap();
            }
        });
    }

    #[test]
    fn actual_trade_post_saves_party_and_dex_without_room_position() {
        run_link_save_fixture(|| {
            let (host, _peer, before) = completed_actual_trade();
            let persisted: MobileSave = serde_json::from_str(&host.export_mobile_save().unwrap()).unwrap();
            let prior: MobileSave = serde_json::from_str(&before).unwrap();
            assert_eq!(persisted.data.party.get(0).unwrap().species, Species::Pikachu,
                "SavePartyAndDexData persists received Pokemon");
            assert!(persisted.data.game_data.pokedex.is_owned(Species::Pikachu));
            assert_eq!(persisted.data.game_data.position, prior.data.game_data.position,
                "reset must load the Pokemon Center save, not the live trade room");
            let mut expected = serde_json::to_value(&prior).unwrap();
            expected["data"]["party"] = serde_json::to_value(&host.save_data.party).unwrap();
            expected["data"]["game_data"]["pokedex"] = serde_json::to_value(&host.save_data.game_data.pokedex).unwrap();
            assert_eq!(serde_json::to_value(&persisted).unwrap(), expected,
                "party and dex only: preserve other saved data and script flags");
            });
    }

    #[test]
    fn actual_trade_post_desktop_sram_preserves_every_unrelated_byte() {
        run_link_save_fixture(|| {
            use pokered_core::save::sram_layout::*;
            let (mut host, mut peer) = both_selected_pair();
            let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let dir = std::env::temp_dir().join(format!("fidelity-link-save-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("before-room.sav");
            let mut baseline = host.save_data.clone();
            baseline.game_data.position.map_id = MapId::ViridianPokecenter as u8;
            baseline.game_data.position.x = 11; baseline.game_data.position.y = 3;
            baseline.game_data.player_money = 1234;
            let mut original = export_sram(&baseline);
            // Sentinel bytes in original sprite SRAM and bank-1 padding are
            // unmodelled data that a full export would erase.
            original[37] = 0xab; original[SRAM_BANK_SIZE_LAYOUT + 17] = 0xcd;
            std::fs::write(&path, &original).unwrap();
            host.external_saves = false; host.save_path = Some(path.clone());
            host.update(&button(GbButton::A)); peer.update(&button(GbButton::A));
            let idle = InputState::new();
            for _ in 0..12000 {
                host.update(&idle); peer.update(&idle);
                if host.link_cable.phase() == &CableClubPhase::TradeCompleted { break; }
            }
            assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeCompleted);
            let after = std::fs::read(&path).unwrap();
            let loaded = import_sram(&after).unwrap();
            assert_eq!(loaded.party.get(0).unwrap().species, Species::Pikachu);
            assert!(loaded.game_data.pokedex.is_owned(Species::Pikachu));
            assert_eq!(loaded.game_data.position, baseline.game_data.position);
            assert_eq!(loaded.game_data.player_money, 1234);
            let mut main_data = Vec::new(); baseline.game_data.serialize_into(&mut main_data);
            let dex = SRAM_BANK_SIZE_LAYOUT + MAIN_DATA_OFFSET;
            let dex_end = dex + baseline.game_data.pokedex.owned_flags().len() + baseline.game_data.pokedex.seen_flags().len();
            let party = dex + main_data.len() + SPRITE_DATA_REGION_SIZE;
            let checksum = SRAM_BANK_SIZE_LAYOUT + GAME_DATA_OFFSET + baseline.serialize_checksummed_region().len();
            assert_eq!(after.len(), original.len());
            for (i, (&a, &b)) in original.iter().zip(&after).enumerate() {
                if !(dex..dex_end).contains(&i) && !(party..party + PARTY_DATA_SIZE).contains(&i) && i != checksum {
                    assert_eq!(a, b, "partial trade save changed unrelated SRAM byte {i:x}");
                }
            }
            assert!(dir.read_dir().unwrap().all(|e| e.unwrap().path() == path), "no rewritten companion flags");
            let mut resumed = PokemonGame::new_with_options(
                GameVersion::Red, Some(path.clone()), None, None, true, None, false, true,
                #[cfg(feature = "debug-server")] None,
            );
            for _ in 0..2000 {
                if resumed.state.screen == GameScreen::Overworld { break; }
                resumed.update(&button(GbButton::A));
            }
            assert_eq!(resumed.state.screen, GameScreen::Overworld, "actual boot and Continue must finish");
            assert_eq!(resumed.overworld.state.current_map, MapId::ViridianPokecenter);
            assert_eq!((resumed.overworld.state.player.x, resumed.overworld.state.player.y), (11, 3));
            assert_eq!(resumed.save_data.party.get(0).unwrap().species, Species::Pikachu);
            std::fs::remove_dir_all(dir).unwrap();
            });
    }

    #[test]
    fn actual_link_simultaneous_menu_keys_follow_original_branch_priority() {
        run_link_save_fixture(|| {
            let (mut host, _peer) = paired_trade_room();
            let mut right_a = button(GbButton::Right); right_a.press(GbButton::A);
            let mut left_a = button(GbButton::Left); left_a.press(GbButton::A);
            let mut right_b = button(GbButton::Right); right_b.press(GbButton::B);
            let mut left_b = button(GbButton::Left); left_b.press(GbButton::B);
            host.update(&right_a);
            assert_eq!(host.link_cable.local_action(), Some((0, false)),
                "original party list handles A before RIGHT");
            assert_eq!(host.link_cable.peer_cursor(), None);
            host.update(&button(GbButton::B)); host.update(&button(GbButton::Right));
            host.update(&left_a);
            assert_eq!(host.link_cable.stats().unwrap().pokemon().species, Species::Pikachu,
                "original peer list handles A before LEFT");
            assert_eq!(host.link_cable.peer_cursor(), Some(0));
            wait_stats_cry(&mut host);
            host.update(&button(GbButton::B)); host.update(&button(GbButton::A));
            host.update(&button(GbButton::Left)); host.update(&button(GbButton::A));
            host.update(&right_a);
            assert_eq!(host.link_cable.local_action(), Some((0, true)),
                "STATS watched RIGHT precedes A; do not send a trade");
            assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeSelect);
            host.update(&left_a);
            assert_eq!(host.link_cable.local_action(), Some((0, false)),
                "TRADE watched LEFT precedes A; do not open stats");
            assert!(host.link_cable.stats().is_none());
            host.update(&right_b);
            assert_eq!(host.link_cable.local_action(), Some((0, true)), "RIGHT precedes B");
            host.update(&left_b);
            assert_eq!(host.link_cable.local_action(), Some((0, false)), "LEFT precedes B");
            let mut a_b = button(GbButton::A); a_b.press(GbButton::B);
            host.update(&a_b);
            assert_eq!(host.link_cable.local_action(), None, "action B precedes A");
            });
    }

    #[test]
    fn actual_link_vertical_and_horizontal_keys_copy_updated_party_cursor() {
        run_link_save_fixture(|| {
            let (mut host, _peer) = unequal_party_pair();
            let mut down_right = button(GbButton::Down); down_right.press(GbButton::Right);
            host.update(&down_right);
            assert_eq!(host.link_cable.peer_cursor(), Some(1),
                "HandleMenuInput moves DOWN before returning watched RIGHT");
            let mut up_left = button(GbButton::Up); up_left.press(GbButton::Left);
            host.update(&up_left);
            assert_eq!(host.link_cable.peer_cursor(), None);
            assert_eq!(host.link_cable.party_select().unwrap().cursor(), 0,
                "HandleMenuInput moves UP before returning watched LEFT");
            });
    }

    #[test]
    fn actual_link_local_menu_a_and_b_emit_press_ab_pcm() {
        run_link_save_fixture(|| {
            let (mut host, _peer) = paired_trade_room();
            let idle = InputState::new();
            let audio = host.audio.as_ref().unwrap();
            audio.stop_all();
            let mut silent = vec![0.0f32; 2048]; audio.render_pcm(&mut silent);
            assert!(silent.iter().all(|s| s.abs() < 0.00001));
            host.update(&button(GbButton::A));
            assert_eq!(host.link_cable.local_action(), Some((0, false)));
            host.update(&idle);
            assert_eq!(host.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id,
                SfxId::PressAB as u8, "HandleMenuInput plays PRESS_AB on party A");
            let mut pcm = vec![0.0f32; 2048]; host.audio.as_ref().unwrap().render_pcm(&mut pcm);
            assert!(pcm.iter().any(|s| s.abs() > 0.00001), "real A button emits audible PCM");
            for _ in 0..120 { host.update(&idle); }
            host.audio.as_ref().unwrap().stop_all();
            host.update(&button(GbButton::B));
            assert_eq!(host.link_cable.local_action(), None);
            host.update(&idle);
            assert_eq!(host.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id,
                SfxId::PressAB as u8, "HandleMenuInput plays PRESS_AB on action B");
            let mut pcm = vec![0.0f32; 2048]; host.audio.as_ref().unwrap().render_pcm(&mut pcm);
            assert!(pcm.iter().any(|s| s.abs() > 0.00001), "real B button emits audible PCM");
            for _ in 0..120 { host.update(&idle); }
            host.audio.as_ref().unwrap().stop_all();
            host.update(&button(GbButton::B)); host.update(&idle);
            assert_eq!(channels(&host), vec![false; 4], "ignored party B stays silent");
            host.update(&button(GbButton::Down));
            assert!(host.link_cable.cancel_selected());
            host.update(&button(GbButton::A)); host.update(&idle);
            assert_eq!(channels(&host), vec![false; 4], "CANCEL uses raw Joypad, not HandleMenuInput sound");
            });
    }

    #[test]
    fn actual_link_stats_waits_for_cry_before_accepting_b() {
        run_link_save_fixture(|| {
            let (mut host, mut peer) = paired_trade_room();
            let idle = InputState::new();
            host.update(&button(GbButton::Right)); host.update(&button(GbButton::A));
            assert_eq!(host.link_cable.stats().unwrap().page(), pokered_core::stats_screen::StatsPage::Stats);
            assert!(host.audio.as_ref().unwrap().is_sfx_playing());
            host.update(&button(GbButton::B));
            assert_eq!(host.link_cable.stats().unwrap().page(), pokered_core::stats_screen::StatsPage::Stats,
                "StatusScreen PlayCry blocks before WaitForTextScrollButtonPress");
            for _ in 0..120 { host.update(&idle); peer.update(&idle); }
            assert!(!host.audio.as_ref().unwrap().is_sfx_playing());
            host.update(&button(GbButton::B));
            assert_eq!(host.link_cable.stats().unwrap().page(), pokered_core::stats_screen::StatsPage::Moves);
            host.update(&button(GbButton::A));
            assert!(host.link_cable.stats().is_none());
        });
    }

    #[test]
    #[ignore]
    fn capture_stats_entry_raw_59() {
        run_link_save_fixture(|| {
            use pokered_core::overworld::Direction;
            use pokered_core::save::sram_import::import_sram;
            let dir = std::path::PathBuf::from(std::env::var("FIDELITY_STATS_CAPTURE").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let mut g = fixture(Species::Exeggutor, 7, Direction::Down);
            g.save_data = import_sram(&std::fs::read(std::env::var("FIDELITY_STATS_SRAM").unwrap()).unwrap()).unwrap();
            g.state.screen = GameScreen::Overworld;
            g.state.config.language = pokered_core::game_state::Lang::En;
            g.overworld = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
            for flag in [pokered_data::event_flags::EventFlag::EVENT_GOT_POKEDEX,
                         pokered_data::event_flags::EventFlag::EVENT_GOT_STARTER] { g.overworld.set_event_flag_live(flag); }
            g.overworld.state.player.x = 5; g.overworld.state.player.y = 6;
            g.overworld.run_on_load();
            let idle = InputState::new();
            for _ in 0..120 { g.update(&idle); }
            g.update(&button(GbButton::Start)); for _ in 0..120 { g.update(&idle); }
            g.update(&button(GbButton::Down)); for _ in 0..120 { g.update(&idle); }
            g.update(&button(GbButton::A)); for _ in 0..120 { g.update(&idle); }
            assert_eq!(g.state.screen, GameScreen::PartyScreen);
            g.update(&button(GbButton::A)); for _ in 0..120 { g.update(&idle); }
            for _ in 0..4 { g.update(&button(GbButton::Down)); for _ in 0..60 {g.update(&idle);} }
            let mut records = Vec::new();
            let start_frame = g.frame_count;
            let mut trigger = InputState::new();
            for t in -1i32..240 {
                if t >= 0 {
                    trigger.begin_frame();
                    if t == 0 { trigger.press(GbButton::A); }
                    if t == 2 { trigger.release(GbButton::A); }
                    g.update(&trigger);
                }
                let mut fb = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
                g.draw(&mut fb); fb.save_png(&dir.join(format!("frame-{:04}.png", t+1))).unwrap();
                let audio = g.audio.as_ref().unwrap();
                let regs = {let m=audio.manager.lock().unwrap(); (0xff10..=0xff26).map(|r|m.apu.read_register(r)).collect::<Vec<_>>()};
                let mut pcm=vec![0f32;1470]; audio.render_pcm(&mut pcm);
                std::fs::write(dir.join(format!("pcm-{:04}.f32",t+1)),pcm.iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>()).unwrap();
                records.push(serde_json::json!({"t":t,"frame":g.frame_count,"trigger_frame":start_frame+1,"input_bits":trigger.raw_current(),
                    "screen":format!("{:?}",g.state.screen),"page":g.stats_screen.as_ref().map(|s|format!("{:?}",s.page())),
                    "sfx_active":audio.is_sfx_playing(),"apu_registers":regs,"party":g.save_data.party}));
            }
            assert!(matches!(g.state.screen,GameScreen::PokemonStatsScreen(0)));
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&records).unwrap()).unwrap();
            trigger.begin_frame(); trigger.press(GbButton::A);
            for t in 0..20 {
                if t > 0 { trigger.begin_frame(); }
                if t == 2 { trigger.release(GbButton::A); }
                g.update(&trigger);
            }
            assert_eq!(g.stats_screen.as_ref().unwrap().page(),pokered_core::stats_screen::StatsPage::Moves);
            let mut fb=FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);
            g.draw(&mut fb);fb.save_png(&dir.join("moves-page.png")).unwrap();
            std::fs::write(dir.join("moves-page.json"),serde_json::to_string_pretty(&serde_json::json!({
                "frame":g.frame_count,"page":"Moves","party":g.save_data.party,
                "input":"after raw t239, A held two frames, release at third, capture after20updates"})).unwrap()).unwrap();
        });
    }

    fn actual_pc_stats_59(from_box: bool) -> PokemonGame {
        use pokered_core::overworld::Direction;
        use pokered_core::pc_screen::PcPhase;
        let mut g = fixture(Species::Exeggutor, 13, Direction::Up);
        let second = create_pokemon_with_moves(Species::Bulbasaur, 25, [0x99,0x88],
            [MoveId::Tackle,MoveId::None,MoveId::None,MoveId::None]).unwrap();
        g.save_data.party.add(second).unwrap();
        let mon = g.save_data.party.get(0).unwrap().clone();
        g.save_data.pc_storage.current_box_mut().deposit(mon).unwrap();
        g.overworld = OverworldScreen::new(MapId::ViridianPokecenter, None, PokemonRedData);
        g.overworld.run_on_load();
        g.overworld.state.player.x=13;g.overworld.state.player.y=4;g.overworld.state.player.facing=Direction::Up;
        let idle=InputState::new();
        for _ in 0..120 {g.update(&idle);}
        g.update(&button(GbButton::A));
        for frame in 0..2000 {
            if g.pc_screen.as_ref().is_some_and(|p| p.phase()==PcPhase::MainMenu) {break;}
            let advance=button(GbButton::A); g.update(if frame%20==19 {&advance} else {&idle});
        }
        assert_eq!(g.state.screen,GameScreen::PC,"actual hidden-event PC activation");
        assert_eq!(g.pc_screen.as_ref().unwrap().phase(),PcPhase::MainMenu);
        g.update(&button(GbButton::A));
        for frame in 0..1000 {
            if g.pc_screen.as_ref().unwrap().phase()==PcPhase::BillsMenu {break;}
            let advance=button(GbButton::A); g.update(if frame%20==19 {&advance} else {&idle});
        }
        assert_eq!(g.pc_screen.as_ref().unwrap().phase(),PcPhase::BillsMenu);
        if !from_box {g.update(&button(GbButton::Down));g.update(&idle);}
        g.update(&button(GbButton::A));for _ in 0..120 {g.update(&idle);}
        assert_eq!(g.pc_screen.as_ref().unwrap().phase(),PcPhase::MonList);
        g.update(&button(GbButton::A));for _ in 0..120 {g.update(&idle);}
        assert_eq!(g.pc_screen.as_ref().unwrap().phase(),PcPhase::MonAction);
        g.update(&button(GbButton::Down));g.update(&idle);g.update(&button(GbButton::A));
        assert_eq!(g.state.screen,GameScreen::PokemonStatsScreen(0));
        g
    }

    #[test]
    fn actual_pc_stats_59_party_and_box_keep_loading_and_cry_input_locked() {
        run_link_save_fixture(|| {
            for from_box in [false,true] {
                let mut g=actual_pc_stats_59(from_box);
                let idle=InputState::new();
                assert_eq!(g.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id,SfxId::PressAB as u8);
                for t in 1..162 {
                    let early=button(GbButton::B); g.update(if [1,10,20,60,71,100,150,161].contains(&t) {&early} else {&idle});
                    assert_eq!(g.stats_screen.as_ref().unwrap().page(),pokered_core::stats_screen::StatsPage::Stats,"box={from_box} t={t}");
                    assert_eq!(g.audio.as_ref().unwrap().manager.lock().unwrap().apu.read_register(0xff24),
                        if t < 71 { 0x33 } else { 0x77 }, "original StatusScreen/Cry volume t={t}");
                    if t==71 {assert_eq!(g.audio.as_ref().unwrap().manager.lock().unwrap().sequencer.current_sfx_id,pokered_data::cries::cry_data(Species::Exeggutor).sfx);}
                }
                g.update(&button(GbButton::A));
                assert_eq!(g.stats_screen.as_ref().unwrap().page(),pokered_core::stats_screen::StatsPage::Moves);
                g.update(&idle);g.update(&button(GbButton::B));
                assert_eq!(g.state.screen,GameScreen::PC);
                assert_eq!(g.audio.as_ref().unwrap().manager.lock().unwrap().apu.read_register(0xff24),0x77);
                assert_eq!(g.pc_screen.as_ref().unwrap().phase(),pokered_core::pc_screen::PcPhase::MonAction);
                assert_eq!(g.save_data.party.count(),2);assert_eq!(g.save_data.pc_storage.current_box().count(),1);
            }
        });
    }

    #[test]
    fn actual_title_continue_faces_down_for_original_and_legacy_direction_bytes() {
        run_link_save_fixture(|| {
            use pokered_core::overworld::Direction;
            use pokered_core::save::sram_export::export_sram;
            let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let dir = std::env::temp_dir().join(format!("fidelity-continue-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let baseline = fixture(Species::Bulbasaur, 13, Direction::Up).save_data;
            for direction in [1, 2, 4, 8, 0, 12] {
                let mut save = baseline.clone();
                save.game_data.position.map_id = MapId::ViridianPokecenter as u8;
                save.game_data.position.x = 13; save.game_data.position.y = 4;
                save.game_data.player_direction = direction;
                save.game_data.player_last_stop_direction = 2;
                save.game_data.player_moving_direction = 0;
                let path = dir.join(format!("direction-{direction}.sav"));
                std::fs::write(&path, export_sram(&save)).unwrap();
                let mut g = PokemonGame::new_with_options(
                    GameVersion::Red, Some(path), None, None, false, None, false, true,
                    #[cfg(feature="debug-server")] None,
                );
                let idle = InputState::new();
                let mut saw_main_menu = false;
                for frame in 0..2000 {
                    saw_main_menu |= g.state.screen == GameScreen::MainMenu;
                    if g.state.screen == GameScreen::Overworld { break; }
                    let advance = button(GbButton::A);
                    g.update(if frame % 20 == 19 { &advance } else { &idle });
                }
                assert!(saw_main_menu, "must consume the real Continue menu");
                assert_eq!(g.state.screen, GameScreen::Overworld);
                assert_eq!(g.overworld.state.current_map, MapId::ViridianPokecenter);
                assert_eq!((g.overworld.state.player.x, g.overworld.state.player.y), (13,4));
                assert_eq!(g.overworld.state.player.facing, Direction::Down, "saved byte {direction}");
                assert_eq!(g.save_data.game_data.player_direction, direction, "Continue must not rewrite SRAM data");
                for _ in 0..120 {g.update(&idle);}
                assert_eq!(g.overworld.player_last_stop_direction,2,"idle Continue preserves previous Left stop despite facing Down");
                assert_eq!(g.overworld.player_moving_direction,0);
                let exported=g.build_save_data();
                assert_eq!(exported.game_data.player_direction,4);
                assert_eq!(exported.game_data.player_last_stop_direction,2);
                assert_eq!(exported.game_data.player_moving_direction,0);
                assert_eq!(g.main_menu.last_choice, Some(pokered_core::game_state::MainMenuChoice::Continue));
            }
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn actual_pc_stats_return_waits_for_tiles_and_restores_retained_frames() {
        run_link_save_fixture(|| {
            use pokered_core::pc_screen::PcPhase;
            for from_box in [false, true] {
                let mut g = actual_pc_stats_59(from_box);
                let idle = InputState::new();
                for _ in 0..200 { g.update(&idle); }
                let party = serde_json::to_string(&g.save_data.party).unwrap();
                let storage = serde_json::to_string(&g.save_data.pc_storage).unwrap();
                g.update(&button(GbButton::A));
                for _ in 0..20 { g.update(&idle); }
                g.update(&button(GbButton::B));
                assert_eq!(g.state.screen, GameScreen::PC);
                let mut retained = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
                let mut session = crate::render::session::RenderSession::new();
                let mut scroll = |_: &mut [u8], _: usize, _: usize, _: i32, _: i32, _: u8| panic!("PC must not scroll");
                for t in 0..=10 {
                    let early = button(GbButton::A);
                    if t > 0 { g.update(if t < 6 && t % 2 == 1 { &early } else { &idle }); }
                    assert_eq!(g.pc_screen.as_ref().unwrap().phase(), PcPhase::MonAction,
                        "early A must not reopen STATS: box={from_box} t={t}");
                    assert_eq!(g.state.screen, GameScreen::PC);
                    session.render(&mut g, &mut retained, &mut scroll);
                    let mut full = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
                    g.draw(&mut full);
                    for y in 0..144 { for x in 0..160 {
                        assert_eq!(retained.get_pixel(x,y), full.get_pixel(x,y), "box={from_box} t={t} at {x},{y}");
                        if (1..=6).contains(&t) {
                            assert_eq!(full.get_pixel(x,y), Some(Rgba::WHITE), "original PC reload frame {t}");
                        }
                    } }
                    assert_eq!(g.audio.as_ref().unwrap().manager.lock().unwrap().apu.read_register(0xff24), 0x77);
                }
                assert_eq!(serde_json::to_string(&g.save_data.party).unwrap(), party);
                assert_eq!(serde_json::to_string(&g.save_data.pc_storage).unwrap(), storage);
                g.update(&button(GbButton::A));
                assert_eq!(g.state.screen, GameScreen::PokemonStatsScreen(0));
                assert_eq!(g.pc_stats_return_frame, None, "reopening STATS discards the finished return");
            }
        });
    }

    #[test]
    fn actual_pc_stats_menu_resumes_at_original_frame_six() {
        run_link_save_fixture(|| {
            for from_box in [false, true] {
                let mut g = actual_pc_stats_59(from_box);
                let idle = InputState::new();
                for _ in 0..200 { g.update(&idle); }
                g.update(&button(GbButton::A));
                for _ in 0..20 { g.update(&idle); }
                g.update(&button(GbButton::B));
                for t in 1..6 { let early = button(GbButton::B); g.update(if t == 1 || t == 3 { &early } else { &idle }); }
                assert_eq!(g.pc_screen.as_ref().unwrap().phase(), pokered_core::pc_screen::PcPhase::MonAction);
                g.update(&button(GbButton::B));
                assert_eq!(g.pc_screen.as_ref().unwrap().phase(), pokered_core::pc_screen::PcPhase::MonList);
            }
        });
    }

    #[test]
    fn actual_stats_loading_retained_frames_match_full_draw() {
        run_link_save_fixture(|| {
            let mut g = ordinary_muted_party_stats();
            let mut session = crate::render::session::RenderSession::new();
            let mut retained = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
            let mut scroll = |_: &mut [u8], _: usize, _: usize, _: i32, _: i32, _: u8| panic!("stats must not scroll");
            for frame in 0..150 {
                session.render(&mut g, &mut retained, &mut scroll);
                let mut full = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
                g.draw(&mut full);
                for y in 0..144 { for x in 0..160 {
                    assert_eq!(retained.get_pixel(x,y),full.get_pixel(x,y),"stats frame {frame} at {x},{y}");
                }}
                g.update(&InputState::new());
            }
        });
    }

    fn saved_reference_pc_stats_80(from_box: bool, dir: &std::path::Path) -> PokemonGame {
        use pokered_core::pc_screen::PcPhase;
        let path=dir.join("fixture.sav");
        std::fs::write(&path,std::fs::read(std::env::var("FIDELITY_PC_REFERENCE_SRAM").unwrap()).unwrap()).unwrap();
        let mut g=PokemonGame::new_with_options(GameVersion::Red,Some(path),None,None,false,None,false,true,
            #[cfg(feature="debug-server")] None);
        let idle=InputState::new();
        for frame in 0..2000 {
            if g.state.screen==GameScreen::Overworld {break;}
            let advance=button(GbButton::A);g.update(if frame%20==19 {&advance} else {&idle});
        }
        assert_eq!(g.state.screen,GameScreen::Overworld);
        assert_eq!(g.overworld.state.current_map,MapId::ViridianPokecenter);
        assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),(13,4));
        std::fs::write(dir.join("continue.json"),serde_json::to_string_pretty(&serde_json::json!({
            "frame":g.frame_count,"facing":format!("{:?}",g.overworld.state.player.facing),
            "saved_direction":g.save_data.game_data.player_direction,"party":g.save_data.party})).unwrap()).unwrap();
        let mut fb = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
        g.draw(&mut fb); fb.save_png(&dir.join("continue.png")).unwrap();
        g.overworld.set_rng_seed(0);
        // Original Continue also requires facing the actual PC before A.
        g.update(&button(GbButton::Up));for _ in 0..120 {g.update(&idle);}
        g.update(&button(GbButton::A));
        for frame in 0..2000 {
            if g.pc_screen.as_ref().is_some_and(|p| p.phase()==PcPhase::MainMenu) {break;}
            let advance=button(GbButton::A);g.update(if frame%20==19 {&advance} else {&idle});
        }
        assert_eq!(g.pc_screen.as_ref().unwrap().phase(),PcPhase::MainMenu);
        g.update(&button(GbButton::A));
        for frame in 0..1000 {
            if g.pc_screen.as_ref().unwrap().phase()==PcPhase::BillsMenu {break;}
            let advance=button(GbButton::A);g.update(if frame%20==19 {&advance} else {&idle});
        }
        assert_eq!(g.pc_screen.as_ref().unwrap().phase(),PcPhase::BillsMenu);
        if !from_box {g.update(&button(GbButton::Down));g.update(&idle);}
        g.update(&button(GbButton::A));for _ in 0..120 {g.update(&idle);}
        assert_eq!(g.pc_screen.as_ref().unwrap().phase(),PcPhase::MonList);
        g.update(&button(GbButton::A));for _ in 0..120 {g.update(&idle);}
        assert_eq!(g.pc_screen.as_ref().unwrap().phase(),PcPhase::MonAction);
        g.update(&button(GbButton::Down));g.update(&idle);
        g
    }

    #[test]
    #[ignore]
    fn capture_stats_transitions_raw_80_82() {
        run_link_save_fixture(|| {
            let root=std::path::PathBuf::from(std::env::var("FIDELITY_STATS_TRANSITIONS").unwrap());
            for from_box in [false,true] {
                let dir=root.join(if from_box {"box"} else {"party"});std::fs::create_dir_all(&dir).unwrap();
                let mut g=saved_reference_pc_stats_80(from_box,&dir);
                for (window,key,frames) in [("entry",GbButton::A,240),("page2",GbButton::B,80),("exit",GbButton::B,100)] {
                    let folder=dir.join(window);std::fs::create_dir_all(&folder).unwrap();
                    let mut input=InputState::new();let mut records=Vec::new();
                    for t in -1i32..frames {
                        if t>=0 {
                            input.begin_frame();if t==0 {input.press(key);}if t==2 {input.release(key);}g.update(&input);
                        }
                        let mut fb=FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);g.draw(&mut fb);
                        fb.save_png(&folder.join(format!("frame-{:04}.png",t+1))).unwrap();
                        records.push(serde_json::json!({"t":t,"frame":g.frame_count,"input_bits":input.raw_current(),
                            "screen":format!("{:?}",g.state.screen),"page":g.stats_screen.as_ref().map(|s|format!("{:?}",s.page())),
                            "nr50":g.audio.as_ref().unwrap().manager.lock().unwrap().apu.read_register(0xff24),
                            "sfx_active":g.audio.as_ref().unwrap().is_sfx_playing(),"party":g.save_data.party}));
                    }
                    std::fs::write(folder.join("frames.json"),serde_json::to_string_pretty(&records).unwrap()).unwrap();
                }
                assert_eq!(g.state.screen,GameScreen::PC);
            }
        });
    }

    #[test]
    #[ignore]
    fn capture_actual_field_origin_raw_90() {
        run_link_save_fixture(|| {
            let dir = std::path::PathBuf::from(std::env::var("FIDELITY_FIELD_CAPTURE").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("fixture.sav");
            std::fs::write(&path, std::fs::read(std::env::var("FIDELITY_FIELD_SRAM").unwrap()).unwrap()).unwrap();
            let mut g = PokemonGame::new_with_options(
                GameVersion::Red, Some(path), None, None, false, None, false, true,
                #[cfg(feature="debug-server")] None,
            );
            let idle = InputState::new();
            let mut saw_main_menu = false;
            for frame in 0..2000 {
                saw_main_menu |= g.state.screen == GameScreen::MainMenu;
                if g.state.screen == GameScreen::Overworld { break; }
                let advance = button(GbButton::A);
                g.update(if frame % 20 == 19 { &advance } else { &idle });
            }
            assert!(saw_main_menu);
            assert_eq!(g.state.screen, GameScreen::Overworld);
            assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y), (13,4));
            assert_eq!(g.overworld.state.player.facing,pokered_core::overworld::Direction::Down);
            g.overworld.set_rng_seed(0);
            for _ in 0..120 { g.update(&idle); }
            let mut input = InputState::new();
            let mut records = Vec::new();
            for t in -1i32..100 {
                if t >= 0 {
                    input.begin_frame();
                    if t == 0 { input.press(GbButton::Left); }
                    if t == 16 { input.release(GbButton::Left); }
                    g.update(&input);
                }
                let mut fb = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
                g.draw(&mut fb); fb.save_png(&dir.join(format!("frame-{:04}.png", t+1))).unwrap();
                records.push(serde_json::json!({"t":t,"frame":g.frame_count,"input_bits":input.raw_current(),
                    "screen":format!("{:?}",g.state.screen),"map":g.overworld.state.current_map as u8,
                    "x":g.overworld.state.player.x,"y":g.overworld.state.player.y,
                    "facing":format!("{:?}",g.overworld.state.player.facing),
                    "movement":format!("{:?}",g.overworld.state.player.movement_state),
                    "walk_counter":g.overworld.state.walk_counter,"party":g.save_data.party}));
            }
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&records).unwrap()).unwrap();
        });
    }

    #[test]
    #[ignore]
    fn capture_actual_walk_bike_raw_91() {
        run_link_save_fixture(|| {
            let dir = std::path::PathBuf::from(std::env::var("FIDELITY_MOVEMENT_CAPTURE").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("fixture.sav");
            std::fs::write(&path, std::fs::read(std::env::var("FIDELITY_MOVEMENT_SRAM").unwrap()).unwrap()).unwrap();
            let mut g = PokemonGame::new_with_options(
                GameVersion::Red, Some(path), None, None, false, None, false, true,
                #[cfg(feature="debug-server")] None,
            );
            let idle = InputState::new();
            let mut saw_main_menu = false;
            for frame in 0..2000 {
                saw_main_menu |= g.state.screen == GameScreen::MainMenu;
                if g.state.screen == GameScreen::Overworld { break; }
                let advance = button(GbButton::A);
                g.update(if frame % 20 == 19 { &advance } else { &idle });
            }
            assert!(saw_main_menu);
            assert_eq!(g.state.screen, GameScreen::Overworld);
            let start_x=std::env::var("FIDELITY_MOVEMENT_X").unwrap_or_else(|_|"23".into()).parse::<u16>().unwrap();
            let start_y=std::env::var("FIDELITY_MOVEMENT_Y").unwrap_or_else(|_|"29".into()).parse::<u16>().unwrap();
            assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y), (start_x,start_y));
            assert_eq!(g.overworld.state.player.facing,pokered_core::overworld::Direction::Down);
            let pc_case=std::env::var("FIDELITY_INPUT_PC_CASE").ok();
            if pc_case.is_some() {
                g.overworld.warp_to_map(MapId::ViridianPokecenter,13,4);
                for _ in 0..120 {g.update(&idle);}
                g.update(&button(GbButton::Down));g.update(&button(GbButton::Down));
                for _ in 0..120 {g.update(&idle);}
                assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),(13,5));
                // Original preparation turns Up without walking; native turn
                // timing remains a separate open audit, so stage this facing.
                g.overworld.state.player.facing=pokered_core::overworld::Direction::Up;
                let mut controlled=g.save_data.game_data.clone();
                controlled.player_last_stop_direction=8;controlled.player_moving_direction=0;
                g.overworld.restore_system_save_state(&controlled);
            }
            let bike=std::env::var("FIDELITY_MOVEMENT_BIKE").is_ok_and(|s|s=="true");
            if bike {
                let mut held_start=button(GbButton::Start);
                g.update(&held_start);held_start.begin_frame();g.update(&held_start);
                for _ in 0..23 {g.update(&idle);}
                assert_eq!(g.state.screen,GameScreen::StartMenu);
                for _ in 0..7 {
                    if g.start_menu.current_item()==pokered_core::start_menu::StartMenuItem::Item {break;}
                    g.update(&button(GbButton::Down));for _ in 0..3 {g.update(&idle);}
                }
                g.update(&button(GbButton::A));g.update(&idle);
                assert_eq!(g.state.screen,GameScreen::Bag);
                for _ in 0..21 {
                    if g.bag_screen.items().get(g.bag_screen.cursor()).is_some_and(|(id,_)|*id==pokered_data::items::ItemId::Bicycle) {break;}
                    g.update(&button(GbButton::Down));g.update(&idle);
                }
                assert_eq!(g.bag_screen.items()[g.bag_screen.cursor()].0,pokered_data::items::ItemId::Bicycle);
                g.update(&button(GbButton::A));g.update(&idle);
                g.update(&button(GbButton::A));g.update(&idle);
                for t in 0..1000 {
                    if g.state.screen==GameScreen::Overworld && g.overworld.pending_dialogue.is_none() {break;}
                    let advance=button(GbButton::A);g.update(if t%20==19 {&advance} else {&idle});
                }
                assert_eq!(g.overworld.state.player.transport,dotzuki_engine::overworld::types::TransportMode::Biking);
            }
            let trigger=if pc_case.is_some() {GbButton::Up} else {match std::env::var("FIDELITY_MOVEMENT_DIRECTION").unwrap_or_else(|_|"left".into()).as_str() {
                "left"=>GbButton::Left,"right"=>GbButton::Right,
                "up"=>GbButton::Up,"down"=>GbButton::Down,_=>panic!("unsupported direction"),
            }};
            g.overworld.set_rng_seed(0);
            for _ in 0..120 { g.update(&idle); }
            if let Ok(portion) = std::env::var("FIDELITY_MENU_BG_PORTION") {
                g.overworld.bg_transfer_portion = portion.parse().unwrap();
            }
            let npc_ready_walk = std::env::var_os("FIDELITY_NPC_READY_WALK").is_some();
            let npc_already_moving = npc_ready_walk && std::env::var_os("FIDELITY_NPC_ALREADY_MOVING").is_some();
            let npc_font_case = std::env::var_os("FIDELITY_NPC_FONT_CASE").is_some();
            if npc_font_case || npc_ready_walk {
                if npc_ready_walk {
                    let mut controlled = g.save_data.game_data.clone();
                    controlled.player_last_stop_direction = 2;
                    controlled.player_moving_direction = 0;
                    g.overworld.restore_system_save_state(&controlled);
                }
                assert_eq!(g.overworld.state.current_map, MapId::ViridianCity);
                let npc = &mut g.overworld.npc_states[0];
                assert_eq!(npc.sprite_id, 4);
                npc.x = if std::env::var_os("FIDELITY_NPC_OFFSCREEN").is_some() { start_x + 7 } else if std::env::var_os("FIDELITY_NPC_BOXED").is_some() { start_x + 2 } else { start_x - 1 };
                npc.y = start_y - 1;
                npc.facing = pokered_core::overworld::Direction::Down;
                npc.walk_counter = std::env::var("FIDELITY_NPC_INITIAL_REMAINING").ok().map(|value|value.parse().unwrap()).unwrap_or(if npc_ready_walk && !npc_already_moving { 0 } else { 12 });
                npc.delay_counter = std::env::var("FIDELITY_NPC_INITIAL_DELAY").ok().map(|value|value.parse().unwrap()).unwrap_or(if npc_ready_walk && !npc_already_moving {3} else if std::env::var_os("FIDELITY_NPC_BOXED").is_some() { 27 } else { 54 });
                npc.visible = true;
                // Match the original controlled counter12 setup and four
                // real hardware frames that prime its OAM/LCD pipeline.
                for _ in 0..4 { g.update(&idle); }
            }
            let duration=std::env::var("FIDELITY_MOVEMENT_HOLD").unwrap_or_else(|_|"16".into()).parse::<i32>().unwrap();
            let start_hold=std::env::var("FIDELITY_MOVEMENT_START_HOLD").ok().map(|s|s.parse::<i32>().unwrap());
            let mut input = InputState::new();
            let mut retained_session = crate::render::session::RenderSession::new();
            let mut retained_fb = FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);
            let mut records = Vec::new();
            for t in -1i32..100 {
                if t >= 0 {
                    input.begin_frame();
                    if t == 0 && !npc_font_case { input.press(trigger); }
                    if t == duration { input.release(trigger); }
                    if let Some(hold)=start_hold {
                        let opening = if npc_font_case { 0 } else { 5 };
                        if t==opening {input.press(GbButton::Start);}
                        if t==opening+hold {input.release(GbButton::Start);}
                    }
                    if npc_font_case && std::env::var("FIDELITY_NPC_FONT_CASE").is_ok_and(|s|s=="close") {
                        if t==50 {input.press(GbButton::B);}
                        if t==52 {input.release(GbButton::B);}
                    }
                    if let Some(case)=pc_case.as_deref() {
                        let hold=if case=="a-short" {1} else {40};
                        if t==5 {input.press(GbButton::A);if case=="a-start-held" {input.press(GbButton::Start);}}
                        if t==5+hold {input.release(GbButton::A);if case=="a-start-held" {input.release(GbButton::Start);}}
                    }
                    g.update(&input);
                }
                let saved=g.build_save_data();
                let mut fb = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
                if std::env::var_os("FIDELITY_RETAINED_CAPTURE").is_some() {
                    retained_session.render(&mut g, &mut retained_fb,
                        &mut |_,_,_,_,_,_| {});
                    retained_fb.save_png(&dir.join(format!("frame-{:04}.png",t+1))).unwrap();
                } else {
                    g.draw(&mut fb); fb.save_png(&dir.join(format!("frame-{:04}.png", t+1))).unwrap();
                }
                records.push(serde_json::json!({"t":t,"frame":g.frame_count,"input_bits":input.raw_current(),
                    "screen":format!("{:?}",g.state.screen),"map":g.overworld.state.current_map as u8,
                    "npc_states":if npc_font_case || npc_ready_walk {Some(&g.overworld.npc_states)} else {None},
                    "npc_sprite_states":if npc_font_case || npc_ready_walk {Some(pokered_core::snapshot::OverworldSnapshot::capture(&g.overworld).npc_sprite_states)} else {None},
                    "field_text_restore":g.overworld.field_text_restore,
                    "x":g.overworld.state.player.x,"y":g.overworld.state.player.y,
                    "facing":format!("{:?}",g.overworld.state.player.facing),
                    "movement":format!("{:?}",g.overworld.state.player.movement_state),
                    "last_stop":saved.game_data.player_last_stop_direction,"moving_direction":saved.game_data.player_moving_direction,
                    "pc_phase":g.pc_screen.as_ref().map(|p|format!("{:?}",p.phase())),
                    "pending_pc":g.overworld.pending_pc.is_some(),
                    "walk_counter":g.overworld.state.walk_counter,"transport":format!("{:?}",g.overworld.state.player.transport),"party":g.save_data.party}));
            }
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&records).unwrap()).unwrap();
        });
    }

    fn npc_grass_fixture_137(walking: bool, future_delay: u16) -> (PokemonGame, usize) {
        use pokered_core::overworld::Direction;
        use pokered_core::snapshot::OverworldSnapshot;
        use dotzuki_engine::overworld::collision::CollisionProvider as _;
            let mut g = fixture(Species::Bulbasaur, 3, Direction::Down);
            g.overworld.warp_to_map(MapId::Route1, 12, 22);
            let idle = InputState::new();
            g.overworld.state.player.facing = Direction::Left;
            for _ in 0..120 { g.update(&idle); }
            let map = g.overworld.map_data.as_ref().unwrap();
            let provider = pokered_core::overworld::collision::PokemonCollisionProvider::new(MapId::Route1, map.tileset);
            let tile = provider.get_tile_at_position(map.tileset, &map.blocks, map.width, 14, 22);
            assert_eq!(Some(tile), pokered_data::tileset_data::get_grass_tile(map.tileset));
            let mut saved = OverworldSnapshot::capture(&g.overworld);
            saved.field_loop_wait = 1;
            saved.player_last_stop_direction = 2;
            saved.player_moving_direction = 0;
            saved.check_player_turn = true;
            let slot = if walking {1} else {0};
            saved.npc_states[slot].x = if walking {15} else {14};
            saved.npc_states[slot].y = 22;
            saved.npc_states[slot].facing = if walking {Direction::Right} else {Direction::Down};
            saved.npc_states[slot].walk_counter = if walking {12} else {0};
            saved.npc_states[slot].delay_counter = future_delay;
            saved.npc_states[slot].visible = true;
            saved.npc_sprite_states.clear();
            saved.restore_into(&mut g.overworld);
        g.overworld.state.encounter_cooldown = 255;
        (g, slot)
    }

    #[test]
    fn npc_grass_priority_matches_original_pixels_and_half_tile_boundaries() {
        use pokered_core::snapshot::OverworldSnapshot;
        run_link_save_fixture(|| {
            let reference: serde_json::Value = serde_json::from_str(include_str!("../tests/fixtures/npc-grass-137.json")).unwrap();
            for case in ["stand", "walk", "player"] {
                let oracle = &reference["cases"][case];
                let (mut g, slot) = npc_grass_fixture_137(case != "stand", oracle["future_delay"].as_u64().unwrap() as u16);
                let idle = InputState::new();
                let mut cached = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
                let mut session = crate::render::session::RenderSession::new();
                session.render(&mut g, &mut cached, &mut |_,_,_,_,_,_| {});
                // Prime while rendering, so priority changes without actor
                // movement must invalidate a previously composited frame.
                for _ in 0..4 {
                    g.update(&idle);
                    session.render(&mut g, &mut cached, &mut |_,_,_,_,_,_| {});
                    let mut full = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
                    g.draw(&mut full);
                    for y in 0..144 { for x in 0..160 { assert_eq!(cached.get_pixel(x,y),full.get_pixel(x,y),"prime {case} {x},{y}"); } }
                }
                let mut input = InputState::new();
                for row in oracle["frames"].as_array().unwrap() {
                    let t = row["t"].as_i64().unwrap();
                    if t >= 0 {
                        input.begin_frame();
                        if case == "player" && t == 0 { input.press(GbButton::Down); }
                        if case == "player" && t == 16 { input.release(GbButton::Down); }
                        g.update(&input);
                    }
                    let snap = OverworldSnapshot::capture(&g.overworld);
                    let npc = &snap.npc_states[slot];
                    let sprite = &snap.npc_sprite_states[slot];
                    assert_eq!(u64::from(g.overworld.state.walk_counter),row["player_counter"].as_u64().unwrap(),"player {case} t={t}");
                    assert_eq!(u64::from(npc.walk_counter),row["remaining"].as_u64().unwrap(),"NPC {case} t={t}");
                    assert_eq!(u64::from(sprite.phase),row["phase"].as_u64().unwrap(),"phase {case} t={t}");
                    assert_eq!(u64::from(sprite.intra_frame),row["intra"].as_u64().unwrap(),"intra {case} t={t}");
                    assert_eq!(u64::from(sprite.pending[1].image),row["image"].as_u64().unwrap(),"image {case} t={t}");
                    assert_eq!(sprite.pending[1].grass_priority,row["priority"].as_bool().unwrap(),"priority {case} t={t}");
                    if npc.walk_counter == 0 { assert_eq!(u64::from(npc.delay_counter),row["delay"].as_u64().unwrap(),"delay {case} t={t}"); }
                    let mut full = FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);
                    g.draw(&mut full);
                    session.render(&mut g, &mut cached, &mut |_,_,_,_,_,_| {});
                    for y in 0..144 { for x in 0..160 { assert_eq!(cached.get_pixel(x,y),full.get_pixel(x,y),"cached {case} t={t} {x},{y}"); } }
                    let region = oracle["region"].as_array().unwrap();
                    let x = region[0].as_u64().unwrap() as u32;
                    let y = region[1].as_u64().unwrap() as u32;
                    let width = region[2].as_u64().unwrap() as u32;
                    let height = region[3].as_u64().unwrap() as u32;
                    let packed: Vec<u8> = row["pixels"].as_str().unwrap().as_bytes().chunks_exact(2)
                        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(),16).unwrap()).collect();
                    let colors = [Rgba::WHITE,Rgba::rgb(170,170,170),Rgba::rgb(85,85,85),Rgba::BLACK];
                    for py in 0..height { for px in 0..width {
                        let i = (py*width+px) as usize;
                        let index = (packed[i/4] >> (6-2*(i%4))) & 3;
                        assert_eq!(full.get_pixel(x+px,y+py),Some(colors[index as usize]),"source {case} t={t} {px},{py}");
                    } }
                    if t == 10 {
                        let decoded: OverworldSnapshot = serde_json::from_str(&serde_json::to_string(&snap).unwrap()).unwrap();
                        let mut live = OverworldScreen::new(MapId::Route1,None,PokemonRedData);
                        let mut restored = OverworldScreen::new(MapId::Route1,None,PokemonRedData);
                        snap.restore_into(&mut live); decoded.restore_into(&mut restored);
                        for frame in 0..30 {
                            for screen in [&mut live,&mut restored] { screen.update_frame(dotzuki_engine::overworld::OverworldInput::new(false,false,false,false,false,false,false,false)); }
                            assert_eq!(serde_json::to_value(OverworldSnapshot::capture(&live).npc_sprite_states).unwrap(),serde_json::to_value(OverworldSnapshot::capture(&restored).npc_sprite_states).unwrap(),"snapshot {case} frame{frame}");
                        }
                        let mut legacy = serde_json::to_value(&snap).unwrap();
                        for sprite in legacy["npc_sprite_states"].as_array_mut().unwrap() {
                            sprite["visible"].as_object_mut().unwrap().remove("grass_priority").unwrap();
                            for pose in sprite["pending"].as_array_mut().unwrap() { pose.as_object_mut().unwrap().remove("grass_priority").unwrap(); }
                        }
                        let old: OverworldSnapshot = serde_json::from_value(legacy).unwrap();
                        assert!(old.npc_sprite_states.iter().all(|sprite|!sprite.visible.grass_priority && sprite.pending.iter().all(|pose|!pose.grass_priority)));
                    }
                }
            }
        });
    }

    #[test]
    #[ignore = "controlled valid Route1 NPC grass-priority capture"]
    fn capture_npc_grass_raw_136() {
        use pokered_core::overworld::Direction;
        use pokered_core::snapshot::OverworldSnapshot;
        use dotzuki_engine::overworld::collision::CollisionProvider as _;
        run_link_save_fixture(|| {
            let dir = std::path::PathBuf::from(std::env::var("FIDELITY_NPC_GRASS_CAPTURE").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let walking = std::env::var_os("FIDELITY_NPC_GRASS_WALK").is_some();
            let player_walk = std::env::var_os("FIDELITY_NPC_GRASS_PLAYER_WALK").is_some();
            let delay = std::env::var("FIDELITY_NPC_GRASS_FUTURE_DELAY").ok().map(|v|v.parse().unwrap()).unwrap_or(if walking {16} else {127});
            let (mut g, slot) = npc_grass_fixture_137(walking, delay);
            let idle = InputState::new();
            for _ in 0..4 { g.update(&idle); }
            let mut records = Vec::new();
            let mut input = InputState::new();
            g.overworld.state.encounter_cooldown = 255;
            for t in -1i32..if walking {30} else {20} {
                if t >= 0 {
                    input.begin_frame();
                    if player_walk && t == 0 { input.press(GbButton::Down); }
                    if player_walk && t == 16 { input.release(GbButton::Down); }
                    g.update(&input);
                }
                let mut fb = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
                g.draw(&mut fb);
                fb.save_png(&dir.join(format!("frame-{:04}.png",t+1))).unwrap();
                let state = OverworldSnapshot::capture(&g.overworld);
                records.push(serde_json::json!({"t":t,"map":g.overworld.state.current_map as u8,"player":g.overworld.state.player,"npc":state.npc_states[slot],"sprite":state.npc_sprite_states[slot]}));
            }
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&records).unwrap()).unwrap();
        });
    }

    #[test]
    #[ignore = "actual Continue/Strength/boulder input continuous capture"]
    fn capture_actual_boulder_dust_raw_93() {
        run_link_save_fixture(|| {
            use pokered_core::party_screen::PartyScreenPhase;
            let dir=std::path::PathBuf::from(std::env::var("FIDELITY_DUST_CAPTURE").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let path=dir.join("fixture.sav");
            std::fs::write(&path,std::fs::read(std::env::var("FIDELITY_DUST_SRAM").unwrap()).unwrap()).unwrap();
            let mut g=PokemonGame::new_with_options(GameVersion::Red,Some(path),None,None,false,None,false,true,
                #[cfg(feature="debug-server")] None);
            let idle=InputState::new(); let mut saw_menu=false;
            let scenario=std::env::var("FIDELITY_DUST_SCENARIO").unwrap_or_default();
            let victory_hole=scenario=="victory-hole";
            let victory_switch=match scenario.as_str() {
                "victory-switch1f" => Some((MapId::VictoryRoad1F,17,11,4,"EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH",4,6)),
                "victory-switch2f1" => Some((MapId::VictoryRoad2F,1,14,10,"EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH1",3,4)),
                "victory-switch2f2" => Some((MapId::VictoryRoad2F,9,14,10,"EVENT_VICTORY_ROAD_2_BOULDER_ON_SWITCH2",11,7)),
                _ => None,
            };
            for frame in 0..2000 {
                saw_menu |= g.state.screen==GameScreen::MainMenu;
                if g.state.screen==GameScreen::Overworld {break;}
                let advance=button(GbButton::A);g.update(if frame%20==19 {&advance} else {&idle});
            }
            assert!(saw_menu);assert_eq!(g.overworld.state.current_map,MapId::SeafoamIslands1F);
            assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),(18,9));
            // The extra hole fixture uses the normal map-load/debug warp
            // before the actual Strength menu, preserving a valid tile view.
            if victory_hole {
                g.overworld.warp_to_map(MapId::VictoryRoad3F,21,15);
                for _ in 0..120 {g.update(&idle);}
                assert_eq!(g.overworld.state.current_map,MapId::VictoryRoad3F);
                assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),(21,15));
            }
            if let Some((map,x,y,npc,_,_,_))=victory_switch {
                g.overworld.warp_to_map(map,x,y);
                for _ in 0..120 {g.update(&idle);}
                let n=g.overworld.npc_states.iter_mut().find(|n|n.npc_index==npc).unwrap();
                n.x=u16::from(x);n.y=u16::from(y)+1;n.walk_counter=0;
                g.overworld.state.player.facing=pokered_core::overworld::Direction::Down;
            }
            // Controlled no-encounter fixture matches original BIT_NO_BATTLES.
            g.overworld.state.encounter_cooldown=255;g.overworld.set_rng_seed(0);
            // Keep START through one field sample; a one-frame pulse can
            // fall entirely inside DelayFrame after the controlled map warp.
            let mut start_input=button(GbButton::Start);
            g.update(&start_input);start_input.begin_frame();g.update(&start_input);
            assert_eq!(g.state.screen,GameScreen::StartMenu);
            // The source completes DisplayTextIDInit/DrawStartMenu before
            // reading party-selection input; wait its first menu Joypad.
            for _ in 0..23 {g.update(&idle);}
            for _ in 0..7 {
                if g.start_menu.current_item()==pokered_core::start_menu::StartMenuItem::Pokemon {break;}
                g.update(&button(GbButton::Down));for _ in 0..3 {g.update(&idle);}
            }
            g.update(&button(GbButton::A));g.update(&idle);
            assert_eq!(g.state.screen,GameScreen::PartyScreen);
            for _ in 0..6 {
                if g.party_screen.cursor()==0 {break;}
                g.update(&button(GbButton::Up));g.update(&idle);
            }
            g.update(&button(GbButton::A));g.update(&idle);
            for _ in 0..3 {g.update(&button(GbButton::Down));g.update(&idle);}
            assert_eq!(g.party_screen.phase(),PartyScreenPhase::ActionMenu {cursor:3});
            g.update(&button(GbButton::A));g.update(&idle);
            for t in 0..1000 {
                if g.state.screen==GameScreen::Overworld && g.overworld.strength_active && g.overworld.pending_dialogue.is_none() {break;}
                let advance=button(GbButton::A);g.update(if t%20==19 {&advance} else {&idle});
            }
            assert_eq!(g.state.screen,GameScreen::Overworld);assert!(g.overworld.strength_active);
            assert!(g.overworld.pending_dialogue.is_none());
            let direction=if victory_hole {"right".into()} else {std::env::var("FIDELITY_DUST_DIRECTION").unwrap_or_else(|_|"down".into())};
            let (preparation,trigger,expected)=if victory_hole {
                (vec![],GbButton::Right,(21,15))
            } else if let Some((_,x,y,_,_,_,_))=victory_switch {
                (vec![],GbButton::Down,(u16::from(x),u16::from(y)))
            } else {match direction.as_str() {
                "down" => (vec![],GbButton::Down,(18,9)),
                "up" => (vec![GbButton::Right,GbButton::Down,GbButton::Down,GbButton::Left],GbButton::Up,(18,11)),
                "left" => (vec![GbButton::Right,GbButton::Down],GbButton::Left,(19,10)),
                "right" => (vec![GbButton::Left,GbButton::Down],GbButton::Right,(17,10)),
                _ => panic!("unknown boulder direction"),
            }};
            for b in preparation {
                let (x,y)=(g.overworld.state.player.x,g.overworld.state.player.y);
                let target=match b {
                    GbButton::Up=>(x,y-1),GbButton::Down=>(x,y+1),
                    GbButton::Left=>(x-1,y),GbButton::Right=>(x+1,y),_=>unreachable!(),
                };
                let mut walking=button(b);
                for t in 0..64 {
                    if t>0 {walking.begin_frame();}
                    g.update(&walking);
                    if (g.overworld.state.player.x,g.overworld.state.player.y)==target {break;}
                }
                assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),target);
                for _ in 0..30 {g.update(&idle);}
            }
            assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),expected);
            // Turn toward the boulder without a second contact. Ordinary turn
            // timing is a separate open audit; trigger recordings start after idle.
            if direction!="down" {
                let mut turn=button(trigger);g.update(&turn);turn.begin_frame();g.update(&turn);
            }
            for _ in 0..120 {g.update(&idle);}
            assert!(!g.overworld.boulder_dust.is_active());
            let menu_return_probe=std::env::var("FIDELITY_DUST_MENU_RETURN").is_ok();
            if menu_return_probe {
                let mut open=button(GbButton::Start);open.press(GbButton::Down);
                g.update(&open);open.begin_frame();g.update(&open);
                assert_eq!(g.state.screen,GameScreen::StartMenu);
                for _ in 0..60 {g.update(&idle);}
                let mut close=button(GbButton::B);
                g.update(&close);close.begin_frame();g.update(&close);
                for _ in 0..6 {g.update(&idle);}
                assert_eq!(g.state.screen,GameScreen::Overworld);
            }
            if std::env::var("FIDELITY_DUST_LOGICAL_AUDIO").is_ok() {g.audio=Some(AudioOutput::new_pcm());}
            let start_at=std::env::var("FIDELITY_DUST_START_AT").ok().map(|s|s.parse::<i32>().unwrap());
            let menu_only=std::env::var("FIDELITY_DUST_MENU_ONLY").is_ok();
            let menu_key=match std::env::var("FIDELITY_DUST_MENU_KEY").as_deref() {
                Ok("a")=>GbButton::A,Ok("b")=>GbButton::B,Ok("up")=>GbButton::Up,_=>GbButton::Down,
            };
            let menu_down_frames=std::env::var("FIDELITY_DUST_MENU_DOWN_FRAMES").ok().map(|s|s.parse::<i32>().unwrap()).unwrap_or(1);
            let menu_down_at=std::env::var("FIDELITY_DUST_MENU_DOWN_AT").ok().map(|s|s.parse::<i32>().unwrap());
            let start_frames=std::env::var("FIDELITY_DUST_START_FRAMES").ok().map(|s|s.parse::<i32>().unwrap()).unwrap_or(40);
            let mut input=InputState::new();let mut rows=Vec::new();
            for t in -1i32..200 {
                if t>=0 {
                    input.begin_frame();
                    if t==0 && !menu_return_probe && !menu_only {input.press(trigger);}
                    if t==16 {input.release(trigger);}
                    if start_at==Some(t) {input.press(GbButton::Start);}
                    if start_at.map(|v|v+start_frames)==Some(t) {input.release(GbButton::Start);}
                    if menu_down_at==Some(t) {input.press(menu_key);}
                    if menu_down_at.map(|v|v+menu_down_frames)==Some(t) {input.release(menu_key);}
                    g.update(&input);
                }
                let mut fb=FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);
                g.draw(&mut fb);fb.save_png(&dir.join(format!("frame-{:04}.png",t+1))).unwrap();
                let recorded_save=g.build_save_data();
                rows.push(serde_json::json!({"t":t,"frame":g.frame_count,"input_bits":input.raw_current(),
                    "last_stop":recorded_save.game_data.player_last_stop_direction,
                    "moving_direction":recorded_save.game_data.player_moving_direction,
                    "push_frame":g.overworld.boulder_push.map(|p|p.frame),
                    "start_item":format!("{:?}",g.start_menu.current_item()),
                    "start_initializing":g.start_menu.field_initialization_active(),
                    "sfx_playing":g.audio.as_ref().map(|a|a.is_sfx_playing()),
                    "sfx_id":g.audio.as_ref().map(|a|a.manager.lock().unwrap().sequencer.current_sfx_id),
                    "screen":format!("{:?}",g.state.screen),"x":g.overworld.state.player.x,"y":g.overworld.state.player.y,
                    "movement":format!("{:?}",g.overworld.state.player.movement_state),"walk_counter":g.overworld.state.walk_counter,
                    "dust_active":g.overworld.boulder_dust.is_active(),"dust_step":g.overworld.boulder_dust.step(),
                    "dust_flash":g.overworld.boulder_dust.palette_flipped(),"dust_anchor":g.overworld.boulder_dust.anchor(),
                    "npcs":g.overworld.npc_states.iter().map(|n|serde_json::json!({"id":n.npc_index,"x":n.x,"y":n.y,"walk_counter":n.walk_counter})).collect::<Vec<_>>() }));
                if victory_hole {
                    let row=rows.last_mut().unwrap();
                    row["hole_event"]=serde_json::json!(g.overworld.unified_flags().get_flag("EVENT_VICTORY_ROAD_3_BOULDER_ON_SWITCH2"));
                    row["boulder_visible"]=serde_json::json!(g.overworld.npc_states.iter().find(|n|n.text_id==10).unwrap().visible);
                }
                if let Some((_,_,_,npc,flag,bx,by))=victory_switch {
                    let row=rows.last_mut().unwrap();
                    row["switch"]=serde_json::json!(g.overworld.unified_flags().get_flag(flag));
                    let map=g.overworld.map_data.as_ref().unwrap();
                    row["block"]=serde_json::json!(map.blocks[by as usize*map.width as usize+bx as usize]);
                    row["boulder_visible"]=serde_json::json!(g.overworld.npc_states.iter().find(|n|n.npc_index==npc).unwrap().visible);
                }
            }
            if !menu_return_probe && !menu_only {assert!(rows.iter().any(|r|r["dust_active"]==true));}
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&rows).unwrap()).unwrap();
        });
    }

    #[test]
    fn menu_direction_delay_matches_original_short_and_held_pulses_108() {
        run_link_save_fixture(|| {
            // Original-ROM repeated probes: Down at 120, Up at 121/122 is
            // missed; 123/124/125 is accepted; Up held from 121 is read at 123.
            for (at, duration, first_up) in [
                (1, 1, None), (2, 1, None), (3, 1, Some(3)),
                (4, 1, Some(4)), (5, 1, Some(5)), (1, 8, Some(3)),
            ] {
                let mut g = fixture(Species::Bulbasaur, 3, pokered_core::overworld::Direction::Down);
                let idle = InputState::new();
                g.handle_transition(GameScreen::StartMenu);
                for _ in 0..23 { g.update(&idle); }
                g.update(&button(GbButton::Down));
                for _ in 0..27 { g.update(&idle); }
                assert_eq!(g.start_menu.current_item(), pokered_core::start_menu::StartMenuItem::Pokemon);
                let mut input = InputState::new();
                for t in 0..25 {
                    input.begin_frame();
                    if t == 0 { input.press(GbButton::Down); }
                    if t == 1 { input.release(GbButton::Down); }
                    if t == at { input.press(GbButton::Up); }
                    if t == at + duration { input.release(GbButton::Up); }
                    g.update(&input);
                    assert_eq!(g.state.screen, GameScreen::StartMenu);
                    let expected = if first_up.is_some_and(|first| t >= first) {
                        pokered_core::start_menu::StartMenuItem::Pokemon
                    } else { pokered_core::start_menu::StartMenuItem::Item };
                    assert_eq!(g.start_menu.current_item(), expected, "Up at {at} for {duration}, frame {t}");
                }
            }
        });
    }

    #[test]
    #[ignore = "START direction Delay3 matched-input comparison capture"]
    fn capture_menu_direction_boundary_108() {
        run_link_save_fixture(|| {
            let root = std::path::PathBuf::from(std::env::var("FIDELITY_MENU_DIRECTION_CAPTURE").unwrap());
            for (at, duration) in [(1, 1), (2, 1), (3, 1), (4, 1), (5, 1), (1, 8)] {
                for trial in 1..=2 {
                    let dir = root.join(format!("up-{at}-{duration}-{trial}"));
                    std::fs::create_dir_all(&dir).unwrap();
                    let mut g = fixture(Species::Bulbasaur, 3, pokered_core::overworld::Direction::Down);
                    let idle = InputState::new();
                    g.handle_transition(GameScreen::StartMenu);
                    for _ in 0..23 { g.update(&idle); }
                    g.update(&button(GbButton::Down));
                    for _ in 0..27 { g.update(&idle); }
                    assert_eq!(g.state.screen, GameScreen::StartMenu);
                    assert_eq!(g.start_menu.current_item(), pokered_core::start_menu::StartMenuItem::Pokemon);
                    let mut input = InputState::new();
                    let mut rows = Vec::new();
                    for t in -1i32..25 {
                        if t >= 0 {
                            input.begin_frame();
                            if t == 0 { input.press(GbButton::Down); }
                            if t == 1 { input.release(GbButton::Down); }
                            if t == at { input.press(GbButton::Up); }
                            if t == at + duration { input.release(GbButton::Up); }
                            g.update(&input);
                        }
                        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                        g.draw(&mut fb);
                        fb.save_png(&dir.join(format!("frame-{:04}.png", t + 1))).unwrap();
                        rows.push(serde_json::json!({"t": t, "input_bits": input.raw_current(),
                            "screen": format!("{:?}", g.state.screen),
                            "item": format!("{:?}", g.start_menu.current_item()),
                            "sfx_id": g.audio.as_ref().map(|a| a.manager.lock().unwrap().sequencer.current_sfx_id)}));
                    }
                    std::fs::write(dir.join("frames.json"), serde_json::to_string_pretty(&rows).unwrap()).unwrap();
                }
            }
        });
    }

    fn wait_stats_cry(game: &mut PokemonGame) {
        for _ in 0..120 {
            game.update(&InputState::new());
        }
        assert!(!game.audio.as_ref().unwrap().is_sfx_playing());
    }

    fn ordinary_muted_party_stats() -> PokemonGame {
        use pokered_core::overworld::Direction;
        let template = fixture(Species::Bulbasaur, 7, Direction::Down);
        let mut game = PokemonGame::new_with_options(
            GameVersion::Red, None, None, None, false, None, false, true,
            #[cfg(feature = "debug-server")] None,
        );
        game.save_data = template.save_data;
        game.state.screen = GameScreen::Overworld;
        game.main_menu.last_choice = Some(pokered_core::game_state::MainMenuChoice::Continue);
        // Retain the production --no-audio logical PCM sequencer.
        game.overworld = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
        game.overworld.set_event_flag_live(pokered_data::event_flags::EventFlag::EVENT_GOT_POKEDEX);
        game.overworld.set_event_flag_live(pokered_data::event_flags::EventFlag::EVENT_GOT_STARTER);
        game.overworld.state.player.x = 7; game.overworld.state.player.y = 7;
        game.overworld.run_on_load();
        let idle = InputState::new();
        for _ in 0..60 { game.update(&idle); }
        game.update(&button(GbButton::Start));
        for _ in 0..24 { game.update(&idle); }
        assert_eq!(game.state.screen, GameScreen::StartMenu);
        game.update(&button(GbButton::Down));
        for _ in 0..3 { game.update(&idle); }
        game.update(&button(GbButton::A));
        for _ in 0..4 { game.update(&idle); }
        assert_eq!(game.state.screen, GameScreen::PartyScreen);
        game.update(&button(GbButton::A));
        game.update(&button(GbButton::A));
        assert!(matches!(game.state.screen, GameScreen::PokemonStatsScreen(_)));
        game
    }

    #[test]
    fn actual_party_stats_no_audio_waits_for_cry_before_accepting_b() {
        run_link_save_fixture(|| {
            let mut game = ordinary_muted_party_stats();
            let idle = InputState::new();
            assert!(game.audio.as_ref().unwrap().is_sfx_playing());
            game.update(&button(GbButton::B));
            assert_eq!(game.stats_screen.as_ref().unwrap().page(), pokered_core::stats_screen::StatsPage::Stats,
                "ordinary muted party stats also waits for the logical cry");
            for _ in 0..120 { game.update(&idle); }
            game.update(&button(GbButton::B));
            assert_eq!(game.stats_screen.as_ref().unwrap().page(), pokered_core::stats_screen::StatsPage::Moves);
            game.update(&button(GbButton::A));
            assert_eq!(game.state.screen, GameScreen::PartyScreen);
        });
    }

    #[test]
    fn actual_trade_confirmation_clamps_vertical_cursor() {
        run_link_save_fixture(|| {
            let (mut host, _peer) = both_selected_pair();
            host.update(&button(GbButton::Up));
            assert!(matches!(host.link_cable.phase(), CableClubPhase::TradeConfirm { selected: 0, .. }),
                "HandleMenuInput UP at first choice does not wrap to CANCEL");
            host.update(&button(GbButton::Down));
            host.update(&InputState::new());
            host.update(&button(GbButton::Down));
            assert!(matches!(host.link_cable.phase(), CableClubPhase::TradeConfirm { selected: 1, .. }),
                "DOWN at last choice does not wrap to TRADE");
        });
    }

    #[test]
    fn actual_trade_confirmation_vertical_and_ab_choose_updated_item() {
        run_link_save_fixture(|| {
            let (mut host, mut peer) = both_selected_pair();
            let mut down_a = button(GbButton::Down); down_a.press(GbButton::A);
            host.update(&down_a);
            assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeWaitingConfirm,
                "HandleMenuInput moves DOWN then returns A in the same input");
            assert_eq!(host.link_cable.text_box().as_deref(), Some(crate::link::cable_club::TEXT_TRADE_CANCELED));
            peer.update(&button(GbButton::B));
            for _ in 0..120 { host.update(&InputState::new()); peer.update(&InputState::new()); }
            assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeSelect);
            let (mut host, _peer) = both_selected_pair();
            let mut a_b = button(GbButton::A); a_b.press(GbButton::B);
            host.update(&a_b);
            assert_eq!(host.link_cable.text_box().as_deref(), Some(crate::link::cable_club::TEXT_TRADE_CANCELED),
                "DisplayTwoOptionMenu B chooses CANCEL even with A");
        });
    }

    #[test]
    fn actual_trade_confirmation_up_and_a_confirms_after_moving() {
        run_link_save_fixture(|| {
            let (mut host, mut peer) = both_selected_pair();
            host.update(&button(GbButton::Down));
            let mut up_a = button(GbButton::Up); up_a.press(GbButton::A);
            host.update(&up_a);
            assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeWaitingConfirm);
            assert_eq!(host.link_cable.text_box().as_deref(), Some(crate::link::cable_club::TEXT_WAITING));
            peer.update(&button(GbButton::A));
            for _ in 0..120 { host.update(&InputState::new()); peer.update(&InputState::new()); }
            assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeAnim,
                "both real confirmations start the exchange");
        });
    }

    #[test]
    fn actual_trade_confirmation_b_overrides_simultaneous_a() {
        run_link_save_fixture(|| {
            let (mut host, _peer) = both_selected_pair();
            let mut a_b = button(GbButton::A); a_b.press(GbButton::B);
            host.update(&a_b);
            assert_eq!(host.link_cable.text_box().as_deref(), Some(crate::link::cable_club::TEXT_TRADE_CANCELED),
                "DisplayTwoOptionMenu B chooses CANCEL even with A");
        });
    }

    #[test]
    fn actual_confirmation_no_is_not_a_list_cancel() {
        let (mut host, mut peer) = both_selected_pair();
        let host_party = serde_json::to_value(&host.save_data.party).unwrap();
        let peer_party = serde_json::to_value(&peer.save_data.party).unwrap();
        let idle = InputState::new();
        host.update(&button(GbButton::B));
        peer.update(&idle);
        peer.update(&button(GbButton::B));
        for _ in 0..120 { host.update(&idle); peer.update(&idle); }
        for g in [&mut host, &mut peer] {
            assert_eq!(g.link_cable.phase(), &CableClubPhase::TradeSelect);
            assert_eq!(g.link_cable.text_box(), None);
        }
        peer.update(&button(GbButton::Down)); peer.update(&button(GbButton::A));
        host.update(&idle);
        assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeSelect, "earlier NO must not count as own list CANCEL");
        assert_eq!(peer.link_cable.phase(), &CableClubPhase::TradeWaitingPeer);
        assert_eq!(serde_json::to_value(&host.save_data.party).unwrap(), host_party);
        assert_eq!(serde_json::to_value(&peer.save_data.party).unwrap(), peer_party);
        host.update(&button(GbButton::Down)); host.update(&button(GbButton::A));
        for _ in 0..20 { host.update(&idle); peer.update(&idle); }
        assert_eq!(host.link_cable.phase(), &CableClubPhase::InRoom);
        assert_eq!(peer.link_cable.phase(), &CableClubPhase::InRoom);
    }

    #[test]
    fn actual_peer_cancel_does_not_interrupt_stats_and_voids_both_indices() {
        let (mut host, mut peer) = paired_trade_room();
        let idle = InputState::new();
        peer.update(&button(GbButton::A)); peer.update(&idle);
        peer.update(&button(GbButton::A));
        assert!(peer.link_cable.stats().is_some());
        host.update(&button(GbButton::Down)); host.update(&button(GbButton::A));
        peer.update(&idle);
        assert!(peer.link_cable.stats().is_some(), "serial cancel waits for our choice");
        assert!(peer.link_cable.text_box().is_none());
        wait_stats_cry(&mut peer);
        peer.update(&button(GbButton::B)); peer.update(&button(GbButton::A)); peer.update(&idle);
        choose_actual_trade(&mut peer);
        host.update(&idle);
        assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeSelect);
        assert!(host.link_cable.cancel_selected());
        assert_eq!(peer.link_cable.phase(), &CableClubPhase::TradeSelect);
        assert!(peer.link_cable.text_box().is_none());
        host.update(&button(GbButton::Up)); choose_actual_trade(&mut host);
        peer.update(&idle);
        assert_eq!(peer.link_cable.phase(), &CableClubPhase::TradeSelect, "voided old index must not confirm a new round");
        choose_actual_trade(&mut peer);
        for _ in 0..20 { host.update(&idle); peer.update(&idle); }
        assert!(matches!(host.link_cable.phase(), CableClubPhase::TradeConfirm { .. }));
        assert!(matches!(peer.link_cable.phase(), CableClubPhase::TradeConfirm { .. }));
    }

    #[test]
    fn actual_yes_then_no_rejects_without_payload_error_or_party_change() {
        let (mut host, mut peer) = both_selected_pair();
        let host_party = serde_json::to_value(&host.save_data.party).unwrap();
        let peer_party = serde_json::to_value(&peer.save_data.party).unwrap();
        let idle = InputState::new();
        peer.update(&button(GbButton::A)); // YES first; our NO follows while its messages arrive.
        host.update(&button(GbButton::B));
        for _ in 0..120 { host.update(&idle); peer.update(&idle); }
        assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeSelect);
        assert_eq!(peer.link_cable.phase(), &CableClubPhase::TradeSelect);
        assert_eq!(serde_json::to_value(&host.save_data.party).unwrap(), host_party);
        assert_eq!(serde_json::to_value(&peer.save_data.party).unwrap(), peer_party);
    }

    #[test]
    fn actual_trade_lists_clamp_and_watch_cancel_a_and_up_only() {
        let (mut host, mut peer) = paired_trade_room();
        let idle = InputState::new();
        host.update(&button(GbButton::Up));
        assert_eq!(host.link_cable.party_select().unwrap().cursor(), 0);
        host.update(&button(GbButton::B));
        assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeSelect);
        host.update(&button(GbButton::Down));
        assert!(host.link_cable.cancel_selected());
        for key in [GbButton::Down, GbButton::B, GbButton::Left, GbButton::Right] {
            host.update(&button(key)); host.update(&idle);
            assert!(host.link_cable.cancel_selected());
            assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeSelect);
        }
        host.update(&button(GbButton::Up));
        assert!(!host.link_cable.cancel_selected());
        assert_eq!(host.link_cable.peer_cursor(), None);
        host.update(&button(GbButton::Right));
        host.update(&button(GbButton::B));
        assert_eq!(host.link_cable.peer_cursor(), Some(0), "B not watched by peer list");
        host.update(&button(GbButton::Down));
        assert!(host.link_cable.cancel_selected());
        host.update(&button(GbButton::Up));
        assert_eq!(host.link_cable.peer_cursor(), None, "CANCEL UP returns own list");
        assert_eq!(host.link_cable.party_select().unwrap().cursor(), 0);
        host.update(&button(GbButton::Down));
        host.update(&button(GbButton::A));
        assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeWaitingPeer);
        // Receiving $f does not print confirmation-rejection text or consume
        // an extra A; our own CANCEL must acknowledge it and leave too.
        peer.update(&idle);
        assert!(peer.link_cable.text_box().is_none());
        peer.update(&button(GbButton::Down));
        assert!(peer.link_cable.cancel_selected());
        peer.update(&button(GbButton::A));
        for _ in 0..20 { host.update(&idle); peer.update(&idle); }
        assert_eq!(host.link_cable.phase(), &CableClubPhase::InRoom);
        assert_eq!(peer.link_cable.phase(), &CableClubPhase::InRoom);
        assert_eq!(host.overworld.state.current_map, MapId::TradeCenter);
        assert_eq!(peer.overworld.state.current_map, MapId::TradeCenter);
    }

    #[test]
    fn actual_trade_list_simultaneous_keys_keep_original_priority() {
        let (mut host, _peer) = paired_trade_room();
        let mut down_a = InputState::new(); down_a.press(GbButton::Down); down_a.press(GbButton::A);
        host.update(&down_a);
        assert_eq!(host.link_cable.local_action(), Some((0, false)));
        assert!(!host.link_cable.cancel_selected());
        host.update(&button(GbButton::B));
        host.update(&button(GbButton::Right));
        host.update(&down_a);
        assert_eq!(host.link_cable.stats().unwrap().pokemon().species, Species::Pikachu);
        assert!(!host.link_cable.cancel_selected());
        wait_stats_cry(&mut host);
        host.update(&button(GbButton::B));
        host.update(&button(GbButton::A));
        host.update(&button(GbButton::Down));
        assert!(host.link_cable.cancel_selected());
        let mut up_a = InputState::new();
        up_a.press(GbButton::Up); up_a.press(GbButton::A);
        host.update(&up_a);
        assert_eq!(host.link_cable.phase(), &CableClubPhase::TradeWaitingPeer,
            "CANCEL watches A before UP");
    }

    fn unequal_party_pair() -> (PokemonGame, PokemonGame) {
        use pokered_core::overworld::Direction;
        let mut host = fixture(Species::Bulbasaur, 3, Direction::Right);
        let mut peer = fixture(Species::Pikachu, 6, Direction::Left);
        for species in [Species::Pidgey, Species::Rattata] {
            host.save_data
                .party
                .add(
                    create_pokemon_with_moves(
                        species,
                        25,
                        [0x99, 0x88],
                        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        peer.save_data
            .party
            .add(
                create_pokemon_with_moves(
                    Species::Squirtle,
                    25,
                    [0x99, 0x88],
                    [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
                )
                .unwrap(),
            )
            .unwrap();
        linked_trade_room(host, peer)
    }

    #[test]
    fn actual_side_switch_copies_cursor_and_clamps_to_other_party() {
        let (mut host, mut peer) = unequal_party_pair();
        let idle = InputState::new();
        host.update(&button(GbButton::Down));
        host.update(&idle);
        host.update(&button(GbButton::Down));
        assert_eq!(host.link_cable.party_select().unwrap().cursor(), 2);
        host.update(&button(GbButton::Right));
        assert_eq!(host.link_cable.peer_cursor(), Some(1));
        host.update(&button(GbButton::Left));
        assert_eq!(host.link_cable.peer_cursor(), None);
        assert_eq!(host.link_cable.party_select().unwrap().cursor(), 1);
        host.update(&button(GbButton::Right));
        host.update(&button(GbButton::Up));
        assert_eq!(host.link_cable.peer_cursor(), Some(0));
        host.update(&button(GbButton::Left));
        assert_eq!(host.link_cable.party_select().unwrap().cursor(), 0);
        peer.update(&button(GbButton::Down));
        peer.update(&button(GbButton::Right));
        assert_eq!(peer.link_cable.peer_cursor(), Some(1));
        peer.update(&button(GbButton::Down));
        assert_eq!(peer.link_cable.peer_cursor(), Some(2));
        peer.update(&button(GbButton::Left));
        assert_eq!(peer.link_cable.party_select().unwrap().cursor(), 1);
    }

    #[test]
    #[ignore = "writes matched post-trade serial captures to FIDELITY_LINK_CAPTURES"]
    fn capture_post_trade_serial_sync() {
        run_link_save_fixture(|| {
            use pokered_core::overworld::Direction;
            let dir = PathBuf::from(std::env::var("FIDELITY_LINK_CAPTURES").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let mut records = Vec::new();
            let mut capture = |stage: &str, g: &mut PokemonGame| {
                let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                g.draw(&mut fb); fb.save_png(&dir.join(format!("{stage}.png"))).unwrap();
                records.push(serde_json::json!({"stage":stage,"frame":g.frame_count,
                    "phase":format!("{:?}",g.link_cable.phase()),
                    "party":serde_json::to_value(&g.save_data.party).unwrap(),
                    "committed_party":serde_json::to_value(&serde_json::from_str::<MobileSave>(&g.export_mobile_save().unwrap()).unwrap().data.party).unwrap()}));
            };
            let host = fixture(Species::Bulbasaur, 3, Direction::Right);
            let peer = fixture(Species::Kadabra, 6, Direction::Left);
            let (mut host, mut peer) = linked_trade_room(host, peer);
            choose_actual_trade(&mut host); choose_actual_trade(&mut peer);
            let idle = InputState::new();
            for _ in 0..20 { host.update(&idle); peer.update(&idle); }
            host.update(&button(GbButton::A)); peer.update(&button(GbButton::A));
            let mut saw_evolution = false;
            let mut captured = false;
            let mut finished = false;
            for _ in 0..12000 {
                host.update(&idle); peer.update(&idle);
                if host.evolution_anim.is_some() {
                    saw_evolution = true;
                    if !captured && peer.trade_anim.is_none() {
                        capture("peer-during-evolution", &mut peer);
                        captured = true;
                    }
                }
                if saw_evolution && host.evolution_anim.is_none() { finished = true; break; }
            }
            assert!(captured && finished);
            for _ in 0..30 { host.update(&idle); peer.update(&idle); }
            capture("completion-delay", &mut host);
            for _ in 0..31 { host.update(&idle); peer.update(&idle); }
            capture("trade-result", &mut host);
            std::fs::write(dir.join("frames.json"), serde_json::to_string_pretty(&records).unwrap()).unwrap();
        });
    }

    #[test]
    #[ignore = "writes matched NPC trade dialogue captures to FIDELITY_NPC_TRADE_CAPTURES"]
    fn capture_npc_trade_dialogue() {
        run_link_save_fixture(|| {
            use pokered_core::overworld::Direction;
            let dir = PathBuf::from(std::env::var("FIDELITY_NPC_TRADE_CAPTURES").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let idle = InputState::new();
            let setup = |map, x, y, species| {
                let mut g = fixture(species,x,Direction::Up);
                g.audio = None;
                g.overworld = OverworldScreen::new(map,None,PokemonRedData);
                g.overworld.state.player.x=x; g.overworld.state.player.y=y;
                g.overworld.state.player.facing=Direction::Up;
                g.overworld.run_on_load();
                for npc in &mut g.overworld.npc_states { npc.movement_type=pokered_core::overworld::NpcMovementType::Stationary; }
                for _ in 0..120 { g.update(&idle); }
                g.update(&button(GbButton::A));
                g
            };
            let mut records = Vec::new();
            let mut capture = |stage: &str, g: &mut PokemonGame| {
                let mut fb = FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);
                g.draw(&mut fb); fb.save_png(&dir.join(format!("{stage}.png"))).unwrap();
                records.push(serde_json::json!({"stage":stage,"frame":g.frame_count,
                    "map":format!("{:?}",g.overworld.state.current_map),
                    "position":[g.overworld.state.player.x,g.overworld.state.player.y],
                    "party":g.save_data.party.iter().map(|m|serde_json::json!({"species":format!("{:?}",m.species),"level":m.level,"moves":m.moves})).collect::<Vec<_>>(),
                    "dialogue":g.overworld.pending_dialogue.as_ref().map(|d|d.pages().iter().map(|p|format!("{} {}",p.line1,p.line2)).collect::<Vec<_>>())}));
            };
            let mut g=setup(MapId::Route2TradeHouse,4,2,Species::Abra);
            let mut saw_movie=false;
            let mut done=false;
            for n in 0..12000 {
                let input=if n%20==0 {button(GbButton::A)} else {InputState::new()};
                g.update(&input);
                saw_movie |= g.trade_anim.is_some();
                if saw_movie && g.trade_anim.is_none() && g.pending_trade.is_none() { done=true; break; }
            }
            assert!(done);for _ in 0..180 {g.update(&idle);}
            capture("summary-order",&mut g);
            let mut g=setup(MapId::CeruleanTradeHouse,1,3,Species::Poliwhirl);
            for _ in 0..180 {g.update(&idle);}
            capture("cerulean-offer",&mut g);
            let mut g=setup(MapId::CinnabarLabTradeRoom,1,5,Species::Bulbasaur);
            let mut wrong=false;
            for n in 0..12000 {
                let input=if n%20==0 {button(GbButton::A)} else {InputState::new()};g.update(&input);
                if g.overworld.pending_dialogue.as_ref().is_some_and(|d|d.pages().iter().any(|p|p.line1.contains("Hmmm?") || p.line1.contains("...This"))) {wrong=true;break;}
            }
            assert!(wrong);for _ in 0..180 {g.update(&idle);}
            capture("wrong-raichu",&mut g);
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&records).unwrap()).unwrap();
        });
    }

    #[test]
    #[ignore = "writes matched confirmation captures to FIDELITY_LINK_CAPTURES"]
    fn capture_trade_confirmation_keys() {
        run_link_save_fixture(|| {
            let dir = PathBuf::from(std::env::var("FIDELITY_LINK_CAPTURES").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let mut records = Vec::new();
            for (stage, keys) in [
                ("confirm-up-edge", vec![GbButton::Up]),
                ("confirm-down-a", vec![GbButton::Down, GbButton::A]),
                ("confirm-a-b", vec![GbButton::A, GbButton::B]),
            ] {
                let (mut host, _peer) = both_selected_pair();
                let mut input = InputState::new();
                for key in keys { input.press(key); }
                host.update(&input);
                let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                host.draw(&mut fb);
                fb.save_png(&dir.join(format!("{stage}.png"))).unwrap();
                records.push(serde_json::json!({"stage":stage,"frame":host.frame_count,
                    "phase":format!("{:?}",host.link_cable.phase()),
                    "party":serde_json::to_value(&host.save_data.party).unwrap(),
                    "text":host.link_cable.text_box()}));
            }
            std::fs::write(dir.join("frames.json"), serde_json::to_string_pretty(&records).unwrap()).unwrap();
        });
    }

    #[test]
    #[ignore = "writes matched gameboy role and room captures to FIDELITY_LINK_CAPTURES"]
    fn capture_gameboy_roles_and_room_entry() {
        run_link_save_fixture(|| {
            use pokered_core::overworld::Direction;
            let dir = PathBuf::from(std::env::var("FIDELITY_LINK_CAPTURES").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let mut records = Vec::new();
            let mut capture = |stage: &str, g: &mut PokemonGame| {
                let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                g.draw(&mut fb); fb.save_png(&dir.join(format!("{stage}.png"))).unwrap();
                records.push(serde_json::json!({"stage":stage,"frame":g.frame_count,
                    "phase":format!("{:?}",g.link_cable.phase()),
                    "party":serde_json::to_value(&g.save_data.party).unwrap(),
                    "position":[g.overworld.state.player.x,g.overworld.state.player.y],
                    "npcs":g.overworld.npc_states.iter().map(|n|serde_json::json!({"id":n.text_id,"x":n.x,"y":n.y,"facing":format!("{:?}",n.facing)})).collect::<Vec<_>>()}));
            };
            let idle = InputState::new();
            let mut local = fixture(Species::Bulbasaur, 4, Direction::Up);
            local.overworld.state.player.y = 5;
            let mut peer = fixture(Species::Pikachu, 7, Direction::Down);
            peer.overworld.state.player.y = 6;
            let (a,b) = ChannelTransport::new_pair();
            local.attach_link_transport(Box::new(a), LinkRole::Host);
            peer.attach_link_transport(Box::new(b), LinkRole::Guest);
            for _ in 0..120 { local.update(&idle); peer.update(&idle); }
            local.update(&button(GbButton::A)); peer.update(&idle);
            for _ in 0..20 { local.update(&idle); peer.update(&idle); }
            capture("wrong-facing", &mut local);
            for role in [LinkRole::Host, LinkRole::Guest] {
                let mut local = fixture(Species::Bulbasaur, 11, Direction::Up);
                let mut peer = fixture(Species::Pikachu, 11, Direction::Up);
                for g in [&mut local, &mut peer] {
                    g.overworld = OverworldScreen::new(MapId::ViridianPokecenter, None, PokemonRedData);
                    g.overworld.set_event_flag_live(pokered_data::event_flags::EventFlag::EVENT_GOT_POKEDEX);
                    g.overworld.state.player.x = 11; g.overworld.state.player.y = 3;
                    g.overworld.state.player.facing = Direction::Up; g.overworld.run_on_load();
                }
                let (a,b) = ChannelTransport::new_pair();
                local.attach_link_transport(Box::new(a), role);
                peer.attach_link_transport(Box::new(b), if role == LinkRole::Host { LinkRole::Guest } else { LinkRole::Host });
                for _ in 0..120 { local.update(&idle); peer.update(&idle); }
                local.update(&button(GbButton::A)); peer.update(&idle);
                for frame in 0..2000 {
                    if matches!(local.link_cable.phase(), CableClubPhase::ReceptionSave { .. }) { break; }
                    let advance = button(GbButton::A);
                    local.update(if frame % 20 == 19 { &advance } else { &idle }); peer.update(&idle);
                }
                assert!(matches!(local.link_cable.phase(), CableClubPhase::ReceptionSave { .. }));
                local.update(&button(GbButton::A)); peer.update(&idle);
                local.update(&idle); local.update(&button(GbButton::A)); peer.update(&idle);
                for _ in 0..1000 {
                    local.update(&idle); peer.update(&idle);
                    if local.overworld.state.current_map == MapId::TradeCenter { break; }
                }
                assert_eq!(local.overworld.state.current_map, MapId::TradeCenter);
                for _ in 0..120 { local.update(&idle); peer.update(&idle); }
                capture(if role == LinkRole::Host { "room-host" } else { "room-guest" }, &mut local);
            }
            std::fs::write(dir.join("frames.json"), serde_json::to_string_pretty(&records).unwrap()).unwrap();
        });
    }

    #[test]
    #[ignore = "writes matched input/cry captures to FIDELITY_LINK_CAPTURES"]
    fn capture_menu_input_and_cry_fidelity() {
        run_link_save_fixture(|| {
            let dir = PathBuf::from(std::env::var("FIDELITY_LINK_CAPTURES").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let mut records = Vec::new();
            let mut capture = |stage: &str, g: &mut PokemonGame| {
                let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                g.draw(&mut fb);
                fb.save_png(&dir.join(format!("{stage}.png"))).unwrap();
                records.push(serde_json::json!({"stage":stage,"frame":g.frame_count,
                    "phase":format!("{:?}",g.link_cable.phase()),
                    "screen":format!("{:?}",g.state.screen),
                    "party":serde_json::to_value(&g.save_data.party).unwrap(),
                    "local_action":g.link_cable.local_action(),
                    "peer_cursor":g.link_cable.peer_cursor(),
                    "stats_page":g.link_cable.stats().map(|s|format!("{:?}",s.page())),
                    "ordinary_stats_page":g.stats_screen.as_ref().map(|s|format!("{:?}",s.page()))}));
            };
            let (mut host, _peer) = paired_trade_room();
            let mut right_a = button(GbButton::Right); right_a.press(GbButton::A);
            host.update(&right_a);
            capture("party-right-a", &mut host);
            let (mut host, _peer) = paired_trade_room();
            host.update(&button(GbButton::A));
            host.update(&right_a);
            capture("action-right-a", &mut host);
            let (mut host, _peer) = unequal_party_pair();
            let mut down_right = button(GbButton::Down); down_right.press(GbButton::Right);
            host.update(&down_right);
            capture("party-down-right", &mut host);
            let (mut host, _peer) = paired_trade_room();
            host.update(&button(GbButton::Right)); host.update(&button(GbButton::A));
            host.update(&button(GbButton::B));
            capture("link-stats-early-b", &mut host);
            let mut game = ordinary_muted_party_stats();
            game.update(&button(GbButton::B));
            capture("party-stats-early-b", &mut game);
            std::fs::write(dir.join("frames.json"), serde_json::to_string_pretty(&records).unwrap()).unwrap();
        });
    }

    #[test]
    #[ignore = "writes matched gameboy startup captures to FIDELITY_LINK_CAPTURES"]
    fn capture_gameboy_startup_fidelity() {
        run_link_save_fixture(|| {
            use pokered_core::overworld::Direction;
            let dir = PathBuf::from(std::env::var("FIDELITY_LINK_CAPTURES").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let mut host = fixture(Species::Bulbasaur, 3, Direction::Right);
            let mut peer = fixture(Species::Pikachu, 7, Direction::Down);
            peer.overworld.state.player.y = 6;
            let (a, b) = ChannelTransport::new_pair();
            host.attach_link_transport(Box::new(a), LinkRole::Host);
            peer.attach_link_transport(Box::new(b), LinkRole::Guest);
            let idle = InputState::new();
            for _ in 0..120 { host.update(&idle); peer.update(&idle); }
            host.update(&button(GbButton::A)); peer.update(&idle);
            for _ in 0..2 { host.update(&idle); peer.update(&idle); }
            host.update(&button(GbButton::A)); peer.update(&idle);
            let mut records = Vec::new();
            let mut capture = |stage: &str, g: &mut PokemonGame| {
                let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                g.draw(&mut fb);
                fb.save_png(&dir.join(format!("{stage}.png"))).unwrap();
                records.push(serde_json::json!({"stage":stage,"frame":g.frame_count,
                    "phase":format!("{:?}",g.link_cable.phase()),
                    "party":serde_json::to_value(&g.save_data.party).unwrap(),
                    "position":[g.overworld.state.player.x,g.overworld.state.player.y],
                    "text":g.link_cable.text_box()}));
            };
            for _ in 0..79 { host.update(&idle); peer.update(&idle); }
            capture("before-serial-frame79", &mut host);
            host.update(&idle); peer.update(&idle);
            capture("serial-frame80", &mut host);
            for _ in 0..40 { host.update(&idle); peer.update(&idle); }
            capture("peer-request-away", &mut peer);
            std::fs::write(dir.join("frames.json"), serde_json::to_string_pretty(&records).unwrap()).unwrap();
        });
    }

    #[test]
    #[ignore = "writes matched post-trade captures to FIDELITY_LINK_CAPTURES"]
    fn capture_post_trade_fidelity() {
        use pokered_core::evolution_screen::EvolutionPhase;
        use pokered_core::overworld::Direction;
        let dir = PathBuf::from(std::env::var("FIDELITY_LINK_CAPTURES").unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        let idle = InputState::new();
        let mut records = Vec::new();
        let mut capture = |stage: &str, g: &mut PokemonGame| {
            let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            g.draw(&mut fb);
            fb.save_png(&dir.join(format!("{stage}.png"))).unwrap();
            records.push(serde_json::json!({"stage":stage,"frame":g.frame_count,
                "phase":format!("{:?}",g.link_cable.phase()),
                "evolution":g.evolution_anim.as_ref().map(|a|format!("{:?}",a.phase())),
                "party":serde_json::to_value(&g.save_data.party).unwrap(),
                "committed":g.export_mobile_save()}));
        };
        let (mut host, mut peer) = both_selected_pair();
        host.update(&button(GbButton::B));
        for _ in 0..120 { host.update(&idle); peer.update(&idle); }
        capture("reject-unanswered", &mut peer);
        peer.update(&button(GbButton::B));
        for _ in 0..110 { host.update(&idle); peer.update(&idle); }
        capture("reject-auto-return", &mut host);
        let (mut host, mut peer, _) = completed_actual_trade();
        for _ in 0..50 { host.update(&idle); peer.update(&idle); }
        capture("trade-auto-return", &mut host);
        let host = fixture(Species::Bulbasaur, 3, Direction::Right);
        let mut peer = fixture(Species::Pikachu, 6, Direction::Left);
        peer.save_data.party = pokered_core::pokemon::party::Party::from(vec![
            create_pokemon_with_moves(Species::Kadabra, 25, [0x99, 0x88],
                [MoveId::Confusion, MoveId::None, MoveId::None, MoveId::None]).unwrap()
        ]);
        let (mut host, mut peer) = linked_trade_room(host, peer);
        choose_actual_trade(&mut host); choose_actual_trade(&mut peer);
        for _ in 0..20 { host.update(&idle); peer.update(&idle); }
        host.update(&button(GbButton::A)); peer.update(&button(GbButton::A));
        for _ in 0..12000 {
            host.update(&idle); peer.update(&idle);
            if host.evolution_anim.is_some() { break; }
        }
        assert!(host.evolution_anim.is_some());
        capture("trade-evolution-name", &mut host);
        for _ in 0..12000 {
            if host.evolution_anim.as_ref().map(|a|a.phase()) == Some(EvolutionPhase::Morph) { break; }
            host.update(&idle); peer.update(&idle);
        }
        assert_eq!(host.evolution_anim.as_ref().map(|a|a.phase()), Some(EvolutionPhase::Morph));
        host.update(&button(GbButton::B)); peer.update(&idle);
        for _ in 0..20 { host.update(&idle); peer.update(&idle); }
        capture("trade-forced-evolution", &mut host);
        std::fs::write(dir.join("frames.json"), serde_json::to_string_pretty(&records).unwrap()).unwrap();
    }

    #[test]
    #[ignore = "writes matched cancel and rejection captures to FIDELITY_LINK_CAPTURES"]
    fn capture_cancel_and_rejection() {
        let dir = PathBuf::from(std::env::var("FIDELITY_LINK_CAPTURES").unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        let idle = InputState::new();
        let mut records = Vec::new();
        let mut capture = |stage: &str, g: &mut PokemonGame| {
            let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            g.draw(&mut fb);
            fb.save_png(&dir.join(format!("{stage}.png"))).unwrap();
            records.push(serde_json::json!({"stage":stage,"frame":g.frame_count,
                "phase":format!("{:?}",g.link_cable.phase()),
                "party":serde_json::to_value(&g.save_data.party).unwrap()}));
        };
        let (mut host, mut peer) = paired_trade_room();
        host.update(&button(GbButton::Down)); host.update(&idle);
        capture("cancel-row", &mut host);
        host.update(&button(GbButton::A)); host.update(&idle); peer.update(&idle);
        capture("cancel-wait", &mut host);
        peer.update(&button(GbButton::Down)); peer.update(&idle);
        peer.update(&button(GbButton::A)); peer.update(&idle); host.update(&idle);
        for _ in 0..20 { host.update(&idle); peer.update(&idle); }
        capture("sequential-exit", &mut peer);
        let (mut host, mut peer) = both_selected_pair();
        peer.update(&button(GbButton::A)); host.update(&button(GbButton::B));
        for _ in 0..20 { host.update(&idle); peer.update(&idle); }
        capture("yes-no", &mut host);
        std::fs::write(dir.join("frames.json"), serde_json::to_string_pretty(&records).unwrap()).unwrap();
    }

    #[test]
    #[ignore = "writes same-input linked-game captures to FIDELITY_LINK_CAPTURES"]
    fn capture_own_mon_menu_and_stats() {
        let dir = PathBuf::from(std::env::var("FIDELITY_LINK_CAPTURES").unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        let (mut host, mut peer) = paired_trade_room();
        let idle = InputState::new();
        let mut records = Vec::new();
        for stage in ["own-menu", "own-stats"] {
            host.update(&button(GbButton::A));
            host.update(&idle);
            peer.update(&idle);
            let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            host.draw(&mut fb);
            fb.save_png(&dir.join(format!("{stage}.png"))).unwrap();
            records.push(serde_json::json!({"stage": stage,"frame":host.frame_count,
                "phase":format!("{:?}",host.link_cable.phase()),
                "stats":host.link_cable.stats().map(|s|format!("{:?}",s.pokemon().species))}));
        }
        let (mut host, mut peer) = unequal_party_pair();
        host.update(&button(GbButton::Down));
        host.update(&idle);
        host.update(&button(GbButton::Down));
        host.update(&button(GbButton::Right));
        host.update(&idle);
        peer.update(&idle);
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        host.draw(&mut fb);
        fb.save_png(&dir.join("side-cursor.png")).unwrap();
        records.push(
            serde_json::json!({"stage":"side-cursor", "frame":host.frame_count,
            "peer_cursor":host.link_cable.peer_cursor()}),
        );
        std::fs::write(
            dir.join("frames.json"),
            serde_json::to_string_pretty(&records).unwrap(),
        )
        .unwrap();
    }
    #[test]
    #[ignore = "actual SRAM Continue and party-cancel restoration recording"]
    fn capture_actual_party_restore_140() {
        run_link_save_fixture(|| {
            let dir = std::path::PathBuf::from(std::env::var("FIDELITY_PARTY_RESTORE_CAPTURE").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let save_path = dir.join("fixture.sav");
            std::fs::copy(std::env::var("FIDELITY_PARTY_RESTORE_SRAM").unwrap(), &save_path).unwrap();
            let mut g = PokemonGame::new_with_options(GameVersion::Red, Some(save_path), None, None,
                false, None, false, true, #[cfg(feature = "debug-server")] None);
            g.audio = Some(AudioOutput::new_pcm());
            g.state.config.language = pokered_core::game_state::Lang::En;
            let idle = InputState::new();
            let mut saw_menu = false;
            for t in 0..2000 {
                saw_menu |= g.state.screen == GameScreen::MainMenu;
                if g.state.screen == GameScreen::Overworld { break; }
                let a = button(GbButton::A);
                g.update(if t % 20 == 19 { &a } else { &idle });
            }
            assert!(saw_menu);
            assert_eq!(g.overworld.state.current_map, MapId::ViridianCity);
            assert_eq!((g.overworld.state.player.x, g.overworld.state.player.y), (20, 30));
            for _ in 0..120 { g.update(&idle); }
            let mut start = button(GbButton::Start);
            for _ in 0..40 { g.update(&start); start.begin_frame(); }
            for _ in 0..120 { g.update(&idle); }
            assert_eq!(g.state.screen, GameScreen::StartMenu);
            for _ in 0..8 {
                if g.start_menu.current_item() == pokered_core::start_menu::StartMenuItem::Pokemon { break; }
                g.update(&button(GbButton::Up));
                for _ in 0..121 { g.update(&idle); }
            }
            assert_eq!(g.start_menu.current_item(), pokered_core::start_menu::StartMenuItem::Pokemon);
            let mut a = button(GbButton::A);
            for _ in 0..2 { g.update(&a); a.begin_frame(); }
            for _ in 0..120 { g.update(&idle); }
            assert_eq!(g.state.screen, GameScreen::PartyScreen);
            assert_eq!(g.save_data.party.count(), 5);
            let probe = std::env::var("FIDELITY_PARTY_RESTORE_PROBE").is_ok();
            let mut rows = Vec::new();
            let mut input = InputState::new();
            for t in -1i32..80 {
                if t >= 0 {
                    input.begin_frame();
                    if t == 0 { input.press(GbButton::B); }
                    if t == 2 { input.release(GbButton::B); }
                    if probe {
                        if t == 10 { input.press(GbButton::Down); }
                        if t == 12 { input.release(GbButton::Down); }
                        if t == 16 { input.press(GbButton::A); }
                        if t == 18 { input.release(GbButton::A); }
                    }
                    g.update(&input);
                }
                let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                g.draw(&mut fb);
                fb.save_png(&dir.join(format!("frame-{:04}.png", t + 1))).unwrap();
                rows.push(serde_json::json!({"t":t,"input_bits":input.raw_current(),
                    "screen":format!("{:?}",g.state.screen),"item":format!("{:?}",g.start_menu.current_item()),
                    "party_phase":format!("{:?}",g.party_screen.phase()),
                    "start_initializing":g.start_menu.field_initialization_active(),
                    "overworld":pokered_core::snapshot::OverworldSnapshot::capture(&g.overworld)}));
            }
            std::fs::write(dir.join("frames.json"), serde_json::to_string_pretty(&rows).unwrap()).unwrap();
        });
    }

    #[test]
    #[ignore = "original one-step Safari SRAM, actual Continue and walking capture"]
    fn capture_actual_safari_timeout_147() {
        run_link_save_fixture(|| {
            let dir = std::path::PathBuf::from(std::env::var("FIDELITY_SAFARI_CAPTURE").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let save_path = dir.join("fixture.sav");
            std::fs::copy(std::env::var("FIDELITY_SAFARI_SRAM").unwrap(), &save_path).unwrap();
            let mut g = PokemonGame::new_with_options(GameVersion::Red, Some(save_path), None, None,
                false, None, false, true, #[cfg(feature = "debug-server")] None);
            g.audio = Some(AudioOutput::new_pcm());
            g.state.config.language = pokered_core::game_state::Lang::En;
            let idle = InputState::new();
            let mut saw_menu = false;
            for t in 0..2000 {
                saw_menu |= g.state.screen == GameScreen::MainMenu;
                if g.state.screen == GameScreen::Overworld { break; }
                let a = button(GbButton::A);
                g.update(if t % 20 == 19 { &a } else { &idle });
            }
            assert!(saw_menu);
            assert_eq!(g.state.screen, GameScreen::Overworld);
            assert_eq!(g.overworld.state.current_map, MapId::SafariZoneCenter);
            assert_eq!((g.overworld.state.player.x, g.overworld.state.player.y), (14, 25));
            assert!(g.overworld.is_safari_game_active());
            assert_eq!(g.overworld.safari_steps_remaining(), 1);
            assert_eq!(g.overworld.safari_balls_remaining(), 30);
            for _ in 0..120 { g.update(&idle); }
            let mut rows = Vec::new();
            let mut input = InputState::new();
            for t in -1i32..241 {
                if t >= 0 {
                    input.begin_frame();
                    if t == 0 { input.press(GbButton::Up); }
                    if t == 70 { input.release(GbButton::Up); }
                    if t >= 100 && t % 30 == 10 { input.press(GbButton::A); }
                    if t >= 100 && t % 30 == 12 { input.release(GbButton::A); }
                    g.update(&input);
                }
                let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                g.draw(&mut fb);
                fb.save_png(&dir.join(format!("frame-{:04}.png", t + 1))).unwrap();
                rows.push(serde_json::json!({"t":t,"input_bits":input.raw_current(),
                    "screen":format!("{:?}",g.state.screen),
                    "map":format!("{:?}",g.overworld.state.current_map),
                    "x":g.overworld.state.player.x,"y":g.overworld.state.player.y,
                    "steps":g.overworld.safari_steps_remaining(),
                    "balls":g.overworld.safari_balls_remaining(),
                    "active":g.overworld.is_safari_game_active(),
                    "dialogue":g.overworld.pending_dialogue,
                    "overworld":pokered_core::snapshot::OverworldSnapshot::capture(&g.overworld)}));
            }
            std::fs::write(dir.join("frames.json"), serde_json::to_string_pretty(&rows).unwrap()).unwrap();
        });
    }

    #[test]
    #[ignore = "original one-step Safari SRAM, actual Continue and walking capture"]
    fn capture_actual_safari_return_148() {
        run_link_save_fixture(|| {
            let dir = std::path::PathBuf::from(std::env::var("FIDELITY_SAFARI_CAPTURE").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let save_path = dir.join("fixture.sav");
            std::fs::copy(std::env::var("FIDELITY_SAFARI_SRAM").unwrap(), &save_path).unwrap();
            let mut g = PokemonGame::new_with_options(GameVersion::Red, Some(save_path), None, None,
                false, None, false, true, #[cfg(feature = "debug-server")] None);
            g.audio = Some(AudioOutput::new_pcm());
            g.state.config.language = pokered_core::game_state::Lang::En;
            let idle = InputState::new();
            let mut saw_menu = false;
            for t in 0..2000 {
                saw_menu |= g.state.screen == GameScreen::MainMenu;
                if g.state.screen == GameScreen::Overworld { break; }
                let a = button(GbButton::A);
                g.update(if t % 20 == 19 { &a } else { &idle });
            }
            assert!(saw_menu);
            assert_eq!(g.state.screen, GameScreen::Overworld);
            assert_eq!(g.overworld.state.current_map, MapId::SafariZoneCenter);
            assert_eq!((g.overworld.state.player.x, g.overworld.state.player.y), (14, 25));
            assert!(g.overworld.is_safari_game_active());
            assert_eq!(g.overworld.safari_steps_remaining(), 1);
            assert_eq!(g.overworld.safari_balls_remaining(), 30);
            for _ in 0..120 { g.update(&idle); }
            let empty_balls = std::env::var("FIDELITY_SAFARI_EMPTY_BALLS").is_ok();
            if empty_balls { for _ in 0..30 { g.overworld.use_safari_ball(); } }
            let mut rows = Vec::new();
            let mut input = InputState::new();
            for t in -1i32..701 {
                if t >= 0 {
                    input.begin_frame();
                    if t == 0 && !empty_balls { input.press(GbButton::Up); }
                    if t == 70 && !empty_balls { input.release(GbButton::Up); }
                    if t >= 100 && t % 30 == 10 { input.press(GbButton::A); }
                    if t >= 100 && t % 30 == 12 { input.release(GbButton::A); }
                    g.update(&input);
                }
                let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                g.draw(&mut fb);
                fb.save_png(&dir.join(format!("frame-{:04}.png", t + 1))).unwrap();
                rows.push(serde_json::json!({"t":t,"input_bits":input.raw_current(),
                    "screen":format!("{:?}",g.state.screen),
                    "map":format!("{:?}",g.overworld.state.current_map),
                    "x":g.overworld.state.player.x,"y":g.overworld.state.player.y,
                    "facing":format!("{:?}",g.overworld.state.player.facing),
                    "steps":g.overworld.safari_steps_remaining(),
                    "balls":g.overworld.safari_balls_remaining(),
                    "active":g.overworld.is_safari_game_active(),
                    "dialogue":g.overworld.pending_dialogue,
                    "overworld":pokered_core::snapshot::OverworldSnapshot::capture(&g.overworld)}));
            }
            std::fs::write(dir.join("frames.json"), serde_json::to_string_pretty(&rows).unwrap()).unwrap();
        });
    }

    #[test]
    fn eevee_nickname_print_done_opens_choice_without_skip_or_an_extra_press() {
        use pokered_core::overworld::Direction;
        run_link_save_fixture(|| {
            let mut g=fixture(Species::Bulbasaur,3,Direction::Up);
            g.overworld.warp_to_map(MapId::CeladonMansionRoofHouse,4,4);
            let idle=InputState::new();let a=button(GbButton::A);
            for _ in 0..120 {g.update(&idle);}
            g.update(&button(GbButton::Up));for _ in 0..16 {g.update(&idle);}
            for t in 0..1200 {
                g.update(if t<2 {&a} else {&idle});
                if g.overworld.pending_choice.is_some() {break;}
            }
            assert_eq!(g.overworld.pending_choice.as_ref().expect("nickname choice without extra A").options,["YES","NO"]);
            assert!(!g.overworld.is_naming_screen_active());
            assert_eq!(g.save_data.party.count(),1,"gift awaits the nickname answer");
            let (top,bottom)=g.overworld.displayed_field_dialogue().unwrap().get_display_text().unwrap();
            assert!(format!("{top} {bottom}").contains("nickname to EEVEE?"));
            for _ in 0..20 {g.update(&idle);}
            g.update(&button(GbButton::B));
            for _ in 0..180 {g.update(&idle);}
            assert_eq!(g.save_data.party.count(),2);
            assert_eq!(g.save_data.party.get(1).unwrap().species,Species::Eevee);
            assert!(g.overworld.script_flags().get("EVENT_GOT_EEVEE").copied().unwrap_or(false));
            assert!(g.overworld.pending_choice.is_none());
        });
    }

    #[test]
    fn safari_admission_keeps_inner_window_through_money_box_and_refusal() {
        use pokered_core::overworld::Direction;
        use pokered_core::snapshot::OverworldSnapshot;
        run_link_save_fixture(|| {
            let mut g=fixture(Species::Bulbasaur,3,Direction::Up);
            g.overworld.end_safari_game();g.overworld.warp_to_map(MapId::SafariZoneGate,3,3);
            let idle=InputState::new();let a=button(GbButton::A);let b=button(GbButton::B);
            for _ in 0..120 {g.update(&idle);}
            let balance=g.save_data.game_data.player_money;
            for _ in 0..20 {g.update(&button(GbButton::Up));}
            let mut handoff=false;
            for _ in 0..1800 {
                if g.overworld.pending_choice.is_some() {break;}
                let advance=g.overworld.pending_dialogue.as_ref().is_some_and(|d|d.waiting_for_input() && !d.holding_open() &&
                    (d.has_more_pages() || d.get_display_text().is_some_and(|(top,_)|top.starts_with("Welcome"))));
                g.update(if advance {&a} else {&idle});
                if g.overworld.active_script_effect_label().as_deref()==Some("ShowMoneyBox") && g.overworld.pending_dialogue.is_none() {
                    handoff=true;
                    assert!(g.overworld.inner_field_text_open);
                    assert!(g.overworld.displayed_field_dialogue().is_some(),"PrintText does not close before MONEY_BOX");
                    assert!(g.overworld.field_text_restore.is_none(),"no premature CloseTextDisplay sprite reload");
                    assert!(!g.overworld.dialogue_needs_button());
                    let raw=serde_json::to_string(&OverworldSnapshot::capture(&g.overworld)).unwrap();
                    let snap:OverworldSnapshot=serde_json::from_str(&raw).unwrap();snap.restore_into(&mut g.overworld);
                    assert!(g.overworld.inner_field_text_open && g.overworld.displayed_field_dialogue().is_some());
                }
            }
            assert!(handoff && g.overworld.pending_choice.is_some(),"only paragraphs and welcome were acknowledged: {:?} at ({},{})",g.overworld.active_script_effect_label(),g.overworld.state.player.x,g.overworld.state.player.y);
            assert_eq!(g.overworld.script_money_box,Some(balance));
            for _ in 0..20 {g.update(&idle);}
            g.update(&b);
            for _ in 0..1000 {
                let advance=g.overworld.pending_dialogue.as_ref().is_some_and(|d|d.waiting_for_input() && !d.holding_open());
                g.update(if advance {&b} else {&idle});
                if g.overworld.script_engine_idle() && g.overworld.active_script_effect_label().is_none() && g.overworld.pending_dialogue.is_none() {break;}
            }
            assert_eq!(g.save_data.game_data.player_money,balance);
            assert!(!g.overworld.is_safari_game_active());
            assert!(!g.overworld.inner_field_text_open);
            assert!(g.overworld.displayed_field_dialogue().is_none());
            assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),(4,3));
        });
    }

    #[test]
    fn safari_early_exit_question_auto_opens_and_no_preserves_the_existing_hunt() {
        use pokered_core::overworld::Direction;
        run_link_save_fixture(|| {
            let mut g=fixture(Species::Bulbasaur,3,Direction::Down);
            g.overworld.warp_to_map(MapId::SafariZoneCenter,3,3);
            g.overworld.start_safari_game(); // controlled active-hunt setup
            assert!(g.overworld.is_safari_game_active());
            assert_eq!(g.overworld.use_safari_ball(),29); // controlled allowance fixture
            g.overworld.warp_to_map(MapId::SafariZoneGate,4,0);
            let idle=InputState::new();let a=button(GbButton::A);
            for _ in 0..800 {g.update(&idle);if g.overworld.pending_choice.is_some() {break;}}
            assert!(g.overworld.pending_choice.is_some(),"no A is sent for Leaving early?");
            assert_eq!(g.overworld.displayed_field_dialogue().unwrap().get_display_text(),Some(("Leaving early?".into(),"".into())));
            g.update(&button(GbButton::B));
            for _ in 0..1800 {
                let advance=g.overworld.pending_dialogue.as_ref().is_some_and(|d|d.waiting_for_input() && !d.holding_open());
                g.update(if advance {&a} else {&idle});
                if g.overworld.state.current_map==MapId::SafariZoneCenter && g.overworld.active_script_effect_label().is_none() {break;}
            }
            assert_eq!(g.overworld.state.current_map,MapId::SafariZoneCenter);
            assert!(g.overworld.is_safari_game_active());
            assert_eq!(g.overworld.safari_balls_remaining(),29,"continuing must not reset the allowance");
            assert!(!g.overworld.inner_field_text_open);
        });
    }

    #[test]
    fn safari_print_done_enters_choice_without_an_extra_press_and_survives_final_wait_restore() {
        use pokered_core::overworld::Direction;
        use pokered_core::snapshot::OverworldSnapshot;
        run_link_save_fixture(|| {
            let mut g=fixture(Species::Bulbasaur,3,Direction::Left);
            g.state.config.text_speed=pokered_core::game_state::TextSpeed::Medium;
            g.overworld.warp_to_map(MapId::SafariZoneGate,3,4);
            let idle=InputState::new();let a=button(GbButton::A);
            for _ in 0..120 {g.update(&idle);}
            g.update(&button(GbButton::Left));for _ in 0..16 {g.update(&idle);}
            let mut first=None;let mut restored=false;let mut entry=None;
            for t in 0..200 {
                g.update(if t<2 {&a} else {&idle});
                if let Some(d)=&g.overworld.pending_dialogue {
                    if first.is_none() && d.char_index()>0 {first=Some(t);}
                    if d.char_index()==30 && !restored {
                        assert!(!d.waiting_for_input(),"final glyph still owns its delay");
                        let raw=serde_json::to_string(&OverworldSnapshot::capture(&g.overworld)).unwrap();
                        let snap:OverworldSnapshot=serde_json::from_str(&raw).unwrap();
                        snap.restore_into(&mut g.overworld);restored=true;
                    }
                }
                if entry.is_none() && g.overworld.active_script_effect_label().as_deref()==Some("ShowChoice") {entry=Some(t);}
                if g.overworld.pending_choice.is_some() {break;}
            }
            assert!(restored && g.overworld.pending_choice.is_some(),"no third A is sent");
            assert_eq!(entry.unwrap()-first.unwrap(),90,"original first24 -> YesNoChoice114");
            assert!(g.overworld.pending_dialogue.is_none());
            assert_eq!(g.overworld.displayed_field_dialogue().unwrap().get_display_text(),Some(("Hi! Is it your first time".into(),"here?".into())));
            for _ in 0..20 {g.update(&idle);}
            assert_eq!(g.overworld.pending_choice.as_ref().unwrap().selected,0,"opening A must not answer YES");
            g.update(&button(GbButton::B));
            for _ in 0..160 {g.update(&idle);}
            let (top,bottom)=g.overworld.displayed_field_dialogue().unwrap().get_display_text().unwrap();
            assert!(format!("{top} {bottom}").starts_with("Sorry, you're a regular"));
        });
    }

    #[test]
    fn safari_information_choice_keeps_question_across_json_restore_and_replaces_it() {
        use pokered_core::overworld::Direction;
        use pokered_core::snapshot::OverworldSnapshot;
        run_link_save_fixture(|| {
            for (answer, expected) in [(GbButton::B, "Sorry, you're a regular"),
                                      (GbButton::A, "SAFARI ZONE has 4 zones")] {
                let mut g = fixture(Species::Bulbasaur, 3, Direction::Left);
                g.overworld.warp_to_map(MapId::SafariZoneGate, 3, 4);
                let idle = InputState::new();
                let a = button(GbButton::A);
                let b = button(GbButton::B);
                let mut cache = crate::render::OverworldBackgroundCache::new(160,144);
                let mut cached = FrameBuffer::new(dotzuki_engine::render_config::RenderConfig::new(160,144),pokered_renderer::Rgba::WHITE);
                let mut cached_resources = Some(ResourceManager::new(pokered_renderer::resource::AssetRoot::auto_detect().unwrap()));
                let mut compare_draws = |g: &mut PokemonGame| {
                    let mut full = FrameBuffer::new(dotzuki_engine::render_config::RenderConfig::new(160,144),pokered_renderer::Rgba::WHITE);
                    g.draw(&mut full);
                    crate::render::draw_overworld_cached(&mut g.overworld,&mut cached_resources,&mut cached,
                        g.state.config.language,&mut cache);
                    assert_eq!(cached.packed(),full.packed(),"cached/full question handoff");
                };
                for _ in 0..120 { g.update(&idle); }
                compare_draws(&mut g);
                g.update(&button(GbButton::Left));
                for _ in 0..16 { g.update(&idle); }
                for frame in 0..1200 {
                    g.update(if frame % 30 == 0 { &a } else { &idle });
                    compare_draws(&mut g);
                    if g.overworld.pending_choice.is_some() { break; }
                }
                assert!(g.overworld.pending_choice.is_some(), "worker question did not open");
                assert!(g.overworld.pending_dialogue.is_none(), "question must not consume choice input");
                let question = g.overworld.displayed_field_dialogue().unwrap().get_display_text().unwrap();
                assert_eq!(question, ("Hi! Is it your first time".into(), "here?".into()));
                assert!(!g.overworld.dialogue_needs_button(), "question has no text-scroll arrow during choice");
                let snap = OverworldSnapshot::capture(&g.overworld);
                let raw = serde_json::to_string(&snap).unwrap();
                let restored: OverworldSnapshot = serde_json::from_str(&raw).unwrap();
                restored.restore_into(&mut g.overworld);
                assert_eq!(g.overworld.displayed_field_dialogue().unwrap().get_display_text(), Some(question));
                // Earlier JSON fixtures omit the new retained-window state.
                let mut legacy = serde_json::to_value(&snap).unwrap();
                legacy.as_object_mut().unwrap().remove("last_script_dialogue");
                legacy.as_object_mut().unwrap().remove("inner_field_text_open");
                assert!(serde_json::from_value::<OverworldSnapshot>(legacy).unwrap().last_script_dialogue.is_none());
                for _ in 0..20 { g.update(&idle); }
                g.update(&button(answer));
                for _ in 0..200 { g.update(&idle); }
                assert!(g.overworld.pending_choice.is_none());
                let (top, bottom) = g.overworld.displayed_field_dialogue().unwrap().get_display_text().unwrap();
                assert!(format!("{top} {bottom}").starts_with(expected), "answer text: {top} / {bottom}");
                for frame in 0..1800 {
                    g.update(if frame % 30 == 0 { &b } else { &idle });
                    compare_draws(&mut g);
                    if g.overworld.script_engine_idle() && g.overworld.pending_dialogue.is_none() { break; }
                }
                assert!(g.overworld.displayed_field_dialogue().is_none(), "question leaked past completed conversation");
                assert!(g.overworld.last_script_dialogue.is_none());
            }
        });
    }

    fn bill_copycat_fixture_168(map: MapId) -> PokemonGame {
        use pokered_core::overworld::{Direction,NpcMovementType};
        let mut g=fixture(Species::Bulbasaur,4,Direction::Up);
        g.state.config.language=pokered_core::game_state::Lang::En;
        g.save_data.game_data.bag=pokered_core::items::inventory::Inventory::new();
        if map==MapId::CopycatsHouse2F {g.save_data.game_data.bag.add_item(pokered_data::items::ItemId::from_const_name("POKE_DOLL").unwrap(),1).unwrap();}
        g.overworld.warp_to_map(map,4,if map==MapId::BillsHouse {5} else {4});
        let idle=InputState::new();for _ in 0..120 {g.update(&idle);}
        if map==MapId::BillsHouse {
            for flag in ["EVENT_BILL_SAID_USE_CELL_SEPARATOR","EVENT_USED_CELL_SEPARATOR_ON_BILL","EVENT_MET_BILL","EVENT_MET_BILL_2","__OBJ_SHOWN_BILLS_HOUSE_OBJ_2","__OBJ_HIDDEN_BILLS_HOUSE_OBJ_1"] {g.overworld.set_flag_live(flag,true);}
            g.overworld.apply_hidden_object_flags();assert!(g.overworld.npc_states[1].visible);
        }
        for n in &mut g.overworld.npc_states {n.movement_type=NpcMovementType::Stationary;n.x=n.home_x;n.y=n.home_y;n.walk_counter=0;}
        g.update(&button(GbButton::Up));for _ in 0..20 {g.update(&idle);}
        g
    }

    fn aide_fixture_169(map: MapId,owned: u8) -> PokemonGame {
        use pokered_core::overworld::{Direction,NpcMovementType};
        let (x,y)=match map {MapId::Route2Gate=>(1,5),MapId::Route11Gate2F=>(2,7),MapId::Route15Gate2F=>(4,3),_=>panic!("unknown aide fixture")};
        let mut g=fixture(Species::Bulbasaur,x,Direction::Up);g.state.config.language=pokered_core::game_state::Lang::En;
        g.save_data.game_data.bag=pokered_core::items::inventory::Inventory::new();
        g.save_data.game_data.pokedex=pokered_core::pokemon::pokedex::Pokedex::new();
        for n in 1..=owned {g.save_data.game_data.pokedex.set_owned(Species::from_index_id(n));}
        assert_eq!(g.save_data.game_data.pokedex.owned_count(),u32::from(owned));
        g.overworld.warp_to_map(map,x as u8,y);let idle=InputState::new();for _ in 0..120 {g.update(&idle);}
        for n in &mut g.overworld.npc_states {n.movement_type=NpcMovementType::Stationary;n.x=n.home_x;n.y=n.home_y;n.walk_counter=0;}
        g.update(&button(GbButton::Up));for _ in 0..20 {g.update(&idle);}g
    }

    fn gift_fixture_170(map: MapId) -> PokemonGame {
        use pokered_core::overworld::{Direction,NpcMovementType};
        let (x,y)=match map {MapId::SilphCo11F=>(7,6),MapId::CeladonDiner=>(0,2),MapId::Route16FlyHouse=>(2,4),MapId::SafariZoneSecretHouse=>(3,4),MapId::CeladonMart3F=>(16,4),_=>panic!("unknown gift fixture")};
        let mut g=fixture(Species::Bulbasaur,x,Direction::Up);g.state.config.language=pokered_core::game_state::Lang::En;
        g.save_data.game_data.bag=pokered_core::items::inventory::Inventory::new();
        if map==MapId::SilphCo11F {g.overworld.set_flag_live("EVENT_BEAT_SILPH_CO_GIOVANNI",true);g.overworld.set_flag_live("EVENT_SILPH_CO_11_UNLOCKED_DOOR",true);}
        g.overworld.warp_to_map(map,x as u8,y);let idle=InputState::new();for _ in 0..120 {g.update(&idle);}
        for n in &mut g.overworld.npc_states {n.movement_type=NpcMovementType::Stationary;n.x=n.home_x;n.y=n.home_y;n.walk_counter=0;}
        if map==MapId::CeladonMart3F {for _ in 0..20 {g.update(&button(GbButton::Down));}} else {g.update(&button(GbButton::Up));}for _ in 0..20 {g.update(&idle);}
        assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),(x,u16::from(y)));
        assert_eq!(g.overworld.state.player.facing,if map==MapId::CeladonMart3F {Direction::Down} else {Direction::Up});g
    }

    #[test]
    #[ignore = "matched NPC reward interaction and raw frame evidence"]
    fn capture_reward_receipt_166() {
        use pokered_core::overworld::Direction;
        run_link_save_fixture(|| {
            let dir=std::path::PathBuf::from(std::env::var("FIDELITY_RECEIPT_CAPTURE").unwrap());std::fs::create_dir_all(&dir).unwrap();
            let (map,x,y,item,flag)=match std::env::var("FIDELITY_RECEIPT_CASE").unwrap().as_str() {
                "bike" => (MapId::BikeShop,6,4,"BICYCLE","EVENT_GOT_BICYCLE"),
                "masterball" => (MapId::SilphCo11F,7,6,"MASTER_BALL","EVENT_GOT_MASTER_BALL"),
                "coincase" => (MapId::CeladonDiner,0,2,"COIN_CASE","EVENT_GOT_COIN_CASE"),
                "fly" => (MapId::Route16FlyHouse,2,4,"HM02","EVENT_GOT_HM02"),
                "surf" => (MapId::SafariZoneSecretHouse,3,4,"HM03","EVENT_GOT_HM03"),
                "counter" => (MapId::CeladonMart3F,16,4,"TM18","EVENT_GOT_TM18"),
                "aide2" => (MapId::Route2Gate,1,5,"HM05","EVENT_GOT_HM05"),
                "aide11" => (MapId::Route11Gate2F,2,7,"ITEMFINDER","EVENT_GOT_ITEMFINDER"),
                "aide15" => (MapId::Route15Gate2F,4,3,"EXP_ALL","EVENT_GOT_EXP_ALL"),
                "bill" => (MapId::BillsHouse,4,5,"SS_TICKET","EVENT_GOT_SS_TICKET"),
                "copycat" => (MapId::CopycatsHouse2F,4,4,"TM31","EVENT_GOT_TM31"),
                "metronome" => (MapId::CinnabarLabMetronomeRoom,7,3,"TM_35","EVENT_GOT_TM35"),
                "old" => (MapId::VermilionOldRodHouse,2,5,"OLD_ROD","EVENT_GOT_OLD_ROD"),
                "good" => (MapId::FuchsiaGoodRodHouse,5,4,"GOOD_ROD","EVENT_GOT_GOOD_ROD"),
                "super" => (MapId::Route12SuperRodHouse,2,5,"SUPER_ROD","EVENT_GOT_SUPER_ROD"),
                "chairman" => (MapId::PokemonFanClub,3,2,"BIKE_VOUCHER","EVENT_GOT_BIKE_VOUCHER"),
                _ => panic!("unknown fixture"),
            };
            let gift=matches!(map,MapId::BikeShop|MapId::SilphCo11F|MapId::CeladonDiner|MapId::Route16FlyHouse|MapId::SafariZoneSecretHouse|MapId::CeladonMart3F);
            let aide=matches!(map,MapId::Route2Gate|MapId::Route11Gate2F|MapId::Route15Gate2F);
            let mut g=if map==MapId::BikeShop {bike_fixture_171(true)} else if gift {gift_fixture_170(map)} else if aide {aide_fixture_169(map,match map {MapId::Route2Gate=>10,MapId::Route11Gate2F=>30,_=>50})} else if matches!(map,MapId::BillsHouse|MapId::CopycatsHouse2F) {bill_copycat_fixture_168(map)} else {fixture(Species::Bulbasaur,x,Direction::Up)};g.state.config.language=pokered_core::game_state::Lang::En;
            let idle=InputState::new();if !gift && !aide && !matches!(map,MapId::BillsHouse|MapId::CopycatsHouse2F) {g.overworld.warp_to_map(map,x as u8,y);for _ in 0..120 {g.update(&idle);}}
            for n in &mut g.overworld.npc_states {n.movement_type=pokered_core::overworld::NpcMovementType::Stationary;n.x=n.home_x;n.y=n.home_y;n.walk_counter=0;}
            let replay:Option<Vec<Vec<String>>>=std::env::var("FIDELITY_RECEIPT_INPUTS").ok().map(|p|serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap());
            let mut input=InputState::new();let mut rows=Vec::new();let mut controls=Vec::new();let mut sound_started=None;
            for t in 0..replay.as_ref().map_or(4000,Vec::len) {
                let effect=g.overworld.active_script_effect_value().unwrap_or(serde_json::Value::Null);
                let kind=effect["effect"].as_str().unwrap_or("");
                let sound=(kind=="ShowItemDialogue" && effect["sound_started"]==true)
                    || ((kind=="PrintFieldParagraph" || kind=="PrintItemFieldText") && effect["phase"]=="PlayingSound");
                if sound {sound_started.get_or_insert(t);}
                let buttons=if let Some(replay)=&replay {replay[t].clone()} else if t<20 {vec![if map==MapId::CeladonMart3F {"down".to_string()} else {"up".to_string()}]} else if (20..40).contains(&t) {vec!["a".to_string()]} else if let Some(start)=sound_started {
                    if (start+180..start+182).contains(&t) {vec!["a".to_string()]} else {Vec::new()}
                } else {
                    let pages=g.overworld.pending_dialogue.as_ref().is_some_and(|d|d.waiting_for_input() && !d.holding_open() && (kind=="ShowDialogue" || d.has_more_pages()));
                    let wait=(kind=="PrintFieldParagraph" && effect["phase"]=="WaitForButton") || (kind=="WaitFieldPrompt" && effect["protected_remaining"]==0) || kind=="WaitFieldButton";
                    if pages || wait || g.overworld.pending_choice.is_some() {if gift && input.raw_current() & 1 != 0 {Vec::new()} else {vec!["a".to_string()]}} else {Vec::new()}
                };
                input.begin_frame();for (name,button) in [("up",GbButton::Up),("down",GbButton::Down),("a",GbButton::A)] {if buttons.iter().any(|v|v==name) {input.press(button);} else {input.release(button);}}
                g.update(&input);let mut fb=FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);g.draw(&mut fb);fb.save_png(&dir.join(format!("frame-{t:04}.png"))).unwrap();
                let raw=serde_json::to_value(pokered_core::snapshot::OverworldSnapshot::capture(&g.overworld)).unwrap();
                if t==0 {std::fs::write(dir.join("initial-snapshot.json"),serde_json::to_string_pretty(&raw).unwrap()).unwrap();}
                let selected:serde_json::Map<String,serde_json::Value>=["state","player_sprite_state","npc_states","active_script_effect","pending_dialogue","last_script_dialogue","inner_field_text_open","field_text_restore","field_loop_wait"].into_iter().map(|k|(k.into(),raw.get(k).cloned().unwrap_or(serde_json::Value::Null))).collect();
                rows.push(serde_json::json!({"t":t,"input_bits":input.raw_current(),"sfx_playing":g.audio.as_ref().unwrap().is_sfx_playing(),"audio_channels":channels(&g),"has_item":g.save_data.game_data.bag.has_item_const(item),"has_doll":g.save_data.game_data.bag.has_item_const("POKE_DOLL"),"obtained_flag":g.overworld.script_flags().get(flag).copied().unwrap_or(false),"overworld":selected}));controls.push(buttons);
                if replay.is_none() && sound_started.is_some_and(|start|t>=start+260) {break;}
            }
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&rows).unwrap()).unwrap();std::fs::write(dir.join("inputs.json"),serde_json::to_string_pretty(&controls).unwrap()).unwrap();
            assert!(g.save_data.game_data.bag.has_item_const(item),"both branch fixtures must actually receive the reward");
        });
    }

    fn bike_fixture_171(voucher: bool) -> PokemonGame {
        use pokered_core::overworld::{Direction,NpcMovementType};
        let mut g=fixture(Species::Bulbasaur,6,Direction::Up);
        g.state.config.language=pokered_core::game_state::Lang::En;
        g.state.config.text_speed=pokered_core::game_state::TextSpeed::Slow;
        g.save_data.game_data.player_money=999999;
        g.save_data.game_data.bag=pokered_core::items::inventory::Inventory::new();
        if voucher {g.save_data.game_data.bag.add_item(pokered_data::items::ItemId::BikeVoucher,1).unwrap();}
        g.overworld.warp_to_map(MapId::BikeShop,6,4);
        let idle=InputState::new();
        for _ in 0..120 {g.update(&idle);}
        for n in &mut g.overworld.npc_states {n.movement_type=NpcMovementType::Stationary;n.x=n.home_x;n.y=n.home_y;n.walk_counter=0;}
        for _ in 0..20 {g.update(&button(GbButton::Up));}
        for _ in 0..20 {g.update(&idle);}
        assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),(6,4));
        assert_eq!(g.overworld.state.player.facing,Direction::Up);
        g
    }

    fn bike_open_talk_171(g: &mut PokemonGame) {
        let a=button(GbButton::A);
        for _ in 0..60 {g.update(&a);if g.overworld.displayed_field_dialogue().is_some() {break;}}
        assert!(g.overworld.displayed_field_dialogue().is_some(),"actual counter interaction");
        g.update(&InputState::new());
    }

    #[test]
    fn bike_shop_purchase_cancel_and_b_text_mode_survive_real_dialogues() {
        use pokered_core::snapshot::OverworldSnapshot;
        use pokered_core::overworld::script_bridge::ScriptEffect;
        run_link_save_fixture(|| {
            for key in [GbButton::A,GbButton::B,GbButton::Down] {
                let mut g=bike_fixture_171(false);
                let idle=InputState::new();let a=button(GbButton::A);
                let bag=g.save_data.game_data.bag.clone();
                bike_open_talk_171(&mut g);
                let mut saw_prompt=false;
                for _ in 0..6000 {
                    if g.overworld.pending_choice.is_some() {break;}
                    let snap=OverworldSnapshot::capture(&g.overworld);
                    if matches!(snap.active_script_effect,Some(ScriptEffect::WaitFieldPrompt {protected_remaining:0})) {
                        saw_prompt=true;
                        assert!(!g.overworld.text_delay_disabled);
                        for _ in 0..20 {g.update(&idle);assert!(g.overworld.pending_choice.is_none());}
                    }
                    let ack=receipt_prompt_needs_press(&g);
                    g.update(if ack {&a} else {&idle});
                }
                assert!(saw_prompt);
                assert_eq!(g.overworld.pending_choice.as_ref().expect("instant menu").options,["BICYCLE ¥1000000","CANCEL"]);
                assert!(g.overworld.text_delay_disabled);
                assert!(g.overworld.pending_dialogue.is_none());
                let question=g.overworld.displayed_field_dialogue().unwrap().get_display_text().unwrap();
                assert!(format!("{} {}",question.0,question.1).contains("want it?"));
                let raw=serde_json::to_string(&OverworldSnapshot::capture(&g.overworld)).unwrap();
                serde_json::from_str::<OverworldSnapshot>(&raw).unwrap().restore_into(&mut g.overworld);
                assert!(g.overworld.text_delay_disabled);
                for _ in 0..20 {g.update(&idle);}
                if key==GbButton::Down {g.update(&button(GbButton::Down));g.update(&idle);assert_eq!(g.overworld.pending_choice.as_ref().unwrap().selected,1);}
                g.update(&button(if key==GbButton::B {GbButton::B} else {GbButton::A}));
                g.update(&idle);
                assert_eq!(g.overworld.text_delay_disabled,key==GbButton::B);
                let mut saw_cant_afford=false;
                for _ in 0..6000 {
                    if matches!(OverworldSnapshot::capture(&g.overworld).active_script_effect,Some(ScriptEffect::WaitFieldPrompt {..})) {saw_cant_afford=true;}
                    if g.overworld.active_script_effect_label().as_deref()==Some("FinishFieldText") {break;}
                    let ack=receipt_prompt_needs_press(&g);g.update(if ack {&a} else {&idle});
                }
                assert_eq!(saw_cant_afford,key==GbButton::A);
                assert_eq!(g.overworld.active_script_effect_label().as_deref(),Some("FinishFieldText"));
                assert!(g.overworld.displayed_field_dialogue().unwrap().get_display_text().is_some_and(|(a,b)|format!("{a} {b}").contains("Come back again")));
                for _ in 0..20 {g.update(&idle);}
                for _ in 0..8 {g.update(&a);assert!(g.overworld.displayed_field_dialogue().is_some());}
                for _ in 0..40 {g.update(&idle);}
                assert!(g.overworld.displayed_field_dialogue().is_none());
                assert_eq!(g.save_data.game_data.bag,bag);
                assert_eq!(g.save_data.game_data.player_money,999999);
                assert!(!g.overworld.script_flags().get("EVENT_GOT_BICYCLE").copied().unwrap_or(false));
                assert_eq!(g.overworld.text_delay_disabled,key==GbButton::B);
                let snap=OverworldSnapshot::capture(&g.overworld);
                let mut legacy=serde_json::to_value(&snap).unwrap();legacy.as_object_mut().unwrap().remove("text_delay_disabled");
                assert!(!serde_json::from_value::<OverworldSnapshot>(legacy).unwrap().text_delay_disabled);
                serde_json::from_str::<OverworldSnapshot>(&serde_json::to_string(&snap).unwrap()).unwrap().restore_into(&mut g.overworld);
                bike_open_talk_171(&mut g);
                let d=g.overworld.displayed_field_dialogue().unwrap();
                assert_eq!(d.waiting_for_input(),key==GbButton::B,"B carry changes next conversation; A uses slow letters");
            }
        });
    }

    #[test]
    fn bike_voucher_exchange_preserves_original_capacity_flag_and_sound_order() {
        use pokered_core::snapshot::OverworldSnapshot;
        use pokered_core::overworld::script_bridge::{ScriptEffect,FieldParagraphPhase};
        use pokered_data::items::ItemId;
        run_link_save_fixture(|| {
            for full in [false,true] {
                let mut g=bike_fixture_171(true);
                if full {
                    for id in 1..=255 {let item=ItemId::from_id(id);if matches!(item,ItemId::Bicycle|ItemId::BikeVoucher) {continue;}g.save_data.game_data.bag.add_item(item,1).unwrap();if g.save_data.game_data.bag.is_full() {break;}}
                }
                let original_bag=g.save_data.game_data.bag.clone();let idle=InputState::new();let a=button(GbButton::A);
                bike_open_talk_171(&mut g);
                for _ in 0..6000 {
                    if matches!(OverworldSnapshot::capture(&g.overworld).active_script_effect,Some(ScriptEffect::WaitFieldPrompt {protected_remaining:0})) {break;}
                    let ack=receipt_prompt_needs_press(&g);g.update(if ack {&a} else {&idle});
                }
                assert!(matches!(OverworldSnapshot::capture(&g.overworld).active_script_effect,Some(ScriptEffect::WaitFieldPrompt {protected_remaining:0})));
                for _ in 0..40 {g.update(&idle);assert_eq!(g.save_data.game_data.bag,original_bag);}
                assert!(!g.overworld.script_flags().get("EVENT_GOT_BICYCLE").copied().unwrap_or(false));
                g.update(&a);g.update(&idle);
                let mut start=None;let mut end=None;
                for t in 0..6000 {
                    let snap=OverworldSnapshot::capture(&g.overworld);
                    if matches!(snap.active_script_effect,Some(ScriptEffect::PrintItemFieldText {phase:FieldParagraphPhase::PlayingSound,..})) {
                        assert!(!full);
                        assert!(g.save_data.game_data.bag.has_item_const("BICYCLE"));
                        assert!(!g.save_data.game_data.bag.has_item_const("BIKE_VOUCHER"));
                        assert!(g.overworld.script_flags().get("EVENT_GOT_BICYCLE").copied().unwrap_or(false));
                        if start.is_none() {start=Some(t);serde_json::from_str::<OverworldSnapshot>(&serde_json::to_string(&snap).unwrap()).unwrap().restore_into(&mut g.overworld);}
                        let mut keys=InputState::new();if start.is_some_and(|n|t==n+1) {keys.press(GbButton::B);keys.press(GbButton::Down);}
                        g.update(&keys);assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),(6,4));
                    } else if start.is_some() {end=Some(t);break;} else {
                        if g.overworld.active_script_effect_label().as_deref()==Some("FinishFieldText") {break;}
                        let ack=receipt_prompt_needs_press(&g);g.update(if ack {&a} else {&idle});
                    }
                }
                assert_eq!(start.is_some(),!full);
                if !full {
                    let expected=AudioOutput::new_pcm();expected.play_sfx(SfxId::GetKeyItem);let mut duration=0;
                    while expected.is_sfx_playing()&&duration<1000 {expected.update_frame();duration+=1;}
                    assert_eq!(end.unwrap()-start.unwrap(),duration,"snapshot does not duplicate or shorten key-item fanfare");
                }
                for _ in 0..60 {g.update(&idle);}
                assert_eq!(g.overworld.active_script_effect_label().as_deref(),Some("FinishFieldText"));
                assert_eq!(g.overworld.script_flags().get("EVENT_GOT_BICYCLE").copied().unwrap_or(false),!full);
                let text=g.overworld.displayed_field_dialogue().unwrap().get_display_text().unwrap();
                assert!(format!("{} {}",text.0,text.1).contains(if full {"make room"} else {"exchanged"}));
                for _ in 0..8 {g.update(&a);assert!(g.overworld.displayed_field_dialogue().is_some());}
                for _ in 0..40 {g.update(&idle);}
                assert!(g.overworld.displayed_field_dialogue().is_none());
                if full {assert_eq!(g.save_data.game_data.bag,original_bag);} else {
                    let bag=g.save_data.game_data.bag.clone();bike_open_talk_171(&mut g);
                    let mut explanation=false;
                    for _ in 0..6000 {
                        assert!(!matches!(OverworldSnapshot::capture(&g.overworld).active_script_effect,Some(ScriptEffect::PrintItemFieldText {..})));
                        explanation|=g.overworld.displayed_field_dialogue().is_some_and(|d|d.get_display_text().is_some_and(|(a,b)|format!("{a} {b}").contains("CYCLING")));
                        if g.overworld.active_script_effect_label().as_deref()==Some("FinishFieldText") {break;}
                        let ack=receipt_prompt_needs_press(&g);g.update(if ack {&a} else {&idle});
                    }
                    assert!(explanation);assert_eq!(g.save_data.game_data.bag,bag);
                }
            }
        });
    }

    #[test]
    #[ignore = "deterministic before/after Bike Shop capture"]
    fn capture_bike_shopping_171() {
        run_link_save_fixture(|| {
            let dir=std::path::PathBuf::from(std::env::var("FIDELITY_BIKE_CAPTURE").unwrap());std::fs::create_dir_all(&dir).unwrap();
            let mode=std::env::var("FIDELITY_BIKE_MODE").unwrap();assert!(["purchase","cancel_a","cancel_b"].contains(&mode.as_str()));
            let mut g=bike_fixture_171(false);let mut input=InputState::new();let mut rows=Vec::new();let mut menu=None;let mut close=None;let mut second=None;
            for t in 0..6000 {
                if menu.is_none() && g.overworld.pending_choice.is_some() {menu=Some(t);}
                if menu.is_some() && close.is_none() && g.overworld.script_engine_idle() && g.overworld.active_script_effect_label().is_none() && g.overworld.displayed_field_dialogue().is_none() {close=Some(t);}
                if close.is_some_and(|n|t>=n+60) && second.is_none() && g.overworld.displayed_field_dialogue().is_some() {second=Some(t);}
                let keys=if t<20 {vec![GbButton::A]} else if close.is_some_and(|n|(n+60..n+80).contains(&t)) {vec![GbButton::A]} else if close.is_some() {Vec::new()} else if let Some(cue)=menu {
                    if (cue+40..cue+42).contains(&t) {vec![match mode.as_str() {"cancel_b"=>GbButton::B,"cancel_a"=>GbButton::Down,_=>GbButton::A}]} else if mode=="cancel_a" && (cue+60..cue+62).contains(&t) {vec![GbButton::A]} else if t>=cue+70 && receipt_prompt_needs_press(&g) && input.raw_current()&1==0 {vec![GbButton::A]} else {Vec::new()}
                } else if receipt_prompt_needs_press(&g) && input.raw_current()&1==0 {vec![GbButton::A]} else {Vec::new()};
                input.begin_frame();for key in [GbButton::A,GbButton::B,GbButton::Down] {if keys.contains(&key) {input.press(key);}else {input.release(key);}}
                g.update(&input);let mut fb=FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);g.draw(&mut fb);fb.save_png(&dir.join(format!("frame-{t:04}.png"))).unwrap();
                let raw=serde_json::to_value(pokered_core::snapshot::OverworldSnapshot::capture(&g.overworld)).unwrap();
                let selected:serde_json::Map<String,serde_json::Value>=["state","active_script_effect","pending_dialogue","last_script_dialogue","inner_field_text_open","text_delay_disabled","pending_choice","field_text_restore","field_loop_wait"].into_iter().map(|k|(k.into(),raw.get(k).cloned().unwrap_or(serde_json::Value::Null))).collect();
                rows.push(serde_json::json!({"t":t,"input_bits":input.raw_current(),"menu_cue":menu,"close_cue":close,"second_talk":second,"money":g.save_data.game_data.player_money,"overworld":selected}));
                if second.is_some_and(|n|t>=n+45) {break;}
            }
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&rows).unwrap()).unwrap();
            assert!(menu.is_some()&&close.is_some()&&second.is_some(),"all phases recorded: menu={menu:?} close={close:?} second={second:?}");
            assert_eq!(g.save_data.game_data.player_money,999999);assert!(!g.save_data.game_data.bag.has_item_const("BICYCLE"));
        });
    }

    #[test]
    fn player_pc_exit_clears_bike_b_cancel_text_mode() {
        use pokered_core::pc_screen::PcPhase;
        run_link_save_fixture(|| {
            for player_pc in [true,false] {
            let mut g=bike_fixture_171(false);let idle=InputState::new();let a=button(GbButton::A);let b=button(GbButton::B);
            bike_open_talk_171(&mut g);
            for _ in 0..6000 {if g.overworld.pending_choice.is_some() {break;}let ack=receipt_prompt_needs_press(&g);g.update(if ack {&a} else {&idle});}
            assert!(g.overworld.pending_choice.is_some());
            for _ in 0..20 {g.update(&idle);}
            g.update(&b);g.update(&idle);
            for _ in 0..200 {if g.overworld.active_script_effect_label().as_deref()==Some("FinishFieldText") {break;}g.update(&idle);}
            assert_eq!(g.overworld.active_script_effect_label().as_deref(),Some("FinishFieldText"));
            for _ in 0..8 {g.update(&a);}for _ in 0..40 {g.update(&idle);}
            assert!(g.overworld.displayed_field_dialogue().is_none());assert!(g.overworld.text_delay_disabled);
            let (map,x,y)=if player_pc {(MapId::RedsHouse2F,0,2)} else {(MapId::ViridianPokecenter,13,4)};
            g.overworld.warp_to_map(map,x,y);
            for _ in 0..120 {g.update(&idle);}for _ in 0..20 {g.update(&button(GbButton::Up));}for _ in 0..20 {g.update(&idle);}
            assert!(g.overworld.text_delay_disabled,"warp preserves actual original B carry");
            for _ in 0..60 {g.update(&a);if g.state.screen==GameScreen::PC {break;}}
            assert_eq!(g.state.screen,GameScreen::PC,"bedroom hidden PC actual interaction");g.update(&idle);
            let target=if player_pc {PcPhase::ItemMenu} else {PcPhase::MainMenu};
            for _ in 0..200 {if g.pc_screen.as_ref().unwrap().phase()==target {break;}g.update(&a);g.update(&idle);}
            assert_eq!(g.pc_screen.as_ref().unwrap().phase(),target);
            g.update(&b);for _ in 0..120 {g.update(&idle);if g.state.screen==GameScreen::Overworld && g.overworld.script_engine_idle() {break;}}
            assert_eq!(g.state.screen,GameScreen::Overworld);assert!(g.pc_screen.is_none());
            assert_eq!(g.overworld.text_delay_disabled,!player_pc,"ExitPlayerPC clears; generic PC LogOff preserves original NO_TEXT_DELAY");
            }
        });
    }

    #[test]
    #[ignore = "actual generic PC opening glyph evidence"]
    fn capture_pc_opening_173() {
        run_link_save_fixture(|| {
            let dir=std::path::PathBuf::from(std::env::var("FIDELITY_PC_CAPTURE").unwrap());std::fs::create_dir_all(&dir).unwrap();
            let mut g=bike_fixture_171(false);let idle=InputState::new();let a=button(GbButton::A);
            g.overworld.warp_to_map(MapId::ViridianPokecenter,13,4);
            for _ in 0..120 {g.update(&idle);}for _ in 0..20 {g.update(&button(GbButton::Up));}for _ in 0..20 {g.update(&idle);}
            assert!(!g.overworld.text_delay_disabled);
            let mut rows=Vec::new();let mut cue=None;
            for t in 0..300 {
                let input=if cue.is_none() {&a} else {&idle};g.update(input);
                if cue.is_none()&&g.state.screen==GameScreen::PC {cue=Some(t);}
                let mut fb=FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);g.draw(&mut fb);fb.save_png(&dir.join(format!("frame-{t:04}.png"))).unwrap();
                let pc=g.pc_screen.as_ref();rows.push(serde_json::json!({"t":t,"input":if cue.is_some_and(|n|t>n) {"idle"} else {"a"},"pc_entry":cue,"field_text_delay_disabled":g.overworld.text_delay_disabled,"phase":pc.map(|v|format!("{:?}",v.phase())),"message":pc.map(|v|v.message_lines()),"message_page":pc.map(|v|v.message_page()),"visible_chars":pc.map(|v|v.message_visible_chars()),"visible_page_lines":pc.map(|v|v.message_page_lines())}));
                if cue.is_some_and(|n|t>=n+150) {break;}
            }
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&rows).unwrap()).unwrap();assert!(cue.is_some());
        });
    }

    fn receipt_prompt_needs_press(g: &PokemonGame) -> bool {
        use pokered_core::overworld::script_bridge::{ScriptEffect,FieldParagraphPhase};
        match pokered_core::snapshot::OverworldSnapshot::capture(&g.overworld).active_script_effect {
            Some(ScriptEffect::WaitFieldPrompt {protected_remaining:0})
            | Some(ScriptEffect::WaitFieldButton {..})
            | Some(ScriptEffect::FinishFieldText {acknowledged:false})
            | Some(ScriptEffect::PrintFieldParagraph {phase:FieldParagraphPhase::WaitForButton,..}) => true,
            Some(ScriptEffect::ShowDialogue {..}) => g.overworld.pending_dialogue.as_ref().is_some_and(|d|d.waiting_for_input() && !d.holding_open()),
            Some(ScriptEffect::PrintFieldText {..})
            | Some(ScriptEffect::ShowItemDialogue {..})
            | Some(ScriptEffect::PrintItemFieldText {phase:FieldParagraphPhase::Printing,..})
            | Some(ScriptEffect::PrintFieldParagraph {phase:FieldParagraphPhase::Printing,..}) => g.overworld.pending_dialogue.as_ref().is_some_and(|d|d.waiting_for_input() && d.has_more_pages()),
            _ => false,
        }
    }

    #[test]
    fn full_bags_do_not_start_a_receipt_sound_or_set_reward_flags() {
        use pokered_core::overworld::Direction;
        use pokered_data::items::ItemId;
        run_link_save_fixture(|| {
            for (map,x,y,item,flag) in [
                (MapId::VermilionOldRodHouse,2,5,"OLD_ROD","EVENT_GOT_OLD_ROD"),
                (MapId::FuchsiaGoodRodHouse,5,4,"GOOD_ROD","EVENT_GOT_GOOD_ROD"),
                (MapId::Route12SuperRodHouse,2,5,"SUPER_ROD","EVENT_GOT_SUPER_ROD"),
                (MapId::PokemonFanClub,3,2,"BIKE_VOUCHER","EVENT_GOT_BIKE_VOUCHER"),
            ] {
                let mut g=fixture(Species::Bulbasaur,x,Direction::Up);
                g.state.config.language=pokered_core::game_state::Lang::En;
                g.save_data.game_data.bag=pokered_core::items::inventory::Inventory::new();
                for id in 1..=255 {
                    let id=ItemId::from_id(id);
                    if matches!(id,ItemId::Bicycle|ItemId::BikeVoucher|ItemId::OldRod|ItemId::GoodRod|ItemId::SuperRod) {continue;}
                    g.save_data.game_data.bag.add_item(id,1).unwrap();
                    if g.save_data.game_data.bag.is_full() {break;}
                }
                assert!(g.save_data.game_data.bag.is_full());
                let before=g.save_data.game_data.bag.clone();
                g.overworld.warp_to_map(map,x as u8,y);let idle=InputState::new();let a=button(GbButton::A);
                for _ in 0..120 {g.update(&idle);}
                g.update(&button(GbButton::Up));for _ in 0..20 {g.update(&idle);}
                let mut saw_choice=false;let mut saw_refusal=false;
                for t in 0..6000 {
                    let effect=g.overworld.active_script_effect_value().unwrap_or(serde_json::Value::Null);
                    assert!(effect["phase"]!="PlayingSound", "{map:?}: no receipt fanfare with full bag");
                    if let Some(d)=g.overworld.displayed_field_dialogue() {
                        saw_refusal|=d.get_display_text().is_some_and(|(a,b)|format!("{a} {b}").contains(if map==MapId::PokemonFanClub {"Make room"} else {"no room"}));
                    }
                    saw_choice|=g.overworld.pending_choice.is_some();
                    let ack=receipt_prompt_needs_press(&g)||g.overworld.pending_choice.is_some();
                    g.update(if t==0||ack {&a} else {&idle});
                    if saw_choice && g.overworld.script_engine_idle() && g.overworld.active_script_effect_label().is_none() {break;}
                }
                assert!(saw_choice && saw_refusal,"{map:?}: actual full-bag refusal");
                assert!(!g.save_data.game_data.bag.has_item_const(item));
                assert!(!g.overworld.script_flags().get(flag).copied().unwrap_or(false));
                assert_eq!(g.save_data.game_data.bag,before);
                assert!(g.overworld.displayed_field_dialogue().is_none());
            }
        });
    }

    #[test]
    fn five_remaining_gifts_preserve_source_prompt_sound_flags_and_outer_confirmation() {
        use pokered_core::overworld::script_bridge::{ScriptEffect,FieldParagraphPhase};
        use pokered_core::snapshot::OverworldSnapshot;
        use pokered_data::items::ItemId;
        run_link_save_fixture(|| {
            for (map,item,flag,sfx,late,paragraphs) in [(MapId::SilphCo11F,"MASTER_BALL","EVENT_GOT_MASTER_BALL",SfxId::GetKeyItem,true,4),(MapId::CeladonDiner,"COIN_CASE","EVENT_GOT_COIN_CASE",SfxId::GetKeyItem,false,3),(MapId::Route16FlyHouse,"HM02","EVENT_GOT_HM02",SfxId::GetKeyItem,false,1),(MapId::SafariZoneSecretHouse,"HM03","EVENT_GOT_HM03",SfxId::GetItem1,true,3),(MapId::CeladonMart3F,"TM18","EVENT_GOT_TM18",SfxId::GetItem1,false,1)] {
                for full in [false,true] {
                    let mut g=gift_fixture_170(map);
                    if full {for id in 1..=255 {let id=ItemId::from_id(id);if id==ItemId::from_const_name(item).unwrap() {continue;}g.save_data.game_data.bag.add_item(id,1).unwrap();if g.save_data.game_data.bag.is_full() {break;}}assert!(g.save_data.game_data.bag.is_full());}
                    let bag=g.save_data.game_data.bag.clone();let idle=InputState::new();let a=button(GbButton::A);let pos=(g.overworld.state.player.x,g.overworld.state.player.y);let mut prompt=false;let mut clears=0;let mut start=None;let mut end=None;
                    for _ in 0..60 {g.update(&a);if g.overworld.active_script_effect_label().as_deref()==Some("PrintFieldText") {break;}}g.update(&idle);
                    for t in 0..6000 {
                        let snap=OverworldSnapshot::capture(&g.overworld);
                        if start.is_some() && end.is_none() && !matches!(snap.active_script_effect,Some(ScriptEffect::PrintItemFieldText {phase:FieldParagraphPhase::PlayingSound,..})) {end=Some(t);}
                        match snap.active_script_effect {
                            Some(ScriptEffect::WaitFieldPrompt {protected_remaining:0}) => {
                                assert!(!prompt);prompt=true;assert_eq!(clears,paragraphs,"{map:?} intro paragraphs");assert_eq!(g.save_data.game_data.bag,bag);assert!(!g.overworld.script_flags().get(flag).copied().unwrap_or(false));
                                for _ in 0..40 {g.update(&idle);assert_eq!(g.save_data.game_data.bag,bag);}
                                for _ in 0..8 {g.update(&a);}g.update(&idle);
                            }
                            Some(ScriptEffect::PrintFieldParagraph {phase:FieldParagraphPhase::BlankDelay {remaining:20},..}) => {clears+=1;for _ in 0..10 {g.update(&idle);}assert!(matches!(OverworldSnapshot::capture(&g.overworld).active_script_effect,Some(ScriptEffect::PrintFieldParagraph {phase:FieldParagraphPhase::BlankDelay {remaining:10},..})));}
                            Some(ScriptEffect::PrintItemFieldText {phase:FieldParagraphPhase::PlayingSound,..}) => {
                                assert!(!full);assert!(prompt);assert!(g.save_data.game_data.bag.has_item_const(item));assert_eq!(g.overworld.script_flags().get(flag).copied().unwrap_or(false),!late,"{map:?} flag during sound");
                                if start.is_none() {start=Some(t);let raw=serde_json::to_string(&snap).unwrap();serde_json::from_str::<OverworldSnapshot>(&raw).unwrap().restore_into(&mut g.overworld);}
                                let mut keys=InputState::new();if start.is_some_and(|s|t==s+1) {keys.press(GbButton::B);keys.press(GbButton::Down);}g.update(&keys);assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),pos);
                            }
                            Some(ScriptEffect::FinishFieldText {acknowledged:false}) => {break;}
                            _ => {let ack=receipt_prompt_needs_press(&g);g.update(if ack {&a} else {&idle});}
                        }
                    }
                    assert!(prompt,"{map:?} full={full}: actual NPC must reach PROMPT");assert_eq!(start.is_some(),!full);
                    if full {assert_eq!(g.save_data.game_data.bag,bag);assert!(!g.overworld.script_flags().get(flag).copied().unwrap_or(false));assert!(!g.audio.as_ref().unwrap().is_sfx_playing());assert!(g.overworld.displayed_field_dialogue().unwrap().get_display_text().is_some_and(|(a,b)|{let s=format!("{a} {b}");s.contains("room")||s.contains("full")}));}
                    else {let expected=AudioOutput::new_pcm();expected.play_sfx(sfx);let mut duration=0;while expected.is_sfx_playing()&&duration<1000 {expected.update_frame();duration+=1;}assert_eq!(end.unwrap()-start.unwrap(),duration,"{map:?} complete sound");assert!(g.overworld.script_flags().get(flag).copied().unwrap_or(false));}
                    assert!(matches!(OverworldSnapshot::capture(&g.overworld).active_script_effect,Some(ScriptEffect::FinishFieldText {acknowledged:false})));
                    for _ in 0..40 {g.update(&idle);assert!(g.overworld.displayed_field_dialogue().is_some());}for _ in 0..8 {g.update(&a);assert!(g.overworld.displayed_field_dialogue().is_some());}for _ in 0..40 {g.update(&idle);}assert!(g.overworld.displayed_field_dialogue().is_none());
                    if !full {let bag=g.save_data.game_data.bag.clone();for _ in 0..60 {g.update(&a);if g.overworld.active_script_effect_label().as_deref()==Some("PrintFieldText") {break;}}g.update(&idle);let mut seen=false;for _t in 0..6000 {assert!(g.overworld.pending_choice.is_none());assert!(!matches!(OverworldSnapshot::capture(&g.overworld).active_script_effect,Some(ScriptEffect::PrintItemFieldText {..})));seen|=g.overworld.displayed_field_dialogue().is_some();let ack=receipt_prompt_needs_press(&g);g.update(if ack {&a} else {&idle});if seen&&g.overworld.script_engine_idle()&&g.overworld.active_script_effect_label().is_none() {break;}}assert!(seen);assert_eq!(g.save_data.game_data.bag,bag);assert!(g.overworld.displayed_field_dialogue().is_none());}
                }
            }
        });
    }

    #[test]
    fn oaks_aides_keep_original_question_promise_reward_and_refusal_order() {
        use pokered_core::overworld::script_bridge::{ScriptEffect,FieldParagraphPhase};
        use pokered_core::snapshot::OverworldSnapshot;
        use pokered_data::items::ItemId;
        run_link_save_fixture(|| {
            for (map,requirement,item,flag) in [(MapId::Route2Gate,10,"HM05","EVENT_GOT_HM05"),(MapId::Route11Gate2F,30,"ITEMFINDER","EVENT_GOT_ITEMFINDER"),(MapId::Route15Gate2F,50,"EXP_ALL","EVENT_GOT_EXP_ALL")] {
                for mode in ["success","full","low","no"] {
                    let mut g=aide_fixture_169(map,if mode=="low" {requirement-1} else {requirement});
                    if mode=="full" {
                        for id in 1..=255 {
                            let id=ItemId::from_id(id);if id==ItemId::from_const_name(item).unwrap() {continue;}
                            g.save_data.game_data.bag.add_item(id,1).unwrap();if g.save_data.game_data.bag.is_full() {break;}
                        }
                        assert!(g.save_data.game_data.bag.is_full());
                    }
                    let bag=g.save_data.game_data.bag.clone();let idle=InputState::new();let a=button(GbButton::A);
                    for t in 0..4000 {
                        if g.overworld.pending_choice.is_some() {break;}
                        let ack=receipt_prompt_needs_press(&g);g.update(if t==0||ack {&a} else {&idle});
                    }
                    assert_eq!(g.overworld.pending_choice.as_ref().expect("last question DONE must auto-return").options,["YES","NO"]);
                    assert!(g.overworld.displayed_field_dialogue().unwrap().get_display_text().is_some_and(|(a,b)|format!("{a} {b}").contains("POKeMON?")));
                    for _ in 0..30 {g.update(&idle);}
                    g.update(&button(if mode=="no" {GbButton::B} else {GbButton::A}));g.update(&idle);
                    let mut promise=false;let mut sound_start=None;let mut sound_end=None;let pos=(g.overworld.state.player.x,g.overworld.state.player.y);let mut refusal=false;
                    for t in 0..5000 {
                        let snap=OverworldSnapshot::capture(&g.overworld);
                        if matches!(snap.active_script_effect,Some(ScriptEffect::WaitFieldPrompt {protected_remaining:0})) {
                            assert!(mode=="success"||mode=="full");promise=true;
                            assert!(!g.save_data.game_data.bag.has_item_const(item));assert!(!g.overworld.script_flags().get(flag).copied().unwrap_or(false));
                            assert!(g.overworld.displayed_field_dialogue().unwrap().get_display_text().is_some_and(|(a,b)|format!("{a} {b}").contains("Here you go")));
                            for _ in 0..60 {g.update(&idle);assert!(!g.save_data.game_data.bag.has_item_const(item));}
                            for _ in 0..8 {g.update(&a);}g.update(&idle);
                        } else if matches!(snap.active_script_effect,Some(ScriptEffect::PrintItemFieldText {phase:FieldParagraphPhase::PlayingSound,..})) {
                            assert_eq!(mode,"success");assert!(promise);assert!(g.save_data.game_data.bag.has_item_const(item));assert!(!g.overworld.script_flags().get(flag).copied().unwrap_or(false));
                            if sound_start.is_none() {sound_start=Some(t);let raw=serde_json::to_string(&snap).unwrap();serde_json::from_str::<OverworldSnapshot>(&raw).unwrap().restore_into(&mut g.overworld);}
                            let mut keys=InputState::new();if sound_start.is_some_and(|start|t==start+1) {keys.press(GbButton::B);keys.press(GbButton::Down);}
                            g.update(&keys);assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),pos);
                        } else if sound_start.is_some() {sound_end=Some(t);break;} else {
                            if let Some(d)=g.overworld.displayed_field_dialogue() {refusal|=d.get_display_text().is_some_and(|(a,b)|format!("{a} {b}").contains(match mode {"full"=>"room", "low"=>"You need", _=>"When you get"}));}
                            if g.overworld.active_script_effect_label().as_deref()==Some("FinishFieldText") {break;}
                            let ack=receipt_prompt_needs_press(&g);g.update(if ack {&a} else {&idle});
                        }
                    }
                    assert_eq!(promise,mode=="success"||mode=="full");assert_eq!(sound_start.is_some(),mode=="success");
                    if mode=="success" {
                        let expected=AudioOutput::new_pcm();expected.play_sfx(SfxId::GetItem1);let mut duration=0;while expected.is_sfx_playing()&&duration<1000 {expected.update_frame();duration+=1;}
                        assert_eq!(sound_end.unwrap()-sound_start.unwrap(),duration);
                        // No confirmation between sound and description PrintText.
                        for _ in 0..30 {g.update(&idle);}
                        assert!(g.overworld.script_flags().get(flag).copied().unwrap_or(false));
                        assert!(matches!(OverworldSnapshot::capture(&g.overworld).active_script_effect,Some(ScriptEffect::PrintFieldText {..})));
                        assert!(g.overworld.field_text_restore.is_none());
                        for _ in 0..4000 {if g.overworld.active_script_effect_label().as_deref()==Some("FinishFieldText") {break;}let ack=receipt_prompt_needs_press(&g);g.update(if ack {&a} else {&idle});}
                    } else {assert!(refusal,"{map:?} {mode}: refusal must be visible");assert_eq!(g.save_data.game_data.bag,bag);assert!(!g.overworld.script_flags().get(flag).copied().unwrap_or(false));}
                    assert_eq!(g.overworld.active_script_effect_label().as_deref(),Some("FinishFieldText"));
                    for _ in 0..40 {g.update(&idle);assert!(g.overworld.displayed_field_dialogue().is_some());}
                    for _ in 0..8 {g.update(&a);assert!(g.overworld.displayed_field_dialogue().is_some());}
                    for _ in 0..40 {g.update(&idle);}assert!(g.overworld.displayed_field_dialogue().is_none());
                    if mode=="success" {
                        let bag=g.save_data.game_data.bag.clone();let mut seen=false;
                        for t in 0..4000 {
                            assert!(g.overworld.pending_choice.is_none());assert!(!matches!(OverworldSnapshot::capture(&g.overworld).active_script_effect,Some(ScriptEffect::PrintItemFieldText {..})));
                            seen|=g.overworld.displayed_field_dialogue().is_some();let ack=receipt_prompt_needs_press(&g);g.update(if t==0||ack {&a} else {&idle});
                            if seen&&g.overworld.script_engine_idle()&&g.overworld.active_script_effect_label().is_none() {break;}
                        }
                        assert!(seen);assert_eq!(g.save_data.game_data.bag,bag);assert!(g.overworld.displayed_field_dialogue().is_none());
                    }
                }
            }
        });
    }

    #[test]
    fn bill_and_copycat_rewards_keep_original_wait_and_consumption_order() {
        use pokered_core::overworld::{script_bridge::{ScriptEffect,FieldParagraphPhase}};
        use pokered_core::snapshot::OverworldSnapshot;
        run_link_save_fixture(|| {
            for (map,item,flag) in [(MapId::BillsHouse,"SS_TICKET","EVENT_GOT_SS_TICKET"),(MapId::CopycatsHouse2F,"TM31","EVENT_GOT_TM31")] {
                let mut g=bill_copycat_fixture_168(map);let idle=InputState::new();let a=button(GbButton::A);
                let mut sound_start=None;let mut sound_end=None;
                for t in 0..6000 {
                    let snap=OverworldSnapshot::capture(&g.overworld);
                    assert!(!g.overworld.script_flags().get(flag).copied().unwrap_or(false));
                    if map==MapId::CopycatsHouse2F {assert!(g.save_data.game_data.bag.has_item_const("POKE_DOLL"));}
                    if matches!(snap.active_script_effect,Some(ScriptEffect::PrintItemFieldText {phase:FieldParagraphPhase::PlayingSound,..})) {
                        assert!(g.save_data.game_data.bag.has_item_const(item));
                        if sound_start.is_none() {
                            sound_start=Some(t);let raw=serde_json::to_string(&snap).unwrap();
                            serde_json::from_str::<OverworldSnapshot>(&raw).unwrap().restore_into(&mut g.overworld);
                        }
                        let mut keys=InputState::new();if sound_start.is_some_and(|start|t==start+1) {keys.press(GbButton::B);keys.press(GbButton::Down);}
                        g.update(&keys);assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),(4,if map==MapId::BillsHouse {5} else {4}));
                    } else if sound_start.is_some() {sound_end=Some(t);break;} else {
                        let ack=receipt_prompt_needs_press(&g);g.update(if t==0||ack {&a} else {&idle});
                    }
                }
                assert!(sound_start.is_some() && sound_end.is_some(),"{map:?}: actual receipt");
                let expected=AudioOutput::new_pcm();expected.play_sfx(if map==MapId::BillsHouse {SfxId::GetKeyItem} else {SfxId::GetItem1});
                let mut duration=0;while expected.is_sfx_playing() && duration<1000 {expected.update_frame();duration+=1;}
                assert_eq!(sound_end.unwrap()-sound_start.unwrap(),duration);
                for _ in 0..60 {g.update(&idle);}
                assert!(!g.overworld.script_flags().get(flag).copied().unwrap_or(false));
                assert!(g.overworld.displayed_field_dialogue().unwrap().get_display_text().is_some_and(|(a,b)|format!("{a} {b}").contains("received")));
                assert!(g.overworld.dialogue_needs_button());
                if map==MapId::BillsHouse {
                    assert!(matches!(OverworldSnapshot::capture(&g.overworld).active_script_effect,Some(ScriptEffect::WaitFieldButton {show_arrow:true})));
                    assert!(!g.overworld.script_flags().get("__OBJ_HIDDEN_CERULEAN_GUARD_2").copied().unwrap_or(false));
                    // The receipt's opcode wait returns while A is still held.
                    for _ in 0..12 {g.update(&a);}
                    assert!(g.overworld.script_flags().get(flag).copied().unwrap_or(false));
                    assert!(g.overworld.script_flags().get("__OBJ_HIDDEN_CERULEAN_GUARD_2").copied().unwrap_or(false));
                    assert!(g.overworld.field_text_restore.is_none());
                    g.update(&idle);
                    for _ in 0..3000 {
                        if g.overworld.active_script_effect_label().as_deref()==Some("FinishFieldText") {break;}
                        let ack=receipt_prompt_needs_press(&g);g.update(if ack {&a} else {&idle});
                    }
                    assert_eq!(g.overworld.active_script_effect_label().as_deref(),Some("FinishFieldText"));
                } else {
                    assert!(g.save_data.game_data.bag.has_item_const("POKE_DOLL"));
                    g.update(&button(GbButton::B));
                    for n in 1..20 {g.update(&idle);assert_eq!(g.overworld.displayed_field_dialogue().unwrap().get_display_text(),Some((String::new(),String::new())),"blank{n}");}
                    g.update(&idle);assert_eq!(g.overworld.displayed_field_dialogue().unwrap().char_index(),1);
                    for _ in 0..3000 {
                        assert!(g.save_data.game_data.bag.has_item_const("POKE_DOLL"));
                        assert!(!g.overworld.script_flags().get(flag).copied().unwrap_or(false));
                        if matches!(OverworldSnapshot::capture(&g.overworld).active_script_effect,Some(ScriptEffect::WaitFieldButton {show_arrow:false})) {break;}
                        let ack=receipt_prompt_needs_press(&g);g.update(if ack {&a} else {&idle});
                    }
                    assert!(matches!(OverworldSnapshot::capture(&g.overworld).active_script_effect,Some(ScriptEffect::WaitFieldButton {show_arrow:false})));
                    assert!(!g.overworld.dialogue_needs_button(),"TX_WAIT_BUTTON must not draw an arrow");
                    let raw=serde_json::to_string(&OverworldSnapshot::capture(&g.overworld)).unwrap();
                    serde_json::from_str::<OverworldSnapshot>(&raw).unwrap().restore_into(&mut g.overworld);
                    for _ in 0..60 {g.update(&idle);assert!(g.save_data.game_data.bag.has_item_const("POKE_DOLL"));}
                    for _ in 0..12 {g.update(&a);assert!(g.overworld.displayed_field_dialogue().is_some());}
                    assert!(!g.save_data.game_data.bag.has_item_const("POKE_DOLL"));
                    assert!(g.overworld.script_flags().get(flag).copied().unwrap_or(false));
                    assert_eq!(g.overworld.active_script_effect_label().as_deref(),Some("CloseFieldText"));
                    assert!(!g.overworld.dialogue_needs_button());
                }
                // Final A-hold preserves the window; release closes it. Copycat
                // uses outer-skip, Bill uses the outer fresh confirmation.
                for _ in 0..8 {g.update(&a);assert!(g.overworld.displayed_field_dialogue().is_some());}
                for _ in 0..40 {g.update(&idle);}
                assert!(g.overworld.displayed_field_dialogue().is_none());
                let bag=g.save_data.game_data.bag.clone();let mut explanation=false;
                for t in 0..3000 {
                    assert!(!matches!(OverworldSnapshot::capture(&g.overworld).active_script_effect,Some(ScriptEffect::PrintItemFieldText {..})));
                    explanation|=g.overworld.displayed_field_dialogue().is_some_and(|d|d.get_display_text().is_some_and(|(a,b)|format!("{a} {b}").contains(if map==MapId::BillsHouse {"instead of me"} else {"scream"})));
                    let ack=receipt_prompt_needs_press(&g);g.update(if t==0||ack {&a} else {&idle});
                    if explanation && g.overworld.script_engine_idle() && g.overworld.active_script_effect_label().is_none() {break;}
                }
                assert!(explanation,"repeat conversation");assert_eq!(g.save_data.game_data.bag,bag);assert!(g.overworld.displayed_field_dialogue().is_none());
            }
        });
    }

    #[test]
    fn bill_and_copycat_refusals_preserve_items_flags_and_confirmation_kind() {
        use pokered_core::overworld::script_bridge::ScriptEffect;
        use pokered_core::snapshot::OverworldSnapshot;
        use pokered_data::items::ItemId;
        run_link_save_fixture(|| {
            for (map,full) in [(MapId::BillsHouse,true),(MapId::CopycatsHouse2F,true),(MapId::CopycatsHouse2F,false)] {
                let mut g=bill_copycat_fixture_168(map);let idle=InputState::new();let a=button(GbButton::A);
                if full {
                    for id in 1..=255 {
                        let id=ItemId::from_id(id);
                        if id==ItemId::from_const_name(if map==MapId::BillsHouse {"SS_TICKET"} else {"TM31"}).unwrap() || id==ItemId::from_const_name("POKE_DOLL").unwrap() {continue;}
                        g.save_data.game_data.bag.add_item(id,1).unwrap();if g.save_data.game_data.bag.is_full() {break;}
                    }
                    assert!(g.save_data.game_data.bag.is_full());
                } else {g.save_data.game_data.bag=pokered_core::items::inventory::Inventory::new();}
                let bag=g.save_data.game_data.bag.clone();let mut seen=false;
                for t in 0..6000 {
                    let effect=OverworldSnapshot::capture(&g.overworld).active_script_effect;
                    assert!(!matches!(effect,Some(ScriptEffect::PrintItemFieldText {..})),"no receipt with no gift");
                    seen|=matches!(effect,Some(ScriptEffect::WaitFieldButton {show_arrow:false})) && full;
                    if matches!(effect,Some(ScriptEffect::CloseFieldText)) || (map==MapId::BillsHouse && matches!(effect,Some(ScriptEffect::FinishFieldText {..}))) {seen=true;break;}
                    let ack=receipt_prompt_needs_press(&g);g.update(if t==0||ack {&a} else {&idle});
                }
                assert!(seen,"{map:?}, full={full}");
                assert_eq!(g.save_data.game_data.bag,bag);
                assert!(!g.overworld.script_flags().get(if map==MapId::BillsHouse {"EVENT_GOT_SS_TICKET"} else {"EVENT_GOT_TM31"}).copied().unwrap_or(false));
                if map==MapId::BillsHouse {
                    for _ in 0..40 {g.update(&idle);assert!(g.overworld.displayed_field_dialogue().is_some());}
                    g.update(&a);
                }
                for _ in 0..40 {g.update(&idle);}
                assert!(g.overworld.displayed_field_dialogue().is_none());
            }
        });
    }

    #[test]
    fn metronome_gift_waits_for_intro_then_sound_then_outer_confirmation() {
        use pokered_core::overworld::{Direction,script_bridge::{ScriptEffect,FieldParagraphPhase}};
        use pokered_core::snapshot::OverworldSnapshot;
        use pokered_data::items::ItemId;
        run_link_save_fixture(|| {
            for full in [false,true] {
                let mut g=fixture(Species::Bulbasaur,7,Direction::Up);
                g.state.config.language=pokered_core::game_state::Lang::En;
                g.save_data.game_data.bag=pokered_core::items::inventory::Inventory::new();
                if full {
                    for id in 1..=255 {
                        let id=ItemId::from_id(id);
                        if id==ItemId::from_id(235) {continue;} // TM35 must not stack into the full bag.
                        g.save_data.game_data.bag.add_item(id,1).unwrap();
                        if g.save_data.game_data.bag.is_full() {break;}
                    }
                }
                let original_bag=g.save_data.game_data.bag.clone();
                let idle=InputState::new();let a=button(GbButton::A);
                g.overworld.warp_to_map(MapId::CinnabarLabMetronomeRoom,7,3);
                for _ in 0..120 {g.update(&idle);}
                g.update(&button(GbButton::Up));for _ in 0..20 {g.update(&idle);}
                let mut intro=false;
                for t in 0..2400 {
                    let snap=OverworldSnapshot::capture(&g.overworld);
                    assert!(!g.save_data.game_data.bag.has_item_const("TM_35"));
                    assert!(!g.overworld.script_flags().get("EVENT_GOT_TM35").copied().unwrap_or(false));
                    if matches!(snap.active_script_effect,Some(ScriptEffect::WaitFieldPrompt {protected_remaining:0})) {intro=true;break;}
                    let ack=receipt_prompt_needs_press(&g);g.update(if t==0||ack {&a} else {&idle});
                }
                assert!(intro,"scientist's introductory PROMPT must precede GiveItem");
                for _ in 0..60 {g.update(&idle);}
                assert!(!g.save_data.game_data.bag.has_item_const("TM_35"));
                for _ in 0..8 {g.update(&a);}
                g.update(&idle);
                assert_eq!(g.save_data.game_data.bag.has_item_const("TM_35"),!full);
                let mut sound_start=None;let mut sound_end=None;
                for t in 0..2400 {
                    let snap=OverworldSnapshot::capture(&g.overworld);
                    if matches!(snap.active_script_effect,Some(ScriptEffect::PrintItemFieldText {phase:FieldParagraphPhase::PlayingSound,..})) {
                        assert!(!full);
                        assert!(!g.overworld.script_flags().get("EVENT_GOT_TM35").copied().unwrap_or(false));
                        if sound_start.is_none() {
                            sound_start=Some(t);
                            let raw=serde_json::to_string(&snap).unwrap();
                            serde_json::from_str::<OverworldSnapshot>(&raw).unwrap().restore_into(&mut g.overworld);
                        }
                        let mut keys=InputState::new();
                        if sound_start.is_some_and(|start|t==start+1) {keys.press(GbButton::B);keys.press(GbButton::Down);}
                        g.update(&keys);
                        assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),(7,3));
                    } else if sound_start.is_some() {sound_end=Some(t);break;} else {
                        if g.overworld.active_script_effect_label().as_deref()==Some("FinishFieldText") {break;}
                        let ack=receipt_prompt_needs_press(&g);g.update(if ack {&a} else {&idle});
                    }
                }
                assert_eq!(sound_start.is_some(),!full);
                if !full {
                    let expected=AudioOutput::new_pcm();expected.play_sfx(SfxId::GetItem1);
                    let mut duration=0;while expected.is_sfx_playing() && duration<1000 {expected.update_frame();duration+=1;}
                    assert_eq!(sound_end.unwrap()-sound_start.unwrap(),duration,"snapshot must not replay or shorten fanfare");
                }
                for _ in 0..60 {g.update(&idle);}
                assert_eq!(g.overworld.active_script_effect_label().as_deref(),Some("FinishFieldText"));
                assert_eq!(g.overworld.script_flags().get("EVENT_GOT_TM35").copied().unwrap_or(false),!full);
                let (top,bottom)=g.overworld.displayed_field_dialogue().unwrap().get_display_text().unwrap();
                assert!(format!("{top} {bottom}").contains(if full {"crammed full"} else {"received TM35"}));
                for _ in 0..8 {g.update(&a);assert!(g.overworld.displayed_field_dialogue().is_some());}
                for _ in 0..40 {g.update(&idle);}
                assert!(g.overworld.displayed_field_dialogue().is_none());
                if full {assert_eq!(g.save_data.game_data.bag,original_bag);} else {
                    let bag=g.save_data.game_data.bag.clone();let mut explanation=false;
                    for t in 0..3000 {
                        assert!(!matches!(OverworldSnapshot::capture(&g.overworld).active_script_effect,Some(ScriptEffect::PrintItemFieldText {..})),"repeat must not give another TM or receipt");
                        if let Some(d)=g.overworld.displayed_field_dialogue() {
                            explanation|=d.get_display_text().is_some_and(|(a,b)|format!("{a} {b}").contains("doesn't know"));
                        }
                        let ack=receipt_prompt_needs_press(&g);g.update(if t==0||ack {&a} else {&idle});
                        if explanation && g.overworld.script_engine_idle() && g.overworld.active_script_effect_label().is_none() {break;}
                    }
                    assert!(explanation,"repeat explains METRONOME");
                    assert_eq!(g.save_data.game_data.bag,bag);
                    assert!(g.overworld.displayed_field_dialogue().is_none());
                }
            }
        });
    }

    #[test]
    fn rod_and_voucher_receipts_keep_inner_sound_and_fresh_outer_confirmation() {
        use pokered_core::overworld::{Direction,script_bridge::{ScriptEffect,FieldParagraphPhase}};
        use pokered_core::snapshot::OverworldSnapshot;
        run_link_save_fixture(|| {
            for (map,x,y,item,flag) in [
                (MapId::VermilionOldRodHouse,2,5,"OLD_ROD","EVENT_GOT_OLD_ROD"),
                (MapId::FuchsiaGoodRodHouse,5,4,"GOOD_ROD","EVENT_GOT_GOOD_ROD"),
                (MapId::Route12SuperRodHouse,2,5,"SUPER_ROD","EVENT_GOT_SUPER_ROD"),
                (MapId::PokemonFanClub,3,2,"BIKE_VOUCHER","EVENT_GOT_BIKE_VOUCHER"),
            ] {
                let mut g=fixture(Species::Bulbasaur,x,Direction::Up);
                g.state.config.language=pokered_core::game_state::Lang::En;
                g.overworld.warp_to_map(map,x as u8,y);
                let idle=InputState::new();let a=button(GbButton::A);
                for _ in 0..120 {g.update(&idle);}
                g.update(&button(GbButton::Up));for _ in 0..20 {g.update(&idle);}
                let mut saw_story=false;let mut sound_started=None;let mut sound_ended=None;
                for t in 0..6000 {
                    let before=OverworldSnapshot::capture(&g.overworld);
                    let sound=matches!(before.active_script_effect,
                        Some(ScriptEffect::PrintItemFieldText {phase:FieldParagraphPhase::PlayingSound,..})
                        | Some(ScriptEffect::PrintFieldParagraph {phase:FieldParagraphPhase::PlayingSound,..}));
                    if sound {
                        if sound_started.is_none() {
                            sound_started=Some(t);
                            assert!(g.save_data.game_data.bag.has_item_const(item));
                            assert_eq!(g.overworld.script_flags().get(flag).copied().unwrap_or(false),map!=MapId::PokemonFanClub);
                            assert!(g.overworld.displayed_field_dialogue().unwrap().get_display_text().is_some_and(|(a,b)|format!("{a} {b}").contains("received")));
                            let raw=serde_json::to_string(&before).unwrap();serde_json::from_str::<OverworldSnapshot>(&raw).unwrap().restore_into(&mut g.overworld);
                        }
                        assert!(!g.overworld.dialogue_needs_button());
                        assert!(before.field_text_restore.is_none());
                        let pulse=sound_started.is_some_and(|start|t==start+1);
                        let mut keys=InputState::new();if pulse {keys.press(GbButton::B);keys.press(GbButton::Down);}
                        g.update(&keys);
                        assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),(x,u16::from(y)));
                    } else if sound_started.is_some() {sound_ended=Some(t);break;} else {
                        let prompt=receipt_prompt_needs_press(&g);
                        if matches!(before.active_script_effect,Some(ScriptEffect::WaitFieldPrompt {protected_remaining:0})) {
                            assert!(!g.save_data.game_data.bag.has_item_const(item));saw_story=true;
                            // Keep A held after PROMPT acknowledgement: inner
                            // ManualTextScroll must return without HoldA/Close.
                            for _ in 0..8 {g.update(&a);}
                            assert!(g.save_data.game_data.bag.has_item_const(item));
                            assert!(OverworldSnapshot::capture(&g.overworld).field_text_restore.is_none());
                            g.update(&idle);
                        } else {g.update(if t==0 || prompt || g.overworld.pending_choice.is_some() {&a} else {&idle});}
                    }
                }
                assert!(sound_started.is_some() && sound_ended.is_some(),"{map:?}: actual receipt jingle");
                let expected=AudioOutput::new_pcm();expected.play_sfx(if map==MapId::PokemonFanClub {SfxId::GetKeyItem} else {SfxId::GetItem1});
                let mut duration=0;while expected.is_sfx_playing() && duration<1000 {expected.update_frame();duration+=1;}
                assert_eq!(sound_ended.unwrap()-sound_started.unwrap(),duration,"{map:?}: uninterrupted full PCM jingle");
                assert_eq!(saw_story,map==MapId::PokemonFanClub);
                for _ in 0..60 {g.update(&idle);}
                assert!(g.overworld.dialogue_needs_button());
                assert_eq!(g.overworld.active_script_effect_label().as_deref(),Some(if map==MapId::FuchsiaGoodRodHouse {"FinishFieldText"} else {"PrintFieldParagraph"}));
                assert!(g.overworld.displayed_field_dialogue().unwrap().get_display_text().is_some_and(|(a,b)|format!("{a} {b}").contains("received")),"jingle must not dismiss receipt or start explanation automatically");
                if map!=MapId::FuchsiaGoodRodHouse {
                    g.update(&button(GbButton::B));
                    for t in 1..20 {g.update(&idle);assert_eq!(g.overworld.displayed_field_dialogue().unwrap().get_display_text(),Some((String::new(),String::new())),"blank {t}");}
                    g.update(&idle);assert_eq!(g.overworld.displayed_field_dialogue().unwrap().char_index(),1);
                    for _ in 0..3000 {
                        if g.overworld.active_script_effect_label().as_deref()==Some("FinishFieldText") {break;}
                        let ack=receipt_prompt_needs_press(&g);g.update(if ack {&a} else {&idle});
                    }
                }
                assert_eq!(g.overworld.active_script_effect_label().as_deref(),Some("FinishFieldText"));
                assert!(g.overworld.script_flags().get(flag).copied().unwrap_or(false));
                for _ in 0..40 {g.update(&idle);assert!(g.overworld.displayed_field_dialogue().is_some());}
                for _ in 0..8 {g.update(&a);assert!(g.overworld.displayed_field_dialogue().is_some());}
                for _ in 0..40 {g.update(&idle);}
                assert!(g.overworld.displayed_field_dialogue().is_none());
            }
        });
    }

    #[test]
    fn fishing_and_chairman_questions_auto_return_but_story_prompt_still_waits() {
        use pokered_core::overworld::Direction;
        run_link_save_fixture(|| {
            let cases = [
                (MapId::VermilionOldRodHouse, 2, 5, "OLD_ROD", "EVENT_GOT_OLD_ROD"),
                (MapId::FuchsiaGoodRodHouse, 5, 4, "GOOD_ROD", "EVENT_GOT_GOOD_ROD"),
                (MapId::Route12SuperRodHouse, 2, 5, "SUPER_ROD", "EVENT_GOT_SUPER_ROD"),
                (MapId::PokemonFanClub, 3, 2, "BIKE_VOUCHER", "EVENT_GOT_BIKE_VOUCHER"),
            ];
            for (map,x,y,item,flag) in cases {
                for accept in [false,true] {
                    let mut g=fixture(Species::Bulbasaur,u16::from(x),Direction::Up);
                    g.overworld.warp_to_map(map,x,y);
                    let idle=InputState::new();let a=button(GbButton::A);
                    for _ in 0..120 {g.update(&idle);}
                    g.update(&button(GbButton::Up));for _ in 0..20 {g.update(&idle);}
                    for t in 0..2400 {
                        let ack=receipt_prompt_needs_press(&g);
                        g.update(if t==0 || ack {&a} else {&idle});
                        if g.overworld.pending_choice.is_some() {break;}
                    }
                    assert_eq!(g.overworld.pending_choice.as_ref().expect("inner question must auto-return").options,["YES","NO"],"{map:?}");
                    let (top,bottom)=g.overworld.displayed_field_dialogue().unwrap().get_display_text().unwrap();
                    assert!(format!("{top} {bottom}").contains(if map==MapId::PokemonFanClub {"about my POKeMON?"} else {"like to fish?"}),"{map:?}: {top} / {bottom}");
                    assert!(!g.save_data.game_data.bag.has_item_const(item));
                    for _ in 0..20 {g.update(&idle);}
                    g.update(&button(if accept {GbButton::A} else {GbButton::B}));
                    let mut saw_story_prompt=false;
                    for _ in 0..6000 {
                        let ack=receipt_prompt_needs_press(&g);
                        if accept && map==MapId::PokemonFanClub && ack && g.overworld.active_script_effect_label().as_deref()==Some("WaitFieldPrompt") {
                            let d=g.overworld.displayed_field_dialogue().unwrap();
                            if !d.has_more_pages() && d.get_display_text().is_some_and(|(top,bottom)|format!("{top} {bottom}").contains("want you to have this!")) {
                                assert!(!g.save_data.game_data.bag.has_item_const(item),"original story ends in PROMPT before GiveItem");
                                for _ in 0..12 {g.update(&idle);}
                                assert!(!g.save_data.game_data.bag.has_item_const(item));
                                saw_story_prompt=true;
                            }
                        }
                        g.update(if ack {&a} else {&idle});
                        if g.overworld.script_engine_idle() && g.overworld.pending_dialogue.is_none()
                            && g.overworld.active_script_effect_label().is_none() && g.overworld.pending_choice.is_none() {break;}
                    }
                    assert_eq!(g.save_data.game_data.bag.has_item_const(item),accept,"{map:?}");
                    assert_eq!(g.overworld.script_flags().get(flag).copied().unwrap_or(false),accept,"{map:?}");
                    assert_eq!(saw_story_prompt,accept && map==MapId::PokemonFanClub);
                    assert!(g.overworld.displayed_field_dialogue().is_none(),"{map:?}: no leaked question accept={accept} choice={:?} effect={:?} pending={:?} inner={}",g.overworld.pending_choice,g.overworld.active_script_effect_label(),g.overworld.pending_dialogue,g.overworld.inner_field_text_open);
                }
            }
        });
    }

    #[test]
    #[ignore = "actual PCM NPC dialogue/cry ordering evidence"]
    fn capture_npc_cry_order_164() {
        use pokered_core::overworld::Direction;
        run_link_save_fixture(|| {
            let dir=std::path::PathBuf::from(std::env::var("FIDELITY_CRY_CAPTURE").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let mut g=fixture(Species::Bulbasaur,6,Direction::Up);
            g.overworld.warp_to_map(MapId::PokemonFanClub,6,5);
            let idle=InputState::new();for _ in 0..120 {g.update(&idle);}
            let mut input=InputState::new();let mut rows=Vec::new();
            for t in 0..360 {
                input.begin_frame();for (button,on) in [(GbButton::Up,t<20),(GbButton::A,(20..40).contains(&t)||(200..202).contains(&t)),(GbButton::Down,(220..240).contains(&t))] {
                    if on {input.press(button);} else {input.release(button);}
                }
                g.update(&input);
                let mut fb=FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);g.draw(&mut fb);
                fb.save_png(&dir.join(format!("frame-{t:04}.png"))).unwrap();
                rows.push(serde_json::json!({"t":t,"input_bits":input.raw_current(),"audio_channels":channels(&g),"sfx_playing":g.audio.as_ref().unwrap().is_sfx_playing(),"overworld":pokered_core::snapshot::OverworldSnapshot::capture(&g.overworld)}));
            }
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&rows).unwrap()).unwrap();
        });
    }

    #[test]
    #[ignore = "actual PCM NPC dialogue/cry ordering evidence"]
    fn capture_pet_text_session_165() {
        use pokered_core::overworld::Direction;
        run_link_save_fixture(|| {
            let dir=std::path::PathBuf::from(std::env::var("FIDELITY_CRY_CAPTURE").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let mut g=fixture(Species::Bulbasaur,6,Direction::Up);
            let machop=std::env::var("FIDELITY_PET_CASE").as_deref()==Ok("machop");
            let (map,x,y)=if machop {(MapId::VermilionCity,29,10)} else {(MapId::PokemonFanClub,6,5)};
            g.state.config.language=pokered_core::game_state::Lang::En;
            g.overworld.warp_to_map(map,x,y);
            let idle=InputState::new();for _ in 0..120 {g.update(&idle);}
            for n in &mut g.overworld.npc_states {n.movement_type=pokered_core::overworld::NpcMovementType::Stationary;n.x=n.home_x;n.y=n.home_y;n.walk_counter=0;}
            let mut input=InputState::new();let mut rows=Vec::new();
            for t in 0..720 {
                input.begin_frame();for (button,on) in [(GbButton::Up,t<20),(GbButton::A,(20..40).contains(&t)||(400..402).contains(&t)||(600..602).contains(&t)),(GbButton::Down,(450..452).contains(&t))] {
                    if on {input.press(button);} else {input.release(button);}
                }
                g.update(&input);
                let mut fb=FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);g.draw(&mut fb);
                fb.save_png(&dir.join(format!("frame-{t:04}.png"))).unwrap();
                rows.push(serde_json::json!({"t":t,"input_bits":input.raw_current(),"audio_channels":channels(&g),"sfx_playing":g.audio.as_ref().unwrap().is_sfx_playing(),"overworld":pokered_core::snapshot::OverworldSnapshot::capture(&g.overworld)}));
            }
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&rows).unwrap()).unwrap();
        });
    }

    #[test]
    fn all_pet_cries_follow_printing_block_input_and_keep_the_outer_prompt() {
        use pokered_core::overworld::Direction;
        use pokered_core::snapshot::OverworldSnapshot;
        run_link_save_fixture(|| {
            let cases = [
                (MapId::SSAnneB1FRooms, 8, "MACHOKE", ""),
                (MapId::MrFujisHouse, 3, "PSYDUCK", ""),
                (MapId::MrFujisHouse, 4, "NIDORINO", ""),
                (MapId::VermilionPidgeyHouse, 2, "PIDGEY", ""),
                (MapId::CeladonCity, 7, "POLIWRATH", ""),
                (MapId::VermilionCity, 5, "MACHOP", ""),
                (MapId::PokemonFanClub, 3, "PIKACHU", ""),
                (MapId::PokemonFanClub, 4, "SEEL", ""),
                (MapId::PewterNidoranHouse, 1, "NIDORAN_M", ""),
                (MapId::SaffronPidgeyHouse, 2, "PIDGEY", ""),
                (MapId::CopycatsHouse1F, 3, "CHANSEY", ""),
                (MapId::Route16FlyHouse, 2, "FEAROW", ""),
                (MapId::LavenderCuboneHouse, 1, "CUBONE", ""),
                (MapId::SSAnne1FRooms, 8, "WIGGLYTUFF", ""),
                (MapId::ViridianNicknameHouse, 3, "SPEAROW", ""),
                (MapId::SaffronCity, 12, "PIDGEOT", "SAFFRON_CITY_OBJ_12"),
                (MapId::CeladonMansion1F, 1, "MEOWTH", ""),
                (MapId::CeladonMansion1F, 3, "CLEFAIRY", ""),
                (MapId::CeladonMansion1F, 4, "NIDORAN_F", ""),
            ];
            for (map,npc_id,species,toggle) in cases {
                let mut g=fixture(Species::Bulbasaur,1,Direction::Up);
                g.state.config.language=pokered_core::game_state::Lang::En;
                g.overworld.warp_to_map(map,1,1);
                let idle=InputState::new();for _ in 0..120 {g.update(&idle);}
                let npc=g.overworld.npc_states.iter().find(|n|n.text_id==npc_id).unwrap_or_else(||panic!("{map:?}/{species}: NPC not loaded, current={:?}",g.overworld.state.current_map)).clone();
                let positions=[(npc.home_x,npc.home_y.saturating_add(1),Direction::Up,GbButton::Up),
                    (npc.home_x.saturating_sub(1),npc.home_y,Direction::Right,GbButton::Right),
                    (npc.home_x.saturating_add(1),npc.home_y,Direction::Left,GbButton::Left),
                    (npc.home_x,npc.home_y.saturating_sub(1),Direction::Down,GbButton::Down)];
                let (x,y,facing,key)=positions.into_iter().find(|(x,y,_,_)| {
                    pokered_core::overworld::update::is_script_walkable_tile(g.overworld.map_data.as_ref().unwrap(),*x,*y)
                        && !g.overworld.npc_states.iter().any(|n|n.home_x==*x && n.home_y==*y)
                }).expect("a valid adjacent standing tile");
                g.overworld.warp_to_map(map,x as u8,y as u8);
                if !toggle.is_empty() {g.overworld.set_flag_live(&format!("__OBJ_HIDDEN_{toggle}"),false);}
                let idle=InputState::new();for _ in 0..120 {g.update(&idle);}
                for n in &mut g.overworld.npc_states {n.movement_type=pokered_core::overworld::NpcMovementType::Stationary;n.x=n.home_x;n.y=n.home_y;n.walk_counter=0;if n.text_id==npc_id {n.visible=true;}}
                g.overworld.state.player.facing=facing;
                let mut input=InputState::new();let mut started=None;let mut ended=None;
                for t in 0..2400 {
                    input.begin_frame();
                    for (button,on) in [(key,t<20),(GbButton::A,(20..40).contains(&t)),(GbButton::B,started.is_some_and(|begin|t==begin+1)),(GbButton::Down,started.is_some_and(|begin|t==begin+3))] {
                        if on {input.press(button);} else {input.release(button);}
                    }
                    g.update(&input);
                    let snap=OverworldSnapshot::capture(&g.overworld);
                    let playing=snap.active_script_effect.as_ref().is_some_and(|e|matches!(e,pokered_core::overworld::script_bridge::ScriptEffect::PlayCry{started:true,..}));
                    if playing {
                        if started.is_none() {started=Some(t);let json=serde_json::to_string(&snap).unwrap();let restored:OverworldSnapshot=serde_json::from_str(&json).unwrap();restored.restore_into(&mut g.overworld);}
                        let d=g.overworld.displayed_field_dialogue().expect("complete pet text remains throughout cry");
                        assert!(d.waiting_for_input() && !d.has_more_pages(),"{map:?}/{species}");
                        assert!(!g.overworld.dialogue_needs_button(),"cry is not an A/B prompt");
                        assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),(x,y));
                    } else if started.is_some() {ended=Some(t);break;}
                }
                let (begin,end)=(started.unwrap_or_else(||panic!("{map:?}/{species}: no cry, position={:?}, target={:?}, effect={:?}, dialogue={:?}",g.overworld.state.player,npc,g.overworld.active_script_effect_label(),g.overworld.pending_dialogue)),ended.expect("cry finishes"));
                let expected=AudioOutput::new_pcm();play_species_cry(&expected,Species::from_scene_name(species).unwrap());
                let mut duration=0;while expected.is_sfx_playing() && duration<1000 {expected.update_frame();duration+=1;}
                assert_eq!(end-begin,duration,"{map:?}/{species}: full uninterrupted PCM cry length");
                for _ in 0..30 {g.update(&idle);}
                for _ in 0..1000 {
                    if g.overworld.displayed_field_dialogue().is_some_and(|d|d.waiting_for_input() && !d.has_more_pages()) {break;}
                    g.update(&idle);
                }
                assert!(g.overworld.displayed_field_dialogue().is_some(),"outer dialogue must still wait after the sound");
                assert!(g.overworld.dialogue_needs_button());
                if map==MapId::VermilionCity {
                    let (a,b)=g.overworld.displayed_field_dialogue().unwrap().get_display_text().unwrap();
                    assert!(format!("{a} {b}").contains("Guoh"), "leading PARA must wait before narration");
                    // A/B during the cry was released before the paragraph's
                    // Joypad poll, so it cannot acknowledge this wait.
                    assert_eq!(g.overworld.active_script_effect_label().as_deref(),Some("PrintFieldParagraph"));
                    for _ in 0..40 {g.update(&idle);}
                    assert_eq!(g.overworld.active_script_effect_label().as_deref(),Some("PrintFieldParagraph"));
                    let ack=button(GbButton::B);g.update(&ack);
                    let snap=OverworldSnapshot::capture(&g.overworld);
                    assert!(matches!(snap.active_script_effect,Some(pokered_core::overworld::script_bridge::ScriptEffect::PrintFieldParagraph {phase:pokered_core::overworld::script_bridge::FieldParagraphPhase::BlankDelay {remaining:20},..})));
                    let json=serde_json::to_string(&snap).unwrap();
                    serde_json::from_str::<OverworldSnapshot>(&json).unwrap().restore_into(&mut g.overworld);
                    for frame in 1..=19 {
                        g.update(&idle);
                        assert_eq!(g.overworld.displayed_field_dialogue().unwrap().get_display_text(),Some((String::new(),String::new())),"blank frame {frame}");
                        assert!(!g.overworld.dialogue_needs_button());
                        assert!(OverworldSnapshot::capture(&g.overworld).field_text_restore.is_none(),"paragraph must not close text/sprites");
                    }
                    g.update(&idle);
                    assert_eq!(g.overworld.active_script_effect_label().as_deref(),Some("PrintFieldParagraph"));
                    assert_eq!(g.overworld.displayed_field_dialogue().unwrap().get_display_text().unwrap().0,"A");
                    for _ in 0..1000 {
                        g.update(&idle);
                        if g.overworld.active_script_effect_label().as_deref()==Some("FinishFieldText") {break;}
                    }
                    let (a,b)=g.overworld.displayed_field_dialogue().unwrap().get_display_text().unwrap();
                    assert!(format!("{a} {b}").contains("stomping"));
                    assert!(g.overworld.dialogue_needs_button());
                }
                let mut a=button(GbButton::A);for _ in 0..4 {g.update(&a);a.begin_frame();assert!(g.overworld.displayed_field_dialogue().is_some(),"A hold must keep the outer window open");}
                for _ in 0..30 {g.update(&idle);}
                assert!(g.overworld.displayed_field_dialogue().is_none(),"{map:?}/{species}: closes after release");
            }
        });
    }

    #[test]
    fn a_button_first_held_during_the_cry_is_polled_after_sound_returns() {
        use pokered_core::overworld::Direction;
        use pokered_core::snapshot::OverworldSnapshot;
        run_link_save_fixture(|| {
            for held in [GbButton::A,GbButton::B] {
                let mut g=fixture(Species::Bulbasaur,6,Direction::Up);
                g.overworld.warp_to_map(MapId::PokemonFanClub,6,5);
                let idle=InputState::new();for _ in 0..120 {g.update(&idle);}
                let mut input=InputState::new();let mut cry_started=None;let mut returned=None;
                for t in 0..2400 {
                    input.begin_frame();for (key,on) in [(GbButton::Up,t<20),(GbButton::A,(20..40).contains(&t)|| (held==GbButton::A && cry_started.is_some_and(|c|t>c))),(GbButton::B,held==GbButton::B && cry_started.is_some_and(|c|t>c))] {if on {input.press(key);} else {input.release(key);}}
                    g.update(&input);
                    let snapshot=OverworldSnapshot::capture(&g.overworld);
                    if snapshot.active_script_effect.as_ref().is_some_and(|e|matches!(e,pokered_core::overworld::script_bridge::ScriptEffect::PlayCry {started:true,..})) {
                        cry_started.get_or_insert(t);assert!(g.overworld.displayed_field_dialogue().is_some());
                    } else if cry_started.is_some() {returned=Some(t);break;}
                }
                assert!(returned.is_some(),"sound must return while the button is held");
                // Joypad did not run during WaitForSoundToFinish. Its first
                // subsequent poll sees the newly held key against hJoyLast.
                input.begin_frame();g.update(&input);
                if held==GbButton::A {
                    assert!(g.overworld.displayed_field_dialogue().is_some(),"HoldTextDisplayOpen waits for A release");
                    let json=serde_json::to_string(&OverworldSnapshot::capture(&g.overworld)).unwrap();
                    serde_json::from_str::<OverworldSnapshot>(&json).unwrap().restore_into(&mut g.overworld);
                    for _ in 0..8 {input.begin_frame();g.update(&input);assert!(g.overworld.displayed_field_dialogue().is_some());}
                    for _ in 0..20 {g.update(&idle);}
                }
                assert!(g.overworld.displayed_field_dialogue().is_none(),"held B closes immediately; A closes on release without a second press");
            }
        });
    }

    #[test]
    fn museum_choice_retains_final_question_page_with_original_early_money_box() {
        use pokered_core::overworld::Direction;
        run_link_save_fixture(|| {
            let mut g = fixture(Species::Bulbasaur, 10, Direction::Up);
            g.overworld.warp_to_map(MapId::Museum1F, 10, 5);
            let idle = InputState::new();
            let a = button(GbButton::A);
            let b = button(GbButton::B);
            let up = button(GbButton::Up);
            for _ in 0..120 { g.update(&idle); }
            for _ in 0..30 { g.update(&up); }
            let mut saw_price_page_with_money=false;
            for frame in 0..1800 {
                if g.overworld.pending_choice.is_some() { break; }
                g.update(if frame % 30 == 0 { &a } else { &idle });
                if let Some(d)=&g.overworld.pending_dialogue {
                    if d.current_page()==0 {
                        assert!(g.overworld.script_money_box.is_some(),"original MONEY_BOX precedes PrintText");
                        saw_price_page_with_money=true;
                    }
                }
            }
            assert!(saw_price_page_with_money);
            assert!(g.overworld.pending_choice.is_some());
            assert!(g.overworld.script_money_box.is_some());
            let question = g.overworld.displayed_field_dialogue().unwrap();
            assert!(question.current_page() > 0, "must preserve the final page, not the ticket-price page");
            let (top,bottom) = question.get_display_text().unwrap();
            assert_eq!(format!("{top} {bottom}").trim(), "Would you like to come in?");
            for _ in 0..20 { g.update(&idle); }
            g.update(&b);
            for frame in 0..1800 {
                g.update(if frame % 30 == 0 { &b } else { &idle });
                if g.overworld.script_engine_idle() && g.overworld.pending_dialogue.is_none() { break; }
            }
            assert!(g.overworld.pending_choice.is_none());
            assert!(g.overworld.last_script_dialogue.is_none());
            assert!(g.overworld.displayed_field_dialogue().is_none());
            assert!(g.overworld.script_money_box.is_none());
            assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),(10,5));
        });
    }

}

#[cfg(all(test, not(target_os = "none")))]
mod choice_question_capture_158 {
    use super::*;
    #[test]
    #[ignore = "controlled SRAM Continue, identical inputs and hardware frames"]
    fn capture_choice_question_158() {
        std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
            let dir = std::path::PathBuf::from(std::env::var("FIDELITY_CHOICE_CAPTURE").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let save = dir.join("fixture.sav");
            std::fs::copy(std::env::var("FIDELITY_SAFARI_SRAM").unwrap(), &save).unwrap();
            let mut g = PokemonGame::new_with_options(GameVersion::Red, Some(save), None, None,
                false, None, false, true, #[cfg(feature = "debug-server")] None);
            g.state.config.language = pokered_core::game_state::Lang::En;
            g.state.config.text_speed = pokered_core::game_state::TextSpeed::Fast;
            let idle = InputState::new();
            let mut a = InputState::new(); a.press(GbButton::A);
            let mut saw_menu = false;
            for t in 0..2000 {
                saw_menu |= g.state.screen == GameScreen::MainMenu;
                if g.state.screen == GameScreen::Overworld { break; }
                g.update(if t % 20 == 19 { &a } else { &idle });
            }
            assert!(saw_menu);
            assert_eq!(g.state.screen, GameScreen::Overworld);
            // Diagnostic endpoint, not paid admission or last-ball proof.
            g.overworld.end_safari_game();
            g.overworld.warp_to_map(MapId::SafariZoneGate, 3, 4);
            g.overworld.set_rng_seed(1);
            for _ in 0..120 { g.update(&idle); }
            let mut left = InputState::new(); left.press(GbButton::Left);
            for _ in 0..8 { g.update(&left); }
            for _ in 0..8 { g.update(&idle); }
            assert_eq!((g.overworld.state.player.x,g.overworld.state.player.y),(3,4));
            let mut input = InputState::new();
            let mut rows = Vec::new();
            for t in -1i32..151 {
                if t >= 0 {
                    input.begin_frame();
                    if t == 0 || t == 80 { input.press(GbButton::A); }
                    if t == 2 || t == 82 { input.release(GbButton::A); }
                    g.update(&input);
                }
                let mut fb = FrameBuffer::new(dotzuki_engine::render_config::RenderConfig::new(160,144), pokered_renderer::Rgba::WHITE);
                g.draw(&mut fb);
                fb.save_png(&dir.join(format!("frame-{:04}.png",t+1))).unwrap();
                rows.push(serde_json::json!({"t":t,"input_bits":input.raw_current(),"screen":format!("{:?}",g.state.screen),
                    "overworld":pokered_core::snapshot::OverworldSnapshot::capture(&g.overworld)}));
                if t == 120 { assert!(g.overworld.pending_choice.is_some(),"choice must be open at same hardware frame"); }
            }
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&rows).unwrap()).unwrap();
        }).unwrap().join().unwrap();
    }
}

#[cfg(all(test, not(target_os = "none")))]
mod typing_pulse_capture_159 {
    use super::*;
    #[test]
    #[ignore = "controlled original-SRAM Continue and short typing pulse"]
    fn capture_typing_pulse_159() {
        std::thread::Builder::new().stack_size(16*1024*1024).spawn(|| {
            let dir=std::path::PathBuf::from(std::env::var("FIDELITY_TYPING_CAPTURE").unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let save=dir.join("fixture.sav");
            std::fs::copy(std::env::var("FIDELITY_SAFARI_SRAM").unwrap(),&save).unwrap();
            let mut g=PokemonGame::new_with_options(GameVersion::Red,Some(save),None,None,false,None,false,true,#[cfg(feature="debug-server")] None);
            g.state.config.language=pokered_core::game_state::Lang::En;
            let idle=InputState::new();let mut a=InputState::new();a.press(GbButton::A);
            let mut saw_menu=false;
            for t in 0..2000 {saw_menu|=g.state.screen==GameScreen::MainMenu;if g.state.screen==GameScreen::Overworld {break;}g.update(if t%20==19 {&a} else {&idle});}
            assert!(saw_menu);assert_eq!(g.state.screen,GameScreen::Overworld);
            // Match original wOptions &15 ==3; exclude text-speed confounding.
            g.state.config.text_speed=pokered_core::game_state::TextSpeed::Medium;
            g.overworld.end_safari_game();g.overworld.warp_to_map(MapId::SafariZoneGate,3,4);g.overworld.set_rng_seed(1);
            for _ in 0..120 {g.update(&idle);}
            let mut left=InputState::new();left.press(GbButton::Left);for _ in 0..8 {g.update(&left);}for _ in 0..8 {g.update(&idle);}
            let pulse_button=if std::env::var_os("FIDELITY_TYPING_B").is_some() {GbButton::B} else {GbButton::A};
            let fixed=std::env::var("FIDELITY_TYPING_FIXED_PULSE").ok().map(|v|v.parse::<i32>().unwrap());
            let pulse=std::env::var_os("FIDELITY_TYPING_PULSE").is_some();let mut first=None;let mut input=InputState::new();let mut rows=Vec::new();
            for t in 0i32..145 {
                input.begin_frame();if t==0 {input.press(GbButton::A);}if t==2 {input.release(GbButton::A);}
                if pulse {if fixed.map_or_else(||first.is_some_and(|f|t==f+10),|f|t==f) {input.press(pulse_button);}if fixed.map_or_else(||first.is_some_and(|f|t==f+12),|f|t==f+2) {input.release(pulse_button);}}
                g.update(&input);
                let count=g.overworld.pending_dialogue.as_ref().map(|d|d.char_index());if first.is_none()&&count.is_some_and(|n|n>0) {first=Some(t);}
                let mut fb=FrameBuffer::new(dotzuki_engine::render_config::RenderConfig::new(160,144),pokered_renderer::Rgba::WHITE);g.draw(&mut fb);fb.save_png(&dir.join(format!("frame-{t:04}.png"))).unwrap();
                rows.push(serde_json::json!({"t":t,"first_letter":first,"letters":count,"input_bits":input.raw_current(),"config_speed":format!("{:?}",g.state.config.text_speed),"overworld":pokered_core::snapshot::OverworldSnapshot::capture(&g.overworld)}));
            }
            assert!(first.is_some());std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&rows).unwrap()).unwrap();
        }).unwrap().join().unwrap();
    }
}

#[cfg(all(test, not(target_os = "none")))]
mod admission_capture_162 {
    use super::*;
    #[test]
    #[ignore = "matched SRAM Continue, admission input replay and complete raw frames"]
    fn capture_admission_162() {
        std::thread::Builder::new().stack_size(16*1024*1024).spawn(|| {
            let dir=std::path::PathBuf::from(std::env::var("FIDELITY_ADMISSION_CAPTURE").unwrap());std::fs::create_dir_all(&dir).unwrap();
            let save=dir.join("fixture.sav");std::fs::copy(std::env::var("FIDELITY_SAFARI_SRAM").unwrap(),&save).unwrap();
            let mut g=PokemonGame::new_with_options(GameVersion::Red,Some(save),None,None,false,None,false,true,#[cfg(feature="debug-server")] None);
            g.state.config.language=pokered_core::game_state::Lang::En;
            let idle=InputState::new();let mut a=InputState::new();a.press(GbButton::A);let mut menu=false;
            for t in 0..2000 {menu|=g.state.screen==GameScreen::MainMenu;if g.state.screen==GameScreen::Overworld {break;}g.update(if t%20==19 {&a} else {&idle});}
            assert!(menu);assert_eq!(g.state.screen,GameScreen::Overworld);
            g.state.config.text_speed=pokered_core::game_state::TextSpeed::Medium;
            g.overworld.end_safari_game();g.overworld.warp_to_map(MapId::SafariZoneGate,3,3);g.overworld.set_rng_seed(1);for _ in 0..120 {g.update(&idle);}
            let replay:Option<Vec<Vec<String>>>=std::env::var("FIDELITY_ADMISSION_INPUTS").ok().map(|p|serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap());
            let mut input=InputState::new();let mut rows=Vec::new();let mut controls=Vec::new();
            for t in 0..650 {
                let buttons=if let Some(replay)=&replay {replay[t].clone()} else if t<20 {vec!["up".to_string()]} else {
                    let advance=g.overworld.pending_dialogue.as_ref().is_some_and(|d|d.waiting_for_input() && !d.holding_open() &&
                        (d.has_more_pages() || d.get_display_text().is_some_and(|(top,_)|top.starts_with("Welcome"))));
                    if advance {vec!["a".to_string()]} else {Vec::new()}
                };
                input.begin_frame();for (name,button) in [("up",GbButton::Up),("a",GbButton::A)] {if buttons.iter().any(|v|v==name) {input.press(button);}else {input.release(button);}}
                g.update(&input);let mut fb=FrameBuffer::new(dotzuki_engine::render_config::RenderConfig::new(160,144),pokered_renderer::Rgba::WHITE);g.draw(&mut fb);fb.save_png(&dir.join(format!("frame-{t:04}.png"))).unwrap();
                rows.push(serde_json::json!({"t":t,"input_bits":input.raw_current(),"screen":format!("{:?}",g.state.screen),"overworld":pokered_core::snapshot::OverworldSnapshot::capture(&g.overworld)}));controls.push(buttons);
            }
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&rows).unwrap()).unwrap();std::fs::write(dir.join("inputs.json"),serde_json::to_string_pretty(&controls).unwrap()).unwrap();
        }).unwrap().join().unwrap();
    }
}

#[cfg(all(test, not(target_os = "none")))]
mod offer_capture_163 {
    use super::*;
    #[test]
    #[ignore = "matched SRAM Continue, admission input replay and complete raw frames"]
    fn capture_offer_163() {
        std::thread::Builder::new().stack_size(16*1024*1024).spawn(|| {
            let dir=std::path::PathBuf::from(std::env::var("FIDELITY_OFFER_CAPTURE").unwrap());std::fs::create_dir_all(&dir).unwrap();
            let save=dir.join("fixture.sav");std::fs::copy(std::env::var("FIDELITY_SAFARI_SRAM").unwrap(),&save).unwrap();
            let mut g=PokemonGame::new_with_options(GameVersion::Red,Some(save),None,None,false,None,false,true,#[cfg(feature="debug-server")] None);
            g.state.config.language=pokered_core::game_state::Lang::En;
            let idle=InputState::new();let mut a=InputState::new();a.press(GbButton::A);let mut menu=false;
            for t in 0..2000 {menu|=g.state.screen==GameScreen::MainMenu;if g.state.screen==GameScreen::Overworld {break;}g.update(if t%20==19 {&a} else {&idle});}
            assert!(menu);assert_eq!(g.state.screen,GameScreen::Overworld);
            g.state.config.text_speed=pokered_core::game_state::TextSpeed::Medium;
            g.overworld.end_safari_game();
            // Controlled first-visit setup on the same continued SRAM, in both builds.
            for flag in ["EVENT_GOT_OLD_ROD","EVENT_GOT_GOOD_ROD","EVENT_GOT_SUPER_ROD","EVENT_GOT_BIKE_VOUCHER"] {g.overworld.set_flag_live(flag,false);}
            g.save_data.game_data.event_flags=g.overworld.unified_flags().as_bytes().to_vec();
            let _=g.save_data.game_data.bag.remove_item(pokered_data::items::ItemId::Bicycle,1);
            let _=g.save_data.game_data.bag.remove_item(pokered_data::items::ItemId::BikeVoucher,1);
            let (map,x,y)=match std::env::var("FIDELITY_OFFER_MAP").unwrap().as_str() {
                "old" => (MapId::VermilionOldRodHouse,2,5),
                "good" => (MapId::FuchsiaGoodRodHouse,5,4),
                "super" => (MapId::Route12SuperRodHouse,2,5),
                "chairman" => (MapId::PokemonFanClub,3,2),
                other => panic!("unknown case {other}"),
            };g.overworld.warp_to_map(map,x,y);g.overworld.set_rng_seed(1);for _ in 0..120 {g.update(&idle);}
            let replay:Option<Vec<Vec<String>>>=std::env::var("FIDELITY_OFFER_INPUTS").ok().map(|p|serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap());
            let mut input=InputState::new();let mut rows=Vec::new();let mut controls=Vec::new();
            for t in 0..1200 {
                let buttons=if let Some(replay)=&replay {replay[t].clone()} else if t<20 {vec!["up".to_string()]} else if (20..40).contains(&t) {vec!["a".to_string()]} else {
                    let advance=g.overworld.pending_dialogue.as_ref().is_some_and(|d|d.waiting_for_input() && !d.holding_open() &&
                        d.has_more_pages());
                    if advance {vec!["a".to_string()]} else {Vec::new()}
                };
                input.begin_frame();for (name,button) in [("up",GbButton::Up),("a",GbButton::A)] {if buttons.iter().any(|v|v==name) {input.press(button);}else {input.release(button);}}
                g.update(&input);let mut fb=FrameBuffer::new(dotzuki_engine::render_config::RenderConfig::new(160,144),pokered_renderer::Rgba::WHITE);g.draw(&mut fb);fb.save_png(&dir.join(format!("frame-{t:04}.png"))).unwrap();
                rows.push(serde_json::json!({"t":t,"input_bits":input.raw_current(),"screen":format!("{:?}",g.state.screen),"overworld":pokered_core::snapshot::OverworldSnapshot::capture(&g.overworld)}));controls.push(buttons);
            }
            std::fs::write(dir.join("frames.json"),serde_json::to_string_pretty(&rows).unwrap()).unwrap();std::fs::write(dir.join("inputs.json"),serde_json::to_string_pretty(&controls).unwrap()).unwrap();
        }).unwrap().join().unwrap();
    }
}
