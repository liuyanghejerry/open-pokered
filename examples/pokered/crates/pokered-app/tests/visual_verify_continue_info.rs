//! Visual verification test for the main-menu CONTINUE save-info panel
//! (`DisplayContinueGameInfo`, engine/menus/main_menu.asm:357-379).
//!
//! Run with:
//!   cargo test -p pokered-app --test visual_verify_continue_info -- --nocapture

use jrpg_engine::render_config::RenderConfig;
use pokered_app::render::draw_main_menu;
use pokered_core::game_state::{Lang, SaveFileSummary};
use pokered_core::main_menu::{MainMenuState, MenuInput, CONTINUE_INFO_DELAY_FRAMES};
use pokered_renderer::{FrameBuffer, Rgba};

fn save_frame(fb: &FrameBuffer, filename: &str) {
    let img = image::RgbaImage::from_fn(fb.width(), fb.height(), |x, y| {
        let c = fb.get_pixel(x, y).unwrap_or(pokered_renderer::Rgba::WHITE);
        image::Rgba(c.to_array())
    });
    img.save(filename).expect("Failed to save PNG");
    eprintln!("Saved: {filename}");
}

#[test]
fn render_continue_info_panel() {
    let summary = SaveFileSummary {
        player_name: vec![0x91, 0x84, 0x83], // "RED" in pokered charmap
        badges: 0b0011_1111,
        pokedex_owned: 81,
        play_time_hours: 25,
        play_time_minutes: 30,
        play_time_seconds: 0,
    };
    let mut menu = MainMenuState::new(Some(summary));
    while !menu.init_delay_done {
        menu.update_frame(MenuInput::none());
    }
    // Plain menu first.
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_main_menu(&menu, &mut fb, Lang::En);
    save_frame(&fb, "/tmp/main_menu_1_plain.png");

    // A on CONTINUE → the save-info panel.
    menu.update_frame(MenuInput {
        a: true,
        ..MenuInput::none()
    });
    assert!(menu.is_showing_continue_info());
    for _ in 0..CONTINUE_INFO_DELAY_FRAMES {
        menu.update_frame(MenuInput::none());
    }
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_main_menu(&menu, &mut fb, Lang::En);
    save_frame(&fb, "/tmp/main_menu_2_continue_info.png");
}
