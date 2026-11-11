use super::{Arena, Region, wide};
use anyhow::{Context as _, Result};

pub const REFRESH_BYTES: usize = 0x2200;
/// Only reached shared fields. Unreached masks, payloads and other context
/// members are not inferred from this sparse owner.
#[derive(Default)]
pub struct Context {
    pub(super) regions: [Option<Region>; 4], // +10,+28,+14,+2c, in logical region order
    pub(super) flags: Option<u32>,
    pub(super) refreshes: u64,
}
#[derive(Debug)]
pub struct Refresh {
    owner: u64,
    allocation: u64,
    ordinal: u64,
    start: usize,
    cursor_after: usize,
    live_after: usize,
}
impl Context {
    pub fn refresh_empty(&mut self, arena: &mut Arena) -> Result<Refresh> {
        let ordinal = self
            .refreshes
            .checked_add(1)
            .context("animation refresh ordinal overflow")?;
        let mut reservation = arena
            .reserve(REFRESH_BYTES)?
            .context("shared animation scratch exhausted")?;
        let start = reservation
            .start
            .expect("successful reservation has a start");
        let result: Result<()> = (|| {
            // Source writes +28 before +10, then +14 and +2c. No scratch
            // payload, sample callback or bone commit is reached by empty lists.
            self.regions[1] = Some(arena.region(&reservation, 0x1000, 0x1000)?);
            self.regions[0] = Some(arena.region(&reservation, 0, 0x1000)?);
            self.regions[2] = Some(arena.region(&reservation, 0x2000, 0x100)?);
            self.regions[3] = Some(arena.region(&reservation, 0x2100, 0x100)?);
            self.flags = Some(0);
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
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        wide(&mut emit, self.refreshes);
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
