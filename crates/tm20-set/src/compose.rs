//! Small evaluator: [`Sheet`] + [`FaceTable`] → a tape-wide raster, packed as
//! one or more [`tm20::Graphics`].

use std::num::NonZeroU32;

use crate::error::Error;
use crate::face::{Cut, DisplayFace, Face, FaceTable, Shaped, TextFace};
use crate::frame::{
    Code, ColAlign, ColBody, Cols, EN_DASH, Figure, Frame, Head, ItemMark, List, ListFit, ListItem,
    MarkAlign, Marker, Math, Note, Quote, Rule, Sheet, TextBlock, Thickness, decimal_text,
};
use crate::leading::{GRID, HANG, NOTE_RULE, TASK_BOX, pt_dots};
use crate::size::{DisplaySize, TextSize};
use tm20::graphics::{Graphics, GraphicsScale, max_height, pack};

const THRESHOLD: u8 = 96;
const NEST_CAP: u8 = 3;
const NOTE_RAISE: f32 = 0.4;

struct Canvas {
    width: u16,
    height: u16,
    bits: Vec<bool>,
    seams: Vec<u16>,
}

impl Canvas {
    fn new(width: u16) -> Self {
        Self {
            width,
            height: 0,
            bits: Vec::new(),
            seams: Vec::new(),
        }
    }

