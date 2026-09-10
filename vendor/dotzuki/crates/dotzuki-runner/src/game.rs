//! The `dotzuki run` game runtime: [`RunnerGame`].
//!
//! [`RunnerGame`] boots a [`LoadedProject`] into a playable game with **zero
//! game-specific code**: an overworld driven by the generic
//! [`OverworldActor`], storyline dispatch from the DSL routing table
//! (`@trigger` declarations), a scene-pumping VM on top of
//! [`ScriptEngine`] (one short-lived engine per scene activation, the wuxia
//! pattern), textbox/choice UI from `dotzuki-ui`, and placeholder sprites drawn
//! procedurally (no embedded assets).
//!
//! # State machine
//!
//! ```text
//! Overworld ──A on NPC──▶ Text ──last page + A──▶ (pump) ──▶ Overworld
//!    │                     ▲   │ ShowChoice        │ next command
//!    │step onto warp       │   ▼                   ▼
//!    │                     └── Choice ──A──▶ signal_done(Number) ──▶ (pump)
//!    │Start
//!    ▼
//! Menu (party / bag / save) ──B/Start──▶ Overworld
//! WarpTransition (fade out → load map → fade in → opening dispatch)
//! ```
//!
//! A [`Mode::Delay`] suspends the scene for N frames. `Mode::Idle` is the
//! resting state of a map-less (dialogue-only) project once its entry scene
//! has finished.
//!
//! # Scene dispatch rules
//!
//! On entering a map (boot or warp), after any fade-in:
//!
//! 1. every `on_enter` route for the map fires, sequentially;
//! 2. else the map scene's `<SceneName>OnLoad` (from `@load`) runs;
//! 3. else the map scene's storyline `main` plays once, guarded by the
//!    `__played_main_<map>` flag.
//!
//! Talking to an NPC tries, in order: the NPC's `talk` field as a storyline
//! name, a route whose `npc` matches the NPC's name/id, the map scene's
//! `main`, and finally the `talk` field shown as a raw one-off line.
//!
//! # Command handling
//!
//! v1 handles `ShowText`, `ShowChoice`, `Delay`, `WarpTo`, `FadeScreen`,
//! `SetFlag`/`ResetFlag`/`CheckFlag` and the audio commands (played through
//! [`RunnerAudio`] when the project ships `data/audio/` tracks and a device
//! is available; silent no-ops otherwise). `StartBattle`/`StartWildBattle`
//! suspend the scene and arm the generic battle system ([`crate::battle`])
//! when the manifest has a `battle` section — otherwise they auto-complete
//! with `"win"` like any unimplemented command; a lost battle arms the
//! game-over whiteout (see [`menu`]). `OpenShop` suspends the scene and
//! opens the buy-only shop UI. Any other command logs a loud warning and is
//! auto-completed with `Void` — an unimplemented command must never deadlock
//! the scene VM.
//!
//! # Menus, shops, game-over
//!
//! Start in the overworld opens the pause menu (party view / bag / save);
//! the scene `openShop` command opens a buy-only shop; losing a battle
//! triggers the whiteout (heal + return to the entry spawn). All three live
//! in the [`menu`] child module; money is runner-owned, seeded from the
//! manifest's `shop` section and carried in the v3 save.
//!
//! # Random encounters
//!
//! A map's objects sidecar may carry an `encounters` block (`{rate, zones}`
//! — pokered `wild_data`-shaped, `rate` in /256 per-step units). A completed
//! walk step onto a zoned tile rolls once; a hit picks a weighted id from
//! the zone's table and arms a **sceneless** battle (no scene suspended —
//! win/run returns to the overworld in place, a loss goes straight to the
//! whiteout). Warp tiles take priority over the roll on the same tile.

#[cfg(all(feature = "watch", not(target_arch = "wasm32")))]
use std::collections::HashSet;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use dotzuki_engine::camera::{Camera, Rect, Vec2};
use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::overworld::actor::{frame_col, OverworldActor, OverworldCollision};
use dotzuki_engine::overworld::types::Direction;
use dotzuki_engine::render::{FrameBuffer, Rgba, TileRect, Ui};
use dotzuki_engine_script::command::{CommandResult, ScriptCommand};
use dotzuki_engine_script::engine::ScriptEngine;
use dotzuki_renderer::embedded_font;
use dotzuki_renderer::input::{GbButton, InputState};
use dotzuki_renderer::walk_sprite::WalkSprite;
use dotzuki_ui::widgets::dialog::{draw_dialog, wrap_lines};
use dotzuki_ui::widgets::flex_menu::{draw_flex_menu, FlexMenuState};
use dotzuki_ui::FrameBufferPainter;

use crate::audio::RunnerAudio;
use crate::battle::{
    Battle, BattleOutcome, BattleRng, BattleSetup, PartyMemberState, ScriptedRng, XorshiftRng,
};
use crate::manifest::DEFAULT_START_MONEY;
use crate::map::RuntimeMap;
use crate::project::LoadedProject;
use crate::save::{GameSave, PartyMemberSave, PlayerSave, DEFAULT_SAVE_FILE, SAVE_VERSION};
use crate::vfs::join_path;
#[cfg(all(feature = "watch", not(target_arch = "wasm32")))]
use crate::watch::ProjectWatcher;

mod menu;
use menu::{MenuState, ShopState, WhiteoutState};

/// Logical framebuffer width (window mode and headless rendering).
pub const SCREEN_W: i32 = 320;
/// Logical framebuffer height.
pub const SCREEN_H: i32 = 240;

/// Fade frames per warp-transition phase (out / in).
const FADE_FRAMES: u32 = 10;
/// Cosmetic blackout frames for a `FadeScreen` command.
const FLASH_FRAMES: u32 = 10;
/// Frames to wait after applying a hot-reload batch before the next one —
/// coalesces editor save bursts into a single reload.
#[cfg(all(feature = "watch", not(target_arch = "wasm32")))]
const WATCH_DEBOUNCE_FRAMES: u32 = 10;

/// Tile-grid geometry of the bottom dialogue box (8px tiles, 40×30 grid).
pub(crate) const DIALOG_AREA: TileRect = TileRect::new(0, 24, 40, 6);
/// Text lines per dialogue page (`content.th / line_height`, as in draw_dialog).
const DIALOG_LINES_PER_PAGE: usize = 2;
/// Wrap budget for a dialogue line: the box interior width in pixels
/// (Fusion Pixel font: Latin 5px, CJK 10px advance — see
/// `dotzuki_renderer::embedded_font::char_advance`).
const DIALOG_WIDTH_PX: usize = (DIALOG_AREA.tw as usize - 2) * 8;

/// Options for booting a [`RunnerGame`].
#[derive(Debug, Clone, Default)]
pub struct RunnerOptions {
    /// Map to spawn on, overriding the manifest's `game.entryMap`.
    pub map: Option<String>,
    /// UI/script language (`"en"` / `"zh"`); drives `@t` bilingual text.
    pub lang: String,
    /// Watch the project's data/gfx/scene dirs and hot-reload changed
    /// content (windowed mode; the CLI ignores this for headless runs).
    pub watch: bool,
    /// Headless run: never opens an audio device — audio commands resolve
    /// silently (CI/smoke tests).
    pub headless: bool,
    /// PCM pull-render audio (WASM shell): play commands create the audio
    /// engine without an output device, and the host pulls samples via
    /// [`RunnerGame::render_audio`]. Independent of `headless` — cpal is
    /// never touched either way.
    pub pcm_audio: bool,
    /// Ignore an existing save file on boot (`--fresh`).
    pub fresh: bool,
    /// Save file location override (`--save-file`); the default is
    /// `<project>/.dotzuki-save.json`.
    pub save_file: Option<PathBuf>,
    /// Deterministic battle rng: when set, every battle draws from this byte
    /// script (cycling) instead of a seeded PRNG. Test/CI hook.
    pub rng_script: Option<Vec<u8>>,
    /// Write saves at stable points (warp/scene completion). The CLI sets
    /// this for windowed runs; headless runs only with an explicit opt-in
    /// (`--save`), keeping CI side-effect-free. Loading is independent — a
    /// valid save always resumes unless `fresh`/`map` say otherwise.
    pub write_saves: bool,
}

/// Live textbox state: pages waiting on A presses. `engine` is `Some` for a
/// scene-driven text (A on the last page resolves the `showText` promise);
/// `None` for a one-off line (raw NPC `talk` text), which just closes.
struct TextState {
    engine: Option<ScriptEngine>,
    pages: VecDeque<String>,
}

/// Live choice state: the scene is paused on `await game.showChoice([...])`;
/// A resumes it with `signal_done(Number(cursor))`.
struct ChoiceState {
    engine: ScriptEngine,
    options: Vec<String>,
    cursor: usize,
    /// The text page that preceded the choice (redrawn beneath the menu).
    context_text: String,
}

/// Live delay state: the scene resumes after `frames_left` frames.
struct DelayState {
    engine: ScriptEngine,
    frames_left: u16,
}

/// Live battle state: the battle itself plus the scene engine suspended on
/// `await startBattle(...)` — `None` for a sceneless battle (a random
/// encounter armed by walking; nothing to resume). When a scene battle
/// resolves, the engine resumes with `signal_done(Text("win"|"lose"|"run"))`
/// so the scene's own JS branches on the result (the wuxia pattern); a
/// sceneless battle returns straight to the overworld (or the whiteout).
struct BattleState {
    engine: Option<ScriptEngine>,
    battle: Battle,
}

/// Runtime mode — what currently owns input.
enum Mode {
    /// Free overworld movement.
    Overworld,
    /// A textbox is on screen.
    Text(TextState),
    /// A choice menu is on screen.
    Choice(ChoiceState),
    /// A scene-imposed delay is counting down.
    Delay(DelayState),
    /// A battle is running (the scene is suspended). Boxed: the battle is
    /// by far the largest mode payload.
    Battle(Box<BattleState>),
    /// The overworld Start menu (party view / bag / save) is open; the
    /// overworld is frozen underneath.
    Menu(MenuState),
    /// A scene-opened shop (`openShop`) is open; the scene is suspended.
    Shop(Box<ShopState>),
    /// The game-over whiteout after a lost battle: blackout + message, then
    /// the party heals and the player returns to the entry map's spawn.
    Whiteout(WhiteoutState),
    /// Map-less project whose entry scene has finished; nothing to do.
    Idle,
}

