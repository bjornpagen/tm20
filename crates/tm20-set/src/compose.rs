//! Small evaluator: [`Sheet`] + [`FaceTable`] → packed [`tm20::Graphics`].

use std::num::NonZeroU32;

use crate::error::Error;
use crate::face::{Cut, DisplayFace, Face, FaceTable, Shaped, TextFace};
use crate::frame::{
    Code, ColAlign, ColBody, Cols, EN_DASH, Figure, Frame, Head, ItemMark, List, ListFit, ListItem,
    MarkAlign, Marker, Math, Note, Quote, Rule, Sheet, TextBlock, Thickness, decimal_text,
};
use crate::leading::{GRID, HANG, NOTE_RULE, TASK_BOX, pt_dots};
use crate::size::TextSize;
use tm20::graphics::{Graphics, GraphicsScale, pack};

const THRESHOLD: u8 = 96;
const NEST_CAP: u8 = 3;
const NOTE_RAISE: f32 = 0.4;

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

/// Last completed frame’s adjacency class. `Place` is only geometry.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Rhythm {
    Mark,
    Head,
    Prose,
    Hang,
    Cols,
    Rule,
}

#[derive(Clone, Copy)]
enum Place {
    Origin { floor: f32, slug_bottom: f32 },
    Line { baseline: f32, slug_bottom: f32 },
    Rule { bottom: f32 },
}

struct Cursor {
    place: Place,
    last: Option<Rhythm>,
    mark_slug: u16,
}

