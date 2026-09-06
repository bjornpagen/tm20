//! Cursor, rhythm classes, and pair spacing. Page motion is [`Advance`], not `f32`.

use crate::error::Error;
use crate::frame::Frame;
use crate::geometry::Advance;
use crate::leading::{GRID, HANG};
use crate::size::TextSize;

pub(crate) use super::{NEST_CAP, NOTE_RAISE_DEN, NOTE_RAISE_NUM};

/// Quote hang per depth: two modules for the first voice so it reads on
/// 80 mm paper, one more per nest. Indexed by the depth the guard just checked.
pub(crate) const QUOTE_HANG: [u16; NEST_CAP as usize] = [2 * GRID, GRID, GRID];

/// Last completed frame’s adjacency class. Hang is not a tag: list, quote,
/// and code are distinct, and [`Rhythm::Inner`] is the same-list interior.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rhythm {
    Mark,
    Head,
    Prose,
    List,
    Quote,
    Code,
    Cols,
    Rule,
    /// Same-list / hang-interior. Children stick; this is not a sibling hang.
    Inner,
}

/// Why two frames meet. Extra is a function of this, not of two Hang tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pair {
    Stick,
    Line,
    Seam,
    AfterMark,
}

#[derive(Clone, Copy)]
pub(crate) enum Place {
    Origin {
        floor: Advance,
        slug_bottom: Advance,
    },
    Line {
        baseline: Advance,
        slug_bottom: Advance,
    },
    Rule {
        bottom: Advance,
    },
}

pub(crate) struct Cursor {
    pub(crate) place: Place,
    pub(crate) last: Option<Rhythm>,
    pub(crate) mark_slug: u16,
}

impl Cursor {
    pub(crate) fn new() -> Self {
        Self {
            place: Place::Origin {
                floor: Advance::ZERO,
                slug_bottom: Advance::ZERO,
            },
            last: None,
            mark_slug: 0,
        }
    }

    pub(crate) fn slug_bottom(&self) -> Advance {
        match self.place {
            Place::Origin { slug_bottom, .. } | Place::Line { slug_bottom, .. } => slug_bottom,
            Place::Rule { bottom } => bottom,
        }
    }

    /// Air raises the whole slug, not just the pen: a following rule reads
    /// `slug_bottom`, a following line reads `baseline`; both must move.
    pub(crate) fn bump(&mut self, extra: u16) -> Result<(), Error> {
        if extra == 0 {
            return Ok(());
        }
        let extra = dots(extra)?;
        self.place = match self.place {
            Place::Line {
                baseline,
                slug_bottom,
            } => Place::Line {
                baseline: add(baseline, extra)?,
                slug_bottom: add(slug_bottom, extra)?,
            },
            Place::Origin { floor, slug_bottom } => Place::Origin {
                floor: add(floor, extra)?,
                slug_bottom,
            },
            Place::Rule { bottom } => Place::Origin {
                floor: add(bottom, extra)?,
                slug_bottom: bottom,
            },
        };
        Ok(())
    }

    /// Pen for the next line. [`Place`] is the boundary sentinel — Origin and
    /// Rule are frame starts — so one method serves the first line and every
    /// wrapped line after it. No caller counts lines.
    pub(crate) fn baseline(
        &mut self,
        ascent: Advance,
        depth: Advance,
        skip: u16,
    ) -> Result<Advance, Error> {
        let skip = dots(skip)?;
        let hang = dots(HANG)?;
        let b = match self.place {
            Place::Rule { bottom } => add(add(bottom, hang)?, ascent)?,
            Place::Origin { floor, .. } => add(floor, ascent)?,
            Place::Line {
                baseline,
                slug_bottom,
            } => adv_max(add(baseline, skip)?, add(slug_bottom, ascent)?),
        };
        let from_skip = add(sub(b, ascent)?, skip)?;
        let from_depth = add(b, depth)?;
        let slug_bottom = adv_max(self.slug_bottom(), adv_max(from_skip, from_depth));
        self.place = Place::Line {
            baseline: b,
            slug_bottom,
        };
        Ok(b)
    }

    pub(crate) fn set_rule(&mut self, bottom: Advance) {
        self.place = Place::Rule { bottom };
    }

