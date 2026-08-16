//! Designed sheets. Not a dump of every size and weight.

use crate::face::{DisplayFace, TextFace, Weight};
use crate::frame::{Frame, Mark, MarkAlign, Pair, Rule, Sheet, TextBlock, Thickness, Tracking};
use crate::leading::GridSkip;
use crate::size::{DisplaySize, TextSize};
use tm20::document::Document;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Case {
    pub id: &'static str,
    pub title: &'static str,
}

pub fn catalog() -> &'static [Case] {
    &[
        Case {
            id: "scale",
            title: "five sizes, flush left",
        },
        Case {
            id: "ticket",
            title: "mark, rules, body, pairs",
        },
        Case {
            id: "nhg",
            title: "Neue Haas Grotesk Text 11 / Display 18",
        },
    ]
}

pub fn find(id: &str) -> Option<Case> {
    catalog().iter().copied().find(|c| c.id == id)
}

impl Case {
    pub fn doc(self) -> crate::Result<Document> {
        match self.id {
            "scale" => scale(),
            "ticket" => ticket(),
            "nhg" => nhg(),
            _ => unreachable!("catalog ids are closed"),
        }
    }
}

struct Faces {
    text_roman: TextFace,
    text_bold: TextFace,
    display_roman: DisplayFace,
}

fn system_faces() -> crate::Result<Faces> {
    Ok(Faces {
        text_roman: TextFace::sans(Weight::Roman)?,
        text_bold: TextFace::sans(Weight::Bold).or_else(|_| TextFace::sans(Weight::Roman))?,
        display_roman: DisplayFace::sans(Weight::Roman)?,
    })
}

fn nhg_path(file: &str) -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(std::path::Path::new(&home).join("Library/Fonts").join(file))
}

fn nhg_faces() -> Option<(TextFace, DisplayFace)> {
    let text = TextFace::from_path(nhg_path("Neue Haas Grotesk Text Pro 55 Roman.otf")?).ok()?;
    let display =
        DisplayFace::from_path(nhg_path("Neue Haas Grotesk Display Pro 55 Roman.otf")?).ok()?;
    Some((text, display))
}

fn scale_sheet<'a>(text: &'a TextFace, display: &'a DisplayFace) -> Sheet<'a> {
    Sheet::tape(vec![
        Frame::Text(TextBlock {
            face: text,
            size: TextSize::Pt8,
            text: "8pt  Hamburgerfonstiv",
            indent: 0,
        }),
        Frame::Text(TextBlock {
            face: text,
            size: TextSize::Pt11,
            text: "11pt Hamburgerfonstiv",
            indent: 0,
        }),
        Frame::Mark(Mark {
            face: display,
            size: DisplaySize::Pt14,
            text: "14pt Hamburgerfonstiv",
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::Mark(Mark {
            face: display,
            size: DisplaySize::Pt18,
            text: "18pt Grotesk",
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::Mark(Mark {
            face: display,
            size: DisplaySize::Pt24,
            text: "24pt",
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
    ])
}

fn ticket_sheet<'a>(text: &'a TextFace, bold: &'a TextFace, display: &'a DisplayFace) -> Sheet<'a> {
    Sheet::tape(vec![
        Frame::Mark(Mark {
            face: display,
            size: DisplaySize::Pt18,
            text: "RECEIPT",
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::Skip(GridSkip::ONE),
        Frame::Rule(Rule {
            thickness: Thickness::Two,
        }),
        Frame::Text(TextBlock {
            face: text,
            size: TextSize::Pt11,
            text: "Flush left, rag right, tabular prices. The printer never saw a TTF.",
            indent: 0,
        }),
        Frame::Skip(GridSkip::ONE),
        Frame::Pair(Pair {
            left_face: text,
            left_size: TextSize::Pt11,
            left: "Espresso",
            right_face: text,
            right_size: TextSize::Pt11,
            right: "$4.50",
            gutter: GridSkip::ONE,
        }),
        Frame::Pair(Pair {
            left_face: text,
            left_size: TextSize::Pt11,
            left: "Filter coffee",
            right_face: text,
            right_size: TextSize::Pt11,
            right: "$3.00",
            gutter: GridSkip::ONE,
        }),
        Frame::Skip(GridSkip::ONE),
        Frame::Rule(Rule {
            thickness: Thickness::One,
        }),
        Frame::Pair(Pair {
            left_face: bold,
            left_size: TextSize::Pt11,
            left: "Total",
            right_face: bold,
            right_size: TextSize::Pt11,
            right: "$7.50",
            gutter: GridSkip::ONE,
        }),
    ])
}

fn nhg_sheet<'a>(text: &'a TextFace, display: &'a DisplayFace) -> Sheet<'a> {
    Sheet::tape(vec![
        Frame::Mark(Mark {
            face: display,
            size: DisplaySize::Pt18,
            text: "Neue Haas Grotesk",
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::Skip(GridSkip::ONE),
        Frame::Rule(Rule {
            thickness: Thickness::Two,
        }),
        Frame::Text(TextBlock {
            face: text,
            size: TextSize::Pt11,
            text: "Text optical size at 11pt on a 576-dot measure. Kerning is in the shaper. Leading is a baseline skip on an 8-dot grid.",
            indent: 0,
        }),
        Frame::Skip(GridSkip::ONE),
        Frame::Pair(Pair {
            left_face: text,
            left_size: TextSize::Pt11,
            left: "Espresso",
            right_face: text,
            right_size: TextSize::Pt11,
            right: "$4.50",
            gutter: GridSkip::ONE,
        }),
        Frame::Pair(Pair {
            left_face: text,
            left_size: TextSize::Pt11,
            left: "Filter coffee",
            right_face: text,
            right_size: TextSize::Pt11,
            right: "$3.00",
            gutter: GridSkip::ONE,
        }),
    ])
}

fn scale() -> crate::Result<Document> {
    let faces = system_faces()?;
    crate::lower(&scale_sheet(&faces.text_roman, &faces.display_roman))
}

fn ticket() -> crate::Result<Document> {
    let faces = system_faces()?;
    crate::lower(&ticket_sheet(
        &faces.text_roman,
        &faces.text_bold,
        &faces.display_roman,
    ))
}

fn nhg() -> crate::Result<Document> {
    match nhg_faces() {
        Some((text, display)) => crate::lower(&nhg_sheet(&text, &display)),
        None => {
            let faces = system_faces()?;
            crate::lower(&Sheet::tape(vec![Frame::Text(TextBlock {
                face: &faces.text_roman,
                size: TextSize::Pt11,
                text: "no Neue Haas Grotesk in ~/Library/Fonts",
                indent: 0,
            })]))
        }
    }
}
