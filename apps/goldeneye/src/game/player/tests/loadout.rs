//! Independently authored adapter inputs; no supplied assets or DOL tables.
use super::*;
use ge_assets::weapon_metadata::InventoryMetadata;
use ge_core::mission::{
    Difficulty,
    session::{NewGameRequest, OperationResult, SessionId, UserId},
};

fn metadata() -> DefaultMetadata {
    DefaultMetadata {
        inventory: InventoryMetadata {
            inventory_key: DEFAULT_ITEM,
            primary_key: 0x4b00_00a7,
            type_key: 0x4b20_0000,
            category_key: 0x4b10_0001,
            byte_9_raw: 0,
            ammunition_raw: [0; 2],
            source_index: 12,
            group_zero_address: 0x200,
            group_four_address: 0x500,
        },
        native_sorted_index: 5,
        byte_8_raw: 0,
        normal_values_bits: [20., 40., 60., 1., 2., 30., 50., 70., 3., 4.].map(f32::to_bits),
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
#[derive(Default)]
struct Flags {
    bound: bool,
    fail_at: Option<usize>,
    checks: usize,
    calls: Vec<u32>,
}
impl FlagProvider for Flags {
    fn interface_bound(&mut self) -> Result<bool> {
        self.checks += 1;
        Ok(self.bound)
    }
    fn clear_interface_flags(&mut self, mask: u32) -> Result<()> {
        let index = self.calls.len();
        self.calls.push(mask);
        ensure!(self.fail_at != Some(index), "authored interface failure");
        Ok(())
    }
}
fn session() -> Session {
    Session::begin(
        NewGameRequest::new(Difficulty::Agent),
        SessionId::new(19).unwrap(),
        UserId::new(41).unwrap(),
    )
}

#[test]
fn active_matching_session_is_required_before_source_loading() {
    let mut session = session();
    let level = session.request().level_hash();
    assert!(validate_session(&session, level).is_err());
    session.mode_result(OperationResult::Succeeded).unwrap();
    session.begin_create().unwrap();
    session.create_result(OperationResult::Succeeded).unwrap();
    assert!(validate_session(&session, level).is_err());
    session.select_pending(session.identity()).unwrap();
    session
        .admission_result(OperationResult::Succeeded)
        .unwrap();
    assert!(validate_session(&session, level).is_ok());
    assert!(validate_session(&session, level ^ 1).is_err());
    assert!(!session.multiple_local_users_in_mode_three());
}

#[test]
fn typed_asset_adapter_keeps_sorted_index_distinct_from_source_index_and_values() {
    let source = metadata();
    let row = adapt_metadata(source);
    assert_eq!((row.source_index, row.native_sorted_index), (12, 5));
    assert_eq!(row.inventory_key, source.inventory.inventory_key);
    assert_eq!(row.primary_key, source.inventory.primary_key);
    assert_eq!(row.type_key, source.inventory.type_key);
    assert_eq!(row.category_key, source.inventory.category_key);
    assert_eq!(row.byte_8_raw, source.byte_8_raw);
    assert_eq!(row.byte_9_raw, source.inventory.byte_9_raw);
    assert_eq!(row.ammunition_raw, source.inventory.ammunition_raw);
    assert_eq!(row.normal_values_bits, source.normal_values_bits);
    assert_eq!(row.modifier_bits, source.modifier_bits);
}

#[test]
fn caller_container_is_reset_and_both_actual_orders_drive_selection() {
    let row = adapt_metadata(metadata());
    let mut inventory = Inventory::from_constructor();
    inventory.admit_default(row, INVENTORY_BUDGET).unwrap();
    let mut flags = Flags::default();
    let selected = initialize(
        &mut inventory,
        &row,
        &[0x0200_00a1, DEFAULT_ITEM],
        &[0x0200_00b2],
        inputs(),
        &mut flags,
    )
    .unwrap();
    assert_eq!(inventory.records().len(), 1);
    assert_eq!((inventory.capacity(), inventory.growth()), (16, 16));
    assert_eq!(
        (selected.primary_record, selected.primary_order_index),
        (0, 1)
    );
    assert_eq!(selected.metadata_sorted_index, 5);
    assert_eq!(inventory.slots().c98, Some(5));
    assert_eq!(inventory.slots().c99, Some(0xff));
    assert_eq!((flags.checks, selected.interface_callbacks), (3, 0));
    let selected = initialize(
        &mut inventory,
        &row,
        &[DEFAULT_ITEM],
        &[DEFAULT_ITEM],
        inputs(),
        &mut flags,
    )
    .unwrap();
    assert_eq!(selected.secondary_record, Some(0));
    assert_eq!(inventory.slots().c99, Some(0));
    assert_eq!(inventory.records()[0].padding, None);
}

#[test]
fn admission_or_callback_failure_keeps_actual_reached_container_and_slot_state() {
    let row = adapt_metadata(metadata());
    let mut inventory = Inventory::from_constructor();
    inventory.admit_default(row, INVENTORY_BUDGET).unwrap();
    let mut unsupported = row;
    unsupported.ammunition_raw[0] = 3;
    assert!(
        initialize(
            &mut inventory,
            &unsupported,
            &[DEFAULT_ITEM],
            &[],
            inputs(),
            &mut Flags::default()
        )
        .is_err()
    );
    assert!(inventory.records().is_empty());
    assert_eq!(inventory.capacity(), 0); // reset really preceded failed admission
    let mut flags = Flags {
        bound: true,
        fail_at: Some(1),
        ..Flags::default()
    };
    assert!(
        initialize(
            &mut inventory,
            &row,
            &[DEFAULT_ITEM],
            &[],
            inputs(),
            &mut flags
        )
        .is_err()
    );
    assert_eq!(flags.calls, [0x40, 0x80]);
    assert_eq!(inventory.records().len(), 1);
    assert_eq!(inventory.slots().c98, Some(5));
    assert!(inventory.slots().pose_bits.is_some());
    assert!(inventory.selection().is_none());
}

#[test]
fn constructor_metadata_requires_the_actual_selected_retained_row() {
    let row = adapt_metadata(metadata());
    let mut inventory = Inventory::from_constructor();
    assert!(selected_metadata(&row, 5, &inventory).is_err());
    initialize(
        &mut inventory,
        &row,
        &[DEFAULT_ITEM],
        &[],
        inputs(),
        &mut Flags::default(),
    )
    .unwrap();
    assert!(std::ptr::eq(
        selected_metadata(&row, 5, &inventory).unwrap(),
        &row
    ));
    assert!(selected_metadata(&row, 4, &inventory).is_err());
    assert!(selected_metadata(&row, 0xff, &inventory).is_err());
    let mut foreign_row = row;
    foreign_row.native_sorted_index = 4;
    assert!(selected_metadata(&foreign_row, 5, &inventory).is_err());
    inventory.reset();
    assert!(selected_metadata(&row, 5, &inventory).is_err());
}
