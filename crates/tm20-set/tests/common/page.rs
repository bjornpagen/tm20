//! Raster observation. Public [`compose`] returns [`Raster`].
#![allow(dead_code)]

use tm20::Raster;
use tm20_set::{FaceTable, Sheet, compose};

pub fn compose_raster(sheet: &Sheet<'_>, faces: &FaceTable) -> Raster {
    compose(sheet, faces).unwrap_or_else(|e| panic!("compose: {e}"))
}

pub fn first_ink_after(r: &Raster, from: u32) -> u32 {
    for y in from..r.height() {
        if row_has_ink(r, y) {
            return y;
        }
    }
    panic!("no ink after {from}")
}

pub fn full_width_row(r: &Raster, y: u32) -> bool {
    (0..r.width()).all(|x| r.pixel(x, y) == Some(true))
}

pub fn packed_ink(r: &Raster, y: u32, x: u16) -> bool {
    r.pixel(x, y) == Some(true)
}

pub fn row_has_ink(r: &Raster, y: u32) -> bool {
    (0..r.width()).any(|x| packed_ink(r, y, x))
}

pub fn leftmost_ink(r: &Raster) -> u16 {
    for x in 0..r.width() {
        for y in 0..r.height() {
            if packed_ink(r, y, x) {
                return x;
            }
        }
    }
    panic!("no ink")
}

/// Inclusive ink bounding box `(x0, x1, y0, y1)`.
pub fn ink_bbox(r: &Raster) -> (u16, u16, u32, u32) {
    let mut x0 = r.width();
    let mut x1 = 0u16;
    let mut y0 = r.height();
    let mut y1 = 0u32;
    let mut any = false;
    for y in 0..r.height() {
        for x in 0..r.width() {
            if packed_ink(r, y, x) {
                any = true;
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    assert!(any, "no ink");
    (x0, x1, y0, y1)
}

pub fn rightmost_in(r: &Raster, y0: u32, y1: u32) -> u16 {
    let mut x1 = 0u16;
    for y in y0..y1.min(r.height()) {
        for x in (0..r.width()).rev() {
            if packed_ink(r, y, x) {
                x1 = x1.max(x);
                break;
            }
        }
    }
    x1
}

pub fn ink_runs(r: &Raster, y0: u32, y1: u32) -> Vec<(u16, u16)> {
    let mut ink = vec![false; r.width() as usize];
    for y in y0..y1.min(r.height()) {
        for x in 0..r.width() {
            if packed_ink(r, y, x) {
                ink[x as usize] = true;
            }
        }
    }
    let mut runs = Vec::new();
    let mut x = 0u16;
    while x < r.width() {
        if !ink[x as usize] {
            x += 1;
            continue;
        }
        let start = x;
        while x < r.width() && ink[x as usize] {
            x += 1;
        }
        runs.push((start, x));
    }
    runs
}

pub fn merge_runs(runs: Vec<(u16, u16)>) -> Vec<(u16, u16)> {
    merge_runs_at(runs, 3)
}

pub fn merge_runs_at(runs: Vec<(u16, u16)>, join: u16) -> Vec<(u16, u16)> {
    let mut merged: Vec<(u16, u16)> = Vec::new();
    for r in runs {
        if let Some(last) = merged.last_mut()
            && r.0.saturating_sub(last.1) <= join
        {
            last.1 = r.1;
        } else {
            merged.push(r);
        }
    }
    merged
}

pub fn ink_bands(r: &Raster) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut y = 0u32;
    let h = r.height();
    while y < h {
        if !row_has_ink(r, y) {
            y += 1;
            continue;
        }
        let y0 = y;
        while y < h && row_has_ink(r, y) {
            y += 1;
        }
        out.push((y0, y));
    }
    out
}

pub fn ink_count(r: &Raster) -> usize {
    let mut n = 0;
    for y in 0..r.height() {
        for x in 0..r.width() {
            if packed_ink(r, y, x) {
                n += 1;
            }
        }
    }
    n
}

/// Concatenate packed band bodies and compare to the page. Heights alone are not enough.
pub fn concat_equals_page(page: &Raster, bands: &[&Raster]) {
    let mut concat = Vec::new();
    let mut rows = 0u32;
    for band in bands {
        assert_eq!(band.width(), page.width(), "band width");
        assert!(band.height() > 0, "band must be nonempty");
        concat.extend_from_slice(band.pixels());
        rows = rows.checked_add(band.height()).expect("band height");
    }
    assert_eq!(rows, page.height(), "band rows must partition the page");
    assert_eq!(
        concat.as_slice(),
        page.pixels(),
        "concat(band.pixels) must equal page.pixels"
    );
}
