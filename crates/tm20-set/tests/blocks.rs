//! Block rhythm, markers, lists, quotes, heads, rules, and notes apparatus.

mod common;

use tm20::PRINTABLE_DOTS;
use tm20_set::{
    Code, Cut, DisplayCut, DisplaySize, Figure, Frame, GRID, HANG, Head, List, ListFit, ListItem,
    Mark, MarkAlign, Math, NOTE_RULE, Note, Quote, Rule, Sheet, TASK_BOX, TextBlock, TextSize,
    Thickness, Tracking, compose, pt_dots,
};

#[test]
fn larger_ancestor_marker_clears_nested_box_at_page_origin() {
    use tm20_set::{DecimalDelim, Marker};
    use tm20_set::{DrawOp, layout};
    let faces = common::table();
    for child in [
        Frame::Figure(Figure::from_bits(8, 8, &[true; 64]).unwrap()),
        Frame::Math(Math::from_bits(8, 8, &[true; 64], 4).unwrap()),
        Frame::Rule(Rule::tape(Thickness::Two)),
    ] {
        let list = |size, frames| {
            Frame::List(List {
                size,
                cut: Cut::Roman,
                marker: Marker::Decimal {
                    start: 1,
                    delim: DecimalDelim::Period,
                },
                fit: ListFit::Tight,
                items: vec![ListItem::new(frames)],
            })
        };
        let sheet = Sheet::tape(vec![
            list(TextSize::Pt11, vec![list(TextSize::Pt8, vec![child])]),
            common::text("after"),
        ]);
        let checked = sheet.admit().unwrap();
        let resolved = faces.resolve(&checked.face_requirements()).unwrap();
        let plan = layout(&checked, &resolved).unwrap();
        let mut markers = Vec::new();
        for op in plan.ops() {
            if let DrawOp::GlyphRun { run, baseline, .. } = op {
                let top = baseline.checked_add(run.ink().y0()).unwrap();
                assert!(top.units() >= 0, "marker top escaped page: {}", top.units());
                markers.push(*baseline);
            }
        }
        assert_eq!(markers.len(), 3);
        assert_eq!(
            markers[0], markers[1],
            "nested markers share the first anchor"
        );
        assert!(
            markers[2] > markers[1],
            "next sibling must not reuse marker anchor"
        );
    }
}

#[test]
fn head_sticks_to_the_following_text() {
    let faces = common::table();
    let stuck = common::compose_raster(
        &Sheet::tape(vec![
            Frame::Head(Head {
                size: TextSize::Pt11,
                text: "H".into(),
            }),
            common::text("H"),
        ]),
        &faces,
    );
    let paras = common::compose_raster(
        &Sheet::tape(vec![common::text("H"), common::text("H")]),
        &faces,
    );
    assert!(
        stuck.height() + 8 < paras.height(),
        "head+text {} should be tighter than two paragraphs {}",
        stuck.height(),
        paras.height()
    );
}

#[test]
fn mark_then_text_has_more_air_than_head_then_text() {
    let faces = common::table();
    let after_head = common::compose_raster(
        &Sheet::tape(vec![
            Frame::Head(Head {
                size: TextSize::Pt11,
                text: "H".into(),
            }),
            common::text("H"),
        ]),
        &faces,
    );
    let after_mark = common::compose_raster(
        &Sheet::tape(vec![
            Frame::Mark(Mark {
                cut: DisplayCut::Roman,
                size: DisplaySize::Pt18,
                text: "H".into(),
                align: MarkAlign::Start,
                tracking: Tracking(0),
            }),
            common::text("H"),
        ]),
        &faces,
    );
    assert!(
        after_mark.height() > after_head.height(),
        "display contrast {} should exceed head stick {}",
        after_mark.height(),
        after_head.height()
    );
}

#[test]
fn mark_center_is_not_start() {
    let faces = common::table();
    let start = common::compose_raster(
        &Sheet::tape(vec![Frame::Mark(Mark {
            cut: DisplayCut::Roman,
            size: DisplaySize::Pt18,
            text: "H".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        })]),
        &faces,
    );
    let center = common::compose_raster(
        &Sheet::tape(vec![Frame::Mark(Mark {
            cut: DisplayCut::Roman,
            size: DisplaySize::Pt18,
            text: "H".into(),
            align: MarkAlign::Center,
            tracking: Tracking(0),
        })]),
        &faces,
    );
    assert!(common::leftmost_ink(&center) > common::leftmost_ink(&start) + 50);
}

