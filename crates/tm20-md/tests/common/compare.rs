//! Golden comparison. Bless writes only when TM20_SNAP=bless; lock is never rewritten.

use std::path::{Path, PathBuf};

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use tm20::Raster;
use tm20_md::{image_bytes, sheet};
use tm20_set::{Measure, compose};

use super::faces::table;
use super::tests_dir;

const PAPER: [u8; 3] = [255, 255, 255];
const MATCH: [u8; 3] = [0x66, 0x66, 0x66];
const ADDED: [u8; 3] = [0xDD, 0x00, 0x00];
const REMOVED: [u8; 3] = [0x00, 0x66, 0xCC];

pub fn bless() -> bool {
    matches!(std::env::var("TM20_SNAP").as_deref(), Ok("bless"))
}

/// Temporarily list reviewed stems when deliberately updating goldens; clear after comparison.
pub fn approved_stems() -> &'static [&'static str] {
    &[]
}

pub fn bless_allowed(stem: &str) -> bool {
    bless() && approved_stems().contains(&stem)
}

pub fn goldens_dir() -> PathBuf {
    tests_dir().join("goldens")
}

pub fn artifact_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/snap")
}

pub fn stem_of(path: &Path) -> String {
    path.file_stem()
        .expect("stem")
        .to_string_lossy()
        .into_owned()
}

