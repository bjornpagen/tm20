//! Normalized runs, wrapping, and immutable [`LinePlan`]s.
//!
//! Compatible same-context text is shaped once. Notes, math, hard breaks,
//! cut changes and independently authored code boxes stay barriers. Painting
//! consumes the positioned atoms; this module does not blit.

use std::sync::Arc;

use tm20::Raster;

use crate::error::Error;
use crate::face::{Cut, DisplayCut, DisplayFace, ResolvedFaces, ShapedRun, TextFace};
use crate::frame::{Math, NonEmpty, NoteId, Span};
use crate::geometry::{Advance, InkBounds, Measure};
use crate::size::{DisplaySize, FRAC, TextSize};

use super::plan::{DigitMode, LinePlan, LinePlans, PaintAtom, WrapCost};
use super::{NOTE_RAISE_DEN, NOTE_RAISE_NUM};

const FR: i64 = FRAC as i64;

/// Adjacency between nonempty tokens. A word-space is a variant; `i > 0`
/// cannot invent one. A legal zero-width break is a variant, not a wrap scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Join {
    None,
    Space,
    Break,
    ItalicCorrect,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum After {
    Start,
    Space,
    Join,
    /// Previous token was an independently authored code box.
    Box,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stance {
    Slant,
    Upright,
}

fn stance(cut: Cut) -> Stance {
    match cut {
        Cut::Italic | Cut::BoldItalic => Stance::Slant,
        _ => Stance::Upright,
    }
}

fn join_of(after: After, prev: Stance, next: Stance) -> Join {
    match after {
        After::Space => Join::Space,
        After::Box => Join::Break,
        After::Start | After::Join => match (prev, next) {
            (Stance::Slant, Stance::Upright) => Join::ItalicCorrect,
            _ => Join::None,
        },
    }
}

/// First-plus-joined-rest. There is no parallel-array length equation.
#[derive(Clone)]
struct Chain<T> {
    first: T,
    rest: Vec<(Join, T)>,
}

impl<T> Chain<T> {
    fn singleton(first: T) -> Self {
        Self {
            first,
            rest: Vec::new(),
        }
    }

    fn len(&self) -> usize {
        1 + self.rest.len()
    }
}

impl<T: Clone> Chain<T> {
    fn append(&mut self, join: Join, other: &Self) {
        self.rest.push((join, other.first.clone()));
        self.rest.extend(other.rest.iter().cloned());
    }
}

/// One shaped or fitted box. Width and ink are stored; wrap does not reshape.
#[derive(Clone)]
struct Atom {
    kind: AtomKind,
    width: Advance,
    bounds: InkBounds,
    overhang: Advance,
    italic_tan: f32,
    strike: bool,
    /// Recorded at construction; join decisions use [`Seq::prev`].
    #[allow(dead_code)]
    voice: Stance,
}

#[derive(Clone)]
enum AtomKind {
    Glyph { run: Arc<ShapedRun> },
    Note { run: Arc<ShapedRun> },
    Math { raster: Raster },
}

struct Seq<T> {
    head: Option<T>,
    tail: Vec<(Join, T)>,
    after: After,
    prev: Stance,
}

impl<T> Seq<T> {
    fn new() -> Self {
        Self {
            head: None,
            tail: Vec::new(),
            after: After::Start,
            prev: Stance::Upright,
        }
    }

    fn push(&mut self, item: T, next: Stance) {
        let join = join_of(self.after, self.prev, next);
        match self.head {
            None => self.head = Some(item),
            Some(_) => self.tail.push((join, item)),
        }
        self.prev = next;
        self.after = After::Join;
    }

    fn take(&mut self) -> Option<Chain<T>> {
        let first = self.head.take()?;
        self.after = After::Start;
        Some(Chain {
            first,
            rest: std::mem::take(&mut self.tail),
        })
    }
}

enum Item {
    Text {
        cut: Cut,
        text: String,
        strike: bool,
    },
    Note {
        id: NoteId,
        strike: bool,
    },
    Math {
        math: Math,
        strike: bool,
    },
}

/// Break opportunities inside one token, after URI punctuation. A token with
/// no punctuation is one fragment and still clips — honest loss beats a
/// hyphen the source never wrote.
pub(crate) const BREAK_AFTER: [char; 12] =
    ['/', '?', '&', '=', '#', '-', '_', '.', ',', ';', ':', '@'];

pub(crate) fn frags(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, c) in text.char_indices() {
        if BREAK_AFTER.contains(&c) {
            let end = i + c.len_utf8();
            out.push(&text[start..end]);
            start = end;
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    if out.is_empty() {
        out.push(text);
    }
    out
}

struct Cx<'f> {
    size: TextSize,
    measure: Measure,
    measure_u: i64,
    faces: &'f ResolvedFaces<'f>,
    digits: DigitMode,
    space: Advance,
    raise: Advance,
}

impl<'f> Cx<'f> {
    fn new(
        size: TextSize,
        measure: Measure,
        faces: &'f ResolvedFaces<'f>,
        digits: DigitMode,
    ) -> Result<Self, Error> {
        let measure_u = i64::from(measure.get())
            .checked_mul(FR)
            .ok_or(Error::CoordinateOverflow)?;
        let raise = note_raise(size)?;
        let roman = faces.text(Cut::Roman)?;
        let space = roman.shape_run(" ", size)?.advance();
        Ok(Self {
            size,
            measure,
            measure_u,
            faces,
            digits,
            space,
            raise,
        })
    }

    fn text_face(&self, cut: Cut) -> Result<&'f TextFace, Error> {
        self.faces.text(cut)
    }

    fn roman(&self) -> Result<&'f TextFace, Error> {
        self.text_face(Cut::Roman)
    }
}

fn note_raise(size: TextSize) -> Result<Advance, Error> {
    let units = i64::from(size.ppem())
        .checked_mul(i64::from(NOTE_RAISE_NUM))
        .and_then(|v| v.checked_mul(FR))
        .ok_or(Error::CoordinateOverflow)?
        / i64::from(NOTE_RAISE_DEN);
    Ok(Advance::from_units(units))
}

fn blank_slug(size: TextSize) -> Result<Advance, Error> {
    Advance::from_dots(i32::from(size.skip_dots())).ok_or(Error::CoordinateOverflow)
}

