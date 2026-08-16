//! Parsed OpenType. Optical role is a second parse into [`TextFace`] or
//! [`DisplayFace`]. Which [`Cut`] a face fills is decided outside this crate.

use fontdue::{Font as RasterFont, FontSettings};
use harfrust::{Feature, FontRef, ShapeOptions, ShaperData, Tag, UnicodeBuffer};

use crate::error::Error;
use crate::size::{DisplaySize, TextSize};

/// Named voice. The sheet writes these; [`FaceTable`] says what they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Cut {
    Light,
    Roman,
    Italic,
    Medium,
    Bold,
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
            Cut::Light => "Light",
            Cut::Roman => "Roman",
            Cut::Italic => "Italic",
            Cut::Medium => "Medium",
            Cut::Bold => "Bold",
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

/// Parsed face. Not an authoring type; call [`text`](Self::text) or [`display`](Self::display).
pub struct Face {
    bytes: Vec<u8>,
    index: u32,
    raster: RasterFont,
    hb: ShaperData,
}

/// Text optical role. Accepts only [`TextSize`].
pub struct TextFace(Face);

/// Display optical role. Accepts only [`DisplaySize`].
pub struct DisplayFace(Face);

enum ShapeKind {
    Run,
    Figure,
    Mark,
}

impl Face {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        Self::from_bytes_index(bytes, 0)
    }

    pub fn from_bytes_index(bytes: Vec<u8>, index: u32) -> Result<Self, Error> {
        let font = FontRef::from_index(&bytes, index).map_err(|_| Error::Font)?;
        let raster = RasterFont::from_bytes(bytes.as_slice(), FontSettings::default())
            .map_err(|_| Error::Font)?;
        let hb = ShaperData::new(&font);
        Ok(Self {
            bytes,
            index,
            raster,
            hb,
        })
    }

    pub fn text(self) -> TextFace {
        TextFace(self)
    }

    pub fn display(self) -> DisplayFace {
        DisplayFace(self)
    }

    pub(crate) fn ascent(&self, px: f32) -> f32 {
        self.raster
            .horizontal_line_metrics(px)
            .expect("parsed face has hhea metrics")
            .ascent
    }

    fn shape(&self, text: &str, px: f32, kind: ShapeKind, tracking: f32) -> Shaped {
        if text.is_empty() {
            return Shaped {
                glyphs: Vec::new(),
                width: 0.0,
                ascent: self.ascent(px),
            };
        }
        let font = FontRef::from_index(&self.bytes, self.index).expect("Face bytes already parsed");
        let shaper = self.hb.shaper(&font).build();
        let mut buf = UnicodeBuffer::new();
        buf.push_str(text);
        buf.guess_segment_properties();
        let features = ot_features(kind);
        let glyphs = shaper.shape(buf, ShapeOptions::new().features(&features));
        let scale = px / shaper.units_per_em() as f32;
        let mut x = 0.0;
        let mut placed = Vec::with_capacity(glyphs.len());
        let infos = glyphs.glyph_infos();
        let positions = glyphs.glyph_positions();
        for (i, (info, pos)) in infos.iter().zip(positions).enumerate() {
            placed.push(Placed {
                glyph_id: info.glyph_id as u16,
                x: x + pos.x_offset as f32 * scale,
                y: pos.y_offset as f32 * scale,
            });
            x += pos.x_advance as f32 * scale;
            if tracking != 0.0 && i + 1 < infos.len() {
                x += tracking;
            }
        }
        Shaped {
            glyphs: placed,
            width: x,
            ascent: self.ascent(px),
        }
    }

    pub(crate) fn raster_glyph(&self, glyph_id: u16, px: f32) -> (fontdue::Metrics, Vec<u8>) {
        self.raster.rasterize_indexed(glyph_id, px)
    }
}

impl TextFace {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        Ok(Face::from_bytes(bytes)?.text())
    }

    pub(crate) fn px(size: TextSize) -> f32 {
        size.pt() * crate::DPI / 72.0
    }

    pub(crate) fn shape(&self, text: &str, size: TextSize) -> Shaped {
        self.0.shape(text, Self::px(size), ShapeKind::Run, 0.0)
    }

    pub(crate) fn shape_figure(&self, text: &str, size: TextSize) -> Shaped {
        self.0.shape(text, Self::px(size), ShapeKind::Figure, 0.0)
    }

    pub(crate) fn inner(&self) -> &Face {
        &self.0
    }
}

impl DisplayFace {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        Ok(Face::from_bytes(bytes)?.display())
    }

    pub(crate) fn px(size: DisplaySize) -> f32 {
        size.pt() * crate::DPI / 72.0
    }

    pub(crate) fn shape(&self, text: &str, size: DisplaySize, tracking_em: i16) -> Shaped {
        let px = Self::px(size);
        let tracking = px * tracking_em as f32 / 1000.0;
        self.0.shape(text, px, ShapeKind::Mark, tracking)
    }

    pub(crate) fn inner(&self) -> &Face {
        &self.0
    }
}

pub(crate) struct Placed {
    pub glyph_id: u16,
    pub x: f32,
    pub y: f32,
}

pub(crate) struct Shaped {
    pub glyphs: Vec<Placed>,
    pub width: f32,
    pub ascent: f32,
}

fn ot_features(kind: ShapeKind) -> Vec<Feature> {
    let mut f = vec![
        Feature::new(Tag::new(b"kern"), 1, ..),
        Feature::new(Tag::new(b"liga"), 1, ..),
        Feature::new(Tag::new(b"calt"), 1, ..),
    ];
    match kind {
        ShapeKind::Run => {}
        ShapeKind::Figure => {
            f.push(Feature::new(Tag::new(b"tnum"), 1, ..));
            f.push(Feature::new(Tag::new(b"lnum"), 1, ..));
        }
        ShapeKind::Mark => {
            f.push(Feature::new(Tag::new(b"case"), 1, ..));
        }
    }
    f
}
