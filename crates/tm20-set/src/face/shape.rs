//! HarfRust shaping and strike attachment. One interpretation of a glyph buffer.

use harfrust::font::FontFuncs;
use harfrust::{Feature, GlyphId, ShapeOptions, Tag};
use std::sync::Arc;

use crate::error::Error;
use crate::geometry::{Advance, InkBounds};
use crate::size::{DisplaySize, FRAC, TextSize};
use crate::strike::{SharedStrike, Strike};

use super::parse::{DisplayFace, Face, TextFace};

pub(super) fn scale(ppem: u16) -> i32 {
    i32::from(ppem) * FRAC
}

#[derive(Clone, Copy)]
enum ShapeKind {
    Run,
    Figure,
    Mark,
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
        raw * scale(self.ppem) / i32::from(self.face.upem().max(1))
    }
}

/// One glyph of a [`ShapedRun`]: pen offset in 26.6 and a shared strike.
#[derive(Clone)]
pub struct PositionedGlyph {
    glyph_id: u16,
    x: Advance,
    y: Advance,
    strike: SharedStrike,
}

impl PositionedGlyph {
    pub fn glyph_id(&self) -> u16 {
        self.glyph_id
    }

    pub fn x(&self) -> Advance {
        self.x
    }

    pub fn y(&self) -> Advance {
        self.y
    }

    pub fn strike(&self) -> &Strike {
        &self.strike
    }

    #[cfg(test)]
    pub(crate) fn strike_arc(&self) -> &SharedStrike {
        &self.strike
    }

    /// Strike ink translated by this glyph's pen. `y` is HarfRust (up); ink is y-down.
    pub fn ink(&self) -> Result<InkBounds, Error> {
        let dy = Advance::from_units(
            self.y
                .units()
                .checked_neg()
                .ok_or(Error::CoordinateOverflow)?,
        );
        self.strike
            .ink()
            .translate(self.x, dy)
            .ok_or(Error::CoordinateOverflow)
    }
}

/// Immutable shaped run: ordered glyphs, checked 26.6 advance, actual ink bounds.
#[derive(Clone)]
pub struct ShapedRun {
    text: Arc<str>,
    glyphs: Vec<PositionedGlyph>,
    advance: Advance,
    ink: InkBounds,
}

impl ShapedRun {
    /// Admit the complete metric domain, not just individually valid endpoints.
    fn new(glyphs: Vec<PositionedGlyph>, advance: Advance, text: Arc<str>) -> Result<Self, Error> {
        let ink = union_ink(&glyphs)?;
        if !ink.is_empty() {
            ink.y0()
                .units()
                .checked_neg()
                .ok_or(Error::CoordinateOverflow)?;
            let overhang = (i128::from(ink.x1().units()) - i128::from(advance.units())).max(0);
            i64::try_from(overhang).map_err(|_| Error::CoordinateOverflow)?;
        }
        Ok(Self {
            text,
            glyphs,
            advance,
            ink,
        })
    }

    pub fn empty() -> Self {
        Self {
            text: Arc::from(""),
            glyphs: Vec::new(),
            advance: Advance::ZERO,
            ink: InkBounds::EMPTY,
        }
    }

    pub fn glyphs(&self) -> &[PositionedGlyph] {
        &self.glyphs
    }

    /// Decoded text behind this run, retained for layout diagnostics.
    pub(crate) fn text(&self) -> &Arc<str> {
        &self.text
    }

    pub fn advance(&self) -> Advance {
        self.advance
    }

    pub fn ink(&self) -> InkBounds {
        self.ink
    }

    /// Distance from the baseline up to the top of ink, in 26.6. Zero if none.
    pub fn ascent(&self) -> Advance {
        if self.ink.is_empty() {
            return Advance::ZERO;
        }
        Advance::from_units((-self.ink.y0().units()).max(0))
    }

    /// Distance from the baseline down to the bottom of ink, in 26.6. Zero if none.
    pub fn depth(&self) -> Advance {
        if self.ink.is_empty() {
            return Advance::ZERO;
        }
        Advance::from_units(self.ink.y1().units().max(0))
    }

