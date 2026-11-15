use super::*;
use crate::mission::player::{components::Registry, constructor::Kind};

struct Owners {
    shared: Shared,
    lease: Lease,
    fp: Identity,
    _registry: Registry,
    caches: transform_cache::Owner,
}
fn owners() -> Owners {
    let player = PlayerId::new(41).unwrap();
    let mut shared = Shared::from_boot();
    let lease = shared
        .acquire(
            player,
            super::super::Budget {
                native_bytes: 63_680,
                leases: 4,
            },
            || Ok(()),
        )
        .unwrap();
    let mut registry = Registry::from_constructor(4);
    registry.allocate(player).unwrap();
    let fp = registry.allocate(player).unwrap();
    Owners {
        shared,
        lease,
        fp,
        _registry: registry,
        caches: transform_cache::Owner::from_boot(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Count,
    Enabled(usize),
    Weapon(usize),
    Remap(Request),
    Extract(Identity, u32),
}
struct Sources {
    rows: Vec<(u8, u32)>,
    mapping: Mapping,
    reference: Option<u32>,
    events: Vec<Event>,
    fail_count: bool,
    fail_remap: bool,
    fail_extract: bool,
}
impl Sources {
    fn new(rows: &[(u8, u32)]) -> Self {
        Self {
            rows: rows.to_vec(),
            mapping: Mapping {
                output: 0x0900_0123,
                accepted: true,
            },
            reference: Some(0x8400_0042),
            events: Vec::new(),
            fail_count: false,
            fail_remap: false,
            fail_extract: false,
        }
    }
    fn remaps(&self) -> Vec<Request> {
        self.events
            .iter()
            .filter_map(|event| match event {
                Event::Remap(request) => Some(*request),
                _ => None,
            })
            .collect()
    }
    fn extraction_count(&self) -> usize {
        self.events
            .iter()
            .filter(|event| matches!(event, Event::Extract(..)))
            .count()
    }
}
impl Source for Sources {
    fn registry_count(&mut self) -> Result<usize> {
        self.events.push(Event::Count);
        if std::mem::take(&mut self.fail_count) {
            anyhow::bail!("synthetic count provider unavailable");
        }
        Ok(self.rows.len())
    }
    fn enabled_byte(&mut self, ordinal: usize) -> Result<u8> {
        self.events.push(Event::Enabled(ordinal));
        Ok(self
            .rows
            .get(ordinal)
            .context("unexpected registry ordinal")?
            .0)
    }
    fn weapon(&mut self, ordinal: usize) -> Result<u32> {
        self.events.push(Event::Weapon(ordinal));
        Ok(self
            .rows
            .get(ordinal)
            .context("unexpected weapon ordinal")?
            .1)
    }
    fn remap(&mut self, request: Request) -> Result<Mapping> {
        self.events.push(Event::Remap(request));
        if std::mem::take(&mut self.fail_remap) {
            anyhow::bail!("synthetic mapping provider unavailable");
        }
        Ok(self.mapping)
    }
    fn extract_reference(&mut self, receiver: Identity, mapped: u32) -> Result<Option<u32>> {
        self.events.push(Event::Extract(receiver, mapped));
        if std::mem::take(&mut self.fail_extract) {
            anyhow::bail!("synthetic reference provider unavailable");
        }
        Ok(self.reference)
    }
}
fn request(owners: &Owners, variant: u8) -> Request {
    Request {
        player: owners.lease.player(),
        receiver: owners.fp,
        ordinal: 0,
        source_ordinal: 3,
        weapon: 0x0200_0081,
        alternate: false,
        variant,
    }
}
fn pending_words(pending: &Pending) -> Vec<u32> {
    let mut out = Vec::new();
    pending.visit_state_words(|word| out.push(word));
    out
}
fn call_words(call: &Call) -> Vec<u32> {
    let mut out = Vec::new();
    call.visit_state_words(|word| out.push(word));
    out
}
fn shared_words(shared: &Shared) -> Vec<u32> {
    let mut out = Vec::new();
    shared.visit_state_words(|word| out.push(word));
    out
}
fn prepare_call(owners: &mut Owners) {
    owners
        .shared
        .clear_preload_raw_flag(owners.lease, 0)
        .unwrap();
    owners
        .shared
        .clear_preload_entry_key(owners.lease, 0)
        .unwrap();
}

#[test]
fn missing_entry_allocation_retains_exact_raw_prefix_and_resume_never_reclears() {
    let mut owner = owners();
    let entries = owner.shared.allocations[Kind::Array34.index()].take();
    let mut pending = Pending::new(owner.lease, owner.fp);
    let mut source = Sources::new(&[]);
    assert!(
        pending
            .advance(
                &mut owner.shared,
                &mut owner.caches,
                &mut source,
                Budget::default()
            )
            .is_err()
    );
    assert_eq!(pending.stage(), Stage::EntryKeys);
    assert_eq!(
        (pending.raw_flags_written(), pending.entry_keys_written()),
        (32, 0)
    );
    assert!(source.events.is_empty());
    let raw = owner.shared.allocation(Kind::Raw).unwrap();
    for offset in 0..512 {
        assert_eq!(
            raw.payload_byte(offset).unwrap(),
            (offset % 16 == 4).then_some(0)
        );
    }
    owner.shared.allocations[Kind::Array34.index()] = entries;
    // Sentinel proves resumed work does not replay the completed flag stores.
    owner
        .shared
        .store_preload_raw_occupied(owner.lease, 0)
        .unwrap();
    pending
        .advance(
            &mut owner.shared,
            &mut owner.caches,
            &mut source,
            Budget::default(),
        )
        .unwrap();
    assert_eq!(owner.shared.preload_raw_flag(owner.lease, 0).unwrap(), 1);
    assert_eq!(
        (pending.raw_flags_written(), pending.entry_keys_written()),
        (32, 34)
    );
    let entries = owner.shared.allocation(Kind::Array34).unwrap();
    for offset in 0..7072 {
        assert_eq!(
            entries.payload_byte(offset).unwrap(),
            (offset % 208 < 4).then_some(0)
        );
    }
    assert_eq!(entries.header_word(8), None);
    assert_eq!(entries.header_word(12), None);
    assert_eq!(
        owner
            .shared
            .allocation(Kind::Array64)
            .unwrap()
            .known_payload_bytes(),
        0
    );
    assert!(pending.helper_returned());
    assert!(pending.flag_cleared());
    assert_eq!(source.events, vec![Event::Count]);
}

#[test]
fn zero_entry_flag_skips_all_allocations_and_registry_work() {
    let mut owner = owners();
    owner.shared.cache_enabled = 0;
    owner.shared.allocations = [None, None, None];
    let before = shared_words(&owner.shared);
    let mut pending = Pending::new(owner.lease, owner.fp);
    let mut source = Sources::new(&[(1, 99)]);
    pending
        .advance(
            &mut owner.shared,
            &mut owner.caches,
            &mut source,
            Budget::default(),
        )
        .unwrap();
    assert_eq!(pending.entry_flag(), Some(0));
    assert_eq!(
        (pending.raw_flags_written(), pending.entry_keys_written()),
        (0, 0)
    );
    assert!(pending.helper_returned());
    assert!(!pending.flag_cleared());
    assert!(source.events.is_empty());
    assert_eq!(shared_words(&owner.shared), before);
}

#[test]
fn disabled_rows_skip_weapon_and_mapping_and_only_exhaustion_clears_the_flag() {
    let mut owner = owners();
    let mut pending = Pending::new(owner.lease, owner.fp);
    // Disabled rows deliberately have unusable zero weapon values.
    let mut source = Sources::new(&[(0, 0), (0, 0), (0, 0)]);
    source.fail_count = true;
    assert!(
        pending
            .advance(
                &mut owner.shared,
                &mut owner.caches,
                &mut source,
                Budget::default()
            )
            .is_err()
    );
    assert_eq!(pending.stage(), Stage::RegistryCount);
    assert_eq!(owner.shared.cache_enabled(), 1);
    pending
        .advance(
            &mut owner.shared,
            &mut owner.caches,
            &mut source,
            Budget::default(),
        )
        .unwrap();
    assert_eq!(
        source.events,
        vec![
            Event::Count,
            Event::Count,
            Event::Enabled(0),
            Event::Enabled(1),
            Event::Enabled(2)
        ]
    );
    assert_eq!(pending.row_flags(), &[0, 0, 0]);
    assert_eq!(pending.enabled_rows(), 0);
    assert_eq!(pending.source_row(), 3);
    assert!(pending.completed_calls().is_empty());
    assert!(pending.flag_cleared());
    assert_eq!(owner.shared.cache_enabled(), 0);
    let state = pending_words(&pending);
    pending
        .advance(
            &mut owner.shared,
            &mut owner.caches,
            &mut source,
            Budget::default(),
        )
        .unwrap();
    assert_eq!(pending_words(&pending), state);
    assert_eq!(source.events.len(), 5);
}

#[test]
fn high_bit_row_is_enabled_and_uses_actual_source_order_before_temporary_wait() {
    let mut owner = owners();
    let mut pending = Pending::new(owner.lease, owner.fp);
    let mut source = Sources::new(&[(0, 1), (0x80, 90), (1, 2)]);
    pending
        .advance(
            &mut owner.shared,
            &mut owner.caches,
            &mut source,
            Budget::default(),
        )
        .unwrap();
    let first = source.remaps();
    assert_eq!(first.len(), 1);
    assert_eq!(
        (
            first[0].source_ordinal,
            first[0].weapon,
            first[0].generic(),
            first[0].variant
        ),
        (1, 90, 0x0900_0765, 0)
    );
    assert_eq!(pending.row_flags(), &[0, 0x80]);
    assert_eq!(pending.enabled_rows(), 1);
    assert_eq!(pending.stage(), Stage::AwaitingTemporary);
    assert!(!pending.helper_returned());
    assert!(!pending.flag_cleared());
    assert_eq!(owner.shared.cache_enabled(), 1);
    assert!(!source.events.contains(&Event::Enabled(2)));
}

#[test]
fn six_calls_keep_alternating_variant_order_and_source_ordinals_across_unsorted_rows() {
    let mut owner = owners();
    let mut pending = Pending::new(owner.lease, owner.fp);
    let mut source = Sources::new(&[(0, 0), (0xff, 90), (1, 2)]);
    source.fail_count = true;
    assert!(
        pending
            .advance(
                &mut owner.shared,
                &mut owner.caches,
                &mut source,
                Budget::default()
            )
            .is_err()
    );
    // A real retained key makes each child take the source existing-entry arm.
    owner
        .shared
        .store_preload_entry_key(owner.lease, 0, source.mapping.output)
        .unwrap();
    pending
        .advance(
            &mut owner.shared,
            &mut owner.caches,
            &mut source,
            Budget::default(),
        )
        .unwrap();
    let requests = source.remaps();
    assert_eq!(requests.len(), 12);
    for (ordinal, request) in requests.iter().enumerate() {
        let in_row = ordinal % 6;
        assert_eq!(request.ordinal, ordinal);
        assert_eq!(request.source_ordinal, 1 + ordinal / 6);
        assert_eq!(request.weapon, if ordinal < 6 { 90 } else { 2 });
        assert_eq!(request.alternate, in_row % 2 != 0);
        assert_eq!(usize::from(request.variant), in_row / 2);
    }
    assert_eq!(source.extraction_count(), 0);
    assert_eq!(pending.completed_calls().len(), 12);
    assert!(
        pending
            .completed_calls()
            .iter()
            .all(
                |call| call.return_reason() == Some(ReturnReason::ExistingEntry)
                    && call.return_word() == Some(1)
                    && !call.invalidated()
            )
    );
    assert!(pending.helper_returned());
    assert!(pending.flag_cleared());
    assert_eq!(owner.shared.cache_enabled(), 0);
}

#[test]
fn unavailable_nonzero_variant_returns_before_any_unwritten_cache_read() {
    let mut owner = owners();
    owner.shared.allocations = [None, None, None];
    let mut source = Sources::new(&[]);
    source.mapping = Mapping {
        output: 0x0900_0765,
        accepted: false,
    };
    let mut call = Call::begin(request(&owner, 2)).unwrap();
    call.advance(
        owner.lease,
        &mut owner.shared,
        &mut owner.caches,
        &mut source,
    )
    .unwrap();
    assert_eq!(call.return_reason(), Some(ReturnReason::VariantUnavailable));
    assert_eq!(call.return_word(), Some(0));
    assert_eq!(
        (
            call.raw_flags_read(),
            call.raw_keys_read(),
            call.entry_keys_read()
        ),
        (0, 0, 0)
    );
    assert_eq!(source.remaps().len(), 1);
    assert_eq!(source.extraction_count(), 0);
    assert_eq!(owner.shared.cache_enabled(), 1);
}

#[test]
fn variant_zero_fallback_skips_unknown_raw_key_and_retains_all_ordered_writes() {
    let mut owner = owners();
    prepare_call(&mut owner);
    let mut source = Sources::new(&[]);
    source.mapping = Mapping {
        output: 0x0900_0765,
        accepted: false,
    };
    source.fail_extract = true;
    let req = request(&owner, 0);
    let mut call = Call::begin(req).unwrap();
    assert!(
        call.advance(
            owner.lease,
            &mut owner.shared,
            &mut owner.caches,
            &mut source
        )
        .is_err()
    );
    assert_eq!(call.stage(), CallStage::Reference);
    assert_eq!(
        (
            call.raw_flags_read(),
            call.raw_keys_read(),
            call.entry_keys_read()
        ),
        (1, 0, 2)
    );
    assert!(
        call.raw_indices_written()
            && call.occupied_written()
            && call.weapon_written()
            && call.mapping_written()
    );
    assert_eq!(owner.shared.preload_raw_flag(owner.lease, 0).unwrap(), 1);
    assert_eq!(
        owner.shared.preload_raw_key(owner.lease, 0).unwrap(),
        req.weapon
    );
    let raw = owner.shared.allocation(Kind::Raw).unwrap();
    for field in 5..=12 {
        assert_eq!(
            raw.payload_byte(field).unwrap(),
            Some(if field == 5 { 0 } else { 0xff })
        );
    }
    assert_eq!(raw.payload_byte(13).unwrap(), None);
    assert_eq!(owner.shared.preload_entry_key(owner.lease, 0).unwrap(), 0);
    assert_eq!(source.remaps(), vec![req]);
    assert_eq!(owner.shared.cache_enabled(), 1);
}

#[test]
fn ff_index_prefix_survives_missing_or_exhausted_entry_table_without_occupancy() {
    for full in [false, true] {
        let mut owner = owners();
        owner.shared.clear_preload_raw_flag(owner.lease, 0).unwrap();
        if full {
            for index in 0..34 {
                owner
                    .shared
                    .store_preload_entry_key(owner.lease, index, u32::from(index) + 1)
                    .unwrap();
            }
        } else {
            owner.shared.allocations[Kind::Array34.index()] = None;
        }
        let mut source = Sources::new(&[]);
        let mut call = Call::begin(request(&owner, 0)).unwrap();
        let result = call.advance(
            owner.lease,
            &mut owner.shared,
            &mut owner.caches,
            &mut source,
        );
        assert_eq!(result.is_ok(), full);
        assert!(call.raw_indices_written());
        assert!(!call.occupied_written() && !call.weapon_written() && !call.mapping_written());
        assert_eq!(owner.shared.preload_raw_flag(owner.lease, 0).unwrap(), 0);
        assert!(owner.shared.preload_raw_key(owner.lease, 0).is_err());
        for field in 5..=12 {
            assert_eq!(
                owner
                    .shared
                    .allocation(Kind::Raw)
                    .unwrap()
                    .payload_byte(field)
                    .unwrap(),
                Some(0xff)
            );
        }
        assert_eq!(call.entry_keys_read(), if full { 34 } else { 0 });
        assert_eq!(
            call.return_reason(),
            full.then_some(ReturnReason::EntriesFull)
        );
        assert_eq!(source.extraction_count(), 0);
    }
}

#[test]
fn occupied_flag_precedes_weapon_then_mapping_and_resume_keeps_each_prefix() {
    let mut owner = owners();
    prepare_call(&mut owner);
    let mut source = Sources::new(&[]);
    let req = request(&owner, 0);
    let mut call = Call::begin(req).unwrap();
    call.stage = CallStage::RawOccupied;
    call.mapping = Some(source.mapping);
    call.raw_index = Some(0);
    call.entry_index = Some(34); // Fails at the later narrow mapping validation.
    assert!(
        call.advance(
            owner.lease,
            &mut owner.shared,
            &mut owner.caches,
            &mut source
        )
        .is_err()
    );
    assert_eq!(call.stage(), CallStage::RawMapping);
    assert!(call.occupied_written() && call.weapon_written() && !call.mapping_written());
    assert_eq!(owner.shared.preload_raw_flag(owner.lease, 0).unwrap(), 1);
    assert_eq!(
        owner.shared.preload_raw_key(owner.lease, 0).unwrap(),
        req.weapon
    );
    assert_eq!(
        owner
            .shared
            .allocation(Kind::Raw)
            .unwrap()
            .payload_byte(5)
            .unwrap(),
        None
    );
    // Correct the synthetic invalid input; continuing does not repeat weapon.
    owner
        .shared
        .store_preload_raw_weapon(owner.lease, 0, 77)
        .unwrap();
    call.entry_index = Some(0);
    call.advance(
        owner.lease,
        &mut owner.shared,
        &mut owner.caches,
        &mut source,
    )
    .unwrap();
    assert_eq!(owner.shared.preload_raw_key(owner.lease, 0).unwrap(), 77);
    assert_eq!(
        owner
            .shared
            .allocation(Kind::Raw)
            .unwrap()
            .payload_byte(5)
            .unwrap(),
        Some(0)
    );
    assert_eq!(call.stage(), CallStage::AwaitingTemporary);
}

#[test]
fn existing_key_returns_one_without_invalidation_extraction_or_temporary_claim() {
    let mut owner = owners();
    prepare_call(&mut owner);
    let mut source = Sources::new(&[]);
    owner
        .shared
        .store_preload_entry_key(owner.lease, 0, source.mapping.output)
        .unwrap();
    let before = owner.caches.snapshot();
    let mut call = Call::begin(request(&owner, 0)).unwrap();
    call.advance(
        owner.lease,
        &mut owner.shared,
        &mut owner.caches,
        &mut source,
    )
    .unwrap();
    assert_eq!(call.return_reason(), Some(ReturnReason::ExistingEntry));
    assert_eq!(call.return_word(), Some(1));
    assert!(!call.invalidated() && !call.extraction_returned());
    assert_eq!(source.extraction_count(), 0);
    assert_eq!(owner.caches.snapshot(), before);
    assert_eq!(owner.shared.cache_enabled(), 1);
}

#[test]
fn both_found_and_known_absent_reference_wait_without_key_commit_or_flag_clear() {
    for reference in [Some(0x8400_0042), None] {
        let mut owner = owners();
        let mut source = Sources::new(&[(1, 0x0200_0081)]);
        source.reference = reference;
        let mut pending = Pending::new(owner.lease, owner.fp);
        pending
            .advance(
                &mut owner.shared,
                &mut owner.caches,
                &mut source,
                Budget::default(),
            )
            .unwrap();
        assert_eq!(pending.stage(), Stage::AwaitingTemporary);
        let call = pending.current_call().unwrap();
        assert_eq!(call.stage(), CallStage::AwaitingTemporary);
        assert!(call.extraction_returned());
        assert_eq!(call.reference(), reference);
        assert_eq!(call.return_word(), None);
        assert_eq!(owner.shared.preload_entry_key(owner.lease, 0).unwrap(), 0);
        assert_eq!(owner.shared.cache_enabled(), 1);
        assert!(!pending.flag_cleared());
        let words = pending_words(&pending);
        let events = source.events.clone();
        pending
            .advance(
                &mut owner.shared,
                &mut owner.caches,
                &mut source,
                Budget::default(),
            )
            .unwrap();
        assert_eq!(pending_words(&pending), words);
        assert_eq!(source.events, events);
    }
}

#[test]
fn failed_extraction_resume_keeps_mapping_storage_prefix_and_read_counters() {
    let mut owner = owners();
    prepare_call(&mut owner);
    let mut source = Sources::new(&[]);
    source.fail_extract = true;
    let mut call = Call::begin(request(&owner, 0)).unwrap();
    assert!(
        call.advance(
            owner.lease,
            &mut owner.shared,
            &mut owner.caches,
            &mut source
        )
        .is_err()
    );
    let before = shared_words(&owner.shared);
    let counts = (
        call.raw_flags_read(),
        call.raw_keys_read(),
        call.entry_keys_read(),
    );
    let failed = call_words(&call);
    call.advance(
        owner.lease,
        &mut owner.shared,
        &mut owner.caches,
        &mut source,
    )
    .unwrap();
    assert_ne!(call_words(&call), failed);
    assert_eq!(shared_words(&owner.shared), before);
    assert_eq!(
        (
            call.raw_flags_read(),
            call.raw_keys_read(),
            call.entry_keys_read()
        ),
        counts
    );
    assert_eq!(source.remaps().len(), 1);
    assert_eq!(source.extraction_count(), 2);
    let returned = call_words(&call);
    call.advance(
        owner.lease,
        &mut owner.shared,
        &mut owner.caches,
        &mut source,
    )
    .unwrap();
    assert_eq!(call_words(&call), returned);
    assert_eq!(source.extraction_count(), 2);
}

#[test]
fn row_and_mapping_failure_resume_never_replays_completed_prefix_or_enabled_read() {
    let mut owner = owners();
    let mut source = Sources::new(&[(1, 7)]);
    source.fail_remap = true;
    let mut pending = Pending::new(owner.lease, owner.fp);
    assert!(
        pending
            .advance(
                &mut owner.shared,
                &mut owner.caches,
                &mut source,
                Budget::default()
            )
            .is_err()
    );
    assert_eq!(
        (pending.raw_flags_written(), pending.entry_keys_written()),
        (32, 34)
    );
    owner
        .shared
        .store_preload_entry_key(owner.lease, 33, 99)
        .unwrap();
    pending
        .advance(
            &mut owner.shared,
            &mut owner.caches,
            &mut source,
            Budget::default(),
        )
        .unwrap();
    assert_eq!(owner.shared.preload_entry_key(owner.lease, 33).unwrap(), 99);
    assert_eq!(
        source.events.iter().filter(|e| **e == Event::Count).count(),
        1
    );
    assert_eq!(
        source
            .events
            .iter()
            .filter(|e| **e == Event::Enabled(0))
            .count(),
        1
    );
    assert_eq!(
        source
            .events
            .iter()
            .filter(|e| **e == Event::Weapon(0))
            .count(),
        1
    );
    assert_eq!(source.remaps().len(), 2);
    assert_eq!(source.remaps()[0], source.remaps()[1]);
    assert_eq!(pending.row_flags(), &[1]);
    assert_eq!(pending.enabled_rows(), 1);
    assert_eq!(pending.stage(), Stage::AwaitingTemporary);
}
