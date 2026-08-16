//! The document: a [`Sheet`] of [`Frame`]s. Ugly layouts have no variant.

use crate::face::{DisplayFace, TextFace};
use crate::leading::GridSkip;
use crate::size::{DisplaySize, TextSize};
use tm20::PRINTABLE_DOTS;

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

/// Flush-left, rag-right paragraph. Wraps at the sheet measure minus indent.
pub struct TextBlock<'a> {
    pub face: &'a TextFace,
    pub size: TextSize,
    pub text: &'a str,
    pub indent: u16,
}

/// Single line, no wrap. Center is only legal here.
pub struct Mark<'a> {
    pub face: &'a DisplayFace,
    pub size: DisplaySize,
    pub text: &'a str,
    pub align: MarkAlign,
    pub tracking: Tracking,
}

/// Item / price on one baseline. Right is tabular, no wrap.
pub struct Pair<'a> {
    pub left_face: &'a TextFace,
    pub left_size: TextSize,
    pub left: &'a str,
    pub right_face: &'a TextFace,
    pub right_size: TextSize,
    pub right: &'a str,
    pub gutter: GridSkip,
}

pub struct Rule {
    pub thickness: Thickness,
}

/// One node of the typesetting language.
pub enum Frame<'a> {
    Text(TextBlock<'a>),
    Mark(Mark<'a>),
    Pair(Pair<'a>),
    Rule(Rule),
    Skip(GridSkip),
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
