//! Parsed sfnt face (CFF or glyf, one file or a collection). Optical role is a
//! second parse into [`TextFace`] or [`DisplayFace`]. Which [`Cut`] a face fills
//! is decided outside this crate. HarfRust shapes; fontdue paints; this crate
//! caches the strike.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use fontdue::{Font as RasterFont, FontSettings};
use harfrust::font::FontFuncs;
use harfrust::{Feature, FontRef, GlyphId, ShapeOptions, ShaperData, Tag, UnicodeBuffer};

use crate::error::Error;
use crate::size::{DisplaySize, TextSize, FRAC};
use crate::strike::{self, Strike};

/// Named text voice. The sheet writes these; [`FaceTable`] says what they are.
/// Display voices are [`DisplayCut`]: a Light paragraph or a Mono masthead is
/// unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Cut {
    Roman,
    Italic,
    Bold,
    BoldItalic,
    Mono,
}

impl Cut {
    const COUNT: usize = 5;

    fn index(self) -> usize {
        self as usize
    }
}

impl std::fmt::Display for Cut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Cut::Roman => "Roman",
            Cut::Italic => "Italic",
            Cut::Bold => "Bold",
            Cut::BoldItalic => "BoldItalic",
            Cut::Mono => "Mono",
        })
    }
}

/// Named display voice. Only a [`crate::Mark`] speaks at display size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DisplayCut {
    Roman,
    Light,
}

/// Optical slot a PostScript name fills. One name may fill text and display
/// (Helvetica Regular is both Roman voices). The assignment is this table,
/// not a match in each loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voice {
    Text(Cut),
    Display(DisplayCut),
    Both(Cut, DisplayCut),
}

/// House names. Paths stay with the program that reads the files; this table
/// is the parse from a collection face onto a [`Cut`].
pub const HOUSE: &[(&str, Voice)] = &[
    ("Helvetica", Voice::Both(Cut::Roman, DisplayCut::Roman)),
    ("Helvetica-Bold", Voice::Text(Cut::Bold)),
    ("Helvetica-Oblique", Voice::Text(Cut::Italic)),
    ("Helvetica-BoldOblique", Voice::Text(Cut::BoldItalic)),
    ("Helvetica-Light", Voice::Display(DisplayCut::Light)),
    ("Menlo-Regular", Voice::Text(Cut::Mono)),
];

impl DisplayCut {
    const COUNT: usize = 2;

    fn index(self) -> usize {
        self as usize
    }
}

impl std::fmt::Display for DisplayCut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DisplayCut::Roman => "Roman",
            DisplayCut::Light => "Light",
        })
    }
}

/// Loaded cuts. The sheet names [`Cut`]s; this table is what those names mean.
#[derive(Default)]
pub struct FaceTable {
    text: [Option<TextFace>; Cut::COUNT],
    display: [Option<DisplayFace>; DisplayCut::COUNT],
}

impl FaceTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_text(&mut self, cut: Cut, face: TextFace) {
        self.text[cut.index()] = Some(face);
    }

    pub fn set_display(&mut self, cut: DisplayCut, face: DisplayFace) {
        self.display[cut.index()] = Some(face);
    }

    /// Offer a parsed face to the slots its PostScript name owns. Unknown
    /// names are ignored — a collection is a bag, not a kit.
    pub fn offer(&mut self, face: Face) -> bool {
        let Some(name) = face.postscript_name() else {
            return false;
        };
        let Some((_, voice)) = HOUSE.iter().find(|(n, _)| *n == name.as_str()) else {
            return false;
        };
        match *voice {
            Voice::Text(cut) => self.set_text(cut, face.text()),
            Voice::Display(cut) => self.set_display(cut, face.display()),
            Voice::Both(text, display) => {
                self.set_display(display, face.reopen().display());
                self.set_text(text, face.text());
            }
        }
        true
    }

    /// Walk every face in an sfnt collection and [`offer`](Self::offer) each.
    /// The first unparseable index is the end of the collection.
    pub fn absorb(&mut self, bytes: impl Into<Arc<[u8]>>) {
        let bytes = bytes.into();
        for index in 0.. {
            let Ok(face) = Face::from_bytes_index(bytes.clone(), index) else {
                break;
            };
            self.offer(face);
        }
    }

    pub fn text(&self, cut: Cut) -> Result<&TextFace, Error> {
        self.text[cut.index()]
            .as_ref()
            .ok_or(Error::MissingText(cut))
    }

    pub fn display(&self, cut: DisplayCut) -> Result<&DisplayFace, Error> {
        self.display[cut.index()]
            .as_ref()
            .ok_or(Error::MissingDisplay(cut))
    }

    /// Parse this table into a [`Kit`]. Every text cut and the Roman display
    /// are required once, here; Light stays optional until a Mark names it.
    pub fn kit(&self) -> Result<Kit<'_>, Error> {
        Ok(Kit {
            text: [
                self.text(Cut::Roman)?,
                self.text(Cut::Italic)?,
                self.text(Cut::Bold)?,
                self.text(Cut::BoldItalic)?,
                self.text(Cut::Mono)?,
            ],
            display: self.display(DisplayCut::Roman)?,
            light: self.display(DisplayCut::Light).ok(),
        })
    }
}