    /// Ink beyond the pen advance, to the right. Zero if the ink is inside.
    pub fn overhang(&self) -> Advance {
        if self.ink.is_empty() || self.ink.x1() <= self.advance {
            return Advance::ZERO;
        }
        Advance::from_units(self.ink.x1().units() - self.advance.units())
    }

    /// Checked translation of every glyph pen. `dy` is HarfRust-up (same as
    /// [`PositionedGlyph::y`]). Paint uses `origin_y = round(baseline - g.y()) - top`.
    /// Ink is y-down, so a positive `dy` raises painted and measured ink together.
    pub fn translate(&self, dx: Advance, dy: Advance) -> Result<Self, Error> {
        if self.glyphs.is_empty() && dx == Advance::ZERO && dy == Advance::ZERO {
            return Ok(self.clone());
        }
        let mut glyphs = Vec::new();
        glyphs
            .try_reserve(self.glyphs.len())
            .map_err(|_| Error::Alloc)?;
        for g in &self.glyphs {
            glyphs.push(PositionedGlyph {
                glyph_id: g.glyph_id,
                x: g.x.checked_add(dx).ok_or(Error::CoordinateOverflow)?,
                y: g.y.checked_add(dy).ok_or(Error::CoordinateOverflow)?,
                strike: g.strike.clone(),
            });
        }
        Self::new(glyphs, self.advance, Arc::clone(&self.text))
    }
}

fn features_for(kind: ShapeKind, out: &mut [Feature; 5]) -> usize {
    match kind {
        ShapeKind::Run => {
            out[0] = Feature::new(Tag::new(b"kern"), 1, ..);
            out[1] = Feature::new(Tag::new(b"liga"), 1, ..);
            out[2] = Feature::new(Tag::new(b"calt"), 1, ..);
            3
        }
        ShapeKind::Figure => {
            out[0] = Feature::new(Tag::new(b"kern"), 1, ..);
            out[1] = Feature::new(Tag::new(b"liga"), 1, ..);
            out[2] = Feature::new(Tag::new(b"calt"), 1, ..);
            out[3] = Feature::new(Tag::new(b"tnum"), 1, ..);
            out[4] = Feature::new(Tag::new(b"lnum"), 1, ..);
            5
        }
        ShapeKind::Mark => {
            out[0] = Feature::new(Tag::new(b"kern"), 1, ..);
            out[1] = Feature::new(Tag::new(b"liga"), 1, ..);
            out[2] = Feature::new(Tag::new(b"calt"), 1, ..);
            out[3] = Feature::new(Tag::new(b"case"), 1, ..);
            4
        }
    }
}

fn union_ink(glyphs: &[PositionedGlyph]) -> Result<InkBounds, Error> {
    let mut ink = InkBounds::EMPTY;
    for g in glyphs {
        ink = ink.union(g.ink()?).ok_or(Error::CoordinateOverflow)?;
    }
    Ok(ink)
}

