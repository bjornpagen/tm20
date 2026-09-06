//! Frozen layout/paint values. Only the layout owner constructs [`LayoutPlan`].

use std::sync::Arc;

use tm20::Raster;

use crate::error::Error;
use crate::face::ShapedRun;
use crate::frame::NonEmpty;
use crate::geometry::{Advance, Clip, InkBounds, Measure, Row, RowRange};
use crate::size::TextSize;

/// Lexicographic column-allocation cost. No NaN or fallback-to-zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct WrapCost {
    pub extra_lines: u32,
    pub raggedness: u128,
}

impl WrapCost {
    pub fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            extra_lines: self.extra_lines.checked_add(other.extra_lines)?,
            raggedness: self.raggedness.checked_add(other.raggedness)?,
        })
    }
}

/// Figure/tabular digit mode for a wrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigitMode {
    Proportional,
    Tabular,
}

/// Positioned paint atom on a measured line.
#[derive(Clone)]
pub enum PaintAtom {
    Rule {
        bounds: InkBounds,
    },
    Glyph {
        run: Arc<ShapedRun>,
        x: Advance,
        bounds: InkBounds,
    },
    Bitmap {
        raster: Raster,
        x: Advance,
        bounds: InkBounds,
    },
}

/// Immutable measured line. Blank owns a slug; ink owns nonempty atoms.
#[derive(Clone)]
pub enum LinePlan {
    Blank {
        slug: Advance,
    },
    Ink {
        atoms: NonEmpty<PaintAtom>,
        advance: Advance,
        bounds: InkBounds,
    },
}

/// Wrap result: lines plus the cost of the selected plan.
#[derive(Clone)]
pub struct LinePlans {
    pub lines: Vec<LinePlan>,
    pub cost: WrapCost,
}

/// Closed draw list consumed by paint. The plan owns operations.
#[derive(Clone)]
pub enum DrawOp {
    GlyphRun {
        run: ShapedRun,
        x: Advance,
        baseline: Advance,
        clip: Clip,
    },
    Bitmap {
        raster: Raster,
        x: i32,
        top: Row,
        clip: Clip,
    },
    Rule {
        rows: RowRange,
        clip: Clip,
    },
}

/// Sealed layout result. Only [`super::layout`] constructs this.
#[derive(Clone)]
pub struct LayoutPlan {
    measure: Measure,
    height: u32,
    ops: Vec<DrawOp>,
    seams: Vec<Row>,
}

impl LayoutPlan {
    pub(crate) fn new(
        measure: Measure,
        height: u32,
        ops: Vec<DrawOp>,
        seams: Vec<Row>,
    ) -> Result<Self, Error> {
        if height == 0 {
            return Err(Error::CoordinateOverflow);
        }
        Ok(Self {
            measure,
            height,
            ops,
            seams,
        })
    }

    pub fn measure(&self) -> Measure {
        self.measure
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn ops(&self) -> &[DrawOp] {
        &self.ops
    }

    pub fn seams(&self) -> &[Row] {
        &self.seams
    }
}

pub fn wrap_text(
    size: TextSize,
    spans: &[crate::frame::Span<'_>],
    measure: Measure,
    faces: &crate::face::ResolvedFaces<'_>,
    digits: DigitMode,
) -> Result<LinePlans, Error> {
    super::inline::wrap_text(size, spans, measure, faces, digits)
}
