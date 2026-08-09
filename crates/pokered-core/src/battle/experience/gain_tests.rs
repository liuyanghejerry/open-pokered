#[cfg(test)]
mod tests {
    use super::super::gain::*;
    use crate::battle::state::*;
    use pokered_data::moves::MoveId;
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;

    fn make_mon(species: Species, level: u8, total_exp: u32, is_traded: bool) -> Pokemon {
        Pokemon {
            species,
            nickname: None,
            level,
            hp: 100,
            max_hp: 100,
            attack: 50,
            defense: 50,
            speed: 50,
            special: 50,
            type1: PokemonType::Normal,
            type2: PokemonType::Normal,
            moves: [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            pp: [35, 0, 0, 0],
            pp_ups: [0; 4],
            status: StatusCondition::None,
            dv_bytes: [0xFF, 0xFF],
            stat_exp: [0; 5],
            total_exp,
            is_traded,
            ot_id: 0,
            ot_name: None,
        }
    }

    #[test]
    fn calc_exp_gain_basic() {
        // base_exp=64, enemy_level=10: 64*10/7 = 91
        assert_eq!(calc_exp_gain(64, 10, false, false), 91);
    }

    #[test]
    fn calc_exp_gain_traded_boost() {
        // 91 * 3/2 = 136
        assert_eq!(calc_exp_gain(64, 10, true, false), 136);
    }

    #[test]
    fn calc_exp_gain_trainer_boost() {
        // 91 * 3/2 = 136
        assert_eq!(calc_exp_gain(64, 10, false, true), 136);
    }

    #[test]
    fn calc_exp_gain_both_boosts() {
        // 91 * 3/2 = 136, then 136 * 3/2 = 204
        assert_eq!(calc_exp_gain(64, 10, true, true), 204);
    }

    #[test]
    fn add_stat_exp_accumulates() {
        let mut mon = make_mon(Species::Pikachu, 25, 0, false);
        let base = pokered_data::pokemon_data::get_base_stats(Species::Pikachu).unwrap();
        add_stat_exp(&mut mon, base);
        assert_eq!(mon.stat_exp[0], 35); // hp
        assert_eq!(mon.stat_exp[1], 55); // atk
        assert_eq!(mon.stat_exp[2], 30); // def
        assert_eq!(mon.stat_exp[3], 90); // spd
        assert_eq!(mon.stat_exp[4], 50); // spc
    }

    #[test]
    fn add_stat_exp_saturates_at_max() {
        let mut mon = make_mon(Species::Pikachu, 25, 0, false);
        mon.stat_exp = [0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF];
        let base = pokered_data::pokemon_data::get_base_stats(Species::Pikachu).unwrap();
        add_stat_exp(&mut mon, base);
        assert_eq!(mon.stat_exp[0], 0xFFFF);
    }

    #[test]
    fn gain_experience_single_mon() {
        // Bulbasaur at level 10 defeat a level 10 Pidgey (base_exp=55)
        // 55*10/7=78 EXP gained, not enough to level from 10 to 11
        let mon = make_mon(Species::Bulbasaur, 10, 560, false);
        let mut state = new_battle_state(
            BattleType::Wild,
            vec![mon],
            vec![make_mon(Species::Pidgey, 10, 0, false)],
        );
        state.party_gain_exp_flags[0] = true;

        let result = gain_experience(&mut state, Species::Pidgey, 10, false);
        // 560 + 78 = 638, level 11 needs 742, so no level up
        assert_eq!(state.player.party[0].total_exp, 638);
        assert!(result.leveled_up.is_empty());
    }

    #[test]
    fn gain_experience_triggers_level_up() {
        // Bulbasaur at level 5 with 215 EXP. Gaining 78 → 293.
        // Level 7 needs 236 ≤ 293, so levels up from 5 to 7.
        let mon = make_mon(Species::Bulbasaur, 5, 215, false);
        let mut state = new_battle_state(
            BattleType::Wild,
            vec![mon],
            vec![make_mon(Species::Pidgey, 10, 0, false)],
        );
        state.party_gain_exp_flags[0] = true;

        let result = gain_experience(&mut state, Species::Pidgey, 10, false);
        assert_eq!(result.leveled_up, vec![0]);
        assert!(state.player.party[0].level > 5);
    }

    #[test]
    fn no_gainers_no_exp() {
        let mon = make_mon(Species::Bulbasaur, 5, 0, false);
        let mut state = new_battle_state(
            BattleType::Wild,
            vec![mon],
            vec![make_mon(Species::Pidgey, 10, 0, false)],
        );
        // No flags set
        let result = gain_experience(&mut state, Species::Pidgey, 10, false);
        assert!(result.leveled_up.is_empty());
        assert_eq!(state.player.party[0].total_exp, 0);
    }

    #[test]
    fn fainted_participant_gains_no_exp() {
        // experience.asm:9-11 — a fainted mon (HP 0) is SKIPPED even when its
        // gain-exp flag is set (it still counts toward the division).
        // Pidgey base_exp=55, level 10; 2 flagged mons ⇒ divided base 27 ⇒
        // 27*10/7 = 38 EXP for the survivor; the fainted one gets NOTHING.
        let alive = make_mon(Species::Bulbasaur, 10, 0, false);
        let mut fainted = make_mon(Species::Charmander, 10, 0, false);
        fainted.hp = 0;
        let mut state = new_battle_state(
            BattleType::Wild,
            vec![alive, fainted],
            vec![make_mon(Species::Pidgey, 10, 0, false)],
        );
        state.party_gain_exp_flags[0] = true;
        state.party_gain_exp_flags[1] = true;

        gain_experience(&mut state, Species::Pidgey, 10, false);
        assert_eq!(state.player.party[0].total_exp, 38, "split 55/2=27 ⇒ 27*10/7=38");
        assert_eq!(state.player.party[1].total_exp, 0, "fainted mon gains no EXP");
        // Stat EXP is divided too (DivideExpDataByNumMonsGainingExp divides the
        // base STATS): Pidgey [40,45,40,56,35] / 2 = [20,22,20,28,17].
        assert_eq!(state.player.party[0].stat_exp, [20, 22, 20, 28, 17]);
        assert_eq!(state.player.party[1].stat_exp, [0; 5], "fainted mon gains no stat EXP");
    }

    #[test]
    fn two_participants_split_exp_and_stat_exp() {
        // Without EXP ALL the enemy data is divided by the participant count —
        // base EXP AND the base stats feeding stat EXP.
        let mut state = new_battle_state(
            BattleType::Wild,
            vec![
                make_mon(Species::Bulbasaur, 10, 0, false),
                make_mon(Species::Charmander, 10, 0, false),
            ],
            vec![make_mon(Species::Pidgey, 10, 0, false)],
        );
        state.party_gain_exp_flags[0] = true;
        state.party_gain_exp_flags[1] = true;

        gain_experience(&mut state, Species::Pidgey, 10, false);
        for i in 0..2 {
            assert_eq!(state.player.party[i].total_exp, 38, "mon {i}: 55/2=27 ⇒ 38 EXP");
            assert_eq!(state.player.party[i].stat_exp, [20, 22, 20, 28, 17], "mon {i}: divided stat EXP");
        }
    }

    #[test]
    fn exp_all_two_pass_division() {
        // EXP ALL (core.asm:818-857): data HALVED, pass 1 to participants
        // (divided by participants), pass 2 to the whole party — divided AGAIN,
        // by the party count, on the already-divided data (in-place quirk).
        // Party of 2, one participant, Pidgey (base_exp 55) at level 10:
        //   halved 27; pass 1 (1 gainer ⇒ no division): 27*10/7 = 38 to mon 0.
        //   pass 2 (party 2 ⇒ 27/2 = 13): 13*10/7 = 18 to BOTH.
        let mut state = new_battle_state(
            BattleType::Wild,
            vec![
                make_mon(Species::Bulbasaur, 10, 0, false),
                make_mon(Species::Charmander, 10, 0, false),
            ],
            vec![make_mon(Species::Pidgey, 10, 0, false)],
        );
        state.party_gain_exp_flags[0] = true;

        gain_experience(&mut state, Species::Pidgey, 10, true);
        assert_eq!(state.player.party[0].total_exp, 38 + 18, "participant: both passes");
        assert_eq!(state.player.party[1].total_exp, 18, "bench: pass 2 only");
        // Stat EXP: halved Pidgey [20,22,20,28,17] to mon 0 (pass 1); pass 2
        // divides again by 2 ⇒ [10,11,10,14,8] to both.
        assert_eq!(state.player.party[0].stat_exp, [30, 33, 30, 42, 25]);
        assert_eq!(state.player.party[1].stat_exp, [10, 11, 10, 14, 8]);
    }

    #[test]
    fn exp_all_pass_two_skips_fainted() {
        // With EXP ALL the pass-2 loop covers the WHOLE party — but fainted
        // mons are still skipped (experience.asm:9-11).
        let mut fainted = make_mon(Species::Charmander, 10, 0, false);
        fainted.hp = 0;
        let mut state = new_battle_state(
            BattleType::Wild,
            vec![make_mon(Species::Bulbasaur, 10, 0, false), fainted],
            vec![make_mon(Species::Pidgey, 10, 0, false)],
        );
        state.party_gain_exp_flags[0] = true;

        gain_experience(&mut state, Species::Pidgey, 10, true);
        assert_eq!(state.player.party[0].total_exp, 38 + 18);
        assert_eq!(state.player.party[1].total_exp, 0, "fainted bench mon gains nothing");
        assert_eq!(state.player.party[1].stat_exp, [0; 5]);
    }
}
