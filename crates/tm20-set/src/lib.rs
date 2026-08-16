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
pub use face::{DisplayFace, Face, TextFace, Weight};
pub use frame::{
    Frame, Mark, MarkAlign, Measure, Pair, Rule, Sheet, TextBlock, Thickness, Tracking,
};
pub use leading::{GridSkip, Leading, GRID};
pub use lower::lower;
pub use size::{DisplaySize, TextSize, DPI};

pub type Result<T> = std::result::Result<T, Error>;

pub use specimens::{catalog, find as find_sheet, Case as SheetCase};
