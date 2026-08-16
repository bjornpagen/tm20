//! Small evaluator: [`Sheet`] → packed [`tm20::Graphics`].

use crate::error::Error;
use crate::face::{DisplayFace, Face, Shaped, TextFace};
use crate::frame::{Cell, Frame, List, MarkAlign, Sheet, Span, Table, TextBlock, EN_DASH};
use crate::leading::{Leading, HANG};
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

#[derive(Clone, Copy)]
enum Last {
    Start,
    Mark { slug: u16 },
    Head,
    Text,
    List,
    Table,
    Rule,
}

struct Cursor {
    baseline: Option<f32>,
    hang_from: Option<f32>,
    floor: f32,
    slug_bottom: f32,
    last: Last,
}

impl Cursor {
    fn new() -> Self {
        Self {
            baseline: None,
            hang_from: None,
            floor: 0.0,
            slug_bottom: 0.0,
            last: Last::Start,
        }
    }

    fn bump(&mut self, extra: u16) {
        if extra == 0 {
            return;
        }
        match self.baseline {
            Some(b) => self.baseline = Some(b + extra as f32),
            None => self.floor += extra as f32,
        }
    }

    fn set_baseline(&mut self, b: f32, ascent: f32, skip: u16) -> f32 {
        self.baseline = Some(b);
        self.slug_bottom = self.slug_bottom.max(b - ascent + skip as f32);
        b
    }

    fn first_baseline(&mut self, ascent: f32, skip: u16) -> f32 {
        if let Some(origin) = self.hang_from.take() {
            return self.set_baseline(origin + HANG as f32 + ascent, ascent, skip);
        }
        let b = match self.baseline {
            None => self.floor + ascent,
            Some(prev) => prev + skip as f32,
        };
        self.set_baseline(b, ascent, skip)
    }

    fn later_baseline(&mut self, ascent: f32, skip: u16) -> f32 {
        let b = self.baseline.unwrap_or(self.floor) + skip as f32;
        self.set_baseline(b, ascent, skip)
    }
}

fn text_leading(size: TextSize) -> u16 {
    Leading::for_text(size).skip_dots(size.body_dots())
}

fn apply_gap(cur: &mut Cursor, frame: &Frame<'_>) {
    let next_l = match frame {
        Frame::Text(b) => text_leading(b.size),
        Frame::Head(h) => text_leading(h.size),
        Frame::List(l) => text_leading(l.size),
        Frame::Table(t) => text_leading(t.size),
        Frame::Mark(m) => Leading::for_display(m.size).skip_dots(m.size.body_dots()),
        Frame::Rule(_) => 0,
    };
    match (cur.last, frame) {
        (Last::Start, _) => {}
        (Last::Mark { .. }, Frame::Mark(_) | Frame::Rule(_)) => {}
        (Last::Mark { slug }, _) => cur.bump(slug),
        (Last::Head, Frame::Text(_) | Frame::List(_) | Frame::Table(_) | Frame::Rule(_)) => {}
        (Last::Table, Frame::Table(_)) => {}
        (Last::Rule, Frame::Table(_)) => {}
        (Last::Rule, _) => {
            cur.hang_from = None;
            cur.bump(next_l);
        }
        (_, Frame::Head(_)) => cur.bump(next_l),
        (
            Last::Text | Last::List | Last::Head,
            Frame::Text(_) | Frame::List(_) | Frame::Table(_) | Frame::Mark(_),
        ) => cur.bump(next_l),
        (Last::Table, Frame::Text(_) | Frame::List(_) | Frame::Mark(_)) => cur.bump(next_l),
        (Last::Text | Last::List | Last::Table, Frame::Rule(_)) => {}
    }
}

