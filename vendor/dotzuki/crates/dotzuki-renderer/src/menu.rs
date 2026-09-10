use crate::charmap::encode_char;
use crate::text_renderer::{write_tiles_at, ScreenTileBuffer};
use crate::textbox::{TextBoxFrame, TILE_SPACE};

pub const TILE_CURSOR_FILLED: u8 = 0xED;
pub const TILE_CURSOR_UNFILLED: u8 = 0xEC;
pub const TILE_SCROLL_INDICATOR: u8 = 0xEE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuCursor {
    pub x: u32,
    pub y: u32,
    pub tile_behind: u8,
    pub visible: bool,
}

impl MenuCursor {
    pub fn new(x: u32, y: u32) -> Self {
        Self {
            x,
            y,
            tile_behind: TILE_SPACE,
            visible: false,
        }
    }

    pub fn place(&mut self, buf: &mut ScreenTileBuffer) {
        let current = buf.get(self.x, self.y);
        if current != TILE_CURSOR_FILLED {
            self.tile_behind = current;
        }
        buf.set(self.x, self.y, TILE_CURSOR_FILLED);
        self.visible = true;
    }

    pub fn erase(&self, buf: &mut ScreenTileBuffer) {
        buf.set(self.x, self.y, TILE_SPACE);
    }

    pub fn place_unfilled(&self, buf: &mut ScreenTileBuffer) {
        buf.set(self.x, self.y, TILE_CURSOR_UNFILLED);
    }

    pub fn restore(&self, buf: &mut ScreenTileBuffer) {
        buf.set(self.x, self.y, self.tile_behind);
    }

