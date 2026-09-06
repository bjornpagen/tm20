//! Pixel-exact visual regression harness. Helpers live in `common/`.

mod common;

use common::admit::{admit_corpus, admit_corpus_allowing, admit_reject, defective_skip_on_missing};
use common::compare;
use common::compare::{compare_raster, format_row, render, write_golden, write_triplet};
use common::faces::require_locked_fonts;
use common::{tests_dir, uniq_temp};

#[test]
fn faces_are_locked() {
    require_locked_fonts();
}

#[test]
fn warm_and_cold_font_caches_produce_identical_pixels() {
    let faces = common::table();
    for stem in [
        "ext-strike-b-mixed",
        "cm-5.3-e-nested-mixed",
        "ext-table-e-cell-content",
    ] {
        let path = tests_dir().join("corpus").join(format!("{stem}.md"));
        let a = compare::try_render_with(&path, &faces).unwrap();
        let b = compare::try_render_with(&path, &faces).unwrap();
        let cold = compare::try_render_with(&path, &faces.clone()).unwrap();
        assert_eq!(
            (a.width(), a.height(), a.pixels()),
            (b.width(), b.height(), b.pixels()),
            "warm cache changed {stem}"
        );
        assert_eq!(
            (a.width(), a.height(), a.pixels()),
            (cold.width(), cold.height(), cold.pixels()),
            "fresh cache changed {stem}"
        );
    }
}

fn reject_case(stem: &str) {
    let path = tests_dir().join("reject").join(format!("{stem}.md"));
    let err = compare::try_render(&path).expect_err("rejection fixture rendered cleanly");
    let at = err
        .location()
        .map_or("0:0".into(), |p| format!("{}:{}", p.line, p.column));
    let got = format!("{at} {}", err.code());
    let want = include_str!("reject/expect.txt")
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == stem).then_some(value.trim())
        })
        .expect("reject expectation");
    assert_eq!(got, want, "{stem}: {err}");
}

fn corpus_case(stem: &str) {
    let path = tests_dir().join("corpus").join(format!("{stem}.md"));
    let fresh = render(&path);
    if let Err(mismatch) = compare_raster(&path, &fresh) {
        if compare::bless_allowed(stem) {
            write_golden(&path, &fresh);
            return;
        }
        write_triplet(&mismatch, &fresh);
        panic!("{}", format_row(&mismatch));
    }
}

include!("common/cases.rs");

