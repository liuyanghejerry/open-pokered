//! Framebuffer special effects (SE) — shared by the pokered frontends.
//!
//! Faithful ports of the classic special-effect routines (the SE id →
//! routine dispatch table). Each effect is a per-frame state machine:
//! the caller feeds `AnimEffect`s in via [`BattleEffects::apply`] (which returns
//! how many frames the original routine would have blocked the animation
//! command stream), calls [`BattleEffects::tick`] once per frame, and uses the
//! query/draw methods while rendering the battle scene.
//!
//! All effects operate on the same `FrameBuffer`/`TileSet`/`Palette`
//! abstractions the frontends already use; game assets (the substitute doll
//! sprite, the move-animation tilesets) are passed in by the caller.

use crate::indexed_framebuffer::RgbaIndexedFrameBuffer;
use crate::palette::{GbColor, Palette};
use crate::sprite::{SpriteLayer, SpriteOamEntry, OAM_X_FLIP};
use crate::tile::{TileSet, TILE_PIXELS};
use crate::{Rgba, TILE_SIZE};

use super::types::ANIM_BASE_TILE_ID;
use super::AnimEffect;

// ─── Shared constants ────────────────────────────────────────────────

/// Which mon a special effect acts on (whose turn it is).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonSide {
    Player,
    Enemy,
}

impl MonSide {
    /// The opposing side (the original flips whose-turn).
    pub fn other(self) -> Self {
        match self {
            Self::Player => Self::Enemy,
            Self::Enemy => Self::Player,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Player => 0,
            Self::Enemy => 1,
        }
    }
}

/// Pixel rectangle of a mon pic on screen (both pics are 7×7 tiles = 56×56).
#[derive(Debug, Clone, Copy)]
pub struct MonRect {
    pub x: i32,
    pub y: i32,
}

/// Per-scanline SCX values for the wavy-screen effect, looping; the start
/// advances one entry per frame for 255 frames.
pub const WAVY_LINE_OFFSETS: [i8; 32] = [
    0, 0, 0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 2, 1, 1, 1, //
    0, 0, 0, 0, 0, -1, -1, -1, -2, -2, -2, -2, -2, -1, -1, -1,
];

/// Duration of the wavy-screen effect in frames.
pub const WAVY_SCREEN_FRAMES: u16 = 255;

/// (Y, X) OAM pairs the three spiralling balls of the spiral-balls-inward
/// effect step through, one pair per 5 frames. Terminated by $FF in the
/// original; the 21 pairs are stored here.
pub const SPIRAL_BALL_COORDS: [(u8, u8); 21] = [
    (0x38, 0x28),
    (0x40, 0x18),
    (0x50, 0x10),
    (0x60, 0x18),
    (0x68, 0x28),
    (0x60, 0x38),
    (0x50, 0x40),
    (0x40, 0x38),
    (0x40, 0x28),
    (0x46, 0x1E),
    (0x50, 0x18),
    (0x5B, 0x1E),
    (0x60, 0x28),
    (0x5B, 0x32),
    (0x50, 0x38),
    (0x46, 0x32),
    (0x48, 0x28),
    (0x50, 0x20),
    (0x58, 0x28),
    (0x50, 0x30),
    (0x50, 0x28),
];

/// X coordinates of the upward-shooting balls on the player's turn.
pub const UPWARD_BALLS_X_PLAYER: [u8; 6] = [0x10, 0x40, 0x28, 0x18, 0x38, 0x30];
/// X coordinates of the upward-shooting balls on the enemy's turn.
pub const UPWARD_BALLS_X_ENEMY: [u8; 6] = [0x60, 0x90, 0x78, 0x68, 0x88, 0x80];

/// Initial OAM X values of the falling objects (petals/leaves).
pub const FALLING_INITIAL_X: [u8; 20] = [
    0x38, 0x40, 0x50, 0x60, 0x70, 0x88, 0x90, 0x56, 0x67, 0x4A, //
    0x77, 0x84, 0x98, 0x32, 0x22, 0x5C, 0x6C, 0x7D, 0x8E, 0x99,
];

/// Initial movement data of the falling objects.
pub const FALLING_INITIAL_MOVEMENT: [u8; 20] = [
    0x00, 0x84, 0x06, 0x81, 0x02, 0x88, 0x01, 0x83, 0x05, 0x89, //
    0x09, 0x80, 0x07, 0x87, 0x03, 0x82, 0x04, 0x85, 0x08, 0x86,
];

/// Per-step X deltas of the falling objects. The original table has only 9
/// entries; two of the initial movement bytes ($09/$89 → index 10) read past
/// it into the following code bytes in the original (an original-game bug —
/// those objects dart sideways with a garbage delta). We approximate the
/// garbage with 0xFA, the first byte after the table.
pub const FALLING_DELTA_XS: [u8; 9] = [0, 1, 3, 5, 7, 9, 11, 13, 15];
const FALLING_DELTA_XS_OVERFLOW: u8 = 0xFA;

/// Ball tile used by the spiral/shoot-balls effects, move anim tileset 0.
/// Stored as the absolute VRAM tile id (tileset base $31).
pub const BALL_TILE: u8 = 0x7A;
/// Water droplet tile (tile id $71), tileset 0.
pub const DROPLET_TILE: u8 = 0x71;
/// Petal tile (tile id $71), tileset 1.
pub const PETAL_TILE: u8 = 0x71;
/// Leaf tile (tile id $37), tileset 1.
pub const LEAF_TILE: u8 = 0x37;

/// Long-flash BG palette sequence (12 steps), cycled 3 times: first cycle
/// 2 frames/step, then 1 frame/step → 12*2 + 12 + 12 = 48 frames.
pub const FLASH_SCREEN_LONG_PALETTE: [[u8; 4]; 12] = [
    [3, 3, 2, 1],
    [3, 3, 3, 2],
    [3, 3, 3, 3],
    [3, 3, 3, 2],
    [3, 3, 2, 1],
    [3, 2, 1, 0],
    [2, 1, 0, 0],
    [1, 0, 0, 0],
    [0, 0, 0, 0],
    [1, 0, 0, 0],
    [2, 1, 0, 0],
    [3, 2, 1, 0],
];

/// Minimized-mon sprite: 8×5 px blob that replaces the mon pic for the
/// minimize effect. Each byte is written twice (both bitplanes) → color
/// index 3 (black).
pub const MINIMIZED_BLOB: [[u8; 8]; 5] = [
    [0, 0, 0, 1, 1, 0, 0, 0],
    [0, 0, 1, 1, 1, 1, 0, 0],
    [0, 1, 1, 1, 1, 1, 1, 0],
    [0, 0, 1, 1, 1, 1, 0, 0],
    [0, 0, 1, 0, 0, 1, 0, 0],
];

// Effect durations in frames (every ~3-frame delay in the original routines
// is counted, since the original blocks the animation command stream while
// the effect runs).
const FLASH_SHORT_FRAMES: u8 = 4; // short flash: 2 + 2
const FLASH_LONG_FRAMES: u8 = 48; // long flash
const BLINK_FRAMES: u8 = 60; // blink: 6 × (5 hidden + 5 shown)
const SQUISH_FRAMES: u8 = 26; // squish: 8 passes × 3 + 2
const SHAKE_BACK_AND_FORTH_FRAMES: u8 = 96; // back-and-forth shake: $10 × 2 × 3
const SPIRAL_BALLS_FRAMES: u8 = 105; // 21 coordinate pairs × 5
const FALLING_FRAMES: u16 = 156; // falling objects: 52 ticks × 3
const DROPLET_FRAMES: u8 = 64; // droplets: 32 × 2
const HUD_SHAKE_FRAMES: u8 = 32; // ShakeEnemyHUD_ShakeBG: 8 × (2 + 2)
const MINIMIZE_FRAMES: u8 = 6; // minimize: 3 + re-show's 3
const SUBSTITUTE_FRAMES: u8 = 3; // substitute: re-show's 3
/// Bounce: 5 × slide-down runs (7 rows × 3) + the re-show's 3.
const BOUNCE_FRAMES: u8 = 5 * 7 * 3 + 3;
/// Slide-down-and-hide: 2 steps (7×5, 7×3) × 8 frames.
const SLIDE_DOWN_HIDE_FRAMES: u8 = 16;
/// Transform: the original synchronously swaps the pic (the preceding poof
/// subanimation covers the swap) and runs a palette command — it blocks the
/// command stream for the SE call itself. We model that block: the pic
/// stays hidden for a beat (under the poof) and reappears as the opposing
/// species (the frontend draws the target sprite once the core TRANSFORMED
/// flag is set).
const TRANSFORM_FRAMES: u8 = 12;
/// Frames the pic stays hidden at the start of the transform block.
const TRANSFORM_HIDDEN_FRAMES: u8 = 6;

/// Height of the top screen region shaken by the enemy-HUD shake
/// (the window is parked at WY = 7*8, so rows 0..7 scroll with SCX).
const HUD_SHAKE_HEIGHT: u32 = 7 * TILE_SIZE;
/// Height of the region moved by the classic screen shakes
/// (everything above the bottom text box).
const SCREEN_SHAKE_HEIGHT: u32 = 12 * TILE_SIZE;

