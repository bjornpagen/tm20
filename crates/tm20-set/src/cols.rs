//! Compact vs Hang. Paint walks boxes; leftover is not re-derived.
//! Adjacent [`ColAlign::End`] columns take a two-module gap; a single-module
//! gutter between two hanging figure columns is not a legal plan.

use crate::compose::plan::WrapCost;
use crate::error::Error;
use crate::frame::ColAlign;
use crate::geometry::Advance;
use crate::leading::{GRID, GridSkip};

/// Unwrapped (`pref`) and longest-word (`min`) widths, with ink alignment.
/// `1 <= min <= pref` is a constructor law, not a later clamp.
#[derive(Clone, Copy)]
pub(crate) struct Natural<const N: usize> {
    pub(crate) align: [ColAlign; N],
    pub(crate) pref: [u64; N],
    pub(crate) min: [u64; N],
}

impl<const N: usize> Natural<N> {
    pub(crate) fn new(align: [ColAlign; N], min: [u64; N], pref: [u64; N]) -> Result<Self, Error> {
        if N < 2 {
            return Err(Error::ImpossibleColumns);
        }
        for i in 0..N {
            if min[i] < 1 || pref[i] < min[i] {
                return Err(Error::ImpossibleColumns);
            }
        }
        Ok(Self { align, pref, min })
    }
}

/// A cell box. [`ColAlign`] is ink inside the box, not leftover policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cell {
    pub origin: u16,
    pub width: u16,
    pub align: ColAlign,
}

impl Cell {
    pub(crate) fn end(self) -> Result<u16, Error> {
        self.origin
            .checked_add(self.width)
            .ok_or(Error::ImpossibleColumns)
    }

    pub(crate) fn ink_x(self, line_width: Advance) -> Result<Advance, Error> {
        let origin = Advance::from_dots(i32::from(self.origin)).ok_or(Error::CoordinateOverflow)?;
        match self.align {
            ColAlign::Start => Ok(origin),
            ColAlign::End => {
                let box_w =
                    Advance::from_dots(i32::from(self.width)).ok_or(Error::CoordinateOverflow)?;
                match box_w.checked_sub(line_width) {
                    Some(slack) if slack.units() > 0 => {
                        origin.checked_add(slack).ok_or(Error::CoordinateOverflow)
                    }
                    Some(_) => Ok(origin),
                    None => Ok(origin),
                }
            }
        }
    }
}

/// Packed columns. Compact may be underfull; Hang’s last box ends on the measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Placed<const N: usize> {
    pub col: [Cell; N],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Compact,
    Hang,
}

#[derive(Clone, Copy)]
enum Width {
    Locked { min: u64, pref: u64 },
    Flex { min: u64, pref: u64 },
}

enum Room {
    Fit,
    Squeeze,
    Overflow,
}

impl Width {
    fn new(align: ColAlign, min: u64, pref: u64) -> Self {
        match align {
            ColAlign::End => Self::Locked { min, pref },
            ColAlign::Start => Self::Flex { min, pref },
        }
    }

    fn min(self) -> u64 {
        match self {
            Self::Locked { min, .. } | Self::Flex { min, .. } => min,
        }
    }

    fn pref(self) -> u64 {
        match self {
            Self::Locked { pref, .. } | Self::Flex { pref, .. } => pref,
        }
    }

    fn is_flex(self) -> bool {
        matches!(self, Self::Flex { .. })
    }
}

fn kind<const N: usize>(align: &[ColAlign; N]) -> Kind {
    match align[N - 1] {
        ColAlign::End => Kind::Hang,
        ColAlign::Start => Kind::Compact,
    }
}

fn room(pref_sum: u64, min_sum: u64, inner: u16) -> Room {
    if pref_sum <= u64::from(inner) {
        Room::Fit
    } else if min_sum <= u64::from(inner) {
        Room::Squeeze
    } else {
        Room::Overflow
    }
}