fn display_slug(size: DisplaySize) -> Result<Advance, Error> {
    Advance::from_dots(i32::from(size.skip_dots())).ok_or(Error::CoordinateOverflow)
}

fn shape_kind(
    face: &TextFace,
    size: TextSize,
    text: &str,
    digits: DigitMode,
) -> Result<ShapedRun, Error> {
    match digits {
        DigitMode::Tabular => face.shape_figure_run(text, size),
        DigitMode::Proportional => face.shape_run(text, size),
    }
}

fn glyph_atom(
    face: &TextFace,
    size: TextSize,
    text: &str,
    digits: DigitMode,
    voice: Stance,
) -> Result<Atom, Error> {
    let run = shape_kind(face, size, text, digits)?;
    let width = run.advance();
    let bounds = run.ink();
    let overhang = run.overhang();
    Ok(Atom {
        kind: AtomKind::Glyph { run: Arc::new(run) },
        width,
        bounds,
        overhang,
        italic_tan: face.inner().italic_tan(),
        strike: false,
        voice,
    })
}

fn note_atom(cx: &Cx<'_>, id: NoteId) -> Result<Atom, Error> {
    let face = cx.roman()?;
    let label = id.get_u32().to_string();
    let run = face
        .shape_run(&label, TextSize::Pt8)?
        .translate(Advance::ZERO, cx.raise)?;
    let width = run.advance();
    let bounds = run.ink();
    Ok(Atom {
        kind: AtomKind::Note { run: Arc::new(run) },
        width,
        bounds,
        overhang: Advance::ZERO,
        italic_tan: 0.0,
        strike: false,
        voice: Stance::Upright,
    })
}

fn math_atom(cx: &Cx<'_>, math: &Math) -> Result<Atom, Error> {
    let fitted = math.fit(cx.measure)?;
    let raster = fitted.raster().clone();
    let width = Advance::from_dots(i32::from(raster.width())).ok_or(Error::CoordinateOverflow)?;
    let ascent = i64::from(fitted.ascent())
        .checked_mul(FR)
        .ok_or(Error::CoordinateOverflow)?;
    let depth = i64::from(fitted.depth())
        .checked_mul(FR)
        .ok_or(Error::CoordinateOverflow)?;
    let bounds = InkBounds::new(
        Advance::ZERO,
        Advance::from_units(ascent.checked_neg().ok_or(Error::CoordinateOverflow)?),
        width,
        Advance::from_units(depth),
    )
    .ok_or(Error::CoordinateOverflow)?;
    Ok(Atom {
        kind: AtomKind::Math { raster },
        width,
        bounds,
        overhang: Advance::ZERO,
        italic_tan: 0.0,
        strike: false,
        voice: Stance::Upright,
    })
}

fn italic_correct(prev: &Atom, next: &Atom, raise: Advance) -> Result<Advance, Error> {
    let AtomKind::Glyph { .. } = prev.kind else {
        return Ok(Advance::ZERO);
    };
    let geo = if matches!(next.kind, AtomKind::Note { .. }) {
        let v = (prev.italic_tan * raise.units() as f32).round() as i64;
        v.max(0)
    } else {
        0
    };
    Ok(Advance::from_units(prev.overhang.units().max(geo)))
}

fn join_width(cx: &Cx<'_>, join: Join, prev: &Atom, next: &Atom) -> Result<Advance, Error> {
    match join {
        Join::None | Join::Break => Ok(Advance::ZERO),
        Join::Space => Ok(cx.space),
        Join::ItalicCorrect => italic_correct(prev, next, cx.raise),
    }
}

fn place_chain(cx: &Cx<'_>, chain: &Chain<Atom>) -> Result<LinePlan, Error> {
    let mut atoms = Vec::new();
    atoms.try_reserve(chain.len()).map_err(|_| Error::Alloc)?;
    let mut x = Advance::ZERO;
    let mut bounds = InkBounds::EMPTY;
    let mut prev = &chain.first;
    push_atom(&mut atoms, &mut bounds, &chain.first, x)?;
    if chain.first.strike {
        push_strike(cx, &mut atoms, &mut bounds, x, chain.first.width)?;
    }
    x = x
        .checked_add(chain.first.width)
        .ok_or(Error::CoordinateOverflow)?;
    for (join, atom) in &chain.rest {
        let before_join = x;
        x = x
            .checked_add(join_width(cx, *join, prev, atom)?)
            .ok_or(Error::CoordinateOverflow)?;
        push_atom(&mut atoms, &mut bounds, atom, x)?;
        let end = x.checked_add(atom.width).ok_or(Error::CoordinateOverflow)?;
        if atom.strike {
            let start = if prev.strike { before_join } else { x };
            push_strike(cx, &mut atoms, &mut bounds, start, end)?;
        }
        x = end;
        prev = atom;
    }
    let atoms = NonEmpty::from_vec(atoms).ok_or(Error::CoordinateOverflow)?;
    Ok(LinePlan::Ink {
        atoms,
        advance: x,
        bounds,
    })
}

fn push_strike(
    cx: &Cx<'_>,
    atoms: &mut Vec<PaintAtom>,
    bounds: &mut InkBounds,
    start: Advance,
    end: Advance,
) -> Result<(), Error> {
    let y = -i32::from(cx.size.ppem()) / 3;
    let thickness = i32::from(cx.size.ppem().div_ceil(24));
    let rule = InkBounds::new(
        start,
        Advance::from_dots(y).ok_or(Error::CoordinateOverflow)?,
        end,
        Advance::from_dots(y + thickness).ok_or(Error::CoordinateOverflow)?,
    )
    .ok_or(Error::CoordinateOverflow)?;
    if !rule.is_empty() {
        *bounds = bounds.union(rule).ok_or(Error::CoordinateOverflow)?;
        atoms.push(PaintAtom::Rule { bounds: rule });
    }
    Ok(())
}

fn push_atom(
    atoms: &mut Vec<PaintAtom>,
    bounds: &mut InkBounds,
    atom: &Atom,
    x: Advance,
) -> Result<(), Error> {
    let placed = atom
        .bounds
        .translate(x, Advance::ZERO)
        .ok_or(Error::CoordinateOverflow)?;
    *bounds = bounds.union(placed).ok_or(Error::CoordinateOverflow)?;
    atoms.push(match &atom.kind {
        AtomKind::Glyph { run } | AtomKind::Note { run } => PaintAtom::Glyph {
            run: Arc::clone(run),
            x,
            bounds: placed,
        },
        AtomKind::Math { raster } => PaintAtom::Bitmap {
            raster: raster.clone(),
            x,
            bounds: placed,
        },
    });
    Ok(())
}

