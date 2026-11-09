use super::*;

fn metadata() -> Metadata {
    Metadata {
        inventory_key: 0x0200_0830,
        primary_key: 0x4b00_00a1,
        type_key: 0x4b20_0000,
        category_key: 0x4b10_0001,
        byte_8_raw: 0,
        byte_9_raw: 0,
        ammunition_raw: [0; 2],
        source_index: 9,
        native_sorted_index: 5,
        normal_values_bits: [
            60f32.to_bits(),
            40f32.to_bits(),
            0,
            1f32.to_bits(),
            1f32.to_bits(),
            0,
            0,
            0,
            1f32.to_bits(),
            1f32.to_bits(),
        ],
        modifier_bits: [1f32.to_bits(); 4],
    }
}
fn inputs() -> PoseInputs {
    PoseInputs {
        blend_bits: 0,
        c54_raw: None,
        player_flags_raw: 0,
        flag_5ee_raw: 0,
        flag_95c_raw: 0,
        multiplayer_raw: 0,
    }
}
fn budget() -> Budget {
    Budget {
        records: 32,
        order_words: 16,
    }
}
#[derive(Default)]
struct Flags {
    bound: bool,
    checks: usize,
    calls: Vec<u32>,
    fail_at: Option<usize>,
}
impl FlagProvider for Flags {
    fn interface_bound(&mut self) -> Result<bool> {
        self.checks += 1;
        Ok(self.bound)
    }
    fn clear_interface_flags(&mut self, mask: u32) -> Result<()> {
        ensure!(
            self.fail_at != Some(self.calls.len()),
            "authored callback failure"
        );
        self.calls.push(mask);
        Ok(())
    }
}
fn admitted() -> Inventory {
    let mut i = Inventory::from_constructor();
    i.admit_default(metadata(), budget()).unwrap();
    i
}
#[test]
fn actual_container_constructor_and_reset_preserve_growth_and_unknown_padding() {
    let mut i = Inventory::from_constructor();
    assert_eq!(i.capacity(), 0);
    assert_eq!(i.growth(), 16);
    assert_eq!(i.slots().c96, None);
    assert!(i.records().is_empty());
    assert_eq!(i.admit_default(metadata(), budget()).unwrap(), 0);
    assert_eq!(i.capacity(), 16);
    assert_eq!(i.records()[0].padding, None);
    assert_eq!(i.records()[0].count, 1);
    assert_eq!(i.records()[0].ordinal, 0xff);
    let r = i.reset();
    assert_eq!(r.previous_capacity, 16);
    assert_eq!(r.previous_count, 1);
    assert!(r.had_backing);
    assert_eq!(i.growth(), 16);
    assert_eq!(i.capacity(), 0);
    assert!(i.records().is_empty());
}
#[test]
fn unsupported_metadata_and_backing_budget_leave_empty_owner() {
    let mut i = Inventory::from_constructor();
    let mut m = metadata();
    m.ammunition_raw[0] = 7;
    assert!(i.admit_default(m, budget()).is_err());
    assert!(
        i.admit_default(
            metadata(),
            Budget {
                records: 15,
                ..budget()
            }
        )
        .is_err()
    );
    assert!(i.records().is_empty());
    assert_eq!(i.capacity(), 0);
}
#[test]
fn supplied_orders_and_runtime_sorted_index_drive_distinct_retained_slots() {
    let mut i = admitted();
    let mut f = Flags::default();
    let selected = i
        .select_default(
            &[0x0200_00a5, 0x0200_0830],
            &[0x0200_00a6],
            &metadata(),
            inputs(),
            &mut f,
            budget(),
        )
        .unwrap();
    assert_eq!(selected.primary_order_index, 1);
    assert_eq!(selected.primary_record, 0);
    assert_eq!(selected.metadata_sorted_index, 5);
    assert_eq!(selected.secondary_record, None);
    assert_eq!(
        (i.slots().c96, i.slots().c97, i.slots().c98, i.slots().c99),
        (Some(0), Some(0), Some(5), Some(0xff))
    );
    assert_eq!((i.slots().c55, i.slots().ca0), (Some(0xff), Some(0)));
    assert_eq!(f.checks, 3);
    assert!(f.calls.is_empty());
    assert_eq!(selected.interface_callbacks, 0);
}
#[test]
fn bound_interface_callbacks_keep_source_order_and_failure_preserves_reached_fields() {
    let mut i = admitted();
    let mut f = Flags {
        bound: true,
        fail_at: Some(1),
        ..Flags::default()
    };
    assert!(
        i.select_default(&[0x0200_0830], &[], &metadata(), inputs(), &mut f, budget())
            .is_err()
    );
    assert_eq!(f.calls, vec![0x40]);
    assert!(i.slots().pose_bits.is_some());
    assert!(i.selection().is_none());
    let mut f = Flags {
        bound: true,
        ..Flags::default()
    };
    let selected = i
        .select_default(&[0x0200_0830], &[], &metadata(), inputs(), &mut f, budget())
        .unwrap();
    assert_eq!(f.calls, vec![0x40, 0x80, 0x100]);
    assert_eq!(selected.interface_callbacks, 3);
}
#[test]
fn unknown_conditional_is_supported_only_when_actual_branches_converge() {
    let mut i = admitted();
    let mut f = Flags::default();
    let mut m = metadata();
    m.modifier_bits[0] = 2f32.to_bits();
    assert!(
        i.select_default(&[0x0200_0830], &[], &m, inputs(), &mut f, budget())
            .is_err()
    );
    assert!(i.slots().pose_bits.is_some());
    assert!(i.selection().is_none());
    assert_eq!(f.checks, 0);
    let chosen = i
        .select_default(&[0x0200_0830], &[], &metadata(), inputs(), &mut f, budget())
        .unwrap();
    assert_eq!(chosen.pose_bits[4], 1f32.to_bits());
}
#[test]
fn zero_modifier_skips_reciprocal_and_keeps_fifth_word_finite() {
    let mut i = admitted();
    let mut m = metadata();
    m.modifier_bits[0] = 0;
    let selected = i
        .select_default(
            &[0x0200_0830],
            &[],
            &m,
            PoseInputs {
                c54_raw: Some(1),
                ..inputs()
            },
            &mut Flags::default(),
            budget(),
        )
        .unwrap();
    assert_eq!(selected.pose_bits[..2], [0, 0]);
    assert_eq!(selected.pose_bits[4], 1f32.to_bits());
}
#[test]
fn reset_does_not_retroactively_clear_player_slots_and_failed_lookup_keeps_source_ff_prefix() {
    let mut i = admitted();
    i.select_default(
        &[0x0200_0830],
        &[],
        &metadata(),
        inputs(),
        &mut Flags::default(),
        budget(),
    )
    .unwrap();
    i.reset();
    assert_eq!(i.slots().c96, Some(0));
    assert!(i.selection().is_none());
    i.admit_default(metadata(), budget()).unwrap();
    assert!(
        i.select_default(
            &[0x0200_00a9],
            &[],
            &metadata(),
            inputs(),
            &mut Flags::default(),
            budget()
        )
        .is_err()
    );
    assert_eq!(i.slots().c96, Some(0xff));
    assert_eq!(i.slots().c97, Some(0xff));
}
