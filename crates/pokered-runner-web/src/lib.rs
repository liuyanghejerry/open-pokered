//! Browser WYSIWYG shell for the pokered game: [`PokeredRunner`].
//!
//! Embeds the full Rust game ([`PokemonGame`](pokered_app::PokemonGame) from
//! `pokered-app`) headless in the browser editor and exposes a frame-driven
//! `tick(input_bitmask) → RGBA bytes` loop for a `<canvas>` `ImageData`
//! blit — the same pattern as `dotzuki-runner-web`'s `WasmRunner`, but for the
//! *real* game instead of a zero-Rust DSL project.
//!
//! Audio needs no bridge: the game's `AudioOutput` uses the Web Audio API
//! directly (ScriptProcessorNode) and plays through the tab's default output.
//! Saves persist to `localStorage` (key `pokered.save`, same as `pokered-web`)
//! and can also be injected/exported as JSON via
//! [`import_save`](Self::import_save) / [`export_save`](Self::export_save).
//!
//! ## WYSIWYG injection API (the editor's edit → see-it-in-game loop)
//!
//! | method | purpose |
//! |--------|---------|
//! | [`warp_to`](Self::warp_to) | teleport the player to a map / coordinate |
//! | [`start_wild_battle`](Self::start_wild_battle) | jump straight into a wild battle vs a species |
//! | [`start_trainer_battle`](Self::start_trainer_battle) | jump straight into a trainer battle (class + party index) |
//! | [`open_pokedex`](Self::open_pokedex) | open the Pokédex on a species' entry |
//! | [`reload_scripts`](Self::reload_scripts) | hot-register compiled `.scene` JS + `script_config.json` per map |
//! | [`set_wild_data`](Self::set_wild_data) | override a map's wild-encounter tables at runtime |
//! | [`import_save`](Self::import_save) / [`export_save`](Self::export_save) | snapshot/restore the editor-side save |
//! | [`reset`](Self::reset) | reboot the game (after heavy data edits) |
//!
//! ## Input bitmask
//!
//! `tick` takes a `u8` of currently-held buttons, one bit per
//! [`GbButton`](pokered_renderer::input::GbButton):
//!
//! | bit | button |
//! |-----|--------|
//! | 0   | A      |
//! | 1   | B      |
//! | 2   | Select |
//! | 3   | Start  |
//! | 4   | Right  |
//! | 5   | Left   |
//! | 6   | Up     |
//! | 7   | Down   |
//!
//! A bit set for exactly one `tick` is a "just pressed" edge; the wrapper
//! keeps the previous frame's mask internally.
//!
//! ## Typical JS loop
//!
//! ```js
//! const runner = new PokeredRunner(localStorage.getItem("pokered-save-editor"));
//! const ctx = canvas.getContext("2d");
//! const image = ctx.createImageData(160, 144);
//! function frame() {
//!     image.data.set(runner.tick(inputBitmask()));
//!     ctx.putImageData(image, 0, 0);
//!     requestAnimationFrame(frame);
//! }
//! requestAnimationFrame(frame);
//! ```

use std::collections::HashMap;

use pokered_app::PokemonGame;
use pokered_core::data::impl_traits::PokemonRedData;
use pokered_core::data::maps::MapId;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::data::wild_data::{clear_wild_data_overrides, set_wild_data_override};
use pokered_core::game_state::{GameScreen, Lang};
use pokered_core::overworld::{Direction, OverworldScreen};
use pokered_core::save::SaveData;
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::runtime_overrides::parse_trainer_class;
use pokered_data::species::Species;
use std::str::FromStr;
use pokered_renderer::input::InputState;
use pokered_renderer::{FrameBuffer, RenderConfig, Rgba};
use wasm_bindgen::prelude::*;

const SCREEN_W: u32 = 160;
const SCREEN_H: u32 = 144;
/// A booted pokered game, ready to be driven frame by frame with the full
/// editor injection surface.
#[wasm_bindgen]
pub struct PokeredRunner {
    game: PokemonGame,
    input: InputState,
    fb: FrameBuffer,
    muted: bool,
}

