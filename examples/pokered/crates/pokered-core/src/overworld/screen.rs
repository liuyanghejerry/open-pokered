//! OverworldScreen struct definition, constructors, property methods, and
//! supporting type definitions.
//!
//! The game-loop (`update_frame`) and its helpers live in `update.rs`.

use jrpg_engine::overworld::map_transitions::{
    ConnectionTransition as EngineConnectionTransition,
};
use jrpg_engine::overworld::{
    MapData as EngineMapData, OverworldState as EngineOverworldState,
    Direction, MovementState, NpcDefinition,
};
use jrpg_engine::overworld::types::TransportMode;
use jrpg_engine::trigger_manager::TriggerManager;
use jrpg_engine::GameData;
use jrpg_engine::overworld::collision::CollisionProvider;
use jrpg_engine_script::{CutsceneManager, MapScriptConfig, ScriptEngine, ScriptLoader};
use pokered_data::map_flags::is_city_map;
use pokered_data::maps::MapId;
use pokered_data::music::MusicId;
use pokered_data::script_api::PokemonScriptApi;
use pokered_data::tilesets::TilesetId;
use rand::SeedableRng;
use std::collections::VecDeque;

use super::forced_bike;
use super::hm_effects;

// Re-export engine types for submodules and tests.
pub use jrpg_engine::overworld::{
    MapConnection, MapConnections, NpcMovementType, Sign, WarpPoint,
};

// ── Pokémon-specific NPC data ─────────────────────────────────────

/// Pokémon-specific NPC data stored alongside the engine's generic [`NpcDefinition`].
///
/// The engine's `NpcDefinition` contains only generic fields (sprite, position,
/// movement, facing). Game-specific fields like trainer info and item drops
/// are stored here so they can be referenced during NPC interaction and
/// runtime state construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PokemonNpcData {
    pub is_trainer: bool,
    pub trainer_class: u8,
    pub trainer_set: u8,
    pub item_id: u8,
    /// One-shot victory quip shown after this trainer is beaten in a sight or
    /// talk battle (original per-map `TrainerHeader` `TextEndBattle`). `None`
    /// for non-trainers or trainers with no converted text. Sight/talk battles
    /// only — script-driven gym leaders handle their own reward text.
    pub end_battle_text: Option<String>,
}

// ── Type aliases ──────────────────────────────────────────────────

/// Pokémon-specific runtime map data wrapper.
pub type MapData<T = TilesetId> = EngineMapData<MapId, T, MusicId>;

/// Pokémon-specific overworld state wrapper.
pub type OverworldState = EngineOverworldState<MapId>;

// ── Overworld SFX Events ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverworldSfxEvent {
    None,
    /// SFX_GO_INSIDE — standing on door tile ($0b). home/overworld.asm:PlayMapChangeSound
    GoInside,
    /// SFX_GO_OUTSIDE — non-door warp (stairs, cave). home/overworld.asm:PlayMapChangeSound
    GoOutside,
    /// SFX_COLLISION — bumped into wall. home/overworld.asm:1246,1923
    Collision,
    /// SFX_LEDGE — jumped a ledge. engine/overworld/ledges.asm:53
    Ledge,
    /// SFX_PRESS_AB — advance dialogue page. Matches Oak's speech behavior.
    TextAdvance,
}

// ── Overworld Audio Requests (script-driven music/SFX) ────────────

/// Audio requests produced by script effects and map transitions.
/// The app layer reads and drains this queue each frame to dispatch
/// to pokered-audio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverworldAudioRequest {
    /// Play a music track by string ID (e.g. "MUSIC_PALLET_TOWN").
    PlayMusic { music_id: String },
    /// Play an SFX by string ID (e.g. "SFX_STOP_ALL_MUSIC").
    PlaySound { sound_id: String },
    /// Stop all music.
    StopMusic,
    /// Fade out the current music.
    FadeOutMusic,
    /// Play the default map music for the given map (used on warp).
    PlayMapMusic { map: MapId },
    /// Play a Pokémon cry by species name (script `game.playCry(...)`).
    PlayCry { species: String },
}

// ── Overworld Game-Data Requests (script-driven bag/money mutations) ─

/// Mutations to the player's persistent game data (bag, money) requested by
/// script effects. `pokered-core` (the overworld) is pure logic and cannot
/// reach `SaveData`, so it enqueues these and the app layer drains + applies
/// them each frame — mirroring the `audio_requests` pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverworldGameDataRequest {
    /// Add `quantity` of the item with the given constant name to the bag.
    GiveItem { item: String, quantity: u8 },
    /// Remove `quantity` of the item with the given constant name from the bag.
    TakeItem { item: String, quantity: u8 },
    /// Add money (capped at the game's maximum).
    GiveMoney { amount: u32 },
    /// Subtract money (saturating at zero).
    TakeMoney { amount: u32 },
    /// Set the badge bit (0..7) in the player's badge bitfield.
    GiveBadge { badge: u8 },
    /// In-game trade: if the party holds `offered`, remove it and add `received`
    /// (nicknamed) at the offered mon's level. No-op if `offered` isn't present.
    TradePokemon {
        offered: String,
        received: String,
        nickname: String,
    },
    /// Add casino coins (capped at the game's 9999 maximum).
    GiveCoins { amount: u16 },
    /// Subtract casino coins (saturating at zero).
    TakeCoins { amount: u16 },
    /// Deposit the party member at `index` into the Day Care (removes it from
    /// the party and stores it off-party in `game_data.daycare`).
    DepositDaycare { index: u8 },
    /// Withdraw the Day Care Pokémon back into the party at its grown level.
    WithdrawDaycare,
    /// Advance the deposited Day Care Pokémon's experience by one overworld
    /// step (no-op if nothing is deposited).
    TickDaycareExp,
    /// Mark a city map as visited in `town_visited_flags` (gates the FLY
    /// destination list). Pushed on map load for maps below FIRST_ROUTE_MAP,
    /// mirroring MarkTownVisitedAndLoadToggleableObjects.
    MarkTownVisited { map: MapId },
    /// Record the map blackout/Teleport returns to (`wLastBlackoutMap`).
    /// Pushed when a script heals the party (`SetLastBlackoutMap`,
    /// engine/events/set_blackout_map.asm: the original stores `wLastMap`,
    /// the outdoor map the player entered the healing map from), except in
    /// Safari Zone rest houses. The app writes it to
    /// `game_data.last_blackout_map`.
    SetBlackoutMap { map: MapId },
}

// ── Wild Encounter ─────────────────────────────────────────────────

/// Wild encounter data ready to be passed to BattleScreen.
#[derive(Debug, Clone)]
pub struct PendingWildEncounter {
    pub species: pokered_data::species::Species,
    pub level: u8,
    /// The Viridian Old-Man catch tutorial (auto-play, guaranteed catch, not kept).
    pub old_man: bool,
    /// A fishing-rod hook (Gen-1 `wMoveMissed = 1` set by `RodResponse`,
    /// item_effects.asm:1872-1873): the battle intro shows "The hooked X
    /// attacked!" (HookedMonAttackedText) instead of "Wild X appeared!".
    pub hooked: bool,
}

/// Trainer battle data ready to be passed to BattleScreen.
/// trainer_id is a string like "OPP_RIVAL1" that maps to a trainer class and party.
#[derive(Debug, Clone)]
pub struct PendingTrainerBattle {
    pub trainer_id: String,
    /// NPC index in the overworld npcs list (for marking defeated after win).
    /// May be u8::MAX for script-triggered battles.
    pub npc_index: u8,
    /// The trainer's one-shot post-battle victory quip, if any (see
    /// [`PokemonNpcData::end_battle_text`]). Shown inside the battle victory
    /// sequence, before the prize-money text.
    pub end_battle_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PendingGivePokemon {
    pub species: pokered_data::species::Species,
    pub level: u8,
    pub nickname: Option<String>,
}

// ── Warp Fade Transition ──────────────────────────────────────────
//
// Mirrors the original game's map transition:
//   1. PlayMapChangeSound → GBFadeOutToBlack (4 palette steps × 8 frames = 32 frames)
//   2. LoadMapData (while screen is black)
//   3. GBFadeInFromWhite (3 palette steps × 8 frames = 24 frames)
//
// During fade, player input is frozen (the original sets wJoyIgnore).

/// Number of frames per fade palette step (matches FADE_DELAY_FRAMES in transition.rs).
pub const WARP_FADE_DELAY: u8 = 8;
/// Fade-out: 4 palette steps (FadePal4→FadePal1).
pub const WARP_FADE_OUT_STEPS: u8 = 4;
/// Fade-in: 3 palette steps (FadePal7→FadePal5 for InFromWhite, or 4 for InFromBlack).
pub const WARP_FADE_IN_STEPS: u8 = 3;

/// Total frames for fade-out phase.
pub const WARP_FADE_OUT_FRAMES: u8 = WARP_FADE_OUT_STEPS * WARP_FADE_DELAY;
/// Total frames for fade-out-to-white phase (GBFadeOutToWhite: 3 steps × 8).
pub const WARP_FADE_OUT_WHITE_FRAMES: u8 = 3 * WARP_FADE_DELAY;
/// Total frames for fade-in phase.
pub const WARP_FADE_IN_FRAMES: u8 = WARP_FADE_IN_STEPS * WARP_FADE_DELAY;

/// Steps granted on entering the Safari Zone (`SAFARI_STEP_COUNT` in the original).
pub const SAFARI_ZONE_STEP_COUNT: u16 = 500;
/// Safari Balls granted on entering the Safari Zone.
pub const SAFARI_ZONE_BALL_COUNT: u8 = 30;
/// Tile the player is dropped on inside the gate after the game ends.
pub const SAFARI_GATE_RETURN_X: u8 = 3;
pub const SAFARI_GATE_RETURN_Y: u8 = 1;

/// Warp transition visual state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarpFadeState {
    /// No warp transition in progress.
    Idle,
    /// Fading screen to black before loading new map.
    FadingOut { frames_remaining: u8 },
    /// Screen is fully black; map data is being swapped this frame.
    BlackScreen,
    /// Fading screen back in after loading new map.
    FadingIn { frames_remaining: u8 },
}

/// Pending warp destination, stored when a warp is detected during fade-out.
#[derive(Debug, Clone, Copy)]
pub struct PendingWarp {
    pub dest_map: MapId,
    pub dest_x: u8,
    pub dest_y: u8,
    pub save_last_map: bool,
    /// FLY / TELEPORT / DIG / ESCAPE ROPE / dungeon-warp arrivals play
    /// `EnterMapAnim`'s spin-in (BIT_FLY_WARP | BIT_DUNGEON_WARP in the
    /// original, home/overworld.asm:23-28); door and connection transitions
    /// fade in without it.
    pub arrival_spin: bool,
}

/// Pending map connection transition, stored when the player walks across a
/// map boundary. The actual map swap is deferred until the walk animation
/// completes, matching the original Game Boy's seamless scrolling behavior.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingConnection {
    pub(crate) transition: EngineConnectionTransition<MapId>,
    pub(crate) save_last_map: bool,
    pub(crate) old_map: MapId,
}

