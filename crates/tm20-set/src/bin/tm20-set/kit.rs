//! Faces this machine brings. The library only parses bytes.

use std::io;
use std::path::{Path, PathBuf};

use font_kit::family_name::FamilyName;
use font_kit::handle::Handle;
use font_kit::properties::{Properties, Style as KitStyle, Weight as KitWeight};
use font_kit::source::SystemSource;
use tm20_set::{Cut, DisplayFace, Face, FaceTable, TextFace};

use crate::Result;

pub fn system_table() -> Result<FaceTable> {
    let mut table = FaceTable::new();
    table.set_text(
        Cut::Roman,
        system_text(KitWeight::NORMAL, KitStyle::Normal)?,
    );
    table.set_text(
        Cut::Italic,
        system_text(KitWeight::NORMAL, KitStyle::Italic)?,
    );
    table.set_text(Cut::Bold, system_text(KitWeight::BOLD, KitStyle::Normal)?);
    table.set_display(
        Cut::Roman,
        system_display(KitWeight::NORMAL, KitStyle::Normal)?,
    );
    Ok(table)
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
    table.set_display(
        Cut::Roman,
        nhg_display("Neue Haas Grotesk Display Pro 55 Roman.otf")?,
    );
    if let Ok(light) = nhg_display("Neue Haas Grotesk Display Pro 45 Light.otf") {
        table.set_display(Cut::Light, light);
    }
    Ok(table)
}

pub fn fonts_dir() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    Path::new(&home).join("Library/Fonts")
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

fn system_text(weight: KitWeight, style: KitStyle) -> Result<TextFace> {
    Ok(from_handle(sans_handle(weight, style)?)?.text())
}

fn system_display(weight: KitWeight, style: KitStyle) -> Result<DisplayFace> {
    Ok(from_handle(sans_handle(weight, style)?)?.display())
}

fn sans_handle(weight: KitWeight, style: KitStyle) -> io::Result<Handle> {
    SystemSource::new()
        .select_best_match(
            &[FamilyName::SansSerif],
            Properties::new().weight(weight).style(style),
        )
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "system sans-serif typeface not found",
            )
        })
}

fn from_handle(handle: Handle) -> Result<Face> {
    match handle {
        Handle::Path { path, font_index } => {
            Ok(Face::from_bytes_index(std::fs::read(path)?, font_index)?)
        }
        Handle::Memory { bytes, font_index } => {
            Ok(Face::from_bytes_index((*bytes).clone(), font_index)?)
        }
    }
}
