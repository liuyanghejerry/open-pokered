//! One-shot reproduction driver for the Oak's-lab rival battle freeze
//! (`--features repro-rival`). Routes through the production trainer-battle
//! setup (`debug_start_trainer_battle` → `start_trainer_battle`), which is
//! the path real hardware froze on.

use pokered_app::game::PokemonGame;
use pokered_renderer::input::{GbButton, InputState};

pub const START_AT: u32 = 600;

/// Largest currently-allocatable block, via fallible reserve probing.
/// try_reserve_exact reports allocation failure instead of aborting, so a
/// failed probe is safe to walk back from.
fn largest_free_block() -> usize {
    let mut lo = 0usize;
    let mut hi = 256 * 1024usize;
    while lo < hi {
        let mid = lo + (hi - lo + 1) / 2;
        let mut v: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
        if v.try_reserve_exact(mid).is_ok() {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

pub fn drive(game: &mut PokemonGame, frame: u32, state: &mut InputState) {
    if frame == START_AT - 10 {
        agb::println!("repro: heap before battle: {}B", largest_free_block());
    }
    if frame == START_AT {
        use pokered_core::pokemon::stats::create_pokemon;
        use pokered_data::species::Species;
        agb::println!("repro: seeding starter party");
        if game.save_data.party.is_empty() {
            if let Some(mon) = create_pokemon(Species::Bulbasaur, 5, [0x9A, 0x78]) {
                let _ = game.save_data.party.add(mon);
            }
        }
        agb::println!("repro: starting Rival1 trainer battle (party index 2)");
        game.debug_start_trainer_battle(pokered_data::trainer_data::TrainerClass::Rival1, 2);
        agb::println!("repro: battle entry returned");
    }
    // Advance battle dialogue so the intro state machine keeps moving through
    // trainer reveal, send-out, and the fight menu.
    if frame > START_AT && (frame - START_AT) % 64 < 8 {
        state.press(GbButton::A);
    }
    if frame % 600 == 0 {
        agb::println!("repro: alive at frame {} heap {}", frame, largest_free_block());
    }
}
