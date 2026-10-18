//! DisplayTextIDInit's VBlank requests, shared by field text and START.
//! CopyScreenTileBufferToVRAM publishes three background thirds. The 128
//! font tiles use eight-tile requests plus the final size-zero DelayFrame.
//! These are resource requests, not a scene-total delay or font renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DisplayTextInit {
    background_thirds: u8,
    font_tiles: u8,
    final_font_wait: bool,
}
impl DisplayTextInit {
    pub const BACKGROUND_TRANSFERS: u8 = 3;
    pub const FONT_TILES: u8 = 128;
    pub const TILES_PER_REQUEST: u8 = 8;
    pub const VBLANKS: u8 =
        Self::BACKGROUND_TRANSFERS + Self::FONT_TILES / Self::TILES_PER_REQUEST + 1;
    pub fn new() -> Self {
        Self {
            background_thirds: Self::BACKGROUND_TRANSFERS,
            font_tiles: Self::FONT_TILES,
            final_font_wait: true,
        }
    }
    pub fn advance_vblank(&mut self) -> bool {
        if self.background_thirds != 0 {
            self.background_thirds -= 1;
        } else if self.font_tiles != 0 {
            self.font_tiles = self.font_tiles.saturating_sub(Self::TILES_PER_REQUEST);
        } else {
            self.final_font_wait = false;
        }
        self.returned()
    }
    pub fn returned(&self) -> bool {
        self.background_thirds == 0 && self.font_tiles == 0 && !self.final_font_wait
    }
    /// WY changes after the background copy; the next LCD publication is
    /// visible during the first font request, not in the preceding raster.
    pub fn border_visible(&self) -> bool {
        self.background_thirds == 0 && self.font_tiles < Self::FONT_TILES
    }
}
impl Default for DisplayTextInit {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requests_publish_window_then_return_after_final_empty_font_request() {
        let mut work = DisplayTextInit::new();
        for tick in 1..=DisplayTextInit::VBLANKS {
            assert_eq!(work.advance_vblank(), tick == 20);
            assert_eq!(work.border_visible(), tick >= 4);
            let restored: DisplayTextInit =
                serde_json::from_value(serde_json::to_value(work).unwrap()).unwrap();
            assert_eq!(restored, work);
        }
    }
}
