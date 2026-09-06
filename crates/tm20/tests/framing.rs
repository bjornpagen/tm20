//! ReplyReader partitions, coalescing, EOF, overlength, Interrupted, wrong ID.

mod common;

use std::io::{Error, ErrorKind};

use tm20::{FramingError, Memory, ReplyReader, Transport};

use common::{Scripted, partitions};

const NAME: &[u8] = b"TM-T20III\0";
/// Production `GS ( H` completion: header `0x37 0x22`, four-byte id, NUL.
const DONE: &[u8] = &[0x37, 0x22, b't', b'm', b'2', b'0', 0];

#[test]
fn every_nul_partition_decodes_identically() {
    for chunks in partitions(NAME) {
        let reads = chunks.into_iter().map(Ok).collect();
        let mut reader = ReplyReader::new(Scripted::new(reads));
        let got = reader
            .read_nul_reply(32)
            .unwrap_or_else(|e| panic!("partition failed: {e}"));
        assert_eq!(got, b"TM-T20III");
    }
}

#[test]
fn every_completion_partition_decodes_identically() {
    let requested = *b"tm20";
    for chunks in partitions(DONE) {
        let reads = chunks.into_iter().map(Ok).collect();
        let mut reader = ReplyReader::new(Scripted::new(reads));
        let got = reader
            .read_process_completion(requested)
            .unwrap_or_else(|e| panic!("partition failed: {e}"));
        assert_eq!(got, requested);
    }
}

#[test]
fn coalesced_replies_are_separately_readable() {
    let mut both = NAME.to_vec();
    both.extend_from_slice(DONE);
    let mut reader = ReplyReader::new(Scripted::new(vec![Ok(both)]));
    assert_eq!(reader.read_nul_reply(32).unwrap(), b"TM-T20III");
    assert_eq!(reader.read_process_completion(*b"tm20").unwrap(), *b"tm20");
}

#[test]
fn incomplete_nul_at_eof_is_unexpected_eof() {
    let mut reader = ReplyReader::new(Scripted::new(vec![Ok(b"TM-".to_vec())]));
    let err = reader.read_nul_reply(32).unwrap_err();
    assert!(matches!(
        err,
        tm20::Error::Framing(FramingError::UnexpectedEof)
    ));
}

#[test]
fn overlong_nul_is_overlength() {
    let mut reader = ReplyReader::new(Scripted::new(vec![Ok(b"ABCDEFGHIJKLMNOP\0".to_vec())]));
    let err = reader.read_nul_reply(8).unwrap_err();
    assert!(matches!(
        err,
        tm20::Error::Framing(FramingError::Overlength { .. })
    ));
}

#[test]
fn interrupted_then_data_completes() {
    let mut reader = ReplyReader::new(Scripted::new(vec![
        Err(Error::new(ErrorKind::Interrupted, "eintr")),
        Ok(b"TM-".to_vec()),
        Err(Error::new(ErrorKind::Interrupted, "eintr")),
        Ok(b"T20III\0".to_vec()),
    ]));
    assert_eq!(reader.read_nul_reply(32).unwrap(), b"TM-T20III");
}

#[test]
fn wrong_completion_id_errors() {
    let mut reader = ReplyReader::new(Memory::with_replies(DONE.to_vec()));
    let err = reader.read_process_completion(*b"nope").unwrap_err();
    assert!(matches!(
        err,
        tm20::Error::Framing(FramingError::WrongId { .. })
    ));
}

#[test]
fn exact_reply_zero_before_completion_is_eof() {
    let mut reader = ReplyReader::new(Scripted::new(vec![Ok(b"ab".to_vec()), Ok(vec![])]));
    let err = reader.read_exact_reply(4).unwrap_err();
    assert!(matches!(
        err,
        tm20::Error::Framing(FramingError::UnexpectedEof)
    ));
}

#[test]
fn memory_accumulates_a_short_read() {
    let mut reader = ReplyReader::new(Memory::with_replies(b"pong".to_vec()));
    let got = reader.read_exact_reply(4).unwrap();
    assert_eq!(got, b"pong");
}

#[test]
fn write_is_not_resent_by_the_reader() {
    let mut reader = ReplyReader::new(Scripted::new(vec![Ok(NAME.to_vec())]));
    reader.transport_mut().write(b"query").unwrap();
    reader.read_nul_reply(32).unwrap();
    assert_eq!(reader.into_inner().written, b"query");
}
