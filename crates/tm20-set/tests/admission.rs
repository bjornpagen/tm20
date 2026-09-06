//! Constructor and admission integration. Font-free where possible; compose
//! facts still require the locked house faces.

mod common;

use tm20::PRINTABLE_DOTS;
use tm20::Raster;
use tm20_set::{
    CheckedSheet, Code, Cut, DisplayCut, DisplaySize, FaceRequirements, Figure, Frame, Head, Mark,
    MarkAlign, Math, Measure, Note, NoteId, Quote, Sheet, SheetBuilder, Span, TextBlock, TextSize,
    Tracking, compose,
};

fn note_id(n: u32) -> NoteId {
    NoteId::from_index(std::num::NonZeroU32::new(n).expect("nonzero"))
}

#[test]
fn compose_is_tape_wide() {
    let faces = common::table();
    let r = common::compose_raster(&Sheet::tape(vec![common::text("Hello")]), &faces);
    assert_eq!(r.width(), PRINTABLE_DOTS);
    assert!(r.pixels().iter().any(|&b| b != 0));
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
                cut: DisplayCut::Roman,
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
fn empty_sheet_charges_nothing() {
    let faces = common::table();
    let r = common::compose_raster(&Sheet::tape(vec![]), &faces);
    assert_eq!(r.height(), 1, "an empty sheet is one blank row, not a slug");
    assert!(r.pixels().iter().all(|&b| b == 0), "and it has no ink");
}

#[test]
fn missing_cut_is_an_error_at_the_boundary() {
    let faces = common::partial_table(false, true, true);
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
    let faces = common::partial_table(true, false, true);
    let err = compose(
        &Sheet::tape(vec![Frame::Code(Code::new(TextSize::Pt11, "H"))]),
        &faces,
    )
    .unwrap_err();
    assert!(matches!(err, tm20_set::Error::MissingText(Cut::Mono)));
}

#[test]
fn missing_display_is_an_error() {
    let faces = common::partial_table(true, true, false);
    let err = compose(
        &Sheet::tape(vec![Frame::Mark(Mark {
            cut: DisplayCut::Roman,
            size: DisplaySize::Pt18,
            text: "H".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        })]),
        &faces,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        tm20_set::Error::MissingDisplay(DisplayCut::Roman)
    ));
}

#[test]
fn unused_light_is_not_required() {
    let faces = common::table();
    let sheet = Sheet::tape(vec![common::text("H")]);
    let checked = sheet.admit().unwrap();
    let req = checked.face_requirements();
    assert!(
        !req.display[DisplayCut::Light as usize],
        "unused Light must not be a required cut"
    );
    faces.resolve(&req).unwrap();
}

#[test]
fn measure_zero_and_over_tape_reject() {
    assert!(Measure::new(0).is_none());
    assert!(Measure::new(PRINTABLE_DOTS + 1).is_none());
    assert_eq!(Measure::new(1).unwrap().get(), 1);
    assert_eq!(Measure::TAPE.get(), PRINTABLE_DOTS);
}

#[test]
fn raster_constructors_reject_malformed_public_values() {
    assert!(matches!(
        Raster::from_packed(0, 1, vec![0]),
        Err(tm20::RasterError::ZeroWidth)
    ));
    assert!(matches!(
        Raster::from_packed(8, 0, vec![]),
        Err(tm20::RasterError::ZeroHeight)
    ));
    assert!(matches!(
        Raster::from_packed(8, 1, vec![]),
        Err(tm20::RasterError::LengthMismatch { .. })
    ));
    assert!(matches!(
        Raster::from_packed(8, 1, vec![0, 0]),
        Err(tm20::RasterError::LengthMismatch { .. })
    ));
    assert!(Raster::from_bits(1, 1, &[true]).is_ok());
    assert!(Raster::from_bits(9, 1, &[false; 9]).is_ok());
}

#[test]
fn figure_and_math_reject_ragged_and_zero() {
    assert!(Figure::from_bits(8, 1, &[]).is_err());
    assert!(Figure::from_bits(0, 1, &[true]).is_err());
    assert!(Math::from_bits(8, 8, &[true; 64], 9).is_err());
}

#[test]
fn code_normalizes_crlf_cr_lf_and_tabs() {
    let a = Code::new(TextSize::Pt11, "col\tumn\r\nnext\rlast\n");
    let b = Code::new(TextSize::Pt11, "col\tumn\nnext\nlast\n");
    assert_eq!(a.lines(), b.lines());
    assert_eq!(a.lines()[0].as_ref(), "col     umn");
    assert!(a.lines().iter().all(|l| !l.contains('\t')));
}

#[test]
fn sheet_builder_rejects_unknown_double_defined_and_reserved() {
    let mut b = SheetBuilder::new(Measure::TAPE);
    let id = b.reserve_note().unwrap();
    assert_eq!(id.get_u32(), 1);
    b.define_note(id, Note::dest("https://example.com"))
        .unwrap();
    assert!(b.define_note(id, Note::dest("twice")).is_err());
    assert!(b.define_note(note_id(9), Note::dest("ghost")).is_err());
    let mut unfinished = SheetBuilder::new(Measure::TAPE);
    unfinished.reserve_note().unwrap();
    assert!(unfinished.finish().is_err());
}