/// Fade phase of an overworld warp transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FadePhase {
    Out,
    In,
}

/// An in-progress overworld warp: fade out, switch maps, fade in.
struct WarpTransition {
    dest_map: String,
    dest_x: i32,
    dest_y: i32,
    phase: FadePhase,
    frames: u32,
}

impl WarpTransition {
    fn new(dest_map: String, dest_x: i32, dest_y: i32) -> Self {
        Self {
            dest_map,
            dest_x,
            dest_y,
            phase: FadePhase::Out,
            frames: FADE_FRAMES,
        }
    }
}

/// Walkability view the [`OverworldActor`] queries: map collision unioned
/// with the current NPC tiles (the player stops in front of NPCs).
struct CollisionView<'a> {
    map: &'a RuntimeMap,
}

impl CollisionView<'_> {
    /// Any NPC standing on `(x, y)`?
    fn npc_blocks(&self, x: i32, y: i32) -> bool {
        self.map.objects().npcs.iter().any(|n| (n.x, n.y) == (x, y))
    }
}

impl OverworldCollision for CollisionView<'_> {
    fn is_blocked(&self, x: i32, y: i32) -> bool {
        self.map.is_blocked(x, y) || self.npc_blocks(x, y)
    }

    fn is_blocked_at(&self, level: u8, x: i32, y: i32) -> bool {
        // NPCs block their tile at every elevation level (simplest rule: a
        // person is in the way regardless of the player's height).
        self.map.is_blocked_at(level, x, y) || self.npc_blocks(x, y)
    }
}

/// A booted zero-Rust game: overworld + scene VM + dialogue/choice UI.
///
/// Owns the [`LoadedProject`], the current [`RuntimeMap`], the player
/// [`OverworldActor`], the persistent flag store (seeded into / harvested
/// from each short-lived scene engine) and the mode state machine. Drive it
/// with [`update`](Self::update) + [`draw`](Self::draw) — directly (headless)
/// or via the `dotzuki_app::GameLoop` impl (windowed).
pub struct RunnerGame {
    project: LoadedProject,
    /// The current map; `None` in dialogue-only mode (project without maps).
    map: Option<RuntimeMap>,
    camera: Camera,
    actor: OverworldActor,
    /// `gfx/overworld/player/sheet.png` when the project ships one.
    player_sprite: Option<WalkSprite>,
    /// Persistent story flags (cross-scene truth).
    flags: HashMap<String, bool>,
    lang: String,
    mode: Mode,
    /// In-progress overworld warp fade (owns input while active).
    transition: Option<WarpTransition>,
    /// `on_enter` storylines queued to fire one after another.
    pending_scenes: VecDeque<(String, String)>,
    /// A scene-triggered `WarpTo` defers the destination's opening dispatch
    /// until the scene completes (post-warp text must play out first).
    opening_dispatch_pending: bool,
    /// Name of the scene currently being pumped (for diagnostics).
    active_scene: Option<String>,
    /// The text page most recently shown (context under a choice menu).
    last_text: String,
    /// Cosmetic blackout counter for `FadeScreen` commands.
    flash: u32,
    /// Total frames updated (animation pacing / diagnostics).
    frame_count: u64,
    /// File watcher for `--watch` (`None` when watching is off or the
    /// watcher failed to start — the game runs fine either way).
    #[cfg(all(feature = "watch", not(target_arch = "wasm32")))]
    watcher: Option<ProjectWatcher>,
    /// Changed paths accumulated since the last applied reload batch.
    #[cfg(all(feature = "watch", not(target_arch = "wasm32")))]
    watch_pending: HashSet<PathBuf>,
    /// Frames until the next reload batch may apply (burst coalescing).
    #[cfg(all(feature = "watch", not(target_arch = "wasm32")))]
    watch_cooldown: u32,
    /// Music/SFX playback for scene audio commands; silent when the project
    /// ships no `data/audio/` tracks or no output device is available.
    audio: RunnerAudio,
    /// Where the save file lives (`<project>/.dotzuki-save.json` by default).
    save_path: PathBuf,
    /// Whether stable-state saves are written (see [`RunnerOptions::write_saves`]).
    write_saves: bool,
    /// Deterministic battle rng byte script (see [`RunnerOptions::rng_script`]).
    rng_script: Option<Vec<u8>>,
    /// The persistent party state (v2-b): every party member's current
    /// HP/MP/status, harvested at the end of each battle (win AND lose) and
    /// restored from a save. `None` until the first battle (or a save
    /// carrying it) — the next battle then starts from the records.
    party_state: Option<Vec<PartyMemberState>>,
    /// The persistent battle inventory (v2-b), same lifecycle as
    /// `party_state`; `None` ⇒ the manifest's `items.starting` counts.
    inventory: Option<HashMap<String, u32>>,
    /// The player's money (v3): initialized from the manifest's
    /// `shop.startMoney` (default 100) on a fresh boot, carried in the save.
    money: u32,
    /// A lost battle arms the game-over whiteout, triggered when the scene
    /// that received `"lose"` finishes into the overworld/idle (its post-lose
    /// text plays first). Cleared when a battle is won instead.
    pending_whiteout: bool,
    /// The weather a scene armed with `setWeather` (v2-e): a `kind: Weather`
    /// RON record id handed to the NEXT battle (battle-local — cleared when
    /// that battle ends, never saved). `clearWeather` resets it to `None`.
    pending_weather: Option<String>,
    /// The overworld's own entropy source for random-encounter rolls,
    /// seeded lazily on the first roll (a scripted stream when
    /// [`RunnerOptions::rng_script`] is set). Separate from the per-battle
    /// rng so step rolls can't perturb battle determinism.
    overworld_rng: Option<Box<dyn BattleRng>>,
    /// Completed walk steps since boot; mixed into the wasm32 rng seed (the
    /// frame counter alone is 0 at boot there).
    steps_taken: u64,
}

impl RunnerGame {
    /// Boot the project: load the entry map (`opts.map` override or
    /// `game.entryMap`), spawn the player, and run the map's opening
    /// dispatch. A project with **no maps** boots dialogue-only: the entry
    /// scene's `main` storyline runs to completion, then the game idles.
    ///
    /// # Errors
    ///
    /// Fails when the entry map (or `--map` override) cannot be loaded, or
    /// when a map-less project has no entry scene.
    pub fn new(project: LoadedProject, opts: RunnerOptions) -> Result<Self> {
        let lang = if opts.lang.is_empty() {
            "en".to_string()
        } else {
            opts.lang
        };
        let gfx_rel = project.gfx_root_rel();
        let player_sprite = {
            let rel = join_path(&gfx_rel, "overworld/player/sheet.png");
            match project.files().read(&rel) {
                Ok(bytes) => decode_walk_sheet(&bytes, &rel, 24, 32)
                    .map_err(|e| log::warn!("player sprite {rel}: {e}"))
                    .ok(),
                Err(_) => None,
            }
        };

        let mut camera = Camera::new(SCREEN_W as f32, SCREEN_H as f32);
        camera.smooth_factor = 0.0; // locked to the player

        #[cfg(all(feature = "watch", not(target_arch = "wasm32")))]
        let watcher = if opts.watch {
            ProjectWatcher::new(&watch_dirs(&project))
                .map_err(|e| log::warn!("hot-reload disabled: {e}"))
                .ok()
        } else {
            None
        };
        let mut audio = RunnerAudio::from_files(
            project.files().as_ref(),
            project.data_root_rel(),
            !opts.headless,
        );
        if opts.pcm_audio {
            audio.set_pcm_render(true);
        }
        let save_path = opts
            .save_file
            .clone()
            .unwrap_or_else(|| project.root().join(DEFAULT_SAVE_FILE));

        let start_money = project
            .manifest()
            .shop
            .as_ref()
            .map(|s| s.start_money)
            .unwrap_or(DEFAULT_START_MONEY);

        let mut game = Self {
            project,
            map: None,
            camera,
            actor: OverworldActor::new(0, 0, 16),
            player_sprite,
            flags: HashMap::new(),
            lang,
            mode: Mode::Idle,
            transition: None,
            pending_scenes: VecDeque::new(),
            opening_dispatch_pending: false,
            active_scene: None,
            last_text: String::new(),
            flash: 0,
            frame_count: 0,
            #[cfg(all(feature = "watch", not(target_arch = "wasm32")))]
            watcher,
            #[cfg(all(feature = "watch", not(target_arch = "wasm32")))]
            watch_pending: HashSet::new(),
            #[cfg(all(feature = "watch", not(target_arch = "wasm32")))]
            watch_cooldown: 0,
            audio,
            save_path,
            write_saves: opts.write_saves,
            rng_script: opts.rng_script.clone(),
            party_state: None,
            inventory: None,
            money: start_money,
            pending_whiteout: false,
            pending_weather: None,
            overworld_rng: None,
            steps_taken: 0,
        };

        // Resume from a valid save unless `--fresh` or `--map` says
        // otherwise. A corrupt/incompatible save logs and falls through to
        // the normal boot. Disk saves are native-only; the WASM shell
        // restores its localStorage save via `import_save` after boot.
        #[cfg(not(target_arch = "wasm32"))]
        if !opts.fresh && opts.map.is_none() {
            if let Some(save) = GameSave::load(&game.save_path) {
                if game.resume_from(save) {
                    return Ok(game);
                }
            }
        }

        let map_ids = game.project.map_ids();
        if map_ids.is_empty() {
            // Dialogue-only boot: run the entry scene's `main` to completion.
            let scene = game.project.entry_scene_name()?.to_string();
            log::info!("dotzuki-runner: no maps; booting dialogue-only scene '{scene}'");
            if !game.activate(&scene, "main") {
                log::warn!("entry scene '{scene}' has no playable 'main' storyline");
            }
            return Ok(game);
        }

        let map_id = match &opts.map {
            Some(id) => id.clone(),
            None => game.project.entry_map()?,
        };
        game.boot_map(&map_id)
            .with_context(|| format!("failed to load entry map '{map_id}'"))?;
        game.dispatch_opening();
        Ok(game)
    }

