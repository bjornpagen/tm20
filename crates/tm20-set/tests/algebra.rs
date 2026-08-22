//! Compose proofs. Each test is one typesetter fact. Faces come from the harness.

mod common;

use tm20::PRINTABLE_DOTS;
use tm20::graphics::{Graphics, width_bytes};
use tm20_set::{
    Code, ColAlign, Cols, Cut, DisplaySize, Figure, Frame, GRID, GridSkip, HANG, Head, List,
    ListFit, ListItem, Mark, MarkAlign, Math, NOTE_RULE, Note, Quote, Rule, Sheet, Span, TASK_BOX,
    TextBlock, TextSize, Thickness, Tracking, compose, preview_png, pt_dots,
};

fn l11() -> u16 {
    TextSize::Pt11.skip_dots()
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

fn packed_ink(g: &Graphics, y: usize, x: u16) -> bool {
    let stride = width_bytes(g.width_dots);
    let byte = g.pixels[y * stride + x as usize / 8];
    byte & (0x80 >> (x % 8)) != 0
}

fn leftmost_ink(g: &Graphics) -> u16 {
    for x in 0..g.width_dots {
        for y in 0..g.height_dots as usize {
            if packed_ink(g, y, x) {
                return x;
            }
        }
    }
    panic!("no ink")
}

fn ink_bbox(g: &Graphics) -> (u16, u16, usize, usize) {
    let mut x0 = g.width_dots;
    let mut x1 = 0u16;
    let mut y0 = g.height_dots as usize;
    let mut y1 = 0usize;
    for y in 0..g.height_dots as usize {
        for x in 0..g.width_dots {
            if packed_ink(g, y, x) {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    assert!(y1 >= y0, "no ink");
    (x0, x1, y0, y1)
}

fn rightmost_in(g: &Graphics, y0: usize, y1: usize) -> u16 {
    let mut x1 = 0u16;
    for y in y0..y1.min(g.height_dots as usize) {
        for x in (0..g.width_dots).rev() {
            if packed_ink(g, y, x) {
                x1 = x1.max(x);
                break;
            }
        }
    }
    x1
}

fn text(s: &str) -> Frame<'_> {
    Frame::Text(TextBlock::plain(Cut::Roman, TextSize::Pt11, s))
}

fn nest_quotes(depth: usize) -> Frame<'static> {
    if depth == 0 {
        text("H")
    } else {
        Frame::Quote(Quote {
            frames: vec![nest_quotes(depth - 1)],
        })
    }
}

fn nest_lists(depth: usize) -> Frame<'static> {
    if depth == 0 {
        text("H")
    } else {
        Frame::List(common::dash_list(vec![ListItem::new(vec![nest_lists(
            depth - 1,
        )])]))
    }
}

#[test]
fn compose_is_tape_wide() {
    let faces = common::table();
    let g = compose(&Sheet::tape(vec![text("Hello")]), &faces).unwrap();
    assert_eq!(g.width_dots, PRINTABLE_DOTS);
    assert!(g.pixels.iter().any(|&b| b != 0));
}

#[test]
fn closed_sizes_compose() {
    let faces = common::table();
    for size in [TextSize::Pt8, TextSize::Pt11] {
        compose(
            &Sheet::tape(vec![Frame::Text(TextBlock::plain(Cut::Roman, size, "H"))]),
            &faces,
        )
        .unwrap();
    }
    for size in [DisplaySize::Pt14, DisplaySize::Pt18, DisplaySize::Pt24] {
        compose(
            &Sheet::tape(vec![Frame::Mark(Mark {
                cut: Cut::Roman,
                size,
                text: "H".into(),
                align: MarkAlign::Start,
                tracking: Tracking(0),
            })]),
            &faces,
        )
        .unwrap();
    }
}

#[test]
fn wrap_makes_taller_than_one_line() {
    let faces = common::table();
    let one = compose(&Sheet::tape(vec![text("Hello")]), &faces).unwrap();
    let wrapped = compose(
        &Sheet::tape(vec![text(
            "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
        )]),
        &faces,
    )
    .unwrap();
    assert!(wrapped.height_dots > one.height_dots);
}

#[test]
fn wrap_first_line_hugs_the_measure() {
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![text(
            "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
        )]),
        &faces,
    )
    .unwrap();
    let skip = l11() as usize;
    let (_, _, y0, y1) = ink_bbox(&g);
    assert!(y1 - y0 > skip, "need at least two lines to judge the rag");
    let first = rightmost_in(&g, y0, y0 + skip);
    assert!(
        first as u32 * 10 >= g.width_dots as u32 * 7,
        "first line rightmost {first} should hug the measure {}, not an even rag",
        g.width_dots
    );
}

#[test]
fn wrap_last_line_is_not_a_widow() {
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![text(
            "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello you type.",
        )]),
        &faces,
    )
    .unwrap();
    let skip = l11() as usize;
    let (_, _, y0, y1) = ink_bbox(&g);
    assert!(y1 - y0 > skip, "need at least two lines");
    let last0 = y1.saturating_sub(skip);
    let last = rightmost_in(&g, last0, y1 + 1);
    let widow = compose(&Sheet::tape(vec![text("type.")]), &faces).unwrap();
    let (_, w1, _, _) = ink_bbox(&widow);
    assert!(
        last > w1 + 20,
        "last line rightmost {last} should hold more than a widow (type. ends at {w1})"
    );
}

#[test]
fn mono_span_does_not_break_at_its_spaces() {
    let faces = common::table();
    let filler = "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello";
    let roman = compose(
        &Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![Span::new(Cut::Roman, format!("{filler} aa bb"))],
        })]),
        &faces,
    )
    .unwrap();
    let mono = compose(
        &Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![
                Span::new(Cut::Roman, format!("{filler} ")),
                Span::new(Cut::Mono, "aa bb"),
            ],
        })]),
        &faces,
    )
    .unwrap();
    assert!(
        mono.height_dots >= roman.height_dots,
        "mono box {} should not split aa onto the first line the way roman {} can",
        mono.height_dots,
        roman.height_dots
    );
}

