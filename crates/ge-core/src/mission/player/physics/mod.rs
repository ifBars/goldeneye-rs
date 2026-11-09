//! Sparse owned constructor state for the source player physics item. No
//! collision step, elapsed clock, player completion or camera is implied.
use crate::mission::world_hooks::buffer::Buffer;
use anyhow::{Context, Result, ensure};
use std::{fmt, num::NonZeroU64};

pub const SOURCE_BYTES: usize = 0x200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingHandlerKind;
impl fmt::Display for MissingHandlerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("player physics requires the actual handler +224 input")
    }
}
impl std::error::Error for MissingHandlerKind {}

pub struct Owner {
    // Written scalar bytes only. Unknown padding, pointers and vtables do not
    // become zero merely because the host uses initialized Rust storage.
    known: [Option<u8>; SOURCE_BYTES],
    item: Option<NonZeroU64>,
    buffer: Buffer,
}

impl fmt::Debug for Owner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlayerPhysicsOwner")
            .field("item", &self.item)
            .field(
                "known_scalar_bytes",
                &self.known.iter().filter(|v| v.is_some()).count(),
            )
            .field("buffer", &self.buffer)
            .finish()
    }
}

impl Owner {
    /// Source 800ee850 with its two actual nested constructors. Original native
    /// addresses are represented by semantic ownership, never fabricated bytes.
    pub fn from_constructor() -> Self {
        let mut owner = Self {
            known: [None; SOURCE_BYTES],
            item: None,
            buffer: Buffer::empty(16),
        };
        for offset in [0x8, 0xa, 0xc] {
            owner.store_half(offset, 0);
        }
        owner.store_word(0x10, 3);
        for offset in (0x14..=0x30).step_by(4) {
            owner.store_word(offset, 0);
        }
        owner.store_word(0x44, 0);
        for offset in (0x4c..=0x158).step_by(4) {
            owner.store_word(offset, 0);
        }
        for offset in [
            0x15c, 0x16c, 0x170, 0x174, 0x178, 0x17c, 0x180, 0x184, 0x188,
        ] {
            owner.store_word(offset, 0);
        }
        owner.store_word(0x160, 0x411c_cccd);
        owner.store_word(0x164, 0x3f7a_e148);
        owner.store_word(0x168, 0x3f80_0000);
        owner.store_half(0x1f8, 0);
        owner.store_byte(0x1fa, 1);
        for offset in 0x1fb..=0x1ff {
            owner.store_byte(offset, 0);
        }
        let radius = f32::from_bits(0x3f99_999a) * 0.5;
        owner.store_word(0x1cc, radius.to_bits());
        owner.store_word(0x1d0, radius.to_bits());
        owner.store_vector(0x1d4, [0., radius, 0., 0.]);
        owner.store_vector(0x18c, [0., radius, 0., 0.]);
        owner.store_word(0x19c, radius.to_bits());
        owner.store_word(0x1c8, 0);
        owner
    }