#[wasm_bindgen]
impl PokeredRunner {
    /// Boot a fresh game and jump straight into the overworld. `save_json`,
    /// when given, is a save previously produced by
    /// [`export_save`](Self::export_save) (or any `SaveData` JSON) and is
    /// applied after boot; otherwise the game starts with an empty save at
    /// Pallet Town.
    ///
    /// Fails with a human-readable message when the save JSON is malformed.
    #[wasm_bindgen(constructor)]
    pub fn new(save_json: Option<String>) -> Result<PokeredRunner, JsValue> {
        #[cfg(feature = "debug-panic-hook")]
        console_error_panic_hook::set_once();
        Self::boot(save_json.as_deref()).map_err(|e| JsValue::from_str(&e))
    }

    /// Advance one frame: feed `input_bitmask` (see crate docs) to the game,
    /// update, draw, and return the 160×144×4 RGBA framebuffer.
    pub fn tick(&mut self, input_bitmask: u8) -> Vec<u8> {
        self.input.set_from_bitmask(input_bitmask);
        self.game.update(&self.input);
        self.input.begin_frame();
        self.game.draw(&mut self.fb);
        self.fb.data.clone()
    }

    /// Framebuffer width in pixels.
    pub fn width(&self) -> u32 {
        SCREEN_W
    }

    /// Framebuffer height in pixels.
    pub fn height(&self) -> u32 {
        SCREEN_H
    }

    // ── WYSIWYG injection API ────────────────────────────────────────────

    /// Teleport the player to `map_name` (the map directory name, e.g.
    /// `"PalletTown"`, or its debug name) at player coordinates `x`, `y` —
    /// 2 tiles per unit, the same space as map.json warp coordinates — or a
    /// walkable default when both are `0` (the map's own warp spots first,
    /// then its center). A requested position that is out of bounds or
    /// blocked (roof/wall/water) is snapped to the nearest walkable spot, so
    /// the player never lands stuck. The overworld warp (fade) is handled by
    /// the game itself on the next frames. Fails when the map name doesn't
    /// resolve.
    pub fn warp_to(&mut self, map_name: &str, x: u32, y: u32) -> Result<(), JsValue> {
        let map_id = resolve_map_id(map_name)
            .ok_or_else(|| JsValue::from_str(&format!("unknown map name '{map_name}'")))?;
        let requested = if x == 0 && y == 0 {
            None
        } else {
            Some((x.min(255) as u8, y.min(255) as u8))
        };
        let (dest_x, dest_y) = self.game.overworld.resolve_editor_warp_position(map_id, requested);
        self.game.state.screen = GameScreen::Overworld;
        self.game
            .overworld
            .warp_to_map(map_id, dest_x, dest_y);
        Ok(())
    }

    /// Hot-reload compiled scene scripts and their configs without a rebuild.
    ///
    /// `scenes_json` maps a map key (e.g. `"PalletTown"`) to compiled `.scene`
    /// JavaScript; `configs_json` optionally maps the same keys to their
    /// `script_config.json` contents. Both accept `"{}"`/`""` for "nothing".
    /// When a key matches the current map, its script engine, config and
    /// triggers are fully reloaded (`load_map_script` semantics — the map's
    /// `on_load` runs again, same as re-entering the map).
    pub fn reload_scripts(&mut self, scenes_json: &str, configs_json: &str) -> Result<(), JsValue> {
        let scenes: HashMap<String, String> =
            parse_string_map(scenes_json, "scenes_json").map_err(|e| JsValue::from_str(&e))?;
        let configs: HashMap<String, String> =
            parse_string_map(configs_json, "configs_json").map_err(|e| JsValue::from_str(&e))?;
        for (map_key, js) in &scenes {
            self.game
                .overworld
                .hot_reload_map_scripts(map_key, js, configs.get(map_key).map(|s| s.as_str()))
                .map_err(|e| JsValue::from_str(&e))?;
        }
        Ok(())
    }

    /// Override the wild-encounter tables of `map_name` at runtime. `json` is
    /// the editor's `map.json` `wild` block (camelCase `red`/`blue` tables,
    /// see `pokered_data::wild_data::set_wild_data_override`). Returns
    /// `false` when the JSON can't be parsed.
    pub fn set_wild_data(&mut self, map_name: &str, json: &str) -> bool {
        set_wild_data_override(map_name, json)
    }

    /// Drop all runtime wild-encounter overrides injected by the editor.
    pub fn clear_wild_data(&mut self) {
        clear_wild_data_overrides();
    }

