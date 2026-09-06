//! Nonempty, readable, bijective fixture admission. Isolated roots only for negatives.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum AdmitError {
    MissingDir { path: PathBuf },
    Empty { path: PathBuf },
    Unreadable { path: PathBuf, err: String },
    Duplicate { key: String },
    OrphanGolden { stem: String },
    MissingGolden { stem: String },
    ExtraExpect { stem: String },
    MissingExpect { stem: String },
    BadExpectLine { line: usize },
}

impl std::fmt::Display for AdmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdmitError::MissingDir { path } => {
                write!(f, "missing fixture directory {}", path.display())
            }
            AdmitError::Empty { path } => write!(f, "empty fixture set {}", path.display()),
            AdmitError::Unreadable { path, err } => {
                write!(f, "unreadable {}: {err}", path.display())
            }
            AdmitError::Duplicate { key } => write!(f, "duplicate expectation key {key}"),
            AdmitError::OrphanGolden { stem } => write!(f, "orphan golden {stem}.png"),
            AdmitError::MissingGolden { stem } => write!(f, "missing golden for {stem}"),
            AdmitError::ExtraExpect { stem } => write!(f, "{stem}: extra expect line"),
            AdmitError::MissingExpect { stem } => write!(f, "{stem}: missing expect line"),
            AdmitError::BadExpectLine { line } => {
                write!(f, "expect.txt:{line}: want `stem = message`")
            }
        }
    }
}

pub struct AdmittedCorpus {
    pub files: Vec<PathBuf>,
}

pub struct AdmittedReject {
    pub files: Vec<PathBuf>,
    pub expect: BTreeMap<String, String>,
}

fn stem_of(path: &Path) -> String {
    path.file_stem()
        .expect("stem")
        .to_string_lossy()
        .into_owned()
}

fn read_md_files(dir: &Path) -> Result<Vec<PathBuf>, AdmitError> {
    if !dir.is_dir() {
        return Err(AdmitError::MissingDir {
            path: dir.to_path_buf(),
        });
    }
    let entries = std::fs::read_dir(dir).map_err(|e| AdmitError::Unreadable {
        path: dir.to_path_buf(),
        err: e.to_string(),
    })?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| AdmitError::Unreadable {
            path: dir.to_path_buf(),
            err: e.to_string(),
        })?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "md") {
            std::fs::read_to_string(&path).map_err(|e| AdmitError::Unreadable {
                path: path.clone(),
                err: e.to_string(),
            })?;
            files.push(path);
        }
    }
    files.sort();
    if files.is_empty() {
        return Err(AdmitError::Empty {
            path: dir.to_path_buf(),
        });
    }
    Ok(files)
}

pub fn admit_corpus(root: &Path) -> Result<AdmittedCorpus, AdmitError> {
    admit_corpus_allowing(root, &[])
}

/// Like [`admit_corpus`], but approved stems may lack a golden so a bless
/// grant can create it. Unapproved missing goldens still fail.
pub fn admit_corpus_allowing(
    root: &Path,
    allow_missing: &[&str],
) -> Result<AdmittedCorpus, AdmitError> {
    let corpus = root.join("corpus");
    let goldens = root.join("goldens");
    let files = read_md_files(&corpus)?;
    if !goldens.is_dir() {
        return Err(AdmitError::MissingDir { path: goldens });
    }
    let golden_entries = std::fs::read_dir(&goldens).map_err(|e| AdmitError::Unreadable {
        path: goldens.clone(),
        err: e.to_string(),
    })?;
    let mut golden_stems = BTreeSet::new();
    for entry in golden_entries {
        let entry = entry.map_err(|e| AdmitError::Unreadable {
            path: goldens.clone(),
            err: e.to_string(),
        })?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "png") {
            std::fs::read(&path).map_err(|e| AdmitError::Unreadable {
                path: path.clone(),
                err: e.to_string(),
            })?;
            golden_stems.insert(stem_of(&path));
        }
    }
    let md_stems: BTreeSet<String> = files.iter().map(|p| stem_of(p)).collect();
    if golden_stems.is_empty()
        && !md_stems
            .iter()
            .all(|s| allow_missing.iter().any(|a| *a == s))
    {
        return Err(AdmitError::Empty { path: goldens });
    }
    for stem in &md_stems {
        if !golden_stems.contains(stem) && !allow_missing.iter().any(|a| *a == stem) {
            return Err(AdmitError::MissingGolden { stem: stem.clone() });
        }
    }
    for stem in &golden_stems {
        if !md_stems.contains(stem) {
            return Err(AdmitError::OrphanGolden { stem: stem.clone() });
        }
    }
    Ok(AdmittedCorpus { files })
}

pub fn admit_reject(root: &Path) -> Result<AdmittedReject, AdmitError> {
    let dir = root.join("reject");
    let files = read_md_files(&dir)?;
    let expect_path = dir.join("expect.txt");
    let expect_src = std::fs::read_to_string(&expect_path).map_err(|e| AdmitError::Unreadable {
        path: expect_path.clone(),
        err: e.to_string(),
    })?;
    let mut expect = BTreeMap::new();
    for (i, line) in expect_src.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((stem, msg)) = line.split_once('=') else {
            return Err(AdmitError::BadExpectLine { line: i + 1 });
        };
        let stem = stem.trim().to_string();
        if expect.contains_key(&stem) {
            return Err(AdmitError::Duplicate { key: stem });
        }
        expect.insert(stem, msg.trim().to_string());
    }
    let md_stems: BTreeSet<String> = files.iter().map(|p| stem_of(p)).collect();
    for stem in expect.keys() {
        if !md_stems.contains(stem) {
            return Err(AdmitError::ExtraExpect { stem: stem.clone() });
        }
    }
    for stem in &md_stems {
        if !expect.contains_key(stem) {
            return Err(AdmitError::MissingExpect { stem: stem.clone() });
        }
    }
    Ok(AdmittedReject { files, expect })
}

/// Known defective skip: missing corpus was treated as success.
pub fn defective_skip_on_missing(root: &Path) -> bool {
    !root.join("corpus").is_dir()
}