    /// Extent of a fitted box: the next slug hangs `pad` under `bottom`.
    pub(crate) fn after_box(&mut self, bottom: Advance, pad: u16) -> Result<(), Error> {
        self.place = Place::Line {
            baseline: bottom,
            slug_bottom: add(bottom, dots(pad)?)?,
        };
        Ok(())
    }

    /// Raise the occupied slug so a hanging marker's line is inside the page.
    pub(crate) fn cover(&mut self, at_least: Advance) {
        match &mut self.place {
            Place::Origin { slug_bottom, .. } | Place::Line { slug_bottom, .. } => {
                if at_least.units() > slug_bottom.units() {
                    *slug_bottom = at_least;
                }
            }
            Place::Rule { bottom } => {
                if at_least.units() > bottom.units() {
                    *bottom = at_least;
                }
            }
        }
    }
}

pub(crate) fn rhythm(frame: &Frame<'_>) -> Rhythm {
    match frame {
        Frame::Source { frame, .. } => rhythm(frame),
        Frame::Mark(_) => Rhythm::Mark,
        Frame::Head(_) => Rhythm::Head,
        Frame::Text(_) | Frame::Figure(_) | Frame::Math(_) => Rhythm::Prose,
        Frame::List(_) => Rhythm::List,
        Frame::Quote(_) => Rhythm::Quote,
        Frame::Code(_) => Rhythm::Code,
        Frame::Cols(_) => Rhythm::Cols,
        Frame::Rule(_) => Rhythm::Rule,
    }
}

pub(crate) fn pair(from: Rhythm, to: Rhythm) -> Pair {
    use Rhythm::{Code, Cols, Head, Inner, List, Mark, Prose, Quote, Rule};
    match (from, to) {
        // A rule kisses the masthead it opens and the table it totals;
        // everywhere else it takes one module of air on each side.
        (Mark | Cols, Rule) => Pair::Stick,
        (_, Rule) => Pair::Seam,
        (Mark, Mark) => Pair::Stick,
        (Mark, _) => Pair::AfterMark,
        (Head, Head) => Pair::Seam,
        (Head, Mark) => Pair::Line,
        (Head, _) => Pair::Stick,
        (Cols, Cols) => Pair::Stick,
        (Cols, _) => Pair::Line,
        (_, Head) => Pair::Line,
        (Inner, Cols | Mark) => Pair::Line,
        (Inner, _) => Pair::Stick,
        (List | Quote | Code, List | Quote | Code) => Pair::Seam,
        (Quote | Code, Prose) => Pair::Seam,
        (Prose, Prose) => Pair::Line,
        (_, Cols | Mark) => Pair::Line,
        (List, Prose) | (Prose, List | Quote | Code) => Pair::Stick,
        (Rule, _) => Pair::Line,
        (Prose | List | Quote | Code, Inner) => Pair::Stick,
    }
}

pub(crate) fn slug(frame: &Frame<'_>) -> u16 {
    match frame {
        Frame::Source { frame, .. } => slug(frame),
        Frame::Text(b) => b.size.skip_dots(),
        Frame::Head(h) => h.size.skip_dots(),
        Frame::List(l) => l.size.skip_dots(),
        Frame::Cols(c) => c.size.skip_dots(),
        Frame::Quote(q) => q.frames.first().map_or(0, slug),
        Frame::Code(c) => c.size().skip_dots(),
        Frame::Mark(m) => m.size.skip_dots(),
        Frame::Figure(_) => GRID,
        Frame::Math(_) => TextSize::Pt11.skip_dots(),
        Frame::Rule(_) => 0,
    }
}

