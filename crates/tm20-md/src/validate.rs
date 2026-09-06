//! Reject unsupported AST forms before lowering can discard their meaning.

use comrak::nodes::{AstNode, NodeValue, TableAlignment};
use tm20_set::SourceLocation;

use crate::Error;

pub(super) fn location(node: &AstNode<'_>, source: &[&str]) -> SourceLocation {
    let pos = node.data.borrow().sourcepos;
    SourceLocation {
        line: pos.start.line.max(1),
        column: pos.start.column.max(1),
        text: source
            .get(pos.start.line.saturating_sub(1)..pos.end.line.min(source.len()))
            .unwrap_or(&[])
            .join("\n"),
    }
}

pub(super) fn source(lines: &[&str]) -> Result<(), Error> {
    for (i, line) in lines.iter().enumerate() {
        if let Some(column) = line.chars().position(|c| c == '\0') {
            return Err(Error::unsupported(
                "NUL character",
                "the parser would replace U+0000",
                "remove the NUL character",
            )
            .at(SourceLocation {
                line: i + 1,
                column: column + 1,
                text: (*line).to_owned(),
            }));
        }
    }
    Ok(())
}

pub(super) fn tree<'a>(root: &'a AstNode<'a>, source: &[&str]) -> Result<(), Error> {
    for node in root.descendants() {
        validate(node, source).map_err(|e| e.at(location(node, source)))?;
    }
    Ok(())
}

fn validate<'a>(node: &'a AstNode<'a>, source: &[&str]) -> Result<(), Error> {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Text(text)
            if text.contains("[^") && unresolved_footnote_source(node, source).is_some() =>
        {
            let label = unresolved_footnote_source(node, source).unwrap();
            Err(Error::unsupported(
                "undefined footnote",
                format!("no definition exists for {label}"),
                "add a matching [^name]: definition or escape the literal brackets",
            )
            .at(location(node, source).locate(&label)))
        }
        NodeValue::Link(link) | NodeValue::Image(link)
            if link.url.starts_with("tm20-unresolved:") =>
        {
            Err(Error::unsupported(
                "undefined reference",
                format!("no definition exists for {:?}", &link.url[16..]),
                "add its reference definition, use an inline destination, or escape literal brackets",
            ))
        }
        NodeValue::HtmlInline(_) | NodeValue::HtmlBlock(_) => Err(Error::Html),
        NodeValue::Math(math)
            if math.display_math
                && !node.parent().is_some_and(|parent| {
                    matches!(parent.data.borrow().value, NodeValue::Paragraph)
                }) =>
        {
            Err(Error::unsupported(
                "nested display math",
                "display math cannot retain its block layout inside an inline construct or table cell",
                "move \\[display math\\] into a paragraph outside styling, links, and tables, or use \\(inline math\\)",
            ))
        }
        NodeValue::Heading(_) => {
            if node.first_child().is_none() {
                return Err(Error::unsupported(
                    "empty heading",
                    "an empty heading has no printable content",
                    "add heading text or remove the heading",
                ));
            }
            for child in node.descendants().skip(1) {
                let child_data = child.data.borrow();
                match child_data.value {
                    NodeValue::Text(_) | NodeValue::SoftBreak | NodeValue::Escaped => {}
                    NodeValue::HtmlInline(_) => return Err(Error::Html.at(location(child, source))),
                    NodeValue::Math(_) => return Err(Error::Math.at(location(child, source))),
                    _ => {
                        return Err(Error::unsupported(
                            format!("{} in heading", child_data.value.xml_node_name()),
                            "heading rendering cannot preserve this inline construct",
                            "use plain heading text and move styled content into a paragraph",
                        )
                        .at(location(child, source)));
                    }
                }
            }
            Ok(())
        }
        NodeValue::Table(table) => {
            if !(2..=3).contains(&table.alignments.len()) {
                return Err(Error::Cols);
            }
            if table.alignments.contains(&TableAlignment::Center) {
                let mut at = location(node, source);
                at.line += 1;
                at.text = source.get(at.line - 1).unwrap_or(&"").to_string();
                return Err(Error::unsupported(
                    "centered table column",
                    "the table renderer supports only left and right alignment",
                    "replace :---: with --- or ---:",
                )
                .at(at));
            }
            for row in node.children() {
                let at = location(row, source);
                let raw: String = source
                    .get(at.line - 1)
                    .unwrap_or(&"")
                    .chars()
                    .skip(at.column - 1)
                    .collect();
                let count = cells(&raw);
                if count != table.alignments.len() {
                    return Err(Error::unsupported("ragged table row",
                        format!("row has {count} cells, but header has {}; padding or discarding cells is not allowed", table.alignments.len()),
                        "supply exactly one cell per header column; escape literal pipes as \\|, including inside code spans").at(at));
                }
            }
            Ok(())
        }
        NodeValue::List(_) | NodeValue::BlockQuote => {
            let is_list = matches!(data.value, NodeValue::List(_));
            let depth = node
                .ancestors()
                .take_while(|n| !matches!(n.data.borrow().value, NodeValue::FootnoteDefinition(_)))
                .filter(|n| {
                    if is_list {
                        matches!(n.data.borrow().value, NodeValue::List(_))
                    } else {
                        matches!(n.data.borrow().value, NodeValue::BlockQuote)
                    }
                })
                .count();
            if depth > 3 {
                Err(Error::Nesting)
            } else {
                Ok(())
            }
        }
        NodeValue::Document
        | NodeValue::Paragraph
        | NodeValue::Item(_)
        | NodeValue::TaskItem(_)
        | NodeValue::CodeBlock(_)
        | NodeValue::ThematicBreak
        | NodeValue::TableRow(_)
        | NodeValue::TableCell
        | NodeValue::FootnoteDefinition(_)
        | NodeValue::FootnoteReference(_)
        | NodeValue::Text(_)
        | NodeValue::Code(_)
        | NodeValue::SoftBreak
        | NodeValue::LineBreak
        | NodeValue::Emph
        | NodeValue::Strong
        | NodeValue::Strikethrough
        | NodeValue::Escaped
        | NodeValue::Link(_)
        | NodeValue::Image(_)
        | NodeValue::Math(_) => Ok(()),
        other => Err(Error::unsupported(
            other.xml_node_name(),
            "no renderer exists for this node",
            "rewrite it using a supported Markdown construct",
        )),
    }
}

fn unresolved_footnote_source(node: &AstNode<'_>, source: &[&str]) -> Option<String> {
    let pos = node.data.borrow().sourcepos;
    let mut raw = String::new();
    for line in pos.start.line..=pos.end.line {
        let text = source.get(line.checked_sub(1)?)?;
        let start = if line == pos.start.line {
            pos.start.column.saturating_sub(1)
        } else {
            0
        };
        let end = if line == pos.end.line {
            pos.end.column
        } else {
            text.chars().count()
        };
        raw.extend(text.chars().skip(start).take(end.saturating_sub(start)));
        raw.push('\n');
    }
    let start = raw.find("[^")?;
    let end = raw[start..].find(']')? + start;
    Some(raw[start..=end].to_owned())
}

fn cells(row: &str) -> usize {
    let row = row.trim();
    let mut separators = Vec::new();
    let mut escaped = false;
    for (i, c) in row.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
        } else if c == '|' {
            separators.push(i);
        }
    }
    let leading = usize::from(separators.first() == Some(&0));
    let trailing = usize::from(separators.last().is_some_and(|&i| i + 1 == row.len()));
    separators.len() + 1 - leading - trailing
}
