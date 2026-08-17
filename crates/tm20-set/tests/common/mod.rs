//! Load a [`FaceTable`]: Neue Haas Grotesk plus Commit Mono. CFF OpenType only.

use std::path::Path;

use tm20_set::{
    ColAlign, Cols, Cut, DecimalDelim, Face, FaceTable, Frame, GridSkip, List, ListFit, ListItem,
    Marker, Span, TextBlock, TextSize,
};

pub fn table() -> FaceTable {
    let mut table = FaceTable::new();
    table.set_text(
        Cut::Roman,
        load("Neue Haas Grotesk Text Pro 55 Roman.otf").text(),
    );
    table.set_text(
        Cut::Italic,
        load("Neue Haas Grotesk Text Pro 56 Italic.otf").text(),
    );
    table.set_text(
        Cut::Bold,
        load("Neue Haas Grotesk Text Pro 75 Bold.otf").text(),
    );
    table.set_text(
        Cut::BoldItalic,
        load_or(
            "Neue Haas Grotesk Text Pro 76 Bold Italic.otf",
            "Neue Haas Grotesk Text Pro 75 Bold.otf",
        )
        .text(),
    );
    table.set_text(Cut::Mono, load("CommitMono-400-Regular.otf").text());
    table.set_display(
        Cut::Roman,
        load("Neue Haas Grotesk Display Pro 55 Roman.otf").display(),
    );
    table
}

fn load(file: &str) -> Face {
    let home = std::env::var_os("HOME").unwrap_or_default();
    let path = Path::new(&home).join("Library/Fonts").join(file);
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("{file} not in {}", path.display()));
    Face::from_bytes(bytes).unwrap_or_else(|_| panic!("{file} is CFF OpenType"))
}

fn load_or(file: &str, fallback: &str) -> Face {
    let home = std::env::var_os("HOME").unwrap_or_default();
    let path = Path::new(&home).join("Library/Fonts").join(file);
    if path.exists() {
        load(file)
    } else {
        load(fallback)
    }
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
