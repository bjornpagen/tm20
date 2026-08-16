//! Byte sink. Protocol is [`crate::encode`]; this is I/O.

use crate::error::Result;

pub trait Transport {
    fn write(&mut self, data: &[u8]) -> Result<()>;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
}
