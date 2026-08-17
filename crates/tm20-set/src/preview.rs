//! 1-bit [`Graphics`] as a PNG for screen inspection. Not a print path.

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use tm20::graphics::{Graphics, width_bytes};

use crate::error::Error;

/// Unpack `g` to a 2× nearest-neighbor Luma PNG (black ink on white).
pub fn preview_png(g: &Graphics) -> Result<Vec<u8>, Error> {
    let w = g.width_dots as u32;
    let h = g.height_dots as u32;
    let pw = w.saturating_mul(2).max(1);
    let ph = h.saturating_mul(2).max(1);
    let stride = width_bytes(g.width_dots);
    let mut raw = vec![255u8; pw as usize * ph as usize];
    for y in 0..h {
        for x in 0..w {
            let byte = g.pixels[y as usize * stride + x as usize / 8];
            if byte & (0x80 >> (x % 8)) == 0 {
                continue;
            }
            for dy in 0..2u32 {
                for dx in 0..2u32 {
                    raw[((y * 2 + dy) * pw + (x * 2 + dx)) as usize] = 0;
                }
            }
        }
    }
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(&raw, pw, ph, ExtendedColorType::L8)
        .map_err(|_| Error::Image)?;
    Ok(out)
}
