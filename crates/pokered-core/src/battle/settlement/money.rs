use pokered_data::trainer_data::{get_base_money, TrainerClass};

/// Prize money = base_money × level of last enemy Pokémon.
/// In the original game this is done via BCD addition looped `level` times;
/// we just multiply directly since we don't need BCD compatibility.
pub fn calc_prize_money(trainer_class: TrainerClass, last_mon_level: u8) -> u32 {
    let base = get_base_money(trainer_class) as u32;
    base * last_mon_level as u32
}

/// On blackout, player loses half their money (integer division).
pub fn calc_blackout_penalty(player_money: u32) -> u32 {
    player_money / 2
}

/// Total money gained = prize money + Pay Day bonus.
/// Capped at 999_999 (max displayable in Gen 1).
pub fn calc_total_winnings(prize_money: u32, payday_bonus: u32) -> u32 {
    let total = prize_money.saturating_add(payday_bonus);
    total.min(999_999)
}
