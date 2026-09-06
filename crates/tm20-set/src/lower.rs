//! Lower a painted page to Init / PC437 / Graphics / Feed 3 / Cut.
//!
//! Sheet composition lives in `validate → layout → paint`. This module does
//! not paint. `lower` consumes that shared result plus bands; it does not
//! keep a second full-page composer.

use crate::compose::{layout, paint, select_bands};
use crate::error::Error;
use crate::face::FaceTable;
use crate::frame::Sheet;
use crate::geometry::{Row, RowRange};
use tm20::command::{CodePage, Command};
use tm20::document::Document;
use tm20::graphics::{Graphics, GraphicsScale, max_height};
use tm20::{EncodeError, Raster};

/// Layout `sheet`, paint it, partition the page, and emit one encoded job.
pub fn lower(sheet: &Sheet<'_>, faces: &FaceTable) -> Result<Document, Error> {
    let checked = sheet.admit()?;
    let resolved = faces.resolve(&checked.face_requirements())?;
    let plan = layout(&checked, &resolved)?;
    let page = paint(&plan)?;
    lower_page(&page, plan.seams())
}

/// Partition an already-painted page and emit the job sequence.
///
/// Band math is [`select_bands`], using page-wide row coordinates.
pub fn lower_page(page: &Raster, seams: &[Row]) -> Result<Document, Error> {
    let cap = max_height(page.width());
    let ranges = select_bands(page.height(), cap, seams)?;
    lower_bands(page, &ranges)
}

/// Emit Init / PC437 / admitted Graphics / Feed 3 / Cut from known ranges.
///
/// Ranges must be a contiguous nonempty cover of `page`. Each band is narrowed
/// only through [`Graphics::new`].
pub fn lower_bands(page: &Raster, ranges: &[RowRange]) -> Result<Document, Error> {
    let mut cmds = Vec::new();
    cmds.try_reserve(
        ranges
            .len()
            .checked_add(4)
            .ok_or(Error::CoordinateOverflow)?,
    )
    .map_err(|_| Error::Alloc)?;
    cmds.push(Command::Init);
    cmds.push(Command::CodePage(CodePage::Pc437));
    let mut cursor = 0u32;
    for range in ranges {
        if range.start().get() != cursor || range.end().get() > page.height() {
            return Err(Error::CoordinateOverflow);
        }
        let raster = page.slice_rows(range.start().get()..range.end().get())?;
        cmds.push(Command::Graphics(admit_graphics(raster)?));
        cursor = range.end().get();
    }
    if cursor != page.height() {
        return Err(Error::CoordinateOverflow);
    }
    cmds.push(Command::Feed { lines: 3 });
    cmds.push(Command::Cut);
    Ok(Document::new(cmds))
}

fn admit_graphics(raster: Raster) -> Result<Graphics, Error> {
    Graphics::new(raster, GraphicsScale::Normal).map_err(|err| match err {
        EncodeError::GraphicsTooLong { .. } => Error::CoordinateOverflow,
        EncodeError::GraphicsPackedLen { expected, got } => {
            Error::Raster(tm20::RasterError::LengthMismatch { expected, got })
        }
        _ => Error::CoordinateOverflow,
    })
}

#[cfg(test)]
mod tests {
    use super::{admit_graphics, lower_bands};
    use crate::geometry::{Row, RowRange};
    use tm20::Raster;
    use tm20::command::{CodePage, Command};
    use tm20::encode::encode;
    use tm20::graphics::{GraphicsScale, max_height};