    /// Load `map_id` as the first map, spawning at its centre.
    fn boot_map(&mut self, map_id: &str) -> Result<()> {
        let map = self.project.load_map(map_id)?;
        let spawn = find_spawn(&map);
        let tile = map.tile_size().0 as i32;
        self.camera.clamp_to_bounds(Rect::new(
            0.0,
            0.0,
            map.pixel_width() as f32,
            map.pixel_height() as f32,
        ));
        self.actor = OverworldActor::new(spawn.0, spawn.1, tile);
        self.map = Some(map);
        self.center_camera();
        self.camera.update(0.0);
        log::info!("loaded map {map_id} @ ({},{})", spawn.0, spawn.1);
        Ok(())
    }

    /// Switch to another map, placing the player at `spawn` facing `facing`.
    /// Returns `false` (leaving the current map untouched) when the
    /// destination can't be loaded, so a bad warp aborts gracefully.
    #[must_use]
    fn enter_map(&mut self, map_id: &str, spawn: (i32, i32), facing: Direction) -> bool {
        let map = match self.project.load_map(map_id) {
            Ok(m) => m,
            Err(e) => {
                log::error!("warp to {map_id} aborted: {e:#}");
                return false;
            }
        };
        let spawn = if map.is_blocked(spawn.0, spawn.1)
            || map.objects().npcs.iter().any(|n| (n.x, n.y) == spawn)
        {
            let fallback = find_spawn(&map);
            log::warn!(
                "spawn ({},{}) on {map_id} is occupied; using ({},{})",
                spawn.0,
                spawn.1,
                fallback.0,
                fallback.1
            );
            fallback
        } else {
            spawn
        };
        self.camera.clamp_to_bounds(Rect::new(
            0.0,
            0.0,
            map.pixel_width() as f32,
            map.pixel_height() as f32,
        ));
        self.map = Some(map);
        self.actor.place(spawn.0, spawn.1, facing);
        // Warps land on the ground level (stairs are the only way up).
        self.actor.set_elevation(0);
        self.center_camera();
        self.camera.update(0.0);
        log::info!("loaded map {map_id} @ ({},{})", spawn.0, spawn.1);
        true
    }

    // ── save/load ───────────────────────────────────────────────────────────

    /// Resume a save: seed the persistent flags, load its map and place the
    /// player at the saved tile (falling back to the spawn scan when that
    /// tile has become occupied). The opening dispatch is **skipped** on
    /// resume — the player is continuing, not entering; the restored
    /// `__played_main_*` flags keep `main` from replaying on later entries.
    ///
    /// Flags are restored from any valid save (they are the only resumable
    /// state of a dialogue-only project). Returns `false` — fresh boot —
    /// when the saved map can't be loaded.
    fn resume_from(&mut self, save: GameSave) -> bool {
        self.flags.extend(save.flags);
        // v2 fields: a v1 save (or a v2 save that never battled) has neither —
        // the first battle then starts from the records / starting counts.
        if let Some(party) = save.party {
            self.party_state = Some(
                party
                    .into_iter()
                    .map(|m| PartyMemberState {
                        id: m.id,
                        hp: m.hp,
                        mp: m.mp,
                        status: m.status,
                        level: m.level,
                        exp: m.exp,
                    })
                    .collect(),
            );
        }
        if save.inventory.is_some() {
            self.inventory = save.inventory;
        }
        if let Some(money) = save.money {
            self.money = money;
        }
        let Some(map_id) = &save.map else {
            log::info!("save: restored flags (dialogue-only project)");
            return false;
        };
        let map = match self.project.load_map(map_id) {
            Ok(m) => m,
            Err(e) => {
                log::warn!("save: saved map '{map_id}' failed to load ({e:#}) — starting fresh");
                return false;
            }
        };
        let (x, y) = (save.player.x, save.player.y);
        let facing = parse_facing(&save.player.facing);
        let spawn =
            if map.is_blocked(x, y) || map.objects().npcs.iter().any(|n| (n.x, n.y) == (x, y)) {
                let fallback = find_spawn(&map);
                log::warn!(
                    "save: position ({x},{y}) on {map_id} is occupied; using ({},{})",
                    fallback.0,
                    fallback.1
                );
                fallback
            } else {
                (x, y)
            };
        self.camera.clamp_to_bounds(Rect::new(
            0.0,
            0.0,
            map.pixel_width() as f32,
            map.pixel_height() as f32,
        ));
        self.map = Some(map);
        self.actor.place(spawn.0, spawn.1, facing);
        // Multi-level maps: restore the saved elevation, clamped to the map.
        let max_level = self.map.as_ref().map(|m| m.level_count() - 1).unwrap_or(0) as u8;
        self.actor.set_elevation(save.player.level.min(max_level));
        self.center_camera();
        self.camera.update(0.0);
        self.mode = Mode::Overworld;
        log::info!("save: resumed {map_id} @ ({},{})", spawn.0, spawn.1);
        true
    }

    /// Write the current state to the save file. Called only from **stable**
    /// states — after a completed warp transition and when a scene finishes
    /// into the overworld/idle — never mid-scene or mid-warp (a suspended
    /// scene engine can't be resumed). Closing the window mid-dialogue
    /// therefore keeps the save from the last stable point. No-op when
    /// [`RunnerOptions::write_saves`] is off.
    fn write_save(&self) {
        if !self.write_saves {
            return;
        }
        self.write_save_now();
    }

    /// Write the current state to the save file, unconditionally. The Start
    /// menu's Save entry uses this — saving from the menu is always allowed,
    /// even where automatic stable-state saves are off (headless runs).
    /// No-op on WASM (no disk; the shell persists [`export_save`] output).
    ///
    /// [`export_save`]: Self::export_save
    pub(crate) fn write_save_now(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let save = self.current_save();
            match save.write(&self.save_path) {
                Ok(()) => log::info!("save: wrote {}", self.save_path.display()),
                Err(e) => {
                    log::warn!("save: write to {} failed: {e:#}", self.save_path.display())
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = &self.save_path; // disk saves are native-only
        }
    }

    /// The current state as a [`GameSave`] (map, player tile/facing, flags,
    /// language, party, inventory, money).
    fn current_save(&self) -> GameSave {
        let (x, y) = self.actor.tile();
        GameSave {
            version: SAVE_VERSION,
            map: self.current_map_id().map(str::to_string),
            player: PlayerSave {
                x,
                y,
                facing: facing_name(self.actor.facing()).to_string(),
                level: self.actor.elevation(),
            },
            flags: self.flags.clone(),
            lang: Some(self.lang.clone()),
            party: self.party_state.as_ref().map(|party| {
                party
                    .iter()
                    .map(|m| PartyMemberSave {
                        id: m.id.clone(),
                        hp: m.hp,
                        mp: m.mp,
                        status: m.status.clone(),
                        level: m.level,
                        exp: m.exp,
                    })
                    .collect()
            }),
            inventory: self.inventory.clone(),
            money: Some(self.money),
        }
    }

    /// Serialize the current state as save JSON — the persistence bridge for
    /// the WASM shell (localStorage). Returns `None` while the game is in a
    /// transient state that cannot round-trip (a scene engine suspended on
    /// text/choice/delay/battle/shop, or a warp transition mid-flight);
    /// stable states (overworld, menu, whiteout, idle) always export.
    pub fn export_save(&self) -> Option<String> {
        if self.transition.is_some()
            || matches!(
                self.mode,
                Mode::Text(_) | Mode::Choice(_) | Mode::Delay(_) | Mode::Battle(_) | Mode::Shop(_)
            )
        {
            return None;
        }
        Some(self.current_save().to_json())
    }

    /// Restore a save produced by [`export_save`](Self::export_save) (the
    /// WASM shell's localStorage bridge). Returns `false` — the game keeps
    /// its current state — on unparseable JSON, a NEWER save version, or a
    /// saved map that no longer loads.
    pub fn import_save(&mut self, json: &str) -> bool {
        let save = match GameSave::from_json(json) {
            Ok(save) => save,
            Err(e) => {
                log::warn!("save: import failed ({e:#}) — keeping current state");
                return false;
            }
        };
        if save.version > SAVE_VERSION {
            log::warn!(
                "save: imported save is version {} (newer than {SAVE_VERSION}) — keeping current state",
                save.version
            );
            return false;
        }
        self.resume_from(save)
    }

    // ── scene dispatch ──────────────────────────────────────────────────────

    /// Fire the current map's opening scene(s): `on_enter` routes first (all,
    /// sequentially), then `<SceneName>OnLoad`, then a once-only `main`.
    fn dispatch_opening(&mut self) {
        let Some(map) = &self.map else {
            return;
        };
        let map_id = map.id().to_string();

        let on_enters: Vec<(String, String)> = self
            .project
            .routes()
            .iter()
            .filter(|r| r.map == map_id && r.on_enter)
            .filter_map(|r| {
                self.scene_with_storyline(&map_id, &r.storyline)
                    .map(|scene| (scene, r.storyline.clone()))
            })
            .collect();
        if !on_enters.is_empty() {
            self.pending_scenes.extend(on_enters);
            self.pop_pending_scene();
            return;
        }

        let Some(scene) = self.scene_for_map(&map_id) else {
            return;
        };
        let onload = format!("{scene}OnLoad");
        if scene_has_fn(&self.project, &scene, &onload) {
            self.activate(&scene, &onload);
            return;
        }
        let played_flag = format!("__played_main_{map_id}");
        if !self.flags.get(&played_flag).copied().unwrap_or(false)
            && scene_has_fn(&self.project, &scene, "main")
        {
            // Set before playing so a failing scene can't retrigger every entry.
            self.flags.insert(played_flag, true);
            self.activate(&scene, "main");
        }
    }

    /// The compiled scene belonging to a map: the scene whose source file is
    /// `<maps_dir>/<map>/script.scene`, else a scene named like the map.
    fn scene_for_map(&self, map_id: &str) -> Option<String> {
        for (name, _js, source_path) in &self.project.report().scenes {
            let path = Path::new(source_path);
            if path.file_stem().is_some_and(|s| s == "script")
                && path
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|d| d == map_id)
            {
                return Some(name.clone());
            }
        }
        if self.project.scripts().has_script(map_id) {
            return Some(map_id.to_string());
        }
        None
    }

