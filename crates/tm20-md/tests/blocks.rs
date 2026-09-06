//! Block Markdown semantics: headings, lists, quotes, code, figures, nesting.

mod common;

use std::path::Path;

use tm20_md::{Error, image_bytes, sheet};
use tm20_set::{
    DecimalDelim, Frame, ItemBody, ItemMark, ListFit, Marker, Measure, RuleSpan, TextSize,
    Thickness,
};

use common::{parse, parse_err};

#[test]
fn thematic_break() {
    let s = parse("Hello\n\n---\n\nThere");
    assert!(matches!(
        s.frames[1],
        Frame::Rule(ref r) if r.thickness == Thickness::Two && r.span == RuleSpan::Tape
    ));
}

#[test]
fn atx_and_setext() {
    let s = parse("# Title\n\n## Section\n\nSetext\n======\n");
    assert!(matches!(&s.frames[0], Frame::Mark(m) if m.text == "Title"));
    assert!(matches!(&s.frames[1], Frame::Head(h) if h.text == "Section"));
    assert!(matches!(&s.frames[2], Frame::Mark(m) if m.text == "Setext"));
}

#[test]
fn heading_inlines_reject_instead_of_flattening() {
    assert!(matches!(
        parse_err("## Hello *world*"),
        Error::Unsupported { .. }
    ));
}

#[test]
fn empty_atx_heading_is_an_error() {
    assert!(matches!(
        parse_err("#\n\n# Title"),
        Error::Unsupported { .. }
    ));
}

#[test]
fn heading_html_is_rejected() {
    assert!(matches!(parse_err("# hello <b>world</b>"), Error::Html));
    assert!(matches!(
        parse_err("# *hello <b>x</b>*"),
        Error::Unsupported { .. }
    ));
}

#[test]
fn math_in_a_heading_is_an_error() {
    assert!(matches!(parse_err("# \\(x\\)"), Error::Math));
}

#[test]
fn two_paragraphs() {
    let s = parse("One\n\nTwo");
    assert_eq!(s.frames.len(), 2);
    assert!(matches!(s.frames[0], Frame::Text(_)));
    assert!(matches!(s.frames[1], Frame::Text(_)));
}

#[test]
fn fenced_and_indented_code() {
    let fenced = parse("```rust\nfn measure() -> u16 { 576 }\n```");
    assert_eq!(fenced.frames.len(), 1);
    assert!(matches!(
        &fenced.frames[0],
        Frame::Code(c) if c.lines().iter().any(|l| l.contains("fn measure"))
    ));
    let indented = parse("    let x = 1;\n");
    assert!(matches!(&indented.frames[0], Frame::Code(_)));
}

#[test]
fn fence_tab_is_a_column_advance() {
    let s = parse("```\ncol\tumn\n```");
    match &s.frames[0] {
        Frame::Code(c) => {
            assert_eq!(c.lines()[0].as_ref(), "col     umn");
            assert!(!c.lines()[0].contains('\t'));
            assert_eq!(c.size, TextSize::Pt11);
        }
        _ => panic!("code"),
    }
}

#[test]
fn block_quote() {
    let s = parse("> the tape");
    assert!(matches!(&s.frames[0], Frame::Quote(q) if !q.frames.is_empty()));
}

#[test]
fn quote_nest_cap() {
    let md = "> a\n> > b\n> > > c\n> > > > d\n";
    assert!(matches!(parse_err(md), Error::Nesting));
}

#[test]
fn bullet_and_ordered_lists() {
    let dash = parse("- one\n- two");
    match &dash.frames[0] {
        Frame::List(l) => {
            assert!(matches!(l.marker, Marker::Dash));
            assert_eq!(l.fit, ListFit::Tight);
            assert_eq!(l.items.len(), 2);
        }
        _ => panic!("list"),
    }
    let ordered = parse("3) alpha\n4) beta");
    match &ordered.frames[0] {
        Frame::List(l) => match l.marker {
            Marker::Decimal { start, delim } => {
                assert_eq!(start, 3);
                assert_eq!(delim, DecimalDelim::Paren);
            }
            Marker::Dash => panic!("decimal"),
        },
        _ => panic!("list"),
    }
}

#[test]
fn loose_list() {
    let s = parse("- a\n\n- b");
    match &s.frames[0] {
        Frame::List(l) => {
            assert_eq!(l.fit, ListFit::Loose);
            assert_eq!(l.items.len(), 2);
        }
        _ => panic!("list"),
    }
}

