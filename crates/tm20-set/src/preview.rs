//! 1-bit [`Graphics`] as a PNG for screen inspection. Not a print path.

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use tm20::graphics::{Graphics, is_black, width_bytes};

use crate::error::Error;

/// Unpack `g` to a 2× nearest-neighbor Luma PNG (black ink on white).
pub fn preview_png(g: &Graphics) -> Result<Vec<u8>, Error> {
    preview_pngs(std::iter::once(g))
}

/// Stitch bands top-to-bottom with no extra rows, then the same 2× PNG.
pub fn preview_pngs<'a, I>(bands: I) -> Result<Vec<u8>, Error>
where
    I: IntoIterator<Item = &'a Graphics>,
{
    let bands: Vec<&Graphics> = bands.into_iter().collect();
    let w = bands.first().map_or(1, |g| g.width_dots);
    let h: u32 = bands
        .iter()
        .map(|g| u32::from(g.height_dots))
        .sum::<u32>()
        .max(1);
    let pw = u32::from(w).saturating_mul(2).max(1);
    let ph = h.saturating_mul(2).max(1);
    let mut raw = vec![255u8; pw as usize * ph as usize];
    let mut y0 = 0u32;
    for g in bands {
        if g.width_dots != w {
            return Err(Error::Image);
        }
        let stride = width_bytes(g.width_dots);
        for y in 0..u32::from(g.height_dots) {
            for x in 0..u32::from(w) {
                if !is_black(&g.pixels, stride, x as usize, y as usize) {
                    continue;
                }
                let yy = y0 + y;
                for dy in 0..2u32 {
                    for dx in 0..2u32 {
                        raw[((yy * 2 + dy) * pw + (x * 2 + dx)) as usize] = 0;
                    }
                }
            }
        }
        y0 += u32::from(g.height_dots);
    }
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(&raw, pw, ph, ExtendedColorType::L8)
        .map_err(|_| Error::Image)?;
    Ok(out)
}
