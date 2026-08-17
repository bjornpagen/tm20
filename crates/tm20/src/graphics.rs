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
                out[y * stride + x / 8] |= 0x80 >> (x % 8);
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
