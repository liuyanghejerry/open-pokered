use pokered_data::moves::MoveId;

use super::state::{BattleState, BattlerState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOrder {
    PlayerFirst,
    EnemyFirst,
}

/// Determine who moves first this turn.
///
/// Gen 1 priority rules (from ASM):
/// 1. Quick Attack has +1 priority (explicit check, not a generic system)
/// 2. Counter always goes second (priority -1)
/// 3. If both have same priority bracket: compare speed stats
/// 4. Equal speed: 50/50 coin flip
pub fn determine_order(state: &BattleState, random_byte: u8) -> TurnOrder {
    let player_move = state.player.selected_move;
    let enemy_move = state.enemy.selected_move;

    let player_priority = move_priority(player_move);
    let enemy_priority = move_priority(enemy_move);

    if player_priority > enemy_priority {
        return TurnOrder::PlayerFirst;
    }
    if enemy_priority > player_priority {
        return TurnOrder::EnemyFirst;
    }

    let player_speed = effective_speed(&state.player);
    let enemy_speed = effective_speed(&state.enemy);

    if player_speed > enemy_speed {
        TurnOrder::PlayerFirst
    } else if enemy_speed > player_speed {
        TurnOrder::EnemyFirst
    } else {
        // Equal speed: coin flip (ASM: random, compare with $80)
        if random_byte < 128 {
            TurnOrder::PlayerFirst
        } else {
            TurnOrder::EnemyFirst
        }
    }
}

fn move_priority(move_id: MoveId) -> i8 {
    match move_id {
        MoveId::QuickAttack => 1,
        MoveId::Counter => -1,
        _ => 0,
    }
}

fn effective_speed(battler: &BattlerState) -> u16 {
    use super::stat_stages::apply_stage;
    // Badge stat boosts: the player side's working Speed is the boosted
    // `wBattleMonSpeed` copy (the turn-order comparison in the original reads
    // exactly that); the enemy side never carries one.
    let base = battler
        .badge_boosted_stats
        .map(|b| b[2])
        .unwrap_or_else(|| battler.active_mon().speed);
    let staged = apply_stage(base, battler.stat_stages.speed);
    // Paralysis quarters speed (ASM: QuarterSpeedDueToParalysis)
    if battler.active_mon().status == super::state::StatusCondition::Paralysis {
        (staged / 4).max(1)
    } else {
        staged
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::state::*;
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;

    fn pokemon_with_speed(speed: u16) -> Pokemon {
        Pokemon {
            species: Species::Pikachu,
            nickname: None,
            level: 50,
            hp: 100,
            max_hp: 100,
            attack: 80,
            defense: 60,
            speed,
            special: 70,
            type1: PokemonType::Electric,
            type2: PokemonType::Electric,
            moves: [
                MoveId::Thundershock,
                MoveId::QuickAttack,
                MoveId::None,
                MoveId::None,
            ],
            pp: [30, 30, 0, 0],
            pp_ups: [0; 4],
            status: StatusCondition::None,
            dv_bytes: [0xFF, 0xFF],
            stat_exp: [0; 5],
            total_exp: 0,
            is_traded: false, ot_id: 0, ot_name: None,
        }
    }

    fn make_battle(player_speed: u16, enemy_speed: u16) -> BattleState {
        let p = vec![pokemon_with_speed(player_speed)];
        let e = vec![pokemon_with_speed(enemy_speed)];
        let mut state = new_battle_state(BattleType::Wild, p, e);
        state.player.selected_move = MoveId::Thundershock;
        state.enemy.selected_move = MoveId::Thundershock;
        state
    }

    #[test]
    fn faster_pokemon_goes_first() {
        let state = make_battle(100, 50);
        assert_eq!(determine_order(&state, 0), TurnOrder::PlayerFirst);

        let state = make_battle(50, 100);
        assert_eq!(determine_order(&state, 0), TurnOrder::EnemyFirst);
    }

    #[test]
    fn equal_speed_coin_flip() {
        let state = make_battle(80, 80);
        assert_eq!(determine_order(&state, 0), TurnOrder::PlayerFirst);
        assert_eq!(determine_order(&state, 127), TurnOrder::PlayerFirst);
        assert_eq!(determine_order(&state, 128), TurnOrder::EnemyFirst);
        assert_eq!(determine_order(&state, 255), TurnOrder::EnemyFirst);
    }

    #[test]
    fn quick_attack_priority() {
        let mut state = make_battle(50, 100);
        state.player.selected_move = MoveId::QuickAttack;
        state.enemy.selected_move = MoveId::Thundershock;
        assert_eq!(determine_order(&state, 0), TurnOrder::PlayerFirst);
    }

    #[test]
    fn counter_goes_last() {
        let mut state = make_battle(100, 50);
        state.player.selected_move = MoveId::Counter;
        state.enemy.selected_move = MoveId::Thundershock;
        assert_eq!(determine_order(&state, 0), TurnOrder::EnemyFirst);
    }

    #[test]
    fn both_quick_attack_uses_speed() {
        let mut state = make_battle(100, 50);
        state.player.selected_move = MoveId::QuickAttack;
        state.enemy.selected_move = MoveId::QuickAttack;
        assert_eq!(determine_order(&state, 0), TurnOrder::PlayerFirst);
    }

    #[test]
    fn paralysis_quarters_speed() {
        let mut state = make_battle(100, 50);
        state.player.active_mon_mut().status = StatusCondition::Paralysis;
        assert_eq!(determine_order(&state, 0), TurnOrder::EnemyFirst);
    }
}

// ─── P0b differential parity: engine driver vs. legacy turn loop ─────────
//
// Proves the generic engine turn-ordering path (`PokemonRedData::turn_order_key`
// + the engine's ascending stable-sort + one-byte speed-tie coin flip) picks the
// SAME first mover as pokered's legacy `determine_order` on identical inputs and
// the same RNG stream. Both real code paths are exercised — no stubs, no
// hardcoded expected answers: the legacy fn IS the oracle.
#[cfg(test)]
mod engine_parity_tests {
    use super::{determine_order, move_priority, TurnOrder};
    use crate::battle::state::{
        new_battle_state, BattleType, Pokemon, StatusCondition as CoreStatus,
    };
    use dotzuki_engine::battle::rng::ScriptedRng;
    use dotzuki_engine::battle::{
        BattleAction, BattleProvider, BattleState as EngineState,
        BattlerState as EngineBattler, BattlerRef, EnumMap, OrderKey,
    };
    use pokered_data::impl_traits::{PokemonRedData, StatIndex, StatusCondition as DataStatus};
    use pokered_data::moves::MoveId;
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;

    /// One scalar test case shared by both code paths.
    #[derive(Clone, Copy)]
    struct Case {
        player_speed: u16,
        enemy_speed: u16,
        player_move: MoveId,
        enemy_move: MoveId,
        player_paralyzed: bool,
        enemy_paralyzed: bool,
        rng_byte: u8,
    }

    // ── Legacy path: build a pokered-core BattleState, call determine_order ──

    fn legacy_pokemon(speed: u16, paralyzed: bool) -> Pokemon {
        Pokemon {
            species: Species::Pikachu,
            nickname: None,
            level: 50,
            hp: 100,
            max_hp: 100,
            attack: 80,
            defense: 60,
            speed,
            special: 70,
            type1: PokemonType::Electric,
            type2: PokemonType::Electric,
            moves: [MoveId::Thundershock, MoveId::None, MoveId::None, MoveId::None],
            pp: [30, 0, 0, 0],
            pp_ups: [0; 4],
            status: if paralyzed {
                CoreStatus::Paralysis
            } else {
                CoreStatus::None
            },
            dv_bytes: [0xFF, 0xFF],
            stat_exp: [0; 5],
            total_exp: 0,
            is_traded: false, ot_id: 0, ot_name: None,
        }
    }

    fn legacy_first_mover(c: &Case) -> TurnOrder {
        let p = vec![legacy_pokemon(c.player_speed, c.player_paralyzed)];
        let e = vec![legacy_pokemon(c.enemy_speed, c.enemy_paralyzed)];
        let mut state = new_battle_state(BattleType::Wild, p, e);
        state.player.selected_move = c.player_move;
        state.enemy.selected_move = c.enemy_move;
        determine_order(&state, c.rng_byte)
    }

    // ── Engine path: build BattleState<PokemonRedData>, run turn_order_key
    //    + the engine's ascending stable-sort exactly as BattleDriver does. ──

    fn engine_battler(speed: u16, paralyzed: bool) -> EngineBattler<PokemonRedData> {
        let mut stats = EnumMap::new();
        stats.set(StatIndex::Speed, speed);
        let mut b = EngineBattler::new(Species::Pikachu, 100, 100, stats, vec![]);
        if paralyzed {
            b.status = Some(DataStatus::Paralysis);
        }
        b
    }

    fn engine_first_mover(c: &Case) -> TurnOrder {
        let provider = PokemonRedData;
        let state = EngineState::new(
            vec![engine_battler(c.player_speed, c.player_paralyzed)],
            vec![engine_battler(c.enemy_speed, c.enemy_paralyzed)],
        );
        let mut rng = ScriptedRng::new(vec![c.rng_byte]);

        // Mirror BattleDriver::execute_turn's ordering block (driver.rs:245-251):
        // key each actor in submission order (player idx 0, opponent idx 1),
        // then stable-sort ascending by OrderKey.
        let actions = [
            BattleAction::<PokemonRedData>::Fight { move_: c.player_move },
            BattleAction::<PokemonRedData>::Fight { move_: c.enemy_move },
        ];
        let actors = [BattlerRef::PLAYER, BattlerRef::OPPONENT];
        let mut keyed: Vec<(OrderKey, usize)> = actors
            .iter()
            .enumerate()
            .map(|(idx, who)| {
                (provider.turn_order_key(&state, *who, &actions[idx], &mut rng), idx)
            })
            .collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0));

        // Legacy consumes exactly one order byte; the engine path must too.
        assert_eq!(
            rng.consumed(),
            1,
            "engine turn-order must draw exactly one byte (legacy parity)"
        );

        if keyed[0].1 == 0 {
            TurnOrder::PlayerFirst
        } else {
            TurnOrder::EnemyFirst
        }
    }

    /// The representative matrix required by the task.
    fn matrix() -> Vec<(&'static str, Case)> {
        vec![
            (
                "player faster",
                Case {
                    player_speed: 100,
                    enemy_speed: 50,
                    player_move: MoveId::Thundershock,
                    enemy_move: MoveId::Thundershock,
                    player_paralyzed: false,
                    enemy_paralyzed: false,
                    rng_byte: 200, // tie byte present but unused on a non-tie
                },
            ),
            (
                "enemy faster",
                Case {
                    player_speed: 50,
                    enemy_speed: 100,
                    player_move: MoveId::Thundershock,
                    enemy_move: MoveId::Thundershock,
                    player_paralyzed: false,
                    enemy_paralyzed: false,
                    rng_byte: 10,
                },
            ),
            (
                "equal speed, rng byte < 128 -> player first",
                Case {
                    player_speed: 80,
                    enemy_speed: 80,
                    player_move: MoveId::Thundershock,
                    enemy_move: MoveId::Thundershock,
                    player_paralyzed: false,
                    enemy_paralyzed: false,
                    rng_byte: 127,
                },
            ),
            (
                "equal speed, rng byte == 128 -> enemy first",
                Case {
                    player_speed: 80,
                    enemy_speed: 80,
                    player_move: MoveId::Thundershock,
                    enemy_move: MoveId::Thundershock,
                    player_paralyzed: false,
                    enemy_paralyzed: false,
                    rng_byte: 128,
                },
            ),
            (
                "equal speed, rng byte >= 128 -> enemy first",
                Case {
                    player_speed: 80,
                    enemy_speed: 80,
                    player_move: MoveId::Thundershock,
                    enemy_move: MoveId::Thundershock,
                    player_paralyzed: false,
                    enemy_paralyzed: false,
                    rng_byte: 200,
                },
            ),
            (
                "priority move (Quick Attack) beats faster normal move",
                Case {
                    player_speed: 50,
                    enemy_speed: 100,
                    player_move: MoveId::QuickAttack,
                    enemy_move: MoveId::Thundershock,
                    player_paralyzed: false,
                    enemy_paralyzed: false,
                    rng_byte: 0,
                },
            ),
            (
                "Counter (-1 priority) goes after faster normal move",
                Case {
                    player_speed: 100,
                    enemy_speed: 50,
                    player_move: MoveId::Counter,
                    enemy_move: MoveId::Thundershock,
                    player_paralyzed: false,
                    enemy_paralyzed: false,
                    rng_byte: 0,
                },
            ),
            (
                "paralysis quarters player speed -> enemy first",
                Case {
                    player_speed: 100,
                    enemy_speed: 50,
                    player_move: MoveId::Thundershock,
                    enemy_move: MoveId::Thundershock,
                    player_paralyzed: true,
                    enemy_paralyzed: false,
                    rng_byte: 0,
                },
            ),
        ]
    }

    #[test]
    fn engine_turn_order_matches_legacy_across_matrix() {
        // Sanity: move_priority is the same table the engine hook encodes.
        assert_eq!(move_priority(MoveId::QuickAttack), 1);
        assert_eq!(move_priority(MoveId::Counter), -1);
        assert_eq!(move_priority(MoveId::Thundershock), 0);

        for (name, case) in matrix() {
            let legacy = legacy_first_mover(&case);
            let engine = engine_first_mover(&case);
            assert_eq!(
                engine, legacy,
                "first-mover mismatch for case '{name}': engine={engine:?} legacy={legacy:?}"
            );
        }
    }

    #[test]
    fn engine_and_legacy_agree_on_full_equal_speed_coin_flip_sweep() {
        // Sweep every coin-flip byte at equal speed: the engine's encoded
        // tie-break must reproduce `byte < 128 => PlayerFirst` for all 256
        // values, drawing exactly one byte each time.
        for byte in 0u16..=255 {
            let case = Case {
                player_speed: 80,
                enemy_speed: 80,
                player_move: MoveId::Thundershock,
                enemy_move: MoveId::Thundershock,
                player_paralyzed: false,
                enemy_paralyzed: false,
                rng_byte: byte as u8,
            };
            assert_eq!(
                engine_first_mover(&case),
                legacy_first_mover(&case),
                "coin-flip mismatch at byte={byte}"
            );
        }
    }
}
