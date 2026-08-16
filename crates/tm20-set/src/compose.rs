//! Small evaluator: [`Sheet`] → packed [`tm20::Graphics`].

use crate::error::Error;
use crate::face::{DisplayFace, Face, Shaped, TextFace};
use crate::frame::{Frame, MarkAlign, Pair, Sheet, TextBlock};
use crate::leading::{GridSkip, Leading};
use crate::size::TextSize;
use tm20::graphics::{pack, Graphics, GraphicsScale};

const THRESHOLD: u8 = 96;

struct Canvas {
    width: u16,
    height: u16,
    bits: Vec<bool>,
}

impl Canvas {
    fn new(width: u16) -> Self {
        Self {
            width,
            height: 0,
            bits: Vec::new(),
        }
    }

    fn ensure(&mut self, h: u16) {
        if h <= self.height {
            return;
        }
        self.bits.resize(self.width as usize * h as usize, false);
        self.height = h;
    }

    fn set(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 {
            return;
        }
        let x = x as u16;
        let y = y as u16;
        if x >= self.width {
            return;
        }
        self.ensure(y + 1);
        self.bits[y as usize * self.width as usize + x as usize] = true;
    }

    fn fill_row(&mut self, y: u16, x0: u16, x1: u16) {
        self.ensure(y + 1);
        let x1 = x1.min(self.width);
        for x in x0..x1 {
            self.bits[y as usize * self.width as usize + x as usize] = true;
        }
    }

    fn into_graphics(self) -> Result<Graphics, Error> {
        let height = self.height.max(1);
        let width = self.width;
        let mut bits = self.bits;
        bits.resize(width as usize * height as usize, false);
        let pixels = pack(width, height, &bits).map_err(|_| Error::Overflow {
            width: width as u32,
            height: height as u32,
        })?;
        Ok(Graphics {
            width_dots: width,
            height_dots: height,
            pixels,
            scale: GraphicsScale::Normal,
        })
    }
}

/// Layout `sheet` onto one 1-bit canvas and pack it.
pub fn compose(sheet: &Sheet<'_>) -> Result<Graphics, Error> {
    let width = sheet.width.get();
    let mut canvas = Canvas::new(width);
    let mut baseline: Option<f32> = None;

    for frame in &sheet.frames {
        match frame {
            Frame::Skip(skip) => match baseline {
                None => {
                    canvas.ensure(skip.dots());
                }
                Some(b) => {
                    let b = b + skip.dots() as f32;
                    baseline = Some(b);
                    canvas.ensure(b.ceil() as u16);
                }
            },
            Frame::Rule(rule) => {
                let y = baseline.unwrap_or(0.0).round().max(0.0) as u16;
                for dy in 0..rule.thickness.dots() {
                    canvas.fill_row(y + dy, 0, width);
                }
                let next = y as f32 + GridSkip::ONE.dots() as f32;
                baseline = Some(next);
            }
            Frame::Mark(mark) => {
                let shaped = mark.face.shape(mark.text, mark.size, mark.tracking.0);
                let skip = Leading::for_display(mark.size).skip_dots(mark.size.body_dots());
                let b = advance_baseline(&mut baseline, shaped.ascent, skip);
                let x0 = match mark.align {
                    MarkAlign::Start => 0.0,
                    MarkAlign::Center => ((width as f32 - shaped.width) * 0.5).max(0.0),
                };
                blit(
                    &mut canvas,
                    mark.face.inner(),
                    DisplayFace::px(mark.size),
                    x0,
                    b,
                    &shaped,
                );
            }
            Frame::Text(block) => {
                paint_text(&mut canvas, &mut baseline, width, block, false)?;
            }
            Frame::Pair(pair) => {
                paint_pair(&mut canvas, &mut baseline, width, pair)?;
            }
        }
    }

    canvas.into_graphics()
}

fn advance_baseline(baseline: &mut Option<f32>, ascent: f32, skip: u16) -> f32 {
    let b = match *baseline {
        None => ascent,
        Some(prev) => prev + skip as f32,
    };
    *baseline = Some(b);
    b
}

fn paint_text(
    canvas: &mut Canvas,
    baseline: &mut Option<f32>,
    width: u16,
    block: &TextBlock<'_>,
    _tabular: bool,
) -> Result<(), Error> {
    let measure = (width.saturating_sub(block.indent)).max(1);
    let skip = Leading::for_text(block.size).skip_dots(block.size.body_dots());
    let paras: Vec<&str> = block.text.split("\n\n").collect();
    for (i, para) in paras.iter().enumerate() {
        if i > 0 {
            if let Some(b) = baseline {
                *b += GridSkip::ONE.dots() as f32;
            }
        }
        for line in wrap_para(block.face, block.size, para, measure as f32) {
            let b = advance_baseline(baseline, line.ascent, skip);
            blit(
                canvas,
                block.face.inner(),
                TextFace::px(block.size),
                block.indent as f32,
                b,
                &line,
            );
        }
    }
    Ok(())
}

