//! Parsed OpenType. Optical role is a second parse into [`TextFace`] or [`DisplayFace`].

use std::path::Path;

use font_kit::family_name::FamilyName;
use font_kit::handle::Handle;
use font_kit::properties::{Properties, Weight as KitWeight};
use font_kit::source::SystemSource;
use fontdue::{Font as RasterFont, FontSettings};
use harfrust::{Feature, FontRef, ShapeOptions, ShaperData, Tag, UnicodeBuffer};

use crate::error::Error;
use crate::size::{DisplaySize, TextSize};

/// CSS-like weight used only when matching a system family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Roman,
    Bold,
}

impl Weight {
    fn kit(self) -> KitWeight {
        match self {
            Weight::Roman => KitWeight(400.0),
            Weight::Bold => KitWeight(700.0),
        }
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

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::from_bytes(std::fs::read(path)?)
    }

    fn from_handle(handle: Handle) -> Result<Self, Error> {
        match handle {
            Handle::Path { path, font_index } => {
                Self::from_bytes_index(std::fs::read(path)?, font_index)
            }
            Handle::Memory { bytes, font_index } => {
                Self::from_bytes_index((*bytes).clone(), font_index)
            }
        }
    }

    pub fn system(family: &str, weight: Weight) -> Result<Self, Error> {
        Self::from_handle(match_family(
            &[FamilyName::Title(family.into())],
            weight,
            family,
        )?)
    }

    pub fn sans(weight: Weight) -> Result<Self, Error> {
        Self::from_handle(match_family(
            &[FamilyName::SansSerif],
            weight,
            "sans-serif",
        )?)
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

    pub(crate) fn shape(&self, text: &str, px: f32, tabular: bool, tracking: f32) -> Shaped {
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
        let features = ot_features(tabular);
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

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, Error> {
        Ok(Face::from_path(path)?.text())
    }

    pub fn system(family: &str, weight: Weight) -> Result<Self, Error> {
        Ok(Face::system(family, weight)?.text())
    }

    pub fn sans(weight: Weight) -> Result<Self, Error> {
        Ok(Face::sans(weight)?.text())
    }

    pub(crate) fn px(size: TextSize) -> f32 {
        size.pt() * crate::DPI / 72.0
    }

    pub(crate) fn shape(&self, text: &str, size: TextSize, tabular: bool) -> Shaped {
        self.0.shape(text, Self::px(size), tabular, 0.0)
    }

    pub(crate) fn inner(&self) -> &Face {
        &self.0
    }
}

impl DisplayFace {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        Ok(Face::from_bytes(bytes)?.display())
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, Error> {
        Ok(Face::from_path(path)?.display())
    }

    pub fn system(family: &str, weight: Weight) -> Result<Self, Error> {
        Ok(Face::system(family, weight)?.display())
    }

    pub fn sans(weight: Weight) -> Result<Self, Error> {
        Ok(Face::sans(weight)?.display())
    }

    pub(crate) fn px(size: DisplaySize) -> f32 {
        size.pt() * crate::DPI / 72.0
    }

    pub(crate) fn shape(&self, text: &str, size: DisplaySize, tracking_em: i16) -> Shaped {
        let px = Self::px(size);
        let tracking = px * tracking_em as f32 / 1000.0;
        self.0.shape(text, px, false, tracking)
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

fn ot_features(tabular: bool) -> Vec<Feature> {
    let mut f = vec![
        Feature::new(Tag::new(b"kern"), 1, ..),
        Feature::new(Tag::new(b"liga"), 1, ..),
        Feature::new(Tag::new(b"calt"), 1, ..),
    ];
    if tabular {
        f.push(Feature::new(Tag::new(b"tnum"), 1, ..));
        f.push(Feature::new(Tag::new(b"lnum"), 1, ..));
    }
    f
}

fn match_family(names: &[FamilyName], weight: Weight, label: &str) -> Result<Handle, Error> {
    SystemSource::new()
        .select_best_match(names, Properties::new().weight(weight.kit()))
        .map_err(|_| Error::NotFound {
            family: label.into(),
        })
}