    pub fn bind_item(&mut self, item: u64) -> Result<()> {
        self.item = Some(NonZeroU64::new(item).context("physics item identity must be nonzero")?);
        Ok(())
    }
    pub fn item(&self) -> Option<u64> {
        self.item.map(NonZeroU64::get)
    }
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }
    /// Explicit supplied ownership for adapters/fixtures, not a native pointer.
    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }
    pub fn byte(&self, offset: usize) -> Option<u8> {
        self.known.get(offset).copied().flatten()
    }
    pub fn half(&self, offset: usize) -> Option<u16> {
        Some(u16::from_be_bytes([
            self.byte(offset)?,
            self.byte(offset.checked_add(1)?)?,
        ]))
    }
    pub fn word(&self, offset: usize) -> Option<u32> {
        Some(u32::from_be_bytes([
            self.byte(offset)?,
            self.byte(offset.checked_add(1)?)?,
            self.byte(offset.checked_add(2)?)?,
            self.byte(offset.checked_add(3)?)?,
        ]))
    }

    /// Source 800ef760. Finite arithmetic is the checked host subset; rejected
    /// inputs do not pretend that a source exceptional-data branch completed.
    pub fn set_height_radius(&mut self, height_bits: u32, radius_bits: u32) -> Result<()> {
        let height = f32::from_bits(height_bits);
        let radius = f32::from_bits(radius_bits);
        let half_height = height * 0.5;
        let half_radius = radius * 0.5;
        ensure!(
            [height, radius, half_height, half_radius]
                .into_iter()
                .all(f32::is_finite),
            "physics shape arithmetic exceeds the finite host subset"
        );
        self.store_word(0x1d0, half_height.to_bits());
        self.store_pair(0x1dc, [0.; 2]);
        self.store_pair(0x1d4, [0., half_height]);
        self.store_word(0x1cc, half_radius.to_bits());
        if half_height <= half_radius {
            self.store_vector(0x18c, [0., half_height, 0., 0.]);
            self.store_word(0x19c, half_height.to_bits());
            self.store_word(0x1c8, 0);
        } else {
            let span = half_height - half_radius;
            let diameter = 2. * span;
            ensure!(
                [span, diameter].into_iter().all(f32::is_finite),
                "physics capsule arithmetic exceeds the finite host subset"
            );
            self.store_pair(0x194, [0., 0.]);
            self.store_pair(0x18c, [0., half_height - span]);
            self.store_pair(0x19c, [0., diameter]);
            self.store_pair(0x1a4, [0., 0.]);
            self.store_word(0x1ac, half_radius.to_bits());
            self.store_word(0x1c8, 1);
        }
        Ok(())
    }

    /// An explicit raw handler input, not an inferred PlayerNew type/default.
    pub fn apply_handler_kind(&mut self, kind: Option<u32>) -> Result<()> {
        let kind = kind.ok_or(MissingHandlerKind)?;
        let value = if kind & 0xff00_0000 == 0x0100_0000 {
            4000. * f32::from_bits(self.word(0x1d0).context("physics height is unwritten")?)
        } else {
            match kind & 0xffff_0000 {
                0x4003_0000 | 0x4004_0000 => 1000.,
                0x4000_0000 | 0x4001_0000 | 0x4002_0000 => 500.,
                _ => 0.,
            }
        };
        ensure!(
            value.is_finite(),
            "physics handler scalar exceeds the finite host subset"
        );
        self.store_word(0x168, value.to_bits());
        Ok(())
    }

    /// Source 80100db0..e00 ordering. The caller owns the real world membership
    /// operation (800f24a0); failure keeps earlier stores and skips the suffix.
    pub fn apply_player_initialization(
        &mut self,
        kind: Option<u32>,
        activate: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        ensure!(self.item.is_some(), "physics item owner is not bound");
        self.store_word(0x160, 30f32.to_bits());
        self.set_height_radius(0x3fd3_3333, 1f32.to_bits())?;
        self.apply_handler_kind(kind)?;
        activate()?;
        self.store_byte(0x1ff, 1);
        self.store_word(0x180, 2f32.to_bits());
        self.store_half(
            0x8,
            self.half(0x8).context("physics flags are unwritten")? | 0x4000,
        );
        Ok(())
    }

    /// Canonical private fingerprint input: version, byte count, ordered
    /// (offset,presence,value), semantic item presence/id high/low, then buffer
    /// capacity/growth/count, backing presence/length/capacity high/low and bytes.
    /// This emits owned state, not an original native image or addresses.
    pub fn visit_state_words(&self, mut visit: impl FnMut(u32)) {
        visit(1);
        visit(SOURCE_BYTES as u32);
        for (offset, value) in self.known.iter().enumerate() {
            visit(offset as u32);
            visit(u32::from(value.is_some()));
            if let Some(value) = value {
                visit(u32::from(*value));
            }
        }
        visit(u32::from(self.item.is_some()));
        if let Some(item) = self.item {
            visit((item.get() >> 32) as u32);
            visit(item.get() as u32);
        }
        visit(self.buffer.capacity());
        visit(self.buffer.growth());
        visit(self.buffer.count());
        visit(u32::from(self.buffer.has_backing()));
        for value in [self.buffer.backing_len(), self.buffer.backing_capacity()] {
            visit(((value as u64) >> 32) as u32);
            visit(value as u32);
        }
        if let Some(bytes) = self.buffer.backing() {
            for byte in bytes {
                visit(u32::from(*byte));
            }
        }
    }
    fn store_byte(&mut self, offset: usize, value: u8) {
        self.known[offset] = Some(value);
    }
    fn store_half(&mut self, offset: usize, value: u16) {
        for (i, byte) in value.to_be_bytes().into_iter().enumerate() {
            self.store_byte(offset + i, byte);
        }
    }
    fn store_word(&mut self, offset: usize, value: u32) {
        for (i, byte) in value.to_be_bytes().into_iter().enumerate() {
            self.store_byte(offset + i, byte);
        }
    }
    fn store_vector(&mut self, offset: usize, value: [f32; 4]) {
        for (i, value) in value.into_iter().enumerate() {
            self.store_word(offset + i * 4, value.to_bits());
        }
    }
    fn store_pair(&mut self, offset: usize, value: [f32; 2]) {
        for (i, value) in value.into_iter().enumerate() {
            self.store_word(offset + i * 4, value.to_bits());
        }
    }
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
