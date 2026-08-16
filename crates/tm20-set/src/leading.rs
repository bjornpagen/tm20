//! Baseline skip on an 8-dot grid. Not a CSS ratio.

use crate::size::{DisplaySize, TextSize};

/// Snap unit in dots. Divides Font A’s 24-dot cell.
pub const GRID: u16 = 8;

fn snap_up(dots: u16) -> u16 {
    dots.div_ceil(GRID) * GRID
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

/// Distance from one baseline to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leading {
    /// Body size, snapped up onto the grid.
    Solid,
    /// Body (snapped) plus extra grid units.
    Extra(GridSkip),
}

impl Leading {
    pub fn for_text(size: TextSize) -> Self {
        match size {
            TextSize::Pt8 => Leading::Extra(GridSkip::n(2).unwrap()),
            TextSize::Pt11 => Leading::Extra(GridSkip::ONE),
        }
    }

    pub fn for_display(_size: DisplaySize) -> Self {
        Leading::Solid
    }

    pub fn skip_dots(self, body_dots: u16) -> u16 {
        let solid = snap_up(body_dots);
        match self {
            Leading::Solid => solid,
            Leading::Extra(extra) => solid + extra.dots(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_always_on_grid() {
        for size in [TextSize::Pt8, TextSize::Pt11] {
            let s = Leading::for_text(size).skip_dots(size.body_dots());
            assert_eq!(s % GRID, 0, "text {size:?} skip {s}");
        }
        for size in [DisplaySize::Pt14, DisplaySize::Pt18, DisplaySize::Pt24] {
            let s = Leading::for_display(size).skip_dots(size.body_dots());
            assert_eq!(s % GRID, 0, "display {size:?} skip {s}");
        }
    }
}
