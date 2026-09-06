//! 1-bit page and protocol graphics as a PNG for screen inspection. Not a print path.

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use tm20::Raster;
use tm20::graphics::Graphics;

use crate::error::Error;

/// 2× nearest-neighbor page view of a packed raster. No protocol magnification.
pub fn preview_raster(raster: &Raster) -> Result<Vec<u8>, Error> {
    let (pw, ph, mut raw) = alloc_preview(u32::from(raster.width()), raster.height())?;
    paint_scaled(
        &mut raw,
        pw,
        0,
        raster.width(),
        raster.height(),
        1,
        1,
        |x, y| raster.pixel(x, y) == Some(true),
    )?;
    encode_luma(&raw, pw, ph)
}

/// Unpack `g` to a print-scale-aware 2× nearest-neighbor Luma PNG (black ink on white).
pub fn preview_png(g: &Graphics) -> Result<Vec<u8>, Error> {
    preview_pngs(std::iter::once(g))
}

/// Stitch bands top-to-bottom at their commanded print scale, then the same 2× PNG.
///
/// Equal *effective* widths are required. Incompatible bands fail before allocation.
/// An empty iterator keeps the historical 2×2 white preview of one logical pixel.
pub fn preview_pngs<'a, I>(bands: I) -> Result<Vec<u8>, Error>
where
    I: IntoIterator<Item = &'a Graphics>,
{
    let bands: Vec<&Graphics> = bands.into_iter().collect();
    if bands.is_empty() {
        return encode_luma(&[255, 255, 255, 255], 2, 2);
    }

    let mut views = Vec::new();
    views.try_reserve(bands.len()).map_err(|_| Error::Alloc)?;
    let mut eff_w = None;
    let mut eff_h = 0u32;
    for g in &bands {
        let view = BandView::from_graphics(g)?;
        match eff_w {
            None => eff_w = Some(view.eff_w),
            Some(w) if w != view.eff_w => return Err(Error::InvalidImage),
            Some(_) => {}
        }
        eff_h = checked_add(eff_h, view.eff_h)?;
        views.push(view);
    }
    let eff_w = eff_w.ok_or(Error::InvalidImage)?;
    let (pw, ph, mut raw) = alloc_preview(eff_w, eff_h)?;

    let mut y0 = 0u32;
    for view in &views {
        paint_scaled(
            &mut raw,
            pw,
            y0,
            view.raster.width(),
            view.raster.height(),
            view.sx,
            view.sy,
            |x, y| view.raster.pixel(x, y) == Some(true),
        )?;
        y0 = checked_add(y0, checked_mul(view.eff_h, 2)?)?;
    }
    encode_luma(&raw, pw, ph)
}

struct BandView<'a> {
    raster: &'a Raster,
    sx: u8,
    sy: u8,
    eff_w: u32,
    eff_h: u32,
}

impl<'a> BandView<'a> {
    fn from_graphics(g: &'a Graphics) -> Result<Self, Error> {
        let raster = g.raster();
        let (sx, sy) = g.scale().factors();
        let eff_w = checked_mul(u32::from(raster.width()), u32::from(sx))?;
        let eff_h = checked_mul(raster.height(), u32::from(sy))?;
        if eff_w == 0 || eff_h == 0 {
            return Err(Error::InvalidImage);
        }
        Ok(Self {
            raster,
            sx,
            sy,
            eff_w,
            eff_h,
        })
    }
}

fn alloc_preview(eff_w: u32, eff_h: u32) -> Result<(u32, u32, Vec<u8>), Error> {
    let pw = checked_mul(eff_w, 2)?;
    let ph = checked_mul(eff_h, 2)?;
    let n = usize::try_from(checked_mul(pw, ph)?).map_err(|_| Error::CoordinateOverflow)?;
    let mut raw = Vec::new();
    raw.try_reserve(n).map_err(|_| Error::Alloc)?;
    raw.resize(n, 255);
    Ok((pw, ph, raw))
}

#[allow(clippy::too_many_arguments)]
fn paint_scaled(
    raw: &mut [u8],
    pw: u32,
    y0: u32,
    width: u16,
    height: u32,
    sx: u8,
    sy: u8,
    is_ink: impl Fn(u16, u32) -> bool,
) -> Result<(), Error> {
    let cell_w = checked_mul(u32::from(sx), 2)?;
    let cell_h = checked_mul(u32::from(sy), 2)?;
    for y in 0..height {
        for x in 0..width {
            if !is_ink(x, y) {
                continue;
            }
            let x0 = checked_mul(u32::from(x), cell_w)?;
            let yy = checked_add(y0, checked_mul(y, cell_h)?)?;
            for dy in 0..cell_h {
                for dx in 0..cell_w {
                    let px = checked_add(x0, dx)?;
                    let py = checked_add(yy, dy)?;
                    let idx = usize::try_from(checked_add(checked_mul(py, pw)?, px)?)
                        .map_err(|_| Error::CoordinateOverflow)?;
                    if idx >= raw.len() || px >= pw {
                        return Err(Error::CoordinateOverflow);
                    }
                    raw[idx] = 0;
                }
            }
        }
    }
    Ok(())
}

