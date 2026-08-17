//! Faces this machine brings. The library only parses bytes. CFF OpenType only.

use std::io;
use std::path::{Path, PathBuf};

use tm20_set::{Cut, DisplayFace, Face, FaceTable, TextFace};

use crate::Result;

pub fn system_table() -> Result<FaceTable> {
    nhg_table()
}

pub fn nhg_table() -> Result<FaceTable> {
    let mut table = FaceTable::new();
    table.set_text(
        Cut::Roman,
        nhg_text("Neue Haas Grotesk Text Pro 55 Roman.otf")?,
    );
    table.set_text(
        Cut::Italic,
        nhg_text("Neue Haas Grotesk Text Pro 56 Italic.otf")?,
    );
    table.set_text(
        Cut::Medium,
        nhg_text("Neue Haas Grotesk Text Pro 65 Medium.otf")?,
    );
    table.set_text(
        Cut::Bold,
        nhg_text("Neue Haas Grotesk Text Pro 75 Bold.otf")?,
    );
    table.set_text(
        Cut::BoldItalic,
        nhg_text("Neue Haas Grotesk Text Pro 76 Bold Italic.otf")
            .or_else(|_| nhg_text("Neue Haas Grotesk Text Pro 75 Bold.otf"))?,
    );
    table.set_display(
        Cut::Roman,
        nhg_display("Neue Haas Grotesk Display Pro 55 Roman.otf")?,
    );
    if let Ok(light) = nhg_display("Neue Haas Grotesk Display Pro 45 Light.otf") {
        table.set_display(Cut::Light, light);
    }
    table.set_text(Cut::Mono, commit_mono()?);
    Ok(table)
}

pub fn fonts_dir() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    Path::new(&home).join("Library/Fonts")
}

fn commit_mono() -> Result<TextFace> {
    Ok(from_file("CommitMono-400-Regular.otf")?.text())
}

fn nhg_text(file: &str) -> Result<TextFace> {
    Ok(from_file(file)?.text())
}

fn nhg_display(file: &str) -> Result<DisplayFace> {
    Ok(from_file(file)?.display())
}

fn from_file(file: &str) -> Result<Face> {
    let path = fonts_dir().join(file);
    let bytes = std::fs::read(&path).map_err(|_| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} not in {}", file, fonts_dir().display()),
        )
    })?;
    Ok(Face::from_bytes(bytes)?)
}