pub(crate) fn extra(cur: &Cursor, to: Rhythm, next: u16) -> u16 {
    match cur.place {
        Place::Origin { .. } => 0,
        // After a rule: a table hangs from it; everything else takes the
        // same module of air the rule took above.
        Place::Rule { .. } => match to {
            Rhythm::Cols => 0,
            _ => GRID,
        },
        Place::Line { .. } => {
            let Some(from) = cur.last else {
                return 0;
            };
            match pair(from, to) {
                Pair::Stick => 0,
                Pair::Line => next,
                Pair::Seam => GRID,
                Pair::AfterMark => cur.mark_slug,
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{Cursor, Pair, Rhythm, adv_max, extra, pair};
    use crate::geometry::Advance;
    use crate::leading::GRID;

    #[test]
    fn new_list_is_a_seam_not_a_hang_collapse() {
        assert_eq!(pair(Rhythm::List, Rhythm::List), Pair::Seam);
        assert_eq!(pair(Rhythm::Quote, Rhythm::Code), Pair::Seam);
        assert_eq!(pair(Rhythm::Quote, Rhythm::Prose), Pair::Seam);
        assert_eq!(pair(Rhythm::Code, Rhythm::Prose), Pair::Seam);
        assert_eq!(pair(Rhythm::Inner, Rhythm::Prose), Pair::Stick);
        assert_eq!(pair(Rhythm::List, Rhythm::Prose), Pair::Stick);
        assert_eq!(pair(Rhythm::Prose, Rhythm::List), Pair::Stick);
        assert_eq!(pair(Rhythm::Prose, Rhythm::Prose), Pair::Line);
    }

    #[test]
    fn rules_and_stacked_heads_breathe_one_module() {
        assert_eq!(pair(Rhythm::Prose, Rhythm::Rule), Pair::Seam);
        assert_eq!(pair(Rhythm::Head, Rhythm::Rule), Pair::Seam);
        assert_eq!(pair(Rhythm::List, Rhythm::Rule), Pair::Seam);
        assert_eq!(pair(Rhythm::Mark, Rhythm::Rule), Pair::Stick);
        assert_eq!(pair(Rhythm::Cols, Rhythm::Rule), Pair::Stick);
        assert_eq!(pair(Rhythm::Head, Rhythm::Head), Pair::Seam);
        assert_eq!(pair(Rhythm::Head, Rhythm::Prose), Pair::Stick);
    }

    #[test]
    fn origin_charges_no_air() {
        let cur = Cursor::new();
        assert_eq!(extra(&cur, Rhythm::Prose, 37), 0);
        assert_eq!(extra(&cur, Rhythm::Cols, 37), 0);
        assert_eq!(cur.slug_bottom(), Advance::ZERO);
    }

    #[test]
    fn rule_then_table_kisses_and_prose_takes_a_module() {
        let mut cur = Cursor::new();
        cur.set_rule(Advance::from_dots(18).unwrap());
        assert_eq!(extra(&cur, Rhythm::Cols, 37), 0);
        assert_eq!(extra(&cur, Rhythm::Prose, 37), GRID);
        assert_eq!(extra(&cur, Rhythm::Head, 37), GRID);
    }

    #[test]
    fn sibling_hangs_are_a_seam_after_a_line() {
        let mut cur = Cursor::new();
        let ascent = Advance::from_dots(20).unwrap();
        cur.baseline(ascent, Advance::ZERO, 37).unwrap();
        cur.last = Some(Rhythm::List);
        assert_eq!(extra(&cur, Rhythm::List, 37), GRID);
        assert_eq!(extra(&cur, Rhythm::Quote, 37), GRID);
        cur.last = Some(Rhythm::Inner);
        assert_eq!(extra(&cur, Rhythm::Prose, 37), 0);
    }

    #[test]
    fn after_mark_uses_the_display_slug() {
        let mut cur = Cursor::new();
        let ascent = Advance::from_dots(40).unwrap();
        cur.baseline(ascent, Advance::ZERO, 51).unwrap();
        cur.last = Some(Rhythm::Mark);
        cur.mark_slug = 51;
        assert_eq!(extra(&cur, Rhythm::Prose, 37), 51);
    }

    #[test]
    fn first_baseline_is_floor_plus_ascent() {
        let mut cur = Cursor::new();
        let ascent = Advance::from_dots(20).unwrap();
        let b = cur.baseline(ascent, Advance::ZERO, 37).unwrap();
        assert_eq!(b, ascent);
        assert_eq!(cur.slug_bottom().ceil_dots().unwrap(), 37);
    }

    #[test]
    fn bump_raises_line_slug_and_baseline_together() {
        let mut cur = Cursor::new();
        let ascent = Advance::from_dots(20).unwrap();
        cur.baseline(ascent, Advance::ZERO, 37).unwrap();
        cur.bump(GRID).unwrap();
        assert_eq!(cur.slug_bottom().ceil_dots().unwrap(), i32::from(37 + GRID));
    }

    #[test]
    fn advance_max_is_unit_order() {
        let a = Advance::from_dots(1).unwrap();
        let b = Advance::from_dots(2).unwrap();
        assert_eq!(adv_max(a, b), b);
        assert_eq!(adv_max(b, a), b);
    }
}
