//! The document: a [`Sheet`] of [`Frame`]s. Ugly layouts have no variant.
//! Faces are named [`Cut`]s; [`crate::FaceTable`] says what those names mean.

mod blocks;
mod document;
mod image;
mod inline;
mod nonempty;

pub use crate::geometry::Measure;
pub(crate) use blocks::decimal_text;
pub use blocks::{
    Code, ColAlign, ColBody, Cols, DecimalDelim, EN_DASH, Frame, ItemBody, ItemMark, List, ListFit,
    ListItem, Marker, Quote, Rule, RuleSpan, detab,
};
pub use document::{CheckedSheet, Note, Sheet, SheetBuilder};
pub use image::{Figure, FittedMath, Math};
pub use inline::{Head, Mark, MarkAlign, NoteId, Span, TextBlock, Thickness, Tracking};
pub use nonempty::NonEmpty;

#[cfg(test)]
mod tests {
    use super::*;
    use tm20::PRINTABLE_DOTS;

    #[test]
    fn measure_zero_is_none() {
        assert!(Measure::new(0).is_none());
        assert_eq!(Measure::new(1).unwrap().get(), 1);
        assert_eq!(Measure::TAPE.get(), PRINTABLE_DOTS);
        assert!(Measure::new(PRINTABLE_DOTS + 1).is_none());
    }
}
