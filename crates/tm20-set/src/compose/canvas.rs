//! Evaluate a measured [`LayoutPlan`] onto a validated page [`Raster`].
//!
//! Paint does not wrap, attach notes, or choose seams. It allocates the plan
//! extent with checked arithmetic and clips signed coordinates with wide
//! intermediates. Glyph runs blit shared strikes; they do not look up faces.

use tm20::Raster;

use super::{DrawOp, LayoutPlan};
use crate::error::Error;
use crate::face::ShapedRun;
use crate::geometry::{Advance, Clip, RowRange};

/// Evaluate `plan` onto one packed page. A zero-height plan is a single white row.
pub fn paint(plan: &LayoutPlan) -> Result<Raster, Error> {
    let width = plan.measure().get();
    let mut page = Page::allocate(width, plan.height().max(1))?;
    let page_clip = Clip::full(plan.measure());
    for op in plan.ops() {
        match op {
            DrawOp::GlyphRun {
                run,
                x,
                baseline,
                clip,
            } => paint_glyph_run(&mut page, run, *x, *baseline, clip.intersection(page_clip))?,
            DrawOp::Bitmap {
                raster,
                x,
                top,
                clip,
            } => blit_src(
                &mut page,
                raster,
                *x,
                i64::from(top.get()),
                clip.intersection(page_clip),
            )?,
            DrawOp::Rule { rows, clip } => {
                fill_rule(&mut page, *rows, clip.intersection(page_clip))?;
            }
        }
    }
    page.into_raster()
}

/// Checked page buffer. Height is the allocated plan extent, never a u16.
struct Page {
    width: u16,
    height: u32,
    stride: usize,
    bits: Vec<u8>,
}

impl Page {
    fn allocate(width: u16, height: u32) -> Result<Self, Error> {
        if width == 0 {
            return Err(Error::UnsupportedMeasure);
        }
        let height = height.max(1);
        let stride = stride_bytes(width);
        let rows = usize::try_from(height).map_err(|_| Error::CoordinateOverflow)?;
        let total = stride.checked_mul(rows).ok_or(Error::CoordinateOverflow)?;
        let mut bits = Vec::new();
        bits.try_reserve(total).map_err(|_| Error::Alloc)?;
        bits.resize(total, 0);
        Ok(Self {
            width,
            height,
            stride,
            bits,
        })
    }

    fn into_raster(self) -> Result<Raster, Error> {
        Raster::from_packed(self.width, self.height, self.bits).map_err(Error::from)
    }
}

fn stride_bytes(width: u16) -> usize {
    usize::from(width).div_ceil(8)
}

/// Ink a packed source at a signed origin. Negative or out-of-range dots clip;
/// they never wrap through a narrowing cast.
fn blit_src(page: &mut Page, src: &Raster, x0: i32, y0: i64, clip: Clip) -> Result<(), Error> {
    blit_packed(page, src.pixels(), src.width(), src.height(), x0, y0, clip)
}

fn blit_packed(
    page: &mut Page,
    bits: &[u8],
    width: u16,
    height: u32,
    x0: i32,
    y0: i64,
    clip: Clip,
) -> Result<(), Error> {
    if width == 0 || height == 0 {
        return Ok(());
    }
    let Some((clip0, clip1)) = visible_x(clip, page.width) else {
        return Ok(());
    };
    let stride = stride_bytes(width);
    for sy in 0..height {
        let Some(y) = y0.checked_add(i64::from(sy)) else {
            return Err(Error::CoordinateOverflow);
        };
        let Ok(y) = u32::try_from(y) else {
            continue;
        };
        if y >= page.height {
            continue;
        }
        for sx in 0..width {
            if !packed_bit(bits, stride, sx, sy) {
                continue;
            }
            let Some(x) = i64::from(x0).checked_add(i64::from(sx)) else {
                return Err(Error::CoordinateOverflow);
            };
            let Ok(x) = u16::try_from(x) else {
                continue;
            };
            if x < clip0 || x >= clip1 {
                continue;
            }
            set_dot(page, x, y);
        }
    }
    Ok(())
}

fn packed_bit(bits: &[u8], stride: usize, x: u16, y: u32) -> bool {
    let Ok(row) = usize::try_from(y) else {
        return false;
    };
    let i = row
        .checked_mul(stride)
        .and_then(|off| off.checked_add(usize::from(x) / 8));
    match i.and_then(|i| bits.get(i)) {
        Some(byte) => byte & (0x80 >> (x % 8)) != 0,
        None => false,
    }
}

