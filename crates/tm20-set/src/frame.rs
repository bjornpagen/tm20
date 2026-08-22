//! The document: a [`Sheet`] of [`Frame`]s. Ugly layouts have no variant.
//! Faces are named [`Cut`]s; [`crate::FaceTable`] says what those names mean.

use std::borrow::Cow;
use std::num::NonZeroU32;

use crate::error::Error;
use crate::face::{Cut, FaceTable};
use crate::leading::{GridSkip, GRID, TASK_BOX};
use crate::size::{DisplaySize, TextSize};
use tm20::PRINTABLE_DOTS;

/// List mark. Hyphen-minus in the copy is not a list.
pub const EN_DASH: &str = "\u{2013}";

/// Canvas width in dots. Half-open `[0, get)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measure(std::num::NonZeroU16);

impl Measure {
    pub const TAPE: Self = Self(std::num::NonZeroU16::new(PRINTABLE_DOTS).unwrap());

    pub fn new(n: u16) -> Option<Self> {
        std::num::NonZeroU16::new(n).map(Self)
    }

    pub fn get(self) -> u16 {
        self.0.get()
    }
}

/// Tracking in thousandths of an em. Only legal on [`Mark`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tracking(pub i16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkAlign {
    Start,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Thickness {
    One,
    Two,
}

impl Thickness {
    pub fn dots(self) -> u16 {
        match self {
            Thickness::One => 1,
            Thickness::Two => 2,
        }
    }
}

/// One voice on a wrapping line. Size lives on the block, not here.
#[derive(Clone)]
pub enum Span<'a> {
    Type {
        cut: Cut,
        text: Cow<'a, str>,
        note: Option<NonZeroU32>,
    },
    Math(Math),
}

impl<'a> Span<'a> {
    pub fn new(cut: Cut, text: impl Into<Cow<'a, str>>) -> Self {
        Self::Type {
            cut,
            text: text.into(),
            note: None,
        }
    }

    pub fn math(m: Math) -> Self {
        Self::Math(m)
    }

    #[must_use]
    pub fn noted(self, n: NonZeroU32) -> Self {
        match self {
            Self::Type { cut, text, .. } => Self::Type {
                cut,
                text,
                note: Some(n),
            },
            Self::Math(_) => panic!("a note attaches to type, not math"),
        }
    }
}

/// One paragraph. Flush left, rag right. Adjacent [`Frame::Text`] values
/// are adjacent paragraphs; compose inserts the blank line.
pub struct TextBlock<'a> {
    pub size: TextSize,
    pub spans: Vec<Span<'a>>,
}

impl<'a> TextBlock<'a> {
    pub fn plain(cut: Cut, size: TextSize, text: impl Into<Cow<'a, str>>) -> Self {
        Self {
            size,
            spans: vec![Span::new(cut, text)],
        }
    }
}

/// Body size, Bold upright, space above, none below.
pub struct Head<'a> {
    pub size: TextSize,
    pub text: Cow<'a, str>,
}

/// Display line. Wraps to the measure. Center is only legal here.
pub struct Mark<'a> {
    pub cut: Cut,
    pub size: DisplaySize,
    pub text: Cow<'a, str>,
    pub align: MarkAlign,
    pub tracking: Tracking,
}

/// Where a rule sits. Only the tape: a hung walk cannot name leftover dots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSpan {
    Tape,
}

/// A break. The span is the tape, not leftover measure after a hang.
pub struct Rule {
    pub thickness: Thickness,
    pub span: RuleSpan,
}

impl Rule {
    pub fn tape(thickness: Thickness) -> Self {
        Self {
            thickness,
            span: RuleSpan::Tape,
        }
    }
}

/// Ink in the cell. Last [`End`] hangs the table on the tape; all-Start is compact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColAlign {
    Start,
    End,
}

/// Two or three columns. [`ColAlign::End`] shapes with tabular figures.
/// Consecutive tables stay tight; a table after a [`Frame::Rule`] hangs.
pub struct Cols<'a> {
    pub size: TextSize,
    pub gutter: GridSkip,
    pub body: ColBody<'a>,
}

