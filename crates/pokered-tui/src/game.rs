use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use pokered_audio::music_data::MusicId;
use pokered_audio::sfx_data::SfxId;
use pokered_core::battle::{BattleInput, BattleScreen};
use pokered_core::bag_screen::{BagScreenAction, BagScreenInput, BagScreenState};
use pokered_core::data::maps::MapId;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::{GameScreen, GameState, SaveFileSummary, ScreenAction};
use pokered_core::gamefreak_splash::{GameFreakSplashState, SplashInput};
use pokered_core::intro_scene::IntroSceneState;
use pokered_core::items::{MartUpdate, PlayerData, ShopInventory, SoundId};
use pokered_core::items::bag_use::{self, ItemApplyOutcome};
use pokered_core::intro_scene::IntroSfxEvent;
use pokered_core::main_menu::{MainMenuState, MenuInput};
use pokered_core::pc_screen::{PcContext, PcEntry, PcOpenContext, PcScreen, PcScreenAction, PcSfx};
use pokered_core::naming_screen::NamingInput;
use pokered_core::oak_speech::{OakSpeechInput, OakSpeechPhase, OakSpeechResult, OakSpeechState};
use pokered_core::options_menu::{BattleAnimation, GameOptions, OptionsInput, OptionsMenuResult, OptionsMenuState};
use pokered_core::data::impl_traits::PokemonRedData;
use pokered_core::elevator_screen::{ElevatorAction, ElevatorInput, ElevatorScreen};
use pokered_core::overworld::{
    BedroomDialogue, OverworldAudioRequest, OverworldGameDataRequest, OverworldInput,
    OverworldScreen, OverworldSfxEvent,
};
use pokered_core::party_screen::{PartyScreenAction, PartyScreenInput, PartyScreenState};
use pokered_core::stats_screen::{StatsScreenAction, StatsScreenInput, StatsScreenState};
use pokered_core::save::sram_export::export_sram;

#[cfg(not(target_arch = "wasm32"))]
use pokered_core::save::sram_import::import_sram;
use pokered_core::save::SaveData;
use pokered_core::save_menu::{
    SaveMenuResult, SaveMenuState, SavePhase, SaveScreenInfo, SaveSfxEvent, YesNoInput,
};
use pokered_core::slots_screen::{SlotsAction, SlotsInput, SlotsScreen};
use pokered_core::start_menu::{StartMenuAction, StartMenuInput, StartMenuState};
use pokered_core::town_map_screen::{TownMapScreenAction, TownMapScreenInput, TownMapScreenState};
use pokered_core::pokedex_screen::{
    PokedexScreenAction, PokedexScreenInput, PokedexScreenState,
};
use pokered_core::trainer_card_screen::{
    TrainerCardAction, TrainerCardInput, TrainerCardScreenState,
};
use pokered_core::title_screen::{TitlePhase, TitleScreenState};
use dotzuki_engine::render_config::RenderConfig;
use dotzuki_tui::InputState;
use pokered_renderer::input::GbButton;
use pokered_renderer::resource::ResourceManager;

use pokered_renderer::resource::AssetRoot;

use pokered_renderer::{FrameBuffer, Rgba};

use crate::audio::{play_species_cry, AudioOutput};
use crate::render::{
    draw_bag, draw_battle, draw_credits, draw_diploma, draw_elevator, draw_evolution, draw_filter_bag, draw_gamefreak_splash, draw_hof_ceremony, draw_intro_scene, draw_main_menu, draw_mart, draw_oak_speech, draw_options_menu,
    draw_overworld, draw_party_screen, draw_pc, draw_pokedex_screen, draw_save_menu, draw_slots, draw_start_menu, draw_stats_screen,
    draw_title_screen, draw_town_map, draw_trainer_card,
    BattleVisualEffects,
};

const SAVE_FILE_NAME: &str = "pokered.sav";
const SCRIPT_FLAGS_FILE_NAME: &str = "pokered.script_flags.json";

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

#[cfg(not(target_arch = "wasm32"))]
fn save_file_path() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(SAVE_FILE_NAME)))
        .unwrap_or_else(|| std::path::PathBuf::from(SAVE_FILE_NAME))
}

#[cfg(not(target_arch = "wasm32"))]
fn script_flags_file_path() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(SCRIPT_FLAGS_FILE_NAME)))
        .unwrap_or_else(|| std::path::PathBuf::from(SCRIPT_FLAGS_FILE_NAME))
}

fn save_summary_from_data(save: &SaveData) -> SaveFileSummary {
    SaveFileSummary {
        player_name: save.player_name.clone(),
        badges: save.game_data.obtained_badges,
        pokedex_owned: save.game_data.pokedex.owned_count() as u8,
        play_time_hours: save.game_data.play_time.hours as u16,
        play_time_minutes: save.game_data.play_time.minutes,
        play_time_seconds: save.game_data.play_time.seconds,
    }
}

