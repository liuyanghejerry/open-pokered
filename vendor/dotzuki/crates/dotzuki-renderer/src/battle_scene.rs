//! Battle scene rendering — HUDs, HP bars, sprite placement, animations.
//!
//! Faithful to the classic battle-scene layout:
//!   - Player HUD: (9,7) area, HP bar at (10,9)
//!   - Enemy HUD:  (0,0) area, HP bar at (2,2)
//!   - HP bar tiles: $71=HP:, $62=bar_left, $63=empty, $6B=full, $6D=end_cap
//!   - HUD border tiles: $73=connector, $76=horizontal, $77/$74=corners, $6F/$78=triangles
//!   - Ball status: $31=normal, $32=status, $33=fainted, $34=empty

use crate::palette::{Palette, HP_BAR_GREEN_PALETTE, HP_BAR_RED_PALETTE, HP_BAR_YELLOW_PALETTE};
use crate::text_renderer::{write_tiles_at, ScreenTileBuffer};
use crate::textbox::TILE_SPACE;

// ─── HP bar tile constants ───
pub const TILE_HP_LABEL: u8 = 0x71; // "HP:" combined tile
pub const TILE_HP_BAR_LEFT: u8 = 0x62; // left edge of HP bar
pub const TILE_HP_EMPTY: u8 = 0x63; // empty 8-pixel segment
pub const TILE_HP_FULL: u8 = 0x6B; // full 8-pixel segment
pub const TILE_HP_END_CAP_BATTLE: u8 = 0x6D; // right end cap: wHPBarType=1 (player battle/status)
pub const TILE_HP_END_CAP_ENEMY: u8 = 0x6C; // right end cap: wHPBarType=0 (enemy HUD)
pub const TILE_HP_PARTIAL_BASE: u8 = 0x63; // +N for N pixels (1-7)

// ─── HUD border tile constants ───
pub const TILE_HUD_CONNECTOR: u8 = 0x73; // vertical connector above HUD
pub const TILE_HUD_HORIZONTAL: u8 = 0x76; // horizontal bar in HUD border
pub const TILE_HUD_PLAYER_CORNER: u8 = 0x77; // player HUD right corner
pub const TILE_HUD_PLAYER_TRIANGLE: u8 = 0x6F; // player HUD left triangle
pub const TILE_HUD_ENEMY_CORNER: u8 = 0x74; // enemy HUD left corner
pub const TILE_HUD_ENEMY_TRIANGLE: u8 = 0x78; // enemy HUD right triangle

// ─── Ball status tile constants ───
pub const TILE_BALL_NORMAL: u8 = 0x31;
pub const TILE_BALL_STATUS: u8 = 0x32;
pub const TILE_BALL_FAINTED: u8 = 0x33;
pub const TILE_BALL_EMPTY: u8 = 0x34;

// ─── Misc battle tiles ───
pub const TILE_BALL_RETREATED: u8 = 0x4C; // shown after mon retreats

// ─── Battle HP bar width (48 pixels = 6 tiles) ───
pub const BATTLE_HP_BAR_TILES: u32 = 6;
pub const BATTLE_HP_BAR_PIXELS: u32 = BATTLE_HP_BAR_TILES * 8;

/// HP bar color thresholds:
///   ≥27 pixels → Green (healthy)
///   ≥10 pixels → Yellow (caution)
///   <10 pixels → Red (danger)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HpBarColor {
    Green = 0,
    Yellow = 1,
    Red = 2,
}

impl HpBarColor {
    /// Thresholds: 27+ → green, 10+ → yellow, else red.
    pub fn from_pixels(pixels: u32) -> Self {
        if pixels >= 27 {
            Self::Green
        } else if pixels >= 10 {
            Self::Yellow
        } else {
            Self::Red
        }
    }

    pub fn from_hp(current_hp: u16, max_hp: u16) -> Self {
        Self::from_pixels(calc_hp_bar_pixels(current_hp, max_hp))
    }