    /// The scene exporting a storyline: the current map's scene first, then
    /// any compiled scene (matched on the generated export names).
    fn scene_with_storyline(&self, map_id: &str, storyline: &str) -> Option<String> {
        if let Some(scene) = self.scene_for_map(map_id) {
            if scene_has_fn(&self.project, &scene, storyline) {
                return Some(scene);
            }
        }
        self.project
            .report()
            .scenes
            .iter()
            .map(|(name, _, _)| name)
            .find(|name| scene_has_fn(&self.project, name, storyline))
            .cloned()
    }

    /// Activate `storyline` from `scene` on a fresh [`ScriptEngine`] seeded
    /// with the persistent flags, language and player position. Returns
    /// `false` when the scene/function is missing or fails to start.
    fn activate(&mut self, scene: &str, storyline: &str) -> bool {
        let Some(js) = self.project.scripts().get_script(scene) else {
            log::warn!("scene '{scene}' not registered");
            return false;
        };
        let js = js.to_string();
        let mut engine = ScriptEngine::new();
        // Battle commands are not part of the engine's core `game.*` set —
        // register them locally (the wuxia pattern) so `@command("startBattle", …)`
        // / `result = startBattle(…)` yield ScriptCommands the pump can arm.
        engine.register_async_fn("startBattle", |args, ctx| {
            let trainer_id = match args.first() {
                Some(v) => v.to_string(ctx)?.to_std_string_lossy(),
                None => String::new(),
            };
            Ok(ScriptCommand::StartBattle { trainer_id })
        });
        engine.register_async_fn("startWildBattle", |args, ctx| {
            let species = match args.first() {
                Some(v) => v.to_string(ctx)?.to_std_string_lossy(),
                None => String::new(),
            };
            let level = match args.get(1) {
                Some(v) => v.to_number(ctx)? as u8,
                None => 1,
            };
            Ok(ScriptCommand::StartWildBattle { species, level })
        });
        // Weather is runner-local too (the wuxia pattern): `setWeather(id)`
        // arms a `kind: Weather` rules.ron record for the NEXT battle,
        // `clearWeather()` cancels a previously armed one.
        engine.register_async_fn("setWeather", |args, ctx| {
            let weather = match args.first() {
                Some(v) => v.to_string(ctx)?.to_std_string_lossy(),
                None => String::new(),
            };
            Ok(ScriptCommand::SetWeather {
                weather: Some(weather),
            })
        });
        engine.register_async_fn("clearWeather", |_args, _ctx| {
            Ok(ScriptCommand::SetWeather { weather: None })
        });
        engine.seed_flags(&self.flags);
        engine.set_lang(&self.lang);
        let (px, py) = self.actor.tile();
        engine.set_player_position(px.clamp(0, 255) as u8, py.clamp(0, 255) as u8);
        if let Err(e) = engine.load_script(&js) {
            log::warn!("load scene '{scene}': {e}");
            return false;
        }
        if !engine.has_function(storyline) {
            log::warn!("scene '{scene}' has no function '{storyline}'");
            return false;
        }
        log::info!("activate {scene}::{storyline}");
        match engine.call_function_no_args(storyline) {
            Ok(cmd) => {
                self.active_scene = Some(scene.to_string());
                self.pump(engine, cmd, true);
                true
            }
            Err(e) => {
                log::warn!("run {scene}::{storyline}: {e}");
                false
            }
        }
    }

    /// Pop the next queued `on_enter` storyline and activate it. Returns
    /// `false` when the queue is drained (or every activation failed).
    fn pop_pending_scene(&mut self) -> bool {
        while let Some((scene, storyline)) = self.pending_scenes.pop_front() {
            if self.activate(&scene, &storyline) {
                return true;
            }
        }
        false
    }

    /// The scene VM: drive the command stream from `cmd` onward, owning
    /// `engine`. Suspends (storing the engine in [`Mode`]) on commands that
    /// need UI or time; transparently resolves side-effect commands; ends the
    /// scene on stream end or error. `first` marks the initial command of a
    /// fresh activation — an immediate `None` there means the scene produced
    /// nothing at all (typically an unregistered `game.*` call killed it).
    fn pump(&mut self, mut engine: ScriptEngine, mut cmd: Option<ScriptCommand>, first: bool) {
        let scene = self.active_scene.clone().unwrap_or_default();
        let mut first = first;
        loop {
            match cmd {
                Some(ScriptCommand::ShowText { text }) => {
                    self.last_text = text.clone();
                    self.mode = Mode::Text(TextState {
                        engine: Some(engine),
                        pages: paginate(&text),
                    });
                    return;
                }
                Some(ScriptCommand::ShowChoice { options }) => {
                    self.mode = Mode::Choice(ChoiceState {
                        engine,
                        options,
                        cursor: 0,
                        context_text: self.last_text.clone(),
                    });
                    return;
                }
                Some(ScriptCommand::Delay { frames }) => {
                    self.mode = Mode::Delay(DelayState {
                        engine,
                        frames_left: frames,
                    });
                    return;
                }
                Some(ScriptCommand::WarpTo { map, x, y }) => {
                    log::info!("scene warp → {map} ({x},{y})");
                    // On a bad dest, enter_map logs and leaves us put; resume
                    // the scene regardless so it can't deadlock on the warp.
                    if self.enter_map(&map, (x as i32, y as i32), Direction::Down) {
                        self.opening_dispatch_pending = true;
                    }
                    cmd = self.signal(&mut engine, CommandResult::Void);
                }
                Some(ScriptCommand::FadeScreen { fade_type }) => {
                    log::info!("fadeScreen({fade_type}) (cosmetic in dotzuki run v1)");
                    self.flash = FLASH_FRAMES;
                    cmd = self.signal(&mut engine, CommandResult::Void);
                }
                Some(ScriptCommand::SetFlag { flag }) => {
                    engine.set_flag(&flag, true);
                    self.flags.insert(flag, true);
                    cmd = self.signal(&mut engine, CommandResult::Void);
                }
                Some(ScriptCommand::ResetFlag { flag }) => {
                    engine.set_flag(&flag, false);
                    self.flags.insert(flag, false);
                    cmd = self.signal(&mut engine, CommandResult::Void);
                }
                Some(ScriptCommand::CheckFlag { flag }) => {
                    let value = self.flags.get(&flag).copied().unwrap_or(false);
                    cmd = self.signal(&mut engine, CommandResult::Bool(value));
                }
                Some(ScriptCommand::PlayMusic { music_id }) => {
                    self.audio.play_music(&music_id);
                    cmd = self.signal(&mut engine, CommandResult::Void);
                }
                Some(ScriptCommand::PlaySound { sound_id }) => {
                    self.audio.play_sound(&sound_id);
                    cmd = self.signal(&mut engine, CommandResult::Void);
                }
                Some(ScriptCommand::StopMusic) => {
                    self.audio.stop_music();
                    cmd = self.signal(&mut engine, CommandResult::Void);
                }
                Some(ScriptCommand::FadeOutMusic) => {
                    self.audio.fade_out_music();
                    cmd = self.signal(&mut engine, CommandResult::Void);
                }
                Some(ScriptCommand::StartBattle { trainer_id }) => {
                    log::info!("scene → battle: startBattle({trainer_id})");
                    match self.build_battle(&trainer_id) {
                        Some(battle) => {
                            // Suspend the scene; Mode::Battle resumes it with
                            // the outcome when the battle ends.
                            self.mode = Mode::Battle(Box::new(BattleState {
                                engine: Some(engine),
                                battle,
                            }));
                            return;
                        }
                        None => {
                            cmd = self.signal(&mut engine, CommandResult::Text("win".to_string()));
                        }
                    }
                }
                Some(ScriptCommand::StartWildBattle { species, level }) => {
                    log::info!("scene → battle: startWildBattle({species}, lv{level}) — level ignored in v1");
                    match self.build_battle(&species) {
                        Some(battle) => {
                            self.mode = Mode::Battle(Box::new(BattleState {
                                engine: Some(engine),
                                battle,
                            }));
                            return;
                        }
                        None => {
                            cmd = self.signal(&mut engine, CommandResult::Text("win".to_string()));
                        }
                    }
                }
                Some(ScriptCommand::SetWeather { weather }) => {
                    log::info!("scene → weather: {weather:?} (applies to the next battle)");
                    self.pending_weather = weather;
                    cmd = self.signal(&mut engine, CommandResult::Void);
                }
                Some(ScriptCommand::OpenShop { items }) => {
                    log::info!("scene → shop: openShop({items:?})");
                    // Suspend the scene; Mode::Shop resumes it with Void on
                    // exit. Buy + Sell (see game::menu).
                    let items = self.build_shop_items(&items);
                    self.mode = Mode::Shop(Box::new(ShopState::new(engine, items)));
                    return;
                }
                Some(other) => {
                    // An unimplemented command must never deadlock the VM.
                    log::warn!(
                        "unhandled scene command {other:?} in scene '{scene}' — \
                         auto-completing with Void"
                    );
                    cmd = self.signal(&mut engine, CommandResult::Void);
                }
                None => {
                    if first {
                        log::warn!(
                            "scene '{scene}' finished without producing any command — \
                             did it call an unregistered game.* function?"
                        );
                    }
                    self.finish_scene(engine);
                    return;
                }
            }
            first = false;
        }
    }

    /// `signal_done` wrapper translating errors into a stream end.
    fn signal(
        &mut self,
        engine: &mut ScriptEngine,
        result: CommandResult,
    ) -> Option<ScriptCommand> {
        match engine.signal_done(result) {
            Ok(cmd) => cmd,
            Err(e) => {
                let scene = self.active_scene.clone().unwrap_or_default();
                log::warn!("scene '{scene}' signal failed: {e}");
                None
            }
        }
    }

