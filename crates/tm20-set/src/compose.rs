//! Small evaluator: [`Sheet`] + [`FaceTable`] → packed [`tm20::Graphics`].

use std::borrow::Cow;
use std::num::NonZeroU32;

use crate::error::Error;
use crate::face::{Cut, DisplayFace, Face, FaceTable, Shaped, TextFace};
use crate::frame::{
    decimal_text, ColAlign, Cols, Figure, Frame, Head, List, MarkAlign, Marker, Quote, Rule, Sheet,
    TextBlock, Thickness, EN_DASH,
};
use crate::leading::{GRID, HANG};
use crate::size::TextSize;
use tm20::graphics::{pack, Graphics, GraphicsScale};

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

/// What the last painted frame was, for adjacency. Text, List, Quote, and Figure share [`Kind::Run`].
#[derive(Clone, Copy)]
enum Kind {
    Mark { slug: u16 },
    Head,
    Run,
    Cols,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Rhythm {
    Mark,
    Head,
    Run,
    Cols,
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
        Frame::Text(_) | Frame::List(_) | Frame::Quote(_) | Frame::Figure(_) => Rhythm::Run,
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
        Frame::Quote(q) => q.frames.first().map(slug).unwrap_or(0),
        Frame::Mark(m) => m.size.skip_dots(),
        Frame::Figure(_) => GRID,
        Frame::Rule(_) => 0,
    }
}

fn extra(place: Place, to: Rhythm, next: u16) -> u16 {
    match place {
        Place::Origin { .. } => 0,
        Place::Rule { .. } => match to {
            Rhythm::Cols => 0,
            Rhythm::Mark | Rhythm::Head | Rhythm::Run | Rhythm::Rule => next,
        },
        Place::Line { kind, .. } => match (kind, to) {
            (Kind::Mark { .. }, Rhythm::Mark | Rhythm::Rule) => 0,
            (Kind::Mark { slug }, _) => slug,
            (Kind::Head, Rhythm::Run | Rhythm::Cols | Rhythm::Rule) => 0,
            (Kind::Cols, Rhythm::Cols) => 0,
            (_, Rhythm::Head) => next,
            (Kind::Run | Kind::Head, Rhythm::Run | Rhythm::Cols | Rhythm::Mark) => next,
            (Kind::Cols, Rhythm::Run | Rhythm::Mark) => next,
            (Kind::Run | Kind::Cols, Rhythm::Rule) => 0,
        },
    }
}

struct MarkInk<'a> {
    x: f32,
    face: &'a TextFace,
    shaped: Shaped,
    px: f32,
}