/// Pre-loaded NPCs from the destination map, rendered with a coordinate
/// offset during the connection walk so they scroll into view naturally
/// before the actual map swap occurs.
#[derive(Debug, Clone)]
pub struct ConnectionNpcPreview {
    pub npcs: Vec<crate::overworld::npc_movement::NpcRuntimeState>,
    pub step_offset_x: i32,
    pub step_offset_y: i32,
}

// ── Bedroom Dialogue (new-game intro sequence) ────────────────────

use crate::overworld::event_flags;

const TYPEWRITER_CHARS_PER_FRAME: u16 = 1;

/// Default frames between revealed characters when no frontend has pushed a
/// configured text speed. 1 preserves the historical behavior (one character
/// per frame); the app layer pushes the configured `wOptions` text delay
/// (1/3/5) every frame via `set_text_delay_frames`.
const DEFAULT_TEXT_DELAY_FRAMES: u16 = 1;

/// State machine for the new-game bedroom dialogue sequence.
/// Mirrors the original "RED is playing the SNES!" hidden event text.
#[derive(Debug, Clone)]
pub struct DialoguePage {
    pub line1: &'static str,
    pub line2: &'static str,
}

#[derive(Debug, Clone)]
pub struct BedroomDialogue {
    pages: Vec<DialoguePage>,
    current_page: usize,
    char_index: u16,
    waiting_for_input: bool,
    holding_open: bool,
    /// Frames between revealed characters (original `wOptions` text delay:
    /// 1 = fast, 3 = medium, 5 = slow — `PrintLetterDelay` in
    /// home/print_text.asm waits this many frames per letter).
    text_delay_frames: u16,
    /// Countdown until the next character may be revealed.
    delay_counter: u16,
}

impl BedroomDialogue {
    pub fn new(player_name: &str) -> Self {
        let line1 = Box::leak(format!("{} is", player_name).into_boxed_str()) as &'static str;
        Self {
            pages: vec![
                DialoguePage {
                    line1,
                    line2: "playing the SNES!",
                },
                DialoguePage {
                    line1: "...Okay!",
                    line2: "It's time to go!",
                },
            ],
            current_page: 0,
            char_index: 0,
            waiting_for_input: false,
            holding_open: false,
            text_delay_frames: DEFAULT_TEXT_DELAY_FRAMES,
            delay_counter: 0,
        }
    }

    /// Build a one-off message box from `\n`-separated text, paginated two
    /// lines per box. Used for overworld item-use messages.
    pub fn from_message(text: &str) -> Self {
        let lines: Vec<&str> = text.split('\n').collect();
        let mut pages: Vec<DialoguePage> = lines
            .chunks(2)
            .map(|c| DialoguePage {
                line1: Box::leak(c.first().copied().unwrap_or("").to_string().into_boxed_str()),
                line2: Box::leak(c.get(1).copied().unwrap_or("").to_string().into_boxed_str()),
            })
            .collect();
        if pages.is_empty() {
            pages.push(DialoguePage { line1: "", line2: "" });
        }
        Self {
            pages,
            current_page: 0,
            char_index: 0,
            waiting_for_input: false,
            holding_open: false,
            text_delay_frames: DEFAULT_TEXT_DELAY_FRAMES,
            delay_counter: 0,
        }
    }

    pub fn from_text_pages(
        text_pages: &[pokered_data::map_json::TextPageJson],
        player_name: &str,
        rival_name: &str,
        starter_name: &str,
    ) -> Self {
        let pages = text_pages
            .iter()
            .map(|tp| {
                let l1 = resolve_placeholders(&tp.line1, player_name, rival_name, starter_name);
                let l2 = resolve_placeholders(&tp.line2, player_name, rival_name, starter_name);
                DialoguePage {
                    line1: Box::leak(l1.into_boxed_str()),
                    line2: Box::leak(l2.into_boxed_str()),
                }
            })
            .collect();
        Self {
            pages,
            current_page: 0,
            char_index: 0,
            waiting_for_input: false,
            holding_open: false,
            text_delay_frames: DEFAULT_TEXT_DELAY_FRAMES,
            delay_counter: 0,
        }
    }

    pub fn from_pages(pages: Vec<DialoguePage>) -> Self {
        Self {
            pages,
            current_page: 0,
            char_index: 0,
            waiting_for_input: false,
            holding_open: false,
            text_delay_frames: DEFAULT_TEXT_DELAY_FRAMES,
            delay_counter: 0,
        }
    }

    pub fn current(&self) -> Option<&DialoguePage> {
        self.pages.get(self.current_page)
    }

    /// All pages (test/debug observability).
    pub fn pages(&self) -> &[DialoguePage] {
        &self.pages
    }

    pub fn char_index(&self) -> u16 {
        self.char_index
    }

    pub fn waiting_for_input(&self) -> bool {
        self.waiting_for_input
    }

    pub fn holding_open(&self) -> bool {
        self.holding_open
    }

    pub fn is_last_page(&self) -> bool {
        self.current_page + 1 >= self.pages.len()
    }

    pub fn start_holding_open(&mut self) {
        self.holding_open = true;
    }

    pub fn stop_holding_open(&mut self) {
        self.holding_open = false;
    }

    pub fn get_display_text(&self) -> Option<(String, String)> {
        let page = self.pages.get(self.current_page)?;
        let idx = self.char_index as usize;
        let l1_chars: Vec<char> = page.line1.chars().collect();
        let l2_chars: Vec<char> = page.line2.chars().collect();
        if idx <= l1_chars.len() {
            Some((l1_chars[..idx].iter().collect(), String::new()))
        } else {
            let l2_idx = idx - l1_chars.len();
            let d2: String = if l2_idx <= l2_chars.len() {
                l2_chars[..l2_idx].iter().collect()
            } else {
                page.line2.to_string()
            };
            Some((page.line1.to_string(), d2))
        }
    }

    pub fn total_chars(&self) -> u16 {
        self.pages.get(self.current_page).map_or(0, |p| {
            (p.line1.chars().count() + p.line2.chars().count()) as u16
        })
    }

    /// Set the frames between revealed characters (the `wOptions` text delay,
    /// 1/3/5). The app layer pushes this from the configured text speed every
    /// frame; values below 1 are clamped to 1 (one character per frame).
    pub fn set_text_delay_frames(&mut self, frames: u16) {
        self.text_delay_frames = frames.max(1);
    }

    pub fn text_delay_frames(&self) -> u16 {
        self.text_delay_frames
    }

    /// Advance the typewriter by one frame. Honors the configured text speed:
    /// after a character is revealed, `text_delay_frames - 1` frames pass
    /// before the next one (one character per `text_delay_frames` frames,
    /// matching `PrintLetterDelay`'s per-letter `DelayFrames` wait).
    pub fn reveal_next_char(&mut self) {
        if self.delay_counter > 0 {
            self.delay_counter -= 1;
            return;
        }
        let total = self.total_chars();
        if self.char_index < total {
            self.char_index += TYPEWRITER_CHARS_PER_FRAME;
            if self.char_index >= total {
                self.char_index = total;
                self.waiting_for_input = true;
            } else {
                self.delay_counter = self.text_delay_frames - 1;
            }
        }
    }

    pub fn skip_to_full_page(&mut self) {
        self.char_index = self.total_chars();
        self.waiting_for_input = true;
    }

    pub fn advance(&mut self) -> bool {
        self.current_page += 1;
        self.char_index = 0;
        self.waiting_for_input = false;
        self.delay_counter = 0;
        self.current_page < self.pages.len()
    }

    pub fn has_more_pages(&self) -> bool {
        self.current_page + 1 < self.pages.len()
    }

    pub fn is_done(&self) -> bool {
        self.current_page >= self.pages.len()
    }
}

fn resolve_placeholders(text: &str, player_name: &str, rival_name: &str, starter_name: &str) -> String {
    text.replace("<PLAYER>", player_name)
        .replace("<RIVAL>", rival_name)
        .replace("<STARTER>", starter_name)
}

// ── Misc state structs ────────────────────────────────────────────

use crate::overworld::presentation;
use crate::overworld::special_terrain;

pub struct PokedexEntryState {
    pub species: String,
    pub page: usize,
    /// Total description pages (set by renderer on first draw so input handler
    /// knows when A/B should close the screen rather than advance).
    pub total_pages: usize,
}

pub struct EmotionBubbleState {
    pub npc_id: String,
    pub emotion: String,
    pub frames_remaining: u16,
}

pub use crate::overworld::script_bridge::HealingMachinePhase;

pub struct HealingMachineState {
    pub phase: HealingMachinePhase,
    pub frames_remaining: u16,
    pub pokeballs_visible: u8,
    pub flash_active: bool,
}

// ── OverworldScreen struct ────────────────────────────────────────

use crate::overworld::collision;

