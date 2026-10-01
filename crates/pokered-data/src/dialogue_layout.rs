//! Pure font metrics and box geometry shared by dialogue logic and drawing.
pub use dotzuki_renderer::embedded_font::{char_advance, measure_text};

pub const LINE_WIDTH_PX: usize = crate::SCREEN_WIDTH_PX as usize - 2 * crate::TILE_SIZE_PX as usize;
// The down arrow occupies the bottom-right corner of the second text row.
pub const SECOND_LINE_WIDTH_PX: usize = LINE_WIDTH_PX - crate::TILE_SIZE_PX as usize;

pub fn contains_chinese(text: &str) -> bool {
    text.chars()
        .any(|c| matches!(c as u32, 0x3400..=0x9fff | 0xf900..=0xfaff | 0x20000..=0x3ffff))
}
