//! Ignored corpus-asset generation. Not part of compare or bless.

use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};

pub fn write_all(dir: &Path) {
    std::fs::create_dir_all(dir).expect("assets dir");
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
    std::fs::write(path, out).expect("alpha.png");
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
