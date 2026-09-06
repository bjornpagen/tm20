//! Shared Markdown → Sheet observation.

use tm20_md::{Error, sheet};
use tm20_set::{Cut, Frame, Measure, Span};

pub fn parse(md: &str) -> tm20_set::Sheet<'static> {
    let mut sheet = sheet(md, Measure::TAPE, |_| Err(Error::Image)).unwrap();
    sheet.frames = sheet.frames.into_iter().map(unsource).collect();
    sheet
}

pub fn unsource(frame: Frame<'static>) -> Frame<'static> {
    match frame {
        Frame::Source { frame, .. } => unsource(*frame),
        Frame::Quote(mut quote) => {
            quote.frames = quote.frames.into_iter().map(unsource).collect();
            Frame::Quote(quote)
        }
        Frame::List(mut list) => {
            for item in &mut list.items {
                let old = std::mem::replace(&mut item.body, tm20_set::ItemBody::Blank);
                item.body = match old {
                    tm20_set::ItemBody::Blank => tm20_set::ItemBody::Blank,
                    tm20_set::ItemBody::Frames(frames) => {
                        tm20_set::ItemBody::from_frames(frames.into_iter().map(unsource).collect())
                    }
                };
            }
            Frame::List(list)
        }
        other => other,
    }
}

fn cause(e: Error) -> Error {
    match e {
        Error::At { cause: inner, .. } => cause(*inner),
        other => other,
    }
}

pub fn parse_err(md: &str) -> Error {
    match sheet(md, Measure::TAPE, |_| Err(Error::Image)) {
        Err(e) => cause(e),
        Ok(_) => panic!("expected a lowering error"),
    }
}

pub fn span_cut(s: &Span<'_>) -> Cut {
    match s {
        Span::Type { cut, .. } => *cut,
        Span::Math(_) => panic!("math"),
        Span::Note(_) => panic!("note"),
        Span::Strike(_) => panic!("strike"),
    }
}

pub fn span_text<'a>(s: &'a Span<'_>) -> &'a str {
    match s {
        Span::Type { text, .. } => text,
        Span::Math(_) => "",
        Span::Note(_) => "",
        Span::Strike(_) => "",
    }
}

pub fn span_note(s: &Span<'_>) -> Option<u32> {
    match s {
        Span::Note(id) => Some(id.get_u32()),
        Span::Type { .. } | Span::Math(_) | Span::Strike(_) => None,
    }
}

pub fn text_runs(sheet: &tm20_set::Sheet<'_>) -> Vec<(Cut, String)> {
    match &sheet.frames[..] {
        [Frame::Text(b)] => b
            .spans
            .iter()
            .filter_map(|s| match s {
                Span::Type { cut, text, .. } => Some((*cut, text.to_string())),
                Span::Note(_) => None,
                Span::Math(_) => panic!("math"),
                Span::Strike(_) => panic!("strike"),
            })
            .collect(),
        other => panic!("expected one Text, got {} frames", other.len()),
    }
}
