//! Retained fresh map instances and their actual post-AddMap publication work.
use super::{super::Attempt, maps_source};
use crate::mission::world::TriggerMap;
use anyhow::{Context, Result, ensure};
use ge_core::mission::{
    loader::outer::{ContextSlot, Owner},
    maps::{self, Contexts, Instance, SectionKeys, publication},
    world::{InstanceId, Key},
};
use sha2::{Digest, Sha256};

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Counts {
    pub constructed: usize,
    pub zones: usize,
    pub component_slots: usize,
    pub context_bindings: usize,
    pub sections: usize,
    pub sections_published: usize,
    pub zones_published: usize,
    pub bindings: usize,
}
struct Entry {
    key: Key,
    instance: Instance,
    cache: maps_source::Cache,
    section_keys: SectionKeys,
}
type ContextSnapshot = [(ContextSlot, [u32; 8], [u32; 8]); 4];

pub(super) struct Owned {
    entries: Vec<Entry>,
    // Actual fresh source world +30/+34 owner, separate from resource contexts.
    live_zone_contexts: Vec<(InstanceId, u8)>,
    contexts: ContextSnapshot,
    publication_words: [Option<u32>; 2],
    publication_byte: Option<u8>,
    complete: bool,
}
impl Owned {
    pub fn build(hook: &Attempt, source: &[TriggerMap<'_>]) -> Result<Self> {
        ensure!(
            !source.is_empty() && source.len() <= 128,
            "runtime map owner budget exceeded"
        );
        let mut owned = Self {
            entries: Vec::new(),
            live_zone_contexts: Vec::new(),
            contexts: [(ContextSlot::Unknown, [0; 8], [0; 8]); 4],
            publication_words: [None; 2],
            publication_byte: None,
            complete: false,
        };
        owned.entries.try_reserve_exact(source.len())?;
        for map in source {
            hook.ensure_current()?;
            let view = map
                .source
                .mission_map_runtime(
                    map.loaded.key.map_reference.raw(),
                    ge_assets::mission::map_runtime::Budget::default(),
                )?
                .context("runtime map source is absent")?;
            let input = view.initializer()?;
            let keys = view.section_keys()?;
            let section_keys = SectionKeys {
                count_raw: keys.count_raw,
                keys: keys.keys,
            };
            let instance = Instance::from_constructor(
                map.loaded.key,
                map.loaded.instance,
                map.loaded.slot,
                maps::Initializer {
                    declared_bounds_bits: input.declared_bounds_bits,
                    zones: input
                        .zones
                        .into_iter()
                        .map(|zone| maps::ZoneInput {
                            initial_values_bits: zone.initial_values_bits,
                            flag_word10_raw: zone.flag_word10_raw,
                            word2c_raw: zone.word2c_raw,
                        })
                        .collect(),
                    placement_count: input.placement_count,
                    component_slots: input.component_slots,
                    auxiliary_records: input.auxiliary_records,
                    bitset_bits: input.bitset_bits,
                    section_count: input.section_count,
                },
                maps::Budget::default(),
            )?;
            let cache = maps_source::Cache::new(
                map.source,
                map.data.descriptor.stored_hash,
                map.loaded.key,
            )?;
            ensure!(
                cache.key() == instance.key()
                    && cache.zone_count() == instance.zones().len()
                    && cache.placement_count() == instance.publication().placement_bounds().len(),
                "map publication input ownership differs"
            );
            owned.entries.push(Entry {
                key: map.loaded.key,
                instance,
                cache,
                section_keys,
            });
            // A real AddMap reaches invalidation on every loaded map, followed
            // by the publication hook. Preserve initialized zones and bindings.
            owned.publication_words = [Some(0); 2];
            for entry in &mut owned.entries {
                entry.instance.invalidate();
            }
            owned.publication_byte = Some(0);
            let scope = publication::FreshScope {
                live_contexts: owned.live_zone_contexts.len(),
                non_null_components: owned
                    .entries
                    .iter()
                    .map(|entry| {
                        entry
                            .instance
                            .component_slots()
                            .iter()
                            .filter(|slot| slot.is_some())
                            .count()
                    })
                    .sum(),
            };
            for (ordinal, entry) in owned.entries.iter_mut().enumerate() {
                hook.ensure_current()?;
                let map = &source[ordinal];
                ensure!(
                    entry.key == map.loaded.key,
                    "publication map owner order differs"
                );
                let (publication, sections) = entry.instance.publication_parts();
                let mut adapter =
                    maps_source::Source::new(map.source, &mut entry.cache, scope, sections)?;
                publication::run(
                    publication,
                    sections,
                    &mut adapter,
                    publication::Budget::default(),
                )?;
            }
        }
        {
            let mut state = hook
                .shared
                .0
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("map context owner lock is poisoned"))?;
            hook.ensure_current()?;
            // Source binds resource contexts only AFTER every AddMap hook,
            // then starts trigger factories. Resource and live zone contexts
            // have separate owners and lifetimes.
            let super::Persistent {
                outer, contexts, ..
            } = &mut state.loader;
            for (map, entry) in source.iter().zip(&owned.entries) {
                if map.flags[1] != 0 {
                    contexts.bind_primary(outer, map.loaded.instance, &entry.section_keys)?;
                } else if map.flags[2] != 0 {
                    contexts.bind_secondary(outer, map.loaded.instance, &entry.section_keys)?;
                }
            }
            owned.contexts = context_snapshot(&state.loader.outer, &state.loader.contexts)?;
        }
        owned.complete = owned
            .entries
            .iter()
            .all(|entry| !entry.instance.publication().failed());
        Ok(owned)
    }
    pub fn is_complete(&self) -> bool {
        self.complete
    }
    pub fn counts(&self) -> Counts {
        let mut counts = Counts {
            constructed: self.entries.len(),
            context_bindings: self
                .contexts
                .iter()
                .filter(|slot| matches!(slot.0, ContextSlot::Bound(_)))
                .count(),
            ..Counts::default()
        };
        for entry in &self.entries {
            counts.zones += entry.instance.zones().len();
            counts.component_slots += entry.instance.component_slots().len();
            counts.sections += entry.instance.sections().section_count();
            counts.sections_published += 1;
            for zone in entry.instance.publication().zones() {
                counts.zones_published += usize::from(zone.initialized());
                counts.bindings += zone.bindings().len();
            }
        }
        counts
    }
    pub fn state_sha256(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(b"ge_loader_maps_v1\0");
        count(&mut hash, self.live_zone_contexts.len());
        count(&mut hash, self.entries.len());
        for word in self.publication_words {
            hash.update([u8::from(word.is_some())]);
            if let Some(word) = word {
                hash.update(word.to_be_bytes());
            }
        }
        hash.update([u8::from(self.publication_byte.is_some())]);
        if let Some(byte) = self.publication_byte {
            hash.update([byte]);
        }
        for entry in &self.entries {
            hash.update(entry.key.wrapper.get().to_be_bytes());
            hash.update(entry.key.map_reference.raw().to_be_bytes());
            entry
                .instance
                .visit_state_words(|word| hash.update(word.to_be_bytes()));
            count(&mut hash, entry.instance.sections().section_count());
            for word in entry.instance.sections().loaded_mask() {
                hash.update(word.to_be_bytes());
            }
            entry.cache.fingerprint(&mut hash);
            let state = entry.instance.publication();
            hash.update([u8::from(state.failed())]);
            count(&mut hash, state.zones().len());
            for zone in state.zones() {
                hash.update([u8::from(zone.initialized())]);
                count(&mut hash, zone.bindings().len());
                for binding in zone.bindings() {
                    for word in binding.words() {
                        hash.update(word.to_be_bytes());
                    }
                }
            }
            count(&mut hash, state.placement_bounds().len());
            for bounds in state.placement_bounds() {
                bounds_hash(&mut hash, *bounds);
            }
            bounds_hash(&mut hash, state.aggregate());
        }
        for (slot, first, second) in &self.contexts {
            match slot {
                ContextSlot::Unknown => hash.update([0]),
                ContextSlot::Absent => hash.update([1]),
                ContextSlot::Bound(id) => {
                    hash.update([2]);
                    hash.update(id.get().to_be_bytes());
                }
            }
            for word in first.iter().chain(second) {
                hash.update(word.to_be_bytes());
            }
        }
        hash.update([u8::from(self.complete)]);
        format!("{:x}", hash.finalize())
    }
}
fn context_snapshot(outer: &Owner, contexts: &Contexts) -> Result<ContextSnapshot> {
    let mut result = [(ContextSlot::Unknown, [0; 8], [0; 8]); 4];
    for (i, slot) in result.iter_mut().enumerate() {
        *slot = (
            outer.contexts()[i],
            *contexts
                .first_mask(i)
                .context("context first mask absent")?,
            *contexts
                .second_mask(i)
                .context("context second mask absent")?,
        );
    }
    Ok(result)
}
fn count(hash: &mut Sha256, value: usize) {
    hash.update((value as u64).to_be_bytes());
}
fn bounds_hash(hash: &mut Sha256, bounds: Option<ge_core::collision::bounds::Bounds>) {
    hash.update([u8::from(bounds.is_some())]);
    if let Some(bounds) = bounds {
        for value in bounds.minimum.into_iter().chain(bounds.maximum) {
            hash.update(value.to_bits().to_be_bytes());
        }
    }
}
