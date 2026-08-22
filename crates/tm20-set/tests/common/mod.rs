//! Load a [`FaceTable`]: Helvetica plus Menlo.

use std::sync::Arc;

use tm20_set::{
    ColAlign, Cols, Cut, DecimalDelim, DisplayCut, Face, FaceTable, Frame, GridSkip, List, ListFit,
    ListItem, Marker, Span, TextBlock, TextSize,
};

pub fn table() -> FaceTable {
    let bytes: Arc<[u8]> = std::fs::read("/System/Library/Fonts/Helvetica.ttc")
        .expect("Helvetica.ttc")
        .into();
    let mut table = FaceTable::new();
    for index in 0.. {
        let Ok(face) = Face::from_bytes_index(bytes.clone(), index) else {
            break;
        };
        let Some(name) = face.postscript_name() else {
            continue;
        };
        match name.as_str() {
            "Helvetica" => {
                table.set_text(
                    Cut::Roman,
                    Face::from_bytes_index(bytes.clone(), index)
                        .expect("Helvetica Regular")
                        .text(),
                );
                table.set_display(DisplayCut::Roman, face.display());
            }
            "Helvetica-Bold" => table.set_text(Cut::Bold, face.text()),
            "Helvetica-Oblique" => table.set_text(Cut::Italic, face.text()),
            "Helvetica-BoldOblique" => table.set_text(Cut::BoldItalic, face.text()),
            "Helvetica-Light" => table.set_display(DisplayCut::Light, face.display()),
            _ => {}
        }
    }
    table.set_text(Cut::Mono, load_mono().text());
    table
}

fn load_mono() -> Face {
    let bytes: Arc<[u8]> = std::fs::read("/System/Library/Fonts/Menlo.ttc")
        .expect("Menlo.ttc")
        .into();
    for index in 0.. {
        let Ok(face) = Face::from_bytes_index(bytes.clone(), index) else {
            break;
        };
        if face.postscript_name().as_deref() == Some("Menlo-Regular") {
            return face;
        }
    }
    panic!("Menlo-Regular not in Menlo.ttc")
}

/// A table missing exactly the named pieces, for boundary-error facts.
#[allow(dead_code)]
pub fn partial_table(bold: bool, mono: bool, display: bool) -> FaceTable {
    let bytes: Arc<[u8]> = std::fs::read("/System/Library/Fonts/Helvetica.ttc")
        .expect("Helvetica.ttc")
        .into();
    let mut out = FaceTable::new();
    for index in 0.. {
        let Ok(face) = Face::from_bytes_index(bytes.clone(), index) else {
            break;
        };
        let Some(name) = face.postscript_name() else {
            continue;
        };
        match name.as_str() {
            "Helvetica" => {
                out.set_text(
                    Cut::Roman,
                    Face::from_bytes_index(bytes.clone(), index)
                        .expect("Helvetica Regular")
                        .text(),
                );
                if display {
                    out.set_display(DisplayCut::Roman, face.display());
                }
            }
            "Helvetica-Bold" if bold => out.set_text(Cut::Bold, face.text()),
            "Helvetica-Oblique" => out.set_text(Cut::Italic, face.text()),
            "Helvetica-BoldOblique" => out.set_text(Cut::BoldItalic, face.text()),
            _ => {}
        }
    }
    if mono {
        out.set_text(Cut::Mono, load_mono().text());
    }
    out
}

#[allow(dead_code)]
pub fn cols<'a>(cut: Cut, item: &'a str, amount: &'a str) -> Cols<'a> {
    Cols::two(
        TextSize::Pt11,
        GridSkip::ONE,
        [ColAlign::Start, ColAlign::End],
        vec![[vec![Span::new(cut, item)], vec![Span::new(cut, amount)]]],
    )
}

#[allow(dead_code)]
pub fn plain(text: &str) -> Vec<Frame<'_>> {
    vec![Frame::Text(TextBlock::plain(
        Cut::Roman,
        TextSize::Pt11,
        text,
    ))]
}

#[allow(dead_code)]
pub fn item(text: &str) -> ListItem<'_> {
    ListItem::new(plain(text))
}

#[allow(dead_code)]
pub fn dash_list(items: Vec<ListItem<'_>>) -> List<'_> {
    List {
        size: TextSize::Pt11,
        cut: Cut::Roman,
        marker: Marker::Dash,
        fit: ListFit::Tight,
        items,
    }
}

#[allow(dead_code)]
pub fn decimal_list(start: u32, items: Vec<ListItem<'_>>) -> List<'_> {
    List {
        size: TextSize::Pt11,
        cut: Cut::Roman,
        marker: Marker::Decimal {
            start,
            delim: DecimalDelim::Period,
        },
        fit: ListFit::Tight,
        items,
    }
}