#[test]
fn mark_tracking_widens() {
    let faces = common::table();
    let tight = common::compose_raster(
        &Sheet::tape(vec![Frame::Mark(Mark {
            cut: DisplayCut::Roman,
            size: DisplaySize::Pt18,
            text: "MM".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        })]),
        &faces,
    );
    let tracked = common::compose_raster(
        &Sheet::tape(vec![Frame::Mark(Mark {
            cut: DisplayCut::Roman,
            size: DisplaySize::Pt18,
            text: "MM".into(),
            align: MarkAlign::Start,
            tracking: Tracking(200),
        })]),
        &faces,
    );
    let (_, t1, _, _) = common::ink_bbox(&tight);
    let (_, k1, _, _) = common::ink_bbox(&tracked);
    assert!(k1 > t1, "tracking 200 should widen MM ({k1} vs {t1})");
}

#[test]
fn mark_wider_than_the_measure_wraps() {
    let faces = common::table();
    let one = common::compose_raster(
        &Sheet::tape(vec![Frame::Mark(Mark {
            cut: DisplayCut::Roman,
            size: DisplaySize::Pt18,
            text: "H".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        })]),
        &faces,
    );
    let wrapped = common::compose_raster(
        &Sheet::tape(vec![Frame::Mark(Mark {
            cut: DisplayCut::Roman,
            size: DisplaySize::Pt18,
            text: "Functional geometric algebra".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        })]),
        &faces,
    );
    assert!(
        wrapped.height() > one.height(),
        "a mark longer than the tape should wrap ({} vs {})",
        wrapped.height(),
        one.height()
    );
    let (_, right, _, _) = common::ink_bbox(&wrapped);
    assert!(
        right < PRINTABLE_DOTS,
        "wrapped mark should not clip at the tape edge (rightmost {right})"
    );
}

#[test]
fn rule_sits_below_the_line_slug() {
    let faces = common::table();
    let r = common::compose_raster(
        &Sheet::tape(vec![
            common::text("H"),
            Frame::Rule(Rule::tape(Thickness::One)),
        ]),
        &faces,
    );
    let mut last_type = 0u32;
    let mut rule_y = None;
    for y in 0..r.height() {
        if common::full_width_row(&r, y) {
            rule_y = Some(y);
            break;
        }
        if common::row_has_ink(&r, y) {
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
        let r = common::compose_raster(&Sheet::tape(vec![Frame::Rule(Rule::tape(t))]), &faces);
        (0..r.height())
            .filter(|&y| common::full_width_row(&r, y))
            .count()
    };
    assert_eq!(count(Thickness::One), 1);
    assert_eq!(count(Thickness::Two), 2);
}

#[test]
fn text_after_a_rule_takes_grid_air() {
    let faces = common::table();
    let r = common::compose_raster(
        &Sheet::tape(vec![
            Frame::Rule(Rule::tape(Thickness::Two)),
            common::text("H"),
        ]),
        &faces,
    );
    let gap = common::first_ink_after(&r, 2) - 2;
    assert!(
        gap >= u32::from(GRID) - 1 && gap < u32::from(common::l11()),
        "gap {gap}: one module of air, not the hang ({HANG}) and not a full slug"
    );
}

#[test]
fn rule_takes_a_module_of_air_both_sides() {
    let faces = common::table();
    let r = common::compose_raster(
        &Sheet::tape(vec![
            common::text("H"),
            Frame::Rule(Rule::tape(Thickness::Two)),
            common::text("H"),
        ]),
        &faces,
    );
    let rule_top = (0..r.height())
        .find(|&y| common::full_width_row(&r, y))
        .expect("rule");
    let ink_above = (0..rule_top)
        .rev()
        .find(|&y| common::row_has_ink(&r, y))
        .expect("ink above");
    let below = common::first_ink_after(&r, rule_top + 2);
    assert!(
        rule_top - ink_above >= u32::from(GRID),
        "air above the rule: {}",
        rule_top - ink_above
    );
    let after = below - (rule_top + 2);
    assert!(
        (u32::from(GRID) - 1..u32::from(GRID) + 3).contains(&after),
        "air below the rule is one module: {after}"
    );
}

#[test]
fn stacked_heads_take_a_seam() {
    let faces = common::table();
    let head = |t: &'static str| {
        Frame::Head(Head {
            size: TextSize::Pt11,
            text: t.into(),
        })
    };
    let stacked = common::compose_raster(&Sheet::tape(vec![head("One"), head("Two")]), &faces);
    let flowing =
        common::compose_raster(&Sheet::tape(vec![head("One"), common::text("Two")]), &faces);
    let delta = stacked.height() as i32 - flowing.height() as i32;
    assert!(
        (delta - i32::from(GRID)).abs() <= 1,
        "stacked heads breathe one module over head-then-prose: {delta}"
    );
}

