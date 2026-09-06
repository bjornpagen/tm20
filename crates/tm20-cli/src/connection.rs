//! CLI boundary for selecting one transport. No connections during parsing.

use std::num::NonZeroU32;
use std::str::FromStr;

use tm20::{Serial, Tcp, Transport, Usb};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    Usb { serial: Option<String> },
    Tcp(Endpoint),
    Serial { path: String, baud: NonZeroU32 },
}

impl Destination {
    pub fn open(&self) -> tm20::Result<Box<dyn Transport>> {
        match self {
            Self::Usb { serial } => Ok(Box::new(Usb::open(serial.as_deref())?)),
            Self::Tcp(endpoint) => Ok(Box::new(Tcp::connect((
                endpoint.host.as_str(),
                endpoint.port,
            ))?)),
            Self::Serial { path, baud } => Ok(Box::new(Serial::open(path, baud.get())?)),
        }
    }
}

/// A host and nonzero port, checked before any DNS or device access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    host: String,
    port: u16,
}

impl FromStr for Endpoint {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        const ERROR: &str = "expected HOST:PORT or [IPv6]:PORT, with port 1..65535";
        let (host, port) = value.rsplit_once(':').ok_or(ERROR)?;
        let host = if host.starts_with('[') && host.ends_with(']') {
            let inner = &host[1..host.len() - 1];
            inner.parse::<std::net::Ipv6Addr>().map_err(|_| ERROR)?;
            inner
        } else {
            if host.is_empty()
                || host.contains([':', '/', '[', ']'])
                || host.chars().any(char::is_whitespace)
            {
                return Err(ERROR);
            }
            host
        };
        let port = port
            .parse::<std::num::NonZeroU16>()
            .map_err(|_| ERROR)?
            .get();
        Ok(Self {
            host: host.into(),
            port,
        })
    }
}

#[derive(Debug, usage::Args)]
#[usage(group("transport"))]
pub struct ConnectionArgs {
    /// Use the TM-T20III USB interface (default).
    #[usage(long, group = "transport")]
    usb: bool,
    /// Select USB by device serial number; --serial is an alias.
    #[usage(long, alias = "--serial", conflicts("--tcp", "--serial-port"))]
    usb_serial: Option<String>,
    /// Deliver to a raw TCP printer port, e.g. printer.local:9100.
    #[usage(long, group = "transport", value_name = "HOST:PORT")]
    tcp: Option<Endpoint>,
    /// Open a named serial port; requires --baud.
    #[usage(long, group = "transport", requires("--baud"), value_name = "PATH")]
    serial_port: Option<String>,
    /// Serial baud rate (nonzero); requires --serial-port.
    #[usage(long, requires("--serial-port"), value_name = "RATE")]
    baud: Option<NonZeroU32>,
}

impl ConnectionArgs {
    /// Absence stays distinct from an explicitly requested USB destination.
    pub fn destination(self) -> Result<Option<Destination>, &'static str> {
        match (self.tcp, self.serial_port, self.baud) {
            (Some(endpoint), None, None) => Ok(Some(Destination::Tcp(endpoint))),
            (None, Some(path), Some(baud)) if !path.is_empty() => {
                Ok(Some(Destination::Serial { path, baud }))
            }
            (None, None, None) => {
                if self.usb_serial.as_deref() == Some("") {
                    return Err("--usb-serial must not be empty");
                }
                Ok(
                    (self.usb || self.usb_serial.is_some()).then_some(Destination::Usb {
                        serial: self.usb_serial,
                    }),
                )
            }
            _ => Err("--serial-port needs a nonempty path and --baud needs a nonzero rate"),
        }
    }
}

impl Default for Destination {
    fn default() -> Self {
        Self::Usb { serial: None }
    }
}
