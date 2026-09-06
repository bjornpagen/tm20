//! Typed declarations for the low-level protocol CLI.
use crate::connection::{ConnectionArgs, Destination};
use std::io;
use std::num::NonZeroU8;

/// Low-level ESC/POS commands. Device operations may feed paper or change state.
#[derive(Debug, usage::Cli)]
#[usage(bin = "tm20", version)]
pub struct Cli {
    #[usage(flatten)]
    connection: ConnectionArgs,
    /// Print planned hex bytes without opening a printer.
    #[usage(long)]
    dry: bool,
    /// Wait for the printer's completion reply after sending a job.
    #[usage(long)]
    wait: bool,
    /// Failure status for a valid invocation (1..255); success is always 0.
    #[usage(long, default = "1")]
    pub failure_exit_code: NonZeroU8,
    #[usage(subcommand)]
    command: RawCommand,
}

#[derive(Debug, usage::Subcommands)]
enum RawCommand {
    /// List USB devices.
    List,
    /// Inspect the USB printer interface.
    Debug,
    /// Print a greeting.
    Hello,
    /// Print a text page.
    Text(Text),
    /// Print a width ruler.
    Ruler,
    /// Query printer status.
    Status,
    /// Send error recovery.
    Recover,
    /// Query identity.
    Id,
    /// Print a QR code.
    Qr(Data),
    /// Print an EAN13 barcode.
    Ean13(Data),
    /// List or print protocol test pages.
    Test(Test),
}

#[derive(Debug, usage::Args)]
struct Text {
    text: Option<String>,
}
#[derive(Debug, usage::Args)]
struct Data {
    data: String,
}
#[derive(Debug, usage::Args)]
struct Test {
    id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Dry {
        wait: bool,
    },
    Deliver {
        destination: Destination,
        wait: bool,
    },
}

impl Mode {
    pub fn is_dry(&self) -> bool {
        matches!(self, Self::Dry { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    List,
    Debug,
    Hello,
    Text(String),
    Ruler,
    Status,
    Recover,
    Id,
    Qr(String),
    Ean13(String),
    TestList,
    TestAll,
    TestOne(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub mode: Mode,
    pub command: Command,
}

impl Cli {
    pub fn into_command(self) -> tm20::Result<Parsed> {
        if self.wait
            && matches!(
                self.command,
                RawCommand::List
                    | RawCommand::Debug
                    | RawCommand::Status
                    | RawCommand::Id
                    | RawCommand::Test(Test { id: None })
            )
        {
            return Err(invalid(
                "--wait applies only to commands that send a job, not queries or listings",
            )
            .into());
        }
        let destination = self.connection.destination().map_err(invalid)?;
        if self.dry && destination.is_some() {
            return Err(invalid("--dry cannot be combined with a printer destination").into());
        }
        if matches!(self.command, RawCommand::Debug | RawCommand::List)
            && destination
                .as_ref()
                .is_some_and(|d| !matches!(d, Destination::Usb { .. }))
        {
            return Err(invalid("list and debug are USB-only").into());
        }
        let mode = if self.dry {
            Mode::Dry { wait: self.wait }
        } else {
            Mode::Deliver {
                destination: destination.unwrap_or_default(),
                wait: self.wait,
            }
        };
        let command = match self.command {
            RawCommand::List => Command::List,
            RawCommand::Debug => Command::Debug,
            RawCommand::Hello => Command::Hello,
            RawCommand::Text(text) => Command::Text(text.text.unwrap_or_default()),
            RawCommand::Ruler => Command::Ruler,
            RawCommand::Status => Command::Status,
            RawCommand::Recover => Command::Recover,
            RawCommand::Id => Command::Id,
            RawCommand::Qr(data) => Command::Qr(data.data),
            RawCommand::Ean13(data) => Command::Ean13(data.data),
            RawCommand::Test(Test { id: None }) => Command::TestList,
            RawCommand::Test(Test { id: Some(id) }) if id == "all" => Command::TestAll,
            RawCommand::Test(Test { id: Some(id) }) => Command::TestOne(id),
        };
        Ok(Parsed { mode, command })
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

#[cfg(test)]
pub fn parse(args: impl IntoIterator<Item = impl AsRef<str>>) -> tm20::Result<Parsed> {
    let args: Vec<_> = args
        .into_iter()
        .map(|s| std::ffi::OsString::from(s.as_ref()))
        .collect();
    let refs: Vec<_> = args.iter().map(std::ffi::OsString::as_os_str).collect();
    Cli::parse_from(&refs)
        .map_err(|e| invalid(Cli::render_failure(&refs, &e)))?
        .into_command()
}
