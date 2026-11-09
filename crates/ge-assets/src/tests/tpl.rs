use super::*;

pub(crate) fn fixture(width: u16, height: u16, format: u32, pixels: &[u8]) -> Vec<u8> {
    let mut data = vec![0; 64];
    for (offset, value) in [
        (0, 0x0020af30u32),
        (4, 1),
        (8, 12),
        (12, 20),
        (24, format),
        (28, 64),
    ] {
        data[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }
    data[20..22].copy_from_slice(&height.to_be_bytes());
    data[22..24].copy_from_slice(&width.to_be_bytes());
    data.extend(pixels);
    data
}

#[test]
fn intensity_and_alpha_byte_order_match_gx_samples() {
    for (format, encoded, expected) in [
        (0, vec![0x38; 32], [51, 51, 51, 51]),
        (1, vec![0x42; 32], [66, 66, 66, 66]),
        (2, vec![0xa3; 32], [51, 51, 51, 170]),
        (3, [0x80, 0x42].repeat(16), [66, 66, 66, 128]),
        (4, [0xf8, 0x00].repeat(16), [255, 0, 0, 255]),
        (5, [0x3f, 0x0f].repeat(16), [255, 0, 255, 109]),
    ] {
        let data = fixture(1, 1, format, &encoded);
        let image = Tpl::parse(&data).unwrap().decode(0, 0).unwrap();
        assert_eq!(image.pixels, expected, "format {format}");
    }
}

#[test]
fn rgba8_reads_separate_ar_and_gb_planes_and_clips_padding() {
    let mut data = vec![0; 128];
    data[0..4].copy_from_slice(&[17, 255, 34, 0]);
    data[32..36].copy_from_slice(&[0, 0, 255, 0]);
    data[64..66].copy_from_slice(&[255, 0]);
    data[96..98].copy_from_slice(&[0, 255]);
    let input = fixture(5, 1, 6, &data);
    let image = Tpl::parse(&input).unwrap().decode(0, 0).unwrap();
    assert_eq!(&image.pixels[..8], &[255, 0, 0, 17, 0, 255, 0, 34]);
    assert_eq!(&image.pixels[16..20], &[0, 0, 255, 255]);
    assert_eq!(image.pixels.len(), 20);
}

#[test]
fn tile_rows_and_mip_offsets_include_full_padding() {
    let input = fixture(
        9,
        5,
        1,
        &[vec![20; 32], vec![40; 32], vec![60; 32], vec![80; 32]].concat(),
    );
    let image = Tpl::parse(&input).unwrap().decode(0, 0).unwrap();
    assert_eq!(image.pixels[0], 20);
    assert_eq!(image.pixels[8 * 4], 40);
    assert_eq!(image.pixels[4 * 9 * 4], 60);
    assert_eq!(image.pixels[(4 * 9 + 8) * 4], 80);
    let mut input = fixture(
        4,
        4,
        1,
        &[vec![31; 32], vec![63; 32], vec![127; 32]].concat(),
    );
    input[54] = 2;
    let tpl = Tpl::parse(&input).unwrap();
    let mip = tpl.decode(0, 2).unwrap();
    assert_eq!((mip.width, mip.height), (1, 1));
    assert_eq!(mip.pixels, [127; 4]);
    assert!(tpl.decode(0, 3).is_err());
}

#[test]
fn cmpr_uses_macroblocks_big_endian_selectors_and_gx_interpolation() {
    let mut data = Vec::new();
    for selectors in [0x00u8, 0x55, 0xaa, 0xff] {
        data.extend([0xf8, 0, 0, 0x1f, selectors, selectors, selectors, selectors]);
    }
    let input = fixture(8, 8, 14, &data);
    let image = Tpl::parse(&input).unwrap().decode(0, 0).unwrap();
    assert_eq!(&image.pixels[0..4], &[255, 0, 0, 255]);
    assert_eq!(&image.pixels[4 * 4..5 * 4], &[0, 0, 255, 255]);
    assert_eq!(&image.pixels[4 * 8 * 4..4 * 8 * 4 + 4], &[159, 0, 95, 255]);
    assert_eq!(
        &image.pixels[(4 * 8 + 4) * 4..(4 * 8 + 5) * 4],
        &[95, 0, 159, 255]
    );
    let input = fixture(1, 1, 14, &[0, 0, 255, 255, 255, 255, 255, 255].repeat(4));
    assert_eq!(
        Tpl::parse(&input).unwrap().decode(0, 0).unwrap().pixels,
        [127, 127, 127, 0]
    );
}

#[test]
fn palette_formats_and_index_bounds_are_checked() {
    for format in [8, 9, 10] {
        let mut data = fixture(1, 1, format, &[0; 64]);
        data[16..20].copy_from_slice(&64u32.to_be_bytes());
        data[28..32].copy_from_slice(&96u32.to_be_bytes());
        data[64..66].copy_from_slice(&2u16.to_be_bytes());
        data[68..72].copy_from_slice(&1u32.to_be_bytes());
        data[72..76].copy_from_slice(&80u32.to_be_bytes());
        data[80..84].copy_from_slice(&[0, 0, 0xf8, 0]);
        if format == 8 {
            data[96] = 0x10;
        } else if format == 9 {
            data[96] = 1;
        } else {
            data[97] = 1;
        }
        assert_eq!(
            Tpl::parse(&data).unwrap().decode(0, 0).unwrap().pixels,
            [255, 0, 0, 255]
        );
        data[64..66].copy_from_slice(&1u16.to_be_bytes());
        assert!(Tpl::parse(&data).unwrap().decode(0, 0).is_err());
    }
}

#[test]
fn malformed_headers_and_every_truncation_are_errors() {
    let data = fixture(1, 1, 1, &[128; 32]);
    for end in 0..data.len() {
        assert!(Tpl::parse(&data[..end]).is_err(), "truncation {end}");
    }
    for (offset, value) in [
        (0, 0u32),
        (4, u32::MAX),
        (8, u32::MAX),
        (12, u32::MAX),
        (20, 0),
        (24, 99),
        (28, u32::MAX),
    ] {
        let mut input = data.clone();
        input[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        assert!(Tpl::parse(&input).is_err());
    }
}
