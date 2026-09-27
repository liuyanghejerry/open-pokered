//! Diagnostic fixture for connection, encounter and move frame pacing.
use pokered_app::game::PokemonGame;
use pokered_core::{game_state::GameScreen, pokemon::stats::create_pokemon};
use pokered_data::{maps::MapId, moves::MoveId, species::Species};
use pokered_renderer::input::{GbButton, InputState};

const MOVES: [MoveId; 10] = [
    MoveId::Tackle,
    MoveId::Growl,
    MoveId::Scratch,
    MoveId::Ember,
    MoveId::WaterGun,
    MoveId::Thunderbolt,
    MoveId::ThunderWave,
    MoveId::Splash,
    MoveId::DoubleTeam,
    MoveId::Explosion,
];

/// Emulator capture hook: scene and relative tick of the committed page.
/// Updated during VBlank, never included in playable builds.
#[no_mangle]
pub static mut FRAME_TIMING_VIEW: [u32; 2] = [0; 2];

#[derive(Default)]
pub struct Trace {
    pub scene: u32,
    stage: u8,
    since: u32,
    move_case: usize,
}

impl Trace {
    pub fn view(&self, frame: u32) -> [u32; 2] {
        [self.scene, frame.saturating_sub(self.since)]
    }

    pub fn drive(&mut self, game: &mut PokemonGame, frame: u32, input: &mut InputState) {
        if frame < 4300 {
            return;
        }
        input.set_from_bitmask(0);
        let elapsed = frame - self.since;
        match self.stage {
            0 => {
                game.overworld.set_rng_seed(42);
                game.save_data.party.clear();
                game.save_data
                    .party
                    .add(create_pokemon(Species::Charmander, 20, [0x99; 2]).unwrap())
                    .unwrap();
                for flag in [
                    "EVENT_GOT_STARTER",
                    "EVENT_GOT_POKEDEX",
                    "EVENT_FOLLOWED_OAK_INTO_LAB",
                ] {
                    game.overworld.set_flag_live(flag, true);
                }
                game.overworld.warp_to_map(MapId::PalletTown, 10, 1);
                game.handle_transition(GameScreen::Overworld);
                self.since = frame;
                self.stage = 1;
            }
            1 if elapsed >= 90 => {
                self.scene = 1;
                agb::println!("timing: START pallet-route1 tick={}", frame);
                self.since = frame;
                self.stage = 2;
            }
            2 => {
                input.press(GbButton::Up);
                if elapsed >= 70 {
                    assert_eq!(game.overworld.state.current_map, MapId::Route1);
                    self.scene = 0;
                    game.overworld.warp_to_map(MapId::Route1, 10, 1);
                    self.stage = 3;
                    self.since = frame;
                }
            }
            3 if elapsed >= 90 => {
                self.scene = 2;
                agb::println!("timing: START route1-viridian tick={}", frame);
                self.since = frame;
                self.stage = 4;
            }
            4 => {
                input.press(GbButton::Up);
                if elapsed >= 70 {
                    assert_eq!(game.overworld.state.current_map, MapId::ViridianCity);
                    self.scene = 0;
                    game.overworld.warp_to_map(MapId::Route1, 10, 28);
                    self.stage = 5;
                    self.since = frame;
                }
            }
            5 if elapsed >= 90 => {
                self.scene = 3;
                agb::println!("timing: START wild-entry tick={}", frame);
                // The same pending encounter consumed by an actual grass roll;
                // construction remains inside the measured update, not setup.
                game.overworld.pending_wild_encounter =
                    Some(pokered_core::overworld::screen::PendingWildEncounter {
                        species: Species::Pidgey,
                        level: 5,
                        old_man: false,
                        hooked: false,
                    });
                self.stage = 6;
                self.since = frame;
            }
            6 => {
                if matches!(
                    game.battle.phase,
                    pokered_core::battle::BattlePhase::Intro { .. }
                ) && elapsed % 32 < 16
                {
                    input.press(GbButton::A);
                }
                if elapsed >= 700 {
                    assert_eq!(game.state.screen, GameScreen::Battle);
                    self.stage = 7;
                    self.since = frame;
                    self.scene = 0;
                }
            }
            7 => {
                if self.move_case == MOVES.len() * 2 {
                    agb::println!("timing: DONE tick={}", frame);
                    self.stage = 9;
                    self.scene = 0;
                    return;
                }
                let mv = MOVES[self.move_case / 2];
                let player = self.move_case % 2 == 0;
                game.debug_start_memory_move(mv, player);
                self.scene = 10 + self.move_case as u32;
                agb::println!(
                    "timing: START move={} player={} scene={} tick={}",
                    mv as u8,
                    player,
                    self.scene,
                    frame
                );
                self.since = frame;
                self.stage = 8;
            }
            8 if elapsed > 0 && game.debug_memory_move_finished() => {
                agb::println!("timing: END scene={} ticks={}", self.scene, elapsed);
                self.move_case += 1;
                self.stage = 7;
                self.scene = 0;
            }
            _ => {}
        }
    }
}