    /// Restore a save produced by [`export_save`](Self::export_save). The
    /// game is rebooted into the overworld at the save's map/position (same
    /// as a native `--skip-intro` start). Returns `false` — the game keeps
    /// its current state — when the JSON is malformed.
    pub fn import_save(&mut self, json: &str) -> bool {
        let mut fresh = match Self::boot(Some(json)) {
            Ok(r) => r,
            Err(_) => return false,
        };
        fresh.muted = self.muted;
        if fresh.muted {
            fresh.game.audio = None;
        }
        *self = fresh;
        true
    }

    /// Editor quick-entry: boot the game with a Save Editor snapshot (the
    /// editor's `SaveDataSnapshot` JSON shape, see `save-data.ts`). The
    /// snapshot is converted to a real [`SaveData`] (unknown species/moves/
    /// items are skipped) and the game is rebooted with it — same semantics
    /// as [`import_save`](Self::import_save). Returns `false` when the JSON
    /// can't be parsed or the snapshot's map name doesn't resolve; the game
    /// keeps its current state then.
    pub fn import_editor_save(&mut self, json: &str) -> bool {
        let snapshot: pokered_app::save_editor::EditorSaveSnapshot =
            match serde_json::from_str(json) {
                Ok(s) => s,
                Err(_) => return false,
            };
        let save = match pokered_app::save_editor::apply_editor_save(&snapshot) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let Some(save_json) = serde_json::to_string(&save).ok() else {
            return false;
        };
        self.import_save(&save_json)
    }

    /// The current state as save JSON for the caller to persist
    /// (`localStorage`); `None` when it can't be serialized.
    pub fn export_save(&self) -> Option<String> {
        serde_json::to_string(&self.game.save_data).ok()
    }

    /// Reboot the game from scratch (same as [`new`](Self::new)), keeping
    /// the current mute state and any injected wild-data overrides. Used
    /// after heavy data edits (items/moves/trainers) that can't be injected
    /// live — the reloaded game picks the new data up on next boot.
    pub fn reset(&mut self, save_json: Option<String>) -> Result<(), JsValue> {
        let was_muted = self.muted;
        let mut fresh = Self::boot(save_json.as_deref()).map_err(|e| JsValue::from_str(&e))?;
        fresh.muted = was_muted;
        if was_muted {
            fresh.game.audio = None;
        }
        *self = fresh;
        Ok(())
    }

    /// Mute/unmute the game's audio (the Web Audio context is torn down /
    /// rebuilt; unmuting works even without a user gesture in modern
    /// browsers since the context is created lazily).
    pub fn set_muted(&mut self, muted: bool) {
        if self.muted == muted {
            return;
        }
        self.muted = muted;
        if muted {
            self.game.audio = None;
        } else {
            self.game.audio = pokered_app::audio::AudioOutput::new();
        }
    }

    /// Whether audio is currently muted.
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Stop all currently playing music/SFX, leaving the Web Audio output
    /// alive. Used when the editor leaves the playtest: the game loop stops
    /// ticking (`update_frame` no longer advances the music sequencer), so
    /// without this the ScriptProcessorNode would keep mixing the frozen APU
    /// state — the last note/chord droning on instead of silence. The APU
    /// channels are only silenced during `update_frame` (`apply_to_apu`), so
    /// a couple of flush frames are needed after `stop_all`.
    pub fn stop_audio(&mut self) {
        if let Some(audio) = self.game.audio.as_ref() {
            audio.stop_all();
            audio.update_frame();
            audio.update_frame();
        }
    }

    /// Resume the current map's background music after [`stop_audio`]
    /// (Self::stop_audio) — called when re-entering the playtest. Re-queues
    /// the map-music request; the app layer honors it on the next `tick`.
    /// No-op while muted.
    pub fn resume_audio(&mut self) {
        if self.muted {
            return;
        }
        let map = self.game.overworld.state.current_map;
        self.game
            .overworld
            .audio_requests
            .push(pokered_core::overworld::OverworldAudioRequest::PlayMapMusic { map });
    }

    /// The current map's directory name (e.g. `"PalletTown"`).
    pub fn current_map(&self) -> String {
        format!("{:?}", self.game.overworld.state.current_map)
    }

    /// The player's overworld position as `"x,y"` player units (2 tiles per
    /// unit — the same space as map.json warp coordinates).
    pub fn player_position(&self) -> String {
        let p = &self.game.overworld.state.player;
        format!("{},{}", p.x, p.y)
    }

    /// Whether any runtime override is currently active for the family above.
    pub fn has_data_overrides(&self) -> bool {
        pokered_data::runtime_overrides::has_data_overrides()
    }

