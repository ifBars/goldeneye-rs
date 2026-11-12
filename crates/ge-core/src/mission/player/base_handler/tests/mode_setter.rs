use super::super::*;
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
        assert_eq!(owner.word(0x6e4), Some(0));
        assert_eq!(owner.word(0x4f8), binding);
    }
}
