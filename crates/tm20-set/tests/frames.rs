//! Catalog analog: every Frame variant composes and encodes.

mod common;

use tm20::encode::encode;
use tm20::PRINTABLE_DOTS;
use tm20_set::{
    compose, lower, Cut, DisplaySize, Frame, Head, List, Mark, MarkAlign, Rule, Sheet, Span,
    TextBlock, TextSize, Thickness, Tracking,
};

fn cover(frame: &Frame<'_>) {
    match frame {
        Frame::Text(_)
        | Frame::Head(_)
        | Frame::Mark(_)
        | Frame::Pair(_)
        | Frame::List(_)
        | Frame::Rule(_) => {}
    }
}

fn kinds() -> Vec<Frame<'static>> {
    vec![
        Frame::Text(TextBlock::plain(Cut::Roman, TextSize::Pt11, "Hello")),
        Frame::Head(Head {
            size: TextSize::Pt11,
            text: "Head",
        }),
        Frame::Mark(Mark {
            cut: Cut::Roman,
            size: DisplaySize::Pt18,
            text: "MARK",
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::Pair(common::pair(Cut::Roman, "Coffee", "$1")),
        Frame::List(List {
            size: TextSize::Pt11,
            cut: Cut::Roman,
            items: vec![vec![Span {
                cut: Cut::Roman,
                text: "An item on the tape.",
            }]],
        }),
        Frame::Rule(Rule {
            thickness: Thickness::Two,
        }),
    ]
}

#[test]
fn every_frame_kind_encodes() {
    let faces = common::table();
    let frames = kinds();
    assert_eq!(frames.len(), 6);
    for frame in frames {
        cover(&frame);
        let sheet = Sheet::tape(vec![frame]);
        let g = compose(&sheet, &faces).unwrap();
        assert_eq!(g.width_dots, PRINTABLE_DOTS);
        encode(&lower(&sheet, &faces).unwrap()).unwrap();
    }
}

#[test]
fn mixed_adjacency_encodes() {
    let faces = common::table();
    let sheet = Sheet::tape(vec![
        Frame::Mark(Mark {
            cut: Cut::Roman,
            size: DisplaySize::Pt18,
            text: "MARK",
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::Rule(Rule {
            thickness: Thickness::Two,
        }),
        Frame::Head(Head {
            size: TextSize::Pt11,
            text: "Today",
        }),
        Frame::Text(TextBlock::plain(Cut::Roman, TextSize::Pt11, "A paragraph.")),
        Frame::List(List {
            size: TextSize::Pt11,
            cut: Cut::Roman,
            items: vec![vec![Span {
                cut: Cut::Roman,
                text: "First.",
            }]],
        }),
        Frame::Pair(common::pair(Cut::Roman, "Espresso", "$4.50")),
    ]);
    for frame in &sheet.frames {
        cover(frame);
    }
    let g = compose(&sheet, &faces).unwrap();
    assert_eq!(g.width_dots, PRINTABLE_DOTS);
    assert!(g.pixels.iter().any(|&b| b != 0));
    encode(&lower(&sheet, &faces).unwrap()).unwrap();
}

#[test]
fn table_lookup_is_by_cut() {
    let faces = common::table();
    assert!(faces.text(Cut::Roman).is_ok());
    assert!(faces.text(Cut::Bold).is_ok());
    assert!(faces.display(Cut::Roman).is_ok());
}
