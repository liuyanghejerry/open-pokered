//! Capture the same trade-selection fixture before/after a UI change.
//! GAP_SCREENSHOTS=/absolute/output cargo test -p pokered-app --test visual_verify_link -- --ignored
use pokered_app::link::cable_club::CableClubFlow;
use pokered_core::pokemon::stats::create_pokemon;
use pokered_core::{link::link_trade::LinkTradePollResult, party_screen::PartyScreenInput};
use pokered_data::species::Species;
use pokered_renderer::{FrameBuffer, Rgba};

#[test]
#[ignore = "writes review screenshots to GAP_SCREENSHOTS"]
fn capture_trade_selection() {
    let output =
        std::path::PathBuf::from(std::env::var("GAP_SCREENSHOTS").expect("output directory"));
    std::fs::create_dir_all(&output).unwrap();
    let local = vec![
        create_pokemon(Species::Pikachu, 25, [0xAB, 0xCD]).unwrap(),
        create_pokemon(Species::Charizard, 36, [0xAB, 0xCD]).unwrap(),
    ];
    let peer = vec![
        create_pokemon(Species::Kadabra, 28, [0xAB, 0xCD]).unwrap(),
        create_pokemon(Species::Machoke, 30, [0xAB, 0xCD]).unwrap(),
    ];
    let mut flow = CableClubFlow::new();
    flow.on_trade_event(&LinkTradePollResult::TradeAccepted);
    flow.set_remote_party(&peer);
    flow.set_trainer_names("RED", "BLUE");
    flow.update(PartyScreenInput::none(), &local);
    let config = dotzuki_engine::render_config::RenderConfig::new(160, 144);
    let mut fb = FrameBuffer::new(config, Rgba::WHITE);
    let mut resources = pokered_renderer::resource::ResourceManager::new(
        pokered_renderer::resource::AssetRoot::auto_detect().unwrap(),
    );
    pokered_app::render::draw_link_flow(&flow, &mut fb, false, Some(&mut resources));
    image::RgbaImage::from_fn(160, 144, |x, y| {
        image::Rgba(fb.get_pixel(x, y).unwrap().to_array())
    })
    .save(output.join("trade.png"))
    .unwrap();
    flow.update_with_navigation(PartyScreenInput::none(), &local, true);
    flow.update(
        PartyScreenInput {
            a: true,
            ..PartyScreenInput::none()
        },
        &local,
    );
    let mut resources = pokered_renderer::resource::ResourceManager::new(
        pokered_renderer::resource::AssetRoot::auto_detect().unwrap(),
    );
    pokered_app::render::draw_link_flow(&flow, &mut fb, false, Some(&mut resources));
    image::RgbaImage::from_fn(160, 144, |x, y| {
        image::Rgba(fb.get_pixel(x, y).unwrap().to_array())
    })
    .save(output.join("peer-stats.png"))
    .unwrap();
}
