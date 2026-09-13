//! Frame-level fork/restore snapshots (agent M5).
//!
//! [`OverworldSnapshot`] / [`BattleSnapshot`] capture every piece of
//! runtime state that influences future frames: positions, NPCs,
//! dialogue/typewriter machine, warp/transition machinery, script engine
//! (interpreter stack + suspended awaits + flag/RNG host), trigger and
//! cutscene managers, encounter pendings, presentation state machines,
//! and both RNG streams. Restoring one of these and replaying the same
//! inputs produces the same frames.
//!
//! Deliberately NOT captured (rebuilt or transient):
//! - script function tables, script loader, map script config, scene
//!   providers — deterministic per map, rebuilt via
//!   `load_map_script_ex(current_map, false)` on restore;
//! - script query seeds (`script_bag_names` / `script_party_species`) —
//!   re-seeded by the app every frame;
//! - output queues (audio/sfx/game-data requests) — drained by the
//!   frontend each frame;
//! - link-play state (`link_rng`, link pending fields) — link battles
//!   are out of scope for fork/restore;
//! - battle visual effects (app-side renderer state) — presentation
//!   only; a restore restarts transient animations.

use crate::alloc_prelude::*;

use dotzuki_engine_script::CutsceneManager;
use serde::{Deserialize, Serialize};

use crate::battle::menu::{BagMenuState, BattleMenuState, MoveMenuState, PartySubMenuState};
use crate::battle::pokered_rules::runtime::StdBattleRng;
use crate::battle::safari::SafariState;
use crate::battle::settlement::BattleSettlement;
use crate::battle::state::{BattleState, Pokemon};
use crate::battle::{
    BattleAnimEvent, BattleItemSfx, BattlePhase, BattleScreen, BattleTransition, HpBarAnim,
    PokeballSlotStatus,
};
use crate::game_state::BattleStyle;
use crate::items::inventory::{Inventory, BAG_ITEM_CAPACITY};
use crate::overworld::event_flags::EventFlags;
use crate::overworld::fishing::PendingFishing;
use crate::overworld::forced_bike::ForcedBikeState;
use crate::overworld::npc_movement::NpcRuntimeState;
use crate::overworld::presentation::{
    BoulderDustState, CutAnimState, EnterMapFlyState, EnterMapSpinState, FieldMoveRestoreState,
    FieldMoveStepState, FishingAnimState, LedgeJumpState, LeaveMapFlyState, TeleportSpinState,
    TileAnimState,
};
use crate::overworld::screen::{
    BedroomDialogue, ConnectionNpcPreview, EmotionBubbleState, HealingMachineState,
    PendingGivePokemon, PendingTrainerBattle, PendingWarp, PendingWildEncounter, PokedexEntryState,
    PokemonNpcData, TrainerEncounterIntro, WarpFadeState,
};
use crate::overworld::script_bridge::{PendingChoice, ScriptEffect};
use crate::overworld::special_terrain::DarkCaveState;
use crate::overworld::{
    MapData, MovementState, OverworldScreen, OverworldState, PendingConnection, PendingCut,
};
use crate::party_select::PartySelectState;
use dotzuki_engine::trigger_manager::TriggerManager;
use pokered_data::items::ItemId;
use pokered_data::maps::MapId;
use pokered_data::species::Species;
use pokered_data::trainer_data::TrainerClass;

use crate::overworld::native_script::NativeScriptEngineSnapshot;

