//! Inline lowering. Code boxes stay separate; notes are independent spans.

use comrak::nodes::{AstNode, NodeValue};
use tm20_set::{Cut, Span};

use crate::error::Error;
use crate::math;

use super::{Cx, ellipsis};

#[derive(Clone, Copy)]
pub(super) enum Voice {
    Roman,
    Italic,
    Bold,
    BoldItalic,
}

pub(super) fn cut(v: Voice) -> Cut {
    match v {
        Voice::Roman => Cut::Roman,
        Voice::Italic => Cut::Italic,
        Voice::Bold => Cut::Bold,
        Voice::BoldItalic => Cut::BoldItalic,
    }
}

pub(super) fn emph(v: Voice) -> Voice {
    match v {
        Voice::Roman | Voice::Italic => Voice::Italic,
        Voice::Bold | Voice::BoldItalic => Voice::BoldItalic,
    }
}

pub(super) fn strong(v: Voice) -> Voice {
    match v {
        Voice::Roman | Voice::Bold => Voice::Bold,
        Voice::Italic | Voice::BoldItalic => Voice::BoldItalic,
    }
}

/// Header cells: Roman→Bold, Italic→BoldItalic; Mono and already-bold stay.
pub(super) fn header_cut(c: Cut) -> Cut {
    match c {
        Cut::Roman => Cut::Bold,
        Cut::Italic => Cut::BoldItalic,
        Cut::Bold | Cut::BoldItalic | Cut::Mono => c,
    }
}

pub(super) fn style_header(spans: &mut [Span<'static>]) {
    for s in spans {
        if let Span::Type { cut, .. } = s {
            *cut = header_cut(*cut);
        } else if let Span::Strike(inner) = s {
            style_header(inner);
        }
    }
}

impl<'a, L> Cx<'a, '_, L>
where
    L: FnMut(&str) -> Result<Vec<u8>, Error>,
{
    pub(super) fn inlines(
        &mut self,
        node: &'a AstNode<'a>,
        voice: Voice,
    ) -> Result<Vec<Span<'static>>, Error> {
        let mut spans = Vec::new();
        for child in node.children() {
            self.inline(child, voice, &mut spans)?;
        }
        if spans.is_empty() {
            spans.push(Span::new(cut(voice), ""));
        }
        Ok(spans)
    }

    pub(super) fn inline(
        &mut self,
        node: &'a AstNode<'a>,
        voice: Voice,
        spans: &mut Vec<Span<'static>>,
    ) -> Result<(), Error> {
        self.inline_inner(node, voice, spans)
            .map_err(|e| e.at(self.location(node)))
    }

    fn inline_inner(
        &mut self,
        node: &'a AstNode<'a>,
        voice: Voice,
        spans: &mut Vec<Span<'static>>,
    ) -> Result<(), Error> {
        enum Job {
            Emph,
            Strong,
            Strike,
            Escaped,
            Link { url: String, title: String },
            Foot(String),
            Math(String),
            Image,
            Html,
        }
        let job = {
            let data = node.data.borrow();
            match &data.value {
                NodeValue::Text(t) => {
                    push_text(spans, cut(voice), &ellipsis(t.as_ref()));
                    return Ok(());
                }
                NodeValue::SoftBreak => {
                    push_text(spans, cut(voice), " ");
                    return Ok(());
                }
                NodeValue::LineBreak => {
                    push_text(spans, cut(voice), "\n");
                    return Ok(());
                }
                NodeValue::Code(c) => {
                    push_code(spans, c.literal.clone());
                    return Ok(());
                }
                NodeValue::Emph => Job::Emph,
                NodeValue::Strong => Job::Strong,
                NodeValue::Strikethrough => Job::Strike,
                NodeValue::Escaped => Job::Escaped,
                NodeValue::Link(link) => Job::Link {
                    url: link.url.clone(),
                    title: link.title.clone(),
                },
                NodeValue::FootnoteReference(fr) => Job::Foot(fr.name.clone()),
                NodeValue::Math(m) => Job::Math(m.literal.clone()),
                NodeValue::Image(_) => Job::Image,
                NodeValue::HtmlInline(_) => Job::Html,
                other => {
                    return Err(Error::unsupported(
                        other.xml_node_name(),
                        "this node cannot be rendered inline",
                        "use a supported inline construct",
                    ));
                }
            }
        };
        match job {
            Job::Strike => spans.push(Span::Strike(self.inlines(node, voice)?)),
            Job::Emph => {
                for child in node.children() {
                    self.inline(child, emph(voice), spans)?;
                }
            }
            Job::Strong => {
                for child in node.children() {
                    self.inline(child, strong(voice), spans)?;
                }
            }
            Job::Escaped => {
                for child in node.children() {
                    self.inline(child, voice, spans)?;
                }
            }
            Job::Link { url, title } => {
                let inner = emph(voice);
                let mut inner_spans = Vec::new();
                for child in node.children() {
                    self.inline(child, inner, &mut inner_spans)?;
                }
                let text: String = inner_spans.iter().map(plain).collect();
                let dest_note = self.intern_dest(&url, &text, &title, self.location(node))?;
                spans.extend(inner_spans);
                if let Some(id) = dest_note {
                    spans.push(Span::note(id));
                }
            }
            Job::Foot(name) => {
                let id = self.intern_foot(&name)?;
                spans.push(Span::note(id));
            }
            Job::Math(lit) => {
                let math = math::inline(&lit, self.text_size())?;
                spans.push(Span::math(math));
            }
            Job::Image => return Err(Error::MixedImage),
            Job::Html => return Err(Error::Html),
        }
        Ok(())
    }
}

pub(super) fn flatten<'a>(node: &'a AstNode<'a>) -> String {
    let mut s = String::new();
    flatten_into(node, &mut s);
    s
}

fn flatten_into<'a>(node: &'a AstNode<'a>, s: &mut String) {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Text(t) => s.push_str(t.as_ref()),
        NodeValue::SoftBreak => s.push(' '),
        NodeValue::LineBreak => s.push('\n'),
        NodeValue::Code(c) => s.push_str(&c.literal),
        _ => {
            drop(data);
            for child in node.children() {
                flatten_into(child, s);
            }
        }
    }
}

fn plain(s: &Span<'_>) -> String {
    match s {
        Span::Type { text, .. } => text.as_ref().to_string(),
        Span::Strike(inner) => inner.iter().map(plain).collect(),
        Span::Math(_) | Span::Note(_) => String::new(),
    }
}

fn push_text(spans: &mut Vec<Span<'static>>, cut: Cut, text: &str) {
    if text.is_empty() {
        return;
    }
    if cut == Cut::Mono {
        spans.push(Span::new(cut, text.to_string()));
        return;
    }
    match spans.last_mut() {
        Some(Span::Type {
            cut: prev_cut,
            text: prev,
            ..
        }) if *prev_cut == cut && *prev_cut != Cut::Mono => {
            prev.to_mut().push_str(text);
        }
        _ => spans.push(Span::new(cut, text.to_string())),
    }
}

fn push_code(spans: &mut Vec<Span<'static>>, lit: String) {
    spans.push(Span::new(Cut::Mono, lit));
}
