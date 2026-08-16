//! Small evaluator: [`Sheet`] + [`FaceTable`] → packed [`tm20::Graphics`].

use crate::error::Error;
use crate::face::{Cut, DisplayFace, Face, FaceTable, Shaped, TextFace};
use crate::frame::{Frame, Head, List, MarkAlign, Pair, Sheet, Span, TextBlock, EN_DASH};
use crate::leading::HANG;
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

/// What the last painted frame was, for adjacency. Text and List share [`Kind::Run`].
#[derive(Clone, Copy)]
enum Kind {
    Mark { slug: u16 },
    Head,
    Run,
    Pair,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Rhythm {
    Mark,
    Head,
    Run,
    Pair,
    Rule,
}

#[derive(Clone, Copy)]
enum Place {
    Origin {
        floor: f32,
        slug_bottom: f32,
    },
    Line {
        baseline: f32,
        slug_bottom: f32,
        kind: Kind,
    },
    Rule {
        bottom: f32,
    },
}

struct Cursor {
    place: Place,
}

impl Cursor {
    fn new() -> Self {
        Self {
            place: Place::Origin {
                floor: 0.0,
                slug_bottom: 0.0,
            },
        }
    }

    fn slug_bottom(&self) -> f32 {
        match self.place {
            Place::Origin { slug_bottom, .. } | Place::Line { slug_bottom, .. } => slug_bottom,
            Place::Rule { bottom } => bottom,
        }
    }

    fn bump(&mut self, extra: u16) {
        if extra == 0 {
            return;
        }
        let extra = extra as f32;
        self.place = match self.place {
            Place::Line {
                baseline,
                slug_bottom,
                kind,
            } => Place::Line {
                baseline: baseline + extra,
                slug_bottom,
                kind,
            },
            Place::Origin { floor, slug_bottom } => Place::Origin {
                floor: floor + extra,
                slug_bottom,
            },
            Place::Rule { bottom } => Place::Origin {
                floor: bottom + extra,
                slug_bottom: bottom,
            },
        };
    }

    fn first_baseline(&mut self, ascent: f32, skip: u16, kind: Kind) -> f32 {
        let b = match self.place {
            Place::Rule { bottom } => bottom + HANG as f32 + ascent,
            Place::Origin { floor, .. } => floor + ascent,
            Place::Line { baseline, .. } => baseline + skip as f32,
        };
        let slug_bottom = self.slug_bottom().max(b - ascent + skip as f32);
        self.place = Place::Line {
            baseline: b,
            slug_bottom,
            kind,
        };
        b
    }

    fn later_baseline(&mut self, ascent: f32, skip: u16) -> f32 {
        let Place::Line {
            baseline,
            slug_bottom,
            kind,
        } = self.place
        else {
            unreachable!("wrapped lines follow the first");
        };
        let b = baseline + skip as f32;
        let slug_bottom = slug_bottom.max(b - ascent + skip as f32);
        self.place = Place::Line {
            baseline: b,
            slug_bottom,
            kind,
        };
        b
    }

