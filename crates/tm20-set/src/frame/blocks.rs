//! Block authoring values. Empty item bodies stay [`ItemBody::Blank`].

use std::borrow::Cow;

use crate::error::Error;
use crate::face::{Cut, TextFace};
use crate::frame::image::{Figure, Math};
use crate::frame::inline::{Head, Mark, Span, TextBlock, Thickness};
use crate::frame::nonempty::NonEmpty;
use crate::geometry::Advance;
use crate::leading::{GRID, TASK_BOX};
use crate::size::TextSize;

/// List mark. Hyphen-minus in the copy is not a list.
pub const EN_DASH: &str = "\u{2013}";

/// Where a rule sits. Only the tape: a hung walk cannot name leftover dots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSpan {
    Tape,
}

/// A break. The span is the tape, not leftover measure after a hang.
pub struct Rule {
    pub thickness: Thickness,
    pub span: RuleSpan,
}

impl Rule {
    pub fn tape(thickness: Thickness) -> Self {
        Self {
            thickness,
            span: RuleSpan::Tape,
        }
    }
}

/// Ink in the cell. Last [`ColAlign::End`] hangs the table on the tape; all-Start is compact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColAlign {
    Start,
    End,
}

/// Two or three columns. [`ColAlign::End`] shapes with tabular figures.
pub struct Cols<'a> {
    pub size: TextSize,
    pub gutter: crate::leading::GridSkip,
    pub body: ColBody<'a>,
}

/// Column count is the variant. A one-column or four-column grid has no value.
pub enum ColBody<'a> {
    Two {
        align: [ColAlign; 2],
        rows: Vec<[Vec<Span<'a>>; 2]>,
    },
    Three {
        align: [ColAlign; 3],
        rows: Vec<[Vec<Span<'a>>; 3]>,
    },
}

impl<'a> Cols<'a> {
    pub fn two(
        size: TextSize,
        gutter: crate::leading::GridSkip,
        align: [ColAlign; 2],
        rows: Vec<[Vec<Span<'a>>; 2]>,
    ) -> Self {
        Self {
            size,
            gutter,
            body: ColBody::Two { align, rows },
        }
    }

    pub fn three(
        size: TextSize,
        gutter: crate::leading::GridSkip,
        align: [ColAlign; 3],
        rows: Vec<[Vec<Span<'a>>; 3]>,
    ) -> Self {
        Self {
            size,
            gutter,
            body: ColBody::Three { align, rows },
        }
    }
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

/// List mark for one item. A task is not a nullable dash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemMark {
    List,
    Task { checked: bool },
}

/// Body of one item. An empty `Vec<Frame>` becomes [`ItemBody::Blank`].
pub enum ItemBody<'a> {
    Blank,
    Frames(NonEmpty<Frame<'a>>),
}

impl<'a> ItemBody<'a> {
    pub fn from_frames(frames: Vec<Frame<'a>>) -> Self {
        match NonEmpty::from_vec(frames) {
            None => Self::Blank,
            Some(frames) => Self::Frames(frames),
        }
    }

    pub fn frames(&self) -> &[Frame<'a>] {
        match self {
            Self::Blank => &[],
            Self::Frames(frames) => frames.as_slice(),
        }
    }
}

/// One list item. A task replaces the dash or decimal with a drawn checkbox.
pub struct ListItem<'a> {
    pub mark: ItemMark,
    pub body: ItemBody<'a>,
}

impl<'a> ListItem<'a> {
    pub fn new(frames: Vec<Frame<'a>>) -> Self {
        Self {
            mark: ItemMark::List,
            body: ItemBody::from_frames(frames),
        }
    }

    pub fn task(checked: bool, frames: Vec<Frame<'a>>) -> Self {
        Self {
            mark: ItemMark::Task { checked },
            body: ItemBody::from_frames(frames),
        }
    }

    pub fn frames(&self) -> &[Frame<'a>] {
        self.body.frames()
    }
}

/// CommonMark list density. A bool would not say which way is tight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListFit {
    Tight,
    Loose,
}

/// Hanging list. Marker in the margin; runovers align with the text, not the mark.
pub struct List<'a> {
    pub size: TextSize,
    pub cut: Cut,
    pub marker: Marker,
    pub fit: ListFit,
    pub items: Vec<ListItem<'a>>,
}

