//! USB sink for the TM-T20III (`04b8:0e28`). Prefers printer class 7 with
//! bulk OUT. IN is optional and only used for status reads.
//!
//! The printer queues jobs in its receive buffer. This module is a pipe:
//! claim, write, read. Do not send a USB zero-length packet (`flush_end`);
//! printer-class bulk OUT wedges on it.

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use nusb::MaybeFuture;
use nusb::descriptors::TransferType;
use nusb::transfer::{Bulk, ControlIn, ControlType, Direction, In, Out, Recipient};

use crate::error::{Result, UsbError};
use crate::transport::Transport;
use crate::{PID, VID};

const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const READ_TIMEOUT: Duration = Duration::from_secs(2);
/// Long enough for a queued job plus cut before `GS ( H` replies.
pub const WAIT_TIMEOUT: Duration = Duration::from_secs(30);
const PRINTER_CLASS: u8 = 7;

fn trace(msg: &str) {
    if std::env::var_os("TM20_TRACE").is_some() {
        eprintln!("tm20: {msg}");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortStatus {
    pub byte: u8,
    pub not_error: bool,
    pub selected: bool,
    pub paper_empty: bool,
}

impl PortStatus {
    pub fn from_byte(byte: u8) -> Self {
        Self {
            byte,
            not_error: (byte >> 3) & 1 == 1,
            selected: (byte >> 4) & 1 == 1,
            paper_empty: (byte >> 5) & 1 == 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UsbDeviceInfo {
    pub vid: u16,
    pub pid: u16,
    pub bus_id: String,
    pub address: u8,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
}

impl UsbDeviceInfo {
    pub fn is_tm20(&self) -> bool {
        self.vid == VID && self.pid == PID
    }
}

pub fn list() -> Result<Vec<UsbDeviceInfo>> {
    let devices = nusb::list_devices().wait().map_err(UsbError::from)?;
    Ok(devices
        .map(|d| UsbDeviceInfo {
            vid: d.vendor_id(),
            pid: d.product_id(),
            bus_id: d.bus_id().to_string(),
            address: d.device_address(),
            manufacturer: d.manufacturer_string().map(str::to_string),
            product: d.product_string().map(str::to_string),
            serial: d.serial_number().map(str::to_string),
        })
        .collect())
}

pub struct Usb {
    interface: nusb::Interface,
    iface: u8,
    out: u8,
    inp: Option<u8>,
}

impl Usb {
    pub fn open(serial: Option<&str>) -> Result<Self> {
        let started = Instant::now();
        let devices = nusb::list_devices().wait().map_err(UsbError::from)?;
        let info = devices
            .into_iter()
            .find(|d| {
                d.vendor_id() == VID
                    && d.product_id() == PID
                    && serial.is_none_or(|want| d.serial_number() == Some(want))
            })
            .ok_or(UsbError::NotFound {
                vid: VID,
                pid: PID,
                serial: serial.map(str::to_string),
            })?;

        let device = info.open().wait().map_err(UsbError::from)?;
        let configuration = device
            .active_configuration()
            .map_err(nusb::Error::from)
            .map_err(UsbError::from)?;

        let mut best: Option<(u8, u8, Option<u8>, bool)> = None;
        for alt in configuration.interface_alt_settings() {
            if alt.alternate_setting() != 0 {
                continue;
            }
            let mut out_addr = None;
            let mut in_addr = None;
            for ep in alt.endpoints() {
                if ep.transfer_type() != TransferType::Bulk {
                    continue;
                }
                match ep.direction() {
                    Direction::Out => out_addr = Some(ep.address()),
                    Direction::In => in_addr = Some(ep.address()),
                }
            }
            let Some(out_addr) = out_addr else {
                continue;
            };
            let is_printer = alt.class() == PRINTER_CLASS;
            let candidate = (alt.interface_number(), out_addr, in_addr, is_printer);
            match best {
                None => best = Some(candidate),
                Some((_, _, _, true)) => {}
                Some(_) if is_printer => best = Some(candidate),
                Some(_) => {}
            }
        }
        let (iface_num, out, inp, _) = best.ok_or(UsbError::NoBulkOut)?;
        let interface = device
            .claim_interface(iface_num)
            .wait()
            .map_err(UsbError::from)?;

        trace(&format!(
            "open {}ms iface={iface_num} out={out:#04x} in={}",
            started.elapsed().as_millis(),
            inp.map_or_else(|| "none".into(), |a| format!("{a:#04x}"))
        ));

        Ok(Self {
            interface,
            iface: iface_num,
            out,
            inp,
        })
    }

    pub fn bulk_out(&self) -> u8 {
        self.out
    }

    pub fn bulk_in(&self) -> Option<u8> {
        self.inp
    }

    pub fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        let addr = self.inp.ok_or(UsbError::NoBulkOut)?;
        let started = Instant::now();
        let endpoint = self
            .interface
            .endpoint::<Bulk, In>(addr)
            .map_err(UsbError::from)?;
        let max = endpoint.max_packet_size();
        let mut reader = endpoint.reader(max.max(64)).with_read_timeout(timeout);
        let n = reader.read(buf).map_err(UsbError::Transfer)?;
        trace(&format!(
            "bulk IN {n} bytes {}ms (timeout {}ms)",
            started.elapsed().as_millis(),
            timeout.as_millis()
        ));
        Ok(n)
    }

    pub fn port_status(&self) -> Result<PortStatus> {
        let data = self
            .interface
            .control_in(
                ControlIn {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request: 1,
                    value: 0,
                    index: u16::from(self.iface),
                    length: 1,
                },
                Duration::from_millis(500),
            )
            .wait()
            .map_err(|e| UsbError::Transfer(std::io::Error::other(e.to_string())))?;
        let byte = *data.first().ok_or_else(|| {
            UsbError::Transfer(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "empty GET_PORT_STATUS",
            ))
        })?;
        Ok(PortStatus::from_byte(byte))
    }
}

impl Transport for Usb {
    fn write(&mut self, data: &[u8]) -> Result<()> {
        let started = Instant::now();
        let endpoint = self
            .interface
            .endpoint::<Bulk, Out>(self.out)
            .map_err(UsbError::from)?;
        let max = endpoint.max_packet_size().max(64);
        let mut writer = endpoint.writer(max).with_write_timeout(WRITE_TIMEOUT);
        writer.write_all(data).map_err(UsbError::Transfer)?;
        writer.flush().map_err(UsbError::Transfer)?;
        trace(&format!(
            "bulk OUT {} bytes {}ms",
            data.len(),
            started.elapsed().as_millis()
        ));
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.read_timeout(buf, READ_TIMEOUT)
    }
}
