//! Map preregistration and publication bookkeeping for an explicitly fresh world.
//!
//! The host supplies canonical resident-wrapper identities, references resolved
//! by its asset reader, and handles for actual constructed map instances. During
//! native construction the map preregisters itself; publication is a later step.
//! This module constructs no map, invokes no world hooks and establishes no
//! trigger, player or camera readiness.
use std::num::NonZeroU64;

use anyhow::{Result, ensure};

/// A semantic host identity, never an original pointer or resource hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WrapperId(NonZeroU64);

impl WrapperId {
    pub fn new(value: u64) -> Result<Self> {
        Ok(Self(NonZeroU64::new(value).ok_or_else(|| {
            anyhow::anyhow!("world wrapper identity must be nonzero")
        })?))
    }

    pub fn get(self) -> u64 {
        self.0.get()
    }
}

/// A semantic handle for an already constructed, host-owned map instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstanceId(NonZeroU64);

impl InstanceId {
    pub fn new(value: u64) -> Result<Self> {
        Ok(Self(NonZeroU64::new(value).ok_or_else(|| {
            anyhow::anyhow!("world instance identity must be nonzero")
        })?))
    }

    pub fn get(self) -> u64 {
        self.0.get()
    }
}

/// Coded map-directory reference. Literal hash resolution and first-body alias
/// selection belong to the checked asset reader, before entering this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MapReference(u32);

impl MapReference {
    pub fn from_directory_index(index: u32) -> Result<Self> {
        ensure!(
            index <= 0x00ff_ffff,
            "map directory index exceeds coded-reference representation"
        );
        Ok(Self(0x8500_0000 | index))
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    pub wrapper: WrapperId,
    pub map_reference: MapReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// Includes the constructor's null slot. At most 256 slots are supported
    /// here so source `index << 24` does not discard index bits. This is a host
    /// bound, not a checked native limit or support for signed-byte consumers.
    pub preregistered_slots: usize,
    pub loaded_instances: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub wrapper: Option<WrapperId>,
    /// Zero only for the constructor's null slot.
    pub map_reference_raw: u32,
    pub index: u8,
    pub instance: Option<InstanceId>,
}

impl Slot {
    pub fn packed_tag(self) -> u32 {
        u32::from(self.index) << 24
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loaded {
    pub key: Key,
    pub instance: InstanceId,
    /// Source preregistration index, distinct from this row's loaded ordinal.
    pub slot: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Published {
    pub slot: u8,
    pub loaded_ordinal: usize,
    pub instance: InstanceId,
    /// A loaded-key hit performs no new publication or cache/world hooks.
    pub reused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationError {
    MissingPreregistration {
        key: Key,
    },
    LoadedBudgetExhausted {
        limit: usize,
    },
    /// The native optional-instance path can rebind a pointer already present
    /// in the loaded vector. Mutable pointer aliases are outside this narrowly
    /// supported fresh-instance path; silently copying immutable rows differs.
    UnsupportedInstanceReuse {
        instance: InstanceId,
    },
}

impl std::fmt::Display for PublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "world map publication failed: {self:?}")
    }
}

impl std::error::Error for PublicationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    budget: Budget,
    slots: Vec<Slot>,
    loaded: Vec<Loaded>,
}

impl Registry {
    /// Explicit constructor-only history, not a default for arbitrary sessions.
    pub fn fresh(budget: Budget) -> Result<Self> {
        ensure!(
            (1..=256).contains(&budget.preregistered_slots),
            "world preregistration budget must include null slot and fit 256 slots"
        );
        Ok(Self {
            budget,
            slots: vec![Slot {
                wrapper: None,
                map_reference_raw: 0,
                index: 0,
                instance: None,
            }],
            loaded: Vec::new(),
        })
    }

    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    /// Storage order of actual publications, with no constructor null row.
    pub fn loaded(&self) -> &[Loaded] {
        &self.loaded
    }

    /// Called by actual map construction, with the asset reader's first body
    /// match. A failed later bind/publication does not erase this source side
    /// effect. Existing pairs keep their source index and bound instance.
    pub fn preregister(&mut self, key: Key) -> Result<Slot> {
        if let Some(slot) = self.find_slot(key) {
            return Ok(slot);
        }
        ensure!(
            self.slots.len() < self.budget.preregistered_slots,
            "world preregistration slot budget exhausted"
        );
        let slot = Slot {
            wrapper: Some(key.wrapper),
            map_reference_raw: key.map_reference.raw(),
            index: u8::try_from(self.slots.len())?,
            instance: None,
        };
        self.slots.push(slot);
        Ok(slot)
    }

    /// Publishes an actual constructed map. The native loaded-key lookup
    /// precedes factory/candidate handling: a hit ignores the supplied handle.
    ///
    /// A missing preregistration can occur after a body alias mismatch; it is
    /// an error, never an invented slot. This operation performs no source
    /// cache invalidation or post-publication hooks. Those remain host work.
    pub fn publish_constructed(
        &mut self,
        key: Key,
        instance: InstanceId,
    ) -> std::result::Result<Published, PublicationError> {
        if let Some((ordinal, loaded)) = self
            .loaded
            .iter()
            .enumerate()
            .find(|(_, loaded)| loaded.key == key)
        {
            return Ok(Published {
                slot: loaded.slot,
                loaded_ordinal: ordinal,
                instance: loaded.instance,
                reused: true,
            });
        }
        if self.loaded.iter().any(|loaded| loaded.instance == instance) {
            return Err(PublicationError::UnsupportedInstanceReuse { instance });
        }
        let slot = self
            .find_slot(key)
            .ok_or(PublicationError::MissingPreregistration { key })?;
        if self.loaded.len() >= self.budget.loaded_instances {
            return Err(PublicationError::LoadedBudgetExhausted {
                limit: self.budget.loaded_instances,
            });
        }
        let published = Published {
            slot: slot.index,
            loaded_ordinal: self.loaded.len(),
            instance,
            reused: false,
        };
        self.slots[usize::from(slot.index)].instance = Some(instance);
        self.loaded.push(Loaded {
            key,
            instance,
            slot: slot.index,
        });
        Ok(published)
    }

    fn find_slot(&self, key: Key) -> Option<Slot> {
        self.slots.iter().copied().find(|slot| {
            slot.wrapper == Some(key.wrapper) && slot.map_reference_raw == key.map_reference.raw()
        })
    }
}

#[cfg(test)]
#[path = "tests/world.rs"]
mod tests;