cases! { corpus, corpus_case;
    case_cm_2_1_a_line_endings => "cm-2.1-a-line-endings",
    case_cm_2_2_a_tabs_code => "cm-2.2-a-tabs-code",
    case_cm_2_2_b_tabs_lists => "cm-2.2-b-tabs-lists",
    case_cm_2_4_a_escapes => "cm-2.4-a-escapes",
    case_cm_2_4_b_escape_contexts => "cm-2.4-b-escape-contexts",
    case_cm_2_5_a_entities => "cm-2.5-a-entities",
    case_cm_3_1_a_precedence => "cm-3.1-a-precedence",
    case_cm_3_2_a_containers => "cm-3.2-a-containers",
    case_cm_4_1_a_break_markers => "cm-4.1-a-break-markers",
    case_cm_4_1_b_break_vs_setext => "cm-4.1-b-break-vs-setext",
    case_cm_4_2_a_atx_levels => "cm-4.2-a-atx-levels",
    case_cm_4_3_a_setext => "cm-4.3-a-setext",
    case_cm_4_4_a_indented_code => "cm-4.4-a-indented-code",
    case_cm_4_5_a_fences => "cm-4.5-a-fences",
    case_cm_4_5_b_fence_info => "cm-4.5-b-fence-info",
    case_cm_4_5_c_fence_content => "cm-4.5-c-fence-content",
    case_cm_4_5_d_fence_in_contexts => "cm-4.5-d-fence-in-contexts",
    case_cm_4_7_a_ref_links => "cm-4.7-a-ref-links",
    case_cm_4_7_b1_dup_unused => "cm-4.7-b1-dup-unused",
    case_cm_4_7_b2_only_defs => "cm-4.7-b2-only-defs",
    case_cm_4_8_a_paragraphs => "cm-4.8-a-paragraphs",
    case_cm_4_9_a_blank_lines => "cm-4.9-a-blank-lines",
    case_cm_5_1_a_quote_basic => "cm-5.1-a-quote-basic",
    case_cm_5_1_b_quote_lazy => "cm-5.1-b-quote-lazy",
    case_cm_5_1_c_quote_nested => "cm-5.1-c-quote-nested",
    case_cm_5_1_d_quote_contents => "cm-5.1-d-quote-contents",
    case_cm_5_2_a_item_indent => "cm-5.2-a-item-indent",
    case_cm_5_2_b_item_blocks => "cm-5.2-b-item-blocks",
    case_cm_5_2_c_item_empty => "cm-5.2-c-item-empty",
    case_cm_5_2_d_item_heading => "cm-5.2-d-item-heading",
    case_cm_5_3_a_markers => "cm-5.3-a-markers",
    case_cm_5_3_b_ordered_start => "cm-5.3-b-ordered-start",
    case_cm_5_3_c_tight_loose => "cm-5.3-c-tight-loose",
    case_cm_5_3_d_interrupt => "cm-5.3-d-interrupt",
    case_cm_5_3_e_nested_mixed => "cm-5.3-e-nested-mixed",
    case_cm_5_3_f_runover => "cm-5.3-f-runover",
    case_cm_6_1_a_code_spans => "cm-6.1-a-code-spans",
    case_cm_6_1_b_span_breaks => "cm-6.1-b-span-breaks",
    case_cm_6_2_a_flanking => "cm-6.2-a-flanking",
    case_cm_6_2_b_intraword => "cm-6.2-b-intraword",
    case_cm_6_2_c_mixed_delims => "cm-6.2-c-mixed-delims",
    case_cm_6_2_d_adjacent_runs => "cm-6.2-d-adjacent-runs",
    case_cm_6_2_e_punct_flanks => "cm-6.2-e-punct-flanks",
    case_cm_6_3_a_inline_links => "cm-6.3-a-inline-links",
    case_cm_6_3_b_link_nesting => "cm-6.3-b-link-nesting",
    case_cm_6_3_c_link_notes => "cm-6.3-c-link-notes",
    case_cm_6_4_a_images => "cm-6.4-a-images",
    case_cm_6_5_a_autolinks => "cm-6.5-a-autolinks",
    case_cm_6_5_b_gfm_autolink => "cm-6.5-b-gfm-autolink",
    case_cm_6_7_a_hard_breaks => "cm-6.7-a-hard-breaks",
    case_cm_6_8_a_soft_breaks => "cm-6.8-a-soft-breaks",
    case_cm_6_9_a_unicode => "cm-6.9-a-unicode",
    case_doc_a_receipt => "doc-a-receipt",
    case_doc_b_reading => "doc-b-reading",
    case_doc_c_changelog => "doc-c-changelog",
    case_doc_d_spec_sheet => "doc-d-spec-sheet",
    case_ext_foot_a_basic => "ext-foot-a-basic",
    case_ext_foot_b_multiblock => "ext-foot-b-multiblock",
    case_ext_foot_c_order => "ext-foot-c-order",
    case_ext_foot_d_in_cell => "ext-foot-d-in-cell",
    case_ext_foot_e_in_quote => "ext-foot-e-in-quote",
    case_ext_math_a_inline => "ext-math-a-inline",
    case_ext_math_b_display => "ext-math-b-display",
    case_ext_math_c_contexts => "ext-math-c-contexts",
    case_ext_math_d_in_note => "ext-math-d-in-note",
    case_ext_math_e_zoo => "ext-math-e-zoo",
    case_ext_math_f_dollars => "ext-math-f-dollars",
    case_ext_never_b_autolink_off_cases => "ext-never-b-autolink-off-cases",
    case_ext_smart_a_quotes => "ext-smart-a-quotes",
    case_ext_smart_b_dashes => "ext-smart-b-dashes",
    case_ext_strike_a_basic => "ext-strike-a-basic",
    case_ext_strike_b_mixed => "ext-strike-b-mixed",
    case_ext_table_a_two_col => "ext-table-a-two-col",
    case_ext_table_b_three_col => "ext-table-b-three-col",
    case_ext_table_c_squeeze => "ext-table-c-squeeze",
    case_ext_table_e_cell_content => "ext-table-e-cell-content",
    case_ext_table_g_degenerate => "ext-table-g-degenerate",
    case_ext_table_h_numeric => "ext-table-h-numeric",
    case_ext_task_a_basic => "ext-task-a-basic",
    case_ext_task_b_nested_loose => "ext-task-b-nested-loose",
    case_set_cols_a_natural => "set-cols-a-natural",
    case_set_cols_b_end_hang => "set-cols-b-end-hang",
    case_set_edge_a_empty => "set-edge-a-empty",
    case_set_edge_b_blank_only => "set-edge-b-blank-only",
    case_set_edge_c_ends_rule => "set-edge-c-ends-rule",
    case_set_edge_d_ends_figure => "set-edge-d-ends-figure",
    case_set_edge_e_only_figure => "set-edge-e-only-figure",
    case_set_edge_f_notes_after_figure => "set-edge-f-notes-after-figure",
    case_set_fig_a_native => "set-fig-a-native",
    case_set_fig_b_measure_edge => "set-fig-b-measure-edge",
    case_set_fig_c_extreme_aspect => "set-fig-c-extreme-aspect",
    case_set_fig_d_dither => "set-fig-d-dither",
    case_set_fig_e_modes => "set-fig-e-modes",
    case_set_fig_f_jpeg => "set-fig-f-jpeg",
    case_set_nest_a_quote_cap => "set-nest-a-quote-cap",
    case_set_nest_b_list_marker_width => "set-nest-b-list-marker-width",
    case_set_nest_c_hang_pileup => "set-nest-c-hang-pileup",
    case_set_notes_a_many => "set-notes-a-many",
    case_set_notes_c_title_url => "set-notes-c-title-url",
    case_set_pair_a_after_text => "set-pair-a-after-text",
    case_set_pair_b_after_head => "set-pair-b-after-head",
    case_set_pair_c_after_mark => "set-pair-c-after-mark",
    case_set_pair_d_after_list => "set-pair-d-after-list",
    case_set_pair_e_after_cols => "set-pair-e-after-cols",
    case_set_pair_f_after_quote => "set-pair-f-after-quote",
    case_set_pair_g_after_code => "set-pair-g-after-code",
    case_set_pair_h_after_figure => "set-pair-h-after-figure",
    case_set_pair_i_after_math => "set-pair-i-after-math",
    case_set_pair_j_after_rule => "set-pair-j-after-rule",
    case_set_tall_a_band_cross => "set-tall-a-band-cross",
    case_set_wrap_c_cut_boundaries => "set-wrap-c-cut-boundaries",
    case_set_wrap_d_note_at_margin => "set-wrap-d-note-at-margin",
    case_set_wrap_e_full_measure => "set-wrap-e-full-measure",
}

