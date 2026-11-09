use super::*;

fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn half(data: &mut [u8], at: usize, value: u16) {
    data[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn fixture() -> Vec<u8> {
    let mut data = vec![0; 0x340];
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, 0x010a_00c1),
        (8, 263),
        (0x14, 0x340),
        (0x18, 0x340),
        (0x60, 0x0001_0001),
        (0x64, 0x100 - 0x64),
        (0x100, 0x0d00_00c1),
        (0x104, 0x1234_5678),
        (0x108, 0x180),
        (0x10c, u32::MAX),
        (0x110, 0x7fc0_0001),
        (0x120, 0xdead_beef),
        (0x180, 0x200),
        (0x184, 3),
        (0x1c8, 0x240 - 0x1c8),
        (0x1dc, 0x300 - 0x1dc),
    ] {
        word(&mut data, at, value);
    }
    for (index, values) in [[u16::MAX, 2, 0xabcd, 0x1234], [0, 2, 7, 8], [1, 2, 9, 10]]
        .iter()
        .enumerate()
    {
        for (lane, value) in values.iter().enumerate() {
            half(&mut data, 0x240 + index * 8 + lane * 2, *value);
        }
    }
    for (index, (bone, tag)) in [(2, 0x42), (1, 0x42), (u16::MAX, u16::MAX)]
        .iter()
        .enumerate()
    {
        half(&mut data, 0x300 + index * 4, *bone);
        half(&mut data, 0x302 + index * 4, *tag);
    }
    data
}
fn budget() -> Budget {
    Budget { skins: 1, bones: 3 }
}

#[test]
fn translation_defaults_are_lazy_preserve_raw_vectors_and_use_signed_relative_fields() {
    let mut data = fixture();
    word(&mut data, 0x1c4, (0x140i32 - 0x1c4) as u32);
    let expected = [
        [0x7fc0_0001, 0x8000_0000, 0x3f80_0000, u32::MAX],
        [1, 2, 3, 4],
        [5, 6, 7, 8],
    ];
    for (index, vector) in expected.iter().enumerate() {
        for (lane, bits) in vector.iter().enumerate() {
            word(&mut data, 0x140 + index * 16 + lane * 4, *bits);
        }
    }
    let edb = Edb::parse(data).unwrap();
    let skin = edb.skin(0, budget()).unwrap().unwrap();
    let translations = skin.fallback_translations(&edb, 3).unwrap();
    assert_eq!(translations.address, Some(0x140));
    assert_eq!(translations.vectors, expected);
    assert!(skin.fallback_translations(&edb, 2).is_err());
}

#[test]
fn missing_or_truncated_translation_defaults_error_but_empty_skin_skips_pointer() {
    for pointer in [0, (0x330i32 - 0x1c4) as u32] {
        let mut data = fixture();
        word(&mut data, 0x1c4, pointer);
        let edb = Edb::parse(data).unwrap();
        let skin = edb.skin(0, budget()).unwrap().unwrap();
        assert!(skin.fallback_translations(&edb, 3).is_err());
    }
    let mut data = fixture();
    word(&mut data, 0x184, 0);
    word(&mut data, 0x1c4, u32::MAX);
    let edb = Edb::parse(data).unwrap();
    let skin = edb.skin(0, budget()).unwrap().unwrap();
    assert_eq!(
        skin.fallback_translations(&edb, 0).unwrap(),
        TranslationDefaults {
            address: None,
            vectors: Vec::new()
        }
    );
}

#[test]
fn auxiliary_count_is_lazy_raw_and_checked_against_the_retained_skin() {
    let mut data = fixture();
    word(&mut data, 0x200, 7);
    let edb = Edb::parse(data).unwrap();
    let skin = edb.skin(0, budget()).unwrap().unwrap();
    assert_eq!(skin.auxiliary_count(&edb).unwrap(), 7);
    let mut data = fixture();
    word(&mut data, 4, 0x010a_00c2);
    assert!(skin.auxiliary_count(&Edb::parse(data).unwrap()).is_err());
    let mut data = fixture();
    word(&mut data, 0x184, 0); // No topology needed for this empty skin.
    data.truncate(0x200); // Body+0x80 is absent; basic read must stay lazy.
    word(&mut data, 0x14, 0x200);
    word(&mut data, 0x18, 0x200);
    let edb = Edb::parse(data).unwrap();
    let skin = edb.skin(0, budget()).unwrap().unwrap();
    assert!(skin.auxiliary_count(&edb).is_err());
}