    fn range(start: u32, end: u32) -> RowRange {
        RowRange::new(Row::new(start), Row::new(end)).unwrap()
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

    fn graphics_of(doc: &tm20::Document) -> Vec<&tm20::Graphics> {
        doc.commands()
            .iter()
            .filter_map(|c| match c {
                Command::Graphics(g) => Some(g),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn lower_bands_is_init_pc437_graphics_feed_cut() {
        let page = Raster::from_bits(
            8,
            1,
            &[true, false, false, false, false, false, false, true],
        )
        .unwrap();
        let doc = lower_bands(&page, &[range(0, 1)]).unwrap();
        match doc.commands() {
            [
                Command::Init,
                Command::CodePage(CodePage::Pc437),
                Command::Graphics(g),
                Command::Feed { lines: 3 },
                Command::Cut,
            ] => {
                assert_eq!(g.scale(), GraphicsScale::Normal);
                assert_eq!(g.raster().width(), 8);
                assert_eq!(g.raster().height(), 1);
                assert_eq!(g.raster().pixels(), page.pixels());
                assert_eq!(g.raster().pixels(), &[0b1000_0001]);
            }
            other => panic!("unexpected lower sequence: {other:?}"),
        }
        assert!(
            !doc.commands()
                .iter()
                .any(|c| matches!(c, Command::PrintSpeed(_))),
            "typesetter must not inject PrintSpeed"
        );
        let bytes = encode(&doc).unwrap();
        assert_eq!(&bytes[..2], &[0x1b, 0x40]);
        assert!(bytes.windows(3).any(|w| w == [0x1d, b'(', b'L']));
        assert!(
            bytes.windows(3).any(|w| w == [0x1d, b'V', b'B']),
            "job ends in a cut (GS V B)"
        );
    }

    #[test]
    fn concatenated_band_bytes_equal_the_page() {
        let page = patterned(8, 2000);
        let cap = max_height(page.width());
        let ranges = vec![range(0, 910), range(910, 1820), range(1820, 2000)];
        assert!(ranges.iter().all(|r| r.height() <= u32::from(cap)));
        let doc = lower_bands(&page, &ranges).unwrap();
        let gs = graphics_of(&doc);
        assert_eq!(gs.len(), 3);
        let mut concat = Vec::new();
        for g in &gs {
            concat.extend_from_slice(g.raster().pixels());
            assert!(tm20::graphics::encode(g).is_ok());
        }
        assert_eq!(concat, page.pixels());
        assert_eq!(concat.last().copied(), page.pixels().last().copied());
        encode(&doc).unwrap();
    }

    #[test]
    fn one_row_one_bit_job_bytes_match_the_page() {
        let page = Raster::from_bits(1, 1, &[true]).unwrap();
        let doc = lower_bands(&page, &[range(0, 1)]).unwrap();
        let gs = graphics_of(&doc);
        assert_eq!(gs.len(), 1);
        assert_eq!(gs[0].raster().pixels(), &[0x80]);
        assert_eq!(gs[0].raster().pixels(), page.pixels());
    }

    #[test]
    fn dropped_or_padded_seams_are_rejected() {
        let page = patterned(8, 4);
        assert!(lower_bands(&page, &[range(0, 2)]).is_err());
        assert!(lower_bands(&page, &[range(1, 4)]).is_err());
        assert!(lower_bands(&page, &[range(0, 2), range(3, 4)]).is_err());
    }

    #[test]
    fn last_row_of_a_u16_overflow_page_is_in_the_job() {
        let page = patterned(1, 65_536);
        let cap = max_height(1);
        let first = u32::from(cap);
        let doc = lower_bands(&page, &[range(0, first), range(first, 65_536)]).unwrap();
        let gs = graphics_of(&doc);
        let mut concat = Vec::new();
        for g in &gs {
            assert!(tm20::graphics::encode(g).is_ok());
            concat.extend_from_slice(g.raster().pixels());
        }
        assert_eq!(concat, page.pixels());
        assert_eq!(concat[65_535], page.pixels()[65_535]);
        assert_eq!(
            gs.last().unwrap().raster().pixels().last().copied(),
            Some(page.pixels()[65_535])
        );
    }

    #[test]
    fn admit_graphics_rejects_an_overlong_band() {
        let tall = Raster::from_packed(576, 911, vec![0u8; 72 * 911]).unwrap();
        assert!(admit_graphics(tall).is_err());
    }
}
