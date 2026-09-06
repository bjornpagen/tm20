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

include!(concat!(env!("OUT_DIR"), "/snap_cases.rs"));

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
        "rebuild to discover changed corpus fixtures"
    );
    let admitted = admit_reject(&tests_dir()).unwrap_or_else(|e| panic!("{e}"));
    let stems: Vec<_> = admitted.files.iter().map(|p| compare::stem_of(p)).collect();
    assert_eq!(
        stems,
        reject::STEMS,
        "rebuild to discover changed rejection fixtures"
    );
}
