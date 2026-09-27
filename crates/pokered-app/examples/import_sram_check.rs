//! Import a raw 32 KiB SRAM image and report the result:
//!   cargo run --release -p pokered-app --example import_sram_check -- path/to/file.sav

use pokered_core::save::sram_import::import_sram;

fn main() {
    let path = std::env::args().nth(1).expect("usage: import_sram_check <file.sav>");
    let data = std::fs::read(&path).expect("read");
    println!("len={} (expected 32768)", data.len());
    {
        use pokered_core::save::sram_import::canonical_region_len;
        let len = canonical_region_len();
        let offset = 0x598usize;
        let region = &data[0x2000 + offset..0x2000 + offset + len];
        let computed = pokered_core::save_menu::calc_checksum(region);
        println!(
            "canonical_len={} checksum_offset={:#x} stored={:#04x} computed={:#04x}",
            len,
            offset + len,
            data[0x2000 + offset + len],
            computed
        );
    }
    match import_sram(&data) {
        Ok(save) => {
            println!(
                "IMPORT OK: player_id={} name={:?} party={} badges={}",
                save.game_data.player_id,
                pokered_data::charmap::decode_string(&save.player_name),
                save.party.count(),
                save.game_data.badge_count()
            );
        }
        Err(e) => println!("IMPORT FAILED: {:?}", e),
    }
}
