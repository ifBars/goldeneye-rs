use super::super::pose::tests::fixture;
use super::*;

#[test]
fn completed_pose_stays_immutable_while_outer_tail_overwrites_scale_with_character() {
    let (mut pose, mut base, mut reset, mut prep, mut item, mut source) = fixture();
    let rotation = [0x3f00_0000, 0x3f80_0000, 0x4000_0000, 0x4040_0000];
    item.set_pose([0; 4], rotation);
    pose.advance(&mut base, &mut reset, &mut prep, &mut item, &mut source)
        .unwrap();
    assert_eq!(
        item.scale_bits(),
        [1f32.to_bits(), 1f32.to_bits(), 1f32.to_bits(), 0]
    );
    let mut before = Vec::new();
    pose.visit_state_words(|word| before.push(word));
    item.pose_test_flags(0xd8);
    let mut tail = Pending::begin(&pose).unwrap();
    tail.advance(&base, &reset, &mut item, &mut source).unwrap();
    assert_eq!(tail.helper_return(), Some(1));
    assert!(tail.helper_returned());
    assert_eq!(
        tail.rotation_reads(),
        [
            Some([rotation[0], rotation[1]]),
            Some([rotation[2], rotation[3]])
        ]
    );
    assert_eq!(tail.rotation_stores(), [true; 2]);
    assert!(tail.rotation_invalidated());
    assert_eq!(item.rotation_bits(), rotation);
    assert_eq!(
        item.scale_bits(),
        [2f32.to_bits(), 3f32.to_bits(), 2f32.to_bits(), 0]
    );
    assert_eq!(tail.scale_stores(), [true; 2]);
    assert!(tail.scale_invalidated());
    assert_eq!(
        (tail.flags_before(), tail.flags_after(), item.byte_8c()),
        (Some(0xd8), Some(0xdb), 0xdb)
    );
    assert_eq!(
        &source.calls[source.calls.len() - 3..],
        ["char10", "char14", "session"]
    );
    let mut after = Vec::new();
    pose.visit_state_words(|word| after.push(word));
    assert_eq!(before, after);
}

#[test]
fn source_known_absence_returns_after_rotation_without_scale_or_flag_stores() {
    let (mut pose, mut base, mut reset, mut prep, mut item, mut source) = fixture();
    pose.advance(&mut base, &mut reset, &mut prep, &mut item, &mut source)
        .unwrap();
    let mut tail = Pending::begin(&pose).unwrap();
    // Explicit provider-owned NULL is distinct from an unavailable sparse binding.
    source.character_lookup = Some(CharacterLookup::KnownAbsent);
    let unknown = PendingReset::pose_fixture(base.player(), None);
    let calls = source.calls.clone();
    let scale = item.scale_bits();
    tail.advance(&base, &unknown, &mut item, &mut source)
        .unwrap();
    assert_eq!(tail.helper_return(), Some(1));
    assert!(tail.rotation_invalidated());
    assert_eq!(tail.character_lookup(), Some(CharacterLookup::KnownAbsent));
    assert_eq!(tail.scale_stores(), [false; 2]);
    assert_eq!(tail.flags_after(), None);
    assert_eq!(item.scale_bits(), scale);
    assert_eq!(source.calls, calls);
}

#[test]
fn missing_character_owner_keeps_rotation_prefix_and_never_uses_old_numeric_zero() {
    let (mut pose, mut base, mut reset, mut prep, mut item, mut source) = fixture();
    pose.advance(&mut base, &mut reset, &mut prep, &mut item, &mut source)
        .unwrap();
    let mut tail = Pending::begin(&pose).unwrap();
    let unknown = PendingReset::pose_fixture(base.player(), None);
    assert_eq!(base.word(0x6e0), Some(0));
    assert!(
        tail.advance(&base, &unknown, &mut item, &mut source)
            .is_err()
    );
    assert_eq!(tail.stage(), Stage::Character);
    assert!(tail.rotation_invalidated());
    assert_eq!(tail.helper_return(), None);
}

#[test]
fn outer_multiplayer_dependency_preserves_prior_ones_scale_until_provider_exists() {
    let (mut pose, mut base, mut reset, mut prep, mut item, mut source) = fixture();
    pose.advance(&mut base, &mut reset, &mut prep, &mut item, &mut source)
        .unwrap();
    let mut tail = Pending::begin(&pose).unwrap();
    source.multiple = true;
    let scale = item.scale_bits();
    tail.advance(&base, &reset, &mut item, &mut source).unwrap();
    assert_eq!(tail.dependency(), Some(Dependency::MultiplayerScale));
    assert_eq!(
        tail.character_scalars(),
        [Some(2f32.to_bits()), Some(3f32.to_bits())]
    );
    assert_eq!(tail.scale_stores(), [false; 2]);
    assert_eq!(item.scale_bits(), scale);
    assert_eq!(tail.helper_return(), None);
}
