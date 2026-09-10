// ─── Constants ───────────────────────────────────────────────────────

/// Base tile ID added to frame block tile indices (original: $31).

pub const ANIM_BASE_TILE_ID: u8 = 0x31;

/// Number of subanimations (NUM_SUBANIMS = 86).
pub const NUM_SUBANIMS: usize = 86;

/// Number of frame blocks (NUM_FRAMEBLOCKS = 122, 0x00..0x79).
pub const NUM_FRAMEBLOCKS: usize = 122;

/// Number of base coordinates (NUM_BASECOORDS = 177, 0x00..0xB0).
pub const NUM_BASECOORDS: usize = 177;

/// First special-effect ID in the command stream.
/// IDs < this are subanimation commands; IDs >= this are special effects.
pub const FIRST_SE_ID: u8 = 0xD8;

/// Animation terminator byte.
pub const ANIM_END: u8 = 0xFF;

/// Flip constants matching OAM flags.
pub const OAM_XFLIP: u8 = 0x20;
pub const OAM_YFLIP: u8 = 0x40;

// ─── Subanimation transform types ────────────────────────────────────

/// How a subanimation's frame block tiles are transformed when rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SubAnimTransform {
    /// No transformation — use base coords + offsets directly.
    Normal = 0,
    /// Flip both horizontally and vertically (rotate 180°).
    HvFlip = 1,
    /// Flip horizontally and translate 40px downward (enemy perspective).
    HFlip = 2,
    /// Flip base coordinates (mirror around screen center).
    CoordFlip = 3,
    /// Play frames in reverse order.
    Reverse = 4,
    /// Enemy-side variant: choose transform based on wPlayerMonIsAttacker.
    Enemy = 5,
}

impl SubAnimTransform {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Normal,
            1 => Self::HvFlip,
            2 => Self::HFlip,
            3 => Self::CoordFlip,
            4 => Self::Reverse,
            5 => Self::Enemy,
            _ => Self::Normal,
        }
    }
}

// ─── Frame block modes ───────────────────────────────────────────────

/// Controls delay and OAM buffer behavior after rendering a frame block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameBlockMode {
    /// Normal: delay, then clean OAM and reset dest address.
    Mode00 = 0,
    /// Unused in practice — same as Mode00.
    Mode01 = 1,
    /// No delay, keep OAM, advance dest address.
    Mode02 = 2,
    /// Delay, keep OAM, advance dest address.
    Mode03 = 3,
    /// Delay, keep OAM, do NOT advance dest address.
    Mode04 = 4,
}

impl FrameBlockMode {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Mode00,
            1 => Self::Mode01,
            2 => Self::Mode02,
            3 => Self::Mode03,
            4 => Self::Mode04,
            _ => Self::Mode00,
        }
    }

    /// Whether this mode delays (waits) after drawing.
    pub fn has_delay(&self) -> bool {
        matches!(
            self,
            Self::Mode00 | Self::Mode01 | Self::Mode03 | Self::Mode04
        )
    }

    /// Whether this mode cleans OAM after drawing.
    pub fn cleans_oam(&self) -> bool {
        matches!(self, Self::Mode00 | Self::Mode01)
    }

    /// Whether this mode advances the OAM destination pointer.
    pub fn advances_dest(&self) -> bool {
        matches!(self, Self::Mode02 | Self::Mode03)
    }
}

// ─── Animation type (screen-level effects) ───────────────────────────

/// Screen-level animation types from `AnimationTypePointerTable`.
/// These control per-subanimation-frame screen effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AnimationType {
    None = 0,
    ShakeScreenVertically = 1,
    ShakeScreenHorizontallyHeavy = 2,
    ShakeScreenHorizontallySlow = 3,
    BlinkEnemyMonSprite = 4,
    ShakeScreenHorizontallyLight = 5,
    ShakeScreenHorizontallySlow2 = 6,
}

impl AnimationType {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::ShakeScreenVertically,
            2 => Self::ShakeScreenHorizontallyHeavy,
            3 => Self::ShakeScreenHorizontallySlow,
            4 => Self::BlinkEnemyMonSprite,
            5 => Self::ShakeScreenHorizontallyLight,
            6 => Self::ShakeScreenHorizontallySlow2,
            _ => Self::None,
        }
    }
}

// ─── Special effects ─────────────────────────────────────────────────

