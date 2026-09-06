//! Shaping, wrapping, and positioned-ink laws.

mod common;

use tm20_set::{
    Cut, Frame, GRID, Math, Measure, NOTE_RULE, Note, Sheet, Span, TextBlock, TextSize, WrapCost,
    compose,
};

#[test]
fn strikethrough_follows_wrapping_without_changing_glyph_positions() {
    use tm20_set::{DrawOp, layout, paint};
    let faces = common::table();
    for (size, content) in [
        (TextSize::Pt11, "old words old words\nnew line"),
        (TextSize::Pt8, "https://example.com/long/path/to/old/page"),
    ] {
        let make = |strike| {
            let mut sheet = Sheet::tape(vec![Frame::Text(TextBlock {
                size,
                spans: if strike {
                    vec![Span::Strike(vec![Span::new(Cut::Roman, content)])]
                } else {
                    vec![Span::new(Cut::Roman, content)]
                },
            })]);
            sheet.width = Measure::new(120).unwrap();
            sheet
        };
        let plain_sheet = make(false);
        let struck_sheet = make(true);
        let plain_checked = plain_sheet.admit().unwrap();
        let struck_checked = struck_sheet.admit().unwrap();
        let resolved = faces.resolve(&plain_checked.face_requirements()).unwrap();
        let plain = layout(&plain_checked, &resolved).unwrap();
        let struck = layout(&struck_checked, &resolved).unwrap();
        let positions = |plan: &tm20_set::LayoutPlan| {
            plan.ops()
                .iter()
                .filter_map(|op| match op {
                    DrawOp::GlyphRun {
                        x, baseline, run, ..
                    } => Some((x.units(), baseline.units(), run.advance().units())),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(positions(&plain), positions(&struck));
        assert_eq!(plain.height(), struck.height());
        let raster = paint(&struck).unwrap();
        let mut rules = 0;
        for op in struck.ops() {
            if let DrawOp::Rule { rows, clip } = op {
                rules += 1;
                assert!(clip.end() <= 120);
                for y in rows.start().get()..rows.end().get() {
                    for x in clip.start()..clip.end() {
                        assert_eq!(raster.pixel(x, y), Some(true));
                    }
                }
            }
        }
        assert!(rules > 1);
    }
}

#[test]
fn strikethrough_does_not_hide_invalid_note_ids() {
    let id = tm20_set::NoteId::from_index(std::num::NonZeroU32::new(1).unwrap());
    let sheet = Sheet::tape(vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![Span::Strike(vec![Span::note(id)])],
    })]);
    assert!(matches!(
        sheet.admit(),
        Err(tm20_set::Error::InvalidNote { .. })
    ));
}

fn utf8_splits(s: &str) -> Vec<(String, String)> {
    let mut out = vec![
        (String::new(), s.to_string()),
        (s.to_string(), String::new()),
    ];
    for (i, _) in s.char_indices().skip(1) {
        out.push((s[..i].to_string(), s[i..].to_string()));
    }
    out
}

#[test]
fn wrap_makes_taller_than_one_line() {
    let faces = common::table();
    let one = common::compose_raster(&Sheet::tape(vec![common::text("Hello")]), &faces);
    let wrapped = common::compose_raster(
        &Sheet::tape(vec![common::text(
            "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
        )]),
        &faces,
    );
    assert!(wrapped.height() > one.height());
}

#[test]
fn wrap_first_line_hugs_the_measure() {
    let faces = common::table();
    let r = common::compose_raster(
        &Sheet::tape(vec![common::text(
            "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello",
        )]),
        &faces,
    );
    let skip = u32::from(common::l11());
    let (_, _, y0, y1) = common::ink_bbox(&r);
    assert!(y1 - y0 > skip, "need at least two lines to judge the rag");
    let first = common::rightmost_in(&r, y0, y0 + skip);
    assert!(
        u32::from(first) * 10 >= u32::from(r.width()) * 7,
        "first line rightmost {first} should hug the measure {}, not an even rag",
        r.width()
    );
}

#[test]
fn wrap_last_line_is_not_a_widow() {
    let faces = common::table();
    let r = common::compose_raster(
        &Sheet::tape(vec![common::text(
            "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello you type.",
        )]),
        &faces,
    );
    let skip = u32::from(common::l11());
    let (_, _, y0, y1) = common::ink_bbox(&r);
    assert!(y1 - y0 > skip, "need at least two lines");
    let last0 = y1.saturating_sub(skip);
    let last = common::rightmost_in(&r, last0, y1 + 1);
    let widow = common::compose_raster(&Sheet::tape(vec![common::text("type.")]), &faces);
    let (_, w1, _, _) = common::ink_bbox(&widow);
    assert!(
        last > w1 + 20,
        "last line rightmost {last} should hold more than a widow (type. ends at {w1})"
    );
}

