use super::*;

fn data(bytes: Vec<u8>) -> Data {
    Data {
        address: DESCRIPTOR_BASE,
        bytes: bytes.into_boxed_slice(),
    }
}

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}

#[test]
fn known_rows_read_supplied_fields_and_preserve_signed_enable_without_embedded_values() {
    let mut bytes = vec![0; 89 * 16 + 13];
    for (index, class, subtype, enable) in [
        (0, 0x1e00_00a1, 0x1f00_00a2, 1),
        (4, 0x1e00_00a7, 0x1f00_00a8, 1),
        (3, 0x1e00_00a9, 0x1f00_00aa, 0),
        (8, 0x1e00_00ab, 0x1f00_00ac, 1),
        (47, 0x1e00_00ad, 0x1f00_00ae, 1),
        (9, 0x1e00_00b1, 0x1f00_00b2, 1),
        (38, 0x1e00_00b3, 0x1f00_00b4, 1),
        (17, 0x1e00_00b5, 0x1f00_00b6, 1),
        (63, 0x1e00_00b7, 0x1f00_00b8, 1),
        (6, 0x1e00_00b9, 0x1f00_00ba, 1),
        (73, 0x1e00_00bb, 0x1f00_00bc, 1),
        (2, 0x1e00_00bd, 0x1f00_00c0, 1),
        (40, 0x1e00_00c1, 0x1f00_00c2, 1),
        (12, 0x1e00_00c3, 0x1f00_00c4, 1),
        (29, 0x1e00_00c5, 0x1f00_00c6, 1),
        (35, 0x1e00_00c7, 0x1f00_00c8, 1),
        (39, 0x1e00_00c9, 0x1f00_00ca, 1),
        (62, 0x1e00_00cb, 0x1f00_00cc, 1),
        (72, 0x1e00_00cd, 0x1f00_00ce, 1),
        (28, 0x1e00_00a3, 0x1f00_00a4, 0xff),
        (89, 0x1e00_00a5, 0x1f00_00a6, 0x80),
    ] {
        put(&mut bytes, index * 16, class);
        put(&mut bytes, index * 16 + 4, subtype);
        bytes[index * 16 + 12] = enable;
    }
    let owner = data(bytes);
    let view = View::from_data(&owner);
    let first = view.descriptor(0).unwrap();
    assert_eq!(
        (first.class, first.subtype, first.enable_byte_raw),
        (0x1e00_00a1, 0x1f00_00a2, 1)
    );
    let resource = view.descriptor(4).unwrap();
    assert_eq!(
        (resource.class, resource.subtype, resource.source_address),
        (0x1e00_00a7, 0x1f00_00a8, DESCRIPTOR_BASE + 4 * 16)
    );
    for (index, class, subtype) in [
        (3, 0x1e00_00a9, 0x1f00_00aa),
        (8, 0x1e00_00ab, 0x1f00_00ac),
        (47, 0x1e00_00ad, 0x1f00_00ae),
        (9, 0x1e00_00b1, 0x1f00_00b2),
        (38, 0x1e00_00b3, 0x1f00_00b4),
        (17, 0x1e00_00b5, 0x1f00_00b6),
        (63, 0x1e00_00b7, 0x1f00_00b8),
        (6, 0x1e00_00b9, 0x1f00_00ba),
        (73, 0x1e00_00bb, 0x1f00_00bc),
        (2, 0x1e00_00bd, 0x1f00_00c0),
        (40, 0x1e00_00c1, 0x1f00_00c2),
        (12, 0x1e00_00c3, 0x1f00_00c4),
        (29, 0x1e00_00c5, 0x1f00_00c6),
        (35, 0x1e00_00c7, 0x1f00_00c8),
        (39, 0x1e00_00c9, 0x1f00_00ca),
        (62, 0x1e00_00cb, 0x1f00_00cc),
        (72, 0x1e00_00cd, 0x1f00_00ce),
    ] {
        let row = view.descriptor(index).unwrap();
        assert_eq!(
            (row.class, row.subtype, row.source_address),
            (class, subtype, DESCRIPTOR_BASE + index * 16)
        );
    }
    let last = view.descriptor(28).unwrap();
    assert_eq!(last.native_type_index, 28);
    assert_eq!(last.source_address, DESCRIPTOR_BASE + 28 * 16);
    assert_eq!(
        (last.class, last.subtype, last.enable_byte_raw),
        (0x1e00_00a3, 0x1f00_00a4, -1)
    );
    let be = view.descriptor(89).unwrap();
    assert_eq!(be.source_address, DESCRIPTOR_BASE + 89 * 16);
    assert_eq!(
        (be.class, be.subtype, be.enable_byte_raw),
        (0x1e00_00a5, 0x1f00_00a6, -128)
    );
}

#[test]
fn unused_descriptor_words_are_not_demanded_but_reached_enable_is_checked() {
    let mut bytes = vec![0; 13];
    put(&mut bytes, 0, 0x1e00_00a5);
    put(&mut bytes, 4, 0x1f00_00a6);
    bytes[8..12].copy_from_slice(&u32::MAX.to_be_bytes()); // Uninterpreted native field.
    let owner = data(bytes.clone());
    assert_eq!(
        View::from_data(&owner)
            .descriptor(0)
            .unwrap()
            .enable_byte_raw,
        0
    );
    bytes.truncate(12);
    assert!(View::from_data(&data(bytes)).descriptor(0).is_err());
}

#[test]
fn unknown_index_is_unsupported_even_when_no_data_could_be_read() {
    let owner = data(Vec::new());
    let view = View::from_data(&owner);
    for index in [1, 27, 30, 61, 88, 90, u32::MAX] {
        assert!(!supports(index));
        assert!(
            view.descriptor(index)
                .unwrap_err()
                .to_string()
                .contains("unsupported native trigger classification")
        );
    }
    assert!(supports(0) && supports(4) && supports(28) && supports(89));
    assert!(view.descriptor(0).is_err());
    assert!(view.descriptor(89).is_err());
}
