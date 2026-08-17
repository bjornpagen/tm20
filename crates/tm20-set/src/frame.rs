//! The document: a [`Sheet`] of [`Frame`]s. Ugly layouts have no variant.
//! Faces are named [`Cut`]s; [`crate::FaceTable`] says what those names mean.

use std::borrow::Cow;
use std::num::NonZeroU32;

use crate::error::Error;
use crate::face::{Cut, FaceTable};
use crate::leading::{GridSkip, GRID, TASK_BOX};
use crate::size::{DisplaySize, TextSize};
use tm20::PRINTABLE_DOTS;

/// List mark. Hyphen-minus in the copy is not a list.
pub const EN_DASH: &str = "\u{2013}";

/// Canvas width in dots. Half-open `[0, get)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measure(std::num::NonZeroU16);

impl Measure {
    pub const TAPE: Self = Self(std::num::NonZeroU16::new(PRINTABLE_DOTS).unwrap());

    pub fn new(n: u16) -> Option<Self> {
        std::num::NonZeroU16::new(n).map(Self)
    }

    pub fn get(self) -> u16 {
        self.0.get()
    }
}

/// Tracking in thousandths of an em. Only legal on [`Mark`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tracking(pub i16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkAlign {
    Start,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Thickness {
    One,
    Two,
}

impl Thickness {
    pub fn dots(self) -> u16 {
        match self {
            Thickness::One => 1,
            Thickness::Two => 2,
        }
    }
}

/// One voice on a wrapping line. Size lives on the block, not here.
#[derive(Clone)]
pub struct Span<'a> {
    pub cut: Cut,
    pub text: Cow<'a, str>,
    pub note: Option<NonZeroU32>,
}

impl<'a> Span<'a> {
    pub fn new(cut: Cut, text: impl Into<Cow<'a, str>>) -> Self {
        Self {
            cut,
            text: text.into(),
            note: None,
        }
    }
}

/// One paragraph. Flush left, rag right. Adjacent [`Frame::Text`] values
/// are adjacent paragraphs; compose inserts the blank line.
pub struct TextBlock<'a> {
    pub size: TextSize,
    pub spans: Vec<Span<'a>>,
}

impl<'a> TextBlock<'a> {
    pub fn plain(cut: Cut, size: TextSize, text: impl Into<Cow<'a, str>>) -> Self {
        Self {
            size,
            spans: vec![Span::new(cut, text)],
        }
    }
}

/// Body size, Bold upright, space above, none below.
pub struct Head<'a> {
    pub size: TextSize,
    pub text: Cow<'a, str>,
}

/// Single line, no wrap. Center is only legal here.
pub struct Mark<'a> {
    pub cut: Cut,
    pub size: DisplaySize,
    pub text: Cow<'a, str>,
    pub align: MarkAlign,
    pub tracking: Tracking,
}

pub struct Rule {
    pub thickness: Thickness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColAlign {
    Start,
    End,
}

/// Two or three columns. [`ColAlign::End`] shapes with tabular figures.
/// Consecutive tables stay tight; a table after a [`Frame::Rule`] hangs.
pub struct Cols<'a> {
    pub size: TextSize,
    pub gutter: GridSkip,
    pub align: Vec<ColAlign>,
    pub rows: Vec<Vec<Vec<Span<'a>>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecimalDelim {
    Period,
    Paren,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    Dash,
    Decimal { start: u32, delim: DecimalDelim },
}

/// One list item. A task replaces the dash or decimal with a drawn checkbox.
pub struct ListItem<'a> {
    pub task: Option<bool>,
    pub frames: Vec<Frame<'a>>,
}

impl<'a> ListItem<'a> {
    pub fn new(frames: Vec<Frame<'a>>) -> Self {
        Self { task: None, frames }
    }

    pub fn task(checked: bool, frames: Vec<Frame<'a>>) -> Self {
        Self {
            task: Some(checked),
            frames,
        }
    }
}

/// Hanging list. Marker in the margin; runovers align with the text, not the mark.
pub struct List<'a> {
    pub size: TextSize,
    pub cut: Cut,
    pub marker: Marker,
    pub tight: bool,
    pub items: Vec<ListItem<'a>>,
}

impl<'a> List<'a> {
    /// Marker plus a word space, ceiled to [`GRID`]. Closed so dash, two-digit
    /// decimal, and a task box share a text column at this size.
    pub fn hang_dots(&self, faces: &FaceTable) -> Result<u16, Error> {
        let face = faces.text(self.cut)?;
        let space = face.shape(" ", self.size).width;
        let units = ((self.mark_width(faces)? + space) / GRID as f32)
            .ceil()
            .max(1.0) as u16;
        Ok(units * GRID)
    }

    /// Width of the mark column, before the word space and grid leftover.
    /// Decimal figures right-align in this band; dash and task sit at its start.
    pub(crate) fn mark_width(&self, faces: &FaceTable) -> Result<f32, Error> {
        let face = faces.text(self.cut)?;
        let mut mark_w = face.shape(EN_DASH, self.size).width;
        mark_w = mark_w.max(face.shape_figure("10.", self.size).width);
        mark_w = mark_w.max(TASK_BOX as f32);
        if let Marker::Decimal { start, delim } = self.marker {
            let n = self.items.len() as u32;
            for i in 0..n {
                let t = decimal_text(start.saturating_add(i), delim);
                mark_w = mark_w.max(face.shape_figure(&t, self.size).width);
            }
        }
        Ok(mark_w)
    }
}

