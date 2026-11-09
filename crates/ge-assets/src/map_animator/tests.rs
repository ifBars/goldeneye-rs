use super::*;

fn word(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn relative(bytes: &mut [u8], field: usize, target: usize) {
    word(bytes, field, (target as i32 - field as i32) as u32);
}

fn fixture() -> (Vec<u8>, Table, Budget) {
    let mut bytes = vec![0; 0x260];
    word(&mut bytes, 0x20, 0x0500_00a5);
    word(&mut bytes, 0x28, 0x80);
    word(&mut bytes, 0x80, 0x500);
    relative(&mut bytes, 0xd8, 0x100);
    relative(&mut bytes, 0x108, 0x140);
    relative(&mut bytes, 0x140, 0x180);
    // The second row is deliberately unaddressable; index zero is valid.
    word(&mut bytes, 0x148, i32::MAX as u32);
    bytes[0x184..0x1a4].fill(0xff);
    bytes[0x185] = 0;
    bytes[0x1a4..0x1a6].copy_from_slice(&2u16.to_be_bytes());
    bytes[0x1ad] = 4;
    bytes[0x1ae] = 2;
    bytes[0x1af] = 1;
    bytes[0x1b0] = 0x38;
    bytes[0x1b4..0x1b6].copy_from_slice(&1u16.to_be_bytes());
    bytes[0x1c3] = 7;
    bytes[0x1c4] = 9;
    bytes[0x1c8] = 0xe1;
    bytes[0x1d4] = 0x17;
    (
        bytes,
        Table {
            name: "maps",
            count: 1,
            hash_size_raw: 1,
            offset: 0x20,
        },
        Budget {
            maps: 2,
            animator_indices: 4,
            states: 4,
            instructions: 8,
            bytes: 0x100,
        },
    )
}

#[test]
fn coded_zero_and_literal_map_use_same_first_animator() {
    let (bytes, maps, budget) = fixture();
    let coded = read(&bytes, &maps, 0x8500_0000, 0, budget)
        .unwrap()
        .unwrap();
    let literal = read(&bytes, &maps, 0x0500_00a5, 0, budget)
        .unwrap()
        .unwrap();
    assert_eq!(coded.locator().body_address, literal.locator().body_address);
    assert_eq!(coded.locator().map_hash, 0x0500_00a5);
    assert_eq!(coded.locator().animator_index, 0);
    assert_eq!(coded.header().shared_variables, 2);
}

#[test]
fn header_and_events_do_not_address_truncated_program() {
    let (mut bytes, maps, budget) = fixture();
    bytes.truncate(0x1b4);
    let view = read(&bytes, &maps, 0x8500_0000, 0, budget)
        .unwrap()
        .unwrap();
    assert_eq!(view.event_mapping(0x100).unwrap(), 0);
    assert_eq!(view.event_mapping(0xff).unwrap(), 0xff);
    assert!(view.state(0).is_err());
    assert!(view.instruction(0).is_err());
}

#[test]
fn reached_state_and_statement_have_distinct_strides() {
    let (bytes, maps, budget) = fixture();
    let view = read(&bytes, &maps, 0x8500_0000, 0, budget)
        .unwrap()
        .unwrap();
    let state = view.state(0).unwrap();
    assert_eq!(
        (state.program, state.behavior_raw, state.execution_flag_raw),
        (1, 7, 9)
    );
    assert_eq!(state.raw().len(), 20);
    assert_eq!(view.instruction(0).unwrap().opcode, 0xe1);
    let next = view.instruction(1).unwrap();
    assert_eq!(next.address, 0x1d4);
    assert_eq!(next.raw().len(), 12);
}

#[test]
fn mapped_events_do_not_alias_direct_state_or_unmapped_index() {
    let (bytes, maps, budget) = fixture();
    let view = read(&bytes, &maps, 0x8500_0000, 0, budget)
        .unwrap()
        .unwrap();
    assert!(view.event_mapping(0).is_err());
    assert!(view.event_mapping(0x11f).is_err());
    assert!(view.state(255).is_err());
    assert!(view.instruction(2).is_err());
}

#[test]
fn missing_map_and_absent_body_are_distinct_from_corruption() {
    let (mut bytes, maps, budget) = fixture();
    assert!(
        read(&bytes, &maps, 0x0500_00ee, 0, budget)
            .unwrap()
            .is_none()
    );
    word(&mut bytes, 0x140, 0);
    assert!(
        read(&bytes, &maps, 0x8500_0000, 0, budget)
            .unwrap()
            .is_none()
    );
    word(&mut bytes, 0x140, i32::MIN as u32);
    assert!(read(&bytes, &maps, 0x8500_0000, 0, budget).is_err());
    relative(&mut bytes, 0x140, 0x180);
    word(&mut bytes, 0x108, 0);
    assert!(read(&bytes, &maps, 0x8500_0000, 0, budget).is_err());
}

#[test]
fn header_format_and_truncation_are_checked_at_binding() {
    let (mut bytes, maps, budget) = fixture();
    bytes[0x1b0] = 0x30;
    assert!(read(&bytes, &maps, 0x8500_0000, 0, budget).is_err());
    bytes[0x1b0] = 0x38;
    bytes.truncate(0x1b3);
    assert!(read(&bytes, &maps, 0x8500_0000, 0, budget).is_err());
}

#[test]
fn lookup_budgets_do_not_force_unused_state_or_instruction_reads() {
    let (bytes, maps, mut budget) = fixture();
    budget.states = 0;
    budget.instructions = 0;
    let view = read(&bytes, &maps, 0x8500_0000, 0, budget)
        .unwrap()
        .unwrap();
    assert!(view.state(0).is_err());
    assert!(view.instruction(0).is_err());
    budget.animator_indices = 0;
    assert!(read(&bytes, &maps, 0x8500_0000, 0, budget).is_err());
    budget.animator_indices = 1;
    budget.maps = 0;
    assert!(read(&bytes, &maps, 0x8500_0000, 0, budget).is_err());
}

#[test]
fn body_byte_budget_is_checked_on_each_reached_span() {
    let (bytes, maps, mut budget) = fixture();
    budget.bytes = 0x34;
    let view = read(&bytes, &maps, 0x8500_0000, 0, budget)
        .unwrap()
        .unwrap();
    assert!(view.state(0).is_err());
    budget.bytes = 0x54;
    let view = read(&bytes, &maps, 0x8500_0000, 0, budget)
        .unwrap()
        .unwrap();
    assert!(view.instruction(0).is_ok());
    assert!(view.instruction(1).is_err());
    budget.bytes = 0x33;
    assert!(read(&bytes, &maps, 0x8500_0000, 0, budget).is_err());
}
