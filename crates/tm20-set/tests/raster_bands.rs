//! Raster packing, long pages, local image fit, and exact band reconstruction.

mod common;

use tm20::PRINTABLE_DOTS;
use tm20::command::Command;
use tm20::encode::encode;
use tm20::graphics::max_height;
use tm20::{Raster, graphics};
use tm20_set::{
    Code, Figure, Frame, Head, Measure, Row, Sheet, TextSize, compose, lower, paint, select_bands,
};

fn black_png(w: u32, h: u32) -> Vec<u8> {
    let img = image::GrayImage::from_pixel(w, h, image::Luma([0]));
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    buf
}

fn graphics_bands(doc: &tm20::Document) -> Vec<&tm20::graphics::Graphics> {
    doc.commands()
        .iter()
        .filter_map(|c| match c {
            Command::Graphics(g) => Some(g),
            _ => None,
        })
        .collect()
}

#[test]
fn figure_blits_into_the_canvas() {
    let faces = common::table();
    let bits = vec![true; 8 * 8];
    let r = common::compose_raster(
        &Sheet::tape(vec![Frame::Figure(Figure::from_bits(8, 8, &bits).unwrap())]),
        &faces,
    );
    assert!(common::ink_count(&r) > 0);
}

#[test]
fn figure_narrower_than_the_measure_is_centered() {
    let faces = common::table();
    let bits = vec![true; 8 * 8];
    let r = common::compose_raster(
        &Sheet::tape(vec![Frame::Figure(Figure::from_bits(8, 8, &bits).unwrap())]),
        &faces,
    );
    let left = common::leftmost_ink(&r);
    let want = (PRINTABLE_DOTS - 8) / 2;
    assert_eq!(left, want);
}

#[test]
fn figure_as_wide_as_the_measure_is_a_full_slice() {
    let faces = common::table();
    let w = PRINTABLE_DOTS;
    let bits = vec![true; w as usize];
    let r = common::compose_raster(
        &Sheet::tape(vec![Frame::Figure(Figure::from_bits(w, 1, &bits).unwrap())]),
        &faces,
    );
    assert_eq!(common::leftmost_ink(&r), 0);
}

#[test]
fn packing_round_trip_normalizes_unused_low_bits() {
    let ragged = vec![0b1111_1111];
    let r = Raster::from_packed(5, 1, ragged).unwrap();
    assert_eq!(r.stride(), 1);
    assert_eq!(r.pixels()[0] & 0b0000_0111, 0, "padding bits are zero");
    assert_eq!(r.pixel(0, 0), Some(true));
    assert_eq!(r.pixel(4, 0), Some(true));
    assert_eq!(r.pixel(5, 0), None);
    let again = Raster::from_packed(5, 1, r.pixels().to_vec()).unwrap();
    assert_eq!(r, again);
}

#[test]
fn raster_one_by_65536_retains_every_row() {
    let bits = vec![true; 65_536];
    let r = Raster::from_bits(1, 65_536, &bits).unwrap();
    assert_eq!(r.width(), 1);
    assert_eq!(r.height(), 65_536);
    assert_eq!(r.pixel(0, 65_535), Some(true));
    let last = r.slice_rows(65_535..65_536).unwrap();
    assert_eq!(last.height(), 1);
    assert_eq!(last.pixel(0, 0), Some(true));
}

#[test]
fn figure_from_1x65536_png_retains_height() {
    let png = black_png(1, 65_536);
    let fig = Figure::from_image(&png).unwrap();
    let fitted = fig.fit(Measure::TAPE).unwrap();
    assert_eq!(fitted.width(), 1);
    assert_eq!(fitted.height(), 65_536);
    assert_eq!(fitted.pixel(0, 65_535), Some(true));
}

#[test]
fn sixteen_by_eight_fits_to_eight_by_four() {
    let bits = vec![true; 16 * 8];
    let fig = Figure::from_bits(16, 8, &bits).unwrap();
    let fitted = fig.fit(Measure::new(8).unwrap()).unwrap();
    assert_eq!(fitted.width(), 8);
    assert_eq!(fitted.height(), 4);
    assert!(
        fitted.pixels().iter().any(|&b| b != 0),
        "fit is a reduced image, not a crop of empty paper"
    );
}

