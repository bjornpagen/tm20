//! Catalog analog: every Frame variant lowers. A new variant fails the match.

mod common;

use tm20::encode::encode;
use tm20_set::{
    Code, Cut, DisplaySize, Figure, Frame, Head, Mark, MarkAlign, Math, Quote, Rule, Sheet, Span,
    TextBlock, TextSize, Thickness, Tracking, lower,
};

fn cover(frame: &Frame<'_>) {
    match frame {
        Frame::Text(_)
        | Frame::Head(_)
        | Frame::Mark(_)
        | Frame::Cols(_)
        | Frame::List(_)
        | Frame::Quote(_)
        | Frame::Code(_)
        | Frame::Figure(_)
        | Frame::Math(_)
        | Frame::Rule(_) => {}
    }
}

fn kinds() -> Vec<Frame<'static>> {
    vec![
        Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![
                Span::new(Cut::Roman, "Hello "),
                Span::new(Cut::Italic, "there, "),
                Span::new(Cut::Bold, "now, "),
                Span::new(Cut::BoldItalic, "both."),
            ],
        }),
        Frame::Head(Head {
            size: TextSize::Pt11,
            text: "Head".into(),
        }),
        Frame::Mark(Mark {
            cut: Cut::Roman,
            size: DisplaySize::Pt18,
            text: "MARK".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::Cols(common::cols(Cut::Roman, "Coffee", "$1")),
        Frame::List(common::dash_list(vec![common::item(
            "An item on the tape.",
        )])),
        Frame::Quote(Quote {
            frames: common::plain("Quoted."),
        }),
        Frame::Code(Code {
            size: TextSize::Pt11,
            lines: vec!["fn measure() -> u16 { 576 }".into()],
        }),
        Frame::Figure(Figure::from_bits(8, 8, &[true; 64]).unwrap()),
        Frame::Math(Math::from_bits(8, 8, &[true; 64], 6).unwrap()),
        Frame::Rule(Rule {
            thickness: Thickness::Two,
        }),
    ]
}

#[test]
fn every_frame_kind_encodes() {
    let faces = common::table();
    let frames = kinds();
    assert_eq!(frames.len(), 10);
    for frame in frames {
        cover(&frame);
        encode(&lower(&Sheet::tape(vec![frame]), &faces).unwrap()).unwrap();
    }
}
