//! Note internment through [`SheetBuilder`]. Lookup maps hold [`NoteId`]s only.

use std::collections::HashSet;

use comrak::nodes::NodeValue;
use tm20_set::{Note, NoteId};

use crate::error::Error;

use super::Cx;

impl<'a, L> Cx<'a, '_, L>
where
    L: FnMut(&str) -> Result<Vec<u8>, Error>,
{
    pub(super) fn intern_dest(
        &mut self,
        dest: &str,
        text: &str,
        title: &str,
        location: tm20_set::SourceLocation,
    ) -> Result<Option<NoteId>, Error> {
        let stored = match dest.strip_prefix("mailto:") {
            Some(addr) if !addr.is_empty() => addr.to_string(),
            _ => dest.to_string(),
        };
        if stored.is_empty() {
            return Ok(None);
        }
        let auto = dest == text
            || stored == text
            || stored.strip_prefix("http://").is_some_and(|r| r == text)
            || stored.strip_prefix("https://").is_some_and(|r| r == text);
        if title.is_empty() && auto {
            return Ok(None);
        }
        if let Some((id, first_title)) = self.dests.get(&stored) {
            if first_title != title {
                return Err(Error::unsupported(
                    "conflicting link titles",
                    format!(
                        "destination {stored:?} already has title {first_title:?}, not {title:?}"
                    ),
                    "use one consistent title for a shared destination",
                )
                .at(location));
            }
            return Ok(Some(*id));
        }
        let id = self.builder.reserve_note()?;
        let title = if title.is_empty() {
            None
        } else {
            Some(title.to_string())
        };
        self.builder.define_note(
            id,
            Note::Dest {
                dest: stored.clone().into(),
                title: title.clone().map(Into::into),
                location: Some(location),
            },
        )?;
        self.dests.insert(stored, (id, title.unwrap_or_default()));
        Ok(Some(id))
    }

    pub(super) fn intern_foot(&mut self, name: &str) -> Result<NoteId, Error> {
        if let Some(&id) = self.foots.get(name) {
            return Ok(id);
        }
        let id = self.builder.reserve_note()?;
        self.foots.insert(name.to_string(), id);
        self.pending_foot.insert(id, name.to_string());
        Ok(id)
    }

    pub(super) fn define_referenced_notes(&mut self) -> Result<(), Error> {
        let mut processed = HashSet::new();
        loop {
            let mut pending: Vec<(NoteId, String)> = self
                .pending_foot
                .iter()
                .filter(|(id, _)| !processed.contains(*id))
                .map(|(id, name)| (*id, name.clone()))
                .collect();
            if pending.is_empty() {
                break;
            }
            pending.sort_by_key(|(id, _)| id.get_u32());
            for (id, name) in pending {
                if !processed.insert(id) {
                    continue;
                }
                let Some(node) = self.foot_defs.get(&name).copied() else {
                    return Err(Error::Note);
                };
                let was = self.in_note;
                self.in_note = true;
                let frames = self.blocks(node)?;
                self.in_note = was;
                self.builder.define_note(id, Note::Blocks(frames))?;
            }
        }
        Ok(())
    }

    pub(super) fn index_footnote_defs(&mut self, root: &'a comrak::nodes::AstNode<'a>) {
        for node in root.descendants() {
            if let NodeValue::FootnoteDefinition(def) = &node.data.borrow().value {
                self.foot_defs.entry(def.name.clone()).or_insert(node);
            }
        }
    }
}
