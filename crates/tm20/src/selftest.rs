//! Named paper cases. Each one is a [`Document`]; the CLI prints them.

use crate::barcode::{Barcode, BarcodeKind, BarcodeOptions, Code128Set};
use crate::command::{Align, CashDrawerPin, CodePage, Command, Font, Underline};
use crate::document::Document;
use crate::graphics::{Graphics, GraphicsScale, pack};
use crate::host::{hello, rule, ruler};
use crate::symbol::{
    DataMatrix, DataMatrixType, Gs1DataBar, Gs1DataBarType, Gs1DataBarWidth, MaxiCode,
    MaxiCodeMode, Pdf417, Qr, QrEcc, QrModel,
};
use crate::{COLS_A, PRINTABLE_DOTS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Case {
    pub id: &'static str,
    pub title: &'static str,
    pub expect: &'static str,
    /// Printed by `tm20 test all`. Drawer is opt-in.
    pub in_all: bool,
}

impl Case {
    pub fn doc(self) -> Document {
        match self.id {
            "hello" => hello(),
            "ruler" => ruler(),
            "style" => style(),
            "glyphs" => glyphs(),
            "barcodes" => barcodes(),
            "code128" => code128(),
            "qr" => qr(),
            "pdf417" => pdf417(),
            "datamatrix" => datamatrix(),
            "maxicode" => maxicode(),
            "gs1" => gs1(),
            "graphics" => graphics(),
            "layout" => layout(),
            "upside" => upside(),
            "rotate" => rotate(),
            "drawer" => drawer(),
            _ => unreachable!("catalog ids are closed"),
        }
    }
}

pub fn catalog() -> &'static [Case] {
    &[
        Case {
            id: "hello",
            title: "connectivity",
            expect: "SYSTEM ONLINE, then a partial cut",
            in_all: false,
        },
        Case {
            id: "ruler",
            title: "column / spacing calibration",
            expect: "48-col Font A and 64-col Font B rulers; even ▀ stripes",
            in_all: false,
        },
        Case {
            id: "style",
            title: "text style",
            expect: "left/center/right, Font A/B, bold, underlines, invert, 2x size",
            in_all: true,
        },
        Case {
            id: "glyphs",
            title: "CP437 block art",
            expect: "░▒▓█▀▄ and a box-drawing frame, not question marks",
            in_all: true,
        },
        Case {
            id: "barcodes",
            title: "1D barcodes",
            expect: "UPC-A, EAN-13, EAN-8, CODE39, ITF, CODABAR with HRI below",
            in_all: true,
        },
        Case {
            id: "code128",
            title: "CODE93 / CODE128 / GS1-128",
            expect: "CODE93 TM20, CODE128 TM20, GS1-128; function B, no NUL terminator",
            in_all: true,
        },
        Case {
            id: "qr",
            title: "QR Model 2",
            expect: "scannable QR for https://example.com/tm20",
            in_all: true,
        },
        Case {
            id: "pdf417",
            title: "PDF417",
            expect: "a PDF417 stack, or firmware ignores it",
            in_all: true,
        },
        Case {
            id: "datamatrix",
            title: "DataMatrix",
            expect: "a square DataMatrix, or firmware ignores it",
            in_all: true,
        },
        Case {
            id: "maxicode",
            title: "MaxiCode",
            expect: "a MaxiCode bullseye, or firmware ignores it",
            in_all: true,
        },
        Case {
            id: "gs1",
            title: "GS1 DataBar stacked",
            expect: "a stacked GS1 DataBar, or firmware ignores it",
            in_all: true,
        },
        Case {
            id: "graphics",
            title: "GS ( L bitmap",
            expect: "128x64 checkerboard, then a 576-dot black strip",
            in_all: true,
        },
        Case {
            id: "layout",
            title: "print position / feed dots",
            expect: "ITEM left, $12.00 at a fixed column; spaced text; a 30-dot gap",
            in_all: true,
        },
        Case {
            id: "upside",
            title: "upside-down text",
            expect: "UPSIDE DOWN on the physical right until you flip the slip",
            in_all: true,
        },
        Case {
            id: "rotate",
            title: "ESC V 90° clockwise",
            expect: "ROTATE stays on the left; each glyph turned 90°",
            in_all: true,
        },
        Case {
            id: "drawer",
            title: "cash drawer pulse",
            expect: "pin 2 pulse; kick if a drawer is wired. skip if not.",
            in_all: false,
        },
    ]
}

pub fn find(id: &str) -> Option<Case> {
    catalog().iter().copied().find(|c| c.id == id)
}

fn start(id: &str) -> Vec<Command> {
    vec![
        Command::Init,
        Command::CodePage(CodePage::Pc437),
        Command::Align(Align::Left),
        Command::Font(Font::A),
        Command::Text(format!("TEST {id}\n")),
        rule(COLS_A as usize, '─'),
        Command::Feed { lines: 1 },
    ]
}

