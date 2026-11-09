use anyhow::Result;
use ge_assets::{actor_definitions::Definitions, edb::Edb};
use ge_core::mission::{
    actor_definitions::Registry,
    source_rng::{Origin, SourceRngOwner},
};
use sha2::{Digest, Sha256};

pub(super) fn state(
    edb: &Edb,
    view: &Definitions<'_>,
    registry: &Registry,
    rng: &SourceRngOwner,
    selection_calls: usize,
) -> Result<String> {
    let mut hash = Sha256::new();
    hash.update(b"actor_definition_registry_be_v2\0");
    for word in [
        edb.header.hash,
        edb.header.version,
        edb.header.payload_bytes,
    ] {
        hash.update(word.to_be_bytes());
    }
    hash.update([u8::from(view.primary_count().is_some())]);
    number(&mut hash, view.primary_count().unwrap_or(0));
    hash.update([u8::from(view.secondary_count().is_some())]);
    number(&mut hash, view.secondary_count().unwrap_or(0));
    for index in 0..view.primary_count().unwrap_or(0) {
        let row = view.primary(index)?;
        number(&mut hash, row.source_index());
        words(&mut hash, row.words());
    }
    number(&mut hash, registry.record_count());
    for row in registry.records() {
        hash.update(row.id().get().to_be_bytes());
        number(&mut hash, row.source_index());
        words(&mut hash, row.words());
    }
    number(&mut hash, registry.duplicates().len());
    for duplicate in registry.duplicates() {
        hash.update(duplicate.key.to_be_bytes());
        number(&mut hash, duplicate.retained_source_index);
        number(&mut hash, duplicate.rejected_source_index);
    }
    number(&mut hash, registry.group_count());
    for group in registry.groups() {
        hash.update(group.key().to_be_bytes());
        number(&mut hash, group.cursor());
        number(&mut hash, group.count());
        for id in group.members() {
            hash.update(id.get().to_be_bytes());
        }
    }
    for index in 0..view.secondary_count().unwrap_or(0) {
        let row = view.secondary(index)?;
        number(&mut hash, row.source_index());
        words(&mut hash, row.words());
    }
    hash.update([match rng.origin() {
        Origin::FreshConstructor => 0,
        Origin::SuppliedSnapshot => 1,
    }]);
    hash.update(rng.state_raw().to_be_bytes());
    hash.update(rng.draws_since_snapshot().to_be_bytes());
    number(&mut hash, selection_calls);
    Ok(format!("{:x}", hash.finalize()))
}
fn number(hash: &mut Sha256, value: usize) {
    hash.update((value as u64).to_be_bytes());
}
fn words(hash: &mut Sha256, values: &[u32]) {
    for value in values {
        hash.update(value.to_be_bytes());
    }
}

pub(super) fn rng(owner: &SourceRngOwner) -> String {
    let mut hash = Sha256::new();
    hash.update(b"source_rng_snapshot_be_v1\0");
    hash.update(owner.state_raw().to_be_bytes());
    hash.update(owner.draws_since_snapshot().to_be_bytes());
    format!("{:x}", hash.finalize())
}
