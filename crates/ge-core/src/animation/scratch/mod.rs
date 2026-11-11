//! Shared animation scratch and the reached empty-control refresh. Regions are
//! semantic offsets into real aligned backing, never original addresses. The
//! source leaves context pointers after release; their regions become unreadable.
mod refresh;
use anyhow::{Context as _, Result, ensure};
pub use refresh::{Context, Refresh};

pub const ARENA_BYTES: usize = 0x4000;
#[repr(align(16))]
struct Backing([u8; ARENA_BYTES]);

#[derive(Debug)]
pub struct Reservation {
    owner: u64,
    ordinal: u64,
    start: Option<usize>,
    bytes: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    owner: u64,
    ordinal: u64,
    start: usize,
    bytes: usize,
}
pub struct Arena {
    owner: u64,
    backing: Box<Backing>,
    cursor: usize,
    scopes: Vec<(u64, usize, usize)>,
    allocations: u64,
    releases: u64,
}
impl Arena {
    /// Explicit startup-initializer snapshot. The supplied semantic identity
    /// belongs to one persistent arena; this does not import original history.
    pub fn from_constructor(owner: u64) -> Result<Self> {
        ensure!(owner != 0, "scratch arena identity is zero");
        Ok(Self {
            owner,
            backing: Box::new(Backing([0; ARENA_BYTES])),
            cursor: 0,
            scopes: Vec::new(),
            allocations: 0,
            releases: 0,
        })
    }
    pub fn capacity(&self) -> usize {
        self.backing.0.len()
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn live_reservations(&self) -> usize {
        self.scopes.len()
    }
    /// Strict native capacity comparison: an exactly full arena is rejected.
    /// Checked host overflow fails before changing the shared cursor.
    pub fn reserve(&mut self, request: usize) -> Result<Option<Reservation>> {
        ensure!(
            request <= u32::MAX as usize - 15,
            "scratch request exceeds supported native alignment"
        );
        let bytes = request
            .checked_add(15)
            .context("scratch alignment overflow")?
            & !15;
        if bytes >= self.capacity() - self.cursor {
            return Ok(None);
        }
        let ordinal = self
            .allocations
            .checked_add(1)
            .context("scratch allocation ordinal overflow")?;
        self.scopes.try_reserve(1)?;
        let start = self.cursor;
        self.scopes.push((ordinal, start, bytes));
        self.cursor += bytes;
        self.allocations = ordinal;
        Ok(Some(Reservation {
            owner: self.owner,
            ordinal,
            start: Some(start),
            bytes,
        }))
    }
    pub fn release(&mut self, reservation: &mut Reservation) -> Result<()> {
        // Source NULL scoped release returns without touching the cursor.
        let Some(start) = reservation.start else {
            return Ok(());
        };
        ensure!(
            reservation.owner == self.owner
                && self.scopes.last() == Some(&(reservation.ordinal, start, reservation.bytes)),
            "scratch release requires the matching last live reservation"
        );
        let releases = self
            .releases
            .checked_add(1)
            .context("scratch release count overflow")?;
        self.cursor -= reservation.bytes;
        self.scopes.pop();
        self.releases = releases;
        reservation.start = None;
        Ok(())
    }
    pub fn region(&self, reservation: &Reservation, offset: usize, bytes: usize) -> Result<Region> {
        let start = reservation
            .start
            .context("scratch reservation was released")?;
        ensure!(
            reservation.owner == self.owner
                && self
                    .scopes
                    .contains(&(reservation.ordinal, start, reservation.bytes))
                && offset
                    .checked_add(bytes)
                    .is_some_and(|end| end <= reservation.bytes),
            "scratch region has no matching live reservation or exceeds its bounds"
        );
        Ok(Region {
            owner: self.owner,
            ordinal: reservation.ordinal,
            start: start + offset,
            bytes,
        })
    }
    pub fn region_is_live(&self, region: Region) -> bool {
        region.owner == self.owner
            && self.scopes.iter().any(|&(ordinal, start, bytes)| {
                ordinal == region.ordinal
                    && region.start >= start
                    && region
                        .start
                        .checked_add(region.bytes)
                        .is_some_and(|end| end <= start + bytes)
            })
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        wide(&mut emit, self.owner);
        emit(self.capacity() as u32);
        emit(self.cursor as u32);
        wide(&mut emit, self.allocations);
        wide(&mut emit, self.releases);
        emit(self.scopes.len() as u32);
        for &(ordinal, start, bytes) in &self.scopes {
            wide(&mut emit, ordinal);
            emit(start as u32);
            emit(bytes as u32);
        }
        // Scratch payloads are not read on the empty path. Host zeroed backing
        // is not evidence of source payload contents and is not fingerprinted.
    }
}
fn wide(emit: &mut impl FnMut(u32), value: u64) {
    emit((value >> 32) as u32);
    emit(value as u32);
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
