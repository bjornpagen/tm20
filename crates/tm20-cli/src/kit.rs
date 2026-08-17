//! Faces this machine brings. The library only parses bytes.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tm20_set::{Cut, Face, FaceTable};

use crate::Result;

const HELVETICA: &str = "/System/Library/Fonts/Helvetica.ttc";

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
                table.set_display(Cut::Roman, face.display());
            }
            "Helvetica-Bold" => table.set_text(Cut::Bold, face.text()),
            "Helvetica-Oblique" => table.set_text(Cut::Italic, face.text()),
            "Helvetica-BoldOblique" => table.set_text(Cut::BoldItalic, face.text()),
            "Helvetica-Light" => table.set_display(Cut::Light, face.display()),
            _ => {}
        }
    }
    table.set_text(Cut::Mono, commit_mono()?);
    table.text(Cut::Roman)?;
    table.text(Cut::Italic)?;
    table.text(Cut::Bold)?;
    table.text(Cut::BoldItalic)?;
    table.display(Cut::Roman)?;
    Ok(table)
}

pub fn fonts_dir() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    Path::new(&home).join("Library/Fonts")
}

fn commit_mono() -> Result<tm20_set::TextFace> {
    let file = "CommitMono-400-Regular.otf";
    let path = fonts_dir().join(file);
    let bytes = std::fs::read(&path).map_err(|_| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} not in {}", file, fonts_dir().display()),
        )
    })?;
    Ok(Face::from_bytes(bytes)?.text())
}