/// Everything on [`OverworldScreen`] that influences future frames.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverworldSnapshot {
    pub frame_counter: u32,
    pub state: OverworldState,
    pub map_data: Option<MapData>,
    pub npc_states: Vec<NpcRuntimeState>,
    pub npc_pokemon_data: Vec<PokemonNpcData>,
    pub pending_dialogue: Option<BedroomDialogue>,
    pub pending_choice: Option<PendingChoice>,
    pub pending_pokedex_entry: Option<PokedexEntryState>,
    pub pending_naming_screen: Option<crate::naming_screen::NamingScreenState>,
    pub naming_flash_frames: u8,
    pub pending_party_select: Option<PartySelectState>,
    pub party_select_requested: bool,
    pub pending_set_nickname: Option<(u8, String)>,
    pub pending_emotion_bubble: Option<EmotionBubbleState>,
    pub pending_healing_machine: Option<HealingMachineState>,
    pub last_map: Option<MapId>,
    pub last_map_entry: Option<(u8, u8)>,
    pub warp_fade_state: WarpFadeState,
    pub pending_warp: Option<PendingWarp>,
    pub pending_connection: Option<PendingConnection>,
    pub connection_npc_preview: Option<ConnectionNpcPreview>,
    pub pending_wild_encounter: Option<PendingWildEncounter>,
    pub pending_trainer_battle: Option<PendingTrainerBattle>,
    pub trainer_encounter_intro: Option<TrainerEncounterIntro>,
    pub trainer_intro_text_pending: Option<TrainerEncounterIntro>,
    pub pending_give_pokemon: Option<PendingGivePokemon>,
    pub bump_anim_counter: u8,
    pub ledge_jump: Option<LedgeJumpState>,
    pub field_move_step: Option<FieldMoveStepState>,
    pub pending_field_move_step: Option<FieldMoveStepState>,
    pub field_move_step_needs_restore: bool,
    pub field_move_restore: Option<FieldMoveRestoreState>,
    pub pending_cut: Option<PendingCut>,
    pub cut_anim: Option<CutAnimState>,
    pub cut_retained_dialogue: Option<BedroomDialogue>,
    pub player_name: String,
    pub rival_name: String,
    pub text_delay_frames: u16,
    pub prev_a_pressed: bool,
    pub prev_movement_state: MovementState,
    pub prev_b_pressed: bool,
    pub prev_up_pressed: bool,
    pub prev_down_pressed: bool,
    pub cutscene_manager: CutsceneManager,
    pub trigger_manager: TriggerManager,
    pub active_script_effect: Option<ScriptEffect>,
    pub joy_ignore_mask: u8,
    pub scripted_player_path: VecDeque<(u16, u16)>,
    pub script_awaiting_battle: bool,
    pub script_awaiting_elevator: bool,
    pub script_awaiting_filter_bag: bool,
    pub script_awaiting_trade: bool,
    pub player_starter: u8,
    pub pending_shop: Option<Vec<String>>,
    pub pending_slots: Option<bool>,
    pub active_sign_text_id: Option<u8>,
    pub lucky_slot_machine_sign: Option<u8>,
    pub pending_elevator: Option<Vec<String>>,
    pub pending_filter_bag: Option<Vec<String>>,
    pub pending_diploma: bool,
    pub pending_pc: Option<String>,
    pub pending_town_map: bool,
    pub pending_hof_ceremony: bool,
    pub heal_requested: bool,
    pub party_count: u8,
    pub box_count: u8,
    pub poison_step_counter: u32,
    pub party_lead_level: u8,
    pub unified_flags: EventFlags,
    #[serde(with = "bytes_32_serde")]
    pub toggleable_object_flags: [u8; 32],
    pub hidden_item_flags: [u8; crate::save::game_data::HIDDEN_ITEMS_BYTES],
    pub hidden_coin_flags: [u8; crate::save::game_data::HIDDEN_COINS_BYTES],
    pub player_coins: u16,
    pub itemfinder_dings: Option<(u8, u8)>,
    pub rng: crate::rng::SeededRng,
    pub safari_steps: u16,
    pub safari_balls: u8,
    pub safari_game_active: bool,
    pub safari_eject_pending: Option<PendingWarp>,
    pub strength_active: bool,
    pub tried_push_boulder: bool,
    pub boulder_dust_frames: u8,
    pub boulder_dust: BoulderDustState,
    pub dark_cave: DarkCaveState,
    pub forced_bike: ForcedBikeState,
    pub flash_lit_frames: u8,
    pub flash_pending_white: bool,
    pub warp_fade_to_white: bool,
    pub teleport_spin: Option<TeleportSpinState>,
    pub fly_departure: Option<LeaveMapFlyState>,
    pub enter_map_anim: Option<EnterMapSpinState>,
    pub enter_map_fly_anim: Option<EnterMapFlyState>,
    pub pending_fly_arrival: bool,
    pub fly_arrival_delay_frames: u8,
    pub elevator_shake: Option<crate::overworld::presentation::ElevatorShakeState>,
    pub elevator_shake_pending: bool,
    pub fishing_cast_delay: u16,
    pub tile_anim: TileAnimState,
    pub post_dialogue_warp: Option<PendingWarp>,
    pub post_dialogue_battle: Option<PendingWildEncounter>,
    pub fishing_anim: Option<FishingAnimState>,
    pub pending_fishing: Option<PendingFishing>,
    pub ship_departure: Option<dotzuki_engine::overworld::presentation::ShipDepartureState>,
    /// Script-engine runtime state (function tables rebuilt on restore).
    pub script_engine: Option<NativeScriptEngineSnapshot>,
}