/// Column count is the variant. A one-column or four-column grid has no value.
pub enum ColBody<'a> {
    Two {
        align: [ColAlign; 2],
        rows: Vec<[Vec<Span<'a>>; 2]>,
    },
    Three {
        align: [ColAlign; 3],
        rows: Vec<[Vec<Span<'a>>; 3]>,
    },
}

impl<'a> Cols<'a> {
    pub fn two(
        size: TextSize,
        gutter: GridSkip,
        align: [ColAlign; 2],
        rows: Vec<[Vec<Span<'a>>; 2]>,
    ) -> Self {
        Self {
            size,
            gutter,
            body: ColBody::Two { align, rows },
        }
    }

    pub fn three(
        size: TextSize,
        gutter: GridSkip,
        align: [ColAlign; 3],
        rows: Vec<[Vec<Span<'a>>; 3]>,
    ) -> Self {
        Self {
            size,
            gutter,
            body: ColBody::Three { align, rows },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecimalDelim {
    Period,
    Paren,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    Dash,
    Decimal { start: u32, delim: DecimalDelim },
}

/// List mark for one item. A task is not a nullable dash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemMark {
    List,
    Task { checked: bool },
}

/// Body of one item. An empty `Vec<Frame>` is not an item: it is [`Blank`].
pub enum ItemBody<'a> {
    /// One empty InkLine at the list face. The mark sits on this slug.
    Blank,
    Frames(Vec<Frame<'a>>),
}

impl<'a> ItemBody<'a> {
    pub fn from_frames(frames: Vec<Frame<'a>>) -> Self {
        if frames.is_empty() {
            Self::Blank
        } else {
            Self::Frames(frames)
        }
    }

    pub fn frames(&self) -> &[Frame<'a>] {
        match self {
            Self::Blank => &[],
            Self::Frames(frames) => frames,
        }
    }
}

/// One list item. A task replaces the dash or decimal with a drawn checkbox.
pub struct ListItem<'a> {
    pub mark: ItemMark,
    pub body: ItemBody<'a>,
}

impl<'a> ListItem<'a> {
    pub fn new(frames: Vec<Frame<'a>>) -> Self {
        Self {
            mark: ItemMark::List,
            body: ItemBody::from_frames(frames),
        }
    }

    pub fn task(checked: bool, frames: Vec<Frame<'a>>) -> Self {
        Self {
            mark: ItemMark::Task { checked },
            body: ItemBody::from_frames(frames),
        }
    }

    pub fn frames(&self) -> &[Frame<'a>] {
        self.body.frames()
    }
}

/// CommonMark list density. A bool would not say which way is tight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListFit {
    Tight,
    Loose,
}

/// Hanging list. Marker in the margin; runovers align with the text, not the mark.
pub struct List<'a> {
    pub size: TextSize,
    pub cut: Cut,
    pub marker: Marker,
    pub fit: ListFit,
    pub items: Vec<ListItem<'a>>,
}

impl List<'_> {
    /// Marker plus a word space, ceiled to [`GRID`]. Closed so dash, two-digit
    /// decimal, and a task box share a text column at this size.
    pub fn hang_dots(&self, faces: &FaceTable) -> Result<u16, Error> {
        let space = faces.text(self.cut)?.shape(" ", self.size).width;
        let grid = crate::size::to_frac(GRID);
        let units = ((self.mark_width(faces)? + space + grid - 1) / grid).max(1) as u16;
        Ok(units * GRID)
    }

    /// Width of the mark column, before the word space and grid leftover.
    /// Decimal figures right-align in this band; dash and task sit at its start.
    pub(crate) fn mark_width(&self, faces: &FaceTable) -> Result<i32, Error> {
        let face = faces.text(self.cut)?;
        let mut mark_w = face.shape(EN_DASH, self.size).width;
        mark_w = mark_w.max(face.shape_figure("10.", self.size).width);
        mark_w = mark_w.max(crate::size::to_frac(TASK_BOX));
        if let Marker::Decimal { start, delim } = self.marker {
            let n = self.items.len() as u32;
            for i in 0..n {
                let t = decimal_text(start.saturating_add(i), delim);
                mark_w = mark_w.max(face.shape_figure(&t, self.size).width);
            }
        }
        Ok(mark_w)
    }
}

pub(crate) fn decimal_text(n: u32, delim: DecimalDelim) -> String {
    match delim {
        DecimalDelim::Period => format!("{n}."),
        DecimalDelim::Paren => format!("{n})"),
    }
}

/// Block quote. Idle column, not a bar. Nested cap is three.
pub struct Quote<'a> {
    pub frames: Vec<Frame<'a>>,
}

