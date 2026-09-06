//! Block lowering from the final whole-document AST.

use comrak::nodes::{AstNode, ListDelimType, ListType, NodeValue};
use tm20_set::{
    Code, Cut, DecimalDelim, DisplayCut, DisplaySize, Figure, Frame, Head, ItemBody, ItemMark,
    List, ListFit, ListItem, Mark, MarkAlign, Marker, Quote, Rule, TextBlock, TextSize, Thickness,
    Tracking,
};

use crate::error::Error;
use crate::math;

use super::inline::{Voice, flatten};
use super::{BODY, Cx};

impl<'a, L> Cx<'a, '_, L>
where
    L: FnMut(&str) -> Result<Vec<u8>, Error>,
{
    pub(super) fn blocks(&mut self, node: &'a AstNode<'a>) -> Result<Vec<Frame<'static>>, Error> {
        let mut out = Vec::new();
        for child in node.children() {
            let frames = match &child.data.borrow().value {
                NodeValue::Paragraph => self
                    .paragraph(child)
                    .map_err(|e| e.at(self.location(child)))?,
                NodeValue::FootnoteDefinition(_) => continue,
                _ => self
                    .block(child)
                    .map_err(|e| e.at(self.location(child)))?
                    .into_iter()
                    .collect(),
            };
            out.extend(frames.into_iter().map(|frame| Frame::Source {
                location: self.location(child),
                frame: Box::new(frame),
            }));
        }
        Ok(out)
    }

    pub(super) fn block(&mut self, node: &'a AstNode<'a>) -> Result<Option<Frame<'static>>, Error> {
        enum Job {
            Quote,
            List(comrak::nodes::NodeList),
            Code(String),
            Html,
            Heading(u8),
            Rule,
            Table(Vec<comrak::nodes::TableAlignment>),
        }
        let job = match &node.data.borrow().value {
            NodeValue::BlockQuote => Job::Quote,
            NodeValue::List(nl) => Job::List(*nl),
            NodeValue::CodeBlock(cb) => Job::Code(cb.literal.clone()),
            NodeValue::HtmlBlock(_) | NodeValue::HtmlInline(_) => Job::Html,
            NodeValue::Heading(h) => Job::Heading(h.level),
            NodeValue::ThematicBreak => Job::Rule,
            NodeValue::Table(t) => Job::Table(t.alignments.clone()),
            other => {
                return Err(Error::unsupported(
                    other.xml_node_name(),
                    "this node cannot be rendered as a block",
                    "use a supported block construct",
                ));
            }
        };
        match job {
            Job::Quote => {
                let frames = self.blocks(node)?;
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

    fn heading(&self, node: &'a AstNode<'a>, level: u8) -> Result<Option<Frame<'static>>, Error> {
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

    fn list(
        &mut self,
        node: &'a AstNode<'a>,
        nl: comrak::nodes::NodeList,
    ) -> Result<Frame<'static>, Error> {
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

    fn paragraph(&mut self, node: &'a AstNode<'a>) -> Result<Vec<Frame<'static>>, Error> {
        let kids: Vec<_> = node.children().collect();
        if kids.len() == 1
            && let NodeValue::Image(link) = &kids[0].data.borrow().value
        {
            let bytes = (self.load)(link.url.as_ref()).map_err(|e| e.at(self.location(kids[0])))?;
            let fig = Figure::from_image(&bytes).map_err(|e| {
                Error::Resource {
                    destination: link.url.clone(),
                    reason: e.to_string(),
                }
                .at(self.location(kids[0]))
            })?;
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
                let m = math::display(&lit, self.text_size())
                    .map_err(|e| e.at(self.location(child)))?;
                frames.push(Frame::Math(m));
                continue;
            }
            self.inline(child, Voice::Roman, &mut spans)?;
        }
        flush_text(self.text_size(), &mut spans, &mut frames);
        Ok(frames)
    }
}

fn code_frame(literal: &str, size: TextSize) -> Frame<'static> {
    Frame::Code(Code::new(size, literal))
}

fn flush_text(
    size: TextSize,
    spans: &mut Vec<tm20_set::Span<'static>>,
    frames: &mut Vec<Frame<'static>>,
) {
    if spans.is_empty() {
        return;
    }
    frames.push(Frame::Text(TextBlock {
        size,
        spans: std::mem::take(spans),
    }));
}
