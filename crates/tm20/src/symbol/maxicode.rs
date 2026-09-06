//! MaxiCode (`GS ( k` cn=50). Variant retained; T20III lists MaxiCode as supported
//! (`GS ( k` model notes for TM-T20III).

use crate::error::EncodeError;

use super::packet::gs_k;

const MAXI_CN: u8 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxiCodeMode {
    Mode2,
    Mode3,
    Mode4,
    Mode5,
    Mode6,
}

impl MaxiCodeMode {
    fn byte(self) -> u8 {
        match self {
            MaxiCodeMode::Mode2 => 50,
            MaxiCodeMode::Mode3 => 51,
            MaxiCodeMode::Mode4 => 52,
            MaxiCodeMode::Mode5 => 53,
            MaxiCodeMode::Mode6 => 54,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaxiCode {
    pub data: String,
    pub mode: MaxiCodeMode,
}

impl Default for MaxiCode {
    fn default() -> Self {
        Self {
            data: String::new(),
            mode: MaxiCodeMode::Mode2,
        }
    }
}

pub fn encode_maxi(code: &MaxiCode) -> Result<Vec<u8>, EncodeError> {
    // fn=280: p=4..=141, k=p-3. Mode-specific printable capacity may be lower.
    // https://web.archive.org/web/20250207015146/https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lparen_lk_fn280.html
    if !(1..=138).contains(&code.data.len()) {
        return Err(EncodeError::SymbolDataLength {
            symbol: "MaxiCode",
            len: code.data.len(),
            max: 138,
        });
    }
    let mut store = Vec::new();
    store.push(48);
    store.extend_from_slice(code.data.as_bytes());
    let mut out = Vec::new();
    out.extend(gs_k(MAXI_CN, 65, &[code.mode.byte()])?);
    out.extend(gs_k(MAXI_CN, 80, &store)?);
    out.extend(gs_k(MAXI_CN, 81, &[48])?);
    Ok(out)
}
