use super::*;
use ge_core::mission::{actor_definitions as actors, catalog};

const FILE: u32 = 0x0100_0041;
const SKIN: u32 = 0x0d00_0101;
const TEXTURE: u32 = 0x0600_0101;

fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn half(data: &mut [u8], at: usize, value: u16) {
    data[at..at + 2].copy_from_slice(&value.to_be_bytes());
}
fn table(data: &mut [u8], index: usize, count: u16, at: usize) {
    let field = 0x40 + index * 8;
    half(data, field, count);
    half(data, field + 2, count);
    word(data, field + 4, (at as i32 - (field + 4) as i32) as u32);
}
fn bytes() -> Vec<u8> {
    let mut data = vec![0; 0x300];
    word(&mut data, 0, 0x4745_4f4d);
    word(&mut data, 4, FILE);
    word(&mut data, 8, 263);
    word(&mut data, 0x14, 0x300);
    word(&mut data, 0x18, 0x200);
    for index in 0..16 {
        table(&mut data, index, 0, 0xc8);
    }
    table(&mut data, 0, 2, 0x100);
    table(&mut data, 4, 1, 0x140);
    table(&mut data, 15, 1, 0x180);
    word(&mut data, 0x100, 0x0800_0000);
    word(&mut data, 0x110, 0x0800_0001);
    word(&mut data, 0x114, 0x220);
    word(&mut data, 0x118, 0x230);
    word(&mut data, 0x140, SKIN);
    half(&mut data, 0x144, 1);
    word(&mut data, 0x180, TEXTURE);
    half(&mut data, 0x184, 1);
    word(&mut data, 0x188, 0x240);
    data[0x220..0x230].copy_from_slice(b"authored payload");
    data
}
fn operation(first: u32, second: u32) -> Operation {
    let mut definition = [0; 17];
    definition[0] = 0x4d00_0101;
    definition[1] = 1;
    definition[6] = FILE;
    definition[7] = first;
    definition[10] = second;
    let mut definitions = actors::Registry::build(
        vec![actors::Primary {
            source_index: 4,
            words: definition,
        }],
        actors::Budget {
            rows: 1,
            groups: 1,
            members: 1,
        },
    )
    .unwrap();
    let mut level = [0; 45];
    level[0] = 0x0181_0001;
    level[33] = 0x4d00_0001;
    let levels = catalog::LevelRegistry::new(
        vec![level],
        vec![],
        catalog::Budget {
            primary: 1,
            secondary: 0,
        },
        catalog::InitialSelection {
            current_hash: u32::MAX,
            previous_hash: 0,
            primary_hash: None,
            secondary_hash: None,
        },
    )
    .unwrap();
    Operation::prepare(
        false,
        Some(&levels.primary()[0]),
        &mut definitions,
        actors::LookupBudget { comparisons: 16 },
    )
    .unwrap()
}

#[test]
fn two_ordered_targets_share_one_file_and_actual_section_without_dropping_a_call() {
    let mut loads = Vec::new();
    let owned = Owned::build_with(operation(SKIN, TEXTURE), |file| {
        loads.push(file);
        Edb::parse(bytes())
    })
    .unwrap();
    assert_eq!(loads, [FILE]);
    assert_eq!(
        owned
            .returned
            .iter()
            .map(|r| r.request.target)
            .collect::<Vec<_>>(),
        [SKIN, TEXTURE]
    );
    assert!(owned.evidence().operation_complete);
    assert_eq!(
        (
            owned.evidence.retained_files,
            owned.evidence.retained_sections,
            owned.evidence.resolved_requests,
            owned.evidence.retained_section_bytes
        ),
        (1, 1, 2, 16)
    );
    assert_eq!(
        owned.operation.selected_definition().unwrap().source_index,
        4
    );
    assert!(!owned.evidence.native_stream_readiness_verified);
}

#[test]
fn target_miss_keeps_consumed_plan_and_still_executes_second_request_and_tail() {
    let mut loads = 0;
    let owned = Owned::build_with(operation(0x0d00_ffff, TEXTURE), |_| {
        loads += 1;
        Edb::parse(bytes())
    })
    .unwrap();
    assert_eq!(loads, 1);
    assert!(!owned.evidence().operation_complete);
    assert_eq!(
        (
            owned.evidence.ordered_requests,
            owned.evidence.resolved_requests
        ),
        (2, 1)
    );
    assert_eq!(owned.evidence.retained_files, 1);
    assert!(owned.returned[0]._failure.is_some());
    assert!(owned.returned[1].resolution.is_some());
    assert!(owned.evidence.tail_reset_applied);
    assert_eq!(
        owned.operation.selected_definition().unwrap().record.get(),
        1
    );
}

#[test]
fn failed_file_loads_do_not_hide_second_call_or_invent_residency() {
    let mut loads = 0;
    let owned = Owned::build_with(operation(SKIN, TEXTURE), |_| {
        loads += 1;
        anyhow::bail!("authored read failure")
    })
    .unwrap();
    assert_eq!(loads, 2);
    assert!(!owned.evidence().operation_complete);
    assert_eq!(
        (
            owned.evidence.retained_files,
            owned.evidence.resolved_requests,
            owned.evidence.retained_section_bytes
        ),
        (0, 0, 0)
    );
    assert!(owned.evidence.tail_reset_applied);
    assert!(owned.returned.iter().all(|r| r._failure.is_some()));
}

#[test]
fn exact_base_key_resolves_without_a_streamed_span_or_loader_success_claim() {
    let mut data = bytes();
    word(&mut data, 0x110, 0x0800_0000);
    word(&mut data, 0x114, u32::MAX);
    word(&mut data, 0x118, 0);
    let owned = Owned::build_with(operation(SKIN, TEXTURE), |_| Edb::parse(data.clone())).unwrap();
    assert!(owned.evidence().operation_complete);
    assert_eq!(
        (
            owned.evidence.resolved_requests,
            owned.evidence.retained_sections,
            owned.evidence.retained_section_bytes
        ),
        (2, 0, 0)
    );
    assert!(!owned.evidence.native_stream_timing_verified);
}

#[test]
fn shared_file_overlapping_sections_count_covered_bytes_once() {
    let mut data = bytes();
    table(&mut data, 0, 3, 0x100);
    word(&mut data, 0x120, 0x0800_0002);
    word(&mut data, 0x124, 0x228);
    word(&mut data, 0x128, 0x238);
    half(&mut data, 0x184, 2);
    let owned = Owned::build_with(operation(SKIN, TEXTURE), |_| Edb::parse(data.clone())).unwrap();
    assert!(owned.evidence().operation_complete);
    assert_eq!(
        (
            owned.evidence.retained_sections,
            owned.evidence.retained_section_bytes
        ),
        (2, 24)
    );
}

#[test]
fn empty_nonbase_section_is_a_real_validated_span_with_zero_covered_bytes() {
    let mut data = bytes();
    word(&mut data, 0x118, 0x220);
    let owned = Owned::build_with(operation(SKIN, TEXTURE), |_| Edb::parse(data.clone())).unwrap();
    assert!(owned.evidence().operation_complete);
    assert_eq!(
        (
            owned.evidence.retained_sections,
            owned.evidence.retained_section_bytes
        ),
        (1, 0)
    );
    assert_eq!(
        owned.files[&FILE].sections[0].sha256,
        <[u8; 32]>::from(Sha256::digest([]))
    );
    assert!(!owned.evidence.native_stream_readiness_verified);
}