#[test]
fn mono_span_does_not_break_at_its_spaces() {
    let faces = common::table();
    let filler = "Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello Hello";
    let roman = common::compose_raster(
        &Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![Span::new(Cut::Roman, format!("{filler} aa bb"))],
        })]),
        &faces,
    );
    let mono = common::compose_raster(
        &Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![
                Span::new(Cut::Roman, format!("{filler} ")),
                Span::new(Cut::Mono, "aa bb"),
            ],
        })]),
        &faces,
    );
    assert!(
        mono.height() >= roman.height(),
        "mono box {} should not split aa onto the first line the way roman {} can",
        mono.height(),
        roman.height()
    );
}

#[test]
fn two_paragraphs_are_a_blank_line_apart() {
    let faces = common::table();
    let lines = common::compose_raster(&Sheet::tape(vec![common::text("H\nH")]), &faces);
    let paras = common::compose_raster(
        &Sheet::tape(vec![common::text("H"), common::text("H")]),
        &faces,
    );
    let extra = paras.height() as i32 - lines.height() as i32;
    let l = i32::from(common::l11());
    assert!(
        extra >= l - 4 && extra <= l + 8,
        "paragraph extra {extra} should be one leading ({l}), not line skip"
    );
}

#[test]
fn adjacent_cuts_do_not_grow_a_word_space() {
    let faces = common::table();
    let r = common::compose_raster(
        &Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![Span::new(Cut::Roman, "mid"), Span::new(Cut::Roman, "word")],
        })]),
        &faces,
    );
    let spaced = common::compose_raster(
        &Sheet::tape(vec![Frame::Text(TextBlock::plain(
            Cut::Roman,
            TextSize::Pt11,
            "mid word",
        ))]),
        &faces,
    );
    let (_, tight_r, ..) = common::ink_bbox(&r);
    let (_, loose_r, ..) = common::ink_bbox(&spaced);
    assert!(
        tight_r + GRID < loose_r,
        "Glue::None run ({tight_r}) must be shorter than a word-spaced run ({loose_r})"
    );
}

#[test]
fn inline_math_slug_follows_the_ink() {
    let faces = common::table();
    let bits = vec![true; 8 * 60];
    let tall = Math::from_bits(8, 60, &bits, 40).unwrap();
    let short = Math::from_bits(8, 8, &bits[..64], 6).unwrap();
    let with_frac = common::compose_raster(
        &Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![Span::new(Cut::Roman, "x"), Span::math(tall)],
        })]),
        &faces,
    );
    let with_short = common::compose_raster(
        &Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![Span::new(Cut::Roman, "x"), Span::math(short)],
        })]),
        &faces,
    );
    assert!(
        with_frac.height() >= 60,
        "tall inline math should grow the slug, got {}",
        with_frac.height()
    );
    assert!(
        with_short.height() <= u32::from(common::l11()) + 4,
        "short inline math should sit on the prose slug, got {}",
        with_short.height()
    );
}

#[test]
fn word_space_after_math_is_a_word_space() {
    let faces = common::table();
    let bits = vec![true; 8 * 8];
    let math = Math::from_bits(8, 8, &bits, 6).unwrap();
    let jammed = common::compose_raster(
        &Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![
                Span::new(Cut::Roman, "x"),
                Span::math(math.clone()),
                Span::new(Cut::Roman, "y"),
            ],
        })]),
        &faces,
    );
    let spaced = common::compose_raster(
        &Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![
                Span::new(Cut::Roman, "x "),
                Span::math(math),
                Span::new(Cut::Roman, " y"),
            ],
        })]),
        &faces,
    );
    let (_, jam_r, ..) = common::ink_bbox(&jammed);
    let (_, space_r, ..) = common::ink_bbox(&spaced);
    assert!(
        space_r >= jam_r + GRID,
        "a word space after math cannot collapse; jammed {jam_r} spaced {space_r}"
    );
}

#[test]
fn long_token_breaks_at_its_punctuation() {
    let faces = common::table();
    let url = "https://example.com/a/very/long/path/that/cannot/fit/on/one/line/of/the/tape/ever";
    let r = common::compose_raster(
        &Sheet::tape(vec![Frame::Text(TextBlock::plain(
            Cut::Italic,
            TextSize::Pt11,
            url,
        ))]),
        &faces,
    );
    assert!(
        r.height() >= u32::from(common::l11()) * 2,
        "the token wrapped over lines instead of clipping on one: {}",
        r.height()
    );
}

