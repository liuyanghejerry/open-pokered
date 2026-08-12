#[cfg(test)]
mod tests {
    use super::super::level_up::*;
    use crate::battle::state::*;
    use pokered_data::moves::MoveId;
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;

    fn make_mon_at(species: Species, level: u8, total_exp: u32) -> Pokemon {
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
            total_exp,
            is_traded: false, ot_id: 0, ot_name: [0x50; 11],
        }
    }

    #[test]
    fn no_level_up_if_not_enough_exp() {
        let mut mon = make_mon_at(Species::Bulbasaur, 5, 100);
        let result = process_level_up(&mut mon);
        assert!(!result.leveled_up);
        assert_eq!(mon.level, 5);
    }

    #[test]
    fn level_up_when_exp_sufficient() {
        // Bulbasaur MediumSlow: level 6 needs 179 EXP, level 7 needs 236
        let mut mon = make_mon_at(Species::Bulbasaur, 5, 200);
        let result = process_level_up(&mut mon);
        assert!(result.leveled_up);
        assert_eq!(result.old_level, 5);
        assert_eq!(result.new_level, 6);
        assert_eq!(mon.level, 6);
    }

    #[test]
    fn multi_level_up() {
        // Bulbasaur: level 5 -> give massive EXP to reach level 10+
        // MediumSlow level 10 needs 560
        let mut mon = make_mon_at(Species::Bulbasaur, 5, 1000);
        let result = process_level_up(&mut mon);
        assert!(result.leveled_up);
        assert!(result.new_level >= 10);
    }

    #[test]
    fn hp_increases_on_level_up() {
        // Bulbasaur at level 5 with realistic HP, then level to 6
        // Level 5 HP: (45+15)*2 = 120, 120*5/100=6, +5+10=21
        let mut mon = make_mon_at(Species::Bulbasaur, 5, 200);
        mon.hp = 21;
        mon.max_hp = 21;
        process_level_up(&mut mon);
        // Level 6 HP: 120*6/100=7, +6+10=23
        assert_eq!(mon.max_hp, 23);
        assert_eq!(mon.hp, 23); // hp increased by same delta (2)
    }

    #[test]
    fn learns_move_at_level() {
        // Bulbasaur learns LeechSeed at level 7
        // MediumSlow level 7 needs exp_for_level(MediumSlow, 7)
        // 6/5*343 - 15*49 + 700 - 140 = 411 - 735 + 700 - 140 = 236? Let's use a high value.
        let mut mon = make_mon_at(Species::Bulbasaur, 6, 500);
        let result = process_level_up(&mut mon);
        assert!(result.leveled_up);
        assert!(result.new_level >= 7);
        assert!(result.learned_moves.contains(&MoveId::LeechSeed));
    }

    #[test]
    fn does_not_learn_duplicate_move() {
        let mut mon = make_mon_at(Species::Bulbasaur, 6, 500);
        // Already knows LeechSeed
        mon.moves[1] = MoveId::LeechSeed;
        mon.pp[1] = 10;
        let result = process_level_up(&mut mon);
        // Should not re-learn LeechSeed
        assert!(!result.learned_moves.contains(&MoveId::LeechSeed));
    }
}