#[test]
fn two_paragraphs_are_a_blank_line_apart() {
    let faces = common::table();
    let lines = compose(&Sheet::tape(vec![text("H\nH")]), &faces).unwrap();
    let paras = compose(&Sheet::tape(vec![text("H"), text("H")]), &faces).unwrap();
    let extra = paras.height_dots as i32 - lines.height_dots as i32;
    let l = l11() as i32;
    assert!(
        extra >= l - 4 && extra <= l + 8,
        "paragraph extra {extra} should be one leading ({l}), not line skip"
    );
}

#[test]
fn head_sticks_to_the_following_text() {
    let faces = common::table();
    let stuck = compose(
        &Sheet::tape(vec![
            Frame::Head(Head {
                size: TextSize::Pt11,
                text: "H".into(),
            }),
            text("H"),
        ]),
        &faces,
    )
    .unwrap();
    let paras = compose(&Sheet::tape(vec![text("H"), text("H")]), &faces).unwrap();
    assert!(
        stuck.height_dots + 8 < paras.height_dots,
        "head+text {} should be tighter than two paragraphs {}",
        stuck.height_dots,
        paras.height_dots
    );
}

#[test]
fn mark_then_text_has_more_air_than_head_then_text() {
    let faces = common::table();
    let after_head = compose(
        &Sheet::tape(vec![
            Frame::Head(Head {
                size: TextSize::Pt11,
                text: "H".into(),
            }),
            text("H"),
        ]),
        &faces,
    )
    .unwrap();
    let after_mark = compose(
        &Sheet::tape(vec![
            Frame::Mark(Mark {
                cut: Cut::Roman,
                size: DisplaySize::Pt18,
                text: "H".into(),
                align: MarkAlign::Start,
                tracking: Tracking(0),
            }),
            text("H"),
        ]),
        &faces,
    )
    .unwrap();
    assert!(
        after_mark.height_dots > after_head.height_dots,
        "display contrast {} should exceed head stick {}",
        after_mark.height_dots,
        after_head.height_dots
    );
}

#[test]
fn mark_center_is_not_start() {
    let faces = common::table();
    let start = compose(
        &Sheet::tape(vec![Frame::Mark(Mark {
            cut: Cut::Roman,
            size: DisplaySize::Pt18,
            text: "H".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        })]),
        &faces,
    )
    .unwrap();
    let center = compose(
        &Sheet::tape(vec![Frame::Mark(Mark {
            cut: Cut::Roman,
            size: DisplaySize::Pt18,
            text: "H".into(),
            align: MarkAlign::Center,
            tracking: Tracking(0),
        })]),
        &faces,
    )
    .unwrap();
    assert!(leftmost_ink(&center) > leftmost_ink(&start) + 50);
}

#[test]
fn mark_tracking_widens() {
    let faces = common::table();
    let tight = compose(
        &Sheet::tape(vec![Frame::Mark(Mark {
            cut: Cut::Roman,
            size: DisplaySize::Pt18,
            text: "MM".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        })]),
        &faces,
    )
    .unwrap();
    let tracked = compose(
        &Sheet::tape(vec![Frame::Mark(Mark {
            cut: Cut::Roman,
            size: DisplaySize::Pt18,
            text: "MM".into(),
            align: MarkAlign::Start,
            tracking: Tracking(200),
        })]),
        &faces,
    )
    .unwrap();
    let (_, t1, _, _) = ink_bbox(&tight);
    let (_, k1, _, _) = ink_bbox(&tracked);
    assert!(k1 > t1, "tracking 200 should widen MM ({k1} vs {t1})");
}

#[test]
fn mark_wider_than_the_measure_wraps() {
    let faces = common::table();
    let one = compose(
        &Sheet::tape(vec![Frame::Mark(Mark {
            cut: Cut::Roman,
            size: DisplaySize::Pt18,
            text: "H".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        })]),
        &faces,
    )
    .unwrap();
    let wrapped = compose(
        &Sheet::tape(vec![Frame::Mark(Mark {
            cut: Cut::Roman,
            size: DisplaySize::Pt18,
            text: "Functional geometric algebra".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        })]),
        &faces,
    )
    .unwrap();
    assert!(
        wrapped.height_dots > one.height_dots,
        "a mark longer than the tape should wrap ({} vs {})",
        wrapped.height_dots,
        one.height_dots
    );
    let (_, right, _, _) = ink_bbox(&wrapped);
    assert!(
        right < PRINTABLE_DOTS,
        "wrapped mark should not clip at the tape edge (rightmost {right})"
    );
}

#[test]
fn rule_sits_below_the_line_slug() {
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![
            text("H"),
            Frame::Rule(Rule::tape(Thickness::One)),
        ]),
        &faces,
    )
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
fn rule_two_is_thicker_than_one() {
    let faces = common::table();
    let count = |t: Thickness| {
        let g = compose(
            &Sheet::tape(vec![Frame::Rule(Rule::tape(t))]),
            &faces,
        )
        .unwrap();
        (0..g.height_dots as usize)
            .filter(|&y| full_width_row(&g, y))
            .count()
    };
    assert_eq!(count(Thickness::One), 1);
    assert_eq!(count(Thickness::Two), 2);
}

#[test]
fn cols_has_ink_on_both_sides() {
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![Frame::Cols(common::cols(
            Cut::Roman,
            "Coffee",
            "$4.50",
        ))]),
        &faces,
    )
    .unwrap();
    let stride = width_bytes(g.width_dots);
    let mut left = false;
    let mut right = false;
    for row in 0..g.height_dots as usize {
        left |= g.pixels[row * stride] != 0;
        right |= g.pixels[row * stride + stride - 1] != 0;
    }
    assert!(left && right);
}

