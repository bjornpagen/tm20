//! 2D symbols: validation and `GS ( k` bytes.

use crate::error::EncodeError;

const QR_MAX: usize = 7089;

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
pub enum SymbolKind {
    Qr,
    Pdf417,
    Gs1DataBar,
    MaxiCode,
    DataMatrix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrModel {
    Model1,
    Model2,
    Micro,
}

impl QrModel {
    fn byte(self) -> u8 {
        match self {
            QrModel::Model1 => 49,
            QrModel::Model2 => 50,
            QrModel::Micro => 51,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrEcc {
    L,
    M,
    Q,
    H,
}

impl QrEcc {
    fn byte(self) -> u8 {
        match self {
            QrEcc::L => 48,
            QrEcc::M => 49,
            QrEcc::Q => 50,
            QrEcc::H => 51,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Qr {
    pub data: String,
    pub model: QrModel,
    pub size: u8,
    pub ecc: QrEcc,
}

impl Default for Qr {
    fn default() -> Self {
        Self {
            data: String::new(),
            model: QrModel::Model2,
            size: 4,
            ecc: QrEcc::M,
        }
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gs1DataBarType {
    Stacked,
    StackedOmnidirectional,
    ExpandedStacked,
}

impl Gs1DataBarType {
    fn byte(self) -> u8 {
        match self {
            Gs1DataBarType::Stacked => 72,
            Gs1DataBarType::StackedOmnidirectional => 73,
            Gs1DataBarType::ExpandedStacked => 76,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gs1DataBarWidth {
    S,
    M,
    L,
}

impl Gs1DataBarWidth {
    fn byte(self) -> u8 {
        match self {
            Gs1DataBarWidth::S => 2,
            Gs1DataBarWidth::M => 1,
            Gs1DataBarWidth::L => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gs1DataBar {
    pub data: String,
    pub width: Gs1DataBarWidth,
    pub kind: Gs1DataBarType,
}

impl Default for Gs1DataBar {
    fn default() -> Self {
        Self {
            data: String::new(),
            width: Gs1DataBarWidth::M,
            kind: Gs1DataBarType::Stacked,
        }
    }
}

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

fn ph_pl(data_len: usize, extra: usize) -> Result<(u8, u8), EncodeError> {
    let n = data_len.saturating_add(extra);
    if n > 65535 {
        return Err(EncodeError::Gs2dTooLong { len: n });
    }
    Ok(((n % 256) as u8, (n / 256) as u8))
}

pub fn encode_qr(qr: &Qr) -> Result<Vec<u8>, EncodeError> {
    if qr.data.len() > QR_MAX {
        return Err(EncodeError::QrTooLong { len: qr.data.len() });
    }
    if qr.size > 16 {
        return Err(EncodeError::QrSize(qr.size));
    }
    let (pl, ph) = ph_pl(qr.data.len(), 3)?;
    let mut out = Vec::new();
    out.extend_from_slice(&[0x1d, b'(', b'k', 4, 0, 49, 65, qr.model.byte(), 0]);
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 49, 67, qr.size]);
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 49, 69, qr.ecc.byte()]);
    out.extend_from_slice(&[0x1d, b'(', b'k', pl, ph, 49, 80, 48]);
    out.extend_from_slice(qr.data.as_bytes());
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 49, 81, 48]);
    Ok(out)
}

pub fn encode_pdf417(code: &Pdf417) -> Result<Vec<u8>, EncodeError> {
    if code.columns > 30 {
        return Err(EncodeError::Pdf417Columns(code.columns));
    }
    if code.rows != 0 && !(3..=90).contains(&code.rows) {
        return Err(EncodeError::Pdf417Rows(code.rows));
    }
    if code.width != 0 && !(2..=8).contains(&code.width) {
        return Err(EncodeError::Pdf417Width(code.width));
    }
    if code.row_height != 0 && !(2..=8).contains(&code.row_height) {
        return Err(EncodeError::Pdf417RowHeight(code.row_height));
    }
    let (ecc_m, ecc_n) = code.ecc.bytes()?;
    let (pl, ph) = ph_pl(code.data.len(), 3)?;
    let mut out = Vec::new();
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 48, 65, code.columns]);
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 48, 66, code.rows]);
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 48, 67, code.width]);
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 48, 68, code.row_height]);
    out.extend_from_slice(&[0x1d, b'(', b'k', 4, 0, 48, 69, ecc_m, ecc_n]);
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 48, 70, code.kind.byte()]);
    out.extend_from_slice(&[0x1d, b'(', b'k', pl, ph, 48, 80, 48]);
    out.extend_from_slice(code.data.as_bytes());
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 48, 81, 48]);
    Ok(out)
}