/// Recorded Hall of Fame teams for the #MON LEAGUE PC viewer
/// (`LoadHallOfFameTeams` + `wHoFTeamNo`, engine/menus/league_pc.asm:16-35):
/// oldest first, numbered by their all-time index.
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
    /// Overworld ITEM bag (Start menu → ITEM). The screen state lives in
    /// `pokered_core::bag_screen`; the bag itself is owned by the save data
    /// and mirrored into the screen on entry (app mirror).
    pub bag_screen: BagScreenState,
    /// Town Map viewer / FLY destination picker (bag TOWN MAP item, party-menu
    /// FLY). Pure logic in `pokered_core::town_map_screen`; rendering mirrors
    /// the native app's render/town_map.rs.
    pub town_map_screen: TownMapScreenState,
    /// Bag USE on a party-targeted item: set while the party screen is open in
    /// item-use mode, cleared when the item resolves (app mirror).
    pending_bag_item: Option<pokered_data::items::ItemId>,
    /// The SOFTBOILED user (party index): set when the party menu chose the
    /// field move for it; the party screen reopens in target-pick mode
    /// (Gen-1 `.softboiled` → `GoBackToPartyMenu`), cleared when the heal is
    /// applied or the pick is cancelled.
    pending_softboiled_user: Option<usize>,
    /// A move the party screen must offer to forget: a level-up/evolution
    /// move that could not be learned because the moveset is full (Gen-1
    /// `LearnMove`'s forget-a-move prompt, learn_move.asm:98-184).
    pending_evolve_move_replace: Option<(usize, pokered_data::moves::MoveId)>,
    /// FLY: the town map opens in destination-picker mode (pending_fly_map).
    pending_fly_map: bool,
    pub pokedex_screen: PokedexScreenState,
    pub trainer_card_screen: TrainerCardScreenState,
    pub stats_screen: Option<StatsScreenState>,
    /// PC storage screen (Bill's PC / item PC / Oak's rating), opened by
    /// `game.openPC()` / `game.openItemPC()` via `pending_pc`.
    pub pc_screen: Option<PcScreen>,
    /// Active evolution cutscene (`pokered_core::evolution_screen`,
    /// engine/movie/evolution.asm); while Some, it takes over update + render
    /// from the overworld. Queued by the post-battle writeback (level-ups).
    pub evolution_anim: Option<pokered_core::evolution_screen::EvolutionScreenState>,
    /// Hall of Fame roll-call takeover (engine/movie/hall_of_fame.asm),
    /// started when the HallOfFame scene calls `game.enterHallOfFame()`.
    pub hof_ceremony: Option<pokered_core::hof_ceremony::HofCeremonyState>,
    /// End-credits takeover (engine/movie/credits.asm); on completion the
    /// game saves and resets to the title screen.
    pub credits: Option<pokered_core::credits::CreditsState>,
    /// Game Corner slot machine (`GameScreen::Slots`), opened by the scene
    /// script's `game.openSlotsMachine()` via `pending_slots`; owns the live
    /// coin balance while playing (app mirror).
    pub slots_screen: Option<SlotsScreen>,
    /// Elevator floor menu / filtered-bag menu (`GameScreen::Elevator` /
    /// `GameScreen::FilterBag`), opened via `pending_elevator` /
    /// `pending_filter_bag`; both share the `ElevatorScreen` state machine
    /// (app mirror).
    pub elevator_screen: Option<ElevatorScreen>,
    pub save_data: SaveData,
    pub player_name: String,
    pub rival_name: String,
    pub frame_count: u64,
    pub exit_requested: bool,
    pub resources: Option<ResourceManager>,
    prev_title_phase: Option<TitlePhase>,
    prev_oak_phase_tag: u8,
    /// Frames A+B+Start+Select has been held; at `SOFT_RESET_HOLD_FRAMES` the
    /// game soft-resets to the title screen (engine/joypad.asm `TrySoftReset`
    /// → home/init.asm `SoftReset`).
    soft_reset_frames: u8,
    /// Save file path used by the soft reset to reload from disk (unsaved
    /// progress is lost, as on hardware); mirrors the native app's field.
    save_path: Option<PathBuf>,
    pub black_screen_frames: u32,
    pub pending_screen: Option<GameScreen>,
    pub scripts_dir: Option<PathBuf>,
    pub audio: Option<AudioOutput>,
    /// SFX_FAINT_THUD has been scheduled (SFX_FAINT_FALL played) for an enemy
    /// faint in a trainer battle; it fires once the fall SFX finishes
    /// (engine/battle/core.asm:782-791).
    faint_thud_pending: bool,
    /// Last battle message text, for the message-based SFX block
    /// (SuperEffective / FaintFall / etc. fire on message change).
    battle_prev_message: Option<String>,
}