    pub fn palette(self) -> &'static Palette {
        match self {
            Self::Green => &HP_BAR_GREEN_PALETTE,
            Self::Yellow => &HP_BAR_YELLOW_PALETTE,
            Self::Red => &HP_BAR_RED_PALETTE,
        }
    }
}

/// Status condition identifiers matching the classic status byte layout.
///   SLP = bits 0-2 (mask %111), PSN = bit 3, BRN = bit 4, FRZ = bit 5, PAR = bit 6
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCondition {
    Sleep,
    Poison,
    Burn,
    Freeze,
    Paralysis,
}

impl StatusCondition {
    /// Get the 3-character status text as tile IDs.
    ///   PSN, BRN, FRZ, PAR, SLP — written as charmap-encoded tiles.
    pub fn tiles(self) -> [u8; 3] {
        match self {
            Self::Sleep => [
                0x92, 0x8B, 0x8F, // S, L, P
            ],
            Self::Poison => [
                0x8F, 0x92, 0x8D, // P, S, N
            ],
            Self::Burn => [
                0x81, 0x91, 0x8D, // B, R, N
            ],
            Self::Freeze => [
                0x85, 0x91, 0x99, // F, R, Z
            ],
            Self::Paralysis => [
                0x8F, 0x80, 0x91, // P, A, R
            ],
        }
    }
}

/// Party member status for ball indicator display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BallStatus {
    /// Healthy mon in party
    Normal,
    /// Mon has a status ailment (poisoned, paralyzed, etc.)
    StatusAilment,
    /// Mon has fainted
    Fainted,
    /// Empty party slot
    Empty,
}

impl BallStatus {
    /// Get the tile ID for this ball status.
    pub fn tile(self) -> u8 {
        match self {
            Self::Normal => TILE_BALL_NORMAL,
            Self::StatusAilment => TILE_BALL_STATUS,
            Self::Fainted => TILE_BALL_FAINTED,
            Self::Empty => TILE_BALL_EMPTY,
        }
    }
}

/// Calculate HP bar pixels from current/max HP.
///
/// Returns the number of filled pixels (0..=48).
///   pixels = current_hp * 48 / max_hp  (minimum 1 if HP > 0)
pub fn calc_hp_bar_pixels(current_hp: u16, max_hp: u16) -> u32 {
    if max_hp == 0 || current_hp == 0 {
        return 0;
    }
    let pixels = (current_hp as u32) * BATTLE_HP_BAR_PIXELS / (max_hp as u32);
    pixels.max(1).min(BATTLE_HP_BAR_PIXELS)
}

/// Draw an HP bar into the tile buffer at the given position.
///
/// Uses the classic HP-bar layout:
/// - Tile $71 = "HP:" label
/// - Tile $62 = left edge of bar
/// - Full segments use tile $6B (8 pixels filled)
/// - Partial segments use tiles $64..$6A (1-7 pixels)
/// - Empty segments use tile $63
/// - End cap tile $6D
///
/// `show_hp_numbers`: if true, also writes "current/max" after the bar (player side).
pub fn draw_hp_bar(
    buf: &mut ScreenTileBuffer,
    x: u32,
    y: u32,
    current_hp: u16,
    max_hp: u16,
    end_cap_tile: u8,
    show_hp_numbers: bool,
) -> HpBarColor {
    let pixels = calc_hp_bar_pixels(current_hp, max_hp);
    let color = HpBarColor::from_pixels(pixels);

    // HP: label
    buf.set(x, y, TILE_HP_LABEL);

    // Left edge
    buf.set(x + 1, y, TILE_HP_BAR_LEFT);

    // 6 segments, each 8 pixels wide
    let mut remaining = pixels;
    for i in 0..BATTLE_HP_BAR_TILES {
        let tile = if remaining >= 8 {
            remaining -= 8;
            TILE_HP_FULL
        } else if remaining > 0 {
            let partial = remaining;
            remaining = 0;
            TILE_HP_PARTIAL_BASE + partial as u8 // $64..$6A for 1..7 pixels
        } else {
            TILE_HP_EMPTY
        };
        buf.set(x + 2 + i, y, tile);
    }

    // End cap
    buf.set(x + 2 + BATTLE_HP_BAR_TILES, y, end_cap_tile);

    // Optionally show HP numbers (player HUD only)
    // HP numbers sit at row+1, col+1 relative to the "HP:" label position.
    if show_hp_numbers {
        draw_hp_numbers(buf, x + 1, y + 1, current_hp, max_hp);
    }

    color
}