    /// Build a battle against enemy record `enemy_id`. Returns `None` — the
    /// scene continues with `"win"` (undefeated-continue) — when the project
    /// has no `battle` section; a broken section (unknown table ids, bad
    /// records, unparseable rules) logs a clear error and also yields `None`,
    /// so a misconfigured battle can never deadlock the scene VM.
    fn build_battle(&mut self, enemy_id: &str) -> Option<Battle> {
        if self.project.manifest().battle.is_none() {
            log::warn!(
                "startBattle({enemy_id}): project has no battle section — \
                 auto-completing with \"win\""
            );
            return None;
        }
        let rng: Box<dyn BattleRng> = match &self.rng_script {
            Some(bytes) => Box::new(ScriptedRng::new(bytes.clone())),
            None => {
                // SystemTime::now() panics on wasm32-unknown-unknown; there
                // the seed is pure-Rust entropy from the frame counter.
                #[cfg(not(target_arch = "wasm32"))]
                let seed = {
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.subsec_nanos() as u64)
                        .unwrap_or(0);
                    nanos ^ self.frame_count
                };
                #[cfg(target_arch = "wasm32")]
                let seed =
                    self.frame_count.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xA076_1D64_78BD_642F;
                Box::new(XorshiftRng::seed(seed))
            }
        };
        match BattleSetup::from_project(&self.project).and_then(|setup| {
            setup.start_with(
                enemy_id,
                rng,
                self.party_state.as_deref(),
                self.inventory.as_ref(),
            )
        }) {
            Ok(mut battle) => {
                battle.set_lang(&self.lang);
                battle.set_currency(self.currency());
                // v2-e: hand the scene-armed weather to the battle (it stays
                // pending until the battle ends — a failed build keeps it for
                // the next startBattle), then run the battle-start hook pass
                // (weather intro + both actives' ability SwitchIn hooks).
                battle.set_weather(self.pending_weather.clone());
                battle.begin();
                Some(battle)
            }
            Err(e) => {
                log::error!("startBattle({enemy_id}): battle setup failed: {e:#}");
                None
            }
        }
    }

    /// End the active scene: harvest flags into the persistent store, return
    /// to the overworld (or idle), then continue with any queued `on_enter`
    /// storyline or a deferred opening dispatch (scene `WarpTo`). A lost
    /// battle's whiteout takes precedence over both — the queued scenes
    /// belong to a map the player is about to leave.
    fn finish_scene(&mut self, engine: ScriptEngine) {
        self.flags.extend(engine.get_all_flags());
        drop(engine);
        self.active_scene = None;
        self.mode = if self.map.is_some() {
            Mode::Overworld
        } else {
            Mode::Idle
        };
        if self.pending_whiteout {
            self.pending_whiteout = false;
            self.pending_scenes.clear();
            self.opening_dispatch_pending = false;
            self.start_whiteout();
            return;
        }
        if self.pop_pending_scene() {
            return;
        }
        if self.opening_dispatch_pending {
            self.opening_dispatch_pending = false;
            self.dispatch_opening();
        }
        // Scene over and settled (no queued/deferred scene took over): the
        // flags it set are safe to persist.
        if matches!(self.mode, Mode::Overworld | Mode::Idle) {
            self.write_save();
        }
    }

    // ── hot reload (`--watch`) ─────────────────────────────────────────────

    /// Poll the file watcher and apply pending changes as one batch.
    ///
    /// Called at the top of every [`update`](Self::update); a no-op when
    /// watching is off. Batches are debounced by [`WATCH_DEBOUNCE_FRAMES`]
    /// so an editor save burst applies once.
    #[cfg(all(feature = "watch", not(target_arch = "wasm32")))]
    pub fn poll_watch(&mut self) {
        let Some(watcher) = &mut self.watcher else {
            return;
        };
        self.watch_pending.extend(watcher.poll_events());
        if self.watch_cooldown > 0 {
            self.watch_cooldown -= 1;
            return;
        }
        if self.watch_pending.is_empty() {
            return;
        }
        let paths: Vec<PathBuf> = self.watch_pending.drain().collect();
        self.watch_cooldown = WATCH_DEBOUNCE_FRAMES;
        self.apply_watch_batch(paths);
    }

    /// No-op without the `watch` feature (and on WASM, where hot reload is
    /// unsupported).
    #[cfg(any(not(feature = "watch"), target_arch = "wasm32"))]
    pub fn poll_watch(&mut self) {}

    /// Classify a batch of changed paths and reload accordingly:
    ///
    /// - any `.scene` → recompile every DSL dir and swap scenes in place;
    /// - a content file under the **current** map dir (`map.tmx.json`,
    ///   `tileset.png`, objects sidecar) → reload that [`RuntimeMap`];
    /// - anything else (other maps, data tables, gfx) is ignored — it is
    ///   picked up on next map enter / next boot.
    #[cfg(all(feature = "watch", not(target_arch = "wasm32")))]
    fn apply_watch_batch(&mut self, paths: Vec<PathBuf>) {
        let map_dir = self
            .map
            .as_ref()
            .map(|m| crate::map::map_dir(&self.project.maps_dir(), m.id()));
        let mut scenes_changed = false;
        let mut map_changed = false;
        for path in &paths {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "scene" {
                scenes_changed = true;
            } else if map_dir
                .as_ref()
                .is_some_and(|d| path.parent() == Some(d.as_path()))
            {
                map_changed = true;
            }
        }
        if scenes_changed {
            self.reload_scenes();
        }
        if map_changed {
            self.reload_current_map();
        }
    }

    /// Recompile the project's DSL and swap the compiled scenes in place.
    /// A scene mid-activation keeps running its old engine; the next
    /// activation (talk/enter) picks up the new source. On a compiler
    /// diagnostic the old scenes keep running (`false`).
    pub fn reload_scenes(&mut self) -> bool {
        match self.project.recompile_scripts() {
            Ok(()) => {
                log::info!("hot-reload: scenes recompiled");
                true
            }
            Err(e) => {
                log::warn!("hot-reload: scene reload failed, keeping old scenes: {e:#}");
                false
            }
        }
    }

    /// Reload the current map from disk in place, preserving the player's
    /// pixel position and the story flags. On a load error the old map is
    /// kept (`false`); `false` also when there is no current map.
    pub fn reload_current_map(&mut self) -> bool {
        let Some(map) = &self.map else {
            return false;
        };
        let map_id = map.id().to_string();
        match self.project.load_map(&map_id) {
            Ok(new_map) => {
                self.camera.clamp_to_bounds(Rect::new(
                    0.0,
                    0.0,
                    new_map.pixel_width() as f32,
                    new_map.pixel_height() as f32,
                ));
                self.map = Some(new_map);
                self.center_camera();
                self.camera.update(0.0);
                log::info!("hot-reload: reloaded map '{map_id}'");
                true
            }
            Err(e) => {
                log::warn!("hot-reload: map '{map_id}' reload failed, keeping old map: {e:#}");
                false
            }
        }
    }

    // ── per-frame update ────────────────────────────────────────────────────

    /// Advance the game one frame. Callable without any window (headless
    /// tests, the `run_headless` driver); the `GameLoop` impl forwards here.
    pub fn update(&mut self, input: &InputState) {
        self.frame_count += 1;
        self.poll_watch();
        self.audio.update_frame();
        if self.flash > 0 {
            self.flash -= 1;
        }

        // A warp fade owns the frame: input is frozen while the map switches.
        if self.transition.is_some() {
            self.update_transition();
            return;
        }

        match std::mem::replace(&mut self.mode, Mode::Overworld) {
            Mode::Text(mut state) => {
                if input.is_just_pressed(GbButton::A) {
                    state.pages.pop_front();
                    if state.pages.is_empty() {
                        match state.engine {
                            Some(mut engine) => {
                                let cmd = self.signal(&mut engine, CommandResult::Void);
                                self.pump(engine, cmd, false);
                            }
                            None => {
                                // One-off line: just close the box.
                                self.mode = if self.map.is_some() {
                                    Mode::Overworld
                                } else {
                                    Mode::Idle
                                };
                            }
                        }
                    } else {
                        self.mode = Mode::Text(state);
                    }
                } else {
                    self.mode = Mode::Text(state);
                }
            }
            Mode::Choice(mut state) => {
                let n = state.options.len().max(1);
                if input.is_just_pressed(GbButton::Up) {
                    state.cursor = (state.cursor + n - 1) % n;
                } else if input.is_just_pressed(GbButton::Down) {
                    state.cursor = (state.cursor + 1) % n;
                }
                if input.is_just_pressed(GbButton::A) {
                    let cursor = state.cursor;
                    let mut engine = state.engine;
                    log::info!("choice picked: {cursor}");
                    let cmd = self.signal(&mut engine, CommandResult::Number(cursor as f64));
                    self.pump(engine, cmd, false);
                } else {
                    self.mode = Mode::Choice(state);
                }
            }
            Mode::Delay(mut state) => {
                if state.frames_left > 0 {
                    state.frames_left -= 1;
                }
                if state.frames_left == 0 {
                    let mut engine = state.engine;
                    let cmd = self.signal(&mut engine, CommandResult::Void);
                    self.pump(engine, cmd, false);
                } else {
                    self.mode = Mode::Delay(state);
                }
            }
            Mode::Battle(mut state) => {
                state.battle.update(input);
                if let Some(outcome) = state.battle.outcome() {
                    // Battle over: harvest the persistent party state and
                    // inventory (win, lose AND run), then resume the
                    // suspended scene with the result.
                    self.party_state = Some(state.battle.party_state());
                    self.inventory = Some(state.battle.inventory().clone());
                    let result = match outcome {
                        BattleOutcome::Win => "win",
                        BattleOutcome::Lose => "lose",
                        BattleOutcome::Run => "run",
                    };
                    // A trainer win pays the encounter's money reward (v2-d).
                    if outcome == BattleOutcome::Win {
                        self.money = self.money.saturating_add(state.battle.trainer_money());
                    }
                    // A loss arms the whiteout, which fires when the scene
                    // that receives "lose" finishes (its post-lose text
                    // plays first); any other outcome cancels a previously
                    // armed one.
                    self.pending_whiteout = outcome == BattleOutcome::Lose;
                    // The armed weather was battle-local (v2-e): it dies with
                    // the battle and is never saved.
                    self.pending_weather = None;
                    log::info!("battle ended: {result}");
                    match state.engine {
                        // Scene battle: resume the suspended scene with the
                        // outcome (its post-battle text/branches play out;
                        // finish_scene fires an armed whiteout).
                        Some(mut engine) => {
                            let cmd =
                                self.signal(&mut engine, CommandResult::Text(result.to_string()));
                            self.pump(engine, cmd, false);
                        }
                        // Sceneless (a random encounter armed by walking):
                        // win/run returns to the overworld in place; a loss
                        // goes straight to the whiteout — there is no scene
                        // whose post-lose text would play first.
                        None => {
                            if outcome == BattleOutcome::Lose {
                                self.pending_whiteout = false;
                                self.start_whiteout();
                            } else {
                                self.mode = Mode::Overworld;
                            }
                        }
                    }
                } else {
                    self.mode = Mode::Battle(state);
                }
            }
            Mode::Overworld => self.update_overworld(input),
            Mode::Menu(state) => self.update_menu(state, input),
            Mode::Shop(state) => self.update_shop(*state, input),
            Mode::Whiteout(state) => self.update_whiteout(state, input),
            // Restore Idle: the replace above defaults the mode to Overworld,
            // which would silently erase the resting state of map-less
            // projects (and with it the end card).
            Mode::Idle => {
                self.mode = Mode::Idle;
            }
        }

        self.center_camera();
        self.camera.update(0.0);
    }

    /// Fade out → switch map → fade in → opening dispatch.
    fn update_transition(&mut self) {
        let Some(t) = &mut self.transition else {
            return;
        };
        if t.frames > 0 {
            t.frames -= 1;
        }
        if t.frames > 0 {
            return;
        }
        match t.phase {
            FadePhase::Out => {
                let (dest_map, dest) = (t.dest_map.clone(), (t.dest_x, t.dest_y));
                let facing = self.actor.facing();
                if self.enter_map(&dest_map, dest, facing) {
                    if let Some(t) = &mut self.transition {
                        t.phase = FadePhase::In;
                        t.frames = FADE_FRAMES;
                    }
                } else {
                    // Broken warp dest: stay put (logged by enter_map).
                    self.transition = None;
                }
            }
            FadePhase::In => {
                self.transition = None;
                // The warp is complete and the mode is a stable Overworld —
                // save before the opening dispatch can start a scene.
                self.write_save();
                self.dispatch_opening();
            }
        }
    }

    /// Free-roam input: talk on A, walk with the D-pad, warp on arrival.
    /// Start opens the pause menu (party / bag / save).
    fn update_overworld(&mut self, input: &InputState) {
        if self.map.is_none() {
            return;
        }
        if input.is_just_pressed(GbButton::Start) {
            self.mode = Mode::Menu(MenuState::new());
            return;
        }
        if input.is_just_pressed(GbButton::A) {
            if let Some(npc_index) = self.faced_npc_index() {
                self.talk_to(npc_index);
                return;
            }
            if let Some(sign_index) = self.faced_sign_index() {
                self.read_sign(sign_index);
                return;
            }
        }

        let held = held_direction(input);
        self.actor.set_running(input.is_held(GbButton::B));
        let step = {
            let map = self.map.as_ref().expect("checked above");
            let view = CollisionView { map };
            self.actor.update(held, &view)
        };
        if let Some((tx, ty)) = step {
            self.steps_taken += 1;
            // Stairs: a tile is either a stair or a warp in practice.
            self.apply_stairs(tx, ty);
            // Warp takes priority over an encounter roll on the same tile.
            let warp = {
                let map = self.map.as_ref().expect("checked above");
                map.objects()
                    .warps
                    .iter()
                    .find(|w| (w.x, w.y) == (tx, ty) && !w.dest_map.is_empty())
                    .map(|w| (w.dest_map.clone(), w.dest_x, w.dest_y))
            };
            if let Some((dest_map, dest_x, dest_y)) = warp {
                self.transition = Some(WarpTransition::new(dest_map, dest_x, dest_y));
            } else {
                self.roll_encounter(tx, ty);
            }
        }
    }

    /// Roll a random encounter after a completed step onto `(tx, ty)`: when
    /// the tile lies inside one of the map's encounter zones, draw one byte
    /// — a hit (`< rate`, the config's /256 per-step chance) picks a
    /// weighted id from the zone's table and arms a sceneless battle.
    /// Turning in place never completes a step, and battles/scenes don't run
    /// `update_overworld`, so a tile is rolled exactly once per walk onto it.
    fn roll_encounter(&mut self, tx: i32, ty: i32) {
        let (rate, table) = {
            let map = self.map.as_ref().expect("roll_encounter requires a map");
            let Some(encounters) = &map.objects().encounters else {
                return;
            };
            match encounters.zones.iter().find(|z| z.contains(tx, ty)) {
                Some(zone) => (encounters.rate, zone.table.clone()),
                None => return,
            }
        };
        if rate == 0 || table.is_empty() {
            return;
        }
        if self.overworld_rng_byte() >= rate {
            return;
        }
        let total: u32 = table.iter().map(|e| e.weight).sum();
        if total == 0 {
            return;
        }
        let mut pick = u32::from(self.overworld_rng_byte()) % total;
        let id = table
            .iter()
            .find(|e| {
                if pick < e.weight {
                    true
                } else {
                    pick -= e.weight;
                    false
                }
            })
            .map(|e| e.id.clone());
        if let Some(id) = id {
            self.start_overworld_battle(&id);
        }
    }

    /// The next overworld rng byte, seeding the generator on first use: a
    /// scripted stream under [`RunnerOptions::rng_script`] (deterministic
    /// test hook), else a xorshift seeded like the per-battle rng in
    /// [`build_battle`](Self::build_battle) — SystemTime entropy on native;
    /// on wasm32 the frame counter mixed with the step counter (the frame
    /// counter alone is 0 at boot there, and SystemTime panics).
    fn overworld_rng_byte(&mut self) -> u8 {
        if self.overworld_rng.is_none() {
            let rng: Box<dyn BattleRng> = match &self.rng_script {
                Some(bytes) => Box::new(ScriptedRng::new(bytes.clone())),
                None => {
                    #[cfg(not(target_arch = "wasm32"))]
                    let seed = {
                        let nanos = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.subsec_nanos() as u64)
                            .unwrap_or(0);
                        nanos ^ self.frame_count
                    };
                    #[cfg(target_arch = "wasm32")]
                    let seed = self
                        .frame_count
                        .wrapping_add(self.steps_taken)
                        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                        ^ 0xA076_1D64_78BD_642F;
                    Box::new(XorshiftRng::seed(seed))
                }
            };
            self.overworld_rng = Some(rng);
        }
        self.overworld_rng
            .as_deref_mut()
            .expect("seeded above")
            .byte()
    }

    /// Arm a sceneless battle from a random encounter: same construction as
    /// a scene's `startBattle` ([`build_battle`](Self::build_battle) — an
    /// encounter record first, a single enemy record as the fallback), but
    /// no scene engine is suspended, so the outcome flows straight back to
    /// the overworld (win/run) or the whiteout (lose). A failed build logs
    /// (inside `build_battle`) and simply keeps the player walking.
    fn start_overworld_battle(&mut self, id: &str) {
        log::info!("random encounter: {id}");
        if let Some(battle) = self.build_battle(id) {
            self.mode = Mode::Battle(Box::new(BattleState {
                engine: None,
                battle,
            }));
        }
    }

    /// Elevation transition on arrival: stair GID 1 ascends one level, GID 2
    /// descends one (both clamped to the map's level count).
    fn apply_stairs(&mut self, x: i32, y: i32) {
        let Some(map) = &self.map else {
            return;
        };
        let level = self.actor.elevation();
        let next = match map.stair_at(x, y) {
            Some(1) => (level as usize + 1).min(map.level_count() - 1) as u8,
            Some(2) => level.saturating_sub(1),
            _ => return,
        };
        if next != level {
            self.actor.set_elevation(next);
        }
    }

    /// The index of the NPC on the tile the player faces, if any.
    fn faced_npc_index(&self) -> Option<usize> {
        let map = self.map.as_ref()?;
        let (dx, dy) = direction_delta(self.actor.facing());
        let (tx, ty) = self.actor.tile();
        let faced = (tx + dx, ty + dy);
        map.objects().npcs.iter().position(|n| (n.x, n.y) == faced)
    }

    /// The index of the sign on the tile the player faces, if any.
    fn faced_sign_index(&self) -> Option<usize> {
        let map = self.map.as_ref()?;
        let (dx, dy) = direction_delta(self.actor.facing());
        let (tx, ty) = self.actor.tile();
        let faced = (tx + dx, ty + dy);
        map.objects().signs.iter().position(|s| (s.x, s.y) == faced)
    }

    /// Read the sign at `index`: its `text` is plain text, shown as one-off
    /// pages (same shape as an NPC's raw `talk` fallback).
    fn read_sign(&mut self, index: usize) {
        let map = self.map.as_ref().expect("read_sign requires a map");
        let text = map.objects().signs[index].text.clone();
        if text.is_empty() {
            return;
        }
        self.last_text = text.clone();
        self.mode = Mode::Text(TextState {
            engine: None,
            pages: paginate(&text),
        });
    }

    /// Talk dispatch for NPC `index`: `talk` as storyline name → matching
    /// route → map scene `main` → raw `talk` text as a one-off line.
    fn talk_to(&mut self, index: usize) {
        let (map_id, npc_name, npc_id, talk) = {
            let map = self.map.as_ref().expect("talk requires a map");
            let npc = &map.objects().npcs[index];
            (
                map.id().to_string(),
                npc.name.clone(),
                npc.id,
                npc.talk.clone(),
            )
        };

        // 1. The talk field names a storyline.
        if !talk.is_empty() {
            if let Some(scene) = self.scene_with_storyline(&map_id, &talk) {
                if self.activate(&scene, &talk) {
                    return;
                }
            }
        }

        // 2. A route whose npc matches this NPC (name, or id as a string).
        let route = self
            .project
            .routes()
            .iter()
            .filter(|r| r.map == map_id && !r.on_enter)
            .find(|r| {
                r.npc.as_deref().is_some_and(|n| {
                    (!npc_name.is_empty() && n == npc_name) || n == npc_id.to_string()
                })
            })
            .map(|r| r.storyline.clone());
        if let Some(storyline) = route {
            if let Some(scene) = self.scene_with_storyline(&map_id, &storyline) {
                if self.activate(&scene, &storyline) {
                    return;
                }
            }
        }

        // 3. The map scene's main storyline.
        if let Some(scene) = self.scene_for_map(&map_id) {
            if scene_has_fn(&self.project, &scene, "main") && self.activate(&scene, "main") {
                return;
            }
        }

        // 4. The talk field is plain text — show it as a one-off line.
        if !talk.is_empty() {
            self.last_text = talk.clone();
            self.mode = Mode::Text(TextState {
                engine: None,
                pages: paginate(&talk),
            });
        }
    }

    // ── rendering ───────────────────────────────────────────────────────────

    /// Render the current frame. Callable without any window; the `GameLoop`
    /// impl forwards here.
    pub fn draw(&mut self, fb: &mut FrameBuffer) {
        if let Some(map) = &self.map {
            fb.clear(Rgba::BLACK);
            let cam_x = self.camera.position.x.round() as i32;
            let cam_y = self.camera.position.y.round() as i32;
            let level = self.actor.elevation() as i32;
            // Layers at/below the player's elevation draw under the sprites;
            // higher layers (e.g. wall tops seen from the ground) over them.
            if let Err(e) =
                map.render_below(fb, cam_x, cam_y, SCREEN_W as u32, SCREEN_H as u32, level)
            {
                log::warn!("map render: {e:#}");
            }
            self.draw_npcs(fb, cam_x, cam_y);
            self.draw_player(fb, cam_x, cam_y);
            if let Err(e) =
                map.render_above(fb, cam_x, cam_y, SCREEN_W as u32, SCREEN_H as u32, level)
            {
                log::warn!("map render: {e:#}");
            }
        } else {
            // Dialogue-only backdrop.
            fb.clear(Rgba::rgb(0x10, 0x10, 0x18));
        }

        match &self.mode {
            Mode::Text(state) => {
                if let Some(page) = state.pages.front() {
                    draw_textbox(fb, page);
                }
            }
            Mode::Choice(state) => {
                if !state.context_text.is_empty() {
                    draw_textbox(fb, &state.context_text);
                }
                draw_choice_menu(fb, &state.options, state.cursor);
            }
            Mode::Battle(state) => state.battle.draw(fb),
            Mode::Menu(state) => self.draw_menu(fb, state),
            Mode::Shop(state) => self.draw_shop(fb, state),
            Mode::Whiteout(state) => {
                if let Some(page) = state.pages.front() {
                    draw_textbox(fb, page);
                }
            }
            Mode::Idle if self.map.is_none() => {
                // Dialogue-only projects end here: show a small end card
                // instead of leaving a void on screen.
                draw_end_card(fb, &self.project.manifest().name, &self.lang);
            }
            _ => {}
        }

        // Fade overlays (warp transition / cosmetic flash).
        let darkness = match &self.transition {
            Some(t) => match t.phase {
                FadePhase::Out => 1.0 - t.frames as f32 / FADE_FRAMES as f32,
                FadePhase::In => t.frames as f32 / FADE_FRAMES as f32,
            },
            None if self.flash > 0 => 0.5 * self.flash as f32 / FLASH_FRAMES as f32,
            None => 0.0,
        };
        if darkness > 0.0 {
            darken(fb, 1.0 - darkness.clamp(0.0, 1.0));
        }

        // The whiteout's blackout phase covers everything.
        if let Mode::Whiteout(state) = &self.mode {
            if state.blackout > 0 {
                fb.fill_rect(0, 0, SCREEN_W as u32, SCREEN_H as u32, Rgba::BLACK);
            }
        }
    }

    /// NPC placeholders: a two-tone person blob per NPC, palette derived from
    /// the NPC id so distinct NPCs read as distinct people.
    fn draw_npcs(&self, fb: &mut FrameBuffer, cam_x: i32, cam_y: i32) {
        let Some(map) = &self.map else {
            return;
        };
        let tile = map.tile_size().0 as i32;
        for npc in &map.objects().npcs {
            let facing = parse_facing(&npc.facing);
            let colors = npc_palette(npc.id);
            draw_person(
                fb,
                npc.x * tile - cam_x,
                npc.y * tile - cam_y,
                tile,
                facing,
                &colors,
            );
        }
    }

    fn draw_player(&self, fb: &mut FrameBuffer, cam_x: i32, cam_y: i32) {
        let tile = self
            .map
            .as_ref()
            .map(|m| m.tile_size().0 as i32)
            .unwrap_or(16);
        let foot_x = self.actor.px().round() as i32 - cam_x;
        let foot_y = self.actor.py().round() as i32 - cam_y;
        if let Some(sprite) = &self.player_sprite {
            let col = frame_col(
                self.actor.locomotion(),
                self.actor.step_phase(),
                sprite.cols,
            );
            sprite.draw_on_tile(fb, self.actor.facing_row(), col, foot_x, foot_y, tile);
            return;
        }
        draw_person(
            fb,
            foot_x,
            foot_y,
            tile,
            self.actor.facing(),
            &PLAYER_COLORS,
        );
    }

    fn center_camera(&mut self) {
        let tile = self
            .map
            .as_ref()
            .map(|m| m.tile_size().0 as i32)
            .unwrap_or(16);
        let cx = self.actor.px() + (tile / 2) as f32;
        let cy = self.actor.py() + (tile / 2) as f32;
        self.camera.follow_target(Vec2::new(
            cx - SCREEN_W as f32 / 2.0,
            cy - SCREEN_H as f32 / 2.0,
        ));
    }

    // ── introspection (headless driver, tests) ──────────────────────────────

    /// The currently loaded map id (`None` in dialogue-only mode).
    pub fn current_map_id(&self) -> Option<&str> {
        self.map.as_ref().map(RuntimeMap::id)
    }

    /// A persistent story flag's value (defaults to `false`).
    pub fn flag(&self, name: &str) -> bool {
        self.flags.get(name).copied().unwrap_or(false)
    }

    /// The text page currently on screen, if a textbox is open (including
    /// the game-over whiteout message).
    pub fn dialogue_text(&self) -> Option<&str> {
        match &self.mode {
            Mode::Text(state) => state.pages.front().map(String::as_str),
            Mode::Whiteout(state) if state.blackout == 0 => state.pages.front().map(String::as_str),
            _ => None,
        }
    }

    /// The choice options currently on screen, if a choice menu is open.
    pub fn choice_options(&self) -> Option<&[String]> {
        match &self.mode {
            Mode::Choice(state) => Some(&state.options),
            _ => None,
        }
    }

    /// The live battle, if one is running (test/debug introspection).
    pub fn battle(&self) -> Option<&Battle> {
        match &self.mode {
            Mode::Battle(state) => Some(&state.battle),
            _ => None,
        }
    }

    /// The persistent party state, once a battle has completed or a save
    /// restored one (test/debug introspection).
    pub fn party_state(&self) -> Option<&[PartyMemberState]> {
        self.party_state.as_deref()
    }

    /// The persistent battle inventory, once a battle has completed or a
    /// save restored one (test/debug introspection).
    pub fn inventory(&self) -> Option<&HashMap<String, u32>> {
        self.inventory.as_ref()
    }

    /// The player's current money (test/debug introspection).
    pub fn money(&self) -> u32 {
        self.money
    }

    /// The rows the open Start menu currently displays (root labels, party
    /// detail lines, bag rows, target rows, or the note text); `None` when
    /// the menu is closed (test/debug introspection).
    pub fn menu_lines(&self) -> Option<Vec<String>> {
        match &self.mode {
            Mode::Menu(state) => Some(self.menu_lines_for(state)),
            _ => None,
        }
    }

    /// The item rows the open shop displays (`×`-prefixed when
    /// unaffordable), plus the transient note as a final row when one is
    /// showing; `None` when no shop is open (test/debug introspection).
    pub fn shop_lines(&self) -> Option<Vec<String>> {
        match &self.mode {
            Mode::Shop(state) => Some(self.shop_lines_for(state)),
            _ => None,
        }
    }

    /// `true` while the game-over whiteout owns the screen (blackout or
    /// message phase; test/debug introspection).
    pub fn whiteout_active(&self) -> bool {
        matches!(self.mode, Mode::Whiteout(_))
    }

    /// The player's current tile.
    pub fn player_tile(&self) -> (i32, i32) {
        self.actor.tile()
    }

    /// The player's current elevation level (multi-level maps; test/debug
    /// introspection).
    pub fn player_elevation(&self) -> u8 {
        self.actor.elevation()
    }

    /// `true` when tile `(x, y)` is map-solid on the current map (solid when
    /// there is no map). NPC occupancy is not included; test/debug helper.
    pub fn is_blocked(&self, x: i32, y: i32) -> bool {
        match &self.map {
            Some(map) => map.is_blocked(x, y),
            None => true,
        }
    }

    /// The audio subsystem (test/debug introspection).
    pub fn audio(&self) -> &RunnerAudio {
        &self.audio
    }

    /// Pull `frames` stereo PCM frames (44100 Hz, interleaved L/R `f32`,
    /// length `2 * frames`) from the audio engine — the render path for
    /// hosts without an audio callback thread (the WASM shell feeding
    /// WebAudio). Empty unless [`RunnerOptions::pcm_audio`] is on and a play
    /// command has arrived.
    pub fn render_audio(&mut self, frames: usize) -> Vec<f32> {
        self.audio.render_samples(frames)
    }

    /// Teleport the player (test/debug helper; no scene side effects).
    pub fn debug_place(&mut self, x: i32, y: i32, facing: Direction) {
        self.actor.place(x, y, facing);
        self.center_camera();
        self.camera.update(0.0);
    }

    /// Frames updated so far.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl dotzuki_app::GameLoop for RunnerGame {
    type Fb = FrameBuffer;

    fn update(&mut self, input: &InputState) {
        RunnerGame::update(self, input);
    }

    fn draw(&mut self, frame_buffer: &mut FrameBuffer) {
        RunnerGame::draw(self, frame_buffer);
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

/// Decode a PNG walk sheet from in-memory bytes (the VFS counterpart of
/// `WalkSprite::load`, which is disk-only).
fn decode_walk_sheet(
    bytes: &[u8],
    path: &str,
    frame_w: u32,
    frame_h: u32,
) -> Result<WalkSprite, String> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| format!("decode {path}: {e}"))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    let pixels = img
        .pixels()
        .map(|p| Rgba::new(p.0[0], p.0[1], p.0[2], p.0[3]))
        .collect();
    WalkSprite::from_rgba(pixels, w, h, frame_w, frame_h)
}

