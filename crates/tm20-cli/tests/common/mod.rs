//! CLI process helpers. Every delivery path is dry or `--fake-delivery`.
//! Shared across CLI integration binaries; unused items stay for the others.
#![allow(dead_code)]

use std::path::PathBuf;
use std::process::{Command, Output};

pub fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_tm20-set")
}

pub fn uniq_temp(prefix: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "tm20-{prefix}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    p
}

pub fn run(args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn tm20-set: {e}"))
}

pub fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

pub fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}
