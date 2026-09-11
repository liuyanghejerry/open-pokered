use core::marker::PhantomData;

use dotzuki_engine::palette::{PaletteProvider, PaletteTrait, SgbColor, SgbPaletteEntry};
use dotzuki_engine::render::Rgba;

// ============================================================================
// ColorIndex trait
// ============================================================================

/// Trait for color index types that can be used with [`Palette`].
///
/// Implementations define the maximum number of colors and provide
/// conversion from raw byte values to typed indices.
pub trait ColorIndex: Copy + Clone + core::fmt::Debug + PartialEq + Eq {
    /// Maximum number of colors in a palette using this index type.
    const MAX: usize;

    /// Create a color index from a raw byte, applying the appropriate mask.
    fn from_u8(val: u8) -> Self;

    /// Convert this index to a `usize` for array lookup.
    fn to_index(self) -> usize;
}

// ============================================================================
// GbColor — 2-bit Game Boy color index (4 colors)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GbColor {
    White = 0,
    LightGray = 1,
    DarkGray = 2,
    Black = 3,
}

impl ColorIndex for GbColor {
    const MAX: usize = 4;

    fn from_u8(val: u8) -> Self {
        match val & 0x03 {
            0 => GbColor::White,
            1 => GbColor::LightGray,
            2 => GbColor::DarkGray,
            _ => GbColor::Black,
        }
    }

    fn to_index(self) -> usize {
        self as usize
    }
}

impl GbColor {
    pub const ALL: [GbColor; 4] = [
        GbColor::White,
        GbColor::LightGray,
        GbColor::DarkGray,
        GbColor::Black,
    ];

    pub fn from_u8(val: u8) -> Self {
        <Self as ColorIndex>::from_u8(val)
    }
}

// ============================================================================
// GbaColor — 4-bit GBA color index (16 colors)
// ============================================================================

/// A 4-bit color index for GBA-style palettes (up to 16 colors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GbaColor(pub u8);

impl ColorIndex for GbaColor {
    const MAX: usize = 16;

    fn from_u8(val: u8) -> Self {
        GbaColor(val & 0x0F)
    }

    fn to_index(self) -> usize {
        self.0 as usize
    }
}

// ============================================================================
// Palette — generic over color index type
// ============================================================================

/// A color palette mapping color indices to RGBA values.
///
/// The type parameter `C` controls the maximum number of colors.
/// - `Palette` (defaults to `Palette<GbColor>`) — 4-color DMG palette.
/// - `Palette<GbaColor>` — 16-color GBA palette.
///
/// The internal storage is always `[Rgba; 16]` with a `count` field,
/// so const construction works for both 4- and 16-color palettes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette<C: ColorIndex = GbColor> {
    /// Color data. Only the first `count` entries are meaningful.
    pub colors: [Rgba; 16],
    /// Number of valid colors (1–16).
    pub count: u8,
    #[doc(hidden)]
    pub _phantom: PhantomData<C>,
}

impl<C: ColorIndex> Palette<C> {
    /// Create a palette from a slice of RGBA colors.
    ///
    /// Colors beyond index 15 are silently ignored.
    pub fn new(colors: &[Rgba]) -> Self {
        let mut arr = [Rgba::BLACK; 16];
        let count = colors.len().min(16);
        arr[..count].copy_from_slice(&colors[..count]);
        Self {
            colors: arr,
            count: count as u8,
            _phantom: PhantomData,
        }
    }

    /// Look up the RGBA color for a given color index.
    pub fn color(&self, index: C) -> Rgba {
        self.colors[index.to_index()]
    }
}

impl Palette<GbColor> {
    /// Create a GB palette from a BGP/OBP register value and a base palette.
    ///
    /// Each 2-bit field of the register selects a color from the base palette,
    /// effectively remapping the 4 palette entries.
    pub fn from_bgp_register(bgp: u8, base_palette: &Self) -> Self {
        let mut arr = [Rgba::BLACK; 16];
        for i in 0..4 {
            let shade = (bgp >> (i * 2)) & 0x03;
            arr[i] = base_palette.colors[shade as usize];
        }
        Self {
            colors: arr,
            count: 4,
            _phantom: PhantomData,
        }
    }
}

