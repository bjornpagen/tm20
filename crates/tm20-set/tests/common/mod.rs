//! Shared typesetter integration helpers. Not a test binary.
//! Each integration binary uses a subset; unused items stay for the others.
#![allow(dead_code, unused_imports)]

pub mod faces;
pub mod page;

use tm20_set::{
    ColAlign, Cols, Cut, DecimalDelim, Frame, GridSkip, List, ListFit, ListItem, Marker, Span,
    TextBlock, TextSize,
};

pub use faces::{partial_table, require_locked_fonts, table};
pub use page::{
    compose_raster, concat_equals_page, first_ink_after, full_width_row, ink_bands, ink_bbox,
    ink_count, ink_runs, leftmost_ink, merge_runs, merge_runs_at, packed_ink, rightmost_in,
    row_has_ink,
};

pub fn text(s: &str) -> Frame<'_> {
    Frame::Text(TextBlock::plain(Cut::Roman, TextSize::Pt11, s))
}

pub fn cols<'a>(cut: Cut, item: &'a str, amount: &'a str) -> Cols<'a> {
    Cols::two(
        TextSize::Pt11,
        GridSkip::ONE,
        [ColAlign::Start, ColAlign::End],
        vec![[vec![Span::new(cut, item)], vec![Span::new(cut, amount)]]],
    )
}

pub fn plain(text: &str) -> Vec<Frame<'_>> {
    vec![self::text(text)]
}

pub fn item(text: &str) -> ListItem<'_> {
    ListItem::new(plain(text))
}

pub fn dash_list(items: Vec<ListItem<'_>>) -> List<'_> {
    List {
        size: TextSize::Pt11,
        cut: Cut::Roman,
        marker: Marker::Dash,
        fit: ListFit::Tight,
        items,
    }
}

pub fn decimal_list(start: u32, items: Vec<ListItem<'_>>) -> List<'_> {
    List {
        size: TextSize::Pt11,
        cut: Cut::Roman,
        marker: Marker::Decimal {
            start,
            delim: DecimalDelim::Period,
        },
        fit: ListFit::Tight,
        items,
    }
}

pub fn nest_quotes(depth: usize) -> Frame<'static> {
    if depth == 0 {
        text("H")
    } else {
        Frame::Quote(tm20_set::Quote {
            frames: vec![nest_quotes(depth - 1)],
        })
    }
}

pub fn nest_lists(depth: usize) -> Frame<'static> {
    if depth == 0 {
        text("H")
    } else {
        Frame::List(dash_list(vec![ListItem::new(vec![nest_lists(depth - 1)])]))
    }
}

pub fn l11() -> u16 {
    TextSize::Pt11.skip_dots()
}

pub fn three_start(a: &'static str, b: &'static str, c: &'static str) -> Frame<'static> {
    Frame::Cols(Cols::three(
        TextSize::Pt11,
        GridSkip::ONE,
        [ColAlign::Start, ColAlign::Start, ColAlign::Start],
        vec![[
            vec![Span::new(Cut::Roman, a)],
            vec![Span::new(Cut::Roman, b)],
            vec![Span::new(Cut::Roman, c)],
        ]],
    ))
}

pub fn two_start(a: &'static str, b: &'static str) -> Frame<'static> {
    Frame::Cols(Cols::two(
        TextSize::Pt11,
        GridSkip::ONE,
        [ColAlign::Start, ColAlign::Start],
        vec![[
            vec![Span::new(Cut::Roman, a)],
            vec![Span::new(Cut::Roman, b)],
        ]],
    ))
}

pub fn three_start_rows(rows: Vec<[&'static str; 3]>, header: bool) -> Frame<'static> {
    let body: Vec<[Vec<Span<'static>>; 3]> = rows
        .into_iter()
        .enumerate()
        .map(|(i, [a, b, c])| {
            let cut = if header && i == 0 {
                Cut::Bold
            } else {
                Cut::Roman
            };
            [
                vec![Span::new(cut, a)],
                vec![Span::new(cut, b)],
                vec![Span::new(cut, c)],
            ]
        })
        .collect();
    Frame::Cols(Cols::three(
        TextSize::Pt11,
        GridSkip::ONE,
        [ColAlign::Start, ColAlign::Start, ColAlign::Start],
        body,
    ))
}

pub fn uniq_temp(prefix: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "tm20-{prefix}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    p
}