#[test]
fn cols_hangs_from_rule() {
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![
            Frame::Rule(Rule::tape(Thickness::Two)),
            Frame::Cols(common::cols(Cut::Roman, "H", "$1")),
        ]),
        &faces,
    )
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
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![
            Frame::Rule(Rule::tape(Thickness::Two)),
            text("H"),
        ]),
        &faces,
    )
    .unwrap();
    let gap = first_ink_after(&g, 2) - 2;
    assert!(
        gap >= l11() as usize - 4,
        "gap {gap} should be a line of the text, not hang ({HANG})"
    );
}

#[test]
fn consecutive_cols_are_tight() {
    let faces = common::table();
    let two = compose(
        &Sheet::tape(vec![
            Frame::Cols(common::cols(Cut::Roman, "A", "$1")),
            Frame::Cols(common::cols(Cut::Roman, "B", "$2")),
        ]),
        &faces,
    )
    .unwrap();
    let stacked = compose(
        &Sheet::tape(vec![Frame::Cols(Cols::two(
            TextSize::Pt11,
            GridSkip::ONE,
            [ColAlign::Start, ColAlign::End],
            vec![
                [
                    vec![Span::new(Cut::Roman, "A")],
                    vec![Span::new(Cut::Roman, "$1")],
                ],
                [
                    vec![Span::new(Cut::Roman, "B")],
                    vec![Span::new(Cut::Roman, "$2")],
                ],
            ],
        ))]),
        &faces,
    )
    .unwrap();
    let delta = (two.height_dots as i32 - stacked.height_dots as i32).abs();
    assert!(
        delta <= 4,
        "two Cols frames {} should match two rows {} (delta {delta})",
        two.height_dots,
        stacked.height_dots
    );
}

#[test]
fn cols_wrap_the_start_column() {
    let faces = common::table();
    let short = compose(
        &Sheet::tape(vec![Frame::Cols(common::cols(Cut::Roman, "A", "$1"))]),
        &faces,
    )
    .unwrap();
    let long = compose(
        &Sheet::tape(vec![Frame::Cols(common::cols(
            Cut::Roman,
            "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
            "$1",
        ))]),
        &faces,
    )
    .unwrap();
    assert!(long.height_dots > short.height_dots);
}

#[test]
fn three_columns_compose() {
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![Frame::Cols(Cols::three(
            TextSize::Pt11,
            GridSkip::ONE,
            [ColAlign::Start, ColAlign::Start, ColAlign::End],
            vec![[
                vec![Span::new(Cut::Roman, "A")],
                vec![Span::new(Cut::Roman, "B")],
                vec![Span::new(Cut::Roman, "$1")],
            ]],
        ))]),
        &faces,
    )
    .unwrap();
    assert!(g.pixels.iter().any(|&b| b != 0));
}

fn three_start(a: &'static str, b: &'static str, c: &'static str) -> Frame<'static> {
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

fn ink_runs(g: &Graphics, y0: usize, y1: usize) -> Vec<(u16, u16)> {
    let mut ink = vec![false; g.width_dots as usize];
    for y in y0..y1.min(g.height_dots as usize) {
        for x in 0..g.width_dots {
            if packed_ink(g, y, x) {
                ink[x as usize] = true;
            }
        }
    }
    let mut runs = Vec::new();
    let mut x = 0u16;
    while x < g.width_dots {
        if !ink[x as usize] {
            x += 1;
            continue;
        }
        let start = x;
        while x < g.width_dots && ink[x as usize] {
            x += 1;
        }
        runs.push((start, x));
    }
    runs
}

fn two_start(a: &'static str, b: &'static str) -> Frame<'static> {
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

#[test]
fn adjacent_cuts_do_not_grow_a_word_space() {
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![
                Span::new(Cut::Roman, "mid"),
                Span::new(Cut::Roman, "word"),
            ],
        })]),
        &faces,
    )
    .unwrap();
    let spaced = compose(
        &Sheet::tape(vec![Frame::Text(TextBlock::plain(
            Cut::Roman,
            TextSize::Pt11,
            "mid word",
        ))]),
        &faces,
    )
    .unwrap();
    let (_, tight_r, ..) = ink_bbox(&g);
    let (_, loose_r, ..) = ink_bbox(&spaced);
    assert!(
        tight_r + GRID < loose_r,
        "Glue::None run ({tight_r}) must be shorter than a word-spaced run ({loose_r})"
    );
}

#[test]
fn two_start_columns_keep_a_grid_gutter() {
    let faces = common::table();
    let g = compose(&Sheet::tape(vec![two_start("x", "y")]), &faces).unwrap();
    let merged = merge_runs(ink_runs(&g, 0, g.height_dots as usize));
    assert_eq!(merged.len(), 2, "two short Start columns; got {merged:?}");
    let gap = merged[1].0 - merged[0].1;
    assert!(
        gap < 64,
        "gutter is GRID, not leftover across the tape (gap {gap})"
    );
    assert!(
        gap >= GRID / 2,
        "columns should not touch (gap {gap}, GRID {GRID})"
    );
    let (_, right, ..) = ink_bbox(&g);
    assert!(
        u32::from(right) * 2 < u32::from(g.width_dots),
        "compact table sits left ({right} of {})",
        g.width_dots
    );
    assert!(leftmost_ink(&g) < GRID);
}

#[test]
fn start_columns_size_to_content() {
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![three_start(
            "0",
            "scalar",
            "discourse orientation now",
        )]),
        &faces,
    )
    .unwrap();
    let (_, _, y0, y1) = ink_bbox(&g);
    assert!(
        y1 - y0 + 1 < l11() as usize,
        "discourse orientation should stay one line (ink span {})",
        y1 - y0 + 1
    );
    let merged = merge_runs(ink_runs(&g, 0, g.height_dots as usize));
    assert!(
        merged.len() >= 4,
        "0, scalar, discourse, orientation; got {merged:?}"
    );
    let grade_w = merged[0].1 - merged[0].0;
    let object_w = merged[1].1 - merged[1].0;
    assert!(
        object_w > grade_w,
        "object box {object_w} should be wider than grade {grade_w}"
    );
    let gap_grade = merged[1].0 - merged[0].1;
    let gap_object = merged[2].0 - merged[1].1;
    assert!(
        gap_grade < 64,
        "gutter is GRID, not leftover (gap {gap_grade})"
    );
    let delta = i32::from(gap_grade) - i32::from(gap_object);
    assert!(
        delta.abs() <= 8,
        "air after 0 ({gap_grade}) and after scalar ({gap_object}) should match"
    );
}