pub struct OverworldScreen<G: GameData = pokered_data::impl_traits::PokemonRedData> {
    pub(crate) game_data: G,
    pub state: OverworldState,
    pub map_data: Option<MapData<G::Tileset>>,
    pub npc_states: Vec<crate::overworld::npc_movement::NpcRuntimeState>,
    pub npc_pokemon_data: Vec<PokemonNpcData>,
    pub pending_dialogue: Option<BedroomDialogue>,
    pub pending_choice: Option<crate::overworld::script_bridge::PendingChoice>,
    pub pending_pokedex_entry: Option<PokedexEntryState>,
    pub pending_naming_screen: Option<crate::naming_screen::NamingScreenState>,
    /// Remaining frames of the white flash when the naming screen opens and
    /// after it submits (`GBPalWhiteOutWithDelay3`, naming_screen.asm:88/163):
    /// the renderer whites the screen while > 0 and gameplay stays frozen.
    pub naming_flash_frames: u8,
    /// Active party selector (Name Rater). Driven by the app via
    /// `update_party_select_input` while `is_party_select_active()` is true.
    pub pending_party_select: Option<crate::party_select::PartySelectState>,
    /// Set when a script requests a party selection; the app layer must call
    /// `begin_party_select(party)` to hand in the party members to choose from.
    pub party_select_requested: bool,
    /// Set when a Cable Club "gameboy" script calls `game.linkStart()`; the
    /// app layer drains it with `take_link_start_request()` and starts the
    /// link battle/trade flow (which owns the session).
    pub link_start_requested: bool,
    /// Overworld placement override for the remote player's avatar in the
    /// Cable Club rooms (Colosseum / TradeCenter). Set by the app while a
    /// link session is connected and the player is inside one of the rooms;
    /// applied every frame to the room's opponent NPC (index 1). `None`
    /// keeps the map's static NPC at its configured spot.
    pub link_opponent: Option<crate::link::LinkOpponentPresence>,
    /// A `(party_index, nickname)` write requested by a script (Name Rater),
    /// drained + applied by the app layer (which owns the party).
    pub pending_set_nickname: Option<(u8, String)>,
    pub pending_emotion_bubble: Option<EmotionBubbleState>,
    pub pending_healing_machine: Option<HealingMachineState>,
    pub last_map: Option<MapId>,
    /// Position on `last_map` where the player stepped onto the entrance warp —
    /// the tile just outside a dungeon/building. Recorded alongside `last_map`
    /// and used as the ESCAPE ROPE return point.
    pub last_map_entry: Option<(u8, u8)>,
    pub warp_fade_state: WarpFadeState,
    pub pending_warp: Option<PendingWarp>,
    pub(crate) pending_connection: Option<PendingConnection>,
    pub connection_npc_preview: Option<ConnectionNpcPreview>,
    pub pending_wild_encounter: Option<PendingWildEncounter>,
    pub pending_trainer_battle: Option<PendingTrainerBattle>,
    pub pending_give_pokemon: Option<PendingGivePokemon>,
    pub sfx_event: OverworldSfxEvent,
    pub bump_anim_counter: u8,
    pub player_name: String,
    pub rival_name: String,
    pub frame_counter: u32,
    /// Frames between revealed dialogue typewriter characters — the frontend
    /// pushes the configured text speed here every frame (original `wOptions`
    /// text delay: 1 = fast, 3 = medium, 5 = slow); applied to any active
    /// `pending_dialogue` at the top of `update_frame`.
    pub text_delay_frames: u16,
    pub(crate) prev_a_pressed: bool,
    pub(crate) prev_movement_state: MovementState,
    pub(crate) prev_b_pressed: bool,
    pub(crate) prev_up_pressed: bool,
    pub(crate) prev_down_pressed: bool,
    pub(crate) script_engine: ScriptEngine,
    pub(crate) script_loader: ScriptLoader,
    pub(crate) scene_script_provider: pokered_data::scene_loader::SceneScriptProvider,
    pub(crate) map_script_config: MapScriptConfig,
    pub(crate) cutscene_manager: CutsceneManager,
    pub(crate) trigger_manager: TriggerManager,
    pub(crate) active_script_effect: Option<crate::overworld::script_bridge::ScriptEffect>,
    pub(crate) joy_ignore_mask: u8,
    pub(crate) scripts_dir: Option<std::path::PathBuf>,
    pub(crate) scripted_player_path: VecDeque<(u16, u16)>,
    pub audio_requests: Vec<OverworldAudioRequest>,
    /// Bag/money mutations requested by scripts, drained + applied by the app layer.
    pub game_data_requests: Vec<OverworldGameDataRequest>,
    /// True while a script is suspended on `await game.startBattle(...)`,
    /// waiting for the battle to end so it can be resumed with the outcome.
    pub script_awaiting_battle: bool,
    /// Snapshot of the player's bag (item const names), seeded each frame by the
    /// app layer alongside `seed_set("bag", ...)`. Lets `giveItem(...)` report
    /// bag-full success/failure synchronously (matching `hasItem`) so scenes'
    /// `@if (given = giveItem(...))` "no room" branches work correctly.
    pub(crate) script_bag_names: Vec<String>,
    /// Snapshot of the party's species (UPPERCASE names), seeded each frame, so
    /// script queries (`@if` conditions over the party) can inspect it
    /// (mirrors `script_bag_names` / `hasItem`).
    pub(crate) script_party_species: Vec<String>,
    /// The player's chosen starter species id (`game_data.player_starter`),
    /// seeded each frame alongside `seed_script_query_state`. Used to resolve
    /// the `<STARTER>` placeholder; 0 means unset (old saves).
    pub(crate) player_starter: u8,
    pub pending_shop: Option<Vec<String>>,
    /// Set by `game.openSlots(lucky)`; the app opens the slot-machine screen.
    pub pending_slots: Option<bool>,
    /// Set by `game.elevatorMenu(floors)`; the app opens the elevator floor
    /// menu. The script stays suspended until the app calls
    /// `resume_script_after_elevator` with the chosen floor index.
    pub pending_elevator: Option<Vec<String>>,
    /// True while the script is suspended on an `elevatorMenu` await, waiting
    /// for the app to return the chosen floor.
    pub script_awaiting_elevator: bool,
    /// Set by `game.filterBag(itemIds)`; the app opens a filtered-bag menu
    /// showing only the carried candidates. The script resumes via
    /// `resume_script_after_filter_bag` with the chosen item's const name.
    pub pending_filter_bag: Option<Vec<String>>,
    /// True while the script is suspended on a `filterBag` await.
    pub script_awaiting_filter_bag: bool,
    /// True while the script is suspended on a `tradePokemon(...)` await,
    /// waiting for the app to play the trade cutscene, apply the party
    /// mutation, and resume via `resume_script_after_trade`.
    pub script_awaiting_trade: bool,
    /// Set by `game.showDiploma()`; the app opens the full-screen diploma.
    pub pending_diploma: bool,
    /// Set by `game.openPC()` / `game.openItemPC()`; the app opens the PC
    /// storage screen ("center" / "items" / "bills"). Instant effect — the
    /// script has already run on, so no resume call is needed.
    pub pending_pc: Option<String>,
    /// Set by `game.enterHallOfFame()`; the app records the party in the
    /// Hall of Fame, plays the roll-call movie + credits, saves, and resets
    /// to the title screen. Instant effect — no resume call is needed.
    pub pending_hof_ceremony: bool,
    pub heal_requested: bool,
    pub party_count: u8,
    /// Level of the lead (first) party Pokémon, kept in sync by the app layer.
    /// Used for the Gen-1 repel check, which blocks wild encounters whose level
    /// is below the lead party member's level. The OverworldScreen does not own
    /// the full party (only the app/save layer does), so the app threads this in
    /// wherever it syncs `party_count`.
    pub party_lead_level: u8,
    pub(crate) unified_flags: event_flags::EventFlags,
    pub(crate) toggleable_object_flags: [u8; pokered_data::toggleable_objects::TOGGLEABLE_OBJECT_FLAGS_SIZE],
    /// Runtime copy of the save's `obtained_hidden_items` bitfield
    /// (`wObtainedHiddenItemsFlags`), seeded by the app at load and read back
    /// at save — same pattern as `toggleable_object_flags`.
    pub(crate) hidden_item_flags: [u8; crate::save::game_data::HIDDEN_ITEMS_BYTES],
    /// ITEMFINDER ding sequencer: `(dings_remaining, frames_until_next)`. The
    /// original blocks on each SFX (PlaySoundWaitForCurrent, 4× HEALING_MACHINE
    /// + PURCHASE); here one ding is emitted every ITEMFINDER_DING_FRAMES.
    pub(crate) itemfinder_dings: Option<(u8, u8)>,
    pub(crate) rng: rand::rngs::StdRng,
    /// Remaining Safari Zone steps (of [`SAFARI_ZONE_STEP_COUNT`]). Counts down
    /// once per completed step while the player is inside the Safari Zone.
    pub(crate) safari_steps: u16,
    /// Remaining Safari Balls (of [`SAFARI_ZONE_BALL_COUNT`]). Decremented by the
    /// battle layer when a ball is thrown; the game ends when it (or the step
    /// counter) reaches zero.
    pub(crate) safari_balls: u8,
    /// True while a Safari Zone "game" is in progress (between paying at the
    /// gate and being ejected). Prevents the step counter from re-arming.
    pub(crate) safari_game_active: bool,
    /// Deferred eject warp, set when the Safari game ends. It fires (as a normal
    /// fade-warp to the gate) once the "SAFARI GAME is over!" message closes.
    pub(crate) safari_eject_pending: Option<PendingWarp>,
    /// wStatusFlags1 BIT_STRENGTH_ACTIVE — set by using STRENGTH in the field,
    /// cleared on every map change (ResetUsingStrengthOutOfBattleBit, called
    /// from EnterMap in home/overworld.asm).
    pub strength_active: bool,
    /// wMiscFlags BIT_TRIED_PUSH_BOULDER — the player must "push twice"
    /// (two consecutive frames) before a boulder moves
    /// (engine/overworld/push_boulder.asm).
    pub(crate) tried_push_boulder: bool,
    /// Frames remaining of the boulder-dust lockout after a successful push
    /// (wMiscFlags BIT_BOULDER_DUST — pushing is blocked while it plays).
    pub(crate) boulder_dust_frames: u8,
    /// The boulder-push smoke puff (`AnimateBoulderDust`, dust_smoke.asm) —
    /// a frame-stepped 2×2 smoke-tile block anchored to the push spot. The
    /// renderer draws it while [`BoulderDustState::is_active`]; `update.rs`
    /// ticks it every frame.
    pub boulder_dust: presentation::BoulderDustState,
    /// Dark-cave palette state (wMapPalOffset): set on entering Rock Tunnel,
    /// cleared by FLASH or by leaving. The darkened *rendering* is a
    /// renderer-side follow-up; this tracks the logic state.
    pub dark_cave: special_terrain::DarkCaveState,
    /// wStatusFlags6 BIT_ALWAYS_ON_BIKE — the Cycling Road forced-bike lock
    /// (CheckForceBikeOrSurf on map entry; released by the gates, FLY/DIG/
    /// TELEPORT and blackout). While active the BICYCLE item is refused and
    /// SURF is blocked.
    pub forced_bike: forced_bike::ForcedBikeState,
    /// Remaining frames of the all-white screen flash after FIELD FLASH lit a
    /// dark cave (`GBPalWhiteOutWithDelay3`, home/palettes.asm). Started once
    /// the "blinding FLASH" text is dismissed.
    pub flash_lit_frames: u8,
    /// Set by field FLASH when it lights a cave; converted into
    /// `flash_lit_frames` when the message dialogue closes (the original
    /// whites out after PrintText returns).
    pub(crate) flash_pending_white: bool,
    /// True when the current warp fade-out goes to WHITE instead of black
    /// (escape warps / FLY: `_LeaveMapAnim` ends in `GBFadeOutToWhite`;
    /// normal tile warps use `GBFadeOutToBlack` via PlayMapChangeSound).
    pub warp_fade_to_white: bool,
    /// Active TELEPORT/DIG/ESCAPE ROPE leave-map spin animation
    /// (`_LeaveMapAnim`). While `Some`, the warp fade is deferred and player
    /// input is frozen.
    pub teleport_spin: Option<presentation::TeleportSpinState>,
    /// Active `EnterMapAnim` arrival spin-in (FLY / TELEPORT / DIG / ESCAPE
    /// ROPE / dungeon-warp arrivals). Created when the warp commits; ticked
    /// once the fade-in-from-white completes. The player stays hidden during
    /// the fade (`Y=$ec`) and descends while this is active.
    pub enter_map_anim: Option<presentation::EnterMapSpinState>,
    /// Active `ShakeElevator` animation (screen BG shake after riding an
    /// elevator). While `Some`, player input is frozen.
    pub elevator_shake: Option<presentation::ElevatorShakeState>,
    /// Set when an elevator floor was chosen; the shake starts once the
    /// elevator warp has finished (the original runs ShakeElevator from the
    /// elevator map's script on re-entry after BIT_CUR_MAP_USED_ELEVATOR).
    pub(crate) elevator_shake_pending: bool,
    /// Frames remaining of the 80-frame pause between the rod's "You used the
    /// <ROD>!" text and `FishingAnim` (`FishingInit`'s `ld c, 80;
    /// call DelayFrames`, item_effects.asm:1910-1911). The pause freezes
    /// gameplay like the original's blocking DelayFrames.
    pub(crate) fishing_cast_delay: u16,
    /// Water/flower tile animation state (`UpdateMovingBgTiles`,
    /// home/vcopy.asm). Ticked every frame; the renderer derives the current
    /// water rotation and flower frame from it.
    pub tile_anim: presentation::TileAnimState,
    /// Deferred warp fired once the current dialogue closes — used by field
    /// TELEPORT, which prints "Warp to the last #MON CENTER." and warps only
    /// after the text is dismissed (start_sub_menus.asm .teleport).
    pub(crate) post_dialogue_warp: Option<PendingWarp>,
    /// Deferred wild battle fired once the current dialogue closes — used by
    /// the fishing rods, which print "Oh! It's a bite!" and start the hooked
    /// mon's battle only after the text is dismissed (home/overworld.asm's
    /// `.newBattle` on `wCurOpponent != 0` after the item menu closes).
    pub(crate) post_dialogue_battle: Option<PendingWildEncounter>,
    /// Active fishing rod animation (`FishingAnim`, player_animations.asm:
    /// 378-469). While `Some`, gameplay is frozen (the original runs the
    /// animation as blocking `DelayFrames` loops); on completion the result
    /// text (and, on a bite, `post_dialogue_battle`) is queued.
    pub fishing_anim: Option<presentation::FishingAnimState>,
    /// A rod use whose animation has not played yet: the response is rolled
    /// at item use time (`RodResponse`, item_effects.asm:1869-1877), the
    /// "You used the <ROD>!" text shows, and `fishing_anim` starts once that
    /// dialogue closes (the original prints the item-use text, then runs the
    /// blocking `FishingAnim`, then prints the result text).
    pub(crate) pending_fishing: Option<crate::overworld::fishing::PendingFishing>,
    /// Active S.S. Anne departure cutscene
    /// (`VermilionDockSSAnneLeavesScript` + `VermilionDock_EraseSSAnne`,
    /// scripts/VermilionDock.asm). Started by the dock scene's
    /// `playShipDeparture()` effect; while `Some`, gameplay is frozen
    /// (the original's blocking animation routine). `update.rs` applies
    /// the ship erase and the dock→ship warp removal when the erase phase
    /// begins; the renderers draw the scroll + smoke puffs from it.
    pub ship_departure: Option<presentation::ShipDepartureState>,
}

