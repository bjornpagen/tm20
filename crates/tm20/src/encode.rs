//! `encode` is a function. Valid IR becomes bytes; illegal combinations error.

use crate::barcode;
use crate::command::{
    Align, CashDrawerPin, Command, CutKind, Font, LineSpacing, PrintSpeed, Underline,
};
use crate::cp437::encode_cp437;
use crate::document::Document;
use crate::error::EncodeError;
use crate::graphics;
use crate::status;
use crate::symbol;

const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;
const HT: u8 = 0x09;
const CAN: u8 = 0x18;
const DRAWER_T1: u8 = 0x19;
const DRAWER_T2: u8 = 0x78;

pub fn encode(doc: &Document) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::new();
    for cmd in doc.commands() {
        encode_one(cmd, &mut out)?;
    }
    Ok(out)
}

fn u16_le(n: u16) -> [u8; 2] {
    [n as u8, (n >> 8) as u8]
}

fn encode_one(cmd: &Command, out: &mut Vec<u8>) -> Result<(), EncodeError> {
    match cmd {
        Command::Init => out.extend_from_slice(&[ESC, b'@']),
        Command::Cancel => out.push(CAN),
        Command::Cut { kind } => match kind {
            CutKind::Full => out.extend_from_slice(&[GS, b'V', b'A', 0]),
            CutKind::Partial => out.extend_from_slice(&[GS, b'V', b'B', 0]),
        },
        Command::Feed { lines } => out.extend(std::iter::repeat(0x0a).take(*lines as usize)),
        Command::FeedDots { dots } => out.extend_from_slice(&[ESC, b'J', *dots]),
        Command::CharSpacing { dots } => out.extend_from_slice(&[ESC, b' ', *dots]),
        Command::AbsolutePosition { dots } => {
            let [lo, hi] = u16_le(*dots);
            out.extend_from_slice(&[ESC, b'$', lo, hi]);
        }
        Command::RelativePosition { dots } => {
            let [lo, hi] = u16_le(*dots as u16);
            out.extend_from_slice(&[ESC, b'\\', lo, hi]);
        }
        Command::HorizontalTab => out.push(HT),
        Command::SetTabs(stops) => {
            out.extend_from_slice(&[ESC, b'D']);
            out.extend_from_slice(stops);
            out.push(0);
        }
        Command::LeftMargin { dots } => {
            let [lo, hi] = u16_le(*dots);
            out.extend_from_slice(&[GS, b'L', lo, hi]);
        }
        Command::PrintAreaWidth { dots } => {
            let [lo, hi] = u16_le(*dots);
            out.extend_from_slice(&[GS, b'W', lo, hi]);
        }
        Command::PrintSpeed(speed) => {
            let m = match speed {
                PrintSpeed::Default => 0,
                PrintSpeed::Level(n) if (1..=13).contains(n) => *n,
                PrintSpeed::Level(n) => return Err(EncodeError::PrintSpeed(*n)),
            };
            out.extend_from_slice(&[GS, b'(', b'K', 2, 0, 50, m]);
        }
        Command::LineSpacing(LineSpacing::Default) => out.extend_from_slice(&[ESC, b'2']),
        Command::LineSpacing(LineSpacing::Dots(n)) => out.extend_from_slice(&[ESC, b'3', *n]),
        Command::Align(a) => {
            let n = match a {
                Align::Left => 0,
                Align::Center => 1,
                Align::Right => 2,
            };
            out.extend_from_slice(&[ESC, b'a', n]);
        }
        Command::Font(font) => {
            let n = match font {
                Font::A => 0,
                Font::B => 1,
            };
            out.extend_from_slice(&[ESC, b'M', n]);
        }
        Command::Bold(on) => out.extend_from_slice(&[ESC, b'E', u8::from(*on)]),
        Command::Underline(u) => {
            let n = match u {
                Underline::Off => 0,
                Underline::Single => 1,
                Underline::Double => 2,
            };
            out.extend_from_slice(&[ESC, b'-', n]);
        }
        Command::DoubleStrike(on) => out.extend_from_slice(&[ESC, b'G', u8::from(*on)]),
        Command::Invert(on) => out.extend_from_slice(&[GS, b'B', u8::from(*on)]),
        Command::UpsideDown(on) => out.extend_from_slice(&[ESC, b'{', u8::from(*on)]),
        Command::Rotate90(on) => out.extend_from_slice(&[ESC, b'V', u8::from(*on)]),
        Command::Size { width, height } => {
            if !(1..=8).contains(width) || !(1..=8).contains(height) {
                return Err(EncodeError::Size {
                    width: *width,
                    height: *height,
                });
            }
            out.extend_from_slice(&[GS, b'!', ((width - 1) << 4) | (height - 1)]);
        }
        Command::Smoothing(on) => out.extend_from_slice(&[GS, b'b', u8::from(*on)]),
        Command::CodePage(page) => out.extend_from_slice(&[ESC, b't', page.byte()]),
        Command::CharacterSet(n) => out.extend_from_slice(&[ESC, b'R', *n]),
        Command::Text(s) => out.extend(encode_cp437(s)),
        Command::Raw(bytes) => out.extend_from_slice(bytes),
        Command::MotionUnits { x, y } => out.extend_from_slice(&[GS, b'P', *x, *y]),
        Command::CashDrawer(pin) => {
            let m = match pin {
                CashDrawerPin::Pin2 => 0,
                CashDrawerPin::Pin5 => 1,
            };
            out.extend_from_slice(&[ESC, b'p', m, DRAWER_T1, DRAWER_T2]);
        }
        Command::Barcode(b) => out.extend(barcode::encode(b)?),
        Command::Qr(q) => out.extend(symbol::encode_qr(q)?),
        Command::Pdf417(p) => out.extend(symbol::encode_pdf417(p)?),
        Command::Gs1DataBar(g) => out.extend(symbol::encode_gs1(g)?),
        Command::MaxiCode(m) => out.extend(symbol::encode_maxi(m)?),
        Command::DataMatrix(d) => out.extend(symbol::encode_data_matrix(d)?),
        Command::Graphics(g) => out.extend(graphics::encode(g)?),
        Command::StatusRequest(req) => out.extend(status::encode_request(*req)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CodePage, Command};
    use crate::host::hello;

    #[test]
    fn hello_golden() {
        let bytes = encode(&hello()).unwrap();
        assert_eq!(
            bytes,
            [
                0x1b, 0x40, 0x1b, 0x74, 0x00, b'S', b'Y', b'S', b'T', b'E', b'M', b' ', b'O', b'N',
                b'L', b'I', b'N', b'E', 0x0a, 0x0a, 0x0a, 0x1d, 0x56, 0x42, 0x00,
            ]
        );
    }

    #[test]
    fn size_rejects_out_of_range() {
        let doc = Document::new(vec![Command::Size {
            width: 9,
            height: 1,
        }]);
        assert!(matches!(
            encode(&doc),
            Err(EncodeError::Size {
                width: 9,
                height: 1
            })
        ));
    }

    #[test]
    fn codepage_other_is_esc_t() {
        let doc = Document::new(vec![Command::CodePage(CodePage::Other(16))]);
        assert_eq!(encode(&doc).unwrap(), vec![0x1b, 0x74, 16]);
    }

    #[test]
    fn print_speed_is_gs_k_fn50() {
        use crate::command::PrintSpeed;
        let doc = Document::new(vec![
            Command::PrintSpeed(PrintSpeed::Default),
            Command::PrintSpeed(PrintSpeed::level(8).unwrap()),
            Command::PrintSpeed(PrintSpeed::level(13).unwrap()),
        ]);
        assert_eq!(
            encode(&doc).unwrap(),
            vec![
                0x1d, b'(', b'K', 2, 0, 50, 0, 0x1d, b'(', b'K', 2, 0, 50, 8, 0x1d, b'(', b'K', 2,
                0, 50, 13,
            ]
        );
        let bad = Document::new(vec![Command::PrintSpeed(PrintSpeed::Level(14))]);
        assert!(matches!(encode(&bad), Err(EncodeError::PrintSpeed(14))));
    }

    #[test]
    fn motion_and_drawer() {
        let doc = Document::new(vec![
            Command::MotionUnits { x: 10, y: 20 },
            Command::CashDrawer(CashDrawerPin::Pin2),
        ]);
        assert_eq!(
            encode(&doc).unwrap(),
            vec![0x1d, b'P', 10, 20, 0x1b, b'p', 0, 0x19, 0x78]
        );
    }

    #[test]
    fn position_and_feed_dots() {
        let doc = Document::new(vec![
            Command::AbsolutePosition { dots: 576 },
            Command::RelativePosition { dots: -16 },
            Command::HorizontalTab,
            Command::SetTabs(vec![8, 16]),
            Command::LeftMargin { dots: 24 },
            Command::PrintAreaWidth { dots: 480 },
            Command::FeedDots { dots: 30 },
            Command::CharSpacing { dots: 2 },
        ]);
        let bytes = encode(&doc).unwrap();
        assert!(bytes.windows(4).any(|w| w == [0x1b, b'$', 0x40, 0x02]));
        assert!(bytes.windows(4).any(|w| w == [0x1b, b'\\', 0xf0, 0xff]));
        assert!(bytes.contains(&0x09));
        assert!(bytes.windows(5).any(|w| w == [0x1b, b'D', 8, 16, 0]));
        assert!(bytes.windows(4).any(|w| w == [0x1d, b'L', 24, 0]));
        assert!(bytes.windows(4).any(|w| w == [0x1d, b'W', 0xe0, 0x01]));
        assert!(bytes.windows(3).any(|w| w == [0x1b, b'J', 30]));
        assert!(bytes.windows(3).any(|w| w == [0x1b, b' ', 2]));
    }

    #[test]
    fn every_command_variant_encodes() {
        use crate::barcode::{Barcode, BarcodeKind, BarcodeOptions, Code128Set};
        use crate::command::PrintSpeed;
        use crate::graphics::{pack, Graphics, GraphicsScale};
        use crate::status::StatusRequest;
        use crate::symbol::{
            DataMatrix, DataMatrixType, Gs1DataBar, Gs1DataBarType, Gs1DataBarWidth, MaxiCode,
            MaxiCodeMode, Pdf417, Qr,
        };

        let pixels = pack(8, 8, &[true; 64]).unwrap();
        let cmds = vec![
            Command::Init,
            Command::Cancel,
            Command::Cut {
                kind: CutKind::Full,
            },
            Command::Cut {
                kind: CutKind::Partial,
            },
            Command::Feed { lines: 2 },
            Command::FeedDots { dots: 8 },
            Command::CharSpacing { dots: 1 },
            Command::AbsolutePosition { dots: 0 },
            Command::RelativePosition { dots: 8 },
            Command::HorizontalTab,
            Command::SetTabs(vec![10]),
            Command::LeftMargin { dots: 0 },
            Command::PrintAreaWidth { dots: 576 },
            Command::PrintSpeed(PrintSpeed::Default),
            Command::LineSpacing(LineSpacing::Default),
            Command::LineSpacing(LineSpacing::Dots(24)),
            Command::Align(Align::Center),
            Command::Font(Font::B),
            Command::Bold(true),
            Command::Underline(Underline::Double),
            Command::DoubleStrike(true),
            Command::Invert(true),
            Command::UpsideDown(true),
            Command::Rotate90(true),
            Command::Size {
                width: 2,
                height: 3,
            },
            Command::Smoothing(true),
            Command::CodePage(CodePage::Pc437),
            Command::CharacterSet(0),
            Command::Text("x".into()),
            Command::Raw(vec![0x1b, b'@']),
            Command::MotionUnits { x: 1, y: 1 },
            Command::CashDrawer(CashDrawerPin::Pin5),
            Command::Barcode(Barcode {
                kind: BarcodeKind::Code128 { set: Code128Set::B },
                data: "A".into(),
                options: BarcodeOptions::default(),
            }),
            Command::Qr(Qr {
                data: "q".into(),
                ..Qr::default()
            }),
            Command::Pdf417(Pdf417 {
                data: "p".into(),
                ..Pdf417::default()
            }),
            Command::Gs1DataBar(Gs1DataBar {
                data: "12401234567890".into(),
                width: Gs1DataBarWidth::M,
                kind: Gs1DataBarType::Stacked,
            }),
            Command::MaxiCode(MaxiCode {
                data: "m".into(),
                mode: MaxiCodeMode::Mode4,
            }),
            Command::DataMatrix(DataMatrix {
                data: "d".into(),
                kind: DataMatrixType::Square(0),
                size: 3,
            }),
            Command::Graphics(Graphics {
                width_dots: 8,
                height_dots: 8,
                pixels,
                scale: GraphicsScale::Normal,
            }),
            Command::StatusRequest(StatusRequest::Printer),
        ];
        encode(&Document::new(cmds)).unwrap();
    }
}
