//! Host typeface rasterizer: any OpenType face, 203 dpi, 1-bit [`Graphics`].
//!
//! Bytes, a path, and a system family name all parse into [`Face`]. Layout is a
//! slice of [`Line`]; [`raster`] evaluates it onto one canvas. Alignment is a
//! shift on that canvas, not a second encoder.

use std::num::NonZeroU16;
use std::path::Path;

use font_kit::family_name::FamilyName;
use font_kit::handle::Handle;
use font_kit::properties::{Properties, Weight as KitWeight};
use font_kit::source::SystemSource;
use fontdue::{Font as RasterFont, FontSettings};
use harfrust::{FontRef, ShapeOptions, ShaperData, UnicodeBuffer};

use crate::error::TypefaceError;
use crate::graphics::{pack, Graphics, GraphicsScale};
use crate::PRINTABLE_DOTS;

const THRESHOLD: u8 = 96;
const LINE_HEIGHT: f32 = 1.2;

/// TM-T20III thermal resolution.
pub const DPI: f32 = 203.0;

/// CSS weight class. Only used when querying the system font database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weight(pub u16);

impl Weight {
    pub const REGULAR: Self = Self(400);
    pub const MEDIUM: Self = Self(500);
    pub const SEMIBOLD: Self = Self(600);
    pub const BOLD: Self = Self(700);
}

/// Positive point size. Construct with [`Pt::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pt(f32);

impl Pt {
    pub fn new(pt: f32) -> Option<Self> {
        (pt > 0.0 && pt.is_finite()).then_some(Self(pt))
    }

    pub fn px(self) -> f32 {
        self.0 * DPI / 72.0
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

/// Non-zero canvas width in dots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dots(NonZeroU16);

impl Dots {
    pub const TAPE: Self = Self(NonZeroU16::new(PRINTABLE_DOTS).unwrap());

    pub fn new(n: u16) -> Option<Self> {
        NonZeroU16::new(n).map(Self)
    }

    pub fn get(self) -> u16 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

impl Align {
    fn offset(self, line_width: f32, canvas: f32) -> f32 {
        match self {
            Align::Left => 0.0,
            Align::Center => ((canvas - line_width) * 0.5).max(0.0),
            Align::Right => (canvas - line_width).max(0.0),
        }
    }
}

/// Parsed typeface. Unrepresentable unless both the shaper and rasterizer accept the bytes.
pub struct Face {
    bytes: Vec<u8>,
    index: u32,
    raster: RasterFont,
    hb: ShaperData,
}

impl Face {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, TypefaceError> {
        Self::from_bytes_index(bytes, 0)
    }

    pub fn from_bytes_index(bytes: Vec<u8>, index: u32) -> Result<Self, TypefaceError> {
        let font = FontRef::from_index(&bytes, index).map_err(|_| TypefaceError::Font)?;
        let raster = RasterFont::from_bytes(bytes.as_slice(), FontSettings::default())
            .map_err(|_| TypefaceError::Font)?;
        let hb = ShaperData::new(&font);
        Ok(Self {
            bytes,
            index,
            raster,
            hb,
        })
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, TypefaceError> {
        Self::from_bytes(std::fs::read(path)?)
    }

    fn from_handle(handle: Handle) -> Result<Self, TypefaceError> {
        match handle {
            Handle::Path { path, font_index } => {
                Self::from_bytes_index(std::fs::read(path)?, font_index)
            }
            Handle::Memory { bytes, font_index } => {
                Self::from_bytes_index((*bytes).clone(), font_index)
            }
        }
    }

    pub fn system(family: &str) -> Result<Self, TypefaceError> {
        Self::system_with(family, Weight::REGULAR)
    }

    pub fn system_with(family: &str, weight: Weight) -> Result<Self, TypefaceError> {
        Self::from_handle(Self::match_family(
            &[FamilyName::Title(family.into())],
            weight,
            family,
        )?)
    }

    /// Platform default sans-serif (Helvetica, Arial, etc.).
    pub fn sans() -> Result<Self, TypefaceError> {
        Self::sans_with(Weight::REGULAR)
    }

    pub fn sans_with(weight: Weight) -> Result<Self, TypefaceError> {
        Self::from_handle(Self::match_family(
            &[FamilyName::SansSerif],
            weight,
            "sans-serif",
        )?)
    }

    fn match_family(
        names: &[FamilyName],
        weight: Weight,
        label: &str,
    ) -> Result<Handle, TypefaceError> {
        SystemSource::new()
            .select_best_match(names, Properties::new().weight(KitWeight(weight.0 as f32)))
            .map_err(|_| TypefaceError::NotFound {
                family: label.into(),
            })
    }

    pub fn advance(&self, ch: char, pt: Pt) -> f32 {
        self.raster.metrics(ch, pt.px()).advance_width
    }

    pub fn shaped_width(&self, text: &str, pt: Pt) -> f32 {
        self.shape(text, pt).width
    }

    fn metrics(&self, px: f32) -> (f32, f32) {
        let m = self
            .raster
            .horizontal_line_metrics(px)
            .expect("parsed face has hhea metrics");
        (m.ascent, (m.ascent - m.descent) * LINE_HEIGHT)
    }

    fn shape(&self, text: &str, pt: Pt) -> Shaped<'_> {
        let px = pt.px();
        let (ascent, height) = self.metrics(px);
        if text.is_empty() {
            return Shaped {
                face: self,
                px,
                glyphs: Vec::new(),
                width: 0.0,
                ascent,
                height,
            };
        }
        let font = FontRef::from_index(&self.bytes, self.index).expect("Face bytes already parsed");
        let shaper = self.hb.shaper(&font).build();
        let mut buf = UnicodeBuffer::new();
        buf.push_str(text);
        buf.guess_segment_properties();
        let glyphs = shaper.shape(buf, ShapeOptions::new());
        let scale = px / shaper.units_per_em() as f32;
        let mut x = 0.0;
        let mut placed = Vec::with_capacity(glyphs.len());
        for (info, pos) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
            placed.push(Placed {
                glyph_id: info.glyph_id as u16,
                x: x + pos.x_offset as f32 * scale,
                y: pos.y_offset as f32 * scale,
            });
            x += pos.x_advance as f32 * scale;
        }
        Shaped {
            face: self,
            px,
            glyphs: placed,
            width: x,
            ascent,
            height,
        }
    }

    fn wrap(&self, text: &str, pt: Pt, canvas: f32) -> Vec<Shaped<'_>> {
        text.split('\n')
            .flat_map(|para| wrap_para(self, para, pt, canvas))
            .collect()
    }
}

/// Face + size + string. The face is already parsed; this is just a borrow.
#[derive(Clone, Copy)]
pub struct Run<'a> {
    pub face: &'a Face,
    pub pt: Pt,
    pub text: &'a str,
}

