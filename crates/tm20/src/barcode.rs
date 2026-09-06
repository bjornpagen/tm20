//! 1D barcodes: typed validated payloads and `GS k` function B bytes.
//!
//! Authoring [`Barcode`] is parsed once into an encoded payload; emission uses
//! that payload only.
//!
//! TM-T20III `GS k` (Wayback of
//! <https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lk.html>,
//! model list includes TM-T20III):
//! - Function B `n` is the encoded-byte count after escaping.
//! - CODABAR (m=71): n=2..=255; d1 and dn are A–D / a–d; those letters are not
//!   legal in the interior. Start/stop are not added by the printer.
//! - CODE128 (m=73): n=2..=255; d=0..=127; d1=123 (`{`), d2=65..=67 (`A`/`B`/`C`).
//!   Literal `{` and specials are sent as `7Bh` + character. This encoder
//!   accepts A = 00h..=5Fh, B = 20h..=7Fh, C = even ASCII digit pairs.
//!   C's wire representation still needs verification against Epson's
//!   per-code-set table. B escapes plain data so `{1` cannot inject FNC1.
//! - CODE128 / CODE93 / ITF / CODE39 length bounds as in the Function B table.
//!
//! `GS w` on TM-T20III: n = 2..=6
//! (<https://web.archive.org/web/20250213162224/https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lw.html>).

use crate::error::EncodeError;

const CODE39_VALID: &[u8] = b"0123456789$%*+-./ABCDEFGHIJKLMNOPQRSTUVWXYZ ";
const CODE93_VALID: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%+-./";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarcodeKind {
    UpcA,
    UpcE,
    Ean13,
    Ean8,
    Code39,
    Itf,
    Codabar,
    Code93,
    Code128 { set: Code128Set },
    Gs1_128,
}

/// CODE128 code set. Encoded as `{A` / `{B` / `{C` ahead of the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code128Set {
    A,
    B,
    C,
}

impl Code128Set {
    fn prefix(self) -> &'static [u8] {
        match self {
            Code128Set::A => b"{A",
            Code128Set::B => b"{B",
            Code128Set::C => b"{C",
        }
    }
}

