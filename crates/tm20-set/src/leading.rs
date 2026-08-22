//! Baseline skip in points; column gutters in 8-dot modules. Two coordinates.

/// White-space module in dots. Divides Font A’s 24-dot cell. Not leading.
pub const GRID: u16 = 8;

/// Task-list checkbox side. Three modules; stroke sits on this square.
pub const TASK_BOX: u16 = 3 * GRID;

/// Notes rule length. GPO’s 50-point rule, snapped to the grid (~144 dots).
pub const NOTE_RULE: u16 = 18 * GRID;

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

    /// Two modules. Three-column gutter.
    pub const TWO: Self = Self {
        units: std::num::NonZeroU8::new(2).unwrap(),
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
    use crate::size::TextSize;

    #[test]
    fn skip_modules_on_grid() {
        assert_eq!(GridSkip::ONE.dots() % GRID, 0);
        assert_eq!(GridSkip::TWO.dots(), 2 * GRID);
        assert_eq!(GridSkip::n(2).unwrap().dots() % GRID, 0);
        assert_eq!(TASK_BOX % GRID, 0);
        assert_eq!(NOTE_RULE % GRID, 0);
        assert_eq!(TASK_BOX, 24);
        assert_eq!(NOTE_RULE, 144);
    }

    #[test]
    fn eleven_pt_plus1_is_34_dots() {
        let body = TextSize::Pt11.body_dots();
        assert_eq!(Leading::Plus1.skip_dots(body), 34);
        assert_eq!(Leading::Solid.skip_dots(body), body);
    }
}