impl PokemonGame {
    /// Creates a new game with default settings (no save file, no scripts dir).
    /// This is the primary constructor used by both web and native builds.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(version: GameVersion) -> Self {
        Self::new_with_options(version, None, None, None)
    }

    /// Creates a new game with optional save file, snapshot, and scripts directory.
    /// Only available for native builds (wasm doesn't support file system operations).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_with_options(
        version: GameVersion,
        save_path: Option<PathBuf>,
        snapshot_path: Option<PathBuf>,
        scripts_dir: Option<PathBuf>,
    ) -> Self {
        let (save_data, save_summary) = if let Some(ref path) = snapshot_path {
            Self::load_snapshot_from_path(path)
        } else if let Some(ref path) = save_path {
            Self::load_sram_from_path(path)
        } else {
            Self::try_load_default_save()
        };
        let mut state = GameState {
            screen: GameScreen::GameFreakSplash,
            config: pokered_core::game_state::GameConfig::new(version),
            save_summary: save_summary.clone(),
        };
        apply_saved_options(&mut state.config, &save_data.game_data.options);
        let title_screen = TitleScreenState::new(version);
        let main_menu = MainMenuState::new(save_summary);
        let oak_speech = OakSpeechState::new();
        let overworld = OverworldScreen::new(MapId::PalletTown, scripts_dir.clone(), PokemonRedData);
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

        let audio = match AudioOutput::new() {
            Some(ao) => {
                eprintln!("Audio output initialized (cpal 44100 Hz stereo)");
                Some(ao)
            }
            None => {
                eprintln!("Warning: Could not initialize audio output.");
                None
            }
        };

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
            pending_bag_item: None,
            pending_softboiled_user: None,
            pending_evolve_move_replace: None,
            pending_fly_map: false,
            pokedex_screen: PokedexScreenState::new(
                pokered_core::pokemon::pokedex::Pokedex::new(),
                version,
            ),
            trainer_card_screen: TrainerCardScreenState::new(),
            stats_screen: None,
            pc_screen: None,
            evolution_anim: None,
            hof_ceremony: None,
            credits: None,
            slots_screen: None,
            elevator_screen: None,
            save_data,
            player_name: "RED".to_string(),
            rival_name: "BLUE".to_string(),
            frame_count: 0,
            exit_requested: false,
            resources,
            prev_title_phase: None,
            prev_oak_phase_tag: 0,
            soft_reset_frames: 0,
            save_path,
            black_screen_frames: 0,
            pending_screen: None,
            scripts_dir,
            audio,
            faint_thud_pending: false,
            battle_prev_message: None,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new(version: GameVersion) -> Self {
        let save_data = SaveData::new();
        let save_summary = None;
        let state = GameState {
            screen: GameScreen::GameFreakSplash,
            config: pokered_core::game_state::GameConfig::new(version),
            save_summary: save_summary.clone(),
        };
        let title_screen = TitleScreenState::new(version);
        let main_menu = MainMenuState::new(save_summary);
        let oak_speech = OakSpeechState::new();
        let overworld = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
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
            pending_bag_item: None,
            pending_softboiled_user: None,
            pending_evolve_move_replace: None,
            pending_fly_map: false,
            pokedex_screen: PokedexScreenState::new(
                pokered_core::pokemon::pokedex::Pokedex::new(),
            ),
            trainer_card_screen: TrainerCardScreenState::new(),
            stats_screen: None,
            pc_screen: None,
            evolution_anim: None,
            hof_ceremony: None,
            credits: None,
            slots_screen: None,
            elevator_screen: None,
            save_data,
            player_name: "RED".to_string(),
            rival_name: "BLUE".to_string(),
            frame_count: 0,
            exit_requested: false,
            resources,
            prev_title_phase: None,
            prev_oak_phase_tag: 0,
            soft_reset_frames: 0,
            save_path: None,
            black_screen_frames: 0,
            pending_screen: None,
            scripts_dir: None,
            audio,
            faint_thud_pending: false,
            battle_prev_message: None,
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

        // The event-flag bitset serializes directly into the original
        // 320-byte SRAM region (wEventFlags, NUM_EVENTS = $A00 bits).
        save.game_data.event_flags = self.overworld.unified_flags().as_bytes().to_vec();

        // SRAM writebacks for runtime-mutated overworld state (mirrors the
        // app): the toggleable object bits and the itemfinder hidden-item
        // bitfield.
        save.game_data.toggleable_object_flags = *self.overworld.toggleable_object_flags();
        save.game_data.obtained_hidden_items = *self.overworld.hidden_item_flags();
        save.game_data.obtained_hidden_coins = *self.overworld.hidden_coin_flags();

        save
    }

    /// `game.enterHallOfFame()` (drained from `overworld.pending_hof_ceremony`):
    /// record the party into the Hall of Fame (engine/movie/hall_of_fame.asm
    /// HoFRecordMonInfo + SaveHallOfFameTeams), reset `wLastBlackoutMap` to
    /// PALLET_TOWN (scripts/HallOfFame.asm:48-49), start the roll-call.
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
    /// The original saves on the HallOfFame map and CONTINUE special-warps
    /// out (main_menu.asm:114-125); we reposition the save to Pallet Town's
    /// fly point before saving — same player-visible result.
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
        let path = save_file_path();
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
        // Save file persistence not supported on wasm32
    }

    pub fn handle_transition(&mut self, screen: GameScreen) {
        // Set in the Battle→Overworld settle below when a caught species was
        // newly added to the Pokédex — the post-capture entry then opens
        // instead of the overworld (engine/items/item_effects.asm:521-546).
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
                // NOTE (deviation from pokered-app, which omits Bag / PartyScreen /
                // TownMap here): those screens return to the overworld *in place*
                // after field-item use / medicine / TM / FLY. Rebuilding the
                // overworld from the (save-time) position would teleport the player
                // and drop the pending result dialogue, so they must NOT rebuild.
                match self.main_menu.last_choice {
                    Some(MainMenuChoice::Continue)
                        if self.state.screen != GameScreen::Overworld
                            && self.state.screen != GameScreen::StartMenu
                            && self.state.screen != GameScreen::OptionsMenu
                            && self.state.screen != GameScreen::SaveMenu
                            && self.state.screen != GameScreen::Bag
                            && self.state.screen != GameScreen::PartyScreen
                            && self.state.screen != GameScreen::TownMap
                            && self.state.screen != GameScreen::Pokedex
                            && self.state.screen != GameScreen::TrainerCard
                            && self.state.screen != GameScreen::PC
                            && self.state.screen != GameScreen::Battle =>
                    {
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
                        let mut overworld = OverworldScreen::new(map_id, self.scripts_dir.clone(), PokemonRedData);
                        overworld.state.player.x = pos.x as u16;
                        overworld.state.player.y = pos.y as u16;
                        overworld.state.player.facing =
                            match self.save_data.game_data.player_direction {
                                4 => pokered_core::overworld::Direction::Up,
                                8 => pokered_core::overworld::Direction::Left,
                                12 => pokered_core::overworld::Direction::Right,
                                _ => pokered_core::overworld::Direction::Down,
                            };
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
                        overworld.run_on_load();
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
                        if self.state.screen != GameScreen::Overworld
                            && self.state.screen != GameScreen::StartMenu
                            && self.state.screen != GameScreen::OptionsMenu
                            && self.state.screen != GameScreen::SaveMenu
                            && self.state.screen != GameScreen::Bag
                            && self.state.screen != GameScreen::PartyScreen
                            && self.state.screen != GameScreen::TownMap
                            && self.state.screen != GameScreen::Pokedex
                            && self.state.screen != GameScreen::TrainerCard
                            && self.state.screen != GameScreen::PC
                            && self.state.screen != GameScreen::Battle =>
                    {
                        // InitOptions (engine/menus/main_menu.asm): a NEW GAME
                        // resets wOptions to defaults (medium text, animation
                        // on, shift style), discarding any save-file options.
                        let defaults =
                            pokered_core::game_state::GameConfig::new(self.state.config.version);
                        self.state.config.text_speed = defaults.text_speed;
                        self.state.config.battle_animation = defaults.battle_animation;
                        self.state.config.battle_style = defaults.battle_style;
                        let mut overworld =
                            OverworldScreen::new(NEW_GAME_WARP.map_id, self.scripts_dir.clone(), PokemonRedData);
                        overworld.state.player.x = NEW_GAME_WARP.coords.x as u16;
                        overworld.state.player.y = NEW_GAME_WARP.coords.y as u16;
                        overworld.player_name = self.player_name.clone();
                        overworld.rival_name = self.rival_name.clone();
                        self.overworld = overworld;
                        if let Some(ref audio) = self.audio {
                            audio.play_music(MusicId::PALLET_TOWN);
                        }
                    }
                    _ => {
                        // Returning from a BATTLE → fold the results into the save
                        // (money / party writeback / catch / Pokédex / bag / blackout),
                        // shared verbatim with the native app. Other sub-screen returns
                        // keep the existing overworld intact.
                        if self.state.screen == GameScreen::Battle {
                            // Was the caught species new to the Pokédex? Checked
                            // BEFORE the settle flips the owned bit.
                            post_catch_species = self
                                .battle
                                .captured_mon
                                .as_ref()
                                .map(|c| c.species)
                                .filter(|sp| !self.save_data.game_data.pokedex.is_owned(*sp));
                            let writeback = pokered_core::battle::settlement::settle_battle_into_save(
                                &mut self.battle,
                                &mut self.save_data,
                                &mut self.overworld,
                            );
                            // EvolutionAfterBattle: detected level-up
                            // evolutions play as the cutscene in the
                            // overworld before anything else resumes.
                            if !writeback.pending_evolutions.is_empty() {
                                self.queue_evolution_cutscene(writeback.pending_evolutions);
                            }
                            // Safari: fold the balls thrown this battle back
                            // into the overworld game (its zero-ball
                            // game-over / eject keys off this), app mirror.
                            if self.battle.is_safari {
                                let remaining =
                                    self.battle.safari.as_ref().map_or(0, |s| s.balls);
                                while self.overworld.safari_balls_remaining() > remaining {
                                    self.overworld.use_safari_ball();
                                }
                            }
                            if let Some(outcome) = writeback.outcome {
                                self.overworld.resume_script_after_battle(outcome);
                            }
                            if let Some(ref audio) = self.audio {
                                // Battle end clears the low-health alarm
                                // (engine/battle/end_of_battle.asm:48).
                                audio.set_low_health_alarm(false);
                            }
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
                self.save_menu = SaveMenuState::new(
                    SaveScreenInfo {
                        player_name: self.player_name.clone(),
                        num_badges: self.save_data.game_data.badge_count(),
                        pokedex_owned: self.save_data.game_data.pokedex.owned_count() as u16,
                        play_time_hours: self.save_data.game_data.play_time.hours as u16,
                        play_time_minutes: self.save_data.game_data.play_time.minutes,
                    },
                    has_previous,
                    false,
                );
            }
            GameScreen::PartyScreen => {
                // Opened from the bag to apply an item → item-use mode (A on a
                // Pokémon applies the pending item, no STATS/SWITCH menu).
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
            GameScreen::LanguageSelect => {},
            GameScreen::CopyrightSplash => {
                self.title_screen.reset();
            }
            GameScreen::GameFreakSplash => {
                self.gamefreak_splash.reset();
            }
            GameScreen::Shop(_) => {}
            GameScreen::PC => {
                // PC system wiring belongs to the PC work; placeholder arm.
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
        }
        if let Some(species) = post_catch_species {
            // "New DEX data will be added…": show the new species' entry before
            // returning to the overworld (item_effects.asm:541-546).
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

    fn game_timer_active(&self) -> bool {
        self.state.screen == GameScreen::Overworld && self.black_screen_frames == 0
    }

    /// A+B+Start+Select held for 16 frames — the original's soft reset
    /// (engine/joypad.asm `TrySoftReset` → home/init.asm `SoftReset`): stop
    /// all sounds, reload the save (unsaved progress is lost, as on
    /// hardware), and return to the title screen.
    ///
    /// Deviation from the native app: the app always reloads from disk
    /// (`self.save_path` or the default save file). The TUI reloads from disk
    /// when a save path is known (explicit `--save` path or an existing
    /// default `pokered.sav`); otherwise it keeps the in-memory save, which
    /// is the best mirror available to a session started from a snapshot or
    /// a fresh game (no save file exists on disk to reload).
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
                if let Ok(data) = std::fs::read(&path) {
                    if let Ok(save) = import_sram(&data) {
                        pokered_core::log_save!("soft reset: save reloaded from {:?}", path);
                        self.save_data = save;
                    }
                }
            }
        }
        self.handle_transition(GameScreen::TitleScreen);
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
        self.battle_prev_message = None;
        self.faint_thud_pending = false;
        // Pre-battle setup mirroring the native app — REQUIRED so the post-battle
        // writeback (settle_battle_into_save) doesn't wipe the save bag: carry the map
        // id + bag into the battle, and register the wild species as seen.
        self.battle.map_id = self.overworld.state.current_map as u8;
        self.battle.player_bag = self.save_data.game_data.bag.clone();
        // Badge stat boosts + traded-mon obedience context (wObtainedBadges / wPlayerID).
        self.battle.player_badges = self.save_data.game_data.obtained_badges;
        self.battle.player_id = self.save_data.game_data.player_id;
        // Pokémon Tower ghost handling, mirroring the native app's start_wild_battle:
        // no scope → unidentified uncatchable GHOST; the 6F Marowak WITH the scope
        // gets the SILPH SCOPE unveil intro.
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
        self.battle.ghost_marowak_reveal = in_pokemon_tower
            && has_silph_scope
            && species == pokered_data::species::Species::Marowak;
        if !is_ghost {
            self.save_data.game_data.pokedex.set_seen(species);
        }

        // Safari Zone during an active Safari Game → the BALL/BAIT/ROCK/RUN
        // Safari mode (no attacking; the ball economy + bait/rock catch-flee
        // mechanics take over), mirroring the native app.
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

        self.battle.player_money = self.save_data.game_data.player_money;
        self.battle_vfx = BattleVisualEffects::default();

        if let Some(ref audio) = self.audio {
            audio.play_music(MusicId::WILD_BATTLE);
        }
    }

    /// ReadTrainer's special-move pass — same rules as the app's
    /// `apply_trainer_special_moves` (read_trainer_party.asm
    /// `.AddLoneMove`/`.AddTeamMove`/`.ChampionRival`).
    fn apply_trainer_special_moves(
        class: pokered_data::trainer_data::TrainerClass,
        party: &mut [pokered_core::battle::state::Pokemon],
    ) {
        use pokered_core::battle::special_moves::apply_trainer_special_moves as apply;
        apply(class, party);
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
        // Pre-battle setup mirroring the native app (REQUIRED — see start_wild_battle):
        // carry the map id + bag into the battle and register the enemy team as seen.
        self.battle.map_id = self.overworld.state.current_map as u8;
        self.battle.player_bag = self.save_data.game_data.bag.clone();
        // Badge stat boosts + traded-mon obedience context (wObtainedBadges / wPlayerID).
        self.battle.player_badges = self.save_data.game_data.obtained_badges;
        self.battle.player_id = self.save_data.game_data.player_id;
        for mon in &enemy_party {
            self.save_data.game_data.pokedex.set_seen(mon.species);
        }

        self.battle.player_money = self.save_data.game_data.player_money;
        self.battle_vfx = BattleVisualEffects::default();

        if let Some(ref audio) = self.audio {
            audio.play_music(MusicId::TRAINER_BATTLE);
        }
    }

    pub fn update(&mut self, input: &InputState<GbButton>) {
        use pokered_core::game_state::Lang;
        self.frame_count += 1;

        // Soft reset: hold A+B+Start+Select for 16 consecutive frames
        // (engine/joypad.asm `_Joypad`: hJoyInput == PAD_BUTTONS → TrySoftReset
        // decrements hSoftReset from 16 while the combo stays held). Mirrors
        // pokered-app; the TUI reloads from disk when a save path is known,
        // otherwise keeps the in-memory save (see `soft_reset`).
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

        // Evolution cutscene (engine/movie/evolution.asm): takes over the
        // frame while active; each evolution's mutation is applied only when
        // its morph resolves (a B-cancel applies nothing).
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
                            EvolutionSfx::Cry(species) => crate::audio::play_species_cry(audio, species),
                        }
                    }
                }
                done
            };
            let mut outcomes = Vec::new();
            while let Some(outcome) = self.evolution_anim.as_mut().unwrap().take_outcome() {
                outcomes.push(outcome);
            }
            for outcome in outcomes {
                self.apply_evolution_outcome(&outcome);
            }
            if done {
                self.evolution_anim = None;
                // PlayDefaultMusic (evos_moves.asm:257-259).
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
        // takeover after the Champion, started by `game.enterHallOfFame()`.
        if self.hof_ceremony.is_some() {
            let done = {
                let hof = self.hof_ceremony.as_mut().unwrap();
                let done = hof.update_frame();
                for sfx in hof.take_sfx() {
                    if let Some(ref audio) = self.audio {
                        let pokered_core::hof_ceremony::HofSfx::Cry(species) = sfx;
                        crate::audio::play_species_cry(audio, species);
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
                // HallOfFamePC (credits.asm:24-33).
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
                // PlayShootingStar (engine/movie/intro.asm:305-341). TUI's
                // boot flow skips LanguageSelect (like its CopyrightSplash
                // arm), so map the splash's exit there.
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
                if action == ScreenAction::Transition(GameScreen::LanguageSelect) {
                    ScreenAction::Transition(GameScreen::IntroScene)
                } else {
                    action
                }
            }
            GameScreen::LanguageSelect => {
                if input.is_just_pressed(GbButton::Up) || input.is_just_pressed(GbButton::Down) {
                    self.state.config.language = match self.state.config.language {
                        Lang::En => Lang::Zh,
                        Lang::Zh => Lang::En,
                    };
                }
                if input.is_just_pressed(GbButton::A) || input.is_just_pressed(GbButton::Start) {
                    ScreenAction::Transition(GameScreen::IntroScene)
                } else {
                    ScreenAction::Continue
                }
            }
            GameScreen::CopyrightSplash => {
                let any_pressed = input.any_just_pressed();
                let action = self.title_screen.update_frame(any_pressed);
                if self.title_screen.phase == TitlePhase::Init {
                    ScreenAction::Transition(GameScreen::IntroScene)
                } else {
                    action
                }
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
                    self.oak_speech.update_naming_frame(naming_input, false)
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

                if input.is_just_pressed(GbButton::A)
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
                    OakSpeechResult::Finished => ScreenAction::Transition(GameScreen::Overworld),
                    OakSpeechResult::Active => ScreenAction::Continue,
                }
            }
            GameScreen::Overworld => {
                if self.overworld.is_naming_screen_active() {
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
                    self.overworld.update_naming_input(naming_input, false);
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
                    // Seed synchronous script-query state before update_frame.
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

                    // Apply script-requested bag/money mutations.
                    // Trade outcomes resume the suspended script AFTER the
                    // drain (the drain borrows self.overworld).
                    let mut trade_results: Vec<bool> = Vec::new();
                    for req in self.overworld.game_data_requests.drain(..) {
                        match req {
                            OverworldGameDataRequest::GiveItem { item, quantity } => {
                                if let Some(id) = pokered_data::items::ItemId::from_const_name(&item)
                                {
                                    let _ = self.save_data.game_data.bag.add_item(id, quantity);
                                }
                            }
                            OverworldGameDataRequest::TakeItem { item, quantity } => {
                                if let Some(id) = pokered_data::items::ItemId::from_const_name(&item)
                                {
                                    let _ = self.save_data.game_data.bag.remove_item(id, quantity);
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
                            // SetLastBlackoutMap (engine/events/set_blackout_map.asm).
                            OverworldGameDataRequest::SetBlackoutMap { map } => {
                                self.save_data.game_data.last_blackout_map = map as u8;
                            }
                            OverworldGameDataRequest::TradePokemon {
                                offered,
                                received,
                                nickname,
                            } => {
                                use pokered_core::trade::{
                                    assemble_npc_trade_mon, roll_npc_trade_randoms_thread,
                                };
                                use pokered_data::species::Species;
                                use pokered_data::trades::find_npc_trade;
                                // No cutscene in the terminal frontend: the
                                // mutation applies immediately and the
                                // suspended script resumes with the outcome
                                // (deferred until after the drain — borrow).
                                let mut traded = false;
                                if let (Some(off_sp), Some(rec_sp)) = (
                                    Species::from_scene_name(&offered),
                                    Species::from_scene_name(&received),
                                ) {
                                    if let Some(idx) = self.save_data.party.find_species(off_sp) {
                                        if let Ok(removed) = self.save_data.party.remove(idx) {
                                            // TradeMons-table nickname is
                                            // authoritative; script arg is the
                                            // fallback for non-table pairs.
                                            let nick = find_npc_trade(off_sp, rec_sp)
                                                .map(|t| t.nickname.to_string())
                                                .unwrap_or(nickname);
                                            let (dv_bytes, ot_id) = roll_npc_trade_randoms_thread();
                                            let player_id = self.save_data.game_data.player_id;
                                            if let Some(mon) = assemble_npc_trade_mon(
                                                rec_sp,
                                                removed.level,
                                                &nick,
                                                dv_bytes,
                                                ot_id,
                                                player_id,
                                            ) {
                                                let _ = self.save_data.party.add(mon);
                                                // AddPartyMon sets the Pokédex
                                                // seen+owned flags.
                                                self.save_data
                                                    .game_data
                                                    .pokedex
                                                    .set_seen(rec_sp);
                                                self.save_data
                                                    .game_data
                                                    .pokedex
                                                    .set_owned(rec_sp);
                                                traded = true;
                                            }
                                        }
                                    }
                                }
                                self.overworld.party_count = self.save_data.party.count() as u8;
                                self.overworld.party_lead_level =
                                    self.save_data.party.leader_level();
                                trade_results.push(traded);
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
                                self.overworld.party_count = self.save_data.party.count() as u8;
                                self.overworld.party_lead_level =
                                    self.save_data.party.leader_level();
                            }
                            OverworldGameDataRequest::WithdrawDaycare => {
                                self.save_data.withdraw_daycare();
                                self.overworld.party_count = self.save_data.party.count() as u8;
                                self.overworld.party_lead_level =
                                    self.save_data.party.leader_level();
                            }
                        }
                    }
                    for traded in trade_results {
                        self.overworld.resume_script_after_trade(traded);
                    }

                    if let Some(ref audio) = self.audio {
                        match self.overworld.sfx_event {
                            OverworldSfxEvent::GoInside => audio.play_sfx(SfxId::GoInside),
                            OverworldSfxEvent::GoOutside => audio.play_sfx(SfxId::GoOutside),
                            OverworldSfxEvent::Collision => audio.play_sfx(SfxId::Collision),
                            OverworldSfxEvent::Ledge => audio.play_sfx(SfxId::Ledge),
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
                                    audio.fade_out(4);
                                }
                                OverworldAudioRequest::PlayMapMusic { map } => {
                                    let data_id =
                                        pokered_core::overworld::map_loading::get_map_music(map);
                                    if let Some(id) = MusicId::from_u8(data_id as u8) {
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

                    if let Some(encounter) = self.overworld.pending_wild_encounter.take() {
                        self.start_wild_battle(encounter.species, encounter.level);
                        // The Old-Man catch tutorial (ViridianCity's demo):
                        // the battle auto-plays and the player side is "OLD
                        // MAN" (BATTLE_TYPE_OLD_MAN) — mirroring the native
                        // app's encounter consumer.
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
                    } else if let Some(shop_items) = self.overworld.pending_shop.take() {
                        match ShopInventory::from_item_id_strings(&shop_items) {
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
                        // game.openSlotsMachine() — the coin balance is the
                        // save's live `player_coins`; the seed mixes in the
                        // frame counter so two spins never share a reel layout
                        // (app mirror).
                        let coins = self.save_data.game_data.player_coins;
                        let seed =
                            (self.frame_count as u32).wrapping_mul(2654435761).wrapping_add(1);
                        self.slots_screen = Some(SlotsScreen::new(lucky, coins, seed));
                        ScreenAction::Transition(GameScreen::Slots)
                    } else if let Some(floors) = self.overworld.pending_elevator.take() {
                        self.elevator_screen = Some(ElevatorScreen::new(floors));
                        ScreenAction::Transition(GameScreen::Elevator)
                    } else if let Some(candidates) = self.overworld.pending_filter_bag.take() {
                        // Show only the candidate items the player actually
                        // carries (e.g. CeladonMartRoof drinks, fossil room,
                        // badge list), mirroring the native app.
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
                // wOptions BIT_BATTLE_SHIFT + the player name used by the
                // "Will <PLAYER> change #MON?" prompt — pushed every frame like
                // battle_animation below.
                self.battle.battle_style = self.state.config.battle_style;
                self.battle.player_name = Some(self.player_name.clone());
                let action = self.battle.update_frame(battle_input);
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
                    if let Some(species) = self.battle_vfx.take_cry_pending() {
                        play_species_cry(audio, species);
                    }
                    // Trainer-appear SFX (plain SFX_SILPH_SCOPE — the
                    // original's wTempoModifier write is dead for non-cries).
                    if self.battle_vfx.take_trainer_appear_sfx_pending() {
                        audio.play_sfx(SfxId::SilphScope);
                    }
                    // Ball-flow SFX (SFX_BALL_TOSS / SFX_TINK per shake /
                    // SFX_BALL_POOF).
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
                    // Message-based battle SFX (mirrors pokered-app): the
                    // effectiveness jingles, enemy-faint FALL/THUD pair, and
                    // withdraw/crit cues fire on battle-message change. Move
                    // SFX are NOT here — those are per-command sounds of the
                    // move animation (take_move_sfx above).
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
                                // battles play the victory music instead. A
                                // PLAYER faint plays the mon's own cry, not
                                // the fall SFX (queued via cry_pending).
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

                action
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
                    StartMenuAction::OpenPokedex => {
                        ScreenAction::Transition(GameScreen::Pokedex)
                    }
                    StartMenuAction::OpenTrainerInfo => {
                        ScreenAction::Transition(GameScreen::TrainerCard)
                    }
                    StartMenuAction::OpenItem => {
                        let items: Vec<(pokered_data::items::ItemId, u32)> =
                            self.save_data.game_data.bag.items().to_vec();
                        self.bag_screen = BagScreenState::new(items);
                        ScreenAction::Transition(GameScreen::Bag)
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

                if let Some((a, b)) = self.party_screen.take_pending_swap() {
                    if let Err(e) = self.save_data.party.swap(a, b) {
                        eprintln!("party swap {a}<->{b} failed: {e:?}");
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
                                            let _ = self
                                                .save_data
                                                .game_data
                                                .bag
                                                .remove_item(item, 1);
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
                                            let _ = self
                                                .save_data
                                                .game_data
                                                .bag
                                                .remove_item(item, 1);
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
                            None => {
                                pokered_core::overworld::field_moves::FieldMoveOutcome::Done
                            }
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
                            // GoBackToPartyMenu). The entry hook swaps it into
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
                    if let Some(ref audio) = self.audio {
                        use pokered_core::slots_screen::SlotsPhase;
                        // Reel-stop / spin-start feedback (app mirror).
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
                // Persist the running coin balance every frame (the save is
                // the authoritative coin store; the overworld HUD reads it).
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
            GameScreen::Bag => {
                let bag_input = BagScreenInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    left: input.is_just_pressed(GbButton::Left),
                    right: input.is_just_pressed(GbButton::Right),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
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
                                        let _ =
                                            self.save_data.game_data.bag.remove_item(item, 1);
                                    }
                                    ScreenAction::Transition(GameScreen::Overworld)
                                }
                            }
                        }
                    }
                    BagScreenAction::Active => ScreenAction::Continue,
                }
            }
            GameScreen::TownMap => {
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
            GameScreen::PC => {
                let menu_input = MenuInput {
                    up: input.is_just_pressed(GbButton::Up),
                    down: input.is_just_pressed(GbButton::Down),
                    a: input.is_just_pressed(GbButton::A),
                    b: input.is_just_pressed(GbButton::B),
                };
                if self.pc_screen.is_none() {
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
                    if pc.take_save_request() {
                        // CHANGE BOX saves the game (save.asm ChangeBox →
                        // SaveGameData); keep the SRAM box-num byte in sync.
                        self.save_data.game_data.current_box_num =
                            self.save_data.pc_storage.current_box_index() as u8 | 0x80;
                        self.save_to_file();
                    }
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
                    TrainerCardAction::Closed => {
                        ScreenAction::Transition(GameScreen::StartMenu)
                    }
                    TrainerCardAction::Active => ScreenAction::Continue,
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

    /// Queue the evolution cutscene for the evolutions detected at battle end
    /// (EvolutionAfterBattle, engine/pokemon/evos_moves.asm).
    fn queue_evolution_cutscene(
        &mut self,
        events: Vec<pokered_core::battle::settlement::EvolutionEvent>,
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
                    // Post-battle evolutions are cancellable (wForceEvolution
                    // is cleared by EndOfBattle, end_of_battle.asm:43-44).
                    force: false,
                }
            })
            .collect();
        if !queue.is_empty() {
            self.evolution_anim = Some(EvolutionScreenState::new(queue, None, is_zh));
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

    /// Apply one resolved evolution from the cutscene; a B-cancel applies
    /// nothing (the mon retries on its next level-up).
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

        // The evolution cutscene takes over the whole screen while it plays.
        if let Some(ref anim) = self.evolution_anim {
            draw_evolution(anim, &mut self.resources, frame_buffer);
            return;
        }

        // The Hall of Fame roll call and the end credits take over the whole
        // screen while they play.
        if let Some(ref hof) = self.hof_ceremony {
            draw_hof_ceremony(hof, &mut self.resources, frame_buffer);
            return;
        }
        if let Some(ref roll) = self.credits {
            draw_credits(roll, &mut self.resources, frame_buffer);
            return;
        }

        frame_buffer.clear(Rgba::WHITE);

        match self.state.screen {
            GameScreen::LanguageSelect => {},
            GameScreen::GameFreakSplash => {
                draw_gamefreak_splash(&self.gamefreak_splash, &mut self.resources, frame_buffer);
            }
            GameScreen::CopyrightSplash => {
                draw_title_screen(&self.title_screen, true, &mut self.resources, frame_buffer);
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
                draw_oak_speech(&self.oak_speech, &mut self.resources, frame_buffer);
            }
            GameScreen::Overworld => {
                draw_overworld(&mut self.overworld, &mut self.resources, frame_buffer);
            }
            GameScreen::Battle => {
                // Capture the pre-battle overworld frame once, so the screen
                // wipe can eat it tile by tile (mirrors pokered-app).
                if self.battle_vfx.has_transition()
                    && self.battle_vfx.overworld_snapshot.is_none()
                {
                    let mut snapshot =
                        pokered_renderer::FrameBuffer::new(RenderConfig::new(160, 144), Rgba::BLACK);
                    draw_overworld(&mut self.overworld, &mut self.resources, &mut snapshot);
                    self.battle_vfx.overworld_snapshot = Some(snapshot);
                }
                draw_battle(
                    &self.battle,
                    &mut self.resources,
                    frame_buffer,
                    &mut self.battle_vfx,
                );
                if !self.battle_vfx.has_transition() {
                    self.battle_vfx.clear_snapshot();
                }
            }
            GameScreen::StartMenu => {
                draw_overworld(&mut self.overworld, &mut self.resources, frame_buffer);
                draw_start_menu(&self.start_menu, &self.player_name, frame_buffer, self.state.config.language);
            }
            GameScreen::OptionsMenu => {
                draw_options_menu(&self.options_menu, frame_buffer, self.state.config.language);
            }
            GameScreen::SaveMenu => {
                draw_overworld(&mut self.overworld, &mut self.resources, frame_buffer);
                draw_save_menu(&self.save_menu, frame_buffer, self.state.config.language);
            }
            GameScreen::PartyScreen => {
                draw_party_screen(&self.party_screen, None, 0, frame_buffer, self.state.config.language);
            }
            GameScreen::PokemonStatsScreen(_) => {
                if let Some(ref ss) = self.stats_screen {
                    draw_stats_screen(ss, frame_buffer, self.state.config.language);
                }
            }
            GameScreen::Shop(ref mart_state) => {
                let money = self.save_data.game_data.player_money;
                let bag_slice = self.save_data.game_data.bag.items();
                draw_mart(mart_state, money, &bag_slice[..], frame_buffer, self.state.config.language);
            }
            GameScreen::PC => {
                if let Some(ref pc) = self.pc_screen {
                    draw_pc(pc, &self.save_data, &mut self.resources, frame_buffer);
                }
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
                );
            }
            GameScreen::Slots => {
                if let Some(ref slots) = self.slots_screen {
                    draw_slots(slots, frame_buffer);
                }
            }
            GameScreen::Elevator => {
                if let Some(ref elevator) = self.elevator_screen {
                    draw_elevator(elevator, frame_buffer);
                }
            }
            GameScreen::FilterBag => {
                if let Some(ref filter) = self.elevator_screen {
                    draw_filter_bag(filter, frame_buffer);
                }
            }
            GameScreen::Diploma => {
                draw_diploma(&self.player_name, frame_buffer);
            }
            GameScreen::Pokedex => {
                draw_pokedex_screen(
                    &self.pokedex_screen,
                    matches!(
                        self.state.config.language,
                        pokered_core::game_state::Lang::Zh
                    ),
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
                    frame_buffer,
                );
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

impl dotzuki_tui::TuiGame for PokemonGame {
    type Button = GbButton;
    type Fb = FrameBuffer;

    fn update(&mut self, input: &dotzuki_tui::InputState<Self::Button>) {
        self.update(input);
    }

    fn draw(&mut self, fb: &mut FrameBuffer) {
        self.draw(fb);
    }

    fn exit_requested(&self) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_tui::InputState;
    use pokered_core::game_state::MainMenuChoice;

    /// A game standing in the overworld as if a save was loaded via CONTINUE:
    /// save-file position (5,5) on Viridian City, live overworld position
    /// (30,20) on Pallet Town. Any rebuild of the overworld from the save data
    /// becomes visible as a position reset.
    fn game_at_overworld() -> PokemonGame {
        let mut game = PokemonGame::new(pokered_core::data::wild_data::GameVersion::Red);
        game.state.screen = GameScreen::Overworld;
        game.main_menu.last_choice = Some(MainMenuChoice::Continue);
        game.save_data.game_data.position.map_id =
            pokered_data::maps::MapId::ViridianCity as u8;
        game.save_data.game_data.position.x = 5;
        game.save_data.game_data.position.y = 5;
        game.overworld.state.current_map = pokered_data::maps::MapId::PalletTown;
        game.overworld.state.player.x = 30;
        game.overworld.state.player.y = 20;
        game
    }

    fn press(button: GbButton) -> InputState<GbButton> {
        let mut input = InputState::new();
        input.press(button);
        input
    }

    #[test]
    fn bag_field_item_use_returns_to_overworld_without_rebuild() {
        let mut game = game_at_overworld();
        let _ = game
            .save_data
            .game_data
            .bag
            .add_item(pokered_data::items::ItemId::Bicycle, 1);
        game.handle_transition(GameScreen::Bag);
        assert_eq!(game.state.screen, GameScreen::Bag);

        // A: open the USE/TOSS/CANCEL menu; A again: USE.
        game.update(&press(GbButton::A));
        assert_eq!(game.state.screen, GameScreen::Bag);
        game.update(&press(GbButton::A));
        assert_eq!(game.state.screen, GameScreen::Overworld);
        // The live overworld must survive (BICYCLE toggles riding in place);
        // the last-saved position is (5,5), so a rebuild would show up here.
        assert_eq!(game.overworld.state.player.x, 30);
        assert_eq!(game.overworld.state.player.y, 20);
        assert_eq!(game.overworld.state.current_map, pokered_data::maps::MapId::PalletTown);
        assert!(
            game.overworld.pending_dialogue.is_some(),
            "field-item result text must be shown on return"
        );
    }

    #[test]
    fn bag_cancel_returns_to_start_menu() {
        let mut game = game_at_overworld();
        let _ = game
            .save_data
            .game_data
            .bag
            .add_item(pokered_data::items::ItemId::Bicycle, 1);
        game.handle_transition(GameScreen::Bag);
        game.update(&press(GbButton::B));
        assert_eq!(game.state.screen, GameScreen::StartMenu);
    }

    #[test]
    fn fly_picks_destination_and_warps_without_overworld_rebuild() {
        let mut game = game_at_overworld();
        // Viridian City is a visited fly destination (a fresh save has none).
        game.save_data
            .game_data
            .mark_town_visited(pokered_data::maps::MapId::ViridianCity);
        game.pending_fly_map = true;
        game.handle_transition(GameScreen::TownMap);
        assert_eq!(
            game.town_map_screen.mode(),
            pokered_core::town_map_screen::TownMapMode::Fly,
            "FLY opens the town map in destination-picker mode"
        );

        // A on the first destination → FlyTo: the warp lands on the CORE
        // overworld (pending_warp + fade) and the screen returns to the
        // overworld — which must NOT be rebuilt from the save position.
        game.update(&press(GbButton::A));
        assert_eq!(game.state.screen, GameScreen::Overworld);
        assert!(
            game.overworld.pending_warp.is_some(),
            "FlyTo must queue the warp on the live overworld"
        );
        assert_eq!(game.overworld.state.player.x, 30, "overworld not rebuilt");

        // Re-open in view mode (bag TOWN MAP item): B closes back to the
        // overworld directly (no pending_fly_map).
        game.town_map_screen = TownMapScreenState::new(game.overworld.state.current_map);
        game.state.screen = GameScreen::TownMap;
        game.update(&press(GbButton::B));
        assert_eq!(game.state.screen, GameScreen::Overworld);
    }

    // ── Slots / Elevator / FilterBag / Diploma / soft reset ──────────────
    // End-to-end arms: the state machines themselves are tested in
    // pokered-core (slots_screen / elevator_screen); these verify the TUI
    // wiring (coin persistence, screen exit, script-resume delivery).

    fn hold_all(buttons: &[GbButton]) -> InputState<GbButton> {
        let mut input = InputState::new();
        for b in buttons {
            input.press(*b);
        }
        input
    }

    #[test]
    fn slots_full_lifecycle_persists_coins_and_exits() {
        let mut game = game_at_overworld();
        game.slots_screen = Some(SlotsScreen::new(false, 100, 1));
        game.state.screen = GameScreen::Slots;

        // A: deduct the bet and spin.
        game.update(&press(GbButton::A));
        let slots = game.slots_screen.as_ref().unwrap();
        assert_eq!(slots.phase, pokered_core::slots_screen::SlotsPhase::Spinning);
        assert_eq!(slots.coins, 99);
        assert_eq!(
            game.save_data.game_data.player_coins, 99,
            "running coin balance must persist to the save every frame"
        );

        // Keep A held: each reel stops when aligned; all stop → Result.
        for _ in 0..2000 {
            game.update(&press(GbButton::A));
            if game.slots_screen.as_ref().unwrap().phase
                == pokered_core::slots_screen::SlotsPhase::Result
            {
                break;
            }
        }
        let slots = game.slots_screen.as_ref().unwrap();
        assert_eq!(slots.phase, pokered_core::slots_screen::SlotsPhase::Result);
        assert!(slots.reels_stopped.iter().all(|&s| s));
        assert_eq!(game.save_data.game_data.player_coins, slots.coins);

        // A on the result screen → bet selection again; B → exit to the
        // overworld (the coin balance is already persisted).
        game.update(&press(GbButton::A));
        assert_eq!(
            game.slots_screen.as_ref().unwrap().phase,
            pokered_core::slots_screen::SlotsPhase::BetSelect
        );
        game.update(&press(GbButton::B));
        assert_eq!(game.state.screen, GameScreen::Overworld);
        assert!(game.slots_screen.is_none());
    }

    #[test]
    fn elevator_selects_floor_and_returns_to_overworld() {
        let mut game = game_at_overworld();
        game.elevator_screen = Some(ElevatorScreen::new(vec![
            "1F".into(),
            "2F".into(),
            "3F".into(),
        ]));
        game.state.screen = GameScreen::Elevator;

        // Down, Down → 3F (index 2); A confirms → script resumed + overworld.
        game.update(&press(GbButton::Down));
        game.update(&press(GbButton::Down));
        game.update(&press(GbButton::A));
        assert_eq!(game.state.screen, GameScreen::Overworld);
        assert!(game.elevator_screen.is_none());
    }

    #[test]
    fn elevator_b_cancels_back_to_overworld() {
        let mut game = game_at_overworld();
        game.elevator_screen = Some(ElevatorScreen::new(vec!["1F".into()]));
        game.state.screen = GameScreen::Elevator;
        game.update(&press(GbButton::B));
        assert_eq!(game.state.screen, GameScreen::Overworld);
        assert!(game.elevator_screen.is_none());
    }

    #[test]
    fn filter_bag_select_returns_to_overworld_with_item() {
        let mut game = game_at_overworld();
        game.elevator_screen = Some(ElevatorScreen::new(vec![
            "FRESH WATER".into(),
            "SODA POP".into(),
        ]));
        game.state.screen = GameScreen::FilterBag;

        // Down → SODA POP; A confirms → script resumed with the item name.
        game.update(&press(GbButton::Down));
        game.update(&press(GbButton::A));
        assert_eq!(game.state.screen, GameScreen::Overworld);
        assert!(game.elevator_screen.is_none());
    }

    #[test]
    fn diploma_a_closes_to_overworld() {
        let mut game = game_at_overworld();
        game.state.screen = GameScreen::Diploma;
        game.update(&press(GbButton::A));
        assert_eq!(game.state.screen, GameScreen::Overworld);
    }

    #[test]
    fn soft_reset_needs_16_frame_hold_then_title() {
        let mut game = game_at_overworld();
        let combo = hold_all(&[GbButton::A, GbButton::B, GbButton::Start, GbButton::Select]);

        // 15 frames of the combo: not reset yet. (The Start press opens the
        // START menu during the hold — the app behaves identically; the
        // soft-reset check only returns once the 16-frame threshold hits.)
        for _ in 0..15 {
            game.update(&combo);
        }
        assert_ne!(
            game.state.screen,
            GameScreen::TitleScreen,
            "a short hold must not soft reset"
        );
        assert_eq!(game.soft_reset_frames, 15);
        // The 16th frame triggers the reset → title screen.
        game.update(&combo);
        assert_eq!(
            game.state.screen,
            GameScreen::TitleScreen,
            "a 16-frame hold soft-resets to the title"
        );
        assert_eq!(game.soft_reset_frames, 0, "the hold counter clears on reset");

        // Releasing the combo resets the hold counter: a fresh 15-frame hold
        // after a release must not reset again.
        let mut game2 = game_at_overworld();
        let released = InputState::new();
        game2.update(&released);
        for _ in 0..15 {
            game2.update(&combo);
        }
        assert_eq!(game2.soft_reset_frames, 15);
        assert_ne!(game2.state.screen, GameScreen::TitleScreen);
    }
}
