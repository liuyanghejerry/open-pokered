pub mod provider;

#[cfg(test)]
mod tests;

use pokered_data::charmap;

pub const SCREEN_WIDTH: u8 = 20;
pub const SCREEN_HEIGHT: u8 = 18;
pub const TEXT_BOX_WIDTH: u8 = 18;
pub const TEXT_BOX_LINES: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileCoord {
    pub x: u8,
    pub y: u8,
}

impl TileCoord {
    pub fn new(x: u8, y: u8) -> Self { Self { x, y } }
    pub fn to_tilemap_index(self) -> usize { self.y as usize * SCREEN_WIDTH as usize + self.x as usize }
    pub fn from_tilemap_index(index: usize) -> Self {
        Self { x: (index % SCREEN_WIDTH as usize) as u8, y: (index / SCREEN_WIDTH as usize) as u8 }
    }
}

pub const TILE_TOP_LEFT: u8 = 0x79;
pub const TILE_TOP_RIGHT: u8 = 0x7B;
pub const TILE_BOTTOM_LEFT: u8 = 0x7C;
pub const TILE_BOTTOM_RIGHT: u8 = 0x7E;
pub const TILE_HORIZONTAL: u8 = 0x7A;
pub const TILE_VERTICAL: u8 = 0x7F;
pub const TILE_SPACE: u8 = 0x7F;
pub const TILE_DOWN_ARROW: u8 = 0xED;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextBox {
    pub origin: TileCoord,
    pub width: u8,
    pub height: u8,
}

impl TextBox {
    pub fn new(origin: TileCoord, width: u8, height: u8) -> Self { Self { origin, width, height } }
    pub fn standard_dialog() -> Self { Self { origin: TileCoord::new(0, 12), width: SCREEN_WIDTH, height: 6 } }
    pub fn text_start_coord(&self) -> TileCoord { TileCoord::new(self.origin.x + 1, self.origin.y + 2) }
    pub fn second_line_coord(&self) -> TileCoord { TileCoord::new(self.origin.x + 1, self.origin.y + 4) }
    pub fn arrow_coord(&self) -> TileCoord {
        TileCoord::new(self.origin.x + self.width - 2, self.origin.y + self.height - 2)
    }
}

pub const TILEMAP_SIZE: usize = SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize;

#[derive(Clone)]
pub struct TilemapBuffer {
    pub tiles: [u8; TILEMAP_SIZE],
}

impl Default for TilemapBuffer {
    fn default() -> Self { Self { tiles: [charmap::CHAR_SPACE; TILEMAP_SIZE] } }
}

impl TilemapBuffer {
    pub fn get(&self, coord: TileCoord) -> u8 { self.tiles[coord.to_tilemap_index()] }
    pub fn set(&mut self, coord: TileCoord, tile: u8) { self.tiles[coord.to_tilemap_index()] = tile; }
    pub fn draw_box_border(&mut self, text_box: &TextBox) {
        let x = text_box.origin.x; let y = text_box.origin.y;
        let w = text_box.width; let h = text_box.height; let iw = w - 2;
        self.set(TileCoord::new(x, y), TILE_TOP_LEFT);
        for i in 0..iw { self.set(TileCoord::new(x + 1 + i, y), TILE_HORIZONTAL); }
        self.set(TileCoord::new(x + w - 1, y), TILE_TOP_RIGHT);
        for row in 1..h - 1 {
            self.set(TileCoord::new(x, y + row), TILE_VERTICAL);
            for col in 0..iw { self.set(TileCoord::new(x + 1 + col, y + row), charmap::CHAR_SPACE); }
            self.set(TileCoord::new(x + w - 1, y + row), TILE_VERTICAL);
        }
        self.set(TileCoord::new(x, y + h - 1), TILE_BOTTOM_LEFT);
        for i in 0..iw { self.set(TileCoord::new(x + 1 + i, y + h - 1), TILE_HORIZONTAL); }
        self.set(TileCoord::new(x + w - 1, y + h - 1), TILE_BOTTOM_RIGHT);
    }
    pub fn clear_area(&mut self, origin: TileCoord, width: u8, height: u8) {
        for row in 0..height {
            for col in 0..width {
                self.set(TileCoord::new(origin.x + col, origin.y + row), charmap::CHAR_SPACE);
            }
        }
    }
    pub fn scroll_lines_up(&mut self, src_y: u8, num_rows: u8) {
        for row in 0..num_rows {
            let src = (src_y + row) as usize * SCREEN_WIDTH as usize;
            let dst = (src_y + row - 1) as usize * SCREEN_WIDTH as usize;
            for col in 0..SCREEN_WIDTH as usize { self.tiles[dst + col] = self.tiles[src + col]; }
        }
        let last = (src_y + num_rows - 1) as usize * SCREEN_WIDTH as usize;
        for col in 1..(SCREEN_WIDTH - 1) as usize { self.tiles[last + col] = charmap::CHAR_SPACE; }
    }
    pub fn copy_from_tile_buffer(&mut self, tb: &dotzuki_engine::text::TileBuffer) {
        for (i, entry) in tb.tiles.iter().enumerate() {
            if i < TILEMAP_SIZE { self.tiles[i] = entry.tile_id as u8; }
        }
    }
}

