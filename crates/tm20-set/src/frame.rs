//! The document: a [`Sheet`] of [`Frame`]s. Ugly layouts have no variant.
//! Faces are named [`Cut`]s; [`crate::FaceTable`] says what those names mean.

use crate::error::Error;
use crate::face::{Cut, FaceTable};
use crate::leading::{GridSkip, GRID};
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
#[derive(Clone, Copy)]
pub struct Span<'a> {
    pub cut: Cut,
    pub text: &'a str,
}

/// One paragraph. Flush left, rag right. Adjacent [`Frame::Text`] values
/// are adjacent paragraphs; compose inserts the blank line.
pub struct TextBlock<'a> {
    pub size: TextSize,
    pub spans: Vec<Span<'a>>,
}

impl<'a> TextBlock<'a> {
    pub fn plain(cut: Cut, size: TextSize, text: &'a str) -> Self {
        Self {
            size,
            spans: vec![Span { cut, text }],
        }
    }
}

/// Body size, Bold upright, space above, none below.
pub struct Head<'a> {
    pub size: TextSize,
    pub text: &'a str,
}

/// Single line, no wrap. Center is only legal here.
pub struct Mark<'a> {
    pub cut: Cut,
    pub size: DisplaySize,
    pub text: &'a str,
    pub align: MarkAlign,
    pub tracking: Tracking,
}

pub struct Rule {
    pub thickness: Thickness,
}

/// Item flush left, tabular figure hanging on the first baseline.
pub struct Pair<'a> {
    pub size: TextSize,
    pub gutter: GridSkip,
    pub left: Vec<Span<'a>>,
    pub figure: Cut,
    pub amount: &'a str,
}

/// Hanging list. Marker in the margin; runovers align with the text, not the dash.
pub struct List<'a> {
    pub size: TextSize,
    pub cut: Cut,
    pub items: Vec<Vec<Span<'a>>>,
}

impl<'a> List<'a> {
    /// Dash plus a word space, ceiled to [`GRID`].
    pub fn hang_dots(&self, faces: &FaceTable) -> Result<u16, Error> {
        let face = faces.text(self.cut)?;
        let w = face.shape(EN_DASH, self.size).width + face.shape(" ", self.size).width;
        let units = (w / GRID as f32).ceil().max(1.0) as u16;
        Ok(units * GRID)
    }
}

/// One node of the typesetting language.
pub enum Frame<'a> {
    Text(TextBlock<'a>),
    Head(Head<'a>),
    Mark(Mark<'a>),
    Pair(Pair<'a>),
    List(List<'a>),
    Rule(Rule),
}

/// Authoring document. Compiles to one `Graphics`.
pub struct Sheet<'a> {
    pub width: Measure,
    pub frames: Vec<Frame<'a>>,
}

impl<'a> Sheet<'a> {
    pub fn tape(frames: Vec<Frame<'a>>) -> Self {
        Self {
            width: Measure::TAPE,
            frames,
        }
    }
}
