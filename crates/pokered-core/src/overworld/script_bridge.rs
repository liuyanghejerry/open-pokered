use pokered_data::maps::MapId;
use dotzuki_engine_script::{CommandResult, ScriptCommand};
use serde_json::Value;

use super::npc_movement::NpcRuntimeState;
use super::{BedroomDialogue, DialoguePage, Direction};
use crate::naming_screen::NamingScreenState;

pub use dotzuki_engine_script::config::{CoordEventBinding, NpcBinding, SignBinding};

#[derive(Debug, Clone)]
pub struct PendingChoice {
    pub options: Vec<String>,
    pub selected: u32,
}

impl PendingChoice {
    pub fn new(options: Vec<String>) -> Self {
        Self {
            options,
            selected: 0,
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

#[derive(Debug, Clone)]
pub enum ScriptEffect {
    ShowDialogue {
        text: String,
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
    },
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
    StopMusic,
    FadeOutMusic,
    StartBattle {
        trainer_id: String,
        rival_triplet_base: Option<u8>,
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
    OpenShop {
        items: Vec<String>,
    },
    /// Open the Game Corner slot-machine minigame (`lucky` = higher-odds).
    OpenSlots {
        lucky: bool,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealingMachinePhase {
    FadeOutMusic,
    WaitForFadeOut,
    HealPartyMember { member_index: u8, total_members: u8 },
    PlayHealedMusic,
    FlashSprite { flashes_remaining: u8 },
    WaitForMusic,
    Done,
}

impl ScriptEffect {
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
            ScriptEffect::GivePokemon {
                species,
                nickname,
                level,
            } => json!({
                "effect": "GivePokemon",
                "species": species,
                "nickname": nickname,
                "level": level,
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
            ScriptEffect::PlaySound { sound_id } => {
                json!({ "effect": "PlaySound", "sound_id": sound_id })
            }
            ScriptEffect::StartBattle {
                trainer_id,
                rival_triplet_base,
            } => json!({
                "effect": "StartBattle",
                "trainer_id": trainer_id,
                "rival_triplet_base": rival_triplet_base,
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
                phase,
            } => json!({
                "effect": "FollowNpc",
                "npc_id": npc_id,
                "target_x": target_x,
                "target_y": target_y,
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
            ScriptEffect::OpenShop { items } => {
                json!({ "effect": "OpenShop", "items": items })
            }
            ScriptEffect::OpenSlots { lucky } => {
                json!({ "effect": "OpenSlots", "lucky": lucky })
            }
            ScriptEffect::ElevatorMenu { floors } => {
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
        },
        ScriptCommand::StartWildBattle { species, level } => ScriptEffect::StartWildBattle {
            species: species.clone(),
            level: *level,
        },
        // Game-defined commands: formerly dedicated engine variants, now
        // `ScriptCommand::Custom` dispatched by JS verb name (registered in
        // `pokered-data::script_api`).
        ScriptCommand::Custom { name, args } => dispatch_custom(name, args),
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
        // dotzuki-engine UI/scene commands — not used by pokered, no-op.
        ScriptCommand::ShowScene { .. }
        | ScriptCommand::HideScene { .. }
        | ScriptCommand::UpdateUI { .. } => ScriptEffect::Immediate {
            result: CommandResult::Void,
        },
        // dotzuki-runner battle weather — registered only by the jrpg runner's
        // scene engine; never produced by pokered scripts. No-op.
        ScriptCommand::SetWeather { .. } => ScriptEffect::Immediate {
            result: CommandResult::Void,
        },
    }
}

/// Dispatch a game-defined [`ScriptCommand::Custom`] to the matching
/// [`ScriptEffect`] by its JS verb name. `args` are the raw JSON values the
/// registrar passed through (see `pokered-data::script_api`); unknown names
/// are a defensive no-op (Void), like the unhandled engine commands above.
fn dispatch_custom(name: &str, args: &[Value]) -> ScriptEffect {
    match name {
        "oldManTutorial" => ScriptEffect::OldManTutorial,
        "tradePokemon" => ScriptEffect::TradePokemon {
            offered: custom_str(args, 0),
            received: custom_str(args, 1),
            nickname: custom_str(args, 2),
        },
        "animateHealingMachine" => ScriptEffect::AnimateHealingMachine {
            phase: HealingMachinePhase::FadeOutMusic,
            frames_remaining: 0,
        },
        "showPokedexEntry" => ScriptEffect::ShowPokedexEntry {
            species: custom_str(args, 0),
            started: false,
        },
        "openNamingScreen" => ScriptEffect::NamingScreen {
            species: custom_str(args, 0),
            naming_state: None,
            started: false,
            result_name: None,
        },
        "choosePartyPokemon" => ScriptEffect::ChoosePartyPokemon {
            started: false,
            result_index: None,
        },
        "setPartyNickname" => ScriptEffect::SetPartyNickname {
            index: custom_u64(args, 0) as u8,
            nickname: custom_str(args, 1),
        },
        "startBattleSet" => ScriptEffect::StartBattle {
            trainer_id: custom_str(args, 0),
            rival_triplet_base: Some(custom_u64(args, 1) as u8),
        },
        "openSlots" => ScriptEffect::OpenSlots {
            lucky: custom_bool(args, 0),
        },
        "elevatorMenu" => ScriptEffect::ElevatorMenu {
            floors: json_string_vec(args.first()),
        },
        "filterBag" => ScriptEffect::FilterBag {
            item_ids: json_string_vec(args.first()),
        },
        "showDiploma" => ScriptEffect::ShowDiploma,
        "openPC" => ScriptEffect::OpenPc {
            kind: "center".to_string(),
        },
        "openItemPC" => ScriptEffect::OpenPc {
            kind: "items".to_string(),
        },
        "openBillsPC" => ScriptEffect::OpenPc {
            kind: "bills".to_string(),
        },
        "linkStart" => ScriptEffect::LinkStart,
        "giveCoins" => ScriptEffect::GiveCoins {
            amount: custom_u64(args, 0) as u16,
        },
        "takeCoins" => ScriptEffect::TakeCoins {
            amount: custom_u64(args, 0) as u16,
        },
        "depositDaycare" => ScriptEffect::DepositDaycare {
            index: custom_u64(args, 0) as u8,
        },
        "withdrawDaycare" => ScriptEffect::WithdrawDaycare,
        "replaceTileBlock" => ScriptEffect::ReplaceTileBlock {
            x: custom_u64(args, 0) as u8,
            y: custom_u64(args, 1) as u8,
            block_id: custom_u64(args, 2) as u8,
        },
        "playShipDeparture" => ScriptEffect::PlayShipDeparture { started: false },
        "enterHallOfFame" => ScriptEffect::HallOfFameCeremony,
        _ => ScriptEffect::Immediate {
            result: CommandResult::Void,
        },
    }
}

/// Read the `i`-th `Custom` argument as a string ("" when missing or not a
/// string).
fn custom_str(args: &[Value], i: usize) -> String {
    args.get(i).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

/// Read the `i`-th `Custom` argument as an unsigned integer (0 when missing
/// or not an integer).
fn custom_u64(args: &[Value], i: usize) -> u64 {
    args.get(i).and_then(|v| v.as_u64()).unwrap_or(0)
}

/// Read the `i`-th `Custom` argument as a boolean (false when missing or not
/// a boolean).
fn custom_bool(args: &[Value], i: usize) -> bool {
    args.get(i).and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Convert a `Custom` array argument into a `Vec<String>` (empty when absent
/// or not an array of strings).
fn json_string_vec(arg: Option<&Value>) -> Vec<String> {
    arg.and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn resolve_placeholders(text: &str, player_name: &str, rival_name: &str, starter_name: &str) -> String {
    text.replace("<PLAYER>", player_name)
        .replace("<RIVAL>", rival_name)
        .replace("<STARTER>", starter_name)
}

pub fn text_to_dialogue(text: &str) -> BedroomDialogue {
    let lines: Vec<&str> = text.lines().collect();
    let mut pages = Vec::new();

    if lines.is_empty() {
        pages.push(DialoguePage {
            line1: Box::leak(text.to_string().into_boxed_str()),
            line2: "",
        });
    } else {
        let mut i = 0;
        while i < lines.len() {
            let line1 = Box::leak(lines[i].to_string().into_boxed_str()) as &'static str;
            let line2 = if i + 1 < lines.len() {
                i += 1;
                Box::leak(lines[i].to_string().into_boxed_str()) as &'static str
            } else {
                ""
            };
            pages.push(DialoguePage { line1, line2 });
            i += 1;
        }
    }

    BedroomDialogue::from_pages(pages)
}

pub fn map_id_to_script_key(map_id: MapId) -> String {
    format!("{:?}", map_id)
}

pub fn find_npc_index_by_id(npcs: &[NpcRuntimeState], npc_id: &str) -> Option<usize> {
    if let Ok(idx) = npc_id.parse::<u8>() {
        return npcs.iter().position(|n| n.npc_index == idx);
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
