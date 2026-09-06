//! Inline Markdown semantics: emphasis, code, breaks, punctuation, math, HTML.

mod common;

use tm20_md::Error;
use tm20_set::{Cut, Frame, Measure, Span};

use common::{parse, parse_err, span_text, text_runs};

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
fn emphasis_and_strong() {
    let s = parse("a *i* and **b** and ***both***");
    let runs = text_runs(&s);
    assert!(
        runs.iter()
            .any(|(c, t)| *c == Cut::Italic && t.contains('i'))
    );
    assert!(runs.iter().any(|(c, t)| *c == Cut::Bold && t.contains('b')));
    assert!(
        runs.iter()
            .any(|(c, t)| *c == Cut::BoldItalic && t.contains("both"))
    );
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
fn code_span_keeps_surrounding_spaces() {
    let one = parse("see `  x  ` done");
    assert_eq!(
        text_runs(&one),
        vec![
            (Cut::Roman, "see ".into()),
            (Cut::Mono, " x ".into()),
            (Cut::Roman, " done".into()),
        ]
    );
    let zero = parse("see `x` done");
    assert_eq!(
        text_runs(&zero),
        vec![
            (Cut::Roman, "see ".into()),
            (Cut::Mono, "x".into()),
            (Cut::Roman, " done".into()),
        ]
    );
    let all_space = parse("see `   ` done");
    let runs = text_runs(&all_space);
    assert!(
        runs.iter().any(|(c, t)| *c == Cut::Mono && t.contains(' ')),
        "all-space code is not stripped to empty: {runs:?}"
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
        Frame::Code(c) => assert!(c.lines().iter().any(|l| l.contains("it's \"x\""))),
        _ => panic!("code"),
    }
}

#[test]
fn html_block_and_inline_are_errors() {
    assert!(matches!(parse_err("<div>no</div>"), Error::Html));
    assert!(matches!(parse_err("a <span>b</span> c"), Error::Html));
}

#[test]
fn hard_and_soft_breaks() {
    let hard = parse("a  \nb");
    match &hard.frames[0] {
        Frame::Text(b) => {
            let t: String = b.spans.iter().map(span_text).collect();
            assert!(t.contains('\n'), "{t:?}");
        }
        _ => panic!("text"),
    }
    let soft = parse("a\nb");
    match &soft.frames[0] {
        Frame::Text(b) => {
            let t: String = b.spans.iter().map(span_text).collect();
            assert!(!t.contains('\n'));
            assert!(t.contains(' '));
        }
        _ => panic!("text"),
    }
}

#[test]
fn ellipsis_is_one_glyph() {
    let s = parse("wait...");
    assert_eq!(text_runs(&s), vec![(Cut::Roman, "wait…".into())]);
    let code = parse("see `...` done");
    assert_eq!(
        text_runs(&code),
        vec![
            (Cut::Roman, "see ".into()),
            (Cut::Mono, "...".into()),
            (Cut::Roman, " done".into()),
        ]
    );
}

#[test]
fn strikethrough_preserves_nested_inline_styles() {
    let s = parse("~~no **bold** `code` [link](https://example.com)~~ yes");
    let Frame::Text(block) = &s.frames[0] else {
        panic!("text")
    };
    let Span::Strike(inner) = &block.spans[0] else {
        panic!("strike")
    };
    assert!(
        inner
            .iter()
            .any(|s| matches!(s, Span::Type { cut: Cut::Bold, text } if text == "bold"))
    );
    assert!(
        inner
            .iter()
            .any(|s| matches!(s, Span::Type { cut: Cut::Mono, text } if text == "code"))
    );
    assert!(inner.iter().any(|s| matches!(s, Span::Note(_))));
    assert!(matches!(&block.spans[1], Span::Type { text, .. } if text == " yes"));
}

#[test]
fn strikethrough_delimiters_remain_literal_in_code() {
    let s = parse("`~~no~~`");
    assert_eq!(text_runs(&s), vec![(Cut::Mono, "~~no~~".into())]);
}

#[test]
fn strikethrough_in_table_headers_keeps_header_weight() {
    let s = parse("| ~~old~~ | new |\n| --- | --- |\n");
    let Frame::Cols(cols) = &s.frames[0] else {
        panic!("table")
    };
    match &cols.body {
        tm20_set::ColBody::Two { rows, .. } => {
            let Span::Strike(inner) = &rows[0][0][0] else {
                panic!("strike")
            };
            assert!(matches!(&inner[0], Span::Type { cut: Cut::Bold, .. }));
        }
        tm20_set::ColBody::Three { .. } => panic!("two columns"),
    }
}

#[test]
fn dollars_are_currency() {
    let s = parse("espresso is $4.50");
    assert_eq!(
        text_runs(&s),
        vec![(Cut::Roman, "espresso is $4.50".into())]
    );
}

#[test]
fn inline_frac_box_has_tex_depth() {
    let s = parse("tall \\(\\frac{a}{b}\\)");
    match &s.frames[0] {
        Frame::Text(b) => match b.spans.iter().find(|s| matches!(s, Span::Math(_))) {
            Some(Span::Math(m)) => {
                assert!(m.tex_depth() > 0, "frac depth is parsed, not leftover PNG");
                assert!(
                    m.tex_ascent() + m.tex_depth() >= 20,
                    "TeX box is taller than a letter"
                );
            }
            _ => panic!("frac"),
        },
        _ => panic!("text"),
    }
}

#[test]
fn inline_math_is_a_raster_box() {
    let s = parse("see \\(1+2\\) now");
    match &s.frames[..] {
        [Frame::Text(b)] => {
            assert_eq!(
                b.spans.len(),
                3,
                "{:?}",
                b.spans.iter().map(span_text).collect::<Vec<_>>()
            );
            assert_eq!(span_text(&b.spans[0]), "see ");
            match &b.spans[1] {
                Span::Math(m) => {
                    assert!(m.width() > 0 && m.height() > 0);
                    assert!(m.tex_ascent() <= m.height());
                }
                Span::Type { .. } | Span::Note(_) | Span::Strike(_) => panic!("inline math"),
            }
            assert_eq!(span_text(&b.spans[2]), " now");
        }
        other => panic!("expected one Text, got {} frames", other.len()),
    }
}

#[test]
fn display_math_breaks_the_paragraph() {
    let s = parse("see \\[x+y\\] then");
    assert_eq!(s.frames.len(), 3, "text, display, text");
    match &s.frames[0] {
        Frame::Text(b) => assert_eq!(span_text(&b.spans[0]), "see "),
        _ => panic!("lead"),
    }
    match &s.frames[1] {
        Frame::Math(m) => {
            assert!(m.width() > 0 && m.width() < u32::from(Measure::TAPE.get()));
        }
        _ => panic!("display"),
    }
    match &s.frames[2] {
        Frame::Text(b) => assert_eq!(span_text(&b.spans[0]), " then"),
        _ => panic!("trail"),
    }
}

#[test]
fn bad_latex_is_an_error() {
    assert!(matches!(parse_err("\\(\\frac{\\)"), Error::MathDetail(_)));
}

#[test]
fn nul_is_rejected_not_replaced() {
    assert!(matches!(
        parse_err("before\u{0}after"),
        Error::Unsupported { .. }
    ));
}

#[test]
fn code_in_heading_is_rejected_not_flattened() {
    assert!(matches!(
        parse_err("# hello `<b>`"),
        Error::Unsupported { .. }
    ));
}
