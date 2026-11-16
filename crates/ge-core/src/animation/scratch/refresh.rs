use super::{Arena, Region, Reservation, wide};
use anyhow::{Context as _, Result, ensure};

pub const REFRESH_BYTES: usize = 0x2200;
/// Only reached shared fields. Unreached payloads and other context members
/// are not inferred from this sparse owner. Masks stay unknown until the
/// nonempty evaluator's reached clears.
#[derive(Default)]
pub struct Context {
    /// Host provenance; excluded from source state hashes and public receipts.
    pub(super) identity: std::sync::Arc<()>,
    pub(super) regions: [Option<Region>; 4], // +10,+28,+14,+2c, in logical region order
    pub(super) flags: Option<u32>,
    pub(super) masks: [[Option<u32>; 4]; 2], // 806426b0 then 806426c0 words
    /// Evaluator invocations that reserved scratch, shared by both paths.
    pub(super) refreshes: u64,
    /// Ordinal of a retained, unreleased nonempty evaluation using this context.
    pub(super) holder: Option<u64>,
}
#[derive(Debug)]
pub struct Refresh {
    pub(super) owner: u64,
    pub(super) allocation: u64,
    pub(super) ordinal: u64,
    start: usize,
    cursor_after: usize,
    live_after: usize,
}
impl Context {
    pub(super) fn next_ordinal(&self) -> Result<u64> {
        ensure!(
            self.holder.is_none(),
            "animation context is held by a retained evaluation"
        );
        self.refreshes
            .checked_add(1)
            .context("animation refresh ordinal overflow")
    }
    /// Shared leaf: +28 before +10, then +14 and +2c, then flags0. Regions are
    /// handles into the actual reservation; their payload stays unknown.
    pub(super) fn store_regions(&mut self, arena: &Arena, reservation: &Reservation) -> Result<()> {
        self.regions[1] = Some(arena.region(reservation, 0x1000, 0x1000)?);
        self.regions[0] = Some(arena.region(reservation, 0, 0x1000)?);
        self.regions[2] = Some(arena.region(reservation, 0x2000, 0x100)?);
        self.regions[3] = Some(arena.region(reservation, 0x2100, 0x100)?);
        self.flags = Some(0);
        Ok(())
    }
    pub(super) fn clear_masks(&mut self) {
        // Four words at 806426b0, then four at 806426c0, in source order.
        self.masks = [[Some(0); 4]; 2];
    }
    pub fn refresh_empty(&mut self, arena: &mut Arena) -> Result<Refresh> {
        let ordinal = self.next_ordinal()?;
        let mut reservation = arena
            .reserve(REFRESH_BYTES)?
            .context("shared animation scratch exhausted")?;
        let start = reservation
            .start
            .expect("successful reservation has a start");
        let result: Result<()> = (|| {
            // No scratch payload, sample callback or bone commit is reached by
            // empty lists.
            self.store_regions(arena, &reservation)?;
            // The one active slot has weight1 >= source epsilon0.001 and NULL
            // tail. It only clears bit8000; bit4000 stays absent, so no commit.
            self.flags = self.flags.map(|flags| flags & !0x8000);
            Ok(())
        })();
        arena.release(&mut reservation)?;
        result?;
        self.refreshes = ordinal;
        Ok(Refresh {
            owner: arena.owner,
            allocation: reservation.ordinal,
            ordinal,
            start,
            cursor_after: arena.cursor(),
            live_after: arena.live_reservations(),
        })
    }
    pub fn regions(&self) -> &[Option<Region>; 4] {
        &self.regions
    }
    pub fn flags(&self) -> Option<u32> {
        self.flags
    }
    pub fn masks(&self) -> &[[Option<u32>; 4]; 2] {
        &self.masks
    }
    pub fn holder(&self) -> Option<u64> {
        self.holder
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(2);
        wide(&mut emit, self.refreshes);
        emit(u32::from(self.holder.is_some()));
        if let Some(holder) = self.holder {
            wide(&mut emit, holder);
        }
        for word in self.masks.iter().flatten() {
            emit(u32::from(word.is_some()));
            if let Some(word) = word {
                emit(*word);
            }
        }
        emit(u32::from(self.flags.is_some()));
        if let Some(flags) = self.flags {
            emit(flags);
        }
        for region in self.regions {
            emit(u32::from(region.is_some()));
            if let Some(region) = region {
                wide(&mut emit, region.owner);
                wide(&mut emit, region.ordinal);
                emit(region.start as u32);
                emit(region.bytes as u32);
            }
        }
    }
}
impl Refresh {
    pub fn owner(&self) -> u64 {
        self.owner
    }
    /// Both operations must come from the same retained arena, in order.
    pub fn follows_in_same_arena(&self, previous: &Self) -> bool {
        self.owner == previous.owner
            && self.ordinal > previous.ordinal
            && self.allocation > previous.allocation
    }
    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }
    pub fn allocation_ordinal(&self) -> u64 {
        self.allocation
    }
    pub fn reservation_start(&self) -> usize {
        self.start
    }
    pub fn cursor_after(&self) -> usize {
        self.cursor_after
    }
    pub fn live_after(&self) -> usize {
        self.live_after
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        wide(&mut emit, self.owner);
        wide(&mut emit, self.allocation);
        wide(&mut emit, self.ordinal);
        emit(REFRESH_BYTES as u32);
        emit(self.start as u32);
        emit(self.cursor_after as u32);
        emit(self.live_after as u32);
        for offset in [0, 0x1000, 0x2000, 0x2100] {
            emit(offset);
        }
        emit(0); // reached flags after both source stores
    }
}