fn encode_luma(raw: &[u8], width: u32, height: u32) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(raw, width, height, ExtendedColorType::L8)
        .map_err(Error::ImageDetail)?;
    Ok(out)
}

fn checked_mul(a: u32, b: u32) -> Result<u32, Error> {
    a.checked_mul(b).ok_or(Error::CoordinateOverflow)
}

fn checked_add(a: u32, b: u32) -> Result<u32, Error> {
    a.checked_add(b).ok_or(Error::CoordinateOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tm20::graphics::{Graphics, GraphicsScale};

    fn luma(png: &[u8]) -> (u32, u32, Vec<u8>) {
        let img = image::load_from_memory(png).unwrap().to_luma8();
        (img.width(), img.height(), img.into_raw())
    }

    fn graphics(width: u16, height: u32, bits: &[bool], scale: GraphicsScale) -> Graphics {
        let raster = Raster::from_bits(width, height, bits).unwrap();
        Graphics::new(raster, scale).unwrap()
    }

    fn first_bit_black(width: u16, height: u32) -> Vec<bool> {
        let n = usize::from(width) * usize::try_from(height).unwrap();
        let mut bits = vec![false; n];
        bits[0] = true;
        bits
    }

    fn ink_rect(px: &[u8], w: u32, x0: u32, y0: u32, rw: u32, rh: u32) -> bool {
        for y in y0..y0 + rh {
            for x in x0..x0 + rw {
                if px[(y * w + x) as usize] != 0 {
                    return false;
                }
            }
        }
        true
    }

    #[test]
    fn preview_raster_is_2x_nearest_neighbor() {
        let raster = Raster::from_bits(8, 1, &first_bit_black(8, 1)).unwrap();
        let (w, h, px) = luma(&preview_raster(&raster).unwrap());
        assert_eq!((w, h), (16, 2));
        assert!(ink_rect(&px, w, 0, 0, 2, 2));
        assert_eq!(px[2], 255);
        assert_eq!(px[16], 0);
    }

    #[test]
    fn preview_png_applies_each_graphics_scale_before_display_2x() {
        let bits = first_bit_black(8, 1);
        let cases = [
            (GraphicsScale::Normal, 16, 2, 2, 2),
            (GraphicsScale::DoubleWidth, 32, 2, 4, 2),
            (GraphicsScale::DoubleHeight, 16, 4, 2, 4),
            (GraphicsScale::Quadruple, 32, 4, 4, 4),
        ];
        for (scale, pw, ph, cw, ch) in cases {
            let g = graphics(8, 1, &bits, scale);
            let (w, h, px) = luma(&preview_png(&g).unwrap());
            assert_eq!((w, h), (pw, ph), "{scale:?}");
            assert!(ink_rect(&px, w, 0, 0, cw, ch), "{scale:?} ink block");
            let outside = (cw * ph) as usize;
            if outside < px.len() {
                assert_eq!(px[cw as usize], 255, "{scale:?} right of ink is paper");
            }
        }
    }

    #[test]
    fn preview_pngs_stitches_equal_effective_widths_including_mixed_scale() {
        let top = graphics(8, 1, &first_bit_black(8, 1), GraphicsScale::Normal);
        let bottom = graphics(4, 1, &first_bit_black(4, 1), GraphicsScale::DoubleWidth);
        let (w, h, px) = luma(&preview_pngs([&top, &bottom]).unwrap());
        assert_eq!((w, h), (16, 4));
        assert!(ink_rect(&px, w, 0, 0, 2, 2), "normal band 2× display");
        assert!(
            ink_rect(&px, w, 0, 2, 4, 2),
            "double-width band: 2× print then 2× display"
        );
    }

    #[test]
    fn preview_pngs_rejects_incompatible_effective_widths_before_allocation() {
        let a = graphics(8, 1, &first_bit_black(8, 1), GraphicsScale::Normal);
        let b = graphics(8, 1, &first_bit_black(8, 1), GraphicsScale::DoubleWidth);
        assert!(matches!(preview_pngs([&a, &b]), Err(Error::InvalidImage)));
    }

    #[test]
    fn preview_pngs_empty_is_minimal_white() {
        let (w, h, px) = luma(&preview_pngs(std::iter::empty()).unwrap());
        assert_eq!((w, h), (2, 2));
        assert!(px.iter().all(|&p| p == 255));
    }

    #[test]
    fn checked_arithmetic_rejects_overflow() {
        assert!(matches!(
            checked_mul(u32::MAX, 2),
            Err(Error::CoordinateOverflow)
        ));
        assert!(matches!(
            checked_add(u32::MAX, 1),
            Err(Error::CoordinateOverflow)
        ));
    }
}