fn merge_runs(runs: Vec<(u16, u16)>) -> Vec<(u16, u16)> {
    merge_runs_at(runs, 3)
}

fn merge_runs_at(runs: Vec<(u16, u16)>, join: u16) -> Vec<(u16, u16)> {
    let mut merged: Vec<(u16, u16)> = Vec::new();
    for r in runs {
        if let Some(last) = merged.last_mut()
            && r.0.saturating_sub(last.1) <= join
        {
            last.1 = r.1;
        } else {
            merged.push(r);
        }
    }
    merged
}

fn three_start_rows(rows: Vec<[&'static str; 3]>, header: bool) -> Frame<'static> {
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

fn ink_bands(g: &Graphics) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut y = 0usize;
    let h = g.height_dots as usize;
    while y < h {
        let mut ink = false;
        for x in 0..g.width_dots {
            if packed_ink(g, y, x) {
                ink = true;
                break;
            }
        }
        if !ink {
            y += 1;
            continue;
        }
        let y0 = y;
        while y < h {
            let mut row = false;
            for x in 0..g.width_dots {
                if packed_ink(g, y, x) {
                    row = true;
                    break;
                }
            }
            if !row {
                break;
            }
            y += 1;
        }
        out.push((y0, y));
    }
    out
}

#[test]
fn fga_grade_row_keeps_three_columns() {
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![three_start_rows(
            vec![
                ["Grade", "Object", "Meaning"],
                ["n", "pseudoscalar", "discourse orientation"],
            ],
            true,
        )]),
        &faces,
    )
    .unwrap();
    let bands = ink_bands(&g);
    assert!(
        (2..=3).contains(&bands.len()),
        "header plus a data row (meaning may wrap); got {} bands {bands:?}",
        bands.len()
    );
    let header = merge_runs_at(ink_runs(&g, bands[0].0, bands[0].1), GRID);
    let data = merge_runs_at(ink_runs(&g, bands[1].0, bands[1].1), GRID);
    assert_eq!(header.len(), 3, "Grade Object Meaning; got {header:?}");
    assert!(data.len() >= 3, "n, pseudoscalar, meaning; got {data:?}");
    let delta = i32::from(header[2].0) - i32::from(data[2].0);
    assert!(
        delta.abs() <= 2,
        "Meaning cells share a left edge (header {} vs data {})",
        header[2].0,
        data[2].0
    );
    assert!(
        header[1].1 - header[1].0 < data[1].1 - data[1].0,
        "Object ink should be narrower than pseudoscalar"
    );
}

#[test]
fn list_runover_clears_the_mark_column() {
    let faces = common::table();
    let list = common::dash_list(vec![common::item(
        "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
    )]);
    let hang = list.hang_dots(&faces).unwrap();
    assert_eq!(hang % GRID, 0);
    assert!(hang >= GRID);
    let g = compose(&Sheet::tape(vec![Frame::List(list)]), &faces).unwrap();
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
    assert!(
        runover,
        "wrapped line should sit at the hang, not under the dash"
    );
}

#[test]
fn decimal_hang_fits_the_widest_marker() {
    let faces = common::table();
    let dash = common::dash_list(vec![common::item("H"), common::item("H")]);
    let decimal = common::decimal_list(10, vec![common::item("H"), common::item("H")]);
    let dash_h = dash.hang_dots(&faces).unwrap();
    let dec_h = decimal.hang_dots(&faces).unwrap();
    assert_eq!(dec_h % GRID, 0);
    assert!(dec_h >= dash_h);
    assert_eq!(
        dash_h,
        common::decimal_list(1, vec![common::item("H")])
            .hang_dots(&faces)
            .unwrap(),
        "dash and one-digit decimal share a closed hang"
    );
    let g = compose(&Sheet::tape(vec![Frame::List(decimal)]), &faces).unwrap();
    let mut mark = false;
    for y in 0..g.height_dots as usize {
        for x in 0..dec_h {
            mark |= packed_ink(&g, y, x);
        }
    }
    assert!(mark, "decimal marker in the hang column");
}

#[test]
fn decimal_sits_in_the_mark_band() {
    let faces = common::table();
    let one = common::decimal_list(1, vec![common::item("H")]);
    let ten = common::decimal_list(10, vec![common::item("H")]);
    let task = common::dash_list(vec![ListItem::task(false, common::plain("H"))]);
    let hang = one.hang_dots(&faces).unwrap();
    let rightmost = |list: List<'_>| {
        let g = compose(&Sheet::tape(vec![Frame::List(list)]), &faces).unwrap();
        let mut x1 = 0u16;
        for y in 0..g.height_dots as usize {
            for x in 0..hang {
                if packed_ink(&g, y, x) {
                    x1 = x1.max(x);
                }
            }
        }
        x1
    };
    let d1 = rightmost(one);
    let d10 = rightmost(ten);
    let box_r = rightmost(task);
    assert!(
        d1 + 4 >= box_r.saturating_sub(4),
        "one-digit {d1} should sit in the task-box band (box {box_r}), not hug the text at hang {hang}"
    );
    assert!(
        hang > d1 + 8,
        "gutter after {d1} to hang {hang} should be more than a tight hug"
    );
    assert!(
        (d10 as i32 - d1 as i32).abs() <= 4,
        "1. and 10. should share a right edge ({d1} vs {d10})"
    );
}