pub const NAME_LENGTH: usize = 11;

pub struct NameBuffers {
    pub player_name: [u8; NAME_LENGTH],
    pub rival_name: [u8; NAME_LENGTH],
}

impl Default for NameBuffers {
    fn default() -> Self {
        let mut player = [charmap::CHAR_TERMINATOR; NAME_LENGTH];
        let mut rival = [charmap::CHAR_TERMINATOR; NAME_LENGTH];
        let p = charmap::encode_string("RED").unwrap_or_default();
        let r = charmap::encode_string("BLUE").unwrap_or_default();
        for (i, &b) in p.iter().enumerate().take(NAME_LENGTH) { player[i] = b; }
        for (i, &b) in r.iter().enumerate().take(NAME_LENGTH) { rival[i] = b; }
        Self { player_name: player, rival_name: rival }
    }
}

use dotzuki_engine::text::{DialogEngine, DialogMode, TileBuffer};
use self::provider::PokemonTextProvider;

pub struct DialogRunner {
    pub engine: DialogEngine<PokemonTextProvider>,
    pub tile_buffer: TileBuffer,
    pub tilemap: TilemapBuffer,
    pub auto_advance: bool,
    has_started: bool,
}

impl DialogRunner {
    pub fn new(provider: PokemonTextProvider) -> Self {
        Self { engine: DialogEngine::new(provider), tile_buffer: TileBuffer::new(20, 18), tilemap: TilemapBuffer::default(), auto_advance: false, has_started: false }
    }
    pub fn begin_text(&mut self, data: Vec<u8>) {
        let text_box = TextBox::standard_dialog();
        self.tilemap.draw_box_border(&text_box);
        self.tile_buffer.clear();
        let start = text_box.text_start_coord();
        self.tile_buffer.cursor = dotzuki_engine::text::TilePos::new(start.x as u16, start.y as u16);
        self.engine.open_dialog(&data);
        self.has_started = true;
    }
    pub fn is_done(&self) -> bool { self.has_started && !self.engine.is_active() }
    pub fn tick(&mut self, button_pressed: bool) {
        if self.engine.is_active() {
            match self.engine.state.mode {
                DialogMode::Typing => { self.engine.update(&mut self.tile_buffer); }
                DialogMode::WaitingForInput | DialogMode::Scrolling | DialogMode::Paused => {
                    if button_pressed || self.auto_advance { self.engine.advance(); }
                }
                DialogMode::Done => { self.engine.advance(); }
            }
            self.tilemap.copy_from_tile_buffer(&self.tile_buffer);
        }
    }
    pub fn run_to_completion(&mut self) {
        self.auto_advance = true;
        while self.engine.is_active() { self.tick(true); }
        self.auto_advance = false;
    }
    pub fn read_tilemap_text(&self, x: u8, y: u8, len: u8) -> String {
        let mut result = String::new();
        for i in 0..len {
            let tile = self.tilemap.get(TileCoord::new(x + i, y));
            if tile == charmap::CHAR_TERMINATOR { break; }
            if let Some(s) = charmap::decode_char(tile) { result.push_str(s); }
        }
        result
    }
}

impl Default for DialogRunner {
    fn default() -> Self { Self::new(PokemonTextProvider::default()) }
}
