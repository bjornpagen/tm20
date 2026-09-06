//! Typesetting language for [`tm20_set`]. comrak parses CommonMark; this crate
//! walks the AST into a [`tm20_set::Sheet`]. HTML never becomes a Frame.
//!
//! With `tm20`, `tm20-md`, and `tm20-set` as dependencies:
//! ```
//! use std::path::Path;
//!
//! let faces = tm20_set::FaceTable::portable()?;
//! let sheet = tm20_md::sheet("# Hello\n\nPrinted **locally**.", tm20_set::Measure::TAPE,
//!     |url| tm20_md::image_bytes(Path::new("."), url))?;
//! let document = tm20_set::lower(&sheet, &faces)?;
//! let bytes = tm20::encode(&document)?;
//! assert!(!bytes.is_empty()); // Ready for a file or Transport::write; no printer opened.
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! [`image_bytes`] reads local files only; it does not sandbox paths.
//! Supply your own image-loader callback for different access/network policies.
//! Parsing, layout, and encoding finish before you choose to perform delivery.

mod error;
mod image;
mod lower;
mod math;

pub use error::Error;
pub use image::image_bytes;
pub use lower::sheet;

pub type Result<T> = std::result::Result<T, Error>;
