use ge_core::mission::resource_state::{Origin, Owner};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Evidence {
    pub origin: &'static str,
    pub lifetime: &'static str,
    pub shared_owner: bool,
    pub records: usize,
    pub capacity: usize,
    pub nonzero_values: usize,
    pub reset_limit: u32,
    pub lookups: usize,
    pub hits: usize,
    pub misses: usize,
    pub snapshot_bound: bool,
    pub original_history_verified: bool,
    pub state_sha256: String,
    pub scope: &'static str,
}

impl Evidence {
    pub(super) fn snapshot(owner: &Owner, lookups: usize, hits: usize, misses: usize) -> Self {
        let mut hash = Sha256::new();
        hash.update(b"resource_state_owner_be_v1\0");
        hash.update([match owner.origin() {
            Origin::FreshConstructor => 0,
            Origin::SuppliedSnapshot => 1,
        }]);
        hash.update(owner.reset_limit().to_be_bytes());
        hash.update((owner.capacity() as u64).to_be_bytes());
        hash.update((owner.rows().len() as u64).to_be_bytes());
        for record in owner.rows() {
            hash.update(record.key.to_be_bytes());
            hash.update(record.value.to_be_bytes());
        }
        Self {
            origin: match owner.origin() {
                Origin::FreshConstructor => "fresh_constructor_snapshot",
                Origin::SuppliedSnapshot => "supplied_snapshot",
            },
            lifetime: "native_window_controller",
            shared_owner: true,
            records: owner.rows().len(),
            capacity: owner.capacity() as usize,
            nonzero_values: owner.rows().iter().filter(|row| row.value != 0).count(),
            reset_limit: owner.reset_limit(),
            lookups,
            hits,
            misses,
            snapshot_bound: false,
            original_history_verified: false,
            state_sha256: format!("{:x}", hash.finalize()),
            scope: "One actual pure resource-state owner is retained by the native window controller and shared with CPU mission attempts. Its initial rows/capacity and reset limit are explicit source constructor/file snapshots; a lookup miss returns the real caller fallback from that owned state. Rows and values remain private. Lookup counters are attempt-local and do not reset the shared owner. Original saved/runtime consumer history and capture-buffer admission are unbound; this is not original New Game history or world/player/gameplay readiness.",
        }
    }
}
