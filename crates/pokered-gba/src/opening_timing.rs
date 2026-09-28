//! Unskipped Gengar choreography, logo bounce and version slide.
use pokered_app::game::PokemonGame;
use pokered_core::{game_state::GameScreen, title_screen::TitlePhase};
use pokered_renderer::input::InputState;

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
        [self.scene, frame - self.since]
    }
    pub fn drive(&mut self, game: &mut PokemonGame, frame: u32, input: &mut InputState) {
        if frame < 600 {
            return;
        }
        input.set_from_bitmask(0);
        let next = match self.stage {
            0 => {
                game.handle_transition(GameScreen::IntroScene);
                Some(41)
            }
            1 if game.state.screen == GameScreen::TitleScreen => Some(42),
            2 if game.title_screen.phase == TitlePhase::VersionScroll => Some(43),
            3 if game.title_screen.phase == TitlePhase::WaitingForInput
                && frame - self.since > 90 =>
            {
                agb::println!("timing: DONE");
                Some(0)
            }
            _ => None,
        };
        if let Some(scene) = next {
            self.scene = scene;
            self.since = frame;
            self.stage += 1;
        }
    }
}
