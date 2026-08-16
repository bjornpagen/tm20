//! Designed sheets. Copy and catalog — not the typesetting engine.

use std::num::NonZeroU32;

use tm20::document::Document;
use tm20_set::{
    ColAlign, Cols, Cut, DecimalDelim, DisplaySize, Figure, Frame, GridSkip, Head, List, Mark,
    MarkAlign, Marker, Quote, Rule, Sheet, Span, TextBlock, TextSize, Thickness, Tracking,
};

use crate::kit::{nhg_table, system_table};
use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Case {
    pub id: &'static str,
    pub title: &'static str,
}

pub fn catalog() -> &'static [Case] {
    &[
        Case {
            id: "ticket",
            title: "mark, columns, section rules",
        },
        Case {
            id: "prose",
            title: "markdown-shaped: heads, emphasis, quote, list, columns",
        },
        Case {
            id: "nhg",
            title: "Neue Haas Grotesk voices, not a weight ladder",
        },
        Case {
            id: "suite",
            title: "quote, hung code, nested blocks, notes, figure",
        },
    ]
}

pub fn find(id: &str) -> Option<Case> {
    catalog().iter().copied().find(|c| c.id == id)
}

impl Case {
    pub fn doc(self) -> Result<Document> {
        match self.id {
            "ticket" => ticket(),
            "prose" => prose(),
            "nhg" => nhg(),
            "suite" => suite(),
            _ => unreachable!("catalog ids are closed"),
        }
    }
}

fn cols<'a>(cut: Cut, item: &'a str, amount: &'a str) -> Cols<'a> {
    Cols {
        size: TextSize::Pt11,
        gutter: GridSkip::ONE,
        align: vec![ColAlign::Start, ColAlign::End],
        rows: vec![vec![
            vec![Span::new(cut, item)],
            vec![Span::new(cut, amount)],
        ]],
    }
}

fn item<'a>(cut: Cut, size: TextSize, text: &'a str) -> Vec<Frame<'a>> {
    vec![Frame::Text(TextBlock::plain(cut, size, text))]
}