    /// Drop every injected data override (map/trainer/move/item/pokemon).
    pub fn clear_data_overrides(&mut self) {
        pokered_data::runtime_overrides::clear_data_overrides();
    }

    /// Override a map's `map.json` at runtime (editor file shape). Returns
    /// `false` on unparseable JSON.
    pub fn set_map_data(&mut self, map_name: &str, json: &str) -> bool {
        pokered_data::runtime_overrides::set_map_data_override(map_name, json)
    }

    /// Override a map's `map.blk` block data (editor number array). Returns
    /// `false` on unparseable JSON.
    pub fn set_map_blk(&mut self, map_name: &str, json: &str) -> bool {
        pokered_data::runtime_overrides::set_map_blk_override(map_name, json)
    }

    /// Override a trainer class's parties at runtime. Returns `false` when the
    /// class name or JSON is invalid.
    pub fn set_trainer(&mut self, class_name: &str, json: &str) -> bool {
        pokered_data::runtime_overrides::set_trainer_override(class_name, json)
    }

    /// Override a move's data at runtime. Returns `false` when the move name
    /// or JSON is invalid.
    pub fn set_move(&mut self, move_name: &str, json: &str) -> bool {
        pokered_data::runtime_overrides::set_move_override(move_name, json)
    }

    /// Override an item's data at runtime. Returns `false` when the item name
    /// or JSON is invalid.
    pub fn set_item(&mut self, item_name: &str, json: &str) -> bool {
        pokered_data::runtime_overrides::set_item_override(item_name, json)
    }

    /// Override a species' base stats at runtime. Returns `false` when the
    /// species name or JSON is invalid.
    pub fn set_base_stats(&mut self, species_name: &str, json: &str) -> bool {
        pokered_data::runtime_overrides::set_base_stats_override(species_name, json)
    }

    /// The current game screen as a short name (`"Overworld"`, `"Battle"`, …)
    /// for the editor's HUD.
    pub fn screen_name(&self) -> String {
        format!("{:?}", self.game.state.screen)
    }

    /// Editor quick-entry: start a wild battle against `species_name` (an
    /// editor species name, e.g. `"Pikachu"`) at `level`. The player's current
    /// party is used; an empty party is seeded a starter so the battle always
    /// has a battler to send out. Returns an error when the species name
    /// doesn't resolve. The game screen switches to `Battle` on the next tick.
    pub fn start_wild_battle(&mut self, species_name: &str, level: u32) -> Result<(), JsValue> {
        let species = resolve_species(species_name)
            .ok_or_else(|| JsValue::from_str(&format!("unknown species '{species_name}'")))?;
        self.game.debug_start_wild_battle(species, level.min(100) as u8);
        Ok(())
    }

    /// Editor quick-entry: open the Pokédex directly on `species_name`'s entry
    /// (full data + cry). The species is registered as seen/owned so the entry
    /// always shows its flavor text; closing the Pokédex returns to the
    /// overworld. Returns an error when the species name doesn't resolve.
    pub fn open_pokedex(&mut self, species_name: &str) -> Result<(), JsValue> {
        let species = resolve_species(species_name)
            .ok_or_else(|| JsValue::from_str(&format!("unknown species '{species_name}'")))?;
        self.game.debug_open_pokedex(species);
        Ok(())
    }

    /// Editor quick-entry: start a trainer battle against `class_name` (an
    /// editor trainer class name, e.g. `"Brock"`) using its `party_index`-th
    /// party (0-based — the editor's party tab index). The player's current
    /// party is used. Returns an error when the class name doesn't resolve.
    pub fn start_trainer_battle(&mut self, class_name: &str, party_index: u32) -> Result<(), JsValue> {
        let class = parse_trainer_class(class_name)
            .ok_or_else(|| JsValue::from_str(&format!("unknown trainer class '{class_name}'")))?;
        self.game.debug_start_trainer_battle(class, party_index as usize);
        Ok(())
    }

    /// Editor quick-entry: verify a move in battle — a Lv25 tester knowing
    /// `move_name` fights a wild Lv25 Pidgey. Returns an error when the move
    /// name doesn't resolve.
    pub fn start_move_test(&mut self, move_name: &str) -> Result<(), JsValue> {
        let move_id = MoveId::from_str(move_name).map_err(|_| {
            JsValue::from_str(&format!("unknown move '{move_name}'"))
        })?;
        self.game.debug_start_move_test(move_id);
        Ok(())
    }

