//! In-memory transport for tests.

use crate::error::Result;
use crate::transport::Transport;

#[derive(Debug, Default)]
pub struct Memory {
    pub written: Vec<u8>,
    replies: Vec<u8>,
    read_at: usize,
}

impl Memory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_replies(replies: Vec<u8>) -> Self {
        Self {
            written: Vec::new(),
            replies,
            read_at: 0,
        }
    }
}

impl Transport for Memory {
    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.written.extend_from_slice(data);
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let rest = self.replies.len().saturating_sub(self.read_at);
        let n = rest.min(buf.len());
        buf[..n].copy_from_slice(&self.replies[self.read_at..self.read_at + n]);
        self.read_at += n;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::encode;
    use crate::host::hello;

    #[test]
    fn records_hello() {
        let mut mem = Memory::new();
        let bytes = encode(&hello()).unwrap();
        mem.write(&bytes).unwrap();
        assert_eq!(mem.written, bytes);
    }
}