/// Special effects that can appear in move animation command streams.
/// Values 0xD8..0xFE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SpecialEffect {
    WavyScreen = 0xD8,
    SubstituteMon = 0xD9,
    ShakeBackAndForth = 0xDA,
    SlideEnemyMonOff = 0xDB,
    ShowEnemyMonPic = 0xDC,
    ShowMonPic = 0xDD,
    BlinkEnemyMon = 0xDE,
    HideEnemyMonPic = 0xDF,
    FlashEnemyMonPic = 0xE0,
    DelayAnimation10 = 0xE1,
    SpiralBallsInward = 0xE2,
    ShakeEnemyHud2 = 0xE3,
    ShakeEnemyHud = 0xE4,
    SlideMonHalfOff = 0xE5,
    PetalsFalling = 0xE6,
    LeavesFalling = 0xE7,
    TransformMon = 0xE8,
    SlideMonDownAndHide = 0xE9,
    MinimizeMon = 0xEA,
    BounceUpAndDown = 0xEB,
    ShootManyBallsUpward = 0xEC,
    ShootBallsUpward = 0xED,
    SquishMonPic = 0xEE,
    HideMonPic = 0xEF,
    LightScreenPalette = 0xF0,
    ResetMonPosition = 0xF1,
    MoveMonHorizontally = 0xF2,
    BlinkMon = 0xF3,
    SlideMonOff = 0xF4,
    FlashMonPic = 0xF5,
    SlideMonDown = 0xF6,
    SlideMonUp = 0xF7,
    FlashScreenLong = 0xF8,
    DarkenMonPalette = 0xF9,
    WaterDropletsEverywhere = 0xFA,
    ShakeScreen = 0xFB,
    ResetScreenPalette = 0xFC,
    DarkScreenPalette = 0xFD,
    DarkScreenFlash = 0xFE,
}

impl SpecialEffect {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0xD8 => Some(Self::WavyScreen),
            0xD9 => Some(Self::SubstituteMon),
            0xDA => Some(Self::ShakeBackAndForth),
            0xDB => Some(Self::SlideEnemyMonOff),
            0xDC => Some(Self::ShowEnemyMonPic),
            0xDD => Some(Self::ShowMonPic),
            0xDE => Some(Self::BlinkEnemyMon),
            0xDF => Some(Self::HideEnemyMonPic),
            0xE0 => Some(Self::FlashEnemyMonPic),
            0xE1 => Some(Self::DelayAnimation10),
            0xE2 => Some(Self::SpiralBallsInward),
            0xE3 => Some(Self::ShakeEnemyHud2),
            0xE4 => Some(Self::ShakeEnemyHud),
            0xE5 => Some(Self::SlideMonHalfOff),
            0xE6 => Some(Self::PetalsFalling),
            0xE7 => Some(Self::LeavesFalling),
            0xE8 => Some(Self::TransformMon),
            0xE9 => Some(Self::SlideMonDownAndHide),
            0xEA => Some(Self::MinimizeMon),
            0xEB => Some(Self::BounceUpAndDown),
            0xEC => Some(Self::ShootManyBallsUpward),
            0xED => Some(Self::ShootBallsUpward),
            0xEE => Some(Self::SquishMonPic),
            0xEF => Some(Self::HideMonPic),
            0xF0 => Some(Self::LightScreenPalette),
            0xF1 => Some(Self::ResetMonPosition),
            0xF2 => Some(Self::MoveMonHorizontally),
            0xF3 => Some(Self::BlinkMon),
            0xF4 => Some(Self::SlideMonOff),
            0xF5 => Some(Self::FlashMonPic),
            0xF6 => Some(Self::SlideMonDown),
            0xF7 => Some(Self::SlideMonUp),
            0xF8 => Some(Self::FlashScreenLong),
            0xF9 => Some(Self::DarkenMonPalette),
            0xFA => Some(Self::WaterDropletsEverywhere),
            0xFB => Some(Self::ShakeScreen),
            0xFC => Some(Self::ResetScreenPalette),
            0xFD => Some(Self::DarkScreenPalette),
            0xFE => Some(Self::DarkScreenFlash),
            _ => None,
        }
    }
}

// ─── Data structures ─────────────────────────────────────────────────

/// A single OAM tile entry within a frame block.
/// Encodes position offset, tile index, and flip/attribute flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameBlockTile {
    /// X offset in pixels (dbsprite byte: x_tile*8 + x_px).
    pub x_px: u8,
    /// Y offset in pixels (dbsprite byte: y_tile*8 + y_px).
    pub y_px: u8,
    /// Tile ID (before adding ANIM_BASE_TILE_ID).
    pub tile_id: u8,
    /// OAM attribute flags (OAM_XFLIP, OAM_YFLIP, etc.).
    pub flags: u8,
}

