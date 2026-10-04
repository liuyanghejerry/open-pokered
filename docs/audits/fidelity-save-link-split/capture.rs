//! Deterministic capture harness; run on master and this branch with the same code.
use pokered_app::render::draw_trade;
use pokered_core::trade::{TradeAnim, TradeAnimPhase};
use pokered_data::species::Species;
use pokered_renderer::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;
use pokered_renderer::resource::{AssetRoot, ResourceManager};

#[test]
#[ignore = "writes review screenshots; requires gfx and CAPTURE_DIR/CAPTURE_SIDE"]
fn capture_trade_text_at_matched_phase_and_frame() {
    let output = std::path::PathBuf::from(std::env::var("CAPTURE_DIR").unwrap());
    let side = std::env::var("CAPTURE_SIDE").unwrap();
    std::fs::create_dir_all(&output).unwrap();
    let gfx = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gfx");
    let mut resources = Some(ResourceManager::new(AssetRoot::new(gfx).unwrap()));
    let mut animation = TradeAnim::new(Species::Cubone, Species::Machoke, "RED".into(), false);
    while animation.phase() != TradeAnimPhase::TextWentTo { animation.tick(); }
    for _ in 0..40 { animation.tick(); }
    let mut frame = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_trade(&animation, &mut resources, &mut frame);
    frame.save_png(&output.join(format!("trade-went-40-{side}.png"))).unwrap();
}
