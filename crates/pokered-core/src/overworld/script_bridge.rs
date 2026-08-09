use pokered_data::maps::MapId;
use jrpg_engine_script::{CommandResult, ScriptCommand};

use super::npc_movement::NpcRuntimeState;
use super::{BedroomDialogue, DialoguePage, Direction};
use crate::naming_screen::NamingScreenState;

pub use jrpg_engine_script::config::{CoordEventBinding, NpcBinding, SignBinding};

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
    /// Open the PC storage screen (see `ScriptCommand::OpenPc` for `kind`).
    /// Instant effect: the app opens the screen; the script runs on.
    OpenPc {
        kind: String,
    },
    /// Start the Cable Club link flow (see `ScriptCommand::LinkStart`).
    /// Instant effect: flags `link_start_requested`; the app layer (which
    /// owns the link session) drives the request/accept flow.
    LinkStart,
    /// Record the party and play the Hall of Fame ceremony + credits
    /// (see `ScriptCommand::EnterHallOfFame`). Instant effect: the app runs
    /// the ceremony and resets to the title screen; the script runs on.
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
        ScriptCommand::GivePokemon { species, level } => ScriptEffect::GivePokemon {
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
        },
        ScriptCommand::StartWildBattle { species, level } => ScriptEffect::StartWildBattle {
            species: species.clone(),
            level: *level,
        },
        ScriptCommand::OldManTutorial => ScriptEffect::OldManTutorial,
        ScriptCommand::TradePokemon {
            offered,
            received,
            nickname,
        } => ScriptEffect::TradePokemon {
            offered: offered.clone(),
            received: received.clone(),
            nickname: nickname.clone(),
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
        ScriptCommand::AnimateHealingMachine => ScriptEffect::AnimateHealingMachine {
            phase: HealingMachinePhase::FadeOutMusic,
            frames_remaining: 0,
        },
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
        ScriptCommand::ShowPokedexEntry { species } => ScriptEffect::ShowPokedexEntry {
            species: species.clone(),
            started: false,
        },
        ScriptCommand::OpenNamingScreen { species } => ScriptEffect::NamingScreen {
            species: species.clone(),
            naming_state: None,
            started: false,
            result_name: None,
        },
        ScriptCommand::ChoosePartyPokemon => ScriptEffect::ChoosePartyPokemon {
            started: false,
            result_index: None,
        },
        ScriptCommand::SetPartyNickname { index, nickname } => ScriptEffect::SetPartyNickname {
            index: *index,
            nickname: nickname.clone(),
        },
        ScriptCommand::OpenShop { items } => ScriptEffect::OpenShop {
            items: items.clone(),
        },
        ScriptCommand::OpenSlots { lucky } => ScriptEffect::OpenSlots { lucky: *lucky },
        ScriptCommand::ElevatorMenu { floors } => ScriptEffect::ElevatorMenu {
            floors: floors.clone(),
        },
        ScriptCommand::FilterBag { item_ids } => ScriptEffect::FilterBag {
            item_ids: item_ids.clone(),
        },
        ScriptCommand::ShowDiploma => ScriptEffect::ShowDiploma,
        ScriptCommand::OpenPc { kind } => ScriptEffect::OpenPc { kind: kind.clone() },
        ScriptCommand::LinkStart => ScriptEffect::LinkStart,
        ScriptCommand::EnterHallOfFame => ScriptEffect::HallOfFameCeremony,
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
        ScriptCommand::GiveCoins { amount } => ScriptEffect::GiveCoins { amount: *amount },
        ScriptCommand::TakeCoins { amount } => ScriptEffect::TakeCoins { amount: *amount },
        ScriptCommand::DepositDaycare { index } => ScriptEffect::DepositDaycare { index: *index },
        ScriptCommand::WithdrawDaycare => ScriptEffect::WithdrawDaycare,
        ScriptCommand::PlayCry { species } => ScriptEffect::PlayCry {
            species: species.clone(),
        },
        ScriptCommand::GiveBadge { badge } => ScriptEffect::GiveBadge { badge: *badge },
        ScriptCommand::ReplaceTileBlock { x, y, block_id } => ScriptEffect::ReplaceTileBlock {
            x: *x,
            y: *y,
            block_id: *block_id,
        },
        ScriptCommand::PlayShipDeparture => ScriptEffect::PlayShipDeparture { started: false },
        // Sync flag ops never reach dispatch — defensive fallback.
        ScriptCommand::SetFlag { .. }
        | ScriptCommand::ResetFlag { .. }
        | ScriptCommand::CheckFlag { .. } => ScriptEffect::Immediate {
            result: CommandResult::Void,
        },
        // jrpg-engine UI/scene commands — not used by pokered, no-op.
        ScriptCommand::ShowScene { .. }
        | ScriptCommand::HideScene { .. }
        | ScriptCommand::UpdateUI { .. } => ScriptEffect::Immediate {
            result: CommandResult::Void,
        },
        // jrpg-runner battle weather — registered only by the jrpg runner's
        // scene engine; never produced by pokered scripts. No-op.
        ScriptCommand::SetWeather { .. } => ScriptEffect::Immediate {
            result: CommandResult::Void,
        },
    }
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
        let eff = dispatch_command(&ScriptCommand::ChoosePartyPokemon);
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
        let eff = dispatch_command(&ScriptCommand::SetPartyNickname {
            index: 2,
            nickname: "SPARKY".to_string(),
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
