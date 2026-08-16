//! Designed sheets. Not a dump of every size and weight.

use crate::face::{DisplayFace, Slope, TextFace, Weight};
use crate::frame::{
    Frame, Head, List, Mark, MarkAlign, Measure, Rule, Sheet, Span, Table, TextBlock, Thickness,
    Tracking,
};
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
    pub fn doc(self) -> crate::Result<Document> {
        match self.id {
            "ticket" => ticket(),
            "prose" => prose(),
            "nhg" => nhg(),
            _ => unreachable!("catalog ids are closed"),
        }
    }
}

struct Kit {
    roman: TextFace,
    italic: TextFace,
    bold: TextFace,
    display: DisplayFace,
}

fn system_kit() -> crate::Result<Kit> {
    Ok(Kit {
        roman: TextFace::sans(Weight::Roman, Slope::Upright)?,
        italic: TextFace::sans(Weight::Roman, Slope::Italic)
            .or_else(|_| TextFace::sans(Weight::Roman, Slope::Upright))?,
        bold: TextFace::sans(Weight::Bold, Slope::Upright)
            .or_else(|_| TextFace::sans(Weight::Roman, Slope::Upright))?,
        display: DisplayFace::sans(Weight::Roman, Slope::Upright)?,
    })
}

fn nhg_path(file: &str) -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(std::path::Path::new(&home).join("Library/Fonts").join(file))
}

fn nhg_text(file: &str, weight: Weight, slope: Slope) -> Option<TextFace> {
    TextFace::from_path(nhg_path(file)?, weight, slope).ok()
}

fn nhg_display(file: &str, weight: Weight, slope: Slope) -> Option<DisplayFace> {
    DisplayFace::from_path(nhg_path(file)?, weight, slope).ok()
}

fn ticket() -> crate::Result<Document> {
    let kit = system_kit()?;
    let body = TextSize::Pt11;
    let espresso = Table::pair(
        Measure::TAPE,
        GridSkip::ONE,
        body,
        vec![Span {
            face: &kit.roman,
            text: "Espresso",
        }],
        &kit.roman,
        "$4.50",
    )?;
    let filter = Table::pair(
        Measure::TAPE,
        GridSkip::ONE,
        body,
        vec![Span {
            face: &kit.roman,
            text: "Filter coffee",
        }],
        &kit.roman,
        "$3.00",
    )?;
    let total = Table::pair(
        Measure::TAPE,
        GridSkip::ONE,
        body,
        vec![Span {
            face: &kit.bold,
            text: "Total",
        }],
        &kit.bold,
        "$7.50",
    )?;
    let head = Head::new(&kit.bold, body, "Today").ok();
    let mut frames = vec![
        Frame::Mark(Mark {
            face: &kit.display,
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
                    face: &kit.roman,
                    text: "Flush left, rag right. ",
                },
                Span {
                    face: &kit.italic,
                    text: "Tabular prices hang on the first baseline.",
                },
            ],
        }),
    ];
    if let Some(h) = head {
        frames.push(Frame::Head(h));
    }
    frames.extend([
        Frame::Table(espresso),
        Frame::Table(filter),
        Frame::Rule(Rule {
            thickness: Thickness::One,
        }),
        Frame::Table(total),
    ]);
    crate::lower(&Sheet::tape(frames))
}

