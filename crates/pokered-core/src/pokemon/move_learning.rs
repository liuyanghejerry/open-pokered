use pokered_data::evos_moves::evos_moves_data;
use pokered_data::items::{HM_MOVES, TM_MOVES};
use pokered_data::move_data::MOVES;
use pokered_data::moves::MoveId;
use pokered_data::pokemon_data::get_base_stats;
use pokered_data::species::Species;

use crate::battle::state::Pokemon;

pub const NUM_MOVES: usize = 4;
pub const NUM_TMS: usize = 50;
pub const NUM_HMS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearnMoveResult {
    Learned { slot: usize },
    AlreadyKnown,
    MoveSlotsFull,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgetMoveResult {
    Forgotten { slot: usize },
    CannotForgetHm,
    MoveNotKnown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeachError {
    Incompatible,
    AlreadyKnown,
    MoveSlotsFull,
    InvalidTmHm,
}

pub fn is_hm_move(move_id: MoveId) -> bool {
    HM_MOVES.contains(&move_id)
}

pub fn get_move_max_pp(move_id: MoveId) -> u8 {
    MOVES
        .iter()
        .find(|m| m.id == move_id)
        .map(|m| m.pp)
        .unwrap_or(0)
}

/// `tm_hm_flags` in BaseStats is a 7-byte (56-bit) bitfield.
/// Bits 0..49 = TM01..TM50, bits 50..54 = HM01..HM05.
pub fn can_learn_tm(species: Species, tm_number: u8) -> bool {
    if tm_number < 1 || tm_number > NUM_TMS as u8 {
        return false;
    }
    check_tm_hm_bit(species, (tm_number - 1) as usize)
}

pub fn can_learn_hm(species: Species, hm_number: u8) -> bool {
    if hm_number < 1 || hm_number > NUM_HMS as u8 {
        return false;
    }
    check_tm_hm_bit(species, NUM_TMS + (hm_number - 1) as usize)
}

fn check_tm_hm_bit(species: Species, bit_index: usize) -> bool {
    let base = match get_base_stats(species) {
        Some(b) => b,
        None => return false,
    };
    let byte_index = bit_index / 8;
    let bit_offset = bit_index % 8;
    if byte_index >= base.tm_hm_flags.len() {
        return false;
    }
    (base.tm_hm_flags[byte_index] >> bit_offset) & 1 != 0
}

pub fn tm_to_move(tm_number: u8) -> Option<MoveId> {
    if tm_number < 1 || tm_number > NUM_TMS as u8 {
        return None;
    }
    Some(TM_MOVES[(tm_number - 1) as usize])
}

pub fn hm_to_move(hm_number: u8) -> Option<MoveId> {
    if hm_number < 1 || hm_number > NUM_HMS as u8 {
        return None;
    }
    Some(HM_MOVES[(hm_number - 1) as usize])
}

pub fn moves_at_level(species: Species, level: u8) -> Vec<MoveId> {
    let all_data = evos_moves_data();
    let entry = match all_data.iter().find(|e| e.species == species) {
        Some(e) => e,
        None => return vec![],
    };
    entry
        .learnset
        .iter()
        .filter(|lm| lm.level == level)
        .map(|lm| lm.move_id)
        .collect()
}

/// Fill `moves` (and `pp` when `with_pp`) with the moves a mon has at `level`,
/// Gen-1 `WriteMonMoves` semantics (evos_moves.asm:383-470): start from the
/// L1 initial moves, then apply every learnset entry whose level is <= the
/// mon's level in order — into the first empty slot, or by shifting all
/// slots up (deleting the oldest) when full. Assumes the learnset is sorted
/// by level, as the original does.
pub fn write_mon_moves(species: Species, level: u8, moves: &mut [MoveId; 4]) {
    let entry = match pokered_data::evos_moves::get_evos_moves(species) {
        Some(e) => e,
        None => return,
    };

    for lm in entry.learnset.iter() {
        if lm.level > level {
            // The original stops scanning at the first entry above the level
            // (learnsets are sorted).
            break;
        }
        let mv = lm.move_id;
        if mv == MoveId::None || moves.contains(&mv) {
            continue;
        }
        if let Some(slot) = moves.iter().position(|m| *m == MoveId::None) {
            moves[slot] = mv;
        } else {
            moves.copy_within(1..4, 0);
            moves[3] = mv;
        }
    }
}

pub fn try_learn_move(mon: &mut Pokemon, move_id: MoveId) -> LearnMoveResult {
    if move_id == MoveId::None {
        return LearnMoveResult::AlreadyKnown;
    }

    if mon.moves.contains(&move_id) {
        return LearnMoveResult::AlreadyKnown;
    }

    for i in 0..NUM_MOVES {
        if mon.moves[i] == MoveId::None {
            mon.moves[i] = move_id;
            mon.pp[i] = get_move_max_pp(move_id);
            return LearnMoveResult::Learned { slot: i };
        }
    }

    LearnMoveResult::MoveSlotsFull
}

pub fn replace_move(mon: &mut Pokemon, slot: usize, new_move: MoveId) {
    if slot < NUM_MOVES {
        mon.moves[slot] = new_move;
        mon.pp[slot] = get_move_max_pp(new_move);
    }
}

/// Why a replace-move swap was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaceMoveError {
    /// The slot is out of range or empty.
    InvalidSlot,
    /// The move in the slot is an HM move — Gen 1 has no Move Deleter
    /// (HMCantDeleteText, learn_move.asm:178-181).
    HmCantDelete,
}

/// The LearnMove replace step (learn_move.asm:160-181): swap `slot` for
/// `new_move` and return the forgotten move. The HM guard (`IsMoveHM` →
/// HMCantDeleteText) applies to EVERY teach path — TMs, HMs AND level-up
/// learning — the check runs before the swap unconditionally.
pub fn replace_move_guarded(
    mon: &mut Pokemon,
    slot: usize,
    new_move: MoveId,
) -> Result<MoveId, ReplaceMoveError> {
    if slot >= NUM_MOVES || mon.moves[slot] == MoveId::None {
        return Err(ReplaceMoveError::InvalidSlot);
    }
    if is_hm_move(mon.moves[slot]) {
        return Err(ReplaceMoveError::HmCantDelete);
    }
    let old = mon.moves[slot];
    replace_move(mon, slot, new_move);
    Ok(old)
}

pub fn teach_tm(mon: &mut Pokemon, tm_number: u8) -> Result<LearnMoveResult, TeachError> {
    let move_id = tm_to_move(tm_number).ok_or(TeachError::InvalidTmHm)?;

    if !can_learn_tm(mon.species, tm_number) {
        return Err(TeachError::Incompatible);
    }

    let result = try_learn_move(mon, move_id);
    match result {
        LearnMoveResult::AlreadyKnown => Err(TeachError::AlreadyKnown),
        other => Ok(other),
    }
}

pub fn teach_hm(mon: &mut Pokemon, hm_number: u8) -> Result<LearnMoveResult, TeachError> {
    let move_id = hm_to_move(hm_number).ok_or(TeachError::InvalidTmHm)?;

    if !can_learn_hm(mon.species, hm_number) {
        return Err(TeachError::Incompatible);
    }

    let result = try_learn_move(mon, move_id);
    match result {
        LearnMoveResult::AlreadyKnown => Err(TeachError::AlreadyKnown),
        other => Ok(other),
    }
}

/// Gen 1: HM moves cannot be forgotten (no Move Deleter).
pub fn forget_move(mon: &mut Pokemon, move_id: MoveId) -> ForgetMoveResult {
    if is_hm_move(move_id) {
        return ForgetMoveResult::CannotForgetHm;
    }

    for i in 0..NUM_MOVES {
        if mon.moves[i] == move_id {
            for j in i..NUM_MOVES - 1 {
                mon.moves[j] = mon.moves[j + 1];
                mon.pp[j] = mon.pp[j + 1];
            }
            mon.moves[NUM_MOVES - 1] = MoveId::None;
            mon.pp[NUM_MOVES - 1] = 0;
            return ForgetMoveResult::Forgotten { slot: i };
        }
    }

    ForgetMoveResult::MoveNotKnown
}

pub fn process_level_up_moves(
    mon: &mut Pokemon,
    old_level: u8,
    new_level: u8,
) -> LevelUpMovesResult {
    let mut learned = vec![];
    let mut pending = vec![];

    for lv in (old_level + 1)..=new_level {
        let moves = moves_at_level(mon.species, lv);
        for move_id in moves {
            match try_learn_move(mon, move_id) {
                LearnMoveResult::Learned { slot } => {
                    learned.push(LearnedMoveInfo {
                        move_id,
                        slot,
                        level: lv,
                    });
                }
                LearnMoveResult::AlreadyKnown => {}
                LearnMoveResult::MoveSlotsFull => {
                    pending.push(PendingMove { move_id, level: lv });
                }
            }
        }
    }

    LevelUpMovesResult { learned, pending }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearnedMoveInfo {
    pub move_id: MoveId,
    pub slot: usize,
    pub level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingMove {
    pub move_id: MoveId,
    pub level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelUpMovesResult {
    pub learned: Vec<LearnedMoveInfo>,
    pub pending: Vec<PendingMove>,
}
