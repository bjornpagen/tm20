//! Lower a [`Sheet`] to a protocol [`tm20::Document`].

use crate::compose::compose;
use crate::error::Error;
use crate::face::FaceTable;
use crate::frame::Sheet;
use tm20::command::{CodePage, Command, CutKind};
use tm20::document::Document;

pub fn lower(sheet: &Sheet<'_>, faces: &FaceTable) -> Result<Document, Error> {
    let graphics = compose(sheet, faces)?;
    Ok(Document::new(vec![
        Command::Init,
        Command::CodePage(CodePage::Pc437),
        Command::Graphics(graphics),
        Command::Feed { lines: 3 },
        Command::Cut {
            kind: CutKind::Partial,
        },
    ]))
}
