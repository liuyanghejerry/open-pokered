#[cfg(feature = "gpu")]
use winit::keyboard::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GbButton {
    A = 0,
    B = 1,
    Select = 2,
    Start = 3,
    Right = 4,
    Left = 5,
    Up = 6,
    Down = 7,
}

impl GbButton {
    pub const ALL: [GbButton; 8] = [
        GbButton::A,
        GbButton::B,
        GbButton::Select,
        GbButton::Start,
        GbButton::Right,
        GbButton::Left,
        GbButton::Up,
        GbButton::Down,
    ];

    pub fn bit_mask(self) -> u8 {
        1 << (self as u8)
    }
}

#[derive(Debug, Clone)]
pub struct InputState {
    current: u8,
    previous: u8,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            current: 0,
            previous: 0,
        }
    }

    pub fn is_held(&self, button: GbButton) -> bool {
        self.current & button.bit_mask() != 0
    }

    pub fn is_just_pressed(&self, button: GbButton) -> bool {
        let mask = button.bit_mask();
        (self.current & mask != 0) && (self.previous & mask == 0)
    }

    pub fn is_just_released(&self, button: GbButton) -> bool {
        let mask = button.bit_mask();
        (self.current & mask == 0) && (self.previous & mask != 0)
    }

    pub fn any_held(&self) -> bool {
        self.current != 0
    }

    pub fn any_just_pressed(&self) -> bool {
        (self.current & !self.previous) != 0
    }

    pub fn raw_current(&self) -> u8 {
        self.current
    }

    pub fn raw_previous(&self) -> u8 {
        self.previous
    }

    pub fn begin_frame(&mut self) {
        self.previous = self.current;
    }

    pub fn press(&mut self, button: GbButton) {
        self.current |= button.bit_mask();
    }

    pub fn release(&mut self, button: GbButton) {
        self.current &= !button.bit_mask();
    }

    #[cfg(feature = "gpu")]
    pub fn set_from_keycode(&mut self, keycode: KeyCode, pressed: bool) {
        if let Some(button) = keycode_to_gb_button(keycode) {
            if pressed {
                self.press(button);
            } else {
                self.release(button);
            }
        }
    }

    pub fn clear(&mut self) {
        self.current = 0;
        self.previous = 0;
    }

    pub fn set_from_bitmask(&mut self, mask: u8) {
        self.current = mask;
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "gpu")]
pub fn keycode_to_gb_button(keycode: KeyCode) -> Option<GbButton> {
    match keycode {
        // Arrow keys / WASD → D-pad
        KeyCode::ArrowUp | KeyCode::KeyW => Some(GbButton::Up),
        KeyCode::ArrowDown | KeyCode::KeyS => Some(GbButton::Down),
        KeyCode::ArrowLeft | KeyCode::KeyA => Some(GbButton::Left),
        KeyCode::ArrowRight | KeyCode::KeyD => Some(GbButton::Right),
        // Z → A button, X → B button
        KeyCode::KeyZ => Some(GbButton::A),
        KeyCode::KeyX => Some(GbButton::B),
        // Enter/Space → Start, Backspace/RightShift → Select
        KeyCode::Enter | KeyCode::Space => Some(GbButton::Start),
        KeyCode::Backspace | KeyCode::ShiftRight => Some(GbButton::Select),
        _ => None,
    }
}
