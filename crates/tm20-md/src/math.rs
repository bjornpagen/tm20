//! Raster LaTeX with RaTeX. Faces stay in KaTeX; they never enter FaceTable.
//! Natural sources: `Math::from_png(bytes, ascent, depth)` — no sheet measure.

use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_parser::parser::parse;
use ratex_render::{RenderOptions, render_to_png};
use ratex_types::color::Color;
use ratex_types::math_style::MathStyle;
use tm20_set::{Math, TextSize};

use crate::error::Error;

pub fn inline(latex: &str, size: TextSize) -> Result<Math, Error> {
    raster(latex, MathStyle::Text, size)
}

/// One TeX box. Wider than the local measure shrinks at fit time; there is no
/// atom list to wrap.
pub fn display(latex: &str, size: TextSize) -> Result<Math, Error> {
    raster(latex, MathStyle::Display, size)
}

fn raster(latex: &str, style: MathStyle, size: TextSize) -> Result<Math, Error> {
    let ast = parse(latex).map_err(|e| Error::MathDetail(e.to_string()))?;
    let lbox = layout(&ast, &LayoutOptions::default().with_style(style));
    let list = to_display_list(&lbox);
    let font_size = size.body_dots() as f32;
    let png = render_to_png(
        &list,
        &RenderOptions {
            font_size,
            padding: 0.0,
            background_color: Color::WHITE,
            font_dir: String::new(),
            device_pixel_ratio: 1.0,
        },
    )
    .map_err(Error::MathDetail)?;
    let ascent = (lbox.height as f32 * font_size).round().max(0.0) as u32;
    let depth = (lbox.depth as f32 * font_size).round().max(0.0) as u32;
    Math::from_png(&png, ascent, depth).map_err(Error::from)
}
