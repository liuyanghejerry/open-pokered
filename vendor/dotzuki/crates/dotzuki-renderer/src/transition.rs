// Fade palette system — the classic BGP/OBP register fades.
// dc a,b,c,d = (a<<6)|(b<<4)|(c<<2)|d — packs 4 shades into one byte.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FadePalette {
    pub bgp: u8,
    pub obp0: u8,
    pub obp1: u8,
}

impl FadePalette {
    pub const fn new(bgp: u8, obp0: u8, obp1: u8) -> Self {
        Self { bgp, obp0, obp1 }
    }
}

// FadePal1 = all-black, FadePal4/5 = normal, FadePal8 = all-white
pub const FADE_PAL_1: FadePalette = FadePalette::new(0xFF, 0xFF, 0xFF); // dc 3,3,3,3 / dc 3,3,3,3 / dc 3,3,3,3
pub const FADE_PAL_2: FadePalette = FadePalette::new(0xFE, 0xFE, 0xF8); // dc 3,3,3,2 / dc 3,3,3,2 / dc 3,3,2,0
pub const FADE_PAL_3: FadePalette = FadePalette::new(0xF9, 0xE4, 0xE4); // dc 3,3,2,1 / dc 3,2,1,0 / dc 3,2,1,0
pub const FADE_PAL_4: FadePalette = FadePalette::new(0xE4, 0xD0, 0xE0); // dc 3,2,1,0 / dc 3,1,0,0 / dc 3,2,0,0 (normal)
pub const FADE_PAL_5: FadePalette = FadePalette::new(0xE4, 0xD0, 0xE0); // same as FadePal4
pub const FADE_PAL_6: FadePalette = FadePalette::new(0x90, 0x80, 0x90); // dc 2,1,0,0 / dc 2,0,0,0 / dc 2,1,0,0
pub const FADE_PAL_7: FadePalette = FadePalette::new(0x40, 0x40, 0x40); // dc 1,0,0,0 / dc 1,0,0,0 / dc 1,0,0,0
pub const FADE_PAL_8: FadePalette = FadePalette::new(0x00, 0x00, 0x00); // dc 0,0,0,0 / dc 0,0,0,0 / dc 0,0,0,0 (all-white)

pub const FADE_PALETTES: [FadePalette; 8] = [
    FADE_PAL_1, FADE_PAL_2, FADE_PAL_3, FADE_PAL_4, FADE_PAL_5, FADE_PAL_6, FADE_PAL_7, FADE_PAL_8,
];

pub const FADE_DELAY_FRAMES: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeDirection {
    InFromBlack, // GBFadeInFromBlack: FadePal1→4 (4 steps)
    OutToBlack,  // GBFadeOutToBlack: FadePal4→1 (4 steps)
    OutToWhite,  // GBFadeOutToWhite: FadePal6→8 (3 steps)
    InFromWhite, // GBFadeInFromWhite: FadePal7→5 (3 steps)
}

impl FadeDirection {
    pub fn palette_indices(&self) -> &'static [usize] {
        match self {
            FadeDirection::InFromBlack => &[0, 1, 2, 3],
            FadeDirection::OutToBlack => &[3, 2, 1, 0],
            FadeDirection::OutToWhite => &[5, 6, 7],
            FadeDirection::InFromWhite => &[6, 5, 4],
        }
    }

    pub fn step_count(&self) -> usize {
        self.palette_indices().len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstantPalette {
    Normal,   // GBPalNormal: BGP=0xE4, OBP0=0xD0
    WhiteOut, // GBPalWhiteOut: all zeros
}

impl InstantPalette {
    pub fn palette(&self) -> FadePalette {
        match self {
            InstantPalette::Normal => FadePalette::new(0xE4, 0xD0, 0xE4),
            InstantPalette::WhiteOut => FadePalette::new(0x00, 0x00, 0x00),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeState {
    Idle,
    Fading { step: usize, delay_remaining: u8 },
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeTickResult {
    Fading(FadePalette),
    Done(FadePalette),
    Idle,
}

#[derive(Debug, Clone)]
pub struct FadeTransition {
    direction: Option<FadeDirection>,
    state: FadeState,
}

impl FadeTransition {
    pub fn new() -> Self {
        Self {
            direction: None,
            state: FadeState::Idle,
        }
    }

    pub fn start(&mut self, direction: FadeDirection) {
        self.direction = Some(direction);
        self.state = FadeState::Fading {
            step: 0,
            delay_remaining: FADE_DELAY_FRAMES,
        };
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state, FadeState::Fading { .. })
    }

    pub fn is_done(&self) -> bool {
        matches!(self.state, FadeState::Done)
    }

    pub fn state(&self) -> FadeState {
        self.state
    }

    pub fn current_palette(&self) -> Option<FadePalette> {
        let dir = self.direction.as_ref()?;
        let indices = dir.palette_indices();
        match self.state {
            FadeState::Fading { step, .. } => {
                if step < indices.len() {
                    Some(FADE_PALETTES[indices[step]])
                } else {
                    Some(FADE_PALETTES[*indices.last().unwrap()])
                }
            }
            FadeState::Done => Some(FADE_PALETTES[*indices.last().unwrap()]),
            FadeState::Idle => None,
        }
    }

    pub fn tick(&mut self) -> FadeTickResult {
        let dir = match &self.direction {
            Some(d) => *d,
            None => return FadeTickResult::Idle,
        };

        match self.state {
            FadeState::Idle => FadeTickResult::Idle,
            FadeState::Done => {
                self.state = FadeState::Idle;
                self.direction = None;
                FadeTickResult::Idle
            }
            FadeState::Fading {
                step,
                delay_remaining,
            } => {
                let indices = dir.palette_indices();
                let palette = FADE_PALETTES[indices[step]];

                if delay_remaining > 1 {
                    self.state = FadeState::Fading {
                        step,
                        delay_remaining: delay_remaining - 1,
                    };
                    FadeTickResult::Fading(palette)
                } else {
                    let next_step = step + 1;
                    if next_step >= indices.len() {
                        self.state = FadeState::Done;
                        FadeTickResult::Done(palette)
                    } else {
                        self.state = FadeState::Fading {
                            step: next_step,
                            delay_remaining: FADE_DELAY_FRAMES,
                        };
                        FadeTickResult::Fading(FADE_PALETTES[indices[next_step]])
                    }
                }
            }
        }
    }

    pub fn cancel(&mut self) {
        self.state = FadeState::Idle;
        self.direction = None;
    }
}

impl Default for FadeTransition {
    fn default() -> Self {
        Self::new()
    }
}

// LoadGBPal: palette based on wMapPalOffset (dark caves shift toward FadePal1).
// Offset is in bytes (0,3,6,9); each 3-byte step = 1 palette index back from FadePal4.
pub fn load_gb_pal(map_pal_offset: u8) -> FadePalette {
    let steps_back = (map_pal_offset / 3) as usize;
    let index = 3usize.saturating_sub(steps_back);
    FADE_PALETTES[index]
}

// GetHealthBarColor: >=27px → 0 (green), >=10px → 1 (yellow), <10px → 2 (red)
pub fn get_health_bar_color(hp_pixels: u8) -> u8 {
    if hp_pixels >= 27 {
        0
    } else if hp_pixels >= 10 {
        1
    } else {
        2
    }
}
