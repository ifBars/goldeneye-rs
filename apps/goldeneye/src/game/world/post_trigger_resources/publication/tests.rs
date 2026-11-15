use super::super::{
    files, resolve,
    tests::{FILE, SKIN, TEXTURE, bytes},
};
use super::*;
use ge_core::animation::script_child::{BodyState, Provider};
use ge_core::mission::post_trigger::Request as TargetRequest;
use std::{collections::BTreeMap, sync::Arc};

fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
fn input() -> Vec<u8> {
    let mut data = bytes();
    for (at, value) in [
        (0x104, 0x1d0), // section-zero header start
        (0x1d4, 8),     // reached script fixup callback
        (0x68, 0x0001_0001),
        (0x6c, 0x1a0 - 0x6c),
        (0x1a0, 0x0400_00ab),
        (0x1a4, 0x0001_0000),
        // A NULL section job skips both this bad address and body reads.
        (0x1a8, u32::MAX),
        (0x1ac, 0),
    ] {
        word(&mut data, at, value);
    }
    data
}
fn lookup(files: &mut BTreeMap<u32, super::super::File>, data: &[u8]) -> files::FileBinding {
    files::retain(files, FILE, &mut |_| Edb::parse(data.to_vec())).unwrap()
}
fn binding(source: &mut files::FileBinding) -> Binding {
    source.resolve_script(0x8400_0000).unwrap().unwrap()
}
fn request(files: &mut BTreeMap<u32, super::super::File>, target: u32) -> Result<()> {
    resolve(
        TargetRequest {
            ordinal: 0,
            file: FILE,
            target,
        },
        files,
        &mut |_| panic!("retained wrapper reloaded"),
    )?;
    Ok(())
}

#[test]
fn checked_fresh_callback_proves_null_without_touching_cached_body_or_span() {
    let mut data = input();
    word(&mut data, 0x114, u32::MAX);
    word(&mut data, 0x118, 0);
    let mut files = BTreeMap::new();
    let mut source = lookup(&mut files, &data);
    let binding = binding(&mut source);
    assert_eq!(source.body_state(binding).unwrap(), BodyState::Absent);
    let proof = source
        .script_publication_evidence(binding)
        .unwrap()
        .unwrap();
    assert_eq!(proof.body_state, "known_null");
    assert_eq!(proof.callback_mask, Some(8));
    assert!(proof.fresh_base);
    assert_eq!(proof.request_count, 0);
    assert_eq!(proof.active_requests, 0);
    assert_eq!(proof.retained_sections, 0);
    assert_eq!(proof.raw_job, Some(0));
    assert!(!proof.history_unknown);
    assert!(!proof.native_history_verified);
    assert!(!source.bind_body(binding).unwrap());
    assert!(source.phase_zero_source(binding).is_err());
    assert!(source.script_position_source(binding).is_err());
    assert!(
        ge_core::animation::script_child::forward_tick::Source::duration(&mut source, binding)
            .is_err()
    );
    assert!(files[&FILE].sections.is_empty());
    assert_eq!(source.generation().unwrap(), 0);
}

#[test]
fn existing_binding_observes_same_owner_request_without_generation_change() {
    let data = input();
    let mut files = BTreeMap::new();
    let mut source = lookup(&mut files, &data);
    let binding = binding(&mut source);
    assert_eq!(source.body_state(binding).unwrap(), BodyState::Absent);
    request(&mut files, SKIN).unwrap();
    assert_eq!(source.body_state(binding).unwrap(), BodyState::Pending);
    let proof = source
        .script_publication_evidence(binding)
        .unwrap()
        .unwrap();
    assert_eq!(proof.body_state, "pending");
    assert_eq!(proof.request_count, 1);
    assert_eq!(proof.retained_sections, 1);
    assert_eq!(proof.raw_job, None); // dependency state blocks the job query
    assert!(source.bind_body(binding).is_err());
    assert!(source.phase_zero_source(binding).is_err());
    assert!(source.script_position_source(binding).is_err());
    assert!(
        ge_core::animation::script_child::forward_tick::Source::fps_bits(&mut source, binding)
            .is_err()
    );
    assert_eq!(files[&FILE].sections.len(), 1);
    assert_eq!(source.generation().unwrap(), 0);
    request(&mut files, SKIN).unwrap();
    let owner = Arc::clone(&files[&FILE].edb);
    let mut reused = lookup(&mut files, &data);
    assert!(Arc::ptr_eq(&owner, &files[&FILE].edb));
    drop(files);
    assert_eq!(reused.body_state(binding).unwrap(), BodyState::Pending);
    assert_eq!(owner.publication.0.lock().unwrap().requests, 2);
}

