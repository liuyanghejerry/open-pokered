use pokered_core::battle::link_battle_driver::{LinkBattleDriver, LinkDriverPhase};
use pokered_core::battle::{BattleInput, BattlePhase};
use pokered_core::link::transport::ChannelTransport;
use pokered_core::pokemon::{party::Party, stats::create_pokemon_with_moves};
use pokered_data::{moves::MoveId, species::Species};

use pokered_app::render::{draw_battle, BattleVisualEffects};
use pokered_core::game_state::Lang;
use pokered_renderer::resource::{AssetRoot, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;
fn main() {
    let output=std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    std::fs::create_dir_all(&output).unwrap();
    for first in [false, true] {
        let make = |species, level, mv| create_pokemon_with_moves(species, level, [0x9a, 0x78],
            [mv, MoveId::None, MoveId::None, MoveId::None]).unwrap();
        let mut player = make(Species::Snorlax, 100, MoveId::Rest);
        player.set_nickname("WARD"); player.hp -= 30;
        let foe = make(Species::Magikarp, if first { 1 } else { 100 }, MoveId::Splash);
        let (ta, tb) = ChannelTransport::new_pair();
        let mut a = LinkBattleDriver::new(Box::new(ta), Party::from(vec![player]), "ALICE".into())
            .with_host_random_list([255; 10]);
        let mut b = LinkBattleDriver::new(Box::new(tb), Party::from(vec![foe]), "BOB".into());
        a.start_handshake().unwrap(); b.poll(); a.poll();
        a.request_battle().unwrap(); b.poll(); b.accept_battle().unwrap(); a.poll(); b.poll();
        assert_eq!(a.phase(), &LinkDriverPhase::Battling);
        assert_eq!(b.phase(), &LinkDriverPhase::Battling);
        let press = BattleInput { a: true, ..BattleInput::none() };
        for driver in [&mut a, &mut b] {
            for frame in 0..1000 {
                if driver.screen().unwrap().phase == BattlePhase::PlayerMenu { break; }
                driver.update(if frame % 2 == 0 { press } else { BattleInput::none() });
            }
            assert_eq!(driver.screen().unwrap().phase, BattlePhase::PlayerMenu);
            assert!(!driver.screen().unwrap().presentation.enabled);
            driver.update(press); driver.update(BattleInput::none()); driver.update(press);
            assert_eq!(driver.screen().unwrap().phase, BattlePhase::LinkWaiting);
        }
        a.poll(); b.poll(); a.poll(); b.poll();
        for (peer, driver) in [("A", &mut a), ("B", &mut b)] {
            let mut fx=BattleVisualEffects::default();
            let mut res=Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
            let label=format!("{}-{peer}",if first {"fast"}else{"slow"});
            let mut inputs=Vec::new();
            for frame in 0..81 {
                let pressed=frame <= 56 && frame % 2 == 0;
                inputs.push(u8::from(pressed));
                driver.update(if pressed {press}else{BattleInput::none()});
                let screen=driver.screen().unwrap();
                fx.update(screen);
                let mut fb=FrameBuffer::new(RenderConfig::new(160,144),Rgba::WHITE);
                draw_battle(screen,&mut res,&mut fb,&mut fx,Lang::En);
                let capture=[0,10,12,22,34,46,56,60,64,80].contains(&frame);
                if capture {fb.save_png(&output.join(format!("{label}-{frame}.png"))).unwrap();}
                println!("{label} frame={frame} capture={capture} message={:?} phase={:?}",screen.current_message,screen.phase);
            }
            println!("INPUTS {label} {inputs:?}");
        }
        assert_eq!(a.screen().unwrap().link_rng.as_ref().unwrap().list(), b.screen().unwrap().link_rng.as_ref().unwrap().list());
        assert_eq!(a.screen().unwrap().link_rng.as_ref().unwrap().consumed(), b.screen().unwrap().link_rng.as_ref().unwrap().consumed());
    }
}