/// Cheap probe for "does this scene export this storyline/function name"
/// without instantiating a [`ScriptEngine`] — matched on the DSL's generated
/// export names (`storyline_<name>`, `<Scene>OnLoad`). [`RunnerGame::activate`]
/// re-validates authoritatively with `has_function` before running.
fn scene_has_fn(project: &LoadedProject, scene: &str, fn_name: &str) -> bool {
    let Some(js) = project.scripts().get_script(scene) else {
        return false;
    };
    js.contains(&format!("storyline_{fn_name}")) || js.contains(&format!("function {fn_name}"))
}

/// Directories `--watch` monitors: every DSL dir, the data root and the
/// gfx root (deduplicated; missing dirs are skipped by the watcher).
#[cfg(all(feature = "watch", not(target_arch = "wasm32")))]
fn watch_dirs(project: &LoadedProject) -> Vec<PathBuf> {
    let mut dirs = project.manifest().dsl_dirs(project.root());
    dirs.push(project.data_root().to_path_buf());
    if let Some(gfx) = project.gfx_root() {
        dirs.push(gfx.to_path_buf());
    }
    let mut seen = HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));
    dirs
}

/// First free tile scanning outward (Chebyshev rings) from the map centre.
/// "Free" = not map-solid and not NPC-occupied.
fn find_spawn(map: &RuntimeMap) -> (i32, i32) {
    let (cx, cy) = (map.width() as i32 / 2, map.height() as i32 / 2);
    let free = |x: i32, y: i32| {
        !map.is_blocked(x, y) && !map.objects().npcs.iter().any(|n| (n.x, n.y) == (x, y))
    };
    let max_r = (map.width().max(map.height()) as i32) + 1;
    for r in 0..max_r {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dy.abs()) != r {
                    continue; // ring perimeter only
                }
                let (x, y) = (cx + dx, cy + dy);
                if free(x, y) {
                    return (x, y);
                }
            }
        }
    }
    (cx, cy)
}

