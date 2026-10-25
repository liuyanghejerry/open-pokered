use pokered_app::render::{draw_battle, BattleVisualEffects};
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::game_state::Lang;
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species, types::PokemonType};
use pokered_renderer::resource::{AssetRoot, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;

fn main() {
    let output=std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let mut capture_frame=std::env::args().nth(2).and_then(|s|s.parse::<u32>().ok());
    let attacker=create_pokemon_with_moves(Species::Mewtwo,20,[0x9a,0x78],
        [MoveId::Thunderbolt,MoveId::IceBeam,MoveId::None,MoveId::None]).unwrap();
    let mut target=create_pokemon_with_moves(Species::Snorlax,100,[0x9a,0x78],
        [MoveId::Splash,MoveId::None,MoveId::None,MoveId::None]).unwrap();
    target.status=pokered_core::battle::state::StatusCondition::Sleep(7);
    let mut battle=BattleScreen::from_parties(true,&[attacker],&[target],None);
    battle.link_rng=Some(LinkRng::new([255,0,0,0,255,255,0,255,255,255]));
    battle.link_enemy_move_override=Some((MoveId::Splash,0));
    let state=battle.battle_state.as_mut().unwrap();
    state.player.active_mon_mut().speed=100;state.enemy.active_mon_mut().speed=200;
    state.enemy.conversion_type1=Some(PokemonType::Ground);
    state.enemy.conversion_type2=Some(PokemonType::Ground);
    battle.set_presentation_enabled(true);battle.phase=BattlePhase::PlayerMenu;
    let mut effects=BattleVisualEffects::default();
    let mut resources=Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
    let mut completed=0;let mut left_menu=false;let mut captured=false;let mut inputs=Vec::new();
    for frame in 0..3000 {
        if battle.presentation.waiting && effects.is_frame_stable() {battle.complete_move_presentation();}
        let mut input=BattleInput::none();
        input.a=(frame<600 && completed==0 && (frame==0 || frame==2 || (frame>2 && frame%16==0)))
            || frame==600 || frame==604 || (frame>604 && frame%16==0);
        input.down=frame==602;
        inputs.push(u8::from(input.a) | (u8::from(input.down)<<1));
        battle.update_frame(input);effects.update(&battle);
        if matches!(battle.phase,BattlePhase::PlayerMenu) {
            if left_menu {completed+=1;left_menu=false;battle.link_enemy_move_override=Some((MoveId::Splash,0));}
        } else {left_menu=true;}
        if capture_frame.is_none() && completed==1 && battle.current_message.as_ref().is_some_and(|m|m.contains("Critical hit!")) {capture_frame=Some(frame);}
        let mut fb=FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);
        draw_battle(&battle,&mut resources,&mut fb,&mut effects,Lang::En);
        if capture_frame==Some(frame) {
            fb.save_png(&output).unwrap();captured=true;
            println!("INPUTS {inputs:?}");
            println!("CAPTURE frame={frame} completed_turns={completed} enemy_display_hp={} draws={} message={:?} phase={:?}",battle.enemy_hp,battle.link_rng.as_ref().unwrap().consumed(),battle.current_message,battle.phase);
            break;
        }
    }
    assert!(captured);
}
