use pokered_data::pokemon_data::BaseStats;

pub fn extract_atk_iv(dv_bytes: [u8; 2]) -> u8 {
    (dv_bytes[0] >> 4) & 0x0F
}

pub fn extract_def_iv(dv_bytes: [u8; 2]) -> u8 {
    dv_bytes[0] & 0x0F
}

pub fn extract_spd_iv(dv_bytes: [u8; 2]) -> u8 {
    (dv_bytes[1] >> 4) & 0x0F
}

pub fn extract_spc_iv(dv_bytes: [u8; 2]) -> u8 {
    dv_bytes[1] & 0x0F
}

pub fn extract_hp_iv(dv_bytes: [u8; 2]) -> u8 {
    ((extract_atk_iv(dv_bytes) & 1) << 3)
        | ((extract_def_iv(dv_bytes) & 1) << 2)
        | ((extract_spd_iv(dv_bytes) & 1) << 1)
        | (extract_spc_iv(dv_bytes) & 1)
}

/// `⌈√stat_exp⌉` — exact integer ceiling square root.
///
/// Replaces the original `(stat_exp as f64).sqrt().ceil()`; for every
/// reachable input (u16, ≤ 65535) the two agree: f64 sqrt is correctly
/// rounded, and at these magnitudes (root ≤ 256, ~2⁻⁴⁵ ulp) the rounding
/// never crosses an integer boundary, so the result is exactly the smallest
/// `n` with `n * n >= stat_exp` — identical stats on hosted and bare metal
/// (where f64 math is unavailable).
pub(crate) fn ceil_sqrt_u16(x: u16) -> u16 {
    if x == 0 {
        return 0;
    }
    // Bit-by-bit floor isqrt (exact for any u32), then bump to the ceiling.
    let mut n: u32 = 0;
    let mut bit: u32 = 1 << 30; // highest power of 4 in u32
    let mut v = x as u32;
    while bit > v {
        bit >>= 2;
    }
    while bit != 0 {
        if v >= n + bit {
            v -= n + bit;
            n = (n >> 1) + bit;
        } else {
            n >>= 1;
        }
        bit >>= 2;
    }
    if n * n < x as u32 {
        n += 1;
    }
    n as u16
}

fn stat_exp_contribution(stat_exp: u16) -> u16 {
    let s = ceil_sqrt_u16(stat_exp);
    s / 4
}

pub fn calc_stat(base: u8, iv: u8, stat_exp: u16, level: u8, is_hp: bool) -> u16 {
    let ev_term = stat_exp_contribution(stat_exp);
    let temp = ((base as u16 + iv as u16) * 2 + ev_term) as u32;
    let stat = (temp * level as u32) / 100;
    let bonus = if is_hp { level as u32 + 10 } else { 5 };
    let result = stat + bonus;
    result.min(999) as u16
}

pub fn calc_all_stats(
    base_stats: &BaseStats,
    dv_bytes: [u8; 2],
    stat_exp: &[u16; 5],
    level: u8,
) -> (u16, u16, u16, u16, u16) {
    let hp = calc_stat(
        base_stats.hp,
        extract_hp_iv(dv_bytes),
        stat_exp[0],
        level,
        true,
    );
    let atk = calc_stat(
        base_stats.attack,
        extract_atk_iv(dv_bytes),
        stat_exp[1],
        level,
        false,
    );
    let def = calc_stat(
        base_stats.defense,
        extract_def_iv(dv_bytes),
        stat_exp[2],
        level,
        false,
    );
    let spd = calc_stat(
        base_stats.speed,
        extract_spd_iv(dv_bytes),
        stat_exp[3],
        level,
        false,
    );
    let spc = calc_stat(
        base_stats.special,
        extract_spc_iv(dv_bytes),
        stat_exp[4],
        level,
        false,
    );
    (hp, atk, def, spd, spc)
}
