//! Native image and math sources. Fitted at the local [`Measure`], never at parse.

use std::fmt;
use std::num::NonZeroU32;

use image::imageops::FilterType;
use tm20::Raster;

use crate::error::Error;
use crate::geometry::Measure;

/// Decoded authoring source. Native gray stays `u32` until local fit.
#[derive(Clone)]
enum ImageSource {
    Gray(image::GrayImage),
    Mono(Raster),
}

impl ImageSource {
    fn width(&self) -> u32 {
        match self {
            Self::Gray(gray) => gray.width(),
            Self::Mono(raster) => u32::from(raster.width()),
        }
    }

    fn height(&self) -> u32 {
        match self {
            Self::Gray(gray) => gray.height(),
            Self::Mono(raster) => raster.height(),
        }
    }
}

/// Fitted math raster plus validated ascent. Depth is `height - ascent`.
#[derive(Clone, Debug)]
pub struct FittedMath {
    raster: Raster,
    ascent: u32,
}

impl FittedMath {
    pub fn new(raster: Raster, ascent: u32) -> Result<Self, Error> {
        if ascent > raster.height() {
            return Err(Error::InvalidBaseline);
        }
        Ok(Self { raster, ascent })
    }

    pub fn raster(&self) -> &Raster {
        &self.raster
    }

    pub fn ascent(&self) -> u32 {
        self.ascent
    }

    pub fn depth(&self) -> u32 {
        self.raster.height() - self.ascent
    }
}

/// Photograph. Native size; shrinks if wider than the measure; centered if narrower.
///
/// Optional caption references are one-based and checked during sheet admission.
#[derive(Clone)]
pub struct Figure {
    source: ImageSource,
    note: Option<NonZeroU32>,
}

impl fmt::Debug for Figure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Figure")
            .field("width", &self.width())
            .field("height", &self.height())
            .field("note", &self.note)
            .finish_non_exhaustive()
    }
}

impl Figure {
    /// Validate a packed mono source. Native dimensions are kept until [`Self::fit`].
    pub fn from_bits(width: u16, height: u32, bits: &[bool]) -> Result<Self, Error> {
        let raster = Raster::from_bits(width, height, bits)?;
        Ok(Self {
            source: ImageSource::Mono(raster),
            note: None,
        })
    }

    /// Decode PNG or JPEG at native size. Alpha composites onto white paper.
    /// Floyd–Steinberg waits for [`Self::fit`].
    pub fn from_image(bytes: &[u8]) -> Result<Self, Error> {
        let gray = decode_gray(bytes)?;
        Ok(Self {
            source: ImageSource::Gray(gray),
            note: None,
        })
    }

    pub fn fit(&self, measure: Measure) -> Result<Raster, Error> {
        fit_source(&self.source, measure)
    }

    #[must_use]
    pub fn noted(mut self, n: NonZeroU32) -> Self {
        self.note = Some(n);
        self
    }

    #[must_use]
    pub fn note(&self) -> Option<NonZeroU32> {
        self.note
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.source.width()
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.source.height()
    }
}

/// TeX box. Natural source plus a validated baseline ratio. Fit locally.
///
/// `ascent + depth` of a fitted box equals the fitted height. The PNG *is*
/// the box: there is no math-atom list to wrap.
#[derive(Clone)]
pub struct Math {
    source: ImageSource,
    tex_ascent: u32,
    tex_depth: u32,
}

impl fmt::Debug for Math {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Math")
            .field("width", &self.width())
            .field("height", &self.height())
            .field("tex_ascent", &self.tex_ascent)
            .field("tex_depth", &self.tex_depth)
            .finish_non_exhaustive()
    }
}

impl Math {
    /// Mono source. `ascent` is in source pixels and must be `<= height`.
    pub fn from_bits(width: u16, height: u32, bits: &[bool], ascent: u32) -> Result<Self, Error> {
        let raster = Raster::from_bits(width, height, bits)?;
        let raster_h = raster.height();
        if ascent > raster_h {
            return Err(Error::InvalidBaseline);
        }
        Ok(Self {
            source: ImageSource::Mono(raster),
            tex_ascent: ascent,
            tex_depth: raster_h - ascent,
        })
    }

