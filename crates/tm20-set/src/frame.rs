//! The document: a [`Sheet`] of [`Frame`]s. Ugly layouts have no variant.

use crate::error::Error;
use crate::face::{DisplayFace, TextFace, Weight};
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
    pub face: &'a TextFace,
    pub text: &'a str,
}

/// One paragraph. Flush left, rag right. Adjacent [`Frame::Text`] values
/// are adjacent paragraphs; compose inserts the blank line.
pub struct TextBlock<'a> {
    pub size: TextSize,
    pub spans: Vec<Span<'a>>,
}

impl<'a> TextBlock<'a> {
    pub fn plain(face: &'a TextFace, size: TextSize, text: &'a str) -> Self {
        Self {
            size,
            spans: vec![Span { face, text }],
        }
    }
}

/// Body size, Bold, space above, none below.
pub struct Head<'a> {
    pub face: &'a TextFace,
    pub size: TextSize,
    pub text: &'a str,
}

impl<'a> Head<'a> {
    pub fn new(face: &'a TextFace, size: TextSize, text: &'a str) -> Result<Self, Error> {
        if face.weight() != Weight::Bold {
            return Err(Error::HeadNotBold);
        }
        Ok(Self { face, size, text })
    }
}

/// Single line, no wrap. Center is only legal here.
pub struct Mark<'a> {
    pub face: &'a DisplayFace,
    pub size: DisplaySize,
    pub text: &'a str,
    pub align: MarkAlign,
    pub tracking: Tracking,
}

pub struct Rule {
    pub thickness: Thickness,
}

/// Column widths plus gutters equal the tape. Parse, don't validate later.
#[derive(Debug, Clone)]
pub struct Columns {
    widths: Vec<Measure>,
    gutter: GridSkip,
}

impl Columns {
    pub fn new(widths: Vec<Measure>, gutter: GridSkip, tape: Measure) -> Result<Self, Error> {
        if widths.len() < 2 {
            return Err(Error::Columns);
        }
        let gutters = (widths.len() as u16 - 1).saturating_mul(gutter.dots());
        let sum: u32 = widths.iter().map(|w| u32::from(w.get())).sum::<u32>() + u32::from(gutters);
        if sum != u32::from(tape.get()) {
            return Err(Error::Columns);
        }
        Ok(Self { widths, gutter })
    }

    pub fn widths(&self) -> &[Measure] {
        &self.widths
    }

    pub fn gutter(&self) -> GridSkip {
        self.gutter
    }

    /// Half-open `[x0, x1)` origins for each column.
    pub fn origins(&self) -> Vec<(u16, u16)> {
        let mut x = 0u16;
        let g = self.gutter.dots();
        let mut out = Vec::with_capacity(self.widths.len());
        for (i, w) in self.widths.iter().enumerate() {
            let x1 = x + w.get();
            out.push((x, x1));
            x = x1;
            if i + 1 < self.widths.len() {
                x = x.saturating_add(g);
            }
        }
        out
    }
}

pub enum Cell<'a> {
    Empty,
    Label(Vec<Span<'a>>),
    Figure { face: &'a TextFace, text: &'a str },
}

pub struct Row<'a> {
    pub rule: Option<Thickness>,
    pub cells: Vec<Cell<'a>>,
}

pub struct Table<'a> {
    pub columns: Columns,
    pub size: TextSize,
    pub rows: Vec<Row<'a>>,
}

impl<'a> Table<'a> {
    /// Two columns: wrapping label, tabular figure on the first baseline.
    pub fn pair(
        tape: Measure,
        gutter: GridSkip,
        size: TextSize,
        left: Vec<Span<'a>>,
        right_face: &'a TextFace,
        right: &'a str,
    ) -> Result<Self, Error> {
        let fig_w = right_face.shape(right, size, true).width.ceil().max(1.0) as u16;
        let left_w = tape
            .get()
            .saturating_sub(gutter.dots())
            .saturating_sub(fig_w);
        let columns = Columns::new(
            vec![
                Measure::new(left_w).ok_or(Error::Columns)?,
                Measure::new(fig_w).ok_or(Error::Columns)?,
            ],
            gutter,
            tape,
        )?;
        Ok(Self {
            columns,
            size,
            rows: vec![Row {
                rule: None,
                cells: vec![
                    Cell::Label(left),
                    Cell::Figure {
                        face: right_face,
                        text: right,
                    },
                ],
            }],
        })
    }
}

/// Hanging list. Marker in the margin; runovers align with the text, not the dash.
pub struct List<'a> {
    pub size: TextSize,
    pub face: &'a TextFace,
    pub items: Vec<Vec<Span<'a>>>,
}

impl<'a> List<'a> {
    /// Dash plus a word space, ceiled to [`GRID`].
    pub fn hang_dots(&self) -> u16 {
        let w = self.face.shape(EN_DASH, self.size, false).width
            + self.face.shape(" ", self.size, false).width;
        let units = (w / GRID as f32).ceil().max(1.0) as u16;
        units * GRID
    }
}

/// One node of the typesetting language.
pub enum Frame<'a> {
    Text(TextBlock<'a>),
    Head(Head<'a>),
    Mark(Mark<'a>),
    Table(Table<'a>),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leading::GridSkip;

    #[test]
    fn columns_must_sum_to_tape() {
        let a = Measure::new(200).unwrap();
        let b = Measure::new(200).unwrap();
        assert!(Columns::new(vec![a, b], GridSkip::ONE, Measure::TAPE).is_err());
        let g = GridSkip::ONE;
        let left = Measure::new(Measure::TAPE.get() - g.dots() - 80).unwrap();
        let right = Measure::new(80).unwrap();
        assert!(Columns::new(vec![left, right], g, Measure::TAPE).is_ok());
    }
}
