use pokered_data::species::GrowthRate;

/// Gen1 growth rate EXP table.
/// Each growth type uses a cubic polynomial:
///   E(n) = coeff_num/coeff_den * n^3 + quad * n^2 + linear * n - sub
/// Values from data/growth_rates.asm.
struct GrowthCoeffs {
    coeff_num: i64,
    coeff_den: i64,
    quad: i64,
    linear: i64,
    sub: i64,
}

const GROWTH_TABLE: [GrowthCoeffs; 6] = [
    // MediumFast (index 0): n^3
    GrowthCoeffs {
        coeff_num: 1,
        coeff_den: 1,
        quad: 0,
        linear: 0,
        sub: 0,
    },
    // SlightlyFast (index 1): 3/4 * n^3 + 10*n^2 - 30
    GrowthCoeffs {
        coeff_num: 3,
        coeff_den: 4,
        quad: 10,
        linear: 0,
        sub: 30,
    },
    // SlightlySlow (index 2): 3/4 * n^3 + 20*n^2 - 70
    GrowthCoeffs {
        coeff_num: 3,
        coeff_den: 4,
        quad: 20,
        linear: 0,
        sub: 70,
    },
    // MediumSlow (index 3): 6/5 * n^3 - 15*n^2 + 100*n - 140
    GrowthCoeffs {
        coeff_num: 6,
        coeff_den: 5,
        quad: -15,
        linear: 100,
        sub: 140,
    },
    // Fast (index 4): 4/5 * n^3
    GrowthCoeffs {
        coeff_num: 4,
        coeff_den: 5,
        quad: 0,
        linear: 0,
        sub: 0,
    },
    // Slow (index 5): 5/4 * n^3
    GrowthCoeffs {
        coeff_num: 5,
        coeff_den: 4,
        quad: 0,
        linear: 0,
        sub: 0,
    },
];

pub fn exp_for_level(growth_rate: GrowthRate, level: u8) -> u32 {
    if level <= 1 {
        return 0;
    }
    let n = level as i64;
    let c = &GROWTH_TABLE[growth_rate as usize];
    let n3 = n * n * n;
    let n2 = n * n;
    let raw = (c.coeff_num * n3) / c.coeff_den + c.quad * n2 + c.linear * n - c.sub;
    raw.max(0) as u32
}

pub fn level_from_exp(growth_rate: GrowthRate, total_exp: u32) -> u8 {
    for level in 2..=100u8 {
        if exp_for_level(growth_rate, level) > total_exp {
            return level - 1;
        }
    }
    100
}

pub fn max_exp(growth_rate: GrowthRate) -> u32 {
    exp_for_level(growth_rate, 100)
}
