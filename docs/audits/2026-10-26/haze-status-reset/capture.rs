use pokered_app::render::{draw_battle, BattleVisualEffects};
use pokered_core::battle::{state::StatusCondition, BattleInput, BattlePhase, BattleScreen};
use pokered_core::game_state::Lang;
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};
use pokered_renderer::resource::{AssetRoot, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;

fn main() {
    let output = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    std::fs::create_dir_all(&output).unwrap();
    for (label, enemy, first, status) in [
        ("player-sleep", false, false, StatusCondition::Sleep(2)),
        ("player-freeze", false, false, StatusCondition::Freeze),
        ("enemy-sleep", true, false, StatusCondition::Sleep(2)),
        ("enemy-freeze", true, false, StatusCondition::Freeze),
        ("slower-player-sleep", false, true, StatusCondition::Sleep(2)),
        ("slower-enemy-freeze", true, true, StatusCondition::Freeze),
    ] {
        let make = |species, mv| create_pokemon_with_moves(species, 100, [0x9a,0x78],
            [mv, MoveId::None, MoveId::None, MoveId::None]).unwrap();
        let mut actor = make(Species::Chansey, MoveId::Splash); actor.set_nickname("WARD");
        actor.status = status;
        let mut target = make(Species::Snorlax, MoveId::Haze); target.set_nickname("TANK");
        let (player, foe) = if enemy { (target, actor) } else { (actor, target) };
        let mut b = BattleScreen::from_parties(true, &[player], &[foe], None);
        b.link_rng = Some(LinkRng::new([255,255,0,255,3,82,73,64,55,46]));
        b.link_enemy_move_override = Some((foe.moves[0],0));
        let state = b.battle_state.as_mut().unwrap();
        let (actor, other) = if enemy { (&mut state.enemy, &mut state.player) } else { (&mut state.player, &mut state.enemy) };
        actor.active_mon_mut().speed = if first { 800 } else { 100 };
        other.active_mon_mut().speed = if first { 100 } else { 200 };
        b.set_presentation_enabled(true); b.phase = BattlePhase::PlayerMenu;
        let mut fx = BattleVisualEffects::default();
        let mut res = Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
        let mut inputs = Vec::new(); let mut previous = None; let mut previous_move = None;
        for frame in 0..481 {
            if b.presentation.waiting && fx.is_frame_stable() { b.complete_move_presentation(); }
            let input = BattleInput { a: [0, 2, 16, 224, 320].contains(&frame), ..BattleInput::none() };
            inputs.push(u8::from(input.a)); b.update_frame(input); fx.update(&b);
            let mut fb = FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);
            draw_battle(&b,&mut res,&mut fb,&mut fx,Lang::En);
            let capture = [0,80,160,224,240,320,480].contains(&frame);
            if capture { fb.save_png(&output.join(format!("{label}-{frame}.png"))).unwrap(); }
            if capture || b.current_message != previous || b.presentation.current_move() != previous_move {
                println!("label={label} frame={frame} capture={capture} hp={} move={:?} message={:?} phase={:?}",
                    b.player_hp,b.presentation.current_move(),b.current_message,b.phase);
                previous=b.current_message.clone(); previous_move=b.presentation.current_move();
            }
        }
        println!("INPUTS {label} {inputs:?}");
    }
}
