//! Compose proofs. Each test is one typesetter fact. Faces come from the harness.

mod common;

use tm20::PRINTABLE_DOTS;
use tm20::graphics::{Graphics, width_bytes};
use tm20_set::{
    Code, ColAlign, Cols, Cut, DisplaySize, Figure, Frame, GRID, GridSkip, HANG, Head, List,
    ListFit, ListItem, Mark, MarkAlign, NOTE_RULE, Note, Quote, Rule, Sheet, Span, TASK_BOX,
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
fn rule_sits_below_the_line_slug() {
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![
            text("H"),
            Frame::Rule(Rule {
                thickness: Thickness::One,
            }),
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
            &Sheet::tape(vec![Frame::Rule(Rule { thickness: t })]),
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
            Frame::Rule(Rule {
                thickness: Thickness::Two,
            }),
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
            Frame::Rule(Rule {
                thickness: Thickness::Two,
            }),
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
        "code hang {shift} should be GRID plus Commit Mono sidebearing"
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
        &Sheet::tape(vec![Frame::Figure(Figure::from_bits(8, 8, bits).unwrap())]),
        &faces,
    )
    .unwrap();
    let mut ink = 0;
    for y in 0..8 {
        for x in 0..8 {
            if packed_ink(&g, y, x) {
                ink += 1;
            }
        }
    }
    assert!(ink > 0);
}

#[test]
fn notes_follow_the_frames() {
    let faces = common::table();
    let span = Span::new(Cut::Italic, "Canon").noted(std::num::NonZeroU32::new(1).unwrap());
    let mut sheet = Sheet::tape(vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![span],
    })]);
    sheet.notes.push(Note::Dest("https://example.com".into()));
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
    sheet.notes.push(Note::Dest("https://example.com".into()));
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
    notes.notes.push(Note::Dest("https://example.com".into()));
    let both = Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![
            Span::new(Cut::Roman, "Roman "),
            Span::new(Cut::Italic, "italic "),
            Span::new(Cut::Bold, "bold "),
            Span::new(Cut::BoldItalic, "both"),
        ],
    });
    let cases: [(&str, Sheet<'_>); 6] = [
        ("nested-list", Sheet::tape(vec![Frame::List(nested)])),
        ("ul-then-ol", Sheet::tape(ul_ol)),
        ("tasks", Sheet::tape(vec![Frame::List(tasks)])),
        ("wrap", Sheet::tape(vec![wrap])),
        ("notes", notes),
        ("bold-italic", Sheet::tape(vec![both])),
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
