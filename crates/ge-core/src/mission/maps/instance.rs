use super::{publication, sections};
use crate::mission::world::{InstanceId, Key};
use anyhow::{Result, ensure};
use std::num::NonZeroU64;

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub zones: usize,
    pub placements: usize,
    pub component_slots: usize,
    pub auxiliary_records: usize,
    pub bitset_bits: usize,
    pub sections: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            zones: 256,
            placements: 1_000_000,
            component_slots: 1_000_000,
            auxiliary_records: 0,
            bitset_bits: 0,
            sections: 32767,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoneInput {
    pub initial_values_bits: [u32; 4],
    pub flag_word10_raw: u32,
    pub word2c_raw: u32,
}
#[derive(Debug)]
pub struct Initializer {
    pub declared_bounds_bits: [[u32; 3]; 2],
    pub zones: Vec<ZoneInput>,
    pub placement_count: usize,
    pub component_slots: usize,
    pub auxiliary_records: usize,
    pub bitset_bits: usize,
    pub section_count: usize,
}
#[derive(Debug)]
pub struct Zone {
    pub handle: u16,
    pub initial_values_bits: [u32; 4],
    pub current_values_bits: [u32; 4],
    pub flag_word10_raw: u32,
    pub flag_28: bool,
    pub flag_40: bool,
    pub word_24: u32,
    pub word_3c: u32,
    /// Source +8 self, expressed in the owning instance's zone domain.
    pub self_ordinal: usize,
    /// Source +0,+4,+c,+10. No native pointers are synthesized.
    pub links: [Option<usize>; 4],
    /// Source +6b..+6e. Publication separately owns +6a.
    pub cache_flags: [u8; 4],
}
#[derive(Debug)]
pub struct Instance {
    key: Key,
    identity: InstanceId,
    prereg_index: u8,
    declared_bounds_bits: [[u32; 3]; 2],
    zones: Vec<Zone>,
    component_slots: Vec<Option<NonZeroU64>>,
    section_cache: Vec<u32>,
    fixed_cache: [u8; 32],
    cache_flag: u8,
    sections: sections::Owner,
    publication: publication::State,
}
impl Instance {
    pub fn from_constructor(
        key: Key,
        identity: InstanceId,
        prereg_index: u8,
        input: Initializer,
        budget: Budget,
    ) -> Result<Self> {
        ensure!(
            prereg_index != 0,
            "constructed map cannot use null preregistration slot"
        );
        ensure!(
            input.zones.len() <= budget.zones && input.zones.len() <= 256,
            "map zone count exceeds supported host handle budget"
        );
        ensure!(
            input.placement_count <= budget.placements,
            "map placement budget exhausted"
        );
        ensure!(
            input.component_slots <= budget.component_slots,
            "map component budget exhausted"
        );
        ensure!(
            input.auxiliary_records <= budget.auxiliary_records
                && input.bitset_bits <= budget.bitset_bits,
            "map auxiliary budget exhausted"
        );
        ensure!(
            input.auxiliary_records == 0 && input.bitset_bits == 0,
            "nonempty map auxiliary/optional bitset constructor unsupported"
        );
        let sections =
            sections::Owner::from_fresh_base(key.wrapper, input.section_count, budget.sections)?;
        let publication = publication::State::from_fresh_constructor(
            key,
            input.zones.len(),
            input.placement_count,
            publication::Budget {
                zones: budget.zones,
                placements: budget.placements,
                ..publication::Budget::default()
            },
        )?;
        let mut zones = Vec::new();
        zones.try_reserve_exact(input.zones.len())?;
        for (ordinal, source) in input.zones.into_iter().enumerate() {
            zones.push(Zone {
                handle: (u16::from(prereg_index) << 8) | ordinal as u16,
                initial_values_bits: source.initial_values_bits,
                current_values_bits: source.initial_values_bits,
                flag_word10_raw: source.flag_word10_raw,
                flag_28: source.flag_word10_raw != 0,
                flag_40: source.flag_word10_raw != 0,
                word_24: source.word2c_raw,
                word_3c: source.word2c_raw,
                self_ordinal: ordinal,
                links: [None; 4],
                cache_flags: [0; 4],
            });
        }
        let mut component_slots = Vec::new();
        component_slots.try_reserve_exact(input.component_slots)?;
        component_slots.resize(input.component_slots, None);
        let words = input.placement_count.div_ceil(32);
        let mut section_cache = Vec::new();
        section_cache.try_reserve_exact(words)?;
        section_cache.resize(words, 0);
        Ok(Self {
            key,
            identity,
            prereg_index,
            declared_bounds_bits: input.declared_bounds_bits,
            zones,
            component_slots,
            section_cache,
            fixed_cache: [0; 32],
            cache_flag: 0,
            sections,
            publication,
        })
    }
    /// Source list/cache invalidation preserves +6a, bindings, bounds and defaults.
    pub fn invalidate(&mut self) {
        for zone in &mut self.zones {
            zone.links = [None; 4];
            zone.cache_flags = [0; 4];
        }
        self.fixed_cache.fill(0);
        self.cache_flag = 0;
        self.section_cache.fill(0);
    }
    pub fn key(&self) -> Key {
        self.key
    }
    pub fn identity(&self) -> InstanceId {
        self.identity
    }
    pub fn prereg_index(&self) -> u8 {
        self.prereg_index
    }
    pub fn declared_bounds_bits(&self) -> &[[u32; 3]; 2] {
        &self.declared_bounds_bits
    }
    pub fn zones(&self) -> &[Zone] {
        &self.zones
    }
    pub fn component_slots(&self) -> &[Option<NonZeroU64>] {
        &self.component_slots
    }
    pub fn section_cache(&self) -> &[u32] {
        &self.section_cache
    }
    pub fn fixed_cache(&self) -> &[u8; 32] {
        &self.fixed_cache
    }
    pub fn publication(&self) -> &publication::State {
        &self.publication
    }
    pub fn publication_mut(&mut self) -> &mut publication::State {
        &mut self.publication
    }
    pub fn sections(&self) -> &sections::Owner {
        &self.sections
    }
    pub fn publication_parts(&mut self) -> (&mut publication::State, &sections::Owner) {
        (&mut self.publication, &self.sections)
    }
    /// Canonical u32 stream, not a native object image. u64 semantic IDs/lengths
    /// are high word then low word; optional links carry presence then ordinal.
    /// Publication and section owners are deliberately visited separately.
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        fn wide(emit: &mut impl FnMut(u32), n: u64) {
            emit((n >> 32) as u32);
            emit(n as u32);
        }
        wide(&mut emit, self.key.wrapper.get());
        emit(self.key.map_reference.raw());
        wide(&mut emit, self.identity.get());
        emit(u32::from(self.prereg_index));
        for v in self.declared_bounds_bits {
            for w in v {
                emit(w);
            }
        }
        wide(&mut emit, self.zones.len() as u64);
        for z in &self.zones {
            emit(u32::from(z.handle));
            for w in z.initial_values_bits {
                emit(w);
            }
            for w in z.current_values_bits {
                emit(w);
            }
            emit(z.flag_word10_raw);
            emit(u32::from(z.flag_28));
            emit(u32::from(z.flag_40));
            emit(z.word_24);
            emit(z.word_3c);
            wide(&mut emit, z.self_ordinal as u64);
            for link in z.links {
                emit(u32::from(link.is_some()));
                if let Some(i) = link {
                    wide(&mut emit, i as u64);
                }
            }
            for flag in z.cache_flags {
                emit(u32::from(flag));
            }
        }
        wide(&mut emit, self.component_slots.len() as u64);
        for slot in &self.component_slots {
            emit(u32::from(slot.is_some()));
            if let Some(id) = slot {
                wide(&mut emit, id.get());
            }
        }
        wide(&mut emit, self.section_cache.len() as u64);
        for &w in &self.section_cache {
            emit(w);
        }
        for b in self.fixed_cache {
            emit(u32::from(b));
        }
        emit(u32::from(self.cache_flag));
    }
}
#[cfg(test)]
#[path = "tests/instance.rs"]
mod tests;