pub fn render(path: &Path) -> Raster {
    try_render(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

pub fn try_render(path: &Path) -> Result<Raster, tm20_md::Error> {
    // Parse-only rejections should not load any fonts.
    let sheet = load_sheet(path)?;
    compose(&sheet, &table()).map_err(Into::into)
}

pub fn try_render_with(path: &Path, faces: &tm20_set::FaceTable) -> Result<Raster, tm20_md::Error> {
    compose(&load_sheet(path)?, faces).map_err(Into::into)
}

fn load_sheet(path: &Path) -> Result<tm20_set::Sheet<'static>, tm20_md::Error> {
    let src = std::fs::read_to_string(path).map_err(|e| tm20_md::Error::Resource {
        destination: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let base = path.parent().expect("parent");
    sheet(&src, Measure::TAPE, |d| image_bytes(base, d))
}

pub fn encode_golden(r: &Raster) -> Vec<u8> {
    let w = u32::from(r.width());
    let h = r.height().max(1);
    let inverted: Vec<u8> = r.pixels().iter().map(|b| !b).collect();
    let mut out = Vec::new();
    if PngEncoder::new(&mut out)
        .write_image(&inverted, w, h, ExtendedColorType::L1)
        .is_ok()
    {
        return out;
    }
    out.clear();
    let luma = unpack_luma(r);
    PngEncoder::new(&mut out)
        .write_image(&luma, w, h, ExtendedColorType::L8)
        .expect("encode L8 golden");
    out
}

fn unpack_luma(r: &Raster) -> Vec<u8> {
    let w = usize::from(r.width());
    let h = usize::try_from(r.height()).expect("height");
    let mut luma = vec![0xFFu8; w * h];
    for y in 0..r.height() {
        for x in 0..r.width() {
            if r.pixel(x, y) == Some(true) {
                luma[usize::try_from(y).unwrap() * w + usize::from(x)] = 0x00;
            }
        }
    }
    luma
}

fn ink_from_raster(r: &Raster) -> (u16, u32, Vec<bool>) {
    let w = r.width();
    let h = r.height();
    let mut ink = vec![false; usize::from(w) * usize::try_from(h).expect("h")];
    for y in 0..h {
        for x in 0..w {
            ink[usize::try_from(y).unwrap() * usize::from(w) + usize::from(x)] =
                r.pixel(x, y) == Some(true);
        }
    }
    (w, h, ink)
}

fn ink_from_png(bytes: &[u8]) -> (u16, u32, Vec<bool>) {
    let img = image::load_from_memory(bytes)
        .expect("decode golden")
        .to_luma8();
    let w = u16::try_from(img.width()).expect("golden width");
    let h = img.height();
    let ink = img.pixels().map(|p| p.0[0] < 128).collect();
    (w, h, ink)
}

pub fn raster_from_png(bytes: &[u8]) -> Raster {
    let img = image::load_from_memory(bytes)
        .expect("decode golden")
        .to_luma8();
    let width = u16::try_from(img.width()).expect("golden width");
    let stride = usize::from(width).div_ceil(8);
    let mut packed = vec![0; stride * img.height() as usize];
    for (y, row) in img.as_raw().chunks_exact(usize::from(width)).enumerate() {
        for (x, &pixel) in row.iter().enumerate() {
            if pixel < 128 {
                packed[y * stride + x / 8] |= 0x80 >> (x % 8);
            }
        }
    }
    Raster::from_packed(width, img.height(), packed).expect("golden raster")
}

pub struct Mismatch {
    pub stem: String,
    pub kind: Kind,
    pub bbox: Option<(u16, u32, u16, u32)>,
    pub height_delta: i64,
}

pub enum Kind {
    Missing,
    Dims { want: (u16, u32), got: (u16, u32) },
    Pixels(usize),
}

pub fn compare_raster(path: &Path, fresh: &Raster) -> Result<(), Mismatch> {
    let stem = stem_of(path);
    let golden_path = goldens_dir().join(format!("{stem}.png"));
    let Ok(bytes) = std::fs::read(&golden_path) else {
        return Err(Mismatch {
            stem,
            kind: Kind::Missing,
            bbox: None,
            height_delta: i64::from(fresh.height()),
        });
    };
    let golden = raster_from_png(&bytes);
    compare_images(stem, &golden, fresh)
}

pub fn compare_images(stem: String, golden: &Raster, fresh: &Raster) -> Result<(), Mismatch> {
    let (gw, gh) = (golden.width(), golden.height());
    let (fw, fh) = (fresh.width(), fresh.height());
    if (gw, gh) != (fw, fh) {
        return Err(Mismatch {
            stem,
            kind: Kind::Dims {
                want: (gw, gh),
                got: (fw, fh),
            },
            bbox: None,
            height_delta: i64::from(fh) - i64::from(gh),
        });
    }
    if golden.pixels() == fresh.pixels() {
        return Ok(());
    }
    let mut n = 0usize;
    let mut min_x = u16::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0u16;
    let mut max_y = 0u32;
    for y in 0..fh {
        for x in 0..fw {
            if golden.pixel(x, y) == fresh.pixel(x, y) {
                continue;
            }
            n += 1;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    if n == 0 {
        return Ok(());
    }
    Err(Mismatch {
        stem,
        kind: Kind::Pixels(n),
        bbox: Some((min_x, min_y, max_x, max_y)),
        height_delta: 0,
    })
}

pub fn write_golden(path: &Path, raster: &Raster) {
    let stem = stem_of(path);
    let dir = goldens_dir();
    std::fs::create_dir_all(&dir).expect("goldens dir");
    std::fs::write(dir.join(format!("{stem}.png")), encode_golden(raster)).expect("write golden");
}

pub fn write_triplet(mismatch: &Mismatch, fresh: &Raster) {
    let dir = artifact_dir();
    std::fs::create_dir_all(&dir).expect("target/snap");
    let stem = &mismatch.stem;
    std::fs::write(dir.join(format!("{stem}.actual.png")), encode_golden(fresh))
        .expect("actual.png");
    let golden_path = goldens_dir().join(format!("{stem}.png"));
    let Ok(golden_bytes) = std::fs::read(&golden_path) else {
        return;
    };
    std::fs::write(dir.join(format!("{stem}.expected.png")), &golden_bytes).expect("expected.png");
    let (gw, gh, gink) = ink_from_png(&golden_bytes);
    let (fw, fh, fink) = ink_from_raster(fresh);
    let dw = gw.max(fw);
    let dh = gh.max(fh);
    let mut rgb = vec![0u8; usize::from(dw) * usize::try_from(dh).unwrap() * 3];
    for y in 0..dh {
        for x in 0..dw {
            let exp = sample(&gink, gw, gh, x, y);
            let act = sample(&fink, fw, fh, x, y);
            let c = match (exp, act) {
                (false, false) => PAPER,
                (true, true) => MATCH,
                (false, true) => ADDED,
                (true, false) => REMOVED,
            };
            let i = (usize::try_from(y).unwrap() * usize::from(dw) + usize::from(x)) * 3;
            rgb[i..i + 3].copy_from_slice(&c);
        }
    }
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(&rgb, u32::from(dw), dh, ExtendedColorType::Rgb8)
        .expect("diff.png");
    std::fs::write(dir.join(format!("{stem}.diff.png")), out).expect("write diff");
}

fn sample(ink: &[bool], w: u16, h: u32, x: u16, y: u32) -> bool {
    if x >= w || y >= h {
        return false;
    }
    ink[usize::try_from(y).unwrap() * usize::from(w) + usize::from(x)]
}

pub fn format_row(m: &Mismatch) -> String {
    let kind = match m.kind {
        Kind::Missing => "missing golden".into(),
        Kind::Dims { want, got } => {
            format!("dims changed {}x{} → {}x{}", want.0, want.1, got.0, got.1)
        }
        Kind::Pixels(n) => format!("{n} pixels differ"),
    };
    let bbox = m
        .bbox
        .map(|(x0, y0, x1, y1)| format!(" ({x0},{y0})-({x1},{y1})"))
        .unwrap_or_default();
    format!("  {:<32} {kind}{bbox}  Δh={}", m.stem, m.height_delta)
}