/// Draw "current/ max" HP number text.
fn draw_hp_numbers(buf: &mut ScreenTileBuffer, x: u32, y: u32, current: u16, max: u16) {
    // Format: "CCC/MMM" right-aligned with spaces
    let cur_str = format!("{:>3}", current.min(999));
    let max_str = format!("{:>3}", max.min(999));
    let combined = format!("{}/{}", cur_str, max_str);

    let tiles: Vec<u8> = combined
        .chars()
        .filter_map(|c| match c {
            '0'..='9' => Some(0xF6 + (c as u8 - b'0')),
            '/' => Some(0xF3), // slash tile
            ' ' => Some(TILE_SPACE),
            _ => None,
        })
        .collect();
    write_tiles_at(buf, x, y, &tiles);
}

/// Player-side battle HUD layout.
///
///   - Clear area: (9,7), 5 rows × 11 cols
///   - Name at (10,7)
///   - Level at (14,8) (or status if has status condition)
///   - HP bar at (10,9)
///   - HUD border tiles: connector $73 at (18,9),
///     then at (18,10): $77(corner), $6F(triangle), $76×8(horizontal bars)
pub struct PlayerHud;

impl PlayerHud {
    // Layout constants
    pub const CLEAR_X: u32 = 9;
    pub const CLEAR_Y: u32 = 7;
    pub const CLEAR_W: u32 = 11;
    pub const CLEAR_H: u32 = 5;

    pub const NAME_X: u32 = 10;
    pub const NAME_Y: u32 = 7;

    pub const LEVEL_X: u32 = 14;
    pub const LEVEL_Y: u32 = 8;

    pub const HP_BAR_X: u32 = 10;
    pub const HP_BAR_Y: u32 = 9;

    /// Clear the player HUD area.
    pub fn clear(buf: &mut ScreenTileBuffer) {
        for row in 0..Self::CLEAR_H {
            for col in 0..Self::CLEAR_W {
                buf.set(Self::CLEAR_X + col, Self::CLEAR_Y + row, TILE_SPACE);
            }
        }
    }

    /// Draw the player HUD border tiles.
    ///
    ///   (18,10) = $73 (connector), then one row down:
    ///   (18,11) = $77(corner), (17..10,11) = 8× $76(horizontal), (9,11) = $6F(triangle)
    pub fn draw_border(buf: &mut ScreenTileBuffer) {
        // $73 connector above the HUD at (18,9)
        buf.set(18, 9, TILE_HUD_CONNECTOR);
        // $73 connector at (18,10)
        buf.set(18, 10, TILE_HUD_CONNECTOR);

        buf.set(18, 11, TILE_HUD_PLAYER_CORNER);
        for i in 0..8 {
            buf.set(17 - i, 11, TILE_HUD_HORIZONTAL);
        }
        buf.set(9, 11, TILE_HUD_PLAYER_TRIANGLE);
    }

