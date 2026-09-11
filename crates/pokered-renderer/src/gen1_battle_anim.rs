//! Display-frame-accurate driver for Generation I battle animations.
//!
//! `dotzuki_renderer::battle_anim::AnimationPlayer` exposes the decoded
//! animation data and command effects, but intentionally leaves VBlank waits
//! to its caller. Pokémon Red's interpreter also has two timing details which
//! cannot be reconstructed from that command-level API after the fact:
//!
//! - every subanimation uploads 64 or 79 tiles, eight tiles per VBlank;
//! - frame-block modes decide whether several blocks are composed before the
//!   next VBlank and whether `AnimationCleanOAM` contributes another frame.
//!
//! This driver consumes the same public data tables while advancing exactly
//! one visible Game Boy frame for every [`AnimTickResult::Display`] or
//! [`AnimTickResult::Loading`] result. Zero-time commands are surfaced so the
//! frontend can apply their effects and continue in the same update.

use dotzuki_renderer::battle_anim::{
    get_frame_hook, get_move_animation, get_subanimation, AnimCommand, AnimEffect,
    AnimationPlayer as DataAnimationPlayer, FrameBlockMode, FrameHook, MonSide, SpecialEffect,
    SubAnimFrame, SubAnimTransform, AMNESIA, BALL_TILE, CONF_ANIM, FALLING_INITIAL_MOVEMENT,
    FALLING_INITIAL_X, LEAF_TILE, NUM_MOVE_ANIMS, OAM_XFLIP, PETAL_TILE, REST, SLP_ANIM,
    SPIRAL_BALL_COORDS, UPWARD_BALLS_X_ENEMY, UPWARD_BALLS_X_PLAYER,
};
use dotzuki_renderer::palette::{GbColor, Palette};
use dotzuki_renderer::sprite::{SpriteOamEntry, MAX_OAM_ENTRIES};
use dotzuki_renderer::tile::{TileSet, TILE_PIXELS};
use dotzuki_renderer::TILE_SIZE;

const OAM_X_OFFSET: i32 = 8;
const OAM_Y_OFFSET: i32 = 16;
/// Composite move-animation OAM with the DMG's ten-objects-per-scanline
/// selection and X-coordinate priority. `SpriteLayer::render` intentionally
/// draws an unrestricted software layer; battle animations need the hardware
/// limit because effects such as Bubble fill all forty OAM slots.
pub fn render_gen1_oam(
    fb: &mut crate::FrameBuffer,
    entries: &[SpriteOamEntry],
    tileset: &TileSet,
    palette: &Palette,
) {
    render_gen1_oam_palette_split(fb, entries, tileset, palette, palette, None);
}

/// Render OAM while honoring one mid-scanout OBJ-palette write. Ball tosses
/// on the DMG toggle OBP0 after every frame block, so the scanout containing
/// the write can use the old palette above `split_y` and the new one below it.
pub fn render_gen1_oam_palette_split(
    fb: &mut crate::FrameBuffer,
    entries: &[SpriteOamEntry],
    tileset: &TileSet,
    palette_before: &Palette,
    palette_after: &Palette,
    split_y: Option<u32>,
) {
    let width = fb.width() as i32;
    let height = fb.height() as i32;
    for screen_y in 0..height {
        let palette = if split_y.is_some_and(|line| screen_y as u32 >= line) {
            palette_after
        } else {
            palette_before
        };
        let mut selected: Vec<(usize, &SpriteOamEntry)> = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                screen_y >= entry.y
                    && screen_y < entry.y + TILE_PIXELS as i32
                    && entry.x < width
                    && entry.x + TILE_PIXELS as i32 > 0
            })
            .take(10)
            .collect();
        // Smaller X wins on DMG; equal X falls back to lower OAM index. Draw
        // the lowest-priority selected object first so the winner lands last.
        selected.sort_by_key(|(index, entry)| (entry.x, *index));
        for (_, entry) in selected.into_iter().rev() {
            let source_row = (screen_y - entry.y) as usize;
            let tile_row = if entry.y_flip() {
                TILE_PIXELS - 1 - source_row
            } else {
                source_row
            };
            let tile = tileset.get(entry.tile_id as usize);
            for source_col in 0..TILE_PIXELS {
                let screen_x = entry.x + source_col as i32;
                if !(0..width).contains(&screen_x) {
                    continue;
                }
                let tile_col = if entry.x_flip() {
                    TILE_PIXELS - 1 - source_col
                } else {
                    source_col
                };
                let color = tile.get(tile_row, tile_col);
                if color != 0 {
                    // DMG OBJ priority bit: the object is behind nonzero BG
                    // color numbers, but remains visible through BG color 0.
                    if entry.attributes & 0x80 != 0
                        && fb.get_index(screen_x as u32, screen_y as u32) != Some(GbColor::White)
                    {
                        continue;
                    }
                    fb.set_pixel(
                        screen_x as u32,
                        screen_y as u32,
                        palette.colors[color as usize],
                    );
                }
            }
        }
    }
}

const SQUISH_INITIAL: [i8; 7] = [0, 1, 2, 3, 4, 5, 6];
const SQUISH_COLUMNS: [[i8; 7]; 7] = [
    [0, 1, 2, 4, 5, 6, -1],
    [-1, 0, 1, 2, 5, 6, -1],
    [-1, 0, 1, 5, 6, -1, -1],
    [-1, -1, 0, 1, 6, -1, -1],
    [-1, -1, 0, 6, -1, -1, -1],
    [-1, -1, -1, 0, -1, -1, -1],
    [-1; 7],
];

/// Draw the tile-column copies performed by `AnimationSquishMonPic`.
/// `frame` is the zero-based display frame of the blocking special effect.
/// The first transfer is delayed two scanouts; each following transfer has
/// one partial scanout and two complete ones.
pub fn render_gen1_squish(
    fb: &mut crate::FrameBuffer,
    tileset: &TileSet,
    x: i32,
    y: i32,
    palette: &Palette,
    side: MonSide,
    frame: u8,
) {
    fn draw_band(
        fb: &mut crate::FrameBuffer,
        tileset: &TileSet,
        x: i32,
        y: i32,
        palette: &Palette,
        columns: &[i8; 7],
        top: usize,
        bottom: usize,
    ) {
        let width = fb.width() as i32;
        let height = fb.height() as i32;
        for rel_y in top..bottom {
            let screen_y = y + rel_y as i32;
            if !(0..height).contains(&screen_y) {
                continue;
            }
            let tile_row = rel_y / TILE_PIXELS;
            let pixel_row = rel_y % TILE_PIXELS;
            for (dest_col, &source_col) in columns.iter().enumerate() {
                if source_col < 0 {
                    continue;
                }
                let tile = tileset.get(tile_row * 7 + source_col as usize);
                for pixel_col in 0..TILE_PIXELS {
                    let color = tile.get(pixel_row, pixel_col);
                    let screen_x = x + (dest_col * TILE_PIXELS + pixel_col) as i32;
                    if color != 0 && (0..width).contains(&screen_x) {
                        fb.set_pixel(
                            screen_x as u32,
                            screen_y as u32,
                            palette.colors[color as usize],
                        );
                    }
                }
            }
        }
    }

    if frame < 2 {
        draw_band(fb, tileset, x, y, palette, &SQUISH_INITIAL, 0, 56);
        return;
    }
    if frame > 20 {
        return;
    }
    let pass = usize::from((frame - 2) / 3);
    let partial = (frame - 2) % 3 == 0;
    if !partial {
        draw_band(fb, tileset, x, y, palette, &SQUISH_COLUMNS[pass], 0, 56);
        return;
    }

    let previous = if pass == 0 {
        &SQUISH_INITIAL
    } else {
        &SQUISH_COLUMNS[pass - 1]
    };
    draw_band(fb, tileset, x, y, palette, previous, 0, 56);
    let (top, bottom) = match side {
        MonSide::Player => (4, 8),
        MonSide::Enemy => (3, 48),
    };
    for rel_y in top..bottom {
        for rel_x in 0..56 {
            let screen_x = x + rel_x;
            let screen_y = y + rel_y;
            if screen_x >= 0 && screen_y >= 0 {
                fb.set_pixel_index(screen_x as u32, screen_y as u32, GbColor::White);
            }
        }
    }
    draw_band(
        fb,
        tileset,
        x,
        y,
        palette,
        &SQUISH_COLUMNS[pass],
        top as usize,
        bottom as usize,
    );
}

// The original table contains only nine values. Two falling-object seeds use
// index 10 and then keep walking through the machine code and data that follow
// it. Preserve the Red ROM's bytes through the largest index reached by the
// 52-iteration effect instead of smoothing over this original-game bug.
const FALLING_DELTA_BYTES: [u8; 62] = [
    0x00, 0x01, 0x03, 0x05, 0x07, 0x09, 0x0b, 0x0d, 0x0f, 0xfa, 0x8a, 0xd0, 0x3c, 0x47, 0xe6, 0x7f,
    0xfe, 0x09, 0x78, 0x20, 0x04, 0xe6, 0x80, 0xee, 0x80, 0xea, 0x8a, 0xd0, 0xc9, 0x21, 0x01, 0xc3,
    0x11, 0x3e, 0x5d, 0xfa, 0x8b, 0xd0, 0x4f, 0x1a, 0x22, 0x23, 0x23, 0x23, 0x13, 0x0d, 0x20, 0xf7,
    0xc9, 0x38, 0x40, 0x50, 0x60, 0x70, 0x88, 0x90, 0x56, 0x67, 0x4a, 0x77, 0x84, 0x98,
];

/// Result of advancing the animation interpreter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimTickResult {
    /// One VBlank spent uploading the active move-animation tileset.
    Loading {
        sound: Option<u8>,
    },
    /// One visible frame has elapsed. The current OAM must be rendered.
    Display {
        sound: Option<u8>,
    },
    /// A zero-time frame hook. Apply it, then continue in the same update if
    /// the effect itself does not block.
    Hook {
        sound: Option<u8>,
        effect: AnimEffect,
    },
    /// A command-stream special effect. Apply it, then continue in the same
    /// update if the effect itself does not block.
    Effect {
        sound: Option<u8>,
        effect: SpecialEffect,
    },
    Done,
}

/// A per-frame-block callback used by the original ball-animation helpers.
///
/// The normal move interpreter can treat these hooks as no-ops, but the
/// item-use flow needs the exact countdown value to reproduce sound timing,
/// trainer deflection, Ghost Marowak's dodge, and repeated shakes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BallFrameEvent {
    Toss { counter: u8 },
    Shake { counter: u8 },
    Poof { counter: u8 },
}

/// Scanline boundaries for the three palette writes made by
/// `AnimationFlashScreen`. The CPU writes BGP while LCD scanout is active, so
/// the first frame of each new palette is split rather than frame-uniform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortFlashTiming {
    pub entry_scanline: u32,
    pub white_scanline: u32,
    pub restore_scanline: u32,
}

impl Default for ShortFlashTiming {
    fn default() -> Self {
        Self {
            entry_scanline: 9,
            white_scanline: 9,
            restore_scanline: 9,
        }
    }
}

