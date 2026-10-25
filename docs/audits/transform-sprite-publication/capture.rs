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
    let output = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    std::fs::create_dir_all(&output).unwrap();
    for enemy in [false, true] { for first in [false, true] {
        let make = |species, mv| create_pokemon_with_moves(species, 40, [0x9a,0x78],
            [mv, MoveId::None, MoveId::None, MoveId::None]).unwrap();
        let mut actor = make(Species::Mewtwo, MoveId::Transform); actor.set_nickname("COPY");
        let mut target = make(Species::Snorlax, MoveId::Splash); target.set_nickname("TANK");
        let bench = make(Species::Tangela, MoveId::Splash);
        let (player, foe) = if enemy { (target, actor) } else { (actor, target) };
        let mut b = BattleScreen::from_parties(true, &[player, bench], &[foe], None);
        b.link_rng = Some(LinkRng::new([255,255,0,255,3,82,73,64,55,46]));
        b.link_enemy_move_override = Some((foe.moves[0],0));
        let state = b.battle_state.as_mut().unwrap();
        let (actor, target) = if enemy { (&mut state.enemy, &mut state.player) } else { (&mut state.player, &mut state.enemy) };
        actor.active_mon_mut().speed = if first {200} else {100};
        target.active_mon_mut().speed = if first {100} else {200};
        b.set_presentation_enabled(true); b.phase = BattlePhase::PlayerMenu;
        let mut fx = BattleVisualEffects::default();
        let mut res = Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
        let mut left_menu = false; let mut turns = 0; let mut result_seen = false;
        let mut switch_step = None; let mut finished = false; let mut inputs = Vec::new();
        for frame in 0..4000 {
            if b.presentation.waiting && fx.is_frame_stable() { b.complete_move_presentation(); }
            let mut input = BattleInput::none();
            if let Some(step) = switch_step {
                match step {
                    0 => input.right = true,
                    2 | 6 | 8 => input.a = true,
                    4 => input.down = true,
                    _ => {}
                }
                switch_step = if step < 8 {Some(step+1)} else {None};
            } else { input.a = frame == 0 || frame == 2 || (frame > 2 && frame % 16 == 0); }
            inputs.push(u8::from(input.a) | u8::from(input.right)<<1 | u8::from(input.down)<<2);
            b.update_frame(input); fx.update(&b);
            let mut captures = Vec::new();
            if frame == 0 { captures.push("initial"); }
            if frame == 3 { captures.push("early"); }
            if !result_seen && b.current_message.as_ref().is_some_and(|m|m.contains("transformed into")) {
                result_seen = true; captures.push("result");
            }
            if b.phase == BattlePhase::PlayerMenu {
                if left_menu {
                    turns += 1; left_menu = false;
                    if turns == 1 {
                        captures.push("first-menu");
                        if enemy && !first { switch_step = Some(0); b.link_enemy_move_override = Some((MoveId::Splash,0)); }
                        else {finished = true;}
                    } else { captures.push("after-switch"); finished = true; }
                }
            } else { left_menu = true; }
            let mut fb = FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);
            draw_battle(&b,&mut res,&mut fb,&mut fx,Lang::En);
            for label in captures {
                fb.save_png(&output.join(format!("enemy-{enemy}-first-{first}-{label}.png"))).unwrap();
                let state = b.battle_state.as_ref().unwrap();
                println!("enemy={enemy} first={first} label={label} frame={frame} turns={turns} display={:?}/{:?} state={:?}/{:?} active={} draws={} message={:?} phase={:?}",
                    b.player_species,b.enemy_species,state.player.active_mon().species,state.enemy.active_mon().species,
                    state.player.active_pokemon_index,b.link_rng.as_ref().unwrap().consumed(),b.current_message,b.phase);
            }
            if finished {break;}
        }
        println!("INPUTS enemy={enemy} first={first} {inputs:?}");
        assert!(finished && result_seen, "enemy={enemy} first={first} phase={:?}",b.phase);
        if enemy && !first { assert_eq!(b.battle_state.as_ref().unwrap().player.active_pokemon_index,1); }
    }}
}
