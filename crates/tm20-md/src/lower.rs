//! Walk a CommonMark AST into a [`Sheet`].

use std::collections::HashMap;
use std::num::NonZeroU32;

use comrak::nodes::{AstNode, ListDelimType, ListType, NodeValue, TableAlignment};
use comrak::{Arena, Options, parse_document};
use tm20_set::{
    Code, ColAlign, Cols, Cut, DecimalDelim, DisplaySize, Figure, Frame, GridSkip, Head, ItemMark,
    List, ListFit, ListItem, Mark, MarkAlign, Marker, Measure, Note, Quote, Rule, Sheet, Span,
    TextBlock, TextSize, Thickness, Tracking,
};

use crate::error::Error;
use crate::math;

const NEST_CAP: u8 = 3;
const BODY: TextSize = TextSize::Pt11;

fn options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.autolink = true;
    options.extension.footnotes = true;
    options.extension.math_latex = true;
    options.parse.smart = true;
    options
}

/// Lower `markdown` to a tape-wide [`Sheet`]. `load` resolves image destinations
/// to bytes. [`crate::image_bytes`] reads relative and `file:` URLs from disk;
/// HTTP is the caller’s choice to reject.
pub fn sheet(
    markdown: &str,
    measure: Measure,
    load: impl FnMut(&str) -> Result<Vec<u8>, Error>,
) -> Result<Sheet<'static>, Error> {
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &options());
    let mut cx = Cx {
        measure: measure.get(),
        load,
        slots: Vec::new(),
        foot_defs: HashMap::new(),
        quote_depth: 0,
        list_depth: 0,
        in_note: false,
    };
    let frames = cx.blocks(root)?;
    let notes = cx.materialize_notes()?;
    Ok(Sheet {
        width: measure,
        frames,
        notes,
    })
}

enum Slot {
    Dest { dest: String, title: Option<String> },
    Foot(String),
}

struct Cx<L> {
    measure: u16,
    load: L,
    slots: Vec<Slot>,
    foot_defs: HashMap<String, Vec<Frame<'static>>>,
    quote_depth: u8,
    list_depth: u8,
    in_note: bool,
}

#[derive(Clone, Copy)]
struct Voice {
    italic: bool,
    bold: bool,
}

impl Voice {
    const ROMAN: Self = Self {
        italic: false,
        bold: false,
    };
    const BOLD: Self = Self {
        italic: false,
        bold: true,
    };
}

fn cut(v: Voice) -> Cut {
    match (v.bold, v.italic) {
        (false, false) => Cut::Roman,
        (false, true) => Cut::Italic,
        (true, false) => Cut::Bold,
        (true, true) => Cut::BoldItalic,
    }
}

fn emph(v: Voice) -> Voice {
    Voice {
        italic: true,
        bold: v.bold,
    }
}

fn strong(v: Voice) -> Voice {
    Voice {
        italic: v.italic,
        bold: true,
    }
}

