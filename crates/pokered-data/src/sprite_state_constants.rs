// SpriteStateData1 field offsets (16 bytes per sprite, 16 sprites at $C100)
pub const SPRITESTATEDATA1_PICTUREID: usize = 0x00;
pub const SPRITESTATEDATA1_MOVEMENTSTATUS: usize = 0x01;
pub const SPRITESTATEDATA1_IMAGEINDEX: usize = 0x02;
pub const SPRITESTATEDATA1_YSTEPVECTOR: usize = 0x03;
pub const SPRITESTATEDATA1_YPIXELS: usize = 0x04;
pub const SPRITESTATEDATA1_XSTEPVECTOR: usize = 0x05;
pub const SPRITESTATEDATA1_XPIXELS: usize = 0x06;
pub const SPRITESTATEDATA1_INTRAANIMFRAMECOUNTER: usize = 0x07;
pub const SPRITESTATEDATA1_ANIMFRAMECOUNTER: usize = 0x08;
pub const SPRITESTATEDATA1_FACINGDIRECTION: usize = 0x09;
pub const SPRITESTATEDATA1_YADJUSTED: usize = 0x0A;
pub const SPRITESTATEDATA1_XADJUSTED: usize = 0x0B;
pub const SPRITESTATEDATA1_COLLISIONDATA: usize = 0x0C;
pub const SPRITESTATEDATA1_0D: usize = 0x0D;
pub const SPRITESTATEDATA1_0E: usize = 0x0E;
pub const SPRITESTATEDATA1_0F: usize = 0x0F;

// SpriteStateData2 field offsets (16 bytes per sprite, 16 sprites at $C200)
pub const SPRITESTATEDATA2_WALKANIMATIONCOUNTER: usize = 0x00;
pub const SPRITESTATEDATA2_01: usize = 0x01;
pub const SPRITESTATEDATA2_YDISPLACEMENT: usize = 0x02;
pub const SPRITESTATEDATA2_XDISPLACEMENT: usize = 0x03;
pub const SPRITESTATEDATA2_MAPY: usize = 0x04;
pub const SPRITESTATEDATA2_MAPX: usize = 0x05;
pub const SPRITESTATEDATA2_MOVEMENTBYTE1: usize = 0x06;
pub const SPRITESTATEDATA2_GRASSPRIORITY: usize = 0x07;
pub const SPRITESTATEDATA2_MOVEMENTDELAY: usize = 0x08;
pub const SPRITESTATEDATA2_ORIGFACINGDIRECTION: usize = 0x09;
pub const SPRITESTATEDATA2_0A: usize = 0x0A;
pub const SPRITESTATEDATA2_0B: usize = 0x0B;
pub const SPRITESTATEDATA2_0C: usize = 0x0C;
pub const SPRITESTATEDATA2_PICTUREID: usize = 0x0D;
pub const SPRITESTATEDATA2_IMAGEBASEOFFSET: usize = 0x0E;
pub const SPRITESTATEDATA2_0F: usize = 0x0F;

// Movement status values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MovementStatus {
    Uninitialized = 0,
    Ready = 1,
    Delayed = 2,
    Moving = 3,
}

impl MovementStatus {
    pub fn from_byte(b: u8) -> Self {
        match b & 0x7F {
            0 => Self::Uninitialized,
            1 => Self::Ready,
            2 => Self::Delayed,
            3 => Self::Moving,
            _ => Self::Uninitialized,
        }
    }

    pub fn face_player_bit(b: u8) -> bool {
        b & (1 << BIT_FACE_PLAYER) != 0
    }
}

pub const BIT_FACE_PLAYER: u8 = 7;

// Facing direction values (stored in SPRITESTATEDATA1_FACINGDIRECTION)
pub const FACING_DOWN: u8 = 0x00;
pub const FACING_UP: u8 = 0x04;
pub const FACING_LEFT: u8 = 0x08;
pub const FACING_RIGHT: u8 = 0x0C;

// Movement byte1 special values
pub const MOVEMENT_WALK: u8 = 0xFE;
pub const MOVEMENT_STAY: u8 = 0xFF;

// Movement direction constraints
pub const DIR_ANY: u8 = 0x00;
pub const DIR_UP_DOWN: u8 = 0x01;
pub const DIR_LEFT_RIGHT: u8 = 0x02;
pub const DIR_DOWN: u8 = 0xD0;
pub const DIR_UP: u8 = 0xD1;
pub const DIR_LEFT: u8 = 0xD2;
pub const DIR_RIGHT: u8 = 0xD3;
pub const DIR_NONE: u8 = 0xFF;

// People event type bits
pub const BIT_TRAINER: u8 = 6;
pub const BIT_ITEM: u8 = 7;
pub const TRAINER: u8 = 1 << BIT_TRAINER;
pub const ITEM: u8 = 1 << BIT_ITEM;

// Boulder movement
pub const BOULDER_MOVEMENT_BYTE_2: u8 = 0x10;

// Grass priority flag
pub const GRASS_PRIORITY: u8 = 0x80;

// Image index: 0xFF means offscreen/not visible
pub const IMAGE_INDEX_OFFSCREEN: u8 = 0xFF;
