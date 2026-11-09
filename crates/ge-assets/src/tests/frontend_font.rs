use super::*;

const FONT: usize = 0x100;
fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn relative(bytes: &mut [u8], at: usize, target: usize) {
    put(bytes, at, (target as i32 - at as i32) as u32);
}
fn glyph(bytes: &mut [u8], at: usize, uv: [f32; 4], metrics: [u16; 4]) {
    for (index, value) in uv.into_iter().enumerate() {
        put(bytes, at + index * 4, value.to_bits());
    }
    for (index, value) in metrics.into_iter().enumerate() {
        bytes[at + 16 + index * 2..at + 18 + index * 2].copy_from_slice(&value.to_be_bytes());
    }
}
fn data() -> Vec<u8> {
    let mut bytes = vec![0; 0xf00];
    put(&mut bytes, FONT, 3);
    relative(&mut bytes, FONT + 4, 0x600);
    relative(&mut bytes, FONT + 4 + 0x20 * 4, 0xa00);
    relative(&mut bytes, 0x600 + 0x20 * 4, 0xe00);
    relative(&mut bytes, 0x600 + 0x41 * 4, 0xe18);
    relative(&mut bytes, 0xa00 + 0xac * 4, 0xe30);
    glyph(&mut bytes, 0xe00, [0., 0., 0.1, 0.2], [5, 20, 0, 0]);
    glyph(
        &mut bytes,
        0xe18,
        [0.25, 0.5, 0.5, 0.75],
        [12, 20, 1, 0xffff],
    );
    glyph(&mut bytes, 0xe30, [0.5, 0.25, 0.75, 0.5], [13, 20, 2, 3]);
    bytes
}

#[test]
fn sparse_unicode_pages_preserve_uv_dimensions_and_raw_spacing() {
    let font = read_at(&data(), FONT).unwrap();
    assert_eq!(font.texture_selector_raw, 3);
    assert_eq!(
        font.glyphs.iter().map(|g| g.code_unit).collect::<Vec<_>>(),
        vec![0x20, 0x41, 0x20ac]
    );
    let a = font.glyph(0x41).unwrap();
    assert_eq!(a.uv_min, [0.25, 0.5]);
    assert_eq!(a.uv_max, [0.5, 0.75]);
    assert_eq!((a.width, a.height, a.spacing_raw), (12, 20, [1, -1]));
    assert!(font.glyph(0x42).is_none());
    assert!(font.glyph(0xffff).is_none());
}

#[test]
fn negative_relative_offsets_and_aliased_glyphs_are_supported() {
    let mut bytes = data();
    // A second page above its target, then a glyph before its referring entry.
    relative(&mut bytes, FONT + 4 + 0xfe * 4, 0xa00);
    glyph(&mut bytes, 0x80, [0., 0., 0.1, 0.2], [7, 20, 0, 0]);
    relative(&mut bytes, 0xa00 + 0xff * 4, 0x80);
    relative(&mut bytes, 0x600 + 0x42 * 4, 0xe18);
    let font = read_at(&bytes, FONT).unwrap();
    assert_eq!(
        font.glyph(0x42).unwrap().uv_min,
        font.glyph(0x41).unwrap().uv_min
    );
    assert_eq!(font.glyph(0xfeac).unwrap().width, 13);
    assert_eq!(font.glyph(0xfeff).unwrap().width, 7);
    // Move the font header beyond a page to exercise a truly negative page link.
    bytes.resize(0x1600, 0);
    bytes.copy_within(FONT..FONT + 1028, 0x1100);
    relative(&mut bytes, 0x1104, 0x600);
    relative(&mut bytes, 0x1104 + 0x20 * 4, 0xa00);
    relative(&mut bytes, 0x1104 + 0xfe * 4, 0xa00);
    assert_eq!(
        read_at(&bytes, 0x1100).unwrap().glyph(0x41).unwrap().width,
        12
    );
}

#[test]
fn directory_selection_preserves_descriptor_identity_and_native_index_modes() {
    let mut bytes = data();
    let table = Table {
        name: "fonts",
        count: 1,
        hash_size_raw: 1,
        offset: 0x40,
    };
    put(&mut bytes, 0x40, 0x0700002e);
    put(&mut bytes, 0x44, 0x00020009);
    put(&mut bytes, 0x48, FONT as u32);
    put(&mut bytes, 0x4c, 0xdeadbeef);
    for key in [0, 0x80000000, 0x0700002e] {
        let font = read(&bytes, &table, key).unwrap().unwrap();
        assert_eq!(
            (
                font.directory_index,
                font.hash,
                font.section,
                font.debug,
                font.runtime_pointer_raw
            ),
            (0, 0x0700002e, 2, 9, 0xdeadbeef)
        );
        assert_eq!(font.body.glyph(0x41).unwrap().width, 12);
    }
    assert!(read(&bytes, &table, 1).unwrap().is_none());
    assert!(read(&bytes, &table, 0x0700002f).unwrap().is_none());
}

#[test]
fn null_pages_and_null_glyphs_have_no_synthetic_fallback_or_invalid_target_read() {
    let mut bytes = vec![0; FONT + 1028];
    put(&mut bytes, FONT, 0xffffffff);
    let font = read_at(&bytes, FONT).unwrap();
    assert_eq!(font.texture_selector_raw, u32::MAX);
    assert!(font.glyphs.is_empty());
    assert!(font.glyph(0x3f).is_none());
    assert!(read_at(&bytes, usize::MAX).is_err());
}

#[test]
fn truncated_header_page_glyph_and_nonfinite_uv_are_errors() {
    assert!(read_at(&data()[..FONT + 1027], FONT).is_err());
    let mut bytes = data();
    put(&mut bytes, FONT + 4, 0x7fffffff);
    assert!(read_at(&bytes, FONT).is_err());
    let mut bytes = data();
    relative(&mut bytes, 0x600 + 0x41 * 4, 0xef0);
    assert!(read_at(&bytes, FONT).is_err());
    let mut bytes = data();
    put(&mut bytes, 0xe18, f32::NAN.to_bits());
    assert!(read_at(&bytes, FONT).is_err());
    let mut bytes = data();
    put(&mut bytes, 0x600 + 0x41 * 4, 0x80000000);
    assert!(read_at(&bytes, FONT).is_err());
}

#[test]
fn sorted_hash_prefix_resolves_font_without_treating_it_as_descriptor_data() {
    let mut bytes = data();
    let table = Table {
        name: "fonts",
        count: 1,
        hash_size_raw: -1,
        offset: 0x40,
    };
    put(&mut bytes, 0x3c, 0x07000030);
    put(&mut bytes, 0x40, 0x07000030);
    put(&mut bytes, 0x48, FONT as u32);
    assert_eq!(
        read(&bytes, &table, 0x07000030).unwrap().unwrap().hash,
        0x07000030
    );
    let wrong = Table {
        name: "textures",
        ..table
    };
    assert!(read(&bytes, &wrong, 0).is_err());
}