#[test]
fn list_runover_clears_the_mark_column() {
    let faces = common::table();
    let list = common::dash_list(vec![common::item(
        "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
    )]);
    let hang = list.hang_dots(faces.text(Cut::Roman).unwrap()).unwrap();
    assert_eq!(hang % GRID, 0);
    assert!(hang >= GRID);
    let r = common::compose_raster(&Sheet::tape(vec![Frame::List(list)]), &faces);
    let mut dash_last = None;
    for y in 0..r.height() {
        let mut mark = false;
        for x in 0..hang {
            mark |= common::packed_ink(&r, y, x);
        }
        if mark {
            dash_last = Some(y);
        }
    }
    let dash_last = dash_last.expect("en-dash in the mark column");
    let mut runover = false;
    for y in dash_last + 1..r.height() {
        for x in 0..hang {
            assert!(
                !common::packed_ink(&r, y, x),
                "runover ink in mark column at ({x},{y})"
            );
        }
        for x in hang..r.width() {
            runover |= common::packed_ink(&r, y, x);
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
    let dash_h = dash.hang_dots(faces.text(Cut::Roman).unwrap()).unwrap();
    let dec_h = decimal.hang_dots(faces.text(Cut::Roman).unwrap()).unwrap();
    assert_eq!(dec_h % GRID, 0);
    assert!(dec_h >= dash_h);
    assert_eq!(
        dash_h,
        common::decimal_list(1, vec![common::item("H")])
            .hang_dots(faces.text(Cut::Roman).unwrap())
            .unwrap(),
        "dash and one-digit decimal share a closed hang"
    );
    let r = common::compose_raster(&Sheet::tape(vec![Frame::List(decimal)]), &faces);
    let mut mark = false;
    for y in 0..r.height() {
        for x in 0..dec_h {
            mark |= common::packed_ink(&r, y, x);
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
    let hang = one.hang_dots(faces.text(Cut::Roman).unwrap()).unwrap();
    let rightmost = |list: List<'_>| {
        let r = common::compose_raster(&Sheet::tape(vec![Frame::List(list)]), &faces);
        let mut x1 = 0u16;
        for y in 0..r.height() {
            for x in 0..hang {
                if common::packed_ink(&r, y, x) {
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
        (i32::from(d10) - i32::from(d1)).abs() <= 4,
        "1. and 10. should share a right edge ({d1} vs {d10})"
    );
}

#[test]
fn loose_list_is_taller_than_tight() {
    let faces = common::table();
    let tight = common::dash_list(vec![common::item("H"), common::item("H")]);
    let mut loose = common::dash_list(vec![common::item("H"), common::item("H")]);
    loose.fit = ListFit::Loose;
    let a = common::compose_raster(&Sheet::tape(vec![Frame::List(tight)]), &faces);
    let b = common::compose_raster(&Sheet::tape(vec![Frame::List(loose)]), &faces);
    assert!(
        b.height() > a.height(),
        "loose {} should exceed tight {}",
        b.height(),
        a.height()
    );
}

#[test]
fn two_texts_in_an_item_are_paragraphs() {
    let faces = common::table();
    let items = common::dash_list(vec![common::item("H"), common::item("H")]);
    let mut paras = common::dash_list(vec![ListItem::new(vec![
        common::text("H"),
        common::text("H"),
    ])]);
    paras.fit = ListFit::Tight;
    let a = common::compose_raster(&Sheet::tape(vec![Frame::List(items)]), &faces);
    let b = common::compose_raster(&Sheet::tape(vec![Frame::List(paras)]), &faces);
    let extra = b.height() as i32 - a.height() as i32;
    let l = i32::from(common::l11());
    assert!(
        extra >= l - 4 && extra <= l + 8,
        "two texts in one item {} vs two tight items {} extra {extra} should be one leading ({l})",
        b.height(),
        a.height()
    );
}

#[test]
fn nested_tight_list_is_not_a_blank_taller() {
    let faces = common::table();
    let siblings = common::dash_list(vec![common::item("H"), common::item("H")]);
    let nested = common::dash_list(vec![ListItem::new(vec![
        common::text("H"),
        Frame::List(common::dash_list(vec![common::item("H")])),
    ])]);
    let a = common::compose_raster(&Sheet::tape(vec![Frame::List(siblings)]), &faces);
    let b = common::compose_raster(&Sheet::tape(vec![Frame::List(nested)]), &faces);
    let delta = (b.height() as i32 - a.height() as i32).abs();
    assert!(
        delta <= 8,
        "nested {} vs two tight siblings {} should share a slug, not a blank",
        b.height(),
        a.height()
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
    let mixed = common::compose_raster(&Sheet::tape(vec![dash, decimal]), &faces);
    let tight = common::compose_raster(&Sheet::tape(vec![Frame::List(four)]), &faces);
    let extra = mixed.height() as i32 - tight.height() as i32;
    let l = i32::from(common::l11());
    assert!(
        extra < l - 4,
        "ul then ol extra {extra} should not be a paragraph blank ({l}); mixed {} tight {}",
        mixed.height(),
        tight.height()
    );
}

#[test]
fn loose_item_two_paras_still_taller() {
    let faces = common::table();
    let mut tight = common::dash_list(vec![
        ListItem::new(vec![common::text("H"), common::text("H")]),
        ListItem::new(vec![common::text("H"), common::text("H")]),
    ]);
    tight.fit = ListFit::Tight;
    let mut loose = common::dash_list(vec![
        ListItem::new(vec![common::text("H"), common::text("H")]),
        ListItem::new(vec![common::text("H"), common::text("H")]),
    ]);
    loose.fit = ListFit::Loose;
    let a = common::compose_raster(&Sheet::tape(vec![Frame::List(tight)]), &faces);
    let b = common::compose_raster(&Sheet::tape(vec![Frame::List(loose)]), &faces);
    assert!(
        b.height() > a.height(),
        "loose two-para items {} should exceed tight {}",
        b.height(),
        a.height()
    );
}

#[test]
fn task_box_hangs_on_the_grid() {
    let faces = common::table();
    let list = common::dash_list(vec![ListItem::task(false, common::plain("H"))]);
    let hang = list.hang_dots(faces.text(Cut::Roman).unwrap()).unwrap();
    assert_eq!(hang % GRID, 0);
    assert!(hang >= TASK_BOX);
}

#[test]
fn task_box_sits_in_the_cap_band() {
    let faces = common::table();
    let r = common::compose_raster(
        &Sheet::tape(vec![Frame::List(common::dash_list(vec![ListItem::task(
            false,
            common::plain("H"),
        )]))]),
        &faces,
    );
    let mut y0 = r.height();
    let mut y1 = 0u32;
    for y in 0..r.height() {
        for x in 0..TASK_BOX {
            if common::packed_ink(&r, y, x) {
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    assert!(y1 >= y0, "task box has ink");
    let center = u32::midpoint(y0, y1);
    let (_, _, _, text_y1) = common::ink_bbox(&r);
    assert!(
        center < text_y1.saturating_sub(u32::from(TASK_BOX) / 4),
        "box center {center} should sit in the cap band, not on the baseline (text bottom {text_y1})"
    );
}

#[test]
fn checked_task_has_more_ink_than_open() {
    let faces = common::table();
    let open = common::compose_raster(
        &Sheet::tape(vec![Frame::List(common::dash_list(vec![ListItem::task(
            false,
            common::plain("H"),
        )]))]),
        &faces,
    );
    let done = common::compose_raster(
        &Sheet::tape(vec![Frame::List(common::dash_list(vec![ListItem::task(
            true,
            common::plain("H"),
        )]))]),
        &faces,
    );
    let count = |r: &tm20::Raster| {
        let mut n = 0;
        for y in 0..r.height() {
            for x in 0..TASK_BOX {
                if common::packed_ink(r, y, x) {
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
    compose(&Sheet::tape(vec![common::nest_lists(3)]), &faces).unwrap();
    let err = compose(&Sheet::tape(vec![common::nest_lists(4)]), &faces).unwrap_err();
    assert!(matches!(err, tm20_set::Error::Nesting));
}

#[test]
fn nested_quote_is_not_a_blank_taller() {
    let faces = common::table();
    let nested = Frame::Quote(Quote {
        frames: vec![
            common::text("H"),
            Frame::Quote(Quote {
                frames: vec![common::text("H")],
            }),
        ],
    });
    let paras = common::compose_raster(
        &Sheet::tape(vec![common::text("H"), common::text("H")]),
        &faces,
    );
    let quoted = common::compose_raster(&Sheet::tape(vec![nested]), &faces);
    assert!(
        quoted.height() + 8 < paras.height(),
        "nested quote {} should share a slug, not a paragraph blank ({})",
        quoted.height(),
        paras.height()
    );
}

#[test]
fn sibling_quotes_share_a_slug() {
    let faces = common::table();
    let quotes = common::compose_raster(
        &Sheet::tape(vec![
            Frame::Quote(Quote {
                frames: vec![common::text("H")],
            }),
            Frame::Quote(Quote {
                frames: vec![common::text("H")],
            }),
        ]),
        &faces,
    );
    let paras = common::compose_raster(
        &Sheet::tape(vec![common::text("H"), common::text("H")]),
        &faces,
    );
    assert!(
        quotes.height() + 8 < paras.height(),
        "sibling quotes {} should share a slug, not a paragraph blank ({})",
        quotes.height(),
        paras.height()
    );
}

#[test]
fn quote_then_code_share_a_slug() {
    let faces = common::table();
    let mixed = common::compose_raster(
        &Sheet::tape(vec![
            Frame::Quote(Quote {
                frames: vec![common::text("H")],
            }),
            Frame::Code(Code::new(TextSize::Pt11, "H")),
        ]),
        &faces,
    );
    let paras = common::compose_raster(
        &Sheet::tape(vec![common::text("H"), common::text("H")]),
        &faces,
    );
    assert!(
        mixed.height() + 8 < paras.height(),
        "quote then code {} should share a slug, not a paragraph blank ({})",
        mixed.height(),
        paras.height()
    );
}

#[test]
fn rule_in_a_quote_is_the_tape() {
    let faces = common::table();
    let r = common::compose_raster(
        &Sheet::tape(vec![Frame::Quote(Quote {
            frames: vec![Frame::Rule(Rule::tape(Thickness::Two))],
        })]),
        &faces,
    );
    let rows = (0..r.height())
        .filter(|&y| common::full_width_row(&r, y))
        .count();
    assert_eq!(rows, 2, "quote rule is tape-wide, not leftover hang");
}

#[test]
fn sibling_lists_take_a_grid_seam() {
    let faces = common::table();
    let one = common::compose_raster(
        &Sheet::tape(vec![Frame::List(common::dash_list(vec![
            common::item("H"),
            common::item("H"),
            common::item("H"),
        ]))]),
        &faces,
    );
    let three = common::compose_raster(
        &Sheet::tape(vec![
            Frame::List(common::dash_list(vec![common::item("H")])),
            Frame::List(common::dash_list(vec![common::item("H")])),
            Frame::List(common::dash_list(vec![common::item("H")])),
        ]),
        &faces,
    );
    let extra = three.height() as i32 - one.height() as i32;
    assert_eq!(
        extra,
        2 * i32::from(GRID),
        "two new-list seams are 2×GRID ({} vs {})",
        three.height(),
        one.height()
    );
}

#[test]
fn empty_item_occupies_a_body_slug() {
    let faces = common::table();
    let full = common::compose_raster(
        &Sheet::tape(vec![Frame::List(common::dash_list(vec![common::item(
            "H",
        )]))]),
        &faces,
    );
    let empty = common::compose_raster(
        &Sheet::tape(vec![Frame::List(common::dash_list(vec![ListItem::new(
            vec![],
        )]))]),
        &faces,
    );
    let delta = (empty.height() as i32 - full.height() as i32).abs();
    assert!(
        delta <= 4,
        "blank item {} should share a body slug with a one-word item {}",
        empty.height(),
        full.height()
    );
    let mut mark = false;
    for y in 0..empty.height() {
        for x in 0..24 {
            mark |= common::packed_ink(&empty, y, x);
        }
    }
    assert!(mark, "empty item still has a mark on the slug");
}

#[test]
fn quote_first_voice_hangs_two_modules() {
    let faces = common::table();
    let plain = common::compose_raster(&Sheet::tape(vec![common::text("H")]), &faces);
    let quoted = common::compose_raster(
        &Sheet::tape(vec![Frame::Quote(Quote {
            frames: common::plain("H"),
        })]),
        &faces,
    );
    let nested = common::compose_raster(
        &Sheet::tape(vec![Frame::Quote(Quote {
            frames: vec![Frame::Quote(Quote {
                frames: common::plain("H"),
            })],
        })]),
        &faces,
    );
    let shift = i32::from(common::leftmost_ink(&quoted)) - i32::from(common::leftmost_ink(&plain));
    let step = i32::from(common::leftmost_ink(&nested)) - i32::from(common::leftmost_ink(&quoted));
    assert_eq!(shift, 2 * i32::from(GRID), "first voice is two modules");
    assert_eq!(step, i32::from(GRID), "each nest adds one");
}

#[test]
fn code_hangs_by_the_grid() {
    let faces = common::table();
    let plain = common::compose_raster(&Sheet::tape(vec![common::text("H")]), &faces);
    let code = common::compose_raster(
        &Sheet::tape(vec![Frame::Code(Code::new(TextSize::Pt11, "H"))]),
        &faces,
    );
    let shift = i32::from(common::leftmost_ink(&code)) - i32::from(common::leftmost_ink(&plain));
    assert!(
        (shift - i32::from(GRID)).abs() <= 2,
        "code hang {shift} should be GRID plus Menlo sidebearing"
    );
}

#[test]
fn overwide_code_rejects_instead_of_wrapping_or_clipping() {
    let faces = common::table();
    let one = common::compose_raster(
        &Sheet::tape(vec![Frame::Code(Code::new(TextSize::Pt11, "Hello"))]),
        &faces,
    );
    let many = compose(
        &Sheet::tape(vec![Frame::Code(Code::new(
            TextSize::Pt11,
            "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
        ))]),
        &faces,
    );
    assert!(one.height() > 0);
    assert!(matches!(many, Err(tm20_set::Error::InText { .. })));
}

#[test]
fn ordered_list_numbers_cannot_saturate_into_duplicates() {
    let list = common::decimal_list(u32::MAX, vec![common::item("one"), common::item("two")]);
    assert!(matches!(
        compose(&Sheet::tape(vec![Frame::List(list)]), &common::table()),
        Err(tm20_set::Error::CoordinateOverflow)
    ));
}

#[test]
fn three_quotes_compose_a_fourth_does_not() {
    let faces = common::table();
    compose(&Sheet::tape(vec![common::nest_quotes(3)]), &faces).unwrap();
    let err = compose(&Sheet::tape(vec![common::nest_quotes(4)]), &faces).unwrap_err();
    assert!(matches!(err, tm20_set::Error::Nesting));
}

#[test]
fn notes_follow_the_frames() {
    let faces = common::table();
    let mut sheet = Sheet::tape(vec![]);
    let id = sheet.add_note(Note::dest("https://example.com")).unwrap();
    sheet.frames = vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![
            tm20_set::Span::new(Cut::Italic, "Canon"),
            tm20_set::Span::note(id),
        ],
    })];
    let r = common::compose_raster(&sheet, &faces);
    let mut rule = false;
    for y in 0..r.height() {
        let mut ink = 0u16;
        let mut last = 0u16;
        for x in 0..r.width() {
            if common::packed_ink(&r, y, x) {
                ink += 1;
                last = x;
            }
        }
        if ink >= NOTE_RULE.saturating_sub(8) && last < r.width() / 2 {
            rule = true;
        }
    }
    assert!(rule, "notes sit after a short rule, not a full-tape rule");
}

#[test]
fn notes_rule_has_two_points_of_air() {
    let faces = common::table();
    let mut sheet = Sheet::tape(vec![]);
    let id = sheet.add_note(Note::dest("https://example.com")).unwrap();
    sheet.frames = vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![
            tm20_set::Span::new(Cut::Roman, "H"),
            tm20_set::Span::note(id),
        ],
    })];
    let r = common::compose_raster(&sheet, &faces);
    let mut rule_y = None;
    for y in 0..r.height() {
        let mut ink = 0u16;
        let mut last = 0u16;
        for x in 0..r.width() {
            if common::packed_ink(&r, y, x) {
                ink += 1;
                last = x;
            }
        }
        if ink >= NOTE_RULE.saturating_sub(8) && last < r.width() / 2 {
            rule_y = Some(y);
            break;
        }
    }
    let rule_y = rule_y.expect("short notes rule");
    let mut body_last = 0u32;
    for y in 0..rule_y {
        if common::row_has_ink(&r, y) {
            body_last = y;
        }
    }
    let above = rule_y.saturating_sub(body_last + 1);
    let air = pt_dots(2.0) as u32;
    let blank = u32::from(common::l11());
    assert!(
        above >= air && above < blank,
        "gap above notes rule {above} should include 2pt ({air}) and not a paragraph blank ({blank})"
    );
    let notes = common::first_ink_after(&r, rule_y + 1);
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
        let mut sheet = Sheet::tape(vec![]);
        let id = sheet.add_note(Note::dest("https://example.com")).unwrap();
        sheet.frames = vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![
                tm20_set::Span::new(Cut::Italic, "Canon"),
                tm20_set::Span::note(id),
            ],
        })];
        common::compose_raster(&sheet, &faces)
    };
    let g2 = {
        let mut sheet = Sheet::tape(vec![]);
        let id = sheet
            .add_note(Note::Dest {
                location: None,
                dest: "https://example.com".into(),
                title: Some("The Canon".into()),
            })
            .unwrap();
        sheet.frames = vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![
                tm20_set::Span::new(Cut::Italic, "Canon"),
                tm20_set::Span::note(id),
            ],
        })];
        common::compose_raster(&sheet, &faces)
    };
    assert!(
        g2.height() > g1.height(),
        "title then dest is taller than dest alone ({} vs {})",
        g2.height(),
        g1.height()
    );
}

#[test]
fn figure_note_has_raised_ink() {
    let faces = common::table();
    let bits = vec![true; 8 * 8];
    let mut sheet = Sheet::tape(vec![]);
    let id = sheet.add_note(Note::dest("grid")).unwrap();
    let fig = Figure::from_bits(8, 8, &bits).unwrap().noted(id.get());
    sheet.frames = vec![Frame::Figure(fig)];
    let r = common::compose_raster(&sheet, &faces);
    let left = common::leftmost_ink(&r);
    let after_bitmap = left + 8;
    let mut after = false;
    let max_y = 16.min(r.height());
    for y in 0..max_y {
        for x in after_bitmap..r.width() {
            if common::packed_ink(&r, y, x) {
                after = true;
            }
        }
    }
    assert!(after, "note sits after the bitmap");
}

#[test]
fn note_dest_wraps_instead_of_clipping() {
    let faces = common::table();
    let mut sheet = Sheet::tape(vec![]);
    let id = sheet
        .add_note(Note::dest(
            "https://example.com/one/two/three/four/five/six/seven/eight/nine/ten/eleven/x",
        ))
        .unwrap();
    sheet.frames = vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![
            tm20_set::Span::new(Cut::Roman, "See"),
            tm20_set::Span::note(id),
        ],
    })];
    let r = common::compose_raster(&sheet, &faces);
    let (_, x1, _, _) = common::ink_bbox(&r);
    assert!(x1 < PRINTABLE_DOTS - 1, "note URL clipped at {x1}");
}

