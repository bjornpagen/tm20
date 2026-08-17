mod kit;
mod sheets;

use std::env;
use std::path::Path;
use std::process::ExitCode;

use sheets::{catalog, find};
use tm20::command::Command;
use tm20::encode::encode;
use tm20::{Transport, Usb};
use tm20_set::{Measure, preview_pngs};

use crate::kit::system_table;

pub(crate) type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn usage() {
    let ids: Vec<_> = catalog().iter().map(|c| c.id).collect();
    eprintln!(
        "tm20-set [--serial S] [--dry] [--png DIR] print [{}|all|md <path>]\n  sheets: {}\n  md path may be a file or a directory of *.md\n  --png writes a 2× preview next to USB; --dry stays bytes\n  nhg reads Neue Haas Grotesk from ~/Library/Fonts; Mono is Commit Mono",
        ids.join("|"),
        ids.join(", ")
    );
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

struct Opts {
    serial: Option<String>,
    dry: bool,
    png: Option<String>,
    args: Vec<String>,
}

fn parse_opts() -> Result<Opts> {
    let mut serial = None;
    let mut dry = false;
    let mut png = None;
    let mut args = Vec::new();
    let mut raw = env::args().skip(1);
    while let Some(a) = raw.next() {
        match a.as_str() {
            "--serial" => {
                serial = Some(raw.next().ok_or("--serial needs a value")?);
            }
            "--dry" => dry = true,
            "--png" => {
                png = Some(raw.next().ok_or("--png needs a directory")?);
            }
            _ => {
                args.push(a);
                args.extend(raw);
                break;
            }
        }
    }
    Ok(Opts {
        serial,
        dry,
        png,
        args,
    })
}

fn run() -> Result<()> {
    let opts = parse_opts()?;
    let mut cmd = opts.args.into_iter();
    if let Some("print") = cmd.next().as_deref() {
        let next = cmd.next();
        if next.as_deref() == Some("md") {
            let path = cmd.next().ok_or("print md needs a path")?;
            print_md(&path, opts.serial.as_deref(), opts.dry, opts.png.as_deref())
        } else {
            print_cmd(
                next.as_deref(),
                opts.serial.as_deref(),
                opts.dry,
                opts.png.as_deref(),
            )
        }
    } else {
        usage();
        for case in catalog() {
            eprintln!("  {:<8} {}", case.id, case.title);
        }
        Err("unknown command".into())
    }
}

fn print_cmd(id: Option<&str>, serial: Option<&str>, dry: bool, png: Option<&str>) -> Result<()> {
    match id {
        None | Some("all") => {
            let ids: Vec<_> = if id == Some("all") {
                catalog().iter().map(|c| c.id).collect()
            } else {
                usage();
                for case in catalog() {
                    eprintln!("  {:<8} {}", case.id, case.title);
                }
                return Ok(());
            };
            if dry {
                for id in ids {
                    let case = find(id).unwrap();
                    let n = encode(&case.doc()?)?.len();
                    eprintln!("{}: {} ({n} bytes)", case.id, case.title);
                }
                return Ok(());
            }
            let mut usb = Usb::open(serial)?;
            for id in ids {
                let case = find(id).unwrap();
                eprintln!("{}: {}", case.id, case.title);
                let doc = case.doc()?;
                write_png(png, id, &doc)?;
                usb.write(&encode(&doc)?)?;
            }
            Ok(())
        }
        Some(id) => {
            let case = find(id).ok_or_else(|| format!("unknown sheet {id}"))?;
            eprintln!("{}: {}", case.id, case.title);
            let doc = case.doc()?;
            let bytes = encode(&doc)?;
            if dry {
                println!("{}", bytes.len());
                return Ok(());
            }
            write_png(png, id, &doc)?;
            Usb::open(serial)?.write(&bytes)?;
            Ok(())
        }
    }
}

fn print_md(path: &str, serial: Option<&str>, dry: bool, png: Option<&str>) -> Result<()> {
    let path = Path::new(path);
    if path.is_dir() {
        let mut files: Vec<_> = std::fs::read_dir(path)?
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "md"))
            .collect();
        files.sort();
        if files.is_empty() {
            return Err("no markdown in that directory".into());
        }
        if dry {
            for f in &files {
                let n = md_bytes(f)?.len();
                println!("{}: {n} bytes", f.file_name().unwrap().to_string_lossy());
            }
            return Ok(());
        }
        let mut usb = Usb::open(serial)?;
        for f in &files {
            eprintln!("md: {}", f.display());
            let (bytes, doc) = md_job(f)?;
            let name = f.file_stem().unwrap().to_string_lossy();
            write_png(png, &name, &doc)?;
            usb.write(&bytes)?;
        }
        return Ok(());
    }
    let (bytes, doc) = md_job(path)?;
    eprintln!("md: {}", path.display());
    let name = path.file_stem().unwrap().to_string_lossy();
    write_png(png, &name, &doc)?;
    if dry {
        let heights: Vec<_> = doc
            .commands()
            .iter()
            .filter_map(|c| match c {
                Command::Graphics(g) => Some(g.height_dots),
                _ => None,
            })
            .collect();
        let h: u32 = heights.iter().map(|h| u32::from(*h)).sum();
        eprintln!(
            "graphics: {} band(s), heights {heights:?}, H={h}",
            heights.len()
        );
        println!("{}", bytes.len());
        return Ok(());
    }
    Usb::open(serial)?.write(&bytes)?;
    Ok(())
}

fn write_png(dir: Option<&str>, name: &str, doc: &tm20::Document) -> Result<()> {
    let Some(dir) = dir else {
        return Ok(());
    };
    let gs: Vec<_> = doc
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
    let path = Path::new(dir).join(format!("{name}.png"));
    std::fs::write(&path, preview_pngs(gs)?)?;
    eprintln!("png: {}", path.display());
    Ok(())
}

fn md_job(path: &Path) -> Result<(Vec<u8>, tm20::Document)> {
    let src = std::fs::read_to_string(path)?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let sheet = tm20_md::sheet(&src, Measure::TAPE, |dest| tm20_md::image_bytes(base, dest))?;
    let faces = system_table()?;
    let doc = tm20_set::lower(&sheet, &faces)?;
    Ok((encode(&doc)?, doc))
}

fn md_bytes(path: &Path) -> Result<Vec<u8>> {
    Ok(md_job(path)?.0)
}