// ─── Shade remap helpers ─────────────────────────────────────────────

/// Apply a DMG palette map (shade → shade) to the whole framebuffer,
/// e.g. rBGP = $6f for the dark-screen palette. On the indexed
/// framebuffer this is a display-palette remap — the GB-hardware way.
fn remap_shades(fb: &mut RgbaIndexedFrameBuffer, map: &[u8; 4]) {
    fb.remap_shades(map);
}

/// Shift a horizontal strip of the framebuffer sideways, filling the exposed
/// edge with white. Used for the SCX-based shakes. Operates on the packed
/// 2bpp indices (cloned cheaply — 5.7 KiB), so the result is identical to
/// the old per-pixel RGBA shift.
fn shift_rows_h(fb: &mut RgbaIndexedFrameBuffer, y_end: u32, dx: i32) {
    if dx == 0 {
        return;
    }
    let w = fb.width() as i32;
    let src = fb.indexed().clone();
    for y in 0..y_end.min(fb.height()) {
        for x in 0..w {
            let sx = x - dx;
            let color = if sx >= 0 && sx < w {
                src.get_pixel(sx as u32, y).unwrap_or(GbColor::White)
            } else {
                GbColor::White
            };
            fb.set_pixel_index(x as u32, y, color);
        }
    }
}

/// Shift the top strip of the framebuffer vertically, filling with white.
fn shift_rows_v(fb: &mut RgbaIndexedFrameBuffer, y_end: u32, dy: i32) {
    if dy == 0 {
        return;
    }
    let src = fb.indexed().clone();
    for y in 0..y_end.min(fb.height()) as i32 {
        let sy = y - dy;
        for x in 0..fb.width() as i32 {
            let color = if sy >= 0 && (sy as u32) < y_end.min(fb.height()) {
                src.get_pixel(x as u32, sy as u32).unwrap_or(GbColor::White)
            } else {
                GbColor::White
            };
            fb.set_pixel_index(x as u32, y as u32, color);
        }
    }
}

// ─── Individual effect state machines ────────────────────────────────

/// Short flash (invert 2 frames + white 2 frames) and long flash
/// (12-step palette cycle ×3, 48 frames).
#[derive(Debug, Clone, Copy)]
enum Flash {
    Short { frame: u8 },
    Long { frame: u8 },
}

/// Screen shake. Two flavors, matching the original routines:
/// - `decay`: per amplitude step, 4 frames displaced + 5 frames at rest,
///   amplitude decaying pixels → 1 (total pixels*9 frames).
/// - `alternate`: the small ±1 px shakes used by the applying-attack
///   feedback.
#[derive(Debug, Clone, Copy)]
struct Shake {
    horizontal: bool,
    vertical: bool,
    pixels: i32,
    decay: bool,
    frame: u16,
    total: u16,
}

impl Shake {
    fn offset(&self) -> (i32, i32) {
        let amp = if self.decay {
            // 9 frames per amplitude step: displaced for 4, at rest for 5.
            let step = self.frame / 9;
            let within = self.frame % 9;
            let amp = (self.pixels - step as i32).max(1);
            if within < 4 {
                amp
            } else {
                0
            }
        } else {
            // ±pixels every frame.
            if self.frame % 2 == 0 {
                self.pixels
            } else {
                -self.pixels
            }
        };
        (
            if self.horizontal { amp } else { 0 },
            if self.vertical { amp } else { 0 },
        )
    }
}

/// Persistent BG palette remaps (dark screen rBGP=$6f, light screen
/// rBGP=$90, darken mon rBGP=$f9). These stay in effect until the palette
/// is reset.
#[derive(Debug, Clone, Copy)]
enum Tint {
    Dark,
    Light,
    DarkenMon,
}

impl Tint {
    fn map(&self) -> [u8; 4] {
        match self {
            // $6f = %01_10_11_11
            Tint::Dark => [3, 3, 2, 1],
            // $90 = %10_01_00_00
            Tint::Light => [0, 0, 1, 2],
            // $f9 = %11_11_10_01
            Tint::DarkenMon => [1, 2, 3, 3],
        }
    }
}

/// Blink: 6 cycles of (hide 5 frames, show 5 frames).
#[derive(Debug, Clone, Copy)]
struct Blink {
    side: MonSide,
    frame: u8,
}

/// Squish: 4 iterations × (left pass + right pass); each pass narrows the
/// pic by one tile (alternating anchor) and waits ~3 frames, then the pic
/// is hidden. 8 passes × 3 frames + 2 = 26 frames.
#[derive(Debug, Clone, Copy)]
struct Squish {
    side: MonSide,
    frame: u8,
}

impl Squish {
    /// (width in tiles 0..=7, anchor_right) for the current frame.
    fn params(&self) -> (u32, bool) {
        let pass = (self.frame / 3).min(7);
        let width = 7u32.saturating_sub(pass as u32 + 1);
        (width, pass % 2 == 1)
    }
}

/// Transform: the mon's pic is hidden while the SE blocks the command
/// stream, then reappears (already drawn as the opposing species by the
/// frontend, which keys the sprite off the core TRANSFORMED flag).
#[derive(Debug, Clone, Copy)]
struct Transform {
    side: MonSide,
    frame: u8,
}

/// Back-and-forth shake (Double Team): the mon's pic jumps ±1 tile
/// horizontally every ~3 frames, $10 iterations, then stays hidden.
#[derive(Debug, Clone, Copy)]
struct ShakeBackAndForth {
    side: MonSide,
    frame: u8,
}

/// Enemy-HUD shake: SCX shake of the top 7 tile rows, ±2 px every 2
/// frames, 8 iterations.
#[derive(Debug, Clone, Copy)]
struct HudShake {
    frame: u8,
}

/// Bounce (Splash): five runs of a slide-down (the pic slides down one row
/// per ~3 frames, 7 rows = 21 frames) followed by a re-show — the mon sinks
/// out of its box and instantly pops back to the top, 5 times.
#[derive(Debug, Clone, Copy)]
struct Bounce {
    side: MonSide,
    frame: u8,
}

/// Slide-down-and-hide (Acid Armor): redraw the pic with the 7×5 then 7×3
/// tile-id lists (top 5 / top 3 rows, drawn 2 / 4 rows lower), 8 frames
/// each, then hide the pic and blank its tile data.
#[derive(Debug, Clone, Copy)]
struct SlideDownHide {
    side: MonSide,
    frame: u8,
}

/// Object (sprite) effects drawn with the move-animation tilesets.
#[derive(Debug, Clone)]
enum Objects {
    None,
    /// Spiral-balls-inward: 3 balls step through SPIRAL_BALL_COORDS,
    /// 5 frames per step; ends with a short flash.
    SpiralBalls {
        side: MonSide,
        frame: u8,
    },
    /// Shoot-balls-upward: pillars of balls rising 4 px/frame, removed at
    /// the pillar top.
    ShootBalls {
        pillars: Vec<Pillar>,
        active: usize,
        frame: u8,
    },
    /// Falling objects (petals / leaves).
    Falling {
        tile: u8,
        frame: u16,
        tick: u8,
        objects: Vec<FallingObject>,
    },
    /// Droplets: a full-screen droplet grid that scrolls; 32 iterations ×
    /// 2 frames.
    Droplets {
        iter: u8,
        half: u8,
        base_x: i32,
    },
}

#[derive(Debug, Clone)]
struct Pillar {
    base_y: i32, // OAM Y of the pillar top
    base_x: i32, // OAM X
    /// OAM Y of each ball (removed when it wraps to base_y + 8).
    balls: Vec<i32>,
}

#[derive(Debug, Clone, Copy)]
struct FallingObject {
    x: u8, // OAM X
    y: u8, // OAM Y
    movement: u8,
}

// ─── BattleEffects ───────────────────────────────────────────────────

/// Shared framebuffer special-effect state, driven by `AnimEffect`s from the
/// animation player. Both pokered frontends own one of these instead of
/// their own duplicated effect code.
#[derive(Debug, Clone)]
pub struct BattleEffects {
    flash: Option<Flash>,
    shake: Option<Shake>,
    tint: Option<Tint>,
    wave_frame: u16,
    hud_shake: Option<HudShake>,
    blink: Option<Blink>,
    squish: Option<Squish>,
    shake_bnf: Option<ShakeBackAndForth>,
    bounce: Option<Bounce>,
    slide_down_hide: Option<SlideDownHide>,
    /// Transform: pic hidden for a beat while the opposing species' sprite
    /// takes its place (see TRANSFORM_FRAMES).
    transform: Option<Transform>,
    /// Pic hidden by the squish / shake effects until a Show effect (the
    /// original hides it at the end of both routines).
    forced_hidden: [bool; 2],
    /// Minimize: pic replaced by the mini blob. Persists until the mon
    /// leaves (the original keeps per-side minimized flags).
    minimized: [bool; 2],
    /// Substitute: pic replaced by the mini doll sprite. Frontends normally
    /// drive this from the core HAS_SUBSTITUTE_UP flag via
    /// [`BattleEffects::set_substitute`]; the SE itself latches it too.
    substitute: [bool; 2],
    objects: Objects,
}

