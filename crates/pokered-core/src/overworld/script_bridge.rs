use crate::alloc_prelude::*;
use pokered_data::maps::MapId;
use pokered_data::script_command::PokemonScriptCommand;
use dotzuki_engine_script::{CommandResult, ScriptCommand};
use serde_json::Value;

use super::npc_movement::NpcRuntimeState;
use super::{BedroomDialogue, DialoguePage, Direction};
use crate::naming_screen::NamingScreenState;

pub use dotzuki_engine_script::config::{CoordEventBinding, NpcBinding, SignBinding};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingChoice {
    pub options: Vec<String>,
    pub selected: u32,
    #[serde(default = "choice_cursor_default")]
    pub cursor_visible: bool,
    #[serde(default = "choice_cursor_default")]
    pub input_enabled: bool,
}

fn choice_cursor_default() -> bool { true }

impl PendingChoice {
    pub fn new(options: Vec<String>) -> Self {
        Self {
            options,
            selected: 0,
            cursor_visible: true,
            input_enabled: true,
        }
    }

    /// Move cursor up (wraps around).
    pub fn move_up(&mut self) {
        if self.options.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.options.len() as u32 - 1;
        } else {
            self.selected -= 1;
        }
    }

    /// Move cursor down (wraps around).
    pub fn move_down(&mut self) {
        if self.options.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.options.len() as u32;
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ScriptEffect {
    BikeShopOffer { state: super::bike_shop::BikeShopOfferState },
    FieldCry { state: super::field_cry::FieldCryState },
    /// Inner PrintText ending in DONE. Keep the window for its caller, and
    /// return after the final letter wait instead of waiting for a new press.
    PrintFieldText {
        text: String,
        #[serde(default)]
        retain_window: bool,
        #[serde(default)]
        prompt: pokered_data::script_command::FieldTextPrompt,
        #[serde(default)]
        prompt_frames_remaining: u8,
        #[serde(default)]
        prompt_last_buttons: Option<u8>,
        #[serde(default)]
        leading_sound: Option<String>,
        #[serde(default)]
        leading_sound_started: bool,
    },
    FinishFieldText,
    ShowDialogue {
        text: String,
    },
    /// Original FoundItemText: print, then play/wait GET_ITEM_1, no A prompt.
    ShowItemDialogue {
        text: String,
        #[serde(default)]
        sound_id: Option<String>,
        sound_started: bool,
    },
    ShowChoice {
        options: Vec<String>,
        started: bool,
        selected: u32,
    },
    GiveItem {
        item_id: String,
        quantity: u8,
    },
    TakeItem {
        item_id: String,
        quantity: u8,
    },
    GivePokemon {
        species: String,
        nickname: Option<String>,
        level: u8,
        #[serde(default)]
        flow: Option<super::script_interactions::GiftPokemonFlow>,
    },
    ReadingMenu { menu: super::script_interactions::ReadingMenu },
    ShowMoneyBox { amount: i64 },
    ShowCoinBox { amount: i64 },
    VendingDelivery { frames_elapsed: u8 },
    ShowObject {
        object_index: u8,
    },
    HideObject {
        object_index: u8,
    },
    ShowObjectByName {
        toggle_id: String,
    },
    HideObjectByName {
        toggle_id: String,
    },
    MoveNpc {
        npc_id: String,
        path: Vec<(u8, u8)>,
        started: bool,
    },
    StartNpcMove {
        npc_id: String,
        path: Vec<(u8, u8)>,
    },
    AwaitNpcMove {
        npc_id: String,
    },
    MovePlayer {
        path: Vec<(u8, u8)>,
        started: bool,
    },
    MovePlayerRelative {
        steps: Vec<(i16, i16)>,
        started: bool,
    },
    MoveNpcTo {
        npc_id: String,
        x: u8,
        y: u8,
        started: bool,
    },
    StartNpcMoveTo {
        npc_id: String,
        x: u8,
        y: u8,
    },
    MovePlayerTo {
        x: u8,
        y: u8,
        started: bool,
    },
    FaceNpc {
        npc_id: String,
        direction: Direction,
    },
    FacePlayer {
        direction: Direction,
    },
    PlayMusic {
        music_id: String,
    },
    PlaySound {
        sound_id: String,
    },
    /// Block until the frontend sequencer reports the current jingle ended.
    WaitMusic,
    WaitSound,
    StopMusic,
    FadeOutMusic,
    StartBattle {
        trainer_id: String,
        rival_triplet_base: Option<u8>,
        /// Optional explicit caller payload; old script snapshots keep None.
        #[serde(default)]
        end_battle_text: Option<String>,
    },
    StartWildBattle {
        species: String,
        level: u8,
    },
    /// The Viridian Old-Man catch tutorial (auto-played demo battle).
    OldManTutorial,
    TradePokemon {
        offered: String,
        received: String,
        nickname: String,
    },
    Delay {
        frames: u16,
        frames_remaining: u16,
    },
    WarpTo {
        map: String,
        x: u8,
        y: u8,
    },
    Heal,
    AnimateHealingMachine {
        phase: HealingMachinePhase,
        frames_remaining: u16,
    },
    FadeScreen {
        fade_type: String,
    },
    SetJoyIgnore {
        mask: u8,
    },
    ClearJoyIgnore,
    FollowNpc {
        npc_id: String,
        target_x: u8,
        target_y: u8,
        #[serde(default)]
        path: Option<Vec<(u8, u8)>>,
        #[serde(default)]
        player_steps: Option<Vec<(i8, i8)>>,
        phase: FollowNpcPhase,
    },
    ShowPokedexEntry {
        species: String,
        started: bool,
    },
    NamingScreen {
        species: String,
        naming_state: Option<NamingScreenState>,
        started: bool,
        result_name: Option<String>,
    },
    /// Open the party menu as a selector (Name Rater). `result_index` is -1
    /// while pending; set to the chosen 0-based index (or -1 on cancel) once
    /// the selection resolves.
    ChoosePartyPokemon {
        started: bool,
        result_index: Option<i32>,
    },
    /// Write a new nickname onto the party member at `index`.
    SetPartyNickname {
        index: u8,
        nickname: String,
    },
    Immediate {
        result: CommandResult,
    },
    /// A command reached pokered that its script host does not implement.
    UnsupportedCommand {
        name: String,
        reason: Option<String>,
    },
    OpenShop {
        items: Vec<String>,
    },
    /// Open the Game Corner slot-machine minigame.
    ///
    /// `Some(true)` = the high-odds lucky machine (explicit `openSlots(true)`);
    /// `None` = no explicit argument: the machine is lucky when the sign that
    /// fired the script matches the per-map-entry lucky roll
    /// (`GameCornerSelectLuckySlotMachine`).
    OpenSlots {
        lucky: Option<bool>,
    },
    /// Open the elevator floor-selection menu; the script resumes with the
    /// chosen floor index (0-based) when the app returns it.
    ElevatorMenu {
        floors: Vec<String>,
    },
    /// Open a filtered-bag menu; the script resumes with the chosen item's
    /// const name ("" on cancel).
    FilterBag {
        item_ids: Vec<String>,
    },
    /// Show the full-screen diploma (completed-POKeDEX reward).
    ShowDiploma,
    /// Open the PC storage screen (dispatched from the `openPC` /
    /// `openItemPC` / `openBillsPC` custom commands).
    /// Instant effect: the app opens the screen; the script runs on.
    OpenPc {
        kind: String,
    },
    /// Start the Cable Club link flow (dispatched from the `linkStart` custom
    /// command).
    /// Instant effect: flags `link_start_requested`; the app layer (which
    /// owns the link session) drives the request/accept flow.
    LinkStart,
    /// Record the party and play the Hall of Fame ceremony + credits
    /// (dispatched from the `enterHallOfFame` custom command). Instant
    /// effect: the app runs the ceremony and resets to the title screen;
    /// the script runs on.
    HallOfFameCeremony,
    ShowEmotionBubble {
        npc_id: String,
        emotion: String,
        frames_remaining: u16,
        started: bool,
    },
    SetNpcPosition {
        npc_id: String,
        x: u8,
        y: u8,
    },
    SetNpcFrame {
        npc_id: String,
        frame: u8,
    },
    GiveMoney {
        amount: u32,
    },
    TakeMoney {
        amount: u32,
    },
    GiveCoins {
        amount: u16,
    },
    TakeCoins {
        amount: u16,
    },
    /// Deposit the party member at `index` into the Day Care.
    DepositDaycare {
        index: u8,
    },
    /// Withdraw the Day Care Pokémon back into the party.
    WithdrawDaycare,
    PlayCry {
        species: String,
    },
    GiveBadge {
        badge: u8,
    },
    /// Replace a map block at runtime (BLOCK coordinates).
    ReplaceTileBlock {
        x: u8,
        y: u8,
        block_id: u8,
    },
    /// Play the S.S. Anne departure cutscene (VermilionDock) — the
    /// blocking ship-sail animation (smoke puffs + view scroll + erase).
    /// The script stays blocked until the animation completes.
    PlayShipDeparture {
        started: bool,
    },
    SilphRocketDeparture {
        state: super::silph_departure::SilphRocketDepartureState,
    },
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum FollowNpcPhase {
    StartNpc,
    Following {
        last_npc_x: u16,
        last_npc_y: u16,
        /// Set once the NPC's final tile has been appended to the player's
        /// shadow path — without it the npc_done branch re-appends the
        /// final tile every frame and the follow never completes.
        final_push_done: bool,
    },
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HealingMachinePhase {
    FadeOutMusic,
    WaitForFadeOut,
    HealPartyMember { member_index: u8, total_members: u8 },
    PlayHealedMusic,
    FlashSprite { flashes_remaining: u8 },
    WaitForMusic,
    PostMusicDelay,
    Done,
}

impl ScriptEffect {
    /// Both waitSound and a typed text_asm cry caller consume the actual
    /// sequencer completion sampled by App, never a guessed cry duration.
    pub(super) fn sound_wait_complete(sfx_playing: bool) -> bool { !sfx_playing }

    /// These actual controls have no source Joypad implementation yet. Do not
    /// carry a field last across them and call it verified history.
    pub fn has_unmodeled_input_owner(&self) -> bool {
        matches!(self,
            Self::ShowChoice { .. } | Self::ShowItemDialogue { .. } | Self::GivePokemon { .. } | Self::ReadingMenu { .. }
            | Self::TradePokemon { .. } | Self::ShowPokedexEntry { .. }
            | Self::NamingScreen { .. } | Self::ChoosePartyPokemon { .. }
            | Self::OpenShop { .. } | Self::OpenSlots { .. } | Self::ElevatorMenu { .. }
            | Self::FilterBag { .. } | Self::ShowDiploma | Self::OpenPc { .. }
            | Self::LinkStart | Self::HallOfFameCeremony
            | Self::MoveNpc { .. } | Self::MoveNpcTo { .. } | Self::AwaitNpcMove { .. }
            | Self::MovePlayer { .. } | Self::MovePlayerTo { .. }
            | Self::MovePlayerRelative { .. } | Self::FollowNpc { .. })
    }

    /// Nested gift/reading flows use the same text-window choice handoff.
    pub fn is_choice(&self) -> bool {
        match self {
            Self::BikeShopOffer { state } => state.accepts_input(),
            Self::ShowChoice { .. } => true,
            Self::GivePokemon { flow: Some(flow), .. } => flow.child.is_choice(),
            Self::ReadingMenu { menu } => menu.child.is_choice(),
            _ => false,
        }
    }

    /// Structured JSON form of an active script effect, for debug protocol
    /// observability: the driver sees effect progress fields (Delay
    /// countdown, move-path state, FollowNpc phase, choice cursor, …)
    /// instead of an opaque variant label. Every arm is a mechanical
    /// field→JSON mapping; new variants must be added here too.
    pub fn to_debug_json(&self) -> serde_json::Value {
        use serde_json::json;
        match self {
            ScriptEffect::OpenPc { kind } => json!({ "effect": "OpenPc", "kind": kind }),
            ScriptEffect::ShowDiploma => json!({ "effect": "ShowDiploma" }),
            ScriptEffect::LinkStart => json!({ "effect": "LinkStart" }),
            ScriptEffect::HallOfFameCeremony => json!({ "effect": "HallOfFameCeremony" }),
            ScriptEffect::ShowItemDialogue { text, sound_id, sound_started } => {
                json!({ "effect": "ShowItemDialogue", "text": text, "sound_id": sound_id, "sound_started": sound_started })
            }
            ScriptEffect::BikeShopOffer { state } => json!({ "effect": "BikeShopOffer", "state": state }),
            ScriptEffect::FieldCry { state } => json!({ "effect": "FieldCry", "state": state }),
            ScriptEffect::PrintFieldText { text, retain_window, prompt, leading_sound, leading_sound_started, .. } => {
                json!({ "effect": "PrintFieldText", "text": text, "retain_window": retain_window, "prompt": prompt, "leading_sound": leading_sound, "leading_sound_started": leading_sound_started })
            }
            ScriptEffect::ShowDialogue { text } => {
                json!({ "effect": "ShowDialogue", "text": text })
            }
            ScriptEffect::ShowChoice {
                options,
                started,
                selected,
            } => json!({
                "effect": "ShowChoice",
                "options": options,
                "started": started,
                "selected": selected,
            }),
            ScriptEffect::GiveItem { item_id, quantity } => {
                json!({ "effect": "GiveItem", "item_id": item_id, "quantity": quantity })
            }
            ScriptEffect::TakeItem { item_id, quantity } => {
                json!({ "effect": "TakeItem", "item_id": item_id, "quantity": quantity })
            }
            ScriptEffect::ReadingMenu { menu } => serde_json::json!({ "effect": "ReadingMenu", "child": menu.child.to_debug_json() }),
            ScriptEffect::ShowMoneyBox { amount } => serde_json::json!({ "effect": "ShowMoneyBox", "amount": amount }),
            ScriptEffect::ShowCoinBox { amount } => serde_json::json!({ "effect": "ShowCoinBox", "amount": amount }),
            ScriptEffect::VendingDelivery { frames_elapsed } => serde_json::json!({ "effect": "VendingDelivery", "frames_elapsed": frames_elapsed }),
            ScriptEffect::GivePokemon {
                species,
                nickname,
                level,
                flow
            } => json!({
                "effect": "GivePokemon",
                "species": species,
                "nickname": nickname,
                "level": level,
                "child": flow.as_ref().map(|flow| flow.child.to_debug_json()),
            }),
            ScriptEffect::ShowObject { object_index } => {
                json!({ "effect": "ShowObject", "object_index": object_index })
            }
            ScriptEffect::HideObject { object_index } => {
                json!({ "effect": "HideObject", "object_index": object_index })
            }
            ScriptEffect::ShowObjectByName { toggle_id } => {
                json!({ "effect": "ShowObjectByName", "toggle_id": toggle_id })
            }
            ScriptEffect::HideObjectByName { toggle_id } => {
                json!({ "effect": "HideObjectByName", "toggle_id": toggle_id })
            }
            ScriptEffect::MoveNpc { npc_id, path, started } => json!({
                "effect": "MoveNpc",
                "npc_id": npc_id,
                "path": path,
                "started": started,
            }),
            ScriptEffect::StartNpcMove { npc_id, path } => {
                json!({ "effect": "StartNpcMove", "npc_id": npc_id, "path": path })
            }
            ScriptEffect::AwaitNpcMove { npc_id } => {
                json!({ "effect": "AwaitNpcMove", "npc_id": npc_id })
            }
            ScriptEffect::MovePlayer { path, started } => {
                json!({ "effect": "MovePlayer", "path": path, "started": started })
            }
            ScriptEffect::MovePlayerRelative { steps, started } => json!({
                "effect": "MovePlayerRelative",
                "steps": steps,
                "started": started,
            }),
            ScriptEffect::MoveNpcTo {
                npc_id,
                x,
                y,
                started,
            } => json!({
                "effect": "MoveNpcTo",
                "npc_id": npc_id,
                "x": x,
                "y": y,
                "started": started,
            }),
            ScriptEffect::StartNpcMoveTo { npc_id, x, y } => {
                json!({ "effect": "StartNpcMoveTo", "npc_id": npc_id, "x": x, "y": y })
            }
            ScriptEffect::MovePlayerTo { x, y, started } => {
                json!({ "effect": "MovePlayerTo", "x": x, "y": y, "started": started })
            }
            ScriptEffect::FaceNpc { npc_id, direction } => {
                json!({ "effect": "FaceNpc", "npc_id": npc_id, "direction": format!("{:?}", direction) })
            }
            ScriptEffect::FacePlayer { direction } => {
                json!({ "effect": "FacePlayer", "direction": format!("{:?}", direction) })
            }
            ScriptEffect::PlayMusic { music_id } => {
                json!({ "effect": "PlayMusic", "music_id": music_id })
            }
            ScriptEffect::WaitMusic => json!({ "effect": "WaitMusic" }),
            ScriptEffect::WaitSound => json!({ "effect": "WaitSound" }),
            ScriptEffect::FinishFieldText => json!({ "effect": "FinishFieldText" }),
            ScriptEffect::PlaySound { sound_id } => {
                json!({ "effect": "PlaySound", "sound_id": sound_id })
            }
            ScriptEffect::StartBattle {
                trainer_id,
                rival_triplet_base,
                end_battle_text,
            } => json!({
                "effect": "StartBattle",
                "trainer_id": trainer_id,
                "rival_triplet_base": rival_triplet_base,
                "end_battle_text": end_battle_text,
            }),
            ScriptEffect::StartWildBattle { species, level } => {
                json!({ "effect": "StartWildBattle", "species": species, "level": level })
            }
            ScriptEffect::TradePokemon {
                offered,
                received,
                nickname,
            } => json!({
                "effect": "TradePokemon",
                "offered": offered,
                "received": received,
                "nickname": nickname,
            }),
            ScriptEffect::Delay {
                frames,
                frames_remaining,
            } => json!({
                "effect": "Delay",
                "frames": frames,
                "frames_remaining": frames_remaining,
            }),
            ScriptEffect::WarpTo { map, x, y } => {
                json!({ "effect": "WarpTo", "map": map, "x": x, "y": y })
            }
            ScriptEffect::AnimateHealingMachine {
                phase,
                frames_remaining,
            } => json!({
                "effect": "AnimateHealingMachine",
                "phase": healing_phase_json(phase),
                "frames_remaining": frames_remaining,
            }),
            ScriptEffect::FadeScreen { fade_type } => {
                json!({ "effect": "FadeScreen", "fade_type": fade_type })
            }
            ScriptEffect::SetJoyIgnore { mask } => {
                json!({ "effect": "SetJoyIgnore", "mask": mask })
            }
            ScriptEffect::ClearJoyIgnore => json!({ "effect": "ClearJoyIgnore" }),
            ScriptEffect::FollowNpc {
                npc_id,
                target_x,
                target_y,
                path,
                player_steps,
                phase,
            } => json!({
                "effect": "FollowNpc",
                "npc_id": npc_id,
                "target_x": target_x,
                "target_y": target_y,
                "path": path,
                "player_steps": player_steps,
                "phase": follow_phase_json(phase),
            }),
            ScriptEffect::ShowPokedexEntry { species, started } => {
                json!({ "effect": "ShowPokedexEntry", "species": species, "started": started })
            }
            ScriptEffect::NamingScreen {
                species,
                naming_state,
                started,
                result_name,
            } => json!({
                "effect": "NamingScreen",
                "species": species,
                "naming_state": naming_state.as_ref().map(|s| format!("{:?}", s)),
                "started": started,
                "result_name": result_name,
            }),
            ScriptEffect::ChoosePartyPokemon { started, result_index } => {
                json!({ "effect": "ChoosePartyPokemon", "started": started, "result_index": result_index })
            }
            ScriptEffect::SetPartyNickname { index, nickname } => {
                json!({ "effect": "SetPartyNickname", "index": index, "nickname": nickname })
            }
            ScriptEffect::Immediate { result } => json!({
                "effect": "Immediate",
                "result": match result {
                    dotzuki_engine_script::CommandResult::Void => json!(null),
                    dotzuki_engine_script::CommandResult::Bool(b) => json!(b),
                    dotzuki_engine_script::CommandResult::Number(n) => json!(n),
                    dotzuki_engine_script::CommandResult::Text(t) => json!(t),
                },
            }),
            ScriptEffect::UnsupportedCommand { name, reason } => {
                json!({ "effect": "UnsupportedCommand", "name": name, "reason": reason })
            }
            ScriptEffect::OpenShop { items } => {
                json!({ "effect": "OpenShop", "items": items })
            }
            ScriptEffect::OpenSlots { lucky } => {
                json!({ "effect": "OpenSlots", "lucky": lucky })
            }            ScriptEffect::ElevatorMenu { floors } => {
                json!({ "effect": "ElevatorMenu", "floors": floors })
            }
            ScriptEffect::FilterBag { item_ids } => {
                json!({ "effect": "FilterBag", "item_ids": item_ids })
            }
            ScriptEffect::ShowEmotionBubble {
                npc_id,
                emotion,
                frames_remaining,
                started,
            } => json!({
                "effect": "ShowEmotionBubble",
                "npc_id": npc_id,
                "emotion": emotion,
                "frames_remaining": frames_remaining,
                "started": started,
            }),
            ScriptEffect::SetNpcPosition { npc_id, x, y } => {
                json!({ "effect": "SetNpcPosition", "npc_id": npc_id, "x": x, "y": y })
            }
            ScriptEffect::SetNpcFrame { npc_id, frame } => {
                json!({ "effect": "SetNpcFrame", "npc_id": npc_id, "frame": frame })
            }
            ScriptEffect::GiveMoney { amount } => {
                json!({ "effect": "GiveMoney", "amount": amount })
            }
            ScriptEffect::TakeMoney { amount } => {
                json!({ "effect": "TakeMoney", "amount": amount })
            }
            ScriptEffect::GiveCoins { amount } => {
                json!({ "effect": "GiveCoins", "amount": amount })
            }
            ScriptEffect::TakeCoins { amount } => {
                json!({ "effect": "TakeCoins", "amount": amount })
            }
            ScriptEffect::DepositDaycare { index } => {
                json!({ "effect": "DepositDaycare", "index": index })
            }
            ScriptEffect::WithdrawDaycare => json!({ "effect": "WithdrawDaycare" }),
            ScriptEffect::PlayCry { species } => {
                json!({ "effect": "PlayCry", "species": species })
            }
            ScriptEffect::GiveBadge { badge } => {
                json!({ "effect": "GiveBadge", "badge": badge })
            }
            ScriptEffect::ReplaceTileBlock { x, y, block_id } => {
                json!({ "effect": "ReplaceTileBlock", "x": x, "y": y, "block_id": block_id })
            }
            ScriptEffect::PlayShipDeparture { started } => {
                json!({ "effect": "PlayShipDeparture", "started": started })
            }
            ScriptEffect::SilphRocketDeparture { state } => {
                json!({ "effect": "SilphRocketDeparture", "state": state })
            }
            // Unit variants.
            ScriptEffect::Heal => json!({ "effect": "Heal" }),
            ScriptEffect::StopMusic => json!({ "effect": "StopMusic" }),
            ScriptEffect::FadeOutMusic => json!({ "effect": "FadeOutMusic" }),
            ScriptEffect::OldManTutorial => json!({ "effect": "OldManTutorial" }),
        }
    }
}

fn follow_phase_json(phase: &FollowNpcPhase) -> serde_json::Value {
    use serde_json::json;
    match phase {
        FollowNpcPhase::StartNpc => json!({ "phase": "StartNpc" }),
        FollowNpcPhase::Following {
            last_npc_x,
            last_npc_y,
            final_push_done,
        } => json!({
            "phase": "Following",
            "last_npc_x": last_npc_x,
            "last_npc_y": last_npc_y,
            "final_push_done": final_push_done,
        }),
        FollowNpcPhase::Done => json!({ "phase": "Done" }),
    }
}

fn healing_phase_json(phase: &HealingMachinePhase) -> serde_json::Value {
    use serde_json::json;
    match phase {
        HealingMachinePhase::FadeOutMusic => json!({ "phase": "FadeOutMusic" }),
        HealingMachinePhase::WaitForFadeOut => json!({ "phase": "WaitForFadeOut" }),
        HealingMachinePhase::HealPartyMember {
            member_index,
            total_members,
        } => json!({
            "phase": "HealPartyMember",
            "member_index": member_index,
            "total_members": total_members,
        }),
        HealingMachinePhase::PlayHealedMusic => json!({ "phase": "PlayHealedMusic" }),
        HealingMachinePhase::FlashSprite { flashes_remaining } => json!({
            "phase": "FlashSprite",
            "flashes_remaining": flashes_remaining,
        }),
        HealingMachinePhase::WaitForMusic => json!({ "phase": "WaitForMusic" }),
        HealingMachinePhase::PostMusicDelay => json!({ "phase": "PostMusicDelay" }),
        HealingMachinePhase::Done => json!({ "phase": "Done" }),
    }
}

pub fn parse_direction(s: &str) -> Option<Direction> {
    match s.to_lowercase().as_str() {
        "up" | "north" => Some(Direction::Up),
        "down" | "south" => Some(Direction::Down),
        "left" | "west" => Some(Direction::Left),
        "right" | "east" => Some(Direction::Right),
        _ => None,
    }
}

pub fn dispatch_command(cmd: &ScriptCommand) -> ScriptEffect {
    dispatch_command_with_names(cmd, "", "", "")
}

pub fn dispatch_command_with_names(
    cmd: &ScriptCommand,
    player_name: &str,
    rival_name: &str,
    starter_name: &str,
) -> ScriptEffect {
    match cmd {
        ScriptCommand::ShowText { text } => {
            let resolved = resolve_placeholders(text, player_name, rival_name, starter_name);
            ScriptEffect::ShowDialogue { text: resolved }
        }
        ScriptCommand::ShowChoice { options } => ScriptEffect::ShowChoice {
            options: options.clone(),
            started: false,
            selected: 0,
        },
        ScriptCommand::GiveItem { item_id, quantity } => ScriptEffect::GiveItem {
            item_id: item_id.clone(),
            quantity: *quantity,
        },
        ScriptCommand::TakeItem { item_id, quantity } => ScriptEffect::TakeItem {
            item_id: item_id.clone(),
            quantity: *quantity,
        },
        ScriptCommand::GiveMonster { species, level } => ScriptEffect::GivePokemon {
            species: species.clone(),
            nickname: None,
            level: *level,
            flow: None,
        },
        ScriptCommand::ShowObject { object_index } => ScriptEffect::ShowObject {
            object_index: *object_index,
        },
        ScriptCommand::HideObject { object_index } => ScriptEffect::HideObject {
            object_index: *object_index,
        },
        ScriptCommand::ShowObjectByName { toggle_id } => ScriptEffect::ShowObjectByName {
            toggle_id: toggle_id.clone(),
        },
        ScriptCommand::HideObjectByName { toggle_id } => ScriptEffect::HideObjectByName {
            toggle_id: toggle_id.clone(),
        },
        ScriptCommand::MoveNpc { npc_id, path } => ScriptEffect::MoveNpc {
            npc_id: npc_id.clone(),
            path: path.clone(),
            started: false,
        },
        ScriptCommand::StartNpcMove { npc_id, path } => ScriptEffect::StartNpcMove {
            npc_id: npc_id.clone(),
            path: path.clone(),
        },
        ScriptCommand::AwaitNpcMove { npc_id } => ScriptEffect::AwaitNpcMove {
            npc_id: npc_id.clone(),
        },
        ScriptCommand::MovePlayer { path } => ScriptEffect::MovePlayer {
            path: path.clone(),
            started: false,
        },
        ScriptCommand::MovePlayerRelative { steps } => ScriptEffect::MovePlayerRelative {
            steps: steps.clone(),
            started: false,
        },
        ScriptCommand::MoveNpcTo { npc_id, x, y } => ScriptEffect::MoveNpcTo {
            npc_id: npc_id.clone(),
            x: *x,
            y: *y,
            started: false,
        },
        ScriptCommand::StartNpcMoveTo { npc_id, x, y } => ScriptEffect::StartNpcMoveTo {
            npc_id: npc_id.clone(),
            x: *x,
            y: *y,
        },
        ScriptCommand::MovePlayerTo { x, y } => ScriptEffect::MovePlayerTo {
            x: *x,
            y: *y,
            started: false,
        },
        ScriptCommand::FaceNpc { npc_id, direction } => ScriptEffect::FaceNpc {
            npc_id: npc_id.clone(),
            direction: parse_direction(direction).unwrap_or(Direction::Down),
        },
        ScriptCommand::FacePlayer { direction } => ScriptEffect::FacePlayer {
            direction: parse_direction(direction).unwrap_or(Direction::Down),
        },
        ScriptCommand::PlayMusic { music_id } => ScriptEffect::PlayMusic {
            music_id: music_id.clone(),
        },
        ScriptCommand::PlaySound { sound_id } => ScriptEffect::PlaySound {
            sound_id: sound_id.clone(),
        },
        ScriptCommand::StopMusic => ScriptEffect::StopMusic,
        ScriptCommand::FadeOutMusic => ScriptEffect::FadeOutMusic,
        ScriptCommand::StartBattle { trainer_id } => ScriptEffect::StartBattle {
            trainer_id: trainer_id.clone(),
            rival_triplet_base: None,
            end_battle_text: None,
        },
        ScriptCommand::StartWildBattle { species, level } => ScriptEffect::StartWildBattle {
            species: species.clone(),
            level: *level,
        },
        // Game-defined commands: formerly dedicated engine variants, now
        // `ScriptCommand::Custom` dispatched by JS verb name (registered in
        // `pokered-data::script_api`).
        ScriptCommand::Custom { name, args } => {
            let mut effect = dispatch_custom(name, args);
            if let ScriptEffect::ShowItemDialogue { text, .. } | ScriptEffect::PrintFieldText { text, .. } = &mut effect {
                *text = resolve_placeholders(text, player_name, rival_name, starter_name);
            }
            if let ScriptEffect::StartBattle { end_battle_text: Some(text), .. } = &mut effect {
                *text = resolve_placeholders(text, player_name, rival_name, starter_name);
            }
            effect
        },
        ScriptCommand::Delay { frames } => ScriptEffect::Delay {
            frames: *frames,
            frames_remaining: *frames,
        },
        ScriptCommand::WarpTo { map, x, y } => ScriptEffect::WarpTo {
            map: map.clone(),
            x: *x,
            y: *y,
        },
        ScriptCommand::Heal => ScriptEffect::Heal,
        ScriptCommand::FadeScreen { fade_type } => ScriptEffect::FadeScreen {
            fade_type: fade_type.clone(),
        },
        ScriptCommand::SetJoyIgnore { mask } => ScriptEffect::SetJoyIgnore { mask: *mask },
        ScriptCommand::ClearJoyIgnore => ScriptEffect::ClearJoyIgnore,
        ScriptCommand::FollowNpc {
            npc_id,
            target_x,
            target_y,
        } => ScriptEffect::FollowNpc {
            npc_id: npc_id.clone(),
            target_x: *target_x,
            target_y: *target_y,
            path: None,
            player_steps: None,
            phase: FollowNpcPhase::StartNpc,
        },
        ScriptCommand::OpenShop { items } => ScriptEffect::OpenShop {
            items: items.clone(),
        },
        ScriptCommand::ShowEmotionBubble { npc_id, emotion } => ScriptEffect::ShowEmotionBubble {
            npc_id: npc_id.clone(),
            emotion: emotion.clone(),
            frames_remaining: 60,
            started: false,
        },
        ScriptCommand::SetNpcPosition { npc_id, x, y } => ScriptEffect::SetNpcPosition {
            npc_id: npc_id.clone(),
            x: *x,
            y: *y,
        },
        ScriptCommand::SetNpcFrame { npc_id, frame } => ScriptEffect::SetNpcFrame {
            npc_id: npc_id.clone(),
            frame: *frame,
        },
        ScriptCommand::GiveMoney { amount } => ScriptEffect::GiveMoney { amount: *amount },
        ScriptCommand::TakeMoney { amount } => ScriptEffect::TakeMoney { amount: *amount },
        ScriptCommand::PlayCry { species } => ScriptEffect::PlayCry {
            species: species.clone(),
        },
        ScriptCommand::GiveBadge { badge } => ScriptEffect::GiveBadge { badge: *badge },
        // Sync flag ops never reach dispatch — defensive fallback.
        ScriptCommand::SetFlag { .. }
        | ScriptCommand::ResetFlag { .. }
        | ScriptCommand::CheckFlag { .. } => ScriptEffect::Immediate {
            result: CommandResult::Void,
        },
        // These commands belong to host capabilities pokered does not expose.
        ScriptCommand::ShowScene { .. } => unsupported("showScene"),
        ScriptCommand::HideScene { .. } => unsupported("hideScene"),
        ScriptCommand::UpdateUI { .. } => unsupported("updateUI"),
        // dotzuki-runner battle weather — registered only by the jrpg runner's
        // scene engine; never produced by pokered scripts.
        ScriptCommand::SetWeather { .. } => unsupported("setWeather"),
    }
}

/// Dispatch a game-defined [`ScriptCommand::Custom`] to the matching
/// [`ScriptEffect`] by its JS verb name. `args` are the raw JSON values the
/// registrar passed through (see `pokered-data::script_api`); unknown names
/// become an explicit unsupported-capability effect.
fn dispatch_custom(name: &str, args: &[Value]) -> ScriptEffect {
    let command = match PokemonScriptCommand::from_custom(name, args) {
        Ok(command) => command,
        Err(error) => return unsupported_with_reason(name, error),
    };
    match command {
        PokemonScriptCommand::WalkTogether { npc_id, path, player_steps } => {
            let (target_x,target_y)=*path.last().expect("validated nonempty route");
            ScriptEffect::FollowNpc { npc_id, target_x, target_y, path: Some(path), player_steps: Some(player_steps), phase: FollowNpcPhase::StartNpc }
        },
        PokemonScriptCommand::FollowNpcPath { npc_id, path } => {
            let (target_x, target_y) = *path.last().expect("validated nonempty escort route");
            ScriptEffect::FollowNpc { npc_id, target_x, target_y, path: Some(path), player_steps: None, phase: FollowNpcPhase::StartNpc }
        },
        PokemonScriptCommand::FieldCry { text, species } => ScriptEffect::FieldCry {
            state: super::field_cry::FieldCryState::new(text, species),
        },
        PokemonScriptCommand::PrintTextCry { text, species } => ScriptEffect::FieldCry {
            state: super::field_cry::FieldCryState::inner_print_text(text, species),
        },
        PokemonScriptCommand::BikeShopOffer { texts } => ScriptEffect::BikeShopOffer {
            state: super::bike_shop::BikeShopOfferState::new(texts),
        },
        PokemonScriptCommand::PrintFieldText { text, retain_window, prompt, leading_sound } => ScriptEffect::PrintFieldText {
            text,
            retain_window,
            prompt,
            prompt_frames_remaining: 0,
            prompt_last_buttons: None,
            leading_sound,
            leading_sound_started: false,
        },
        PokemonScriptCommand::ShowItemDialogue { text, sound_id } => ScriptEffect::ShowItemDialogue { text, sound_id, sound_started: false },
        PokemonScriptCommand::PokemonMenu { options, species } => ScriptEffect::ReadingMenu {
            menu: super::script_interactions::ReadingMenu::pokemon(options, species),
        },
        PokemonScriptCommand::ReadingMenu { options, texts } => ScriptEffect::ReadingMenu { menu: super::script_interactions::ReadingMenu::new(options, texts) },
        PokemonScriptCommand::ShowMoneyBox { amount } => ScriptEffect::ShowMoneyBox { amount },
        PokemonScriptCommand::ShowCoinBox { amount } => ScriptEffect::ShowCoinBox { amount },
        PokemonScriptCommand::VendingDelivery => ScriptEffect::VendingDelivery { frames_elapsed: 0 },
        PokemonScriptCommand::OldManTutorial => ScriptEffect::OldManTutorial,
        PokemonScriptCommand::TradePokemon { offered, received, nickname } => {
            ScriptEffect::TradePokemon { offered, received, nickname }
        }
        PokemonScriptCommand::AnimateHealingMachine => ScriptEffect::AnimateHealingMachine {
            phase: HealingMachinePhase::FadeOutMusic,
            frames_remaining: 0,
        },
        PokemonScriptCommand::ShowPokedexEntry { species } => ScriptEffect::ShowPokedexEntry {
            species,
            started: false,
        },
        PokemonScriptCommand::OpenNamingScreen { species } => ScriptEffect::NamingScreen {
            species,
            naming_state: None,
            started: false,
            result_name: None,
        },
        PokemonScriptCommand::ChoosePartyPokemon => ScriptEffect::ChoosePartyPokemon {
            started: false,
            result_index: None,
        },
        PokemonScriptCommand::SetPartyNickname { index, nickname } => ScriptEffect::SetPartyNickname {
            index,
            nickname,
        },
        PokemonScriptCommand::StartBattleSet { trainer_id, rival_triplet_base } => ScriptEffect::StartBattle {
            trainer_id,
            rival_triplet_base: Some(rival_triplet_base),
            end_battle_text: None,
        },
        PokemonScriptCommand::StartBattleWithText { trainer_id, end_battle_text } => ScriptEffect::StartBattle {
            trainer_id,
            rival_triplet_base: None,
            end_battle_text: Some(end_battle_text),
        },
        PokemonScriptCommand::OpenSlots { lucky } => ScriptEffect::OpenSlots { lucky },
        PokemonScriptCommand::ElevatorMenu { floors } => ScriptEffect::ElevatorMenu { floors },
        PokemonScriptCommand::FilterBag { item_ids } => ScriptEffect::FilterBag { item_ids },
        PokemonScriptCommand::ShowDiploma => ScriptEffect::ShowDiploma,
        PokemonScriptCommand::OpenPc => ScriptEffect::OpenPc {
            kind: "center".to_string(),
        },
        PokemonScriptCommand::OpenItemPc => ScriptEffect::OpenPc {
            kind: "items".to_string(),
        },
        PokemonScriptCommand::OpenBillsPc => ScriptEffect::OpenPc {
            kind: "bills".to_string(),
        },
        PokemonScriptCommand::LinkStart => ScriptEffect::LinkStart,
        PokemonScriptCommand::GiveCoins { amount } => ScriptEffect::GiveCoins { amount },
        PokemonScriptCommand::TakeCoins { amount } => ScriptEffect::TakeCoins { amount },
        PokemonScriptCommand::DepositDaycare { index } => ScriptEffect::DepositDaycare { index },
        PokemonScriptCommand::WithdrawDaycare => ScriptEffect::WithdrawDaycare,
        PokemonScriptCommand::ReplaceTileBlock { x, y, block_id } => ScriptEffect::ReplaceTileBlock {
            x,
            y,
            block_id,
        },
        PokemonScriptCommand::PlayShipDeparture => ScriptEffect::PlayShipDeparture { started: false },
        PokemonScriptCommand::SilphRocketDeparture => ScriptEffect::SilphRocketDeparture {
            state: super::silph_departure::SilphRocketDepartureState::new(),
        },
        PokemonScriptCommand::EnterHallOfFame => ScriptEffect::HallOfFameCeremony,
        PokemonScriptCommand::WaitMusic => ScriptEffect::WaitMusic,
        PokemonScriptCommand::WaitSound => ScriptEffect::WaitSound,
        PokemonScriptCommand::FinishFieldText => ScriptEffect::FinishFieldText,
    }
}

fn unsupported(name: &str) -> ScriptEffect {
    ScriptEffect::UnsupportedCommand {
        name: name.to_string(),
        reason: None,
    }
}

fn unsupported_with_reason(name: &str, reason: String) -> ScriptEffect {
    ScriptEffect::UnsupportedCommand {
        name: name.to_string(),
        reason: Some(reason),
    }
}

fn resolve_placeholders(text: &str, player_name: &str, rival_name: &str, starter_name: &str) -> String {
    text.replace("<PLAYER>", player_name)
        .replace("<RIVAL>", rival_name)
        .replace("<STARTER>", starter_name)
}

pub fn text_to_dialogue(text: &str) -> BedroomDialogue {
    text_to_dialogue_with_names(text, &[])
}

pub fn text_to_dialogue_with_names(text: &str, names: &[&str]) -> BedroomDialogue {
    if text.trim().is_empty() {
        return BedroomDialogue::from_pages(Vec::new());
    }
    if pokered_data::dialogue_layout::contains_chinese(text) {
        return BedroomDialogue::from_pages(crate::text::zh_dialogue::paginate(text, names));
    }
    BedroomDialogue::from_message(text)
}

pub fn map_id_to_script_key(map_id: MapId) -> String {
    format!("{:?}", map_id)
}

/// Numeric ids are the NPC's text id (map.json `textId` == script_config.json
/// `npcs[].id`, the same number `@trigger(npc = N)` binds), NOT the 0-based
/// object index — the Pokécenter nurse is the first object on her map but the
/// scripts face her with `faceNpc("1", ...)`.
pub fn find_npc_index_by_id(npcs: &[NpcRuntimeState], npc_id: &str) -> Option<usize> {
    if let Ok(text_id) = npc_id.parse::<u8>() {
        return npcs.iter().position(|n| n.text_id == text_id);
    }
    None
}

#[cfg(test)]
mod name_rater_tests {
    use super::*;

    #[test]
    fn choose_party_pokemon_maps_to_effect() {
        let eff = dispatch_command(&ScriptCommand::Custom {
            name: "choosePartyPokemon".to_string(),
            args: vec![],
        });
        match eff {
            ScriptEffect::ChoosePartyPokemon {
                started,
                result_index,
            } => {
                assert!(!started);
                assert_eq!(result_index, None);
            }
            other => panic!("expected ChoosePartyPokemon, got {other:?}"),
        }
    }

    #[test]
    fn set_party_nickname_maps_to_effect() {
        let eff = dispatch_command(&ScriptCommand::Custom {
            name: "setPartyNickname".to_string(),
            args: vec![serde_json::json!(2), serde_json::json!("SPARKY")],
        });
        match eff {
            ScriptEffect::SetPartyNickname { index, nickname } => {
                assert_eq!(index, 2);
                assert_eq!(nickname, "SPARKY");
            }
            other => panic!("expected SetPartyNickname, got {other:?}"),
        }
    }

    #[test]
    fn unknown_custom_command_is_not_silently_accepted() {
        let eff = dispatch_command(&ScriptCommand::Custom {
            name: "notRegistered".to_string(),
            args: vec![],
        });
        assert!(matches!(
            eff,
            ScriptEffect::UnsupportedCommand { ref name, .. } if name == "notRegistered"
        ));
    }

    #[test]
    fn unsupported_engine_command_is_observable() {
        let eff = dispatch_command(&ScriptCommand::SetWeather {
            weather: Some("rain".to_string()),
        });
        assert!(matches!(
            eff,
            ScriptEffect::UnsupportedCommand { ref name, .. } if name == "setWeather"
        ));
    }
}

#[cfg(test)]
mod script_effect_json_tests {
    use super::*;

    #[test]
    fn effect_json_includes_progress_fields() {
        // Delay countdown is the key driver-facing field for cutscene waits.
        let j = ScriptEffect::Delay {
            frames: 60,
            frames_remaining: 12,
        }
        .to_debug_json();
        assert_eq!(j["effect"], "Delay");
        assert_eq!(j["frames"], 60);
        assert_eq!(j["frames_remaining"], 12);

        // Choice menu exposes the cursor so a driver can navigate blindly.
        let j = ScriptEffect::ShowChoice {
            options: vec!["YES".to_string(), "NO".to_string()],
            started: true,
            selected: 1,
        }
        .to_debug_json();
        assert_eq!(j["effect"], "ShowChoice");
        assert_eq!(j["selected"], 1);
        assert_eq!(j["options"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn effect_json_follow_phase_and_immediate_result() {
        let j = ScriptEffect::FollowNpc {
            npc_id: "PALLET_TOWN_OBJ_1".to_string(),
            target_x: 11,
            target_y: 2,
            path: None,
            player_steps: None,
            phase: FollowNpcPhase::Following {
                last_npc_x: 10,
                last_npc_y: 3,
                final_push_done: true,
            },
        }
        .to_debug_json();
        assert_eq!(j["effect"], "FollowNpc");
        assert_eq!(j["phase"]["phase"], "Following");
        assert_eq!(j["phase"]["final_push_done"], true);

        let j = ScriptEffect::Immediate {
            result: dotzuki_engine_script::CommandResult::Bool(true),
        }
        .to_debug_json();
        assert_eq!(j["result"], true);
    }
}

#[cfg(test)]
mod print_field_text_contract_tests {
    use super::*;
    #[test]
    fn inner_print_uses_the_same_name_resolution_as_outer_text_and_restores_its_mode() {
        let text="<PLAYER> asks <RIVAL> about <STARTER>.".to_string();
        let command=PokemonScriptCommand::PrintFieldText {text:text.clone(),retain_window:false, prompt: pokered_data::script_command::FieldTextPrompt::Done, leading_sound: None}.into_script_command();
        let inner=dispatch_command_with_names(&command,"RED","BLUE","BULBASAUR");
        let outer=dispatch_command_with_names(&ScriptCommand::ShowText {text},"RED","BLUE","BULBASAUR");
        let ScriptEffect::ShowDialogue {text:expected}=outer else {panic!()};
        let raw=serde_json::to_string(&inner).unwrap();
        let restored:ScriptEffect=serde_json::from_str(&raw).unwrap();
        let ScriptEffect::PrintFieldText {text:actual,retain_window:false, prompt: pokered_data::script_command::FieldTextPrompt::Done, ..}=restored else {panic!("lost inner return mode")};
        assert_eq!(actual,expected);
        assert_eq!(actual,"RED asks BLUE about BULBASAUR.");
    }
}

#[cfg(test)]
mod field_sound_window_snapshot_tests {
    use super::*;

    #[test]
    fn legacy_inner_text_effect_does_not_acquire_window_ownership() {
        let old = serde_json::json!({"PrintFieldText": {"text": "old"}});
        let effect: ScriptEffect = serde_json::from_value(old).unwrap();
        assert!(matches!(effect, ScriptEffect::PrintFieldText {
            retain_window: false,
            prompt: pokered_data::script_command::FieldTextPrompt::Done,
            prompt_frames_remaining: 0, prompt_last_buttons: None,
            leading_sound: None, leading_sound_started: false, ..
        }));
        let wait = dispatch_command(&PokemonScriptCommand::WaitSound.into_script_command());
        assert_eq!(wait.to_debug_json()["effect"], "WaitSound");
        assert!(matches!(serde_json::from_str::<ScriptEffect>(&serde_json::to_string(&wait).unwrap()).unwrap(), ScriptEffect::WaitSound));
    }


    #[test]
    fn retained_prompt_snapshot_keeps_its_partial_protected_delay_and_button_sample() {
        let original = ScriptEffect::PrintFieldText {
            text: "last page".into(), retain_window: true,
            prompt: pokered_data::script_command::FieldTextPrompt::Protected,
            prompt_frames_remaining: 2, prompt_last_buttons: Some(1),
            leading_sound: None, leading_sound_started: false,
        };
        for effect in [
            original,
            ScriptEffect::PrintFieldText {
                text: "correct".into(), retain_window: true,
                prompt: pokered_data::script_command::FieldTextPrompt::Button,
                prompt_frames_remaining: 0, prompt_last_buttons: None,
                leading_sound: Some("SFX_GET_ITEM_1".into()), leading_sound_started: false,
            },
            ScriptEffect::PrintFieldText {
                text: "correct".into(), retain_window: true,
                prompt: pokered_data::script_command::FieldTextPrompt::Button,
                prompt_frames_remaining: 0, prompt_last_buttons: None,
                leading_sound: Some("SFX_GET_ITEM_1".into()), leading_sound_started: true,
            },
        ] {
            let value = serde_json::to_value(&effect).unwrap();
            let restored: ScriptEffect = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(restored).unwrap(), value);
        }
        let finish = dispatch_command(&PokemonScriptCommand::FinishFieldText.into_script_command());
        assert!(matches!(finish, ScriptEffect::FinishFieldText));
        assert_eq!(finish.to_debug_json()["effect"], "FinishFieldText");
    }
    #[test]
    fn explicit_battle_quote_does_not_change_old_api_or_legacy_effect_defaults() {
        let command = PokemonScriptCommand::StartBattleWithText {
            trainer_id: "OPP_SUPER_NERD9".into(), end_battle_text: "<PLAYER> won!".into(),
        }.into_script_command();
        let effect = dispatch_command_with_names(&command, "RED", "BLUE", "BULBASAUR");
        assert!(matches!(&effect, ScriptEffect::StartBattle {
            trainer_id, rival_triplet_base: None, end_battle_text: Some(text),
        } if trainer_id == "OPP_SUPER_NERD9" && text == "RED won!"));
        let restored: ScriptEffect = serde_json::from_value(serde_json::to_value(effect).unwrap()).unwrap();
        assert!(matches!(restored, ScriptEffect::StartBattle { end_battle_text: Some(_), .. }));
        let old: ScriptEffect = serde_json::from_value(serde_json::json!({
            "StartBattle": {"trainer_id":"OPP_BLAINE1", "rival_triplet_base":null}
        })).unwrap();
        assert!(matches!(old, ScriptEffect::StartBattle { end_battle_text: None, .. }));
        assert!(matches!(dispatch_command(&ScriptCommand::StartBattle { trainer_id: "OPP_BLAINE1".into() }),
            ScriptEffect::StartBattle { end_battle_text: None, rival_triplet_base: None, .. }));
        assert!(matches!(dispatch_command(&PokemonScriptCommand::StartBattleSet {
            trainer_id: "OPP_RIVAL2".into(), rival_triplet_base: 6,
        }.into_script_command()), ScriptEffect::StartBattle {
            end_battle_text: None, rival_triplet_base: Some(6), ..
        }));
    }

}