/// Preformatted lines, hung by [`GRID`]. Not a paragraph: spaces do not wrap.
/// Tabs are column advances (every 8), parsed out so Menlo never sees U+0009.
pub struct Code<'a> {
    pub size: TextSize,
    pub lines: Vec<Cow<'a, str>>,
}

const TAB_STOP: usize = 8;

/// Expand U+0009 to the next multiple of [`TAB_STOP`] columns. A code line
/// that still contains a tab is a parse miss.
pub fn detab(s: &str) -> Cow<'_, str> {
    if !s.contains('\t') {
        return Cow::Borrowed(s);
    }
    let mut col = 0usize;
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '\t' {
            let n = TAB_STOP - (col % TAB_STOP);
            out.extend(std::iter::repeat_n(' ', n));
            col += n;
        } else {
            out.push(c);
            col = if c == '\n' { 0 } else { col + 1 };
        }
    }
    Cow::Owned(out)
}

fn bitmap(width: u16, height: u16, bits: &[bool]) -> Result<(u16, u16, Vec<u8>), Error> {
    if width == 0 || height == 0 || bits.len() != width as usize * height as usize {
        return Err(Error::Image);
    }
    let pixels = tm20::graphics::pack(width, height, bits).map_err(|_| Error::Image)?;
    Ok((width, height, pixels))
}

/// Photograph. Native size; shrinks if wider than the measure; centered if narrower.
/// `bits` is the protocol table: packed MSB-first, 1 is black.
#[derive(Clone)]
pub struct Figure {
    pub width: u16,
    pub height: u16,
    pub bits: Vec<u8>,
    pub note: Option<NonZeroU32>,
}

impl Figure {
    pub fn from_bits(width: u16, height: u16, bits: &[bool]) -> Result<Self, Error> {
        let (width, height, bits) = bitmap(width, height, bits)?;
        Ok(Self {
            width,
            height,
            bits,
            note: None,
        })
    }

    /// Decode PNG or JPEG at native size. Shrink if wider than `measure`; never scale up.
    /// Floyd–Steinberg to 1-bit after the size is settled.
    pub fn from_image(bytes: &[u8], measure: u16) -> Result<Self, Error> {
        let luma = decode_luma(bytes)?;
        let (w, h, samples) = fit_luma(&luma, measure)?;
        Ok(Self {
            width: w as u16,
            height: h as u16,
            bits: floyd_steinberg(w, h, samples),
            note: None,
        })
    }

    #[must_use]
    pub fn noted(mut self, n: NonZeroU32) -> Self {
        self.note = Some(n);
        self
    }
}

/// TeX box. Natural size; shrinks if wider than the measure; never scales up.
/// One token: there is no math-atom list to wrap.
/// `ascent` / `depth` are the TeX box (dots). A line’s slug reads these, not a
/// constant Plus2. `height` is the PNG; it may be smaller than ascent+depth.
/// `bits` is the protocol table: packed MSB-first, 1 is black.
#[derive(Clone)]
pub struct Math {
    pub width: u16,
    pub height: u16,
    pub bits: Vec<u8>,
    pub ascent: u16,
    pub depth: u16,
}

impl Math {
    pub fn from_bits(width: u16, height: u16, bits: &[bool], ascent: u16) -> Result<Self, Error> {
        let (width, height, bits) = bitmap(width, height, bits)?;
        let ascent = ascent.min(height);
        Ok(Self {
            width,
            height,
            bits,
            ascent,
            depth: height.saturating_sub(ascent),
        })
    }

    /// Decode PNG at native size. Shrink if wider than `max_w`; never scale up.
    /// `ascent` and `depth` are the TeX box in source pixels; both scale if the PNG shrinks.
    pub fn from_png(bytes: &[u8], max_w: u16, ascent: u16, depth: u16) -> Result<Self, Error> {
        let luma = decode_luma(bytes)?;
        let src_w = luma.width();
        let (dst_w, dst_h, samples) = fit_luma(&luma, max_w)?;
        let scale = dst_w as f32 / src_w as f32;
        let ascent = ((ascent as f32 * scale).round() as u16).min(dst_h as u16);
        let png_depth = (dst_h as u16).saturating_sub(ascent);
        let depth = ((depth as f32 * scale).round() as u16).max(png_depth);
        Ok(Self {
            width: dst_w as u16,
            height: dst_h as u16,
            bits: floyd_steinberg(dst_w, dst_h, samples),
            ascent,
            depth,
        })
    }
}