/// Layout `sheet` onto one 1-bit canvas and pack it.
pub fn compose(sheet: &Sheet<'_>) -> Result<Graphics, Error> {
    let width = sheet.width.get();
    let mut canvas = Canvas::new(width);
    let mut cur = Cursor::new();

    for frame in &sheet.frames {
        apply_gap(&mut cur, frame);
        match frame {
            Frame::Rule(rule) => {
                let y = canvas.height.max(cur.slug_bottom.ceil() as u16);
                for dy in 0..rule.thickness.dots() {
                    canvas.fill_row(y + dy, 0, width);
                }
                let bottom = (y + rule.thickness.dots()) as f32;
                cur.hang_from = Some(bottom);
                cur.baseline = None;
                cur.floor = bottom;
                cur.slug_bottom = bottom;
                cur.last = Last::Rule;
            }
            Frame::Mark(mark) => {
                let shaped = mark.face.shape(mark.text, mark.size, mark.tracking.0);
                let skip = Leading::for_display(mark.size).skip_dots(mark.size.body_dots());
                let ascent =
                    shaped_ink_ascent(mark.face.inner(), DisplayFace::px(mark.size), &shaped);
                let b = cur.first_baseline(ascent, skip);
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
                cur.last = Last::Mark { slug: skip };
            }
            Frame::Text(block) => {
                paint_text(&mut canvas, &mut cur, width, block)?;
                cur.last = Last::Text;
            }
            Frame::Head(head) => {
                let block = TextBlock::plain(head.face, head.size, head.text);
                paint_text(&mut canvas, &mut cur, width, &block)?;
                cur.last = Last::Head;
            }
            Frame::Table(table) => {
                paint_table(&mut canvas, &mut cur, width, table)?;
                cur.last = Last::Table;
            }
            Frame::List(list) => {
                paint_list(&mut canvas, &mut cur, width, list)?;
                cur.last = Last::List;
            }
        }
    }

    canvas.into_graphics()
}

fn paint_text(
    canvas: &mut Canvas,
    cur: &mut Cursor,
    width: u16,
    block: &TextBlock<'_>,
) -> Result<(), Error> {
    let measure = width.max(1);
    let skip = Leading::for_text(block.size).skip_dots(block.size.body_dots());
    let lines = wrap_spans(block.size, &block.spans, measure as f32);
    for (li, line) in lines.iter().enumerate() {
        let ascent = line_ascent(block.size, line);
        let b = if li == 0 {
            cur.first_baseline(ascent, skip)
        } else {
            cur.later_baseline(ascent, skip)
        };
        paint_line(canvas, block.size, 0.0, b, line);
    }
    Ok(())
}