#[test]
fn failed_or_unresolved_request_cannot_leave_a_fresh_unrequested_proof() {
    for target in [0x0400_0001, SKIN] {
        let mut data = input();
        if target == SKIN {
            word(&mut data, 0x114, u32::MAX);
        }
        let mut files = BTreeMap::new();
        let mut source = lookup(&mut files, &data);
        let binding = binding(&mut source);
        assert_eq!(source.body_state(binding).unwrap(), BodyState::Absent);
        assert!(request(&mut files, target).is_err());
        assert_eq!(source.body_state(binding).unwrap(), BodyState::Pending);
        assert!(files[&FILE].sections.is_empty());
        let mut reused = lookup(&mut files, &data);
        assert_eq!(reused.body_state(binding).unwrap(), BodyState::Pending);
    }
}

#[test]
fn unrelated_section_dependencies_block_null_but_other_wrappers_stay_independent() {
    let mut data = input();
    data[0x40..0x42].copy_from_slice(&3u16.to_be_bytes());
    data[0x184..0x186].copy_from_slice(&2u16.to_be_bytes());
    for (at, value) in [(0x120, 0x0800_0002), (0x124, 0x240), (0x128, 0x250)] {
        word(&mut data, at, value);
    }
    let mut files = BTreeMap::new();
    let mut source = lookup(&mut files, &data);
    let binding = binding(&mut source);
    request(&mut files, TEXTURE).unwrap();
    assert_eq!(source.body_state(binding).unwrap(), BodyState::Pending);
    let mut other_files = BTreeMap::new();
    let mut other = lookup(&mut other_files, &data);
    assert!(!Arc::ptr_eq(&files[&FILE].edb, &other_files[&FILE].edb));
    request(&mut files, SKIN).unwrap();
    assert_eq!(source.body_state(binding).unwrap(), BodyState::Pending);
    assert_eq!(other.body_state(binding).unwrap(), BodyState::Absent);
}

#[test]
fn missing_callback_or_unknown_job_stays_pending_despite_available_payload() {
    for (at, value) in [(0x1d4, 0), (0x104, u32::MAX), (0x11c, 1)] {
        let mut data = input();
        word(&mut data, at, value);
        let mut files = BTreeMap::new();
        let mut source = lookup(&mut files, &data);
        let binding = binding(&mut source);
        assert_eq!(source.body_state(binding).unwrap(), BodyState::Pending);
    }
}

#[test]
fn a_nonzero_token_with_base_or_unknown_key_cannot_prove_streamed_null() {
    for key in [0x0800_0000, 0x0700_0001] {
        let mut data = input();
        word(&mut data, 0x110, key);
        let mut files = BTreeMap::new();
        let mut source = lookup(&mut files, &data);
        let binding = binding(&mut source);
        assert_eq!(source.body_state(binding).unwrap(), BodyState::Pending);
        let proof = source
            .script_publication_evidence(binding)
            .unwrap()
            .unwrap();
        assert_eq!(proof.body_state, "pending");
        assert_eq!(proof.raw_job, None);
    }
}

#[test]
fn in_flight_prefix_blocks_null_and_dropping_it_retains_unknown_history() {
    let data = input();
    let mut files = BTreeMap::new();
    let mut source = lookup(&mut files, &data);
    let binding = binding(&mut source);
    let owner = Arc::clone(&files[&FILE].edb);
    let request = owner.publication.begin_request().unwrap();
    assert_eq!(source.body_state(binding).unwrap(), BodyState::Pending);
    request.finish(None).unwrap();
    assert_eq!(source.body_state(binding).unwrap(), BodyState::Absent);
    drop(owner.publication.begin_request().unwrap());
    assert_eq!(source.body_state(binding).unwrap(), BodyState::Pending);
}
