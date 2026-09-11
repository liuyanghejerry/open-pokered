use crate::{FrameBuffer, TILE_SIZE};

// Tile $FF is the solid-black battle transition tile (the original loads it at character-RAM tile $7F)
const BLACK_TILE: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleTransitionKind {
    DoubleCircle,
    Spiral { outward: bool },
    Circle,
    SpiralTrainerStronger,
    HorizontalStripes,
    Shrink,
    VerticalStripes,
    Split,
}

// ── Circle arc data ─────────────────────────────────────────────────
//
// Five per-row (fill_count, skip_count) pair tables for the circle wipe,
// terminated by -1 ($FF). The classic circle-arc renderer interprets one
// pair per tilemap row: fill N tiles toward the screen edge, step one row,
// then move the row start M tiles back toward the arc center.
const CIRCLE_DATA_1: &[u8] = &[2, 3, 5, 4, 9, 0xFF];
const CIRCLE_DATA_2: &[u8] = &[1, 1, 2, 2, 4, 2, 4, 2, 3, 0xFF];
const CIRCLE_DATA_3: &[u8] = &[2, 1, 3, 1, 4, 1, 4, 1, 4, 1, 3, 1, 2, 1, 1, 1, 1, 0xFF];
const CIRCLE_DATA_4: &[u8] = &[4, 1, 4, 0, 3, 1, 3, 0, 2, 1, 2, 0, 1, 0xFF];
const CIRCLE_DATA_5: &[u8] = &[4, 0, 3, 0, 3, 0, 2, 0, 2, 0, 1, 0, 1, 0, 1, 0xFF];

// Half-circle side constants (the original's `halves` const block).
const CIRCLE_LEFT: u8 = 0;
const CIRCLE_RIGHT: u8 = 1;

/// One `half_circle` macro entry: quadrant x, circle data, target coord.
struct HalfCircleEntry {
    quadrant_x: u8,
    data: &'static [u8],
    x: i16,
    y: i16,
}

// HalfCircle1 — the top half of the circle wipe.
const HALF_CIRCLE_1: [HalfCircleEntry; 10] = [
    HalfCircleEntry {
        quadrant_x: CIRCLE_RIGHT,
        data: CIRCLE_DATA_1,
        x: 18,
        y: 6,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_RIGHT,
        data: CIRCLE_DATA_2,
        x: 19,
        y: 3,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_RIGHT,
        data: CIRCLE_DATA_3,
        x: 18,
        y: 0,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_RIGHT,
        data: CIRCLE_DATA_4,
        x: 14,
        y: 0,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_RIGHT,
        data: CIRCLE_DATA_5,
        x: 10,
        y: 0,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_LEFT,
        data: CIRCLE_DATA_5,
        x: 9,
        y: 0,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_LEFT,
        data: CIRCLE_DATA_4,
        x: 5,
        y: 0,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_LEFT,
        data: CIRCLE_DATA_3,
        x: 1,
        y: 0,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_LEFT,
        data: CIRCLE_DATA_2,
        x: 0,
        y: 3,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_LEFT,
        data: CIRCLE_DATA_1,
        x: 1,
        y: 6,
    },
];

// HalfCircle2 — the bottom half of the circle wipe.
const HALF_CIRCLE_2: [HalfCircleEntry; 10] = [
    HalfCircleEntry {
        quadrant_x: CIRCLE_LEFT,
        data: CIRCLE_DATA_1,
        x: 1,
        y: 11,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_LEFT,
        data: CIRCLE_DATA_2,
        x: 0,
        y: 14,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_LEFT,
        data: CIRCLE_DATA_3,
        x: 1,
        y: 17,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_LEFT,
        data: CIRCLE_DATA_4,
        x: 5,
        y: 17,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_LEFT,
        data: CIRCLE_DATA_5,
        x: 9,
        y: 17,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_RIGHT,
        data: CIRCLE_DATA_5,
        x: 10,
        y: 17,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_RIGHT,
        data: CIRCLE_DATA_4,
        x: 14,
        y: 17,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_RIGHT,
        data: CIRCLE_DATA_3,
        x: 18,
        y: 17,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_RIGHT,
        data: CIRCLE_DATA_2,
        x: 19,
        y: 14,
    },
    HalfCircleEntry {
        quadrant_x: CIRCLE_RIGHT,
        data: CIRCLE_DATA_1,
        x: 18,
        y: 11,
    },
];

