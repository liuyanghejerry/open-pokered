use crate::alloc_prelude::*;
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

/// End-of-battle winnings text (`_MoneyForWinningText`, text_2.asm:867):
/// "<PLAYER> got $<prize> for winning!". EndOfBattle prints Pay Day separately.
pub fn trainer_winnings_messages(player_name: &str, prize_money: u32) -> Vec<String> {
    if prize_money == 0 {
        return Vec::new();
    }
    vec![format!(
        "{} got ${} for\nwinning!",
        player_name, prize_money
    )]
}

/// `_PickUpPayDayMoneyText` (text_2.asm:1343), before EvolutionAfterBattle.
pub fn payday_pickup_message(player_name: &str, payday_bonus: u32) -> String {
    format!("{} picked up\n${}!", player_name, payday_bonus)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winnings_and_payday_are_distinct_original_messages() {
        let msgs = trainer_winnings_messages("RED", 210);
        assert_eq!(msgs, vec!["RED got $210 for\nwinning!"]);
        assert_eq!(payday_pickup_message("RED", 40), "RED picked up\n$40!");
        assert_eq!(calc_total_winnings(210, 40), 250);
        assert!(trainer_winnings_messages("RED", 0).is_empty());
    }
}
