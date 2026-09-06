//! Designed sheets. Copy and catalog — not the typesetting engine.

use tm20::document::Document;
use tm20_set::{
    Code, ColAlign, Cols, Cut, DecimalDelim, DisplayCut, DisplaySize, Figure, Frame, GridSkip,
    Head, List, ListFit, ListItem, Mark, MarkAlign, Marker, Note, Quote, Rule, Sheet, Span,
    TextBlock, TextSize, Thickness, Tracking,
};

use crate::Result;
use tm20_set::FaceTable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    Ticket,
    Prose,
    Helvetica,
    Suite,
}

pub fn catalog() -> &'static [Case] {
    &[Case::Ticket, Case::Prose, Case::Helvetica, Case::Suite]
}

impl Case {
    pub fn id(self) -> &'static str {
        match self {
            Self::Ticket => "ticket",
            Self::Prose => "prose",
            Self::Helvetica => "helvetica",
            Self::Suite => "suite",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Ticket => "mark, columns, section rules",
            Self::Prose => "heads, emphasis, quote, list, columns",
            Self::Helvetica => "selected font voices, not a weight ladder",
            Self::Suite => "quote, code, nested blocks, notes, figure",
        }
    }

    pub fn doc(self, faces: &FaceTable) -> Result<Document> {
        match self {
            Self::Ticket => ticket(faces),
            Self::Prose => prose(faces),
            Self::Helvetica => helvetica(faces),
            Self::Suite => suite(faces),
        }
    }
}

fn cols<'a>(cut: Cut, item: &'a str, amount: &'a str) -> Cols<'a> {
    Cols::two(
        TextSize::Pt11,
        GridSkip::ONE,
        [ColAlign::Start, ColAlign::End],
        vec![[vec![Span::new(cut, item)], vec![Span::new(cut, amount)]]],
    )
}

fn item(cut: Cut, size: TextSize, text: &str) -> Vec<Frame<'_>> {
    vec![Frame::Text(TextBlock::plain(cut, size, text))]
}

fn li(cut: Cut, size: TextSize, text: &str) -> ListItem<'_> {
    ListItem::new(item(cut, size, text))
}

fn ticket(faces: &FaceTable) -> Result<Document> {
    let body = TextSize::Pt11;
    let frames = vec![
        Frame::Mark(Mark {
            cut: DisplayCut::Roman,
            size: DisplaySize::Pt18,
            text: "RECEIPT".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::Rule(Rule::tape(Thickness::Two)),
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
        Frame::Rule(Rule::tape(Thickness::One)),
        Frame::Cols(cols(Cut::Bold, "Total", "$7.50")),
    ];
    Ok(tm20_set::lower(&Sheet::tape(frames), faces)?)
}

fn prose(faces: &FaceTable) -> Result<Document> {
    let body = TextSize::Pt11;
    let frames = vec![
        Frame::Mark(Mark {
            cut: DisplayCut::Roman,
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
            fit: ListFit::Tight,
            items: vec![
                li(
                    Cut::Roman,
                    body,
                    "Leading follows the process: two points of slug, not one.",
                ),
                li(Cut::Roman, body, "A rule is a section, or it is nothing."),
            ],
        }),
        Frame::Rule(Rule::tape(Thickness::Two)),
        Frame::Head(Head {
            size: body,
            text: "Receipt".into(),
        }),
        Frame::Cols(cols(Cut::Roman, "Espresso", "$4.50")),
        Frame::Cols(cols(Cut::Roman, "Filter coffee", "$3.00")),
        Frame::Rule(Rule::tape(Thickness::One)),
        Frame::Cols(cols(Cut::Bold, "Total", "$7.50")),
    ];
    Ok(tm20_set::lower(&Sheet::tape(frames), faces)?)
}

fn helvetica(faces: &FaceTable) -> Result<Document> {
    let body = TextSize::Pt11;
    let mut frames = vec![Frame::Mark(Mark {
        cut: DisplayCut::Roman,
        size: DisplaySize::Pt18,
        text: "Type specimen".into(),
        align: MarkAlign::Start,
        tracking: Tracking(0),
    })];
    frames.push(Frame::Mark(Mark {
        cut: DisplayCut::Light,
        size: DisplaySize::Pt18,
        text: "Light".into(),
        align: MarkAlign::Start,
        tracking: Tracking(40),
    }));
    frames.extend([
        Frame::Rule(Rule::tape(Thickness::Two)),
        Frame::Text(TextBlock {
            size: body,
            spans: vec![
                Span::new(Cut::Roman, "Roman body. "),
                Span::new(Cut::Italic, "Italic voice. "),
                Span::new(Cut::Bold, "Bold is a voice, not a size."),
            ],
        }),
        Frame::Head(Head {
            size: body,
            text: "Items".into(),
        }),
        Frame::Cols(cols(Cut::Roman, "Espresso", "$4.50")),
    ]);
    Ok(tm20_set::lower(&Sheet::tape(frames), faces)?)
}

fn suite(faces: &FaceTable) -> Result<Document> {
    let body = TextSize::Pt11;
    let pig = Figure::from_image(include_bytes!("pig.png"))?;
    let mut sheet = Sheet::tape(Vec::new());
    let canon = sheet.add_note(Note::dest("https://www.vignelli.com/canon.pdf"))?;
    let ruder = sheet.add_note(Note::dest("Ruder, Typographie"))?;
    sheet.frames = vec![
        Frame::Mark(Mark {
            cut: DisplayCut::Roman,
            size: DisplaySize::Pt18,
            text: "SUITE".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::Text(TextBlock {
            size: body,
            spans: vec![
                Span::new(Cut::Roman, "A link is italic with a note: "),
                Span::new(Cut::Italic, "The Vignelli Canon"),
                Span::note(canon),
                Span::new(Cut::Roman, " and "),
                Span::new(Cut::Italic, "Typographie"),
                Span::note(ruder),
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
        Frame::Code(Code::new(body, "fn measure() -> u16 { 576 }")),
        Frame::List(List {
            size: body,
            cut: Cut::Roman,
            marker: Marker::Dash,
            fit: ListFit::Tight,
            items: vec![ListItem::new(vec![
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
                    fit: ListFit::Tight,
                    items: vec![
                        li(Cut::Roman, body, "Nested."),
                        li(Cut::Roman, body, "Still nested."),
                    ],
                }),
            ])],
        }),
        Frame::Figure(pig),
    ];
    Ok(tm20_set::lower(&sheet, faces)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tm20_set::Span;

    #[test]
    fn suite_notes_are_independent_spans() {
        let mut sheet = Sheet::tape(Vec::new());
        let canon = sheet
            .add_note(Note::dest("https://www.vignelli.com/canon.pdf"))
            .unwrap();
        let ruder = sheet.add_note(Note::dest("Ruder, Typographie")).unwrap();
        assert_eq!(sheet.note_count(), 2);
        let spans = [
            Span::new(tm20_set::Cut::Italic, "The Vignelli Canon"),
            Span::note(canon),
            Span::new(tm20_set::Cut::Italic, "Typographie"),
            Span::note(ruder),
        ];
        assert!(matches!(spans[1], Span::Note(_)));
        assert!(matches!(spans[3], Span::Note(_)));
        assert_ne!(canon, ruder);
    }
}
