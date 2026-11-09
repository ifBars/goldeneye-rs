use super::*;
fn word(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_be_bytes());
}
fn half(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_be_bytes());
}
fn fixture() -> (Edb, Animation) {
    let mut b = vec![0; 0x400];
    for (at, v) in [
        (0, 0x47454f4d),
        (4, 0x01020016),
        (8, 263),
        (0x14, 0x400),
        (0x18, 0x400),
        (0x58, 0x00010001),
        (0x5c, 0xd0 - 0x5c),
        (0xd0, 0x030000a5),
        (0xd8, 0x180),
        (0xe0, 0x280),
        (0xe8, 64),
        (0x180, 0x102),
        (0x18c, 0x1e020002),
    ] {
        word(&mut b, at, v);
    }
    b[0x194] = 2;
    half(&mut b, 0x280, 10);
    for scalar in 0..6 {
        let at = 0x282 + 6 * scalar;
        half(&mut b, at, 1);
        half(&mut b, at + 2, 3);
        half(&mut b, at + 4, 0x41);
    }
    let owner = Edb::parse(b).unwrap();
    let animation = owner
        .skeletal_animation(
            0,
            ge_assets::skeletal_animation::Budget {
                directory_entries: 1,
                rotation_channels: 2,
                translation_channels: 0,
                auxiliary_channels: 0,
                payload_bytes: 64,
            },
        )
        .unwrap()
        .unwrap();
    (owner, animation)
}
#[test]
fn raw_cache_reuses_valid_channels_and_merges_new_bones_without_duplicates() {
    let (owner, animation) = fixture();
    let mut cache = RawFrames::new();
    let (a, stats) = cache.get(&owner, &animation, 0, [1, 0, 0, 0]).unwrap();
    assert_eq!(a.rotations.len(), 1);
    assert_eq!(stats, (1, 1, 0));
    let (b, stats) = cache.get(&owner, &animation, 0, [2, 0, 0, 0]).unwrap();
    assert_eq!(
        b.rotations.iter().map(|r| r.bone_index).collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(b.valid_mask, [3, 0, 0, 0]);
    assert_eq!(stats, (1, 2, 0));
    let (c, stats) = cache.get(&owner, &animation, 0, [3, 0, 0, 0]).unwrap();
    assert_eq!(c.rotations.len(), 2);
    assert_eq!(stats, (1, 2, 1));
    assert!(c.has_unverified_sqrt());
}
#[test]
fn invalid_frame_does_not_publish_cache_data_or_promote_numeric_or_pose_readiness() {
    let (owner, animation) = fixture();
    let mut cache = RawFrames::new();
    assert!(cache.get(&owner, &animation, 2, [3, 0, 0, 0]).is_err());
    assert_eq!(cache.len(), 0);
    let (frame, stats) = cache.get(&owner, &animation, 0, [3, 0, 0, 0]).unwrap();
    let evidence = Evidence::new(0x840000a5, 0, 0, &animation, frame, stats, 1);
    assert_eq!(evidence.unverified_sqrt_rotations, 2);
    assert!(
        !evidence.native_numeric_parity_verified
            && !evidence.skeleton_committed
            && !evidence.camera_frame_ready
    );
    let json = serde_json::to_value(evidence).unwrap();
    assert!(json.get("rotations_raw").is_none() && json.get("translations_raw").is_none());
}
