//! Typesetter coordinates. Protocol [`tm20::Raster`] is not a [`Measure`].

use std::num::NonZeroU16;

use tm20::PRINTABLE_DOTS;

use crate::error::Error;
use crate::size::FRAC;

/// Canvas width in dots. Half-open `[0, get)`. Admits `1..=PRINTABLE_DOTS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Measure(NonZeroU16);

impl Measure {
    pub const TAPE: Self = Self(NonZeroU16::new(PRINTABLE_DOTS).unwrap());

    pub fn new(n: u16) -> Option<Self> {
        if n == 0 || n > PRINTABLE_DOTS {
            return None;
        }
        NonZeroU16::new(n).map(Self)
    }

    pub fn get(self) -> u16 {
        self.0.get()
    }
}

/// Absolute page row. Zero is the top of the painted raster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Row(u32);

impl Row {
    pub const ZERO: Self = Self(0);

    pub fn new(n: u32) -> Self {
        Self(n)
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

/// Signed 26.6 device units. One dot is 64 units.
///
/// Rounding and ceiling go through [`Self::round_dots`] / [`Self::ceil_dots`];
/// paint never casts a raw `i64` page height to `u16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Advance(i64);

impl Advance {
    pub const ZERO: Self = Self(0);

    pub fn from_units(units: i64) -> Self {
        Self(units)
    }

    pub fn from_dots(dots: i32) -> Option<Self> {
        i64::from(dots).checked_mul(i64::from(FRAC)).map(Self)
    }

    pub fn units(self) -> i64 {
        self.0
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }

    pub fn round_dots(self) -> Result<i32, Error> {
        let rounded = self.0.saturating_add(i64::from(FRAC) / 2) >> 6;
        i32::try_from(rounded).map_err(|_| Error::CoordinateOverflow)
    }

    /// Ceiling toward +∞ in whole dots. Negative values use toward-zero
    /// division so −1 unit → 0 and −65 units → −1, not an arithmetic floor.
    pub fn ceil_dots(self) -> Result<i32, Error> {
        let frac = i64::from(FRAC);
        let dots = if self.0 >= 0 {
            self.0
                .checked_add(frac - 1)
                .ok_or(Error::CoordinateOverflow)?
                / frac
        } else {
            self.0 / frac
        };
        i32::try_from(dots).map_err(|_| Error::CoordinateOverflow)
    }
}

/// Signed 26.6 half-open rectangle. Empty ink is [`InkBounds::EMPTY`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InkBounds {
    x0: Advance,
    y0: Advance,
    x1: Advance,
    y1: Advance,
    empty: bool,
}

impl InkBounds {
    pub const EMPTY: Self = Self {
        x0: Advance::ZERO,
        y0: Advance::ZERO,
        x1: Advance::ZERO,
        y1: Advance::ZERO,
        empty: true,
    };

    pub fn new(x0: Advance, y0: Advance, x1: Advance, y1: Advance) -> Option<Self> {
        if x1.units() < x0.units() || y1.units() < y0.units() {
            return None;
        }
        if x1.units() <= x0.units() || y1.units() <= y0.units() {
            return Some(Self::EMPTY);
        }
        Some(Self {
            x0,
            y0,
            x1,
            y1,
            empty: false,
        })
    }

    pub fn is_empty(self) -> bool {
        self.empty || self.x1.units() <= self.x0.units() || self.y1.units() <= self.y0.units()
    }

    pub fn translate(self, dx: Advance, dy: Advance) -> Option<Self> {
        if self.is_empty() {
            return Some(Self::EMPTY);
        }
        let next = Self {
            x0: self.x0.checked_add(dx)?,
            y0: self.y0.checked_add(dy)?,
            x1: self.x1.checked_add(dx)?,
            y1: self.y1.checked_add(dy)?,
            empty: false,
        };
        if next.is_empty() {
            return Some(Self::EMPTY);
        }
        Some(next)
    }

    pub fn union(self, other: Self) -> Option<Self> {
        match (self.is_empty(), other.is_empty()) {
            (true, true) => Some(Self::EMPTY),
            (true, false) => Some(other),
            (false, true) => Some(self),
            (false, false) => Self::new(
                Advance::from_units(self.x0.units().min(other.x0.units())),
                Advance::from_units(self.y0.units().min(other.y0.units())),
                Advance::from_units(self.x1.units().max(other.x1.units())),
                Advance::from_units(self.y1.units().max(other.y1.units())),
            ),
        }
    }

    pub fn x0(self) -> Advance {
        self.x0
    }

