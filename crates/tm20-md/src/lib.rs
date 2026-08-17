//! Typesetting language for [`tm20-set`]. comrak parses CommonMark; this crate
//! walks the AST into a [`Sheet`]. HTML never becomes a Frame.

mod error;
mod lower;

pub use error::Error;
pub use lower::sheet;

pub type Result<T> = std::result::Result<T, Error>;