impl Face {
    fn shape_run(
        &self,
        text: &str,
        ppem: u16,
        kind: ShapeKind,
        tracking: i32,
    ) -> Result<ShapedRun, Error> {
        if text.is_empty() {
            return Ok(ShapedRun::empty());
        }
        let font = self.font();
        let shaper = self.hb().shaper(&font).build();
        let mut buf = self.take_buf();
        buf.clear();
        buf.push_str(text);
        buf.guess_segment_properties();
        let mut advances = StrikeAdvance { face: self, ppem };
        let mut stored = [Feature::new(Tag::new(b"kern"), 1, ..); 5];
        let n = features_for(kind, &mut stored);
        let glyph_buf = shaper.shape(
            buf,
            ShapeOptions::new()
                .scale(Some(scale(ppem)))
                .features(&stored[..n])
                .font_funcs(Some(&mut advances)),
        );
        let (glyphs, advance) = {
            let infos = glyph_buf.glyph_infos();
            let positions = glyph_buf.glyph_positions();
            let mut x = 0i32;
            let mut glyphs = Vec::with_capacity(infos.len());
            for (i, (info, pos)) in infos.iter().zip(positions).enumerate() {
                let glyph_id = info.glyph_id as u16;
                if glyph_id == 0 {
                    return Err(Error::MissingGlyph {
                        text: text.to_owned(),
                    });
                }
                let gx = x
                    .checked_add(pos.x_offset)
                    .ok_or(Error::CoordinateOverflow)?;
                glyphs.push(PositionedGlyph {
                    glyph_id,
                    x: Advance::from_units(i64::from(gx)),
                    y: Advance::from_units(i64::from(pos.y_offset)),
                    strike: self.strike(glyph_id, ppem),
                });
                x = x
                    .checked_add(pos.x_advance)
                    .ok_or(Error::CoordinateOverflow)?;
                if tracking != 0 && i + 1 < infos.len() {
                    x = x.checked_add(tracking).ok_or(Error::CoordinateOverflow)?;
                }
            }
            (glyphs, Advance::from_units(i64::from(x)))
        };
        self.put_buf(glyph_buf.clear());
        ShapedRun::new(glyphs, advance, Arc::from(text))
    }
}

impl TextFace {
    /// Proportional run: kern/liga/calt. Bind size before shaping.
    pub fn shape_run(&self, text: &str, size: TextSize) -> Result<ShapedRun, Error> {
        self.inner().shape_run(text, size.ppem(), ShapeKind::Run, 0)
    }

    /// Tabular/lining figures: kern/liga/calt plus tnum/lnum.
    pub fn shape_figure_run(&self, text: &str, size: TextSize) -> Result<ShapedRun, Error> {
        self.inner()
            .shape_run(text, size.ppem(), ShapeKind::Figure, 0)
    }
}

