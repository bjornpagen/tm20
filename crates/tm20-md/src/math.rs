//! Raster LaTeX with RaTeX. Faces stay in KaTeX; they never enter FaceTable.

use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_parser::parser::parse;
use ratex_render::{RenderOptions, render_to_png};
use ratex_types::color::Color;
use ratex_types::math_style::MathStyle;
use tm20_set::{Math, TextSize};

use crate::error::Error;

pub fn inline(latex: &str, size: TextSize, measure: u16) -> Result<Math, Error> {
    raster(latex, MathStyle::Text, size, measure)
}

/// One TeX box. Wider than the measure shrinks; there is no atom list to wrap.
pub fn display(latex: &str, size: TextSize, measure: u16) -> Result<Math, Error> {
    raster(latex, MathStyle::Display, size, measure)
}

fn raster(latex: &str, style: MathStyle, size: TextSize, measure: u16) -> Result<Math, Error> {
    let ast = parse(latex).map_err(|_| Error::Math)?;
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
    .map_err(|_| Error::Math)?;
    let ascent = (lbox.height as f32 * font_size).round().max(0.0) as u16;
    let depth = (lbox.depth as f32 * font_size).round().max(0.0) as u16;
    Math::from_png(&png, measure, ascent, depth).map_err(|_| Error::Math)
}
