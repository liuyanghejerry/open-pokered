//! Pokemon adapter for dotzuki's shared mobile host.
use dotzuki_mobile::{MobileGame, MobileRuntime};
use pokered_app::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_renderer::input::InputState;
use pokered_renderer::{FrameBuffer, RenderConfig, Rgba};
struct PokemonMobile {
    game: PokemonGame,
    input: InputState,
    frame: FrameBuffer,
}
/// Initialization payload is the ASCII string `pokered:red:v1` or `pokered:blue:v1`.
/// Saves use Pokemon's versioned JSON envelope, not RunnerGame save JSON.
pub fn create(payload: Vec<u8>, save: Option<&str>) -> Result<MobileRuntime, String> {
    let version = match payload.as_slice() {
        b"pokered:red:v1" => GameVersion::Red,
        b"pokered:blue:v1" => GameVersion::Blue,
        _ => return Err("invalid pokered mobile initialization payload".into()),
    };
    MobileRuntime::new(PokemonMobile {
        game: PokemonGame::new_mobile(version, save)?,
        input: InputState::new(),
        frame: FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE),
    })
}
impl MobileGame for PokemonMobile {
    fn dimensions(&self) -> (u32, u32) {
        (160, 144)
    }
    fn tick(&mut self, bits: u8) {
        self.input.set_from_bitmask(bits);
        self.game.update(&self.input);
        self.input.begin_frame();
        self.game.draw(&mut self.frame);
    }
    fn copy_rgba(&self, output: &mut [u8]) {
        self.frame.to_rgba(output);
    }
    fn render_audio(&mut self, output: &mut [f32]) {
        if let Some(audio) = &self.game.audio {
            audio.render_pcm(output);
        } else {
            output.fill(0.0);
        }
    }
    fn export_save(&self) -> Option<String> {
        self.game.export_mobile_save()
    }
    fn import_save(&mut self, json: &str) -> bool {
        self.game.import_mobile_save(json).is_ok()
    }
}
dotzuki_mobile::export_mobile_abi!(create);
