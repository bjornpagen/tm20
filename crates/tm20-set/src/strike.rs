//! Unhinted coverage mask → packed 1-bit strike. A printer dot is ink at coverage 96.

use tm20::graphics::{set_black, width_bytes};

use crate::size::FRAC;

/// Packed glyph at one [`ppem`](crate::TextSize::ppem). Bearings are dots; advance is 26.6.
#[derive(Clone)]
pub(crate) struct Strike {
    pub left: i32,
    pub top: i32,
    pub width: u16,
    pub height: u16,
    pub bits: Vec<u8>,
    pub advance: i32,
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
}

/// Coverage occupies the pixel when it meets the original compose cut.
const INK: u8 = 96;

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
