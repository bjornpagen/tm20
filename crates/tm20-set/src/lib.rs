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
mod size;

pub use compose::compose;
pub use error::Error;
pub use face::{Cut, DisplayFace, Face, FaceTable, TextFace};
pub use frame::{
    Frame, Head, List, Mark, MarkAlign, Measure, Pair, Rule, Sheet, Span, TextBlock, Thickness,
    Tracking, EN_DASH,
};
pub use leading::{GridSkip, Leading, GRID, HANG};
pub use lower::lower;
pub use size::{DisplaySize, TextSize, DPI};

pub type Result<T> = std::result::Result<T, Error>;