#[test]
fn empty_quote_in_a_list_item_keeps_its_mark() {
    let faces = common::table();
    let r = common::compose_raster(
        &Sheet::tape(vec![Frame::List(common::dash_list(vec![ListItem::new(
            vec![Frame::Quote(Quote { frames: vec![] })],
        )]))]),
        &faces,
    );
    assert!(
        r.height() > 1,
        "empty quote item is not a one-row blank page"
    );
    let mut mark = false;
    for y in 0..r.height() {
        for x in 0..24 {
            mark |= common::packed_ink(&r, y, x);
        }
    }
    assert!(mark, "owning list mark occurs once for `- >`");
}

#[test]
fn empty_note_quote_keeps_its_label() {
    let faces = common::table();
    let mut sheet = Sheet::tape(vec![]);
    let id = sheet
        .add_note(Note::Blocks(vec![Frame::Quote(Quote { frames: vec![] })]))
        .unwrap();
    sheet.frames = vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![
            tm20_set::Span::new(Cut::Roman, "A"),
            tm20_set::Span::note(id),
        ],
    })];
    let r = common::compose_raster(&sheet, &faces);
    assert!(common::ink_count(&r) > 0);
}

#[test]
fn empty_item_mark_does_not_leak_to_the_next_sibling() {
    let faces = common::table();
    let r = common::compose_raster(
        &Sheet::tape(vec![Frame::List(common::dash_list(vec![
            ListItem::new(vec![Frame::Quote(Quote { frames: vec![] })]),
            common::item("H"),
        ]))]),
        &faces,
    );
    let bands = common::ink_bands(&r);
    assert!(
        bands.len() >= 2,
        "empty item and following sibling are separate ({bands:?})"
    );
}

