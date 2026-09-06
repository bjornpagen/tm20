//! Embedded, explicitly assigned fonts; no system lookup or fallback.
//! Source Sans 3 3.052R and Source Code Pro 2.042R, unmodified SIL OFL assets.

use crate::{Cut, DisplayCut, Face, FaceTable, Result};

/// Notices for the embedded Source Sans 3 and Source Code Pro fonts.
pub const FONT_LICENSES: &str = concat!(
    "Source Sans 3 (3.052R)\n",
    include_str!("../fonts/SourceSans3-LICENSE.txt"),
    "\nSource Code Pro (2.042R)\n",
    include_str!("../fonts/SourceCodePro-LICENSE.txt"),
);

impl FaceTable {
    /// Load every text and display role from embedded portable fonts.
    /// Reuse the returned table across documents; no files or network are read.
    /// Available with the default `portable-fonts` feature.
    ///
    /// ```
    /// let faces = tm20_set::FaceTable::portable()?;
    /// for cut in tm20_set::Cut::ALL {
    ///     faces.text(cut)?;
    /// }
    /// # Ok::<(), tm20_set::Error>(())
    /// ```
    pub fn portable() -> Result<FaceTable> {
        let mut table = FaceTable::new();
        let roman = Face::from_bytes_index(
            include_bytes!("../fonts/SourceSans3-Regular.ttf").as_slice(),
            0,
        )?;
        table.set_display(DisplayCut::Roman, roman.clone().display());
        table.set_text(Cut::Roman, roman.text());
        for (cut, bytes) in [
            (
                Cut::Italic,
                include_bytes!("../fonts/SourceSans3-It.ttf").as_slice(),
            ),
            (
                Cut::Bold,
                include_bytes!("../fonts/SourceSans3-Bold.ttf").as_slice(),
            ),
            (
                Cut::BoldItalic,
                include_bytes!("../fonts/SourceSans3-BoldIt.ttf").as_slice(),
            ),
            (
                Cut::Mono,
                include_bytes!("../fonts/SourceCodePro-Regular.ttf").as_slice(),
            ),
        ] {
            table.set_text(cut, Face::from_bytes_index(bytes, 0)?.text());
        }
        table.set_display(
            DisplayCut::Light,
            Face::from_bytes_index(
                include_bytes!("../fonts/SourceSans3-Light.ttf").as_slice(),
                0,
            )?
            .display(),
        );
        Ok(table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_profile_fills_every_role() {
        let table = FaceTable::portable().unwrap();
        for cut in Cut::ALL {
            table.text(cut).unwrap();
        }
        for cut in DisplayCut::ALL {
            table.display(cut).unwrap();
        }
        assert!(FONT_LICENSES.contains("SIL OPEN FONT LICENSE"));
    }
}