/// 32-byte flag array ↔ length-checked byte vector.
mod bytes_32_serde {
    use crate::alloc_prelude::Vec;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(bits: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        bits.as_slice().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let bytes = Vec::<u8>::deserialize(d)?;
        bytes
            .try_into()
            .map_err(|bytes: Vec<u8>| serde::de::Error::custom(format!("expected 32 bytes, got {}", bytes.len())))
    }
}

macro_rules! snapshot_fields {
    (restore $screen:expr, $snap:expr, $($field:ident),* $(,)?) => {
        $( $screen.$field = $snap.$field.clone(); )*
    };
}

impl OverworldSnapshot {
    pub fn capture<G: dotzuki_engine::GameData<Tileset = pokered_data::tilesets::TilesetId>>(
        screen: &OverworldScreen<G>,
    ) -> Self {
        let mut snap = Self {
            script_engine: None,
            frame_counter: 0,
            state: screen.state.clone(),
            map_data: None,
            npc_states: Vec::new(),
            npc_pokemon_data: Vec::new(),
            pending_dialogue: None,
            pending_choice: None,
            pending_pokedex_entry: None,
            pending_naming_screen: None,
            naming_flash_frames: 0,
            pending_party_select: None,
            party_select_requested: false,
            pending_set_nickname: None,
            pending_emotion_bubble: None,
            pending_healing_machine: None,
            last_map: None,
            last_map_entry: None,
            warp_fade_state: WarpFadeState::Idle,
            pending_warp: None,
            pending_connection: None,
            connection_npc_preview: None,
            pending_wild_encounter: None,
            pending_trainer_battle: None,
            trainer_encounter_intro: None,
            trainer_intro_text_pending: None,
            pending_give_pokemon: None,
            bump_anim_counter: 0,
            ledge_jump: None,
            field_move_step: None,
            pending_field_move_step: None,
            field_move_step_needs_restore: false,
            field_move_restore: None,
            pending_cut: None,
            cut_anim: None,
            cut_retained_dialogue: None,
            player_name: String::new(),
            rival_name: String::new(),
            text_delay_frames: 0,
            prev_a_pressed: false,
            prev_movement_state: MovementState::Idle,
            prev_b_pressed: false,
            prev_up_pressed: false,
            prev_down_pressed: false,
            cutscene_manager: CutsceneManager::new(),
            trigger_manager: TriggerManager::new(),
            active_script_effect: None,
            joy_ignore_mask: 0,
            scripted_player_path: VecDeque::new(),
            script_awaiting_battle: false,
            script_awaiting_elevator: false,
            script_awaiting_filter_bag: false,
            script_awaiting_trade: false,
            player_starter: 0,
            pending_shop: None,
            pending_slots: None,
            active_sign_text_id: None,
            lucky_slot_machine_sign: None,
            pending_elevator: None,
            pending_filter_bag: None,
            pending_diploma: false,
            pending_pc: None,
            pending_town_map: false,
            pending_hof_ceremony: false,
            heal_requested: false,
            party_count: 0,
            box_count: 0,
            poison_step_counter: 0,
            party_lead_level: 0,
            unified_flags: screen.unified_flags.clone(),
            toggleable_object_flags: screen.toggleable_object_flags,
            hidden_item_flags: screen.hidden_item_flags,
            hidden_coin_flags: screen.hidden_coin_flags,
            player_coins: 0,
            itemfinder_dings: None,
            rng: screen.rng.clone(),
            safari_steps: 0,
            safari_balls: 0,
            safari_game_active: false,
            safari_eject_pending: None,
            strength_active: false,
            tried_push_boulder: false,
            boulder_dust_frames: 0,
            boulder_dust: screen.boulder_dust,
            dark_cave: screen.dark_cave.clone(),
            forced_bike: screen.forced_bike,
            flash_lit_frames: 0,
            flash_pending_white: false,
            warp_fade_to_white: false,
            teleport_spin: None,
            fly_departure: None,
            enter_map_anim: None,
            enter_map_fly_anim: None,
            pending_fly_arrival: false,
            fly_arrival_delay_frames: 0,
            elevator_shake: None,
            elevator_shake_pending: false,
            fishing_cast_delay: 0,
            tile_anim: screen.tile_anim,
            post_dialogue_warp: None,
            post_dialogue_battle: None,
            fishing_anim: None,
            pending_fishing: None,
            ship_departure: None,
        };
        snapshot_fields!(restore &mut snap, screen,
            frame_counter, state, map_data, npc_states, npc_pokemon_data, pending_dialogue,
            pending_choice, pending_pokedex_entry, pending_naming_screen, naming_flash_frames,
            pending_party_select, party_select_requested, pending_set_nickname,
            pending_emotion_bubble, pending_healing_machine, last_map, last_map_entry,
            warp_fade_state, pending_warp, pending_connection, connection_npc_preview,
            pending_wild_encounter, pending_trainer_battle, trainer_encounter_intro,
            trainer_intro_text_pending, pending_give_pokemon, bump_anim_counter, ledge_jump,
            field_move_step, pending_field_move_step, field_move_step_needs_restore,
            field_move_restore, pending_cut, cut_anim, cut_retained_dialogue, player_name,
            rival_name, text_delay_frames, prev_a_pressed, prev_movement_state, prev_b_pressed,
            prev_up_pressed, prev_down_pressed, cutscene_manager, trigger_manager,
            active_script_effect, joy_ignore_mask, scripted_player_path, script_awaiting_battle,
            script_awaiting_elevator, script_awaiting_filter_bag, script_awaiting_trade,
            player_starter, pending_shop, pending_slots, active_sign_text_id,
            lucky_slot_machine_sign, pending_elevator, pending_filter_bag, pending_diploma,
            pending_pc, pending_town_map, pending_hof_ceremony, heal_requested, party_count,
            box_count, poison_step_counter, party_lead_level, unified_flags,
            toggleable_object_flags, hidden_item_flags, hidden_coin_flags, player_coins,
            itemfinder_dings, rng, safari_steps, safari_balls, safari_game_active,
            safari_eject_pending, strength_active, tried_push_boulder, boulder_dust_frames,
            boulder_dust, dark_cave, forced_bike, flash_lit_frames, flash_pending_white,
            warp_fade_to_white, teleport_spin, fly_departure, enter_map_anim,
            enter_map_fly_anim, pending_fly_arrival, fly_arrival_delay_frames, elevator_shake,
            elevator_shake_pending, fishing_cast_delay, tile_anim, post_dialogue_warp,
            post_dialogue_battle, fishing_anim, pending_fishing, ship_departure,
        );
        snap.script_engine = screen.script_engine.snapshot();
        snap
    }

