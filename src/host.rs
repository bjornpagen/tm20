//! Host helpers that lower to [`crate::command::Command`]. Not engine surface.

use crate::barcode::{Barcode, BarcodeKind, BarcodeOptions};
use crate::command::{Align, CodePage, Command, CutKind, Font, LineSpacing};
use crate::document::Document;
use crate::symbol::{Qr, QrEcc, QrModel};
use crate::{COLS_A, COLS_B, ROW_DOTS_A};

pub fn rule(cols: usize, glyph: char) -> Command {
    Command::Text(glyph.to_string().repeat(cols))
}

pub fn hello() -> Document {
    Document::new(vec![
        Command::Init,
        Command::CodePage(CodePage::Pc437),
        Command::Text("SYSTEM ONLINE".into()),
        Command::Feed { lines: 3 },
        Command::Cut {
            kind: CutKind::Partial,
        },
    ])
}

pub fn text_page(text: &str) -> Document {
    Document::new(vec![
        Command::Init,
        Command::CodePage(CodePage::Pc437),
        Command::Text(text.to_string()),
        Command::Feed { lines: 3 },
        Command::Cut {
            kind: CutKind::Partial,
        },
    ])
}

fn digit_ruler(cols: usize) -> String {
    (1..=cols)
        .map(|i| char::from(b'0' + (i % 10) as u8))
        .collect()
}

/// morningprint calibration page as a document.
pub fn ruler() -> Document {
    let mut cmds = vec![
        Command::Init,
        Command::CodePage(CodePage::Pc437),
        Command::Align(Align::Left),
        Command::Text("FONT A ruler (48 cols):\n".into()),
        Command::Text(digit_ruler(COLS_A as usize)),
        Command::Feed { lines: 2 },
        Command::Font(Font::B),
        Command::Text("FONT B ruler (64 cols):\n".into()),
        Command::Text(digit_ruler(COLS_B as usize)),
        Command::Feed { lines: 2 },
        Command::Font(Font::A),
        Command::Text("SPACING: pick the n with EQUAL clean\n".into()),
        Command::Text("black/white stripes. solid black =\n".into()),
        Command::Text("overlap; wide white = gaps.\n\n".into()),
    ];
    for n in [ROW_DOTS_A, 43, 48] {
        cmds.push(Command::Text(format!("n={n}:\n")));
        cmds.push(Command::LineSpacing(LineSpacing::Dots(n)));
        for _ in 0..4 {
            cmds.push(Command::Text("▀".repeat(20)));
            cmds.push(Command::Feed { lines: 1 });
        }
        cmds.push(Command::LineSpacing(LineSpacing::Default));
        cmds.push(Command::Feed { lines: 1 });
    }
    cmds.extend([
        Command::Invert(true),
        Command::Text("  INVERT BAND  ".into()),
        Command::Invert(false),
        Command::Feed { lines: 1 },
        Command::Feed { lines: 3 },
        Command::Cut {
            kind: CutKind::Partial,
        },
    ]);
    Document::new(cmds)
}

pub fn qr_page(data: &str) -> Document {
    Document::new(vec![
        Command::Init,
        Command::Align(Align::Center),
        Command::Qr(Qr {
            data: data.to_string(),
            model: QrModel::Model2,
            size: 4,
            ecc: QrEcc::M,
        }),
        Command::Feed { lines: 3 },
        Command::Cut {
            kind: CutKind::Partial,
        },
    ])
}

pub fn ean13_page(digits: &str) -> Document {
    Document::new(vec![
        Command::Init,
        Command::Align(Align::Center),
        Command::Barcode(Barcode {
            kind: BarcodeKind::Ean13,
            data: digits.to_string(),
            options: BarcodeOptions::default(),
        }),
        Command::Feed { lines: 3 },
        Command::Cut {
            kind: CutKind::Partial,
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::encode;

    #[test]
    fn rule_is_text() {
        assert_eq!(rule(4, '─'), Command::Text("────".into()));
    }

    #[test]
    fn ruler_encodes() {
        assert!(!encode(&ruler()).unwrap().is_empty());
    }
}
