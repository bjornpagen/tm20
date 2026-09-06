//! Packed 1-bit graphics (`GS ( L` fn=112 + fn=50).

use crate::error::{EncodeError, RasterError};
use crate::raster::{Raster, stride_bytes};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsScale {
    Normal,
    DoubleWidth,
    DoubleHeight,
    Quadruple,
}

impl GraphicsScale {
    pub fn factors(self) -> (u8, u8) {
        match self {
            GraphicsScale::Normal => (1, 1),
            GraphicsScale::DoubleWidth => (2, 1),
            GraphicsScale::DoubleHeight => (1, 2),
            GraphicsScale::Quadruple => (2, 2),
        }
    }
}

/// A [`Raster`] that fits the fn=112 16-bit body, plus commanded scale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graphics {
    raster: Raster,
    scale: GraphicsScale,
}

impl Graphics {
    pub fn new(raster: Raster, scale: GraphicsScale) -> Result<Self, EncodeError> {
        let cap = u32::from(max_height(raster.width()));
        if raster.height() > cap {
            return Err(EncodeError::GraphicsTooLong {
                len: 10 + raster.pixels().len(),
            });
        }
        Ok(Self { raster, scale })
    }

    pub fn raster(&self) -> &Raster {
        &self.raster
    }

    pub fn scale(&self) -> GraphicsScale {
        self.scale
    }
}

pub fn width_bytes(width_dots: u16) -> usize {
    stride_bytes(width_dots)
}

/// Largest raster height whose fn=112 body fits in a 16-bit length.
/// At 576 dots wide this is 910 (`10 + 72*h ≤ 65535`).
pub fn max_height(width_dots: u16) -> u16 {
    let stride = width_bytes(width_dots).max(1);
    ((65535 - 10) / stride) as u16
}

/// Pack row-major pixels, `true` = black, MSB first in each byte.
///
/// Prefer [`Raster::from_bits`]; this remains while the crate root still
/// re-exports it.
pub fn pack(width_dots: u16, height_dots: u16, pixels: &[bool]) -> Result<Vec<u8>, EncodeError> {
    match Raster::from_bits(width_dots, u32::from(height_dots), pixels) {
        Ok(raster) => Ok(raster.pixels().to_vec()),
        Err(RasterError::LengthMismatch { expected, got }) => {
            Err(EncodeError::GraphicsPackedLen { expected, got })
        }
        Err(RasterError::Overflow | RasterError::Alloc) => {
            Err(EncodeError::GraphicsTooLong { len: pixels.len() })
        }
        Err(RasterError::ZeroWidth | RasterError::ZeroHeight | RasterError::InvalidRowRange) => {
            Err(EncodeError::GraphicsPackedLen {
                expected: usize::from(width_dots).saturating_mul(usize::from(height_dots)),
                got: pixels.len(),
            })
        }
    }
}

