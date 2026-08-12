#[cfg(test)]
mod tests {
    use crate::battle::state::*;
    use crate::battle::trainer_ai::move_choice::*;
    use crate::battle::trainer_ai::MoveChoiceLayer;
    use pokered_data::moves::MoveId;
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;

    fn make_mon(
        moves: [MoveId; 4],
        type1: PokemonType,
        type2: PokemonType,
        status: StatusCondition,
    ) -> Pokemon {
        Pokemon {
            species: Species::Mew,
            nickname: [0x50; 11],
            level: 50,
            hp: 200,
            max_hp: 200,
            attack: 100,
            defense: 100,
            speed: 100,
            special: 100,
            type1,
            type2,
            moves,
            pp: [10, 10, 10, 10],
            pp_ups: [0; 4],
            status,
            dv_bytes: [0xFF, 0xFF],
            stat_exp: [0; 5],
            total_exp: 0,
            is_traded: false, ot_id: 0, ot_name: [0x50; 11],
        }
    }

    fn make_battler(mon: Pokemon) -> BattlerState {
        new_battler_state(vec![mon])
    }

    #[test]
    fn no_layers_all_moves_equal() {
        let enemy_mon = make_mon(
            [MoveId::Tackle, MoveId::Scratch, MoveId::Pound, MoveId::Slam],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let player_mon = make_mon(
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let enemy = make_battler(enemy_mon);
        let player = make_battler(player_mon);

        let result = choose_moves(&[], &enemy, &player, 0);
        assert_eq!(result.candidates, [1, 1, 1, 1]);
    }

    #[test]
    fn empty_move_slots_excluded() {
        let enemy_mon = make_mon(
            [MoveId::Tackle, MoveId::Scratch, MoveId::None, MoveId::None],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let player_mon = make_mon(
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let enemy = make_battler(enemy_mon);
        let player = make_battler(player_mon);

        let result = choose_moves(&[], &enemy, &player, 0);
        assert_eq!(result.candidates, [1, 1, 0, 0]);
    }

    #[test]
    fn disabled_move_excluded() {
        let enemy_mon = make_mon(
            [MoveId::Tackle, MoveId::Scratch, MoveId::Pound, MoveId::Slam],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let player_mon = make_mon(
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let mut enemy = make_battler(enemy_mon);
        enemy.disabled_move = 2; // move slot 2 (Scratch) disabled
        let player = make_battler(player_mon);

        let result = choose_moves(&[], &enemy, &player, 0);
        // Disabled move gets 0x50, others get 10. Min is 10, so disabled excluded.
        assert_eq!(result.candidates[0], 1);
        assert_eq!(result.candidates[1], 0);
        assert_eq!(result.candidates[2], 1);
        assert_eq!(result.candidates[3], 1);
    }

    #[test]
    fn layer1_discourages_status_when_player_has_status() {
        let enemy_mon = make_mon(
            [
                MoveId::Tackle,
                MoveId::ThunderWave,
                MoveId::Hypnosis,
                MoveId::Pound,
            ],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let player_mon = make_mon(
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::Paralysis,
        );
        let enemy = make_battler(enemy_mon);
        let player = make_battler(player_mon);

        let result = choose_moves(&[MoveChoiceLayer::Layer1], &enemy, &player, 0);
        // ThunderWave(ParalyzeEffect) and Hypnosis(SleepEffect) discouraged
        // Tackle and Pound should be candidates
        assert_eq!(result.candidates[0], 1); // Tackle
        assert_eq!(result.candidates[1], 0); // ThunderWave discouraged
        assert_eq!(result.candidates[2], 0); // Hypnosis discouraged
        assert_eq!(result.candidates[3], 1); // Pound
    }

    #[test]
    fn layer1_no_effect_when_player_healthy() {
        let enemy_mon = make_mon(
            [
                MoveId::Tackle,
                MoveId::ThunderWave,
                MoveId::Pound,
                MoveId::None,
            ],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let player_mon = make_mon(
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let enemy = make_battler(enemy_mon);
        let player = make_battler(player_mon);

        let result = choose_moves(&[MoveChoiceLayer::Layer1], &enemy, &player, 0);
        assert_eq!(result.candidates, [1, 1, 1, 0]);
    }

    #[test]
    fn layer2_encourages_stat_moves_when_flag_is_1() {
        let enemy_mon = make_mon(
            [
                MoveId::Tackle,
                MoveId::SwordsDance,
                MoveId::Agility,
                MoveId::Pound,
            ],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let player_mon = make_mon(
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let enemy = make_battler(enemy_mon);
        let player = make_battler(player_mon);

        let result = choose_moves(&[MoveChoiceLayer::Layer2], &enemy, &player, 1);
        // SwordsDance(AttackUp2Effect=0x32) and Agility(SpeedUp2Effect=0x34) both in range
        // They get encouraged (score 9), Tackle and Pound stay at 10
        // Min is 9, so only stat moves selected
        assert_eq!(result.candidates[0], 0); // Tackle (score 10, not min)
        assert_eq!(result.candidates[1], 1); // SwordsDance (score 9, min)
        assert_eq!(result.candidates[2], 1); // Agility (score 9, min)
        assert_eq!(result.candidates[3], 0); // Pound (score 10, not min)
    }

    #[test]
    fn layer2_no_effect_when_flag_is_0() {
        let enemy_mon = make_mon(
            [
                MoveId::Tackle,
                MoveId::SwordsDance,
                MoveId::Pound,
                MoveId::None,
            ],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let player_mon = make_mon(
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let enemy = make_battler(enemy_mon);
        let player = make_battler(player_mon);

        let result = choose_moves(&[MoveChoiceLayer::Layer2], &enemy, &player, 0);
        assert_eq!(result.candidates, [1, 1, 1, 0]);
    }

    #[test]
    fn layer3_encourages_super_effective() {
        let enemy_mon = make_mon(
            [
                MoveId::Tackle,
                MoveId::Thunderbolt,
                MoveId::Pound,
                MoveId::None,
            ],
            PokemonType::Electric,
            PokemonType::Electric,
            StatusCondition::None,
        );
        let player_mon = make_mon(
            [MoveId::Surf, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Water,
            PokemonType::Water,
            StatusCondition::None,
        );
        let enemy = make_battler(enemy_mon);
        let player = make_battler(player_mon);

        let result = choose_moves(&[MoveChoiceLayer::Layer3], &enemy, &player, 0);
        // Thunderbolt vs Water = super effective → encouraged (score 9)
        // Tackle/Pound are Normal vs Water = neutral (score 10)
        assert_eq!(result.candidates[0], 0); // Tackle neutral
        assert_eq!(result.candidates[1], 1); // Thunderbolt SE
        assert_eq!(result.candidates[2], 0); // Pound neutral
    }

    #[test]
    fn layer3_discourages_not_effective_when_better_exists() {
        let enemy_mon = make_mon(
            [
                MoveId::Surf,
                MoveId::Thunderbolt,
                MoveId::None,
                MoveId::None,
            ],
            PokemonType::Water,
            PokemonType::Water,
            StatusCondition::None,
        );
        let player_mon = make_mon(
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Water,
            PokemonType::Water,
            StatusCondition::None,
        );
        let enemy = make_battler(enemy_mon);
        let player = make_battler(player_mon);

        let result = choose_moves(&[MoveChoiceLayer::Layer3], &enemy, &player, 0);
        // Surf vs Water = NVE → discouraged (11) because Thunderbolt is a different-type damaging move
        // Thunderbolt vs Water = neutral (10)
        assert_eq!(result.candidates[0], 0); // Surf NVE
        assert_eq!(result.candidates[1], 1); // Thunderbolt neutral (min)
    }

    #[test]
    fn layer3_no_discourage_if_no_better_move() {
        let enemy_mon = make_mon(
            [MoveId::Surf, MoveId::WaterGun, MoveId::None, MoveId::None],
            PokemonType::Water,
            PokemonType::Water,
            StatusCondition::None,
        );
        let player_mon = make_mon(
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Water,
            PokemonType::Water,
            StatusCondition::None,
        );
        let enemy = make_battler(enemy_mon);
        let player = make_battler(player_mon);

        let result = choose_moves(&[MoveChoiceLayer::Layer3], &enemy, &player, 0);
        // Both Water vs Water = NVE, but no "better move" exists (same type)
        // So neither discouraged, both stay at 10
        assert_eq!(result.candidates, [1, 1, 0, 0]);
    }

    #[test]
    fn combined_layers_1_and_3() {
        let enemy_mon = make_mon(
            [
                MoveId::ThunderWave,
                MoveId::Thunderbolt,
                MoveId::Tackle,
                MoveId::None,
            ],
            PokemonType::Electric,
            PokemonType::Electric,
            StatusCondition::None,
        );
        let player_mon = make_mon(
            [MoveId::Surf, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Water,
            PokemonType::Water,
            StatusCondition::Paralysis,
        );
        let enemy = make_battler(enemy_mon);
        let player = make_battler(player_mon);

        let result = choose_moves(
            &[MoveChoiceLayer::Layer1, MoveChoiceLayer::Layer3],
            &enemy,
            &player,
            0,
        );
        // Layer1: ThunderWave discouraged +5 (player has status) → 15
        // Layer3: Thunderbolt SE → 9, Tackle neutral → 10
        // Min = 9 (Thunderbolt)
        assert_eq!(result.candidates[0], 0); // ThunderWave
        assert_eq!(result.candidates[1], 1); // Thunderbolt
        assert_eq!(result.candidates[2], 0); // Tackle
    }

    #[test]
    fn pick_move_selects_from_candidates() {
        let result = MoveChoiceResult {
            candidates: [0, 1, 0, 1],
        };
        assert_eq!(result.pick_move(0), Some(1));
        assert_eq!(result.pick_move(1), Some(3));
        assert_eq!(result.pick_move(2), Some(1)); // wraps: 2 % 2 = 0
    }

    #[test]
    fn pick_move_returns_none_when_empty() {
        let result = MoveChoiceResult {
            candidates: [0, 0, 0, 0],
        };
        assert_eq!(result.pick_move(0), None);
    }

    #[test]
    fn single_move_always_selected() {
        let enemy_mon = make_mon(
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let player_mon = make_mon(
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let enemy = make_battler(enemy_mon);
        let player = make_battler(player_mon);

        let result = choose_moves(&[], &enemy, &player, 0);
        assert_eq!(result.candidates, [1, 0, 0, 0]);
    }

    #[test]
    fn layer1_only_affects_zero_power_status_moves() {
        let enemy_mon = make_mon(
            [MoveId::Tackle, MoveId::BodySlam, MoveId::None, MoveId::None],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::None,
        );
        let player_mon = make_mon(
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Normal,
            PokemonType::Normal,
            StatusCondition::Paralysis,
        );
        let enemy = make_battler(enemy_mon);
        let player = make_battler(player_mon);

        let result = choose_moves(&[MoveChoiceLayer::Layer1], &enemy, &player, 0);
        // BodySlam has ParalyzeSideEffect2 which is NOT in the status-only list, AND power > 0
        assert_eq!(result.candidates, [1, 1, 0, 0]);
    }

    #[test]
    fn super_fang_counts_as_better_move() {
        let enemy_mon = make_mon(
            [MoveId::Surf, MoveId::SuperFang, MoveId::None, MoveId::None],
            PokemonType::Water,
            PokemonType::Water,
            StatusCondition::None,
        );
        let player_mon = make_mon(
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Water,
            PokemonType::Water,
            StatusCondition::None,
        );
        let enemy = make_battler(enemy_mon);
        let player = make_battler(player_mon);

        let result = choose_moves(&[MoveChoiceLayer::Layer3], &enemy, &player, 0);
        // Surf vs Water = NVE, SuperFang exists as "better" → Surf discouraged (11)
        // SuperFang vs Water: power=1, SpecialDamageEffect... actually SuperFangEffect
        // SuperFang has SuperFangEffect (0x28), not in SE/NVE path → stays 10
        // Min = 10 → SuperFang selected
        assert_eq!(result.candidates[0], 0); // Surf discouraged
        assert_eq!(result.candidates[1], 1); // SuperFang
    }
}
