//! Font bytes → a parsed [`Face`]. Optical role is a second parse into
//! [`TextFace`] or [`DisplayFace`]. Strike pixels are cached on the face.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use fontdue::{Font as RasterFont, FontSettings};
use harfrust::{FontRef, ShaperData, Tag, UnicodeBuffer};

use crate::error::Error;
use crate::strike::{self, SharedStrike, Strike};

/// Parsed face. Not an authoring type; call [`text`](Self::text) or [`display`](Self::display).
///
/// Strike and HarfRust buffers are `RefCell`-cached, so a face is not `Sync`.
/// There is no process-global font registry; the cache dies with this value.
pub struct Face {
    bytes: Arc<[u8]>,
    index: u32,
    raster: Arc<RasterFont>,
    hb: Arc<ShaperData>,
    buf: RefCell<Option<UnicodeBuffer>>,
    upem: u16,
    italic_tan: f32,
    strikes: RefCell<HashMap<(u16, u16), SharedStrike>>,
}

/// Text optical role. Accepts only [`crate::size::TextSize`].
#[derive(Clone)]
pub struct TextFace(Face);

/// Display optical role. Accepts only [`crate::size::DisplaySize`].
#[derive(Clone)]
pub struct DisplayFace(Face);

impl Clone for Face {
    /// Share immutable font data, but keep shaping buffers and strike caches local.
    fn clone(&self) -> Self {
        Self {
            bytes: Arc::clone(&self.bytes),
            index: self.index,
            raster: Arc::clone(&self.raster),
            hb: Arc::clone(&self.hb),
            buf: RefCell::new(Some(UnicodeBuffer::new())),
            upem: self.upem,
            italic_tan: self.italic_tan,
            strikes: RefCell::new(HashMap::new()),
        }
    }
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
        let hb = ShaperData::new(&font);
        let italic_tan = italic_tan(&font);
        Ok(Self {
            bytes,
            index,
            raster: Arc::new(raster),
            hb: Arc::new(hb),
            buf: RefCell::new(Some(UnicodeBuffer::new())),
            upem,
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

    pub(crate) fn reopen(&self) -> Self {
        self.clone()
    }

    /// Inspect the collection directory before paying to decode every glyph.
    pub(super) fn name_at(bytes: &[u8], index: u32) -> Result<Option<String>, Error> {
        let font = FontRef::from_index(bytes, index).map_err(|_| Error::Font)?;
        Ok(font
            .table_data(Tag::new(b"name"))
            .and_then(|data| parse_postscript_name(data.as_bytes())))
    }

    /// PostScript name (name id 6). [`super::HOUSE`] maps this to a [`super::Voice`].
    pub fn postscript_name(&self) -> Option<String> {
        let data = self.font().table_data(Tag::new(b"name"))?;
        parse_postscript_name(data.as_bytes())
    }

    pub(crate) fn font(&self) -> FontRef<'_> {
        FontRef::from_index(self.bytes.as_ref(), self.index).expect("Face bytes already parsed")
    }

    pub(crate) fn hb(&self) -> &ShaperData {
        &self.hb
    }

    pub(crate) fn take_buf(&self) -> UnicodeBuffer {
        self.buf.borrow_mut().take().unwrap_or_default()
    }

    pub(crate) fn put_buf(&self, buf: UnicodeBuffer) {
        *self.buf.borrow_mut() = Some(buf);
    }

    pub(crate) fn upem(&self) -> u16 {
        self.upem
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

    /// Shared immutable strike. A cache hit clones the [`Arc`], not the pixels.
    pub(crate) fn strike(&self, glyph_id: u16, ppem: u16) -> SharedStrike {
        {
            let cache = self.strikes.borrow();
            if let Some(s) = cache.get(&(glyph_id, ppem)) {
                return Arc::clone(s);
            }
        }
        let s = Arc::new(self.paint_strike(glyph_id, ppem));
        self.strikes
            .borrow_mut()
            .insert((glyph_id, ppem), Arc::clone(&s));
        s
    }
}

impl TextFace {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        Ok(Face::from_bytes(bytes)?.text())
    }

    pub(crate) fn inner(&self) -> &Face {
        &self.0
    }
}

impl DisplayFace {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        Ok(Face::from_bytes(bytes)?.display())
    }

    pub(crate) fn inner(&self) -> &Face {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::Face;
    use std::sync::Arc;

    #[test]
    fn cloned_faces_share_parsed_data_but_not_mutable_caches() {
        let face = Face::from_bytes(std::fs::read("/System/Library/Fonts/Helvetica.ttc").unwrap())
            .unwrap();
        let strike = face.strike(40, 31);
        let cloned = face.clone();
        assert!(Arc::ptr_eq(&face.raster, &cloned.raster));
        assert!(Arc::ptr_eq(&face.hb, &cloned.hb));
        assert!(cloned.strikes.borrow().is_empty());
        let cloned_strike = cloned.strike(40, 31);
        assert!(!Arc::ptr_eq(&strike, &cloned_strike));
        assert_eq!(strike.bits(), cloned_strike.bits());
    }
}