    /// Decode PNG at native size. `ascent` / `depth` name the TeX baseline
    /// ratio; they are not fitted here. A `0/0` ratio uses the centered cut.
    pub fn from_png(bytes: &[u8], ascent: u32, depth: u32) -> Result<Self, Error> {
        let gray = decode_gray(bytes)?;
        Ok(Self {
            source: ImageSource::Gray(gray),
            tex_ascent: ascent,
            tex_depth: depth,
        })
    }

    pub fn fit(&self, measure: Measure) -> Result<FittedMath, Error> {
        let raster = fit_source(&self.source, measure)?;
        let ascent = fitted_ascent(raster.height(), self.tex_ascent, self.tex_depth)?;
        FittedMath::new(raster, ascent)
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.source.width()
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.source.height()
    }

    #[must_use]
    pub fn tex_ascent(&self) -> u32 {
        self.tex_ascent
    }

    #[must_use]
    pub fn tex_depth(&self) -> u32 {
        self.tex_depth
    }
}

/// Luma on paper. Alpha is applied here: transparent is paper (255), not
/// dropped RGB. A figure cannot represent “RGB without alpha interpretation.”
fn decode_gray(bytes: &[u8]) -> Result<image::GrayImage, Error> {
    let img = image::load_from_memory(bytes).map_err(Error::ImageDetail)?;
    let luma = paper_gray(&img);
    if luma.width() == 0 || luma.height() == 0 {
        return Err(Error::InvalidImage);
    }
    Ok(luma)
}

fn paper_gray(img: &image::DynamicImage) -> image::GrayImage {
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    image::GrayImage::from_fn(width, height, |x, y| {
        let pixel = rgba.get_pixel(x, y).0;
        let ink =
            (u32::from(pixel[0]) * 77 + u32::from(pixel[1]) * 150 + u32::from(pixel[2]) * 29) >> 8;
        let paper = 255u32;
        let out = (ink * u32::from(pixel[3]) + paper * (255 - u32::from(pixel[3]))) / 255;
        image::Luma([out as u8])
    })
}

fn fit_source(source: &ImageSource, measure: Measure) -> Result<Raster, Error> {
    match source {
        ImageSource::Gray(gray) => fit_gray(gray, measure),
        ImageSource::Mono(raster) => fit_mono(raster, measure),
    }
}

fn fit_gray(luma: &image::GrayImage, measure: Measure) -> Result<Raster, Error> {
    let (src_w, src_h) = luma.dimensions();
    if src_w == 0 || src_h == 0 {
        return Err(Error::InvalidImage);
    }
    let max_w = u32::from(measure.get());
    let (dst_w, dst_h, samples) = if src_w <= max_w {
        let samples = samples_from_gray(luma)?;
        (src_w, src_h, samples)
    } else {
        let dst_w = max_w;
        let dst_h = scale_dim(src_h, dst_w, src_w)?;
        let resized = image::imageops::resize(luma, dst_w, dst_h, FilterType::Triangle);
        let samples = samples_from_gray(&resized)?;
        (dst_w, dst_h, samples)
    };
    let dst_w = u16::try_from(dst_w).map_err(|_| Error::CoordinateOverflow)?;
    floyd_steinberg(dst_w, dst_h, samples)
}

fn fit_mono(raster: &Raster, measure: Measure) -> Result<Raster, Error> {
    if u32::from(raster.width()) <= u32::from(measure.get()) {
        return Ok(raster.clone());
    }
    let gray = mono_to_gray(raster)?;
    fit_gray(&gray, measure)
}

fn mono_to_gray(raster: &Raster) -> Result<image::GrayImage, Error> {
    let width = u32::from(raster.width());
    let height = raster.height();
    let n = usize::try_from(width)
        .ok()
        .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
        .ok_or(Error::CoordinateOverflow)?;
    let mut buf = Vec::new();
    buf.try_reserve(n).map_err(|_| Error::Alloc)?;
    for y in 0..height {
        for x in 0..raster.width() {
            let ink = raster.pixel(x, y) == Some(true);
            buf.push(if ink { 0 } else { 255 });
        }
    }
    image::GrayImage::from_raw(width, height, buf).ok_or(Error::InvalidImage)
}

