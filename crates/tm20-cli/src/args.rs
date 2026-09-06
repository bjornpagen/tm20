//! Pure argument parsing for `tm20-set`.
//!
//! Options are leading only. Unknown flags and leftover positionals are errors.
//! `--png DIR/NAME.png` overwrites that path if it already exists.

use std::path::{Path, PathBuf};

use crate::Result;
use crate::images::ImagePolicy;

/// How the prepared batch is interpreted. Selection never carries this.
///
/// `--fake-delivery DIR` is process-level Deliver-fake: write encoded job
/// bytes to `DIR/{name}.bin` and never open USB. `--dry` and default
/// Deliver are unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputMode {
    Dry {
        preview_dir: Option<PathBuf>,
    },
    Deliver {
        preview_dir: Option<PathBuf>,
        selector: Option<String>,
    },
    Fake {
        sink_dir: PathBuf,
        preview_dir: Option<PathBuf>,
    },
}

impl OutputMode {
    pub fn preview_dir(&self) -> Option<&Path> {
        match self {
            Self::Dry { preview_dir }
            | Self::Deliver { preview_dir, .. }
            | Self::Fake { preview_dir, .. } => preview_dir.as_deref(),
        }
    }

    #[cfg(test)]
    pub fn selector(&self) -> Option<&str> {
        match self {
            Self::Deliver { selector, .. } => selector.as_deref(),
            Self::Dry { .. } | Self::Fake { .. } => None,
        }
    }

    #[cfg(test)]
    pub fn fake_sink(&self) -> Option<&Path> {
        match self {
            Self::Fake { sink_dir, .. } => Some(sink_dir),
            Self::Dry { .. } | Self::Deliver { .. } => None,
        }
    }

    pub fn is_dry(&self) -> bool {
        matches!(self, Self::Dry { .. })
    }
}

/// What to turn into [`crate::jobs::PreparedJob`] values. Not an output effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// `print` with no id: list the catalog and stop.
    ListCatalog,
    All,
    Builtin(String),
    Markdown(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub mode: OutputMode,
    pub selection: Selection,
    pub images: ImagePolicy,
}

pub fn parse<I, S>(args: I) -> Result<Parsed>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut serial = None;
    let mut dry = false;
    let mut png = None;
    let mut fake = None;
    let mut images = ImagePolicy::default();
    let mut rest = Vec::new();
    let mut raw = args.into_iter().map(|s| s.as_ref().to_owned());
    while let Some(a) = raw.next() {
        match a.as_str() {
            "--serial" => {
                serial = Some(take_value(&mut raw, "--serial needs a value")?);
            }
            "--dry" => dry = true,
            "--allow-remote-images" => images = ImagePolicy::AllowRemote,
            "--png" => {
                png = Some(PathBuf::from(take_value(
                    &mut raw,
                    "--png needs a directory",
                )?));
            }
            "--fake-delivery" => {
                fake = Some(PathBuf::from(take_value(
                    &mut raw,
                    "--fake-delivery needs a directory",
                )?));
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option {other}").into());
            }
            other => {
                rest.push(other.to_owned());
                rest.extend(raw);
                break;
            }
        }
    }

    let mode = match (dry, fake) {
        (true, Some(_)) => {
            return Err("--dry cannot be combined with --fake-delivery".into());
        }
        (true, None) => OutputMode::Dry { preview_dir: png },
        (false, Some(sink_dir)) => OutputMode::Fake {
            sink_dir,
            preview_dir: png,
        },
        (false, None) => OutputMode::Deliver {
            preview_dir: png,
            selector: serial,
        },
    };

    let mut cmd = rest.into_iter();
    let Some(verb) = cmd.next() else {
        return Err("unknown command".into());
    };
    if verb != "print" {
        return Err("unknown command".into());
    }

    let selection = match cmd.next().as_deref() {
        None => Selection::ListCatalog,
        Some("all") => {
            reject_extra(cmd.next())?;
            Selection::All
        }
        Some("md") => {
            let path = cmd.next().ok_or("print md needs a path")?;
            reject_extra(cmd.next())?;
            Selection::Markdown(PathBuf::from(path))
        }
        Some(id) => {
            reject_extra(cmd.next())?;
            Selection::Builtin(id.to_owned())
        }
    };

    Ok(Parsed {
        mode,
        selection,
        images,
    })
}

