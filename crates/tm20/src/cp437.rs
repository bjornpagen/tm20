//! Unicode → CP437 mapping used by morningprint for TM-T20III block art.

fn map_extended(ch: char) -> Option<u8> {
    Some(match ch {
        '░' => 0xb0,
        '▒' => 0xb1,
        '▓' => 0xb2,
        '█' => 0xdb,
        '▄' => 0xdc,
        '▌' => 0xdd,
        '▐' => 0xde,
        '▀' => 0xdf,
        '■' => 0xfe,
        '─' => 0xc4,
        '│' => 0xb3,
        '┌' => 0xda,
        '┐' => 0xbf,
        '└' => 0xc0,
        '┘' => 0xd9,
        '├' => 0xc3,
        '┤' => 0xb4,
        '┬' => 0xc2,
        '┴' => 0xc1,
        '┼' => 0xc5,
        '═' => 0xcd,
        '║' => 0xba,
        '╔' => 0xc9,
        '╗' => 0xbb,
        '╚' => 0xc8,
        '╝' => 0xbc,
        '╠' => 0xcc,
        '╣' => 0xb9,
        '╦' => 0xcb,
        '╩' => 0xca,
        '╬' => 0xce,
        '╒' => 0xd5,
        '╓' => 0xd6,
        '╕' => 0xb8,
        '╖' => 0xb7,
        '╘' => 0xd4,
        '╙' => 0xd3,
        '╛' => 0xbe,
        '╜' => 0xbd,
        '╞' => 0xc6,
        '╟' => 0xc7,
        '╡' => 0xb5,
        '╢' => 0xb6,
        '╤' => 0xd1,
        '╥' => 0xd2,
        '╧' => 0xcf,
        '╨' => 0xd0,
        '╪' => 0xd8,
        '╫' => 0xd7,
        '°' => 0xf8,
        '·' => 0xfa,
        '∙' | '•' => 0xf9,
        '√' => 0xfb,
        '±' => 0xf1,
        '≈' => 0xf7,
        '∞' => 0xec,
        '²' => 0xfd,
        'ⁿ' => 0xfc,
        '÷' => 0xf6,
        '≥' => 0xf2,
        '≤' => 0xf3,
        '≡' => 0xf0,
        '∩' => 0xef,
        '⌐' => 0xa9,
        '¬' => 0xaa,
        '½' => 0xab,
        '¼' => 0xac,
        '¡' => 0xad,
        '¿' => 0xa8,
        '«' => 0xae,
        '»' => 0xaf,
        '¢' => 0x9b,
        '£' => 0x9c,
        '¥' => 0x9d,
        '₧' => 0x9e,
        'ƒ' => 0x9f,
        'ª' => 0xa6,
        'º' => 0xa7,
        '⌠' => 0xf4,
        '⌡' => 0xf5,
        'α' => 0xe0,
        'ß' | 'β' => 0xe1,
        'Γ' => 0xe2,
        'π' => 0xe3,
        'Σ' => 0xe4,
        'σ' => 0xe5,
        'µ' | 'μ' => 0xe6,
        'τ' => 0xe7,
        'Φ' => 0xe8,
        'Θ' | 'θ' => 0xe9,
        'Ω' => 0xea,
        'δ' => 0xeb,
        'φ' => 0xed,
        'ε' => 0xee,
        'Ç' => 0x80,
        'ü' => 0x81,
        'é' => 0x82,
        'â' => 0x83,
        'ä' => 0x84,
        'à' => 0x85,
        'å' => 0x86,
        'ç' => 0x87,
        'ê' => 0x88,
        'ë' => 0x89,
        'è' => 0x8a,
        'ï' => 0x8b,
        'î' => 0x8c,
        'ì' => 0x8d,
        'Ä' => 0x8e,
        'Å' => 0x8f,
        'É' => 0x90,
        'æ' => 0x91,
        'Æ' => 0x92,
        'ô' => 0x93,
        'ö' => 0x94,
        'ò' => 0x95,
        'û' => 0x96,
        'ù' => 0x97,
        'ÿ' => 0x98,
        'Ö' => 0x99,
        'Ü' => 0x9a,
        'á' => 0xa0,
        'í' => 0xa1,
        'ó' => 0xa2,
        'ú' => 0xa3,
        'ñ' => 0xa4,
        'Ñ' => 0xa5,
        _ => return None,
    })
}

fn normalize_char(ch: char) -> Option<char> {
    match ch {
        '\u{2018}' | '\u{2019}' | '\u{02bc}' => Some('\''),
        '\u{201c}' | '\u{201d}' => Some('"'),
        '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
        | '\u{2212}' => Some('-'),
        '\u{00a0}' => Some(' '),
        _ => None,
    }
}

/// Encode `s` as CP437 printer bytes.
///
/// Printable ASCII passes through. LF is kept. Other C0 controls are dropped.
/// Unmapped characters become `?`.
pub fn encode_cp437(s: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(s.len());
    for ch in s.chars() {
        if ch == '\u{2026}' {
            bytes.extend_from_slice(b"...");
            continue;
        }
        let ch = normalize_char(ch).unwrap_or(ch);
        let code = ch as u32;
        if ch == '\n' {
            bytes.push(0x0a);
        } else if (0x20..=0x7e).contains(&code) {
            bytes.push(code as u8);
        } else if let Some(mapped) = map_extended(ch) {
            bytes.push(mapped);
        } else if code >= 0x20 && code != 0x7f {
            bytes.push(b'?');
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_passthrough() {
        assert_eq!(encode_cp437("SYSTEM ONLINE"), b"SYSTEM ONLINE");
    }

    #[test]
    fn block_art() {
        assert_eq!(encode_cp437("░▒▓█▀▄"), [0xb0, 0xb1, 0xb2, 0xdb, 0xdf, 0xdc]);
    }

    #[test]
    fn unknown_becomes_question() {
        assert_eq!(encode_cp437("hello 😀"), b"hello ?");
    }

    #[test]
    fn smart_punctuation() {
        assert_eq!(encode_cp437("it’s…"), b"it's...");
    }
}
