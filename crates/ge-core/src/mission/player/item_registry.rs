//! Persistent source numeric-ID registry, separate from actor-list membership.
//! Entries contain semantic owned item identities, never source pointers.
use super::identity::ItemId;
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// Maximum logical record capacity, including source growth increments.
    pub records: usize,
    /// Maximum logical native allocation at eight bytes per source record.
    pub native_bytes: usize,
    pub growth_steps: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record {
    numeric_id: u32,
    item: ItemId,
}
impl Record {
    pub fn numeric_id(&self) -> u32 {
        self.numeric_id
    }
    pub fn item(&self) -> ItemId {
        self.item
    }
}

/// Successful append authority; construction and fields remain private.
/// It does not prove actor membership, whole factory or player readiness.
#[derive(Debug)]
pub struct RegistrationReceipt {
    item: ItemId,
    numeric_id: u32,
}
impl RegistrationReceipt {
    pub fn item(&self) -> ItemId {
        self.item
    }
    pub fn numeric_id(&self) -> u32 {
        self.numeric_id
    }
}

#[derive(Debug)]
pub struct Registry {
    counter: u32,
    capacity: u32,
    growth: u32,
    records: Vec<Record>,
}
impl Registry {
    /// Specific boot/static-constructor snapshot. Keep this owner across player
    /// attempts; this initializer does not reconstruct original later history.
    pub fn from_constructor() -> Self {
        Self {
            counter: 0,
            capacity: 0,
            growth: 16,
            records: Vec::new(),
        }
    }
    pub fn register(&mut self, item: ItemId, budget: Budget) -> Result<RegistrationReceipt> {
        // 8027bbb8/bbbc: consume the next numeric ID before capacity checks.
        // Host rejects source wrapping rather than publishing an aliased ID0.
        self.counter = self
            .counter
            .checked_add(1)
            .context("item numeric ID counter overflow")?;
        let numeric_id = self.counter;
        let count =
            u32::try_from(self.records.len()).context("item registry count width overflow")?;
        let next_count = count
            .checked_add(1)
            .context("item registry count overflow")?;
        ensure!(
            next_count <= i32::MAX as u32 && self.capacity <= i32::MAX as u32,
            "item registry exceeds supported source signed header range"
        );
        let mut capacity = self.capacity;
        let mut steps = 0;
        while capacity < next_count {
            ensure!(
                steps < budget.growth_steps,
                "item registry growth work budget exhausted"
            );
            ensure!(
                self.growth > 0 && self.growth <= i32::MAX as u32,
                "item registry growth cannot reach requested count"
            );
            capacity = capacity
                .checked_add(self.growth)
                .context("item registry capacity overflow")?;
            ensure!(
                capacity <= i32::MAX as u32,
                "item registry capacity exceeds source signed range"
            );
            steps += 1;
        }
        ensure!(
            capacity as usize <= budget.records,
            "item registry record budget exhausted"
        );
        let bytes = capacity
            .checked_mul(8)
            .context("item registry native byte size overflow")?;
        ensure!(
            bytes as usize <= budget.native_bytes,
            "item registry native byte budget exhausted"
        );
        if capacity != self.capacity {
            // Actual host storage is larger than native (u32,pointer32) pairs:
            // ItemId also retains world identity. Never label host bytes native.
            let additional = (capacity as usize)
                .checked_sub(self.records.len())
                .context("item registry capacity is below retained count")?;
            self.records
                .try_reserve_exact(additional)
                .context("item registry backing allocation failed")?;
            // Publish actual owned backing before its logical header capacity.
            self.capacity = capacity;
        }
        ensure!(
            self.records.len() < self.records.capacity(),
            "item registry has no owned append storage"
        );
        self.records.push(Record { numeric_id, item });
        Ok(RegistrationReceipt { item, numeric_id })
    }
    pub fn count(&self) -> usize {
        self.records.len()
    }
    pub fn capacity(&self) -> u32 {
        self.capacity
    }
    pub fn growth(&self) -> u32 {
        self.growth
    }
    pub fn counter(&self) -> u32 {
        self.counter
    }
    pub fn records(&self) -> &[Record] {
        &self.records
    }
    pub fn native_storage_bytes(&self) -> usize {
        self.capacity as usize * 8
    }
    /// Actual semantic Record Vec capacity, not original heap allocation size.
    pub fn host_capacity_bytes(&self) -> usize {
        self.records.capacity() * std::mem::size_of::<Record>()
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit(self.counter);
        emit(self.capacity);
        emit(self.growth);
        emit(self.records.len() as u32);
        emit(u32::from(self.records.capacity() != 0));
        for record in &self.records {
            emit(record.numeric_id);
            for id in [record.item.world().get(), record.item.get()] {
                emit((id >> 32) as u32);
                emit(id as u32);
            }
        }
    }
}

#[cfg(test)]
#[path = "item_registry/tests/mod.rs"]
mod tests;
