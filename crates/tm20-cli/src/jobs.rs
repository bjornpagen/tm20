//! Deterministic job enumeration and preparation. No device effects.

use std::fs;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};

use tm20::document::Document;
use tm20::encode::encode;
use tm20_set::{FaceTable, Measure};

use crate::Result;
use crate::args::Selection;
use crate::images::ImagePolicy;
use crate::sheets::{Case, catalog};

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
    Stdin { base_dir: PathBuf },
}

/// Build the selected specs. Read-directory errors are returned, never dropped.
pub fn enumerate(selection: &Selection) -> Result<Vec<JobSpec>> {
    match selection {
        Selection::All => Ok(catalog().iter().copied().map(JobSpec::Catalog).collect()),
        Selection::Builtin(case) => Ok(vec![JobSpec::Catalog(*case)]),
        Selection::Stdin { base_dir } => Ok(vec![JobSpec::Stdin {
            base_dir: base_dir.clone(),
        }]),
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
pub fn prepare_all(
    specs: &[JobSpec],
    images: ImagePolicy,
    faces: &FaceTable,
) -> Result<Vec<PreparedJob>> {
    specs
        .iter()
        .map(|spec| prepare(spec, images, faces))
        .collect()
}

fn prepare(spec: &JobSpec, images: ImagePolicy, faces: &FaceTable) -> Result<PreparedJob> {
    match spec {
        JobSpec::Catalog(case) => {
            let document = case.doc(faces)?;
            let bytes = encode(&document)?;
            Ok(PreparedJob {
                name: case.id().to_owned(),
                document,
                bytes,
                origin: JobOrigin::Catalog {
                    id: case.id().to_owned(),
                    title: case.title().to_owned(),
                },
            })
        }
        JobSpec::Markdown(path) => {
            let src = fs::read_to_string(path)
                .map_err(|e| format!("{}: cannot read UTF-8 Markdown: {e}", path.display()))?;
            let document = md_document(
                &path.display().to_string(),
                &src,
                path.parent().unwrap_or_else(|| Path::new(".")),
                images,
                faces,
            )?;
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
        JobSpec::Stdin { base_dir } => {
            let mut src = String::new();
            std::io::stdin()
                .lock()
                .read_to_string(&mut src)
                .map_err(|e| format!("<stdin>: cannot read UTF-8 Markdown: {e}"))?;
            let document = md_document("<stdin>", &src, base_dir, images, faces)?;
            let bytes = encode(&document)?;
            Ok(PreparedJob {
                name: "stdin".into(),
                document,
                bytes,
                origin: JobOrigin::Markdown {
                    path_display: "<stdin>".into(),
                },
            })
        }
    }
}

fn md_document(
    label: &str,
    src: &str,
    base: &Path,
    images: ImagePolicy,
    faces: &FaceTable,
) -> Result<Document> {
    let sheet = tm20_md::sheet(src, Measure::TAPE, |dest| {
        crate::images::load(images, base, dest)
    })
    .map_err(|e| format!("{label}: [{}] {e}", e.code()))?;
    Ok(tm20_set::lower(&sheet, faces).map_err(|e| format!("{label}: [{}] {e}", e.code()))?)
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
                JobSpec::Catalog(c) => c.id(),
                JobSpec::Markdown(_) | JobSpec::Stdin { .. } => panic!("catalog"),
            })
            .collect();
        assert_eq!(ids, ["ticket", "prose", "helvetica", "suite"]);

        let one = enumerate(&Selection::Builtin(crate::sheets::Case::Ticket)).unwrap();
        assert_eq!(one.len(), 1);
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
                JobSpec::Catalog(_) | JobSpec::Stdin { .. } => panic!("md"),
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
        let faces = crate::kit::FontProfile::Portable.load().unwrap();
        assert!(prepare_all(&specs, ImagePolicy::default(), &faces).is_err());
    }
}