    pub fn move_to(&mut self, buf: &mut ScreenTileBuffer, new_x: u32, new_y: u32) {
        if self.visible {
            self.restore(buf);
        }
        self.x = new_x;
        self.y = new_y;
        self.place(buf);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuLayout {
    pub frame: TextBoxFrame,
    pub top_item_x: u32,
    pub top_item_y: u32,
    pub item_count: u32,
    pub double_spaced: bool,
    pub current_item: u32,
    pub wrapping: bool,
}

impl MenuLayout {
    pub fn new(frame: TextBoxFrame, top_item_x: u32, top_item_y: u32, item_count: u32) -> Self {
        Self {
            frame,
            top_item_x,
            top_item_y,
            item_count,
            double_spaced: true,
            current_item: 0,
            wrapping: false,
        }
    }

    pub fn draw_frame(&self, buf: &mut ScreenTileBuffer) {
        self.frame.draw_frame(buf);
    }

    pub fn cursor_x(&self) -> u32 {
        self.top_item_x.saturating_sub(1)
    }

    pub fn cursor_y_for_item(&self, item_index: u32) -> u32 {
        let spacing = if self.double_spaced { 2 } else { 1 };
        self.top_item_y + item_index * spacing
    }

    pub fn item_position(&self, item_index: u32) -> (u32, u32) {
        (self.top_item_x, self.cursor_y_for_item(item_index))
    }

    pub fn place_cursor(&self, buf: &mut ScreenTileBuffer) -> MenuCursor {
        let cx = self.cursor_x();
        let cy = self.cursor_y_for_item(self.current_item);
        let mut cursor = MenuCursor::new(cx, cy);
        cursor.place(buf);
        cursor
    }

    pub fn move_cursor_up(&mut self) -> bool {
        if self.current_item > 0 {
            self.current_item -= 1;
            true
        } else if self.wrapping && self.item_count > 0 {
            self.current_item = self.item_count - 1;
            true
        } else {
            false
        }
    }

    pub fn move_cursor_down(&mut self) -> bool {
        if self.current_item + 1 < self.item_count {
            self.current_item += 1;
            true
        } else if self.wrapping && self.item_count > 0 {
            self.current_item = 0;
            true
        } else {
            false
        }
    }

    pub fn place_item_text(&self, buf: &mut ScreenTileBuffer, item_index: u32, tiles: &[u8]) {
        let (x, y) = self.item_position(item_index);
        write_tiles_at(buf, x, y, tiles);
    }
}

pub struct StartMenuRenderer;

impl StartMenuRenderer {
    pub fn draw(buf: &mut ScreenTileBuffer, has_dex: bool) -> MenuLayout {
        let height = if has_dex { 16 } else { 14 };
        let frame = TextBoxFrame::new(10, 0, 10, height);
        frame.draw_frame(buf);

        let item_x = 12;
        let item_y = 2;
        let mut y = item_y;

        if has_dex {
            write_tiles_at(buf, item_x, y, &Self::encode_str("DEX"));
            y += 2;
        }
        write_tiles_at(buf, item_x, y, &Self::encode_str("MONSTER"));
        y += 2;
        write_tiles_at(buf, item_x, y, &Self::encode_str("ITEM"));
        y += 2;
        write_tiles_at(buf, item_x, y, &Self::encode_str("SAVE"));
        y += 2;
        write_tiles_at(buf, item_x, y, &Self::encode_str("OPTION"));
        y += 2;
        write_tiles_at(buf, item_x, y, &Self::encode_str("EXIT"));

        let item_count = if has_dex { 7 } else { 6 };
        MenuLayout::new(frame, item_x, item_y, item_count)
    }

    pub fn draw_with_player_name(
        buf: &mut ScreenTileBuffer,
        has_dex: bool,
        player_name: &[u8],
        is_link: bool,
    ) -> MenuLayout {
        let height = if has_dex { 16 } else { 14 };
        let frame = TextBoxFrame::new(10, 0, 10, height);
        frame.draw_frame(buf);

        let item_x = 12;
        let item_y = 2;
        let mut y = item_y;

        if has_dex {
            write_tiles_at(buf, item_x, y, &Self::encode_str("DEX"));
            y += 2;
        }
        write_tiles_at(buf, item_x, y, &Self::encode_str("MONSTER"));
        y += 2;
        write_tiles_at(buf, item_x, y, &Self::encode_str("ITEM"));
        y += 2;
        write_tiles_at(buf, item_x, y, player_name);
        y += 2;
        let save_or_reset = if is_link { "RESET" } else { "SAVE" };
        write_tiles_at(buf, item_x, y, &Self::encode_str(save_or_reset));
        y += 2;
        write_tiles_at(buf, item_x, y, &Self::encode_str("OPTION"));
        y += 2;
        write_tiles_at(buf, item_x, y, &Self::encode_str("EXIT"));

        let item_count = if has_dex { 7 } else { 6 };
        MenuLayout::new(frame, item_x, item_y, item_count)
    }

    fn encode_str(s: &str) -> Vec<u8> {
        s.chars().filter_map(encode_char).collect()
    }
}

pub fn encode_menu_str(s: &str) -> Vec<u8> {
    s.chars().filter_map(encode_char).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextBoxTemplateId {
    MessageBox,
    MenuTemplate03,
    MenuTemplate07,
    ListMenuBox,
    MenuTemplate10,
    MonSpritePopup,
    BattleMenu,
    SafariBattleMenu,
    UseTossMenu,
    SwitchStatsCancelMenu,
    BuySellQuitMenu,
    MoneyBox,
}

impl TextBoxTemplateId {
    pub fn coords(self) -> (u32, u32, u32, u32) {
        match self {
            Self::MessageBox => (0, 12, 19, 17),
            Self::MenuTemplate03 => (0, 0, 19, 14),
            Self::MenuTemplate07 => (0, 0, 11, 6),
            Self::ListMenuBox => (4, 2, 19, 12),
            Self::MenuTemplate10 => (7, 0, 19, 17),
            Self::MonSpritePopup => (6, 4, 14, 13),
            Self::BattleMenu => (8, 12, 19, 17),
            Self::SafariBattleMenu => (0, 12, 19, 17),
            Self::UseTossMenu => (13, 10, 19, 14),
            Self::SwitchStatsCancelMenu => (11, 11, 19, 17),
            Self::BuySellQuitMenu => (0, 0, 10, 6),
            Self::MoneyBox => (11, 0, 19, 2),
        }
    }

    pub fn to_frame(self) -> TextBoxFrame {
        let (x1, y1, x2, y2) = self.coords();
        let width = x2 - x1 + 1;
        let height = y2 - y1 + 1;
        TextBoxFrame::new(x1, y1, width, height)
    }

    pub fn draw(self, buf: &mut ScreenTileBuffer) {
        self.to_frame().draw_frame(buf);
    }

    pub fn draw_with_text(self, buf: &mut ScreenTileBuffer) {
        self.draw(buf);
        match self {
            Self::BattleMenu => {
                let tiles_line1 = encode_menu_str("FIGHT  ");
                let tiles_line2 = encode_menu_str("ITEM  RUN");
                write_tiles_at(buf, 10, 14, &tiles_line1);
                write_tiles_at(buf, 10, 16, &tiles_line2);
            }
            Self::SafariBattleMenu => {
                let tiles_line1 = encode_menu_str("BALL");
                let tiles_line2 = encode_menu_str("THROW ROCK  RUN");
                write_tiles_at(buf, 2, 14, &tiles_line1);
                buf.set(4, 14, 0xF1); // ×
                write_tiles_at(buf, 14, 14, &encode_menu_str("BAIT"));
                write_tiles_at(buf, 2, 16, &tiles_line2);
            }
            Self::UseTossMenu => {
                write_tiles_at(buf, 15, 11, &encode_menu_str("USE"));
                write_tiles_at(buf, 15, 13, &encode_menu_str("TOSS"));
            }
            Self::SwitchStatsCancelMenu => {
                write_tiles_at(buf, 13, 12, &encode_menu_str("SWITCH"));
                write_tiles_at(buf, 13, 14, &encode_menu_str("STATS"));
                write_tiles_at(buf, 13, 16, &encode_menu_str("CANCEL"));
            }
            Self::BuySellQuitMenu => {
                write_tiles_at(buf, 2, 1, &encode_menu_str("BUY"));
                write_tiles_at(buf, 2, 3, &encode_menu_str("SELL"));
                write_tiles_at(buf, 2, 5, &encode_menu_str("QUIT"));
            }
            Self::MoneyBox => {
                write_tiles_at(buf, 13, 0, &encode_menu_str("MONEY"));
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
pub struct TwoOptionMenu {
    pub frame: TextBoxFrame,
    pub option1_tiles: Vec<u8>,
    pub option2_tiles: Vec<u8>,
    pub text_x: u32,
    pub text_y: u32,
    pub current_item: u32,
    saved_tiles: Vec<u8>,
    saved_x: u32,
    saved_y: u32,
    saved_w: u32,
    saved_h: u32,
}

impl TwoOptionMenu {
    pub fn yes_no(x: u32, y: u32) -> Self {
        let frame = TextBoxFrame::new(x, y, 6, 5);
        Self {
            frame,
            option1_tiles: encode_menu_str("YES"),
            option2_tiles: encode_menu_str("NO"),
            text_x: x + 2,
            text_y: y + 1,
            current_item: 0,
            saved_tiles: Vec::new(),
            saved_x: x,
            saved_y: y,
            saved_w: 6,
            saved_h: 5,
        }
    }

    pub fn custom(frame: TextBoxFrame, opt1: &str, opt2: &str) -> Self {
        Self {
            text_x: frame.x + 2,
            text_y: frame.y + 1,
            option1_tiles: encode_menu_str(opt1),
            option2_tiles: encode_menu_str(opt2),
            current_item: 0,
            saved_tiles: Vec::new(),
            saved_x: frame.x,
            saved_y: frame.y,
            saved_w: frame.width,
            saved_h: frame.height,
            frame,
        }
    }

    pub fn save_area(&mut self, buf: &ScreenTileBuffer) {
        self.saved_tiles.clear();
        for row in 0..self.saved_h {
            for col in 0..self.saved_w {
                self.saved_tiles
                    .push(buf.get(self.saved_x + col, self.saved_y + row));
            }
        }
    }

    pub fn restore_area(&self, buf: &mut ScreenTileBuffer) {
        let mut i = 0;
        for row in 0..self.saved_h {
            for col in 0..self.saved_w {
                if i < self.saved_tiles.len() {
                    buf.set(self.saved_x + col, self.saved_y + row, self.saved_tiles[i]);
                    i += 1;
                }
            }
        }
    }

    pub fn draw(&self, buf: &mut ScreenTileBuffer) {
        self.frame.draw_frame(buf);
        write_tiles_at(buf, self.text_x, self.text_y, &self.option1_tiles);
        write_tiles_at(buf, self.text_x, self.text_y + 2, &self.option2_tiles);
    }

    pub fn cursor_position(&self) -> (u32, u32) {
        let cy = self.text_y + self.current_item * 2;
        (self.text_x - 1, cy)
    }
}

#[derive(Debug, Clone)]
pub struct ListMenuRenderer {
    pub frame: TextBoxFrame,
    pub visible_count: u32,
    pub scroll_offset: u32,
    pub total_count: u32,
    pub current_item: u32,
    pub item_x: u32,
    pub item_start_y: u32,
    pub row_spacing: u32,
}

impl ListMenuRenderer {
    pub fn new(total_count: u32) -> Self {
        let frame = TextBoxFrame::new(4, 2, 16, 11);
        Self {
            frame,
            visible_count: 4,
            scroll_offset: 0,
            total_count,
            current_item: 0,
            item_x: 6,
            item_start_y: 4,
            row_spacing: 2,
        }
    }

    pub fn draw_frame(&self, buf: &mut ScreenTileBuffer) {
        self.frame.draw_frame(buf);
    }

    pub fn clear_items(&self, buf: &mut ScreenTileBuffer) {
        for row in 0..self.visible_count {
            let y = self.item_start_y + row * self.row_spacing;
            for col in self.item_x..(self.frame.x + self.frame.width - 1) {
                buf.set(col, y, TILE_SPACE);
                if y + 1 < buf.height_tiles {
                    buf.set(col, y + 1, TILE_SPACE);
                }
            }
        }
    }

    pub fn draw_item(&self, buf: &mut ScreenTileBuffer, visible_index: u32, name_tiles: &[u8]) {
        let y = self.item_start_y + visible_index * self.row_spacing;
        write_tiles_at(buf, self.item_x, y, name_tiles);
    }

    pub fn draw_cancel(&self, buf: &mut ScreenTileBuffer, visible_index: u32) {
        let y = self.item_start_y + visible_index * self.row_spacing;
        write_tiles_at(buf, self.item_x, y, &encode_menu_str("CANCEL"));
    }

    pub fn draw_scroll_indicator(&self, buf: &mut ScreenTileBuffer) {
        let has_more = self.scroll_offset + self.visible_count < self.total_count;
        if has_more {
            let y = self.item_start_y + (self.visible_count - 1) * self.row_spacing + 1;
            let x = self.frame.x + self.frame.width - 2;
            if x < buf.width_tiles && y < buf.height_tiles {
                buf.set(x, y, TILE_SCROLL_INDICATOR);
            }
        }
    }

    pub fn draw_item_quantity(&self, buf: &mut ScreenTileBuffer, visible_index: u32, quantity: u8) {
        let y = self.item_start_y + visible_index * self.row_spacing + 1;
        let x = self.item_x + 8;
        if x < buf.width_tiles && y < buf.height_tiles {
            buf.set(x, y, 0xF1); // ×
            let tens = quantity / 10;
            let ones = quantity % 10;
            if tens > 0 {
                buf.set(x + 1, y, 0xF6 + tens);
            }
            buf.set(x + 2, y, 0xF6 + ones);
        }
    }

    pub fn cursor_position(&self) -> (u32, u32) {
        let relative = self.current_item.saturating_sub(self.scroll_offset);
        let y = self.item_start_y + relative * self.row_spacing;
        (self.item_x - 1, y)
    }

    pub fn scroll_down(&mut self) -> bool {
        if self.scroll_offset + self.visible_count <= self.total_count {
            self.scroll_offset += 1;
            true
        } else {
            false
        }
    }

    pub fn scroll_up(&mut self) -> bool {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
            true
        } else {
            false
        }
    }
}

pub struct PartyMenuRenderer;

impl PartyMenuRenderer {
    pub const NAME_X: u32 = 3;
    pub const FIRST_ROW_Y: u32 = 0;
    pub const ROW_HEIGHT: u32 = 2;
    pub const LEVEL_OFFSET_X: u32 = 10;
    pub const STATUS_OFFSET_X: u32 = 14;
    pub const HP_BAR_OFFSET_X: u32 = 4;
    pub const HP_BAR_OFFSET_Y: u32 = 1;

    pub fn draw_mon_entry(buf: &mut ScreenTileBuffer, index: u32, name_tiles: &[u8]) {
        let y = Self::FIRST_ROW_Y + index * Self::ROW_HEIGHT;
        write_tiles_at(buf, Self::NAME_X, y, name_tiles);
    }

    pub fn draw_level(buf: &mut ScreenTileBuffer, index: u32, level: u8) {
        let y = Self::FIRST_ROW_Y + index * Self::ROW_HEIGHT;
        let x = Self::NAME_X + Self::LEVEL_OFFSET_X;
        buf.set(x, y, 0x6E); // "Lv" tile
        if level >= 100 {
            let h = level / 100;
            let t = (level % 100) / 10;
            let o = level % 10;
            buf.set(x + 1, y, 0xF6 + h);
            buf.set(x + 2, y, 0xF6 + t);
            buf.set(x + 3, y, 0xF6 + o);
        } else if level >= 10 {
            let t = level / 10;
            let o = level % 10;
            buf.set(x + 1, y, 0xF6 + t);
            buf.set(x + 2, y, 0xF6 + o);
        } else {
            buf.set(x + 1, y, 0xF6 + level);
        }
    }

    pub fn draw_status(buf: &mut ScreenTileBuffer, index: u32, status_tiles: &[u8]) {
        let y = Self::FIRST_ROW_Y + index * Self::ROW_HEIGHT;
        let x = Self::NAME_X + Self::STATUS_OFFSET_X;
        write_tiles_at(buf, x, y, status_tiles);
    }

    pub fn hp_bar_position(index: u32) -> (u32, u32) {
        let y = Self::FIRST_ROW_Y + index * Self::ROW_HEIGHT + Self::HP_BAR_OFFSET_Y;
        let x = Self::NAME_X + Self::HP_BAR_OFFSET_X;
        (x, y)
    }

    pub fn draw_hp_bar(
        buf: &mut ScreenTileBuffer,
        index: u32,
        current_hp: u16,
        max_hp: u16,
        bar_width: u32,
    ) {
        let (x, y) = Self::hp_bar_position(index);
        let filled = if max_hp == 0 {
            0
        } else {
            ((current_hp as u32) * bar_width / (max_hp as u32)).min(bar_width)
        };
        buf.set(x, y, 0x71); // HP label tile
        for i in 0..bar_width {
            let tile = if i < filled { 0x72 } else { 0x73 };
            buf.set(x + 1 + i, y, tile);
        }
        buf.set(x + 1 + bar_width, y, 0x6D); // end cap
    }

    pub fn draw_hp_text(
        buf: &mut ScreenTileBuffer,
        index: u32,
        current_hp: u16,
        max_hp: u16,
        bar_width: u32,
    ) {
        let (x, y) = Self::hp_bar_position(index);
        let text_x = x + 2 + bar_width;
        let cur_str = format!("{:>3}", current_hp);
        let max_str = format!("{:>3}", max_hp);
        let combined = format!("{}/ {}", cur_str, max_str);
        let tiles: Vec<u8> = combined.chars().filter_map(encode_char).collect();
        write_tiles_at(buf, text_x, y, &tiles);
    }

    pub fn draw_unfilled_arrow(buf: &mut ScreenTileBuffer, index: u32) {
        let y = Self::FIRST_ROW_Y + index * Self::ROW_HEIGHT;
        buf.set(0, y, TILE_CURSOR_UNFILLED);
    }

    pub fn cursor_position(index: u32) -> (u32, u32) {
        let y = Self::FIRST_ROW_Y + index * Self::ROW_HEIGHT;
        (0, y)
    }
}
