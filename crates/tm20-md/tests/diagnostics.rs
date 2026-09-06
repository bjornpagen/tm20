mod common;

use tm20_md::{Error, sheet};
use tm20_set::{Measure, compose};

fn error(source: &str) -> Error {
    match sheet(source, Measure::TAPE, |_| Err(Error::Image)) {
        Ok(_) => panic!("expected rejection: {source}"),
        Err(e) => e,
    }
}

#[test]
fn unsupported_heading_has_character_column_and_actionable_reason() {
    let err = error("intro\r\n\r\n## café *old*\r\n");
    let Error::At { location, cause } = &err else {
        panic!("location")
    };
    assert_eq!((location.line, location.column), (3, 9));
    assert!(matches!(**cause, Error::Unsupported { .. }));
    assert!(err.to_string().contains("## café *old*"));
    assert!(err.to_string().contains("help:"));
}

#[test]
fn raw_html_is_located_inside_a_container() {
    let err = error("first\n\n> prose <b>bold</b>\n");
    let Error::At { location, cause } = err else {
        panic!("location")
    };
    assert_eq!((location.line, location.column), (3, 9));
    assert!(matches!(*cause, Error::Html));
}

#[test]
fn tables_never_realign_pad_discard_or_repair() {
    for (source, line) in [
        ("| a | b |\n| :---: | --- |\n", 2),
        ("| a | b |\n| --- | --- |\n| x |\n", 3),
        ("| a | b |\n| --- | --- |\n| x | y | z |\n", 3),
        ("| a | b |\n| --- | --- |\n| `x|y` | z |\n", 3),
    ] {
        let Error::At { location, cause } = error(source) else {
            panic!("location")
        };
        assert_eq!(location.line, line);
        assert!(matches!(*cause, Error::Unsupported { .. }));
    }
}

#[test]
fn display_math_cannot_silently_become_inline_math() {
    for source in [
        "intro\n\n*\\[x^2\\]*",
        "intro\n\n~~\\[x^2\\]~~",
        "intro\n\n[\\[x^2\\]](/math)",
        "intro\n\n| a | \\[x^2\\] |\n| --- | --- |",
    ] {
        let err = error(source);
        assert_eq!(err.location().unwrap().line, 3);
        assert!(err.to_string().contains("nested display math"));
        assert!(err.to_string().contains("help:"));
    }
    for source in [r"*\(x^2\)*", r"> \[x^2\]", r"- \[x^2\]"] {
        sheet(source, Measure::TAPE, |_| Err(Error::Image)).unwrap();
    }
}

#[test]
fn conflicting_destination_titles_are_not_discarded() {
    let err = error("[first](/same \"one\")\n\n[second](/same \"two\")");
    assert_eq!(err.location().unwrap().line, 3);
    assert!(err.to_string().contains("conflicting link titles"));
}

#[test]
fn code_overflow_names_the_exact_source_line() {
    let content = "abcdefghij".repeat(20);
    let source = format!("intro\n\n```\nfine\n{content}\n```\n");
    let sheet = sheet(&source, Measure::TAPE, |_| Err(Error::Image)).unwrap();
    let err = compose(&sheet, &common::table()).unwrap_err();
    assert_eq!(err.code(), "layout.overflow");
    assert_eq!(err.location().unwrap().line, 5);
    assert!(err.to_string().contains(&content));
}

#[test]
fn overflow_locates_text_beyond_the_start_of_a_block() {
    let long = "unbreakable".repeat(20);
    let cases = [
        (format!("intro\n\nshort\n{long}"), 4, 1),
        (
            format!("| a | b |\n| --- | --- |\n| fine | fine |\n| {long} | {long} |"),
            4,
            3,
        ),
    ];
    for (source, line, column) in cases {
        let sheet = sheet(&source, Measure::TAPE, |_| Err(Error::Image)).unwrap();
        let err = compose(&sheet, &common::table()).unwrap_err();
        assert_eq!(err.code(), "layout.overflow");
        let at = err.location().unwrap();
        assert_eq!((at.line, at.column), (line, column));
        assert!(err.to_string().contains(&long));
    }
}

#[test]
fn escaped_brackets_and_code_are_not_broken_references() {
    for source in [
        r"\[^missing]",
        "`[^missing]`",
        r"\[literal]",
        "- [x] done",
        "- [ ] open",
        "&#91;^missing]",
        "`[^missing]` and &#91;^missing]",
    ] {
        sheet(source, Measure::TAPE, |_| Err(Error::Image)).unwrap();
    }
    for source in ["[label][missing]", "[^missing]", "[^under_score]"] {
        assert!(matches!(error(source), Error::At { .. }));
    }
}

#[test]
fn image_failure_keeps_url_and_loader_reason() {
    let Err(err) = sheet(
        "intro\n\n![logo](https://example.com/logo.png)",
        Measure::TAPE,
        |url| {
            Err(Error::Resource {
                destination: url.to_owned(),
                reason: "HTTP 404".into(),
            })
        },
    ) else {
        panic!("load must fail")
    };
    let message = err.to_string();
    assert!(message.contains("line 3, column 1"));
    assert!(message.contains("https://example.com/logo.png"));
    assert!(message.contains("HTTP 404"));
}

#[test]
fn invalid_image_preserves_the_decoder_reason() {
    let Err(err) = sheet("![broken](broken.png)", Measure::TAPE, |_| {
        Ok(b"not a png".to_vec())
    }) else {
        panic!("invalid image must fail")
    };
    assert_eq!(err.location().unwrap().line, 1);
    let message = err.to_string();
    assert!(message.contains("broken.png"));
    assert!(message.contains("image codec failed:"));
    assert!(message.contains("help:"));
}

#[test]
fn missing_glyph_and_overflow_keep_markdown_origin() {
    let faces = common::table();
    for body in ["😀".to_owned(), "unbreakable".repeat(20)] {
        let source = format!("ok\n\n{body}");
        let sheet = sheet(&source, Measure::TAPE, |_| Err(Error::Image)).unwrap();
        let err = compose(&sheet, &faces).unwrap_err();
        let tm20_set::Error::At { location, cause } = err else {
            panic!("layout location")
        };
        assert_eq!(location.line, 3);
        assert!(matches!(
            *cause,
            tm20_set::Error::MissingGlyph { .. } | tm20_set::Error::Clipped { .. }
        ));
    }
}
