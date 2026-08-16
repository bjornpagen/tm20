//! Lowering golden: a Sheet becomes this Document, not a speed setting.

mod common;

use tm20::command::{CodePage, Command, CutKind};
use tm20::encode::encode;
use tm20::PRINTABLE_DOTS;
use tm20_set::{lower, Cut, Frame, Sheet, TextBlock, TextSize};

#[test]
fn lower_is_init_page_graphics_feed_cut() {
    let faces = common::table();
    let sheet = Sheet::tape(vec![Frame::Text(TextBlock::plain(
        Cut::Roman,
        TextSize::Pt11,
        "ok",
    ))]);
    let doc = lower(&sheet, &faces).unwrap();
    match doc.commands() {
        [Command::Init, Command::CodePage(CodePage::Pc437), Command::Graphics(g), Command::Feed { lines: 3 }, Command::Cut {
            kind: CutKind::Partial,
        }] => {
            assert_eq!(g.width_dots, PRINTABLE_DOTS);
            assert!(g.pixels.iter().any(|&b| b != 0));
        }
        other => panic!("unexpected lower sequence: {other:?}"),
    }
    assert!(
        !doc.commands()
            .iter()
            .any(|c| matches!(c, Command::PrintSpeed(_))),
        "typesetter must not inject PrintSpeed"
    );
    let bytes = encode(&doc).unwrap();
    assert_eq!(&bytes[..2], &[0x1b, 0x40]);
    assert!(bytes.windows(3).any(|w| w == [0x1d, b'(', b'L']));
}
