//! Exact accesses to the two retained animation-preload allocations.
use super::*;

const RAW_ENTRIES: u8 = 32;
const PRELOAD_ENTRIES: u8 = 34;

impl Shared {
    pub fn clear_preload_raw_flag(&mut self, lease: Lease, index: u8) -> Result<()> {
        self.preload_allocation_mut(lease, Kind::Raw)?
            .put_payload_byte(raw_offset(index, 4)?, 0)
    }

    pub fn clear_preload_entry_key(&mut self, lease: Lease, index: u8) -> Result<()> {
        self.store_preload_entry_key(lease, index, 0)
    }

    /// A reached unknown read is an error, not an empty cache slot.
    pub fn preload_raw_flag(&self, lease: Lease, index: u8) -> Result<u8> {
        self.preload_allocation(lease, Kind::Raw)?
            .payload_byte(raw_offset(index, 4)?)?
            .context("preload raw flag is unwritten")
    }

    pub fn preload_raw_key(&self, lease: Lease, index: u8) -> Result<u32> {
        self.preload_allocation(lease, Kind::Raw)?
            .payload_word(raw_offset(index, 0)?)?
            .context("preload raw key is unwritten")
    }

    pub fn preload_entry_key(&self, lease: Lease, index: u8) -> Result<u32> {
        self.preload_allocation(lease, Kind::Array34)?
            .payload_word(entry_offset(index)?)?
            .context("preload animation entry key is unwritten")
    }

    /// First matching occupied or first empty entry, in native source order.
    /// Empty flags skip keys that the prefix deliberately left unwritten.
    pub fn find_preload_raw_entry(&self, lease: Lease, weapon: u32) -> Result<Option<u8>> {
        for index in 0..RAW_ENTRIES {
            if self.preload_raw_flag(lease, index)? == 0
                || self.preload_raw_key(lease, index)? == weapon
            {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    /// Known exhaustion is distinct from an unread or unknown key.
    pub fn find_preload_entry(&self, lease: Lease, key: u32) -> Result<Option<u8>> {
        for index in 0..PRELOAD_ENTRIES {
            let current = self.preload_entry_key(lease, index)?;
            if current == key || current == 0 {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    /// The native empty-slot branch writes eight independent U8 values.
    pub fn initialize_preload_raw_indices(&mut self, lease: Lease, index: u8) -> Result<()> {
        let offset = raw_offset(index, 5)?;
        let allocation = self.preload_allocation_mut(lease, Kind::Raw)?;
        for delta in 0..8 {
            allocation.put_payload_byte(offset + delta, u8::MAX)?;
        }
        Ok(())
    }

    pub fn store_preload_raw_occupied(&mut self, lease: Lease, index: u8) -> Result<()> {
        self.preload_allocation_mut(lease, Kind::Raw)?
            .put_payload_byte(raw_offset(index, 4)?, 1)
    }

    pub fn store_preload_raw_weapon(&mut self, lease: Lease, index: u8, weapon: u32) -> Result<()> {
        self.preload_allocation_mut(lease, Kind::Raw)?
            .put_payload_word(raw_offset(index, 0)?, weapon)
    }

    pub fn store_preload_raw_mapping(
        &mut self,
        lease: Lease,
        index: u8,
        alternate: bool,
        variant: u8,
        entry: u8,
    ) -> Result<()> {
        ensure!(
            variant < 3,
            "preload variant is outside the three source calls"
        );
        ensure!(
            entry < PRELOAD_ENTRIES,
            "preload mapping entry is outside the cache"
        );
        let field = if alternate { 9 } else { 5 } + variant;
        self.preload_allocation_mut(lease, Kind::Raw)?
            .put_payload_byte(raw_offset(index, field)?, entry)
    }

    pub fn store_preload_entry_key(&mut self, lease: Lease, index: u8, key: u32) -> Result<()> {
        self.preload_allocation_mut(lease, Kind::Array34)?
            .put_payload_word(entry_offset(index)?, key)
    }

    fn validate_preload_lease(&self, lease: Lease) -> Result<()> {
        ensure!(
            self.reset_stage(lease)? == ResetStage::Complete,
            "preload storage requires the actual completed constructor reset"
        );
        Ok(())
    }

    fn preload_allocation(&self, lease: Lease, kind: Kind) -> Result<&Allocation> {
        self.validate_preload_lease(lease)?;
        self.allocations[kind.index()]
            .as_ref()
            .context("reached constructor preload allocation is absent")
    }

    fn preload_allocation_mut(&mut self, lease: Lease, kind: Kind) -> Result<&mut Allocation> {
        self.validate_preload_lease(lease)?;
        self.allocations[kind.index()]
            .as_mut()
            .context("reached constructor preload allocation is absent")
    }
}

fn raw_offset(index: u8, field: u8) -> Result<usize> {
    ensure!(
        index < RAW_ENTRIES,
        "preload raw entry is outside the cache"
    );
    Ok(usize::from(index) * 16 + usize::from(field))
}

fn entry_offset(index: u8) -> Result<usize> {
    ensure!(
        index < PRELOAD_ENTRIES,
        "preload animation entry is outside the cache"
    );
    Ok(usize::from(index) * 208)
}

#[cfg(test)]
#[path = "tests/preload_storage.rs"]
mod tests;
