//! Host errors for the markdown walk. OpenType stays in tm20-set.

use std::fmt;

#[derive(Debug)]
pub enum Error {
    Html,
    MixedImage,
    Image,
    Nesting,
    Cols,
    Note,
    Set(tm20_set::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Html => write!(f, "raw HTML is not representable"),
            Error::MixedImage => write!(f, "a paragraph cannot mix text and an image"),
            Error::Image => write!(f, "could not load figure"),
            Error::Nesting => write!(f, "quote or list nested more than three deep"),
            Error::Cols => write!(f, "table must have two or three columns"),
            Error::Note => write!(f, "footnote has no definition"),
            Error::Set(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<tm20_set::Error> for Error {
    fn from(e: tm20_set::Error) -> Self {
        match e {
            tm20_set::Error::Image => Error::Image,
            tm20_set::Error::Nesting => Error::Nesting,
            tm20_set::Error::Cols => Error::Cols,
            other => Error::Set(other),
        }
    }
}
