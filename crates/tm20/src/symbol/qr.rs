//! QR Code (`GS ( k` cn=49).
//!
//! TM-T20III citations (Wayback of download4.epson.biz ESC/POS pages):
//! - Model fn=165: n1 = 49 or 50 (Model 1 / Model 2). n2 = 0. Micro (51) is
//!   not in the T20III range; admission rejects that authoring variant.
//!   <https://web.archive.org/web/20250207010857/https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lparen_lk_fn165.html>
//! - Module size fn=167: n = 1..=16. Zero is not legal.
//!   <https://web.archive.org/web/20250317141415/https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lparen_lk_fn167.html>
//! - Store fn=180: `(pL + pH × 256) = 4..=7092`, so k = 1..=7089.
//!   <https://web.archive.org/web/20250207011943/https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lparen_lk_fn180.html>

use crate::error::EncodeError;

use super::packet::gs_k;

const QR_CN: u8 = 49;
const QR_MAX: usize = 7089;

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

/// Validated QR store body (`m` + data). Built once for emission.
struct QrPayload {
    model: u8,
    size: u8,
    ecc: u8,
    store: Vec<u8>,
}

fn admit(qr: &Qr) -> Result<QrPayload, EncodeError> {
    if qr.model == QrModel::Micro {
        return Err(EncodeError::QrModel(51));
    }
    let len = qr.data.len();
    if !(1..=QR_MAX).contains(&len) {
        return Err(EncodeError::QrTooLong { len });
    }
    if !(1..=16).contains(&qr.size) {
        return Err(EncodeError::QrSize(qr.size));
    }
    let mut store = Vec::new();
    store.push(48);
    store.extend_from_slice(qr.data.as_bytes());
    Ok(QrPayload {
        model: qr.model.byte(),
        size: qr.size,
        ecc: qr.ecc.byte(),
        store,
    })
}

pub fn encode_qr(qr: &Qr) -> Result<Vec<u8>, EncodeError> {
    let payload = admit(qr)?;
    let mut out = Vec::new();
    out.extend(gs_k(QR_CN, 65, &[payload.model, 0])?);
    out.extend(gs_k(QR_CN, 67, &[payload.size])?);
    out.extend(gs_k(QR_CN, 69, &[payload.ecc])?);
    out.extend(gs_k(QR_CN, 80, &payload.store)?);
    out.extend(gs_k(QR_CN, 81, &[48])?);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn micro_is_not_a_t20iii_model() {
        let qr = Qr {
            data: "A".into(),
            model: QrModel::Micro,
            ..Qr::default()
        };
        assert!(matches!(encode_qr(&qr), Err(EncodeError::QrModel(51))));
    }

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
    fn qr_rejects_empty_and_zero_module() {
        // fn=180 minimum (pL+pH×256)=4 ⇒ k≥1.
        assert!(matches!(
            encode_qr(&Qr::default()),
            Err(EncodeError::QrTooLong { len: 0 })
        ));
        // T20III fn=167: n = 1..=16. Zero is not a legal auto size.
        let mut qr = Qr {
            data: "A".into(),
            ..Qr::default()
        };
        qr.size = 0;
        assert!(matches!(encode_qr(&qr), Err(EncodeError::QrSize(0))));
        qr.size = 1;
        assert!(encode_qr(&qr).is_ok());
        qr.size = 16;
        assert!(encode_qr(&qr).is_ok());
        qr.size = 17;
        assert!(matches!(encode_qr(&qr), Err(EncodeError::QrSize(17))));
    }
}
