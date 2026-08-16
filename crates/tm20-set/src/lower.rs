//! Lower a [`Sheet`] to a protocol [`tm20::Document`].

use crate::compose::compose;
use crate::error::Error;
use crate::frame::Sheet;
use tm20::command::{CodePage, Command, CutKind};
use tm20::document::Document;

pub fn lower(sheet: &Sheet<'_>) -> Result<Document, Error> {
    let graphics = compose(sheet)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::face::{Slope, TextFace, Weight};
    use crate::frame::{Frame, Sheet, TextBlock};
    use crate::size::TextSize;
    use tm20::encode::encode;

    #[test]
    fn ticket_encodes() {
        let face = TextFace::sans(Weight::Roman, Slope::Upright).expect("sans");
        let sheet = Sheet::tape(vec![Frame::Text(TextBlock::plain(
            &face,
            TextSize::Pt11,
            "ok",
        ))]);
        let bytes = encode(&lower(&sheet).unwrap()).unwrap();
        assert!(bytes.windows(3).any(|w| w == [0x1d, b'(', b'L']));
    }
}
