//! Faces this machine brings. The library only parses bytes.

use std::io;
use std::sync::Arc;

use tm20_set::{Cut, DisplayCut, Face, FaceTable};

use crate::Result;

const HELVETICA: &str = "/System/Library/Fonts/Helvetica.ttc";
const MENLO: &str = "/System/Library/Fonts/Menlo.ttc";

pub fn system_table() -> Result<FaceTable> {
    let bytes: Arc<[u8]> = std::fs::read(HELVETICA)
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{HELVETICA} not on this machine"),
            )
        })?
        .into();
    let mut table = FaceTable::new();
    for index in 0.. {
        let Ok(face) = Face::from_bytes_index(bytes.clone(), index) else {
            break;
        };
        let Some(name) = face.postscript_name() else {
            continue;
        };
        match name.as_str() {
            "Helvetica" => {
                table.set_text(
                    Cut::Roman,
                    Face::from_bytes_index(bytes.clone(), index)?.text(),
                );
                table.set_display(DisplayCut::Roman, face.display());
            }
            "Helvetica-Bold" => table.set_text(Cut::Bold, face.text()),
            "Helvetica-Oblique" => table.set_text(Cut::Italic, face.text()),
            "Helvetica-BoldOblique" => table.set_text(Cut::BoldItalic, face.text()),
            "Helvetica-Light" => table.set_display(DisplayCut::Light, face.display()),
            _ => {}
        }
    }
    let menlo: Arc<[u8]> = std::fs::read(MENLO)
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{MENLO} not on this machine"),
            )
        })?
        .into();
    for index in 0.. {
        let Ok(face) = Face::from_bytes_index(menlo.clone(), index) else {
            break;
        };
        if face.postscript_name().as_deref() == Some("Menlo-Regular") {
            table.set_text(Cut::Mono, face.text());
            break;
        }
    }
    // Completeness is the compose boundary's parse: lower() builds a Kit
    // and reports any missing cut once, there.
    Ok(table)
}
