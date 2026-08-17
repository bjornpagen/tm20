//! Parsed CFF OpenType. Optical role is a second parse into [`TextFace`] or
//! [`DisplayFace`]. Which [`Cut`] a face fills is decided outside this crate.
//! HarfRust shapes; fontdue paints; this crate caches the strike.

use std::cell::RefCell;
use std::collections::HashMap;

use fontdue::{Font as RasterFont, FontSettings};
use harfrust::font::FontFuncs;
use harfrust::{Feature, FontRef, GlyphId, ShapeOptions, ShaperData, Tag, UnicodeBuffer};

use crate::error::Error;
use crate::size::{DisplaySize, FRAC, TextSize};
use crate::strike::{self, Strike};

/// Named voice. The sheet writes these; [`FaceTable`] says what they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Cut {
    Light,
    Roman,
    Italic,
    Medium,
    Bold,
    BoldItalic,
    Mono,
}

impl Cut {
    const COUNT: usize = 7;

    fn index(self) -> usize {
        self as usize
    }
}

impl std::fmt::Display for Cut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Cut::Light => "Light",
            Cut::Roman => "Roman",
            Cut::Italic => "Italic",
            Cut::Medium => "Medium",
            Cut::Bold => "Bold",
            Cut::BoldItalic => "BoldItalic",
            Cut::Mono => "Mono",
        })
    }
}

/// Loaded cuts. The sheet names [`Cut`]s; this table is what those names mean.
#[derive(Default)]
pub struct FaceTable {
    text: [Option<TextFace>; Cut::COUNT],
    display: [Option<DisplayFace>; Cut::COUNT],
}

impl FaceTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_text(&mut self, cut: Cut, face: TextFace) {
        self.text[cut.index()] = Some(face);
    }

    pub fn set_display(&mut self, cut: Cut, face: DisplayFace) {
        self.display[cut.index()] = Some(face);
    }

    pub fn text(&self, cut: Cut) -> Result<&TextFace, Error> {
        self.text[cut.index()]
            .as_ref()
            .ok_or(Error::MissingText(cut))
    }

    pub fn display(&self, cut: Cut) -> Result<&DisplayFace, Error> {
        self.display[cut.index()]
            .as_ref()
            .ok_or(Error::MissingDisplay(cut))
    }
}

/// Parsed CFF face. Not an authoring type; call [`text`](Self::text) or [`display`](Self::display).
pub struct Face {
    bytes: Vec<u8>,
    index: u32,
    raster: RasterFont,
    hb: ShaperData,
    buf: RefCell<Option<UnicodeBuffer>>,
    upem: u16,
    ascent: i16,
    strikes: RefCell<HashMap<(u16, u16), Strike>>,
}

/// Text optical role. Accepts only [`TextSize`].
pub struct TextFace(Face);

/// Display optical role. Accepts only [`DisplaySize`].
pub struct DisplayFace(Face);

#[derive(Clone, Copy)]
enum ShapeKind {
    Run,
    Figure,
    Mark,
}

fn scale(ppem: u16) -> i32 {
    i32::from(ppem) * FRAC
}

/// CFF OpenType is `OTTO` plus a `CFF ` or `CFF2` table. TrueType is a different magic.
fn is_cff_sfnt(data: &[u8]) -> bool {
    let Some(magic) = data.get(..4) else {
        return false;
    };
    if magic != b"OTTO" {
        return false;
    }
    let Some(n) = data.get(4..6) else {
        return false;
    };
    let n = u16::from_be_bytes([n[0], n[1]]) as usize;
    for i in 0..n {
        let rec = 12 + i * 16;
        let Some(tag) = data.get(rec..rec + 4) else {
            return false;
        };
        if tag == b"CFF " || tag == b"CFF2" {
            return true;
        }
    }
    false
}

struct StrikeAdvance<'a> {
    face: &'a Face,
    ppem: u16,
}

