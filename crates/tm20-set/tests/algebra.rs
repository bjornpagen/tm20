//! Compose proofs. Faces come from whatever table the harness loaded.

mod common;

use tm20::graphics::{width_bytes, Graphics};
use tm20::PRINTABLE_DOTS;
use tm20_set::{
    compose, Cut, DisplaySize, Frame, Head, List, Mark, MarkAlign, Measure, Rule, Sheet, TextBlock,
    TextSize, Thickness, Tracking, GRID, HANG,
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

#[test]
fn compose_is_tape_wide() {
    let faces = common::table();
    let sheet = Sheet::tape(vec![Frame::Text(TextBlock::plain(
        Cut::Roman,
        TextSize::Pt11,
        "Hello",
    ))]);
    let g = compose(&sheet, &faces).unwrap();
    assert_eq!(g.width_dots, PRINTABLE_DOTS);
    assert!(g.pixels.iter().any(|&b| b != 0));
}

#[test]
fn pair_has_ink_on_both_sides() {
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![Frame::Pair(common::pair(
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
    assert_eq!(g.width_dots, Measure::TAPE.get());
}

#[test]
fn wrap_makes_taller_than_one_line() {
    let faces = common::table();
    let one = compose(
        &Sheet::tape(vec![Frame::Text(TextBlock::plain(
            Cut::Roman,
            TextSize::Pt11,
            "Hello",
        ))]),
        &faces,
    )
    .unwrap();
    let wrapped = compose(
        &Sheet::tape(vec![Frame::Text(TextBlock::plain(
            Cut::Roman,
            TextSize::Pt11,
            "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
        ))]),
        &faces,
    )
    .unwrap();
    assert!(wrapped.height_dots > one.height_dots);
}

#[test]
fn two_paragraphs_are_a_blank_line_apart() {
    let faces = common::table();
    let lines = compose(
        &Sheet::tape(vec![Frame::Text(TextBlock::plain(
            Cut::Roman,
            TextSize::Pt11,
            "H\nH",
        ))]),
        &faces,
    )
    .unwrap();
    let paras = compose(
        &Sheet::tape(vec![
            Frame::Text(TextBlock::plain(Cut::Roman, TextSize::Pt11, "H")),
            Frame::Text(TextBlock::plain(Cut::Roman, TextSize::Pt11, "H")),
        ]),
        &faces,
    )
    .unwrap();
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
                text: "H",
            }),
            Frame::Text(TextBlock::plain(Cut::Roman, TextSize::Pt11, "H")),
        ]),
        &faces,
    )
    .unwrap();
    let paras = compose(
        &Sheet::tape(vec![
            Frame::Text(TextBlock::plain(Cut::Roman, TextSize::Pt11, "H")),
            Frame::Text(TextBlock::plain(Cut::Roman, TextSize::Pt11, "H")),
        ]),
        &faces,
    )
    .unwrap();
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
                text: "H",
            }),
            Frame::Text(TextBlock::plain(Cut::Roman, TextSize::Pt11, "H")),
        ]),
        &faces,
    )
    .unwrap();
    let after_mark = compose(
        &Sheet::tape(vec![
            Frame::Mark(Mark {
                cut: Cut::Roman,
                size: DisplaySize::Pt18,
                text: "H",
                align: MarkAlign::Start,
                tracking: Tracking(0),
            }),
            Frame::Text(TextBlock::plain(Cut::Roman, TextSize::Pt11, "H")),
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
fn rule_sits_below_the_line_slug() {
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![
            Frame::Text(TextBlock::plain(Cut::Roman, TextSize::Pt11, "H")),
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
fn pair_hangs_from_rule() {
    let faces = common::table();
    let g = compose(
        &Sheet::tape(vec![
            Frame::Rule(Rule {
                thickness: Thickness::Two,
            }),
            Frame::Pair(common::pair(Cut::Roman, "H", "$1")),
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
            Frame::Text(TextBlock::plain(Cut::Roman, TextSize::Pt11, "H")),
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
fn list_runover_clears_the_mark_column() {
    let faces = common::table();
    let list = List {
        size: TextSize::Pt11,
        cut: Cut::Roman,
        items: vec![vec![tm20_set::Span {
            cut: Cut::Roman,
            text: "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
        }]],
    };
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
fn missing_cut_is_an_error() {
    let faces = tm20_set::FaceTable::new();
    let err = compose(
        &Sheet::tape(vec![Frame::Head(Head {
            size: TextSize::Pt11,
            text: "Hello",
        })]),
        &faces,
    )
    .unwrap_err();
    assert!(matches!(err, tm20_set::Error::MissingText(Cut::Bold)));
}