impl Palette<GbaColor> {
    /// Create a GBA palette from a full 16-color array.
    pub fn from_gba_palette(colors: [Rgba; 16]) -> Self {
        Self {
            colors,
            count: 16,
            _phantom: PhantomData,
        }
    }
}

// ============================================================================
// Built-in palettes (all Palette = Palette<GbColor>)
// ============================================================================

pub const DMG_PALETTE: Palette = Palette {
    colors: [
        Rgba::rgb(0x9B, 0xBC, 0x0F), // White (lightest green)
        Rgba::rgb(0x8B, 0xAC, 0x0F), // Light gray
        Rgba::rgb(0x30, 0x62, 0x30), // Dark gray
        Rgba::rgb(0x0F, 0x38, 0x0F), // Black (darkest green)
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
    ],
    count: 4,
    _phantom: PhantomData,
};

pub const GRAYSCALE_PALETTE: Palette = Palette {
    colors: [
        Rgba::rgb(0xFF, 0xFF, 0xFF),
        Rgba::rgb(0xAA, 0xAA, 0xAA),
        Rgba::rgb(0x55, 0x55, 0x55),
        Rgba::rgb(0x00, 0x00, 0x00),
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
    ],
    count: 4,
    _phantom: PhantomData,
};

pub const GRAYSCALE_SPRITE_PALETTE: Palette = Palette {
    colors: [
        Rgba::TRANSPARENT,
        Rgba::rgb(0xAA, 0xAA, 0xAA),
        Rgba::rgb(0x55, 0x55, 0x55),
        Rgba::rgb(0x00, 0x00, 0x00),
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
    ],
    count: 4,
    _phantom: PhantomData,
};

pub const POCKET_SPRITE_PALETTE: Palette = Palette {
    colors: [
        Rgba::TRANSPARENT,
        Rgba::rgb(0x8B, 0x95, 0x6D),
        Rgba::rgb(0x4D, 0x53, 0x3C),
        Rgba::rgb(0x1F, 0x1F, 0x1F),
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
    ],
    count: 4,
    _phantom: PhantomData,
};

pub const POCKET_PALETTE: Palette = Palette {
    colors: [
        Rgba::rgb(0xC4, 0xCF, 0xA1), // lightest
        Rgba::rgb(0x8B, 0x95, 0x6D), // light
        Rgba::rgb(0x4D, 0x53, 0x3C), // dark
        Rgba::rgb(0x1F, 0x1F, 0x1F), // darkest
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
    ],
    count: 4,
    _phantom: PhantomData,
};

pub const HP_BAR_GREEN_PALETTE: Palette = Palette {
    colors: [
        Rgba::rgb(0xFF, 0xFF, 0xFF),
        Rgba::rgb(0xAA, 0xAA, 0xAA),
        Rgba::rgb(0x00, 0xC8, 0x00),
        Rgba::rgb(0x00, 0x00, 0x00),
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
    ],
    count: 4,
    _phantom: PhantomData,
};

pub const HP_BAR_YELLOW_PALETTE: Palette = Palette {
    colors: [
        Rgba::rgb(0xFF, 0xFF, 0xFF),
        Rgba::rgb(0xAA, 0xAA, 0xAA),
        Rgba::rgb(0xE8, 0xA8, 0x00),
        Rgba::rgb(0x00, 0x00, 0x00),
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
    ],
    count: 4,
    _phantom: PhantomData,
};

pub const HP_BAR_RED_PALETTE: Palette = Palette {
    colors: [
        Rgba::rgb(0xFF, 0xFF, 0xFF),
        Rgba::rgb(0xAA, 0xAA, 0xAA),
        Rgba::rgb(0xD8, 0x20, 0x00),
        Rgba::rgb(0x00, 0x00, 0x00),
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
        Rgba::BLACK,
    ],
    count: 4,
    _phantom: PhantomData,
};

