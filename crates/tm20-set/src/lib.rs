//! Typesetting language for [`tm20`]. A [`Sheet`] of [`Frame`]s lowers to
//! protocol [`tm20::Graphics`]. OpenType never enters the protocol crate.

mod compose;
mod error;
mod face;
mod frame;
mod leading;
mod lower;
mod size;
mod specimens;

pub use compose::compose;
pub use error::Error;
pub use face::{DisplayFace, Face, Slope, TextFace, Weight};
pub use frame::{
    Cell, Columns, Frame, Head, List, Mark, MarkAlign, Measure, Row, Rule, Sheet, Span, Table,
    TextBlock, Thickness, Tracking, EN_DASH,
};
pub use leading::{GridSkip, Leading, GRID, HANG};
pub use lower::lower;
pub use size::{DisplaySize, TextSize, DPI};

pub type Result<T> = std::result::Result<T, Error>;

pub use specimens::{catalog, find as find_sheet, Case as SheetCase};
