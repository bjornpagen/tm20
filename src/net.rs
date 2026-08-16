//! TCP raw-port transport. Default port is 9100.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::error::Result;
use crate::transport::Transport;

const TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_PORT: u16 = 9100;

pub struct Tcp {
    stream: TcpStream,
}

impl Tcp {
    pub fn connect(addr: impl ToSocketAddrs) -> Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        Ok(Self { stream })
    }

    pub fn connect_9100(host: &str) -> Result<Self> {
        Self::connect((host, DEFAULT_PORT))
    }
}

impl Transport for Tcp {
    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.stream.write_all(data)?;
        self.stream.flush()?;
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        Ok(self.stream.read(buf)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn roundtrip() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4];
            sock.read_exact(&mut buf).unwrap();
            assert_eq!(&buf, b"ping");
            sock.write_all(b"pong").unwrap();
        });
        let mut client = Tcp::connect(addr).unwrap();
        client.write(b"ping").unwrap();
        let mut buf = [0u8; 4];
        client.read(&mut buf).unwrap();
        assert_eq!(&buf, b"pong");
        server.join().unwrap();
    }
}