pub const DEFAULT_BGP: u8 = 0b11100100; // shade 3,2,1,0 (normal)
pub const DEFAULT_OBP0: u8 = 0b11010000; // shade 3,1,0,0 (sprite palette 0, color 0 = transparent)
pub const DEFAULT_OBP1: u8 = 0b11100100; // shade 3,2,1,0

// ============================================================================
// PaletteState
// ============================================================================

#[derive(Debug, Clone)]
pub struct PaletteState {
    pub base: Palette,
    pub bgp: u8,
    pub obp0: u8,
    pub obp1: u8,
}

impl PaletteState {
    pub fn new(base: Palette) -> Self {
        Self {
            base,
            bgp: DEFAULT_BGP,
            obp0: DEFAULT_OBP0,
            obp1: DEFAULT_OBP1,
        }
    }

    pub fn bg_palette(&self) -> Palette {
        Palette::from_bgp_register(self.bgp, &self.base)
    }

    pub fn obj_palette0(&self) -> Palette {
        let mut pal = Palette::from_bgp_register(self.obp0, &self.base);
        pal.colors[0] = Rgba::TRANSPARENT;
        pal
    }

    pub fn obj_palette1(&self) -> Palette {
        let mut pal = Palette::from_bgp_register(self.obp1, &self.base);
        pal.colors[0] = Rgba::TRANSPARENT;
        pal
    }

    pub fn white_out(&mut self) {
        self.bgp = 0x00;
        self.obp0 = 0x00;
        self.obp1 = 0x00;
    }

    pub fn reset_normal(&mut self) {
        self.bgp = DEFAULT_BGP;
        self.obp0 = DEFAULT_OBP0;
    }
}

impl Default for PaletteState {
    fn default() -> Self {
        Self::new(DMG_PALETTE)
    }
}

// ============================================================================
// SGB palette slot table — default palette IDs for classic GB-style games
// ============================================================================

/// SGB palette IDs — indexes into the game's SuperPalettes.
///
/// A convenience default palette-slot table in the style of classic Game Boy
/// JRPGs (route/town/logo/monster/bar slots). Games are free to define their
/// own palette ID type and use [`ColorPaletteState`] generically instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SgbPaletteId {
    Route = 0x00,
    HomeTown = 0x01,
    Viridian = 0x02,
    Pewter = 0x03,
    Cerulean = 0x04,
    Lavender = 0x05,
    Vermilion = 0x06,
    Celadon = 0x07,
    Fuchsia = 0x08,
    Cinnabar = 0x09,
    Indigo = 0x0A,
    Saffron = 0x0B,
    TownMap = 0x0C,
    Logo1 = 0x0D,
    Logo2 = 0x0E,
    Pal0F = 0x0F,
    PaleMon = 0x10,
    BlueMon = 0x11,
    RedMon = 0x12,
    CyanMon = 0x13,
    PurpleMon = 0x14,
    BrownMon = 0x15,
    GreenMon = 0x16,
    PinkMon = 0x17,
    YellowMon = 0x18,
    GrayMon = 0x19,
    Slots1 = 0x1A,
    Slots2 = 0x1B,
    Slots3 = 0x1C,
    Slots4 = 0x1D,
    Black = 0x1E,
    GreenBar = 0x1F,
    YellowBar = 0x20,
    RedBar = 0x21,
    Badge = 0x22,
    Cave = 0x23,
    CompanyLogo = 0x24,
}

impl PaletteTrait for SgbPaletteId {}

/// Total number of SGB palettes (NUM_SGB_PALS = 0x25 = 37).
pub const NUM_SGB_PALS: usize = 0x25;

