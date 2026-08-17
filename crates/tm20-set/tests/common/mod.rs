//! Load a [`FaceTable`]: system sans plus Commit Mono. Not a house grotesque.

use std::path::Path;

use font_kit::family_name::FamilyName;
use font_kit::handle::Handle;
use font_kit::properties::{Properties, Style as KitStyle, Weight as KitWeight};
use font_kit::source::SystemSource;
use tm20_set::{
    ColAlign, Cols, Cut, DecimalDelim, Face, FaceTable, Frame, GridSkip, List, ListFit, ListItem,
    Marker, Span, TextBlock, TextSize,
};

pub fn table() -> FaceTable {
    let mut table = FaceTable::new();
    table.set_text(
        Cut::Roman,
        load(KitWeight::NORMAL, KitStyle::Normal)
            .expect("system roman")
            .text(),
    );
    table.set_text(
        Cut::Italic,
        load(KitWeight::NORMAL, KitStyle::Italic)
            .or_else(|_| load(KitWeight::NORMAL, KitStyle::Normal))
            .expect("system italic")
            .text(),
    );
    table.set_text(
        Cut::Bold,
        load(KitWeight::BOLD, KitStyle::Normal)
            .or_else(|_| load(KitWeight::NORMAL, KitStyle::Normal))
            .expect("system bold")
            .text(),
    );
    table.set_text(
        Cut::BoldItalic,
        load(KitWeight::BOLD, KitStyle::Italic)
            .or_else(|_| load(KitWeight::BOLD, KitStyle::Normal))
            .or_else(|_| load(KitWeight::NORMAL, KitStyle::Italic))
            .or_else(|_| load(KitWeight::NORMAL, KitStyle::Normal))
            .expect("system bold italic")
            .text(),
    );
    table.set_text(Cut::Mono, commit_mono().text());
    table.set_display(
        Cut::Roman,
        load(KitWeight::NORMAL, KitStyle::Normal)
            .expect("system display")
            .display(),
    );
    table
}

pub fn commit_mono() -> Face {
    let home = std::env::var_os("HOME").unwrap_or_default();
    let path = Path::new(&home).join("Library/Fonts/CommitMono-400-Regular.otf");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|_| panic!("Commit Mono Regular not in {}", path.display()));
    Face::from_bytes(bytes).expect("Commit Mono parses")
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
pub fn plain<'a>(text: &'a str) -> Vec<Frame<'a>> {
    vec![Frame::Text(TextBlock::plain(
        Cut::Roman,
        TextSize::Pt11,
        text,
    ))]
}

#[allow(dead_code)]
pub fn item<'a>(text: &'a str) -> ListItem<'a> {
    ListItem::new(plain(text))
}

#[allow(dead_code)]
pub fn dash_list<'a>(items: Vec<ListItem<'a>>) -> List<'a> {
    List {
        size: TextSize::Pt11,
        cut: Cut::Roman,
        marker: Marker::Dash,
        fit: ListFit::Tight,
        items,
    }
}

#[allow(dead_code)]
pub fn decimal_list<'a>(start: u32, items: Vec<ListItem<'a>>) -> List<'a> {
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

fn load(weight: KitWeight, style: KitStyle) -> Result<Face, Box<dyn std::error::Error>> {
    let handle = SystemSource::new()
        .select_best_match(
            &[FamilyName::SansSerif],
            Properties::new().weight(weight).style(style),
        )
        .map_err(|_| "system sans-serif typeface not found")?;
    match handle {
        Handle::Path { path, font_index } => {
            Ok(Face::from_bytes_index(std::fs::read(path)?, font_index)?)
        }
        Handle::Memory { bytes, font_index } => {
            Ok(Face::from_bytes_index((*bytes).clone(), font_index)?)
        }
    }
}
