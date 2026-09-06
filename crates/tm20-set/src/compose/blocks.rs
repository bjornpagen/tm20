//! Measured block planning. A subtree is [`Outcome::Empty`] or
//! [`Outcome::Occupied`]; list items and notes place their own first anchor.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use crate::cols::{self, Natural};
use crate::error::Error;
use crate::face::{Cut, ResolvedFaces, ShapedRun};
use crate::frame::{CheckedSheet, DecimalDelim, MarkAlign};
use crate::frame::{
    Code, ColAlign, ColBody, Cols, EN_DASH, Figure, Frame, Head, ItemBody, ItemMark, List, ListFit,
    Marker, Math, Note, Quote, Rule, RuleSpan, TextBlock, decimal_text,
};
use crate::geometry::{Advance, Clip, Measure, Row, RowRange};
use crate::leading::{GRID, NOTE_RULE, TASK_BOX, pt_dots};
use crate::size::TextSize;
use tm20::Raster;
use tm20::graphics::max_height;

use super::inline::{
    max_line_advance, natural_advance, plan_ascent, plan_depth, plan_skip, wrap_display,
};
use super::plan::{
    DigitMode, DrawOp, LayoutPlan, LinePlan, LinePlans, PaintAtom, WrapCost, wrap_text,
};
use super::rhythm::{
    Cursor, NEST_CAP, NOTE_RAISE_DEN, NOTE_RAISE_NUM, QUOTE_HANG, Rhythm, extra, rhythm, slug,
};

/// First-line ownership of a laid-out subtree. Empty has no line; the owning
/// item or note synthesizes a blank slug and keeps the label in scope.
#[derive(Clone, Copy)]
enum Outcome {
    Empty,
    Occupied {
        first_anchor: Advance,
    },
    /// First child is a box or rule top, not a text baseline.
    BoxTop {
        top: Advance,
    },
}

struct Plan<'f> {
    page: Measure,
    faces: &'f ResolvedFaces<'f>,
    cur: Cursor,
    ops: Vec<DrawOp>,
    seams: Vec<Row>,
    /// Only the current table's wraps; cleared after its draw operations exist.
    wrap_memo: RefCell<HashMap<WrapKey, Arc<LinePlans>>>,
}

type WrapKey = (usize, usize, u16, u16, u8);

/// Measured block planning from an admitted sheet, behind [`super::layout`].
pub(crate) fn layout(
    sheet: &CheckedSheet<'_, '_>,
    faces: &ResolvedFaces<'_>,
) -> Result<LayoutPlan, Error> {
    let page = sheet.width();
    let mut plan = Plan {
        page,
        faces,
        cur: Cursor::new(),
        ops: Vec::new(),
        seams: Vec::new(),
        wrap_memo: RefCell::new(HashMap::new()),
    };
    plan.seq(sheet.frames(), 0, page.get(), 0, 0, Advance::ZERO)?;
    if !sheet.notes().is_empty() {
        plan.notes(sheet.notes())?;
    }
    let height = plan.final_height()?;
    if plan.seams.last().map(|r| r.get()) != Some(height) {
        plan.push_seam(Row::new(height))?;
    }
    LayoutPlan::new(page, height, plan.ops, plan.seams)
}

