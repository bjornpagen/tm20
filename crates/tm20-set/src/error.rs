//! Host type errors. Protocol errors stay in `tm20`.

use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
    Font,
    NotFound { family: String },
    Overflow { width: u32, height: u32 },
    HeadNotBold,
    Columns,
    Io(io::Error),
    Protocol(tm20::Error),
    Encode(tm20::EncodeError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Font => write!(f, "could not parse typeface"),
            Error::NotFound { family } => write!(f, "system typeface {family:?} not found"),
            Error::Overflow { width, height } => {
                write!(f, "sheet raster {width}x{height} does not fit Graphics")
            }
            Error::HeadNotBold => write!(f, "Head requires a Bold TextFace"),
            Error::Columns => write!(f, "column widths plus gutters must equal the measure"),
            Error::Io(e) => write!(f, "{e}"),
            Error::Protocol(e) => write!(f, "{e}"),
            Error::Encode(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<tm20::Error> for Error {
    fn from(e: tm20::Error) -> Self {
        Error::Protocol(e)
    }
}

impl From<tm20::EncodeError> for Error {
    fn from(e: tm20::EncodeError) -> Self {
        Error::Encode(e)
    }
}