// ── Constructor + accessor methods ────────────────────────────────

impl<G: GameData<Tileset = TilesetId>> OverworldScreen<G> {
    pub fn new(start_map: MapId, scripts_dir: Option<std::path::PathBuf>, game_data: G) -> Self {
        log::info!(target: "pokered::overworld", "[Overworld] Creating new OverworldScreen for {:?}", start_map);
        let (map, npc_pokemon_data) =
            crate::overworld::map_data_loading::load_full_map_data(start_map, game_data.tileset_provider());
        let map_data = Some(map);

        let mut scene_provider = pokered_data::scene_loader::SceneScriptProvider::new();
        let mut has_scenes = false;
        if let Some(ref dir) = scripts_dir {
            match scene_provider.load_from_directory(dir) {
                Ok(count) if count > 0 => {
                    log::info!(target: "pokered::overworld", "[SceneLoader] loaded {} .scene files", count);
                    has_scenes = true;
                }
                Ok(_) => log::info!(target: "pokered::overworld", "[SceneLoader] no .scene files found, falling back to .js"),
                Err(e) => log::warn!(target: "pokered::overworld", "[SceneLoader] .scene load error: {}", e),
            }
        }

        let mut script_loader = ScriptLoader::new();
        if has_scenes {
            for (map_id, js) in &scene_provider.scenes {
                script_loader.register_script(map_id, js);
                // Also load the map's `script_config.json` (npc/coord/sign
                // trigger bindings). Without it no triggers are built, so
                // disk-loaded (`--scripts-dir`) scenes would have no working
                // interactions — the embedded path already bakes these configs.
                if let Some(ref dir) = scripts_dir {
                    let cfg_path = dir.join(map_id).join("script_config.json");
                    if let Ok(json) = std::fs::read_to_string(&cfg_path) {
                        if let Err(e) = script_loader.register_config_json(map_id, &json) {
                            log::warn!(target: "pokered::overworld", "[SceneLoader] config parse error for {}: {}", map_id, e);
                        }
                    }
                }
            }
        } else {
            // Default path (no --scripts-dir): compiled `.scene` JS and
            // `script_config.json` embedded into pokered-data at build time.
            // This serves every frontend (native, TUI, wasm) — the legacy
            // `load_auto` fallback below only covers setups that still ship
            // hand-written `script.js` files (none remain in this repo).
            let mut script_count = 0usize;
            for (map_id, js) in pokered_data::embedded_scenes::scene_scripts() {
                script_loader.register_script(map_id, js);
                script_count += 1;
            }
            let mut config_count = 0usize;
            for (map_id, json) in pokered_data::embedded_scenes::scene_configs() {
                match script_loader.register_config_json(map_id, json) {
                    Ok(()) => config_count += 1,
                    Err(e) => log::warn!(target: "pokered::overworld", "[SceneLoader] embedded config parse error for {}: {}", map_id, e),
                }
            }
            log::info!(target: "pokered::overworld", "[SceneLoader] registered {} embedded scenes, {} configs", script_count, config_count);
            if script_count == 0 {
                match script_loader.load_auto(scripts_dir.as_deref()) {
                    Ok(count) => log::info!(target: "pokered::overworld", "[ScriptLoader] loaded {} .js files via load_auto", count),
                    Err(e) => log::warn!(target: "pokered::overworld", "[ScriptLoader] load_auto failed: {}", e),
                }
            }
        }

        let mut script_engine = ScriptEngine::with_api(&PokemonScriptApi);
        let map_key = crate::overworld::script_bridge::map_id_to_script_key(start_map);
        if let Some(source) = script_loader.get_script(&map_key) {
            let _ = script_engine.load_script(source);
        }
        let map_script_config = script_loader
            .get_config(&map_key)
            .cloned()
            .unwrap_or_default();

        let hidden_npc_ids = map_script_config.hidden_npc_ids();
        let npc_states = map_data
            .as_ref()
            .map(|md| build_npc_runtime_states(&md.npcs, &npc_pokemon_data, &hidden_npc_ids))
            .unwrap_or_default();

        let mut dark_cave = special_terrain::DarkCaveState::new();
        dark_cave.enter_map(start_map);
        // LoadTilesetHeader: hTileAnimations follows the start map's tileset.
        let mut tile_anim = presentation::TileAnimState::new();
        if let Some(ref md) = map_data {
            tile_anim.set_tileset(
                pokered_data::tileset_data::get_tileset_header(md.tileset).animation,
            );
        }
        let mut game_data_requests = Vec::new();
        if is_city_map(start_map) {
            game_data_requests.push(OverworldGameDataRequest::MarkTownVisited { map: start_map });
        }

        let mut screen = Self {
            game_data,
            state: OverworldState::new(start_map),
            map_data,
            npc_states,
            npc_pokemon_data,
            pending_dialogue: None,
            pending_choice: None,
            pending_pokedex_entry: None,
            pending_naming_screen: None,
            naming_flash_frames: 0,
            pending_party_select: None,
            party_select_requested: false,
            link_start_requested: false,
            link_opponent: None,
            pending_set_nickname: None,
            pending_emotion_bubble: None,
            pending_healing_machine: None,
            last_map: Some(MapId::PalletTown),
            last_map_entry: None,
            warp_fade_state: WarpFadeState::Idle,
            pending_warp: None,
            pending_connection: None,
            connection_npc_preview: None,
            pending_wild_encounter: None,
            pending_trainer_battle: None,
            pending_give_pokemon: None,
            sfx_event: OverworldSfxEvent::None,
            bump_anim_counter: 0,
            player_name: "RED".to_string(),
            rival_name: "BLUE".to_string(),
            frame_counter: 0,
            text_delay_frames: DEFAULT_TEXT_DELAY_FRAMES,
            prev_a_pressed: false,
            prev_movement_state: MovementState::Idle,
            prev_b_pressed: false,
            prev_up_pressed: false,
            prev_down_pressed: false,
            script_engine,
            script_loader,
            scene_script_provider: scene_provider,
            map_script_config,
            cutscene_manager: CutsceneManager::new(),
            trigger_manager: TriggerManager::new(),
            active_script_effect: None,
            joy_ignore_mask: 0,
            scripts_dir,
            scripted_player_path: VecDeque::new(),
            audio_requests: Vec::new(),
            game_data_requests,
            script_awaiting_battle: false,
            script_awaiting_elevator: false,
            script_awaiting_filter_bag: false,
            script_awaiting_trade: false,
            script_bag_names: Vec::new(),
            script_party_species: Vec::new(),
            player_starter: 0,
            pending_shop: None,
            pending_slots: None,
            pending_elevator: None,
            pending_filter_bag: None,
            pending_diploma: false,
            pending_pc: None,
            pending_hof_ceremony: false,
            heal_requested: false,
            party_count: 0,
            party_lead_level: 0,
            unified_flags: event_flags::EventFlags::new(),
            toggleable_object_flags: [0u8;
                pokered_data::toggleable_objects::TOGGLEABLE_OBJECT_FLAGS_SIZE],
            hidden_item_flags: [0u8; crate::save::game_data::HIDDEN_ITEMS_BYTES],
            itemfinder_dings: None,
            rng: rand::rngs::StdRng::from_entropy(),
            safari_steps: 0,
            safari_balls: 0,
            safari_game_active: false,
            safari_eject_pending: None,
            strength_active: false,
            tried_push_boulder: false,
            boulder_dust_frames: 0,
            boulder_dust: presentation::BoulderDustState::inactive(),
            dark_cave,
            forced_bike: forced_bike::ForcedBikeState::default(),
            flash_lit_frames: 0,
            flash_pending_white: false,
            warp_fade_to_white: false,
            teleport_spin: None,
            enter_map_anim: None,
            elevator_shake: None,
            elevator_shake_pending: false,
            fishing_cast_delay: 0,
            tile_anim,
            post_dialogue_warp: None,
            post_dialogue_battle: None,
            fishing_anim: None,
            pending_fishing: None,
            ship_departure: None,
        };
        // EnterMap: CheckForceBikeOrSurf runs for the start map too — mount/
        // lock the bike if the screen starts on a Cycling Road tile.
        screen.apply_map_entry_transport(start_map, screen.state.player.x, screen.state.player.y);
        screen
    }

    /// `CheckForceBikeOrSurf` applied to the player transport: entering a
    /// Cycling Road tile mounts and locks the bike; a Seafoam current tile
    /// forces the surf state (e.g. falling through a floor hole into the
    /// water — `wWalkBikeSurfState = 2`); entering either gate (or leaving
    /// by FLY/DIG/TELEPORT / blackout) releases the lock and restores
    /// walking. Runs from `OverworldScreen::new`, `commit_pending_warp` and
    /// the connection-walk map swap.
    pub(crate) fn apply_map_entry_transport(&mut self, map: MapId, x: u16, y: u16) {
        match self.forced_bike.enter_map(map, x, y) {
            forced_bike::ForcedBikeMapEntry::Mount => {
                self.state.player.transport = TransportMode::Biking;
            }
            forced_bike::ForcedBikeMapEntry::ForceSurf => {
                self.state.player.transport = TransportMode::Surfing;
            }
            forced_bike::ForcedBikeMapEntry::Dismount => {
                self.state.player.transport = TransportMode::Walking;
            }
            forced_bike::ForcedBikeMapEntry::Keep => {}
        }
    }

