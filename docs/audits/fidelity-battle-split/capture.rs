use pokered_core::battle::{BattleScreen, BattlePhase, BattleInput};
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{species::Species, moves::MoveId};
use pokered_app::render::{draw_battle, BattleVisualEffects};
use pokered_renderer::{FrameBuffer, Rgba};
use pokered_renderer::RenderConfig;
use pokered_renderer::resource::{AssetRoot, ResourceManager};
fn main() {
 let player = create_pokemon_with_moves(Species::Snorlax,50,[255;2],[MoveId::Mimic,MoveId::None,MoveId::None,MoveId::None]).unwrap();
 let enemy = create_pokemon_with_moves(Species::Snorlax,50,[255;2],[MoveId::Growl,MoveId::Tackle,MoveId::None,MoveId::None]).unwrap();
 let mut screen = BattleScreen::from_parties(true,&[player],&[enemy],None);
 screen.rng=pokered_core::battle::pokered_rules::runtime::StdBattleRng::from_seed(42);
 screen.phase=BattlePhase::PlayerMenu;
 screen.update_frame(BattleInput {a:true,..BattleInput::none()});
 screen.update_frame(BattleInput::none());
 screen.update_frame(BattleInput {a:true,..BattleInput::none()});
 let mut res=Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
 let mut fb=FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);
 draw_battle(&screen,&mut res,&mut fb,&mut BattleVisualEffects::default(),pokered_core::game_state::Lang::En);
 fb.save_png(std::path::Path::new(&std::env::args().nth(1).unwrap())).unwrap();
 eprintln!("phase {:?}",screen.phase);
}
