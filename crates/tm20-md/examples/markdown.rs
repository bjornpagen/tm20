//! Device-free Markdown → ESC/POS. Redirect stdout to a file, not a terminal.
use std::io::{self, Write};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let faces = tm20_set::FaceTable::portable()?;
    let sheet = tm20_md::sheet(
        "# Hello\n\nPrinted **locally**.",
        tm20_set::Measure::TAPE,
        |url| tm20_md::image_bytes(Path::new("."), url),
    )?;
    let document = tm20_set::lower(&sheet, &faces)?;
    let bytes = tm20::encode(&document)?;
    io::stdout().lock().write_all(&bytes)?;
    Ok(())
}
