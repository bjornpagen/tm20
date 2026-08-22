//! Face table and font digests shared by snap and paper.

use std::sync::Arc;

use tm20_set::{Cut, Face, FaceTable};

pub const HELVETICA: &str = "/System/Library/Fonts/Helvetica.ttc";
pub const MENLO: &str = "/System/Library/Fonts/Menlo.ttc";

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;

pub fn table() -> FaceTable {
    let bytes: Arc<[u8]> = std::fs::read(HELVETICA).expect("Helvetica.ttc").into();
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
    let bytes: Arc<[u8]> = std::fs::read(MENLO).expect("Menlo.ttc").into();
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

/// FNV-1a 64 over `bytes`. Drift detector, not a security hash.
#[allow(dead_code)]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[allow(dead_code)]
pub fn file_digest(path: &str) -> u64 {
    fnv1a64(&std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}")))
}

#[allow(dead_code)]
pub fn lock_text() -> String {
    format!(
        "helvetica {:016x}\nmenlo {:016x}\n",
        file_digest(HELVETICA),
        file_digest(MENLO)
    )
}