/// Luma on paper. Alpha is applied here: transparent is paper (255), not
/// dropped RGB. A figure cannot represent “RGB without alpha interpretation.”
fn decode_luma(bytes: &[u8]) -> Result<image::GrayImage, Error> {
    let img = image::load_from_memory(bytes).map_err(|_| Error::Image)?;
    let luma = paper_gray(&img);
    if luma.width() == 0 || luma.height() == 0 {
        return Err(Error::Image);
    }
    Ok(luma)
}

fn paper_gray(img: &image::DynamicImage) -> image::GrayImage {
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    image::GrayImage::from_fn(width, height, |x, y| {
        let pixel = rgba.get_pixel(x, y).0;
        let ink =
            (u32::from(pixel[0]) * 77 + u32::from(pixel[1]) * 150 + u32::from(pixel[2]) * 29) >> 8;
        let paper = 255u32;
        let out = (ink * u32::from(pixel[3]) + paper * (255 - u32::from(pixel[3]))) / 255;
        image::Luma([out as u8])
    })
}

/// Native size if it already fits `max_w`. Otherwise shrink. Never scale up.
fn fit_luma(luma: &image::GrayImage, max_w: u16) -> Result<(u32, u32, Vec<f32>), Error> {
    if max_w == 0 {
        return Err(Error::Image);
    }
    let (src_w, src_h) = luma.dimensions();
    let max_w = u32::from(max_w);
    if src_w <= max_w {
        let samples: Vec<f32> = luma.pixels().map(|p| p.0[0] as f32).collect();
        Ok((src_w, src_h, samples))
    } else {
        let dst_w = max_w;
        let dst_h = ((src_h as f32 * dst_w as f32 / src_w as f32).round() as u32).max(1);
        let resized =
            image::imageops::resize(luma, dst_w, dst_h, image::imageops::FilterType::Triangle);
        let samples: Vec<f32> = resized.pixels().map(|p| p.0[0] as f32).collect();
        Ok((dst_w, dst_h, samples))
    }
}

fn floyd_steinberg(w: u32, h: u32, mut px: Vec<f32>) -> Vec<u8> {
    let stride = tm20::graphics::width_bytes(w as u16);
    let mut bits = vec![0u8; stride * h as usize];
    let idx = |x: u32, y: u32| (y * w + x) as usize;
    for y in 0..h {
        for x in 0..w {
            let i = idx(x, y);
            let old = px[i].clamp(0.0, 255.0);
            let new = if old < 128.0 { 0.0 } else { 255.0 };
            if new == 0.0 {
                tm20::graphics::set_black(&mut bits, stride, x as usize, y as usize);
            }
            let err = old - new;
            if x + 1 < w {
                px[idx(x + 1, y)] += err * 7.0 / 16.0;
            }
            if y + 1 < h {
                if x > 0 {
                    px[idx(x - 1, y + 1)] += err * 3.0 / 16.0;
                }
                px[idx(x, y + 1)] += err * 5.0 / 16.0;
                if x + 1 < w {
                    px[idx(x + 1, y + 1)] += err * 1.0 / 16.0;
                }
            }
        }
    }
    bits
}

