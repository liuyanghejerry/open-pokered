use crate::text_renderer::ScreenTileBuffer;

pub const TILE_TOP_LEFT: u8 = 0x79;
pub const TILE_TOP_RIGHT: u8 = 0x7B;
pub const TILE_BOTTOM_LEFT: u8 = 0x7D;
pub const TILE_BOTTOM_RIGHT: u8 = 0x7E;
pub const TILE_HORIZONTAL: u8 = 0x7A;
pub const TILE_VERTICAL: u8 = 0x7C;
pub const TILE_SPACE: u8 = 0x7F;
pub const TILE_DOWN_ARROW: u8 = 0xED;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextBoxFrame {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl TextBoxFrame {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn standard_dialog() -> Self {
        Self {
            x: 0,
            y: 12,
            width: 20,
            height: 6,
        }
    }

    pub fn menu_box(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn draw_frame(&self, buf: &mut ScreenTileBuffer) {
        let x = self.x;
        let y = self.y;
        let w = self.width;
        let h = self.height;

        if w < 2 || h < 2 {
            return;
        }

        let inner_w = w - 2;

        buf.set(x, y, TILE_TOP_LEFT);
        for i in 0..inner_w {
            buf.set(x + 1 + i, y, TILE_HORIZONTAL);
        }
        buf.set(x + w - 1, y, TILE_TOP_RIGHT);

        for row in 1..h - 1 {
            buf.set(x, y + row, TILE_VERTICAL);
            for col in 0..inner_w {
                buf.set(x + 1 + col, y + row, TILE_SPACE);
            }
            buf.set(x + w - 1, y + row, TILE_VERTICAL);
        }

        buf.set(x, y + h - 1, TILE_BOTTOM_LEFT);
        for i in 0..inner_w {
            buf.set(x + 1 + i, y + h - 1, TILE_HORIZONTAL);
        }
        buf.set(x + w - 1, y + h - 1, TILE_BOTTOM_RIGHT);
    }

    pub fn clear(&self, buf: &mut ScreenTileBuffer) {
        for row in 0..self.height {
            for col in 0..self.width {
                buf.set(self.x + col, self.y + row, TILE_SPACE);
            }
        }
    }

    pub fn clear_inner(&self, buf: &mut ScreenTileBuffer) {
        if self.width < 2 || self.height < 2 {
            return;
        }
        for row in 1..self.height - 1 {
            for col in 1..self.width - 1 {
                buf.set(self.x + col, self.y + row, TILE_SPACE);
            }
        }
    }

    pub fn show_down_arrow(&self, buf: &mut ScreenTileBuffer) {
        if self.width >= 2 && self.height >= 2 {
            let ax = self.x + self.width - 2;
            let ay = self.y + self.height - 2;
            buf.set(ax, ay, TILE_DOWN_ARROW);
        }
    }

    pub fn hide_down_arrow(&self, buf: &mut ScreenTileBuffer) {
        if self.width >= 2 && self.height >= 2 {
            let ax = self.x + self.width - 2;
            let ay = self.y + self.height - 2;
            buf.set(ax, ay, TILE_SPACE);
        }
    }

    pub fn text_start(&self) -> (u32, u32) {
        (self.x + 1, self.y + 2)
    }

    pub fn second_line_start(&self) -> (u32, u32) {
        (self.x + 1, self.y + 4)
    }
}
