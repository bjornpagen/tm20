//! Typesetting language for [`tm20_set`]. comrak parses CommonMark; this crate
//! walks the AST into a [`tm20_set::Sheet`]. HTML never becomes a Frame.

mod error;
mod image;
mod lower;
mod math;

pub use error::Error;
pub use image::image_bytes;
pub use lower::sheet;

pub type Result<T> = std::result::Result<T, Error>;
