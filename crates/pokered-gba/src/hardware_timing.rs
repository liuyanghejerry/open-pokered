//! Reproduce the slow-cart report: intro, Red's naming slides, house doors.
//! Uses the same timer/display hooks as `frame-timing`; never in playable ROMs.
use pokered_app::game::PokemonGame;
use pokered_core::{
    game_state::GameScreen,
    oak_speech::{OakSpeechPhase, PicSlideDirection, PicSlideSubject},
};
use pokered_data::maps::MapId;
use pokered_renderer::input::{GbButton, InputState};

#[no_mangle]
pub static mut FRAME_TIMING_VIEW: [u32; 2] = [0; 2];

#[derive(Default)]
pub struct Trace {
    pub scene: u32,
    stage: u8,
    since: u32,
}

impl Trace {
    pub fn view(&self, frame: u32) -> [u32; 2] {
        [self.scene, frame.saturating_sub(self.since)]
    }

    fn start(&mut self, scene: u32, frame: u32) {
        self.scene = scene;
        self.since = frame;
        self.stage += 1;
    }

    pub fn drive(&mut self, game: &mut PokemonGame, frame: u32, input: &mut InputState) {
        if frame < 600 {
            return;
        }
        input.set_from_bitmask(0);
        let elapsed = frame - self.since;
        match self.stage {
            0 => {
                game.handle_transition(GameScreen::IntroScene);
                self.start(31, frame);
            }
            1 if elapsed >= 1000 => {
                game.handle_transition(GameScreen::OakSpeech);
                game.oak_speech.phase = OakSpeechPhase::IntroducePlayer {
                    page_index: 0,
                    char_index: 0,
                    waiting_for_input: false,
                };
                game.oak_speech.phase_frame = 0;
                self.start(32, frame);
            }
            2 if elapsed >= 100 => {
                game.oak_speech.phase = OakSpeechPhase::SlidePic {
                    subject: PicSlideSubject::Player,
                    direction: PicSlideDirection::Right,
                    frame: 0,
                };
                self.start(33, frame);
            }
            3 if elapsed >= 60 => {
                game.oak_speech.phase = OakSpeechPhase::SlidePic {
                    subject: PicSlideSubject::Player,
                    direction: PicSlideDirection::Left,
                    frame: 0,
                };
                self.start(34, frame);
            }
            4 if elapsed >= 70 => {
                for flag in [
                    "EVENT_GOT_STARTER",
                    "EVENT_GOT_POKEDEX",
                    "EVENT_FOLLOWED_OAK_INTO_LAB",
                ] {
                    game.overworld.set_flag_live(flag, true);
                }
                game.overworld.warp_to_map(MapId::PalletTown, 5, 6);
                game.handle_transition(GameScreen::Overworld);
                self.start(0, frame);
            }
            5 if elapsed >= 90 => self.start(35, frame),
            6 => {
                input.press(GbButton::Up);
                if elapsed >= 90 {
                    assert_eq!(game.overworld.state.current_map, MapId::RedsHouse1F);
                    self.start(0, frame);
                }
            }
            7 if elapsed >= 30 => self.start(36, frame),
            8 => {
                input.press(GbButton::Down);
                if elapsed >= 90 {
                    assert_eq!(game.overworld.state.current_map, MapId::PalletTown);
                    self.start(0, frame);
                    agb::println!("timing: DONE");
                }
            }
            _ => {}
        }
    }
}
