//! M5 determinism proofs: seeded RNG + frame-level fork/restore.
//!
//! - Replay: save_state, run a scripted input sequence into grass until
//!   a wild battle and fight, restore, replay the exact same inputs →
//!   identical snapshot hash.
//! - Fork: restore, run different inputs → different hash.
//! - Battle: same seed → same outcome across fresh games; different
//!   seed → different trajectory.
//! - Script engine: save mid-cutscene (Oak escort's FollowNpc), restore,
//!   run on → identical hash (interpreter stack round-trips).

use dotzuki_app::InputState;
use dotzuki_renderer::input::GbButton;
use pokered_app::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::GameScreen;
use pokered_core::overworld::{Direction, OverworldScreen};
use pokered_core::pokemon::stats::create_pokemon;
use pokered_core::save::SaveData;
use pokered_data::{impl_traits::PokemonRedData, maps::MapId, species::Species};

/// Start position note: (10, 6) is the open column north of Red's house —
/// (10, 9) sits in a tile-pair pocket that can't be exited northbound.
fn game_at_pallet(seed: u64, x: u16, y: u16) -> PokemonGame {
    let mut game = PokemonGame::new_with_options(
        GameVersion::Red,
        None,
        None,
        None,
        false,
        None,
        false,
        true,
        #[cfg(feature = "debug-server")]
        None,
    );
    game.save_data = SaveData::new();
    game.state.screen = GameScreen::Overworld;
    game.overworld = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    game.overworld.state.player.x = x;
    game.overworld.state.player.y = y;
    game.overworld.state.player.facing = Direction::Down;
    game.save_data
        .party
        .add(create_pokemon(Species::Charmander, 8, [0x9a, 0x78]).unwrap())
        .unwrap();
    game.overworld.party_count = 1;
    game.overworld.party_lead_level = 8;
    // Story flags that gate raw geography (Oak escort already done).
    game.overworld.set_flag_live("EVENT_FOLLOWED_OAK_INTO_LAB", true);
    game.overworld.set_flag_live("EVENT_GOT_STARTER", true);
    game.set_seed(seed);
    game
}

/// Drive the game with an exact per-frame input list.
fn drive(game: &mut PokemonGame, frames: &[Option<GbButton>]) {
    for button in frames {
        let mut input = InputState::new();
        if let Some(button) = button {
            input.press(*button);
        }
        game.update(&input);
    }
}

fn held(button: GbButton, frames: usize) -> Vec<Option<GbButton>> {
    std::iter::repeat_n(Some(button), frames).collect()
}

/// The replay sequence: walk north through the connection onto Route 1
/// (60 tiles at 8 frames/tile), then pace the grass until a wild battle
/// starts, then confirm the first move a few times.
fn sequence_a() -> Vec<Option<GbButton>> {
    let mut frames = held(GbButton::Up, 520);
    // Pace up/down in the grass to roll encounters.
    for _ in 0..14 {
        frames.extend(held(GbButton::Down, 9));
        frames.extend(held(GbButton::Up, 9));
    }
    // Battle interaction: menu confirms (FIGHT → first move) with gaps.
    for _ in 0..6 {
        frames.push(Some(GbButton::A));
        frames.extend(held(GbButton::A, 0));
        frames.extend(std::iter::repeat_n(None, 30));
    }
    frames
}

#[test]
fn save_restore_replays_identically() {
    let mut game = game_at_pallet(42, 10, 6);
    game.agent_save_state_slot(0).unwrap();

    let inputs = sequence_a();
    drive(&mut game, &inputs);
    let h1 = game.agent_save_state_slot(1).unwrap();
    // Precondition: the sequence reached Route 1 and a wild battle.
    assert_eq!(game.overworld.state.current_map, MapId::Route1);
    let saw_battle = matches!(game.state.screen, GameScreen::Battle)
        || game.battle.battle_state.is_some()
        || game.save_data.party.get(0).map_or(0, |m| m.hp) < game.save_data.party.get(0).unwrap().max_hp;
    assert!(saw_battle, "sequence should have produced a wild battle");

    game.agent_restore_state_slot(0).unwrap();
    drive(&mut game, &inputs);
    let h1_replay = game.agent_save_state_slot(1).unwrap();
    assert_eq!(h1, h1_replay, "restored + replayed inputs must reproduce the state");
}

