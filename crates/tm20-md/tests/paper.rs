//! Fixtures encode. A sheet taller than 910 dots splits into several Graphics.

mod common;

use std::path::{Path, PathBuf};

use tm20::Raster;
use tm20::command::Command;
use tm20::encode::encode;
use tm20::graphics::max_height;
use tm20_md::{image_bytes, sheet};
use tm20_set::{Measure, compose, lower};

use common::table;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn markdown_files() -> Vec<PathBuf> {
    let dir = fixtures_dir();
    assert!(dir.is_dir(), "fixtures/ must exist");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| {
            e.unwrap_or_else(|err| panic!("{}: {err}", dir.display()))
                .path()
        })
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "fixtures/*.md must be nonempty");
    files
}

fn load_sheet(path: &Path) -> tm20_set::Sheet<'static> {
    let src = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let base = path.parent().unwrap();
    sheet(&src, Measure::TAPE, |dest| image_bytes(base, dest))
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn fixture_case(stem: &str) {
    let faces = table();
    let path = fixtures_dir().join(format!("{stem}.md"));
    let sheet = load_sheet(&path);
    let doc = tm20_set::lower(&sheet, &faces).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let bytes = encode(&doc).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(!bytes.is_empty(), "{}", path.display());
}

include!(concat!(env!("OUT_DIR"), "/paper_cases.rs"));

#[test]
fn fixture_inventory_is_complete() {
    let stems: Vec<_> = markdown_files()
        .iter()
        .map(|p| common::compare::stem_of(p))
        .collect();
    assert_eq!(
        stems,
        fixtures::STEMS,
        "rebuild to discover changed paper fixtures"
    );
}

#[test]
fn fga_lesson_splits_into_min_payloads_with_exact_bytes() {
    let faces = table();
    let path = fixtures_dir().join("14-fga.md");
    let sheet = load_sheet(&path);
    let page = compose(&sheet, &faces).unwrap();
    let cap = max_height(page.width());
    let n = page.height().div_ceil(u32::from(cap)) as usize;
    assert!(
        n > 1,
        "lesson should exceed one payload (H={} cap={cap})",
        page.height()
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
    assert!(bands.iter().all(|g| g.raster().height() <= u32::from(cap)));
    let rasters: Vec<Raster> = bands.iter().map(|g| g.raster().clone()).collect();
    let mut concat = Vec::new();
    for r in &rasters {
        concat.extend_from_slice(r.pixels());
    }
    assert_eq!(concat.as_slice(), page.pixels());
    encode(&doc).unwrap();
}