    /// Restore into a LIVE screen: rebuilds the script function tables
    /// for the snapshot's map (without firing `@load`), then overwrites
    /// every runtime field.
    pub fn restore_into<G: dotzuki_engine::GameData<Tileset = pokered_data::tilesets::TilesetId>>(
        &self,
        screen: &mut OverworldScreen<G>,
    ) {
        screen.load_map_script_ex(self.state.current_map, false);
        snapshot_fields!(restore screen, self,
            frame_counter, state, map_data, npc_states, npc_pokemon_data, pending_dialogue,
            pending_choice, pending_pokedex_entry, pending_naming_screen, naming_flash_frames,
            pending_party_select, party_select_requested, pending_set_nickname,
            pending_emotion_bubble, pending_healing_machine, last_map, last_map_entry,
            warp_fade_state, pending_warp, pending_connection, connection_npc_preview,
            pending_wild_encounter, pending_trainer_battle, trainer_encounter_intro,
            trainer_intro_text_pending, pending_give_pokemon, bump_anim_counter, ledge_jump,
            field_move_step, pending_field_move_step, field_move_step_needs_restore,
            field_move_restore, pending_cut, cut_anim, cut_retained_dialogue, player_name,
            rival_name, text_delay_frames, prev_a_pressed, prev_movement_state, prev_b_pressed,
            prev_up_pressed, prev_down_pressed, cutscene_manager, trigger_manager,
            active_script_effect, joy_ignore_mask, scripted_player_path, script_awaiting_battle,
            script_awaiting_elevator, script_awaiting_filter_bag, script_awaiting_trade,
            player_starter, pending_shop, pending_slots, active_sign_text_id,
            lucky_slot_machine_sign, pending_elevator, pending_filter_bag, pending_diploma,
            pending_pc, pending_town_map, pending_hof_ceremony, heal_requested, party_count,
            box_count, poison_step_counter, party_lead_level, unified_flags,
            toggleable_object_flags, hidden_item_flags, hidden_coin_flags, player_coins,
            itemfinder_dings, rng, safari_steps, safari_balls, safari_game_active,
            safari_eject_pending, strength_active, tried_push_boulder, boulder_dust_frames,
            boulder_dust, dark_cave, forced_bike, flash_lit_frames, flash_pending_white,
            warp_fade_to_white, teleport_spin, fly_departure, enter_map_anim,
            enter_map_fly_anim, pending_fly_arrival, fly_arrival_delay_frames, elevator_shake,
            elevator_shake_pending, fishing_cast_delay, tile_anim, post_dialogue_warp,
            post_dialogue_battle, fishing_anim, pending_fishing, ship_departure,
        );
        if let Some(engine) = &self.script_engine {
            screen.script_engine.restore_snapshot(engine);
        }
        screen
            .script_engine
            .seed_flags(&screen.unified_flags.to_hashmap());
        // Transient output queues reset (presentation; they are
        // re-produced by future frames).
        screen.sfx_event = crate::overworld::OverworldSfxEvent::None;
        screen.audio_requests.clear();
        screen.game_data_requests.clear();
        screen.link_start_requested = false;
        screen.link_opponent = None;
    }
}