pub fn encode_gs1(code: &Gs1DataBar) -> Result<Vec<u8>, EncodeError> {
    if code.data.is_empty() {
        return Err(EncodeError::Gs1DataEmpty);
    }
    let (pl, ph) = ph_pl(code.data.len(), 4)?;
    let mut out = Vec::new();
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 51, 67, code.width.byte()]);
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 51, 71, 0, 0]);
    out.extend_from_slice(&[0x1d, b'(', b'k', pl, ph, 51, 80, 48, code.kind.byte()]);
    out.extend_from_slice(code.data.as_bytes());
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 51, 81, 48]);
    Ok(out)
}

pub fn encode_maxi(code: &MaxiCode) -> Result<Vec<u8>, EncodeError> {
    let (pl, ph) = ph_pl(code.data.len(), 3)?;
    let mut out = Vec::new();
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 50, 65, code.mode.byte()]);
    out.extend_from_slice(&[0x1d, b'(', b'k', pl, ph, 50, 80, 48]);
    out.extend_from_slice(code.data.as_bytes());
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 50, 81, 48]);
    Ok(out)
}

pub fn encode_data_matrix(code: &DataMatrix) -> Result<Vec<u8>, EncodeError> {
    if !(2..=16).contains(&code.size) {
        return Err(EncodeError::DataMatrixSize(code.size));
    }
    let (m, d1, d2) = code.kind.triple()?;
    let (pl, ph) = ph_pl(code.data.len(), 3)?;
    let mut out = Vec::new();
    out.extend_from_slice(&[0x1d, b'(', b'k', 5, 0, 54, 66, m, d1, d2]);
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 54, 67, code.size]);
    out.extend_from_slice(&[0x1d, b'(', b'k', pl, ph, 54, 80, 48]);
    out.extend_from_slice(code.data.as_bytes());
    out.extend_from_slice(&[0x1d, b'(', b'k', 3, 0, 54, 81, 48]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_option_bytes() {
        let bytes = encode_qr(&Qr {
            data: "HELLO".into(),
            model: QrModel::Model2,
            size: 4,
            ecc: QrEcc::M,
        })
        .unwrap();
        assert!(
            bytes
                .windows(9)
                .any(|w| w == [0x1d, b'(', b'k', 4, 0, 49, 65, 50, 0])
        );
        assert!(
            bytes
                .windows(8)
                .any(|w| w == [0x1d, b'(', b'k', 3, 0, 49, 67, 4])
        );
        assert!(
            bytes
                .windows(8)
                .any(|w| w == [0x1d, b'(', b'k', 3, 0, 49, 69, 49])
        );
        assert!(
            bytes
                .windows(8)
                .any(|w| w == [0x1d, b'(', b'k', 8, 0, 49, 80, 48])
        );
        assert!(bytes.windows(5).any(|w| w == b"HELLO"));
        assert!(bytes.ends_with(&[0x1d, b'(', b'k', 3, 0, 49, 81, 48]));
    }

    #[test]
    fn qr_too_long() {
        let qr = Qr {
            data: "x".repeat(QR_MAX + 1),
            ..Qr::default()
        };
        assert!(matches!(encode_qr(&qr), Err(EncodeError::QrTooLong { .. })));
    }

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
}