#[test]
fn same_context_utf8_partitions_match_av() {
    let faces = common::table();
    let whole = common::compose_raster(
        &Sheet::tape(vec![Frame::Text(TextBlock::plain(
            Cut::Roman,
            TextSize::Pt11,
            "AV",
        ))]),
        &faces,
    );
    for (a, b) in utf8_splits("AV") {
        let mut spans = Vec::new();
        if !a.is_empty() {
            spans.push(Span::new(Cut::Roman, a.clone()));
        }
        if !b.is_empty() {
            spans.push(Span::new(Cut::Roman, b.clone()));
        }
        if spans.is_empty() {
            continue;
        }
        let part = common::compose_raster(
            &Sheet::tape(vec![Frame::Text(TextBlock {
                size: TextSize::Pt11,
                spans,
            })]),
            &faces,
        );
        assert_eq!(
            (part.width(), part.height(), part.pixels()),
            (whole.width(), whole.height(), whole.pixels()),
            "Roman split {a:?}+{b:?} must match AV"
        );
    }
}

#[test]
fn same_context_partitions_match_hello_and_fi() {
    let faces = common::table();
    for word in ["hello", "fi"] {
        let whole = common::compose_raster(
            &Sheet::tape(vec![Frame::Text(TextBlock::plain(
                Cut::Roman,
                TextSize::Pt11,
                word,
            ))]),
            &faces,
        );
        for (a, b) in utf8_splits(word) {
            let mut spans = Vec::new();
            if !a.is_empty() {
                spans.push(Span::new(Cut::Roman, a.clone()));
            }
            if !b.is_empty() {
                spans.push(Span::new(Cut::Roman, b.clone()));
            }
            if spans.is_empty() {
                continue;
            }
            let part = common::compose_raster(
                &Sheet::tape(vec![Frame::Text(TextBlock {
                    size: TextSize::Pt11,
                    spans,
                })]),
                &faces,
            );
            assert_eq!(
                part.pixels(),
                whole.pixels(),
                "{word} split {a:?}+{b:?} must match"
            );
        }
    }
}

#[test]
fn cut_change_and_separate_code_boxes_are_barriers() {
    let faces = common::table();
    let roman = common::compose_raster(
        &Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![Span::new(Cut::Roman, "AV")],
        })]),
        &faces,
    );
    let mixed = common::compose_raster(
        &Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![Span::new(Cut::Roman, "A"), Span::new(Cut::Italic, "V")],
        })]),
        &faces,
    );
    assert_ne!(
        roman.pixels(),
        mixed.pixels(),
        "a cut change is a real shaping barrier"
    );
    // Menlo has no AV kerning, so tape-wide pixels of `AV` vs `A`+`V` can
    // match. The barrier is wrap: one atomic box stays one line; two boxes
    // break once the measure fits a letter and not a pair.
    let probe = common::compose_raster(
        &Sheet::tape(vec![Frame::Text(TextBlock {
            size: TextSize::Pt11,
            spans: vec![Span::new(Cut::Mono, "AV")],
        })]),
        &faces,
    );
    let (_, x1, _, _) = common::ink_bbox(&probe);
    let pair = u32::from(x1.saturating_add(1));
    let letter = (pair / 2).max(1);
    let narrow = Measure::new(u16::try_from(letter.saturating_add(2)).unwrap().max(1))
        .expect("letter-wide measure");
    assert!(
        u32::from(narrow.get()) < pair,
        "probe must leave a one-letter measure ({} >= pair {pair})",
        narrow.get()
    );
    let mut one_box = Sheet::tape(vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![Span::new(Cut::Mono, "AV")],
    })]);
    one_box.width = narrow;
    let mut two_boxes = Sheet::tape(vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![Span::new(Cut::Mono, "A"), Span::new(Cut::Mono, "V")],
    })]);
    two_boxes.width = narrow;
    assert!(matches!(
        compose(&one_box, &faces),
        Err(tm20_set::Error::Clipped { .. })
    ));
    let two_mono = common::compose_raster(&two_boxes, &faces);
    assert!(
        two_mono.height() > u32::from(TextSize::Pt11.skip_dots()),
        "independently authored code boxes must wrap apart"
    );
}

