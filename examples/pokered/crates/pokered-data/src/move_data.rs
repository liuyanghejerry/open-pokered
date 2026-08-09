use crate::moves::{MoveEffect, MoveId};
use crate::types::PokemonType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveData {
    pub id: MoveId,
    pub effect: MoveEffect,
    pub power: u8,
    pub move_type: PokemonType,
    pub accuracy: u8,
    pub pp: u8,
}

pub const MOVES: &[MoveData] =
    &include!(concat!(env!("OUT_DIR"), "/move_data_gen.rs"));

impl MoveData {
    pub fn get(id: MoveId) -> Option<&'static MoveData> {
        // Editor-injected runtime override shadows the baseline (leaked once
        // at injection — no re-leak per query).
        if let Some(ov) = crate::runtime_overrides::move_override(id) {
            return Some(ov);
        }
        let idx = id as usize;
        if idx == 0 || idx > MOVES.len() {
            None
        } else {
            Some(&MOVES[idx - 1])
        }
    }
}
