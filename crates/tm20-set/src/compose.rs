//! Small evaluator: [`Sheet`] + [`FaceTable`] → a tape-wide raster.
//! Facade owns validation/layout/paint orchestration.

mod bands;
mod blocks;
mod canvas;
mod inline;
pub mod plan;
mod rhythm;

pub use plan::{
    DigitMode, DrawOp, LayoutPlan, LinePlan, LinePlans, PaintAtom, WrapCost, wrap_text,
};

use crate::error::Error;
use crate::face::{FaceTable, ResolvedFaces};
use crate::frame::{CheckedSheet, Sheet};
use crate::geometry::{Row, RowRange};
use tm20::Raster;

pub(crate) const NEST_CAP: u8 = 3;
pub(crate) const NOTE_RAISE_NUM: i32 = 2;
pub(crate) const NOTE_RAISE_DEN: i32 = 5;

/// Validate → layout → paint. Bands are a later narrowing, not this return.
pub fn compose(sheet: &Sheet<'_>, faces: &FaceTable) -> Result<Raster, Error> {
    let checked = sheet.admit()?;
    let resolved = faces.resolve(&checked.face_requirements())?;
    paint(&layout(&checked, &resolved)?)
}

pub fn layout(
    sheet: &CheckedSheet<'_, '_>,
    faces: &ResolvedFaces<'_>,
) -> Result<LayoutPlan, Error> {
    blocks::layout(sheet, faces)
}

pub fn paint(plan: &LayoutPlan) -> Result<Raster, Error> {
    canvas::paint(plan)
}

pub fn select_bands(height: u32, cap: u16, seams: &[Row]) -> Result<Vec<RowRange>, Error> {
    bands::select_bands(height, cap, seams)
}

/// Slice a painted page on admitted ranges and construct one [`tm20::graphics::Graphics`] each.
pub fn graphics_bands(
    page: &Raster,
    ranges: &[RowRange],
) -> Result<Vec<tm20::graphics::Graphics>, Error> {
    bands::graphics_bands(page, ranges)
}

/// Partition a painted page with the encode cap for its width.
pub fn page_bands(page: &Raster, seams: &[Row]) -> Result<Vec<tm20::graphics::Graphics>, Error> {
    bands::page_bands(page, seams)
}