#[test]
fn coded_and_literal_resolution_preserve_disk_descriptor_and_topology() {
    let edb = Edb::parse(fixture()).unwrap();
    for reference in [0, 0x8d00_0000, 0x0d00_00c1] {
        let skin = edb.skin(reference, budget()).unwrap().unwrap();
        assert_eq!(skin.descriptor().directory_index, 0);
        assert_eq!(skin.descriptor().address, 0x180);
        assert_eq!(skin.descriptor().words_raw[3], u32::MAX);
        assert_eq!(skin.descriptor().words_raw[4], 0x7fc0_0001);
        assert_eq!(skin.bone_count(), 3);
        assert_eq!(
            skin.bones()[0],
            Bone {
                parent_raw: u16::MAX,
                range_raw: 2,
                word_4_raw: 0xabcd,
                word_6_raw: 0x1234,
                source_address: 0x240
            }
        );
        assert_eq!(skin.bones()[2].parent_raw, 1);
    }
    assert!(edb.skin(1, budget()).unwrap().is_none());
    assert!(edb.skin(0x0d00_ffff, budget()).unwrap().is_none());
}

#[test]
fn tag_match_uses_low_half_and_returns_first_before_later_bad_records() {
    let mut data = fixture();
    // Later bone is corrupt; it is never reached after the first matching tag.
    half(&mut data, 0x304, u16::MAX);
    let edb = Edb::parse(data).unwrap();
    let skin = edb.skin(0, budget()).unwrap().unwrap();
    let tag = skin.find_tag(&edb, 0x0e00_0042, 1).unwrap().unwrap();
    assert_eq!(
        tag,
        Tag {
            bone_index: 2,
            tag_raw: 0x42,
            source_index: 0,
            source_address: 0x300
        }
    );
}

#[test]
fn sentinel_precedes_bone_read_and_never_matches_requested_ffff() {
    let mut data = fixture();
    half(&mut data, 0x300, u16::MAX);
    half(&mut data, 0x302, u16::MAX);
    let edb = Edb::parse(data).unwrap();
    let skin = edb.skin(0, budget()).unwrap().unwrap();
    assert!(skin.find_tag(&edb, 0x0e00_ffff, 1).unwrap().is_none());
    assert!(skin.find_tag(&edb, 0x0e00_0042, 1).unwrap().is_none());
}

#[test]
fn matching_bone_bounds_budget_and_truncation_are_explicit_errors() {
    let edb = Edb::parse(fixture()).unwrap();
    let skin = edb.skin(0, budget()).unwrap().unwrap();
    assert!(
        skin.find_tag(&edb, 0x43, 2)
            .unwrap_err()
            .to_string()
            .contains("budget")
    );
    assert!(skin.find_tag(&edb, 0x42, 0).is_err());
    assert!(skin.find_tag(&edb, 0x43, 3).unwrap().is_none());
    let mut data = fixture();
    half(&mut data, 0x300, 3);
    let edb = Edb::parse(data).unwrap();
    assert!(
        edb.skin(0, budget())
            .unwrap()
            .unwrap()
            .find_tag(&edb, 0x42, 1)
            .is_err()
    );
    let mut data = fixture();
    word(&mut data, 0x1dc, 0x33e - 0x1dc);
    let edb = Edb::parse(data).unwrap();
    assert!(
        edb.skin(0, budget())
            .unwrap()
            .unwrap()
            .find_tag(&edb, 0x42, 1)
            .is_err()
    );
}

#[test]
fn skin_rejects_reached_wrong_body_missing_hierarchy_and_exhausted_limits() {
    for (at, value) in [
        (0x180, 0x300),
        (0x1c8, 0),
        (0x1c8, 0x7fff_ffff),
        (0x1c8, 0x338 - 0x1c8),
    ] {
        let mut data = fixture();
        word(&mut data, at, value);
        assert!(Edb::parse(data).unwrap().skin(0, budget()).is_err());
    }
    let edb = Edb::parse(fixture()).unwrap();
    assert!(edb.skin(0, Budget { skins: 0, bones: 3 }).is_err());
    assert!(edb.skin(0, Budget { skins: 1, bones: 2 }).is_err());
}

#[test]
fn empty_topology_and_unread_tags_stay_lazy_signed_offsets_are_supported() {
    let mut data = fixture();
    word(&mut data, 0x184, 0);
    word(&mut data, 0x1c8, u32::MAX);
    word(&mut data, 0x1dc, 0);
    let edb = Edb::parse(data).unwrap();
    let skin = edb.skin(0, Budget { skins: 1, bones: 0 }).unwrap().unwrap();
    assert!(skin.bones().is_empty());
    assert!(skin.find_tag(&edb, 0x42, 1).is_err());
    let mut data = fixture();
    word(&mut data, 0x1dc, (0x160i32 - 0x1dc) as u32);
    half(&mut data, 0x160, 1);
    half(&mut data, 0x162, 0x42);
    let edb = Edb::parse(data).unwrap();
    assert_eq!(
        edb.skin(0, budget())
            .unwrap()
            .unwrap()
            .find_tag(&edb, 0x42, 1)
            .unwrap()
            .unwrap()
            .bone_index,
        1
    );
}

#[test]
fn tag_lookup_rejects_a_different_owner_descriptor() {
    let edb = Edb::parse(fixture()).unwrap();
    let skin = edb.skin(0, budget()).unwrap().unwrap();
    let mut data = fixture();
    word(&mut data, 0x110, 0);
    assert!(skin.find_tag(&Edb::parse(data).unwrap(), 0x42, 1).is_err());
}