/// Held D-pad direction (Up > Down > Left > Right priority, as wuxia).
fn held_direction(input: &InputState) -> Option<Direction> {
    if input.is_held(GbButton::Up) {
        Some(Direction::Up)
    } else if input.is_held(GbButton::Down) {
        Some(Direction::Down)
    } else if input.is_held(GbButton::Left) {
        Some(Direction::Left)
    } else if input.is_held(GbButton::Right) {
        Some(Direction::Right)
    } else {
        None
    }
}

/// Unit step delta for a cardinal direction.
fn direction_delta(dir: Direction) -> (i32, i32) {
    match dir {
        Direction::Down => (0, 1),
        Direction::Up => (0, -1),
        Direction::Left => (-1, 0),
        Direction::Right => (1, 0),
    }
}

/// `"down"`/`"up"`/`"left"`/`"right"` sidecar string → [`Direction`].
fn parse_facing(facing: &str) -> Direction {
    match facing {
        "up" => Direction::Up,
        "left" => Direction::Left,
        "right" => Direction::Right,
        _ => Direction::Down,
    }
}

/// [`Direction`] → the sidecar/save string form (inverse of [`parse_facing`]).
fn facing_name(dir: Direction) -> &'static str {
    match dir {
        Direction::Down => "down",
        Direction::Up => "up",
        Direction::Left => "left",
        Direction::Right => "right",
    }
}