#[test]
fn blank_between_same_type_items_is_one_loose_list() {
    let s = parse("- a\n- b\n\n- c\n");
    assert_eq!(s.frames.len(), 1, "same-type items do not start a new list");
    match &s.frames[0] {
        Frame::List(l) => {
            assert_eq!(l.fit, ListFit::Loose);
            assert_eq!(l.items.len(), 3);
        }
        _ => panic!("list"),
    }
}

#[test]
fn a_paragraph_breaks_lists() {
    let s = parse("- a\n- b\n\nBreak.\n\n- c\n\n- d\n");
    assert_eq!(s.frames.len(), 3);
    match &s.frames[0] {
        Frame::List(l) => {
            assert_eq!(l.fit, ListFit::Tight);
            assert_eq!(l.items.len(), 2);
        }
        _ => panic!("first list"),
    }
    assert!(matches!(&s.frames[1], Frame::Text(_)));
    match &s.frames[2] {
        Frame::List(l) => {
            assert_eq!(l.fit, ListFit::Loose);
            assert_eq!(l.items.len(), 2);
        }
        _ => panic!("second list"),
    }
}

#[test]
fn empty_list_item_is_a_blank_line() {
    let s = parse("- full\n-\n- also full\n");
    match &s.frames[0] {
        Frame::List(l) => {
            assert_eq!(l.items.len(), 3);
            assert!(matches!(l.items[1].body, ItemBody::Blank));
            assert!(matches!(l.items[0].body, ItemBody::Frames(_)));
            assert!(matches!(l.items[2].body, ItemBody::Frames(_)));
        }
        _ => panic!("list"),
    }
}

#[test]
fn nested_list_item_blocks() {
    let s = parse("- outer\n  - inner");
    match &s.frames[0] {
        Frame::List(l) => {
            assert!(
                l.items[0]
                    .frames()
                    .iter()
                    .any(|f| matches!(f, Frame::List(_)))
            );
        }
        _ => panic!("list"),
    }
}

#[test]
fn list_nest_cap() {
    let md = "- a\n  - b\n    - c\n      - d\n";
    assert!(matches!(parse_err(md), Error::Nesting));
}

#[test]
fn task_list_items() {
    let s = parse("- [ ] foo\n- [x] bar\n");
    match &s.frames[0] {
        Frame::List(l) => {
            assert_eq!(l.items[0].mark, ItemMark::Task { checked: false });
            assert_eq!(l.items[1].mark, ItemMark::Task { checked: true });
        }
        _ => panic!("list"),
    }
}

#[test]
fn nested_task_list() {
    let s = parse("- [x] foo\n  - [ ] bar\n  - [x] baz\n- [ ] bim\n");
    match &s.frames[0] {
        Frame::List(l) => {
            assert_eq!(l.items[0].mark, ItemMark::Task { checked: true });
            assert_eq!(l.items[1].mark, ItemMark::Task { checked: false });
            let inner = l.items[0]
                .frames()
                .iter()
                .find_map(|f| match f {
                    Frame::List(n) => Some(n),
                    _ => None,
                })
                .expect("nested");
            assert_eq!(inner.items[0].mark, ItemMark::Task { checked: false });
            assert_eq!(inner.items[1].mark, ItemMark::Task { checked: true });
        }
        _ => panic!("list"),
    }
}

#[test]
fn image_paragraph_is_a_figure() {
    let img = image::GrayImage::from_pixel(1, 1, image::Luma([0]));
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    let mut s = sheet("![pig](pig.png)", Measure::TAPE, |_| Ok(buf.clone())).unwrap();
    match common::parse::unsource(s.frames.remove(0)) {
        Frame::Figure(fig) => assert!(fig.note().is_none()),
        _ => panic!("figure"),
    }
    assert_eq!(s.note_count(), 0);
}

#[test]
fn figure_dest_resolves_and_keeps_native_size() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let src = image::load_from_memory(&std::fs::read(dir.join("cat.png")).unwrap()).unwrap();
    let mut s = sheet("![cat](./cat.png)", Measure::TAPE, |d| image_bytes(&dir, d)).unwrap();
    match common::parse::unsource(s.frames.remove(0)) {
        Frame::Figure(fig) => {
            assert_eq!(fig.width(), src.width());
            assert_eq!(fig.height(), src.height());
        }
        _ => panic!("figure"),
    }
}

#[test]
fn mixed_text_and_image_is_an_error() {
    assert!(matches!(parse_err("hello ![x](x.png)"), Error::MixedImage));
}