fn fill_rule(page: &mut Page, rows: RowRange, clip: Clip) -> Result<(), Error> {
    let start = rows.start().get();
    let end = rows.end().get();
    if start >= end {
        return Err(Error::CoordinateOverflow);
    }
    if end > page.height {
        return Err(Error::CoordinateOverflow);
    }
    let Some((x0, x1)) = visible_x(clip, page.width) else {
        return Ok(());
    };
    let mut y = start;
    while y < end {
        fill_span(page, y, x0, x1);
        y = y.checked_add(1).ok_or(Error::CoordinateOverflow)?;
    }
    Ok(())
}

fn visible_x(clip: Clip, width: u16) -> Option<(u16, u16)> {
    if clip.is_empty() {
        return None;
    }
    let start = clip.start();
    let end = clip.end().min(width);
    (start < end).then_some((start, end))
}

fn fill_span(page: &mut Page, y: u32, x0: u16, x1: u16) {
    if y >= page.height || x0 >= x1 {
        return;
    }
    let Ok(row) = usize::try_from(y) else {
        return;
    };
    let row = row * page.stride;
    let mut x = usize::from(x0);
    let x1 = usize::from(x1);
    while x < x1 && !x.is_multiple_of(8) {
        page.bits[row + x / 8] |= 0x80 >> (x % 8);
        x += 1;
    }
    while x + 8 <= x1 {
        page.bits[row + x / 8] = 0xFF;
        x += 8;
    }
    while x < x1 {
        page.bits[row + x / 8] |= 0x80 >> (x % 8);
        x += 1;
    }
}

fn set_dot(page: &mut Page, x: u16, y: u32) {
    if x >= page.width || y >= page.height {
        return;
    }
    let Ok(row) = usize::try_from(y) else {
        return;
    };
    page.bits[row * page.stride + usize::from(x) / 8] |= 0x80 >> (x % 8);
}