impl Default for BattleEffects {
    fn default() -> Self {
        Self::new()
    }
}

impl BattleEffects {
    pub fn new() -> Self {
        Self {
            flash: None,
            shake: None,
            tint: None,
            wave_frame: 0,
            hud_shake: None,
            blink: None,
            squish: None,
            shake_bnf: None,
            bounce: None,
            slide_down_hide: None,
            transform: None,
            forced_hidden: [false; 2],
            minimized: [false; 2],
            substitute: [false; 2],
            objects: Objects::None,
        }
    }

    /// Clear per-mon latches when a mon leaves the field (switch/faint).
    pub fn clear_side(&mut self, side: MonSide) {
        let i = side.index();
        self.forced_hidden[i] = false;
        self.minimized[i] = false;
        self.substitute[i] = false;
        if self.transform.is_some_and(|t| t.side == side) {
            self.transform = None;
        }
    }

    /// Sync the substitute doll latch from the core HAS_SUBSTITUTE_UP flag.
    pub fn set_substitute(&mut self, side: MonSide, up: bool) {
        self.substitute[side.index()] = up;
    }

    /// Show the mon pic again (player or enemy side). Clears the
    /// forced-hidden latch left by the squish / shake effects.
    pub fn show_mon(&mut self, side: MonSide) {
        self.forced_hidden[side.index()] = false;
    }

    /// Apply an `AnimEffect`. `attacker` is whose turn it is; the
    /// "PlayerMon"/"EnemyMonPic" effect names follow the original convention
    /// of whose-turn vs flipped-turn mon. Returns the number of frames the
    /// original routine blocks the animation command stream — the caller
    /// should hold the animation player for that long.
    pub fn apply(&mut self, effect: &AnimEffect, attacker: MonSide) -> u8 {
        match *effect {
            AnimEffect::None => 0,
            AnimEffect::FlashScreen { frames } => {
                if frames as usize >= FLASH_LONG_FRAMES as usize {
                    self.flash = Some(Flash::Long { frame: 0 });
                    FLASH_LONG_FRAMES
                } else {
                    self.flash = Some(Flash::Short { frame: 0 });
                    FLASH_SHORT_FRAMES
                }
            }
            AnimEffect::ShakeScreenH { pixels, frames } => {
                self.start_shake(true, false, pixels, frames);
                self.shake_duration()
            }
            AnimEffect::ShakeScreenV { pixels, frames } => {
                self.start_shake(false, true, pixels, frames);
                self.shake_duration()
            }
            AnimEffect::ShakeScreenHV { pixels, frames } => {
                self.start_shake(true, true, pixels, frames);
                self.shake_duration()
            }
            AnimEffect::DarkScreenPalette => {
                self.tint = Some(Tint::Dark);
                0
            }
            AnimEffect::LightScreenPalette => {
                self.tint = Some(Tint::Light);
                0
            }
            AnimEffect::DarkenMonPalette => {
                self.tint = Some(Tint::DarkenMon);
                0
            }
            AnimEffect::ResetScreenPalette => {
                self.tint = None;
                0
            }
            AnimEffect::WavyScreen => {
                self.wave_frame = self.wave_frame.max(1);
                WAVY_SCREEN_FRAMES as u8
            }
            AnimEffect::SubstituteMon => {
                // Substitute: swap the pic for the mini doll sprite, then
                // re-show the pic.
                self.substitute[attacker.index()] = true;
                self.forced_hidden[attacker.index()] = false;
                SUBSTITUTE_FRAMES
            }
            AnimEffect::MinimizeMon => {
                // Minimize: swap the pic for the 8×5 mini blob, wait ~3
                // frames, then re-show the pic.
                self.minimized[attacker.index()] = true;
                self.forced_hidden[attacker.index()] = false;
                MINIMIZE_FRAMES
            }
            AnimEffect::TransformMon => {
                // Transform: the pic is instantly reloaded as the opposing
                // mon's sprite — the morph reads as instant because the
                // preceding poof covers it. The SE call itself is
                // synchronous, so block the command stream while the swap
                // settles: hide the pic for a beat (under the poof), then
                // re-show it (the frontend draws the opposing species'
                // sprite as soon as the core TRANSFORMED flag is set).
                self.transform = Some(Transform {
                    side: attacker,
                    frame: 0,
                });
                self.forced_hidden[attacker.index()] = true;
                TRANSFORM_FRAMES
            }
            AnimEffect::SquishMonPic => {
                self.squish = Some(Squish {
                    side: attacker,
                    frame: 0,
                });
                SQUISH_FRAMES
            }
            AnimEffect::ShakeBackAndForth => {
                self.shake_bnf = Some(ShakeBackAndForth {
                    side: attacker,
                    frame: 0,
                });
                SHAKE_BACK_AND_FORTH_FRAMES
            }
            AnimEffect::BounceUpAndDown => {
                self.bounce = Some(Bounce {
                    side: attacker,
                    frame: 0,
                });
                BOUNCE_FRAMES
            }
            AnimEffect::SlidePlayerMonDownAndHide => {
                self.slide_down_hide = Some(SlideDownHide {
                    side: attacker,
                    frame: 0,
                });
                SLIDE_DOWN_HIDE_FRAMES
            }
            // The blink effect blinks whose-turn mon; the enemy variant
            // blinks the flipped-turn mon.
            AnimEffect::BlinkPlayerMon { .. } | AnimEffect::FlashPlayerMonPic => {
                self.blink = Some(Blink {
                    side: attacker,
                    frame: 0,
                });
                BLINK_FRAMES
            }
            AnimEffect::BlinkEnemyMon { .. } | AnimEffect::FlashEnemyMonPic => {
                self.blink = Some(Blink {
                    side: attacker.other(),
                    frame: 0,
                });
                BLINK_FRAMES
            }
            AnimEffect::ShowPlayerMon => {
                self.show_mon(attacker);
                0
            }
            AnimEffect::ShowEnemyMon => {
                self.show_mon(attacker.other());
                0
            }
            AnimEffect::HidePlayerMon => {
                self.forced_hidden[attacker.index()] = true;
                0
            }
            AnimEffect::HideEnemyMon => {
                self.forced_hidden[attacker.other().index()] = true;
                0
            }
            AnimEffect::ShakeEnemyHud { .. } => {
                // The enemy-HUD-shake effect ids (one unused) both map to
                // the same HUD shake.
                self.hud_shake = Some(HudShake { frame: 0 });
                HUD_SHAKE_FRAMES
            }
            AnimEffect::SpiralBallsInward => {
                self.objects = Objects::SpiralBalls {
                    side: attacker,
                    frame: 0,
                };
                SPIRAL_BALLS_FRAMES
            }
            AnimEffect::ShootBallsUpward { many } => {
                self.objects = Self::new_shoot_balls(attacker, many);
                self.shoot_balls_frames() as u8
            }
            AnimEffect::PetalsFalling => {
                self.objects = Self::new_falling(PETAL_TILE, 20);
                FALLING_FRAMES as u8
            }
            AnimEffect::LeavesFalling => {
                self.objects = Self::new_falling(LEAF_TILE, 3);
                FALLING_FRAMES as u8
            }
            AnimEffect::WaterDroplets => {
                self.objects = Objects::Droplets {
                    iter: 0,
                    half: 0,
                    base_x: -16,
                };
                DROPLET_FRAMES
            }
            // Slides, lunges, delays and visibility flows are handled by the
            // frontends (they own the mon slide/lunge state machines).
            _ => 0,
        }
    }

    fn start_shake(&mut self, h: bool, v: bool, pixels: i8, frames: u8) {
        let pixels = pixels.unsigned_abs() as i32;
        // A pixel-count > 1 decays the amplitude step by step; the ±1 px
        // feedback shakes simply alternate.
        let decay = pixels > 1;
        let total = if decay {
            (pixels as u16) * 9
        } else {
            frames.max(1) as u16
        };
        self.shake = Some(Shake {
            horizontal: h,
            vertical: v,
            pixels,
            decay,
            frame: 0,
            total,
        });
    }

    fn shake_duration(&self) -> u8 {
        self.shake.map_or(0, |s| s.total.min(255) as u8)
    }

    fn new_shoot_balls(side: MonSide, many: bool) -> Objects {
        // Shoot-balls-upward: balls start at baseY + 8*i (each OAM entry
        // adds 8 *before* the write).
        let make_pillar = |base_y: i32, base_x: i32, count: usize| Pillar {
            base_y,
            base_x,
            balls: (1..=count).map(|i| base_y + 8 * i as i32).collect(),
        };
        let pillars = if many {
            // Many-balls variant: 6 sequential pillars of 4 balls.
            let (xs, y) = match side {
                MonSide::Player => (&UPWARD_BALLS_X_PLAYER[..], 0x50),
                MonSide::Enemy => (&UPWARD_BALLS_X_ENEMY[..], 0x28),
            };
            xs.iter()
                .map(|&x| make_pillar(y as i32, x as i32, 4))
                .collect()
        } else {
            // Single-pillar variant: 5 balls.
            let (y, x) = match side {
                MonSide::Player => (6 * 8, 5 * 8),
                MonSide::Enemy => (0, 16 * 8),
            };
            vec![make_pillar(y, x, 5)]
        };
        Objects::ShootBalls {
            pillars,
            active: 0,
            frame: 0,
        }
    }