#[test]
fn three_deep_lists_inside_a_note_compose() {
    let faces = common::table();
    let mut sheet = Sheet::tape(vec![]);
    let id = sheet
        .add_note(Note::Blocks(vec![common::nest_lists(3)]))
        .unwrap();
    sheet.frames = vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![
            tm20_set::Span::new(Cut::Roman, "A"),
            tm20_set::Span::note(id),
        ],
    })];
    compose(&sheet, &faces).unwrap();
}

#[test]
fn narrow_display_math_is_centered() {
    let faces = common::table();
    let bits = vec![true; 8 * 8];
    let math = Math::from_bits(8, 8, &bits, 6).unwrap();
    let r = common::compose_raster(&Sheet::tape(vec![Frame::Math(math)]), &faces);
    let (x0, x1, _, _) = common::ink_bbox(&r);
    let mid = u16::midpoint(x0, x1);
    assert!(
        (i32::from(mid) - i32::from(PRINTABLE_DOTS) / 2).abs() <= 8,
        "narrow display [{x0},{x1}] should center on the tape"
    );
}

fn hang_has_ink(r: &tm20::Raster, hang: u16) -> bool {
    for y in 0..r.height() {
        for x in 0..hang.min(r.width()) {
            if common::packed_ink(r, y, x) {
                return true;
            }
        }
    }
    false
}

