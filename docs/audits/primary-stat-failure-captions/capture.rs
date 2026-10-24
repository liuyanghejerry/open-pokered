use pokered_app::render::{draw_battle, BattleVisualEffects};
use pokered_core::battle::{state, BattleInput, BattlePhase, BattleScreen};
use pokered_core::game_state::Lang;
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};
use pokered_renderer::resource::{AssetRoot, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let wanted = std::env::args().nth(2).unwrap_or_else(|| "attack missed".into());
    std::fs::create_dir_all(&out).unwrap();
    for enemy in [false, true] {
        let mon = |species, mv| create_pokemon_with_moves(species, 30, [0x9a,0x78],
            [mv,MoveId::None,MoveId::None,MoveId::None]).unwrap();
        let attacker = mon(Species::Mewtwo, MoveId::Growl);
        let target = mon(Species::Snorlax, MoveId::Splash);
        let (player, foe) = if enemy { (target,attacker) } else { (attacker,target) };
        let mut battle = BattleScreen::from_parties(true, &[player], &[foe], None);
        battle.link_enemy_move_override = Some((if enemy {MoveId::Growl} else {MoveId::Splash},0));
        battle.link_rng = Some(LinkRng::new([64,0,82,73,64,55,46,37,28,19]));
        let state = battle.battle_state.as_mut().unwrap();
        let (attacker,target) = if enemy {(&mut state.enemy,&mut state.player)} else {(&mut state.player,&mut state.enemy)};
        attacker.active_mon_mut().speed=100;
        target.active_mon_mut().speed=200;
        target.set_status2(state::status2::PROTECTED_BY_MIST);
        battle.phase=BattlePhase::PlayerMenu;
        let mut a=BattleInput::none();a.a=true;
        battle.update_frame(a);battle.update_frame(a);
        let mut resources=Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
        let mut effects=BattleVisualEffects::default();
        let mut captured=false;
        for frame in 0..1000 {
            effects.update(&battle);
            if battle.current_message.as_ref().is_some_and(|m|m.split_whitespace().collect::<Vec<_>>().join(" ").contains(&wanted)) {
                let mut fb=FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);
                draw_battle(&battle,&mut resources,&mut fb,&mut effects,Lang::En);
                let file=out.join(if enemy {"enemy-growl-mist.png"} else {"player-growl-mist.png"});
                fb.save_png(&file).unwrap();
                println!("enemy={enemy} frame={frame} message={:?} draws={} file={}",battle.current_message,battle.link_rng.as_ref().unwrap().consumed(),file.display());
                captured=true;break;
            }
            battle.update_frame(if frame%2==0 {a} else {BattleInput::none()});
        }
        assert!(captured);
    }
}
