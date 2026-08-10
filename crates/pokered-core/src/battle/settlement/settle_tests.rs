#[cfg(test)]
mod tests {
    use pokered_data::moves::MoveId;
    use pokered_data::species::Species;
    use pokered_data::trainer_data::TrainerClass;
    use pokered_data::types::PokemonType;

    use crate::battle::settlement::settle::settle_battle;
    use crate::battle::settlement::BattleOutcome;
    use crate::battle::state::*;

    fn make_test_pokemon(species: Species, level: u8) -> Pokemon {
        Pokemon {
            species,
            nickname: [0x50; 11],
            level,
            hp: 50,
            max_hp: 50,
            attack: 30,
            defense: 30,
            speed: 30,
            special: 30,
            type1: PokemonType::Normal,
            type2: PokemonType::Normal,
            moves: [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            pp: [35, 0, 0, 0],
            pp_ups: [0; 4],
            status: StatusCondition::None,
            dv_bytes: [0xFF, 0xFF],
            stat_exp: [0; 5],
            total_exp: 0,
            is_traded: false, ot_id: 0, ot_name: [0x50; 11],
        }
    }

    #[test]
    fn win_trainer_battle_gains_money() {
        let player_party = vec![make_test_pokemon(Species::Pikachu, 25)];
        let enemy_party = vec![make_test_pokemon(Species::Geodude, 14)];
        let mut state = new_battle_state(BattleType::Trainer, player_party, enemy_party);

        let result = settle_battle(
            &mut state,
            BattleOutcome::Win,
            Some(TrainerClass::Brock),
            5000,
        );

        assert_eq!(result.outcome, BattleOutcome::Win);
        assert_eq!(result.money_gained, 99 * 14);
        assert_eq!(result.money_lost, 0);
    }

    #[test]
    fn win_wild_battle_no_prize_money() {
        let player_party = vec![make_test_pokemon(Species::Pikachu, 25)];
        let enemy_party = vec![make_test_pokemon(Species::Rattata, 5)];
        let mut state = new_battle_state(BattleType::Wild, player_party, enemy_party);

        let result = settle_battle(&mut state, BattleOutcome::Win, None, 5000);

        assert_eq!(result.money_gained, 0);
    }

    #[test]
    fn win_with_payday_adds_bonus() {
        let player_party = vec![make_test_pokemon(Species::Pikachu, 25)];
        let enemy_party = vec![make_test_pokemon(Species::Rattata, 5)];
        let mut state = new_battle_state(BattleType::Wild, player_party, enemy_party);
        state.total_payday_money = 500;

        let result = settle_battle(&mut state, BattleOutcome::Win, None, 5000);

        assert_eq!(result.money_gained, 500);
        assert_eq!(result.payday_bonus, 500);
    }

    #[test]
    fn loss_halves_money() {
        let player_party = vec![make_test_pokemon(Species::Pikachu, 25)];
        let enemy_party = vec![make_test_pokemon(Species::Rattata, 5)];
        let mut state = new_battle_state(BattleType::Wild, player_party, enemy_party);

        let result = settle_battle(&mut state, BattleOutcome::Loss, None, 10_000);

        assert_eq!(result.outcome, BattleOutcome::Loss);
        assert_eq!(result.money_lost, 5_000);
        assert_eq!(result.money_gained, 0);
    }

    #[test]
    fn evolution_detected_on_win_after_level_up() {
        let player_party = vec![make_test_pokemon(Species::Bulbasaur, 16)];
        let enemy_party = vec![make_test_pokemon(Species::Rattata, 5)];
        let mut state = new_battle_state(BattleType::Wild, player_party, enemy_party);
        // wCanEvolveFlags: the mon leveled up during this battle.
        state.party_leveled_up_flags[0] = true;

        let result = settle_battle(&mut state, BattleOutcome::Win, None, 0);

        assert_eq!(result.evolutions.len(), 1);
        assert_eq!(result.evolutions[0].old_species, Species::Bulbasaur);
        assert_eq!(result.evolutions[0].new_species, Species::Ivysaur);
        // Detection only: the cutscene (and its B-cancel) decides; the party
        // mon is not mutated here.
        assert_eq!(state.player.party[0].species, Species::Bulbasaur);
    }

    /// Without the leveled-up-this-battle flag (wCanEvolveFlags), an eligible
    /// mon is NOT checked — this is what makes a B-cancelled evolution retry
    /// only on the next level-up, not after every battle.
    #[test]
    fn no_evolution_without_level_up_flag() {
        let player_party = vec![make_test_pokemon(Species::Bulbasaur, 16)];
        let enemy_party = vec![make_test_pokemon(Species::Rattata, 5)];
        let mut state = new_battle_state(BattleType::Wild, player_party, enemy_party);

        let result = settle_battle(&mut state, BattleOutcome::Win, None, 0);

        assert!(result.evolutions.is_empty());
        assert_eq!(state.player.party[0].species, Species::Bulbasaur);
    }

    #[test]
    fn no_evolution_if_fainted() {
        let mut mon = make_test_pokemon(Species::Bulbasaur, 16);
        mon.hp = 0;
        let player_party = vec![mon];
        let enemy_party = vec![make_test_pokemon(Species::Rattata, 5)];
        let mut state = new_battle_state(BattleType::Wild, player_party, enemy_party);
        state.party_leveled_up_flags[0] = true;

        let result = settle_battle(&mut state, BattleOutcome::Win, None, 0);

        assert!(result.evolutions.is_empty());
        assert_eq!(state.player.party[0].species, Species::Bulbasaur);
    }

    #[test]
    fn no_evolution_on_loss() {
        let player_party = vec![make_test_pokemon(Species::Bulbasaur, 16)];
        let enemy_party = vec![make_test_pokemon(Species::Rattata, 5)];
        let mut state = new_battle_state(BattleType::Wild, player_party, enemy_party);

        let result = settle_battle(&mut state, BattleOutcome::Loss, None, 0);

        assert!(result.evolutions.is_empty());
    }

    /// EndOfBattle runs EvolutionAfterBattle only when wBattleResult == 0
    /// (player won): `and a / jr nz, .resetVariables`
    /// (engine/battle/end_of_battle.asm:29-33). Running away sets
    /// wBattleResult = $2 (core.asm:2293), so no evolution check happens.
    #[test]
    fn escaped_skips_evolution() {
        let player_party = vec![make_test_pokemon(Species::Bulbasaur, 16)];
        let enemy_party = vec![make_test_pokemon(Species::Rattata, 5)];
        let mut state = new_battle_state(BattleType::Wild, player_party, enemy_party);
        state.party_leveled_up_flags[0] = true;

        let result = settle_battle(&mut state, BattleOutcome::Escaped, None, 0);

        assert!(result.evolutions.is_empty());
    }

    #[test]
    fn trainer_battle_prize_with_payday() {
        let player_party = vec![make_test_pokemon(Species::Pikachu, 25)];
        let enemy_party = vec![make_test_pokemon(Species::Geodude, 14)];
        let mut state = new_battle_state(BattleType::Trainer, player_party, enemy_party);
        state.total_payday_money = 200;

        let result = settle_battle(
            &mut state,
            BattleOutcome::Win,
            Some(TrainerClass::Brock),
            5000,
        );

        let expected = 99u32 * 14 + 200;
        assert_eq!(result.money_gained, expected);
    }
}