#[test]
fn loose_list_is_taller_than_tight() {
    let faces = common::table();
    let tight = common::dash_list(vec![common::item("H"), common::item("H")]);
    let mut loose = common::dash_list(vec![common::item("H"), common::item("H")]);
    loose.fit = ListFit::Loose;
    let a = compose(&Sheet::tape(vec![Frame::List(tight)]), &faces).unwrap();
    let b = compose(&Sheet::tape(vec![Frame::List(loose)]), &faces).unwrap();
    assert!(
        b.height_dots > a.height_dots,
        "loose {} should exceed tight {}",
        b.height_dots,
        a.height_dots
    );
}

#[test]
fn two_texts_in_an_item_are_paragraphs() {
    let faces = common::table();
    let items = common::dash_list(vec![common::item("H"), common::item("H")]);
    let mut paras = common::dash_list(vec![ListItem::new(vec![text("H"), text("H")])]);
    paras.fit = ListFit::Tight;
    let a = compose(&Sheet::tape(vec![Frame::List(items)]), &faces).unwrap();
    let b = compose(&Sheet::tape(vec![Frame::List(paras)]), &faces).unwrap();
    let extra = b.height_dots as i32 - a.height_dots as i32;
    let l = l11() as i32;
    assert!(
        extra >= l - 4 && extra <= l + 8,
        "two texts in one item {} vs two tight items {} extra {extra} should be one leading ({l})",
        b.height_dots,
        a.height_dots
    );
}

#[test]
fn nested_tight_list_is_not_a_blank_taller() {
    let faces = common::table();
    let siblings = common::dash_list(vec![common::item("H"), common::item("H")]);
    let nested = common::dash_list(vec![ListItem::new(vec![
        text("H"),
        Frame::List(common::dash_list(vec![common::item("H")])),
    ])]);
    let a = compose(&Sheet::tape(vec![Frame::List(siblings)]), &faces).unwrap();
    let b = compose(&Sheet::tape(vec![Frame::List(nested)]), &faces).unwrap();
    let delta = (b.height_dots as i32 - a.height_dots as i32).abs();
    assert!(
        delta <= 8,
        "nested {} vs two tight siblings {} should share a slug, not a blank",
        b.height_dots,
        a.height_dots
    );
}

#[test]
fn sibling_ul_then_ol_is_not_a_blank_apart() {
    let faces = common::table();
    let dash = Frame::List(common::dash_list(vec![
        common::item("H"),
        common::item("H"),
    ]));
    let decimal = Frame::List(common::decimal_list(
        3,
        vec![common::item("H"), common::item("H")],
    ));
    let four = common::dash_list(vec![
        common::item("H"),
        common::item("H"),
        common::item("H"),
        common::item("H"),
    ]);
    let mixed = compose(&Sheet::tape(vec![dash, decimal]), &faces).unwrap();
    let tight = compose(&Sheet::tape(vec![Frame::List(four)]), &faces).unwrap();
    let extra = mixed.height_dots as i32 - tight.height_dots as i32;
    let l = l11() as i32;
    assert!(
        extra < l - 4,
        "ul then ol extra {extra} should not be a paragraph blank ({l}); mixed {} tight {}",
        mixed.height_dots,
        tight.height_dots
    );
}

#[test]
fn loose_item_two_paras_still_taller() {
    let faces = common::table();
    let mut tight = common::dash_list(vec![
        ListItem::new(vec![text("H"), text("H")]),
        ListItem::new(vec![text("H"), text("H")]),
    ]);
    tight.fit = ListFit::Tight;
    let mut loose = common::dash_list(vec![
        ListItem::new(vec![text("H"), text("H")]),
        ListItem::new(vec![text("H"), text("H")]),
    ]);
    loose.fit = ListFit::Loose;
    let a = compose(&Sheet::tape(vec![Frame::List(tight)]), &faces).unwrap();
    let b = compose(&Sheet::tape(vec![Frame::List(loose)]), &faces).unwrap();
    assert!(
        b.height_dots > a.height_dots,
        "loose two-para items {} should exceed tight {}",
        b.height_dots,
        a.height_dots
    );
}

#[test]
fn task_box_hangs_on_the_grid() {
    let faces = common::table();
    let list = common::dash_list(vec![ListItem::task(false, common::plain("H"))]);
    let hang = list.hang_dots(&faces).unwrap();
    assert_eq!(hang % GRID, 0);
    assert!(hang >= TASK_BOX);
}

