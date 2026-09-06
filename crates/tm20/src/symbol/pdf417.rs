//! PDF417 (`GS ( k` cn=48).
//!
//! TM-T20III citations (Wayback of download4.epson.biz ESC/POS pages):
//! - Columns fn=065: n = 0..=30; 0 = automatic.
//!   <https://web.archive.org/web/20241006010305/https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lparen_lk_fn065.html>
//! - Rows fn=066: n = 0 or 3..=90; 0 = automatic.
//!   <https://web.archive.org/web/20250207011456/https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lparen_lk_fn066.html>
//! - Module width fn=067: T20III n = 1..=8. Zero is not legal on this model.
//!   <https://web.archive.org/web/20241005230846/https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lparen_lk_fn067.html>
//! - Row height fn=068: n = 2..=8. Zero is not legal.
//!   <https://web.archive.org/web/20250211143457/https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lparen_lk_fn068.html>

use crate::error::EncodeError;

use super::packet::gs_k;

const PDF_CN: u8 = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pdf417Kind {
    Standard,
    Truncated,
}

impl Pdf417Kind {
    fn byte(self) -> u8 {
        match self {
            Pdf417Kind::Standard => 0,
            Pdf417Kind::Truncated => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pdf417Ecc {
    Level(u8),
    Ratio(u8),
}

impl Pdf417Ecc {
    fn bytes(self) -> Result<(u8, u8), EncodeError> {
        match self {
            Pdf417Ecc::Level(n) if n <= 8 => Ok((48, 48 + n)),
            Pdf417Ecc::Ratio(n) if (1..=40).contains(&n) => Ok((49, n)),
            Pdf417Ecc::Level(n) | Pdf417Ecc::Ratio(n) => Err(EncodeError::Pdf417CorrectionRatio(n)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pdf417 {
    pub data: String,
    pub columns: u8,
    pub rows: u8,
    pub width: u8,
    pub row_height: u8,
    pub kind: Pdf417Kind,
    pub ecc: Pdf417Ecc,
}

impl Default for Pdf417 {
    fn default() -> Self {
        Self {
            data: String::new(),
            columns: 0,
            rows: 0,
            width: 3,
            row_height: 3,
            kind: Pdf417Kind::Standard,
            ecc: Pdf417Ecc::Ratio(1),
        }
    }
}

struct Pdf417Payload {
    columns: u8,
    rows: u8,
    width: u8,
    row_height: u8,
    kind: u8,
    ecc: (u8, u8),
    store: Vec<u8>,
}

fn admit(code: &Pdf417) -> Result<Pdf417Payload, EncodeError> {
    // fn=080: p=4..=65535, k=p-3. These are store limits, not symbol capacity.
    // https://web.archive.org/web/20250207015146/https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lparen_lk_fn080.html
    if !(1..=65532).contains(&code.data.len()) {
        return Err(EncodeError::SymbolDataLength {
            symbol: "PDF417",
            len: code.data.len(),
            max: 65532,
        });
    }
    if code.columns > 30 {
        return Err(EncodeError::Pdf417Columns(code.columns));
    }
    if code.rows != 0 && !(3..=90).contains(&code.rows) {
        return Err(EncodeError::Pdf417Rows(code.rows));
    }
    if !(1..=8).contains(&code.width) {
        return Err(EncodeError::Pdf417Width(code.width));
    }
    if !(2..=8).contains(&code.row_height) {
        return Err(EncodeError::Pdf417RowHeight(code.row_height));
    }
    let ecc = code.ecc.bytes()?;
    let mut store = Vec::new();
    store.push(48);
    store.extend_from_slice(code.data.as_bytes());
    Ok(Pdf417Payload {
        columns: code.columns,
        rows: code.rows,
        width: code.width,
        row_height: code.row_height,
        kind: code.kind.byte(),
        ecc,
        store,
    })
}

pub fn encode_pdf417(code: &Pdf417) -> Result<Vec<u8>, EncodeError> {
    let payload = admit(code)?;
    let mut out = Vec::new();
    out.extend(gs_k(PDF_CN, 65, &[payload.columns])?);
    out.extend(gs_k(PDF_CN, 66, &[payload.rows])?);
    out.extend(gs_k(PDF_CN, 67, &[payload.width])?);
    out.extend(gs_k(PDF_CN, 68, &[payload.row_height])?);
    out.extend(gs_k(PDF_CN, 69, &[payload.ecc.0, payload.ecc.1])?);
    out.extend(gs_k(PDF_CN, 70, &[payload.kind])?);
    out.extend(gs_k(PDF_CN, 80, &payload.store)?);
    out.extend(gs_k(PDF_CN, 81, &[48])?);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf417_rejects_bad_columns() {
        let mut code = Pdf417 {
            data: "hi".into(),
            ..Pdf417::default()
        };
        code.columns = 31;
        assert!(matches!(
            encode_pdf417(&code),
            Err(EncodeError::Pdf417Columns(31))
        ));
    }

    #[test]
    fn pdf417_zero_settings_follow_t20iii() {
        let mut code = Pdf417 {
            data: "hi".into(),
            ..Pdf417::default()
        };
        // Columns/rows 0 = automatic (fn=065 / fn=066).
        code.columns = 0;
        code.rows = 0;
        assert!(encode_pdf417(&code).is_ok());
        // Width 0 is not in the T20III fn=067 range (1..=8); 1 is.
        code.width = 0;
        assert!(matches!(
            encode_pdf417(&code),
            Err(EncodeError::Pdf417Width(0))
        ));
        code.width = 1;
        assert!(encode_pdf417(&code).is_ok());
        // Row height 0 is not in fn=068 (2..=8).
        code.row_height = 0;
        assert!(matches!(
            encode_pdf417(&code),
            Err(EncodeError::Pdf417RowHeight(0))
        ));
        code.row_height = 2;
        assert!(encode_pdf417(&code).is_ok());
    }
}
