//! [`FaceTable`] registry and borrowed [`ResolvedFaces`] for one layout.

use std::sync::Arc;

use crate::error::Error;

use super::parse::{DisplayFace, Face, TextFace};
use super::{Cut, DisplayCut, FaceRequirements, HOUSE, Voice};

/// Loaded cuts. The sheet names [`Cut`]s; this table is what those names mean.
#[derive(Clone, Default)]
pub struct FaceTable {
    text: [Option<TextFace>; Cut::COUNT],
    display: [Option<DisplayFace>; DisplayCut::COUNT],
}

impl FaceTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_text(&mut self, cut: Cut, face: TextFace) {
        self.text[cut.index()] = Some(face);
    }

    pub fn set_display(&mut self, cut: DisplayCut, face: DisplayFace) {
        self.display[cut.index()] = Some(face);
    }

    /// Offer a parsed face to the slots its PostScript name owns. Unknown
    /// names are ignored — a collection is a bag, not a kit.
    pub fn offer(&mut self, face: Face) -> bool {
        let Some(name) = face.postscript_name() else {
            return false;
        };
        let Some((_, voice)) = HOUSE.iter().find(|(n, _)| *n == name.as_str()) else {
            return false;
        };
        match *voice {
            Voice::Text(cut) => self.set_text(cut, face.text()),
            Voice::Display(cut) => self.set_display(cut, face.display()),
            Voice::Both(text, display) => {
                self.set_display(display, face.reopen().display());
                self.set_text(text, face.text());
            }
        }
        true
    }

    /// Walk every face in an sfnt collection and [`offer`](Self::offer) each.
    /// The first unparseable index is the end of the collection.
    pub fn absorb(&mut self, bytes: impl Into<Arc<[u8]>>) {
        let bytes = bytes.into();
        for index in 0.. {
            let Ok(name) = Face::name_at(&bytes, index) else {
                break;
            };
            if !name.is_some_and(|name| HOUSE.iter().any(|(known, _)| *known == name)) {
                continue;
            }
            let Ok(face) = Face::from_bytes_index(bytes.clone(), index) else {
                break;
            };
            self.offer(face);
        }
    }

    pub fn text(&self, cut: Cut) -> Result<&TextFace, Error> {
        self.text[cut.index()]
            .as_ref()
            .ok_or(Error::MissingText(cut))
    }

    pub fn display(&self, cut: DisplayCut) -> Result<&DisplayFace, Error> {
        self.display[cut.index()]
            .as_ref()
            .ok_or(Error::MissingDisplay(cut))
    }

    /// Resolve the cuts a checked sheet actually uses. Unused Light (or any
    /// unused cut) is not required. A used missing cut fails here, before paint.
    pub fn resolve(&self, requirements: &FaceRequirements) -> Result<ResolvedFaces<'_>, Error> {
        let mut text = [None; Cut::COUNT];
        let mut display = [None; DisplayCut::COUNT];
        for cut in Cut::ALL {
            if requirements.needs_text(cut) {
                text[cut.index()] = Some(self.text(cut)?);
            }
        }
        for cut in DisplayCut::ALL {
            if requirements.needs_display(cut) {
                display[cut.index()] = Some(self.display(cut)?);
            }
        }
        Ok(ResolvedFaces {
            table: self,
            text,
            display,
        })
    }
}

/// Borrowed faces for one layout, tied to the [`FaceTable`] that owns them.
///
/// Handles are `'f` borrows of table slots. Prepared runs own [`super::ShapedRun`]
/// values and do not look up this table again. The table's `RefCell` caches are
/// not `Sync`; do not send a live table across threads.
pub struct ResolvedFaces<'f> {
    table: &'f FaceTable,
    text: [Option<&'f TextFace>; Cut::COUNT],
    display: [Option<&'f DisplayFace>; DisplayCut::COUNT],
}

impl<'f> ResolvedFaces<'f> {
    pub fn table(&self) -> &'f FaceTable {
        self.table
    }

    pub fn text(&self, cut: Cut) -> Result<&'f TextFace, Error> {
        self.text[cut.index()].ok_or(Error::MissingText(cut))
    }

    pub fn display(&self, cut: DisplayCut) -> Result<&'f DisplayFace, Error> {
        self.display[cut.index()].ok_or(Error::MissingDisplay(cut))
    }
}