struct Cx<'a, 'f> {
    canvas: &'a mut Canvas,
    cur: &'a mut Cursor,
    faces: &'f FaceTable,
    pending: Vec<MarkInk<'f>>,
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
    paint_seq(&mut cx, &sheet.frames, 0, width, 0, 0, false)?;
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
    first_continues: bool,
) -> Result<(), Error> {
    if frames.is_empty() {
        let b = cx.cur.first_baseline(0.0, GRID, Kind::Run);
        flush_marks(cx, b);
        return Ok(());
    }
    for (i, frame) in frames.iter().enumerate() {
        if !(i == 0 && first_continues) {
            cx.cur.bump(extra(cx.cur.place, rhythm(frame), slug(frame)));
        }
        paint_one(cx, frame, x0, measure, quote_depth, list_depth)?;
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
        Frame::Text(block) => paint_run(cx, block, x0, measure, Kind::Run),
        Frame::Head(head) => paint_head(cx, head, x0, measure),
        Frame::Cols(cols) => paint_cols(cx, cols, x0, measure),
        Frame::List(list) => paint_list(cx, list, x0, measure, quote_depth, list_depth),
        Frame::Quote(quote) => paint_quote(cx, quote, x0, measure, quote_depth, list_depth),
        Frame::Figure(fig) => paint_figure(cx, fig, x0, measure),
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
    let b = cx
        .cur
        .first_baseline(ascent, skip, Kind::Mark { slug: skip });
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
    paint_run(cx, &block, x0, measure, Kind::Head)
}

fn paint_run(
    cx: &mut Cx<'_, '_>,
    block: &TextBlock<'_>,
    x0: u16,
    measure: u16,
    kind: Kind,
) -> Result<(), Error> {
    let measure = measure.max(1);
    let skip = block.size.skip_dots();
    let lines = wrap_spans(block.size, &block.spans, measure as f32, cx.faces, false)?;
    for (li, line) in lines.iter().enumerate() {
        let ascent = line_ascent(block.size, line);
        let b = if li == 0 {
            cx.cur.first_baseline(ascent, skip, kind)
        } else {
            cx.cur.later_baseline(ascent, skip)
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
    let x = x0.saturating_add(GRID);
    let w = measure.saturating_sub(GRID).max(1);
    paint_seq(cx, &quote.frames, x, w, quote_depth + 1, list_depth, true)
}

fn paint_figure(cx: &mut Cx<'_, '_>, fig: &Figure, x0: u16, measure: u16) -> Result<(), Error> {
    let b = cx.cur.first_baseline(0.0, GRID, Kind::Run);
    flush_marks(cx, b);
    let top = b.round().max(0.0) as u16;
    blit_figure(cx.canvas, x0, top, fig, measure);
    let bottom = top as f32 + fig.height as f32;
    cx.cur.place = Place::Line {
        baseline: bottom,
        slug_bottom: bottom + GRID as f32,
        kind: Kind::Run,
    };
    Ok(())
}

fn blit_figure(canvas: &mut Canvas, x0: u16, y0: u16, fig: &Figure, max_w: u16) {
    let w = fig.width.min(max_w);
    for y in 0..fig.height {
        for x in 0..w {
            if fig.bits[y as usize * fig.width as usize + x as usize] {
                canvas.set(x0 as i32 + x as i32, y0 as i32 + y as i32);
            }
        }
    }
}

fn paint_notes(cx: &mut Cx<'_, '_>, width: u16, notes: &[Cow<'_, str>]) -> Result<(), Error> {
    let rule = Rule {
        thickness: Thickness::One,
    };
    cx.cur.bump(extra(cx.cur.place, Rhythm::Rule, 0));
    paint_rule(cx, &rule, 0, width)?;
    let items: Vec<Vec<Frame<'_>>> = notes
        .iter()
        .map(|n| {
            vec![Frame::Text(TextBlock::plain(
                Cut::Roman,
                TextSize::Pt8,
                n.as_ref(),
            ))]
        })
        .collect();
    let list = List {
        size: TextSize::Pt8,
        cut: Cut::Roman,
        marker: Marker::Decimal {
            start: 1,
            delim: crate::frame::DecimalDelim::Period,
        },
        tight: true,
        items,
    };
    cx.cur
        .bump(extra(cx.cur.place, Rhythm::Run, list.size.skip_dots()));
    paint_list(cx, &list, 0, width, 0, 0)
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
    let content_x = x0.saturating_add(hang);
    let content_w = measure.saturating_sub(hang).max(1);
    let face = faces.text(list.cut)?;
    let space = face.shape(" ", list.size).width;
    let px = TextFace::px(list.size);
    for (i, item) in list.items.iter().enumerate() {
        let (mx, shaped) = match list.marker {
            Marker::Dash => (x0 as f32, face.shape(EN_DASH, list.size)),
            Marker::Decimal { start, delim } => {
                let t = decimal_text(start.saturating_add(i as u32), delim);
                let s = face.shape_figure(&t, list.size);
                let x = (x0 as f32 + hang as f32 - space - s.width).max(x0 as f32);
                (x, s)
            }
        };
        cx.pending.push(MarkInk {
            x: mx,
            face,
            shaped,
            px,
        });
        let tight_continue = list.tight && i > 0;
        paint_seq(
            cx,
            item,
            content_x,
            content_w,
            quote_depth,
            list_depth + 1,
            tight_continue,
        )?;
    }
    Ok(())
}

fn paint_cols(cx: &mut Cx<'_, '_>, cols: &Cols<'_>, x0: u16, measure: u16) -> Result<(), Error> {
    let n = cols.align.len();
    if !(2..=3).contains(&n) {
        return Err(Error::Cols);
    }
    for row in &cols.rows {
        if row.len() != n {
            return Err(Error::Cols);
        }
    }
    if cols.rows.is_empty() {
        let b = cx
            .cur
            .first_baseline(0.0, cols.size.skip_dots(), Kind::Cols);
        flush_marks(cx, b);
        return Ok(());
    }
    let gutter = cols.gutter.dots();
    let gutters = gutter * (n as u16 - 1);
    let inner = measure.saturating_sub(gutters).max(1);
    let widths = col_widths(cols, n, inner, cx.faces)?;
    let skip = cols.size.skip_dots();
    for (ri, row) in cols.rows.iter().enumerate() {
        let mut cell_lines = Vec::with_capacity(n);
        for (c, cell) in row.iter().enumerate() {
            let figure = cols.align[c] == ColAlign::End;
            cell_lines.push(wrap_spans(
                cols.size,
                cell,
                widths[c] as f32,
                cx.faces,
                figure,
            )?);
        }
        let nlines = cell_lines.iter().map(|l| l.len().max(1)).max().unwrap_or(1);
        for li in 0..nlines {
            let mut ascent = 0.0f32;
            for lines in &cell_lines {
                if let Some(line) = lines.get(li) {
                    ascent = ascent.max(line_ascent(cols.size, line));
                }
            }
            let b = if ri == 0 && li == 0 {
                cx.cur.first_baseline(ascent, skip, Kind::Cols)
            } else {
                cx.cur.later_baseline(ascent, skip)
            };
            if ri == 0 && li == 0 {
                flush_marks(cx, b);
            }
            let mut x = x0 as f32;
            for c in 0..n {
                if let Some(line) = cell_lines[c].get(li) {
                    let notes = note_face_of(cx.faces, line)?;
                    let lw = line_width(cols.size, line, notes);
                    let lx = if cols.align[c] == ColAlign::End {
                        x + (widths[c] as f32 - lw).max(0.0)
                    } else {
                        x
                    };
                    paint_line(cx, cols.size, lx, b, line)?;
                }
                x += widths[c] as f32 + gutter as f32;
            }
        }
    }
    Ok(())
}

fn col_widths(cols: &Cols<'_>, n: usize, inner: u16, faces: &FaceTable) -> Result<Vec<u16>, Error> {
    let mut widths = vec![0u16; n];
    let mut end_sum = 0u16;
    let mut start_n = 0u16;
    for (c, align) in cols.align.iter().enumerate() {
        if *align == ColAlign::Start {
            start_n += 1;
            continue;
        }
        let mut w = 1u16;
        for row in &cols.rows {
            let lines = wrap_spans(cols.size, &row[c], inner as f32, faces, true)?;
            let mut lw = 0.0f32;
            for line in &lines {
                let notes = note_face_of(faces, line)?;
                lw = lw.max(line_width(cols.size, line, notes));
            }
            w = w.max(lw.ceil().max(1.0) as u16);
        }
        w = w.min(inner);
        widths[c] = w;
        end_sum = end_sum.saturating_add(w);
    }
    if end_sum > inner {
        let mut used = 0u16;
        for (c, align) in cols.align.iter().enumerate() {
            if *align == ColAlign::End {
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
    for (c, align) in cols.align.iter().enumerate() {
        if *align == ColAlign::Start {
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
    if line.iter().any(|p| p.note.is_some()) {
        Ok(Some(faces.text(Cut::Roman)?))
    } else {
        Ok(None)
    }
}

fn flush_marks(cx: &mut Cx<'_, '_>, baseline: f32) {
    let pending = std::mem::take(&mut cx.pending);
    for m in pending {
        blit(cx.canvas, m.face.inner(), m.px, m.x, baseline, &m.shaped);
    }
}

#[derive(Clone)]
struct LineSpan<'a> {
    face: &'a TextFace,
    text: String,
    note: Option<NonZeroU32>,
    figure: bool,
}

struct Word<'a> {
    face: &'a TextFace,
    text: String,
    note: Option<NonZeroU32>,
    figure: bool,
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
        if i > 0 {
            x += piece.face.shape(" ", size).width;
        }
        let shaped = shape_piece(size, piece);
        blit(
            cx.canvas,
            piece.face.inner(),
            TextFace::px(size),
            x,
            baseline,
            &shaped,
        );
        x += shaped.width;
        if let Some(n) = piece.note {
            let face = note_face.expect("note_face_of checked");
            let label = n.get().to_string();
            let note = face.shape(&label, TextSize::Pt8);
            let raise = TextFace::px(size) * NOTE_RAISE;
            blit(
                cx.canvas,
                face.inner(),
                TextFace::px(TextSize::Pt8),
                x,
                baseline - raise,
                &note,
            );
            x += note.width;
        }
    }
    Ok(())
}

fn shape_piece(size: TextSize, piece: &LineSpan<'_>) -> Shaped {
    if piece.figure {
        piece.face.shape_figure(&piece.text, size)
    } else {
        piece.face.shape(&piece.text, size)
    }
}

fn line_ascent(size: TextSize, line: &[LineSpan<'_>]) -> f32 {
    line.iter()
        .map(|p| {
            let shaped = shape_piece(size, p);
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
    spans: &[crate::frame::Span<'_>],
    measure: f32,
    faces: &'f FaceTable,
    figure: bool,
) -> Result<Vec<Vec<LineSpan<'f>>>, Error> {
    let note_face = if spans.iter().any(|s| s.note.is_some()) {
        Some(faces.text(Cut::Roman)?)
    } else {
        None
    };
    let mut chunks: Vec<Vec<Word<'f>>> = vec![Vec::new()];
    let mut empty_face: Option<&'f TextFace> = None;
    for span in spans {
        let face = faces.text(span.cut)?;
        empty_face = Some(face);
        let text = span.text.as_ref();
        for (hi, hard) in text.split('\n').enumerate() {
            if hi > 0 {
                chunks.push(Vec::new());
            }
            let words: Vec<&str> = hard.split(' ').filter(|w| !w.is_empty()).collect();
            let n = words.len();
            for (wi, word) in words.into_iter().enumerate() {
                let note = if wi + 1 == n { span.note } else { None };
                chunks.last_mut().unwrap().push(Word {
                    face,
                    text: word.to_string(),
                    note,
                    figure,
                });
            }
        }
    }
    let mut out = Vec::new();
    for chunk in chunks {
        if chunk.is_empty() {
            if let Some(face) = empty_face {
                out.push(vec![LineSpan {
                    face,
                    text: String::new(),
                    note: None,
                    figure,
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
    let greedy = wrap_greedy(size, words, measure, note_face);
    let n = greedy.len();
    if n <= 1 {
        return greedy;
    }
    let mut lo = 0.0f32;
    let mut hi = measure;
    for _ in 0..40 {
        let mid = (lo + hi) * 0.5;
        if wrap_greedy(size, words, mid, note_face).len() <= n {
            hi = mid;
        } else {
            lo = mid;
        }
        if hi - lo < 0.25 {
            break;
        }
    }
    wrap_greedy(size, words, hi, note_face)
}

fn wrap_greedy<'f>(
    size: TextSize,
    words: &[Word<'f>],
    measure: f32,
    note_face: Option<&'f TextFace>,
) -> Vec<Vec<LineSpan<'f>>> {
    let mut out = Vec::new();
    let mut line: Vec<LineSpan<'f>> = Vec::new();
    for word in words {
        let mut trial = line.clone();
        push_word(&mut trial, word);
        if line_width(size, &trial, note_face) <= measure || line.is_empty() {
            line = trial;
        } else {
            out.push(std::mem::take(&mut line));
            push_word(&mut line, word);
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
    if out.is_empty() {
        out.push(Vec::new());
    }
    out
}

fn push_word<'a>(line: &mut Vec<LineSpan<'a>>, word: &Word<'a>) {
    match line.last_mut() {
        Some(prev)
            if std::ptr::eq(prev.face, word.face)
                && prev.figure == word.figure
                && prev.note.is_none()
                && !prev.text.is_empty() =>
        {
            prev.text.push(' ');
            prev.text.push_str(&word.text);
            prev.note = word.note;
        }
        _ => line.push(LineSpan {
            face: word.face,
            text: word.text.clone(),
            note: word.note,
            figure: word.figure,
        }),
    }
}

fn line_width(size: TextSize, line: &[LineSpan<'_>], note_face: Option<&TextFace>) -> f32 {
    let mut w = 0.0;
    for (i, piece) in line.iter().enumerate() {
        if i > 0 {
            w += piece.face.shape(" ", size).width;
        }
        w += shape_piece(size, piece).width;
        if let Some(n) = piece.note {
            if let Some(face) = note_face {
                w += face.shape(&n.get().to_string(), TextSize::Pt8).width;
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
