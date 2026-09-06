//! Column geometry. The old 578-on-576 slight-overflow assertion is replaced.

mod common;

use tm20::PRINTABLE_DOTS;
use tm20_set::{
    ColAlign, Cols, Cut, Frame, GRID, GridSkip, HANG, Measure, Rule, Sheet, Span, TextSize,
    Thickness, compose,
};

#[test]
fn long_breakable_cell_is_not_limited_by_unwrapped_u16_width() {
    let faces = common::table();
    let text = "word ".repeat(2_000);
    let natural = faces
        .text(Cut::Roman)
        .unwrap()
        .shape_run(&text, TextSize::Pt11)
        .unwrap();
    assert!(natural.advance().ceil_dots().unwrap() > i32::from(u16::MAX));
    let sheet = Sheet::tape(vec![Frame::Cols(common::cols(Cut::Roman, &text, "$1"))]);
    let page = compose(&sheet, &faces).expect("unwrapped width is not a wire dimension");
    assert_eq!(page.width(), PRINTABLE_DOTS);
    assert!(page.height() > 1_000);
}

#[test]
fn cols_has_ink_on_both_sides() {
    let faces = common::table();
    let r = common::compose_raster(
        &Sheet::tape(vec![Frame::Cols(common::cols(
            Cut::Roman,
            "Coffee",
            "$4.50",
        ))]),
        &faces,
    );
    let (x0, x1, _, _) = common::ink_bbox(&r);
    let mid = r.width() / 2;
    let mut left = false;
    let mut right = false;
    for y in 0..r.height() {
        left |= (0..mid).any(|x| common::packed_ink(&r, y, x));
        right |= (mid..r.width()).any(|x| common::packed_ink(&r, y, x));
    }
    assert!(left && right, "item and price must occupy both halves");
    assert!(
        x0 < GRID,
        "start column sits on the left (x0={x0}, GRID={GRID})"
    );
    assert!(
        u32::from(x1) + 1 + u32::from(GRID) >= u32::from(r.width()),
        "end column hangs on the right (x1={x1}, width={})",
        r.width()
    );
}

#[test]
fn cols_hangs_from_rule() {
    let faces = common::table();
    let r = common::compose_raster(
        &Sheet::tape(vec![
            Frame::Rule(Rule::tape(Thickness::Two)),
            Frame::Cols(common::cols(Cut::Roman, "H", "$1")),
        ]),
        &faces,
    );
    let gap = common::first_ink_after(&r, 2) - 2;
    assert!(
        gap <= u32::from(HANG) + 2,
        "gap {gap} should be hang ({HANG})"
    );
    assert!(gap >= 1, "type should not sit in the rule");
}

#[test]
fn consecutive_cols_are_tight() {
    let faces = common::table();
    let two = common::compose_raster(
        &Sheet::tape(vec![
            Frame::Cols(common::cols(Cut::Roman, "A", "$1")),
            Frame::Cols(common::cols(Cut::Roman, "B", "$2")),
        ]),
        &faces,
    );
    let stacked = common::compose_raster(
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
    );
    let delta = (two.height() as i32 - stacked.height() as i32).abs();
    assert!(
        delta <= 4,
        "two Cols frames {} should match two rows {} (delta {delta})",
        two.height(),
        stacked.height()
    );
}

#[test]
fn cols_wrap_the_start_column() {
    let faces = common::table();
    let short = common::compose_raster(
        &Sheet::tape(vec![Frame::Cols(common::cols(Cut::Roman, "A", "$1"))]),
        &faces,
    );
    let long = common::compose_raster(
        &Sheet::tape(vec![Frame::Cols(common::cols(
            Cut::Roman,
            "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
            "$1",
        ))]),
        &faces,
    );
    assert!(long.height() > short.height());
}

#[test]
fn three_columns_compose() {
    let faces = common::table();
    let r = common::compose_raster(
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
    );
    assert!(r.pixels().iter().any(|&b| b != 0));
}

#[test]
fn two_start_columns_keep_a_grid_gutter() {
    let faces = common::table();
    let r = common::compose_raster(&Sheet::tape(vec![common::two_start("x", "y")]), &faces);
    let merged = common::merge_runs(common::ink_runs(&r, 0, r.height()));
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
    let (_, right, ..) = common::ink_bbox(&r);
    assert!(
        u32::from(right) * 2 < u32::from(r.width()),
        "compact table sits left ({right} of {})",
        r.width()
    );
    assert!(common::leftmost_ink(&r) < GRID);
}

#[test]
fn start_columns_size_to_content() {
    let faces = common::table();
    let r = common::compose_raster(
        &Sheet::tape(vec![common::three_start(
            "0",
            "scalar",
            "discourse orientation now",
        )]),
        &faces,
    );
    let (_, _, y0, y1) = common::ink_bbox(&r);
    assert!(
        y1 - y0 + 1 < u32::from(common::l11()),
        "discourse orientation should stay one line (ink span {})",
        y1 - y0 + 1
    );
    let merged = common::merge_runs(common::ink_runs(&r, 0, r.height()));
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

#[test]
fn fga_grade_row_keeps_three_columns() {
    let faces = common::table();
    let r = common::compose_raster(
        &Sheet::tape(vec![common::three_start_rows(
            vec![
                ["Grade", "Object", "Meaning"],
                ["n", "pseudoscalar", "discourse orientation"],
            ],
            true,
        )]),
        &faces,
    );
    let bands = common::ink_bands(&r);
    assert!(
        (2..=3).contains(&bands.len()),
        "header plus a data row (meaning may wrap); got {} bands {bands:?}",
        bands.len()
    );
    let header = common::merge_runs_at(common::ink_runs(&r, bands[0].0, bands[0].1), GRID);
    let data = common::merge_runs_at(common::ink_runs(&r, bands[1].0, bands[1].1), GRID);
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

/// Replaces `compact_overfull_less_than_a_grid_keeps_pref`, which expected a
/// box ending at 578 on a 576-dot page. Slight deficit follows the same
/// fitting policy as any other deficit; ink stays inside the measure.
#[test]
fn slight_overfull_three_start_stays_inside_the_measure() {
    let faces = common::table();
    // Preferred widths 90/185/287 plus two GRID gutters are 578 — the old
    // exemption. Content here is sized to force that compact-overfull case.
    let r = common::compose_raster(
        &Sheet::tape(vec![common::three_start_rows(
            vec![[
                "Grade object meaning grade",
                "pseudoscalar discourse now",
                "orientation of the whole tape line",
            ]],
            false,
        )]),
        &faces,
    );
    let (_, x1, ..) = common::ink_bbox(&r);
    assert!(
        x1 < PRINTABLE_DOTS,
        "old 578-on-576 placement is refused; rightmost ink {x1} must stay in 576"
    );
    assert_eq!(r.width(), PRINTABLE_DOTS);
}

#[test]
fn impossible_gutters_are_a_geometry_error() {
    let faces = common::table();
    let width = Measure::new(10).expect("10-dot measure");
    let mut sheet = Sheet::tape(vec![common::three_start("A", "B", "C")]);
    sheet.width = width;
    let err = compose(&sheet, &faces).unwrap_err();
    assert!(
        matches!(err, tm20_set::Error::ImpossibleColumns),
        "mandatory gaps plus one dot per column cannot fit 10: {err}"
    );
}
