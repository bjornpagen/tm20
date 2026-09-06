//! One interpreter for preview and optional delivery.
//!
//! Order is prepare (caller) → preview → deliver. `open` runs only in
//! [`OutputMode::Deliver`]. Dry and Fake never open a transport.
//!
//! `--fake-delivery DIR` writes `DIR/{name}.bin` (encoded job bytes) and
//! never calls USB. `--dry` and default Deliver USB are unchanged.
//!
//! [`run`] / [`execute`] take `FnOnce() -> Result<T: Transport>`.
//! Tests inject [`tm20::Memory`] or any other `Transport`. There is no USB
//! factory and no general DI. PNG `DIR/{name}.png` overwrites if present.

use std::path::Path;

use tm20::Transport;
use tm20::command::Command;
use tm20_set::preview_pngs;

use crate::Result;
use crate::args::{OutputMode, Selection};
use crate::jobs::{PreparedJob, enumerate, prepare_all};

/// Enumerate, prepare the whole batch, then interpret. Device open is last.
pub fn run<T: Transport>(
    selection: &Selection,
    mode: &OutputMode,
    open: impl FnOnce() -> Result<T>,
) -> Result<()> {
    let specs = enumerate(selection)?;
    let jobs = prepare_all(&specs)?;
    execute(&jobs, mode, open)
}

/// Preview every job, report, then deliver only in Deliver mode.
pub fn execute<T: Transport>(
    jobs: &[PreparedJob],
    mode: &OutputMode,
    open: impl FnOnce() -> Result<T>,
) -> Result<()> {
    write_previews(jobs, mode.preview_dir())?;
    for job in jobs {
        report(job, mode.is_dry());
    }
    match mode {
        OutputMode::Dry { .. } => Ok(()),
        OutputMode::Fake { sink_dir, .. } => write_fake_delivery(jobs, sink_dir),
        OutputMode::Deliver { .. } => {
            let mut transport = open()?;
            for job in jobs {
                transport.write(&job.bytes)?;
            }
            Ok(())
        }
    }
}

/// Writes each job's encoded bytes to `{dir}/{name}.bin`. Never USB.
fn write_fake_delivery(jobs: &[PreparedJob], dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    for job in jobs {
        let path = dir.join(format!("{}.bin", job.name));
        std::fs::write(&path, &job.bytes)?;
    }
    Ok(())
}

fn write_previews(jobs: &[PreparedJob], dir: Option<&Path>) -> Result<()> {
    let Some(dir) = dir else {
        return Ok(());
    };
    for job in jobs {
        write_preview(dir, job)?;
    }
    Ok(())
}

/// Writes `{dir}/{job.name}.png`, replacing any file already at that path.
fn write_preview(dir: &Path, job: &PreparedJob) -> Result<()> {
    let gs: Vec<_> = job
        .document
        .commands()
        .iter()
        .filter_map(|c| match c {
            Command::Graphics(g) => Some(g),
            _ => None,
        })
        .collect();
    if gs.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.png", job.name));
    std::fs::write(&path, preview_pngs(gs)?)?;
    eprintln!("png: {}", path.display());
    Ok(())
}

fn report(job: &PreparedJob, dry: bool) {
    eprintln!("{}", job.label());
    if !dry {
        return;
    }
    let heights = graphics_heights(&job.document);
    if !heights.is_empty() {
        let h: u32 = heights.iter().copied().sum();
        eprintln!(
            "graphics: {} band(s), heights {heights:?}, H={h}",
            heights.len()
        );
    }
    println!("{}", job.bytes.len());
}

fn graphics_heights(doc: &tm20::Document) -> Vec<u32> {
    doc.commands()
        .iter()
        .filter_map(|c| match c {
            Command::Graphics(g) => Some(g.raster().height()),
            _ => None,
        })
        .collect()
}

