//! Compact vs Hang. Paint walks boxes; leftover is not re-derived.

use crate::error::Error;
use crate::frame::ColAlign;
use crate::leading::GRID;

/// Unwrapped (`pref`) and longest-word (`min`) widths, with ink alignment.
#[derive(Clone, Copy)]
pub(crate) struct Natural<const N: usize> {
    pub align: [ColAlign; N],
    pub pref: [u16; N],
    pub min: [u16; N],
}

/// A cell box. [`ColAlign`] is ink inside the box, not leftover policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cell {
    pub origin: u16,
    pub width: u16,
    pub align: ColAlign,
}

impl Cell {
    pub(crate) fn end(self) -> u16 {
        self.origin.saturating_add(self.width)
    }

    pub(crate) fn ink_x(self, line_width: f32) -> f32 {
        match self.align {
            ColAlign::Start => f32::from(self.origin),
            ColAlign::End => f32::from(self.origin) + (f32::from(self.width) - line_width).max(0.0),
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
    Locked { min: u16, pref: u16 },
    Flex { min: u16, pref: u16 },
}

enum Room {
    Fit,
    Squeeze,
    Overflow,
}

impl Width {
    fn new(align: ColAlign, min: u16, pref: u16) -> Self {
        match align {
            ColAlign::End => Self::Locked { min, pref },
            ColAlign::Start => Self::Flex { min, pref },
        }
    }

    fn min(self) -> u16 {
        match self {
            Self::Locked { min, .. } | Self::Flex { min, .. } => min,
        }
    }

    fn pref(self) -> u16 {
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

fn room(pref_sum: u16, min_sum: u16, inner: u16) -> Room {
    let over = pref_sum.saturating_sub(inner);
    // A deficit smaller than a gutter is overfull, not a wrap. GRID is the quantum.
    if pref_sum <= inner || over < GRID {
        Room::Fit
    } else if min_sum <= inner {
        Room::Squeeze
    } else {
        Room::Overflow
    }
}

fn inner_width(measure: u16, gutter: u16, n: usize) -> u16 {
    let gutters = gutter.saturating_mul(n.saturating_sub(1) as u16);
    measure.saturating_sub(gutters).max(1)
}

fn sum(xs: impl Iterator<Item = u16>) -> u16 {
    xs.fold(0u16, u16::saturating_add)
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

fn flex_min_from<const N: usize>(spec: &[Width; N], k: usize) -> u16 {
    sum(spec
        .iter()
        .skip(k)
        .map(|w| if w.is_flex() { w.min() } else { 0 }))
}

fn absorb<const N: usize>(spec: &[Width; N], widths: &mut [u16; N], inner: u16) {
    let leftover = inner.saturating_sub(sum(widths.iter().copied()));
    if leftover == 0 {
        return;
    }
    if let Some(i) = last_flex(spec) {
        widths[i] = widths[i].saturating_add(leftover);
    }
}

fn pack<const N: usize>(
    origin0: u16,
    widths: [u16; N],
    align: [ColAlign; N],
    gutter: u16,
) -> Placed<N> {
    let mut origin = origin0;
    Placed {
        col: std::array::from_fn(|i| {
            let cell = Cell {
                origin,
                width: widths[i],
                align: align[i],
            };
            origin = origin.saturating_add(widths[i]);
            if i + 1 < N {
                origin = origin.saturating_add(gutter);
            }
            cell
        }),
    }
}

impl<const N: usize> Placed<N> {
    fn compact(widths: [u16; N], align: [ColAlign; N], x0: u16, gutter: u16) -> Self {
        pack(x0, widths, align, gutter)
    }

    fn hang(widths: [u16; N], align: [ColAlign; N], x0: u16, measure: u16, gutter: u16) -> Self {
        let gutters = gutter.saturating_mul(N.saturating_sub(1) as u16);
        let used = sum(widths.iter().copied()).saturating_add(gutters);
        let slack = measure.saturating_sub(used);
        let placed = pack(x0.saturating_add(slack), widths, align, gutter);
        if used <= measure {
            debug_assert_eq!(placed.col[N - 1].end(), x0.saturating_add(measure));
        }
        placed
    }
}

fn place<const N: usize>(
    table: Kind,
    widths: [u16; N],
    align: [ColAlign; N],
    x0: u16,
    measure: u16,
    gutter: u16,
) -> Placed<N> {
    match table {
        Kind::Compact => Placed::compact(widths, align, x0, gutter),
        Kind::Hang => Placed::hang(widths, align, x0, measure, gutter),
    }
}

/// Parse leftover policy, allocate widths, pack boxes. The only leftover consumer.
pub(crate) fn layout<const N: usize>(
    natural: Natural<N>,
    x0: u16,
    measure: u16,
    gutter: u16,
    cost: impl FnMut(&[u16; N]) -> Result<f64, Error>,
) -> Result<Placed<N>, Error> {
    debug_assert!(N >= 2);
    let inner = inner_width(measure, gutter, N);
    let pref = natural.pref.map(|w| w.min(inner).max(1));
    let min: [u16; N] = std::array::from_fn(|i| natural.min[i].min(pref[i]).max(1));
    let spec: [Width; N] = std::array::from_fn(|i| Width::new(natural.align[i], min[i], pref[i]));
    let table = kind(&natural.align);
    let pref_sum = sum(pref.iter().copied());
    let min_sum = sum(min.iter().copied());
    let widths = match room(pref_sum, min_sum, inner) {
        Room::Fit => fit(table, &spec, inner),
        Room::Squeeze => squeeze(table, &spec, inner, cost)?,
        Room::Overflow => overflow(&spec, inner),
    };
    Ok(place(table, widths, natural.align, x0, measure, gutter))
}

fn fit<const N: usize>(table: Kind, spec: &[Width; N], inner: u16) -> [u16; N] {
    let mut widths = spec.map(Width::pref);
    if table == Kind::Hang {
        absorb(spec, &mut widths, inner);
    }
    widths
}

fn overflow<const N: usize>(spec: &[Width; N], inner: u16) -> [u16; N] {
    let mut widths = spec.map(Width::min);
    let locked_min = sum(spec.iter().map(|w| if w.is_flex() { 0 } else { w.min() }));
    if spec.iter().any(|w| w.is_flex()) && locked_min < inner {
        scale_where(&mut widths, spec, true, inner - locked_min);
        return widths;
    }
    let mut flex_n = 0u16;
    for (i, w) in spec.iter().enumerate() {
        if w.is_flex() {
            widths[i] = 1;
            flex_n = flex_n.saturating_add(1);
        }
    }
    let left = inner.saturating_sub(flex_n).max(1);
    scale_where(&mut widths, spec, false, left);
    widths
}

fn scale_where<const N: usize>(widths: &mut [u16; N], spec: &[Width; N], flex: bool, budget: u16) {
    let count = spec.iter().filter(|w| w.is_flex() == flex).count();
    if count == 0 {
        return;
    }
    let group: u32 = (0..N)
        .filter(|&i| spec[i].is_flex() == flex)
        .map(|i| u32::from(widths[i]))
        .sum::<u32>()
        .max(1);
    let mut used = 0u16;
    let mut seen = 0usize;
    for i in 0..N {
        if spec[i].is_flex() != flex {
            continue;
        }
        seen += 1;
        if seen == count {
            widths[i] = budget.saturating_sub(used).max(1);
        } else {
            widths[i] = ((u32::from(widths[i]) * u32::from(budget)) / group) as u16;
            widths[i] = widths[i].max(1);
            used = used.saturating_add(widths[i]);
        }
    }
}

fn squeeze<const N: usize>(
    table: Kind,
    spec: &[Width; N],
    inner: u16,
    cost: impl FnMut(&[u16; N]) -> Result<f64, Error>,
) -> Result<[u16; N], Error> {
    if next_flex(spec, 0).is_none() {
        return Ok(overflow(spec, inner));
    }
    let (mut widths, budget) = squeeze_budget(spec, inner);
    let mut cx = Squeeze {
        spec,
        table,
        widths,
        best: widths,
        best_cost: f64::INFINITY,
        cost,
    };
    cx.search(0, budget)?;
    if cx.best_cost.is_finite() {
        return Ok(cx.best);
    }
    for (i, w) in spec.iter().enumerate() {
        if let Width::Flex { min, .. } = *w {
            widths[i] = min;
        }
    }
    if table == Kind::Hang {
        absorb(spec, &mut widths, inner);
    }
    Ok(widths)
}

fn squeeze_budget<const N: usize>(spec: &[Width; N], inner: u16) -> ([u16; N], u16) {
    let mut widths = spec.map(Width::pref);
    let flex_min = sum(spec.iter().map(|w| if w.is_flex() { w.min() } else { 0 }));
    let mut locked = sum(spec.iter().map(|w| if w.is_flex() { 0 } else { w.pref() }));
    if flex_min.saturating_add(locked) > inner {
        let have = inner.saturating_sub(locked);
        let mut deficit = flex_min.saturating_sub(have);
        for (i, w) in spec.iter().enumerate() {
            if w.is_flex() || deficit == 0 {
                continue;
            }
            let take = w.pref().saturating_sub(w.min()).min(deficit);
            widths[i] = w.pref() - take;
            locked -= take;
            deficit -= take;
        }
    }
    (widths, inner.saturating_sub(locked))
}

struct Squeeze<'a, const N: usize, C> {
    spec: &'a [Width; N],
    table: Kind,
    widths: [u16; N],
    best: [u16; N],
    best_cost: f64,
    cost: C,
}

impl<const N: usize, C> Squeeze<'_, N, C>
where
    C: FnMut(&[u16; N]) -> Result<f64, Error>,
{
    fn search(&mut self, k: usize, remaining: u16) -> Result<(), Error> {
        let Some(i) = next_flex(self.spec, k) else {
            return Ok(());
        };
        let Width::Flex { min, pref } = self.spec[i] else {
            return Ok(());
        };
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
            if cost < self.best_cost {
                self.best_cost = cost;
                self.best = self.widths;
            }
            return Ok(());
        }
        let later_min = flex_min_from(self.spec, i + 1);
        for w in grid_ticks(min, pref) {
            if w.saturating_add(later_min) > remaining {
                continue;
            }
            self.widths[i] = w;
            self.search(i + 1, remaining - w)?;
        }
        Ok(())
    }
}

fn grid_ticks(lo: u16, hi: u16) -> Vec<u16> {
    if lo >= hi {
        return vec![lo];
    }
    let mut out = vec![lo];
    let mut x = (lo / GRID + 1) * GRID;
    if x <= lo {
        x = x.saturating_add(GRID);
    }
    while x < hi {
        out.push(x);
        let next = x.saturating_add(GRID);
        if next <= x {
            break;
        }
        x = next;
    }
    out.push(hi);
    out
}

#[cfg(test)]
fn idle_cost<const N: usize>(_: &[u16; N]) -> Result<f64, Error> {
    Ok(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place_fit<const N: usize>(
        align: [ColAlign; N],
        pref: [u16; N],
        x0: u16,
        measure: u16,
        gutter: u16,
    ) -> Placed<N> {
        layout(
            Natural {
                align,
                pref,
                min: pref,
            },
            x0,
            measure,
            gutter,
            idle_cost,
        )
        .unwrap()
    }

    #[test]
    fn compact_does_not_absorb_leftover() {
        let p = place_fit([ColAlign::Start, ColAlign::Start], [10, 20], 0, 100, GRID);
        assert_eq!(p.col[0].width, 10);
        assert_eq!(p.col[1].width, 20);
        assert_eq!(p.col[0].origin, 0);
        assert_eq!(p.col[1].origin, 10 + GRID);
        assert_eq!(p.col[1].end(), 10 + GRID + 20);
        assert!(p.col[1].end() < 100);
        assert_eq!(p.col[1].origin - p.col[0].end(), GRID);
    }

    #[test]
    fn hang_last_box_ends_on_the_measure() {
        let p = place_fit([ColAlign::Start, ColAlign::End], [10, 20], 0, 100, GRID);
        assert_eq!(p.col[1].width, 20);
        assert_eq!(p.col[0].width, 100 - GRID - 20);
        assert_eq!(p.col[0].origin, 0);
        assert_eq!(p.col[1].end(), 100);
        assert_eq!(p.col[1].origin - p.col[0].end(), GRID);
        assert_eq!(p.col[0].ink_x(10.0), 0.0);
        assert_eq!(p.col[1].ink_x(20.0), f32::from(p.col[1].origin));
    }

    #[test]
    fn hang_without_flex_packs_from_the_right() {
        let p = place_fit([ColAlign::End, ColAlign::End], [10, 20], 0, 100, GRID);
        assert_eq!(p.col[0].width, 10);
        assert_eq!(p.col[1].width, 20);
        assert_eq!(p.col[1].end(), 100);
        assert_eq!(p.col[0].origin, 100 - 20 - GRID - 10);
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
        assert_eq!(p.col[2].end(), 4 + 8 + GRID + 16 + GRID + 24);
        assert!(p.col[2].end() < 4 + 200);
        assert_eq!(p.col[1].origin - p.col[0].end(), GRID);
        assert_eq!(p.col[2].origin - p.col[1].end(), GRID);
    }

    #[test]
    fn start_ink_stays_at_origin() {
        let p = place_fit([ColAlign::Start, ColAlign::Start], [40, 10], 0, 100, GRID);
        assert_eq!(p.col[1].ink_x(6.0), f32::from(p.col[1].origin));
        assert_ne!(
            p.col[1].ink_x(6.0),
            f32::from(p.col[1].origin) + (f32::from(p.col[1].width) - 6.0)
        );
    }

    #[test]
    fn compact_overfull_less_than_a_grid_keeps_pref() {
        let p = place_fit(
            [ColAlign::Start, ColAlign::Start, ColAlign::Start],
            [90, 185, 287],
            0,
            576,
            GRID,
        );
        assert_eq!(p.col.map(|c| c.width), [90, 185, 287]);
        assert_eq!(p.col[2].origin, 90 + GRID + 185 + GRID);
        assert_eq!(p.col[2].end(), 576 + 2);
    }
}
