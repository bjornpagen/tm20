//! Lowering golden: a Sheet becomes this Document, not a speed setting.

mod common;

use tm20::PRINTABLE_DOTS;
use tm20::command::{CodePage, Command};
use tm20::encode::encode;
use tm20::graphics::{Graphics, max_height};
use tm20_set::{Cut, Figure, Frame, Head, Sheet, TextBlock, TextSize, compose, lower};

fn text(s: &'static str) -> Frame<'static> {
    Frame::Text(TextBlock::plain(Cut::Roman, TextSize::Pt11, s))
}

fn bands(doc: &tm20::Document) -> Vec<&Graphics> {
    doc.commands()
        .iter()
        .filter_map(|c| match c {
            Command::Graphics(g) => Some(g),
            _ => None,
        })
        .collect()
}

#[test]
fn lower_is_init_page_graphics_feed_cut() {
    let faces = common::table();
    let sheet = Sheet::tape(vec![text("ok")]);
    let doc = lower(&sheet, &faces).unwrap();
    match doc.commands() {
        [
            Command::Init,
            Command::CodePage(CodePage::Pc437),
            Command::Graphics(g),
            Command::Feed { lines: 3 },
            Command::Cut,
        ] => {
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
    assert!(
        bytes.windows(3).any(|w| w == [0x1d, b'V', b'B']),
        "job ends in a cut (GS V B)"
    );
}

#[test]
fn a_tall_sheet_is_ceil_h_over_cap_graphics() {
    let faces = common::table();
    let sheet = Sheet::tape((0..80).map(|_| text("H")).collect());
    let full = compose(&sheet, &faces).unwrap();
    let cap = max_height(full.width_dots);
    assert!(
        full.height_dots > cap,
        "need a sheet taller than one payload ({} ≤ {cap})",
        full.height_dots
    );
    let doc = lower(&sheet, &faces).unwrap();
    let gs = bands(&doc);
    let n = u32::from(full.height_dots).div_ceil(u32::from(cap)) as usize;
    assert_eq!(gs.len(), n, "min payloads for height {}", full.height_dots);
    assert!(gs.iter().all(|g| g.width_dots == PRINTABLE_DOTS));
    assert!(gs.iter().all(|g| g.height_dots <= cap));
    let sum: u32 = gs.iter().map(|g| u32::from(g.height_dots)).sum();
    assert_eq!(sum, u32::from(full.height_dots));
    encode(&doc).unwrap();
}

#[test]
fn head_stays_with_following_ink_when_that_keeps_min_count() {
    let faces = common::table();
    let fig_h = 900;
    let bits = vec![true; PRINTABLE_DOTS as usize * fig_h as usize];
    let sheet = Sheet::tape(vec![
        Frame::Head(Head {
            size: TextSize::Pt11,
            text: "Head".into(),
        }),
        Frame::Figure(Figure::from_bits(PRINTABLE_DOTS, fig_h as u16, bits).unwrap()),
    ]);
    let head_only = compose(
        &Sheet::tape(vec![Frame::Head(Head {
            size: TextSize::Pt11,
            text: "Head".into(),
        })]),
        &faces,
    )
    .unwrap();
    let full = compose(&sheet, &faces).unwrap();
    let cap = max_height(PRINTABLE_DOTS);
    let doc = lower(&sheet, &faces).unwrap();
    let gs = bands(&doc);
    assert_eq!(
        gs.len(),
        u32::from(full.height_dots).div_ceil(u32::from(cap)) as usize
    );
    assert!(
        gs[0].height_dots > head_only.height_dots + 8,
        "first band {} should keep the head with following ink, not split after {} dots",
        gs[0].height_dots,
        head_only.height_dots
    );
    assert_eq!(gs[0].height_dots, cap);
}