/// Wrap `text` into pages of at most [`DIALOG_LINES_PER_PAGE`] lines each
/// (the join of the page's lines with `\n`). Always at least one page.
fn paginate(text: &str) -> VecDeque<String> {
    let lines = wrap_lines(text, DIALOG_WIDTH_PX, 4096);
    let mut pages: VecDeque<String> = lines
        .chunks(DIALOG_LINES_PER_PAGE)
        .map(|chunk| chunk.join("\n"))
        .collect();
    if pages.is_empty() {
        pages.push_back(String::new());
    }
    pages
}

/// The shared dialogue [`MenuConfig`] (bottom box on the 40×30 tile grid).
fn dialog_config() -> MenuConfig {
    MenuConfig::new(
        DIALOG_AREA,
        None,
        TileRect::new(
            DIALOG_AREA.tx + 1,
            DIALOG_AREA.ty + 1,
            DIALOG_AREA.tw - 2,
            DIALOG_AREA.th - 2,
        ),
        Default::default(),
    )
}

/// Draw the bottom dialogue textbox with one page of text.
pub(crate) fn draw_textbox(fb: &mut FrameBuffer, text: &str) {
    let mut painter = FrameBufferPainter::new(fb);
    draw_dialog(text, &[dialog_config()], &mut painter);
}

/// Centered end card for dialogue-only projects whose entry scene finished:
/// the game name plus a localized "fin." so the screen isn't a void.
fn draw_end_card(fb: &mut FrameBuffer, game_name: &str, lang: &str) {
    let fin = if lang == "zh" { "完" } else { "fin." };
    let cx = SCREEN_W as u32 / 2;
    let name_w = embedded_font::measure_text(game_name);
    embedded_font::draw_text(
        game_name,
        cx.saturating_sub(name_w / 2),
        100,
        Rgba::rgb(0xf0, 0xf0, 0xf0),
        fb,
    );
    let fin_w = embedded_font::measure_text(fin);
    embedded_font::draw_text(
        fin,
        cx.saturating_sub(fin_w / 2),
        120,
        Rgba::rgb(0x90, 0x90, 0xa8),
        fb,
    );
}

/// Draw the choice menu as a flex box just above the dialogue area,
/// right-aligned, sized to the options.
fn draw_choice_menu(fb: &mut FrameBuffer, options: &[String], cursor: usize) {
    let n = options.len() as u32;
    if n == 0 {
        return;
    }
    let max_len = options.iter().map(|o| o.chars().count()).max().unwrap_or(1) as u32;
    // +4: left/right border, cursor column, one padding column.
    let w = (max_len + 4).clamp(8, 20);
    let h = n + 2;
    let tx = (40 - w) as i32;
    let ty = DIALOG_AREA.ty as i32 - h as i32;
    let config = MenuConfig::new(
        TileRect::new(tx.max(0) as u32, ty.max(0) as u32, w, h),
        None,
        TileRect::new(tx.max(0) as u32 + 1, ty.max(0) as u32 + 1, w - 2, n),
        Default::default(),
    );
    let state = FlexMenuState {
        cursor,
        scroll_offset: 0,
    };
    let mut painter = FrameBufferPainter::new(fb);
    let mut ui = Ui::new(&mut painter);
    draw_flex_menu(options, &[config], &state, options.len(), &mut ui);
}

/// Multiply every framebuffer pixel by `factor` (1.0 = unchanged, 0.0 = black).
fn darken(fb: &mut FrameBuffer, factor: f32) {
    for px in fb.data.chunks_exact_mut(4) {
        px[0] = (px[0] as f32 * factor) as u8;
        px[1] = (px[1] as f32 * factor) as u8;
        px[2] = (px[2] as f32 * factor) as u8;
    }
}

// ── placeholder people ──────────────────────────────────────────────────────

/// Palette of a procedurally drawn placeholder person.
struct PersonColors {
    outline: Rgba,
    skin: Rgba,
    body: Rgba,
}

/// The player's palette (red jacket, the classic protagonist read).
const PLAYER_COLORS: PersonColors = PersonColors {
    outline: Rgba::rgb(0x30, 0x18, 0x18),
    skin: Rgba::rgb(0xF0, 0xC8, 0xA0),
    body: Rgba::rgb(0xC8, 0x30, 0x30),
};

/// NPC body colours; the NPC id hashes into this palette.
const NPC_BODIES: [(u8, u8, u8); 6] = [
    (0x30, 0x58, 0xC8), // blue
    (0x38, 0x90, 0x40), // green
    (0x88, 0x48, 0xA8), // purple
    (0xC8, 0x78, 0x28), // orange
    (0x28, 0x98, 0x98), // teal
    (0x80, 0x58, 0x38), // brown
];

/// Per-NPC palette: body colour derived from the id hash.
fn npc_palette(id: u32) -> PersonColors {
    let (r, g, b) = NPC_BODIES[(id.wrapping_mul(2_654_435_761) >> 16) as usize % NPC_BODIES.len()];
    PersonColors {
        outline: Rgba::rgb(r / 3, g / 3, b / 3),
        skin: Rgba::rgb(0xF0, 0xC8, 0xA0),
        body: Rgba::rgb(r, g, b),
    }
}

/// Draw a ~12×14 two-tone placeholder person, centred on and bottom-aligned
/// to the `tile`-px tile whose top-left is `(sx, sy)` in screen pixels. Pure
/// code — no embedded assets. Eyes (or the back of the head when facing up)
/// give a 1-px facing indicator.
fn draw_person(
    fb: &mut FrameBuffer,
    sx: i32,
    sy: i32,
    tile: i32,
    facing: Direction,
    colors: &PersonColors,
) {
    const W: i32 = 12;
    const H: i32 = 14;
    let ox = sx + (tile - W) / 2;
    let oy = sy + tile - H;
    let (w, h) = (fb.width() as i32, fb.height() as i32);
    let mut put = |x: i32, y: i32, c: Rgba| {
        let (px, py) = (ox + x, oy + y);
        if px >= 0 && py >= 0 && px < w && py < h {
            fb.set_pixel(px as u32, py as u32, c);
        }
    };
    let fill = |x: i32, y: i32, fw: i32, fh: i32, c: Rgba, put: &mut dyn FnMut(i32, i32, Rgba)| {
        for dy in 0..fh {
            for dx in 0..fw {
                put(x + dx, y + dy, c);
            }
        }
    };

    // Head (6×5) and torso (8×7), 1-px outline.
    fill(3, 0, 6, 5, colors.outline, &mut put);
    fill(4, 1, 4, 3, colors.skin, &mut put);
    fill(2, 5, 8, 7, colors.outline, &mut put);
    fill(3, 6, 6, 5, colors.body, &mut put);
    // Legs.
    fill(3, 12, 2, 2, colors.outline, &mut put);
    fill(7, 12, 2, 2, colors.outline, &mut put);

    // Facing indicator on the face rows (skin spans x 4..8, y 1..4).
    match facing {
        Direction::Down => {
            put(4, 2, colors.outline);
            put(7, 2, colors.outline);
        }
        Direction::Left => put(4, 2, colors.outline),
        Direction::Right => put(7, 2, colors.outline),
        Direction::Up => fill(4, 1, 4, 3, colors.outline, &mut put), // back of the head
    }
}
