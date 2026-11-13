use super::*;
use crate::mission::player::inventory::{Budget, Inventory, Metadata};

struct Source {
    calls: Vec<(u8, MetadataField)>,
    kind: u32,
    key: u32,
    primary: u32,
    reject_slot: bool,
}
impl MetadataSource for Source {
    fn word(&mut self, slot: u8, field: MetadataField) -> Result<u32> {
        self.calls.push((slot, field));
        ensure!(
            !self.reject_slot && slot == 23,
            "selected metadata row unavailable"
        );
        Ok(match field {
            MetadataField::Type => self.kind,
            MetadataField::InventoryKey => self.key,
            MetadataField::PrimaryKey => self.primary,
        })
    }
}
fn source() -> Source {
    Source {
        calls: Vec::new(),
        kind: 0x4b20_0000,
        key: 0x0200_0830,
        primary: 0x4b00_002c,
        reject_slot: false,
    }
}
fn fixture() -> (
    Pending,
    base_handler::Owner,
    PendingReset,
    PendingPrelude,
    Source,
) {
    let player = PlayerId::new(7).unwrap();
    let base =
        base_handler::Owner::constructor_tail_fixture(player, Some(2), Some(0), Some(0), Some(0));
    let reset = PendingReset::constructor_tail_fixture(player);
    let mut inventory = Inventory::from_constructor();
    inventory
        .admit_default(
            Metadata {
                inventory_key: 0x0200_0830,
                primary_key: 0x4b00_002c,
                type_key: 0x4b20_0000,
                category_key: 0x4b10_0001,
                byte_8_raw: 0,
                byte_9_raw: 0,
                ammunition_raw: [0; 2],
                source_index: 0,
                native_sorted_index: 23,
                normal_values_bits: [0; 10],
                modifier_bits: [0; 4],
            },
            Budget {
                records: 32,
                order_words: 32,
            },
        )
        .unwrap();
    let prelude = PendingPrelude::continuation_fixture(player, inventory);
    // Authored boundary after a returned callback; the app integration owns
    // real callback/entry capture construction and uses Pending::begin.
    let pending = Pending::new(CallbackEntry {
        player,
        saved_state_704: 0,
    });
    (pending, base, reset, prelude, source())
}

#[test]
fn default_owned_cleanup_returns_one_then_waits_for_signed_pose_input() {
    let (mut p, mut base, mut reset, prelude, mut s) = fixture();
    p.advance(&mut base, &mut reset, &prelude, &mut s).unwrap();
    assert_eq!(p.stage(), Stage::AwaitingPose);
    assert_eq!(p.dependency(), Some(Dependency::Pose));
    assert_eq!(p.script_reference(), Some(0x0400_1dac));
    assert_eq!(
        s.calls,
        [
            (23, MetadataField::Type),
            (23, MetadataField::InventoryKey),
            (23, MetadataField::PrimaryKey)
        ]
    );
    assert_eq!(p.current_inventory_key(), Some(0x0200_0830));
    assert_eq!(p.current_record_present(), Some(true));
    assert_eq!(p.current_flags(), Some(0));
    assert_eq!(p.variant(), Some(0));
    assert_eq!(p.cleanup_checks(), &[Some(true); 5]);
    assert!(p.b84_written() && p.bd1_written());
    assert_eq!((reset.word(0xb84), reset.byte(0xbd1)), (Some(0), Some(0)));
    assert_eq!((reset.byte(0xbd0), reset.byte(0xbd2)), (Some(7), Some(11)));
    for at in [0x474, 0x478, 0x47c, 0x484] {
        assert_eq!(base.word(at), None);
    }
    assert_eq!(
        p.override_removals().map(|v| v.unwrap().key),
        [0x0600_056d, 0x0600_067b]
    );
    assert!(
        p.override_removals()
            .iter()
            .all(|v| v.is_some_and(|v| v.count_raw == 0 && !v.removed))
    );
    assert_eq!(p.helper_return(), Some(1));
    assert_eq!(p.pose_input_raw(), Some(255));
    assert_eq!(p.pose_input_signed(), Some(-1));
    assert_eq!(base.word(0x704), Some(2));
    assert!(!p.state_restored());
    let calls = s.calls.clone();
    p.advance(&mut base, &mut reset, &prelude, &mut s).unwrap();
    assert_eq!(s.calls, calls);
}

#[test]
fn actual_entry_capture_controls_restore_instead_of_later_diagnostic() {
    let (_, mut base, mut reset, prelude, mut s) = fixture();
    let incoming =
        base_handler::Owner::constructor_tail_fixture(base.player(), Some(5), None, None, None);
    let entry = CallbackEntry::capture(&incoming).unwrap();
    let mut p = Pending::new(entry);
    p.advance(&mut base, &mut reset, &prelude, &mut s).unwrap();
    assert!(p.state_restored());
    assert_eq!(p.saved_state_704(), 5);
    assert_eq!(base.word(0x704), Some(5));
    let unknown =
        base_handler::Owner::constructor_tail_fixture(base.player(), None, None, None, None);
    assert!(CallbackEntry::capture(&unknown).is_err());
}

#[test]
fn special_script_type_stops_before_unreached_inventory_or_metadata_fields() {
    let (mut p, mut base, mut reset, prelude, mut s) = fixture();
    s.kind = 0x4b20_0001;
    reset.constructor_tail_test_byte(0xc96, None);
    p.advance(&mut base, &mut reset, &prelude, &mut s).unwrap();
    assert_eq!(p.dependency(), Some(Dependency::ScriptSelection));
    assert_eq!(s.calls, [(23, MetadataField::Type)]);
    assert_eq!(p.script_reference(), None);
    assert_eq!(p.current_record_present(), None);
    assert!(!p.b84_written());
}

