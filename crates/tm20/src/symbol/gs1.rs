//! 2-dimensional GS1 DataBar (`GS ( k` cn=51).
//!
//! TM-T20III citations (Wayback of download4.epson.biz ESC/POS pages):
//! - Module width fn=367: n = 2..=8 (default 2).
//!   <https://web.archive.org/web/20250123164331/https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lparen_lk_fn367.html>
//! - Max width fn=371: `GS ( k 4 0 51 71 nL nH`. Hex `1D 28 6B 04 00 33 47 nL nH`.
//!   `(nL + nH × 256) = 0` or `106..=3952`. Zero means “not set” (use print area).
//!   <https://web.archive.org/web/20250324025350/https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lparen_lk_fn371.html>
//! - Store fn=380: Stacked/Omni k=13 digits; Omni d1 ∈ {0,1}; Expanded k via packet
//!   `(pL+pH×256)=6..=259` ⇒ k=2..=255 of the listed charset.
//!   <https://web.archive.org/web/20250115183820/https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lparen_lk_fn380.html>

use crate::error::EncodeError;

use super::packet::gs_k;

const GS1_CN: u8 = 51;

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
            // T20III fn=367 default n=2.
            width: Gs1DataBarWidth::S,
            kind: Gs1DataBarType::Stacked,
        }
    }
}

/// Validated GS1 store/option bytes. Built once; emission does not re-encode.
struct Gs1Payload {
    width: u8,
    store: Vec<u8>,
}

/// fn=380 Data (d) column: 32–34, 37–63, 65–90, 95, 97–122, 123.
/// The ASCII prose lists A–D / `$`; the numeric `d` range is the byte rule.
fn expanded_char(b: u8) -> bool {
    matches!(b, 32..=34 | 37..=63 | 65..=90 | 95 | 97..=122 | 123)
}

fn admit_data(code: &Gs1DataBar) -> Result<Vec<u8>, EncodeError> {
    let data = code.data.as_bytes();
    let ok = match code.kind {
        Gs1DataBarType::Stacked => data.len() == 13 && data.iter().all(u8::is_ascii_digit),
        Gs1DataBarType::StackedOmnidirectional => {
            data.len() == 13
                && data.iter().all(u8::is_ascii_digit)
                && matches!(data.first(), Some(&(b'0' | b'1')))
        }
        Gs1DataBarType::ExpandedStacked => {
            (2..=255).contains(&data.len())
                && data.iter().copied().all(expanded_char)
                && !injects_expanded_special(data)
        }
    };
    if !ok {
        return Err(EncodeError::Gs1Data {
            kind: code.kind,
            len: data.len(),
        });
    }
    Ok(data.to_vec())
}

/// Printer treats `{1`, `{(`, `{)` as FNC1 / HRI-only parens (fn=380). A plain
/// authoring string must not inject those controls.
fn injects_expanded_special(data: &[u8]) -> bool {
    data.windows(2)
        .any(|w| w[0] == b'{' && matches!(w[1], b'1' | b'(' | b')'))
}

fn admit(code: &Gs1DataBar) -> Result<Gs1Payload, EncodeError> {
    let width = code.width.byte();
    if !(2..=8).contains(&width) {
        return Err(EncodeError::Gs1ModuleWidth(width));
    }
    let data = admit_data(code)?;
    let mut store = Vec::new();
    store.push(48);
    store.push(code.kind.byte());
    store.extend_from_slice(&data);
    Ok(Gs1Payload { width, store })
}

