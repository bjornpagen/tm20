//! Immutable packed 1-bit page or image raster. Wire banding lives in [`crate::graphics::Graphics`].

use std::ops::Range;

use crate::error::RasterError;

/// Packed MSB-first 1-bit image. Width and height are nonzero; stride is derived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raster {
    width: u16,
    height: u32,
    pixels: Vec<u8>,
}

impl Raster {
    pub fn from_packed(width: u16, height: u32, mut pixels: Vec<u8>) -> Result<Self, RasterError> {
        if width == 0 {
            return Err(RasterError::ZeroWidth);
        }
        if height == 0 {
            return Err(RasterError::ZeroHeight);
        }
        let expected = packed_len(width, height)?;
        if pixels.len() != expected {
            return Err(RasterError::LengthMismatch {
                expected,
                got: pixels.len(),
            });
        }
        normalize_padding(width, &mut pixels);
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn from_bits(width: u16, height: u32, pixels: &[bool]) -> Result<Self, RasterError> {
        if width == 0 {
            return Err(RasterError::ZeroWidth);
        }
        if height == 0 {
            return Err(RasterError::ZeroHeight);
        }
        let w = usize::from(width);
        let h = usize::try_from(height).map_err(|_| RasterError::Overflow)?;
        let expected = w.checked_mul(h).ok_or(RasterError::Overflow)?;
        if pixels.len() != expected {
            return Err(RasterError::LengthMismatch {
                expected,
                got: pixels.len(),
            });
        }
        let total = packed_len(width, height)?;
        let mut packed = Vec::new();
        packed.try_reserve(total).map_err(|_| RasterError::Alloc)?;
        packed.resize(total, 0);
        let stride = stride_bytes(width);
        for y in 0..h {
            for x in 0..w {
                if pixels[y * w + x] {
                    packed[y * stride + x / 8] |= bit_mask(x);
                }
            }
        }
        Self::from_packed(width, height, packed)
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn stride(&self) -> usize {
        stride_bytes(self.width)
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn pixel(&self, x: u16, y: u32) -> Option<bool> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let stride = self.stride();
        let row = usize::try_from(y).ok()?;
        Some(test_bit(&self.pixels, stride, usize::from(x), row))
    }

    pub fn slice_rows(&self, rows: Range<u32>) -> Result<Self, RasterError> {
        if rows.start >= rows.end || rows.end > self.height {
            return Err(RasterError::InvalidRowRange);
        }
        let height = rows.end - rows.start;
        let stride = self.stride();
        let start = usize::try_from(rows.start).map_err(|_| RasterError::Overflow)?;
        let end = usize::try_from(rows.end).map_err(|_| RasterError::Overflow)?;
        let start_off = start.checked_mul(stride).ok_or(RasterError::Overflow)?;
        let end_off = end.checked_mul(stride).ok_or(RasterError::Overflow)?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve(end_off - start_off)
            .map_err(|_| RasterError::Alloc)?;
        pixels.extend_from_slice(&self.pixels[start_off..end_off]);
        Self::from_packed(self.width, height, pixels)
    }
}

pub fn stride_bytes(width: u16) -> usize {
    usize::from(width).div_ceil(8)
}

fn packed_len(width: u16, height: u32) -> Result<usize, RasterError> {
    let stride = stride_bytes(width);
    let rows = usize::try_from(height).map_err(|_| RasterError::Overflow)?;
    stride.checked_mul(rows).ok_or(RasterError::Overflow)
}

fn bit_mask(x: usize) -> u8 {
    0x80 >> (x % 8)
}

fn test_bit(pixels: &[u8], stride: usize, x: usize, y: usize) -> bool {
    pixels[y * stride + x / 8] & bit_mask(x) != 0
}

fn normalize_padding(width: u16, pixels: &mut [u8]) {
    let used = width % 8;
    if used == 0 {
        return;
    }
    let mask = 0xffu8 << (8 - used);
    let stride = stride_bytes(width);
    for row in pixels.chunks_mut(stride) {
        if let Some(last) = row.last_mut() {
            *last &= mask;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_width_is_not_a_length_error() {
        assert_eq!(
            Raster::from_packed(0, 1, vec![]).unwrap_err(),
            RasterError::ZeroWidth
        );
        assert_eq!(
            Raster::from_bits(0, 8, &[]).unwrap_err(),
            RasterError::ZeroWidth
        );
    }

    #[test]
    fn zero_height_is_not_a_length_error() {
        assert_eq!(
            Raster::from_packed(8, 0, vec![]).unwrap_err(),
            RasterError::ZeroHeight
        );
        assert_eq!(
            Raster::from_bits(8, 0, &[]).unwrap_err(),
            RasterError::ZeroHeight
        );
    }

    #[test]
    fn ragged_packed_length_is_rejected() {
        assert_eq!(
            Raster::from_packed(8, 1, vec![]).unwrap_err(),
            RasterError::LengthMismatch {
                expected: 1,
                got: 0
            }
        );
        assert_eq!(
            Raster::from_packed(8, 1, vec![0, 0]).unwrap_err(),
            RasterError::LengthMismatch {
                expected: 1,
                got: 2
            }
        );
        assert_eq!(
            Raster::from_bits(8, 1, &[true; 7]).unwrap_err(),
            RasterError::LengthMismatch {
                expected: 8,
                got: 7
            }
        );
    }

    #[test]
    fn one_by_65536_is_a_valid_raster() {
        let raster = Raster::from_packed(1, 65_536, vec![0u8; 65_536]).unwrap();
        assert_eq!(raster.width(), 1);
        assert_eq!(raster.height(), 65_536);
        assert_eq!(raster.stride(), 1);
        assert_eq!(raster.pixels().len(), 65_536);
        assert_eq!(raster.pixel(0, 65_535), Some(false));
        assert_eq!(raster.pixel(0, 65_536), None);

        let mut bits = vec![false; 65_536];
        bits[65_535] = true;
        let from_bits = Raster::from_bits(1, 65_536, &bits).unwrap();
        assert_eq!(from_bits.height(), 65_536);
        assert_eq!(from_bits.pixel(0, 65_535), Some(true));
        assert_eq!(from_bits.pixels().len(), 65_536);
    }

    #[test]
    fn padding_bits_normalize_to_one_representation() {
        for width in 1u16..=24 {
            let height = 2u32;
            let n = usize::from(width) * 2;
            let mut bits = vec![false; n];
            for (i, bit) in bits.iter_mut().enumerate() {
                *bit = i % 3 == 0;
            }
            let from_bits = Raster::from_bits(width, height, &bits).unwrap();
            let stride = from_bits.stride();
            assert_eq!(from_bits.pixels().len(), stride * 2);

            let mut dirty = from_bits.pixels().to_vec();
            if width % 8 != 0 {
                for row in 0..2 {
                    dirty[row * stride + stride - 1] |= 0x01;
                }
            }
            let from_packed = Raster::from_packed(width, height, dirty).unwrap();
            assert_eq!(from_packed, from_bits);
            assert_eq!(
                from_packed.pixels().len(),
                stride * usize::try_from(height).unwrap()
            );

            for y in 0..height {
                for x in 0..width {
                    assert_eq!(
                        from_packed.pixel(x, y),
                        Some(
                            bits[usize::try_from(y).unwrap() * usize::from(width) + usize::from(x)]
                        )
                    );
                }
            }

            let used = width % 8;
            if used != 0 {
                let mask = 0xffu8 << (8 - used);
                for row in from_packed.pixels().chunks(stride) {
                    assert_eq!(row[stride - 1] & !mask, 0);
                }
            }
        }
    }

    #[test]
    fn pixel_out_of_bounds_is_none() {
        let raster = Raster::from_bits(3, 2, &[true, false, true, false, true, false]).unwrap();
        assert_eq!(raster.pixel(0, 0), Some(true));
        assert_eq!(raster.pixel(2, 1), Some(false));
        assert_eq!(raster.pixel(3, 0), None);
        assert_eq!(raster.pixel(0, 2), None);
        assert_eq!(raster.pixel(3, 2), None);
    }

    #[test]
    fn slice_rows_is_checked_and_nonempty() {
        let bits = [
            true, false, false, false, false, false, false, false, false, true, false, false,
            false, false, false, false, true, true, false, false, false, false, false, false,
        ];
        let raster = Raster::from_bits(8, 3, &bits).unwrap();
        let mid = raster.slice_rows(1..2).unwrap();
        assert_eq!(mid.width(), 8);
        assert_eq!(mid.height(), 1);
        assert_eq!(mid.pixel(1, 0), Some(true));
        assert_eq!(mid.pixels(), &raster.pixels()[1..2]);

        let rest = raster.slice_rows(0..3).unwrap();
        assert_eq!(rest, raster);

        assert_eq!(
            raster.slice_rows(1..1).unwrap_err(),
            RasterError::InvalidRowRange
        );
        #[allow(clippy::reversed_empty_ranges)]
        let reversed = 2u32..1;
        assert_eq!(
            raster.slice_rows(reversed).unwrap_err(),
            RasterError::InvalidRowRange
        );
        assert_eq!(
            raster.slice_rows(0..4).unwrap_err(),
            RasterError::InvalidRowRange
        );
        assert_eq!(
            raster.slice_rows(3..4).unwrap_err(),
            RasterError::InvalidRowRange
        );
    }

    #[test]
    fn derived_length_equals_stored_bytes() {
        let raster = Raster::from_bits(9, 3, &[true; 27]).unwrap();
        assert_eq!(
            raster.pixels().len(),
            raster.stride() * usize::try_from(raster.height()).unwrap()
        );
        assert_eq!(
            raster.pixels(),
            &[
                0b1111_1111,
                0b1000_0000,
                0b1111_1111,
                0b1000_0000,
                0b1111_1111,
                0b1000_0000
            ]
        );
    }
}
