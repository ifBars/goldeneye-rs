//! Attempts share the controller's retained resource registry. No asset I/O or
//! resource-registry lock is taken while holding the manager-state mutex.
use super::Attempt;
use crate::game::world::post_trigger_resources::Registry;
use anyhow::Result;

impl Attempt {
    pub(in crate::game) fn resource_registry(&self) -> Result<Registry> {
        self.ensure_current()?;
        let registry = self.shared.0.resources.clone();
        self.ensure_current()?;
        Ok(registry)
    }
}

#[cfg(test)]
#[path = "resource_registry/tests.rs"]
mod tests;