/// A frame block — a group of OAM tiles composing one animation "sprite".
/// Frame blocks are indexed 0x00..0x79 (NUM_FRAMEBLOCKS).
#[derive(Debug, Clone)]
pub struct FrameBlock {
    pub tiles: Vec<FrameBlockTile>,
}

impl FrameBlock {
    pub const fn empty() -> Self {
        Self { tiles: Vec::new() }
    }
}

/// A single frame within a subanimation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubAnimFrame {
    /// Frame block index (0..NUM_FRAMEBLOCKS).
    pub frame_block_id: u8,
    /// Base coordinate index (0..NUM_BASECOORDS).
    pub base_coord_id: u8,
    /// Frame block rendering mode.
    pub mode: FrameBlockMode,
}

/// A subanimation definition — specifies transform type and a sequence of frames.
#[derive(Debug, Clone)]
pub struct SubAnimation {
    pub transform: SubAnimTransform,
    pub frames: Vec<SubAnimFrame>,
}

/// A single command in a move's animation sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimCommand {
    /// Play a subanimation with the given parameters.
    /// `sound_id` is the move sound index (0 = no sound, stored as sound-1 in ROM).
    /// `subanim_id` is the subanimation table index.
    /// `tileset` selects animation tileset 0 or 1.
    /// `delay` is the per-frame delay in vblank frames.
    SubAnim {
        sound_id: u8,
        subanim_id: u8,
        tileset: u8,
        delay: u8,
    },
    /// Execute a special effect (screen flash, mon slide, etc.).
    /// `sound_id` is the move sound index (0 = no sound).
    Effect { sound_id: u8, effect: SpecialEffect },
}

/// A complete move animation — a sequence of commands.
#[derive(Debug, Clone)]
pub struct MoveAnimation {
    pub commands: Vec<AnimCommand>,
}

impl MoveAnimation {
    pub const fn empty() -> Self {
        Self {
            commands: Vec::new(),
        }
    }
}

// ─── Per-animation-id frame hooks ────────────────────────────────────

/// Per-frame-block hook kinds. In the original engine a special effect is
/// applied after every frame block; the hook is selected by the animation id
/// and inspects the frame counter (which counts down from the subanimation's
/// frame count to 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameHook {
    /// Flash the screen after every frame block.
    FlashScreen,
    /// Flash when counter % 4 == 0 (hyper beam).
    FlashScreenEveryFour,
    /// Flash when counter % 8 == 0 (thunderbolt).
    FlashScreenEveryEight,
    /// Flash at counters 13, 9, 5 and 1.
    BlizzardFlash,
    /// Flash every 4 frame blocks; hide the attacking mon at counter 1.
    Explode,
    /// Shake the screen horizontally and vertically at counters 8..=11;
    /// flash at counter 1.
    RockSlide,
    /// Duplicate the first 4 OAM entries (second music note). Handled
    /// internally by the player; nothing emitted.
    Growl,
    /// Unreachable — Tail Whip's animation has no subanimations, so the
    /// hook never fires.
    TailWhipUnused,
    /// Ball toss flow — depends on battle state the player does not have
    /// (held item, in-battle flag).
    BallToss,
    /// Ball shake flow — restarts the subanimation a number of times, which
    /// is capture logic, not playback.
    BallShake,
    /// Plays a poof sound at counter 5; sound playback is a frontend
    /// concern, nothing visual is emitted.
    BallPoof,
    /// Trade sequence only, never used in battle.
    TradeHideMonster,
    /// Trade sequence only, never used in battle.
    TradeShakeBall,
    /// Trade sequence only, never used in battle.
    TradeJumpBall,
}

// ─── Shared move/animation ids ───────────────────────────────────────

// Moves do double duty as animation identifiers; animation ids are 1-based.

/// AMNESIA move/animation id ($85).
pub const AMNESIA: u8 = 0x85;
/// REST move/animation id ($9C).
pub const REST: u8 = 0x9C;
/// CONF_ANIM (confused monster) animation id ($BF).
pub const CONF_ANIM: u8 = 0xBF;
/// SLP_ANIM (sleeping monster) animation id ($BD).
pub const SLP_ANIM: u8 = 0xBD;

