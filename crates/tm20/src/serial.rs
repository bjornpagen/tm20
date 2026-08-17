//! Serial-port transport.

use std::io::{Read, Write};
use std::time::Duration;

use crate::error::Result;
use crate::transport::Transport;

pub struct Serial {
    port: Box<dyn serialport::SerialPort>,
}

impl Serial {
    pub fn open(path: &str, baud: u32) -> Result<Self> {
        let port = serialport::new(path, baud).timeout(Duration::MAX).open()?;
        Ok(Self { port })
    }
}

impl Transport for Serial {
    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.port.write_all(data)?;
        self.port.flush()?;
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        Ok(self.port.read(buf)?)
    }
}
