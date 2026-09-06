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

mod cols;
mod compose;
mod error;
mod face;
mod frame;
mod geometry;
mod leading;
mod lower;
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
pub use preview::{preview_png, preview_pngs, preview_raster};
pub use size::{DPI, DisplaySize, TextSize};

pub type Result<T> = std::result::Result<T, Error>;
