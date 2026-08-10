#[cfg(test)]
mod tests {
    use crate::battle::state::*;
    use crate::battle::trainer_ai::ai_action::*;
    use crate::battle::trainer_ai::AiRoutine;
    use pokered_data::moves::MoveId;
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;

    fn make_mon_hp(hp: u16, max_hp: u16, status: StatusCondition) -> Pokemon {
        Pokemon {
            species: Species::Mew,
            nickname: [0x50; 11],
            level: 50,
            hp,
            max_hp,
            attack: 100,
            defense: 100,
            speed: 100,
            special: 100,
            type1: PokemonType::Normal,
            type2: PokemonType::Normal,
            moves: [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            pp: [10, 0, 0, 0],
            pp_ups: [0; 4],
            status,
            dv_bytes: [0xFF, 0xFF],
            stat_exp: [0; 5],
            total_exp: 0,
            is_traded: false, ot_id: 0, ot_name: [0x50; 11],
        }
    }

    fn make_battler_hp(hp: u16, max_hp: u16, status: StatusCondition) -> BattlerState {
        new_battler_state(vec![make_mon_hp(hp, max_hp, status)])
    }

    // --- Generic ---

    #[test]
    fn generic_always_does_nothing() {
        let enemy = make_battler_hp(100, 200, StatusCondition::None);
        let mut count = 3;
        let action = execute_ai_action(AiRoutine::Generic, &mut count, &enemy, 0);
        assert_eq!(action, AiAction::DoNothing);
        assert_eq!(count, 3); // no charge consumed
    }

    // --- Juggler ---

    #[test]
    fn juggler_switches_below_threshold() {
        let enemy = make_battler_hp(100, 200, StatusCondition::None);
        let mut count = 3;
        let action = execute_ai_action(AiRoutine::Juggler, &mut count, &enemy, 63);
        assert_eq!(action, AiAction::SwitchPokemon);
        assert_eq!(count, 2);
    }

    #[test]
    fn juggler_does_nothing_above_threshold() {
        let enemy = make_battler_hp(100, 200, StatusCondition::None);
        let mut count = 3;
        let action = execute_ai_action(AiRoutine::Juggler, &mut count, &enemy, 64);
        assert_eq!(action, AiAction::DoNothing);
        assert_eq!(count, 3);
    }

    // --- Blackbelt ---

    #[test]
    fn blackbelt_uses_x_attack() {
        let enemy = make_battler_hp(100, 200, StatusCondition::None);
        let mut count = 2;
        let action = execute_ai_action(AiRoutine::Blackbelt, &mut count, &enemy, 32);
        assert_eq!(action, AiAction::UseXAttack);
        assert_eq!(count, 1);
    }

    #[test]
    fn blackbelt_does_nothing_above() {
        let enemy = make_battler_hp(100, 200, StatusCondition::None);
        let mut count = 2;
        let action = execute_ai_action(AiRoutine::Blackbelt, &mut count, &enemy, 33);
        assert_eq!(action, AiAction::DoNothing);
    }

    // --- Giovanni ---

    #[test]
    fn giovanni_uses_guard_spec() {
        let enemy = make_battler_hp(100, 200, StatusCondition::None);
        let mut count = 1;
        let action = execute_ai_action(AiRoutine::Giovanni, &mut count, &enemy, 0);
        assert_eq!(action, AiAction::UseGuardSpec);
        assert_eq!(count, 0);
    }

    // --- CooltrainerM ---

    #[test]
    fn cooltrainer_m_uses_x_attack() {
        let enemy = make_battler_hp(100, 200, StatusCondition::None);
        let mut count = 2;
        let action = execute_ai_action(AiRoutine::CooltrainerM, &mut count, &enemy, 10);
        assert_eq!(action, AiAction::UseXAttack);
    }

    // --- CooltrainerF (bugged) ---

    #[test]
    fn cooltrainer_f_heals_when_low_hp() {
        // HP 19 out of 100 → below 100/5=20 → heals
        let enemy = make_battler_hp(19, 100, StatusCondition::None);
        let mut count = 1;
        let action = execute_ai_action(AiRoutine::CooltrainerF, &mut count, &enemy, 255);
        assert_eq!(action, AiAction::UsePotion { heal_amount: 200 });
    }

    #[test]
    fn cooltrainer_f_switches_when_hp_ok_and_rand_low() {
        // HP 50 out of 100 → not below 20% → check rand < 51
        let enemy = make_battler_hp(50, 100, StatusCondition::None);
        let mut count = 1;
        let action = execute_ai_action(AiRoutine::CooltrainerF, &mut count, &enemy, 50);
        assert_eq!(action, AiAction::SwitchPokemon);
    }

    #[test]
    fn cooltrainer_f_does_nothing_when_hp_ok_and_rand_high() {
        let enemy = make_battler_hp(50, 100, StatusCondition::None);
        let mut count = 1;
        let action = execute_ai_action(AiRoutine::CooltrainerF, &mut count, &enemy, 51);
        assert_eq!(action, AiAction::DoNothing);
    }

    // --- Brock ---

    #[test]
    fn brock_uses_full_heal_when_status() {
        let enemy = make_battler_hp(100, 200, StatusCondition::Poison);
        let mut count = 5;
        let action = execute_ai_action(AiRoutine::Brock, &mut count, &enemy, 0);
        assert_eq!(action, AiAction::UseFullHeal);
        assert_eq!(count, 4);
    }

    #[test]
    fn brock_does_nothing_when_healthy() {
        let enemy = make_battler_hp(100, 200, StatusCondition::None);
        let mut count = 5;
        let action = execute_ai_action(AiRoutine::Brock, &mut count, &enemy, 0);
        assert_eq!(action, AiAction::DoNothing);
        assert_eq!(count, 5);
    }

    // --- Bruno, Misty, LtSurge, Koga ---

    #[test]
    fn bruno_uses_x_defend() {
        let enemy = make_battler_hp(100, 200, StatusCondition::None);
        let mut count = 2;
        let action = execute_ai_action(AiRoutine::Bruno, &mut count, &enemy, 0);
        assert_eq!(action, AiAction::UseXDefend);
    }

    #[test]
    fn misty_uses_x_defend() {
        let enemy = make_battler_hp(100, 200, StatusCondition::None);
        let mut count = 1;
        let action = execute_ai_action(AiRoutine::Misty, &mut count, &enemy, 10);
        assert_eq!(action, AiAction::UseXDefend);
    }

    #[test]
    fn lt_surge_uses_x_speed() {
        let enemy = make_battler_hp(100, 200, StatusCondition::None);
        let mut count = 1;
        let action = execute_ai_action(AiRoutine::LtSurge, &mut count, &enemy, 10);
        assert_eq!(action, AiAction::UseXSpeed);
    }

    #[test]
    fn koga_uses_x_attack() {
        let enemy = make_battler_hp(100, 200, StatusCondition::None);
        let mut count = 2;
        let action = execute_ai_action(AiRoutine::Koga, &mut count, &enemy, 10);
        assert_eq!(action, AiAction::UseXAttack);
    }

    // --- Erika ---

    #[test]
    fn erika_heals_when_low_hp_and_rand_ok() {
        // max_hp=200, 10%=20, hp=19 → heals
        let enemy = make_battler_hp(19, 200, StatusCondition::None);
        let mut count = 1;
        let action = execute_ai_action(AiRoutine::Erika, &mut count, &enemy, 127);
        assert_eq!(action, AiAction::UsePotion { heal_amount: 50 });
    }

    #[test]
    fn erika_does_nothing_when_rand_high() {
        let enemy = make_battler_hp(19, 200, StatusCondition::None);
        let mut count = 1;
        let action = execute_ai_action(AiRoutine::Erika, &mut count, &enemy, 128);
        assert_eq!(action, AiAction::DoNothing);
    }

    // --- Blaine (no HP check!) ---

    #[test]
    fn blaine_heals_even_at_full_hp() {
        let enemy = make_battler_hp(200, 200, StatusCondition::None);
        let mut count = 2;
        let action = execute_ai_action(AiRoutine::Blaine, &mut count, &enemy, 10);
        assert_eq!(action, AiAction::UsePotion { heal_amount: 50 });
    }

    // --- Sabrina ---

    #[test]
    fn sabrina_heals_when_low_and_rand_ok() {
        let enemy = make_battler_hp(19, 200, StatusCondition::None);
        let mut count = 1;
        let action = execute_ai_action(AiRoutine::Sabrina, &mut count, &enemy, 0);
        assert_eq!(action, AiAction::UsePotion { heal_amount: 200 });
    }

    // --- Rival2 ---

    #[test]
    fn rival2_heals_when_low_and_rand_ok() {
        let enemy = make_battler_hp(19, 100, StatusCondition::None);
        let mut count = 1;
        let action = execute_ai_action(AiRoutine::Rival2, &mut count, &enemy, 0);
        assert_eq!(action, AiAction::UsePotion { heal_amount: 20 });
    }

    #[test]
    fn rival2_does_nothing_when_hp_ok() {
        let enemy = make_battler_hp(50, 100, StatusCondition::None);
        let mut count = 1;
        let action = execute_ai_action(AiRoutine::Rival2, &mut count, &enemy, 0);
        assert_eq!(action, AiAction::DoNothing);
    }

    // --- Rival3 ---

    #[test]
    fn rival3_full_restores_when_low() {
        let enemy = make_battler_hp(19, 100, StatusCondition::None);
        let mut count = 1;
        let action = execute_ai_action(AiRoutine::Rival3, &mut count, &enemy, 0);
        assert_eq!(action, AiAction::UsePotion { heal_amount: 0 });
    }

    // --- Lorelei ---

    #[test]
    fn lorelei_heals_when_low_and_rand_ok() {
        let enemy = make_battler_hp(19, 100, StatusCondition::None);
        let mut count = 2;
        let action = execute_ai_action(AiRoutine::Lorelei, &mut count, &enemy, 0);
        assert_eq!(action, AiAction::UsePotion { heal_amount: 50 });
    }

    // --- Agatha ---

    #[test]
    fn agatha_switches_on_very_low_rand() {
        let enemy = make_battler_hp(100, 200, StatusCondition::None);
        let mut count = 2;
        let action = execute_ai_action(AiRoutine::Agatha, &mut count, &enemy, 19);
        assert_eq!(action, AiAction::SwitchPokemon);
    }

    #[test]
    fn agatha_heals_when_low_hp_and_rand_mid() {
        // rand=50 → not < 20 (no switch), not >= 128 (passes gate), HP 49/200 < 200/4=50 → heal
        let enemy = make_battler_hp(49, 200, StatusCondition::None);
        let mut count = 2;
        let action = execute_ai_action(AiRoutine::Agatha, &mut count, &enemy, 50);
        assert_eq!(action, AiAction::UsePotion { heal_amount: 50 });
    }

    // --- Lance ---

    #[test]
    fn lance_heals_when_low_and_rand_ok() {
        let enemy = make_battler_hp(19, 100, StatusCondition::None);
        let mut count = 1;
        let action = execute_ai_action(AiRoutine::Lance, &mut count, &enemy, 0);
        assert_eq!(action, AiAction::UsePotion { heal_amount: 200 });
    }

    // --- ai_count exhaustion ---

    #[test]
    fn zero_ai_count_always_does_nothing() {
        let enemy = make_battler_hp(1, 200, StatusCondition::Poison);
        let mut count = 0;
        let action = execute_ai_action(AiRoutine::Brock, &mut count, &enemy, 0);
        assert_eq!(action, AiAction::DoNothing);
        assert_eq!(count, 0);
    }

    #[test]
    fn ai_count_decrements_to_zero() {
        let enemy = make_battler_hp(100, 200, StatusCondition::None);
        let mut count = 1;
        let action = execute_ai_action(AiRoutine::Juggler, &mut count, &enemy, 0);
        assert_eq!(action, AiAction::SwitchPokemon);
        assert_eq!(count, 0);
        // Next call with count=0 → DoNothing
        let action2 = execute_ai_action(AiRoutine::Juggler, &mut count, &enemy, 0);
        assert_eq!(action2, AiAction::DoNothing);
    }
}
