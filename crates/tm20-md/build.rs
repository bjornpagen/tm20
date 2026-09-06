//! Expose each fixture to libtest/nextest instead of hiding it in a serial loop.

use std::fmt::Write as _;
use std::path::Path;

fn cases(root: &str, module: &str, runner: &str) -> String {
    println!("cargo:rerun-if-changed={root}");
    let mut stems: Vec<_> = std::fs::read_dir(root)
        .expect("fixture directory")
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .map(|path| path.file_stem().unwrap().to_str().unwrap().to_owned())
        .collect();
    stems.sort();
    assert!(!stems.is_empty(), "{root}: no fixtures");
    let mut out = format!("mod {module} {{\npub const STEMS: &[&str] = &{stems:?};\n");
    let mut names = std::collections::BTreeSet::new();
    for stem in stems {
        let name: String = stem
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        assert!(names.insert(name.clone()), "duplicate fixture name: {stem}");
        writeln!(
            out,
            "#[test] fn case_{name}() {{ super::{runner}({stem:?}); }}"
        )
        .unwrap();
    }
    out.push_str("}\n");
    out
}

fn main() {
    let out = std::env::var_os("OUT_DIR").expect("OUT_DIR");
    let out = Path::new(&out);
    let snap = cases("tests/corpus", "corpus", "corpus_case")
        + &cases("tests/reject", "reject", "reject_case");
    std::fs::write(out.join("snap_cases.rs"), snap).expect("snapshot cases");
    std::fs::write(
        out.join("paper_cases.rs"),
        cases("fixtures", "fixtures", "fixture_case"),
    )
    .expect("paper cases");
}