    /// CollisionCheckOnWater .stopSurfing across a map connection: a surfer
    /// who crossed onto a passable land tile dismounts on arrival. The new
    /// map's music was already queued by the map swap, so no audio change is
    /// needed here.
    pub(crate) fn dismount_surf_if_on_land(&mut self) {
        if self.state.player.transport != TransportMode::Surfing {
            return;
        }
        let Some(ref md) = self.map_data else {
            return;
        };
        let provider =
            collision::PokemonCollisionProvider::new(self.state.current_map, md.tileset);
        let standing = provider.get_tile_at_position(
            md.tileset,
            &md.blocks,
            md.width,
            self.state.player.x,
            self.state.player.y,
        );
        if !provider.is_water_tile(md.tileset, standing) {
            self.state.player.transport = TransportMode::Walking;
        }
    }

    pub fn set_script_lang(&mut self, lang: &str) {
        self.script_engine.set_lang(lang);
    }

    /// Push the configured text speed (frames between revealed characters —
    /// 1/3/5) into any active dialogue. Called by the frontend every frame.
    pub fn set_text_delay_frames(&mut self, frames: u16) {
        self.text_delay_frames = frames.max(1);
    }

    /// Seed the script engine's synchronous query state from the persistent
    /// game data. The app layer calls this each frame *before* `update_frame`
    /// so `@if`-style script conditions (`hasItem`, `getMoney`, `hasMoney`,
    /// `getPokedexOwnedCount`, `getPokedexSeenCount`, `getPlayerFacing`,
    /// `getRivalStarter`, `getBadgeCount`) see up-to-date values.
    pub fn seed_script_query_state(
        &mut self,
        money: u32,
        bag_const_names: &[String],
        dex_owned: u8,
        dex_seen: u8,
        rival_starter: u8,
        player_starter: u8,
        party_species: &[String],
        coins: u16,
        obtained_badges: u8,
    ) {
        let facing = match self.state.player.facing {
            Direction::Up => "up",
            Direction::Down => "down",
            Direction::Left => "left",
            Direction::Right => "right",
        };
        self.script_engine.seed_number("money", money as f64);
        self.script_engine.seed_number("coins", coins as f64);
        self.script_engine.seed_set("bag", bag_const_names);
        self.script_bag_names = bag_const_names.to_vec();
        self.script_engine.seed_set("party", party_species);
        self.script_party_species = party_species.to_vec();
        self.script_engine
            .seed_number("pokedexOwned", dex_owned as f64);
        self.script_engine
            .seed_number("pokedexSeen", dex_seen as f64);
        self.script_engine.seed_text("playerFacing", facing);
        self.script_engine
            .seed_number("rivalStarter", rival_starter as f64);
        self.script_engine
            .seed_number("playerStarter", player_starter as f64);
        self.player_starter = player_starter;
        self.script_engine
            .seed_number("obtainedBadges", obtained_badges as f64);
        // Feed real entropy from the overworld RNG into the script-side RNG so
        // `game.showRandomText(...)` (flavor-text pools) varies between plays.
        // Scripts have no Math.random/Date.now, so randomness must come from here.
        self.script_engine
            .mix_rng(rand::RngCore::next_u64(&mut self.rng));
    }

    /// Display name of the player's chosen starter Pokémon, used to resolve
    /// the `<STARTER>` placeholder in dialogue text (e.g. Oak's "first left
    /// with <STARTER>!" line in ChampionsRoom). Uses `player_starter` from the
    /// save data; falls back to the lead party member when unset (old saves),
    /// then to the empty string when the party is empty or unknown.
    pub fn starter_display_name(&self) -> String {
        if self.player_starter != 0 {
            return pokered_data::species::Species::from_index_id(self.player_starter).pascal_name();
        }
        self.script_party_species.first().cloned().unwrap_or_default()
    }

    /// Seed the Day Care + per-party query state consumed by the Day Care
    /// scene (`isDaycareInUse`, `getDaycareMonName`, `getDaycareLevelsGrown`,
    /// `getDaycareCost`, `getPartyCount`, `getPartyMonName`,
    /// `partyMonKnowsHm`). The app layer computes the grown-levels/cost from
    /// `game_data.daycare` and passes per-party display names + HM flags.
    /// Called each frame before `update_frame`, alongside
    /// [`Self::seed_script_query_state`].
    #[allow(clippy::too_many_arguments)]
    pub fn seed_daycare_query_state(
        &mut self,
        in_use: bool,
        mon_name: &str,
        levels_grown: u8,
        cost: u32,
        party_names: &[String],
        party_knows_hm: &[bool],
    ) {
        self.script_engine
            .seed_number("daycareInUse", if in_use { 1.0 } else { 0.0 });
        self.script_engine.seed_text("daycareMonName", mon_name);
        self.script_engine
            .seed_number("daycareLevelsGrown", levels_grown as f64);
        self.script_engine.seed_number("daycareCost", cost as f64);
        self.script_engine
            .seed_number("partyCount", party_names.len() as f64);
        for (i, name) in party_names.iter().enumerate() {
            self.script_engine.seed_text(&format!("partyName{i}"), name);
        }
        for (i, hm) in party_knows_hm.iter().enumerate() {
            self.script_engine
                .seed_number(&format!("partyKnowsHm{i}"), if *hm { 1.0 } else { 0.0 });
        }
    }

    pub fn reload_scene_script(&mut self, map_key: &str, js: &str) {
        self.script_loader.register_script(map_key, js);
        if map_key == crate::overworld::script_bridge::map_id_to_script_key(self.state.current_map) {
            let _ = self.script_engine.load_script(js);
        }
    }

    /// Editor hot-reload: register (or override) a compiled scene script and
    /// optional `script_config.json` for a map key, then — when the key is
    /// the current map — fully reload the script engine, config and triggers
    /// (`load_map_script`), so a saved `.scene` edit shows up in the running
    /// game without a rebuild. This is the runtime seam the browser editor's
    /// WYSIWYG preview feeds after saving a script.
    pub fn hot_reload_map_scripts(
        &mut self,
        map_key: &str,
        js: &str,
        config_json: Option<&str>,
    ) -> Result<(), String> {
        self.script_loader.register_script(map_key, js);
        if let Some(json) = config_json {
            self.script_loader
                .register_config_json(map_key, json)
                .map_err(|e| format!("config JSON for '{map_key}': {e}"))?;
        }
        if map_key == crate::overworld::script_bridge::map_id_to_script_key(self.state.current_map)
        {
            self.load_map_script(self.state.current_map);
        }
        Ok(())
    }

    /// Editor warp: teleport the player to `map` at block coordinates `x`,
    /// `y` through the normal fade transition (same as stepping on a warp
    /// tile — sets the pending warp AND starts the fade-out; without the
    /// fade state the warp would never be committed).
    pub fn warp_to_map(&mut self, map: MapId, x: u8, y: u8) {
        self.pending_warp = Some(PendingWarp {
            dest_map: map,
            dest_x: x,
            dest_y: y,
            save_last_map: false,
            // Editor/debug warp: plain fade transition, no EnterMapAnim.
            arrival_spin: false,
        });
        self.warp_fade_state = WarpFadeState::FadingOut {
            frames_remaining: WARP_FADE_OUT_FRAMES,
        };
    }

    /// Resolve a safe editor-warp destination on `map`. Coordinates are player
    /// units (2 tiles per unit — the same space as map.json warp coordinates),
    /// so `(0, 0)` is never a valid spot and means "pick a good default".
    ///
    /// Candidate order:
    /// 1. the requested position, when given, in-bounds and walkable;
    /// 2. the map's own warp spots (placed on walkable tiles by the original
    ///    data), then the map center;
    /// 3. a spiral scan from the center for the nearest walkable position;
    /// 4. a clamped `(1, 1)` fallback.
    ///
    /// Walkability uses the same tile rules as movement collision
    /// (`overworld::update::is_script_walkable_tile`), so the player can never
    /// be warped onto a roof, wall or water tile.
    pub fn resolve_editor_warp_position(&self, map: MapId, requested: Option<(u8, u8)>) -> (u8, u8) {
        let (map_data, _npc) =
            crate::overworld::map_data_loading::load_full_map_data(map, self.game_data.tileset_provider());
        let w_units = (map_data.width as u16) * 2;
        let h_units = (map_data.height as u16) * 2;
        let walkable = |x: u16, y: u16| {
            crate::overworld::update::is_script_walkable_tile(&map_data, x, y)
        };

        if let Some((rx, ry)) = requested {
            let (rx, ry) = (rx as u16, ry as u16);
            if rx < w_units && ry < h_units && walkable(rx, ry) {
                return (rx as u8, ry as u8);
            }
        }

        // Default candidates: the map's own warp spots first, then the center.
        let mut candidates: Vec<(u16, u16)> = map_data
            .warps
            .iter()
            .map(|w| (w.x as u16, w.y as u16))
            .collect();
        candidates.push((w_units / 2, h_units / 2));
        for (x, y) in candidates {
            if x < w_units && y < h_units && walkable(x, y) {
                return (x.min(255) as u8, y.min(255) as u8);
            }
        }

        // Spiral scan from the center — expanding square rings, each cell once.
        let (cx, cy) = (w_units as i32 / 2, h_units as i32 / 2);
        let max_r = w_units.max(h_units) as i32;
        for r in 0..=max_r {
            for x in -r..=r {
                for (y) in [cy - r, cy + r] {
                    if x + cx >= 0 && y >= 0 && (x + cx) < w_units as i32 && y < h_units as i32
                        && walkable((x + cx) as u16, y as u16)
                    {
                        return ((x + cx).min(255) as u8, y.min(255) as u8);
                    }
                }
            }
            for y in -r..=r {
                for x in [cx - r, cx + r] {
                    if x >= 0 && y + cy >= 0 && x < w_units as i32 && (y + cy) < h_units as i32
                        && walkable(x as u16, (y + cy) as u16)
                    {
                        return (x.min(255) as u8, (y + cy).min(255) as u8);
                    }
                }
            }
        }
        (1, 1)
    }

    pub fn script_flags(&self) -> &std::collections::HashMap<String, bool> {
        self.unified_flags.as_hashmap()
    }

    /// Label of the currently active script effect (e.g. "ShowDialogue",
    /// "FollowNpc") for debug/test observability, or `None` when no
    /// storyline command is being processed.
    pub fn active_script_effect_label(&self) -> Option<String> {
        self.active_script_effect.as_ref().map(|e| {
            // `Debug` of an enum value is `Variant { .. }` / `Variant(..)`;
            // keep just the variant name.
            let dbg = format!("{:?}", e);
            dbg.split([' ', '(']).next().unwrap_or(&dbg).to_string()
        })
    }

    pub fn set_script_flags(&mut self, flags: std::collections::HashMap<String, bool>) {
        self.unified_flags.merge_from(&flags);
    }

    /// Set a script flag on BOTH the persistent `unified_flags` and the live
    /// script engine's flag store, so a running scene's `getFlag(...)` observes
    /// the change immediately (the engine is otherwise seeded only at map load).
    /// Used by the debug server's `SetFlag` command.
    pub fn set_flag_live(&mut self, name: &str, value: bool) {
        self.unified_flags.set_flag(name, value);
        self.script_engine.set_flag(name, value);
    }