#[test]
fn task_box_sits_in_the_cap_band() {
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![Frame::List(common::dash_list(vec![ListItem::task(
            false,
            common::plain("H"),
        )]))]),
        &faces,
    )
    .unwrap();
    let mut y0 = g.height_dots as usize;
    let mut y1 = 0usize;
    for y in 0..g.height_dots as usize {
        for x in 0..TASK_BOX {
            if packed_ink(&g, y, x) {
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    assert!(y1 >= y0, "task box has ink");
    let center = usize::midpoint(y0, y1);
    let (_, _, _, text_y1) = ink_bbox(&g);
    assert!(
        center < text_y1.saturating_sub(TASK_BOX as usize / 4),
        "box center {center} should sit in the cap band, not on the baseline (text bottom {text_y1})"
    );
}

#[test]
fn checked_task_has_more_ink_than_open() {
    let faces = common::table();
    let open = compose(
        &Sheet::tape(vec![Frame::List(common::dash_list(vec![ListItem::task(
            false,
            common::plain("H"),
        )]))]),
        &faces,
    )
    .unwrap();
    let done = compose(
        &Sheet::tape(vec![Frame::List(common::dash_list(vec![ListItem::task(
            true,
            common::plain("H"),
        )]))]),
        &faces,
    )
    .unwrap();
    let count = |g: &tm20::graphics::Graphics| {
        let mut n = 0;
        for y in 0..g.height_dots as usize {
            for x in 0..TASK_BOX {
                if packed_ink(g, y, x) {
                    n += 1;
                }
            }
        }
        n
    };
    assert!(
        count(&done) > count(&open),
        "a check adds ink inside the box"
    );
}

#[test]
fn three_lists_compose_a_fourth_does_not() {
    let faces = common::table();
    compose(&Sheet::tape(vec![nest_lists(3)]), &faces).unwrap();
    let err = compose(&Sheet::tape(vec![nest_lists(4)]), &faces).unwrap_err();
    assert!(matches!(err, tm20_set::Error::Nesting));
}

#[test]
fn nested_quote_is_not_a_blank_taller() {
    let faces = common::table();
    let nested = Frame::Quote(Quote {
        frames: vec![
            text("H"),
            Frame::Quote(Quote {
                frames: vec![text("H")],
            }),
        ],
    });
    let paras = compose(&Sheet::tape(vec![text("H"), text("H")]), &faces).unwrap();
    let quoted = compose(&Sheet::tape(vec![nested]), &faces).unwrap();
    assert!(
        quoted.height_dots + 8 < paras.height_dots,
        "nested quote {} should share a slug, not a paragraph blank ({})",
        quoted.height_dots,
        paras.height_dots
    );
}

#[test]
fn sibling_quotes_share_a_slug() {
    let faces = common::table();
    let quotes = compose(
        &Sheet::tape(vec![
            Frame::Quote(Quote {
                frames: vec![text("H")],
            }),
            Frame::Quote(Quote {
                frames: vec![text("H")],
            }),
        ]),
        &faces,
    )
    .unwrap();
    let paras = compose(&Sheet::tape(vec![text("H"), text("H")]), &faces).unwrap();
    assert!(
        quotes.height_dots + 8 < paras.height_dots,
        "sibling quotes {} should share a slug, not a paragraph blank ({})",
        quotes.height_dots,
        paras.height_dots
    );
}

#[test]
fn quote_then_code_share_a_slug() {
    let faces = common::table();
    let mixed = compose(
        &Sheet::tape(vec![
            Frame::Quote(Quote {
                frames: vec![text("H")],
            }),
            Frame::Code(Code {
                size: TextSize::Pt11,
                lines: vec!["H".into()],
            }),
        ]),
        &faces,
    )
    .unwrap();
    let paras = compose(&Sheet::tape(vec![text("H"), text("H")]), &faces).unwrap();
    assert!(
        mixed.height_dots + 8 < paras.height_dots,
        "quote then code {} should share a slug, not a paragraph blank ({})",
        mixed.height_dots,
        paras.height_dots
    );
}

#[test]
fn rule_in_a_quote_is_the_tape() {
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![Frame::Quote(Quote {
            frames: vec![Frame::Rule(Rule::tape(Thickness::Two))],
        })]),
        &faces,
    )
    .unwrap();
    let rows = (0..g.height_dots as usize)
        .filter(|&y| full_width_row(&g, y))
        .count();
    assert_eq!(rows, 2, "quote rule is tape-wide, not leftover hang");
}

#[test]
fn sibling_lists_take_a_grid_seam() {
    let faces = common::table();
    let one = compose(
        &Sheet::tape(vec![Frame::List(common::dash_list(vec![
            common::item("H"),
            common::item("H"),
            common::item("H"),
        ]))]),
        &faces,
    )
    .unwrap();
    let three = compose(
        &Sheet::tape(vec![
            Frame::List(common::dash_list(vec![common::item("H")])),
            Frame::List(common::dash_list(vec![common::item("H")])),
            Frame::List(common::dash_list(vec![common::item("H")])),
        ]),
        &faces,
    )
    .unwrap();
    let extra = three.height_dots as i32 - one.height_dots as i32;
    assert_eq!(
        extra, 2 * GRID as i32,
        "two new-list seams are 2×GRID ({} vs {})",
        three.height_dots, one.height_dots
    );
}

#[test]
fn empty_item_occupies_a_body_slug() {
    let faces = common::table();
    let full = compose(
        &Sheet::tape(vec![Frame::List(common::dash_list(vec![common::item("H")]))]),
        &faces,
    )
    .unwrap();
    let empty = compose(
        &Sheet::tape(vec![Frame::List(common::dash_list(vec![ListItem::new(
            vec![],
        )]))]),
        &faces,
    )
    .unwrap();
    let delta = (empty.height_dots as i32 - full.height_dots as i32).abs();
    assert!(
        delta <= 4,
        "blank item {} should share a body slug with a one-word item {}",
        empty.height_dots,
        full.height_dots
    );
    let mut mark = false;
    for y in 0..empty.height_dots as usize {
        for x in 0..24 {
            mark |= packed_ink(&empty, y, x);
        }
    }
    assert!(mark, "empty item still has a mark on the slug");
}

#[test]
fn inline_math_slug_follows_the_ink() {
    let faces = common::table();
    let bits = vec![true; 8 * 60];
    let tall = Math::from_bits(8, 60, &bits, 40).unwrap();
    let short = Math::from_bits(8, 8, &bits[..64], 6).unwrap();
    let with_frac = compose(
        &Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![Span::new(Cut::Roman, "x"), Span::math(tall)],
        })]),
        &faces,
    )
    .unwrap();
    let with_short = compose(
        &Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![Span::new(Cut::Roman, "x"), Span::math(short)],
        })]),
        &faces,
    )
    .unwrap();
    assert!(
        with_frac.height_dots >= 60,
        "tall inline math should grow the slug, got {}",
        with_frac.height_dots
    );
    assert!(
        with_short.height_dots <= l11() + 4,
        "short inline math should sit on the prose slug, got {}",
        with_short.height_dots
    );
}

#[test]
fn narrow_display_math_is_centered() {
    let faces = common::table();
    let bits = vec![true; 8 * 8];
    let math = Math::from_bits(8, 8, &bits, 6).unwrap();
    let g = compose(&Sheet::tape(vec![Frame::Math(math)]), &faces).unwrap();
    let (x0, x1, _, _) = ink_bbox(&g);
    let mid = u16::midpoint(x0, x1);
    assert!(
        (mid as i32 - PRINTABLE_DOTS as i32 / 2).abs() <= 8,
        "narrow display [{x0},{x1}] should center on the tape"
    );
}

