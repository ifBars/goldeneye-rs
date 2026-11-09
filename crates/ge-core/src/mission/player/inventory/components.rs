use super::Inventory;
use anyhow::{Context, Result};

impl Inventory {
    /// 80130d28..54 acts on the already selected inventory's actual slot bytes.
    pub(crate) fn prepare_component_slots(&mut self) -> Result<()> {
        let pending = self
            .slots
            .c55
            .context("component pending inventory slot is unwritten")?;
        if pending as i8 != -1 {
            self.slots.c96 = Some(pending);
            self.slots.c97 = Some(pending);
            self.slots.c55 = Some(0xff);
        } else {
            self.slots.c97 = Some(
                self.slots
                    .c96
                    .context("component current inventory slot is unwritten")?,
            );
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/components.rs"]
mod tests;