#[test]
fn nested_figure_fits_the_local_measure() {
    let faces = common::table();
    let src_w = 80u16;
    let src_h = 16u32;
    // Left bar + right bar: a left crop would drop the right bar.
    let mut bits = vec![false; src_w as usize * src_h as usize];
    for y in 0..src_h {
        for x in 0..8u16 {
            bits[y as usize * src_w as usize + x as usize] = true;
        }
        bits[y as usize * src_w as usize + (src_w as usize - 1)] = true;
    }
    let fig = Figure::from_bits(src_w, src_h, &bits).unwrap();
    let list = common::dash_list(vec![tm20_set::ListItem::new(vec![Frame::Figure(fig)])]);
    let hang = list
        .hang_dots(faces.text(tm20_set::Cut::Roman).unwrap())
        .unwrap();
    let page_w = hang.saturating_add(40).max(96);
    let local = page_w.saturating_sub(hang);
    assert!(
        local < src_w,
        "page must constrain the figure (local {local} hang {hang})"
    );
    let mut sheet = Sheet::tape(vec![tm20_set::Frame::List(list)]);
    sheet.width = Measure::new(page_w).expect("narrow page");
    let r = common::compose_raster(&sheet, &faces);
    let mut mark = false;
    for y in 0..r.height() {
        for x in 0..hang.min(r.width()) {
            mark |= common::packed_ink(&r, y, x);
        }
    }
    assert!(mark, "list marker occupies the hang column");
    let mut fx0 = r.width();
    let mut fx1 = 0u16;
    let mut fy0 = r.height();
    let mut fy1 = 0u32;
    let mut fig_ink = false;
    for y in 0..r.height() {
        for x in hang..r.width() {
            if common::packed_ink(&r, y, x) {
                fig_ink = true;
                fx0 = fx0.min(x);
                fx1 = fx1.max(x);
                fy0 = fy0.min(y);
                fy1 = fy1.max(y);
            }
        }
    }
    assert!(fig_ink, "fitted figure ink sits in the local measure");
    let fw = fx1 - fx0 + 1;
    let fh = fy1 - fy0 + 1;
    assert!(
        fw <= local,
        "figure width {fw} must fit local measure {local}, hang {hang}"
    );
    assert!(fw < src_w, "asymmetric 80-dot source must shrink, got {fw}");
    assert!(
        fh < src_h,
        "proportional fit shortens height (got {fh}, source {src_h})"
    );
    let left_bar = (fy0..=fy1).any(|y| common::packed_ink(&r, y, fx0));
    let right_bar = (fy0..=fy1).any(|y| common::packed_ink(&r, y, fx1));
    assert!(
        left_bar && right_bar,
        "both source bars must survive; a left crop would drop the right bar"
    );
}

#[test]
fn eighteen_hundred_code_lines_survive_exact_reconstruct() {
    let faces = common::table();
    let src = "x\n".repeat(1800);
    let sheet = Sheet::tape(vec![Frame::Code(Code::new(TextSize::Pt11, &src))]);
    let page = common::compose_raster(&sheet, &faces);
    assert!(
        page.height() > 65_535,
        "1,800 code lines must exceed a u16 page (got {})",
        page.height()
    );
    let last_ink = (0..page.height())
        .rev()
        .find(|&y| common::row_has_ink(&page, y))
        .expect("last code line must leave ink");
    let skip = u32::from(TextSize::Pt11.skip_dots());
    assert!(
        page.height() - 1 - last_ink < skip,
        "last-line ink must sit in the final slug (last_ink={last_ink}, height={}, skip={skip}); trailing slug rows may be empty",
        page.height()
    );

    let doc = lower(&sheet, &faces).unwrap();
    let gs = graphics_bands(&doc);
    assert!(!gs.is_empty());
    let cap = max_height(page.width());
    assert!(gs.iter().all(|g| g.raster().height() <= u32::from(cap)));
    for g in &gs {
        graphics::encode(g).unwrap();
    }
    let rasters: Vec<Raster> = gs.iter().map(|g| g.raster().clone()).collect();
    let refs: Vec<&Raster> = rasters.iter().collect();
    common::concat_equals_page(&page, &refs);
    encode(&doc).unwrap();
}

#[test]
fn select_bands_is_minimum_count_latest_seam() {
    let cap = 910u16;
    let height = 2000u32;
    let seams = [Row::new(800), Row::new(1600), Row::new(1990)];
    let ranges = select_bands(height, cap, &seams).unwrap();
    assert!(!ranges.is_empty());
    assert!(ranges.iter().all(|r| r.end().get() - r.start().get() > 0));
    assert_eq!(ranges.first().unwrap().start().get(), 0);
    assert_eq!(ranges.last().unwrap().end().get(), height);
    let mut prev = 0u32;
    for r in &ranges {
        assert_eq!(r.start().get(), prev, "contiguous nonempty ranges");
        assert!(r.end().get() - r.start().get() <= u32::from(cap));
        prev = r.end().get();
    }
    let min = height.div_ceil(u32::from(cap));
    assert_eq!(ranges.len() as u32, min);
}

#[test]
fn paint_rejects_nothing_it_is_given_a_legal_empty_plan() {
    let faces = common::table();
    let sheet = Sheet::tape(vec![]);
    let checked = sheet.admit().unwrap();
    let req = checked.face_requirements();
    let resolved = faces.resolve(&req).unwrap();
    if let Ok(plan) = tm20_set::layout(&checked, &resolved) {
        let raster = paint(&plan).unwrap();
        assert_eq!(raster.height(), 1);
        assert!(raster.pixels().iter().all(|&b| b == 0));
    }
}

#[test]
fn compose_and_lower_agree_on_bytes_for_a_head_and_figure() {
    let faces = common::table();
    let fig_h = 900u32;
    let bits = vec![true; PRINTABLE_DOTS as usize * fig_h as usize];
    let sheet = Sheet::tape(vec![
        Frame::Head(Head {
            size: TextSize::Pt11,
            text: "Head".into(),
        }),
        Frame::Figure(Figure::from_bits(PRINTABLE_DOTS, fig_h, &bits).unwrap()),
    ]);
    let page = compose(&sheet, &faces).unwrap();
    let doc = lower(&sheet, &faces).unwrap();
    let gs = graphics_bands(&doc);
    let rasters: Vec<Raster> = gs.iter().map(|g| g.raster().clone()).collect();
    let refs: Vec<&Raster> = rasters.iter().collect();
    common::concat_equals_page(&page, &refs);
}
