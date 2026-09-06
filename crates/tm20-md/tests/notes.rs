//! Link, autolink, and footnote note-registry semantics.

mod common;

use tm20_set::{Cut, Frame, Note, Span};

use common::{parse, span_cut, span_note, span_text};

#[test]
fn link_definition_is_not_rendered() {
    let s = parse("[foo]: /url\n\nSee [foo].");
    assert_eq!(s.frames.len(), 1);
    assert!(matches!(s.frames[0], Frame::Text(_)));
}

#[test]
fn link_is_italic_with_a_note() {
    let s = parse("See [the canon](https://example.com/canon).");
    let runs = common::text_runs(&s);
    let link = runs
        .iter()
        .find(|(c, t)| *c == Cut::Italic && t.contains("canon"))
        .unwrap();
    match &s.frames[0] {
        Frame::Text(b) => {
            assert!(b.spans.iter().any(|sp| span_note(sp) == Some(1)));
        }
        _ => panic!("text"),
    }
    assert_eq!(s.note_count(), 1);
    assert!(matches!(
        &s.notes()[0],
        Note::Dest { dest, title: None, .. } if dest == "https://example.com/canon"
    ));
    let _ = link;
}

#[test]
fn link_title_is_the_work_then_the_location() {
    let s = parse("[the canon](https://example.com/canon.pdf \"The Vignelli Canon\")");
    assert!(matches!(
        &s.notes()[0],
        Note::Dest { dest, title: Some(t), .. }
            if dest == "https://example.com/canon.pdf" && t == "The Vignelli Canon"
    ));
}

#[test]
fn same_destination_reuses_the_number() {
    let s = parse("[a](/x) then [b](/x)");
    match &s.frames[0] {
        Frame::Text(b) => {
            let nums: Vec<_> = b.spans.iter().filter_map(span_note).collect();
            assert_eq!(nums, vec![1, 1]);
        }
        _ => panic!("text"),
    }
    assert_eq!(s.note_count(), 1);
}

#[test]
fn autolink_has_no_note() {
    let s = parse("See <https://example.com>.");
    match &s.frames[0] {
        Frame::Text(b) => {
            assert!(b.spans.iter().all(|sp| span_note(sp).is_none()));
            assert!(b.spans.iter().any(|sp| span_cut(sp) == Cut::Italic));
        }
        _ => panic!("text"),
    }
    assert_eq!(s.note_count(), 0);
}

#[test]
fn email_autolink_is_italic_without_a_note() {
    let s = parse("Write <foo@bar.com>.");
    match &s.frames[0] {
        Frame::Text(b) => {
            assert!(b.spans.iter().any(|sp| span_cut(sp) == Cut::Italic));
            assert!(b.spans.iter().all(|sp| span_note(sp).is_none()));
        }
        _ => panic!("text"),
    }
    assert_eq!(s.note_count(), 0);
}

#[test]
fn labeled_mailto_still_has_a_note() {
    let s = parse("[email me](mailto:a@b.com)");
    match &s.frames[0] {
        Frame::Text(b) => {
            assert!(b.spans.iter().any(|sp| span_note(sp) == Some(1)));
        }
        _ => panic!("text"),
    }
    assert!(matches!(
        &s.notes()[0],
        Note::Dest { dest, title: None, .. } if dest == "a@b.com"
    ));
}

#[test]
fn bare_autolink_is_italic_without_a_note() {
    let s = parse("See https://example.com now.");
    match &s.frames[0] {
        Frame::Text(b) => {
            assert!(b.spans.iter().all(|sp| span_note(sp).is_none()));
            assert!(
                b.spans
                    .iter()
                    .any(|sp| span_cut(sp) == Cut::Italic && span_text(sp).contains("example.com"))
            );
        }
        _ => panic!("text"),
    }
    assert_eq!(s.note_count(), 0);
}

#[test]
fn bare_www_autolink_gets_no_note() {
    let s = parse("Go to www.example.com now");
    assert_eq!(s.note_count(), 0, "a bare autolink carries no note");
    match &s.frames[0] {
        Frame::Text(b) => assert!(b.spans.iter().all(|sp| span_note(sp).is_none())),
        _ => panic!("text"),
    }
}

#[test]
fn footnotes_share_the_link_registry() {
    let s = parse("See [canon](https://example.com) and a note.[^x]\n\n[^x]: Ruder.\n");
    match &s.frames[0] {
        Frame::Text(b) => {
            let nums: Vec<_> = b.spans.iter().filter_map(span_note).collect();
            assert_eq!(nums, vec![1, 2]);
        }
        _ => panic!("text"),
    }
    assert_eq!(s.note_count(), 2);
    assert!(matches!(
        &s.notes()[0],
        Note::Dest { dest, title: None, .. } if dest == "https://example.com"
    ));
    assert!(matches!(&s.notes()[1], Note::Blocks(f) if !f.is_empty()));
}

#[test]
fn footnote_reuses_the_number() {
    let s = parse("A[^x] then B[^x].\n\n[^x]: Once.\n");
    match &s.frames[0] {
        Frame::Text(b) => {
            let nums: Vec<_> = b.spans.iter().filter_map(span_note).collect();
            assert_eq!(nums, vec![1, 1]);
        }
        _ => panic!("text"),
    }
    assert_eq!(s.note_count(), 1);
}

#[test]
fn undefined_footnote_is_rejected() {
    assert!(matches!(
        common::parse_err("Hi[^missing]."),
        tm20_md::Error::Unsupported { .. }
    ));
}

#[test]
fn link_with_inner_footnote_emits_two_markers() {
    let s = parse("[label[^a]](https://example.com)\n\n[^a]: note\n");
    match &s.frames[0] {
        Frame::Text(b) => {
            let nums: Vec<_> = b.spans.iter().filter_map(span_note).collect();
            assert_eq!(
                nums,
                vec![1, 2],
                "footnote inside a link precedes the destination: {nums:?}"
            );
            assert!(
                b.spans.iter().any(|sp| matches!(sp, Span::Note(_))),
                "notes are independent Span::Note nodes"
            );
        }
        _ => panic!("text"),
    }
    assert_eq!(s.note_count(), 2);
}

#[test]
fn math_ending_link_still_gets_a_destination_marker() {
    let s = parse("[\\(x\\)](https://example.com)");
    match &s.frames[0] {
        Frame::Text(b) => {
            assert!(b.spans.iter().any(|sp| matches!(sp, Span::Math(_))));
            let nums: Vec<_> = b.spans.iter().filter_map(span_note).collect();
            assert_eq!(nums, vec![1], "destination marker follows math: {nums:?}");
        }
        _ => panic!("text"),
    }
}

#[test]
fn three_deep_note_lists_parse() {
    let s = parse("A[^a]\n\n[^a]:\n    - one\n      - two\n        - three\n");
    assert_eq!(s.note_count(), 1);
    s.admit().unwrap();
}

#[test]
fn four_deep_note_lists_reject() {
    let err = common::parse_err(
        "A[^a]\n\n[^a]:\n    - one\n      - two\n        - three\n          - four\n",
    );
    assert!(matches!(
        err,
        tm20_md::Error::Nesting | tm20_md::Error::Set(_)
    ));
}
