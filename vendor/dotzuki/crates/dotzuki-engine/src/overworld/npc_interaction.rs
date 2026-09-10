//! Generic NPC interaction system — talk, line-of-sight, sign interaction.
//!
//! Implements interaction logic: the talk/use interaction handler and
//! trainer sight checks (fighting-map trainers).

use crate::map::MapTrait;
use crate::tileset::TilesetTrait;

use super::collision::CollisionProvider;
use super::npc_movement::{npc_in_front_of_player, NpcRuntimeState};
use super::player_movement::direction_delta;
use super::types::{Direction, MapData};

// ── Interaction Result ─────────────────────────────────────────────

/// Result of an NPC interaction attempt (pressing A near an NPC).
///
/// Game-specific interaction types (e.g., trainer battle, item pickup)
/// are handled by the consuming crate using additional NPC metadata
/// stored alongside the runtime state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractionResult {
    /// No NPC in front of the player.
    NoTarget,
    /// Regular NPC dialog — show text_id from the map's text table.
    Talk { npc_index: u8, text_id: u8 },
    /// NPC already defeated/collected — shows post-defeat dialog.
    AlreadyDefeated { npc_index: u8, text_id: u8 },
}

// ── Try Interact ───────────────────────────────────────────────────

/// Attempt to interact with the NPC the player is facing.
///
/// In the original game, pressing A checks the tile in front of
/// the player for an NPC sprite, then dispatches based on the NPC's
/// type (regular text, trainer, or item ball).
///
/// If the tile in front is a counter tile, extends the range by one
/// more tile to allow talking to NPCs behind counters.
///
/// Game-specific NPC types (trainers, item balls) should be checked
/// by the caller before or after calling this function.
pub fn try_interact<M: MapTrait, T: TilesetTrait, Mus>(
    npcs: &[NpcRuntimeState],
    player_x: u16,
    player_y: u16,
    facing: Direction,
    map: Option<&MapData<M, T, Mus>>,
    provider: &impl CollisionProvider<T>,
) -> InteractionResult {
    let npc = match npc_in_front_of_player(npcs, player_x, player_y, facing, map, provider) {
        Some(n) => n,
        None => return InteractionResult::NoTarget,
    };

    if npc.defeated {
        return InteractionResult::AlreadyDefeated {
            npc_index: npc.npc_index,
            text_id: npc.text_id,
        };
    }

    InteractionResult::Talk {
        npc_index: npc.npc_index,
        text_id: npc.text_id,
    }
}

// ── Line of Sight ──────────────────────────────────────────────────

/// Result of a line-of-sight check between an NPC and the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineOfSightResult {
    pub npc_index: u8,
    pub distance: u8,
}

/// Check if any NPC can see the player along their facing direction.
///
/// Each NPC's range determines how far they can see in their facing
/// direction.  An NPC with range of 0 is never checked.
///
/// This is the generic LOS algorithm — the caller should filter which
/// NPCs to include (e.g., only trainer-type NPCs).
pub fn check_line_of_sight(
    npcs: &[NpcRuntimeState],
    player_x: u16,
    player_y: u16,
) -> Option<LineOfSightResult> {
    for npc in npcs {
        if !npc.visible || npc.defeated || npc.range == 0 {
            continue;
        }

        let (dx, dy) = direction_delta(npc.facing);
        let mut check_x = npc.x as i32;
        let mut check_y = npc.y as i32;

        for dist in 1..=npc.range {
            check_x += dx as i32;
            check_y += dy as i32;

            if check_x < 0 || check_y < 0 {
                break;
            }

            if check_x as u16 == player_x && check_y as u16 == player_y {
                return Some(LineOfSightResult {
                    npc_index: npc.npc_index,
                    distance: dist,
                });
            }
        }
    }
    None
}

// ── Mark Defeated ──────────────────────────────────────────────────

/// Mark an NPC as defeated after a battle or other interaction.
pub fn mark_defeated(npcs: &mut [NpcRuntimeState], npc_index: u8) {
    if let Some(npc) = npcs.iter_mut().find(|n| n.npc_index == npc_index) {
        npc.defeated = true;
    }
}

// ── Sign Interaction ───────────────────────────────────────────────

/// Check if a sign is at the tile the player is facing.
///
/// Signs are interacted with by pressing A while facing them.
pub fn check_sign_interaction(
    signs: &[(u8, u8, u8)],
    player_x: u16,
    player_y: u16,
    facing: Direction,
) -> Option<u8> {
    let (dx, dy) = direction_delta(facing);
    let target_x = (player_x as i32 + dx as i32) as u8;
    let target_y = (player_y as i32 + dy as i32) as u8;

    signs
        .iter()
        .find(|&&(sx, sy, _)| sx == target_x && sy == target_y)
        .map(|&(_, _, text_id)| text_id)
}
