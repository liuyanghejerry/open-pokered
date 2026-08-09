#[cfg(test)]
mod tests {
    use super::super::growth::*;
    use pokered_data::species::GrowthRate;

    #[test]
    fn medium_fast_is_n_cubed() {
        assert_eq!(exp_for_level(GrowthRate::MediumFast, 1), 0);
        assert_eq!(exp_for_level(GrowthRate::MediumFast, 2), 8);
        assert_eq!(exp_for_level(GrowthRate::MediumFast, 10), 1000);
        assert_eq!(exp_for_level(GrowthRate::MediumFast, 100), 1000000);
    }

    #[test]
    fn fast_is_four_fifths_cubed() {
        // 4/5 * 10^3 = 800
        assert_eq!(exp_for_level(GrowthRate::Fast, 10), 800);
        // 4/5 * 100^3 = 800000
        assert_eq!(exp_for_level(GrowthRate::Fast, 100), 800000);
    }

    #[test]
    fn slow_is_five_fourths_cubed() {
        // 5/4 * 10^3 = 1250
        assert_eq!(exp_for_level(GrowthRate::Slow, 10), 1250);
        // 5/4 * 100^3 = 1250000
        assert_eq!(exp_for_level(GrowthRate::Slow, 100), 1250000);
    }

    #[test]
    fn medium_slow_formula() {
        // 6/5 * n^3 - 15*n^2 + 100*n - 140
        // n=10: 6/5*1000 - 15*100 + 100*10 - 140 = 1200 - 1500 + 1000 - 140 = 560
        assert_eq!(exp_for_level(GrowthRate::MediumSlow, 10), 560);
    }

    #[test]
    fn slightly_fast_formula() {
        // 3/4 * n^3 + 10*n^2 - 30
        // n=10: 750 + 1000 - 30 = 1720
        assert_eq!(exp_for_level(GrowthRate::SlightlyFast, 10), 1720);
    }

    #[test]
    fn slightly_slow_formula() {
        // 3/4 * n^3 + 20*n^2 - 70
        // n=10: 750 + 2000 - 70 = 2680
        assert_eq!(exp_for_level(GrowthRate::SlightlySlow, 10), 2680);
    }

    #[test]
    fn level_1_always_zero() {
        assert_eq!(exp_for_level(GrowthRate::MediumFast, 1), 0);
        assert_eq!(exp_for_level(GrowthRate::Fast, 1), 0);
        assert_eq!(exp_for_level(GrowthRate::Slow, 1), 0);
        assert_eq!(exp_for_level(GrowthRate::MediumSlow, 1), 0);
    }

    #[test]
    fn level_from_exp_basic() {
        // MediumFast: level 10 needs 1000, level 11 needs 1331
        assert_eq!(level_from_exp(GrowthRate::MediumFast, 999), 9);
        assert_eq!(level_from_exp(GrowthRate::MediumFast, 1000), 10);
        assert_eq!(level_from_exp(GrowthRate::MediumFast, 1330), 10);
        assert_eq!(level_from_exp(GrowthRate::MediumFast, 1331), 11);
    }

    #[test]
    fn level_from_exp_caps_at_100() {
        assert_eq!(level_from_exp(GrowthRate::MediumFast, 9999999), 100);
    }

    #[test]
    fn max_exp_values() {
        assert_eq!(max_exp(GrowthRate::MediumFast), 1000000);
        assert_eq!(max_exp(GrowthRate::Fast), 800000);
        assert_eq!(max_exp(GrowthRate::Slow), 1250000);
    }
}
