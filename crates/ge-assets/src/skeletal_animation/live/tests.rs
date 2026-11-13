use super::*;
fn fixture() -> Edb {
    Edb::parse(super::super::tests::fixture()).unwrap()
}
#[test]
fn metadata_cache_lookup_does_not_touch_body_or_later_descriptor_fields() {
    let mut e = fixture();
    e.tables[3].offset = 0x2fc;
    let binding = e.base_animation_binding(0).unwrap().unwrap();
    assert_eq!(binding.directory_index, 0);
    assert!(e.base_animation_body(0).is_err());
    assert!(e.base_animation_binding(0x8300_0001).unwrap().is_none());
}
#[test]
fn live_body_fields_are_lazy_and_do_not_preflight_compressed_frames_or_other_channels() {
    let mut data = super::super::tests::fixture();
    data[0xe8..0xec].copy_from_slice(&0x2f0u32.to_be_bytes());
    data[0x2f0..0x2f4].copy_from_slice(&0x102u32.to_be_bytes());
    data[0x2fc] = 47;
    data[0x2fe..0x300].copy_from_slice(&123u16.to_be_bytes());
    let e = Edb::parse(data).unwrap();
    let BodyState::Resident(body) = e.base_animation_body(0).unwrap() else {
        panic!("body absent")
    };
    assert_eq!(body.frames_per_second().unwrap(), 47);
    assert_eq!(body.frame_count().unwrap(), 123);
    assert!(body.consumer_flags().is_err());
}
#[test]
fn full_cached_payload_does_not_publish_nonbase_animation_or_hide_true_absence() {
    let mut data = super::super::tests::fixture();
    data[0xe4..0xe6].copy_from_slice(&1u16.to_be_bytes());
    let e = Edb::parse(data.clone()).unwrap();
    assert!(e.base_animation_binding(0).unwrap().is_some());
    assert!(matches!(
        e.base_animation_body(0).unwrap(),
        BodyState::NotResident
    ));
    data[0xe4..0xe6].fill(0);
    data[0xe8..0xec].fill(0);
    assert!(matches!(
        Edb::parse(data).unwrap().base_animation_body(0).unwrap(),
        BodyState::Absent
    ));
    let e = fixture();
    let BodyState::Resident(body) = e.base_animation_body(0).unwrap() else {
        panic!("body absent")
    };
    assert_eq!(body.consumer_flags().unwrap(), 0);
    assert!(e.base_animation_body(1).is_err());
}