    fn shoot_balls_frames(&self) -> u16 {
        match &self.objects {
            // Balls rise 4 px/frame and pop at base_y + 8; the lowest ball
            // (base_y + 8*n) takes 2*(n-1) frames, +1 for the removal check.
            Objects::ShootBalls { pillars, .. } => pillars
                .iter()
                .map(|p| 2 * (p.balls.len() as u16).saturating_sub(1))
                .sum(),
            _ => 0,
        }
    }

    fn new_falling(tile: u8, count: usize) -> Objects {
        let objects = (0..count)
            .map(|i| FallingObject {
                // Object init: Y = 8*(i+1); the first object's Y is then
                // set to 0.
                y: if i == 0 { 0 } else { 8 * (i as u8 + 1) },
                x: FALLING_INITIAL_X[i],
                movement: FALLING_INITIAL_MOVEMENT[i],
            })
            .collect();
        Objects::Falling {
            tile,
            frame: 0,
            tick: 0,
            objects,
        }
    }

    /// Advance all effect state machines by one frame.
    pub fn tick(&mut self) {
        if let Some(flash) = &mut self.flash {
            let done = match flash {
                Flash::Short { frame } => {
                    *frame += 1;
                    *frame >= FLASH_SHORT_FRAMES
                }
                Flash::Long { frame } => {
                    *frame += 1;
                    *frame >= FLASH_LONG_FRAMES
                }
            };
            if done {
                self.flash = None;
            }
        }

        if let Some(shake) = &mut self.shake {
            shake.frame += 1;
            if shake.frame >= shake.total {
                self.shake = None;
            }
        }

        if self.wave_frame > 0 {
            self.wave_frame += 1;
            if self.wave_frame > WAVY_SCREEN_FRAMES {
                self.wave_frame = 0;
            }
        }

        if let Some(hud) = &mut self.hud_shake {
            hud.frame += 1;
            if hud.frame >= HUD_SHAKE_FRAMES {
                self.hud_shake = None;
            }
        }

        if let Some(blink) = &mut self.blink {
            blink.frame += 1;
            if blink.frame >= BLINK_FRAMES {
                self.blink = None;
            }
        }

        if let Some(squish) = &mut self.squish {
            squish.frame += 1;
            if squish.frame >= SQUISH_FRAMES {
                // The squish ends by hiding the pic.
                self.forced_hidden[squish.side.index()] = true;
                self.squish = None;
            }
        }

        if let Some(bnf) = &mut self.shake_bnf {
            bnf.frame += 1;
            if bnf.frame >= SHAKE_BACK_AND_FORTH_FRAMES {
                // "The mon's sprite disappears after this animation."
                self.forced_hidden[bnf.side.index()] = true;
                self.shake_bnf = None;
            }
        }

        if let Some(bounce) = &mut self.bounce {
            bounce.frame += 1;
            if bounce.frame >= BOUNCE_FRAMES {
                // Ends with a re-show — the mon stays visible.
                self.bounce = None;
            }
        }

        if let Some(sdh) = &mut self.slide_down_hide {
            sdh.frame += 1;
            if sdh.frame >= SLIDE_DOWN_HIDE_FRAMES {
                // The slide-down-and-hide ends by hiding the pic and
                // blanking its tile data.
                self.forced_hidden[sdh.side.index()] = true;
                self.slide_down_hide = None;
            }
        }

        if let Some(tf) = &mut self.transform {
            tf.frame += 1;
            if tf.frame == TRANSFORM_HIDDEN_FRAMES {
                // The swap has happened: re-show the pic (now drawn as the
                // opposing species, keyed off the core TRANSFORMED flag).
                self.forced_hidden[tf.side.index()] = false;
            }
            if tf.frame >= TRANSFORM_FRAMES {
                self.transform = None;
            }
        }

        match &mut self.objects {
            Objects::None => {}
            Objects::SpiralBalls { frame, .. } => {
                *frame += 1;
                if *frame >= SPIRAL_BALLS_FRAMES {
                    self.objects = Objects::None;
                    // The spiral-balls effect ends with a short flash.
                    self.flash = Some(Flash::Short { frame: 0 });
                }
            }
            Objects::ShootBalls {
                pillars,
                active,
                frame,
            } => {
                *frame += 1;
                let pillar = &mut pillars[*active];
                let top = pillar.base_y + 8;
                for ball in &mut pillar.balls {
                    if *ball != top {
                        // Balls rise 4 px/frame; removed once they reach the
                        // pillar top (checked before the move, as in the original).
                        *ball = ball.wrapping_sub(4);
                    }
                }
                // Drop removed balls (keep the vec compact for rendering).
                pillar.balls.retain(|b| *b != top);
                if pillar.balls.is_empty() {
                    if *active + 1 < pillars.len() {
                        *active += 1;
                        *frame = 0;
                    } else {
                        self.objects = Objects::None;
                    }
                }
            }
            Objects::Falling {
                frame,
                tick,
                objects,
                ..
            } => {
                *frame += 1;
                // Falling-object updates run once per ~3 frames.
                if *frame % 3 == 0 {
                    *tick += 1;
                    for obj in objects.iter_mut() {
                        // Update the movement byte.
                        let next = obj.movement.wrapping_add(1);
                        obj.movement = if next & 0x7f == 9 {
                            (next & 0x80) ^ 0x80
                        } else {
                            next
                        };
                        // Falling-object update: Y += 2, off-screen at
                        // >= 112; X ± DeltaXs[movement & $7f], X flip when
                        // moving left.
                        obj.y = obj.y.wrapping_add(2);
                        if obj.y >= 112 {
                            obj.y = 160; // SCREEN_HEIGHT_PX + OAM_Y_OFS
                        }
                        let idx = (obj.movement & 0x7f) as usize;
                        let delta = FALLING_DELTA_XS
                            .get(idx)
                            .copied()
                            .unwrap_or(FALLING_DELTA_XS_OVERFLOW);
                        if obj.movement & 0x80 != 0 {
                            obj.x = obj.x.wrapping_sub(delta);
                        } else {
                            obj.x = obj.x.wrapping_add(delta);
                        }
                    }
                }
                // The loop ends when the first object's Y reaches 104.
                if *frame >= FALLING_FRAMES {
                    self.objects = Objects::None;
                }
            }
            Objects::Droplets { iter, half, base_x } => {
                // The droplets pass draws the whole grid (advancing
                // base_x), then cleans OAM and waits a frame — one frame per
                // half; the base X persists between calls, scrolling the
                // grid.
                *base_x = Self::droplet_grid_end_x(*base_x, if *half == 0 { 16 } else { 24 });
                *half += 1;
                if *half >= 2 {
                    *half = 0;
                    *iter += 1;
                    if *iter >= 32 {
                        self.objects = Objects::None;
                    }
                }
            }
        }
    }

    /// Simulate one droplets pass over the grid and return the resulting
    /// base X.
    fn droplet_grid_end_x(mut x: i32, base_y: i32) -> i32 {
        let mut y = base_y;
        loop {
            x += 27;
            if x >= 144 {
                x -= 168;
                y += 16;
                if y >= 112 {
                    return x;
                }
            }
        }
    }

    // ─── Queries used while drawing the scene ────────────────────────

    /// Whether the mon pic is currently hidden by an effect (blink-off phase,
    /// squish/shake-back-and-forth aftermath, Hide effects).
    pub fn mon_hidden(&self, side: MonSide) -> bool {
        if self.forced_hidden[side.index()] {
            return true;
        }
        if let Some(blink) = self.blink {
            if blink.side == side && blink.frame % 10 < 5 {
                return true;
            }
        }
        false
    }

    /// Horizontal pic offset from the back-and-forth shake (±1 tile around
    /// the normal position: (0,5)/(2,5) vs (1,5), and (11,0)/(13,0) vs
    /// (12,0)).
    pub fn mon_dx(&self, side: MonSide) -> i32 {
        if let Some(bnf) = self.shake_bnf {
            if bnf.side == side {
                return if (bnf.frame / 3) % 2 == 0 { -8 } else { 8 };
            }
        }
        0
    }

    /// Vertical pic offset from the bounce (Splash): five 21-frame
    /// slide-down runs (one row per ~3 frames), the mon popping back to the
    /// top between runs; the final 3 frames are the re-show (back at the
    /// normal position).
    pub fn mon_dy(&self, side: MonSide) -> i32 {
        if let Some(bounce) = self.bounce {
            if bounce.side == side {
                let frame = bounce.frame as i32;
                if frame >= 5 * 21 {
                    return 0;
                }
                let within = frame % 21;
                return (within / 3 + 1) * 8;
            }
        }
        0
    }