impl Plan<'_> {
    fn seq(
        &mut self,
        frames: &[Frame<'_>],
        x0: u16,
        measure: u16,
        quote_depth: u8,
        list_depth: u8,
        first_ascent: Advance,
    ) -> Result<Outcome, Error> {
        let mut first = Outcome::Empty;
        for frame in frames {
            self.cur
                .bump(extra(&self.cur, rhythm(frame), slug(frame)))?;
            let required = if matches!(first, Outcome::Empty) {
                first_ascent
            } else {
                Advance::ZERO
            };
            let outcome = self.one(frame, x0, measure, quote_depth, list_depth, required)?;
            self.cur.last = Some(rhythm(frame));
            if matches!(first, Outcome::Empty) && !matches!(outcome, Outcome::Empty) {
                first = outcome;
            }
        }
        Ok(first)
    }

    fn one(
        &mut self,
        frame: &Frame<'_>,
        x0: u16,
        measure: u16,
        quote_depth: u8,
        list_depth: u8,
        first_ascent: Advance,
    ) -> Result<Outcome, Error> {
        match frame {
            Frame::Source { location, frame } => self
                .one(frame, x0, measure, quote_depth, list_depth, first_ascent)
                .map_err(|e| location.annotate(e)),
            Frame::Rule(rule) => self.rule(rule),
            Frame::Mark(mark) => self.mark(mark, x0, measure, first_ascent),
            Frame::Text(block) => self.run(block, x0, measure, true, first_ascent),
            Frame::Head(head) => self.head(head, x0, measure, first_ascent),
            Frame::Cols(cols) => self.cols(cols, x0, measure, first_ascent),
            Frame::List(list) => {
                self.list(list, x0, measure, quote_depth, list_depth, first_ascent)
            }
            Frame::Quote(quote) => {
                self.quote(quote, x0, measure, quote_depth, list_depth, first_ascent)
            }
            Frame::Code(code) => self.code(code, x0, measure, first_ascent),
            Frame::Figure(fig) => self.figure(fig, x0, measure),
            Frame::Math(math) => self.math(math, x0, measure),
        }
    }

    fn rule(&mut self, rule: &Rule) -> Result<Outcome, Error> {
        let RuleSpan::Tape = rule.span;
        let y = row_of(self.cur.slug_bottom())?;
        let thick = u32::from(rule.thickness.dots());
        let end = y
            .get()
            .checked_add(thick)
            .ok_or(Error::CoordinateOverflow)?;
        let rows = RowRange::new(y, Row::new(end)).ok_or(Error::CoordinateOverflow)?;
        self.push_op(DrawOp::Rule {
            rows,
            clip: Clip::full(self.page),
        })?;
        self.cur.set_rule(advance_from_row(Row::new(end))?);
        self.record_slug_seam()?;
        Ok(Outcome::BoxTop {
            top: advance_from_row(y)?,
        })
    }

    fn mark(
        &mut self,
        mark: &crate::frame::Mark<'_>,
        x0: u16,
        measure: u16,
        first_ascent: Advance,
    ) -> Result<Outcome, Error> {
        let local = dots_measure(measure)?;
        let plans = wrap_display(
            mark.cut,
            mark.size,
            mark.tracking.0,
            mark.text.as_ref(),
            local,
            self.faces,
        )?;
        let skip = mark.size.skip_dots();
        self.cur.mark_slug = skip;
        let clip = clip_span(self.page, x0, add_u16(x0, measure)?)?;
        let measure_a = dots(measure.max(1))?;
        let mut first = None;
        for line in &plans.lines {
            let ascent = if first.is_none() {
                adv_max(plan_ascent(line), first_ascent)
            } else {
                plan_ascent(line)
            };
            let depth = plan_depth(line);
            let line_skip = display_skip(skip, line)?;
            let b = self.cur.baseline(ascent, depth, line_skip)?;
            if first.is_none() {
                first = Some(b);
            }
            let line_w = match line {
                LinePlan::Ink { advance, .. } => *advance,
                LinePlan::Blank { .. } => Advance::ZERO,
            };
            let x = match mark.align {
                MarkAlign::Start => dots(x0)?,
                MarkAlign::Center => {
                    let slack = match measure_a.checked_sub(line_w) {
                        Some(s) if s.units() > 0 => s,
                        _ => Advance::ZERO,
                    };
                    add(dots(x0)?, Advance::from_units(slack.units() / 2))?
                }
            };
            self.place_line(line, x, b, clip)?;
            self.record_slug_seam()?;
        }
        Ok(
            first.map_or(Outcome::Empty, |first_anchor| Outcome::Occupied {
                first_anchor,
            }),
        )
    }

    fn head(
        &mut self,
        head: &Head<'_>,
        x0: u16,
        measure: u16,
        first_ascent: Advance,
    ) -> Result<Outcome, Error> {
        let block = TextBlock::plain(Cut::Bold, head.size, head.text.as_ref());
        self.run(&block, x0, measure, false, first_ascent)
    }

    fn run(
        &mut self,
        block: &TextBlock<'_>,
        x0: u16,
        measure: u16,
        split: bool,
        first_ascent: Advance,
    ) -> Result<Outcome, Error> {
        let local = dots_measure(measure)?;
        let plans = wrap_text(
            block.size,
            &block.spans,
            local,
            self.faces,
            DigitMode::Proportional,
        )?;
        let clip = clip_span(self.page, x0, add_u16(x0, measure)?)?;
        self.place_lines(&plans, block.size, dots(x0)?, clip, split, first_ascent)
    }

    fn quote(
        &mut self,
        quote: &Quote<'_>,
        x0: u16,
        measure: u16,
        quote_depth: u8,
        list_depth: u8,
        first_ascent: Advance,
    ) -> Result<Outcome, Error> {
        if quote_depth >= NEST_CAP {
            return Err(Error::Nesting);
        }
        self.cur.last = Some(Rhythm::Inner);
        let hang = QUOTE_HANG[usize::from(quote_depth)];
        let x = add_u16(x0, hang)?;
        let w = sub_measure(measure, hang)?;
        self.seq(
            &quote.frames,
            x,
            w,
            quote_depth + 1,
            list_depth,
            first_ascent,
        )
    }

    fn code(
        &mut self,
        code: &Code<'_>,
        x0: u16,
        measure: u16,
        first_ascent: Advance,
    ) -> Result<Outcome, Error> {
        let face = self.faces.text(Cut::Mono)?;
        let skip = code.size().skip_dots();
        let x = dots(add_u16(x0, GRID)?)?;
        let clip = clip_span(self.page, x0, add_u16(x0, measure)?)?;
        let empty = std::borrow::Cow::Borrowed("");
        let lines: Vec<&std::borrow::Cow<'_, str>> = if code.lines().is_empty() {
            vec![&empty]
        } else {
            code.lines().iter().collect()
        };
        let mut first = None;
        for line in lines {
            let run = face.shape_run(line.as_ref(), code.size())?;
            let ascent = if first.is_none() {
                adv_max(run.ascent(), first_ascent)
            } else {
                run.ascent()
            };
            let b = self.cur.baseline(ascent, Advance::ZERO, skip)?;
            if first.is_none() {
                first = Some(b);
            }
            self.push_glyph(run, x, b, clip)
                .map_err(|e| Error::InText {
                    text: line.to_string(),
                    cause: Box::new(e),
                })?;
            self.record_slug_seam()?;
        }
        Ok(
            first.map_or(Outcome::Empty, |first_anchor| Outcome::Occupied {
                first_anchor,
            }),
        )
    }

    fn figure(&mut self, fig: &Figure, x0: u16, measure: u16) -> Result<Outcome, Error> {
        let local = dots_measure(measure)?;
        let fitted = fig.fit(local)?;
        self.place_fitted_box(fitted, x0, measure, fig.note())
    }

    fn math(&mut self, math: &Math, x0: u16, measure: u16) -> Result<Outcome, Error> {
        let local = dots_measure(measure)?;
        let fitted = math.fit(local)?;
        self.place_fitted_box(fitted.raster().clone(), x0, measure, None)
    }

    fn place_fitted_box(
        &mut self,
        raster: Raster,
        x0: u16,
        measure: u16,
        note: Option<std::num::NonZeroU32>,
    ) -> Result<Outcome, Error> {
        let b = self.cur.baseline(Advance::ZERO, Advance::ZERO, GRID)?;
        let w = raster.width();
        let h = raster.height();
        let slack = measure.saturating_sub(w);
        let x = i32::from(x0) + i32::from(slack / 2);
        let clip = clip_span(self.page, x0, add_u16(x0, measure)?)?;
        let top_a = b;
        self.push_bitmap(raster, x, top_a, clip)?;
        let top = row_round(top_a)?;
        let image_bottom = top.get().checked_add(h).ok_or(Error::CoordinateOverflow)?;
        let mut occupied_bottom =
            Advance::from_dots(i32::try_from(image_bottom).map_err(|_| Error::CoordinateOverflow)?)
                .ok_or(Error::CoordinateOverflow)?;
        if let Some(n) = note {
            let note_bottom = self.figure_note(n, x, w, top_a, clip)?;
            if note_bottom.units() > occupied_bottom.units() {
                occupied_bottom = note_bottom;
            }
        }
        let bottom = occupied_bottom.ceil_dots()?;
        let bottom = u32::try_from(bottom.max(0)).map_err(|_| Error::CoordinateOverflow)?;
        let bottom = bottom.max(image_bottom);
        self.grid_seams_through(top, Row::new(bottom))?;
        let bottom_a =
            Advance::from_dots(i32::try_from(bottom).map_err(|_| Error::CoordinateOverflow)?)
                .ok_or(Error::CoordinateOverflow)?;
        self.cur.after_box(bottom_a, GRID)?;
        self.record_slug_seam()?;
        Ok(Outcome::BoxTop { top: b })
    }

    fn figure_note(
        &mut self,
        n: std::num::NonZeroU32,
        fig_x: i32,
        fig_w: u16,
        top: Advance,
        clip: Clip,
    ) -> Result<Advance, Error> {
        let face = self.faces.text(Cut::Roman)?;
        let raise = note_raise_adv(TextSize::Pt8)?;
        let run = face
            .shape_run(&n.get().to_string(), TextSize::Pt8)?
            .translate(Advance::ZERO, raise)?;
        let ppem = TextSize::Pt8.ppem();
        let mut nx = Advance::from_dots(fig_x)
            .ok_or(Error::CoordinateOverflow)?
            .checked_add(dots(fig_w)?)
            .ok_or(Error::CoordinateOverflow)?;
        let right = dots(self.page.get())?;
        let note_w = run.advance();
        if add(nx, note_w)?.units() > right.units() {
            nx = adv_max(
                sub(right, note_w)?,
                Advance::from_dots(fig_x).ok_or(Error::CoordinateOverflow)?,
            );
        }
        let baseline = add(top, dots(ppem)?)?;
        let ink_bottom = if run.ink().is_empty() {
            baseline
        } else {
            add(baseline, run.ink().y1())?
        };
        self.push_glyph(run, nx, baseline, clip)?;
        Ok(ink_bottom)
    }

    fn notes(&mut self, notes: &[Note<'_>]) -> Result<(), Error> {
        let air = pt_dots(2.0);
        let y = row_of(add(self.cur.slug_bottom(), dots(air)?)?)?;
        let x1 = NOTE_RULE.min(self.page.get());
        let end = y.get().checked_add(1).ok_or(Error::CoordinateOverflow)?;
        let rows = RowRange::new(y, Row::new(end)).ok_or(Error::CoordinateOverflow)?;
        self.push_op(DrawOp::Rule {
            rows,
            clip: Clip::new(0, x1).ok_or(Error::CoordinateOverflow)?,
        })?;
        self.cur.set_rule(advance_from_row(Row::new(end))?);
        self.record_slug_seam()?;
        self.cur.bump(air)?;
        let size = TextSize::Pt8;
        let hang_list = List {
            size,
            cut: Cut::Roman,
            marker: Marker::Decimal {
                start: 1,
                delim: DecimalDelim::Period,
            },
            fit: ListFit::Tight,
            items: notes
                .iter()
                .map(|_| crate::frame::ListItem::new(vec![]))
                .collect(),
        };
        let face = self.faces.text(Cut::Roman)?;
        let hang = hang_list.hang_dots(face)?;
        let mark_w = hang_list.mark_width(face)?;
        let content_x = hang;
        let content_w = sub_measure(self.page.get(), hang)?;
        let cap = face.shape_run("H", size)?.ascent();
        let clip = Clip::full(self.page);
        for (i, note) in notes.iter().enumerate() {
            self.cur.last = Some(Rhythm::Inner);
            let outcome = match note {
                Note::Dest {
                    dest,
                    title,
                    location,
                } => {
                    let body = match title {
                        Some(t) => format!("{t}\n{dest}"),
                        None => dest.to_string(),
                    };
                    let block = TextBlock::plain(Cut::Roman, size, body);
                    self.run(&block, content_x, content_w, true, cap).map_err(
                        |e| match location {
                            Some(location) => location.annotate(e),
                            None => e,
                        },
                    )?
                }
                // Apparatus indent is geometry. Source depth starts at zero.
                Note::Blocks(frames) => self.seq(frames, content_x, content_w, 0, 0, cap)?,
            };
            let anchor = self.mark_anchor(outcome, cap, size.skip_dots())?;
            let t = decimal_text(1 + i as u32, DecimalDelim::Period);
            let run = face.shape_figure_run(&t, size)?;
            let mx = match mark_w.checked_sub(run.advance()) {
                Some(s) if s.units() > 0 => s,
                _ => Advance::ZERO,
            };
            self.push_glyph(run, mx, anchor, clip)?;
            self.occupy_mark(anchor, cap, size.skip_dots())?;
        }
        Ok(())
    }

    fn list(
        &mut self,
        list: &List<'_>,
        x0: u16,
        measure: u16,
        quote_depth: u8,
        list_depth: u8,
        first_ascent: Advance,
    ) -> Result<Outcome, Error> {
        if list_depth >= NEST_CAP {
            return Err(Error::Nesting);
        }
        let face = self.faces.text(list.cut)?;
        let hang = list.hang_dots(face)?;
        let mark_w = list.mark_width(face)?;
        let content_x = add_u16(x0, hang)?;
        let content_w = sub_measure(measure, hang)?;
        let cap = face.shape_run("H", list.size)?.ascent();
        let mut first = None;
        for (i, item) in list.items.iter().enumerate() {
            if i > 0 && list.fit == ListFit::Loose {
                self.cur.bump(list.size.skip_dots())?;
            }
            self.cur.last = Some(Rhythm::Inner);
            // Ancestor requirements belong to this first item, not mutable cursor state.
            let required = if i == 0 {
                adv_max(cap, first_ascent)
            } else {
                cap
            };
            let outcome = match &item.body {
                ItemBody::Blank => Outcome::Empty,
                ItemBody::Frames(frames) => self.seq(
                    frames.as_slice(),
                    content_x,
                    content_w,
                    quote_depth,
                    list_depth + 1,
                    required,
                )?,
            };
            let anchor = self.mark_anchor(outcome, required, list.size.skip_dots())?;
            if first.is_none() {
                first = Some(anchor);
            }
            match item.mark {
                ItemMark::Task { checked } => {
                    self.place_task(x0, anchor, cap, checked)?;
                }
                ItemMark::List => {
                    let (mx, run) = match list.marker {
                        Marker::Dash => {
                            let run = face.shape_run(EN_DASH, list.size)?;
                            let bearing = Advance::from_units(run.ink().x0().units().min(0));
                            (sub(dots(x0)?, bearing)?, run)
                        }
                        Marker::Decimal { start, delim } => {
                            let index = u32::try_from(i).map_err(|_| Error::CoordinateOverflow)?;
                            let t = decimal_text(
                                start.checked_add(index).ok_or(Error::CoordinateOverflow)?,
                                delim,
                            );
                            let s = face.shape_figure_run(&t, list.size)?;
                            let x = {
                                let raw = add(dots(x0)?, mark_w)?.checked_sub(s.advance());
                                match raw {
                                    Some(x) if x.units() >= dots(x0)?.units() => x,
                                    _ => dots(x0)?,
                                }
                            };
                            (x, s)
                        }
                    };
                    let clip = clip_span(self.page, x0, add_u16(x0, measure)?)?;
                    self.push_glyph(run, mx, anchor, clip)?;
                }
            }
            self.occupy_mark(anchor, cap, list.size.skip_dots())?;
            self.record_slug_seam()?;
        }
        Ok(
            first.map_or(Outcome::Empty, |first_anchor| Outcome::Occupied {
                first_anchor,
            }),
        )
    }

    fn cols(
        &mut self,
        cols: &Cols<'_>,
        x0: u16,
        measure: u16,
        first_ascent: Advance,
    ) -> Result<Outcome, Error> {
        let result = match &cols.body {
            ColBody::Two { align, rows } => self.grid(cols, align, rows, x0, measure, first_ascent),
            ColBody::Three { align, rows } => {
                self.grid(cols, align, rows, x0, measure, first_ascent)
            }
        };
        self.wrap_memo.get_mut().clear();
        result
    }

    fn grid<const N: usize>(
        &mut self,
        cols: &Cols<'_>,
        align: &[ColAlign; N],
        rows: &[[Vec<crate::frame::Span<'_>>; N]],
        x0: u16,
        measure: u16,
        first_ascent: Advance,
    ) -> Result<Outcome, Error> {
        if rows.is_empty() {
            return Ok(Outcome::Empty);
        }
        let size = cols.size;
        let gutter = cols.gutter.dots();
        let natural = self.measure_cols(size, align, rows)?;
        let placed = cols::layout(natural, x0, measure, gutter, |w| {
            self.allocation_cost(size, align, rows, w)
        })?;
        let mut first = None;
        for row in rows {
            let mut cell_plans = Vec::with_capacity(N);
            for (cell, spans) in placed.col.iter().zip(row.iter()) {
                let m = Measure::new(cell.width).ok_or(Error::UnsupportedMeasure)?;
                cell_plans.push(self.wrap_cell(size, spans, m, col_digits(cell.align))?);
            }
            let nlines = cell_plans
                .iter()
                .map(|p| p.lines.len().max(1))
                .max()
                .unwrap_or(1);
            for li in 0..nlines {
                let mut ascent = Advance::ZERO;
                let mut depth = Advance::ZERO;
                let mut skip = size.skip_dots();
                for plans in &cell_plans {
                    if let Some(line) = plans.lines.get(li) {
                        let (a, d, s) = line_metrics(line, size)?;
                        if a.units() > ascent.units() {
                            ascent = a;
                        }
                        if d.units() > depth.units() {
                            depth = d;
                        }
                        skip = skip.max(s);
                    }
                }
                let ascent = if first.is_none() {
                    adv_max(ascent, first_ascent)
                } else {
                    ascent
                };
                let b = self.cur.baseline(ascent, depth, skip)?;
                if first.is_none() {
                    first = Some(b);
                }
                for (cell, plans) in placed.col.iter().zip(&cell_plans) {
                    if let Some(line) = plans.lines.get(li) {
                        let line_w = match line {
                            LinePlan::Ink { advance, .. } => *advance,
                            LinePlan::Blank { .. } => Advance::ZERO,
                        };
                        let x = cell.ink_x(line_w)?;
                        let clip = clip_span(self.page, cell.origin, cell.end()?)?;
                        self.place_line(line, x, b, clip)?;
                    }
                }
            }
            self.record_slug_seam()?;
        }
        Ok(
            first.map_or(Outcome::Empty, |first_anchor| Outcome::Occupied {
                first_anchor,
            }),
        )
    }

    fn wrap_cell(
        &self,
        size: TextSize,
        spans: &[crate::frame::Span<'_>],
        measure: Measure,
        digits: DigitMode,
    ) -> Result<Arc<LinePlans>, Error> {
        let key = (
            spans.as_ptr() as usize,
            spans.len(),
            measure.get(),
            size.ppem(),
            match digits {
                DigitMode::Proportional => 0,
                DigitMode::Tabular => 1,
            },
        );
        if let Some(hit) = self.wrap_memo.borrow().get(&key) {
            return Ok(hit.clone());
        }
        let plans = Arc::new(wrap_text(size, spans, measure, self.faces, digits)?);
        self.wrap_memo.borrow_mut().insert(key, plans.clone());
        Ok(plans)
    }

    fn measure_cols<const N: usize>(
        &self,
        size: TextSize,
        align: &[ColAlign; N],
        rows: &[[Vec<crate::frame::Span<'_>>; N]],
    ) -> Result<Natural<N>, Error> {
        let tape = Measure::TAPE;
        let tight = Measure::new(1).ok_or(Error::UnsupportedMeasure)?;
        let mut pref = [1u64; N];
        let mut min = [1u64; N];
        for c in 0..N {
            let digits = col_digits(align[c]);
            let mut p = 1u64;
            let mut m = 1u64;
            for row in rows {
                let natural = natural_advance(size, &row[c], tape, self.faces, digits)?;
                let dots = natural.ceil_dots()?.max(1);
                let dots = u64::try_from(dots).map_err(|_| Error::CoordinateOverflow)?;
                p = p.max(dots);
                let words = self.wrap_cell(size, &row[c], tight, digits)?;
                m = m.max(max_line_width(&words)?);
            }
            pref[c] = p.max(1);
            min[c] = m.min(pref[c]).max(1);
        }
        Natural::new(*align, min, pref)
    }

    fn allocation_cost<const N: usize>(
        &self,
        size: TextSize,
        align: &[ColAlign; N],
        rows: &[[Vec<crate::frame::Span<'_>>; N]],
        widths: &[u16; N],
    ) -> Result<WrapCost, Error> {
        let mut total = WrapCost::default();
        for (c, w) in widths.iter().enumerate() {
            let m = Measure::new(*w).ok_or(Error::UnsupportedMeasure)?;
            let digits = col_digits(align[c]);
            for row in rows {
                let plans = self.wrap_cell(size, &row[c], m, digits)?;
                total = total
                    .checked_add(plans.cost)
                    .ok_or(Error::CoordinateOverflow)?;
            }
        }
        Ok(total)
    }

    fn place_lines(
        &mut self,
        plans: &LinePlans,
        size: TextSize,
        x0: Advance,
        clip: Clip,
        split: bool,
        first_ascent: Advance,
    ) -> Result<Outcome, Error> {
        let mut first = None;
        for line in &plans.lines {
            let (ascent, depth, skip) = line_metrics(line, size)?;
            let ascent = if first.is_none() {
                adv_max(ascent, first_ascent)
            } else {
                ascent
            };
            let b = self.cur.baseline(ascent, depth, skip)?;
            if first.is_none() {
                first = Some(b);
            }
            self.place_line(line, x0, b, clip)?;
            if split {
                self.record_slug_seam()?;
            }
        }
        Ok(
            first.map_or(Outcome::Empty, |first_anchor| Outcome::Occupied {
                first_anchor,
            }),
        )
    }

    fn place_line(
        &mut self,
        line: &LinePlan,
        x0: Advance,
        baseline: Advance,
        clip: Clip,
    ) -> Result<(), Error> {
        let LinePlan::Ink { atoms, bounds, .. } = line else {
            return Ok(());
        };
        // The measure bounds ink, not the font's pen origin. Keep a negative
        // leading sidebearing without shaving its leftmost pixels.
        let left = add(x0, bounds.x0())?;
        let x0 = if left < dots(clip.start())? {
            add(x0, sub(dots(clip.start())?, left)?)?
        } else {
            x0
        };
        for atom in atoms {
            match atom {
                PaintAtom::Rule { bounds } => {
                    let start = add(x0, bounds.x0())?
                        .round_dots()?
                        .max(i32::from(clip.start()));
                    let end = add(x0, bounds.x1())?
                        .round_dots()?
                        .min(i32::from(clip.end()));
                    if start < end {
                        let clip = Clip::new(
                            u16::try_from(start).map_err(|_| Error::CoordinateOverflow)?,
                            u16::try_from(end).map_err(|_| Error::CoordinateOverflow)?,
                        )
                        .ok_or(Error::CoordinateOverflow)?;
                        let rows = RowRange::new(
                            row_round(add(baseline, bounds.y0())?)?,
                            row_round(add(baseline, bounds.y1())?)?,
                        )
                        .ok_or(Error::CoordinateOverflow)?;
                        self.push_op(DrawOp::Rule { rows, clip })?;
                    }
                }
                PaintAtom::Glyph { run, x, .. } => {
                    self.push_op(DrawOp::GlyphRun {
                        run: (**run).clone(),
                        x: add(x0, *x)?,
                        baseline,
                        clip,
                    })?;
                }
                PaintAtom::Bitmap { raster, x, bounds } => {
                    let x = add(x0, *x)?.round_dots()?;
                    let top = add(baseline, bounds.y0())?;
                    self.push_bitmap(raster.clone(), x, top, clip)?;
                }
            }
        }
        Ok(())
    }

    fn blank_slug(&mut self, ascent: Advance, skip: u16) -> Result<Advance, Error> {
        self.cur.baseline(ascent, Advance::ZERO, skip)
    }

    fn mark_anchor(&mut self, outcome: Outcome, cap: Advance, skip: u16) -> Result<Advance, Error> {
        match outcome {
            Outcome::Empty => self.blank_slug(cap, skip),
            Outcome::Occupied { first_anchor } => Ok(first_anchor),
            Outcome::BoxTop { top } => add(top, cap),
        }
    }

    fn occupy_mark(&mut self, anchor: Advance, cap: Advance, skip: u16) -> Result<(), Error> {
        let top = sub(anchor, cap)?;
        let bottom = add(top, dots(skip)?)?;
        self.cur.cover(adv_max(bottom, anchor));
        Ok(())
    }

    fn place_task(
        &mut self,
        x0: u16,
        baseline: Advance,
        ascent: Advance,
        checked: bool,
    ) -> Result<(), Error> {
        let raster = task_raster(checked)?;
        let half_ascent = Advance::from_units(ascent.units() / 2);
        let center = sub(baseline, half_ascent)?;
        let half_side = dots(TASK_BOX / 2)?;
        let top = sub(center, half_side)?;
        let clip = clip_span(self.page, x0, add_u16(x0, TASK_BOX)?)?;
        self.push_bitmap(raster, i32::from(x0), top, clip)
    }

    fn push_glyph(
        &mut self,
        run: ShapedRun,
        x: Advance,
        baseline: Advance,
        clip: Clip,
    ) -> Result<(), Error> {
        self.push_op(DrawOp::GlyphRun {
            run,
            x,
            baseline,
            clip,
        })
    }

    fn push_bitmap(
        &mut self,
        raster: Raster,
        x: i32,
        top: Advance,
        clip: Clip,
    ) -> Result<(), Error> {
        let (raster, top) = on_page_bitmap(raster, top)?;
        self.push_op(DrawOp::Bitmap {
            raster,
            x,
            top,
            clip,
        })
    }

    fn push_op(&mut self, op: DrawOp) -> Result<(), Error> {
        let extent = match &op {
            DrawOp::GlyphRun { run, x, clip, .. } if !run.ink().is_empty() => Some((
                add(*x, run.ink().x0())?.round_dots()?,
                add(*x, run.ink().x1())?.round_dots()?,
                *clip,
            )),
            DrawOp::Bitmap {
                raster, x, clip, ..
            } => Some((
                *x,
                x.checked_add(i32::from(raster.width()))
                    .ok_or(Error::CoordinateOverflow)?,
                *clip,
            )),
            _ => None,
        };
        if let Some((left, right, clip)) = extent
            && (left < i32::from(clip.start()) || right > i32::from(clip.end()))
        {
            return Err(Error::Clipped {
                text: match &op {
                    DrawOp::GlyphRun { run, .. } => Some(Arc::clone(run.text())),
                    _ => None,
                },
                left,
                right,
                available_start: clip.start(),
                available_end: clip.end(),
            });
        }
        self.ops.try_reserve(1).map_err(|_| Error::Alloc)?;
        self.ops.push(op);
        Ok(())
    }

    fn push_seam(&mut self, row: Row) -> Result<(), Error> {
        if row.get() == 0 {
            return Ok(());
        }
        self.seams.try_reserve(1).map_err(|_| Error::Alloc)?;
        self.seams.push(row);
        Ok(())
    }

    fn record_slug_seam(&mut self) -> Result<(), Error> {
        self.push_seam(row_of(self.cur.slug_bottom())?)
    }

    fn grid_seams_through(&mut self, top: Row, bottom: Row) -> Result<(), Error> {
        let cap = u32::from(max_height(self.page.get()).max(1));
        if bottom.get().saturating_sub(top.get()) <= cap {
            return Ok(());
        }
        let mut y = top.get().saturating_add(u32::from(GRID));
        while y < bottom.get() {
            self.push_seam(Row::new(y))?;
            let next = y.saturating_add(u32::from(GRID));
            if next <= y {
                break;
            }
            y = next;
        }
        Ok(())
    }

    fn final_height(&self) -> Result<u32, Error> {
        let slug = row_of(self.cur.slug_bottom())?.get();
        Ok(slug.max(1))
    }
}

fn col_digits(align: ColAlign) -> DigitMode {
    match align {
        ColAlign::End => DigitMode::Tabular,
        ColAlign::Start => DigitMode::Proportional,
    }
}

fn line_metrics(line: &LinePlan, size: TextSize) -> Result<(Advance, Advance, u16), Error> {
    Ok((plan_ascent(line), plan_depth(line), plan_skip(size, line)?))
}

fn display_skip(solid: u16, line: &LinePlan) -> Result<u16, Error> {
    match line {
        LinePlan::Blank { slug } => {
            let dots =
                u16::try_from(slug.ceil_dots()?.max(0)).map_err(|_| Error::CoordinateOverflow)?;
            Ok(solid.max(dots))
        }
        LinePlan::Ink { .. } => {
            let span = add(plan_ascent(line), plan_depth(line))?;
            let ink =
                u16::try_from(span.ceil_dots()?.max(0)).map_err(|_| Error::CoordinateOverflow)?;
            Ok(solid.max(ink))
        }
    }
}

fn max_line_width(plans: &LinePlans) -> Result<u64, Error> {
    let dots = max_line_advance(plans)?.ceil_dots()?.max(0);
    let dots = u64::try_from(dots).map_err(|_| Error::CoordinateOverflow)?;
    Ok(dots.max(1))
}

fn on_page_bitmap(raster: Raster, top: Advance) -> Result<(Raster, Row), Error> {
    let dots = top.round_dots()?;
    if dots >= 0 {
        let row = u32::try_from(dots).map_err(|_| Error::CoordinateOverflow)?;
        return Ok((raster, Row::new(row)));
    }
    let skip = u32::try_from(-dots).map_err(|_| Error::CoordinateOverflow)?;
    if skip >= raster.height() {
        return Err(Error::CoordinateOverflow);
    }
    Ok((raster.slice_rows(skip..raster.height())?, Row::ZERO))
}

fn task_raster(checked: bool) -> Result<Raster, Error> {
    let side = TASK_BOX;
    let n = usize::from(side) * usize::from(side);
    let mut bits = vec![false; n];
    let set = |bits: &mut [bool], x: i32, y: i32| {
        if x >= 0 && y >= 0 {
            let x = x as u16;
            let y = y as u16;
            if x < side && y < side {
                bits[usize::from(y) * usize::from(side) + usize::from(x)] = true;
            }
        }
    };
    let stroke = 2i32;
    let side_i = i32::from(side);
    for t in 0..stroke {
        for x in 0..side_i {
            set(&mut bits, x, t);
            set(&mut bits, x, side_i - 1 - t);
        }
        for y in 0..side_i {
            set(&mut bits, t, y);
            set(&mut bits, side_i - 1 - t, y);
        }
    }
    if checked {
        let inset = stroke * 2;
        stroke_bits(
            &mut bits,
            side,
            inset,
            side_i / 2,
            side_i / 2,
            side_i - inset,
            stroke,
        );
        stroke_bits(
            &mut bits,
            side,
            side_i / 2,
            side_i - inset,
            side_i - inset,
            inset,
            stroke,
        );
    }
    Raster::from_bits(side, u32::from(side), &bits).map_err(Error::from)
}

fn stroke_bits(bits: &mut [bool], side: u16, x0: i32, y0: i32, x1: i32, y1: i32, thick: i32) {
    let set = |bits: &mut [bool], x: i32, y: i32| {
        if x >= 0 && y >= 0 {
            let x = x as u16;
            let y = y as u16;
            if x < side && y < side {
                bits[usize::from(y) * usize::from(side) + usize::from(x)] = true;
            }
        }
    };
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
                set(bits, x + ox, y + oy);
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

fn note_raise_adv(size: TextSize) -> Result<Advance, Error> {
    let units = i64::from(size.ppem())
        .checked_mul(i64::from(NOTE_RAISE_NUM))
        .and_then(|v| v.checked_mul(i64::from(crate::size::FRAC)))
        .ok_or(Error::CoordinateOverflow)?
        / i64::from(NOTE_RAISE_DEN);
    Ok(Advance::from_units(units))
}

fn dots(n: u16) -> Result<Advance, Error> {
    Advance::from_dots(i32::from(n)).ok_or(Error::CoordinateOverflow)
}

fn add(a: Advance, b: Advance) -> Result<Advance, Error> {
    a.checked_add(b).ok_or(Error::CoordinateOverflow)
}

fn sub(a: Advance, b: Advance) -> Result<Advance, Error> {
    a.checked_sub(b).ok_or(Error::CoordinateOverflow)
}

fn adv_max(a: Advance, b: Advance) -> Advance {
    if a.units() >= b.units() { a } else { b }
}

fn add_u16(a: u16, b: u16) -> Result<u16, Error> {
    a.checked_add(b).ok_or(Error::CoordinateOverflow)
}

fn sub_measure(measure: u16, hang: u16) -> Result<u16, Error> {
    match measure.checked_sub(hang) {
        Some(w) if w >= 1 => Ok(w),
        _ => Err(Error::CoordinateOverflow),
    }
}

fn dots_measure(dots: u16) -> Result<Measure, Error> {
    Measure::new(dots.max(1)).ok_or(Error::UnsupportedMeasure)
}

fn clip_span(page: Measure, start: u16, end: u16) -> Result<Clip, Error> {
    let clip = Clip::new(start, end).ok_or(Error::CoordinateOverflow)?;
    Ok(clip.intersection(Clip::full(page)))
}

fn row_of(a: Advance) -> Result<Row, Error> {
    let dots = a.ceil_dots()?;
    if dots < 0 {
        return Err(Error::CoordinateOverflow);
    }
    Ok(Row::new(
        u32::try_from(dots).map_err(|_| Error::CoordinateOverflow)?,
    ))
}

fn row_round(a: Advance) -> Result<Row, Error> {
    let dots = a.round_dots()?;
    if dots < 0 {
        return Err(Error::CoordinateOverflow);
    }
    Ok(Row::new(
        u32::try_from(dots).map_err(|_| Error::CoordinateOverflow)?,
    ))
}

fn advance_from_row(row: Row) -> Result<Advance, Error> {
    let dots = i32::try_from(row.get()).map_err(|_| Error::CoordinateOverflow)?;
    Advance::from_dots(dots).ok_or(Error::CoordinateOverflow)
}

#[cfg(test)]
mod tests {
    use super::{Outcome, task_raster};
    use crate::leading::TASK_BOX;
    use tm20::Raster;

    #[test]
    fn empty_outcome_is_distinct_from_occupied() {
        assert!(matches!(Outcome::Empty, Outcome::Empty));
        assert!(!matches!(
            Outcome::Occupied {
                first_anchor: crate::geometry::Advance::ZERO
            },
            Outcome::Empty
        ));
        assert!(!matches!(
            Outcome::BoxTop {
                top: crate::geometry::Advance::ZERO
            },
            Outcome::Empty
        ));
    }

    #[test]
    fn task_raster_is_a_checked_square() {
        let open = task_raster(false).unwrap();
        let checked = task_raster(true).unwrap();
        assert_eq!(open.width(), TASK_BOX);
        assert_eq!(open.height(), u32::from(TASK_BOX));
        assert_eq!(checked.width(), TASK_BOX);
        assert!(open.pixels().iter().any(|&b| b != 0));
        assert_ne!(open.pixels(), checked.pixels());
        let _ = Raster::from_packed(open.width(), open.height(), open.pixels().to_vec()).unwrap();
    }
}