cases! { reject, reject_case;
    case_cm_2_3_a_insecure => "cm-2.3-a-insecure",
    case_cm_4_2_b_atx_forms => "cm-4.2-b-atx-forms",
    case_cm_4_2_c_atx_flatten => "cm-4.2-c-atx-flatten",
    case_cm_6_3_d_brackets => "cm-6.3-d-brackets",
    case_cm_6_9_b_scripts => "cm-6.9-b-scripts",
    case_cm_6_9_c_tofu => "cm-6.9-c-tofu",
    case_ext_table_d_overflow => "ext-table-d-overflow",
    case_ext_table_f_pipes => "ext-table-f-pipes",
    case_rej_html_a_block => "rej-html-a-block",
    case_rej_html_b_inline => "rej-html-b-inline",
    case_rej_html_c_comment => "rej-html-c-comment",
    case_rej_html_d_bare_tag => "rej-html-d-bare-tag",
    case_rej_html_e_heading => "rej-html-e-heading",
    case_rej_image_a_mixed => "rej-image-a-mixed",
    case_rej_image_b_remote => "rej-image-b-remote",
    case_rej_image_c_missing => "rej-image-c-missing",
    case_rej_image_d_garbage => "rej-image-d-garbage",
    case_rej_math_a_heading => "rej-math-a-heading",
    case_rej_math_b_bad_latex => "rej-math-b-bad-latex",
    case_rej_nest_a_quote_4 => "rej-nest-a-quote-4",
    case_rej_nest_b_list_4 => "rej-nest-b-list-4",
    case_rej_table_a_one_col => "rej-table-a-one-col",
    case_rej_table_b_four_col => "rej-table-b-four-col",
    case_set_notes_b_long_url => "set-notes-b-long-url",
    case_set_wrap_a_long_url => "set-wrap-a-long-url",
    case_set_wrap_b_long_word => "set-wrap-b-long-word",
}

#[test]
fn packed_comparison_detects_dimensions_and_the_last_partial_byte() {
    let blank = tm20::Raster::from_bits(9, 2, &[false; 18]).unwrap();
    let mut bits = [false; 18];
    bits[17] = true;
    let ink = tm20::Raster::from_bits(9, 2, &bits).unwrap();
    let decoded = compare::raster_from_png(&compare::encode_golden(&ink));
    assert_eq!(decoded.pixels(), ink.pixels());
    assert!(compare::compare_images("equal".into(), &ink, &decoded).is_ok());
    let mismatch = compare::compare_images("pixel".into(), &blank, &ink)
        .err()
        .unwrap();
    assert!(matches!(mismatch.kind, compare::Kind::Pixels(1)));
    assert_eq!(mismatch.bbox, Some((8, 1, 8, 1)));
    let narrow = tm20::Raster::from_bits(8, 2, &[false; 16]).unwrap();
    let mismatch = compare::compare_images("width".into(), &blank, &narrow)
        .err()
        .unwrap();
    assert!(matches!(
        mismatch.kind,
        compare::Kind::Dims {
            want: (9, 2),
            got: (8, 2)
        }
    ));
}

