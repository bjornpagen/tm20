//! `GS I` printer identity and `GS ( H` process ID.

use crate::error::IdentifyError;
use crate::transport::Transport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoRequest {
    ModelId,
    TypeId,
    VersionId,
    Firmware,
    Manufacturer,
    Name,
    Serial,
    Fonts,
}

impl InfoRequest {
    pub fn n(self) -> u8 {
        match self {
            InfoRequest::ModelId => 1,
            InfoRequest::TypeId => 2,
            InfoRequest::VersionId => 3,
            InfoRequest::Firmware => 65,
            InfoRequest::Manufacturer => 66,
            InfoRequest::Name => 67,
            InfoRequest::Serial => 68,
            InfoRequest::Fonts => 69,
        }
    }

    pub fn is_byte(self) -> bool {
        matches!(
            self,
            InfoRequest::ModelId | InfoRequest::TypeId | InfoRequest::VersionId
        )
    }
}

pub fn encode_info(request: InfoRequest) -> [u8; 3] {
    [0x1d, b'I', request.n()]
}

pub fn encode_process_id(id: [u8; 4]) -> [u8; 11] {
    [0x1d, b'(', b'H', 6, 0, 48, 48, id[0], id[1], id[2], id[3]]
}

pub fn parse_process_id(buf: &[u8]) -> std::result::Result<[u8; 4], IdentifyError> {
    match buf {
        [0x37, 0x22, a, b, c, d, 0x00] => Ok([*a, *b, *c, *d]),
        _ => Err(IdentifyError::Unexpected { got: buf.to_vec() }),
    }
}

pub fn read_until_nul<T: Transport>(t: &mut T, max: usize) -> crate::error::Result<Vec<u8>> {
    let mut buf = vec![0u8; max];
    let n = t.read(&mut buf)?;
    buf.truncate(n);
    match buf.iter().position(|&b| b == 0) {
        Some(i) => Ok(buf[..i].to_vec()),
        None => Err(IdentifyError::Unexpected { got: buf }.into()),
    }
}

pub fn query_info<T: Transport>(t: &mut T, request: InfoRequest) -> crate::error::Result<Vec<u8>> {
    t.write(&encode_info(request))?;
    if request.is_byte() {
        let mut buf = [0u8; 1];
        let n = t.read(&mut buf)?;
        if n != 1 {
            return Err(IdentifyError::Unexpected {
                got: buf[..n].to_vec(),
            }
            .into());
        }
        return Ok(vec![buf[0]]);
    }
    let mut data = read_until_nul(t, 80)?;
    if data.first() == Some(&0x5f) {
        data.remove(0);
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_id_bytes() {
        assert_eq!(
            encode_process_id(*b"tm20"),
            [0x1d, b'(', b'H', 6, 0, 48, 48, b't', b'm', b'2', b'0']
        );
        assert_eq!(
            parse_process_id(&[0x37, 0x22, b't', b'm', b'2', b'0', 0]).unwrap(),
            *b"tm20"
        );
    }

    #[test]
    fn info_n() {
        assert_eq!(encode_info(InfoRequest::Firmware), [0x1d, b'I', 65]);
        assert_eq!(encode_info(InfoRequest::VersionId), [0x1d, b'I', 3]);
    }

    #[test]
    fn query_info_strips_underscore_and_nul() {
        let mut mem = crate::memory::Memory::with_replies(b"_TM-T20III\0".to_vec());
        let name = query_info(&mut mem, InfoRequest::Name).unwrap();
        assert_eq!(name, b"TM-T20III");
        assert_eq!(mem.written, encode_info(InfoRequest::Name));
    }

    #[test]
    fn query_info_byte_replies() {
        let mut mem = crate::memory::Memory::with_replies(vec![0x27]);
        let id = query_info(&mut mem, InfoRequest::ModelId).unwrap();
        assert_eq!(id, vec![0x27]);
    }
}
