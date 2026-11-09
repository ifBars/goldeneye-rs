use super::*;
use crate::edb::Edb;
#[test]
fn spatial_headers_preserve_flags_and_unread_w_bits_without_following_components() {
    let mut bytes = crate::fixtures::graph_fixture();
    let entity_address = Edb::parse(bytes.clone()).unwrap().entities().unwrap()[0].address as usize;
    bytes[entity_address + 4..entity_address + 8].copy_from_slice(&0x12345678u32.to_be_bytes());
    bytes[entity_address + 8..entity_address + 12].copy_from_slice(&(-321i32).to_be_bytes());
    for (side, values) in [[-1f32, -2., -3.], [4., 5., 6.]].into_iter().enumerate() {
        for (axis, value) in values.into_iter().enumerate() {
            let at = entity_address + 0x10 + side * 16 + axis * 4;
            bytes[at..at + 4].copy_from_slice(&value.to_bits().to_be_bytes());
        }
    }
    bytes[entity_address + 0x1c..entity_address + 0x20]
        .copy_from_slice(&f32::NAN.to_bits().to_be_bytes());
    bytes[entity_address + 0x2c..entity_address + 0x30]
        .copy_from_slice(&f32::INFINITY.to_bits().to_be_bytes());
    bytes[entity_address + 0x40..entity_address + 0x44]
        .copy_from_slice(&0xffffff7fu32.to_be_bytes());
    let header = Edb::parse(bytes.clone())
        .unwrap()
        .entity_spatial_at(0)
        .unwrap();
    assert_eq!(header.flags_raw, 0x12345678);
    assert_eq!(header.sort_field_raw, -321);
    assert_eq!(header.bounds, [[-1., -2., -3.], [4., 5., 6.]]);
    assert_eq!(
        header.packed_w_raw,
        [f32::NAN.to_bits(), f32::INFINITY.to_bits()]
    );
    assert_eq!(header.components_raw, 0xffffff7f);
    bytes[entity_address + 0x10..entity_address + 0x14]
        .copy_from_slice(&f32::NAN.to_bits().to_be_bytes());
    assert!(Edb::parse(bytes).unwrap().entity_spatial_at(0).is_err());
}
#[test]
fn spatial_headers_reject_unknown_types_bad_indices_and_short_headers() {
    let bytes = crate::fixtures::graph_fixture();
    let edb = Edb::parse(bytes.clone()).unwrap();
    assert!(edb.entity_spatial_at(1000).is_err());
    let entity = edb.entities().unwrap().remove(0);
    let mut unknown = bytes.clone();
    unknown[entity.address as usize..entity.address as usize + 4]
        .copy_from_slice(&0x999u32.to_be_bytes());
    assert!(read(&unknown, entity).is_err());
    let entity = edb.entities().unwrap().remove(0);
    assert!(read(&bytes[..entity.address as usize + 0x43], entity).is_err());
}

#[test]
fn packed_component_lookup_uses_signed_relative24_and_popcount_rank() {
    let mut bytes = crate::fixtures::graph_fixture();
    bytes.resize(0x500, 0);
    bytes[20..24].copy_from_slice(&0x500u32.to_be_bytes());
    bytes[24..28].copy_from_slice(&0x500u32.to_be_bytes());
    let address = Edb::parse(bytes.clone()).unwrap().entities().unwrap()[0].address as usize;
    let put = |bytes: &mut [u8], at: usize, target: usize, flags: u8| {
        let value = ((target as i32 - at as i32) << 8) as u32 | u32::from(flags);
        bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
    };
    put(&mut bytes, address + 0x40, 0x300, 0b0101_0101);
    for (rank, target) in [0x180, 0x340, 0x180, 0x380].into_iter().enumerate() {
        put(&mut bytes, 0x300 + rank * 4, target, 0xa0 + rank as u8);
        bytes[target..target + 4].copy_from_slice(&0x601u32.to_be_bytes());
    }
    let edb = Edb::parse(bytes.clone()).unwrap();
    for (slot, address, flags) in [
        (0, 0x180, 0xa0),
        (2, 0x340, 0xa1),
        (4, 0x180, 0xa2),
        (6, 0x380, 0xa3),
    ] {
        let component = edb.entity_component_at(0, slot).unwrap().unwrap();
        assert_eq!(
            (component.address, component.descriptor_low_byte_raw),
            (address, flags)
        );
        assert_eq!(
            edb.component_spatial_at(0, slot)
                .unwrap()
                .unwrap()
                .entity_type,
            0x601
        );
    }
    assert!(edb.entity_component_at(0, 1).unwrap().is_none());
    assert!(edb.entity_component_at(0, 8).is_err());
    // The header can contain an unusable table offset when the requested bit is clear.
    bytes[address + 0x40..address + 0x44].copy_from_slice(&0x7fffff00u32.to_be_bytes());
    assert!(
        Edb::parse(bytes.clone())
            .unwrap()
            .entity_component_at(0, 6)
            .unwrap()
            .is_none()
    );
    bytes[address + 0x40..address + 0x44].copy_from_slice(&0x7fffff01u32.to_be_bytes());
    assert!(
        Edb::parse(bytes)
            .unwrap()
            .entity_component_at(0, 0)
            .is_err()
    );
}