impl FontFuncs for StrikeAdvance<'_> {
    fn advance_width(&mut self, builtin: &harfrust::font::BuiltinFontFuncs, glyph: GlyphId) -> i32 {
        let id = u16::try_from(glyph.to_u32()).unwrap_or(0);
        let strike = self.face.strike(id, self.ppem);
        if strike.advance != 0 || strike.width != 0 {
            return strike.advance;
        }
        let raw = builtin.advance_width(glyph);
        raw * scale(self.ppem) / i32::from(self.face.upem.max(1))
    }
}

impl Face {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        Self::from_bytes_index(bytes, 0)
    }

    pub fn from_bytes_index(bytes: Vec<u8>, index: u32) -> Result<Self, Error> {
        let font = FontRef::from_index(&bytes, index).map_err(|_| Error::Font)?;
        if !is_cff_sfnt(&bytes) {
            return Err(Error::Font);
        }
        let raster = RasterFont::from_bytes(
            bytes.as_slice(),
            FontSettings {
                collection_index: index,
                ..FontSettings::default()
            },
        )
        .map_err(|_| Error::Font)?;
        let upem_f = raster.units_per_em();
        if !(upem_f > 0.0 && upem_f <= f32::from(u16::MAX)) {
            return Err(Error::Font);
        }
        let upem = upem_f as u16;
        let ascent = raster
            .horizontal_line_metrics(upem_f)
            .ok_or(Error::Font)?
            .ascent
            .round() as i16;
        let hb = ShaperData::new(&font);
        Ok(Self {
            bytes,
            index,
            raster,
            hb,
            buf: RefCell::new(Some(UnicodeBuffer::new())),
            upem,
            ascent,
            strikes: RefCell::new(HashMap::new()),
        })
    }

    pub fn text(self) -> TextFace {
        TextFace(self)
    }

    pub fn display(self) -> DisplayFace {
        DisplayFace(self)
    }

    fn font(&self) -> FontRef<'_> {
        FontRef::from_index(&self.bytes, self.index).expect("Face bytes already parsed")
    }

    fn ascent_frac(&self, ppem: u16) -> i32 {
        i32::from(self.ascent) * scale(ppem) / i32::from(self.upem)
    }

    fn paint_strike(&self, glyph_id: u16, ppem: u16) -> Strike {
        let (metrics, bitmap) = self.raster.rasterize_indexed(glyph_id, f32::from(ppem));
        let width = u32::try_from(metrics.width).unwrap_or(0);
        let height = u32::try_from(metrics.height).unwrap_or(0);
        let top = metrics.ymin.saturating_add(height as i32);
        strike::from_mask(
            metrics.xmin,
            top,
            width,
            height,
            &bitmap,
            metrics.advance_width,
        )
    }

    pub(crate) fn strike(&self, glyph_id: u16, ppem: u16) -> Strike {
        {
            let cache = self.strikes.borrow();
            if let Some(s) = cache.get(&(glyph_id, ppem)) {
                return s.clone();
            }
        }
        let s = self.paint_strike(glyph_id, ppem);
        self.strikes
            .borrow_mut()
            .insert((glyph_id, ppem), s.clone());
        s
    }

    fn shape(&self, text: &str, ppem: u16, kind: ShapeKind, tracking: i32) -> Shaped {
        let ascent = self.ascent_frac(ppem);
        if text.is_empty() {
            return Shaped {
                glyphs: Vec::new(),
                width: 0,
                ascent,
            };
        }
        let font = self.font();
        let shaper = self.hb.shaper(&font).build();
        let mut buf = self.buf.borrow_mut().take().unwrap_or_default();
        buf.clear();
        buf.push_str(text);
        buf.guess_segment_properties();
        let mut advances = StrikeAdvance { face: self, ppem };
        let glyph_buf = match kind {
            ShapeKind::Run => {
                let features = [
                    Feature::new(Tag::new(b"kern"), 1, ..),
                    Feature::new(Tag::new(b"liga"), 1, ..),
                    Feature::new(Tag::new(b"calt"), 1, ..),
                ];
                shaper.shape(
                    buf,
                    ShapeOptions::new()
                        .scale(Some(scale(ppem)))
                        .features(&features)
                        .font_funcs(Some(&mut advances)),
                )
            }
            ShapeKind::Figure => {
                let features = [
                    Feature::new(Tag::new(b"kern"), 1, ..),
                    Feature::new(Tag::new(b"liga"), 1, ..),
                    Feature::new(Tag::new(b"calt"), 1, ..),
                    Feature::new(Tag::new(b"tnum"), 1, ..),
                    Feature::new(Tag::new(b"lnum"), 1, ..),
                ];
                shaper.shape(
                    buf,
                    ShapeOptions::new()
                        .scale(Some(scale(ppem)))
                        .features(&features)
                        .font_funcs(Some(&mut advances)),
                )
            }
            ShapeKind::Mark => {
                let features = [
                    Feature::new(Tag::new(b"kern"), 1, ..),
                    Feature::new(Tag::new(b"liga"), 1, ..),
                    Feature::new(Tag::new(b"calt"), 1, ..),
                    Feature::new(Tag::new(b"case"), 1, ..),
                ];
                shaper.shape(
                    buf,
                    ShapeOptions::new()
                        .scale(Some(scale(ppem)))
                        .features(&features)
                        .font_funcs(Some(&mut advances)),
                )
            }
        };
        let (placed, x) = {
            let infos = glyph_buf.glyph_infos();
            let positions = glyph_buf.glyph_positions();
            let mut x = 0i32;
            let mut placed = Vec::with_capacity(infos.len());
            for (i, (info, pos)) in infos.iter().zip(positions).enumerate() {
                placed.push(Placed {
                    glyph_id: info.glyph_id as u16,
                    x: x + pos.x_offset,
                    y: pos.y_offset,
                });
                x += pos.x_advance;
                if tracking != 0 && i + 1 < infos.len() {
                    x += tracking;
                }
            }
            (placed, x)
        };
        *self.buf.borrow_mut() = Some(glyph_buf.clear());
        Shaped {
            glyphs: placed,
            width: x,
            ascent,
        }
    }
}