impl List<'_> {
    pub fn hang_dots(&self, face: &TextFace) -> Result<u16, Error> {
        let space = face.shape_run(" ", self.size)?.advance();
        let grid = crate::size::to_frac(GRID);
        let mark = self.mark_width(face)?;
        let units = i64::from(grid);
        let cells = ((mark.units() + space.units() + units - 1) / units).max(1);
        let cells = u16::try_from(cells).map_err(|_| Error::CoordinateOverflow)?;
        cells.checked_mul(GRID).ok_or(Error::CoordinateOverflow)
    }

    pub(crate) fn mark_width(&self, face: &TextFace) -> Result<Advance, Error> {
        let mut mark_w = face.shape_run(EN_DASH, self.size)?.advance();
        let fig = face.shape_figure_run("10.", self.size)?.advance();
        if fig.units() > mark_w.units() {
            mark_w = fig;
        }
        let task = Advance::from_dots(i32::from(TASK_BOX)).ok_or(Error::CoordinateOverflow)?;
        if task.units() > mark_w.units() {
            mark_w = task;
        }
        if let Marker::Decimal { start, delim } = self.marker {
            let n = u32::try_from(self.items.len()).map_err(|_| Error::CoordinateOverflow)?;
            for i in 0..n {
                let t = decimal_text(
                    start.checked_add(i).ok_or(Error::CoordinateOverflow)?,
                    delim,
                );
                let w = face.shape_figure_run(&t, self.size)?.advance();
                if w.units() > mark_w.units() {
                    mark_w = w;
                }
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
    lines: Vec<Cow<'a, str>>,
}

impl<'a> Code<'a> {
    pub fn new(size: TextSize, literal: &str) -> Code<'static> {
        let mut lines: Vec<Cow<'static, str>> = split_code_lines(literal)
            .into_iter()
            .map(|s| Cow::Owned(detab(s).into_owned()))
            .collect();
        if lines.last().is_some_and(|s| s.is_empty()) {
            lines.pop();
        }
        Code { size, lines }
    }

    pub fn size(&self) -> TextSize {
        self.size
    }

    pub fn lines(&self) -> &[Cow<'a, str>] {
        &self.lines
    }
}

const TAB_STOP: usize = 8;

fn split_code_lines(literal: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut rest = literal;
    loop {
        match rest.find(['\r', '\n']) {
            None => {
                lines.push(rest);
                break;
            }
            Some(at) => {
                lines.push(&rest[..at]);
                rest = match rest.as_bytes().get(at..at + 2) {
                    Some(b"\r\n") => &rest[at + 2..],
                    _ => &rest[at + 1..],
                };
            }
        }
    }
    lines
}

/// Expand U+0009 to the next multiple of eight columns.
pub fn detab(s: &str) -> Cow<'_, str> {
    if !s.contains('\t') {
        return Cow::Borrowed(s);
    }
    let mut col = 0usize;
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '\t' {
            let n = TAB_STOP - (col % TAB_STOP);
            out.extend(std::iter::repeat_n(' ', n));
            col += n;
        } else {
            out.push(c);
            col = if c == '\n' { 0 } else { col + 1 };
        }
    }
    Cow::Owned(out)
}

/// One node of the typesetting language.
pub enum Frame<'a> {
    Source {
        location: crate::error::SourceLocation,
        frame: Box<Frame<'a>>,
    },
    Text(TextBlock<'a>),
    Head(Head<'a>),
    Mark(Mark<'a>),
    Cols(Cols<'a>),
    List(List<'a>),
    Quote(Quote<'a>),
    Code(Code<'a>),
    Figure(Figure),
    Math(Math),
    Rule(Rule),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::face::Cut;
    use crate::frame::inline::Thickness;

    fn plain(text: &str) -> Frame<'static> {
        Frame::Text(TextBlock::plain(
            Cut::Roman,
            TextSize::Pt11,
            text.to_owned(),
        ))
    }

    #[test]
    fn empty_item_frames_are_blank() {
        assert!(matches!(ItemBody::from_frames(vec![]), ItemBody::Blank));
        assert!(matches!(ListItem::new(vec![]).body, ItemBody::Blank));
        assert!(ListItem::new(vec![]).frames().is_empty());
        assert_eq!(Rule::tape(Thickness::Two).span, RuleSpan::Tape);
    }

    #[test]
    fn nonempty_item_is_not_blank() {
        match ItemBody::from_frames(vec![plain("x")]) {
            ItemBody::Blank => panic!("nonempty vec is Frames"),
            ItemBody::Frames(frames) => {
                assert_eq!(frames.len(), 1);
                assert!(matches!(frames.first(), Frame::Text(_)));
            }
        }
        assert_eq!(ListItem::new(vec![plain("x")]).frames().len(), 1);
    }

    #[test]
    fn detab_advances_to_column_eight() {
        assert_eq!(detab("col\tumn"), "col     umn");
        assert_eq!(detab("\tx"), "        x");
        assert_eq!(detab("no tabs"), "no tabs");
    }

    #[test]
    fn code_new_is_the_parse() {
        let code = Code::new(TextSize::Pt11, "col\tumn\nnext\n");
        assert_eq!(code.size(), TextSize::Pt11);
        assert_eq!(
            code.lines().iter().map(AsRef::as_ref).collect::<Vec<_>>(),
            ["col     umn", "next"]
        );
        assert!(code.lines().iter().all(|l| !l.contains('\t')));
    }

    #[test]
    fn code_splits_crlf_cr_and_lf() {
        let code = Code::new(TextSize::Pt11, "a\r\nb\rc\nd");
        assert_eq!(
            code.lines().iter().map(AsRef::as_ref).collect::<Vec<_>>(),
            ["a", "b", "c", "d"]
        );
        let trailing = Code::new(TextSize::Pt11, "a\r\nb\r\n");
        assert_eq!(
            trailing
                .lines()
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        let blank = Code::new(TextSize::Pt11, "keep\n\nlast\n");
        assert_eq!(
            blank.lines().iter().map(AsRef::as_ref).collect::<Vec<_>>(),
            ["keep", "", "last"]
        );
    }
}