    /// Active slide-down-and-hide (Acid Armor) parameters for a side:
    /// (visible tile rows, vertical offset in px) — the pic is cropped to
    /// its top rows and drawn lower, matching the 7×5/7×3 tile-id lists.
    pub fn slide_down_hide_params(&self, side: MonSide) -> Option<(u32, i32)> {
        match self.slide_down_hide {
            Some(s) if s.side == side => {
                if s.frame < 8 {
                    Some((5, 16))
                } else {
                    Some((3, 32))
                }
            }
            _ => None,
        }
    }

    /// Active squish parameters for a side: (width in tiles, anchor_right).
    pub fn squish_params(&self, side: MonSide) -> Option<(u32, bool)> {
        match self.squish {
            Some(s) if s.side == side => Some(s.params()),
            _ => None,
        }
    }

    /// Whether the mon pic is replaced by the minimize blob.
    pub fn is_minimized(&self, side: MonSide) -> bool {
        self.minimized[side.index()]
    }

    /// Whether the mon pic is replaced by the substitute doll.
    pub fn is_substitute(&self, side: MonSide) -> bool {
        self.substitute[side.index()]
    }

    /// Whether an object effect (balls/petals/leaves/droplets) is active.
    pub fn objects_active(&self) -> bool {
        !matches!(self.objects, Objects::None)
    }

    /// Current SCX offset for the enemy-HUD shake (0 when inactive).
    pub fn enemy_hud_shake_offset(&self) -> i32 {
        match self.hud_shake {
            Some(hud) => {
                if (hud.frame / 2) % 2 == 0 {
                    2
                } else {
                    -2
                }
            }
            None => 0,
        }
    }

    // ─── Framebuffer passes ──────────────────────────────────────────

    /// SCX shake of the enemy HUD strip. Call after the background/HUD is
    /// drawn but BEFORE the mon sprites, so only the HUD (and background
    /// rows 0..7) shakes — the original protects the player back pic by
    /// copying it to OAM first.
    pub fn apply_enemy_hud_shake(&self, fb: &mut RgbaIndexedFrameBuffer) {
        let dx = self.enemy_hud_shake_offset();
        if dx != 0 {
            shift_rows_h(fb, HUD_SHAKE_HEIGHT, dx);
        }
    }

    /// Full-screen post effects: screen shake, wavy screen, palette tint,
    /// screen flash. Call once the scene is fully drawn.
    pub fn apply_screen_effects(&self, fb: &mut RgbaIndexedFrameBuffer) {
        if let Some(shake) = self.shake {
            let (dx, dy) = shake.offset();
            shift_rows_h(fb, SCREEN_SHAKE_HEIGHT, dx);
            shift_rows_v(fb, SCREEN_SHAKE_HEIGHT, dy);
        }

        if self.wave_frame > 0 {
            // Wavy screen: per-scanline SCX from WAVY_LINE_OFFSETS;
            // the table start advances one entry per frame.
            let w = fb.width() as usize;
            let src = fb.indexed().clone();
            let start = (self.wave_frame - 1) as usize;
            for y in 0..fb.height() as usize {
                let shift = WAVY_LINE_OFFSETS[(start + y) % 32] as i32;
                if shift == 0 {
                    continue;
                }
                for x in 0..w as i32 {
                    let sx = (x + shift).clamp(0, w as i32 - 1) as usize;
                    let color = src.get_pixel(sx as u32, y as u32).unwrap_or(GbColor::White);
                    fb.set_pixel_index(x as u32, y as u32, color);
                }
            }
        }

        if let Some(tint) = self.tint {
            remap_shades(fb, &tint.map());
        }

        match self.flash {
            Some(Flash::Short { frame }) => {
                if frame < 2 {
                    // rBGP = %00011011: inverted colors.
                    remap_shades(fb, &[3, 2, 1, 0]);
                } else {
                    // rBGP = 0: white out.
                    fb.clear(Rgba::WHITE);
                }
            }
            Some(Flash::Long { frame }) => {
                // 3 cycles through the 12-step table: the first cycle holds
                // each step 2 frames, cycles 2-3 hold 1 frame (48 total).
                let step = if frame < 24 {
                    (frame / 2) as usize
                } else {
                    (frame - 24) as usize % 12
                };
                remap_shades(fb, &FLASH_SCREEN_LONG_PALETTE[step]);
            }
            None => {}
        }
    }

    /// Draw the active object effect (balls/petals/leaves/droplets) with the
    /// move-animation tilesets. `ts0`/`ts1` are the tilesets loaded from
    /// move_anim_0.png / move_anim_1.png (tiles indexed from 0; the absolute
    /// VRAM tile ids used by the effects are converted with
    /// [`ANIM_BASE_TILE_ID`]).
    pub fn render_objects(
        &self,
        fb: &mut RgbaIndexedFrameBuffer,
        ts0: &TileSet,
        ts1: &TileSet,
        pal: &Palette,
    ) {
        let mut layer = SpriteLayer::new();
        match &self.objects {
            Objects::None => return,
            Objects::SpiralBalls { side, frame } => {
                let step = (*frame / 5) as usize;
                // Enemy turn: spiral-balls base offset (-40, 80).
                let (base_y, base_x) = match side {
                    MonSide::Player => (0i32, 0i32),
                    MonSide::Enemy => (-40, 80),
                };
                for k in 0..3 {
                    let Some(&(y, x)) = SPIRAL_BALL_COORDS.get(step + k) else {
                        break;
                    };
                    layer.add(Self::oam(
                        base_y + y as i32,
                        base_x + x as i32,
                        BALL_TILE,
                        0,
                    ));
                }
                layer.render(fb, ts0, pal, pal, None);
            }
            Objects::ShootBalls {
                pillars, active, ..
            } => {
                let pillar = &pillars[*active];
                for &y in &pillar.balls {
                    layer.add(Self::oam(y, pillar.base_x, BALL_TILE, 0));
                }
                layer.render(fb, ts0, pal, pal, None);
            }
            Objects::Falling { tile, objects, .. } => {
                for obj in objects {
                    let attr = if obj.movement & 0x80 != 0 {
                        OAM_X_FLIP
                    } else {
                        0
                    };
                    layer.add(Self::oam(obj.y as i32, obj.x as i32, *tile, attr));
                }
                layer.render(fb, ts1, pal, pal, None);
            }
            Objects::Droplets { half, base_x, .. } => {
                let base_y = if *half == 0 { 16 } else { 24 };
                let mut x = *base_x;
                let mut y = base_y;
                loop {
                    x += 27;
                    if x >= 144 {
                        x -= 168;
                        y += 16;
                        if y >= 112 {
                            break;
                        }
                    }
                    layer.add(Self::oam(y, x, DROPLET_TILE, 0));
                }
                layer.render(fb, ts0, pal, pal, None);
            }
        }
    }

    /// OAM → screen coordinates (hardware OAM Y = screen Y + 16, X = screen
    /// X + 8) and absolute → tileset-relative tile id.
    fn oam(oam_y: i32, oam_x: i32, tile: u8, attr: u8) -> SpriteOamEntry {
        SpriteOamEntry::new(
            oam_y - 16,
            oam_x - 8,
            tile.wrapping_sub(ANIM_BASE_TILE_ID),
            attr,
        )
    }

    // ─── Mon pic replacements / transforms ───────────────────────────

    /// Draw the substitute doll over a mon pic rect.
    ///
    /// `doll` is the mini-doll tileset (gfx/sprites/monster.png, 24
    /// tiles). Placement within the 7×7 pic (mon pic tiles are column-major,
    /// tile = col*7 + row):
    ///   - enemy turn (facing down): tiles [0,1;2,3] at (col 2, row 4)
    ///   - player turn (facing up):  tiles [4,5;6,7] at (col 3, row 4)
    pub fn draw_substitute(
        fb: &mut RgbaIndexedFrameBuffer,
        rect: MonRect,
        doll: &TileSet,
        pal: &Palette,
        side: MonSide,
    ) {
        let (base_tile, dx, dy) = match side {
            MonSide::Enemy => (0usize, 2 * TILE_SIZE, 4 * TILE_SIZE),
            MonSide::Player => (4, 3 * TILE_SIZE, 4 * TILE_SIZE),
        };
        let origin_x = rect.x + dx as i32;
        let origin_y = rect.y + dy as i32;
        for t in 0..4usize {
            let tile = doll.get(base_tile + t);
            let tx = origin_x + (t % 2) as i32 * TILE_PIXELS as i32;
            let ty = origin_y + (t / 2) as i32 * TILE_PIXELS as i32;
            Self::draw_tile_opaque(fb, tile, tx, ty, pal);
        }
    }

    /// Draw the minimize blob over a mon pic rect. Placement: pic base +
    /// (7*3+4) tiles + TILE_SIZE/4 → (col 3, row 4) + 2 px, i.e.
    /// pic-relative (24, 34); color index 3.
    pub fn draw_minimized(fb: &mut RgbaIndexedFrameBuffer, rect: MonRect, pal: &Palette) {
        let color = pal.color(GbColor::from_u8(3));
        let ox = rect.x + 3 * TILE_SIZE as i32;
        let oy = rect.y + 4 * TILE_SIZE as i32 + 2;
        for (row, bits) in MINIMIZED_BLOB.iter().enumerate() {
            for (col, &on) in bits.iter().enumerate() {
                if on != 0 {
                    let x = ox + col as i32;
                    let y = oy + row as i32;
                    if x >= 0 && y >= 0 {
                        fb.set_pixel(x as u32, y as u32, color);
                    }
                }
            }
        }
    }