fn paint_table(
    canvas: &mut Canvas,
    cur: &mut Cursor,
    width: u16,
    table: &Table<'_>,
) -> Result<(), Error> {
    let origins = table.columns.origins();
    let skip = Leading::for_text(table.size).skip_dots(table.size.body_dots());
    for row in &table.rows {
        if row.cells.len() != table.columns.widths().len() {
            return Err(Error::Columns);
        }
        if let Some(th) = row.rule {
            let y = canvas.height.max(cur.slug_bottom.ceil() as u16);
            for dy in 0..th.dots() {
                canvas.fill_row(y + dy, 0, width);
            }
            let bottom = (y + th.dots()) as f32;
            cur.hang_from = Some(bottom);
            cur.baseline = None;
            cur.floor = bottom;
            cur.slug_bottom = bottom;
        }

        let mut wrapped: Vec<Vec<Vec<LineSpan<'_>>>> = Vec::new();
        let mut first_ascent = 0.0f32;
        for (i, cell) in row.cells.iter().enumerate() {
            let col_w = (origins[i].1 - origins[i].0).max(1) as f32;
            match cell {
                Cell::Label(spans) => {
                    let lines = wrap_spans(table.size, spans, col_w);
                    if let Some(first) = lines.first() {
                        first_ascent = first_ascent.max(line_ascent(table.size, first));
                    }
                    wrapped.push(lines);
                }
                Cell::Figure { face, text } => {
                    let shaped = face.shape(text, table.size, true);
                    first_ascent = first_ascent.max(shaped_ink_ascent(
                        face.inner(),
                        TextFace::px(table.size),
                        &shaped,
                    ));
                    wrapped.push(Vec::new());
                }
                Cell::Empty => wrapped.push(Vec::new()),
            }
        }

        let max_wrap = wrapped.iter().map(|l| l.len()).max().unwrap_or(0).max(1);
        for li in 0..max_wrap {
            let ascent = if li == 0 {
                first_ascent
            } else {
                wrapped
                    .iter()
                    .filter_map(|lines| lines.get(li).map(|l| line_ascent(table.size, l)))
                    .fold(0.0f32, f32::max)
            };
            let b = if li == 0 {
                cur.first_baseline(ascent, skip)
            } else {
                cur.later_baseline(ascent, skip)
            };
            for (i, lines) in wrapped.iter().enumerate() {
                if let Some(line) = lines.get(li) {
                    paint_line(canvas, table.size, origins[i].0 as f32, b, line);
                }
            }
            if li == 0 {
                for (i, cell) in row.cells.iter().enumerate() {
                    if let Cell::Figure { face, text } = cell {
                        let shaped = face.shape(text, table.size, true);
                        let x = (origins[i].1 as f32 - shaped.width).max(0.0);
                        blit(
                            canvas,
                            face.inner(),
                            TextFace::px(table.size),
                            x,
                            b,
                            &shaped,
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn paint_list(
    canvas: &mut Canvas,
    cur: &mut Cursor,
    width: u16,
    list: &List<'_>,
) -> Result<(), Error> {
    let hang = list.hang_dots();
    let measure = (width.saturating_sub(hang)).max(1);
    let skip = Leading::for_text(list.size).skip_dots(list.size.body_dots());
    let dash = list.face.shape(EN_DASH, list.size, false);
    let dash_ascent = shaped_ink_ascent(list.face.inner(), TextFace::px(list.size), &dash);
    for (ii, item) in list.items.iter().enumerate() {
        let lines = wrap_spans(list.size, item, measure as f32);
        for (li, line) in lines.iter().enumerate() {
            let ascent = line_ascent(list.size, line).max(dash_ascent);
            let b = if ii == 0 && li == 0 {
                cur.first_baseline(ascent, skip)
            } else {
                cur.later_baseline(ascent, skip)
            };
            if li == 0 {
                blit(
                    canvas,
                    list.face.inner(),
                    TextFace::px(list.size),
                    0.0,
                    b,
                    &dash,
                );
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
            x += piece.face.shape(" ", size, false).width;
        }
        let shaped = piece.face.shape(&piece.text, size, false);
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
            let shaped = p.face.shape(&p.text, size, false);
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

fn wrap_spans<'a>(size: TextSize, spans: &[Span<'a>], measure: f32) -> Vec<Vec<LineSpan<'a>>> {
    let mut out = Vec::new();
    let mut line: Vec<LineSpan<'a>> = Vec::new();
    for span in spans {
        for (hi, hard) in span.text.split('\n').enumerate() {
            if hi > 0 && !line.is_empty() {
                out.push(std::mem::take(&mut line));
            }
            if hard.is_empty() {
                if line.is_empty() {
                    out.push(vec![LineSpan {
                        face: span.face,
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
                push_word(&mut trial, span.face, word);
                if line_width(size, &trial) <= measure || line.is_empty() {
                    line = trial;
                } else {
                    out.push(std::mem::take(&mut line));
                    push_word(&mut line, span.face, word);
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
    out
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
            w += piece.face.shape(" ", size, false).width;
        }
        w += piece.face.shape(&piece.text, size, false).width;
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
mod tests {
    use super::*;
    use crate::face::{DisplayFace, Slope, TextFace, Weight};
    use crate::frame::{
        Frame, Head, List, Mark, MarkAlign, Measure, Rule, Sheet, Table, TextBlock, Thickness,
        Tracking,
    };
    use crate::leading::{GridSkip, Leading, GRID, HANG};
    use crate::size::{DisplaySize, TextSize};
    use tm20::graphics::width_bytes;
    use tm20::PRINTABLE_DOTS;

    fn text() -> TextFace {
        TextFace::sans(Weight::Roman, Slope::Upright).expect("system sans")
    }

    fn bold() -> TextFace {
        TextFace::sans(Weight::Bold, Slope::Upright)
            .or_else(|_| TextFace::sans(Weight::Roman, Slope::Upright))
            .expect("system sans")
    }

    fn display() -> DisplayFace {
        DisplayFace::sans(Weight::Roman, Slope::Upright).expect("system sans")
    }

    fn l11() -> u16 {
        Leading::for_text(TextSize::Pt11).skip_dots(TextSize::Pt11.body_dots())
    }

    fn first_ink_after(g: &Graphics, from: usize) -> usize {
        let stride = width_bytes(g.width_dots);
        for y in from..g.height_dots as usize {
            if g.pixels[y * stride..(y + 1) * stride]
                .iter()
                .any(|&b| b != 0)
            {
                return y;
            }
        }
        panic!("no ink after {from}")
    }

    fn full_width_row(g: &Graphics, y: usize) -> bool {
        let stride = width_bytes(g.width_dots);
        let row = &g.pixels[y * stride..(y + 1) * stride];
        row.iter().all(|&b| b == 0xff)
    }

    #[test]
    fn compose_is_tape_wide() {
        let face = text();
        let sheet = Sheet::tape(vec![Frame::Text(TextBlock::plain(
            &face,
            TextSize::Pt11,
            "Hello",
        ))]);
        let g = compose(&sheet).unwrap();
        assert_eq!(g.width_dots, PRINTABLE_DOTS);
        assert!(g.pixels.iter().any(|&b| b != 0));
    }

    #[test]
    fn pair_has_ink_on_both_sides() {
        let face = text();
        let table = Table::pair(
            Measure::TAPE,
            GridSkip::ONE,
            TextSize::Pt11,
            vec![crate::frame::Span {
                face: &face,
                text: "Coffee",
            }],
            &face,
            "$4.50",
        )
        .unwrap();
        let g = compose(&Sheet::tape(vec![Frame::Table(table)])).unwrap();
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
        let one = compose(&Sheet::tape(vec![Frame::Text(TextBlock::plain(
            &face,
            TextSize::Pt11,
            "Hello",
        ))]))
        .unwrap();
        let wrapped = compose(&Sheet::tape(vec![Frame::Text(TextBlock::plain(
            &face,
            TextSize::Pt11,
            "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
        ))]))
        .unwrap();
        assert!(wrapped.height_dots > one.height_dots);
    }

    #[test]
    fn two_paragraphs_are_a_blank_line_apart() {
        let face = text();
        let lines = compose(&Sheet::tape(vec![Frame::Text(TextBlock::plain(
            &face,
            TextSize::Pt11,
            "H\nH",
        ))]))
        .unwrap();
        let paras = compose(&Sheet::tape(vec![
            Frame::Text(TextBlock::plain(&face, TextSize::Pt11, "H")),
            Frame::Text(TextBlock::plain(&face, TextSize::Pt11, "H")),
        ]))
        .unwrap();
        let extra = paras.height_dots as i32 - lines.height_dots as i32;
        let l = l11() as i32;
        assert!(
            extra >= l - 4 && extra <= l + 8,
            "paragraph extra {extra} should be one leading ({l}), not line skip"
        );
    }

    #[test]
    fn head_sticks_to_the_following_text() {
        let b = bold();
        if b.weight() != Weight::Bold {
            return;
        }
        let face = text();
        let head = Head::new(&b, TextSize::Pt11, "H").unwrap();
        let stuck = compose(&Sheet::tape(vec![
            Frame::Head(head),
            Frame::Text(TextBlock::plain(&face, TextSize::Pt11, "H")),
        ]))
        .unwrap();
        let paras = compose(&Sheet::tape(vec![
            Frame::Text(TextBlock::plain(&face, TextSize::Pt11, "H")),
            Frame::Text(TextBlock::plain(&face, TextSize::Pt11, "H")),
        ]))
        .unwrap();
        assert!(
            stuck.height_dots + 8 < paras.height_dots,
            "head+text {} should be tighter than two paragraphs {}",
            stuck.height_dots,
            paras.height_dots
        );
    }

    #[test]
    fn mark_then_text_has_more_air_than_head_then_text() {
        let b = bold();
        if b.weight() != Weight::Bold {
            return;
        }
        let face = text();
        let disp = display();
        let head = Head::new(&b, TextSize::Pt11, "H").unwrap();
        let after_head = compose(&Sheet::tape(vec![
            Frame::Head(head),
            Frame::Text(TextBlock::plain(&face, TextSize::Pt11, "H")),
        ]))
        .unwrap();
        let after_mark = compose(&Sheet::tape(vec![
            Frame::Mark(Mark {
                face: &disp,
                size: DisplaySize::Pt18,
                text: "H",
                align: MarkAlign::Start,
                tracking: Tracking(0),
            }),
            Frame::Text(TextBlock::plain(&face, TextSize::Pt11, "H")),
        ]))
        .unwrap();
        assert!(
            after_mark.height_dots > after_head.height_dots,
            "display contrast {} should exceed head stick {}",
            after_mark.height_dots,
            after_head.height_dots
        );
    }

    #[test]
    fn rule_sits_below_the_line_slug() {
        let face = text();
        let g = compose(&Sheet::tape(vec![
            Frame::Text(TextBlock::plain(&face, TextSize::Pt11, "H")),
            Frame::Rule(Rule {
                thickness: Thickness::One,
            }),
        ]))
        .unwrap();
        let mut last_type = 0;
        let mut rule_y = None;
        for y in 0..g.height_dots as usize {
            if full_width_row(&g, y) {
                rule_y = Some(y);
                break;
            }
            if g.pixels[y * width_bytes(g.width_dots)..(y + 1) * width_bytes(g.width_dots)]
                .iter()
                .any(|&b| b != 0)
            {
                last_type = y;
            }
        }
        let gap = rule_y.expect("rule") - last_type;
        assert!(
            gap > 2,
            "rule at gap {gap} from last type ink; must clear the slug, not sit in the letters"
        );
    }

    #[test]
    fn table_hangs_from_rule() {
        let face = text();
        let table = Table::pair(
            Measure::TAPE,
            GridSkip::ONE,
            TextSize::Pt11,
            vec![crate::frame::Span {
                face: &face,
                text: "H",
            }],
            &face,
            "$1",
        )
        .unwrap();
        let g = compose(&Sheet::tape(vec![
            Frame::Rule(Rule {
                thickness: Thickness::Two,
            }),
            Frame::Table(table),
        ]))
        .unwrap();
        let gap = first_ink_after(&g, 2) - 2;
        assert!(
            gap <= HANG as usize + 2,
            "gap {gap} should be hang ({HANG})"
        );
        assert!(gap >= 1, "type should not sit in the rule");
    }

    #[test]
    fn text_does_not_hang_from_a_section_rule() {
        let face = text();
        let g = compose(&Sheet::tape(vec![
            Frame::Rule(Rule {
                thickness: Thickness::Two,
            }),
            Frame::Text(TextBlock::plain(&face, TextSize::Pt11, "H")),
        ]))
        .unwrap();
        let gap = first_ink_after(&g, 2) - 2;
        assert!(
            gap >= l11() as usize - 4,
            "gap {gap} should be a line of the text, not hang ({HANG})"
        );
    }

    fn packed_ink(g: &Graphics, y: usize, x: u16) -> bool {
        let stride = width_bytes(g.width_dots);
        let byte = g.pixels[y * stride + x as usize / 8];
        byte & (0x80 >> (x % 8)) != 0
    }

    #[test]
    fn list_runover_clears_the_mark_column() {
        let face = text();
        let list = List {
            size: TextSize::Pt11,
            face: &face,
            items: vec![vec![crate::frame::Span {
                face: &face,
                text: "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
            }]],
        };
        let hang = list.hang_dots();
        assert_eq!(hang % GRID, 0);
        assert!(hang >= GRID);
        let g = compose(&Sheet::tape(vec![Frame::List(list)])).unwrap();
        let mut dash_last = None;
        for y in 0..g.height_dots as usize {
            let mut mark = false;
            for x in 0..hang {
                mark |= packed_ink(&g, y, x);
            }
            if mark {
                dash_last = Some(y);
            }
        }
        let dash_last = dash_last.expect("en-dash in the mark column");
        let mut runover = false;
        for y in dash_last + 1..g.height_dots as usize {
            for x in 0..hang {
                assert!(
                    !packed_ink(&g, y, x),
                    "runover ink in mark column at ({x},{y})"
                );
            }
            for x in hang..g.width_dots {
                runover |= packed_ink(&g, y, x);
            }
        }
        assert!(runover, "wrapped line should sit at the hang, not under the dash");
    }

    #[test]
    fn head_rejects_roman() {
        let face = text();
        assert!(matches!(
            Head::new(&face, TextSize::Pt11, "Hello"),
            Err(Error::HeadNotBold)
        ));
        let b = bold();
        if b.weight() == Weight::Bold {
            assert!(Head::new(&b, TextSize::Pt11, "Hello").is_ok());
        }
    }
}
