//! Packed 1-bit graphics (`GS ( L` fn=112 + fn=50).

use crate::error::EncodeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsScale {
    Normal,
    DoubleWidth,
    DoubleHeight,
    Quadruple,
}

impl GraphicsScale {
    fn mag(self) -> (u8, u8) {
        match self {
            GraphicsScale::Normal => (1, 1),
            GraphicsScale::DoubleWidth => (2, 1),
            GraphicsScale::DoubleHeight => (1, 2),
            GraphicsScale::Quadruple => (2, 2),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graphics {
    pub width_dots: u16,
    pub height_dots: u16,
    pub pixels: Vec<u8>,
    pub scale: GraphicsScale,
}

pub fn width_bytes(width_dots: u16) -> usize {
    (width_dots as usize).div_ceil(8)
}

/// Set a black dot. Same bit order as [`Graphics::pixels`]: row-major, MSB first.
#[inline]
pub fn set_black(pixels: &mut [u8], stride: usize, x: usize, y: usize) {
    pixels[y * stride + x / 8] |= 0x80 >> (x % 8);
}

/// True if that dot is black in packed MSB-first bytes.
#[inline]
pub fn is_black(pixels: &[u8], stride: usize, x: usize, y: usize) -> bool {
    pixels[y * stride + x / 8] & (0x80 >> (x % 8)) != 0
}

/// Largest `height_dots` whose fn=112 body fits in a 16-bit length.
/// At 576 dots wide this is 910 (`10 + 72*h ≤ 65535`).
pub fn max_height(width_dots: u16) -> u16 {
    let stride = width_bytes(width_dots).max(1);
    ((65535 - 10) / stride) as u16
}

/// Pack row-major pixels, `true` = black, MSB first in each byte.
pub fn pack(width_dots: u16, height_dots: u16, pixels: &[bool]) -> Result<Vec<u8>, EncodeError> {
    let expected = width_dots as usize * height_dots as usize;
    if pixels.len() != expected {
        return Err(EncodeError::GraphicsPackedLen {
            expected,
            got: pixels.len(),
        });
    }
    let stride = width_bytes(width_dots);
    let mut out = vec![0u8; stride * height_dots as usize];
    for y in 0..height_dots as usize {
        for x in 0..width_dots as usize {
            if pixels[y * width_dots as usize + x] {
                set_black(&mut out, stride, x, y);
            }
        }
    }
    Ok(out)
}

pub fn encode(image: &Graphics) -> Result<Vec<u8>, EncodeError> {
    let stride = width_bytes(image.width_dots);
    let expected = stride * image.height_dots as usize;
    if image.pixels.len() != expected {
        return Err(EncodeError::GraphicsPackedLen {
            expected,
            got: image.pixels.len(),
        });
    }
    let body = 10 + image.pixels.len();
    if body > 65535 {
        return Err(EncodeError::GraphicsTooLong { len: body });
    }
    let (bx, by) = image.scale.mag();
    let x = image.width_dots;
    let y = image.height_dots;
    let pl = (body % 256) as u8;
    let ph = (body / 256) as u8;
    let mut out = Vec::with_capacity(8 + body + 8);
    out.extend_from_slice(&[
        0x1d,
        b'(',
        b'L',
        pl,
        ph,
        48,
        112,
        48,
        bx,
        by,
        49,
        (x & 0xff) as u8,
        (x >> 8) as u8,
        (y & 0xff) as u8,
        (y >> 8) as u8,
    ]);
    out.extend_from_slice(&image.pixels);
    out.extend_from_slice(&[0x1d, b'(', b'L', 2, 0, 48, 50]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_8x8_all_black() {
        let pixels = pack(8, 8, &[true; 64]).unwrap();
        assert_eq!(pixels, vec![0xff; 8]);
        let bytes = encode(&Graphics {
            width_dots: 8,
            height_dots: 8,
            pixels,
            scale: GraphicsScale::Normal,
        })
        .unwrap();
        assert_eq!(
            &bytes[..15],
            &[0x1d, b'(', b'L', 18, 0, 48, 112, 48, 1, 1, 49, 8, 0, 8, 0]
        );
        assert_eq!(&bytes[15..23], &[0xff; 8]);
        assert_eq!(&bytes[23..], &[0x1d, b'(', b'L', 2, 0, 48, 50]);
    }

    #[test]
    fn pack_msb_first() {
        let mut bits = [false; 8];
        bits[0] = true;
        bits[7] = true;
        assert_eq!(pack(8, 1, &bits).unwrap(), vec![0b1000_0001]);
    }

    #[test]
    fn set_black_is_the_pack_table() {
        let mut pixels = vec![0u8; 2];
        set_black(&mut pixels, 1, 0, 0);
        set_black(&mut pixels, 1, 7, 1);
        assert!(is_black(&pixels, 1, 0, 0));
        assert!(is_black(&pixels, 1, 7, 1));
        assert!(!is_black(&pixels, 1, 1, 0));
        assert_eq!(pixels, vec![0b1000_0000, 0b0000_0001]);
    }

    #[test]
    fn tape_wide_max_height_is_910() {
        assert_eq!(max_height(576), 910);
        assert!(10 + width_bytes(576) * 910 <= 65535);
        assert!(10 + width_bytes(576) * 911 > 65535);
    }

    #[test]
    fn quadruple_is_2_by_2() {
        let pixels = pack(8, 1, &[true; 8]).unwrap();
        let bytes = encode(&Graphics {
            width_dots: 8,
            height_dots: 1,
            pixels,
            scale: GraphicsScale::Quadruple,
        })
        .unwrap();
        assert_eq!(bytes[8], 2);
        assert_eq!(bytes[9], 2);
    }
}