fn prose() -> crate::Result<Document> {
    let kit = system_kit()?;
    let body = TextSize::Pt11;
    let espresso = Table::pair(
        Measure::TAPE,
        GridSkip::ONE,
        body,
        vec![Span {
            face: &kit.roman,
            text: "Espresso",
        }],
        &kit.roman,
        "$4.50",
    )?;
    let filter = Table::pair(
        Measure::TAPE,
        GridSkip::ONE,
        body,
        vec![Span {
            face: &kit.roman,
            text: "Filter coffee",
        }],
        &kit.roman,
        "$3.00",
    )?;
    let total = Table::pair(
        Measure::TAPE,
        GridSkip::ONE,
        body,
        vec![Span {
            face: &kit.bold,
            text: "Total",
        }],
        &kit.bold,
        "$7.50",
    )?;
    let measure = Head::new(&kit.bold, body, "Measure").ok();
    let receipt = Head::new(&kit.bold, body, "Receipt").ok();
    let mut frames = vec![
        Frame::Mark(Mark {
            face: &kit.display,
            size: DisplaySize::Pt18,
            text: "TM-T20III",
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::Text(TextBlock {
            size: body,
            spans: vec![
                Span {
                    face: &kit.roman,
                    text: "The printer writes ",
                },
                Span {
                    face: &kit.bold,
                    text: "576 dots",
                },
                Span {
                    face: &kit.roman,
                    text: " across an 80 millimetre tape. That is the ",
                },
                Span {
                    face: &kit.italic,
                    text: "measure",
                },
                Span {
                    face: &kit.roman,
                    text: ". Everything else is a decision about type on that line.",
                },
            ],
        }),
    ];
    if let Some(h) = measure {
        frames.push(Frame::Head(h));
    }
    frames.extend([
        Frame::Text(TextBlock {
            size: body,
            spans: vec![
                Span {
                    face: &kit.roman,
                    text: "Eleven point body. ",
                },
                Span {
                    face: &kit.italic,
                    text: "Italic is a cut, not a slant. ",
                },
                Span {
                    face: &kit.bold,
                    text: "Bold is a voice, not a size.",
                },
            ],
        }),
        Frame::List(List {
            size: body,
            face: &kit.roman,
            items: vec![
                vec![Span {
                    face: &kit.roman,
                    text: "The column is the tape, not a page.",
                }],
                vec![Span {
                    face: &kit.roman,
                    text: "Leading follows the process: thermal ink spreads, so the slug is two points, not one.",
                }],
                vec![Span {
                    face: &kit.roman,
                    text: "A rule is a section, or it is nothing.",
                }],
            ],
        }),
        Frame::Rule(Rule {
            thickness: Thickness::Two,
        }),
    ]);
    if let Some(h) = receipt {
        frames.push(Frame::Head(h));
    }
    frames.extend([
        Frame::Text(TextBlock::plain(
            &kit.roman,
            body,
            "Item flush left, price tabular, hanging on the first baseline. No rules between rows.",
        )),
        Frame::Table(espresso),
        Frame::Table(filter),
        Frame::Rule(Rule {
            thickness: Thickness::One,
        }),
        Frame::Table(total),
    ]);
    crate::lower(&Sheet::tape(frames))
}

struct Nhg {
    text: TextFace,
    italic: TextFace,
    medium: TextFace,
    bold: TextFace,
    display: DisplayFace,
    display_light: Option<DisplayFace>,
}

fn nhg_kit() -> Option<Nhg> {
    Some(Nhg {
        text: nhg_text(
            "Neue Haas Grotesk Text Pro 55 Roman.otf",
            Weight::Roman,
            Slope::Upright,
        )?,
        italic: nhg_text(
            "Neue Haas Grotesk Text Pro 56 Italic.otf",
            Weight::Roman,
            Slope::Italic,
        )?,
        medium: nhg_text(
            "Neue Haas Grotesk Text Pro 65 Medium.otf",
            Weight::Medium,
            Slope::Upright,
        )?,
        bold: nhg_text(
            "Neue Haas Grotesk Text Pro 75 Bold.otf",
            Weight::Bold,
            Slope::Upright,
        )?,
        display: nhg_display(
            "Neue Haas Grotesk Display Pro 55 Roman.otf",
            Weight::Roman,
            Slope::Upright,
        )?,
        display_light: nhg_display(
            "Neue Haas Grotesk Display Pro 45 Light.otf",
            Weight::Light,
            Slope::Upright,
        ),
    })
}

fn nhg() -> crate::Result<Document> {
    match nhg_kit() {
        Some(n) => {
            let body = TextSize::Pt11;
            let espresso = Table::pair(
                Measure::TAPE,
                GridSkip::ONE,
                body,
                vec![Span {
                    face: &n.text,
                    text: "Espresso",
                }],
                &n.text,
                "$4.50",
            )?;
            let mut frames = vec![Frame::Mark(Mark {
                face: &n.display,
                size: DisplaySize::Pt18,
                text: "Neue Haas Grotesk",
                align: MarkAlign::Start,
                tracking: Tracking(0),
            })];
            if let Some(light) = &n.display_light {
                frames.push(Frame::Mark(Mark {
                    face: light,
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
                            face: &n.text,
                            text: "Roman body. ",
                        },
                        Span {
                            face: &n.italic,
                            text: "Italic is a cut, not a slant. ",
                        },
                        Span {
                            face: &n.medium,
                            text: "Medium is a grey, not Bold’s neighbour.",
                        },
                    ],
                }),
                Frame::Head(Head::new(&n.bold, body, "Items")?),
                Frame::Table(espresso),
            ]);
            crate::lower(&Sheet::tape(frames))
        }
        None => {
            let kit = system_kit()?;
            crate::lower(&Sheet::tape(vec![Frame::Text(TextBlock::plain(
                &kit.roman,
                TextSize::Pt11,
                "no Neue Haas Grotesk in ~/Library/Fonts",
            ))]))
        }
    }
}
