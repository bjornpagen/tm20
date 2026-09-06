//! Host errors for the markdown walk. OpenType stays in tm20-set.

use std::fmt;

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    RemoteImageDenied {
        destination: String,
    },
    At {
        location: tm20_set::SourceLocation,
        cause: Box<Error>,
    },
    Unsupported {
        feature: String,
        reason: String,
        help: String,
    },
    Resource {
        destination: String,
        reason: String,
    },
    MathDetail(String),
    Html,
    MixedImage,
    Image,
    Math,
    Nesting,
    Cols,
    Note,
    Set(tm20_set::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::RemoteImageDenied { destination } => write!(
                f,
                "remote image {destination:?} is disabled by the image policy\nhelp: use a local PNG/JPEG; pass --allow-remote-images only when network access is intended"
            ),
            Error::At { location, cause } => write!(f, "{location}\n{cause}"),
            Error::Unsupported {
                feature,
                reason,
                help,
            } => write!(f, "unsupported {feature}: {reason}\nhelp: {help}"),
            Error::Resource {
                destination,
                reason,
            } => write!(
                f,
                "cannot load image {destination:?}: {reason}\nhelp: use a readable PNG/JPEG file or a working HTTP(S) image URL"
            ),
            Error::MathDetail(reason) => write!(
                f,
                "invalid math: {reason}\nhelp: correct the LaTeX expression using supported RaTeX commands"
            ),
            Error::Html => write!(
                f,
                "raw HTML is not representable\nhelp: replace it with Markdown, or escape it to print literal text"
            ),
            Error::MixedImage => write!(
                f,
                "an image must stand alone in its paragraph\nhelp: separate the image from surrounding text, links, or formatting with blank lines"
            ),
            Error::Image => write!(f, "could not load figure"),
            Error::Math => write!(
                f,
                "could not typeset math in this context\nhelp: move heading math into a paragraph and check the LaTeX expression"
            ),
            Error::Nesting => write!(
                f,
                "quote or list nested more than three deep\nhelp: flatten the fourth nesting level"
            ),
            Error::Cols => write!(
                f,
                "table must have two or three columns\nhelp: split wider tables or use a list for one-column content"
            ),
            Error::Note => write!(f, "footnote has no definition"),
            Error::Set(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Set(error) => Some(error),
            Self::At { cause, .. } => Some(cause),
            _ => None,
        }
    }
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::At { cause, .. } => cause.code(),
            Self::RemoteImageDenied { .. } => "image.remote-denied",
            Self::Set(cause) => cause.code(),
            Self::Unsupported { .. } => "markdown.unsupported",
            Self::Resource { .. } | Self::Image => "image.failed",
            Self::MathDetail(_) | Self::Math => "math.invalid",
            Self::Html => "markdown.html",
            Self::MixedImage => "markdown.mixed-image",
            Self::Nesting => "layout.nesting",
            Self::Cols => "markdown.columns",
            Self::Note => "markdown.undefined-note",
        }
    }

    pub fn location(&self) -> Option<&tm20_set::SourceLocation> {
        match self {
            Self::At { location, .. } => Some(location),
            Self::Set(cause) => cause.location(),
            _ => None,
        }
    }

    pub(crate) fn at(self, location: tm20_set::SourceLocation) -> Self {
        if matches!(self, Self::At { .. }) {
            self
        } else {
            Self::At {
                location,
                cause: Box::new(self),
            }
        }
    }

    pub fn unsupported(
        feature: impl Into<String>,
        reason: impl Into<String>,
        help: impl Into<String>,
    ) -> Self {
        Self::Unsupported {
            feature: feature.into(),
            reason: reason.into(),
            help: help.into(),
        }
    }
}

impl From<tm20_set::Error> for Error {
    fn from(e: tm20_set::Error) -> Self {
        match e {
            tm20_set::Error::Image => Error::Image,
            tm20_set::Error::Nesting => Error::Nesting,
            other => Error::Set(other),
        }
    }
}