    /// Draw the complete player HUD.
    ///
    /// - `name_tiles`: encoded mon name
    /// - `level`: mon level (1-100)
    /// - `status_tiles`: if Some, show status instead of level display
    /// - `current_hp` / `max_hp`: HP values
    pub fn draw(
        buf: &mut ScreenTileBuffer,
        name_tiles: &[u8],
        level: u8,
        status_tiles: Option<&[u8]>,
        current_hp: u16,
        max_hp: u16,
    ) -> HpBarColor {
        Self::clear(buf);
        Self::draw_border(buf);

        let name_offset = center_name_offset(name_tiles.len());
        write_tiles_at(buf, Self::NAME_X + name_offset, Self::NAME_Y, name_tiles);

        // Level at (14,8); status condition at (15,8) when present.
        if let Some(status) = status_tiles {
            write_tiles_at(buf, Self::LEVEL_X + 1, Self::LEVEL_Y, status);
        } else {
            draw_level(buf, Self::LEVEL_X, Self::LEVEL_Y, level);
        }

        draw_hp_bar(
            buf,
            Self::HP_BAR_X,
            Self::HP_BAR_Y,
            current_hp,
            max_hp,
            TILE_HP_END_CAP_BATTLE,
            true,
        )
    }
}

/// Name centering: shift short names right for visual centering.
/// 1-2 chars → +2, 3-4 chars → +1, 5+ chars → +0
fn center_name_offset(name_len: usize) -> u32 {
    match name_len {
        0..=2 => 2,
        3..=4 => 1,
        _ => 0,
    }
}

/// Enemy-side battle HUD layout.
///
///   - Clear area: (0,0), 4 rows × 12 cols
///   - Name at (1,0)
///   - Level at (4,1) (or status if has status condition)
///   - HP bar at (2,2) — NO HP numbers
///   - HUD border tiles at (1,2):
///     $74(corner-left), $78(triangle-right), then 8× $76(horizontal)
pub struct EnemyHud;

impl EnemyHud {
    pub const CLEAR_X: u32 = 0;
    pub const CLEAR_Y: u32 = 0;
    pub const CLEAR_W: u32 = 12;
    pub const CLEAR_H: u32 = 4;

    pub const NAME_X: u32 = 1;
    pub const NAME_Y: u32 = 0;

    pub const LEVEL_X: u32 = 4;
    pub const LEVEL_Y: u32 = 1;

    pub const HP_BAR_X: u32 = 2;
    pub const HP_BAR_Y: u32 = 2;

    /// Clear the enemy HUD area.
    pub fn clear(buf: &mut ScreenTileBuffer) {
        for row in 0..Self::CLEAR_H {
            for col in 0..Self::CLEAR_W {
                buf.set(Self::CLEAR_X + col, Self::CLEAR_Y + row, TILE_SPACE);
            }
        }
    }

    /// Draw the enemy HUD border tiles.
    ///
    ///   (1,2) = $73 (connector), then one row down:
    ///   (1,3) = $74(corner), (2..9,3) = 8× $76(horizontal), (10,3) = $78(triangle)
    pub fn draw_border(buf: &mut ScreenTileBuffer) {
        buf.set(1, 2, TILE_HUD_CONNECTOR);

        buf.set(1, 3, TILE_HUD_ENEMY_CORNER);
        for i in 0..8 {
            buf.set(2 + i, 3, TILE_HUD_HORIZONTAL);
        }
        buf.set(10, 3, TILE_HUD_ENEMY_TRIANGLE);
    }

    /// Draw the complete enemy HUD.
    ///
    /// Note: Enemy HUD does NOT show HP numbers (only the bar).
    pub fn draw(
        buf: &mut ScreenTileBuffer,
        name_tiles: &[u8],
        level: u8,
        status_tiles: Option<&[u8]>,
        current_hp: u16,
        max_hp: u16,
    ) -> HpBarColor {
        Self::clear(buf);
        Self::draw_border(buf);

        // Mon name (centered)
        let name_offset = center_name_offset(name_tiles.len());
        write_tiles_at(buf, Self::NAME_X + name_offset, Self::NAME_Y, name_tiles);

        // Level at (4,1); status condition at (5,1) when present.
        if let Some(status) = status_tiles {
            write_tiles_at(buf, Self::LEVEL_X + 1, Self::LEVEL_Y, status);
        } else {
            draw_level(buf, Self::LEVEL_X, Self::LEVEL_Y, level);
        }

        // HP bar WITHOUT numbers (enemy side)
        draw_hp_bar(
            buf,
            Self::HP_BAR_X,
            Self::HP_BAR_Y,
            current_hp,
            max_hp,
            TILE_HP_END_CAP_ENEMY,
            false,
        )
    }
}