fn gap_after<const N: usize>(align: &[ColAlign; N], i: usize, base: u16) -> u16 {
    if i + 1 >= N {
        return 0;
    }
    match (align[i], align[i + 1]) {
        (ColAlign::End, ColAlign::End) => base.max(GridSkip::TWO.dots()),
        _ => base,
    }
}

fn gutters<const N: usize>(align: &[ColAlign; N], base: u16) -> Result<u16, Error> {
    let mut g = 0u16;
    for i in 0..N.saturating_sub(1) {
        g = g
            .checked_add(gap_after(align, i, base))
            .ok_or(Error::ImpossibleColumns)?;
    }
    Ok(g)
}

fn sum_checked(mut xs: impl Iterator<Item = u16>) -> Result<u16, Error> {
    xs.try_fold(0u16, |a, x| {
        a.checked_add(x).ok_or(Error::ImpossibleColumns)
    })
}

fn sum_natural(mut xs: impl Iterator<Item = u64>) -> Result<u64, Error> {
    xs.try_fold(0u64, |a, x| {
        a.checked_add(x).ok_or(Error::CoordinateOverflow)
    })
}

fn box_width(n: u64) -> Result<u16, Error> {
    u16::try_from(n).map_err(|_| Error::ImpossibleColumns)
}

fn box_widths<const N: usize>(natural: [u64; N]) -> Result<[u16; N], Error> {
    let mut widths = [0; N];
    for (to, from) in widths.iter_mut().zip(natural) {
        *to = box_width(from)?;
    }
    Ok(widths)
}

fn last_flex<const N: usize>(spec: &[Width; N]) -> Option<usize> {
    spec.iter().rposition(|w| w.is_flex())
}

fn next_flex<const N: usize>(spec: &[Width; N], mut k: usize) -> Option<usize> {
    while k < N {
        if spec[k].is_flex() {
            return Some(k);
        }
        k += 1;
    }
    None
}

fn flex_min_from<const N: usize>(spec: &[Width; N], k: usize) -> Result<u16, Error> {
    box_width(sum_natural(
        spec.iter()
            .skip(k)
            .map(|w| if w.is_flex() { w.min() } else { 0 }),
    )?)
}

fn absorb<const N: usize>(
    spec: &[Width; N],
    widths: &mut [u16; N],
    inner: u16,
) -> Result<(), Error> {
    let used = sum_checked(widths.iter().copied())?;
    let Some(leftover) = inner.checked_sub(used) else {
        return Err(Error::ImpossibleColumns);
    };
    if leftover == 0 {
        return Ok(());
    }
    if let Some(i) = last_flex(spec) {
        widths[i] = widths[i]
            .checked_add(leftover)
            .ok_or(Error::ImpossibleColumns)?;
    }
    Ok(())
}

fn pack<const N: usize>(
    origin0: u16,
    widths: [u16; N],
    align: [ColAlign; N],
    gutter: u16,
    limit: u16,
) -> Result<Placed<N>, Error> {
    let mut origin = origin0;
    let mut col = [Cell {
        origin: origin0,
        width: 1,
        align: align[0],
    }; N];
    for i in 0..N {
        if widths[i] < 1 {
            return Err(Error::ImpossibleColumns);
        }
        let end = origin
            .checked_add(widths[i])
            .ok_or(Error::ImpossibleColumns)?;
        if end > limit {
            return Err(Error::ImpossibleColumns);
        }
        col[i] = Cell {
            origin,
            width: widths[i],
            align: align[i],
        };
        if i + 1 < N {
            origin = end
                .checked_add(gap_after(&align, i, gutter))
                .ok_or(Error::ImpossibleColumns)?;
        }
    }
    for w in col.windows(2) {
        if w[0].end()? > w[1].origin {
            return Err(Error::ImpossibleColumns);
        }
    }
    Ok(Placed { col })
}