fn finish(mut cmds: Vec<Command>) -> Document {
    cmds.extend([
        Command::Align(Align::Left),
        Command::Font(Font::A),
        Command::Size {
            width: 1,
            height: 1,
        },
        Command::Bold(false),
        Command::Underline(Underline::Off),
        Command::Invert(false),
        Command::UpsideDown(false),
        Command::Rotate90(false),
        Command::CharSpacing { dots: 0 },
        Command::LeftMargin { dots: 0 },
        Command::PrintAreaWidth {
            dots: PRINTABLE_DOTS,
        },
        Command::Feed { lines: 3 },
        Command::Cut,
    ]);
    Document::new(cmds)
}

fn labeled(cmds: &mut Vec<Command>, label: &str) {
    cmds.push(Command::Text(format!("{label}\n")));
}

fn style() -> Document {
    let mut c = start("style");
    labeled(&mut c, "align left");
    c.push(Command::Align(Align::Center));
    labeled(&mut c, "align center");
    c.push(Command::Align(Align::Right));
    labeled(&mut c, "align right");
    c.push(Command::Align(Align::Left));
    c.push(Command::Font(Font::A));
    labeled(&mut c, "Font A 12x24");
    c.push(Command::Font(Font::B));
    labeled(&mut c, "Font B 9x17");
    c.push(Command::Font(Font::A));
    c.push(Command::Bold(true));
    labeled(&mut c, "bold on");
    c.push(Command::Bold(false));
    c.push(Command::Underline(Underline::Single));
    labeled(&mut c, "underline single");
    c.push(Command::Underline(Underline::Double));
    labeled(&mut c, "underline double");
    c.push(Command::Underline(Underline::Off));
    c.push(Command::DoubleStrike(true));
    labeled(&mut c, "double strike");
    c.push(Command::DoubleStrike(false));
    c.push(Command::Smoothing(true));
    labeled(&mut c, "smoothing on");
    c.push(Command::Smoothing(false));
    c.push(Command::Invert(true));
    labeled(&mut c, "  INVERT  ");
    c.push(Command::Invert(false));
    c.push(Command::Feed { lines: 1 });
    c.push(Command::Size {
        width: 2,
        height: 2,
    });
    labeled(&mut c, "SIZE 2x");
    c.push(Command::Size {
        width: 1,
        height: 1,
    });
    labeled(&mut c, "size 1x");
    finish(c)
}

fn glyphs() -> Document {
    let mut c = start("glyphs");
    labeled(&mut c, "blocks: ░▒▓█▀▄");
    labeled(&mut c, "box:");
    labeled(&mut c, "┌────────┐");
    labeled(&mut c, "│ tm20   │");
    labeled(&mut c, "└────────┘");
    labeled(&mut c, "rule:");
    c.push(rule(COLS_A as usize, '─'));
    c.push(Command::Feed { lines: 1 });
    finish(c)
}

fn barcode(kind: BarcodeKind, data: &str) -> Command {
    Command::Barcode(Barcode {
        kind,
        data: data.into(),
        options: BarcodeOptions::default(),
    })
}

fn barcodes() -> Document {
    let mut c = start("barcodes");
    c.push(Command::Align(Align::Center));
    labeled(&mut c, "UPC-A");
    c.push(barcode(BarcodeKind::UpcA, "042100005264"));
    c.push(Command::Feed { lines: 1 });
    labeled(&mut c, "EAN-13");
    c.push(barcode(BarcodeKind::Ean13, "5901234123457"));
    c.push(Command::Feed { lines: 1 });
    labeled(&mut c, "EAN-8");
    c.push(barcode(BarcodeKind::Ean8, "96385074"));
    c.push(Command::Feed { lines: 1 });
    labeled(&mut c, "CODE39");
    c.push(barcode(BarcodeKind::Code39, "TM20"));
    c.push(Command::Feed { lines: 1 });
    labeled(&mut c, "ITF");
    c.push(barcode(BarcodeKind::Itf, "123456"));
    c.push(Command::Feed { lines: 1 });
    labeled(&mut c, "CODABAR");
    c.push(barcode(BarcodeKind::Codabar, "A40156B"));
    c.push(Command::Feed { lines: 1 });
    finish(c)
}

fn code128() -> Document {
    let mut c = start("code128");
    c.push(Command::Align(Align::Center));
    labeled(&mut c, "CODE93");
    c.push(barcode(BarcodeKind::Code93, "TM20"));
    c.push(Command::Feed { lines: 1 });
    labeled(&mut c, "CODE128 set B");
    c.push(barcode(BarcodeKind::Code128 { set: Code128Set::B }, "TM20"));
    c.push(Command::Feed { lines: 1 });
    labeled(&mut c, "GS1-128");
    c.push(barcode(BarcodeKind::Gs1_128, "0101234567890128"));
    c.push(Command::Feed { lines: 1 });
    finish(c)
}