    pub fn y0(self) -> Advance {
        self.y0
    }

    pub fn x1(self) -> Advance {
        self.x1
    }

    pub fn y1(self) -> Advance {
        self.y1
    }
}

/// Half-open horizontal interval inside a page [`Measure`]. Intersection may be empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clip {
    start: u16,
    end: u16,
}

impl Clip {
    pub fn new(start: u16, end: u16) -> Option<Self> {
        (start <= end).then_some(Self { start, end })
    }

    pub fn full(measure: Measure) -> Self {
        Self {
            start: 0,
            end: measure.get(),
        }
    }

    pub fn start(self) -> u16 {
        self.start
    }

    pub fn end(self) -> u16 {
        self.end
    }

    pub fn is_empty(self) -> bool {
        self.start >= self.end
    }

    #[must_use]
    pub fn intersection(self, other: Self) -> Self {
        let start = self.start.max(other.start);
        let end = self.end.min(other.end);
        if start >= end {
            Self { start, end: start }
        } else {
            Self { start, end }
        }
    }
}

/// Half-open page-row interval, ordered and inside a named extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowRange {
    start: Row,
    end: Row,
}

impl RowRange {
    pub fn new(start: Row, end: Row) -> Option<Self> {
        (start.get() < end.get()).then_some(Self { start, end })
    }

    pub fn start(self) -> Row {
        self.start
    }

    pub fn end(self) -> Row {
        self.end
    }

    pub fn height(self) -> u32 {
        self.end.get().saturating_sub(self.start.get())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceil_dots_is_signed_ceiling() {
        assert_eq!(Advance::from_units(0).ceil_dots().unwrap(), 0);
        assert_eq!(Advance::from_units(1).ceil_dots().unwrap(), 1);
        assert_eq!(Advance::from_units(63).ceil_dots().unwrap(), 1);
        assert_eq!(Advance::from_units(64).ceil_dots().unwrap(), 1);
        assert_eq!(Advance::from_units(65).ceil_dots().unwrap(), 2);
        assert_eq!(Advance::from_units(-1).ceil_dots().unwrap(), 0);
        assert_eq!(Advance::from_units(-63).ceil_dots().unwrap(), 0);
        assert_eq!(Advance::from_units(-64).ceil_dots().unwrap(), -1);
        assert_eq!(Advance::from_units(-65).ceil_dots().unwrap(), -1);
        assert_eq!(Advance::from_units(-128).ceil_dots().unwrap(), -2);
        assert!(Advance::from_units(i64::MAX).ceil_dots().is_err());
    }

    #[test]
    fn empty_bounds_are_canonical() {
        let zero = InkBounds::new(
            Advance::from_units(100),
            Advance::from_units(100),
            Advance::from_units(100),
            Advance::from_units(100),
        )
        .unwrap();
        assert!(zero.is_empty());
        assert_eq!(zero, InkBounds::EMPTY);
        let ink = InkBounds::new(
            Advance::ZERO,
            Advance::ZERO,
            Advance::from_units(10),
            Advance::from_units(10),
        )
        .unwrap();
        assert_eq!(zero.union(ink).unwrap(), ink);
        assert_eq!(ink.union(zero).unwrap(), ink);
        assert_eq!(zero.union(zero).unwrap(), InkBounds::EMPTY);
        assert_eq!(
            zero.translate(Advance::from_units(50), Advance::from_units(-20))
                .unwrap(),
            InkBounds::EMPTY
        );
        let a = InkBounds::new(
            Advance::from_units(0),
            Advance::from_units(0),
            Advance::from_units(4),
            Advance::from_units(4),
        )
        .unwrap();
        let b = InkBounds::new(
            Advance::from_units(2),
            Advance::from_units(2),
            Advance::from_units(6),
            Advance::from_units(6),
        )
        .unwrap();
        let c = InkBounds::new(
            Advance::from_units(10),
            Advance::from_units(0),
            Advance::from_units(12),
            Advance::from_units(2),
        )
        .unwrap();
        let ab_c = a.union(b).unwrap().union(c).unwrap();
        let a_bc = a.union(b.union(c).unwrap()).unwrap();
        assert_eq!(ab_c, a_bc);
        assert_eq!(a.union(b).unwrap(), b.union(a).unwrap());
        let shifted = a
            .translate(Advance::from_units(3), Advance::from_units(-1))
            .unwrap();
        assert!(!shifted.is_empty());
        assert_eq!(shifted.x0().units(), 3);
        assert_eq!(shifted.y0().units(), -1);
    }
}