impl SgbPaletteId {
    pub fn from_u8(val: u8) -> Option<Self> {
        if val < NUM_SGB_PALS as u8 {
            Some(unsafe { core::mem::transmute(val) })
        } else {
            None
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Palette command IDs dispatched during palette transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SetPalCommand {
    BattleBlack = 0x00,
    Battle = 0x01,
    TownMap = 0x02,
    StatusScreen = 0x03,
    Dex = 0x04,
    Slots = 0x05,
    TitleScreen = 0x06,
    MonsterIntro = 0x07,
    Generic = 0x08,
    Overworld = 0x09,
    PartyMenu = 0x0A,
    WholeScreen = 0x0B,
    CompanyLogoIntro = 0x0C,
    TrainerCard = 0x0D,
}

/// Special command: update HP bar colors in party menu BLK packet.
pub const SET_PAL_PARTY_MENU_HP_BARS: u8 = 0xFC;
/// Special command: use the stored default palette command instead.
pub const SET_PAL_DEFAULT: u8 = 0xFF;

impl SetPalCommand {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x00 => Some(Self::BattleBlack),
            0x01 => Some(Self::Battle),
            0x02 => Some(Self::TownMap),
            0x03 => Some(Self::StatusScreen),
            0x04 => Some(Self::Dex),
            0x05 => Some(Self::Slots),
            0x06 => Some(Self::TitleScreen),
            0x07 => Some(Self::MonsterIntro),
            0x08 => Some(Self::Generic),
            0x09 => Some(Self::Overworld),
            0x0A => Some(Self::PartyMenu),
            0x0B => Some(Self::WholeScreen),
            0x0C => Some(Self::CompanyLogoIntro),
            0x0D => Some(Self::TrainerCard),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

// ============================================================================
// SGB → Renderer Bridge Functions
// ============================================================================

/// Convert 5-bit SGB color to 8-bit RGBA.
/// Uses the standard conversion: `(val << 3) | (val >> 2)`.
pub fn sgb_color_to_rgba(color: &SgbColor) -> Rgba {
    let r8 = (color.r << 3) | (color.r >> 2);
    let g8 = (color.g << 3) | (color.g >> 2);
    let b8 = (color.b << 3) | (color.b >> 2);
    Rgba::rgb(r8, g8, b8)
}

/// Convert an SgbPaletteEntry (4 SgbColors) into a Palette (4 Rgba colors).
pub fn sgb_entry_to_palette(entry: &SgbPaletteEntry) -> Palette {
    Palette::new(&[
        sgb_color_to_rgba(&entry[0]),
        sgb_color_to_rgba(&entry[1]),
        sgb_color_to_rgba(&entry[2]),
        sgb_color_to_rgba(&entry[3]),
    ])
}

// ============================================================================
// Rendering Palette Mode
// ============================================================================

/// The rendering palette mode.
/// - `Dmg`: Original Game Boy 4-shade grayscale (green-tinted).
/// - `Sgb`: Super Game Boy colorized mode (uses SuperPalettes).
/// - `Grayscale`: Clean grayscale (no green tint).
/// - `Pocket`: Game Boy Pocket palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteMode {
    Dmg,
    Sgb,
    Grayscale,
    Pocket,
}

/// Extended palette state that supports both DMG (4-shade) and SGB (colorized) rendering.
///
/// Generic over the game's palette ID type `P` (defaults to the classic
/// [`SgbPaletteId`] slot table). When in DMG/Grayscale/Pocket mode, uses
/// `PaletteState` (bgp/obp0/obp1 registers). When in SGB mode, overlays SGB
/// color palettes on top of the DMG register state.
pub struct ColorPaletteState<'a, P: PaletteTrait = SgbPaletteId> {
    /// The base DMG palette register state (always maintained).
    pub dmg: PaletteState,
    /// Current palette mode.
    pub mode: PaletteMode,
    /// Whether this is the Red (true) or Blue (false) palette set.
    pub is_red: bool,
    /// Palette provider for SGB palette ID lookups.
    pub palette_provider: &'a dyn PaletteProvider<P>,
    /// Current SGB palette for background (active when mode == Sgb).
    pub sgb_bg_palette: P,
    /// Current SGB palette for OBJ0 (sprites).
    pub sgb_obj0_palette: P,
    /// Current SGB palette for OBJ1 (sprites).
    pub sgb_obj1_palette: P,
    /// The default palette command.
    pub default_command: SetPalCommand,
}

impl<'a, P: PaletteTrait> ColorPaletteState<'a, P> {
    pub fn new(
        mode: PaletteMode,
        is_red: bool,
        palette_provider: &'a dyn PaletteProvider<P>,
        bg: P,
        obj0: P,
        obj1: P,
    ) -> Self {
        Self {
            dmg: PaletteState::default(),
            mode,
            is_red,
            palette_provider,
            sgb_bg_palette: bg,
            sgb_obj0_palette: obj0,
            sgb_obj1_palette: obj1,
            default_command: SetPalCommand::Generic,
        }
    }

    /// Get the effective background palette for rendering.
    pub fn bg_palette(&self) -> Palette {
        match self.mode {
            PaletteMode::Sgb => sgb_entry_to_palette(
                &self
                    .palette_provider
                    .sgb_palette_data(self.sgb_bg_palette, self.is_red),
            ),
            PaletteMode::Dmg => self.dmg.bg_palette(),
            PaletteMode::Grayscale => {
                let mut state = self.dmg.clone();
                state.base = GRAYSCALE_PALETTE;
                state.bg_palette()
            }
            PaletteMode::Pocket => {
                let mut state = self.dmg.clone();
                state.base = POCKET_PALETTE;
                state.bg_palette()
            }
        }
    }

    /// Get the effective OBJ palette 0 for rendering.
    pub fn obj_palette0(&self) -> Palette {
        match self.mode {
            PaletteMode::Sgb => sgb_entry_to_palette(
                &self
                    .palette_provider
                    .sgb_palette_data(self.sgb_obj0_palette, self.is_red),
            ),
            PaletteMode::Dmg => self.dmg.obj_palette0(),
            PaletteMode::Grayscale => {
                let mut state = self.dmg.clone();
                state.base = GRAYSCALE_PALETTE;
                state.obj_palette0()
            }
            PaletteMode::Pocket => {
                let mut state = self.dmg.clone();
                state.base = POCKET_PALETTE;
                state.obj_palette0()
            }
        }
    }

    /// Get the effective OBJ palette 1 for rendering.
    pub fn obj_palette1(&self) -> Palette {
        match self.mode {
            PaletteMode::Sgb => sgb_entry_to_palette(
                &self
                    .palette_provider
                    .sgb_palette_data(self.sgb_obj1_palette, self.is_red),
            ),
            PaletteMode::Dmg => self.dmg.obj_palette1(),
            PaletteMode::Grayscale => {
                let mut state = self.dmg.clone();
                state.base = GRAYSCALE_PALETTE;
                state.obj_palette1()
            }
            PaletteMode::Pocket => {
                let mut state = self.dmg.clone();
                state.base = POCKET_PALETTE;
                state.obj_palette1()
            }
        }
    }

    /// Set the overworld palette based on current map context.
    pub fn set_overworld_palette(&mut self, tileset: u8, map_id: u8, last_map: u8) {
        let pal = self
            .palette_provider
            .overworld_palette_for(tileset, map_id, last_map);
        self.sgb_bg_palette = pal;
        self.default_command = SetPalCommand::Overworld;
    }

    /// Set battle palettes from the two battlers' palettes and the player HP bar color.
    ///
    /// The caller supplies the per-battler palette IDs (including any
    /// transformed/fainted fallbacks — the renderer does not know a game's
    /// palette semantics).
    pub fn set_battle_palette(&mut self, player_pal: P, enemy_pal: P, player_hp_bar_color: u8) {
        // In battle, BG palette areas are:
        // - Player HP bar → GreenBar/YellowBar/RedBar based on HP
        // - Enemy HP bar → GreenBar/YellowBar/RedBar based on HP
        // - Player mon → species palette
        // - Enemy mon → species palette
        // For simplicity, we store the main bg as player HP bar, obj0 as player mon, obj1 as enemy mon.
        self.sgb_bg_palette = self
            .palette_provider
            .hp_bar_to_palette_id(player_hp_bar_color);
        self.sgb_obj0_palette = player_pal;
        self.sgb_obj1_palette = enemy_pal;
        self.default_command = SetPalCommand::Battle;
    }
}

impl<'a> ColorPaletteState<'a, SgbPaletteId> {
    /// Convenience constructor using the classic slot table, defaulting all
    /// three palette slots to [`SgbPaletteId::Route`].
    pub fn new_classic(
        mode: PaletteMode,
        is_red: bool,
        palette_provider: &'a dyn PaletteProvider<SgbPaletteId>,
    ) -> Self {
        Self::new(
            mode,
            is_red,
            palette_provider,
            SgbPaletteId::Route,
            SgbPaletteId::Route,
            SgbPaletteId::Route,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gb_color_from_u8() {
        assert_eq!(GbColor::from_u8(0), GbColor::White);
        assert_eq!(GbColor::from_u8(1), GbColor::LightGray);
        assert_eq!(GbColor::from_u8(2), GbColor::DarkGray);
        assert_eq!(GbColor::from_u8(3), GbColor::Black);
        // Masking
        assert_eq!(GbColor::from_u8(4), GbColor::White);
        assert_eq!(GbColor::from_u8(7), GbColor::Black);
    }

    #[test]
    fn gb_color_to_index() {
        assert_eq!(GbColor::White.to_index(), 0);
        assert_eq!(GbColor::LightGray.to_index(), 1);
        assert_eq!(GbColor::DarkGray.to_index(), 2);
        assert_eq!(GbColor::Black.to_index(), 3);
    }

    #[test]
    fn gba_color_from_u8() {
        assert_eq!(GbaColor::from_u8(0).0, 0);
        assert_eq!(GbaColor::from_u8(15).0, 15);
        // Masking to 0x0F
        assert_eq!(GbaColor::from_u8(16).0, 0);
        assert_eq!(GbaColor::from_u8(31).0, 15);
        assert_eq!(GbaColor::from_u8(0xFF).0, 15);
    }

    #[test]
    fn gba_color_to_index() {
        assert_eq!(GbaColor(0).to_index(), 0);
        assert_eq!(GbaColor(7).to_index(), 7);
        assert_eq!(GbaColor(15).to_index(), 15);
    }

    #[test]
    fn palette_color_lookup() {
        let pal = DMG_PALETTE;
        assert_eq!(pal.color(GbColor::White), Rgba::rgb(0x9B, 0xBC, 0x0F));
        assert_eq!(pal.color(GbColor::Black), Rgba::rgb(0x0F, 0x38, 0x0F));
    }

    #[test]
    fn palette_new_from_slice() {
        let colors = [
            Rgba::rgb(0xFF, 0x00, 0x00),
            Rgba::rgb(0x00, 0xFF, 0x00),
            Rgba::rgb(0x00, 0x00, 0xFF),
            Rgba::TRANSPARENT,
        ];
        let pal: Palette = Palette::new(&colors);
        assert_eq!(pal.count, 4);
        assert_eq!(pal.color(GbColor::White), Rgba::rgb(0xFF, 0x00, 0x00));
        assert_eq!(pal.color(GbColor::LightGray), Rgba::rgb(0x00, 0xFF, 0x00));
        assert_eq!(pal.color(GbColor::DarkGray), Rgba::rgb(0x00, 0x00, 0xFF));
        assert_eq!(pal.color(GbColor::Black), Rgba::TRANSPARENT);
    }

    #[test]
    fn palette_from_bgp_register() {
        let base = GRAYSCALE_PALETTE;
        // Default BGP: shade 3,2,1,0 → identity mapping
        let pal = Palette::from_bgp_register(DEFAULT_BGP, &base);
        assert_eq!(pal.color(GbColor::White), Rgba::rgb(0xFF, 0xFF, 0xFF));
        assert_eq!(pal.color(GbColor::LightGray), Rgba::rgb(0xAA, 0xAA, 0xAA));
        assert_eq!(pal.color(GbColor::DarkGray), Rgba::rgb(0x55, 0x55, 0x55));
        assert_eq!(pal.color(GbColor::Black), Rgba::rgb(0x00, 0x00, 0x00));

        // All white (bgp=0x00)
        let pal = Palette::from_bgp_register(0x00, &base);
        assert_eq!(pal.color(GbColor::White), Rgba::rgb(0xFF, 0xFF, 0xFF));
        assert_eq!(pal.color(GbColor::Black), Rgba::rgb(0xFF, 0xFF, 0xFF));
    }

    #[test]
    fn gba_palette_from_array() {
        let mut colors = [Rgba::BLACK; 16];
        colors[0] = Rgba::rgb(0xFF, 0x00, 0x00);
        colors[15] = Rgba::rgb(0x00, 0x00, 0xFF);
        let pal = Palette::<GbaColor>::from_gba_palette(colors);
        assert_eq!(pal.count, 16);
        assert_eq!(pal.color(GbaColor(0)), Rgba::rgb(0xFF, 0x00, 0x00));
        assert_eq!(pal.color(GbaColor(15)), Rgba::rgb(0x00, 0x00, 0xFF));
    }

    #[test]
    fn palette_state_defaults() {
        let state = PaletteState::default();
        assert_eq!(state.bgp, DEFAULT_BGP);
        assert_eq!(state.obp0, DEFAULT_OBP0);
        assert_eq!(state.obp1, DEFAULT_OBP1);
    }

    #[test]
    fn palette_state_white_out() {
        let mut state = PaletteState::default();
        state.white_out();
        assert_eq!(state.bgp, 0x00);
        assert_eq!(state.obp0, 0x00);
    }

    #[test]
    fn dmg_palette_const_is_valid() {
        assert_eq!(DMG_PALETTE.count, 4);
        assert_eq!(
            DMG_PALETTE.color(GbColor::White),
            Rgba::rgb(0x9B, 0xBC, 0x0F)
        );
    }

    #[test]
    fn grayscale_palette_const_is_valid() {
        assert_eq!(GRAYSCALE_PALETTE.count, 4);
        assert_eq!(
            GRAYSCALE_PALETTE.color(GbColor::White),
            Rgba::rgb(0xFF, 0xFF, 0xFF)
        );
    }

    #[test]
    fn palette_copy_and_eq() {
        let a = DMG_PALETTE;
        let b = a; // Copy
        assert_eq!(a, b);
    }

    #[test]
    fn color_palette_state_is_generic_over_custom_palette() {
        // A game can drive the SGB state with its own palette ID type —
        // no dependency on the renderer's default SgbPaletteId table.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        enum TestPalette {
            Field,
            HpBar,
        }
        impl PaletteTrait for TestPalette {}

        struct TestProvider;
        impl PaletteProvider<TestPalette> for TestProvider {
            fn bg_palette(&self, _: TestPalette) -> [u8; 4] {
                [0; 4]
            }
            fn obj_palette0(&self, _: TestPalette) -> [u8; 4] {
                [0; 4]
            }
            fn obj_palette1(&self, _: TestPalette) -> [u8; 4] {
                [0; 4]
            }
            fn overworld_palette_for(&self, _: u8, _: u8, _: u8) -> TestPalette {
                TestPalette::Field
            }
            fn monster_palette(&self, _: u8) -> TestPalette {
                TestPalette::Field
            }
            fn hp_bar_to_palette_id(&self, _: u8) -> TestPalette {
                TestPalette::HpBar
            }
        }

        let mut state = ColorPaletteState::new(
            PaletteMode::Sgb,
            true,
            &TestProvider,
            TestPalette::Field,
            TestPalette::Field,
            TestPalette::Field,
        );
        state.set_overworld_palette(1, 2, 0);
        assert_eq!(state.sgb_bg_palette, TestPalette::Field);
        assert_eq!(state.default_command, SetPalCommand::Overworld);

        state.set_battle_palette(TestPalette::Field, TestPalette::Field, 1);
        assert_eq!(state.sgb_bg_palette, TestPalette::HpBar);
        assert_eq!(state.default_command, SetPalCommand::Battle);

        // The default `sgb_palette_data` is black → SGB bg renders as black.
        let pal = state.bg_palette();
        assert_eq!(pal.color(GbColor::White), Rgba::rgb(0, 0, 0));
    }
}