pub(crate) fn decimal_text(n: u32, delim: DecimalDelim) -> String {
    match delim {
        DecimalDelim::Period => format!("{n}."),
        DecimalDelim::Paren => format!("{n})"),
    }
}

/// Block quote. Idle column, not a bar. Nested cap is three.
pub struct Quote<'a> {
    pub frames: Vec<Frame<'a>>,
}

/// Preformatted lines, hung by [`GRID`]. Not a paragraph: spaces do not wrap.
pub struct Code<'a> {
    pub size: TextSize,
    pub lines: Vec<Cow<'a, str>>,
}

/// 1-bit figure, already scaled to the measure. `true` is black.
pub struct Figure {
    pub width: u16,
    pub height: u16,
    pub bits: Vec<bool>,
}

impl Figure {
    pub fn from_bits(width: u16, height: u16, bits: Vec<bool>) -> Result<Self, Error> {
        if width == 0 || height == 0 || bits.len() != width as usize * height as usize {
            return Err(Error::Image);
        }
        Ok(Self {
            width,
            height,
            bits,
        })
    }

    /// Decode PNG or JPEG, scale to `measure` flush left, Floyd–Steinberg to 1-bit.
    pub fn from_image(bytes: &[u8], measure: u16) -> Result<Self, Error> {
        if measure == 0 {
            return Err(Error::Image);
        }
        let img = image::load_from_memory(bytes).map_err(|_| Error::Image)?;
        let luma = img.to_luma8();
        let (src_w, src_h) = luma.dimensions();
        if src_w == 0 || src_h == 0 {
            return Err(Error::Image);
        }
        let dst_w = u32::from(measure);
        let dst_h = ((src_h as f32 * dst_w as f32 / src_w as f32).round() as u32).max(1);
        let resized =
            image::imageops::resize(&luma, dst_w, dst_h, image::imageops::FilterType::Triangle);
        let samples: Vec<f32> = resized.pixels().map(|p| p.0[0] as f32).collect();
        let bits = floyd_steinberg(dst_w, dst_h, samples);
        Self::from_bits(dst_w as u16, dst_h as u16, bits)
    }
}

fn floyd_steinberg(w: u32, h: u32, mut px: Vec<f32>) -> Vec<bool> {
    let mut bits = vec![false; (w * h) as usize];
    let idx = |x: u32, y: u32| (y * w + x) as usize;
    for y in 0..h {
        for x in 0..w {
            let i = idx(x, y);
            let old = px[i].clamp(0.0, 255.0);
            let new = if old < 128.0 { 0.0 } else { 255.0 };
            bits[i] = new == 0.0;
            let err = old - new;
            if x + 1 < w {
                px[idx(x + 1, y)] += err * 7.0 / 16.0;
            }
            if y + 1 < h {
                if x > 0 {
                    px[idx(x - 1, y + 1)] += err * 3.0 / 16.0;
                }
                px[idx(x, y + 1)] += err * 5.0 / 16.0;
                if x + 1 < w {
                    px[idx(x + 1, y + 1)] += err * 1.0 / 16.0;
                }
            }
        }
    }
    bits
}

/// One node of the typesetting language.
pub enum Frame<'a> {
    Text(TextBlock<'a>),
    Head(Head<'a>),
    Mark(Mark<'a>),
    Cols(Cols<'a>),
    List(List<'a>),
    Quote(Quote<'a>),
    Code(Code<'a>),
    Figure(Figure),
    Rule(Rule),
}

/// One slot in the sheet’s note apparatus. Links and footnotes share the numbers.
pub enum Note<'a> {
    Dest(Cow<'a, str>),
    Blocks(Vec<Frame<'a>>),
}

/// Authoring document. Compiles to one `Graphics`.
pub struct Sheet<'a> {
    pub width: Measure,
    pub frames: Vec<Frame<'a>>,
    /// 1-based; compose paints them after the frames. [`Span::note`] indexes this.
    pub notes: Vec<Note<'a>>,
}

impl<'a> Sheet<'a> {
    pub fn tape(frames: Vec<Frame<'a>>) -> Self {
        Self {
            width: Measure::TAPE,
            frames,
            notes: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_zero_is_none() {
        assert!(Measure::new(0).is_none());
        assert_eq!(Measure::new(1).unwrap().get(), 1);
        assert_eq!(Measure::TAPE.get(), PRINTABLE_DOTS);
    }

    #[test]
    fn figure_rejects_ragged_bits() {
        assert!(matches!(
            Figure::from_bits(2, 2, vec![true]),
            Err(Error::Image)
        ));
        assert!(matches!(
            Figure::from_bits(0, 1, vec![true]),
            Err(Error::Image)
        ));
    }

    #[test]
    fn figure_from_image_fits_the_measure() {
        let img = image::GrayImage::from_pixel(1, 1, image::Luma([0]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        let fig = Figure::from_image(&buf, 16).unwrap();
        assert_eq!(fig.width, 16);
        assert!(fig.height >= 1);
        assert!(fig.bits.iter().any(|&b| b));
        assert!(matches!(Figure::from_image(&buf, 0), Err(Error::Image)));
        assert!(matches!(
            Figure::from_image(&[0xff; 8], 16),
            Err(Error::Image)
        ));
    }
}