    /// Draw a mon pic tileset squished horizontally to `width_tiles` tiles
    /// (the squish narrows the 7-tile pic one tile per pass, alternating the
    /// anchored side). Nearest-neighbor scale; color 0 is transparent like
    /// the normal mon blit.
    pub fn draw_squished(
        fb: &mut RgbaIndexedFrameBuffer,
        ts: &TileSet,
        x: i32,
        y: i32,
        tiles_per_row: u32,
        pal: &Palette,
        width_tiles: u32,
        anchor_right: bool,
    ) {
        if width_tiles == 0 {
            return;
        }
        let full_w = tiles_per_row * TILE_SIZE;
        let out_w = width_tiles * TILE_SIZE;
        let ox = if anchor_right {
            x + (full_w - out_w) as i32
        } else {
            x
        };
        let total_rows = ts.len() as u32 / tiles_per_row;
        for dy in 0..total_rows * TILE_SIZE {
            let ty = dy / TILE_SIZE;
            for dx in 0..out_w {
                // Map the output column back to the full-width source.
                let sx = dx * full_w / out_w;
                let tx = sx / TILE_SIZE;
                let tile = ts.get((ty * tiles_per_row + tx) as usize);
                let c = tile.get((dy % TILE_SIZE) as usize, (sx % TILE_SIZE) as usize);
                if c == 0 {
                    continue;
                }
                let px = ox + dx as i32;
                let py = y + dy as i32;
                if px >= 0 && py >= 0 {
                    fb.set_pixel(px as u32, py as u32, pal.color(GbColor::from_u8(c)));
                }
            }
        }
    }

    /// Draw only the top `rows` tile rows of a mon pic tileset (the
    /// slide-down-and-hide redraws the pic with the 7×5 / 7×3 tile-id lists
    /// — a crop, not a scale). Color 0 is transparent like the normal mon
    /// blit.
    pub fn draw_mon_rows(
        fb: &mut RgbaIndexedFrameBuffer,
        ts: &TileSet,
        x: i32,
        y: i32,
        tiles_per_row: u32,
        pal: &Palette,
        rows: u32,
    ) {
        let max_tiles = (rows * tiles_per_row) as usize;
        for idx in 0..max_tiles.min(ts.len()) {
            let tile = ts.get(idx);
            let tx = idx as u32 % tiles_per_row;
            let ty = idx as u32 / tiles_per_row;
            for row in 0..TILE_PIXELS {
                for col in 0..TILE_PIXELS {
                    let c = tile.get(row, col);
                    if c == 0 {
                        continue;
                    }
                    let px = x + (tx * TILE_SIZE) as i32 + col as i32;
                    let py = y + (ty * TILE_SIZE) as i32 + row as i32;
                    if px >= 0 && py >= 0 {
                        fb.set_pixel(px as u32, py as u32, pal.color(GbColor::from_u8(c)));
                    }
                }
            }
        }
    }

