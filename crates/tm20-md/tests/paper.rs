//! Fixtures encode. A sheet taller than 910 dots splits into several Graphics.

use std::path::{Path, PathBuf};

use tm20::command::Command;
use tm20::encode::encode;
use tm20::graphics::max_height;
use tm20_md::{image_bytes, sheet};
use tm20_set::{Cut, Face, FaceTable, Measure, compose, lower, preview_png};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn table() -> FaceTable {
    let mut table = FaceTable::new();
    table.set_text(
        Cut::Roman,
        load("Neue Haas Grotesk Text Pro 55 Roman.otf").text(),
    );
    table.set_text(
        Cut::Italic,
        load("Neue Haas Grotesk Text Pro 56 Italic.otf").text(),
    );
    table.set_text(
        Cut::Bold,
        load("Neue Haas Grotesk Text Pro 75 Bold.otf").text(),
    );
    table.set_text(
        Cut::BoldItalic,
        load("Neue Haas Grotesk Text Pro 76 Bold Italic.otf").text(),
    );
    table.set_text(Cut::Mono, load("CommitMono-400-Regular.otf").text());
    table.set_display(
        Cut::Roman,
        load("Neue Haas Grotesk Display Pro 55 Roman.otf").display(),
    );
    table
}

fn load(file: &str) -> Face {
    let home = std::env::var_os("HOME").unwrap_or_default();
    let path = Path::new(&home).join("Library/Fonts").join(file);
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("{file} not in {}", path.display()));
    Face::from_bytes(bytes).unwrap_or_else(|_| panic!("{file} is CFF OpenType"))
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