fn first_ink_row_in(r: &tm20::Raster, x0: u16, x1: u16) -> Option<u32> {
    for y in 0..r.height() {
        for x in x0..x1.min(r.width()) {
            if common::packed_ink(r, y, x) {
                return Some(y);
            }
        }
    }
    None
}

#[test]
fn empty_text_item_still_owns_a_visible_marker() {
    let faces = common::table();
    let list = common::dash_list(vec![ListItem::new(vec![Frame::Text(TextBlock::plain(
        Cut::Roman,
        TextSize::Pt11,
        "",
    ))])]);
    let hang = list.hang_dots(faces.text(Cut::Roman).unwrap()).unwrap();
    let r = common::compose_raster(&Sheet::tape(vec![Frame::List(list)]), &faces);
    assert!(
        hang_has_ink(&r, hang),
        "empty text is a slug, not a skipped mark"
    );
}

#[test]
fn empty_code_item_still_owns_a_visible_marker() {
    let faces = common::table();
    let list = common::dash_list(vec![ListItem::new(vec![Frame::Code(Code::new(
        TextSize::Pt11,
        "",
    ))])]);
    let hang = list.hang_dots(faces.text(Cut::Roman).unwrap()).unwrap();
    let r = common::compose_raster(&Sheet::tape(vec![Frame::List(list)]), &faces);
    assert!(
        hang_has_ink(&r, hang),
        "empty code is a slug, not a skipped mark"
    );
}

