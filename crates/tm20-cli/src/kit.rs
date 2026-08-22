//! Faces this machine brings. The library only parses bytes; [`tm20_set::HOUSE`]
//! is the name → voice table.

use std::io;

use tm20_set::FaceTable;

use crate::Result;

const HELVETICA: &str = "/System/Library/Fonts/Helvetica.ttc";
const MENLO: &str = "/System/Library/Fonts/Menlo.ttc";

pub fn system_table() -> Result<FaceTable> {
    let mut table = FaceTable::new();
    table.absorb(read_font(HELVETICA)?);
    table.absorb(read_font(MENLO)?);
    Ok(table)
}

fn read_font(path: &str) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|_| {
        io::Error::new(io::ErrorKind::NotFound, format!("{path} not on this machine")).into()
    })
}
