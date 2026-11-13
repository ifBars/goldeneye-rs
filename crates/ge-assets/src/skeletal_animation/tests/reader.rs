use super::*;

fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn half(data: &mut [u8], at: usize, value: u16) {
    data[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
pub(super) fn fixture() -> Vec<u8> {
    let mut data = vec![0; 0x300];
    for (at, value) in [
        (0, 0x4745_4f4d),
        (4, 0x0102_00c1),
        (8, 263),
        (0x14, 0x300),
        (0x18, 0x300),
        (0x58, 0x0001_0001),
        (0x5c, 0xe0 - 0x5c),
        (0xe0, 0x0300_00c1),
        (0xe4, 0x0000_1234),
        (0xe8, 0x180),
        (0xec, u32::MAX),
        (0xf0, 0x280),
        (0xf4, u32::MAX),
        (0xf8, 4),
        (0xfc, 0xdead_beef),
        (0x180, 0x102),
        (0x184, 0x0300_00c1),
        (0x188, 0x7fc0_0001),
        (0x18c, 0x1e12_0007),
        (0x194, 0x0402_0001),
        (0x1a0, u32::MAX),
        (0x1a4, 0x0d00_00c1),
        (0x1e0, 9),
        (0x280, 0x1234_abcd),
    ] {
        word(&mut data, at, value);
    }
    data
}
fn budget() -> Budget {
    Budget {
        directory_entries: 1,
        rotation_channels: 4,
        translation_channels: 2,
        auxiliary_channels: 1,
        payload_bytes: 4,
    }
}

#[test]
fn root_bias_and_consumer_flags_are_raw_lazy_fields_of_the_same_checked_owner() {
    let mut data = fixture();
    word(&mut data, 0x1b8, 0x1234_5604);
    for (offset, bits) in [
        (0x20c, 0x8000_0000),
        (0x210, 0x7fc0_0123),
        (0x214, 0x7f80_0000),
    ] {
        word(&mut data, offset, bits);
    }
    let edb = Edb::parse(data.clone()).unwrap();
    let animation = edb.skeletal_animation(0, budget()).unwrap().unwrap();
    assert_eq!(
        animation.root_bias_bits(&edb).unwrap(),
        [0x8000_0000, 0x7fc0_0123, 0x7f80_0000]
    );
    assert_eq!(animation.consumer_flags_bits(&edb).unwrap(), 0x1234_5604);
    word(&mut data, 4, 0x0102_ffff);
    let other = Edb::parse(data).unwrap();
    assert!(animation.root_bias_bits(&other).is_err());
    assert!(animation.consumer_flags_bits(&other).is_err());
}

#[test]
fn native_references_preserve_header_masks_cache_word_and_separate_payload() {
    let edb = Edb::parse(fixture()).unwrap();
    for reference in [0, 0x8300_0000, 0x0300_00c1] {
        let animation = edb
            .skeletal_animation(reference, budget())
            .unwrap()
            .unwrap();
        assert_eq!(animation.descriptor().reference, reference);
        assert_eq!(animation.descriptor().directory_index, 0);
        assert_eq!(animation.descriptor().hash, 0x0300_00c1);
        assert_eq!(animation.descriptor().words_raw[7], 0xdead_beef);
        assert_eq!(animation.header().stored_key_raw, 0x0300_00c1);
        assert_eq!(animation.header().word_08_raw, 0x7fc0_0001);
        assert_eq!(
            (
                animation.header().frames_per_second,
                animation.header().cache_chunk_frames,
                animation.header().frame_count
            ),
            (30, 18, 7)
        );
        assert_eq!(animation.header().model_pair_raw, [u32::MAX, 0x0d00_00c1]);
        assert_eq!(animation.translation_bones(), [0, 3]);
        assert_eq!(
            animation.payload_span(),
            PayloadSpan {
                address: 0x280,
                bytes: 4
            }
        );
        assert_eq!(
            animation.compressed_payload(&edb).unwrap(),
            [0x12, 0x34, 0xab, 0xcd]
        );
    }
    assert!(edb.skeletal_animation(1, budget()).unwrap().is_none());
    assert!(
        edb.skeletal_animation(0x0300_ffff, budget())
            .unwrap()
            .is_none()
    );
}

#[test]
fn coded_lookup_leaves_unselected_sections_bodies_and_payloads_unread() {
    let mut data = fixture();
    let selected = data[0xe0..0x100].to_vec();
    data[0x100..0x120].copy_from_slice(&selected);
    word(&mut data, 0x58, 0x0002_0000);
    word(&mut data, 0xe4, 0x1234_0000);
    word(&mut data, 0xe8, u32::MAX);
    word(&mut data, 0xf0, u32::MAX);
    let edb = Edb::parse(data).unwrap();
    let mut limits = budget();
    limits.directory_entries = 2;
    assert_eq!(
        edb.skeletal_animation(0x8300_0001, limits)
            .unwrap()
            .unwrap()
            .descriptor()
            .directory_index,
        1
    );
    assert!(edb.skeletal_animation(2, limits).unwrap().is_none());
    assert!(edb.skeletal_animation(0, limits).is_err());
}

#[test]
fn each_host_budget_is_checked_and_frames_are_opaque_metadata() {
    let edb = Edb::parse(fixture()).unwrap();
    for limits in [
        Budget {
            directory_entries: 0,
            ..budget()
        },
        Budget {
            rotation_channels: 3,
            ..budget()
        },
        Budget {
            translation_channels: 1,
            ..budget()
        },
        Budget {
            auxiliary_channels: 0,
            ..budget()
        },
        Budget {
            payload_bytes: 3,
            ..budget()
        },
    ] {
        assert!(
            edb.skeletal_animation(0, limits)
                .unwrap_err()
                .to_string()
                .contains("budget")
        );
    }
    let mut data = fixture();
    word(&mut data, 0x18c, 0); // Not a supported decoder clock; metadata stays raw.
    let edb = Edb::parse(data).unwrap();
    assert_eq!(
        edb.skeletal_animation(0, budget())
            .unwrap()
            .unwrap()
            .header()
            .frame_count,
        0
    );
}

#[test]
fn reached_body_header_and_compressed_span_errors_do_not_become_misses() {
    for (at, value) in [
        (0x180, 0x300),
        (0xe8, 0x2f0),
        (0xf0, 0x2ff),
        (0xf8, u32::MAX),
    ] {
        let mut data = fixture();
        word(&mut data, at, value);
        assert!(
            Edb::parse(data)
                .unwrap()
                .skeletal_animation(0, budget())
                .is_err()
        );
    }
    let mut data = fixture();
    half(&mut data, 0xe4, 0x8001);
    assert!(
        Edb::parse(data)
            .unwrap()
            .skeletal_animation(0, budget())
            .unwrap_err()
            .to_string()
            .contains("section")
    );
    // The runtime-initialized high bit does not itself change section zero.
    let mut data = fixture();
    half(&mut data, 0xe4, 0x8000);
    assert!(
        Edb::parse(data)
            .unwrap()
            .skeletal_animation(0, budget())
            .unwrap()
            .is_some()
    );
}

#[test]
fn sparse_mask_count_indices_and_supported_width_are_validated() {
    for (at, value) in [(0x1e0, 1), (0x1e0, 0x11), (0x194, 0x8102_0001)] {
        let mut data = fixture();
        word(&mut data, at, value);
        let limits = Budget {
            rotation_channels: 255,
            ..budget()
        };
        assert!(
            Edb::parse(data)
                .unwrap()
                .skeletal_animation(0, limits)
                .is_err()
        );
    }
}

#[test]
fn payload_borrow_checks_retained_resource_and_descriptor_identity() {
    let edb = Edb::parse(fixture()).unwrap();
    let animation = edb.skeletal_animation(0, budget()).unwrap().unwrap();
    let mut data = fixture();
    word(&mut data, 4, 0x0102_00c2);
    assert!(
        animation
            .compressed_payload(&Edb::parse(data).unwrap())
            .is_err()
    );
    let mut data = fixture();
    word(&mut data, 0xfc, 0);
    assert!(
        animation
            .compressed_payload(&Edb::parse(data).unwrap())
            .is_err()
    );
}