impl<L> Cx<L>
where
    L: FnMut(&str) -> Result<Vec<u8>, Error>,
{
    fn text_size(&self) -> TextSize {
        if self.in_note { TextSize::Pt8 } else { BODY }
    }

    fn blocks<'a>(&mut self, node: &'a AstNode<'a>) -> Result<Vec<Frame<'static>>, Error> {
        let mut out = Vec::new();
        for child in node.children() {
            match &child.data.borrow().value {
                NodeValue::Paragraph => out.extend(self.paragraph(child)?),
                _ => {
                    if let Some(frame) = self.block(child)? {
                        out.push(frame);
                    }
                }
            }
        }
        Ok(out)
    }

    fn block<'a>(&mut self, node: &'a AstNode<'a>) -> Result<Option<Frame<'static>>, Error> {
        let value = node.data.borrow().value.clone();
        match value {
            NodeValue::FrontMatter(_) => Ok(None),
            NodeValue::FootnoteDefinition(def) => {
                let was = self.in_note;
                self.in_note = true;
                let frames = self.blocks(node)?;
                self.in_note = was;
                self.foot_defs.entry(def.name).or_insert(frames);
                Ok(None)
            }
            NodeValue::BlockQuote => {
                if self.quote_depth >= NEST_CAP {
                    return Err(Error::Nesting);
                }
                self.quote_depth += 1;
                let frames = self.blocks(node)?;
                self.quote_depth -= 1;
                Ok(Some(Frame::Quote(Quote { frames })))
            }
            NodeValue::List(nl) => Ok(Some(self.list(node, nl)?)),
            NodeValue::Item(_) | NodeValue::TaskItem(_) => Ok(None),
            NodeValue::CodeBlock(cb) => Ok(Some(code_frame(&cb.literal, self.text_size()))),
            NodeValue::HtmlBlock(_) => Err(Error::Html),
            NodeValue::Heading(h) => Ok(Some(self.heading(node, h.level)?)),
            NodeValue::ThematicBreak => Ok(Some(Frame::Rule(Rule {
                thickness: Thickness::Two,
            }))),
            NodeValue::Table(t) => Ok(Some(self.table(node, t.alignments.as_slice())?)),
            NodeValue::TableRow(_) | NodeValue::TableCell => Ok(None),
            NodeValue::HtmlInline(_) => Err(Error::Html),
            _ => Err(Error::Html),
        }
    }

    fn heading<'a>(&self, node: &'a AstNode<'a>, level: u8) -> Result<Frame<'static>, Error> {
        if has_math(node) {
            return Err(Error::Math);
        }
        let text = flatten(node);
        if self.in_note {
            return Ok(Frame::Head(Head {
                size: TextSize::Pt8,
                text: text.into(),
            }));
        }
        if level <= 1 {
            Ok(Frame::Mark(Mark {
                cut: Cut::Roman,
                size: DisplaySize::Pt18,
                text: text.into(),
                align: MarkAlign::Start,
                tracking: Tracking(0),
            }))
        } else {
            Ok(Frame::Head(Head {
                size: BODY,
                text: text.into(),
            }))
        }
    }

    fn list<'a>(
        &mut self,
        node: &'a AstNode<'a>,
        nl: comrak::nodes::NodeList,
    ) -> Result<Frame<'static>, Error> {
        if self.list_depth >= NEST_CAP {
            return Err(Error::Nesting);
        }
        self.list_depth += 1;
        let marker = match nl.list_type {
            ListType::Bullet => Marker::Dash,
            ListType::Ordered => Marker::Decimal {
                start: nl.start as u32,
                delim: match nl.delimiter {
                    ListDelimType::Period => DecimalDelim::Period,
                    ListDelimType::Paren => DecimalDelim::Paren,
                },
            },
        };
        let mut items = Vec::new();
        for child in node.children() {
            let value = child.data.borrow().value.clone();
            let mark = match value {
                NodeValue::TaskItem(t) => ItemMark::Task {
                    checked: t.symbol.is_some(),
                },
                NodeValue::Item(_) => ItemMark::List,
                _ => return Err(Error::Html),
            };
            items.push(ListItem {
                mark,
                frames: self.blocks(child)?,
            });
        }
        self.list_depth -= 1;
        Ok(Frame::List(List {
            size: self.text_size(),
            cut: Cut::Roman,
            marker,
            fit: if nl.tight {
                ListFit::Tight
            } else {
                ListFit::Loose
            },
            items,
        }))
    }

    fn paragraph<'a>(&mut self, node: &'a AstNode<'a>) -> Result<Vec<Frame<'static>>, Error> {
        let kids: Vec<_> = node.children().collect();
        if kids.len() == 1
            && let NodeValue::Image(link) = &kids[0].data.borrow().value
        {
            let bytes = (self.load)(link.url.as_ref())?;
            let fig = Figure::from_image(&bytes, self.measure)?;
            return Ok(vec![Frame::Figure(fig)]);
        }
        if kids
            .iter()
            .any(|k| matches!(k.data.borrow().value, NodeValue::Image(_)))
        {
            return Err(Error::MixedImage);
        }
        let mut frames = Vec::new();
        let mut spans = Vec::new();
        for child in kids {
            let display = match &child.data.borrow().value {
                NodeValue::Math(m) => m.display_math,
                _ => false,
            };
            if display {
                flush_text(self.text_size(), &mut spans, &mut frames);
                let NodeValue::Math(m) = child.data.borrow().value.clone() else {
                    unreachable!("display math");
                };
                let m = math::display(&m.literal, self.text_size(), self.measure)?;
                frames.push(Frame::Math(m));
                continue;
            }
            self.inline(child, Voice::ROMAN, &mut spans)?;
        }
        flush_text(self.text_size(), &mut spans, &mut frames);
        Ok(frames)
    }

    fn table<'a>(
        &mut self,
        node: &'a AstNode<'a>,
        alignments: &[TableAlignment],
    ) -> Result<Frame<'static>, Error> {
        let n = alignments.len();
        if !(2..=3).contains(&n) {
            return Err(Error::Cols);
        }
        let align: Vec<ColAlign> = alignments
            .iter()
            .map(|a| match a {
                TableAlignment::Right => ColAlign::End,
                _ => ColAlign::Start,
            })
            .collect();
        let mut rows = Vec::new();
        for row in node.children() {
            let header = matches!(row.data.borrow().value, NodeValue::TableRow(true));
            let voice = if header { Voice::BOLD } else { Voice::ROMAN };
            let mut cells = Vec::new();
            for cell in row.children() {
                let mut spans = self.inlines(cell, voice)?;
                if header {
                    for s in &mut spans {
                        if let Span::Type { cut, .. } = s {
                            *cut = Cut::Bold;
                        }
                    }
                }
                cells.push(spans);
            }
            if cells.len() != n {
                return Err(Error::Cols);
            }
            rows.push(cells);
        }
        Ok(Frame::Cols(cols_frame(self.text_size(), &align, rows)?))
    }

    fn inlines<'a>(
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

    fn inline<'a>(
        &mut self,
        node: &'a AstNode<'a>,
        voice: Voice,
        spans: &mut Vec<Span<'static>>,
    ) -> Result<(), Error> {
        let value = node.data.borrow().value.clone();
        match value {
            NodeValue::Text(t) => push(spans, cut(voice), t.as_ref(), None),
            NodeValue::SoftBreak => push(spans, cut(voice), " ", None),
            NodeValue::LineBreak => push(spans, cut(voice), "\n", None),
            NodeValue::Code(c) => push(spans, Cut::Mono, strip_code(&c.literal), None),
            NodeValue::Emph => {
                for child in node.children() {
                    self.inline(child, emph(voice), spans)?;
                }
            }
            NodeValue::Strong => {
                for child in node.children() {
                    self.inline(child, strong(voice), spans)?;
                }
            }
            NodeValue::Link(link) => {
                let inner = emph(voice);
                let mut inner_spans = Vec::new();
                for child in node.children() {
                    self.inline(child, inner, &mut inner_spans)?;
                }
                let text: String = inner_spans
                    .iter()
                    .map(|s| match s {
                        Span::Type { text, .. } => text.as_ref(),
                        Span::Math(_) => "",
                    })
                    .collect();
                let dest_note = self.note_for_dest(&link.url, &text, &link.title);
                if inner_spans.is_empty() {
                    inner_spans.push(Span::Type {
                        cut: cut(inner),
                        text: std::borrow::Cow::Owned(String::new()),
                        note: dest_note,
                    });
                } else if let Some(n) = dest_note
                    && let Some(Span::Type { note, .. }) = inner_spans.last_mut()
                {
                    *note = Some(n);
                }
                spans.extend(inner_spans);
            }
            NodeValue::FootnoteReference(fr) => {
                let n = self.note_for_foot(&fr.name);
                match spans.last_mut() {
                    Some(Span::Type { note, .. }) if note.is_none() => *note = Some(n),
                    _ => spans.push(Span::Type {
                        cut: cut(voice),
                        text: std::borrow::Cow::Owned(String::new()),
                        note: Some(n),
                    }),
                }
            }
            NodeValue::Image(_) => return Err(Error::MixedImage),
            NodeValue::Math(m) => {
                let math = math::inline(&m.literal, self.text_size(), self.measure)?;
                spans.push(Span::math(math));
            }
            NodeValue::HtmlInline(_) => return Err(Error::Html),
            NodeValue::Escaped => {
                for child in node.children() {
                    self.inline(child, voice, spans)?;
                }
            }
            _ => return Err(Error::Html),
        }
        Ok(())
    }

    fn note_for_dest(&mut self, dest: &str, text: &str, title: &str) -> Option<NonZeroU32> {
        let stored = match dest.strip_prefix("mailto:") {
            Some(addr) if !addr.is_empty() => addr.to_string(),
            _ => dest.to_string(),
        };
        if stored.is_empty() {
            return None;
        }
        if title.is_empty() && (dest == text || stored == text) {
            return None;
        }
        if let Some(i) = self.slots.iter().position(|s| match s {
            Slot::Dest { dest: d, .. } => d == &stored,
            Slot::Foot(_) => false,
        }) {
            return NonZeroU32::new(i as u32 + 1);
        }
        let title = if title.is_empty() {
            None
        } else {
            Some(title.to_string())
        };
        self.slots.push(Slot::Dest {
            dest: stored,
            title,
        });
        NonZeroU32::new(self.slots.len() as u32)
    }

    fn note_for_foot(&mut self, name: &str) -> NonZeroU32 {
        if let Some(i) = self.slots.iter().position(|s| match s {
            Slot::Foot(n) => n == name,
            Slot::Dest { .. } => false,
        }) {
            return NonZeroU32::new(i as u32 + 1).unwrap();
        }
        self.slots.push(Slot::Foot(name.to_string()));
        NonZeroU32::new(self.slots.len() as u32).unwrap()
    }

    fn materialize_notes(self) -> Result<Vec<Note<'static>>, Error> {
        let mut defs = self.foot_defs;
        self.slots
            .into_iter()
            .map(|s| match s {
                Slot::Dest { dest, title } => Ok(Note::Dest {
                    dest: dest.into(),
                    title: title.map(Into::into),
                }),
                Slot::Foot(name) => {
                    let frames = defs.remove(&name).ok_or(Error::Note)?;
                    Ok(Note::Blocks(frames))
                }
            })
            .collect()
    }
}

