use super::*;

use crate::fixtures::edb_fixture as fixture;

#[test]
fn actor_script_lookup_uses_table_five_and_sixteen_byte_descriptors() {
    let mut bytes = crate::fixtures::graph_fixture();
    bytes[0x68..0x6c].copy_from_slice(&0x00020002u32.to_be_bytes());
    bytes[0x6c..0x70].copy_from_slice(&0x1dcu32.to_be_bytes());
    bytes[0x248..0x24c].copy_from_slice(&0x04000001u32.to_be_bytes());
    bytes[0x24c..0x250].copy_from_slice(&0x00020003u32.to_be_bytes());
    bytes[0x250..0x254].copy_from_slice(&0x100u32.to_be_bytes());
    bytes[0x258..0x25c].copy_from_slice(&0x04000002u32.to_be_bytes());
    bytes[0x260..0x264].copy_from_slice(&0x100u32.to_be_bytes());
    let edb = Edb::parse(bytes.clone()).unwrap();
    assert!(edb.entity_reference(0x04000001).unwrap().is_none());
    let script = edb.animation_script_reference(0x04000001).unwrap().unwrap();
    assert_eq!(
        (
            script.directory_index,
            script.section,
            script.debug,
            script.address
        ),
        (0, 2, 3, 0x100)
    );
    assert_eq!(
        edb.animation_script_reference(0x84000001)
            .unwrap()
            .unwrap()
            .hash,
        0x04000002
    );
    assert!(
        edb.animation_script_reference(0x04000003)
            .unwrap()
            .is_none()
    );
    bytes[0x250..0x254].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(
        Edb::parse(bytes)
            .unwrap()
            .animation_script_reference(0x04000001)
            .is_err()
    );
}

#[test]
fn native_entity_references_distinguish_directory_index_from_literal_hash() {
    let mut bytes = crate::fixtures::graph_fixture();
    bytes[0xd0..0xd4].copy_from_slice(&0x04000d8eu32.to_be_bytes());
    let edb = Edb::parse(bytes.clone()).unwrap();
    let indexed = edb.entity_reference(0x82000000).unwrap().unwrap();
    assert_eq!(indexed.directory_index, 0);
    assert_eq!(indexed.entity.hash, 0x04000d8e);
    assert!(edb.entity_reference(0x04000d8e).unwrap().is_none());
    bytes[0x50..0x54].copy_from_slice(&0x00010001u32.to_be_bytes());
    let edb = Edb::parse(bytes).unwrap();
    assert_eq!(
        edb.entity_reference(0x04000d8e)
            .unwrap()
            .unwrap()
            .directory_index,
        0
    );
    assert!(edb.entity_reference(0x82000001).unwrap().is_none());
}

#[test]
fn malformed_native_entity_hash_directory_is_an_error() {
    let mut bytes = crate::fixtures::graph_fixture();
    bytes[0x50..0x54].copy_from_slice(&0x0001fffeu32.to_be_bytes());
    let edb = Edb::parse(bytes).unwrap();
    assert!(edb.entity_reference(0x04000d8e).is_err());
}

#[test]
fn streamed_payload_decodes_texture_beyond_the_base() {
    let edb = Edb::parse(fixture()).unwrap();
    assert_eq!(edb.header.base_bytes, 0x140);
    assert_eq!(edb.header.payload_bytes, 0x1a0);
    assert_eq!(edb.tables[15].count, 1);
    let image = edb.decode_texture(0x86000001, 0, 0).unwrap();
    assert_eq!((image.width, image.height), (1, 1));
    assert_eq!(image.pixels, [128; 4]);
    assert!(edb.decode_texture(0x86000001, 1, 0).is_err());
    assert!(edb.texture_info(0xdeadbeef).is_err());
}

#[test]
fn rejects_bad_edb_ranges_versions_and_truncations() {
    let original = fixture();
    for end in 0..original.len() {
        assert!(Edb::parse(original[..end].to_vec()).is_err());
    }
    for (offset, value) in [
        (0, 0u32),
        (8, 261),
        (20, u32::MAX),
        (24, 0),
        (0xb8, 0xffff0000),
        (0xbc, u32::MAX),
        (0xd8, u32::MAX),
    ] {
        let mut input = original.clone();
        input[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        assert!(Edb::parse(input).is_err(), "offset {offset}");
    }
}

#[test]
fn external_hash_is_preserved_and_never_read_as_an_offset() {
    let mut input = fixture();
    input[0x11c..0x120].copy_from_slice(&0x1000002u32.to_be_bytes());
    input[0x12c..0x130].copy_from_slice(&0x86000004u32.to_be_bytes());
    input[0x10d] = 20;
    let edb = Edb::parse(input).unwrap();
    let info = edb.texture_info(0x86000001).unwrap();
    assert_eq!(info.external_file, Some(0x1000002));
    assert_eq!(info.image_count, 20);
    assert_eq!(info.frames, [0x86000004]);
    assert!(edb.decode_texture(0x86000001, 0, 0).is_err());
}

#[test]
fn power_of_two_storage_preserves_logical_extent_and_row_stride() {
    let mut input = fixture();
    input[0xe0..0xe4].copy_from_slice(&0x30003u32.to_be_bytes());
    input[0x100..0x104].copy_from_slice(&0x30003u32.to_be_bytes());
    input[0x154..0x158].copy_from_slice(&0x40004u32.to_be_bytes());
    for (i, pixel) in input[0x180..].iter_mut().enumerate() {
        *pixel = i as u8;
    }
    let edb = Edb::parse(input).unwrap();
    let stored = edb
        .texture_tpl(0x86000001, 0)
        .unwrap()
        .decode(0, 0)
        .unwrap();
    assert_eq!((stored.width, stored.height), (4, 4));
    let logical = edb.decode_texture(0x86000001, 0, 0).unwrap();
    assert_eq!((logical.width, logical.height), (3, 3));
    assert_eq!(
        logical
            .pixels
            .iter()
            .step_by(4)
            .copied()
            .collect::<Vec<_>>(),
        [0, 1, 2, 8, 9, 10, 16, 17, 18]
    );
}

#[test]
fn accepts_observed_262_layout_and_large_streamed_payload_header() {
    let mut input = fixture();
    input[8..12].copy_from_slice(&262u32.to_be_bytes());
    assert!(Edb::parse(input.clone()).is_ok());
    input[20..24].copy_from_slice(&(96u32 * 1024 * 1024).to_be_bytes());
    assert_eq!(
        Edb::read_header(&input[..Edb::HEADER_BYTES])
            .unwrap()
            .payload_bytes,
        96 * 1024 * 1024
    );
    assert!(Edb::parse(input).is_err());
}
