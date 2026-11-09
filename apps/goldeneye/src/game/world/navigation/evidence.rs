//! Scalar receipts and canonical fingerprints of the retained owner state.
use anyhow::{Context, Result, ensure};
use ge_assets::{edb::Edb, graph::TriangleGraph};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::Entry;
use crate::mission::world::TriggerMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Evidence {
    pub origin: &'static str,
    pub initialized: bool,
    pub source_placements: usize,
    pub eligible_placements: usize,
    pub owned_wrappers: usize,
    pub bound_wrappers: usize,
    pub initialized_caches: usize,
    pub vertices: usize,
    pub faces: usize,
    pub category_slots: usize,
    pub nonempty_categories: usize,
    pub category_members: usize,
    pub boundary_pairs: usize,
    pub bindings_sha256: String,
    pub categories_sha256: String,
    pub cache_sha256: String,
    pub state_sha256: String,
    pub original_history_verified: bool,
    pub native_precision_verified: bool,
    pub queries_executed: bool,
    pub scope: &'static str,
}

pub(super) fn build(
    level: &Edb,
    maps: &[TriggerMap<'_>],
    entries: &[Entry],
    graphs: &[TriangleGraph],
) -> Result<Evidence> {
    let first = maps
        .first()
        .context("navigation evidence has no initial map")?;
    let mut bindings = Fingerprint::new(b"ge_navigation_bindings_v1\0");
    let mut categories = Fingerprint::new(b"ge_navigation_owner_categories_v1\0");
    let mut cache = Fingerprint::new(b"ge_navigation_owner_pairs_v1\0");
    bindings.u32(level.header.hash);
    for hash in [&mut bindings, &mut categories, &mut cache] {
        hash.count(entries.len());
    }
    let (mut vertices, mut faces, mut nonempty, mut members, mut pairs) = (0, 0, 0, 0, 0);
    for (ordinal, entry) in entries.iter().enumerate() {
        let owner = &entry.owner;
        let binding = owner.binding();
        let graph = graphs
            .get(entry.graph)
            .context("navigation graph owner absent")?;
        ensure!(
            binding.source.instance == first.loaded.instance
                && owner.face_count() == graph.triangles.len(),
            "navigation evidence owner identity differs"
        );
        bindings.count(ordinal);
        bindings.u64(binding.level_wrapper.get());
        map_identity(&mut bindings, maps, binding.source.instance)?;
        bindings.count(binding.source.index);
        bindings.u32(binding.key.raw());
        bindings.u32(binding.source_entity_reference_raw);
        bindings.count(binding.source_entity_directory_index);
        map_identity(&mut bindings, maps, binding.found.instance)?;
        bindings.count(binding.found.index);
        bindings.u32(binding.cached_entity_reference_raw);
        bindings.count(binding.entity_directory_index);
        bindings.u32(graph.entity.hash);
        for word in binding.translation_bits {
            bindings.u32(word);
        }
        categories.count(owner.categories().len());
        for category in owner.categories() {
            categories.count(category.len());
            for &face in category {
                categories.u32(face);
            }
            nonempty += usize::from(!category.is_empty());
            members += category.len();
        }
        cache.count(owner.cache().edges().len());
        for edge in owner.cache().edges() {
            for vertex in edge.vertices {
                cache.u32(u32::try_from(vertex)?);
            }
        }
        vertices += graph.vertices.len();
        faces += graph.triangles.len();
        pairs += owner.cache().edges().len();
    }
    let bindings = bindings.finish();
    let categories = categories.finish();
    let cache = cache.finish();
    let counts = [
        first.data.placements.len(),
        entries.len(),
        entries.len(),
        entries.len(),
        entries.len(),
        vertices,
        faces,
        entries.len() * 15,
        nonempty,
        members,
        pairs,
    ];
    let mut state = Fingerprint::new(b"ge_navigation_owner_state_v1\0");
    // Fixed-size raw digests, followed by the eleven scalar counters in field
    // declaration order. The domain denotes an explicit fresh, initialized owner.
    state.0.update(bindings);
    state.0.update(categories);
    state.0.update(cache);
    for count in counts {
        state.count(count);
    }
    Ok(Evidence {
        origin: "fresh_owner_snapshot",
        initialized: true,
        source_placements: counts[0],
        eligible_placements: counts[1],
        owned_wrappers: counts[2],
        bound_wrappers: counts[3],
        initialized_caches: counts[4],
        vertices,
        faces,
        category_slots: counts[7],
        nonempty_categories: nonempty,
        category_members: members,
        boundary_pairs: pairs,
        bindings_sha256: hex(&bindings),
        categories_sha256: hex(&categories),
        cache_sha256: hex(&cache),
        state_sha256: hex(&state.finish()),
        original_history_verified: false,
        native_precision_verified: false,
        queries_executed: false,
        scope: "Fresh owned navigation wrappers bind actual retained level entities through source-ordered global placement lookup, preserve W0 translations and retain fifteen runtime face categories and the existing integer boundary cache. Checked graph readers validate additional unused fields. These fingerprints cover initial binding/category/cache state, not arbitrary prior manager history, floating-point query parity, later world hooks, user readiness, player or camera creation. Original geometry and poses remain private.",
    })
}

fn map_identity(
    hash: &mut Fingerprint,
    maps: &[TriggerMap<'_>],
    instance: ge_core::mission::world::InstanceId,
) -> Result<()> {
    let (ordinal, map) = maps
        .iter()
        .enumerate()
        .find(|(_, map)| map.loaded.instance == instance)
        .context("navigation evidence map owner absent")?;
    hash.count(ordinal);
    hash.count(map.data.source_ordinal);
    hash.u64(map.loaded.key.wrapper.get());
    hash.u64(instance.get());
    hash.0.update([map.loaded.slot]);
    hash.u32(map.source.header.hash);
    hash.u32(map.data.descriptor.canonical_reference);
    hash.u32(map.data.descriptor.stored_hash);
    Ok(())
}

struct Fingerprint(Sha256);
impl Fingerprint {
    fn new(domain: &[u8]) -> Self {
        let mut hash = Sha256::new();
        hash.update(domain);
        Self(hash)
    }
    fn u32(&mut self, word: u32) {
        self.0.update(word.to_be_bytes());
    }
    fn u64(&mut self, word: u64) {
        self.0.update(word.to_be_bytes());
    }
    fn count(&mut self, count: usize) {
        self.u64(count as u64);
    }
    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}
fn hex(hash: &[u8; 32]) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}