impl BarcodeKind {
    fn function_b_byte(self) -> u8 {
        match self {
            BarcodeKind::UpcA => b'A',
            BarcodeKind::UpcE => b'B',
            BarcodeKind::Ean13 => b'C',
            BarcodeKind::Ean8 => b'D',
            BarcodeKind::Code39 => b'E',
            BarcodeKind::Itf => b'F',
            BarcodeKind::Codabar => b'G',
            BarcodeKind::Code93 => b'H',
            BarcodeKind::Code128 { .. } => b'I',
            BarcodeKind::Gs1_128 => b'J',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarcodeFont {
    A,
    B,
    C,
    D,
    E,
}

impl BarcodeFont {
    fn byte(self) -> u8 {
        match self {
            BarcodeFont::A => 0,
            BarcodeFont::B => 1,
            BarcodeFont::C => 2,
            BarcodeFont::D => 3,
            BarcodeFont::E => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HriPosition {
    None,
    Above,
    Below,
    Both,
}

impl HriPosition {
    fn byte(self) -> u8 {
        match self {
            HriPosition::None => 0,
            HriPosition::Above => 1,
            HriPosition::Below => 2,
            HriPosition::Both => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarcodeOptions {
    pub width: u8,
    pub height: u8,
    pub font: BarcodeFont,
    pub hri_position: HriPosition,
}

impl Default for BarcodeOptions {
    fn default() -> Self {
        Self {
            width: 3,
            height: 102,
            font: BarcodeFont::A,
            hri_position: HriPosition::Below,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Barcode {
    pub kind: BarcodeKind,
    pub data: String,
    pub options: BarcodeOptions,
}

/// Encoded Function B payload. This is the only input to emission.
struct ValidatedBarcode {
    kind: BarcodeKind,
    payload: Vec<u8>,
    options: BarcodeOptions,
}

fn is_digits(data: &str) -> bool {
    !data.is_empty() && data.bytes().all(|b| b.is_ascii_digit())
}

fn encode_code128(set: Code128Set, data: &str) -> Result<Vec<u8>, EncodeError> {
    let kind = BarcodeKind::Code128 { set };
    if data.is_empty() || !data.is_ascii() {
        return Err(EncodeError::BarcodeData { kind });
    }
    let mut body = Vec::new();
    match set {
        Code128Set::C => {
            if !data.len().is_multiple_of(2) || !data.bytes().all(|b| b.is_ascii_digit()) {
                return Err(EncodeError::BarcodeData { kind });
            }
            body.extend_from_slice(data.as_bytes());
        }
        Code128Set::A => {
            for b in data.bytes() {
                if b > 0x5f {
                    return Err(EncodeError::BarcodeData { kind });
                }
                body.push(b);
            }
        }
        Code128Set::B => {
            for b in data.bytes() {
                if !(0x20..=0x7f).contains(&b) {
                    return Err(EncodeError::BarcodeData { kind });
                }
                if b == b'{' {
                    body.extend_from_slice(b"{{");
                } else {
                    body.push(b);
                }
            }
        }
    }
    let mut out = set.prefix().to_vec();
    out.extend_from_slice(&body);
    if !(2..=255).contains(&out.len()) {
        return Err(EncodeError::BarcodeTooLong { len: out.len() });
    }
    Ok(out)
}

fn is_codabar_stop(b: u8) -> bool {
    matches!(b, b'A'..=b'D' | b'a'..=b'd')
}

fn is_codabar_body(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'$' | b'+' | b'-' | b'.' | b'/' | b':')
}

fn encode_codabar(data: &str) -> Result<Vec<u8>, EncodeError> {
    let kind = BarcodeKind::Codabar;
    if !data.is_ascii() {
        return Err(EncodeError::BarcodeData { kind });
    }
    let bytes = data.as_bytes();
    if !(2..=255).contains(&bytes.len()) {
        return Err(EncodeError::BarcodeData { kind });
    }
    let (first, last) = (bytes[0], bytes[bytes.len() - 1]);
    if !is_codabar_stop(first) || !is_codabar_stop(last) {
        return Err(EncodeError::BarcodeData { kind });
    }
    if !bytes[1..bytes.len() - 1]
        .iter()
        .copied()
        .all(is_codabar_body)
    {
        return Err(EncodeError::BarcodeData { kind });
    }
    Ok(bytes.to_vec())
}

fn encoded_payload(kind: BarcodeKind, data: &str) -> Result<Vec<u8>, EncodeError> {
    let payload = match kind {
        BarcodeKind::Code128 { set } => return encode_code128(set, data),
        BarcodeKind::Codabar => return encode_codabar(data),
        BarcodeKind::UpcA if is_digits(data) && matches!(data.len(), 11 | 12) => data.as_bytes(),
        BarcodeKind::UpcE
            if is_digits(data)
                && matches!(data.len(), 6 | 7 | 8 | 11 | 12)
                && (data.len() == 6 || data.starts_with('0')) =>
        {
            data.as_bytes()
        }
        BarcodeKind::Ean13 if is_digits(data) && matches!(data.len(), 12 | 13) => data.as_bytes(),
        BarcodeKind::Ean8 if is_digits(data) && matches!(data.len(), 7 | 8) => data.as_bytes(),
        BarcodeKind::Itf
            if is_digits(data)
                && (2..=254).contains(&data.len())
                && data.len().is_multiple_of(2) =>
        {
            data.as_bytes()
        }
        BarcodeKind::Code39
            if (1..=255).contains(&data.len())
                && data.is_ascii()
                && data.bytes().all(|b| CODE39_VALID.contains(&b)) =>
        {
            data.as_bytes()
        }
        BarcodeKind::Code93
            if (1..=255).contains(&data.len())
                && data.is_ascii()
                && data.bytes().all(|b| CODE93_VALID.contains(&b)) =>
        {
            data.as_bytes()
        }
        BarcodeKind::Gs1_128 if (2..=255).contains(&data.len()) && data.is_ascii() => {
            data.as_bytes()
        }
        _ => return Err(EncodeError::BarcodeData { kind }),
    };
    let n = payload.len();
    if n > 255 {
        return Err(EncodeError::BarcodeTooLong { len: n });
    }
    Ok(payload.to_vec())
}

fn admit_options(options: BarcodeOptions) -> Result<BarcodeOptions, EncodeError> {
    // TM-T20III GS f permits only A/B (0,1 or their ASCII equivalents).
    // https://web.archive.org/web/20250207015146/https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lf.html
    if !matches!(options.font, BarcodeFont::A | BarcodeFont::B) {
        return Err(EncodeError::HriFont(options.font.byte()));
    }
    if !(2..=6).contains(&options.width) {
        return Err(EncodeError::BarcodeWidth(options.width));
    }
    if options.height == 0 {
        return Err(EncodeError::BarcodeHeight(options.height));
    }
    Ok(options)
}

fn admit(barcode: &Barcode) -> Result<ValidatedBarcode, EncodeError> {
    let payload = encoded_payload(barcode.kind, &barcode.data)?;
    let options = admit_options(barcode.options)?;
    Ok(ValidatedBarcode {
        kind: barcode.kind,
        payload,
        options,
    })
}

pub fn validate(kind: BarcodeKind, data: &str) -> Result<(), EncodeError> {
    encoded_payload(kind, data).map(|_| ())
}

pub fn encode(barcode: &Barcode) -> Result<Vec<u8>, EncodeError> {
    let checked = admit(barcode)?;
    let n = checked.payload.len();
    debug_assert!(n <= 255);
    let mut out = Vec::new();
    out.extend_from_slice(&[0x1d, b'w', checked.options.width]);
    out.extend_from_slice(&[0x1d, b'h', checked.options.height]);
    out.extend_from_slice(&[0x1d, b'f', checked.options.font.byte()]);
    out.extend_from_slice(&[0x1d, b'H', checked.options.hri_position.byte()]);
    out.extend_from_slice(&[0x1d, b'k', checked.kind.function_b_byte(), n as u8]);
    out.extend_from_slice(&checked.payload);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ean13_length() {
        assert!(validate(BarcodeKind::Ean13, "5901234123457").is_ok());
        assert!(validate(BarcodeKind::Ean13, "590123412345").is_ok());
        assert!(validate(BarcodeKind::Ean13, "123").is_err());
        assert!(validate(BarcodeKind::Ean13, "590123412345a").is_err());
    }

    #[test]
    fn upc_a_length() {
        assert!(validate(BarcodeKind::UpcA, "01234567890").is_ok());
        assert!(validate(BarcodeKind::UpcA, "012345678901").is_ok());
        assert!(validate(BarcodeKind::UpcA, "123").is_err());
    }

    #[test]
    fn itf_even_digits() {
        assert!(validate(BarcodeKind::Itf, "1234").is_ok());
        assert!(validate(BarcodeKind::Itf, "123").is_err());
        assert!(validate(BarcodeKind::Itf, &"12".repeat(127)).is_ok());
        assert!(validate(BarcodeKind::Itf, &"12".repeat(128)).is_err());
    }

    #[test]
    fn remaining_kinds() {
        assert!(validate(BarcodeKind::Ean8, "96385074").is_ok());
        assert!(validate(BarcodeKind::UpcE, "012345").is_ok());
        assert!(validate(BarcodeKind::Code39, "TM20").is_ok());
        assert!(validate(BarcodeKind::Codabar, "A40156B").is_ok());
        assert!(validate(BarcodeKind::Code39, "nope!").is_err());
    }

    #[test]
    fn function_b_ean13() {
        let bytes = encode(&Barcode {
            kind: BarcodeKind::Ean13,
            data: "5901234123457".into(),
            options: BarcodeOptions::default(),
        })
        .unwrap();
        assert!(bytes.windows(4).any(|w| w == [0x1d, b'k', b'C', 13]));
        assert!(bytes.ends_with(b"5901234123457"));
        assert!(!bytes.ends_with(b"\0"));
    }

    #[test]
    fn code128_function_b_prefixes_set() {
        let bytes = encode(&Barcode {
            kind: BarcodeKind::Code128 { set: Code128Set::B },
            data: "TM20".into(),
            options: BarcodeOptions::default(),
        })
        .unwrap();
        assert!(bytes.windows(4).any(|w| w == [0x1d, b'k', b'I', 6]));
        assert!(bytes.ends_with(b"{BTM20"));
    }

    #[test]
    fn code93_function_b() {
        let bytes = encode(&Barcode {
            kind: BarcodeKind::Code93,
            data: "TM20".into(),
            options: BarcodeOptions::default(),
        })
        .unwrap();
        assert!(bytes.windows(4).any(|w| w == [0x1d, b'k', b'H', 4]));
        assert!(bytes.ends_with(b"TM20"));
    }

    #[test]
    fn code128_set_c_needs_even_digits() {
        let kind = BarcodeKind::Code128 { set: Code128Set::C };
        assert!(validate(kind, "1234").is_ok());
        assert!(validate(kind, "123").is_err());
        assert!(validate(kind, "12AB").is_err());
        let bytes = encode(&Barcode {
            kind,
            data: "1234".into(),
            options: BarcodeOptions::default(),
        })
        .unwrap();
        assert!(bytes.ends_with(b"{C1234"));
    }

    #[test]
    fn code128_set_b_escapes_literal_brace() {
        let kind = BarcodeKind::Code128 { set: Code128Set::B };
        assert!(validate(kind, "{").is_ok());
        let bytes = encode(&Barcode {
            kind,
            data: "{1}".into(),
            options: BarcodeOptions::default(),
        })
        .unwrap();
        // `{` → `{{`; `1` and `}` stay literal. Not FNC1.
        // Encoded Function B body is `{B{{1}` (6 bytes). n is that count.
        assert!(bytes.ends_with(b"{B{{1}"));
        assert!(bytes.windows(4).any(|w| w == [0x1d, b'k', b'I', 6]));
    }

    #[test]
    fn code128_set_a_rejects_lowercase() {
        let kind = BarcodeKind::Code128 { set: Code128Set::A };
        assert!(validate(kind, "ABC").is_ok());
        assert!(validate(kind, "abc").is_err());
        assert!(validate(kind, "{").is_err());
    }

    #[test]
    fn code128_length_counts_encoded_bytes() {
        let kind = BarcodeKind::Code128 { set: Code128Set::B };
        let max_plain = "A".repeat(253);
        assert!(validate(kind, &max_plain).is_ok());
        assert!(validate(kind, &"A".repeat(254)).is_err());
        let max_braces = "{".repeat(126);
        assert!(validate(kind, &max_braces).is_ok());
        assert!(validate(kind, &"{".repeat(127)).is_err());
    }

    #[test]
    fn codabar_requires_start_stop() {
        assert!(validate(BarcodeKind::Codabar, "12").is_err());
        assert!(validate(BarcodeKind::Codabar, "A40156B").is_ok());
        assert!(validate(BarcodeKind::Codabar, "AB").is_ok());
        assert!(validate(BarcodeKind::Codabar, "A1B2C").is_err());
        assert!(validate(BarcodeKind::Codabar, "a40156b").is_ok());
    }

    #[test]
    fn gs1_128() {
        assert!(validate(BarcodeKind::Gs1_128, "{1012345").is_ok());
        assert!(validate(BarcodeKind::Gs1_128, "x").is_err());
    }

    #[test]
    fn barcode_width_rejects_one() {
        let err = encode(&Barcode {
            kind: BarcodeKind::Ean13,
            data: "5901234123457".into(),
            options: BarcodeOptions {
                width: 1,
                ..BarcodeOptions::default()
            },
        })
        .unwrap_err();
        assert!(matches!(err, EncodeError::BarcodeWidth(1)));
        assert!(
            encode(&Barcode {
                kind: BarcodeKind::Ean13,
                data: "5901234123457".into(),
                options: BarcodeOptions {
                    width: 2,
                    ..BarcodeOptions::default()
                },
            })
            .is_ok()
        );
    }
}
