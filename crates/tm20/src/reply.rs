//! Persistent unread remainder for length- and NUL-delimited printer replies.

use std::io::ErrorKind;

use crate::error::{Error, FramingError, Result, UsbError};
use crate::identify::parse_process_id;
use crate::transport::Transport;

/// Owns a transport and any bytes already read past a completed frame.
pub struct ReplyReader<T: Transport> {
    transport: T,
    unread: Vec<u8>,
}

impl<T: Transport> ReplyReader<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            unread: Vec::new(),
        }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn into_inner(self) -> T {
        self.transport
    }

    /// Read exactly `n` bytes, retaining any coalesced tail for later frames.
    pub fn read_exact_reply(&mut self, n: usize) -> Result<Vec<u8>> {
        while self.unread.len() < n {
            if self.pull()? == 0 {
                return Err(FramingError::UnexpectedEof.into());
            }
        }
        Ok(self.unread.drain(..n).collect())
    }

    /// Read through a NUL, bounded by `max` bytes examined (including the NUL).
    /// The returned payload excludes the terminator; bytes after it stay unread.
    ///
    /// `Overlength.got` is the observed buffered length when the bound is
    /// exceeded — the unread remainder after `pull`, not the `max` threshold
    /// and not a transport chunk size. A 10-byte no-NUL payload with `max = 4`
    /// therefore reports `got = 10`.
    pub fn read_nul_reply(&mut self, max: usize) -> Result<Vec<u8>> {
        loop {
            if let Some(i) = self.unread.iter().position(|&b| b == 0) {
                if i >= max {
                    return Err(FramingError::Overlength { max, got: i }.into());
                }
                let mut frame: Vec<u8> = self.unread.drain(..=i).collect();
                frame.pop();
                return Ok(frame);
            }
            if self.unread.len() >= max {
                return Err(FramingError::Overlength {
                    max,
                    got: self.unread.len(),
                }
                .into());
            }
            if self.pull()? == 0 {
                return Err(FramingError::UnexpectedEof.into());
            }
        }
    }

    /// Read the seven-byte `GS ( H` reply and require its ID to equal `requested`.
    pub fn read_process_completion(&mut self, requested: [u8; 4]) -> Result<[u8; 4]> {
        let buf = self.read_exact_reply(7)?;
        let got = parse_process_id(&buf)?;
        if got != requested {
            return Err(FramingError::WrongId { requested, got }.into());
        }
        Ok(got)
    }

    fn pull(&mut self) -> Result<usize> {
        let mut buf = [0u8; 256];
        loop {
            match self.transport.read(&mut buf) {
                Ok(n) => {
                    if n > 0 {
                        self.unread.extend_from_slice(&buf[..n]);
                    }
                    return Ok(n);
                }
                Err(e) if interrupted(&e) => {}
                Err(e) => return Err(e),
            }
        }
    }
}

