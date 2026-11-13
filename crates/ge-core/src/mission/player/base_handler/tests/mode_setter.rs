use super::super::*;

#[test]
fn secondary_mode_entry_clears_only_source_without_dirty_interface_dependency() {
    for binding in [None, Some(0), Some(7)] {
        let mut owner = Owner::new(PlayerId::new(1).unwrap());
        owner.stage = Stage::VirtualBasicComplete;
        for (offset, value) in [(0x1c0, 77), (0x6e4, 88), (0x70c, 91), (0x714, 92)] {
            owner.w(offset, value);
        }
        if let Some(binding) = binding {
            owner.w(0x4f8, binding);
        }
        let mut expected = owner.words.clone();
        expected.insert(0x1c0, 0);
        owner.begin_secondary_mode_update().unwrap();
        assert_eq!(owner.words, expected);
        assert_eq!(owner.stage(), Stage::VirtualBasicComplete);
    }
}

#[test]
fn secondary_mode_entry_checks_stage_and_failure_before_any_clear() {
    for failed in [false, true] {
        let mut owner = Owner::new(PlayerId::new(1).unwrap());
        owner.w(0x1c0, 77);
        if failed {
            owner.stage = Stage::VirtualBasicComplete;
            owner.failure = Some(Failure::RuntimeGlobalFloat);
        }
        let before = owner.words.clone();
        assert!(owner.begin_secondary_mode_update().is_err());
        assert_eq!(owner.words, before);
    }
}

#[test]
fn primary_mode_entry_preserves_the_clear_prefix_before_dirty_provider_failure() {
    for binding in [None, Some(0), Some(7)] {
        let mut owner = Owner::new(PlayerId::new(1).unwrap());
        owner.stage = Stage::VirtualBasicComplete;
        owner.w(0x1c0, 77);
        owner.w(0x6e4, 88);
        if let Some(binding) = binding {
            owner.w(0x4f8, binding);
        }
        let result = owner.begin_primary_mode_update();
        assert_eq!(result.is_ok(), binding == Some(0));
        assert_eq!(owner.word(0x1c0), Some(0));
        assert_eq!(
            owner.word(0x6e4),
            Some(if binding.is_some() { 0 } else { 88 })
        );
        assert_eq!(owner.word(0x4f8), binding);
    }
}
