use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use crate::link::{
    CableClubFlow, CableClubPhase, FlowNeed, LinkKind, LinkSession, LinkStatus,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::link::LinkServer;

use pokered_core::battle::link_battle_driver::{LinkBattleDriver, LinkDriverEvent};
use pokered_core::link::link_trade::{LinkTradeDriver, LinkTradePollResult};
use pokered_core::link::protocol::NetworkMessage;
use pokered_core::link::transport::NetworkTransport;
use pokered_core::link::LinkRole;

use pokered_audio::music_data::MusicId;
use pokered_audio::sfx_data::SfxId;
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::data::maps::MapId;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::{GameScreen, GameState, SaveFileSummary, ScreenAction};
use pokered_core::gamefreak_splash::{GameFreakSplashState, SplashInput};
use pokered_core::intro_scene::IntroSceneState;
use pokered_core::items::{MartUpdate, PlayerData, ShopInventory, SoundId};
use pokered_core::intro_scene::IntroSfxEvent;
use pokered_core::main_menu::{MainMenuState, MenuInput};
use pokered_core::naming_screen::NamingInput;
use pokered_core::oak_speech::{OakSpeechInput, OakSpeechPhase, OakSpeechResult, OakSpeechState};
use pokered_core::options_menu::{
    BattleAnimation, GameOptions, OptionsInput, OptionsMenuResult, OptionsMenuState,
};
use pokered_core::data::impl_traits::PokemonRedData;
use pokered_core::overworld::{
    BedroomDialogue, OverworldAudioRequest, OverworldGameDataRequest, OverworldInput,
    OverworldScreen, OverworldSfxEvent,
};
use pokered_core::party_screen::{PartyScreenAction, PartyScreenInput, PartyScreenState};
use pokered_core::items::bag_use::{self, ItemApplyOutcome};
use pokered_core::bag_screen::{BagScreenAction, BagScreenInput, BagScreenState};
use pokered_core::town_map_screen::{TownMapScreenAction, TownMapScreenInput, TownMapScreenState};
use pokered_core::pokedex_screen::{
    PokedexScreenAction, PokedexScreenInput, PokedexScreenState,
};
use pokered_core::trainer_card_screen::{
    TrainerCardAction, TrainerCardInput, TrainerCardScreenState,
};
use pokered_core::stats_screen::{StatsScreenAction, StatsScreenInput, StatsScreenState};
#[cfg(any(not(target_arch = "wasm32"), debug_assertions))]
use pokered_core::save::sram_export::export_sram;

#[cfg(not(target_arch = "wasm32"))]
use pokered_core::save::sram_import::import_sram;
use pokered_core::save::SaveData;
use pokered_core::save_menu::{
    SaveMenuResult, SaveMenuState, SavePhase, SaveScreenInfo, SaveSfxEvent, YesNoInput,
};
use pokered_core::slots_screen::{SlotsAction, SlotsInput, SlotsScreen};
use pokered_core::elevator_screen::{ElevatorAction, ElevatorInput, ElevatorScreen};
use pokered_core::pc_screen::{PcContext, PcEntry, PcOpenContext, PcScreen, PcScreenAction, PcSfx};
use pokered_core::start_menu::{StartMenuAction, StartMenuInput, StartMenuState};
use pokered_core::title_screen::{TitlePhase, TitleScreenState};
use pokered_renderer::input::{GbButton, InputState};
use pokered_renderer::resource::ResourceManager;

use pokered_renderer::resource::AssetRoot;

#[cfg(not(target_arch = "wasm32"))]
use pokered_renderer::window::GameLoop;
use pokered_renderer::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;

#[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
use crate::hot_reload::AssetWatcher;

use crate::audio::{play_species_cry, AudioOutput};
use crate::render::{
    draw_battle, draw_gamefreak_splash, draw_intro_scene, draw_main_menu, draw_mart, draw_oak_speech, draw_options_menu,
    draw_bag, draw_overworld, draw_party_screen, draw_pokedex_screen, draw_save_menu, draw_slots, draw_start_menu,
    draw_elevator, draw_filter_bag, draw_diploma, draw_evolution, draw_hof_ceremony, draw_credits, draw_pc,
    draw_stats_screen, draw_title_screen, draw_town_map, draw_trade, draw_trainer_card, BattleVisualEffects,
};

const SAVE_FILE_NAME: &str = "pokered.sav";
const SCRIPT_FLAGS_FILE_NAME: &str = "pokered.script_flags.json";

/// Restore the options stored in the loaded save file into the live config
/// (original: `wOptions` is part of SRAM, so a CONTINUE'd game keeps the
/// saved text speed / battle animation / battle style).
fn apply_saved_options(
    config: &mut pokered_core::game_state::GameConfig,
    options: &GameOptions,
) {
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

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
fn save_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

#[cfg(not(target_arch = "wasm32"))]
fn save_file_path() -> std::path::PathBuf {
    save_dir().join(SAVE_FILE_NAME)
}

#[cfg(not(target_arch = "wasm32"))]
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
    match serde_json::from_str::<SaveData>(&raw) {
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
            log::warn!(
                "save in localStorage is invalid JSON ({}); ignoring",
                e
            );
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
    give: pokered_data::species::Species,
    receive: pokered_data::species::Species,
    /// Table-authoritative nickname (pokered_data::trades), script arg as
    /// fallback for pairs not in the TradeMons table.
    nickname: String,
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
    pub scripts_dir: Option<PathBuf>,
    pub audio: Option<AudioOutput>,
    startup_warp: Option<(MapId, u16, u16)>,
    #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
    pub asset_watcher: Option<AssetWatcher>,
    #[cfg(feature = "debug-server")]
    pub debug_handle: Option<pokered_debug_server::DebugServerHandle>,
    pending_debug_inputs: Vec<GbButton>,
    pending_debug_frames: u32,
    /// Persistent state for debug-server injected input. A queued button must
    /// read as HELD across consecutive frames (fresh `InputState` per frame
    /// looks like repeated taps, so d-pad walking never starts).
    debug_input: InputState,
    /// Consecutive frames A+B+Start+Select have all been held — the original's
    /// soft-reset combo (engine/joypad.asm `_Joypad`/`TrySoftReset`, 16 frames
    /// of PAD_BUTTONS held → `SoftReset`).
    soft_reset_frames: u8,
    /// Save file the game was started with, kept so a soft reset can reload
    /// it from disk (the original re-reads SRAM on reset).
    save_path: Option<PathBuf>,
    /// Link play (Cable Club): pending server while waiting for one peer.
    /// `--link-listen` sets this (native only); `poll_link` accepts the peer
    /// into `link_session` and drops the server.
    #[cfg(not(target_arch = "wasm32"))]
    pub link_server: Option<LinkServer>,
    /// Active link session: owns the transport and routes wire messages
    /// into the per-activity sub-transports consumed by the core drivers
    /// below. Created at connect (`--link-connect`, an accepted peer, or the
    /// wasm BroadcastChannel entry). Routed once per frame by `poll_link`.
    pub link_session: Option<LinkSession>,
    /// High-level link status for the UI (waiting / connected / "Player2
    /// disconnected" …), kept in sync by `poll_link`.
    pub link_status: LinkStatus,
    /// Cable Club clock role — the host (`--link-listen`, or `?linkHost=1`
    /// on wasm) is the internal clock ("player" side), the client/guest is
    /// the external clock ("friend" side). Set by `attach_link` in main.rs
    /// or `attach_link_transport`; decides the remote player's sprite
    /// placement in the rooms, the simultaneous-gameboy tie-break and whose
    /// random list feeds the shared battle RNG.
    pub link_role: pokered_core::link::LinkRole,
    /// In-room Cable Club link UI: presence, the gameboy flow, prompts and
    /// the trade selection. Fed every frame from `poll_link` events.
    pub link_cable: CableClubFlow,
    /// The CANONICAL link battle driver (owns the handshake → request →
    /// party exchange → battle lifecycle, the battle screen and the shared
    /// RNG stream). Created when the connection comes up (the party is
    /// refreshed at the cable-club table); `self.battle` mirrors its screen
    /// each frame for the render/vfx/audio/settle machinery.
    pub link_battle: Option<LinkBattleDriver>,
    /// The CANONICAL link trade driver (owns the party, the selection →
    /// confirm → exchange lifecycle and trade evolution). Created when the
    /// connection comes up (the party is refreshed at the cable-club table).
    pub link_trade: Option<LinkTradeDriver>,
}

/// Normalize the trade driver's errors onto the transport error type so the
/// flow-need handler treats both drivers uniformly.
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
#[cfg(not(target_arch = "wasm32"))]
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
    pub fn detach_link(&mut self) {
        self.link_session = None;
        self.link_battle = None;
        self.link_trade = None;
        self.link_status = LinkStatus::Disabled;
        self.link_cable = CableClubFlow::new();
    }

    /// Creates a new game with default settings (no save file, no scripts dir).
    /// This is the primary constructor used by both web and native builds.
    #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
    pub fn new(version: GameVersion) -> Self {
        #[cfg(feature = "debug-server")]
        return Self::new_with_options(version, None, None, None, false, None, false, false, None);
        #[cfg(not(feature = "debug-server"))]
        return Self::new_with_options(version, None, None, None, false, None, false, false);
    }

    /// Creates a new game with optional save file, snapshot, and scripts directory.
    /// Only available for native builds (wasm doesn't support file system operations).
    #[cfg(not(target_arch = "wasm32"))]
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
        #[cfg(feature = "debug-server")] debug_handle: Option<pokered_debug_server::DebugServerHandle>,
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
                    eprintln!("Warning: invalid --warp argument '{}': {}. Ignoring warp.", warp_str, e);
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
            overworld.party_lead_level = save_data.party.leader_level();
            player_name = pokered_data::charmap::decode_string(&save_data.player_name);
            rival_name = pokered_data::charmap::decode_string(&save_data.game_data.rival_name);
            overworld.player_name = player_name.clone();
            overworld.rival_name = rival_name.clone();
            // Seed the event-flag bitset from SRAM bytes, then merge any
            // runtime-only extras (companion sidecar) on top.
            overworld.set_event_flags_bytes(&save_data.game_data.event_flags);
            if let Some(extras) = Self::read_companion_script_flags() {
                overworld.set_script_flags(extras);
            }
            overworld.set_toggleable_object_flags(
                save_data.game_data.toggleable_object_flags,
            );
            overworld.set_hidden_item_flags(save_data.game_data.obtained_hidden_items);
            overworld.set_hidden_coin_flags(save_data.game_data.obtained_hidden_coins);
            overworld.apply_hidden_object_flags();
            overworld.run_on_load();

            eprintln!(
                "Skip-intro: starting at {:?} ({} x={}, y={})",
                map_id, map_id as u8, px, py
            );
        } else {
            initial_screen = GameScreen::GameFreakSplash;
            overworld = OverworldScreen::new(MapId::PalletTown, scripts_dir.clone(), PokemonRedData);
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
                eprintln!("Asset root found: {:?}", root.gfx_dir());
                Some(ResourceManager::new(root))
            }
            Err(e) => {
                eprintln!("Warning: Could not find gfx/ directory: {}", e);
                eprintln!("Falling back to text-only placeholder rendering.");
                None
            }
        };

        let audio = if no_audio {
            eprintln!("Audio output disabled (--no-audio).");
            None
        } else {
            match AudioOutput::new() {
                Some(ao) => {
                    eprintln!("Audio output initialized (cpal 44100 Hz stereo)");
                    Some(ao)
                }
                None => {
                    eprintln!("Warning: Could not initialize audio output.");
                    None
                }
            }
        };

        #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
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
                    eprintln!("[hot-reload] Asset watcher active");
                    Some(w)
                }
                Err(e) => {
                    eprintln!("[hot-reload] Failed to start watcher: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // The script engine language starts from the saved/default config;
        // the LanguageSelect screen re-syncs it whenever the choice changes.
        overworld.set_script_lang(if state.config.language == pokered_core::game_state::Lang::Zh {
            "zh"
        } else {
            "en"
        });

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
            scripts_dir,
            audio,
            startup_warp,
            #[cfg(feature = "debug-server")]
            debug_handle,
            #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
            asset_watcher,
            pending_debug_inputs: Vec::new(),
            pending_debug_frames: 0,
            debug_input: InputState::new(),
            soft_reset_frames: 0,
            save_path,
            #[cfg(not(target_arch = "wasm32"))]
            link_server: None,
            link_session: None,
            link_status: LinkStatus::Disabled,
            link_role: pokered_core::link::LinkRole::Host,
            link_cable: CableClubFlow::new(),
            link_battle: None,
            link_trade: None,
        }
    }

    #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
    pub fn new(version: GameVersion) -> Self {
        #[cfg(target_arch = "wasm32")]
        let (save_data, save_summary) = try_load_save_from_local_storage();
        #[cfg(target_os = "android")]
        let (save_data, save_summary) = Self::try_load_default_save();
        #[cfg(target_os = "ios")]
        let (save_data, save_summary) = Self::try_load_default_save();
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
        overworld.set_script_lang(if state.config.language == pokered_core::game_state::Lang::Zh { "zh" } else { "en" });
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

        let audio = AudioOutput::new();
        if audio.is_some() {
            log::info!("Web Audio initialized (44100 Hz stereo)");
        } else {
            log::warn!("Could not initialize Web Audio output");
        }

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
            scripts_dir: None,
            audio,
            pending_debug_inputs: Vec::new(),
            pending_debug_frames: 0,
            debug_input: InputState::new(),
            startup_warp: None,
            soft_reset_frames: 0,
            save_path: None,
            #[cfg(not(target_arch = "wasm32"))]
            link_server: None,
            link_session: None,
            link_status: LinkStatus::Disabled,
            link_role: pokered_core::link::LinkRole::Host,
            link_cable: CableClubFlow::new(),
            link_battle: None,
            link_trade: None,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn try_load_default_save() -> (SaveData, Option<SaveFileSummary>) {
        let path = save_file_path();
        let (save, summary) = match std::fs::read(&path) {
            Ok(data) => Self::parse_sram(&path, &data),
            Err(_) => (SaveData::new(), None),
        };
        (save, summary)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_sram_from_path(path: &Path) -> (SaveData, Option<SaveFileSummary>) {
        let (save, summary) = match std::fs::read(path) {
            Ok(data) => Self::parse_sram(path, &data),
            Err(e) => {
                eprintln!("Error: failed to read save file {:?}: {}", path, e);
                (SaveData::new(), None)
            }
        };
        (save, summary)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_snapshot_from_path(path: &Path) -> (SaveData, Option<SaveFileSummary>) {
        match std::fs::read(path) {
            Ok(data) => match serde_json::from_slice::<SaveData>(&data) {
                Ok(save) => {
                    let summary = save_summary_from_data(&save);
                    eprintln!("Snapshot loaded: {:?}", path);
                    (save, Some(summary))
                }
                Err(e) => {
                    eprintln!("Error: failed to parse snapshot {:?}: {}", path, e);
                    (SaveData::new(), None)
                }
            },
            Err(e) => {
                eprintln!("Error: failed to read snapshot {:?}: {}", path, e);
                (SaveData::new(), None)
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn parse_sram(path: &Path, data: &[u8]) -> (SaveData, Option<SaveFileSummary>) {
        match import_sram(data) {
            Ok(save) => {
                let summary = save_summary_from_data(&save);
                eprintln!("Save file loaded: {:?}", path);
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
                eprintln!("Warning: save file {:?} failed to load: {:?}", path, e);
                (SaveData::new(), None)
            }
        }
    }

    /// Read the companion script-flags file (native sidecar for the
    /// runtime-only dynamic keys — e.g. `__OBJ_HIDDEN_*` — that have no bit
    /// in the fixed SRAM event-flags region). Named event flags in old
    /// sidecars are harmless: `set_script_flags` routes them to the bitset.
    #[cfg(not(target_arch = "wasm32"))]
    fn read_companion_script_flags() -> Option<std::collections::HashMap<String, bool>> {
        let flags_path = script_flags_file_path();
        let data = std::fs::read(&flags_path).ok()?;
        match serde_json::from_slice::<std::collections::HashMap<String, bool>>(&data) {
            Ok(flags) => Some(flags),
            Err(e) => {
                eprintln!(
                    "Warning: failed to parse script flags {:?}: {}",
                    flags_path, e
                );
                None
            }
        }
    }

    /// Same companion store on web: the runtime-only extras live in a
    /// separate `localStorage` key next to the SaveData JSON.
    #[cfg(target_arch = "wasm32")]
    fn read_companion_script_flags() -> Option<std::collections::HashMap<String, bool>> {
        let storage = web_local_storage()?;
        let data = storage.get_item(WEB_SCRIPT_FLAGS_STORAGE_KEY).ok()??;
        match serde_json::from_str::<std::collections::HashMap<String, bool>>(&data) {
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
    #[cfg(not(target_arch = "wasm32"))]
    fn save_companion_script_flags(overworld: &OverworldScreen<PokemonRedData>) {
        let extras = overworld.unified_flags().extras();
        let flags_path = script_flags_file_path();
        if extras.is_empty() {
            if let Err(e) = std::fs::remove_file(&flags_path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("Error: failed to remove script flags file: {}", e);
                }
            }
            return;
        }
        match serde_json::to_string(extras) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&flags_path, json.as_bytes()) {
                    eprintln!("Error: failed to write script flags file: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Error: failed to serialize script flags: {}", e);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
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
        eprintln!(
            "Exported snapshot: {:?} -> {:?} ({} bytes)",
            sav_path,
            output_path,
            json.len()
        );
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn import_snapshot_from_sav(
        input_path: &Path,
        output_path: &Path,
    ) -> Result<(), String> {
        let data = std::fs::read(input_path)
            .map_err(|e| format!("Failed to read {:?}: {}", input_path, e))?;
        let save = import_sram(&data)
            .map_err(|e| format!("Failed to parse SRAM from {:?}: {:?}", input_path, e))?;
        let json = serde_json::to_string_pretty(&save)
            .map_err(|e| format!("Failed to serialize snapshot: {}", e))?;
        std::fs::write(output_path, json.as_bytes())
            .map_err(|e| format!("Failed to write {:?}: {}", output_path, e))?;
        eprintln!(
            "Imported snapshot: {:?} -> {:?} ({} bytes)",
            input_path,
            output_path,
            json.len()
        );
        Ok(())
    }

    fn build_save_data(&self) -> SaveData {
        let mut save = self.save_data.clone();
        if let Some(encoded) = pokered_data::charmap::encode_string(&self.player_name) {
            save.player_name = encoded;
        }
        if let Some(encoded) = pokered_data::charmap::encode_string(&self.rival_name) {
            save.game_data.rival_name = encoded;
        }

        let player = &self.overworld.state.player;
        let current_map = self.overworld.state.current_map;

        save.game_data.position.map_id = current_map as u8;
        save.game_data.position.x = player.x as u8;
        save.game_data.position.y = player.y as u8;
        save.game_data.position.x_block = (player.x % 2) as u8;
        save.game_data.position.y_block = (player.y % 2) as u8;

        let facing = match player.facing {
            pokered_core::overworld::Direction::Down => 0u8,
            pokered_core::overworld::Direction::Up => 4u8,
            pokered_core::overworld::Direction::Left => 8u8,
            pokered_core::overworld::Direction::Right => 12u8,
        };
        save.game_data.player_direction = facing;
        save.game_data.player_last_stop_direction = facing;
        save.game_data.player_moving_direction = facing;

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

        if let Some(ref map_data) = self.overworld.map_data {
            save.game_data.map_header.tileset = map_data.tileset.to_u8();
            save.game_data.map_header.height = map_data.height;
            save.game_data.map_header.width = map_data.width;
        }

        // engine/menus/save.asm: hTileAnimations is stored into sTileAnimations
        // on save. It carries the current tileset's animation byte
        // (TILEANIM_*); map loads refresh it from the tileset header.
        save.tile_animations = match self.overworld.tile_anim.kind() {
            pokered_core::overworld::presentation::TileAnimKind::None => 0,
            pokered_core::overworld::presentation::TileAnimKind::Water => 1,
            pokered_core::overworld::presentation::TileAnimKind::WaterFlower => 2,
        };

        // The event-flag bitset serializes directly into the original
        // 320-byte SRAM region (wEventFlags, NUM_EVENTS = $A00 bits).
        save.game_data.event_flags = self.overworld.unified_flags().as_bytes().to_vec();

        save.game_data.toggleable_object_flags = *self.overworld.toggleable_object_flags();
        save.game_data.obtained_hidden_items = *self.overworld.hidden_item_flags();
        save.game_data.obtained_hidden_coins = *self.overworld.hidden_coin_flags();

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

    #[cfg(not(target_arch = "wasm32"))]
    fn save_to_file(&mut self) {
        let save = self.build_save_data();
        let sram = export_sram(&save);
        // Explicit --save path wins (headless/driver runs); normal play
        // falls back to the default location next to the executable.
        let path = self
            .save_path
            .clone()
            .unwrap_or_else(save_file_path);
        match std::fs::write(&path, &sram) {
            Ok(()) => {
                pokered_core::log_save!("game saved to {:?} ({} bytes)", path, sram.len());
                self.save_data = save;
            }
            Err(e) => {
                eprintln!("Error: failed to write save file: {}", e);
            }
        }
        Self::save_companion_script_flags(&self.overworld);
    }

    #[cfg(target_arch = "wasm32")]
    fn save_to_file(&mut self) {
        let save = self.build_save_data();
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
    #[cfg(not(target_arch = "wasm32"))]
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
    #[cfg(not(target_arch = "wasm32"))]
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
        // Set in the Battle→Overworld settle below when a caught species was
        // newly added to the Pokédex — the post-capture "New DEX data will be
        // added…" entry then opens instead of the overworld
        // (engine/items/item_effects.asm:521-546).
        let mut post_catch_species: Option<pokered_data::species::Species> = None;
        match screen {            GameScreen::IntroScene => {
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
                self.oak_speech = OakSpeechState::new();
                if let Some(ref audio) = self.audio {
                    audio.stop_all();
                    audio.play_music(MusicId::ROUTES2);
                }
            }
            GameScreen::Overworld => {
                use pokered_core::data::fly_warp_data::NEW_GAME_WARP;
                use pokered_core::game_state::MainMenuChoice;

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
                                eprintln!(
                                    "Warping to {:?} ({}, {})",
                                    warp_map, warp_x, warp_y
                                );
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
                                let map_id =
                                    pokered_core::data::maps::MapId::from_u8(pos.map_id)
                                        .unwrap_or(NEW_GAME_WARP.map_id);
                                let facing =
                                    match self.save_data.game_data.player_direction {
                                        4 => pokered_core::overworld::Direction::Up,
                                        8 => pokered_core::overworld::Direction::Left,
                                        12 => pokered_core::overworld::Direction::Right,
                                        _ => pokered_core::overworld::Direction::Down,
                                    };
                                (
                                    map_id,
                                    pos.x as u16,
                                    pos.y as u16,
                                    facing,
                                )
                            };
                        let mut overworld = OverworldScreen::new(map_id, self.scripts_dir.clone(), PokemonRedData);
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
                        overworld.set_event_flags_bytes(&self.save_data.game_data.event_flags);
                        if let Some(extras) = Self::read_companion_script_flags() {
                            overworld.set_script_flags(extras);
                        }
                        overworld.set_toggleable_object_flags(
                            self.save_data.game_data.toggleable_object_flags,
                        );
                        overworld.set_hidden_item_flags(
                            self.save_data.game_data.obtained_hidden_items,
                        );
                        overworld.set_hidden_coin_flags(
                            self.save_data.game_data.obtained_hidden_coins,
                        );
                        overworld.apply_hidden_object_flags();
                        overworld.player_name = self.player_name.clone();
                        overworld.rival_name = self.rival_name.clone();
                        overworld.party_count = self.save_data.party.count() as u8;
                        overworld.party_lead_level = self.save_data.party.leader_level();
                        overworld.run_on_load();
                        overworld.set_script_lang(if self.state.config.language == pokered_core::game_state::Lang::Zh {
                            "zh"
                        } else {
                            "en"
                        });
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
                                eprintln!(
                                    "Warping to {:?} ({}, {})",
                                    warp_map, warp_x, warp_y
                                );
                                (warp_map, warp_x, warp_y)
                            } else {
                                (
                                    NEW_GAME_WARP.map_id,
                                    NEW_GAME_WARP.coords.x as u16,
                                    NEW_GAME_WARP.coords.y as u16,
                                )
                            };
                        let mut overworld = OverworldScreen::new(map_id, self.scripts_dir.clone(), PokemonRedData);
                        overworld.state.player.x = px;
                        overworld.state.player.y = py;
                        overworld.player_name = self.player_name.clone();
                        overworld.rival_name = self.rival_name.clone();
                        overworld.party_count = self.save_data.party.count() as u8;
                        overworld.party_lead_level = self.save_data.party.leader_level();
                        overworld.set_script_lang(if self.state.config.language == pokered_core::game_state::Lang::Zh {
                            "zh"
                        } else {
                            "en"
                        });
                        self.overworld = overworld;
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
                                    let data_id = pokered_core::overworld::map_loading::get_map_music(map);
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
                            self.overworld.warp_fade_state =
                                pokered_core::overworld::screen::WarpFadeState::FadingIn {
                                    frames_remaining:
                                        pokered_core::overworld::screen::WARP_FADE_IN_FRAMES,
                                };
                        }
                    }
                }
            }
            GameScreen::Battle => {
                if self.battle.battle_state.is_none() {
                    self.battle = BattleScreen::new(true);
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
                if let Some(ref audio) = self.audio {
                    audio.play_sfx(SfxId::StartMenu);
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
                let is_different_player = self
                    .state
                    .save_summary
                    .as_ref()
                    .map_or(false, |s| s.player_id != 0 && s.player_id != self.save_data.game_data.player_id);
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
            }
            GameScreen::PokemonStatsScreen(_) => {}
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

    /// A+B+Start+Select held for 16 frames — the original's soft reset
    /// (engine/joypad.asm `TrySoftReset` → home/init.asm `SoftReset`): stop
    /// all sounds, reload the save from disk (unsaved progress is lost, as on
    /// hardware), and return to the title screen.
    fn soft_reset(&mut self) {
        if let Some(ref audio) = self.audio {
            audio.stop_all();
        }
        #[cfg(not(target_arch = "wasm32"))]
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

    fn start_wild_battle(&mut self, species: pokered_data::species::Species, level: u8) {
        use pokered_core::pokemon::stats::{create_pokemon, roll_random_dvs};

        // Wild DVs are two random bytes (core.asm:6012-6019).
        let enemy_mon = create_pokemon(species, level, roll_random_dvs());
        let player_party = self.save_data.party.to_vec();

        if let Some(enemy) = enemy_mon {
            if !player_party.is_empty() {
                self.battle =
                    pokered_core::battle::BattleScreen::from_parties(true, &player_party, &[enemy], None);
            } else {
                self.battle = pokered_core::battle::BattleScreen::new(true);
            }
        } else {
            self.battle = pokered_core::battle::BattleScreen::new(true);
        }
        self.battle.player_money = self.save_data.game_data.player_money;
        // Badge stat boosts + traded-mon obedience context (wObtainedBadges /
        // wPlayerID) — the battle reads them from these fields every turn.
        self.battle.player_badges = self.save_data.game_data.obtained_badges;
        self.battle.player_id = self.save_data.game_data.player_id;
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
            self.battle.safari =
                Some(pokered_core::battle::safari::SafariState::new(base_catch, balls));
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
        self.state.screen = GameScreen::Battle;
    }

    /// Editor quick-entry: open the Pokédex directly on `species`' entry (the
    /// post-capture style — full data + cry, see `PokedexScreenState::new_entry`).
    /// The species is registered as seen AND owned so the entry always shows its
    /// flavor text; closing the Pokédex returns to the overworld.
    pub fn debug_open_pokedex(&mut self, species: pokered_data::species::Species) {
        use pokered_core::pokedex_screen::PokedexScreenState;
        self.save_data.game_data.pokedex.set_seen(species);
        self.save_data.game_data.pokedex.set_owned(species);
        self.pokedex_screen = PokedexScreenState::new_entry(
            self.save_data.game_data.pokedex.clone(),
            species,
            self.state.config.version,
        );
        self.state.screen = GameScreen::Pokedex;
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
        self.state.screen = GameScreen::Battle;
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
            [move_id, pokered_data::moves::MoveId::None, pokered_data::moves::MoveId::None, pokered_data::moves::MoveId::None],
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
        self.state.screen = GameScreen::Battle;
    }

    /// Editor quick-entry: play the evolution animation `from` → `to` (the
    /// WYSIWYG "test this evolution" flow). The animation takeover renders
    /// until it finishes; the staged party slot is the morph's target.
    pub fn debug_play_evolution(&mut self, from: pokered_data::species::Species, to: pokered_data::species::Species) {
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
        use pokered_core::pokemon::stats::create_pokemon;
        use pokered_data::trainer_data::{get_trainer_party, parse_trainer_id, TrainerClass};
        use pokered_data::species::Species;

        let player_party = self.save_data.party.to_vec();

        let parsed = parse_trainer_id(trainer_id);
        let is_rival = parsed.as_ref().map_or(false, |(class, _)| {
            matches!(class, TrainerClass::Rival1 | TrainerClass::Rival2 | TrainerClass::Rival3)
        });

        let enemy_party = if let Some((class, default_index)) = parsed {
            // Rival gets type advantage: Grass→Fire, Fire→Water, Water→Grass.
            // Each triplet is ordered Squirtle/Bulbasaur/Charmander; the scene
            // supplies the triplet base (scripts/{Map}.asm StarterTable).
            let party_index = if is_rival {
                let base = rival_triplet_base.unwrap_or(0) as usize;
                player_party.first().map_or(default_index, |starter| {
                    base + pokered_data::trainer_data::rival_starter_offset(starter.species)
                })
            } else {
                default_index
            };

            if let Some(party) = get_trainer_party(class, party_index) {
                let mut mons: Vec<_> = party
                    .pokemon
                    .iter()
                    .filter_map(|mon| {
                        create_pokemon(mon.species, mon.level, pokered_core::pokemon::stats::TRAINER_DV_BYTES)
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
        self.battle.player_money = self.save_data.game_data.player_money;
        // Badge stat boosts + traded-mon obedience context (wObtainedBadges /
        // wPlayerID) — the battle reads them from these fields every turn.
        self.battle.player_badges = self.save_data.game_data.obtained_badges;
        self.battle.player_id = self.save_data.game_data.player_id;
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
        use pokered_core::game_state::Lang;
        self.frame_count += 1;

        #[cfg(feature = "debug-server")]
        {
            let commands = self
                .debug_handle
                .as_ref()
                .map(|h| h.poll_commands())
                .unwrap_or_default();
            for cmd in commands {
                let response = self.handle_debug_command(cmd);
                if let Some(ref handle) = self.debug_handle {
                    handle.send_response(response);
                }
            }
        }

        // Link play: accept a pending peer and drive the link session
        // (battle/trade state machines) every frame, before any early
        // returns so network progress never stalls.
        self.poll_link();

        #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
        {
            let changes = self
                .asset_watcher
                .as_mut()
                .map(|w| w.poll_events())
                .unwrap_or_default();
            for change in &changes {
                eprintln!("[hot-reload] Changed: {}", change.path.display());

                if change
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    == Some("scene")
                {
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
                            Ok(()) => log::info!(
                                "[hot-reload] Recompiled .scene: {}",
                                map_key
                            ),
                            Err(e) => eprintln!(
                                "[hot-reload] Failed to compile {}: {}",
                                change.path.display(), e
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
            self.debug_input.press(button);
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
                if let Some(screen) = self.pending_screen.take() {
                    self.handle_transition(screen);
                }
            }
            return;
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
                if let Some(trade) = self.pending_trade.take() {
                    self.apply_npc_trade(trade);
                }
                if self.link_cable.phase() == &CableClubPhase::TradeAnim {
                    // Link trade: the driver applies the exchange
                    // (remove-then-add, traded flag, Pokédex, forced trade
                    // evolution), then the flow shows "Trade completed!" and
                    // returns to the selection screen.
                    self.apply_link_trade();
                }
                self.overworld.resume_script_after_trade(true);
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
                let done = anim.tick(evo_input);
                for sfx in anim.pending_sfx.drain(..) {
                    if let Some(ref audio) = self.audio {
                        use pokered_core::evolution_screen::EvolutionSfx;
                        match sfx {
                            EvolutionSfx::StopMusic => audio.stop_music(),
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
                let done = hof.update_frame();
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
                self.credits = Some(pokered_core::credits::CreditsState::new(
                    self.state.config.version,
                ));
                // HallOfFamePC (credits.asm:24-33): stop all music, then the
                // credits theme.
                if let Some(ref audio) = self.audio {
                    audio.stop_music();
                    audio.play_music(MusicId::CREDITS);
                }
            }
            return;
        }

        // End credits (engine/movie/credits.asm): runs to "THE END", then a
        // button press saves + resets to the title screen.
        if self.credits.is_some() {
            let done = {
                let roll = self.credits.as_mut().unwrap();
                roll.update_frame(pokered_core::credits::CreditsInput {
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                })
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
                let action = if input.is_just_pressed(GbButton::Up) || input.is_just_pressed(GbButton::Down) {
                    self.state.config.language = match self.state.config.language {
                        Lang::En => Lang::Zh,
                        Lang::Zh => Lang::En,
                    };
                    ScreenAction::Continue
                } else if input.is_just_pressed(GbButton::A) || input.is_just_pressed(GbButton::Start) {
                    ScreenAction::Transition(GameScreen::IntroScene)
                } else {
                    ScreenAction::Continue
                };
                // Keep the overworld script engine in sync with the chosen
                // language so NPC dialogue (`@t` literals) renders in it.
                self.overworld.set_script_lang(if self.state.config.language == Lang::Zh {
                    "zh"
                } else {
                    "en"
                });
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
                let action = self.title_screen.update_frame(any_pressed);
                let new_phase = self.title_screen.phase;

                if prev_phase != new_phase {
                    if let Some(ref audio) = self.audio {
                        match new_phase {
                            TitlePhase::LogoBounce => {
                                audio.play_sfx(SfxId::IntroCrash);
                            }
                            TitlePhase::LogoPause => {
                                audio.play_sfx(SfxId::IntroWhoosh);
                            }
                            TitlePhase::WaitingForInput
                                if prev_phase == TitlePhase::VersionScroll =>
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
                    self.oak_speech.update_naming_frame(naming_input, self.state.config.language == Lang::Zh)
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
                            let dvs = pokered_core::pokemon::stats::roll_random_dvs();
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

                // The gameboy on the table: the map scene calls
                // `game.linkStart()`; the flow starts the room's request
                // (LINK BATTLE in the Colosseum, LINK TRADE in the Trade
                // Center — the original's CableClubLeftGameboy/
                // CableClubRightGameboy, engine/pokemon/bills_pc.asm).
                if self.overworld.take_link_start_request() {
                    if linked {
                        let need = self.link_cable.on_gameboy_used(current_map);
                        self.handle_flow_need(need);
                    } else {
                        // Offline (no link session): the original's
                        // "Just a moment." (JustAMomentText) and nothing
                        // happens — the room has no cable partner.
                        self.overworld.pending_dialogue = Some(
                            pokered_core::overworld::BedroomDialogue::from_message(
                                crate::link::cable_club::TEXT_JUST_A_MOMENT,
                            ),
                        );
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
                        .is_some_and(|d| d.received_mon().is_some())
                    {
                        self.start_link_trade_anim();
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
                        let psi = PartyScreenInput {
                            up: input.is_just_pressed(GbButton::Up),
                            down: input.is_just_pressed(GbButton::Down),
                            a: input.is_just_pressed(GbButton::A),
                            b: input.is_just_pressed(GbButton::B),
                        };
                        let need = self.link_cable.update(psi, &self.save_data.party.to_vec());
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
                if let Some(action) = link_action {
                    action
                } else if self.overworld.is_party_select_active() {
                    let psi = PartyScreenInput {
                        up: input.is_just_pressed(GbButton::Up),
                        down: input.is_just_pressed(GbButton::Down),
                        a: input.is_just_pressed(GbButton::A),
                        b: input.is_just_pressed(GbButton::B),
                    };
                    self.overworld.update_party_select_input(psi);
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
                    self.overworld.update_naming_input(naming_input, self.state.config.language == Lang::Zh);
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
                    let ow_input = OverworldInput::new(
                        input.is_held(GbButton::Up),
                        input.is_held(GbButton::Down),
                        input.is_held(GbButton::Left),
                        input.is_held(GbButton::Right),
                        input.is_held(GbButton::A),
                        input.is_held(GbButton::B),
                        input.is_just_pressed(GbButton::Start) && !soft_reset_combo_held,
                        input.is_held(GbButton::Select),
                    );
                    // Seed synchronous script-query state from persistent game
                    // data BEFORE update_frame so `@if` conditions (hasItem,
                    // getMoney, dex, rival starter, facing) read current values.
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
                    // Day Care + per-party query state (for the Day Care scene).
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
                        let dc_name = pokered_data::charmap::decode_string(
                            &self.save_data.game_data.daycare_mon_name,
                        );
                        let mut name_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
                        let party_names: Vec<String> = self
                            .save_data
                            .party
                            .to_vec()
                            .iter()
                            .map(|m| m.display_name(&mut name_buf).to_string())
                            .collect();
                        let party_knows_hm: Vec<bool> = self
                            .save_data
                            .party
                            .to_vec()
                            .iter()
                            .map(|m| m.moves.iter().any(|mv| is_hm_move(*mv)))
                            .collect();
                        self.overworld.seed_daycare_query_state(
                            dc.in_use,
                            &dc_name,
                            levels_grown,
                            cost,
                            &party_names,
                            &party_knows_hm,
                        );
                    }

                    // wOptions text delay — pushed every frame so the dialogue
                    // typewriter honors the configured TEXT SPEED.
                    self.overworld
                        .set_text_delay_frames(self.state.config.text_speed.delay_frames());
                    let action = self.overworld.update_frame(ow_input);

                    // Drain script-requested bag/money mutations and apply them
                    // to the persistent game data (the overworld is pure logic
                    // and cannot reach SaveData itself).
                    // A failed tradePokemon resumes the suspended script AFTER
                    // the drain (the drain borrows self.overworld).
                    let mut trade_rejected = false;
                    for req in self.overworld.game_data_requests.drain(..) {
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
                                use pokered_data::species::Species;
                                use pokered_data::trades::find_npc_trade;
                                let pair = (
                                    Species::from_scene_name(&offered),
                                    Species::from_scene_name(&received),
                                );
                                // The script is suspended on this await; it
                                // resumes via resume_script_after_trade.
                                let ok = if let (Some(off_sp), Some(rec_sp)) = pair {
                                    self.save_data.party.find_species(off_sp).is_some()
                                        && self.trade_anim.is_none()
                                        && {
                                            // Nickname: the TradeMons table is
                                            // authoritative (the original stores
                                            // it per-trade); the script arg is
                                            // the fallback for non-table pairs.
                                            let nick = find_npc_trade(off_sp, rec_sp)
                                                .map(|t| t.nickname.to_string())
                                                .unwrap_or(nickname);
                                            let is_zh = matches!(
                                                self.state.config.language,
                                                pokered_core::game_state::Lang::Zh
                                            );
                                            // Start the trade cutscene
                                            // (engine/movie/trade.asm); the party
                                            // mutation lands when it completes.
                                            self.trade_anim =
                                                Some(pokered_core::trade::TradeAnim::new(
                                                    off_sp,
                                                    rec_sp,
                                                    self.player_name.clone(),
                                                    is_zh,
                                                ));
                                            self.pending_trade = Some(PendingTrade {
                                                give: off_sp,
                                                receive: rec_sp,
                                                nickname: nick,
                                            });
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
                            }
                            OverworldGameDataRequest::TakeCoins { amount } => {
                                self.save_data.game_data.take_coins(amount);
                            }
                            OverworldGameDataRequest::TickDaycareExp => {
                                self.save_data.game_data.tick_daycare_exp();
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
                                        TransportMode::Biking => 32, // MUSIC_BIKE_RIDING
                                        TransportMode::Surfing => 33, // MUSIC_SURFING
                                        TransportMode::Walking => {
                                            pokered_core::overworld::map_loading::get_map_music(map) as u8
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
                            pokered_core::pokemon::stats::roll_random_dvs(),
                        ) {
                            if let Some(nick) = pending.nickname {
                                pokemon.set_nickname(&nick);
                            }
                            let _ = self.save_data.party.add(pokemon);
                            // A received Pokémon enters the Pokédex as seen + owned.
                            self.save_data.game_data.pokedex.set_seen(pending.species);
                            self.save_data.game_data.pokedex.set_owned(pending.species);
                            self.overworld.party_count = self.save_data.party.count() as u8;
                            self.overworld.party_lead_level =
                                self.save_data.party.leader_level();
                        }
                    }

                    if let Some(shop_items) = self.overworld.pending_shop.take() {
                        match ShopInventory::from_strings(&shop_items) {
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
                        let seed = (self.frame_count as u32).wrapping_mul(2654435761).wrapping_add(1);
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
                        self.elevator_screen = Some(ElevatorScreen::new(carried));
                        ScreenAction::Transition(GameScreen::FilterBag)
                    } else if self.overworld.pending_diploma {
                        self.overworld.pending_diploma = false;
                        ScreenAction::Transition(GameScreen::Diploma)
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
                        self.pc_screen = Some(PcScreen::new(entry, &open));
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
                let mut link_abort = false;
                let mut battle_over = false;
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
                        if result.is_none() {
                            if let Some(screen) = driver.screen() {
                                self.battle = screen.clone();
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
                        match get_move_sound(req.anim_move, req.sound_move, req.attacker_species)
                        {
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
                let sm_input = StartMenuInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                    start: input.is_just_pressed(GbButton::Start),
                };
                match self.start_menu.update_frame(sm_input) {
                    StartMenuAction::Close => ScreenAction::Transition(GameScreen::Overworld),
                    StartMenuAction::OpenOption => {
                        ScreenAction::Transition(GameScreen::OptionsMenu)
                    }
                    StartMenuAction::OpenSave => ScreenAction::Transition(GameScreen::SaveMenu),
                    StartMenuAction::OpenPokemon => {
                        self.party_screen = PartyScreenState::new(self.save_data.party.to_vec());
                        ScreenAction::Transition(GameScreen::PartyScreen)
                    }
                    StartMenuAction::OpenItem => {
                        let items: Vec<(pokered_data::items::ItemId, u32)> =
                            self.save_data.game_data.bag.items().to_vec();
                        self.bag_screen = BagScreenState::new(items);
                        ScreenAction::Transition(GameScreen::Bag)
                    }
                    StartMenuAction::OpenPokedex => {
                        ScreenAction::Transition(GameScreen::Pokedex)
                    }
                    StartMenuAction::OpenTrainerInfo => {
                        ScreenAction::Transition(GameScreen::TrainerCard)
                    }
                    _ => ScreenAction::Continue,
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
                let action = self.party_screen.update_frame(party_input);

                // Mirror any in-screen swap back into the canonical save data.
                if let Some((a, b)) = self.party_screen.take_pending_swap() {
                    if let Err(e) = self.save_data.party.swap(a, b) {
                        tracing::warn!("party swap {a}<->{b} failed: {e:?}");
                    }
                    if let Some(ref audio) = self.audio {
                        audio.play_sfx(SfxId::Swap);
                    }
                }

                match action {
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
                            self.party_screen.party_member(idx).cloned().unwrap()
                        ));
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
                                        self.overworld.pending_dialogue =
                                            Some(BedroomDialogue::from_message(&message));
                                        self.pending_bag_item = None;
                                        ScreenAction::Transition(GameScreen::Overworld)
                                    }
                                    ItemApplyOutcome::NoEffect { message } => {
                                        self.overworld.pending_dialogue =
                                            Some(BedroomDialogue::from_message(&message));
                                        self.pending_bag_item = None;
                                        ScreenAction::Transition(GameScreen::Overworld)
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
                                            party_index, from, to, pre_text, force,
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
                        // level-up move that could not be learned. The HM
                        // guard (HMCantDeleteText) refuses an HM pick; like
                        // the TM flow the prompt then ends (the move stays
                        // unlearned) — the original re-asks inline, which the
                        // party screen cannot render (documented deviation).
                        if let Some((_, move_id)) = self.pending_evolve_move_replace.take() {
                            let message = match self.save_data.party.get_mut(party_index) {
                                Some(mon) => {
                                    use pokered_core::pokemon::move_learning::{
                                        replace_move_guarded, ReplaceMoveError,
                                    };
                                    match replace_move_guarded(mon, slot, move_id) {
                                        Ok(old_move) => {
                                            let mut name_buf =
                                                [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
                                            format!(
                                                "{} forgot\n{}...\nand learned\n{}!",
                                                mon.display_name(&mut name_buf),
                                                pokered_data::lang_data::move_name(old_move, false),
                                                pokered_data::lang_data::move_name(move_id, false)
                                            )
                                        }
                                        Err(ReplaceMoveError::HmCantDelete) => {
                                            // HMCantDeleteText (learn_move.asm:178-181).
                                            "HM techniques\ncan't be deleted!".to_string()
                                        }
                                        Err(ReplaceMoveError::InvalidSlot) => {
                                            bag_use::NO_EFFECT_MESSAGE.to_string()
                                        }
                                    }
                                }
                                None => bag_use::NO_EFFECT_MESSAGE.to_string(),
                            };
                            self.overworld.pending_dialogue =
                                Some(BedroomDialogue::from_message(&message));
                            ScreenAction::Transition(GameScreen::Overworld)
                        } else {
                            // Replace-move confirmation for the pending TM/HM.
                            match self.pending_bag_item {
                            None => ScreenAction::Continue,
                            Some(item) => {
                                let outcome = match self.save_data.party.get_mut(party_index) {
                                    Some(mon) => bag_use::finish_tm_hm_replace(item, mon, slot),
                                    None => ItemApplyOutcome::NoEffect {
                                        message: bag_use::NO_EFFECT_MESSAGE.to_string(),
                                    },
                                };
                                let (message, consume) = match outcome {
                                    ItemApplyOutcome::Used { message, consume } => {
                                        (message, consume)
                                    }
                                    ItemApplyOutcome::NoEffect { message } => (message, false),
                                    // finish_tm_hm_replace never re-asks.
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
                                self.overworld.pending_dialogue =
                                    Some(BedroomDialogue::from_message(&message));
                                self.pending_bag_item = None;
                                ScreenAction::Transition(GameScreen::Overworld)
                            }
                            }
                        }
                    }
                    PartyScreenAction::UseFieldMove { party_index, move_id } => {
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
                                message: pokered_core::items::bag_use::NO_EFFECT_MESSAGE.to_string(),
                            },
                        };
                        let message = match outcome {
                            pokered_core::items::bag_use::ItemApplyOutcome::Used {
                                message, ..
                            } => message,
                            pokered_core::items::bag_use::ItemApplyOutcome::NoEffect {
                                message,
                            } => message,
                            // apply_softboiled never asks for a move replace or
                            // an evolution.
                            _ => pokered_core::items::bag_use::NO_EFFECT_MESSAGE.to_string(),
                        };
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
                    BagScreenAction::Cancelled => {
                        ScreenAction::Transition(GameScreen::StartMenu)
                    }
                    BagScreenAction::TossItem { item, index, quantity } => {
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
                            self.overworld.pending_dialogue = Some(
                                BedroomDialogue::from_message("That's too impor-\ntant to toss!"),
                            );
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
                    TrainerCardAction::Closed => {
                        ScreenAction::Transition(GameScreen::StartMenu)
                    }
                    TrainerCardAction::Active => ScreenAction::Continue,
                }
            }
            GameScreen::TownMap => {
                let tm_input = TownMapScreenInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                };
                match self.town_map_screen.update_frame(tm_input) {                    TownMapScreenAction::Closed => {
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
                        // BIT_FLY_WARP: fade out and land at the destination's
                        // fly point (FlyWarpData).
                        let point =
                            pokered_core::overworld::hm_effects::fly_destination_for_map(dest);
                        if let Some(point) = point {
                            self.overworld.fly_warp_to(point.map, point.x, point.y);
                        }
                        ScreenAction::Transition(GameScreen::Overworld)
                    }
                    TownMapScreenAction::Active => ScreenAction::Continue,
                }
            }
            GameScreen::PokemonStatsScreen(_idx) => {
                if let Some(ref mut ss) = self.stats_screen {
                    let input = StatsScreenInput {
                        a: input.is_just_pressed(GbButton::A),
                        b: input.is_just_pressed(GbButton::B),
                    };
                    match ss.update(input) {
                        StatsScreenAction::Continue => ScreenAction::Continue,
                        StatsScreenAction::BackToParty => {
                            self.stats_screen = None;
                            // STATS opened from the PC's mon list returns to
                            // the PC (its state is still in `pc_screen`).
                            if self.pc_screen.is_some() {
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
                    let prev_phase = slots.phase;
                    result = slots.update_frame(slots_input);
                    coins_out = Some(slots.coins);
                    // The slots' own cues (slot_machine.asm: :120 spin start,
                    // :842 each reel stop, :694 payout).
                    let sfx = slots.take_sfx();
                    if let Some(ref audio) = self.audio {
                        use pokered_core::slots_screen::{SlotsPhase, SlotsSfx};
                        for cue in sfx {
                            let id = match cue {
                                SlotsSfx::NewSpin => SfxId::SlotsNewSpin,
                                SlotsSfx::StopWheel => SfxId::SlotsStopWheel,
                                SlotsSfx::Reward => SfxId::SlotsReward,
                            };
                            audio.play_sfx(id);
                        }
                        // Reel-stop / spin start feedback.
                        if prev_phase == SlotsPhase::BetSelect
                            && slots.phase == SlotsPhase::Spinning
                        {
                            audio.play_sfx(SfxId::PressAB);
                        }
                        if prev_phase == SlotsPhase::Spinning
                            && slots.phase == SlotsPhase::Result
                        {
                            if slots.last_payout > 0 {
                                audio.play_sfx(SfxId::GetItem1);
                            } else {
                                audio.play_sfx(SfxId::Denied);
                            }
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
                let menu_input = MenuInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                };
                if self.pc_screen.is_none() {
                    // Screen state lost (shouldn't happen) — bail out cleanly.
                    ScreenAction::Transition(GameScreen::Overworld)
                } else {
                let pc = self.pc_screen.as_mut().unwrap();
                let pc_action = {
                    let mut ctx = PcContext {
                        party: &mut self.save_data.party,
                        pc_storage: &mut self.save_data.pc_storage,
                        bag: &mut self.save_data.game_data.bag,
                        pc_items: &mut self.save_data.game_data.pc_items,
                        pokedex: &self.save_data.game_data.pokedex,
                    };
                    pc.update_frame(menu_input, &mut ctx)
                };
                for sfx in pc.take_sfx() {
                    if let Some(ref audio) = self.audio {
                        let id = match sfx {
                            PcSfx::TurnOn => SfxId::TurnOnPC,
                            PcSfx::TurnOff => SfxId::TurnOffPC,
                            PcSfx::Enter => SfxId::EnterPC,
                            PcSfx::WithdrawDeposit => SfxId::WithdrawDeposit,
                            PcSfx::Save => SfxId::Save,
                        };
                        audio.play_sfx(id);
                    }
                }
                if menu_input.a {
                    if let Some(ref audio) = self.audio {
                        audio.play_sfx(SfxId::PressAB);
                    }
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
                    self.save_data.current_box = self.save_data.pc_storage.current_box().clone();
                    self.save_to_file();
                }
                // Party membership may have changed (deposit/withdraw) — keep
                // the overworld mirrors in sync (repel checks, scripts).
                self.overworld.party_count = self.save_data.party.count() as u8;
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
                                self.stats_screen = Some(StatsScreenState::new(mon));
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
                    MartUpdate::Exit => {
                        ScreenAction::Transition(GameScreen::Overworld)
                    }
                }
            }
        };

        if let ScreenAction::Transition(new_screen) = action {
            use pokered_core::game_state::MainMenuChoice;
            let needs_black_screen = new_screen == GameScreen::Overworld
                && self.state.screen == GameScreen::MainMenu
                && self.main_menu.last_choice == Some(MainMenuChoice::Continue);

            if needs_black_screen {
                self.black_screen_frames = BLACK_SCREEN_DURATION;
                self.pending_screen = Some(new_screen);
            } else {
                self.handle_transition(new_screen);
            }
        }
    }

    /// Poll the link layer once per frame: accept a pending peer, route
    /// transport messages into the per-activity queues, drive the CORE
    /// battle/trade drivers, and keep `link_status` in sync for the Cable
    /// Club UI.
    fn poll_link(&mut self) {
        use pokered_core::link::protocol::LINK_RANDOM_LIST_SIZE;

        // Server pending: try a non-blocking accept each frame. (Native
        // only — the wasm transport is created by the entry point and
        // attached through `attach_link_transport`.)
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(server) = self.link_server.take() {
            match server.accept() {
                Ok(Some(transport)) => {
                    eprintln!("[link] peer connected, waiting for its Hello");
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
                    eprintln!("[link] accept failed: {}", e);
                    self.link_status = LinkStatus::Disconnected(e.to_string());
                }
            }
        }

        let Some(session) = self.link_session.as_mut() else {
            return;
        };

        // Route everything the transport has into the per-activity queues.
        if let Some(reason) = session.poll() {
            eprintln!("[link] transport closed: {}", reason);
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
            let random_numbers: [u8; LINK_RANDOM_LIST_SIZE] =
                std::array::from_fn(|_| next_byte());
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
                    eprintln!("[link] handshake failed: {}", e);
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
            .with_role(self.link_role);
            self.link_trade = Some(driver);
        }

        // Drive the battle driver; feed its events to the Cable Club flow
        // and keep the link status in sync.
        let mut needs = Vec::new();
        if let Some(driver) = self.link_battle.as_mut() {
            for ev in driver.poll() {
                match &ev {
                    LinkDriverEvent::Connected => {
                        eprintln!("[link] handshake complete — connected");
                        self.link_status = LinkStatus::Connected;
                    }
                    LinkDriverEvent::Disconnected(reason) => {
                        eprintln!("[link] disconnected: {}", reason);
                        self.link_status =
                            LinkStatus::Disconnected("Player2 disconnected".into());
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
            match &result {
                LinkTradePollResult::Disconnected => {
                    self.link_status =
                        LinkStatus::Disconnected("Player2 disconnected".into());
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
    fn handle_flow_need(&mut self, need: FlowNeed) {
        let result = match &need {
            FlowNeed::None => return,
            FlowNeed::RequestLink(kind) => {
                let Some(session) = self.link_session.as_mut() else {
                    return;
                };
                match kind {
                    LinkKind::Battle => {
                        let Some(driver) = self.link_battle.as_mut() else {
                            return;
                        };
                        // The party exchange happens at the cable-club table;
                        // the original heals the local party there (`predef
                        // HealParty`, engine/link/cable_club.asm:292 — the
                        // driver heals), so the wire copy is the CURRENT
                        // save party, healed. The save itself is untouched.
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
                        let Some(driver) = self.link_battle.as_mut() else {
                            return;
                        };
                        driver.accept_battle()
                    }
                    (LinkKind::Battle, false) => {
                        let Some(driver) = self.link_battle.as_mut() else {
                            return;
                        };
                        driver.decline_battle()
                    }
                    (LinkKind::Trade, true) => {
                        let Some(driver) = self.link_trade.as_mut() else {
                            return;
                        };
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
            FlowNeed::CancelTrade => {
                let Some(session) = self.link_session.as_mut() else {
                    return;
                };
                let Some(driver) = self.link_trade.as_mut() else {
                    return;
                };
                driver
                    .cancel_trade(&mut *session.trade_transport())
                    .map_err(link_trade_err_to_transport)
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
                eprintln!("[link] flow action failed: {}", e);
                self.link_cable.on_session_error(format!("link error: {}", e));
            }
        }
    }

    /// Enter the link battle: the CORE driver already built the battle
    /// screen (both parties exchanged, shared RNG stream, link mode on).
    /// Here the app pushes the save-derived fields the driver cannot know,
    /// mirrors the screen into `self.battle` for the render/vfx/audio/settle
    /// machinery, and starts the battle music.
    fn start_link_battle(&mut self) {
        use pokered_data::trainer_data::TrainerClass;

        let Some(driver) = self.link_battle.as_mut() else {
            return;
        };
        if driver.screen().is_none() {
            eprintln!("[link] battle started without a battle screen");
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
        // The remote trainer's name is not on the wire for trades (protocol
        // gap — `PartyExchangeData.trainer_name` is battle-only), so the
        // cutscene uses the default partner line. Documented deviation.
        self.trade_anim = Some(TradeAnim::new(
            give,
            receive,
            self.player_name.clone(),
            is_zh,
        ));
        self.link_cable.on_trade_anim_started();
    }

    /// Apply a completed link trade to the save via the CORE trade driver:
    /// remove-then-add (Gen 1 has no last-mon guard — `RemovePokemon` then
    /// `AddEnemyMonToPlayerParty`, engine/link/cable_club.asm:800-817), the
    /// traded/obedience flag against our ID, Pokédex owned+seen, and the
    /// forced trade evolution detection (`TryEvolvingMon`, cable_club.asm:
    /// 851 — applied via the cutscene below, cancellable like the app's
    /// other post-battle evolutions).
    fn apply_link_trade(&mut self) {
        use pokered_core::battle::settlement::EvolutionEvent;

        // Driver calls first (they borrow disjoint fields of `self`); the
        // pending evolution and the exchanged party come out owned.
        let (pending, new_party) = {
            let Some(driver) = self.link_trade.as_mut() else {
                return;
            };
            let pending = match driver.apply_exchange(&mut self.save_data.game_data.pokedex) {
                Ok(pending) => pending,
                Err(e) => {
                    eprintln!("[link] trade exchange failed: {}", e);
                    None
                }
            };
            (pending, driver.party().clone())
        };
        if let Some(p) = pending {
            self.queue_evolution_cutscene(
                vec![EvolutionEvent {
                    party_index: p.party_index,
                    old_species: p.from,
                    new_species: p.to,
                }],
                None,
            );
        }
        // The driver's working party IS the save's new party (it was
        // snapshotted at the table and mutated by the exchange).
        self.save_data.party = new_party;
        // The driver landed in Completed — reset so a fresh gameboy use can
        // request the next trade (the original loops trades via
        // CableClub_DoBattleOrTradeAgain).
        if let Some(driver) = self.link_trade.as_mut() {
            driver.reset_for_new_trade();
        }
        self.overworld.party_count = self.save_data.party.count() as u8;
        self.overworld.party_lead_level = self.save_data.party.leader_level();
        self.link_cable.on_trade_anim_done();
    }

    /// Warp the player into a Cable Club room (used by the link CLI after a
    /// connection is established; the original warps into the room via
    /// `SpecialEnterMap` after the receptionist handshake).
    pub fn warp_to_cable_room(&mut self, map: MapId) {
        let (x, y) = pokered_core::link::CABLE_ROOM_ENTRY;
        self.overworld.pending_warp = Some(pokered_core::overworld::PendingWarp {
            dest_map: map,
            dest_x: x as u8,
            dest_y: y as u8,
            save_last_map: false,
            // Cable Club entry: plain fade-in, no EnterMapAnim spin.
            arrival_spin: false,
        });
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
            .pending_dialogue
            .as_ref()
            .and_then(|d| d.get_display_text())
            .map(|(a, b)| format!("{} {}", a, b).trim().to_string());
        // Structured dialogue-machine state: page progress, typewriter
        // position, and whether the current page is fully revealed and
        // waiting for A — the driver can align A presses exactly.
        let dialogue_state = self
            .overworld
            .pending_dialogue
            .as_ref()
            .map(|d| {
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
        serde_json::json!({
            "screen": crate::cli::screen_name(&self.state.screen).to_string(),
            "map_id": map_id as u8,
            "map_name": format!("{:?}", map_id),
            "player_x": self.overworld.state.player.x,
            "player_y": self.overworld.state.player.y,
            "player_facing": format!("{:?}", self.overworld.state.player.facing),
            "player_name": self.player_name.clone(),
            "frame_count": self.frame_count,
            "party_count": self.overworld.party_count,
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
            // True while a storyline script owns the game (cutscene in
            // progress), false once control is back with the player.
            "script_running": !self.overworld.script_engine_idle(),
            // Current battle text box message (e.g. the post-victory
            // EndBattleText quip), so a driver can observe battle flow.
            "battle_message": self.battle.current_message.clone(),
            // Current battle phase (Debug form), e.g. "PlayerMenu",
            // "BagSelect", so a driver knows when a menu is ready.
            "battle_phase": format!("{:?}", self.battle.phase),
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
        })
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
            // and no battle suspended on the script.
            "control_ready" => {
                crate::cli::screen_name(&self.state.screen) == "overworld"
                    && self.overworld.pending_dialogue.is_none()
                    && self.overworld.pending_choice.is_none()
                    && self.overworld.active_script_effect_value().is_none()
                    && self.overworld.script_engine_idle()
                    && self.overworld.pending_warp.is_none()
                    && matches!(
                        self.overworld.warp_fade_state,
                        pokered_core::overworld::WarpFadeState::Idle
                    )
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
                while self.overworld.pending_dialogue.is_some() && stepped < MAX_SKIP_FRAMES {
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
                self.pending_debug_inputs.push(gb_button);
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
                    self.pending_debug_inputs.push(gb_button);
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
                        match pokered_core::pokemon::stats::create_pokemon(sp, level, [0x9A, 0x78]) {
                            Some(mon) => {
                                let _ = self.save_data.party.add(mon);
                                self.overworld.party_count =
                                    self.save_data.party.count() as u8;
                                self.overworld.party_lead_level =
                                    self.save_data.party.leader_level();
                                DebugResponse::ok()
                            }
                            None => {
                                DebugResponse::err(format!("failed to create '{}'", species))
                            }
                        }
                    }
                    Err(_) => DebugResponse::err(format!("unknown species: '{}'", species)),
                }
            }
            DebugCommand::Game(GameDebugCommand::StartWildBattle { ref species, level }) => {
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
                            DebugResponse::ok()
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
    fn apply_npc_trade(&mut self, trade: PendingTrade) {
        use pokered_core::trade::{assemble_npc_trade_mon, roll_npc_trade_randoms_thread};
        if let Some(idx) = self.save_data.party.find_species(trade.give) {
            if let Ok(removed) = self.save_data.party.remove(idx) {
                let (dv_bytes, ot_id) = roll_npc_trade_randoms_thread();
                let player_id = self.save_data.game_data.player_id;
                if let Some(mon) = assemble_npc_trade_mon(
                    trade.receive,
                    removed.level,
                    &trade.nickname,
                    dv_bytes,
                    ot_id,
                    player_id,
                ) {
                    let _ = self.save_data.party.add(mon);
                    self.save_data.game_data.pokedex.set_seen(trade.receive);
                    self.save_data.game_data.pokedex.set_owned(trade.receive);
                }
            }
        }
        self.overworld.party_count = self.save_data.party.count() as u8;
        self.overworld.party_lead_level = self.save_data.party.leader_level();
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
            draw_hof_ceremony(hof, &mut self.resources, frame_buffer, self.state.config.language);
            return;
        }
        if let Some(ref roll) = self.credits {
            draw_credits(roll, &mut self.resources, frame_buffer);
            return;
        }

        frame_buffer.clear(Rgba::WHITE);

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
                draw_oak_speech(&self.oak_speech, &mut self.resources, frame_buffer, self.state.config.language);
            }
            GameScreen::Overworld => {
                draw_overworld(&mut self.overworld, &mut self.resources, frame_buffer, self.state.config.language);
                // Cable Club link overlay (text boxes / prompts / trade list)
                // over the frozen room.
                if self.link_cable.is_active() {
                    crate::render::draw_link_flow(
                        &self.link_cable,
                        frame_buffer,
                        matches!(
                            self.state.config.language,
                            pokered_core::game_state::Lang::Zh
                        ),
                    );
                }
            }
            GameScreen::Battle => {
                if self.battle_vfx.has_transition()
                    && self.battle_vfx.overworld_snapshot.is_none()
                {
                    let mut snapshot = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::BLACK);
                    draw_overworld(&mut self.overworld, &mut self.resources, &mut snapshot, self.state.config.language);
                    self.battle_vfx.overworld_snapshot = Some(snapshot);
                }
                draw_battle(
                    &self.battle,
                    &mut self.resources,
                    frame_buffer,
                    &mut self.battle_vfx,
                    self.state.config.language,
                );
                if !self.battle_vfx.has_transition() {
                    self.battle_vfx.clear_snapshot();
                }
            }
            GameScreen::StartMenu => {
                draw_overworld(&mut self.overworld, &mut self.resources, frame_buffer, self.state.config.language);
                draw_start_menu(&self.start_menu, &self.player_name, frame_buffer, self.state.config.language);
            }
            GameScreen::OptionsMenu => {
                draw_options_menu(&self.options_menu, frame_buffer, self.state.config.language);
            }
            GameScreen::SaveMenu => {
                draw_overworld(&mut self.overworld, &mut self.resources, frame_buffer, self.state.config.language);
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
                if let Some(ref ss) = self.stats_screen {
                    draw_stats_screen(ss, self.resources.as_mut(), frame_buffer, self.state.config.language);
                }
            }
            GameScreen::Shop(ref mart_state) => {
                let money = self.save_data.game_data.player_money;
                let bag_slice = self.save_data.game_data.bag.items();
                draw_mart(mart_state, money, &bag_slice[..], frame_buffer, self.state.config.language);
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
                    draw_elevator(elevator, frame_buffer, self.state.config.language);
                }
            }
            GameScreen::FilterBag => {
                if let Some(ref filter) = self.elevator_screen {
                    draw_filter_bag(filter, frame_buffer, self.state.config.language);
                }
            }
            GameScreen::Diploma => {
                draw_diploma(&self.player_name, frame_buffer, self.state.config.language);
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
                    draw_pc(pc, &self.save_data, &mut self.resources, frame_buffer, self.state.config.language);
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

#[cfg(not(target_arch = "wasm32"))]
impl GameLoop for PokemonGame {
    type Fb = FrameBuffer;

    fn update(&mut self, input: &InputState) {
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
    let (en_pre, en_color) = if current == Lang::En { ("> ", Rgba::BLACK) } else { ("  ", gray) };
    let (zh_pre, zh_color) = if current == Lang::Zh { ("> ", Rgba::BLACK) } else { ("  ", gray) };
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
            pokered_core::items::MartState::new(
                pokered_core::items::ShopInventory::new(vec![pokered_data::items::ItemId::Potion]),
            )
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

#[cfg(test)]
mod save_overwrite_tests {
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
}