#[test]
fn quote_hangs_by_the_grid() {
    let faces = common::table();
    let plain = compose(&Sheet::tape(vec![text("H")]), &faces).unwrap();
    let quoted = compose(
        &Sheet::tape(vec![Frame::Quote(Quote {
            frames: common::plain("H"),
        })]),
        &faces,
    )
    .unwrap();
    let shift = leftmost_ink(&quoted) as i32 - leftmost_ink(&plain) as i32;
    assert_eq!(shift, GRID as i32);
}

#[test]
fn code_hangs_by_the_grid() {
    let faces = common::table();
    let plain = compose(&Sheet::tape(vec![text("H")]), &faces).unwrap();
    let code = compose(
        &Sheet::tape(vec![Frame::Code(Code {
            size: TextSize::Pt11,
            lines: vec!["H".into()],
        })]),
        &faces,
    )
    .unwrap();
    let shift = leftmost_ink(&code) as i32 - leftmost_ink(&plain) as i32;
    assert!(
        (shift - GRID as i32).abs() <= 2,
        "code hang {shift} should be GRID plus Menlo sidebearing"
    );
}

#[test]
fn code_does_not_wrap() {
    let faces = common::table();
    let one = compose(
        &Sheet::tape(vec![Frame::Code(Code {
            size: TextSize::Pt11,
            lines: vec!["Hello".into()],
        })]),
        &faces,
    )
    .unwrap();
    let many = compose(
        &Sheet::tape(vec![Frame::Code(Code {
            size: TextSize::Pt11,
            lines: vec![
                "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello".into(),
            ],
        })]),
        &faces,
    )
    .unwrap();
    assert_eq!(many.height_dots, one.height_dots);
}

#[test]
fn three_quotes_compose_a_fourth_does_not() {
    let faces = common::table();
    compose(&Sheet::tape(vec![nest_quotes(3)]), &faces).unwrap();
    let err = compose(&Sheet::tape(vec![nest_quotes(4)]), &faces).unwrap_err();
    assert!(matches!(err, tm20_set::Error::Nesting));
}

#[test]
fn figure_blits_into_the_canvas() {
    let faces = common::table();
    let bits = vec![true; 8 * 8];
    let g = compose(
        &Sheet::tape(vec![Frame::Figure(Figure::from_bits(8, 8, &bits).unwrap())]),
        &faces,
    )
    .unwrap();
    let mut ink = 0;
    for y in 0..g.height_dots as usize {
        for x in 0..g.width_dots {
            if packed_ink(&g, y, x) {
                ink += 1;
            }
        }
    }
    assert!(ink > 0);
}

#[test]
fn figure_narrower_than_the_measure_is_centered() {
    let faces = common::table();
    let bits = vec![true; 8 * 8];
    let g = compose(
        &Sheet::tape(vec![Frame::Figure(Figure::from_bits(8, 8, &bits).unwrap())]),
        &faces,
    )
    .unwrap();
    let left = leftmost_ink(&g);
    let want = (PRINTABLE_DOTS - 8) / 2;
    assert_eq!(left, want);
}

#[test]
fn figure_as_wide_as_the_measure_is_a_full_slice() {
    let faces = common::table();
    let w = PRINTABLE_DOTS;
    let bits = vec![true; w as usize];
    let g = compose(
        &Sheet::tape(vec![Frame::Figure(Figure::from_bits(w, 1, &bits).unwrap())]),
        &faces,
    )
    .unwrap();
    assert_eq!(leftmost_ink(&g), 0);
}

#[test]
fn notes_follow_the_frames() {
    let faces = common::table();
    let span = Span::new(Cut::Italic, "Canon").noted(std::num::NonZeroU32::new(1).unwrap());
    let mut sheet = Sheet::tape(vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![span],
    })]);
    sheet.notes.push(Note::dest("https://example.com"));
    let g = compose(&sheet, &faces).unwrap();
    let mut rule = false;
    for y in 0..g.height_dots as usize {
        let mut ink = 0u16;
        let mut last = 0u16;
        for x in 0..g.width_dots {
            if packed_ink(&g, y, x) {
                ink += 1;
                last = x;
            }
        }
        if ink >= NOTE_RULE.saturating_sub(8) && last < g.width_dots / 2 {
            rule = true;
        }
    }
    assert!(rule, "notes sit after a short rule, not a full-tape rule");
}

#[test]
fn notes_rule_has_two_points_of_air() {
    let faces = common::table();
    let span = Span::new(Cut::Roman, "H").noted(std::num::NonZeroU32::new(1).unwrap());
    let mut sheet = Sheet::tape(vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![span],
    })]);
    sheet.notes.push(Note::dest("https://example.com"));
    let g = compose(&sheet, &faces).unwrap();
    let mut rule_y = None;
    for y in 0..g.height_dots as usize {
        let mut ink = 0u16;
        let mut last = 0u16;
        for x in 0..g.width_dots {
            if packed_ink(&g, y, x) {
                ink += 1;
                last = x;
            }
        }
        if ink >= NOTE_RULE.saturating_sub(8) && last < g.width_dots / 2 {
            rule_y = Some(y);
            break;
        }
    }
    let rule_y = rule_y.expect("short notes rule");
    let mut body_last = 0usize;
    for y in 0..rule_y {
        for x in 0..g.width_dots {
            if packed_ink(&g, y, x) {
                body_last = y;
            }
        }
    }
    let above = rule_y.saturating_sub(body_last + 1);
    let air = pt_dots(2.0) as usize;
    let blank = l11() as usize;
    assert!(
        above >= air && above < blank,
        "gap above notes rule {above} should include 2pt ({air}) and not a paragraph blank ({blank})"
    );
    let notes = first_ink_after(&g, rule_y + 1);
    let below = notes.saturating_sub(rule_y + 1);
    assert!(
        below >= air.saturating_sub(1) && below <= air + 4,
        "gap below notes rule {below} should be ~2pt ({air}), not HANG ({HANG})"
    );
}