impl DisplayFace {
    /// Display mark: kern/liga/calt plus case. `tracking_em` is thousandths of an em.
    pub fn shape_run(
        &self,
        text: &str,
        size: DisplaySize,
        tracking_em: i16,
    ) -> Result<ShapedRun, Error> {
        let ppem = size.ppem();
        let tracking = scale(ppem) * i32::from(tracking_em) / 1000;
        self.inner()
            .shape_run(text, ppem, ShapeKind::Mark, tracking)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::face::{Cut, DisplayCut, FaceTable};
    use crate::geometry::Advance;
    use crate::size::TextSize;
    use std::sync::Arc;

    fn house_table() -> FaceTable {
        let mut table = FaceTable::new();
        table.absorb(
            std::fs::read("/System/Library/Fonts/Helvetica.ttc")
                .expect("Helvetica.ttc not on this machine — locked house face required"),
        );
        table.absorb(
            std::fs::read("/System/Library/Fonts/Menlo.ttc")
                .expect("Menlo.ttc not on this machine — locked house face required"),
        );
        table
    }

    #[test]
    fn descender_and_ascent_live_in_ink() {
        let table = house_table();
        let roman = table.text(Cut::Roman).expect("Helvetica Roman");
        let run = roman.shape_run("gyp", TextSize::Pt11).expect("shape gyp");
        assert!(!run.ink().is_empty());
        assert!(
            run.ascent().units() > 0,
            "caps/ascenders sit above baseline"
        );
        assert!(
            run.depth().units() > 0,
            "g/y/p must contribute descender ink"
        );
        assert!(run.ink().y0().units() < 0);
        assert!(run.ink().y1().units() > 0);
        let advance = run.advance().units();
        assert!(advance > 0);
        let last = run.glyphs().last().expect("glyphs");
        assert!(last.x().units() < advance);
    }

    #[test]
    fn negative_bearing_is_inside_bounds() {
        let table = house_table();
        let italic = table.text(Cut::Italic).expect("Helvetica Oblique");
        let run = ["f", "j", "y", "T", "A"]
            .into_iter()
            .find_map(|s| {
                let r = italic.shape_run(s, TextSize::Pt11).ok()?;
                let g = r.glyphs().first()?;
                if g.strike().left() < 0 || r.ink().x0().units() < 0 {
                    Some(r)
                } else {
                    None
                }
            })
            .expect("italic face must expose a left-bearing glyph");
        let g = &run.glyphs()[0];
        let ink = g.ink().expect("glyph ink is representable");
        assert!(!ink.is_empty());
        let run_ink = run.ink();
        assert!(run_ink.x0().units() <= ink.x0().units());
        assert!(run_ink.x1().units() >= ink.x1().units());
        assert!(run_ink.y0().units() <= ink.y0().units());
        assert!(run_ink.y1().units() >= ink.y1().units());
    }

    #[test]
    fn repeated_glyphs_share_strike_owner() {
        let table = house_table();
        let roman = table.text(Cut::Roman).expect("Helvetica Roman");
        let once = roman.shape_run("A", TextSize::Pt11).expect("A");
        let twice = roman.shape_run("AA", TextSize::Pt11).expect("AA");
        assert_eq!(twice.glyphs().len(), 2);
        assert!(
            Arc::ptr_eq(
                twice.glyphs()[0].strike_arc(),
                twice.glyphs()[1].strike_arc()
            ),
            "same glyph at one size shares the cached strike"
        );
        assert!(
            Arc::ptr_eq(
                once.glyphs()[0].strike_arc(),
                twice.glyphs()[0].strike_arc()
            ),
            "a later shape must hit the same immutable owner"
        );
    }

    #[test]
    fn text_and_display_sizes_stay_distinct() {
        let table = house_table();
        let roman = table.text(Cut::Roman).expect("text Roman");
        let display = table.display(DisplayCut::Roman).expect("display Roman");
        let text = roman.shape_run("H", TextSize::Pt11).expect("text H");
        let mark = display
            .shape_run("H", crate::size::DisplaySize::Pt18, 0)
            .expect("display H");
        assert_ne!(text.advance(), mark.advance());
        assert_ne!(text.ink(), mark.ink());
    }

    #[test]
    fn empty_run_is_empty() {
        let table = house_table();
        let roman = table.text(Cut::Roman).expect("Roman");
        let run = roman.shape_run("", TextSize::Pt11).expect("empty");
        assert!(run.glyphs().is_empty());
        assert_eq!(run.advance(), Advance::ZERO);
        assert!(run.ink().is_empty());
    }

    #[test]
    fn translated_metrics_reject_unrepresentable_ascent() {
        let table = house_table();
        let run = table
            .text(Cut::Roman)
            .unwrap()
            .shape_run("H", TextSize::Pt11)
            .unwrap();
        let dy = i64::try_from(i128::from(run.ink().y0().units()) - i128::from(i64::MIN)).unwrap();
        assert!(matches!(
            run.translate(Advance::ZERO, Advance::from_units(dy)),
            Err(Error::CoordinateOverflow)
        ));
        let edge = run
            .translate(Advance::ZERO, Advance::from_units(dy - 1))
            .unwrap();
        assert_eq!(edge.ascent().units(), i64::MAX);
    }

    #[test]
    fn translate_near_signed_limits() {
        let table = house_table();
        let roman = table.text(Cut::Roman).expect("Roman");
        let run = roman.shape_run("H", TextSize::Pt11).expect("H");
        assert!(!run.ink().is_empty());
        let ok = run
            .translate(Advance::from_units(100), Advance::from_units(-20))
            .expect("modest translate");
        assert!(!ok.ink().is_empty());
        assert_eq!(
            ok.glyphs()[0].x().units(),
            run.glyphs()[0].x().units() + 100
        );
        assert_eq!(ok.glyphs()[0].y().units(), run.glyphs()[0].y().units() - 20);
        let overflow = run.translate(Advance::from_units(i64::MAX), Advance::ZERO);
        assert!(
            matches!(overflow, Err(Error::CoordinateOverflow)),
            "pen overflow is an error, not empty ink"
        );
        let neg = run.translate(Advance::ZERO, Advance::from_units(i64::MIN));
        assert!(
            matches!(neg, Err(Error::CoordinateOverflow)),
            "y negation overflow is an error, not a panic"
        );
    }
}
