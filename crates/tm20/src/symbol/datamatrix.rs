//! DataMatrix (`GS ( k` cn=54).
//!
//! TM-T20III TRG (files.support.epson.com `tm-t20iii_trg_en_reva.pdf`) lists
//! DataMatrix as supported. The `GS ( k` overview model blurb for TM-T20III
//! names PDF417 / QR / MaxiCode / GS1 DataBar / Composite and omits DataMatrix.
//! Model support remains unverified because those sources disagree.

use crate::error::EncodeError;

use super::packet::gs_k;

const DM_CN: u8 = 54;

const DM_SQUARE: &[u8] = &[
    0, 10, 12, 14, 16, 18, 20, 22, 24, 26, 32, 36, 40, 44, 48, 52, 64, 72, 80, 88, 96, 104, 120,
    132, 144,
];
const DM_RECT: &[(u8, u8)] = &[
    (8, 0),
    (8, 18),
    (8, 32),
    (12, 0),
    (12, 26),
    (12, 36),
    (16, 0),
    (16, 36),
    (16, 48),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataMatrixType {
    Square(u8),
    Rectangle { rows: u8, cols: u8 },
}

impl DataMatrixType {
    fn triple(self) -> Result<(u8, u8, u8), EncodeError> {
        match self {
            DataMatrixType::Square(d) => {
                if DM_SQUARE.contains(&d) {
                    Ok((0, d, d))
                } else {
                    Err(EncodeError::DataMatrixType { rows: d, cols: d })
                }
            }
            DataMatrixType::Rectangle { rows, cols } => {
                if DM_RECT.contains(&(rows, cols)) {
                    Ok((1, rows, cols))
                } else {
                    Err(EncodeError::DataMatrixType { rows, cols })
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataMatrix {
    pub data: String,
    pub kind: DataMatrixType,
    pub size: u8,
}

impl Default for DataMatrix {
    fn default() -> Self {
        Self {
            data: String::new(),
            kind: DataMatrixType::Square(0),
            size: 3,
        }
    }
}

pub fn encode_data_matrix(code: &DataMatrix) -> Result<Vec<u8>, EncodeError> {
    if !(2..=16).contains(&code.size) {
        return Err(EncodeError::DataMatrixSize(code.size));
    }
    let (m, d1, d2) = code.kind.triple()?;
    let mut store = Vec::new();
    store.push(48);
    store.extend_from_slice(code.data.as_bytes());
    let mut out = Vec::new();
    out.extend(gs_k(DM_CN, 66, &[m, d1, d2])?);
    out.extend(gs_k(DM_CN, 67, &[code.size])?);
    out.extend(gs_k(DM_CN, 80, &store)?);
    out.extend(gs_k(DM_CN, 81, &[48])?);
    Ok(out)
}