    fn set_rule(&mut self, bottom: f32) {
        self.place = Place::Rule { bottom };
    }
}

fn rhythm(frame: &Frame<'_>) -> Rhythm {
    match frame {
        Frame::Mark(_) => Rhythm::Mark,
        Frame::Head(_) => Rhythm::Head,
        Frame::Text(_) | Frame::List(_) => Rhythm::Run,
        Frame::Pair(_) => Rhythm::Pair,
        Frame::Rule(_) => Rhythm::Rule,
    }
}

fn slug(frame: &Frame<'_>) -> u16 {
    match frame {
        Frame::Text(b) => b.size.skip_dots(),
        Frame::Head(h) => h.size.skip_dots(),
        Frame::List(l) => l.size.skip_dots(),
        Frame::Pair(p) => p.size.skip_dots(),
        Frame::Mark(m) => m.size.skip_dots(),
        Frame::Rule(_) => 0,
    }
}

fn extra(place: Place, to: Rhythm, next: u16) -> u16 {
    match place {
        Place::Origin { .. } => 0,
        Place::Rule { .. } => match to {
            Rhythm::Pair => 0,
            Rhythm::Mark | Rhythm::Head | Rhythm::Run | Rhythm::Rule => next,
        },
        Place::Line { kind, .. } => match (kind, to) {
            (Kind::Mark { .. }, Rhythm::Mark | Rhythm::Rule) => 0,
            (Kind::Mark { slug }, _) => slug,
            (Kind::Head, Rhythm::Run | Rhythm::Pair | Rhythm::Rule) => 0,
            (Kind::Pair, Rhythm::Pair) => 0,
            (_, Rhythm::Head) => next,
            (Kind::Run | Kind::Head, Rhythm::Run | Rhythm::Pair | Rhythm::Mark) => next,
            (Kind::Pair, Rhythm::Run | Rhythm::Mark) => next,
            (Kind::Run | Kind::Pair, Rhythm::Rule) => 0,
        },
    }
}

/// Layout `sheet` onto one 1-bit canvas and pack it.
pub fn compose(sheet: &Sheet<'_>, faces: &FaceTable) -> Result<Graphics, Error> {
    let width = sheet.width.get();
    let mut canvas = Canvas::new(width);
    let mut cur = Cursor::new();

    for frame in &sheet.frames {
        cur.bump(extra(cur.place, rhythm(frame), slug(frame)));
        match frame {
            Frame::Rule(rule) => {
                let y = canvas.height.max(cur.slug_bottom().ceil() as u16);
                for dy in 0..rule.thickness.dots() {
                    canvas.fill_row(y + dy, 0, width);
                }
                cur.set_rule((y + rule.thickness.dots()) as f32);
            }
            Frame::Mark(mark) => {
                let face = faces.display(mark.cut)?;
                let shaped = face.shape(mark.text, mark.size, mark.tracking.0);
                let skip = mark.size.skip_dots();
                let ascent = shaped_ink_ascent(face.inner(), DisplayFace::px(mark.size), &shaped);
                let b = cur.first_baseline(ascent, skip, Kind::Mark { slug: skip });
                let x0 = match mark.align {
                    MarkAlign::Start => 0.0,
                    MarkAlign::Center => ((width as f32 - shaped.width) * 0.5).max(0.0),
                };
                blit(
                    &mut canvas,
                    face.inner(),
                    DisplayFace::px(mark.size),
                    x0,
                    b,
                    &shaped,
                );
            }
            Frame::Text(block) => {
                paint_run(&mut canvas, &mut cur, width, block, faces, Kind::Run)?;
            }
            Frame::Head(head) => {
                paint_head(&mut canvas, &mut cur, width, head, faces)?;
            }
            Frame::Pair(pair) => {
                paint_pair(&mut canvas, &mut cur, width, pair, faces)?;
            }
            Frame::List(list) => {
                paint_list(&mut canvas, &mut cur, width, list, faces)?;
            }
        }
    }

    canvas.into_graphics()
}

fn paint_head(
    canvas: &mut Canvas,
    cur: &mut Cursor,
    width: u16,
    head: &Head<'_>,
    faces: &FaceTable,
) -> Result<(), Error> {
    let block = TextBlock::plain(Cut::Bold, head.size, head.text);
    paint_run(canvas, cur, width, &block, faces, Kind::Head)
}

fn paint_run(
    canvas: &mut Canvas,
    cur: &mut Cursor,
    width: u16,
    block: &TextBlock<'_>,
    faces: &FaceTable,
    kind: Kind,
) -> Result<(), Error> {
    let measure = width.max(1);
    let skip = block.size.skip_dots();
    let lines = wrap_spans(block.size, &block.spans, measure as f32, faces)?;
    for (li, line) in lines.iter().enumerate() {
        let ascent = line_ascent(block.size, line);
        let b = if li == 0 {
            cur.first_baseline(ascent, skip, kind)
        } else {
            cur.later_baseline(ascent, skip)
        };
        paint_line(canvas, block.size, 0.0, b, line);
    }
    Ok(())
}

fn paint_pair(
    canvas: &mut Canvas,
    cur: &mut Cursor,
    width: u16,
    pair: &Pair<'_>,
    faces: &FaceTable,
) -> Result<(), Error> {
    let skip = pair.size.skip_dots();
    let fig = faces.text(pair.figure)?;
    let shaped = fig.shape_figure(pair.amount, pair.size);
    let fig_w = shaped.width.ceil().max(1.0) as u16;
    let left_w = width
        .saturating_sub(pair.gutter.dots())
        .saturating_sub(fig_w)
        .max(1);
    let lines = wrap_spans(pair.size, &pair.left, left_w as f32, faces)?;
    let fig_ascent = shaped_ink_ascent(fig.inner(), TextFace::px(pair.size), &shaped);
    let first_ascent = lines
        .first()
        .map(|l| line_ascent(pair.size, l))
        .unwrap_or(0.0)
        .max(fig_ascent);
    let n = lines.len().max(1);
    for li in 0..n {
        let ascent = if li == 0 {
            first_ascent
        } else {
            line_ascent(pair.size, &lines[li])
        };
        let b = if li == 0 {
            cur.first_baseline(ascent, skip, Kind::Pair)
        } else {
            cur.later_baseline(ascent, skip)
        };
        if let Some(line) = lines.get(li) {
            paint_line(canvas, pair.size, 0.0, b, line);
        }
        if li == 0 {
            let x = (width as f32 - shaped.width).max(0.0);
            blit(canvas, fig.inner(), TextFace::px(pair.size), x, b, &shaped);
        }
    }
    Ok(())
}

fn paint_list(
    canvas: &mut Canvas,
    cur: &mut Cursor,
    width: u16,
    list: &List<'_>,
    faces: &FaceTable,
) -> Result<(), Error> {
    let hang = list.hang_dots(faces)?;
    let measure = (width.saturating_sub(hang)).max(1);
    let skip = list.size.skip_dots();
    let face = faces.text(list.cut)?;
    let dash = face.shape(EN_DASH, list.size);
    let dash_ascent = shaped_ink_ascent(face.inner(), TextFace::px(list.size), &dash);
    for (ii, item) in list.items.iter().enumerate() {
        let lines = wrap_spans(list.size, item, measure as f32, faces)?;
        for (li, line) in lines.iter().enumerate() {
            let ascent = line_ascent(list.size, line).max(dash_ascent);
            let b = if ii == 0 && li == 0 {
                cur.first_baseline(ascent, skip, Kind::Run)
            } else {
                cur.later_baseline(ascent, skip)
            };
            if li == 0 {
                blit(canvas, face.inner(), TextFace::px(list.size), 0.0, b, &dash);
            }
            paint_line(canvas, list.size, hang as f32, b, line);
        }
    }
    Ok(())
}

#[derive(Clone)]
struct LineSpan<'a> {
    face: &'a TextFace,
    text: String,
}