/// State machine for battle screen-wipe transitions.
/// Faithful to the original screen-wipe routines — writing tile $FF to
/// specific tilemap positions at 60fps.
///
/// Copy-based transitions (Shrink/Split) additionally redirect each tile to
/// a shifted source tile (`src`), reproducing the original tile-copy pass:
/// the overworld visibly compresses toward / splits away from the center
/// instead of being eaten by growing black borders.
#[derive(Debug, Clone)]
pub struct BattleTransitionState {
    kind: BattleTransitionKind,
    /// Flattened tile grid (row-major): 0 = show (shifted) overworld pixel,
    /// 1 = black tile. index = row * width_tiles + col
    tiles: Vec<u8>,
    /// Per-tile source tile coordinate in the overworld frame. Identity for
    /// pure-wipe transitions; Shrink/Split shift these the way the original
    /// tile-copy pass does. (Black tiles ignore this.)
    src: Vec<(u8, u8)>,
    width_tiles: usize,
    height_tiles: usize,
    done: bool,
    /// Generic frame counter / step index
    frame: u8,
    /// Secondary counter
    counter: u8,
    /// Generic position trackers
    x: i16,
    y: i16,
    dir: u8,
    /// Half-circle entry index (0..10) and which half (Circle only).
    hc_step: usize,
    hc_half: u8,
    /// Spiral direction state
    spiral_dir: u8,
    spiral_x: i16,
    spiral_y: i16,
}

/// Minimal tile-level framebuffer view used by transition wipes.
///
/// Implemented for the engine's RGBA [`FrameBuffer`] and for the packed
/// indexed [`crate::IndexedFrameBuffer`]; both layouts store an 8×8 tile
/// contiguously, so a tile copy is a small memcpy.
pub trait TransitionFb {
    /// (width, height) in pixels.
    fn size(&self) -> (usize, usize);
    /// Fill the 8×8 tile at tile coordinates (`tx`, `ty`) with black.
    /// Out-of-bounds tiles are clamped to the visible area.
    fn tile_black(&mut self, tx: usize, ty: usize);
    /// Copy the 8×8 tile at tile coordinates (`stx`, `sty`) of `src` into
    /// tile (`tx`, `ty`) of `self`. Out-of-bounds areas are clamped.
    fn tile_copy(&mut self, tx: usize, ty: usize, src: &Self, stx: usize, sty: usize);
}

impl TransitionFb for FrameBuffer {
    fn size(&self) -> (usize, usize) {
        (self.width as usize, self.height as usize)
    }
    fn tile_black(&mut self, tx: usize, ty: usize) {
        let (w, h) = self.size();
        let px = tx * TILE_SIZE as usize;
        let py = ty * TILE_SIZE as usize;
        for dy in 0..TILE_SIZE as usize {
            let y = py + dy;
            if y >= h {
                break;
            }
            for dx in 0..TILE_SIZE as usize {
                let x = px + dx;
                if x >= w {
                    break;
                }
                let off = (y * w + x) * 4;
                self.data[off] = 0;
                self.data[off + 1] = 0;
                self.data[off + 2] = 0;
            }
        }
    }
    fn tile_copy(&mut self, tx: usize, ty: usize, src: &Self, stx: usize, sty: usize) {
        let (w, h) = self.size();
        let px = tx * TILE_SIZE as usize;
        let py = ty * TILE_SIZE as usize;
        let spx = stx * TILE_SIZE as usize;
        let spy = sty * TILE_SIZE as usize;
        for dy in 0..TILE_SIZE as usize {
            let y = py + dy;
            let sy = spy + dy;
            if y >= h || sy >= h {
                break;
            }
            for dx in 0..TILE_SIZE as usize {
                let x = px + dx;
                let sx = spx + dx;
                if x >= w || sx >= w {
                    break;
                }
                let off = (y * w + x) * 4;
                let soff = (sy * w + sx) * 4;
                self.data[off..off + 4].copy_from_slice(&src.data[soff..soff + 4]);
            }
        }
    }
}

impl<C: crate::palette::ColorIndex> TransitionFb for crate::IndexedFrameBuffer<C> {
    fn size(&self) -> (usize, usize) {
        (self.width(), self.height())
    }
    fn tile_black(&mut self, tx: usize, ty: usize) {
        // Index 3 is black (GbColor::Black); the palette maps it to the
        // darkest shade at present time.
        self.fill_rect(
            (tx * TILE_SIZE as usize) as u32,
            (ty * TILE_SIZE as usize) as u32,
            TILE_SIZE,
            TILE_SIZE,
            C::from_u8(3),
        );
    }
    fn tile_copy(&mut self, tx: usize, ty: usize, src: &Self, stx: usize, sty: usize) {
        let bits = crate::index_bits::<C>();
        let gpr = (self.width() + 7) / 8;
        let (w, h) = self.size();
        let px = tx * TILE_SIZE as usize;
        let py = ty * TILE_SIZE as usize;
        let spx = stx * TILE_SIZE as usize;
        let spy = sty * TILE_SIZE as usize;
        for dy in 0..TILE_SIZE as usize {
            let y = py + dy;
            let sy = spy + dy;
            if y >= h || sy >= h {
                break;
            }
            for dx in 0..TILE_SIZE as usize {
                let x = px + dx;
                let sx = spx + dx;
                if x >= w || sx >= w {
                    break;
                }
                // Packed layout: per row, bytes run `gpr * bits` per
                // row-group; a pixel spans one bit per plane byte.
                let doff = (y * gpr + x / 8) * bits;
                let soff = (sy * gpr + sx / 8) * bits;
                let bit_shift = 7 - (x % 8);
                let sbit_shift = 7 - (sx % 8);
                let dst = self.packed_mut();
                for plane in 0..bits {
                    let sb = (src.packed()[soff + plane] >> sbit_shift) & 1;
                    dst[doff + plane] = (dst[doff + plane] & !(1 << bit_shift)) | (sb << bit_shift);
                }
            }
        }
    }
}

