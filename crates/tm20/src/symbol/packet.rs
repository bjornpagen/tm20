//! One `GS ( k` builder. Declared length is derived from the bytes after `pH`.
//!
//! Epson TM-T20III `GS ( k` format (Wayback 2025-02-07 of
//! <https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lparen_lk.html>):
//! `pL` and `pH` specify the number of bytes following `pH` as `(pL + pH × 256)`.
//! Body after `pH` is always `cn fn [parameters]`.

use crate::error::EncodeError;

const GS: u8 = 0x1d;

/// Build `GS ( k pL pH cn fn [params…]` with `pL`/`pH` taken from the actual
/// `cn + fn + params` length. Callers must not pass a hand-counted length.
pub(super) fn gs_k(cn: u8, fn_byte: u8, params: &[u8]) -> Result<Vec<u8>, EncodeError> {
    let after_ph = 2usize
        .checked_add(params.len())
        .ok_or(EncodeError::Gs2dTooLong { len: usize::MAX })?;
    if after_ph > 65535 {
        return Err(EncodeError::Gs2dTooLong { len: after_ph });
    }
    let pl = (after_ph & 0xff) as u8;
    let ph = (after_ph >> 8) as u8;
    let mut out = Vec::new();
    out.extend_from_slice(&[GS, b'(', b'k', pl, ph, cn, fn_byte]);
    out.extend_from_slice(params);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_length_matches_bytes_after_ph() {
        let bytes = gs_k(51, 71, &[0, 0]).unwrap();
        assert_eq!(bytes, [0x1d, b'(', b'k', 4, 0, 51, 71, 0, 0]);
        let after_ph = usize::from(bytes[3]) + (usize::from(bytes[4]) << 8);
        assert_eq!(after_ph, bytes.len() - 5);
    }

    #[test]
    fn rejects_body_over_u16() {
        let params = vec![0; 65534];
        assert!(matches!(
            gs_k(49, 80, &params),
            Err(EncodeError::Gs2dTooLong { len: 65536 })
        ));
    }
}
