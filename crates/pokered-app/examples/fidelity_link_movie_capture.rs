//! Reproduce the received peer-name movie through the production app hook.
//! Link the identical source against preserved base and integrated app rlibs.
//! Arguments: output directory and before|after label.
use pokered_app::PokemonGame;
use pokered_core::game_state::{GameScreen, Lang};
use pokered_core::link::link_trade::{LinkTradeDriver, LinkTradePollResult};
use pokered_core::link::protocol::NetworkMessage;
use pokered_core::link::transport::ChannelTransport;
use pokered_core::pokemon::{party::Party, stats::create_pokemon};
use pokered_core::trade::TradeAnimPhase;
use pokered_data::{species::Species, wild_data::GameVersion};
use pokered_renderer::{input::InputState, FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = Path::new(&args[1]);
    let label = &args[2];
    assert!(matches!(label.as_str(), "before" | "after"));
    std::fs::create_dir_all(out).unwrap();
    let (mut local_wire, mut remote_wire) = ChannelTransport::<NetworkMessage>::new_pair();
    let pikachu = create_pokemon(Species::Pikachu, 20, [0x99, 0x88]).unwrap();
    let charmander = create_pokemon(Species::Charmander, 20, [0x99, 0x88]).unwrap();
    let mut local = LinkTradeDriver::new(Party::from(vec![pikachu]), 1)
        .with_trainer_name("RED".to_string());
    let mut remote = LinkTradeDriver::new(Party::from(vec![charmander]), 2)
        .with_trainer_name("GREEN".to_string());
    local.request_trade(&mut local_wire).unwrap();
    assert_eq!(remote.poll(&mut remote_wire), LinkTradePollResult::TradeRequested);
    remote.accept_trade(&mut remote_wire).unwrap();
    assert_eq!(local.poll(&mut local_wire), LinkTradePollResult::TradeAccepted);
    assert_eq!(local.remote_name(), "GREEN");
    local.select_mon(&mut local_wire, 0).unwrap();
    remote.poll(&mut remote_wire);
    remote.select_mon(&mut remote_wire, 0).unwrap();
    local.poll(&mut local_wire);
    local.confirm_trade(&mut local_wire).unwrap();
    remote.poll(&mut remote_wire);
    remote.confirm_trade(&mut remote_wire).unwrap();
    remote.poll(&mut remote_wire);
    assert_eq!(local.poll(&mut local_wire), LinkTradePollResult::PeerConfirmed);
    let event = local.poll(&mut local_wire);
    assert!(matches!(event, LinkTradePollResult::TradeExecute { .. }));
    let mut game = PokemonGame::new(GameVersion::Red);
    game.audio = None;
    game.state.screen = GameScreen::Overworld;
    game.state.config.language = Lang::En;
    game.player_name = "RED".to_string();
    game.link_trade = Some(local);
    // Route the production driver's TradeExecute into the real movie hook.
    game.link_cable.on_trade_event(&event);
    game.update(&InputState::new());
    let anim = game.trade_anim.as_mut().expect("frontend started movie");
    assert_eq!((anim.give, anim.receive), (Species::Pikachu, Species::Charmander));
    while anim.phase() != TradeAnimPhase::TextWentTo { anim.tick(); }
    for _ in 0..40 { anim.tick(); }
    let lines = anim.text_lines().unwrap();
    assert_eq!(lines.0, "PIKACHU went");
    assert_eq!(lines.1, if label == "after" { "to GREEN." } else { "to <TRAINER>." });
    println!("received peer GREEN; phase {:?}; phase frame 40; text {:?}", anim.phase(), lines);
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    game.draw(&mut fb);
    fb.save_png(&out.join(format!("link-trade-went-40-{label}.png"))).unwrap();
}
