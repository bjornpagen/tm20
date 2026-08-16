//! Baseline skip in points; column gutters in 8-dot modules. Two coordinates.

use crate::size::{DisplaySize, TextSize};

/// White-space module in dots. Divides Font A’s 24-dot cell. Not leading.
pub const GRID: u16 = 8;

/// Gap from the bottom of a rule to the cap-line, in device dots (~1 pt).
pub const HANG: u16 = 3;

/// One point in device dots at 203 dpi.
pub fn pt_dots(pt: f32) -> u16 {
    (pt * crate::DPI / 72.0).round() as u16
}

/// Positive multiple of [`GRID`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridSkip {
    units: std::num::NonZeroU8,
}

impl GridSkip {
    pub const ONE: Self = Self {
        units: std::num::NonZeroU8::new(1).unwrap(),
    };

    pub fn n(units: u8) -> Option<Self> {
        std::num::NonZeroU8::new(units).map(|units| Self { units })
    }

    pub fn dots(self) -> u16 {
        u16::from(self.units.get()) * GRID
    }
}

/// Metal slug: body, or body plus 1 or 2 points. Not an 8-dot snap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leading {
    Solid,
    Plus1,
    Plus2,
}

impl Leading {
    pub fn for_text(_size: TextSize) -> Self {
        Leading::Plus2
    }

    pub fn for_display(_size: DisplaySize) -> Self {
        Leading::Solid
    }

    pub fn skip_dots(self, body_dots: u16) -> u16 {
        match self {
            Leading::Solid => body_dots,
            Leading::Plus1 => body_dots + pt_dots(1.0),
            Leading::Plus2 => body_dots + pt_dots(2.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_modules_on_grid() {
        assert_eq!(GridSkip::ONE.dots() % GRID, 0);
        assert_eq!(GridSkip::n(2).unwrap().dots() % GRID, 0);
    }

    #[test]
    fn eleven_pt_plus1_is_34_dots() {
        let body = TextSize::Pt11.body_dots();
        assert_eq!(body, 31);
        assert_eq!(Leading::Plus1.skip_dots(body), 34);
        assert_eq!(Leading::Solid.skip_dots(body), 31);
    }

    #[test]
    fn eleven_pt_plus2_is_37_dots() {
        let body = TextSize::Pt11.body_dots();
        assert_eq!(body, 31);
        assert_eq!(Leading::Plus2.skip_dots(body), 37);
        assert_eq!(Leading::for_text(TextSize::Pt11), Leading::Plus2);
    }
}
