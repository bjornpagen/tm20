//! Parsed CFF OpenType. Optical role is a second parse into [`TextFace`] or
//! [`DisplayFace`]. Which [`Cut`] a face fills is decided outside this crate.

use std::cell::RefCell;
use std::collections::HashMap;

use harfrust::font::FontFuncs;
use harfrust::{Feature, FontRef, GlyphId, ShapeOptions, ShaperData, Tag, UnicodeBuffer};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{
    DrawSettings, HintingInstance, OutlineGlyphCollection, OutlineGlyphFormat, Target,
};
use skrifa::raw::TableProvider;

use crate::error::Error;
use crate::size::{DisplaySize, FRAC, TextSize};
use crate::strike::{self, Path, Strike};

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
    hb: ShaperData,
    buf: RefCell<Option<UnicodeBuffer>>,
    upem: u16,
    ascent: i16,
    hinters: RefCell<HashMap<u16, HintingInstance>>,
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

struct Hinted<'a> {
    face: &'a Face,
    ppem: u16,
}

impl FontFuncs for Hinted<'_> {
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
        OutlineGlyphCollection::with_format(&font, OutlineGlyphFormat::Cff).ok_or(Error::Font)?;
        let upem = font.head().map_err(|_| Error::Font)?.units_per_em();
        if upem == 0 {
            return Err(Error::Font);
        }
        let ascent = font.hhea().map_err(|_| Error::Font)?.ascender().to_i16();
        let hb = ShaperData::new(&font);
        Ok(Self {
            bytes,
            index,
            hb,
            buf: RefCell::new(Some(UnicodeBuffer::new())),
            upem,
            ascent,
            hinters: RefCell::new(HashMap::new()),
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

    fn ensure_hinter(&self, ppem: u16) {
        if self.hinters.borrow().contains_key(&ppem) {
            return;
        }
        let font = self.font();
        let outlines = OutlineGlyphCollection::with_format(&font, OutlineGlyphFormat::Cff)
            .expect("CFF checked at parse");
        let hinter = HintingInstance::new(
            &outlines,
            Size::new(f32::from(ppem)),
            LocationRef::default(),
            Target::Mono,
        )
        .expect("CFF hinter");
        self.hinters.borrow_mut().insert(ppem, hinter);
    }

    fn paint_strike(&self, glyph_id: u16, ppem: u16) -> Strike {
        self.ensure_hinter(ppem);
        let font = self.font();
        let outlines = OutlineGlyphCollection::with_format(&font, OutlineGlyphFormat::Cff)
            .expect("CFF checked at parse");
        let linear = font
            .glyph_metrics(Size::new(f32::from(ppem)), LocationRef::default())
            .advance_width(GlyphId::from(glyph_id))
            .unwrap_or(0.0);
        let Some(glyph) = outlines.get(GlyphId::from(glyph_id)) else {
            return Strike::empty((linear * FRAC as f32).round() as i32);
        };
        let hinters = self.hinters.borrow();
        let hinter = hinters.get(&ppem).expect("ensure_hinter");
        let mut path = Path::default();
        match glyph.draw(DrawSettings::hinted(hinter, false), &mut path) {
            Ok(metrics) => {
                let advance_px = metrics.advance_width.unwrap_or(linear);
                strike::from_pen(&path, advance_px)
            }
            Err(_) => Strike::empty((linear * FRAC as f32).round() as i32),
        }
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
        let mut hinted = Hinted { face: self, ppem };
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
                        .font_funcs(Some(&mut hinted)),
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
                        .font_funcs(Some(&mut hinted)),
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
                        .font_funcs(Some(&mut hinted)),
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