/// One node of the typesetting language.
pub enum Frame<'a> {
    Text(TextBlock<'a>),
    Head(Head<'a>),
    Mark(Mark<'a>),
    Cols(Cols<'a>),
    List(List<'a>),
    Quote(Quote<'a>),
    Code(Code<'a>),
    Figure(Figure),
    Math(Math),
    Rule(Rule),
}

/// One slot in the sheet’s note apparatus. Links, captions, and footnotes share the numbers.
pub enum Note<'a> {
    Dest {
        dest: Cow<'a, str>,
        title: Option<Cow<'a, str>>,
    },
    Blocks(Vec<Frame<'a>>),
}

impl<'a> Note<'a> {
    pub fn dest(dest: impl Into<Cow<'a, str>>) -> Self {
        Self::Dest {
            dest: dest.into(),
            title: None,
        }
    }
}

/// Authoring document. Compiles to one or more `Graphics`.
pub struct Sheet<'a> {
    pub width: Measure,
    pub frames: Vec<Frame<'a>>,
    /// 1-based; compose paints them after the frames. A type span’s note indexes this.
    pub notes: Vec<Note<'a>>,
}

impl<'a> Sheet<'a> {
    pub fn tape(frames: Vec<Frame<'a>>) -> Self {
        Self {
            width: Measure::TAPE,
            frames,
            notes: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_zero_is_none() {
        assert!(Measure::new(0).is_none());
        assert_eq!(Measure::new(1).unwrap().get(), 1);
        assert_eq!(Measure::TAPE.get(), PRINTABLE_DOTS);
    }

    #[test]
    fn empty_item_frames_are_blank() {
        assert!(matches!(ItemBody::from_frames(vec![]), ItemBody::Blank));
        assert!(matches!(ListItem::new(vec![]).body, ItemBody::Blank));
        assert_eq!(Rule::tape(Thickness::Two).span, RuleSpan::Tape);
    }

    #[test]
    fn figure_rejects_ragged_bits() {
        assert!(matches!(
            Figure::from_bits(2, 2, &[true]),
            Err(Error::Image)
        ));
        assert!(matches!(
            Figure::from_bits(0, 1, &[true]),
            Err(Error::Image)
        ));
    }

    fn gray_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::GrayImage::from_pixel(w, h, image::Luma([0]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn figure_keeps_native_size() {
        let fig = Figure::from_image(&gray_png(8, 4), 16).unwrap();
        assert_eq!(fig.width, 8);
        assert_eq!(fig.height, 4);
        assert!(fig.bits.iter().any(|&b| b != 0));
    }

    #[test]
    fn figure_already_the_measure_is_untouched() {
        let fig = Figure::from_image(&gray_png(8, 4), 8).unwrap();
        assert_eq!(fig.width, 8);
        assert_eq!(fig.height, 4);
    }

    #[test]
    fn figure_shrinks_to_the_measure() {
        let fig = Figure::from_image(&gray_png(8, 4), 4).unwrap();
        assert_eq!(fig.width, 4);
        assert_eq!(fig.height, 2);
    }

    #[test]
    fn figure_rejects_empty_measure_and_garbage() {
        let buf = gray_png(1, 1);
        assert!(matches!(Figure::from_image(&buf, 0), Err(Error::Image)));
        assert!(matches!(
            Figure::from_image(&[0xff; 8], 16),
            Err(Error::Image)
        ));
    }

    fn rgba_png(pixels: &[[u8; 4]]) -> Vec<u8> {
        let n = pixels.len() as u32;
        let side = (n as f32).sqrt() as u32;
        let img = image::RgbaImage::from_fn(side, side, |x, y| {
            image::Rgba(pixels[(y * side + x) as usize])
        });
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn figure_treats_transparent_as_paper() {
        // Black opaque + fully transparent: to_luma8 would make a black square.
        let buf = rgba_png(&[[0, 0, 0, 0], [0, 0, 0, 255], [0, 0, 0, 0], [0, 0, 0, 255]]);
        let fig = Figure::from_image(&buf, 16).unwrap();
        assert_eq!(fig.width, 2);
        assert_eq!(fig.height, 2);
        let stride = tm20::graphics::width_bytes(2);
        assert!(
            !tm20::graphics::is_black(&fig.bits, stride, 0, 0),
            "transparent is paper"
        );
        assert!(
            tm20::graphics::is_black(&fig.bits, stride, 1, 0),
            "opaque black is ink"
        );
    }

    #[test]
    fn detab_advances_to_column_eight() {
        assert_eq!(detab("col\tumn"), "col     umn");
        assert_eq!(detab("\tx"), "        x");
        assert_eq!(detab("no tabs"), "no tabs");
    }

    #[test]
    fn math_from_png_does_not_scale_up() {
        let img = image::GrayImage::from_pixel(4, 2, image::Luma([0]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        let m = Math::from_png(&buf, 16, 2, 0).unwrap();
        assert_eq!(m.width, 4);
        assert_eq!(m.height, 2);
        assert_eq!(m.ascent, 2);
        assert_eq!(m.depth, 0);
        let shrink = Math::from_png(&buf, 2, 2, 0).unwrap();
        assert_eq!(shrink.width, 2);
        assert!(shrink.height >= 1);
        assert!(shrink.ascent <= shrink.height);
    }
}
