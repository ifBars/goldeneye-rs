use super::*;

fn put(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn packed(delta: i32, low: u8) -> u32 {
    assert!((-0x80_0000..=0x7f_ffff).contains(&delta));
    (delta as u32).wrapping_shl(8) | u32::from(low)
}
fn body(data: &[u8], address: usize) -> LiveBody<'_> {
    LiveBody { data, address }
}
fn fixture(presence: u8) -> (Vec<u8>, usize, usize) {
    let mut data = vec![0; 0xa0];
    let table = 0x48;
    let entry = table + (presence & 7).count_ones() as usize * 4;
    let payload = 0x80;
    put(&mut data, 0x38, packed(table as i32 - 0x38, presence | 8));
    put(
        &mut data,
        entry,
        packed(payload as i32 - entry as i32, 0xe5),
    );
    for (i, value) in [0x0123_4567, 0x89ab_cdef, 0xfedc_ba98, 0x7654_3210]
        .into_iter()
        .enumerate()
    {
        put(&mut data, payload + i * 4, value);
    }
    (data, entry, payload)
}

#[test]
fn missing_kind_three_returns_zero_without_dereferencing_packed_offsets() {
    for raw in [0, 7, 0xffff_ff07, 0x7fff_ff80] {
        let mut data = vec![0; 0x3c];
        put(&mut data, 0x38, raw);
        assert_eq!(
            body(&data, 0).bone_mask().unwrap(),
            BoneMask {
                attribute_present: false,
                words: [0; 4],
            }
        );
    }
}

#[test]
fn presence_counts_only_earlier_kinds_and_skips_their_entries() {
    for presence in [0, 1, 2, 3, 4, 5, 6, 7, 0xf7] {
        let (mut data, entry, _) = fixture(presence);
        for at in (0x48..entry).step_by(4) {
            put(&mut data, at, u32::MAX);
        }
        assert_eq!(
            body(&data, 0).bone_mask().unwrap(),
            BoneMask {
                attribute_present: true,
                words: [0x0123_4567, 0x89ab_cdef, 0xfedc_ba98, 0x7654_3210],
            }
        );
    }
}

#[test]
fn both_packed_offsets_can_point_backwards_from_their_own_words() {
    let mut data = vec![0; 0xbc];
    let address = 0x80;
    let field = address + 0x38;
    let table = 0x60;
    let entry = table + 12;
    let payload = 0x10;
    put(&mut data, field, packed(table as i32 - field as i32, 0x0f));
    put(
        &mut data,
        entry,
        packed(payload as i32 - entry as i32, 0x7b),
    );
    for (i, value) in [1, 0x8000_0000, u32::MAX, 0].into_iter().enumerate() {
        put(&mut data, payload + i * 4, value);
    }
    assert_eq!(
        body(&data, address).bone_mask().unwrap().words,
        [1, 0x8000_0000, u32::MAX, 0]
    );
}

#[test]
fn required_zero_table_delta_is_invalid_instead_of_a_missing_attribute() {
    let mut data = vec![0; 0x50];
    put(&mut data, 0x38, 8);
    assert!(body(&data, 0).bone_mask().is_err());
}

#[test]
fn zero_payload_delta_addresses_the_entry_and_ignores_its_low_byte() {
    let (mut data, entry, _) = fixture(3);
    for low in [0, 1, 0x7f, 0xff] {
        put(&mut data, entry, packed(0, low));
        for (i, value) in [0x1020_3040, 0x5060_7080, 0x90a0_b0c0]
            .into_iter()
            .enumerate()
        {
            put(&mut data, entry + 4 + i * 4, value);
        }
        assert_eq!(
            body(&data, 0).bone_mask().unwrap().words,
            [u32::from(low), 0x1020_3040, 0x5060_7080, 0x90a0_b0c0]
        );
    }
}

#[test]
fn mask_reader_requires_only_reached_packed_words_and_four_payload_words() {
    let (mut data, _, payload) = fixture(1);
    // A full 0x98-byte animation header is unavailable. Its unrelated header,
    // channel and frame fields are not part of this consumer's requirements.
    data.truncate(payload + 16);
    assert_eq!(data.len(), 0x90);
    assert!(body(&data, 0).bone_mask().is_ok());
    for available in 0..16 {
        let truncated = &data[..payload + available];
        assert!(
            body(truncated, 0).bone_mask().is_err(),
            "accepted {available} payload bytes"
        );
    }
}

#[test]
fn malformed_or_truncated_reached_addresses_fail_without_a_fallback_mask() {
    let (data, entry, _) = fixture(7);
    assert!(body(&data[..0x3b], 0).bone_mask().is_err());
    assert!(body(&data[..entry + 3], 0).bone_mask().is_err());
    for delta in [-0x40, 0x7f_ffff] {
        let mut invalid = data.clone();
        put(&mut invalid, 0x38, packed(delta, 8));
        assert!(body(&invalid, 0).bone_mask().is_err());
    }
    for delta in [-0x80_0000, 0x7f_ffff] {
        let mut invalid = data.clone();
        put(&mut invalid, entry, packed(delta, 0));
        assert!(body(&invalid, 0).bone_mask().is_err());
    }
}

#[test]
fn address_arithmetic_is_checked_before_reading() {
    assert!(body(&[], usize::MAX).bone_mask().is_err());
    assert!(relative(0, -1).is_err());
    assert!(relative(usize::MAX, 1).is_err());
}
