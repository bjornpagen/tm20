//! USB sink for the TM-T20III (`04b8:0e28`). Prefers printer class 7 with
//! bulk OUT. IN is optional and only used for status reads.
//!
//! The printer queues jobs in its receive buffer. This module is a pipe:
//! claim, write, read. Bulk OUT blocks until the printer drains; USB NAK is
//! backpressure. Do not send a USB zero-length packet (`flush_end`);
//! printer-class bulk OUT wedges on it.

use std::io::{self, Write};
use std::time::{Duration, Instant};

use nusb::MaybeFuture;
use nusb::descriptors::TransferType;
use nusb::transfer::{Buffer, Bulk, ControlIn, ControlType, Direction, In, Out, Recipient};

use crate::error::{Result, UsbError};
use crate::transport::Transport;
use crate::{PID, VID};

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
    in_ep: Option<nusb::Endpoint<Bulk, In>>,
    unread: Vec<u8>,
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
            in_ep: None,
            unread: Vec::new(),
        })
    }

    pub fn bulk_out(&self) -> u8 {
        self.out
    }

    pub fn bulk_in(&self) -> Option<u8> {
        self.inp
    }

    fn claim_in(&mut self) -> Result<()> {
        if self.in_ep.is_none() {
            let addr = self.inp.ok_or(UsbError::NoBulkIn)?;
            self.in_ep = Some(
                self.interface
                    .endpoint::<Bulk, In>(addr)
                    .map_err(UsbError::from)?,
            );
        }
        Ok(())
    }

    fn read_in(&mut self, buf: &mut [u8]) -> Result<usize> {
        let n = take_unread(&mut self.unread, buf);
        if n > 0 || buf.is_empty() {
            return Ok(n);
        }
        let started = Instant::now();
        self.claim_in()?;
        let packet = self
            .in_ep
            .as_ref()
            .expect("IN endpoint claimed")
            .max_packet_size();
        if packet == 0 {
            return Err(UsbError::Transfer(io::Error::new(
                io::ErrorKind::InvalidInput,
                "IN endpoint max packet size is 0",
            ))
            .into());
        }
        // nusb 0.2 Endpoint::reader() owns unread packet bytes only inside
        // EndpointRead. Dropping that reader (the previous per-call pattern)
        // discards the tail. A completed transfer is copied into `unread`
        // and lives until take_unread consumes it.
        let endpoint = self.in_ep.as_mut().expect("IN endpoint claimed");
        let n = read_packets(&mut self.unread, buf, || {
            endpoint
                .transfer_blocking(Buffer::new(packet), Duration::MAX)
                .into_result()
                .map(|data| data.to_vec())
                .map_err(|e| UsbError::Transfer(e.into()).into())
        })?;
        trace(&format!(
            "bulk IN {n} bytes {}ms",
            started.elapsed().as_millis()
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
                Duration::MAX,
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

fn read_packets(
    unread: &mut Vec<u8>,
    buf: &mut [u8],
    mut next: impl FnMut() -> Result<Vec<u8>>,
) -> Result<usize> {
    loop {
        let n = take_unread(unread, buf);
        if n > 0 || buf.is_empty() {
            return Ok(n);
        }
        // A successful USB zero-length packet is a packet boundary, not EOF.
        // Keep reading; disconnection and timeout still propagate as errors.
        *unread = next()?;
    }
}

fn take_unread(unread: &mut Vec<u8>, buf: &mut [u8]) -> usize {
    let n = unread.len().min(buf.len());
    if n == 0 {
        return 0;
    }
    buf[..n].copy_from_slice(&unread[..n]);
    unread.drain(..n);
    n
}

impl Transport for Usb {
    fn write(&mut self, data: &[u8]) -> Result<()> {
        let started = Instant::now();
        let endpoint = self
            .interface
            .endpoint::<Bulk, Out>(self.out)
            .map_err(UsbError::from)?;
        let max = endpoint.max_packet_size().max(64);
        let mut writer = endpoint.writer(max);
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
        self.read_in(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::{read_packets, take_unread};
    use crate::error::UsbError;

    #[test]
    fn zero_length_packets_are_not_eof() {
        let mut packets = [vec![], vec![], b"abc".to_vec()].into_iter();
        let mut unread = Vec::new();
        let mut buf = [0; 2];
        assert_eq!(
            read_packets(&mut unread, &mut buf, || Ok(packets.next().unwrap())).unwrap(),
            2
        );
        assert_eq!(&buf, b"ab");
        assert_eq!(unread, b"c");
        assert_eq!(
            read_packets(&mut unread, &mut buf, || panic!("tail already buffered")).unwrap(),
            1
        );
        assert_eq!(buf[0], b'c');
    }

    #[test]
    fn empty_read_does_not_request_packet() {
        assert_eq!(
            read_packets(&mut Vec::new(), &mut [], || panic!("empty read")).unwrap(),
            0
        );
    }

    #[test]
    fn transfer_error_after_zero_length_packet_propagates() {
        let mut calls = 0;
        let result = read_packets(&mut Vec::new(), &mut [0], || {
            calls += 1;
            if calls == 1 {
                Ok(Vec::new())
            } else {
                Err(UsbError::Transfer(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "test timeout",
                ))
                .into())
            }
        });
        assert!(result.is_err());
        assert_eq!(calls, 2);
    }

    #[test]
    fn packet_tail_survives_small_reads() {
        let mut unread = b"abcdef".to_vec();
        let mut first = [0u8; 2];
        assert_eq!(take_unread(&mut unread, &mut first), 2);
        assert_eq!(&first, b"ab");
        let mut second = [0u8; 3];
        assert_eq!(take_unread(&mut unread, &mut second), 3);
        assert_eq!(&second, b"cde");
        let mut third = [0u8; 8];
        assert_eq!(take_unread(&mut unread, &mut third), 1);
        assert_eq!(third[0], b'f');
        assert!(unread.is_empty());
    }
}
