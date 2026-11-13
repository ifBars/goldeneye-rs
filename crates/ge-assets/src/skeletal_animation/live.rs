//! Metadata and reached scalar fields only. Cached payload bytes do not grant
//! logical residency, and compressed/channel data is never preflighted here.
use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveBinding {
    pub directory_index: usize,
}
#[derive(Clone, Copy)]
pub struct LiveBody<'a> {
    data: &'a [u8],
    address: usize,
}
pub enum BodyState<'a> {
    Absent,
    NotResident,
    Resident(LiveBody<'a>),
}
impl LiveBody<'_> {
    pub fn frames_per_second(self) -> Result<u8> {
        Bytes(self.data).u8(self.address + 0xc)
    }
    pub fn frame_count(self) -> Result<u16> {
        Bytes(self.data).u16(self.address + 0xe)
    }
    pub fn consumer_flags(self) -> Result<u32> {
        Bytes(self.data).u32(self.address + 0x38)
    }
}
impl Edb {
    pub fn base_animation_binding(&self, reference: u32) -> Result<Option<LiveBinding>> {
        let (base, table) = self.live_animation_directory()?;
        let Some(index) = crate::reference::resolve(base, table, STRIDE, reference)? else {
            return Ok(None);
        };
        Ok(Some(LiveBinding {
            directory_index: index,
        }))
    }
    pub fn base_animation_body(&self, index: usize) -> Result<BodyState<'_>> {
        let (base, table) = self.live_animation_directory()?;
        ensure!(
            (table.count as i16) > 0 && index < usize::from(table.count),
            "cached animation directory index differs"
        );
        let at = table
            .offset
            .checked_add(
                index
                    .checked_mul(STRIDE)
                    .context("animation descriptor overflow")?,
            )
            .context("animation descriptor overflow")?;
        let b = Bytes(base);
        if b.u16(at + 4)? & 0x7fff != 0 {
            return Ok(BodyState::NotResident);
        }
        let address = b.u32(at + 8)? as usize;
        if address == 0 {
            return Ok(BodyState::Absent);
        }
        ensure!(
            b.u32(address)? == 0x102,
            "unsupported live animation body tag"
        );
        Ok(BodyState::Resident(LiveBody {
            data: base,
            address,
        }))
    }
    fn live_animation_directory(&self) -> Result<(&[u8], &Table)> {
        ensure!(
            self.header.version == 263,
            "live animations require supported EDB version"
        );
        let base = self.base_bytes()?;
        let table = self.tables.get(3).context("animation directory absent")?;
        Ok((base, table))
    }
}

#[cfg(test)]
mod tests;
