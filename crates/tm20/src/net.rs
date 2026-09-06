//! TCP raw-port transport. Default port is 9100.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::error::Result;
use crate::transport::Transport;

pub const DEFAULT_PORT: u16 = 9100;

pub struct Tcp {
    stream: TcpStream,
}

impl Tcp {
    pub fn connect(addr: impl ToSocketAddrs) -> Result<Self> {
        let stream = TcpStream::connect(addr)?;
        Ok(Self { stream })
    }

    pub fn connect_9100(host: &str) -> Result<Self> {
        Self::connect((host, DEFAULT_PORT))
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        self.stream.set_read_timeout(timeout)?;
        Ok(())
    }

    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        self.stream.set_write_timeout(timeout)?;
        Ok(())
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
    use crate::reply::ReplyReader;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn roundtrip() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            sock.set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut buf = [0u8; 4];
            sock.read_exact(&mut buf).unwrap();
            assert_eq!(&buf, b"ping");
            sock.write_all(b"po").unwrap();
            sock.flush().unwrap();
            sock.write_all(b"ng").unwrap();
            sock.flush().unwrap();
        });
        let mut client = Tcp::connect(addr).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client.write(b"ping").unwrap();
        let mut reader = ReplyReader::new(client);
        let pong = reader.read_exact_reply(4).unwrap();
        assert_eq!(pong, b"pong");
        server.join().unwrap();
    }
}
