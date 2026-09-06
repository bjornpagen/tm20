//! Deterministic job enumeration and preparation. No device effects.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tm20::document::Document;
use tm20::encode::encode;
use tm20_set::Measure;

use crate::Result;
use crate::args::Selection;
use crate::images::ImagePolicy;
use crate::kit::system_table;
use crate::sheets::{Case, catalog, find};

/// One encoded job. `name` is the PNG stem and catalog id / markdown stem.
#[derive(Debug, Clone)]
pub struct PreparedJob {
    pub name: String,
    pub document: Document,
    pub bytes: Vec<u8>,
    origin: JobOrigin,
}

#[derive(Debug, Clone)]
enum JobOrigin {
    Catalog { id: String, title: String },
    Markdown { path_display: String },
}

impl PreparedJob {
    pub fn label(&self) -> String {
        match &self.origin {
            JobOrigin::Catalog { id, title } => format!("{id}: {title}"),
            JobOrigin::Markdown { path_display } => format!("md: {path_display}"),
        }
    }

    #[cfg(test)]
    pub fn synthetic(name: impl Into<String>, document: Document, bytes: Vec<u8>) -> Self {
        let name = name.into();
        Self {
            origin: JobOrigin::Catalog {
                id: name.clone(),
                title: name.clone(),
            },
            name,
            document,
            bytes,
        }
    }
}

#[derive(Debug, Clone)]
pub enum JobSpec {
    Catalog(Case),
    Markdown(PathBuf),
}

/// Build the selected specs. Read-directory errors are returned, never dropped.
pub fn enumerate(selection: &Selection) -> Result<Vec<JobSpec>> {
    match selection {
        Selection::ListCatalog => Ok(Vec::new()),
        Selection::All => Ok(catalog().iter().copied().map(JobSpec::Catalog).collect()),
        Selection::Builtin(id) => {
            let case = find(id).ok_or_else(|| format!("unknown sheet {id}"))?;
            Ok(vec![JobSpec::Catalog(case)])
        }
        Selection::Markdown(path) => {
            if path.is_dir() {
                let files = markdown_in_dir(path)?;
                if files.is_empty() {
                    return Err("no markdown in that directory".into());
                }
                Ok(files.into_iter().map(JobSpec::Markdown).collect())
            } else {
                Ok(vec![JobSpec::Markdown(path.clone())])
            }
        }
    }
}

/// Prepare every spec. The first failure stops the batch; nothing is delivered.
pub fn prepare_all(specs: &[JobSpec], images: ImagePolicy) -> Result<Vec<PreparedJob>> {
    specs.iter().map(|spec| prepare(spec, images)).collect()
}

fn prepare(spec: &JobSpec, images: ImagePolicy) -> Result<PreparedJob> {
    match spec {
        JobSpec::Catalog(case) => {
            let document = case.doc()?;
            let bytes = encode(&document)?;
            Ok(PreparedJob {
                name: case.id.to_owned(),
                document,
                bytes,
                origin: JobOrigin::Catalog {
                    id: case.id.to_owned(),
                    title: case.title.to_owned(),
                },
            })
        }
        JobSpec::Markdown(path) => {
            let document = md_document(path, images)?;
            let bytes = encode(&document)?;
            let name = path
                .file_stem()
                .map_or_else(|| "md".into(), |s| s.to_string_lossy().into_owned());
            Ok(PreparedJob {
                name,
                document,
                bytes,
                origin: JobOrigin::Markdown {
                    path_display: path.display().to_string(),
                },
            })
        }
    }
}

fn md_document(path: &Path, images: ImagePolicy) -> Result<Document> {
    let src = fs::read_to_string(path)?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let sheet = tm20_md::sheet(&src, Measure::TAPE, |dest| {
        crate::images::load(images, base, dest)
    })
    .map_err(|e| format!("{}: [{}] {e}", path.display(), e.code()))?;
    let faces = system_table()?;
    Ok(tm20_set::lower(&sheet, &faces)
        .map_err(|e| format!("{}: [{}] {e}", path.display(), e.code()))?)
}

fn markdown_in_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    collect_md_paths(fs::read_dir(dir)?.map(|entry| entry.map(|e| e.path())))
}

/// Surface every iterator error. Sort surviving `*.md` paths.
pub fn collect_md_paths<I>(entries: I) -> Result<Vec<PathBuf>>
where
    I: IntoIterator<Item = io::Result<PathBuf>>,
{
    let mut files = Vec::new();
    for entry in entries {
        let path = entry?;
        if path.extension().is_some_and(|ext| ext == "md") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::Selection;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("tm20-u11-{label}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    struct Tmp(PathBuf);
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn catalog_selection_is_stable() {
        let all = enumerate(&Selection::All).unwrap();
        let ids: Vec<_> = all
            .iter()
            .map(|s| match s {
                JobSpec::Catalog(c) => c.id,
                JobSpec::Markdown(_) => panic!("catalog"),
            })
            .collect();
        assert_eq!(ids, ["ticket", "prose", "helvetica", "suite"]);

        let one = enumerate(&Selection::Builtin("ticket".into())).unwrap();
        assert_eq!(one.len(), 1);
        assert!(enumerate(&Selection::Builtin("nope".into())).is_err());
    }

    #[test]
    fn directory_paths_sort_and_surface_read_errors() {
        let ok = collect_md_paths([
            Ok(PathBuf::from("z.md")),
            Ok(PathBuf::from("notes.txt")),
            Ok(PathBuf::from("a.md")),
        ])
        .unwrap();
        assert_eq!(ok, [PathBuf::from("a.md"), PathBuf::from("z.md")]);

        let err = collect_md_paths([
            Ok(PathBuf::from("a.md")),
            Err(io::Error::other("entry failed")),
            Ok(PathBuf::from("b.md")),
        ]);
        assert!(err.is_err(), "read errors must not be filtered out");
        assert!(err.unwrap_err().to_string().contains("entry failed"));
    }

    #[test]
    fn real_directory_order_and_empty_dir() {
        let tmp = Tmp(unique_temp("md-dir"));
        fs::write(tmp.0.join("b.md"), "# B\n").unwrap();
        fs::write(tmp.0.join("a.md"), "# A\n").unwrap();
        fs::write(tmp.0.join("skip.txt"), "no").unwrap();
        let specs = enumerate(&Selection::Markdown(tmp.0.clone())).unwrap();
        let names: Vec<_> = specs
            .iter()
            .map(|s| match s {
                JobSpec::Markdown(p) => p.file_name().unwrap().to_string_lossy().into_owned(),
                JobSpec::Catalog(_) => panic!("md"),
            })
            .collect();
        assert_eq!(names, ["a.md", "b.md"]);

        let empty = Tmp(unique_temp("md-empty"));
        let err = enumerate(&Selection::Markdown(empty.0.clone())).unwrap_err();
        assert_eq!(err.to_string(), "no markdown in that directory");
    }

    #[test]
    fn bad_preparation_is_an_error() {
        let tmp = Tmp(unique_temp("bad-prep"));
        fs::create_dir(tmp.0.join("a.md")).unwrap();
        fs::write(tmp.0.join("b.md"), "# ok\n").unwrap();
        let specs = enumerate(&Selection::Markdown(tmp.0.clone())).unwrap();
        assert!(prepare_all(&specs, ImagePolicy::default()).is_err());
    }
}
