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
/// "<PLAYER> got $<total> for winning!" — Pay Day money is part of the single
/// total; the original has no tip request or running-total pages.
pub fn trainer_winnings_messages(player_name: &str, total_winnings: u32) -> Vec<String> {
    if total_winnings == 0 {
        return Vec::new();
    }
    vec![format!(
        "{} got ${} for\nwinning!",
        player_name, total_winnings
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winnings_text_uses_player_name_and_total() {
        let msgs = trainer_winnings_messages("RED", calc_total_winnings(210, 0));
        assert_eq!(msgs, vec!["RED got $210 for\nwinning!"]);

        // Pay Day is folded into the single original amount.
        let msgs = trainer_winnings_messages("RED", calc_total_winnings(210, 40));
        assert_eq!(msgs, vec!["RED got $250 for\nwinning!"]);

        assert!(trainer_winnings_messages("RED", 0).is_empty());
    }
}
