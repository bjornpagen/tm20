//! Walk a CommonMark AST into a [`Sheet`]. One authoritative final parse.

use std::borrow::Cow;
use std::collections::HashMap;

use comrak::nodes::AstNode;
use comrak::{Arena, Options, parse_document};
use tm20_set::{NoteId, Sheet, SheetBuilder, TextSize};

use crate::error::Error;

#[path = "blocks.rs"]
mod blocks;
#[path = "inline.rs"]
mod inline;
#[path = "notes.rs"]
mod notes;
#[path = "tables.rs"]
mod tables;
#[path = "validate.rs"]
mod validate;

const BODY: TextSize = TextSize::Pt11;

pub(crate) fn options(tables: bool) -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = tables;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.footnotes = true;
    options.extension.math_latex = true;
    options.parse.smart = true;
    options.parse.sourcepos_chars = true;
    options.parse.escaped_char_spans = true;
    options.parse.leave_footnote_definitions = true;
    options.parse.broken_link_callback = Some(std::sync::Arc::new(
        |reference: comrak::options::BrokenLinkReference<'_>| {
            if reference.original.starts_with('^') || matches!(reference.original, " " | "x" | "X")
            {
                return None;
            }
            Some(comrak::ResolvedReference {
                url: format!("tm20-unresolved:{}", reference.original),
                title: String::new(),
            })
        },
    ));
    options
}

/// Lower `markdown` to a tape-wide [`Sheet`]. `load` resolves image destinations
/// to bytes. [`crate::image_bytes`] reads relative and `file:` URLs from disk;
/// HTTP is the caller’s choice to reject.
pub fn sheet(
    markdown: &str,
    measure: tm20_set::Measure,
    load: impl FnMut(&str) -> Result<Vec<u8>, Error>,
) -> Result<Sheet<'static>, Error> {
    let normalized = markdown.replace("\r\n", "\n").replace('\r', "\n");
    let source: Vec<&str> = normalized.split('\n').collect();
    validate::source(&source)?;
    let arena = Arena::new();
    let root = parse_document(&arena, &normalized, &options(true));
    validate::tree(root, &source)?;
    let mut cx = Cx {
        builder: SheetBuilder::new(measure),
        load,
        dests: HashMap::new(),
        foots: HashMap::new(),
        pending_foot: HashMap::new(),
        foot_defs: HashMap::new(),
        source: &source,
        in_note: false,
    };
    cx.index_footnote_defs(root);
    for frame in cx.blocks(root)? {
        cx.builder.push_frame(frame);
    }
    cx.define_referenced_notes()?;
    let sheet = cx.builder.finish()?;
    sheet.admit()?;
    Ok(sheet)
}

struct Cx<'a, 'p, L> {
    builder: SheetBuilder<'static>,
    load: L,
    dests: HashMap<String, (NoteId, String)>,
    foots: HashMap<String, NoteId>,
    pending_foot: HashMap<NoteId, String>,
    foot_defs: HashMap<String, &'a AstNode<'a>>,
    source: &'p [&'p str],
    in_note: bool,
}

impl<L> Cx<'_, '_, L> {
    fn location(&self, node: &AstNode<'_>) -> tm20_set::SourceLocation {
        validate::location(node, self.source)
    }

    fn text_size(&self) -> TextSize {
        if self.in_note { TextSize::Pt8 } else { BODY }
    }
}

fn ellipsis(s: &str) -> Cow<'_, str> {
    if s.contains("...") {
        Cow::Owned(s.replace("...", "\u{2026}"))
    } else {
        Cow::Borrowed(s)
    }
}
