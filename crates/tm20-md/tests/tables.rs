//! Strict GFM table semantics.

mod common;

use tm20_md::Error;
use tm20_set::{ColAlign, ColBody, Cut, Frame, TextSize};

use common::{parse, parse_err, span_cut, span_note, span_text, text_runs};

#[test]
fn pipe_table() {
    let s = parse("| a | b |\n| :--- | ---: |\n| 1 | 2 |\n");
    match &s.frames[0] {
        Frame::Cols(c) => {
            match &c.body {
                ColBody::Two { align, rows } => {
                    assert_eq!(align, &[ColAlign::Start, ColAlign::End]);
                    assert_eq!(rows.len(), 2);
                    assert_eq!(span_cut(&rows[0][0][0]), Cut::Bold);
                }
                ColBody::Three { .. } => panic!("two"),
            }
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
fn four_column_table_is_an_error() {
    assert!(matches!(
        parse_err("| a | b | c | d |\n| --- | --- | --- | --- |\n| 1 | 2 | 3 | 4 |\n"),
        Error::Cols
    ));
}

#[test]
fn pipe_inside_code_span_is_cell_content() {
    let s = parse("| escaped | code |\n| :--- | :--- |\n| a \\| b | `a\\|b` |\n");
    match &s.frames[0] {
        Frame::Cols(c) => match &c.body {
            ColBody::Two { rows, .. } => {
                let start: String = rows[1][0].iter().map(span_text).collect();
                let code: String = rows[1][1].iter().map(span_text).collect();
                assert!(start.contains('|'), "{start}");
                assert_eq!(code, "a|b", "{code:?}");
            }
            ColBody::Three { .. } => panic!("two"),
        },
        _ => panic!("cols"),
    }
}

#[test]
fn footnote_in_a_cell_still_gets_a_note() {
    let s = parse("| a | b |\n| :--- | :--- |\n| cell[^c] | x |\n\n[^c]: From a cell.\n");
    match &s.frames[0] {
        Frame::Cols(c) => match &c.body {
            ColBody::Two { rows, .. } => {
                let notes: Vec<_> = rows[1][0].iter().filter_map(span_note).collect();
                assert_eq!(
                    notes,
                    vec![1],
                    "{:?}",
                    rows[1][0].iter().map(span_text).collect::<Vec<_>>()
                );
            }
            ColBody::Three { .. } => panic!("two"),
        },
        _ => panic!("cols"),
    }
    assert_eq!(s.note_count(), 1);
}

#[test]
fn table_reference_link_resolves_from_the_document() {
    let s = parse("| a | b |\n|---|---|\n| [label][ref] | ok |\n\n[ref]: https://example.com\n");
    match &s.frames[0] {
        Frame::Cols(c) => match &c.body {
            ColBody::Two { rows, .. } => {
                let notes: Vec<_> = rows[1][0].iter().filter_map(span_note).collect();
                assert_eq!(notes, vec![1], "reference in a cell must allocate a note");
                let literal: String = rows[1][0].iter().map(span_text).collect();
                assert!(
                    !literal.contains("[ref]"),
                    "must not keep literal reference syntax: {literal}"
                );
            }
            ColBody::Three { .. } => panic!("two"),
        },
        _ => panic!("cols"),
    }
    assert_eq!(s.note_count(), 1);
}

#[test]
fn quoted_table_keeps_its_cells() {
    let s = parse("> a | b\n> --- | ---\n> hello | world\n");
    match &s.frames[0] {
        Frame::Quote(q) => match &q.frames[0] {
            Frame::Cols(c) => match &c.body {
                ColBody::Two { rows, .. } => {
                    let hello: String = rows[1][0].iter().map(span_text).collect();
                    let world: String = rows[1][1].iter().map(span_text).collect();
                    assert!(hello.contains("hello"), "{hello}");
                    assert!(world.contains("world"), "{world}");
                }
                ColBody::Three { .. } => panic!("two"),
            },
            _ => panic!("expected cols in quote"),
        },
        _ => panic!("quote"),
    }
}

#[test]
fn list_table_keeps_cells() {
    let s = parse("- | a | b |\n  | --- | --- |\n  | hello | world |\n");
    match &s.frames[0] {
        Frame::List(l) => {
            let cols = l.items[0]
                .frames()
                .iter()
                .find_map(|f| match f {
                    Frame::Cols(c) => Some(c),
                    _ => None,
                })
                .expect("table in list");
            match &cols.body {
                ColBody::Two { rows, .. } => {
                    let hello: String = rows[1][0].iter().map(span_text).collect();
                    assert!(hello.contains("hello"), "{hello}");
                }
                ColBody::Three { .. } => panic!("two"),
            }
        }
        _ => panic!("list"),
    }
}

#[test]
fn block_looking_cells_stay_literal_inline() {
    let s = parse("| a | b |\n| --- | --- |\n| # hello | - hello |\n");
    match &s.frames[0] {
        Frame::Cols(c) => match &c.body {
            ColBody::Two { rows, .. } => {
                let a: String = rows[1][0].iter().map(span_text).collect();
                let b: String = rows[1][1].iter().map(span_text).collect();
                assert!(a.contains("# hello") || a.contains("hello"), "{a}");
                assert!(b.contains("hello"), "{b}");
                assert!(
                    !a.is_empty() && !b.is_empty(),
                    "cells must not be discarded"
                );
            }
            ColBody::Three { .. } => panic!("two"),
        },
        _ => panic!("cols"),
    }
}

#[test]
fn header_style_is_contextual() {
    let s = parse("| `x` | *y* | **z** |\n| --- | --- | --- |\n| a | b | c |\n");
    match &s.frames[0] {
        Frame::Cols(c) => match &c.body {
            ColBody::Three { rows, .. } => {
                let cuts: Vec<Cut> = rows[0]
                    .iter()
                    .flat_map(|cell| cell.iter())
                    .filter_map(|sp| match sp {
                        tm20_set::Span::Type { cut, .. } => Some(*cut),
                        _ => None,
                    })
                    .collect();
                assert!(
                    cuts.contains(&Cut::Mono),
                    "header code stays Mono: {cuts:?}"
                );
                assert!(
                    cuts.contains(&Cut::BoldItalic),
                    "header italic becomes BoldItalic: {cuts:?}"
                );
                assert!(
                    cuts.contains(&Cut::Bold),
                    "already-bold header stays bold: {cuts:?}"
                );
            }
            ColBody::Two { .. } => panic!("three"),
        },
        _ => panic!("cols"),
    }
}

#[test]
fn header_code_span_keeps_spaces() {
    let s = parse("| `  x  ` | y |\n| --- | --- |\n| a | b |\n");
    match &s.frames[0] {
        Frame::Cols(c) => match &c.body {
            ColBody::Two { rows, .. } => {
                let code: String = rows[0][0]
                    .iter()
                    .filter(|sp| matches!(sp, tm20_set::Span::Type { cut: Cut::Mono, .. }))
                    .map(span_text)
                    .collect();
                assert_eq!(code, " x ", "header code is not double-stripped: {code:?}");
            }
            ColBody::Three { .. } => panic!("two"),
        },
        _ => panic!("cols"),
    }
}

#[test]
fn ordinary_non_table_code_with_pipes_is_unchanged() {
    let s = parse("see `a|b` and `c\\|d` done");
    assert_eq!(
        text_runs(&s),
        vec![
            (Cut::Roman, "see ".into()),
            (Cut::Mono, "a|b".into()),
            (Cut::Roman, " and ".into()),
            (Cut::Mono, "c\\|d".into()),
            (Cut::Roman, " done".into()),
        ]
    );
}

#[test]
fn fenced_code_pipes_are_not_inline_protected() {
    let s = parse("```\na|b\n```");
    match &s.frames[0] {
        Frame::Code(c) => assert!(c.lines().iter().any(|l| l.contains("a|b"))),
        _ => panic!("code"),
    }
}
