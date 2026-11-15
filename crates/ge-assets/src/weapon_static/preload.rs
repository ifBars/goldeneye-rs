//! Source-ordinal registry iteration and its reached preload flag. No map,
//! parent, ordering table or neighboring row field is consumed by this read.
use super::{WeaponRecord, WeaponTables};
use anyhow::{Context, Result, ensure};

impl WeaponTables {
    pub fn record_at_source_ordinal(&self, ordinal: usize) -> Result<WeaponRecord<'_>> {
        let record = self
            .source_record(ordinal)
            .context("preload weapon registry ordinal is absent")?;
        ensure!(
            record.source_index() == ordinal,
            "preload weapon registry ordinal differs from retained source order"
        );
        Ok(record)
    }
}
impl WeaponRecord<'_> {
    /// Exact U8 at hex24. Every nonzero value, including80/ff, is enabled.
    pub fn preload_enabled_byte(&self) -> Result<u8> {
        self.byte(0x24)
    }
}
