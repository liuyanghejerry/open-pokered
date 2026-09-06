//! Pokémon-specific NPC interaction system.
//!
//! Generic interaction logic (talk, sign) is provided by
//! `dotzuki_engine::overworld::npc_interaction`. This module adds Pokémon-specific
//! handling for trainer battles and item pickups, plus the trainer
//! line-of-sight check driven by the per-map trainer-header tables
//! (original `CheckForEngagingTrainers` semantics).

use crate::overworld::collision::PokemonCollisionProvider;
use crate::overworld::npc_movement::NpcRuntimeState;
use crate::overworld::PokemonNpcData;

use dotzuki_engine::overworld::Direction;

use super::npc_movement::npc_in_front_of_player;

// ── Re-exports ─────────────────────────────────────────────────────

pub use dotzuki_engine::overworld::npc_interaction::{
    check_line_of_sight, check_sign_interaction, mark_defeated as mark_trainer_defeated,
    LineOfSightResult,
};

// ── Pokémon-Specific Interaction Result ────────────────────────────

/// Result of an NPC interaction attempt (pressing A near an NPC).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractionResult {
    /// No NPC in front of the player.
    NoTarget,
    /// Regular NPC dialog.
    Talk { npc_index: u8, text_id: u8 },
    /// Trainer battle trigger.
    TrainerBattle {
        npc_index: u8,
        trainer_class: u8,
        trainer_set: u8,
    },
    /// Item pickup (item ball NPC).
    ItemPickup { npc_index: u8, item_id: u8 },
    /// NPC already defeated/collected.
    AlreadyDefeated { npc_index: u8, text_id: u8 },
}

// ── Pokémon-Specific Interaction ───────────────────────────────────

/// Attempt to interact with the NPC the player is facing.
///
/// Checks for trainer battles and item pickups before falling through
/// to the engine's generic talk/dialogue logic.
pub fn try_interact(
    npcs: &[NpcRuntimeState],
    pokemon_data: &[PokemonNpcData],
    player_x: u16,
    player_y: u16,
    facing: Direction,
    map: Option<&super::MapData>,
    provider: &PokemonCollisionProvider,
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

    // Look up Pokémon-specific NPC data
    let extra = pokemon_data.get(npc.npc_index as usize);

    if extra.map_or(false, |e| e.is_trainer) {
        let e = extra.unwrap();
        return InteractionResult::TrainerBattle {
            npc_index: npc.npc_index,
            trainer_class: e.trainer_class,
            trainer_set: e.trainer_set,
        };
    }

    if extra.map_or(false, |e| e.item_id != 0) {
        return InteractionResult::ItemPickup {
            npc_index: npc.npc_index,
            item_id: extra.unwrap().item_id,
        };
    }

    InteractionResult::Talk {
        npc_index: npc.npc_index,
        text_id: npc.text_id,
    }
}

// ── Trainer Line of Sight ──────────────────────────────────────────

/// Result of a trainer line-of-sight check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainerSighting {
    pub npc_index: u8,
    pub trainer_class: u8,
    pub trainer_set: u8,
    pub distance: u8,
}

/// Facing direction → tile delta.
fn facing_delta(facing: Direction) -> (i8, i8) {
    match facing {
        Direction::Up => (0, -1),
        Direction::Down => (0, 1),
        Direction::Left => (-1, 0),
        Direction::Right => (1, 0),
    }
}

/// Check if any trainer NPC can see the player.
///
/// Reproduces `CheckForEngagingTrainers` + `TrainerEngage`
/// (home/trainers.asm:264, engine/overworld/trainer_sight.asm:164): the
/// engage distance comes from the map's trainer-header table
/// (`db view_range << 4` — NOT the map object's range byte, which for
/// STAY trainers encodes the facing direction), the facing is read live
/// from the NPC, and trainers whose `EVENT_BEAT_*` flag is set (already
/// defeated) are skipped. The k-th header belongs to the k-th trainer
/// NPC in object order; headers with `sight_range == 0` are talk-only
/// trainers that never engage by sight in the original.
pub fn check_trainer_line_of_sight(
    npcs: &[NpcRuntimeState],
    pokemon_data: &[PokemonNpcData],
    headers: &[pokered_data::trainer_headers::TrainerHeaderData],
    flags: &super::event_flags::EventFlags,
    player_x: u16,
    player_y: u16,
) -> Option<TrainerSighting> {
    let mut k = 0usize;
    for (npc, extra) in npcs.iter().zip(pokemon_data.iter()) {
        if !extra.is_trainer {
            continue;
        }
        let header = headers.get(k);
        k += 1;
        let Some(header) = header else { break };
        if header.sight_range == 0 {
            continue; // talk-only trainer (view range 0)
        }
        if npc.defeated || !npc.visible {
            continue;
        }
        if flags.check(header.event_flag) {
            continue;
        }
        let (fdx, fdy) = facing_delta(npc.facing);
        if super::trainer_engine::can_trainer_see_player(
            npc.x as u8,
            npc.y as u8,
            fdx,
            fdy,
            player_x as u8,
            player_y as u8,
            header.sight_range,
        ) {
            // Aligned on one axis, so the max delta IS the sight distance.
            let distance = (player_x as i16 - npc.x as i16)
                .abs()
                .max((player_y as i16 - npc.y as i16).abs()) as u8;
            return Some(TrainerSighting {
                npc_index: npc.npc_index,
                trainer_class: extra.trainer_class,
                trainer_set: extra.trainer_set,
                distance,
            });
        }
    }
    None
}

// ── Item & Trainer Helpers ─────────────────────────────────────────

/// Process an item pickup: mark the NPC as defeated (collected) and
/// return the item_id.
pub fn collect_item(npcs: &mut [NpcRuntimeState], pokemon_data: &[PokemonNpcData], npc_index: u8) -> Option<u8> {
    let npc = npcs.iter_mut().find(|n| n.npc_index == npc_index)?;
    if npc.defeated {
        return None;
    }
    let extra = pokemon_data.get(npc_index as usize)?;
    if extra.item_id == 0 {
        return None;
    }
    npc.defeated = true;
    npc.visible = false;
    Some(extra.item_id)
}
