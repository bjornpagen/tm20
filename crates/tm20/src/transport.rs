//! Byte sink. Protocol is [`crate::encode()`]; this is I/O.
//!
//! [`Transport::read`] may return a short count. A zero count is end-of-stream.
//! `ErrorKind::Interrupted` is returned to the caller so
//! [`crate::reply::ReplyReader`] can retry that read. [`Transport::write`]
//! delivers the slice once; a failed or uncertain write is not turned into a
//! second job.

use crate::error::Result;

pub trait Transport {
    fn write(&mut self, data: &[u8]) -> Result<()>;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
}
