//! Lowering proofs. One markdown snippet per spec subsection. Not HTML goldens.

use tm20_md::{sheet, Error};
use tm20_set::{ColAlign, Cut, DecimalDelim, Frame, Marker, Measure, Note, TextSize, Thickness};

fn parse(md: &str) -> tm20_set::Sheet<'static> {
    sheet(md, Measure::TAPE, |_| Err(Error::Image)).unwrap()
}

fn parse_err(md: &str) -> Error {
    match sheet(md, Measure::TAPE, |_| Err(Error::Image)) {
        Err(e) => e,
        Ok(_) => panic!("expected a lowering error"),
    }
}

fn text_runs(sheet: &tm20_set::Sheet<'_>) -> Vec<(Cut, String)> {
    match &sheet.frames[..] {
        [Frame::Text(b)] => b
            .spans
            .iter()
            .map(|s| (s.cut, s.text.as_ref().to_string()))
            .collect(),
        other => panic!("expected one Text, got {} frames", other.len()),
    }
}

#[test]
fn empty_and_paragraph() {
    let empty = parse("");
    assert!(empty.frames.is_empty());
    let p = parse("Hello");
    assert_eq!(text_runs(&p), vec![(Cut::Roman, "Hello".into())]);
}

#[test]
fn backslash_and_entity() {
    let s = parse("\\*star\\* and &amp;");
    assert_eq!(text_runs(&s), vec![(Cut::Roman, "*star* and &".into())]);
}

