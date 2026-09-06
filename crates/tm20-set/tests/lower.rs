//! Lowering golden: a Sheet becomes this Document, not a speed setting.

mod common;

use tm20::PRINTABLE_DOTS;
use tm20::Raster;
use tm20::command::{CodePage, Command};
use tm20::encode::encode;
use tm20::graphics::max_height;
use tm20_set::{Cut, Figure, Frame, Head, Sheet, TextBlock, TextSize, compose, lower};

fn text(s: &'static str) -> Frame<'static> {
    Frame::Text(TextBlock::plain(Cut::Roman, TextSize::Pt11, s))
}

fn bands(doc: &tm20::Document) -> Vec<&tm20::graphics::Graphics> {
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
            assert_eq!(g.raster().width(), PRINTABLE_DOTS);
            assert!(g.raster().pixels().iter().any(|&b| b != 0));
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
fn a_tall_sheet_is_min_bands_and_exact_bytes() {
    let faces = common::table();
    let sheet = Sheet::tape((0..80).map(|_| text("H")).collect());
    let page = compose(&sheet, &faces).unwrap();
    let cap = max_height(page.width());
    assert!(
        page.height() > u32::from(cap),
        "need a sheet taller than one payload ({} ≤ {cap})",
        page.height()
    );
    let doc = lower(&sheet, &faces).unwrap();
    let gs = bands(&doc);
    let n = page.height().div_ceil(u32::from(cap)) as usize;
    assert_eq!(gs.len(), n, "min payloads for height {}", page.height());
    assert!(gs.iter().all(|g| g.raster().width() == PRINTABLE_DOTS));
    assert!(gs.iter().all(|g| g.raster().height() <= u32::from(cap)));
    let rasters: Vec<Raster> = gs.iter().map(|g| g.raster().clone()).collect();
    let refs: Vec<&Raster> = rasters.iter().collect();
    common::concat_equals_page(&page, &refs);
    encode(&doc).unwrap();
}

#[test]
fn head_stays_with_following_ink_when_that_keeps_min_count() {
    let faces = common::table();
    let fig_h = 900u32;
    let bits = vec![true; PRINTABLE_DOTS as usize * fig_h as usize];
    let sheet = Sheet::tape(vec![
        Frame::Head(Head {
            size: TextSize::Pt11,
            text: "Head".into(),
        }),
        Frame::Figure(Figure::from_bits(PRINTABLE_DOTS, fig_h, &bits).unwrap()),
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
    assert_eq!(gs.len(), full.height().div_ceil(u32::from(cap)) as usize);
    assert!(
        gs[0].raster().height() > head_only.height() + 8,
        "first band {} should keep the head with following ink, not split after {} dots",
        gs[0].raster().height(),
        head_only.height()
    );
    assert_eq!(gs[0].raster().height(), u32::from(cap));
}
