//! House FaceTable and the authoritative faces.lock.

use std::path::{Path, PathBuf};

use tm20_set::FaceTable;

pub const HELVETICA: &str = "/System/Library/Fonts/Helvetica.ttc";
pub const MENLO: &str = "/System/Library/Fonts/Menlo.ttc";

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;

pub fn tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

pub fn lock_path() -> PathBuf {
    tests_dir().join("faces.lock")
}

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

pub fn file_digest(path: &str) -> u64 {
    fnv1a64(&std::fs::read(path).unwrap_or_else(|e| panic!("{path} not on this machine: {e}")))
}

pub fn lock_text() -> String {
    format!(
        "helvetica {:016x}\nmenlo {:016x}\n",
        file_digest(HELVETICA),
        file_digest(MENLO)
    )
}

/// Fail if fonts are missing or the lock disagrees. Never skip. Never rewrite.
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
    assert_eq!(have, lock_text(), "font drift — inspect and re-bless");
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