pub fn encode(image: &Graphics) -> Result<Vec<u8>, EncodeError> {
    let raster = image.raster();
    let pixels = raster.pixels();
    let body = 10 + pixels.len();
    if body > 65535 {
        return Err(EncodeError::GraphicsTooLong { len: body });
    }
    let (bx, by) = image.scale().factors();
    let x = raster.width();
    let y =
        u16::try_from(raster.height()).map_err(|_| EncodeError::GraphicsTooLong { len: body })?;
    let pl = (body % 256) as u8;
    let ph = (body / 256) as u8;
    let mut out = Vec::with_capacity(15 + pixels.len() + 7);
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
    out.extend_from_slice(pixels);
    out.extend_from_slice(&[0x1d, b'(', b'L', 2, 0, 48, 50]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graphics_from_bits(
        width: u16,
        height: u32,
        bits: &[bool],
        scale: GraphicsScale,
    ) -> Graphics {
        let raster = Raster::from_bits(width, height, bits).unwrap();
        Graphics::new(raster, scale).unwrap()
    }

    #[test]
    fn pack_8x8_all_black() {
        let image = graphics_from_bits(8, 8, &[true; 64], GraphicsScale::Normal);
        assert_eq!(image.raster().pixels(), vec![0xff; 8]);
        let bytes = encode(&image).unwrap();
        assert_eq!(
            &bytes[..15],
            &[0x1d, b'(', b'L', 18, 0, 48, 112, 48, 1, 1, 49, 8, 0, 8, 0]
        );
        assert_eq!(&bytes[15..23], &[0xff; 8]);
        assert_eq!(&bytes[23..], &[0x1d, b'(', b'L', 2, 0, 48, 50]);
        let body = 10 + image.raster().pixels().len();
        assert_eq!(bytes[3] as usize + 256 * bytes[4] as usize, body);
        assert_eq!(bytes.len(), 15 + image.raster().pixels().len() + 7);
    }

    #[test]
    fn pack_msb_first() {
        let mut bits = [false; 8];
        bits[0] = true;
        bits[7] = true;
        assert_eq!(pack(8, 1, &bits).unwrap(), vec![0b1000_0001]);
    }

    #[test]
    fn scale_factors_are_the_magnification_table() {
        assert_eq!(GraphicsScale::Normal.factors(), (1, 1));
        assert_eq!(GraphicsScale::DoubleWidth.factors(), (2, 1));
        assert_eq!(GraphicsScale::DoubleHeight.factors(), (1, 2));
        assert_eq!(GraphicsScale::Quadruple.factors(), (2, 2));
    }

    #[test]
    fn tape_wide_max_height_is_910() {
        assert_eq!(max_height(576), 910);
        assert!(10 + width_bytes(576) * 910 <= 65535);
        assert!(10 + width_bytes(576) * 911 > 65535);
    }

    #[test]
    fn tape_wide_910_encodes_and_911_is_capacity() {
        let stride = width_bytes(576);
        let ok = Raster::from_packed(576, 910, vec![0u8; stride * 910]).unwrap();
        assert_eq!(ok.height(), 910);
        let image = Graphics::new(ok, GraphicsScale::Normal).unwrap();
        let bytes = encode(&image).unwrap();
        let payload = image.raster().pixels();
        let body = 10 + payload.len();
        assert_eq!(payload.len(), stride * 910);
        assert_eq!(bytes[3] as usize + 256 * bytes[4] as usize, body);
        assert_eq!(&bytes[15..15 + payload.len()], payload);
        assert_eq!(bytes.len(), 15 + payload.len() + 7);
        assert_eq!(
            &bytes[11..15],
            &[
                (0x240u16 & 0xff) as u8,
                (0x240u16 >> 8) as u8,
                (0x38eu16 & 0xff) as u8,
                (0x38eu16 >> 8) as u8
            ]
        );

        let tall = Raster::from_packed(576, 911, vec![0u8; stride * 911]).unwrap();
        assert_eq!(tall.height(), 911);
        assert!(matches!(
            Graphics::new(tall, GraphicsScale::Normal),
            Err(EncodeError::GraphicsTooLong { len }) if len == 10 + stride * 911
        ));
    }

    #[test]
    fn raster_dimensions_succeed_where_wire_capacity_fails() {
        let page = Raster::from_packed(1, 65_536, vec![0u8; 65_536]).unwrap();
        assert_eq!(page.width(), 1);
        assert_eq!(page.height(), 65_536);
        assert!(matches!(
            Graphics::new(page, GraphicsScale::Normal),
            Err(EncodeError::GraphicsTooLong { .. })
        ));
    }

    #[test]
    fn quadruple_is_2_by_2() {
        let bytes = encode(&graphics_from_bits(
            8,
            1,
            &[true; 8],
            GraphicsScale::Quadruple,
        ))
        .unwrap();
        assert_eq!(bytes[8], 2);
        assert_eq!(bytes[9], 2);
    }
}
