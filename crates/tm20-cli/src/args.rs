//! Usage declarations → checked job configuration. No I/O at this boundary.
use crate::Result;
use crate::connection::{ConnectionArgs, Destination};
use crate::images::ImagePolicy;
use crate::kit::FontProfile;
use std::num::NonZeroU8;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputMode {
    Dry {
        preview_dir: Option<PathBuf>,
    },
    Deliver {
        preview_dir: Option<PathBuf>,
        destination: Destination,
    },
    Fake {
        sink_dir: PathBuf,
        preview_dir: Option<PathBuf>,
    },
    Raw {
        target: RawTarget,
        preview_dir: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawTarget {
    Stdout,
    File(PathBuf),
}

impl OutputMode {
    pub fn preview_dir(&self) -> Option<&Path> {
        match self {
            Self::Dry { preview_dir }
            | Self::Deliver { preview_dir, .. }
            | Self::Fake { preview_dir, .. }
            | Self::Raw { preview_dir, .. } => preview_dir.as_deref(),
        }
    }
    pub fn is_dry(&self) -> bool {
        matches!(self, Self::Dry { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    All,
    Builtin(crate::sheets::Case),
    Markdown(PathBuf),
    Stdin { base_dir: PathBuf },
}

#[derive(Debug)]
pub enum Request {
    FontLicenses,
    ListCatalog,
    Print(Parsed),
}

#[derive(Debug)]
pub struct Parsed {
    pub mode: OutputMode,
    pub selection: Selection,
    pub images: ImagePolicy,
    pub fonts: FontProfile,
}

/// Strict Markdown → 576-dot ESC/POS. Prepare the complete batch before delivery.
#[derive(Debug, usage::Cli)]
#[usage(bin = "tm20-set", version)]
pub struct Cli {
    #[usage(flatten)]
    connection: ConnectionArgs,
    #[usage(arg_group)]
    output: Option<OutputChoice>,
    /// Write DIR/<name>.png previews at 2×; alone this still prints.
    #[usage(long, value_name = "DIR")]
    png: Option<PathBuf>,
    /// Explicitly permit HTTP(S) image fetching, including in dry mode.
    #[usage(long)]
    allow_remote_images: bool,
    /// Explicit font profile. Portable embeds Source Sans 3 / Source Code Pro.
    #[usage(long, value_enum, default = "portable")]
    fonts: FontProfile,
    /// Relative image root for stdin only; defaults to the working directory.
    #[usage(long, value_name = "DIR")]
    base_dir: Option<PathBuf>,
    /// Failure status for a valid invocation (1..255); success is always 0.
    #[usage(long, default = "1", value_name = "CODE")]
    pub failure_exit_code: NonZeroU8,
    #[usage(subcommand)]
    command: Commands,
}

#[derive(Debug, usage::ArgGroup)]
enum OutputChoice {
    /// Validate and encode, without opening a printer.
    Dry,
    /// Write one encoded job per DIR/<name>.bin; no printer.
    #[usage(value_name = "DIR")]
    FakeDelivery(PathBuf),
    /// Write concatenated ESC/POS to FILE, or '-' for stdout; no printer.
    #[usage(value_name = "FILE")]
    Output(PathBuf),
}

#[derive(Debug, usage::Subcommands)]
enum Commands {
    /// Print embedded font copyrights and licenses.
    FontLicenses,
    /// Print a built-in sheet, all built-ins, or Markdown. Bare print lists sheets.
    Print(Print),
}

#[derive(Debug, usage::Args)]
struct Print {
    #[usage(subcommand)]
    selection: Option<PrintSelection>,
}

#[derive(Debug, usage::Subcommands)]
enum PrintSelection {
    /// Receipt with columns and rules.
    Ticket,
    /// Paragraphs, headings, and lists.
    Prose,
    /// Font voice specimen (uses the selected font profile).
    Helvetica,
    /// Nested blocks, notes, code, and figure.
    Suite,
    /// All built-in sheets in catalog order.
    All,
    /// File, directory of sorted *.md entries, or '-' for UTF-8 stdin.
    Md(Markdown),
}

#[derive(Debug, usage::Args)]
struct Markdown {
    path: PathBuf,
}

impl Cli {
    pub fn into_request(self) -> Result<Request> {
        let destination = self.connection.destination()?;
        let has_job_options = destination.is_some()
            || self.output.is_some()
            || self.png.is_some()
            || self.base_dir.is_some()
            || self.allow_remote_images
            || self.fonts != FontProfile::Portable;
        let print = match self.command {
            Commands::FontLicenses => {
                if has_job_options {
                    return Err("font-licenses does not accept print-job options".into());
                }
                return Ok(Request::FontLicenses);
            }
            Commands::Print(print) => print,
        };
        if print.selection.is_none() && has_job_options {
            return Err(
                "print-job options require a sheet or Markdown input; bare print only lists sheets"
                    .into(),
            );
        }
        let preview_dir = self.png;
        if self.output.is_some() && destination.is_some() {
            return Err(
                "a printer destination cannot be combined with --dry, --output, or --fake-delivery"
                    .into(),
            );
        }
        let mode = match self.output {
            None => OutputMode::Deliver {
                preview_dir,
                destination: destination.unwrap_or_default(),
            },
            Some(OutputChoice::Dry) => OutputMode::Dry { preview_dir },
            Some(OutputChoice::FakeDelivery(sink_dir)) => OutputMode::Fake {
                sink_dir,
                preview_dir,
            },
            Some(OutputChoice::Output(path)) => OutputMode::Raw {
                target: if path == Path::new("-") {
                    RawTarget::Stdout
                } else {
                    RawTarget::File(path)
                },
                preview_dir,
            },
        };
        let selection = match print.selection {
            Some(PrintSelection::Md(Markdown { path })) if path == Path::new("-") => {
                Selection::Stdin {
                    base_dir: self.base_dir.clone().unwrap_or_else(|| PathBuf::from(".")),
                }
            }
            Some(PrintSelection::Md(Markdown { path })) => Selection::Markdown(path),
            Some(PrintSelection::All) => Selection::All,
            Some(PrintSelection::Ticket) => Selection::Builtin(crate::sheets::Case::Ticket),
            Some(PrintSelection::Prose) => Selection::Builtin(crate::sheets::Case::Prose),
            Some(PrintSelection::Helvetica) => Selection::Builtin(crate::sheets::Case::Helvetica),
            Some(PrintSelection::Suite) => Selection::Builtin(crate::sheets::Case::Suite),
            None => return Ok(Request::ListCatalog),
        };
        if self.base_dir.is_some() && !matches!(selection, Selection::Stdin { .. }) {
            return Err(
                "--base-dir applies only to print md -; files resolve images beside the source"
                    .into(),
            );
        }
        Ok(Request::Print(Parsed {
            mode,
            selection,
            fonts: self.fonts,
            images: if self.allow_remote_images {
                ImagePolicy::AllowRemote
            } else {
                ImagePolicy::LocalOnly
            },
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn path_arguments_preserve_non_utf8_bytes() {
        use std::os::unix::ffi::OsStrExt;
        let path = OsStr::from_bytes(b"tape-\xff.md");
        let cli = Cli::parse_from(&[
            OsStr::new("--dry"),
            OsStr::new("print"),
            OsStr::new("md"),
            path,
        ])
        .unwrap();
        let Request::Print(job) = cli.into_request().unwrap() else {
            panic!("print")
        };
        assert_eq!(job.selection, Selection::Markdown(PathBuf::from(path)));
    }

    fn parse(args: &[&str]) -> Result<Parsed> {
        let args: Vec<_> = args.iter().map(OsStr::new).collect();
        Cli::parse_from(&args)
            .map_err(|e| Cli::render_failure(&args, &e))?
            .into_request()
            .and_then(|request| match request {
                Request::Print(job) => Ok(job),
                _ => Err("expected a print job".into()),
            })
    }

    #[test]
    fn admission_is_strict() {
        for args in [
            vec!["--dry", "--output", "-", "print", "ticket"],
            vec!["--usb", "--tcp", "host:9100", "print", "ticket"],
            vec!["--serial", "S", "--tcp", "host:9100", "print", "ticket"],
            vec!["--dry", "--usb", "print", "ticket"],
            vec!["--baud", "9600", "print", "ticket"],
            vec!["--serial-port", "/dev/ttyS0", "print", "ticket"],
            vec![
                "--serial-port",
                "/dev/ttyS0",
                "--baud",
                "0",
                "print",
                "ticket",
            ],
            vec!["--tcp", "host:0", "print", "ticket"],
            vec!["--tcp", "host", "print", "ticket"],
            vec!["--failure-exit-code", "0", "print", "ticket"],
            vec!["--failure-exit-code", "256", "print", "ticket"],
            vec!["--fonts", "unknown", "print", "ticket"],
            vec!["--base-dir", "/tmp", "print", "ticket"],
            vec!["--wat", "print"],
            vec!["print", "md"],
            vec!["print", "md", "a", "b"],
            vec!["print", "ticket", "x"],
            vec!["print", "--dry"],
            vec!["--output", "-", "print"],
            vec!["--output", "-", "font-licenses"],
        ] {
            assert!(parse(&args).is_err(), "accepted {args:?}");
        }
    }

    #[test]
    fn input_and_effect_are_independent() {
        let cli = Cli::parse_from(&[OsStr::new("print")]).unwrap();
        assert!(matches!(cli.into_request().unwrap(), Request::ListCatalog));
        let parsed = parse(&["--output", "-", "--base-dir", "/tmp", "print", "md", "-"]).unwrap();
        assert_eq!(
            parsed.selection,
            Selection::Stdin {
                base_dir: "/tmp".into()
            }
        );
        assert_eq!(
            parsed.mode,
            OutputMode::Raw {
                target: RawTarget::Stdout,
                preview_dir: None
            }
        );
        assert_eq!(parsed.fonts, FontProfile::Portable);
        assert_eq!(parsed.images, ImagePolicy::LocalOnly);
        for flag in ["--serial", "--usb-serial"] {
            assert_eq!(
                parse(&[flag, "S", "print", "ticket"]).unwrap().mode,
                OutputMode::Deliver {
                    preview_dir: None,
                    destination: Destination::Usb {
                        serial: Some("S".into())
                    }
                }
            );
        }
        for flags in [
            vec![],
            vec!["--dry"],
            vec!["--fake-delivery", "/tmp"],
            vec!["--output", "-"],
        ] {
            let mut argv = flags;
            argv.extend(["--allow-remote-images", "print", "ticket"]);
            assert_eq!(parse(&argv).unwrap().images, ImagePolicy::AllowRemote);
        }
    }
}
