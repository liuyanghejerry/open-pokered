use pokered_data::move_data::MoveData;
use pokered_data::moves::{MoveEffect, MoveId};
use pokered_data::types::PokemonType;

use super::MoveChoiceLayer;
use crate::battle::state::BattlerState;
use crate::battle::types::get_type_effectiveness;

const INITIAL_SCORE: u8 = 10;
const DISABLED_PENALTY: u8 = 0x50;
const DISCOURAGE: u8 = 1;
const ENCOURAGE: u8 = 1;

fn is_status_only_effect(effect: MoveEffect) -> bool {
    matches!(
        effect,
        MoveEffect::Effect01
            | MoveEffect::SleepEffect
            | MoveEffect::PoisonEffect
            | MoveEffect::ParalyzeEffect
    )
}

fn is_stat_up_or_bide_range(effect: MoveEffect) -> bool {
    let e = effect as u8;
    // ATTACK_UP1_EFFECT (0x0A) ..< BIDE_EFFECT (0x1A)
    // or ATTACK_UP2_EFFECT (0x32) ..< POISON_EFFECT (0x42)
    (e >= 0x0A && e < 0x1A) || (e >= 0x32 && e < 0x42)
}

fn has_better_move(
    moves: &[MoveId; 4],
    current_index: usize,
    _defender_type1: PokemonType,
    _defender_type2: PokemonType,
) -> bool {
    let current_move = moves[current_index];
    let current_data = match MoveData::get(current_move) {
        Some(d) => d,
        None => return false,
    };

    for (i, &m) in moves.iter().enumerate() {
        if i == current_index || m == MoveId::None {
            continue;
        }
        let data = match MoveData::get(m) {
            Some(d) => d,
            None => continue,
        };

        // Super Fang is always "better"
        if data.effect == MoveEffect::SuperFangEffect {
            return true;
        }
        // SpecialDamageEffect (fixed damage) is "better"
        if data.effect == MoveEffect::SpecialDamageEffect {
            return true;
        }
        // FlyEffect is "better"
        if data.effect == MoveEffect::FlyEffect {
            return true;
        }
        // Any damaging move of a different type
        if data.power > 0 && data.move_type != current_data.move_type {
            return true;
        }
    }
    false
}

pub fn apply_layer1(buffer: &mut [u8; 4], enemy_moves: &[MoveId; 4], player: &BattlerState) {
    let has_status = !player.active_mon().status.is_none();
    if !has_status {
        return;
    }
    for i in 0..4 {
        if enemy_moves[i] == MoveId::None {
            continue;
        }
        if let Some(data) = MoveData::get(enemy_moves[i]) {
            if data.power == 0 && is_status_only_effect(data.effect) {
                buffer[i] = buffer[i].saturating_add(DISCOURAGE * 5);
            }
        }
    }
}

pub fn apply_layer2(buffer: &mut [u8; 4], enemy_moves: &[MoveId; 4], ai_layer2_encouragement: u8) {
    if ai_layer2_encouragement != 1 {
        return;
    }
    for i in 0..4 {
        if enemy_moves[i] == MoveId::None {
            continue;
        }
        if let Some(data) = MoveData::get(enemy_moves[i]) {
            if is_stat_up_or_bide_range(data.effect) {
                buffer[i] = buffer[i].saturating_sub(ENCOURAGE);
            }
        }
    }
}

pub fn apply_layer3(
    buffer: &mut [u8; 4],
    enemy_moves: &[MoveId; 4],
    enemy: &BattlerState,
    player: &BattlerState,
) {
    let def_mon = player.active_mon();
    for i in 0..4 {
        if enemy_moves[i] == MoveId::None {
            continue;
        }
        let data = match MoveData::get(enemy_moves[i]) {
            Some(d) => d,
            None => continue,
        };

        let eff = get_type_effectiveness(data.move_type, def_mon.type1, def_mon.type2);

        if eff.is_super_effective() {
            buffer[i] = buffer[i].saturating_sub(ENCOURAGE);
        } else if eff.is_not_very_effective() || eff.is_no_effect() {
            if has_better_move(enemy_moves, i, def_mon.type1, def_mon.type2) {
                buffer[i] = buffer[i].saturating_add(DISCOURAGE);
            }
        }
    }
    let _ = enemy;
}

fn find_minimum_and_filter(buffer: &mut [u8; 4], moves: &[MoveId; 4]) {
    let mut min_val = u8::MAX;
    for i in 0..4 {
        if moves[i] != MoveId::None && buffer[i] < min_val {
            min_val = buffer[i];
        }
    }
    if min_val == u8::MAX {
        return;
    }
    for i in 0..4 {
        if moves[i] == MoveId::None || buffer[i] != min_val {
            buffer[i] = 0;
        } else {
            buffer[i] = 1;
        }
    }
}

pub struct MoveChoiceResult {
    pub candidates: [u8; 4],
}

impl MoveChoiceResult {
    pub fn pick_move(&self, rand_val: u8) -> Option<usize> {
        let count: u8 = self.candidates.iter().filter(|&&v| v > 0).count() as u8;
        if count == 0 {
            return None;
        }
        let idx = (rand_val as usize) % (count as usize);
        let mut seen = 0usize;
        for i in 0..4 {
            if self.candidates[i] > 0 {
                if seen == idx {
                    return Some(i);
                }
                seen += 1;
            }
        }
        None
    }
}

pub fn choose_moves(
    layers: &[MoveChoiceLayer],
    enemy: &BattlerState,
    player: &BattlerState,
    ai_layer2_encouragement: u8,
) -> MoveChoiceResult {
    let moves = enemy.active_mon().moves;
    let mut buffer = [INITIAL_SCORE; 4];

    for i in 0..4 {
        if moves[i] == MoveId::None {
            buffer[i] = 0;
        }
    }

    if enemy.disabled_move > 0 && (enemy.disabled_move as usize) <= 4 {
        let idx = (enemy.disabled_move as usize) - 1;
        buffer[idx] = DISABLED_PENALTY;
    }

    for layer in layers {
        match layer {
            MoveChoiceLayer::Layer1 => apply_layer1(&mut buffer, &moves, player),
            MoveChoiceLayer::Layer2 => apply_layer2(&mut buffer, &moves, ai_layer2_encouragement),
            MoveChoiceLayer::Layer3 => apply_layer3(&mut buffer, &moves, enemy, player),
        }
    }

    find_minimum_and_filter(&mut buffer, &moves);

    MoveChoiceResult { candidates: buffer }
}

#[path = "move_choice_tests.rs"]
#[cfg(test)]
mod move_choice_tests;
