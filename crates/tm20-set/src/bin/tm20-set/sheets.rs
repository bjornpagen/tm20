//! Designed sheets. Copy and catalog — not the typesetting engine.

use tm20::document::Document;
use tm20_set::{
    Cut, DisplaySize, Frame, GridSkip, Head, List, Mark, MarkAlign, Pair, Rule, Sheet, Span,
    TextBlock, TextSize, Thickness, Tracking,
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
            title: "mark, pairs, section rules",
        },
        Case {
            id: "prose",
            title: "markdown-shaped: heads, emphasis, rule, pair",
        },
        Case {
            id: "nhg",
            title: "Neue Haas Grotesk voices, not a weight ladder",
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
            _ => unreachable!("catalog ids are closed"),
        }
    }
}

fn pair<'a>(cut: Cut, item: &'a str, amount: &'a str) -> Pair<'a> {
    Pair {
        size: TextSize::Pt11,
        gutter: GridSkip::ONE,
        left: vec![Span { cut, text: item }],
        figure: cut,
        amount,
    }
}

fn ticket() -> Result<Document> {
    let faces = system_table()?;
    let body = TextSize::Pt11;
    let frames = vec![
        Frame::Mark(Mark {
            cut: Cut::Roman,
            size: DisplaySize::Pt18,
            text: "RECEIPT",
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::Rule(Rule {
            thickness: Thickness::Two,
        }),
        Frame::Text(TextBlock {
            size: body,
            spans: vec![
                Span {
                    cut: Cut::Roman,
                    text: "Flush left, rag right. ",
                },
                Span {
                    cut: Cut::Italic,
                    text: "Tabular prices hang on the first baseline.",
                },
            ],
        }),
        Frame::Head(Head {
            size: body,
            text: "Today",
        }),
        Frame::Pair(pair(Cut::Roman, "Espresso", "$4.50")),
        Frame::Pair(pair(Cut::Roman, "Filter coffee", "$3.00")),
        Frame::Rule(Rule {
            thickness: Thickness::One,
        }),
        Frame::Pair(pair(Cut::Bold, "Total", "$7.50")),
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
            text: "TM-T20III",
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::Text(TextBlock {
            size: body,
            spans: vec![
                Span {
                    cut: Cut::Roman,
                    text: "The printer writes ",
                },
                Span {
                    cut: Cut::Bold,
                    text: "576 dots",
                },
                Span {
                    cut: Cut::Roman,
                    text: " across an 80 millimetre tape. That is the ",
                },
                Span {
                    cut: Cut::Italic,
                    text: "measure",
                },
                Span {
                    cut: Cut::Roman,
                    text: ". Everything else is a decision about type on that line.",
                },
            ],
        }),
        Frame::Head(Head {
            size: body,
            text: "Measure",
        }),
        Frame::Text(TextBlock {
            size: body,
            spans: vec![
                Span {
                    cut: Cut::Roman,
                    text: "Eleven point body. ",
                },
                Span {
                    cut: Cut::Italic,
                    text: "Italic is a cut, not a slant. ",
                },
                Span {
                    cut: Cut::Bold,
                    text: "Bold is a voice, not a size.",
                },
            ],
        }),
        Frame::List(List {
            size: body,
            cut: Cut::Roman,
            items: vec![
                vec![Span {
                    cut: Cut::Roman,
                    text: "The column is the tape, not a page.",
                }],
                vec![Span {
                    cut: Cut::Roman,
                    text: "Leading follows the process: thermal ink spreads, so the slug is two points, not one.",
                }],
                vec![Span {
                    cut: Cut::Roman,
                    text: "A rule is a section, or it is nothing.",
                }],
            ],
        }),
        Frame::Rule(Rule {
            thickness: Thickness::Two,
        }),
        Frame::Head(Head {
            size: body,
            text: "Receipt",
        }),
        Frame::Text(TextBlock::plain(
            Cut::Roman,
            body,
            "Item flush left, price tabular, hanging on the first baseline. No rules between rows.",
        )),
        Frame::Pair(pair(Cut::Roman, "Espresso", "$4.50")),
        Frame::Pair(pair(Cut::Roman, "Filter coffee", "$3.00")),
        Frame::Rule(Rule {
            thickness: Thickness::One,
        }),
        Frame::Pair(pair(Cut::Bold, "Total", "$7.50")),
    ];
    Ok(tm20_set::lower(&Sheet::tape(frames), &faces)?)
}

fn nhg() -> Result<Document> {
    let faces = nhg_table()?;
    let body = TextSize::Pt11;
    let mut frames = vec![Frame::Mark(Mark {
        cut: Cut::Roman,
        size: DisplaySize::Pt18,
        text: "Neue Haas Grotesk",
        align: MarkAlign::Start,
        tracking: Tracking(0),
    })];
    if faces.display(Cut::Light).is_ok() {
        frames.push(Frame::Mark(Mark {
            cut: Cut::Light,
            size: DisplaySize::Pt18,
            text: "Display Light",
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
                Span {
                    cut: Cut::Roman,
                    text: "Roman body. ",
                },
                Span {
                    cut: Cut::Italic,
                    text: "Italic is a cut, not a slant. ",
                },
                Span {
                    cut: Cut::Medium,
                    text: "Medium is a grey, not Bold’s neighbour.",
                },
            ],
        }),
        Frame::Head(Head {
            size: body,
            text: "Items",
        }),
        Frame::Pair(pair(Cut::Roman, "Espresso", "$4.50")),
    ]);
    Ok(tm20_set::lower(&Sheet::tape(frames), &faces)?)
}