fn code_frame(literal: &str, size: TextSize) -> Frame<'static> {
    let mut lines: Vec<std::borrow::Cow<'static, str>> = literal
        .split('\n')
        .map(|s| std::borrow::Cow::Owned(s.to_string()))
        .collect();
    if lines.last().is_some_and(|s| s.is_empty()) {
        lines.pop();
    }
    Frame::Code(Code { size, lines })
}

fn flush_text(size: TextSize, spans: &mut Vec<Span<'static>>, frames: &mut Vec<Frame<'static>>) {
    if spans.is_empty() {
        return;
    }
    frames.push(Frame::Text(TextBlock {
        size,
        spans: std::mem::take(spans),
    }));
}

fn has_math<'a>(node: &'a AstNode<'a>) -> bool {
    matches!(node.data.borrow().value, NodeValue::Math(_)) || node.children().any(has_math)
}

fn flatten<'a>(node: &'a AstNode<'a>) -> String {
    let mut s = String::new();
    flatten_into(node, &mut s);
    s
}

fn flatten_into<'a>(node: &'a AstNode<'a>, s: &mut String) {
    match &node.data.borrow().value {
        NodeValue::Text(t) => s.push_str(t.as_ref()),
        NodeValue::SoftBreak => s.push(' '),
        NodeValue::LineBreak => s.push('\n'),
        NodeValue::Code(c) => s.push_str(strip_code(&c.literal)),
        _ => {
            for child in node.children() {
                flatten_into(child, s);
            }
        }
    }
}