#[test]
fn note_only_and_mixed_note_math_lie_inside_measured_bounds() {
    let faces = common::table();
    let mut note_only = Sheet::tape(vec![]);
    let id = note_only
        .add_note(Note::dest("https://example.com"))
        .unwrap();
    note_only.frames = vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![Span::note(id)],
    })];
    let r = common::compose_raster(&note_only, &faces);
    let (x0, x1, y0, y1) = common::ink_bbox(&r);
    assert!(y1 >= y0, "note-only line has ink");
    assert!(x1 >= x0);
    assert!(y0 < r.height(), "topmost reference ink is on the canvas");
    assert!(
        y0 < 8,
        "note-only first ink must not sit above the canvas or be clipped away (y0={y0})"
    );

    let mut lower = Sheet::tape(vec![]);
    let id = lower.add_note(Note::dest("n")).unwrap();
    lower.frames = vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![Span::new(Cut::Roman, "a"), Span::note(id)],
    })];
    let mixed = common::compose_raster(&lower, &faces);
    let (_, _, my0, my1) = common::ink_bbox(&mixed);
    assert!(my1 >= my0);

    let bits = vec![true; 8 * 8];
    let math = Math::from_bits(8, 8, &bits, 6).unwrap();
    let mut math_note = Sheet::tape(vec![]);
    let id = math_note.add_note(Note::dest("m")).unwrap();
    math_note.frames = vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![Span::math(math), Span::note(id)],
    })];
    let mn = common::compose_raster(&math_note, &faces);
    let (nx0, nx1, ny0, ny1) = common::ink_bbox(&mn);
    assert!(ny1 >= ny0 && nx1 >= nx0);

    let mut many = Sheet::tape(vec![]);
    let a = many.add_note(Note::dest("one")).unwrap();
    let b = many.add_note(Note::dest("two")).unwrap();
    many.frames = vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![Span::note(a), Span::note(b)],
    })];
    let adj = common::compose_raster(&many, &faces);
    assert!(common::ink_count(&adj) > 0);
}

#[test]
fn independent_note_spans_preserve_every_occurrence() {
    let faces = common::table();
    let mut sheet = Sheet::tape(vec![]);
    let dest = sheet.add_note(Note::dest("https://example.com")).unwrap();
    let foot = sheet
        .add_note(Note::Blocks(vec![common::text("note")]))
        .unwrap();
    sheet.frames = vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![
            Span::new(Cut::Italic, "label"),
            Span::note(foot),
            Span::note(dest),
        ],
    })];
    let r = common::compose_raster(&sheet, &faces);
    let skip = u32::from(TextSize::Pt11.skip_dots());
    let body_end = skip.max(1);
    let body_runs = common::ink_runs(&r, 0, body_end);
    assert!(
        body_runs.len() >= 3,
        "label plus two independent note markers: {body_runs:?}"
    );
    let rule_y = (0..r.height())
        .find(|&y| {
            let mut ink = 0u16;
            let mut last = 0u16;
            for x in 0..r.width() {
                if common::packed_ink(&r, y, x) {
                    ink += 1;
                    last = x;
                }
            }
            ink >= NOTE_RULE.saturating_sub(8) && last < r.width() / 2
        })
        .expect("apparatus rule");
    assert!(rule_y >= body_end, "rule follows the body, at {rule_y}");
    let hang = 2 * GRID;
    let mut apparatus_marks = 0u8;
    let mut y = rule_y + 1;
    while y < r.height() {
        let mut hit = false;
        for x in 0..hang.min(r.width()) {
            hit |= common::packed_ink(&r, y, x);
        }
        if hit {
            apparatus_marks += 1;
            y += skip;
        } else {
            y += 1;
        }
    }
    assert_eq!(apparatus_marks, 2, "each note owns one apparatus marker");
    compose(&sheet, &faces).unwrap();
}

#[test]
fn wrap_cost_is_lexicographic_and_checked() {
    let a = WrapCost {
        extra_lines: 0,
        raggedness: 10,
    };
    let b = WrapCost {
        extra_lines: 1,
        raggedness: 0,
    };
    assert!(a < b, "extra_lines outranks raggedness");
    assert_eq!(a.checked_add(a).unwrap().raggedness, 20);
    assert!(
        WrapCost {
            extra_lines: u32::MAX,
            raggedness: 0,
        }
        .checked_add(WrapCost {
            extra_lines: 1,
            raggedness: 0,
        })
        .is_none()
    );
    assert!(
        WrapCost {
            extra_lines: 0,
            raggedness: u128::MAX,
        }
        .checked_add(WrapCost {
            extra_lines: 0,
            raggedness: 1,
        })
        .is_none()
    );
}

#[test]
fn unbreakable_token_rejects_instead_of_clipping() {
    let faces = common::table();
    let token = "supercalifragilisticexpialidocioussupercalifragilistic";
    let result = compose(
        &Sheet::tape(vec![Frame::Text(TextBlock::plain(
            Cut::Roman,
            TextSize::Pt11,
            token,
        ))]),
        &faces,
    );
    assert!(matches!(result, Err(tm20_set::Error::Clipped { .. })));
}
