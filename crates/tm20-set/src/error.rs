//! Host type errors. Protocol and I/O stay outside.

use std::fmt;

use crate::face::Cut;

#[derive(Debug)]
pub enum Error {
    Font,
    MissingText(Cut),
    MissingDisplay(Cut),
    Overflow { width: u32, height: u32 },
    Image,
    Nesting,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Font => write!(f, "could not parse typeface"),
            Error::MissingText(cut) => write!(f, "no text face for {cut}"),
            Error::MissingDisplay(cut) => write!(f, "no display face for {cut}"),
            Error::Overflow { width, height } => {
                write!(f, "sheet raster {width}x{height} does not fit Graphics")
            }
            Error::Image => write!(f, "could not decode figure"),
            Error::Nesting => write!(f, "quote or list nested more than three deep"),
        }
    }
}

impl std::error::Error for Error {}
