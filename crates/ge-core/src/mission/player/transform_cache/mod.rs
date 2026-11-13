//! Persistent shared transform-cache storage and selection. Construction leaves
//! matrices unwritten; neither allocation nor selection evaluates a pose.
mod backing;
mod build;
mod state;
mod types;

pub use backing::{Backing, BackingId, BackingKind};
pub use types::{Budget, BuildStage, BuildTrace, Header, Input, Slot, Snapshot, Stage, Trace};

use super::identity::PlayerId;
use anyhow::{Result, ensure};

/// Two shared headers and their independent selected-header pointer. Boot and
/// header-constructor effects happen once, rather than once per player attempt.
#[derive(Debug)]
pub struct Owner {
    headers: [Header; 2],
    selected: Option<Slot>,
    next_backing: u64,
    invocation_count: u64,
    trace: Option<Trace>,
}

impl Owner {
    pub fn from_boot() -> Self {
        // Explicit startup clear establishes association/selection NULL. Each
        // pre-main header ctor clears only the first five words afterward.
        Self {
            headers: [Header::from_boot(), Header::from_boot()],
            selected: None,
            next_backing: 0,
            invocation_count: 0,
            trace: None,
        }
    }
    pub fn header(&self, slot: Slot) -> &Header {
        &self.headers[slot.index()]
    }
    pub fn selected(&self) -> Option<Slot> {
        self.selected
    }
    pub fn trace(&self) -> Option<&Trace> {
        self.trace.as_ref()
    }
    pub fn invocation_count(&self) -> u64 {
        self.invocation_count
    }
    pub fn native_bytes(&self) -> usize {
        self.headers.iter().map(Header::native_bytes).sum()
    }
    pub fn allocations(&self) -> usize {
        self.headers.iter().map(Header::allocations).sum()
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            selected: self.selected,
            counts: self.headers.each_ref().map(|header| header.count),
            matrix_backings: self
                .headers
                .each_ref()
                .map(|header| header.matrix.as_ref().map(Backing::id)),
            associations: self.headers.each_ref().map(|header| header.association),
            native_bytes: self.native_bytes(),
            allocations: self.allocations(),
            invocation_count: self.invocation_count,
        }
    }

    /// Only matrix-pointer presence gates selection, including partial builds.
    /// This does not associate a player or assert that a matrix is initialized.
    pub fn select(&mut self, mode: i32) -> Option<Slot> {
        self.selected = match mode {
            0 => Some(Slot::FirstPerson),
            1 => Some(Slot::ThirdPerson),
            _ => None,
        }
        .filter(|slot| self.header(*slot).matrix.is_some());
        self.selected
    }

    /// The lazy provider is the actual player+98c semantic alias read. It is
    /// never queried when selected or input player is NULL. Unknown is an error.
    pub fn bind_selected_with(
        &mut self,
        player: Option<PlayerId>,
        alias: impl FnOnce(PlayerId) -> Result<Option<PlayerId>>,
    ) -> Result<bool> {
        let (Some(slot), Some(player)) = (self.selected, player) else {
            return Ok(false);
        };
        let target = alias(player)?.unwrap_or(player);
        self.headers[slot.index()].association = Some(target);
        Ok(true)
    }
    pub fn bind_selected(
        &mut self,
        player: Option<PlayerId>,
        alias: Option<PlayerId>,
    ) -> Result<bool> {
        self.bind_selected_with(player, |_| Ok(alias))
    }
    pub fn selected_matches_with(
        &self,
        player: Option<PlayerId>,
        alias: impl FnOnce(PlayerId) -> Result<Option<PlayerId>>,
    ) -> Result<bool> {
        let (Some(slot), Some(player)) = (self.selected, player) else {
            return Ok(false);
        };
        // Source reads alias before selected association.
        let target = alias(player)?.unwrap_or(player);
        Ok(self.header(slot).association == Some(target))
    }
    pub fn selected_matches(
        &self,
        player: Option<PlayerId>,
        alias: Option<PlayerId>,
    ) -> Result<bool> {
        self.selected_matches_with(player, |_| Ok(alias))
    }

    /// Read count and then validity-pointer presence even when count is zero.
    /// Missing validity is harmless at zero, and an error for a positive count.
    pub fn invalidate_selected(&mut self) -> Result<()> {
        let Some(slot) = self.selected else {
            return Ok(());
        };
        let header = &mut self.headers[slot.index()];
        let count = header.count as usize;
        let validity = header.validity.as_mut();
        header.invalidation_count_reads += 1;
        header.invalidation_pointer_reads += 1;
        if count == 0 {
            return Ok(());
        }
        let validity = validity
            .ok_or_else(|| anyhow::anyhow!("selected transform validity backing absent"))?;
        ensure!(
            validity.native_bytes() >= count,
            "selected transform validity backing is short"
        );
        validity.fill(0, count, 0)?;
        Ok(())
    }

    /// Actual owned backing release, independently checking every field. The
    /// association survives. A selected pointer remains selected until select.
    /// A retained failed-call temporary is not a native header field and is
    /// intentionally not acknowledged as released by this header destructor.
    pub fn release_slot(&mut self, slot: Slot) -> Result<Vec<BackingId>> {
        ensure!(
            !self
                .trace
                .as_ref()
                .is_some_and(|trace| !trace.returned && !trace.failed),
            "cannot release a pending transform-cache invocation"
        );
        Ok(self.headers[slot.index()].release(false))
    }
    /// Source last-lease order: TP, FP, then selected=NULL. The caller must have
    /// reached actual last lease; this does not release other constructor caches.
    pub fn release_all(&mut self) -> Result<Vec<BackingId>> {
        let mut released = self.release_slot(Slot::ThirdPerson)?;
        released.extend(self.release_slot(Slot::FirstPerson)?);
        self.select(-1);
        Ok(released)
    }
}

#[cfg(test)]
mod tests;
