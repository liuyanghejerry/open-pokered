use pokered_data::event_flags::EventFlag;
use pokered_data::maps::MapId;

use super::event_flags::EventFlags;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainerHeader {
    pub event_flag: EventFlag,
    pub sight_range: u8,
    pub before_battle_text_id: u8,
    pub end_battle_text_id: u8,
    pub after_battle_text_id: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrainerBattleState {
    NotEngaged,
    Spotted,
    WalkingToPlayer,
    ShowBeforeBattleText,
    InBattle,
    ShowEndBattleText,
    Defeated,
}

#[derive(Debug, Clone)]
pub struct TrainerEncounter {
    pub map: MapId,
    pub trainer_index: u8,
    pub npc_index: u8,
    pub state: TrainerBattleState,
    /// Frames remaining in current state
    pub wait_frames: u8,
    /// Direction to walk toward player (dx, dy)
    pub walk_dx: i8,
    pub walk_dy: i8,
    /// Steps remaining to reach player
    pub walk_steps_remaining: u8,
    /// Whether "!" bubble is currently visible
    pub emotion_bubble_visible: bool,
}

impl TrainerEncounter {
    pub fn new(map: MapId, trainer_index: u8, npc_index: u8) -> Self {
        Self {
            map,
            trainer_index,
            npc_index,
            state: TrainerBattleState::NotEngaged,
            wait_frames: 0,
            walk_dx: 0,
            walk_dy: 0,
            walk_steps_remaining: 0,
            emotion_bubble_visible: false,
        }
    }

    pub fn engage(&mut self, player_x: u8, player_y: u8, npc_x: u8, npc_y: u8) {
        self.state = TrainerBattleState::Spotted;
        self.wait_frames = 30; // "!" bubble display duration
        self.emotion_bubble_visible = true;

        // Calculate direction toward player
        let dx = player_x as i16 - npc_x as i16;
        let dy = player_y as i16 - npc_y as i16;
        if dx.abs() > dy.abs() {
            self.walk_dx = dx.signum() as i8;
            self.walk_dy = 0;
            self.walk_steps_remaining = dx.abs().saturating_sub(1) as u8;
        } else {
            self.walk_dx = 0;
            self.walk_dy = dy.signum() as i8;
            self.walk_steps_remaining = dy.abs().saturating_sub(1) as u8;
        }
    }
}

pub fn is_trainer_defeated(flags: &EventFlags, header: &TrainerHeader) -> bool {
    flags.check(header.event_flag)
}

pub fn mark_trainer_defeated(flags: &mut EventFlags, header: &TrainerHeader) {
    flags.set(header.event_flag);
}

pub fn can_trainer_see_player(
    trainer_x: u8,
    trainer_y: u8,
    trainer_facing_dx: i8,
    trainer_facing_dy: i8,
    player_x: u8,
    player_y: u8,
    sight_range: u8,
) -> bool {
    if trainer_facing_dx != 0 {
        if trainer_y != player_y {
            return false;
        }
        let dx = player_x as i16 - trainer_x as i16;
        if trainer_facing_dx > 0 {
            dx > 0 && dx <= sight_range as i16
        } else {
            dx < 0 && dx >= -(sight_range as i16)
        }
    } else if trainer_facing_dy != 0 {
        if trainer_x != player_x {
            return false;
        }
        let dy = player_y as i16 - trainer_y as i16;
        if trainer_facing_dy > 0 {
            dy > 0 && dy <= sight_range as i16
        } else {
            dy < 0 && dy >= -(sight_range as i16)
        }
    } else {
        false
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TrainerPosition {
    pub x: u8,
    pub y: u8,
    pub facing_dx: i8,
    pub facing_dy: i8,
}

pub fn check_all_trainers(
    headers: &[TrainerHeader],
    flags: &EventFlags,
    trainer_positions: &[TrainerPosition],
    player_x: u8,
    player_y: u8,
) -> Option<usize> {
    for (i, header) in headers.iter().enumerate() {
        if is_trainer_defeated(flags, header) {
            continue;
        }
        if let Some(pos) = trainer_positions.get(i) {
            if can_trainer_see_player(
                pos.x,
                pos.y,
                pos.facing_dx,
                pos.facing_dy,
                player_x,
                player_y,
                header.sight_range,
            ) {
                return Some(i);
            }
        }
    }
    None
}

pub fn advance_trainer_battle(encounter: &mut TrainerEncounter) -> TrainerBattleState {
    if encounter.wait_frames > 0 {
        encounter.wait_frames -= 1;
        return encounter.state;
    }

    match encounter.state {
        TrainerBattleState::NotEngaged => {
            // Should not be called in this state
            encounter.state
        }
        TrainerBattleState::Spotted => {
            encounter.emotion_bubble_visible = false;
            encounter.state = TrainerBattleState::WalkingToPlayer;
            encounter.wait_frames = 16; // walk step delay
            encounter.state
        }
        TrainerBattleState::WalkingToPlayer => {
            if encounter.walk_steps_remaining > 0 {
                encounter.walk_steps_remaining -= 1;
                encounter.wait_frames = 16;
                encounter.state
            } else {
                encounter.state = TrainerBattleState::ShowBeforeBattleText;
                encounter.wait_frames = 0;
                encounter.state
            }
        }
        TrainerBattleState::ShowBeforeBattleText => {
            encounter.state = TrainerBattleState::InBattle;
            encounter.state
        }
        TrainerBattleState::InBattle => {
            encounter.state = TrainerBattleState::ShowEndBattleText;
            encounter.state
        }
        TrainerBattleState::ShowEndBattleText => {
            encounter.state = TrainerBattleState::Defeated;
            encounter.state
        }
        TrainerBattleState::Defeated => TrainerBattleState::Defeated,
    }
}
