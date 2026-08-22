//! Face table and font digests shared by snap and paper.

use tm20_set::FaceTable;

pub const HELVETICA: &str = "/System/Library/Fonts/Helvetica.ttc";
pub const MENLO: &str = "/System/Library/Fonts/Menlo.ttc";

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;

pub fn table() -> FaceTable {
    let mut table = FaceTable::new();
    table.absorb(std::fs::read(HELVETICA).expect("Helvetica.ttc"));
    table.absorb(std::fs::read(MENLO).expect("Menlo.ttc"));
    table
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