impl TextFace {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        Ok(Face::from_bytes(bytes)?.text())
    }

    pub(crate) fn shape(&self, text: &str, size: TextSize) -> Shaped {
        self.0.shape(text, size.ppem(), ShapeKind::Run, 0)
    }

    pub(crate) fn shape_figure(&self, text: &str, size: TextSize) -> Shaped {
        self.0.shape(text, size.ppem(), ShapeKind::Figure, 0)
    }

    pub(crate) fn inner(&self) -> &Face {
        &self.0
    }
}

impl DisplayFace {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        Ok(Face::from_bytes(bytes)?.display())
    }

    pub(crate) fn shape(&self, text: &str, size: DisplaySize, tracking_em: i16) -> Shaped {
        let ppem = size.ppem();
        let tracking = scale(ppem) * i32::from(tracking_em) / 1000;
        self.0.shape(text, ppem, ShapeKind::Mark, tracking)
    }

    pub(crate) fn inner(&self) -> &Face {
        &self.0
    }
}

pub(crate) struct Placed {
    pub glyph_id: u16,
    pub x: i32,
    pub y: i32,
}

pub(crate) struct Shaped {
    pub glyphs: Vec<Placed>,
    pub width: i32,
    pub ascent: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_bytes_are_font_error() {
        assert!(matches!(
            Face::from_bytes(vec![0, 1, 2, 3]),
            Err(Error::Font)
        ));
    }

    #[test]
    fn truetype_is_font_error() {
        let bytes = std::fs::read("/System/Library/Fonts/Helvetica.ttc").expect("Helvetica.ttc");
        assert!(matches!(Face::from_bytes_index(bytes, 0), Err(Error::Font)));
    }
}
