//! Walk a CommonMark AST into a [`Sheet`].

use std::collections::HashMap;
use std::num::NonZeroU32;

use comrak::nodes::{AstNode, ListDelimType, ListType, NodeValue, TableAlignment};
use comrak::{Arena, Options, parse_document};
use tm20_set::{
    Code, ColAlign, Cols, Cut, DecimalDelim, DisplayCut, DisplaySize, Figure, Frame, GridSkip, Head, ItemBody,
    ItemMark, List, ListFit, ListItem, Mark, MarkAlign, Marker, Measure, Note, Quote, Rule, Sheet,
    Span, TextBlock, TextSize, Thickness, Tracking,
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
    // CommonMark 2.3: U+0000 is insecure. The tape shows U+FFFD, visibly,
    // instead of a silent zero-width hole between two words.
    let owned;
    let markdown = if markdown.contains('\u{0}') {
        owned = markdown.replace('\u{0}', "\u{FFFD}");
        owned.as_str()
    } else {
        markdown
    };
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &options());
    let mut cx = Cx {
        source: markdown,
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

struct Cx<'s, L> {
    source: &'s str,
    measure: u16,
    load: L,
    slots: Vec<Slot>,
    foot_defs: HashMap<String, Vec<Frame<'static>>>,
    quote_depth: u8,
    list_depth: u8,
    in_note: bool,
}

#[derive(Clone, Copy)]
enum Voice {
    Roman,
    Italic,
    Bold,
    BoldItalic,
}

fn cut(v: Voice) -> Cut {
    match v {
        Voice::Roman => Cut::Roman,
        Voice::Italic => Cut::Italic,
        Voice::Bold => Cut::Bold,
        Voice::BoldItalic => Cut::BoldItalic,
    }
}

fn emph(v: Voice) -> Voice {
    match v {
        Voice::Roman | Voice::Italic => Voice::Italic,
        Voice::Bold | Voice::BoldItalic => Voice::BoldItalic,
    }
}

fn strong(v: Voice) -> Voice {
    match v {
        Voice::Roman | Voice::Bold => Voice::Bold,
        Voice::Italic | Voice::BoldItalic => Voice::BoldItalic,
    }
}

