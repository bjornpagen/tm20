//! Lower a [`Sheet`] to a protocol [`tm20::Document`].

use crate::compose::compose_bands;
use crate::error::Error;
use crate::face::FaceTable;
use crate::frame::Sheet;
use tm20::command::{CodePage, Command};
use tm20::document::Document;

pub fn lower(sheet: &Sheet<'_>, faces: &FaceTable) -> Result<Document, Error> {
    let bands = compose_bands(sheet, faces)?;
    let mut cmds = Vec::with_capacity(bands.len() + 4);
    cmds.push(Command::Init);
    cmds.push(Command::CodePage(CodePage::Pc437));
    cmds.extend(bands.into_iter().map(Command::Graphics));
    cmds.push(Command::Feed { lines: 3 });
    cmds.push(Command::Cut);
    Ok(Document::new(cmds))
}
