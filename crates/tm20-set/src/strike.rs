//! Unhinted coverage mask → packed 1-bit strike. A printer dot is ink at coverage 96.

use std::sync::Arc;

use tm20::graphics::width_bytes;

use crate::geometry::{Advance, InkBounds};
use crate::size::FRAC;

/// Packed glyph at one [`ppem`](crate::TextSize::ppem). Bearings are dots; advance is 26.6.
///
/// Immutable after construction. Repeated use shares one owner via [`Arc`]; do not
/// clone the packed pixels.
pub struct Strike {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) bits: Vec<u8>,
    pub(crate) advance: i32,
}

impl Strike {
    pub(crate) fn empty(advance: i32) -> Self {
        Self {
            left: 0,
            top: 0,
            width: 0,
            height: 0,
            bits: Vec::new(),
            advance,
        }
    }

    pub fn left(&self) -> i32 {
        self.left
    }

    pub fn top(&self) -> i32 {
        self.top
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn bits(&self) -> &[u8] {
        &self.bits
    }

    pub fn advance_units(&self) -> i32 {
        self.advance
    }

    /// Local ink in 26.6, origin at the pen, y down. Left bearing may be negative;
    /// descenders have `y1 > 0`. Empty coverage is [`InkBounds::EMPTY`].
    pub fn ink(&self) -> InkBounds {
        if self.width == 0 || self.height == 0 {
            return InkBounds::EMPTY;
        }
        let Some(x0) = Advance::from_dots(self.left) else {
            return InkBounds::EMPTY;
        };
        let Some(y0) = Advance::from_dots(-self.top) else {
            return InkBounds::EMPTY;
        };
        let Some(w) = Advance::from_dots(i32::from(self.width)) else {
            return InkBounds::EMPTY;
        };
        let Some(h) = Advance::from_dots(i32::from(self.height)) else {
            return InkBounds::EMPTY;
        };
        let Some(x1) = x0.checked_add(w) else {
            return InkBounds::EMPTY;
        };
        let Some(y1) = y0.checked_add(h) else {
            return InkBounds::EMPTY;
        };
        InkBounds::new(x0, y0, x1, y1).unwrap_or(InkBounds::EMPTY)
    }
}

/// Coverage occupies the pixel when it meets the original compose cut.
const INK: u8 = 96;

fn set_black(bits: &mut [u8], stride: usize, x: usize, y: usize) {
    bits[y * stride + x / 8] |= 0x80 >> (x % 8);
}

pub(crate) fn from_mask(
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    coverage: &[u8],
    advance_px: f32,
) -> Strike {
    let advance = (advance_px * FRAC as f32).round() as i32;
    let Ok(width) = u16::try_from(width) else {
        return Strike::empty(advance);
    };
    let Ok(height) = u16::try_from(height) else {
        return Strike::empty(advance);
    };
    if width == 0 || height == 0 {
        return Strike::empty(advance);
    }
    let w = usize::from(width);
    let h = usize::from(height);
    if coverage.len() < w * h {
        return Strike::empty(advance);
    }
    let stride = width_bytes(width);
    let mut bits = vec![0u8; stride * h];
    for y in 0..h {
        for x in 0..w {
            if coverage[y * w + x] >= INK {
                set_black(&mut bits, stride, x, y);
            }
        }
    }
    Strike {
        left,
        top,
        width,
        height,
        bits,
        advance,
    }
}

pub(crate) type SharedStrike = Arc<Strike>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ink_keeps_negative_bearing_and_descender() {
        let coverage = vec![255u8; 8 * 12];
        let strike = from_mask(-4, 10, 8, 12, &coverage, 10.0);
        assert_eq!(strike.left, -4);
        assert_eq!(strike.top, 10);
        let ink = strike.ink();
        assert!(!ink.is_empty());
        assert_eq!(ink.x0().units(), i64::from(-4) * i64::from(FRAC));
        assert_eq!(ink.y0().units(), i64::from(-10) * i64::from(FRAC));
        assert_eq!(ink.x1().units(), i64::from(4) * i64::from(FRAC));
        assert_eq!(ink.y1().units(), i64::from(2) * i64::from(FRAC));
    }

    #[test]
    fn empty_coverage_is_empty_ink() {
        assert!(Strike::empty(64).ink().is_empty());
        assert!(from_mask(0, 0, 0, 0, &[], 1.0).ink().is_empty());
    }
}