/// Draw a level number at a given tile position.
///
/// Uses tile $6E for the "Lv" prefix, then a 1-3 digit number.
pub fn draw_level(buf: &mut ScreenTileBuffer, x: u32, y: u32, level: u8) {
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

/// Ball indicator sprites for battle.
///
///   Player balls: 6 indicators starting from slot 0.
///     Base position ($60, $60) = (96, 96) in screen coords (after -16/-8 OAM offset).
///     Each ball offset +8 pixels in X.
///   Enemy balls: 6 indicators starting from slot 6.
///     Base position ($48, $20) = (72, 32) in screen coords (after -16/-8 offsets).
///     Each ball offset -8 pixels in X.
///
/// In our rendering, we map ball status to tiles in the tile buffer
/// rather than OAM sprites, since we don't need hardware-level OAM emulation.
pub struct BallIndicators;

impl BallIndicators {
    /// Player ball base derived from OAM ($60,$60) → screen (80,88) → tile (10,11).
    /// Row 11 avoids overlap with HP numbers at row 10.
    /// Left-to-right: cols 10,11,12,13,14,15.
    pub const PLAYER_BASE_X: u32 = 10;
    pub const PLAYER_BASE_Y: u32 = 11;

    /// Enemy ball base derived from OAM ($48,$20) → screen (56,24) → tile (7,3).
    /// Row 4 avoids overlap with enemy HUD border at row 3.
    /// Right-to-left: cols 7,6,5,4,3,2.
    pub const ENEMY_BASE_X: u32 = 7;
    pub const ENEMY_BASE_Y: u32 = 4;

    /// Draw player's 6 ball indicators.
    ///
    /// `party_status`: slice of up to 6 BallStatus values.
    /// Drawn left-to-right starting at (10, 11).
    pub fn draw_player(buf: &mut ScreenTileBuffer, party_status: &[BallStatus]) {
        for i in 0..6 {
            let status = party_status.get(i).copied().unwrap_or(BallStatus::Empty);
            let x = Self::PLAYER_BASE_X + i as u32;
            if x < buf.width_tiles {
                buf.set(x, Self::PLAYER_BASE_Y, status.tile());
            }
        }
    }

    /// Draw enemy's 6 ball indicators.
    ///
    /// `party_status`: slice of up to 6 BallStatus values.
    /// Drawn right-to-left starting at (7, 4).
    pub fn draw_enemy(buf: &mut ScreenTileBuffer, party_status: &[BallStatus]) {
        for i in 0..6 {
            let status = party_status.get(i).copied().unwrap_or(BallStatus::Empty);
            let x = Self::ENEMY_BASE_X.wrapping_sub(i as u32);
            if x < buf.width_tiles {
                buf.set(x, Self::ENEMY_BASE_Y, status.tile());
            }
        }
    }
}

/// Mon retreat animation stages.
///
///   Stage 0: 7×7 tiles at (1,5)
///   Stage 1: 5×5 tiles at (3,7)
///   Stage 2: 3×3 tiles at (4,9)
///   Stage 3: single ball tile $4C at (5,11)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetreatStage {
    Full,   // 7×7 at (1,5)
    Medium, // 5×5 at (3,7)
    Small,  // 3×3 at (4,9)
    Ball,   // single tile at (5,11)
}

impl RetreatStage {
    /// Get the tile position and size for this retreat stage.
    /// Returns (x, y, width, height).
    pub fn layout(self) -> (u32, u32, u32, u32) {
        match self {
            Self::Full => (1, 5, 7, 7),
            Self::Medium => (3, 7, 5, 5),
            Self::Small => (4, 9, 3, 3),
            Self::Ball => (5, 11, 1, 1),
        }
    }

    /// Next stage in the retreat animation.
    pub fn next(self) -> Option<Self> {
        match self {
            Self::Full => Some(Self::Medium),
            Self::Medium => Some(Self::Small),
            Self::Small => Some(Self::Ball),
            Self::Ball => None,
        }
    }
}