fn normalize(spans: &[Span<'_>]) -> Vec<Vec<Item>> {
    fn append(spans: &[Span<'_>], strike: bool, chunks: &mut Vec<Vec<Item>>, cur: &mut Vec<Item>) {
        for span in spans {
            match span {
                Span::Strike(inner) => append(inner, true, chunks, cur),
                Span::Math(math) => cur.push(Item::Math {
                    math: math.clone(),
                    strike,
                }),
                Span::Note(id) => cur.push(Item::Note { id: *id, strike }),
                Span::Type { cut, text } => {
                    for (i, hard) in text.split('\n').enumerate() {
                        if i > 0 {
                            chunks.push(std::mem::take(cur));
                        }
                        if hard.is_empty() {
                            continue;
                        }
                        if *cut != Cut::Mono
                            && let Some(Item::Text {
                                cut: prev,
                                text: buf,
                                strike: prev_strike,
                            }) = cur.last_mut()
                            && prev == cut
                            && *prev_strike == strike
                        {
                            buf.push_str(hard);
                        } else {
                            cur.push(Item::Text {
                                cut: *cut,
                                text: hard.to_owned(),
                                strike,
                            });
                        }
                    }
                }
            }
        }
    }
    let mut chunks = Vec::new();
    let mut cur = Vec::new();
    append(spans, false, &mut chunks, &mut cur);
    chunks.push(cur);
    chunks
}

/// Glyph atom plus the source used only for overwide URI fragmentation.
#[derive(Clone)]
struct Token {
    atom: Atom,
    text: Option<(Cut, String)>,
}

fn tokenize(cx: &Cx<'_>, items: &[Item]) -> Result<Option<Chain<Token>>, Error> {
    let mut seq = Seq::new();
    for item in items {
        match item {
            Item::Math { math, strike } => seq.push(
                Token {
                    atom: Atom {
                        strike: *strike,
                        ..math_atom(cx, math)?
                    },
                    text: None,
                },
                Stance::Upright,
            ),
            Item::Note { id, strike } => seq.push(
                Token {
                    atom: Atom {
                        strike: *strike,
                        ..note_atom(cx, *id)?
                    },
                    text: None,
                },
                Stance::Upright,
            ),
            Item::Text { cut, text, strike } => {
                let face = cx.text_face(*cut)?;
                let voice = stance(*cut);
                let atomic = *cut == Cut::Mono && !text.is_empty();
                if atomic {
                    let atom = Atom {
                        strike: *strike,
                        ..glyph_atom(face, cx.size, text, cx.digits, voice)?
                    };
                    if atom.width.units() <= cx.measure_u || !text.contains(' ') {
                        seq.push(
                            Token {
                                atom,
                                text: Some((*cut, text.clone())),
                            },
                            voice,
                        );
                        seq.after = After::Box;
                        continue;
                    }
                }
                if text.starts_with(' ') {
                    seq.after = After::Space;
                }
                for (wi, word) in text.split(' ').filter(|w| !w.is_empty()).enumerate() {
                    if wi > 0 {
                        seq.after = After::Space;
                    }
                    let atom = Atom {
                        strike: *strike,
                        ..glyph_atom(face, cx.size, word, cx.digits, voice)?
                    };
                    seq.push(
                        Token {
                            atom,
                            text: Some((*cut, word.to_owned())),
                        },
                        voice,
                    );
                }
                if text.ends_with(' ') {
                    seq.after = After::Space;
                }
            }
        }
    }
    Ok(seq.take())
}

fn token_chain_width(cx: &Cx<'_>, chain: &Chain<Token>) -> Result<Advance, Error> {
    let mut w = chain.first.atom.width;
    let mut prev = &chain.first.atom;
    for (join, tok) in &chain.rest {
        w = w
            .checked_add(join_width(cx, *join, prev, &tok.atom)?)
            .ok_or(Error::CoordinateOverflow)?;
        w = w
            .checked_add(tok.atom.width)
            .ok_or(Error::CoordinateOverflow)?;
        prev = &tok.atom;
    }
    Ok(w)
}

fn trailing_overhang(chain: &Chain<Token>) -> Advance {
    chain
        .rest
        .last()
        .map_or(chain.first.atom.overhang, |(_, t)| t.atom.overhang)
}

fn token_chain_extent(cx: &Cx<'_>, chain: &Chain<Token>) -> Result<Advance, Error> {
    token_chain_width(cx, chain)?
        .checked_add(trailing_overhang(chain))
        .and_then(|a| {
            a.checked_sub(Advance::from_units(
                chain.first.atom.bounds.x0().units().min(0),
            ))
        })
        .ok_or(Error::CoordinateOverflow)
}

fn explode_tokens(cx: &Cx<'_>, word: Chain<Token>) -> Result<Vec<Chain<Token>>, Error> {
    if token_chain_extent(cx, &word)?.units() <= cx.measure_u {
        return Ok(vec![word]);
    }
    let Chain { first, rest } = word;
    let mut out: Vec<Chain<Token>> = Vec::new();
    let mut cur: Option<Chain<Token>> = None;
    for (join, tok) in std::iter::once((Join::None, first)).chain(rest) {
        let pieces: Vec<Token> = match &tok.text {
            Some((cut, text)) => {
                let face = cx.text_face(*cut)?;
                let voice = stance(*cut);
                let mut parts = Vec::new();
                for frag in frags(text) {
                    let atom = Atom {
                        strike: tok.atom.strike,
                        ..glyph_atom(face, cx.size, frag, cx.digits, voice)?
                    };
                    parts.push(Token {
                        atom,
                        text: Some((*cut, frag.to_owned())),
                    });
                }
                parts
            }
            None => vec![tok],
        };
        for (fi, p) in pieces.into_iter().enumerate() {
            match (&mut cur, fi) {
                (None, _) => cur = Some(Chain::singleton(p)),
                (Some(line), 0) => line.rest.push((join, p)),
                (Some(_), _) => {
                    out.push(cur.take().expect("cur set"));
                    cur = Some(Chain::singleton(p));
                }
            }
        }
    }
    out.extend(cur);
    Ok(out)
}

struct Words {
    first: Chain<Token>,
    rest: Vec<(Join, Chain<Token>)>,
}

impl Words {
    fn len(&self) -> usize {
        1 + self.rest.len()
    }

    fn item(&self, i: usize) -> &Chain<Token> {
        if i == 0 {
            &self.first
        } else {
            &self.rest[i - 1].1
        }
    }

    fn join_before(&self, i: usize) -> Join {
        self.rest[i - 1].0
    }

    fn slice(&self, i: usize, j: usize) -> Chain<Token> {
        let mut line = self.item(i).clone();
        for k in i + 1..j {
            line.append(self.join_before(k), self.item(k));
        }
        line
    }
}

fn split_words(cx: &Cx<'_>, line: Chain<Token>) -> Result<Words, Error> {
    let mut first: Option<Chain<Token>> = None;
    let mut rest: Vec<(Join, Chain<Token>)> = Vec::new();
    let mut take = |join: Join, word: Chain<Token>| -> Result<(), Error> {
        for (fi, frag) in explode_tokens(cx, word)?.into_iter().enumerate() {
            match &mut first {
                None => first = Some(frag),
                Some(_) => {
                    rest.push((if fi == 0 { join } else { Join::Break }, frag));
                }
            }
        }
        Ok(())
    };
    let mut cur = Chain::singleton(line.first);
    let mut pending = Join::Space;
    for (glue, tok) in line.rest {
        match glue {
            Join::Space => {
                take(pending, cur)?;
                pending = Join::Space;
                cur = Chain::singleton(tok);
            }
            Join::Break => {
                take(pending, cur)?;
                pending = Join::Break;
                cur = Chain::singleton(tok);
            }
            Join::None | Join::ItalicCorrect => cur.rest.push((glue, tok)),
        }
    }
    take(pending, cur)?;
    let first = first.ok_or(Error::CoordinateOverflow)?;
    Ok(Words { first, rest })
}

/// Paragraph DP: slack² in 26.6 units, last line of two or more boxes free.
/// Extra line count is not part of this objective.
pub(crate) fn break_ranges(
    n: usize,
    mut width: impl FnMut(usize, usize) -> Result<i64, Error>,
    measure: i64,
) -> Result<(Vec<(usize, usize)>, u128), Error> {
    if n == 0 {
        return Ok((Vec::new(), 0));
    }
    let mut best: Vec<Option<u128>> = Vec::new();
    best.try_reserve(n + 1).map_err(|_| Error::Alloc)?;
    best.resize(n + 1, None);
    let mut prev = vec![0usize; n + 1];
    best[0] = Some(0);
    for j in 1..=n {
        for i in (0..j).rev() {
            let w = width(i, j)?;
            if w > measure && j - i > 1 {
                break;
            }
            let n_boxes = j - i;
            let last = j == n;
            let slack = if w > measure || (last && n_boxes >= 2) {
                0
            } else {
                let r = measure.checked_sub(w).ok_or(Error::CoordinateOverflow)?;
                let r = u128::try_from(r).map_err(|_| Error::CoordinateOverflow)?;
                r.checked_mul(r).ok_or(Error::CoordinateOverflow)?
            };
            let Some(prefix) = best[i] else {
                continue;
            };
            let total = prefix.checked_add(slack).ok_or(Error::CoordinateOverflow)?;
            match best[j] {
                None => {
                    best[j] = Some(total);
                    prev[j] = i;
                }
                Some(cur) if total <= cur => {
                    best[j] = Some(total);
                    prev[j] = i;
                }
                _ => {}
            }
        }
    }
    let raggedness = best[n].ok_or(Error::CoordinateOverflow)?;
    let mut ends = Vec::new();
    let mut j = n;
    while j > 0 {
        let i = prev[j];
        ends.push((i, j));
        j = i;
    }
    ends.reverse();
    Ok((ends, raggedness))
}

fn wrap_words(cx: &Cx<'_>, words: &Words) -> Result<(Vec<LinePlan>, WrapCost), Error> {
    let n = words.len();
    let mut ink = vec![0i64; n + 1];
    for i in 0..n {
        ink[i + 1] = ink[i]
            .checked_add(token_chain_width(cx, words.item(i))?.units())
            .ok_or(Error::CoordinateOverflow)?;
    }
    let mut gsum = vec![0i64; n];
    for i in 0..n.saturating_sub(1) {
        let gap = match words.join_before(i + 1) {
            Join::Space => cx.space.units(),
            _ => 0,
        };
        gsum[i + 1] = gsum[i].checked_add(gap).ok_or(Error::CoordinateOverflow)?;
    }
    let width = |i: usize, j: usize| {
        let gaps = if j == 0 {
            0
        } else {
            gsum[j.saturating_sub(1)]
                .checked_sub(gsum[i])
                .ok_or(Error::CoordinateOverflow)?
        };
        ink[j]
            .checked_sub(ink[i])
            .and_then(|v| v.checked_add(gaps))
            .and_then(|v| v.checked_add(trailing_overhang(words.item(j - 1)).units()))
            .and_then(|v| v.checked_sub(words.item(i).first.atom.bounds.x0().units().min(0)))
            .ok_or(Error::CoordinateOverflow)
    };
    let (ends, raggedness) = break_ranges(n, width, cx.measure_u)?;
    let extra_lines =
        u32::try_from(ends.len().saturating_sub(1)).map_err(|_| Error::CoordinateOverflow)?;
    let mut lines = Vec::new();
    lines.try_reserve(ends.len()).map_err(|_| Error::Alloc)?;
    for (i, j) in ends {
        let chain = words.slice(i, j);
        lines.push(place_tokens(cx, &chain)?);
    }
    Ok((
        lines,
        WrapCost {
            extra_lines,
            raggedness,
        },
    ))
}

fn place_tokens(cx: &Cx<'_>, chain: &Chain<Token>) -> Result<LinePlan, Error> {
    let atoms = Chain {
        first: chain.first.atom.clone(),
        rest: chain
            .rest
            .iter()
            .map(|(j, t)| (*j, t.atom.clone()))
            .collect(),
    };
    place_chain(cx, &atoms)
}

fn empty_plans(size: TextSize) -> Result<LinePlans, Error> {
    Ok(LinePlans {
        lines: vec![LinePlan::Blank {
            slug: blank_slug(size)?,
        }],
        cost: WrapCost::default(),
    })
}

/// Normalized wrap against the frozen [`wrap_text`](super::plan::wrap_text) signature.
pub(crate) fn wrap_text(
    size: TextSize,
    spans: &[Span<'_>],
    measure: Measure,
    faces: &ResolvedFaces<'_>,
    digits: DigitMode,
) -> Result<LinePlans, Error> {
    wrap_body(size, spans, measure, faces, digits)
}

/// Unwrapped natural width: each hard-break chunk’s token-chain advance, no
/// line-break search. This is the column preference, not a finite probe.
pub(crate) fn natural_advance(
    size: TextSize,
    spans: &[Span<'_>],
    fit: Measure,
    faces: &ResolvedFaces<'_>,
    digits: DigitMode,
) -> Result<Advance, Error> {
    let mut cx = Cx::new(size, fit, faces, digits)?;
    cx.measure_u = i64::MAX / 4;
    let mut best = Advance::ZERO;
    for chunk in normalize(spans) {
        if let Some(ink) = tokenize(&cx, &chunk)? {
            let w = token_chain_extent(&cx, &ink)?;
            if w.units() > best.units() {
                best = w;
            }
        }
    }
    Ok(best)
}

/// Same wrap as [`wrap_text`], but the break measure is an Advance so a caller
/// can wrap at a width that is not a [`Measure`]. Math still fits at `fit`.
#[allow(dead_code)]
pub(crate) fn wrap_to_advance(
    size: TextSize,
    spans: &[Span<'_>],
    measure: Advance,
    fit: Measure,
    faces: &ResolvedFaces<'_>,
    digits: DigitMode,
) -> Result<LinePlans, Error> {
    let mut cx = Cx::new(size, fit, faces, digits)?;
    cx.measure_u = measure.units();
    wrap_with(&cx, spans)
}

fn wrap_body(
    size: TextSize,
    spans: &[Span<'_>],
    measure: Measure,
    faces: &ResolvedFaces<'_>,
    digits: DigitMode,
) -> Result<LinePlans, Error> {
    let cx = Cx::new(size, measure, faces, digits)?;
    wrap_with(&cx, spans)
}

fn wrap_with(cx: &Cx<'_>, spans: &[Span<'_>]) -> Result<LinePlans, Error> {
    let mut lines = Vec::new();
    let mut cost = WrapCost::default();
    for chunk in normalize(spans) {
        match tokenize(cx, &chunk)? {
            None => lines.push(LinePlan::Blank {
                slug: blank_slug(cx.size)?,
            }),
            Some(ink) => {
                let words = split_words(cx, ink)?;
                let (chunk_lines, chunk_cost) = wrap_words(cx, &words)?;
                cost = cost
                    .checked_add(chunk_cost)
                    .ok_or(Error::CoordinateOverflow)?;
                lines.extend(chunk_lines);
            }
        }
    }
    if lines.is_empty() {
        return empty_plans(cx.size);
    }
    Ok(LinePlans { lines, cost })
}

/// Roman word space at `size`. A [`Join::Space`] is this width even after math.
#[allow(dead_code)]
pub(crate) fn word_space(faces: &ResolvedFaces<'_>, size: TextSize) -> Result<Advance, Error> {
    Ok(faces.text(Cut::Roman)?.shape_run(" ", size)?.advance())
}

/// Display wrapping keeps its own shaping context (tracking, case, whole-line
/// reshape). The line-break objective matches the paragraph DP.
pub(crate) fn wrap_display(
    cut: DisplayCut,
    size: DisplaySize,
    tracking: i16,
    text: &str,
    measure: Measure,
    faces: &ResolvedFaces<'_>,
) -> Result<LinePlans, Error> {
    let face = faces.display(cut)?;
    let measure_u = i64::from(measure.get())
        .checked_mul(FR)
        .ok_or(Error::CoordinateOverflow)?;
    let mut lines = Vec::new();
    let mut cost = WrapCost::default();
    for hard in text.split('\n') {
        let words: Vec<&str> = hard.split(' ').filter(|w| !w.is_empty()).collect();
        if words.is_empty() {
            lines.push(LinePlan::Blank {
                slug: display_slug(size)?,
            });
            continue;
        }
        let (chunk_lines, chunk_cost) =
            wrap_display_words(face, size, tracking, &words, measure_u)?;
        cost = cost
            .checked_add(chunk_cost)
            .ok_or(Error::CoordinateOverflow)?;
        lines.extend(chunk_lines);
    }
    if lines.is_empty() {
        lines.push(LinePlan::Blank {
            slug: display_slug(size)?,
        });
    }
    Ok(LinePlans { lines, cost })
}

fn wrap_display_words(
    face: &DisplayFace,
    size: DisplaySize,
    tracking: i16,
    words: &[&str],
    measure_u: i64,
) -> Result<(Vec<LinePlan>, WrapCost), Error> {
    let n = words.len();
    let width = |i: usize, j: usize| -> Result<i64, Error> {
        let mut scratch = String::new();
        for (k, word) in words[i..j].iter().enumerate() {
            if k > 0 {
                scratch.push(' ');
            }
            scratch.push_str(word);
        }
        Ok(face.shape_run(&scratch, size, tracking)?.advance().units())
    };
    let (ends, raggedness) = break_ranges(n, width, measure_u)?;
    let extra_lines =
        u32::try_from(ends.len().saturating_sub(1)).map_err(|_| Error::CoordinateOverflow)?;
    let mut lines = Vec::new();
    lines.try_reserve(ends.len()).map_err(|_| Error::Alloc)?;
    for (i, j) in ends {
        let mut scratch = String::new();
        for (k, word) in words[i..j].iter().enumerate() {
            if k > 0 {
                scratch.push(' ');
            }
            scratch.push_str(word);
        }
        lines.push(place_display_line(face, size, tracking, &scratch)?);
    }
    Ok((
        lines,
        WrapCost {
            extra_lines,
            raggedness,
        },
    ))
}

fn place_display_line(
    face: &DisplayFace,
    size: DisplaySize,
    tracking: i16,
    text: &str,
) -> Result<LinePlan, Error> {
    if text.is_empty() {
        return Ok(LinePlan::Blank {
            slug: display_slug(size)?,
        });
    }
    let run = face.shape_run(text, size, tracking)?;
    let bounds = run.ink();
    let advance = run.advance();
    let atom = PaintAtom::Glyph {
        run: Arc::new(run),
        x: Advance::ZERO,
        bounds,
    };
    Ok(LinePlan::Ink {
        atoms: NonEmpty::singleton(atom),
        advance,
        bounds,
    })
}

/// Ascent above the baseline from every atom, including raised notes.
pub(crate) fn plan_ascent(plan: &LinePlan) -> Advance {
    match plan {
        LinePlan::Blank { .. } => Advance::ZERO,
        LinePlan::Ink { bounds, .. } => {
            if bounds.is_empty() {
                Advance::ZERO
            } else {
                Advance::from_units((-bounds.y0().units()).max(0))
            }
        }
    }
}

/// Depth below the baseline from every atom.
pub(crate) fn plan_depth(plan: &LinePlan) -> Advance {
    match plan {
        LinePlan::Blank { .. } => Advance::ZERO,
        LinePlan::Ink { bounds, .. } => {
            if bounds.is_empty() {
                Advance::ZERO
            } else {
                Advance::from_units(bounds.y1().units().max(0))
            }
        }
    }
}

/// Plus2 (or display solid) skip, maxed with the ink bbox.
pub(crate) fn plan_skip(size: TextSize, plan: &LinePlan) -> Result<u16, Error> {
    match plan {
        LinePlan::Blank { slug } => {
            let dots = slug.ceil_dots()?;
            u16::try_from(dots).map_err(|_| Error::CoordinateOverflow)
        }
        LinePlan::Ink { .. } => {
            let span = plan_ascent(plan)
                .checked_add(plan_depth(plan))
                .ok_or(Error::CoordinateOverflow)?;
            let bbox = span.ceil_dots()?;
            let bbox = u16::try_from(bbox.max(0)).map_err(|_| Error::CoordinateOverflow)?;
            Ok(size.skip_dots().max(bbox))
        }
    }
}

/// Longest line advance in the selected plan. Blank is zero.
pub(crate) fn max_line_advance(plans: &LinePlans) -> Result<Advance, Error> {
    let mut best = Advance::ZERO;
    for line in &plans.lines {
        if let LinePlan::Ink {
            advance, bounds, ..
        } = line
        {
            let extent = (*advance)
                .max(bounds.x1())
                .checked_sub(Advance::from_units(bounds.x0().units().min(0)))
                .ok_or(Error::CoordinateOverflow)?;
            best = best.max(extent);
        }
    }
    Ok(best)
}

/// First line's ascent, depth and skip, derived from all atoms.
#[allow(dead_code)]
pub(crate) fn first_line_metrics(
    size: TextSize,
    plans: &LinePlans,
) -> Result<(Advance, Advance, u16), Error> {
    let Some(first) = plans.lines.first() else {
        return Ok((Advance::ZERO, Advance::ZERO, size.skip_dots()));
    };
    Ok((
        plan_ascent(first),
        plan_depth(first),
        plan_skip(size, first)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        After, BREAK_AFTER, Join, Stance, break_ranges, frags, join_of, natural_advance,
        plan_ascent, plan_depth, wrap_text,
    };
    use crate::compose::plan::{DigitMode, LinePlan, WrapCost};
    use crate::face::{Cut, DisplayCut, FaceRequirements, FaceTable};
    use crate::frame::{Math, NoteId, Span};
    use crate::geometry::Measure;
    use crate::size::TextSize;
    use std::num::NonZeroU32;

    fn table() -> FaceTable {
        let mut table = FaceTable::new();
        table.absorb(std::fs::read("/System/Library/Fonts/Helvetica.ttc").expect("Helvetica.ttc"));
        table.absorb(std::fs::read("/System/Library/Fonts/Menlo.ttc").expect("Menlo.ttc"));
        table
    }

    fn house_req() -> FaceRequirements {
        let mut req = FaceRequirements::default();
        for cut in Cut::ALL {
            req.need_text(cut);
        }
        req.need_display(DisplayCut::Roman);
        req
    }

    fn resolved(table: &FaceTable) -> crate::face::ResolvedFaces<'_> {
        table.resolve(&house_req()).expect("house faces")
    }

    fn tape() -> Measure {
        Measure::TAPE
    }

    fn wrap<'a>(
        table: &'a FaceTable,
        spans: &[Span<'a>],
        measure: Measure,
    ) -> crate::compose::plan::LinePlans {
        wrap_text(
            TextSize::Pt11,
            spans,
            measure,
            &resolved(table),
            DigitMode::Proportional,
        )
        .expect("wrap")
    }

    fn ink_advance(plans: &crate::compose::plan::LinePlans) -> Vec<i64> {
        plans
            .lines
            .iter()
            .map(|line| match line {
                LinePlan::Blank { .. } => 0,
                LinePlan::Ink { advance, .. } => advance.units(),
            })
            .collect()
    }

    fn ink_atom_count(plans: &crate::compose::plan::LinePlans) -> Vec<usize> {
        plans
            .lines
            .iter()
            .map(|line| match line {
                LinePlan::Blank { .. } => 0,
                LinePlan::Ink { atoms, .. } => atoms.len(),
            })
            .collect()
    }

    #[test]
    fn uri_tokens_fragment_after_punctuation() {
        assert_eq!(frags("a/b/c"), ["a/", "b/", "c"]);
        assert_eq!(frags("x?y=1&z=2"), ["x?", "y=", "1&", "z=", "2"]);
        assert_eq!(frags("plainword"), ["plainword"]);
        assert_eq!(frags("end."), ["end."]);
        assert_eq!(frags("a//"), ["a/", "/"]);
        for c in BREAK_AFTER {
            assert_eq!(frags(&format!("x{c}y")), [format!("x{c}").as_str(), "y"]);
        }
    }

    #[test]
    fn slant_then_upright_is_italic_correct_not_a_space() {
        assert_eq!(
            join_of(After::Join, Stance::Slant, Stance::Upright),
            Join::ItalicCorrect
        );
        assert_eq!(
            join_of(After::Join, Stance::Upright, Stance::Slant),
            Join::None
        );
        assert_eq!(
            join_of(After::Join, Stance::Slant, Stance::Slant),
            Join::None
        );
        assert_eq!(
            join_of(After::Space, Stance::Slant, Stance::Upright),
            Join::Space
        );
    }

    #[test]
    fn wrap_cost_is_lexicographic_extra_lines_first() {
        let short = WrapCost {
            extra_lines: 0,
            raggedness: u128::MAX,
        };
        let tall = WrapCost {
            extra_lines: 1,
            raggedness: 0,
        };
        assert!(tall > short);
        assert_eq!(
            short.checked_add(tall),
            Some(WrapCost {
                extra_lines: 1,
                raggedness: u128::MAX,
            })
        );
    }

    #[test]
    fn roman_hello_same_context_split_matches() {
        let table = table();
        let whole = wrap(&table, &[Span::new(Cut::Roman, "hello")], tape());
        let part = wrap(
            &table,
            &[Span::new(Cut::Roman, "hel"), Span::new(Cut::Roman, "lo")],
            tape(),
        );
        assert_eq!(ink_advance(&part), ink_advance(&whole));
        assert_eq!(ink_atom_count(&part), ink_atom_count(&whole));
    }

    #[test]
    fn roman_av_every_same_context_split_matches() {
        let table = table();
        let whole = wrap(&table, &[Span::new(Cut::Roman, "AV")], tape());
        let splits: Vec<(String, String)> = {
            let s = "AV";
            s.char_indices()
                .map(|(i, _)| (s[..i].to_string(), s[i..].to_string()))
                .chain(std::iter::once((s.to_string(), String::new())))
                .collect()
        };
        for (a, b) in splits {
            let mut spans = Vec::new();
            if !a.is_empty() {
                spans.push(Span::new(Cut::Roman, a.clone()));
            }
            if !b.is_empty() {
                spans.push(Span::new(Cut::Roman, b.clone()));
            }
            let part = wrap(&table, &spans, tape());
            assert_eq!(
                ink_advance(&part),
                ink_advance(&whole),
                "split {a:?}+{b:?} must shape like AV"
            );
            assert_eq!(
                ink_atom_count(&part),
                ink_atom_count(&whole),
                "split {a:?}+{b:?} must be one run"
            );
        }
    }

    #[test]
    fn different_cut_and_separate_code_boxes_stay_barriers() {
        let table = table();
        let av = wrap(&table, &[Span::new(Cut::Roman, "AV")], tape());
        let mixed = wrap(
            &table,
            &[Span::new(Cut::Roman, "A"), Span::new(Cut::Italic, "V")],
            tape(),
        );
        assert_eq!(ink_atom_count(&mixed), vec![2]);
        assert_ne!(ink_advance(&mixed), ink_advance(&av));

        let boxes = wrap(
            &table,
            &[Span::new(Cut::Mono, "A"), Span::new(Cut::Mono, "V")],
            tape(),
        );
        assert_eq!(
            ink_atom_count(&boxes),
            vec![2],
            "authored code boxes stay distinct"
        );
        let one = wrap(&table, &[Span::new(Cut::Mono, "AV")], tape());
        assert_eq!(ink_atom_count(&one), vec![1]);
        let narrow = Measure::new(20).unwrap();
        let two_n = wrap(
            &table,
            &[Span::new(Cut::Mono, "A"), Span::new(Cut::Mono, "V")],
            narrow,
        );
        let one_n = wrap(&table, &[Span::new(Cut::Mono, "AV")], narrow);
        assert!(
            two_n.lines.len() > one_n.lines.len(),
            "separate code boxes wrap independently ({} vs {})",
            two_n.lines.len(),
            one_n.lines.len()
        );
    }

    #[test]
    fn note_only_line_has_raised_ink() {
        let table = table();
        let id = NoteId::from_index(NonZeroU32::new(1).unwrap());
        let plans = wrap(&table, &[Span::note(id)], tape());
        assert_eq!(plans.lines.len(), 1);
        let LinePlan::Ink { bounds, .. } = &plans.lines[0] else {
            panic!("note-only must be ink, not a blank slug");
        };
        assert!(!bounds.is_empty(), "raised note contributes ink");
        assert!(
            bounds.y0().units() < 0,
            "note ink sits above the baseline: y0={}",
            bounds.y0().units()
        );
        assert!(
            plan_ascent(&plans.lines[0]).units() > 0,
            "first baseline must clear the raised reference"
        );
    }

    #[test]
    fn lowercase_plus_note_bounds_cover_both() {
        let table = table();
        let id = NoteId::from_index(NonZeroU32::new(2).unwrap());
        let plans = wrap(
            &table,
            &[Span::new(Cut::Roman, "g"), Span::note(id)],
            tape(),
        );
        let LinePlan::Ink { bounds, atoms, .. } = &plans.lines[0] else {
            panic!("expected ink");
        };
        assert_eq!(atoms.len(), 2);
        assert!(plan_depth(&plans.lines[0]).units() > 0, "g has depth");
        assert!(
            bounds.y0().units() < 0,
            "note raise participates in the union"
        );
        for atom in atoms {
            let b = match atom {
                crate::compose::plan::PaintAtom::Glyph { bounds, .. }
                | crate::compose::plan::PaintAtom::Bitmap { bounds, .. }
                | crate::compose::plan::PaintAtom::Rule { bounds } => *bounds,
            };
            assert!(
                b.y0().units() >= bounds.y0().units() && b.y1().units() <= bounds.y1().units(),
                "atom ink must lie inside the line bounds"
            );
        }
    }

    #[test]
    fn type_note_and_span_note_both_occur() {
        let table = table();
        let n1 = NoteId::from_index(NonZeroU32::new(1).unwrap());
        let n2 = NoteId::from_index(NonZeroU32::new(2).unwrap());
        let plans = wrap(
            &table,
            &[Span::new(Cut::Roman, "See"), Span::note(n1), Span::note(n2)],
            tape(),
        );
        assert_eq!(ink_atom_count(&plans), vec![3], "text + both references");
        let empty_carrier = wrap(&table, &[Span::note(n1)], tape());
        assert_eq!(
            ink_atom_count(&empty_carrier),
            vec![1],
            "independent note span emits only the reference"
        );
    }

    #[test]
    fn last_line_two_boxes_has_zero_raggedness() {
        let table = table();
        let one = wrap(&table, &[Span::new(Cut::Roman, "Hi")], tape());
        let two = wrap(&table, &[Span::new(Cut::Roman, "Hi Hi")], tape());
        assert_eq!(two.cost.extra_lines, 0);
        assert_eq!(two.cost.raggedness, 0, "last line with two boxes is free");
        assert_eq!(one.cost.extra_lines, 0);
        assert!(
            one.cost.raggedness > 0,
            "singleton last line retains slack², got {}",
            one.cost.raggedness
        );
        let slack = i64::from(tape().get()) * 64 - ink_advance(&one)[0];
        assert_eq!(one.cost.raggedness, u128::try_from(slack * slack).unwrap());
    }

    #[test]
    fn natural_advance_is_unwrapped_width() {
        let table = table();
        let text = "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello";
        let at_tape = wrap(&table, &[Span::new(Cut::Roman, text)], tape());
        let faces = resolved(&table);
        let natural = natural_advance(
            TextSize::Pt11,
            &[Span::new(Cut::Roman, text)],
            tape(),
            &faces,
            DigitMode::Proportional,
        )
        .expect("natural");
        assert!(
            natural.units() > i64::from(tape().get()) * 64,
            "twelve Helloes are wider than the tape"
        );
        assert!(
            at_tape.lines.len() > 1,
            "the same sentence wraps on the tape"
        );
        let long = "W".repeat(800);
        let long_w = natural_advance(
            TextSize::Pt11,
            &[Span::new(Cut::Roman, &long)],
            tape(),
            &faces,
            DigitMode::Proportional,
        )
        .expect("long natural");
        assert!(
            long_w.units() > 10_000 * 64,
            "natural width of 800 W exceeds the old 10_000-dot probe"
        );
    }

    #[test]
    fn hard_break_is_not_an_extra_wrap_line() {
        let table = table();
        let plans = wrap(&table, &[Span::new(Cut::Roman, "Hi\nHi")], tape());
        assert_eq!(plans.lines.len(), 2);
        assert_eq!(plans.cost.extra_lines, 0);
    }

    #[test]
    fn unbreakable_token_clips_explicitly() {
        let table = table();
        let measure = Measure::new(40).unwrap();
        let plans = wrap(
            &table,
            &[Span::new(Cut::Roman, "supercalifragilistic")],
            measure,
        );
        assert_eq!(plans.lines.len(), 1);
        assert_eq!(plans.cost.extra_lines, 0);
        assert_eq!(plans.cost.raggedness, 0, "overflow singleton costs zero");
        assert!(
            ink_advance(&plans)[0] > i64::from(measure.get()) * 64,
            "unbreakable token may exceed the measure"
        );
    }

    #[test]
    fn long_url_retains_legal_breaks() {
        let table = table();
        let url =
            "https://example.com/a/very/long/path/that/cannot/fit/on/one/line/of/the/tape/ever";
        let plans = wrap(&table, &[Span::new(Cut::Italic, url)], tape());
        assert!(
            plans.cost.extra_lines >= 1,
            "URI punctuation must yield extra lines, got {}",
            plans.cost.extra_lines
        );
        assert!(plans.lines.len() >= 2);
    }

    #[test]
    fn fitting_mono_stays_atomic() {
        let table = table();
        let roman = wrap(&table, &[Span::new(Cut::Roman, "aa bb")], tape());
        let mono = wrap(&table, &[Span::new(Cut::Mono, "aa bb")], tape());
        assert_eq!(ink_atom_count(&roman), vec![2]);
        assert_eq!(ink_atom_count(&mono), vec![1], "fitting mono is one box");
    }

    #[test]
    fn break_ranges_tie_prefers_the_longer_line() {
        // widths 10, 10; measure 25; two boxes on one line cost 0 (last, n>=2)
        // vs 1+1. Equal-or-better longer cover wins because i walks downward
        // and `<=` replaces.
        let widths = [10i64, 10];
        let width = |i: usize, j: usize| Ok(widths[i..j].iter().sum::<i64>() + (j - i - 1) as i64);
        let (ends, rag) = break_ranges(2, width, 25).unwrap();
        assert_eq!(ends, vec![(0, 2)]);
        assert_eq!(rag, 0);
    }

    #[test]
    fn math_and_note_share_measured_bounds() {
        let table = table();
        let bits = [true; 64];
        let math = Math::from_bits(8, 8, &bits, 5).expect("math");
        let id = NoteId::from_index(NonZeroU32::new(1).unwrap());
        let plans = wrap_text(
            TextSize::Pt11,
            &[Span::math(math), Span::note(id)],
            tape(),
            &resolved(&table),
            DigitMode::Proportional,
        )
        .expect("locally fitted math");
        let LinePlan::Ink { bounds, atoms, .. } = &plans.lines[0] else {
            panic!("math+note is ink");
        };
        assert_eq!(atoms.len(), 2);
        assert!(
            plan_ascent(&plans.lines[0]).units() >= 5 * 64,
            "math ascent is in the first-line metric"
        );
        assert!(!bounds.is_empty());
        for atom in atoms {
            let b = match atom {
                crate::compose::plan::PaintAtom::Glyph { bounds, .. }
                | crate::compose::plan::PaintAtom::Bitmap { bounds, .. }
                | crate::compose::plan::PaintAtom::Rule { bounds } => *bounds,
            };
            assert!(b.y0().units() >= bounds.y0().units());
            assert!(b.y1().units() <= bounds.y1().units());
        }
    }

    #[test]
    #[allow(clippy::float_cmp)] // An upright font has exactly zero slant, not an approximation.
    fn roman_has_no_overhang() {
        let table = table();
        let roman = table.text(Cut::Roman).unwrap();
        let size = TextSize::Pt11;
        let run = roman.shape_run("Used.", size).unwrap();
        assert_eq!(run.overhang().units(), 0);
        assert_eq!(roman.inner().italic_tan(), 0.0);
    }

    #[test]
    fn oblique_carries_a_slant() {
        let table = table();
        let italic = table.text(Cut::Italic).unwrap();
        let bold_italic = table.text(Cut::BoldItalic).unwrap();
        assert!(italic.inner().italic_tan() > 0.1, "Helvetica Oblique slant");
        assert!(
            bold_italic.inner().italic_tan() > 0.1,
            "Helvetica BoldOblique slant"
        );
    }
}
