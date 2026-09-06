//! Table lowering from the final contextual AST. Cells are inlines, not documents.

use comrak::nodes::{AstNode, NodeValue, TableAlignment};
use tm20_set::{ColAlign, Cols, Frame, GridSkip, Span};

use crate::error::Error;

use super::Cx;
use super::inline::{Voice, style_header};

impl<'a, L> Cx<'a, '_, L>
where
    L: FnMut(&str) -> Result<Vec<u8>, Error>,
{
    pub(super) fn table(
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
            let voice = Voice::Roman;
            let mut parsed = Vec::new();
            for cell in row.children() {
                if !matches!(cell.data.borrow().value, NodeValue::TableCell) {
                    return Err(Error::Html);
                }
                let mut spans = self.inlines(cell, voice)?;
                if header {
                    style_header(&mut spans);
                }
                parsed.push(spans);
            }
            if parsed.len() != n {
                return Err(Error::Cols);
            }
            rows.push(parsed);
        }
        Ok(Frame::Cols(cols_frame(self.text_size(), &align, rows)?))
    }
}

fn cols_frame(
    size: tm20_set::TextSize,
    align: &[ColAlign],
    rows: Vec<Vec<Vec<Span<'static>>>>,
) -> Result<Cols<'static>, Error> {
    match align.len() {
        2 => {
            let align = [align[0], align[1]];
            let rows = rows
                .into_iter()
                .map(|r| <[Vec<Span<'static>>; 2]>::try_from(r).map_err(|_| Error::Cols))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Cols::two(size, GridSkip::ONE, align, rows))
        }
        3 => {
            let align = [align[0], align[1], align[2]];
            let rows = rows
                .into_iter()
                .map(|r| <[Vec<Span<'static>>; 3]>::try_from(r).map_err(|_| Error::Cols))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Cols::three(size, GridSkip::ONE, align, rows))
        }
        _ => Err(Error::Cols),
    }
}