/// Clear a rectangular tile area and optionally place a ball.
pub fn draw_retreat_stage(buf: &mut ScreenTileBuffer, stage: RetreatStage) {
    // First clear the full 7×7 area where the mon originally was
    for row in 0..7 {
        for col in 0..7 {
            buf.set(1 + col, 5 + row, TILE_SPACE);
        }
    }

    // For the ball stage, place the ball tile
    if stage == RetreatStage::Ball {
        buf.set(5, 11, TILE_BALL_RETREATED);
    }
    // For other stages, the actual sprite data would be drawn by the sprite system
    // Here we just mark the area (sprite rendering handles the actual image)
}

/// Sprite scaling: doubles each pixel of a source sprite.
///
/// Takes a 4×4 tile (32×32 pixel, but only 28×28 used) front sprite
/// and scales it to 7×7 tiles (56×56 pixels) for the player's back sprite.
///
/// Each source pixel becomes a 2×2 block in the output.
pub fn scale_sprite_2x(src_pixels: &[u8], src_w: usize, src_h: usize) -> Vec<u8> {
    let dst_w = src_w * 2;
    let dst_h = src_h * 2;
    let mut dst = vec![0u8; dst_w * dst_h];

    for sy in 0..src_h {
        for sx in 0..src_w {
            let color = src_pixels[sy * src_w + sx];
            let dx = sx * 2;
            let dy = sy * 2;
            dst[dy * dst_w + dx] = color;
            dst[dy * dst_w + dx + 1] = color;
            dst[(dy + 1) * dst_w + dx] = color;
            dst[(dy + 1) * dst_w + dx + 1] = color;
        }
    }

    dst
}

/// Battle scene sprite positions (in tile coordinates).
///
///   - Enemy front sprite: 7×7 tiles at approximately (12, 0) area
///   - Player back sprite: 7×7 tiles at (1, 5) area (after 2x scaling)
///   - Send-out position: (4, 11) for player mon appearing
pub struct BattleSpritePositions;

impl BattleSpritePositions {
    /// Player back sprite top-left (in tiles). 7×7 tile area.
    pub const PLAYER_SPRITE_X: u32 = 1;
    pub const PLAYER_SPRITE_Y: u32 = 5;
    pub const PLAYER_SPRITE_SIZE: u32 = 7; // 7×7 tiles

    /// Enemy front sprite top-left (in tiles). 7×7 tile area.
    pub const ENEMY_SPRITE_X: u32 = 12;
    pub const ENEMY_SPRITE_Y: u32 = 0;
    pub const ENEMY_SPRITE_SIZE: u32 = 7; // 7×7 tiles

    /// Position where player mon appears when sent out.
    pub const SEND_OUT_X: u32 = 4;
    pub const SEND_OUT_Y: u32 = 11;
}

/// Trainer pic scroll-in parameters.
///
/// The trainer pic is 7 columns wide, scrolled in from the right edge.
/// Each frame moves 1 column, so 7 frames total.
pub struct TrainerPicScroll {
    /// Current column offset (0 = fully visible, 7 = off-screen right)
    pub columns_remaining: u32,
    /// Screen width in tiles (right edge for scroll-in).
    pub screen_tiles_x: u32,
}

impl TrainerPicScroll {
    /// 7 columns wide
    pub const TOTAL_COLUMNS: u32 = 7;
    /// Start row for trainer pic
    pub const START_ROW: u32 = 0;
    /// Number of tile rows for the pic (7 tiles tall)
    pub const PIC_HEIGHT: u32 = 7;

    pub fn new(screen_tiles_x: u32) -> Self {
        Self {
            columns_remaining: Self::TOTAL_COLUMNS,
            screen_tiles_x,
        }
    }

    /// Returns true if the scroll animation is still in progress.
    pub fn is_scrolling(&self) -> bool {
        self.columns_remaining > 0
    }