    /// Use a bag item from the overworld ITEM menu. Applies the field effect and
    /// sets `pending_dialogue` with the result message. Returns `true` if the
    /// item should be consumed (removed from the bag) by the caller.
    ///
    /// `last_blackout_map` is `wLastBlackoutMap` from the persistent game data
    /// (the map the player last healed at); the ESCAPE ROPE warp target is the
    /// fly point of that map, exactly like TELEPORT / DIG (see below).
    pub fn use_field_item(
        &mut self,
        item: pokered_data::items::ItemId,
        last_blackout_map: MapId,
    ) -> bool {
        use pokered_data::event_flags::EventFlag;
        use pokered_data::items::ItemId;

        let mut consumed = false;
        // `Some(text)` shows a message on return to the field; `None` means the
        // item already triggered an animated action (e.g. the ESCAPE ROPE warp)
        // and no lingering dialogue should be shown.
        let msg: Option<String> = match item {
            // POKe FLUTE: on Route 12 / 16, before the Snorlax is beaten, wake it
            // (sets the FIGHT flag so talking to it starts the battle). Elsewhere
            // it just plays. (Original ItemUsePokeFlute; a key item, not consumed.)
            ItemId::PokeFlute => {
                let woke = match self.state.current_map {
                    MapId::Route12
                        if !self
                            .unified_flags
                            .check(EventFlag::EVENT_BEAT_ROUTE12_SNORLAX) =>
                    {
                        self.set_flag_live("EVENT_FIGHT_ROUTE12_SNORLAX", true);
                        true
                    }
                    MapId::Route16
                        if !self
                            .unified_flags
                            .check(EventFlag::EVENT_BEAT_ROUTE16_SNORLAX) =>
                    {
                        self.set_flag_live("EVENT_FIGHT_ROUTE16_SNORLAX", true);
                        true
                    }
                    _ => false,
                };
                Some(if woke {
                    // PlayedFluteHadEffectText (engine/items/item_effects.asm:1794-1811):
                    // when the flute had an effect outside battle, the original
                    // stops the music, plays SFX_POKEFLUTE, then restarts the map
                    // music. Here the jingle plays as a plain SFX with the map
                    // music continuing underneath (no blocking wait).
                    self.audio_requests.push(OverworldAudioRequest::PlaySound {
                        sound_id: "SFX_POKEFLUTE".to_string(),
                    });
                    "You played the\nPOKe FLUTE!\n\nThe SNORLAX\nwoke up!".to_string()
                } else {
                    "You played the\nPOKe FLUTE.\n\nNothing happened.".to_string()
                })
            }
            // BICYCLE: toggle riding, which halves the frames-per-step (Biking = 4
            // vs Walking = 8). Can't ride while SURFING; on the Cycling Road the
            // forced-bike lock (BIT_ALWAYS_ON_BIKE) refuses to get off — the item
            // menu check at engine/menus/start_sub_menus.asm:374-379 prints
            // CannotGetOffHereText ("You can't get off here.").
            ItemId::Bicycle => {
                if self.forced_bike.active {
                    Some("You can't get off\nhere.".to_string())
                } else {
                    Some(match self.state.player.transport {
                        TransportMode::Surfing => "You can't\nBICYCLE here!".to_string(),
                        TransportMode::Biking => {
                            self.state.player.transport = TransportMode::Walking;
                            "You got off the\nBICYCLE.".to_string()
                        }
                        TransportMode::Walking => {
                            self.state.player.transport = TransportMode::Biking;
                            "You got on the\nBICYCLE!".to_string()
                        }
                    })
                }
            },
            // TOWN MAP: the full scrollable KANTO map screen is a follow-up.
            ItemId::TownMap => Some("You checked the\nTOWN MAP.".to_string()),
            // ESCAPE ROPE / DIG (ItemUseEscapeRope, engine/items/
            // item_effects.asm:1492-1528): usable only in a dungeon — a map
            // whose tileset is in EscapeRopeTilesets (FOREST/CEMETERY/CAVERN/
            // FACILITY/INTERIOR, data/tilesets/escape_rope_tilesets.asm) —
            // and never in Agatha's Room. On success it sets BIT_FLY_WARP |
            // BIT_ESCAPE_WARP; LoadSpecialWarpData (engine/overworld/
            // special_warps.asm:76-80) then warps to wLastBlackoutMap's fly
            // point — the last Pokémon Center the player healed at
            // (SetLastBlackoutMap, engine/events/set_blackout_map.asm:1-23).
            // DIG reuses this whole flow via wPseudoItemID
            // (start_sub_menus.asm:195-199); only the bag path consumes the
            // item (RemoveUsedItem is skipped for the move).
            ItemId::EscapeRope => {
                let in_dungeon = self
                    .map_data
                    .as_ref()
                    .map(|m| hm_effects::is_escape_rope_tileset(m.tileset))
                    .unwrap_or(false);
                let in_agathas_room = self.state.current_map == MapId::AgathasRoom;
                if in_dungeon && !in_agathas_room {
                    let dest = hm_effects::fly_destination_for_map(last_blackout_map)
                        .or_else(|| hm_effects::fly_destination_for_map(MapId::PalletTown))
                        .expect("Pallet Town always has a fly point");
                    self.pending_warp = Some(PendingWarp {
                        dest_map: dest.map,
                        dest_x: dest.x,
                        dest_y: dest.y,
                        save_last_map: false,
                        // ItemUseEscapeRope sets BIT_FLY_WARP
                        // (item_effects.asm:1509) → the arrival plays
                        // EnterMapAnim's spin-in.
                        arrival_spin: true,
                    });
                    // _LeaveMapAnim (BIT_ESCAPE_WARP): spin in place, rise
                    // off screen, then GBFadeOutToWhite — the fade starts
                    // when the spin finishes.
                    self.warp_fade_to_white = true;
                    self.teleport_spin = Some(presentation::TeleportSpinState::new(
                        self.state.player.facing,
                    ));
                    consumed = true;
                    None
                } else {
                    Some("Can't use that\nhere.".to_string())
                }
            }
            // ITEMFINDER (ItemUseItemfinder, engine/items/item_effects.asm:1920-1941
            // + HiddenItemNear, engine/items/itemfinder.asm): if an unobtained
            // hidden item on this map is within the scan window (±4 tiles in Y,
            // -4/+5 in X — the original's asymmetric range), play the ding pair
            // (SFX_HEALING_MACHINE + SFX_PURCHASE) four times and answer "Yes!";
            // otherwise "Nope!". Key item — never consumed. Coins are NOT
            // detected (HiddenItemCoords only).
            //
            // The original plays each SFX with PlaySoundWaitForCurrent, so the
            // eight dings run sequentially; the port's audio layer plays one SFX
            // per drained request and has no SFX queue, so the dings are metered
            // out by `itemfinder_dings` (ticked in update_frame, one ding every
            // ITEMFINDER_DING_FRAMES — the same 30-frame spacing the Poké Center
            // healing machine uses per SFX).
            ItemId::Itemfinder => {
                let found = pokered_data::hidden_items::hidden_item_near(
                    self.state.current_map,
                    self.state.player.x as u8,
                    self.state.player.y as u8,
                    |i| crate::overworld::hidden_items::check_obtained(&self.hidden_item_flags, i),
                );
                if found {
                    self.itemfinder_dings = Some((8, 0));
                    Some(crate::overworld::hidden_items::ITEMFINDER_FOUND_MESSAGE.to_string())
                } else {
                    Some(crate::overworld::hidden_items::ITEMFINDER_NOTHING_MESSAGE.to_string())
                }
            }
            // REPEL / SUPER REPEL / MAX REPEL (ItemUseRepel &
            // ItemUseRepelCommon, engine/items/item_effects.asm:1532-1541 &
            // 1622-1628): set wRepelRemainingSteps to 100/200/250. Field-only,
            // consumed. The wild-encounter check suppresses battles below the
            // lead mon's level while steps remain (update.rs).
            ItemId::Repel | ItemId::SuperRepel | ItemId::MaxRepel => {
                self.state.repel_steps = match item {
                    ItemId::Repel => 100,
                    ItemId::SuperRepel => 200,
                    _ => 250,
                };
                consumed = true;
                let name = pokered_data::item_data::get_item_data(item)
                    .map(|d| d.name)
                    .unwrap_or("REPEL");
                Some(format!("You used the\n{}!", name))
            }
            // OLD ROD / GOOD ROD / SUPER ROD (ItemUseOldRod / ItemUseGoodRod /
            // ItemUseSuperRod, engine/items/item_effects.asm:1826-1889): key
            // items, never consumed. Fishing is possible only facing a
            // shore/water tile while not surfing; on a bite the hooked mon's
            // wild battle starts once the result text is dismissed.
            ItemId::OldRod | ItemId::GoodRod | ItemId::SuperRod => {
                let rod = crate::overworld::fishing::RodKind::from_item(item)
                    .expect("rod item maps to a rod kind");
                Some(self.use_fishing_rod(rod))
            }
            // Anything else can't be used from the field.
            _ => Some("This isn't the\ntime to use that!".to_string()),
        };
        if let Some(text) = msg {
            self.pending_dialogue = Some(BedroomDialogue::from_message(&text));
        }
        consumed
    }

    /// Gen-1 DisplayRepelWoreOffText (home/text_script.asm:209): when the last
    /// REPEL step is taken, the "REPEL's effect wore off." text shows INSTEAD
    /// of that step's wild-encounter roll.
    ///
    /// [`repel_wore_off`](Self::repel_wore_off) is the pure predicate (the
    /// movement update needs it before it can mutate `self` again);
    /// [`show_repel_wore_off_message`](Self::show_repel_wore_off_message)
    /// queues the text.
    pub(crate) fn repel_wore_off(&self, repel_before: u16) -> bool {
        repel_before > 0 && self.state.repel_steps == 0
    }

    /// Queue the "REPEL's effect wore off." text (unless a dialogue is
    /// already up). The caller skips the current step's encounter check,
    /// matching wild_encounters.asm `.lastRepelStep`.
    pub(crate) fn show_repel_wore_off_message(&mut self) {
        if self.pending_dialogue.is_none() {
            self.pending_dialogue =
                Some(BedroomDialogue::from_message("REPEL's effect\nwore off."));
        }
    }

    /// Set the encounter cooldown after returning from a battle.
    /// Matches the original game's EnterMap logic in home/overworld.asm:
    /// when BIT_WILD_ENCOUNTER_COOLDOWN is set (after battle), it sets
    /// wNumberOfNoRandomBattleStepsLeft = 3.
    pub fn set_post_battle_encounter_cooldown(&mut self) {
        self.state.encounter_cooldown = 3;
    }

    pub fn cutscene_manager(&self) -> &CutsceneManager {
        &self.cutscene_manager
    }

    pub fn cutscene_manager_mut(&mut self) -> &mut CutsceneManager {
        &mut self.cutscene_manager
    }

    pub fn unified_flags(&self) -> &event_flags::EventFlags {
        &self.unified_flags
    }

    pub fn unified_flags_mut(&mut self) -> &mut event_flags::EventFlags {
        &mut self.unified_flags
    }

    pub fn toggleable_object_flags(
        &self,
    ) -> &[u8; pokered_data::toggleable_objects::TOGGLEABLE_OBJECT_FLAGS_SIZE] {
        &self.toggleable_object_flags
    }

