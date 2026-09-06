//! Parsed sfnt face (CFF or glyf, one file or a collection). Optical role is a
//! second parse into [`TextFace`] or [`DisplayFace`]. Which [`Cut`] a face fills
//! is decided outside this crate. HarfRust shapes; fontdue paints; this crate
//! caches the strike.

mod parse;
mod registry;
mod shape;

pub use crate::strike::Strike;
pub use parse::{DisplayFace, Face, TextFace};
pub use registry::{FaceTable, ResolvedFaces};
pub use shape::{PositionedGlyph, ShapedRun};

/// Required cuts collected from a checked sheet before [`FaceTable::resolve`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FaceRequirements {
    pub text: [bool; 5],
    pub display: [bool; 2],
}

impl FaceRequirements {
    pub fn need_text(&mut self, cut: Cut) {
        self.text[cut as usize] = true;
    }

    pub fn need_display(&mut self, cut: DisplayCut) {
        self.display[cut as usize] = true;
    }

    pub fn needs_text(&self, cut: Cut) -> bool {
        self.text[cut as usize]
    }

    pub fn needs_display(&self, cut: DisplayCut) -> bool {
        self.display[cut as usize]
    }
}

/// Named text voice. The sheet writes these; [`FaceTable`] says what they are.
/// Display voices are [`DisplayCut`]: a Light paragraph or a Mono masthead is
/// unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Cut {
    Roman,
    Italic,
    Bold,
    BoldItalic,
    Mono,
}

impl Cut {
    pub(crate) const COUNT: usize = 5;
    pub const ALL: [Self; 5] = [
        Self::Roman,
        Self::Italic,
        Self::Bold,
        Self::BoldItalic,
        Self::Mono,
    ];

    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

impl std::fmt::Display for Cut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Cut::Roman => "Roman",
            Cut::Italic => "Italic",
            Cut::Bold => "Bold",
            Cut::BoldItalic => "BoldItalic",
            Cut::Mono => "Mono",
        })
    }
}

/// Named display voice. Only a [`crate::Mark`] speaks at display size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DisplayCut {
    Roman,
    Light,
}

/// Optical slot a PostScript name fills. One name may fill text and display
/// (Helvetica Regular is both Roman voices). The assignment is this table,
/// not a match in each loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voice {
    Text(Cut),
    Display(DisplayCut),
    Both(Cut, DisplayCut),
}

/// House names. Paths stay with the program that reads the files; this table
/// is the parse from a collection face onto a [`Cut`].
pub const HOUSE: &[(&str, Voice)] = &[
    ("Helvetica", Voice::Both(Cut::Roman, DisplayCut::Roman)),
    ("Helvetica-Bold", Voice::Text(Cut::Bold)),
    ("Helvetica-Oblique", Voice::Text(Cut::Italic)),
    ("Helvetica-BoldOblique", Voice::Text(Cut::BoldItalic)),
    ("Helvetica-Light", Voice::Display(DisplayCut::Light)),
    ("Menlo-Regular", Voice::Text(Cut::Mono)),
];

impl DisplayCut {
    pub(crate) const COUNT: usize = 2;
    pub const ALL: [Self; 2] = [Self::Roman, Self::Light];

    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

impl std::fmt::Display for DisplayCut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DisplayCut::Roman => "Roman",
            DisplayCut::Light => "Light",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use std::sync::Arc;

    #[test]
    fn garbage_bytes_are_font_error() {
        assert!(matches!(
            Face::from_bytes(vec![0, 1, 2, 3]),
            Err(Error::Font)
        ));
    }

    #[test]
    fn helvetica_ttc_parses() {
        let bytes = std::fs::read("/System/Library/Fonts/Helvetica.ttc")
            .expect("Helvetica.ttc not on this machine — locked house face required");
        let face = Face::from_bytes_index(bytes, 0).expect("glyf OpenType");
        let name = face.postscript_name().expect("PostScript name");
        assert!(name.starts_with("Helvetica"), "{name}");
    }

