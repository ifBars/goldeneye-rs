//! Lazy mesh component-five color frames used by frontend frame-selection tracks.
//! Positions, UVs and texture bindings remain the ordinary mesh's indexed data.
use crate::binary::Bytes;
use anyhow::{Context, Result, bail, ensure};

#[derive(Clone, Copy)]
pub struct ColorFrames<'a> {
    data: &'a [u8],
    pub address: usize,
    frame_count: u32,
}

/// Find component type five in a 0x601 mesh's tagged extension words. The u16
/// presence mask at +0xb4 gates all extension reads; it is distinct from the
/// common spatial component table at +0x40. The explicit host work limit bounds
/// the native scan without assuming sorted tags, uniqueness or a list length.
pub fn read_at(
    data: &[u8],
    mesh_address: usize,
    maximum_component_words: usize,
) -> Result<Option<ColorFrames<'_>>> {
    let b = Bytes(data);
    ensure!(
        b.u32(mesh_address)? == 0x601,
        "color frames require a 0x601 mesh"
    );
    let mask = b.u16(field(mesh_address, 0xb4)?)?;
    if mask & 0x20 == 0 {
        return Ok(None);
    }
    let mut at = field(mesh_address, 0xb8)?;
    for _ in 0..maximum_component_words {
        let word = b.u32(at)?;
        if word as u8 == 5 {
            let target = at as i64 + i64::from((word as i32) >> 8);
            ensure!(target >= 0, "color-frame component pointer underflow");
            let address =
                usize::try_from(target).context("color-frame component pointer overflow")?;
            let frame_count = b.u32(address)?;
            ensure!(frame_count != 0, "empty color-frame component");
            return Ok(Some(ColorFrames {
                data,
                address,
                frame_count,
            }));
        }
        at = field(at, 4)?;
    }
    bail!("color-frame component scan budget exhausted")
}

fn field(address: usize, offset: usize) -> Result<usize> {
    address
        .checked_add(offset)
        .context("color-frame address overflow")
}

impl ColorFrames<'_> {
    pub fn frame_count(self) -> u32 {
        self.frame_count
    }

    /// Each frame table entry is a signed 32-bit displacement relative to that
    /// entry itself. Null is retained as None, never replaced with the base pool.
    /// Other frame entries and all color bytes remain unread.
    pub fn frame_address(self, frame: u32) -> Result<Option<usize>> {
        ensure!(frame < self.frame_count, "color frame index out of range");
        let offset = usize::try_from(frame)
            .context("color frame index overflow")?
            .checked_mul(4)
            .context("color-frame table offset overflow")?;
        let at = field(field(self.address, 4)?, offset)?;
        let b = Bytes(self.data);
        if b.u32(at)? == 0 {
            return Ok(None);
        }
        Ok(Some(b.relative(at)?))
    }

    /// Decode one addressed RGBA color. No serialized per-frame color count is
    /// established, so the original corner's independent color index supplies
    /// the address; it must not be substituted with its position/UV index.
    pub fn color(self, frame: u32, color_index: u16) -> Result<[u8; 4]> {
        let address = self
            .frame_address(frame)?
            .context("null addressed color frame")?;
        let at = field(address, usize::from(color_index) * 4)?;
        Ok(Bytes(self.data).range(at, 4)?.try_into()?)
    }
}

#[cfg(test)]
#[path = "tests/frontend_morph.rs"]
mod tests;
