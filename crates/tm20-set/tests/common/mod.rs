//! Load a [`FaceTable`] the same way ticket does: system sans, not a house face.

use font_kit::family_name::FamilyName;
use font_kit::handle::Handle;
use font_kit::properties::{Properties, Style as KitStyle, Weight as KitWeight};
use font_kit::source::SystemSource;
use tm20_set::{Cut, Face, FaceTable, GridSkip, Pair, Span, TextSize};

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
    table.set_display(
        Cut::Roman,
        load(KitWeight::NORMAL, KitStyle::Normal)
            .expect("system display")
            .display(),
    );
    table
}

#[allow(dead_code)]
pub fn pair<'a>(cut: Cut, item: &'a str, amount: &'a str) -> Pair<'a> {
    Pair {
        size: TextSize::Pt11,
        gutter: GridSkip::ONE,
        left: vec![Span { cut, text: item }],
        figure: cut,
        amount,
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