/// Everything on [`BattleScreen`] that influences future frames
/// (link-play fields excluded — out of scope).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleSnapshot {
    pub phase: BattlePhase,
    pub battle_menu: BattleMenuState,
    pub party_submenu: Option<PartySubMenuState>,
    pub bag_menu: Option<BagMenuState>,
    pub is_wild: bool,
    pub trainer_class: Option<TrainerClass>,
    pub trainer_name: Option<String>,
    pub player_bag: Inventory<BAG_ITEM_CAPACITY>,
    pub enemy_species: Species,
    pub enemy_level: u8,
    pub enemy_hp: u16,
    pub enemy_max_hp: u16,
    pub enemy_status: crate::battle::state::StatusCondition,
    pub player_species: Species,
    pub player_level: u8,
    pub player_hp: u16,
    pub player_max_hp: u16,
    pub player_status: crate::battle::state::StatusCondition,
    pub player_party_size: usize,
    pub enemy_party_size: usize,
    pub player_pokeball_status: [PokeballSlotStatus; 6],
    pub enemy_pokeball_status: [PokeballSlotStatus; 6],
    pub show_player_pokeballs: bool,
    pub show_enemy_pokeballs: bool,
    pub battle_state: Option<BattleState>,
    pub move_menu: Option<MoveMenuState>,
    pub current_message: Option<String>,
    pub party_cursor: usize,
    pub settlement: Option<BattleSettlement>,
    pub player_money: u32,
    pub trainer_npc_index: Option<u8>,
    pub end_battle_text: Option<String>,
    pub captured_mon: Option<Pokemon>,
    pub escaped_via_poke_doll: bool,
    pub map_id: u8,
    pub battle_transition: BattleTransition,
    pub enemy_ai_count: u8,
    pub is_ghost: bool,
    pub ghost_marowak_reveal: bool,
    pub ghost_marowak_unveiled: bool,
    pub is_safari: bool,
    pub safari: Option<SafariState>,
    pub safari_menu: crate::battle::menu::SafariBattleMenuState,
    pub is_old_man: bool,
    pub player_box_full: bool,
    pub hooked: bool,
    pub poke_flute_sfx_pending: bool,
    pub pending_item_sfx: Option<BattleItemSfx>,
    pub is_zh: bool,
    pub pending_anim_events: VecDeque<BattleAnimEvent>,
    pub hp_bar_anim: HpBarAnim,
    pub battle_style: BattleStyle,
    pub player_name: Option<String>,
    pub shift_prompt_yes: bool,
    pub pending_learn_moves: Vec<(usize, pokered_data::moves::MoveId)>,
    pub pending_shift_switch: Option<usize>,
    pub player_badges: u8,
    pub player_id: u16,
    pub rng: StdBattleRng,
}

