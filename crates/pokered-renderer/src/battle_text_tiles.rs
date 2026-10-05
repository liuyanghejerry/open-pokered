//! Legacy battle text cells are empty; all text is drawn with Fusion Pixel.
pub const fn is_text_tile(id: usize) -> bool {
    id >= 0x80 || id == 0x6e || id == 0x71 // alphabet/numbers, Lv, HP:
}

/// The bar's left cap shares a tile with original punctuation; keep only the cap.
pub const fn is_text_pixel(id: usize, x: usize) -> bool {
    is_text_tile(id) || (id == 0x62 && x < 4)
}