#[test]
fn dest_title_is_two_lines() {
    let faces = common::table();
    let g1 = {
        let span = Span::new(Cut::Italic, "Canon").noted(std::num::NonZeroU32::new(1).unwrap());
        let mut sheet = Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![span],
        })]);
        sheet.notes.push(Note::dest("https://example.com"));
        compose(&sheet, &faces).unwrap()
    };
    let g2 = {
        let span = Span::new(Cut::Italic, "Canon").noted(std::num::NonZeroU32::new(1).unwrap());
        let mut sheet = Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![span],
        })]);
        sheet.notes.push(Note::Dest {
            dest: "https://example.com".into(),
            title: Some("The Canon".into()),
        });
        compose(&sheet, &faces).unwrap()
    };
    assert!(
        g2.height_dots > g1.height_dots,
        "title then dest is taller than dest alone ({} vs {})",
        g2.height_dots,
        g1.height_dots
    );
}

#[test]
fn figure_note_has_raised_ink() {
    let faces = common::table();
    let bits = vec![true; 8 * 8];
    let fig = Figure::from_bits(8, 8, &bits)
        .unwrap()
        .noted(std::num::NonZeroU32::new(1).unwrap());
    let mut sheet = Sheet::tape(vec![Frame::Figure(fig)]);
    sheet.notes.push(Note::dest("grid"));
    let g = compose(&sheet, &faces).unwrap();
    let left = leftmost_ink(&g);
    let after_bitmap = left + 8;
    let mut after = false;
    let max_y = 16.min(g.height_dots as usize);
    for y in 0..max_y {
        for x in after_bitmap..g.width_dots {
            if packed_ink(&g, y, x) {
                after = true;
            }
        }
    }
    assert!(after, "note sits after the bitmap");
}

#[test]
fn preview_png_is_a_png() {
    let faces = common::table();
    let g = compose(&Sheet::tape(vec![text("H")]), &faces).unwrap();
    let png = preview_png(&g).unwrap();
    assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
}

#[test]
fn dump_preview_pngs() {
    let faces = common::table();
    let dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tm20-preview");
    std::fs::create_dir_all(&dir).unwrap();
    let nested = common::dash_list(vec![ListItem::new(vec![
        text("outer"),
        Frame::List(common::decimal_list(1, vec![common::item("nested")])),
    ])]);
    let ul_ol = vec![
        Frame::List(common::dash_list(vec![common::item("dash")])),
        Frame::List(common::decimal_list(1, vec![common::item("one")])),
    ];
    let tasks = common::dash_list(vec![
        ListItem::task(false, common::plain("open")),
        ListItem::task(true, common::plain("done")),
    ]);
    let wrap = text(
        "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
    );
    let mut notes = Sheet::tape(vec![text("Body.")]);
    let span = Span::new(Cut::Italic, "Canon").noted(std::num::NonZeroU32::new(1).unwrap());
    notes.frames = vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![span],
    })];
    notes.notes.push(Note::dest("https://example.com"));
    let both = Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![
            Span::new(Cut::Roman, "Roman "),
            Span::new(Cut::Italic, "italic "),
            Span::new(Cut::Bold, "bold "),
            Span::new(Cut::BoldItalic, "both"),
        ],
    });
    let lilt = "lilt qt coco";
    let cases: [(&str, Sheet<'_>); 9] = [
        ("nested-list", Sheet::tape(vec![Frame::List(nested)])),
        ("ul-then-ol", Sheet::tape(ul_ol)),
        ("tasks", Sheet::tape(vec![Frame::List(tasks)])),
        ("wrap", Sheet::tape(vec![wrap])),
        ("notes", notes),
        ("bold-italic", Sheet::tape(vec![both])),
        (
            "lilt-8",
            Sheet::tape(vec![Frame::Text(TextBlock::plain(
                Cut::Roman,
                TextSize::Pt8,
                lilt,
            ))]),
        ),
        (
            "lilt-11",
            Sheet::tape(vec![Frame::Text(TextBlock::plain(
                Cut::Roman,
                TextSize::Pt11,
                lilt,
            ))]),
        ),
        (
            "lilt-18",
            Sheet::tape(vec![Frame::Mark(Mark {
                cut: Cut::Roman,
                size: DisplaySize::Pt18,
                text: lilt.into(),
                align: MarkAlign::Start,
                tracking: Tracking(0),
            })]),
        ),
    ];
    for (name, sheet) in cases {
        let g = compose(&sheet, &faces).unwrap();
        let png = preview_png(&g).unwrap();
        std::fs::write(dir.join(format!("{name}.png")), png).unwrap();
    }
}

#[test]
fn missing_cut_is_an_error() {
    let faces = tm20_set::FaceTable::new();
    let err = compose(
        &Sheet::tape(vec![Frame::Head(Head {
            size: TextSize::Pt11,
            text: "Hello".into(),
        })]),
        &faces,
    )
    .unwrap_err();
    assert!(matches!(err, tm20_set::Error::MissingText(Cut::Bold)));
}

#[test]
fn code_needs_mono() {
    let faces = tm20_set::FaceTable::new();
    let err = compose(
        &Sheet::tape(vec![Frame::Code(Code {
            size: TextSize::Pt11,
            lines: vec!["H".into()],
        })]),
        &faces,
    )
    .unwrap_err();
    assert!(matches!(err, tm20_set::Error::MissingText(Cut::Mono)));
}

#[test]
fn missing_display_is_an_error() {
    let faces = tm20_set::FaceTable::new();
    let err = compose(
        &Sheet::tape(vec![Frame::Mark(Mark {
            cut: Cut::Roman,
            size: DisplaySize::Pt18,
            text: "H".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        })]),
        &faces,
    )
    .unwrap_err();
    assert!(matches!(err, tm20_set::Error::MissingDisplay(Cut::Roman)));
}
