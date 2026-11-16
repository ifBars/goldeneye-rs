use super::*;
use crate::supplied_data::{Identity, SuppliedData};

const BASE: u32 = 0x1000;

fn identity() -> Identity {
    Identity {
        game_id: "TEST00".into(),
        revision: 0,
        main_dol_partition_offset: 0,
        dol_bytes: 0,
        dol_sha256: "a".repeat(64),
        data_section_index: 0,
        data_virtual_address: BASE,
        data_bytes: 0,
        data_sha256: "b".repeat(64),
    }
}
fn supplied(address: u32, words: &[u32]) -> SuppliedData {
    let mut bytes = Vec::new();
    for word in words {
        bytes.extend_from_slice(&word.to_be_bytes());
    }
    SuppliedData::initialize(
        Data {
            address,
            bytes: bytes.into_boxed_slice(),
        },
        identity(),
    )
}

#[test]
fn six_words_are_read_individually_at_the_table_location() {
    // Words before and after the table are poison and never part of it.
    let words = [
        0xffff_ffff,
        0x7100_0001,
        0x7100_0002,
        0,
        0x7100_0004,
        0x7100_0005,
        0x7100_0006,
        0xeeee_eeee,
    ];
    let table = supplied(BASE, &words).preload_bone_tags_at(BASE + 4);
    assert_eq!(table.count(), BONE_TAG_COUNT);
    let read: Vec<u32> = (0..6).map(|i| table.hash(i).unwrap()).collect();
    assert_eq!(&read[..], &words[1..7]);
    // A zero word is returned raw; deciding what it means is the caller's job.
    assert_eq!(table.hash(2).unwrap(), 0);
    // The seventh word exists in DATA but is not part of the table.
    assert!(table.hash(6).is_err());
    assert!(table.hash(u8::MAX).is_err());
}

#[test]
fn reads_are_lazy_a_window_holding_only_the_first_word_serves_index_zero() {
    let table = supplied(BASE, &[0x7100_0001]).preload_bone_tags_at(BASE);
    assert_eq!(table.hash(0).unwrap(), 0x7100_0001);
    assert!(table.hash(1).is_err());
    // Words before the window are also outside it.
    let before = supplied(BASE + 4, &[1, 2]).preload_bone_tags_at(BASE);
    assert!(before.hash(0).is_err());
}

#[test]
fn unaligned_and_overflowing_locations_are_errors_not_zeroes() {
    let data = supplied(BASE, &[1, 2, 3, 4, 5, 6, 7]);
    assert!(data.preload_bone_tags_at(BASE + 2).hash(0).is_err());
    assert!(data.preload_bone_tags_at(u32::MAX - 3).hash(1).is_err());
}

#[test]
fn the_handle_is_owned_and_outlives_its_supplied_data_binding() {
    let table = {
        let data = supplied(BASE, &[9, 8, 7, 6, 5, 4]);
        data.preload_bone_tags_at(BASE)
    };
    let again = table.clone();
    assert_eq!(table.hash(5).unwrap(), 4);
    assert_eq!(again.hash(0).unwrap(), 9);
}

#[test]
fn the_actual_location_is_the_supported_image_table() {
    // A window ending just before the real location serves nothing; one that
    // starts exactly there serves index zero. The address is not repeated here.
    let at = BONE_TAG_TABLE_ADDRESS;
    let short = supplied(at - 4, &[1]).preload_bone_tags();
    assert!(short.hash(0).is_err());
    let exact = supplied(at, &[0x7100_0abc]).preload_bone_tags();
    assert_eq!(exact.hash(0).unwrap(), 0x7100_0abc);
}