/// Proof that a [`FaceTable`] covers every voice a sheet can name. Compose
/// parses one at its boundary; paint indexes and never checks again.
pub struct Kit<'f> {
    /// Indexed by [`Cut`] discriminant; [`FaceTable::kit`] builds it in order.
    text: [&'f TextFace; Cut::COUNT],
    display: &'f DisplayFace,
    light: Option<&'f DisplayFace>,
}

impl<'f> Kit<'f> {
    pub(crate) fn text(&self, cut: Cut) -> &'f TextFace {
        self.text[cut.index()]
    }

    pub(crate) fn display(&self, cut: DisplayCut) -> Result<&'f DisplayFace, Error> {
        match cut {
            DisplayCut::Roman => Ok(self.display),
            DisplayCut::Light => self.light.ok_or(Error::MissingDisplay(DisplayCut::Light)),
        }
    }
}

/// Parsed face. Not an authoring type; call [`text`](Self::text) or [`display`](Self::display).
pub struct Face {
    bytes: Arc<[u8]>,
    index: u32,
    raster: RasterFont,
    hb: ShaperData,
    buf: RefCell<Option<UnicodeBuffer>>,
    upem: u16,
    ascent: i16,
    italic_tan: f32,
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

fn italic_tan(font: &FontRef<'_>) -> f32 {
    let Some(post) = font.table_data(Tag::new(b"post")) else {
        return 0.0;
    };
    let bytes = post.as_bytes();
    let Ok(raw) = bytes.get(4..8).unwrap_or(&[]).try_into() else {
        return 0.0;
    };
    let degrees = i32::from_be_bytes(raw) as f32 / 65536.0;
    degrees.to_radians().tan().abs()
}

fn parse_postscript_name(name: &[u8]) -> Option<String> {
    let count = u16_be(name, 2)? as usize;
    let storage = u16_be(name, 4)? as usize;
    let mut best: Option<(u8, String)> = None;
    for i in 0..count {
        let rec = 6 + i * 12;
        let platform = u16_be(name, rec)?;
        let encoding = u16_be(name, rec + 2)?;
        let name_id = u16_be(name, rec + 6)?;
        if name_id != 6 {
            continue;
        }
        let len = u16_be(name, rec + 8)? as usize;
        let off = u16_be(name, rec + 10)? as usize;
        let bytes = name.get(storage + off..storage + off + len)?;
        let rank = match (platform, encoding) {
            (3, 1 | 10) => 0,
            (0, _) => 1,
            (1, 0) => 2,
            _ => 3,
        };
        let Some(s) = decode_name(platform, encoding, bytes) else {
            continue;
        };
        if s.is_empty() {
            continue;
        }
        if best.as_ref().is_none_or(|(r, _)| rank < *r) {
            best = Some((rank, s));
        }
    }
    best.map(|(_, s)| s)
}

fn u16_be(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn decode_name(platform: u16, encoding: u16, bytes: &[u8]) -> Option<String> {
    match (platform, encoding) {
        (0, _) | (3, 1 | 10) => {
            if !bytes.len().is_multiple_of(2) {
                return None;
            }
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_be_bytes(*c))
                .collect();
            String::from_utf16(&units).ok()
        }
        _ => {
            if bytes.iter().all(|b| *b < 128) {
                Some(bytes.iter().map(|&b| char::from(b)).collect())
            } else {
                None
            }
        }
    }
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

    pub fn from_bytes_index(bytes: impl Into<Arc<[u8]>>, index: u32) -> Result<Self, Error> {
        let bytes = bytes.into();
        let font = FontRef::from_index(bytes.as_ref(), index).map_err(|_| Error::Font)?;
        let raster = RasterFont::from_bytes(
            bytes.as_ref(),
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
        let italic_tan = italic_tan(&font);
        Ok(Self {
            bytes,
            index,
            raster,
            hb,
            buf: RefCell::new(Some(UnicodeBuffer::new())),
            upem,
            ascent,
            italic_tan,
            strikes: RefCell::new(HashMap::new()),
        })
    }

    pub(crate) fn italic_tan(&self) -> f32 {
        self.italic_tan
    }

    pub fn text(self) -> TextFace {
        TextFace(self)
    }

    pub fn display(self) -> DisplayFace {
        DisplayFace(self)
    }

    fn reopen(&self) -> Self {
        Self::from_bytes_index(Arc::clone(&self.bytes), self.index)
            .expect("Face bytes already parsed")
    }

    /// PostScript name (name id 6). [`HOUSE`] maps this to a [`Voice`].
    pub fn postscript_name(&self) -> Option<String> {
        let data = self.font().table_data(Tag::new(b"name"))?;
        parse_postscript_name(data.as_bytes())
    }

    fn font(&self) -> FontRef<'_> {
        FontRef::from_index(self.bytes.as_ref(), self.index).expect("Face bytes already parsed")
    }

    fn ascent_frac(&self, ppem: u16) -> i32 {
        i32::from(self.ascent) * scale(ppem) / i32::from(self.upem)
    }

    fn paint_strike(&self, glyph_id: u16, ppem: u16) -> Strike {
        if glyph_id >= self.raster.glyph_count() {
            return Strike::empty(0);
        }
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
    fn helvetica_ttc_parses() {
        let bytes = std::fs::read("/System/Library/Fonts/Helvetica.ttc").expect("Helvetica.ttc");
        let face = Face::from_bytes_index(bytes, 0).expect("glyf OpenType");
        let name = face.postscript_name().expect("PostScript name");
        assert!(name.starts_with("Helvetica"), "{name}");
    }

    #[test]
    fn kit_indexes_text_by_cut() {
        let mut table = FaceTable::new();
        table.absorb(std::fs::read("/System/Library/Fonts/Helvetica.ttc").expect("Helvetica.ttc"));
        table.absorb(std::fs::read("/System/Library/Fonts/Menlo.ttc").expect("Menlo.ttc"));
        let kit = table.kit().expect("house collections fill a kit");
        for cut in [
            Cut::Roman,
            Cut::Italic,
            Cut::Bold,
            Cut::BoldItalic,
            Cut::Mono,
        ] {
            assert!(
                std::ptr::eq(kit.text(cut), table.text(cut).unwrap()),
                "kit[{cut}] must be the table's {cut}"
            );
        }
        assert!(
            kit.display(DisplayCut::Light).is_ok(),
            "Helvetica-Light is a house voice"
        );
    }

    #[test]
    fn kit_without_light_is_still_a_kit() {
        let mut table = FaceTable::new();
        table.absorb(std::fs::read("/System/Library/Fonts/Menlo.ttc").expect("Menlo.ttc"));
        let bytes: Arc<[u8]> = std::fs::read("/System/Library/Fonts/Helvetica.ttc")
            .expect("Helvetica.ttc")
            .into();
        for index in 0.. {
            let Ok(face) = Face::from_bytes_index(bytes.clone(), index) else {
                break;
            };
            if face.postscript_name().as_deref() == Some("Helvetica-Light") {
                continue;
            }
            table.offer(face);
        }
        let kit = table.kit().expect("Light is optional until a Mark names it");
        assert!(matches!(
            kit.display(DisplayCut::Light),
            Err(Error::MissingDisplay(DisplayCut::Light))
        ));
    }

    #[test]
    fn house_names_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for (name, _) in HOUSE {
            assert!(seen.insert(*name), "duplicate house name {name}");
        }
    }

    #[test]
    fn helvetica_ttc_names_the_cuts() {
        let bytes: Arc<[u8]> = std::fs::read("/System/Library/Fonts/Helvetica.ttc")
            .expect("Helvetica.ttc")
            .into();
        let mut names = Vec::new();
        for index in 0.. {
            let Ok(face) = Face::from_bytes_index(bytes.clone(), index) else {
                break;
            };
            names.push(face.postscript_name().unwrap_or_default());
        }
        for need in [
            "Helvetica",
            "Helvetica-Bold",
            "Helvetica-Oblique",
            "Helvetica-BoldOblique",
        ] {
            assert!(names.iter().any(|n| n == need), "{need} not in {names:?}");
        }
    }
}