fn paint_pair(
    canvas: &mut Canvas,
    baseline: &mut Option<f32>,
    width: u16,
    pair: &Pair<'_>,
) -> Result<(), Error> {
    let right = pair.right_face.shape(pair.right, pair.right_size, true);
    let gutter = pair.gutter.dots();
    let right_w = right.width.ceil() as u16;
    let left_measure = width.saturating_sub(gutter).saturating_sub(right_w).max(1);
    let skip = Leading::for_text(pair.left_size).skip_dots(pair.left_size.body_dots());
    let lines = wrap_para(
        pair.left_face,
        pair.left_size,
        pair.left,
        left_measure as f32,
    );
    for (i, line) in lines.iter().enumerate() {
        let b = advance_baseline(baseline, line.ascent, skip);
        blit(
            canvas,
            pair.left_face.inner(),
            TextFace::px(pair.left_size),
            0.0,
            b,
            line,
        );
        if i == 0 {
            let x = (width as f32 - right.width).max(0.0);
            blit(
                canvas,
                pair.right_face.inner(),
                TextFace::px(pair.right_size),
                x,
                b,
                &right,
            );
        }
    }
    Ok(())
}

fn wrap_para(face: &TextFace, size: TextSize, para: &str, measure: f32) -> Vec<Shaped> {
    let mut out = Vec::new();
    for hard in para.split('\n') {
        if hard.is_empty() {
            out.push(face.shape("", size, false));
            continue;
        }
        let mut line = String::new();
        for word in hard.split(' ') {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if face.shape(&candidate, size, false).width <= measure || line.is_empty() {
                line = candidate;
            } else {
                out.push(face.shape(&line, size, false));
                line = word.to_string();
            }
        }
        out.push(face.shape(&line, size, false));
    }
    out
}

fn blit(canvas: &mut Canvas, face: &Face, px: f32, x0: f32, baseline: f32, shaped: &Shaped) {
    for g in &shaped.glyphs {
        let (metrics, bitmap) = face.raster_glyph(g.glyph_id, px);
        if metrics.width == 0 || metrics.height == 0 {
            continue;
        }
        let origin_x = (x0 + g.x).round() as i32 + metrics.xmin;
        let origin_y = (baseline - g.y).round() as i32 - metrics.ymin - metrics.height as i32;
        for gy in 0..metrics.height {
            for gx in 0..metrics.width {
                if bitmap[gy * metrics.width + gx] < THRESHOLD {
                    continue;
                }
                canvas.set(origin_x + gx as i32, origin_y + gy as i32);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::face::{TextFace, Weight};
    use crate::frame::{Measure, Pair, Sheet, TextBlock};
    use crate::leading::GridSkip;
    use crate::size::TextSize;
    use tm20::graphics::width_bytes;
    use tm20::PRINTABLE_DOTS;

    fn text() -> TextFace {
        TextFace::sans(Weight::Roman).expect("system sans")
    }

    #[test]
    fn compose_is_tape_wide() {
        let face = text();
        let sheet = Sheet::tape(vec![Frame::Text(TextBlock {
            face: &face,
            size: TextSize::Pt11,
            text: "Hello",
            indent: 0,
        })]);
        let g = compose(&sheet).unwrap();
        assert_eq!(g.width_dots, PRINTABLE_DOTS);
        assert!(g.pixels.iter().any(|&b| b != 0));
    }

    #[test]
    fn pair_has_ink_on_both_sides() {
        let face = text();
        let sheet = Sheet::tape(vec![Frame::Pair(Pair {
            left_face: &face,
            left_size: TextSize::Pt11,
            left: "Coffee",
            right_face: &face,
            right_size: TextSize::Pt11,
            right: "$4.50",
            gutter: GridSkip::ONE,
        })]);
        let g = compose(&sheet).unwrap();
        let stride = width_bytes(g.width_dots);
        let mut left = false;
        let mut right = false;
        for row in 0..g.height_dots as usize {
            left |= g.pixels[row * stride] != 0;
            right |= g.pixels[row * stride + stride - 1] != 0;
        }
        assert!(left && right);
        assert_eq!(g.width_dots, Measure::TAPE.get());
    }

    #[test]
    fn wrap_makes_taller_than_one_line() {
        let face = text();
        let one = compose(&Sheet::tape(vec![Frame::Text(TextBlock {
            face: &face,
            size: TextSize::Pt11,
            text: "Hello",
            indent: 0,
        })]))
        .unwrap();
        let wrapped = compose(&Sheet::tape(vec![Frame::Text(TextBlock {
            face: &face,
            size: TextSize::Pt11,
            text: "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
            indent: 0,
        })]))
        .unwrap();
        assert!(wrapped.height_dots > one.height_dots);
    }
}
