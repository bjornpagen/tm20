//! House faces plus the checked-in lock. Missing fonts and lock drift fail.
#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

use tm20_set::{Cut, DisplayCut, Face, FaceTable};

pub const HELVETICA: &str = "/System/Library/Fonts/Helvetica.ttc";
pub const MENLO: &str = "/System/Library/Fonts/Menlo.ttc";

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn file_digest(path: &str) -> u64 {
    fnv1a64(&std::fs::read(path).unwrap_or_else(|e| panic!("{path} not on this machine: {e}")))
}

pub fn lock_text() -> String {
    format!(
        "helvetica {:016x}\nmenlo {:016x}\n",
        file_digest(HELVETICA),
        file_digest(MENLO)
    )
}

pub fn lock_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/faces.lock")
}

/// Fail if either house face is absent or `faces.lock` disagrees. Never skip.
pub fn require_locked_fonts() {
    static LOCKED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    LOCKED.get_or_init(check_locked_fonts);
}

fn check_locked_fonts() {
    assert!(
        Path::new(HELVETICA).is_file(),
        "{HELVETICA} not on this machine"
    );
    assert!(Path::new(MENLO).is_file(), "{MENLO} not on this machine");
    let path = lock_path();
    let have = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} — font lock is mandatory evidence", path.display()));
    let want = lock_text();
    assert_eq!(have, want, "font drift — inspect and re-bless");
}

pub fn table() -> FaceTable {
    thread_local! {
        static TABLE: FaceTable = load_table();
    }
    TABLE.with(Clone::clone)
}

fn load_table() -> FaceTable {
    require_locked_fonts();
    let mut table = FaceTable::new();
    table.absorb(std::fs::read(HELVETICA).expect("Helvetica.ttc"));
    table.absorb(std::fs::read(MENLO).expect("Menlo.ttc"));
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

/// A table missing exactly the named pieces, for boundary-error facts.
pub fn partial_table(bold: bool, mono: bool, display: bool) -> FaceTable {
    require_locked_fonts();
    let bytes: Arc<[u8]> = std::fs::read(HELVETICA).expect("Helvetica.ttc").into();
    let mut out = FaceTable::new();
    for index in 0.. {
        let Ok(face) = Face::from_bytes_index(bytes.clone(), index) else {
            break;
        };
        let Some(name) = face.postscript_name() else {
            continue;
        };
        match name.as_str() {
            "Helvetica" => {
                out.set_text(
                    Cut::Roman,
                    Face::from_bytes_index(bytes.clone(), index)
                        .expect("Helvetica Regular")
                        .text(),
                );
                if display {
                    out.set_display(DisplayCut::Roman, face.display());
                }
            }
            "Helvetica-Bold" if bold => out.set_text(Cut::Bold, face.text()),
            "Helvetica-Oblique" => out.set_text(Cut::Italic, face.text()),
            "Helvetica-BoldOblique" => out.set_text(Cut::BoldItalic, face.text()),
            _ => {}
        }
    }
    if mono {
        out.set_text(Cut::Mono, load_mono().text());
    }
    out
}