/// Preview size: commanded scale, then 2× screen nearest-neighbor.
#[cfg(test)]
pub fn expected_preview_dims(doc: &tm20::Document) -> (u32, u32) {
    let bands: Vec<_> = doc
        .commands()
        .iter()
        .filter_map(|c| match c {
            Command::Graphics(g) => Some(g),
            _ => None,
        })
        .collect();
    if bands.is_empty() {
        return (2, 2);
    }
    let (sx0, _) = bands[0].scale().factors();
    let w0 = u32::from(bands[0].raster().width()) * u32::from(sx0);
    let mut h = 0u32;
    for g in bands {
        let (_, sy) = g.scale().factors();
        h = h.saturating_add(g.raster().height().saturating_mul(u32::from(sy)));
    }
    (w0.saturating_mul(2).max(1), h.saturating_mul(2).max(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::OutputMode;
    use crate::jobs::PreparedJob;
    use std::cell::Cell;
    use std::fs;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tm20::command::Command;
    use tm20::encode::encode;
    use tm20::graphics::{Graphics, GraphicsScale};
    use tm20::{Document, Memory, Raster};

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

    fn graphics_job(name: &str, scale: GraphicsScale) -> PreparedJob {
        let raster = Raster::from_bits(8, 1, &[true; 8]).unwrap();
        let g = Graphics::new(raster, scale).unwrap();
        let document = Document::new(vec![Command::Graphics(g)]);
        let bytes = encode(&document).unwrap();
        PreparedJob::synthetic(name, document, bytes)
    }

    fn empty_job(name: &str) -> PreparedJob {
        let document = Document::new(vec![Command::Init]);
        let bytes = encode(&document).unwrap();
        PreparedJob::synthetic(name, document, bytes)
    }

    fn png_ihdr_dims(bytes: &[u8]) -> (u32, u32) {
        assert!(bytes.len() >= 24, "png too short");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&bytes[12..16], b"IHDR");
        let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
        (w, h)
    }

    fn dry(preview: Option<PathBuf>) -> OutputMode {
        OutputMode::Dry {
            preview_dir: preview,
        }
    }

    fn deliver(preview: Option<PathBuf>) -> OutputMode {
        OutputMode::Deliver {
            preview_dir: preview,
            selector: Some("fake".into()),
        }
    }

    fn fake(sink: PathBuf) -> OutputMode {
        OutputMode::Fake {
            sink_dir: sink,
            preview_dir: None,
        }
    }

    #[test]
    fn dry_never_opens_or_writes() {
        let job = graphics_job("ticket", GraphicsScale::Normal);
        let opens = Rc::new(Cell::new(0u32));
        let opens_c = Rc::clone(&opens);
        execute(std::slice::from_ref(&job), &dry(None), move || {
            opens_c.set(opens_c.get() + 1);
            Ok(Memory::new())
        })
        .unwrap();
        assert_eq!(opens.get(), 0);
    }

    #[test]
    fn deliver_fake_writes_each_job_once() {
        let a = graphics_job("a", GraphicsScale::Normal);
        let b = empty_job("b");
        let jobs = [a, b];
        let mut sink = Memory::new();
        execute(&jobs, &deliver(None), || Ok(Recording::new(&mut sink))).unwrap();
        assert_eq!(
            sink.written,
            [jobs[0].bytes.as_slice(), jobs[1].bytes.as_slice()].concat()
        );
    }

    struct Recording<'a> {
        inner: &'a mut Memory,
    }

    impl<'a> Recording<'a> {
        fn new(inner: &'a mut Memory) -> Self {
            Self { inner }
        }
    }

    impl Transport for Recording<'_> {
        fn write(&mut self, data: &[u8]) -> tm20::Result<()> {
            self.inner.write(data)
        }
        fn read(&mut self, buf: &mut [u8]) -> tm20::Result<usize> {
            self.inner.read(buf)
        }
    }

    #[test]
    fn preview_matrix_names_count_and_dims() {
        for (mode_preview, scale, name) in [
            (true, GraphicsScale::Normal, "ticket"),
            (true, GraphicsScale::Quadruple, "suite"),
            (false, GraphicsScale::Normal, "prose"),
        ] {
            let tmp = Tmp(unique_temp(&format!("png-{name}")));
            let job = graphics_job(name, scale);
            let preview = mode_preview.then(|| tmp.0.clone());
            for mode in [dry(preview.clone()), deliver(preview.clone())] {
                let mut sink = Memory::new();
                execute(std::slice::from_ref(&job), &mode, || {
                    Ok(Recording::new(&mut sink))
                })
                .unwrap();
                let png = tmp.0.join(format!("{name}.png"));
                if mode_preview {
                    let bytes = fs::read(&png).unwrap();
                    let got = png_ihdr_dims(&bytes);
                    assert_eq!(
                        got,
                        expected_preview_dims(&job.document),
                        "{name} {scale:?}"
                    );
                    let entries: Vec<_> = fs::read_dir(&tmp.0)
                        .unwrap()
                        .map(|e| e.unwrap().file_name())
                        .collect();
                    assert_eq!(entries.len(), 1, "only the named preview");
                } else {
                    assert!(!png.exists());
                }
                if mode.is_dry() {
                    assert!(sink.written.is_empty());
                } else {
                    assert_eq!(sink.written, job.bytes);
                }
            }
        }
    }

    #[test]
    fn all_four_scales_have_distinct_effective_preview_size() {
        let tmp = Tmp(unique_temp("scales"));
        for (scale, name) in [
            (GraphicsScale::Normal, "n"),
            (GraphicsScale::DoubleWidth, "w"),
            (GraphicsScale::DoubleHeight, "h"),
            (GraphicsScale::Quadruple, "q"),
        ] {
            let job = graphics_job(name, scale);
            execute(
                std::slice::from_ref(&job),
                &dry(Some(tmp.0.clone())),
                || -> Result<Memory> { panic!("dry must not open") },
            )
            .unwrap();
            let bytes = fs::read(tmp.0.join(format!("{name}.png"))).unwrap();
            assert_eq!(png_ihdr_dims(&bytes), expected_preview_dims(&job.document));
        }
        let n = png_ihdr_dims(&fs::read(tmp.0.join("n.png")).unwrap());
        let w = png_ihdr_dims(&fs::read(tmp.0.join("w.png")).unwrap());
        let h = png_ihdr_dims(&fs::read(tmp.0.join("h.png")).unwrap());
        let q = png_ihdr_dims(&fs::read(tmp.0.join("q.png")).unwrap());
        assert_eq!(n, (16, 2));
        assert_eq!(w, (32, 2));
        assert_eq!(h, (16, 4));
        assert_eq!(q, (32, 4));
    }

    #[test]
    fn directory_and_builtin_names_write_one_png_each() {
        let tmp = Tmp(unique_temp("names"));
        let jobs = [
            graphics_job("ticket", GraphicsScale::Normal),
            graphics_job("01-prose", GraphicsScale::Normal),
        ];
        execute(&jobs, &dry(Some(tmp.0.clone())), || -> Result<Memory> {
            panic!("dry must not open")
        })
        .unwrap();
        assert!(tmp.0.join("ticket.png").is_file());
        assert!(tmp.0.join("01-prose.png").is_file());
        assert_eq!(fs::read_dir(&tmp.0).unwrap().count(), 2);
    }

    #[test]
    fn fake_delivery_writes_job_bytes_and_never_opens() {
        let tmp = Tmp(unique_temp("fake-sink"));
        let job = graphics_job("ticket", GraphicsScale::Normal);
        let opens = Rc::new(Cell::new(0u32));
        let opens_c = Rc::clone(&opens);
        execute(
            std::slice::from_ref(&job),
            &fake(tmp.0.clone()),
            move || {
                opens_c.set(opens_c.get() + 1);
                Ok(Memory::new())
            },
        )
        .unwrap();
        assert_eq!(opens.get(), 0);
        let path = tmp.0.join("ticket.bin");
        assert_eq!(fs::read(&path).unwrap(), job.bytes);
        assert_eq!(fs::read_dir(&tmp.0).unwrap().count(), 1);
    }

    #[test]
    fn run_prepare_failure_never_opens() {
        let tmp = Tmp(unique_temp("run-fail"));
        fs::create_dir(tmp.0.join("gone.md")).unwrap();
        let opens = Rc::new(Cell::new(0u32));
        let opens_c = Rc::clone(&opens);
        let err = run(
            &Selection::Markdown(tmp.0.clone()),
            &deliver(None),
            move || {
                opens_c.set(opens_c.get() + 1);
                Ok(Memory::new())
            },
        );
        assert!(err.is_err());
        assert_eq!(opens.get(), 0);
    }
}
