//! Fixtures encode. A sheet taller than 910 dots splits into several Graphics.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tm20::command::Command;
use tm20::encode::encode;
use tm20::graphics::max_height;
use tm20_md::{image_bytes, sheet};
use tm20_set::{Cut, Face, FaceTable, Measure, compose, lower, preview_png};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn table() -> FaceTable {
    let bytes: Arc<[u8]> = std::fs::read("/System/Library/Fonts/Helvetica.ttc")
        .expect("Helvetica.ttc")
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
                    Face::from_bytes_index(bytes.clone(), index)
                        .expect("Helvetica Regular")
                        .text(),
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
    table.set_text(Cut::Mono, load_mono().text());
    table
}

fn load_mono() -> Face {
    let bytes: Arc<[u8]> = std::fs::read("/System/Library/Fonts/Menlo.ttc")
        .expect("Menlo.ttc")
        .into();
    for index in 0.. {
        let Ok(face) = Face::from_bytes_index(bytes.clone(), index) else {
            break;
        };
        if face.postscript_name().as_deref() == Some("Menlo-Regular") {
            return face;
        }
    }
    panic!("Menlo-Regular not in Menlo.ttc")
}

fn markdown_files() -> Vec<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(fixtures_dir())
        .unwrap()
        .filter_map(|e| {
            let p = e.ok()?.path();
            if p.extension().is_some_and(|e| e == "md") {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "fixtures/*.md");
    files
}

fn load_sheet(path: &Path) -> tm20_set::Sheet<'static> {
    let src = std::fs::read_to_string(path).unwrap();
    let base = path.parent().unwrap();
    sheet(&src, Measure::TAPE, |dest| image_bytes(base, dest))
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn fixtures_encode() {
    let faces = table();
    for path in markdown_files() {
        let sheet = load_sheet(&path);
        let doc =
            tm20_set::lower(&sheet, &faces).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let bytes = encode(&doc).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(!bytes.is_empty(), "{}", path.display());
    }
}

#[test]
fn fixtures_preview_png() {
    let faces = table();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tm20-preview");
    std::fs::create_dir_all(&dir).unwrap();
    for path in markdown_files() {
        let sheet = load_sheet(&path);
        let g = compose(&sheet, &faces).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let name = path.file_stem().unwrap().to_string_lossy();
        std::fs::write(dir.join(format!("{name}.png")), preview_png(&g).unwrap()).unwrap();
    }
}

#[test]
fn fga_lesson_splits_into_min_payloads() {
    let faces = table();
    let path = fixtures_dir().join("14-fga.md");
    let sheet = load_sheet(&path);
    let full = compose(&sheet, &faces).unwrap();
    let cap = max_height(full.width_dots);
    let n = u32::from(full.height_dots).div_ceil(u32::from(cap)) as usize;
    assert!(
        n > 1,
        "lesson should exceed one payload (H={} cap={cap})",
        full.height_dots
    );
    let doc = lower(&sheet, &faces).unwrap();
    let bands: Vec<_> = doc
        .commands()
        .iter()
        .filter_map(|c| match c {
            Command::Graphics(g) => Some(g),
            _ => None,
        })
        .collect();
    assert_eq!(bands.len(), n);
    assert!(bands.iter().all(|g| g.height_dots <= cap));
    let sum: u32 = bands.iter().map(|g| u32::from(g.height_dots)).sum();
    assert_eq!(sum, u32::from(full.height_dots));
    encode(&doc).unwrap();
}