impl<const N: usize> Placed<N> {
    fn compact(
        widths: [u16; N],
        align: [ColAlign; N],
        x0: u16,
        gutter: u16,
        limit: u16,
    ) -> Result<Self, Error> {
        pack(x0, widths, align, gutter, limit)
    }

    fn hang(
        widths: [u16; N],
        align: [ColAlign; N],
        x0: u16,
        measure: u16,
        gutter: u16,
        limit: u16,
    ) -> Result<Self, Error> {
        let used = sum_checked(widths.iter().copied())?
            .checked_add(gutters(&align, gutter)?)
            .ok_or(Error::ImpossibleColumns)?;
        if used > measure {
            return Err(Error::ImpossibleColumns);
        }
        let slack = measure - used;
        let origin = x0.checked_add(slack).ok_or(Error::ImpossibleColumns)?;
        let placed = pack(origin, widths, align, gutter, limit)?;
        if placed.col[N - 1].end()? != limit {
            return Err(Error::ImpossibleColumns);
        }
        Ok(placed)
    }
}

fn place<const N: usize>(
    table: Kind,
    widths: [u16; N],
    align: [ColAlign; N],
    x0: u16,
    measure: u16,
    gutter: u16,
) -> Result<Placed<N>, Error> {
    let limit = x0.checked_add(measure).ok_or(Error::ImpossibleColumns)?;
    match table {
        Kind::Compact => Placed::compact(widths, align, x0, gutter, limit),
        Kind::Hang => Placed::hang(widths, align, x0, measure, gutter, limit),
    }
}

/// Parse leftover policy, allocate widths, pack boxes. The only leftover consumer.
pub(crate) fn layout<const N: usize>(
    natural: Natural<N>,
    x0: u16,
    measure: u16,
    gutter: u16,
    cost: impl FnMut(&[u16; N]) -> Result<WrapCost, Error>,
) -> Result<Placed<N>, Error> {
    if N < 2 {
        return Err(Error::ImpossibleColumns);
    }
    let gaps = gutters(&natural.align, gutter)?;
    let min_boxes = u16::try_from(N).map_err(|_| Error::ImpossibleColumns)?;
    let need = gaps
        .checked_add(min_boxes)
        .ok_or(Error::ImpossibleColumns)?;
    if need > measure {
        return Err(Error::ImpossibleColumns);
    }
    let inner = measure.checked_sub(gaps).ok_or(Error::ImpossibleColumns)?;
    if inner < min_boxes {
        return Err(Error::ImpossibleColumns);
    }
    let spec: [Width; N] =
        std::array::from_fn(|i| Width::new(natural.align[i], natural.min[i], natural.pref[i]));
    let table = kind(&natural.align);
    let pref_sum = sum_natural(natural.pref.iter().copied())?;
    let min_sum = sum_natural(natural.min.iter().copied())?;
    let widths = match room(pref_sum, min_sum, inner) {
        Room::Fit => fit(table, &spec, inner)?,
        Room::Squeeze => squeeze(table, &spec, inner, cost)?,
        Room::Overflow => overflow(&spec, inner)?,
    };
    let used = sum_checked(widths.iter().copied())?;
    if used > inner || widths.iter().any(|&w| w < 1) {
        return Err(Error::ImpossibleColumns);
    }
    place(table, widths, natural.align, x0, measure, gutter)
}

fn fit<const N: usize>(table: Kind, spec: &[Width; N], inner: u16) -> Result<[u16; N], Error> {
    let mut widths = box_widths(spec.map(Width::pref))?;
    if table == Kind::Hang {
        absorb(spec, &mut widths, inner)?;
    }
    Ok(widths)
}

