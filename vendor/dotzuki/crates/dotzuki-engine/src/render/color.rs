/// An RGBA colour with 8 bits per channel.
///
/// Each channel (`r`, `g`, `b`, `a`) ranges from `0` to `255`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel (`0` = fully transparent, `255` = fully opaque).
    pub a: u8,
}

impl Rgba {
    /// Pure white, fully opaque.
    pub const WHITE: Self = Self::rgb(0xFF, 0xFF, 0xFF);

    /// Pure black, fully opaque.
    pub const BLACK: Self = Self::rgb(0x00, 0x00, 0x00);

    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self::new(0, 0, 0, 0);

    /// Darkest shade of the default 4-step ink ramp used for menu/text
    /// rendering (matches the historical `InkColor::Black` RGBA value).
    pub const INK_BLACK: Self = Self::rgb(0x20, 0x20, 0x20);

    /// Dark-gray shade of the default ink ramp.
    pub const INK_DARK_GRAY: Self = Self::rgb(0x60, 0x60, 0x60);

    /// Light-gray shade of the default ink ramp.
    pub const INK_LIGHT_GRAY: Self = Self::rgb(0xA0, 0xA0, 0xA0);

    /// Lightest shade of the default ink ramp (paper background).
    pub const INK_WHITE: Self = Self::rgb(0xE0, 0xE0, 0xE0);

    /// Create a colour from individual channel values.
    #[inline]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create an opaque colour from RGB values (alpha = 255).
    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 0xFF }
    }

    /// Return the channels as a flat `[r, g, b, a]` byte array.
    #[inline]
    pub const fn to_array(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

impl From<Rgba> for [u8; 4] {
    fn from(c: Rgba) -> Self {
        c.to_array()
    }
}

impl Default for Rgba {
    /// Returns [`Rgba::TRANSPARENT`].
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

impl From<[u8; 4]> for Rgba {
    /// Convert a `[r, g, b, a]` byte array into an [`Rgba`].
    fn from(arr: [u8; 4]) -> Self {
        Self {
            r: arr[0],
            g: arr[1],
            b: arr[2],
            a: arr[3],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn const_white() {
        assert_eq!(
            Rgba::WHITE,
            Rgba {
                r: 0xFF,
                g: 0xFF,
                b: 0xFF,
                a: 0xFF
            }
        );
    }

    #[test]
    fn const_black() {
        assert_eq!(
            Rgba::BLACK,
            Rgba {
                r: 0x00,
                g: 0x00,
                b: 0x00,
                a: 0xFF
            }
        );
    }

    #[test]
    fn const_transparent() {
        assert_eq!(
            Rgba::TRANSPARENT,
            Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 0
            }
        );
    }

    #[test]
    fn new_constructor() {
        let c = Rgba::new(10, 20, 30, 40);
        assert_eq!(c.r, 10);
        assert_eq!(c.g, 20);
        assert_eq!(c.b, 30);
        assert_eq!(c.a, 40);
    }

    #[test]
    fn rgb_constructor() {
        let c = Rgba::rgb(10, 20, 30);
        assert_eq!(c.r, 10);
        assert_eq!(c.g, 20);
        assert_eq!(c.b, 30);
        assert_eq!(c.a, 0xFF);
    }

    #[test]
    fn to_array() {
        let c = Rgba::new(1, 2, 3, 4);
        assert_eq!(c.to_array(), [1, 2, 3, 4]);
    }

    #[test]
    fn from_array() {
        let c = Rgba::from([10, 20, 30, 40]);
        assert_eq!(c, Rgba::new(10, 20, 30, 40));
    }

    #[test]
    fn default_is_transparent() {
        assert_eq!(Rgba::default(), Rgba::TRANSPARENT);
    }

    #[test]
    fn equality() {
        assert_eq!(Rgba::new(1, 2, 3, 4), Rgba::new(1, 2, 3, 4));
        assert_ne!(Rgba::new(1, 2, 3, 4), Rgba::new(5, 2, 3, 4));
    }

    #[test]
    fn copy_works() {
        let a = Rgba::new(1, 2, 3, 4);
        let b = a;
        assert_eq!(a, b);
    }
}