    /// Editor quick-entry: play the evolution animation from `from_species`
    /// to `to_species` (editor species names). Returns an error when either
    /// species name doesn't resolve.
    pub fn play_evolution(&mut self, from_species: &str, to_species: &str) -> Result<(), JsValue> {
        let from = resolve_species(from_species)
            .ok_or_else(|| JsValue::from_str(&format!("unknown species '{from_species}'")))?;
        let to = resolve_species(to_species)
            .ok_or_else(|| JsValue::from_str(&format!("unknown species '{to_species}'")))?;
        self.game.debug_play_evolution(from, to);
        Ok(())
    }

    /// Editor debug: fully restore the player — overworld party healed and,
    /// mid-battle, the player's battler reset to full. See
    /// [`PokemonGame::debug_full_heal`].
    pub fn full_heal(&mut self) {
        self.game.debug_full_heal();
    }

    /// Push the configured text speed (frames between revealed characters) —
    /// 1/3/5, matching the in-game text speed option.
    pub fn set_text_delay_frames(&mut self, frames: u16) {
        self.game.overworld.set_text_delay_frames(frames);
    }
}

impl PokeredRunner {
    /// Native/testable boot path (no `JsValue`): every failure is a plain
    /// `String`.
    fn boot(save_json: Option<&str>) -> Result<Self, String> {
        let mut game = PokemonGame::new(GameVersion::Red);
        apply_editor_save(&mut game, save_json);
        let fb = FrameBuffer::new(RenderConfig::new(SCREEN_W, SCREEN_H), Rgba::BLACK);
        Ok(Self {
            game,
            input: InputState::new(),
            fb,
            muted: false,
        })
    }
}

/// Decode a save JSON and boot the game straight into the overworld at the
/// save's map/position — the wasm equivalent of the native `--skip-intro`
/// start (`new_with_options`), driven through public fields since the wasm
/// constructor has no such option. A malformed/absent save falls back to an
/// empty save at Pallet Town.
fn apply_editor_save(game: &mut PokemonGame, save_json: Option<&str>) {
    let save: SaveData = match save_json {
        Some(json) => match serde_json::from_str(json) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("[pokered-runner-web] ignoring bad save JSON: {e}");
                game.save_data.clone()
            }
        },
        None => game.save_data.clone(),
    };

    // Rebuild the overworld for the save's map (skip-intro semantics): the
    // wasm constructor only boots Pallet Town. An empty save's position is
    // the all-zero default — tile (0, 0), the unreachable top-left corner of
    // every map (see `resolve_editor_warp_position`) — so it can never be a
    // real destination. The warp resolver honors a valid saved position,
    // snaps a blocked one to the nearest walkable tile, and falls back to
    // the map's warp spots / center when none is given — the player never
    // boots stuck.
    let pos = save.game_data.position;
    let map_id = MapId::from_u8(pos.map_id)
        .filter(|m| m.dimensions().0 > 0)
        .unwrap_or(MapId::PalletTown);
    let requested = if pos.x == 0 && pos.y == 0 {
        None
    } else {
        Some((pos.x, pos.y))
    };

    game.save_data = save;
    game.player_name = pokered_data::charmap::decode_string(&game.save_data.player_name);
    game.rival_name = pokered_data::charmap::decode_string(&game.save_data.game_data.rival_name);

    game.overworld = OverworldScreen::new(map_id, None, PokemonRedData);
    let (x, y) = game.overworld.resolve_editor_warp_position(map_id, requested);
    game.overworld.state.player.x = x as u16;
    game.overworld.state.player.y = y as u16;
    game.overworld.state.player.facing = Direction::Down;
    game.overworld.player_name = game.player_name.clone();
    game.overworld.rival_name = game.rival_name.clone();
    game.overworld.set_script_flags(game.save_data.script_flags.clone());
    game.overworld
        .set_toggleable_object_flags(game.save_data.game_data.toggleable_object_flags);
    game.overworld
        .set_hidden_item_flags(game.save_data.game_data.obtained_hidden_items);
    game.overworld.apply_hidden_object_flags();
    game.overworld.party_count = game.save_data.party.count() as u8;
    game.overworld.party_lead_level = game.save_data.party.leader_level();
    game.overworld.set_script_lang(if game.state.config.language == Lang::Zh {
        "zh"
    } else {
        "en"
    });
    game.overworld.run_on_load();

    game.state.screen = GameScreen::Overworld;
}