fn overflow<const N: usize>(spec: &[Width; N], inner: u16) -> Result<[u16; N], Error> {
    let mut widths = [1; N];
    let locked_min = sum_natural(spec.iter().map(|w| if w.is_flex() { 0 } else { w.min() }))?;
    let flex_count = u64::try_from(spec.iter().filter(|w| w.is_flex()).count())
        .map_err(|_| Error::ImpossibleColumns)?;
    if flex_count > 0 && locked_min <= u64::from(inner) - flex_count {
        for (i, w) in spec.iter().enumerate() {
            if !w.is_flex() {
                widths[i] = box_width(w.min())?;
            }
        }
        let budget = inner - box_width(locked_min)?;
        scale_where(&mut widths, spec, true, budget)?;
        return Ok(widths);
    }
    let mut flex_n = 0u16;
    for (i, w) in spec.iter().enumerate() {
        if w.is_flex() {
            widths[i] = 1;
            flex_n = flex_n.checked_add(1).ok_or(Error::ImpossibleColumns)?;
        }
    }
    let left = inner.checked_sub(flex_n).ok_or(Error::ImpossibleColumns)?;
    if left < 1 && spec.iter().any(|w| !w.is_flex()) {
        return Err(Error::ImpossibleColumns);
    }
    scale_where(&mut widths, spec, false, left.max(1))?;
    Ok(widths)
}

fn scale_where<const N: usize>(
    widths: &mut [u16; N],
    spec: &[Width; N],
    flex: bool,
    budget: u16,
) -> Result<(), Error> {
    let count = spec.iter().filter(|w| w.is_flex() == flex).count();
    if count == 0 {
        return Ok(());
    }
    let group = sum_natural(
        (0..N)
            .filter(|&i| spec[i].is_flex() == flex)
            .map(|i| spec[i].min()),
    )?;
    let count_dots = u16::try_from(count).map_err(|_| Error::ImpossibleColumns)?;
    if budget < count_dots {
        return Err(Error::ImpossibleColumns);
    }
    let mut used = 0u16;
    let mut seen = 0usize;
    for i in 0..N {
        if spec[i].is_flex() != flex {
            continue;
        }
        seen += 1;
        if seen == count {
            widths[i] = budget.checked_sub(used).ok_or(Error::ImpossibleColumns)?;
            if widths[i] < 1 {
                return Err(Error::ImpossibleColumns);
            }
        } else {
            let raw = (u128::from(spec[i].min()) * u128::from(budget)) / u128::from(group);
            // Leave one dot for every remaining box, even with skewed weights.
            let reserve = u16::try_from(count - seen).map_err(|_| Error::ImpossibleColumns)?;
            widths[i] = u16::try_from(raw)
                .map_err(|_| Error::ImpossibleColumns)?
                .max(1)
                .min(budget - used - reserve);
            used = used
                .checked_add(widths[i])
                .ok_or(Error::ImpossibleColumns)?;
        }
    }
    Ok(())
}

fn squeeze<const N: usize>(
    table: Kind,
    spec: &[Width; N],
    inner: u16,
    cost: impl FnMut(&[u16; N]) -> Result<WrapCost, Error>,
) -> Result<[u16; N], Error> {
    if next_flex(spec, 0).is_none() {
        return overflow(spec, inner);
    }
    let (mut widths, budget) = squeeze_budget(spec, inner)?;
    let mut cx = Squeeze {
        spec,
        table,
        widths,
        best: widths,
        best_cost: None,
        cost,
    };
    cx.search(0, budget)?;
    if cx.best_cost.is_some() {
        return Ok(cx.best);
    }
    for (i, w) in spec.iter().enumerate() {
        if let Width::Flex { min, .. } = *w {
            widths[i] = box_width(min)?;
        }
    }
    if table == Kind::Hang {
        absorb(spec, &mut widths, inner)?;
    }
    Ok(widths)
}

