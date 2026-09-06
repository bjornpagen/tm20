//! Inline authoring values. [`Span::Note`] is an independent reference.

use std::borrow::Cow;
use std::num::NonZeroU32;

use crate::face::Cut;
use crate::frame::image::Math;
use crate::size::{DisplaySize, TextSize};

/// Private nonzero one-based note index. Sheet-local, not a global identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NoteId(NonZeroU32);

impl NoteId {
    pub fn from_index(n: NonZeroU32) -> Self {
        Self(n)
    }

    pub fn get(self) -> NonZeroU32 {
        self.0
    }

    pub fn get_u32(self) -> u32 {
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
/// A note is its own span, not a field on text.
#[derive(Clone)]
pub enum Span<'a> {
    Type { cut: Cut, text: Cow<'a, str> },
    Math(Math),
    Note(NoteId),
    Strike(Vec<Span<'a>>),
}

impl<'a> Span<'a> {
    pub fn new(cut: Cut, text: impl Into<Cow<'a, str>>) -> Self {
        Self::Type {
            cut,
            text: text.into(),
        }
    }

    pub fn math(m: Math) -> Self {
        Self::Math(m)
    }

    pub fn note(id: NoteId) -> Self {
        Self::Note(id)
    }
}

/// One paragraph. Flush left, rag right. Adjacent [`crate::Frame::Text`] values
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

/// Display line. Wraps to the measure. Center is only legal here.
pub struct Mark<'a> {
    pub cut: crate::face::DisplayCut,
    pub size: DisplaySize,
    pub text: Cow<'a, str>,
    pub align: MarkAlign,
    pub tracking: Tracking,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::face::Cut;

    fn nid(n: u32) -> NoteId {
        NoteId::from_index(NonZeroU32::new(n).unwrap())
    }

    #[test]
    fn type_span_has_no_note_field() {
        let s = Span::new(Cut::Roman, "hello");
        match s {
            Span::Type { cut, text } => {
                assert_eq!(cut, Cut::Roman);
                assert_eq!(text, "hello");
            }
            Span::Math(_) | Span::Note(_) | Span::Strike(_) => panic!("type span"),
        }
    }

    #[test]
    fn note_is_an_independent_span() {
        let spans = [
            Span::new(Cut::Italic, "Canon"),
            Span::note(nid(1)),
            Span::new(Cut::Roman, " and "),
            Span::note(nid(2)),
        ];
        let notes: Vec<u32> = spans
            .iter()
            .filter_map(|s| match s {
                Span::Note(id) => Some(id.get_u32()),
                _ => None,
            })
            .collect();
        assert_eq!(notes, [1, 2]);
    }

    #[test]
    fn convenience_constructors() {
        assert!(matches!(Span::new(Cut::Mono, "x"), Span::Type { .. }));
        assert!(matches!(Span::note(nid(3)), Span::Note(_)));
        let block = TextBlock::plain(Cut::Bold, TextSize::Pt11, "Head");
        assert_eq!(block.spans.len(), 1);
    }
}
