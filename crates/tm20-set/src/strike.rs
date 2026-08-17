//! Hinted CFF outline → packed 1-bit strike. The fill is winding, not coverage.

use skrifa::outline::OutlinePen;
use tm20::graphics::{set_black, width_bytes};

use crate::size::FRAC;

/// Packed glyph at one [`ppem`](crate::TextSize::ppem). Bearings are dots; advance is 26.6.
#[derive(Clone)]
pub(crate) struct Strike {
    pub left: i32,
    pub top: i32,
    pub width: u16,
    pub height: u16,
    pub bits: Vec<u8>,
    pub advance: i32,
}

impl Strike {
    pub(crate) fn empty(advance: i32) -> Self {
        Self {
            left: 0,
            top: 0,
            width: 0,
            height: 0,
            bits: Vec::new(),
            advance,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Pt {
    x: f32,
    y: f32,
}

impl Pt {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn mid(self, other: Self) -> Self {
        Self {
            x: f32::midpoint(self.x, other.x),
            y: f32::midpoint(self.y, other.y),
        }
    }
}

pub(crate) fn from_pen(path: &Path, advance_px: f32) -> Strike {
    let advance = (advance_px * FRAC as f32).round() as i32;
    if path.pts.is_empty() {
        return Strike::empty(advance);
    }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for p in &path.pts {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    let left = min_x.floor() as i32;
    let bottom = min_y.floor() as i32;
    let right = max_x.ceil() as i32;
    let top = max_y.ceil() as i32;
    let width = right.saturating_sub(left).max(0) as u16;
    let height = top.saturating_sub(bottom).max(0) as u16;
    if width == 0 || height == 0 {
        return Strike {
            left,
            top,
            width: 0,
            height: 0,
            bits: Vec::new(),
            advance,
        };
    }
    let stride = width_bytes(width);
    let mut bits = vec![0u8; stride * height as usize];
    let edges = path.edges();
    for gy in 0..height {
        let sy = top as f32 - f32::from(gy) - 0.5;
        let mut hits: Vec<(f32, i8)> = Vec::new();
        for &(a, b) in &edges {
            if (a.y - b.y).abs() < 1e-6 {
                continue;
            }
            let (lo, hi) = if a.y < b.y { (a.y, b.y) } else { (b.y, a.y) };
            if sy < lo || sy >= hi {
                continue;
            }
            let t = (sy - a.y) / (b.y - a.y);
            let x = a.x + t * (b.x - a.x);
            let dir = if b.y > a.y { 1i8 } else { -1 };
            hits.push((x, dir));
        }
        hits.sort_by(|p, q| p.0.total_cmp(&q.0));
        let mut wind = 0i32;
        let mut x_on = 0.0;
        for (x, dir) in hits {
            if wind == 0 {
                x_on = x;
            }
            wind += i32::from(dir);
            if wind == 0 {
                let first = (x_on - left as f32 - 0.5).ceil() as i32;
                let last = (x - left as f32 - 0.5).ceil() as i32;
                for gx in first.max(0)..last.min(i32::from(width)).max(0) {
                    set_black(&mut bits, stride, gx as usize, gy as usize);
                }
            }
        }
    }
    Strike {
        left,
        top,
        width,
        height,
        bits,
        advance,
    }
}

/// Outline sink. Font y is up; the strike flips it for the canvas.
#[derive(Default)]
pub(crate) struct Path {
    pts: Vec<Pt>,
    ends: Vec<usize>,
    cur: Pt,
    start: Pt,
    open: bool,
}

impl Path {
    fn close_open(&mut self) {
        if !self.open {
            return;
        }
        self.pts.push(self.start);
        self.ends.push(self.pts.len());
        self.open = false;
    }

    fn edges(&self) -> Vec<(Pt, Pt)> {
        let mut out = Vec::new();
        let mut start = 0;
        for &end in &self.ends {
            if end.saturating_sub(start) >= 2 {
                for i in start..end - 1 {
                    out.push((self.pts[i], self.pts[i + 1]));
                }
            }
            start = end;
        }
        out
    }
}

impl OutlinePen for Path {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close_open();
        let p = Pt::new(x, y);
        self.start = p;
        self.cur = p;
        self.open = true;
        self.pts.push(p);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = Pt::new(x, y);
        self.pts.push(p);
        self.cur = p;
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let end = Pt::new(x, y);
        flatten_quad(self, self.cur, Pt::new(cx0, cy0), end, 0);
        self.cur = end;
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let end = Pt::new(x, y);
        flatten_cubic(self, self.cur, Pt::new(cx0, cy0), Pt::new(cx1, cy1), end, 0);
        self.cur = end;
    }

    fn close(&mut self) {
        self.close_open();
    }
}

const FLAT: f32 = 0.25;
const FLAT_DEPTH: u8 = 8;

fn flatten_quad(path: &mut Path, a: Pt, ctrl: Pt, b: Pt, depth: u8) {
    let mid = a.mid(b);
    if depth >= FLAT_DEPTH || ((ctrl.x - mid.x).abs() <= FLAT && (ctrl.y - mid.y).abs() <= FLAT) {
        path.line_to(b.x, b.y);
        return;
    }
    let ab = a.mid(ctrl);
    let cb = ctrl.mid(b);
    let m = ab.mid(cb);
    flatten_quad(path, a, ab, m, depth + 1);
    flatten_quad(path, m, cb, b, depth + 1);
}

fn flatten_cubic(path: &mut Path, p0: Pt, p1: Pt, p2: Pt, p3: Pt, depth: u8) {
    let ux = (3.0 * p1.x - 2.0 * p0.x - p3.x).abs();
    let uy = (3.0 * p1.y - 2.0 * p0.y - p3.y).abs();
    let vx = (3.0 * p2.x - 2.0 * p3.x - p0.x).abs();
    let vy = (3.0 * p2.y - 2.0 * p3.y - p0.y).abs();
    if depth >= FLAT_DEPTH || (ux.max(vx) <= FLAT && uy.max(vy) <= FLAT) {
        path.line_to(p3.x, p3.y);
        return;
    }
    let q0 = p0.mid(p1);
    let q1 = p1.mid(p2);
    let q2 = p2.mid(p3);
    let r0 = q0.mid(q1);
    let r1 = q1.mid(q2);
    let m = r0.mid(r1);
    flatten_cubic(path, p0, q0, r0, m, depth + 1);
    flatten_cubic(path, m, r1, q2, p3, depth + 1);
}