/// Per-call LCD write phases for moves whose frame hooks enter
/// `AnimationFlashScreen` from different points in the OAM interpreter. The
/// move id is the retail Gen-I animation id (1..=165).
pub fn move_short_flash_timing(move_id: u8, player: bool, call: usize) -> Option<ShortFlashTiming> {
    let (entry, white, restore): (&[u8], &[u8], &[u8]) = match (move_id, player) {
        (15, true) => (&[17], &[9], &[9]),
        (15, false) => (&[18], &[9], &[9]),
        (27, true) => (&[17], &[9], &[15]),
        (27, false) => (&[18], &[9], &[16]),
        (5, true) => (&[33, 19, 18], &[9, 9, 8], &[9, 9, 8]),
        (5, false) => (&[18, 18, 19], &[9, 8, 9], &[9, 9, 9]),
        (12, true) => (&[34, 26, 17, 18], &[9, 9, 8, 8], &[9, 9, 8, 8]),
        (12, false) => (&[18, 25, 18, 18], &[9, 8, 8, 8], &[9, 9, 9, 16]),
        (25, true) => (&[34, 19, 17], &[9, 9, 8], &[9, 10, 8]),
        (25, false) => (&[19, 18, 18], &[9, 8, 8], &[9, 10, 9]),
        (29, true) => (&[34], &[9], &[9]),
        (29, false) => (&[19], &[9], &[9]),
        (59, true) => (
            &[11, 11, 12, 20, 11, 11, 18, 19],
            &[9, 10, 20, 9, 9, 9, 9, 9],
            &[9, 9, 9, 9, 9, 9, 9, 8],
        ),
        (59, false) => (
            &[11, 11, 12, 20, 11, 11, 11, 18],
            &[9, 10, 20, 9, 9, 9, 9, 8],
            &[9, 9, 17, 9, 9, 9, 9, 8],
        ),
        (61, true) => (&[12, 12, 12, 28], &[10, 10, 17, 10], &[17, 16, 17, 8]),
        (61, false) => (&[12, 12, 12, 20], &[10, 10, 17, 10], &[17, 24, 10, 8]),
        (85, true) => (
            &[21, 21, 12, 21, 21, 12],
            &[29, 24, 36, 29, 24, 36],
            &[10, 20, 10, 10, 20, 10],
        ),
        (85, false) => (
            &[21, 28, 12, 21, 21, 20],
            &[29, 24, 36, 29, 24, 36],
            &[10, 20, 10, 10, 20, 10],
        ),
        (90, true) => (&[20, 27], &[9, 9], &[9, 9]),
        (90, false) => (&[21, 19], &[9, 9], &[9, 9]),
        (115, true) => (
            &[35, 27, 19, 20, 19, 19, 20, 27, 42, 20, 20, 19],
            &[15, 9, 8, 8, 9, 8, 24, 9, 8, 8, 8, 9],
            &[9, 9, 8, 8, 9, 16, 9, 9, 8, 8, 8, 9],
        ),
        (115, false) => (
            &[20, 26, 20, 19, 19, 19, 29, 26, 20, 20, 19, 19],
            &[16, 8, 9, 8, 9, 8, 16, 8, 9, 8, 8, 9],
            &[9, 9, 9, 16, 8, 8, 9, 8, 9, 9, 8, 8],
        ),
        (116, _) => (&[17], &[8], &[9]),
        (120, true) => (
            &[21, 21, 19, 19, 20],
            &[25, 10, 8, 9, 8],
            &[10, 10, 9, 8, 8],
        ),
        (120, false) => (
            &[21, 21, 19, 20, 21],
            &[25, 10, 8, 9, 8],
            &[10, 10, 9, 8, 8],
        ),
        (141, true) => (&[46, 21], &[8, 16], &[8, 9]),
        (141, false) => (&[22, 21], &[9, 8], &[9, 9]),
        (147, true) => (
            &[35, 21, 20, 20, 19, 19, 19, 19],
            &[9, 16, 8, 8, 9, 8, 17, 8],
            &[9, 9, 8, 8, 9, 16, 9, 8],
        ),
        (147, false) => (
            &[20, 20, 21, 19, 19, 19, 28, 19],
            &[9, 15, 9, 8, 9, 8, 8, 8],
            &[9, 9, 9, 16, 8, 8, 9, 9],
        ),
        (153, true) => (
            &[21, 21, 21, 19, 20],
            &[10, 10, 9, 8, 9],
            &[10, 10, 9, 9, 8],
        ),
        (153, false) => (
            &[21, 21, 21, 19, 20],
            &[10, 10, 9, 8, 9],
            &[10, 10, 9, 9, 8],
        ),
        (157, true) => (&[11, 21, 20], &[8, 9, 9], &[9, 9, 17]),
        (157, false) => (&[11, 21, 20], &[9, 9, 9], &[8, 9, 17]),
        (160, true) => (&[19, 44], &[8, 8], &[8, 8]),
        (160, false) => (&[20, 21], &[8, 9], &[8, 8]),
        (161, true) => (&[20, 30], &[9, 9], &[9, 9]),
        (161, false) => (&[21, 23], &[9, 9], &[9, 9]),
        _ => return None,
    };
    Some(ShortFlashTiming {
        entry_scanline: u32::from(*entry.get(call)?),
        white_scanline: u32::from(*white.get(call)?),
        restore_scanline: u32::from(*restore.get(call)?),
    })
}

#[derive(Debug, Clone, Copy)]
struct ShortFlashState {
    frame: u8,
    entry_bgp: u8,
    restore_bgp: u8,
    timing: ShortFlashTiming,
    chained_restore: Option<(u8, u32)>,
}

/// Display-side state for the four blocking frames and the non-blocking
/// palette-restore edge of `AnimationFlashScreen`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShortScreenFlash {
    state: Option<ShortFlashState>,
}

impl ShortScreenFlash {
    pub fn start(&mut self, restore_bgp: u8, timing: ShortFlashTiming) {
        // Back-to-back flashes begin on the same scanout that restores the
        // first one's white palette. Both writes occur before another VBlank,
        // so the visible boundary belongs to the preceding restore write, not
        // to the nominal entry timing of the second call.
        let (entry_bgp, chained_restore) = match self.state {
            Some(ShortFlashState {
                frame: 4,
                timing: previous,
                restore_bgp: previous_restore_bgp,
                ..
            }) => (
                0x00,
                Some((previous_restore_bgp, previous.restore_scanline)),
            ),
            _ => (restore_bgp, None),
        };
        self.state = Some(ShortFlashState {
            frame: 0,
            entry_bgp,
            restore_bgp,
            timing,
            chained_restore,
        });
    }

    pub fn is_restoring(&self) -> bool {
        matches!(self.state, Some(ShortFlashState { frame: 4, .. }))
    }

    pub fn is_entry(&self) -> bool {
        matches!(self.state, Some(ShortFlashState { frame: 0, .. }))
    }

    pub fn is_active(&self) -> bool {
        self.state.is_some()
    }

    pub fn tick(&mut self) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if state.frame == 4 {
            self.state = None;
        } else {
            state.frame += 1;
        }
    }

    pub fn apply(&self, fb: &mut crate::FrameBuffer) {
        let Some(state) = self.state else {
            return;
        };
        match state.frame {
            0 => apply_bgp_split(fb, state.entry_bgp, 0x1b, state.timing.entry_scanline),
            1 => fb.apply_bgp(0x1b),
            2 => apply_bgp_split(fb, 0x1b, 0x00, state.timing.white_scanline),
            3 => fb.apply_bgp(0x00),
            4 => apply_bgp_split(fb, 0x00, state.restore_bgp, state.timing.restore_scanline),
            _ => unreachable!("short flash has only five visible phases"),
        }
    }

    /// Apply the effective BGP sequence after combining persistent palette
    /// commands with AnimationFlashScreen's writes. This preserves the narrow
    /// middle band when a palette change and flash start in one scanout, and
    /// the two writes between back-to-back flashes.
    pub fn apply_with_palette(&self, palette: &BgPaletteState, fb: &mut crate::FrameBuffer) {
        let Some(state) = self.state else {
            palette.apply(fb);
            return;
        };
        match state.frame {
            0 => {
                if palette.has_writes() {
                    let mut writes = palette.writes.clone();
                    writes.push((state.timing.entry_scanline, 0x1b));
                    apply_bgp_bands(fb, palette.frame_initial_bgp, &writes);
                } else if let Some((restored_bgp, restore_scanline)) = state.chained_restore {
                    apply_bgp_bands(
                        fb,
                        state.entry_bgp,
                        &[
                            (restore_scanline, restored_bgp),
                            (state.timing.entry_scanline, 0x1b),
                        ],
                    );
                } else {
                    apply_bgp_split(fb, state.entry_bgp, 0x1b, state.timing.entry_scanline);
                }
            }
            1 => bake_bgp(fb, 0x1b),
            2 => apply_bgp_split(fb, 0x1b, 0x00, state.timing.white_scanline),
            3 => bake_bgp(fb, 0x00),
            4 if palette.has_writes() => {
                let mut writes = Vec::with_capacity(1 + palette.writes.len());
                writes.push((state.timing.restore_scanline, state.restore_bgp));
                writes.extend_from_slice(&palette.writes);
                apply_bgp_bands(fb, 0x00, &writes);
            }
            4 => apply_bgp_split(fb, 0x00, state.restore_bgp, state.timing.restore_scanline),
            _ => unreachable!("short flash has only five visible phases"),
        }
    }
}

/// Sequential horizontal/vertical window movement used by each of Rock
/// Slide's four frame hooks. The original calls the two predefs one after
/// another; treating the request as a simultaneous XY shake produces a very
/// different nine-frame strobe.
#[derive(Debug, Clone, Copy, Default)]
pub struct RockSlideShake {
    frame: Option<u8>,
    call: u8,
    player_is_attacker: bool,
}

impl RockSlideShake {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn start(&mut self, player_is_attacker: bool) {
        self.frame = Some(0);
        self.player_is_attacker = player_is_attacker;
        self.call = self.call.saturating_add(1);
    }

    pub fn tick(&mut self) {
        let Some(frame) = self.frame.as_mut() else {
            return;
        };
        if *frame >= 14 {
            self.frame = None;
        } else {
            *frame += 1;
        }
    }

    pub fn apply(&self, fb: &mut crate::FrameBuffer) {
        let Some(frame) = self.frame else {
            return;
        };
        let call = usize::from(self.call.saturating_sub(1).min(3));
        let (entry_boundary, exit_boundary) = if self.player_is_attacker {
            ([52, 22, 22, 14][call], [16, 31, 9, 18][call])
        } else {
            ([51, 22, 22, 14][call], [36, 31, 17, 25][call])
        };
        match frame {
            0 => shift_horizontal_band(fb, entry_boundary, fb.height(), 1),
            1..=4 => shift_horizontal_band(fb, 0, fb.height(), 1),
            5 => shift_horizontal_band(fb, 0, exit_boundary, 1),
            // Writing WY=1 during phase 9 happens after the window has
            // already started; the displacement first appears next scanout.
            10..=12 => shift_vertical_band(fb, 1),
            _ => {}
        }
    }
}

const LONG_FLASH_BGP: [u8; 12] = [
    0xf9, 0xfe, 0xff, 0xfe, 0xf9, 0xe4, 0x90, 0x40, 0x00, 0x40, 0x90, 0xe4,
];
const LONG_FLASH_BOUNDARY_FRAMES: [u8; 12] = [0, 10, 12, 22, 24, 29, 30, 35, 36, 41, 42, 47];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LongFlashTiming {
    pub write_scanlines: [u8; 12],
}

impl Default for LongFlashTiming {
    fn default() -> Self {
        Self {
            write_scanlines: [18, 16, 10, 9, 10, 9, 10, 9, 10, 8, 8, 8],
        }
    }
}

/// Raster-accurate display state for `AnimationFlashScreenLong`.
///
/// The first pass holds each of the twelve BGP values for two VBlanks; the
/// next two passes hold each value for one. Every BGP write happens during
/// active LCD scanout, so the first frame of a new value contains the tail of
/// the previous value above the CPU write point.
#[derive(Debug, Clone, Copy)]
pub struct LongScreenFlash {
    frame: Option<u8>,
    initial_bgp: u8,
    timing: LongFlashTiming,
}

impl LongScreenFlash {
    pub fn new() -> Self {
        Self {
            frame: None,
            initial_bgp: 0xe4,
            timing: LongFlashTiming::default(),
        }
    }

    pub fn start(&mut self, initial_bgp: u8, timing: LongFlashTiming) {
        self.frame = Some(0);
        self.initial_bgp = initial_bgp;
        self.timing = timing;
    }

    pub fn is_active(&self) -> bool {
        self.frame.is_some()
    }

