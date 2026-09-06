//! Deterministic slots captures; set SLOTS_CAPTURE_DIR to save comparison frames.
use dotzuki_engine::render_config::RenderConfig;
use pokered_app::render::draw_slots;
use pokered_core::{
    game_state::Lang,
    slots_screen::{PayoutStage, SlotsPhase, SlotsScreen},
};
use pokered_renderer::{FrameBuffer, Rgba};

#[test]
fn capture_slots() {
    let Ok(dir) = std::env::var("SLOTS_CAPTURE_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = SlotsScreen::new(false, 100, 42);
    for name in ["bet", "spin", "win", "flash", "result"] {
        match name {
            "spin" => {
                s.phase = SlotsPhase::Spinning;
                s.message = "START!".into();
                s.coins = 99;
                s.machine.wheel_offsets = [7, 10, 13];
            }
            "win" => {
                s.phase = SlotsPhase::Payout;
                s.machine.wheel_offsets = [7, 11, 13];
                s.reels_stopped = [true; 3];
                s.last_payout = 8;
                s.payout_remaining = 8;
                s.payout_stage = PayoutStage::WaitPress;
                s.message = "CHERRY lined up! Scored 8 coins!".into();
            }
            "flash" => {
                s.payout_stage = PayoutStage::Flash;
                s.flash_on = true;
            }
            "result" => {
                s.phase = SlotsPhase::Result;
                s.payout_remaining = 0;
                s.message = "ONE MORE GO?".into();
            }
            _ => {}
        }
        for (lang, suffix) in [(Lang::En, "en"), (Lang::Zh, "zh")] {
            let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            draw_slots(&s, &mut fb, lang);
            let img = image::RgbaImage::from_fn(160, 144, |x, y| {
                image::Rgba(fb.get_pixel(x, y).unwrap().to_array())
            });
            img.save(format!("{dir}/{name}-{suffix}.png")).unwrap();
        }
    }
}
