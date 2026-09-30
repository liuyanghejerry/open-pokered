//! Gen-1 PrintSendOutMonMessage / RetreatMon (engine/battle/common_text.asm).
use crate::alloc_prelude::*;

fn hp_percentage(hp: u16, max_hp: u16) -> u8 {
    // The assembly divides by the LOW BYTE of (max HP >> 2), then reads
    // only the low quotient byte. Keep its rounding and wraparound quirks.
    let divisor = (max_hp >> 2) as u8;
    if divisor == 0 {
        return 0; // Defensive for invalid/custom fixtures; real Gen-1 HP >= 4.
    }
    (u32::from(hp) * 25 / u32::from(divisor)) as u8
}

pub(super) fn send_out(name: &str, hp: u16, max_hp: u16) -> String {
    let prefix = if hp == 0 {
        "Go!"
    } else {
        match hp_percentage(hp, max_hp) {
            70..=255 => "Go!",
            40..=69 => "Do it!",
            10..=39 => "Get'm!",
            _ => "The enemy's weak!\nGet'm!",
        }
    };
    format!("{prefix} {name}!")
}

pub(super) fn recall(name: &str, entry_hp: u16, hp: u16, max_hp: u16) -> String {
    let praise = match hp_percentage(entry_hp.wrapping_sub(hp), max_hp) {
        0 => " enough!",
        1..=29 => "",
        30..=69 => " OK!",
        _ => " good!",
    };
    format!("{name}{praise}\nCome back!")
}
