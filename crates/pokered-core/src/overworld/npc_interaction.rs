//! Pokémon-specific NPC interaction system.
//!
//! Generic interaction logic (talk, sign, line-of-sight) is provided by
//! `jrpg_engine::overworld::npc_interaction`. This module adds Pokémon-specific
//! handling for trainer battles and item pickups.

use crate::overworld::collision::PokemonCollisionProvider;
use crate::overworld::npc_movement::NpcRuntimeState;
use crate::overworld::PokemonNpcData;

use jrpg_engine::overworld::Direction;
use jrpg_engine::overworld::npc_interaction as engine;

use super::npc_movement::npc_in_front_of_player;

// ── Re-exports ─────────────────────────────────────────────────────

pub use jrpg_engine::overworld::npc_interaction::{
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

/// Check if any trainer NPC can see the player.
///
/// Uses the engine's generic line-of-sight algorithm, filtering only
/// trainer-type NPCs and enriching the result with trainer data.
pub fn check_trainer_line_of_sight(
    npcs: &[NpcRuntimeState],
    pokemon_data: &[PokemonNpcData],
    player_x: u16,
    player_y: u16,
) -> Option<TrainerSighting> {
    // Generic LOS check returns the first NPC that can see the player.
    let sighting = engine::check_line_of_sight(npcs, player_x, player_y)?;

    let extra = pokemon_data.get(sighting.npc_index as usize)?;
    if !extra.is_trainer {
        return None;
    }

    Some(TrainerSighting {
        npc_index: sighting.npc_index,
        trainer_class: extra.trainer_class,
        trainer_set: extra.trainer_set,
        distance: sighting.distance,
    })
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
