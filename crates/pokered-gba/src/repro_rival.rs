//! Real-path reproduction driver for the Oak's-lab rival battle
//! (`--features repro-rival`, implies `autopilot` for the intro).
//!
//! The desktop-path debug entry (`debug_start_trainer_battle`) sidesteps the
//! lab scripts entirely; this driver walks the production flow instead:
//! warp into Oak's Lab with the starter flags set, step onto the exit-row
//! coord trigger (which decodes the split `battle_before` script and starts
//! the battle through `startBattle("OPP_RIVAL1")`), fight with A-mashes, and
//! run through the post-battle continuation decode.

use pokered_app::game::PokemonGame;
use pokered_renderer::input::{GbButton, InputState};

const SETUP_AT: u32 = 4300;

pub fn drive(game: &mut PokemonGame, frame: u32, state: &mut InputState) {
    if frame == SETUP_AT {
        use pokered_core::pokemon::stats::create_pokemon;
        use pokered_data::event_flags::EventFlag;
        use pokered_data::maps::MapId;
        use pokered_data::species::Species;

        agb::println!("repro: seeding starter + flags, warping to OaksLab");
        if game.save_data.party.is_empty() {
            if let Some(mon) = create_pokemon(Species::Bulbasaur, 5, [0x9A, 0x78]) {
                let _ = game.save_data.party.add(mon);
            }
        }
        game.overworld.set_event_flag_live(EventFlag::EVENT_OAK_ASKED_TO_CHOOSE_MON);
        game.overworld.set_event_flag_live(EventFlag::EVENT_GOT_STARTER);
        game.overworld.warp_to_map(MapId::OaksLab, 5, 7);
        game.overworld.party_count = game.save_data.party.count() as u8;
        game.overworld.party_lead_level = game.save_data.party.leader_level();
        agb::println!("repro: warp issued");
    }
    match frame {
        // Walk up onto row 6: the coord trigger fires and the rival scene
        // (split `battle_before`) runs, ending in the real battle start.
        f if (SETUP_AT + 60..SETUP_AT + 220).contains(&f) => {
            if (f - SETUP_AT) % 64 < 40 {
                state.press(GbButton::Up);
            }
        }
        // Advance dialogue and fight: A on text pages, A on FIGHT, A on the
        // first move — a plain mash keeps every menu one press deep.
        f if f > SETUP_AT + 220 && (f - SETUP_AT) % 64 < 8 => {
            state.press(GbButton::A);
        }
        _ => {}
    }
    // Post-battle (lab, heap loaded with script/pokemon state): probe the
    // largest free block, then run the production save path.
    static mut SAVED: bool = false;
    if frame >= SETUP_AT + 900 && !unsafe { SAVED } {
        unsafe { SAVED = true };
        agb::println!(
            "repro: lab post-battle largest free block {}B",
            largest_free_block()
        );
        agb::println!("repro: debug save start");
        game.debug_save_now();
        agb::println!("repro: debug save done");
    }
    // After the save, run a battle loop: every wild battle walks the full
    // battle resource load/free cycle, the best probe for heap fragmentation
    // and leaks that a short run cannot show.
    static mut BATTLES: u32 = 0;
    if frame >= SETUP_AT + 1200 && (frame - (SETUP_AT + 1200)) % 1500 == 0 {
        let n = unsafe { BATTLES };
        if n < 40 {
            unsafe { BATTLES = n + 1 };
            game.debug_start_wild_battle(pokered_data::species::Species::Pidgey, 5);
            agb::println!("repro: battle loop #{} at frame {}", n + 1, frame);
        }
    }
    // Battle-loop A-mash: fight to the end so the exit path (and its frees)
    // runs too.
    if frame >= SETUP_AT + 1200 && (frame - (SETUP_AT + 1200)) % 64 < 8 {
        state.press(GbButton::A);
    }
    // Periodic heap samples (not only new lows) for trend analysis.
    if frame % 1800 == 0 {
        agb::println!(
            "repro: heap sample frame={} free={}B",
            frame,
            pokered_app::game::largest_free_block()
        );
    }
    if frame % 600 == 0 {
        agb::println!("repro: alive at frame {}", frame);
    }
}

/// Largest currently-allocatable block, via fallible reserve probing.
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
