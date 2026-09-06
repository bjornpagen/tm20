//! Explicit role-to-font profiles, loaded once per batch. No font discovery.
//! Portable assets: adobe-fonts/source-sans tag 3.052R; source-code-pro
//! commit d3f1a5962cde503f9409c21e58527611d4a19ef1 (2.042R).

use tm20_set::{Cut, DisplayCut, Face, FaceTable};

use crate::Result;

const HELVETICA: &str = "/System/Library/Fonts/Helvetica.ttc";
const MENLO: &str = "/System/Library/Fonts/Menlo.ttc";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, usage::ValueEnum)]
pub enum FontProfile {
    #[default]
    Portable,
    Macos,
}

impl FontProfile {
    pub fn load(self) -> Result<FaceTable> {
        match self {
            Self::Portable => portable_table(),
            Self::Macos => system_table(),
        }
    }
}

fn portable_table() -> Result<FaceTable> {
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

fn system_table() -> Result<FaceTable> {
    let mut table = FaceTable::new();
    table.absorb(read_font(HELVETICA)?);
    table.absorb(read_font(MENLO)?);
    for cut in Cut::ALL {
        table.text(cut)?;
    }
    for cut in DisplayCut::ALL {
        table.display(cut)?;
    }
    Ok(table)
}

fn read_font(path: &str) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| {
        format!("cannot load macos font {path}: {e}; use --fonts portable for embedded fonts")
            .into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_profile_fills_every_role() {
        let table = FontProfile::Portable.load().unwrap();
        for cut in Cut::ALL {
            table.text(cut).unwrap();
        }
        for cut in DisplayCut::ALL {
            table.display(cut).unwrap();
        }
    }
}