fn take_value(raw: &mut impl Iterator<Item = String>, err: &'static str) -> Result<String> {
    raw.next().ok_or_else(|| err.into())
}

fn reject_extra(extra: Option<String>) -> Result<()> {
    match extra {
        Some(a) => Err(format!("unexpected argument {a}").into()),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(args: &[&str]) -> Parsed {
        parse(args).unwrap_or_else(|e| panic!("parse {args:?} failed: {e}"))
    }

    fn parse_err(args: &[&str]) -> String {
        parse(args).unwrap_err().to_string()
    }

    #[test]
    fn remote_images_require_explicit_permission_in_every_mode() {
        for options in [vec![], vec!["--dry"], vec!["--fake-delivery", "/tmp/sink"]] {
            let mut args = options.clone();
            args.extend(["print", "md", "tape.md"]);
            assert_eq!(parse_ok(&args).images, ImagePolicy::LocalOnly);
            let mut args = options;
            args.extend(["--allow-remote-images", "print", "md", "tape.md"]);
            assert_eq!(parse_ok(&args).images, ImagePolicy::AllowRemote);
        }
    }

    #[test]
    fn leading_options_and_print_spellings() {
        let p = parse_ok(&[
            "--serial", "S", "--dry", "--png", "/tmp/p", "print", "ticket",
        ]);
        assert_eq!(
            p.mode,
            OutputMode::Dry {
                preview_dir: Some(PathBuf::from("/tmp/p")),
            }
        );
        assert_eq!(p.selection, Selection::Builtin("ticket".into()));

        let p = parse_ok(&["print", "all"]);
        assert_eq!(
            p.mode,
            OutputMode::Deliver {
                preview_dir: None,
                selector: None,
            }
        );
        assert_eq!(p.selection, Selection::All);

        let p = parse_ok(&["--png", "out", "print", "md", "tape.md"]);
        assert_eq!(p.mode.preview_dir(), Some(Path::new("out")));
        assert_eq!(p.selection, Selection::Markdown(PathBuf::from("tape.md")));

        assert_eq!(parse_ok(&["print"]).selection, Selection::ListCatalog);

        let p = parse_ok(&["--fake-delivery", "/tmp/sink", "print", "ticket"]);
        assert_eq!(
            p.mode,
            OutputMode::Fake {
                sink_dir: PathBuf::from("/tmp/sink"),
                preview_dir: None,
            }
        );
        assert_eq!(p.mode.fake_sink(), Some(Path::new("/tmp/sink")));
        assert!(p.mode.selector().is_none());
        assert!(!p.mode.is_dry());
    }

    #[test]
    fn unknown_options_and_extra_positionals_are_rejected() {
        assert_eq!(
            parse_err(&["--foo", "print", "ticket"]),
            "unknown option --foo"
        );
        assert_eq!(
            parse_err(&["print", "ticket", "leftover"]),
            "unexpected argument leftover"
        );
        assert_eq!(parse_err(&["print", "all", "x"]), "unexpected argument x");
        assert_eq!(
            parse_err(&["print", "md", "a.md", "b.md"]),
            "unexpected argument b.md"
        );
        assert_eq!(parse_err(&["print", "md"]), "print md needs a path");
        assert_eq!(parse_err(&["hello"]), "unknown command");
        assert_eq!(parse_err(&[]), "unknown command");
        assert_eq!(parse_err(&["--serial"]), "--serial needs a value");
        assert_eq!(parse_err(&["--png"]), "--png needs a directory");
        assert_eq!(
            parse_err(&["--fake-delivery"]),
            "--fake-delivery needs a directory"
        );
        assert_eq!(
            parse_err(&["--dry", "--fake-delivery", "/tmp/s", "print", "ticket"]),
            "--dry cannot be combined with --fake-delivery"
        );
        assert_eq!(
            parse_err(&["--fake-delivery", "/tmp/s", "print", "ticket", "x"]),
            "unexpected argument x"
        );
    }

    #[test]
    fn options_after_print_are_sheet_ids_not_flags() {
        let p = parse_ok(&["print", "--dry"]);
        assert_eq!(p.selection, Selection::Builtin("--dry".into()));
        assert!(!p.mode.is_dry());
    }
}
