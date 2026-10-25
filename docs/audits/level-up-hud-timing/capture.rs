use pokered_app::render::{draw_battle, BattleVisualEffects};
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::game_state::Lang;
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};
use pokered_renderer::resource::{AssetRoot, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;
fn main() {
    let output=std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let capture_frame=std::env::args().nth(2).map_or(3, |s| s.parse::<u32>().unwrap());
    let mut player=create_pokemon_with_moves(Species::Mewtwo,20,[0x9a,0x78],
        [MoveId::DoubleKick,MoveId::None,MoveId::None,MoveId::None]).unwrap();
    player.hp-=7;player.set_nickname("ACE");
    let mut enemy=create_pokemon_with_moves(Species::Snorlax,100,[0x9a,0x78],
        [MoveId::Splash,MoveId::None,MoveId::None,MoveId::None]).unwrap();enemy.hp=1;
    let mut battle=BattleScreen::from_parties(true,&[player],&[enemy],None);
    battle.link_rng=Some(LinkRng::new([255,255,0,255,3,82,73,64,55,46]));
    battle.link_enemy_move_override=Some((MoveId::Splash,0));
    let state=battle.battle_state.as_mut().unwrap();state.player.active_mon_mut().speed=100;state.enemy.active_mon_mut().speed=200;
    battle.set_presentation_enabled(true);battle.phase=BattlePhase::PlayerMenu;
    let mut effects=BattleVisualEffects::default();let mut resources=Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
    let mut inputs=Vec::new();
    for frame in 0..=capture_frame {
        if battle.presentation.waiting && effects.is_frame_stable() {battle.complete_move_presentation();}
        let mut input=BattleInput::none();input.a=frame==0 || frame==2 || (frame>2 && frame%16==0);inputs.push(u8::from(input.a));
        battle.update_frame(input);effects.update(&battle);
        let mut fb=FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);
        draw_battle(&battle,&mut resources,&mut fb,&mut effects,Lang::En);
        if frame==capture_frame {
            fb.save_png(&output).unwrap();println!("INPUTS {inputs:?}");
            println!("CAPTURE frame={frame} level={} hp={} max_hp={} draws={} message={:?}",battle.player_level,battle.player_hp,battle.player_max_hp,battle.link_rng.as_ref().unwrap().consumed(),battle.current_message);
        }
    }
}
