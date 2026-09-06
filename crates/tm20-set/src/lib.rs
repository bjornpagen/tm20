//! Typesetting language for [`tm20`]. A [`Sheet`] of [`Frame`]s plus a
//! [`FaceTable`] composes to one tape-wide [`tm20::Raster`].
//!
//! Pipeline: author a [`Sheet`] → [`Sheet::admit`] → resolve →
//! [`compose::layout`] → [`compose::paint`]. [`select_bands`] /
//! [`page_bands`] are a later narrowing. OpenType never
//! enters the protocol crate. Faces are bytes; which files they came from is
//! not this crate’s decision. [`HOUSE`] is the PostScript-name → [`Voice`]
//! table that fills a table from a collection; compose resolves a
//! [`ResolvedFaces`] once. Coordinates are [`Advance`] (26.6) and
//! [`InkBounds`]; notes raise by translating a [`ShapedRun`], not by a
//! parallel note field on [`Span`].
//!
//! The default `portable-fonts` feature provides `FaceTable::portable()` and
//! `FONT_LICENSES`. Disable default features to supply only your own fonts.
//! ```
//! # #[cfg(feature = "portable-fonts")]
//! # {
//! let faces = tm20_set::FaceTable::portable()?;
//! let mut sheet = tm20_set::SheetBuilder::new(tm20_set::Measure::TAPE);
//! sheet.push_frame(tm20_set::Frame::Text(tm20_set::TextBlock::plain(
//!     tm20_set::Cut::Roman, tm20_set::TextSize::Pt11, "Hello, tape!",
//! )));
//! let document = tm20_set::lower(&sheet.finish()?, &faces)?;
//! assert!(!tm20::encode(&document)?.is_empty());
//! # }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod cols;
mod compose;
mod error;
mod face;
mod frame;
mod geometry;
mod leading;
mod lower;
#[cfg(feature = "portable-fonts")]
mod portable;
mod preview;
mod size;
mod strike;

pub use compose::{
    DigitMode, DrawOp, LayoutPlan, LinePlan, LinePlans, PaintAtom, WrapCost, compose,
    graphics_bands, layout, page_bands, paint, select_bands, wrap_text,
};
pub use error::{Error, SourceLocation};
pub use face::{
    Cut, DisplayCut, DisplayFace, Face, FaceRequirements, FaceTable, HOUSE, PositionedGlyph,
    ResolvedFaces, ShapedRun, Strike, TextFace, Voice,
};
pub use frame::{
    CheckedSheet, Code, ColAlign, ColBody, Cols, DecimalDelim, EN_DASH, Figure, FittedMath, Frame,
    Head, ItemBody, ItemMark, List, ListFit, ListItem, Mark, MarkAlign, Marker, Math, Measure,
    NonEmpty, Note, NoteId, Quote, Rule, RuleSpan, Sheet, SheetBuilder, Span, TextBlock, Thickness,
    Tracking, detab,
};
pub use geometry::{Advance, Clip, InkBounds, Row, RowRange};
pub use leading::{GRID, GridSkip, HANG, Leading, NOTE_RULE, TASK_BOX, pt_dots};
pub use lower::{lower, lower_bands, lower_page};
#[cfg(feature = "portable-fonts")]
pub use portable::FONT_LICENSES;
pub use preview::{preview_png, preview_pngs, preview_raster};
pub use size::{DPI, DisplaySize, TextSize};

pub type Result<T> = std::result::Result<T, Error>;