// ─── Animation tick result ───────────────────────────────────────────

/// Result of a single animation tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimTickResult {
    /// Animation is still playing — OAM entries ready to render.
    /// `sound`: move id whose sound this command plays (None = NO_MOVE);
    /// only set on the command's first frame. `hook`: per-frame effect
    /// from `AnimationIdSpecialEffects`, if one fired for this frame.
    Playing {
        sound: Option<u8>,
        hook: Option<AnimEffect>,
    },
    /// Waiting for a delay (number of frames remaining).
    /// `sound`: move id whose sound this command plays (None = NO_MOVE);
    /// only set on the command's first frame. `hook`: per-frame effect
    /// from `AnimationIdSpecialEffects`, if any.
    WaitDelay {
        frames: u8,
        sound: Option<u8>,
        hook: Option<AnimEffect>,
    },
    /// A special effect needs to be applied by the caller.
    /// `sound`: move id whose sound this command plays (None = NO_MOVE).
    Effect {
        sound: Option<u8>,
        effect: SpecialEffect,
    },
    /// The entire move animation is complete.
    Done,
}

/// High-level effect actions the caller should perform.
/// These map special effects to concrete rendering operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimEffect {
    /// No action needed (effect handled internally or is a no-op).
    None,
    /// Flash/invert the screen palette for N frames.
    FlashScreen { frames: u8 },
    /// Shake the screen horizontally by N pixels for M frames.
    ShakeScreenH { pixels: i8, frames: u8 },
    /// Shake the screen vertically by N pixels for M frames.
    ShakeScreenV { pixels: i8, frames: u8 },
    /// Shake the screen horizontally and vertically (DoRockSlideSpecialEffects).
    ShakeScreenHV { pixels: i8, frames: u8 },
    /// Hide the player's mon sprite.
    HidePlayerMon,
    /// Show the player's mon sprite.
    ShowPlayerMon,
    /// Hide the enemy's mon sprite.
    HideEnemyMon,
    /// Show the enemy's mon sprite.
    ShowEnemyMon,
    /// Slide the enemy mon off screen.
    SlideEnemyMonOff,
    /// Slide the player mon partially off.
    SlidePlayerMonHalfOff,
    /// Slide the player mon fully off the screen (AnimationSlideMonOff:
    /// slides 8 tiles, i.e. the whole 7-tile-wide pic, wSlideMonDelay = 3).
    SlidePlayerMonOff,
    /// Slide the player mon down and hide.
    SlidePlayerMonDown,
    /// Slide the mon down and hide with the two-step 7×5/7×3 row-shrink
    /// (`AnimationSlideMonDownAndHide`, Acid Armor) — distinct from
    /// `AnimationSlideMonDown`'s plain one-row-at-a-time slide.
    SlidePlayerMonDownAndHide,
    /// Slide the player mon up to original position.
    SlidePlayerMonUp,
    /// Reset the player mon to original position.
    ResetPlayerMonPosition,
    /// Move the player mon horizontally.
    MovePlayerMonH,
    /// Blink the enemy mon sprite.
    BlinkEnemyMon { times: u8 },
    /// Blink the player mon sprite.
    BlinkPlayerMon { times: u8 },
    /// Flash the enemy mon picture.
    FlashEnemyMonPic,
    /// Flash the player mon picture.
    FlashPlayerMonPic,
    /// Set screen to dark palette.
    DarkScreenPalette,
    /// Set screen to light palette.
    LightScreenPalette,
    /// Reset screen palette to normal.
    ResetScreenPalette,
    /// Darken the active mon's palette.
    DarkenMonPalette,
    /// Wavy screen distortion effect.
    WavyScreen,
    /// Substitute doll animation.
    SubstituteMon,
    /// Transform mon into another.
    TransformMon,
    /// Minimize mon (shrink).
    MinimizeMon,
    /// Bounce up and down.
    BounceUpAndDown,
    /// Squish the mon picture.
    SquishMonPic,
    /// Spiral balls inward.
    SpiralBallsInward,
    /// Shoot balls upward.
    ShootBallsUpward { many: bool },
    /// Petals falling.
    PetalsFalling,
    /// Leaves falling.
    LeavesFalling,
    /// Water droplets everywhere.
    WaterDroplets,
    /// Shake back and forth.
    ShakeBackAndForth,
    /// Shake enemy HUD.
    ShakeEnemyHud { variant: u8 },
    /// Delay for 10 frames.
    Delay10,
}