fn interrupted(err: &Error) -> bool {
    match err {
        Error::Io(e) => e.kind() == ErrorKind::Interrupted,
        Error::Usb(UsbError::Transfer(e)) => e.kind() == ErrorKind::Interrupted,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::IdentifyError;
    use crate::identify::{InfoRequest, encode_info, query_info};
    use crate::memory::{Memory, MemoryRead};

    const NAME_REPLY: &[u8] = b"_TM-T20III\0";
    const COMPLETION: [u8; 7] = [0x37, 0x22, b't', b'm', b'2', b'0', 0];

    fn partitions(data: &[u8]) -> Vec<Vec<Vec<u8>>> {
        let n = data.len();
        if n == 0 {
            return vec![Vec::new()];
        }
        let mut out = Vec::with_capacity(1 << (n - 1));
        for mask in 0..(1usize << (n - 1)) {
            let mut segs = Vec::new();
            let mut start = 0;
            for i in 0..(n - 1) {
                if mask & (1 << i) != 0 {
                    segs.push(data[start..=i].to_vec());
                    start = i + 1;
                }
            }
            segs.push(data[start..].to_vec());
            out.push(segs);
        }
        out
    }

    #[test]
    fn nul_reply_invariant_under_every_partition() {
        for segs in partitions(NAME_REPLY) {
            let mut reader = ReplyReader::new(Memory::with_segments(segs));
            let got = reader.read_nul_reply(80).unwrap();
            assert_eq!(got, b"_TM-T20III");
            assert!(reader.unread.is_empty());
        }
    }

    #[test]
    fn completion_invariant_under_every_partition() {
        for segs in partitions(&COMPLETION) {
            let mut reader = ReplyReader::new(Memory::with_segments(segs));
            let id = reader.read_process_completion(*b"tm20").unwrap();
            assert_eq!(id, *b"tm20");
        }
    }

    #[test]
    fn coalesced_nul_replies_are_separately_readable() {
        let mut reader = ReplyReader::new(Memory::with_replies(b"one\0two\0".to_vec()));
        assert_eq!(reader.read_nul_reply(16).unwrap(), b"one");
        assert_eq!(reader.read_nul_reply(16).unwrap(), b"two");
    }

    #[test]
    fn coalesced_completions_are_separately_readable() {
        let mut both = COMPLETION.to_vec();
        both.extend_from_slice(&[0x37, 0x22, b'x', b'x', b'x', b'x', 0]);
        let mut reader = ReplyReader::new(Memory::with_replies(both));
        assert_eq!(reader.read_process_completion(*b"tm20").unwrap(), *b"tm20");
        assert_eq!(reader.read_process_completion(*b"xxxx").unwrap(), *b"xxxx");
    }

    #[test]
    fn incomplete_nul_at_eof_is_unexpected_eof() {
        let mut reader = ReplyReader::new(Memory::with_replies(b"abcd".to_vec()));
        match reader.read_nul_reply(80) {
            Err(Error::Framing(FramingError::UnexpectedEof)) => {}
            other => panic!("expected UnexpectedEof, got {other:?}"),
        }
    }

    #[test]
    fn incomplete_exact_at_eof_is_unexpected_eof() {
        let mut reader = ReplyReader::new(Memory::with_replies(b"12".to_vec()));
        match reader.read_exact_reply(7) {
            Err(Error::Framing(FramingError::UnexpectedEof)) => {}
            other => panic!("expected UnexpectedEof, got {other:?}"),
        }
    }

    #[test]
    fn overlong_nul_frame_errors() {
        let mut reader = ReplyReader::new(Memory::with_replies(b"0123456789".to_vec()));
        match reader.read_nul_reply(4) {
            Err(Error::Framing(FramingError::Overlength { max: 4, got: 10 })) => {}
            other => panic!("expected Overlength {{ max: 4, got: 10 }}, got {other:?}"),
        }
    }

    #[test]
    fn wrong_completion_id_errors() {
        let mut reader = ReplyReader::new(Memory::with_replies(COMPLETION.to_vec()));
        match reader.read_process_completion(*b"xxxx") {
            Err(Error::Framing(FramingError::WrongId {
                requested,
                got: [b't', b'm', b'2', b'0'],
            })) if requested == *b"xxxx" => {}
            other => panic!("expected WrongId, got {other:?}"),
        }
    }

    #[test]
    fn wrong_completion_kind_is_not_scanned() {
        let mut reader = ReplyReader::new(Memory::with_replies(vec![0, 1, 2, 3, 4, 5, 6]));
        match reader.read_process_completion(*b"tm20") {
            Err(Error::Identify(IdentifyError::Unexpected { got })) => {
                assert_eq!(got, vec![0, 1, 2, 3, 4, 5, 6]);
            }
            other => panic!("expected Identify Unexpected, got {other:?}"),
        }
    }

    #[test]
    fn interrupted_then_data_completes() {
        let mut reader = ReplyReader::new(Memory::with_reads([
            MemoryRead::Interrupt,
            MemoryRead::Bytes(b"tm20\0".to_vec()),
        ]));
        assert_eq!(reader.read_nul_reply(16).unwrap(), b"tm20");
    }

    #[test]
    fn frame_helpers_do_not_write() {
        let mut reader = ReplyReader::new(Memory::with_replies(COMPLETION.to_vec()));
        reader.read_process_completion(*b"tm20").unwrap();
        assert!(reader.transport().written.is_empty());
    }

    #[test]
    fn query_info_writes_once_for_segmented_reply() {
        let mut reader = ReplyReader::new(Memory::with_segments([
            b"_TM-".to_vec(),
            b"T20III\0".to_vec(),
        ]));
        let name = query_info(&mut reader, InfoRequest::Name).unwrap();
        assert_eq!(name, b"TM-T20III");
        assert_eq!(reader.transport().written, encode_info(InfoRequest::Name));
    }

    #[test]
    fn query_info_eof_does_not_resend() {
        let mut reader = ReplyReader::new(Memory::new());
        match query_info(&mut reader, InfoRequest::Name) {
            Err(Error::Framing(FramingError::UnexpectedEof)) => {}
            other => panic!("expected UnexpectedEof, got {other:?}"),
        }
        assert_eq!(reader.transport().written, encode_info(InfoRequest::Name));
    }
}