impl<C: crate::palette::ColorIndex> TransitionFb for crate::RgbaIndexedFrameBuffer<C> {
    fn size(&self) -> (usize, usize) {
        (self.width() as usize, self.height() as usize)
    }
    fn tile_black(&mut self, tx: usize, ty: usize) {
        self.indexed_mut().tile_black(tx, ty);
    }
    fn tile_copy(&mut self, tx: usize, ty: usize, src: &Self, stx: usize, sty: usize) {
        self.indexed_mut()
            .tile_copy(tx, ty, src.indexed(), stx, sty);
    }
}

impl BattleTransitionState {
    pub fn new(kind: BattleTransitionKind, width_tiles: usize, height_tiles: usize) -> Self {
        let w = width_tiles;
        let h = height_tiles;
        let mut src = Vec::with_capacity(w * h);
        for ty in 0..h {
            for tx in 0..w {
                src.push((tx as u8, ty as u8));
            }
        }
        Self {
            kind,
            tiles: vec![0u8; w * h],
            src,
            width_tiles: w,
            height_tiles: h,
            done: false,
            frame: 0,
            counter: 0,
            x: 0,
            y: 0,
            dir: 0,
            hc_step: 0,
            hc_half: 0,
            spiral_dir: 3,
            spiral_x: 10,
            spiral_y: 10,
        }
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn tick(&mut self) {
        if self.done {
            return;
        }
        match self.kind {
            BattleTransitionKind::DoubleCircle => self.tick_double_circle(),
            BattleTransitionKind::Spiral { outward } => {
                if outward {
                    self.tick_spiral_outward()
                } else {
                    self.tick_spiral_inward()
                }
            }
            BattleTransitionKind::Circle => self.tick_circle(),
            BattleTransitionKind::SpiralTrainerStronger => self.tick_spiral_outward(),
            BattleTransitionKind::HorizontalStripes => self.tick_horizontal_stripes(),
            BattleTransitionKind::Shrink => self.tick_shrink(),
            BattleTransitionKind::VerticalStripes => self.tick_vertical_stripes(),
            BattleTransitionKind::Split => self.tick_split(),
        }
    }

    /// Render the transition onto `dest_fb`.
    /// `source_fb` is the overworld frame to wipe away.
    /// Returns true when the screen is fully blacked (for downstream silhouette slide).
    ///
    /// Generic over [`TransitionFb`]: the RGBA engine buffer and the packed
    /// 2bpp indexed buffer both implement it (tile-granular copies, so the
    /// indexed variant moves 16 bytes per tile instead of 256).
    pub fn render<F: TransitionFb>(&self, source_fb: &F, dest_fb: &mut F) -> bool {
        for ty in 0..self.height_tiles {
            for tx in 0..self.width_tiles {
                let idx = ty * self.width_tiles + tx;
                let black = self.tiles[idx] != 0;
                // Copy-based transitions (Shrink/Split) redirect this tile to
                // a shifted source tile; wipe transitions use the identity.
                let (stx, sty) = self.src[idx];
                if black {
                    dest_fb.tile_black(tx, ty);
                } else {
                    dest_fb.tile_copy(tx, ty, source_fb, stx as usize, sty as usize);
                }
            }
        }
        self.all_black()
    }

    fn all_black(&self) -> bool {
        for &t in &self.tiles {
            if t == 0 {
                return false;
            }
        }
        true
    }

    fn fill_black(&mut self) {
        self.tiles.fill(BLACK_TILE);
    }

    // ── Spirals ─────────────────────────────────────────────────────

    fn tick_spiral_inward(&mut self) {
        // Inward spiral wipe: walks an inward spiral from the top-left
        // corner, writing $FF (black) at every tile until the screen is
        // filled. The original bursts ~7 tiles per ~3-frame delay, giving
        // roughly 2-3 tiles per 60Hz frame; we match that by stepping a
        // small batch per tick.
        // Algorithm: start at (0,0) heading down, turn counter-clockwise
        // (down → right → up → left → down …) whenever the next cell is
        // off-screen or already black. This produces a true inward spiral
        // wipe instead of the previous "left column then snap-to-black" jump.
        const TILES_PER_TICK: u8 = 3;
        // Counter-clockwise rotation order starting with "down".
        const DIRECTIONS: [(i16, i16); 4] = [
            (0, 1),  // 0: down
            (1, 0),  // 1: right
            (0, -1), // 2: up
            (-1, 0), // 3: left
        ];

        let w = self.width_tiles as i16;
        let h = self.height_tiles as i16;

        // First entry: start at (0,0) heading down.
        if self.frame == 0 {
            self.x = 0;
            self.y = 0;
            self.dir = 0;
            self.frame = 1;
        }

        for _ in 0..TILES_PER_TICK {
            // Plot current tile.
            if self.x >= 0 && self.y >= 0 {
                self.set_tile(self.x as usize, self.y as usize);
            }

            // Try to advance in the current direction; turn until we find a
            // free cell. If we can't find one after a full rotation the
            // spiral has filled the screen.
            let mut moved = false;
            for _ in 0..4 {
                let (dx, dy) = DIRECTIONS[self.dir as usize];
                let nx = self.x + dx;
                let ny = self.y + dy;
                let in_bounds = nx >= 0 && nx < w && ny >= 0 && ny < h;
                if in_bounds && self.tiles[ny as usize * self.width_tiles + nx as usize] == 0 {
                    self.x = nx;
                    self.y = ny;
                    moved = true;
                    break;
                }
                self.dir = (self.dir + 1) % 4;
            }

            if !moved {
                self.fill_black();
                self.done = true;
                return;
            }
        }
    }

    fn tick_spiral_outward(&mut self) {
        // Outward spiral wipe: starts from center (10,10), fills 3 tiles per
        // inner loop, 120 outer loops. Direction rotates when hitting a
        // filled tile: up → left → down → right
        const DIRECTIONS: [(i16, i16); 4] = [(0, -1), (-1, 0), (0, 1), (1, 0)];

        let w = self.width_tiles as i16;
        let h = self.height_tiles as i16;

        if self.spiral_x < 0 || self.spiral_x >= w || self.spiral_y < 0 || self.spiral_y >= h {
            self.fill_black();
            self.done = true;
            return;
        }

        // Write 3 tiles per frame (the original's inner loop count is 3)
        for _ in 0..3 {
            if self.spiral_x >= 0 && self.spiral_x < w && self.spiral_y >= 0 && self.spiral_y < h {
                self.set_tile(self.spiral_x as usize, self.spiral_y as usize);
            }

            let (dx, dy) = DIRECTIONS[self.spiral_dir as usize];
            let nx = self.spiral_x + dx;
            let ny = self.spiral_y + dy;

            // Check if next tile in current direction is already filled
            if nx >= 0
                && nx < w
                && ny >= 0
                && ny < h
                && self.tiles[ny as usize * self.width_tiles + nx as usize] == 0
            {
                self.spiral_x = nx;
                self.spiral_y = ny;
            } else {
                // Change direction
                self.spiral_dir = (self.spiral_dir + 1) % 4;
                let (ndx, ndy) = DIRECTIONS[self.spiral_dir as usize];
                self.spiral_x += ndx;
                self.spiral_y += ndy;
            }
        }

        if self.spiral_x < 0 || self.spiral_x >= w || self.spiral_y < 0 || self.spiral_y >= h {
            self.fill_black();
            self.done = true;
        }
    }

    // ── Circle / DoubleCircle ───────────────────────────────────────

    /// Draw one `half_circle` arc entry (the classic circle-arc renderer).
    /// `quadrant_y` selects the row-step direction (0 = down for HalfCircle1,
    /// 1 = up for HalfCircle2).
    fn draw_arc(&mut self, entry: &HalfCircleEntry, quadrant_y: u8) {
        let fill_dir: i16 = if entry.quadrant_x == CIRCLE_RIGHT {
            1
        } else {
            -1
        };
        let row_dir: i16 = if quadrant_y == 0 { 1 } else { -1 };
        let mut x = entry.x;
        let mut y = entry.y;
        let mut i = 0;
        loop {
            // Fill `fill` tiles toward the screen edge. The row start is
            // preserved (the original saves and restores it) — the fill
            // does NOT move the persistent cursor.
            let fill = entry.data[i];
            i += 1;
            let row_start_x = x;
            for _ in 0..fill {
                if x >= 0 && y >= 0 {
                    self.set_tile(x as usize, y as usize);
                }
                x += fill_dir;
            }
            x = row_start_x;
            // Step one row toward the arc interior.
            y += row_dir;
            let skip = entry.data[i];
            i += 1;
            if skip == 0xFF {
                break; // -1 terminator
            }
            // Move the next row's start back toward the arc center.
            for _ in 0..skip {
                x -= fill_dir;
            }
        }
    }

    fn tick_circle(&mut self) {
        // Circle wipe — plays the top half-circle fully, then the bottom
        // half-circle. One arc entry per step, with a ~3-frame delay
        // between entries.
        if self.counter > 0 {
            self.counter -= 1;
            return;
        }
        let entry = if self.hc_half == 0 {
            &HALF_CIRCLE_1[self.hc_step]
        } else {
            &HALF_CIRCLE_2[self.hc_step]
        };
        self.draw_arc(entry, self.hc_half);
        self.hc_step += 1;
        if self.hc_step >= 10 {
            if self.hc_half == 0 {
                self.hc_half = 1;
                self.hc_step = 0;
                self.counter = 2; // ~3-frame delay
            } else {
                self.fill_black();
                self.done = true;
            }
        } else {
            self.counter = 2; // ~3-frame delay
        }
    }

    fn tick_double_circle(&mut self) {
        // Double circle wipe — animates BOTH half circles at the same time:
        // one entry from each per ~3-frame delay.
        if self.counter > 0 {
            self.counter -= 1;
            return;
        }
        self.draw_arc(&HALF_CIRCLE_1[self.hc_step], 0);
        self.draw_arc(&HALF_CIRCLE_2[self.hc_step], 1);
        self.hc_step += 1;
        if self.hc_step >= 10 {
            self.fill_black();
            self.done = true;
        } else {
            self.counter = 2; // ~3-frame delay
        }
    }

    // ── Horizontal Stripes ──────────────────────────────────────────

    fn tick_horizontal_stripes(&mut self) {
        // Horizontal-stripes wipe.
        // Works on columns. Left side starts at (0,0), right at (19,1).
        // Each tick: fill every SCREEN_HEIGHT/2 row in current column, move inward.
        // ~3 frames between every column transfer (matching the original).
        if self.counter > 0 {
            self.counter -= 1;
            return;
        }
        let col = self.frame as usize;
        if col >= self.width_tiles {
            self.fill_black();
            self.done = true;
            return;
        }

        // Left column: every 2nd row
        for row in (0..self.height_tiles).step_by(2) {
            self.set_tile(col, row);
        }
        // Right column: every 2nd row (offset by 1)
        let rcol = self.width_tiles - 1 - col;
        for row in (1..self.height_tiles).step_by(2) {
            self.set_tile(rcol, row);
        }

        self.frame += 1;
        self.counter = 3; // ~3-frame delay
    }

    // ── Vertical Stripes ────────────────────────────────────────────

    fn tick_vertical_stripes(&mut self) {
        // Vertical-stripes wipe.
        // Works on rows. Top starts at (0,0), bottom at (1,17).
        // Each tick: fill every SCREEN_WIDTH/2 col in current row, move inward.
        if self.counter > 0 {
            self.counter -= 1;
            return;
        }
        let row = self.frame as usize;
        if row >= self.height_tiles {
            self.fill_black();
            self.done = true;
            return;
        }

        // Top row: every 2nd column
        for col in (0..self.width_tiles).step_by(2) {
            self.set_tile(col, row);
        }
        // Bottom row: every 2nd column
        let brow = self.height_tiles - 1 - row;
        for col in (0..self.width_tiles).step_by(2) {
            self.set_tile(col, brow);
        }

        self.frame += 1;
        self.counter = 3;
    }

    // ── Shrink ──────────────────────────────────────────────────────

    /// Copy one tile's (black-flag, source-tile) pair — the equivalent of
    /// the original tilemap byte copy, which also carries already-black tiles.
    fn copy_tile(&mut self, sx: usize, sy: usize, dx: usize, dy: usize) {
        let s = sy * self.width_tiles + sx;
        let d = dy * self.width_tiles + dx;
        self.tiles[d] = self.tiles[s];
        self.src[d] = self.src[s];
    }

    fn tick_shrink(&mut self) {
        // Shrink wipe: 9 steps (SCREEN_HEIGHT/2), ~6 frames between steps.
        // Each step tile-COPIES rows/columns one tile toward the screen
        // center, so the overworld visibly COMPRESSES; the freed outer
        // row/column is filled with tile $FF.
        if self.counter > 0 {
            self.counter -= 1;
            return;
        }
        let step = self.frame as usize;
        let w = self.width_tiles;
        let h = self.height_tiles;
        if step >= h / 2 {
            self.fill_black();
            self.done = true;
            return;
        }

        let rmid = h / 2; // 9
        let cmid = w / 2; // 10

        // Vertical pass: top half rows shift DOWN one row
        // (rows 0..7 copied into 1..8), bottom half rows shift UP one row
        // (rows 10..17 copied into 9..16).
        for y in (1..rmid).rev() {
            for x in 0..w {
                self.copy_tile(x, y - 1, x, y);
            }
        }
        for y in rmid..(h - 1) {
            for x in 0..w {
                self.copy_tile(x, y + 1, x, y);
            }
        }
        // Freed outer rows are blacked.
        for x in 0..w {
            self.set_tile(x, 0);
            self.set_tile(x, h - 1);
        }

        // Horizontal pass: left half cols shift RIGHT one col
        // (cols 0..8 copied into 1..9), right half cols shift LEFT one col
        // (cols 11..19 copied into 10..18).
        for x in (1..cmid).rev() {
            for y in 0..h {
                self.copy_tile(x - 1, y, x, y);
            }
        }
        for x in cmid..(w - 1) {
            for y in 0..h {
                self.copy_tile(x + 1, y, x, y);
            }
        }
        // Freed outer columns are blacked.
        for y in 0..h {
            self.set_tile(0, y);
            self.set_tile(w - 1, y);
        }

        self.frame += 1;
        self.counter = 5; // ~6 frames between steps
    }

    // ── Split ───────────────────────────────────────────────────────

    fn tick_split(&mut self) {
        // Split wipe: 9 steps, ~6 frames between steps. Each step
        // tile-COPIES rows/columns one tile AWAY from the center, so the
        // overworld visibly SPLITS apart; the freed center row/column is
        // filled with tile $FF.
        if self.counter > 0 {
            self.counter -= 1;
            return;
        }
        let step = self.frame as usize;
        let w = self.width_tiles;
        let h = self.height_tiles;
        if step >= h / 2 {
            self.fill_black();
            self.done = true;
            return;
        }

        let rmid = h / 2; // 9
        let cmid = w / 2; // 10

        // Vertical pass: bottom half rows shift DOWN one row
        // (rows 9..16 copied into 10..17), top half rows shift UP one row
        // (rows 1..8 copied into 0..7).
        for y in ((rmid + 1)..h).rev() {
            for x in 0..w {
                self.copy_tile(x, y - 1, x, y);
            }
        }
        for y in 0..(rmid - 1) {
            for x in 0..w {
                self.copy_tile(x, y + 1, x, y);
            }
        }
        // Freed center rows are blacked.
        for x in 0..w {
            self.set_tile(x, rmid - 1);
            self.set_tile(x, rmid);
        }

        // Horizontal pass: right half cols shift RIGHT one col
        // (cols 10..18 copied into 11..19), left half cols shift LEFT one
        // col (cols 1..9 copied into 0..8).
        for x in ((cmid + 1)..w).rev() {
            for y in 0..h {
                self.copy_tile(x - 1, y, x, y);
            }
        }
        for x in 0..(cmid - 1) {
            for y in 0..h {
                self.copy_tile(x + 1, y, x, y);
            }
        }
        // Freed center columns are blacked.
        for y in 0..h {
            self.set_tile(cmid - 1, y);
            self.set_tile(cmid, y);
        }

        self.frame += 1;
        self.counter = 5; // 6 frames between steps (3 + 3)
    }

    // ── Helpers ─────────────────────────────────────────────────────

    fn set_tile(&mut self, x: usize, y: usize) {
        if x < self.width_tiles && y < self.height_tiles {
            self.tiles[y * self.width_tiles + x] = BLACK_TILE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rgba;
    use dotzuki_engine::render_config::RenderConfig;

    fn tile_is_black(state: &BattleTransitionState, x: usize, y: usize) -> bool {
        state.tiles[y * state.width_tiles + x] != 0
    }

    fn src_of(state: &BattleTransitionState, x: usize, y: usize) -> (u8, u8) {
        state.src[y * state.width_tiles + x]
    }

    #[test]
    fn test_battle_transition_state_new() {
        let state = BattleTransitionState::new(BattleTransitionKind::Circle, 20, 18);
        assert_eq!(state.tiles.len(), 360); // width_tiles * height_tiles
        assert_eq!(state.width_tiles, 20);
        assert_eq!(state.height_tiles, 18);
        assert!(!state.is_done());
        // Source map starts as the identity.
        assert_eq!(src_of(&state, 5, 7), (5, 7));
    }

    #[test]
    fn test_battle_transition_state_set_tile() {
        let mut state = BattleTransitionState::new(BattleTransitionKind::Circle, 20, 18);
        state.set_tile(0, 0);
        assert_eq!(state.tiles[0], 1); // BLACK_TILE at index 0
    }

    // ── Circle arc rendering ──────────────────────────────────────

    #[test]
    fn test_circle_first_arc_entry() {
        // First HalfCircle1 entry: (CIRCLE_RIGHT, CircleData1, 18, 6),
        // quadrant_y=0 (rows step down). CircleData1 = fill 2 / skip 3 /
        // fill 5 / skip 4 / fill 9 / end.
        let mut state = BattleTransitionState::new(BattleTransitionKind::Circle, 20, 18);
        state.tick();
        // Row 6: fill 2 from (18,6) rightward → (18,6), (19,6)
        assert!(tile_is_black(&state, 18, 6));
        assert!(tile_is_black(&state, 19, 6));
        assert!(!tile_is_black(&state, 17, 6));
        // Row 7: row start 18, skip 3 → fill 5 from (15,7) → 15..=19
        assert!(tile_is_black(&state, 15, 7));
        assert!(tile_is_black(&state, 19, 7));
        assert!(!tile_is_black(&state, 14, 7));
        // Row 8: row start 15, skip 4 → fill 9 from (11,8) → 11..=19
        assert!(tile_is_black(&state, 11, 8));
        assert!(tile_is_black(&state, 19, 8));
        assert!(!tile_is_black(&state, 10, 8));
        // Nothing else yet.
        assert!(!tile_is_black(&state, 0, 0));
        assert!(!tile_is_black(&state, 10, 10));
        assert!(!state.is_done());
    }

    #[test]
    fn test_double_circle_first_tick_draws_both_halves() {
        // DoubleCircle draws one entry from EACH half per step.
        let mut state = BattleTransitionState::new(BattleTransitionKind::DoubleCircle, 20, 18);
        state.tick();
        // Top-half entry: (CIRCLE_RIGHT, CircleData1, 18, 6) — same as Circle.
        assert!(tile_is_black(&state, 18, 6));
        assert!(tile_is_black(&state, 15, 7));
        assert!(tile_is_black(&state, 11, 8));
        // Bottom-half entry: (CIRCLE_LEFT, CircleData1, 1, 11), rows step UP.
        // Row 11: fill 2 leftward from (1,11) → (1,11), (0,11)
        assert!(tile_is_black(&state, 1, 11));
        assert!(tile_is_black(&state, 0, 11));
        assert!(!tile_is_black(&state, 2, 11));
        // Row 10: row start 1, skip 3 (toward center = +x for LEFT) → fill 5
        // leftward from (4,10) → 0..=4
        assert!(tile_is_black(&state, 4, 10));
        assert!(tile_is_black(&state, 0, 10));
        assert!(!tile_is_black(&state, 5, 10));
        // Row 9: row start 4, skip 4 → fill 9 leftward from (8,9) → 0..=8
        assert!(tile_is_black(&state, 8, 9));
        assert!(tile_is_black(&state, 0, 9));
        assert!(!tile_is_black(&state, 9, 9));
    }

    #[test]
    fn test_circle_completes_fully_black() {
        // 20 arc entries (10 per half) × 3 frames per entry.
        let mut state = BattleTransitionState::new(BattleTransitionKind::Circle, 20, 18);
        let mut ticks = 0;
        while !state.is_done() && ticks < 120 {
            state.tick();
            ticks += 1;
        }
        assert!(state.is_done());
        assert!(ticks <= 20 * 3, "took {ticks} ticks");
        assert!(state.all_black());
    }

    #[test]
    fn test_double_circle_completes_fully_black() {
        // 10 steps (both halves simultaneously) × 3 frames per step.
        let mut state = BattleTransitionState::new(BattleTransitionKind::DoubleCircle, 20, 18);
        let mut ticks = 0;
        while !state.is_done() && ticks < 60 {
            state.tick();
            ticks += 1;
        }
        assert!(state.is_done());
        assert!(ticks <= 10 * 3, "took {ticks} ticks");
        assert!(state.all_black());
    }

    // ── Shrink / Split tile copies ───────────────────────────────

    #[test]
    fn test_shrink_first_step_compresses_toward_center() {
        let mut state = BattleTransitionState::new(BattleTransitionKind::Shrink, 20, 18);
        state.tick();
        // Freed outer rows/columns are black.
        for x in 0..20 {
            assert!(tile_is_black(&state, x, 0), "row 0, col {x}");
            assert!(tile_is_black(&state, x, 17), "row 17, col {x}");
        }
        for y in 0..18 {
            assert!(tile_is_black(&state, 0, y), "col 0, row {y}");
            assert!(tile_is_black(&state, 19, y), "col 19, row {y}");
        }
        // Interior shifts one tile toward the center — vertically first,
        // then horizontally on the shifted content (original order: vertical
        // top/bottom, then horizontal left/right). So dest (5,1) shows the
        // tile that was at (4,0): up one row, then left one column.
        assert_eq!(src_of(&state, 5, 1), (4, 0));
        assert_eq!(src_of(&state, 5, 8), (4, 7));
        assert_eq!(src_of(&state, 5, 9), (4, 10));
        assert_eq!(src_of(&state, 5, 16), (4, 17));
        assert_eq!(src_of(&state, 1, 5), (0, 4));
        assert_eq!(src_of(&state, 9, 5), (8, 4));
        assert_eq!(src_of(&state, 10, 5), (11, 4));
        assert_eq!(src_of(&state, 18, 5), (19, 4));
        assert!(!state.is_done());
    }

    #[test]
    fn test_split_first_step_pushes_away_from_center() {
        let mut state = BattleTransitionState::new(BattleTransitionKind::Split, 20, 18);
        state.tick();
        // Freed CENTER rows/columns are black.
        for x in 0..20 {
            assert!(tile_is_black(&state, x, 8), "row 8, col {x}");
            assert!(tile_is_black(&state, x, 9), "row 9, col {x}");
        }
        for y in 0..18 {
            assert!(tile_is_black(&state, 9, y), "col 9, row {y}");
            assert!(tile_is_black(&state, 10, y), "col 10, row {y}");
        }
        // Interior shifts one tile AWAY from the center — vertically first,
        // then horizontally on the shifted content (same ordering as
        // Shrink). Dest (5,0) shows the tile that was at (6,1): down one
        // row, then right one column.
        assert_eq!(src_of(&state, 5, 0), (6, 1));
        assert_eq!(src_of(&state, 5, 7), (6, 8));
        assert_eq!(src_of(&state, 5, 10), (6, 9));
        assert_eq!(src_of(&state, 5, 17), (6, 16));
        assert_eq!(src_of(&state, 0, 5), (1, 6));
        assert_eq!(src_of(&state, 8, 5), (9, 6));
        assert_eq!(src_of(&state, 11, 5), (10, 6));
        assert_eq!(src_of(&state, 19, 5), (18, 6));
        assert!(!state.is_done());
    }

    #[test]
    fn test_shrink_completes_fully_black() {
        // 9 steps × 6 frames between steps.
        let mut state = BattleTransitionState::new(BattleTransitionKind::Shrink, 20, 18);
        let mut ticks = 0;
        while !state.is_done() && ticks < 120 {
            state.tick();
            ticks += 1;
        }
        assert!(state.is_done());
        assert!(ticks <= 9 * 6 + 1, "took {ticks} ticks");
        assert!(state.all_black());
    }

    #[test]
    fn test_split_completes_fully_black() {
        let mut state = BattleTransitionState::new(BattleTransitionKind::Split, 20, 18);
        let mut ticks = 0;
        while !state.is_done() && ticks < 120 {
            state.tick();
            ticks += 1;
        }
        assert!(state.is_done());
        assert!(ticks <= 9 * 6 + 1, "took {ticks} ticks");
        assert!(state.all_black());
    }

    #[test]
    fn test_shrink_render_copies_source_pixels() {
        // The render must blit the SHIFTED overworld content, not a mask.
        let mut state = BattleTransitionState::new(BattleTransitionKind::Shrink, 20, 18);
        let mut source = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::BLACK);
        // Paint every tile a distinct shade: tile (x,y) → gray value x+y*20.
        for ty in 0..18usize {
            for tx in 0..20usize {
                let v = (tx + ty * 20) as u8;
                for dy in 0..8usize {
                    for dx in 0..8usize {
                        let off = ((ty * 8 + dy) * 160 + (tx * 8 + dx)) * 4;
                        source.data[off] = v;
                        source.data[off + 1] = v;
                        source.data[off + 2] = v;
                        source.data[off + 3] = 255;
                    }
                }
            }
        }
        state.tick(); // one shrink step
        let mut dest = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::BLACK);
        state.render(&source, &mut dest);
        // Tile (5,1) now shows source tile (4,0) → value 4.
        let off = ((1 * 8 + 3) * 160 + (5 * 8 + 3)) * 4;
        assert_eq!(dest.data[off], 4);
        // Tile (0,0) is black.
        assert_eq!(dest.data[0], 0);
        // Tile (5,10) shows source tile (4,11) → value 4 + 11*20.
        let off2 = ((10 * 8 + 3) * 160 + (5 * 8 + 3)) * 4;
        assert_eq!(dest.data[off2], (4 + 11 * 20) as u8);
    }
}