    pub fn set_toggleable_object_flags(
        &mut self,
        flags: [u8; pokered_data::toggleable_objects::TOGGLEABLE_OBJECT_FLAGS_SIZE],
    ) {
        self.toggleable_object_flags = flags;
    }

    pub fn hidden_item_flags(&self) -> &[u8; crate::save::game_data::HIDDEN_ITEMS_BYTES] {
        &self.hidden_item_flags
    }

    pub fn set_hidden_item_flags(
        &mut self,
        flags: [u8; crate::save::game_data::HIDDEN_ITEMS_BYTES],
    ) {
        self.hidden_item_flags = flags;
    }

    /// `HiddenItems` (engine/events/hidden_items.asm:1-50): the A-button
    /// handler found the tile in front of the player in `HiddenItemCoords`.
    ///
    /// * Already obtained → show nothing (the original returns on the flag
    ///   test); the A press is still consumed by the caller.
    /// * Otherwise "<PLAYER> found X!", then `GiveItem`: success sets the
    ///   obtained flag, queues the bag add and plays `SFX_GET_ITEM_2`; a full
    ///   bag shows the "no more room" text instead and leaves the flag clear
    ///   so the item can be picked up later.
    pub(crate) fn handle_hidden_item(&mut self, facing_x: u8, facing_y: u8) {
        use crate::overworld::hidden_items as hi;
        let find = hi::examine_facing_tile(
            self.state.current_map,
            facing_x,
            facing_y,
            &self.hidden_item_flags,
            &self.script_bag_names,
        );
        match find {
            None | Some(hi::HiddenItemFind::AlreadyObtained) => {}
            Some(hi::HiddenItemFind::Found {
                index,
                item,
                bag_full,
            }) => {
                let mut text = hi::found_message(&self.player_name, item);
                if bag_full {
                    // GiveItem failed: no flag, no item (hidden_items.asm:40-46).
                    text.push_str("\n\n");
                    text.push_str(&hi::bag_full_message(&self.player_name));
                } else {
                    hi::set_obtained(&mut self.hidden_item_flags, index);
                    self.game_data_requests
                        .push(OverworldGameDataRequest::GiveItem {
                            item: item.const_name(),
                            quantity: 1,
                        });
                    self.audio_requests.push(OverworldAudioRequest::PlaySound {
                        sound_id: "SFX_GET_ITEM_2".to_string(),
                    });
                }
                self.pending_dialogue = Some(BedroomDialogue::from_message(&text));
            }
        }
    }

    pub fn apply_hidden_object_flags(&mut self) {
        use pokered_data::toggleable_objects::{is_object_hidden, toggle_id_to_bit_index};

        // First: apply from persisted toggleable_object_flags (SRAM-restored state)
        for npc_cfg in &self.map_script_config.npcs {
            if let Some(ref toggle_id) = npc_cfg.toggle_id {
                if let Some(bit_index) = toggle_id_to_bit_index(toggle_id) {
                    if is_object_hidden(&self.toggleable_object_flags, bit_index) {
                        if let Some(npc) =
                            self.npc_states.iter_mut().find(|n| n.text_id == npc_cfg.id)
                        {
                            npc.visible = false;
                        }
                    }
                }
            }
        }

        // Second: apply from unified_flags (__OBJ_HIDDEN_* runtime flags set by scripts)
        let hidden_keys: Vec<(String, String)> = self
            .unified_flags
            .iter()
            .filter(|(key, &val)| val && key.starts_with("__OBJ_HIDDEN_"))
            .map(|(key, _)| {
                let toggle_id = key["__OBJ_HIDDEN_".len()..].to_owned();
                (key.clone(), toggle_id)
            })
            .collect();
        for (_key, toggle_id) in hidden_keys {
            if let Some(npc_id) = self.map_script_config.npc_id_by_toggle(&toggle_id) {
                if let Some(npc) = self.npc_states.iter_mut().find(|n| n.text_id == npc_id) {
                    npc.visible = false;
                }
            }
        }

        // Third: handle default_hidden NPCs that may have been shown by script
        for npc_cfg in &self.map_script_config.npcs {
            if !npc_cfg.default_hidden {
                continue;
            }
            if let Some(ref toggle_id) = npc_cfg.toggle_id {
                let shown_key = format!("__OBJ_SHOWN_{}", toggle_id);
                if self.unified_flags.get_flag(&shown_key) {
                    if let Some(npc) = self.npc_states.iter_mut().find(|n| n.text_id == npc_cfg.id)
                    {
                        npc.visible = true;
                    }
                }
            }
        }
    }

    pub fn run_on_load(&mut self) {
        self.script_engine
            .seed_flags(self.unified_flags.as_hashmap());

        if let Some(fn_name) = self.map_script_config.on_load() {
            if self.script_engine.has_function(fn_name) {
                self.script_engine
                    .set_player_position(self.state.player.x as u8, self.state.player.y as u8);
                if let Ok(Some(cmd)) = self.script_engine.call_function_no_args(fn_name) {
                    self.active_script_effect = Some(crate::overworld::script_bridge::dispatch_command_with_names(
                        &cmd,
                        &self.player_name,
                        &self.rival_name,
                        &self.starter_display_name(),
                    ));
                }
                self.sync_flags_from_engine();
            }
        }
    }

    /// Queue the new-game bedroom SNES dialogue.
    pub fn start_bedroom_dialogue(&mut self, player_name: &str) {
        self.pending_dialogue = Some(BedroomDialogue::new(player_name));
    }

    /// Returns the current warp fade darkness level (0.0 = fully visible, 1.0 = fully black).
    /// Used by the renderer to draw a black overlay during map transitions.
    pub fn warp_fade_progress(&self) -> f32 {
        match self.warp_fade_state {
            WarpFadeState::Idle => 0.0,
            WarpFadeState::FadingOut { frames_remaining } => {
                1.0 - (frames_remaining as f32 / WARP_FADE_OUT_FRAMES as f32)
            }
            WarpFadeState::BlackScreen => 1.0,
            WarpFadeState::FadingIn { frames_remaining } => {
                frames_remaining as f32 / WARP_FADE_IN_FRAMES as f32
            }
        }
    }

    /// Remaining Safari Zone steps.
    pub fn safari_steps_remaining(&self) -> u16 {
        self.safari_steps
    }

    /// Remaining Safari Balls.
    pub fn safari_balls_remaining(&self) -> u8 {
        self.safari_balls
    }

    /// Whether a Safari Zone game is currently in progress.
    pub fn is_safari_game_active(&self) -> bool {
        self.safari_game_active
    }

    /// Begin a fresh Safari Zone game: full step + ball allowance.
    pub fn start_safari_game(&mut self) {
        self.safari_steps = SAFARI_ZONE_STEP_COUNT;
        self.safari_balls = SAFARI_ZONE_BALL_COUNT;
        self.safari_game_active = true;
    }

    /// End the current Safari game and clear its counters (e.g. when the player
    /// leaves the zone or is ejected).
    pub fn end_safari_game(&mut self) {
        self.safari_game_active = false;
        self.safari_steps = 0;
        self.safari_balls = 0;
    }

    /// Consume one Safari Ball (called by the battle layer on a ball throw).
    /// Returns the number of balls remaining.
    pub fn use_safari_ball(&mut self) -> u8 {
        self.safari_balls = self.safari_balls.saturating_sub(1);
        self.safari_balls
    }

    pub(crate) fn commit_pending_warp(&mut self) {
        if let Some(warp) = self.pending_warp.take() {
            // Clear movement/script carry-over from the previous map to avoid
            // retriggering stale scripted paths or collision-side warp checks.
            self.scripted_player_path.clear();
            self.state.player.movement_state = MovementState::Idle;
            self.state.walk_counter = 0;
            self.state.exiting_door = false;
            self.state.standing_on_warp = false;

            // EnterMap: ResetUsingStrengthOutOfBattleBit — STRENGTH wears off
            // on every map change. Boulder-push state goes with it.
            self.strength_active = false;
            self.tried_push_boulder = false;
            self.boulder_dust_frames = 0;
            self.boulder_dust = presentation::BoulderDustState::inactive();
            // A mid-cutscene map change (e.g. the departure's walk-out warp)
            // must not carry the animation into the next map.
            self.ship_departure = None;
            // EnterMap: dark-cave palette follows the new map; mark city maps
            // as visited for the FLY destination list.
            self.dark_cave.enter_map(warp.dest_map);
            if is_city_map(warp.dest_map) {
                self.game_data_requests
                    .push(OverworldGameDataRequest::MarkTownVisited { map: warp.dest_map });
            }

            if warp.save_last_map {
                self.last_map = Some(self.state.current_map);
                // player.x/y still hold the tile on the outside map where the
                // entrance warp was stepped on — the ESCAPE ROPE return point.
                self.last_map_entry =
                    Some((self.state.player.x as u8, self.state.player.y as u8));
            }
            self.state.current_map = warp.dest_map;
            self.state.player.x = warp.dest_x as u16;
            self.state.player.y = warp.dest_y as u16;

            // HandleFlyWarpOrDungeonWarp (home/overworld.asm:783-793): FLY/DIG/
            // TELEPORT/ESCAPE ROPE arrivals reset BIT_ALWAYS_ON_BIKE and
            // wWalkBikeSurfState — the player is never on the forced bike after
            // these.
            if warp.arrival_spin {
                self.forced_bike.clear();
                self.state.player.transport = TransportMode::Walking;
            }
            // EnterMap: CheckForceBikeOrSurf — the Cycling Road tiles mount and
            // lock the bike; entering a gate map releases it.
            self.apply_map_entry_transport(warp.dest_map, warp.dest_x as u16, warp.dest_y as u16);

            // Safari Zone timer/ball economy: arm the game when the player first
            // walks into the zone, and reset it once they leave (or are ejected
            // back to the gate, which is not a step-counting map).
            if pokered_data::map_flags::is_safari_zone_map(warp.dest_map) {
                if !self.safari_game_active {
                    self.start_safari_game();
                }
            } else if self.safari_game_active {
                self.end_safari_game();
            }
            let (map_data, npc_pokemon_data) =
                crate::overworld::map_data_loading::load_full_map_data(warp.dest_map, self.game_data.tileset_provider());
            self.map_data = Some(map_data);
            self.npc_pokemon_data = npc_pokemon_data;
            // LoadTilesetHeader: hTileAnimations follows the new tileset.
            if let Some(ref md) = self.map_data {
                self.tile_anim.set_tileset(
                    pokered_data::tileset_data::get_tileset_header(md.tileset).animation,
                );
            }
            self.load_map_script(warp.dest_map);
            self.audio_requests
                .push(OverworldAudioRequest::PlayMapMusic { map: warp.dest_map });
            let hidden_npc_ids = self.map_script_config.hidden_npc_ids();
            self.npc_states = self
                .map_data
                .as_ref()
                .map(|md| build_npc_runtime_states(&md.npcs, &self.npc_pokemon_data, &hidden_npc_ids))
                .unwrap_or_default();
            self.apply_hidden_object_flags();

            // PlayerStepOutFromDoor: if the player landed on a door tile,
            // flag it so update_frame will auto-walk one step down.
            // Skip when on_load started a script — the script controls the player.
            if self.active_script_effect.is_none() {
                if let Some(map) = &self.map_data {
                    let provider = &collision::PokemonCollisionProvider::new(map.id, map.tileset);
                    let tile = provider.get_tile_at_position(
                        map.tileset, &map.blocks, map.width,
                        self.state.player.x,
                        self.state.player.y,
                    );
                    if crate::overworld::doors_elevators::is_standing_on_door(map.tileset, tile) {
                        self.state.standing_on_door = true;
                        self.state.player.facing = Direction::Down;
                    }
                }
            }

            // EnterMapAnim (player_animations.asm:1-91): FLY / TELEPORT /
            // DIG / ESCAPE ROPE / dungeon-warp arrivals spin the player in
            // from off the top of the screen after the fade-in-from-white.
            // The player stays hidden during the fade (the original sets
            // Y=$ec before GBFadeInFromWhite and descends after it); update
            // ticks the state once the fade completes. Arrivals that land ON
            // a warp pad or hole skip the final spin-in-place
            // (IsPlayerStandingOnWarpPadOrHole).
            if warp.arrival_spin {
                let spin_in_place = self
                    .map_data
                    .as_ref()
                    .map(|md| {
                        let provider =
                            &collision::PokemonCollisionProvider::new(md.id, md.tileset);
                        let tile = provider.get_tile_at_position(
                            md.tileset,
                            &md.blocks,
                            md.width,
                            warp.dest_x as u16,
                            warp.dest_y as u16,
                        );
                        special_terrain::check_warp_pad_or_hole(md.tileset, tile)
                            == pokered_data::tileset_data::WarpPadOrHoleType::None
                    })
                    .unwrap_or(true);
                self.enter_map_anim = Some(presentation::EnterMapSpinState::new(
                    self.state.player.facing,
                    spin_in_place,
                ));
            }
        }
    }