fn squeeze_budget<const N: usize>(spec: &[Width; N], inner: u16) -> Result<([u16; N], u16), Error> {
    let mut natural = spec.map(Width::min);
    let flex_min = sum_natural(spec.iter().map(|w| if w.is_flex() { w.min() } else { 0 }))?;
    let mut locked = sum_natural(spec.iter().map(|w| if w.is_flex() { 0 } else { w.pref() }))?;
    for (i, w) in spec.iter().enumerate() {
        if !w.is_flex() {
            natural[i] = w.pref();
        }
    }
    let locked_budget = u64::from(inner)
        .checked_sub(flex_min)
        .ok_or(Error::ImpossibleColumns)?;
    if locked > locked_budget {
        let mut deficit = locked - locked_budget;
        for (i, w) in spec.iter().enumerate() {
            if w.is_flex() || deficit == 0 {
                continue;
            }
            let take = w.pref().saturating_sub(w.min()).min(deficit);
            natural[i] = w.pref() - take;
            locked -= take;
            deficit -= take;
        }
    }
    let budget = inner
        .checked_sub(box_width(locked)?)
        .ok_or(Error::ImpossibleColumns)?;
    Ok((box_widths(natural)?, budget))
}

struct Squeeze<'a, const N: usize, C> {
    spec: &'a [Width; N],
    table: Kind,
    widths: [u16; N],
    best: [u16; N],
    best_cost: Option<WrapCost>,
    cost: C,
}

impl<const N: usize, C> Squeeze<'_, N, C>
where
    C: FnMut(&[u16; N]) -> Result<WrapCost, Error>,
{
    fn search(&mut self, k: usize, remaining: u16) -> Result<(), Error> {
        let Some(i) = next_flex(self.spec, k) else {
            return Ok(());
        };
        let Width::Flex { min, pref } = self.spec[i] else {
            return Ok(());
        };
        if min > u64::from(remaining) {
            return Ok(());
        }
        let min = box_width(min)?;
        let pref = box_width(pref.min(u64::from(remaining)))?;
        if next_flex(self.spec, i + 1).is_none() {
            let w = match self.table {
                Kind::Compact => remaining.min(pref),
                Kind::Hang => remaining,
            };
            if w < min {
                return Ok(());
            }
            self.widths[i] = w;
            let cost = (self.cost)(&self.widths)?;
            let better = match self.best_cost {
                None => true,
                Some(best) => cost < best,
            };
            if better {
                self.best_cost = Some(cost);
                self.best = self.widths;
            }
            return Ok(());
        }
        let later_min = flex_min_from(self.spec, i + 1)?;
        each_grid_tick(min, pref, |w| {
            if w.saturating_add(later_min) <= remaining {
                self.widths[i] = w;
                self.search(i + 1, remaining - w)?;
            }
            Ok(())
        })
    }
}

fn each_grid_tick(
    lo: u16,
    hi: u16,
    mut visit: impl FnMut(u16) -> Result<(), Error>,
) -> Result<(), Error> {
    visit(lo)?;
    if lo >= hi {
        return Ok(());
    }
    let mut x = (lo / GRID + 1) * GRID;
    if x <= lo {
        x = x.checked_add(GRID).ok_or(Error::ImpossibleColumns)?;
    }
    while x < hi {
        visit(x)?;
        let Some(next) = x.checked_add(GRID) else {
            break;
        };
        if next <= x {
            break;
        }
        x = next;
    }
    visit(hi)
}