fn qr() -> Document {
    let mut c = start("qr");
    c.push(Command::Align(Align::Center));
    labeled(&mut c, "https://example.com/tm20");
    c.push(Command::Qr(Qr {
        data: "https://example.com/tm20".into(),
        model: QrModel::Model2,
        size: 4,
        ecc: QrEcc::M,
    }));
    finish(c)
}

fn pdf417() -> Document {
    let mut c = start("pdf417");
    c.push(Command::Align(Align::Center));
    c.push(Command::Pdf417(Pdf417 {
        data: "tm20 pdf417".into(),
        ..Pdf417::default()
    }));
    finish(c)
}

fn datamatrix() -> Document {
    let mut c = start("datamatrix");
    c.push(Command::Align(Align::Center));
    c.push(Command::DataMatrix(DataMatrix {
        data: "tm20".into(),
        kind: DataMatrixType::Square(0),
        size: 3,
    }));
    finish(c)
}

fn maxicode() -> Document {
    let mut c = start("maxicode");
    c.push(Command::Align(Align::Center));
    labeled(&mut c, "Mode 4 (arbitrary data)");
    c.push(Command::MaxiCode(MaxiCode {
        data: "tm20".into(),
        mode: MaxiCodeMode::Mode4,
    }));
    finish(c)
}

fn gs1() -> Document {
    let mut c = start("gs1");
    c.push(Command::Align(Align::Center));
    c.push(Command::Gs1DataBar(Gs1DataBar {
        data: "12401234567890".into(),
        width: Gs1DataBarWidth::M,
        kind: Gs1DataBarType::Stacked,
    }));
    finish(c)
}

fn graphics() -> Document {
    let width = 128u16;
    let height = 64u16;
    let mut bits = vec![false; width as usize * height as usize];
    for y in 0..height as usize {
        for x in 0..width as usize {
            bits[y * width as usize + x] = ((x / 8) + (y / 8)) % 2 == 0;
        }
    }
    let pixels = pack(width, height, &bits).expect("checkerboard size is consistent");
    let strip = pack(
        PRINTABLE_DOTS,
        16,
        &vec![true; PRINTABLE_DOTS as usize * 16],
    )
    .expect("576-dot strip size is consistent");
    let mut c = start("graphics");
    c.push(Command::Align(Align::Center));
    labeled(&mut c, "128x64 checkerboard GS ( L");
    c.push(Command::Graphics(Graphics {
        width_dots: width,
        height_dots: height,
        pixels,
        scale: GraphicsScale::Normal,
    }));
    c.push(Command::Feed { lines: 1 });
    labeled(&mut c, "576-dot black strip");
    c.push(Command::Graphics(Graphics {
        width_dots: PRINTABLE_DOTS,
        height_dots: 16,
        pixels: strip,
        scale: GraphicsScale::Normal,
    }));
    finish(c)
}

fn layout() -> Document {
    let mut c = start("layout");
    labeled(&mut c, "absolute: ITEM then $12.00");
    c.push(Command::Text("ITEM".into()));
    c.push(Command::AbsolutePosition { dots: 400 });
    c.push(Command::Text("$12.00\n".into()));
    labeled(&mut c, "tabs at 16 and 32");
    c.push(Command::SetTabs(vec![16, 32]));
    c.push(Command::Text("A".into()));
    c.push(Command::HorizontalTab);
    c.push(Command::Text("B".into()));
    c.push(Command::HorizontalTab);
    c.push(Command::Text("C\n".into()));
    labeled(&mut c, "char spacing 3");
    c.push(Command::CharSpacing { dots: 3 });
    c.push(Command::Text("SPACED\n".into()));
    c.push(Command::CharSpacing { dots: 0 });
    labeled(&mut c, "feed 30 dots");
    c.push(Command::FeedDots { dots: 30 });
    labeled(&mut c, "after feed-dots");
    labeled(&mut c, "left margin 48");
    c.push(Command::LeftMargin { dots: 48 });
    c.push(Command::Text("indented\n".into()));
    c.push(Command::LeftMargin { dots: 0 });
    finish(c)
}

fn upside() -> Document {
    let mut c = start("upside");
    labeled(&mut c, "before");
    c.push(Command::UpsideDown(true));
    labeled(&mut c, "UPSIDE DOWN");
    c.push(Command::UpsideDown(false));
    labeled(&mut c, "after");
    finish(c)
}

fn rotate() -> Document {
    let mut c = start("rotate");
    labeled(&mut c, "before");
    c.push(Command::Rotate90(true));
    labeled(&mut c, "ROTATE");
    c.push(Command::Rotate90(false));
    labeled(&mut c, "after");
    finish(c)
}

fn drawer() -> Document {
    let mut c = start("drawer");
    labeled(&mut c, "pulse pin 2");
    c.push(Command::CashDrawer(CashDrawerPin::Pin2));
    finish(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::encode;

    #[test]
    fn every_case_encodes() {
        for case in catalog() {
            encode(&case.doc()).unwrap_or_else(|e| panic!("{}: {e}", case.id));
        }
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = catalog().iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), catalog().len());
    }
}
