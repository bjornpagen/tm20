mod kit;
mod sheets;

use std::env;
use std::process::ExitCode;

use sheets::{catalog, find};
use tm20::encode::encode;
use tm20::{Transport, Usb};

pub(crate) type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn usage() {
    let ids: Vec<_> = catalog().iter().map(|c| c.id).collect();
    eprintln!(
        "tm20-set [--serial S] [--dry] print [{}|all]\n  sheets: {}\n  nhg reads Neue Haas Grotesk from ~/Library/Fonts",
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
    args: Vec<String>,
}

fn parse_opts() -> Result<Opts> {
    let mut serial = None;
    let mut dry = false;
    let mut args = Vec::new();
    let mut raw = env::args().skip(1);
    while let Some(a) = raw.next() {
        match a.as_str() {
            "--serial" => {
                serial = Some(raw.next().ok_or("--serial needs a value")?);
            }
            "--dry" => dry = true,
            _ => {
                args.push(a);
                args.extend(raw);
                break;
            }
        }
    }
    Ok(Opts { serial, dry, args })
}

fn run() -> Result<()> {
    let opts = parse_opts()?;
    let mut cmd = opts.args.into_iter();
    match cmd.next().as_deref() {
        Some("print") => print_cmd(cmd.next(), &opts.serial, opts.dry),
        _ => {
            usage();
            for case in catalog() {
                eprintln!("  {:<8} {}", case.id, case.title);
            }
            Err("unknown command".into())
        }
    }
}

fn print_cmd(id: Option<String>, serial: &Option<String>, dry: bool) -> Result<()> {
    match id.as_deref() {
        None | Some("all") => {
            let ids: Vec<_> = if id.as_deref() == Some("all") {
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
            let mut usb = Usb::open(serial.as_deref())?;
            for id in ids {
                let case = find(id).unwrap();
                eprintln!("{}: {}", case.id, case.title);
                usb.write(&encode(&case.doc()?)?)?;
            }
            Ok(())
        }
        Some(id) => {
            let case = find(id).ok_or_else(|| format!("unknown sheet {id}"))?;
            eprintln!("{}: {}", case.id, case.title);
            let bytes = encode(&case.doc()?)?;
            if dry {
                println!("{}", bytes.len());
                return Ok(());
            }
            Usb::open(serial.as_deref())?.write(&bytes)?;
            Ok(())
        }
    }
}
