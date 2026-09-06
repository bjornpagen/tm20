//! Explicit role-to-font profiles, loaded once per batch. No font discovery.

use tm20_set::{Cut, DisplayCut, FaceTable};

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
            Self::Portable => FaceTable::portable().map_err(Into::into),
            Self::Macos => system_table(),
        }
    }
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
