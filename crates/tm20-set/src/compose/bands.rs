//! Minimum-count, latest-seam partition of a u32 page into fn=112 bands.

use tm20::graphics::{Graphics, GraphicsScale, max_height};
use tm20::{EncodeError, Raster};

use crate::error::Error;
use crate::geometry::{Row, RowRange};

/// Partition `[0, height)` into the fewest nonempty half-open ranges of
/// height ≤ `cap`. Among cuts that keep that count, take the latest seam in
/// each window. Seams outside the feasible window never increase the count.
pub fn select_bands(height: u32, cap: u16, seams: &[Row]) -> Result<Vec<RowRange>, Error> {
    let height = height.max(1);
    if cap == 0 {
        return Err(Error::CoordinateOverflow);
    }
    let cap = u32::from(cap);
    let n = height.div_ceil(cap);
    let mut bands = Vec::new();
    bands
        .try_reserve(usize::try_from(n).map_err(|_| Error::CoordinateOverflow)?)
        .map_err(|_| Error::Alloc)?;
    let mut start = 0u32;
    let mut remaining = n;
    while start < height {
        remaining = remaining.checked_sub(1).ok_or(Error::CoordinateOverflow)?;
        let rest = remaining
            .checked_mul(cap)
            .ok_or(Error::CoordinateOverflow)?;
        let lo = start
            .checked_add(1)
            .ok_or(Error::CoordinateOverflow)?
            .max(height.saturating_sub(rest));
        let hi = start.saturating_add(cap).min(height);
        if lo > hi {
            return Err(Error::CoordinateOverflow);
        }
        let cut = seams
            .iter()
            .map(|r| r.get())
            .filter(|&y| y >= lo && y <= hi)
            .max()
            .unwrap_or(hi);
        let range =
            RowRange::new(Row::new(start), Row::new(cut)).ok_or(Error::CoordinateOverflow)?;
        bands.push(range);
        start = cut;
    }
    Ok(bands)
}

/// Slice `page` on `ranges` and admit each band through [`Graphics::new`].
pub fn graphics_bands(page: &Raster, ranges: &[RowRange]) -> Result<Vec<Graphics>, Error> {
    let mut out = Vec::new();
    out.try_reserve(ranges.len()).map_err(|_| Error::Alloc)?;
    let mut cursor = 0u32;
    for range in ranges {
        if range.start().get() != cursor || range.end().get() > page.height() {
            return Err(Error::CoordinateOverflow);
        }
        let raster = page.slice_rows(range.start().get()..range.end().get())?;
        out.push(admit_graphics(raster)?);
        cursor = range.end().get();
    }
    if cursor != page.height() {
        return Err(Error::CoordinateOverflow);
    }
    Ok(out)
}

/// Partition a painted page with the encode cap for its width.
pub fn page_bands(page: &Raster, seams: &[Row]) -> Result<Vec<Graphics>, Error> {
    let cap = max_height(page.width());
    let ranges = select_bands(page.height(), cap, seams)?;
    graphics_bands(page, &ranges)
}

pub(super) fn admit_graphics(raster: Raster) -> Result<Graphics, Error> {
    Graphics::new(raster, GraphicsScale::Normal).map_err(graphics_error)
}

#[allow(clippy::needless_pass_by_value)]
fn graphics_error(err: EncodeError) -> Error {
    match err {
        EncodeError::GraphicsTooLong { .. } => Error::CoordinateOverflow,
        EncodeError::GraphicsPackedLen { expected, got } => {
            Error::Raster(tm20::RasterError::LengthMismatch { expected, got })
        }
        _ => Error::CoordinateOverflow,
    }
}

#[cfg(test)]
mod pack_tests {
    use super::{graphics_bands, page_bands, select_bands};
    use crate::geometry::{Row, RowRange};
    use tm20::Raster;
    use tm20::graphics::{GraphicsScale, max_height};

    fn ranges(height: u32, cap: u16, seams: &[u32]) -> Vec<(u32, u32)> {
        select_bands(
            height,
            cap,
            &seams.iter().copied().map(Row::new).collect::<Vec<_>>(),
        )
        .unwrap()
        .into_iter()
        .map(|r| (r.start().get(), r.end().get()))
        .collect()
    }

    fn assert_partition(height: u32, cap: u16, seams: &[u32], got: &[(u32, u32)]) {
        assert!(!got.is_empty());
        assert_eq!(got[0].0, 0);
        assert_eq!(got.last().unwrap().1, height.max(1));
        for window in got.windows(2) {
            assert_eq!(window[0].1, window[1].0);
        }
        for &(start, end) in got {
            assert!(start < end);
            assert!(end - start <= u32::from(cap.max(1)));
        }
        let min = height.max(1).div_ceil(u32::from(cap.max(1)));
        assert_eq!(got.len() as u32, min);
        let _ = seams;
    }

    #[test]
    fn short_sheet_is_one_band() {
        let got = ranges(500, 910, &[500]);
        assert_eq!(got, vec![(0, 500)]);
        assert_partition(500, 910, &[500], &got);
    }

    #[test]
    fn latest_seam_in_the_min_count_window() {
        let got = ranges(1000, 910, &[100, 200, 400, 800, 1000]);
        assert_eq!(got, vec![(0, 800), (800, 1000)]);
        assert_partition(1000, 910, &[100, 200, 400, 800, 1000], &got);
    }

    #[test]
    fn h_1818_is_two_payloads_not_three() {
        let got = ranges(1818, 910, &[1818]);
        assert_eq!(got, vec![(0, 910), (910, 1818)]);
        assert_partition(1818, 910, &[1818], &got);
    }

