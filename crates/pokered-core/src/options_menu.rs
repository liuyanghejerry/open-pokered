use serde::{Deserialize, Serialize};

pub const TEXT_DELAY_FAST: u8 = 1;
pub const TEXT_DELAY_MEDIUM: u8 = 3;
pub const TEXT_DELAY_SLOW: u8 = 5;
pub const TEXT_DELAY_MASK: u8 = 0b111;
pub const BIT_BATTLE_SHIFT: u8 = 6;
pub const BIT_BATTLE_ANIMATION: u8 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextSpeed {
    Fast,
    Medium,
    Slow,
}

impl TextSpeed {
    pub fn delay_frames(self) -> u8 {
        match self {
            Self::Fast => TEXT_DELAY_FAST,
            Self::Medium => TEXT_DELAY_MEDIUM,
            Self::Slow => TEXT_DELAY_SLOW,
        }
    }

    pub fn from_delay(delay: u8) -> Self {
        match delay & TEXT_DELAY_MASK {
            TEXT_DELAY_FAST => Self::Fast,
            TEXT_DELAY_SLOW => Self::Slow,
            _ => Self::Medium,
        }
    }

    fn cycle_left(self) -> Self {
        match self {
            Self::Fast => Self::Fast,
            Self::Medium => Self::Fast,
            Self::Slow => Self::Medium,
        }
    }

    fn cycle_right(self) -> Self {
        match self {
            Self::Fast => Self::Medium,
            Self::Medium => Self::Slow,
            Self::Slow => Self::Slow,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattleAnimation {
    On,
    Off,
}

impl BattleAnimation {
    fn toggle(self) -> Self {
        match self {
            Self::On => Self::Off,
            Self::Off => Self::On,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattleStyle {
    Shift,
    Set,
}

impl BattleStyle {
    fn toggle(self) -> Self {
        match self {
            Self::Shift => Self::Set,
            Self::Set => Self::Shift,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameOptions {
    pub text_speed: TextSpeed,
    pub battle_animation: BattleAnimation,
    pub battle_style: BattleStyle,
}

impl GameOptions {
    /// Pack into the wOptions byte format: bits 0-2 = text delay, bit 6 = battle style, bit 7 = battle animation
    pub fn to_byte(self) -> u8 {
        let mut b = self.text_speed.delay_frames() & TEXT_DELAY_MASK;
        if self.battle_animation == BattleAnimation::Off {
            b |= 1 << BIT_BATTLE_ANIMATION;
        }
        if self.battle_style == BattleStyle::Set {
            b |= 1 << BIT_BATTLE_SHIFT;
        }
        b
    }

    pub fn from_byte(b: u8) -> Self {
        let text_speed = TextSpeed::from_delay(b & TEXT_DELAY_MASK);
        let battle_animation = if b & (1 << BIT_BATTLE_ANIMATION) != 0 {
            BattleAnimation::Off
        } else {
            BattleAnimation::On
        };
        let battle_style = if b & (1 << BIT_BATTLE_SHIFT) != 0 {
            BattleStyle::Set
        } else {
            BattleStyle::Shift
        };
        Self {
            text_speed,
            battle_animation,
            battle_style,
        }
    }
}

impl Default for GameOptions {
    fn default() -> Self {
        Self {
            text_speed: TextSpeed::Medium,
            battle_animation: BattleAnimation::On,
            battle_style: BattleStyle::Shift,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionsRow {
    TextSpeed,
    BattleAnimation,
    BattleStyle,
    Cancel,
}

impl OptionsRow {
    pub fn y_coord(self) -> u8 {
        match self {
            Self::TextSpeed => 3,
            Self::BattleAnimation => 8,
            Self::BattleStyle => 13,
            Self::Cancel => 16,
        }
    }

    fn down(self) -> Self {
        match self {
            Self::TextSpeed => Self::BattleAnimation,
            Self::BattleAnimation => Self::BattleStyle,
            Self::BattleStyle => Self::Cancel,
            Self::Cancel => Self::TextSpeed,
        }
    }

    fn up(self) -> Self {
        match self {
            Self::TextSpeed => Self::Cancel,
            Self::BattleAnimation => Self::TextSpeed,
            Self::BattleStyle => Self::BattleAnimation,
            Self::Cancel => Self::BattleStyle,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptionsInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub a: bool,
    pub b: bool,
    pub start: bool,
}

impl OptionsInput {
    pub fn none() -> Self {
        Self {
            up: false,
            down: false,
            left: false,
            right: false,
            a: false,
            b: false,
            start: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionsMenuResult {
    Active,
    Closed,
}

#[derive(Debug, Clone)]
pub struct OptionsMenuState {
    pub row: OptionsRow,
    pub options: GameOptions,
}

impl OptionsMenuState {
    pub fn new(options: GameOptions) -> Self {
        Self {
            row: OptionsRow::TextSpeed,
            options,
        }
    }

    pub fn tick(&mut self, input: OptionsInput) -> OptionsMenuResult {
        if input.b || input.start {
            return OptionsMenuResult::Closed;
        }

        if input.a && self.row == OptionsRow::Cancel {
            return OptionsMenuResult::Closed;
        }

        if input.down {
            self.row = self.row.down();
            return OptionsMenuResult::Active;
        }

        if input.up {
            self.row = self.row.up();
            return OptionsMenuResult::Active;
        }

        match self.row {
            OptionsRow::TextSpeed => {
                if input.left {
                    self.options.text_speed = self.options.text_speed.cycle_left();
                } else if input.right {
                    self.options.text_speed = self.options.text_speed.cycle_right();
                }
            }
            OptionsRow::BattleAnimation => {
                if input.left || input.right {
                    self.options.battle_animation = self.options.battle_animation.toggle();
                }
            }
            OptionsRow::BattleStyle => {
                if input.left || input.right {
                    self.options.battle_style = self.options.battle_style.toggle();
                }
            }
            OptionsRow::Cancel => {}
        }

        OptionsMenuResult::Active
    }

    pub fn to_options_byte(&self) -> u8 {
        self.options.to_byte()
    }
}
