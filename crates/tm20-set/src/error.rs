//! Host type errors. Protocol and I/O stay outside.

use std::fmt;

use tm20::RasterError;

use crate::face::{Cut, DisplayCut};

/// Original source location, retained through lowering and layout.
#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
    pub text: String,
}

impl SourceLocation {
    pub fn annotate(&self, error: Error) -> Error {
        if matches!(error, Error::At { .. }) {
            error
        } else {
            let text = match &error {
                Error::MissingGlyph { text } | Error::InText { text, .. } => Some(text.as_str()),
                Error::Clipped { text, .. } => text.as_deref(),
                _ => None,
            };
            let location = text.map_or_else(|| self.clone(), |text| self.locate(text));
            Error::At {
                location,
                cause: Box::new(error),
            }
        }
    }

    #[must_use]
    pub fn locate(&self, text: &str) -> Self {
        if text.is_empty() {
            return self.clone();
        }
        // The stored snippet includes the complete first line, even when the
        // source node starts later. Never attribute an error to earlier text.
        let start = self
            .text
            .char_indices()
            .nth(self.column.saturating_sub(1))
            .map_or(self.text.len(), |(offset, _)| offset);
        let Some(relative) = self.text[start..].find(text) else {
            return self.clone();
        };
        let offset = start + relative;
        let prefix = &self.text[..offset];
        let line_delta = prefix.bytes().filter(|&b| b == b'\n').count();
        let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        Self {
            line: self.line + line_delta,
            column,
            text: self
                .text
                .lines()
                .skip(line_delta)
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "line {}, column {}\n  {}\n  {}^",
            self.line,
            self.column,
            self.text.lines().next().unwrap_or(""),
            " ".repeat(self.column.saturating_sub(1))
        )
    }
}

#[derive(Debug)]
pub enum Error {
    InText {
        text: String,
        cause: Box<Error>,
    },
    At {
        location: SourceLocation,
        cause: Box<Error>,
    },
    Clipped {
        text: Option<std::sync::Arc<str>>,
        left: i32,
        right: i32,
        available_start: u16,
        available_end: u16,
    },
    MissingGlyph {
        text: String,
    },
    Font,
    MissingText(Cut),
    MissingDisplay(DisplayCut),
    Image,
    ImageDetail(image::ImageError),
    Nesting,
    Raster(RasterError),
    InvalidNote {
        number: u32,
        count: usize,
    },
    InvalidImage,
    InvalidBaseline,
    UnsupportedMeasure,
    CoordinateOverflow,
    Alloc,
    ImpossibleColumns,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InText { text, cause } => write!(f, "{cause}; offending text: {text:?}"),
            Error::At { location, cause } => write!(f, "{location}\n{cause}"),
            Error::Clipped {
                text,
                left,
                right,
                available_start,
                available_end,
            } => {
                write!(
                    f,
                    "content occupies dots {left}..{right}, outside available {available_start}..{available_end}; shorten the text, break the code line, or reduce nesting"
                )?;
                if let Some(text) = text {
                    write!(f, "; offending text: {text:?}")?;
                }
                Ok(())
            }
            Error::MissingGlyph { text } => write!(
                f,
                "the selected font cannot render {text:?}; replace unsupported characters or supply a font containing them"
            ),
            Error::Font => write!(f, "could not parse typeface"),
            Error::MissingText(cut) => write!(f, "no text face for {cut}"),
            Error::MissingDisplay(cut) => write!(f, "no display face for {cut}"),
            Error::Image => write!(f, "could not decode figure"),
            Error::ImageDetail(cause) => write!(f, "image codec failed: {cause}"),
            Error::Nesting => write!(f, "quote or list nested more than three deep"),
            Error::Raster(e) => write!(f, "{e}"),
            Error::InvalidNote { number, count } => {
                write!(f, "note {number} is not among {count} notes")
            }
            Error::InvalidImage => write!(f, "image source is not a valid raster"),
            Error::InvalidBaseline => write!(f, "math ascent exceeds height"),
            Error::UnsupportedMeasure => write!(f, "measure is not admitted for this typesetter"),
            Error::CoordinateOverflow => write!(f, "layout coordinate overflow"),
            Error::Alloc => write!(f, "layout allocation failed"),
            Error::ImpossibleColumns => write!(f, "column geometry cannot fit the measure"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::At { cause, .. } | Self::InText { cause, .. } => Some(cause),
            Self::Raster(cause) => Some(cause),
            Self::ImageDetail(cause) => Some(cause),
            _ => None,
        }
    }
}

impl From<RasterError> for Error {
    fn from(e: RasterError) -> Self {
        Error::Raster(e)
    }
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::At { cause, .. } | Self::InText { cause, .. } => cause.code(),
            Self::Clipped { .. } => "layout.overflow",
            Self::MissingGlyph { .. } => "font.missing-glyph",
            Self::Font => "font.invalid",
            Self::MissingText(_) | Self::MissingDisplay(_) => "font.missing-face",
            Self::Image | Self::ImageDetail(_) | Self::InvalidImage | Self::InvalidBaseline => {
                "image.invalid"
            }
            Self::Nesting => "layout.nesting",
            Self::Raster(_) => "raster.invalid",
            Self::InvalidNote { .. } => "note.invalid",
            Self::UnsupportedMeasure => "layout.measure",
            Self::CoordinateOverflow => "layout.coordinate-overflow",
            Self::Alloc => "allocation.failed",
            Self::ImpossibleColumns => "layout.columns",
        }
    }

    pub fn location(&self) -> Option<&SourceLocation> {
        match self {
            Self::At { location, .. } => Some(location),
            Self::InText { cause, .. } => cause.location(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SourceLocation;

    #[test]
    fn source_lookup_cannot_point_before_its_node() {
        let source = SourceLocation {
            line: 7,
            column: 6,
            text: "écho écho\nnext".into(),
        };
        let at = source.locate("écho");
        assert_eq!((at.line, at.column), (7, 6));
        let next = source.locate("next");
        assert_eq!((next.line, next.column), (8, 1));
    }
}