#[test]
fn code_leading_blank_line_keeps_the_marker_on_the_first_slug() {
    let faces = common::table();
    let list = common::dash_list(vec![ListItem::new(vec![Frame::Code(Code::new(
        TextSize::Pt11,
        "\nX",
    ))])]);
    let hang = list.hang_dots(faces.text(Cut::Roman).unwrap()).unwrap();
    let r = common::compose_raster(&Sheet::tape(vec![Frame::List(list)]), &faces);
    let mark_y = first_ink_row_in(&r, 0, hang).expect("marker");
    let body_y = first_ink_row_in(&r, hang, r.width()).expect("code X");
    assert!(
        mark_y < body_y,
        "marker sits on the blank first slug, body X is the second line ({mark_y} vs {body_y})"
    );
}

#[test]
fn decimal_marker_aligns_with_lowercase_first_line() {
    let faces = common::table();
    let list = common::decimal_list(1, vec![common::item("gyp")]);
    let hang = list.hang_dots(faces.text(Cut::Roman).unwrap()).unwrap();
    let r = common::compose_raster(&Sheet::tape(vec![Frame::List(list)]), &faces);
    let mark_y = first_ink_row_in(&r, 0, hang).expect("decimal mark");
    let body_y = first_ink_row_in(&r, hang, r.width()).expect("lowercase body");
    assert!(
        mark_y <= body_y,
        "marker may rise above lowercase but must share the first line ({mark_y} vs {body_y})"
    );
    assert!(hang_has_ink(&r, hang));
}