    pub fn tick(&mut self) {
        let Some(frame) = self.frame.as_mut() else {
            return;
        };
        if *frame == 47 {
            self.frame = None;
        } else {
            *frame += 1;
        }
    }

    fn step(frame: u8) -> usize {
        if frame < 24 {
            usize::from(frame / 2)
        } else {
            usize::from((frame - 24) % 12)
        }
    }

    fn is_palette_write(frame: u8) -> bool {
        frame >= 24 || frame % 2 == 0
    }

    fn previous_bgp(&self, frame: u8, step: usize) -> u8 {
        if frame == 0 {
            self.initial_bgp
        } else if step == 0 {
            LONG_FLASH_BGP[11]
        } else {
            LONG_FLASH_BGP[step - 1]
        }
    }

    /// First scanline that observes the BGP write. These values are measured
    /// from a pinned retail Red ROM. Only writes that change the binary delta
    /// mask need a special boundary; the other palette-to-palette edges use
    /// the normal command-dispatch phase.
    fn write_scanline(&self, frame: u8) -> u32 {
        LONG_FLASH_BOUNDARY_FRAMES
            .iter()
            .position(|&boundary_frame| boundary_frame == frame)
            .map_or(9, |index| u32::from(self.timing.write_scanlines[index]))
    }

    pub fn apply_with_palette(&self, palette: &BgPaletteState, fb: &mut crate::FrameBuffer) {
        let Some(frame) = self.frame else {
            palette.apply(fb);
            return;
        };
        let step = Self::step(frame);
        let bgp = LONG_FLASH_BGP[step];
        if Self::is_palette_write(frame) {
            let previous = self.previous_bgp(frame, step);
            apply_bgp_split(fb, previous, bgp, self.write_scanline(frame));
        } else {
            bake_bgp(fb, bgp);
        }
    }
}

impl Default for LongScreenFlash {
    fn default() -> Self {
        Self::new()
    }
}

/// Raster-accurate BG-tilemap motion used by Double Team. The player pic is
/// below the copy routine's scanline and changes as a whole after the first
/// edge; the enemy pic crosses scanout, so every direction change exposes a
/// top/bottom split at scanline 48.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShakeBackAndForth {
    side: Option<MonSide>,
    frame: u8,
}

impl ShakeBackAndForth {
    /// Begin the first BG-map copy on the same scanout as the preceding
    /// zero-time palette reset. `start` then adopts the already advanced edge
    /// instead of restarting it one frame late.
    pub fn prime(&mut self, side: MonSide) {
        self.side = Some(side);
        self.frame = 0;
    }

    pub fn start(&mut self, side: MonSide) {
        if self.side == Some(side) && self.frame == 1 {
            return;
        }
        self.side = Some(side);
        self.frame = 0;
    }

    pub fn tick(&mut self) {
        if self.side.is_none() {
            return;
        }
        if self.frame >= 98 {
            self.side = None;
        } else {
            self.frame += 1;
        }
    }

    fn phase_dx(phase: u8) -> i32 {
        if phase % 2 == 0 {
            -8
        } else {
            8
        }
    }

    /// Whole-picture offset below the scanline-48 transfer boundary.
    pub fn dx(&self, side: MonSide) -> i32 {
        if self.side != Some(side) {
            return 0;
        }
        match self.frame {
            2..=96 => Self::phase_dx((self.frame - 2) / 3),
            97 => 8,
            _ => 0,
        }
    }

    /// Additional offset for the portion above scanline 48. The caller draws
    /// the lower band at [`Self::dx`] and redraws the upper band with this
    /// delta, reproducing the BG-map copy that races LCD scanout.
    pub fn top_dx(&self, side: MonSide) -> Option<i32> {
        if self.side != Some(side) {
            return None;
        }
        match self.frame {
            1 => Some(-8),
            97 => Some(-8),
            frame if (4..=94).contains(&frame) && (frame - 1) % 3 == 0 => {
                let old_dx = self.dx(side);
                let new_dx = Self::phase_dx((frame - 1) / 3);
                Some(new_dx - old_dx)
            }
            _ => None,
        }
    }

    pub fn top_split(&self, side: MonSide) -> u32 {
        if self.side == Some(MonSide::Enemy) && side == MonSide::Enemy && self.frame == 0 {
            9
        } else {
            48
        }
    }

    /// Screen-space vertical band in which the preceding normal-position
    /// rightmost tile column is still present while the copied pic is left.
    pub fn stale_normal_right_band(&self, side: MonSide, screen_bottom: u32) -> Option<(u32, u32)> {
        if self.side != Some(side) || self.frame > 4 {
            return None;
        }
        let base_dx = self.dx(side);
        let (top_dx, split) = self.top_dx(side).map_or((base_dx, 0), |delta| {
            (base_dx + delta, self.top_split(side))
        });
        match (top_dx == -8, base_dx == -8) {
            (true, true) => Some((0, screen_bottom)),
            (true, false) => Some((0, split)),
            (false, true) => Some((split, screen_bottom)),
            (false, false) => None,
        }
    }
}

/// Raster-accurate `AnimationBlinkMon` tilemap visibility.
///
/// The original clears and restores the whose-turn 7x7 BG picture six times.
/// Those copies race LCD scanout: depending on the CPU phase, a transfer can
/// affect only the rows above or below scanline 48 in the current frame.  A
/// frontend therefore needs a vertical clip, not a simple visible/hidden bit.
#[derive(Debug, Clone, Copy, Default)]
pub struct BlinkMon {
    side: Option<MonSide>,
    frame: u8,
}

impl BlinkMon {
    pub fn start(&mut self, side: MonSide) {
        self.side = Some(side);
        // The effect command is reached after the first captured VBlank; its
        // first tilemap-copy phase is therefore already one tick old when the
        // first blocking frame is scanned out.
        self.frame = 1;
    }

    pub fn tick(&mut self) {
        if self.side.is_none() {
            return;
        }
        if self.frame >= 77 {
            self.side = None;
        } else {
            self.frame += 1;
        }
    }

    /// Visible screen-space band for `side`. `None` means the picture is
    /// fully blanked for this scanout.
    pub fn visible_band(&self, side: MonSide, screen_bottom: u32) -> Option<(u32, u32)> {
        if self.side != Some(side) {
            return Some((0, screen_bottom));
        }
        match self.frame {
            // Clear/restore reached the LCD before scanline 48.
            3 | 7 | 8 | 15 | 42 | 46 | 47 | 54 => Some((48, screen_bottom)),
            // The entire 7x7 tilemap has been blanked.
            4..=6 | 16..=20 | 30..=32 | 43..=45 | 55..=59 | 69..=71 => None,
            // Clear/restore reached the LCD after scanline 48.
            21 | 28 | 29 | 33 | 60 | 67 | 68 | 72 => Some((0, 48)),
            _ => Some((0, screen_bottom)),
        }
    }
}

/// Background-palette register state for one LCD scanout.
///
/// Move-animation palette commands write DMG's BGP register without waiting
/// for VBlank. A write can therefore divide the current frame into two bands,
/// and two zero-time commands can create a one-scanline band (Agility). The
/// palette is baked into the already-rendered background so OAM composited by
/// the frontend afterward continues to use OBP, just like the hardware.
#[derive(Debug, Clone)]
pub struct BgPaletteState {
    current_bgp: u8,
    frame_initial_bgp: u8,
    writes: Vec<(u32, u8)>,
}

impl BgPaletteState {
    pub fn new() -> Self {
        Self {
            current_bgp: 0xe4,
            frame_initial_bgp: 0xe4,
            writes: Vec::with_capacity(2),
        }
    }

    pub fn reset(&mut self) {
        self.current_bgp = 0xe4;
        self.frame_initial_bgp = 0xe4;
        self.writes.clear();
    }

    /// Begin a new display frame before the command interpreter advances.
    pub fn begin_frame(&mut self) {
        self.frame_initial_bgp = self.current_bgp;
        self.writes.clear();
    }

    pub fn current_bgp(&self) -> u8 {
        self.current_bgp
    }

    pub fn has_writes(&self) -> bool {
        !self.writes.is_empty()
    }

    /// Record a BGP write at the first scanline that observes its new value.
    pub fn write(&mut self, bgp: u8, scanline: u32) {
        self.writes.push((scanline, bgp));
        self.current_bgp = bgp;
    }

    /// Apply the scanout's palette bands to the rendered background.
    pub fn apply(&self, fb: &mut crate::FrameBuffer) {
        apply_bgp_bands(fb, self.frame_initial_bgp, &self.writes);
    }
}

impl Default for BgPaletteState {
    fn default() -> Self {
        Self::new()
    }
}

const FULL_MON_ROWS: &[u8] = &[0, 1, 2, 3, 4, 5, 6];
const SLIDE_UP_ROWS_1: &[u8] = &[1, 2, 3, 4, 5, 6, 0];
const SLIDE_UP_ROWS_2: &[u8] = &[2, 3, 4, 5, 6, 0, 1];
const SLIDE_UP_ROWS_3: &[u8] = &[3, 4, 5, 6, 0, 1, 2];
const SLIDE_UP_ROWS_4: &[u8] = &[4, 5, 6, 0, 1, 2, 3];
const SLIDE_UP_ROWS_5: &[u8] = &[5, 6, 0, 1, 2, 3, 4];
const SLIDE_UP_ROWS_6: &[u8] = &[6, 0, 1, 2, 3, 4, 5];
const ACID_STAGE_1_ROWS: &[u8] = &[0, 1, 3, 4, 5];
const ACID_STAGE_2_ROWS: &[u8] = &[0, 2, 4];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MonPicPlan {
    source_rows: &'static [u8],
    down_rows: u8,
    blanked_vram_tiles: u8,
}

impl MonPicPlan {
    const NORMAL: Self = Self {
        source_rows: FULL_MON_ROWS,
        down_rows: 0,
        blanked_vram_tiles: 0,
    };

    const ACID_STAGE_1: Self = Self {
        source_rows: ACID_STAGE_1_ROWS,
        down_rows: 2,
        blanked_vram_tiles: 0,
    };

    const ACID_STAGE_2: Self = Self {
        source_rows: ACID_STAGE_2_ROWS,
        down_rows: 4,
        blanked_vram_tiles: 0,
    };

    fn splash(down_rows: u8) -> Self {
        Self {
            source_rows: &FULL_MON_ROWS[..7 - down_rows as usize],
            down_rows,
            blanked_vram_tiles: 0,
        }
    }

    fn slide_up(rotation: u8) -> Self {
        let source_rows = match rotation % 7 {
            0 => FULL_MON_ROWS,
            1 => SLIDE_UP_ROWS_1,
            2 => SLIDE_UP_ROWS_2,
            3 => SLIDE_UP_ROWS_3,
            4 => SLIDE_UP_ROWS_4,
            5 => SLIDE_UP_ROWS_5,
            6 => SLIDE_UP_ROWS_6,
            _ => unreachable!(),
        };
        Self {
            source_rows,
            down_rows: 0,
            blanked_vram_tiles: 0,
        }
    }

    fn slide_up_fill(rows: u8) -> Self {
        Self {
            source_rows: &FULL_MON_ROWS[..usize::from(rows)],
            down_rows: 7 - rows,
            blanked_vram_tiles: 0,
        }
    }