fn strip_code(lit: &str) -> &str {
    let b = lit.as_bytes();
    if b.len() >= 2 && b[0] == b' ' && b[b.len() - 1] == b' ' && lit.bytes().any(|c| c != b' ') {
        &lit[1..lit.len() - 1]
    } else {
        lit
    }
}

fn cols_frame(
    size: TextSize,
    align: &[ColAlign],
    rows: Vec<Vec<Vec<Span<'static>>>>,
) -> Result<Cols<'static>, Error> {
    match align.len() {
        2 => {
            let align = [align[0], align[1]];
            let rows = rows
                .into_iter()
                .map(|r| pair(r))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Cols::two(size, GridSkip::ONE, align, rows))
        }
        3 => {
            let align = [align[0], align[1], align[2]];
            let rows = rows
                .into_iter()
                .map(|r| triple(r))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Cols::three(size, GridSkip::ONE, align, rows))
        }
        _ => Err(Error::Cols),
    }
}

fn pair(cells: Vec<Vec<Span<'static>>>) -> Result<[Vec<Span<'static>>; 2], Error> {
    let mut it = cells.into_iter();
    Ok([it.next().ok_or(Error::Cols)?, it.next().ok_or(Error::Cols)?])
}

fn triple(cells: Vec<Vec<Span<'static>>>) -> Result<[Vec<Span<'static>>; 3], Error> {
    let mut it = cells.into_iter();
    Ok([
        it.next().ok_or(Error::Cols)?,
        it.next().ok_or(Error::Cols)?,
        it.next().ok_or(Error::Cols)?,
    ])
}

fn push(spans: &mut Vec<Span<'static>>, cut: Cut, text: &str, note: Option<NonZeroU32>) {
    if text.is_empty() && note.is_none() {
        return;
    }
    match spans.last_mut() {
        Some(Span::Type {
            cut: prev_cut,
            text: prev,
            note: prev_note,
        }) if *prev_cut == cut && prev_note.is_none() && note.is_none() => {
            prev.to_mut().push_str(text);
        }
        _ => spans.push(Span::Type {
            cut,
            text: std::borrow::Cow::Owned(text.to_string()),
            note,
        }),
    }
}