#[test]
fn sheet_builder_forward_reference_finishes() {
    let mut b = SheetBuilder::new(Measure::TAPE);
    let id = b.reserve_note().unwrap();
    b.push_frame(Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![Span::new(Cut::Roman, "See"), Span::note(id)],
    }));
    b.define_note(id, Note::dest("https://example.com"))
        .unwrap();
    let sheet = b.finish().unwrap();
    assert_eq!(sheet.note_count(), 1);
    sheet.admit().unwrap();
}

#[test]
fn invalid_note_id_is_rejected_at_admission() {
    let sheet = Sheet::tape(vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![Span::note(note_id(1))],
    })]);
    match sheet.admit() {
        Err(tm20_set::Error::InvalidNote {
            number: 1,
            count: 0,
        }) => {}
        Ok(_) => panic!("expected InvalidNote, admit succeeded"),
        Err(other) => panic!("expected InvalidNote {{ number: 1, count: 0 }}, got {other:?}"),
    }
}

#[test]
fn add_note_then_reference_admits() {
    let mut sheet = Sheet::tape(vec![]);
    let id = sheet.add_note(Note::dest("https://example.com")).unwrap();
    sheet.frames.push(Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![Span::new(Cut::Italic, "Canon"), Span::note(id)],
    }));
    let checked: CheckedSheet<'_, '_> = sheet.admit().unwrap();
    assert_eq!(checked.notes().len(), 1);
    assert_eq!(checked.frames().len(), 1);
}

#[test]
fn three_deep_body_and_note_lists_admit_four_reject() {
    let faces = common::table();
    compose(&Sheet::tape(vec![common::nest_lists(3)]), &faces).unwrap();
    assert!(matches!(
        compose(&Sheet::tape(vec![common::nest_lists(4)]), &faces),
        Err(tm20_set::Error::Nesting)
    ));
    compose(&Sheet::tape(vec![common::nest_quotes(3)]), &faces).unwrap();
    assert!(matches!(
        compose(&Sheet::tape(vec![common::nest_quotes(4)]), &faces),
        Err(tm20_set::Error::Nesting)
    ));

    let mut body_ok = Sheet::tape(vec![common::text("A"), {
        let mut t = TextBlock::plain(Cut::Roman, TextSize::Pt11, "ref");
        // filled after add_note
        t.spans = vec![];
        Frame::Text(t)
    }]);
    let id = body_ok
        .add_note(Note::Blocks(vec![common::nest_lists(3)]))
        .unwrap();
    body_ok.frames = vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![Span::new(Cut::Roman, "A"), Span::note(id)],
    })];
    body_ok.admit().unwrap();
    compose(&body_ok, &faces).unwrap();

    let mut body_bad = Sheet::tape(vec![]);
    let id = body_bad
        .add_note(Note::Blocks(vec![common::nest_lists(4)]))
        .unwrap();
    body_bad.frames = vec![Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![Span::new(Cut::Roman, "A"), Span::note(id)],
    })];
    assert!(
        matches!(body_bad.admit(), Err(tm20_set::Error::Nesting))
            || matches!(compose(&body_bad, &faces), Err(tm20_set::Error::Nesting))
    );
}

#[test]
fn face_requirements_include_notes_lists_and_display() {
    let mut sheet = Sheet::tape(vec![
        Frame::Mark(Mark {
            cut: DisplayCut::Roman,
            size: tm20_set::DisplaySize::Pt18,
            text: "H".into(),
            align: MarkAlign::Start,
            tracking: Tracking(0),
        }),
        Frame::List(common::dash_list(vec![common::item("H")])),
        Frame::Code(Code::new(TextSize::Pt11, "x")),
    ]);
    let id = sheet.add_note(Note::dest("https://example.com")).unwrap();
    sheet.frames.push(Frame::Text(TextBlock {
        size: TextSize::Pt11,
        spans: vec![Span::new(Cut::Italic, "see"), Span::note(id)],
    }));
    let req: FaceRequirements = sheet.admit().unwrap().face_requirements();
    assert!(req.text[Cut::Roman as usize]);
    assert!(req.text[Cut::Mono as usize]);
    assert!(req.display[DisplayCut::Roman as usize]);
}

#[test]
fn empty_quote_is_legal_authoring() {
    let sheet = Sheet::tape(vec![Frame::Quote(Quote { frames: vec![] })]);
    sheet.admit().unwrap();
}

#[test]
fn graphics_cannot_be_assembled_from_ragged_public_fields() {
    let ok = Raster::from_bits(8, 1, &[true; 8]).unwrap();
    tm20::Graphics::new(ok, tm20::GraphicsScale::Normal).unwrap();
    let tall = Raster::from_bits(
        PRINTABLE_DOTS,
        911,
        &vec![false; PRINTABLE_DOTS as usize * 911],
    )
    .unwrap();
    assert!(tm20::Graphics::new(tall, tm20::GraphicsScale::Normal).is_err());
}
