//! Diploma layout and behind-background Red OBJ from engine/events/diploma.asm.
use pokered_core::game_state::Lang;
use pokered_data::lang_data;
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::resource::{AssetCategory, ResourceManager};
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};
pub fn draw_diploma(
    player_name: &str,
    res: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
    lang: Lang,
) {
    let is_zh = lang == Lang::Zh;
    fb.clear(Rgba::WHITE);
    if let Some(rm) = res.as_mut() {
        if let Ok(cached) = rm.load_asset(AssetCategory::TrainerCard, "trainer_info.png") {
            super::draw_trainer_info_box(fb, &cached.tileset, 0, 0, 18, 16);
        }
        if let Ok(cached) = rm.load_asset(AssetCategory::TrainerCard, "circle_tile.png") {
            for x in [5, 13] {
                super::blit_single_tile(
                    fb,
                    &cached.tileset,
                    0,
                    x * 8,
                    16,
                    &pokered_renderer::palette::GRAYSCALE_PALETTE,
                );
            }
        }
    }
    draw_text(
        lang_data::ui_label("Diploma", is_zh),
        48,
        16,
        Rgba::BLACK,
        fb,
    );
    draw_text(
        if is_zh { "玩家" } else { "Player" },
        24,
        32,
        Rgba::BLACK,
        fb,
    );
    draw_text(player_name, 80, 32, Rgba::BLACK, fb);
    let lines: &[&str] = if is_zh {
        &["恭喜！这份文凭", "证明你已完成了", "宝可梦图鉴！"]
    } else {
        &[
            "Congrats! This",
            "diploma certifies",
            "that you have",
            "completed your",
            "POKéDEX.",
        ]
    };
    for (i, line) in lines.iter().enumerate() {
        draw_text(line, 16, 48 + i as u32 * 16, Rgba::BLACK, fb);
    }
    draw_text("GAME FREAK", 72, 128, Rgba::BLACK, fb);
    if let Some(rm) = res.as_mut() {
        if let Ok(cached) = rm.load_title("player") {
            // OAM priority bit leaves the text/border foreground above Red.
            let palette = [
                Rgba::TRANSPARENT,
                Rgba::WHITE,
                Rgba::rgb(0xAA, 0xAA, 0xAA),
                Rgba::rgb(0x55, 0x55, 0x55),
            ];
            for i in 0..cached.tileset.len() {
                let tile = cached.tileset.get(i);
                let x = 115 + i as u32 % 5 * TILE_SIZE;
                let y = 80 + i as u32 / 5 * TILE_SIZE;
                for row in 0..8 {
                    for col in 0..8 {
                        let index = tile.get(row, col) as usize;
                        if index != 0
                            && fb.get_pixel(x + col as u32, y + row as u32) == Some(Rgba::WHITE)
                        {
                            fb.set_pixel(x + col as u32, y + row as u32, palette[index]);
                        }
                    }
                }
            }
        }
    }
}
