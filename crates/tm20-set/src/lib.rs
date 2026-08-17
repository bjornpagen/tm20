//! Typesetting language for [`tm20`]. A [`Sheet`] of [`Frame`]s plus a
//! [`FaceTable`] lowers to protocol [`tm20::Graphics`]. OpenType never enters
//! the protocol crate. Faces are bytes; which files they came from is not
//! this crate’s decision.

mod compose;
mod error;
mod face;
mod frame;
mod leading;
mod lower;
mod preview;
mod size;

pub use compose::compose;
pub use error::Error;
pub use face::{Cut, DisplayFace, Face, FaceTable, TextFace};
pub use frame::{
    Code, ColAlign, ColBody, Cols, DecimalDelim, Figure, Frame, Head, ItemMark, List, ListFit,
    ListItem, Mark, MarkAlign, Marker, Math, Measure, Note, Quote, Rule, Sheet, Span, TextBlock,
    Thickness, Tracking, EN_DASH,
};
pub use leading::{pt_dots, GridSkip, Leading, GRID, HANG, NOTE_RULE, TASK_BOX};
pub use lower::lower;
pub use preview::preview_png;
pub use size::{DisplaySize, TextSize, DPI};

pub type Result<T> = std::result::Result<T, Error>;
