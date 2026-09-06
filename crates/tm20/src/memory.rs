//! In-memory transport for tests.

use std::collections::VecDeque;
use std::io::{self, ErrorKind};

use crate::error::Result;
use crate::transport::Transport;

/// One `read` outcome for a deterministic fake transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryRead {
    Bytes(Vec<u8>),
    Interrupt,
}

#[derive(Debug, Default)]
pub struct Memory {
    pub written: Vec<u8>,
    reads: VecDeque<MemoryRead>,
}

impl Memory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Deliver `replies` as a single pool; each `read` takes up to `buf.len()`.
    pub fn with_replies(replies: Vec<u8>) -> Self {
        Self::with_reads([MemoryRead::Bytes(replies)])
    }

    /// Each `read` returns at most the next segment, even when `buf` is larger.
    pub fn with_segments<I, S>(segments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Vec<u8>>,
    {
        Self::with_reads(segments.into_iter().map(|s| MemoryRead::Bytes(s.into())))
    }

    pub fn with_reads<I>(reads: I) -> Self
    where
        I: IntoIterator<Item = MemoryRead>,
    {
        Self {
            written: Vec::new(),
            reads: reads.into_iter().collect(),
        }
    }
}

impl Transport for Memory {
    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.written.extend_from_slice(data);
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        loop {
            match self.reads.front_mut() {
                None => return Ok(0),
                Some(MemoryRead::Interrupt) => {
                    self.reads.pop_front();
                    return Err(io::Error::new(ErrorKind::Interrupted, "test interrupt").into());
                }
                Some(MemoryRead::Bytes(chunk)) => {
                    if chunk.is_empty() {
                        self.reads.pop_front();
                        continue;
                    }
                    let n = chunk.len().min(buf.len());
                    buf[..n].copy_from_slice(&chunk[..n]);
                    chunk.drain(..n);
                    if chunk.is_empty() {
                        self.reads.pop_front();
                    }
                    return Ok(n);
                }
            }
        }
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

    #[test]
    fn segments_cap_a_single_read() {
        let mut mem = Memory::with_segments([b"ab".to_vec(), b"cd".to_vec()]);
        let mut buf = [0u8; 8];
        assert_eq!(mem.read(&mut buf).unwrap(), 2);
        assert_eq!(&buf[..2], b"ab");
        assert_eq!(mem.read(&mut buf).unwrap(), 2);
        assert_eq!(&buf[..2], b"cd");
        assert_eq!(mem.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn interrupt_is_a_distinct_read() {
        let mut mem = Memory::with_reads([MemoryRead::Interrupt, MemoryRead::Bytes(b"x".to_vec())]);
        match mem.read(&mut [0u8; 1]) {
            Err(crate::error::Error::Io(e)) if e.kind() == ErrorKind::Interrupted => {}
            other => panic!("expected Interrupted, got {other:?}"),
        }
        let mut buf = [0u8; 1];
        assert_eq!(mem.read(&mut buf).unwrap(), 1);
        assert_eq!(buf[0], b'x');
    }
}
