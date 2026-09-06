//! Raster LaTeX with RaTeX. Faces stay in KaTeX; they never enter FaceTable.
//! Natural sources: `Math::from_png(bytes, ascent, depth)` — no sheet measure.

use ab_glyph::{Font, FontRef};
use ratex_font::FontId;
use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_parser::parser::parse;
use ratex_render::{RenderOptions, render_to_png};
use ratex_types::color::Color;
use ratex_types::display_item::{DisplayItem, DisplayList};
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
    let list = embedded_display_list(to_display_list(&lbox)).map_err(Error::MathDetail)?;
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

/// Admit only glyphs with KaTeX metrics and drawable embedded outlines.
/// The upstream renderer silently tries host fonts for missing outlines.
/// Canonicalize mathematical alphanumerics to their font's metric codepoints;
/// whitespace has layout extent but no drawing command. Neither needs fallback.
fn embedded_display_list(mut list: DisplayList) -> Result<DisplayList, String> {
    for item in &mut list.items {
        if let DisplayItem::GlyphPath {
            font, char_code, ..
        } = item
        {
            let id = FontId::parse(font).ok_or_else(|| format!("unknown math font {font}"))?;
            *char_code = u32::from(ratex_font::katex_ttf_glyph_char(id, *char_code));
            if matches!(
                id,
                FontId::CjkRegular | FontId::CjkFallback | FontId::EmojiFallback
            ) || ratex_font::get_char_metrics(id, *char_code).is_none()
            {
                return Err(format!(
                    "U+{char_code:04X} in {font} has no embedded math metrics; use a KaTeX-supported symbol or move the text outside math"
                ));
            }
        }
    }
    list.items.retain(|item| {
        !matches!(item, DisplayItem::GlyphPath { char_code, .. }
        if char::from_u32(*char_code).is_some_and(char::is_whitespace))
    });
    // No fallback font ids or metric-less glyphs reach this loader.
    let fonts = ratex_font_loader::load_fonts_for_items("", &list.items)?;
    for item in &list.items {
        if let DisplayItem::GlyphPath {
            font, char_code, ..
        } = item
        {
            let id = FontId::parse(font).ok_or_else(|| format!("unknown math font {font}"))?;
            let bytes = fonts
                .get(&id)
                .ok_or_else(|| format!("missing embedded math font {font}"))?;
            let face = FontRef::try_from_slice(bytes).map_err(|e| e.to_string())?;
            let ch = ratex_font::katex_ttf_glyph_char(id, *char_code);
            let glyph = face.glyph_id(ch);
            if glyph.0 == 0
                || ratex_font_loader::outline_cache::get_or_compute_outline(id, &face, glyph)
                    .is_none_or(|curves| curves.is_empty())
            {
                return Err(format!(
                    "U+{char_code:04X} has no drawable glyph in embedded {font}; use a supported math symbol"
                ));
            }
        }
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_alphanumeric_and_space_have_no_fallback_draw_commands() {
        for source in [r"𝑥", r"\text{a b}", r"\frac{1}{2}"] {
            assert!(inline(source, TextSize::Pt11).is_ok(), "{source}");
        }
    }

    #[test]
    fn absent_math_glyph_is_an_error_not_host_fallback() {
        for source in [r"\text{你}", r"\text{🦀}", r"\text{⌘}"] {
            let err = inline(source, TextSize::Pt11).unwrap_err();
            assert!(err.to_string().contains("embedded"), "{source}: {err}");
        }
    }
}
