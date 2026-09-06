mod args;
mod images;
mod jobs;
mod kit;
mod output;
mod sheets;

use std::process::ExitCode;

use sheets::catalog;
use tm20::Usb;

use crate::args::{OutputMode, Selection, parse};

pub(crate) type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn usage() {
    let ids: Vec<_> = catalog().iter().map(|c| c.id).collect();
    eprintln!(
        "tm20-set [--serial S] [--dry] [--png DIR] [--fake-delivery DIR] [--allow-remote-images] print [{}|all|md <path>]\n  sheets: {}\n  md path may be a file or a directory of *.md\n  --png writes DIR/<name>.png at 2× (overwrites that file) next to USB; --dry never opens USB\n  --fake-delivery writes DIR/<name>.bin (encoded job) and never opens USB\n  remote images are denied unless --allow-remote-images is set (also applies to --dry)\n  faces are Helvetica and Menlo from /System/Library/Fonts",
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

fn run() -> Result<()> {
    let parsed = match parse(std::env::args().skip(1)) {
        Ok(p) => p,
        Err(e) => {
            if e.to_string() == "unknown command" {
                usage();
                for case in catalog() {
                    eprintln!("  {:<8} {}", case.id, case.title);
                }
            }
            return Err(e);
        }
    };

    if matches!(parsed.selection, Selection::ListCatalog) {
        usage();
        for case in catalog() {
            eprintln!("  {:<8} {}", case.id, case.title);
        }
        return Ok(());
    }

    match &parsed.mode {
        OutputMode::Deliver { selector, .. } => {
            let selector = selector.clone();
            output::run(&parsed.selection, &parsed.mode, parsed.images, move || {
                Usb::open(selector.as_deref()).map_err(Into::into)
            })
        }
        OutputMode::Dry { .. } | OutputMode::Fake { .. } => output::run(
            &parsed.selection,
            &parsed.mode,
            parsed.images,
            || -> Result<tm20::Memory> {
                Err("dry/fake-delivery must not open a transport".into())
            },
        ),
    }
}