pub fn encode_gs1(code: &Gs1DataBar) -> Result<Vec<u8>, EncodeError> {
    let payload = admit(code)?;
    let mut out = Vec::new();
    out.extend(gs_k(GS1_CN, 67, &[payload.width])?);
    // fn=371: nL=0, nH=0 ⇒ “maximum width is not set” (print-area width).
    out.extend(gs_k(GS1_CN, 71, &[0, 0])?);
    out.extend(gs_k(GS1_CN, 80, &payload.store)?);
    out.extend(gs_k(GS1_CN, 81, &[48])?);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    type GsKPacket = (u8, u8, Vec<u8>);

    /// Walk `GS ( k` packets; each declared length must equal the bytes after `pH`.
    fn walk_gs_k(bytes: &[u8]) -> Result<Vec<GsKPacket>, &'static str> {
        let mut i = 0;
        let mut packets = Vec::new();
        while i < bytes.len() {
            if i + 7 > bytes.len()
                || bytes[i] != 0x1d
                || bytes[i + 1] != b'('
                || bytes[i + 2] != b'k'
            {
                return Err("not a GS ( k command");
            }
            let after_ph = usize::from(bytes[i + 3]) + (usize::from(bytes[i + 4]) << 8);
            let start = i + 5;
            let end = start.checked_add(after_ph).ok_or("length overflow")?;
            if end > bytes.len() {
                return Err("declared length overruns the stream");
            }
            let body = &bytes[start..end];
            if body.len() < 2 {
                return Err("body shorter than cn fn");
            }
            packets.push((body[0], body[1], body[2..].to_vec()));
            i = end;
        }
        Ok(packets)
    }

    #[test]
    fn f15_fn71_length_matches_cited_body() {
        // Official fn=371: 1D 28 6B 04 00 33 47 nL nH. nL=nH=0 is legal.
        let bytes = encode_gs1(&Gs1DataBar {
            data: "1240123456789".into(),
            width: Gs1DataBarWidth::S,
            kind: Gs1DataBarType::Stacked,
        })
        .unwrap();
        let packets = walk_gs_k(&bytes).expect("stream is only GS ( k packets");
        assert_eq!(packets.len(), 4);
        assert_eq!(packets[0], (51, 67, vec![2]));
        assert_eq!(packets[1], (51, 71, vec![0, 0]));
        assert_eq!(
            packets[2],
            (51, 80, {
                let mut p = vec![48, 72];
                p.extend(b"1240123456789");
                p
            })
        );
        assert_eq!(packets[3], (51, 81, vec![48]));
        assert_eq!(
            &bytes[packets_offset_fn71(&bytes)],
            &[0x1d, b'(', b'k', 4, 0, 51, 71, 0, 0]
        );
    }

    fn packets_offset_fn71(bytes: &[u8]) -> std::ops::Range<usize> {
        let start = bytes
            .windows(8)
            .position(|w| w == [0x1d, b'(', b'k', 4, 0, 51, 71, 0])
            .expect("fn=71 packet");
        start..start + 9
    }

    #[test]
    fn stacked_requires_thirteen_digits() {
        let mut code = Gs1DataBar {
            data: "12401234567890".into(),
            width: Gs1DataBarWidth::S,
            kind: Gs1DataBarType::Stacked,
        };
        assert!(matches!(
            encode_gs1(&code),
            Err(EncodeError::Gs1Data { len: 14, .. })
        ));
        code.data = "1240123456789".into();
        assert!(encode_gs1(&code).is_ok());
        code.data = "124012345678".into();
        assert!(matches!(
            encode_gs1(&code),
            Err(EncodeError::Gs1Data { len: 12, .. })
        ));
    }

    #[test]
    fn omni_first_digit_is_zero_or_one() {
        let mut code = Gs1DataBar {
            data: "2001234567890".into(),
            width: Gs1DataBarWidth::S,
            kind: Gs1DataBarType::StackedOmnidirectional,
        };
        assert!(matches!(
            encode_gs1(&code),
            Err(EncodeError::Gs1Data { len: 13, .. })
        ));
        code.data = "0001234567890".into();
        assert!(encode_gs1(&code).is_ok());
    }

    #[test]
    fn expanded_rejects_control_injection_and_short_payload() {
        let mut code = Gs1DataBar {
            data: "x".into(),
            width: Gs1DataBarWidth::S,
            kind: Gs1DataBarType::ExpandedStacked,
        };
        assert!(matches!(
            encode_gs1(&code),
            Err(EncodeError::Gs1Data { len: 1, .. })
        ));
        code.data = "01".into();
        assert!(encode_gs1(&code).is_ok());
        code.data = "{1AB".into();
        assert!(matches!(
            encode_gs1(&code),
            Err(EncodeError::Gs1Data { len: 4, .. })
        ));
    }

    #[test]
    fn module_width_m_is_outside_t20iii_range() {
        let code = Gs1DataBar {
            data: "1240123456789".into(),
            width: Gs1DataBarWidth::M,
            kind: Gs1DataBarType::Stacked,
        };
        assert!(matches!(
            encode_gs1(&code),
            Err(EncodeError::Gs1ModuleWidth(1))
        ));
    }
}
