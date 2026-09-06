//! Font-free Raster packing laws.

use tm20::{
    Raster, RasterError,
    graphics::{Graphics, GraphicsScale, max_height},
};

#[test]
fn zero_dimensions_reject() {
    assert!(matches!(
        Raster::from_packed(0, 1, vec![0]),
        Err(RasterError::ZeroWidth)
    ));
    assert!(matches!(
        Raster::from_packed(8, 0, vec![]),
        Err(RasterError::ZeroHeight)
    ));
}

#[test]
fn ragged_and_overlong_packed_length_reject() {
    assert!(matches!(
        Raster::from_packed(8, 1, vec![]),
        Err(RasterError::LengthMismatch {
            expected: 1,
            got: 0
        })
    ));
    assert!(matches!(
        Raster::from_packed(8, 1, vec![0, 0]),
        Err(RasterError::LengthMismatch {
            expected: 1,
            got: 2
        })
    ));
}

#[test]
fn non_byte_aligned_widths_normalize_padding() {
    for width in 1u16..=16 {
        let bits: Vec<bool> = (0..usize::from(width)).map(|i| i % 2 == 0).collect();
        let r = Raster::from_bits(width, 1, &bits).unwrap();
        assert_eq!(r.width(), width);
        assert_eq!(r.height(), 1);
        let used = width % 8;
        if used != 0 {
            let mask = 0xffu8 >> used;
            assert_eq!(r.pixels()[r.stride() - 1] & mask, 0);
        }
        let again = Raster::from_packed(width, 1, r.pixels().to_vec()).unwrap();
        assert_eq!(r, again);
        assert_eq!(r.pixel(width, 0), None);
        assert_eq!(r.pixel(0, 1), None);
    }
}

#[test]
fn one_by_65536_succeeds_and_graphics_cap_is_separate() {
    let r = Raster::from_bits(1, 65_536, &vec![true; 65_536]).unwrap();
    assert_eq!(r.height(), 65_536);
    assert_eq!(r.pixel(0, 65_535), Some(true));
    assert!(Graphics::new(r, GraphicsScale::Normal).is_err());
}

#[test]
fn slice_rows_rejects_empty_and_oob() {
    let r = Raster::from_bits(8, 4, &[true; 32]).unwrap();
    assert!(r.slice_rows(1..1).is_err());
    assert!(r.slice_rows(3..5).is_err());
    assert_eq!(r.slice_rows(1..3).unwrap().height(), 2);
}

#[test]
fn tape_910_encodes_911_does_not() {
    let stride = 72usize;
    let ok = Raster::from_packed(576, 910, vec![0u8; stride * 910]).unwrap();
    Graphics::new(ok, GraphicsScale::Normal).unwrap();
    let tall = Raster::from_packed(576, 911, vec![0u8; stride * 911]).unwrap();
    assert!(Graphics::new(tall, GraphicsScale::Normal).is_err());
    assert_eq!(max_height(576), 910);
}