impl<'s, L> Cx<'s, L>
where
    L: FnMut(&str) -> Result<Vec<u8>, Error>,
{
    fn source_line(&self, line: usize) -> &'s str {
        self.source.lines().nth(line.saturating_sub(1)).unwrap_or("")
    }
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
        enum Job {
            Skip,
            Footnote(String),
            Quote,
            List(comrak::nodes::NodeList),
            Code(String),
            Html,
            Heading(u8),
            Rule,
            Table(Vec<TableAlignment>),
        }
        let job = match &node.data.borrow().value {
            NodeValue::FrontMatter(_)
            | NodeValue::Item(_)
            | NodeValue::TaskItem(_)
            | NodeValue::TableRow(_)
            | NodeValue::TableCell => Job::Skip,
            NodeValue::FootnoteDefinition(def) => Job::Footnote(def.name.clone()),
            NodeValue::BlockQuote => Job::Quote,
            NodeValue::List(nl) => Job::List(*nl),
            NodeValue::CodeBlock(cb) => Job::Code(cb.literal.clone()),
            NodeValue::HtmlBlock(_) | NodeValue::HtmlInline(_) => Job::Html,
            NodeValue::Heading(h) => Job::Heading(h.level),
            NodeValue::ThematicBreak => Job::Rule,
            NodeValue::Table(t) => Job::Table(t.alignments.clone()),
            _ => Job::Html,
        };
        match job {
            Job::Skip => Ok(None),
            Job::Footnote(name) => {
                let was = self.in_note;
                self.in_note = true;
                let frames = self.blocks(node)?;
                self.in_note = was;
                self.foot_defs.entry(name).or_insert(frames);
                Ok(None)
            }
            Job::Quote => {
                if self.quote_depth >= NEST_CAP {
                    return Err(Error::Nesting);
                }
                self.quote_depth += 1;
                let frames = self.blocks(node)?;
                self.quote_depth -= 1;
                Ok(Some(Frame::Quote(Quote { frames })))
            }
            Job::List(nl) => Ok(Some(self.list(node, nl)?)),
            Job::Code(literal) => Ok(Some(code_frame(&literal, self.text_size()))),
            Job::Html => Err(Error::Html),
            Job::Heading(level) => Ok(self.heading(node, level)?),
            Job::Rule => Ok(Some(Frame::Rule(Rule::tape(Thickness::Two)))),
            Job::Table(alignments) => Ok(Some(self.table(node, &alignments)?)),
        }
    }

    fn heading<'a>(&self, node: &'a AstNode<'a>, level: u8) -> Result<Option<Frame<'static>>, Error> {
        if has_math(node) {
            return Err(Error::Math);
        }
        let text = flatten(node);
        if text.is_empty() {
            return Ok(None);
        }
        if self.in_note {
            return Ok(Some(Frame::Head(Head {
                size: TextSize::Pt8,
                text: text.into(),
            })));
        }
        if level <= 1 {
            Ok(Some(Frame::Mark(Mark {
                cut: DisplayCut::Roman,
                size: DisplaySize::Pt18,
                text: text.into(),
                align: MarkAlign::Start,
                tracking: Tracking(0),
            })))
        } else {
            Ok(Some(Frame::Head(Head {
                size: BODY,
                text: text.into(),
            })))
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
            let mark = match &child.data.borrow().value {
                NodeValue::TaskItem(t) => ItemMark::Task {
                    checked: t.symbol.is_some(),
                },
                NodeValue::Item(_) => ItemMark::List,
                _ => return Err(Error::Html),
            };
            items.push(ListItem {
                mark,
                body: ItemBody::from_frames(self.blocks(child)?),
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
                NodeValue::Math(m) if m.display_math => Some(m.literal.clone()),
                _ => None,
            };
            if let Some(lit) = display {
                flush_text(self.text_size(), &mut spans, &mut frames);
                let m = math::display(&lit, self.text_size(), self.measure)?;
                frames.push(Frame::Math(m));
                continue;
            }
            self.inline(child, Voice::Roman, &mut spans)?;
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
            let voice = if header { Voice::Bold } else { Voice::Roman };
            let line = self.source_line(row.data.borrow().sourcepos.start.line);
            let cells = split_table_cells(line);
            let mut parsed = if cells.len() == n {
                cells
                    .iter()
                    .map(|cell| self.cell_spans(cell, voice))
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                row.children()
                    .map(|cell| self.inlines(cell, voice))
                    .collect::<Result<Vec<_>, _>>()?
            };
            if parsed.len() != n {
                return Err(Error::Cols);
            }
            if header {
                for spans in &mut parsed {
                    for s in spans {
                        if let Span::Type { cut, .. } = s {
                            *cut = Cut::Bold;
                        }
                    }
                }
            }
            rows.push(parsed);
        }
        Ok(Frame::Cols(cols_frame(self.text_size(), &align, rows)?))
    }

    fn cell_spans(&mut self, cell: &str, voice: Voice) -> Result<Vec<Span<'static>>, Error> {
        let arena = Arena::new();
        let defs = footnote_defs(self.source);
        let src = if defs.is_empty() {
            cell.to_string()
        } else {
            format!("{cell}\n\n{defs}")
        };
        let root = parse_document(&arena, &src, &options());
        let mut spans = Vec::new();
        for child in root.children() {
            match &child.data.borrow().value {
                NodeValue::Paragraph => {
                    for inline in child.children() {
                        self.inline(inline, voice, &mut spans)?;
                    }
                }
                NodeValue::FootnoteDefinition(_) => {}
                _ => {
                    if let Some(Frame::Text(b)) = self.block(child)? {
                        spans.extend(b.spans);
                    }
                }
            }
        }
        if spans.is_empty() {
            spans.push(Span::new(cut(voice), ""));
        }
        Ok(spans)
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
        enum Job {
            Emph,
            Strong,
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
                    push(spans, cut(voice), &ellipsis(t.as_ref()), None);
                    return Ok(());
                }
                NodeValue::SoftBreak => {
                    push(spans, cut(voice), " ", None);
                    return Ok(());
                }
                NodeValue::LineBreak => {
                    push(spans, cut(voice), "\n", None);
                    return Ok(());
                }
                NodeValue::Code(c) => {
                    push(spans, Cut::Mono, strip_code(&c.literal), None);
                    return Ok(());
                }
                NodeValue::Emph => Job::Emph,
                NodeValue::Strong => Job::Strong,
                NodeValue::Escaped => Job::Escaped,
                NodeValue::Link(link) => Job::Link {
                    url: link.url.clone(),
                    title: link.title.clone(),
                },
                NodeValue::FootnoteReference(fr) => Job::Foot(fr.name.clone()),
                NodeValue::Math(m) => Job::Math(m.literal.clone()),
                NodeValue::Image(_) => Job::Image,
                NodeValue::HtmlInline(_) => Job::Html,
                _ => Job::Html,
            }
        };
        match job {
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
                let text: String = inner_spans
                    .iter()
                    .map(|s| match s {
                        Span::Type { text, .. } => text.as_ref(),
                        Span::Math(_) => "",
                    })
                    .collect();
                let dest_note = self.note_for_dest(&url, &text, &title);
                if inner_spans.is_empty() {
                    inner_spans.push(Span::Type {
                        cut: cut(inner),
                        text: std::borrow::Cow::Borrowed(""),
                        note: dest_note,
                    });
                } else if let Some(n) = dest_note
                    && let Some(Span::Type { note, .. }) = inner_spans.last_mut()
                {
                    *note = Some(n);
                }
                spans.extend(inner_spans);
            }
            Job::Foot(name) => {
                let n = self.note_for_foot(&name);
                match spans.last_mut() {
                    Some(Span::Type { note, .. }) if note.is_none() => *note = Some(n),
                    _ => spans.push(Span::Type {
                        cut: cut(voice),
                        text: std::borrow::Cow::Borrowed(""),
                        note: Some(n),
                    }),
                }
            }
            Job::Math(lit) => {
                let math = math::inline(&lit, self.text_size(), self.measure)?;
                spans.push(Span::math(math));
            }
            Job::Image => return Err(Error::MixedImage),
            Job::Html => return Err(Error::Html),
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
        // A GFM bare autolink's dest is its own text plus a scheme comrak
        // added. A link whose destination is its text carries no note.
        let auto = dest == text
            || stored == text
            || stored.strip_prefix("http://").is_some_and(|r| r == text)
            || stored.strip_prefix("https://").is_some_and(|r| r == text);
        if title.is_empty() && auto {
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
    Frame::Code(Code::new(size, literal))
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

fn footnote_defs(source: &str) -> String {
    let mut out = String::new();
    let mut in_def = false;
    for line in source.lines() {
        if line.starts_with("[^") && line.contains("]:") {
            in_def = true;
            out.push_str(line);
            out.push('\n');
        } else if in_def && (line.starts_with("    ") || line.starts_with('\t') || line.is_empty()) {
            out.push_str(line);
            out.push('\n');
        } else {
            in_def = false;
        }
    }
    out
}

fn ellipsis(s: &str) -> std::borrow::Cow<'_, str> {
    if s.contains("...") {
        std::borrow::Cow::Owned(s.replace("...", "\u{2026}"))
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

/// Split a GFM row so `|` inside a code span is cell content, not a column.
fn split_table_cells(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.trim().chars().collect();
    let mut i = 0usize;
    if chars.first() == Some(&'|') {
        i = 1;
    }
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut code = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if code == 0 && c == '\\' && chars.get(i + 1) == Some(&'|') {
            cur.push('|');
            i += 2;
            continue;
        }
        if c == '`' {
            let n = chars[i..].iter().take_while(|ch| **ch == '`').count();
            if code == 0 {
                code = n;
            } else if n == code {
                code = 0;
            }
            cur.extend(std::iter::repeat_n('`', n));
            i += n;
            continue;
        }
        if c == '|' && code == 0 {
            cells.push(cur.trim().to_string());
            cur.clear();
            i += 1;
            continue;
        }
        cur.push(c);
        i += 1;
    }
    if !cur.is_empty() {
        cells.push(cur.trim().to_string());
    }
    cells
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
