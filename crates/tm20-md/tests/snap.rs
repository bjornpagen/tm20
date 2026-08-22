//! Pixel-exact visual regression for the markdown → raster pipeline.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use tm20::graphics::{Graphics, is_black, width_bytes};
use tm20_md::{image_bytes, sheet};
use tm20_set::{Measure, compose};

use common::table;

const PAPER: [u8; 3] = [255, 255, 255];
const MATCH: [u8; 3] = [0x66, 0x66, 0x66];
const ADDED: [u8; 3] = [0xDD, 0x00, 0x00];
const REMOVED: [u8; 3] = [0x00, 0x66, 0xCC];

fn tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

fn corpus_dir() -> PathBuf {
    tests_dir().join("corpus")
}

fn goldens_dir() -> PathBuf {
    tests_dir().join("goldens")
}

fn reject_dir() -> PathBuf {
    tests_dir().join("reject")
}

fn lock_path() -> PathBuf {
    tests_dir().join("faces.lock")
}

fn artifact_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/snap")
}

fn bless() -> bool {
    matches!(std::env::var("TM20_SNAP").as_deref(), Ok("bless"))
}

fn check_or_bless_lock() -> Option<String> {
    let want = common::lock_text();
    let have = std::fs::read_to_string(lock_path()).ok();
    if have.as_deref() == Some(want.as_str()) {
        return None;
    }
    if bless() {
        std::fs::write(lock_path(), &want).expect("write faces.lock");
        return Some("faces.lock".into());
    }
    panic!("font drift — inspect and re-bless");
}

fn corpus_files() -> Option<Vec<PathBuf>> {
    let dir = corpus_dir();
    if !dir.is_dir() {
        return None;
    }
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|e| {
            let p = e.ok()?.path();
            p.extension().is_some_and(|e| e == "md").then_some(p)
        })
        .collect();
    files.sort();
    Some(files)
}

fn stem_of(path: &Path) -> String {
    path.file_stem()
        .expect("stem")
        .to_string_lossy()
        .into_owned()
}