fn paint_line(canvas: &mut Canvas, size: TextSize, x0: f32, baseline: f32, line: &[LineSpan<'_>]) {
    let mut x = x0;
    for (i, piece) in line.iter().enumerate() {
        if i > 0 {
            x += piece.face.shape(" ", size).width;
        }
        let shaped = piece.face.shape(&piece.text, size);
        blit(
            canvas,
            piece.face.inner(),
            TextFace::px(size),
            x,
            baseline,
            &shaped,
        );
        x += shaped.width;
    }
}

fn line_ascent(size: TextSize, line: &[LineSpan<'_>]) -> f32 {
    line.iter()
        .map(|p| {
            let shaped = p.face.shape(&p.text, size);
            shaped_ink_ascent(p.face.inner(), TextFace::px(size), &shaped)
        })
        .fold(0.0f32, f32::max)
}

fn shaped_ink_ascent(face: &Face, px: f32, shaped: &Shaped) -> f32 {
    let mut a = 0.0f32;
    for g in &shaped.glyphs {
        let (m, _) = face.raster_glyph(g.glyph_id, px);
        if m.height == 0 {
            continue;
        }
        a = a.max(g.y + m.ymin as f32 + m.height as f32);
    }
    if a > 0.0 {
        a
    } else {
        shaped.ascent
    }
}

fn wrap_spans<'f>(
    size: TextSize,
    spans: &[Span<'_>],
    measure: f32,
    faces: &'f FaceTable,
) -> Result<Vec<Vec<LineSpan<'f>>>, Error> {
    let mut out = Vec::new();
    let mut line: Vec<LineSpan<'f>> = Vec::new();
    for span in spans {
        let face = faces.text(span.cut)?;
        for (hi, hard) in span.text.split('\n').enumerate() {
            if hi > 0 && !line.is_empty() {
                out.push(std::mem::take(&mut line));
            }
            if hard.is_empty() {
                if line.is_empty() {
                    out.push(vec![LineSpan {
                        face,
                        text: String::new(),
                    }]);
                }
                continue;
            }
            for word in hard.split(' ') {
                if word.is_empty() {
                    continue;
                }
                let mut trial = line.clone();
                push_word(&mut trial, face, word);
                if line_width(size, &trial) <= measure || line.is_empty() {
                    line = trial;
                } else {
                    out.push(std::mem::take(&mut line));
                    push_word(&mut line, face, word);
                }
            }
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
    if out.is_empty() {
        out.push(Vec::new());
    }
    Ok(out)
}

fn push_word<'a>(line: &mut Vec<LineSpan<'a>>, face: &'a TextFace, word: &str) {
    match line.last_mut() {
        Some(prev) if std::ptr::eq(prev.face, face) && !prev.text.is_empty() => {
            prev.text.push(' ');
            prev.text.push_str(word);
        }
        _ => line.push(LineSpan {
            face,
            text: word.to_string(),
        }),
    }
}

fn line_width(size: TextSize, line: &[LineSpan<'_>]) -> f32 {
    let mut w = 0.0;
    for (i, piece) in line.iter().enumerate() {
        if i > 0 {
            w += piece.face.shape(" ", size).width;
        }
        w += piece.face.shape(&piece.text, size).width;
    }
    w
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