impl BattleSnapshot {
    pub fn capture(screen: &BattleScreen) -> Self {
        Self {
            phase: screen.phase.clone(),
            battle_menu: screen.battle_menu.clone(),
            party_submenu: screen.party_submenu.clone(),
            bag_menu: screen.bag_menu.clone(),
            is_wild: screen.is_wild,
            trainer_class: screen.trainer_class,
            trainer_name: screen.trainer_name.clone(),
            player_bag: screen.player_bag.clone(),
            enemy_species: screen.enemy_species,
            enemy_level: screen.enemy_level,
            enemy_hp: screen.enemy_hp,
            enemy_max_hp: screen.enemy_max_hp,
            enemy_status: screen.enemy_status,
            player_species: screen.player_species,
            player_level: screen.player_level,
            player_hp: screen.player_hp,
            player_max_hp: screen.player_max_hp,
            player_status: screen.player_status,
            player_party_size: screen.player_party_size,
            enemy_party_size: screen.enemy_party_size,
            player_pokeball_status: screen.player_pokeball_status,
            enemy_pokeball_status: screen.enemy_pokeball_status,
            show_player_pokeballs: screen.show_player_pokeballs,
            show_enemy_pokeballs: screen.show_enemy_pokeballs,
            battle_state: screen.battle_state.clone(),
            move_menu: screen.move_menu.clone(),
            current_message: screen.current_message.clone(),
            party_cursor: screen.party_cursor,
            settlement: screen.settlement.clone(),
            player_money: screen.player_money,
            trainer_npc_index: screen.trainer_npc_index,
            end_battle_text: screen.end_battle_text.clone(),
            captured_mon: screen.captured_mon.clone(),
            escaped_via_poke_doll: screen.escaped_via_poke_doll,
            map_id: screen.map_id,
            battle_transition: screen.battle_transition,
            enemy_ai_count: screen.enemy_ai_count,
            is_ghost: screen.is_ghost,
            ghost_marowak_reveal: screen.ghost_marowak_reveal,
            ghost_marowak_unveiled: screen.ghost_marowak_unveiled,
            is_safari: screen.is_safari,
            safari: screen.safari.clone(),
            safari_menu: screen.safari_menu.clone(),
            is_old_man: screen.is_old_man,
            player_box_full: screen.player_box_full,
            hooked: screen.hooked,
            poke_flute_sfx_pending: screen.poke_flute_sfx_pending,
            pending_item_sfx: screen.pending_item_sfx,
            is_zh: screen.is_zh,
            pending_anim_events: screen.pending_anim_events.clone(),
            hp_bar_anim: screen.hp_bar_anim,
            battle_style: screen.battle_style,
            player_name: screen.player_name.clone(),
            shift_prompt_yes: screen.shift_prompt_yes,
            pending_learn_moves: screen.pending_learn_moves.clone(),
            pending_shift_switch: screen.pending_shift_switch,
            player_badges: screen.player_badges,
            player_id: screen.player_id,
            rng: screen.rng.clone(),
        }
    }

    pub fn restore_into(&self, screen: &mut BattleScreen) {
        *screen = Self::clone_into_screen(self);
    }

    fn clone_into_screen(snap: &Self) -> BattleScreen {
        let mut screen = BattleScreen::new(snap.is_wild);
        macro_rules! set {
            ($($field:ident),* $(,)?) => { $( screen.$field = snap.$field.clone(); )* };
        }
        set!(
            phase, battle_menu, party_submenu, bag_menu, is_wild, trainer_class, trainer_name,
            player_bag, enemy_species, enemy_level, enemy_hp, enemy_max_hp, enemy_status,
            player_species, player_level, player_hp, player_max_hp, player_status,
            player_party_size, enemy_party_size, player_pokeball_status, enemy_pokeball_status,
            show_player_pokeballs, show_enemy_pokeballs, battle_state, move_menu,
            current_message, party_cursor, settlement, player_money, trainer_npc_index,
            end_battle_text, captured_mon, escaped_via_poke_doll, map_id, battle_transition,
            enemy_ai_count, is_ghost, ghost_marowak_reveal, ghost_marowak_unveiled, is_safari,
            safari, safari_menu, is_old_man, player_box_full, hooked, poke_flute_sfx_pending,
            pending_item_sfx, is_zh, pending_anim_events, hp_bar_anim, battle_style,
            player_name, shift_prompt_yes, pending_learn_moves, pending_shift_switch,
            player_badges, player_id, rng,
        );
        screen
    }
}