    #[test]
    fn house_names_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for (name, _) in HOUSE {
            assert!(seen.insert(*name), "duplicate house name {name}");
        }
    }

    #[test]
    fn helvetica_ttc_names_the_cuts() {
        let bytes: Arc<[u8]> = std::fs::read("/System/Library/Fonts/Helvetica.ttc")
            .expect("Helvetica.ttc not on this machine — locked house face required")
            .into();
        let mut names = Vec::new();
        for index in 0.. {
            let Ok(face) = Face::from_bytes_index(bytes.clone(), index) else {
                break;
            };
            names.push(face.postscript_name().unwrap_or_default());
        }
        for need in [
            "Helvetica",
            "Helvetica-Bold",
            "Helvetica-Oblique",
            "Helvetica-BoldOblique",
        ] {
            assert!(names.iter().any(|n| n == need), "{need} not in {names:?}");
        }
    }

    fn house_without_light() -> FaceTable {
        let mut table = FaceTable::new();
        table.absorb(
            std::fs::read("/System/Library/Fonts/Menlo.ttc")
                .expect("Menlo.ttc not on this machine — locked house face required"),
        );
        let bytes: Arc<[u8]> = std::fs::read("/System/Library/Fonts/Helvetica.ttc")
            .expect("Helvetica.ttc not on this machine — locked house face required")
            .into();
        for index in 0.. {
            let Ok(face) = Face::from_bytes_index(bytes.clone(), index) else {
                break;
            };
            if face.postscript_name().as_deref() == Some("Helvetica-Light") {
                continue;
            }
            table.offer(face);
        }
        table
    }

    #[test]
    fn unused_light_is_accepted() {
        let table = house_without_light();
        let mut req = FaceRequirements::default();
        req.need_text(Cut::Roman);
        req.need_text(Cut::Mono);
        req.need_display(DisplayCut::Roman);
        let resolved = table
            .resolve(&req)
            .expect("Light unused, so a missing Light cut is harmless");
        assert!(resolved.text(Cut::Roman).is_ok());
        assert!(resolved.text(Cut::Mono).is_ok());
        assert!(resolved.display(DisplayCut::Roman).is_ok());
        assert!(matches!(
            resolved.display(DisplayCut::Light),
            Err(Error::MissingDisplay(DisplayCut::Light))
        ));
    }

    #[test]
    fn used_light_missing_fails_at_resolve() {
        let table = house_without_light();
        let mut req = FaceRequirements::default();
        req.need_display(DisplayCut::Light);
        assert!(matches!(
            table.resolve(&req),
            Err(Error::MissingDisplay(DisplayCut::Light))
        ));
    }

    #[test]
    fn used_missing_text_cut_fails_at_resolve() {
        let mut table = FaceTable::new();
        table.absorb(
            std::fs::read("/System/Library/Fonts/Menlo.ttc")
                .expect("Menlo.ttc not on this machine — locked house face required"),
        );
        let mut req = FaceRequirements::default();
        req.need_text(Cut::Bold);
        assert!(matches!(
            table.resolve(&req),
            Err(Error::MissingText(Cut::Bold))
        ));
        req = FaceRequirements::default();
        req.need_text(Cut::Mono);
        assert!(table.resolve(&req).is_ok());
    }

    #[test]
    fn resolve_indexes_house_faces() {
        let mut table = FaceTable::new();
        table.absorb(
            std::fs::read("/System/Library/Fonts/Helvetica.ttc")
                .expect("Helvetica.ttc not on this machine — locked house face required"),
        );
        table.absorb(
            std::fs::read("/System/Library/Fonts/Menlo.ttc")
                .expect("Menlo.ttc not on this machine — locked house face required"),
        );
        let mut req = FaceRequirements::default();
        for cut in Cut::ALL {
            req.need_text(cut);
        }
        req.need_display(DisplayCut::Roman);
        req.need_display(DisplayCut::Light);
        let resolved = table.resolve(&req).expect("house collections fill resolve");
        for cut in Cut::ALL {
            assert!(
                std::ptr::addr_eq(resolved.text(cut).unwrap(), table.text(cut).unwrap()),
                "resolved[{cut}] must be the table's {cut}"
            );
        }
        assert!(std::ptr::addr_eq(
            resolved.display(DisplayCut::Light).unwrap(),
            table.display(DisplayCut::Light).unwrap()
        ));
        assert!(std::ptr::addr_eq(
            resolved.table(),
            std::ptr::from_ref(&table)
        ));
    }
}