impl Cursor {
    fn new() -> Self {
        Self {
            place: Place::Origin {
                floor: 0.0,
                slug_bottom: 0.0,
            },
            last: None,
            mark_slug: 0,
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
            } => Place::Line {
                baseline: baseline + extra,
                slug_bottom,
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

    fn first_baseline(&mut self, ascent: f32, depth: f32, skip: u16) -> f32 {
        let b = match self.place {
            Place::Rule { bottom } => bottom + HANG as f32 + ascent,
            Place::Origin { floor, .. } => floor + ascent,
            Place::Line {
                baseline,
                slug_bottom,
            } => (baseline + skip as f32).max(slug_bottom + ascent),
        };
        let slug_bottom = self
            .slug_bottom()
            .max(b - ascent + skip as f32)
            .max(b + depth);
        self.place = Place::Line {
            baseline: b,
            slug_bottom,
        };
        b
    }

    fn later_baseline(&mut self, ascent: f32, depth: f32, skip: u16) -> f32 {
        let Place::Line {
            baseline,
            slug_bottom,
        } = self.place
        else {
            unreachable!("wrapped lines follow the first");
        };
        let b = (baseline + skip as f32).max(slug_bottom + ascent);
        let slug_bottom = slug_bottom.max(b - ascent + skip as f32).max(b + depth);
        self.place = Place::Line {
            baseline: b,
            slug_bottom,
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
        Frame::Text(_) | Frame::Figure(_) | Frame::Math(_) => Rhythm::Prose,
        Frame::List(_) | Frame::Quote(_) | Frame::Code(_) => Rhythm::Hang,
        Frame::Cols(_) => Rhythm::Cols,
        Frame::Rule(_) => Rhythm::Rule,
    }
}

fn slug(frame: &Frame<'_>) -> u16 {
    match frame {
        Frame::Text(b) => b.size.skip_dots(),
        Frame::Head(h) => h.size.skip_dots(),
        Frame::List(l) => l.size.skip_dots(),
        Frame::Cols(c) => c.size.skip_dots(),
        Frame::Quote(q) => q.frames.first().map_or(0, slug),
        Frame::Code(c) => c.size.skip_dots(),
        Frame::Mark(m) => m.size.skip_dots(),
        Frame::Figure(_) => GRID,
        Frame::Math(_) => TextSize::Pt11.skip_dots(),
        Frame::Rule(_) => 0,
    }
}

fn extra(cur: &Cursor, to: Rhythm, next: u16) -> u16 {
    match cur.place {
        Place::Origin { .. } => 0,
        Place::Rule { .. } => match to {
            Rhythm::Cols => 0,
            Rhythm::Mark | Rhythm::Head | Rhythm::Prose | Rhythm::Hang | Rhythm::Rule => next,
        },
        Place::Line { .. } => {
            let Some(from) = cur.last else {
                return 0;
            };
            match (from, to) {
                (Rhythm::Mark, Rhythm::Mark | Rhythm::Rule) => 0,
                (Rhythm::Mark, _) => cur.mark_slug,
                (Rhythm::Head, Rhythm::Prose | Rhythm::Hang | Rhythm::Cols | Rhythm::Rule) => 0,
                (Rhythm::Head, Rhythm::Head | Rhythm::Mark) => next,
                (Rhythm::Cols, Rhythm::Cols | Rhythm::Rule) => 0,
                (Rhythm::Cols, Rhythm::Mark | Rhythm::Head | Rhythm::Prose | Rhythm::Hang) => next,
                (_, Rhythm::Head) => next,
                (Rhythm::Prose, Rhythm::Prose) => next,
                (Rhythm::Prose | Rhythm::Hang, Rhythm::Hang) => 0,
                (Rhythm::Hang, Rhythm::Prose) => 0,
                (Rhythm::Prose | Rhythm::Hang, Rhythm::Rule) => 0,
                (Rhythm::Prose | Rhythm::Hang, Rhythm::Cols | Rhythm::Mark) => next,
                (Rhythm::Rule, Rhythm::Cols) => 0,
                (Rhythm::Rule, _) => next,
            }
        }
    }
}

struct MarkInk<'a> {
    x: f32,
    face: &'a TextFace,
    shaped: Shaped,
    px: f32,
}

enum Pending<'a> {
    Glyph(MarkInk<'a>),
    Task { x: u16, checked: bool, ascent: f32 },
}

struct Cx<'a, 'f> {
    canvas: &'a mut Canvas,
    cur: &'a mut Cursor,
    faces: &'f FaceTable,
    pending: Vec<Pending<'f>>,
}

/// Layout `sheet` onto one 1-bit canvas and pack it.
pub fn compose(sheet: &Sheet<'_>, faces: &FaceTable) -> Result<Graphics, Error> {
    let width = sheet.width.get();
    let mut canvas = Canvas::new(width);
    let mut cur = Cursor::new();
    let mut cx = Cx {
        canvas: &mut canvas,
        cur: &mut cur,
        faces,
        pending: Vec::new(),
    };
    paint_seq(&mut cx, &sheet.frames, 0, width, 0, 0)?;
    if !sheet.notes.is_empty() {
        paint_notes(&mut cx, width, &sheet.notes)?;
    }
    canvas.into_graphics()
}

fn paint_seq(
    cx: &mut Cx<'_, '_>,
    frames: &[Frame<'_>],
    x0: u16,
    measure: u16,
    quote_depth: u8,
    list_depth: u8,
) -> Result<(), Error> {
    if frames.is_empty() {
        let b = cx.cur.first_baseline(0.0, 0.0, GRID);
        flush_marks(cx, b);
        return Ok(());
    }
    for frame in frames {
        let e = extra(cx.cur, rhythm(frame), slug(frame));
        cx.cur.bump(e);
        paint_one(cx, frame, x0, measure, quote_depth, list_depth)?;
        cx.cur.last = Some(rhythm(frame));
    }
    Ok(())
}

fn paint_one(
    cx: &mut Cx<'_, '_>,
    frame: &Frame<'_>,
    x0: u16,
    measure: u16,
    quote_depth: u8,
    list_depth: u8,
) -> Result<(), Error> {
    match frame {
        Frame::Rule(rule) => paint_rule(cx, rule, x0, measure),
        Frame::Mark(mark) => paint_mark(cx, mark, x0, measure),
        Frame::Text(block) => paint_run(cx, block, x0, measure),
        Frame::Head(head) => paint_head(cx, head, x0, measure),
        Frame::Cols(cols) => paint_cols(cx, cols, x0, measure),
        Frame::List(list) => paint_list(cx, list, x0, measure, quote_depth, list_depth),
        Frame::Quote(quote) => paint_quote(cx, quote, x0, measure, quote_depth, list_depth),
        Frame::Code(code) => paint_code(cx, code, x0, measure),
        Frame::Figure(fig) => paint_figure(cx, fig, x0, measure),
        Frame::Math(math) => paint_math(cx, math, x0, measure),
    }
}

fn paint_rule(cx: &mut Cx<'_, '_>, rule: &Rule, x0: u16, measure: u16) -> Result<(), Error> {
    let y = cx.canvas.height.max(cx.cur.slug_bottom().ceil() as u16);
    flush_marks(cx, y as f32);
    let x1 = x0.saturating_add(measure);
    for dy in 0..rule.thickness.dots() {
        cx.canvas.fill_row(y + dy, x0, x1);
    }
    cx.cur.set_rule((y + rule.thickness.dots()) as f32);
    Ok(())
}

fn paint_mark(
    cx: &mut Cx<'_, '_>,
    mark: &crate::frame::Mark<'_>,
    x0: u16,
    measure: u16,
) -> Result<(), Error> {
    let faces = cx.faces;
    let face = faces.display(mark.cut)?;
    let shaped = face.shape(mark.text.as_ref(), mark.size, mark.tracking.0);
    let skip = mark.size.skip_dots();
    let ascent = shaped_ink_ascent(face.inner(), DisplayFace::px(mark.size), &shaped);
    cx.cur.mark_slug = skip;
    let b = cx.cur.first_baseline(ascent, 0.0, skip);
    flush_marks(cx, b);
    let x = match mark.align {
        MarkAlign::Start => x0 as f32,
        MarkAlign::Center => x0 as f32 + ((measure as f32 - shaped.width) * 0.5).max(0.0),
    };
    blit(
        cx.canvas,
        face.inner(),
        DisplayFace::px(mark.size),
        x,
        b,
        &shaped,
    );
    Ok(())
}

fn paint_head(cx: &mut Cx<'_, '_>, head: &Head<'_>, x0: u16, measure: u16) -> Result<(), Error> {
    let block = TextBlock::plain(Cut::Bold, head.size, head.text.as_ref());
    paint_run(cx, &block, x0, measure)
}

fn paint_run(
    cx: &mut Cx<'_, '_>,
    block: &TextBlock<'_>,
    x0: u16,
    measure: u16,
) -> Result<(), Error> {
    let measure = measure.max(1);
    let skip = block.size.skip_dots();
    let lines = wrap_spans(
        block.size,
        &block.spans,
        measure as f32,
        cx.faces,
        Digits::Proportional,
    )?;
    for (li, line) in lines.iter().enumerate() {
        let (ascent, depth) = line_metrics(block.size, line);
        let b = if li == 0 {
            cx.cur.first_baseline(ascent, depth, skip)
        } else {
            cx.cur.later_baseline(ascent, depth, skip)
        };
        if li == 0 {
            flush_marks(cx, b);
        }
        paint_line(cx, block.size, x0 as f32, b, line)?;
    }
    Ok(())
}

fn paint_quote(
    cx: &mut Cx<'_, '_>,
    quote: &Quote<'_>,
    x0: u16,
    measure: u16,
    quote_depth: u8,
    list_depth: u8,
) -> Result<(), Error> {
    if quote_depth >= NEST_CAP {
        return Err(Error::Nesting);
    }
    cx.cur.last = Some(Rhythm::Hang);
    let x = x0.saturating_add(GRID);
    let w = measure.saturating_sub(GRID).max(1);
    paint_seq(cx, &quote.frames, x, w, quote_depth + 1, list_depth)
}

fn paint_code(cx: &mut Cx<'_, '_>, code: &Code<'_>, x0: u16, _measure: u16) -> Result<(), Error> {
    let x = x0.saturating_add(GRID) as f32;
    let faces = cx.faces;
    let face = faces.text(Cut::Mono)?;
    let skip = code.size.skip_dots();
    let px = TextFace::px(code.size);
    let empty = std::borrow::Cow::Borrowed("");
    let lines: Vec<&std::borrow::Cow<'_, str>> = if code.lines.is_empty() {
        vec![&empty]
    } else {
        code.lines.iter().collect()
    };
    for (li, line) in lines.iter().enumerate() {
        let shaped = face.shape(line.as_ref(), code.size);
        let ascent = shaped_ink_ascent(face.inner(), px, &shaped);
        let b = if li == 0 {
            cx.cur.first_baseline(ascent, 0.0, skip)
        } else {
            cx.cur.later_baseline(ascent, 0.0, skip)
        };
        if li == 0 {
            flush_marks(cx, b);
        }
        blit(cx.canvas, face.inner(), px, x, b, &shaped);
    }
    Ok(())
}

fn paint_figure(cx: &mut Cx<'_, '_>, fig: &Figure, x0: u16, _measure: u16) -> Result<(), Error> {
    let b = cx.cur.first_baseline(0.0, 0.0, GRID);
    flush_marks(cx, b);
    let top = b.round().max(0.0) as u16;
    blit_bits(
        cx.canvas, x0 as i32, top as i32, fig.width, fig.height, &fig.bits,
    );
    if let Some(n) = fig.note {
        let nf = cx.faces.text(Cut::Roman)?;
        let label = n.get().to_string();
        let shaped_note = nf.shape(&label, TextSize::Pt8);
        let px8 = TextFace::px(TextSize::Pt8);
        let raise = px8 * NOTE_RAISE;
        let mut x = x0 as f32 + fig.width as f32;
        if x + shaped_note.width > cx.canvas.width as f32 {
            x = (cx.canvas.width as f32 - shaped_note.width).max(x0 as f32);
        }
        blit(
            cx.canvas,
            nf.inner(),
            px8,
            x,
            top as f32 + px8 - raise,
            &shaped_note,
        );
    }
    let bottom = top as f32 + fig.height as f32;
    cx.cur.place = Place::Line {
        baseline: bottom,
        slug_bottom: bottom + GRID as f32,
    };
    Ok(())
}

fn paint_math(cx: &mut Cx<'_, '_>, math: &Math, x0: u16, measure: u16) -> Result<(), Error> {
    let b = cx.cur.first_baseline(0.0, 0.0, GRID);
    flush_marks(cx, b);
    let top = b.round().max(0.0) as u16;
    let w = math.width.min(measure);
    let x = x0.saturating_add(measure.saturating_sub(w) / 2);
    blit_bits(
        cx.canvas,
        x as i32,
        top as i32,
        math.width,
        math.height,
        &math.bits,
    );
    let bottom = top as f32 + math.height as f32;
    cx.cur.place = Place::Line {
        baseline: bottom,
        slug_bottom: bottom + GRID as f32,
    };
    Ok(())
}

fn blit_bits(canvas: &mut Canvas, x0: i32, y0: i32, width: u16, height: u16, bits: &[bool]) {
    for y in 0..height {
        for x in 0..width {
            if bits[y as usize * width as usize + x as usize] {
                canvas.set(x0 + x as i32, y0 + y as i32);
            }
        }
    }
}

fn paint_notes(cx: &mut Cx<'_, '_>, width: u16, notes: &[Note<'_>]) -> Result<(), Error> {
    let air = pt_dots(2.0);
    let y = cx
        .canvas
        .height
        .max((cx.cur.slug_bottom() + air as f32).ceil() as u16);
    flush_marks(cx, y as f32);
    let x1 = NOTE_RULE.min(width);
    cx.canvas.fill_row(y, 0, x1);
    cx.cur.set_rule((y + Thickness::One.dots()) as f32);
    cx.cur.bump(air);
    let size = TextSize::Pt8;
    let hang_list = List {
        size,
        cut: Cut::Roman,
        marker: Marker::Decimal {
            start: 1,
            delim: crate::frame::DecimalDelim::Period,
        },
        fit: ListFit::Tight,
        items: notes.iter().map(|_| ListItem::new(vec![])).collect(),
    };
    let hang = hang_list.hang_dots(cx.faces)?;
    let mark_w = hang_list.mark_width(cx.faces)?;
    let content_x = hang;
    let content_w = width.saturating_sub(hang).max(1);
    let face = cx.faces.text(Cut::Roman)?;
    let px = TextFace::px(size);
    for (i, note) in notes.iter().enumerate() {
        let t = decimal_text(1 + i as u32, crate::frame::DecimalDelim::Period);
        let s = face.shape_figure(&t, size);
        let mx = (mark_w - s.width).max(0.0);
        cx.pending.push(Pending::Glyph(MarkInk {
            x: mx,
            face,
            shaped: s,
            px,
        }));
        cx.cur.last = Some(Rhythm::Hang);
        match note {
            Note::Dest { dest, title } => {
                let body = match title {
                    Some(t) => format!("{t}\n{dest}"),
                    None => dest.as_ref().to_string(),
                };
                let frame = Frame::Text(TextBlock::plain(Cut::Roman, size, body));
                paint_seq(cx, &[frame], content_x, content_w, 0, 1)?;
            }
            Note::Blocks(frames) => {
                paint_seq(cx, frames, content_x, content_w, 0, 1)?;
            }
        }
    }
    Ok(())
}

fn paint_list(
    cx: &mut Cx<'_, '_>,
    list: &List<'_>,
    x0: u16,
    measure: u16,
    quote_depth: u8,
    list_depth: u8,
) -> Result<(), Error> {
    if list_depth >= NEST_CAP {
        return Err(Error::Nesting);
    }
    let faces = cx.faces;
    let hang = list.hang_dots(faces)?;
    let mark_w = list.mark_width(faces)?;
    let content_x = x0.saturating_add(hang);
    let content_w = measure.saturating_sub(hang).max(1);
    let face = faces.text(list.cut)?;
    let px = TextFace::px(list.size);
    let cap = face.shape("H", list.size);
    let ascent = shaped_ink_ascent(face.inner(), px, &cap);
    for (i, item) in list.items.iter().enumerate() {
        match item.mark {
            ItemMark::Task { checked } => {
                cx.pending.push(Pending::Task {
                    x: x0,
                    checked,
                    ascent,
                });
            }
            ItemMark::List => {
                let (mx, shaped) = match list.marker {
                    Marker::Dash => (x0 as f32, face.shape(EN_DASH, list.size)),
                    Marker::Decimal { start, delim } => {
                        let t = decimal_text(start.saturating_add(i as u32), delim);
                        let s = face.shape_figure(&t, list.size);
                        let x = (x0 as f32 + mark_w - s.width).max(x0 as f32);
                        (x, s)
                    }
                };
                cx.pending.push(Pending::Glyph(MarkInk {
                    x: mx,
                    face,
                    shaped,
                    px,
                }));
            }
        }
        if i > 0 && list.fit == ListFit::Loose {
            cx.cur.bump(list.size.skip_dots());
        }
        cx.cur.last = Some(Rhythm::Hang);
        paint_seq(
            cx,
            &item.frames,
            content_x,
            content_w,
            quote_depth,
            list_depth + 1,
        )?;
    }
    Ok(())
}

fn paint_cols(cx: &mut Cx<'_, '_>, cols: &Cols<'_>, x0: u16, measure: u16) -> Result<(), Error> {
    match &cols.body {
        ColBody::Two { align, rows } => {
            paint_grid(cx, cols.size, cols.gutter, align, rows, x0, measure)
        }
        ColBody::Three { align, rows } => {
            paint_grid(cx, cols.size, cols.gutter, align, rows, x0, measure)
        }
    }
}

fn paint_grid<const N: usize>(
    cx: &mut Cx<'_, '_>,
    size: TextSize,
    gutter: crate::leading::GridSkip,
    align: &[ColAlign; N],
    rows: &[[Vec<crate::frame::Span<'_>>; N]],
    x0: u16,
    measure: u16,
) -> Result<(), Error> {
    if rows.is_empty() {
        let b = cx.cur.first_baseline(0.0, 0.0, size.skip_dots());
        flush_marks(cx, b);
        return Ok(());
    }
    let gutter = gutter.dots();
    let gutters = gutter * (N as u16 - 1);
    let inner = measure.saturating_sub(gutters).max(1);
    let widths = col_widths(size, align, rows, inner, cx.faces)?;
    let skip = size.skip_dots();
    for (ri, row) in rows.iter().enumerate() {
        let mut cell_lines = Vec::with_capacity(N);
        for (c, cell) in row.iter().enumerate() {
            let digits = match align[c] {
                ColAlign::End => Digits::Tabular,
                ColAlign::Start => Digits::Proportional,
            };
            cell_lines.push(wrap_spans(size, cell, widths[c] as f32, cx.faces, digits)?);
        }
        let nlines = cell_lines.iter().map(|l| l.len().max(1)).max().unwrap_or(1);
        for li in 0..nlines {
            let mut ascent = 0.0f32;
            let mut depth = 0.0f32;
            for lines in &cell_lines {
                if let Some(line) = lines.get(li) {
                    let (a, d) = line_metrics(size, line);
                    ascent = ascent.max(a);
                    depth = depth.max(d);
                }
            }
            let b = if ri == 0 && li == 0 {
                cx.cur.first_baseline(ascent, depth, skip)
            } else {
                cx.cur.later_baseline(ascent, depth, skip)
            };
            if ri == 0 && li == 0 {
                flush_marks(cx, b);
            }
            let mut x = x0 as f32;
            for c in 0..N {
                if let Some(line) = cell_lines[c].get(li) {
                    let notes = note_face_of(cx.faces, line)?;
                    let lw = line_width(size, line, notes);
                    let lx = if align[c] == ColAlign::End {
                        x + (widths[c] as f32 - lw).max(0.0)
                    } else {
                        x
                    };
                    paint_line(cx, size, lx, b, line)?;
                }
                x += widths[c] as f32 + gutter as f32;
            }
        }
    }
    Ok(())
}

fn col_widths<const N: usize>(
    size: TextSize,
    align: &[ColAlign; N],
    rows: &[[Vec<crate::frame::Span<'_>>; N]],
    inner: u16,
    faces: &FaceTable,
) -> Result<[u16; N], Error> {
    let mut widths = [0u16; N];
    let mut end_sum = 0u16;
    let mut start_n = 0u16;
    for (c, col) in align.iter().enumerate() {
        if *col == ColAlign::Start {
            start_n += 1;
            continue;
        }
        let mut w = 1u16;
        for row in rows {
            let lines = wrap_spans(size, &row[c], inner as f32, faces, Digits::Tabular)?;
            let mut lw = 0.0f32;
            for line in &lines {
                let notes = note_face_of(faces, line)?;
                lw = lw.max(line_width(size, line, notes));
            }
            w = w.max(lw.ceil().max(1.0) as u16);
        }
        w = w.min(inner);
        widths[c] = w;
        end_sum = end_sum.saturating_add(w);
    }
    if end_sum > inner {
        let mut used = 0u16;
        for (c, col) in align.iter().enumerate() {
            if *col == ColAlign::End {
                widths[c] = ((widths[c] as u32 * inner as u32) / end_sum.max(1) as u32) as u16;
                widths[c] = widths[c].max(1);
                used = used.saturating_add(widths[c]);
            }
        }
        end_sum = used;
    }
    let leftover = inner.saturating_sub(end_sum);
    if start_n == 0 {
        return Ok(widths);
    }
    let each = (leftover / start_n).max(1);
    let mut rem = leftover.saturating_sub(each * start_n);
    for (c, col) in align.iter().enumerate() {
        if *col == ColAlign::Start {
            widths[c] = each + u16::from(rem > 0);
            rem = rem.saturating_sub(1);
        }
    }
    Ok(widths)
}

fn note_face_of<'f>(
    faces: &'f FaceTable,
    line: &[LineSpan<'_>],
) -> Result<Option<&'f TextFace>, Error> {
    if line
        .iter()
        .any(|p| matches!(p, LineSpan::Type { note: Some(_), .. }))
    {
        Ok(Some(faces.text(Cut::Roman)?))
    } else {
        Ok(None)
    }
}

fn flush_marks(cx: &mut Cx<'_, '_>, baseline: f32) {
    let pending = std::mem::take(&mut cx.pending);
    for m in pending {
        match m {
            Pending::Glyph(m) => {
                blit(cx.canvas, m.face.inner(), m.px, m.x, baseline, &m.shaped);
            }
            Pending::Task { x, checked, ascent } => {
                draw_task(cx.canvas, x, baseline, ascent, checked);
            }
        }
    }
}

fn draw_task(canvas: &mut Canvas, x0: u16, baseline: f32, ascent: f32, checked: bool) {
    let side = TASK_BOX as i32;
    let stroke = 2i32;
    let x0 = x0 as i32;
    let center = (baseline - ascent * 0.5).round() as i32;
    let y0 = center - side / 2;
    let y1 = y0 + side;
    let x1 = x0 + side;
    for t in 0..stroke {
        for x in x0..x1 {
            canvas.set(x, y0 + t);
            canvas.set(x, y1 - 1 - t);
        }
        for y in y0..y1 {
            canvas.set(x0 + t, y);
            canvas.set(x1 - 1 - t, y);
        }
    }
    if checked {
        let inset = stroke * 2;
        stroke_line(
            canvas,
            x0 + inset,
            y0 + side / 2,
            x0 + side / 2,
            y1 - inset,
            stroke,
        );
        stroke_line(
            canvas,
            x0 + side / 2,
            y1 - inset,
            x1 - inset,
            y0 + inset,
            stroke,
        );
    }
}

fn stroke_line(canvas: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, thick: i32) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        for ox in -thick / 2..thick - thick / 2 {
            for oy in -thick / 2..thick - thick / 2 {
                canvas.set(x + ox, y + oy);
            }
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Digits {
    Proportional,
    Tabular,
}

#[derive(Clone)]
enum LineSpan<'a> {
    Type {
        face: &'a TextFace,
        text: String,
        note: Option<NonZeroU32>,
        digits: Digits,
    },
    Math(&'a Math),
}

enum Word<'a> {
    Type {
        face: &'a TextFace,
        text: String,
        note: Option<NonZeroU32>,
        digits: Digits,
    },
    Math(&'a Math),
}

impl<'a> LineSpan<'a> {
    fn face(&self) -> Option<&'a TextFace> {
        match self {
            LineSpan::Type { face, .. } => Some(*face),
            LineSpan::Math(_) => None,
        }
    }
}

fn paint_line(
    cx: &mut Cx<'_, '_>,
    size: TextSize,
    x0: f32,
    baseline: f32,
    line: &[LineSpan<'_>],
) -> Result<(), Error> {
    let mut x = x0;
    let note_face = note_face_of(cx.faces, line)?;
    for (i, piece) in line.iter().enumerate() {
        if i > 0
            && let Some(face) = line.iter().find_map(LineSpan::face)
        {
            x += face.shape(" ", size).width;
        }
        match piece {
            LineSpan::Math(math) => {
                let top = (baseline - math.ascent as f32).round() as i32;
                blit_bits(
                    cx.canvas,
                    x.round() as i32,
                    top,
                    math.width,
                    math.height,
                    &math.bits,
                );
                x += math.width as f32;
            }
            LineSpan::Type {
                face,
                text,
                note,
                digits,
            } => {
                let shaped = shape_text(size, face, text, *digits);
                blit(
                    cx.canvas,
                    face.inner(),
                    TextFace::px(size),
                    x,
                    baseline,
                    &shaped,
                );
                x += shaped.width;
                if let Some(n) = note {
                    let nf = note_face.expect("note_face_of checked");
                    let label = n.get().to_string();
                    let shaped_note = nf.shape(&label, TextSize::Pt8);
                    let raise = TextFace::px(size) * NOTE_RAISE;
                    blit(
                        cx.canvas,
                        nf.inner(),
                        TextFace::px(TextSize::Pt8),
                        x,
                        baseline - raise,
                        &shaped_note,
                    );
                    x += shaped_note.width;
                }
            }
        }
    }
    Ok(())
}

fn shape_text(size: TextSize, face: &TextFace, text: &str, digits: Digits) -> Shaped {
    match digits {
        Digits::Tabular => face.shape_figure(text, size),
        Digits::Proportional => face.shape(text, size),
    }
}

fn line_metrics(size: TextSize, line: &[LineSpan<'_>]) -> (f32, f32) {
    let mut ascent = 0.0f32;
    let mut depth = 0.0f32;
    for p in line {
        match p {
            LineSpan::Math(math) => {
                ascent = ascent.max(math.ascent as f32);
                depth = depth.max(math.height.saturating_sub(math.ascent) as f32);
            }
            LineSpan::Type {
                face, text, digits, ..
            } => {
                let shaped = shape_text(size, face, text, *digits);
                ascent = ascent.max(shaped_ink_ascent(face.inner(), TextFace::px(size), &shaped));
            }
        }
    }
    (ascent, depth)
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
    if a > 0.0 { a } else { shaped.ascent }
}

fn wrap_spans<'f>(
    size: TextSize,
    spans: &'f [crate::frame::Span<'_>],
    measure: f32,
    faces: &'f FaceTable,
    digits: Digits,
) -> Result<Vec<Vec<LineSpan<'f>>>, Error> {
    let note_face = if spans
        .iter()
        .any(|s| matches!(s, crate::frame::Span::Type { note: Some(_), .. }))
    {
        Some(faces.text(Cut::Roman)?)
    } else {
        None
    };
    let mut chunks: Vec<Vec<Word<'f>>> = vec![Vec::new()];
    let mut empty_face: Option<&'f TextFace> = None;
    for span in spans {
        match span {
            crate::frame::Span::Math(math) => {
                let face = empty_face.map_or_else(|| faces.text(Cut::Roman), Ok)?;
                empty_face = Some(face);
                chunks.last_mut().unwrap().push(Word::Math(math));
            }
            crate::frame::Span::Type { cut, text, note } => {
                let face = faces.text(*cut)?;
                empty_face = Some(face);
                for (hi, hard) in text.split('\n').enumerate() {
                    if hi > 0 {
                        chunks.push(Vec::new());
                    }
                    let atomic = *cut == Cut::Mono && !hard.is_empty();
                    if atomic {
                        let piece = LineSpan::Type {
                            face,
                            text: hard.to_string(),
                            note: *note,
                            digits,
                        };
                        let w = line_width(size, std::slice::from_ref(&piece), note_face);
                        if w <= measure || !hard.contains(' ') {
                            chunks.last_mut().unwrap().push(Word::Type {
                                face,
                                text: hard.to_string(),
                                note: *note,
                                digits,
                            });
                            continue;
                        }
                    }
                    let words: Vec<&str> = hard.split(' ').filter(|w| !w.is_empty()).collect();
                    let n = words.len();
                    for (wi, word) in words.into_iter().enumerate() {
                        let note = if wi + 1 == n { *note } else { None };
                        chunks.last_mut().unwrap().push(Word::Type {
                            face,
                            text: word.to_string(),
                            note,
                            digits,
                        });
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    for chunk in chunks {
        if chunk.is_empty() {
            if let Some(face) = empty_face {
                out.push(vec![LineSpan::Type {
                    face,
                    text: String::new(),
                    note: None,
                    digits,
                }]);
            } else {
                out.push(Vec::new());
            }
            continue;
        }
        out.extend(wrap_chunk(size, &chunk, measure, note_face));
    }
    if out.is_empty() {
        out.push(Vec::new());
    }
    Ok(out)
}

fn wrap_chunk<'f>(
    size: TextSize,
    words: &[Word<'f>],
    measure: f32,
    note_face: Option<&'f TextFace>,
) -> Vec<Vec<LineSpan<'f>>> {
    let n = words.len();
    if n == 0 {
        return vec![Vec::new()];
    }
    let mut dp = vec![f64::INFINITY; n + 1];
    let mut prev = vec![0usize; n + 1];
    dp[0] = 0.0;
    for j in 1..=n {
        for i in (0..j).rev() {
            let line = words_to_line(&words[i..j]);
            let w = line_width(size, &line, note_face);
            if w > measure && j - i > 1 {
                break;
            }
            let n_boxes = j - i;
            let last = j == n;
            let cost = if w > measure || (last && n_boxes >= 2) {
                0.0
            } else {
                let r = (measure - w) as f64;
                r * r
            };
            let total = dp[i] + cost;
            if total <= dp[j] {
                dp[j] = total;
                prev[j] = i;
            }
        }
    }
    let mut ends = Vec::new();
    let mut j = n;
    while j > 0 {
        let i = prev[j];
        ends.push((i, j));
        j = i;
    }
    ends.reverse();
    ends.into_iter()
        .map(|(i, j)| words_to_line(&words[i..j]))
        .collect()
}

fn words_to_line<'f>(words: &[Word<'f>]) -> Vec<LineSpan<'f>> {
    let mut line = Vec::new();
    for word in words {
        push_word(&mut line, word);
    }
    line
}

fn push_word<'a>(line: &mut Vec<LineSpan<'a>>, word: &Word<'a>) {
    match word {
        Word::Math(math) => line.push(LineSpan::Math(math)),
        Word::Type {
            face,
            text,
            note,
            digits,
        } => match line.last_mut() {
            Some(LineSpan::Type {
                face: prev_face,
                text: prev_text,
                note: prev_note,
                digits: prev_digits,
            }) if std::ptr::eq(*prev_face, *face)
                && *prev_digits == *digits
                && prev_note.is_none()
                && !prev_text.is_empty() =>
            {
                prev_text.push(' ');
                prev_text.push_str(text);
                *prev_note = *note;
            }
            _ => line.push(LineSpan::Type {
                face,
                text: text.clone(),
                note: *note,
                digits: *digits,
            }),
        },
    }
}

fn line_width(size: TextSize, line: &[LineSpan<'_>], note_face: Option<&TextFace>) -> f32 {
    let mut w = 0.0;
    for (i, piece) in line.iter().enumerate() {
        if i > 0
            && let Some(face) = line.iter().find_map(LineSpan::face)
        {
            w += face.shape(" ", size).width;
        }
        match piece {
            LineSpan::Math(math) => w += math.width as f32,
            LineSpan::Type {
                face,
                text,
                note,
                digits,
            } => {
                w += shape_text(size, face, text, *digits).width;
                if let (Some(n), Some(nf)) = (note, note_face) {
                    w += nf.shape(&n.get().to_string(), TextSize::Pt8).width;
                }
            }
        }
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