#[test]
fn missing_empty_orphan_duplicate_fixture_roots_fail() {
    let tmp = uniq_temp("admit");
    assert!(
        defective_skip_on_missing(&tmp),
        "sensitivity: the old skip treated a missing corpus as success"
    );
    assert!(
        admit_corpus(&tmp).is_err(),
        "missing corpus directory must fail admission"
    );

    let empty = uniq_temp("admit-empty");
    std::fs::create_dir_all(empty.join("corpus")).unwrap();
    std::fs::create_dir_all(empty.join("goldens")).unwrap();
    assert!(admit_corpus(&empty).is_err(), "empty corpus must fail");

    let orphan = uniq_temp("admit-orphan");
    std::fs::create_dir_all(orphan.join("corpus")).unwrap();
    std::fs::create_dir_all(orphan.join("goldens")).unwrap();
    std::fs::write(orphan.join("corpus/only.md"), "Hi\n").unwrap();
    std::fs::write(orphan.join("goldens/only.png"), [0u8; 8]).unwrap();
    std::fs::write(orphan.join("goldens/ghost.png"), [0u8; 8]).unwrap();
    assert!(
        admit_corpus(&orphan).is_err(),
        "orphan golden must fail admission"
    );

    let missing_golden = uniq_temp("admit-missing-golden");
    std::fs::create_dir_all(missing_golden.join("corpus")).unwrap();
    std::fs::create_dir_all(missing_golden.join("goldens")).unwrap();
    std::fs::write(missing_golden.join("corpus/only.md"), "Hi\n").unwrap();
    assert!(
        admit_corpus(&missing_golden).is_err(),
        "missing golden must fail admission"
    );

    let dup = uniq_temp("admit-dup");
    std::fs::create_dir_all(dup.join("reject")).unwrap();
    std::fs::write(dup.join("reject/a.md"), "<div></div>\n").unwrap();
    std::fs::write(
        dup.join("reject/expect.txt"),
        "a = raw HTML is not representable\na = again\n",
    )
    .unwrap();
    assert!(
        admit_reject(&dup).is_err(),
        "duplicate expect keys must fail"
    );

    let unread = uniq_temp("admit-unread");
    std::fs::create_dir_all(unread.join("reject")).unwrap();
    std::fs::write(unread.join("reject/a.md"), "<div></div>\n").unwrap();
    // expect.txt absent is unreadable evidence
    assert!(admit_reject(&unread).is_err());
}

#[test]
#[ignore = "run once: cargo test -p tm20-md --test snap -- --ignored write_corpus_assets"]
fn write_corpus_assets() {
    common::assets::write_all(&tests_dir().join("corpus/assets"));
}

#[test]
fn unapproved_stems_cannot_be_blessed() {
    assert!(
        !compare::bless_allowed("not-an-approved-stem"),
        "a stem outside approved_stems must never be blessed"
    );
    let tmp = uniq_temp("bless-deny");
    std::fs::create_dir_all(tmp.join("corpus")).unwrap();
    std::fs::create_dir_all(tmp.join("goldens")).unwrap();
    std::fs::write(tmp.join("corpus/only.md"), "Hi\n").unwrap();
    assert!(
        admit_corpus(&tmp).is_err(),
        "strict admission still requires the golden"
    );
    assert!(
        admit_corpus_allowing(&tmp, &[]).is_err(),
        "empty allow list does not create goldens"
    );
    let allowed = admit_corpus_allowing(&tmp, &["only"]);
    assert!(
        allowed.is_ok(),
        "approved missing stem may be admitted for bless"
    );
}

#[test]
fn real_repository_fixtures_admit() {
    let allow = if compare::bless() {
        compare::approved_stems()
    } else {
        &[]
    };
    let admitted = admit_corpus_allowing(&tests_dir(), allow).unwrap_or_else(|e| panic!("{e}"));
    let stems: Vec<_> = admitted.files.iter().map(|p| compare::stem_of(p)).collect();
    assert_eq!(
        stems,
        corpus::STEMS,
        "update cases! registrations for changed corpus fixtures"
    );
    let admitted = admit_reject(&tests_dir()).unwrap_or_else(|e| panic!("{e}"));
    let stems: Vec<_> = admitted.files.iter().map(|p| compare::stem_of(p)).collect();
    assert_eq!(
        stems,
        reject::STEMS,
        "update cases! registrations for changed rejection fixtures"
    );
}