    fn record_seam(&mut self, y: u16) {
        if y == 0 {
            return;
        }
        self.ensure(y);
        self.seams.push(y);
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

    fn finish(mut self, slug_bottom: f32) -> Self {
        let y = self.height.max(slug_bottom.ceil().max(0.0) as u16).max(1);
        self.ensure(y);
        self.seams.push(y);
        self
    }

    fn pack_full(self) -> Result<Graphics, Error> {
        let Canvas {
            width,
            height,
            mut bits,
            ..
        } = self;
        let height = height.max(1);
        bits.resize(width as usize * height as usize, false);
        pack_slice(width, height, &bits)
    }

    fn into_bands(self) -> Result<Vec<Graphics>, Error> {
        let Canvas {
            width,
            height,
            mut bits,
            mut seams,
        } = self;
        let height = height.max(1);
        bits.resize(width as usize * height as usize, false);
        seams.push(height);
        seams.sort_unstable();
        seams.dedup();
        seams.retain(|&y| y > 0 && y <= height);
        let cap = max_height(width).max(1);
        let ranges = pack_bands(height, cap, &seams);
        let mut out = Vec::with_capacity(ranges.len());
        let stride = width as usize;
        for (start, end) in ranges {
            let h = end - start;
            let row0 = start as usize * stride;
            let row1 = end as usize * stride;
            out.push(pack_slice(width, h, &bits[row0..row1])?);
        }
        Ok(out)
    }
}

fn pack_slice(width: u16, height: u16, bits: &[bool]) -> Result<Graphics, Error> {
    let pixels = pack(width, height, bits).map_err(|_| Error::Overflow {
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

/// Partition `0..h` into the fewest bands of height ≤ `cap`.
/// Among cuts that keep that count, take the latest seam in each window.
fn pack_bands(h: u16, cap: u16, seams: &[u16]) -> Vec<(u16, u16)> {
    let h = u32::from(h.max(1));
    let cap = u32::from(cap.max(1));
    let n = h.div_ceil(cap);
    let mut bands = Vec::new();
    let mut start = 0u32;
    let mut remaining = n;
    while start < h {
        remaining -= 1;
        let lo = (start + 1).max(h.saturating_sub(remaining * cap));
        let hi = (start + cap).min(h);
        let s = seams
            .iter()
            .copied()
            .map(u32::from)
            .filter(|&y| y >= lo && y <= hi)
            .max()
            .unwrap_or(hi);
        bands.push((start as u16, s as u16));
        start = s;
    }
    bands
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
    paint(sheet, faces)?.pack_full()
}

/// Layout `sheet` and slice it into the fewest fn=112 payloads.
pub(crate) fn compose_bands(sheet: &Sheet<'_>, faces: &FaceTable) -> Result<Vec<Graphics>, Error> {
    paint(sheet, faces)?.into_bands()
}

fn paint(sheet: &Sheet<'_>, faces: &FaceTable) -> Result<Canvas, Error> {
    let width = sheet.width.get();
    let mut canvas = Canvas::new(width);
    let mut cur = Cursor::new();
    {
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
    }
    Ok(canvas.finish(cur.slug_bottom()))
}

fn seam(cx: &mut Cx<'_, '_>) {
    let y = cx.cur.slug_bottom().ceil().max(0.0) as u16;
    cx.canvas.record_seam(y);
}

fn grid_seams_through(cx: &mut Cx<'_, '_>, top: u16, bottom: u16) {
    let cap = max_height(cx.canvas.width).max(1);
    if bottom.saturating_sub(top) <= cap {
        return;
    }
    let mut y = top.saturating_add(GRID);
    while y < bottom {
        cx.canvas.record_seam(y);
        let next = y.saturating_add(GRID);
        if next <= y {
            break;
        }
        y = next;
    }
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
        Frame::Text(block) => paint_run(cx, block, x0, measure, true),
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
    seam(cx);
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
    let skip = mark.size.skip_dots();
    let px = DisplayFace::px(mark.size);
    let measure = measure.max(1) as f32;
    let lines = wrap_mark(
        face,
        mark.size,
        mark.tracking.0,
        mark.text.as_ref(),
        measure,
    );
    cx.cur.mark_slug = skip;
    for (li, line) in lines.iter().enumerate() {
        let shaped = face.shape(line, mark.size, mark.tracking.0);
        let ascent = shaped_ink_ascent(face.inner(), px, &shaped);
        let b = if li == 0 {
            cx.cur.first_baseline(ascent, 0.0, skip)
        } else {
            cx.cur.later_baseline(ascent, 0.0, skip)
        };
        if li == 0 {
            flush_marks(cx, b);
        }
        let x = match mark.align {
            MarkAlign::Start => x0 as f32,
            MarkAlign::Center => x0 as f32 + ((measure - shaped.width) * 0.5).max(0.0),
        };
        blit(cx.canvas, face.inner(), px, x, b, &shaped);
        seam(cx);
    }
    Ok(())
}

fn wrap_mark(
    face: &DisplayFace,
    size: DisplaySize,
    tracking: i16,
    text: &str,
    measure: f32,
) -> Vec<String> {
    let mut out = Vec::new();
    for hard in text.split('\n') {
        let words: Vec<&str> = hard.split(' ').filter(|w| !w.is_empty()).collect();
        if words.is_empty() {
            out.push(String::new());
            continue;
        }
        out.extend(wrap_mark_words(face, size, tracking, &words, measure));
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn wrap_mark_words(
    face: &DisplayFace,
    size: DisplaySize,
    tracking: i16,
    words: &[&str],
    measure: f32,
) -> Vec<String> {
    let n = words.len();
    let width = |i: usize, j: usize| face.shape(&words[i..j].join(" "), size, tracking).width;
    let mut dp = vec![f64::INFINITY; n + 1];
    let mut prev = vec![0usize; n + 1];
    dp[0] = 0.0;
    for j in 1..=n {
        for i in (0..j).rev() {
            let w = width(i, j);
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
        .map(|(i, j)| words[i..j].join(" "))
        .collect()
}

fn paint_head(cx: &mut Cx<'_, '_>, head: &Head<'_>, x0: u16, measure: u16) -> Result<(), Error> {
    let block = TextBlock::plain(Cut::Bold, head.size, head.text.as_ref());
    paint_run(cx, &block, x0, measure, false)
}

fn paint_run(
    cx: &mut Cx<'_, '_>,
    block: &TextBlock<'_>,
    x0: u16,
    measure: u16,
    split: bool,
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
        if split {
            seam(cx);
        }
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
        seam(cx);
    }
    Ok(())
}

fn paint_figure(cx: &mut Cx<'_, '_>, fig: &Figure, x0: u16, measure: u16) -> Result<(), Error> {
    let b = cx.cur.first_baseline(0.0, 0.0, GRID);
    flush_marks(cx, b);
    let top = b.round().max(0.0) as u16;
    let w = fig.width.min(measure);
    let x = x0.saturating_add(measure.saturating_sub(w) / 2);
    blit_bits(
        cx.canvas, x as i32, top as i32, fig.width, fig.height, &fig.bits,
    );
    if let Some(n) = fig.note {
        let nf = cx.faces.text(Cut::Roman)?;
        let label = n.get().to_string();
        let shaped_note = nf.shape(&label, TextSize::Pt8);
        let px8 = TextFace::px(TextSize::Pt8);
        let raise = px8 * NOTE_RAISE;
        let mut nx = x as f32 + fig.width as f32;
        if nx + shaped_note.width > cx.canvas.width as f32 {
            nx = (cx.canvas.width as f32 - shaped_note.width).max(x as f32);
        }
        blit(
            cx.canvas,
            nf.inner(),
            px8,
            nx,
            top as f32 + px8 - raise,
            &shaped_note,
        );
    }
    let bottom = top as f32 + fig.height as f32;
    grid_seams_through(cx, top, bottom as u16);
    cx.cur.place = Place::Line {
        baseline: bottom,
        slug_bottom: bottom + GRID as f32,
    };
    seam(cx);
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
    grid_seams_through(cx, top, bottom as u16);
    cx.cur.place = Place::Line {
        baseline: bottom,
        slug_bottom: bottom + GRID as f32,
    };
    seam(cx);
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
    seam(cx);
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
        seam(cx);
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
    let natural = measure_cols(size, align, rows, cx.faces)?;
    let placed = crate::cols::layout(natural, x0, measure, gutter, |w| {
        allocation_cost(size, align, rows, cx.faces, w)
    })?;
    let skip = size.skip_dots();
    for (ri, row) in rows.iter().enumerate() {
        let mut cell_lines = Vec::with_capacity(N);
        for (cell, spans) in placed.col.iter().zip(row.iter()) {
            cell_lines.push(wrap_spans(
                size,
                spans,
                f32::from(cell.width),
                cx.faces,
                col_digits(cell.align),
            )?);
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
            for (cell, lines) in placed.col.iter().zip(&cell_lines) {
                if let Some(line) = lines.get(li) {
                    let notes = note_face_of(cx.faces, line)?;
                    let lw = line_width(size, line, notes);
                    paint_line(cx, size, cell.ink_x(lw), b, line)?;
                }
            }
        }
        seam(cx);
    }
    Ok(())
}

fn measure_cols<const N: usize>(
    size: TextSize,
    align: &[ColAlign; N],
    rows: &[[Vec<crate::frame::Span<'_>>; N]],
    faces: &FaceTable,
) -> Result<crate::cols::Natural<N>, Error> {
    let mut pref = [1u16; N];
    let mut min = [1u16; N];
    for c in 0..N {
        let digits = col_digits(align[c]);
        let mut p = 1u16;
        let mut m = 1u16;
        for row in rows {
            let (unwrapped, _) = wrap_plan(size, &row[c], 10_000.0, faces, digits)?;
            p = p.max(max_line_width(size, &unwrapped, faces)?);
            let (tight, _) = wrap_plan(size, &row[c], 1.0, faces, digits)?;
            m = m.max(max_line_width(size, &tight, faces)?);
        }
        pref[c] = p.max(1);
        min[c] = m.min(pref[c]).max(1);
    }
    Ok(crate::cols::Natural {
        align: *align,
        pref,
        min,
    })
}

fn col_digits(align: ColAlign) -> Digits {
    match align {
        ColAlign::End => Digits::Tabular,
        ColAlign::Start => Digits::Proportional,
    }
}

fn max_line_width(
    size: TextSize,
    lines: &[Vec<LineSpan<'_>>],
    faces: &FaceTable,
) -> Result<u16, Error> {
    let mut w = 1u16;
    for line in lines {
        let notes = note_face_of(faces, line)?;
        w = w.max(line_width(size, line, notes).ceil().max(1.0) as u16);
    }
    Ok(w)
}

fn allocation_cost<const N: usize>(
    size: TextSize,
    align: &[ColAlign; N],
    rows: &[[Vec<crate::frame::Span<'_>>; N]],
    faces: &FaceTable,
    widths: &[u16; N],
) -> Result<f64, Error> {
    let mut total = 0.0;
    for (c, w) in widths.iter().enumerate() {
        let digits = col_digits(align[c]);
        for row in rows {
            let (_, cost) = wrap_plan(size, &row[c], f32::from(*w), faces, digits)?;
            total += cost;
        }
    }
    Ok(total)
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
    Ok(wrap_plan(size, spans, measure, faces, digits)?.0)
}

fn wrap_plan<'f>(
    size: TextSize,
    spans: &'f [crate::frame::Span<'_>],
    measure: f32,
    faces: &'f FaceTable,
    digits: Digits,
) -> Result<(Vec<Vec<LineSpan<'f>>>, f64), Error> {
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
    let mut cost = 0.0;
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
        let (lines, chunk_cost) = wrap_chunk_plan(size, &chunk, measure, note_face);
        cost += chunk_cost;
        cost += 1_000_000_000.0 * lines.len().saturating_sub(1) as f64;
        out.extend(lines);
    }
    if out.is_empty() {
        out.push(Vec::new());
    }
    Ok((out, cost))
}

fn wrap_chunk_plan<'f>(
    size: TextSize,
    words: &[Word<'f>],
    measure: f32,
    note_face: Option<&'f TextFace>,
) -> (Vec<Vec<LineSpan<'f>>>, f64) {
    let n = words.len();
    if n == 0 {
        return (vec![Vec::new()], 0.0);
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
                let r = f64::from(measure - w);
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
    let lines = ends
        .into_iter()
        .map(|(i, j)| words_to_line(&words[i..j]))
        .collect();
    let cost = if dp[n].is_finite() { dp[n] } else { 0.0 };
    (lines, cost)
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

#[cfg(test)]
mod pack_tests {
    use super::pack_bands;

    #[test]
    fn short_sheet_is_one_band() {
        assert_eq!(pack_bands(500, 910, &[500]), vec![(0, 500)]);
    }

    #[test]
    fn latest_seam_in_the_min_count_window() {
        assert_eq!(
            pack_bands(1000, 910, &[100, 200, 400, 800, 1000]),
            vec![(0, 800), (800, 1000)]
        );
    }

    #[test]
    fn h_1818_is_two_payloads_not_three() {
        assert_eq!(pack_bands(1818, 910, &[1818]), vec![(0, 910), (910, 1818)]);
    }

    #[test]
    fn missing_seam_splits_at_the_cap() {
        assert_eq!(pack_bands(1000, 910, &[1000]), vec![(0, 910), (910, 1000)]);
    }

    #[test]
    fn h_2000_is_three_payloads() {
        assert_eq!(
            pack_bands(2000, 910, &[2000]),
            vec![(0, 910), (910, 1820), (1820, 2000)]
        );
    }

    #[test]
    fn a_seam_outside_the_window_does_not_add_a_payload() {
        assert_eq!(
            pack_bands(1818, 910, &[51, 1818]),
            vec![(0, 910), (910, 1818)]
        );
    }
}