    fn acid_blanked(blanked_vram_tiles: u8) -> Self {
        Self {
            blanked_vram_tiles,
            ..Self::ACID_STAGE_2
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MonPicDisplay {
    Full(MonPicPlan),
    /// The top third of wTileMap has transferred; the rest is still old.
    Split {
        top: MonPicPlan,
        bottom: MonPicPlan,
    },
}

#[derive(Debug, Clone, Copy)]
enum MonTilemapState {
    Bounce { side: MonSide, frame: u8 },
    AcidArmor { side: MonSide, frame: u8 },
}

/// Tilemap-accurate mon-picture effects for Splash and Acid Armor.
///
/// The Game Boy copies one third of wTileMap per VBlank. Both routines edit
/// the tilemap between those transfers, so their transition frames contain
/// old rows below scanline 48 and new rows above it. Acid Armor also uses
/// non-contiguous 7x5/7x3 tile-id lists and clears the 49 picture tiles eight
/// per VBlank at the end; a simple sprite offset/crop cannot reproduce it.
#[derive(Debug, Clone, Copy, Default)]
pub struct MonTilemapAnimation {
    state: Option<MonTilemapState>,
}

impl MonTilemapAnimation {
    pub fn start(&mut self, effect: &AnimEffect, side: MonSide) {
        self.state = match effect {
            AnimEffect::BounceUpAndDown => Some(MonTilemapState::Bounce { side, frame: 0 }),
            AnimEffect::SlidePlayerMonDownAndHide => {
                Some(MonTilemapState::AcidArmor { side, frame: 0 })
            }
            _ => self.state,
        };
    }

    pub fn tick(&mut self) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        let (frame, last) = match state {
            MonTilemapState::Bounce { frame, .. } => (frame, 107),
            MonTilemapState::AcidArmor { frame, .. } => (frame, 23),
        };
        if *frame >= last {
            self.state = None;
        } else {
            *frame += 1;
        }
    }

    pub fn controls_side(&self, side: MonSide) -> bool {
        matches!(
            self.state,
            Some(MonTilemapState::Bounce { side: active, .. })
                | Some(MonTilemapState::AcidArmor { side: active, .. })
                if active == side
        )
    }

    pub fn keeps_side_visible(&self, side: MonSide) -> bool {
        matches!(
            self.state,
            Some(MonTilemapState::AcidArmor { side: active, .. }) if active == side
        )
    }

    fn display(&self, side: MonSide) -> Option<MonPicDisplay> {
        match self.state? {
            MonTilemapState::AcidArmor {
                side: active,
                frame,
            } if active == side => Some(match frame {
                0..=1 => MonPicDisplay::Full(MonPicPlan::NORMAL),
                2 => MonPicDisplay::Split {
                    top: MonPicPlan::ACID_STAGE_1,
                    bottom: MonPicPlan::NORMAL,
                },
                3..=8 => MonPicDisplay::Full(MonPicPlan::ACID_STAGE_1),
                9..=10 => MonPicDisplay::Split {
                    top: MonPicPlan::ACID_STAGE_1,
                    bottom: MonPicPlan::ACID_STAGE_2,
                },
                11..=16 => MonPicDisplay::Full(MonPicPlan::ACID_STAGE_2),
                17..=23 => {
                    MonPicDisplay::Full(MonPicPlan::acid_blanked(((frame - 16) * 8).min(49)))
                }
                _ => unreachable!("Acid Armor has only 24 visible frames"),
            }),
            MonTilemapState::Bounce {
                side: active,
                frame,
            } if active == side => {
                if frame < 5 {
                    return Some(MonPicDisplay::Full(MonPicPlan::NORMAL));
                }
                let phase = (frame - 5) % 21;
                Some(match phase {
                    0 | 3 | 6 | 9 | 12 | 15 => {
                        let old = phase / 3;
                        MonPicDisplay::Split {
                            top: MonPicPlan::splash(old + 1),
                            bottom: MonPicPlan::splash(old),
                        }
                    }
                    1 | 2 | 4 | 5 | 7 | 8 | 10 | 11 | 13 | 14 | 16 | 17 => {
                        MonPicDisplay::Full(MonPicPlan::splash((phase + 2) / 3))
                    }
                    18 => MonPicDisplay::Split {
                        top: MonPicPlan::NORMAL,
                        bottom: MonPicPlan::splash(6),
                    },
                    19 | 20 => MonPicDisplay::Full(MonPicPlan::NORMAL),
                    _ => unreachable!("Splash cycle has 21 phases"),
                })
            }
            _ => None,
        }
    }

    /// Draw the controlled mon pic and return true, or return false when the
    /// caller should use its ordinary full-picture path.
    pub fn draw(
        &self,
        fb: &mut crate::FrameBuffer,
        ts: &TileSet,
        x: i32,
        y: i32,
        tiles_per_row: u32,
        pal: &Palette,
        side: MonSide,
    ) -> bool {
        let Some(display) = self.display(side) else {
            return false;
        };
        match display {
            MonPicDisplay::Full(plan) => draw_mon_pic_plan(
                fb,
                ts,
                x,
                y,
                tiles_per_row,
                pal,
                plan,
                false,
                0,
                fb.width() as i32,
                0,
                fb.height(),
            ),
            MonPicDisplay::Split { top, bottom } => {
                draw_mon_pic_plan(
                    fb,
                    ts,
                    x,
                    y,
                    tiles_per_row,
                    pal,
                    top,
                    false,
                    0,
                    fb.width() as i32,
                    0,
                    48,
                );
                draw_mon_pic_plan(
                    fb,
                    ts,
                    x,
                    y,
                    tiles_per_row,
                    pal,
                    bottom,
                    false,
                    0,
                    fb.width() as i32,
                    48,
                    fb.height(),
                );
            }
        }
        true
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_mon_pic_plan(
    fb: &mut crate::FrameBuffer,
    ts: &TileSet,
    x: i32,
    y: i32,
    tiles_per_row: u32,
    pal: &Palette,
    plan: MonPicPlan,
    blank_lower_right: bool,
    clip_left: i32,
    clip_right: i32,
    clip_top: u32,
    clip_bottom: u32,
) {
    for (dest_row, &source_row) in plan.source_rows.iter().enumerate() {
        for source_col in 0..tiles_per_row {
            // _AnimationSlideMonOff compares the next player tile against
            // $61 instead of $62, dropping the lower-right back-pic tile.
            if blank_lower_right && source_row == 6 && source_col == 6 {
                continue;
            }
            // Decompressed Gen-I mon pictures are numbered column-major in
            // VRAM even though TileSet stores the source PNG row-major.
            let vram_tile = source_col * 7 + u32::from(source_row);
            if vram_tile < u32::from(plan.blanked_vram_tiles) {
                continue;
            }
            let tile_index = u32::from(source_row) * tiles_per_row + source_col;
            if tile_index as usize >= ts.len() {
                continue;
            }
            let tile = ts.get(tile_index as usize);
            for row in 0..TILE_PIXELS {
                let py = y
                    + (usize::from(plan.down_rows) + dest_row) as i32 * TILE_SIZE as i32
                    + row as i32;
                if py < clip_top as i32 || py >= clip_bottom as i32 {
                    continue;
                }
                for col in 0..TILE_PIXELS {
                    let color = tile.get(row, col);
                    if color == 0 {
                        continue;
                    }
                    let px = x + (source_col * TILE_SIZE) as i32 + col as i32;
                    if px >= clip_left
                        && px < clip_right
                        && px >= 0
                        && py >= 0
                        && px < fb.width() as i32
                        && py < fb.height() as i32
                    {
                        fb.set_pixel(px as u32, py as u32, pal.color(GbColor::from_u8(color)));
                    }
                }
            }
        }
    }
}

/// Draw a regular mon picture at signed coordinates, clipped to a horizontal
/// scanline band. Battle-animation tilemap writes may move a picture partly
/// beyond the LCD and may become visible one third of the BG map at a time;
/// the ordinary frontend blitter uses unsigned coordinates and therefore
/// cannot represent either case.
pub fn draw_mon_pic_clipped(
    fb: &mut crate::FrameBuffer,
    ts: &TileSet,
    x: i32,
    y: i32,
    tiles_per_row: u32,
    pal: &Palette,
    blank_lower_right: bool,
    clip_left: i32,
    clip_right: i32,
    clip_top: u32,
    clip_bottom: u32,
) {
    draw_mon_pic_plan(
        fb,
        ts,
        x,
        y,
        tiles_per_row,
        pal,
        MonPicPlan::NORMAL,
        blank_lower_right,
        clip_left,
        clip_right,
        clip_top,
        clip_bottom,
    );
}

/// Render `_AnimationSlideMonUp` with the original tilemap/VBlank tearing.
///
/// The routine rotates all seven picture rows every two frames while the
/// battle tilemap reaches VRAM one third at a time. Since the enemy picture
/// straddles the first transfer boundary and the player picture starts just
/// above it, both sides show the same pair of row rotations split at LCD
/// scanline 48.
pub fn render_gen1_slide_up(
    fb: &mut crate::FrameBuffer,
    ts: &TileSet,
    x: i32,
    y: i32,
    tiles_per_row: u32,
    pal: &Palette,
    frame: u8,
    starts_hidden: bool,
) {
    let display = match (starts_hidden, frame) {
        (true, 0..=2) => MonPicDisplay::Full(MonPicPlan::slide_up_fill(0)),
        (true, 3..=4) => MonPicDisplay::Full(MonPicPlan::slide_up_fill(1)),
        (true, 5) => MonPicDisplay::Split {
            top: MonPicPlan::slide_up_fill(2),
            bottom: MonPicPlan::slide_up_fill(1),
        },
        (true, 6..=7) => MonPicDisplay::Split {
            top: MonPicPlan::slide_up_fill(2),
            bottom: MonPicPlan::slide_up_fill(3),
        },
        (true, 8) => MonPicDisplay::Split {
            top: MonPicPlan::slide_up_fill(4),
            bottom: MonPicPlan::slide_up_fill(3),
        },
        (true, 9..=10) => MonPicDisplay::Full(MonPicPlan::slide_up_fill(4)),
        (true, 11) => MonPicDisplay::Split {
            top: MonPicPlan::slide_up_fill(5),
            bottom: MonPicPlan::slide_up_fill(4),
        },
        (true, 12..=13) => MonPicDisplay::Split {
            top: MonPicPlan::slide_up_fill(5),
            bottom: MonPicPlan::slide_up_fill(6),
        },
        (true, 14) => MonPicDisplay::Split {
            top: MonPicPlan::NORMAL,
            bottom: MonPicPlan::slide_up_fill(6),
        },
        (true, _) => MonPicDisplay::Full(MonPicPlan::NORMAL),
        (false, 0..=2) => MonPicDisplay::Full(MonPicPlan::NORMAL),
        (false, 3) => MonPicDisplay::Split {
            top: MonPicPlan::slide_up(1),
            bottom: MonPicPlan::NORMAL,
        },
        (false, 4..=5) => MonPicDisplay::Split {
            top: MonPicPlan::slide_up(1),
            bottom: MonPicPlan::slide_up(2),
        },
        (false, 6) => MonPicDisplay::Split {
            top: MonPicPlan::slide_up(3),
            bottom: MonPicPlan::slide_up(2),
        },
        (false, 7..=8) => MonPicDisplay::Full(MonPicPlan::slide_up(3)),
        (false, 9) => MonPicDisplay::Split {
            top: MonPicPlan::slide_up(4),
            bottom: MonPicPlan::slide_up(3),
        },
        (false, 10..=11) => MonPicDisplay::Split {
            top: MonPicPlan::slide_up(4),
            bottom: MonPicPlan::slide_up(5),
        },
        (false, 12) => MonPicDisplay::Split {
            top: MonPicPlan::slide_up(6),
            bottom: MonPicPlan::slide_up(5),
        },
        (false, 13..=14) => MonPicDisplay::Full(MonPicPlan::slide_up(6)),
        (false, _) => MonPicDisplay::Split {
            top: MonPicPlan::NORMAL,
            bottom: MonPicPlan::slide_up(6),
        },
    };

    match display {
        MonPicDisplay::Full(plan) => draw_mon_pic_plan(
            fb,
            ts,
            x,
            y,
            tiles_per_row,
            pal,
            plan,
            false,
            0,
            fb.width() as i32,
            0,
            fb.height(),
        ),
        MonPicDisplay::Split { top, bottom } => {
            draw_mon_pic_plan(
                fb,
                ts,
                x,
                y,
                tiles_per_row,
                pal,
                top,
                false,
                0,
                fb.width() as i32,
                0,
                48,
            );
            draw_mon_pic_plan(
                fb,
                ts,
                x,
                y,
                tiles_per_row,
                pal,
                bottom,
                false,
                0,
                fb.width() as i32,
                48,
                fb.height(),
            );
        }
    }
}

impl AnimationPlayer {
    /// Whether this scanout is the final window-position restoration of a
    /// blocking horizontal screen shake.
    pub fn is_shake_restore_frame(&self) -> bool {
        matches!(self.display_shake, Some((_, 72)))
    }
}

#[derive(Debug, Clone)]
struct SubAnimState {
    frames: Vec<SubAnimFrame>,
    transform: SubAnimTransform,
    frame_index: usize,
    delay: u8,
    sound_pending: bool,
    sound_id: u8,
    dest_slot: usize,
}

#[derive(Debug, Clone)]
struct PendingSubAnim {
    subanim_id: u8,
    delay: u8,
    sound_id: u8,
}

#[derive(Debug, Clone)]
enum PostFrameAction {
    None,
    Hook(AnimEffect),
    Growl { final_frame: bool },
}

#[derive(Debug, Clone)]
struct FrameWait {
    /// DelayFrames VBlanks still to show after the frame returned when this
    /// wait was created. The draw-return frame is the first DelayFrames frame.
    remaining: u8,
    /// Mode00/01 calls AnimationCleanOAM after DelayFrames. That helper waits
    /// one more VBlank before clearing OAM.
    cleanup_frame_pending: bool,
    clear_before_resume: bool,
    post: PostFrameAction,
}

#[derive(Debug, Clone, Copy)]
struct FallingObject {
    y: u8,
    x: u8,
    movement: u8,
}

#[derive(Debug, Clone)]
enum InternalEffect {
    /// `AnimationSpiralBallsInward`: load tiles, display nineteen three-ball
    /// positions for five frames each, clean OAM, then run the short flash.
    SpiralBalls {
        sound_pending: Option<u8>,
        load_remaining: u8,
        step: u8,
        hold_remaining: u8,
        cleanup_shown: bool,
    },
    /// `AnimationFallingObjects`: the update precedes each three-frame hold.
    /// Petals clear OAM on return; leaves deliberately leave it populated.
    FallingObjects {
        sound_pending: Option<u8>,
        load_remaining: u8,
        tile: u8,
        objects: Vec<FallingObject>,
        steps_remaining: u8,
        hold_remaining: u8,
        clear_after: bool,
    },
    /// One or six upward ball pillars. Every pillar reloads tileset zero, as
    /// `_AnimationShootBallsUpward` does in the original routine.
    ShootBalls {
        sound_pending: Option<u8>,
        load_remaining: u8,
        base_y: u8,
        pillar_xs: Vec<u8>,
        pillar_index: usize,
        ball_count: u8,
        balls: Vec<u8>,
        initialized: bool,
    },
    /// `AnimationWaterDropletsEverywhere`: ten tile-upload VBlanks followed
    /// by 64 calls which each expose OAM for one frame and clear it for one.
    WaterDroplets {
        sound_pending: Option<u8>,
        load_remaining: u8,
        calls_remaining: u8,
        showing: bool,
        half: u8,
        base_x: u8,
    },
    /// `AnimationWavyScreen`: seven setup frames, 127 scanline-distortion
    /// frames, then nine restore/copy frames. The capture's following `Done`
    /// frame is the tenth blank frame visible in the reference trace.
    WavyScreen {
        sound_pending: Option<u8>,
        frame: u8,
    },
    /// `PredefShakeScreenHorizontally` with b=8: each amplitude is held five
    /// frames, followed by four frames at rest, then decremented.
    ShakeScreen {
        sound_pending: Option<u8>,
        frame: u8,
        call: u8,
    },
}

/// Pokémon Red's move-animation command interpreter in display-frame time.
#[derive(Debug, Clone)]
pub struct AnimationPlayer {
    animation_id: usize,
    player_is_attacker: bool,
    commands: Vec<AnimCommand>,
    command_index: usize,
    subanim: Option<SubAnimState>,
    pending_subanim: Option<PendingSubAnim>,
    tile_load_remaining: u8,
    current_tileset: Option<u8>,
    frame_wait: Option<FrameWait>,
    internal_effect: Option<InternalEffect>,
    display_wave_phase: Option<u8>,
    display_shake: Option<(u8, u8)>,
    shake_call_count: u8,
    shake_restore_pending: Option<u8>,
    oam_slots: Vec<Option<SpriteOamEntry>>,
    oam_buffer: Vec<SpriteOamEntry>,
    ball_frame_event: Option<BallFrameEvent>,
    finished: bool,
}

impl AnimationPlayer {
    pub fn new() -> Self {
        Self {
            animation_id: 0,
            player_is_attacker: true,
            commands: Vec::new(),
            command_index: 0,
            subanim: None,
            pending_subanim: None,
            tile_load_remaining: 0,
            current_tileset: None,
            frame_wait: None,
            internal_effect: None,
            display_wave_phase: None,
            display_shake: None,
            shake_call_count: 0,
            shake_restore_pending: None,
            oam_slots: vec![None; MAX_OAM_ENTRIES],
            oam_buffer: Vec::with_capacity(MAX_OAM_ENTRIES),
            ball_frame_event: None,
            finished: true,
        }
    }

    /// Start an animation. `move_id` is the zero-based index used by the
    /// shared animation tables.
    pub fn start(&mut self, move_id: usize, player_is_attacker: bool) {
        self.start_with_oam_policy(move_id, player_is_attacker, false);
    }

    /// Start an animation without clearing the OAM left by the preceding
    /// animation. `TossBallAnimation` relies on this when a mode-04 shake is
    /// followed by a poof: the final closed-ball frame remains visible while
    /// the next tileset is uploaded.
    pub fn start_preserving_oam(&mut self, move_id: usize, player_is_attacker: bool) {
        self.start_with_oam_policy(move_id, player_is_attacker, true);
    }

    fn start_with_oam_policy(
        &mut self,
        move_id: usize,
        player_is_attacker: bool,
        preserve_oam: bool,
    ) {
        let resolved = if !player_is_attacker {
            match move_id + 1 {
                id if id == AMNESIA as usize => CONF_ANIM as usize - 1,
                id if id == REST as usize => SLP_ANIM as usize - 1,
                _ => move_id,
            }
        } else {
            move_id
        };

        self.animation_id = resolved;
        self.player_is_attacker = player_is_attacker;
        self.commands = if resolved < NUM_MOVE_ANIMS {
            get_move_animation(resolved).commands
        } else {
            Vec::new()
        };
        self.command_index = 0;
        self.subanim = None;
        self.pending_subanim = None;
        self.tile_load_remaining = 0;
        self.current_tileset = None;
        self.frame_wait = None;
        self.internal_effect = None;
        self.display_wave_phase = None;
        self.display_shake = None;
        self.shake_call_count = 0;
        self.shake_restore_pending = None;
        self.ball_frame_event = None;
        if !preserve_oam {
            self.clear_oam();
        }
        self.finished = self.commands.is_empty();
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// One-based animation id currently loaded from MOVE_ANIM_DATA.
    pub fn animation_id(&self) -> u8 {
        self.animation_id as u8 + 1
    }

    /// Current screen-space OAM. The +8/+16 hardware coordinate bias has
    /// already been removed.
    pub fn oam_entries(&self) -> &[SpriteOamEntry] {
        &self.oam_buffer
    }

    pub fn current_tileset(&self) -> Option<u8> {
        self.current_tileset
    }

    /// Ball-shake frame blocks use mode 04, and the original leaves the last
    /// closed-ball OAM visible while the following animation loads tiles.
    pub fn preserves_oam_when_finished(&self) -> bool {
        get_frame_hook(self.animation_id as u8 + 1) == Some(FrameHook::BallShake)
    }

    /// Take the ball-specific hook emitted by the most recently drawn frame
    /// block, if any.
    pub fn take_ball_frame_event(&mut self) -> Option<BallFrameEvent> {
        self.ball_frame_event.take()
    }

    /// Skip the next frame block of the active subanimation. The trainer
    /// battle branch applies this at toss counter 3, exactly matching the
    /// original helper's extra decrement of `wSubAnimCounter`.
    pub fn skip_next_subanimation_frame(&mut self) -> bool {
        let Some(state) = self.subanim.as_mut() else {
            return false;
        };
        if state.frame_index >= state.frames.len() {
            return false;
        }
        state.frame_index += 1;
        true
    }

    /// Rewind the active subanimation without reloading tiles or clearing
    /// OAM. Ball shakes use this at counter 1 for every remaining wobble.
    pub fn repeat_current_subanimation(&mut self) -> bool {
        let Some(state) = self.subanim.as_mut() else {
            return false;
        };
        state.frame_index = 0;
        state.dest_slot = 0;
        true
    }

    /// Apply the scanline/window effects whose timing is owned by this
    /// interpreter. These are kept here because their command routines block
    /// for a different number of frames than the generic renderer effects.
    pub fn apply_screen_effects(&self, fb: &mut crate::FrameBuffer) {
        if let Some((call, phase)) = self.display_shake {
            // The battle scene lives in the Game Boy window from WY=0, so a
            // WX mutation moves the text box as well as the combat area. OAM
            // is composited afterward by the frontends and does not move.
            apply_shake_screen(
                fb,
                self.animation_id as u8 + 1,
                self.player_is_attacker,
                call,
                phase,
            );
        }
        if let Some(phase) = self.display_wave_phase {
            apply_wavy_screen(
                fb,
                self.animation_id as u8 + 1,
                self.player_is_attacker,
                phase,
            );
        }
    }

    /// Map an animation-table special effect onto the shared high-level
    /// renderer action.
    pub fn apply_effect(effect: SpecialEffect) -> AnimEffect {
        DataAnimationPlayer::apply_effect(effect)
    }

    /// Exact command-blocking duration for effects whose assembly routines
    /// include VRAM transfers or nested waits omitted by the generic effect
    /// renderer. `None` means the generic duration is already authoritative.
    pub fn effect_duration(effect: &AnimEffect, attacker: MonSide) -> Option<u8> {
        match *effect {
            AnimEffect::ShowPlayerMon | AnimEffect::ShowEnemyMon => Some(3),
            AnimEffect::HideEnemyMon => Some(3),
            AnimEffect::BlinkPlayerMon { .. } | AnimEffect::BlinkEnemyMon { .. } => Some(78),
            AnimEffect::SpiralBallsInward => Some(110),
            AnimEffect::PetalsFalling | AnimEffect::LeavesFalling => Some(166),
            AnimEffect::TransformMon => Some(match attacker {
                MonSide::Player => 29,
                MonSide::Enemy => 51,
            }),
            AnimEffect::SlidePlayerMonDownAndHide => Some(23),
            AnimEffect::MinimizeMon => Some(13),
            AnimEffect::SubstituteMon => Some(10),
            AnimEffect::ShootBallsUpward { many: false } => Some(21),
            AnimEffect::SquishMonPic => Some(25),
            AnimEffect::SlidePlayerMonHalfOff => Some(19),
            AnimEffect::SlidePlayerMonUp => Some(14),
            AnimEffect::ResetPlayerMonPosition => Some(3),
            AnimEffect::ShakeScreenHV {
                pixels: 1,
                frames: 9,
            } => Some(15),
            _ => None,
        }
    }

    fn resolve_transform(&self, raw: SubAnimTransform) -> SubAnimTransform {
        match raw {
            SubAnimTransform::Enemy => {
                if self.player_is_attacker {
                    SubAnimTransform::HFlip
                } else {
                    SubAnimTransform::Normal
                }
            }
            _other if self.player_is_attacker => SubAnimTransform::Normal,
            other => other,
        }
    }

    fn tile_load_frames(tileset: u8) -> u8 {
        // MoveAnimationTiles0/1 contain 79 tiles; tileset 2 contains 64.
        // CopyVideoData uploads eight tiles on each VBlank, rounding up.
        if tileset == 2 {
            8
        } else {
            10
        }
    }

    fn clear_oam(&mut self) {
        self.oam_slots.fill(None);
        self.oam_buffer.clear();
    }

    fn rebuild_oam_buffer(&mut self) {
        self.oam_buffer.clear();
        self.oam_buffer
            .extend(self.oam_slots.iter().filter_map(|entry| *entry));
    }

    fn write_frame(
        &mut self,
        frame: SubAnimFrame,
        transform: SubAnimTransform,
        dest: usize,
    ) -> usize {
        let mut entries = Vec::new();
        DataAnimationPlayer::render_frame_block(
            frame.frame_block_id as usize,
            frame.base_coord_id as usize,
            transform,
            &mut entries,
        );
        let written = entries.len().min(MAX_OAM_ENTRIES.saturating_sub(dest));
        for (slot, mut entry) in entries.into_iter().take(written).enumerate() {
            entry.x -= OAM_X_OFFSET;
            entry.y -= OAM_Y_OFFSET;
            self.oam_slots[dest + slot] = Some(entry);
        }
        self.rebuild_oam_buffer();
        written
    }

    fn copy_growl_note(&mut self) {
        for i in 0..4 {
            self.oam_slots[4 + i] = self.oam_slots[i];
        }
        self.rebuild_oam_buffer();
    }

    fn write_water_droplets(&mut self, base_x: &mut u8, half: u8) {
        self.clear_oam();
        let mut y = if half == 0 { 16u8 } else { 24u8 };
        let mut slot = 0;
        loop {
            *base_x = base_x.wrapping_add(27);
            if slot < MAX_OAM_ENTRIES {
                self.oam_slots[slot] = Some(SpriteOamEntry::new(
                    i32::from(y) - OAM_Y_OFFSET,
                    i32::from(*base_x) - OAM_X_OFFSET,
                    0x71,
                    0,
                ));
                slot += 1;
            }
            if *base_x < 144 {
                continue;
            }
            *base_x = base_x.wrapping_sub(168);
            y = y.wrapping_add(16);
            if y >= 112 {
                break;
            }
        }
        self.rebuild_oam_buffer();
    }

    fn write_spiral_balls(&mut self, step: usize) {
        self.clear_oam();
        let (base_y, base_x) = if self.player_is_attacker {
            (0u8, 0u8)
        } else {
            ((-40i8) as u8, 80u8)
        };
        for (slot, &(y, x)) in SPIRAL_BALL_COORDS[step..step + 3].iter().enumerate() {
            self.oam_slots[slot] = Some(SpriteOamEntry::new(
                i32::from(base_y.wrapping_add(y)) - OAM_Y_OFFSET,
                i32::from(base_x.wrapping_add(x)) - OAM_X_OFFSET,
                BALL_TILE,
                0,
            ));
        }
        self.rebuild_oam_buffer();
    }

    fn write_spiral_terminator_probe(&mut self) {
        // The twentieth loop iteration updates two entries before the third
        // coordinate probe encounters $ff. The cleanup VBlank therefore
        // exposes this partially updated OAM once.
        let (base_y, base_x) = if self.player_is_attacker {
            (0u8, 0u8)
        } else {
            ((-40i8) as u8, 80u8)
        };
        for (slot, &(y, x)) in SPIRAL_BALL_COORDS[19..].iter().enumerate() {
            self.oam_slots[slot] = Some(SpriteOamEntry::new(
                i32::from(base_y.wrapping_add(y)) - OAM_Y_OFFSET,
                i32::from(base_x.wrapping_add(x)) - OAM_X_OFFSET,
                BALL_TILE,
                0,
            ));
        }
        self.rebuild_oam_buffer();
    }

    fn update_falling_objects(&mut self, objects: &mut [FallingObject], tile: u8) {
        self.clear_oam();
        for (slot, object) in objects.iter_mut().enumerate() {
            let next = object.movement.wrapping_add(1);
            object.movement = if next & 0x7f == 9 {
                (next & 0x80) ^ 0x80
            } else {
                next
            };

            object.y = object.y.wrapping_add(2);
            if object.y >= 112 {
                object.y = 160;
            }
            let delta = FALLING_DELTA_BYTES[(object.movement & 0x7f) as usize];
            let attributes = if object.movement & 0x80 != 0 {
                object.x = object.x.wrapping_sub(delta);
                OAM_XFLIP
            } else {
                object.x = object.x.wrapping_add(delta);
                0
            };
            self.oam_slots[slot] = Some(SpriteOamEntry::new(
                i32::from(object.y) - OAM_Y_OFFSET,
                i32::from(object.x) - OAM_X_OFFSET,
                tile,
                attributes,
            ));
        }
        self.rebuild_oam_buffer();
    }

    fn write_shooting_balls(&mut self, base_x: u8, balls: &[u8]) {
        self.clear_oam();
        for (slot, &y) in balls.iter().enumerate() {
            self.oam_slots[slot] = Some(SpriteOamEntry::new(
                i32::from(y) - OAM_Y_OFFSET,
                i32::from(base_x) - OAM_X_OFFSET,
                BALL_TILE,
                0,
            ));
        }
        self.rebuild_oam_buffer();
    }

    fn tick_internal_effect(&mut self) -> Option<AnimTickResult> {
        let mut state = self.internal_effect.take()?;
        let (result, keep) = match &mut state {
            InternalEffect::SpiralBalls {
                sound_pending,
                load_remaining,
                step,
                hold_remaining,
                cleanup_shown,
            } => {
                let sound = sound_pending.take();
                if *load_remaining > 0 {
                    *load_remaining -= 1;
                    (Some(AnimTickResult::Loading { sound }), true)
                } else if *step < 19 {
                    if *hold_remaining == 0 {
                        self.write_spiral_balls(*step as usize);
                        *hold_remaining = 5;
                    }
                    *hold_remaining -= 1;
                    if *hold_remaining == 0 {
                        *step += 1;
                    }
                    (Some(AnimTickResult::Display { sound }), true)
                } else if !*cleanup_shown {
                    self.write_spiral_terminator_probe();
                    *cleanup_shown = true;
                    (Some(AnimTickResult::Display { sound }), true)
                } else {
                    self.clear_oam();
                    (
                        Some(AnimTickResult::Hook {
                            sound,
                            effect: AnimEffect::FlashScreen { frames: 4 },
                        }),
                        false,
                    )
                }
            }
            InternalEffect::FallingObjects {
                sound_pending,
                load_remaining,
                tile,
                objects,
                steps_remaining,
                hold_remaining,
                clear_after,
            } => {
                let sound = sound_pending.take();
                if *load_remaining > 0 {
                    *load_remaining -= 1;
                    (Some(AnimTickResult::Loading { sound }), true)
                } else if *hold_remaining > 0 {
                    *hold_remaining -= 1;
                    (Some(AnimTickResult::Display { sound }), true)
                } else if *steps_remaining > 0 {
                    self.update_falling_objects(objects, *tile);
                    *steps_remaining -= 1;
                    // This returned display frame is the first Delay3 frame.
                    *hold_remaining = 2;
                    (Some(AnimTickResult::Display { sound }), true)
                } else {
                    if *clear_after {
                        self.clear_oam();
                    }
                    (None, false)
                }
            }
            InternalEffect::ShootBalls {
                sound_pending,
                load_remaining,
                base_y,
                pillar_xs,
                pillar_index,
                ball_count,
                balls,
                initialized,
            } => {
                let sound = sound_pending.take();
                if *load_remaining > 0 {
                    *load_remaining -= 1;
                    (Some(AnimTickResult::Loading { sound }), true)
                } else if !*initialized {
                    balls.clear();
                    balls.extend((1..=*ball_count).map(|i| base_y.wrapping_add(i * 8)));
                    self.write_shooting_balls(pillar_xs[*pillar_index], balls);
                    *initialized = true;
                    (Some(AnimTickResult::Display { sound }), true)
                } else if !balls.is_empty() {
                    let top = base_y.wrapping_add(8);
                    for y in balls.iter_mut() {
                        if *y == top {
                            *y = 0;
                        } else {
                            *y = y.wrapping_sub(4);
                        }
                    }
                    balls.retain(|&y| y != 0);
                    self.write_shooting_balls(pillar_xs[*pillar_index], balls);
                    (Some(AnimTickResult::Display { sound }), true)
                } else if *pillar_index + 1 < pillar_xs.len() {
                    *pillar_index += 1;
                    *load_remaining = 9;
                    *initialized = false;
                    // Consume the first tile-upload VBlank of the next pillar.
                    (Some(AnimTickResult::Loading { sound }), true)
                } else {
                    self.clear_oam();
                    // AnimationCleanOAM waits one frame before ClearSprites.
                    (Some(AnimTickResult::Display { sound }), false)
                }
            }
            InternalEffect::WaterDroplets {
                sound_pending,
                load_remaining,
                calls_remaining,
                showing,
                half,
                base_x,
            } => {
                let sound = sound_pending.take();
                if *load_remaining > 0 {
                    *load_remaining -= 1;
                    (Some(AnimTickResult::Loading { sound }), true)
                } else if *showing {
                    self.write_water_droplets(base_x, *half);
                    *showing = false;
                    (Some(AnimTickResult::Display { sound }), true)
                } else {
                    self.clear_oam();
                    *showing = true;
                    *half ^= 1;
                    *calls_remaining -= 1;
                    (
                        Some(AnimTickResult::Display { sound }),
                        *calls_remaining != 0,
                    )
                }
            }
            InternalEffect::WavyScreen {
                sound_pending,
                frame,
            } => {
                let sound = sound_pending.take();
                self.clear_oam();
                if (7..134).contains(frame) {
                    self.display_wave_phase = Some(*frame - 7);
                }
                *frame += 1;
                (Some(AnimTickResult::Display { sound }), *frame < 143)
            }
            InternalEffect::ShakeScreen {
                sound_pending,
                frame,
                call,
            } => {
                let sound = sound_pending.take();
                self.display_shake = Some((*call, *frame));
                *frame += 1;
                if *frame == 72 {
                    self.shake_restore_pending = Some(*call);
                }
                (Some(AnimTickResult::Display { sound }), *frame < 72)
            }
        };
        if keep {
            self.internal_effect = Some(state);
        }
        result
    }

    fn start_pending_subanimation(&mut self) {
        let pending = self
            .pending_subanim
            .take()
            .expect("tile upload can only finish for a pending subanimation");
        let subanimation = get_subanimation(pending.subanim_id as usize);
        let transform = self.resolve_transform(subanimation.transform);
        self.subanim = Some(SubAnimState {
            frames: subanimation.frames,
            transform,
            frame_index: 0,
            delay: pending.delay,
            sound_pending: true,
            sound_id: pending.sound_id,
            // PlaySubanimation always resets wFBDestAddr to wShadowOAM.
            dest_slot: 0,
        });
    }

    fn resume_frame_wait(&mut self, carry_sound: &mut Option<u8>) -> Option<AnimTickResult> {
        let wait = self.frame_wait.as_mut()?;
        if wait.remaining > 0 {
            wait.remaining -= 1;
            return Some(AnimTickResult::Display {
                sound: carry_sound.take(),
            });
        }
        if wait.cleanup_frame_pending {
            wait.cleanup_frame_pending = false;
            wait.clear_before_resume = true;
            return Some(AnimTickResult::Display {
                sound: carry_sound.take(),
            });
        }

        let wait = self.frame_wait.take().expect("frame wait present");
        if wait.clear_before_resume {
            self.clear_oam();
            if let Some(state) = self.subanim.as_mut() {
                state.dest_slot = 0;
            }
        }
        match wait.post {
            PostFrameAction::None => None,
            PostFrameAction::Hook(effect) => Some(AnimTickResult::Hook {
                sound: carry_sound.take(),
                effect,
            }),
            PostFrameAction::Growl { final_frame } => {
                self.copy_growl_note();
                if final_frame {
                    // DoGrowlSpecialEffects calls AnimationCleanOAM after the
                    // final note: one visible frame, then ClearSprites.
                    self.frame_wait = Some(FrameWait {
                        remaining: 0,
                        cleanup_frame_pending: false,
                        clear_before_resume: true,
                        post: PostFrameAction::None,
                    });
                    Some(AnimTickResult::Display {
                        sound: carry_sound.take(),
                    })
                } else {
                    None
                }
            }
        }
    }

    /// Advance until one display frame, one blocking/zero-time effect, or the
    /// end of the animation is reached.
    pub fn tick(&mut self) -> AnimTickResult {
        if self.finished {
            return AnimTickResult::Done;
        }

        self.display_wave_phase = None;
        self.display_shake = self.shake_restore_pending.take().map(|call| (call, 72));
        let mut carry_sound = None;
        loop {
            if self.internal_effect.is_some() {
                if let Some(result) = self.tick_internal_effect() {
                    return result;
                }
            }

            if self.frame_wait.is_some() {
                if let Some(result) = self.resume_frame_wait(&mut carry_sound) {
                    return result;
                }
            }

            if self.tile_load_remaining > 0 {
                self.tile_load_remaining -= 1;
                if self.tile_load_remaining == 0 {
                    // The final CopyVideoData VBlank is still a loading frame;
                    // frame-block execution resumes on the next update.
                }
                return AnimTickResult::Loading {
                    sound: carry_sound.take(),
                };
            }

            if self.pending_subanim.is_some() {
                self.start_pending_subanimation();
            }

            if let Some(mut state) = self.subanim.take() {
                if state.frame_index < state.frames.len() {
                    let logical_index = if state.transform == SubAnimTransform::Reverse {
                        state.frames.len() - 1 - state.frame_index
                    } else {
                        state.frame_index
                    };
                    let frame = state.frames[logical_index];
                    let counter = (state.frames.len() - state.frame_index) as u8;
                    let sound = if state.sound_pending {
                        state.sound_pending = false;
                        (state.sound_id != 0).then_some(state.sound_id)
                    } else {
                        None
                    };
                    if carry_sound.is_none() {
                        carry_sound = sound;
                    }

                    let written = self.write_frame(frame, state.transform, state.dest_slot);
                    state.frame_index += 1;
                    if frame.mode.advances_dest() {
                        state.dest_slot = (state.dest_slot + written).min(MAX_OAM_ENTRIES);
                    }

                    let frame_hook = get_frame_hook(self.animation_id as u8 + 1);
                    self.ball_frame_event = match frame_hook {
                        Some(FrameHook::BallToss) => Some(BallFrameEvent::Toss { counter }),
                        Some(FrameHook::BallShake) => Some(BallFrameEvent::Shake { counter }),
                        Some(FrameHook::BallPoof) => Some(BallFrameEvent::Poof { counter }),
                        _ => None,
                    };
                    let post = if frame_hook == Some(FrameHook::Growl) {
                        PostFrameAction::Growl {
                            final_frame: counter == 1,
                        }
                    } else {
                        frame_hook
                            .and_then(|hook| hook.effect_for_counter(counter))
                            .map_or(PostFrameAction::None, PostFrameAction::Hook)
                    };

                    self.subanim = Some(state);
                    match frame.mode {
                        FrameBlockMode::Mode02 => match post {
                            PostFrameAction::None => continue,
                            PostFrameAction::Hook(effect) => {
                                return AnimTickResult::Hook {
                                    sound: carry_sound.take(),
                                    effect,
                                };
                            }
                            PostFrameAction::Growl { .. } => {
                                unreachable!("Growl frame blocks are delayed")
                            }
                        },
                        FrameBlockMode::Mode00 | FrameBlockMode::Mode01 => {
                            let growl = matches!(&post, PostFrameAction::Growl { .. });
                            self.frame_wait = Some(FrameWait {
                                remaining: state_delay(self.subanim.as_ref()).saturating_sub(1),
                                cleanup_frame_pending: !growl,
                                clear_before_resume: false,
                                post,
                            });
                            return AnimTickResult::Display {
                                sound: carry_sound.take(),
                            };
                        }
                        FrameBlockMode::Mode03 | FrameBlockMode::Mode04 => {
                            self.frame_wait = Some(FrameWait {
                                remaining: state_delay(self.subanim.as_ref()).saturating_sub(1),
                                cleanup_frame_pending: false,
                                clear_before_resume: false,
                                post,
                            });
                            return AnimTickResult::Display {
                                sound: carry_sound.take(),
                            };
                        }
                    }
                }

                // PlaySubanimation returned; the following command executes
                // immediately unless it performs its own VBlank wait.
                self.subanim = None;
                continue;
            }

            let Some(command) = self.commands.get(self.command_index).copied() else {
                self.finished = true;
                if !self.preserves_oam_when_finished() {
                    self.clear_oam();
                }
                return AnimTickResult::Done;
            };
            self.command_index += 1;
            match command {
                AnimCommand::SubAnim {
                    sound_id,
                    subanim_id,
                    tileset,
                    delay,
                } => {
                    self.current_tileset = Some(tileset);
                    self.pending_subanim = Some(PendingSubAnim {
                        subanim_id,
                        delay,
                        sound_id,
                    });
                    self.tile_load_remaining = Self::tile_load_frames(tileset);
                    // Consume the first CopyVideoData VBlank now.
                    self.tile_load_remaining -= 1;
                    return AnimTickResult::Loading {
                        sound: carry_sound.take(),
                    };
                }
                AnimCommand::Effect { sound_id, effect } => {
                    let sound = (sound_id != 0).then_some(sound_id).or(carry_sound);
                    let internal = match effect {
                        SpecialEffect::SpiralBallsInward => {
                            self.current_tileset = Some(0);
                            Some(InternalEffect::SpiralBalls {
                                sound_pending: sound,
                                load_remaining: 10,
                                step: 0,
                                hold_remaining: 0,
                                cleanup_shown: false,
                            })
                        }
                        SpecialEffect::PetalsFalling | SpecialEffect::LeavesFalling => {
                            self.current_tileset = Some(1);
                            let petals = effect == SpecialEffect::PetalsFalling;
                            let count = if petals { 20 } else { 3 };
                            let objects = (0..count)
                                .map(|i| FallingObject {
                                    y: if i == 0 { 0 } else { 8 * (i as u8 + 1) },
                                    x: FALLING_INITIAL_X[i],
                                    movement: FALLING_INITIAL_MOVEMENT[i],
                                })
                                .collect();
                            Some(InternalEffect::FallingObjects {
                                sound_pending: sound,
                                load_remaining: 10,
                                tile: if petals { PETAL_TILE } else { LEAF_TILE },
                                objects,
                                steps_remaining: 52,
                                hold_remaining: 0,
                                clear_after: petals,
                            })
                        }
                        SpecialEffect::ShootBallsUpward | SpecialEffect::ShootManyBallsUpward => {
                            self.current_tileset = Some(0);
                            let many = effect == SpecialEffect::ShootManyBallsUpward;
                            let (base_y, pillar_xs, ball_count) = if many {
                                if self.player_is_attacker {
                                    (0x50, UPWARD_BALLS_X_PLAYER.to_vec(), 4)
                                } else {
                                    (0x28, UPWARD_BALLS_X_ENEMY.to_vec(), 4)
                                }
                            } else if self.player_is_attacker {
                                (6 * 8, vec![5 * 8], 5)
                            } else {
                                (0, vec![16 * 8], 5)
                            };
                            Some(InternalEffect::ShootBalls {
                                sound_pending: sound,
                                load_remaining: 10,
                                base_y,
                                pillar_xs,
                                pillar_index: 0,
                                ball_count,
                                balls: Vec::with_capacity(ball_count as usize),
                                initialized: false,
                            })
                        }
                        SpecialEffect::WaterDropletsEverywhere => {
                            Some(InternalEffect::WaterDroplets {
                                sound_pending: sound,
                                load_remaining: 10,
                                calls_remaining: 64,
                                showing: true,
                                half: 0,
                                base_x: (-16i8) as u8,
                            })
                        }
                        SpecialEffect::WavyScreen => Some(InternalEffect::WavyScreen {
                            sound_pending: sound,
                            frame: 0,
                        }),
                        SpecialEffect::ShakeScreen => {
                            let call = self.shake_call_count;
                            self.shake_call_count = self.shake_call_count.saturating_add(1);
                            Some(InternalEffect::ShakeScreen {
                                sound_pending: sound,
                                frame: 0,
                                call,
                            })
                        }
                        _ => None,
                    };
                    if let Some(internal) = internal {
                        self.clear_oam();
                        self.internal_effect = Some(internal);
                        continue;
                    }
                    return AnimTickResult::Effect { sound, effect };
                }
            }
        }
    }
}

fn shift_horizontal_band(fb: &mut crate::FrameBuffer, y_start: u32, y_end: u32, dx: i32) {
    let width = fb.width() as i32;
    let source = fb.indexed().clone();
    for y in y_start.min(fb.height())..y_end.min(fb.height()) {
        for x in 0..width {
            let source_x = x - dx;
            let color = if (0..width).contains(&source_x) {
                source
                    .get_pixel(source_x as u32, y)
                    .unwrap_or(GbColor::White)
            } else {
                GbColor::White
            };
            fb.set_pixel_index(x as u32, y, color);
        }
    }
}

fn shift_vertical_band(fb: &mut crate::FrameBuffer, dy: i32) {
    let height = fb.height() as i32;
    let source = fb.indexed().clone();
    for y in 0..height {
        let source_y = y - dy;
        for x in 0..fb.width() {
            let color = if (0..height).contains(&source_y) {
                source
                    .get_pixel(x, source_y as u32)
                    .unwrap_or(GbColor::White)
            } else {
                GbColor::White
            };
            fb.set_pixel_index(x, y as u32, color);
        }
    }
}

/// Return the window offset observed on one scanline of
/// `PredefShakeScreenHorizontally`. The routine writes WX during active LCD
/// scanout, so the transition frames contain two (and, between consecutive
/// Earthquake calls, three) horizontal bands.
fn shake_line_offset(move_id: u8, player_is_attacker: bool, call: u8, phase: u8, y: u32) -> i32 {
    if !matches!(move_id, 69 | 89 | 90)
        || (move_id == 69 && call != 0)
        || (matches!(move_id, 89 | 90) && call > 1)
    {
        if phase < 72 && phase % 9 < 5 {
            return 8 - i32::from(phase / 9);
        }
        return 0;
    }

    const ENTRY_PHASES: [u8; 8] = [0, 14, 23, 32, 41, 50, 59, 68];
    const EXIT_PHASES: [u8; 8] = [5, 18, 27, 36, 45, 54, 63, 72];
    const PLAYER_ENTRY: [[u8; 8]; 2] = [[22, 9, 9, 9, 9, 9, 9, 9], [21, 9, 17, 9, 9, 9, 9, 9]];
    const ENEMY_ENTRY: [[u8; 8]; 2] = [[23, 9, 9, 9, 9, 9, 9, 9], [21, 9, 9, 9, 9, 17, 9, 9]];
    const PLAYER_EXIT: [[u8; 8]; 2] = [
        [9, 10, 27, 10, 9, 10, 10, 9],
        [9, 10, 19, 10, 9, 10, 10, 18],
    ];
    const ENEMY_EXIT: [[u8; 8]; 2] = [
        [23, 10, 18, 10, 10, 17, 10, 9],
        [9, 10, 19, 9, 10, 10, 10, 10],
    ];
    let call = usize::from(call);
    let (entry, exit) = match (move_id, player_is_attacker, call) {
        (89, true, call) => (PLAYER_ENTRY[call], PLAYER_EXIT[call]),
        (89, false, call) => (ENEMY_ENTRY[call], ENEMY_EXIT[call]),
        (90, true, 0) => (
            [12, 10, 27, 18, 9, 9, 18, 9],
            [9, 17, 10, 9, 10, 10, 10, 17],
        ),
        (90, false, 0) => (
            [12, 10, 18, 17, 10, 17, 18, 9],
            [9, 17, 9, 10, 10, 10, 10, 9],
        ),
        (90, true, 1) => ([12, 9, 18, 17, 9, 9, 9, 9], [25, 17, 9, 9, 9, 17, 9, 17]),
        (90, false, 1) => ([12, 9, 39, 25, 9, 9, 9, 9], [17, 17, 9, 9, 9, 9, 9, 17]),
        (69, true, 0) => ([11, 9, 17, 16, 16, 9, 9, 8], [9, 16, 10, 9, 9, 10, 8, 30]),
        (69, false, 0) => ([11, 9, 9, 16, 16, 17, 8, 8], [9, 16, 9, 10, 9, 10, 8, 8]),
        _ => unreachable!("validated shake move/call pair"),
    };

    // The final 1px displacement of the first call is restored during the
    // same scanout in which the second call establishes its 8px shift.
    if move_id == 89 && call == 1 && phase == 0 {
        return if y < 9 {
            1
        } else if y < u32::from(entry[0]) {
            0
        } else {
            8
        };
    }

    for index in 0..8 {
        let amplitude = 8 - index as i32;
        let entry_phase = ENTRY_PHASES[index];
        let exit_phase = EXIT_PHASES[index];
        if phase == entry_phase {
            return if y >= u32::from(entry[index]) {
                amplitude
            } else {
                0
            };
        }
        let full_end = if index == 0 {
            entry_phase + 4
        } else {
            entry_phase + 3
        };
        if (entry_phase + 1..=full_end).contains(&phase) {
            return amplitude;
        }
        if phase == exit_phase {
            return if y < u32::from(exit[index]) {
                amplitude
            } else {
                0
            };
        }
    }
    0
}

fn apply_shake_screen(
    fb: &mut crate::FrameBuffer,
    move_id: u8,
    player_is_attacker: bool,
    call: u8,
    phase: u8,
) {
    let width = fb.width() as i32;
    let source = fb.indexed().clone();
    for y in 0..fb.height() {
        let dx = shake_line_offset(move_id, player_is_attacker, call, phase, y);
        for x in 0..width {
            let source_x = x - dx;
            let color = if (0..width).contains(&source_x) {
                source
                    .get_pixel(source_x as u32, y)
                    .unwrap_or(GbColor::White)
            } else {
                GbColor::White
            };
            fb.set_pixel_index(x as u32, y, color);
        }
    }
}

fn apply_wavy_screen(
    fb: &mut crate::FrameBuffer,
    move_id: u8,
    player_is_attacker: bool,
    phase: u8,
) {
    let width = fb.width() as i32;
    let source = fb.indexed().clone();
    for y in 0..fb.height() {
        let shift =
            crate::gen1_wavy_schedule::line_offset(move_id, player_is_attacker, phase, y) as i32;
        for x in 0..width {
            let source_x = x + shift;
            let color = if (0..width).contains(&source_x) {
                source
                    .get_pixel(source_x as u32, y)
                    .unwrap_or(GbColor::White)
            } else {
                GbColor::White
            };
            fb.set_pixel_index(x as u32, y, color);
        }
    }
}

fn apply_bgp_split(fb: &mut crate::FrameBuffer, top_bgp: u8, bottom_bgp: u8, split_y: u32) {
    apply_bgp_bands(fb, top_bgp, &[(split_y, bottom_bgp)]);
}

fn bake_bgp(fb: &mut crate::FrameBuffer, bgp: u8) {
    apply_bgp_bands(fb, bgp, &[]);
}

fn apply_bgp_bands(fb: &mut crate::FrameBuffer, initial_bgp: u8, writes: &[(u32, u8)]) {
    let source = fb.indexed().clone();
    fb.reset_palette();
    for y in 0..fb.height() {
        let mut bgp = initial_bgp;
        for &(scanline, written_bgp) in writes {
            if y < scanline {
                break;
            }
            bgp = written_bgp;
        }
        for x in 0..fb.width() {
            let index = source.get_pixel(x, y).unwrap_or(GbColor::White) as u8;
            let shade = (bgp >> (index * 2)) & 0x03;
            fb.set_pixel_index(x, y, GbColor::from_u8(shade));
        }
    }
}

fn state_delay(state: Option<&SubAnimState>) -> u8 {
    state.map_or(0, |state| state.delay)
}

impl Default for AnimationPlayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oam_signature(entries: &[SpriteOamEntry]) -> Vec<(i32, i32, u8, u8)> {
        entries
            .iter()
            .map(|entry| (entry.x, entry.y, entry.tile_id, entry.attributes))
            .collect()
    }

    fn collect(move_id: usize) -> Vec<(AnimTickResult, Vec<SpriteOamEntry>)> {
        let mut player = AnimationPlayer::new();
        player.start(move_id, true);
        let mut trace = Vec::new();
        for _ in 0..10_000 {
            let result = player.tick();
            trace.push((result.clone(), player.oam_entries().to_vec()));
            if result == AnimTickResult::Done {
                return trace;
            }
        }
        panic!("animation did not finish");
    }

    fn collect_effect(
        effect: SpecialEffect,
        player_is_attacker: bool,
    ) -> Vec<(AnimTickResult, Vec<SpriteOamEntry>)> {
        let mut player = AnimationPlayer::new();
        player.player_is_attacker = player_is_attacker;
        player.commands = vec![AnimCommand::Effect {
            sound_id: 0,
            effect,
        }];
        player.finished = false;
        let mut trace = Vec::new();
        for _ in 0..10_000 {
            let result = player.tick();
            trace.push((result.clone(), player.oam_entries().to_vec()));
            if result == AnimTickResult::Done {
                return trace;
            }
        }
        panic!("effect did not finish");
    }

    #[test]
    fn pound_matches_original_vblank_timing_and_screen_coordinates() {
        let trace = collect(0);
        assert_eq!(trace.len(), 29);
        assert!(trace[..10]
            .iter()
            .all(
                |(result, oam)| matches!(result, AnimTickResult::Loading { .. }) && oam.is_empty()
            ));
        assert_eq!(trace[10].1[0].x, 104);
        assert_eq!(trace[10].1[0].y, 16);
        let first = oam_signature(&trace[10].1);
        let second = oam_signature(&trace[19].1);
        assert!(trace[10..19]
            .iter()
            .all(|(_, oam)| oam_signature(oam) == first));
        assert!(trace[19..28]
            .iter()
            .all(|(_, oam)| oam_signature(oam) == second));
        assert!(trace[28].1.is_empty());
    }

    #[test]
    fn every_delayed_frame_block_has_a_nonzero_delay() {
        for animation_id in 0..NUM_MOVE_ANIMS {
            for command in get_move_animation(animation_id).commands {
                let AnimCommand::SubAnim {
                    subanim_id, delay, ..
                } = command
                else {
                    continue;
                };
                if get_subanimation(subanim_id as usize)
                    .frames
                    .iter()
                    .any(|frame| frame.mode.has_delay())
                {
                    assert_ne!(
                        delay, 0,
                        "animation {animation_id}, subanimation {subanim_id}"
                    );
                }
            }
        }
    }

    #[test]
    fn enemy_amnesia_and_rest_use_shared_status_animations() {
        let mut player = AnimationPlayer::new();
        player.start(AMNESIA as usize - 1, false);
        assert_eq!(player.animation_id, CONF_ANIM as usize - 1);
        player.start(REST as usize - 1, false);
        assert_eq!(player.animation_id, SLP_ANIM as usize - 1);
    }

    #[test]
    fn special_effect_routines_match_reference_frame_counts() {
        assert_eq!(collect(56).len(), 261, "Surf / water droplets");
        assert_eq!(collect(88).len(), 145, "Earthquake / screen shake");
        assert_eq!(collect(148).len(), 224, "Psywave / wavy screen");
    }

    #[test]
    fn nested_effect_waits_include_their_assembly_helpers() {
        assert_eq!(
            AnimationPlayer::effect_duration(&AnimEffect::SpiralBallsInward, MonSide::Player),
            Some(110)
        );
        assert_eq!(
            AnimationPlayer::effect_duration(
                &AnimEffect::ShakeScreenHV {
                    pixels: 1,
                    frames: 9,
                },
                MonSide::Player,
            ),
            Some(15)
        );
        assert_eq!(
            AnimationPlayer::effect_duration(&AnimEffect::TransformMon, MonSide::Enemy),
            Some(51)
        );
    }

    #[test]
    fn spiral_balls_include_tile_upload_cleanup_and_flash_hook() {
        let trace = collect_effect(SpecialEffect::SpiralBallsInward, false);
        assert_eq!(trace.len(), 108);
        assert!(trace[..10]
            .iter()
            .all(|(result, _)| matches!(result, AnimTickResult::Loading { .. })));
        assert_eq!(oam_signature(&trace[10].1)[0], (112, 0, BALL_TILE, 0));
        assert!(matches!(trace[106].0, AnimTickResult::Hook { .. }));
        assert!(trace[106].1.is_empty());
    }

    #[test]
    fn falling_objects_preserve_original_overflow_table_reads() {
        let trace = collect_effect(SpecialEffect::PetalsFalling, true);
        assert_eq!(trace.len(), 167);
        let first = &trace[10].1;
        assert_eq!(first[9].x, 0xb8, "$89 reads byte $8a and moves left");
        assert_eq!(first[9].attributes, OAM_XFLIP);
        assert_eq!(first[10].x, -7, "$09 reads byte $8a and wraps right");
        assert!(trace.last().unwrap().1.is_empty());
    }

    #[test]
    fn shoot_balls_match_single_and_many_routine_frame_counts() {
        let single = collect_effect(SpecialEffect::ShootBallsUpward, true);
        assert_eq!(single.len(), 22);
        assert_eq!(oam_signature(&single[10].1)[0], (32, 40, BALL_TILE, 0));

        let many = collect_effect(SpecialEffect::ShootManyBallsUpward, true);
        assert_eq!(many.len(), 110);
        assert_eq!(oam_signature(&many[10].1)[0], (8, 72, BALL_TILE, 0));
    }

    #[test]
    fn short_flash_keeps_palette_restore_as_a_nonblocking_raster_edge() {
        use crate::{FrameBuffer, Rgba};
        use dotzuki_engine::render_config::RenderConfig;

        let mut flash = ShortScreenFlash::default();
        flash.start(
            0xe4,
            ShortFlashTiming {
                entry_scanline: 1,
                white_scanline: 1,
                restore_scanline: 1,
            },
        );
        let mut fb = FrameBuffer::new(RenderConfig::new(2, 2), Rgba::WHITE);
        fb.set_pixel(0, 0, Rgba::BLACK);
        fb.set_pixel(0, 1, Rgba::BLACK);
        flash.apply(&mut fb);
        assert_eq!(fb.get_pixel(0, 0), Some(Rgba::BLACK));
        assert_eq!(fb.get_pixel(0, 1), Some(Rgba::WHITE));

        for _ in 0..4 {
            flash.tick();
        }
        assert!(flash.is_restoring());
        flash.tick();
        assert!(!flash.is_restoring());
    }

    #[test]
    fn blink_mon_preserves_scanline_48_tilemap_edges() {
        let mut blink = BlinkMon::default();
        blink.start(MonSide::Player);
        for frame in 0..78 {
            let want = match frame + 1 {
                3 | 7 | 8 | 15 | 42 | 46 | 47 | 54 => Some((48, 144)),
                4..=6 | 16..=20 | 30..=32 | 43..=45 | 55..=59 | 69..=71 => None,
                21 | 28 | 29 | 33 | 60 | 67 | 68 | 72 => Some((0, 48)),
                _ => Some((0, 144)),
            };
            assert_eq!(
                blink.visible_band(MonSide::Player, 144),
                want,
                "frame {frame}"
            );
            assert_eq!(
                blink.visible_band(MonSide::Enemy, 144),
                Some((0, 144)),
                "other side frame {frame}"
            );
            blink.tick();
        }
        assert_eq!(blink.visible_band(MonSide::Player, 144), Some((0, 144)));
    }
}