/// Empty ink is a no-op. Nonempty ink blits each shared strike; no face lookup.
fn paint_glyph_run(
    page: &mut Page,
    run: &ShapedRun,
    x: Advance,
    baseline: Advance,
    clip: Clip,
) -> Result<(), Error> {
    if run.ink().is_empty() {
        return Ok(());
    }
    for g in run.glyphs() {
        let strike = g.strike();
        let origin_x = x
            .checked_add(g.x())
            .ok_or(Error::CoordinateOverflow)?
            .round_dots()?
            .checked_add(strike.left())
            .ok_or(Error::CoordinateOverflow)?;
        let origin_y = baseline
            .checked_sub(g.y())
            .ok_or(Error::CoordinateOverflow)?
            .round_dots()?
            .checked_sub(strike.top())
            .ok_or(Error::CoordinateOverflow)
            .map(i64::from)?;
        blit_packed(
            page,
            strike.bits(),
            strike.width(),
            u32::from(strike.height()),
            origin_x,
            origin_y,
            clip,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::bands::{graphics_bands, select_bands};
    use super::{Page, blit_src, packed_bit, paint, stride_bytes};
    use crate::compose::{DrawOp, LayoutPlan};
    use crate::error::Error;
    use crate::face::{Cut, FaceTable, ShapedRun};
    use crate::geometry::{Advance, Clip, Measure, Row, RowRange};
    use crate::size::TextSize;
    use tm20::Raster;
    use tm20::graphics::{GraphicsScale, max_height};

    fn measure(w: u16) -> Measure {
        Measure::new(w).expect("admitted measure")
    }

    fn full(w: u16) -> Clip {
        Clip::full(measure(w))
    }

    fn plan(width: u16, height: u32, ops: Vec<DrawOp>, seams: &[u32]) -> LayoutPlan {
        LayoutPlan::new(
            measure(width),
            height,
            ops,
            seams.iter().copied().map(Row::new).collect(),
        )
        .expect("nonzero height")
    }

    fn bits(width: u16, height: u32, pixels: &[bool]) -> Raster {
        Raster::from_bits(width, height, pixels).expect("test raster")
    }

    fn packed(width: u16, height: u32, pixels: Vec<u8>) -> Raster {
        Raster::from_packed(width, height, pixels).expect("packed raster")
    }

    #[test]
    fn empty_plan_is_one_white_row() {
        let page = paint(&plan(8, 1, vec![], &[])).unwrap();
        assert_eq!(page.width(), 8);
        assert_eq!(page.height(), 1);
        assert_eq!(page.pixels(), &[0x00]);
        assert_eq!(page.pixel(0, 0), Some(false));
    }

    #[test]
    fn one_bit_width_preserves_the_dot_and_padding() {
        let ink = bits(1, 1, &[true]);
        let page = paint(&plan(
            1,
            1,
            vec![DrawOp::Bitmap {
                raster: ink,
                x: 0,
                top: Row::ZERO,
                clip: full(1),
            }],
            &[],
        ))
        .unwrap();
        assert_eq!(page.pixels(), &[0x80]);
        assert_eq!(page.pixel(0, 0), Some(true));
    }

    #[test]
    fn non_byte_width_normalizes_padding_bits() {
        let mut px = vec![false; 9];
        px[0] = true;
        px[8] = true;
        let ink = bits(9, 1, &px);
        let page = paint(&plan(
            9,
            1,
            vec![DrawOp::Bitmap {
                raster: ink,
                x: 0,
                top: Row::ZERO,
                clip: full(9),
            }],
            &[],
        ))
        .unwrap();
        assert_eq!(page.pixels(), &[0b1000_0000, 0b1000_0000]);
        assert_eq!(page.pixel(8, 0), Some(true));
    }

    #[test]
    fn rule_fills_exact_rows_and_clip() {
        let rows = RowRange::new(Row::new(1), Row::new(3)).unwrap();
        let clip = Clip::new(2, 6).unwrap();
        let page = paint(&plan(8, 4, vec![DrawOp::Rule { rows, clip }], &[4])).unwrap();
        assert_eq!(page.pixels(), &[0x00, 0b0011_1100, 0b0011_1100, 0x00]);
    }

    #[test]
    fn bitmap_lands_on_exact_coordinates() {
        let ink = bits(2, 2, &[true, false, false, true]);
        let page = paint(&plan(
            8,
            4,
            vec![DrawOp::Bitmap {
                raster: ink,
                x: 3,
                top: Row::new(1),
                clip: full(8),
            }],
            &[],
        ))
        .unwrap();
        assert_eq!(page.pixel(3, 1), Some(true));
        assert_eq!(page.pixel(4, 1), Some(false));
        assert_eq!(page.pixel(3, 2), Some(false));
        assert_eq!(page.pixel(4, 2), Some(true));
        assert_eq!(page.pixels(), &[0x00, 0b0001_0000, 0b0000_1000, 0x00]);
    }

    #[test]
    fn negative_x_bearing_clips_without_wrapping() {
        let ink = bits(8, 1, &[true; 8]);
        let page = paint(&plan(
            8,
            1,
            vec![DrawOp::Bitmap {
                raster: ink,
                x: -3,
                top: Row::ZERO,
                clip: full(8),
            }],
            &[],
        ))
        .unwrap();
        assert_eq!(page.pixels(), &[0b1111_1000]);
        assert_eq!(page.pixel(0, 0), Some(true));
        assert_eq!(page.pixel(4, 0), Some(true));
        assert_eq!(page.pixel(5, 0), Some(false));
    }

    #[test]
    fn negative_y_bearing_clips_without_wrapping_onto_the_last_row() {
        let mut page = Page::allocate(8, 2).unwrap();
        let ink = bits(8, 2, &[true; 16]);
        blit_src(&mut page, &ink, 0, -1, full(8)).unwrap();
        let page = page.into_raster().unwrap();
        assert_eq!(page.height(), 2);
        assert_eq!(page.pixels(), &[0xff, 0x00]);
        assert_eq!(page.pixel(0, 0), Some(true));
        assert_eq!(page.pixel(0, 1), Some(false));
    }

    #[test]
    fn out_of_range_positive_x_does_not_wrap() {
        let ink = bits(1, 1, &[true]);
        let page = paint(&plan(
            8,
            1,
            vec![DrawOp::Bitmap {
                raster: ink,
                x: 8,
                top: Row::ZERO,
                clip: full(8),
            }],
            &[],
        ))
        .unwrap();
        assert_eq!(page.pixels(), &[0x00]);
    }

    #[test]
    fn last_row_of_an_1800_row_page_survives() {
        let ink = bits(
            8,
            1,
            &[true, false, false, false, false, false, false, true],
        );
        let page = paint(&plan(
            8,
            1800,
            vec![DrawOp::Bitmap {
                raster: ink,
                x: 0,
                top: Row::new(1799),
                clip: full(8),
            }],
            &[],
        ))
        .unwrap();
        assert_eq!(page.height(), 1800);
        assert_eq!(page.pixels().len(), 1800);
        assert_eq!(&page.pixels()[..1799], &[0u8; 1799]);
        assert_eq!(page.pixels()[1799], 0b1000_0001);
        assert_eq!(page.pixel(0, 1799), Some(true));
        assert_eq!(page.pixel(7, 1799), Some(true));
        assert_eq!(page.pixel(0, 1798), Some(false));
    }

    #[test]
    fn last_row_above_u16_survives() {
        let ink = bits(1, 1, &[true]);
        let page = paint(&plan(
            1,
            65_536,
            vec![DrawOp::Bitmap {
                raster: ink,
                x: 0,
                top: Row::new(65_535),
                clip: full(1),
            }],
            &[],
        ))
        .unwrap();
        assert_eq!(page.height(), 65_536);
        assert_eq!(page.pixels().len(), 65_536);
        assert_eq!(page.pixels()[65_534], 0x00);
        assert_eq!(page.pixels()[65_535], 0x80);
        assert_eq!(page.pixel(0, 65_535), Some(true));
    }

    #[test]
    fn rule_past_the_page_is_an_error_not_a_short_raster() {
        let rows = RowRange::new(Row::ZERO, Row::new(3)).unwrap();
        let err = paint(&plan(
            8,
            2,
            vec![DrawOp::Rule {
                rows,
                clip: full(8),
            }],
            &[],
        ))
        .unwrap_err();
        assert!(matches!(err, Error::CoordinateOverflow));
    }

    #[test]
    fn painted_page_bands_reconstruct_exact_bytes() {
        let mut pixels = vec![0u8; 2000];
        for (y, slot) in pixels.iter_mut().enumerate() {
            *slot = (y as u8).rotate_left(1);
        }
        let src = packed(8, 2000, pixels.clone());
        let page = paint(&plan(
            8,
            2000,
            vec![DrawOp::Bitmap {
                raster: src,
                x: 0,
                top: Row::ZERO,
                clip: full(8),
            }],
            &[400, 800, 2000],
        ))
        .unwrap();
        assert_eq!(page.pixels(), pixels);

        let cap = max_height(page.width());
        let ranges = select_bands(
            page.height(),
            cap,
            &[Row::new(400), Row::new(800), Row::new(2000)],
        )
        .unwrap();
        let bands = graphics_bands(&page, &ranges).unwrap();
        let mut concat = Vec::new();
        for band in &bands {
            assert!(band.raster().height() <= u32::from(cap));
            assert_eq!(band.scale(), GraphicsScale::Normal);
            concat.extend_from_slice(band.raster().pixels());
        }
        assert_eq!(concat, page.pixels());
        assert_eq!(concat, pixels);
    }

    fn house_table() -> FaceTable {
        let mut table = FaceTable::new();
        table.absorb(
            std::fs::read("/System/Library/Fonts/Helvetica.ttc")
                .expect("Helvetica.ttc not on this machine — locked house face required"),
        );
        table.absorb(
            std::fs::read("/System/Library/Fonts/Menlo.ttc")
                .expect("Menlo.ttc not on this machine — locked house face required"),
        );
        table
    }

    fn manual_strike_page(
        width: u16,
        height: u32,
        run: &ShapedRun,
        x: Advance,
        baseline: Advance,
    ) -> Vec<u8> {
        let stride = stride_bytes(width);
        let mut bits = vec![0u8; stride * usize::try_from(height).unwrap()];
        for g in run.glyphs() {
            let mask = g.strike();
            let origin_x = x.checked_add(g.x()).unwrap().round_dots().unwrap() + mask.left();
            let origin_y = i64::from(
                baseline
                    .checked_sub(g.y())
                    .unwrap()
                    .round_dots()
                    .unwrap()
                    .checked_sub(mask.top())
                    .unwrap(),
            );
            let src_stride = stride_bytes(mask.width());
            for sy in 0..u32::from(mask.height()) {
                let Ok(y) = u32::try_from(origin_y + i64::from(sy)) else {
                    continue;
                };
                if y >= height {
                    continue;
                }
                for sx in 0..mask.width() {
                    if !packed_bit(mask.bits(), src_stride, sx, sy) {
                        continue;
                    }
                    let Ok(dx) = u16::try_from(i64::from(origin_x) + i64::from(sx)) else {
                        continue;
                    };
                    if dx >= width {
                        continue;
                    }
                    let row = usize::try_from(y).unwrap() * stride;
                    bits[row + usize::from(dx) / 8] |= 0x80 >> (dx % 8);
                }
            }
        }
        bits
    }

    #[test]
    fn empty_glyph_run_is_a_white_noop() {
        let page = paint(&plan(
            8,
            2,
            vec![DrawOp::GlyphRun {
                run: ShapedRun::empty(),
                x: Advance::ZERO,
                baseline: Advance::from_dots(1).unwrap(),
                clip: full(8),
            }],
            &[],
        ))
        .unwrap();
        assert_eq!(page.pixels(), &[0x00, 0x00]);
    }

    #[test]
    fn nonempty_glyph_run_paints_strike_bytes() {
        let table = house_table();
        let roman = table.text(Cut::Roman).expect("Helvetica Roman");
        let run = roman.shape_run("H", TextSize::Pt11).expect("shape H");
        assert!(!run.ink().is_empty());
        let baseline = Advance::from_dots(20).unwrap();
        let painted = paint(&plan(
            64,
            40,
            vec![DrawOp::GlyphRun {
                run: run.clone(),
                x: Advance::ZERO,
                baseline,
                clip: full(64),
            }],
            &[],
        ))
        .unwrap();
        assert!(
            painted.pixels().iter().any(|&b| b != 0),
            "nonempty ink must paint"
        );
        assert_eq!(
            painted.pixels(),
            manual_strike_page(64, 40, &run, Advance::ZERO, baseline)
        );
    }

    #[test]
    fn negative_glyph_bearing_clips_without_wrapping() {
        let table = house_table();
        let italic = table.text(Cut::Italic).expect("Helvetica Oblique");
        let run = ["f", "j", "y", "T", "A"]
            .into_iter()
            .find_map(|s| {
                let r = italic.shape_run(s, TextSize::Pt11).ok()?;
                let g = r.glyphs().first()?;
                if g.strike().left() < 0 || r.ink().x0().units() < 0 {
                    Some(r)
                } else {
                    None
                }
            })
            .expect("need a left-bearing glyph");
        let g = &run.glyphs()[0];
        let baseline = Advance::from_dots(g.strike().top().max(1)).unwrap();
        let painted = paint(&plan(
            32,
            40,
            vec![DrawOp::GlyphRun {
                run: run.clone(),
                x: Advance::ZERO,
                baseline,
                clip: full(32),
            }],
            &[],
        ))
        .unwrap();
        assert!(painted.pixels().iter().any(|&b| b != 0));
        assert_eq!(
            painted.pixels(),
            manual_strike_page(32, 40, &run, Advance::ZERO, baseline)
        );
    }

    #[test]
    fn origin_y_rounds_the_subtracted_26_6_once() {
        let table = house_table();
        let roman = table.text(Cut::Roman).expect("Helvetica Roman");
        let run = roman
            .shape_run("H", TextSize::Pt11)
            .expect("H")
            .translate(Advance::ZERO, Advance::from_units(1))
            .expect("raise 1 unit");
        let g = &run.glyphs()[0];
        let baseline = Advance::from_dots(20)
            .unwrap()
            .checked_add(Advance::from_units(32))
            .unwrap();
        let want = baseline
            .checked_sub(g.y())
            .unwrap()
            .round_dots()
            .unwrap()
            .checked_sub(g.strike().top())
            .unwrap();
        let split = baseline.round_dots().unwrap() - g.y().round_dots().unwrap() - g.strike().top();
        assert_ne!(
            want, split,
            "this fixture must distinguish the two rounding orders"
        );
        let painted = paint(&plan(
            64,
            40,
            vec![DrawOp::GlyphRun {
                run: run.clone(),
                x: Advance::ZERO,
                baseline,
                clip: full(64),
            }],
            &[],
        ))
        .unwrap();
        let first = (0..painted.height())
            .find(|&y| (0..painted.width()).any(|x| painted.pixel(x, y) == Some(true)));
        let expect_row = u32::try_from(want.max(0)).unwrap();
        assert_eq!(
            first,
            Some(expect_row),
            "painted ink starts at round(baseline - g.y()) - top, not split rounds"
        );
    }
}
