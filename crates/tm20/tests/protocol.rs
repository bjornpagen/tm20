//! Independent protocol packet and model-boundary oracles.

mod common;

use tm20::barcode::{Barcode, BarcodeKind, BarcodeOptions, Code128Set};
use tm20::encode::encode;
use tm20::symbol::{Gs1DataBar, Gs1DataBarType, Gs1DataBarWidth};
use tm20::{Command, Document};

use common::walk_gs_k;

fn gs1_bytes(data: &str) -> Vec<u8> {
    encode(&Document::new(vec![Command::Gs1DataBar(Gs1DataBar {
        data: data.into(),
        width: Gs1DataBarWidth::S,
        kind: Gs1DataBarType::Stacked,
    })]))
    .unwrap()
}

fn extract_gs_k(bytes: &[u8]) -> Vec<u8> {
    let mut i = 0usize;
    while i + 3 <= bytes.len() {
        if bytes[i] == 0x1d && bytes[i + 1] == b'(' && bytes[i + 2] == b'k' {
            return bytes[i..].to_vec();
        }
        i += 1;
    }
    panic!("no GS ( k in encoded document");
}

/// Function B `GS k m n [n bytes]`. Declared `n` is the payload length.
fn function_b_payload(bytes: &[u8]) -> Result<(u8, Vec<u8>), String> {
    let mut i = 0usize;
    while i + 4 <= bytes.len() {
        if bytes[i] == 0x1d && bytes[i + 1] == b'k' {
            let m = bytes[i + 2];
            let n = usize::from(bytes[i + 3]);
            let start = i + 4;
            let end = start
                .checked_add(n)
                .ok_or_else(|| "Function B length overflow".to_string())?;
            if end > bytes.len() {
                return Err(format!("declared n={n} overruns buffer at {i}"));
            }
            return Ok((m, bytes[start..end].to_vec()));
        }
        i += 1;
    }
    Err("no GS k Function B header".into())
}

/// Walking length-prefixed GS ( k commands must consume the stream exactly.
/// The 13-digit stacked / width-S vector is the cited oracle.
#[test]
fn gs1_stream_has_no_leftover_bytes() {
    let bytes = gs1_bytes("1240123456789");
    let gs = extract_gs_k(&bytes);
    let packets = walk_gs_k(&gs).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        packets.iter().any(|p| p.cn == 51 && p.fn_ == 71),
        "fn=71 must appear: {packets:?}"
    );
    let want = [
        0x1d, 0x28, 0x6b, 0x03, 0x00, 0x33, 0x43, 0x02, 0x1d, 0x28, 0x6b, 0x04, 0x00, 0x33, 0x47,
        0x00, 0x00, 0x1d, 0x28, 0x6b, 0x11, 0x00, 0x33, 0x50, 0x30, 0x48, 0x31, 0x32, 0x34, 0x30,
        0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x1d, 0x28, 0x6b, 0x03, 0x00, 0x33,
        0x51, 0x30,
    ];
    assert_eq!(gs, want, "stacked DataBar S oracle");
}

#[test]
fn gs1_fn71_body_is_the_declared_payload() {
    let bytes = gs1_bytes("1240123456789");
    let gs = extract_gs_k(&bytes);
    let packets = walk_gs_k(&gs).unwrap_or_else(|e| panic!("{e}"));
    let fn71 = packets
        .iter()
        .find(|p| p.cn == 51 && p.fn_ == 71)
        .expect("fn=71");
    assert_eq!(
        fn71.body.as_slice(),
        &[0, 0],
        "fn=371 frozen ‘not set’ width is nL nH = 0 0"
    );
}

#[test]
fn code128_b_does_not_inject_raw_brace() {
    let kind = BarcodeKind::Code128 { set: Code128Set::B };
    let r = tm20::barcode::encode(&Barcode {
        kind,
        data: "{HI".into(),
        options: BarcodeOptions::default(),
    });
    let bytes = r.expect("CODE128-B {{ escape is representable");
    let (m, payload) = function_b_payload(&bytes).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(m, b'I', "CODE128 Function B m=73");
    assert_eq!(
        payload.as_slice(),
        b"{B{{HI",
        "declared payload must be set B plus escaped brace, not raw {{HI"
    );
    // `{` `B` `{` `{` `H` `I` — six encoded bytes, including prefix and escape.
    assert_eq!(payload.len(), 6);
}

#[test]
fn code128_a_rejects_lowercase() {
    let kind = BarcodeKind::Code128 { set: Code128Set::A };
    assert!(
        tm20::barcode::validate(kind, "abc").is_err(),
        "set A cannot admit lowercase"
    );
}

#[test]
fn code128_c_rejects_odd_or_non_digits() {
    let kind = BarcodeKind::Code128 { set: Code128Set::C };
    assert!(tm20::barcode::validate(kind, "123").is_err());
    assert!(tm20::barcode::validate(kind, "12AB").is_err());
}

#[test]
fn codabar_without_start_stop_is_rejected() {
    assert!(
        tm20::barcode::validate(BarcodeKind::Codabar, "12").is_err(),
        "Codabar digits alone are not a complete symbol"
    );
}

#[test]
fn symbol_store_lengths_respect_command_bounds() {
    use tm20::symbol::{MaxiCode, Pdf417};
    for (symbol, max) in [("PDF417", 65532), ("MaxiCode", 138)] {
        for len in [0, 1, max, max + 1] {
            let data = "a".repeat(len);
            let cmd = if symbol == "PDF417" {
                Command::Pdf417(Pdf417 {
                    data,
                    ..Pdf417::default()
                })
            } else {
                Command::MaxiCode(MaxiCode {
                    data,
                    ..MaxiCode::default()
                })
            };
            let result = encode(&Document::new(vec![cmd]));
            if len == 0 || len > max {
                assert!(result.is_err(), "{symbol} accepted {len}");
            } else {
                let bytes = result.unwrap();
                let packets = walk_gs_k(&bytes).unwrap();
                let store = packets.iter().find(|p| p.fn_ == 80).unwrap();
                assert_eq!(store.body.len(), len + 1);
            }
        }
    }
}

#[test]
fn t20iii_rejects_unsupported_hri_fonts() {
    use tm20::barcode::BarcodeFont;
    for font in [
        BarcodeFont::A,
        BarcodeFont::B,
        BarcodeFont::C,
        BarcodeFont::D,
        BarcodeFont::E,
    ] {
        let code = Barcode {
            kind: BarcodeKind::Code93,
            data: "A".into(),
            options: BarcodeOptions {
                font,
                ..BarcodeOptions::default()
            },
        };
        assert_eq!(
            tm20::barcode::encode(&code).is_ok(),
            matches!(font, BarcodeFont::A | BarcodeFont::B)
        );
    }
}