#[test]
fn negative_metadata_slot_skips_every_row_provider_and_takes_pending_normal_binding() {
    let (mut p, mut base, mut reset, prelude, mut s) = fixture();
    reset.constructor_tail_test_byte(0xc98, Some(0xff));
    s.reject_slot = true;
    p.advance(&mut base, &mut reset, &prelude, &mut s).unwrap();
    assert!(s.calls.is_empty());
    assert_eq!(p.script_reference(), Some(0x0400_1dac));
    assert_eq!(p.dependency(), Some(Dependency::NormalComponentBinding));
}

#[test]
fn wrong_positive_slot_is_an_error_without_a_default_row_substitution() {
    let (mut p, mut base, mut reset, prelude, mut s) = fixture();
    reset.constructor_tail_test_byte(0xc98, Some(24));
    assert!(p.advance(&mut base, &mut reset, &prelude, &mut s).is_err());
    assert_eq!(p.stage(), Stage::SelectScript);
    assert_eq!(p.script_reference(), None);
    assert_eq!(p.metadata_reads(), &[None; 3]);
}

#[test]
fn alternate_gate_does_not_read_flags_before_its_supported_branch() {
    let (mut p, _, mut reset, prelude, mut s) = fixture();
    let mut base = base_handler::Owner::constructor_tail_fixture(
        p.player(),
        Some(2),
        Some(1),
        Some(0),
        Some(0),
    );
    p.advance(&mut base, &mut reset, &prelude, &mut s).unwrap();
    assert_eq!(p.dependency(), Some(Dependency::AlternateVariant));
    assert_eq!(p.current_inventory_key(), Some(0x0200_0830));
    assert_eq!(p.current_flags(), None);
    assert_eq!(s.calls, [(23, MetadataField::Type)]);
}

#[test]
fn null_first_component_skips_early_child_read_and_unknown_body_precedes_stores() {
    let (mut p, mut base, mut reset, prelude, mut s) = fixture();
    reset.constructor_tail_test_word(0xaf8, None);
    reset.constructor_tail_test_word(0xb00, None);
    assert!(p.advance(&mut base, &mut reset, &prelude, &mut s).is_err());
    assert_eq!(p.stage(), Stage::BodyCleanup);
    assert_eq!(p.cleanup_checks()[0], Some(true));
    assert_eq!(p.cleanup_checks()[2], None);
    assert!(!p.b84_written());
    let calls = s.calls.clone();
    reset.constructor_tail_test_word(0xb00, Some(0));
    assert!(p.advance(&mut base, &mut reset, &prelude, &mut s).is_err());
    assert_eq!(p.stage(), Stage::ReadCleanupLink);
    assert_eq!(s.calls, calls);
}

#[test]
fn nonnull_child_keeps_b84_prefix_but_skips_marker_and_later_gates() {
    let (mut p, mut base, mut reset, prelude, mut s) = fixture();
    reset.constructor_tail_test_word(0xaf8, Some(7));
    p.advance(&mut base, &mut reset, &prelude, &mut s).unwrap();
    assert_eq!(p.dependency(), Some(Dependency::ChildCleanup));
    assert_eq!(p.cleanup_checks()[2], Some(false));
    assert_eq!(reset.word(0xb84), Some(0));
    assert_eq!(reset.byte(0xbd1), Some(9));
    assert!(p.b84_written() && !p.bd1_written());
    assert_eq!(p.actor_absent(), None);
}

#[test]
fn unknown_actor_keeps_marker_prefix_and_resumes_without_replaying_metadata() {
    let (mut p, _, mut reset, prelude, mut s) = fixture();
    let mut base =
        base_handler::Owner::constructor_tail_fixture(p.player(), Some(2), Some(0), None, Some(0));
    assert!(p.advance(&mut base, &mut reset, &prelude, &mut s).is_err());
    assert_eq!(p.stage(), Stage::ActorGate);
    assert!(p.bd1_written());
    let calls = s.calls.clone();
    base.constructor_tail_test_word(0x570, 0);
    p.advance(&mut base, &mut reset, &prelude, &mut s).unwrap();
    assert_eq!(s.calls, calls);
    assert_eq!(p.stage(), Stage::AwaitingPose);
}

#[test]
fn positive_visual_count_requires_entries_but_signed_nonpositive_count_does_not() {
    for (count, expected) in [
        (1, Some(Dependency::VisualOverrides)),
        (u32::MAX, Some(Dependency::Pose)),
    ] {
        let (mut p, mut base, mut reset, prelude, mut s) = fixture();
        base.constructor_tail_test_word(0x480, count);
        p.advance(&mut base, &mut reset, &prelude, &mut s).unwrap();
        assert_eq!(p.dependency(), expected);
        assert_eq!(base.word(0x474), None);
        assert_eq!(base.word(0x484), None);
    }
}

#[test]
fn unknown_pose_input_does_not_erase_the_real_helper_return() {
    let (mut p, mut base, mut reset, prelude, mut s) = fixture();
    reset.constructor_tail_test_byte(0xc7d, None);
    assert!(p.advance(&mut base, &mut reset, &prelude, &mut s).is_err());
    assert_eq!(p.stage(), Stage::PoseInput);
    assert_eq!(p.helper_return(), Some(1));
    assert_eq!(p.pose_input_raw(), None);
    let calls = s.calls.clone();
    reset.constructor_tail_test_byte(0xc7d, Some(0x80));
    p.advance(&mut base, &mut reset, &prelude, &mut s).unwrap();
    assert_eq!(p.pose_input_signed(), Some(-128));
    assert_eq!(s.calls, calls);
}