/// One row on the tape. Alignment is a coordinate on the canvas.
pub enum Line<'a> {
    Text { run: Run<'a>, align: Align },
    Split { left: Run<'a>, right: Run<'a> },
}

struct Placed {
    glyph_id: u16,
    x: f32,
    y: f32,
}

struct Shaped<'a> {
    face: &'a Face,
    px: f32,
    glyphs: Vec<Placed>,
    width: f32,
    ascent: f32,
    height: f32,
}

fn wrap_para<'a>(face: &'a Face, para: &str, pt: Pt, canvas: f32) -> Vec<Shaped<'a>> {
    if para.is_empty() {
        return vec![face.shape("", pt)];
    }
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in para.split(' ') {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if face.shape(&candidate, pt).width <= canvas || line.is_empty() {
            line = candidate;
        } else {
            lines.push(face.shape(&line, pt));
            line = word.to_string();
        }
    }
    lines.push(face.shape(&line, pt));
    lines
}

/// Paint `lines` onto a canvas `width` dots wide.
pub fn raster(width: Dots, lines: &[Line<'_>]) -> Result<Graphics, TypefaceError> {
    let canvas = width.get() as f32;
    let mut spots: Vec<(f32, f32, Shaped<'_>)> = Vec::new();
    let mut y = 0.0;
    for line in lines {
        match line {
            Line::Text { run, align } => {
                for shaped in run.face.wrap(run.text, run.pt, canvas) {
                    let x = align.offset(shaped.width, canvas);
                    let h = shaped.height;
                    spots.push((x, y, shaped));
                    y += h;
                }
            }
            Line::Split { left, right } => {
                let l = left.face.shape(left.text, left.pt);
                let r = right.face.shape(right.text, right.pt);
                let h = l.height.max(r.height);
                spots.push((Align::Left.offset(l.width, canvas), y, l));
                spots.push((Align::Right.offset(r.width, canvas), y, r));
                y += h;
            }
        }
    }

    let total_h = y.ceil().max(1.0) as u32;
    if total_h > u16::MAX as u32 {
        return Err(TypefaceError::Overflow {
            width: canvas as u32,
            height: total_h,
        });
    }
    let height = total_h as u16;
    let width_dots = width.get();
    let mut bits = vec![false; width_dots as usize * height as usize];
    for (x0, y0, shaped) in spots {
        blit(&mut bits, width_dots, height, x0, y0, &shaped);
    }
    let pixels = pack(width_dots, height, &bits).map_err(|_| TypefaceError::Overflow {
        width: width_dots as u32,
        height: height as u32,
    })?;
    Ok(Graphics {
        width_dots,
        height_dots: height,
        pixels,
        scale: GraphicsScale::Normal,
    })
}

fn blit(bits: &mut [bool], width: u16, height: u16, x0: f32, y0: f32, shaped: &Shaped<'_>) {
    let baseline = y0 + shaped.ascent;
    let w = width as i32;
    let h = height as i32;
    for g in &shaped.glyphs {
        let (metrics, bitmap) = shaped.face.raster.rasterize_indexed(g.glyph_id, shaped.px);
        if metrics.width == 0 || metrics.height == 0 {
            continue;
        }
        let origin_x = (x0 + g.x).round() as i32 + metrics.xmin;
        let origin_y = (baseline - g.y).round() as i32 - metrics.ymin - metrics.height as i32;
        for gy in 0..metrics.height {
            for gx in 0..metrics.width {
                if bitmap[gy * metrics.width + gx] < THRESHOLD {
                    continue;
                }
                let x = origin_x + gx as i32;
                let y = origin_y + gy as i32;
                if x >= 0 && y >= 0 && x < w && y < h {
                    bits[y as usize * width as usize + x as usize] = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Face {
        Face::sans().expect("system sans-serif")
    }

    fn pt() -> Pt {
        Pt::new(11.0).unwrap()
    }

    #[test]
    fn i_is_narrower_than_m() {
        let face = sample();
        assert!(face.advance('i', pt()) < face.advance('M', pt()));
    }

    #[test]
    fn av_kerns() {
        let face = sample();
        let a = face.advance('A', pt());
        let v = face.advance('V', pt());
        let av = face.shaped_width("AV", pt());
        assert!(av < a + v - 0.5, "AV shaped={av} A+V={}", a + v);
    }

    #[test]
    fn eleven_pt_is_about_31_dots() {
        assert!((pt().px() - 31.0).abs() < 0.1);
    }

    #[test]
    fn pangram_has_ink() {
        let face = sample();
        let g = raster(
            Dots::TAPE,
            &[Line::Text {
                run: Run {
                    face: &face,
                    pt: pt(),
                    text: "The quick brown fox jumps over the lazy dog",
                },
                align: Align::Left,
            }],
        )
        .unwrap();
        assert_eq!(g.width_dots, PRINTABLE_DOTS);
        assert!(g.pixels.iter().any(|&b| b != 0));
    }

    #[test]
    fn wrap_makes_taller_than_one_line() {
        let face = sample();
        let one = raster(
            Dots::TAPE,
            &[Line::Text {
                run: Run {
                    face: &face,
                    pt: pt(),
                    text: "Hello",
                },
                align: Align::Left,
            }],
        )
        .unwrap();
        let wrapped = raster(
            Dots::TAPE,
            &[Line::Text {
                run: Run {
                    face: &face,
                    pt: pt(),
                    text: "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
                },
                align: Align::Left,
            }],
        )
        .unwrap();
        assert!(wrapped.height_dots > one.height_dots);
    }

    #[test]
    fn split_row_has_ink_on_both_sides() {
        let face = sample();
        let g = raster(
            Dots::TAPE,
            &[Line::Split {
                left: Run {
                    face: &face,
                    pt: pt(),
                    text: "Coffee",
                },
                right: Run {
                    face: &face,
                    pt: pt(),
                    text: "$4.50",
                },
            }],
        )
        .unwrap();
        let stride = crate::graphics::width_bytes(g.width_dots);
        let mut left = false;
        let mut right = false;
        for row in 0..g.height_dots as usize {
            left |= g.pixels[row * stride] != 0;
            right |= g.pixels[row * stride + stride - 1] != 0;
        }
        assert!(left && right);
    }
}