    #[test]
    fn missing_seam_splits_at_the_cap() {
        let got = ranges(1000, 910, &[1000]);
        assert_eq!(got, vec![(0, 910), (910, 1000)]);
        assert_partition(1000, 910, &[1000], &got);
    }

    #[test]
    fn h_2000_is_three_payloads() {
        let got = ranges(2000, 910, &[2000]);
        assert_eq!(got, vec![(0, 910), (910, 1820), (1820, 2000)]);
        assert_partition(2000, 910, &[2000], &got);
    }

    #[test]
    fn a_seam_outside_the_window_does_not_add_a_payload() {
        let got = ranges(1818, 910, &[51, 1818]);
        assert_eq!(got, vec![(0, 910), (910, 1818)]);
        assert_partition(1818, 910, &[51, 1818], &got);
    }

    #[test]
    fn head_seam_stays_attached_when_that_keeps_min_count() {
        let got = ranges(1818, 910, &[51, 1818]);
        assert_eq!(got[0], (0, 910));
        assert_ne!(got[0], (0, 51));
    }

    #[test]
    fn zero_height_is_one_row() {
        let got = ranges(0, 910, &[]);
        assert_eq!(got, vec![(0, 1)]);
    }

    #[test]
    fn zero_cap_is_unallocatable() {
        assert!(select_bands(10, 0, &[]).is_err());
    }

    #[test]
    fn ranges_are_rowrange_values() {
        let got = select_bands(8, 5, &[Row::new(8)]).unwrap();
        assert_eq!(
            got,
            vec![
                RowRange::new(Row::ZERO, Row::new(5)).unwrap(),
                RowRange::new(Row::new(5), Row::new(8)).unwrap(),
            ]
        );
    }

    fn patterned(width: u16, height: u32) -> Raster {
        let w = usize::from(width);
        let mut bits = vec![false; w * usize::try_from(height).unwrap()];
        for y in 0..height {
            let row = usize::try_from(y).unwrap() * w;
            bits[row] = true;
            bits[row + usize::try_from(y).unwrap() % w] = true;
            if y + 1 == height {
                bits[row + w - 1] = true;
            }
        }
        Raster::from_bits(width, height, &bits).unwrap()
    }

    fn assert_reconstruct(page: &Raster, seams: &[u32]) {
        let cap = max_height(page.width());
        let rows: Vec<Row> = seams.iter().copied().map(Row::new).collect();
        let ranges = select_bands(page.height(), cap, &rows).unwrap();
        assert_partition(
            page.height(),
            cap,
            seams,
            &ranges
                .iter()
                .map(|r| (r.start().get(), r.end().get()))
                .collect::<Vec<_>>(),
        );
        let bands = graphics_bands(page, &ranges).unwrap();
        assert_eq!(bands.len(), ranges.len());
        let mut concat = Vec::new();
        for (band, range) in bands.iter().zip(&ranges) {
            assert_eq!(band.scale(), GraphicsScale::Normal);
            assert_eq!(band.raster().width(), page.width());
            assert_eq!(band.raster().height(), range.height());
            assert!(band.raster().height() <= u32::from(cap));
            concat.extend_from_slice(band.raster().pixels());
        }
        assert_eq!(concat, page.pixels());
        let via_page = page_bands(page, &rows).unwrap();
        let mut again = Vec::new();
        for band in &via_page {
            again.extend_from_slice(band.raster().pixels());
        }
        assert_eq!(again, page.pixels());
    }

    #[test]
    fn concatenated_bands_equal_an_1800_row_page() {
        let page = patterned(8, 1800);
        assert_eq!(page.pixel(0, 1799), Some(true));
        assert_eq!(page.pixel(7, 1799), Some(true));
        assert_reconstruct(&page, &[1800]);
        assert_eq!(
            page.pixels()[1799],
            page_bands(&page, &[Row::new(1800)])
                .unwrap()
                .last()
                .unwrap()
                .raster()
                .pixels()
                .last()
                .copied()
                .unwrap()
        );
    }

    #[test]
    fn concatenated_bands_preserve_row_65535() {
        let page = patterned(1, 65_536);
        assert_eq!(page.height(), 65_536);
        assert_eq!(page.pixel(0, 65_535), Some(true));
        assert_reconstruct(&page, &[]);
        let cap = max_height(1);
        assert!(u32::from(cap) < 65_536);
        let bands = page_bands(&page, &[]).unwrap();
        assert_eq!(
            bands.last().unwrap().raster().pixels().last().copied(),
            Some(page.pixels()[65_535])
        );
        assert!(bands.iter().all(|b| { tm20::graphics::encode(b).is_ok() }));
    }

    #[test]
    fn one_row_one_bit_reconstructs() {
        let page = Raster::from_bits(1, 1, &[true]).unwrap();
        assert_eq!(page.pixels(), &[0x80]);
        assert_reconstruct(&page, &[1]);
    }

    #[test]
    fn non_byte_width_reconstructs_without_padding_drift() {
        let mut bits = vec![false; 9 * 3];
        bits[0] = true;
        bits[8] = true;
        bits[9 + 8] = true;
        bits[18] = true;
        let page = Raster::from_bits(9, 3, &bits).unwrap();
        assert_eq!(
            page.pixels(),
            &[
                0b1000_0000,
                0b1000_0000,
                0x00,
                0b1000_0000,
                0b1000_0000,
                0x00
            ]
        );
        assert_reconstruct(&page, &[3]);
    }
}