fn samples_from_gray(luma: &image::GrayImage) -> Result<Vec<f32>, Error> {
    let n = usize::try_from(luma.width())
        .ok()
        .and_then(|w| {
            usize::try_from(luma.height())
                .ok()
                .and_then(|h| w.checked_mul(h))
        })
        .ok_or(Error::CoordinateOverflow)?;
    if luma.len() != n {
        return Err(Error::InvalidImage);
    }
    let mut samples = Vec::new();
    samples.try_reserve(n).map_err(|_| Error::Alloc)?;
    samples.extend(luma.as_raw().iter().copied().map(f32::from));
    Ok(samples)
}

/// `round(src * num / den)`, at least one. Wide intermediates; no `f32` dims.
fn scale_dim(src: u32, num: u32, den: u32) -> Result<u32, Error> {
    if den == 0 {
        return Err(Error::InvalidImage);
    }
    let prod = u64::from(src)
        .checked_mul(u64::from(num))
        .ok_or(Error::CoordinateOverflow)?;
    let den = u64::from(den);
    let rounded = prod.checked_add(den / 2).ok_or(Error::CoordinateOverflow)? / den;
    let out = u32::try_from(rounded).map_err(|_| Error::CoordinateOverflow)?;
    Ok(out.max(1))
}

/// Baseline divides the fitted raster in the TeX ratio.
/// A `0/0` span uses the documented centered cut. The cut is never above the bits.
fn fitted_ascent(height: u32, tex_ascent: u32, tex_depth: u32) -> Result<u32, Error> {
    if height == 0 {
        return Err(Error::InvalidImage);
    }
    let span = u64::from(tex_ascent)
        .checked_add(u64::from(tex_depth))
        .ok_or(Error::CoordinateOverflow)?;
    if span == 0 {
        return Ok(height / 2);
    }
    let num = u64::from(height)
        .checked_mul(u64::from(tex_ascent))
        .ok_or(Error::CoordinateOverflow)?;
    let ascent = num.checked_add(span / 2).ok_or(Error::CoordinateOverflow)? / span;
    let ascent = u32::try_from(ascent).map_err(|_| Error::CoordinateOverflow)?;
    Ok(ascent.min(height))
}