#[cfg(test)]
fn idle_cost<const N: usize>(_: &[u16; N]) -> Result<WrapCost, Error> {
    Ok(WrapCost::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Advance;

    fn place_fit<const N: usize>(
        align: [ColAlign; N],
        pref: [u16; N],
        x0: u16,
        measure: u16,
        gutter: u16,
    ) -> Placed<N> {
        layout(
            Natural::new(align, pref.map(u64::from), pref.map(u64::from)).unwrap(),
            x0,
            measure,
            gutter,
            idle_cost,
        )
        .expect("feasible columns")
    }

    #[test]
    fn intrinsic_width_sum_does_not_limit_feasible_boxes() {
        let natural = Natural::new(
            [ColAlign::Start, ColAlign::Start],
            [10, 10],
            [40_000, 40_000],
        )
        .unwrap();
        let placed = layout(natural, 0, 576, GRID, idle_cost).expect("short words fit");
        assert!(
            placed
                .col
                .iter()
                .all(|c| c.width >= 10 && c.end().unwrap() <= 576)
        );
    }

    #[test]
    fn wide_preferences_and_minima_allocate_only_page_local_boxes() {
        for align in [
            [ColAlign::Start, ColAlign::End],
            [ColAlign::End, ColAlign::End],
            [ColAlign::Start, ColAlign::Start],
        ] {
            for (min, pref) in [
                ([10, 10], [100_000, 100_000]),
                ([100_000, 1], [100_000, 1]),
                ([1, 100_000], [1, 100_000]),
            ] {
                let p = layout(
                    Natural::new(align, min, pref).unwrap(),
                    0,
                    576,
                    GRID,
                    idle_cost,
                )
                .unwrap();
                assert!(p.col.iter().all(|c| c.width > 0 && c.end().unwrap() <= 576));
                assert!(p.col[0].end().unwrap() + GRID <= p.col[1].origin);
            }
        }
    }

    #[test]
    fn skewed_overflow_reserves_a_dot_per_column() {
        for measure in 19..40 {
            for min in [[100_000, 1, 1], [1, 100_000, 1], [1, 1, 100_000]] {
                let p = layout(
                    Natural::new([ColAlign::Start; 3], min, min).unwrap(),
                    0,
                    measure,
                    GRID,
                    idle_cost,
                )
                .unwrap();
                assert!(
                    p.col
                        .iter()
                        .all(|c| c.width >= 1 && c.end().unwrap() <= measure)
                );
            }
        }
    }

    #[test]
    fn natural_rejects_min_above_pref() {
        assert!(matches!(
            Natural::new([ColAlign::Start, ColAlign::Start], [10, 4], [8, 4]),
            Err(Error::ImpossibleColumns)
        ));
        assert!(matches!(
            Natural::new([ColAlign::Start, ColAlign::End], [0, 1], [1, 1]),
            Err(Error::ImpossibleColumns)
        ));
    }

    #[test]
    fn compact_does_not_absorb_leftover() {
        let p = place_fit([ColAlign::Start, ColAlign::Start], [10, 20], 0, 100, GRID);
        assert_eq!(p.col[0].width, 10);
        assert_eq!(p.col[1].width, 20);
        assert_eq!(p.col[0].origin, 0);
        assert_eq!(p.col[1].origin, 10 + GRID);
        assert_eq!(p.col[1].end().unwrap(), 10 + GRID + 20);
        assert!(p.col[1].end().unwrap() < 100);
        assert_eq!(p.col[1].origin - p.col[0].end().unwrap(), GRID);
    }

    #[test]
    fn hang_last_box_ends_on_the_measure() {
        let p = place_fit([ColAlign::Start, ColAlign::End], [10, 20], 0, 100, GRID);
        assert_eq!(p.col[1].width, 20);
        assert_eq!(p.col[0].width, 100 - GRID - 20);
        assert_eq!(p.col[0].origin, 0);
        assert_eq!(p.col[1].end().unwrap(), 100);
        assert_eq!(p.col[1].origin - p.col[0].end().unwrap(), GRID);
        assert_eq!(
            p.col[0].ink_x(Advance::from_dots(10).unwrap()).unwrap(),
            Advance::ZERO
        );
        assert_eq!(
            p.col[1].ink_x(Advance::from_dots(20).unwrap()).unwrap(),
            Advance::from_dots(i32::from(p.col[1].origin)).unwrap()
        );
    }

    #[test]
    fn hang_without_flex_packs_from_the_right() {
        let p = place_fit([ColAlign::End, ColAlign::End], [10, 20], 0, 100, GRID);
        assert_eq!(p.col[0].width, 10);
        assert_eq!(p.col[1].width, 20);
        assert_eq!(p.col[1].end().unwrap(), 100);
        assert_eq!(p.col[0].origin, 100 - 20 - GridSkip::TWO.dots() - 10);
        assert_eq!(
            p.col[1].origin - p.col[0].end().unwrap(),
            GridSkip::TWO.dots()
        );
    }

    #[test]
    fn adjacent_end_cannot_share_a_single_module() {
        let p = place_fit(
            [ColAlign::Start, ColAlign::End, ColAlign::End],
            [40, 10, 20],
            0,
            200,
            GRID,
        );
        assert_eq!(p.col[1].origin - p.col[0].end().unwrap(), GRID);
        assert_eq!(
            p.col[2].origin - p.col[1].end().unwrap(),
            GridSkip::TWO.dots()
        );
        assert_eq!(p.col[2].end().unwrap(), 200);
        assert!(p.col.windows(2).all(|w| w[0].end().unwrap() <= w[1].origin));
    }

    #[test]
    fn three_start_is_compact() {
        let p = place_fit(
            [ColAlign::Start, ColAlign::Start, ColAlign::Start],
            [8, 16, 24],
            4,
            200,
            GRID,
        );
        assert_eq!(p.col[0].origin, 4);
        assert_eq!(p.col[2].end().unwrap(), 4 + 8 + GRID + 16 + GRID + 24);
        assert!(p.col[2].end().unwrap() < 4 + 200);
        assert_eq!(p.col[1].origin - p.col[0].end().unwrap(), GRID);
        assert_eq!(p.col[2].origin - p.col[1].end().unwrap(), GRID);
    }

    #[test]
    fn start_ink_stays_at_origin() {
        let p = place_fit([ColAlign::Start, ColAlign::Start], [40, 10], 0, 100, GRID);
        let origin = Advance::from_dots(i32::from(p.col[1].origin)).unwrap();
        assert_eq!(
            p.col[1].ink_x(Advance::from_dots(6).unwrap()).unwrap(),
            origin
        );
        let shifted = origin
            .checked_add(
                Advance::from_dots(i32::from(p.col[1].width))
                    .unwrap()
                    .checked_sub(Advance::from_dots(6).unwrap())
                    .unwrap(),
            )
            .unwrap();
        assert_ne!(
            p.col[1].ink_x(Advance::from_dots(6).unwrap()).unwrap(),
            shifted
        );
    }

    #[test]
    fn compact_slight_overfull_stays_inside_the_measure() {
        let p = place_fit(
            [ColAlign::Start, ColAlign::Start, ColAlign::Start],
            [90, 185, 287],
            0,
            576,
            GRID,
        );
        let end = p.col[2].end().unwrap();
        assert!(end <= 576, "last box ends at {end}");
        assert!(p.col.windows(2).all(|w| w[0].end().unwrap() <= w[1].origin));
        assert!(p.col.iter().all(|c| c.origin < 576 && c.width >= 1));
    }

    #[test]
    fn impossible_gutters_reject() {
        assert!(matches!(
            layout(
                Natural::new([ColAlign::Start, ColAlign::Start], [1, 1], [1, 1]).unwrap(),
                0,
                9,
                GRID,
                idle_cost,
            ),
            Err(Error::ImpossibleColumns)
        ));
        assert!(matches!(
            layout(
                Natural::new([ColAlign::End, ColAlign::End], [1, 1], [1, 1]).unwrap(),
                0,
                17,
                GRID,
                idle_cost,
            ),
            Err(Error::ImpossibleColumns)
        ));
    }

    #[test]
    fn wrap_cost_is_lexicographic() {
        let a = WrapCost {
            extra_lines: 0,
            raggedness: 100,
        };
        let b = WrapCost {
            extra_lines: 1,
            raggedness: 0,
        };
        assert!(a < b);
        assert_eq!(a.checked_add(b).unwrap().extra_lines, 1);
    }
}