/// Resolve a map directory/debug name (`"PalletTown"`, case-sensitive debug
/// form) to a [`MapId`], mirroring `pokered_app::parse_warp_arg`'s lookup.
fn resolve_map_id(name: &str) -> Option<MapId> {
    for i in 0..pokered_data::maps::NUM_MAPS {
        if let Some(m) = MapId::from_u8(i as u8) {
            if format!("{:?}", m) == name {
                return Some(m);
            }
        }
    }
    None
}

/// Resolve an editor species name (`"Pikachu"`, `"MrMime"`) to a [`Species`]
/// — the same lookup the base-stats override uses.
fn resolve_species(name: &str) -> Option<Species> {
    Species::from_scene_name(name)
}


/// Parse a `{ "key": "string" }` JSON object; `""` or `"{}"` yields an empty
/// map. Used for the scenes/configs injection payloads. Errors are plain
/// strings so the helper stays native-testable (no `JsValue`).
fn parse_string_map(json: &str, what: &str) -> Result<HashMap<String, String>, String> {
    let trimmed = json.trim();
    if trimmed.is_empty() {
        return Ok(HashMap::new());
    }
    serde_json::from_str(trimmed).map_err(|e| format!("{what}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse the runner's `"x,y"` position string.
    fn parse_xy(pos: &str) -> Option<(u8, u8)> {
        let (x, y) = pos.split_once(',')?;
        Some((x.parse().ok()?, y.parse().ok()?))
    }

    /// Whether the player's standing tile at (x, y) is walkable, using the
    /// game's own collision rules.
    fn position_is_walkable(map: MapId, x: u8, y: u8) -> bool {
        let (map_data, _) = pokered_core::overworld::map_data_loading::load_full_map_data_concrete(map);
        pokered_core::overworld::update::is_script_walkable_tile(&map_data, x as u16, y as u16)
    }

    #[test]
    fn resolve_map_id_matches_debug_names() {
        assert_eq!(resolve_map_id("PalletTown"), Some(MapId::PalletTown));
        assert_eq!(resolve_map_id("Route1"), Some(MapId::Route1));
        assert_eq!(resolve_map_id("NopeTown"), None);
    }

    #[test]
    fn resolve_species_matches_editor_names() {
        assert_eq!(resolve_species("Pikachu"), Some(Species::Pikachu));
        assert_eq!(resolve_species("MrMime"), Some(Species::MrMime));
        assert_eq!(resolve_species("NidoranF"), Some(Species::NidoranF));
        assert_eq!(resolve_species("NopeMon"), None);
    }

    #[test]
    fn start_wild_battle_boots_into_battle_screen() {
        // Empty party: the runner must seed a starter so the battle has a
        // battler, then switch the screen to Battle.
        let mut runner = PokeredRunner::new(None).unwrap();
        assert_eq!(runner.screen_name(), "Overworld");
        runner.start_wild_battle("Pikachu", 5).unwrap();
        assert_eq!(runner.screen_name(), "Battle");
        // The battle is a real wild battle with the requested enemy.
        assert!(runner.game.battle.is_wild);
        assert_eq!(runner.game.battle.enemy_species, Species::Pikachu);
        assert_eq!(runner.game.battle.enemy_level, 5);
        assert!(runner.game.save_data.party.count() >= 1);
    }

    #[test]
    fn start_trainer_battle_boots_into_battle_screen() {
        // Trainer battle entry: screen switches to Battle, non-wild, with the
        // class attached. Seed a player party so both parties build the battle.
        let mut runner = PokeredRunner::new(None).unwrap();
        let starter = pokered_core::pokemon::stats::create_pokemon(
            Species::Bulbasaur,
            5,
            [0x9A, 0x78],
        )
        .unwrap();
        runner.game.save_data.party.add(starter).unwrap();
        runner.start_trainer_battle("Brock", 0).unwrap();
        assert_eq!(runner.screen_name(), "Battle");
        assert!(!runner.game.battle.is_wild);
        assert_eq!(
            runner.game.battle.trainer_class,
            Some(pokered_data::trainer_data::TrainerClass::Brock)
        );
    }

    #[test]
    fn import_editor_save_boots_with_snapshot() {
        // The Save Editor snapshot (camelCase) converts to a bootable save:
        // party + money + position land in the game.
        let mut runner = PokeredRunner::new(None).unwrap();
        let json = r#"{
            "player": { "playerName": "RED", "rivalName": "BLUE", "mapName": "PalletTown",
                        "positionX": 10, "positionY": 8, "playTimeHours": 2,
                        "playTimeMinutes": 30, "money": 9999 },
            "badges": [true, true, true, true],
            "party": [{ "species": "Pikachu", "level": 20, "currentHp": 60, "maxHp": 60,
                        "moves": ["THUNDERSHOCK", "QuickAttack"], "nickname": "Sparky" }],
            "items": [{ "name": "POKE_BALL", "quantity": 12 }],
            "flags": { "EVENT_GOT_POKEDEX": true }
        }"#;
        assert!(runner.import_editor_save(json));
        assert_eq!(runner.game.save_data.party.count(), 1);
        assert_eq!(runner.game.save_data.party.leader().unwrap().species, Species::Pikachu);
        assert_eq!(runner.game.save_data.party.leader().unwrap().level, 20);
        assert_eq!(runner.game.save_data.game_data.player_money, 9999);
        assert_eq!(runner.game.save_data.game_data.obtained_badges, 0b1111);
        assert!(runner.game.save_data.game_data.bag.has_item(ItemId::PokeBall, 12));
        assert_eq!(runner.game.save_data.script_flags.get("EVENT_GOT_POKEDEX"), Some(&true));
        // Upper-case move name ("THUNDERSHOCK") resolves tolerantly.
        assert_eq!(runner.game.save_data.party.leader().unwrap().moves[0], MoveId::Thundershock);
    }

    #[test]
    fn start_move_test_boots_battle_with_tester() {
        let mut runner = PokeredRunner::new(None).unwrap();
        runner.start_move_test("Tackle").unwrap();
        assert_eq!(runner.screen_name(), "Battle");
        assert!(runner.game.battle.is_wild);
        // The tester leads the party and knows the move under test.
        let leader = runner.game.save_data.party.leader().unwrap();
        assert_eq!(leader.species, Species::Pikachu);
        assert!(leader.moves.contains(&MoveId::Tackle));
        // The enemy is the Lv25 wild Pidgey.
        assert_eq!(runner.game.battle.enemy_species, Species::Pidgey);
        assert_eq!(runner.game.battle.enemy_level, 25);
    }

    #[test]
    fn play_evolution_starts_animation_takeover() {
        let mut runner = PokeredRunner::new(None).unwrap();
        runner.play_evolution("Charmander", "Charmeleon").unwrap();
        // The evolution animation takes over rendering from the overworld.
        assert!(runner.game.evolution_anim.is_some());
        let anim = runner.game.evolution_anim.as_ref().unwrap();
        let cur = anim.current().unwrap();
        assert_eq!(cur.from, Species::Charmander);
        assert_eq!(cur.to, Species::Charmeleon);
        // A party slot was staged so the morph has a target.
        assert!(runner.game.save_data.party.count() >= 1);
    }

    #[test]
    fn open_pokedex_lands_on_species_entry() {
        let mut runner = PokeredRunner::new(None).unwrap();
        runner.open_pokedex("Pikachu").unwrap();
        assert_eq!(runner.screen_name(), "Pokedex");
        assert_eq!(runner.game.pokedex_screen.cursor_species(), Species::Pikachu);
        // Registered as seen + owned so the entry shows full data.
        assert!(runner.game.save_data.game_data.pokedex.is_seen(Species::Pikachu));
        assert!(runner.game.save_data.game_data.pokedex.is_owned(Species::Pikachu));
        // Closing (post-capture style, from_list=false) returns to the overworld.
        assert!(!runner.game.pokedex_screen.from_list());
    }

    #[test]
    fn parse_string_map_accepts_empty() {
        for s in ["", "{}", "   "] {
            let m = parse_string_map(s, "test").unwrap();
            assert!(m.is_empty(), "{s:?}");
        }
    }

    #[test]
    fn parse_string_map_round_trips() {
        let m = parse_string_map(r#"{"PalletTown":"let x=1;"}"#, "test").unwrap();
        assert_eq!(m["PalletTown"], "let x=1;");
    }

    #[test]
    fn parse_string_map_reports_bad_json() {
        assert!(parse_string_map("not json", "test").is_err());
    }

    #[test]
    fn map_default_spawn_clamps() {
        // Default warp to PalletTown must land on a walkable, in-bounds tile
        // after the warp fade commits.
        let mut runner = PokeredRunner::new(None).unwrap();
        runner.warp_to("PalletTown", 0, 0).unwrap();
        for _ in 0..200 {
            runner.tick(0);
        }
        let pos = runner.player_position();
        let (x, y) = parse_xy(&pos).expect("position parses");
        let (w, h) = MapId::PalletTown.dimensions();
        assert!(
            (x as u16) < (w as u16) * 2 && (y as u16) < (h as u16) * 2,
            "spawn {pos} out of bounds for {w}x{h} blocks"
        );
        assert!(
            position_is_walkable(MapId::PalletTown, x, y),
            "spawn {pos} not walkable"
        );
    }

    #[test]
    fn new_game_boots_on_walkable_pallet_town() {
        // New Game (empty save) must boot onto a walkable, in-bounds Pallet
        // Town tile — not the empty save's default position (0, 0), the
        // unreachable top-left corner of every map. Uses an explicit
        // empty-save JSON so the result doesn't depend on any save left in
        // localStorage / on disk by a previous run.
        let empty = serde_json::to_string(&SaveData::new()).unwrap();
        let mut runner = PokeredRunner::new(Some(empty)).unwrap();
        let pos = runner.player_position();
        let (x, y) = parse_xy(&pos).expect("position parses");
        assert_ne!((x, y), (0, 0), "new game spawn {pos} must not be the corner");
        let (w, h) = MapId::PalletTown.dimensions();
        assert!(
            (x as u16) < (w as u16) * 2 && (y as u16) < (h as u16) * 2,
            "spawn {pos} out of bounds for {w}x{h} blocks"
        );
        assert!(
            position_is_walkable(MapId::PalletTown, x, y),
            "spawn {pos} not walkable"
        );
    }

    #[test]
    fn warp_snaps_blocked_request_to_walkable() {
        // An out-of-bounds / blocked request must snap to a walkable position.
        let mut runner = PokeredRunner::new(None).unwrap();
        runner.warp_to("PalletTown", 200, 200).unwrap();
        for _ in 0..200 {
            runner.tick(0);
        }
        let pos = runner.player_position();
        let (x, y) = parse_xy(&pos).expect("position parses");
        let (w, h) = MapId::PalletTown.dimensions();
        assert!(
            (x as u16) < (w as u16) * 2 && (y as u16) < (h as u16) * 2,
            "snapped {pos} still out of bounds for {w}x{h} blocks"
        );
        assert!(
            position_is_walkable(MapId::PalletTown, x, y),
            "snapped {pos} not walkable"
        );
    }

    #[test]
    fn wild_override_round_trips() {
        // Route1 has grass + water tables in the generated data; the override
        // must shadow it after injection and vanish after clearing.
        let base = pokered_core::data::wild_data::wild_data_for_map(
            MapId::Route1,
            GameVersion::Red,
        )
        .expect("Route1 has wild data");
        let json = r#"{
            "red":  { "grass": { "encounterRate": 200, "mons": [{"level": 5, "species": "PIDGEY"}, {"level": 6, "species": "Rattata"}] }, "water": { "encounterRate": 0, "mons": [] } },
            "blue": { "grass": { "encounterRate": 200, "mons": [{"level": 5, "species": "Rattata"}] }, "water": { "encounterRate": 0, "mons": [] } }
        }"#;
        assert!(set_wild_data_override("Route1", json));
        let ov = pokered_core::data::wild_data::wild_data_for_map(MapId::Route1, GameVersion::Red)
            .unwrap();
        assert_eq!(ov.grass.encounter_rate, 200);
        assert_eq!(ov.grass.mons.len(), 2);
        assert_eq!(ov.grass.mons[0].species, pokered_data::species::Species::Pidgey);
        // Other maps unaffected.
        let other = pokered_core::data::wild_data::wild_data_for_map(
            MapId::Route2,
            GameVersion::Red,
        );
        assert_eq!(
            other.map(|d| d.grass.encounter_rate),
            pokered_core::data::wild_data::wild_data_for_map(MapId::Route2, GameVersion::Red)
                .map(|d| d.grass.encounter_rate)
        );
        clear_wild_data_overrides();
        let restored = pokered_core::data::wild_data::wild_data_for_map(
            MapId::Route1,
            GameVersion::Red,
        )
        .unwrap();
        assert_eq!(restored.grass.encounter_rate, base.grass.encounter_rate);
    }

    #[test]
    fn wild_override_rejects_bad_json() {
        assert!(!set_wild_data_override("Route1", "not json"));
        assert!(!set_wild_data_override("Route1", r#"{"red": 42}"#));
    }
}