fn render(path: &Path) -> Graphics {
    let src = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let base = path.parent().expect("parent");
    let sheet = sheet(&src, Measure::TAPE, |d| image_bytes(base, d))
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    compose(&sheet, &table()).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn encode_golden(g: &Graphics) -> Vec<u8> {
    let w = u32::from(g.width_dots);
    let h = u32::from(g.height_dots).max(1);
    let inverted: Vec<u8> = g.pixels.iter().map(|b| !b).collect();
    let mut out = Vec::new();
    if PngEncoder::new(&mut out)
        .write_image(&inverted, w, h, ExtendedColorType::L1)
        .is_ok()
    {
        return out;
    }
    out.clear();
    let luma = unpack_luma(g);
    PngEncoder::new(&mut out)
        .write_image(&luma, w, h, ExtendedColorType::L8)
        .expect("encode L8 golden");
    out
}

fn unpack_luma(g: &Graphics) -> Vec<u8> {
    let w = usize::from(g.width_dots);
    let h = usize::from(g.height_dots);
    let stride = width_bytes(g.width_dots);
    let mut luma = vec![0xFFu8; w * h];
    for y in 0..h {
        for x in 0..w {
            if is_black(&g.pixels, stride, x, y) {
                luma[y * w + x] = 0x00;
            }
        }
    }
    luma
}

fn ink_from_graphics(g: &Graphics) -> (u16, u16, Vec<bool>) {
    let w = g.width_dots;
    let h = g.height_dots;
    let stride = width_bytes(w);
    let mut ink = vec![false; usize::from(w) * usize::from(h)];
    for y in 0..usize::from(h) {
        for x in 0..usize::from(w) {
            ink[y * usize::from(w) + x] = is_black(&g.pixels, stride, x, y);
        }
    }
    (w, h, ink)
}

fn ink_from_png(bytes: &[u8]) -> (u16, u16, Vec<bool>) {
    let img = image::load_from_memory(bytes)
        .expect("decode golden")
        .to_luma8();
    let w = u16::try_from(img.width()).expect("golden width");
    let h = u16::try_from(img.height()).expect("golden height");
    let ink = img.pixels().map(|p| p.0[0] < 128).collect();
    (w, h, ink)
}

struct Mismatch {
    stem: String,
    kind: Kind,
    bbox: Option<(u16, u16, u16, u16)>,
    height_delta: i32,
}

enum Kind {
    Missing,
    Dims { want: (u16, u16), got: (u16, u16) },
    Pixels(usize),
}

fn compare_one(path: &Path) -> Result<(), Mismatch> {
    let stem = stem_of(path);
    let fresh = render(path);
    let golden_path = goldens_dir().join(format!("{stem}.png"));
    let Ok(bytes) = std::fs::read(&golden_path) else {
        return Err(Mismatch {
            stem,
            kind: Kind::Missing,
            bbox: None,
            height_delta: i32::from(fresh.height_dots),
        });
    };
    let (gw, gh, gink) = ink_from_png(&bytes);
    let (fw, fh, fink) = ink_from_graphics(&fresh);
    if (gw, gh) != (fw, fh) {
        return Err(Mismatch {
            stem,
            kind: Kind::Dims {
                want: (gw, gh),
                got: (fw, fh),
            },
            bbox: None,
            height_delta: i32::from(fh) - i32::from(gh),
        });
    }
    let mut n = 0usize;
    let mut min_x = u16::MAX;
    let mut min_y = u16::MAX;
    let mut max_x = 0u16;
    let mut max_y = 0u16;
    for y in 0..fh {
        for x in 0..fw {
            if gink[usize::from(y) * usize::from(fw) + usize::from(x)]
                == fink[usize::from(y) * usize::from(fw) + usize::from(x)]
            {
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

fn write_golden(path: &Path) {
    let stem = stem_of(path);
    let g = render(path);
    let dir = goldens_dir();
    std::fs::create_dir_all(&dir).expect("goldens dir");
    std::fs::write(dir.join(format!("{stem}.png")), encode_golden(&g)).expect("write golden");
}

fn write_triplet(path: &Path, mismatch: &Mismatch) {
    let dir = artifact_dir();
    std::fs::create_dir_all(&dir).expect("target/snap");
    let stem = &mismatch.stem;
    let fresh = render(path);
    std::fs::write(
        dir.join(format!("{stem}.actual.png")),
        encode_golden(&fresh),
    )
    .expect("actual.png");
    let golden_path = goldens_dir().join(format!("{stem}.png"));
    let Ok(golden_bytes) = std::fs::read(&golden_path) else {
        return;
    };
    std::fs::write(dir.join(format!("{stem}.expected.png")), &golden_bytes).expect("expected.png");
    let (gw, gh, gink) = ink_from_png(&golden_bytes);
    let (fw, fh, fink) = ink_from_graphics(&fresh);
    let dw = gw.max(fw);
    let dh = gh.max(fh);
    let mut rgb = vec![0u8; usize::from(dw) * usize::from(dh) * 3];
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
            let i = (usize::from(y) * usize::from(dw) + usize::from(x)) * 3;
            rgb[i..i + 3].copy_from_slice(&c);
        }
    }
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(&rgb, u32::from(dw), u32::from(dh), ExtendedColorType::Rgb8)
        .expect("diff.png");
    std::fs::write(dir.join(format!("{stem}.diff.png")), out).expect("write diff");
}

fn sample(ink: &[bool], w: u16, h: u16, x: u16, y: u16) -> bool {
    if x >= w || y >= h {
        return false;
    }
    ink[usize::from(y) * usize::from(w) + usize::from(x)]
}

fn format_row(m: &Mismatch) -> String {
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

#[test]
fn faces_are_locked() {
    check_or_bless_lock();
}

#[test]
fn compose_is_deterministic() {
    check_or_bless_lock();
    let Some(files) = corpus_files() else {
        eprintln!("tests/corpus absent; skip compose_is_deterministic");
        return;
    };
    let Some(path) = files.first() else {
        eprintln!("tests/corpus empty; skip compose_is_deterministic");
        return;
    };
    let a = render(path);
    let b = render(path);
    assert_eq!(
        (a.width_dots, a.height_dots, a.pixels),
        (b.width_dots, b.height_dots, b.pixels),
        "compose of {} was not deterministic",
        path.display()
    );
}

#[test]
fn rejects_reject() {
    check_or_bless_lock();
    let dir = reject_dir();
    if !dir.is_dir() {
        eprintln!("tests/reject absent; skip rejects_reject");
        return;
    }
    let expect_path = dir.join("expect.txt");
    let expect_src = std::fs::read_to_string(&expect_path).unwrap_or_default();
    let mut expect: BTreeMap<String, String> = BTreeMap::new();
    for (i, line) in expect_src.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((stem, msg)) = line.split_once('=') else {
            panic!("expect.txt:{}: want `stem = message`", i + 1);
        };
        expect.insert(stem.trim().to_string(), msg.trim().to_string());
    }
    let mut md_stems = BTreeSet::new();
    let mut fails = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| {
            let p = e.ok()?.path();
            p.extension().is_some_and(|e| e == "md").then_some(p)
        })
        .collect();
    files.sort();
    for path in files {
        let stem = stem_of(&path);
        md_stems.insert(stem.clone());
        let src = std::fs::read_to_string(&path).unwrap();
        let base = path.parent().expect("parent");
        match sheet(&src, Measure::TAPE, |d| image_bytes(base, d)) {
            Ok(_) => fails.push(format!("{stem}: parsed cleanly")),
            Err(e) => match expect.get(&stem) {
                Some(want) if e.to_string() == *want => {}
                Some(want) => fails.push(format!("{stem}: got `{e}`, want `{want}`")),
                None => fails.push(format!("{stem}: missing expect line")),
            },
        }
    }
    for stem in expect.keys() {
        if !md_stems.contains(stem) {
            fails.push(format!("{stem}: extra expect line"));
        }
    }
    assert!(fails.is_empty(), "reject mismatches:\n{}", fails.join("\n"));
}

#[test]
fn corpus_matches_goldens() {
    let mut wrote = Vec::new();
    if let Some(lock) = check_or_bless_lock() {
        wrote.push(lock);
    }
    let Some(files) = corpus_files() else {
        eprintln!("tests/corpus absent; skip corpus_matches_goldens");
        return;
    };
    let mut fails = Vec::new();
    for path in &files {
        match compare_one(path) {
            Ok(()) => {}
            Err(m) if bless() => {
                write_golden(path);
                wrote.push(format!("goldens/{}.png", m.stem));
            }
            Err(m) => {
                write_triplet(path, &m);
                fails.push(m);
            }
        }
    }
    if bless() {
        if wrote.is_empty() {
            eprintln!("wrote nothing");
        } else {
            for w in &wrote {
                eprintln!("wrote {w}");
            }
        }
        return;
    }
    assert!(
        fails.is_empty(),
        "snap mismatches:\n{}",
        fails.iter().map(format_row).collect::<Vec<_>>().join("\n")
    );
}

#[test]
#[ignore = "run once: cargo test -p tm20-md --test snap -- --ignored write_corpus_assets"]
fn write_corpus_assets() {
    let dir = corpus_dir().join("assets");
    std::fs::create_dir_all(&dir).expect("assets dir");
    write_sq60(&dir.join("sq60.png"));
    write_solid(&dir.join("w575.png"), 575, 24);
    write_solid(&dir.join("w576.png"), 576, 24);
    write_solid(&dir.join("w577.png"), 577, 24);
    write_solid(&dir.join("vline.png"), 1, 1200);
    write_solid(&dir.join("hline.png"), 576, 1);
    write_ramp(&dir.join("ramp.png"));
    write_alpha(&dir.join("alpha.png"));
    write_indexed(&dir.join("indexed.png"));
    write_gray(&dir.join("gray.png"));
    write_photo(&dir.join("photo.jpg"));
    std::fs::write(dir.join("garbage.png"), [0xA5u8; 64]).expect("garbage.png");
}

fn write_luma_png(path: &Path, w: u32, h: u32, luma: &[u8]) {
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(luma, w, h, ExtendedColorType::L8)
        .expect("luma png");
    std::fs::write(path, out).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

fn write_sq60(path: &Path) {
    let mut luma = vec![0xFFu8; 60 * 60];
    for y in 0..60 {
        for x in 0..60 {
            if !(4..56).contains(&x) || !(4..56).contains(&y) {
                luma[y * 60 + x] = 0x00;
            }
        }
    }
    write_luma_png(path, 60, 60, &luma);
}

fn write_solid(path: &Path, w: u32, h: u32) {
    write_luma_png(path, w, h, &vec![0x00; (w * h) as usize]);
}

fn write_ramp(path: &Path) {
    let mut luma = vec![0u8; 256 * 64];
    for y in 0..64 {
        for x in 0..256 {
            luma[y * 256 + x] = x as u8;
        }
    }
    write_luma_png(path, 256, 64, &luma);
}

fn write_gray(path: &Path) {
    let mut luma = vec![0u8; 64 * 64];
    for y in 0..64 {
        let v = (y * 4).min(255) as u8;
        for x in 0..64 {
            luma[y * 64 + x] = v;
        }
    }
    write_luma_png(path, 64, 64, &luma);
}

fn write_alpha(path: &Path) {
    let mut rgba = vec![0u8; 64 * 64 * 4];
    for y in 0..64i32 {
        for x in 0..64i32 {
            let dx = x - 32;
            let dy = y - 32;
            if dx * dx + dy * dy <= 24 * 24 {
                let i = (y * 64 + x) as usize * 4;
                rgba[i + 3] = 255;
            }
        }
    }
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(&rgba, 64, 64, ExtendedColorType::Rgba8)
        .expect("alpha png");
    std::fs::write(path, out).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

fn write_indexed(path: &Path) {
    let mut indices = vec![0u8; 64 * 64];
    for y in 0..64 {
        for x in 0..64 {
            indices[y * 64 + x] = u8::from(((x / 8) + (y / 8)) % 2 == 1);
        }
    }
    write_png_indexed(path, 64, 64, &[[0, 0, 0], [255, 255, 255]], &indices);
}

fn write_photo(path: &Path) {
    let mut rgb = vec![0u8; 128 * 96 * 3];
    for y in 0..96u32 {
        for x in 0..128u32 {
            let v = ((x + y) * 255 / 222) as u8;
            let i = (y * 128 + x) as usize * 3;
            rgb[i] = v;
            rgb[i + 1] = v;
            rgb[i + 2] = v;
        }
    }
    let mut out = Vec::new();
    let enc = JpegEncoder::new_with_quality(&mut out, 80);
    enc.write_image(&rgb, 128, 96, ExtendedColorType::Rgb8)
        .expect("photo.jpg");
    std::fs::write(path, out).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

fn write_png_indexed(path: &Path, width: u32, height: u32, palette: &[[u8; 3]], indices: &[u8]) {
    assert_eq!(indices.len(), (width * height) as usize);
    let mut raw = Vec::with_capacity(((1 + width) * height) as usize);
    for y in 0..height as usize {
        raw.push(0);
        raw.extend_from_slice(&indices[y * width as usize..(y + 1) * width as usize]);
    }
    let mut out = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 3, 0, 0, 0]);
    write_chunk(&mut out, *b"IHDR", &ihdr);
    let mut plte = Vec::new();
    for c in palette {
        plte.extend_from_slice(c);
    }
    write_chunk(&mut out, *b"PLTE", &plte);
    write_chunk(&mut out, *b"IDAT", &zlib_store(&raw));
    write_chunk(&mut out, *b"IEND", &[]);
    std::fs::write(path, out).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

fn write_chunk(out: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    out.extend_from_slice(&u32::try_from(data.len()).expect("chunk").to_be_bytes());
    out.extend_from_slice(&kind);
    out.extend_from_slice(data);
    let mut crc_src = Vec::with_capacity(4 + data.len());
    crc_src.extend_from_slice(&kind);
    crc_src.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_src).to_be_bytes());
}

fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut pos = 0;
    while pos < data.len() {
        let n = (data.len() - pos).min(65535);
        let last = pos + n == data.len();
        out.push(u8::from(last));
        let len = u16::try_from(n).expect("stored block");
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(&data[pos..pos + n]);
        pos += n;
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    let (mut s1, mut s2) = (1u32, 0u32);
    for &b in data {
        s1 = (s1 + u32::from(b)) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    (s2 << 16) | s1
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ 0xEDB8_8320
            };
        }
    }
    !crc
}