#[test]
fn fork_diverges_with_different_inputs() {
    let mut game = game_at_pallet(42, 10, 6);
    game.agent_save_state_slot(0).unwrap();
    drive(&mut game, &sequence_a());
    let h1 = game.agent_save_state_slot(1).unwrap();

    game.agent_restore_state_slot(0).unwrap();
    // Different inputs: south instead of the grass pacing.
    let mut other = held(GbButton::Up, 520);
    other.extend(held(GbButton::Down, 120));
    drive(&mut game, &other);
    let h2 = game.agent_save_state_slot(1).unwrap();
    assert_ne!(h1, h2, "different inputs must diverge the fork");
}

#[test]
fn battle_determinism_same_seed_same_outcome() {
    fn run_battle(seed: u64) -> (u16, u16, u16) {
        let mut game = game_at_pallet(seed, 10, 9);
        game.debug_start_trainer_battle(pokered_data::trainer_data::TrainerClass::BugCatcher, 1);
        game.state.screen = GameScreen::Battle;
        // Spam A through the whole battle (bounded).
        for _ in 0..900 {
            let mut input = InputState::new();
            input.press(GbButton::A);
            game.update(&input);
            game.update(&InputState::new());
            if game.state.screen != GameScreen::Battle {
                break;
            }
        }
        (
            game.battle.player_hp,
            game.battle.enemy_hp,
            game.save_data.party.get(0).map_or(0, |m| m.hp),
        )
    }
    let a1 = run_battle(7);
    let a2 = run_battle(7);
    assert_eq!(a1, a2, "same seed must produce the same battle trajectory");
    // A different seed must change the trajectory. A single battle can
    // legitimately repeat a short trajectory (a 2HKO with no crits), so
    // check a small seed set and require at least one divergence — a
    // bounded flake risk, not seed-fishing.
    let diverged = [8u64, 9, 10, 11].iter().any(|&s| run_battle(s) != a1);
    assert!(diverged, "no seed in 8..=11 changed the trajectory — seeding is broken");
}

#[test]
fn script_engine_snapshot_round_trips_mid_cutscene() {
    // Start Oak's north-exit escort (a real storyline with a FollowNpc
    // script effect), run a few frames into it, fork, run on, and
    // require identical follow-through after restore.
    let mut game = PokemonGame::new_with_options(
        GameVersion::Red,
        None,
        None,
        None,
        false,
        None,
        false,
        true,
        #[cfg(feature = "debug-server")]
        None,
    );
    game.save_data = SaveData::new();
    game.state.screen = GameScreen::Overworld;
    game.overworld = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    game.save_data
        .party
        .add(create_pokemon(Species::Charmander, 8, [0x9a, 0x78]).unwrap())
        .unwrap();
    game.overworld.party_count = 1;
    game.overworld.party_lead_level = 8;
    game.set_seed(99);
    // Step onto the north-exit trigger: the escort storyline starts.
    game.overworld.state.player.x = 10;
    game.overworld.state.player.y = 2;
    game.overworld.state.player.facing = Direction::Up;
    drive(&mut game, &held(GbButton::Up, 16));
    assert!(
        game.overworld.active_script_effect_label().is_some()
            || !game.overworld.script_engine_idle()
            || game.overworld.pending_dialogue.is_some(),
        "the Oak event must be running"
    );
    // Advance a bounded number of frames into the cutscene (dialogue +
    // walk-up), then fork.
    for _ in 0..120 {
        game.update(&InputState::new());
    }
    game.agent_save_state_slot(0).unwrap();
    for _ in 0..240 {
        game.update(&InputState::new());
    }
    let h1 = game.agent_save_state_slot(1).unwrap();
    game.agent_restore_state_slot(0).unwrap();
    for _ in 0..240 {
        game.update(&InputState::new());
    }
    let h2 = game.agent_save_state_slot(1).unwrap();
    assert_eq!(h1, h2, "script-engine state must round-trip through fork/restore");
}
