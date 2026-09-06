mod args;
mod connection;
mod images;
mod jobs;
mod kit;
mod output;
mod sheets;

use args::{Cli, OutputMode, Request};
use std::process::ExitCode;

pub(crate) type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> ExitCode {
    let argv: Vec<_> = std::env::args_os().skip(1).collect();
    match Cli::embedded_outcome(&argv) {
        usage::embedded::Outcome::Exit(exit) => {
            if exit.stderr {
                eprint!("{}", exit.text);
            } else {
                print!("{}", exit.text);
            }
            ExitCode::from(u8::from(exit.code != 0))
        }
        usage::embedded::Outcome::Parsed(cli) => {
            let failure = cli.failure_exit_code.get();
            match cli.into_request().and_then(run) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(failure)
                }
            }
        }
    }
}

fn run(request: Request) -> Result<()> {
    match request {
        Request::FontLicenses => {
            print!("{}", tm20_set::FONT_LICENSES);
            Ok(())
        }
        Request::ListCatalog => {
            for case in sheets::catalog() {
                println!("  {:<10} {}", case.id(), case.title());
            }
            Ok(())
        }
        Request::Print(parsed) => print_job(&parsed),
    }
}

fn print_job(parsed: &args::Parsed) -> Result<()> {
    output::run(
        &parsed.selection,
        &parsed.mode,
        parsed.images,
        parsed.fonts,
        || {
            let OutputMode::Deliver { destination, .. } = &parsed.mode else {
                return Err("only delivery may open a transport".into());
            };
            destination.open().map_err(Into::into)
        },
    )
}