#[test]
fn task_empty_text_owns_the_box() {
    let faces = common::table();
    let list = common::dash_list(vec![ListItem::task(
        false,
        vec![Frame::Text(TextBlock::plain(
            Cut::Roman,
            TextSize::Pt11,
            "",
        ))],
    )]);
    let r = common::compose_raster(&Sheet::tape(vec![Frame::List(list)]), &faces);
    assert!(
        hang_has_ink(&r, TASK_BOX),
        "task box occupies the mark column"
    );
}

#[test]
fn note_apparatus_blank_item_owns_its_number() {
    let faces = common::table();
    let mut sheet = Sheet::tape(vec![]);
    let id = sheet
        .add_note(Note::Blocks(vec![Frame::Text(TextBlock::plain(
            Cut::Roman,
            TextSize::Pt8,
            "",
        ))]))
        .unwrap();
    sheet.frames = vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![
            tm20_set::Span::new(Cut::Roman, "A"),
            tm20_set::Span::note(id),
        ],
    })];
    let r = common::compose_raster(&sheet, &faces);
    let rule = (0..r.height())
        .find(|&y| {
            (0..NOTE_RULE.min(r.width()))
                .filter(|&x| common::packed_ink(&r, y, x))
                .count()
                >= 4
        })
        .expect("note rule");
    let after = (rule + 1..r.height())
        .find(|&y| (0..32.min(r.width())).any(|x| common::packed_ink(&r, y, x)));
    assert!(
        after.is_some(),
        "apparatus marker is below the rule, not clipped"
    );
}
