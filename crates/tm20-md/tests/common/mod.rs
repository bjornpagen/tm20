//! Shared Markdown integration helpers. Not a test binary.
//! Each integration binary uses a subset; unused items stay for the others.
#![allow(dead_code, unused_imports)]

pub mod admit;
pub mod assets;
pub mod compare;
pub mod faces;
pub mod parse;

pub use admit::{admit_corpus, admit_reject, defective_skip_on_missing};
pub use faces::{lock_text, require_locked_fonts, table, tests_dir};
pub use parse::{parse, parse_err, span_cut, span_note, span_text, text_runs};

pub struct TempDir(std::path::PathBuf);

impl std::ops::Deref for TempDir {
    type Target = std::path::Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn uniq_temp(prefix: &str) -> TempDir {
    let p = std::env::temp_dir().join(format!(
        "tm20-{prefix}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    TempDir(p)
}