fn ticket() -> Result<Document> {
    let faces = system_table()?;
    let body = TextSize::Pt11;
    let frames = vec![
        Frame::Mark(Mark {
            cut: Cut::Roman,
            size: DisplaySize::Pt18,
            text: "RECEIPT".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::Rule(Rule {
            thickness: Thickness::Two,
        }),
        Frame::Text(TextBlock {
            size: body,
            spans: vec![
                Span::new(Cut::Roman, "Flush left, rag right. "),
                Span::new(Cut::Italic, "Tabular prices hang on the first baseline."),
            ],
        }),
        Frame::Head(Head {
            size: body,
            text: "Today".into(),
        }),
        Frame::Cols(cols(Cut::Roman, "Espresso", "$4.50")),
        Frame::Cols(cols(Cut::Roman, "Filter coffee", "$3.00")),
        Frame::Rule(Rule {
            thickness: Thickness::One,
        }),
        Frame::Cols(cols(Cut::Bold, "Total", "$7.50")),
    ];
    Ok(tm20_set::lower(&Sheet::tape(frames), &faces)?)
}

fn prose() -> Result<Document> {
    let faces = system_table()?;
    let body = TextSize::Pt11;
    let frames = vec![
        Frame::Mark(Mark {
            cut: Cut::Roman,
            size: DisplaySize::Pt18,
            text: "TM-T20III".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::Text(TextBlock {
            size: body,
            spans: vec![
                Span::new(Cut::Roman, "The printer writes "),
                Span::new(Cut::Bold, "576 dots"),
                Span::new(Cut::Roman, " across an 80 millimetre tape. That is the "),
                Span::new(Cut::Italic, "measure"),
                Span::new(Cut::Roman, "."),
            ],
        }),
        Frame::Head(Head {
            size: body,
            text: "Measure".into(),
        }),
        Frame::Text(TextBlock {
            size: body,
            spans: vec![
                Span::new(Cut::Roman, "Eleven point body. "),
                Span::new(Cut::Italic, "Italic is a cut, not a slant. "),
                Span::new(Cut::Bold, "Bold is a voice, not a size."),
            ],
        }),
        Frame::Quote(Quote {
            frames: item(
                Cut::Italic,
                body,
                "The column is the tape. White is adjacency, not a skip you type.",
            ),
        }),
        Frame::List(List {
            size: body,
            cut: Cut::Roman,
            marker: Marker::Dash,
            tight: true,
            items: vec![
                item(
                    Cut::Roman,
                    body,
                    "Leading follows the process: two points of slug, not one.",
                ),
                item(Cut::Roman, body, "A rule is a section, or it is nothing."),
            ],
        }),
        Frame::Rule(Rule {
            thickness: Thickness::Two,
        }),
        Frame::Head(Head {
            size: body,
            text: "Receipt".into(),
        }),
        Frame::Cols(cols(Cut::Roman, "Espresso", "$4.50")),
        Frame::Cols(cols(Cut::Roman, "Filter coffee", "$3.00")),
        Frame::Rule(Rule {
            thickness: Thickness::One,
        }),
        Frame::Cols(cols(Cut::Bold, "Total", "$7.50")),
    ];
    Ok(tm20_set::lower(&Sheet::tape(frames), &faces)?)
}

fn nhg() -> Result<Document> {
    let faces = nhg_table()?;
    let body = TextSize::Pt11;
    let mut frames = vec![Frame::Mark(Mark {
        cut: Cut::Roman,
        size: DisplaySize::Pt18,
        text: "Neue Haas Grotesk".into(),
        align: MarkAlign::Start,
        tracking: Tracking(0),
    })];
    if faces.display(Cut::Light).is_ok() {
        frames.push(Frame::Mark(Mark {
            cut: Cut::Light,
            size: DisplaySize::Pt18,
            text: "Display Light".into(),
            align: MarkAlign::Start,
            tracking: Tracking(40),
        }));
    }
    frames.extend([
        Frame::Rule(Rule {
            thickness: Thickness::Two,
        }),
        Frame::Text(TextBlock {
            size: body,
            spans: vec![
                Span::new(Cut::Roman, "Roman body. "),
                Span::new(Cut::Italic, "Italic is a cut, not a slant. "),
                Span::new(Cut::Medium, "Medium is a grey, not Bold’s neighbour."),
            ],
        }),
        Frame::Head(Head {
            size: body,
            text: "Items".into(),
        }),
        Frame::Cols(cols(Cut::Roman, "Espresso", "$4.50")),
    ]);
    Ok(tm20_set::lower(&Sheet::tape(frames), &faces)?)
}

fn suite() -> Result<Document> {
    let faces = system_table()?;
    let body = TextSize::Pt11;
    let pig = Figure::from_image(include_bytes!("pig.png"), 160)?;
    let mut canon = Span::new(Cut::Italic, "The Vignelli Canon");
    canon.note = NonZeroU32::new(1);
    let mut ruder = Span::new(Cut::Italic, "Typographie");
    ruder.note = NonZeroU32::new(2);
    let mut sheet = Sheet::tape(vec![
        Frame::Mark(Mark {
            cut: Cut::Roman,
            size: DisplaySize::Pt18,
            text: "SUITE".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::Text(TextBlock {
            size: body,
            spans: vec![
                Span::new(Cut::Roman, "A link is italic with a note: "),
                canon,
                Span::new(Cut::Roman, " and "),
                ruder,
                Span::new(Cut::Roman, "."),
            ],
        }),
        Frame::Quote(Quote {
            frames: item(
                Cut::Italic,
                body,
                "The column is the tape. White is adjacency, not a skip you type.",
            ),
        }),
        Frame::Quote(Quote {
            frames: item(Cut::Roman, body, "fn measure() -> u16 { 576 }"),
        }),
        Frame::List(List {
            size: body,
            cut: Cut::Roman,
            marker: Marker::Dash,
            tight: true,
            items: vec![vec![
                Frame::Text(TextBlock::plain(
                    Cut::Roman,
                    body,
                    "An item is a stack of blocks.",
                )),
                Frame::List(List {
                    size: body,
                    cut: Cut::Roman,
                    marker: Marker::Decimal {
                        start: 1,
                        delim: DecimalDelim::Period,
                    },
                    tight: true,
                    items: vec![
                        item(Cut::Roman, body, "Nested."),
                        item(Cut::Roman, body, "Still nested."),
                    ],
                }),
            ]],
        }),
        Frame::Figure(pig),
    ]);
    sheet.notes = vec![
        "https://www.vignelli.com/canon.pdf".into(),
        "Ruder, Typographie".into(),
    ];
    Ok(tm20_set::lower(&sheet, &faces)?)
}
