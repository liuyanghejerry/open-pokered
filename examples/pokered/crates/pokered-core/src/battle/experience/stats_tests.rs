#[cfg(test)]
mod tests {
    use super::super::stats::*;
    use pokered_data::pokemon_data::get_base_stats;
    use pokered_data::species::Species;

    #[test]
    fn extract_ivs_all_max() {
        let dv = [0xFF, 0xFF];
        assert_eq!(extract_atk_iv(dv), 15);
        assert_eq!(extract_def_iv(dv), 15);
        assert_eq!(extract_spd_iv(dv), 15);
        assert_eq!(extract_spc_iv(dv), 15);
        assert_eq!(extract_hp_iv(dv), 15);
    }

    #[test]
    fn extract_ivs_all_zero() {
        let dv = [0x00, 0x00];
        assert_eq!(extract_atk_iv(dv), 0);
        assert_eq!(extract_def_iv(dv), 0);
        assert_eq!(extract_spd_iv(dv), 0);
        assert_eq!(extract_spc_iv(dv), 0);
        assert_eq!(extract_hp_iv(dv), 0);
    }

    #[test]
    fn extract_ivs_mixed() {
        // atk=0xA=10, def=0x5=5, spd=0x3=3, spc=0xC=12
        let dv = [0xA5, 0x3C];
        assert_eq!(extract_atk_iv(dv), 10);
        assert_eq!(extract_def_iv(dv), 5);
        assert_eq!(extract_spd_iv(dv), 3);
        assert_eq!(extract_spc_iv(dv), 12);
        // HP IV: bit3=(10&1)=0, bit2=(5&1)=1, bit1=(3&1)=1, bit0=(12&1)=0 => 0b0110 = 6
        assert_eq!(extract_hp_iv(dv), 6);
    }

    #[test]
    fn calc_stat_hp_no_evs() {
        // Pikachu base HP=35, IV=15, stat_exp=0, level=25, is_hp=true
        // ev_term = 0
        // temp = (35+15)*2 + 0 = 100
        // stat = 100*25/100 + 25+10 = 25 + 35 = 60
        let result = calc_stat(35, 15, 0, 25, true);
        assert_eq!(result, 60);
    }

    #[test]
    fn calc_stat_non_hp() {
        // Pikachu base Atk=55, IV=15, stat_exp=0, level=25
        // temp = (55+15)*2 = 140
        // stat = 140*25/100 + 5 = 35 + 5 = 40
        let result = calc_stat(55, 15, 0, 25, false);
        assert_eq!(result, 40);
    }

    #[test]
    fn calc_stat_with_stat_exp() {
        // stat_exp=10000 -> sqrt(10000)=100 -> ceil=100 -> /4 = 25
        // base=50, iv=8, ev_term=25, level=50
        // temp = (50+8)*2 + 25 = 141
        // stat = 141*50/100 + 5 = 70 + 5 = 75
        let result = calc_stat(50, 8, 10000, 50, false);
        assert_eq!(result, 75);
    }

    #[test]
    fn calc_stat_caps_at_999() {
        // Massive values should cap at 999
        let result = calc_stat(255, 15, 65535, 100, true);
        assert!(result <= 999);
    }

    #[test]
    fn calc_all_stats_pikachu() {
        let base = get_base_stats(Species::Pikachu).unwrap();
        let dv = [0xFF, 0xFF];
        let stat_exp = [0u16; 5];
        let (hp, atk, def, spd, spc) = calc_all_stats(base, dv, &stat_exp, 5);
        // At level 5 with max IVs and no stat EXP:
        // HP: (35+15)*2=100, 100*5/100=5, +5+10=20
        assert_eq!(hp, 20);
        // Atk: (55+15)*2=140, 140*5/100=7, +5=12
        assert_eq!(atk, 12);
        // Def: (30+15)*2=90, 90*5/100=4, +5=9
        assert_eq!(def, 9);
        // Spd: (90+15)*2=210, 210*5/100=10, +5=15
        assert_eq!(spd, 15);
        // Spc: (50+15)*2=130, 130*5/100=6, +5=11
        assert_eq!(spc, 11);
    }
}
