//! 1D barcodes: validation and `GS k` function B bytes.

use crate::error::EncodeError;

const CODE39_VALID: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '$', '%', '*', '+', '-', '.', '/', 'A', 'B',
    'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U',
    'V', 'W', 'X', 'Y', 'Z', ' ',
];
const CODABAR_VALID: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'a', 'b', 'c', 'd', '$',
    '+', '-', '.', '/', ':',
];
const CODE93_VALID: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I',
    'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', ' ', '$',
    '%', '+', '-', '.', '/',
];

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

fn payload(kind: BarcodeKind, data: &str) -> Vec<u8> {
    match kind {
        BarcodeKind::Code128 { set } => {
            let mut out = set.prefix().to_vec();
            out.extend(data.as_bytes());
            out
        }
        _ => data.as_bytes().to_vec(),
    }
}

pub fn validate(kind: BarcodeKind, data: &str) -> Result<(), EncodeError> {
    let len = data.len();
    let digits = data.chars().all(|c| c.is_ascii_digit());
    let ok = match kind {
        BarcodeKind::UpcA => digits && matches!(len, 11 | 12),
        BarcodeKind::UpcE => {
            digits && matches!(len, 6 | 7 | 8 | 11 | 12) && (len == 6 || data.starts_with('0'))
        }
        BarcodeKind::Ean13 => digits && matches!(len, 12 | 13),
        BarcodeKind::Ean8 => digits && matches!(len, 7 | 8),
        BarcodeKind::Itf => digits && len >= 2 && len % 2 == 0,
        BarcodeKind::Code39 => len >= 1 && data.chars().all(|c| CODE39_VALID.contains(&c)),
        BarcodeKind::Codabar => len >= 2 && data.chars().all(|c| CODABAR_VALID.contains(&c)),
        BarcodeKind::Code93 => {
            (1..=255).contains(&len) && data.chars().all(|c| CODE93_VALID.contains(&c))
        }
        BarcodeKind::Code128 { set: Code128Set::C } => {
            digits && (1..=253).contains(&len) && len % 2 == 0
        }
        BarcodeKind::Code128 { .. } => (1..=253).contains(&len) && data.is_ascii(),
        BarcodeKind::Gs1_128 => (2..=255).contains(&len) && data.is_ascii(),
    };
    if !ok {
        return Err(EncodeError::BarcodeData { kind });
    }
    let n = payload(kind, data).len();
    if n > 255 {
        return Err(EncodeError::BarcodeTooLong { len: n });
    }
    Ok(())
}

pub fn encode(barcode: &Barcode) -> Result<Vec<u8>, EncodeError> {
    validate(barcode.kind, &barcode.data)?;
    let opt = barcode.options;
    if !(1..=6).contains(&opt.width) {
        return Err(EncodeError::BarcodeWidth(opt.width));
    }
    if opt.height == 0 {
        return Err(EncodeError::BarcodeHeight(opt.height));
    }
    let data = payload(barcode.kind, &barcode.data);
    let mut out = Vec::new();
    out.extend_from_slice(&[0x1d, b'w', opt.width]);
    out.extend_from_slice(&[0x1d, b'h', opt.height]);
    out.extend_from_slice(&[0x1d, b'f', opt.font.byte()]);
    out.extend_from_slice(&[0x1d, b'H', opt.hri_position.byte()]);
    out.extend_from_slice(&[0x1d, b'k', barcode.kind.function_b_byte(), data.len() as u8]);
    out.extend_from_slice(&data);
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
    }

    #[test]
    fn gs1_128() {
        assert!(validate(BarcodeKind::Gs1_128, "{1012345").is_ok());
        assert!(validate(BarcodeKind::Gs1_128, "x").is_err());
    }
}