    /// Advance one frame of the scroll animation.
    /// Returns the number of visible columns after this step.
    pub fn step(&mut self) -> u32 {
        if self.columns_remaining > 0 {
            self.columns_remaining -= 1;
        }
        Self::TOTAL_COLUMNS - self.columns_remaining
    }

    /// Get the X position where the visible portion of the pic starts.
    /// The pic scrolls in from column 19 (right edge).
    pub fn visible_start_x(&self) -> u32 {
        self.screen_tiles_x - (Self::TOTAL_COLUMNS - self.columns_remaining)
    }

    /// Draw partial trainer pic tiles into the buffer.
    ///
    /// `pic_tiles` should be a 7×7 array of tile IDs (row-major).
    /// Only the visible columns (from the left of the pic) are drawn.
    pub fn draw_visible(&self, buf: &mut ScreenTileBuffer, pic_tiles: &[u8]) {
        let visible_cols = Self::TOTAL_COLUMNS - self.columns_remaining;
        if visible_cols == 0 {
            return;
        }

        let start_x = self.visible_start_x();
        for row in 0..Self::PIC_HEIGHT {
            for col in 0..visible_cols {
                let tile_idx = (row * Self::TOTAL_COLUMNS + col) as usize;
                if tile_idx < pic_tiles.len() {
                    let screen_x = start_x + col;
                    let screen_y = Self::START_ROW + row;
                    if screen_x < buf.width_tiles && screen_y < buf.height_tiles {
                        buf.set(screen_x, screen_y, pic_tiles[tile_idx]);
                    }
                }
            }
        }
    }
}

impl Default for TrainerPicScroll {
    fn default() -> Self {
        Self::new(20)
    }
}

/// Full battle scene state for rendering.
///
/// Combines all HUD elements, sprite positions, and animation state.
pub struct BattleScene {
    /// Whether to show HUDs (hidden during certain animations)
    pub show_player_hud: bool,
    pub show_enemy_hud: bool,

    /// Player/enemy party status for ball indicators
    pub player_party: [BallStatus; 6],
    pub enemy_party: [BallStatus; 6],

    /// Current retreat animation stage (None = no retreat in progress)
    pub retreat_stage: Option<RetreatStage>,

    /// Trainer pic scroll state
    pub trainer_scroll: Option<TrainerPicScroll>,
}

impl BattleScene {
    pub fn new() -> Self {
        Self {
            show_player_hud: false,
            show_enemy_hud: false,
            player_party: [BallStatus::Empty; 6],
            enemy_party: [BallStatus::Empty; 6],
            retreat_stage: None,
            trainer_scroll: None,
        }
    }

    /// Draw the full battle scene (HUDs + indicators) into the tile buffer.
    ///
    /// Actual mon sprites are handled by the sprite system, not tile buffer.
    /// This draws: player HUD, enemy HUD, ball indicators, retreat overlay.
    pub fn draw(
        &self,
        buf: &mut ScreenTileBuffer,
        player_name: &[u8],
        player_level: u8,
        player_status: Option<&[u8]>,
        player_hp: u16,
        player_max_hp: u16,
        enemy_name: &[u8],
        enemy_level: u8,
        enemy_status: Option<&[u8]>,
        enemy_hp: u16,
        enemy_max_hp: u16,
    ) {
        if self.show_player_hud {
            PlayerHud::draw(
                buf,
                player_name,
                player_level,
                player_status,
                player_hp,
                player_max_hp,
            );
            BallIndicators::draw_player(buf, &self.player_party);
        }

        if self.show_enemy_hud {
            EnemyHud::draw(
                buf,
                enemy_name,
                enemy_level,
                enemy_status,
                enemy_hp,
                enemy_max_hp,
            );
            BallIndicators::draw_enemy(buf, &self.enemy_party);
        }

        // Draw retreat animation if active
        if let Some(stage) = self.retreat_stage {
            draw_retreat_stage(buf, stage);
        }
    }
}

impl Default for BattleScene {
    fn default() -> Self {
        Self::new()
    }
}
