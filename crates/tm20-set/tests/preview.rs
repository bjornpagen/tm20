//! Preview PNG and page-raster 2× views. Scale is not implicit Normal.

mod common;

use tm20::PRINTABLE_DOTS;
use tm20::Raster;
use tm20::graphics::{Graphics, GraphicsScale};
use tm20_set::{Sheet, preview_png, preview_pngs, preview_raster};

fn decode_luma(png: &[u8]) -> (u32, u32, Vec<u8>) {
    let img = image::load_from_memory(png).expect("png").to_luma8();
    (img.width(), img.height(), img.into_raw())
}

fn solid_graphics(scale: GraphicsScale) -> Graphics {
    let bits = vec![true; 8 * 4];
    let raster = Raster::from_bits(8, 4, &bits).unwrap();
    Graphics::new(raster, scale).unwrap()
}

fn first_dot_graphics(scale: GraphicsScale) -> Graphics {
    let mut bits = vec![false; 8 * 4];
    bits[0] = true;
    let raster = Raster::from_bits(8, 4, &bits).unwrap();
    Graphics::new(raster, scale).unwrap()
}

#[test]
fn preview_png_is_a_png() {
    let faces = common::table();
    let raster = common::compose_raster(&Sheet::tape(vec![common::text("H")]), &faces);
    let g = Graphics::new(raster, GraphicsScale::Normal).unwrap();
    let png = preview_png(&g).unwrap();
    assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
}

#[test]
fn preview_raster_is_2x_nearest_neighbor() {
    let bits = vec![true, false, false, true];
    let raster = Raster::from_bits(2, 2, &bits).unwrap();
    let png = preview_raster(&raster).unwrap();
    let (w, h, luma) = decode_luma(&png);
    assert_eq!((w, h), (4, 4));
    let ink = |x: u32, y: u32| luma[(y * w + x) as usize] < 128;
    assert!(ink(0, 0) && ink(1, 0) && ink(0, 1) && ink(1, 1));
    assert!(!ink(2, 0) && !ink(3, 0));
    assert!(ink(2, 2) && ink(3, 3));
}

#[test]
fn graphics_preview_applies_commanded_scale_then_2x() {
    let cases = [
        (GraphicsScale::Normal, 16u32, 8u32),
        (GraphicsScale::DoubleWidth, 32, 8),
        (GraphicsScale::DoubleHeight, 16, 16),
        (GraphicsScale::Quadruple, 32, 16),
    ];
    for (scale, want_w, want_h) in cases {
        let png = preview_png(&first_dot_graphics(scale)).unwrap();
        let (w, h, luma) = decode_luma(&png);
        assert_eq!((w, h), (want_w, want_h), "{scale:?}");
        assert!(luma.iter().any(|&b| b < 128), "{scale:?} has ink");
        assert!(
            luma.iter().any(|&b| b >= 128),
            "{scale:?} commanded scale then 2× must leave paper"
        );
    }
}

#[test]
fn stitched_equal_effective_widths_replicate() {
    let a = solid_graphics(GraphicsScale::Normal);
    let b = solid_graphics(GraphicsScale::Normal);
    let png = preview_pngs([&a, &b]).unwrap();
    let (w, h, _) = decode_luma(&png);
    assert_eq!(w, 16);
    assert_eq!(h, 16);
}

#[test]
fn incompatible_effective_widths_reject_before_allocation() {
    let a = solid_graphics(GraphicsScale::Normal);
    let b = solid_graphics(GraphicsScale::DoubleWidth);
    assert!(preview_pngs([&a, &b]).is_err());
}

#[test]
fn empty_band_iteration_is_minimal_white() {
    let png = preview_pngs(std::iter::empty::<&Graphics>()).unwrap();
    let (w, h, luma) = decode_luma(&png);
    assert!(w >= 1 && h >= 1);
    assert!(luma.iter().all(|&b| b >= 128), "empty preview is paper");
}

#[test]
fn tape_compose_preview_is_1152_wide() {
    let faces = common::table();
    let raster = common::compose_raster(&Sheet::tape(vec![common::text("H")]), &faces);
    assert_eq!(raster.width(), PRINTABLE_DOTS);
    let g = Graphics::new(raster, GraphicsScale::Normal).unwrap();
    let png = preview_png(&g).unwrap();
    let (w, _, _) = decode_luma(&png);
    assert_eq!(w, u32::from(PRINTABLE_DOTS) * 2);
}