#[test]
fn thematic_break() {
    let s = parse("Hello\n\n---\n\nThere");
    assert!(matches!(
        s.frames[1],
        Frame::Rule(ref r) if r.thickness == Thickness::Two
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
fn heading_inlines_flatten() {
    let s = parse("## Hello *world*");
    assert!(matches!(&s.frames[0], Frame::Head(h) if h.text == "Hello world"));
}

#[test]
fn link_definition_is_not_rendered() {
    let s = parse("[foo]: /url\n\nSee [foo].");
    assert_eq!(s.frames.len(), 1);
    assert!(matches!(s.frames[0], Frame::Text(_)));
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
    let fenced = parse("```\nfn measure() -> u16 { 576 }\n```");
    assert!(matches!(
        &fenced.frames[0],
        Frame::Code(c) if c.lines.iter().any(|l| l.contains("fn measure"))
    ));
    let indented = parse("    let x = 1;\n");
    assert!(matches!(&indented.frames[0], Frame::Code(_)));
}

#[test]
fn code_span_is_mono() {
    let s = parse("*emph `code` emph*");
    assert_eq!(
        text_runs(&s),
        vec![
            (Cut::Italic, "emph ".into()),
            (Cut::Mono, "code".into()),
            (Cut::Italic, " emph".into()),
        ]
    );
}

#[test]
fn prose_curls_quotes_and_code_stays_straight() {
    let s = parse("It's \"print,\" not typewriter.");
    assert_eq!(
        text_runs(&s),
        vec![(Cut::Roman, "It’s “print,” not typewriter.".into())]
    );
    let code = parse("see `it's \"x\"` done");
    assert_eq!(
        text_runs(&code),
        vec![
            (Cut::Roman, "see ".into()),
            (Cut::Mono, "it's \"x\"".into()),
            (Cut::Roman, " done".into()),
        ]
    );
    let fenced = parse("```\nit's \"x\"\n```");
    match &fenced.frames[0] {
        Frame::Code(c) => assert!(c.lines.iter().any(|l| l.contains("it's \"x\""))),
        _ => panic!("code"),
    }
}

#[test]
fn html_block_and_inline_are_errors() {
    assert!(matches!(parse_err("<div>no</div>"), Error::Html));
    assert!(matches!(parse_err("a <span>b</span> c"), Error::Html));
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
            assert!(l.tight);
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
            _ => panic!("decimal"),
        },
        _ => panic!("list"),
    }
}

#[test]
fn loose_list() {
    let s = parse("- a\n\n- b");
    match &s.frames[0] {
        Frame::List(l) => {
            assert!(!l.tight);
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
            assert!(!l.tight);
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
            assert!(l.tight);
            assert_eq!(l.items.len(), 2);
        }
        _ => panic!("first list"),
    }
    assert!(matches!(&s.frames[1], Frame::Text(_)));
    match &s.frames[2] {
        Frame::List(l) => {
            assert!(!l.tight);
            assert_eq!(l.items.len(), 2);
        }
        _ => panic!("second list"),
    }
}

#[test]
fn nested_list_item_blocks() {
    let s = parse("- outer\n  - inner");
    match &s.frames[0] {
        Frame::List(l) => {
            assert!(l.items[0]
                .frames
                .iter()
                .any(|f| matches!(f, Frame::List(_))));
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
fn emphasis_and_strong() {
    let s = parse("a *i* and **b** and ***both***");
    let runs = text_runs(&s);
    assert!(runs
        .iter()
        .any(|(c, t)| *c == Cut::Italic && t.contains('i')));
    assert!(runs.iter().any(|(c, t)| *c == Cut::Bold && t.contains('b')));
    assert!(runs
        .iter()
        .any(|(c, t)| *c == Cut::BoldItalic && t.contains("both")));
}

#[test]
fn link_is_italic_with_a_note() {
    let s = parse("See [the canon](https://example.com/canon).");
    let runs = text_runs(&s);
    let link = runs
        .iter()
        .find(|(c, t)| *c == Cut::Italic && t.contains("canon"))
        .unwrap();
    match &s.frames[0] {
        Frame::Text(b) => {
            assert!(b.spans.iter().any(|sp| sp.note.map(|n| n.get()) == Some(1)));
        }
        _ => panic!("text"),
    }
    assert_eq!(s.notes.len(), 1);
    assert!(matches!(&s.notes[0], Note::Dest(d) if d == "https://example.com/canon"));
    let _ = link;
}

#[test]
fn same_destination_reuses_the_number() {
    let s = parse("[a](/x) then [b](/x)");
    match &s.frames[0] {
        Frame::Text(b) => {
            let nums: Vec<_> = b
                .spans
                .iter()
                .filter_map(|sp| sp.note.map(|n| n.get()))
                .collect();
            assert_eq!(nums, vec![1, 1]);
        }
        _ => panic!("text"),
    }
    assert_eq!(s.notes.len(), 1);
}

#[test]
fn autolink_has_no_note() {
    let s = parse("See <https://example.com>.");
    match &s.frames[0] {
        Frame::Text(b) => {
            assert!(b.spans.iter().all(|sp| sp.note.is_none()));
            assert!(b.spans.iter().any(|sp| sp.cut == Cut::Italic));
        }
        _ => panic!("text"),
    }
    assert!(s.notes.is_empty());
}

#[test]
fn hard_and_soft_breaks() {
    let hard = parse("a  \nb");
    match &hard.frames[0] {
        Frame::Text(b) => {
            let t: String = b.spans.iter().map(|s| s.text.as_ref()).collect();
            assert!(t.contains('\n'), "{t:?}");
        }
        _ => panic!("text"),
    }
    let soft = parse("a\nb");
    match &soft.frames[0] {
        Frame::Text(b) => {
            let t: String = b.spans.iter().map(|s| s.text.as_ref()).collect();
            assert!(!t.contains('\n'));
            assert!(t.contains(' '));
        }
        _ => panic!("text"),
    }
}

#[test]
fn image_paragraph_is_a_figure() {
    let img = image::GrayImage::from_pixel(1, 1, image::Luma([0]));
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    let s = sheet("![pig](pig.png)", Measure::TAPE, |_| Ok(buf.clone())).unwrap();
    assert!(matches!(s.frames[0], Frame::Figure(_)));
}

#[test]
fn mixed_text_and_image_is_an_error() {
    assert!(matches!(parse_err("hello ![x](x.png)"), Error::MixedImage));
}

#[test]
fn pipe_table() {
    let s = parse("| a | b |\n| :---: | ---: |\n| 1 | 2 |\n");
    match &s.frames[0] {
        Frame::Cols(c) => {
            assert_eq!(c.align, vec![ColAlign::Start, ColAlign::End]);
            assert_eq!(c.rows.len(), 2);
            assert_eq!(c.rows[0][0][0].cut, Cut::Bold);
            assert_eq!(c.size, TextSize::Pt11);
        }
        _ => panic!("cols"),
    }
}

#[test]
fn one_column_table_is_an_error() {
    assert!(matches!(parse_err("| a |\n| --- |\n| 1 |\n"), Error::Cols));
}

#[test]
fn strikethrough_is_plain_text() {
    let s = parse("~~no~~");
    assert_eq!(text_runs(&s), vec![(Cut::Roman, "~~no~~".into())]);
}

#[test]
fn bare_autolink_is_italic_without_a_note() {
    let s = parse("See https://example.com now.");
    match &s.frames[0] {
        Frame::Text(b) => {
            assert!(b.spans.iter().all(|sp| sp.note.is_none()));
            assert!(b
                .spans
                .iter()
                .any(|sp| sp.cut == Cut::Italic && sp.text.contains("example.com")));
        }
        _ => panic!("text"),
    }
    assert!(s.notes.is_empty());
}

#[test]
fn task_list_items() {
    let s = parse("- [ ] foo\n- [x] bar\n");
    match &s.frames[0] {
        Frame::List(l) => {
            assert_eq!(l.items[0].task, Some(false));
            assert_eq!(l.items[1].task, Some(true));
        }
        _ => panic!("list"),
    }
}

#[test]
fn nested_task_list() {
    let s = parse("- [x] foo\n  - [ ] bar\n  - [x] baz\n- [ ] bim\n");
    match &s.frames[0] {
        Frame::List(l) => {
            assert_eq!(l.items[0].task, Some(true));
            assert_eq!(l.items[1].task, Some(false));
            let inner = l.items[0]
                .frames
                .iter()
                .find_map(|f| match f {
                    Frame::List(n) => Some(n),
                    _ => None,
                })
                .expect("nested");
            assert_eq!(inner.items[0].task, Some(false));
            assert_eq!(inner.items[1].task, Some(true));
        }
        _ => panic!("list"),
    }
}

#[test]
fn footnotes_share_the_link_registry() {
    let s = parse("See [canon](https://example.com) and a note.[^x]\n\n[^x]: Ruder.\n");
    match &s.frames[0] {
        Frame::Text(b) => {
            let nums: Vec<_> = b
                .spans
                .iter()
                .filter_map(|sp| sp.note.map(|n| n.get()))
                .collect();
            assert_eq!(nums, vec![1, 2]);
        }
        _ => panic!("text"),
    }
    assert_eq!(s.notes.len(), 2);
    assert!(matches!(&s.notes[0], Note::Dest(d) if d == "https://example.com"));
    assert!(matches!(&s.notes[1], Note::Blocks(f) if !f.is_empty()));
}

#[test]
fn footnote_reuses_the_number() {
    let s = parse("A[^x] then B[^x].\n\n[^x]: Once.\n");
    match &s.frames[0] {
        Frame::Text(b) => {
            let nums: Vec<_> = b
                .spans
                .iter()
                .filter_map(|sp| sp.note.map(|n| n.get()))
                .collect();
            assert_eq!(nums, vec![1, 1]);
        }
        _ => panic!("text"),
    }
    assert_eq!(s.notes.len(), 1);
}

#[test]
fn undefined_footnote_stays_literal() {
    let s = parse("Hi[^missing].");
    let t: String = match &s.frames[0] {
        Frame::Text(b) => b.spans.iter().map(|sp| sp.text.as_ref()).collect(),
        _ => panic!("text"),
    };
    assert!(t.contains("[^missing]"), "{t:?}");
    assert!(s.notes.is_empty());
}