    /// Draw one 8×8 tile with all four shades opaque (the substitute doll
    /// replaces the mon pic area, whose background was blanked).
    fn draw_tile_opaque(
        fb: &mut RgbaIndexedFrameBuffer,
        tile: &crate::tile::Tile,
        x: i32,
        y: i32,
        pal: &Palette,
    ) {
        for row in 0..TILE_PIXELS {
            for col in 0..TILE_PIXELS {
                let c = tile.get(row, col);
                let px = x + col as i32;
                let py = y + row as i32;
                if px >= 0 && py >= 0 {
                    fb.set_pixel(px as u32, py as u32, pal.color(GbColor::from_u8(c)));
                }
            }
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::GRAYSCALE_PALETTE;
    use crate::tile::Tile;
    use crate::RenderConfig;

    fn fb() -> RgbaIndexedFrameBuffer {
        RgbaIndexedFrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE)
    }

    fn apply(fx: &mut BattleEffects, effect: AnimEffect) -> u8 {
        fx.apply(&effect, MonSide::Player)
    }

    // ── SquishMonPic ─────────────────────────────────────────────────

    #[test]
    fn squish_narrows_one_tile_per_pass_alternating_anchor() {
        let mut fx = BattleEffects::new();
        let wait = apply(&mut fx, AnimEffect::SquishMonPic);
        assert_eq!(wait, 26);
        // Pass 1 (frames 0-2): 6 tiles, anchored left. Pass 2: 5, right. …
        let expected_widths = [6, 5, 4, 3, 2, 1, 0, 0];
        for (pass, &w) in expected_widths.iter().enumerate() {
            let (width, anchor_right) = fx.squish_params(MonSide::Player).unwrap();
            assert_eq!(width, w, "pass {}", pass);
            assert_eq!(anchor_right, pass % 2 == 1, "pass {}", pass);
            for _ in 0..3 {
                fx.tick();
            }
        }
        // 8 passes × 3 frames = 24; two more frames (hide + one more frame).
        fx.tick();
        fx.tick();
        assert!(fx.squish_params(MonSide::Player).is_none());
        // Ends hidden.
        assert!(fx.mon_hidden(MonSide::Player));
        // Re-show clears it.
        fx.apply(&AnimEffect::ShowPlayerMon, MonSide::Player);
        assert!(!fx.mon_hidden(MonSide::Player));
    }

    #[test]
    fn squish_targets_attacker_side_only() {
        let mut fx = BattleEffects::new();
        fx.apply(&AnimEffect::SquishMonPic, MonSide::Enemy);
        assert!(fx.squish_params(MonSide::Enemy).is_some());
        assert!(fx.squish_params(MonSide::Player).is_none());
    }

    // ── TransformMon ─────────────────────────────────────────────────

    #[test]
    fn transform_blocks_stream_and_reveals_after_beat() {
        let mut fx = BattleEffects::new();
        // The SE blocks the command stream (was a 0-wait no-op).
        let wait = apply(&mut fx, AnimEffect::TransformMon);
        assert_eq!(wait, 12);
        // The pic hides immediately (covered by the preceding poof)…
        assert!(fx.mon_hidden(MonSide::Player));
        assert!(!fx.mon_hidden(MonSide::Enemy));
        // …stays hidden for the swap beat…
        for _ in 0..5 {
            fx.tick();
        }
        assert!(fx.mon_hidden(MonSide::Player));
        // …then reappears (drawn as the opposing species by the frontend)
        // before the block ends.
        fx.tick();
        assert!(!fx.mon_hidden(MonSide::Player));
        for _ in 0..6 {
            fx.tick();
        }
        // Latch stays clear once the transform state has run out.
        assert!(!fx.mon_hidden(MonSide::Player));
    }

    #[test]
    fn transform_targets_attacker_side() {
        let mut fx = BattleEffects::new();
        fx.apply(&AnimEffect::TransformMon, MonSide::Enemy);
        assert!(fx.mon_hidden(MonSide::Enemy));
        assert!(!fx.mon_hidden(MonSide::Player));
        // A switch/faint on that side cancels the pending reveal cleanly.
        fx.clear_side(MonSide::Enemy);
        fx.tick();
        assert!(!fx.mon_hidden(MonSide::Enemy));
    }

    // ── ShakeBackAndForth ────────────────────────────────────────────

    #[test]
    fn shake_back_and_forth_offsets_and_duration() {
        let mut fx = BattleEffects::new();
        let wait = apply(&mut fx, AnimEffect::ShakeBackAndForth);
        assert_eq!(wait, 96);
        for i in 0..32u8 {
            let want = if i % 2 == 0 { -8 } else { 8 };
            for _ in 0..3 {
                assert_eq!(fx.mon_dx(MonSide::Player), want);
                fx.tick();
            }
        }
        assert_eq!(fx.mon_dx(MonSide::Player), 0);
        // The mon's sprite disappears after this animation.
        assert!(fx.mon_hidden(MonSide::Player));
    }

    // ── Blink ────────────────────────────────────────────────────────

    #[test]
    fn blink_hides_first_five_of_ten() {
        let mut fx = BattleEffects::new();
        let wait = apply(&mut fx, AnimEffect::BlinkPlayerMon { times: 6 });
        assert_eq!(wait, 60);
        for cycle in 0..6 {
            for _ in 0..5 {
                assert!(fx.mon_hidden(MonSide::Player), "cycle {}", cycle);
                fx.tick();
            }
            for _ in 0..5 {
                assert!(!fx.mon_hidden(MonSide::Player), "cycle {}", cycle);
                fx.tick();
            }
        }
    }

    #[test]
    fn blink_enemy_mon_blinks_defender() {
        // The enemy blink targets the flipped-turn mon: on the player's
        // turn it blinks the enemy, and vice versa.
        let mut fx = BattleEffects::new();
        fx.apply(&AnimEffect::BlinkEnemyMon { times: 6 }, MonSide::Enemy);
        assert!(fx.mon_hidden(MonSide::Player));
        assert!(!fx.mon_hidden(MonSide::Enemy));
    }

    // ── SpiralBallsInward ────────────────────────────────────────────

    #[test]
    fn spiral_balls_follow_coordinate_sequence() {
        let mut fx = BattleEffects::new();
        let wait = apply(&mut fx, AnimEffect::SpiralBallsInward);
        assert_eq!(wait, 105);
        let ts = TileSet::blank(80);
        // Frame 0: 3 balls at the first 3 coordinate pairs (player base 0,0).
        let mut buf = fb();
        fx.render_objects(&mut buf, &ts, &ts, &GRAYSCALE_PALETTE);
        if let Objects::SpiralBalls { frame, .. } = &fx.objects {
            assert_eq!(*frame, 0);
        } else {
            panic!("expected spiral balls");
        }
        // 21 steps × 5 frames, then a short flash fires at the end.
        for _ in 0..105 {
            fx.tick();
        }
        assert!(!fx.objects_active());
        assert!(matches!(fx.flash, Some(Flash::Short { .. })));
    }

    #[test]
    fn spiral_balls_enemy_base_offset() {
        let mut fx = BattleEffects::new();
        fx.apply(&AnimEffect::SpiralBallsInward, MonSide::Enemy);
        // Enemy base (-40, 80): first ball at OAM (0x38-40, 0x28+80).
        if let Objects::SpiralBalls { side, .. } = &fx.objects {
            assert_eq!(*side, MonSide::Enemy);
        } else {
            panic!("expected spiral balls");
        }
    }

    // ── ShootBallsUpward ─────────────────────────────────────────────

    #[test]
    fn shoot_balls_rise_and_pop_at_top() {
        let mut fx = BattleEffects::new();
        let wait = apply(&mut fx, AnimEffect::ShootBallsUpward { many: false });
        // 5 balls: the last pops after 2*(5-1) = 8 frames.
        assert_eq!(wait, 8);
        if let Objects::ShootBalls { pillars, .. } = &fx.objects {
            // Player turn: base Y = 48, balls at 56, 64, 72, 80, 88.
            assert_eq!(pillars[0].base_y, 48);
            assert_eq!(pillars[0].base_x, 40);
            assert_eq!(pillars[0].balls, vec![56, 64, 72, 80, 88]);
        } else {
            panic!("expected shoot balls");
        }
        // Frame 1: first ball already at top (56 = 48+8) → removed.
        fx.tick();
        if let Objects::ShootBalls { pillars, .. } = &fx.objects {
            assert_eq!(pillars[0].balls, vec![60, 68, 76, 84]);
        }
        for _ in 0..7 {
            fx.tick();
        }
        assert!(!fx.objects_active());
    }

    #[test]
    fn shoot_many_balls_runs_six_pillars() {
        let mut fx = BattleEffects::new();
        fx.apply(
            &AnimEffect::ShootBallsUpward { many: true },
            MonSide::Player,
        );
        if let Objects::ShootBalls { pillars, .. } = &fx.objects {
            assert_eq!(pillars.len(), 6);
            assert_eq!(pillars[0].base_x, 0x10);
            assert_eq!(pillars[0].base_y, 0x50);
            assert_eq!(pillars[0].balls.len(), 4);
        } else {
            panic!("expected shoot balls");
        }
        // Run to completion; must terminate.
        for _ in 0..100 {
            fx.tick();
        }
        assert!(!fx.objects_active());
    }

    // ── Falling objects (petals/leaves) ──────────────────────────────

    #[test]
    fn petals_fall_until_first_reaches_104() {
        let mut fx = BattleEffects::new();
        let wait = apply(&mut fx, AnimEffect::PetalsFalling);
        assert_eq!(wait, 156);
        if let Objects::Falling { tile, objects, .. } = &fx.objects {
            assert_eq!(*tile, PETAL_TILE);
            assert_eq!(objects.len(), 20);
            // First object starts at Y 0, others at 8*(i+1).
            assert_eq!(objects[0].y, 0);
            assert_eq!(objects[1].y, 16);
            assert_eq!(objects[0].x, FALLING_INITIAL_X[0]);
        } else {
            panic!("expected falling petals");
        }
        // After one tick (3 frames): first object Y = 2.
        fx.tick();
        fx.tick();
        fx.tick();
        if let Objects::Falling { objects, tick, .. } = &fx.objects {
            assert_eq!(*tick, 1);
            assert_eq!(objects[0].y, 2);
        }
        for _ in 0..153 {
            fx.tick();
        }
        assert!(!fx.objects_active());
    }

    #[test]
    fn leaves_use_leaf_tile_and_three_objects() {
        let mut fx = BattleEffects::new();
        fx.apply(&AnimEffect::LeavesFalling, MonSide::Player);
        if let Objects::Falling { tile, objects, .. } = &fx.objects {
            assert_eq!(*tile, LEAF_TILE);
            assert_eq!(objects.len(), 3);
        } else {
            panic!("expected falling leaves");
        }
    }

    #[test]
    fn falling_movement_byte_wraps_and_flips_direction() {
        // Object 0 starts with movement 0x00: deltas increase 0,1,3,5,7,9,11,13,15
        // then wrap to 0 with the direction bit flipped (moving left).
        let mut fx = BattleEffects::new();
        fx.apply(&AnimEffect::LeavesFalling, MonSide::Player);
        let x0 = FALLING_INITIAL_X[0];
        // Tick 1: movement 1 → delta 1. Tick 2: movement 2 → delta 3. …
        let mut expected_x = x0;
        for (i, delta) in [1u8, 3, 5, 7, 9, 11, 13, 15].iter().enumerate() {
            for _ in 0..3 {
                fx.tick();
            }
            expected_x = expected_x.wrapping_add(*delta);
            if let Objects::Falling { objects, .. } = &fx.objects {
                assert_eq!(objects[0].x, expected_x, "after tick {}", i + 1);
            }
        }
        // Tick 9: movement would be 9 → wraps to 0 with direction flipped
        // (0x80): delta 0, X unchanged but now moving left with X flip.
        for _ in 0..3 {
            fx.tick();
        }
        if let Objects::Falling { objects, .. } = &fx.objects {
            assert_eq!(objects[0].x, expected_x);
            assert_eq!(objects[0].movement, 0x80);
        }
    }

    // ── Water droplets ───────────────────────────────────────────────

    #[test]
    fn droplets_run_64_frames_and_scroll() {
        let mut fx = BattleEffects::new();
        let wait = apply(&mut fx, AnimEffect::WaterDroplets);
        assert_eq!(wait, 64);
        let x_start = match &fx.objects {
            Objects::Droplets { base_x, .. } => *base_x,
            _ => panic!("expected droplets"),
        };
        assert_eq!(x_start, -16);
        fx.tick();
        // After the first half-frame the grid advanced base_x.
        let x_after = match &fx.objects {
            Objects::Droplets { base_x, .. } => *base_x,
            _ => panic!("expected droplets"),
        };
        assert_ne!(x_after, x_start);
        for _ in 0..63 {
            fx.tick();
        }
        assert!(!fx.objects_active());
    }

    // ── HUD shake ────────────────────────────────────────────────────

    #[test]
    fn hud_shake_alternates_plus_minus_two() {
        let mut fx = BattleEffects::new();
        let wait = apply(&mut fx, AnimEffect::ShakeEnemyHud { variant: 1 });
        assert_eq!(wait, 32);
        let pattern = [2, 2, -2, -2];
        for i in 0..32 {
            assert_eq!(fx.enemy_hud_shake_offset(), pattern[i % 4], "frame {}", i);
            fx.tick();
        }
        assert_eq!(fx.enemy_hud_shake_offset(), 0);
    }

    // ── Wavy screen ──────────────────────────────────────────────────

    #[test]
    fn wavy_screen_lasts_255_frames() {
        let mut fx = BattleEffects::new();
        let wait = apply(&mut fx, AnimEffect::WavyScreen);
        assert_eq!(wait, 255);
        for _ in 0..255 {
            fx.tick();
        }
        assert_eq!(fx.wave_frame, 0);
    }

    // ── Screen shake ─────────────────────────────────────────────────

    #[test]
    fn shake_screen_decays_amplitude() {
        // Screen shake (pixels = 8): displaced 4 frames then at rest 5
        // frames per amplitude step, 8 → 1 (72 fr).
        let mut fx = BattleEffects::new();
        let wait = apply(
            &mut fx,
            AnimEffect::ShakeScreenH {
                pixels: 8,
                frames: 72,
            },
        );
        assert_eq!(wait, 72);
        let offsets: Vec<i32> = (0..9)
            .map(|_| {
                let o = fx.shake.unwrap().offset().0;
                fx.tick();
                o
            })
            .collect();
        assert_eq!(offsets, vec![8, 8, 8, 8, 0, 0, 0, 0, 0]);
        // Second step: amplitude 7.
        assert_eq!(fx.shake.unwrap().offset().0, 7);
        for _ in 0..63 {
            fx.tick();
        }
        assert!(fx.shake.is_none());
    }

    #[test]
    fn small_shake_alternates() {
        let mut fx = BattleEffects::new();
        apply(
            &mut fx,
            AnimEffect::ShakeScreenV {
                pixels: 1,
                frames: 4,
            },
        );
        let offsets: Vec<i32> = (0..4)
            .map(|_| {
                let o = fx.shake.unwrap().offset().1;
                fx.tick();
                o
            })
            .collect();
        assert_eq!(offsets, vec![1, -1, 1, -1]);
        assert!(fx.shake.is_none());
    }

    // ── Flash ────────────────────────────────────────────────────────

    #[test]
    fn flash_short_and_long_durations() {
        let mut fx = BattleEffects::new();
        assert_eq!(apply(&mut fx, AnimEffect::FlashScreen { frames: 4 }), 4);
        for _ in 0..4 {
            fx.tick();
        }
        assert!(fx.flash.is_none());
        assert_eq!(apply(&mut fx, AnimEffect::FlashScreen { frames: 48 }), 48);
        for _ in 0..48 {
            fx.tick();
        }
        assert!(fx.flash.is_none());
    }

    #[test]
    fn flash_long_palette_step_timing() {
        // 3 cycles through the 12-step table: first cycle 2 frames/step,
        // cycles 2-3 1 frame/step. Mirrors apply_screen_effects.
        let step_of = |frame: u8| {
            if frame < 24 {
                (frame / 2) as usize
            } else {
                (frame - 24) as usize % 12
            }
        };
        assert_eq!(step_of(0), 0);
        assert_eq!(step_of(1), 0);
        assert_eq!(step_of(2), 1);
        assert_eq!(step_of(23), 11);
        assert_eq!(step_of(24), 0);
        assert_eq!(step_of(35), 11);
        assert_eq!(step_of(36), 0);
        assert_eq!(step_of(47), 11);
    }

    // ── BounceUpAndDown ──────────────────────────────────────────────

    #[test]
    fn bounce_slides_down_five_times_then_shows() {
        let mut fx = BattleEffects::new();
        // Bounce: 5 × slide-down runs + a re-show.
        let wait = apply(&mut fx, AnimEffect::BounceUpAndDown);
        assert_eq!(wait, 5 * 21 + 3);
        for cycle in 0..5 {
            // Each cycle: one row (8 px) per ~3 frames, 8..56 px.
            for step in 0..7 {
                for _ in 0..3 {
                    assert_eq!(
                        fx.mon_dy(MonSide::Player),
                        (step + 1) * 8,
                        "cycle {} step {}",
                        cycle,
                        step
                    );
                    fx.tick();
                }
            }
        }
        // Re-show: back at the top, not hidden.
        assert_eq!(fx.mon_dy(MonSide::Player), 0);
        assert!(!fx.mon_hidden(MonSide::Player));
    }

    // ── SlideMonDownAndHide ──────────────────────────────────────────

    #[test]
    fn slide_down_hide_shrinks_rows_then_hides() {
        let mut fx = BattleEffects::new();
        let wait = apply(&mut fx, AnimEffect::SlidePlayerMonDownAndHide);
        assert_eq!(wait, 16);
        // Step 1 (8 frames): top 5 rows, drawn 2 rows lower.
        for _ in 0..8 {
            assert_eq!(fx.slide_down_hide_params(MonSide::Player), Some((5, 16)));
            fx.tick();
        }
        // Step 2 (8 frames): top 3 rows, drawn 4 rows lower.
        for _ in 0..8 {
            assert_eq!(fx.slide_down_hide_params(MonSide::Player), Some((3, 32)));
            fx.tick();
        }
        assert_eq!(fx.slide_down_hide_params(MonSide::Player), None);
        // Ends hidden, with the pic tile data blanked.
        assert!(fx.mon_hidden(MonSide::Player));
        fx.apply(&AnimEffect::ShowPlayerMon, MonSide::Player);
        assert!(!fx.mon_hidden(MonSide::Player));
    }

    #[test]
    fn draw_mon_rows_crops_bottom_rows() {
        // 7×7 tile pic, tile i solid color (i % 4).
        let mut ts = TileSet::blank(49);
        for i in 0..49 {
            let mut t = Tile::blank();
            for row in 0..8 {
                for col in 0..8 {
                    t.pixels[row][col] = (i % 4) as u8;
                }
            }
            ts.set(i, t);
        }
        let mut buf = fb();
        buf.clear(Rgba::WHITE);
        BattleEffects::draw_mon_rows(&mut buf, &ts, 8, 40, 7, &GRAYSCALE_PALETTE, 5);
        // Row 4 (tiles 28..35, color 0) drawn; row 5 (tile 35, color 3) not.
        let below = buf.get_pixel(8, 40 + 5 * 8).unwrap();
        assert_eq!((below.r, below.g, below.b), (255, 255, 255));
        // Tile 1 (color 1) in the first row is drawn.
        let c1 = buf.get_pixel(8 + 8, 40).unwrap();
        assert_eq!((c1.r, c1.g, c1.b), (170, 170, 170));
    }

    // ── Substitute / minimize latches ────────────────────────────────

    #[test]
    fn substitute_and_minimize_latch_until_cleared() {
        let mut fx = BattleEffects::new();
        apply(&mut fx, AnimEffect::SubstituteMon);
        assert!(fx.is_substitute(MonSide::Player));
        assert!(!fx.is_substitute(MonSide::Enemy));
        fx.clear_side(MonSide::Player);
        assert!(!fx.is_substitute(MonSide::Player));

        apply(&mut fx, AnimEffect::MinimizeMon);
        assert!(fx.is_minimized(MonSide::Player));
        fx.clear_side(MonSide::Player);
        assert!(!fx.is_minimized(MonSide::Player));
    }

    // ── Drawing ──────────────────────────────────────────────────────

    #[test]
    fn draw_substitute_places_doll_tiles() {
        // Doll tileset: tile i filled with color (i % 4).
        let mut doll = TileSet::blank(24);
        for i in 0..24 {
            let mut t = Tile::blank();
            for row in 0..8 {
                for col in 0..8 {
                    t.pixels[row][col] = (i % 4) as u8;
                }
            }
            doll.set(i, t);
        }
        let mut buf = fb();
        let rect = MonRect { x: 96, y: 0 };
        BattleEffects::draw_substitute(&mut buf, rect, &doll, &GRAYSCALE_PALETTE, MonSide::Enemy);
        // Enemy doll: tiles [0,1;2,3] at (col 2, row 4) → px (96+16, 32).
        assert!(buf.get_pixel(96 + 16, 32).is_some());
        // Tile 0 (color 0 → white) at top-left; tile 1 (color 1) at top-right.
        let white = buf.get_pixel(96 + 16, 32).unwrap();
        assert_eq!((white.r, white.g, white.b), (255, 255, 255));
        let c1 = buf.get_pixel(96 + 16 + 8, 32).unwrap();
        assert_eq!((c1.r, c1.g, c1.b), (170, 170, 170));
        // Pixel outside the doll is untouched (white background anyway, so
        // check the boundary: one column left of the doll is from tile 0
        // too — check below the doll instead).
        let below = buf.get_pixel(96 + 16, 32 + 16).unwrap();
        assert_eq!((below.r, below.g, below.b), (255, 255, 255));
    }

    #[test]
    fn draw_minimized_blob_at_pic_offset() {
        let mut buf = fb();
        buf.clear(Rgba::WHITE);
        BattleEffects::draw_minimized(&mut buf, MonRect { x: 8, y: 40 }, &GRAYSCALE_PALETTE);
        // Blob at pic-relative (24, 34) → (32, 74); row 0 = ..XX....
        assert_eq!(buf.get_pixel(32 + 3, 74).unwrap().r, 0);
        assert_eq!(buf.get_pixel(32 + 4, 74).unwrap().r, 0);
        assert_eq!(buf.get_pixel(32 + 2, 74).unwrap().r, 255);
        // Row 4 = ..X..X..
        assert_eq!(buf.get_pixel(32 + 2, 74 + 4).unwrap().r, 0);
        assert_eq!(buf.get_pixel(32 + 5, 74 + 4).unwrap().r, 0);
    }

    #[test]
    fn draw_squished_scales_horizontally() {
        // 7×1 tile strip, each tile a solid color = its index.
        let mut ts = TileSet::blank(7);
        for i in 0..7 {
            let mut t = Tile::blank();
            for row in 0..8 {
                for col in 0..8 {
                    t.pixels[row][col] = (i % 4) as u8;
                }
            }
            ts.set(i, t);
        }
        let mut buf = fb();
        BattleEffects::draw_squished(&mut buf, &ts, 0, 0, 7, &GRAYSCALE_PALETTE, 4, false);
        // 4/7 width → 32 px from the left; source column = dx * 56 / 32.
        // dx 0..8 → src 0..14 (tile 0/1), dx 8..16 → src 14..28 ...
        assert!(buf.get_pixel(0, 4).is_some());
        // Anchored left: nothing drawn at x >= 32.
        let right = buf.get_pixel(33, 4).unwrap();
        assert_eq!((right.r, right.g, right.b), (255, 255, 255));
    }
}