fn floyd_steinberg(width: u16, height: u32, mut px: Vec<f32>) -> Result<Raster, Error> {
    let w = usize::from(width);
    let h = usize::try_from(height).map_err(|_| Error::CoordinateOverflow)?;
    let expected = w.checked_mul(h).ok_or(Error::CoordinateOverflow)?;
    if px.len() != expected {
        return Err(Error::InvalidImage);
    }
    let stride = usize::from(width).div_ceil(8);
    let total = stride.checked_mul(h).ok_or(Error::CoordinateOverflow)?;
    let mut bits = Vec::new();
    bits.try_reserve(total).map_err(|_| Error::Alloc)?;
    bits.resize(total, 0);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let old = px[i].clamp(0.0, 255.0);
            let new = if old < 128.0 { 0.0 } else { 255.0 };
            if new == 0.0 {
                bits[y * stride + x / 8] |= 0x80 >> (x % 8);
            }
            let err = old - new;
            if x + 1 < w {
                px[y * w + x + 1] += err * 7.0 / 16.0;
            }
            if y + 1 < h {
                if x > 0 {
                    px[(y + 1) * w + x - 1] += err * 3.0 / 16.0;
                }
                px[(y + 1) * w + x] += err * 5.0 / 16.0;
                if x + 1 < w {
                    px[(y + 1) * w + x + 1] += err * 1.0 / 16.0;
                }
            }
        }
    }
    Ok(Raster::from_packed(width, height, bits)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tm20::RasterError;

    fn measure(n: u16) -> Measure {
        Measure::new(n).expect("admitted measure")
    }

    fn gray_png(w: u32, h: u32, luma: u8) -> Vec<u8> {
        let img = image::GrayImage::from_pixel(w, h, image::Luma([luma]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    fn rgba_png(pixels: &[[u8; 4]]) -> Vec<u8> {
        let n = pixels.len() as u32;
        let side = (n as f32).sqrt() as u32;
        let img = image::RgbaImage::from_fn(side, side, |x, y| {
            image::Rgba(pixels[(y * side + x) as usize])
        });
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    fn ink_count(raster: &Raster) -> usize {
        let mut n = 0;
        for y in 0..raster.height() {
            for x in 0..raster.width() {
                if raster.pixel(x, y) == Some(true) {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn figure_rejects_ragged_bits() {
        assert!(matches!(
            Figure::from_bits(2, 2, &[true]),
            Err(Error::Raster(RasterError::LengthMismatch { .. }))
        ));
        assert!(matches!(
            Figure::from_bits(0, 1, &[true]),
            Err(Error::Raster(RasterError::ZeroWidth))
        ));
        assert!(matches!(
            Figure::from_bits(1, 0, &[true]),
            Err(Error::Raster(RasterError::ZeroHeight))
        ));
    }

    #[test]
    fn figure_from_image_keeps_native_65536_rows() {
        let fig = Figure::from_image(&gray_png(1, 65_536, 0)).unwrap();
        assert_eq!(fig.width(), 1);
        assert_eq!(fig.height(), 65_536);
        let fitted = fig.fit(Measure::TAPE).unwrap();
        assert_eq!(fitted.width(), 1);
        assert_eq!(fitted.height(), 65_536);
        assert_eq!(fitted.pixel(0, 0), Some(true));
        assert_eq!(fitted.pixel(0, 65_535), Some(true));
    }

    #[test]
    fn figure_from_bits_keeps_native_65536_rows() {
        let bits = vec![true; 65_536];
        let fig = Figure::from_bits(1, 65_536, &bits).unwrap();
        assert_eq!(fig.height(), 65_536);
        let fitted = fig.fit(Measure::TAPE).unwrap();
        assert_eq!(fitted.height(), 65_536);
        assert_eq!(fitted.pixel(0, 65_535), Some(true));
    }

    #[test]
    fn figure_keeps_native_size_when_narrower() {
        let fig = Figure::from_image(&gray_png(8, 4, 0)).unwrap();
        assert_eq!(fig.width(), 8);
        assert_eq!(fig.height(), 4);
        let fitted = fig.fit(measure(16)).unwrap();
        assert_eq!(fitted.width(), 8);
        assert_eq!(fitted.height(), 4);
        assert!(ink_count(&fitted) > 0);
    }

    #[test]
    fn figure_already_the_measure_is_untouched() {
        let fig = Figure::from_image(&gray_png(8, 4, 0)).unwrap();
        let fitted = fig.fit(measure(8)).unwrap();
        assert_eq!(fitted.width(), 8);
        assert_eq!(fitted.height(), 4);
    }

    #[test]
    fn figure_16x8_fits_8x4_with_content_not_crop() {
        let mut bits = vec![false; 16 * 8];
        for y in 0..8 {
            for x in 8..16 {
                bits[y * 16 + x] = true;
            }
        }
        let fig = Figure::from_bits(16, 8, &bits).unwrap();
        let fitted = fig.fit(measure(8)).unwrap();
        assert_eq!(fitted.width(), 8);
        assert_eq!(
            fitted.height(),
            4,
            "proportional 16×8 → 8×4, not an 8×8 crop"
        );
        let mut left = 0;
        let mut right = 0;
        for y in 0..fitted.height() {
            for x in 0..fitted.width() {
                if fitted.pixel(x, y) != Some(true) {
                    continue;
                }
                if x < 4 {
                    left += 1;
                } else {
                    right += 1;
                }
            }
        }
        assert!(right > 0, "right half must keep ink");
        assert!(
            right > left,
            "a left-side crop of the white half would have no right ink"
        );
    }

    #[test]
    fn figure_gray_16x8_fits_8x4() {
        let fig = Figure::from_image(&gray_png(16, 8, 0)).unwrap();
        assert_eq!((fig.width(), fig.height()), (16, 8));
        let fitted = fig.fit(measure(8)).unwrap();
        assert_eq!(fitted.width(), 8);
        assert_eq!(fitted.height(), 4);
        assert_eq!(ink_count(&fitted), 8 * 4);
    }

    #[test]
    fn figure_rejects_garbage() {
        assert!(matches!(
            Figure::from_image(&[0xff; 8]),
            Err(Error::ImageDetail(_))
        ));
    }

    #[test]
    fn figure_treats_transparent_as_paper() {
        // Black opaque + fully transparent: to_luma8 would make a black square.
        let buf = rgba_png(&[[0, 0, 0, 0], [0, 0, 0, 255], [0, 0, 0, 0], [0, 0, 0, 255]]);
        let fig = Figure::from_image(&buf).unwrap();
        assert_eq!(fig.width(), 2);
        assert_eq!(fig.height(), 2);
        let fitted = fig.fit(measure(16)).unwrap();
        assert_eq!(fitted.pixel(0, 0), Some(false), "transparent is paper");
        assert_eq!(fitted.pixel(1, 0), Some(true), "opaque black is ink");
        assert_eq!(fitted.pixel(0, 1), Some(false), "transparent is paper");
        assert_eq!(fitted.pixel(1, 1), Some(true), "opaque black is ink");
    }

    #[test]
    fn math_from_bits_rejects_ascent_above_height() {
        assert!(matches!(
            Math::from_bits(8, 4, &[true; 32], 5),
            Err(Error::InvalidBaseline)
        ));
        assert!(Math::from_bits(8, 4, &[true; 32], 4).is_ok());
    }

    #[test]
    fn fitted_math_rejects_ascent_above_height() {
        let raster = Raster::from_bits(1, 2, &[true, true]).unwrap();
        assert!(matches!(
            FittedMath::new(raster, 3),
            Err(Error::InvalidBaseline)
        ));
    }

    #[test]
    fn math_from_png_is_native_and_does_not_upscale() {
        let m = Math::from_png(&gray_png(4, 2, 0), 2, 0).unwrap();
        assert_eq!(m.width(), 4);
        assert_eq!(m.height(), 2);
        assert_eq!(m.tex_ascent(), 2);
        assert_eq!(m.tex_depth(), 0);
        let native = m.fit(measure(16)).unwrap();
        assert_eq!(native.raster().width(), 4);
        assert_eq!(native.raster().height(), 2);
        assert_eq!(native.ascent(), 2);
        assert_eq!(native.depth(), 0);
        let shrink = m.fit(measure(2)).unwrap();
        assert_eq!(shrink.raster().width(), 2);
        assert!(shrink.raster().height() >= 1);
        assert_eq!(shrink.ascent() + shrink.depth(), shrink.raster().height());
    }

    #[test]
    fn math_zero_ratio_uses_centered_baseline() {
        let m = Math::from_png(&gray_png(4, 5, 0), 0, 0).unwrap();
        let fitted = m.fit(measure(16)).unwrap();
        assert_eq!(fitted.raster().height(), 5);
        assert_eq!(fitted.ascent(), 2);
        assert_eq!(fitted.depth(), 3);
        assert_eq!(fitted.ascent() + fitted.depth(), 5);
    }

    #[test]
    fn math_baseline_sum_equals_fitted_height_at_awkward_ratio() {
        // 7×5 source, TeX 3+2, measure 4 → height round(5*4/7)=3, ascent round(3*3/5)=2.
        let m = Math::from_bits(7, 5, &[true; 35], 3).unwrap();
        assert_eq!(m.tex_ascent(), 3);
        assert_eq!(m.tex_depth(), 2);
        let fitted = m.fit(measure(4)).unwrap();
        assert_eq!(fitted.raster().width(), 4);
        assert_eq!(fitted.raster().height(), 3);
        assert_eq!(fitted.ascent(), 2);
        assert_eq!(fitted.depth(), 1);
        assert_eq!(fitted.ascent() + fitted.depth(), fitted.raster().height());
    }

    #[test]
    fn math_png_ratio_survives_when_tex_span_exceeds_png() {
        let m = Math::from_png(&gray_png(4, 2, 0), 8, 4).unwrap();
        assert_eq!(m.tex_ascent(), 8);
        assert_eq!(m.tex_depth(), 4);
        let fitted = m.fit(measure(16)).unwrap();
        assert_eq!(fitted.raster().height(), 2);
        assert_eq!(fitted.ascent() + fitted.depth(), 2);
        // 2 * 8 / 12 rounded is 1.
        assert_eq!(fitted.ascent(), 1);
    }

    #[test]
    fn math_rejects_garbage_png() {
        assert!(matches!(
            Math::from_png(&[0xff; 8], 1, 0),
            Err(Error::ImageDetail(_))
        ));
    }

    #[test]
    fn scale_dim_rounds_and_rejects_zero_denominator() {
        assert_eq!(scale_dim(8, 4, 8).unwrap(), 4);
        assert_eq!(scale_dim(5, 4, 7).unwrap(), 3);
        assert!(matches!(scale_dim(1, 1, 0), Err(Error::InvalidImage)));
    }
}