    pub fn is_naming_screen_active(&self) -> bool {
        self.pending_naming_screen.is_some()
    }

    /// True while a script-driven party selection (Name Rater) is on screen.
    pub fn is_party_select_active(&self) -> bool {
        self.pending_party_select.is_some()
    }

    /// Returns `true` (once) if a script asked to open the party selector. The
    /// app should respond by calling `begin_party_select` with the party.
    pub fn take_party_select_request(&mut self) -> bool {
        std::mem::take(&mut self.party_select_requested)
    }

    /// Returns `true` (once) if a Cable Club "gameboy" script asked to start
    /// the link flow (`game.linkStart()`). The app drains this and drives the
    /// request/accept/decline state machines (which own the session).
    pub fn take_link_start_request(&mut self) -> bool {
        std::mem::take(&mut self.link_start_requested)
    }

    /// Hand the party members to the pending party selector (app-owned data).
    pub fn begin_party_select(&mut self, party: Vec<crate::battle::state::Pokemon>) {
        self.pending_party_select = Some(crate::party_select::PartySelectState::new(party));
    }

    /// Drive one frame of the party selector. On A/B, records the chosen index
    /// (or -1 on cancel) back into the active `ChoosePartyPokemon` effect.
    pub fn update_party_select_input(&mut self, input: crate::party_screen::PartyScreenInput) {
        if let Some(ref mut sel) = self.pending_party_select {
            let resolved = match sel.update_frame(input) {
                crate::party_select::PartySelectResult::Active => None,
                crate::party_select::PartySelectResult::Selected(idx) => Some(idx as i32),
                crate::party_select::PartySelectResult::Cancelled => Some(-1),
            };
            if let Some(index) = resolved {
                if let Some(crate::overworld::script_bridge::ScriptEffect::ChoosePartyPokemon {
                    result_index,
                    ..
                }) = self.active_script_effect.as_mut()
                {
                    *result_index = Some(index);
                }
                self.pending_party_select = None;
            }
        }
    }

    pub fn update_naming_input(&mut self, input: crate::naming_screen::NamingInput, is_zh: bool) {
        // The opening white flash (GBPalWhiteOutWithDelay3) plays before the
        // naming screen accepts input.
        if self.naming_flash_frames > 0 {
            return;
        }
        if let Some(ref mut ns) = self.pending_naming_screen {
            match ns.update_frame(input, is_zh) {
                crate::naming_screen::NamingScreenResult::Editing => {}
                crate::naming_screen::NamingScreenResult::Submitted(name) => {
                    if let Some(ref mut effect) = self.active_script_effect {
                        if let crate::overworld::script_bridge::ScriptEffect::NamingScreen {
                            result_name,
                            naming_state,
                            ..
                        } = effect
                        {
                            *result_name = Some(name);
                            *naming_state = Some(ns.clone());
                        }
                    }
                    self.pending_naming_screen = None;
                    self.naming_flash_frames = crate::naming_screen::NAMING_FLASH_FRAMES;
                }
                crate::naming_screen::NamingScreenResult::Cancelled => {
                    self.pending_naming_screen = None;
                    self.naming_flash_frames = crate::naming_screen::NAMING_FLASH_FRAMES;
                }
            }
        }
    }
}

// ── Shared helper ─────────────────────────────────────────────────

pub(crate) fn build_npc_runtime_states(
    npcs: &[NpcDefinition],
    _npc_pokemon_data: &[PokemonNpcData],
    hidden_npc_ids: &[u8],
) -> Vec<crate::overworld::npc_movement::NpcRuntimeState> {
    npcs.iter()
        .enumerate()
        .map(|(i, npc)| {
            let visible = !hidden_npc_ids.contains(&npc.text_id);
            crate::overworld::npc_movement::NpcRuntimeState {
                npc_index: i as u8,
                sprite_id: npc.sprite_id,
                x: npc.x as u16,
                y: npc.y as u16,
                home_x: npc.x as u16,
                home_y: npc.y as u16,
                facing: npc.facing,
                scripted_frame: None,
                movement_type: npc.movement,
                range: npc.range,
                walk_counter: 0,
                delay_counter: 0,
                text_id: npc.text_id,
                defeated: false,
                visible,
                scripted_path: std::collections::VecDeque::new(),
            }
        })
        .collect()
}


// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod typewriter_tests {
    use super::*;

    fn dlg(text: &str) -> BedroomDialogue {
        BedroomDialogue::from_message(text)
    }

    /// FAST text (wOptions TEXT_DELAY_FAST = 1): one character per frame —
    /// the historical behavior of this engine.
    #[test]
    fn fast_text_reveals_one_char_per_frame() {
        let mut d = dlg("ABCDEFGH");
        d.set_text_delay_frames(crate::game_state::TextSpeed::Fast.delay_frames());
        for expected in 1..=8u16 {
            d.reveal_next_char();
            assert_eq!(d.char_index(), expected, "after {expected} frames");
        }
        assert!(d.waiting_for_input());
    }

    /// MEDIUM text (TEXT_DELAY_MEDIUM = 3): one character per 3 frames
    /// (PrintLetterDelay waits 3 frames after each printed letter).
    #[test]
    fn medium_text_reveals_one_char_per_three_frames() {
        let mut d = dlg("ABCDEFGH");
        d.set_text_delay_frames(crate::game_state::TextSpeed::Medium.delay_frames());
        // Character k is revealed on frame 3k-2; 8 chars need 22 frames.
        for frame in 1..=22u16 {
            d.reveal_next_char();
            let expected = ((frame + 2) / 3).min(8);
            assert_eq!(d.char_index(), expected, "frame {frame}");
        }
        assert!(d.waiting_for_input());
    }

    /// SLOW text (TEXT_DELAY_SLOW = 5): one character per 5 frames.
    #[test]
    fn slow_text_reveals_one_char_per_five_frames() {
        let mut d = dlg("ABCDE");
        d.set_text_delay_frames(crate::game_state::TextSpeed::Slow.delay_frames());
        // Character k is revealed on frame 5k-4; 5 chars need 21 frames.
        for frame in 1..=21u16 {
            d.reveal_next_char();
            let expected = ((frame + 4) / 5).min(5);
            assert_eq!(d.char_index(), expected, "frame {frame}");
        }
        assert!(d.waiting_for_input());
    }

    /// The reveal cadence of an already-open dialogue follows the currently
    /// configured speed (the app pushes it every frame).
    #[test]
    fn speed_change_applies_mid_dialogue() {
        let mut d = dlg("ABCD");
        d.set_text_delay_frames(1);
        d.reveal_next_char();
        assert_eq!(d.char_index(), 1);
        d.set_text_delay_frames(5);
        d.reveal_next_char();
        assert_eq!(d.char_index(), 2);
        for _ in 0..4 {
            d.reveal_next_char();
        }
        assert_eq!(d.char_index(), 2, "now on slow cadence (4 delay frames)");
        d.reveal_next_char();
        assert_eq!(d.char_index(), 3);
    }

    /// Advancing to the next page restarts the reveal immediately (no
    /// leftover delay from the previous page).
    #[test]
    fn page_advance_resets_delay() {
        // "AB\n\nCD" paginates as ["AB",""] then ["CD"] (two lines per box).
        let mut d = dlg("AB\n\nCD");
        d.set_text_delay_frames(5);
        d.reveal_next_char();
        assert_eq!(d.char_index(), 1);
        d.skip_to_full_page();
        assert!(d.advance());
        d.reveal_next_char();
        assert_eq!(d.char_index(), 1, "first frame of the new page reveals");
    }

    /// Zero/invalid delays are clamped to one character per frame.
    #[test]
    fn zero_delay_clamps_to_fast() {
        let mut d = dlg("AB");
        d.set_text_delay_frames(0);
        d.reveal_next_char();
        d.reveal_next_char();
        assert_eq!(d.char_index(), 2);
    }
}

#[cfg(test)]
mod connection_arrival_tests {
    use super::*;
    use pokered_data::impl_traits::PokemonRedData;

    fn make_screen(map: MapId) -> OverworldScreen {
        OverworldScreen::new(map, None, PokemonRedData)
    }

    #[test]
    fn surfer_arriving_on_land_dismounts() {
        // PalletTown (2,17) is beach grass ($0a → $2c): a surfer who crossed
        // the seam onto it must dismount (CollisionCheckOnWater .stopSurfing).
        let mut screen = make_screen(MapId::PalletTown);
        screen.state.player.x = 2;
        screen.state.player.y = 17;
        screen.state.player.transport = TransportMode::Surfing;
        screen.dismount_surf_if_on_land();
        assert_eq!(screen.state.player.transport, TransportMode::Walking);
    }

    #[test]
    fn surfer_arriving_on_water_keeps_surfing() {
        // Route21 (4,0) is surfable coast ($65 → $32): the surfer stays on
        // the water.
        let mut screen = make_screen(MapId::Route21);
        screen.state.player.x = 4;
        screen.state.player.y = 0;
        screen.state.player.transport = TransportMode::Surfing;
        screen.dismount_surf_if_on_land();
        assert_eq!(screen.state.player.transport, TransportMode::Surfing);
    }

    #[test]
    fn walker_arriving_is_untouched() {
        let mut screen = make_screen(MapId::PalletTown);
        screen.state.player.x = 2;
        screen.state.player.y = 17;
        screen.state.player.transport = TransportMode::Walking;
        screen.dismount_surf_if_on_land();
        assert_eq!(screen.state.player.transport, TransportMode::Walking);
    }
}
